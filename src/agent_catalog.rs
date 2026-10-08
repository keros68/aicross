//! agent 支持清单（TASK-61）：资源库探测、派发通道与推荐共用的一张表。
//!
//! 每项写明命令名、版本参数、模型清单从哪读、官网，以及两条派发约束：
//! `blind_only`（只接盲位置）与 `untested`（命令模板只照官方文档写、没在
//! 真机跑通；一键推荐不选）。表随软件版本发布，新增 agent 在这里加一行，
//! 再在 `resources` 的模型清单读法与 `channels` 的命令模板里各补一处。

use std::path::PathBuf;

/// 模型清单从哪读。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModelSource {
    /// `claude --help` 里的别名举例 + 静态别名表。
    ClaudeHelp,
    /// `~/.codex/models_cache.json`。
    CodexCache,
    /// `~/.kimi-code/config.toml` 的 `[models."别名"]`。
    KimiConfig,
    /// `pi --list-models`（provider 登录另查）。
    PiListModels,
    /// `agy models`：「id<TAB>显示名」行。
    AgyModels,
    /// `cursor-agent models`：「id - 显示名」行。
    CursorModels,
    /// 没有可靠的列模型命令：读随版本发布的 `data/agent-models.json`。
    StaticTable,
    /// Qoder 只能选档位：固定五档。
    QoderTiers,
}

/// 绝对路径兜底的基准目录（Windows 上 PATH 里常常没有）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FallbackBase {
    Home,
    LocalAppData,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SupportedAgent {
    pub id: &'static str,
    pub name: &'static str,
    /// 可执行文件名（不带扩展名）。
    pub command: &'static str,
    /// 命令不在 PATH 里时依次试的绝对路径：(基准目录, 相对路径)。
    pub fallback: Option<(FallbackBase, &'static str)>,
    pub version_arg: &'static str,
    pub models: ModelSource,
    pub homepage: &'static str,
    /// 命令模板只照官方文档写、没在真机跑通。
    pub untested: bool,
    /// 自家模型清单说有就是有、走官方端点：验证状态记 native，不冒烟。
    pub native: bool,
    /// 只接盲（只读、看不到项目）位置时的原因；None 表示不限。
    pub blind_only: Option<&'static str>,
    /// 固定四家：没装时探测报一条错误。清单里其余的 agent 没装是常态，
    /// 只记 `detected: false`，界面折叠，不报错。
    pub core: bool,
}

pub(crate) const UNTESTED_BLIND_ONLY: &str = "未实测通道只接盲（只读、看不到项目）位置";

pub(crate) const SUPPORTED_AGENTS: &[SupportedAgent] = &[
    SupportedAgent {
        id: "claude",
        name: "Claude Code",
        command: "claude",
        fallback: None,
        version_arg: "--version",
        models: ModelSource::ClaudeHelp,
        homepage: "https://claude.com/product/claude-code",
        untested: false,
        native: true,
        blind_only: None,
        core: true,
    },
    SupportedAgent {
        id: "codex",
        name: "Codex",
        command: "codex",
        fallback: None,
        version_arg: "--version",
        models: ModelSource::CodexCache,
        homepage: "https://github.com/openai/codex",
        untested: false,
        native: true,
        blind_only: None,
        core: true,
    },
    SupportedAgent {
        id: "kimi",
        name: "Kimi Code",
        command: "kimi",
        fallback: Some((FallbackBase::Home, ".kimi-code/bin/kimi.exe")),
        version_arg: "--version",
        models: ModelSource::KimiConfig,
        homepage: "https://github.com/MoonshotAI/kimi-cli",
        untested: false,
        native: true,
        blind_only: None,
        core: true,
    },
    SupportedAgent {
        id: "pi",
        name: "pi agent",
        command: "pi",
        fallback: None,
        version_arg: "--version",
        models: ModelSource::PiListModels,
        homepage: "https://github.com/earendil-works/pi",
        untested: false,
        native: false,
        blind_only: None,
        core: true,
    },
    SupportedAgent {
        id: "agy",
        name: "Antigravity",
        command: "agy",
        fallback: Some((FallbackBase::LocalAppData, "agy/bin/agy.exe")),
        version_arg: "--version",
        models: ModelSource::AgyModels,
        homepage: "https://antigravity.google",
        untested: false,
        native: true,
        blind_only: Some("Antigravity 无头模式下工具全被拒，只能做纯文本盲审"),
        core: false,
    },
    SupportedAgent {
        id: "cursor",
        name: "Cursor",
        command: "cursor-agent",
        fallback: None,
        version_arg: "--version",
        models: ModelSource::CursorModels,
        homepage: "https://cursor.com/cli",
        untested: true,
        native: false,
        blind_only: Some(UNTESTED_BLIND_ONLY),
        core: false,
    },
    SupportedAgent {
        id: "codebuddy",
        name: "WorkBuddy / CodeBuddy",
        command: "codebuddy",
        fallback: None,
        version_arg: "--version",
        models: ModelSource::StaticTable,
        homepage: "https://www.workbuddy.ai",
        untested: true,
        native: false,
        blind_only: Some(UNTESTED_BLIND_ONLY),
        core: false,
    },
    SupportedAgent {
        id: "copilot",
        name: "GitHub Copilot CLI",
        command: "copilot",
        fallback: None,
        version_arg: "--version",
        models: ModelSource::StaticTable,
        homepage: "https://github.com/features/copilot/cli",
        untested: true,
        native: false,
        blind_only: Some(UNTESTED_BLIND_ONLY),
        core: false,
    },
    SupportedAgent {
        id: "qoder",
        name: "Qoder",
        command: "qodercli",
        fallback: None,
        version_arg: "--version",
        models: ModelSource::QoderTiers,
        homepage: "https://qoder.com/cli",
        untested: true,
        native: false,
        blind_only: Some(UNTESTED_BLIND_ONLY),
        core: false,
    },
];

/// 按 id 查表，大小写不敏感（手写的 `Codex` 与 `codex` 判成同一家）。
pub(crate) fn find(id: &str) -> Option<&'static SupportedAgent> {
    let id = id.trim();
    SUPPORTED_AGENTS
        .iter()
        .find(|agent| agent.id.eq_ignore_ascii_case(id))
}

/// 该 agent 只接盲位置时的原因。
pub(crate) fn blind_only_reason(id: &str) -> Option<&'static str> {
    find(id).and_then(|agent| agent.blind_only)
}

pub(crate) fn is_untested(id: &str) -> bool {
    find(id).is_some_and(|agent| agent.untested)
}

/// 只能看到 stdin 文字、读不了任何文件的 agent（agy：无头下工具全被拒）。
/// 派给它的材料必须全部内联进提示词；要读项目副本的验收位它做不了。
pub(crate) fn is_text_only(id: &str) -> bool {
    find(id).is_some_and(|agent| agent.id == "agy")
}

/// 位置硬规则（TASK-61）：只接盲位置的 agent 落在可见性不是「盲」的位置时
/// 返回原因。set-seat、set-role、`config show` 与派发共用这一条。
pub(crate) fn seat_violation(agent: &str, visibility: &str) -> Option<&'static str> {
    (visibility != "盲").then(|| blind_only_reason(agent)).flatten()
}

/// 与资源库无关的位置硬规则：[`seat_violation`]，加上只看得到文字的
/// agent 不填验收位（验收要读项目副本）。推荐挑人与补人（`fill_seats`
/// 的角色默认补位）共用这一条。
pub(crate) fn position_violation(agent: &str, role: &str, visibility: &str) -> Option<String> {
    if let Some(reason) = seat_violation(agent, visibility) {
        return Some(reason.to_string());
    }
    if is_text_only(agent) && role == "验收" {
        return Some(format!("{} 读不到项目副本，不填验收位", agent.trim()));
    }
    None
}

/// 用支持清单登记的兜底路径与 PATH 找 agent 的可执行文件：兜底存在就用它，
/// 否则走 PATH。探测、冒烟、派发都经这一处，结果一致（fix-61-r1 4）。
pub(crate) fn resolve_program(id: &str) -> Option<PathBuf> {
    let entry = find(id)?;
    if let Some(fallback) = fallback_path(entry.id).filter(|path| path.is_file()) {
        return Some(fallback);
    }
    crate::channels::resolve_binary(entry.command, None).ok()
}

/// 表里登记的绝对路径兜底（不检查文件是否存在）。
pub(crate) fn fallback_path(id: &str) -> Option<PathBuf> {
    let (base, relative) = find(id)?.fallback?;
    let root = match base {
        FallbackBase::Home => dirs::home_dir(),
        FallbackBase::LocalAppData => dirs::data_local_dir(),
    }?;
    Some(root.join(relative))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task61_catalog_lists_core_four_then_new_agents() {
        let ids: Vec<&str> = SUPPORTED_AGENTS.iter().map(|agent| agent.id).collect();
        assert_eq!(
            ids,
            vec!["claude", "codex", "kimi", "pi", "agy", "cursor", "codebuddy", "copilot", "qoder"]
        );
        for agent in SUPPORTED_AGENTS {
            assert!(agent.homepage.starts_with("https://"), "{}", agent.id);
            assert_eq!(agent.core, matches!(agent.id, "claude" | "codex" | "kimi" | "pi"));
            // 新增五家都只接盲位置；未实测的四家另标 untested
            assert_eq!(agent.blind_only.is_some(), !agent.core, "{}", agent.id);
            assert_eq!(
                agent.untested,
                matches!(agent.id, "cursor" | "codebuddy" | "copilot" | "qoder"),
                "{}",
                agent.id
            );
        }
        assert_eq!(find("AGY").map(|agent| agent.command), Some("agy"));
        assert_eq!(find("cursor").map(|agent| agent.command), Some("cursor-agent"));
        assert_eq!(find("qoder").map(|agent| agent.command), Some("qodercli"));
        assert_eq!(find("codebuddy").map(|agent| agent.name), Some("WorkBuddy / CodeBuddy"));
        assert_eq!(find("codebuddy").map(|agent| agent.homepage), Some("https://www.workbuddy.ai"));
        assert!(is_text_only("agy") && !is_text_only("claude") && !is_text_only("cursor"));
        assert!(seat_violation("agy", "共享").is_some());
        assert!(seat_violation("agy", "盲").is_none());
        assert!(seat_violation("claude", "共享").is_none());
        assert!(find("agy").unwrap().native);
        assert!(blind_only_reason("agy").unwrap().contains("纯文本盲审"));
        assert!(blind_only_reason("claude").is_none());
        assert!(fallback_path("agy").is_some_and(|path| path.ends_with("agy/bin/agy.exe")));
    }
}
