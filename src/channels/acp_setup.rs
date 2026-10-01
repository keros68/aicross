//! ACP 适配器自备（09-18 用户要求「别让用户自己去搞额外的配置」）。
//!
//! claude-agent-acp / codex-acp 各带一份原生 CLI，装好后分别约 270 MB / 400 MB，塞不进安装包。
//! 做法：本机装了对应 CLI、又找不到适配器时，应用在后台用 npm 装进自己的私有目录
//! `~/.aicross/acp`（不碰全局 npm），版本钉死。装好之前控制台照常可用——派发自动回退一次性 CLI，
//! 只是没有 `/` 补全与权限卡片。没有 Node.js（npm）的机器同样回退，并在界面说明原因。

use crate::process::run_process;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

struct Adapter {
    agent: &'static str,
    binary: &'static str,
    package: &'static str,
    version: &'static str,
    /// 本机有这个 CLI 才值得装它的适配器。
    cli: &'static str,
}

const ADAPTERS: [Adapter; 2] = [
    Adapter {
        agent: "claude",
        binary: "claude-agent-acp",
        package: "@agentclientprotocol/claude-agent-acp",
        version: "0.79.0",
        cli: "claude",
    },
    Adapter {
        agent: "codex",
        binary: "codex-acp",
        package: "@agentclientprotocol/codex-acp",
        version: "1.12.0",
        cli: "codex",
    },
];

fn adapter(agent: &str) -> Option<&'static Adapter> {
    ADAPTERS.iter().find(|adapter| adapter.agent == agent)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetupState {
    /// 还没检查或不需要（本机没有对应 CLI）。
    Idle,
    Installing,
    Ready,
    NoNode,
    Failed(String),
}

fn states() -> &'static Mutex<HashMap<&'static str, SetupState>> {
    static STATES: OnceLock<Mutex<HashMap<&'static str, SetupState>>> = OnceLock::new();
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

static INSTALLING: AtomicBool = AtomicBool::new(false);

fn last_failure() -> &'static Mutex<Option<std::time::Instant>> {
    static LAST: OnceLock<Mutex<Option<std::time::Instant>>> = OnceLock::new();
    LAST.get_or_init(|| Mutex::new(None))
}

fn set_state(agent: &'static str, state: SetupState) {
    if let Ok(mut states) = states().lock() {
        states.insert(agent, state);
    }
}

pub fn state(agent: &str) -> SetupState {
    states()
        .lock()
        .ok()
        .and_then(|states| states.get(agent).cloned())
        .unwrap_or(SetupState::Idle)
}

fn prefix() -> Result<PathBuf, String> {
    Ok(crate::paths::aicross_root()?.join("acp"))
}

/// npm `--prefix` 安装后可执行入口在 `<prefix>/node_modules/.bin/` 下（Windows 为 `.cmd`）。
pub(crate) fn private_binary_in(prefix: &Path, binary: &str) -> Option<PathBuf> {
    let bin = prefix.join("node_modules").join(".bin");
    let candidates = if cfg!(windows) {
        vec![bin.join(format!("{binary}.cmd")), bin.join(format!("{binary}.exe"))]
    } else {
        vec![bin.join(binary)]
    };
    candidates.into_iter().find(|path| path.is_file())
}

/// 先私有目录（应用自己装的，版本钉死），再 PATH（用户自己全局装的）。
pub fn resolve_adapter(agent: &str) -> Result<PathBuf, String> {
    let adapter = adapter(agent).ok_or_else(|| format!("{agent} 没有 ACP 适配器"))?;
    if let Some(found) = prefix().ok().and_then(|prefix| private_binary_in(&prefix, adapter.binary)) {
        return Ok(found);
    }
    super::resolve_binary(adapter.binary, None)
}

pub(crate) fn install_args(packages: &[String], prefix: &Path) -> Vec<String> {
    let mut args = vec![
        "install".to_string(),
        "--prefix".to_string(),
        prefix.to_string_lossy().into_owned(),
        "--no-audit".to_string(),
        "--no-fund".to_string(),
        "--loglevel=error".to_string(),
    ];
    args.extend(packages.iter().cloned());
    args
}

/// 用户看得懂的一句话：为什么这一家暂时没有 `/` 命令与权限卡片。
pub fn unavailable_reason(agent: &str) -> String {
    let name = match agent {
        "claude" => "Claude Code",
        "codex" => "Codex",
        other => other,
    };
    match state(agent) {
        SetupState::Installing => format!("正在后台准备 {name} 的命令与权限支持（首次需下载几百 MB，约几分钟）；聊天照常可用"),
        SetupState::NoNode => format!("未检测到 Node.js，{name} 的 / 命令与权限卡片不可用；聊天照常可用。安装 Node.js 后重启本应用即自动启用"),
        SetupState::Failed(detail) => format!("{name} 的命令与权限支持安装失败：{detail}；聊天照常可用，重启本应用会再试"),
        _ => format!("{name} 的命令与权限支持尚未就绪；聊天照常可用"),
    }
}

/// 缺哪家装哪家（只装本机有对应 CLI 的），一次 npm 调用装完；同一时间只跑一个安装。
/// 非阻塞：立即返回，结果经 `acp-setup` 事件推给界面。
pub fn ensure_installed() {
    let missing = ADAPTERS
        .iter()
        .filter(|adapter| resolve_adapter(adapter.agent).is_err())
        .filter(|adapter| super::resolve_binary(adapter.cli, None).is_ok())
        .collect::<Vec<_>>();
    for adapter in ADAPTERS.iter().filter(|adapter| resolve_adapter(adapter.agent).is_ok()) {
        if state(adapter.agent) != SetupState::Ready {
            set_state(adapter.agent, SetupState::Ready);
        }
    }
    if missing.is_empty() {
        return;
    }
    let npm = match super::resolve_binary("npm", None) {
        Ok(npm) => npm,
        Err(_) => {
            for adapter in &missing {
                set_state(adapter.agent, SetupState::NoNode);
            }
            return;
        }
    };
    // 刚失败过（网络、npm 报错）就先别每次输入 / 都重装一遍：10 分钟后或重启应用再试。
    let recently_failed = last_failure()
        .lock()
        .is_ok_and(|at| at.is_some_and(|at| at.elapsed() < Duration::from_secs(600)));
    if recently_failed {
        return;
    }
    if INSTALLING.swap(true, Ordering::AcqRel) {
        return;
    }
    let prefix = match prefix() {
        Ok(prefix) => prefix,
        Err(error) => {
            for adapter in &missing {
                set_state(adapter.agent, SetupState::Failed(error.clone()));
            }
            INSTALLING.store(false, Ordering::Release);
            return;
        }
    };
    for adapter in &missing {
        set_state(adapter.agent, SetupState::Installing);
    }
    let agents = missing.iter().map(|adapter| adapter.agent).collect::<Vec<_>>();
    let packages = missing
        .iter()
        .map(|adapter| format!("{}@{}", adapter.package, adapter.version))
        .collect::<Vec<_>>();
    thread::spawn(move || {
        let _ = std::fs::create_dir_all(&prefix);
        let outcome = run_process(&npm, &install_args(&packages, &prefix), None, Duration::from_secs(1800), Some(&prefix));
        for agent in agents {
            let Some(adapter) = adapter(agent) else { continue };
            let state = if private_binary_in(&prefix, adapter.binary).is_some() {
                SetupState::Ready
            } else {
                let detail = match &outcome {
                    Ok(result) if result.timed_out => "npm 安装超时".to_string(),
                    Ok(result) => result
                        .stderr
                        .lines()
                        .rev()
                        .find(|line| !line.trim().is_empty())
                        .unwrap_or("npm 未报原因")
                        .chars()
                        .take(200)
                        .collect(),
                    Err(error) => error.clone(),
                };
                if let Ok(mut at) = last_failure().lock() {
                    *at = Some(std::time::Instant::now());
                }
                SetupState::Failed(detail)
            };
            set_state(adapter.agent, state);
        }
        INSTALLING.store(false, Ordering::Release);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_install_is_preferred_and_found_under_node_modules_bin() {
        let root = std::env::temp_dir().join(format!("aicross-acp-prefix-{}", uuid::Uuid::new_v4()));
        let bin = root.join("node_modules").join(".bin");
        std::fs::create_dir_all(&bin).unwrap();
        assert!(private_binary_in(&root, "claude-agent-acp").is_none());
        let name = if cfg!(windows) { "claude-agent-acp.cmd" } else { "claude-agent-acp" };
        std::fs::write(bin.join(name), "").unwrap();
        assert_eq!(private_binary_in(&root, "claude-agent-acp"), Some(bin.join(name)));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn npm_install_is_pinned_and_scoped_to_the_private_prefix() {
        let prefix = Path::new("C:/x/.aicross/acp");
        let args = install_args(&["@agentclientprotocol/codex-acp@1.12.0".to_string()], prefix);
        assert_eq!(&args[..3], ["install", "--prefix", "C:/x/.aicross/acp"]);
        assert!(!args.iter().any(|arg| arg == "-g" || arg == "--global"));
        assert_eq!(args.last().unwrap(), "@agentclientprotocol/codex-acp@1.12.0");
        assert!(ADAPTERS.iter().all(|adapter| !adapter.version.is_empty() && !adapter.version.starts_with('^')));
    }

    #[test]
    fn reasons_are_plain_and_say_chat_still_works() {
        for state in [SetupState::Installing, SetupState::NoNode, SetupState::Failed("网络错误".into())] {
            set_state("codex", state);
            let reason = unavailable_reason("codex");
            assert!(reason.contains("Codex") && reason.contains("聊天照常可用"), "{reason}");
        }
        set_state("codex", SetupState::Idle);
    }
}

