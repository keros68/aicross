//! 无窗口派发编排。纯图变换与文件/进程边界集中在本模块。
//!
//! R1 起结构对齐 TS 参照（`src-ui/src/console-channel.ts` 的
//! `runConsoleDispatch` + `src-ui/src/graph/dispatch.ts`）：
//! - `plan_dispatch`：建图 + 盖章 + 派发前检查的纯函数，数据全部注入
//!   （模型清单 / 预设 / 角色 / 在飞数 / 已有图 / git 状态），生产与金标准
//!   重放走同一个函数。
//! - `run_pipeline`：逐阶段推进的注入式循环；落图、写 run 摘要、单阶段派发、
//!   取消信号都由调用方给（`PipelineSink` / 闭包）。
//! - 控制台派发的「惰性更新」语义按 TS 复刻：每次 commit 先落图、记一条待
//!   发更新，到下一个 await 点（preview 前 / dispatch 返回后 / 收尾前）才用
//!   当时的图与在飞清单把积攒的更新发出去。

use crate::dispatch::{
    self, DispatchArtifacts, DispatchNodeRequest, DispatchNodeResult, DispatchSettings,
    DispatchStageRequest, DispatchStageResult, FallbackSpec, PinnedState,
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
#[cfg(not(windows))]
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

const DUPLICATE_BATCH_TEXT: &str = "同一批结果重复送达，已停止推进；这一层的节点已放回待派。";
const MISSING_TARGET_TEXT: &str = "下一阶段有节点未指定模型，请先选择目标。";
const CANCELLED_TEXT: &str = "派发已取消";
const NO_EXTERNAL_ACTIONS: &str =
    "不得 git push、发布、安装依赖或调用付费接口；这些留给用户在验收之后做。";
const VERIFICATION_ROUTE_TEXT: &str =
    "盲审：独立审查同一份产物，只看材料与验证命令输出，不看验收结论。";
/// pid 为 0（父进程已写 run、后台进程尚未报到）的启动宽限。
const STARTUP_WINDOW_SECS: i64 = 30;

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

/// 从设置 JSON 对象夹取；字符串数字不认（与 TS `loadDispatchSettings` 的
/// `Number.isFinite` 口径一致），金标准重放与读文件共用这一份。
pub(crate) fn settings_from_value(map: &serde_json::Map<String, Value>) -> EngineSettings {
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
    settings_from_value(map)
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
    /// 后台进程自己的创建时间（Windows 为创建 FILETIME 的十进制串），
    /// 用于 pid 复用核对；旧 run 文件与非 Windows 平台为空。
    #[serde(default)]
    pub pid_started_at: String,
    /// 本次派发的原始需求文本，建图时写入，后台直接读，不从图上反推。
    #[serde(default)]
    pub original_request: String,
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

pub(crate) fn valid_id(id: &str) -> bool {
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

fn parse_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|t| t.with_timezone(&Utc))
}

/// pid 为 0 的 run 是否还在启动宽限内（updated_at 距现在不超过 30 秒）。
/// updated_at 解析不出来按已过期处理；比当前时间晚 60 秒以上（系统时钟回拨）
/// 同样按过期处理，否则死 run 会一直占着在飞名额（R2-10）。
fn within_startup_window(updated_at: &str) -> bool {
    parse_time(updated_at).is_some_and(|t| {
        let delta = (Utc::now() - t).num_seconds();
        (-60..=STARTUP_WINDOW_SECS).contains(&delta)
    })
}

/// 运行中判定：pid 为 0 看启动窗口，否则看进程存活（Windows 带创建时间核对）。
fn run_is_active(run: &EngineRun) -> bool {
    if run.status != "running" {
        return false;
    }
    if run.pid == 0 {
        return within_startup_window(&run.updated_at);
    }
    process_alive(run.pid, &run.pid_started_at)
}

pub(crate) fn read_run(root: &Path, run_id: &str) -> Result<EngineRun, String> {
    if !valid_id(run_id) {
        return Err("run_id 无效".into());
    }
    let path = run_path(root, run_id);
    let mut run: EngineRun =
        serde_json::from_slice(&fs::read(&path).map_err(|e| format!("读取 run 失败：{e}"))?)
            .map_err(|e| format!("解析 run 失败：{e}"))?;
    if run.status == "running" && !run_is_active(&run) {
        run.status = "failed".into();
        run.reason = if run.pid == 0 {
            "引擎进程未能启动"
        } else {
            "引擎进程已退出"
        }
        .into();
        run.updated_at = now();
        write_json_atomic(&path, &run)?;
    }
    Ok(run)
}

fn write_run(root: &Path, run: &EngineRun) -> Result<(), String> {
    write_json_atomic(&run_path(root, &run.run_id), run)
}

#[cfg(unix)]
fn process_alive(pid: u32, _pid_started_at: &str) -> bool {
    // 非 Windows 平台保持存在性检查（kill(pid, 0)），pid 复用核对不生效，见文档。
    if pid == 0 {
        return false;
    }
    let result = unsafe { libc::kill(pid as i32, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(windows)]
fn process_creation_time(handle: windows::Win32::Foundation::HANDLE) -> Option<u64> {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::GetProcessTimes;
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    let ok = unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) };
    if ok.is_err() {
        return None;
    }
    Some(((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64)
}

#[cfg(windows)]
fn process_alive(pid: u32, pid_started_at: &str) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    if pid == 0 {
        return false;
    }
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) })
    else {
        return false;
    };
    let creation = process_creation_time(handle);
    unsafe {
        let _ = CloseHandle(handle);
    }
    match (creation, pid_started_at.is_empty()) {
        // 有创建时间且 run 记录了创建时间：一致才算存活（pid 复用核对）。
        (Some(creation), false) => creation.to_string() == pid_started_at,
        // 记录缺失或读不到创建时间：退化为存在性检查。
        _ => true,
    }
}

/// 本进程自己的创建时间（Windows 为 FILETIME 十进制串）；其它平台为空串。
#[cfg(windows)]
fn own_process_started_at() -> String {
    use windows::Win32::System::Threading::GetCurrentProcess;
    let handle = unsafe { GetCurrentProcess() };
    process_creation_time(handle)
        .map(|t| t.to_string())
        .unwrap_or_default()
}

#[cfg(not(windows))]
fn own_process_started_at() -> String {
    String::new()
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn rfc3339(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn utf16_excerpt(text: &str, limit: usize) -> String {
    // JS slice 按 UTF-16 码元截断；若边界切中代理对，Rust String 无法承载孤立代理，
    // 这里用 U+FFFD 表示该半个码元，JSON 语义与 JS 序列化后的替换结果一致。
    String::from_utf16_lossy(&text.encode_utf16().take(limit).collect::<Vec<_>>())
}

/// UTF-16 码元口径的尾部窗口（JS `text.slice(-1200)`）。
fn utf16_tail(text: &str, limit: usize) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    String::from_utf16_lossy(&units[units.len().saturating_sub(limit)..])
}

pub(crate) fn node_summary(graph: &Graph, ids: impl IntoIterator<Item = String>) -> Vec<RunNode> {
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

pub(crate) fn new_graph(project: &str, id: String, root_id: String, time: String) -> Graph {
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
        nodes: indexmap::IndexMap::from([(root_id, root)]),
        created_at: time.clone(),
        updated_at: time,
        project: project.into(),
        ..Graph::default()
    }
}

/// 到根的最长路径深度（TS `layout.ts` 的 `depthMap`）：父节点最大深度 + 1，
/// 根为 0；环路防御按 0，悬空 parents 跳过。
fn depth_map(graph: &Graph) -> HashMap<String, u32> {
    fn depth(
        graph: &Graph,
        id: &str,
        memo: &mut HashMap<String, u32>,
        visiting: &mut HashSet<String>,
    ) -> u32 {
        if let Some(d) = memo.get(id) {
            return *d;
        }
        if !visiting.insert(id.to_string()) {
            return 0;
        }
        let Some(node) = graph.nodes.get(id) else {
            visiting.remove(id);
            return 0;
        };
        let mut d = 0;
        for parent in &node.parents {
            if !graph.nodes.contains_key(parent) {
                continue;
            }
            d = d.max(depth(graph, parent, memo, visiting) + 1);
        }
        visiting.remove(id);
        memo.insert(id.to_string(), d);
        d
    }
    let mut memo = HashMap::new();
    let mut visiting = HashSet::new();
    for id in graph.nodes.keys() {
        depth(graph, id, &mut memo, &mut visiting);
    }
    memo
}

/// 默认列末端（TS `latestLeaf`）：深度最大，同深取 created_at 严格较新者，
/// 跳过遗留节点；按插入序迭代（同深同创建时间时先到的留下）。
fn latest_leaf(graph: &Graph) -> String {
    let depths = depth_map(graph);
    let mut best = graph.root.clone();
    let mut best_depth: i64 = -1;
    for (id, node) in &graph.nodes {
        if node.leftover.is_some() {
            continue;
        }
        let d = depths.get(id).copied().unwrap_or(0) as i64;
        let better = if d > best_depth {
            true
        } else if d == best_depth {
            node.created_at.as_str()
                > graph
                    .nodes
                    .get(&best)
                    .map(|b| b.created_at.as_str())
                    .unwrap_or("")
        } else {
            false
        };
        if better {
            best = id.clone();
            best_depth = d;
        }
    }
    best
}

/// TS `appendConsoleMaterial`：根空则 fillRoot（写标题、标 console），否则挂到
/// 列末端。不写 updated_at（那是 commit 的事）。
pub(crate) fn append_material(
    graph: &Graph,
    text: &str,
    attachments: &[String],
    id: String,
    time: String,
) -> (Graph, String) {
    let mut next = graph.clone();
    let root_id = next.root.clone();
    let fill_root = next
        .nodes
        .get(&root_id)
        .is_some_and(|root| root.user.text.is_empty() && root.parents.is_empty());
    if fill_root {
        if next.title.is_empty() {
            let first = text.split('\n').next().unwrap_or("");
            let title = utf16_excerpt(first, 20);
            if !title.is_empty() {
                next.title = title;
            }
        }
        if let Some(root) = next.nodes.get_mut(&root_id) {
            root.user = Message {
                text: text.into(),
                attachments: attachments.to_vec(),
            };
            root.source = "console".into();
        }
        return (next, root_id);
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

/// 盖章时写进节点 user.text 的文本（TS `presetNodeText`）：拆分节点在模板后
/// 补一行本次件数上限（至少 1 件）。
fn preset_node_text(target: &Target, swarm_limit: usize) -> String {
    let text = target.preset_text.clone();
    if text.is_empty() || target.role != "拆分" {
        return text;
    }
    format!("{text}\n\n本次上限 {} 件。", swarm_limit.max(1))
}

/// TS `PRESET_NODE_TEXTS`：内置预设按扁平创建序覆盖节点文本；表里没有的位置
/// 不覆盖（保留 presetNodeText）。
fn preset_node_texts(preset_id: &str) -> Option<&'static [&'static str]> {
    match preset_id {
        "builtin-jury" => Some(&[
            "安全评审：只从安全视角审查这批材料，指出风险、暴露面与缓释建议。",
            "性能评审：只从性能视角审查这批材料，指出瓶颈、复杂度与改进建议。",
            "",
        ]),
        "builtin-economy" => Some(&["", "", "", VERIFICATION_ROUTE_TEXT]),
        "builtin-blind" => Some(&["", VERIFICATION_ROUTE_TEXT, VERIFICATION_ROUTE_TEXT]),
        _ => None,
    }
}

/// TS `stamp`：第 1 阶段每个 Target 成为 node_id 的子节点；第 k 阶段以第 k-1
/// 阶段全部新节点为父，父节点数 ≥2 时 kind=accept。同层 created_at 按阶段内
/// 索引递增 1ms。盖章后只有内置预设按扁平序套用 PRESET_NODE_TEXTS。
pub(crate) fn stamp_with(
    graph: &Graph,
    node_id: &str,
    preset: &Preset,
    swarm_limit: usize,
    ids: &mut dyn FnMut() -> String,
    base: DateTime<Utc>,
) -> (Graph, Vec<String>) {
    if !graph.nodes.contains_key(node_id) {
        return (graph.clone(), vec![]);
    }
    let mut next = graph.clone();
    let mut previous = vec![node_id.to_string()];
    let mut created: Vec<String> = vec![];
    for stage in &preset.stages {
        let mut level = vec![];
        for (index, target) in stage.nodes.iter().enumerate() {
            let id = ids();
            let created_at = rfc3339(base + chrono::Duration::milliseconds(index as i64));
            let node = GraphNode {
                id: id.clone(),
                kind: if previous.len() >= 2 {
                    "accept".into()
                } else {
                    "turn".into()
                },
                parents: previous.clone(),
                user: Message {
                    text: preset_node_text(target, swarm_limit),
                    attachments: vec![],
                },
                reply: Some(Reply {
                    status: "draft".into(),
                    ..Reply::default()
                }),
                target: Some(target.clone()),
                created_at,
                ..GraphNode::default()
            };
            next.nodes.insert(id.clone(), node);
            level.push(id.clone());
            created.push(id);
        }
        previous = level;
    }
    if preset.builtin {
        if let Some(texts) = preset_node_texts(&preset.id) {
            for (index, id) in created.iter().enumerate() {
                if let Some(text) = texts.get(index) {
                    if let Some(node) = next.nodes.get_mut(id) {
                        node.user.text = (*text).to_string();
                    }
                }
            }
        }
    }
    next.updated_at = rfc3339(base);
    (next, created)
}

/// TS `presetWithRoleDefaults` 精确口径：model 非空（trim）不动；命中角色默认时
/// agent/provider/model/tier/thinking 五个字段无条件覆写（provider 空串也写
/// Some("")，与 TS 写 '' 对齐）。擂台 / 盲审 / 评审预设整份跳过。
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
        target.provider = Some(profile.provider.clone());
        target.model.clone_from(&profile.model);
        target.tier = Some(profile.tier.clone());
        target.thinking.clone_from(&profile.thinking);
    }
    preset
}

fn sorted_nodes(graph: &Graph) -> Vec<&GraphNode> {
    let mut nodes: Vec<_> = graph.nodes.values().collect();
    nodes.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
    nodes
}

/// TS `dispatchableNodeIds`：draft + 有 target + 非遗留 + 全部父节点
/// material 或 done；按 created_at（再按 id）稳定排序。
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

/// 祖先闭包（含自身），保持 TS `ancestorClosure` 的插入序：自身在前，之后
/// 按栈式 DFS 首次到达序；悬空 parents 跳过。
fn ancestor_closure(graph: &Graph, node_id: &str) -> Vec<String> {
    let mut out = vec![node_id.to_string()];
    let mut stack: Vec<String> = graph
        .nodes
        .get(node_id)
        .map(|n| n.parents.clone())
        .unwrap_or_default();
    while let Some(id) = stack.pop() {
        if out.iter().any(|seen| seen == &id) {
            continue;
        }
        let Some(node) = graph.nodes.get(&id) else {
            continue;
        };
        out.push(id.clone());
        stack.extend(node.parents.iter().cloned());
    }
    out
}

fn ancestors(graph: &Graph, node_id: &str) -> HashSet<String> {
    ancestor_closure(graph, node_id).into_iter().collect()
}

/// TS `graphStage`：父节点最大阶段 + 1，根为 0；单次调用内记忆化，
/// 环与悬空按 0。
fn graph_stage(graph: &Graph, node_id: &str) -> u32 {
    fn visit(
        graph: &Graph,
        id: &str,
        memo: &mut HashMap<String, u32>,
        seen: &mut HashSet<String>,
    ) -> u32 {
        if let Some(d) = memo.get(id) {
            return *d;
        }
        if seen.contains(id) {
            return 0;
        }
        let Some(node) = graph.nodes.get(id) else {
            return 0;
        };
        if node.parents.is_empty() {
            return 0;
        }
        seen.insert(id.to_string());
        let depth = 1 + node
            .parents
            .iter()
            .map(|p| visit(graph, p, memo, seen))
            .max()
            .unwrap_or(0);
        seen.remove(id);
        memo.insert(id.to_string(), depth);
        depth
    }
    visit(graph, node_id, &mut HashMap::new(), &mut HashSet::new())
}

/// 子节点（TS `childrenOf`）：按 nodes 插入序。
fn children_of<'a>(graph: &'a Graph, node_id: &str) -> Vec<&'a GraphNode> {
    graph
        .nodes
        .values()
        .filter(|n| n.parents.iter().any(|p| p == node_id))
        .collect()
}

/// TS `orderByDfs`：从根深度优先遍历，只保留 keep 里的节点；不在 keep 里的
/// 节点直接剪枝（子树不从它走），多父去重按首达。
fn order_by_dfs(graph: &Graph, keep: &HashSet<String>) -> Vec<String> {
    fn visit(
        graph: &Graph,
        id: &str,
        keep: &HashSet<String>,
        seen: &mut HashSet<String>,
        out: &mut Vec<String>,
    ) {
        if !keep.contains(id) || !seen.insert(id.to_string()) {
            return;
        }
        out.push(id.to_string());
        for child in children_of(graph, id) {
            visit(graph, &child.id, keep, seen, out);
        }
    }
    let mut out = vec![];
    let mut seen = HashSet::new();
    if graph.nodes.contains_key(&graph.root) {
        visit(graph, &graph.root, keep, &mut seen, &mut out);
    }
    out
}

/// 上下文顺序（TS `orderByDfs` + 自身附加在末尾）：祖先闭包内的 DFS 序，
/// 自身最后。
fn context_order(graph: &Graph, node_id: &str) -> Vec<String> {
    let keep = ancestors(graph, node_id);
    let mut out = order_by_dfs(graph, &keep);
    out.retain(|id| id != node_id);
    out.push(node_id.into());
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

/// 盲节点附件的显示名（TS `path.split(/[\\/]/).pop() || path`）：取最后一段，
/// 最后一段为空（路径以分隔符结尾）时回原路径。
fn blind_attachment_name(path: &str) -> String {
    path.split(['\\', '/'])
        .next_back()
        .filter(|last| !last.is_empty())
        .unwrap_or(path)
        .to_string()
}

/// TS `serializeContext`（无选择态）：用户消息部分只受「指定上游」过滤——盲
/// 节点照样带上全部祖先的 [用户] 段与附件；模型回复部分才受盲的
/// defaultSelected 约束（非直接父回复不进 prompt）。盲节点直接父有 artifacts
/// 时跳过其回复（产物走目录）。materials 按首次出现序去重，不排序。
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
    let mut materials: Vec<String> = vec![];
    for id in context_order(graph, node_id) {
        let Some(node) = graph.nodes.get(&id) else {
            continue;
        };
        let keep_user =
            visibility != "指定上游" || id == node_id || upstreams.contains(id.as_str());
        if keep_user && (!node.user.text.is_empty() || !node.user.attachments.is_empty()) {
            if !node.user.text.is_empty() {
                blocks.push(format!("[用户]\n{}", node.user.text));
            }
            if !node.user.attachments.is_empty() {
                for path in &node.user.attachments {
                    if !materials.contains(path) {
                        materials.push(path.clone());
                    }
                }
                let shown = if visibility == "盲" {
                    node.user
                        .attachments
                        .iter()
                        .map(|p| blind_attachment_name(p))
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
        let keep_reply = if visibility == "盲" && parents.contains(id.as_str()) {
            true
        } else if visibility == "共享" {
            true
        } else if visibility == "盲" {
            false // defaultSelected = false：不进 prompt
        } else {
            upstreams.contains(id.as_str())
        };
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

/// TS `upstreamArtifactsOf`：第一轮只在允许集内 DFS（照 `orderByDfs(graph,
/// allowed)`，不在允许集的节点连同子树剪枝）；再把允许集里没贡献过产物的
/// 节点按允许集顺序兑底（悬空父这类从根走不到的情况）；同一节点的产物只进一次。
fn upstream_artifacts(graph: &Graph, node_id: &str) -> Vec<DispatchArtifacts> {
    let convert = |a: &ReplyArtifacts| DispatchArtifacts {
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
    };
    let Some(node) = graph.nodes.get(node_id) else {
        return vec![];
    };
    let allowed: Vec<String> = match node
        .target
        .as_ref()
        .map(|t| t.visibility.as_str())
        .unwrap_or("共享")
    {
        "盲" => node.parents.clone(),
        "指定上游" => node
            .target
            .as_ref()
            .and_then(|t| t.upstreams.clone())
            .unwrap_or_default(),
        _ => ancestor_closure(graph, node_id),
    };
    let allowed_set: HashSet<String> = allowed.iter().cloned().collect();
    let mut contributed: HashSet<String> = HashSet::new();
    let mut out = vec![];
    // 第一轮只在允许集内 DFS（TS `orderByDfs(graph, allowed)`）：不在允许集里的
    // 节点连同子树剪掉，允许集里从根走不到的节点由后面的兑底按允许集顺序补。
    for id in order_by_dfs(graph, &allowed_set) {
        if let Some(artifacts) = graph
            .nodes
            .get(&id)
            .and_then(|n| n.reply.as_ref())
            .and_then(|r| r.artifacts.as_ref())
        {
            contributed.insert(id.clone());
            out.push(convert(artifacts));
        }
    }
    for id in &allowed {
        if contributed.contains(id) {
            continue;
        }
        if let Some(artifacts) = graph
            .nodes
            .get(id)
            .and_then(|n| n.reply.as_ref())
            .and_then(|r| r.artifacts.as_ref())
        {
            out.push(convert(artifacts));
        }
    }
    out
}

/// 「已切备选」整行（TS `fallbackNoticeOf`）：首选 fallback_from 字段；旧图
/// 只有正文首行标记的按标记识别。
fn fallback_notice(reply: &Reply) -> bool {
    if reply.fallback_from.as_deref().is_some_and(|s| !s.is_empty()) {
        return true;
    }
    reply
        .text
        .split('\n')
        .next()
        .unwrap_or("")
        .starts_with("（已切备选：")
}

/// TS `continuationSession` + `continuationRunner`：共享节点、单父、父不是盲、
/// 父回复不是备选作答（字段或旧图首行标记）、目标完全一致（provider 缺省与
/// 空串视为相等）才续会话；runner 只认 acp / oneshot。
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
    let same_target = pt.agent == target.agent
        && pt.provider.as_deref().unwrap_or("") == target.provider.as_deref().unwrap_or("")
        && pt.model == target.model;
    if pt.visibility == "盲"
        || fallback_notice(reply)
        || !same_target
    {
        return (String::new(), String::new());
    }
    let session = reply.session_id.clone().unwrap_or_default();
    if session.is_empty() {
        return (String::new(), String::new());
    }
    let runner = reply
        .runner
        .clone()
        .filter(|v| matches!(v.as_str(), "acp" | "oneshot"))
        .unwrap_or_default();
    (session, runner)
}

/// R1-2 口径（TS `isFixRoundExecution`）：只有角色「执行」且正文以
/// `[修正 N/M]` 或「根据上一轮验收意见修正实现。」开头的节点才置 fix_round。
fn is_fix_round_execution(graph: &Graph, node_id: &str) -> bool {
    let Some(node) = graph.nodes.get(node_id) else {
        return false;
    };
    if node.target.as_ref().is_none_or(|t| t.role != "执行") {
        return false;
    }
    let text = node.user.text.as_str();
    if text.starts_with("根据上一轮验收意见修正实现。") {
        return true;
    }
    let Some(rest) = text.strip_prefix("[修正 ") else {
        return false;
    };
    let Some(end) = rest.find(']') else {
        return false;
    };
    let mut parts = rest[..end].split('/');
    matches!(parts.next(), Some(a) if !a.is_empty() && a.bytes().all(|b| b.is_ascii_digit()))
        && matches!(parts.next(), Some(b) if !b.is_empty() && b.bytes().all(|c| c.is_ascii_digit()))
        && parts.next().is_none()
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

/// TS `buildNodeRequest` + `runConsoleDispatch` 的 buildRequest：模型查找只按
/// provider 精确匹配（vendor / billing_source / tier 兜底用）；备选 vendor 仍是
/// 两段式（先 provider 精确再退 agent+id）；每个节点的 stage 取各自 graph_stage，
/// 请求级取最小值。
fn build_request(
    run_id: &str,
    project: &str,
    settings: &EngineSettings,
    graph: &Graph,
    ids: &[String],
    models: &[ResourceModel],
    original: &str,
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
            let provider = target.provider.clone().unwrap_or_default();
            let model = models.iter().find(|m| {
                m.agent == target.agent && m.id == target.model && m.provider == provider
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
                .filter(|f| !f.model.trim().is_empty())
                .map(|f| FallbackSpec {
                    agent: f.agent.clone(),
                    provider: f.provider.clone().unwrap_or_default(),
                    model: f.model.clone(),
                    vendor: models
                        .iter()
                        .find(|m| {
                            m.agent == f.agent
                                && m.id == f.model
                                && m.provider == f.provider.clone().unwrap_or_default()
                        })
                        .or_else(|| {
                            models
                                .iter()
                                .find(|m| m.agent == f.agent && m.id == f.model)
                        })
                        .map(|m| m.vendor.clone())
                        .unwrap_or_default(),
                });
            let (session_id, previous_runner) = continuation(graph, id);
            Some(DispatchNodeRequest {
                node_id: id.clone(),
                stage: graph_stage(graph, id),
                agent: target.agent.clone(),
                provider,
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
                fix_round: is_fix_round_execution(graph, id),
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

/// TS `markPending`：保留原 reply 全部字段，只把 status 改成 pending；
/// 没有 reply 的节点补一个只有 text 的 reply。
fn mark_pending(graph: &Graph, ids: &[String]) -> Graph {
    let mut next = graph.clone();
    for id in ids {
        if let Some(n) = next.nodes.get_mut(id) {
            let mut reply = n.reply.clone().unwrap_or_default();
            reply.status = "pending".into();
            n.reply = Some(reply);
        }
    }
    next
}

/// TS `resetNodesToDraft`：只处理 pending 节点，保留 reply 其余字段。
fn reset_nodes_to_draft(graph: &Graph, ids: &[String]) -> Graph {
    let mut next = graph.clone();
    for id in ids {
        if let Some(n) = next.nodes.get_mut(id) {
            if n.reply.as_ref().is_some_and(|r| r.status == "pending") {
                if let Some(reply) = n.reply.as_mut() {
                    reply.status = "draft".into();
                }
            }
        }
    }
    next
}

/// TS `applyDispatchResults`。fallback_from 非空时 fallback_to 总写（可为空串）；
/// quota_resets_at 空串滤掉。
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
            fallback_to: (!result.fallback_from.is_empty()).then(|| result.fallback_to.clone()),
            quota_resets_at: result
                .quota_resets_at
                .clone()
                .filter(|s| !s.is_empty()),
            ..Reply::default()
        });
    }
    next
}

/// TS `parseAcceptance`：尾部 1200 个 UTF-16 码元内取最后一个
/// `\b(?:PASS|NEEDS_FIX)\b`（大写后），没有则 UNKNOWN。
fn parse_acceptance(text: &str) -> &'static str {
    let upper = utf16_tail(text, 1200).to_uppercase();
    let mut verdict = "UNKNOWN";
    for word in upper.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
        if word == "PASS" {
            verdict = "PASS";
        } else if word == "NEEDS_FIX" {
            verdict = "NEEDS_FIX";
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

/// TS `appendCorrectionRound`：previous 取 accept.parents 里第一个角色「执行」
/// 的节点；它没有产物就不生成修正轮（不跳到别的执行父）。验证兄弟跟着长，
/// upstreams 清掉。
fn append_correction(
    graph: &Graph,
    accept_id: &str,
    ids: &mut dyn FnMut() -> String,
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
        .find(|n| n.target.as_ref().is_some_and(|t| t.role == "执行"))
    else {
        return (graph.clone(), vec![]);
    };
    if previous
        .reply
        .as_ref()
        .and_then(|r| r.artifacts.as_ref())
        .is_none()
    {
        return (graph.clone(), vec![]);
    }
    let exec_id = ids();
    let acceptance_id = ids();
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
        created_at: rfc3339(base),
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
        created_at: rfc3339(base + chrono::Duration::milliseconds(1)),
        ..GraphNode::default()
    };
    next.nodes.insert(acceptance_id.clone(), acceptance);
    let mut made = vec![exec_id.clone(), acceptance_id];
    for (index, sibling) in verification_siblings(graph, accept).into_iter().enumerate() {
        let id = ids();
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
                created_at: rfc3339(base + chrono::Duration::milliseconds(2 + index as i64)),
                ..GraphNode::default()
            },
        );
        made.push(id)
    }
    (next, made)
}

/// TS `acceptanceRound`：沿祖先数角色「验收」且在 scope 里的节点数（含自身）。
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

/// TS `leftoverItems`：行 trim 后匹配 `-`/`*`/`•` + 空白、数字 + `.` / `)` / `、`、
/// `（N）`、`(N)` 才算条目；取不到条目就整段按行前 20。
fn leftover_items(text: &str) -> Vec<String> {
    let is_bullet = |line: &str| {
        let mut chars = line.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        if matches!(first, '-' | '*' | '•') {
            return chars.next().is_some_and(|c| c.is_whitespace());
        }
        if first == '（' || first == '(' {
            let rest: String = chars.collect();
            let close = if first == '（' { '）' } else { ')' };
            let Some(end) = rest.find(close) else {
                return false;
            };
            return !rest[..end].is_empty()
                && rest[..end].bytes().all(|b| b.is_ascii_digit());
        }
        if first.is_ascii_digit() {
            let digits: String = line.chars().take_while(|c| c.is_ascii_digit()).collect();
            let rest = &line[digits.len()..];
            let Some(mark) = rest.chars().next() else {
                return false;
            };
            return matches!(mark, '.' | ')' | '、');
        }
        false
    };
    let lines: Vec<_> = text
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let bullets: Vec<_> = lines.iter().filter(|line| is_bullet(line)).copied().collect();
    (if bullets.is_empty() { lines } else { bullets })
        .into_iter()
        .take(20)
        .map(str::to_string)
        .collect()
}

/// 遗留节点那一行说明（TS `leftoverLine`）。
fn leftover_line(count: usize, source: Option<&GraphNode>) -> String {
    let target = source.and_then(|n| n.target.as_ref());
    let route = target
        .filter(|t| !t.agent.trim().is_empty() && !t.model.trim().is_empty())
        .map(|t| format!("（{}/{}）", t.agent, t.model))
        .unwrap_or_default();
    let role = target
        .map(|t| t.role.as_str())
        .filter(|role| !role.is_empty())
        .unwrap_or("验收");
    format!("遗留：{count} 条未闭合，来源 {role}{route}")
}

/// TS `appendLeftover`：同一 source_node_id 已有遗留节点就复用其 id 覆写
/// （created_at 也更新）。
fn append_leftover(
    graph: &Graph,
    accept_id: &str,
    ids: &mut dyn FnMut() -> String,
    time: String,
) -> (Graph, Option<String>) {
    let Some(accept) = graph.nodes.get(accept_id) else {
        return (graph.clone(), None);
    };
    let items = leftover_items(accept.reply.as_ref().map(|r| r.text.as_str()).unwrap_or(""));
    if items.is_empty() {
        return (graph.clone(), None);
    }
    let existing = graph
        .nodes
        .values()
        .find(|n| n.leftover.as_ref().is_some_and(|l| l.source_node_id == accept_id))
        .map(|n| n.id.clone());
    let id = existing.unwrap_or_else(&mut *ids);
    let node = GraphNode {
        id: id.clone(),
        kind: "turn".into(),
        parents: vec![accept_id.into()],
        user: Message {
            text: leftover_line(items.len(), Some(accept)),
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
    next.nodes.insert(id.clone(), node);
    (next, Some(id))
}

/// 等待中的验收节点（TS 各处 `Object.values(graph.nodes).find(...)` 的口径）：
/// parents 含 execution_id、角色「验收」、reply draft，按插入序取第一个。
fn waiting_acceptance<'a>(graph: &'a Graph, execution_id: &str) -> Option<&'a GraphNode> {
    graph.nodes.values().find(|n| {
        n.parents.iter().any(|p| p == execution_id)
            && n.target.as_ref().is_some_and(|t| t.role == "验收")
            && n.reply.as_ref().is_some_and(|r| r.status == "draft")
    })
}

/// TS `appendValidationRetry`：等待中的验收标 failed（「验证命令未通过，未送审」/
/// 「测试未钉住，未送审」），同批 draft 的验证兄弟标 skipped_reason，新建
/// `[修正 N/M]` 执行 + 验收两节点（不长验证兄弟）。
#[allow(clippy::too_many_arguments)]
fn append_validation_retry(
    graph: &Graph,
    execution_id: &str,
    detail: &str,
    count: usize,
    limit: usize,
    unpinned: bool,
    ids: &mut dyn FnMut() -> String,
    base: DateTime<Utc>,
) -> (Graph, Vec<String>) {
    let Some(execution) = graph.nodes.get(execution_id) else {
        return (graph.clone(), vec![]);
    };
    let Some(waiting) = waiting_acceptance(graph, execution_id).map(|n| n.id.clone()) else {
        return (graph.clone(), vec![]);
    };
    let reason = if unpinned {
        "上游测试未钉住"
    } else {
        "上游验证未通过"
    };
    let mut next = graph.clone();
    let siblings: Vec<String> = verification_siblings(
        graph,
        graph.nodes.get(&waiting).expect("waiting acceptance exists"),
    )
    .into_iter()
    .filter(|n| n.reply.as_ref().is_some_and(|r| r.status == "draft"))
    .map(|n| n.id.clone())
    .collect();
    for id in siblings {
        if let Some(n) = next.nodes.get_mut(&id) {
            n.reply = Some(Reply {
                status: "draft".into(),
                skipped_reason: Some(reason.into()),
                ..Reply::default()
            })
        }
    }
    if let Some(n) = next.nodes.get_mut(&waiting) {
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
    let exec_id = ids();
    let accept_id = ids();
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
            created_at: rfc3339(base),
            ..GraphNode::default()
        },
    );
    let waiting_target = next
        .nodes
        .get(&waiting)
        .and_then(|n| n.target.clone());
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
            target: waiting_target,
            created_at: rfc3339(base + chrono::Duration::milliseconds(1)),
            ..GraphNode::default()
        },
    );
    (next, vec![exec_id, accept_id])
}

/// TS `closeWaitingAcceptance`：重试到顶时，等待中的验收与同批 draft 的验证
/// 兄弟一并写跳过说明，status 保持 draft。
fn close_waiting_acceptance(graph: &Graph, execution_id: &str, reason: &str) -> (Graph, bool) {
    let Some(waiting) = waiting_acceptance(graph, execution_id).cloned() else {
        return (graph.clone(), false);
    };
    let mut ids = vec![waiting.id.clone()];
    ids.extend(
        verification_siblings(graph, &waiting)
            .into_iter()
            .filter(|n| n.reply.as_ref().is_some_and(|r| r.status == "draft"))
            .map(|n| n.id.clone()),
    );
    let mut next = graph.clone();
    for id in ids {
        if let Some(n) = next.nodes.get_mut(&id) {
            n.reply = Some(Reply {
                status: "draft".into(),
                skipped_reason: Some(reason.into()),
                ..Reply::default()
            })
        }
    }
    (next, true)
}

/// TS `reducePipelineStep`。返回（新图，是否继续推进，是否有改动，新建节点 id）。
/// 新建节点 id 由调用方补进 scope（TS 的 fixRoundNodeIds 在 reduce 之后才加）。
fn reduce_step(
    graph: &Graph,
    results: &[DispatchNodeResult],
    retries: &mut HashMap<String, usize>,
    limit: usize,
    scope: &HashSet<String>,
    ids: &mut dyn FnMut() -> String,
    clock: &mut dyn FnMut() -> DateTime<Utc>,
) -> (Graph, bool, bool, Vec<String>) {
    let mut next = graph.clone();
    let mut changed = false;
    let mut new_ids: Vec<String> = vec![];
    let mut added_correction = false;
    for result in results {
        let is_acceptance = next
            .nodes
            .get(&result.node_id)
            .and_then(|n| n.target.as_ref())
            .is_some_and(|t| t.role == "验收");
        if !is_acceptance || result.status != "ok" || parse_acceptance(&result.text) != "NEEDS_FIX"
        {
            continue;
        }
        // 验收提出修正时优先生成下一轮；同批盲验证的 error / timeout 只保留在该路结果上。
        if acceptance_round(&next, &result.node_id, scope) > limit {
            let (g, made) = append_leftover(&next, &result.node_id, ids, rfc3339(clock()));
            if made.is_some() {
                changed = true;
            }
            next = g;
            continue;
        }
        let (g, made) = append_correction(&next, &result.node_id, ids, clock());
        if !made.is_empty() {
            added_correction = true;
            changed = true;
            new_ids.extend(made);
        }
        next = g;
    }
    if added_correction {
        return (next, true, changed, new_ids);
    }
    let failures: Vec<_> = results
        .iter()
        .filter(|r| r.status == "validation_failed" || r.pinned == Some(PinnedState::Unpinned))
        .collect();
    if !failures.is_empty() {
        let mut added_retry = false;
        for failure in failures {
            let count = *retries.get(&failure.node_id).unwrap_or(&0);
            let unpinned = failure.pinned == Some(PinnedState::Unpinned);
            if count >= limit {
                let (g, closed) = close_waiting_acceptance(
                    &next,
                    &failure.node_id,
                    if unpinned {
                        "上游测试未钉住"
                    } else {
                        "上游验证未通过"
                    },
                );
                changed |= closed;
                next = g;
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
                unpinned,
                ids,
                clock(),
            );
            if let Some(id) = made.first() {
                retries.insert(id.clone(), count + 1);
                added_retry = true;
                changed = true;
                new_ids.extend(made);
            }
            next = g
        }
        return (next, added_retry, changed, new_ids);
    }
    if results.iter().any(|r| r.status != "ok") {
        return (next, false, changed, new_ids);
    }
    let has_acceptance = results.iter().any(|r| {
        next.nodes
            .get(&r.node_id)
            .and_then(|n| n.target.as_ref())
            .is_some_and(|t| t.role == "验收")
    });
    (next, !has_acceptance, changed, new_ids)
}

// ── 派发前检查（plan_dispatch） ────────────────────────────────────────────

/// TS `normalizedPath` 逐条：`\\?\UNC/` → `//`、剥 `\\?\`、`\` → `/`、
/// 去尾部 `/`、小写。前缀剥除按字符边界取子串：路径开头含多字节字符时
/// 算不匹配，而不是按字节切片 panic（R2-1，`pick_project_graph` 会对图目录里
/// 每张图都过这一步）。
fn normalized(value: &str) -> String {
    fn strip_prefix_ci<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
        // get(..n) 在切点不是字符边界时返回 None；前缀本身是 ASCII，长度即字节数。
        value
            .get(..prefix.len())
            .filter(|head| head.eq_ignore_ascii_case(prefix))
            .map(|_| &value[prefix.len()..])
    }
    let stripped = if let Some(rest) = strip_prefix_ci(value, r"\\?\UNC\") {
        format!("//{rest}")
    } else if let Some(rest) = strip_prefix_ci(value, r"\\?\") {
        rest.to_string()
    } else {
        value.to_string()
    };
    stripped
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase()
}

/// TS `selectConsoleGraph` 的无窗口半边：同项目（归一化路径）的图里取
/// updated_at 最新的一张。
fn pick_project_graph(graphs: Vec<Graph>, project: &str) -> Option<Graph> {
    let wanted = normalized(project);
    let mut graphs: Vec<Graph> = graphs
        .into_iter()
        .filter(|g| normalized(&g.project) == wanted)
        .collect();
    graphs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    graphs.into_iter().next()
}

fn latest_project_graph(root: &Path, project: &str) -> Option<Graph> {
    let dir = root.join("graphs");
    let graphs: Vec<Graph> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|e| fs::read(e.path()).ok())
        .filter_map(|b| serde_json::from_slice(&b).ok())
        .collect();
    pick_project_graph(graphs, project)
}

/// TS `validateNode` 的拦截半边：只产「只能只读」/「盲节点只能只读」/「未接入」。
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

/// TS `validate.ts` 的 findModel：agent 留空时只按 model 匹配。
fn find_model<'a>(models: &'a [ResourceModel], target: &Target) -> Option<&'a ResourceModel> {
    models
        .iter()
        .find(|m| m.id == target.model && (target.agent.is_empty() || m.agent == target.agent))
}

/// 派发前检查里的两段式查找（TS `quotaBlockedNodeIds`）：先 provider 精确，
/// 再退 agent+id。
fn find_model_two_level<'a>(
    models: &'a [ResourceModel],
    target: &Target,
) -> Option<&'a ResourceModel> {
    let provider = target.provider.clone().unwrap_or_default();
    models
        .iter()
        .find(|m| m.agent == target.agent && m.id == target.model && m.provider == provider)
        .or_else(|| {
            models
                .iter()
                .find(|m| m.agent == target.agent && m.id == target.model)
        })
}

/// TS `quotaExhaustedUntil`：trim 后非空、能解析、晚于 now 才算额度用尽中。
fn quota_active_until(value: &Option<String>, now: DateTime<Utc>) -> bool {
    value
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .is_some_and(|t| t.with_timezone(&Utc) > now)
}

pub(crate) struct PlanInput<'a> {
    pub run_id: &'a str,
    pub project: &'a str,
    pub text: &'a str,
    pub preset: Option<&'a str>,
    pub attachments: &'a [String],
    pub targets: &'a [Target],
    pub settings: &'a EngineSettings,
    pub models: &'a [ResourceModel],
    pub resources_error: Option<&'a str>,
    /// 原始预设清单（未填角色默认；角色默认由这里自己套）。
    pub presets: &'a [Preset],
    pub roles: &'a RoleSettings,
    /// 不含本 run 的在飞 run 数。
    pub active_runs: usize,
    /// 同项目已有图（selectConsoleGraph 的结果）。
    pub existing: Option<Graph>,
    /// git 脏文件数（uncommitted + untracked）；读不出 git 状态时返回 None。
    /// 惰性：只有首批含 worktree 节点时才调用。
    pub git_dirty_count: &'a mut dyn FnMut() -> Option<usize>,
}

pub(crate) enum Plan {
    Rejected { reason: String, graph_id: String },
    Ready {
        graph: Graph,
        created: Vec<String>,
        original: String,
    },
}

/// `runConsoleDispatch` 的建图 + 盖章 + 派发前检查，拒绝顺序与 TS 一致：
/// 在飞上限 → 读取资源失败 → 找不到预设 →（建图盖章）→ 角色没有可派发目标 →
/// 未接入/只能只读 → 目标验证失败 → 目标额度用尽 → 总请求上限 → git 脏 + worktree。
pub(crate) fn plan_dispatch(
    input: &mut PlanInput,
    ids: &mut dyn FnMut() -> String,
    clock: &mut dyn FnMut() -> DateTime<Utc>,
) -> Plan {
    let reject = |reason: String, graph_id: String| Plan::Rejected { reason, graph_id };
    if input.active_runs >= input.settings.console_run_limit {
        return reject(
            format!("同时在飞 run 已达上限 {}", input.settings.console_run_limit),
            String::new(),
        );
    }
    if let Some(error) = input.resources_error {
        return reject(format!("读取资源失败：{error}"), String::new());
    }
    let selected = if !input.targets.is_empty() {
        Some(Preset {
            id: format!("console-{}", input.run_id),
            name: String::new(),
            builtin: false,
            stages: vec![orchestration::Stage {
                nodes: input.targets.to_vec(),
            }],
        })
    } else {
        let wanted = input.preset.unwrap_or("builtin-economy");
        input.presets.iter().find(|p| p.id == wanted).cloned()
    };
    let Some(selected) = selected else {
        return reject(
            format!(
                "找不到预设：{}",
                input.preset.unwrap_or("builtin-economy")
            ),
            String::new(),
        );
    };
    let preset = apply_role_defaults(selected, input.roles);
    let existing = input.existing.take();
    // R1-16：只有新建图标「来自控制台」，续写已有图保留原 source（含空）。
    let graph_source = existing
        .as_ref()
        .map(|g| g.source.clone())
        .unwrap_or_else(|| "console".into());
    let base = existing.unwrap_or_else(|| {
        // 与 TS newGraph 同一取 id 顺序：先 root 后图 id。
        let root_id = ids();
        let id = ids();
        new_graph(input.project, id, root_id, rfc3339(clock()))
    });
    let base = Graph {
        project: input.project.into(),
        source: graph_source,
        ..base
    };
    let (material, material_id) = append_material(
        &base,
        input.text,
        input.attachments,
        ids(),
        rfc3339(clock()),
    );
    let original = material
        .nodes
        .get(&material_id)
        .map(|n| n.user.text.clone())
        .unwrap_or_else(|| {
            material
                .nodes
                .get(&material.root)
                .map(|n| n.user.text.clone())
                .unwrap_or_default()
        });
    let (mut graph, created) = stamp_with(
        &material,
        &material_id,
        &preset,
        input.settings.swarm_limit,
        ids,
        clock(),
    );
    for id in &created {
        if let Some(node) = graph.nodes.get_mut(id) {
            node.source = "console".into();
        }
    }
    let first: Vec<String> = dispatchable_ids(&graph)
        .into_iter()
        .filter(|id| created.contains(id))
        .collect();
    if let Some(id) = first.iter().find(|id| {
        graph
            .nodes
            .get(*id)
            .and_then(|n| n.target.as_ref())
            .is_none_or(|t| t.agent.trim().is_empty() || t.model.trim().is_empty())
    }) {
        let role = graph
            .nodes
            .get(id)
            .and_then(|n| n.target.as_ref())
            .map(|t| t.role.as_str())
            .unwrap_or("");
        return reject(format!("角色「{role}」没有可派发目标"), graph.id.clone());
    }
    if let Some(hint) = first
        .iter()
        .filter_map(|id| validate_block(&graph, id, input.models))
        .find(|h| h == "未接入" || h.contains("只能只读"))
    {
        return reject(hint, graph.id.clone());
    }
    // 目标验证失败与恢复时间都用 agent+id 严格匹配（不看 provider）。
    let strict = |t: &Target| {
        input
            .models
            .iter()
            .find(|m| m.agent == t.agent && m.id == t.model)
    };
    if let Some(id) = first.iter().find(|id| {
        graph
            .nodes
            .get(*id)
            .and_then(|n| n.target.as_ref())
            .and_then(|t| strict(t))
            .is_some_and(|m| m.verification == "failed")
    }) {
        let t = graph
            .nodes
            .get(id)
            .and_then(|n| n.target.as_ref())
            .expect("target checked above");
        return reject(
            format!("目标验证失败：{}/{}", t.agent, t.model),
            graph.id.clone(),
        );
    }
    let quota_now = clock();
    if let Some(id) = first.iter().find(|id| {
        graph
            .nodes
            .get(*id)
            .and_then(|n| n.target.as_ref())
            .and_then(|t| find_model_two_level(input.models, t))
            .is_some_and(|m| quota_active_until(&m.quota_exhausted_until, quota_now))
    }) {
        let t = graph
            .nodes
            .get(id)
            .and_then(|n| n.target.as_ref())
            .expect("target checked above");
        let reset = strict(t)
            .and_then(|m| m.quota_exhausted_until.as_ref())
            .filter(|s| !s.is_empty())
            .map(|s| format!("；恢复时间 {s}"))
            .unwrap_or_default();
        return reject(
            format!("目标额度用尽：{}/{}{reset}", t.agent, t.model),
            graph.id.clone(),
        );
    }
    if created.len() > input.settings.max_requests {
        return reject(
            format!(
                "本次派发 {} 路，超过总请求上限 {}",
                created.len(),
                input.settings.max_requests
            ),
            graph.id.clone(),
        );
    }
    let has_worktree = first.iter().any(|id| {
        graph
            .nodes
            .get(id)
            .and_then(|n| n.target.as_ref())
            .is_some_and(|t| t.write == "worktree")
    });
    if has_worktree {
        if let Some(count) = (input.git_dirty_count)() {
            if count > 0 {
                return reject(
                    format!("项目目录有 {count} 处未提交改动；worktree 从 HEAD 新建，看不到这些改动"),
                    graph.id.clone(),
                );
            }
        }
    }
    Plan::Ready {
        graph,
        created,
        original,
    }
}

// ── 逐阶段推进（run_pipeline） ─────────────────────────────────────────────

/// 与 `ConsoleRunUpdate` 同形状的 run 摘要。
#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub(crate) struct RunUpdate {
    pub run_id: String,
    pub graph_id: String,
    pub status: String,
    pub nodes: Vec<RunNode>,
    pub leftover: Vec<String>,
    pub in_flight_node_ids: Vec<String>,
    pub reason: String,
}

pub(crate) trait PipelineSink {
    /// 落图。返回 `Some(合并后的图)` 时，推进循环改用它继续（同 TS `commit`
    /// 返回合并结果、`state.graph` 随之更新）；不做合并的 sink 返回 `None`。
    fn save_graph(&mut self, graph: &Graph) -> Result<Option<Graph>, String>;
    fn update(&mut self, update: &RunUpdate) -> Result<(), String>;
    fn dispatch(&mut self, request: &DispatchStageRequest) -> Result<DispatchStageResult, String>;
    /// 在飞节点集合变化时通知（生产用来让取消 watcher 知道该取消谁）。
    fn note_in_flight(&mut self, _ids: &[String]) {}
}

/// 推进循环的结果。生产路径（FileSink）已把图与摘要逐次落盘，这两个字段
/// 只有逐夹具重放测试读取。
#[allow(dead_code)]
pub(crate) struct PipelineOutcome {
    pub graph: Graph,
    pub error: String,
}

fn make_update(
    run_id: &str,
    graph: &Graph,
    status: &str,
    all_run_ids: &[String],
    leftover: Vec<String>,
    in_flight: Vec<String>,
    reason: &str,
) -> RunUpdate {
    RunUpdate {
        run_id: run_id.into(),
        graph_id: graph.id.clone(),
        status: status.into(),
        nodes: node_summary(graph, all_run_ids.iter().cloned()),
        leftover,
        in_flight_node_ids: in_flight,
        reason: reason.into(),
    }
}

fn leftover_lines(graph: &Graph) -> Vec<String> {
    sorted_nodes(graph)
        .into_iter()
        .filter_map(|n| n.leftover.as_ref())
        .flat_map(|l| l.items.clone())
        .collect()
}

/// `startPipelineHeadless` + `runStageLoop` 的注入式移植。commit 的惰性更新
/// 用计数复刻：每次 commit 落图并记一条，到 preview 前 / dispatch 返回后 /
/// 收尾 save 前用当时的图、all_run_ids、in_flight 补发。
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_pipeline(
    run_id: &str,
    project: &str,
    settings: &EngineSettings,
    mut graph: Graph,
    created: &[String],
    models: &[ResourceModel],
    original: &str,
    sink: &mut dyn PipelineSink,
    ids: &mut dyn FnMut() -> String,
    clock: &mut dyn FnMut() -> DateTime<Utc>,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<PipelineOutcome, String> {
    let mut all_run_ids: Vec<String> = vec![];
    let mut in_flight: Vec<String> = vec![];
    let push_unique = |list: &mut Vec<String>, id: &str| {
        if !list.iter().any(|seen| seen == id) {
            list.push(id.to_string());
        }
    };
    for id in created {
        push_unique(&mut all_run_ids, id);
    }
    let mut scope: HashSet<String> = all_run_ids.iter().cloned().collect();
    let mut queued = 0usize;
    let mut collected = 0usize;
    let mut retries: HashMap<String, usize> = HashMap::new();
    let mut processed: HashSet<String> = HashSet::new();
    let mut error = String::new();

    macro_rules! drain {
        () => {
            while queued > 0 {
                queued -= 1;
                sink.update(&make_update(
                    run_id,
                    &graph,
                    "running",
                    &all_run_ids,
                    leftover_lines(&graph),
                    in_flight.clone(),
                    "",
                ))?;
            }
        };
    }
    macro_rules! commit {
        () => {
            graph.updated_at = rfc3339(clock());
            if let Some(merged) = sink.save_graph(&graph)? {
                graph = merged;
            }
            queued += 1;
        };
    }

    // runConsoleDispatch 423-426：进流水线前的落图与初始摘要。
    graph.updated_at = rfc3339(clock());
    if let Some(merged) = sink.save_graph(&graph)? {
        graph = merged;
    }
    sink.update(&make_update(
        run_id,
        &graph,
        "running",
        &all_run_ids,
        vec![],
        vec![],
        "",
    ))?;

    loop {
        if is_cancelled() {
            error = CANCELLED_TEXT.into();
            break;
        }
        let stage_ids: Vec<String> = dispatchable_ids(&graph)
            .into_iter()
            // 只派本 run 的节点（新建的与修正轮生成的）。单 run 时与 TS 的 skip
            // 口径等价；合并写带进来的别的 run 的待派节点不会被本 run 派出。
            .filter(|id| scope.contains(id))
            .collect();
        if stage_ids.is_empty() {
            break;
        }
        let missing = stage_ids.iter().any(|id| {
            graph
                .nodes
                .get(id)
                .and_then(|n| n.target.as_ref())
                .is_none_or(|t| t.agent.trim().is_empty() || t.model.trim().is_empty())
        });
        if missing {
            error = MISSING_TARGET_TEXT.into();
            break;
        }
        if collected + stage_ids.len() > settings.max_requests.max(1) {
            error = format!(
                "已达总请求上限 {}，继续派发需重新确认。",
                settings.max_requests
            );
            break;
        }
        let request = build_request(run_id, project, settings, &graph, &stage_ids, models, original);
        // preview 点：把上一阶段积攒的 commit 更新补发出去。
        drain!();
        sink.note_in_flight(&stage_ids);
        // R2-8：登记在飞之后再查一次取消，标记恰好在循环开头检查与登记之间
        // 落下时不再把这一阶段整段跑完；节点仍是 draft，收尾不补失败桩。
        if is_cancelled() {
            error = CANCELLED_TEXT.into();
            break;
        }
        for id in &stage_ids {
            push_unique(&mut all_run_ids, id);
        }
        in_flight = stage_ids.clone();
        // onStage：同步发出，leftover 恒为空，图还没标 pending。
        sink.update(&make_update(
            run_id,
            &graph,
            "running",
            &all_run_ids,
            vec![],
            in_flight.clone(),
            "",
        ))?;
        graph = mark_pending(&graph, &stage_ids);
        commit!();
        let output = match sink.dispatch(&request) {
            Ok(output) => {
                drain!();
                output
            }
            Err(message) => {
                drain!();
                // startPipelineHeadless 的 catch：给没有结果的在飞节点补失败桩、
                // 停止推进，reason 是 String(error)（「Error: 」前缀）。
                let message = format!("Error: {message}");
                let stubs: Vec<DispatchNodeResult> = request
                    .nodes
                    .iter()
                    .map(|n| DispatchNodeResult {
                        node_id: n.node_id.clone(),
                        stage: graph_stage(&graph, &n.node_id),
                        status: "error".into(),
                        error: message.clone(),
                        isolation: "部分".into(),
                        runner: "unknown".into(),
                        ..DispatchNodeResult::default()
                    })
                    .collect();
                if !stubs.is_empty() {
                    graph = apply_results(&graph, &stubs);
                    commit!();
                    for stub in &stubs {
                        push_unique(&mut all_run_ids, &stub.node_id);
                    }
                    in_flight = vec![];
                }
                error = message;
                break;
            }
        };
        sink.note_in_flight(&[]);
        let mut batch: Vec<String> = output.results.iter().map(|r| r.node_id.clone()).collect();
        batch.sort();
        batch.dedup();
        let key = format!(
            "{}#{}#{}",
            if output.run_id.is_empty() {
                run_id
            } else {
                &output.run_id
            },
            request.stage,
            batch.join(",")
        );
        if !processed.insert(key) {
            // 同一批结果重复送达：本层节点刚 markPending 过，放回 draft 再停。
            graph = reset_nodes_to_draft(&graph, &stage_ids);
            commit!();
            error = DUPLICATE_BATCH_TEXT.into();
            break;
        }
        collected += output.results.len();
        for result in &output.results {
            push_unique(&mut all_run_ids, &result.node_id);
        }
        in_flight = vec![];
        graph = apply_results(&graph, &output.results);
        commit!();
        // R2-8：判定取消时本阶段已返回的结果已写回图；没返回结果的节点保持
        // pending，由收尾统一标「派发已取消」。
        if is_cancelled() {
            error = CANCELLED_TEXT.into();
            break;
        }
        let (next, keep_going, changed, new_ids) = reduce_step(
            &graph,
            &output.results,
            &mut retries,
            settings.fix_round_limit,
            &scope,
            ids,
            clock,
        );
        scope.extend(new_ids);
        graph = next;
        if changed {
            commit!();
        }
        if !keep_going {
            break;
        }
    }

    if error == CANCELLED_TEXT {
        for id in all_run_ids.clone() {
            if let Some(node) = graph.nodes.get_mut(&id) {
                if node.reply.as_ref().is_some_and(|r| r.status == "pending") {
                    node.reply = Some(Reply {
                        status: "failed".into(),
                        text: CANCELLED_TEXT.into(),
                        error_kind: Some("cancelled".into()),
                        ..Reply::default()
                    });
                }
            }
        }
    }
    // 收尾：先落图（排空积攒的 commit 更新），再发终态摘要。
    if let Some(merged) = sink.save_graph(&graph)? {
        graph = merged;
    }
    drain!();
    let summary = make_update(
        run_id,
        &graph,
        "",
        &all_run_ids,
        leftover_lines(&graph),
        vec![],
        &error,
    );
    let status = if !error.is_empty() || summary.nodes.iter().any(|n| n.status == "failed") {
        "failed"
    } else {
        "done"
    };
    sink.update(&RunUpdate { status: status.into(), ..summary })?;
    Ok(PipelineOutcome { graph, error })
}

// ── 生产边界：start_dispatch / run_background / adopt / cancel ─────────────

#[derive(Clone, Debug, Default, Deserialize)]
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

fn load_graph_for_run(root: &Path, run: &EngineRun) -> Result<Graph, String> {
    orchestration::load_graph_in(root, &run.graph_id)?.ok_or_else(|| "找不到 run 对应的图".into())
}

/// 只数 run 目录里 `.json` 且文件名不以 `.` 开头的运行中 run（R1-24）；
/// 运行中判定与 read_run 同一套口径（含启动窗口与 pid 复用核对）。
fn active_runs(root: &Path) -> usize {
    let Ok(entries) = fs::read_dir(runs_dir(root)) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|path| {
            path.extension().and_then(|e| e.to_str()) == Some("json")
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| !n.starts_with('.'))
        })
        .filter_map(|path| fs::read(path).ok())
        .filter_map(|b| serde_json::from_slice::<EngineRun>(&b).ok())
        .filter(|run| run_is_active(run))
        .count()
}

pub(crate) fn start_dispatch(root: &Path, input: DispatchInput) -> Result<Value, StartError> {
    start_dispatch_with(root, input, spawn_detached)
}

/// spawn 单独注入是为了测试退出码 3 的「后台进程起不来」来源（R2-12）。
fn start_dispatch_with(
    root: &Path,
    input: DispatchInput,
    spawn: impl FnOnce(&Path, &[String]) -> Result<u32, String>,
) -> Result<Value, StartError> {
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
    // R1-27：不再 canonicalize 改写项目路径；「项目目录不存在」检查保留在最前。
    if !Path::new(&input.project).is_dir() {
        return reject("项目目录不存在".into(), String::new());
    }
    let resources = orchestration::dispatchable_from(root);
    let presets = orchestration::raw_presets_from(root);
    let roles = orchestration::roles_from(root);
    let existing = latest_project_graph(root, &input.project);
    let project = input.project.clone();
    let mut git_dirty_count = move || {
        match crate::git::git_changes(project.clone()) {
            crate::git::GitChanges::Ok {
                uncommitted,
                untracked,
                ..
            } => Some(uncommitted.len() + untracked.len()),
            _ => None,
        }
    };
    let mut ids = || Uuid::new_v4().to_string();
    let mut clock = Utc::now;
    let mut plan_input = PlanInput {
        run_id: &run_id,
        project: &input.project,
        text: &input.text,
        preset: input.preset.as_deref(),
        attachments: &input.attachments,
        targets: &input.targets,
        settings: &settings,
        models: &resources.models,
        resources_error: resources.error.as_deref(),
        presets: &presets.presets,
        roles: &roles,
        active_runs: active_runs(root),
        existing,
        git_dirty_count: &mut git_dirty_count,
    };
    let plan = plan_dispatch(&mut plan_input, &mut ids, &mut clock);
    let (mut graph, created, original) = match plan {
        Plan::Rejected { reason, graph_id } => return reject(reason, graph_id),
        Plan::Ready {
            graph,
            created,
            original,
        } => (graph, created, original),
    };
    graph.updated_at = now();
    orchestration::save_graph_in(root, &graph).map_err(StartError::Unavailable)?;
    let run = EngineRun {
        run_id: run_id.clone(),
        graph_id: graph.id.clone(),
        project: input.project.clone(),
        status: "running".into(),
        nodes: node_summary(&graph, created.clone()),
        started_at: started,
        original_request: original,
        pid: 0,
        updated_at: now(),
        ..EngineRun::default()
    };
    // R1-6：spawn 前写一次（pid 为 0），之后父进程不再整写 run 文件。
    write_run(root, &run).map_err(StartError::Unavailable)?;
    let exe = std::env::current_exe().map_err(|e| {
        let message = format!("定位引擎程序失败：{e}");
        fail_run_before_spawn(root, &run, &message);
        StartError::Unavailable(message.clone())
    })?;
    spawn(&exe, &["__run".to_string(), run_id.clone()])
        .map(|_| {
            serde_json::json!({"run_id":run_id,"accepted":true,"graph_id":graph.id,"node_ids":created})
        })
        .map_err(|e| {
            fail_run_before_spawn(root, &run, &e);
            StartError::Unavailable(e)
        })
}

fn fail_run_before_spawn(root: &Path, run: &EngineRun, message: &str) {
    let failed = EngineRun {
        status: "failed".into(),
        reason: message.into(),
        updated_at: now(),
        ..run.clone()
    };
    let _ = write_run(root, &failed);
}

/// R1-5：后台进程不继承标准输入输出——调用方用管道读 `aicross dispatch`
/// 的输出时，父进程返回不该等整条流水线结束。Windows 上 std 的 Command
/// 一律以 bInheritHandles=TRUE 创建子进程，调用方管道这类可继承句柄会漏进
/// 后台进程、拖住调用方的 EOF；所以这里直接 CreateProcessW 且
/// bInheritHandles=FALSE，后台进程拿不到任何继承句柄。
#[cfg(windows)]
fn spawn_detached(exe: &Path, args: &[String]) -> Result<u32, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        CreateProcessW, CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW, DETACHED_PROCESS,
        PROCESS_INFORMATION, STARTUPINFOW,
    };
    let application: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
    // R2-11：参数目前只有 `__run` 与已过 valid_id 校验的 run id（字母、数字、
    // -、_），不含引号或空白，朴素 `"{}"` 包裹即可；以后往这里传用户可控文本
    // 必须先补引用转义。
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
        .map_err(|e| format!("启动后台引擎失败：{e}"))?;
        let _ = CloseHandle(information.hThread);
        let _ = CloseHandle(information.hProcess);
    }
    Ok(information.dwProcessId)
}

#[cfg(not(windows))]
fn spawn_detached(exe: &Path, args: &[String]) -> Result<u32, String> {
    Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|child| child.id())
        .map_err(|e| format!("启动后台引擎失败：{e}"))
}

/// 取消 watcher（R1-29）：盯 `<run_id>.cancel` 标记，命中置标志位并对在飞
/// 节点调 `cancel_dispatch_node`；标志位由主循环在收尾时置位让线程退出。
pub(crate) fn watch_cancel(
    root: PathBuf,
    run_id: String,
    active: Arc<Mutex<Vec<String>>>,
    cancelled: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || loop {
        if cancel_path(&root, &run_id).exists() {
            cancelled.store(true, Ordering::SeqCst);
            for id in active.lock().map(|v| v.clone()).unwrap_or_default() {
                dispatch::cancel_dispatch_node(run_id.clone(), id);
            }
            break;
        }
        if cancelled.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    })
}

/// R2-2（TS `mergeConsoleCommit`）：落图前先从磁盘读回该图的最新版本，只用
/// 「本 run 上次提交以来改动过或新增的节点」覆盖它，其余节点以磁盘为准；
/// `root`（默认列末端）取本 run 的值。同图上别的 run 与 adopt 的改动因此不会
/// 被整图回写抹掉。返回合并后实际落盘的图，作为本 run 下次提交的基准。
fn merge_save_graph(
    root: &Path,
    committed: Option<&Graph>,
    next: &Graph,
) -> Result<Graph, String> {
    let latest: Graph = match orchestration::load_graph_in(root, &next.id) {
        Ok(Some(latest)) => latest,
        // 磁盘上没有（TS：store 里没有就以上次提交的图为准；首笔提交即 next）。
        Ok(None) => committed.cloned().unwrap_or_else(|| next.clone()),
        Err(error) => return Err(error),
    };
    let changed: Vec<String> = next
        .nodes
        .keys()
        .filter(|id| {
            committed.is_none_or(|prev| prev.nodes.get(*id) != next.nodes.get(*id))
        })
        .cloned()
        .collect();
    let mut nodes = latest.nodes.clone();
    for id in &changed {
        if let Some(node) = next.nodes.get(id) {
            nodes.insert(id.clone(), node.clone());
        }
    }
    let merged = Graph {
        root: next.root.clone(),
        nodes,
        project: next.project.clone(),
        source: next.source.clone(),
        updated_at: next.updated_at.clone(),
        ..latest
    };
    orchestration::save_graph_in(root, &merged)?;
    Ok(merged)
}

struct FileSink<'a> {
    root: &'a Path,
    run: &'a mut EngineRun,
    active: Arc<Mutex<Vec<String>>>,
    /// 本 run 上一次提交的图（R2-2 合并写的基准；None 表示首笔提交，
    /// 全部节点都算改动）。
    committed: Option<Graph>,
}

impl<'a> FileSink<'a> {
    /// `baseline` 是后台进程读到的图：首笔提交只覆盖相对它改动过的节点（同 TS
    /// `let committed = graph`），读图之后别的 run 写进去的结果不会被旧值盖回。
    fn new(
        root: &'a Path,
        run: &'a mut EngineRun,
        active: Arc<Mutex<Vec<String>>>,
        baseline: Graph,
    ) -> Self {
        FileSink {
            root,
            run,
            active,
            committed: Some(baseline),
        }
    }
}

impl PipelineSink for FileSink<'_> {
    fn save_graph(&mut self, graph: &Graph) -> Result<Option<Graph>, String> {
        let merged = merge_save_graph(self.root, self.committed.as_ref(), graph)?;
        self.committed = Some(merged.clone());
        Ok(Some(merged))
    }
    fn update(&mut self, update: &RunUpdate) -> Result<(), String> {
        self.run.status = update.status.clone();
        self.run.nodes = update.nodes.clone();
        self.run.leftover = update.leftover.clone();
        self.run.in_flight_node_ids = update.in_flight_node_ids.clone();
        self.run.reason = update.reason.clone();
        self.run.updated_at = now();
        write_run(self.root, self.run)
    }
    fn dispatch(&mut self, request: &DispatchStageRequest) -> Result<DispatchStageResult, String> {
        dispatch::dispatch_stage_core(request.clone(), None)
    }
    fn note_in_flight(&mut self, ids: &[String]) {
        if let Ok(mut active) = self.active.lock() {
            *active = ids.to_vec();
        }
    }
}

pub(crate) fn run_background(root: &Path, run_id: &str) -> Result<(), String> {
    run_background_with(root, run_id, |root, run, active, baseline| {
        Box::new(FileSink::new(root, run, active, baseline.clone()))
    })
}

/// sink 工厂单独注入是为了测试（R2-6）：验证推进中途落图失败时收尾只动
/// 本 run 的节点。
fn run_background_with(
    root: &Path,
    run_id: &str,
    make_sink: impl for<'r> FnOnce(
        &'r Path,
        &'r mut EngineRun,
        Arc<Mutex<Vec<String>>>,
        &Graph,
    ) -> Box<dyn PipelineSink + 'r>,
) -> Result<(), String> {
    if !valid_id(run_id) {
        return Err("run_id 无效".into());
    }
    let mut run: EngineRun = serde_json::from_slice(
        &fs::read(run_path(root, run_id)).map_err(|e| format!("读取 run 失败：{e}"))?,
    )
    .map_err(|e| format!("解析 run 失败：{e}"))?;
    // R2-4：run 已被判失败（如启动窗口超时改写为 failed）时不复活：不写 pid、
    // 不派发、run 文件不动。
    if run.status != "running" {
        return Ok(());
    }
    run.pid = std::process::id();
    run.pid_started_at = own_process_started_at();
    run.updated_at = now();
    write_run(root, &run)?;
    let graph = load_graph_for_run(root, &run)?;
    let settings = load_settings(root);
    let resources = orchestration::dispatchable_from(root);
    if let Some(error) = resources.error {
        run.status = "failed".into();
        run.reason = format!("读取资源失败：{error}");
        run.updated_at = now();
        return write_run(root, &run);
    }
    let created: Vec<String> = run.nodes.iter().map(|n| n.node_id.clone()).collect();
    // R1-28：原始需求文本从 run 文件读，不从图上反推。
    let original = run.original_request.clone();
    let project = run.project.clone();
    let active = Arc::new(Mutex::new(Vec::<String>::new()));
    let cancelled = Arc::new(AtomicBool::new(false));
    let watcher = watch_cancel(
        root.to_path_buf(),
        run_id.to_string(),
        Arc::clone(&active),
        Arc::clone(&cancelled),
    );
    let flag = Arc::clone(&cancelled);
    let result = {
        let mut sink = make_sink(root, &mut run, Arc::clone(&active), &graph);
        let mut ids = || Uuid::new_v4().to_string();
        let mut clock = Utc::now;
        run_pipeline(
            run_id,
            &project,
            &settings,
            graph,
            &created,
            &resources.models,
            &original,
            &mut *sink,
            &mut ids,
            &mut clock,
            &move || flag.load(Ordering::SeqCst),
        )
        .map(|_| ())
    };
    cancelled.store(true, Ordering::SeqCst);
    let _ = watcher.join();
    let _ = fs::remove_file(cancel_path(root, run_id));
    if let Err(error) = &result {
        finish_failed_run(root, &mut run, error);
    }
    result
}

/// R1-8 收尾（R2-6 修正）：推进中途读写失败——只把本 run（run 摘要里记录的
/// 节点）仍 pending 的补失败，同图上别的 run 的节点不动；run 摘要的 nodes 按
/// 收尾后的图重算，再连同失败原因落盘。
fn finish_failed_run(root: &Path, run: &mut EngineRun, error: &str) {
    let owned: Vec<String> = run.nodes.iter().map(|n| n.node_id.clone()).collect();
    if let Ok(Some(disk)) = orchestration::load_graph_in(root, &run.graph_id) {
        let mut next = disk.clone();
        let mut touched = false;
        for id in &owned {
            if let Some(node) = next.nodes.get_mut(id) {
                if node.reply.as_ref().is_some_and(|r| r.status == "pending") {
                    node.reply = Some(Reply {
                        status: "failed".into(),
                        text: error.into(),
                        error_kind: Some("error".into()),
                        ..Reply::default()
                    });
                    touched = true;
                }
            }
        }
        if touched {
            next.updated_at = now();
            let _ = merge_save_graph(root, Some(&disk), &next);
        }
        run.nodes = node_summary(&next, owned);
    }
    run.status = "failed".into();
    run.reason = error.into();
    run.in_flight_node_ids = vec![];
    run.updated_at = now();
    let _ = write_run(root, run);
}

pub(crate) fn adopt(root: &Path, run_id: &str, node_id: &str) -> Result<EngineRun, String> {
    let mut run = read_run(root, run_id)?;
    let disk = load_graph_for_run(root, &run)?;
    let Some(node) = disk.nodes.get(node_id) else {
        return Err("找不到节点".into());
    };
    // R1-6：没有结果的节点按 TS `setReplyAdopted` 口径不改图、正常返回。
    if node.reply.is_none() {
        return Ok(run);
    }
    let mut next = disk.clone();
    if let Some(reply) = next.nodes.get_mut(node_id).and_then(|n| n.reply.as_mut()) {
        reply.adopted = Some(true);
    }
    next.updated_at = now();
    // R2-3：只改图（合并写），不整写 run 文件；返回值按刚改好的图现算摘要，
    // 免得把已落盘的终态写回 running。
    merge_save_graph(root, Some(&disk), &next)?;
    let ids: Vec<String> = run.nodes.iter().map(|n| n.node_id.clone()).collect();
    run.nodes = node_summary(&next, ids);
    Ok(run)
}

pub(crate) fn cancel(root: &Path, run_id: &str) -> Result<EngineRun, String> {
    let run = read_run(root, run_id)?;
    // R1-6：run 不在 running 时按 TS 口径静默成功；只写标记文件，不改写 run。
    if run.status != "running" {
        return Ok(run);
    }
    fs::create_dir_all(runs_dir(root)).map_err(|e| format!("创建 run 目录失败：{e}"))?;
    fs::write(cancel_path(root, run_id), []).map_err(|e| format!("写入取消标记失败：{e}"))?;
    Ok(run)
}

#[cfg(test)]
mod golden;

#[cfg(test)]
mod tests;
