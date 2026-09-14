use crate::process::ProcessResult;
use regex::Regex;
use serde_json::Value;

pub(crate) mod acp;

pub(crate) const KNOWN_MODEL_ROUTES: &[(&str, &str, &str)] = &[
    ("deepseek", "deepseek-v4-flash", "deepseek-flash"),
];

fn known_model_route(provider: &str, requested: &str, actual: &str) -> bool {
    KNOWN_MODEL_ROUTES.iter().any(|(p, req, response)| *p == provider && *req == requested && *response == actual)
}
use std::path::{Path, PathBuf};

pub const BLIND_ARTIFACT_NOTICE: &str = "待验材料已在工作目录：REQUEST.md（用户原话）、diff.patch（统一 diff）、changed/（改后文件全文），请先读取再判定";
use std::process::{Command, Stdio};
use std::sync::OnceLock;

#[derive(Clone, Debug)]
pub struct Invocation {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub input: Option<String>,
}

#[derive(Clone, Debug)]
struct InvocationSpec {
    binary: &'static str,
    fallback: Option<PathBuf>,
    args: Vec<String>,
    input: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
}

#[derive(Clone, Debug, Default)]
pub struct ChannelOutput {
    pub text: String,
    pub usage: Usage,
    pub identity: String,
    pub session_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChannelParseError {
    pub kind: ChannelErrorKind,
    pub message: String,
    pub identity: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelErrorKind {
    Output,
    IdentityMismatch,
}

impl ChannelParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            kind: ChannelErrorKind::Output,
            message: message.into(),
            identity: String::new(),
        }
    }

    fn with_identity(message: impl Into<String>, identity: impl Into<String>) -> Self {
        Self {
            kind: ChannelErrorKind::Output,
            message: message.into(),
            identity: identity.into(),
        }
    }

    fn identity_mismatch(message: impl Into<String>, identity: impl Into<String>) -> Self {
        Self {
            kind: ChannelErrorKind::IdentityMismatch,
            message: message.into(),
            identity: identity.into(),
        }
    }
}

pub fn isolation_for(agent: &str, blind: bool) -> &'static str {
    if blind && matches!(normalize_agent(agent), "claude" | "pi") {
        "完整"
    } else {
        "部分"
    }
}

pub fn build_invocation(
    agent: &str,
    provider: &str,
    model: &str,
    tier: &str,
    thinking: &str,
    blind: bool,
    session_id: Option<&str>,
    prompt: &str,
    task_file: Option<&Path>,
    session_dir: Option<&Path>,
    cc_switch_path: Option<&Path>,
    writable: bool,
) -> Result<Invocation, String> {
    #[cfg(test)]
    if normalize_agent(agent) == "kimi" {
        if let Some((_, oneshot, _)) = test_provider_parts(provider) {
            let program = resolve_binary("python", None)?;
            let mut args = vec![oneshot];
            args.extend(build_invocation_spec(
                "kimi", "", model, tier, thinking, blind, session_id, prompt,
                task_file, session_dir, cc_switch_path, writable,
            )?.args);
            return Ok(Invocation { program, args, input: None });
        }
    }
    let normalized = normalize_agent(agent);
    let spec = build_invocation_spec(
        normalized,
        provider,
        model,
        tier,
        thinking,
        blind,
        session_id,
        prompt,
        task_file,
        session_dir,
        cc_switch_path,
        writable,
    )?;
    let program = resolve_binary(spec.binary, spec.fallback)?;
    if normalized == "kimi" && is_command_script(&program) {
        return Err("Kimi 需要原生 kimi.exe，不能通过 .cmd/.bat 启动".into());
    }
    Ok(Invocation {
        program,
        args: spec.args,
        input: spec.input,
    })
}

#[cfg(test)]
pub(crate) fn test_provider_parts(provider: &str) -> Option<(String, String, Vec<String>)> {
    let encoded = provider.strip_prefix("__aicross_test__|")?;
    let mut parts = encoded.split('|');
    Some((parts.next()?.to_string(), parts.next()?.to_string(), parts.map(str::to_string).collect()))
}

#[allow(clippy::too_many_arguments)]
fn build_invocation_spec(
    agent: &str,
    provider: &str,
    model: &str,
    tier: &str,
    thinking: &str,
    blind: bool,
    session_id: Option<&str>,
    prompt: &str,
    task_file: Option<&Path>,
    session_dir: Option<&Path>,
    cc_switch_path: Option<&Path>,
    writable: bool,
) -> Result<InvocationSpec, String> {
    match agent {
        "claude" => {
            let tools = if blind {
                ""
            } else if writable {
                "Read,Grep,Glob,Write,Edit,Bash"
            } else {
                "Read,Grep,Glob"
            };
            let mut args = vec![
                "-p".into(),
                "--model".into(),
                model.into(),
                "--tools".into(),
                tools.into(),
                "--output-format".into(),
                "json".into(),
                "--effort".into(),
                claude_effort(thinking).into(),
            ];
            if blind {
                args.push("--restricted".into());
            }
            if let Some(id) = session_id.filter(|id| !id.is_empty()) {
                args.extend(["--resume".into(), id.into()]);
            }
            Ok(InvocationSpec {
                binary: "claude",
                fallback: None,
                args,
                input: Some(prompt.into()),
            })
        }
        "codex" => {
            let mut args = vec![
                "exec".into(),
                "-m".into(),
                model.into(),
                "-c".into(),
                format!("model_reasoning_effort={}", codex_effort(thinking)),
                "-s".into(),
                if writable {
                    "workspace-write".into()
                } else {
                    "read-only".into()
                },
                "--skip-git-repo-check".into(),
                "--json".into(),
            ];
            if blind {
                args.push("--ephemeral".into());
            }
            if let Some(id) = session_id.filter(|id| !id.is_empty()) {
                args.extend(["resume".into(), id.into(), "-".into()]);
            } else {
                args.push("-".into());
            }
            Ok(InvocationSpec {
                binary: "codex",
                fallback: None,
                args,
                input: Some(prompt.into()),
            })
        }
        "kimi" => {
            if !model.contains('/') {
                return Err("Kimi 模型请使用 config.toml 里的别名".into());
            }
            let prompt = crop_utf8_bytes(prompt, 30_000);
            let mut args = vec![
                "-p".into(),
                prompt,
                "-m".into(),
                model.into(),
                "--output-format".into(),
                "stream-json".into(),
            ];
            if let Some(id) = session_id.filter(|id| !id.is_empty()) {
                args.extend(["-S".into(), id.into()]);
            }
            Ok(InvocationSpec {
                binary: "kimi",
                fallback: kimi_fallback(),
                args,
                input: None,
            })
        }
        "pi" => {
            if provider.is_empty() {
                return Err("pi 通道缺少 provider".into());
            }
            let mut args = vec![
                "--provider".into(),
                provider.into(),
                "--model".into(),
                model.into(),
                "-p".into(),
            ];
            if blind {
                if prompt.contains(BLIND_ARTIFACT_NOTICE) {
                    args.extend(["--tools".into(), "read,grep,find,ls".into()]);
                } else {
                    args.push("--no-tools".into());
                }
                args.extend([
                    "--no-skills".into(),
                    "--no-prompt-templates".into(),
                    "--no-themes".into(),
                    "--no-approve".into(),
                ]);
            } else {
                let tools = if writable {
                    "read,write,edit,bash,grep,find,ls"
                } else {
                    "read,grep,find,ls"
                };
                args.extend(["--tools".into(), tools.into()]);
            }
            args.extend([
                "--no-extensions".into(),
                "--no-context-files".into(),
                "--mode".into(),
                "json".into(),
                "--thinking".into(),
                pi_thinking(thinking).into(),
            ]);
            if let Some(path) = session_dir {
                args.extend(["--session-dir".into(), path.to_string_lossy().into_owned()]);
            }
            if let Some(id) = session_id.filter(|id| !id.is_empty()) {
                args.extend(["--session-id".into(), id.into()]);
            }
            Ok(InvocationSpec {
                binary: "pi",
                fallback: None,
                args,
                input: Some(prompt.into()),
            })
        }
        "cc-switch" => {
            let script = cc_switch_path
                .filter(|path| !path.as_os_str().is_empty())
                .ok_or_else(|| "未配置 cc_switch.py 路径".to_string())?;
            if !script.is_file() {
                return Err(format!("cc_switch.py 路径不存在：{}", script.display()));
            }
            let task_file = task_file.ok_or_else(|| "cc-switch 缺少 task-file".to_string())?;
            let args = vec![
                script.to_string_lossy().into_owned(),
                "exec".into(),
                "--provider".into(),
                provider.into(),
                "--tier".into(),
                cc_tier(tier).into(),
                "--task-file".into(),
                task_file.to_string_lossy().into_owned(),
                "--usage".into(),
            ];
            Ok(InvocationSpec {
                binary: "python",
                fallback: None,
                args,
                input: None,
            })
        }
        other => Err(format!("不支持的派发 agent：{other}")),
    }
}

fn crop_utf8_bytes(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_string();
    }
    const MARKER: &str = "\n...[已裁剪以适配 Kimi 30000 字节上限]...\n";
    let available = limit.saturating_sub(MARKER.len());
    let front_budget = available * 3 / 5;
    let back_budget = available - front_budget;
    let mut front = front_budget.min(text.len());
    while !text.is_char_boundary(front) {
        front -= 1;
    }
    let mut back_start = text.len().saturating_sub(back_budget);
    while back_start < text.len() && !text.is_char_boundary(back_start) {
        back_start += 1;
    }
    format!("{}{}{}", &text[..front], MARKER, &text[back_start..])
}

fn is_command_script(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
        })
}

pub fn parse_output(
    agent: &str,
    provider: &str,
    model: &str,
    result: &ProcessResult,
) -> Result<ChannelOutput, ChannelParseError> {
    if result.timed_out {
        return Err(ChannelParseError::new(format!(
            "执行超过 {} 秒",
            result.timeout.as_secs()
        )));
    }
    match normalize_agent(agent) {
        "claude" => parse_claude(result).map_err(ChannelParseError::new),
        "codex" => parse_codex(result).map_err(ChannelParseError::new),
        "kimi" => parse_kimi(result).map_err(ChannelParseError::new),
        "pi" => parse_pi(result, provider, model),
        "cc-switch" => parse_cc_switch(result).map_err(ChannelParseError::new),
        _ => Err(ChannelParseError::new("无法解析未知通道输出")),
    }
}

pub(crate) fn normalize_agent(agent: &str) -> &str {
    match agent {
        "kimicode" | "kimi-code" => "kimi",
        "cc_switch" => "cc-switch",
        value => value,
    }
}

fn claude_effort(thinking: &str) -> &'static str {
    match thinking {
        "高" => "high",
        "中" => "medium",
        _ => "low",
    }
}

fn codex_effort(thinking: &str) -> &'static str {
    match thinking {
        "高" => "high",
        "中" => "medium",
        _ => "low",
    }
}

fn pi_thinking(thinking: &str) -> &'static str {
    match thinking {
        "关" => "off",
        "低" => "low",
        "中" => "medium",
        "高" => "high",
        _ => "medium",
    }
}

fn cc_tier(tier: &str) -> &'static str {
    match tier {
        "高" => "opus",
        "中" => "sonnet",
        _ => "haiku",
    }
}

fn kimi_fallback() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".kimi-code/bin/kimi.exe"))
}

pub(crate) fn resolve_binary(binary: &str, fallback: Option<PathBuf>) -> Result<PathBuf, String> {
    let path = Path::new(binary);
    if path.components().count() > 1 && path.is_file() {
        return Ok(path.to_path_buf());
    }
    let lookup = if cfg!(windows) { "where.exe" } else { "which" };
    #[cfg(windows)]
    let names = [
        format!("{binary}.exe"),
        format!("{binary}.cmd"),
        format!("{binary}.bat"),
    ];
    #[cfg(not(windows))]
    let names = [binary.to_string()];
    for name in names {
        let mut command = Command::new(lookup);
        command
            .arg(name)
            .env("AI_CROSS_PEER", "1")
            .stdin(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        if let Ok(output) = command.output() {
            if output.status.success() {
                if let Some(found) = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .find(|line| !line.trim().is_empty())
                {
                    return Ok(PathBuf::from(found.trim()));
                }
            }
        }
    }
    if let Some(path) = fallback.filter(|path| path.is_file()) {
        return Ok(path);
    }
    Err(format!("未找到 {binary} 可执行文件"))
}

fn parse_claude(result: &ProcessResult) -> Result<ChannelOutput, String> {
    let value: Value = serde_json::from_str(result.stdout.trim())
        .map_err(|error| format!("Claude 输出不是 JSON：{error}"))?;
    if value.get("is_error").and_then(Value::as_bool) == Some(true)
        || value
            .get("api_error_status")
            .is_some_and(|status| !status.is_null())
    {
        return Err(value
            .get("result")
            .and_then(Value::as_str)
            .unwrap_or("Claude 返回错误")
            .to_string());
    }
    let text = value
        .get("result")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if text.is_empty() {
        return Err("Claude 未返回正文".into());
    }
    let usage = value.get("usage").unwrap_or(&Value::Null);
    Ok(ChannelOutput {
        text,
        usage: Usage {
            input: usage
                .get("input_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            output: usage
                .get("output_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            cache_read: usage
                .get("cache_read_input_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0),
        },
        identity: claude_identity(&value),
        session_id: value
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    })
}

/// `claude -p --output-format json` 的 result 对象没有顶层 `model`；真身在 `modelUsage`
/// 的键里（如 `claude-fable-5-1`）。优先顶层 `model`（老版本），其次 `modelUsage`：
/// 多个键时取 output_tokens 最多的那个，避免子代理模型混进来。
fn claude_identity(value: &Value) -> String {
    if let Some(model) = value.get("model").and_then(Value::as_str) {
        if !model.is_empty() {
            return model.to_string();
        }
    }
    let Some(usage) = value.get("modelUsage").and_then(Value::as_object) else {
        return String::new();
    };
    usage
        .iter()
        .max_by_key(|(_, stats)| stats.get("outputTokens").and_then(Value::as_u64).unwrap_or(0))
        .map(|(model, _)| model.clone())
        .unwrap_or_default()
}

fn parse_codex(result: &ProcessResult) -> Result<ChannelOutput, String> {
    let mut output = ChannelOutput::default();
    for value in json_lines(&result.stdout) {
        match value.get("type").and_then(Value::as_str).unwrap_or("") {
            "thread.started" => {
                output.session_id = value
                    .get("thread_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
            }
            "item.completed" => {
                let item = value.get("item").unwrap_or(&Value::Null);
                if item.get("type").and_then(Value::as_str) == Some("agent_message") {
                    if let Some(text) = item.get("text").and_then(Value::as_str) {
                        if !output.text.is_empty() {
                            output.text.push('\n');
                        }
                        output.text.push_str(text);
                    }
                }
            }
            "turn.completed" => {
                let usage = value.get("usage").unwrap_or(&Value::Null);
                output.usage.input = usage
                    .get("input_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                output.usage.output = usage
                    .get("output_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                output.usage.cache_read = usage
                    .get("cached_input_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
            }
            _ => {}
        }
    }
    output.text = output.text.trim().to_string();
    if output.text.is_empty() {
        return Err(nonempty_error(result, "Codex 未返回正文"));
    }
    Ok(output)
}

fn parse_pi(
    result: &ProcessResult,
    provider: &str,
    model: &str,
) -> Result<ChannelOutput, ChannelParseError> {
    let values: Vec<Value> = json_lines(&result.stdout).collect();
    if !values
        .iter()
        .any(|v| v.get("type").and_then(Value::as_str) == Some("agent_settled"))
    {
        return Err(ChannelParseError::new(nonempty_error(
            result,
            "pi 未完成响应",
        )));
    }
    let message = values
        .iter()
        .rev()
        .filter_map(|v| v.get("message"))
        .find(|m| m.get("role").and_then(Value::as_str) == Some("assistant"))
        .ok_or_else(|| ChannelParseError::new("pi 未返回 assistant 消息"))?;
    let actual_provider = message
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("");
    let actual_model = message
        .get("responseModel")
        .and_then(Value::as_str)
        .or_else(|| message.get("model").and_then(Value::as_str))
        .unwrap_or("");
    let identity = format!("{actual_provider}/{actual_model}");
    if message.get("stopReason").and_then(Value::as_str) == Some("error") {
        let error = message
            .get("errorMessage")
            .and_then(Value::as_str)
            .unwrap_or("pi 返回错误");
        return Err(ChannelParseError::with_identity(error, identity));
    }
    if (actual_provider != provider || actual_model != model)
        && !known_model_route(provider, model, actual_model)
    {
        return Err(ChannelParseError::identity_mismatch(
            format!("真身不符：请求 {provider}/{model}，响应 {identity}"),
            identity,
        ));
    }
    let text = message
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    if text.is_empty() {
        return Err(ChannelParseError::with_identity(
            "pi 的 text 块为空",
            identity,
        ));
    }
    let usage = message.get("usage").unwrap_or(&Value::Null);
    Ok(ChannelOutput {
        text,
        usage: Usage {
            input: usage
                .get("input")
                .or_else(|| usage.get("input_tokens"))
                .and_then(Value::as_u64)
                .unwrap_or(0),
            output: usage
                .get("output")
                .or_else(|| usage.get("output_tokens"))
                .and_then(Value::as_u64)
                .unwrap_or(0),
            cache_read: usage
                .get("cacheRead")
                .or_else(|| usage.get("cache_read"))
                .or_else(|| usage.get("cache_read_tokens"))
                .and_then(Value::as_u64)
                .unwrap_or(0),
        },
        identity,
        session_id: values
            .iter()
            .find(|v| v.get("type").and_then(Value::as_str) == Some("session"))
            .and_then(|v| v.get("id"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    })
}

fn parse_kimi(result: &ProcessResult) -> Result<ChannelOutput, String> {
    let mut texts = Vec::new();
    let mut usage = Usage::default();
    let mut hinted_session = String::new();
    for value in json_lines(&result.stdout) {
        // 0.42.0 实测：最终回答是 {"role":"assistant","content":"<纯字符串>"}，
        // 工具轮是 {"role":"assistant","tool_calls":[...]}，工具结果是 role=tool。
        // 只收 assistant 的正文；tool 结果里的材料原文不能当回答。
        let role = value.get("role").and_then(Value::as_str);
        if role == Some("tool") {
            continue;
        }
        if role == Some("assistant") {
            if let Some(text) = value.get("content").and_then(Value::as_str) {
                if !text.trim().is_empty() {
                    texts.push(text.to_string());
                }
            }
        }
        if value.get("type").and_then(Value::as_str) == Some("session.resume_hint") {
            if let Some(id) = value.get("session_id").and_then(Value::as_str) {
                hinted_session = id.to_string();
            }
        }
        collect_kimi_fields(&value, &mut texts, &mut usage);
    }
    let text = texts
        .into_iter()
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    if text.is_empty() {
        return Err(nonempty_error(result, "Kimi 未返回正文"));
    }
    let resume = if hinted_session.is_empty() {
        kimi_session_id(&format!("{}\n{}", result.stdout, result.stderr))
    } else {
        hinted_session
    };
    Ok(ChannelOutput {
        text,
        usage,
        session_id: resume,
        ..ChannelOutput::default()
    })
}

fn collect_kimi_fields(value: &Value, output: &mut Vec<String>, usage: &mut Usage) {
    match value {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("text") {
                if let Some(text) = map.get("text").and_then(Value::as_str) {
                    output.push(text.to_string());
                }
            }
            if let Some(current) = map.get("usage") {
                merge_usage(usage, current);
            }
            for child in map.values() {
                collect_kimi_fields(child, output, usage);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_kimi_fields(child, output, usage);
            }
        }
        _ => {}
    }
}

fn merge_usage(target: &mut Usage, value: &Value) {
    let field = |names: &[&str]| {
        names
            .iter()
            .find_map(|name| value.get(*name).and_then(Value::as_u64))
            .unwrap_or(0)
    };
    target.input = target.input.max(field(&[
        "input",
        "input_tokens",
        "inputTokens",
        "prompt_tokens",
    ]));
    target.output = target.output.max(field(&[
        "output",
        "output_tokens",
        "outputTokens",
        "completion_tokens",
    ]));
    target.cache_read = target.cache_read.max(field(&[
        "cacheRead",
        "cache_read",
        "cache_read_tokens",
        "cached_input_tokens",
    ]));
}

fn kimi_session_id(text: &str) -> String {
    static RESUME: OnceLock<Regex> = OnceLock::new();
    let regex = RESUME.get_or_init(|| {
        Regex::new(
            r"(?i)To resume this session:\s*kimi(?:\.exe)?\s+(?:-S|--session|-r|--resume)\s+([0-9a-zA-Z_-]{8,})",
        )
        .expect("kimi resume regex")
    });
    text.lines()
        .rev()
        .find(|line| line.contains("To resume this session:"))
        .and_then(|line| regex.captures(line))
        .and_then(|capture| capture.get(1))
        .map(|value| value.as_str().to_string())
        .unwrap_or_default()
}

fn parse_cc_switch(result: &ProcessResult) -> Result<ChannelOutput, String> {
    let text = result.stdout.trim().to_string();
    if text.is_empty() {
        return Err(nonempty_error(result, "cc-switch 未返回正文"));
    }
    static USAGE_FIELD: OnceLock<Regex> = OnceLock::new();
    let usage_field = USAGE_FIELD.get_or_init(|| {
        Regex::new(r"\b(input|cache_read|output)=(\d+)").expect("cc-switch usage field regex")
    });
    let mut usage = Usage::default();
    if let Some(line) = result
        .stderr
        .lines()
        .find(|line| line.starts_with("[usage]"))
    {
        for capture in usage_field.captures_iter(line) {
            let value = capture[2].parse().unwrap_or(0);
            match &capture[1] {
                "input" => usage.input = value,
                "cache_read" => usage.cache_read = value,
                "output" => usage.output = value,
                _ => {}
            }
        }
    }
    Ok(ChannelOutput {
        text,
        usage,
        ..ChannelOutput::default()
    })
}

fn json_lines(text: &str) -> impl Iterator<Item = Value> + '_ {
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
}

fn nonempty_error(result: &ProcessResult, fallback: &str) -> String {
    let error = result.stderr.trim();
    if error.is_empty() {
        fallback.to_string()
    } else {
        error.chars().take(500).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn result(stdout: &str) -> ProcessResult {
        ProcessResult {
            code: Some(0),
            stdout: stdout.into(),
            stderr: String::new(),
            timed_out: false,
            truncated: false,
            timeout: Duration::from_secs(10),
        }
    }

    #[test]
    fn blind_claude_contains_restricted() {
        let invocation = build_invocation_spec(
            "claude", "", "sonnet", "低", "低", true, None, "x", None, None, None, false,
        )
        .unwrap();
        assert!(invocation.args.iter().any(|arg| arg == "--restricted"));
    }

    #[test]
    fn blind_pi_with_no_context_files_is_fully_isolated() {
        let invocation = build_invocation_spec(
            "pi", "provider", "model", "低", "低", true, None, "x", None, None, None, false,
        )
        .unwrap();
        assert!(invocation
            .args
            .iter()
            .any(|arg| arg == "--no-context-files"));
        assert_eq!(isolation_for("pi", true), "完整");
    }

    #[test]
    fn blind_pi_with_artifacts_keeps_read_only_tools() {
        let prompt = format!("x\n{BLIND_ARTIFACT_NOTICE}");
        let invocation = build_invocation_spec(
            "pi", "provider", "model", "低", "低", true, None, &prompt, None, None, None, false,
        )
        .unwrap();
        let tools = invocation.args.iter().position(|arg| arg == "--tools").unwrap();
        assert_eq!(invocation.args[tools + 1], "read,grep,find,ls");
        assert!(!invocation.args.iter().any(|arg| arg == "--no-tools"));
    }

    #[test]
    fn kimi_crops_more_than_30000_utf8_bytes() {
        let prompt = "中".repeat(10_001);
        let invocation = build_invocation_spec(
            "kimi",
            "",
            "kimi-code/k",
            "中",
            "中",
            true,
            None,
            &prompt,
            None,
            None,
            None,
            false,
        )
        .unwrap();
        let sent = &invocation.args[1];
        assert!(sent.len() <= 30_000);
        assert!(sent.contains("已裁剪以适配 Kimi"));
        assert!(sent.is_char_boundary(sent.len()));
    }

    #[test]
    fn codex_resume_follows_parent_options() {
        let invocation = build_invocation_spec(
            "codex",
            "",
            "gpt-test",
            "高",
            "高",
            false,
            Some("thread-1"),
            "x",
            None,
            None,
            None,
            false,
        )
        .unwrap();
        let resume = invocation
            .args
            .iter()
            .position(|arg| arg == "resume")
            .unwrap();
        for option in ["-m", "-c", "-s", "--skip-git-repo-check", "--json"] {
            assert!(
                invocation
                    .args
                    .iter()
                    .position(|arg| arg == option)
                    .unwrap()
                    < resume
            );
        }
    }

    #[test]
    fn writable_codex_uses_workspace_write_sandbox() {
        let invocation = build_invocation_spec(
            "codex", "", "gpt-test", "低", "低", false, None, "x", None, None, None, true,
        )
        .unwrap();
        let sandbox = invocation.args.iter().position(|arg| arg == "-s").unwrap();
        assert_eq!(invocation.args[sandbox + 1], "workspace-write");
    }

    #[test]
    fn writable_claude_excludes_web_tools_from_allowlist() {
        let invocation = build_invocation_spec(
            "claude", "", "sonnet", "低", "低", false, None, "x", None, None, None, true,
        )
        .unwrap();
        let tools = invocation.args.iter().position(|arg| arg == "--tools").unwrap();
        assert_eq!(invocation.args[tools + 1], "Read,Grep,Glob,Write,Edit,Bash");
        assert!(!invocation.args[tools + 1].contains("WebFetch"));
        assert!(!invocation.args[tools + 1].contains("WebSearch"));
    }

    #[test]
    fn writable_pi_enables_write_tools() {
        let invocation = build_invocation_spec(
            "pi", "provider", "model", "低", "低", false, None, "x", None, None, None, true,
        )
        .unwrap();
        let tools = invocation.args.iter().position(|arg| arg == "--tools").unwrap();
        assert_eq!(invocation.args[tools + 1], "read,write,edit,bash,grep,find,ls");
    }

    #[test]
    fn pi_continuation_preserves_thinking_setting() {
        let invocation = build_invocation_spec(
            "pi",
            "provider",
            "model",
            "中",
            "高",
            false,
            Some("session-1"),
            "x",
            None,
            None,
            None,
            false,
        )
        .unwrap();
        let thinking = invocation
            .args
            .iter()
            .position(|arg| arg == "--thinking")
            .unwrap();
        let session = invocation
            .args
            .iter()
            .position(|arg| arg == "--session-id")
            .unwrap();
        assert_eq!(invocation.args[thinking + 1], "high");
        assert_eq!(invocation.args[session + 1], "session-1");
    }

    #[test]
    fn pi_response_model_mismatch_fails_and_reports_true_identity() {
        let output = include_str!("../../fixtures/channels/pi-response-model-mismatch.jsonl");
        let error = parse_pi(&result(output), "zai-coding-cn", "glm-4.7").unwrap_err();
        assert_eq!(error.kind, ChannelErrorKind::IdentityMismatch);
        assert!(error.message.contains("真身不符"));
        assert_eq!(error.identity, "zai-coding-cn/glm-5.3-flash");
    }

    #[test]
    fn r3_6_pi_accepts_known_deepseek_route_alias_but_not_zai_upgrade() {
        let base = include_str!("../../fixtures/channels/pi-response-model-mismatch.jsonl");
        let deepseek = base.replace("zai-coding-cn", "deepseek").replace("glm-5.3-flash", "deepseek-flash");
        let parsed = parse_pi(&result(&deepseek), "deepseek", "deepseek-v4-flash").unwrap();
        assert_eq!(parsed.identity, "deepseek/deepseek-flash");
        let error = parse_pi(&result(base), "zai-coding-cn", "glm-4.7").unwrap_err();
        assert_eq!(error.kind, ChannelErrorKind::IdentityMismatch);
    }

    #[test]
    fn pi_error_event_surfaces_error_message() {
        let output = include_str!("../../fixtures/channels/pi-error-event.jsonl");
        let error =
            parse_pi(&result(output), "qwen-token-plan-cn", "deepseek-v4-flash").unwrap_err();
        assert!(error.message.contains("AccessDenied.Unpurchased"));
        assert_eq!(error.identity, "qwen-token-plan-cn/deepseek-v4-flash");
    }

    #[test]
    fn pi_reads_camel_case_cache_usage() {
        let output = concat!(
            "{\"type\":\"message_end\",\"message\":{\"role\":\"assistant\",\"provider\":\"p\",\"model\":\"m\",\"content\":[{\"type\":\"text\",\"text\":\"OK\"}],\"usage\":{\"input\":115,\"output\":41,\"cacheRead\":1472},\"stopReason\":\"stop\"}}\n",
            "{\"type\":\"agent_settled\"}\n"
        );
        let parsed = parse_pi(&result(output), "p", "m").unwrap();
        assert_eq!(parsed.usage.input, 115);
        assert_eq!(parsed.usage.output, 41);
        assert_eq!(parsed.usage.cache_read, 1472);
    }

    #[test]
    fn pi_uses_explicit_session_and_app_owned_session_directory() {
        let directory = Path::new("session-store");
        let invocation = build_invocation_spec(
            "pi",
            "provider",
            "model",
            "中",
            "中",
            true,
            Some("fixed-session"),
            "x",
            None,
            Some(directory),
            None,
            false,
        )
        .unwrap();
        let session = invocation
            .args
            .iter()
            .position(|value| value == "--session-id")
            .unwrap();
        let directory_arg = invocation
            .args
            .iter()
            .position(|value| value == "--session-dir")
            .unwrap();
        assert_eq!(invocation.args[session + 1], "fixed-session");
        assert_eq!(invocation.args[directory_arg + 1], "session-store");
    }

    #[test]
    fn kimi_resume_uses_current_cli_session_flag() {
        let invocation = build_invocation_spec(
            "kimi",
            "",
            "kimi-code/model",
            "中",
            "中",
            false,
            Some("session-1234"),
            "x",
            None,
            None,
            None,
            false,
        )
        .unwrap();
        let session = invocation
            .args
            .iter()
            .position(|value| value == "-S")
            .unwrap();
        assert_eq!(invocation.args[session + 1], "session-1234");
    }

    #[test]
    fn kimi_requires_configured_alias() {
        let error = build_invocation_spec(
            "kimi",
            "",
            "bare-model",
            "中",
            "中",
            false,
            None,
            "x",
            None,
            None,
            None,
            false,
        )
        .unwrap_err();
        assert_eq!(error, "Kimi 模型请使用 config.toml 里的别名");
    }

    #[test]
    fn kimi_session_parser_ignores_resume_text_in_answer() {
        let output = concat!(
            "模型正文示例：-r abc12345678\n",
            "To resume this session: kimi -S real-session-123\n"
        );
        assert_eq!(kimi_session_id(output), "real-session-123");
        assert_eq!(kimi_session_id("模型正文示例：-r abc12345678"), "");
    }

    #[test]
    fn kimi_reads_plain_string_assistant_content_and_resume_hint() {
        // 0.42.0 真实事件形状（2026-09-13 留痕 20260913-202357-kimi-*.md）
        let output = concat!(
            "{\"role\":\"meta\",\"type\":\"system.version\",\"version\":\"0.42.0\"}\n",
            "{\"role\":\"assistant\",\"tool_calls\":[{\"type\":\"function\",\"id\":\"t1\",\"function\":{\"name\":\"Read\",\"arguments\":\"{}\"}}]}\n",
            "{\"role\":\"tool\",\"tool_call_id\":\"t1\",\"content\":\"1\\t# 材料原文，不是回答\"}\n",
            "{\"role\":\"assistant\",\"content\":\"# 审查意见\\n\\n结论正文\"}\n",
            "{\"role\":\"meta\",\"type\":\"session.resume_hint\",\"session_id\":\"session_ce1012b1\",\"command\":\"kimi -r session_ce1012b1\",\"content\":\"To resume this session: kimi -r session_ce1012b1\"}\n"
        );
        let parsed = parse_kimi(&result(output)).unwrap();
        assert_eq!(parsed.text, "# 审查意见\n\n结论正文");
        assert!(!parsed.text.contains("材料原文"));
        assert_eq!(parsed.session_id, "session_ce1012b1");
    }

    #[test]
    fn kimi_collects_stream_usage_when_present() {
        let output = concat!(
            "{\"type\":\"message\",\"content\":[{\"type\":\"text\",\"text\":\"OK\"}],\"usage\":{\"input_tokens\":12,\"output_tokens\":3,\"cache_read_tokens\":4}}\n",
            "To resume this session: kimi -S session-1234\n"
        );
        let parsed = parse_kimi(&result(output)).unwrap();
        assert_eq!(parsed.text, "OK");
        assert_eq!(parsed.usage.input, 12);
        assert_eq!(parsed.usage.output, 3);
        assert_eq!(parsed.usage.cache_read, 4);
        assert_eq!(parsed.session_id, "session-1234");
    }

    #[test]
    fn cc_switch_reads_usage_from_stderr() {
        let mut process = result("answer");
        process.stderr = "[usage] provider=p model=m input=20 fresh=4 cache_create=6 cache_read=10 output=7 turns=1 ms=100\n".into();
        let parsed = parse_cc_switch(&process).unwrap();
        assert_eq!(parsed.usage.input, 20);
        assert_eq!(parsed.usage.cache_read, 10);
        assert_eq!(parsed.usage.output, 7);
        process.stderr = "[usage] output=9 cache_read=8 provider=p input=7\n".into();
        let reordered = parse_cc_switch(&process).unwrap();
        assert_eq!(
            (
                reordered.usage.input,
                reordered.usage.cache_read,
                reordered.usage.output
            ),
            (7, 8, 9)
        );
    }

    #[test]
    fn cc_switch_requires_configured_script_path() {
        let error = build_invocation_spec(
            "cc-switch",
            "",
            "model",
            "中",
            "中",
            false,
            None,
            "x",
            None,
            None,
            None,
            false,
        )
        .unwrap_err();
        assert_eq!(error, "未配置 cc_switch.py 路径");
    }

    #[test]
    fn scheduling_aliases_normalize_to_one_agent_key() {
        assert_eq!(normalize_agent("kimi-code"), "kimi");
        assert_eq!(normalize_agent("kimicode"), "kimi");
        assert_eq!(normalize_agent("cc_switch"), "cc-switch");
    }

    #[test]
    fn claude_null_api_error_status_is_not_an_error() {
        let parsed = parse_claude(&result(
            r#"{"is_error":false,"api_error_status":null,"result":"OK","usage":{}}"#,
        ))
        .unwrap();
        assert_eq!(parsed.text, "OK");
    }

    #[test]
    fn claude_identity_comes_from_model_usage_keys_when_no_top_level_model() {
        let parsed = parse_claude(&result(
            r#"{"is_error":false,"result":"OK","usage":{},"modelUsage":{"claude-fable-5-1":{"inputTokens":3,"outputTokens":5}}}"#,
        ))
        .unwrap();
        assert_eq!(parsed.identity, "claude-fable-5-1");
        let multi = parse_claude(&result(
            r#"{"is_error":false,"result":"OK","usage":{},"modelUsage":{"claude-haiku-4-5-20251001":{"outputTokens":2},"claude-fable-5-1":{"outputTokens":9}}}"#,
        ))
        .unwrap();
        assert_eq!(multi.identity, "claude-fable-5-1");
        let legacy = parse_claude(&result(r#"{"is_error":false,"result":"OK","model":"claude-opus-5"}"#)).unwrap();
        assert_eq!(legacy.identity, "claude-opus-5");
    }

    #[test]
    fn codex_high_maps_to_high_and_never_to_xhigh() {
        assert_eq!(codex_effort("关"), "low");
        assert_eq!(codex_effort("低"), "low");
        assert_eq!(codex_effort("中"), "medium");
        assert_eq!(codex_effort("高"), "high");
    }
}
