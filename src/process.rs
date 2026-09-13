use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{mpsc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

pub const MAX_PROCESS_OUTPUT: u64 = 4 * 1024 * 1024;

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
    #[cfg(windows)]
    let is_command_script = program
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
        });
    #[cfg(windows)]
    let mut command = if is_command_script {
        let mut command = Command::new("cmd.exe");
        command.args(["/d", "/s", "/c"]).arg(program).args(args);
        command
    } else {
        let mut command = Command::new(program);
        command.args(args);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = Command::new(program);
        command.args(args);
        command
    };
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
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
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
        terminate_process_id(pid)
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
}
