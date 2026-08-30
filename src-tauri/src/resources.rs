use crate::models::{
    AddManualAgentRequest, AddResourceModelRequest, CcSwitchProvider, ConnectionOverview,
    ResourceAgent, ResourceModel, ResourceModelKey, ResourceOperation, ResourceStore, Settings,
    SmokeSelectionRequest, ToolDefinition, UpdateResourceModelRequest,
};
use crate::tools;
use chrono::{Local, SecondsFormat};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

const STATUS_PENDING: &str = "待冒烟";
const STATUS_CONNECTED: &str = "已接入";
const STATUS_FAILED: &str = "冒烟失败";
const MISSING_MARKER: &str = "未在事实源中";
const SMOKE_PROMPT: &str = "只回复 OK";
const MAX_PROCESS_OUTPUT: u64 = 1024 * 1024;

#[derive(Debug)]
struct ProcessResult {
    code: Option<i32>,
    stdout: String,
    stderr: String,
    timed_out: bool,
}

#[derive(Debug, Deserialize)]
struct CodexCache {
    #[serde(default)]
    models: Vec<CodexModel>,
}

#[derive(Debug, Deserialize)]
struct CodexModel {
    slug: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    priority: Option<i64>,
    #[serde(default)]
    supported_reasoning_levels: Vec<Value>,
}

#[derive(Debug, Deserialize)]
struct PiStoreModel {
    id: String,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    reasoning: bool,
    #[serde(default, rename = "contextWindow")]
    context_window: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct PiStoreProvider {
    #[serde(default)]
    models: Vec<PiStoreModel>,
}

#[derive(Debug, Deserialize)]
struct RawCcProvider {
    name: String,
    #[serde(default)]
    app_type: String,
    #[serde(default)]
    tier_models: HashMap<String, String>,
    #[serde(default)]
    has_token: bool,
}

pub fn ensure_agents(store: &mut ResourceStore, definitions: &[ToolDefinition]) {
    store.version = 1;
    for definition in definitions
        .iter()
        .filter(|definition| definition.id != "powershell")
    {
        if store.agents.iter().all(|agent| agent.id != definition.id) {
            let kind = if matches!(definition.id.as_str(), "claude" | "codex" | "kimi" | "pi") {
                "内置"
            } else {
                "手动"
            };
            store.agents.push(ResourceAgent {
                id: definition.id.clone(),
                kind: kind.to_string(),
                detected: false,
                version: "未检测".to_string(),
                models: Vec::new(),
            });
        }
    }
    if store.agents.iter().all(|agent| agent.id != "cc-switch") {
        store.agents.push(ResourceAgent {
            id: "cc-switch".to_string(),
            kind: "provider 桥".to_string(),
            detected: false,
            version: "未检测".to_string(),
            models: Vec::new(),
        });
    }
    for agent in &mut store.agents {
        for model in &mut agent.models {
            model.agent.clone_from(&agent.id);
        }
    }
}

pub fn detect_one(
    mut store: ResourceStore,
    agent_id: &str,
    definitions: &[ToolDefinition],
    settings: &Settings,
) -> Result<ResourceOperation, String> {
    ensure_agents(&mut store, definitions);
    if store.agents.iter().all(|agent| agent.id != agent_id) {
        return Err(format!("探测资源失败：未知 agent：{agent_id}"));
    }
    let mut errors = Vec::new();
    let mut providers = Vec::new();
    detect_into(
        &mut store,
        agent_id,
        definitions,
        settings,
        &mut errors,
        &mut providers,
    );
    Ok(ResourceOperation {
        store,
        errors,
        cc_switch_providers: providers,
    })
}

pub fn detect_many(
    mut store: ResourceStore,
    agent_ids: &[String],
    definitions: &[ToolDefinition],
    settings: &Settings,
) -> ResourceOperation {
    ensure_agents(&mut store, definitions);
    let ids = if agent_ids.is_empty() {
        store
            .agents
            .iter()
            .map(|agent| agent.id.clone())
            .collect::<Vec<_>>()
    } else {
        agent_ids.to_vec()
    };
    let mut errors = Vec::new();
    let mut providers = Vec::new();
    for agent_id in ids {
        if store.agents.iter().all(|agent| agent.id != agent_id) {
            errors.push(format!("探测资源失败：未知 agent：{agent_id}"));
            continue;
        }
        detect_into(
            &mut store,
            &agent_id,
            definitions,
            settings,
            &mut errors,
            &mut providers,
        );
    }
    ResourceOperation {
        store,
        errors,
        cc_switch_providers: providers,
    }
}

fn detect_into(
    store: &mut ResourceStore,
    agent_id: &str,
    definitions: &[ToolDefinition],
    settings: &Settings,
    errors: &mut Vec<String>,
    providers: &mut Vec<CcSwitchProvider>,
) {
    if agent_id == "cc-switch" {
        match detect_cc_switch(settings) {
            Ok((models, found_providers)) => {
                update_agent(store, agent_id, true, "provider 桥", models);
                *providers = found_providers;
            }
            Err(error) => {
                set_agent_detection(store, agent_id, false, "未配置");
                errors.push(format!("探测 cc-switch 失败：{error}"));
            }
        }
        return;
    }

    let Some(definition) = definitions.iter().find(|item| item.id == agent_id) else {
        errors.push(format!("探测 {agent_id} 失败：工具注册表中不存在"));
        return;
    };
    let status = tools::detect_all(std::slice::from_ref(definition))
        .into_iter()
        .next()
        .expect("one definition produces one status");
    set_agent_detection(store, agent_id, status.detected, &status.version);
    if !status.detected {
        errors.push(format!(
            "探测 {agent_id} 失败：未检测到 {}",
            definition.binary
        ));
    }

    let models = match agent_id {
        "claude" => Ok(claude_presets()),
        "codex" => detect_codex(),
        "kimi" => detect_kimi(),
        "pi" => status
            .executable_path
            .as_deref()
            .ok_or_else(|| "未检测到 pi".to_string())
            .and_then(|binary| detect_pi(Path::new(binary))),
        _ => Ok(Vec::new()),
    };
    match models {
        Ok(models) => merge_detected_models(store, agent_id, models),
        Err(error) => errors.push(format!("探测 {agent_id} 失败：{error}")),
    }
}

fn set_agent_detection(store: &mut ResourceStore, agent_id: &str, detected: bool, version: &str) {
    if let Some(agent) = store.agents.iter_mut().find(|agent| agent.id == agent_id) {
        agent.detected = detected;
        agent.version = version.to_string();
    }
}

fn update_agent(
    store: &mut ResourceStore,
    agent_id: &str,
    detected: bool,
    version: &str,
    models: Vec<ResourceModel>,
) {
    set_agent_detection(store, agent_id, detected, version);
    merge_detected_models(store, agent_id, models);
}

fn detect_codex() -> Result<Vec<ResourceModel>, String> {
    let path = home_file(&[".codex", "models_cache.json"])?;
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("读取 {} 失败：{error}", path.display()))?;
    parse_codex_cache(&contents, &source_read(&path.display().to_string()))
}

fn parse_codex_cache(contents: &str, source: &str) -> Result<Vec<ResourceModel>, String> {
    let cache: CodexCache =
        serde_json::from_str(contents).map_err(|error| format!("解析模型缓存失败：{error}"))?;
    Ok(cache
        .models
        .into_iter()
        .filter(|model| !model.slug.trim().is_empty())
        .map(|model| {
            let supports_thinking = !model.supported_reasoning_levels.is_empty();
            let efforts = model
                .supported_reasoning_levels
                .iter()
                .filter_map(|value| {
                    value
                        .as_str()
                        .or_else(|| value.get("effort").and_then(Value::as_str))
                })
                .collect::<Vec<_>>();
            let mut note = Vec::new();
            if let Some(priority) = model.priority {
                note.push(format!("priority：{priority}"));
            }
            if !efforts.is_empty() {
                note.push(format!("effort：{}", efforts.join("/")));
            }
            if !model.description.trim().is_empty() {
                note.push(model.description.trim().to_string());
            }
            detected_model(
                &model.slug,
                "codex",
                "",
                "OpenAI",
                "官方订阅",
                if supports_thinking {
                    "支持"
                } else {
                    "未知"
                },
                source,
                &note.join("；"),
            )
        })
        .collect())
}

fn detect_kimi() -> Result<Vec<ResourceModel>, String> {
    let path = home_file(&[".kimi-code", "config.toml"])?;
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("读取 {} 失败：{error}", path.display()))?;
    parse_kimi_config(&contents, &source_read(&path.display().to_string()))
}

fn parse_kimi_config(contents: &str, source: &str) -> Result<Vec<ResourceModel>, String> {
    #[derive(Default)]
    struct Section {
        alias: String,
        real_model: String,
        thinking: bool,
        context_size: Option<u64>,
        efforts: Vec<String>,
        default_effort: String,
    }

    fn finish(section: Option<Section>, source: &str, output: &mut Vec<ResourceModel>) {
        let Some(section) = section else { return };
        if section.alias.is_empty() {
            return;
        }
        let mut note = Vec::new();
        if !section.real_model.is_empty() && section.real_model != section.alias {
            note.push(format!("真实模型 ID：{}", section.real_model));
        }
        if let Some(context_size) = section.context_size {
            note.push(format!("上下文：{context_size}"));
        }
        if !section.efforts.is_empty() {
            let default = if section.default_effort.is_empty() {
                String::new()
            } else {
                format!("，默认 {}", section.default_effort)
            };
            note.push(format!("effort：{}{default}", section.efforts.join("/")));
        }
        output.push(detected_model(
            &section.alias,
            "kimi",
            "",
            "Moonshot",
            "Kimi Code",
            if section.thinking { "支持" } else { "未知" },
            source,
            &note.join("；"),
        ));
    }

    let mut output = Vec::new();
    let mut current: Option<Section> = None;
    for raw in contents.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            finish(current.take(), source, &mut output);
            if let Some(alias) = line
                .strip_prefix("[models.\"")
                .and_then(|value| value.strip_suffix("\"]"))
            {
                current = Some(Section {
                    alias: alias.to_string(),
                    ..Section::default()
                });
            }
            continue;
        }
        let Some(section) = current.as_mut() else {
            continue;
        };
        if let Some(value) = toml_string_value(line, "model") {
            section.real_model = value;
        } else if let Some(value) = toml_integer_value(line, "max_context_size") {
            section.context_size = Some(value);
        } else if let Some(value) = toml_string_array_value(line, "support_efforts") {
            section.efforts = value;
        } else if let Some(value) = toml_string_value(line, "default_effort") {
            section.default_effort = value;
        } else if line.starts_with("capabilities") && line.to_ascii_lowercase().contains("thinking")
        {
            section.thinking = true;
        }
    }
    finish(current, source, &mut output);
    if output.is_empty() {
        return Err("未找到 [models.\"…\"] 段".to_string());
    }
    Ok(output)
}

fn toml_string_value(line: &str, key: &str) -> Option<String> {
    let (left, right) = line.split_once('=')?;
    if left.trim() != key {
        return None;
    }
    let value = right.trim();
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .map(str::to_string)
}

fn toml_integer_value(line: &str, key: &str) -> Option<u64> {
    let (left, right) = line.split_once('=')?;
    (left.trim() == key)
        .then(|| right.trim().parse().ok())
        .flatten()
}

fn toml_string_array_value(line: &str, key: &str) -> Option<Vec<String>> {
    let (left, right) = line.split_once('=')?;
    if left.trim() != key {
        return None;
    }
    let inner = right.trim().strip_prefix('[')?.strip_suffix(']')?;
    Some(
        inner
            .split(',')
            .filter_map(|value| {
                value
                    .trim()
                    .strip_prefix('"')
                    .and_then(|value| value.strip_suffix('"'))
                    .map(str::to_string)
            })
            .collect(),
    )
}

fn detect_pi(binary: &Path) -> Result<Vec<ResourceModel>, String> {
    let path = home_file(&[".pi", "agent", "models-store.json"])?;
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("读取 {} 失败：{error}", path.display()))?;
    let listed = run_process(
        binary,
        &["--list-models".to_string()],
        None,
        Duration::from_secs(20),
        None,
    )?;
    if listed.timed_out {
        return Err("pi --list-models 超时".to_string());
    }
    let combined = if listed.stdout.trim().is_empty() {
        listed.stderr.as_str()
    } else {
        listed.stdout.as_str()
    };
    if combined.trim().is_empty() {
        return Err(format!(
            "pi --list-models 未返回可用模型：{}",
            process_error_summary(&listed)
        ));
    }
    let models = parse_pi_sources(
        &contents,
        combined,
        &source_read("~/.pi/agent/models-store.json + pi --list-models"),
    )?;
    let providers = models
        .iter()
        .map(|model| model.provider.clone())
        .collect::<HashSet<_>>();
    for provider in providers {
        let auth = run_process(
            binary,
            &[
                "auth".to_string(),
                "check".to_string(),
                "--provider".to_string(),
                provider.clone(),
                "--json".to_string(),
                "--no-refresh".to_string(),
            ],
            None,
            Duration::from_secs(20),
            None,
        )?;
        if !pi_auth_ready(&auth) {
            return Err(format!(
                "pi provider {provider} 未就绪：{}",
                process_error_summary(&auth)
            ));
        }
    }
    Ok(models)
}

fn parse_pi_sources(
    contents: &str,
    listed: &str,
    source: &str,
) -> Result<Vec<ResourceModel>, String> {
    let store: HashMap<String, PiStoreProvider> = serde_json::from_str(contents)
        .map_err(|error| format!("解析 models-store.json 失败：{error}"))?;
    let mut output = Vec::new();
    for (provider_name, provider) in store {
        for model in provider.models {
            let effective_provider = if model.provider.trim().is_empty() {
                provider_name.as_str()
            } else {
                model.provider.as_str()
            };
            let qualified = format!("{effective_provider}/{}", model.id);
            let available = listed.lines().any(|line| {
                let line = line.trim();
                line.contains(&qualified)
                    || (line.contains(effective_provider) && line.contains(&model.id))
            });
            if !available {
                continue;
            }
            output.push(detected_model(
                &model.id,
                "pi",
                effective_provider,
                infer_vendor(&model.id),
                effective_provider,
                if model.reasoning { "支持" } else { "未知" },
                source,
                &model
                    .context_window
                    .map(|value| format!("上下文：{value}"))
                    .unwrap_or_default(),
            ));
        }
    }
    if output.is_empty() {
        return Err("--list-models 与 models-store.json 没有可合并条目".to_string());
    }
    Ok(output)
}

fn claude_presets() -> Vec<ResourceModel> {
    ["opus", "sonnet", "haiku"]
        .into_iter()
        .map(|id| {
            detected_model(
                id,
                "claude",
                "",
                "Anthropic",
                "官方订阅",
                "未知",
                "文档(2026-08-30)",
                "",
            )
        })
        .collect()
}

fn detect_cc_switch(
    settings: &Settings,
) -> Result<(Vec<ResourceModel>, Vec<CcSwitchProvider>), String> {
    let script = find_ai_cross_reference(settings, "cc_switch.py")
        .ok_or_else(|| "未配置 ai-cross 目录".to_string())?;
    let python = tools::resolve_binary_name(if cfg!(windows) {
        "python.exe"
    } else {
        "python"
    })
    .ok_or_else(|| "未检测到 Python".to_string())?;
    let result = run_process(
        &python,
        &[script.to_string_lossy().to_string(), "list".to_string()],
        None,
        Duration::from_secs(20),
        None,
    )?;
    if result.timed_out || result.code != Some(0) {
        return Err(process_error_summary(&result));
    }
    parse_cc_switch_list(
        &result.stdout,
        &source_read(&format!("python {} list", script.display())),
    )
}

fn parse_cc_switch_list(
    contents: &str,
    source: &str,
) -> Result<(Vec<ResourceModel>, Vec<CcSwitchProvider>), String> {
    let raw: Vec<RawCcProvider> = serde_json::from_str(contents)
        .map_err(|error| format!("解析 cc_switch.py list 输出失败：{error}"))?;
    let mut models = Vec::new();
    let mut providers = Vec::new();
    for provider in raw {
        let mut ids = provider
            .tier_models
            .values()
            .filter(|id| !id.trim().is_empty())
            .cloned()
            .collect::<Vec<_>>();
        ids.sort();
        ids.dedup();
        if !ids.is_empty() {
            providers.push(CcSwitchProvider {
                name: provider.name.clone(),
                has_token: provider.has_token,
                models: ids.clone(),
            });
        }
        if provider.app_type != "claude" {
            continue;
        }
        for id in ids {
            models.push(detected_model(
                &id,
                "cc-switch",
                &provider.name,
                infer_vendor(&id),
                &provider.name,
                "未知",
                source,
                "",
            ));
        }
    }
    Ok((models, providers))
}

fn source_read(source: &str) -> String {
    format!(
        "读:{source} ({})",
        Local::now().to_rfc3339_opts(SecondsFormat::Secs, false)
    )
}

#[allow(clippy::too_many_arguments)]
fn detected_model(
    id: &str,
    agent: &str,
    provider: &str,
    vendor: &str,
    billing_source: &str,
    thinking: &str,
    source: &str,
    note: &str,
) -> ResourceModel {
    ResourceModel {
        id: id.to_string(),
        agent: agent.to_string(),
        provider: provider.to_string(),
        vendor: vendor.to_string(),
        billing_source: billing_source.to_string(),
        tier: String::new(),
        thinking: thinking.to_string(),
        status: STATUS_PENDING.to_string(),
        source: source.to_string(),
        smoked_at: String::new(),
        smoke_model: String::new(),
        note: note.to_string(),
    }
}

fn model_key(model: &ResourceModel) -> (&str, &str, &str) {
    (&model.agent, &model.provider, &model.id)
}

fn merge_detected_models(
    store: &mut ResourceStore,
    agent_id: &str,
    discovered: Vec<ResourceModel>,
) {
    let Some(agent) = store.agents.iter_mut().find(|agent| agent.id == agent_id) else {
        return;
    };
    let discovered_keys = discovered
        .iter()
        .map(|model| {
            (
                model.agent.clone(),
                model.provider.clone(),
                model.id.clone(),
            )
        })
        .collect::<HashSet<_>>();

    for mut model in discovered {
        if let Some(existing) = agent
            .models
            .iter_mut()
            .find(|existing| model_key(existing) == model_key(&model))
        {
            model.tier.clone_from(&existing.tier);
            model.status.clone_from(&existing.status);
            model.smoked_at.clone_from(&existing.smoked_at);
            model.smoke_model.clone_from(&existing.smoke_model);
            if !existing.vendor.trim().is_empty() {
                model.vendor.clone_from(&existing.vendor);
            }
            model.note = remove_note_marker(&existing.note, MISSING_MARKER);
            *existing = model;
        } else {
            agent.models.push(model);
        }
    }

    for existing in &mut agent.models {
        let key = (
            existing.agent.clone(),
            existing.provider.clone(),
            existing.id.clone(),
        );
        if !discovered_keys.contains(&key) && !existing.source.starts_with("申报(") {
            existing.note = append_note_marker(&existing.note, MISSING_MARKER);
        }
    }
    agent
        .models
        .sort_by(|left, right| (&left.provider, &left.id).cmp(&(&right.provider, &right.id)));
}

fn append_note_marker(note: &str, marker: &str) -> String {
    if note.split('；').any(|part| part.trim() == marker) {
        return note.to_string();
    }
    if note.trim().is_empty() {
        marker.to_string()
    } else {
        format!("{note}；{marker}")
    }
}

fn remove_note_marker(note: &str, marker: &str) -> String {
    note.split('；')
        .filter(|part| part.trim() != marker)
        .collect::<Vec<_>>()
        .join("；")
}

pub fn add_model(
    mut store: ResourceStore,
    request: AddResourceModelRequest,
) -> Result<ResourceStore, String> {
    let id = request.id.trim();
    if id.is_empty() {
        return Err("手动输入失败：模型 ID 不能为空".to_string());
    }
    let Some(agent) = store
        .agents
        .iter_mut()
        .find(|agent| agent.id == request.agent_id)
    else {
        return Err(format!("手动输入失败：未知 agent：{}", request.agent_id));
    };
    let provider = request.provider.trim();
    if agent.models.iter().any(|model| {
        model.id == id && model.provider == provider && model.agent == request.agent_id
    }) {
        return Err("手动输入失败：该模型已存在".to_string());
    }
    agent.models.push(ResourceModel {
        id: id.to_string(),
        agent: request.agent_id,
        provider: provider.to_string(),
        vendor: if request.vendor.trim().is_empty() {
            infer_vendor(id).to_string()
        } else {
            request.vendor.trim().to_string()
        },
        billing_source: if provider.is_empty() {
            agent.id.clone()
        } else {
            provider.to_string()
        },
        tier: request.tier.trim().to_string(),
        thinking: "未知".to_string(),
        status: STATUS_PENDING.to_string(),
        source: format!("申报({})", Local::now().format("%Y-%m-%d")),
        smoked_at: String::new(),
        smoke_model: String::new(),
        note: request.note.trim().to_string(),
    });
    Ok(store)
}

pub fn update_model(
    mut store: ResourceStore,
    request: UpdateResourceModelRequest,
) -> Result<ResourceStore, String> {
    let model = find_model_mut(
        &mut store,
        &request.agent_id,
        &request.provider,
        &request.id,
    )?;
    model.vendor = request.vendor.trim().to_string();
    model.tier = request.tier.trim().to_string();
    model.note = request.note.trim().to_string();
    Ok(store)
}

pub fn remove_model(
    mut store: ResourceStore,
    request: ResourceModelKey,
) -> Result<ResourceStore, String> {
    let Some(agent) = store
        .agents
        .iter_mut()
        .find(|agent| agent.id == request.agent_id)
    else {
        return Err(format!("移除失败：未知 agent：{}", request.agent_id));
    };
    let before = agent.models.len();
    agent
        .models
        .retain(|model| !(model.id == request.id && model.provider == request.provider));
    if before == agent.models.len() {
        return Err("移除失败：模型不存在".to_string());
    }
    Ok(store)
}

pub fn manual_agent_definition(request: AddManualAgentRequest) -> Result<ToolDefinition, String> {
    let name = request.name.trim();
    let binary = request.binary.trim();
    if name.is_empty() || binary.is_empty() {
        return Err("手动添加失败：名称和二进制名不能为空".to_string());
    }
    Ok(ToolDefinition {
        id: format!("manual-{}", Uuid::new_v4()),
        name: name.to_string(),
        binary: binary.to_string(),
        default_args: request.args,
        description: "用户手动添加的 agent".to_string(),
    })
}

pub fn smoke_one(
    mut store: ResourceStore,
    request: &ResourceModelKey,
    definitions: &[ToolDefinition],
    settings: &Settings,
) -> Result<ResourceStore, String> {
    let model = find_model(&store, &request.agent_id, &request.provider, &request.id)?.clone();
    let outcome = smoke_model(&model, definitions, settings);
    let target = find_model_mut(
        &mut store,
        &request.agent_id,
        &request.provider,
        &request.id,
    )?;
    match outcome {
        Ok(smoke_model) => {
            target.status = STATUS_CONNECTED.to_string();
            target.smoked_at = Local::now().to_rfc3339_opts(SecondsFormat::Secs, false);
            target.smoke_model = smoke_model;
            target.note = remove_smoke_failure(&target.note);
        }
        Err(error) => {
            target.status = STATUS_FAILED.to_string();
            target.smoked_at = Local::now().to_rfc3339_opts(SecondsFormat::Secs, false);
            target.smoke_model.clear();
            target.note = set_smoke_failure(&target.note, &safe_summary(&error));
        }
    }
    Ok(store)
}

pub fn smoke_selected(
    mut store: ResourceStore,
    selection: &SmokeSelectionRequest,
    definitions: &[ToolDefinition],
    settings: &Settings,
) -> ResourceOperation {
    let keys = store
        .agents
        .iter()
        .filter(|agent| selection.agent_ids.iter().any(|id| id == &agent.id))
        .flat_map(|agent| {
            agent.models.iter().filter_map(|model| {
                if model.agent == "cc-switch"
                    && !selection
                        .cc_switch_providers
                        .iter()
                        .any(|provider| provider == &model.provider)
                {
                    return None;
                }
                Some(ResourceModelKey {
                    agent_id: agent.id.clone(),
                    id: model.id.clone(),
                    provider: model.provider.clone(),
                })
            })
        })
        .collect::<Vec<_>>();
    let mut errors = Vec::new();
    for key in keys {
        match smoke_one(store.clone(), &key, definitions, settings) {
            Ok(updated) => {
                store = updated;
                if let Ok(model) = find_model(&store, &key.agent_id, &key.provider, &key.id) {
                    if model.status == STATUS_FAILED {
                        errors.push(format!(
                            "冒烟 {}/{} 失败：{}",
                            key.agent_id, key.id, model.note
                        ));
                    }
                }
            }
            Err(error) => errors.push(error),
        }
    }
    let providers = connection_overview(settings)
        .map(|overview| overview.cc_switch_providers)
        .unwrap_or_default();
    ResourceOperation {
        store,
        errors,
        cc_switch_providers: providers,
    }
}

fn smoke_model(
    model: &ResourceModel,
    definitions: &[ToolDefinition],
    settings: &Settings,
) -> Result<String, String> {
    if model.agent == "cc-switch" {
        return smoke_cc_switch(model, settings);
    }
    let definition = definitions
        .iter()
        .find(|definition| definition.id == model.agent)
        .ok_or_else(|| format!("工具注册表中不存在 {}", model.agent))?;
    let binary =
        tools::resolve_binary(definition).ok_or_else(|| format!("未检测到 {}", definition.name))?;
    let temp = std::env::temp_dir();
    match model.agent.as_str() {
        "claude" => {
            let result = run_process(
                &binary,
                &[
                    "-p".to_string(),
                    "--model".to_string(),
                    model.id.clone(),
                    "--tools".to_string(),
                    String::new(),
                    "--output-format".to_string(),
                    "json".to_string(),
                ],
                Some(SMOKE_PROMPT),
                Duration::from_secs(60),
                Some(&temp),
            )?;
            parse_claude_smoke(&result)
        }
        "codex" => {
            let result = run_process(
                &binary,
                &[
                    "exec".to_string(),
                    "-m".to_string(),
                    model.id.clone(),
                    "-s".to_string(),
                    "read-only".to_string(),
                    "--skip-git-repo-check".to_string(),
                    "--json".to_string(),
                    "-".to_string(),
                ],
                Some(SMOKE_PROMPT),
                Duration::from_secs(60),
                Some(&temp),
            )?;
            parse_codex_smoke(&result)
        }
        "kimi" => {
            let result = run_process(
                &binary,
                &[
                    "-p".to_string(),
                    SMOKE_PROMPT.to_string(),
                    "-m".to_string(),
                    model.id.clone(),
                    "--output-format".to_string(),
                    "text".to_string(),
                ],
                None,
                Duration::from_secs(60),
                Some(&temp),
            )?;
            parse_kimi_smoke(&result)
        }
        "pi" => {
            let result = run_process(
                &binary,
                &[
                    "--provider".to_string(),
                    model.provider.clone(),
                    "--model".to_string(),
                    model.id.clone(),
                    "-p".to_string(),
                    "--no-tools".to_string(),
                    "--no-extensions".to_string(),
                    "--no-context-files".to_string(),
                    "--no-session".to_string(),
                    "--mode".to_string(),
                    "json".to_string(),
                ],
                Some(SMOKE_PROMPT),
                Duration::from_secs(60),
                Some(&temp),
            )?;
            parse_pi_smoke(&result, &model.provider, &model.id)
        }
        _ => Err("手动 agent 只支持手动录入模型，尚无冒烟命令模板".to_string()),
    }
}

fn parse_claude_smoke(result: &ProcessResult) -> Result<String, String> {
    process_completed(result)?;
    let value: Value = serde_json::from_str(result.stdout.trim())
        .map_err(|error| format!("Claude 输出不是 JSON：{error}"))?;
    if value.get("is_error").and_then(Value::as_bool) == Some(false)
        && value
            .get("result")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
    {
        return Ok(value
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string());
    }
    Err(format!(
        "Claude 未返回有效回答：{}",
        process_error_summary(result)
    ))
}

fn parse_codex_smoke(result: &ProcessResult) -> Result<String, String> {
    process_completed(result)?;
    let completed = json_lines(&result.stdout)
        .any(|value| value.get("type").and_then(Value::as_str) == Some("turn.completed"));
    if completed {
        Ok(String::new())
    } else {
        Err(format!(
            "Codex 未出现 turn.completed：{}",
            process_error_summary(result)
        ))
    }
}

fn parse_kimi_smoke(result: &ProcessResult) -> Result<String, String> {
    process_completed(result)?;
    let output = result.stdout.trim();
    let combined = format!("{}\n{}", result.stdout, result.stderr).to_ascii_lowercase();
    if !output.is_empty() && !combined.contains("error") {
        Ok(String::new())
    } else {
        Err(format!(
            "Kimi 未返回有效回答：{}",
            process_error_summary(result)
        ))
    }
}

fn parse_pi_smoke(
    result: &ProcessResult,
    expected_provider: &str,
    expected_model: &str,
) -> Result<String, String> {
    process_completed(result)?;
    let values = json_lines(&result.stdout).collect::<Vec<_>>();
    let settled = values
        .iter()
        .any(|value| value.get("type").and_then(Value::as_str) == Some("agent_settled"));
    if !settled {
        return Err("pi 未出现 agent_settled".to_string());
    }
    let mut observation: Option<(&Value, Option<String>, Option<String>)> = None;
    for value in &values {
        let mut messages = Vec::new();
        collect_assistant_messages(value, &mut messages);
        for message in messages {
            let provider = direct_named_string(message, "provider")
                .or_else(|| direct_named_string(value, "provider"));
            let model = direct_named_string(message, "model")
                .or_else(|| direct_named_string(value, "model"));
            observation = Some((message, provider, model));
        }
    }
    let Some((message, observed_provider, observed_model)) = observation else {
        return Err("pi 未返回 assistant 消息".to_string());
    };
    if observed_provider.as_deref() != Some(expected_provider)
        || observed_model.as_deref() != Some(expected_model)
    {
        return Err(format!(
            "响应模型不符：请求 {expected_provider}/{expected_model}，响应 {}/{}",
            observed_provider.as_deref().unwrap_or("—"),
            observed_model.as_deref().unwrap_or("—")
        ));
    }
    let has_text = message
        .get("content")
        .and_then(Value::as_array)
        .is_some_and(|blocks| {
            blocks.iter().any(|block| {
                block.get("type").and_then(Value::as_str) == Some("text")
                    && block
                        .get("text")
                        .and_then(Value::as_str)
                        .is_some_and(|text| !text.trim().is_empty())
            })
        });
    if !has_text {
        return Err("pi 的末条 assistant 消息没有 text 块".to_string());
    }
    Ok(observed_model.unwrap_or_default())
}

fn collect_assistant_messages<'a>(value: &'a Value, messages: &mut Vec<&'a Value>) {
    match value {
        Value::Object(map) => {
            if map.get("role").and_then(Value::as_str) == Some("assistant") {
                messages.push(value);
            }
            for child in map.values() {
                collect_assistant_messages(child, messages);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_assistant_messages(child, messages);
            }
        }
        _ => {}
    }
}

fn direct_named_string(value: &Value, name: &str) -> Option<String> {
    value.get(name).and_then(Value::as_str).map(str::to_string)
}

fn smoke_cc_switch(model: &ResourceModel, settings: &Settings) -> Result<String, String> {
    let script = find_ai_cross_reference(settings, "cc_switch.py")
        .ok_or_else(|| "未配置 ai-cross 目录".to_string())?;
    let verify = find_ai_cross_reference(settings, "verify_model.py")
        .ok_or_else(|| "未找到 verify_model.py".to_string())?;
    let python = tools::resolve_binary_name(if cfg!(windows) {
        "python.exe"
    } else {
        "python"
    })
    .ok_or_else(|| "未检测到 Python".to_string())?;
    let tier = cc_switch_tier_for_model(&script, &python, model)?;
    let temp = std::env::temp_dir();
    let exec = run_process(
        &python,
        &[
            script.to_string_lossy().to_string(),
            "exec".to_string(),
            "--provider".to_string(),
            model.provider.clone(),
            "--tier".to_string(),
            tier,
            "--task".to_string(),
            SMOKE_PROMPT.to_string(),
        ],
        None,
        Duration::from_secs(60),
        Some(&temp),
    )?;
    if exec.timed_out || exec.code != Some(0) || output_reports_is_error(&exec) {
        return Err(format!(
            "cc-switch 执行失败：{}",
            process_error_summary(&exec)
        ));
    }
    let verified = run_process(
        &python,
        &[
            verify.to_string_lossy().to_string(),
            "--provider".to_string(),
            model.provider.clone(),
        ],
        None,
        Duration::from_secs(60),
        Some(&temp),
    )?;
    let verification_text = format!("{}\n{}", verified.stdout, verified.stderr);
    let verification_passed = cc_verification_passed(&verification_text);
    if verified.timed_out || !verification_passed {
        return Err(format!(
            "模型真身校验失败：{}",
            process_error_summary(&verified)
        ));
    }
    Ok(model.id.clone())
}

fn cc_switch_tier_for_model(
    script: &Path,
    python: &Path,
    model: &ResourceModel,
) -> Result<String, String> {
    let result = run_process(
        python,
        &[script.to_string_lossy().to_string(), "list".to_string()],
        None,
        Duration::from_secs(20),
        None,
    )?;
    if result.timed_out || result.code != Some(0) {
        return Err(process_error_summary(&result));
    }
    let raw: Vec<RawCcProvider> = serde_json::from_str(&result.stdout)
        .map_err(|error| format!("解析 cc-switch 档位失败：{error}"))?;
    let provider = raw
        .into_iter()
        .find(|provider| provider.name == model.provider)
        .ok_or_else(|| format!("cc-switch 中不存在 provider：{}", model.provider))?;
    let preferred = match model.tier.as_str() {
        "低" => Some("haiku"),
        "中" => Some("sonnet"),
        "高" => Some("opus"),
        _ => None,
    };
    if let Some(tier) = preferred {
        if provider
            .tier_models
            .get(tier)
            .is_some_and(|id| id == &model.id)
        {
            return Ok(tier.to_string());
        }
    }
    ["haiku", "sonnet", "opus"]
        .into_iter()
        .find(|tier| {
            provider
                .tier_models
                .get(*tier)
                .is_some_and(|id| id == &model.id)
        })
        .map(str::to_string)
        .ok_or_else(|| {
            format!(
                "provider {} 没有模型 {} 的档位映射",
                model.provider, model.id
            )
        })
}

pub fn connection_overview(settings: &Settings) -> Result<ConnectionOverview, String> {
    let (_, providers) = detect_cc_switch(settings)?;
    Ok(ConnectionOverview {
        cc_switch_providers: providers,
    })
}

fn find_ai_cross_reference(settings: &Settings, file_name: &str) -> Option<PathBuf> {
    let mut roots = Vec::new();
    if !settings.ai_cross_dir.trim().is_empty() {
        roots.push(PathBuf::from(settings.ai_cross_dir.trim()));
    }
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".claude").join("skills").join("ai-cross"));
        roots.push(home.join(".codex").join("skills").join("ai-cross"));
    }
    if cfg!(windows) {
        roots.push(PathBuf::from(r"D:\work\agentteam\dist\ai-cross"));
    }
    roots.into_iter().find_map(|root| {
        [
            root.join("references").join(file_name),
            root.join(file_name),
        ]
        .into_iter()
        .find(|path| path.is_file())
    })
}

fn find_model<'a>(
    store: &'a ResourceStore,
    agent_id: &str,
    provider: &str,
    id: &str,
) -> Result<&'a ResourceModel, String> {
    store
        .agents
        .iter()
        .find(|agent| agent.id == agent_id)
        .and_then(|agent| {
            agent
                .models
                .iter()
                .find(|model| model.id == id && model.provider == provider)
        })
        .ok_or_else(|| format!("资源不存在：{agent_id}/{id}"))
}

fn find_model_mut<'a>(
    store: &'a mut ResourceStore,
    agent_id: &str,
    provider: &str,
    id: &str,
) -> Result<&'a mut ResourceModel, String> {
    store
        .agents
        .iter_mut()
        .find(|agent| agent.id == agent_id)
        .and_then(|agent| {
            agent
                .models
                .iter_mut()
                .find(|model| model.id == id && model.provider == provider)
        })
        .ok_or_else(|| format!("资源不存在：{agent_id}/{id}"))
}

fn infer_vendor(id: &str) -> &'static str {
    let normalized = id.to_ascii_lowercase();
    if normalized.contains("claude") || normalized.contains("anthropic") {
        "Anthropic"
    } else if normalized.starts_with("gpt-")
        || normalized.starts_with("o1")
        || normalized.starts_with("o3")
        || normalized.starts_with("o4")
        || normalized.contains("codex")
    {
        "OpenAI"
    } else if normalized.contains("moonshot") || normalized.contains("kimi") {
        "Moonshot"
    } else if normalized.contains("glm") || normalized.contains("zhipu") {
        "Zhipu"
    } else if normalized.contains("deepseek") {
        "DeepSeek"
    } else if normalized.contains("minimax") {
        "MiniMax"
    } else if normalized.contains("qwen") || normalized.contains("alibaba") {
        "Alibaba"
    } else if normalized.contains("stepfun") || normalized.starts_with("step-") {
        "StepFun"
    } else {
        "其他"
    }
}

fn home_file(parts: &[&str]) -> Result<PathBuf, String> {
    let mut path = dirs::home_dir().ok_or_else(|| "无法确定用户目录".to_string())?;
    for part in parts {
        path.push(part);
    }
    Ok(path)
}

fn process_completed(result: &ProcessResult) -> Result<(), String> {
    if result.timed_out {
        Err("60 秒超时".to_string())
    } else {
        Ok(())
    }
}

fn json_lines(text: &str) -> impl Iterator<Item = Value> + '_ {
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
}

fn pi_auth_ready(result: &ProcessResult) -> bool {
    if result.timed_out {
        return false;
    }
    let values = if let Ok(value) = serde_json::from_str::<Value>(result.stdout.trim()) {
        vec![value]
    } else {
        json_lines(&result.stdout).collect::<Vec<_>>()
    };
    values.iter().any(value_reports_ready)
}

fn value_reports_ready(value: &Value) -> bool {
    match value {
        Value::Object(map) => {
            map.iter().any(|(key, value)| {
                matches!(
                    key.as_str(),
                    "ready" | "ok" | "authenticated" | "available" | "configured" | "valid"
                ) && value.as_bool() == Some(true)
            }) || map
                .get("status")
                .and_then(Value::as_str)
                .is_some_and(|status| {
                    matches!(
                        status.to_ascii_lowercase().as_str(),
                        "ready" | "ok" | "authenticated"
                    )
                })
                || map.values().any(value_reports_ready)
        }
        Value::Array(values) => values.iter().any(value_reports_ready),
        _ => false,
    }
}

fn run_process(
    program: &Path,
    args: &[String],
    input: Option<&str>,
    timeout: Duration,
    cwd: Option<&Path>,
) -> Result<ProcessResult, String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .env("AI_CROSS_PEER", "1")
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("启动 {} 失败：{error}", program.display()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "读取子进程 stdout 失败".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "读取子进程 stderr 失败".to_string())?;
    let stdout_reader = bounded_reader(stdout);
    let stderr_reader = bounded_reader(stderr);
    if let Some(input) = input {
        if let Some(mut stdin) = child.stdin.take() {
            if let Err(error) = stdin.write_all(input.as_bytes()) {
                terminate_process_tree(&mut child);
                return Err(format!("写入子进程 stdin 失败：{error}"));
            }
        }
    }

    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(25)),
            Ok(None) => {
                timed_out = true;
                terminate_process_tree(&mut child);
                break None;
            }
            Err(error) => {
                terminate_process_tree(&mut child);
                return Err(format!("等待子进程失败：{error}"));
            }
        }
    };
    let stdout = finish_bounded_reader(stdout_reader);
    let stderr = finish_bounded_reader(stderr_reader);
    Ok(ProcessResult {
        code,
        stdout: String::from_utf8_lossy(&stdout).to_string(),
        stderr: String::from_utf8_lossy(&stderr).to_string(),
        timed_out,
    })
}

type Reader = (thread::JoinHandle<()>, mpsc::Receiver<Vec<u8>>);

fn bounded_reader<R: Read + Send + 'static>(mut reader: R) -> Reader {
    let (sender, receiver) = mpsc::channel();
    let handle = thread::spawn(move || {
        let mut output = Vec::new();
        let _ = (&mut reader)
            .take(MAX_PROCESS_OUTPUT)
            .read_to_end(&mut output);
        let _ = std::io::copy(&mut reader, &mut std::io::sink());
        let _ = sender.send(output);
    });
    (handle, receiver)
}

fn finish_bounded_reader(reader: Reader) -> Vec<u8> {
    let (handle, receiver) = reader;
    match receiver.recv_timeout(Duration::from_secs(2)) {
        Ok(output) => {
            let _ = handle.join();
            output
        }
        Err(_) => Vec::new(),
    }
}

fn output_reports_is_error(result: &ProcessResult) -> bool {
    let combined = format!("{}\n{}", result.stdout, result.stderr);
    if let Ok(value) = serde_json::from_str::<Value>(combined.trim()) {
        return value.get("is_error").and_then(Value::as_bool) == Some(true)
            || value
                .get("api_error_status")
                .is_some_and(|status| !status.is_null());
    }
    let normalized = combined.to_ascii_lowercase().replace(' ', "");
    normalized.contains("\"is_error\":true") || normalized.contains("is_error=true")
}

fn cc_verification_passed(text: &str) -> bool {
    text.lines()
        .any(|line| line.trim().starts_with("结论：全部一致，无静默降级"))
}

fn terminate_process_tree(child: &mut std::process::Child) {
    #[cfg(windows)]
    {
        let process_id = child.id().to_string();
        let _ = Command::new("taskkill.exe")
            .args(["/PID", &process_id, "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn process_error_summary(result: &ProcessResult) -> String {
    if result.timed_out {
        return "60 秒超时".to_string();
    }
    let text = if result.stderr.trim().is_empty() {
        result.stdout.trim()
    } else {
        result.stderr.trim()
    };
    if text.is_empty() {
        format!(
            "进程退出，exit={}",
            result.code.map_or("—".to_string(), |code| code.to_string())
        )
    } else {
        safe_summary(text)
    }
}

fn safe_summary(text: &str) -> String {
    let mut value = text.replace(['\r', '\n'], " ");
    for (name, secret) in std::env::vars() {
        let upper = name.to_ascii_uppercase();
        if secret.len() >= 4
            && ["KEY", "TOKEN", "SECRET", "PASSWORD", "AUTH"]
                .iter()
                .any(|marker| upper.contains(marker))
        {
            value = value.replace(&secret, "[已隐藏]");
        }
    }
    value.trim().chars().take(200).collect()
}

fn set_smoke_failure(note: &str, error: &str) -> String {
    let clean = remove_smoke_failure(note);
    if clean.is_empty() {
        format!("冒烟失败：{error}")
    } else {
        format!("{clean}；冒烟失败：{error}")
    }
}

fn remove_smoke_failure(note: &str) -> String {
    note.split('；')
        .filter(|part| !part.trim().starts_with("冒烟失败："))
        .collect::<Vec<_>>()
        .join("；")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_model(id: &str) -> ResourceModel {
        detected_model(
            id,
            "codex",
            "",
            "OpenAI",
            "官方订阅",
            "支持",
            "读:fixture",
            "",
        )
    }

    #[test]
    fn parses_four_local_fact_sources() {
        let codex = parse_codex_cache(include_str!("../fixtures/codex_models.json"), "读:fixture")
            .expect("codex fixture");
        assert_eq!(codex[0].id, "gpt-5.6-terra");
        assert_eq!(codex[0].thinking, "支持");
        assert!(codex[0].note.contains("effort"));

        let kimi = parse_kimi_config(include_str!("../fixtures/kimi_config.toml"), "读:fixture")
            .expect("kimi fixture");
        assert_eq!(kimi[0].id, "kimi-code/k3");
        assert!(kimi[0].note.contains("k3"));
        assert!(kimi[0].note.contains("1048576"));
        assert!(kimi[0].note.contains("low/high/max"));

        let pi = parse_pi_sources(
            include_str!("../fixtures/pi_models_store.json"),
            include_str!("../fixtures/pi_list_models.txt"),
            "读:fixture",
        )
        .expect("pi fixture");
        assert_eq!(pi.len(), 1);
        assert_eq!(pi[0].provider, "zai-coding-cn");
        assert_eq!(pi[0].vendor, "Zhipu");
        assert!(pi[0].note.contains("204800"));

        let (cc_models, providers) = parse_cc_switch_list(
            include_str!("../fixtures/cc_switch_list.json"),
            "读:fixture",
        )
        .expect("cc-switch fixture");
        assert_eq!(cc_models.len(), 1);
        assert_eq!(providers[0].name, "Zhipu GLM");
        assert!(providers[0].has_token);
    }

    #[test]
    fn merge_preserves_user_fields_adds_pending_and_marks_missing() {
        let mut store = ResourceStore {
            version: 1,
            agents: vec![ResourceAgent {
                id: "codex".to_string(),
                kind: "内置".to_string(),
                detected: true,
                version: "test".to_string(),
                models: vec![ResourceModel {
                    id: "old-model".to_string(),
                    agent: "codex".to_string(),
                    provider: String::new(),
                    vendor: "用户厂商".to_string(),
                    billing_source: "旧源".to_string(),
                    tier: "高".to_string(),
                    thinking: "未知".to_string(),
                    status: STATUS_CONNECTED.to_string(),
                    source: "读:old".to_string(),
                    smoked_at: "2026-01-01".to_string(),
                    smoke_model: "old-model".to_string(),
                    note: "保留备注".to_string(),
                }],
            }],
        };
        let mut refreshed = fixture_model("old-model");
        refreshed.billing_source = "新源".to_string();
        merge_detected_models(
            &mut store,
            "codex",
            vec![refreshed, fixture_model("new-model")],
        );
        let old = find_model(&store, "codex", "", "old-model").expect("old");
        assert_eq!(old.tier, "高");
        assert_eq!(old.note, "保留备注");
        assert_eq!(old.status, STATUS_CONNECTED);
        assert_eq!(old.vendor, "用户厂商");
        assert_eq!(old.billing_source, "新源");
        assert_eq!(
            find_model(&store, "codex", "", "new-model")
                .expect("new")
                .status,
            STATUS_PENDING
        );

        merge_detected_models(&mut store, "codex", vec![fixture_model("new-model")]);
        assert!(find_model(&store, "codex", "", "old-model")
            .expect("missing retained")
            .note
            .contains(MISSING_MARKER));
    }

    #[test]
    fn pi_smoke_rejects_identity_mismatch_and_thinking_only_answer() {
        let mismatch = ProcessResult {
            code: Some(1),
            stdout: include_str!("../fixtures/pi_smoke_mismatch.jsonl").to_string(),
            stderr: String::new(),
            timed_out: false,
        };
        let error = parse_pi_smoke(&mismatch, "zai-coding-cn", "glm-4.7")
            .expect_err("identity mismatch must fail");
        assert!(error.contains("响应模型不符"));

        let thinking_only = ProcessResult {
            code: Some(0),
            stdout: include_str!("../fixtures/pi_smoke_thinking_only.jsonl").to_string(),
            stderr: String::new(),
            timed_out: false,
        };
        let error = parse_pi_smoke(&thinking_only, "zai-coding-cn", "glm-4.7")
            .expect_err("thinking block must not count as answer");
        assert!(error.contains("没有 text 块"));

        let success = ProcessResult {
            code: Some(1),
            stdout: include_str!("../fixtures/pi_smoke_success.jsonl").to_string(),
            stderr: String::new(),
            timed_out: false,
        };
        assert_eq!(
            parse_pi_smoke(&success, "zai-coding-cn", "glm-4.7").expect("pi success"),
            "glm-4.7"
        );
    }

    #[test]
    fn other_smoke_parsers_apply_channel_specific_success_rules() {
        let claude = ProcessResult {
            code: Some(0),
            stdout: r#"{"is_error":false,"result":"OK","model":"sonnet"}"#.to_string(),
            stderr: String::new(),
            timed_out: false,
        };
        assert_eq!(parse_claude_smoke(&claude).expect("claude"), "sonnet");

        let codex = ProcessResult {
            code: Some(0),
            stdout: "{\"type\":\"thread.started\"}\n{\"type\":\"turn.completed\"}\n".to_string(),
            stderr: String::new(),
            timed_out: false,
        };
        assert!(parse_codex_smoke(&codex).is_ok());

        let kimi = ProcessResult {
            code: Some(1),
            stdout: "OK\n".to_string(),
            stderr: String::new(),
            timed_out: false,
        };
        assert!(parse_kimi_smoke(&kimi).is_ok());

        assert!(cc_verification_passed(
            "glm-4.7 → glm-4.7 一致\n结论：全部一致，无静默降级。可放心使用。"
        ));
        assert!(!cc_verification_passed("结论：1 个静默降级、0 个错误。"));
    }

    #[test]
    #[ignore = "explicit local fact-source check"]
    fn live_codex_and_kimi_fact_sources_parse() {
        let codex = detect_codex().expect("live codex cache");
        let kimi = detect_kimi().expect("live kimi config");
        println!("live codex models={}", codex.len());
        println!("live kimi models={}", kimi.len());
        assert!(!codex.is_empty());
        assert!(!kimi.is_empty());
    }
}
