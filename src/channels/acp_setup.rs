//! ACP 适配器自备（09-18 用户要求「别让用户自己去搞额外的配置」）。
//!
//! claude-agent-acp / codex-acp 各带一份原生 CLI，装好后分别约 270 MB / 400 MB，塞不进安装包。
//! 做法（10-01 定案）：用户执行 `aicross config set acpEnabled true` 时当场检查，本机装了对应 CLI、
//! 又找不到适配器的，在前台用 npm 装进私有目录 `~/.aicross/acp`（不碰全局 npm），版本钉死，
//! 每家结果直接输出。派发途中不安装：适配器不在就回退一次性 CLI，原因写在节点错误里。

use crate::process::{run_process, ProcessResult};
use serde::Serialize;
use std::path::{Path, PathBuf};
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

/// 一家适配器的检查或安装结果；序列化成 `config` 命令输出里 `acp` 数组的一项。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub(crate) enum AdapterState {
    #[serde(rename = "已就绪")]
    Ready,
    #[serde(rename = "刚装好")]
    Installed,
    #[serde(rename = "未装对应 CLI")]
    NoCli,
    #[serde(rename = "未安装")]
    Missing,
    #[serde(rename = "没有 npm")]
    NoNpm,
    #[serde(rename = "安装失败")]
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct AdapterReport {
    pub agent: &'static str,
    pub state: AdapterState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl AdapterReport {
    /// 本机有对应 CLI 却没装上：派发对这家回退一次性 CLI。
    pub fn not_installed(&self) -> bool {
        matches!(self.state, AdapterState::NoNpm | AdapterState::Failed)
    }
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

/// 适配器缺席时进节点错误的一句话：为什么这一轮回退一次性 CLI。
pub fn unavailable_reason(agent: &str) -> String {
    let name = match agent {
        "claude" => "Claude Code",
        "codex" => "Codex",
        other => other,
    };
    format!("{name} 的 ACP 适配器尚未就绪；本轮回退一次性 CLI")
}

/// 安装用到的本机探测与 npm 执行；生产走 `install_missing_blocking`，测试注入假实现。
pub(crate) struct InstallEnv<'a> {
    pub adapter_ready: &'a dyn Fn(&str) -> bool,
    pub cli_present: &'a dyn Fn(&str) -> bool,
    pub npm: Option<PathBuf>,
    pub prefix: Result<PathBuf, String>,
    pub run_npm: &'a dyn Fn(&Path, &[String], &Path) -> Result<ProcessResult, String>,
}

/// 每家适配器当前是否就绪，只查不装。
pub(crate) fn adapter_status() -> Vec<AdapterReport> {
    adapter_status_with(&|agent| resolve_adapter(agent).is_ok(), &|cli| super::resolve_binary(cli, None).is_ok())
}

fn adapter_status_with(adapter_ready: &dyn Fn(&str) -> bool, cli_present: &dyn Fn(&str) -> bool) -> Vec<AdapterReport> {
    ADAPTERS
        .iter()
        .map(|adapter| AdapterReport {
            agent: adapter.agent,
            state: if adapter_ready(adapter.agent) {
                AdapterState::Ready
            } else if !cli_present(adapter.cli) {
                AdapterState::NoCli
            } else {
                AdapterState::Missing
            },
            reason: None,
        })
        .collect()
}

/// 缺哪家装哪家（只装本机有对应 CLI 的），一次 npm 调用装完，装完才返回。
pub(crate) fn install_missing_blocking() -> Vec<AdapterReport> {
    if cfg!(test) {
        panic!("测试里不得真的执行 npm 安装，改用 install_missing_with 注入");
    }
    let adapter_ready = |agent: &str| resolve_adapter(agent).is_ok();
    let cli_present = |cli: &str| super::resolve_binary(cli, None).is_ok();
    let run_npm = |npm: &Path, args: &[String], prefix: &Path| {
        let _ = std::fs::create_dir_all(prefix);
        run_process(npm, args, None, Duration::from_secs(1800), Some(prefix))
    };
    install_missing_with(&InstallEnv {
        adapter_ready: &adapter_ready,
        cli_present: &cli_present,
        npm: super::resolve_binary("npm", None).ok(),
        prefix: prefix(),
        run_npm: &run_npm,
    })
}

pub(crate) fn install_missing_with(env: &InstallEnv) -> Vec<AdapterReport> {
    let mut reports = adapter_status_with(env.adapter_ready, env.cli_present);
    let missing = reports
        .iter()
        .filter(|report| report.state == AdapterState::Missing)
        .filter_map(|report| adapter(report.agent))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return reports;
    }
    let outcome = match (&env.npm, &env.prefix) {
        (None, _) => Err((AdapterState::NoNpm, "未检测到 npm（Node.js）".to_string())),
        (_, Err(error)) => Err((AdapterState::Failed, error.clone())),
        (Some(npm), Ok(prefix)) => {
            let packages = missing
                .iter()
                .map(|adapter| format!("{}@{}", adapter.package, adapter.version))
                .collect::<Vec<_>>();
            Ok((env.run_npm)(npm, &install_args(&packages, prefix), prefix))
        }
    };
    for report in reports.iter_mut().filter(|report| report.state == AdapterState::Missing) {
        let Some(adapter) = adapter(report.agent) else { continue };
        let (state, reason) = match (&outcome, &env.prefix) {
            (Err((state, reason)), _) => (*state, Some(reason.clone())),
            (Ok(_), Ok(prefix)) if private_binary_in(prefix, adapter.binary).is_some() => (AdapterState::Installed, None),
            (Ok(result), _) => (AdapterState::Failed, Some(npm_failure(result))),
        };
        report.state = state;
        report.reason = reason;
    }
    reports
}

/// npm 没装出可执行入口时的原因：超时，或输出的最后几行。
fn npm_failure(result: &Result<ProcessResult, String>) -> String {
    match result {
        Ok(result) if result.timed_out => "npm 安装超时".to_string(),
        Ok(result) => {
            let text = if result.stderr.trim().is_empty() { &result.stdout } else { &result.stderr };
            let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>();
            if lines.is_empty() {
                return "npm 未报原因".to_string();
            }
            let tail = lines.split_off(lines.len().saturating_sub(3));
            tail.join(" / ").chars().take(300).collect()
        }
        Err(error) => error.clone(),
    }
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
    fn reasons_are_plain_and_say_the_turn_falls_back() {
        let reason = unavailable_reason("codex");
        assert!(reason.contains("Codex") && reason.contains("回退一次性 CLI"), "{reason}");
        // 不再提旧界面的概念（TASK-40 R1-7），也不再说后台在装。
        assert!(!reason.contains("聊天"));
        assert!(!reason.contains("权限卡片"));
        assert!(!reason.contains("重启"));
        assert!(!reason.contains("后台"));
    }

    fn temp_prefix() -> PathBuf {
        std::env::temp_dir().join(format!("aicross-acp-install-{}", uuid::Uuid::new_v4()))
    }

    fn npm_result(stderr: &str) -> ProcessResult {
        ProcessResult {
            code: Some(1),
            stdout: String::new(),
            stderr: stderr.to_string(),
            timed_out: false,
            truncated: false,
            timeout: Duration::from_secs(1800),
        }
    }

    fn state_of(reports: &[AdapterReport], agent: &str) -> AdapterState {
        reports.iter().find(|report| report.agent == agent).unwrap().state
    }

    #[test]
    fn task45_install_skips_npm_when_all_adapters_ready() {
        let calls = std::cell::Cell::new(0);
        let run_npm = |_: &Path, _: &[String], _: &Path| -> Result<ProcessResult, String> {
            calls.set(calls.get() + 1);
            Ok(npm_result(""))
        };
        let reports = install_missing_with(&InstallEnv {
            adapter_ready: &|_| true,
            cli_present: &|_| true,
            npm: Some(PathBuf::from("npm")),
            prefix: Ok(temp_prefix()),
            run_npm: &run_npm,
        });
        assert_eq!(calls.get(), 0);
        assert!(reports.iter().all(|report| report.state == AdapterState::Ready && !report.not_installed()));
        assert_eq!(serde_json::to_value(&reports[0]).unwrap(), serde_json::json!({"agent": "claude", "state": "已就绪"}));
    }

    #[test]
    fn task45_install_runs_npm_in_foreground_and_reports_fresh_install() {
        let prefix = temp_prefix();
        let calls = std::cell::RefCell::new(Vec::<Vec<String>>::new());
        let run_npm = |_: &Path, args: &[String], prefix: &Path| -> Result<ProcessResult, String> {
            calls.borrow_mut().push(args.to_vec());
            let bin = prefix.join("node_modules").join(".bin");
            std::fs::create_dir_all(&bin).unwrap();
            let name = if cfg!(windows) { "claude-agent-acp.cmd" } else { "claude-agent-acp" };
            std::fs::write(bin.join(name), "").unwrap();
            Ok(npm_result(""))
        };
        let reports = install_missing_with(&InstallEnv {
            adapter_ready: &|agent| agent == "codex",
            cli_present: &|_| true,
            npm: Some(PathBuf::from("npm")),
            prefix: Ok(prefix.clone()),
            run_npm: &run_npm,
        });
        let calls = calls.into_inner();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], install_args(&["@agentclientprotocol/claude-agent-acp@0.79.0".to_string()], &prefix));
        assert_eq!(state_of(&reports, "claude"), AdapterState::Installed);
        assert_eq!(state_of(&reports, "codex"), AdapterState::Ready);
        let _ = std::fs::remove_dir_all(prefix);
    }

    #[test]
    fn task45_install_skips_agents_without_their_cli() {
        let calls = std::cell::Cell::new(0);
        let run_npm = |_: &Path, _: &[String], _: &Path| -> Result<ProcessResult, String> {
            calls.set(calls.get() + 1);
            Ok(npm_result(""))
        };
        let reports = install_missing_with(&InstallEnv {
            adapter_ready: &|agent| agent == "claude",
            cli_present: &|cli| cli == "claude",
            npm: Some(PathBuf::from("npm")),
            prefix: Ok(temp_prefix()),
            run_npm: &run_npm,
        });
        assert_eq!(calls.get(), 0);
        assert_eq!(state_of(&reports, "codex"), AdapterState::NoCli);
        assert!(!reports.iter().any(AdapterReport::not_installed));
    }

    #[test]
    fn task45_install_reports_missing_npm() {
        let run_npm = |_: &Path, _: &[String], _: &Path| -> Result<ProcessResult, String> {
            panic!("没有 npm 时不应执行安装")
        };
        let reports = install_missing_with(&InstallEnv {
            adapter_ready: &|_| false,
            cli_present: &|_| true,
            npm: None,
            prefix: Ok(temp_prefix()),
            run_npm: &run_npm,
        });
        assert!(reports.iter().all(|report| report.state == AdapterState::NoNpm && report.not_installed()));
        assert!(reports[0].reason.as_deref().unwrap().contains("npm"));
        assert_eq!(serde_json::to_value(&reports[0]).unwrap()["state"], "没有 npm");
    }

    #[test]
    fn task45_install_failure_carries_last_npm_lines() {
        let prefix = temp_prefix();
        let run_npm = |_: &Path, _: &[String], _: &Path| -> Result<ProcessResult, String> {
            Ok(npm_result("npm warn 第一行
npm error code ETIMEDOUT
npm error network 连接超时
npm error 日志在 x.log
"))
        };
        let reports = install_missing_with(&InstallEnv {
            adapter_ready: &|_| false,
            cli_present: &|_| true,
            npm: Some(PathBuf::from("npm")),
            prefix: Ok(prefix.clone()),
            run_npm: &run_npm,
        });
        for report in &reports {
            assert_eq!(report.state, AdapterState::Failed);
            assert!(report.not_installed());
            let reason = report.reason.as_deref().unwrap();
            assert!(reason.contains("ETIMEDOUT") && reason.contains("日志在 x.log"), "{reason}");
            assert!(!reason.contains("第一行"), "{reason}");
        }
        assert_eq!(serde_json::to_value(&reports[1]).unwrap()["state"], "安装失败");
        let _ = std::fs::remove_dir_all(prefix);
    }
}
