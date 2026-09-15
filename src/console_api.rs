//! TASK-16 控制台通道。
//!
//! HTTP 路由挂在 hook server 的 loopback listener 上。本模块只保留协议校验、
//! Tauri 事件中继、run 摘要和清单生命周期；派发与图写入由前端既有流水线完成。

use crate::orchestration::Target;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

const CONSOLE_VERSION: u32 = 1;
const RUN_LIMIT: usize = 200;
const INFLIGHT_LIMIT: usize = 3;
const FRONTEND_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ConsoleManifest {
    pub version: u32,
    pub endpoint: String,
    pub token: String,
    pub pid: u32,
    pub started_at: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ConsoleDispatchRequest {
    pub project: String,
    pub text: String,
    #[serde(default)] pub preset: Option<String>,
    #[serde(default)] pub targets: Option<Vec<Target>>,
    #[serde(default)] pub attachments: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ConsoleDispatchResponse {
    pub run_id: String,
    pub accepted: bool,
    #[serde(default)] pub graph_id: String,
    #[serde(default)] pub node_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ConsoleRunNode {
    pub node_id: String,
    pub role: String,
    pub agent: String,
    pub model: String,
    pub status: String,
    pub text_excerpt: String,
    pub artifacts_dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub quota_resets_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ConsoleRun {
    pub run_id: String,
    pub graph_id: String,
    pub project: String,
    pub status: String,
    pub nodes: Vec<ConsoleRunNode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub leftover: Vec<String>,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub reason: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub in_flight_node_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ConsoleEvent {
    pub kind: String,
    pub graph_id: String,
    pub node_ids: Vec<String>,
    pub runs: Vec<ConsoleRun>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConsoleDispatchEvent {
    pub run_id: String,
    #[serde(flatten)] pub request: ConsoleDispatchRequest,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConsoleActionEvent {
    pub request_id: String,
    pub action: String,
    pub run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub node_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConsoleRunUpdate {
    pub run_id: String,
    #[serde(default)] pub graph_id: String,
    pub status: String,
    #[serde(default)] pub nodes: Vec<ConsoleRunNode>,
    #[serde(default)] pub leftover: Vec<String>,
    #[serde(default)] pub in_flight_node_ids: Vec<String>,
    #[serde(default)] pub reason: String,
}

#[derive(Clone, Debug)]
pub struct ApiError { pub status: u16, pub reason: String }

impl ApiError {
    fn new(status: u16, reason: impl Into<String>) -> Self { Self { status, reason: reason.into() } }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ApiResponse { pub status: u16, pub body: String }

pub trait ConsoleBackend: Send + Sync {
    fn dispatch(&self, request: ConsoleDispatchRequest) -> Result<ConsoleDispatchResponse, ApiError>;
    fn status(&self, run_id: &str) -> Result<ConsoleRun, ApiError>;
    fn adopt(&self, run_id: &str, node_id: &str) -> Result<ConsoleRun, ApiError>;
    fn cancel(&self, run_id: &str) -> Result<ConsoleRun, ApiError>;
}

#[derive(Clone)]
pub struct LiveBackend { app: AppHandle }

impl LiveBackend { pub fn new(app: AppHandle) -> Self { Self { app } } }

impl ConsoleBackend for LiveBackend {
    fn dispatch(&self, request: ConsoleDispatchRequest) -> Result<ConsoleDispatchResponse, ApiError> {
        relay_dispatch(request, FRONTEND_TIMEOUT, |event| self.app.emit("aicross:console-dispatch", event).map_err(|_| ()))
    }
    fn status(&self, run_id: &str) -> Result<ConsoleRun, ApiError> { run_by_id(run_id) }
    fn adopt(&self, run_id: &str, node_id: &str) -> Result<ConsoleRun, ApiError> {
        let run = run_by_id(run_id)?;
        if !run.nodes.iter().any(|node| node.node_id == node_id) {
            return Err(ApiError::new(404, format!("该 run 中没有节点：{node_id}")));
        }
        relay_action(&self.app, "adopt", run_id, Some(node_id))?;
        run_by_id(run_id)
    }
    fn cancel(&self, run_id: &str) -> Result<ConsoleRun, ApiError> {
        let run = run_by_id(run_id)?;
        if !should_relay_cancel(&run.status) { return Ok(run); }
        relay_action(&self.app, "cancel", run_id, None)?;
        run_by_id(run_id)
    }
}

fn should_relay_cancel(status: &str) -> bool { status == "running" }

pub fn route<B: ConsoleBackend>(backend: &B, token: &str, authorization: Option<&str>, method: &str, path: &str, body: &[u8]) -> ApiResponse {
    if !authorized(token, authorization) { return json_error(401, "令牌无效"); }
    let result = if method == "POST" && path == "/v1/dispatch" {
        serde_json::from_slice::<ConsoleDispatchRequest>(body)
            .map_err(|error| ApiError::new(400, format!("请求 JSON 无效：{error}")))
            .and_then(|request| backend.dispatch(request)).and_then(json_value)
    } else if method == "GET" {
        run_path(path, "", |run_id| backend.status(run_id)).and_then(json_value)
    } else if method == "POST" && path.ends_with("/adopt") {
        #[derive(Deserialize)] struct AdoptBody { node_id: String }
        serde_json::from_slice::<AdoptBody>(body)
            .map_err(|error| ApiError::new(400, format!("请求 JSON 无效：{error}")))
            .and_then(|adopt| run_path(path, "/adopt", |run_id| backend.adopt(run_id, &adopt.node_id))).and_then(json_value)
    } else if method == "POST" && path.ends_with("/cancel") {
        run_path(path, "/cancel", |run_id| backend.cancel(run_id)).and_then(json_value)
    } else { Err(ApiError::new(404, "找不到接口")) };
    match result { Ok(body) => ApiResponse { status: 200, body }, Err(error) => json_error(error.status, &error.reason) }
}

fn run_path<T>(path: &str, suffix: &str, f: impl FnOnce(&str) -> Result<T, ApiError>) -> Result<T, ApiError> {
    let rest = path.strip_prefix("/v1/runs/").ok_or_else(|| ApiError::new(404, "找不到接口"))?;
    let run_id = if suffix.is_empty() { rest } else { rest.strip_suffix(suffix).ok_or_else(|| ApiError::new(404, "找不到接口"))? };
    if run_id.is_empty() || run_id.contains('/') { return Err(ApiError::new(404, "找不到接口")); }
    f(run_id)
}

fn json_value<T: Serialize>(value: T) -> Result<String, ApiError> {
    serde_json::to_string(&value).map_err(|error| ApiError::new(500, error.to_string()))
}

fn json_error(status: u16, reason: &str) -> ApiResponse {
    ApiResponse { status, body: serde_json::json!({ "error": reason }).to_string() }
}

pub fn authorized(token: &str, authorization: Option<&str>) -> bool {
    let expected = format!("Bearer {token}");
    let supplied = authorization.unwrap_or("").as_bytes();
    let expected = expected.as_bytes();
    let mut different = supplied.len() ^ expected.len();
    for index in 0..expected.len() {
        different |= usize::from(supplied.get(index).copied().unwrap_or(0) ^ expected.get(index).copied().unwrap_or(0));
    }
    different == 0
}

fn runs() -> &'static Mutex<HashMap<String, ConsoleRun>> {
    static RUNS: OnceLock<Mutex<HashMap<String, ConsoleRun>>> = OnceLock::new();
    RUNS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn pending_dispatches() -> &'static Mutex<HashMap<String, mpsc::SyncSender<Result<(), ApiError>>>> {
    static PENDING: OnceLock<Mutex<HashMap<String, mpsc::SyncSender<Result<(), ApiError>>>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn pending_actions() -> &'static Mutex<HashMap<String, mpsc::SyncSender<Result<(), ApiError>>>> {
    static PENDING: OnceLock<Mutex<HashMap<String, mpsc::SyncSender<Result<(), ApiError>>>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

#[tauri::command]
pub fn list_console_runs() -> Vec<ConsoleRun> {
    runs().lock().map(|values| sorted_runs(&values)).unwrap_or_default()
}

fn sorted_runs(values: &HashMap<String, ConsoleRun>) -> Vec<ConsoleRun> {
    let mut out: Vec<_> = values.values().cloned().collect();
    out.sort_by(|a, b| b.started_at.cmp(&a.started_at).then_with(|| a.run_id.cmp(&b.run_id)));
    out
}

fn run_by_id(run_id: &str) -> Result<ConsoleRun, ApiError> {
    runs().lock().map_err(|_| ApiError::new(409, "运行状态不可用"))?.get(run_id).cloned()
        .ok_or_else(|| ApiError::new(404, format!("找不到 run：{run_id}")))
}

fn emit_runs(app: &AppHandle, kind: &str, graph_id: &str, node_ids: &[String]) {
    let event = ConsoleEvent { kind: kind.into(), graph_id: graph_id.into(), node_ids: node_ids.to_vec(), runs: list_console_runs() };
    let _ = app.emit("console-runs-changed", event);
}

fn relay_dispatch<F>(mut request: ConsoleDispatchRequest, timeout: Duration, emit: F) -> Result<ConsoleDispatchResponse, ApiError>
where F: FnOnce(ConsoleDispatchEvent) -> Result<(), ()> {
    if request.text.trim().is_empty() { return Err(ApiError::new(409, "派发文本不能为空")); }
    let project = fs::canonicalize(PathBuf::from(request.project.trim())).map_err(|_| ApiError::new(404, "项目目录不存在"))?;
    if !project.is_dir() { return Err(ApiError::new(404, "项目目录不存在")); }
    request.project = project.to_string_lossy().into_owned();
    let run_id = Uuid::new_v4().to_string();
    insert_inflight_run(ConsoleRun {
        run_id: run_id.clone(), graph_id: String::new(), project: request.project.clone(), status: "running".into(), nodes: Vec::new(),
        leftover: Vec::new(), started_at: Utc::now().to_rfc3339(), reason: String::new(), in_flight_node_ids: Vec::new(),
    })?;
    let (sender, receiver) = mpsc::sync_channel(1);
    pending_dispatches().lock().map_err(|_| ApiError::new(409, "运行状态不可用"))?.insert(run_id.clone(), sender);
    if emit(ConsoleDispatchEvent { run_id: run_id.clone(), request }).is_err() {
        pending_dispatches().lock().ok().and_then(|mut pending| pending.remove(&run_id));
        reject_run(&run_id, "控制台界面未就绪");
        return Err(ApiError::new(503, "控制台界面未就绪"));
    }
    let acknowledgement = receiver.recv_timeout(timeout);
    pending_dispatches().lock().ok().and_then(|mut pending| pending.remove(&run_id));
    match acknowledgement {
        Ok(Ok(())) => Ok(ConsoleDispatchResponse { run_id, accepted: true, graph_id: String::new(), node_ids: Vec::new() }),
        Ok(Err(error)) => Err(error),
        Err(_) => { reject_run(&run_id, "控制台界面未就绪"); Err(ApiError::new(503, "控制台界面未就绪")) }
    }
}

fn relay_action(app: &AppHandle, action: &str, run_id: &str, node_id: Option<&str>) -> Result<(), ApiError> {
    let request_id = Uuid::new_v4().to_string();
    let (sender, receiver) = mpsc::sync_channel(1);
    pending_actions().lock().map_err(|_| ApiError::new(409, "运行状态不可用"))?.insert(request_id.clone(), sender);
    let event = ConsoleActionEvent { request_id: request_id.clone(), action: action.into(), run_id: run_id.into(), node_id: node_id.map(str::to_string) };
    if app.emit("aicross:console-action", event).is_err() {
        pending_actions().lock().ok().and_then(|mut pending| pending.remove(&request_id));
        return Err(ApiError::new(503, "控制台界面未就绪"));
    }
    let result = receiver.recv_timeout(FRONTEND_TIMEOUT).unwrap_or_else(|_| Err(ApiError::new(503, "控制台界面未就绪")));
    pending_actions().lock().ok().and_then(|mut pending| pending.remove(&request_id));
    result
}

#[cfg(test)]
fn insert_run(run: ConsoleRun) -> Result<(), ApiError> {
    let mut values = runs().lock().map_err(|_| ApiError::new(409, "运行状态不可用"))?;
    if values.len() >= RUN_LIMIT {
        if let Some(oldest) = values.values().min_by(|a, b| a.started_at.cmp(&b.started_at).then_with(|| a.run_id.cmp(&b.run_id))).map(|run| run.run_id.clone()) { values.remove(&oldest); }
    }
    values.insert(run.run_id.clone(), run);
    Ok(())
}

fn insert_inflight_run(run: ConsoleRun) -> Result<(), ApiError> {
    let mut values = runs().lock().map_err(|_| ApiError::new(409, "运行状态不可用"))?;
    if values.values().filter(|value| value.status == "running").count() >= INFLIGHT_LIMIT {
        return Err(ApiError::new(409, format!("同时在飞 run 已达上限 {INFLIGHT_LIMIT}")));
    }
    if values.len() >= RUN_LIMIT {
        if let Some(oldest) = values.values().min_by(|a, b| a.started_at.cmp(&b.started_at).then_with(|| a.run_id.cmp(&b.run_id))).map(|value| value.run_id.clone()) { values.remove(&oldest); }
    }
    values.insert(run.run_id.clone(), run);
    Ok(())
}

fn reject_run(run_id: &str, reason: &str) {
    if let Ok(mut values) = runs().lock() {
        if let Some(run) = values.get_mut(run_id) { run.status = "rejected".into(); run.reason = reason.into(); run.in_flight_node_ids.clear(); }
    }
}

fn apply_run_update(update: ConsoleRunUpdate) -> Result<ConsoleRun, String> {
    let mut values = runs().lock().map_err(|_| "运行状态不可用".to_string())?;
    let run = values.get_mut(&update.run_id).ok_or_else(|| format!("找不到 run：{}", update.run_id))?;
    run.graph_id = update.graph_id;
    run.status = update.status;
    run.nodes = update.nodes;
    run.leftover = update.leftover;
    run.in_flight_node_ids = update.in_flight_node_ids;
    run.reason = update.reason;
    let snapshot = run.clone();
    drop(values);
    if let Ok(mut pending) = pending_dispatches().lock() {
        if let Some(sender) = pending.remove(&snapshot.run_id) {
            let result = if snapshot.status == "rejected" { Err(ApiError::new(409, snapshot.reason.clone())) } else { Ok(()) };
            let _ = sender.send(result);
        }
    }
    Ok(snapshot)
}

#[tauri::command]
pub fn console_run_update(app: AppHandle, update: ConsoleRunUpdate) -> Result<(), String> {
    let snapshot = apply_run_update(update)?;
    emit_runs(&app, "progress", &snapshot.graph_id, &snapshot.in_flight_node_ids);
    Ok(())
}

#[tauri::command]
pub fn console_action_ack(request_id: String, error: Option<String>) -> Result<(), String> {
    let sender = pending_actions().lock().map_err(|_| "运行状态不可用".to_string())?.remove(&request_id)
        .ok_or_else(|| "操作回执已过期".to_string())?;
    let result = error.map_or(Ok(()), |reason| Err(ApiError::new(409, reason)));
    let _ = sender.send(result);
    Ok(())
}

pub fn token_hex() -> String { format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()) }

pub fn manifest_filename() -> &'static str {
    if cfg!(debug_assertions) { "console.dev.json" } else { "console.json" }
}

pub fn write_manifest(port: u16, token: &str) -> Result<PathBuf, String> {
    let root = crate::paths::aicross_root()?;
    write_manifest_at(&root, port, token)
}

fn write_manifest_at(root: &Path, port: u16, token: &str) -> Result<PathBuf, String> {
    write_manifest_at_with(root, port, token, set_private_permissions)
}

fn write_manifest_at_with(root: &Path, port: u16, token: &str, harden: fn(&Path) -> Result<(), String>) -> Result<PathBuf, String> {
    fs::create_dir_all(root).map_err(|error| format!("创建控制台目录失败：{error}"))?;
    let path = root.join(manifest_filename());
    let temp = root.join(format!(".console-{}.tmp", Uuid::new_v4()));
    let manifest = ConsoleManifest { version: CONSOLE_VERSION, endpoint: format!("http://127.0.0.1:{port}"), token: token.into(), pid: std::process::id(), started_at: Utc::now().to_rfc3339() };
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    let result = (|| -> Result<(), String> {
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temp).map_err(|error| format!("创建控制台清单失败：{error}"))?;
        harden(&temp)?;
        file.write_all(&bytes).map_err(|error| format!("写入控制台清单失败：{error}"))?;
        file.sync_all().map_err(|error| format!("同步控制台清单失败：{error}"))?;
        drop(file);
        replace_file(&temp, &path)?;
        Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(&temp); }
    result?;
    Ok(path)
}

#[cfg(not(windows))]
fn replace_file(temp: &Path, path: &Path) -> Result<(), String> {
    fs::rename(temp, path).map_err(|error| format!("发布控制台清单失败：{error}"))
}

#[cfg(windows)]
fn replace_file(temp: &Path, path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH};
    let from: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe { MoveFileExW(PCWSTR(from.as_ptr()), PCWSTR(to.as_ptr()), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) }
        .map_err(|error| format!("发布控制台清单失败：{error}"))
}

pub fn remove_manifest() {
    if let Ok(root) = crate::paths::aicross_root() { let _ = remove_manifest_at(&root); }
}

fn remove_manifest_at(root: &Path) -> Result<(), String> {
    match fs::remove_file(root.join(manifest_filename())) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("删除控制台清单失败：{error}")),
    }
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| format!("设置控制台清单权限失败：{error}"))
}

#[cfg(windows)]
fn set_private_permissions(path: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let system32 = PathBuf::from(std::env::var_os("SystemRoot").ok_or_else(|| "读取 SystemRoot 失败".to_string())?).join("System32");
    let who = std::process::Command::new(system32.join("whoami.exe")).args(["/user", "/fo", "csv", "/nh"]).creation_flags(0x08000000).output()
        .map_err(|error| format!("读取当前用户失败：{error}"))?;
    if !who.status.success() { return Err("读取当前用户失败".into()); }
    let line = String::from_utf8_lossy(&who.stdout);
    let sid = line.trim().trim_matches('"').rsplit("\",\"").next().unwrap_or("").trim_matches('"');
    if !sid.starts_with("S-") { return Err("读取当前用户 SID 失败".into()); }
    let status = std::process::Command::new(system32.join("icacls.exe")).arg(path).args(windows_acl_args(sid)).creation_flags(0x08000000).status()
        .map_err(|error| format!("设置控制台清单权限失败：{error}"))?;
    if status.success() { Ok(()) } else { Err("设置控制台清单权限失败".into()) }
}

#[cfg(windows)]
fn windows_acl_args(sid: &str) -> [String; 3] { ["/grant:r".into(), format!("*{sid}:F"), "/inheritance:r".into()] }

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn clear_runs() { runs().lock().unwrap().clear(); pending_dispatches().lock().unwrap().clear(); }

    fn run(id: &str, status: &str, started: &str) -> ConsoleRun {
        ConsoleRun { run_id: id.into(), graph_id: "g".into(), project: "p".into(), status: status.into(), nodes: Vec::new(), leftover: Vec::new(), started_at: started.into(), reason: String::new(), in_flight_node_ids: Vec::new() }
    }

    #[derive(Default)]
    struct FakeBackend { run: Mutex<Option<ConsoleRun>> }

    impl ConsoleBackend for FakeBackend {
        fn dispatch(&self, _request: ConsoleDispatchRequest) -> Result<ConsoleDispatchResponse, ApiError> {
            *self.run.lock().unwrap() = Some(run("run-1", "done", "2026-09-15T00:00:00Z"));
            Ok(ConsoleDispatchResponse { run_id: "run-1".into(), accepted: true, graph_id: String::new(), node_ids: Vec::new() })
        }
        fn status(&self, id: &str) -> Result<ConsoleRun, ApiError> { self.run.lock().unwrap().clone().filter(|value| value.run_id == id).ok_or_else(|| ApiError::new(404, "missing")) }
        fn adopt(&self, id: &str, _node: &str) -> Result<ConsoleRun, ApiError> { self.status(id) }
        fn cancel(&self, id: &str) -> Result<ConsoleRun, ApiError> { self.status(id) }
    }

    #[test]
    fn constant_time_comparison_accepts_only_exact_bearer() {
        assert!(authorized("secret", Some("Bearer secret")));
        assert!(!authorized("secret", Some("Bearer secreu")));
        assert!(!authorized("secret", Some("Bearer secret-extra")));
        assert!(!authorized("secret", None));
    }

    #[test]
    fn wrong_token_is_401_before_routing() {
        assert_eq!(route(&FakeBackend::default(), "secret", Some("Bearer wrong"), "POST", "/v1/dispatch", b"not-json").status, 401);
    }

    #[test]
    fn missing_project_is_404_before_event_emit() {
        let _guard = test_guard();
        clear_runs();
        let missing = std::env::temp_dir().join(format!("aicross-missing-{}", Uuid::new_v4()));
        let error = relay_dispatch(ConsoleDispatchRequest { project: missing.to_string_lossy().into(), text: "check".into(), ..Default::default() }, Duration::from_millis(1), |_| panic!("must not emit")).unwrap_err();
        assert_eq!(error.status, 404);
    }

    #[test]
    fn event_relay_round_trip_uses_frontend_update() {
        let _guard = test_guard();
        clear_runs();
        let project = std::env::temp_dir().join(format!("aicross-relay-{}", Uuid::new_v4()));
        fs::create_dir_all(&project).unwrap();
        let response = relay_dispatch(ConsoleDispatchRequest { project: project.to_string_lossy().into(), text: "check".into(), ..Default::default() }, Duration::from_secs(1), |event| {
            std::thread::spawn(move || { apply_run_update(ConsoleRunUpdate { run_id: event.run_id, graph_id: "graph-1".into(), status: "running".into(), nodes: Vec::new(), leftover: Vec::new(), in_flight_node_ids: Vec::new(), reason: String::new() }).unwrap(); });
            Ok(())
        }).unwrap();
        assert!(response.accepted);
        let wire = serde_json::to_value(&response).unwrap();
        assert_eq!(wire["graph_id"], "");
        assert_eq!(wire["node_ids"], serde_json::json!([]));
        assert_eq!(run_by_id(&response.run_id).unwrap().graph_id, "graph-1");
        let _ = fs::remove_dir_all(project);
    }

    #[test]
    fn rejected_reason_returns_409_and_remains_in_summary() {
        let _guard = test_guard();
        clear_runs();
        let project = std::env::temp_dir().join(format!("aicross-reject-{}", Uuid::new_v4()));
        fs::create_dir_all(&project).unwrap();
        let error = relay_dispatch(ConsoleDispatchRequest { project: project.to_string_lossy().into(), text: "check".into(), ..Default::default() }, Duration::from_secs(1), |event| {
            std::thread::spawn(move || { apply_run_update(ConsoleRunUpdate { run_id: event.run_id, graph_id: "g".into(), status: "rejected".into(), nodes: Vec::new(), leftover: Vec::new(), in_flight_node_ids: Vec::new(), reason: "目标额度用尽；恢复时间 18:00".into() }).unwrap(); });
            Ok(())
        }).unwrap_err();
        assert_eq!(error.status, 409);
        assert_eq!(list_console_runs()[0].reason, "目标额度用尽；恢复时间 18:00");
        let _ = fs::remove_dir_all(project);
    }

    #[test]
    fn run_summary_evicts_oldest_over_200() {
        let _guard = test_guard();
        clear_runs();
        for index in 0..=RUN_LIMIT { insert_run(run(&format!("r{index:03}"), "done", &format!("2026-09-15T00:{index:03}:00Z"))).unwrap(); }
        let values = runs().lock().unwrap();
        assert_eq!(values.len(), RUN_LIMIT);
        assert!(!values.contains_key("r000"));
    }

    #[test]
    fn fourth_inflight_run_is_rejected() {
        let _guard = test_guard();
        clear_runs();
        for index in 0..INFLIGHT_LIMIT { insert_inflight_run(run(&format!("r{index}"), "running", &format!("{index}"))).unwrap(); }
        let error = insert_inflight_run(run("overflow", "running", "9")).unwrap_err();
        assert_eq!(error.status, 409);
    }

    #[test]
    fn quota_status_and_reset_time_are_preserved() {
        let _guard = test_guard();
        clear_runs();
        insert_run(run("quota", "running", "2026-09-15T00:00:00Z")).unwrap();
        let node = ConsoleRunNode { node_id: "n".into(), status: "quota_exhausted".into(), quota_resets_at: Some("2026-09-16T00:00:00Z".into()), ..Default::default() };
        apply_run_update(ConsoleRunUpdate { run_id: "quota".into(), graph_id: "g".into(), status: "done".into(), nodes: vec![node], leftover: vec!["未闭合".into()], in_flight_node_ids: Vec::new(), reason: String::new() }).unwrap();
        let saved = run_by_id("quota").unwrap();
        assert_eq!(saved.nodes[0].status, "quota_exhausted");
        assert_eq!(saved.nodes[0].quota_resets_at.as_deref(), Some("2026-09-16T00:00:00Z"));
        assert_eq!(saved.leftover, ["未闭合"]);
    }

    #[test]
    fn completed_cancel_does_not_relay_or_change_status() {
        assert!(should_relay_cancel("running"));
        assert!(!should_relay_cancel("done"));
        assert!(!should_relay_cancel("failed"));
        assert!(!should_relay_cancel("rejected"));
    }

    #[test]
    fn frontend_ack_timeout_is_503() {
        let _guard = test_guard();
        clear_runs();
        let project = std::env::temp_dir().join(format!("aicross-timeout-{}", Uuid::new_v4()));
        fs::create_dir_all(&project).unwrap();
        let error = relay_dispatch(ConsoleDispatchRequest { project: project.to_string_lossy().into(), text: "check".into(), ..Default::default() }, Duration::from_millis(1), |_| Ok(())).unwrap_err();
        assert_eq!(error.status, 503);
        assert_eq!(list_console_runs()[0].status, "rejected");
        let _ = fs::remove_dir_all(project);
    }

    #[test]
    fn manifest_write_delete_and_private_permissions() {
        let root = std::env::temp_dir().join(format!("aicross-manifest-{}", Uuid::new_v4()));
        fn harden_before_body(path: &Path) -> Result<(), String> {
            if fs::metadata(path).map_err(|error| error.to_string())?.len() == 0 { Ok(()) } else { Err("正文先于权限".into()) }
        }
        let path = write_manifest_at_with(&root, 43123, &"a".repeat(64), harden_before_body).unwrap();
        let parsed: ConsoleManifest = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(parsed.endpoint, "http://127.0.0.1:43123");
        write_manifest_at_with(&root, 43124, &"b".repeat(64), harden_before_body).unwrap();
        let replaced: ConsoleManifest = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(replaced.endpoint, "http://127.0.0.1:43124");
        remove_manifest_at(&root).unwrap();
        assert!(!path.exists());
        let _ = fs::remove_dir_all(root);
    }
}
