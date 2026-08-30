use crate::models::{Settings, TerminalRequest, ToolDefinition};
use crate::tools::resolve_binary;
use base64::Engine;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

struct PtyRecord {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
    closed: Arc<AtomicBool>,
}

struct PreparedPty {
    master: Box<dyn MasterPty + Send>,
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
    closed: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct PtyManager {
    state: Arc<Mutex<PtyState>>,
}

#[derive(Default)]
struct PtyState {
    records: HashMap<String, PtyRecord>,
    opening: HashSet<String>,
    cancelled: HashSet<String>,
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
        {
            let mut state = self.lock_state();
            if state.cancelled.remove(&request.terminal_id) {
                return Ok(());
            }
            if state.records.contains_key(&request.terminal_id)
                || !state.opening.insert(request.terminal_id.clone())
            {
                return Err("终端标识已存在".to_string());
            }
        }

        let prepared = match prepare_pty(&request, tool, project, settings) {
            Ok(prepared) => prepared,
            Err(error) => {
                let mut state = self.lock_state();
                state.opening.remove(&request.terminal_id);
                state.cancelled.remove(&request.terminal_id);
                return Err(error);
            }
        };
        let terminal_id = request.terminal_id.clone();
        let PreparedPty {
            master,
            mut reader,
            writer,
            child,
            closed,
        } = prepared;
        {
            let mut state = self.lock_state();
            state.opening.remove(&terminal_id);
            if state.cancelled.remove(&terminal_id) {
                drop(state);
                closed.store(true, Ordering::Release);
                terminate_child(&mut **lock_unpoisoned(&child));
                return Ok(());
            }
            state.records.insert(
                terminal_id.clone(),
                PtyRecord {
                    master,
                    writer,
                    child: Arc::clone(&child),
                    closed: Arc::clone(&closed),
                },
            );
        }

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

        let monitor_state = Arc::clone(&self.state);
        thread::spawn(move || loop {
            thread::sleep(Duration::from_millis(150));
            if closed.load(Ordering::Acquire) {
                return;
            }
            let exit = match lock_unpoisoned(&child).try_wait() {
                Ok(Some(status)) => Some(status.exit_code()),
                Ok(None) => None,
                Err(_) => Some(1),
            };

            if let Some(code) = exit {
                if closed.swap(true, Ordering::AcqRel) {
                    return;
                }
                lock_unpoisoned(&monitor_state).records.remove(&terminal_id);
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
        let mut state = self.lock_state();
        let record = state
            .records
            .get_mut(terminal_id)
            .ok_or_else(|| "终端已退出".to_string())?;
        record
            .writer
            .write_all(data.as_bytes())
            .and_then(|_| record.writer.flush())
            .map_err(|error| format!("写入终端失败：{error}"))
    }

    pub fn resize(&self, terminal_id: &str, rows: u16, cols: u16) -> Result<(), String> {
        let state = self.lock_state();
        let record = state
            .records
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
            .map_err(|error| format!("调整终端大小失败：{error}"))
    }

    pub fn close(&self, app: &AppHandle, terminal_id: &str) -> Result<(), String> {
        let record = {
            let mut state = self.lock_state();
            let record = state.records.remove(terminal_id);
            if record.is_none() {
                state.cancelled.insert(terminal_id.to_string());
            }
            record
        };
        if let Some(record) = record {
            record.closed.store(true, Ordering::Release);
            terminate_child(&mut **lock_unpoisoned(&record.child));
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

    fn lock_state(&self) -> std::sync::MutexGuard<'_, PtyState> {
        lock_unpoisoned(&self.state)
    }
}

fn prepare_pty(
    request: &TerminalRequest,
    tool: &ToolDefinition,
    project: &Path,
    settings: &Settings,
) -> Result<PreparedPty, String> {
    let (binary, args) = launch_command(tool, settings)?;
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: request.rows.max(1),
            cols: request.cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| format!("创建终端失败：{error}"))?;

    let mut command = CommandBuilder::new(binary);
    command.args(args);
    command.cwd(project);
    for (key, value) in &request.env {
        if !key.trim().is_empty() {
            command.env(key, value);
        }
    }

    let mut child = pair
        .slave
        .spawn_command(command)
        .map_err(|error| format!("启动失败：{error}"))?;
    drop(pair.slave);

    let reader = match pair.master.try_clone_reader() {
        Ok(reader) => reader,
        Err(error) => {
            terminate_child(&mut *child);
            return Err(format!("读取终端输出失败：{error}"));
        }
    };
    let writer = match pair.master.take_writer() {
        Ok(writer) => writer,
        Err(error) => {
            terminate_child(&mut *child);
            return Err(format!("连接终端输入失败：{error}"));
        }
    };

    Ok(PreparedPty {
        master: pair.master,
        reader,
        writer,
        child: Arc::new(Mutex::new(child)),
        closed: Arc::new(AtomicBool::new(false)),
    })
}

impl Drop for PtyManager {
    fn drop(&mut self) {
        let mut state = lock_unpoisoned(&self.state);
        for (_, record) in state.records.drain() {
            record.closed.store(true, Ordering::Release);
            terminate_child(&mut **lock_unpoisoned(&record.child));
        }
    }
}

fn launch_command(
    tool: &ToolDefinition,
    settings: &Settings,
) -> Result<(String, Vec<String>), String> {
    if tool.id == "powershell" {
        let path = crate::tools::resolve_binary_name(&settings.default_shell)
            .ok_or_else(|| format!("未检测到默认 shell：{}", settings.default_shell))?;
        return Ok((
            path.to_string_lossy().to_string(),
            tool.default_args.clone(),
        ));
    }

    let path = resolve_binary(tool).ok_or_else(|| format!("未检测到 {}", tool.name))?;
    Ok((
        path.to_string_lossy().to_string(),
        tool.default_args.clone(),
    ))
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn terminate_child(child: &mut dyn Child) {
    #[cfg(windows)]
    if let Some(process_id) = child.process_id() {
        let process_id = process_id.to_string();
        let _ = std::process::Command::new("taskkill.exe")
            .args(["/PID", &process_id, "/T", "/F"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::sync::mpsc;
    use std::time::Instant;

    #[test]
    fn powershell_launch_uses_registered_default_args() {
        let tool = ToolDefinition {
            id: "powershell".to_string(),
            name: "PowerShell".to_string(),
            binary: "ignored".to_string(),
            default_args: vec!["registered-argument".to_string()],
            description: "fixture".to_string(),
        };
        let settings = Settings {
            default_shell: if cfg!(windows) {
                "cmd.exe".to_string()
            } else {
                "sh".to_string()
            },
            ..Settings::default()
        };
        let (_, args) = launch_command(&tool, &settings).expect("resolve default shell");
        assert_eq!(args, ["registered-argument"]);
    }

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
