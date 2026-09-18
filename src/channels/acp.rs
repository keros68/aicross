use super::{ChannelOutput, Usage};
use crate::process::{run_process, spawn_interactive, InteractiveProcess, ProcessActivity, RecvLineError};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard, OnceLock, TryLockError};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

const MAX_RAW: usize = 8 * 1024 * 1024;

// 控制台单线聊天（interactive）走 ACP 时，权限请求与 slash 命令清单要送到界面：
// 引擎不持有 AppHandle 的调用链很长，这里在应用启动时存一份全局句柄。
static APP: OnceLock<AppHandle> = OnceLock::new();

pub fn install_app_handle(app: AppHandle) {
    let _ = APP.set(app);
}

fn emit<S: Serialize + Clone>(event: &str, payload: S) {
    if let Some(app) = APP.get() {
        let _ = app.emit(event, payload);
    }
}

pub const PERMISSION_EVENT: &str = "acp-permission";
pub const PERMISSION_CLOSED_EVENT: &str = "acp-permission-closed";
pub const COMMANDS_EVENT: &str = "acp-commands";

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AcpCommand {
    pub name: String,
    pub description: String,
    pub hint: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandsEvent {
    agent: String,
    /// 归一化目录（正斜杠、小写、无尾斜杠），界面按同一规则比对。
    cwd: String,
    commands: Vec<AcpCommand>,
}

/// 正斜杠、去尾斜杠；只有 Windows 盘符路径（大小写不敏感）才转小写，
/// Linux / 大小写敏感的 macOS 上 `/work/Foo` 与 `/work/foo` 是两个项目。
fn normalize_cwd(cwd: &str) -> String {
    let path = cwd.replace('\\', "/");
    let path = path.trim_end_matches('/');
    let bytes = path.as_bytes();
    let windows_drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    if windows_drive { path.to_lowercase() } else { path.to_string() }
}

fn command_key(agent: &str, cwd: &str) -> String {
    format!("{agent}|{cwd}")
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOption {
    pub option_id: String,
    pub name: String,
    pub kind: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionEvent {
    pub request_id: String,
    /// `run_id:node_id`，界面据 run 前缀认领自己那一路。
    pub dispatch_key: String,
    pub agent: String,
    pub title: String,
    pub kind: String,
    pub detail: String,
    pub options: Vec<PermissionOption>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PermissionClosed {
    request_id: String,
}

fn pending_permissions() -> &'static Mutex<HashMap<String, mpsc::Sender<Option<String>>>> {
    static PENDING: OnceLock<Mutex<HashMap<String, mpsc::Sender<Option<String>>>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 界面已关掉的轮次（runId 前缀）：之后到的权限请求不再上卡片，直接拒。
/// 只在持有 pending_permissions 锁时读写（锁序 pending → closed），与登记挂起请求原子。
/// 这一轮（含续聊失效的重派）彻底结束后由界面调 forget_acp_run 删掉。
fn closed_runs() -> &'static Mutex<Vec<String>> {
    static CLOSED: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    CLOSED.get_or_init(|| Mutex::new(Vec::new()))
}

fn run_closed(dispatch_key: &str) -> bool {
    closed_runs().lock().is_ok_and(|closed| {
        closed.iter().any(|prefix| {
            dispatch_key.starts_with(&format!("{prefix}:")) || dispatch_key.starts_with(&format!("{prefix}-fresh:"))
        })
    })
}

/// 控制台 tab 关闭：本轮挂着的权限请求立即按拒绝回，之后再到的也直接拒，
/// 不让引擎等到整轮超时、占着进程锁。
#[tauri::command]
pub fn close_acp_permissions(run_id: String) {
    if run_id.is_empty() {
        return;
    }
    let senders = pending_permissions()
        .lock()
        .map(|mut pending| {
            if let Ok(mut closed) = closed_runs().lock() {
                closed.push(run_id.clone());
            }
            let keys = pending
                .keys()
                .filter(|key| key.starts_with(&format!("{run_id}:")) || key.starts_with(&format!("{run_id}-fresh:")))
                .cloned()
                .collect::<Vec<_>>();
            keys.into_iter().filter_map(|key| pending.remove(&key)).collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for sender in senders {
        let _ = sender.send(None);
    }
}

/// 关 tab 那一轮的派发全部结束后删掉关闭记录（此后不会再有这一轮的权限请求）。
#[tauri::command]
pub fn forget_acp_run(run_id: String) {
    if let Ok(_pending) = pending_permissions().lock() {
        if let Ok(mut closed) = closed_runs().lock() {
            closed.retain(|prefix| prefix != &run_id);
        }
    }
}

/// 取命令清单失败（适配器没起来、建会话失败）只记 5 分钟：短时间内不反复等，恢复后能再取。
fn command_failures() -> &'static Mutex<HashMap<String, Instant>> {
    static FAILURES: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    FAILURES.get_or_init(|| Mutex::new(HashMap::new()))
}

const COMMAND_FAILURE_TTL: Duration = Duration::from_secs(300);

fn command_cache() -> &'static Mutex<HashMap<String, Vec<AcpCommand>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Vec<AcpCommand>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 界面对权限卡片的回答。`option_id` 为空表示拒绝；找不到挂起请求（已超时 / 已取消）时报错。
#[tauri::command]
pub fn answer_acp_permission(request_id: String, option_id: Option<String>) -> Result<(), String> {
    let sender = pending_permissions()
        .lock()
        .map_err(|_| "权限请求表不可用".to_string())?
        .remove(&request_id)
        .ok_or_else(|| "这条权限请求已结束".to_string())?;
    sender.send(option_id).map_err(|_| "这条权限请求已结束".to_string())
}

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
    can_resume_session: bool,
    prelude_raw: String,
    /// 进程工作目录（命令清单按 agent + 目录缓存：项目级技能各目录不同）。
    cwd: String,
    /// 归一化后的 agent（kimi / claude / codex）；配置项取值按它分支。
    agent: String,
    /// 本轮是控制台单线聊天时为 `Some(dispatch_key)`：非读非写的权限请求交给界面裁决。
    interactive: Option<String>,
}

#[derive(Default)]
struct SessionState {
    model: String,
    config_options: Vec<Value>,
    /// 经 session/resume 接回的会话（不是本进程 session/new 建的）。
    resumed: bool,
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
/// kimi 在流水线与控制台都走 ACP；claude / codex 只在控制台单线聊天（interactive）走 ACP——
/// 流水线里的盲节点仍用一次性 CLI（claude 盲审 `--tools ""` 无工具，ACP 适配器做不到）。
pub fn supports(agent: &str, interactive: bool) -> bool {
    match super::normalize_agent(agent) {
        "kimi" => true,
        "claude" | "codex" => interactive,
        _ => false,
    }
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
    interactive: bool,
    registry_key: &str,
    activity: ProcessActivity,
) -> Result<AcpOutput, AcpError> {
    if !supports(agent, interactive) {
        return Err(AcpError::new(
            AcpErrorKind::Protocol,
            "该通道尚未提供 ACP 运行适配器",
        ));
    }
    let deadline = Instant::now() + timeout;
    let key = pool_key(agent, provider, model, cwd, blind);
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
        let client = start_agent(agent, provider, cwd, deadline.saturating_duration_since(Instant::now()))?;
        shared.pid.store(client.process.process_id(), Ordering::Release);
        *guard = Some(client);
    }
    let client = guard.as_mut().expect("ACP slot initialized");
    client.interactive = (interactive && !blind).then(|| registry_key.to_string());
    let result = client.prompt_with_activity(
        model,
        session_id,
        prompt,
        cwd,
        deadline,
        thinking,
        writable,
        blind,
        registry_key,
        &activity,
    );
    if let Some(client) = guard.as_mut() {
        client.interactive = None;
    }
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

fn start_agent(agent: &str, _provider: &str, cwd: &Path, timeout: Duration) -> Result<AcpClient, AcpError> {
    let normalized = super::normalize_agent(agent).to_string();
    #[cfg(test)]
    if let Some((script, _, args)) = super::test_provider_parts(_provider) {
        let python = super::resolve_binary("python", None)
            .map_err(|e| AcpError::new(AcpErrorKind::Process, e))?;
        let mut command = vec![script];
        command.extend(args);
        let mut client = start_client(&python, &command, cwd, timeout)?;
        client.agent = normalized;
        return Ok(client);
    }
    let (binary, fallback, args, install): (&str, _, Vec<String>, &str) = match normalized.as_str() {
        "claude" => ("claude-agent-acp", None, Vec::new(), "npm i -g @agentclientprotocol/claude-agent-acp"),
        "codex" => ("codex-acp", None, Vec::new(), "npm i -g @agentclientprotocol/codex-acp"),
        _ => ("kimi", super::kimi_fallback(), vec!["acp".into()], ""),
    };
    let program = super::resolve_binary(binary, fallback).map_err(|e| {
        let message = if install.is_empty() { e } else { format!("未安装 {binary}（{install}）：{e}") };
        AcpError::new(AcpErrorKind::Process, message)
    })?;
    let mut client = start_client(&program, &args, cwd, timeout)?;
    client.agent = normalized;
    Ok(client)
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
        can_resume_session: false,
        prelude_raw: String::new(),
        cwd: normalize_cwd(&cwd.to_string_lossy()),
        agent: String::new(),
        interactive: None,
    };
    let mut initialize_text = String::new();
    let mut initialize_raw = String::new();
    let initialized = client.request("initialize", json!({"protocolVersion":1,"clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false},"clientInfo":{"name":"aicross","version":env!("CARGO_PKG_VERSION")}}), Instant::now()+timeout.min(Duration::from_secs(30)), Some(("", &mut initialize_text, &mut initialize_raw)), false, false, "", None)?;
    client.prelude_raw = initialize_raw;
    client.can_close_session = initialized
        .pointer("/result/agentCapabilities/sessionCapabilities/close")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    // claude-agent-acp / codex-acp 以空对象 `{}` 声明能力，kimi 与测试桩用布尔值；两种都认。
    let declared = |name: &str| {
        initialized
            .pointer(&format!("/result/agentCapabilities/sessionCapabilities/{name}"))
            .is_some_and(|value| value.as_bool().unwrap_or(value.is_object()))
    };
    client.can_close_session = client.can_close_session || declared("close");
    client.can_resume_session = declared("resume");
    Ok(client)
}

impl AcpClient {
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    fn prompt(
        &mut self, model: &str, session_id: Option<&str>, prompt: &str, cwd: &Path,
        deadline: Instant, thinking: &str, writable: bool, blind: bool, cancel_key: &str,
    ) -> Result<AcpOutput, AcpError> {
        self.prompt_with_activity(model, session_id, prompt, cwd, deadline, thinking, writable, blind, cancel_key, &ProcessActivity::new())
    }

    #[allow(clippy::too_many_arguments)]
    fn prompt_with_activity(
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
        activity: &ProcessActivity,
    ) -> Result<AcpOutput, AcpError> {
        if !self.process.is_running() {
            return Err(AcpError::new(AcpErrorKind::Process, "ACP 进程已退出"));
        }
        let (session_id, new_config_options, resumed) = match session_id.filter(|v| !v.is_empty()) {
            Some(id) if self.sessions.contains_key(id) => (id.to_string(), None, false),
            // claude / codex 的 ACP 会话 id 就是 CLI 自己的会话 id：进程重启或上一轮走的一次性
            // CLI，都能用 session/resume 接回原会话；接不回再按「所属进程已退出」回退。
            Some(id) if self.can_resume_session && matches!(self.agent.as_str(), "claude" | "codex") => {
                let response = self
                    .request(
                        "session/resume",
                        json!({"sessionId":id,"cwd":cwd,"mcpServers":[]}),
                        deadline,
                        None,
                        writable,
                        blind,
                        cancel_key,
                        None,
                    )
                    .map_err(|error| match error.kind {
                        AcpErrorKind::Protocol => AcpError::new(
                            AcpErrorKind::ForeignSession,
                            format!("ACP 会话无法接回：{}", error.message),
                        ),
                        _ => error,
                    })?;
                let options = response
                    .pointer("/result/configOptions")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                (id.to_string(), Some(options), true)
            }
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
                    None,
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
                (id, Some(options), false)
            },
        };
        if let Some(config_options) = new_config_options {
            self.sessions.insert(session_id.clone(), SessionState {
                model: String::new(),
                config_options,
                resumed,
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
            Some(activity),
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
                identity: reported_model(&response, model),
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
        let resumed_without_options = config_options.is_empty()
            && self.sessions.get(session_id).is_some_and(|state| state.resumed);
        if resumed_without_options && !model.is_empty() && current_model != Some(model) {
            // session/resume 没回配置清单时不盲设模型：接回的会话沿用它原来的模型。
            notes.push("ACP 接回的会话未报配置清单，沿用原模型".into());
            if let Some(state) = self.sessions.get_mut(session_id) {
                state.model = model.to_string();
            }
        } else if !model.is_empty() && current_model != Some(model) {
            let model_config = config_options.iter().find(|option| {
                option.get("category").and_then(Value::as_str) == Some("model")
                    || option.get("id").and_then(Value::as_str) == Some("model")
            });
            let mut value = model.to_string();
            if let Some(option) = model_config {
                let values = option_values(option);
                value = resolve_model_value(model, &values.iter().map(String::as_str).collect::<Vec<_>>())
                    .ok_or_else(|| AcpError::new(AcpErrorKind::Protocol, format!("ACP 不支持声明模型 {model}")))?;
            }
            let config_id = model_config
                .and_then(|option| option.get("id").and_then(Value::as_str))
                .unwrap_or("model")
                .to_string();
            let configured = self.request(
                "session/set_config_option",
                json!({"sessionId":session_id,"configId":config_id,"value":value}),
                deadline, None, writable, blind, cancel_key, None,
            );
            if configured.is_err() {
                let legacy = self.request(
                    "session/set_model",
                    json!({"sessionId":session_id,"modelId":model}),
                    deadline, None, writable, blind, cancel_key, None,
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
        // codex-acp 没有 default，模式就是沙箱档：只读 / 盲节点用 read-only，可写执行者用 agent，
        // 永不 agent-full-access。
        let mode = match self.agent.as_str() {
            "codex" if writable && !blind => "agent",
            "codex" => "read-only",
            _ => "default",
        };
        if self.request("session/set_config_option", json!({"sessionId":session_id,"configId":"mode","value":mode}), deadline, None, writable, blind, cancel_key, None).is_err() {
            notes.push(format!("ACP 未能设置 {mode} 模式"));
        }
        // 思考档位的配置项 id 各家不同（kimi thinking / claude effort / codex reasoning_effort），
        // 按类别 thought_level 找；取值缺「medium」的（kimi 只有 low/high）中档按 high。
        let thought_option = config_options
            .iter()
            .find(|option| option.get("category").and_then(Value::as_str) == Some("thought_level"));
        let (thought_id, thought) = match thought_option {
            Some(option) => {
                let values = option_values(option);
                let wanted = match thinking {
                    "关" | "低" => "low",
                    "高" => "high",
                    _ => "medium",
                };
                let value = if values.iter().any(|value| value == wanted) { wanted } else if wanted == "medium" { "high" } else { wanted };
                (option.get("id").and_then(Value::as_str).unwrap_or("thinking").to_string(), value)
            }
            None => ("thinking".to_string(), if matches!(thinking, "关" | "低") { "low" } else { "high" }),
        };
        if self.request("session/set_config_option", json!({"sessionId":session_id,"configId":thought_id,"value":thought}), deadline, None, writable, blind, cancel_key, None).is_err() {
            notes.push(format!("ACP 未能设置 thinking={thought}"));
        }
        Ok(())
    }

    fn observe_commands(&self, message: &Value) {
        if message.pointer("/params/update/sessionUpdate").and_then(Value::as_str) != Some("available_commands_update") {
            return;
        }
        let commands = parse_commands(message);
        if let Ok(mut cache) = command_cache().lock() {
            cache.insert(command_key(&self.agent, &self.cwd), commands.clone());
        }
        emit(COMMANDS_EVENT, CommandsEvent { agent: self.agent.clone(), cwd: self.cwd.clone(), commands });
    }

    /// 把权限请求交给界面卡片，阻塞到用户点选、派发取消或超时；后两者按拒绝回。
    /// 卡片只给「允许一次 / 拒绝」两个选项：永不持久授权。
    fn ask_user_permission(
        &mut self,
        message: &Value,
        dispatch_key: &str,
        deadline: Instant,
        cancel_key: &str,
    ) -> Result<(), AcpError> {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let request_id = format!("{dispatch_key}#{}", COUNTER.fetch_add(1, Ordering::Relaxed));
        let options = permission_options(message);
        let (sender, receiver) = mpsc::channel();
        // 查「已关」与登记挂起在同一把锁里：关 tab 要么先发生（这里直接拒），要么后发生（它能扫到这条）。
        let closed = match pending_permissions().lock() {
            Ok(mut pending) if !run_closed(dispatch_key) => {
                pending.insert(request_id.clone(), sender);
                false
            }
            _ => true,
        };
        if closed {
            let reject = options.iter().find(|option| option.kind == "reject_once").map(|option| option.option_id.clone());
            let result = reject
                .map(|id| json!({"outcome":{"outcome":"selected","optionId":id}}))
                .unwrap_or_else(|| json!({"outcome":{"outcome":"cancelled"}}));
            return self
                .process
                .send_json_line(&json!({"jsonrpc":"2.0","id":message["id"],"result":result}))
                .map_err(|e| AcpError::new(AcpErrorKind::Process, e));
        }
        emit(PERMISSION_EVENT, permission_event(message, &request_id, dispatch_key, &self.agent, options.clone()));
        let choice = loop {
            if !cancel_key.is_empty() && crate::dispatch::is_cancelled(cancel_key) {
                break None;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break None;
            }
            match receiver.recv_timeout(remaining.min(Duration::from_millis(100))) {
                Ok(choice) => break choice,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break None,
            }
        };
        if let Ok(mut pending) = pending_permissions().lock() {
            pending.remove(&request_id);
        }
        emit(PERMISSION_CLOSED_EVENT, PermissionClosed { request_id });
        let selected = choice
            .filter(|id| options.iter().any(|option| &option.option_id == id))
            .or_else(|| options.iter().find(|option| option.kind == "reject_once").map(|option| option.option_id.clone()));
        let result = selected
            .map(|id| json!({"outcome":{"outcome":"selected","optionId":id}}))
            .unwrap_or_else(|| json!({"outcome":{"outcome":"cancelled"}}));
        self.process
            .send_json_line(&json!({"jsonrpc":"2.0","id":message["id"],"result":result}))
            .map_err(|e| AcpError::new(AcpErrorKind::Process, e))
    }

    fn drain_pending(&mut self, raw: &mut String) -> Result<(), AcpError> {
        loop {
            match self.process.try_recv_line() {
                Ok(Some(line)) => {
                    if let Ok(message) = serde_json::from_str::<Value>(&line) {
                        self.observe_commands(&message);
                    }
                    append_bounded(raw, &line)
                }
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
            let _ = self.request("session/close", json!({"sessionId":session_id}), deadline, None, false, false, "", None);
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
        activity: Option<&ProcessActivity>,
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
            if let Some(activity) = activity {
                activity.mark_output();
            }
            if let Some((_, _, raw)) = output.as_mut() {
                append_bounded(raw, &line);
            }
            if std::env::var("AICROSS_ACP_DEBUG").is_ok() { let mut end = line.len().min(300); while !line.is_char_boundary(end) { end -= 1; } eprintln!("[acp-debug] << {}", &line[..end]); }
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            let incoming = message.get("method").and_then(Value::as_str);
            self.observe_commands(&message);
            if incoming == Some("session/update") {
                let current_session = output.as_ref().is_some_and(|(session, _, _)| {
                    message.pointer("/params/sessionId").and_then(Value::as_str) == Some(*session)
                });
                if current_session {
                    if let Some(activity) = activity {
                        if message.pointer("/params/update/sessionUpdate").and_then(Value::as_str) == Some("tool_call") {
                            if let Some(title) = message.pointer("/params/update/title").and_then(Value::as_str) {
                                activity.record_tool_call(title);
                            } else {
                                activity.record_untitled_tool_call();
                            }
                        }
                    }
                }
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
                let current_session = output.as_ref().is_some_and(|(session, _, _)| {
                    message.pointer("/params/sessionId").and_then(Value::as_str) == Some(*session)
                });
                if current_session {
                    if let Some(activity) = activity {
                        if let Some(title) = message.pointer("/params/toolCall/title").and_then(Value::as_str) {
                            activity.record_tool_call(title);
                        } else {
                            activity.record_untitled_tool_call();
                        }
                    }
                }
                match self.interactive.clone() {
                    Some(dispatch_key)
                        if current_session && !blind && !is_read_request(&message) && !is_edit_request(&message) =>
                    {
                        self.ask_user_permission(&message, &dispatch_key, deadline, cancel_key)?;
                    }
                    _ => self.answer_permission(&message, writable, blind)?,
                }
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

fn parse_commands(message: &Value) -> Vec<AcpCommand> {
    message
        .pointer("/params/update/availableCommands")
        .and_then(Value::as_array)
        .map(|commands| {
            commands
                .iter()
                .filter_map(|command| {
                    let name = command.get("name").and_then(Value::as_str)?.trim();
                    (!name.is_empty()).then(|| AcpCommand {
                        name: name.to_string(),
                        description: command.get("description").and_then(Value::as_str).unwrap_or("").to_string(),
                        hint: command.pointer("/input/hint").and_then(Value::as_str).unwrap_or("").to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn permission_options(message: &Value) -> Vec<PermissionOption> {
    message
        .pointer("/params/options")
        .and_then(Value::as_array)
        .map(|options| {
            options
                .iter()
                .filter_map(|option| {
                    let kind = option.get("kind").and_then(Value::as_str)?;
                    matches!(kind, "allow_once" | "reject_once").then(|| PermissionOption {
                        option_id: option.get("optionId").and_then(Value::as_str).unwrap_or("").to_string(),
                        name: if kind == "allow_once" { "允许一次".into() } else { "拒绝".into() },
                        kind: kind.to_string(),
                    })
                })
                .filter(|option| !option.option_id.is_empty())
                .fold(Vec::<PermissionOption>::new(), |mut kept, option| {
                    if !kept.iter().any(|seen| seen.kind == option.kind) {
                        kept.push(option);
                    }
                    kept
                })
        })
        .unwrap_or_default()
}

/// 卡片正文：claude 给 rawInput（命令、参数），kimi 只给 content 文本；脱敏后截到 800 字。
fn permission_event(
    message: &Value,
    request_id: &str,
    dispatch_key: &str,
    agent: &str,
    options: Vec<PermissionOption>,
) -> PermissionEvent {
    let tool = message.pointer("/params/toolCall").unwrap_or(&Value::Null);
    let title = tool.get("title").and_then(Value::as_str).unwrap_or("未命名工具").to_string();
    let raw_input = tool
        .get("rawInput")
        .filter(|value| !value.is_null() && value.as_object().is_none_or(|object| !object.is_empty()))
        .map(|value| value.to_string());
    let content = tool.get("content").and_then(Value::as_array).map(|items| {
        items
            .iter()
            .filter_map(|item| item.pointer("/content/text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n")
    });
    let detail = raw_input.or(content.filter(|text| !text.is_empty())).unwrap_or_default();
    let (detail, _) = crate::dispatch::redact_text(&detail, true);
    let detail = if detail.chars().count() > 800 {
        format!("{}…", detail.chars().take(800).collect::<String>())
    } else {
        detail
    };
    PermissionEvent {
        request_id: request_id.to_string(),
        dispatch_key: dispatch_key.to_string(),
        agent: agent.to_string(),
        title,
        kind: tool.get("kind").and_then(Value::as_str).unwrap_or("").to_string(),
        detail,
        options,
    }
}

fn option_values(option: &Value) -> Vec<String> {
    option
        .get("options")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.get("value").and_then(Value::as_str).map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// 写文件类请求：控制台单线聊天是只读的，这类请求不上卡片、直接拒。kind 缺失（kimi）时按 title 认。
fn is_edit_request(message: &Value) -> bool {
    let kind = message.pointer("/params/toolCall/kind").and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
    if !kind.is_empty() {
        return matches!(kind.as_str(), "edit" | "delete" | "move");
    }
    let title = message.pointer("/params/toolCall/title").and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
    ["write", "edit", "multiedit", "strreplace", "replace", "delete", "move", "notebookedit"]
        .iter()
        .any(|prefix| title.starts_with(prefix))
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

/// claude-agent-acp 在 prompt 结果的 `_meta.quota.model_usage` 里按模型列用量（含起标题用的
/// haiku 小调用）；取 token 最多的那个当真身，去掉 `[1m]` 这类上下文后缀。其它适配器不报则为空。
fn reported_model(response: &Value, requested: &str) -> String {
    let Some(models) = response.pointer("/result/_meta/quota/model_usage").and_then(Value::as_array) else {
        return String::new();
    };
    let entries = models
        .iter()
        .filter_map(|entry| {
            let model = entry.get("model").and_then(Value::as_str)?;
            let tokens = entry.pointer("/token_count/totalTokens").and_then(Value::as_u64).unwrap_or(0);
            Some((tokens, model.split('[').next().unwrap_or(model).trim().to_string()))
        })
        .collect::<Vec<_>>();
    let requested = requested.split('[').next().unwrap_or(requested).trim().to_ascii_lowercase();
    let exact = entries
        .iter()
        .filter(|(_, model)| model.to_ascii_lowercase() == requested)
        .max_by_key(|(tokens, _)| *tokens);
    if let Some((_, model)) = exact {
        return model.clone();
    }
    let family = model_family(&requested);
    if family.is_some_and(|family| requested == family) {
        let mut same_family = entries
            .iter()
            .filter(|(_, model)| family.is_some_and(|family| model.to_ascii_lowercase().contains(family)))
            .map(|(_, model)| model.clone())
            .collect::<Vec<_>>();
        same_family.sort();
        same_family.dedup();
        if !same_family.is_empty() {
            // 同家族多代（如子代理用了旧一代）只列这一家族的，不把起标题的小模型也算进来。
            return same_family.join(" + ");
        }
    }
    // 请求的具体模型不在用量里：可能降级，也可能只是命名不同；用量里混着子代理与起标题的小调用，
    // 分不出哪个是主模型，就按用量从大到小全列出来，让界面如实显示「这一轮跑了这些」。
    let mut ranked = entries.clone();
    ranked.sort_by(|left, right| right.0.cmp(&left.0));
    ranked.dedup_by(|left, right| left.1 == right.1);
    ranked.into_iter().map(|(_, model)| model).collect::<Vec<_>>().join(" + ")
}

fn model_family(model: &str) -> Option<&'static str> {
    let lower = model.to_ascii_lowercase();
    ["opus", "sonnet", "haiku", "fable"].into_iter().find(|family| lower.contains(family))
}

/// ACP 模型选项的取值与资源库里的模型 ID 不一定相同：claude-agent-acp 只给 `opus[1m]` /
/// `sonnet` / `haiku` 这类别名。先精确匹配，再试 `[1m]` 后缀，最后按家族词（opus/sonnet/haiku/fable）
/// 找唯一别名；都不中返回 None（调用方报「不支持声明模型」，派发回退一次性 CLI）。
fn resolve_model_value(model: &str, values: &[&str]) -> Option<String> {
    if values.contains(&model) {
        return Some(model.to_string());
    }
    let long_context = format!("{model}[1m]");
    if values.contains(&long_context.as_str()) {
        return Some(long_context);
    }
    let family = model_family(model)?;
    let mut matches = values.iter().filter(|value| value.to_ascii_lowercase().contains(family));
    let first = matches.next()?;
    matches.next().is_none().then(|| first.to_string())
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

/// 测试用：等到以 `prefix` 开头的挂起权限请求出现，替界面回答它。
#[cfg(test)]
pub(crate) fn test_answer_pending(prefix: &str, option: Option<&str>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let id = pending_permissions().lock().unwrap().keys().find(|key| key.starts_with(prefix)).cloned();
        if let Some(id) = id {
            answer_acp_permission(id, option.map(str::to_string)).unwrap();
            return;
        }
        assert!(Instant::now() < deadline, "没等到挂起的权限请求");
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(test)]
pub(crate) fn test_pool_guard() -> MutexGuard<'static, ()> {
    static ACTIVITY: OnceLock<Mutex<()>> = OnceLock::new();
    ACTIVITY.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn pool_key(agent: &str, provider: &str, model: &str, cwd: &Path, blind: bool) -> String {
    format!(
        "{}/{}/{}/{}/{}",
        super::normalize_agent(agent),
        provider,
        model,
        cwd.display(),
        blind
    )
}

#[cfg(test)]
pub(crate) fn test_wait_for_process_exit(
    agent: &str,
    provider: &str,
    model: &str,
    cwd: &Path,
    blind: bool,
    timeout: Duration,
) -> bool {
    // 与生产共用同一个键函数：键格式若改了，这里不会静默找不到进程而直接放行。
    let key = pool_key(agent, provider, model, cwd, blind);
    let deadline = Instant::now() + timeout;
    loop {
        let shared = pool()
            .lock()
            .ok()
            .and_then(|clients| clients.get(&key).cloned());
        let Some(shared) = shared else {
            return true;
        };
        match shared.client.try_lock() {
            Ok(mut slot) => {
                let Some(client) = slot.as_mut() else {
                    return true;
                };
                if !client.process.is_running() {
                    return true;
                }
            }
            Err(TryLockError::WouldBlock) => {}
            Err(TryLockError::Poisoned(_)) => return false,
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(10));
    }
}
/// 控制台输入 `/` 时的命令清单。先看缓存（任一次 ACP 会话推来的都算）；没有就临时起一个
/// 适配器进程、建会话、等它推 `available_commands_update`，不发 prompt、不耗 token，拿到即关。
#[tauri::command]
pub async fn acp_commands(agent: String, cwd: String) -> Result<Vec<AcpCommand>, String> {
    let normalized = super::normalize_agent(&agent).to_string();
    if !supports(&normalized, true) {
        return Ok(Vec::new());
    }
    let key = command_key(&normalized, &normalize_cwd(&cwd));
    if let Some(cached) = command_cache().lock().ok().and_then(|cache| cache.get(&key).cloned()) {
        return Ok(cached);
    }
    let recently_failed = command_failures()
        .lock()
        .is_ok_and(|failures| failures.get(&key).is_some_and(|at| at.elapsed() < COMMAND_FAILURE_TTL));
    if recently_failed {
        return Ok(Vec::new());
    }
    let outcome = tauri::async_runtime::spawn_blocking(move || probe_commands(&normalized, Path::new(&cwd)))
        .await
        .map_err(|e| format!("读取命令清单失败：{e}"))?;
    if outcome.as_ref().map_or(true, Vec::is_empty) {
        if let Ok(mut failures) = command_failures().lock() {
            failures.insert(key, Instant::now());
        }
    }
    outcome
}

fn probe_commands(agent: &str, cwd: &Path) -> Result<Vec<AcpCommand>, String> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut client = start_agent(agent, "", cwd, Duration::from_secs(20)).map_err(|e| e.message)?;
    let outcome = (|| {
        client
            .request("session/new", json!({"cwd":cwd,"mcpServers":[]}), deadline, None, false, true, "", None)
            .map_err(|e| e.message)?;
        let key = command_key(agent, &client.cwd);
        loop {
            if let Some(found) = command_cache().lock().ok().and_then(|cache| cache.get(&key).cloned()) {
                return Ok(found);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(Vec::new());
            }
            match client.process.recv_line(remaining.min(Duration::from_millis(200))) {
                Ok(line) => {
                    if let Ok(message) = serde_json::from_str::<Value>(&line) {
                        client.observe_commands(&message);
                    }
                }
                Err(RecvLineError::Timeout) => {}
                Err(RecvLineError::Disconnected(error)) => return Err(error),
            }
        }
    })();
    client.process.terminate();
    outcome
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
    fn fake_acp_three_same_updates_plus_permission_count_as_four_repeated_calls() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client_with(&["--repeat-tool-calls"]) else { return };
        let activity = ProcessActivity::new();
        client.prompt_with_activity("fake-model", None, "first", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "", &activity).unwrap();
        let (_, _, calls, repeated, last_tool) = activity.snapshot();
        assert_eq!((calls, repeated, last_tool.as_str()), (4, 4, "Read file"));
    }
    #[test]
    fn r1_permission_request_tool_call_is_counted() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client() else { return };
        let activity = ProcessActivity::new();
        client.prompt_with_activity("fake-model", None, "first", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "", &activity).unwrap();
        let (_, _, calls, repeated, last_tool) = activity.snapshot();
        assert_eq!((calls, repeated, last_tool.as_str()), (1, 1, "Read misleading title"));
    }
    #[test]
    fn r1_other_session_tool_call_is_not_counted() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client_with(&["--other-session-tool-call"]) else { return };
        let activity = ProcessActivity::new();
        client.prompt_with_activity("fake-model", None, "first", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "", &activity).unwrap();
        let (_, _, calls, _, last_tool) = activity.snapshot();
        assert_eq!((calls, last_tool.as_str()), (1, "Read misleading title"));
    }
    #[test]
    fn r1_untitled_tool_call_still_increments_count() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client_with(&["--untitled-tool-call"]) else { return };
        let activity = ProcessActivity::new();
        client.prompt_with_activity("fake-model", None, "first", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "", &activity).unwrap();
        let (_, _, calls, repeated, last_tool) = activity.snapshot();
        assert_eq!((calls, repeated, last_tool), (2, 0, String::new()));
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
                scope.spawn(move || run("kimi", &provider, "fake-model", None, &format!("turn-{index}"), &cwd, Duration::from_secs(10), "中", false, false, false, &format!("concurrent-{index}"), ProcessActivity::new()))
            }).collect::<Vec<_>>();
            for handle in handles {
                assert!(handle.join().unwrap().is_ok());
            }
        });
        assert_eq!(std::fs::read_to_string(&starts).unwrap().lines().count(), 1);
        let _ = std::fs::remove_file(starts);
    }
    #[test]
    fn claude_and_codex_use_acp_only_for_interactive_console_turns() {
        assert!(supports("kimi-code", false));
        assert!(supports("kimi", true));
        assert!(!supports("codex", false));
        assert!(!supports("claude", false));
        assert!(supports("codex", true));
        assert!(supports("claude", true));
        assert!(!supports("pi", true));
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
            false,
            "live-acp-1",
            ProcessActivity::new(),
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
            false,
            "live-acp-2",
            ProcessActivity::new(),
        )
        .unwrap();
        assert!(!first.channel.session_id.is_empty());
        // kimi ACP 的 session/prompt 结果只有 stopReason，不报用量（2026-09-14 真机）；
        // 用量缺口由 dispatch::annotate_kimi_metadata 记进 runner_note。
        assert!(second.channel.text.contains(&nonce));
        assert_eq!(second.channel.session_id, first.channel.session_id);
        shutdown_all();
    }
    #[test]
    fn claude_model_aliases_resolve_to_acp_option_values() {
        let values = ["default", "opus[1m]", "claude-fable-5-1[1m]", "sonnet", "haiku"];
        assert_eq!(resolve_model_value("claude-opus-5", &values).as_deref(), Some("opus[1m]"));
        assert_eq!(resolve_model_value("claude-fable-5-1", &values).as_deref(), Some("claude-fable-5-1[1m]"));
        assert_eq!(resolve_model_value("claude-sonnet-5", &values).as_deref(), Some("sonnet"));
        assert_eq!(resolve_model_value("claude-haiku-4-5-20251001", &values).as_deref(), Some("haiku"));
        assert_eq!(resolve_model_value("sonnet", &values).as_deref(), Some("sonnet"));
        assert_eq!(resolve_model_value("gpt-5.6-sol", &values), None);
        // 家族词命中两个别名时不猜。
        assert_eq!(resolve_model_value("claude-opus-5", &["opus", "opus[1m]x"]), None);
        let codex = ["gpt-6-astra", "gpt-5.6-sol", "gpt-5.6-terra"];
        assert_eq!(resolve_model_value("gpt-5.6-sol", &codex).as_deref(), Some("gpt-5.6-sol"));
    }
    #[test]
    fn reported_model_picks_largest_usage_and_drops_context_suffix() {
        let response = json!({"result":{"_meta":{"quota":{"model_usage":[
            {"model":"claude-haiku-4-5-20251001","token_count":{"totalTokens":971}},
            {"model":"claude-opus-5[1m]","token_count":{"totalTokens":88160}}
        ]}}}});
        assert_eq!(reported_model(&response, "claude-opus-5"), "claude-opus-5");
        assert_eq!(reported_model(&json!({"result":{"stopReason":"end_turn"}}), "opus"), "");
        // 子代理（haiku）用量更大时仍报请求的那一家族；请求家族完全缺席时如实报用量最大的。
        let subagent = json!({"result":{"_meta":{"quota":{"model_usage":[
            {"model":"claude-haiku-4-5-20251001","token_count":{"totalTokens":500000}},
            {"model":"claude-opus-5[1m]","token_count":{"totalTokens":9000}}
        ]}}}});
        assert_eq!(reported_model(&subagent, "opus"), "claude-opus-5");
        // 按别名请求、同家族有两代：不按用量猜，全列出。
        let two_opus = json!({"result":{"_meta":{"quota":{"model_usage":[
            {"model":"claude-opus-4-8","token_count":{"totalTokens":400000}},
            {"model":"claude-opus-5[1m]","token_count":{"totalTokens":9000}}
        ]}}}});
        assert_eq!(reported_model(&two_opus, "opus"), "claude-opus-4-8 + claude-opus-5");
        let mixed = json!({"result":{"_meta":{"quota":{"model_usage":[
            {"model":"claude-haiku-4-5-20251001","token_count":{"totalTokens":900000}},
            {"model":"claude-opus-4-8","token_count":{"totalTokens":400000}},
            {"model":"claude-opus-5[1m]","token_count":{"totalTokens":9000}}
        ]}}}});
        assert_eq!(reported_model(&mixed, "opus"), "claude-opus-4-8 + claude-opus-5");
        // 请求的具体模型缺席：不猜主模型，按用量列出实际跑过的全部模型。
        assert_eq!(reported_model(&subagent, "claude-sonnet-5"), "claude-haiku-4-5-20251001 + claude-opus-5");
        // 同一家族不同代：请求的具体 ID 在列表里时按它报，不按用量。
        let generations = json!({"result":{"_meta":{"quota":{"model_usage":[
            {"model":"claude-opus-4-8","token_count":{"totalTokens":400000}},
            {"model":"claude-opus-5[1m]","token_count":{"totalTokens":9000}}
        ]}}}});
        assert_eq!(reported_model(&generations, "claude-opus-5"), "claude-opus-5");
        assert_eq!(normalize_cwd("C:\\Users\\Me\\Proj\\"), "c:/users/me/proj");
        assert_eq!(normalize_cwd("/work/Foo/"), "/work/Foo");
    }
    #[test]
    fn edit_requests_are_recognised_by_kind_or_by_kimi_title() {
        assert!(is_edit_request(&json!({"params":{"toolCall":{"kind":"edit","title":"Edit a.py"}}})));
        assert!(is_edit_request(&json!({"params":{"toolCall":{"kind":"delete"}}})));
        assert!(!is_edit_request(&json!({"params":{"toolCall":{"kind":"execute","title":"Write file via bash"}}})));
        assert!(!is_edit_request(&json!({"params":{"toolCall":{"kind":"other","title":"mcp__kimi-cu__list_apps"}}})));
        assert!(is_edit_request(&json!({"params":{"toolCall":{"title":"WriteFile"}}})));
        assert!(is_edit_request(&json!({"params":{"toolCall":{"title":"StrReplaceFile"}}})));
        assert!(!is_edit_request(&json!({"params":{"toolCall":{"title":"mcp__kimi-cu__list_apps"}}})));
        assert!(!is_edit_request(&json!({"params":{"toolCall":{"title":"Shell"}}})));
    }
    #[test]
    fn commands_and_permission_options_are_parsed_for_the_console() {
        let update = json!({"params":{"update":{"sessionUpdate":"available_commands_update","availableCommands":[
            {"name":"compact","description":"压缩上下文"},
            {"name":"anthropic-skills:pdf","description":"PDF","input":{"hint":"文件"}},
            {"name":"  ","description":"空名丢弃"}
        ]}}});
        let commands = parse_commands(&update);
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[1], AcpCommand { name: "anthropic-skills:pdf".into(), description: "PDF".into(), hint: "文件".into() });
        let request = json!({"id":7,"params":{"toolCall":{"title":"mcp__kimi-cu__list_apps","kind":"other","rawInput":{"api_key":"sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789"}},
            "options":[{"optionId":"allow-once","kind":"allow_once"},{"optionId":"always","kind":"allow_always"},{"optionId":"reject","kind":"reject_once"}]}});
        let options = permission_options(&request);
        assert_eq!(options.iter().map(|o| o.option_id.as_str()).collect::<Vec<_>>(), ["allow-once", "reject"]);
        let duplicated = json!({"params":{"options":[{"optionId":"a1","kind":"allow_once"},{"optionId":"a2","kind":"allow_once"},{"optionId":"r1","kind":"reject_once"},{"optionId":"r2","kind":"reject_once"}]}});
        assert_eq!(permission_options(&duplicated).iter().map(|o| o.option_id.as_str()).collect::<Vec<_>>(), ["a1", "r1"]);
        let event = permission_event(&request, "r#1", "run:node", "claude", options);
        assert_eq!(event.title, "mcp__kimi-cu__list_apps");
        assert!(!event.detail.contains("abcdefghijklmnopqrstuvwxyz0123456789"), "卡片正文要脱敏：{}", event.detail);
    }
    #[test]
    fn interactive_turn_hands_execute_permission_to_the_user_and_honours_the_answer() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client() else { return };
        client.interactive = Some("card-run:allow".into());
        let answer = thread::spawn(|| test_answer_pending("card-run:allow#", Some("once")));
        let allowed = client
            .prompt("fake-model", None, "first", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "")
            .unwrap();
        answer.join().unwrap();
        // 用户点「允许一次」→ 选 allow_once；卡片从不给 allow_always。
        assert_eq!(allowed.channel.text, "turn-1:once");
        client.interactive = Some("card-run:reject".into());
        let answer = thread::spawn(|| test_answer_pending("card-run:reject#", None));
        let rejected = client
            .prompt("fake-model", Some(&allowed.channel.session_id), "second", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "")
            .unwrap();
        answer.join().unwrap();
        assert_eq!(rejected.channel.text, "turn-2:no");
        assert!(pending_permissions().lock().unwrap().keys().all(|key| !key.starts_with("card-run:")));
    }
    #[test]
    fn closing_the_tab_rejects_pending_and_later_permissions_of_that_run() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client() else { return };
        client.interactive = Some("closed-run:node".into());
        let closer = thread::spawn(|| {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !pending_permissions().lock().unwrap().keys().any(|key| key.starts_with("closed-run:")) {
                assert!(Instant::now() < deadline, "没等到挂起的权限请求");
                thread::sleep(Duration::from_millis(10));
            }
            close_acp_permissions("closed-run".into());
        });
        let first = client
            .prompt("fake-model", None, "first", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "")
            .unwrap();
        closer.join().unwrap();
        assert_eq!(first.channel.text, "turn-1:no");
        // 关掉之后同一轮再来的请求不挂起，立即拒。
        let second = client
            .prompt("fake-model", Some(&first.channel.session_id), "second", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "")
            .unwrap();
        assert_eq!(second.channel.text, "turn-2:no");
        assert!(pending_permissions().lock().unwrap().keys().all(|key| !key.starts_with("closed-run:")));
        forget_acp_run("closed-run".into());
        assert!(!run_closed("closed-run:node"));
    }
    #[test]
    fn late_permission_from_another_session_is_auto_answered_not_carded() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client_with(&["--other-session-permission"]) else { return };
        client.interactive = Some("card-other".into());
        let answer = thread::spawn(|| test_answer_pending("card-other#", Some("once")));
        let result = client
            .prompt("fake-model", None, "first", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "")
            .unwrap();
        answer.join().unwrap();
        // 另一会话的执行类请求按只读策略拒掉；只有本会话那条上了卡片、被「允许一次」。
        assert_eq!(result.channel.text, "turn-1:once:other-no");
        assert!(pending_permissions().lock().unwrap().keys().all(|key| !key.starts_with("card-other")));
    }
    #[test]
    fn interactive_turn_still_auto_answers_reads_and_blind_turns_never_ask() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client() else { return };
        client.interactive = Some("card-read".into());
        let read = client
            .prompt("fake-model", None, "read", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "")
            .unwrap();
        assert_eq!(read.channel.text, "turn-1:once");
        // 盲节点即使带了 interactive 也不弹卡片，按旧策略拒绝执行类。
        let blind = client
            .prompt("fake-model", None, "execute", &cwd, Instant::now() + Duration::from_secs(5), "中", false, true, "")
            .unwrap();
        assert_eq!(blind.channel.text, "turn-2:no");
        assert!(pending_permissions().lock().unwrap().keys().all(|key| !key.starts_with("card-read")));
    }
    #[test]
    fn available_commands_are_cached_per_agent() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client_with(&["--commands"]) else { return };
        client.agent = "fake-commands".into();
        client
            .prompt("fake-model", None, "first", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "")
            .unwrap();
        let key = command_key("fake-commands", &normalize_cwd(&cwd.to_string_lossy()));
        let cached = command_cache().lock().unwrap().get(&key).cloned().unwrap_or_default();
        assert_eq!(cached.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["compact", "skill:demo"]);
    }
    #[test]
    fn codex_mode_follows_write_permission_and_thinking_uses_thought_level_option() {
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client_with(&["--codex-config"]) else { return };
        client.agent = "codex".into();
        let read_only = client
            .prompt("gpt-5.6-sol", None, "config", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "")
            .unwrap();
        assert_eq!(read_only.channel.text, "config:gpt-5.6-sol:read-only:medium");
        let writer = client
            .prompt("gpt-5.6-sol", None, "config", &cwd, Instant::now() + Duration::from_secs(5), "高", true, false, "")
            .unwrap();
        assert_eq!(writer.channel.text, "config:gpt-5.6-sol:agent:high");
    }
}
