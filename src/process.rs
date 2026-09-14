use std::collections::HashMap;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

pub const MAX_PROCESS_OUTPUT: u64 = 4 * 1024 * 1024;
pub const MAX_INTERACTIVE_OUTPUT: usize = 8 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum RecvLineError {
    Timeout,
    Disconnected(String),
}

pub struct InteractiveProcess {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: mpsc::Receiver<InteractiveLine>,
    stdout_queued_bytes: Arc<AtomicUsize>,
    stderr: std::sync::Arc<Mutex<Vec<u8>>>,
}

struct InteractiveLine {
    value: Result<String, String>,
    bytes: usize,
}

impl InteractiveProcess {
    pub fn send_json_line(&mut self, value: &serde_json::Value) -> Result<(), String> {
        serde_json::to_writer(&mut self.stdin, value)
            .map_err(|error| format!("序列化 ACP 请求失败：{error}"))?;
        self.stdin
            .write_all(b"\n")
            .and_then(|_| self.stdin.flush())
            .map_err(|error| format!("写入 ACP 进程失败：{error}"))
    }

    pub fn recv_line(&self, timeout: Duration) -> Result<String, RecvLineError> {
        match self.stdout.recv_timeout(timeout) {
            Ok(line) => {
                self.stdout_queued_bytes.fetch_sub(line.bytes, Ordering::AcqRel);
                line.value.map_err(RecvLineError::Disconnected)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Err(RecvLineError::Timeout),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(RecvLineError::Disconnected("ACP 进程已退出".to_string()))
            }
        }
    }

    pub fn try_recv_line(&self) -> Result<Option<String>, RecvLineError> {
        match self.stdout.try_recv() {
            Ok(line) => {
                self.stdout_queued_bytes.fetch_sub(line.bytes, Ordering::AcqRel);
                line.value.map(Some).map_err(RecvLineError::Disconnected)
            }
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                Err(RecvLineError::Disconnected("ACP 进程已退出".to_string()))
            }
        }
    }

    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub fn process_id(&self) -> u32 {
        self.child.id()
    }

    pub fn take_stderr_text(&self) -> String {
        let bytes = self
            .stderr
            .lock()
            .map(|mut value| std::mem::take(&mut *value))
            .unwrap_or_default();
        String::from_utf8_lossy(&bytes).to_string()
    }

    pub fn terminate(&mut self) {
        terminate_process_tree(&mut self.child);
    }
}

impl Drop for InteractiveProcess {
    fn drop(&mut self) {
        if self.is_running() {
            self.terminate();
        }
    }
}

pub fn spawn_interactive(
    program: &Path,
    args: &[String],
    cwd: Option<&Path>,
) -> Result<InteractiveProcess, String> {
    let mut command = process_command(program, args);
    command
        .env("AI_CROSS_PEER", "1")
        .env("PYTHONUTF8", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("启动 {} 失败：{error}", program.display()))?;
    crate::terminal::assign_process_to_job(child.id());
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "打开 ACP stdin 失败".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "打开 ACP stdout 失败".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "打开 ACP stderr 失败".to_string())?;
    let (sender, receiver) = mpsc::channel();
    let stdout_queued_bytes = Arc::new(AtomicUsize::new(0));
    let queued_bytes = stdout_queued_bytes.clone();
    thread::spawn(move || {
        let mut marked_truncated = false;
        for line in BufReader::new(stdout).lines() {
            let item = line.map_err(|error| format!("读取 ACP stdout 失败：{error}"));
            let failed = item.is_err();
            let item_bytes = item.as_ref().map_or(0, |line| line.len() + 1);
            let reserved = queued_bytes.fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current.saturating_add(item_bytes) <= MAX_INTERACTIVE_OUTPUT)
                    .then_some(current + item_bytes)
            }).is_ok();
            if reserved {
                marked_truncated = false;
                if sender.send(InteractiveLine { value: item, bytes: item_bytes }).is_err() {
                    queued_bytes.fetch_sub(item_bytes, Ordering::AcqRel);
                    break;
                }
            } else if !marked_truncated {
                marked_truncated = true;
                let marker = "[ACP stdout 已截断]".to_string();
                let marker_bytes = marker.len() + 1;
                if queued_bytes.fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                    (current.saturating_add(marker_bytes) <= MAX_INTERACTIVE_OUTPUT)
                        .then_some(current + marker_bytes)
                }).is_ok() && sender.send(InteractiveLine { value: Ok(marker), bytes: marker_bytes }).is_err() {
                    queued_bytes.fetch_sub(marker_bytes, Ordering::AcqRel);
                    break;
                }
            }
            if failed {
                break;
            }
        }
    });
    let stderr_bytes = std::sync::Arc::new(Mutex::new(Vec::new()));
    let stderr_target = stderr_bytes.clone();
    thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut chunk = [0_u8; 8192];
        loop {
            let Ok(read) = reader.read(&mut chunk) else {
                break;
            };
            if read == 0 {
                break;
            }
            if let Ok(mut target) = stderr_target.lock() {
                let remaining = MAX_PROCESS_OUTPUT.saturating_sub(target.len() as u64) as usize;
                target.extend_from_slice(&chunk[..read.min(remaining)]);
            }
        }
    });
    Ok(InteractiveProcess {
        child,
        stdin,
        stdout: receiver,
        stdout_queued_bytes,
        stderr: stderr_bytes,
    })
}

fn process_command(program: &Path, args: &[String]) -> Command {
    #[cfg(windows)]
    let command = {
        use std::os::windows::process::CommandExt;
        let is_script = program
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| {
                value.eq_ignore_ascii_case("cmd") || value.eq_ignore_ascii_case("bat")
            });
        let mut command = if is_script {
            let mut command = Command::new("cmd.exe");
            command.args(["/d", "/s", "/c"]).arg(program).args(args);
            command
        } else {
            let mut command = Command::new(program);
            command.args(args);
            command
        };
        command.creation_flags(0x0800_0000);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = Command::new(program);
        command.args(args);
        command
    };
    command
}

#[derive(Debug)]
pub struct ProcessResult {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub truncated: bool,
    pub timeout: Duration,
}

fn running_processes() -> &'static Mutex<HashMap<String, u32>> {
    static RUNNING: OnceLock<Mutex<HashMap<String, u32>>> = OnceLock::new();
    RUNNING.get_or_init(|| Mutex::new(HashMap::new()))
}

#[allow(dead_code)]
pub fn run_process(
    program: &Path,
    args: &[String],
    input: Option<&str>,
    timeout: Duration,
    cwd: Option<&Path>,
) -> Result<ProcessResult, String> {
    run_process_registered(program, args, input, timeout, cwd, None)
}

pub fn run_process_registered(
    program: &Path,
    args: &[String],
    input: Option<&str>,
    timeout: Duration,
    cwd: Option<&Path>,
    registry_key: Option<&str>,
) -> Result<ProcessResult, String> {
    let mut command = process_command(program, args);
    command
        .env("AI_CROSS_PEER", "1")
        .env("PYTHONUTF8", "1")
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("启动 {} 失败：{error}", program.display()))?;
    if let Some(key) = registry_key {
        if let Ok(mut running) = running_processes().lock() {
            running.insert(key.to_string(), child.id());
        }
    }

    let deadline = Instant::now() + timeout;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "读取子进程 stdout 失败".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "读取子进程 stderr 失败".to_string())?;
    let stdout_reader = bounded_reader(stdout);
    let stderr_reader = bounded_reader(stderr);
    let stdin_writer = input.and_then(|input| {
        child.stdin.take().map(|mut stdin| {
            let input = input.as_bytes().to_vec();
            thread::spawn(move || match stdin.write_all(&input) {
                Err(error) if error.kind() != ErrorKind::BrokenPipe => Some(error),
                _ => None,
            })
        })
    });
    let mut timed_out = false;
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(25)),
            Ok(None) => {
                timed_out = true;
                terminate_process_tree(&mut child);
                break None;
            }
            Err(error) => {
                terminate_process_tree(&mut child);
                remove_registered(registry_key);
                return Err(format!("等待子进程失败：{error}"));
            }
        }
    };
    remove_registered(registry_key);
    let (stdout, stdout_truncated) = finish_bounded_reader(stdout_reader);
    let (stderr, stderr_truncated) = finish_bounded_reader(stderr_reader);
    let stdin_error = stdin_writer.and_then(|writer| writer.join().ok().flatten());
    if let Some(error) = stdin_error {
        return Err(format!("写入子进程 stdin 失败：{error}"));
    }
    Ok(ProcessResult {
        code,
        stdout: String::from_utf8_lossy(&stdout).to_string(),
        stderr: String::from_utf8_lossy(&stderr).to_string(),
        timed_out,
        truncated: stdout_truncated || stderr_truncated,
        timeout,
    })
}

fn remove_registered(key: Option<&str>) {
    if let (Some(key), Ok(mut running)) = (key, running_processes().lock()) {
        running.remove(key);
    }
}

pub fn cancel_registered(key: &str) -> bool {
    let pid = running_processes()
        .lock()
        .ok()
        .and_then(|running| running.get(key).copied());
    if let Some(pid) = pid {
        let _ = terminate_process_id(pid);
        true
    } else {
        false
    }
}

#[allow(dead_code)]
pub fn version_for(binary: &Path) -> Option<String> {
    let result = run_process(
        binary,
        &["--version".to_string()],
        None,
        Duration::from_secs(2),
        None,
    )
    .ok()?;
    if result.timed_out {
        return None;
    }
    let output = if result.stdout.trim().is_empty() {
        result.stderr.as_str()
    } else {
        result.stdout.as_str()
    };
    output
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| {
            let mut value = line.trim().to_string();
            value.truncate(value.floor_char_boundary(120));
            value
        })
}

type Reader = (thread::JoinHandle<()>, mpsc::Receiver<(Vec<u8>, bool)>);

fn bounded_reader<R: Read + Send + 'static>(mut reader: R) -> Reader {
    let (sender, receiver) = mpsc::channel();
    let handle = thread::spawn(move || {
        let mut output = Vec::new();
        let _ = (&mut reader)
            .take(MAX_PROCESS_OUTPUT)
            .read_to_end(&mut output);
        let mut probe = [0_u8; 1];
        let truncated = matches!(reader.read(&mut probe), Ok(n) if n > 0);
        let _ = std::io::copy(&mut reader, &mut std::io::sink());
        let _ = sender.send((output, truncated));
    });
    (handle, receiver)
}

fn finish_bounded_reader(reader: Reader) -> (Vec<u8>, bool) {
    let (handle, receiver) = reader;
    let output = receiver.recv().unwrap_or_default();
    let _ = handle.join();
    output
}

pub fn terminate_process_tree(child: &mut std::process::Child) {
    let _ = terminate_process_id(child.id());
    let _ = child.kill();
    let _ = child.wait();
}

pub fn terminate_process_id(process_id: u32) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let process_id = process_id.to_string();
        return Command::new("taskkill.exe")
            .args(["/PID", &process_id, "/T", "/F"])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
    }
    #[cfg(unix)]
    {
        // SAFETY: kill receives a process id captured from a child process and a
        // constant signal; it does not dereference pointers.
        return unsafe { libc::kill(process_id as i32, libc::SIGKILL) == 0 };
    }
    #[cfg(all(not(windows), not(unix)))]
    {
        let _ = process_id;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn shell() -> (std::path::PathBuf, Vec<String>) {
        #[cfg(windows)]
        {
            (
                std::path::PathBuf::from("pwsh.exe"),
                vec!["-NoProfile".to_string(), "-Command".to_string()],
            )
        }
        #[cfg(not(windows))]
        {
            (std::path::PathBuf::from("/bin/sh"), vec!["-c".to_string()])
        }
    }

    #[test]
    fn run_process_times_out_and_terminates_the_child_tree() {
        let marker = std::env::temp_dir().join(format!("aicross-timeout-{}", Uuid::new_v4()));
        let (program, mut args) = shell();
        #[cfg(windows)]
        args.push(format!(
            "Start-Sleep -Milliseconds 2500; Set-Content -LiteralPath '{}' -Value done",
            marker.display()
        ));
        #[cfg(not(windows))]
        args.push(format!("sleep 3; printf done > '{}'", marker.display()));
        let result = run_process(&program, &args, None, Duration::from_millis(100), None)
            .expect("timeout result");
        assert!(result.timed_out);
        thread::sleep(Duration::from_millis(3000));
        assert!(!marker.exists(), "timed-out process left a running child");
    }

    #[test]
    fn run_process_truncates_and_marks_output() {
        let (program, mut args) = shell();
        #[cfg(windows)]
        args.push(format!(
            "[Console]::Out.Write('x' * {})",
            MAX_PROCESS_OUTPUT + 128
        ));
        #[cfg(not(windows))]
        args.push(format!(
            "head -c {} /dev/zero | tr '\\0' x",
            MAX_PROCESS_OUTPUT + 128
        ));
        let result = run_process(&program, &args, None, Duration::from_secs(10), None)
            .expect("large output");
        assert_eq!(result.stdout.len(), MAX_PROCESS_OUTPUT as usize);
        assert!(result.truncated);
    }

    #[test]
    fn interactive_stdout_queue_recovers_after_oversized_unconsumed_line() {
        let Ok(python) = crate::channels::resolve_binary("python", None) else {
            eprintln!("跳过：PATH 中没有 python");
            return;
        };
        let args = vec!["-c".to_string(), format!("print('x'*{})\nprint('after')", MAX_INTERACTIVE_OUTPUT + 1024)];
        let mut process = spawn_interactive(&python, &args, None).unwrap();
        let first = process.recv_line(Duration::from_secs(5)).unwrap();
        assert_eq!(first, "[ACP stdout 已截断]");
        assert_eq!(process.recv_line(Duration::from_secs(2)).unwrap(), "after");
        process.terminate();
    }
}
