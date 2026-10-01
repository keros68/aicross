use super::{ChannelOutput, Usage};
use crate::process::{spawn_interactive, InteractiveProcess, ProcessActivity, RecvLineError};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
#[cfg(test)]
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, TryLockError};
#[cfg(test)]
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const MAX_RAW: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AcpCommand {
    pub name: String,
    pub description: String,
    pub hint: String,
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

/// 挂起的权限请求（TASK-40 R1-2 起只有测试用）：登记后等回答方送选项。
#[cfg(test)]
fn pending_permissions() -> &'static Mutex<HashMap<String, mpsc::Sender<Option<String>>>> {
    static PENDING: OnceLock<Mutex<HashMap<String, mpsc::Sender<Option<String>>>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 仅测试：旧「登记挂起、等回答」的协议路径开关。生产没有回答方
/// （权限卡片随窗口界面下线），需要授权的请求一律当场拒绝；想验证
/// 应答协议形状的测试先打开它。与 `test_pool_guard` 成对用，避免并行
/// 测试误入这条路。
#[cfg(test)]
static TEST_ANSWER_CHANNEL: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
fn test_answer_channel_enabled() -> bool {
    TEST_ANSWER_CHANNEL.load(Ordering::SeqCst)
}

/// 仅测试：打开 / 关闭挂起应答路径。走 `TestAnswerChannelGuard` 自动复位。
#[cfg(test)]
fn test_set_answer_channel(enabled: bool) {
    TEST_ANSWER_CHANNEL.store(enabled, Ordering::SeqCst);
}

/// 测试作用域内打开挂起应答路径，drop 时自动还原；断言失败也不会留下开关。
/// 供本模块与 dispatch 的 interactive 测试用（fix-40-r1 R1-2）。
#[cfg(test)]
pub(crate) struct TestAnswerChannelGuard {
    previous: bool,
}

#[cfg(test)]
impl TestAnswerChannelGuard {
    pub(crate) fn enable() -> Self {
        let previous = test_answer_channel_enabled();
        test_set_answer_channel(true);
        Self { previous }
    }
}

#[cfg(test)]
impl Drop for TestAnswerChannelGuard {
    fn drop(&mut self) {
        test_set_answer_channel(self.previous);
    }
}

fn command_cache() -> &'static Mutex<HashMap<String, Vec<AcpCommand>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Vec<AcpCommand>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
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
    /// 本轮是 interactive 会话（宿主对话里派发的单线会话）时为
    /// `Some(dispatch_key)`：非读非写的权限请求走授权询问路径。
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
/// kimi 在流水线与 interactive 会话都走 ACP；claude / codex 只在 interactive 会话走 ACP——
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
    // claude / codex 的适配器由应用自备（acp_setup）：缺了就在后台装，本轮以 Process 错误回退一次性 CLI。
    let (program, args) = match normalized.as_str() {
        "claude" | "codex" => {
            let program = super::acp_setup::resolve_adapter(&normalized).map_err(|_| {
                super::acp_setup::ensure_installed();
                AcpError::new(AcpErrorKind::Process, super::acp_setup::unavailable_reason(&normalized))
            })?;
            (program, Vec::new())
        }
        _ => (
            super::resolve_binary("kimi", super::kimi_fallback()).map_err(|e| AcpError::new(AcpErrorKind::Process, e))?,
            vec!["acp".to_string()],
        ),
    };
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
        let text = strip_adapter_notices(&text);
        // interactive 会话只展示回答本身：原文完整留在 raw_stdout（记录），流水线节点不做这一步
        // （审查者的 PASS / NEEDS_FIX 可能是单独一条短消息，不能丢）。
        let text = if self.interactive.is_some() {
            console_reply(&raw, &session_id).unwrap_or(text)
        } else {
            text
        };
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
    }

    /// 权限请求的处理（TASK-40 R1-2）：窗口界面下线后没有回答方，执行命令、
    /// 写文件、MCP 调用这类需要授权的请求当场按拒绝回复，不登记挂起、不等到
    /// 本轮 deadline；读类请求在此之前已按只读策略自动放行。测试可用
    /// `TestAnswerChannelGuard` 打开旧的挂起应答路径，钉住协议形状。
    fn ask_user_permission(
        &mut self,
        message: &Value,
        dispatch_key: &str,
        deadline: Instant,
        cancel_key: &str,
    ) -> Result<(), AcpError> {
        #[cfg(test)]
        if test_answer_channel_enabled() {
            return self.ask_user_permission_awaiting_answer(message, dispatch_key, deadline, cancel_key);
        }
        let _ = (dispatch_key, deadline, cancel_key);
        self.reject_permission_request(message)
    }

    /// 按拒绝回一条权限应答：有 reject_once 选项就选它，否则 outcome=cancelled。
    fn reject_permission_request(&mut self, message: &Value) -> Result<(), AcpError> {
        let selected = permission_options(message)
            .into_iter()
            .find(|option| option.kind == "reject_once")
            .map(|option| option.option_id);
        let result = selected
            .map(|id| json!({"outcome":{"outcome":"selected","optionId":id}}))
            .unwrap_or_else(|| json!({"outcome":{"outcome":"cancelled"}}));
        self.process
            .send_json_line(&json!({"jsonrpc":"2.0","id":message["id"],"result":result}))
            .map_err(|e| AcpError::new(AcpErrorKind::Process, e))
    }

    /// 旧的挂起应答路径（仅测试）：登记挂起后阻塞到引擎取消或超时；两者按
    /// 拒绝回。生产路径不再进入（见 `ask_user_permission`）。
    #[cfg(test)]
    fn ask_user_permission_awaiting_answer(
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
        // 登记挂起；锁不可用时立即按拒绝回。
        let registered = match pending_permissions().lock() {
            Ok(mut pending) => {
                pending.insert(request_id.clone(), sender);
                true
            }
            Err(_) => false,
        };
        if !registered {
            let reject = options.iter().find(|option| option.kind == "reject_once").map(|option| option.option_id.clone());
            let result = reject
                .map(|id| json!({"outcome":{"outcome":"selected","optionId":id}}))
                .unwrap_or_else(|| json!({"outcome":{"outcome":"cancelled"}}));
            return self
                .process
                .send_json_line(&json!({"jsonrpc":"2.0","id":message["id"],"result":result}))
                .map_err(|e| AcpError::new(AcpErrorKind::Process, e));
        }
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

/// 适配器自己塞进回复正文的提示（codex-acp 的技能预算告警），不是模型的回答。
fn is_adapter_notice(text: &str) -> bool {
    text.trim_start().starts_with("Warning: Skill descriptions were shortened")
}

fn strip_adapter_notices(text: &str) -> String {
    text.split("\n\n")
        .filter(|paragraph| !is_adapter_notice(paragraph))
        .collect::<Vec<_>>()
        .join("\n\n")
        .trim()
        .to_string()
}

/// 结尾那句「已完成。」之类的固定客套话（codex 在 Stop 钩子后常补一句），前面有正文时才丢。
fn is_acknowledgement(text: &str) -> bool {
    let bare = text.trim().trim_end_matches(['。', '.', '!', '！']).to_ascii_lowercase();
    matches!(bare.as_str(), "已完成" | "完成" | "完成了" | "done" | "好的" | "ok")
}

/// 按 messageId 与工具调用切段，只留最后一次工具调用之后的消息（之前的是「我先看一下…」这类开场白）。
/// 最后一次工具调用之后没有正文时返回 None，由调用方用完整正文兜底。
fn console_reply(raw: &str, session_id: &str) -> Option<String> {
    let mut segments: Vec<(String, String, usize)> = Vec::new();
    let mut epoch = 0usize;
    for line in raw.lines() {
        let Ok(message) = serde_json::from_str::<Value>(line) else { continue };
        if message.get("method").and_then(Value::as_str) != Some("session/update")
            || message.pointer("/params/sessionId").and_then(Value::as_str) != Some(session_id)
        {
            continue;
        }
        let update = &message["params"]["update"];
        match update.get("sessionUpdate").and_then(Value::as_str) {
            Some("tool_call") => epoch += 1,
            Some("agent_message_chunk") | Some("agent_message") => {
                let text = update.pointer("/content/text").and_then(Value::as_str).unwrap_or("");
                let id = update.get("messageId").and_then(Value::as_str).unwrap_or("").to_string();
                match segments.last_mut() {
                    Some(last) if last.0 == id && last.2 == epoch => last.1.push_str(text),
                    _ => segments.push((id, text.to_string(), epoch)),
                }
            }
            _ => {}
        }
    }
    let mut kept = segments
        .into_iter()
        .filter(|(_, _, segment_epoch)| *segment_epoch == epoch)
        .map(|(_, text, _)| strip_adapter_notices(&text))
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();
    while kept.len() > 1 && kept.last().is_some_and(|text| is_acknowledgement(text)) {
        kept.pop();
    }
    (!kept.is_empty()).then(|| kept.join("\n\n"))
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

/// 写文件类请求：interactive 会话按只读对待，这类请求直接拒。kind 缺失（kimi）时按 title 认。
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
        let sender = pending_permissions()
            .lock()
            .unwrap()
            .iter()
            .find(|(key, _)| key.starts_with(prefix))
            .map(|(key, sender)| (key.clone(), sender.clone()));
        if let Some((id, sender)) = sender {
            pending_permissions().lock().unwrap().remove(&id);
            sender.send(option.map(str::to_string)).unwrap();
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
    fn console_reply_keeps_only_the_answer_after_the_last_tool_call() {
        let line = |update: Value| json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s","update":update}}).to_string();
        let chunk = |id: &str, text: &str| line(json!({"sessionUpdate":"agent_message_chunk","messageId":id,"content":{"text":text}}));
        let raw = [
            chunk("m0", "Warning: Skill descriptions were shortened to fit the skills context budget."),
            chunk("m1", "我查看一下测试文件。"),
            line(json!({"sessionUpdate":"tool_call","title":"Read file"})),
            chunk("m2", "`tests/test_beta.py` 测试了 "),
            chunk("m2", "`shout()` 和 `whisper()`。"),
            line(json!({"sessionUpdate":"agent_thought_chunk","content":{"text":"…"}})),
            chunk("m3", "已完成。"),
            json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"other","update":{"sessionUpdate":"agent_message_chunk","content":{"text":"别的会话"}}}}).to_string(),
        ]
        .join("\n");
        assert_eq!(console_reply(&raw, "s").as_deref(), Some("`tests/test_beta.py` 测试了 `shout()` 和 `whisper()`。"));
        // 只有一句「已完成」时不丢；最后一次工具调用之后没有正文时交给调用方兜底。
        let only_ack = [line(json!({"sessionUpdate":"tool_call"})), chunk("a", "已完成。")].join("\n");
        assert_eq!(console_reply(&only_ack, "s").as_deref(), Some("已完成。"));
        let answer_then_tool = [chunk("a", "答案在这"), line(json!({"sessionUpdate":"tool_call"}))].join("\n");
        assert_eq!(console_reply(&answer_then_tool, "s"), None);
        assert_eq!(strip_adapter_notices("Warning: Skill descriptions were shortened x\n\n正文"), "正文");
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
    }
    #[test]
    fn interactive_execute_permission_is_rejected_immediately_without_waiting() {
        // TASK-40 R1-2：无窗口形态没有回答方，需要授权的请求（执行命令、
        // 写文件、MCP 调用）当场按拒绝回，不登记挂起、不等本轮 deadline；
        // 读类请求仍走自动放行（见
        // `interactive_turn_still_auto_answers_reads_and_blind_turns_never_ask`）。
        let _pool_guard = test_pool_guard();
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client() else { return };
        client.interactive = Some("card-run:immediate".into());
        let deadline = Instant::now() + Duration::from_secs(10);
        let started = Instant::now();
        let rejected = client
            .prompt("fake-model", None, "execute", &cwd, deadline, "中", false, false, "")
            .unwrap();
        let elapsed = started.elapsed();
        // 走挂起路径会等满 deadline；当场拒绝应远小于它。
        assert!(elapsed < Duration::from_secs(2), "拒绝耗时 {elapsed:?}，疑似在等回答方");
        assert_eq!(rejected.channel.text, "turn-1:no");
        assert!(pending_permissions().lock().unwrap().keys().all(|key| !key.starts_with("card-run:")));
    }
    #[test]
    fn late_permission_from_another_session_is_auto_answered_not_carded() {
        // 另一会话的执行类请求按只读策略拒掉；本会话这条走挂起应答路径
        // （仅测试打开，见 TASK-40 R1-2），钉住「回答 → 选中项」的协议形状。
        let _pool_guard = test_pool_guard();
        let _answer_channel = TestAnswerChannelGuard::enable();
        let cwd = std::env::current_dir().unwrap();
        let Some(mut client) = fake_client_with(&["--other-session-permission"]) else { return };
        client.interactive = Some("card-other".into());
        let answer = thread::spawn(|| test_answer_pending("card-other#", Some("once")));
        let result = client
            .prompt("fake-model", None, "first", &cwd, Instant::now() + Duration::from_secs(5), "中", false, false, "")
            .unwrap();
        answer.join().unwrap();
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
        // 盲节点即使带了 interactive 也不询问授权，按只读策略拒绝执行类。
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
