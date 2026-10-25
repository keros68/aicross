use std::collections::HashMap;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

// ── Windows：子进程绑进 kill-on-close 的 Job Object ────────────────────────
// 每个派发出的 CLI 子进程（cmd → claude.exe → node.exe）都挂到进程级 Job
// Object 上（JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE）。引擎进程退出（正常退出、
// 崩溃、任务管理器强杀）时内核关闭 job 句柄，一并终止所有后代进程。没有这
// 一层，孤儿 claude.exe 会在引擎退出后继续占着 ~/.claude/ 的会话锁。
#[cfg(target_os = "windows")]
mod windows_job {
    use std::sync::OnceLock;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE};

    /// 整个进程共用一个 job，第一次用到时创建，句柄到进程结束才由系统关闭。
    /// 句柄本身可跨线程使用，包一层让它能放进 static。
    struct SharedJob(HANDLE);
    unsafe impl Send for SharedJob {}
    unsafe impl Sync for SharedJob {}

    fn shared_job() -> Option<HANDLE> {
        static SHARED: OnceLock<Option<SharedJob>> = OnceLock::new();
        SHARED.get_or_init(create_kill_on_close_job).as_ref().map(|job| job.0)
    }

    fn create_kill_on_close_job() -> Option<SharedJob> {
        let job = unsafe { CreateJobObjectW(None, None) }
            .map_err(|error| eprintln!("创建 Job Object 失败：{error}"))
            .ok()?;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let set = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                std::ptr::addr_of!(limits).cast(),
                std::mem::size_of_val(&limits) as u32,
            )
        };
        if let Err(error) = set {
            eprintln!("设置 Job Object 失败：{error}");
            unsafe {
                let _ = CloseHandle(job);
            }
            return None;
        }
        Some(SharedJob(job))
    }

    /// 绑定失败（如老系统上已在别的 job 里）只记一行，不影响派发；正常退出时
    /// 引擎仍会自己结束子进程。
    pub fn assign_pid(pid: u32) {
        let Some(job) = shared_job() else { return };
        let process = match unsafe { OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, false, pid) } {
            Ok(process) => process,
            Err(error) => {
                eprintln!("打开子进程 {pid} 失败：{error}");
                return;
            }
        };
        if let Err(error) = unsafe { AssignProcessToJobObject(job, process) } {
            eprintln!("子进程 {pid} 绑定 Job Object 失败：{error}");
        }
        unsafe {
            let _ = CloseHandle(process);
        }
    }

    /// 只在测试里用：查某 pid 是否已挂进进程级 kill-on-close Job。
    /// `IsProcessInJob` 对着我们自己的 job 句柄查，嵌套在外层 Job 里也不误报。
    #[cfg(test)]
    pub(crate) fn is_pid_in_job(pid: u32) -> bool {
        use windows::Win32::System::JobObjects::IsProcessInJob;
        use windows::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_INFORMATION,
        };
        let Some(job) = shared_job() else { return false };
        unsafe {
            let proc = match OpenProcess(PROCESS_QUERY_INFORMATION, false, pid) {
                Ok(h) => h,
                Err(_) => return false,
            };
            let mut in_job = Default::default();
            let contained =
                IsProcessInJob(proc, job, &mut in_job).is_ok() && in_job.as_bool();
            let _ = CloseHandle(proc);
            contained
        }
    }
}

/// 把已启动的子进程绑进 kill-on-close Job Object（仅 Windows 生效）。
pub(crate) fn assign_process_to_job(pid: u32) {
    #[cfg(target_os = "windows")]
    windows_job::assign_pid(pid);
    #[cfg(not(target_os = "windows"))]
    let _ = pid;
}

pub const MAX_PROCESS_OUTPUT: u64 = 4 * 1024 * 1024;
pub const MAX_INTERACTIVE_OUTPUT: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct ProcessActivity {
    started: Arc<Mutex<Instant>>,
    last_output: Arc<Mutex<Instant>>,
    tools: Arc<Mutex<ToolActivity>>,
}

#[derive(Debug, Default)]
struct ToolActivity {
    calls: u64,
    repeated: u64,
    last_tool: String,
}

impl ProcessActivity {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            started: Arc::new(Mutex::new(now)),
            last_output: Arc::new(Mutex::new(now)),
            tools: Arc::new(Mutex::new(ToolActivity::default())),
        }
    }

    pub fn mark_output(&self) {
        if let Ok(mut last) = self.last_output.lock() {
            *last = Instant::now();
        }
    }

    pub fn record_tool_call(&self, title: &str) {
        self.mark_output();
        if let Ok(mut tools) = self.tools.lock() {
            tools.calls += 1;
            tools.repeated = if tools.last_tool == title { tools.repeated + 1 } else { 1 };
            tools.last_tool = title.to_string();
        }
    }

    pub fn record_untitled_tool_call(&self) {
        self.mark_output();
        if let Ok(mut tools) = self.tools.lock() {
            tools.calls += 1;
        }
    }

    pub fn reset_attempt(&self) {
        if let Ok(mut started) = self.started.lock() {
            *started = Instant::now();
        }
        if let Ok(mut tools) = self.tools.lock() {
            *tools = ToolActivity::default();
        }
    }

    /// 用量快照（耗时 / 空闲 / 工具调用计数）。进度事件管道随窗口界面下线
    /// （fix-40-r1 R1-9），当前只有测试读取。
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn snapshot(&self) -> (u64, u64, u64, u64, String) {
        let now = Instant::now();
        let idle = self.last_output.lock().map(|last| now.duration_since(*last)).unwrap_or_default();
        let tools = self.tools.lock().ok();
        (
            self.started.lock().map(|started| now.duration_since(*started)).unwrap_or_default().as_millis() as u64,
            idle.as_millis() as u64,
            tools.as_ref().map_or(0, |value| value.calls),
            tools.as_ref().map_or(0, |value| value.repeated),
            tools.as_ref().map_or_else(String::new, |value| value.last_tool.clone()),
        )
    }
}

fn interactive_queue_limit(marker_bytes: usize) -> usize {
    MAX_INTERACTIVE_OUTPUT.saturating_add(marker_bytes)
}

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
    #[cfg(test)]
    fn queued_stdout_bytes(&self) -> usize {
        self.stdout_queued_bytes.load(Ordering::Acquire)
    }
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
    crate::process::assign_process_to_job(child.id());
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
                    (current.saturating_add(marker_bytes) <= interactive_queue_limit(marker_bytes))
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
        // .cmd / .bat 交给标准库：它自己经 cmd.exe 启动并按批处理规则转义参数。
        // 以前手工拼 `cmd /d /s /c <程序> <参数>`，程序路径带空格（`C:\Program Files\nodejs\npm.cmd`、
        // 用户名带空格的 `%APPDATA%\npm\claude.cmd`）时 /s 剥掉引号，报「'C:\Program' 不是内部或外部命令」（09-18）。
        let mut command = Command::new(program);
        command.args(args);
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
    run_process_registered_with_activity(program, args, input, timeout, cwd, registry_key, None)
}

pub fn run_process_registered_with_activity(
    program: &Path,
    args: &[String],
    input: Option<&str>,
    timeout: Duration,
    cwd: Option<&Path>,
    registry_key: Option<&str>,
    activity: Option<ProcessActivity>,
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
    // 一次性派发与冒烟同样挂进 kill-on-close Job（TASK-42）：引擎被杀时，
    // 它正在等待的 CLI 子进程随之结束，不留继续扣额度的孤儿。孙进程按继承
    // 规则自动进入同一个 Job。非 Windows 无操作（保持现状，文档已注明）。
    assign_process_to_job(child.id());
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
    let stdout_reader = bounded_reader(stdout, activity.clone());
    let stderr_reader = bounded_reader(stderr, activity);
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

/// 跑 `<binary> <version_arg>` 取版本行；版本参数由支持清单给（TASK-61）。
#[allow(dead_code)]
pub fn version_with_arg(binary: &Path, version_arg: &str) -> Option<String> {
    let result = run_process(
        binary,
        &[version_arg.to_string()],
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

fn bounded_reader<R: Read + Send + 'static>(mut reader: R, activity: Option<ProcessActivity>) -> Reader {
    let (sender, receiver) = mpsc::channel();
    let handle = thread::spawn(move || {
        let mut output = Vec::new();
        let mut limited = (&mut reader).take(MAX_PROCESS_OUTPUT);
        let mut chunk = [0_u8; 8192];
        loop {
            let Ok(read) = limited.read(&mut chunk) else { break };
            if read == 0 { break; }
            output.extend_from_slice(&chunk[..read]);
            if let Some(activity) = &activity { activity.mark_output(); }
        }
        let mut probe = [0_u8; 1];
        let truncated = matches!(reader.read(&mut probe), Ok(n) if n > 0);
        if truncated {
            if let Some(activity) = &activity { activity.mark_output(); }
        }
        loop {
            let Ok(read) = reader.read(&mut chunk) else { break };
            if read == 0 { break; }
            if let Some(activity) = &activity { activity.mark_output(); }
        }
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

    #[cfg(windows)]
    #[test]
    fn cmd_script_under_a_path_with_spaces_runs_and_receives_arguments() {
        let dir = crate::paths::test_temp_dir().join(format!("aicross cmd {}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("echo args.cmd");
        std::fs::write(&script, "@echo off\r\necho [%~1] [%~2]\r\n").unwrap();
        let result = run_process(
            &script,
            &["hello world".to_string(), "--flag=a,b".to_string()],
            None,
            Duration::from_secs(20),
            None,
        )
        .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(result.code, Some(0), "stderr: {}", result.stderr);
        assert_eq!(result.stdout.trim(), "[hello world] [--flag=a,b]");
    }

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

    /// 一次性派发路径（run_process → run_process_registered_with_activity）
    /// spawn 后必须把子进程挂进 kill-on-close Job（TASK-42）：直启子进程再拉
    /// 一个孙子后退出，孙子靠继承留在同一个 Job 里。改回不挂 Job，这条就红。
    #[cfg(windows)]
    #[test]
    fn oneshot_grandchild_lands_in_the_kill_on_close_job() {
        let (program, mut args) = shell();
        args.push(
            "$p = Start-Process pwsh -ArgumentList '-NoProfile','-Command','Start-Sleep -Seconds 15' -PassThru -WindowStyle Hidden; Write-Output $p.Id"
                .to_string(),
        );
        let result = run_process(&program, &args, None, Duration::from_secs(60), None)
            .expect("直启子进程");
        assert_eq!(result.code, Some(0), "stderr: {}", result.stderr);
        let pid: u32 = result
            .stdout
            .trim()
            .parse()
            .expect("孙子进程 pid 应打印到 stdout");
        assert!(
            windows_job::is_pid_in_job(pid),
            "孙子进程 {pid} 不在 kill-on-close Job 里"
        );
        terminate_process_id(pid);
    }

    #[test]
    fn run_process_times_out_and_terminates_the_child_tree() {
        let marker = crate::paths::test_temp_dir().join(format!("aicross-timeout-{}", Uuid::new_v4()));
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

    #[test]
    fn interactive_stdout_truncation_marker_is_delivered_at_full_queue_boundary() {
        let Ok(python) = crate::channels::resolve_binary("python", None) else {
            eprintln!("跳过：PATH 中没有 python");
            return;
        };
        let first_len = MAX_INTERACTIVE_OUTPUT - 17;
        let args = vec!["-c".to_string(), format!("import time\nprint('x'*{first_len})\nprint('y'*32)\ntime.sleep(1)")];
        let mut process = spawn_interactive(&python, &args, None).unwrap();
        let expected = first_len + 1 + "[ACP stdout 已截断]".len() + 1;
        let deadline = Instant::now() + Duration::from_secs(10);
        while process.queued_stdout_bytes() < expected && Instant::now() < deadline {
            thread::yield_now();
        }
        assert_eq!(process.queued_stdout_bytes(), expected, "生产者应先处理完普通行与截断标记");
        assert_eq!(process.recv_line(Duration::from_secs(10)).unwrap().len(), first_len);
        assert_eq!(process.recv_line(Duration::from_secs(2)).unwrap(), "[ACP stdout 已截断]");
        assert_eq!(process.try_recv_line().unwrap(), None, "超限普通行不应入队");
        process.terminate();
    }

    #[test]
    fn oneshot_output_then_silent_two_seconds_reports_idle_from_last_output() {
        let Ok(python) = crate::channels::resolve_binary("python", None) else {
            eprintln!("跳过：PATH 中没有 python");
            return;
        };
        let activity = ProcessActivity::new();
        run_process_registered_with_activity(
            &python, &["-c".into(), "import time; print('first', flush=True); time.sleep(2.1)".into()], None,
            Duration::from_secs(5), None, None, Some(activity.clone()),
        ).unwrap();
        assert!(activity.snapshot().1 >= 2000);
    }

    #[test]
    fn oneshot_second_output_resets_idle_below_half_second() {
        let Ok(python) = crate::channels::resolve_binary("python", None) else {
            eprintln!("跳过：PATH 中没有 python");
            return;
        };
        let activity = ProcessActivity::new();
        run_process_registered_with_activity(
            &python, &["-c".into(), "import time; print('first', flush=True); time.sleep(2.1); print('second', flush=True)".into()], None,
            Duration::from_secs(5), None, None, Some(activity.clone()),
        ).unwrap();
        assert!(activity.snapshot().1 < 500);
    }

    #[test]
    fn r1_reset_attempt_clears_tool_counts_and_restarts_elapsed_only() {
        let activity = ProcessActivity::new();
        activity.record_tool_call("Read file");
        thread::sleep(Duration::from_millis(20));
        activity.reset_attempt();
        let (elapsed, idle, calls, repeated, last_tool) = activity.snapshot();
        assert!(elapsed < 100);
        assert!(idle >= 20, "重置尝试不应伪造新的子进程输出");
        assert_eq!((calls, repeated, last_tool), (0, 0, String::new()));
    }
}
