//! 按用户口头说的名字在资源库里找模型（TASK-67）。「claude 5.5」「opus5.5」
//! 「pi 里的 glm」这类说法拆成字母段与数字段，与模型编号、实际型号、
//! 提供方、厂商、agent 比对；找的是所有 agent 下的模型，不只看名字相同的
//! 那个 agent。
//!
//! 简化：厂商别名是一张手写的小表（claude ↔ anthropic / opus / sonnet …），
//! 新厂商要补表；版本号从头对上才算（5.5 对上 5-5，对不上 4-5，6 对不上 5.6）。

use serde_json::{json, Value};

use crate::resources::{self, ResourceAgent, ResourceStore};

/// 口语里常带、但不区分模型的词，比对时忽略。
const FILLER: &[&str] = &["code", "cli", "agent", "model", "models", "latest", "the", "use", "via"];

/// 厂商与系列的别名：查询里出现左边的词时，候选里出现右边任一个也算对上。
const ALIASES: &[(&str, &[&str])] = &[
    ("claude", &["anthropic", "opus", "sonnet", "haiku", "fable"]),
    ("anthropic", &["claude", "opus", "sonnet", "haiku", "fable"]),
    ("gpt", &["openai"]),
    ("chatgpt", &["gpt", "openai"]),
    ("openai", &["gpt"]),
    ("gemini", &["google"]),
    ("google", &["gemini"]),
    ("kimi", &["moonshot"]),
    ("moonshot", &["kimi"]),
    ("glm", &["zhipu", "zai"]),
    ("zhipu", &["glm", "zai"]),
    ("zai", &["glm", "zhipu"]),
];

/// 按派发看能不能用：可用、已验证、未验证、冒烟超时可以派；其余（未安装、
/// 未发现、未登录、验证失败、额度用尽、未探测）派不出去。
const USABLE_STATES: &[&str] = &[
    resources::RESOURCE_STATE_AVAILABLE,
    resources::RESOURCE_STATE_VERIFIED,
    resources::RESOURCE_STATE_UNVERIFIED,
    resources::RESOURCE_STATE_TIMEOUT,
];

/// 拆成小写字母段与数字段，其他字符（含中文、标点）都当分隔。
fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut digit = false;
    for ch in text.chars().flat_map(char::to_lowercase) {
        let kind = if ch.is_ascii_digit() {
            Some(true)
        } else if ch.is_ascii_alphabetic() {
            Some(false)
        } else {
            None
        };
        match kind {
            Some(is_digit) if current.is_empty() || is_digit == digit => {
                digit = is_digit;
                current.push(ch);
            }
            Some(is_digit) => {
                out.push(std::mem::take(&mut current));
                digit = is_digit;
                current.push(ch);
            }
            None => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn is_digits(token: &str) -> bool {
    token.chars().all(|ch| ch.is_ascii_digit())
}

/// 查询里的字母段（去掉口语虚词）与数字段。
struct Query {
    words: Vec<String>,
    digits: Vec<String>,
}

fn parse_query(text: &str) -> Query {
    let (digits, words): (Vec<String>, Vec<String>) = tokens(text).into_iter().partition(|token| is_digits(token));
    let words = words.into_iter().filter(|word| !FILLER.contains(&word.as_str())).collect();
    Query { words, digits }
}

fn word_matches(word: &str, candidate: &[String]) -> bool {
    if candidate.iter().any(|token| token == word) {
        return true;
    }
    ALIASES
        .iter()
        .find(|(name, _)| *name == word)
        .is_some_and(|(_, alts)| alts.iter().any(|alt| candidate.iter().any(|token| token == alt)))
}

/// 查询的数字段是候选数字段的开头：说 5 对上 5-5、5.1，对不上 4-5、5.6 之外的 6。
fn digits_match(wanted: &[String], candidate: &[String]) -> bool {
    candidate.starts_with(wanted)
}

fn digit_tokens(text: &str) -> Vec<String> {
    tokens(text).into_iter().filter(|token| is_digits(token)).collect()
}

/// 模型编号里查询没覆盖到的段数，越少越贴近用户说的名字。
fn extra_parts(model_id: &str, query: &Query) -> usize {
    tokens(model_id)
        .iter()
        .filter(|token| !query.words.contains(token) && !query.digits.contains(token))
        .count()
}

struct Hit {
    usable: bool,
    extra: usize,
    value: Value,
}

/// 找模型。`check_digits` 为 false 时只比字母段，用来在没对上时列出同一家的模型。
fn search(store: &ResourceStore, query: &Query, check_digits: bool, now_millis: i64) -> Vec<Hit> {
    let ever_detected = store.detected_at.is_some();
    let mut hits = Vec::new();
    for agent in &store.agents {
        for model in &agent.models {
            let mut words = tokens(&model.id);
            words.extend(tokens(&model.resolved_id));
            words.extend(tokens(&model.provider));
            words.extend(tokens(&model.vendor));
            words.extend(tokens(&agent.id));
            if !query.words.iter().all(|word| word_matches(word, &words)) {
                continue;
            }
            if check_digits
                && !digits_match(&query.digits, &digit_tokens(&model.id))
                && !digits_match(&query.digits, &digit_tokens(&model.resolved_id))
            {
                continue;
            }
            hits.push(hit(agent, model, ever_detected, query, now_millis));
        }
    }
    hits.sort_by(|a, b| b.usable.cmp(&a.usable).then(a.extra.cmp(&b.extra)));
    hits
}

fn hit(agent: &ResourceAgent, model: &resources::ResourceModel, ever_detected: bool, query: &Query, now_millis: i64) -> Hit {
    let (state, note) = if ever_detected {
        resources::model_state(model, agent.detected, now_millis)
    } else {
        (resources::RESOURCE_STATE_NEVER_DETECTED, String::new())
    };
    let usable = USABLE_STATES.contains(&state);
    Hit {
        usable,
        extra: extra_parts(&model.id, query),
        value: json!({
            "agent": agent.id,
            "provider": model.provider,
            "model": model.id,
            "resolved_id": model.resolved_id,
            "vendor": model.vendor,
            "state": state,
            "note": note,
            "usable": usable,
            "target": format!("{}:{}:{}", agent.id, model.provider, model.id),
        }),
    }
}

/// 输出的候选上限；再多说明说法太宽，让用户说得更具体。
const MATCH_LIMIT: usize = 20;

/// `resources find` 的结果：JSON 与给人看的一句说明。对上的按能用在前、
/// 越贴近说法越前排；一个都没对上时 `matches` 为空，`related` 列出只按
/// 名字（不看版本号）对上的模型，供用户改口。
pub(crate) fn find(store: &ResourceStore, text: &str, now_millis: i64) -> Result<(Value, String), String> {
    let query = parse_query(text);
    if query.words.is_empty() && query.digits.is_empty() {
        return Err(format!("「{text}」里没有可比对的字母或数字"));
    }
    let hits = search(store, &query, true, now_millis);
    let usable = hits.iter().filter(|hit| hit.usable).count();
    let total = hits.len();
    let matches: Vec<Value> = hits.into_iter().take(MATCH_LIMIT).map(|hit| hit.value).collect();
    if total > 0 {
        let mut message = format!("对上 {total} 个模型，其中 {usable} 个可以派发");
        if total > MATCH_LIMIT {
            message.push_str(&format!("；只列前 {MATCH_LIMIT} 个，说得具体些可缩小范围"));
        }
        return Ok((json!({ "query": text, "matches": matches, "related": [] }), message));
    }
    let related: Vec<Value> = if query.words.is_empty() {
        Vec::new()
    } else {
        search(store, &query, false, now_millis)
            .into_iter()
            .take(MATCH_LIMIT)
            .map(|hit| hit.value)
            .collect()
    };
    let message = if related.is_empty() {
        format!("资源库里没有对上「{text}」的模型；装好后在 aicross 里重新探测")
    } else {
        format!("资源库里没有对上「{text}」的模型；名字相近的有 {} 个，见 related", related.len())
    };
    Ok((json!({ "query": text, "matches": [], "related": related }), message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::ResourceModel;

    fn model(agent: &str, provider: &str, id: &str, vendor: &str) -> ResourceModel {
        ResourceModel {
            id: id.into(),
            agent: agent.into(),
            provider: provider.into(),
            vendor: vendor.into(),
            ..ResourceModel::default()
        }
    }

    fn agent(id: &str, detected: bool, models: Vec<ResourceModel>) -> ResourceAgent {
        ResourceAgent { id: id.into(), detected, models, ..ResourceAgent::default() }
    }

    fn store() -> ResourceStore {
        let mut claude_opus = model("claude", "", "opus", "Anthropic");
        claude_opus.resolved_id = "claude-opus-5".into();
        ResourceStore {
            agents: vec![
                agent("claude", false, vec![claude_opus]),
                agent(
                    "pi",
                    true,
                    vec![
                        model("pi", "opencode-go", "claude-opus-5-5", "Anthropic"),
                        model("pi", "opencode-go", "claude-sonnet-4-5", "Anthropic"),
                        model("pi", "zai", "glm-5.2", "智谱"),
                    ],
                ),
                agent("codex", true, vec![model("codex", "", "gpt-5.5", "OpenAI")]),
            ],
            detected_at: Some("2026-10-28T00:00:00Z".into()),
            ..ResourceStore::default()
        }
    }

    fn targets(value: &Value, key: &str) -> Vec<String> {
        value[key].as_array().unwrap().iter().map(|item| item["target"].as_str().unwrap().to_string()).collect()
    }

    #[test]
    fn task67_tokens_split_letters_and_digits() {
        assert_eq!(tokens("Claude 5.5"), ["claude", "5", "5"]);
        assert_eq!(tokens("opus5.5"), ["opus", "5", "5"]);
        assert_eq!(tokens("pi 里的 glm-5.2"), ["pi", "glm", "5", "2"]);
    }

    #[test]
    fn task67_spoken_name_finds_model_under_another_agent() {
        // 用户说 claude 5.5，模型挂在 pi 下；Claude Code 未安装不影响找到它。
        let (value, message) = find(&store(), "claude 5.5", 0).unwrap();
        assert_eq!(targets(&value, "matches"), ["pi:opencode-go:claude-opus-5-5"]);
        assert_eq!(value["matches"][0]["usable"], true);
        assert!(message.contains("对上 1 个"), "{message}");
    }

    #[test]
    fn task67_vendor_alias_and_agent_name() {
        let (value, _) = find(&store(), "opus5.5", 0).unwrap();
        assert_eq!(targets(&value, "matches"), ["pi:opencode-go:claude-opus-5-5"]);
        let (value, _) = find(&store(), "pi 里的 glm", 0).unwrap();
        assert_eq!(targets(&value, "matches"), ["pi:zai:glm-5.2"]);
        let (value, _) = find(&store(), "glm 2", 0).unwrap();
        assert!(targets(&value, "matches").is_empty(), "版本号从头比，2 对不上 5.2");
        let (value, _) = find(&store(), "chatgpt 5.5", 0).unwrap();
        assert_eq!(targets(&value, "matches"), ["codex::gpt-5.5"]);
    }

    #[test]
    fn task67_usable_first_then_closest() {
        // 只说 claude：Claude Code 未安装的 opus 排在 pi 的两个可用模型之后。
        let (value, _) = find(&store(), "claude code", 0).unwrap();
        let found = targets(&value, "matches");
        assert_eq!(found.len(), 3);
        assert_eq!(found[2], "claude::opus");
        assert_eq!(value["matches"][2]["usable"], false);
        assert_eq!(value["matches"][2]["state"], resources::RESOURCE_STATE_MISSING);
    }

    #[test]
    fn task67_version_mismatch_lists_related() {
        let (value, message) = find(&store(), "claude 9.9", 0).unwrap();
        assert!(value["matches"].as_array().unwrap().is_empty());
        assert_eq!(targets(&value, "related").len(), 3);
        assert!(message.contains("名字相近的有 3 个"), "{message}");
        assert!(find(&store(), "某某模型", 0).is_err());
    }
}
