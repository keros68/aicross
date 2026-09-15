//! TASK-16 控制台通道。
//!
//! HTTP 路由挂在 `hook_server` 的同一个 loopback listener 上；本模块负责
//! Bearer 令牌后的协议、图写回、派发状态和 `console.json` 生命周期。

use crate::dispatch::{
    self, DispatchArtifacts, DispatchNodeRequest, DispatchNodeResult, DispatchSettings,
    DispatchStageRequest,
};
use crate::orchestration::{
    self, Graph, GraphNode, Message, Reply, ReplyArtifacts, ReplyChangedFile, Target,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

const CONSOLE_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ConsoleManifest {
    pub version: u32,
    pub endpoint: String,
    pub token: String,
    pub pid: u32,
    pub started_at: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ConsoleDispatchRequest {
    pub project: String,
    pub text: String,
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(default)]
    pub targets: Option<Vec<Target>>,
    #[serde(default)]
    pub attachments: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ConsoleDispatchResponse {
    pub run_id: String,
    pub graph_id: String,
    pub node_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ConsoleRunNode {
    pub node_id: String,
    pub role: String,
    pub agent: String,
    pub model: String,
    pub status: String,
    pub text_excerpt: String,
    pub artifacts_dir: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ConsoleRun {
    pub run_id: String,
    pub graph_id: String,
    pub project: String,
    pub status: String,
    pub nodes: Vec<ConsoleRunNode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub leftover: Vec<String>,
    pub started_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ConsoleEvent {
    pub kind: String,
    pub graph_id: String,
    pub node_ids: Vec<String>,
    pub runs: Vec<ConsoleRun>,
}

#[derive(Debug)]
pub struct ApiError {
    pub status: u16,
    pub reason: String,
}

impl ApiError {
    fn new(status: u16, reason: impl Into<String>) -> Self {
        Self { status, reason: reason.into() }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ApiResponse {
    pub status: u16,
    pub body: String,
}

pub trait ConsoleBackend: Send + Sync {
    fn dispatch(&self, request: ConsoleDispatchRequest) -> Result<ConsoleDispatchResponse, ApiError>;
    fn status(&self, run_id: &str) -> Result<ConsoleRun, ApiError>;
    fn adopt(&self, run_id: &str, node_id: &str) -> Result<ConsoleRun, ApiError>;
    fn cancel(&self, run_id: &str) -> Result<ConsoleRun, ApiError>;
}

#[derive(Clone)]
pub struct LiveBackend {
    app: AppHandle,
}

impl LiveBackend {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl ConsoleBackend for LiveBackend {
    fn dispatch(&self, request: ConsoleDispatchRequest) -> Result<ConsoleDispatchResponse, ApiError> {
        start_dispatch(self.app.clone(), request)
    }

    fn status(&self, run_id: &str) -> Result<ConsoleRun, ApiError> {
        run_by_id(run_id)
    }

    fn adopt(&self, run_id: &str, node_id: &str) -> Result<ConsoleRun, ApiError> {
        let run = run_by_id(run_id)?;
        if !run.nodes.iter().any(|node| node.node_id == node_id) {
            return Err(ApiError::new(404, format!("该 run 中没有节点：{node_id}")));
        }
        orchestration::adopt_graph_node(run.graph_id.clone(), node_id.to_string(), true)
            .map_err(|reason| ApiError::new(409, reason))?;
        emit_runs(&self.app, "adopted", &run.graph_id, &[node_id.to_string()]);
        run_by_id(run_id)
    }

    fn cancel(&self, run_id: &str) -> Result<ConsoleRun, ApiError> {
        let mut runs = runs().lock().map_err(|_| ApiError::new(409, "运行状态不可用"))?;
        let run = runs.get_mut(run_id).ok_or_else(|| ApiError::new(404, format!("找不到 run：{run_id}")))?;
        for node in &run.nodes {
            if node.status == "running" {
                dispatch::cancel_dispatch_node(run_id.to_string(), node.node_id.clone());
            }
        }
        run.status = "failed".into();
        for node in &mut run.nodes {
            if node.status == "running" {
                node.status = "failed".into();
                node.text_excerpt = "派发已取消".into();
            }
        }
        let result = run.clone();
        drop(runs);
        emit_runs(&self.app, "cancelled", &result.graph_id, &[]);
        Ok(result)
    }
}

pub fn route<B: ConsoleBackend>(
    backend: &B,
    token: &str,
    authorization: Option<&str>,
    method: &str,
    path: &str,
    body: &[u8],
) -> ApiResponse {
    let expected = format!("Bearer {token}");
    if authorization != Some(expected.as_str()) {
        return json_error(401, "令牌无效");
    }
    let result = if method == "POST" && path == "/v1/dispatch" {
        serde_json::from_slice::<ConsoleDispatchRequest>(body)
            .map_err(|error| ApiError::new(400, format!("请求 JSON 无效：{error}")))
            .and_then(|request| backend.dispatch(request))
            .and_then(json_value)
    } else if method == "GET" {
        run_path(path, "", |run_id, _| backend.status(run_id)).and_then(json_value)
    } else if method == "POST" && path.ends_with("/adopt") {
        #[derive(Deserialize)]
        struct AdoptBody { node_id: String }
        serde_json::from_slice::<AdoptBody>(body)
            .map_err(|error| ApiError::new(400, format!("请求 JSON 无效：{error}")))
            .and_then(|adopt| run_path(path, "/adopt", |run_id, _| backend.adopt(run_id, &adopt.node_id)))
            .and_then(json_value)
    } else if method == "POST" && path.ends_with("/cancel") {
        run_path(path, "/cancel", |run_id, _| backend.cancel(run_id)).and_then(json_value)
    } else {
        Err(ApiError::new(404, "找不到接口"))
    };
    match result {
        Ok(body) => ApiResponse { status: 200, body },
        Err(error) => json_error(error.status, &error.reason),
    }
}

fn run_path<T>(
    path: &str,
    suffix: &str,
    f: impl FnOnce(&str, &str) -> Result<T, ApiError>,
) -> Result<T, ApiError> {
    let prefix = "/v1/runs/";
    let Some(rest) = path.strip_prefix(prefix) else {
        return Err(ApiError::new(404, "找不到接口"));
    };
    let run_id = if suffix.is_empty() {
        rest
    } else {
        rest.strip_suffix(suffix).ok_or_else(|| ApiError::new(404, "找不到接口"))?
    };
    if run_id.is_empty() || run_id.contains('/') {
        return Err(ApiError::new(404, "找不到接口"));
    }
    f(run_id, suffix)
}

fn json_value<T: Serialize>(value: T) -> Result<String, ApiError> {
    serde_json::to_string(&value).map_err(|error| ApiError::new(500, error.to_string()))
}

fn json_error(status: u16, reason: &str) -> ApiResponse {
    ApiResponse {
        status,
        body: serde_json::json!({ "error": reason }).to_string(),
    }
}

fn runs() -> &'static Mutex<HashMap<String, ConsoleRun>> {
    static RUNS: OnceLock<Mutex<HashMap<String, ConsoleRun>>> = OnceLock::new();
    RUNS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[tauri::command]
pub fn list_console_runs() -> Vec<ConsoleRun> {
    runs().lock().map(|runs| sorted_runs(&runs)).unwrap_or_default()
}

fn sorted_runs(runs: &HashMap<String, ConsoleRun>) -> Vec<ConsoleRun> {
    let mut out: Vec<_> = runs.values().cloned().collect();
    out.sort_by(|a, b| b.started_at.cmp(&a.started_at).then_with(|| a.run_id.cmp(&b.run_id)));
    out
}

fn run_by_id(run_id: &str) -> Result<ConsoleRun, ApiError> {
    runs()
        .lock()
        .map_err(|_| ApiError::new(409, "运行状态不可用"))?
        .get(run_id)
        .cloned()
        .ok_or_else(|| ApiError::new(404, format!("找不到 run：{run_id}")))
}

fn emit_runs(app: &AppHandle, kind: &str, graph_id: &str, node_ids: &[String]) {
    let event = ConsoleEvent {
        kind: kind.into(),
        graph_id: graph_id.into(),
        node_ids: node_ids.to_vec(),
        runs: list_console_runs(),
    };
    let _ = app.emit("console-runs-changed", event);
}

#[derive(Debug)]
struct PreparedDispatch {
    response: ConsoleDispatchResponse,
    graph: Graph,
    stages: Vec<Vec<String>>,
    project: String,
    original_request: String,
    materials: Vec<String>,
}

fn start_dispatch(app: AppHandle, request: ConsoleDispatchRequest) -> Result<ConsoleDispatchResponse, ApiError> {
    let root = crate::paths::aicross_root().map_err(|reason| ApiError::new(409, reason))?;
    let prepared = prepare_dispatch_at(&root, request)?;
    orchestration::save_graph(prepared.graph.clone()).map_err(|reason| ApiError::new(409, reason))?;
    let nodes = prepared.stages.iter().flatten().filter_map(|id| {
        let node = prepared.graph.nodes.get(id)?;
        let target = node.target.as_ref()?;
        Some(ConsoleRunNode {
            node_id: id.clone(),
            role: target.role.clone(),
            agent: target.agent.clone(),
            model: target.model.clone(),
            status: "running".into(),
            text_excerpt: String::new(),
            artifacts_dir: String::new(),
        })
    }).collect();
    let run = ConsoleRun {
        run_id: prepared.response.run_id.clone(),
        graph_id: prepared.response.graph_id.clone(),
        project: prepared.project.clone(),
        status: "running".into(),
        nodes,
        leftover: Vec::new(),
        started_at: Utc::now().to_rfc3339(),
    };
    runs().lock().map_err(|_| ApiError::new(409, "运行状态不可用"))?
        .insert(run.run_id.clone(), run);
    emit_runs(&app, "dispatched", &prepared.response.graph_id, &prepared.response.node_ids);
    let response = prepared.response.clone();
    tauri::async_runtime::spawn(async move {
        run_prepared(app, prepared).await;
    });
    Ok(response)
}

fn prepare_dispatch_at(root: &Path, request: ConsoleDispatchRequest) -> Result<PreparedDispatch, ApiError> {
    let project_path = PathBuf::from(request.project.trim());
    if !project_path.is_dir() {
        return Err(ApiError::new(404, "项目目录不存在"));
    }
    if request.text.trim().is_empty() {
        return Err(ApiError::new(409, "派发文本不能为空"));
    }
    let project = fs::canonicalize(&project_path)
        .map_err(|error| ApiError::new(404, format!("项目目录不可用：{error}")))?
        .to_string_lossy().into_owned();
    let original_request = request.text.clone();
    let materials = request.attachments.clone().unwrap_or_default();
    let mut graph = latest_project_graph(root, &project).unwrap_or_else(|| new_console_graph(&project));
    graph.project.clone_from(&project);
    graph.source = "console".into();
    let now = Utc::now().to_rfc3339();
    let request_node = if graph.nodes.get(&graph.root).is_some_and(|node| {
        node.user.text.is_empty() && node.user.attachments.is_empty() && node.parents.is_empty()
    }) {
        let root_id = graph.root.clone();
        let node = graph.nodes.get_mut(&root_id).expect("root checked");
        node.user = Message { text: request.text.clone(), attachments: request.attachments.clone().unwrap_or_default() };
        node.source = "console".into();
        root_id
    } else {
        let id = Uuid::new_v4().to_string();
        let parent = latest_leaf(&graph);
        graph.nodes.insert(id.clone(), GraphNode {
            id: id.clone(), kind: "material".into(), parents: parent.into_iter().collect(),
            user: Message { text: request.text.clone(), attachments: request.attachments.clone().unwrap_or_default() },
            created_at: now.clone(), source: "console".into(), ..GraphNode::default()
        });
        id
    };
    if graph.title.is_empty() {
        graph.title = request.text.lines().next().unwrap_or("对话图").chars().take(20).collect();
    }

    let stages_targets = if let Some(targets) = request.targets.filter(|items| !items.is_empty()) {
        vec![targets]
    } else {
        let preset_id = request.preset.as_deref().unwrap_or("builtin-economy");
        let preset = orchestration::presets_at(root).presets.into_iter()
            .find(|preset| preset.id == preset_id)
            .ok_or_else(|| ApiError::new(409, format!("找不到预设：{preset_id}")))?;
        preset.stages.into_iter().map(|stage| stage.nodes).collect()
    };
    if stages_targets.is_empty() || stages_targets.iter().any(Vec::is_empty) {
        return Err(ApiError::new(409, "预设没有可派发节点"));
    }
    let missing = stages_targets.iter().flatten().find(|target| target.agent.trim().is_empty() || target.model.trim().is_empty());
    if let Some(target) = missing {
        return Err(ApiError::new(409, format!("角色「{}」没有可派发目标", target.role)));
    }
    validate_targets(root, &stages_targets)?;

    let mut previous = vec![request_node];
    let mut stages = Vec::new();
    let mut all_ids = Vec::new();
    for targets in stages_targets {
        let mut ids = Vec::new();
        for (index, target) in targets.into_iter().enumerate() {
            let id = Uuid::new_v4().to_string();
            graph.nodes.insert(id.clone(), GraphNode {
                id: id.clone(),
                kind: if previous.len() > 1 { "accept".into() } else { "turn".into() },
                parents: previous.clone(),
                user: Message::default(),
                reply: Some(Reply { status: "draft".into(), ..Reply::default() }),
                target: Some(target),
                created_at: (Utc::now() + chrono::Duration::milliseconds(index as i64)).to_rfc3339(),
                source: "console".into(),
                ..GraphNode::default()
            });
            ids.push(id.clone());
            all_ids.push(id);
        }
        previous = ids.clone();
        stages.push(ids);
    }
    graph.updated_at = now;
    let run_id = Uuid::new_v4().to_string();
    Ok(PreparedDispatch {
        response: ConsoleDispatchResponse { run_id, graph_id: graph.id.clone(), node_ids: all_ids },
        graph,
        stages,
        project,
        original_request,
        materials,
    })
}

fn validate_targets(root: &Path, stages: &[Vec<Target>]) -> Result<(), ApiError> {
    let targets: Vec<&Target> = stages.iter().flatten().collect();
    if targets.len() > DispatchSettings::default().max_requests {
        return Err(ApiError::new(409, format!("本次派发 {} 路，超过总请求上限", targets.len())));
    }
    let resources = orchestration::dispatchable_at(root);
    if let Some(error) = resources.error {
        return Err(ApiError::new(409, format!("读取资源失败：{error}")));
    }
    for target in targets {
        let readonly_role = matches!(target.role.as_str(), "验收" | "验证" | "仲裁" | "拆分");
        if (readonly_role || target.visibility == "盲") && target.write != "只读" {
            return Err(ApiError::new(409, format!("角色「{}」只能只读派发", target.role)));
        }
        let model = resources.models.iter().find(|model| {
            model.agent == target.agent && model.id == target.model
                && (target.provider.as_deref().unwrap_or("").is_empty()
                    || model.provider == target.provider.as_deref().unwrap_or(""))
        }).ok_or_else(|| ApiError::new(409, format!("未接入目标：{}/{}", target.agent, target.model)))?;
        if model.verification == "failed" {
            return Err(ApiError::new(409, format!("目标验证失败：{}/{}", target.agent, target.model)));
        }
        if model.quota_exhausted_until.as_deref()
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .is_some_and(|until| until > Utc::now())
        {
            return Err(ApiError::new(409, format!("目标额度用尽：{}/{}", target.agent, target.model)));
        }
    }
    Ok(())
}

fn new_console_graph(project: &str) -> Graph {
    let now = Utc::now().to_rfc3339();
    let root_id = Uuid::new_v4().to_string();
    let mut nodes = HashMap::new();
    nodes.insert(root_id.clone(), GraphNode {
        id: root_id.clone(), kind: "material".into(), created_at: now.clone(), source: "console".into(),
        ..GraphNode::default()
    });
    Graph {
        version: 1, id: Uuid::new_v4().to_string(), title: String::new(), root: root_id,
        nodes, created_at: now.clone(), updated_at: now, project: project.into(), source: "console".into(),
    }
}

fn latest_project_graph(root: &Path, project: &str) -> Option<Graph> {
    orchestration::list_graphs_at(root).into_iter()
        .filter_map(|meta| orchestration::load_graph_at(root, &meta.id).ok().flatten())
        .filter(|graph| graph.project.eq_ignore_ascii_case(project))
        .max_by(|a, b| a.updated_at.cmp(&b.updated_at))
}

fn latest_leaf(graph: &Graph) -> Option<String> {
    let parents: HashSet<&str> = graph.nodes.values().flat_map(|node| node.parents.iter().map(String::as_str)).collect();
    graph.nodes.values().filter(|node| !parents.contains(node.id.as_str()))
        .max_by(|a, b| a.created_at.cmp(&b.created_at).then_with(|| a.id.cmp(&b.id)))
        .map(|node| node.id.clone())
}

async fn run_prepared(app: AppHandle, prepared: PreparedDispatch) {
    let mut graph = prepared.graph;
    for (stage_index, ids) in prepared.stages.iter().enumerate() {
        if run_by_id(&prepared.response.run_id).is_ok_and(|run| run.status == "failed") { break; }
        for id in ids {
            if let Some(reply) = graph.nodes.get_mut(id).and_then(|node| node.reply.as_mut()) {
                reply.status = "pending".into();
            }
        }
        if orchestration::save_graph(graph.clone()).is_err() { break; }
        let request = DispatchStageRequest {
            run_id: prepared.response.run_id.clone(),
            stage: (stage_index + 1) as u32,
            project_dir: prepared.project.clone(),
            nodes: ids.iter().filter_map(|id| build_node_request(
                &graph,
                id,
                &prepared.original_request,
                &prepared.materials,
                (stage_index + 1) as u32,
            )).collect(),
            settings: DispatchSettings::default(),
        };
        let output = dispatch::dispatch_stage(request, Some(app.clone())).await;
        let results = match output {
            Ok(output) => output.results,
            Err(error) => ids.iter().map(|id| DispatchNodeResult {
                node_id: id.clone(), stage: (stage_index + 1) as u32, status: "error".into(), error: error.clone(),
                ..DispatchNodeResult::default()
            }).collect(),
        };
        apply_results(&mut graph, &results);
        let _ = orchestration::save_graph(graph.clone());
        update_run_results(&prepared.response.run_id, &results);
        emit_runs(&app, "progress", &prepared.response.graph_id, ids);
        if results.iter().any(|result| result.status != "ok") { break; }
    }
    if let Ok(mut runs) = runs().lock() {
        if let Some(run) = runs.get_mut(&prepared.response.run_id) {
            if run.status == "running" {
                run.status = if run.nodes.iter().all(|node| node.status == "done") { "done".into() } else { "failed".into() };
            }
        }
    }
    emit_runs(&app, "finished", &prepared.response.graph_id, &prepared.response.node_ids);
}

fn build_node_request(
    graph: &Graph,
    node_id: &str,
    original: &str,
    materials: &[String],
    stage: u32,
) -> Option<DispatchNodeRequest> {
    let node = graph.nodes.get(node_id)?;
    let target = node.target.as_ref()?;
    let resource = orchestration::list_dispatchable_models().models.into_iter().find(|model| {
        model.agent == target.agent && model.id == target.model
            && (target.provider.as_deref().unwrap_or("").is_empty()
                || model.provider == target.provider.as_deref().unwrap_or(""))
    });
    let mut prompt = format!("[用户]\n{original}");
    let mut upstream = Vec::new();
    for parent_id in &node.parents {
        if let Some(parent) = graph.nodes.get(parent_id) {
            if target.visibility != "盲" {
                if let Some(reply) = parent.reply.as_ref().filter(|reply| reply.status == "done" && !reply.text.is_empty()) {
                    prompt.push_str("\n\n[模型回复]\n");
                    prompt.push_str(&reply.text);
                }
            }
            if let Some(artifacts) = parent.reply.as_ref().and_then(|reply| reply.artifacts.as_ref()) {
                upstream.push(DispatchArtifacts {
                    diff: artifacts.diff.clone(),
                    changed_files: artifacts.changed_files.iter().map(|file| dispatch::ChangedFile { path: file.path.clone(), content: file.content.clone() }).collect(),
                    worktree: artifacts.worktree.clone(), stdout_summary: artifacts.stdout_summary.clone(), before_dir: String::new(),
                });
            }
        }
    }
    Some(DispatchNodeRequest {
        node_id: node_id.into(), stage, agent: target.agent.clone(), provider: target.provider.clone().unwrap_or_default(),
        model: target.model.clone(), vendor: resource.as_ref().map(|model| model.vendor.clone()).unwrap_or_default(),
        billing_source: resource.as_ref().map(|model| model.billing_source.clone()).unwrap_or_default(), role: target.role.clone(),
        tier: target.tier.clone().unwrap_or_default(), thinking: target.thinking.clone(), visibility: target.visibility.clone(),
        prompt, current_message: node.user.text.clone(), materials: materials.to_vec(), session_id: String::new(),
        previous_runner: String::new(), write: target.write.clone(), original_request: original.into(), upstream_artifacts: upstream,
        fallback: target.fallback.as_ref().map(|fallback| dispatch::FallbackSpec { agent: fallback.agent.clone(), provider: fallback.provider.clone().unwrap_or_default(), model: fallback.model.clone(), vendor: String::new() }),
        rollback_before: false, review_material_validation: target.role == "验证",
    })
}

fn apply_results(graph: &mut Graph, results: &[DispatchNodeResult]) {
    for result in results {
        let Some(node) = graph.nodes.get_mut(&result.node_id) else { continue };
        let old_adopted = node.reply.as_ref().and_then(|reply| reply.adopted);
        node.reply = Some(Reply {
            status: if result.status == "ok" { "done".into() } else if result.status == "quota_exhausted" { "quota_exhausted".into() } else { "failed".into() },
            text: if result.text.is_empty() { result.error.clone() } else { result.text.clone() },
            model_reported: (!result.model_reported.is_empty()).then(|| result.model_reported.clone()),
            tokens_in: (result.tokens_in != 0).then_some(result.tokens_in), tokens_out: (result.tokens_out != 0).then_some(result.tokens_out),
            tokens_cache_read: (result.cache_read != 0).then_some(result.cache_read), ms: Some(result.duration_ms),
            session_id: (!result.session_id.is_empty()).then(|| result.session_id.clone()),
            runner: (!result.runner.is_empty()).then(|| result.runner.clone()), adopted: old_adopted,
            artifacts: result.artifacts.as_ref().map(reply_artifacts), error_kind: (result.status != "ok").then(|| result.status.clone()),
            fallback_from: (!result.fallback_from.is_empty()).then(|| result.fallback_from.clone()),
            fallback_to: (!result.fallback_to.is_empty()).then(|| result.fallback_to.clone()), quota_resets_at: result.quota_resets_at.clone(),
            ..Reply::default()
        });
    }
    graph.updated_at = Utc::now().to_rfc3339();
}

fn reply_artifacts(value: &DispatchArtifacts) -> ReplyArtifacts {
    ReplyArtifacts {
        diff: value.diff.clone(),
        changed_files: value.changed_files.iter().map(|file| ReplyChangedFile { path: file.path.clone(), content: file.content.clone() }).collect(),
        worktree: value.worktree.clone(), stdout_summary: value.stdout_summary.clone(),
    }
}

fn update_run_results(run_id: &str, results: &[DispatchNodeResult]) {
    let Ok(mut runs) = runs().lock() else { return };
    let Some(run) = runs.get_mut(run_id) else { return };
    for result in results {
        let Some(node) = run.nodes.iter_mut().find(|node| node.node_id == result.node_id) else { continue };
        node.status = if result.status == "ok" { "done".into() } else { "failed".into() };
        node.text_excerpt = (if result.text.is_empty() { &result.error } else { &result.text }).chars().take(160).collect();
        node.artifacts_dir = if result.workdir.is_empty() {
            result.artifacts.as_ref().and_then(|artifacts| artifacts.worktree.clone()).unwrap_or_default()
        } else {
            result.workdir.clone()
        };
    }
}

pub fn token_hex() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

pub fn write_manifest(port: u16, token: &str) -> Result<PathBuf, String> {
    let root = crate::paths::aicross_root()?;
    write_manifest_at(&root, port, token)
}

fn write_manifest_at(root: &Path, port: u16, token: &str) -> Result<PathBuf, String> {
    write_manifest_at_with(root, port, token, set_private_permissions)
}

fn write_manifest_at_with(
    root: &Path,
    port: u16,
    token: &str,
    harden: fn(&Path) -> Result<(), String>,
) -> Result<PathBuf, String> {
    fs::create_dir_all(root).map_err(|error| format!("创建控制台目录失败：{error}"))?;
    let path = root.join("console.json");
    let temp = root.join(format!(".console-{}.tmp", Uuid::new_v4()));
    let manifest = ConsoleManifest {
        version: CONSOLE_VERSION, endpoint: format!("http://127.0.0.1:{port}"), token: token.into(),
        pid: std::process::id(), started_at: Utc::now().to_rfc3339(),
    };
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    let result = (|| -> Result<(), String> {
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temp)
            .map_err(|error| format!("创建控制台清单失败：{error}"))?;
        #[cfg(unix)]
        harden(&temp)?;
        file.write_all(&bytes).map_err(|error| format!("写入控制台清单失败：{error}"))?;
        file.sync_all().map_err(|error| format!("同步控制台清单失败：{error}"))?;
        drop(file);
        #[cfg(windows)]
        harden(&temp)?;
        if path.exists() { fs::remove_file(&path).map_err(|error| format!("替换控制台清单失败：{error}"))?; }
        fs::rename(&temp, &path).map_err(|error| format!("发布控制台清单失败：{error}"))?;
        Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(&temp); }
    result?;
    Ok(path)
}

pub fn remove_manifest() {
    if let Ok(root) = crate::paths::aicross_root() { let _ = remove_manifest_at(&root); }
}

fn remove_manifest_at(root: &Path) -> Result<(), String> {
    match fs::remove_file(root.join("console.json")) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("删除控制台清单失败：{error}")),
    }
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("设置控制台清单权限失败：{error}"))
}

#[cfg(windows)]
fn set_private_permissions(path: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let who = std::process::Command::new(r"C:\Windows\System32\whoami.exe")
        .args(["/user", "/fo", "csv", "/nh"]).creation_flags(0x08000000).output()
        .map_err(|error| format!("读取当前用户失败：{error}"))?;
    if !who.status.success() { return Err("读取当前用户失败".into()); }
    let line = String::from_utf8_lossy(&who.stdout);
    let sid = line.trim().trim_matches('"').rsplit("\",\"").next().unwrap_or("").trim_matches('"');
    if !sid.starts_with("S-") { return Err("读取当前用户 SID 失败".into()); }
    let args = windows_acl_args(sid);
    let status = std::process::Command::new(r"C:\Windows\System32\icacls.exe").arg(path).args(&args)
        .creation_flags(0x08000000).status().map_err(|error| format!("设置控制台清单权限失败：{error}"))?;
    if status.success() { Ok(()) } else { Err("设置控制台清单权限失败".into()) }
}

#[cfg(windows)]
fn windows_acl_args(sid: &str) -> [String; 3] {
    ["/grant:r".into(), format!("*{sid}:F"), "/inheritance:r".into()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeBackend { run: Mutex<Option<ConsoleRun>> }

    impl ConsoleBackend for FakeBackend {
        fn dispatch(&self, _request: ConsoleDispatchRequest) -> Result<ConsoleDispatchResponse, ApiError> {
            let run = ConsoleRun {
                run_id: "run-1".into(), graph_id: "graph-1".into(), project: "p".into(), status: "done".into(),
                nodes: vec![ConsoleRunNode { node_id: "node-1".into(), role: "验证".into(), agent: "pi".into(), model: "m".into(), status: "done".into(), text_excerpt: "ok".into(), artifacts_dir: String::new() }],
                leftover: Vec::new(), started_at: "2026-09-15T00:00:00Z".into(),
            };
            *self.run.lock().unwrap() = Some(run);
            Ok(ConsoleDispatchResponse { run_id: "run-1".into(), graph_id: "graph-1".into(), node_ids: vec!["node-1".into()] })
        }
        fn status(&self, run_id: &str) -> Result<ConsoleRun, ApiError> {
            self.run.lock().unwrap().clone().filter(|run| run.run_id == run_id).ok_or_else(|| ApiError::new(404, "missing"))
        }
        fn adopt(&self, run_id: &str, node_id: &str) -> Result<ConsoleRun, ApiError> {
            let mut run = self.status(run_id)?;
            if run.nodes[0].node_id != node_id { return Err(ApiError::new(404, "missing")); }
            run.nodes[0].text_excerpt = "adopted".into();
            *self.run.lock().unwrap() = Some(run.clone());
            Ok(run)
        }
        fn cancel(&self, run_id: &str) -> Result<ConsoleRun, ApiError> { self.status(run_id) }
    }

    #[test]
    fn wrong_token_is_401_before_routing() {
        let response = route(&FakeBackend::default(), "secret", Some("Bearer wrong"), "GET", "/v1/runs/nope", b"");
        assert_eq!(response.status, 401);
    }

    #[test]
    fn missing_project_is_404() {
        let root = std::env::temp_dir().join(format!("aicross-console-missing-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let error = prepare_dispatch_at(&root, ConsoleDispatchRequest { project: root.join("missing").to_string_lossy().into(), text: "x".into(), ..Default::default() }).unwrap_err();
        assert_eq!(error.status, 404);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unavailable_route_is_409_with_reason() {
        let root = std::env::temp_dir().join(format!("aicross-console-unavailable-{}", Uuid::new_v4()));
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        let error = prepare_dispatch_at(&root, ConsoleDispatchRequest { project: project.to_string_lossy().into(), text: "x".into(), preset: Some("builtin-economy".into()), ..Default::default() }).unwrap_err();
        assert_eq!(error.status, 409);
        assert!(error.reason.contains("没有可派发目标"), "{}", error.reason);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prepare_dispatch_marks_new_graph_and_nodes_as_console_source() {
        let root = std::env::temp_dir().join(format!("aicross-console-prepare-{}", Uuid::new_v4()));
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(root.join("resources.json"), serde_json::json!({
            "version": 1,
            "agents": [{
                "id": "codex", "name": "Codex", "kind": "cli", "detected": true, "version": "1.0",
                "models": [{"id": "gpt-test", "agent": "codex", "status": "已接入"}]
            }]
        }).to_string()).unwrap();
        let prepared = prepare_dispatch_at(&root, ConsoleDispatchRequest {
            project: project.to_string_lossy().into(), text: "第一行\n第二行".into(),
            targets: Some(vec![Target {
                agent: "codex".into(), model: "gpt-test".into(), role: "验证".into(),
                visibility: "盲".into(), write: "只读".into(), ..Target::default()
            }]),
            ..Default::default()
        }).unwrap();
        assert_eq!(prepared.graph.source, "console");
        assert_eq!(prepared.graph.project, fs::canonicalize(&project).unwrap().to_string_lossy());
        assert!(prepared.response.node_ids.iter().all(|id| prepared.graph.nodes[id].source == "console"));
        assert_eq!(prepared.graph.nodes[&prepared.graph.root].user.text, "第一行\n第二行");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fake_channel_dispatch_status_adopt_round_trip() {
        let backend = FakeBackend::default();
        let dispatched = route(&backend, "secret", Some("Bearer secret"), "POST", "/v1/dispatch", br#"{"project":"p","text":"check"}"#);
        assert_eq!(dispatched.status, 200);
        assert!(dispatched.body.contains("run-1"));
        let status = route(&backend, "secret", Some("Bearer secret"), "GET", "/v1/runs/run-1", b"");
        assert_eq!(status.status, 200);
        assert!(status.body.contains("node-1"));
        let adopted = route(&backend, "secret", Some("Bearer secret"), "POST", "/v1/runs/run-1/adopt", br#"{"node_id":"node-1"}"#);
        assert_eq!(adopted.status, 200);
        assert!(adopted.body.contains("adopted"));
        let cancelled = route(&backend, "secret", Some("Bearer secret"), "POST", "/v1/runs/run-1/cancel", b"{}");
        assert_eq!(cancelled.status, 200);
    }

    #[test]
    fn manifest_write_delete_and_private_permissions() {
        let root = std::env::temp_dir().join(format!("aicross-console-manifest-{}", Uuid::new_v4()));
        #[cfg(unix)]
        let path = write_manifest_at(&root, 43123, &"a".repeat(64)).unwrap();
        #[cfg(windows)]
        let path = write_manifest_at_with(&root, 43123, &"a".repeat(64), |_| Ok(())).unwrap();
        let manifest: ConsoleManifest = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(manifest.endpoint, "http://127.0.0.1:43123");
        assert_eq!(manifest.token.len(), 64);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
        #[cfg(windows)]
        {
            let args = windows_acl_args("S-1-5-21-1000");
            assert_eq!(args, ["/grant:r", "*S-1-5-21-1000:F", "/inheritance:r"]);
        }
        remove_manifest_at(&root).unwrap();
        assert!(!path.exists());
        let _ = fs::remove_dir_all(root);
    }
}
