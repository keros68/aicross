//! `aicross dispatch/propose/status/adopt/cancel` 命令行入口。

use crate::console_api::ConsoleManifest;
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

pub fn run_if_subcommand(args: &[String]) -> Option<i32> {
    if !matches!(args.get(1).map(String::as_str), Some("dispatch" | "propose" | "status" | "adopt" | "cancel" | "__run")) {
        return None;
    }
    attach_parent_console();
    let root = match crate::paths::aicross_root() {
        Ok(root) => root,
        Err(_) => {
            eprintln!("引擎不可用");
            return Some(3);
        }
    };
    Some(run_at(args, &root, &mut std::io::stdout(), &mut std::io::stderr()))
}

fn run_at(args: &[String], root: &Path, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    if args.get(1).map(String::as_str) == Some("__run") {
        let Some(run_id) = args.get(2) else { return 2 };
        return match crate::engine::run_background(root, run_id) {
            Ok(()) => 0,
            Err(message) => { let _ = writeln!(stderr, "{message}"); 1 }
        };
    }
    let request = match build_request(args) {
        Ok(value) => value,
        Err(message) => {
            let _ = writeln!(stderr, "{message}");
            return 2;
        }
    };
    match args.get(1).map(String::as_str) {
        Some("dispatch") => {
            let input = match serde_json::from_value(request.body.unwrap_or_default()) {
                Ok(value) => value,
                Err(error) => { let _ = writeln!(stderr, "派发请求无效：{error}"); return 2; }
            };
            return match crate::engine::start_dispatch(root, input) {
                Ok(response) => {
                    let accepted = response.get("accepted").and_then(Value::as_bool) == Some(true);
                    let _ = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap_or_else(|_| "{}".into()));
                    let _ = writeln!(stderr, "{}", if accepted { "引擎已接收派发" } else { "派发被拒绝" });
                    if accepted { 0 } else { 1 }
                }
                Err(crate::engine::StartError::Unavailable(message)) => { let _ = writeln!(stderr, "{message}"); 3 }
            };
        }
        Some("status") => return run_status(root, args, request.timeout, stdout, stderr),
        Some("adopt") => {
            let Some(run_id) = positional(args, 2) else { return 2 };
            let Some(node_id) = positional(args, 3) else { return 2 };
            return match crate::engine::adopt(root, &run_id, &node_id) { Ok(run) => output_run(run, stdout, stderr), Err(message) => { let _=writeln!(stderr,"{message}"); 1 } };
        }
        Some("cancel") => {
            let Some(run_id) = positional(args, 2) else { return 2 };
            return match crate::engine::cancel(root, &run_id) { Ok(run) => output_run(run, stdout, stderr), Err(message) => { let _=writeln!(stderr,"{message}"); 1 } };
        }
        _ => {}
    }
    let wait = request.wait;
    let started = Instant::now();
    loop {
        let manifest = match read_manifest(root) {
            Ok(value) => value,
            Err(ManifestError::Stale) => {
                let _ = writeln!(stderr, "控制台未运行（清单已过期）");
                return 3;
            }
            Err(ManifestError::Missing) => {
                let _ = writeln!(stderr, "控制台未运行");
                return 3;
            }
        };
        let response = match send(&manifest, &request.method, &request.path, request.body.as_ref()) {
            Ok(value) => value,
            Err(SendError::Unavailable) => {
                let _ = writeln!(stderr, "控制台未运行");
                return 3;
            }
            Err(SendError::Rejected(status, body)) => {
                let _ = writeln!(stdout, "{body}");
                let _ = writeln!(stderr, "请求失败（HTTP {status}）");
                return 1;
            }
            Err(SendError::InvalidResponse(text)) => {
                let _ = writeln!(stdout, "{}", text.lines().next().unwrap_or(""));
                let _ = writeln!(stderr, "控制台响应无效");
                return 1;
            }
        };
        let done = response.get("status").and_then(Value::as_str).is_some_and(|status| status != "running");
        if !wait || done {
            let _ = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap_or_else(|_| "{}".into()));
            let _ = writeln!(stderr, "控制台请求完成");
            return 0;
        }
        if started.elapsed() >= request.timeout {
            let _ = writeln!(stderr, "等待状态超时");
            return 1;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

fn output_run(run: crate::engine::EngineRun, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let _ = writeln!(stdout, "{}", serde_json::to_string(&run).unwrap_or_else(|_| "{}".into()));
    let _ = writeln!(stderr, "引擎请求完成");
    0
}

fn run_status(root: &Path, args: &[String], timeout: Duration, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let Some(run_id) = positional(args, 2) else { return 2 };
    let wait = args.iter().any(|arg| arg == "--wait");
    let started = Instant::now();
    loop {
        let run = match crate::engine::read_run(root, &run_id) { Ok(run) => run, Err(message) => { let _=writeln!(stderr,"{message}"); return 1 } };
        if !wait || run.status != "running" { return output_run(run, stdout, stderr); }
        if started.elapsed() >= timeout { let _=writeln!(stderr,"等待状态超时"); return 1; }
        std::thread::sleep(Duration::from_millis(500));
    }
}

#[cfg(windows)]
fn attach_parent_console() {
    use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    unsafe { let _ = AttachConsole(ATTACH_PARENT_PROCESS); }
}

#[cfg(not(windows))]
fn attach_parent_console() {}

struct CliRequest {
    method: String,
    path: String,
    body: Option<Value>,
    wait: bool,
    timeout: Duration,
}

fn build_request(args: &[String]) -> Result<CliRequest, String> {
    match args.get(1).map(String::as_str) {
        Some(command @ ("dispatch" | "propose")) => {
            if args.iter().any(|arg| arg == "--text" || arg.starts_with("--text=")) {
                return Err("派发文本必须使用 --text-file".into());
            }
            if command == "propose" && args.iter().any(|arg| arg == "--reason") && option_value(args, "--reason").is_none() {
                return Err("缺少 --reason 的值".into());
            }
            let reason = if command == "propose" { option_value(args, "--reason") } else { None };
            if reason.as_deref().is_some_and(|value| value.contains(['\r', '\n'])) {
                return Err("--reason 只能是一句话".into());
            }
            let project = option_value(args, "--project").ok_or_else(|| "缺少 --project".to_string())?;
            let text_file = option_value(args, "--text-file").ok_or_else(|| "缺少 --text-file".to_string())?;
            let bytes = fs::read(&text_file).map_err(|error| format!("读取文本文件失败：{error}"))?;
            let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
            let text = String::from_utf8(bytes.to_vec()).map_err(|_| "文本文件必须是 UTF-8 编码".to_string())?;
            let preset = option_value(args, "--preset").unwrap_or_else(|| "economy".into());
            let preset = match preset.as_str() {
                "shared" => "builtin-shared",
                "economy" => "builtin-economy",
                "blind" => "builtin-blind",
                other => other,
            };
            let mut body = json!({ "project": project, "preset": preset, "text": text });
            if let Some(reason) = reason { body["reason"] = Value::String(reason); }
            Ok(CliRequest {
                method: "POST".into(), path: format!("/v1/{command}"),
                body: Some(body), wait: false, timeout: Duration::from_secs(30),
            })
        }
        Some("status") => {
            let run_id = positional(args, 2).ok_or_else(|| "缺少 run_id".to_string())?;
            let timeout = option_value(args, "--timeout").map(|value| value.parse::<u64>().map_err(|_| "--timeout 必须是正整数".to_string())).transpose()?.unwrap_or(600);
            if timeout == 0 { return Err("--timeout 必须是正整数".into()); }
            Ok(CliRequest { method: "GET".into(), path: format!("/v1/runs/{run_id}"), body: None, wait: args.iter().any(|arg| arg == "--wait"), timeout: Duration::from_secs(timeout) })
        }
        Some("adopt") => {
            let run_id = positional(args, 2).ok_or_else(|| "缺少 run_id".to_string())?;
            let node_id = positional(args, 3).ok_or_else(|| "缺少 node_id".to_string())?;
            Ok(CliRequest { method: "POST".into(), path: format!("/v1/runs/{run_id}/adopt"), body: Some(json!({ "node_id": node_id })), wait: false, timeout: Duration::from_secs(30) })
        }
        Some("cancel") => {
            let run_id = positional(args, 2).ok_or_else(|| "缺少 run_id".to_string())?;
            Ok(CliRequest { method: "POST".into(), path: format!("/v1/runs/{run_id}/cancel"), body: Some(json!({})), wait: false, timeout: Duration::from_secs(30) })
        }
        _ => Err("未知命令".into()),
    }
}

fn option_value(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .filter(|value| !value.starts_with('-'))
        .cloned()
}

fn positional(args: &[String], index: usize) -> Option<String> {
    args.get(index).filter(|arg| !arg.starts_with('-')).cloned()
}

enum ManifestError { Missing, Stale }

fn read_manifest(root: &Path) -> Result<ConsoleManifest, ManifestError> {
    read_manifest_with(root, process_alive)
}

fn read_manifest_with(root: &Path, alive: impl Fn(u32) -> bool) -> Result<ConsoleManifest, ManifestError> {
    let path = root.join(crate::console_api::manifest_filename());
    let value: ConsoleManifest = serde_json::from_slice(&fs::read(&path).map_err(|_| ManifestError::Missing)?).map_err(|_| ManifestError::Missing)?;
    if value.version != 1 || value.token.len() != 64
        || !value.token.bytes().all(|byte| byte.is_ascii_hexdigit())
        || !loopback_endpoint(&value.endpoint)
    {
        return Err(ManifestError::Missing);
    }
    if !alive(value.pid) {
        let _ = fs::remove_file(path);
        return Err(ManifestError::Stale);
    }
    Ok(value)
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    let result = unsafe { libc::kill(pid as i32, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(windows)]
fn process_alive(pid: u32) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else { return false };
    unsafe { let _ = CloseHandle(handle); }
    true
}

fn loopback_endpoint(endpoint: &str) -> bool {
    endpoint.strip_prefix("http://127.0.0.1:")
        .and_then(|port| port.parse::<u16>().ok()).is_some()
}

enum SendError {
    Unavailable,
    Rejected(u16, String),
    InvalidResponse(String),
}

fn send(manifest: &ConsoleManifest, method: &str, path: &str, body: Option<&Value>) -> Result<Value, SendError> {
    let url = format!("{}{}", manifest.endpoint, path);
    let request = match method {
        "GET" => ureq::get(&url),
        _ => ureq::post(&url),
    }.set("Authorization", &format!("Bearer {}", manifest.token)).timeout(HTTP_TIMEOUT);
    let response = if let Some(body) = body {
        request.set("Content-Type", "application/json").send_string(&body.to_string())
    } else {
        request.call()
    };
    match response {
        Ok(response) => {
            let text = response.into_string().map_err(|_| SendError::InvalidResponse(String::new()))?;
            parse_success_response(text)
        }
        Err(ureq::Error::Status(status, response)) => {
            let text = response.into_string().unwrap_or_else(|_| "{}".into());
            Err(SendError::Rejected(status, text))
        }
        Err(ureq::Error::Transport(_)) => Err(SendError::Unavailable),
    }
}

fn parse_success_response(text: String) -> Result<Value, SendError> {
    serde_json::from_str::<Value>(&text).map_err(|_| SendError::InvalidResponse(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn missing_run_is_a_business_failure_without_console_manifest() {
        let root = std::env::temp_dir().join(format!("aicross-cli-missing-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let args = vec!["aicross".into(), "status".into(), "run-1".into()];
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(run_at(&args, &root, &mut stdout, &mut stderr), 1);
        assert!(stdout.is_empty());
        assert!(String::from_utf8(stderr).unwrap().contains("读取 run 失败"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dispatch_reads_multiline_text_from_file_only() {
        let root = std::env::temp_dir().join(format!("aicross-cli-text-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let text_path = root.join("prompt.txt");
        fs::write(&text_path, "第一行\n第二行").unwrap();
        let args = vec![
            "aicross".into(), "dispatch".into(), "--project".into(), root.to_string_lossy().into(),
            "--preset".into(), "economy".into(), "--text-file".into(), text_path.to_string_lossy().into(),
        ];
        let request = build_request(&args).unwrap();
        assert_eq!(request.body.unwrap()["text"], "第一行\n第二行");
        assert!(!args.iter().any(|arg| arg.contains("第一行")));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dispatch_rejects_text_on_argv() {
        let args = vec![
            "aicross".into(), "dispatch".into(), "--project".into(), "D:/p".into(),
            "--text".into(), "第一行\n第二行".into(),
        ];
        assert!(matches!(
            build_request(&args),
            Err(error) if error == "派发文本必须使用 --text-file"
        ));
        let equals = vec!["aicross".into(), "dispatch".into(), "--text=正文".into()];
        assert!(matches!(build_request(&equals), Err(error) if error == "派发文本必须使用 --text-file"));
    }

    #[test]
    fn dispatch_strips_utf8_bom_and_rejects_non_utf8() {
        let root = std::env::temp_dir().join(format!("aicross-cli-encoding-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("prompt.txt");
        fs::write(&path, [vec![0xef, 0xbb, 0xbf], "正文".as_bytes().to_vec()].concat()).unwrap();
        let args = vec!["aicross".into(), "dispatch".into(), "--project".into(), "D:/p".into(), "--text-file".into(), path.to_string_lossy().into()];
        assert_eq!(build_request(&args).unwrap().body.unwrap()["text"], "正文");
        fs::write(&path, [0xff, 0xfe, 0x00]).unwrap();
        assert!(matches!(build_request(&args), Err(error) if error == "文本文件必须是 UTF-8 编码"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn status_wait_timeout_defaults_to_600_and_accepts_override() {
        let default = build_request(&["aicross".into(), "status".into(), "run".into(), "--wait".into()]).unwrap();
        assert_eq!(default.timeout, Duration::from_secs(600));
        let custom = build_request(&["aicross".into(), "status".into(), "run".into(), "--wait".into(), "--timeout".into(), "7".into()]).unwrap();
        assert_eq!(custom.timeout, Duration::from_secs(7));
    }

    #[test]
    fn stale_pid_removes_manifest() {
        let root = std::env::temp_dir().join(format!("aicross-cli-stale-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join(crate::console_api::manifest_filename());
        fs::write(&path, serde_json::to_vec(&ConsoleManifest { version: 1, endpoint: "http://127.0.0.1:43123".into(), token: "a".repeat(64), pid: u32::MAX, started_at: String::new() }).unwrap()).unwrap();
        assert!(matches!(read_manifest_with(&root, |_| false), Err(ManifestError::Stale)));
        assert!(!path.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn http_200_non_json_is_business_error_with_original_text() {
        assert!(matches!(parse_success_response("proxy error\nsecond".into()), Err(SendError::InvalidResponse(text)) if text == "proxy error\nsecond"));
    }

    #[test]
    fn http_client_total_timeout_is_30_seconds() {
        assert_eq!(HTTP_TIMEOUT, Duration::from_secs(30));
    }

    #[test]
    fn manifest_endpoint_must_be_ipv4_loopback() {
        assert!(loopback_endpoint("http://127.0.0.1:43123"));
        assert!(!loopback_endpoint("http://localhost:43123"));
        assert!(!loopback_endpoint("https://example.com:43123"));
    }

    #[test]
    fn status_wait_adopt_and_cancel_build_protocol_paths() {
        let status = build_request(&["aicross".into(), "status".into(), "run-1".into(), "--wait".into()]).unwrap();
        assert_eq!((status.method.as_str(), status.path.as_str(), status.wait), ("GET", "/v1/runs/run-1", true));
        let adopt = build_request(&["aicross".into(), "adopt".into(), "run-1".into(), "node-1".into()]).unwrap();
        assert_eq!((adopt.method.as_str(), adopt.path.as_str()), ("POST", "/v1/runs/run-1/adopt"));
        assert_eq!(adopt.body.unwrap()["node_id"], "node-1");
        let cancel = build_request(&["aicross".into(), "cancel".into(), "run-1".into()]).unwrap();
        assert_eq!((cancel.method.as_str(), cancel.path.as_str()), ("POST", "/v1/runs/run-1/cancel"));
    }

    #[test]
    fn propose_uses_text_file_reason_and_dispatch_exit_codes() {
        let root = std::env::temp_dir().join(format!("aicross-cli-propose-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("prompt.txt");
        fs::write(&path, "第一行\n第二行").unwrap();
        let args = vec![
            "aicross".into(), "propose".into(), "--project".into(), root.to_string_lossy().into(),
            "--preset".into(), "blind".into(), "--reason".into(), "需要独立核对".into(),
            "--text-file".into(), path.to_string_lossy().into(),
        ];
        let request = build_request(&args).unwrap();
        assert_eq!((request.method.as_str(), request.path.as_str()), ("POST", "/v1/propose"));
        assert_eq!(request.body.as_ref().unwrap()["preset"], "builtin-blind");
        assert_eq!(request.body.as_ref().unwrap()["reason"], "需要独立核对");
        assert_eq!(request.body.as_ref().unwrap()["text"], "第一行\n第二行");

        let invalid = vec!["aicross".into(), "propose".into(), "--project".into(), "D:/p".into(), "--text".into(), "正文".into()];
        assert!(matches!(build_request(&invalid), Err(error) if error == "派发文本必须使用 --text-file"));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(run_at(&invalid, &root, &mut stdout, &mut stderr), 2);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn propose_rejects_multiline_reason() {
        let args = vec![
            "aicross".into(), "propose".into(), "--project".into(), "D:/p".into(),
            "--reason".into(), "第一行\n第二行".into(), "--text-file".into(), "D:/prompt.txt".into(),
        ];
        assert!(matches!(build_request(&args), Err(error) if error == "--reason 只能是一句话"));
    }
}
