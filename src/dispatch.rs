use crate::channels::{self, ChannelOutput};
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
use std::sync::{mpsc, OnceLock};
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
}

impl Default for DispatchSettings {
    fn default() -> Self {
        Self {
            timeout_secs: default_timeout(),
            concurrency: default_concurrency(),
            redact: true,
            write_project_dispatch: true,
            cc_switch_path: String::new(),
        }
    }
}

fn default_timeout() -> u64 {
    600
}
fn default_concurrency() -> usize {
    3
}
fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchNodeRequest {
    pub node_id: String,
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
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchStageResult {
    pub run_id: String,
    pub results: Vec<DispatchNodeResult>,
    pub state_file: String,
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
    cancel_registered(&registry_key(&run_id, &node_id))
}

fn preview_core(request: &DispatchStageRequest) -> Result<DispatchPreview, String> {
    let mut preview = DispatchPreview::default();
    for node in &request.nodes {
        let prompt = if !node.session_id.is_empty()
            && node.visibility == "共享"
            && !node.current_message.is_empty()
        {
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
    write_state(
        &state_file,
        &request.run_id,
        request.stage,
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
    let mut results = Vec::new();
    let (sender, receiver) = mpsc::channel::<(String, DispatchNodeResult)>();

    while !queued.is_empty() || active > 0 {
        while active < limit {
            let next_index = queued
                .iter()
                .position(|node| !active_agents.contains(channels::normalize_agent(&node.agent)));
            let Some(index) = next_index else { break };
            let node = queued.remove(index).expect("queued index");
            let agent = channels::normalize_agent(&node.agent).to_string();
            active_agents.insert(agent.clone());
            active += 1;
            status.insert(node.node_id.clone(), "运行中".into());
            emit_progress(&app, &request.run_id, &node.node_id, "运行中");
            write_state(
                &state_file,
                &request.run_id,
                request.stage,
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
            let stage = request.stage;
            thread::spawn(move || {
                let result = run_one(
                    &run_id, stage, &project, &run_root, &log_dir, &settings, node,
                );
                let _ = tx.send((agent, result));
            });
        }
        if active == 0 {
            return Err("同一 agent 并发调度未能推进".into());
        }
        let (agent, result) = receiver
            .recv()
            .map_err(|error| format!("接收派发结果失败：{error}"))?;
        active -= 1;
        active_agents.remove(&agent);
        let ui_status = match result.status.as_str() {
            "ok" => "完成",
            "timeout" => "超时",
            _ => "失败",
        };
        status.insert(result.node_id.clone(), ui_status.into());
        emit_progress(&app, &request.run_id, &result.node_id, ui_status);
        results.push(result);
        write_state(
            &state_file,
            &request.run_id,
            request.stage,
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

fn run_one(
    run_id: &str,
    stage: u32,
    project: &Path,
    run_root: &Path,
    log_dir: &Path,
    settings: &DispatchSettings,
    node: DispatchNodeRequest,
) -> DispatchNodeResult {
    let started = Local::now();
    let timer = Instant::now();
    let blind = node.visibility == "盲";
    let isolation = channels::isolation_for(&node.agent, blind).to_string();
    let workdir = if blind {
        run_root.join(&node.node_id)
    } else {
        project.to_path_buf()
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
    let continuing = !node.session_id.is_empty() && node.visibility == "共享";
    let (outbound, redacted) = if continuing {
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
    let mut error = preparation_error;

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
        ) {
            Ok(invocation) => match run_process_registered(
                &invocation.program,
                &invocation.args,
                invocation.input.as_deref(),
                Duration::from_secs(settings.timeout_secs.clamp(1, 3600)),
                Some(&workdir),
                Some(&registry_key(run_id, &node.node_id)),
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
                                if parse_error.message.starts_with("真身不符") {
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
    let dispatch_file = write_record(
        log_dir,
        &record,
        &frontmatter,
        &outbound,
        &raw_stdout,
        &raw_stderr,
    )
    .unwrap_or_default();

    DispatchNodeResult {
        node_id: node.node_id,
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
    }
}

fn prepare_outbound(
    node: &DispatchNodeRequest,
    redact: bool,
    workdir: &Path,
    blind: bool,
) -> Result<(String, Vec<String>, usize), String> {
    let (prompt, mut count) = redact_text(&node.prompt, redact);
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
        let mut used = HashSet::new();
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
            } else {
                fs::write(&target, bytes)
                    .map_err(|error| format!("复制附件失败 {}：{error}", target.display()))?;
            }
            names.push(name.to_string());
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
    redact_text(text, true).0
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
    let path = if value.trim().is_empty() {
        std::env::current_dir().map_err(|error| format!("读取当前目录失败：{error}"))?
    } else {
        PathBuf::from(value)
    };
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
        safe_name(&record.agent),
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
    let mut body = format!(
        "## 当前状态\n阶段: 派发第 {stage} 阶段 ｜ run: {run_id} ｜ 更新: {}\n## 已完成\n",
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
        .filter(|(_, state)| *state == "运行中" || *state == "排队")
    {
        body.push_str(&format!("- [ ] {node}: {state}\n"));
    }
    body.push_str("## 失败与超时\n");
    for (node, state) in entries
        .iter()
        .filter(|(_, state)| *state == "失败" || *state == "超时")
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
        .any(|state| state == "运行中" || state == "排队")
    {
        body.push_str("- 等待本阶段各路完成\n");
    } else {
        body.push_str("- 检查失败原因后重派或更换通道\n");
    }
    write_atomic(path, body.as_bytes())
}

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
        }
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
            1,
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
        let status = HashMap::from([("a".into(), "运行中".into()), ("b".into(), "失败".into())]);
        let failed = DispatchNodeResult {
            node_id: "b".into(),
            dispatch_file: "b.md".into(),
            ..DispatchNodeResult::default()
        };
        write_state(&path, "run", 2, 2, &status, &[failed]).unwrap();
        let state = fs::read_to_string(&path).unwrap();
        assert!(state.contains("同阶段并发上限 2"));
        assert!(state.contains("## 进行中\n- [ ] a: 运行中"));
        assert!(state.contains("## 失败与超时\n- [ ] b: 失败 → b.md"));
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
        }
    }

    fn live_settings(concurrency: usize) -> DispatchSettings {
        DispatchSettings {
            timeout_secs: 600,
            concurrency,
            redact: true,
            write_project_dispatch: true,
            cc_switch_path: String::new(),
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
}
