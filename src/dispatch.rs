use crate::channels::{self, ChannelErrorKind, ChannelOutput};
use crate::paths::aicross_root;
use crate::process::{cancel_registered, run_process_registered};
use chrono::{Local, SecondsFormat};
use regex::Regex;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchSettings {
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
    #[serde(default = "default_true")]
    pub redact: bool,
    #[serde(default = "default_true")]
    pub write_project_dispatch: bool,
    #[serde(default)]
    pub cc_switch_path: String,
    #[serde(default)]
    pub validation_command: String,
    #[serde(default = "default_max_requests")]
    pub max_requests: usize,
}

impl Default for DispatchSettings {
    fn default() -> Self {
        Self {
            timeout_secs: default_timeout(),
            concurrency: default_concurrency(),
            redact: true,
            write_project_dispatch: true,
            cc_switch_path: String::new(),
            validation_command: String::new(),
            max_requests: default_max_requests(),
        }
    }
}

fn default_timeout() -> u64 {
    600
}
fn default_concurrency() -> usize {
    3
}
fn default_max_requests() -> usize {
    8
}
fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchNodeRequest {
    pub node_id: String,
    #[serde(default = "default_stage")]
    pub stage: u32,
    pub agent: String,
    #[serde(default)]
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub billing_source: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub thinking: String,
    #[serde(default)]
    pub visibility: String,
    pub prompt: String,
    #[serde(default)]
    pub current_message: String,
    #[serde(default)]
    pub materials: Vec<String>,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub write: String,
    #[serde(default)]
    pub original_request: String,
    #[serde(default)]
    pub upstream_artifacts: Vec<DispatchArtifacts>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ChangedFile {
    pub path: String,
    pub content: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DispatchArtifacts {
    #[serde(default)]
    pub diff: String,
    #[serde(default)]
    pub changed_files: Vec<ChangedFile>,
    #[serde(default)]
    pub worktree: Option<String>,
    #[serde(default)]
    pub stdout_summary: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchStageRequest {
    #[serde(default)]
    pub run_id: String,
    #[serde(default = "default_stage")]
    pub stage: u32,
    #[serde(default)]
    pub project_dir: String,
    #[serde(default)]
    pub nodes: Vec<DispatchNodeRequest>,
    #[serde(default)]
    pub settings: DispatchSettings,
}

fn default_stage() -> u32 {
    1
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchPreview {
    pub total_redacted: usize,
    pub broad_redacted: usize,
    pub nodes: Vec<DispatchPreviewNode>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchPreviewNode {
    pub node_id: String,
    pub redacted: usize,
    pub isolation: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchNodeResult {
    pub node_id: String,
    pub stage: u32,
    pub status: String,
    pub text: String,
    pub raw_stdout: String,
    pub raw_stderr: String,
    pub model_reported: String,
    pub session_id: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cache_read: u64,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub truncated: bool,
    pub redacted: usize,
    pub isolation: String,
    pub workdir: String,
    pub dispatch_file: String,
    pub frontmatter: String,
    pub output_preview: String,
    pub error: String,
    pub artifacts: Option<DispatchArtifacts>,
    pub validation_output: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchStageResult {
    pub run_id: String,
    pub results: Vec<DispatchNodeResult>,
    pub state_file: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchStateSummary {
    pub exists: bool,
    pub unfinished: bool,
    pub content: String,
    pub run_id: String,
    pub active_node_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct DispatchStateHeader {
    schema: String,
    run_id: String,
    stage: u32,
    unfinished: bool,
    active_node_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DispatchProgress {
    run_id: String,
    node_id: String,
    status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct DispatchRecord {
    schema: String,
    run_id: String,
    stage: u32,
    node: String,
    agent: String,
    provider: String,
    model: String,
    vendor: String,
    billing_source: String,
    role: String,
    tier: String,
    thinking: String,
    visibility: String,
    isolation: String,
    workdir: String,
    materials: Vec<String>,
    redacted: usize,
    truncated: bool,
    status: String,
    started_at: String,
    ended_at: String,
    duration_ms: u64,
    tokens_in: u64,
    tokens_out: u64,
    cache_read: u64,
    identity: String,
    session_id: String,
    exit_code: Option<i32>,
    error: String,
}

#[tauri::command]
pub async fn preview_dispatch(request: DispatchStageRequest) -> Result<DispatchPreview, String> {
    tauri::async_runtime::spawn_blocking(move || preview_core(&request))
        .await
        .map_err(|error| format!("预览派发线程失败：{error}"))?
}

#[tauri::command]
pub async fn dispatch_graph_stage(
    request: DispatchStageRequest,
    app: AppHandle,
) -> Result<DispatchStageResult, String> {
    tauri::async_runtime::spawn_blocking(move || dispatch_stage_core(request, Some(app)))
        .await
        .map_err(|error| format!("派发线程失败：{error}"))?
}

#[tauri::command]
pub fn cancel_dispatch_node(run_id: String, node_id: String) -> bool {
    let key = registry_key(&run_id, &node_id);
    mark_cancelled(&key);
    let _ = cancel_registered(&key);
    true
}

#[tauri::command]
pub fn inspect_dispatch_state(project_dir: String) -> Result<DispatchStateSummary, String> {
    let path = project_path(&project_dir)?
        .join(".dispatch")
        .join("STATE.md");
    if !path.is_file() {
        return Ok(DispatchStateSummary::default());
    }
    let content =
        fs::read_to_string(&path).map_err(|error| format!("读取派发状态失败：{error}"))?;
    let header = content
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("aicross-state: "))
        .and_then(|json| serde_json::from_str::<DispatchStateHeader>(json).ok())
        .unwrap_or_default();
    Ok(DispatchStateSummary {
        exists: true,
        unfinished: header.unfinished,
        content,
        run_id: header.run_id,
        active_node_ids: header.active_node_ids,
    })
}

#[tauri::command]
pub fn abandon_dispatch_state(project_dir: String) -> Result<(), String> {
    let path = project_path(&project_dir)?
        .join(".dispatch")
        .join("STATE.md");
    if path.is_file() {
        let stamp = Local::now().format("%Y%m%d-%H%M%S");
        let abandoned = path.with_file_name(format!("STATE.abandoned-{stamp}.md"));
        fs::rename(&path, &abandoned).map_err(|error| format!("放弃派发状态失败：{error}"))?;
    }
    Ok(())
}

fn is_continuing(node: &DispatchNodeRequest) -> bool {
    !node.session_id.is_empty() && node.visibility == "共享"
}

fn requires_single_writer(node: &DispatchNodeRequest, worktree_falls_back: bool) -> bool {
    node.visibility != "盲"
        && (node.write == "单写者" || (node.write == "worktree" && worktree_falls_back))
}

fn preview_core(request: &DispatchStageRequest) -> Result<DispatchPreview, String> {
    let _ = project_path(&request.project_dir)?;
    if request
        .nodes
        .iter()
        .any(|node| channels::normalize_agent(&node.agent) == "cc-switch")
    {
        let path = request.settings.cc_switch_path.trim();
        if path.is_empty() {
            return Err("未配置 cc_switch.py 路径".into());
        }
        if !Path::new(path).is_file() {
            return Err(format!("cc_switch.py 路径不存在：{path}"));
        }
    }
    let mut preview = DispatchPreview::default();
    for node in &request.nodes {
        let prompt = if is_continuing(node) && !node.current_message.is_empty() {
            &node.current_message
        } else {
            &node.prompt
        };
        let redacted = if request.settings.redact {
            redaction_count(prompt, &node.materials, node.visibility == "盲")?
        } else {
            0
        };
        preview.total_redacted += redacted;
        preview.broad_redacted += broad_redaction_count(prompt);
        preview.nodes.push(DispatchPreviewNode {
            node_id: node.node_id.clone(),
            redacted,
            isolation: channels::isolation_for(&node.agent, node.visibility == "盲").into(),
        });
    }
    Ok(preview)
}

fn dispatch_stage_core(
    mut request: DispatchStageRequest,
    app: Option<AppHandle>,
) -> Result<DispatchStageResult, String> {
    if request.nodes.is_empty() {
        return Err("当前阶段没有可派发节点".into());
    }
    if let Some(node) = request.nodes.iter().find(|node| node.agent.trim().is_empty() || node.model.trim().is_empty()) {
        return Ok(DispatchStageResult {
            run_id: request.run_id,
            results: vec![DispatchNodeResult {
                node_id: node.node_id.clone(),
                stage: node.stage,
                status: "error".into(),
                error: format!("节点 {} 未指定 agent/model，不能派发", node.node_id),
                ..DispatchNodeResult::default()
            }],
            state_file: String::new(),
        });
    }
    let mut rejected = Vec::new();
    request.nodes.retain(|node| {
        let invalid = (node.role == "验收" || node.role == "验证") && node.write != "只读";
        if invalid {
            rejected.push(DispatchNodeResult {
                node_id: node.node_id.clone(),
                stage: node.stage,
                status: "error".into(),
                error: format!("节点 {} 的验收/验证写权限必须为只读", node.node_id),
                ..DispatchNodeResult::default()
            });
        }
        !invalid
    });
    if request.nodes.is_empty() {
        return Ok(DispatchStageResult {
            run_id: request.run_id,
            results: rejected,
            state_file: String::new(),
        });
    }
    if request.nodes.len() > request.settings.max_requests.clamp(1, 100) {
        return Err(format!(
            "本次派发 {} 路，超过总请求上限 {}",
            request.nodes.len(),
            request.settings.max_requests.clamp(1, 100)
        ));
    }
    if request.run_id.is_empty() {
        request.run_id = Uuid::new_v4().to_string();
    }
    let project = project_path(&request.project_dir)?;
    let run_root = dispatch_data_root()?.join("runs").join(&request.run_id);
    fs::create_dir_all(&run_root).map_err(|error| format!("创建运行目录失败：{error}"))?;
    let log_dir = if request.settings.write_project_dispatch {
        project.join(".dispatch")
    } else {
        run_root.join("dispatch")
    };
    ensure_log_dir(&log_dir)?;

    let mut status: HashMap<String, String> = request
        .nodes
        .iter()
        .map(|node| (node.node_id.clone(), "排队".to_string()))
        .collect();
    let state_file = log_dir.join("STATE.md");
    let limit = request
        .settings
        .concurrency
        .clamp(1, 3)
        .min(request.nodes.len());
    let timeout = Duration::from_secs(request.settings.timeout_secs.clamp(1, 3600));
    let worktree_falls_back = request.nodes.iter().any(|node| node.write == "worktree")
        && !project_is_git_work_tree(&project, timeout).unwrap_or(false);
    let state_stage = request
        .nodes
        .iter()
        .map(|node| node.stage)
        .min()
        .unwrap_or(request.stage);
    write_state(
        &state_file,
        &request.run_id,
        state_stage,
        limit,
        &status,
        &[],
    )?;
    for node in &request.nodes {
        emit_progress(&app, &request.run_id, &node.node_id, "排队");
    }
    let mut queued: VecDeque<DispatchNodeRequest> = request.nodes.clone().into();
    let mut active_agents = HashSet::new();
    let mut active = 0_usize;
    let mut results = rejected;
    let (sender, receiver) = mpsc::channel::<(String, bool, DispatchNodeResult)>();
    let mut active_single_writer = false;

    while !queued.is_empty() || active > 0 {
        while active < limit {
            let next_index = queued.iter().position(|node| {
                let requires_single_writer = requires_single_writer(node, worktree_falls_back);
                !active_agents.contains(channels::normalize_agent(&node.agent))
                    && !(requires_single_writer && active_single_writer)
            });
            if active_single_writer {
                for node in queued.iter().filter(|node| {
                    requires_single_writer(node, worktree_falls_back)
                }) {
                    if status.get(&node.node_id).map(String::as_str) != Some("等待写锁") {
                        status.insert(node.node_id.clone(), "等待写锁".into());
                        emit_progress(&app, &request.run_id, &node.node_id, "等待写锁");
                    }
                }
            }
            let Some(index) = next_index else { break };
            let node = queued.remove(index).expect("queued index");
            let single_writer = requires_single_writer(&node, worktree_falls_back);
            if single_writer {
                active_single_writer = true;
            }
            let agent = channels::normalize_agent(&node.agent).to_string();
            active_agents.insert(agent.clone());
            active += 1;
            status.insert(node.node_id.clone(), "运行中".into());
            emit_progress(&app, &request.run_id, &node.node_id, "运行中");
            write_state(
                &state_file,
                &request.run_id,
                state_stage,
                limit,
                &status,
                &results,
            )?;

            let tx = sender.clone();
            let run_id = request.run_id.clone();
            let settings = request.settings.clone();
            let project = project.clone();
            let run_root = run_root.clone();
            let log_dir = log_dir.clone();
            thread::spawn(move || {
                let result = run_one(&run_id, &project, &run_root, &log_dir, &settings, node);
                let _ = tx.send((agent, single_writer, result));
            });
        }
        if active == 0 {
            return Err("同一 agent 并发调度未能推进".into());
        }
        let (agent, single_writer, result) = receiver
            .recv()
            .map_err(|error| format!("接收派发结果失败：{error}"))?;
        active -= 1;
        active_agents.remove(&agent);
        if single_writer {
            active_single_writer = false;
        }
        let ui_status = match result.status.as_str() {
            "ok" => "完成",
            "timeout" => "超时",
            "validation_failed" => "验证失败",
            "identity_mismatch" => "真身不符",
            _ => "失败",
        };
        status.insert(result.node_id.clone(), ui_status.into());
        emit_progress(&app, &request.run_id, &result.node_id, ui_status);
        results.push(result);
        write_state(
            &state_file,
            &request.run_id,
            state_stage,
            limit,
            &status,
            &results,
        )?;
    }

    let order: HashMap<&str, usize> = request
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.node_id.as_str(), index))
        .collect();
    results.sort_by_key(|result| {
        order
            .get(result.node_id.as_str())
            .copied()
            .unwrap_or(usize::MAX)
    });
    Ok(DispatchStageResult {
        run_id: request.run_id,
        results,
        state_file: state_file.to_string_lossy().into_owned(),
    })
}

fn dispatch_data_root() -> Result<PathBuf, String> {
    #[cfg(test)]
    if let Some(path) = std::env::var_os("AICROSS_TEST_RUN_ROOT") {
        return Ok(PathBuf::from(path));
    }
    aicross_root()
}

const SNAPSHOT_FILE_LIMIT: u64 = 8 * 1024 * 1024;
const SNAPSHOT_TOTAL_LIMIT: u64 = 128 * 1024 * 1024;
const FALLBACK_FILE_DIFF_LIMIT: usize = 200 * 1024;

#[derive(Clone, Debug, Default)]
struct SnapshotFile {
    bytes: Option<Vec<u8>>,
    size: u64,
    modified_ns: u128,
}

type DirectorySnapshot = HashMap<String, SnapshotFile>;

#[derive(Default)]
struct ProjectLockState {
    held: bool,
    queue: VecDeque<String>,
}

#[derive(Default)]
struct ProjectLock {
    state: Mutex<ProjectLockState>,
    changed: Condvar,
}

fn project_write_locks() -> &'static Mutex<HashMap<PathBuf, Arc<ProjectLock>>> {
    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<ProjectLock>>>> = OnceLock::new();
    LOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cancelled_dispatches() -> &'static Mutex<HashSet<String>> {
    static CANCELLED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    CANCELLED.get_or_init(|| Mutex::new(HashSet::new()))
}

fn mark_cancelled(key: &str) {
    if let Ok(mut cancelled) = cancelled_dispatches().lock() {
        cancelled.insert(key.to_string());
    }
    if let Ok(locks) = project_write_locks().lock() {
        for lock in locks.values() {
            lock.changed.notify_all();
        }
    }
}

fn is_cancelled(key: &str) -> bool {
    cancelled_dispatches()
        .lock()
        .is_ok_and(|cancelled| cancelled.contains(key))
}

fn clear_cancelled(key: &str) {
    if let Ok(mut cancelled) = cancelled_dispatches().lock() {
        cancelled.remove(key);
    }
}

struct ProjectWriteGuard {
    lock: Arc<ProjectLock>,
}

impl Drop for ProjectWriteGuard {
    fn drop(&mut self) {
        if let Ok(mut state) = self.lock.state.lock() {
            state.held = false;
            self.lock.changed.notify_all();
        }
    }
}

fn acquire_project_write(
    project: &Path,
    timeout: Duration,
    cancel_key: &str,
) -> Result<ProjectWriteGuard, String> {
    let project = project
        .canonicalize()
        .unwrap_or_else(|_| project.to_path_buf());
    let lock = {
        let mut locks = project_write_locks()
            .lock()
            .map_err(|_| "写锁状态不可用".to_string())?;
        locks.entry(project).or_default().clone()
    };
    let waiter = Uuid::new_v4().to_string();
    let deadline = Instant::now() + timeout;
    let mut state = lock
        .state
        .lock()
        .map_err(|_| "写锁状态不可用".to_string())?;
    state.queue.push_back(waiter.clone());
    loop {
        if is_cancelled(cancel_key) {
            state.queue.retain(|entry| entry != &waiter);
            lock.changed.notify_all();
            return Err("等待写锁已取消".into());
        }
        if !state.held && state.queue.front() == Some(&waiter) {
            state.queue.pop_front();
            state.held = true;
            drop(state);
            return Ok(ProjectWriteGuard { lock });
        }
        let now = Instant::now();
        if now >= deadline {
            state.queue.retain(|entry| entry != &waiter);
            lock.changed.notify_all();
            return Err(format!("等待写锁超过 {} 秒", timeout.as_secs()));
        }
        let remaining = deadline.saturating_duration_since(now);
        let wait = remaining.min(Duration::from_millis(100));
        let (next, _) = lock
            .changed
            .wait_timeout(state, wait)
            .map_err(|_| "写锁状态不可用".to_string())?;
        state = next;
    }
}

#[cfg(windows)]
fn is_link_or_junction(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_link_or_junction(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn snapshot_directory(root: &Path) -> Result<DirectorySnapshot, String> {
    snapshot_directory_with_limits(root, SNAPSHOT_FILE_LIMIT, SNAPSHOT_TOTAL_LIMIT)
}

fn snapshot_directory_with_limits(
    root: &Path,
    file_limit: u64,
    total_limit: u64,
) -> Result<DirectorySnapshot, String> {
    fn walk(
        root: &Path,
        current: &Path,
        out: &mut DirectorySnapshot,
        file_limit: u64,
        total_limit: u64,
        total_read: &mut u64,
    ) -> Result<(), String> {
        for entry in fs::read_dir(current)
            .map_err(|error| format!("读取目录快照失败 {}：{error}", current.display()))?
        {
            let entry = entry.map_err(|error| format!("读取目录项失败：{error}"))?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("读取快照元数据失败 {}：{error}", path.display()))?;
            if is_link_or_junction(&metadata) {
                continue;
            }
            if metadata.is_dir() {
                if matches!(
                    name.as_str(),
                    ".git"
                        | ".dispatch"
                        | ".aicross-runtime"
                        | ".pytest_cache"
                        | "__pycache__"
                        | "node_modules"
                        | "target"
                        | "dist"
                        | ".venv"
                        | "build"
                        | "coverage"
                        | ".next"
                        | "vendor"
                ) {
                    continue;
                }
                walk(root, &path, out, file_limit, total_limit, total_read)?;
            } else if metadata.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let bytes = if metadata.len() > file_limit {
                    None
                } else {
                    if total_read.saturating_add(metadata.len()) > total_limit {
                        return Err(format!(
                            "目录快照超过总字节上限 {} MB",
                            total_limit / (1024 * 1024)
                        ));
                    }
                    *total_read += metadata.len();
                    Some(fs::read(&path).map_err(|error| {
                        format!("读取快照文件失败 {}：{error}", path.display())
                    })?)
                };
                let modified_ns = metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|duration| duration.as_nanos())
                    .unwrap_or_default();
                out.insert(relative, SnapshotFile { bytes, size: metadata.len(), modified_ns });
            }
        }
        Ok(())
    }

    let mut snapshot = HashMap::new();
    let mut total_read = 0;
    walk(
        root,
        root,
        &mut snapshot,
        file_limit,
        total_limit,
        &mut total_read,
    )?;
    Ok(snapshot)
}

fn snapshot_file_unchanged(old_file: Option<&SnapshotFile>, new_file: Option<&SnapshotFile>) -> bool {
    match (old_file, new_file) {
        (Some(old), Some(new)) => match (&old.bytes, &new.bytes) {
            (Some(old), Some(new)) => old == new,
            (None, None) => old.size == new.size && old.modified_ns == new.modified_ns,
            _ => false,
        },
        (None, None) => true,
        _ => false,
    }
}

fn changed_snapshot_paths(before: &DirectorySnapshot, after: &DirectorySnapshot) -> Vec<String> {
    let mut paths: Vec<String> = before.keys().chain(after.keys()).cloned().collect();
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .filter(|path| !snapshot_file_unchanged(before.get(path), after.get(path)))
        .collect()
}

fn changed_file_contents(after: &DirectorySnapshot, paths: &[String]) -> Vec<ChangedFile> {
    paths
        .iter()
        .filter_map(|path| {
            let bytes = after.get(path)?.bytes.as_deref()?;
            if bytes.len() > 200 * 1024 {
                return None;
            }
            let content = std::str::from_utf8(bytes).ok()?.to_string();
            Some(ChangedFile {
                path: path.clone(),
                content,
            })
        })
        .collect()
}

fn fallback_diff_for_paths(
    before: &DirectorySnapshot,
    after: &DirectorySnapshot,
    paths: &[String],
) -> String {
    let mut diff = String::new();
    for path in paths {
        let old_file = before.get(path);
        let new_file = after.get(path);
        let old = old_file.and_then(|file| file.bytes.as_deref());
        let new = new_file.and_then(|file| file.bytes.as_deref());
        let old_text = old.and_then(|bytes| std::str::from_utf8(bytes).ok());
        let new_text = new.and_then(|bytes| std::str::from_utf8(bytes).ok());
        let old_label = if old_file.is_some() {
            format!("a/{path}")
        } else {
            "/dev/null".into()
        };
        let new_label = if new_file.is_some() {
            format!("b/{path}")
        } else {
            "/dev/null".into()
        };
        let mut file_diff = format!("--- {old_label}\n+++ {new_label}\n");
        if (old_file.is_some() && old_text.is_none()) || (new_file.is_some() && new_text.is_none()) {
            file_diff.push_str(&format!("Binary files {old_label} and {new_label} differ\n"));
            diff.push_str(&file_diff);
            continue;
        }
        let old_text = old_text.unwrap_or("");
        let new_text = new_text.unwrap_or("");
        let old_lines = if old_text.is_empty() {
            0
        } else {
            old_text.lines().count()
        };
        let new_lines = if new_text.is_empty() {
            0
        } else {
            new_text.lines().count()
        };
        let old_start = usize::from(old_file.is_some());
        let new_start = usize::from(new_file.is_some());
        file_diff.push_str(&format!(
            "@@ -{old_start},{old_lines} +{new_start},{new_lines} @@\n"
        ));
        for line in old_text.lines() {
            file_diff.push_str(&format!("-{line}\n"));
        }
        for line in new_text.lines() {
            file_diff.push_str(&format!("+{line}\n"));
        }
        if file_diff.len() > FALLBACK_FILE_DIFF_LIMIT {
            diff.push_str(&format!("文件：{path}\n全文见 changed/\n"));
        } else {
            diff.push_str(&file_diff);
        }
    }
    diff
}

fn artifact_diff(before: &DirectorySnapshot, after: &DirectorySnapshot) -> DispatchArtifacts {
    let paths = changed_snapshot_paths(before, after);
    DispatchArtifacts {
        diff: fallback_diff_for_paths(before, after, &paths),
        changed_files: changed_file_contents(after, &paths),
        ..DispatchArtifacts::default()
    }
}

fn materialize_snapshot(
    root: &Path,
    snapshot: &DirectorySnapshot,
    paths: &[String],
) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|error| format!("创建 diff 临时目录失败：{error}"))?;
    for path in paths {
        let Some(bytes) = snapshot.get(path).and_then(|file| file.bytes.as_deref()) else {
            continue;
        };
        let target = root.join(Path::new(path));
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("创建 diff 临时子目录失败：{error}"))?;
        }
        fs::write(&target, bytes)
            .map_err(|error| format!("写入 diff 临时文件失败 {}：{error}", target.display()))?;
    }
    Ok(())
}

fn project_is_git_work_tree_with(program: &Path, project: &Path, timeout: Duration) -> Result<bool, String> {
    let probe = run_process_registered(
        program,
        &["rev-parse".into(), "--is-inside-work-tree".into()],
        None,
        timeout,
        Some(project),
        None,
    )?;
    Ok(!probe.timed_out && probe.code == Some(0) && probe.stdout.trim() == "true")
}

fn project_is_git_work_tree(project: &Path, timeout: Duration) -> Result<bool, String> {
    project_is_git_work_tree_with(Path::new("git"), project, timeout)
}

fn artifact_diff_for_project(
    project: &Path,
    scratch_parent: &Path,
    before: &DirectorySnapshot,
    after: &DirectorySnapshot,
    timeout: Duration,
) -> Result<DispatchArtifacts, String> {
    let is_git = project_is_git_work_tree(project, timeout).unwrap_or(false);
    artifact_diff_for_project_with_git(Path::new("git"), is_git, scratch_parent, before, after, timeout)
}

fn artifact_diff_for_project_with_git(
    git: &Path,
    is_git: bool,
    scratch_parent: &Path,
    before: &DirectorySnapshot,
    after: &DirectorySnapshot,
    timeout: Duration,
) -> Result<DispatchArtifacts, String> {
    let paths = changed_snapshot_paths(before, after);
    if paths.is_empty() {
        return Ok(DispatchArtifacts::default());
    }
    if !is_git {
        return Ok(artifact_diff(before, after));
    }

    let materialized = paths
        .iter()
        .filter(|path| {
            before.get(*path).is_none_or(|file| file.bytes.is_some())
                && after.get(*path).is_none_or(|file| file.bytes.is_some())
        })
        .cloned()
        .collect::<Vec<_>>();
    let oversized = paths
        .iter()
        .filter(|path| !materialized.contains(path))
        .cloned()
        .collect::<Vec<_>>();
    let scratch = scratch_parent.join(format!("diff-{}", Uuid::new_v4()));
    let before_root = scratch.join("before");
    let after_root = scratch.join("after");
    if let Err(error) = materialize_snapshot(&before_root, before, &materialized)
        .and_then(|_| materialize_snapshot(&after_root, after, &materialized))
    {
        let _ = fs::remove_dir_all(&scratch);
        return Err(error);
    }
    let result = run_process_registered(
        git,
        &[
            "diff".into(),
            "--no-index".into(),
            "--binary".into(),
            "--unified=3".into(),
            "--no-ext-diff".into(),
            "--src-prefix=a/".into(),
            "--dst-prefix=b/".into(),
            "before".into(),
            "after".into(),
        ],
        None,
        timeout,
        Some(&scratch),
        None,
    );
    let _ = fs::remove_dir_all(&scratch);
    let result = match result {
        Ok(result) => result,
        Err(_) => return Ok(artifact_diff(before, after)),
    };
    if result.timed_out || !matches!(result.code, Some(0 | 1)) {
        return Ok(artifact_diff(before, after));
    }
    let mut diff = normalize_git_diff(&result.stdout);
    if !diff.is_empty() { diff.push('\n'); }
    diff.push_str(&fallback_diff_for_paths(before, after, &oversized));
    Ok(DispatchArtifacts {
        diff,
        changed_files: changed_file_contents(after, &paths),
        ..DispatchArtifacts::default()
    })
}

fn normalize_git_diff(raw: &str) -> String {
    raw.lines().map(|line| {
        if line.starts_with("diff --git ") || line.starts_with("--- ") || line.starts_with("+++ ") {
            line.replace("a/before/", "a/")
                .replace("b/after/", "b/")
                .replace("\"before/", "\"a/")
                .replace("\"after/", "\"b/")
        } else { line.to_string() }
    }).collect::<Vec<_>>().join("\n")
}

fn create_worktree(project: &Path, path: &Path, timeout: Duration) -> Result<(), String> {
    if !project_is_git_work_tree(project, timeout)? {
        return Err("项目不是 git 仓库".into());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建 worktree 目录失败：{error}"))?;
    }
    let result = run_process_registered(
        Path::new("git"),
        &[
            "worktree".into(),
            "add".into(),
            "--detach".into(),
            path.to_string_lossy().into_owned(),
            "HEAD".into(),
        ],
        None,
        timeout,
        Some(project),
        None,
    )?;
    if result.timed_out || result.code != Some(0) {
        return Err(nonempty_process_error(&result, "创建 worktree 失败"));
    }
    Ok(())
}

fn run_validation(command: &str, cwd: &Path, timeout: Duration) -> Result<String, String> {
    if command.trim().is_empty() {
        return Ok(String::new());
    }
    #[cfg(windows)]
    let (program, args) = (
        Path::new("powershell.exe"),
        vec!["-NoProfile".into(), "-Command".into(), command.into()],
    );
    #[cfg(not(windows))]
    let (program, args) = (Path::new("sh"), vec!["-lc".into(), command.into()]);
    let result = run_process_registered(program, &args, None, timeout, Some(cwd), None)?;
    let output = [result.stdout.trim(), result.stderr.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let output = strip_ansi(&output);
    if result.timed_out {
        return Err(format!("验证命令超时\n{output}"));
    }
    if result.code != Some(0) {
        return Err(validation_failure(result.code, &output));
    }
    Ok(output)
}

fn successful_validation_output(node: &DispatchNodeRequest, used_worktree: bool) -> Option<&'static str> {
    (node.role == "执行" && used_worktree)
        .then_some("worktree 节点未跑验证命令，采纳合并后再验")
}

fn redact_artifacts(value: &mut DispatchArtifacts) -> usize {
    let mut redacted = 0;
    let (diff, count) = redact_text(&value.diff, true);
    value.diff = diff;
    redacted += count;
    for file in &mut value.changed_files {
        let (content, count) = redact_text(&file.content, true);
        file.content = content;
        redacted += count;
    }
    let (summary, count) = redact_text(&value.stdout_summary, true);
    value.stdout_summary = summary;
    redacted + count
}

fn record_stdout(raw_stdout: &str, run_root: &Path, node_id: &str) -> String {
    if raw_stdout.len() <= 32 * 1024 {
        return raw_stdout.to_string();
    }
    let raw_path = run_root.join(format!("{}.raw.txt", safe_name(node_id)));
    let tail = raw_stdout
        .chars()
        .rev()
        .take(4096)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    match fs::write(&raw_path, raw_stdout.as_bytes()) {
        Ok(()) => format!("原始输出已转存：{}\n末尾 4KB：\n{}", raw_path.display(), tail),
        Err(error) => format!("原始输出转存失败：{error}\n末尾 4KB：\n{tail}"),
    }
}

fn nonempty_process_error(result: &crate::process::ProcessResult, fallback: &str) -> String {
    let text = if result.stderr.trim().is_empty() {
        result.stdout.trim()
    } else {
        result.stderr.trim()
    };
    if text.is_empty() {
        fallback.into()
    } else {
        text.into()
    }
}

fn run_one(
    run_id: &str,
    project: &Path,
    run_root: &Path,
    log_dir: &Path,
    settings: &DispatchSettings,
    mut node: DispatchNodeRequest,
) -> DispatchNodeResult {
    let stage = node.stage.max(1);
    let started = Local::now();
    let timer = Instant::now();
    let cancel_key = registry_key(run_id, &node.node_id);
    let blind = node.visibility == "盲";
    if blind {
        node.write = "只读".into();
    }
    let isolation = channels::isolation_for(&node.agent, blind).to_string();
    let timeout = Duration::from_secs(settings.timeout_secs.clamp(1, 3600));
    let mut worktree_warning = String::new();
    let mut worktree_path = None;
    let mut uses_single_writer = node.write == "单写者";
    let workdir = if blind {
        run_root.join(&node.node_id)
    } else if node.write == "worktree" {
        let path = run_root.join(&node.node_id).join("wt");
        match create_worktree(project, &path, timeout) {
            Ok(()) => {
                worktree_path = Some(path.to_string_lossy().into_owned());
                path
            }
            Err(error) => {
                uses_single_writer = true;
                worktree_warning = format!("worktree 不可用，已退回单写者：{error}");
                project.to_path_buf()
            }
        }
    } else {
        project.to_path_buf()
    };
    let (_write_guard, write_lock_error) = if uses_single_writer {
        match acquire_project_write(project, timeout, &cancel_key) {
            Ok(guard) => (Some(guard), None),
            Err(error) => (None, Some(error)),
        }
    } else {
        (None, None)
    };
    let (before_snapshot, snapshot_error) = if write_lock_error.is_none()
        && !blind
        && node.write != "只读"
    {
        match snapshot_directory(&workdir) {
            Ok(snapshot) => (Some(snapshot), None),
            Err(error) => (None, Some(error)),
        }
    } else {
        (None, None)
    };
    let task_dir = run_root.join("tasks");
    let task_file = task_dir.join(format!("{}.txt", safe_name(&node.node_id)));
    let session_dir = dispatch_data_root()
        .map(|root| root.join("sessions").join(safe_name(&node.agent)))
        .ok();
    let prepared = prepare_outbound(&node, settings.redact, &workdir, blind);
    let effective_session =
        if channels::normalize_agent(&node.agent) == "pi" && node.session_id.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            node.session_id.clone()
        };

    let (prompt, material_names, prepared_redacted, preparation_error) = match prepared {
        Ok(value) => (value.0, value.1, value.2, String::new()),
        Err(error) => ("(未发送)".into(), Vec::new(), 0, error),
    };
    let continuing = is_continuing(&node);
    let (outbound, mut redacted) = if continuing {
        if node.current_message.is_empty() {
            (prompt.clone(), prepared_redacted)
        } else {
            redact_text(&node.current_message, settings.redact)
        }
    } else {
        (prompt.clone(), prepared_redacted)
    };
    let _ = fs::create_dir_all(&task_dir);
    let _ = fs::write(&task_file, outbound.as_bytes());

    let mut raw_stdout = String::new();
    let mut raw_stderr = String::new();
    let mut channel_output = ChannelOutput::default();
    let mut exit_code = None;
    let mut truncated = false;
    let mut timed_out = false;
    let mut status = "error".to_string();
    let mut error = write_lock_error
        .or(snapshot_error)
        .unwrap_or(preparation_error);
    if error.is_empty() && is_cancelled(&cancel_key) {
        error = "派发已取消".into();
    }

    if error.is_empty() {
        match channels::build_invocation(
            &node.agent,
            &node.provider,
            &node.model,
            &node.tier,
            &node.thinking,
            blind,
            (!effective_session.is_empty()).then_some(effective_session.as_str()),
            &outbound,
            Some(&task_file),
            session_dir.as_deref(),
            (!settings.cc_switch_path.trim().is_empty())
                .then(|| Path::new(settings.cc_switch_path.trim())),
            node.write != "只读",
        ) {
            Ok(invocation) => match run_process_registered(
                &invocation.program,
                &invocation.args,
                invocation.input.as_deref(),
                timeout,
                Some(&workdir),
                Some(&cancel_key),
            ) {
                Ok(process) => {
                    raw_stdout = process.stdout.clone();
                    raw_stderr = process.stderr.clone();
                    exit_code = process.code;
                    truncated = process.truncated;
                    timed_out = process.timed_out;
                    if process.timed_out {
                        status = "timeout".into();
                        error = format!("执行超过 {} 秒", process.timeout.as_secs());
                    } else {
                        match channels::parse_output(
                            &node.agent,
                            &node.provider,
                            &node.model,
                            &process,
                        ) {
                            Ok(output) => {
                                channel_output = output;
                                if channels::normalize_agent(&node.agent) == "pi"
                                    && channel_output.session_id.is_empty()
                                {
                                    channel_output.session_id.clone_from(&effective_session);
                                }
                                status = "ok".into();
                            }
                            Err(parse_error) => {
                                if parse_error.kind == ChannelErrorKind::IdentityMismatch {
                                    status = "identity_mismatch".into();
                                }
                                channel_output.identity = parse_error.identity;
                                error = parse_error.message;
                            }
                        }
                    }
                }
                Err(run_error) => error = run_error,
            },
            Err(build_error) => error = build_error,
        }
    }

    let mut validation_output = String::new();
    if status == "ok" && successful_validation_output(&node, worktree_path.is_some()).is_some() {
        validation_output = successful_validation_output(&node, true).unwrap().into();
    } else if status == "ok" && node.role == "执行" && node.write != "只读" {
        match run_validation(&settings.validation_command, &workdir, timeout) {
            Ok(output) => validation_output = output,
            Err(validation_error) => {
                validation_output = validation_error.clone();
                status = "validation_failed".into();
                error = validation_error;
            }
        }
    }
    let mut artifacts = None;
    if let Some(before) = before_snapshot {
        let built = snapshot_directory(&workdir).and_then(|after| {
            artifact_diff_for_project(project, run_root, &before, &after, timeout)
        });
        match built {
            Ok(mut value) => {
                value.worktree = worktree_path;
                let summary = safe_trace(&channel_output.text)
                    .chars()
                    .take(2000)
                    .collect::<String>();
                value.stdout_summary = if worktree_warning.is_empty() {
                    summary
                } else if summary.is_empty() {
                    worktree_warning.clone()
                } else {
                    format!("{worktree_warning}\n{summary}")
                };
                if settings.redact {
                    redacted += redact_artifacts(&mut value);
                }
                artifacts = Some(value);
            }
            Err(snapshot_error) => {
                status = "error".into();
                error = snapshot_error;
            }
        }
    }

    annotate_kimi_metadata(&node.agent, &mut channel_output, &mut error);

    // CLI 输出可能回显请求头或配置片段。无论外发脱敏开关如何，留痕与
    // 返回给前端的错误文本都先做安全过滤。
    let raw_stdout = safe_trace(&raw_stdout);
    let raw_stderr = safe_trace(&raw_stderr);
    let error = safe_trace(&error);

    let ended = Local::now();
    let duration_ms = timer.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let record = DispatchRecord {
        schema: "aicross-dispatch/1".into(),
        run_id: run_id.into(),
        stage,
        node: node.node_id.clone(),
        agent: node.agent.clone(),
        provider: node.provider.clone(),
        model: node.model.clone(),
        vendor: node.vendor.clone(),
        billing_source: node.billing_source.clone(),
        role: node.role.clone(),
        tier: node.tier.clone(),
        thinking: node.thinking.clone(),
        visibility: node.visibility.clone(),
        isolation: isolation.clone(),
        workdir: workdir.to_string_lossy().into_owned(),
        materials: material_names,
        redacted,
        truncated,
        status: status.clone(),
        started_at: started.to_rfc3339_opts(SecondsFormat::Secs, false),
        ended_at: ended.to_rfc3339_opts(SecondsFormat::Secs, false),
        duration_ms,
        tokens_in: channel_output.usage.input,
        tokens_out: channel_output.usage.output,
        cache_read: channel_output.usage.cache_read,
        identity: channel_output.identity.clone(),
        session_id: channel_output.session_id.clone(),
        exit_code,
        error: error.clone(),
    };
    let frontmatter = serialize_frontmatter(&record).unwrap_or_else(|e| format!("error: {e}"));
    let record_stdout = record_stdout(&raw_stdout, run_root, &node.node_id);
    let dispatch_file = write_record(
        log_dir,
        &record,
        &frontmatter,
        &outbound,
        &record_stdout,
        &raw_stderr,
    )
    .unwrap_or_default();

    let result = DispatchNodeResult {
        node_id: node.node_id,
        stage,
        status,
        // 回复正文会进 Reply 并随图落盘（~/.aicross/graphs），与留痕同样过滤
        text: safe_trace(&channel_output.text),
        raw_stdout: raw_stdout.clone(),
        raw_stderr,
        model_reported: channel_output.identity,
        session_id: channel_output.session_id,
        tokens_in: channel_output.usage.input,
        tokens_out: channel_output.usage.output,
        cache_read: channel_output.usage.cache_read,
        duration_ms,
        exit_code,
        timed_out,
        truncated,
        redacted,
        isolation,
        workdir: workdir.to_string_lossy().into_owned(),
        dispatch_file,
        frontmatter,
        output_preview: raw_stdout.chars().take(2000).collect(),
        error,
        artifacts,
        validation_output: safe_trace(&validation_output),
    };
    clear_cancelled(&cancel_key);
    result
}

fn prepare_outbound(
    node: &DispatchNodeRequest,
    redact: bool,
    workdir: &Path,
    blind: bool,
) -> Result<(String, Vec<String>, usize), String> {
    let (mut prompt, mut count) = redact_text(&node.prompt, redact);
    let mut names = Vec::new();
    if blind {
        if workdir.exists() {
            let mut entries =
                fs::read_dir(workdir).map_err(|error| format!("读取盲审目录失败：{error}"))?;
            if entries.next().is_some() {
                return Err(format!("盲审目录不是空目录：{}", workdir.display()));
            }
        } else {
            fs::create_dir_all(workdir).map_err(|error| format!("创建盲审目录失败：{error}"))?;
        }
        if !node.upstream_artifacts.is_empty() {
            let (request, hits) = redact_text(&node.original_request, redact);
            count += hits;
            fs::write(workdir.join("REQUEST.md"), request)
                .map_err(|error| format!("写入 REQUEST.md 失败：{error}"))?;
            let diffs = node
                .upstream_artifacts
                .iter()
                .map(|artifact| artifact.diff.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let (diffs, hits) = redact_text(&diffs, redact);
            count += hits;
            fs::write(workdir.join("diff.patch"), &diffs)
                .map_err(|error| format!("写入 diff.patch 失败：{error}"))?;
            let changed = workdir.join("changed");
            fs::create_dir_all(&changed)
                .map_err(|error| format!("创建 changed 目录失败：{error}"))?;
            for file in node
                .upstream_artifacts
                .iter()
                .flat_map(|artifact| artifact.changed_files.iter())
            {
                let relative = Path::new(&file.path);
                if relative.is_absolute()
                    || relative
                        .components()
                        .any(|part| matches!(part, std::path::Component::Prefix(_) | std::path::Component::ParentDir))
                {
                    return Err(format!("产物路径不是安全相对路径：{}", file.path));
                }
                let target = changed.join(relative);
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|error| format!("创建产物目录失败：{error}"))?;
                }
                let (content, hits) = redact_text(&file.content, redact);
                count += hits;
                fs::write(&target, content)
                    .map_err(|error| format!("写入产物文件失败 {}：{error}", target.display()))?;
            }
            names.extend(["REQUEST.md".into(), "diff.patch".into(), "changed/".into()]);
            prompt.push_str("\n\n");
            prompt.push_str(channels::BLIND_ARTIFACT_NOTICE);
            if diffs.len() < 30 * 1024 {
                prompt.push_str("\n\n[diff.patch 全文]\n");
                prompt.push_str(&diffs);
            }
        }
        let mut used = HashSet::new();
        let mut inline = Vec::new();
        let mut inline_bytes = 0_usize;
        let mut all_text = true;
        for source in &node.materials {
            let source = Path::new(source);
            if !source.is_file() {
                return Err(format!("附件不存在：{}", source.display()));
            }
            let name = source
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| format!("附件文件名无效：{}", source.display()))?;
            if !used.insert(name.to_string()) {
                return Err(format!("附件文件名重复：{name}"));
            }
            let target = workdir.join(name);
            let bytes = fs::read(source)
                .map_err(|error| format!("读取附件失败 {}：{error}", source.display()))?;
            if let Ok(text) = String::from_utf8(bytes.clone()) {
                let (safe, hits) = redact_text(&text, redact);
                count += hits;
                fs::write(&target, safe.as_bytes())
                    .map_err(|error| format!("复制附件失败 {}：{error}", target.display()))?;
                inline_bytes += safe.len();
                inline.push((name.to_string(), safe));
            } else {
                all_text = false;
                fs::write(&target, bytes)
                    .map_err(|error| format!("复制附件失败 {}：{error}", target.display()))?;
            }
            names.push(name.to_string());
        }
        if node.upstream_artifacts.is_empty() && all_text && inline_bytes < 30 * 1024 {
            for (name, text) in inline {
                prompt.push_str(&format!("\n\n[附件全文 {name}]\n{text}"));
            }
            if channels::normalize_agent(&node.agent) == "codex" && !node.materials.is_empty() {
                prompt.push_str("\n\n材料已在正文，勿再用工具读取。");
            }
        }
    } else {
        for source in &node.materials {
            if let Some(name) = Path::new(source).file_name().and_then(|name| name.to_str()) {
                names.push(name.to_string());
            }
        }
    }
    Ok((prompt, names, count))
}

fn redaction_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            r"(?is)-----BEGIN [^-\r\n]*PRIVATE KEY-----.*?-----END [^-\r\n]*PRIVATE KEY-----",
            r"(?i)Authorization:\s*Bearer\s+[^\s]+",
            r#"(?i)(?:["']\s*)?\b(?:password|passwd|pwd)\b\s*["']?\s*[:=]\s*(?:"(?:\\.|[^"\\\r\n])*"|'(?:\\.|[^'\\\r\n])*'|[^\r\n]+)"#,
            r#"(?i)(?:["']\s*)?\b(?:token|api[_-]?key|secret[_-]?key|access[_-]?key)\b\s*["']?\s*[:=]\s*(?:"(?:\\.|[^"\\\r\n])*"|'(?:\\.|[^'\\\r\n])*'|[^\r\n]+)"#,
            r"AKIA[0-9A-Z]{16}",
            r"(?i)\b(?:postgres|mysql|mongodb)://[^:\s/@]+:[^@\s/]+@",
        ]
        .into_iter()
        .map(|pattern| Regex::new(pattern).expect("redaction regex"))
        .collect()
    })
}

fn redact_text(text: &str, enabled: bool) -> (String, usize) {
    if !enabled {
        return (text.to_string(), 0);
    }
    let mut safe = text.to_string();
    let mut count = 0;
    for pattern in redaction_patterns() {
        count += pattern.find_iter(&safe).count();
        safe = pattern.replace_all(&safe, "[REDACTED]").into_owned();
    }
    (safe, count)
}

fn safe_trace(text: &str) -> String {
    redact_text(&strip_ansi(text), true).0
}

fn validation_failure(code: Option<i32>, output: &str) -> String {
    let exit = code.map_or_else(|| "未知".into(), |value| value.to_string());
    format!("验证命令失败（exit={exit}）\n{}", strip_ansi(output))
}

fn annotate_kimi_metadata(agent: &str, output: &mut ChannelOutput, error: &mut String) {
    if agent != "kimi" { return; }
    if output.identity.is_empty() {
        output.identity = "kimi 不报真身".into();
    }
    if output.usage.input == 0 && output.usage.output == 0 && output.usage.cache_read == 0 {
        *error = if error.is_empty() { "kimi 不报用量".into() } else { format!("{error}\nkimi 不报用量") };
    }
}

fn strip_ansi(text: &str) -> String {
    static ANSI: OnceLock<Regex> = OnceLock::new();
    ANSI.get_or_init(|| Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]").expect("ansi regex"))
        .replace_all(text, "")
        .into_owned()
}

fn broad_redaction_count(text: &str) -> usize {
    static BROAD: OnceLock<Regex> = OnceLock::new();
    BROAD
        .get_or_init(|| {
            Regex::new(
                r#"(?im)^\s*(?:password|passwd|pwd|token|api[_-]?key|secret[_-]?key|access[_-]?key)\s*[:=]\s*[^"'\r\n]+$"#,
            )
            .expect("broad redaction regex")
        })
        .find_iter(text)
        .count()
}

fn redaction_count(
    prompt: &str,
    materials: &[String],
    inspect_materials: bool,
) -> Result<usize, String> {
    let (_, mut count) = redact_text(prompt, true);
    if inspect_materials {
        for path in materials {
            let bytes = fs::read(path).map_err(|error| format!("读取附件失败 {path}：{error}"))?;
            if let Ok(text) = String::from_utf8(bytes) {
                count += redact_text(&text, true).1;
            }
        }
    }
    Ok(count)
}

fn project_path(value: &str) -> Result<PathBuf, String> {
    if value.trim().is_empty() {
        return Err("请先在左栏「工作区」选择项目目录".into());
    }
    let path = PathBuf::from(value);
    if !path.is_dir() {
        return Err(format!("项目目录不存在：{}", path.display()));
    }
    Ok(path)
}

fn ensure_log_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|error| format!("创建留痕目录失败：{error}"))?;
    let ignore = path.join(".gitignore");
    if !ignore.exists() {
        fs::write(&ignore, "*\n")
            .map_err(|error| format!("写入 .dispatch/.gitignore 失败：{error}"))?;
    }
    Ok(())
}

fn serialize_frontmatter(record: &DispatchRecord) -> Result<String, String> {
    let yaml = serde_yaml::to_string(record).map_err(|error| format!("序列化留痕失败：{error}"))?;
    Ok(format!("---\n{yaml}---"))
}

fn write_record(
    log_dir: &Path,
    record: &DispatchRecord,
    frontmatter: &str,
    prompt: &str,
    stdout: &str,
    stderr: &str,
) -> Result<String, String> {
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let base = format!(
        "{}-{}-{}-{}",
        stamp,
        if record.agent.trim().is_empty() { "unspecified".into() } else { safe_name(&record.agent) },
        safe_name(&record.model),
        safe_name(&record.role)
    );
    let mut path = log_dir.join(format!("{base}.md"));
    let mut suffix = 2;
    while path.exists() {
        path = log_dir.join(format!("{base}-{suffix}.md"));
        suffix += 1;
    }
    let mut body = format!("{frontmatter}\n\n## 任务全文\n\n{prompt}\n\n## 原始输出\n\n{stdout}\n");
    if !stderr.is_empty() {
        body.push_str("\n## stderr\n\n");
        body.push_str(stderr);
        body.push('\n');
    }
    write_atomic(&path, body.as_bytes())?;
    Ok(path.to_string_lossy().into_owned())
}

fn write_state(
    path: &Path,
    run_id: &str,
    stage: u32,
    concurrency: usize,
    status: &HashMap<String, String>,
    results: &[DispatchNodeResult],
) -> Result<(), String> {
    let mut active_node_ids = status
        .iter()
        .filter(|(_, state)| matches!(state.as_str(), "运行中" | "排队" | "等待写锁"))
        .map(|(node, _)| node.clone())
        .collect::<Vec<_>>();
    active_node_ids.sort();
    let header = DispatchStateHeader {
        schema: "aicross-state/1".into(),
        run_id: run_id.into(),
        stage,
        unfinished: !active_node_ids.is_empty(),
        active_node_ids,
    };
    let mut body = format!(
        "aicross-state: {}\n## 当前状态\n阶段: 派发第 {stage} 阶段 ｜ run: {run_id} ｜ 更新: {}\n## 已完成\n",
        serde_json::to_string(&header).map_err(|error| format!("生成状态头失败：{error}"))?,
        Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    let by_id: HashMap<&str, &DispatchNodeResult> = results
        .iter()
        .map(|result| (result.node_id.as_str(), result))
        .collect();
    let mut entries: Vec<_> = status.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    for (node, _state) in entries.iter().filter(|(_, state)| *state == "完成") {
        let file = by_id
            .get(node.as_str())
            .map(|result| result.dispatch_file.as_str())
            .unwrap_or("");
        body.push_str(&format!("- [x] {node} → {file}\n"));
    }
    body.push_str("## 进行中\n");
    for (node, state) in entries
        .iter()
        .filter(|(_, state)| *state == "运行中" || *state == "排队" || *state == "等待写锁")
    {
        body.push_str(&format!("- [ ] {node}: {state}\n"));
    }
    body.push_str("## 验证失败\n");
    for (node, state) in entries.iter().filter(|(_, state)| *state == "验证失败") {
        let file = by_id
            .get(node.as_str())
            .map(|result| result.dispatch_file.as_str())
            .unwrap_or("");
        body.push_str(&format!("- [ ] {node}: {state} → {file}\n"));
    }
    body.push_str("## 失败与超时\n");
    for (node, state) in entries
        .iter()
        .filter(|(_, state)| *state == "失败" || *state == "超时" || *state == "真身不符")
    {
        let file = by_id
            .get(node.as_str())
            .map(|result| result.dispatch_file.as_str())
            .unwrap_or("");
        body.push_str(&format!("- [ ] {node}: {state} → {file}\n"));
    }
    body.push_str(&format!(
        "## 决策记录\n- 同阶段并发上限 {concurrency}；同一 agent 同时最多 1 路\n"
    ));
    body.push_str("## 下一步\n");
    if status.values().all(|state| state == "完成") {
        body.push_str("- 查看并排结果，采纳、重派、升级或交人裁决\n");
    } else if status
        .values()
        .any(|state| state == "运行中" || state == "排队" || state == "等待写锁")
    {
        body.push_str("- 等待本阶段各路完成\n");
    } else {
        body.push_str("- 检查失败原因后重派或更换通道\n");
    }
    write_atomic(path, body.as_bytes())
}

// JSON 持久化的同类实现见 `orchestration::write_json_atomic`；统一收敛留待第 5 期。
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "目标路径缺少父目录".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("创建目录失败：{error}"))?;
    let temporary = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("dispatch"),
        Uuid::new_v4()
    ));
    let mut file =
        fs::File::create(&temporary).map_err(|error| format!("创建临时文件失败：{error}"))?;
    file.write_all(bytes)
        .map_err(|error| format!("写入临时文件失败：{error}"))?;
    file.sync_all()
        .map_err(|error| format!("同步临时文件失败：{error}"))?;
    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(first) => {
            if path.exists() {
                fs::remove_file(path)
                    .map_err(|error| format!("替换目标失败：{first}；删除旧文件失败：{error}"))?;
                fs::rename(&temporary, path).map_err(|error| format!("替换目标失败：{error}"))
            } else {
                Err(format!("替换目标失败：{first}"))
            }
        }
    }
}

fn safe_name(value: &str) -> String {
    let safe: String = value
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect();
    safe.trim_matches('-').to_string()
}

fn registry_key(run_id: &str, node_id: &str) -> String {
    format!("{run_id}:{node_id}")
}

fn emit_progress(app: &Option<AppHandle>, run_id: &str, node_id: &str, status: &str) {
    if let Some(app) = app {
        let _ = app.emit(
            "dispatch-progress",
            DispatchProgress {
                run_id: run_id.into(),
                node_id: node_id.into(),
                status: status.into(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default, PartialEq)]
    struct Classification {
        consensus: Vec<String>,
        disagreements: Vec<Value>,
        unique: Vec<Value>,
    }

    fn parse_classification(text: &str) -> Result<Classification, String> {
        let start = text
            .find('{')
            .ok_or_else(|| "归类结果缺少 JSON".to_string())?;
        let end = text
            .rfind('}')
            .ok_or_else(|| "归类结果缺少 JSON 结尾".to_string())?;
        let value: Value = serde_json::from_str(&text[start..=end])
            .map_err(|error| format!("归类 JSON 解析失败：{error}"))?;
        Ok(Classification {
            consensus: value
                .get("consensus")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
            disagreements: value
                .get("disagreements")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            unique: value
                .get("unique")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
        })
    }

    fn node(materials: Vec<String>) -> DispatchNodeRequest {
        DispatchNodeRequest {
            node_id: "node-1".into(),
            stage: 1,
            agent: "claude".into(),
            provider: String::new(),
            model: "sonnet".into(),
            vendor: "Anthropic".into(),
            billing_source: "官方订阅".into(),
            role: "验证".into(),
            tier: "中".into(),
            thinking: "中".into(),
            visibility: "盲".into(),
            prompt: "分析材料".into(),
            current_message: "分析材料".into(),
            materials,
            session_id: String::new(),
            write: "只读".into(),
            original_request: "分析材料".into(),
            upstream_artifacts: Vec::new(),
        }
    }

    fn stage_request(project_dir: String, nodes: Vec<DispatchNodeRequest>) -> DispatchStageRequest {
        DispatchStageRequest {
            run_id: "r3-test-run".into(),
            stage: 1,
            project_dir,
            nodes,
            settings: DispatchSettings::default(),
        }
    }

    #[test]
    fn r3_1_artifacts_serialize_with_snake_case_keys() {
        let value = serde_json::to_value(DispatchArtifacts {
            diff: "d".into(),
            changed_files: vec![ChangedFile { path: "a.txt".into(), content: "c".into() }],
            worktree: Some("wt".into()),
            stdout_summary: "s".into(),
        }).unwrap();
        let mut keys = value.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
        keys.sort();
        assert_eq!(keys, ["changed_files", "diff", "stdout_summary", "worktree"]);
        let changed = &value["changed_files"][0];
        let mut changed_keys = changed.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
        changed_keys.sort();
        assert_eq!(changed_keys, ["content", "path"]);
    }

    #[test]
    fn r3_3_empty_target_returns_node_error_without_creating_run_files() {
        let root = std::env::temp_dir().join(format!("aicross-r3-empty-target-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let mut empty = node(Vec::new());
        empty.agent.clear();
        let result = dispatch_stage_core(stage_request(root.to_string_lossy().into_owned(), vec![empty]), None).unwrap();
        assert_eq!(result.results[0].status, "error");
        assert!(result.results[0].error.contains("未指定"));
        assert!(fs::read_dir(&root).unwrap().next().is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn r3_7_large_stdout_is_externalized_but_small_stdout_is_inlined() {
        let root = std::env::temp_dir().join(format!("aicross-r3-stdout-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let large = "x".repeat(40 * 1024);
        let recorded = record_stdout(&large, &root, "node-large");
        assert!(recorded.len() <= 32 * 1024 + 512);
        assert!(recorded.contains("原始输出已转存"));
        assert_eq!(fs::read_to_string(root.join("node-large.raw.txt")).unwrap(), large);
        let small = "y".repeat(20 * 1024);
        assert_eq!(record_stdout(&small, &root, "node-small"), small);
        let failed_target = root.join("node-failed.raw.txt");
        fs::create_dir_all(&failed_target).unwrap();
        let failed = record_stdout(&large, &root, "node-failed");
        assert!(failed.contains("原始输出转存失败："));
        assert!(failed.contains("末尾 4KB"));
        assert!(!root.join("node-small.raw.txt").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn r3_8_empty_project_dir_rejects_preview_and_dispatch_without_writes() {
        let before = fs::read_dir(std::env::current_dir().unwrap()).unwrap().count();
        let request = stage_request(String::new(), vec![node(Vec::new())]);
        assert!(preview_core(&request).unwrap_err().contains("工作区"));
        assert!(dispatch_stage_core(request, None).unwrap_err().contains("工作区"));
        let after = fs::read_dir(std::env::current_dir().unwrap()).unwrap().count();
        assert_eq!(after, before);
    }

    #[test]
    fn r3_12_artifacts_and_blind_delivery_redact_secrets() {
        let secret = "sk-abcdef0123456789abcdef";
        let mut artifacts = DispatchArtifacts {
            diff: format!("+api_key={secret}"),
            changed_files: vec![ChangedFile { path: "a.txt".into(), content: format!("api_key={secret}") }],
            ..DispatchArtifacts::default()
        };
        let hits = redact_artifacts(&mut artifacts);
        assert!(hits >= 1);
        assert!(!artifacts.diff.contains(secret));
        assert!(!artifacts.changed_files[0].content.contains(secret));

        let root = std::env::temp_dir().join(format!("aicross-r3-blind-redact-{}", Uuid::new_v4()));
        let mut blind = node(Vec::new());
        blind.original_request = "review".into();
        blind.upstream_artifacts = vec![DispatchArtifacts {
            diff: format!("+api_key={secret}"),
            changed_files: vec![ChangedFile { path: "a.txt".into(), content: format!("api_key={secret}") }],
            ..DispatchArtifacts::default()
        }];
        let (_, _, count) = prepare_outbound(&blind, true, &root, true).unwrap();
        assert!(count >= 1);
        assert!(!fs::read_to_string(root.join("diff.patch")).unwrap().contains(secret));
        assert!(!fs::read_to_string(root.join("changed/a.txt")).unwrap().contains(secret));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn r3_13_missing_git_probe_falls_back_and_failed_materialization_leaves_no_diff_dir() {
        let root = std::env::temp_dir().join(format!("aicross-r3-git-fallback-{}", Uuid::new_v4()));
        let missing = root.join("missing-project");
        let fake_git = root.join("git-does-not-exist.exe");
        assert!(!project_is_git_work_tree_with(&fake_git, &missing, Duration::from_millis(50)).unwrap_or(false));
        let before = HashMap::from([("a.txt".into(), SnapshotFile { bytes: Some(b"old".to_vec()), ..SnapshotFile::default() })]);
        let after = HashMap::from([("a.txt".into(), SnapshotFile { bytes: Some(b"new".to_vec()), ..SnapshotFile::default() })]);
        fs::create_dir_all(&root).unwrap();
        let fallback = artifact_diff_for_project(&missing, &root, &before, &after, Duration::from_millis(50)).unwrap();
        assert!(!fallback.diff.is_empty());
        let diff_failed = artifact_diff_for_project_with_git(&fake_git, true, &root, &before, &after, Duration::from_millis(50)).unwrap();
        assert_eq!(diff_failed.diff, artifact_diff(&before, &after).diff);
        let blocked = root.join("blocked");
        fs::write(&blocked, "file").unwrap();
        assert!(artifact_diff_for_project_with_git(&fake_git, true, &blocked, &before, &after, Duration::from_millis(50)).is_err());
        let diff_dirs = fs::read_dir(&root).unwrap().filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("diff-")).count();
        assert_eq!(diff_dirs, 0);
        let normalized = normalize_git_diff("diff --git a/before/a.txt b/after/a.txt\n--- a/before/a.txt\n+++ b/after/a.txt\n+a/before/content");
        assert!(normalized.contains("+a/before/content"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn r3_14_blind_single_writer_nodes_do_not_take_the_project_write_lock() {
        let project = std::env::temp_dir().join(format!("aicross-r4-blind-lock-{}", Uuid::new_v4()));
        fs::create_dir_all(&project).unwrap();
        let _held = acquire_project_write(&project, Duration::from_secs(1), "writer").unwrap();
        let mut blind = node(Vec::new());
        blind.write = "单写者".into();
        blind.visibility = "盲".into();
        assert!(!requires_single_writer(&blind, true));
        assert!(!requires_single_writer(&blind, false));
        blind.visibility = "共享".into();
        assert!(requires_single_writer(&blind, true));
        let _ = fs::remove_dir_all(project);
    }

    #[test]
    fn r4_2_invalid_acceptance_write_returns_node_error() {
        let root = std::env::temp_dir().join(format!("aicross-r4-readonly-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let mut invalid = node(Vec::new());
        invalid.node_id = "invalid-review".into();
        invalid.write = "worktree".into();
        let result = dispatch_stage_core(stage_request(root.to_string_lossy().into_owned(), vec![invalid]), None).unwrap();
        assert_eq!(result.results.len(), 1);
        assert_eq!(result.results[0].node_id, "invalid-review");
        assert_eq!(result.results[0].status, "error");
        assert!(result.state_file.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn r4_1_worktree_fallback_runs_validation_in_effective_directory() {
        let root = std::env::temp_dir().join(format!("aicross-r4-validation-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        std::env::set_var("AICROSS_TEST_RUN_ROOT", root.join("runtime"));
        let mut worktree = node(Vec::new());
        worktree.role = "执行".into();
        worktree.visibility = "共享".into();
        worktree.write = "worktree".into();
        assert!(successful_validation_output(&worktree, false).is_none());
        let marker = root.join("validated.txt");
        #[cfg(windows)]
        let command = format!("Set-Content -LiteralPath '{}' -Value validated", marker.display());
        #[cfg(not(windows))]
        let command = format!("printf validated > '{}'", marker.display());
        let output = run_validation(&command, &root, Duration::from_secs(5)).unwrap();
        assert!(!output.contains("未跑验证命令"));
        assert!(marker.exists());
        std::env::remove_var("AICROSS_TEST_RUN_ROOT");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn r5_3_validation_error_strips_ansi_and_formats_exit_code() {
        assert_eq!(validation_failure(Some(1), "\x1b[33mfailed\x1b[0m"), "验证命令失败（exit=1）\nfailed");
    }

    #[test]
    fn r5_8_quoted_new_file_diff_headers_are_normalized() {
        let raw = "diff --git \"before/新 文件.txt\" \"after/新 文件.txt\"\n--- \"before/新 文件.txt\"\n+++ \"after/新 文件.txt\"";
        let normalized = normalize_git_diff(raw);
        assert!(normalized.contains("--- \"a/新 文件.txt\""));
        assert!(normalized.contains("+++ \"b/新 文件.txt\""));
        assert!(!normalized.contains("\"after/"));
    }

    #[test]
    fn r5_9_kimi_missing_identity_and_usage_are_explicit_in_trace_fields() {
        let mut output = ChannelOutput::default();
        let mut error = String::new();
        annotate_kimi_metadata("kimi", &mut output, &mut error);
        assert_eq!(output.identity, "kimi 不报真身");
        assert_eq!(error, "kimi 不报用量");
    }

    #[test]
    fn r3_16_worktree_validation_and_r3_17_abandoned_state_keep_run_identity() {
        let mut worktree = node(Vec::new());
        worktree.role = "执行".into();
        worktree.write = "worktree".into();
        assert!(successful_validation_output(&worktree, true).unwrap().contains("未跑验证命令"));

        let root = std::env::temp_dir().join(format!("aicross-r3-state-{}", Uuid::new_v4()));
        let dispatch = root.join(".dispatch");
        fs::create_dir_all(&dispatch).unwrap();
        let status = HashMap::from([("a".into(), "排队".into())]);
        write_state(&dispatch.join("STATE.md"), "run-from-state", 2, 1, &status, &[]).unwrap();
        let inspected = inspect_dispatch_state(root.to_string_lossy().into_owned()).unwrap();
        assert_eq!(inspected.run_id, "run-from-state");
        abandon_dispatch_state(root.to_string_lossy().into_owned()).unwrap();
        assert!(!dispatch.join("STATE.md").exists());
        assert!(fs::read_dir(&dispatch).unwrap().filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().starts_with("STATE.abandoned-")));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn frontmatter_roundtrips() {
        let record = DispatchRecord {
            schema: "aicross-dispatch/1".into(),
            run_id: "run".into(),
            stage: 1,
            node: "node".into(),
            agent: "codex".into(),
            provider: String::new(),
            model: "gpt".into(),
            vendor: "OpenAI".into(),
            billing_source: "官方订阅".into(),
            role: "验证".into(),
            tier: "低".into(),
            thinking: "中".into(),
            visibility: "盲".into(),
            isolation: "部分".into(),
            workdir: "x".into(),
            materials: vec!["a.csv".into()],
            redacted: 2,
            truncated: false,
            status: "ok".into(),
            started_at: "2026-01-01T00:00:00+08:00".into(),
            ended_at: "2026-01-01T00:00:01+08:00".into(),
            duration_ms: 1000,
            tokens_in: 10,
            tokens_out: 3,
            cache_read: 0,
            identity: "openai/gpt".into(),
            session_id: "s".into(),
            exit_code: Some(0),
            error: String::new(),
        };
        let yaml = serde_yaml::to_string(&record).unwrap();
        let restored: DispatchRecord = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(restored, record);
        assert!(serialize_frontmatter(&record).unwrap().starts_with("---\n"));
    }

    #[test]
    fn redacts_all_six_secret_shapes_and_stays_clean() {
        let cases = [
            "-----BEGIN RSA PRIVATE KEY-----\nabc\n-----END RSA PRIVATE KEY-----",
            "Authorization: Bearer top-secret",
            "password=hunter2",
            "api_key: sk-test-value",
            "AKIAABCDEFGHIJKLMNOP",
            "postgres://alice:secret@db.example/app",
            r#"{"password": "a b,c"}"#,
            r#"{"api_key":"sk-test value,tail"}"#,
        ];
        for case in cases {
            let (safe, count) = redact_text(case, true);
            assert_eq!(count, 1, "{case}");
            assert!(safe.contains("[REDACTED]"));
            assert_eq!(redact_text(&safe, true).1, 0, "脱敏后不得再命中：{safe}");
        }
    }

    #[test]
    fn trace_fields_are_always_sanitized() {
        for trace in [
            "stdout api_key=sk-stdout",
            "stderr password='two words,with comma'",
            "error Authorization: Bearer bearer-secret",
        ] {
            let safe = safe_trace(trace);
            assert!(safe.contains("[REDACTED]"));
            assert!(!safe.contains("sk-stdout"));
            assert!(!safe.contains("two words,with comma"));
            assert!(!safe.contains("bearer-secret"));
        }
    }

    #[test]
    fn blind_directory_contains_only_selected_materials() {
        let root = std::env::temp_dir().join(format!("aicross-blind-{}", Uuid::new_v4()));
        let project = root.join("project");
        let run = root.join("run");
        fs::create_dir_all(project.join(".dispatch")).unwrap();
        fs::write(project.join("CLAUDE.md"), "project rules").unwrap();
        fs::write(project.join("AGENTS.md"), "project rules").unwrap();
        fs::write(project.join("selected.csv"), "a,b\n1,2").unwrap();
        fs::write(project.join("other.txt"), "do not copy").unwrap();
        let request = node(vec![project
            .join("selected.csv")
            .to_string_lossy()
            .into_owned()]);
        let (_, names, _) = prepare_outbound(&request, true, &run, true).unwrap();
        assert_eq!(names, vec!["selected.csv"]);
        let copied: Vec<String> = fs::read_dir(&run)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(copied, vec!["selected.csv"]);
        assert!(!run.join("CLAUDE.md").exists());
        assert!(!run.join("AGENTS.md").exists());
        assert!(!run.join(".dispatch").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn preparation_failure_records_only_unsent_placeholder() {
        let root = std::env::temp_dir().join(format!("aicross-unsent-{}", Uuid::new_v4()));
        let project = root.join("project");
        let run_root = root.join("runs");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let mut request = node(vec![root
            .join("missing.txt")
            .to_string_lossy()
            .into_owned()]);
        request.prompt = r#"{"password": "not-for-disk"}"#.into();
        let result = run_one(
            "run",
            &project,
            &run_root,
            &log_dir,
            &DispatchSettings::default(),
            request,
        );
        assert_eq!(result.status, "error");
        let record = fs::read_to_string(result.dispatch_file).unwrap();
        assert!(record.contains("## 任务全文\n\n(未发送)"));
        assert!(!record.contains("not-for-disk"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn state_uses_actual_limit_and_separates_failures() {
        let root = std::env::temp_dir().join(format!("aicross-state-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("STATE.md");
        let status = HashMap::from([
            ("a".into(), "运行中".into()),
            ("b".into(), "失败".into()),
            ("c".into(), "验证失败".into()),
        ]);
        let failed = [
            DispatchNodeResult {
                node_id: "b".into(),
                dispatch_file: "b.md".into(),
                ..DispatchNodeResult::default()
            },
            DispatchNodeResult {
                node_id: "c".into(),
                dispatch_file: "c.md".into(),
                ..DispatchNodeResult::default()
            },
        ];
        write_state(&path, "run", 2, 2, &status, &failed).unwrap();
        let state = fs::read_to_string(&path).unwrap();
        assert!(state.contains("同阶段并发上限 2"));
        let header: DispatchStateHeader = serde_json::from_str(
            state.lines().next().unwrap().strip_prefix("aicross-state: ").unwrap(),
        )
        .unwrap();
        assert_eq!(header.run_id, "run");
        assert!(header.unfinished);
        assert_eq!(header.active_node_ids, ["a"]);
        assert!(state.contains("## 进行中\n- [ ] a: 运行中"));
        assert!(state.contains("## 验证失败\n- [ ] c: 验证失败 → c.md"));
        assert!(state.contains("## 失败与超时\n- [ ] b: 失败 → b.md"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn inspect_state_uses_machine_header_instead_of_status_prose() {
        let root = std::env::temp_dir().join(format!("aicross-state-header-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join(".dispatch")).unwrap();
        fs::write(
            root.join(".dispatch/STATE.md"),
            "aicross-state: {\"schema\":\"aicross-state/1\",\"run_id\":\"run-1\",\"stage\":2,\"unfinished\":false,\"active_node_ids\":[]}\n- old: 运行中\n",
        )
        .unwrap();
        let summary = inspect_dispatch_state(root.to_string_lossy().into_owned()).unwrap();
        assert!(!summary.unfinished);
        assert_eq!(summary.run_id, "run-1");
        assert!(summary.active_node_ids.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn classification_parser_tolerates_fences_and_missing_arrays() {
        let parsed = parse_classification(
            "模型说明\n```json\n{\"consensus\":[\"A\"],\"unique\":[]}\n```\n结束",
        )
        .unwrap();
        assert_eq!(parsed.consensus, vec!["A"]);
        assert!(parsed.disagreements.is_empty());
        assert!(parsed.unique.is_empty());
    }

    #[test]
    fn snapshot_builds_diff_and_changed_file_content() {
        let before = HashMap::from([(
            "a.py".into(),
            SnapshotFile {
                bytes: Some(b"old\n".to_vec()),
                ..SnapshotFile::default()
            },
        )]);
        let after = HashMap::from([
            (
                "a.py".into(),
                SnapshotFile {
                    bytes: Some(b"new\n".to_vec()),
                    ..SnapshotFile::default()
                },
            ),
            (
                "b.py".into(),
                SnapshotFile {
                    bytes: Some(b"added\n".to_vec()),
                    ..SnapshotFile::default()
                },
            ),
        ]);
        let artifacts = artifact_diff(&before, &after);
        assert!(artifacts.diff.contains("--- a/a.py"));
        assert!(artifacts.diff.contains("+++ b/b.py"));
        assert!(artifacts.diff.contains("@@ -0,0 +1,1 @@\n+added"));
        assert_eq!(artifacts.changed_files.len(), 2);
    }

    #[test]
    fn artifact_diff_covers_deleted_binary_large_and_oversized_markers() {
        let before = HashMap::from([
            ("deleted.txt".into(), SnapshotFile { bytes: Some(b"gone\n".to_vec()), ..SnapshotFile::default() }),
            ("binary.bin".into(), SnapshotFile { bytes: Some(vec![0xff, 0x00]), ..SnapshotFile::default() }),
            ("large.txt".into(), SnapshotFile { bytes: Some(vec![b'a'; 200 * 1024 + 1]), ..SnapshotFile::default() }),
            ("huge.dat".into(), SnapshotFile { bytes: None, size: SNAPSHOT_FILE_LIMIT + 1, modified_ns: 1 }),
        ]);
        let after = HashMap::from([
            ("binary.bin".into(), SnapshotFile { bytes: Some(vec![0xfe, 0x00]), ..SnapshotFile::default() }),
            ("large.txt".into(), SnapshotFile { bytes: Some(vec![b'b'; 200 * 1024 + 1]), ..SnapshotFile::default() }),
            ("huge.dat".into(), SnapshotFile { bytes: None, size: SNAPSHOT_FILE_LIMIT + 1, modified_ns: 2 }),
        ]);
        let artifacts = artifact_diff(&before, &after);
        assert!(artifacts.diff.contains("--- a/deleted.txt\n+++ /dev/null"));
        assert!(artifacts.diff.contains("Binary files a/binary.bin and b/binary.bin differ"));
        assert!(artifacts.diff.contains("Binary files a/huge.dat and b/huge.dat differ"));
        assert!(artifacts.diff.contains("文件：large.txt\n全文见 changed/"));
        assert!(!artifacts.changed_files.iter().any(|file| file.path == "large.txt"));
    }

    #[test]
    fn git_repository_uses_three_line_unified_diff() {
        let root = std::env::temp_dir().join(format!("aicross-git-diff-{}", Uuid::new_v4()));
        let project = root.join("project");
        let scratch = root.join("scratch");
        fs::create_dir_all(&project).unwrap();
        let init = run_process_registered(
            Path::new("git"),
            &["init".into()],
            None,
            Duration::from_secs(5),
            Some(&project),
            None,
        )
        .unwrap();
        assert_eq!(init.code, Some(0));
        let before_text = "unchanged-1\r\nunchanged-2\r\nunchanged-3\r\nold\r\nunchanged-5\r\nunchanged-6\r\nunchanged-7\r\nunchanged-8\r\n";
        let after_text = before_text.replace("old", "new");
        let before = HashMap::from([("a.txt".into(), SnapshotFile { bytes: Some(before_text.as_bytes().to_vec()), ..SnapshotFile::default() })]);
        let after = HashMap::from([("a.txt".into(), SnapshotFile { bytes: Some(after_text.into_bytes()), ..SnapshotFile::default() })]);
        let artifacts = artifact_diff_for_project(
            &project,
            &scratch,
            &before,
            &after,
            Duration::from_secs(5),
        )
        .unwrap();
        assert!(artifacts.diff.contains("@@ -1,7 +1,7 @@"));
        assert!(artifacts.diff.contains(" unchanged-1"));
        assert!(!artifacts.diff.contains(" unchanged-8"));
        assert!(artifacts.diff.contains("-old"));
        assert!(artifacts.diff.contains("+new"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn snapshot_marks_files_over_eight_megabytes_without_reading_content() {
        let root = std::env::temp_dir().join(format!("aicross-large-snapshot-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("large.bin");
        let file = fs::File::create(&path).unwrap();
        file.set_len(SNAPSHOT_FILE_LIMIT + 1).unwrap();
        let snapshot = snapshot_directory(&root).unwrap();
        let entry = snapshot.get("large.bin").unwrap();
        assert!(entry.bytes.is_none());
        assert_eq!(entry.size, SNAPSHOT_FILE_LIMIT + 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn snapshot_skips_excluded_directories_and_enforces_total_bytes() {
        let root = std::env::temp_dir().join(format!("aicross-limited-snapshot-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join(".venv")).unwrap();
        fs::create_dir_all(root.join("coverage")).unwrap();
        fs::write(root.join(".venv/secret.txt"), b"skip").unwrap();
        fs::write(root.join("coverage/report.txt"), b"skip").unwrap();
        fs::write(root.join("a.txt"), b"1234").unwrap();
        let snapshot = snapshot_directory_with_limits(&root, 16, 16).unwrap();
        assert_eq!(snapshot.keys().cloned().collect::<Vec<_>>(), vec!["a.txt"]);
        fs::write(root.join("b.txt"), b"5678").unwrap();
        assert!(snapshot_directory_with_limits(&root, 16, 7)
            .unwrap_err()
            .contains("总字节上限"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_write_lock_is_mutually_exclusive() {
        let root = std::env::temp_dir().join(format!("aicross-lock-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let first = acquire_project_write(&root, Duration::from_secs(1), "lock-first").unwrap();
        let path = root.clone();
        let (sender, receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            let _second = acquire_project_write(&path, Duration::from_secs(1), "lock-second").unwrap();
            sender.send(()).unwrap();
        });
        assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
        drop(first);
        receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        worker.join().unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_write_lock_wait_can_be_cancelled() {
        let root = std::env::temp_dir().join(format!("aicross-lock-cancel-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let first = acquire_project_write(&root, Duration::from_secs(2), "cancel-first").unwrap();
        let path = root.clone();
        let worker = thread::spawn(move || {
            acquire_project_write(&path, Duration::from_secs(2), "cancel-second")
                .map(|_| ())
        });
        thread::sleep(Duration::from_millis(50));
        mark_cancelled("cancel-second");
        let error = worker.join().unwrap().unwrap_err();
        assert!(error.contains("已取消"));
        clear_cancelled("cancel-second");
        drop(first);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn non_git_worktree_is_accounted_as_single_writer() {
        let mut request = node(Vec::new());
        request.write = "worktree".into();
        request.visibility = "共享".into();
        assert!(requires_single_writer(&request, true));
        assert!(!requires_single_writer(&request, false));
    }

    #[test]
    fn node_stage_is_used_for_result_and_record() {
        let root = std::env::temp_dir().join(format!("aicross-node-stage-{}", Uuid::new_v4()));
        let project = root.join("project");
        let run_root = root.join("runs");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let mut request = node(Vec::new());
        request.agent = "missing-agent".into();
        request.visibility = "共享".into();
        request.stage = 7;
        let result = run_one(
            "stage-run",
            &project,
            &run_root,
            &log_dir,
            &DispatchSettings::default(),
            request,
        );
        assert_eq!(result.stage, 7);
        assert!(result.frontmatter.contains("stage: 7"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn blind_acceptance_directory_contains_only_artifact_delivery() {
        let root = std::env::temp_dir().join(format!("aicross-artifact-{}", Uuid::new_v4()));
        let mut request = node(Vec::new());
        request.original_request = "逐字原始需求".into();
        request.upstream_artifacts = vec![DispatchArtifacts {
            diff: "--- a/a.py\n+++ b/a.py\n".into(),
            changed_files: vec![ChangedFile {
                path: "a.py".into(),
                content: "print('ok')\n".into(),
            }],
            stdout_summary: "done".into(),
            worktree: None,
        }];
        let (prompt, _, _) = prepare_outbound(&request, true, &root, true).unwrap();
        let mut entries = fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        entries.sort();
        for required in ["REQUEST.md", "changed", "diff.patch"] {
            assert!(entries.iter().any(|entry| entry == required));
        }
        for project_entry in ["AGENTS.md", "CLAUDE.md", ".dispatch", "fixtures"] {
            assert!(!entries.iter().any(|entry| entry == project_entry));
        }
        assert!(prompt.contains(channels::BLIND_ARTIFACT_NOTICE));
        assert!(prompt.contains("[diff.patch 全文]"));
        assert!(prompt.contains("--- a/a.py"));
        assert_eq!(
            fs::read_to_string(root.join("REQUEST.md")).unwrap(),
            "逐字原始需求"
        );
        assert!(root.join("changed/a.py").is_file());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn blind_acceptance_rejects_drive_relative_changed_path() {
        let root = std::env::temp_dir().join(format!("aicross-artifact-prefix-{}", Uuid::new_v4()));
        let mut request = node(Vec::new());
        request.upstream_artifacts = vec![DispatchArtifacts {
            changed_files: vec![ChangedFile { path: "C:escape.txt".into(), content: "no".into() }],
            ..DispatchArtifacts::default()
        }];
        let error = prepare_outbound(&request, true, &root, true).unwrap_err();
        assert!(error.contains("产物路径不是安全相对路径"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn broad_redaction_warning_only_counts_unquoted_whole_lines() {
        assert_eq!(
            broad_redaction_count("token=ordinary prose\n{\"token\":\"secret\"}"),
            1
        );
    }

    fn live_node(
        id: &str,
        agent: &str,
        provider: &str,
        model: &str,
        role: &str,
        visibility: &str,
        prompt: &str,
        current_message: &str,
        materials: Vec<String>,
        session_id: &str,
    ) -> DispatchNodeRequest {
        DispatchNodeRequest {
            node_id: id.into(),
            stage: 1,
            agent: agent.into(),
            provider: provider.into(),
            model: model.into(),
            vendor: match agent {
                "codex" => "OpenAI",
                "kimi" => "Moonshot AI",
                _ => provider,
            }
            .into(),
            billing_source: "本机 CLI 配置".into(),
            role: role.into(),
            tier: "中".into(),
            thinking: "中".into(),
            visibility: visibility.into(),
            prompt: prompt.into(),
            current_message: current_message.into(),
            materials,
            session_id: session_id.into(),
            write: "只读".into(),
            original_request: prompt.into(),
            upstream_artifacts: Vec::new(),
        }
    }

    fn live_settings(concurrency: usize) -> DispatchSettings {
        DispatchSettings {
            timeout_secs: 600,
            concurrency,
            redact: true,
            write_project_dispatch: true,
            cc_switch_path: String::new(),
            validation_command: String::new(),
            max_requests: 8,
        }
    }

    fn user_data_root_is_writable() -> bool {
        let Ok(root) = aicross_root() else {
            return false;
        };
        if fs::create_dir_all(&root).is_err() {
            return false;
        }
        let probe = root.join(format!(".task03-write-probe-{}", Uuid::new_v4()));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)
        {
            Ok(_) => {
                let _ = fs::remove_file(probe);
                true
            }
            Err(_) => false,
        }
    }

    #[test]
    #[ignore = "real model requests; run once for TASK-03 acceptance"]
    fn live_blind_review() {
        let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let enforce_live_success = user_data_root_is_writable();
        std::env::set_var("AICROSS_TEST_RUN_ROOT", project.join(".dispatch/runtime"));
        let fixtures = project.join("fixtures/blind-task");
        let materials = vec![
            fixtures.join("plan.md").to_string_lossy().into_owned(),
            fixtures.join("data.csv").to_string_lossy().into_owned(),
        ];
        let prompt = concat!(
            "[系统] 这是一次盲审。不要推测或提及其他模型的观点，只根据当前任务与所附材料独立作答。\n\n",
            "[用户] 请独立审查社区降温点位试验计划及数据。给出主要发现、数据局限、风险和下一轮最低成本验证方案；明确区分材料直接支持的事实与推断。\n\n",
            "[附件] plan.md\n[附件] data.csv"
        );
        let original = vec![
            live_node(
                "blind-pi-glm",
                "pi",
                "zai-coding-cn",
                "glm-5.3-flash",
                "数据审查",
                "盲",
                prompt,
                prompt,
                materials.clone(),
                "",
            ),
            live_node(
                "blind-codex-luna",
                "codex",
                "",
                "gpt-5.6-luna",
                "风险审查",
                "盲",
                prompt,
                prompt,
                materials.clone(),
                "",
            ),
            live_node(
                "blind-kimi-fast",
                "kimi",
                "kimi",
                "kimi-code/kimi-for-coding-highspeed",
                "方案审查",
                "盲",
                prompt,
                prompt,
                materials.clone(),
                "",
            ),
        ];
        let result = dispatch_stage_core(
            DispatchStageRequest {
                run_id: format!("task03-blind-{}", Local::now().format("%Y%m%d-%H%M%S")),
                stage: 1,
                project_dir: project.to_string_lossy().into_owned(),
                nodes: original,
                settings: live_settings(3),
            },
            None,
        )
        .unwrap();
        println!("BLIND_RUN={} STATE={}", result.run_id, result.state_file);
        for item in &result.results {
            println!(
                "BLIND_RESULT node={} status={} record={} isolation={} identity_present={}",
                item.node_id,
                item.status,
                item.dispatch_file,
                item.isolation,
                !item.model_reported.is_empty()
            );
            assert!(!item.dispatch_file.is_empty());
            if enforce_live_success {
                assert_eq!(item.status, "ok", "{}: {}", item.node_id, item.error);
                // kimi 的 stream-json 不带 usage（任务书 A 表：有就记，否则 —），不对它要求用量
                if item.node_id != "blind-kimi-fast" {
                    assert!(item.tokens_in > 0, "{} tokens_in 为 0", item.node_id);
                    assert!(item.tokens_out > 0, "{} tokens_out 为 0", item.node_id);
                }
            }
        }
        if !enforce_live_success {
            println!("BLIND_ASSERTIONS_SKIPPED=用户数据目录不可写，当前为受限环境");
        }

        if let Some(failed) = result.results.iter().find(|item| item.status != "ok") {
            let fallback = dispatch_stage_core(
                DispatchStageRequest {
                    run_id: format!("task03-fallback-{}", Local::now().format("%Y%m%d-%H%M%S")),
                    stage: 1,
                    project_dir: project.to_string_lossy().into_owned(),
                    nodes: vec![live_node(
                        &format!("fallback-for-{}", failed.node_id),
                        "pi",
                        "deepseek",
                        "deepseek-v4-flash",
                        "替补审查",
                        "盲",
                        prompt,
                        prompt,
                        materials,
                        "",
                    )],
                    settings: live_settings(1),
                },
                None,
            )
            .unwrap();
            let item = &fallback.results[0];
            println!(
                "BLIND_FALLBACK for={} status={} record={} identity_present={}",
                failed.node_id,
                item.status,
                item.dispatch_file,
                !item.model_reported.is_empty()
            );
            assert!(!item.dispatch_file.is_empty());
        }
    }

    #[test]
    #[ignore = "two real model requests; run once for TASK-03 acceptance"]
    fn live_shared_continuation() {
        let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let enforce_live_success = user_data_root_is_writable();
        std::env::set_var("AICROSS_TEST_RUN_ROOT", project.join(".dispatch/runtime"));
        let marker = "青松-731";
        let first_message =
            format!("请记住本次验证代号 {marker}，并只回答：已记住 {marker}。不要读取项目文件。");
        let first = dispatch_stage_core(
            DispatchStageRequest {
                run_id: format!("task03-cont-a-{}", Local::now().format("%Y%m%d-%H%M%S")),
                stage: 2,
                project_dir: project.to_string_lossy().into_owned(),
                nodes: vec![live_node(
                    "continue-pi-a",
                    "pi",
                    "zai-coding-cn",
                    "glm-5.3-flash",
                    "续接验证",
                    "共享",
                    &first_message,
                    &first_message,
                    Vec::new(),
                    "",
                )],
                settings: live_settings(1),
            },
            None,
        )
        .unwrap();
        let first_item = &first.results[0];
        if !enforce_live_success && first_item.status != "ok" {
            println!(
                "CONTINUATION_ASSERTIONS_SKIPPED=用户数据目录不可写，当前为受限环境；首轮错误={}",
                first_item.error
            );
            return;
        }
        assert_eq!(first_item.status, "ok", "首轮失败：{}", first_item.error);
        assert!(first_item.tokens_in > 0, "首轮 tokens_in 为 0");
        assert!(first_item.tokens_out > 0, "首轮 tokens_out 为 0");
        assert!(!first_item.session_id.is_empty(), "首轮未返回 session_id");

        let second_message = "请只回答我上一条消息要求你记住的验证代号，不要解释。";
        let full_context = format!(
            "[用户] {first_message}\n[模型] {}\n[用户] {second_message}",
            first_item.text
        );
        let second = dispatch_stage_core(
            DispatchStageRequest {
                run_id: format!("task03-cont-b-{}", Local::now().format("%Y%m%d-%H%M%S")),
                stage: 3,
                project_dir: project.to_string_lossy().into_owned(),
                nodes: vec![live_node(
                    "continue-pi-b",
                    "pi",
                    "zai-coding-cn",
                    "glm-5.3-flash",
                    "续接验证",
                    "共享",
                    &full_context,
                    second_message,
                    Vec::new(),
                    &first_item.session_id,
                )],
                settings: live_settings(1),
            },
            None,
        )
        .unwrap();
        let second_item = &second.results[0];
        println!(
            "CONTINUATION first_record={} second_record={} first_session_present={} second_status={} marker_recalled={}",
            first_item.dispatch_file,
            second_item.dispatch_file,
            !first_item.session_id.is_empty(),
            second_item.status,
            second_item.text.contains(marker)
        );
        assert_eq!(second_item.status, "ok", "续接失败：{}", second_item.error);
        assert!(second_item.tokens_in > 0, "续接 tokens_in 为 0");
        assert!(second_item.tokens_out > 0, "续接 tokens_out 为 0");
        assert!(second_item.text.contains(marker), "续接未记住验证代号");
    }

    #[test]
    #[ignore = "three real model requests; run once outside the sandbox for TASK-04 acceptance"]
    fn live_economy_pipeline() {
        const BLIND_PREFIX: &str = include_str!("../fixtures/prompts/blind-prefix.txt");
        const PLAN_TEMPLATE: &str = include_str!("../fixtures/prompts/planning.txt");
        const EXECUTION_TEMPLATE: &str = include_str!("../fixtures/prompts/execution.txt");
        const ACCEPTANCE_TEMPLATE: &str = include_str!("../fixtures/prompts/acceptance.txt");
        let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let project = std::env::temp_dir().join(format!("aicross-pipeline-{}", Uuid::new_v4()));
        fs::create_dir_all(project.join("fixtures/pipeline")).unwrap();
        fs::copy(
            repository.join("fixtures/pipeline/roman.py"),
            project.join("fixtures/pipeline/roman.py"),
        )
        .unwrap();
        std::env::set_var("AICROSS_TEST_RUN_ROOT", project.join(".aicross-runtime"));
        let original = "给 fixtures/pipeline/roman.py 补一个把整数转罗马数字的函数并写 3 个测试";

        let mut plan = live_node(
            "pipeline-plan",
            "pi",
            "zai-coding-cn",
            "glm-5.3-flash",
            "出方案",
            "共享",
            &format!("{original}\n\n{}", PLAN_TEMPLATE.trim()),
            original,
            Vec::new(),
            "",
        );
        plan.thinking = "高".into();
        plan.stage = 1;
        let plan_result = dispatch_stage_core(
            DispatchStageRequest {
                run_id: format!("task04-pipeline-{}", Local::now().format("%Y%m%d-%H%M%S")),
                stage: 1,
                project_dir: project.to_string_lossy().into_owned(),
                nodes: vec![plan],
                settings: live_settings(1),
            },
            None,
        )
        .unwrap();
        let plan_item = &plan_result.results[0];
        assert_eq!(plan_item.status, "ok", "{}", plan_item.error);

        let mut execute = live_node(
            "pipeline-execute", "codex", "", "gpt-5.6-luna", "执行", "共享",
            &format!("{}\n\n原始需求：{original}\n\n方案：{}", EXECUTION_TEMPLATE.trim(), plan_item.text),
            original, Vec::new(), "",
        );
        execute.write = "单写者".into();
        execute.stage = 2;
        let mut execute_settings = live_settings(1);
        execute_settings.validation_command = "python -m pytest".into();
        let execute_result = dispatch_stage_core(
            DispatchStageRequest {
                run_id: plan_result.run_id.clone(),
                stage: 2,
                project_dir: project.to_string_lossy().into_owned(),
                nodes: vec![execute],
                settings: execute_settings,
            },
            None,
        )
        .unwrap();
        let execute_item = &execute_result.results[0];
        assert_eq!(execute_item.status, "ok", "{}", execute_item.error);
        assert!(
            !execute_item.validation_output.trim().is_empty(),
            "执行节点没有保存验证命令输出"
        );
        let artifacts = execute_item
            .artifacts
            .clone()
            .expect("执行节点必须返回产物");
        assert!(!artifacts.diff.trim().is_empty(), "执行产物 diff 为空");

        let mut accept = live_node(
            "pipeline-accept", "kimi", "kimi", "kimi-code/k3-256k", "验收", "盲",
            &format!("{}\n\n[用户]\n{original}\n\n{}", BLIND_PREFIX.trim(), ACCEPTANCE_TEMPLATE.trim()),
            original, Vec::new(), "",
        );
        accept.stage = 3;
        accept.original_request = original.into();
        accept.upstream_artifacts = vec![artifacts];
        let accept_result = dispatch_stage_core(
            DispatchStageRequest {
                run_id: plan_result.run_id,
                stage: 3,
                project_dir: project.to_string_lossy().into_owned(),
                nodes: vec![accept],
                settings: live_settings(1),
            },
            None,
        )
        .unwrap();
        let accept_item = &accept_result.results[0];
        assert_eq!(accept_item.status, "ok", "{}", accept_item.error);
        let mut blind_entries = fs::read_dir(&accept_item.workdir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        blind_entries.sort();
        assert_eq!(blind_entries, ["REQUEST.md", "changed", "diff.patch"]);
        assert!(!Path::new(&accept_item.workdir).join("roman.py").exists());
        assert!(project.join(".dispatch/STATE.md").is_file());
        let dispatch_records = fs::read_dir(project.join(".dispatch"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.path().extension().and_then(|value| value.to_str()) == Some("md")
                    && entry.file_name() != "STATE.md"
            })
            .count();
        assert_eq!(dispatch_records, 3, "每个阶段应各有一份派发记录");
        let request_count = plan_result.results.len()
            + execute_result.results.len()
            + accept_result.results.len();
        assert!(request_count <= 8, "请求总数超过 8：{request_count}");
        println!(
            "PIPELINE_RUN={} REQUESTS=3 STATE={}",
            accept_result.run_id, accept_result.state_file
        );
        let _ = fs::remove_dir_all(project);
    }

    #[test]
    #[ignore = "one real cc-switch request; requires AICROSS_CC_SWITCH_PATH and provider"]
    fn live_cc_switch_route() {
        let Ok(script) = std::env::var("AICROSS_CC_SWITCH_PATH") else {
            panic!("需要 AICROSS_CC_SWITCH_PATH 与 AICROSS_CC_SWITCH_PROVIDER");
        };
        let provider = std::env::var("AICROSS_CC_SWITCH_PROVIDER").unwrap_or_default();
        if provider.is_empty() {
            panic!("需要 AICROSS_CC_SWITCH_PATH 与 AICROSS_CC_SWITCH_PROVIDER");
        }
        let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut request = live_node(
            "cc-switch-live",
            "cc-switch",
            &provider,
            "cc-switch",
            "验证",
            "共享",
            "只回答 OK",
            "只回答 OK",
            Vec::new(),
            "",
        );
        request.tier = "低".into();
        let mut settings = live_settings(1);
        settings.cc_switch_path = script;
        let result = dispatch_stage_core(
            DispatchStageRequest {
                run_id: format!("task04-cc-switch-{}", Local::now().format("%Y%m%d-%H%M%S")),
                stage: 1,
                project_dir: project.to_string_lossy().into_owned(),
                nodes: vec![request],
                settings,
            },
            None,
        )
        .unwrap();
        assert_eq!(
            result.results[0].status, "ok",
            "{}",
            result.results[0].error
        );
        assert!(result.results[0].tokens_in > 0, "cc-switch tokens_in 为 0");
        println!(
            "CC_SWITCH_REQUESTS=1 RECORD={}",
            result.results[0].dispatch_file
        );
    }
}
