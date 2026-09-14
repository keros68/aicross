//! 资源接入：探测本机 agent CLI 的可用模型，合并进 `~/.aicross/resources.json`，
//! 冒烟通过（status = 已接入）的模型才进入派发清单（`connected()`）。
//!
//! 数据契约沿用第 1 期（port/src-tauri-old/src/resources.rs）；冒烟与派发
//! 共用第 3 期 channels 的 argv 构造与真身核对，进程执行走 process.rs。
//! cc-switch 在本期降级为「发现提示」：只列出 cc-switch 里配置过的
//! provider，提示用户加进 pi，不再产生可派发条目。
//!
//! 密钥纪律：凭据只由子进程自己读（pi / claude 等 CLI 的内部配置），本模块
//! 不读 `~/.pi/auth.json`、`~/.cc-switch/*.db`；错误摘要一律过 `safe_summary`
//! 脱敏后才落盘或回传前端。

use crate::channels;
use crate::paths::aicross_root;
use crate::process::{run_process, version_for, ProcessResult};
use chrono::{Local, SecondsFormat};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;
use uuid::Uuid;

const STATUS_PENDING: &str = "待冒烟";
const STATUS_CONNECTED: &str = "已接入";
const STATUS_FAILED: &str = "冒烟失败";
const MISSING_MARKER: &str = "未在事实源中";
const SMOKE_PROMPT: &str = "只回复 OK";

/// 内置 agent：(id, 显示名)。二进制名与 id 相同。
const BUILTIN_AGENTS: [(&str, &str); 4] = [
    ("claude", "Claude Code"),
    ("codex", "Codex"),
    ("kimi", "Kimi Code CLI"),
    ("pi", "pi"),
];

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ResourceStore {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub agents: Vec<ResourceAgent>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ResourceAgent {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub detected: bool,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub models: Vec<ResourceModel>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ResourceModel {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub billing_source: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub thinking: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub smoked_at: String,
    #[serde(default)]
    pub smoke_model: String,
    #[serde(default)]
    pub smoke_failure: String,
    #[serde(default)]
    pub meta: String,
    #[serde(default)]
    pub note: String,
}

/// cc-switch 发现提示：只用于展示「cc-switch 里有 X，建议加进 pi」。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CcSwitchHint {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub has_token: bool,
    #[serde(default)]
    pub models: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ResourceOperation {
    pub store: ResourceStore,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub cc_switch_hints: Vec<CcSwitchHint>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ResourceModelRequest {
    pub agent_id: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ResourceModelKey {
    pub agent_id: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub provider: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct AddManualAgentRequest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub binary: String,
    #[serde(default)]
    pub args: Vec<String>,
}

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
    #[allow(dead_code)]
    app_type: String,
    #[serde(default)]
    tier_models: HashMap<String, String>,
    #[serde(default)]
    has_token: bool,
}

// ── 持久化 ────────────────────────────────────────────────────────────────

fn store_path() -> Result<PathBuf, String> {
    Ok(aicross_root()?.join("resources.json"))
}

/// 严格读取：文件不存在或解析失败都报错（orchestration 依赖这个语义区分
/// 「尚未接入」与「数据损坏」）。
pub fn load(path: &Path) -> Result<ResourceStore, String> {
    let text =
        fs::read_to_string(path).map_err(|error| format!("读取 resources.json 失败：{error}"))?;
    let store: ResourceStore =
        serde_json::from_str(&text).map_err(|error| format!("resources.json 解析失败：{error}"))?;
    if store.version != 1 {
        return Err(format!("resources.json 版本不受支持：{}", store.version));
    }
    Ok(store)
}

/// 命令层读取：文件不存在视为空仓库；存在但损坏则报错，绝不覆盖原文件。
fn load_or_default(path: &Path) -> Result<ResourceStore, String> {
    if path.is_file() {
        load(path)
    } else {
        Ok(ResourceStore {
            version: 1,
            agents: Vec::new(),
        })
    }
}

/// 原子写：同目录临时文件（flush + sync）后 rename。与 orchestration.rs 的
/// write_json_atomic 同一做法。注意：orchestration.rs 里有这份函数的整份
/// 复制（约 254 行），两边改动要互相同步；收敛成共享 util 留第 5 期
/// （orchestration.rs 的逻辑本期不动）。
fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let directory = path
        .parent()
        .ok_or_else(|| format!("写入 {} 失败：目标没有父目录", path.display()))?;
    fs::create_dir_all(directory)
        .map_err(|error| format!("创建目录 {} 失败：{error}", directory.display()))?;
    let contents =
        serde_json::to_string_pretty(value).map_err(|error| format!("生成 JSON 失败：{error}"))?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "data".into());
    let temporary = directory.join(format!(".{name}.{}.tmp", Uuid::new_v4()));
    let result = (|| {
        {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|e| format!("创建临时文件失败：{e}"))?;
            file.write_all(contents.as_bytes())
                .and_then(|_| file.sync_all())
                .map_err(|e| format!("写入临时文件失败：{e}"))?;
        }
        match fs::rename(&temporary, path) {
            Ok(()) => Ok(()),
            Err(first) => {
                let _ = fs::remove_file(path);
                fs::rename(&temporary, path)
                    .map_err(|e| format!("替换 {} 失败：{first}；重试失败：{e}", path.display()))
            }
        }
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn save_store(path: &Path, store: &ResourceStore) -> Result<(), String> {
    write_json_atomic(path, store)
}

/// 进程级写锁：所有 load → 改 → save 序列都必须持有它，防止并发命令的
/// 读改写互相覆盖（前端按钮 busy 只是 UI 纪律，不是后端保证）。
fn store_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 派发清单：已接入模型。cc-switch 在本期不再是可派发通道，一律排除。
pub fn connected(path: &Path) -> Result<Vec<ResourceModel>, String> {
    Ok(load(path)?
        .agents
        .into_iter()
        .flat_map(|agent| agent.models)
        .filter(|model| model.status == STATUS_CONNECTED && model.agent != "cc-switch")
        .collect())
}

/// 保证内置四 agent 骨架存在；迁移掉历史 store 里的 cc-switch agent
/// （已降级为发现提示）；回填 model.agent 冗余字段。
pub fn ensure_agents(store: &mut ResourceStore) {
    store.version = 1;
    store.agents.retain(|agent| agent.id != "cc-switch");
    for (id, name) in BUILTIN_AGENTS {
        if store.agents.iter().all(|agent| agent.id != id) {
            store.agents.push(ResourceAgent {
                id: id.to_string(),
                name: name.to_string(),
                kind: "内置".to_string(),
                detected: false,
                version: "未检测".to_string(),
                models: Vec::new(),
            });
        }
        if let Some(agent) = store.agents.iter_mut().find(|agent| agent.id == id) {
            agent.name = name.to_string();
            if agent.kind.trim().is_empty() {
                agent.kind = "内置".to_string();
            }
        }
    }
    for agent in &mut store.agents {
        for model in &mut agent.models {
            model.agent.clone_from(&agent.id);
        }
    }
}

// ── 二进制解析 ────────────────────────────────────────────────────────────

/// 本地版 resolve_binary（channels 里同名函数是私有的，channels 不许改）：
/// Windows 依次 where.exe name.exe/name.cmd/name.bat 取首行，Unix 用 which。
/// 已知漂移：kimi 若在 PATH 里解析到 .cmd/.bat 包装，探测（--version 经
/// cmd 包装能跑）会过，但冒烟/派发走 channels::build_invocation 会拒
/// （「Kimi 需要原生 kimi.exe」）——同一个 agent「探测通过、冒烟必败」。
/// 缓解：kimi 先查 ~/.kimi-code/bin/kimi.exe（与 channels 的 fallback
/// 同路径），命中即用原生 exe；查不到再退 PATH。channels 解禁后应把
/// resolve_binary 提为 pub(crate)，收敛掉这份重复。
fn resolve_cli(name: &str) -> Option<PathBuf> {
    let path = Path::new(name);
    if path.components().count() > 1 && path.is_file() {
        return Some(path.to_path_buf());
    }
    if name == "kimi" {
        if let Some(fallback) =
            dirs::home_dir().map(|home| home.join(".kimi-code").join("bin").join("kimi.exe"))
        {
            if fallback.is_file() {
                return Some(fallback);
            }
        }
    }
    let lookup = if cfg!(windows) { "where.exe" } else { "which" };
    #[cfg(windows)]
    let names = [
        format!("{name}.exe"),
        format!("{name}.cmd"),
        format!("{name}.bat"),
    ];
    #[cfg(not(windows))]
    let names = [name.to_string()];
    for binary in names {
        let mut command = Command::new(lookup);
        command
            .arg(&binary)
            .env("AI_CROSS_PEER", "1")
            .stdin(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        if let Ok(output) = command.output() {
            if output.status.success() {
                if let Some(found) = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .find(|line| !line.trim().is_empty())
                {
                    return Some(PathBuf::from(found.trim()));
                }
            }
        }
    }
    None
}

fn detect_version(binary: Option<&PathBuf>) -> (bool, String) {
    match binary {
        Some(path) => (
            true,
            version_for(path).unwrap_or_else(|| "已检测".to_string()),
        ),
        None => (false, "未检测".to_string()),
    }
}

// ── 探测 ──────────────────────────────────────────────────────────────────

fn detect_into(
    store: &mut ResourceStore,
    agent_id: &str,
    errors: &mut Vec<String>,
) {
    if store.agents.iter().all(|agent| agent.id != agent_id) {
        errors.push(format!("探测资源失败：未知 agent：{agent_id}"));
        return;
    }

    // 手动 agent：二进制来自 tools.json 里的手动配置，没有事实源可读。
    if agent_id.starts_with("manual-") {
        let entry = crate::tool_config::get(agent_id);
        let (binary, _) = crate::tool_config::parse_command(&entry.command);
        let resolved = binary.as_deref().and_then(resolve_cli);
        let (detected, version) = detect_version(resolved.as_ref());
        set_agent_detection(store, agent_id, detected, &version);
        if !detected {
            errors.push(format!(
                "探测 {agent_id} 失败：未检测到 {}",
                binary.as_deref().unwrap_or("（未配置二进制）")
            ));
        }
        return;
    }

    let resolved = resolve_cli(agent_id);
    let (detected, version) = detect_version(resolved.as_ref());
    set_agent_detection(store, agent_id, detected, &version);
    if !detected {
        errors.push(format!("探测 {agent_id} 失败：未检测到 {agent_id}"));
        if agent_id == "claude" {
            // claude 的模型是静态预设，未检测时没有事实源可读；仍走一次
            // 空合并，只给既有条目刷「未在事实源中」标记（与 codex/kimi
            // 走同一条合并路径），不新增预设条目。
            merge_detected_models(store, agent_id, Vec::new());
            return;
        }
    }

    if agent_id == "pi" {
        let Some(binary) = resolved else {
            return;
        };
        match detect_pi(&binary) {
            Ok((models, provider_errors)) => {
                merge_detected_models(store, agent_id, models);
                errors.extend(provider_errors);
            }
            Err(error) => errors.push(format!("探测 {agent_id} 失败：{error}")),
        }
        return;
    }

    let models = match agent_id {
        "claude" => Ok(claude_presets(resolved.as_ref())),
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

/// kimi 事实源：~/.kimi-code/config.toml 的 [models."别名"] 段。
/// 模型 id 保留完整别名（含 `kimi-code/` 前缀）——channels 要求 kimi 模型
/// 必须含 `/`，截断会让派发直接报错。
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
    // --list-models 是运行时已鉴权清单，为主事实源；models-store.json 可能
    // 是旧清单，只用来补 contextWindow/reasoning 等元数据，读不到不拦截探测。
    let contents = home_file(&[".pi", "agent", "models-store.json"])
        .and_then(|path| {
            fs::read_to_string(&path).map_err(|error| format!("读取 {} 失败：{error}", path.display()))
        })
        .unwrap_or_else(|_| "{}".to_string());
    let mut models = parse_pi_sources(
        &contents,
        combined,
        &source_read("pi --list-models（models-store.json 补元数据）"),
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
        let status = auth.as_ref().ok().and_then(pi_auth_status);
        let ready = status.as_deref() == Some("ready");
        if !ready {
            let detail = auth
                .as_ref()
                .map_or_else(|error| error.clone(), process_error_summary);
            let message = format!("pi provider {provider} 未就绪：{detail}");
            errors.push(message);
            mark_pi_auth_not_ready(&mut models, &provider, status.as_deref());
        }
    }
    Ok((models, errors))
}

fn mark_pi_auth_not_ready(models: &mut [ResourceModel], provider: &str, status: Option<&str>) {
    for model in models.iter_mut().filter(|model| model.provider == provider) {
        model.meta = append_note_marker(&model.meta, "鉴权未就绪");
        if let Some(status) = status {
            model.meta = append_note_marker(&model.meta, &format!("鉴权状态：{status}"));
        }
    }
}

/// pi 事实源合并：`pi --list-models` 为主（运行时已鉴权清单），
/// models-store.json 只用来补 contextWindow / reasoning 等元数据；
/// store 里没有的模型照样收入，store 里多出但已不在清单里的不收入
/// （消失条目的「未在事实源中」标记由合并层处理）。
fn parse_pi_sources(
    contents: &str,
    listed: &str,
    source: &str,
) -> Result<Vec<ResourceModel>, String> {
    // store 只是元数据补充：文件损坏也按空处理，不让旧清单拦截探测。
    let store: HashMap<String, PiStoreProvider> =
        serde_json::from_str(contents).unwrap_or_default();
    let mut meta_by_key: HashMap<(String, String), &PiStoreModel> = HashMap::new();
    for (provider_name, provider) in &store {
        for model in &provider.models {
            let effective_provider = if model.provider.trim().is_empty() {
                provider_name.clone()
            } else {
                model.provider.clone()
            };
            meta_by_key.insert((effective_provider, model.id.clone()), model);
        }
    }
    let mut output = Vec::new();
    let mut seen = HashSet::new();
    for (provider, id) in parse_pi_list_models(listed) {
        if !seen.insert((provider.clone(), id.clone())) {
            continue;
        }
        let meta = meta_by_key.get(&(provider.clone(), id.clone()));
        output.push(detected_model(
            &id,
            "pi",
            &provider,
            infer_vendor(&id),
            &provider,
            match meta {
                Some(model) if model.reasoning => "支持",
                _ => "未知",
            },
            source,
            &meta
                .and_then(|model| model.context_window)
                .map(|value| format!("上下文：{value}"))
                .unwrap_or_default(),
        ));
    }
    if output.is_empty() {
        return Err("pi --list-models 未返回可解析的模型行".to_string());
    }
    Ok(output)
}

/// 解析 `pi --list-models` 输出：每行 `provider model [context]`（表格式，
/// 首行表头跳过）或单 token `provider/model`。整 token 精确取，不切子串。
fn parse_pi_list_models(listed: &str) -> Vec<(String, String)> {
    let mut output = Vec::new();
    for line in listed.lines() {
        let tokens = line.split_whitespace().collect::<Vec<_>>();
        match tokens.as_slice() {
            [single] => {
                if let Some((provider, id)) = single.split_once('/') {
                    if !provider.is_empty() && !id.is_empty() {
                        output.push((provider.to_string(), id.to_string()));
                    }
                }
            }
            [provider, id, ..] => {
                if *provider == "provider" && *id == "model" {
                    continue; // 表头行
                }
                output.push((provider.to_string(), id.to_string()));
            }
            [] => {}
        }
    }
    output
}

/// claude 没有本地模型清单文件，事实源是 `claude --help` 里 `--model` 那一段的
/// 别名举例。抓不到时的兜底表（写表日期见 CLAUDE_STATIC_TABLE_DATE）。
/// `haiku` 一直不在 `--help` 的举例里但确实可用，所以无论走哪条路都补上。
const CLAUDE_FALLBACK_ALIASES: [&str; 4] = ["fable", "opus", "sonnet", "haiku"];
const CLAUDE_STATIC_TABLE_DATE: &str = "2026-09-14";

fn claude_presets(binary: Option<&PathBuf>) -> Vec<ResourceModel> {
    let (aliases, source) = claude_aliases(binary);
    aliases
        .into_iter()
        .map(|id| {
            detected_model(
                &id,
                "claude",
                "",
                "Anthropic",
                "官方订阅",
                "未知",
                &source,
                "CLI 别名",
            )
        })
        .collect()
}

/// 跑一次 `claude --help`，抓 `--model` 段里的别名；进程失败/超时/抓不到都退静态表。
fn claude_aliases(binary: Option<&PathBuf>) -> (Vec<String>, String) {
    if let Some(path) = binary {
        if let Ok(output) = run_process(
            path,
            &["--help".to_string()],
            None,
            Duration::from_secs(20),
            None,
        ) {
            if !output.timed_out {
                let (aliases, from_help) = claude_aliases_from_help(&output.stdout);
                if from_help {
                    return (
                        aliases,
                        format!("claude --help 别名 ({})", Local::now().format("%Y-%m-%d")),
                    );
                }
            }
        }
    }
    (
        CLAUDE_FALLBACK_ALIASES.iter().map(|a| a.to_string()).collect(),
        format!("静态表({CLAUDE_STATIC_TABLE_DATE})"),
    )
}

/// 返回 (别名表, 是否真从 --help 抓到)。抓不到就是静态表原样。
fn claude_aliases_from_help(help: &str) -> (Vec<String>, bool) {
    let mut aliases = parse_claude_model_aliases(help);
    if aliases.is_empty() {
        return (
            CLAUDE_FALLBACK_ALIASES.iter().map(|a| a.to_string()).collect(),
            false,
        );
    }
    if !aliases.iter().any(|alias| alias == "haiku") {
        aliases.push("haiku".to_string());
    }
    (aliases, true)
}

/// 从 `--model <model>` 那一段里抓所有单引号里的别名。同一段里还有完整模型名的
/// 举例（`'claude-fable-5'`），用「纯小写字母、2..=16 字符」把带连字符/数字的
/// 完整名滤掉——别名只可能是 fable/opus/sonnet/haiku 这种单词。
fn parse_claude_model_aliases(help: &str) -> Vec<String> {
    let Some(section) = claude_model_help_section(help) else {
        return Vec::new();
    };
    let quoted = Regex::new(r"'([^']+)'").expect("别名正则是常量");
    let mut aliases: Vec<String> = Vec::new();
    for capture in quoted.captures_iter(&section) {
        let alias = capture[1].trim();
        let is_alias = (2..=16).contains(&alias.len())
            && alias.chars().all(|c| c.is_ascii_lowercase());
        if is_alias && !aliases.iter().any(|kept| kept == alias) {
            aliases.push(alias.to_string());
        }
    }
    aliases
}

/// `--model` 的说明在 help 里换行续排（后续行缩进更深），一直到下一个选项行为止。
/// 把整段折成一行返回，免得引号被换行切开。
fn claude_model_help_section(help: &str) -> Option<String> {
    let mut lines = help.lines();
    let start = lines.by_ref().find(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("--model ") || trimmed.starts_with("--model\t")
    })?;
    let indent = start.len() - start.trim_start().len();
    let mut section = start.trim().to_string();
    for line in lines {
        let trimmed = line.trim_start();
        let line_indent = line.len() - trimmed.len();
        // 空行或回到同级/更浅缩进的下一个选项 → 段结束
        if trimmed.is_empty() || (line_indent <= indent && trimmed.starts_with('-')) {
            break;
        }
        section.push(' ');
        section.push_str(trimmed);
    }
    Some(section)
}

// ── cc-switch 发现提示 ────────────────────────────────────────────────────

/// cc-switch 只读 `cc_switch.py list` 输出（脚本自己读库），产生发现提示；
/// 不再产生可派发条目。path 为空或脚本不存在时返回空 Vec（跳过，不报错）。
fn detect_cc_switch_hints(cc_switch_path: Option<&str>) -> (Vec<CcSwitchHint>, Option<String>) {
    let Some(script) = cc_switch_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    else {
        return (Vec::new(), None);
    };
    if !script.is_file() {
        return (
            Vec::new(),
            Some(format!("cc_switch.py 路径不存在：{}", script.display())),
        );
    }
    let Some(python) = resolve_cli("python") else {
        return (Vec::new(), Some("未检测到 Python，跳过 cc-switch 提示".to_string()));
    };
    let result = run_process(
        &python,
        &[script.to_string_lossy().to_string(), "list".to_string()],
        None,
        Duration::from_secs(20),
        None,
    );
    match result {
        Ok(output) if !output.timed_out && output.code == Some(0) => {
            match parse_cc_switch_hints(&output.stdout) {
                Ok(hints) => (hints, None),
                Err(error) => (Vec::new(), Some(error)),
            }
        }
        Ok(output) => (
            Vec::new(),
            Some(format!("cc_switch.py list 失败：{}", process_error_summary(&output))),
        ),
        Err(error) => (Vec::new(), Some(format!("cc_switch.py list 失败：{error}"))),
    }
}

fn parse_cc_switch_hints(contents: &str) -> Result<Vec<CcSwitchHint>, String> {
    let raw: Vec<RawCcProvider> = serde_json::from_str(contents)
        .map_err(|error| format!("解析 cc_switch.py list 输出失败：{error}"))?;
    let mut hints = Vec::new();
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
            hints.push(CcSwitchHint {
                name: provider.name.clone(),
                has_token: provider.has_token,
                models: ids,
            });
        }
    }
    Ok(hints)
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
        smoke_failure: String::new(),
        meta: meta.to_string(),
        note: String::new(),
    }
}

fn model_key(model: &ResourceModel) -> (&str, &str, &str) {
    (&model.agent, &model.provider, &model.id)
}

/// 合并事实源条目：已存在的保留用户字段（tier/status/冒烟结果/vendor/note），
/// 新条目进「待冒烟」；消失的标「未在事实源中」（申报条目不标）。
fn merge_detected_models(store: &mut ResourceStore, agent_id: &str, discovered: Vec<ResourceModel>) {
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
            model.smoke_failure.clone_from(&existing.smoke_failure);
            if !existing.vendor.trim().is_empty() {
                model.vendor.clone_from(&existing.vendor);
            }
            model.note.clone_from(&existing.note);
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

// ── 手动录入与编辑 ────────────────────────────────────────────────────────

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
        smoke_failure: String::new(),
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

// ── 冒烟 ──────────────────────────────────────────────────────────────────

/// 冒烟成功的结果：响应里的模型真身（identity）+ 需要留进 meta 的说明
/// （如 claude 官方端点未做真身比对）。
struct SmokeSuccess {
    identity: String,
    meta_marker: Option<String>,
}

/// claude 是否走第三方端点：进程环境 ANTHROPIC_BASE_URL，或
/// ~/.claude/settings.json 的 env.ANTHROPIC_BASE_URL（用户常在 settings
/// 里配置，不经过控制台进程环境，只看进程环境会漏判）。
fn claude_third_party_endpoint() -> bool {
    let from_env = std::env::var("ANTHROPIC_BASE_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    if from_env {
        return true;
    }
    home_file(&[".claude", "settings.json"])
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|contents| claude_base_url_from_settings(&contents))
        .map(|url| !url.trim().is_empty())
        .unwrap_or(false)
}

fn claude_base_url_from_settings(contents: &str) -> Option<String> {
    let value: Value = serde_json::from_str(contents).ok()?;
    value
        .get("env")?
        .get("ANTHROPIC_BASE_URL")?
        .as_str()
        .map(str::to_string)
}

/// 冒烟：argv 与真身核对完全复用 channels（盲跑 blind=true，prompt 固定
/// 「只回复 OK」），进程执行走 process.rs。成功返回响应里的模型真身
/// （identity），失败返回脱敏后的原因。
fn run_smoke(model: &ResourceModel) -> Result<SmokeSuccess, String> {
    let session_dir = aicross_root().ok().map(|root| root.join("sessions").join("smoke"));
    if let Some(dir) = &session_dir {
        let _ = fs::create_dir_all(dir);
    }
    let invocation = channels::build_invocation(
        &model.agent,
        &model.provider,
        &model.id,
        "",
        "低",
        true,
        None,
        SMOKE_PROMPT,
        None,
        session_dir.as_deref(),
        None,
        false, // 冒烟只读，不给写工具（第 4 期新增 writable 参数）
    )?;
    let temp = std::env::temp_dir();
    let result = run_process(
        &invocation.program,
        &invocation.args,
        invocation.input.as_deref(),
        Duration::from_secs(60),
        Some(&temp),
    )?;
    match channels::parse_output(&model.agent, &model.provider, &model.id, &result) {
        Ok(output) => {
            // claude 走第三方端点（进程环境或 ~/.claude/settings.json 的
            // ANTHROPIC_BASE_URL 非空）时比对响应体 model：真身与请求别名
            // 不相干即判不符（别名兼容：包含即算过）。两处都没配即官方端点：
            // 没有比对的必要，但要在 meta 留痕，不静默跳过。
            if model.agent == "claude" {
                if claude_third_party_endpoint() {
                    if !output.identity.is_empty() && !output.identity.contains(&model.id) {
                        return Err(format!(
                            "响应模型不符：请求 {}，响应 {}",
                            model.id, output.identity
                        ));
                    }
                } else {
                    return Ok(SmokeSuccess {
                        identity: output.identity,
                        meta_marker: Some("未核对真身（官方端点）".to_string()),
                    });
                }
            }
            Ok(SmokeSuccess {
                identity: output.identity,
                meta_marker: None,
            })
        }
        Err(error) => Err(error.message),
    }
}

/// 把一次冒烟结果写进 store：成功 → 已接入 + 真身 + 可选 meta 标记；
/// 失败 → 冒烟失败 + 脱敏原因。
fn apply_smoke_outcome(
    store: &mut ResourceStore,
    request: &ResourceModelKey,
    outcome: Result<SmokeSuccess, String>,
) -> Result<(), String> {
    let target = find_model_mut(store, &request.agent_id, &request.provider, &request.id)?;
    match outcome {
        Ok(success) => {
            target.status = STATUS_CONNECTED.to_string();
            target.smoked_at = Local::now().to_rfc3339_opts(SecondsFormat::Secs, false);
            target.smoke_model = success.identity;
            target.smoke_failure.clear();
            if let Some(marker) = success.meta_marker {
                target.meta = append_note_marker(&target.meta, &marker);
            }
        }
        Err(error) => {
            target.status = STATUS_FAILED.to_string();
            target.smoked_at = Local::now().to_rfc3339_opts(SecondsFormat::Secs, false);
            target.smoke_model.clear();
            target.smoke_failure = safe_summary(&error);
        }
    }
    Ok(())
}

fn smoke_one_in_place(store: &mut ResourceStore, request: &ResourceModelKey) -> Result<(), String> {
    let model = find_model(store, &request.agent_id, &request.provider, &request.id)?.clone();
    let outcome = run_smoke(&model);
    apply_smoke_outcome(store, request, outcome)
}

// ── 杂项 ──────────────────────────────────────────────────────────────────

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

fn json_lines(text: &str) -> impl Iterator<Item = Value> + '_ {
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
}

fn pi_auth_status(result: &ProcessResult) -> Option<String> {
    if result.timed_out {
        return None;
    }
    let values = if let Ok(value) = serde_json::from_str::<Value>(result.stdout.trim()) {
        vec![value]
    } else {
        json_lines(&result.stdout).collect::<Vec<_>>()
    };
    values
        .into_iter()
        .find_map(|value| match value.get("status") {
            Some(Value::String(status)) => Some(status.clone()),
            Some(status) => Some(status.to_string()),
            None => None,
        })
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

/// 错误摘要脱敏：sk- token、URL 内嵌凭据、query secret、Bearer、以及当前
/// 进程环境里名字像密钥的变量值，全部替换为 [已隐藏]，截 200 字。
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
            Regex::new(r"(?i)([?&](?:key|token|api[-_]?key|access_token|secret)=)[^&#\s]+")
                .expect("valid query secret regex")
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

// ── Tauri 命令 ────────────────────────────────────────────────────────────
// 所有写命令都按「持 store_lock() → load → 改 → 原子写」的序列执行，
// 锁保证并发命令不互相覆盖（审查 2.4）。命令体拆成 *_at(path, …) 自由函数，
// 方便测试用临时目录钉住行为。

fn get_resources_at(path: &Path) -> Result<ResourceStore, String> {
    let mut store = load_or_default(path)?;
    ensure_agents(&mut store);
    Ok(store)
}

fn detect_resource_agent_at(
    path: &Path,
    agent_id: &str,
    cc_switch_path: Option<&str>,
) -> Result<ResourceOperation, String> {
    let _guard = store_lock();
    let mut store = load_or_default(path)?;
    ensure_agents(&mut store);
    let mut errors = Vec::new();
    detect_into(&mut store, agent_id, &mut errors);
    let (cc_switch_hints, hint_error) = detect_cc_switch_hints(cc_switch_path);
    if let Some(error) = hint_error {
        errors.push(error);
    }
    save_store(path, &store)?;
    Ok(ResourceOperation {
        store,
        errors,
        cc_switch_hints,
    })
}

fn detect_all_resources_at(
    path: &Path,
    cc_switch_path: Option<&str>,
) -> Result<ResourceOperation, String> {
    let _guard = store_lock();
    let mut store = load_or_default(path)?;
    ensure_agents(&mut store);
    let ids = store
        .agents
        .iter()
        .map(|agent| agent.id.clone())
        .collect::<Vec<_>>();
    let mut errors = Vec::new();
    for agent_id in ids {
        detect_into(&mut store, &agent_id, &mut errors);
    }
    let (cc_switch_hints, hint_error) = detect_cc_switch_hints(cc_switch_path);
    if let Some(error) = hint_error {
        errors.push(error);
    }
    save_store(path, &store)?;
    Ok(ResourceOperation {
        store,
        errors,
        cc_switch_hints,
    })
}

fn add_resource_model_at(
    path: &Path,
    request: ResourceModelRequest,
) -> Result<ResourceStore, String> {
    let _guard = store_lock();
    let mut store = load_or_default(path)?;
    ensure_agents(&mut store);
    let store = add_model(std::mem::take(&mut store), request)?;
    save_store(path, &store)?;
    Ok(store)
}

fn update_resource_model_at(
    path: &Path,
    request: ResourceModelRequest,
) -> Result<ResourceStore, String> {
    let _guard = store_lock();
    let store = update_model(load_or_default(path)?, request)?;
    save_store(path, &store)?;
    Ok(store)
}

fn remove_resource_model_at(path: &Path, request: ResourceModelKey) -> Result<ResourceStore, String> {
    let _guard = store_lock();
    let store = remove_model(load_or_default(path)?, request)?;
    save_store(path, &store)?;
    Ok(store)
}

fn smoke_resource_model_at(path: &Path, request: ResourceModelKey) -> Result<ResourceStore, String> {
    let _guard = store_lock();
    let mut store = load_or_default(path)?;
    smoke_one_in_place(&mut store, &request)?;
    save_store(path, &store)?;
    Ok(store)
}

/// 冒烟某个 agent 的全部模型；失败汇总进 errors（单条失败不中断整批）。
/// 每冒烟一条都重新 load → 改 → 写盘，不整批长持旧 store——一次冒烟最长
/// 60s，整批持旧数据会把期间其他写方的更新整体抹掉（审查 2.4）。
fn smoke_agent_models_at(
    path: &Path,
    agent_id: &str,
    mut smoke: impl FnMut(&ResourceModel) -> Result<SmokeSuccess, String>,
) -> Result<ResourceOperation, String> {
    let keys = {
        let _guard = store_lock();
        let mut store = load_or_default(path)?;
        ensure_agents(&mut store);
        store
            .agents
            .iter()
            .find(|agent| agent.id == agent_id)
            .map(|agent| {
                agent
                    .models
                    .iter()
                    .map(|model| ResourceModelKey {
                        agent_id: agent_id.to_string(),
                        id: model.id.clone(),
                        provider: model.provider.clone(),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    let mut errors = Vec::new();
    let mut latest = ResourceStore::default();
    for key in &keys {
        let _guard = store_lock();
        let mut store = load_or_default(path)?;
        match find_model(&store, &key.agent_id, &key.provider, &key.id) {
            Ok(model) => {
                let outcome = smoke(&model.clone());
                apply_smoke_outcome(&mut store, key, outcome)?;
                save_store(path, &store)?;
                let model = find_model(&store, &key.agent_id, &key.provider, &key.id)?;
                if model.status == STATUS_FAILED {
                    errors.push(format!(
                        "冒烟 {}/{} 失败：{}",
                        key.agent_id, key.id, model.smoke_failure
                    ));
                }
            }
            Err(error) => errors.push(error),
        }
        latest = store;
    }
    Ok(ResourceOperation {
        store: latest,
        errors,
        cc_switch_hints: Vec::new(),
    })
}

#[tauri::command]
pub fn get_resources() -> Result<ResourceStore, String> {
    get_resources_at(&store_path()?)
}

#[tauri::command]
pub async fn detect_resource_agent(
    agent_id: String,
    cc_switch_path: Option<String>,
) -> Result<ResourceOperation, String> {
    tauri::async_runtime::spawn_blocking(move || {
        detect_resource_agent_at(&store_path()?, &agent_id, cc_switch_path.as_deref())
    })
    .await
    .map_err(|error| format!("探测任务失败：{error}"))?
}

#[tauri::command]
pub async fn detect_all_resources(
    cc_switch_path: Option<String>,
) -> Result<ResourceOperation, String> {
    tauri::async_runtime::spawn_blocking(move || {
        detect_all_resources_at(&store_path()?, cc_switch_path.as_deref())
    })
    .await
    .map_err(|error| format!("探测任务失败：{error}"))?
}

#[tauri::command]
pub fn add_resource_model(request: ResourceModelRequest) -> Result<ResourceStore, String> {
    add_resource_model_at(&store_path()?, request)
}

#[tauri::command]
pub fn update_resource_model(request: ResourceModelRequest) -> Result<ResourceStore, String> {
    update_resource_model_at(&store_path()?, request)
}

#[tauri::command]
pub fn remove_resource_model(request: ResourceModelKey) -> Result<ResourceStore, String> {
    remove_resource_model_at(&store_path()?, request)
}

/// 手动 agent：二进制与启动参数存 ~/.aicross/tools.json（key = manual-<uuid>），
/// 名字存 resources.json。登记后立即做一次探测，探测错误随 errors 返回前端。
#[tauri::command]
pub async fn add_manual_agent(request: AddManualAgentRequest) -> Result<ResourceOperation, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let name = request.name.trim();
        let binary = request.binary.trim();
        if name.is_empty() || binary.is_empty() {
            return Err("手动添加失败：名称和二进制名不能为空".to_string());
        }
        let id = format!("manual-{}", Uuid::new_v4());
        crate::tool_config::set(
            &id,
            crate::tool_config::ToolConfigEntry {
                command: binary.to_string(),
                extra_args: request.args.clone(),
                ..Default::default()
            },
        )
        .map_err(|error| format!("保存手动 agent 配置失败：{error}"))?;
        let path = store_path()?;
        let _guard = store_lock();
        let mut store = load_or_default(&path)?;
        ensure_agents(&mut store);
        store.agents.push(ResourceAgent {
            id: id.clone(),
            name: name.to_string(),
            kind: "手动".to_string(),
            detected: false,
            version: "未检测".to_string(),
            models: Vec::new(),
        });
        let mut errors = Vec::new();
        detect_into(&mut store, &id, &mut errors);
        save_store(&path, &store)?;
        Ok(ResourceOperation {
            store,
            errors,
            cc_switch_hints: Vec::new(),
        })
    })
    .await
    .map_err(|error| format!("手动添加任务失败：{error}"))?
}

#[tauri::command]
pub async fn smoke_resource_model(request: ResourceModelKey) -> Result<ResourceStore, String> {
    tauri::async_runtime::spawn_blocking(move || smoke_resource_model_at(&store_path()?, request))
        .await
        .map_err(|error| format!("冒烟任务失败：{error}"))?
}

#[tauri::command]
pub async fn smoke_agent_models(agent_id: String) -> Result<ResourceOperation, String> {
    tauri::async_runtime::spawn_blocking(move || {
        smoke_agent_models_at(&store_path()?, &agent_id, run_smoke)
    })
    .await
    .map_err(|error| format!("冒烟任务失败：{error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_model(id: &str) -> ResourceModel {
        detected_model(id, "codex", "", "OpenAI", "官方订阅", "支持", "读:fixture", "")
    }

    fn fixture_agent(id: &str, models: Vec<ResourceModel>) -> ResourceAgent {
        ResourceAgent {
            id: id.to_string(),
            name: id.to_string(),
            kind: "内置".to_string(),
            detected: true,
            version: "test".to_string(),
            models,
        }
    }

    fn process_result(code: Option<i32>, stdout: &str) -> ProcessResult {
        ProcessResult {
            code,
            stdout: stdout.to_string(),
            stderr: String::new(),
            timed_out: false,
            truncated: false,
            timeout: Duration::from_secs(20),
        }
    }

    /// 本机 claude 2.1.270 `--help` 里 `--model` 那一段的原样（含换行续排，
    /// 以及同段里那个会误伤的完整模型名举例 'claude-fable-5'）。
    const CLAUDE_HELP_FIXTURE: &str = concat!(
        "  --mcp-config <configs...>             Load MCP servers from JSON files\n",
        "  --model <model>                       Model for the current session. Provide\n",
        "                                        an alias for the latest model (e.g.\n",
        "                                        'fable', 'opus', or 'sonnet') or a\n",
        "                                        model's full name (e.g.\n",
        "                                        'claude-fable-5').\n",
        "  -n, --name <name>                     Set a display name for this session\n",
    );

    #[test]
    fn claude_help_yields_fable_opus_sonnet_plus_haiku() {
        let aliases = parse_claude_model_aliases(CLAUDE_HELP_FIXTURE);
        assert_eq!(aliases, vec!["fable", "opus", "sonnet"]);

        let (list, from_help) = claude_aliases_from_help(CLAUDE_HELP_FIXTURE);
        assert!(from_help);
        // haiku 不在 --help 举例里，按约定补在末尾
        assert_eq!(list, vec!["fable", "opus", "sonnet", "haiku"]);
        // 同段里的完整模型名举例不能被当成别名
        assert!(!list.iter().any(|alias| alias.contains('-')));
        // 下一个选项（-n, --name）的说明不能被卷进来
        assert!(!list.iter().any(|alias| alias == "name"));
    }

    #[test]
    fn claude_help_without_model_section_falls_back_to_static_table() {
        for help in ["", "Usage: claude [options]\n  --version  print version\n"] {
            assert!(parse_claude_model_aliases(help).is_empty());
            let (list, from_help) = claude_aliases_from_help(help);
            assert!(!from_help);
            assert_eq!(list, vec!["fable", "opus", "sonnet", "haiku"]);
        }
    }

    #[test]
    fn parses_four_local_fact_sources() {
        let codex = parse_codex_cache(
            include_str!("../fixtures/resources/codex_models.json"),
            "读:fixture",
        )
        .expect("codex fixture");
        assert_eq!(codex[0].id, "gpt-5.6-terra");
        assert_eq!(codex[0].thinking, "支持");
        assert!(codex[0].meta.contains("effort"));
        assert!(codex[0].meta.contains("2026-08-30T00:00:00Z"));

        let kimi = parse_kimi_config(
            include_str!("../fixtures/resources/kimi_config.toml"),
            "读:fixture",
        )
        .expect("kimi fixture");
        let kimi_k3 = kimi
            .iter()
            .find(|model| model.id == "kimi-code/k3")
            .expect("k3 fixture model");
        assert!(kimi_k3.meta.contains("k3"));
        assert!(kimi_k3.meta.contains("1048576"));
        assert!(kimi_k3.meta.contains("low/high/max"));

        let pi = parse_pi_sources(
            include_str!("../fixtures/resources/pi_models_store.json"),
            include_str!("../fixtures/resources/pi_list_models.txt"),
            "读:fixture",
        )
        .expect("pi fixture");
        assert_eq!(pi.len(), 1);
        assert_eq!(pi[0].provider, "zai-coding-cn");
        assert_eq!(pi[0].vendor, "Zhipu");
        assert!(pi[0].meta.contains("204800"));

        let hints = parse_cc_switch_hints(include_str!("../fixtures/resources/cc_switch_list.json"))
            .expect("cc-switch fixture");
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].name, "Zhipu GLM");
        assert!(hints[0].has_token);
        assert!(parse_codex_cache(r#"{"models":[]}"#, "读:fixture").is_err());
    }

    #[test]
    fn pi_three_providers_parse_from_list_models() {
        let models = parse_pi_sources(
            include_str!("../fixtures/resources/pi_models_store_three.json"),
            include_str!("../fixtures/resources/pi_list_models_three.txt"),
            "读:fixture",
        )
        .expect("three-provider pi fixture");
        assert_eq!(models.len(), 4);
        let providers = models
            .iter()
            .map(|model| model.provider.as_str())
            .collect::<HashSet<_>>();
        assert!(providers.contains("deepseek"));
        assert!(providers.contains("qwen-token-plan-cn"));
        assert!(providers.contains("zai-coding-cn"));
        let flash = models
            .iter()
            .find(|model| model.id == "deepseek-v4-flash")
            .expect("deepseek-v4-flash");
        assert_eq!(flash.provider, "qwen-token-plan-cn");
        assert_eq!(flash.vendor, "DeepSeek");
        assert_eq!(flash.thinking, "支持");
    }

    #[test]
    fn kimi_alias_keeps_full_prefix() {
        let kimi = parse_kimi_config(
            include_str!("../fixtures/resources/kimi_config.toml"),
            "读:fixture",
        )
        .expect("kimi fixture");
        assert!(kimi.iter().all(|model| model.id.contains('/')));
        assert!(kimi.iter().any(|model| model.id.starts_with("kimi-code/")));
    }

    #[test]
    fn cc_switch_produces_hints_only_no_dispatchable_models() {
        // cc-switch 降级为发现提示：解析只给 hints；即使历史 store 里残留
        // cc-switch 的「已接入」模型，connected() 也必须排除。
        let hints = parse_cc_switch_hints(include_str!("../fixtures/resources/cc_switch_list.json"))
            .expect("hints");
        assert!(!hints.is_empty());

        let root = std::env::temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        fs::write(
            &path,
            r#"{"version":1,"agents":[
                {"id":"cc-switch","name":"cc-switch","models":[{"id":"glm-4.7","agent":"cc-switch","provider":"zai","status":"已接入"}]},
                {"id":"pi","name":"pi","models":[{"id":"glm-4.7","agent":"pi","provider":"zai-coding-cn","status":"已接入"}]}
            ]}"#,
        )
        .unwrap();
        let models = connected(&path).unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].agent, "pi");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ensure_agents_drops_cc_switch_and_seeds_builtins() {
        let mut store: ResourceStore = serde_json::from_str(
            r#"{"version":1,"agents":[{"id":"cc-switch","name":"cc-switch","models":[]}]}"#,
        )
        .expect("legacy store");
        ensure_agents(&mut store);
        assert!(store.agents.iter().all(|agent| agent.id != "cc-switch"));
        for id in ["claude", "codex", "kimi", "pi"] {
            assert!(store.agents.iter().any(|agent| agent.id == id));
        }
    }

    #[test]
    fn connected_filters_status() {
        let root = std::env::temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        fs::write(
            &path,
            r#"{"version":1,"agents":[{"id":"codex","name":"Codex","models":[{"id":"a","agent":"codex","status":"已接入"},{"id":"b","agent":"codex","status":"待冒烟"}]}]}"#,
        )
        .unwrap();
        let models = connected(&path).unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "a");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn merge_preserves_user_fields_adds_pending_and_marks_missing() {
        let mut store = ResourceStore {
            version: 1,
            agents: vec![fixture_agent(
                "codex",
                vec![ResourceModel {
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
                    smoke_failure: "旧失败；摘要保留完整".to_string(),
                    meta: "旧元数据".to_string(),
                    note: "保留备注".to_string(),
                }],
            )],
        };
        let mut refreshed = fixture_model("old-model");
        refreshed.billing_source = "新源".to_string();
        refreshed.meta = "新元数据".to_string();
        merge_detected_models(&mut store, "codex", vec![refreshed, fixture_model("new-model")]);
        let old = find_model(&store, "codex", "", "old-model").expect("old");
        assert_eq!(old.tier, "高");
        assert_eq!(old.note, "保留备注");
        assert_eq!(old.meta, "新元数据");
        assert_eq!(old.smoke_failure, "旧失败；摘要保留完整");
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
            agents: vec![fixture_agent(
                "codex",
                vec![ResourceModel {
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
                    smoke_failure: String::new(),
                    meta: "priority：1".to_string(),
                    note: "用户备注".to_string(),
                }],
            )],
        };
        let mut refreshed = fixture_model("same");
        refreshed.meta = "priority：2".to_string();
        merge_detected_models(&mut store, "codex", vec![refreshed]);
        let model = find_model(&store, "codex", "", "same").expect("merged model");
        assert_eq!(model.meta, "priority：2");
        assert_eq!(model.note, "用户备注");

        let kimi = parse_kimi_config(
            include_str!("../fixtures/resources/kimi_config_inline_comments.toml"),
            "读:fixture",
        )
        .expect("commented TOML fixture");
        assert_eq!(kimi[0].id, "kimi-code/commented");
        assert!(kimi[0].meta.contains("kimi-commented"));
        assert!(kimi[0].meta.contains("low/high"));
        assert_eq!(kimi[0].thinking, "支持");
    }

    #[test]
    fn pi_source_matching_uses_exact_tokens_and_auth_uses_top_level_status() {
        let store = r#"{
          "p": {"models": [
            {"id":"glm-4","provider":"p"},
            {"id":"glm-4.7","provider":"p"}
          ]}
        }"#;
        let models = parse_pi_sources(store, "p glm-4.7", "读:fixture").expect("exact match");
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "glm-4.7");

        assert_eq!(
            pi_auth_status(&process_result(
                Some(0),
                include_str!("../fixtures/resources/pi_auth_check_ready.json")
            )),
            Some("ready".to_string())
        );
        assert_eq!(
            pi_auth_status(&process_result(
                Some(0),
                include_str!("../fixtures/resources/pi_auth_check_not_ready.json")
            )),
            Some("not_ready".to_string())
        );
        assert_eq!(pi_auth_status(&process_result(Some(0), r#"{"ready":true}"#)), None);

        let mut model = fixture_model("glm-4.7");
        model.provider = "p".to_string();
        mark_pi_auth_not_ready(std::slice::from_mut(&mut model), "p", Some("not_ready"));
        assert!(model.meta.contains("鉴权未就绪"));
        assert!(model.meta.contains("鉴权状态：not_ready"));
    }

    #[test]
    fn add_model_requires_provider_for_pi() {
        let pi_store = ResourceStore {
            version: 1,
            agents: vec![fixture_agent("pi", Vec::new())],
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
    fn safe_summary_redacts_common_cli_secret_shapes() {
        let summary = safe_summary(
            "sk-abcdefghijk https://user:pass@example.com/a?key=one&token=two&api-key=three&api_key=four&access_token=five&secret=six Bearer abc.def-123",
        );
        assert!(!summary.contains("sk-abcdefghijk"));
        assert!(!summary.contains("user:pass"));
        for secret in ["one", "two", "three", "four", "five", "six"] {
            assert!(!summary.contains(secret));
        }
        assert!(!summary.contains("abc.def-123"));
        assert!(summary.contains("[已隐藏]"));

        let timeout = ProcessResult {
            code: None,
            stdout: String::new(),
            stderr: String::new(),
            timed_out: true,
            truncated: false,
            timeout: Duration::from_secs(20),
        };
        assert_eq!(process_error_summary(&timeout), "20 秒超时");
    }

    #[test]
    fn pi_list_models_is_primary_store_only_supplies_metadata() {
        // --list-models 为主：store 里没有的模型（清单新上的）必须出现在
        // 结果里；store 只补 contextWindow/reasoning 元数据。
        let listed = "provider       model           context\n\
                      deepseek       deepseek-chat   128000\n\
                      zai-coding-cn  glm-5.3-flash   128000";
        let store = r#"{"zai-coding-cn":{"models":[
            {"id":"glm-5.3-flash","provider":"zai-coding-cn","reasoning":true,"contextWindow":200000}
        ]}}"#;
        let models = parse_pi_sources(store, listed, "读:fixture").expect("pi listed primary");
        assert_eq!(models.len(), 2);
        let chat = models
            .iter()
            .find(|model| model.id == "deepseek-chat")
            .expect("store 里没有的模型也要收入");
        assert_eq!(chat.provider, "deepseek");
        assert_eq!(chat.thinking, "未知");
        assert!(!chat.meta.contains("上下文"));
        let flash = models
            .iter()
            .find(|model| model.id == "glm-5.3-flash")
            .expect("glm-5.3-flash");
        assert_eq!(flash.thinking, "支持");
        assert!(flash.meta.contains("200000"));
        // store 损坏不拦截探测（只丢元数据）
        let degraded = parse_pi_sources("{ 损坏", "p glm-4.7", "读:fixture").expect("坏 store 不拦截");
        assert_eq!(degraded.len(), 1);
        assert_eq!(degraded[0].thinking, "未知");
    }

    #[test]
    fn declared_models_are_not_marked_missing() {
        // 申报 source 的消失条目不标「未在事实源中」（修正轮 R1 补测）。
        let mut store = ResourceStore {
            version: 1,
            agents: vec![fixture_agent(
                "codex",
                vec![
                    ResourceModel {
                        id: "declared".to_string(),
                        agent: "codex".to_string(),
                        source: "申报(2026-09-01)".to_string(),
                        status: STATUS_PENDING.to_string(),
                        ..Default::default()
                    },
                    ResourceModel {
                        id: "from-source".to_string(),
                        agent: "codex".to_string(),
                        source: "读:old".to_string(),
                        status: STATUS_PENDING.to_string(),
                        ..Default::default()
                    },
                ],
            )],
        };
        merge_detected_models(&mut store, "codex", vec![fixture_model("other")]);
        assert!(
            !find_model(&store, "codex", "", "declared")
                .expect("declared")
                .meta
                .contains(MISSING_MARKER)
        );
        assert!(
            find_model(&store, "codex", "", "from-source")
                .expect("from-source")
                .meta
                .contains(MISSING_MARKER)
        );
    }

    #[test]
    fn corrupted_store_blocks_writes_and_keeps_bytes() {
        // 损坏的 resources.json：load_or_default 报错不覆盖；add/update 命令
        // 报错且文件字节不变（钉住「损坏不覆盖」保证）。
        let root = std::env::temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        fs::write(&path, "{ 损坏").unwrap();
        let before = fs::read(&path).unwrap();
        assert!(load_or_default(&path).is_err());
        let request = || ResourceModelRequest {
            agent_id: "codex".to_string(),
            id: "x".to_string(),
            provider: String::new(),
            vendor: String::new(),
            tier: String::new(),
            note: String::new(),
        };
        assert!(add_resource_model_at(&path, request()).is_err());
        assert!(update_resource_model_at(&path, request()).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn smoke_agent_models_partial_failure_writes_each_result() {
        // 两条模型一成一败：store 里一已接入一冒烟失败，errors 恰含一条，
        // 且每条结果都落了盘。
        let root = std::env::temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        fs::write(
            &path,
            r#"{"version":1,"agents":[{"id":"pi","name":"pi","models":[
                {"id":"a","agent":"pi","provider":"p","status":"待冒烟"},
                {"id":"b","agent":"pi","provider":"p","status":"待冒烟"}
            ]}]}"#,
        )
        .unwrap();
        let operation = smoke_agent_models_at(&path, "pi", |model| {
            if model.id == "a" {
                Ok(SmokeSuccess {
                    identity: "a".to_string(),
                    meta_marker: None,
                })
            } else {
                Err("403: denied".to_string())
            }
        })
        .expect("smoke batch");
        assert_eq!(operation.errors.len(), 1);
        assert!(operation.errors[0].contains("pi/b"));
        let ok = find_model(&operation.store, "pi", "p", "a").expect("a");
        let failed = find_model(&operation.store, "pi", "p", "b").expect("b");
        assert_eq!(ok.status, STATUS_CONNECTED);
        assert_eq!(ok.smoke_model, "a");
        assert_eq!(failed.status, STATUS_FAILED);
        assert!(failed.smoke_failure.contains("403"));
        let on_disk = load(&path).expect("disk store");
        assert_eq!(
            find_model(&on_disk, "pi", "p", "b").expect("b on disk").status,
            STATUS_FAILED
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn claude_base_url_read_from_settings_env() {
        assert_eq!(
            claude_base_url_from_settings(r#"{"env":{"ANTHROPIC_BASE_URL":"https://proxy.example"}}"#)
                .as_deref(),
            Some("https://proxy.example")
        );
        assert_eq!(claude_base_url_from_settings(r#"{"env":{}}"#), None);
        assert_eq!(claude_base_url_from_settings("{}"), None);
        assert_eq!(claude_base_url_from_settings("not json"), None);
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

    #[test]
    #[ignore = "真机验收：探测四个内置 agent（内存 store，不写 resources.json）"]
    fn live_detect_four_agents() {
        let mut store = ResourceStore {
            version: 1,
            agents: Vec::new(),
        };
        ensure_agents(&mut store);
        let mut errors = Vec::new();
        for id in ["claude", "codex", "kimi", "pi"] {
            detect_into(&mut store, id, &mut errors);
        }
        for agent in &store.agents {
            println!(
                "agent={} detected={} version={} models={}",
                agent.id,
                agent.detected,
                agent.version,
                agent.models.len()
            );
        }
        for error in &errors {
            println!("error: {error}");
        }
    }

    #[test]
    #[ignore = "真机验收：冒烟两条 pi 模型（2 次模型请求；内存 store，不写盘）"]
    fn live_smoke_pi_models() {
        let pi_model = |id: &str, provider: &str| ResourceModel {
            id: id.to_string(),
            agent: "pi".to_string(),
            provider: provider.to_string(),
            status: STATUS_PENDING.to_string(),
            ..Default::default()
        };
        let mut store = ResourceStore {
            version: 1,
            agents: vec![fixture_agent(
                "pi",
                vec![
                    pi_model("glm-5.3-flash", "zai-coding-cn"),
                    pi_model("deepseek-v4-flash", "qwen-token-plan-cn"),
                ],
            )],
        };
        for (id, provider) in [
            ("glm-5.3-flash", "zai-coding-cn"),
            ("deepseek-v4-flash", "qwen-token-plan-cn"),
        ] {
            let key = ResourceModelKey {
                agent_id: "pi".to_string(),
                id: id.to_string(),
                provider: provider.to_string(),
            };
            smoke_one_in_place(&mut store, &key).expect("smoke_one_in_place");
            let model = find_model(&store, "pi", provider, id).expect("model");
            println!(
                "smoke pi/{provider}/{id}: status={} smoke_model={} failure={}",
                model.status, model.smoke_model, model.smoke_failure
            );
        }
    }
}
