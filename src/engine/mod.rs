//! 无窗口派发编排。纯图变换与文件/进程边界集中在本模块。

use crate::dispatch::{
    self, DispatchArtifacts, DispatchNodeRequest, DispatchNodeResult, DispatchSettings,
    DispatchStageRequest, FallbackSpec, PinnedState,
};
use crate::orchestration::{
    self, Graph, GraphNode, Leftover, Message, Preset, Reply, ReplyArtifacts, RoleSettings, Target,
};
use crate::resources::ResourceModel;
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

const DUPLICATE_BATCH_TEXT: &str = "同一批结果重复送达，已停止推进；这一层的节点已放回待派。";
const MISSING_TARGET_TEXT: &str = "下一阶段有节点未指定模型，请先选择目标。";
const NO_EXTERNAL_ACTIONS: &str =
    "不得 git push、发布、安装依赖或调用付费接口；这些留给用户在验收之后做。";
const VERIFICATION_ROUTE_TEXT: &str =
    "盲审：独立审查同一份产物，只看材料与验证命令输出，不看验收结论。";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EngineSettings {
    pub timeout_secs: u64,
    pub concurrency: usize,
    pub max_requests: usize,
    pub classification_model: String,
    pub classify_proposals: bool,
    pub swarm_limit: usize,
    pub console_run_limit: usize,
    pub redact: bool,
    pub write_project_dispatch: bool,
    pub cc_switch_path: String,
    pub validation_command: String,
    pub acp_enabled: bool,
    pub fix_round_limit: usize,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            timeout_secs: 600,
            concurrency: 3,
            max_requests: 8,
            classification_model: String::new(),
            classify_proposals: false,
            swarm_limit: 4,
            console_run_limit: 3,
            redact: true,
            write_project_dispatch: true,
            cc_switch_path: String::new(),
            validation_command: String::new(),
            acp_enabled: false,
            fix_round_limit: 1,
        }
    }
}

impl EngineSettings {
    fn dispatch(&self) -> DispatchSettings {
        DispatchSettings {
            timeout_secs: self.timeout_secs,
            concurrency: self.concurrency,
            redact: self.redact,
            write_project_dispatch: self.write_project_dispatch,
            cc_switch_path: self.cc_switch_path.clone(),
            validation_command: self.validation_command.clone(),
            max_requests: self.max_requests,
            swarm_limit: self.swarm_limit,
            acp_enabled: self.acp_enabled,
            fix_round_limit: self.fix_round_limit,
        }
    }
}

fn finite_number(value: Option<&Value>, default: f64) -> f64 {
    value
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite())
        .unwrap_or(default)
}

pub(crate) fn load_settings(root: &Path) -> EngineSettings {
    let Ok(bytes) = fs::read(root.join("dispatch-settings.json")) else {
        return EngineSettings::default();
    };
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        return EngineSettings::default();
    };
    let Some(map) = value.as_object() else {
        return EngineSettings::default();
    };
    let string = |key: &str| {
        map.get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    EngineSettings {
        timeout_secs: finite_number(map.get("timeoutSecs"), 600.0).clamp(1.0, 3600.0) as u64,
        concurrency: finite_number(map.get("concurrency"), 3.0).clamp(1.0, 3.0) as usize,
        max_requests: finite_number(map.get("maxRequests"), 8.0).clamp(1.0, 100.0) as usize,
        classification_model: string("classificationModel"),
        classify_proposals: map.get("classifyProposals").and_then(Value::as_bool) == Some(true),
        swarm_limit: finite_number(map.get("swarmLimit"), 4.0).clamp(1.0, 4.0) as usize,
        console_run_limit: finite_number(map.get("consoleRunLimit"), 3.0).clamp(1.0, 3.0) as usize,
        redact: map.get("redact").and_then(Value::as_bool) != Some(false),
        write_project_dispatch: map.get("writeProjectDispatch").and_then(Value::as_bool)
            != Some(false),
        cc_switch_path: string("ccSwitchPath"),
        validation_command: string("validationCommand"),
        acp_enabled: map.get("acpEnabled").and_then(Value::as_bool) == Some(true),
        fix_round_limit: finite_number(map.get("fixRoundLimit"), 1.0)
            .floor()
            .clamp(0.0, 9.0) as usize,
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct RunNode {
    pub node_id: String,
    pub role: String,
    pub agent: String,
    pub model: String,
    pub status: String,
    pub text_excerpt: String,
    pub artifacts_dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota_resets_at: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EngineRun {
    pub run_id: String,
    #[serde(default)]
    pub graph_id: String,
    pub project: String,
    pub status: String,
    #[serde(default)]
    pub nodes: Vec<RunNode>,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub leftover: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub in_flight_node_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
    pub pid: u32,
    pub updated_at: String,
}

fn runs_dir(root: &Path) -> PathBuf {
    root.join("runs")
}
fn run_path(root: &Path, run_id: &str) -> PathBuf {
    runs_dir(root).join(format!("{run_id}.json"))
}
fn cancel_path(root: &Path, run_id: &str) -> PathBuf {
    runs_dir(root).join(format!("{run_id}.cancel"))
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

pub(crate) fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let dir = path
        .parent()
        .ok_or_else(|| "写入目标没有父目录".to_string())?;
    fs::create_dir_all(dir).map_err(|e| format!("创建目录 {} 失败：{e}", dir.display()))?;
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| format!("生成 JSON 失败：{e}"))?;
    let tmp = dir.join(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        Uuid::new_v4()
    ));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&tmp)
            .map_err(|e| format!("创建临时文件失败：{e}"))?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("写入临时文件失败：{e}"))?;
        replace_file(&tmp, path).map_err(|e| format!("替换 {} 失败：{e}", path.display()))
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}

#[cfg(not(windows))]
fn replace_file(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(source, target)
}

#[cfg(windows)]
fn replace_file(source: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(target.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))
}

pub(crate) fn read_run(root: &Path, run_id: &str) -> Result<EngineRun, String> {
    if !valid_id(run_id) {
        return Err("run_id 无效".into());
    }
    let path = run_path(root, run_id);
    let mut run: EngineRun =
        serde_json::from_slice(&fs::read(&path).map_err(|e| format!("读取 run 失败：{e}"))?)
            .map_err(|e| format!("解析 run 失败：{e}"))?;
    if run.status == "running" && !process_alive(run.pid) {
        run.status = "failed".into();
        run.reason = "引擎进程已退出".into();
        run.updated_at = now();
        write_json_atomic(&path, &run)?;
    }
    Ok(run)
}

fn write_run(root: &Path, run: &EngineRun) -> Result<(), String> {
    write_json_atomic(&run_path(root, &run.run_id), run)
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let result = unsafe { libc::kill(pid as i32, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(windows)]
fn process_alive(pid: u32) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    if pid == 0 {
        return false;
    }
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else {
        return false;
    };
    unsafe {
        let _ = CloseHandle(handle);
    }
    true
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn utf16_excerpt(text: &str, limit: usize) -> String {
    // JS slice 按 UTF-16 码元截断；若边界切中代理对，Rust String 无法承载孤立代理，
    // 这里用 U+FFFD 表示该半个码元，JSON 语义与 JS 序列化后的替换结果一致。
    String::from_utf16_lossy(&text.encode_utf16().take(limit).collect::<Vec<_>>())
}

pub fn node_summary(graph: &Graph, ids: impl IntoIterator<Item = String>) -> Vec<RunNode> {
    let mut seen = HashSet::new();
    ids.into_iter()
        .filter(|id| seen.insert(id.clone()))
        .filter_map(|id| {
            let node = graph.nodes.get(&id)?;
            let target = node.target.as_ref()?;
            let reply = node.reply.as_ref();
            let text = reply
                .map(|r| {
                    if r.text.is_empty() {
                        r.skipped_reason.as_deref().unwrap_or("")
                    } else {
                        &r.text
                    }
                })
                .unwrap_or("");
            Some(RunNode {
                node_id: id,
                role: target.role.clone(),
                agent: target.agent.clone(),
                model: target.model.clone(),
                status: reply
                    .map(|r| r.status.clone())
                    .unwrap_or_else(|| "draft".into()),
                text_excerpt: utf16_excerpt(text, 160),
                artifacts_dir: reply
                    .and_then(|r| r.artifacts.as_ref())
                    .and_then(|a| a.worktree.clone())
                    .unwrap_or_default(),
                quota_resets_at: reply.and_then(|r| r.quota_resets_at.clone()),
            })
        })
        .collect()
}

pub fn new_graph(project: &str, id: String, root_id: String, time: String) -> Graph {
    let root = GraphNode {
        id: root_id.clone(),
        kind: "material".into(),
        parents: vec![],
        user: Message {
            text: String::new(),
            attachments: vec![],
        },
        created_at: time.clone(),
        ..GraphNode::default()
    };
    Graph {
        version: 1,
        id,
        root: root_id.clone(),
        nodes: HashMap::from([(root_id, root)]),
        created_at: time.clone(),
        updated_at: time,
        project: project.into(),
        ..Graph::default()
    }
}

fn latest_leaf(graph: &Graph) -> String {
    let parents: HashSet<&str> = graph
        .nodes
        .values()
        .flat_map(|n| n.parents.iter().map(String::as_str))
        .collect();
    graph
        .nodes
        .values()
        .filter(|n| !parents.contains(n.id.as_str()))
        .max_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)))
        .map(|n| n.id.clone())
        .unwrap_or_else(|| graph.root.clone())
}

pub fn append_material(
    graph: &Graph,
    text: &str,
    attachments: &[String],
    id: String,
    time: String,
) -> (Graph, String) {
    let mut next = graph.clone();
    if let Some(root) = next.nodes.get_mut(&next.root) {
        if root.user.text.is_empty() && root.parents.is_empty() {
            root.user = Message {
                text: text.into(),
                attachments: attachments.to_vec(),
            };
            root.source = "console".into();
            if next.title.is_empty() {
                next.title = text.lines().next().unwrap_or("").chars().take(20).collect();
            }
            next.updated_at = time;
            return (next, graph.root.clone());
        }
    }
    let node = GraphNode {
        id: id.clone(),
        kind: "material".into(),
        parents: vec![latest_leaf(graph)],
        user: Message {
            text: text.into(),
            attachments: attachments.to_vec(),
        },
        created_at: time,
        source: "console".into(),
        ..GraphNode::default()
    };
    next.nodes.insert(id.clone(), node);
    (next, id)
}

fn preset_text(preset: &Preset, index: usize, target: &Target, swarm_limit: usize) -> String {
    if preset.builtin {
        if preset.id == "builtin-economy" && index == 3 {
            return VERIFICATION_ROUTE_TEXT.into();
        }
        if preset.id == "builtin-blind" && index > 0 {
            return VERIFICATION_ROUTE_TEXT.into();
        }
        if preset.id == "builtin-jury" {
            return [
                "安全评审：只从安全视角审查这批材料，指出风险、暴露面与缓释建议。",
                "性能评审：只从性能视角审查这批材料，指出瓶颈、复杂度与改进建议。",
                "",
            ]
            .get(index)
            .unwrap_or(&"")
            .to_string();
        }
    }
    if target.role == "拆分" && !target.preset_text.is_empty() {
        format!("{}\n\n本次最多拆分 {swarm_limit} 件。", target.preset_text)
    } else {
        target.preset_text.clone()
    }
}

pub fn stamp_with(
    graph: &Graph,
    node_id: &str,
    preset: &Preset,
    swarm_limit: usize,
    ids: &mut dyn Iterator<Item = String>,
    base: DateTime<Utc>,
) -> (Graph, Vec<String>) {
    if !graph.nodes.contains_key(node_id) {
        return (graph.clone(), vec![]);
    }
    let mut next = graph.clone();
    let mut previous = vec![node_id.to_string()];
    let mut created = vec![];
    let mut flat_index = 0usize;
    for stage in &preset.stages {
        let mut level = vec![];
        for (index, target) in stage.nodes.iter().enumerate() {
            let Some(id) = ids.next() else {
                return (next, created);
            };
            let created_at = (base + chrono::Duration::milliseconds(index as i64))
                .to_rfc3339_opts(SecondsFormat::Millis, true);
            let node = GraphNode {
                id: id.clone(),
                kind: if previous.len() >= 2 {
                    "accept".into()
                } else {
                    "turn".into()
                },
                parents: previous.clone(),
                user: Message {
                    text: preset_text(preset, flat_index, target, swarm_limit),
                    attachments: vec![],
                },
                reply: Some(Reply {
                    status: "draft".into(),
                    ..Reply::default()
                }),
                target: Some(target.clone()),
                created_at,
                source: "console".into(),
                ..GraphNode::default()
            };
            next.nodes.insert(id.clone(), node);
            level.push(id.clone());
            created.push(id);
            flat_index += 1;
        }
        previous = level;
    }
    next.updated_at = base.to_rfc3339_opts(SecondsFormat::Millis, true);
    (next, created)
}

fn apply_role_defaults(mut preset: Preset, roles: &RoleSettings) -> Preset {
    if matches!(
        preset.id.as_str(),
        "builtin-arena" | "builtin-blind" | "builtin-jury"
    ) {
        return preset;
    }
    for target in preset.stages.iter_mut().flat_map(|s| s.nodes.iter_mut()) {
        if !target.model.trim().is_empty() {
            continue;
        }
        let profile = match target.role.as_str() {
            "出方案" | "拆分" => &roles.planning,
            "执行" => &roles.execution,
            "验收" | "验证" => &roles.acceptance,
            _ => continue,
        };
        if profile.model.trim().is_empty() {
            continue;
        }
        target.agent.clone_from(&profile.agent);
        target.provider = (!profile.provider.is_empty()).then(|| profile.provider.clone());
        target.model.clone_from(&profile.model);
        target.tier = (!profile.tier.is_empty()).then(|| profile.tier.clone());
        if !profile.thinking.is_empty() {
            target.thinking.clone_from(&profile.thinking);
        }
    }
    preset
}

fn sorted_nodes(graph: &Graph) -> Vec<&GraphNode> {
    let mut nodes: Vec<_> = graph.nodes.values().collect();
    nodes.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
    nodes
}

fn dispatchable_ids(graph: &Graph) -> Vec<String> {
    sorted_nodes(graph)
        .into_iter()
        .filter(|node| {
            node.leftover.is_none()
                && node.target.is_some()
                && node.reply.as_ref().is_some_and(|r| r.status == "draft")
                && node.parents.iter().all(|id| {
                    graph.nodes.get(id).is_some_and(|p| {
                        p.kind == "material" || p.reply.as_ref().is_some_and(|r| r.status == "done")
                    })
                })
        })
        .map(|n| n.id.clone())
        .collect()
}

fn ancestors(graph: &Graph, node_id: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut stack = graph
        .nodes
        .get(node_id)
        .map(|n| n.parents.clone())
        .unwrap_or_default();
    while let Some(id) = stack.pop() {
        if out.insert(id.clone()) {
            if let Some(node) = graph.nodes.get(&id) {
                stack.extend(node.parents.clone());
            }
        }
    }
    out
}

fn graph_stage(graph: &Graph, node_id: &str) -> u32 {
    fn visit(graph: &Graph, id: &str, seen: &mut HashSet<String>) -> u32 {
        if !seen.insert(id.into()) {
            return 0;
        }
        let Some(node) = graph.nodes.get(id) else {
            return 0;
        };
        if node.parents.is_empty() {
            return 0;
        }
        1 + node
            .parents
            .iter()
            .map(|p| visit(graph, p, &mut seen.clone()))
            .max()
            .unwrap_or(0)
    }
    visit(graph, node_id, &mut HashSet::new())
}

fn context_order(graph: &Graph, node_id: &str) -> Vec<String> {
    let mut keep = ancestors(graph, node_id);
    keep.insert(node_id.into());
    let mut out = vec![];
    let mut seen = HashSet::new();
    fn visit(
        graph: &Graph,
        id: &str,
        keep: &HashSet<String>,
        seen: &mut HashSet<String>,
        out: &mut Vec<String>,
    ) {
        if !keep.contains(id) || !seen.insert(id.into()) {
            return;
        }
        out.push(id.into());
        let mut children: Vec<_> = graph
            .nodes
            .values()
            .filter(|n| n.parents.iter().any(|p| p == id))
            .collect();
        children.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
        for child in children {
            visit(graph, &child.id, keep, seen, out);
        }
    }
    if graph.nodes.contains_key(&graph.root) {
        visit(graph, &graph.root, &keep, &mut seen, &mut out);
    }
    if !seen.contains(node_id) {
        out.push(node_id.into());
    }
    out
}

fn artifact_summary(value: &ReplyArtifacts) -> String {
    let files = if value.changed_files.is_empty() {
        "无改后全文".into()
    } else {
        value
            .changed_files
            .iter()
            .map(|f| f.path.as_str())
            .collect::<Vec<_>>()
            .join("、")
    };
    let worktree = value
        .worktree
        .as_ref()
        .map(|v| format!("\nworktree: {v}"))
        .unwrap_or_default();
    format!(
        "[执行产物]\n改动文件：{files}{worktree}\nstdout 摘要：{}\n\n{}",
        if value.stdout_summary.is_empty() {
            "—"
        } else {
            &value.stdout_summary
        },
        value.diff
    )
}

fn serialize_context(graph: &Graph, node_id: &str) -> (String, Vec<String>) {
    let Some(current) = graph.nodes.get(node_id) else {
        return (String::new(), vec![]);
    };
    let visibility = current
        .target
        .as_ref()
        .map(|t| t.visibility.as_str())
        .unwrap_or("共享");
    let upstreams: HashSet<&str> = current
        .target
        .as_ref()
        .and_then(|t| t.upstreams.as_ref())
        .into_iter()
        .flatten()
        .map(String::as_str)
        .collect();
    let parents: HashSet<&str> = current.parents.iter().map(String::as_str).collect();
    let mut blocks = vec![];
    let mut materials = vec![];
    for id in context_order(graph, node_id) {
        let Some(node) = graph.nodes.get(&id) else {
            continue;
        };
        let keep_user =
            visibility != "指定上游" || id == node_id || upstreams.contains(id.as_str());
        let default_selected =
            !(visibility == "盲" && id != node_id && !parents.contains(id.as_str()));
        if keep_user && default_selected {
            if !node.user.text.is_empty() {
                blocks.push(format!("[用户]\n{}", node.user.text));
            }
            if !node.user.attachments.is_empty() {
                materials.extend(node.user.attachments.clone());
                let shown = if visibility == "盲" {
                    node.user
                        .attachments
                        .iter()
                        .map(|p| {
                            Path::new(p)
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned()
                        })
                        .collect::<Vec<_>>()
                } else {
                    node.user.attachments.clone()
                };
                blocks.push(format!("[附件]\n{}", shown.join("\n")));
            }
        }
        let Some(reply) = node
            .reply
            .as_ref()
            .filter(|r| r.status == "done" && !r.text.is_empty())
        else {
            continue;
        };
        if visibility == "盲" && parents.contains(id.as_str()) && reply.artifacts.is_some() {
            continue;
        }
        let keep_reply = visibility == "共享"
            || (visibility == "盲" && parents.contains(id.as_str()))
            || (visibility == "指定上游" && upstreams.contains(id.as_str()));
        if keep_reply {
            let route = node
                .target
                .as_ref()
                .map(|t| {
                    [t.agent.as_str(), t.model.as_str()]
                        .into_iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join("/")
                })
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "模型".into());
            let text = if let Some(a) = &reply.artifacts {
                format!("{}\n\n{}", reply.text, artifact_summary(a))
            } else {
                reply.text.clone()
            };
            blocks.push(format!("[模型回复 {route}]\n{text}"));
        }
    }
    materials.sort();
    materials.dedup();
    let prompt = if visibility == "盲" {
        std::iter::once(
            include_str!("../../fixtures/prompts/blind-prefix.txt")
                .trim()
                .to_string(),
        )
        .chain(blocks)
        .collect::<Vec<_>>()
        .join("\n\n")
    } else {
        blocks.join("\n\n")
    };
    (prompt, materials)
}

fn current_message(message: &Message) -> String {
    let mut blocks = vec![];
    if !message.text.is_empty() {
        blocks.push(format!("[用户]\n{}", message.text));
    }
    if !message.attachments.is_empty() {
        blocks.push(format!("[附件]\n{}", message.attachments.join("\n")));
    }
    blocks.join("\n\n")
}

fn upstream_artifacts(graph: &Graph, node_id: &str) -> Vec<DispatchArtifacts> {
    let Some(node) = graph.nodes.get(node_id) else {
        return vec![];
    };
    let allowed: HashSet<String> = match node
        .target
        .as_ref()
        .map(|t| t.visibility.as_str())
        .unwrap_or("共享")
    {
        "盲" => node.parents.iter().cloned().collect(),
        "指定上游" => node
            .target
            .as_ref()
            .and_then(|t| t.upstreams.clone())
            .unwrap_or_default()
            .into_iter()
            .collect(),
        _ => ancestors(graph, node_id),
    };
    context_order(graph, node_id)
        .into_iter()
        .filter(|id| allowed.contains(id))
        .filter_map(|id| graph.nodes.get(&id)?.reply.as_ref()?.artifacts.as_ref())
        .map(|a| DispatchArtifacts {
            source_node_id: a.source_node_id.clone().unwrap_or_default(),
            diff: a.diff.clone(),
            changed_files: a
                .changed_files
                .iter()
                .map(|f| dispatch::ChangedFile {
                    path: f.path.clone(),
                    content: f.content.clone(),
                })
                .collect(),
            worktree: a.worktree.clone(),
            stdout_summary: a.stdout_summary.clone(),
            before_dir: String::new(),
        })
        .collect()
}

fn continuation(graph: &Graph, node_id: &str) -> (String, String) {
    let Some(node) = graph.nodes.get(node_id) else {
        return (String::new(), String::new());
    };
    let Some(target) = node.target.as_ref() else {
        return (String::new(), String::new());
    };
    if target.visibility != "共享" || node.parents.len() != 1 {
        return (String::new(), String::new());
    }
    let Some(parent) = graph.nodes.get(&node.parents[0]) else {
        return (String::new(), String::new());
    };
    let (Some(pt), Some(reply)) = (parent.target.as_ref(), parent.reply.as_ref()) else {
        return (String::new(), String::new());
    };
    if pt.visibility == "盲"
        || !reply.fallback_from.as_deref().unwrap_or("").is_empty()
        || pt.agent != target.agent
        || pt.provider != target.provider
        || pt.model != target.model
    {
        return (String::new(), String::new());
    }
    let session = reply.session_id.clone().unwrap_or_default();
    let runner = if session.is_empty() {
        String::new()
    } else {
        reply
            .runner
            .clone()
            .filter(|v| matches!(v.as_str(), "acp" | "oneshot"))
            .unwrap_or_default()
    };
    (session, runner)
}

fn role_prompt(role: &str, prompt: &str) -> String {
    let template = match role {
        "执行" => Some(include_str!("../../fixtures/prompts/execution.txt").trim()),
        "验收" => Some(include_str!("../../fixtures/prompts/acceptance.txt").trim()),
        "出方案" => Some(include_str!("../../fixtures/prompts/planning.txt").trim()),
        "顾问" => Some(include_str!("../../fixtures/prompts/advisor.txt").trim()),
        _ => None,
    };
    match (role, template) {
        ("执行", Some(t)) => format!("{t}\n\n{prompt}"),
        (_, Some(t)) => format!("{prompt}\n\n{t}"),
        _ => prompt.into(),
    }
}

fn build_request(
    run_id: &str,
    project: &str,
    settings: &EngineSettings,
    graph: &Graph,
    ids: &[String],
    models: &[ResourceModel],
    original: &str,
    fix_ids: &HashSet<String>,
) -> DispatchStageRequest {
    let stage = ids
        .iter()
        .map(|id| graph_stage(graph, id))
        .min()
        .unwrap_or(1);
    let nodes = ids
        .iter()
        .filter_map(|id| {
            let node = graph.nodes.get(id)?;
            let target = node.target.as_ref()?;
            let model = models
                .iter()
                .find(|m| {
                    m.agent == target.agent
                        && m.id == target.model
                        && m.provider == target.provider.clone().unwrap_or_default()
                })
                .or_else(|| {
                    models
                        .iter()
                        .find(|m| m.agent == target.agent && m.id == target.model)
                });
            let (context, materials) = serialize_context(graph, id);
            let mut prompt = role_prompt(&target.role, &context);
            if target.write != "只读" && !prompt.contains(NO_EXTERNAL_ACTIONS) {
                prompt.push_str("\n\n");
                prompt.push_str(NO_EXTERNAL_ACTIONS);
            }
            let fallback = target
                .fallback
                .as_ref()
                .filter(|f| !f.model.is_empty())
                .map(|f| FallbackSpec {
                    agent: f.agent.clone(),
                    provider: f.provider.clone().unwrap_or_default(),
                    model: f.model.clone(),
                    vendor: models
                        .iter()
                        .find(|m| m.agent == f.agent && m.id == f.model)
                        .map(|m| m.vendor.clone())
                        .unwrap_or_default(),
                });
            let (session_id, previous_runner) = continuation(graph, id);
            Some(DispatchNodeRequest {
                node_id: id.clone(),
                stage,
                agent: target.agent.clone(),
                provider: target.provider.clone().unwrap_or_default(),
                model: target.model.clone(),
                vendor: model.map(|m| m.vendor.clone()).unwrap_or_default(),
                billing_source: model.map(|m| m.billing_source.clone()).unwrap_or_default(),
                role: target.role.clone(),
                tier: target
                    .tier
                    .clone()
                    .or_else(|| model.map(|m| m.tier.clone()))
                    .unwrap_or_default(),
                thinking: target.thinking.clone(),
                visibility: target.visibility.clone(),
                prompt,
                current_message: current_message(&node.user),
                materials,
                session_id,
                previous_runner,
                write: target.write.clone(),
                original_request: original.into(),
                upstream_artifacts: upstream_artifacts(graph, id),
                fallback,
                rollback_before: false,
                review_material_validation: target.role == "验证" && target.visibility == "盲",
                fix_round: fix_ids.contains(id),
                interactive: false,
            })
        })
        .collect();
    DispatchStageRequest {
        run_id: run_id.into(),
        stage,
        project_dir: project.into(),
        nodes,
        settings: settings.dispatch(),
    }
}

fn mark_pending(graph: &Graph, ids: &[String]) -> Graph {
    let mut next = graph.clone();
    for id in ids {
        if let Some(n) = next.nodes.get_mut(id) {
            let adopted = n.reply.as_ref().and_then(|r| r.adopted);
            n.reply = Some(Reply {
                status: "pending".into(),
                adopted,
                ..Reply::default()
            });
        }
    }
    next
}
fn reset_draft(graph: &Graph, ids: &[String]) -> Graph {
    let mut next = graph.clone();
    for id in ids {
        if let Some(n) = next.nodes.get_mut(id) {
            n.reply = Some(Reply {
                status: "draft".into(),
                ..Reply::default()
            });
        }
    }
    next
}

fn apply_results(graph: &Graph, results: &[DispatchNodeResult]) -> Graph {
    let mut next = graph.clone();
    for result in results {
        let Some(node) = next.nodes.get_mut(&result.node_id) else {
            continue;
        };
        let ok = result.status == "ok";
        let mut body =
            if result.status == "validation_failed" && !result.validation_output.is_empty() {
                format!(
                    "{}{}[验证输出]\n{}",
                    result.text,
                    if result.text.is_empty() { "" } else { "\n\n" },
                    result.validation_output
                )
            } else if ok {
                result.text.clone()
            } else if result.text.is_empty() {
                result.error.clone()
            } else {
                result.text.clone()
            };
        let pin = match result.pinned {
            Some(PinnedState::Pinned) => "测试已钉住".into(),
            Some(PinnedState::Unpinned) => format!(
                "测试未钉住{}",
                result
                    .pinned_note
                    .as_ref()
                    .map(|v| format!("：{v}"))
                    .unwrap_or_default()
            ),
            Some(PinnedState::Skipped) => format!(
                "未核对{}",
                result
                    .pinned_note
                    .as_ref()
                    .map(|v| format!("：{v}"))
                    .unwrap_or_default()
            ),
            None => String::new(),
        };
        if !pin.is_empty() {
            if !body.is_empty() {
                body.push_str("\n\n")
            }
            body.push_str("[红绿核对]\n");
            body.push_str(&pin)
        }
        let old_adopted = node.reply.as_ref().and_then(|r| r.adopted);
        node.reply = Some(Reply {
            status: if ok {
                "done"
            } else if result.status == "quota_exhausted" {
                "quota_exhausted"
            } else {
                "failed"
            }
            .into(),
            text: body,
            model_reported: (!result.model_reported.is_empty())
                .then(|| result.model_reported.clone()),
            tokens_in: (result.tokens_in > 0).then_some(result.tokens_in),
            tokens_out: (result.tokens_out > 0).then_some(result.tokens_out),
            tokens_cache_read: (result.cache_read > 0).then_some(result.cache_read),
            ms: Some(result.duration_ms),
            session_id: (!result.session_id.is_empty()).then(|| result.session_id.clone()),
            runner: (!result.runner.is_empty()).then(|| result.runner.clone()),
            adopted: old_adopted,
            artifacts: result.artifacts.as_ref().map(|a| ReplyArtifacts {
                source_node_id: (!a.source_node_id.is_empty()).then(|| a.source_node_id.clone()),
                diff: a.diff.clone(),
                changed_files: a
                    .changed_files
                    .iter()
                    .map(|f| orchestration::ReplyChangedFile {
                        path: f.path.clone(),
                        content: f.content.clone(),
                    })
                    .collect(),
                worktree: a.worktree.clone(),
                stdout_summary: a.stdout_summary.clone(),
            }),
            error_kind: (!ok).then(|| result.status.clone()),
            fallback_from: (!result.fallback_from.is_empty()).then(|| result.fallback_from.clone()),
            fallback_to: (!result.fallback_to.is_empty()).then(|| result.fallback_to.clone()),
            quota_resets_at: result.quota_resets_at.clone(),
            ..Reply::default()
        });
    }
    next
}

fn parse_acceptance(text: &str) -> &'static str {
    let upper = text
        .chars()
        .rev()
        .take(1200)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>()
        .to_uppercase();
    let mut verdict = "UNKNOWN";
    for word in upper.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
        if word == "PASS" || word == "NEEDS_FIX" {
            verdict = if word == "PASS" { "PASS" } else { "NEEDS_FIX" };
        }
    }
    verdict
}

fn verification_siblings(graph: &Graph, accept: &GraphNode) -> Vec<GraphNode> {
    sorted_nodes(graph)
        .into_iter()
        .filter(|n| {
            n.id != accept.id
                && n.target.as_ref().is_some_and(|t| t.role == "验证")
                && n.parents.len() == accept.parents.len()
                && n.parents.iter().all(|p| accept.parents.contains(p))
        })
        .cloned()
        .collect()
}

fn append_correction(
    graph: &Graph,
    accept_id: &str,
    ids: &mut dyn Iterator<Item = String>,
    base: DateTime<Utc>,
) -> (Graph, Vec<String>) {
    let Some(accept) = graph.nodes.get(accept_id) else {
        return (graph.clone(), vec![]);
    };
    if !accept.target.as_ref().is_some_and(|t| t.role == "验收") {
        return (graph.clone(), vec![]);
    }
    let Some(previous) = accept
        .parents
        .iter()
        .filter_map(|id| graph.nodes.get(id))
        .find(|n| {
            n.target.as_ref().is_some_and(|t| t.role == "执行")
                && n.reply
                    .as_ref()
                    .and_then(|r| r.artifacts.as_ref())
                    .is_some()
        })
    else {
        return (graph.clone(), vec![]);
    };
    let (Some(exec_id), Some(acceptance_id)) = (ids.next(), ids.next()) else {
        return (graph.clone(), vec![]);
    };
    let mut next = graph.clone();
    let exec = GraphNode {
        id: exec_id.clone(),
        kind: "turn".into(),
        parents: vec![accept_id.into()],
        user: Message {
            text: format!(
                "根据上一轮验收意见修正实现。\n\n{}",
                accept.reply.as_ref().map(|r| r.text.as_str()).unwrap_or("")
            ),
            attachments: vec![],
        },
        reply: Some(Reply {
            status: "draft".into(),
            ..Reply::default()
        }),
        target: previous.target.clone().map(|mut t| {
            t.upstreams = Some(vec![accept_id.into(), previous.id.clone()]);
            t
        }),
        created_at: base.to_rfc3339_opts(SecondsFormat::Millis, true),
        ..GraphNode::default()
    };
    next.nodes.insert(exec_id.clone(), exec);
    let acceptance = GraphNode {
        id: acceptance_id.clone(),
        kind: "turn".into(),
        parents: vec![exec_id.clone()],
        user: Message::default(),
        reply: Some(Reply {
            status: "draft".into(),
            ..Reply::default()
        }),
        target: accept.target.clone(),
        created_at: (base + chrono::Duration::milliseconds(1))
            .to_rfc3339_opts(SecondsFormat::Millis, true),
        ..GraphNode::default()
    };
    next.nodes.insert(acceptance_id.clone(), acceptance);
    let mut made = vec![exec_id.clone(), acceptance_id];
    for (index, sibling) in verification_siblings(graph, accept).into_iter().enumerate() {
        let Some(id) = ids.next() else { break };
        let mut target = sibling.target;
        if let Some(t) = &mut target {
            t.upstreams = None
        };
        next.nodes.insert(
            id.clone(),
            GraphNode {
                id: id.clone(),
                kind: "turn".into(),
                parents: vec![exec_id.clone()],
                user: Message {
                    text: sibling.user.text,
                    attachments: vec![],
                },
                reply: Some(Reply {
                    status: "draft".into(),
                    ..Reply::default()
                }),
                target,
                created_at: (base + chrono::Duration::milliseconds(2 + index as i64))
                    .to_rfc3339_opts(SecondsFormat::Millis, true),
                ..GraphNode::default()
            },
        );
        made.push(id)
    }
    (next, made)
}

fn acceptance_round(graph: &Graph, node_id: &str, scope: &HashSet<String>) -> usize {
    let mut count = 0;
    let mut stack: Vec<String> = vec![node_id.into()];
    let mut seen = HashSet::new();
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        if let Some(n) = graph.nodes.get(&id) {
            if n.target.as_ref().is_some_and(|t| t.role == "验收") && scope.contains(&id) {
                count += 1
            }
            stack.extend(n.parents.clone())
        }
    }
    count
}

fn leftover_items(text: &str) -> Vec<String> {
    let lines: Vec<_> = text
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let bullets: Vec<_> = lines
        .iter()
        .filter(|line| {
            let s = **line;
            s.starts_with("- ")
                || s.starts_with("* ")
                || s.starts_with("• ")
                || s.chars().next().is_some_and(|c| c.is_ascii_digit())
        })
        .copied()
        .collect();
    (if bullets.is_empty() { lines } else { bullets })
        .into_iter()
        .take(20)
        .map(str::to_string)
        .collect()
}

fn append_leftover(graph: &Graph, accept_id: &str, id: String, time: String) -> Graph {
    let Some(accept) = graph.nodes.get(accept_id) else {
        return graph.clone();
    };
    let items = leftover_items(accept.reply.as_ref().map(|r| r.text.as_str()).unwrap_or(""));
    if items.is_empty() {
        return graph.clone();
    }
    let route = accept
        .target
        .as_ref()
        .map(|t| {
            if t.agent.trim().is_empty() || t.model.trim().is_empty() {
                t.role.clone()
            } else {
                format!("{}（{}/{}）", t.role, t.agent, t.model)
            }
        })
        .unwrap_or_else(|| "验收".into());
    let node = GraphNode {
        id: id.clone(),
        kind: "turn".into(),
        parents: vec![accept_id.into()],
        user: Message {
            text: format!("遗留：{} 条未闭合，来源 {route}", items.len()),
            attachments: vec![],
        },
        leftover: Some(Leftover {
            source_node_id: accept_id.into(),
            items,
        }),
        created_at: time,
        ..GraphNode::default()
    };
    let mut next = graph.clone();
    next.nodes.insert(id, node);
    next
}

fn append_validation_retry(
    graph: &Graph,
    execution_id: &str,
    detail: &str,
    count: usize,
    limit: usize,
    unpinned: bool,
    ids: &mut dyn Iterator<Item = String>,
    base: DateTime<Utc>,
) -> (Graph, Vec<String>) {
    let Some(execution) = graph.nodes.get(execution_id) else {
        return (graph.clone(), vec![]);
    };
    let Some(waiting) = sorted_nodes(graph).into_iter().find(|n| {
        n.parents.contains(&execution_id.to_string())
            && n.target.as_ref().is_some_and(|t| t.role == "验收")
            && n.reply.as_ref().is_some_and(|r| r.status == "draft")
    }) else {
        return (graph.clone(), vec![]);
    };
    let (Some(exec_id), Some(accept_id)) = (ids.next(), ids.next()) else {
        return (graph.clone(), vec![]);
    };
    let reason = if unpinned {
        "上游测试未钉住"
    } else {
        "上游验证未通过"
    };
    let mut next = graph.clone();
    if let Some(n) = next.nodes.get_mut(&waiting.id) {
        n.reply = Some(Reply {
            status: "failed".into(),
            text: if unpinned {
                "测试未钉住，未送审"
            } else {
                "验证命令未通过，未送审"
            }
            .into(),
            ..Reply::default()
        })
    }
    for sibling in verification_siblings(graph, waiting) {
        if sibling.reply.as_ref().is_some_and(|r| r.status == "draft") {
            if let Some(n) = next.nodes.get_mut(&sibling.id) {
                n.reply = Some(Reply {
                    status: "draft".into(),
                    skipped_reason: Some(reason.into()),
                    ..Reply::default()
                })
            }
        }
    }
    let suffix = if unpinned {
        "\n\n上一轮修复回退后测试仍通过，请补能失败的测试"
    } else {
        ""
    };
    next.nodes.insert(
        exec_id.clone(),
        GraphNode {
            id: exec_id.clone(),
            kind: if execution.parents.len() > 1 {
                "accept".into()
            } else {
                "turn".into()
            },
            parents: execution.parents.clone(),
            user: Message {
                text: format!(
                    "[修正 {count}/{limit}] {}，请修正后重新运行。\n\n{detail}{suffix}",
                    if unpinned {
                        "测试未钉住"
                    } else {
                        "验证命令未通过"
                    }
                ),
                attachments: vec![],
            },
            reply: Some(Reply {
                status: "draft".into(),
                ..Reply::default()
            }),
            target: execution.target.clone(),
            created_at: base.to_rfc3339_opts(SecondsFormat::Millis, true),
            ..GraphNode::default()
        },
    );
    next.nodes.insert(
        accept_id.clone(),
        GraphNode {
            id: accept_id.clone(),
            kind: "turn".into(),
            parents: vec![exec_id.clone()],
            user: Message::default(),
            reply: Some(Reply {
                status: "draft".into(),
                ..Reply::default()
            }),
            target: waiting.target.clone(),
            created_at: (base + chrono::Duration::milliseconds(1))
                .to_rfc3339_opts(SecondsFormat::Millis, true),
            ..GraphNode::default()
        },
    );
    (next, vec![exec_id, accept_id])
}

fn reduce_step(
    graph: &Graph,
    results: &[DispatchNodeResult],
    retries: &mut HashMap<String, usize>,
    limit: usize,
    scope: &mut HashSet<String>,
    ids: &mut dyn Iterator<Item = String>,
    clock: &mut dyn FnMut() -> DateTime<Utc>,
) -> (Graph, bool) {
    let mut next = graph.clone();
    let mut added = false;
    for result in results {
        if next
            .nodes
            .get(&result.node_id)
            .and_then(|n| n.target.as_ref())
            .is_some_and(|t| t.role == "验收")
            && result.status == "ok"
            && parse_acceptance(&result.text) == "NEEDS_FIX"
        {
            if acceptance_round(&next, &result.node_id, scope) > limit {
                if let Some(id) = ids.next() {
                    next = append_leftover(
                        &next,
                        &result.node_id,
                        id,
                        clock().to_rfc3339_opts(SecondsFormat::Millis, true),
                    );
                }
            } else {
                let (g, made) = append_correction(&next, &result.node_id, ids, clock());
                added |= !made.is_empty();
                scope.extend(made);
                next = g;
            }
        }
    }
    if added {
        return (next, true);
    }
    let failures: Vec<_> = results
        .iter()
        .filter(|r| r.status == "validation_failed" || r.pinned == Some(PinnedState::Unpinned))
        .collect();
    if !failures.is_empty() {
        for failure in failures {
            let count = *retries.get(&failure.node_id).unwrap_or(&0);
            if count >= limit {
                let waiting = sorted_nodes(&next)
                    .into_iter()
                    .find(|n| {
                        n.parents.contains(&failure.node_id)
                            && n.target.as_ref().is_some_and(|t| t.role == "验收")
                            && n.reply.as_ref().is_some_and(|r| r.status == "draft")
                    })
                    .map(|n| n.id.clone());
                if let Some(id) = waiting {
                    if let Some(n) = next.nodes.get_mut(&id) {
                        n.reply = Some(Reply {
                            status: "draft".into(),
                            skipped_reason: Some(
                                if failure.pinned == Some(PinnedState::Unpinned) {
                                    "上游测试未钉住"
                                } else {
                                    "上游验证未通过"
                                }
                                .into(),
                            ),
                            ..Reply::default()
                        })
                    }
                }
                continue;
            }
            let detail = if failure.validation_output.is_empty() {
                &failure.error
            } else {
                &failure.validation_output
            };
            let (g, made) = append_validation_retry(
                &next,
                &failure.node_id,
                detail,
                count + 1,
                limit,
                failure.pinned == Some(PinnedState::Unpinned),
                ids,
                clock(),
            );
            if let Some(id) = made.first() {
                retries.insert(id.clone(), count + 1);
                added = true
            }
            scope.extend(made);
            next = g
        }
        return (next, added);
    }
    if results.iter().any(|r| r.status != "ok") {
        return (next, false);
    }
    let has_accept = results.iter().any(|r| {
        next.nodes
            .get(&r.node_id)
            .and_then(|n| n.target.as_ref())
            .is_some_and(|t| t.role == "验收")
    });
    (next, !has_accept)
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct DispatchInput {
    pub project: String,
    pub text: String,
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(default)]
    pub attachments: Vec<String>,
    #[serde(default)]
    pub targets: Vec<Target>,
}

#[derive(Debug)]
pub(crate) enum StartError {
    Unavailable(String),
}

fn latest_project_graph(root: &Path, project: &str) -> Option<Graph> {
    let dir = root.join("graphs");
    let mut graphs: Vec<Graph> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|e| fs::read(e.path()).ok())
        .filter_map(|b| serde_json::from_slice(&b).ok())
        .filter(|g: &Graph| normalized(&g.project) == normalized(project))
        .collect();
    graphs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    graphs.into_iter().next()
}
fn normalized(value: &str) -> String {
    value
        .trim_start_matches(r"\\?\")
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase()
}
fn find_model<'a>(models: &'a [ResourceModel], target: &Target) -> Option<&'a ResourceModel> {
    models
        .iter()
        .find(|m| m.id == target.model && (target.agent.is_empty() || m.agent == target.agent))
}
fn validate_block(graph: &Graph, id: &str, models: &[ResourceModel]) -> Option<String> {
    let node = graph.nodes.get(id)?;
    let target = node.target.as_ref()?;
    if matches!(target.role.as_str(), "验收" | "验证" | "仲裁" | "拆分") && target.write != "只读"
    {
        return Some(if matches!(target.role.as_str(), "验收" | "验证") {
            "验收/验证节点只能只读".into()
        } else {
            format!("{}节点只能只读", target.role)
        });
    }
    if target.visibility == "盲" && target.write != "只读" {
        return Some("盲节点只能只读".into());
    }
    if !target.model.is_empty() && find_model(models, target).is_none() {
        return Some("未接入".into());
    }
    None
}
fn quota_active(value: &Option<String>) -> bool {
    value
        .as_ref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .is_some_and(|t| t > Utc::now())
}
fn dirty_reason(project: &str, has_worktree: bool) -> Option<String> {
    if !has_worktree {
        return None;
    }
    match crate::git::git_changes(project.into()) {
        crate::git::GitChanges::Ok {
            uncommitted,
            untracked,
            ..
        } => {
            let count = uncommitted.len() + untracked.len();
            (count > 0).then(|| {
                format!("项目目录有 {count} 处未提交改动；worktree 从 HEAD 新建，看不到这些改动")
            })
        }
        _ => None,
    }
}

fn active_runs(root: &Path) -> usize {
    let Ok(entries) = fs::read_dir(runs_dir(root)) else {
        return 0;
    };
    entries
        .flatten()
        .filter_map(|e| fs::read(e.path()).ok())
        .filter_map(|b| serde_json::from_slice::<EngineRun>(&b).ok())
        .filter(|r| r.status == "running" && process_alive(r.pid))
        .count()
}

pub(crate) fn start_dispatch(root: &Path, input: DispatchInput) -> Result<Value, StartError> {
    let mut input = input;
    let canonical_project = fs::canonicalize(input.project.trim())
        .ok()
        .filter(|project| project.is_dir());
    if let Some(project) = &canonical_project {
        input.project = project.to_string_lossy().into_owned();
    }
    fs::create_dir_all(runs_dir(root))
        .map_err(|e| StartError::Unavailable(format!("创建 run 目录失败：{e}")))?;
    let run_id = Uuid::new_v4().to_string();
    let started = now();
    let settings = load_settings(root);
    let reject = |reason: String, graph_id: String| -> Result<Value, StartError> {
        let run = EngineRun {
            run_id: run_id.clone(),
            graph_id: graph_id.clone(),
            project: input.project.clone(),
            status: "rejected".into(),
            started_at: started.clone(),
            updated_at: now(),
            reason: reason.clone(),
            ..EngineRun::default()
        };
        write_run(root, &run).map_err(StartError::Unavailable)?;
        Ok(
            serde_json::json!({"run_id":run_id,"accepted":false,"graph_id":graph_id,"node_ids":[],"status":"rejected","reason":reason}),
        )
    };
    if canonical_project.is_none() {
        return reject("项目目录不存在".into(), String::new());
    }
    if active_runs(root) >= settings.console_run_limit {
        return reject(
            format!("同时在飞 run 已达上限 {}", settings.console_run_limit),
            String::new(),
        );
    }
    let resources = orchestration::dispatchable_from(root);
    if let Some(error) = resources.error {
        return reject(format!("读取资源失败：{error}"), String::new());
    }
    let presets = orchestration::presets_from(root);
    let selected = if !input.targets.is_empty() {
        Some(Preset {
            id: format!("console-{run_id}"),
            name: String::new(),
            builtin: false,
            stages: vec![orchestration::Stage {
                nodes: input.targets.clone(),
            }],
        })
    } else {
        let wanted = input
            .preset
            .clone()
            .unwrap_or_else(|| "builtin-economy".into());
        presets.presets.into_iter().find(|p| p.id == wanted)
    };
    let Some(selected) = selected else {
        return reject(
            format!(
                "找不到预设：{}",
                input.preset.unwrap_or_else(|| "builtin-economy".into())
            ),
            String::new(),
        );
    };
    let preset = apply_role_defaults(selected, &orchestration::roles_from(root));
    let base = latest_project_graph(root, &input.project).unwrap_or_else(|| {
        new_graph(
            &input.project,
            Uuid::new_v4().to_string(),
            Uuid::new_v4().to_string(),
            now(),
        )
    });
    let graph_source = base.source.clone();
    let (material, material_id) = append_material(
        &Graph {
            project: input.project.clone(),
            source: if graph_source.is_empty() {
                "console".into()
            } else {
                graph_source
            },
            ..base
        },
        &input.text,
        &input.attachments,
        Uuid::new_v4().to_string(),
        now(),
    );
    let original = material
        .nodes
        .get(&material_id)
        .map(|n| n.user.text.clone())
        .unwrap_or_default();
    let mut ids = std::iter::repeat_with(|| Uuid::new_v4().to_string());
    let (mut graph, created) = stamp_with(
        &material,
        &material_id,
        &preset,
        settings.swarm_limit,
        &mut ids,
        Utc::now(),
    );
    let first: Vec<_> = dispatchable_ids(&graph)
        .into_iter()
        .filter(|id| created.contains(id))
        .collect();
    if let Some(id) = first.iter().find(|id| {
        graph
            .nodes
            .get(*id)
            .and_then(|n| n.target.as_ref())
            .is_some_and(|t| t.agent.trim().is_empty() || t.model.trim().is_empty())
    }) {
        let role = graph.nodes[id]
            .target
            .as_ref()
            .map(|t| t.role.as_str())
            .unwrap_or("");
        return reject(format!("角色「{role}」没有可派发目标"), graph.id.clone());
    }
    if let Some(hint) = first
        .iter()
        .filter_map(|id| validate_block(&graph, id, &resources.models))
        .find(|h| h == "未接入" || h.contains("只能只读"))
    {
        return reject(hint, graph.id.clone());
    }
    if let Some(id) = first.iter().find(|id| {
        graph
            .nodes
            .get(*id)
            .and_then(|n| n.target.as_ref())
            .and_then(|t| find_model(&resources.models, t))
            .is_some_and(|m| m.verification == "failed")
    }) {
        let t = graph.nodes[id].target.as_ref().unwrap();
        return reject(
            format!("目标验证失败：{}/{}", t.agent, t.model),
            graph.id.clone(),
        );
    }
    if let Some(id) = first.iter().find(|id| {
        graph
            .nodes
            .get(*id)
            .and_then(|n| n.target.as_ref())
            .and_then(|t| find_model(&resources.models, t))
            .is_some_and(|m| quota_active(&m.quota_exhausted_until))
    }) {
        let t = graph.nodes[id].target.as_ref().unwrap();
        let reset = find_model(&resources.models, t)
            .and_then(|m| m.quota_exhausted_until.as_ref())
            .map(|s| format!("；恢复时间 {s}"))
            .unwrap_or_default();
        return reject(
            format!("目标额度用尽：{}/{}{reset}", t.agent, t.model),
            graph.id.clone(),
        );
    }
    if created.len() > settings.max_requests {
        return reject(
            format!(
                "本次派发 {} 路，超过总请求上限 {}",
                created.len(),
                settings.max_requests
            ),
            graph.id.clone(),
        );
    }
    if let Some(reason) = dirty_reason(
        &input.project,
        first.iter().any(|id| {
            graph.nodes[id]
                .target
                .as_ref()
                .is_some_and(|t| t.write == "worktree")
        }),
    ) {
        return reject(reason, graph.id.clone());
    }
    graph.updated_at = now();
    orchestration::save_graph_in(root, &graph).map_err(StartError::Unavailable)?;
    let run = EngineRun {
        run_id: run_id.clone(),
        graph_id: graph.id.clone(),
        project: input.project.clone(),
        status: "running".into(),
        nodes: node_summary(&graph, created.clone()),
        started_at: started,
        updated_at: now(),
        ..EngineRun::default()
    };
    write_run(root, &run).map_err(StartError::Unavailable)?;
    let child = spawn_background(&run_id).map_err(|e| {
        let mut failed = run.clone();
        failed.status = "failed".into();
        failed.reason = e.clone();
        failed.updated_at = now();
        let _ = write_run(root, &failed);
        StartError::Unavailable(e)
    })?;
    let mut running = run;
    running.pid = child;
    running.updated_at = now();
    write_run(root, &running).map_err(StartError::Unavailable)?;
    let _ = original;
    Ok(serde_json::json!({"run_id":run_id,"accepted":true,"graph_id":graph.id,"node_ids":created}))
}

fn spawn_background(run_id: &str) -> Result<u32, String> {
    let exe = std::env::current_exe().map_err(|e| format!("定位引擎程序失败：{e}"))?;
    let mut command = Command::new(exe);
    command.arg("__run").arg(run_id);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000 | 0x0000_0008);
    }
    command
        .spawn()
        .map(|c| c.id())
        .map_err(|e| format!("启动后台引擎失败：{e}"))
}

fn load_graph_for_run(root: &Path, run: &EngineRun) -> Result<Graph, String> {
    orchestration::load_graph_in(root, &run.graph_id)?.ok_or_else(|| "找不到 run 对应的图".into())
}
fn leftover(graph: &Graph) -> Vec<String> {
    sorted_nodes(graph)
        .into_iter()
        .filter_map(|n| n.leftover.as_ref())
        .flat_map(|l| l.items.clone())
        .collect()
}

fn update_running(
    root: &Path,
    run: &mut EngineRun,
    graph: &Graph,
    ids: &HashSet<String>,
    in_flight: Vec<String>,
) -> Result<(), String> {
    run.nodes = node_summary(graph, ids.iter().cloned());
    run.leftover = leftover(graph);
    run.in_flight_node_ids = in_flight;
    run.updated_at = now();
    write_run(root, run)
}

pub(crate) fn run_background(root: &Path, run_id: &str) -> Result<(), String> {
    let mut run: EngineRun = serde_json::from_slice(
        &fs::read(run_path(root, run_id)).map_err(|e| format!("读取 run 失败：{e}"))?,
    )
    .map_err(|e| format!("解析 run 失败：{e}"))?;
    run.pid = std::process::id();
    run.updated_at = now();
    write_run(root, &run)?;
    let mut graph = load_graph_for_run(root, &run)?;
    let settings = load_settings(root);
    let resources = orchestration::dispatchable_from(root);
    if let Some(error) = resources.error {
        return finish_failed(
            root,
            &mut run,
            &graph,
            &HashSet::new(),
            format!("读取资源失败：{error}"),
        );
    }
    let mut all_ids: HashSet<String> = run.nodes.iter().map(|n| n.node_id.clone()).collect();
    let skip: HashSet<String> = graph
        .nodes
        .values()
        .filter(|n| n.target.is_some() && !all_ids.contains(&n.id))
        .map(|n| n.id.clone())
        .collect();
    let mut scope = all_ids.clone();
    let mut collected: Vec<DispatchNodeResult> = vec![];
    let mut retries = HashMap::new();
    let mut processed = HashSet::new();
    let mut id_source = std::iter::repeat_with(|| Uuid::new_v4().to_string());
    let mut clock = || Utc::now();
    let original = all_ids
        .iter()
        .filter_map(|id| graph.nodes.get(id))
        .flat_map(|n| n.parents.iter())
        .filter_map(|id| graph.nodes.get(id))
        .find(|n| n.kind == "material")
        .map(|n| n.user.text.clone())
        .unwrap_or_else(|| {
            graph
                .nodes
                .get(&graph.root)
                .map(|n| n.user.text.clone())
                .unwrap_or_default()
        });
    let active = Arc::new(Mutex::new(Vec::<String>::new()));
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let watcher_active = Arc::clone(&active);
    let watcher_cancelled = Arc::clone(&cancelled);
    let watcher_root = root.to_path_buf();
    let watcher_run = run_id.to_string();
    let watcher = std::thread::spawn(move || loop {
        if cancel_path(&watcher_root, &watcher_run).exists() {
            watcher_cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
            for id in watcher_active.lock().map(|v| v.clone()).unwrap_or_default() {
                dispatch::cancel_dispatch_node(watcher_run.clone(), id);
            }
            break;
        }
        if watcher_cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    });
    let mut error = String::new();
    loop {
        if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            error = "派发已取消".into();
            break;
        }
        let ids: Vec<_> = dispatchable_ids(&graph)
            .into_iter()
            .filter(|id| !skip.contains(id))
            .collect();
        if ids.is_empty() {
            break;
        }
        let missing: Vec<_> = ids
            .iter()
            .filter(|id| {
                graph
                    .nodes
                    .get(*id)
                    .and_then(|n| n.target.as_ref())
                    .is_none_or(|t| t.agent.trim().is_empty() || t.model.trim().is_empty())
            })
            .cloned()
            .collect();
        if !missing.is_empty() {
            error = MISSING_TARGET_TEXT.into();
            break;
        }
        if collected.len() + ids.len() > settings.max_requests {
            error = format!(
                "已达总请求上限 {}，继续派发需重新确认。",
                settings.max_requests
            );
            break;
        }
        let request = build_request(
            run_id,
            &run.project,
            &settings,
            &graph,
            &ids,
            &resources.models,
            &original,
            &scope,
        );
        all_ids.extend(ids.iter().cloned());
        *active.lock().map_err(|_| "取消状态不可用".to_string())? = ids.clone();
        graph = mark_pending(&graph, &ids);
        graph.updated_at = now();
        orchestration::save_graph_in(root, &graph)?;
        update_running(root, &mut run, &graph, &all_ids, ids.clone())?;
        let output = match dispatch::dispatch_stage_core(request.clone(), None) {
            Ok(v) => v,
            Err(message) => {
                let received = HashSet::<String>::new();
                let results = request
                    .nodes
                    .iter()
                    .filter(|n| !received.contains(&n.node_id))
                    .map(|n| DispatchNodeResult {
                        node_id: n.node_id.clone(),
                        stage: n.stage,
                        status: "error".into(),
                        error: message.clone(),
                        ..DispatchNodeResult::default()
                    })
                    .collect();
                crate::dispatch::DispatchStageResult {
                    run_id: run_id.into(),
                    results,
                    state_file: String::new(),
                }
            }
        };
        *active.lock().map_err(|_| "取消状态不可用".to_string())? = vec![];
        if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            error = "派发已取消".into();
            break;
        }
        let mut batch_ids: Vec<_> = output.results.iter().map(|r| r.node_id.clone()).collect();
        batch_ids.sort();
        batch_ids.dedup();
        let key = format!(
            "{}#{}#{}",
            if output.run_id.is_empty() {
                run_id
            } else {
                &output.run_id
            },
            request.stage,
            batch_ids.join(",")
        );
        if !processed.insert(key) {
            graph = reset_draft(&graph, &ids);
            error = DUPLICATE_BATCH_TEXT.into();
            break;
        }
        collected.extend(output.results.clone());
        graph = apply_results(&graph, &output.results);
        let before: HashSet<_> = graph.nodes.keys().cloned().collect();
        let (next, keep_going) = reduce_step(
            &graph,
            &output.results,
            &mut retries,
            settings.fix_round_limit,
            &mut scope,
            &mut id_source,
            &mut clock,
        );
        for id in next.nodes.keys() {
            if !before.contains(id) {
                all_ids.insert(id.clone());
                scope.insert(id.clone());
            }
        }
        graph = next;
        graph.updated_at = now();
        orchestration::save_graph_in(root, &graph)?;
        update_running(root, &mut run, &graph, &all_ids, vec![])?;
        if !keep_going {
            break;
        }
    }
    cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = watcher.join();
    if error == "派发已取消" {
        for id in all_ids.iter() {
            if let Some(node) = graph.nodes.get_mut(id) {
                if node.reply.as_ref().is_some_and(|r| r.status == "pending") {
                    node.reply = Some(Reply {
                        status: "failed".into(),
                        text: "派发已取消".into(),
                        error_kind: Some("cancelled".into()),
                        ..Reply::default()
                    })
                }
            }
        }
    }
    graph.updated_at = now();
    orchestration::save_graph_in(root, &graph)?;
    run.nodes = node_summary(&graph, all_ids);
    run.leftover = leftover(&graph);
    run.in_flight_node_ids = vec![];
    run.reason = error.clone();
    run.status = if !error.is_empty() || run.nodes.iter().any(|n| n.status == "failed") {
        "failed"
    } else {
        "done"
    }
    .into();
    run.updated_at = now();
    write_run(root, &run)?;
    let _ = fs::remove_file(cancel_path(root, run_id));
    Ok(())
}

fn finish_failed(
    root: &Path,
    run: &mut EngineRun,
    graph: &Graph,
    ids: &HashSet<String>,
    reason: String,
) -> Result<(), String> {
    run.status = "failed".into();
    run.reason = reason;
    run.nodes = node_summary(graph, ids.iter().cloned());
    run.updated_at = now();
    write_run(root, run)
}

pub(crate) fn adopt(root: &Path, run_id: &str, node_id: &str) -> Result<EngineRun, String> {
    let mut run = read_run(root, run_id)?;
    let mut graph = load_graph_for_run(root, &run)?;
    let node = graph
        .nodes
        .get_mut(node_id)
        .ok_or_else(|| "找不到节点".to_string())?;
    let reply = node
        .reply
        .as_mut()
        .ok_or_else(|| "节点没有结果".to_string())?;
    reply.adopted = Some(true);
    graph.updated_at = now();
    orchestration::save_graph_in(root, &graph)?;
    let ids = run
        .nodes
        .iter()
        .map(|n| n.node_id.clone())
        .collect::<Vec<_>>();
    run.nodes = node_summary(&graph, ids);
    run.updated_at = now();
    write_run(root, &run)?;
    Ok(run)
}

pub(crate) fn cancel(root: &Path, run_id: &str) -> Result<EngineRun, String> {
    let mut run = read_run(root, run_id)?;
    if run.status != "running" {
        return Err("run 当前不在运行".into());
    }
    fs::create_dir_all(runs_dir(root)).map_err(|e| format!("创建 run 目录失败：{e}"))?;
    fs::write(cancel_path(root, run_id), []).map_err(|e| format!("写入取消标记失败：{e}"))?;
    run.updated_at = now();
    write_run(root, &run)?;
    Ok(run)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("aicross-engine-{name}-{}", Uuid::new_v4()));
        fs::create_dir_all(&p).unwrap();
        p
    }
    #[test]
    fn settings_missing_invalid_and_clamped() {
        let root = temp("settings");
        assert_eq!(load_settings(&root), EngineSettings::default());
        fs::write(root.join("dispatch-settings.json"), "{").unwrap();
        assert_eq!(load_settings(&root), EngineSettings::default());
        fs::write(root.join("dispatch-settings.json"),r#"{"timeoutSecs":0,"concurrency":99,"maxRequests":101,"swarmLimit":0,"consoleRunLimit":9,"fixRoundLimit":12.8,"redact":false}"#).unwrap();
        let s = load_settings(&root);
        assert_eq!(
            (
                s.timeout_secs,
                s.concurrency,
                s.max_requests,
                s.swarm_limit,
                s.console_run_limit,
                s.fix_round_limit
            ),
            (1, 3, 100, 1, 3, 9)
        );
        assert!(!s.redact);
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn atomic_run_write_leaves_only_target() {
        let root = temp("atomic");
        let mut run = EngineRun {
            run_id: "r1".into(),
            status: "running".into(),
            pid: std::process::id(),
            ..EngineRun::default()
        };
        write_run(&root, &run).unwrap();
        run.status = "done".into();
        write_run(&root, &run).unwrap();
        assert_eq!(fs::read_dir(runs_dir(&root)).unwrap().count(), 1);
        assert_eq!(read_run(&root, "r1").unwrap().status, "done");
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn dead_pid_becomes_failed() {
        let root = temp("dead");
        let run = EngineRun {
            run_id: "r1".into(),
            status: "running".into(),
            pid: u32::MAX,
            ..EngineRun::default()
        };
        write_run(&root, &run).unwrap();
        let got = read_run(&root, "r1").unwrap();
        assert_eq!(got.status, "failed");
        assert_eq!(got.reason, "引擎进程已退出");
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn cancel_marker_is_observable() {
        let root = temp("cancel");
        fs::create_dir_all(runs_dir(&root)).unwrap();
        fs::write(cancel_path(&root, "r1"), []).unwrap();
        assert!(cancel_path(&root, "r1").exists());
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn utf16_excerpt_covers_chinese_and_emoji() {
        assert_eq!(utf16_excerpt(&"中".repeat(161), 160), "中".repeat(160));
        let text = format!("{}😀尾", "a".repeat(159));
        assert_eq!(utf16_excerpt(&text, 160), format!("{}�", "a".repeat(159)));
    }
    #[test]
    fn typescript_golden_fixture_drives_rust_pure_functions() {
        fn without_nulls(value: Value) -> Value {
            match value {
                Value::Object(object) => Value::Object(
                    object
                        .into_iter()
                        .filter_map(|(key, value)| {
                            (!value.is_null()).then(|| (key, without_nulls(value)))
                        })
                        .collect(),
                ),
                Value::Array(values) => {
                    Value::Array(values.into_iter().map(without_nulls).collect())
                }
                other => other,
            }
        }
        fn request_defaults(mut value: Value) -> Value {
            if let Value::Object(object) = &mut value {
                object.remove("fallback");
                for key in [
                    "rollbackBefore",
                    "reviewMaterialValidation",
                    "fixRound",
                    "interactive",
                ] {
                    if object.get(key) == Some(&Value::Bool(false)) {
                        object.remove(key);
                    }
                }
            }
            value
        }
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/engine-golden");
        let mut fixtures = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
            .collect::<Vec<_>>();
        fixtures.sort_by_key(|entry| entry.file_name());
        assert_eq!(fixtures.len(), 24);
        for entry in &fixtures {
            let value: Value = serde_json::from_slice(&fs::read(entry.path()).unwrap()).unwrap();
            assert!(value["inputGraph"].is_object(), "{:?}", entry.path());
            assert!(value["requests"].is_array(), "{:?}", entry.path());
            assert!(value["finalGraph"].is_object(), "{:?}", entry.path());
            assert!(value["runSummary"].is_array(), "{:?}", entry.path());
        }

        // The reference pass fixture exercises request construction, result application, and
        // summary construction field-for-field. The remaining fixtures exercise their matching
        // transition in the TypeScript suite and are loaded above as the shared contract set.
        let value: Value = serde_json::from_slice(
            &fs::read(dir.join("economy-pass.json")).unwrap(),
        )
        .unwrap();
        let graph: Graph = serde_json::from_value(value["inputGraph"].clone()).unwrap();
        let request = build_request(
            "run-1",
            "D:/project",
            &EngineSettings::default(),
            &graph,
            &["node".into()],
            &[],
            "实现任务",
            &HashSet::new(),
        );
        assert_eq!(
            request_defaults(serde_json::to_value(&request.nodes[0]).unwrap()),
            value["requests"][0]
        );
        let raw = &value["injectedResults"][0][0];
        let result = DispatchNodeResult {
            node_id: raw["nodeId"].as_str().unwrap().into(),
            stage: raw["stage"].as_u64().unwrap() as u32,
            status: raw["status"].as_str().unwrap().into(),
            text: raw["text"].as_str().unwrap().into(),
            model_reported: raw["modelReported"].as_str().unwrap().into(),
            session_id: raw["sessionId"].as_str().unwrap().into(),
            tokens_in: raw["tokensIn"].as_u64().unwrap(),
            tokens_out: raw["tokensOut"].as_u64().unwrap(),
            cache_read: raw["cacheRead"].as_u64().unwrap(),
            duration_ms: raw["durationMs"].as_u64().unwrap(),
            error: raw["error"].as_str().unwrap().into(),
            runner: raw["runner"].as_str().unwrap().into(),
            ..DispatchNodeResult::default()
        };
        let final_graph = apply_results(&graph, &[result]);
        assert_eq!(
            without_nulls(serde_json::to_value(&final_graph).unwrap()),
            value["finalGraph"]
        );
        assert_eq!(
            serde_json::to_value(node_summary(&final_graph, ["node".to_string()])).unwrap(),
            value["runSummary"]
        );
    }
    #[test]
    fn adopt_changes_persisted_graph_without_window() {
        let root = temp("adopt");
        let mut graph = new_graph("D:/p", "g1".into(), "root".into(), now());
        graph.nodes.get_mut("root").unwrap().reply = Some(Reply {
            status: "done".into(),
            text: "answer".into(),
            ..Reply::default()
        });
        orchestration::save_graph_in(&root, &graph).unwrap();
        let run = EngineRun {
            run_id: "r1".into(),
            graph_id: "g1".into(),
            project: "D:/p".into(),
            status: "done".into(),
            nodes: node_summary(&graph, vec!["root".into()]),
            ..EngineRun::default()
        };
        write_run(&root, &run).unwrap();
        adopt(&root, "r1", "root").unwrap();
        let loaded = orchestration::load_graph_in(&root, "g1").unwrap().unwrap();
        assert_eq!(
            loaded.nodes["root"].reply.as_ref().unwrap().adopted,
            Some(true)
        );
        let _ = fs::remove_dir_all(root);
    }
}
