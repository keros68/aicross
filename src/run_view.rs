//! TASK-37 查看器第一版：`aicross open <run_id>` 的参数解析，首页列表
//! （TASK-40）与 `load_run_view` / `adopt_run_node` 两条 Tauri 命令。
//!
//! 冷启动经 `AppState::pending_open` 与 `take_pending_open`；热启动由单实例
//! 回调在这里重解析 argv 后向前端发 `open-run-request` 事件（见 server.rs）。
//! 打开查看页只读 run 文件与图文件，不派发、不改文件。
//!
//! 数据只从 TASK-36 的产出读：`~/.aicross/runs/<run_id>.json`（经
//! `engine::read_run`，会顺带完成死进程判定）加 `~/.aicross/graphs/` 里该
//! run 的图。页头的模式名取 run 文件里的 `preset_id`（R1-4），按预设清单
//! 取显示名；旧 run 文件没有该字段，按空处理、显示「自定义模式」。
//! 冷启动的 `aicross open` 以分离方式另起窗口进程后立即退出（R1-5），
//! 拉起与被拉起靠 `OPEN_WINDOW_ENV` 区分，不无限接力。

use crate::orchestration;
use serde::Serialize;
use std::path::Path;

// ── `aicross open <run_id>` 参数解析 ─────────────────────────────────────

/// argv 解析结果：不是 open 请求 / 参数错（带原因，退出码 2）/ 合法（run id）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenParse {
    NotOpen,
    Invalid(String),
    Valid(String),
}

/// argv[0] 形状不定（裸名、完整路径或单实例转发），`open` 认 0 或 1 下标；`--flag` / `--flag=value`
/// 一律拒绝（本命令没有选项），只收一个 run_id，字符集先过 `valid_id`。
pub fn parse_open_args(args: &[String]) -> OpenParse {
    let start = match args.first().map(String::as_str) {
        Some("open") => 1,
        _ if args.get(1).map(String::as_str) == Some("open") => 2,
        _ => return OpenParse::NotOpen,
    };
    let rest = &args[start..];
    if rest.is_empty() {
        return OpenParse::Invalid("缺少 run_id".into());
    }
    if rest.iter().any(|arg| arg.starts_with('-')) {
        return OpenParse::Invalid("open 不接受选项参数".into());
    }
    if rest.len() > 1 {
        return OpenParse::Invalid("open 只接受一个 run_id".into());
    }
    let run_id = rest[0].as_str();
    if !crate::engine::valid_id(run_id) {
        return OpenParse::Invalid("run_id 无效".into());
    }
    OpenParse::Valid(run_id.to_string())
}

// ── 冷启动的窗口拉起（R1-5） ─────────────────────────────────────

/// 区分「宿主里调起的 `aicross open`」与「它拉起的窗口进程」的环境变量：
/// 拉起前置位，被拉起的进程凭它不再拉起下一个，避免无限接力。
pub const OPEN_WINDOW_ENV: &str = "AICROSS_OPEN_WINDOW";

/// 被拉起的窗口进程收到的 argv：`open <run_id>`，照常走 `pending_open`。
pub fn open_window_args(run_id: &str) -> Vec<String> {
    vec!["open".into(), run_id.into()]
}

/// 注入 spawn 的拉起入口（测试用）：置位环境变量后交给 spawn。
/// main 里单线程调用，改进程环境没有并发风险。
pub fn spawn_open_window_with(
    exe: &Path,
    run_id: &str,
    spawn: impl FnOnce(&Path, &[String]) -> Result<(), String>,
) -> Result<(), String> {
    std::env::set_var(OPEN_WINDOW_ENV, "1");
    spawn(exe, &open_window_args(run_id))
}

/// 冷启动 `aicross open`：分离拉起窗口进程（不继承句柄、不挂父控制台），
/// 返回后调用方立即退出 0，不等窗口。Windows 上照引擎 `spawn_detached` 的
/// 做法：std 的 Command 一律以 bInheritHandles=TRUE 创建子进程，宿主的
/// 管道会漏进子进程、拖住调用方的 EOF；这里直接 CreateProcessW 且
/// bInheritHandles=FALSE（引擎侧函数不外放，同做法在本文件内复制）。
pub fn spawn_open_window(run_id: &str) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("定位程序失败：{e}"))?;
    spawn_open_window_with(&exe, run_id, spawn_window_detached)
}

#[cfg(windows)]
fn spawn_window_detached(exe: &Path, args: &[String]) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        CreateProcessW, CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW, DETACHED_PROCESS,
        PROCESS_INFORMATION, STARTUPINFOW,
    };
    let application: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
    // 参数只有 `open` 与已过 valid_id 校验的 run id，不含引号或空白。
    let quoted = std::iter::once(format!("\"{}\"", exe.display()))
        .chain(args.iter().map(|a| format!("\"{a}\"")))
        .collect::<Vec<_>>()
        .join(" ");
    let mut command_line: Vec<u16> = quoted.encode_utf16().chain(Some(0)).collect();
    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..STARTUPINFOW::default()
    };
    let mut information = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessW(
            PCWSTR(application.as_ptr()),
            PWSTR(command_line.as_mut_ptr()),
            None,
            None,
            false,
            CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP,
            None,
            PCWSTR::null(),
            &startup,
            &mut information,
        )
        .map_err(|e| format!("启动窗口进程失败：{e}"))?;
        let _ = CloseHandle(information.hThread);
        let _ = CloseHandle(information.hProcess);
    }
    Ok(())
}

#[cfg(not(windows))]
fn spawn_window_detached(exe: &Path, args: &[String]) -> Result<(), String> {
    std::process::Command::new(exe)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("启动窗口进程失败：{e}"))
}

// ── 数据装配 ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunViewArtifacts {
    pub worktree: String,
    pub diff: String,
    pub changed_file_count: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunViewNode {
    pub node_id: String,
    /// 图深度（根材料为 0），页面上按它分阶段；图读不到时按 1 兜底。
    pub stage: u32,
    pub role: String,
    pub agent: String,
    pub provider: String,
    /// 请求的模型 id。
    pub model: String,
    /// 图上 reply 的状态词：draft / pending / done / failed / quota_exhausted。
    pub status: String,
    /// 失败细分（timeout / identity_mismatch / validation_failed / cancelled…）。
    pub error_kind: String,
    pub adopted: bool,
    /// 图上有没有 reply；adopt 只对有结果的节点生效。
    pub has_result: bool,
    pub text: String,
    pub skipped_reason: String,
    /// 实际回答的模型；拿不到为空串，前端标「未报告」，不回填请求值。
    pub model_reported: String,
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
    pub tokens_cache: Option<u64>,
    pub ms: Option<u64>,
    pub fallback_from: String,
    pub fallback_to: String,
    pub quota_resets_at: String,
    /// 蜂群件的标题（节点正文首行）；不是蜂群件为空串。
    pub swarm_title: String,
    pub artifacts: Option<RunViewArtifacts>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunView {
    pub run_id: String,
    pub graph_id: String,
    pub project: String,
    /// 模式名：按 run 文件的 preset_id 从预设清单取（内置用自己的名字，
    /// 用户自建为「自定义模式 · 名称」）；为空或找不到时「自定义模式」。
    pub preset_name: String,
    pub graph_title: String,
    /// 本次派发的任务名：run 文件里原始需求的第一行（同项目的派发共用一张图，
    /// 图标题只是该项目第一次派发的任务）。
    pub task_title: String,
    pub status: String,
    pub reason: String,
    pub started_at: String,
    pub updated_at: String,
    pub leftover: Vec<String>,
    /// 按 run 摘要里的节点顺序。
    pub nodes: Vec<RunViewNode>,
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

/// CLI 侧的存在性核对（R1-7）：只有「run 文件不存在」算找不到（退出码 1），
/// 其它读取错误（目录不可读等）由调用方按退出码 3 处理。路径布局与引擎的
/// `run_path` 一致（该函数不外放，这里只能重复这一处知识）。
pub fn open_run_exists(root: &Path, run_id: &str) -> Result<bool, String> {
    if !crate::engine::valid_id(run_id) {
        return Err("run_id 无效".into());
    }
    let path = root.join("runs").join(format!("{run_id}.json"));
    match std::fs::metadata(&path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("读取 run 失败：{error}")),
    }
}

/// 模式名：按 preset_id 从预设清单取显示名（R1-4）。内置预设用自己的名字；
/// 用户自建为「自定义模式 · 名称」（名称空白时只留「自定义模式」）；
/// id 为空（旧 run 文件、targets 直给）或清单里找不到时「自定义模式」。
/// 清单由调用方读一次传入（fix-40-r1 R1-10：列表页不逐条重读）。
fn preset_display_name(presets: &orchestration::PresetFile, preset_id: &str) -> String {
    if preset_id.is_empty() {
        return "自定义模式".into();
    }
    let Some(preset) = presets.presets.iter().find(|p| p.id == preset_id) else {
        return "自定义模式".into();
    };
    if preset.builtin {
        return preset.name.clone();
    }
    if preset.name.trim().is_empty() {
        "自定义模式".into()
    } else {
        format!("自定义模式 · {}", preset.name.trim())
    }
}

/// 读 run 文件与图文件，拼查看页数据。图读不出来（被删、解析失败）时降级：
/// run 摘要照常返回，正文与产物缺席。
pub fn build_run_view(root: &Path, run_id: &str) -> Result<RunView, String> {
    let run = crate::engine::read_run(root, run_id)?;
    let presets = orchestration::raw_presets_from(root);
    let graph = if run.graph_id.is_empty() {
        None
    } else {
        crate::orchestration::load_graph_in(root, &run.graph_id)
            .ok()
            .flatten()
    };
    let depths = graph.as_ref().map(crate::engine::depth_map);
    let nodes = run
        .nodes
        .iter()
        .map(|summary| {
            let node = graph.as_ref().and_then(|g| g.nodes.get(&summary.node_id));
            let target = node.and_then(|n| n.target.as_ref());
            let reply = node.and_then(|n| n.reply.as_ref());
            RunViewNode {
                node_id: summary.node_id.clone(),
                stage: depths
                    .as_ref()
                    .and_then(|d| d.get(&summary.node_id))
                    .copied()
                    .unwrap_or(1),
                role: target
                    .map(|t| t.role.clone())
                    .and_then(non_empty)
                    .unwrap_or_else(|| summary.role.clone()),
                agent: target
                    .map(|t| t.agent.clone())
                    .and_then(non_empty)
                    .unwrap_or_else(|| summary.agent.clone()),
                provider: target
                    .and_then(|t| t.provider.clone())
                    .unwrap_or_default(),
                model: target
                    .map(|t| t.model.clone())
                    .and_then(non_empty)
                    .unwrap_or_else(|| summary.model.clone()),
                status: reply
                    .map(|r| r.status.clone())
                    .unwrap_or_else(|| summary.status.clone()),
                error_kind: reply
                    .and_then(|r| r.error_kind.clone())
                    .unwrap_or_default(),
                adopted: reply.and_then(|r| r.adopted) == Some(true),
                has_result: reply.is_some(),
                text: reply.map(|r| r.text.clone()).unwrap_or_default(),
                skipped_reason: reply
                    .and_then(|r| r.skipped_reason.clone())
                    .unwrap_or_default(),
                model_reported: reply
                    .and_then(|r| r.model_reported.clone())
                    .unwrap_or_default(),
                tokens_in: reply.and_then(|r| r.tokens_in),
                tokens_out: reply.and_then(|r| r.tokens_out),
                tokens_cache: reply.and_then(|r| r.tokens_cache_read),
                ms: reply.and_then(|r| r.ms),
                fallback_from: reply
                    .and_then(|r| r.fallback_from.clone())
                    .unwrap_or_default(),
                fallback_to: reply
                    .and_then(|r| r.fallback_to.clone())
                    .unwrap_or_default(),
                quota_resets_at: reply
                    .and_then(|r| r.quota_resets_at.clone())
                    .unwrap_or_default(),
                swarm_title: node
                    .filter(|n| n.swarm.is_some())
                    .map(|n| {
                        n.user
                            .text
                            .lines()
                            .next()
                            .unwrap_or("")
                            .trim()
                            .to_string()
                    })
                    .unwrap_or_default(),
                artifacts: reply.and_then(|r| r.artifacts.as_ref()).map(|a| RunViewArtifacts {
                    worktree: a.worktree.clone().unwrap_or_default(),
                    diff: a.diff.clone(),
                    changed_file_count: a.changed_files.len(),
                }),
            }
        })
        .collect();
    let run_node_ids: std::collections::HashSet<String> =
        run.nodes.iter().map(|n| n.node_id.clone()).collect();
    Ok(RunView {
        run_id: run.run_id.clone(),
        graph_id: run.graph_id.clone(),
        project: run.project.clone(),
        preset_name: preset_display_name(&presets, &run.preset_id),
        graph_title: graph
            .as_ref()
            .map(|g| g.title.clone())
            .unwrap_or_default(),
        task_title: task_title(&run.original_request),
        status: run.status.clone(),
        reason: run.reason.clone(),
        started_at: run.started_at.clone(),
        updated_at: run.updated_at.clone(),
        // 同项目的派发共用一张图，run 摘要里的遗留会带上别的 run 的；这里只取
        // 来源节点属于本 run 的遗留节点。读不到图时退回 run 摘要。
        leftover: match graph.as_ref() {
            Some(g) => run_leftover(g, &run_node_ids),
            None => run.leftover.clone(),
        },
        nodes,
    })
}

// ── Tauri 命令 ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn load_run_view(run_id: String) -> Result<RunView, String> {
    build_run_view(&crate::paths::aicross_root()?, &run_id)
}

/// 包 TASK-36 的 `adopt`（经其合并写，不整写 run 文件），随后按同一口径重读，
/// 返回采纳后的整页数据。命令包一层引擎根目录。
pub fn adopt_view(root: &Path, run_id: &str, node_id: &str) -> Result<RunView, String> {
    crate::engine::adopt(root, run_id, node_id)?;
    build_run_view(root, run_id)
}

#[tauri::command]
pub fn adopt_run_node(run_id: String, node_id: String) -> Result<RunView, String> {
    adopt_view(&crate::paths::aicross_root()?, &run_id, &node_id)
}

// ── TASK-40 首页「最近的派发」────────────────────────────────────────────

/// 首页列表条目。模式名与任务名同查看页一套取名规则（`preset_display_name`
/// 与 `task_title`）。
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunListItem {
    pub run_id: String,
    pub preset_name: String,
    pub status: String,
    pub started_at: String,
    pub task_title: String,
}

/// 首页列表条数上限：只展示最近 50 条，更早的仍在磁盘上，`aicross open <run_id>`
/// 随时可开。
const LIST_RUNS_LIMIT: usize = 50;

/// 读 run 目录里的 run 摘要，按 `updated_at` 倒序取最近 50 条。与引擎
/// `active_runs` 同一口径：只认 `.json` 且文件名不以 `.` 开头；解析失败的
/// 文件跳过，不让一张坏文件拽垮整个列表。
///
/// 读盘量（fix-40-r1 R1-10）：先按文件修改时间粗排，只完整解析最近 50 个
/// 附近的文件（与第 50 名 mtime 并列的一并解析，避免同刻度写入把新文件
/// 挤出窗口）；预设清单整页只读一次。状态用引擎的只读死进程判定
/// （`display_status`），与查看页同一口径。
pub fn list_runs_at(root: &Path) -> Vec<RunListItem> {
    let Ok(entries) = std::fs::read_dir(root.join("runs")) else {
        return Vec::new();
    };
    let mut candidates: Vec<(std::time::SystemTime, std::path::PathBuf)> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().and_then(|e| e.to_str()) == Some("json")
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| !n.starts_with('.'))
        })
        .map(|path| {
            let modified = path
                .metadata()
                .and_then(|meta| meta.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            (modified, path)
        })
        .collect();
    // mtime 取不到的落在最后；这里的排序只是粗排，选出候选后仍按
    // updated_at 精排。
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    // 与第 50 名 mtime 并列的候选一并解析：文件系统时间戳有粒度，同刻度
    // 写入的新文件不能凭粗排挤掉。
    let boundary = (candidates.len() > LIST_RUNS_LIMIT)
        .then(|| candidates.get(LIST_RUNS_LIMIT - 1)).flatten().map(|(t, _)| *t);
    let mut items: Vec<(String, crate::engine::EngineRun)> = candidates
        .into_iter()
        .take_while(|(t, _)| boundary.is_none_or(|b| *t >= b))
        .filter_map(|(_, path)| {
            let bytes = std::fs::read(&path).ok()?;
            let run = serde_json::from_slice::<crate::engine::EngineRun>(&bytes).ok()?;
            Some((path, run))
        })
        .map(|(path, run)| {
            let run_id = if run.run_id.is_empty() {
                path.file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string()
            } else {
                run.run_id.clone()
            };
            (run_id, run)
        })
        .collect();
    // updated_at 是 ISO 字符串，字典序即时间序；倒序取最近的。
    items.sort_by(|a, b| b.1.updated_at.cmp(&a.1.updated_at));
    items.truncate(LIST_RUNS_LIMIT);
    let presets = orchestration::raw_presets_from(root);
    items
        .into_iter()
        .map(|(run_id, run)| RunListItem {
            preset_name: preset_display_name(&presets, &run.preset_id),
            task_title: task_title(&run.original_request),
            run_id,
            status: crate::engine::display_status(&run).to_string(),
            started_at: run.started_at,
        })
        .collect()
}

/// 原始需求的第一行非空文本，最多 40 个字符（超出加「…」）。
fn task_title(original_request: &str) -> String {
    let line = original_request
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let mut out: String = line.chars().take(40).collect();
    if line.chars().count() > 40 {
        out.push('…');
    }
    out
}

/// 只取来源节点属于本 run 的遗留节点的条目，按图上节点顺序。
fn run_leftover(graph: &orchestration::Graph, run_node_ids: &std::collections::HashSet<String>) -> Vec<String> {
    graph
        .nodes
        .values()
        .filter_map(|n| n.leftover.as_ref())
        .filter(|l| run_node_ids.contains(&l.source_node_id))
        .flat_map(|l| l.items.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{EngineRun, RunNode};
    use crate::orchestration::{
        Graph, GraphNode, Message, Preset, Reply, ReplyArtifacts, Stage, SwarmItem, Target,
    };
    use chrono::{SecondsFormat, Utc};
    use uuid::Uuid;

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = crate::paths::test_temp_dir().join(format!("aicross-view-{name}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn now_text() -> String {
        Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
    }

    fn turn(id: &str, parent: &str, role: &str) -> GraphNode {
        GraphNode {
            id: id.into(),
            kind: "turn".into(),
            parents: vec![parent.into()],
            target: Some(Target {
                agent: "claude".into(),
                model: "sonnet".into(),
                role: role.into(),
                thinking: "中".into(),
                visibility: "共享".into(),
                write: "只读".into(),
                ..Target::default()
            }),
            reply: Some(Reply {
                status: "done".into(),
                text: "正文 PASS".into(),
                model_reported: Some("claude-sonnet-4-6".into()),
                tokens_in: Some(100),
                tokens_out: Some(20),
                tokens_cache_read: Some(5),
                ms: Some(1234),
                ..Reply::default()
            }),
            created_at: now_text(),
            ..GraphNode::default()
        }
    }

    /// 三阶段经济形态的图：出方案 → 执行 → 验收 + 验证。
    fn economy_graph() -> Graph {
        let mut graph = crate::engine::new_graph("D:/p", "g1".into(), "root".into(), now_text());
        graph.title = "给解析器加测试".into();
        let plan = turn("n-plan", "root", "出方案");
        let exec = turn("n-exec", "n-plan", "执行");
        let accept = turn("n-accept", "n-exec", "验收");
        let verify = turn("n-verify", "n-exec", "验证");
        for (id, node) in [
            ("n-plan", plan),
            ("n-exec", exec),
            ("n-accept", accept),
            ("n-verify", verify),
        ] {
            graph.nodes.insert(id.into(), node);
        }
        graph
    }

    fn run_node(id: &str, role: &str) -> RunNode {
        RunNode {
            node_id: id.into(),
            role: role.into(),
            agent: "claude".into(),
            model: "sonnet".into(),
            status: "done".into(),
            ..RunNode::default()
        }
    }

    fn write_run_file(root: &Path, run: &EngineRun) {
        let dir = root.join("runs");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{}.json", run.run_id)),
            serde_json::to_vec_pretty(run).unwrap(),
        )
        .unwrap();
    }

    // ── 参数解析 ─────────────────────────────────────────────────────────

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn open_parse_covers_argv0_flags_and_invalid_ids() {
        assert_eq!(parse_open_args(&argv(&["aicross"])), OpenParse::NotOpen);
        assert_eq!(parse_open_args(&argv(&["aicross", "status"])), OpenParse::NotOpen);
        // argv[0] 有无都认。
        assert_eq!(
            parse_open_args(&argv(&["aicross", "open", "run-1"])),
            OpenParse::Valid("run-1".into())
        );
        assert_eq!(
            parse_open_args(&argv(&["open", "run_1"])),
            OpenParse::Valid("run_1".into())
        );
        // 缺 run_id、多位置参数、非法字符。
        assert_eq!(
            parse_open_args(&argv(&["aicross", "open"])),
            OpenParse::Invalid("缺少 run_id".into())
        );
        assert_eq!(
            parse_open_args(&argv(&["aicross", "open", "a", "b"])),
            OpenParse::Invalid("open 只接受一个 run_id".into())
        );
        assert_eq!(
            parse_open_args(&argv(&["aicross", "open", "bad/id"])),
            OpenParse::Invalid("run_id 无效".into())
        );
        // --flag 与 --flag=value 两种形态都拒绝。
        assert_eq!(
            parse_open_args(&argv(&["aicross", "open", "--wait"])),
            OpenParse::Invalid("open 不接受选项参数".into())
        );
        assert_eq!(
            parse_open_args(&argv(&["aicross", "open", "--run=abc"])),
            OpenParse::Invalid("open 不接受选项参数".into())
        );
    }

    // ── load_run_view 四种 run ───────────────────────────────────────────

    #[test]
    fn load_run_view_missing_run_is_error() {
        let root = temp("missing");
        assert!(build_run_view(&root, "no-such-run").is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn load_run_view_running_keeps_status_and_maps_preset_id() {
        let root = temp("running");
        let graph = economy_graph();
        orchestration::save_graph_in(&root, &graph).unwrap();
        let run = EngineRun {
            run_id: "r1".into(),
            graph_id: "g1".into(),
            project: "D:/p".into(),
            status: "running".into(),
            preset_id: "builtin-economy".into(),
            nodes: vec![
                run_node("n-plan", "出方案"),
                run_node("n-exec", "执行"),
                run_node("n-accept", "验收"),
                run_node("n-verify", "验证"),
            ],
            started_at: now_text(),
            updated_at: now_text(),
            ..EngineRun::default()
        };
        write_run_file(&root, &run);
        // pid 为 0 且 updated_at 新鲜：仍在启动窗口内，保持 running。
        let view = build_run_view(&root, "r1").unwrap();
        assert_eq!(view.status, "running");
        assert_eq!(view.preset_name, "经济模式");
        assert_eq!(view.graph_title, "给解析器加测试");
        assert_eq!(view.nodes.len(), 4);
        assert_eq!(
            view.nodes.iter().map(|n| n.stage).collect::<Vec<_>>(),
            vec![1, 2, 3, 3]
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn load_run_view_done_maps_reply_fields() {
        let root = temp("done");
        let mut graph = economy_graph();
        // 执行节点换成蜂群件：正文首行是件标题，产物带 diff 与 worktree。
        let exec = graph.nodes.get_mut("n-exec").unwrap();
        exec.user = Message {
            text: "改解析器\n文件范围：src/a/**".into(),
            attachments: vec![],
        };
        exec.swarm = Some(SwarmItem {
            item: 1,
            files: vec!["src/a/**".into()],
            accept: "cargo test".into(),
        });
        exec.reply = Some(Reply {
            status: "done".into(),
            text: "已改完".into(),
            artifacts: Some(ReplyArtifacts {
                diff: "+一行".into(),
                changed_files: vec![],
                worktree: Some("D:/p/.aicross-worktrees/n-exec".into()),
                ..ReplyArtifacts::default()
            }),
            ..Reply::default()
        });
        // 验收那一路额度用尽，带恢复时间与备选切换。
        let accept = graph.nodes.get_mut("n-accept").unwrap();
        accept.reply = Some(Reply {
            status: "quota_exhausted".into(),
            quota_resets_at: Some("2026-10-02T18:00:00Z".into()),
            fallback_from: Some("claude/sonnet".into()),
            fallback_to: Some("pi/glm".into()),
            ..Reply::default()
        });
        // 遗留：一条来自本 run 的验收，一条来自别的 run（同项目共用一张图）。
        graph.nodes.insert(
            "lo-mine".into(),
            GraphNode {
                id: "lo-mine".into(),
                leftover: Some(crate::orchestration::Leftover {
                    source_node_id: "n-accept".into(),
                    items: vec!["一条未闭合".into()],
                }),
                ..GraphNode::default()
            },
        );
        graph.nodes.insert(
            "lo-other".into(),
            GraphNode {
                id: "lo-other".into(),
                leftover: Some(crate::orchestration::Leftover {
                    source_node_id: "other-run-accept".into(),
                    items: vec!["别的 run 的遗留".into()],
                }),
                ..GraphNode::default()
            },
        );
        orchestration::save_graph_in(&root, &graph).unwrap();
        let run = EngineRun {
            run_id: "r1".into(),
            graph_id: "g1".into(),
            project: "D:/p".into(),
            status: "done".into(),
            nodes: vec![
                run_node("n-plan", "出方案"),
                run_node("n-exec", "执行"),
                run_node("n-accept", "验收"),
                run_node("n-verify", "验证"),
            ],
            leftover: vec!["一条未闭合".into(), "别的 run 的遗留".into()],
            started_at: "2026-10-01T08:00:00.000Z".into(),
            updated_at: "2026-10-01T08:01:00.000Z".into(),
            ..EngineRun::default()
        };
        write_run_file(&root, &run);
        let view = build_run_view(&root, "r1").unwrap();
        assert_eq!(view.status, "done");
        assert_eq!(view.leftover, vec!["一条未闭合"]);
        let exec = view.nodes.iter().find(|n| n.node_id == "n-exec").unwrap();
        assert_eq!(exec.swarm_title, "改解析器");
        assert_eq!(exec.artifacts.as_ref().unwrap().worktree, "D:/p/.aicross-worktrees/n-exec");
        assert_eq!(exec.artifacts.as_ref().unwrap().diff, "+一行");
        let accept = view.nodes.iter().find(|n| n.node_id == "n-accept").unwrap();
        assert_eq!(accept.status, "quota_exhausted");
        assert_eq!(accept.quota_resets_at, "2026-10-02T18:00:00Z");
        assert_eq!(accept.fallback_from, "claude/sonnet");
        assert_eq!(accept.fallback_to, "pi/glm");
        let plan = view.nodes.iter().find(|n| n.node_id == "n-plan").unwrap();
        assert_eq!(plan.model_reported, "claude-sonnet-4-6");
        assert_eq!(plan.tokens_in, Some(100));
        assert_eq!(plan.ms, Some(1234));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn load_run_view_failed_carries_reason_and_degrades_without_graph() {
        let root = temp("failed");
        // 旧 run 文件：不带 preset_id 字段（手写 JSON 模拟 R1-4 之前的产出）。
        std::fs::create_dir_all(root.join("runs")).unwrap();
        std::fs::write(
            root.join("runs/r2.json"),
            r#"{
  "run_id": "r2",
  "graph_id": "gone",
  "project": "D:/p",
  "status": "failed",
  "nodes": [
    {"node_id": "n1", "role": "执行", "agent": "claude", "model": "sonnet", "status": "done", "text_excerpt": "", "artifacts_dir": ""}
  ],
  "started_at": "2026-10-01T08:00:00.000Z",
  "reason": "引擎进程已退出",
  "pid": 0,
  "updated_at": "2026-10-01T08:00:30.000Z"
}"#,
        )
        .unwrap();
        let view = build_run_view(&root, "r2").unwrap();
        assert_eq!(view.status, "failed");
        assert_eq!(view.reason, "引擎进程已退出");
        // 图文件不存在：run 摘要仍可用，角色与目标来自 run 摘要；旧 run 没有
        // preset_id，模式名按「自定义模式」处理。
        assert_eq!(view.graph_title, "");
        assert_eq!(view.nodes.len(), 1);
        assert_eq!(view.nodes[0].role, "执行");
        assert_eq!(view.nodes[0].model, "sonnet");
        assert_eq!(view.nodes[0].has_result, false);
        assert_eq!(view.preset_name, "自定义模式");
        let _ = std::fs::remove_dir_all(root);
    }

    // ── adopt_run_node ───────────────────────────────────────────────────

    #[test]
    fn adopt_run_node_merges_graph_and_leaves_run_file_untouched() {
        let root = temp("adopt");
        let mut graph = economy_graph();
        // 同图上别的 run 的节点：合并写不许抹掉它的改动。
        let other = turn("other-run-node", "root", "顾问");
        graph.nodes.insert("other-run-node".into(), other);
        orchestration::save_graph_in(&root, &graph).unwrap();
        let run = EngineRun {
            run_id: "r1".into(),
            graph_id: "g1".into(),
            project: "D:/p".into(),
            status: "done".into(),
            nodes: vec![run_node("n-exec", "执行")],
            started_at: "2026-10-01T08:00:00.000Z".into(),
            updated_at: "2026-10-01T08:01:00.000Z".into(),
            ..EngineRun::default()
        };
        write_run_file(&root, &run);
        let run_bytes_before = std::fs::read(root.join("runs/r1.json")).unwrap();

        // adopt_run_node 的核心：采纳 → 重读整页。
        let view = adopt_view(&root, "r1", "n-exec").unwrap();
        let exec = view.nodes.iter().find(|n| n.node_id == "n-exec").unwrap();
        assert!(exec.adopted);

        let disk = orchestration::load_graph_in(&root, "g1").unwrap().unwrap();
        assert_eq!(
            disk.nodes["other-run-node"].reply.as_ref().unwrap().text,
            "正文 PASS"
        );
        // run 文件不动（采纳只改图）。
        assert_eq!(
            std::fs::read(root.join("runs/r1.json")).unwrap(),
            run_bytes_before
        );
        let _ = std::fs::remove_dir_all(root);
    }

    // ── 模式名（R1-4：按 preset_id 取，不再按角色序列推断） ─────────────

    #[test]
    fn preset_name_comes_from_preset_id() {
        let root = temp("preset");
        let presets = orchestration::raw_presets_from(&root);
        // 内置预设用自己的名字。
        assert_eq!(preset_display_name(&presets, "builtin-economy"), "经济模式");
        assert_eq!(preset_display_name(&presets, "builtin-blind"), "盲审模式");
        // id 为空（targets 直给、旧 run 文件）与清单里找不到的 id 都退「自定义模式」。
        assert_eq!(preset_display_name(&presets, ""), "自定义模式");
        assert_eq!(preset_display_name(&presets, "builtin-gone"), "自定义模式");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn preset_name_labels_user_presets_as_custom() {
        let root = temp("preset-user");
        // 用户自建预设：按命名规范显示「自定义模式 · 名称」；名称空白只留
        // 「自定义模式」；比内置同形状预设优先按 id 精确命中。
        let file = crate::orchestration::PresetFile {
            version: 1,
            presets: vec![
                Preset {
                    id: "u1".into(),
                    name: "预发回归".into(),
                    builtin: false,
                    stages: vec![Stage {
                        nodes: vec![Target {
                            role: "执行".into(),
                            ..Target::default()
                        }],
                    }],
                },
                Preset {
                    id: "u2".into(),
                    name: "  ".into(),
                    builtin: false,
                    stages: vec![Stage {
                        nodes: vec![Target::default()],
                    }],
                },
            ],
        };
        std::fs::write(
            root.join("presets.json"),
            serde_json::to_string_pretty(&file).unwrap(),
        )
        .unwrap();
        assert_eq!(preset_display_name(&orchestration::raw_presets_from(&root), "u1"), "自定义模式 · 预发回归");
        assert_eq!(preset_display_name(&orchestration::raw_presets_from(&root), "u2"), "自定义模式");
        let _ = std::fs::remove_dir_all(root);
    }

    // ── 冷启动拉起（R1-5）与存在性核对（R1-7） ─────────────────────────

    #[test]
    fn open_window_args_carry_open_and_run_id() {
        assert_eq!(
            open_window_args("run-1"),
            vec!["open".to_string(), "run-1".to_string()]
        );
    }

    #[test]
    fn spawn_open_window_with_spawns_and_marks_env_without_blocking() {
        // 注入的 spawn 立即返回：拉起入口本身不等窗口，只转交参数。
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let recorder = seen.clone();
        spawn_open_window_with(Path::new("D:/fake/aicross.exe"), "run-1", move |exe, args| {
            assert_eq!(exe, Path::new("D:/fake/aicross.exe"));
            *recorder.lock().unwrap() = args.to_vec();
            Ok(())
        })
        .expect("拉起成功");
        assert_eq!(
            seen.lock().unwrap().clone(),
            vec!["open".to_string(), "run-1".to_string()]
        );
        // 环境变量已置位：被拉起的窗口进程凭它不再拉起下一个。
        assert_eq!(std::env::var(OPEN_WINDOW_ENV).as_deref(), Ok("1"));
        std::env::remove_var(OPEN_WINDOW_ENV);
        // spawn 报错原样传出（调用方按退出码 3 处理）。
        let error = spawn_open_window_with(Path::new("D:/fake/aicross.exe"), "run-1", |_, _| {
            Err("启动窗口进程失败：模拟".into())
        })
        .unwrap_err();
        assert!(error.contains("启动窗口进程失败"));
    }

    #[test]
    fn open_run_exists_distinguishes_missing_from_other_errors() {
        let root = temp("exists");
        std::fs::create_dir_all(root.join("runs")).unwrap();
        // 文件不存在：Ok(false)，命令行按退出码 1。
        assert_eq!(open_run_exists(&root, "no-such-run").unwrap(), false);
        std::fs::write(root.join("runs/r1.json"), "{}").unwrap();
        assert_eq!(open_run_exists(&root, "r1").unwrap(), true);
        // 非法 id 是参数错，不是「找不到」。
        assert!(open_run_exists(&root, "bad/id").is_err());
        // 其它读取错误（如路径超长）：Err，命令行按退出码 3，不当「找不到」。
        // 注：Windows 上「runs 被同名文件占住」也报 NotFound，归入找不到。
        let long_id = "r".repeat(300);
        let error = open_run_exists(&root, &long_id).unwrap_err();
        assert!(error.contains("读取 run 失败"), "{error}");
        let _ = std::fs::remove_dir_all(root);
    }
    #[test]
    fn task_title_takes_first_nonempty_line_of_this_runs_request() {
        assert_eq!(task_title("\n  阅读 calc.py，说明 add 做什么\n只读"), "阅读 calc.py，说明 add 做什么");
        let long = "一".repeat(45);
        assert_eq!(task_title(&long), format!("{}…", "一".repeat(40)));
        assert_eq!(task_title(""), "");
    }

    #[test]
    fn leftover_is_scoped_to_this_runs_nodes() {
        use crate::orchestration::{GraphNode, Leftover};
        let mut graph = Graph::default();
        graph.nodes.insert(
            "lo-old".into(),
            GraphNode {
                id: "lo-old".into(),
                leftover: Some(Leftover { source_node_id: "old-accept".into(), items: vec!["别的 run 的遗留".into()] }),
                ..GraphNode::default()
            },
        );
        graph.nodes.insert(
            "lo-mine".into(),
            GraphNode {
                id: "lo-mine".into(),
                leftover: Some(Leftover { source_node_id: "my-accept".into(), items: vec!["本 run 的遗留".into()] }),
                ..GraphNode::default()
            },
        );
        let ids: std::collections::HashSet<String> = ["my-accept".to_string()].into_iter().collect();
        assert_eq!(run_leftover(&graph, &ids), vec!["本 run 的遗留".to_string()]);
    }
    // ── TASK-40 首页列表（list_runs_at）────────────────────────────

    fn run_summary(run_id: &str, updated_at: &str) -> crate::engine::EngineRun {
        crate::engine::EngineRun {
            run_id: run_id.into(),
            graph_id: "g1".into(),
            project: "D:/p".into(),
            status: "done".into(),
            nodes: Vec::new(),
            started_at: "2026-10-01T08:00:00Z".into(),
            reason: String::new(),
            pid: 0,
            pid_started_at: String::new(),
            original_request: "给解析器加测试\n顺便补文档".into(),
            accept_criteria: String::new(),
            preset_id: "builtin-economy".into(),
            leftover: Vec::new(),
            in_flight_node_ids: Vec::new(),
            updated_at: updated_at.into(),
        }
    }

    fn write_run(root: &std::path::Path, run_id: &str, updated_at: &str) {
        let runs = root.join("runs");
        std::fs::create_dir_all(&runs).unwrap();
        let value = serde_json::to_string(&run_summary(run_id, updated_at)).unwrap();
        std::fs::write(runs.join(format!("{run_id}.json")), value).unwrap();
    }

    #[test]
    fn list_runs_sorts_by_updated_at_desc_and_caps_at_50() {
        let root = temp("list-runs-sort");
        for index in 0..55 {
            let stamp = format!("2026-10-01T{:02}:{:02}:00Z", index / 60, index % 60);
            write_run(&root, &format!("run-{index:02}"), &stamp);
        }
        let items = list_runs_at(&root);
        assert_eq!(items.len(), 50);
        // 最新（updated_at 最大）在前。
        assert_eq!(items[0].run_id, "run-54");
        assert_eq!(items[49].run_id, "run-05");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn list_runs_only_counts_json_files_without_leading_dot() {
        let root = temp("list-runs-filter");
        write_run(&root, "run-a", "2026-10-01T08:00:00Z");
        let runs = root.join("runs");
        // 非 .json 与以 . 开头的文件一律不算。
        std::fs::write(runs.join("run-b.txt"), "x").unwrap();
        std::fs::write(runs.join(".run-c.json"), "x").unwrap();
        // json 扩展名大小写也只认小写 .json（与引擎 active_runs 同一口径）。
        std::fs::write(runs.join("run-d.JSON"), "x").unwrap();
        let items = list_runs_at(&root);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].run_id, "run-a");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn list_runs_skips_unparsable_files_and_uses_file_stem_when_run_id_missing() {
        let root = temp("list-runs-parse");
        let runs = root.join("runs");
        std::fs::create_dir_all(&runs).unwrap();
        std::fs::write(runs.join("broken.json"), "not json").unwrap();
        // 旧 run 文件 run_id 为空串：解析成功，run_id 退回文件名去扩展名。
        std::fs::write(
            runs.join("legacy.json"),
            r#"{"run_id":"","project":"D:/p","status":"done","started_at":"2026-10-01T08:00:00Z","pid":0,"updated_at":"2026-10-01T09:00:00Z"}"#,
        )
        .unwrap();
        let items = list_runs_at(&root);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].run_id, "legacy");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn list_runs_names_preset_and_task_like_the_viewer() {
        let root = temp("list-runs-naming");
        write_run(&root, "run-a", "2026-10-01T08:00:00Z");
        let items = list_runs_at(&root);
        assert_eq!(items[0].preset_name, "经济模式");
        assert_eq!(items[0].task_title, "给解析器加测试");
        assert_eq!(items[0].status, "done");
        // 任务名取第一行；空请求给空标题。
        let runs = root.join("runs");
        let mut empty = run_summary("run-b", "2026-10-01T07:00:00Z");
        empty.original_request = String::new();
        empty.preset_id = String::new();
        std::fs::write(runs.join("run-b.json"), serde_json::to_vec(&empty).unwrap()).unwrap();
        let items = list_runs_at(&root);
        let empty_item = items.iter().find(|i| i.run_id == "run-b").unwrap();
        assert_eq!(empty_item.preset_name, "自定义模式");
        assert_eq!(empty_item.task_title, "");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn list_runs_on_missing_dir_is_empty() {
        let root = temp("list-runs-empty");
        assert!(list_runs_at(&root).is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn list_runs_marks_dead_process_running_as_failed() {
        // fix-40-r1 R1-3：running 但 pid 已死 → 首页与查看页同口径显示失败，
        // 不把死 run 当成还在跑（pid 取值同引擎 dead_pid_becomes_failed）。
        let root = temp("list-runs-dead");
        let mut run = run_summary("run-dead", "2026-10-01T08:00:00Z");
        run.status = "running".into();
        run.pid = u32::MAX;
        let runs = root.join("runs");
        std::fs::create_dir_all(&runs).unwrap();
        std::fs::write(runs.join("run-dead.json"), serde_json::to_vec(&run).unwrap()).unwrap();
        let items = list_runs_at(&root);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, "failed");
        let _ = std::fs::remove_dir_all(root);
    }
}
