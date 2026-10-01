//! Per-tool user-configurable launch overrides.
//!
//! Lives at `~/.aicross/tools.json`. Each entry overrides one or more
//! pieces of the built-in defaults for spawning a CLI tool. Empty fields
//! fall through to the built-in behavior — the user only specifies what
//! they want different.
//!
//! Schema:
//! ```json
//! {
//!   "claude": {
//!     "command": "",
//!     "extra_args": ["--dangerously-skip-permissions"],
//!     "default_cwd": "/home/user/work",
//!     "history_path": ""
//!   },
//!   "codex": {
//!     "command": "docker exec mybox codex",
//!     "extra_args": [],
//!     "default_cwd": "",
//!     "history_path": "/var/log/codex-sessions"
//!   }
//! }
//! ```
//!
//! - `command`: full launch executable. Whitespace-split at spawn time —
//!   first token is the binary, the rest are PREPENDED to args. So
//!   `"docker exec mybox codex"` becomes argv `["docker", "exec", "mybox", "codex"]`.
//!   If empty, the built-in default (e.g. `"claude"` for the claude tool)
//!   is used. NOT shell-parsed — paths with spaces are not supported in
//!   v1; document the limitation.
//! - `extra_args`: appended AFTER the built-in args (so `--mcp-config`
//!   etc still come first). String list, NOT split.
//! - `default_cwd` 与 `history_path` 是旧界面的遗留字段：读取时原样保留
//!   （格式兼容），当前代码不使用。
//!
//! All four fields are independent — set just `extra_args` to add a flag
//! without overriding the binary, set just `command` for a wrapper like
//! `docker exec`, etc.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolConfigEntry {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub extra_args: Vec<String>,
    #[serde(default)]
    pub default_cwd: String,
    #[serde(default)]
    pub history_path: String,
}

pub type ToolConfig = HashMap<String, ToolConfigEntry>;

fn config_path() -> Option<PathBuf> {
    Some(crate::paths::aicross_root().ok()?.join("tools.json"))
}

pub fn load() -> ToolConfig {
    let Some(p) = config_path() else {
        return HashMap::new();
    };
    let Ok(s) = std::fs::read_to_string(&p) else {
        return HashMap::new();
    };
    serde_json::from_str::<ToolConfig>(&s).unwrap_or_default()
}

/// 取单个工具的配置；不存在时回默认。资源探测解析手动 agent 的二进制用它。
pub fn get(tool: &str) -> ToolConfigEntry {
    load().remove(tool).unwrap_or_default()
}

/// Whitespace-split the user's `command` field into (binary, prefix_args).
/// Returns (None, vec![]) for an empty string — caller falls through to
/// built-in defaults in that case.
pub fn parse_command(cmd: &str) -> (Option<String>, Vec<String>) {
    let mut parts = cmd.split_whitespace();
    let Some(bin) = parts.next() else {
        return (None, vec![]);
    };
    let rest: Vec<String> = parts.map(|s| s.to_string()).collect();
    (Some(bin.to_string()), rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple() {
        assert_eq!(parse_command(""), (None, vec![]));
        assert_eq!(parse_command("claude"), (Some("claude".into()), vec![]));
        assert_eq!(
            parse_command("/opt/coffee/bin/claude"),
            (Some("/opt/coffee/bin/claude".into()), vec![])
        );
        assert_eq!(
            parse_command("  docker exec mybox claude  "),
            (
                Some("docker".into()),
                vec!["exec".into(), "mybox".into(), "claude".into()]
            )
        );
    }

}
