use crate::models::SessionEntry;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;

struct Candidate {
    path: PathBuf,
    modified: SystemTime,
}

pub fn list_sessions() -> Vec<SessionEntry> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    entries.extend(parse_source(
        "claude",
        &[home.join(".claude").join("projects")],
        |path| path.extension().is_some_and(|value| value == "jsonl"),
    ));
    entries.extend(parse_source(
        "codex",
        &[home.join(".codex").join("sessions")],
        |path| path.extension().is_some_and(|value| value == "jsonl"),
    ));
    entries.extend(parse_source(
        "kimi",
        &[home.join(".kimi-code").join("sessions")],
        |path| path.file_name().is_some_and(|value| value == "wire.jsonl"),
    ));
    entries.extend(parse_source(
        "pi",
        &[
            home.join(".pi").join("sessions"),
            home.join(".pi").join("agent").join("sessions"),
        ],
        |path| path.extension().is_some_and(|value| value == "jsonl"),
    ));
    finalize_sessions(entries)
}

fn finalize_sessions(mut entries: Vec<SessionEntry>) -> Vec<SessionEntry> {
    entries.sort_by(|left, right| {
        timestamp_key(&right.timestamp).cmp(&timestamp_key(&left.timestamp))
    });
    let mut seen = HashSet::new();
    entries.retain(|entry| seen.insert((entry.tool.clone(), entry.id.clone())));
    entries.truncate(300);
    entries
}

fn parse_source<F>(tool: &str, roots: &[PathBuf], matches: F) -> Vec<SessionEntry>
where
    F: Fn(&Path) -> bool,
{
    let mut candidates = Vec::new();
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() || !matches(entry.path()) {
                continue;
            }
            let modified = entry
                .metadata()
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            candidates.push(Candidate {
                path: entry.into_path(),
                modified,
            });
        }
    }
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.modified));
    candidates.truncate(250);
    candidates
        .iter()
        .filter_map(|candidate| parse_session(tool, candidate))
        .collect()
}

fn parse_session(tool: &str, candidate: &Candidate) -> Option<SessionEntry> {
    let file = fs::File::open(&candidate.path).ok()?;
    let reader = BufReader::new(file);
    let mut id = candidate.path.to_string_lossy().to_string();
    let mut timestamp = system_time(candidate.modified);
    let mut model = "—".to_string();
    let mut project_path = "—".to_string();
    let mut first_message = None;
    let mut recognized = false;

    for line in reader.lines().map_while(Result::ok).take(10_000) {
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        match tool {
            "claude" => {
                if let Some(value) = value.get("sessionId").and_then(Value::as_str) {
                    id = value.to_string();
                    recognized = true;
                }
                recognized |= take_string(&mut timestamp, value.get("timestamp"));
                recognized |= take_string(&mut project_path, value.get("cwd"));
                if value.get("type").and_then(Value::as_str) == Some("assistant") {
                    recognized |= take_string(&mut model, value.pointer("/message/model"));
                }
                if first_message.is_none()
                    && value.get("type").and_then(Value::as_str) == Some("user")
                    && value.pointer("/message/role").and_then(Value::as_str) == Some("user")
                {
                    first_message = content_text(value.pointer("/message/content"));
                    recognized |= first_message.is_some();
                }
            }
            "codex" => match value.get("type").and_then(Value::as_str) {
                Some("session_meta") => {
                    recognized |= take_string(&mut id, value.pointer("/payload/id"));
                    recognized |= take_string(&mut timestamp, value.pointer("/payload/timestamp"));
                    recognized |= take_string(&mut project_path, value.pointer("/payload/cwd"));
                }
                Some("turn_context") => {
                    recognized |= take_string(&mut model, value.pointer("/payload/model"));
                    recognized |= take_string(&mut project_path, value.pointer("/payload/cwd"));
                }
                Some("response_item")
                    if first_message.is_none()
                        && value.pointer("/payload/type").and_then(Value::as_str)
                            == Some("message")
                        && value.pointer("/payload/role").and_then(Value::as_str)
                            == Some("user") =>
                {
                    first_message = content_text(value.pointer("/payload/content"));
                    recognized |= first_message.is_some();
                }
                _ => {}
            },
            "kimi" => match value.get("type").and_then(Value::as_str) {
                Some("metadata") => {
                    recognized |= take_time_value(&mut timestamp, value.get("created_at"));
                }
                Some("profile.bind") => {
                    recognized |= take_string(&mut model, value.get("modelAlias"));
                    recognized |= take_string(
                        &mut project_path,
                        value.pointer("/environmentDisclosure/cwd"),
                    );
                }
                Some("turn.prompt") if first_message.is_none() => {
                    first_message = content_text(value.get("input"));
                    recognized |= first_message.is_some();
                    recognized |= take_time_value(&mut timestamp, value.get("time"));
                }
                _ => {}
            },
            "pi" => match value.get("type").and_then(Value::as_str) {
                Some("session") => {
                    recognized |= take_string(&mut id, value.get("id"));
                    recognized |= take_string(&mut timestamp, value.get("timestamp"));
                    recognized |= take_string(&mut project_path, value.get("cwd"));
                }
                Some("model_change") => {
                    let provider = value.get("provider").and_then(Value::as_str).unwrap_or("");
                    let model_id = value.get("modelId").and_then(Value::as_str).unwrap_or("");
                    if !model_id.is_empty() {
                        recognized = true;
                        model = if provider.is_empty() {
                            model_id.to_string()
                        } else {
                            format!("{provider}/{model_id}")
                        };
                    }
                }
                Some("message")
                    if first_message.is_none()
                        && value.pointer("/message/role").and_then(Value::as_str)
                            == Some("user") =>
                {
                    first_message = content_text(value.pointer("/message/content"));
                    recognized |= first_message.is_some();
                }
                _ => {}
            },
            _ => {}
        }
    }

    if !recognized {
        return None;
    }

    Some(SessionEntry {
        id,
        timestamp,
        tool: tool.to_string(),
        model,
        project_path,
        first_message: truncate(first_message.as_deref().unwrap_or("—"), 80),
    })
}

fn content_text(value: Option<&Value>) -> Option<String> {
    let value = value?;
    match value {
        Value::String(text) => nonempty(text),
        Value::Array(items) => {
            let pieces: Vec<String> = items
                .iter()
                .filter_map(|item| {
                    if let Value::String(text) = item {
                        return nonempty(text);
                    }
                    let kind = item.get("type").and_then(Value::as_str).unwrap_or("text");
                    if !matches!(kind, "text" | "input_text") {
                        return None;
                    }
                    item.get("text").and_then(Value::as_str).and_then(nonempty)
                })
                .collect();
            nonempty(&pieces.join(" "))
        }
        Value::Object(_) => value.get("text").and_then(Value::as_str).and_then(nonempty),
        _ => None,
    }
}

fn nonempty(value: &str) -> Option<String> {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    (!compact.is_empty()).then_some(compact)
}

fn take_string(target: &mut String, value: Option<&Value>) -> bool {
    if let Some(value) = value
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        *target = value.to_string();
        return true;
    }
    false
}

fn take_time_value(target: &mut String, value: Option<&Value>) -> bool {
    if let Some(value) = value.and_then(Value::as_i64) {
        let millis = if value < 10_000_000_000 {
            value * 1000
        } else {
            value
        };
        if let Some(time) = DateTime::<Utc>::from_timestamp_millis(millis) {
            *target = time.to_rfc3339();
            return true;
        }
    }
    false
}

fn truncate(value: &str, length: usize) -> String {
    let mut chars = value.chars();
    let result: String = chars.by_ref().take(length).collect();
    if chars.next().is_some() {
        format!("{result}…")
    } else {
        result
    }
}

fn system_time(value: SystemTime) -> String {
    DateTime::<Utc>::from(value).to_rfc3339()
}

fn timestamp_key(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use uuid::Uuid;

    #[test]
    fn pi_parser_keeps_only_requested_metadata() {
        let directory = std::env::temp_dir().join(format!("aicross-session-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).expect("create fixture");
        let path = directory.join("session.jsonl");
        let mut file = fs::File::create(&path).expect("create session");
        writeln!(file, r#"{{"type":"session","id":"fixture","timestamp":"2026-08-30T00:00:00Z","cwd":"D:\\fixture"}}"#).expect("write metadata");
        writeln!(
            file,
            r#"{{"type":"model_change","provider":"provider","modelId":"model"}}"#
        )
        .expect("write model");
        writeln!(file, r#"{{"type":"message","message":{{"role":"user","content":"first message with private tail"}}}}"#).expect("write message");
        let candidate = Candidate {
            path,
            modified: SystemTime::now(),
        };
        let entry = parse_session("pi", &candidate).expect("parse session");
        assert_eq!(entry.id, "fixture");
        assert_eq!(entry.model, "provider/model");
        assert_eq!(entry.project_path, "D:\\fixture");
        assert_eq!(entry.first_message, "first message with private tail");

        fs::remove_dir_all(&directory).expect("remove fixture");
    }

    #[test]
    fn sessions_sort_by_instant_deduplicate_and_skip_garbage() {
        let entry = |id: &str, timestamp: &str| SessionEntry {
            id: id.to_string(),
            timestamp: timestamp.to_string(),
            tool: "claude".to_string(),
            model: "—".to_string(),
            project_path: "—".to_string(),
            first_message: "—".to_string(),
        };
        let entries = finalize_sessions(vec![
            entry("same", "2026-08-30T01:00:00+01:00"),
            entry("newer", "2026-08-30T00:30:00Z"),
            entry("same", "2026-08-29T23:00:00Z"),
        ]);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            ["newer", "same"]
        );

        let directory = std::env::temp_dir().join(format!("aicross-garbage-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).expect("create garbage fixture");
        let path = directory.join("garbage.jsonl");
        fs::write(&path, "not json\n{}\n").expect("write garbage fixture");
        let candidate = Candidate {
            path,
            modified: SystemTime::now(),
        };
        assert!(parse_session("claude", &candidate).is_none());
        fs::remove_dir_all(&directory).expect("remove garbage fixture");
    }
}
