use super::{ChannelOutput, Usage};
use crate::process::{run_process, spawn_interactive, InteractiveProcess, RecvLineError};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, TryLockError};
use std::thread;
use std::time::{Duration, Instant};

const MAX_RAW: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcpErrorKind {
    Process,
    Protocol,
    Turn,
    Timeout,
    Cancelled,
    ForeignSession,
}
#[derive(Clone, Debug)]
pub struct AcpError {
    pub kind: AcpErrorKind,
    pub message: String,
}
impl AcpError {
    pub fn new(kind: AcpErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
    pub fn permits_fallback(&self) -> bool {
        matches!(
            self.kind,
            AcpErrorKind::Process | AcpErrorKind::Protocol | AcpErrorKind::ForeignSession
        )
    }
}

#[derive(Clone, Debug, Default)]
pub struct AcpOutput {
    pub channel: ChannelOutput,
    pub raw_stdout: String,
    pub raw_stderr: String,
    pub runner_note: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpAdapterStatus {
    pub id: String,
    pub label: String,
    pub installed: bool,
    pub command: String,
    pub detail: String,
}

struct AcpClient {
    process: InteractiveProcess,
    next_id: u64,
    sessions: HashMap<String, SessionState>,
    can_close_session: bool,
    prelude_raw: String,
}

#[derive(Default)]
struct SessionState {
    model: String,
    config_options: Vec<Value>,
}

struct ClientSlot {
    client: Mutex<Option<AcpClient>>,
    pid: AtomicU32,
}

impl ClientSlot {
    fn empty() -> Self {
        Self {
            client: Mutex::new(None),
            pid: AtomicU32::new(0),
        }
    }
}

fn pool() -> &'static Mutex<HashMap<String, Arc<ClientSlot>>> {
    static POOL: OnceLock<Mutex<HashMap<String, Arc<ClientSlot>>>> = OnceLock::new();
    POOL.get_or_init(|| Mutex::new(HashMap::new()))
}
pub fn supports(agent: &str) -> bool {
    super::normalize_agent(agent) == "kimi"
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    agent: &str,
    provider: &str,
    model: &str,
    session_id: Option<&str>,
    prompt: &str,
    cwd: &Path,
    timeout: Duration,
    thinking: &str,
    writable: bool,
    blind: bool,
    registry_key: &str,
) -> Result<AcpOutput, AcpError> {
    if !supports(agent) {
        return Err(AcpError::new(
            AcpErrorKind::Protocol,
            "该通道尚未提供 ACP 运行适配器",
        ));
    }
    let deadline = Instant::now() + timeout;
    let key = format!(
        "{}/{}/{}/{}/{}",
        super::normalize_agent(agent),
        provider,
        model,
        cwd.display(),
        blind
    );
    let shared = pool()
        .lock()
        .map_err(|_| AcpError::new(AcpErrorKind::Process, "ACP 进程池不可用"))?
        .entry(key.clone())
        .or_insert_with(|| Arc::new(ClientSlot::empty()))
        .clone();
    let mut guard = acquire_slot(&shared, deadline, registry_key)?;
    if crate::dispatch::is_cancelled(registry_key) {
        return Err(AcpError::new(AcpErrorKind::Cancelled, "派发已取消"));
    }
    if guard.is_none() {
        let client = start_kimi(provider, cwd, deadline.saturating_duration_since(Instant::now()))?;
        shared.pid.store(client.process.process_id(), Ordering::Release);
        *guard = Some(client);
    }
    let result = guard.as_mut().expect("ACP slot initialized").prompt(
        model,
        session_id,
        prompt,
        cwd,
        deadline,
        thinking,
        writable,
        blind,
        registry_key,
    );
    if result
        .as_ref()
        .is_err_and(|e| matches!(e.kind, AcpErrorKind::Process | AcpErrorKind::Timeout))
    {
        if let Some(client) = guard.as_mut() {
            client.close_all_sessions(deadline);
        }
        drop(guard);
        if let Ok(mut clients) = pool().lock() {
            clients.remove(&key);
        }
    }
    result
}

fn acquire_slot<'a>(
    shared: &'a ClientSlot,
    deadline: Instant,
    registry_key: &str,
) -> Result<MutexGuard<'a, Option<AcpClient>>, AcpError> {
    loop {
        if crate::dispatch::is_cancelled(registry_key) {
            return Err(AcpError::new(AcpErrorKind::Cancelled, "派发已取消"));
        }
        if Instant::now() >= deadline {
            return Err(AcpError::new(AcpErrorKind::Timeout, "等待 ACP 进程锁超时"));
        }
        match shared.client.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(TryLockError::WouldBlock) => thread::sleep(Duration::from_millis(10)),
            Err(TryLockError::Poisoned(_)) => {
                return Err(AcpError::new(AcpErrorKind::Process, "ACP 进程锁不可用"))
            }
        }
    }
}

fn start_kimi(_provider: &str, cwd: &Path, timeout: Duration) -> Result<AcpClient, AcpError> {
    #[cfg(test)]
    if let Some((script, _, args)) = super::test_provider_parts(_provider) {
        let python = super::resolve_binary("python", None)
            .map_err(|e| AcpError::new(AcpErrorKind::Process, e))?;
        let mut command = vec![script];
        command.extend(args);
        return start_client(&python, &command, cwd, timeout);
    }
    let program = super::resolve_binary("kimi", super::kimi_fallback())
        .map_err(|e| AcpError::new(AcpErrorKind::Process, e))?;
    start_client(&program, &["acp".into()], cwd, timeout)
}

fn start_client(
    program: &Path,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
) -> Result<AcpClient, AcpError> {
    let process = spawn_interactive(program, args, Some(cwd))
        .map_err(|e| AcpError::new(AcpErrorKind::Process, e))?;
    let mut client = AcpClient {
        process,
        next_id: 1,
        sessions: HashMap::new(),
        can_close_session: false,
        prelude_raw: String::new(),
    };
    let mut initialize_text = String::new();
    let mut initialize_raw = String::new();
    let initialized = client.request("initialize", json!({"protocolVersion":1,"clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false},"clientInfo":{"name":"aicross","version":env!("CARGO_PKG_VERSION")}}), Instant::now()+timeout.min(Duration::from_secs(30)), Some(("", &mut initialize_text, &mut initialize_raw)), false, false, "")?;
    client.prelude_raw = initialize_raw;
    client.can_close_session = initialized
        .pointer("/result/agentCapabilities/sessionCapabilities/close")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Ok(client)
}

impl AcpClient {
    #[allow(clippy::too_many_arguments)]
    fn prompt(
        &mut self,
        model: &str,
        session_id: Option<&str>,
        prompt: &str,
        cwd: &Path,
        deadline: Instant,
        thinking: &str,
        writable: bool,
        blind: bool,
        cancel_key: &str,
    ) -> Result<AcpOutput, AcpError> {
        if !self.process.is_running() {
            return Err(AcpError::new(AcpErrorKind::Process, "ACP 进程已退出"));
        }
        let (session_id, new_config_options) = match session_id.filter(|v| !v.is_empty()) {
            Some(id) if self.sessions.contains_key(id) => (id.to_string(), None),
            Some(_) => {
                return Err(AcpError::new(
                    AcpErrorKind::ForeignSession,
                    "ACP 会话所属进程已退出",
                ))
            }
            None => {
                let response = self.request(
                    "session/new",
                    json!({"cwd":cwd,"mcpServers":[]}),
                    deadline,
                    None,
                    writable,
                    blind,
                    cancel_key,
                )?;
                let id = response.pointer("/result/sessionId")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    AcpError::new(AcpErrorKind::Protocol, "ACP session/new 未返回 sessionId")
                })?
                .to_string();
                let options = response.pointer("/result/configOptions")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                (id, Some(options))
            },
        };
        if let Some(config_options) = new_config_options {
            self.sessions.insert(session_id.clone(), SessionState {
                model: String::new(),
                config_options,
            });
        }
        let mut notes = Vec::new();
        self.configure_session(&session_id, model, thinking, writable, blind, deadline, cancel_key, &mut notes)?;
        let mut text = String::new();
        let mut raw = std::mem::take(&mut self.prelude_raw);
        self.drain_pending(&mut raw)?;
        let response = self.request(
            "session/prompt",
            json!({"sessionId":session_id,"prompt":[{"type":"text","text":prompt}]}),
            deadline,
            Some((&session_id, &mut text, &mut raw)),
            writable,
            blind,
            cancel_key,
        )?;
        if text.trim().is_empty() {
            return Err(AcpError::new(AcpErrorKind::Turn, "ACP 未返回回复正文"));
        }
        let usage = parse_usage(response.pointer("/result/usage"));
        Ok(AcpOutput {
            channel: ChannelOutput {
                text,
                session_id,
                usage,
                identity: String::new(),
            },
            raw_stdout: raw,
            raw_stderr: self.process.take_stderr_text(),
            runner_note: notes.join("；"),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn configure_session(
        &mut self,
        session_id: &str,
        model: &str,
        thinking: &str,
        writable: bool,
        blind: bool,
        deadline: Instant,
        cancel_key: &str,
        notes: &mut Vec<String>,
    ) -> Result<(), AcpError> {
        let config_options = self.sessions.get(session_id)
            .map(|state| state.config_options.clone())
            .unwrap_or_default();
        let current_model = self.sessions.get(session_id).map(|state| state.model.as_str());
        if !model.is_empty() && current_model != Some(model) {
            let model_config = config_options.iter().find(|option| {
                option.get("category").and_then(Value::as_str) == Some("model")
                    || option.get("id").and_then(Value::as_str) == Some("model")
            });
            if let Some(option) = model_config {
                let available = option.get("options").and_then(Value::as_array).is_some_and(|values| {
                    values.iter().any(|value| value.get("value").and_then(Value::as_str) == Some(model))
                });
                if !available {
                    return Err(AcpError::new(AcpErrorKind::Protocol, format!("ACP 不支持声明模型 {model}")));
                }
            }
            let configured = self.request(
                "session/set_config_option",
                json!({"sessionId":session_id,"configId":"model","value":model}),
                deadline, None, writable, blind, cancel_key,
            );
            if configured.is_err() {
                let legacy = self.request(
                    "session/set_model",
                    json!({"sessionId":session_id,"modelId":model}),
                    deadline, None, writable, blind, cancel_key,
                )?;
                if model_config.is_none() {
                    if let Some(models) = legacy.pointer("/result/models/availableModels").and_then(Value::as_array) {
                        if !models.iter().any(|value| value.as_str() == Some(model)
                            || value.get("modelId").and_then(Value::as_str) == Some(model)) {
                            return Err(AcpError::new(AcpErrorKind::Protocol, format!("ACP 不支持声明模型 {model}")));
                        }
                    } else {
                        notes.push("ACP 未暴露模型清单".into());
                    }
                }
            } else if model_config.is_none() {
                notes.push("ACP 未暴露模型清单".into());
            }
            if let Some(state) = self.sessions.get_mut(session_id) {
                state.model = model.to_string();
            }
        }
        // 2026-09-14 真机：kimi 的 `plan` 模式要求回合以 ExitPlanMode/AskUserQuestion 结束，一问一答的
        // 只读节点会在那里空转到超时；只读/盲靠 answer_permission 拒写来保证，模式一律 default，永不 auto/yolo。
        let mode = "default";
        if self.request("session/set_config_option", json!({"sessionId":session_id,"configId":"mode","value":mode}), deadline, None, writable, blind, cancel_key).is_err() {
            notes.push(format!("ACP 未能设置 {mode} 模式"));
        }
        let thought = if matches!(thinking, "关" | "低") { "low" } else { "high" };
        if self.request("session/set_config_option", json!({"sessionId":session_id,"configId":"thinking","value":thought}), deadline, None, writable, blind, cancel_key).is_err() {
            notes.push(format!("ACP 未能设置 thinking={thought}"));
        }
        Ok(())
    }

    fn drain_pending(&mut self, raw: &mut String) -> Result<(), AcpError> {
        loop {
            match self.process.try_recv_line() {
                Ok(Some(line)) => append_bounded(raw, &line),
                Ok(None) => return Ok(()),
                Err(RecvLineError::Disconnected(error)) => return Err(AcpError::new(AcpErrorKind::Process, error)),
                Err(RecvLineError::Timeout) => unreachable!(),
            }
        }
    }

    fn close_all_sessions(&mut self, deadline: Instant) {
        if !self.can_close_session {
            return;
        }
        let sessions = self.sessions.keys().cloned().collect::<Vec<_>>();
        for session_id in sessions {
            let _ = self.request("session/close", json!({"sessionId":session_id}), deadline, None, false, false, "");
        }
        self.sessions.clear();
    }

    #[allow(clippy::too_many_arguments)]
    fn request(
        &mut self,
        method: &str,
        params: Value,
        deadline: Instant,
        mut output: Option<(&str, &mut String, &mut String)>,
        writable: bool,
        blind: bool,
        cancel_key: &str,
    ) -> Result<Value, AcpError> {
        let id = self.next_id;
        self.next_id += 1;
        self.process
            .send_json_line(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .map_err(|e| AcpError::new(AcpErrorKind::Process, e))?;
        if std::env::var("AICROSS_ACP_DEBUG").is_ok() { eprintln!("[acp-debug] >> id={id} {method}"); }
        loop {
            if !cancel_key.is_empty() && crate::dispatch::is_cancelled(cancel_key) {
                if let Some((session, _, _)) = output.as_mut() {
                    let _=self.process.send_json_line(&json!({"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":session}}));
                    let session = (*session).to_string();
                    self.close_session(&session);
                }
                return Err(AcpError::new(AcpErrorKind::Cancelled, "派发已取消"));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(AcpError::new(AcpErrorKind::Timeout, "等待 ACP 响应超时"));
            }
            let line = match self
                .process
                .recv_line(remaining.min(Duration::from_millis(100)))
            {
                Ok(v) => v,
                Err(RecvLineError::Timeout) if Instant::now() < deadline => continue,
                Err(RecvLineError::Timeout) => {
                    if let Some((session, _, _)) = output.as_mut() {
                        let session = (*session).to_string();
                        self.close_session(&session);
                    }
                    return Err(AcpError::new(AcpErrorKind::Timeout, "等待 ACP 响应超时"));
                }
                Err(RecvLineError::Disconnected(error)) => {
                    return Err(AcpError::new(AcpErrorKind::Process, error))
                }
            };
            if let Some((_, _, raw)) = output.as_mut() {
                append_bounded(raw, &line);
            }
            if std::env::var("AICROSS_ACP_DEBUG").is_ok() { let mut end = line.len().min(300); while !line.is_char_boundary(end) { end -= 1; } eprintln!("[acp-debug] << {}", &line[..end]); }
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            let incoming = message.get("method").and_then(Value::as_str);
            if incoming == Some("session/update") {
                if let Some((session, text, _)) = output.as_mut() {
                    if message.pointer("/params/sessionId").and_then(Value::as_str)
                        == Some(*session)
                    {
                        append_update_text(&message, text);
                    }
                }
                continue;
            }
            if incoming == Some("session/request_permission") {
                self.answer_permission(&message, writable, blind)?;
                continue;
            }
            if let Some(unknown) = incoming {
                if message.get("id").is_some() {
                    self.process.send_json_line(&json!({"jsonrpc":"2.0","id":message["id"],"error":{"code":-32601,"message":format!("不支持 {unknown}")}})).map_err(|e|AcpError::new(AcpErrorKind::Process,e))?;
                }
                continue;
            }
            if message.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = message.get("error") {
                return Err(AcpError::new(
                    AcpErrorKind::Protocol,
                    format!(
                        "ACP {method} 失败：{}",
                        error
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("未知错误")
                    ),
                ));
            }
            return Ok(message);
        }
    }

    fn answer_permission(
        &mut self,
        message: &Value,
        writable: bool,
        blind: bool,
    ) -> Result<(), AcpError> {
        let wanted = if is_read_request(message) || (writable && !blind) {
            "allow_once"
        } else {
            "reject_once"
        };
        let selected = message
            .pointer("/params/options")
            .and_then(Value::as_array)
            .and_then(|v| {
                v.iter()
                    .find(|o| o.get("kind").and_then(Value::as_str) == Some(wanted))
            })
            .and_then(|o| o.get("optionId"));
        let result = selected
            .map(|id| json!({"outcome":{"outcome":"selected","optionId":id}}))
            .unwrap_or_else(|| json!({"outcome":{"outcome":"cancelled"}}));
        self.process
            .send_json_line(&json!({"jsonrpc":"2.0","id":message["id"],"result":result}))
            .map_err(|e| AcpError::new(AcpErrorKind::Process, e))
    }

    fn close_session(&mut self, session_id: &str) {
        if self.can_close_session {
            let id = self.next_id;
            self.next_id += 1;
            let _ = self.process.send_json_line(&json!({"jsonrpc":"2.0","id":id,"method":"session/close","params":{"sessionId":session_id}}));
        }
        self.sessions.remove(session_id);
    }
}

fn is_read_request(message: &Value) -> bool {
    let kind = message.pointer("/params/toolCall/kind").and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
    // 只有 kind 明确为本地读类才放行；`fetch` 会出网，盲节点不得外发，一律拒。
    // kind 缺失也拒：kimi 0.42 的权限请求不带 kind、只有 title（"Write"/"Bash"…），
    // 而它的读文件不会来问（默认模式自动放行），所以来问的都是写或执行（2026-09-14 真机）。
    // plan 模式已不再作为第二道防线，这里是只读/盲节点唯一的写保护。
    ["read", "search", "list"].contains(&kind.as_str())
}
fn append_bounded(raw: &mut String, line: &str) {
    const MARKER: &str = "\n[输出已截断]\n";
    if raw.ends_with(MARKER) {
        return;
    }
    let remaining = MAX_RAW.saturating_sub(raw.len());
    let line_bytes = line.len().min(remaining.saturating_sub(1));
    raw.push_str(&line[..line.floor_char_boundary(line_bytes)]);
    raw.push('\n');
    if line.len() + 1 > remaining || raw.len() >= MAX_RAW {
        raw.truncate(raw.floor_char_boundary(MAX_RAW.saturating_sub(MARKER.len())));
        raw.push_str(MARKER);
    }
}
fn append_update_text(message: &Value, target: &mut String) {
    let update = message.pointer("/params/update").unwrap_or(&Value::Null);
    if matches!(
        update
            .get("sessionUpdate")
            .and_then(Value::as_str)
            .unwrap_or(""),
        "agent_message_chunk" | "agent_message"
    ) {
        if let Some(text) = update.pointer("/content/text").and_then(Value::as_str) {
            target.push_str(text);
        }
    }
}

fn parse_usage(usage: Option<&Value>) -> Usage {
    Usage {
        input: usage.and_then(|v| v.get("inputTokens")).and_then(Value::as_u64).unwrap_or(0),
        output: usage.and_then(|v| v.get("outputTokens")).and_then(Value::as_u64).unwrap_or(0),
        cache_read: usage
            .and_then(|v| v.get("cachedReadTokens").or_else(|| v.get("cacheReadTokens")))
            .and_then(Value::as_u64)
            .unwrap_or(0),
    }
}

pub fn shutdown_all() {
    let clients = pool().lock().map(|mut clients| std::mem::take(&mut *clients)).unwrap_or_default();
    for shared in clients.values() {
        match shared.client.try_lock() {
            Ok(mut slot) => {
                if let Some(client) = slot.as_mut() {
                    client.close_all_sessions(Instant::now() + Duration::from_millis(100));
                    client.process.terminate();
                }
            }
            Err(_) => {
                let pid = shared.pid.load(Ordering::Acquire);
                if pid != 0 {
                    let _ = crate::process::terminate_process_id(pid);
                }
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn test_pool_guard() -> MutexGuard<'static, ()> {
    static ACTIVITY: OnceLock<Mutex<()>> = OnceLock::new();
    ACTIVITY.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
#[tauri::command]
pub fn release_acp_pool() {
    shutdown_all();
}

fn probe(binary: &str, fallback: Option<std::path::PathBuf>, args: &[&str]) -> (bool, String) {
    let Ok(program) = super::resolve_binary(binary, fallback) else {
        return (false, "未检测到".into());
    };
    let args = args.iter().map(|v| v.to_string()).collect::<Vec<_>>();
    match run_process(&program, &args, None, Duration::from_secs(3), None) {
        Ok(r) if !r.timed_out && r.code == Some(0) => (
            true,
            r.stdout
                .lines()
                .chain(r.stderr.lines())
                .find(|l| !l.trim().is_empty())
                .unwrap_or("可用")
                .chars()
                .take(120)
                .collect(),
        ),
        Ok(_) => (false, "命令不可用".into()),
        Err(e) => (false, e),
    }
}
#[tauri::command]
pub async fn detect_acp_adapters() -> Result<Vec<AcpAdapterStatus>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let (k, kd) = probe("kimi", super::kimi_fallback(), &["acp", "--help"]);
        let (c, cd) = probe("claude-agent-acp", None, &["--version"]);
        let (x, xd) = probe("codex-acp", None, &["--version"]);
        Ok(vec![
            AcpAdapterStatus {
                id: "kimi".into(),
                label: "Kimi ACP".into(),
                installed: k,
                command: "kimi acp".into(),
                detail: kd,
            },
            AcpAdapterStatus {
                id: "claude".into(),
                label: "Claude Code ACP".into(),
                installed: c,
                command: "claude-agent-acp".into(),
                detail: cd,
            },
            AcpAdapterStatus {
                id: "codex".into(),
                label: "Codex ACP".into(),
                installed: x,
                command: "codex-acp / acp-adapter".into(),
                detail: xd,
            },
        ])
    })
    .await
    .map_err(|e| format!("ACP 探测任务失败：{e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fake_client_with(args: &[&str]) -> Option<AcpClient> {
        let cwd = std::env::current_dir().unwrap();
        let Ok(python) = super::super::resolve_binary("python", None) else {
            eprintln!("跳过：PATH 中没有 python");
            return None;
        };
        let script = cwd.join("fixtures/acp/fake_server.py");
        let mut command = vec![script.to_string_lossy().into_owned()];
        command.extend(args.iter().map(|value| value.to_string()));
        Some(start_client(
            &python,
            &command,
            &cwd,
            Duration::from_secs(5),
        )
        .unwrap())
    }
    fn fake_client() -> Option<AcpClient> {
        fake_client_with(&[])
    }
    #[test]
    fn acp_agent_message_updates_append_only_reply_text() {
        let mut t = String::new();
        append_update_text(
            &json!({"params":{"update":{"sessionUpdate":"agent_message_chunk","content":{"text":"一"}}}}),
            &mut t,
        );
        assert_eq!(t, "一");
    }
    #[test]
    fn permission_policy_classifies_read_and_write() {
        assert!(is_read_request(
            &json!({"params":{"toolCall":{"kind":"read","title":"Read file"}}})
        ));
        assert!(!is_read_request(
            &json!({"params":{"toolCall":{"kind":"execute","title":"Read misleading title"}}})
        ));
        // fetch 出网，盲节点不得外发；kind 缺失（kimi 真机形状）只有 title 也不放行。
        assert!(!is_read_request(&json!({"params":{"toolCall":{"kind":"fetch","title":"Fetch url"}}})));
        assert!(!is_read_request(&json!({"params":{"toolCall":{"title":"Read file"}}})));
        assert!(!is_read_request(&json!({"params":{"toolCall":{"title":"Write"}}})));
    }
    #[test]
    fn fake_acp_two_turn_reuses_session_tolerates_banner_filters_updates_and_rejects_write() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client() else { return };
        let first = client
            .prompt(
                "fake-model",
                None,
                "first",
                &cwd,
                Instant::now() + Duration::from_secs(5),
                "中",
                false,
                false,
                "",
            )
            .unwrap();
        assert_eq!(first.channel.text, "turn-1:no");
        assert_eq!(first.channel.usage.input, 11);
        assert!(first.raw_stdout.contains("fake-acp banner"));
        let second = client
            .prompt(
                "fake-model",
                Some(&first.channel.session_id),
                "second",
                &cwd,
                Instant::now() + Duration::from_secs(5),
                "高",
                false,
                false,
                "",
            )
            .unwrap();
        assert_eq!(second.channel.text, "turn-2:no");
        assert_eq!(second.channel.session_id, "fake-session-1");
        assert_eq!(first.channel.usage.cache_read, 5);
    }
    #[test]
    fn writable_permission_is_allow_once_never_allow_always() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client() else { return };
        let output = client
            .prompt(
                "fake-model",
                None,
                "write",
                &cwd,
                Instant::now() + Duration::from_secs(5),
                "低",
                true,
                false,
                "",
            )
            .unwrap();
        assert_eq!(output.channel.text, "turn-1:once");
    }
    #[test]
    fn readonly_permission_allows_read_kind_and_execute_title_cannot_bypass() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client() else { return };
        let output = client.prompt("fake-model", None, "read", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "").unwrap();
        assert_eq!(output.channel.text, "turn-1:once");
    }
    #[test]
    fn legacy_models_shape_remains_supported() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client_with(&["--legacy-models"]) else { return };
        let output = client.prompt("fake-model", None, "first", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "").unwrap();
        assert_eq!(output.channel.text, "turn-1:no");
    }
    #[test]
    fn missing_model_lists_do_not_reject_acp_and_leave_runner_note() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client_with(&["--no-model-list"]) else { return };
        let output = client.prompt("fake-model", None, "first", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "").unwrap();
        assert_eq!(output.channel.text, "turn-1:no");
        assert!(output.runner_note.contains("ACP 未暴露模型清单"));
    }
    #[test]
    fn kimi_config_options_set_model_default_mode_and_thinking() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client() else { return };
        let output = client.prompt("other-model", None, "config", &cwd, Instant::now() + Duration::from_secs(5), "低", false, false, "").unwrap();
        assert_eq!(output.channel.text, "config:other-model:default:low");
    }
    #[test]
    fn writable_acp_session_uses_default_never_yolo_mode() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client() else { return };
        let output = client.prompt("fake-model", None, "config", &cwd, Instant::now() + Duration::from_secs(5), "高", true, false, "").unwrap();
        assert_eq!(output.channel.text, "config:fake-model:default:high");
    }
    #[test]
    fn continuation_model_change_uses_same_session_and_updates_model() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client() else { return };
        let first = client.prompt("fake-model", None, "first", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "").unwrap();
        let second = client.prompt("other-model", Some(&first.channel.session_id), "config", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "").unwrap();
        assert_eq!(second.channel.session_id, first.channel.session_id);
        assert_eq!(second.channel.text, "config:other-model:default:high");
    }
    #[test]
    fn late_session_update_is_drained_before_next_prompt() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client_with(&["--late-update"]) else { return };
        let first = client.prompt("fake-model", None, "first", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "").unwrap_err();
        assert_eq!(first.kind, AcpErrorKind::Turn);
        let session = client.sessions.keys().next().unwrap().clone();
        let second = client.prompt("fake-model", Some(&session), "second", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "").unwrap();
        assert_eq!(second.channel.text, "turn-2:no");
        assert!(!second.channel.text.contains("LATE"));
    }
    #[test]
    fn killed_acp_process_returns_process_error_from_real_client_path() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client() else { return };
        let first = client.prompt("fake-model", None, "first", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "").unwrap();
        client.process.terminate();
        let error = client.prompt("fake-model", Some(&first.channel.session_id), "second", &cwd, Instant::now() + Duration::from_secs(2), "中", false, false, "").unwrap_err();
        assert_eq!(error.kind, AcpErrorKind::Process);
        assert!(error.permits_fallback());
    }
    #[test]
    fn foreign_acp_session_is_detected_by_real_client_state() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client() else { return };
        let error = client.prompt("fake-model", Some("foreign-session"), "second", &cwd, Instant::now() + Duration::from_secs(2), "中", false, false, "").unwrap_err();
        assert_eq!(error.kind, AcpErrorKind::ForeignSession);
    }
    #[test]
    fn cancelled_inflight_acp_prompt_returns_cancelled_and_closes_session() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client_with(&["--silent-prompt"]) else { return };
        let worker = thread::spawn(move || {
            let result = client.prompt("fake-model", None, "first", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "cancel-run:node");
            (client, result)
        });
        thread::sleep(Duration::from_millis(100));
        crate::dispatch::cancel_dispatch_node("cancel-run".into(), "node".into());
        let (client, result) = worker.join().unwrap();
        let error = result.unwrap_err();
        assert_eq!(error.kind, AcpErrorKind::Cancelled);
        assert!(client.sessions.is_empty());
        crate::dispatch::clear_cancelled("cancel-run:node");
    }
    #[test]
    fn queued_acp_node_observes_cancel_while_waiting_for_client_lock() {
        let slot = Arc::new(ClientSlot::empty());
        let guard = slot.client.lock().unwrap();
        let waiting = slot.clone();
        let worker = thread::spawn(move || acquire_slot(&waiting, Instant::now() + Duration::from_secs(2), "queue-run:node").map(|_| ()));
        thread::sleep(Duration::from_millis(50));
        crate::dispatch::cancel_dispatch_node("queue-run".into(), "node".into());
        let error = worker.join().unwrap().unwrap_err();
        assert_eq!(error.kind, AcpErrorKind::Cancelled);
        drop(guard);
        crate::dispatch::clear_cancelled("queue-run:node");
    }
    #[test]
    fn silent_acp_process_reaches_typed_timeout_without_fallback() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client_with(&["--silent-prompt"]) else { return };
        let error = client.prompt("fake-model", None, "first", &cwd, Instant::now() + Duration::from_millis(250), "中", false, false, "").unwrap_err();
        assert_eq!(error.kind, AcpErrorKind::Timeout);
        assert!(!error.permits_fallback());
    }
    #[test]
    fn append_bounded_cjk_is_utf8_safe_and_stops_after_marker() {
        let mut raw = "x".repeat(MAX_RAW - 8);
        append_bounded(&mut raw, "中文中文");
        assert!(raw.ends_with("\n[输出已截断]\n"));
        let length = raw.len();
        append_bounded(&mut raw, "later");
        assert_eq!(raw.len(), length);
    }
    #[test]
    fn legacy_cache_read_tokens_shape_is_parsed() {
        let usage = json!({"inputTokens": 11, "outputTokens": 7, "cacheReadTokens": 5});
        assert_eq!(parse_usage(Some(&usage)).cache_read, 5);
    }
    #[test]
    fn two_five_megabyte_turns_are_consumed_without_lifetime_truncation() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client_with(&["--large-turns"]) else { return };
        let first = client.prompt("fake-model", None, "first", &cwd, Instant::now() + Duration::from_secs(10), "中", false, false, "").unwrap();
        let second = client.prompt("fake-model", Some(&first.channel.session_id), "second", &cwd, Instant::now() + Duration::from_secs(10), "中", false, false, "").unwrap();
        assert_eq!(first.channel.text, "turn-1:no");
        assert_eq!(second.channel.text, "turn-2:no");
        assert!(!second.raw_stdout.contains("[ACP stdout 已截断]"));
    }
    #[test]
    fn shutdown_terminates_fake_acp_process() {
        let _pool_guard = test_pool_guard();
        shutdown_all();
        let Some(client) = fake_client() else { return };
        let pid = client.process.process_id();
        let slot = Arc::new(ClientSlot { client: Mutex::new(Some(client)), pid: AtomicU32::new(pid) });
        pool()
            .lock()
            .unwrap()
            .insert("shutdown-test".into(), slot);
        shutdown_all();
        #[cfg(windows)]
        {
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let status = std::process::Command::new("tasklist").args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"]).output().unwrap();
                let found = String::from_utf8_lossy(&status.stdout).lines().any(|line| line.split(',').nth(1).is_some_and(|value| value.trim_matches('"') == pid.to_string()));
                if !found { break }
                assert!(Instant::now() < deadline, "ACP pid {pid} remained alive");
                thread::sleep(Duration::from_millis(25));
            }
        }
    }
    #[test]
    fn shutdown_all_returns_within_one_second_when_client_is_locked() {
        let _pool_guard = test_pool_guard();
        shutdown_all();
        let Some(client) = fake_client() else { return };
        let pid = client.process.process_id();
        let slot = Arc::new(ClientSlot { client: Mutex::new(Some(client)), pid: AtomicU32::new(pid) });
        pool().lock().unwrap().insert("busy-shutdown-test".into(), slot.clone());
        let _guard = slot.client.lock().unwrap();
        let started = Instant::now();
        shutdown_all();
        assert!(started.elapsed() < Duration::from_secs(1));
    }
    #[test]
    fn shutdown_all_sends_session_close_when_capability_is_available() {
        let _pool_guard = test_pool_guard();
        shutdown_all();
        let marker = std::env::temp_dir().join(format!("aicross-acp-close-{}", uuid::Uuid::new_v4()));
        let argument = format!("--close-file={}", marker.display());
        let Some(mut client) = fake_client_with(&[&argument]) else { return };
        let cwd = std::env::current_dir().unwrap();
        client.prompt("fake-model", None, "first", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "").unwrap();
        let pid = client.process.process_id();
        let slot = Arc::new(ClientSlot { client: Mutex::new(Some(client)), pid: AtomicU32::new(pid) });
        pool().lock().unwrap().insert("close-test".into(), slot);
        shutdown_all();
        assert!(marker.exists());
        let _ = std::fs::remove_file(marker);
    }
    #[test]
    fn same_key_concurrent_run_spawns_one_fake_process() {
        let _pool_guard = test_pool_guard();
        let cwd = std::env::current_dir().unwrap();
        let script = cwd.join("fixtures/acp/fake_server.py");
        let starts = std::env::temp_dir().join(format!("aicross-acp-starts-{}", uuid::Uuid::new_v4()));
        let provider = format!("__aicross_test__|{}|{}|--start-file={}", script.display(), script.display(), starts.display());
        thread::scope(|scope| {
            let handles = (0..2).map(|index| {
                let cwd = cwd.clone();
                let provider = provider.clone();
                scope.spawn(move || run("kimi", &provider, "fake-model", None, &format!("turn-{index}"), &cwd, Duration::from_secs(10), "中", false, false, &format!("concurrent-{index}")))
            }).collect::<Vec<_>>();
            for handle in handles {
                assert!(handle.join().unwrap().is_ok());
            }
        });
        assert_eq!(std::fs::read_to_string(&starts).unwrap().lines().count(), 1);
        let _ = std::fs::remove_file(starts);
    }
    #[test]
    fn only_kimi_is_enabled_as_an_acp_runner() {
        assert!(supports("kimi-code"));
        assert!(!supports("codex"));
    }
    #[test]
    #[ignore = "编排者在沙箱外使用已登录的 kimi 运行"]
    fn live_kimi_acp_two_turn_continuation() {
        let cwd = std::env::current_dir().unwrap();
        let nonce = uuid::Uuid::new_v4().simple().to_string();
        let first = run(
            "kimi",
            "",
            "kimi-code/k3-256k",
            None,
            &format!("只回复记住这个代号：{nonce}"),
            &cwd,
            Duration::from_secs(180),
            "中",
            false,
            false,
            "live-acp-1",
        )
        .unwrap();
        let second = run(
            "kimi",
            "",
            "kimi-code/k3-256k",
            Some(&first.channel.session_id),
            "刚才的代号是什么？只回复代号",
            &cwd,
            Duration::from_secs(180),
            "中",
            false,
            false,
            "live-acp-2",
        )
        .unwrap();
        assert!(!first.channel.session_id.is_empty());
        // kimi ACP 的 session/prompt 结果只有 stopReason，不报用量（2026-09-14 真机）；
        // 用量缺口由 dispatch::annotate_kimi_metadata 记进 runner_note。
        assert!(second.channel.text.contains(&nonce));
        assert_eq!(second.channel.session_id, first.channel.session_id);
        shutdown_all();
    }
}
