use crate::models::{
    AddManualAgentRequest, CcSwitchProvider, ConnectionOverview, ResourceAgent, ResourceModel,
    ResourceModelKey, ResourceModelRequest, ResourceOperation, ResourceStore, Settings,
    SmokeSelectionRequest, ToolDefinition,
};
use crate::process::{run_process, ProcessResult};
use crate::tools;
use chrono::{Local, SecondsFormat};
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;
use uuid::Uuid;

const STATUS_PENDING: &str = "待冒烟";
const STATUS_CONNECTED: &str = "已接入";
const STATUS_FAILED: &str = "冒烟失败";
const MISSING_MARKER: &str = "未在事实源中";
const SMOKE_PROMPT: &str = "只回复 OK";
#[derive(Debug, Deserialize)]
struct CodexCache {
    #[serde(default)]
    fetched_at: Option<String>,
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
        cc_switch_providers: Some(providers),
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
        cc_switch_providers: Some(providers),
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
        if agent_id == "claude" {
            return;
        }
    }

    if agent_id == "pi" {
        let result = status
            .executable_path
            .as_deref()
            .ok_or_else(|| "未检测到 pi".to_string())
            .and_then(|binary| detect_pi(Path::new(binary)));
        match result {
            Ok((models, provider_errors)) => {
                merge_detected_models(store, agent_id, models);
                errors.extend(provider_errors);
            }
            Err(error) => errors.push(format!("探测 {agent_id} 失败：{error}")),
        }
        return;
    }

    let models = match agent_id {
        "claude" => Ok(claude_presets()),
        "codex" => detect_codex(),
        "kimi" => detect_kimi(),
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
    if cache.models.is_empty() {
        return Err("模型缓存清单为空".to_string());
    }
    let fetched_at = cache.fetched_at.unwrap_or_default();
    let models = cache
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
            if !fetched_at.trim().is_empty() {
                note.push(format!("缓存抓取时间：{fetched_at}"));
            }
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
        .collect::<Vec<_>>();
    if models.is_empty() {
        Err("模型缓存没有有效模型".to_string())
    } else {
        Ok(models)
    }
}

fn detect_kimi() -> Result<Vec<ResourceModel>, String> {
    let path = home_file(&[".kimi-code", "config.toml"])?;
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("读取 {} 失败：{error}", path.display()))?;
    parse_kimi_config(&contents, &source_read(&path.display().to_string()))
}

fn parse_kimi_config(contents: &str, source: &str) -> Result<Vec<ResourceModel>, String> {
    let root: toml::Value =
        toml::from_str(contents).map_err(|error| format!("解析 Kimi TOML 失败：{error}"))?;
    let sections = root
        .get("models")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "未找到 [models.\"…\"] 段".to_string())?;
    let mut output = Vec::new();
    for (alias, value) in sections {
        let Some(section) = value.as_table() else {
            continue;
        };
        let real_model = section
            .get("model")
            .and_then(toml::Value::as_str)
            .unwrap_or_default();
        let context_size = section
            .get("max_context_size")
            .and_then(toml::Value::as_integer)
            .and_then(|value| u64::try_from(value).ok());
        let capabilities = section
            .get("capabilities")
            .and_then(toml::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let efforts = section
            .get("support_efforts")
            .and_then(toml::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let default_effort = section
            .get("default_effort")
            .and_then(toml::Value::as_str)
            .unwrap_or_default();
        let mut note = Vec::new();
        if !real_model.is_empty() && real_model != alias {
            note.push(format!("真实模型 ID：{real_model}"));
        }
        if let Some(context_size) = context_size {
            note.push(format!("上下文：{context_size}"));
        }
        if !efforts.is_empty() {
            let default = if default_effort.is_empty() {
                String::new()
            } else {
                format!("，默认 {default_effort}")
            };
            note.push(format!("effort：{}{default}", efforts.join("/")));
        }
        output.push(detected_model(
            alias,
            "kimi",
            "",
            "Moonshot",
            "Kimi Code",
            if capabilities.contains(&"thinking") {
                "支持"
            } else {
                "未知"
            },
            source,
            &note.join("；"),
        ));
    }
    if output.is_empty() {
        return Err("未找到 [models.\"…\"] 段".to_string());
    }
    Ok(output)
}

fn detect_pi(binary: &Path) -> Result<(Vec<ResourceModel>, Vec<String>), String> {
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
        return Err(format!(
            "pi --list-models {}",
            process_error_summary(&listed)
        ));
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
    let mut models = parse_pi_sources(
        &contents,
        combined,
        &source_read("~/.pi/agent/models-store.json + pi --list-models"),
    )?;
    let providers = models
        .iter()
        .map(|model| model.provider.clone())
        .collect::<HashSet<_>>();
    let mut errors = Vec::new();
    for provider in providers {
        let auth = run_process(
            binary,
            &[
                "auth".to_string(),
                "check".to_string(),
                "--provider".to_string(),
                provider.clone(),
                "--json".to_string(),
            ],
            None,
            Duration::from_secs(20),
            None,
        );
        let ready = auth.as_ref().is_ok_and(pi_auth_ready);
        if !ready {
            let detail = auth
                .as_ref()
                .map_or_else(|error| error.clone(), process_error_summary);
            let message = format!("pi provider {provider} 未就绪：{detail}");
            errors.push(message);
            for model in models.iter_mut().filter(|model| model.provider == provider) {
                model.meta = append_note_marker(&model.meta, "鉴权未就绪");
            }
        }
    }
    Ok((models, errors))
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
                let tokens = line.split_whitespace().collect::<Vec<_>>();
                tokens.iter().any(|token| *token == qualified)
                    || tokens
                        .windows(2)
                        .any(|pair| pair[0] == effective_provider && pair[1] == model.id)
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
                "CLI 别名",
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
    meta: &str,
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
        meta: meta.to_string(),
        note: String::new(),
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
            model.note.clone_from(&existing.note);
            if let Some(failure) = existing
                .meta
                .split('；')
                .find(|part| part.trim().starts_with("冒烟失败："))
            {
                model.meta = append_note_marker(&model.meta, failure.trim());
            }
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
            existing.meta = append_note_marker(&existing.meta, MISSING_MARKER);
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

pub fn add_model(
    mut store: ResourceStore,
    request: ResourceModelRequest,
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
    if request.agent_id == "pi" && provider.is_empty() {
        return Err("手动输入失败：pi 模型必须填写 provider".to_string());
    }
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
        meta: String::new(),
        note: request.note.trim().to_string(),
    });
    Ok(store)
}

pub fn update_model(
    mut store: ResourceStore,
    request: ResourceModelRequest,
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
    smoke_one_in_place(&mut store, request, definitions, settings)?;
    Ok(store)
}

fn smoke_one_in_place(
    store: &mut ResourceStore,
    request: &ResourceModelKey,
    definitions: &[ToolDefinition],
    settings: &Settings,
) -> Result<(), String> {
    let model = find_model(store, &request.agent_id, &request.provider, &request.id)?.clone();
    let outcome = smoke_model(&model, definitions, settings);
    let target = find_model_mut(store, &request.agent_id, &request.provider, &request.id)?;
    match outcome {
        Ok(smoke_model) => {
            target.status = STATUS_CONNECTED.to_string();
            target.smoked_at = Local::now().to_rfc3339_opts(SecondsFormat::Secs, false);
            target.smoke_model = smoke_model;
            target.meta = remove_smoke_failure(&target.meta);
        }
        Err(error) => {
            target.status = STATUS_FAILED.to_string();
            target.smoked_at = Local::now().to_rfc3339_opts(SecondsFormat::Secs, false);
            target.smoke_model.clear();
            target.meta = set_smoke_failure(&target.meta, &safe_summary(&error));
        }
    }
    Ok(())
}

pub fn smoke_selected(
    mut store: ResourceStore,
    selection: &SmokeSelectionRequest,
    definitions: &[ToolDefinition],
    settings: &Settings,
) -> ResourceOperation {
    let keys = selected_model_keys(&store, selection);
    let mut errors = Vec::new();
    if keys.is_empty()
        && (!selection.agent_ids.is_empty() || !selection.cc_switch_providers.is_empty())
    {
        errors.push("所选资源没有可冒烟模型".to_string());
    }
    for key in keys {
        match smoke_one_in_place(&mut store, &key, definitions, settings) {
            Ok(()) => {
                if let Ok(model) = find_model(&store, &key.agent_id, &key.provider, &key.id) {
                    if model.status == STATUS_FAILED {
                        errors.push(format!(
                            "冒烟 {}/{} 失败：{}",
                            key.agent_id, key.id, model.meta
                        ));
                    }
                }
            }
            Err(error) => errors.push(error),
        }
    }
    let providers = match connection_overview(settings) {
        Ok(overview) => Some(overview.cc_switch_providers),
        Err(error) => {
            errors.push(format!("刷新 cc-switch provider 失败：{error}"));
            None
        }
    };
    ResourceOperation {
        store,
        errors,
        cc_switch_providers: providers,
    }
}

fn selected_model_keys(
    store: &ResourceStore,
    selection: &SmokeSelectionRequest,
) -> Vec<ResourceModelKey> {
    store
        .agents
        .iter()
        .filter(|agent| {
            selection.agent_ids.iter().any(|id| id == &agent.id)
                || (agent.id == "cc-switch" && !selection.cc_switch_providers.is_empty())
        })
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
        .collect()
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
            if model.provider.trim().is_empty() {
                return Err("pi 模型缺少 provider，无法冒烟".to_string());
            }
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
        Err(timeout_summary(result))
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
    values
        .iter()
        .any(|value| value.get("ready").and_then(Value::as_bool) == Some(true))
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

fn process_error_summary(result: &ProcessResult) -> String {
    if result.timed_out {
        return timeout_summary(result);
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

fn timeout_summary(result: &ProcessResult) -> String {
    if result.timeout.subsec_millis() == 0 {
        format!("{} 秒超时", result.timeout.as_secs())
    } else {
        format!("{} 毫秒超时", result.timeout.as_millis())
    }
}

fn safe_summary(text: &str) -> String {
    let mut value = text.replace(['\r', '\n'], " ");
    static SK_TOKEN: OnceLock<Regex> = OnceLock::new();
    static URL_CREDENTIALS: OnceLock<Regex> = OnceLock::new();
    static QUERY_SECRET: OnceLock<Regex> = OnceLock::new();
    static BEARER_TOKEN: OnceLock<Regex> = OnceLock::new();
    value = SK_TOKEN
        .get_or_init(|| Regex::new(r"sk-[A-Za-z0-9_-]{8,}").expect("valid token regex"))
        .replace_all(&value, "[已隐藏]")
        .into_owned();
    value = URL_CREDENTIALS
        .get_or_init(|| Regex::new(r"(?i)(https?://)[^/\s:@]+:[^/\s@]+@").expect("valid URL regex"))
        .replace_all(&value, "${1}[已隐藏]@")
        .into_owned();
    value = QUERY_SECRET
        .get_or_init(|| {
            Regex::new(r"(?i)([?&](?:key|token)=)[^&#\s]+").expect("valid query secret regex")
        })
        .replace_all(&value, "${1}[已隐藏]")
        .into_owned();
    value = BEARER_TOKEN
        .get_or_init(|| {
            Regex::new(r"(?i)\bBearer\s+[A-Za-z0-9._~+/=-]+").expect("valid bearer regex")
        })
        .replace_all(&value, "Bearer [已隐藏]")
        .into_owned();
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
        assert!(codex[0].meta.contains("effort"));
        assert!(codex[0].meta.contains("2026-08-30T00:00:00Z"));

        let kimi = parse_kimi_config(include_str!("../fixtures/kimi_config.toml"), "读:fixture")
            .expect("kimi fixture");
        let kimi_k3 = kimi
            .iter()
            .find(|model| model.id == "kimi-code/k3")
            .expect("k3 fixture model");
        assert!(kimi_k3.meta.contains("k3"));
        assert!(kimi_k3.meta.contains("1048576"));
        assert!(kimi_k3.meta.contains("low/high/max"));

        let pi = parse_pi_sources(
            include_str!("../fixtures/pi_models_store.json"),
            include_str!("../fixtures/pi_list_models.txt"),
            "读:fixture",
        )
        .expect("pi fixture");
        assert_eq!(pi.len(), 1);
        assert_eq!(pi[0].provider, "zai-coding-cn");
        assert_eq!(pi[0].vendor, "Zhipu");
        assert!(pi[0].meta.contains("204800"));

        let (cc_models, providers) = parse_cc_switch_list(
            include_str!("../fixtures/cc_switch_list.json"),
            "读:fixture",
        )
        .expect("cc-switch fixture");
        assert_eq!(cc_models.len(), 1);
        assert_eq!(providers[0].name, "Zhipu GLM");
        assert!(providers[0].has_token);
        assert!(parse_codex_cache(r#"{"models":[]}"#, "读:fixture").is_err());
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
                    meta: "旧元数据".to_string(),
                    note: "保留备注".to_string(),
                }],
            }],
        };
        let mut refreshed = fixture_model("old-model");
        refreshed.billing_source = "新源".to_string();
        refreshed.meta = "新元数据".to_string();
        merge_detected_models(
            &mut store,
            "codex",
            vec![refreshed, fixture_model("new-model")],
        );
        let old = find_model(&store, "codex", "", "old-model").expect("old");
        assert_eq!(old.tier, "高");
        assert_eq!(old.note, "保留备注");
        assert_eq!(old.meta, "新元数据");
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
            .meta
            .contains(MISSING_MARKER));
    }

    #[test]
    fn detection_refreshes_meta_preserves_note_and_parses_commented_kimi_toml() {
        let legacy: ResourceModel = serde_json::from_str(
            r#"{"id":"legacy","agent":"codex","provider":"","vendor":"OpenAI","billing_source":"官方订阅","tier":"","thinking":"未知","status":"待冒烟","source":"读:old","smoked_at":"","smoke_model":"","note":"旧备注"}"#,
        )
        .expect("legacy resource without meta");
        assert!(legacy.meta.is_empty());
        assert_eq!(legacy.note, "旧备注");

        let mut store = ResourceStore {
            version: 1,
            agents: vec![ResourceAgent {
                id: "codex".to_string(),
                kind: "内置".to_string(),
                detected: true,
                version: "test".to_string(),
                models: vec![ResourceModel {
                    id: "same".to_string(),
                    agent: "codex".to_string(),
                    provider: String::new(),
                    vendor: "OpenAI".to_string(),
                    billing_source: "官方订阅".to_string(),
                    tier: String::new(),
                    thinking: "未知".to_string(),
                    status: STATUS_PENDING.to_string(),
                    source: "读:old".to_string(),
                    smoked_at: String::new(),
                    smoke_model: String::new(),
                    meta: "priority：1".to_string(),
                    note: "用户备注".to_string(),
                }],
            }],
        };
        let mut refreshed = fixture_model("same");
        refreshed.meta = "priority：2".to_string();
        merge_detected_models(&mut store, "codex", vec![refreshed]);
        let model = find_model(&store, "codex", "", "same").expect("merged model");
        assert_eq!(model.meta, "priority：2");
        assert_eq!(model.note, "用户备注");

        let kimi = parse_kimi_config(
            include_str!("../fixtures/kimi_config_inline_comments.toml"),
            "读:fixture",
        )
        .expect("commented TOML fixture");
        assert_eq!(kimi[0].id, "kimi-code/commented");
        assert!(kimi[0].meta.contains("kimi-commented"));
        assert!(kimi[0].meta.contains("low/high"));
        assert_eq!(kimi[0].thinking, "支持");
    }

    #[test]
    fn pi_source_matching_uses_exact_tokens_and_auth_requires_top_level_ready() {
        let store = r#"{
          "p": {"models": [
            {"id":"glm-4","provider":"p"},
            {"id":"glm-4.7","provider":"p"}
          ]}
        }"#;
        let models = parse_pi_sources(store, "p glm-4.7", "读:fixture").expect("exact match");
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "glm-4.7");

        let result = |stdout: &str| ProcessResult {
            code: Some(0),
            stdout: stdout.to_string(),
            stderr: String::new(),
            timed_out: false,
            timeout: Duration::from_secs(20),
        };
        assert!(!pi_auth_ready(&result(
            r#"{"ready":false,"nested":{"ready":true}}"#
        )));
        assert!(!pi_auth_ready(&result(
            r#"{"status":"ready","nested":{"ok":true}}"#
        )));
        assert!(pi_auth_ready(&result(r#"{"ready":true}"#)));
    }

    #[test]
    fn provider_only_selection_includes_cc_switch_models() {
        let store = ResourceStore {
            version: 1,
            agents: vec![ResourceAgent {
                id: "cc-switch".to_string(),
                kind: "provider 桥".to_string(),
                detected: true,
                version: "test".to_string(),
                models: vec![detected_model(
                    "glm-4.7",
                    "cc-switch",
                    "zai",
                    "Zhipu",
                    "zai",
                    "未知",
                    "读:fixture",
                    "",
                )],
            }],
        };
        let keys = selected_model_keys(
            &store,
            &SmokeSelectionRequest {
                agent_ids: Vec::new(),
                cc_switch_providers: vec!["zai".to_string()],
            },
        );
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].agent_id, "cc-switch");
        assert_eq!(keys[0].provider, "zai");

        let pi_store = ResourceStore {
            version: 1,
            agents: vec![ResourceAgent {
                id: "pi".to_string(),
                kind: "内置".to_string(),
                detected: true,
                version: "test".to_string(),
                models: Vec::new(),
            }],
        };
        let error = add_model(
            pi_store,
            ResourceModelRequest {
                agent_id: "pi".to_string(),
                id: "glm-4.7".to_string(),
                provider: String::new(),
                vendor: "Zhipu".to_string(),
                tier: String::new(),
                note: String::new(),
            },
        )
        .expect_err("pi provider is required");
        assert!(error.contains("必须填写 provider"));
    }

    #[test]
    fn pi_smoke_rejects_identity_mismatch_and_thinking_only_answer() {
        let mismatch = ProcessResult {
            code: Some(1),
            stdout: include_str!("../fixtures/pi_smoke_mismatch.jsonl").to_string(),
            stderr: String::new(),
            timed_out: false,
            timeout: Duration::from_secs(60),
        };
        let error = parse_pi_smoke(&mismatch, "zai-coding-cn", "glm-4.7")
            .expect_err("identity mismatch must fail");
        assert!(error.contains("响应模型不符"));

        let thinking_only = ProcessResult {
            code: Some(0),
            stdout: include_str!("../fixtures/pi_smoke_thinking_only.jsonl").to_string(),
            stderr: String::new(),
            timed_out: false,
            timeout: Duration::from_secs(60),
        };
        let error = parse_pi_smoke(&thinking_only, "zai-coding-cn", "glm-4.7")
            .expect_err("thinking block must not count as answer");
        assert!(error.contains("没有 text 块"));

        let success = ProcessResult {
            code: Some(1),
            stdout: include_str!("../fixtures/pi_smoke_success.jsonl").to_string(),
            stderr: String::new(),
            timed_out: false,
            timeout: Duration::from_secs(60),
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
            timeout: Duration::from_secs(60),
        };
        assert_eq!(parse_claude_smoke(&claude).expect("claude"), "sonnet");

        let codex = ProcessResult {
            code: Some(0),
            stdout: "{\"type\":\"thread.started\"}\n{\"type\":\"turn.completed\"}\n".to_string(),
            stderr: String::new(),
            timed_out: false,
            timeout: Duration::from_secs(60),
        };
        assert!(parse_codex_smoke(&codex).is_ok());

        let kimi = ProcessResult {
            code: Some(1),
            stdout: "OK\n".to_string(),
            stderr: String::new(),
            timed_out: false,
            timeout: Duration::from_secs(60),
        };
        assert!(parse_kimi_smoke(&kimi).is_ok());

        assert!(cc_verification_passed(
            "glm-4.7 → glm-4.7 一致\n结论：全部一致，无静默降级。可放心使用。"
        ));
        assert!(!cc_verification_passed("结论：1 个静默降级、0 个错误。"));
    }

    #[test]
    fn safe_summary_redacts_common_cli_secret_shapes() {
        let summary = safe_summary(
            "sk-abcdefghijk https://user:pass@example.com/a?key=secret&token=other Bearer abc.def-123",
        );
        assert!(!summary.contains("sk-abcdefghijk"));
        assert!(!summary.contains("user:pass"));
        assert!(!summary.contains("key=secret"));
        assert!(!summary.contains("token=other"));
        assert!(!summary.contains("abc.def-123"));
        assert!(summary.contains("[已隐藏]"));

        let timeout = ProcessResult {
            code: None,
            stdout: String::new(),
            stderr: String::new(),
            timed_out: true,
            timeout: Duration::from_secs(20),
        };
        assert_eq!(process_error_summary(&timeout), "20 秒超时");
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
