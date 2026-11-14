//! 支持清单里新增 agent 的一次性 CLI 通道（TASK-61 / TASK-74），与
//! `build_invocation_spec` 里的 claude / codex / kimi / pi 并列。prompt 只走
//! stdin（或文件引用），绝不走 argv 多行。
//!
//! - Antigravity（`agy`）：
//!   - `1.3.1` 实测：无头模式下工具全被拒，只接纯文本盲审；
//!   - `1.5.6` / `1.5.8`（2026-11-13、11-14 真机实测）：无头模式已放行只读工具
//!     （`read_file` / `view_file` / `list_dir` / `grep_search` / `find_by_name`，
//!     可分页读 >300 KB 文件）。权限规则写成「类别(目标)」，类别有 `write_file`、
//!     `command`、`mcp`；只写类别名的规则不生效。`--settings <file>` 传
//!     `permissions.deny: ["write_file(*)", "command(*)", "mcp(*)"]` 可在
//!     `trustedWorkspaces` 内挡住写文件与命令执行，被挡的工具向模型返回
//!     `Permission denied`，本轮继续；`permissions.allow` 用带结尾 `/` 的正斜杠
//!     目录 `write_file(D:/path/)` 并拒绝命令时可跑单写者位置；
//!     **绝不加 `--dangerously-skip-permissions`**。
//! - Cursor / CodeBuddy / Copilot / Qoder：命令模板只照官方文档写，本机没装、
//!   没跑通；prompt 经 stdin 或文件传入的方式也没核实，所以派发直接失败并
//!   提示「未实测通道」，不猜。真机核实后把对应项改成实际的 stdin 用法。

use super::{json_lines, nonempty_error, ChannelOutput, InvocationSpec, Usage};
use crate::process::ProcessResult;
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;

/// `< 1.5.0` 的 agy 与材料全部内联的盲位置，要求直接用文字回答。
pub(crate) const AGY_NO_TOOLS: &str = "不要使用任何工具，直接用文字回答。";

/// `>= 1.5.0` 的 agy 落在「单写者」位置时附加的说明：只改文件、不跑命令。
pub(crate) const AGY_SINGLE_WRITER_NOTE: &str =
    "不能执行命令，改完文件后直接汇报，验证由引擎跑。";

/// `>= 1.5.0` 的 agy 落在只读或带文件盲位置时附加的说明：只调只读文件工具，
/// 避免模型尝试 `run_command` 触发无头模式自动拒绝而中断整轮。
pub(crate) const AGY_READONLY_NOTE: &str =
    "只用只读文件工具（read_file、view_file、list_dir、find_by_name、grep_search），不能执行命令或修改文件。";

/// agy 1.5.6 工具模式。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum AgyToolMode {
    /// `< 1.5.0` 或 <=200 KB 纯文本已全内联的盲位置：不给工具。
    #[default]
    NoTools,
    /// `>= 1.5.0` 读目录 / >200 KB / `project/` 只读副本，配合 `--settings` 禁止写入与命令。
    ReadOnlyFiles,
    /// `>= 1.5.0` 单写者：配合 `--settings` 只允许写工作目录并禁止命令。
    SingleWriter,
}

/// 禁止写文件的规则。agy 的规则必须带目标：1.5.8 真机实测只写 `write_file`
/// 时文件照样写出，`write_file(*)` 才拦住新建与修改。
const AGY_DENY_WRITE: &str = "write_file(*)";
/// 禁止命令与 MCP 工具的规则，同样必须带目标：只写 `command` 时，用户全局
/// 设置里放行过的命令（如 `git status`）照样执行。
const AGY_DENY_COMMAND: [&str; 2] = ["command(*)", "mcp(*)"];

/// 生成盲 / 只读位置用的 `--settings` JSON：即使工作目录位于
/// `trustedWorkspaces`，也显式禁止写文件、命令执行与 MCP 工具。
pub(crate) fn agy_readonly_settings_json() -> String {
    serde_json::to_string_pretty(&json!({
        "permissions": {
            "deny": [AGY_DENY_WRITE, AGY_DENY_COMMAND[0], AGY_DENY_COMMAND[1]]
        }
    }))
    .unwrap_or_default()
}

/// 把工作目录规范成 `write_file(<dir>/)` 规则字符串：剥离 Windows `\\?\`
/// 扩展前缀、统一 `/` 分隔、末尾仅保留一个 `/`（2026-11-13 agy 1.5.6 实测：
/// `*` / `**` 通配规则全部被拒，只有正斜杠加结尾 `/` 的目录前缀规则生效）。
pub(crate) fn agy_write_allow_rule(workdir: &Path) -> String {
    let canonical = std::fs::canonicalize(workdir).unwrap_or_else(|_| workdir.to_path_buf());
    let raw = canonical.to_string_lossy();
    let stripped = if let Some(unc) = raw.strip_prefix(r"\\?\UNC\") {
        format!("//{unc}")
    } else {
        raw.strip_prefix(r"\\?\").unwrap_or(&raw).to_string()
    };
    let slash = stripped.replace('\\', "/");
    let trimmed = slash.trim_end_matches('/');
    format!("write_file({trimmed}/)")
}

/// 生成单写者位置用的 `--settings` JSON：仅放行当前工作目录内写文件，拒绝全部命令。
pub(crate) fn agy_single_writer_settings_json(workdir: &Path) -> String {
    serde_json::to_string_pretty(&json!({
        "permissions": {
            "allow": [agy_write_allow_rule(workdir)],
            "deny": AGY_DENY_COMMAND
        }
    }))
    .unwrap_or_default()
}

/// 是不是本模块接管的 agent。
pub(super) fn handles(agent: &str) -> bool {
    matches!(agent, "agy" | "cursor" | "codebuddy" | "copilot" | "qoder")
}

/// 拼命令（兼容旧测试调用，按无 `--settings` 的 `< 1.5.0` 纯文本盲审路径）。
#[cfg(test)]
pub(super) fn spec(
    agent: &str,
    model: &str,
    thinking: &str,
    blind: bool,
    prompt: &str,
    timeout: Duration,
) -> Result<InvocationSpec, String> {
    spec_with_agy(
        agent,
        model,
        thinking,
        blind,
        if blind { "只读" } else { "" },
        prompt,
        timeout,
        AgyToolMode::NoTools,
        None,
    )
}

/// 拼命令；支持 agy `>= 1.5.0` 经 `--settings` 放行盲目录读、共享只读或单写者。
#[allow(clippy::too_many_arguments)]
pub(super) fn spec_with_agy(
    agent: &str,
    model: &str,
    thinking: &str,
    blind: bool,
    write: &str,
    prompt: &str,
    timeout: Duration,
    agy_mode: AgyToolMode,
    agy_settings: Option<&Path>,
) -> Result<InvocationSpec, String> {
    if agent == "agy" {
        if agy_settings.is_none() && !blind {
            return Err(crate::agent_catalog::AGY_BLIND_ONLY_REASON.to_string());
        }
        if !blind && write == "worktree" {
            return Err(crate::agent_catalog::AGY_NO_WORKTREE_REASON.to_string());
        }
    } else if !blind {
        let reason = crate::agent_catalog::blind_only_reason(agent)
            .unwrap_or(crate::agent_catalog::UNTESTED_BLIND_ONLY);
        return Err(reason.to_string());
    }
    if model.trim().is_empty() {
        return Err(format!("{agent} 通道缺少模型"));
    }
    match agent {
        "agy" => Ok(agy_spec(model, thinking, prompt, timeout, agy_mode, agy_settings)),
        _ => Err(untested_error(agent, model)),
    }
}

fn agy_spec(
    model: &str,
    thinking: &str,
    prompt: &str,
    timeout: Duration,
    agy_mode: AgyToolMode,
    agy_settings: Option<&Path>,
) -> InvocationSpec {
    let mut prompt = prompt.to_string();
    match agy_mode {
        AgyToolMode::NoTools => {
            if !prompt.contains(AGY_NO_TOOLS) {
                prompt.push_str("\n\n");
                prompt.push_str(AGY_NO_TOOLS);
            }
        }
        AgyToolMode::ReadOnlyFiles => {}
        AgyToolMode::SingleWriter => {
            if !prompt.contains(AGY_SINGLE_WRITER_NOTE) {
                prompt.push_str("\n\n");
                prompt.push_str(AGY_SINGLE_WRITER_NOTE);
            }
        }
    }
    let line = json!({
        "event": "user",
        "message": { "role": "user", "content": prompt },
    });
    let mut args = vec![
        "--model".to_string(),
        model.to_string(),
        "--input-format".into(),
        "stream-json".into(),
        "--output-format".into(),
        "stream-json".into(),
        "--print-timeout".into(),
        format!("{}s", timeout.as_secs().max(1)),
    ];
    // 模型 id 自带档位后缀（`gemini-3.8-flash-low` 等，`agy models` 列出的全是）
    // 时不传 --effort：1.3.1 真机实测两者不一致直接报
    // `--model … conflicts with --effort=…`，status ERROR。
    if let Some(effort) = agy_effort(thinking).filter(|_| !model_has_effort_suffix(model)) {
        args.extend(["--effort".into(), effort.into()]);
    }
    if let Some(settings_path) = agy_settings {
        args.extend([
            "--settings".into(),
            settings_path.to_string_lossy().into_owned(),
        ]);
    }
    InvocationSpec {
        binary: "agy",
        // 程序路径由 build_invocation 经 agent_catalog::resolve_program 解析
        fallback: None,
        args,
        input: Some(format!("{line}\n")),
    }
}

fn model_has_effort_suffix(model: &str) -> bool {
    model_effort_suffix(model).is_some()
}

/// 模型 id 末尾自带的档位（`gemini-3.8-flash-low` → `low`）。
fn model_effort_suffix(model: &str) -> Option<&'static str> {
    let model = model.trim().to_ascii_lowercase();
    ["xhigh", "medium", "high", "low", "max"]
        .into_iter()
        .find(|level| model.ends_with(&format!("-{level}")))
}

/// 实际生效的思考强度（fix-61-r1 5）：agy 的模型 id 自带档位时由模型决定，
/// 记成「由模型决定：low」；其余原样返回位置 / 角色设的值。留痕、run 摘要与
/// `config show` 的 effective 都记这个值。
pub(crate) fn effective_thinking(agent: &str, provider: &str, model: &str, thinking: &str) -> String {
    if super::normalize_agent(agent.trim()).eq_ignore_ascii_case("pi") {
        return read_pi_catalog().map_or_else(|| thinking.to_string(), |catalog| {
            effective_pi_thinking(&catalog, provider, model, thinking)
        });
    }
    if super::normalize_agent(agent.trim()).eq_ignore_ascii_case("agy") {
        if let Some(level) = model_effort_suffix(model) {
            return format!("{MODEL_DECIDED_PREFIX}{level}");
        }
    }
    thinking.to_string()
}

fn read_pi_catalog() -> Option<Value> {
    let path = dirs::home_dir()?.join(".pi/agent/models-store.json");
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn effective_pi_thinking(catalog: &Value, provider: &str, model: &str, thinking: &str) -> String {
    let (level, adjusted) = resolve_pi_thinking(catalog, provider, model, thinking);
    if adjusted { format!("已调整为：{level}") } else { thinking.to_string() }
}

fn resolve_pi_thinking(catalog: &Value, provider: &str, model_id: &str, requested: &str) -> (String, bool) {
    const LEVELS: [&str; 7] = ["off", "minimal", "low", "medium", "high", "xhigh", "max"];
    let min_medium = requested.starts_with("已调整为：") && requested.strip_prefix("已调整为：") == Some("中");
    let requested = requested.strip_prefix("已调整为：").unwrap_or(requested);
    let old = super::pi_thinking(requested);
    let Some(model) = catalog.get(provider).and_then(|p| p.get("models")).and_then(Value::as_array)
        .and_then(|models| models.iter().find(|m| m.get("id").and_then(Value::as_str) == Some(model_id))) else { return (old.into(), false) };
    let Some(reasoning) = model.get("reasoning").and_then(Value::as_bool) else { return (old.into(), false) };
    let supported: Vec<&str> = if !reasoning { vec!["off"] } else {
        let map = model.get("thinkingLevelMap").and_then(Value::as_object);
        LEVELS.into_iter().filter(|level| {
            map.and_then(|m| m.get(*level)).is_none_or(|v| !v.is_null())
                && (!matches!(*level, "xhigh" | "max") || map.is_some_and(|m| m.contains_key(*level)))
        }).collect()
    };
    if supported.contains(&old) { return (old.into(), false); }
    let index = LEVELS.iter().position(|l| *l == old).unwrap_or(3);
    let higher = || supported.iter().find(|l| LEVELS.iter().position(|x| *x == **l).unwrap_or(0) > index);
    let lower = || supported.iter().rev().find(|l| LEVELS.iter().position(|x| *x == **l).unwrap_or(0) < index && (**l != "off" || old == "off"));
    // 多步执行任务拉高到不低于「中」时，若模型缺 medium 优先选更高的 supported 档位（如 high），避免又掉回 low。
    let chosen = if min_medium {
        higher().or_else(lower)
    } else {
        lower().or_else(higher)
    }
    .copied()
    .unwrap_or("off");
    (chosen.into(), chosen != old)
}

const MODEL_DECIDED_PREFIX: &str = "由模型决定：";

/// TASK-74：粗略启发式，用于「执行位置遇到多步任务时思考不低于中」。
/// 满足下列任一条即视为超过一步：
/// 1. 在非代码块区域内，以步骤编号开头的非空行不少于 2 行（如 `1.`、`2)`、`1、`、`(1)`、`（1）`、`一、`、`第1步`、`步骤1`）；
/// 2. 文本中出现至少 2 个带常见扩展名的不同文件/路径（忽略大小写，统一 `/` 分隔）。
/// 注意：这是粗略启发式，不解析语义。
pub(crate) fn is_multi_step_task(text: &str) -> bool {
    use regex::Regex;
    use std::collections::HashSet;
    use std::sync::OnceLock;

    static STEP_LINE: OnceLock<Regex> = OnceLock::new();
    static FILE_TOKEN: OnceLock<Regex> = OnceLock::new();
    let step_re = STEP_LINE.get_or_init(|| {
        Regex::new(
            r"^\s*(?:\d+\s*[.)、]|[（(]\d+[)）]|[一二三四五六七八九十]+\s*、|第\s*[一二三四五六七八九十\d]+\s*步|步骤\s*\d+)\s*\S",
        )
        .expect("step line regex")
    });
    let file_re = FILE_TOKEN.get_or_init(|| {
        Regex::new(
            r"(?i)(?:^|[\s`(\[{\x22'：:，,、；;])([A-Za-z0-9_./\\-]+\.(?:rs|ts|tsx|js|jsx|mjs|cjs|py|json|toml|yaml|yml|md|html|css|scss|sh|ps1|cmd|bat|go|java|kt|c|cc|cpp|h|hpp|sql|vue|svelte))\b",
        )
        .expect("file token regex")
    });

    let mut step_lines = 0usize;
    let mut files: HashSet<String> = HashSet::new();
    let mut in_fence = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence && step_re.is_match(line) {
            step_lines += 1;
            if step_lines >= 2 {
                return true;
            }
        }
        for cap in file_re.captures_iter(line) {
            if let Some(m) = cap.get(1) {
                let norm = m.as_str().replace('\\', "/").to_ascii_lowercase();
                if files.insert(norm) && files.len() >= 2 {
                    return true;
                }
            }
        }
    }
    false
}

/// TASK-74：执行位置在多步任务下思考档位不低于「中」，提升时标「已调整为：中」。
pub(crate) fn apply_execution_thinking_floor(role: &str, task_text: &str, thinking: &str) -> String {
    let trimmed = thinking.trim();
    if role == "执行"
        && matches!(trimmed, "" | "关" | "off" | "minimal" | "低" | "low")
        && is_multi_step_task(task_text)
    {
        return "已调整为：中".to_string();
    }
    thinking.to_string()
}

/// 思考档：低 / 中 / 高 → low / medium / high；其余不传，用 agy 默认。
fn agy_effort(thinking: &str) -> Option<&'static str> {
    match thinking.strip_prefix("已调整为：").unwrap_or(thinking) {
        "低" | "low" => Some("low"),
        "中" | "medium" => Some("medium"),
        "高" | "high" | "xhigh" | "max" => Some("high"),
        _ => None,
    }
}

/// 未实测四家的命令模板（按官方文档；prompt 的传入方式未核实，不含 prompt）。
/// - cursor：`cursor-agent -p --output-format json --model <M>`
///   https://docs.cursor.com/en/cli/headless
/// - codebuddy：`codebuddy -p --output-format json --model <M> --permission-mode plan`
///   https://www.workbuddy.ai/docs/cli
/// - copilot：`copilot -p … --model <M> --deny-tool write --deny-tool shell`（禁用写与命令类工具）
///   https://docs.github.com/en/copilot/how-tos/use-copilot-agents/use-copilot-cli
/// - qoder：`qodercli -p --model <档位>`
///   https://docs.qoder.com/cli/using-cli
pub(super) fn untested_template(agent: &str, model: &str) -> Vec<String> {
    let parts: Vec<&str> = match agent {
        "cursor" => vec!["cursor-agent", "-p", "--output-format", "json", "--model", model],
        "codebuddy" => vec![
            "codebuddy", "-p", "--output-format", "json", "--model", model,
            "--permission-mode", "plan",
        ],
        "copilot" => vec![
            "copilot", "-p", "…", "--model", model, "--deny-tool", "write", "--deny-tool", "shell",
        ],
        "qoder" => vec!["qodercli", "-p", "--model", model],
        _ => Vec::new(),
    };
    parts.into_iter().map(str::to_string).collect()
}

fn untested_error(agent: &str, model: &str) -> String {
    format!(
        "未实测通道：{agent} 的命令模板（{}）只照官方文档写，prompt 经 stdin 或文件传入的方式未核实，暂不派发",
        untested_template(agent, model).join(" ")
    )
}

/// 解析输出；None 表示不是本模块的 agent。
pub(super) fn parse(agent: &str, result: &ProcessResult) -> Option<Result<ChannelOutput, String>> {
    Some(match agent {
        "agy" => parse_agy(result),
        "cursor" | "codebuddy" => parse_result_json(result, agent),
        "copilot" | "qoder" => parse_plain_text(result, agent),
        _ => return None,
    })
}

fn contains_output_token_limit(text: &str) -> bool {
    text.to_ascii_lowercase()
        .contains("exceeded the output token limit")
}

fn extract_jetski_no_output(stderr: &str) -> Option<String> {
    stderr
        .lines()
        .find(|line| line.contains("jetski: no output produced"))
        .map(|line| line.trim().chars().take(300).collect())
}

/// agy stream-json：取最后一行 `{"event":"result"}`。退出码是 0 也可能没答
/// （工具全被拒时 `status: SUCCESS` 但 `response` 为空），所以只看这一行。
fn parse_agy(result: &ProcessResult) -> Result<ChannelOutput, String> {
    let event = match json_lines(&result.stdout)
        .filter(|value| value.get("event").and_then(Value::as_str) == Some("result"))
        .last()
    {
        Some(event) => event,
        None => {
            if contains_output_token_limit(&result.stderr)
                || contains_output_token_limit(&result.stdout)
            {
                return Err("回答超过输出上限".into());
            }
            if let Some(jetski) = extract_jetski_no_output(&result.stderr) {
                return Err(format!("agy 没给出回答：没有 result 事件；{jetski}"));
            }
            return Err(format!(
                "agy 没给出回答：没有 result 事件；{}",
                nonempty_error(result, "无输出")
            ));
        }
    };
    let body = event.get("result").unwrap_or(&Value::Null);
    let status = body.get("status").and_then(Value::as_str).unwrap_or("");
    let response = body
        .get("response")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if status != "SUCCESS" || response.is_empty() {
        let error = body
            .get("error")
            .or_else(|| event.get("error"))
            .map(|value| match value {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            })
            .filter(|text| !text.trim().is_empty() && text != "null");
        if error.as_deref().is_some_and(contains_output_token_limit)
            || contains_output_token_limit(&result.stderr)
            || contains_output_token_limit(&result.stdout)
        {
            return Err("回答超过输出上限".into());
        }
        let status = if status.is_empty() { "（空）" } else { status };
        let jetski = extract_jetski_no_output(&result.stderr);
        return Err(match (error, jetski) {
            (Some(error), Some(jetski)) => {
                format!("agy 没给出回答：status {status}，error {error}；{jetski}")
            }
            (Some(error), None) => format!("agy 没给出回答：status {status}，error {error}"),
            (None, Some(jetski)) => {
                format!("agy 没给出回答：status {status}，回答为空：{jetski}")
            }
            (None, None) => format!("agy 没给出回答：status {status}，回答为空"),
        });
    }
    let usage = body.get("usage").unwrap_or(&Value::Null);
    let field = |name: &str| usage.get(name).and_then(Value::as_u64).unwrap_or(0);
    Ok(ChannelOutput {
        text: response,
        usage: Usage {
            input: field("input_tokens"),
            output: field("output_tokens"),
            cache_read: field("cache_read_tokens"),
        },
        ..ChannelOutput::default()
    })
}

/// cursor / codebuddy 的 `--output-format json`：单个结果对象，`result` 为正文，
/// `is_error` 为真即失败（与 claude -p 同形）。
fn parse_result_json(result: &ProcessResult, agent: &str) -> Result<ChannelOutput, String> {
    let value: Value = serde_json::from_str(result.stdout.trim())
        .map_err(|error| format!("{agent} 输出不是 JSON：{error}；{}", nonempty_error(result, "无输出")))?;
    let text = value
        .get("result")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if value.get("is_error").and_then(Value::as_bool) == Some(true) {
        return Err(if text.is_empty() {
            format!("{agent} 返回错误")
        } else {
            text
        });
    }
    if text.is_empty() {
        return Err(format!("{agent} 未返回正文"));
    }
    let usage = value.get("usage").unwrap_or(&Value::Null);
    let field = |name: &str| usage.get(name).and_then(Value::as_u64).unwrap_or(0);
    Ok(ChannelOutput {
        text,
        usage: Usage {
            input: field("input_tokens"),
            output: field("output_tokens"),
            cache_read: field("cache_read_input_tokens"),
        },
        session_id: value
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        ..ChannelOutput::default()
    })
}

/// copilot / qoder 的 `-p` 默认纯文本输出：stdout 即正文。
fn parse_plain_text(result: &ProcessResult, agent: &str) -> Result<ChannelOutput, String> {
    if result.code.is_some_and(|code| code != 0) {
        return Err(nonempty_error(result, &format!("{agent} 退出码非零")));
    }
    let text = result.stdout.trim().to_string();
    if text.is_empty() {
        return Err(nonempty_error(result, &format!("{agent} 未返回正文")));
    }
    Ok(ChannelOutput {
        text,
        ..ChannelOutput::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(stdout: &str, code: Option<i32>) -> ProcessResult {
        ProcessResult {
            code,
            stdout: stdout.to_string(),
            stderr: String::new(),
            timed_out: false,
            truncated: false,
            stdout_truncated: false,
            timeout: Duration::from_secs(5),
            idle_timed_out: false,
            idle_timeout: None,
        }
    }

    #[test]
    fn task61_agy_invocation_is_stdin_ndjson_with_effort_and_no_skip_permissions() {
        let built = spec("agy", "gemini-next", "高", true, "第一行\n第二行", Duration::from_secs(300))
            .unwrap();
        assert_eq!(built.binary, "agy");
        assert_eq!(
            built.args,
            vec![
                "--model", "gemini-next", "--input-format", "stream-json",
                "--output-format", "stream-json", "--print-timeout", "300s", "--effort", "high",
            ]
        );
        assert!(!built.args.iter().any(|arg| arg.contains("dangerously")));
        let input = built.input.unwrap();
        assert!(input.ends_with('\n'));
        assert_eq!(input.lines().count(), 1, "整段材料只占 stdin 一行");
        let line: Value = serde_json::from_str(input.trim()).unwrap();
        assert_eq!(line["event"], "user");
        assert_eq!(line["message"]["role"], "user");
        let content = line["message"]["content"].as_str().unwrap();
        assert!(content.starts_with("第一行\n第二行"));
        assert!(content.ends_with(AGY_NO_TOOLS));
        // 已带这一句的提示词不重复追加
        let again = spec("agy", "m", "关", true, content, Duration::from_secs(1)).unwrap();
        let again: Value = serde_json::from_str(again.input.unwrap().trim()).unwrap();
        assert_eq!(again["message"]["content"].as_str().unwrap().matches(AGY_NO_TOOLS).count(), 1);
    }

    #[test]
    fn task61_agy_effort_maps_low_mid_high_only() {
        assert_eq!(agy_effort("低"), Some("low"));
        assert_eq!(agy_effort("中"), Some("medium"));
        assert_eq!(agy_effort("高"), Some("high"));
        assert_eq!(agy_effort("关"), None);
        let built = spec("agy", "m", "", true, "p", Duration::from_secs(5)).unwrap();
        assert!(!built.args.iter().any(|arg| arg == "--effort"));
        // 模型 id 自带档位（agy models 列出的全是）时不传 --effort：真机实测
        // `--model gemini-3.8-flash-low --effort medium` 直接报冲突
        for model in ["gemini-3.8-flash-low", "gpt-oss-120b-medium", "claude-opus-5-5-high"] {
            let built = spec("agy", model, "中", true, "p", Duration::from_secs(5)).unwrap();
            assert!(!built.args.iter().any(|arg| arg == "--effort"), "{model}");
        }
    }

    /// fix-61-r1 5：agy 模型 id 自带档位时，实际生效的思考强度由模型决定。
    #[test]
    fn task61_r1_effective_thinking_follows_model_suffix_for_agy() {
        assert_eq!(effective_thinking("agy", "", "gemini-3.8-flash-low", "高"), "由模型决定：low");
        assert_eq!(effective_thinking("agy", "", "gpt-oss-120b-medium", ""), "由模型决定：medium");
        assert_eq!(effective_thinking("agy", "", "claude-opus-5-5-high", "低"), "由模型决定：high");
        assert_eq!(effective_thinking("agy", "", "gemini-next", "中"), "中");
        // 其它 agent 不改：codex 模型名里出现 -high 也照样用设的值
        assert_eq!(effective_thinking("codex", "", "gpt-5-high", "中"), "中");
        // 已是生效值的再算一次不变（备选改派、重复经过时幂等）
        assert_eq!(effective_thinking("agy", "", "gemini-3.8-flash-low", "由模型决定：low"), "由模型决定：low");
    }

    #[test]
    fn pi_thinking_uses_nearest_supported_level_and_is_idempotent() {
        let catalog = json!({
            "supports_off": {"models": [{"id":"off", "reasoning":true, "thinkingLevelMap":{"off":true,"minimal":null,"low":null,"medium":null,"high":true,"xhigh":null,"max":true} }]},
            "sparse": {"models": [{"id":"sparse", "reasoning":true, "thinkingLevelMap":{"off":null,"minimal":null,"low":true,"medium":null,"high":true,"xhigh":null,"max":true} }]},
            "supports_medium": {"models": [{"id":"medium", "reasoning":true, "thinkingLevelMap":{"off":null,"minimal":null,"low":true,"medium":true,"high":true,"xhigh":null,"max":true} }]},
            "only_off": {"models": [{"id":"only_off", "reasoning":false}]}
        });
        let cases = [
            ("supports_off", "off", "关", "关", "off", false),
            ("supports_off", "off", "中", "已调整为：high", "high", true),
            ("sparse", "sparse", "关", "已调整为：low", "low", true),
            ("sparse", "sparse", "中", "已调整为：low", "low", true),
            ("sparse", "sparse", "高", "高", "high", false),
            ("sparse", "sparse", "低", "低", "low", false),
            ("supports_medium", "medium", "中", "中", "medium", false),
        ];
        for (provider, model, requested, expected_record, expected_arg, adjusted) in cases {
            let record = effective_pi_thinking(&catalog, provider, model, requested);
            assert_eq!(record, expected_record);
            assert_eq!(super::super::pi_thinking(&record), expected_arg);
            assert_eq!(resolve_pi_thinking(&catalog, provider, model, requested).1, adjusted);
        }
        let missing = effective_pi_thinking(&Value::Null, "missing", "m", "中");
        assert_eq!(missing, "中");
        assert_eq!(super::super::pi_thinking(&missing), "medium");
        assert!(!resolve_pi_thinking(&Value::Null, "missing", "m", "中").1);
        let prefixed = "已调整为：low";
        assert_eq!(effective_pi_thinking(&catalog, "sparse", "sparse", prefixed), prefixed);
        assert_eq!(effective_pi_thinking(&catalog, "supports_off", "off", "low"), "已调整为：high");
        assert_eq!(effective_pi_thinking(&catalog, "supports_off", "off", "high"), "high");
        assert_eq!(super::super::pi_thinking("off"), "off");
        assert_eq!(super::super::pi_thinking("high"), "high");
        let adjusted = effective_pi_thinking(&catalog, "supports_off", "off", "已调整为：medium");
        assert_eq!(adjusted, "已调整为：high");
        let changed_model = effective_pi_thinking(&catalog, "only_off", "only_off", &adjusted);
        assert_eq!(changed_model, "已调整为：off");
        assert_eq!(effective_pi_thinking(&catalog, "only_off", "only_off", &changed_model), changed_model);
    }

    #[test]
    fn task61_new_agents_refuse_non_blind_seats() {
        for agent in ["agy", "cursor", "codebuddy", "copilot", "qoder"] {
            let error = spec(agent, "m", "中", false, "p", Duration::from_secs(5)).unwrap_err();
            assert_eq!(error, crate::agent_catalog::blind_only_reason(agent).unwrap());
        }
        let error = spec("agy", "gemini", "中", false, "p", Duration::from_secs(5)).unwrap_err();
        assert!(error.contains("纯文本盲审"), "{error}");
    }

    #[test]
    fn task61_untested_agents_fail_with_template_in_message() {
        let cases = [
            ("cursor", "auto", "cursor-agent -p --output-format json --model auto"),
            (
                "codebuddy",
                "gpt-5",
                "codebuddy -p --output-format json --model gpt-5 --permission-mode plan",
            ),
            (
                "copilot",
                "gpt-5",
                "copilot -p … --model gpt-5 --deny-tool write --deny-tool shell",
            ),
            ("qoder", "performance", "qodercli -p --model performance"),
        ];
        for (agent, model, template) in cases {
            assert_eq!(untested_template(agent, model).join(" "), template);
            let error = spec(agent, model, "中", true, "多行\n材料", Duration::from_secs(5)).unwrap_err();
            assert!(error.starts_with("未实测通道"), "{error}");
            assert!(error.contains(template), "{error}");
        }
    }

    #[test]
    fn task61_parse_agy_result_success_and_usage() {
        let stdout = concat!(
            "{\"event\":\"init\",\"model\":\"gemini-3.8-flash-low\"}\n",
            "{\"event\":\"step_update\",\"step\":{}}\n",
            "{\"event\":\"result\",\"result\":{\"status\":\"SUCCESS\",\"response\":\" 4 \",",
            "\"usage\":{\"input_tokens\":13000,\"output_tokens\":5,\"cache_read_tokens\":12000}}}\n"
        );
        let output = parse_agy(&process(stdout, Some(0))).unwrap();
        assert_eq!(output.text, "4");
        assert_eq!(output.usage.input, 13000);
        assert_eq!(output.usage.output, 5);
        assert_eq!(output.usage.cache_read, 12000);
    }

    #[test]
    fn task61_parse_agy_empty_response_or_failure_is_error() {
        // 工具全被拒：退出码 0、SUCCESS，但 response 为空
        let empty = "{\"event\":\"result\",\"result\":{\"status\":\"SUCCESS\",\"response\":\"\"}}\n";
        let error = parse_agy(&process(empty, Some(0))).unwrap_err();
        assert!(error.contains("agy 没给出回答") && error.contains("SUCCESS"), "{error}");
        let failed = "{\"event\":\"result\",\"result\":{\"status\":\"ERROR\",\"response\":\"x\",\"error\":\"quota\"}}\n";
        let error = parse_agy(&process(failed, Some(0))).unwrap_err();
        assert!(error.contains("ERROR") && error.contains("quota"), "{error}");
        let error = parse_agy(&process("Fetching...\n", Some(1))).unwrap_err();
        assert!(error.contains("没有 result 事件"), "{error}");
    }

    #[test]
    fn task61_parse_untested_outputs_with_fake_results() {
        let json = "{\"type\":\"result\",\"is_error\":false,\"result\":\"答案\",\"session_id\":\"s1\",\"usage\":{\"input_tokens\":7,\"output_tokens\":2}}";
        for agent in ["cursor", "codebuddy"] {
            let output = parse(agent, &process(json, Some(0))).unwrap().unwrap();
            assert_eq!(output.text, "答案");
            assert_eq!(output.session_id, "s1");
            assert_eq!(output.usage.input, 7);
        }
        let error = parse("cursor", &process("{\"is_error\":true,\"result\":\"boom\"}", Some(1)))
            .unwrap()
            .unwrap_err();
        assert_eq!(error, "boom");
        assert!(parse("codebuddy", &process("not json", Some(0))).unwrap().is_err());
        for agent in ["copilot", "qoder"] {
            let output = parse(agent, &process("  纯文本回答\n", Some(0))).unwrap().unwrap();
            assert_eq!(output.text, "纯文本回答");
            assert!(parse(agent, &process("", Some(0))).unwrap().is_err());
            assert!(parse(agent, &process("x", Some(2))).unwrap().is_err());
        }
        assert!(parse("claude", &process("", Some(0))).is_none());
    }

    #[test]
    fn task74_multi_step_heuristic_and_thinking_floor() {
        assert!(is_multi_step_task("1. 修改入口\n2. 补充测试"));
        assert!(is_multi_step_task("请修改 src/a.rs 与 src\\b.rs"));
        assert!(!is_multi_step_task("请反复核对 src/a.rs 和 SRC\\A.RS"));
        assert!(!is_multi_step_task("检查这段代码块：\n```md\n1. 示例步骤一\n2. 示例步骤二\n```"));
        assert!(!is_multi_step_task("只检查这一句话"));

        assert_eq!(apply_execution_thinking_floor("执行", "1. 第一步\n2. 第二步", "低"), "已调整为：中");
        assert_eq!(apply_execution_thinking_floor("执行", "1. 第一步\n2. 第二步", "高"), "高");
        assert_eq!(apply_execution_thinking_floor("验收", "1. 第一步\n2. 第二步", "低"), "低");
        assert_eq!(apply_execution_thinking_floor("执行", "单步检查", "关"), "关");

        let catalog = serde_json::json!({
            "demo": {
                "models": [
                    {
                        "id": "m-no-med",
                        "reasoning": true,
                        "thinkingLevelMap": {
                            "off": null,
                            "minimal": null,
                            "low": "low",
                            "medium": null,
                            "high": "high"
                        }
                    }
                ]
            }
        });
        assert_eq!(resolve_pi_thinking(&catalog, "demo", "m-no-med", "已调整为：中"), ("high".into(), true));
        assert_eq!(resolve_pi_thinking(&catalog, "demo", "m-no-med", "中"), ("low".into(), true));
        assert_eq!(effective_pi_thinking(&catalog, "demo", "m-no-med", "已调整为：中"), "已调整为：high");
        assert_eq!(effective_pi_thinking(&catalog, "demo", "m-no-med", "中"), "已调整为：low");
        assert_eq!(effective_pi_thinking(&catalog, "unknown", "m", "已调整为：中"), "已调整为：中");
    }

    #[test]
    fn task74_agy_settings_modes_and_parse_errors() {
        let ro: Value = serde_json::from_str(&agy_readonly_settings_json()).unwrap();
        // 规则必须带目标；只写类别名的规则在真机上不生效。
        assert_eq!(
            ro["permissions"]["deny"],
            json!(["write_file(*)", "command(*)", "mcp(*)"])
        );

        assert_eq!(
            agy_write_allow_rule(Path::new(r"\\?\D:\work\demo\")),
            "write_file(D:/work/demo/)"
        );
        let sw: Value = serde_json::from_str(&agy_single_writer_settings_json(Path::new(r"D:\work\demo"))).unwrap();
        let allow = sw["permissions"]["allow"].as_array().unwrap();
        assert_eq!(allow, &vec![json!("write_file(D:/work/demo/)")]);
        assert!(!allow[0].as_str().unwrap().contains('*'));
        assert_eq!(sw["permissions"]["deny"], json!(["command(*)", "mcp(*)"]));

        let settings = Path::new("D:/run/agy-settings/n1.json");
        let built_read = spec_with_agy(
            "agy",
            "gemini-4-carbon-low",
            "中",
            true,
            "只读",
            "请检查代码",
            Duration::from_secs(60),
            AgyToolMode::ReadOnlyFiles,
            Some(settings),
        )
        .unwrap();
        assert!(built_read.args.windows(2).any(|w| w == ["--settings", "D:/run/agy-settings/n1.json"]));
        assert!(!built_read.args.iter().any(|a| a.contains("dangerously")));
        assert!(!built_read.input.as_deref().unwrap().contains(AGY_NO_TOOLS));

        let built_sw = spec_with_agy(
            "agy",
            "gemini-4-carbon-low",
            "中",
            false,
            "单写者",
            "修改 a.txt",
            Duration::from_secs(60),
            AgyToolMode::SingleWriter,
            Some(settings),
        )
        .unwrap();
        let sw_input = built_sw.input.unwrap();
        assert!(sw_input.contains(AGY_SINGLE_WRITER_NOTE));
        assert!(!sw_input.contains(AGY_NO_TOOLS));

        let worktree_err = spec_with_agy(
            "agy",
            "gemini-4-carbon-low",
            "中",
            false,
            "worktree",
            "修改 a.txt",
            Duration::from_secs(60),
            AgyToolMode::SingleWriter,
            Some(settings),
        )
        .unwrap_err();
        assert_eq!(worktree_err, crate::agent_catalog::AGY_NO_WORKTREE_REASON);

        let long_reason = "x".repeat(350);
        let mut proc = process(
            "{\"event\":\"result\",\"result\":{\"status\":\"SUCCESS\",\"response\":\"\"}}\n",
            Some(0),
        );
        proc.stderr = format!("other log\njetski: no output produced — {long_reason}\ntail\n");
        let err = parse_agy(&proc).unwrap_err();
        assert!(err.contains("jetski: no output produced"), "{err}");
        let suffix = err.split("回答为空：").nth(1).unwrap();
        assert_eq!(suffix.chars().count(), 300, "{suffix}");

        let limit_proc = process(
            "{\"event\":\"result\",\"result\":{\"status\":\"ERROR\",\"response\":\"\",\"error\":\"Model response exceeded the output token limit.\"}}\n",
            Some(0),
        );
        assert_eq!(parse_agy(&limit_proc).unwrap_err(), "回答超过输出上限");

        let mut limit_no_result = process("", Some(1));
        limit_no_result.stderr = "Error: exceeded the output token limit while generating".into();
        assert_eq!(parse_agy(&limit_no_result).unwrap_err(), "回答超过输出上限");
    }
}
