//! TASK-37 查看器第一版：`aicross open <run_id>` 的参数解析，以及
//! `load_run_view` / `adopt_run_node` 两条 Tauri 命令。
//!
//! 打开方式照 `launch` 的冷热两条路：冷启动经 `AppState::pending_open` 与
//! `take_pending_open`；热启动由单实例回调在这里重解析 argv 后向前端发
//! `open-run-request` 事件（见 server.rs）。打开查看页只读 run 文件与图
//! 文件，不派发、不改文件。
//!
//! 数据只从 TASK-36 的产出读：`~/.aicross/runs/<run_id>.json`（经
//! `engine::read_run`，会顺带完成死进程判定）加 `~/.aicross/graphs/` 里该
//! run 的图。页头要的模式名 run 文件里没有，按 run 摘要里的角色序列对预设
//! 清单做前缀匹配推断（`infer_preset_name`），推不出来显示「自定义模式」。

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

/// 与 `launch::parse_launch_args` 并列的解析：argv[0] 形状不定（裸名、完整
/// 路径或单实例转发），`open` 认 0 或 1 下标；`--flag` / `--flag=value`
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
    /// 推断出的模式名（「X 模式」/「自定义模式」）；run 被拒前没建节点时为空。
    pub preset_name: String,
    pub graph_title: String,
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

/// 读 run 文件与图文件，拼查看页数据。图读不出来（被删、解析失败）时降级：
/// run 摘要照常返回，正文与产物缺席。
pub fn build_run_view(root: &Path, run_id: &str) -> Result<RunView, String> {
    let run = crate::engine::read_run(root, run_id)?;
    let graph = if run.graph_id.is_empty() {
        None
    } else {
        crate::orchestration::load_graph_in(root, &run.graph_id)
            .ok()
            .flatten()
    };
    let depths = graph.as_ref().map(crate::engine::depth_map);
    let roles: Vec<String> = run.nodes.iter().map(|n| n.role.clone()).collect();
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
    Ok(RunView {
        run_id: run.run_id.clone(),
        graph_id: run.graph_id.clone(),
        project: run.project.clone(),
        preset_name: infer_preset_name(root, &roles),
        graph_title: graph
            .as_ref()
            .map(|g| g.title.clone())
            .unwrap_or_default(),
        status: run.status.clone(),
        reason: run.reason.clone(),
        started_at: run.started_at.clone(),
        updated_at: run.updated_at.clone(),
        leftover: run.leftover.clone(),
        nodes,
    })
}

/// 模式名推断：run 文件与图上都没有预设记录（TASK-36 的产出），这里按 run
/// 摘要里的角色序列（盖章插入序）对预设清单做前缀匹配——修正轮补的节点排在
/// 初始清单之后，不影响前缀。取最长匹配；同长时内置预设优先；推不出来按
/// 命名规范显示「自定义模式」。run 没建节点（派发前被拒）时返回空串，页面
/// 不显示模式名。
fn infer_preset_name(root: &Path, roles: &[String]) -> String {
    if roles.is_empty() {
        return String::new();
    }
    let mut best: Option<(usize, bool, String)> = None; // (角色数, 是否内置, 显示名)
    for preset in &orchestration::raw_presets_from(root).presets {
        let sequence: Vec<&str> = preset
            .stages
            .iter()
            .flat_map(|stage| stage.nodes.iter())
            .map(|t| t.role.trim())
            .collect();
        if sequence.is_empty() || sequence.len() > roles.len() {
            continue;
        }
        let matched = roles
            .iter()
            .take(sequence.len())
            .zip(sequence.iter())
            .all(|(role, preset_role)| role.trim() == *preset_role);
        if !matched {
            continue;
        }
        let display = if preset.builtin {
            preset.name.clone()
        } else if preset.name.trim().is_empty() {
            "自定义模式".into()
        } else {
            format!("自定义模式 · {}", preset.name.trim())
        };
        let better = match &best {
            None => true,
            Some((length, builtin, _)) => {
                sequence.len() > *length || (sequence.len() == *length && preset.builtin && !*builtin)
            }
        };
        if better {
            best = Some((sequence.len(), preset.builtin, display));
        }
    }
    best.map(|(_, _, display)| display)
        .unwrap_or_else(|| "自定义模式".into())
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
        let dir = std::env::temp_dir().join(format!("aicross-view-{name}-{}", Uuid::new_v4()));
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
    fn load_run_view_running_keeps_status_and_infer_mode() {
        let root = temp("running");
        let graph = economy_graph();
        orchestration::save_graph_in(&root, &graph).unwrap();
        let run = EngineRun {
            run_id: "r1".into(),
            graph_id: "g1".into(),
            project: "D:/p".into(),
            status: "running".into(),
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
            leftover: vec!["一条未闭合".into()],
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
        let run = EngineRun {
            run_id: "r2".into(),
            graph_id: "gone".into(),
            project: "D:/p".into(),
            status: "failed".into(),
            nodes: vec![run_node("n1", "执行")],
            reason: "引擎进程已退出".into(),
            started_at: "2026-10-01T08:00:00.000Z".into(),
            updated_at: "2026-10-01T08:00:30.000Z".into(),
            ..EngineRun::default()
        };
        write_run_file(&root, &run);
        let view = build_run_view(&root, "r2").unwrap();
        assert_eq!(view.status, "failed");
        assert_eq!(view.reason, "引擎进程已退出");
        // 图文件不存在：run 摘要仍可用，角色与目标来自 run 摘要；模式推断只看
        // 角色序列，这一路单「执行」照共享模式推断。
        assert_eq!(view.graph_title, "");
        assert_eq!(view.nodes.len(), 1);
        assert_eq!(view.nodes[0].role, "执行");
        assert_eq!(view.nodes[0].model, "sonnet");
        assert_eq!(view.nodes[0].has_result, false);
        assert_eq!(view.preset_name, "共享模式");
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

    // ── 模式名推断 ───────────────────────────────────────────────────────

    #[test]
    fn infer_preset_name_matches_builtin_shapes_and_falls_back() {
        let root = temp("preset");
        let cases: &[(&[&str], &str)] = &[
            (&["出方案", "执行", "验收", "验证"], "经济模式"),
            // 修正轮补的执行与验收排在初始清单之后，前缀仍成立。
            (&["出方案", "执行", "验收", "验证", "执行", "验收"], "经济模式"),
            (&["验收", "验证", "验证"], "盲审模式"),
            (&["执行", "执行", "执行"], "擂台模式"),
            (&["验证", "验证", "验证"], "评审模式"),
            (&["拆分"], "蜂群模式"),
            (&["执行"], "共享模式"),
            // targets 直给 / 对不上任何预设：自定义模式。
            (&["验证", "验证"], "自定义模式"),
        ];
        for (roles, expected) in cases {
            let roles: Vec<String> = roles.iter().map(|r| r.to_string()).collect();
            assert_eq!(&infer_preset_name(&root, &roles), expected, "{roles:?}");
        }
        assert_eq!(infer_preset_name(&root, &[]), "");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn infer_preset_name_prefers_longest_match_then_builtin() {
        let root = temp("preset-user");
        // 用户预设 [执行, 验收]（两层）比内置共享 [执行] 长：匹配更长者胜。
        let file = crate::orchestration::PresetFile {
            version: 1,
            presets: vec![Preset {
                id: "u1".into(),
                name: "预发回归".into(),
                builtin: false,
                stages: vec![
                    Stage {
                        nodes: vec![Target {
                            role: "执行".into(),
                            ..Target::default()
                        }],
                    },
                    Stage {
                        nodes: vec![Target {
                            role: "验收".into(),
                            ..Target::default()
                        }],
                    },
                ],
            }],
        };
        std::fs::write(
            root.join("presets.json"),
            serde_json::to_string_pretty(&file).unwrap(),
        )
        .unwrap();
        let two: Vec<String> = vec!["执行".into(), "验收".into()];
        assert_eq!(infer_preset_name(&root, &two), "自定义模式 · 预发回归");
        // 只有 [执行] 时用户预设两段对不上，内置共享胜出。
        let one: Vec<String> = vec!["执行".into()];
        assert_eq!(infer_preset_name(&root, &one), "共享模式");
        let _ = std::fs::remove_dir_all(root);
    }
}
