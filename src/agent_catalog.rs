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

/// 找 agent 的可执行文件：用户指定的路径（TASK-66）优先，其次支持清单登记的
/// 兜底路径，再走 PATH。探测、冒烟、派发都经这一处，结果一致（fix-61-r1 4）。
pub(crate) fn resolve_program(id: &str) -> Option<PathBuf> {
    let entry = find(id)?;
    if let Some(custom) = crate::resources::custom_program(entry.id) {
        return Some(custom);
    }
    if let Some(fallback) = fallback_path(entry.id).filter(|path| path.is_file()) {
        return Some(fallback);
    }
    crate::channels::resolve_binary(entry.command, desktop_bundled(entry.id)).ok()
}

/// 桌面端自带的命令行程序：只装了 Codex 或 Claude 桌面端、没装 CLI 时用它。
/// PATH 里的 CLI 优先，这里只作最后的兜底。路径里带版本号，每次现查目录，
/// 取版本最高且文件存在的一份。
pub(crate) fn desktop_bundled(id: &str) -> Option<PathBuf> {
    let candidates = match id {
        "codex" => codex_desktop_candidates(),
        "claude" => claude_desktop_candidates(),
        _ => Vec::new(),
    };
    candidates
        .into_iter()
        .filter(|path| path.is_file())
        .max_by(|a, b| version_key(a).cmp(&version_key(b)))
}

/// Codex 桌面端（Windows 商店包 OpenAI.Codex）把 codex.exe 放在
/// `<包目录>\app\resources\` 下。包目录先查注册表里登记的位置，再试直接
/// 列 WindowsApps（普通用户常常没有列目录权限）。
fn codex_desktop_candidates() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let mut roots = win_packages::package_roots("OpenAI.Codex_");
        if let Some(apps) = std::env::var_os("ProgramFiles").map(|dir| PathBuf::from(dir).join("WindowsApps")) {
            roots.extend(child_dirs_with_prefix(&apps, "OpenAI.Codex_"));
        }
        roots.into_iter().map(|root| root.join("app").join("resources").join("codex.exe")).collect()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// Claude 桌面端把 Claude Code 下载到 `<配置目录>\Claude\claude-code\<版本>\`；
/// 商店版的写入被重定向到 `%LOCALAPPDATA%\Packages\Claude_*\LocalCache\Roaming\Claude`。
/// 位置按公开资料写，未在装了 Claude 桌面端的机器上实测。
fn claude_desktop_candidates() -> Vec<PathBuf> {
    let exe = if cfg!(windows) { "claude.exe" } else { "claude" };
    let mut bases: Vec<PathBuf> = dirs::config_dir().into_iter().map(|dir| dir.join("Claude")).collect();
    if cfg!(windows) {
        if let Some(packages) = dirs::data_local_dir().map(|dir| dir.join("Packages")) {
            bases.extend(
                child_dirs_with_prefix(&packages, "Claude_")
                    .into_iter()
                    .map(|dir| dir.join("LocalCache").join("Roaming").join("Claude")),
            );
        }
    }
    bases
        .iter()
        .flat_map(|base| child_dirs_with_prefix(&base.join("claude-code"), ""))
        .map(|dir| dir.join(exe))
        .collect()
}

fn child_dirs_with_prefix(dir: &std::path::Path, prefix: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    entries
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(prefix))
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect()
}

/// 路径里的数字段（`OpenAI.Codex_26.1023.6123.0_x64__…`、`claude-code\2.1.3\`）
/// 按顺序取出，用来比新旧。
fn version_key(path: &std::path::Path) -> Vec<u64> {
    path.to_string_lossy()
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|part| part.parse().ok())
        .collect()
}

#[cfg(windows)]
mod win_packages {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::path::PathBuf;

    use ::windows::core::{w, PCWSTR, PWSTR};
    use ::windows::Win32::System::Registry::{
        RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_READ, RRF_RT_REG_SZ,
    };

    /// 当前用户已装的商店包里，包全名以 `prefix` 开头的安装目录
    /// （注册表 AppModel\Repository\Packages 下各包的 PackageRootFolder）。
    pub(super) fn package_roots(prefix: &str) -> Vec<PathBuf> {
        let mut key = HKEY::default();
        let opened = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                w!("Software\\Classes\\Local Settings\\Software\\Microsoft\\Windows\\CurrentVersion\\AppModel\\Repository\\Packages"),
                0,
                KEY_READ,
                &mut key,
            )
        };
        if opened.is_err() {
            return Vec::new();
        }
        let mut roots = Vec::new();
        for index in 0.. {
            let mut name = vec![0u16; 512];
            let mut len = name.len() as u32;
            let status = unsafe {
                RegEnumKeyExW(key, index, PWSTR(name.as_mut_ptr()), &mut len, None, PWSTR::null(), None, None)
            };
            if status.is_err() {
                break;
            }
            let package = OsString::from_wide(&name[..len as usize]).to_string_lossy().into_owned();
            if !package.starts_with(prefix) {
                continue;
            }
            name.truncate(len as usize);
            name.push(0);
            if let Some(root) = read_string(key, PCWSTR(name.as_ptr())) {
                roots.push(PathBuf::from(root));
            }
        }
        unsafe {
            let _ = RegCloseKey(key);
        }
        roots
    }

    fn read_string(key: HKEY, subkey: PCWSTR) -> Option<String> {
        let mut buffer = vec![0u16; 1024];
        let mut bytes = (buffer.len() * 2) as u32;
        let status = unsafe {
            RegGetValueW(
                key,
                subkey,
                w!("PackageRootFolder"),
                RRF_RT_REG_SZ,
                None,
                Some(buffer.as_mut_ptr().cast()),
                Some(&mut bytes),
            )
        };
        if status.is_err() {
            return None;
        }
        let text = &buffer[..(bytes as usize / 2).min(buffer.len())];
        let end = text.iter().position(|&unit| unit == 0).unwrap_or(text.len());
        Some(OsString::from_wide(&text[..end]).to_string_lossy().into_owned())
    }
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
    fn desktop_bundled_prefers_newest_version_dir() {
        let older = std::path::Path::new(r"C:\Program Files\WindowsApps\OpenAI.Codex_26.923.100.0_x64__2p2nqsd0c76g0\app\resources\codex.exe");
        let newer = std::path::Path::new(r"C:\Program Files\WindowsApps\OpenAI.Codex_26.1023.6123.0_x64__2p2nqsd0c76g0\app\resources\codex.exe");
        assert!(version_key(newer) > version_key(older));
        assert!(version_key(std::path::Path::new("claude-code/2.10.0/claude")) > version_key(std::path::Path::new("claude-code/2.9.9/claude")));
        assert_eq!(desktop_bundled("kimi"), None);
    }

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
