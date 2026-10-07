//! 支持清单里新增 agent 的一次性 CLI 通道（TASK-61），与 `build_invocation_spec`
//! 里的 claude / codex / kimi / pi 并列。全部只接盲位置（只读、看不到项目），
//! 在派发用的空目录里跑；prompt 只走 stdin，绝不走 argv 多行。
//!
//! - Antigravity（`agy`）：真机实测通道（1.3.1）。材料作为 stdin 的一行 NDJSON，
//!   解析最后一行 `{"event":"result"}`。**绝不加 `--dangerously-skip-permissions`**：
//!   用户全局 settings 的 allow 规则会在无头下被自动放行。
//! - Cursor / CodeBuddy / Copilot / Qoder：命令模板只照官方文档写，本机没装、
//!   没跑通；prompt 经 stdin 或文件传入的方式也没核实，所以派发直接失败并
//!   提示「未实测通道」，不猜。真机核实后把对应项改成实际的 stdin 用法。

use super::{nonempty_error, json_lines, ChannelOutput, InvocationSpec, Usage};
use crate::process::ProcessResult;
use serde_json::{json, Value};
use std::time::Duration;

/// agy 无头模式下工具全被拒（读文件也拒），要求它直接用文字回答。
/// 引擎在如实汇报后缀之后拼这一句；通道里再兜一次（冒烟、备选改派到
/// agy 时提示词不经引擎那一处），已有就不重复。
pub(crate) const AGY_NO_TOOLS: &str = "不要使用任何工具，直接用文字回答。";

/// 是不是本模块接管的 agent。
pub(super) fn handles(agent: &str) -> bool {
    matches!(agent, "agy" | "cursor" | "codebuddy" | "copilot" | "qoder")
}

/// 拼命令。非盲位置直接拒（原因取支持清单）；未实测四家返回「未实测通道」。
pub(super) fn spec(
    agent: &str,
    model: &str,
    thinking: &str,
    blind: bool,
    prompt: &str,
    timeout: Duration,
) -> Result<InvocationSpec, String> {
    if !blind {
        let reason = crate::agent_catalog::blind_only_reason(agent)
            .unwrap_or(crate::agent_catalog::UNTESTED_BLIND_ONLY);
        return Err(reason.to_string());
    }
    if model.trim().is_empty() {
        return Err(format!("{agent} 通道缺少模型"));
    }
    match agent {
        "agy" => Ok(agy_spec(model, thinking, prompt, timeout)),
        _ => Err(untested_error(agent, model)),
    }
}

fn agy_spec(model: &str, thinking: &str, prompt: &str, timeout: Duration) -> InvocationSpec {
    let mut prompt = prompt.to_string();
    if !prompt.contains(AGY_NO_TOOLS) {
        prompt.push_str("\n\n");
        prompt.push_str(AGY_NO_TOOLS);
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
pub(crate) fn effective_thinking(agent: &str, model: &str, thinking: &str) -> String {
    if super::normalize_agent(agent.trim()).eq_ignore_ascii_case("agy") {
        if let Some(level) = model_effort_suffix(model) {
            return format!("{MODEL_DECIDED_PREFIX}{level}");
        }
    }
    thinking.to_string()
}

const MODEL_DECIDED_PREFIX: &str = "由模型决定：";

/// 思考档：低 / 中 / 高 → low / medium / high；其余不传，用 agy 默认。
fn agy_effort(thinking: &str) -> Option<&'static str> {
    match thinking {
        "低" => Some("low"),
        "中" => Some("medium"),
        "高" => Some("high"),
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

/// agy stream-json：取最后一行 `{"event":"result"}`。退出码是 0 也可能没答
/// （工具全被拒时 `status: SUCCESS` 但 `response` 为空），所以只看这一行。
fn parse_agy(result: &ProcessResult) -> Result<ChannelOutput, String> {
    let event = json_lines(&result.stdout)
        .filter(|value| value.get("event").and_then(Value::as_str) == Some("result"))
        .last()
        .ok_or_else(|| format!("agy 没给出回答：没有 result 事件；{}", nonempty_error(result, "无输出")))?;
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
        let status = if status.is_empty() { "（空）" } else { status };
        return Err(match error {
            Some(error) => format!("agy 没给出回答：status {status}，error {error}"),
            None => format!("agy 没给出回答：status {status}，回答为空"),
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
            timeout: Duration::from_secs(5),
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
        assert_eq!(effective_thinking("agy", "gemini-3.8-flash-low", "高"), "由模型决定：low");
        assert_eq!(effective_thinking("agy", "gpt-oss-120b-medium", ""), "由模型决定：medium");
        assert_eq!(effective_thinking("agy", "claude-opus-5-5-high", "低"), "由模型决定：high");
        assert_eq!(effective_thinking("agy", "gemini-next", "中"), "中");
        // 其它 agent 不改：codex 模型名里出现 -high 也照样用设的值
        assert_eq!(effective_thinking("codex", "gpt-5-high", "中"), "中");
        // 已是生效值的再算一次不变（备选改派、重复经过时幂等）
        assert_eq!(effective_thinking("agy", "gemini-3.8-flash-low", "由模型决定：low"), "由模型决定：low");
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
}
