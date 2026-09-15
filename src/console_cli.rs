//! `aicross dispatch/status/adopt/cancel` 命令行入口。

use crate::console_api::ConsoleManifest;
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::Path;

pub fn run_if_subcommand(args: &[String]) -> Option<i32> {
    if !matches!(args.get(1).map(String::as_str), Some("dispatch" | "status" | "adopt" | "cancel")) {
        return None;
    }
    attach_parent_console();
    let root = match crate::paths::aicross_root() {
        Ok(root) => root,
        Err(_) => {
            eprintln!("控制台未运行");
            return Some(3);
        }
    };
    Some(run_at(args, &root, &mut std::io::stdout(), &mut std::io::stderr()))
}

fn run_at(args: &[String], root: &Path, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let manifest = match read_manifest(root) {
        Ok(value) => value,
        Err(()) => {
            let _ = writeln!(stderr, "控制台未运行");
            return 3;
        }
    };
    let request = match build_request(args) {
        Ok(value) => value,
        Err(message) => {
            let _ = writeln!(stderr, "{message}");
            return 2;
        }
    };
    let wait = request.wait;
    loop {
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
        };
        let done = response.get("status").and_then(Value::as_str).is_some_and(|status| status != "running");
        if !wait || done {
            let _ = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap_or_else(|_| "{}".into()));
            let _ = writeln!(stderr, "控制台请求完成");
            return 0;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
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
}

fn build_request(args: &[String]) -> Result<CliRequest, String> {
    match args.get(1).map(String::as_str) {
        Some("dispatch") => {
            if args.iter().any(|arg| arg == "--text") {
                return Err("派发文本必须使用 --text-file".into());
            }
            let project = option_value(args, "--project").ok_or_else(|| "缺少 --project".to_string())?;
            let text_file = option_value(args, "--text-file").ok_or_else(|| "缺少 --text-file".to_string())?;
            let text = fs::read_to_string(&text_file).map_err(|error| format!("读取文本文件失败：{error}"))?;
            let preset = option_value(args, "--preset").unwrap_or_else(|| "economy".into());
            let preset = match preset.as_str() {
                "shared" => "builtin-shared",
                "economy" => "builtin-economy",
                "blind" => "builtin-blind",
                other => other,
            };
            Ok(CliRequest {
                method: "POST".into(), path: "/v1/dispatch".into(),
                body: Some(json!({ "project": project, "preset": preset, "text": text })), wait: false,
            })
        }
        Some("status") => {
            let run_id = positional(args, 2).ok_or_else(|| "缺少 run_id".to_string())?;
            Ok(CliRequest { method: "GET".into(), path: format!("/v1/runs/{run_id}"), body: None, wait: args.iter().any(|arg| arg == "--wait") })
        }
        Some("adopt") => {
            let run_id = positional(args, 2).ok_or_else(|| "缺少 run_id".to_string())?;
            let node_id = positional(args, 3).ok_or_else(|| "缺少 node_id".to_string())?;
            Ok(CliRequest { method: "POST".into(), path: format!("/v1/runs/{run_id}/adopt"), body: Some(json!({ "node_id": node_id })), wait: false })
        }
        Some("cancel") => {
            let run_id = positional(args, 2).ok_or_else(|| "缺少 run_id".to_string())?;
            Ok(CliRequest { method: "POST".into(), path: format!("/v1/runs/{run_id}/cancel"), body: Some(json!({})), wait: false })
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

fn read_manifest(root: &Path) -> Result<ConsoleManifest, ()> {
    let value: ConsoleManifest = serde_json::from_slice(&fs::read(root.join("console.json")).map_err(|_| ())?).map_err(|_| ())?;
    if value.version != 1 || value.token.len() != 64
        || !value.token.bytes().all(|byte| byte.is_ascii_hexdigit())
        || !loopback_endpoint(&value.endpoint)
    {
        return Err(());
    }
    Ok(value)
}

fn loopback_endpoint(endpoint: &str) -> bool {
    endpoint.strip_prefix("http://127.0.0.1:")
        .and_then(|port| port.parse::<u16>().ok()).is_some()
}

enum SendError {
    Unavailable,
    Rejected(u16, String),
}

fn send(manifest: &ConsoleManifest, method: &str, path: &str, body: Option<&Value>) -> Result<Value, SendError> {
    let url = format!("{}{}", manifest.endpoint, path);
    let request = match method {
        "GET" => ureq::get(&url),
        _ => ureq::post(&url),
    }.set("Authorization", &format!("Bearer {}", manifest.token));
    let response = if let Some(body) = body {
        request.set("Content-Type", "application/json").send_string(&body.to_string())
    } else {
        request.call()
    };
    match response {
        Ok(response) => response.into_string().ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .ok_or(SendError::Unavailable),
        Err(ureq::Error::Status(status, response)) => {
            let text = response.into_string().unwrap_or_else(|_| "{}".into());
            Err(SendError::Rejected(status, text))
        }
        Err(ureq::Error::Transport(_)) => Err(SendError::Unavailable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn no_console_manifest_exits_three_with_one_line() {
        let root = std::env::temp_dir().join(format!("aicross-cli-missing-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let args = vec!["aicross".into(), "status".into(), "run-1".into()];
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(run_at(&args, &root, &mut stdout, &mut stderr), 3);
        assert!(stdout.is_empty());
        assert_eq!(String::from_utf8(stderr).unwrap(), "控制台未运行\n");
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
}
