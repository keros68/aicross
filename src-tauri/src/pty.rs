use crate::models::{Settings, TerminalRequest, ToolDefinition};
use crate::tools::resolve_binary;
use base64::Engine;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

struct PtyRecord {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

#[derive(Default)]
pub struct PtyManager {
    records: Arc<Mutex<HashMap<String, PtyRecord>>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OutputEvent {
    terminal_id: String,
    data: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExitEvent {
    terminal_id: String,
    code: Option<u32>,
}

impl PtyManager {
    pub fn open(
        &self,
        app: AppHandle,
        request: TerminalRequest,
        tool: &ToolDefinition,
        project: &Path,
        settings: &Settings,
    ) -> Result<(), String> {
        if self
            .records
            .lock()
            .map_err(|error| error.to_string())?
            .contains_key(&request.terminal_id)
        {
            return Err("终端标识已存在".to_string());
        }

        let (binary, args) = launch_command(tool, settings)?;
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: request.rows.max(1),
                cols: request.cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| error.to_string())?;

        let mut command = CommandBuilder::new(binary);
        command.args(args);
        command.cwd(project);
        for (key, value) in &request.env {
            if !key.trim().is_empty() {
                command.env(key, value);
            }
        }

        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("启动失败：{error}"))?;
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| error.to_string())?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| error.to_string())?;
        let terminal_id = request.terminal_id.clone();

        self.records
            .lock()
            .map_err(|error| error.to_string())?
            .insert(
                request.terminal_id.clone(),
                PtyRecord {
                    master: pair.master,
                    writer,
                    child,
                },
            );

        let reader_app = app.clone();
        let reader_id = terminal_id.clone();
        thread::spawn(move || {
            let mut buffer = [0_u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(length) => {
                        let encoded =
                            base64::engine::general_purpose::STANDARD.encode(&buffer[..length]);
                        let _ = reader_app.emit(
                            "pty-output",
                            OutputEvent {
                                terminal_id: reader_id.clone(),
                                data: encoded,
                            },
                        );
                    }
                }
            }
        });

        let monitor_records = Arc::clone(&self.records);
        thread::spawn(move || loop {
            thread::sleep(Duration::from_millis(150));
            let exit = {
                let mut records = match monitor_records.lock() {
                    Ok(records) => records,
                    Err(_) => return,
                };
                let Some(record) = records.get_mut(&terminal_id) else {
                    return;
                };
                match record.child.try_wait() {
                    Ok(Some(status)) => Some(status.exit_code()),
                    Ok(None) => None,
                    Err(_) => Some(1),
                }
            };

            if let Some(code) = exit {
                if let Ok(mut records) = monitor_records.lock() {
                    records.remove(&terminal_id);
                }
                let _ = app.emit(
                    "pty-exit",
                    ExitEvent {
                        terminal_id,
                        code: Some(code),
                    },
                );
                return;
            }
        });

        Ok(())
    }

    pub fn write(&self, terminal_id: &str, data: &str) -> Result<(), String> {
        let mut records = self.records.lock().map_err(|error| error.to_string())?;
        let record = records
            .get_mut(terminal_id)
            .ok_or_else(|| "终端已退出".to_string())?;
        record
            .writer
            .write_all(data.as_bytes())
            .and_then(|_| record.writer.flush())
            .map_err(|error| error.to_string())
    }

    pub fn resize(&self, terminal_id: &str, rows: u16, cols: u16) -> Result<(), String> {
        let records = self.records.lock().map_err(|error| error.to_string())?;
        let record = records
            .get(terminal_id)
            .ok_or_else(|| "终端已退出".to_string())?;
        record
            .master
            .resize(PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| error.to_string())
    }

    pub fn close(&self, app: &AppHandle, terminal_id: &str) -> Result<(), String> {
        let record = self
            .records
            .lock()
            .map_err(|error| error.to_string())?
            .remove(terminal_id);
        if let Some(mut record) = record {
            let _ = record.child.kill();
            let _ = record.child.wait();
        }
        let _ = app.emit(
            "pty-exit",
            ExitEvent {
                terminal_id: terminal_id.to_string(),
                code: None,
            },
        );
        Ok(())
    }
}

impl Drop for PtyManager {
    fn drop(&mut self) {
        if let Ok(mut records) = self.records.lock() {
            for (_, mut record) in records.drain() {
                let _ = record.child.kill();
                let _ = record.child.wait();
            }
        }
    }
}

fn launch_command(
    tool: &ToolDefinition,
    settings: &Settings,
) -> Result<(String, Vec<String>), String> {
    if tool.id == "powershell" {
        let binary = settings.default_shell.clone();
        let args = if binary.to_ascii_lowercase().contains("powershell")
            || binary.to_ascii_lowercase().contains("pwsh")
        {
            vec!["-NoLogo".to_string()]
        } else {
            Vec::new()
        };
        return Ok((binary, args));
    }

    let path = resolve_binary(tool).ok_or_else(|| format!("未检测到 {}", tool.name))?;
    Ok((
        path.to_string_lossy().to_string(),
        tool.default_args.clone(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::sync::mpsc;
    use std::time::Instant;

    #[test]
    fn portable_pty_runs_a_child_and_reads_output() {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 20,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("open pty");
        let mut command = if cfg!(windows) {
            let mut command = CommandBuilder::new("cmd.exe");
            command.args(["/C", "echo", "aicross-pty-smoke"]);
            command
        } else {
            let mut command = CommandBuilder::new("sh");
            command.args(["-lc", "printf aicross-pty-smoke"]);
            command
        };
        command.cwd(std::env::temp_dir());
        let mut child = pair.slave.spawn_command(command).expect("spawn child");
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().expect("clone reader");
        let mut writer = pair.master.take_writer().expect("take writer");
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut buffer = [0_u8; 1024];
            loop {
                let result = reader
                    .read(&mut buffer)
                    .map(|length| buffer[..length].to_vec());
                let finished = result.as_ref().is_ok_and(Vec::is_empty) || result.is_err();
                if sender.send(result).is_err() || finished {
                    break;
                }
            }
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut output = String::new();
        while Instant::now() < deadline && !output.contains("aicross-pty-smoke") {
            let bytes = receiver
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("pty output timeout")
                .expect("read pty");
            output.push_str(&String::from_utf8_lossy(&bytes));
            if output.contains("\u{1b}[6n") {
                writer.write_all(b"\x1b[1;1R").expect("answer cursor query");
                writer.flush().expect("flush cursor answer");
            }
        }
        let _ = child.kill();
        child.wait().expect("wait child");
        assert!(
            output.contains("aicross-pty-smoke"),
            "output was {output:?}"
        );
    }
}
