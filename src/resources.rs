//! 资源接入：探测本机 agent CLI 的可用模型，合并进 `~/.aicross/resources.json`。
//! 09-15 口径起派发清单列全部已探测模型（`detected()`），第三方端点的模型在
//! 第一次被派发到时由引擎自动验证一次（`verify_on_first_use`）。
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
use crate::process::{run_process, run_process_registered, version_for, ProcessResult};
use chrono::{Local, SecondsFormat};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;
use uuid::Uuid;

const STATUS_PENDING: &str = "待冒烟";
const STATUS_CONNECTED: &str = "已接入";
const STATUS_FAILED: &str = "冒烟失败";
const MISSING_MARKER: &str = "未在事实源中";
const SMOKE_PROMPT: &str = "只回复 OK";

/// 09-15 口径的四态（写进 `ResourceModel::verification`，前端按它显示状态点）。
/// 旧的 `status` 字段保留不删：接入页的详情里还要看「最近冒烟」那一段。
pub const VERIFICATION_NATIVE: &str = "native";
pub const VERIFICATION_UNVERIFIED: &str = "unverified";
pub const VERIFICATION_VERIFIED: &str = "verified";
pub const VERIFICATION_FAILED: &str = "failed";

/// 原生 CLI：自家模型清单说有就是有，不冒烟，只在 agent 级别探一次。
/// 「原生」= 原生 agent **且**没走第三方端点（修正轮 R1）：claude 挂了
/// `ANTHROPIC_BASE_URL`（或 settings.json 的 env）就按第三方走自动验证，
/// 静默降级正是在这条路上高发。判定见 `is_native_model`。
const NATIVE_AGENTS: [&str; 3] = ["claude", "codex", "kimi"];

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
    /// 09-15 口径：native / unverified / verified / failed。旧库缺这个字段，
    /// `ensure_verification` 按 agent + 旧 `status` 推断后补上。
    #[serde(default)]
    pub verification: String,
    /// 别名对应的真实模型 ID（TASK-09b）。`id` 是派发时写进命令行的那个词，
    /// claude 的 `fable` / `opus` / `sonnet` / `haiku` 都是别名，界面上只有别名
    /// 看不出跑的是哪个模型。非别名模型这里等于 `id`；冒烟/派发核对到真身后
    /// 由 `apply_smoke_outcome` 覆盖成实测值。旧库缺这个字段，`ensure_resolved_id`
    /// 按 `CLAUDE_ALIAS_IDS` 补上。
    #[serde(default)]
    pub resolved_id: String,
    /// `resolved_id` 是哪来的（修正轮 R1）：`alias-table` = 本地静态别名表算出来的，
    /// `identity` = 冒烟 / 首次派发核对到的真身。旧库缺这个字段，空值按
    /// `alias-table` 处理。作用是让别名表的修正能传播到旧库：静态值可以被新表
    /// 覆盖，实测真身不行——没有这个字段就分不清库里那一串是算的还是量的。
    #[serde(default)]
    pub resolved_source: String,
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
    let mut store = store;
    ensure_verification(&mut store);
    ensure_resolved_id(&mut store);
    Ok(store)
}

/// agent id 是不是原生三家。大小写不敏感——手工添加的 agent 写成 `Codex`
/// 时，前端（`toLowerCase`）和这里必须判成同一档。
fn is_native_agent(agent: &str) -> bool {
    NATIVE_AGENTS
        .iter()
        .any(|native| native.eq_ignore_ascii_case(agent.trim()))
}

/// 第三方端点判定。目前只覆盖 claude（进程环境 `ANTHROPIC_BASE_URL` 或
/// `~/.claude/settings.json` 的 `env.ANTHROPIC_BASE_URL`）——codex 的
/// `model_provider` 与 kimi 的 `config.toml` base_url 本期没有事实源读法，
/// 暂不覆盖，手册里写明这一点。
fn third_party_endpoint(agent: &str) -> bool {
    agent.trim().eq_ignore_ascii_case("claude") && claude_third_party_endpoint()
}

/// 「原生」的完整判据：原生 agent 且没走第三方端点。
/// 这条同时是 `verification == "native"` 的不变式——接入页按它决定给不给
/// 「验证」按钮，引擎按它决定跳不跳过首次验证。
fn is_native_model(agent: &str) -> bool {
    is_native_agent(agent) && !third_party_endpoint(agent)
}

/// 旧库没有 `verification` 时的推断：原生一律 native；其余按旧 `status`
/// 映射——已接入 → verified、冒烟失败 → failed、其余 → unverified。
fn inferred_verification(agent: &str, status: &str) -> &'static str {
    if is_native_model(agent) {
        VERIFICATION_NATIVE
    } else {
        match status {
            STATUS_CONNECTED => VERIFICATION_VERIFIED,
            STATUS_FAILED => VERIFICATION_FAILED,
            _ => VERIFICATION_UNVERIFIED,
        }
    }
}

/// 补齐 / 校正 `verification`：
/// - 空字段按 agent + 旧 `status` 推断；
/// - `native` 与「原生 agent 且官方端点」必须一致：claude 换到第三方端点后，
///   库里存着的 `native` 要降回按 status 推断的档（否则永远不自动验证）；
///   换回官方端点后再回到 native。
///
/// 除这条不变式外不改已有值：用户手动重验过的 verified / failed 不被覆盖。
/// `model.agent` 可能还没回填（`ensure_agents` 才做），用 agent.id 兜底。
/// claude 别名 → 真实模型 ID（TASK-09b）。claude 的事实源是 `--help` 里的别名
/// 举例，拿不到完整 ID，这张表是唯一的本地事实源；写表日期见
/// `CLAUDE_STATIC_TABLE_DATE`，过期由冒烟/派发核对真身后覆盖。
/// `ResourceModel::resolved_source` 的两个取值。空串等同前者。
pub const RESOLVED_SOURCE_ALIAS_TABLE: &str = "alias-table";
pub const RESOLVED_SOURCE_IDENTITY: &str = "identity";

const CLAUDE_ALIAS_IDS: [(&str, &str); 4] = [
    ("fable", "claude-fable-5-1"),
    ("opus", "claude-opus-5"),
    ("sonnet", "claude-sonnet-5"),
    ("haiku", "claude-haiku-4-5-20251001"),
];

/// 别名的真身。只有 claude 有别名；其余 agent 的 `id` 本身就是真实 ID。
fn resolved_id_for(agent: &str, id: &str) -> String {
    if agent.trim().eq_ignore_ascii_case("claude") {
        if let Some((_, real)) = CLAUDE_ALIAS_IDS
            .iter()
            .find(|(alias, _)| alias.eq_ignore_ascii_case(id.trim()))
        {
            return (*real).to_string();
        }
    }
    id.to_string()
}

/// 冒烟/派发核对到的真身 → `resolved_id`。pi 的真身带 provider 前缀
/// （`zai/glm-4.7`），`resolved_id` 只要模型那一半。
///
/// 前缀比对大小写不敏感（修正轮 R1，DS 10）：端点回报的 provider 写法与
/// models-store 里的名字常有 `Zai` / `zai` 这类差别，大小写敏感会让整串
/// `Zai/glm-4.7` 进界面，次文本里再冒出一个 provider 名。前缀不符或
/// provider 为空时原样返回——宁可多一段前缀，也不瞎切模型名。
fn resolved_id_from_identity(identity: &str, provider: &str) -> String {
    let identity = identity.trim();
    let provider = provider.trim();
    if !provider.is_empty() {
        let prefix = format!("{provider}/");
        if identity.len() > prefix.len()
            && identity[..prefix.len()].eq_ignore_ascii_case(&prefix)
        {
            return identity[prefix.len()..].to_string();
        }
    }
    identity.to_string()
}

/// 补齐 / 校正 `resolved_id`（修正轮 R1）：
/// - `resolved_source == identity`（冒烟核对到的真身）一律不动；
/// - 其余（空字段的旧库、静态表算出来的值）按当前别名表重算，
///   于是下一版修正别名表时旧库跟着更新——原来「非空就不覆盖」把静态值
///   也一并保护起来，修了表也传播不下去（DS 7）。
fn ensure_resolved_id(store: &mut ResourceStore) {
    for agent in &mut store.agents {
        let owner = agent.id.clone();
        for model in &mut agent.models {
            if model.resolved_source.trim() == RESOLVED_SOURCE_IDENTITY {
                continue;
            }
            let agent_id = if model.agent.trim().is_empty() {
                owner.clone()
            } else {
                model.agent.clone()
            };
            model.resolved_id = resolved_id_for(&agent_id, &model.id);
            model.resolved_source = RESOLVED_SOURCE_ALIAS_TABLE.to_string();
        }
    }
}

fn ensure_verification(store: &mut ResourceStore) {
    for agent in &mut store.agents {
        let owner = agent.id.clone();
        for model in &mut agent.models {
            let agent_id = if model.agent.trim().is_empty() {
                owner.clone()
            } else {
                model.agent.clone()
            };
            let native = is_native_model(&agent_id);
            let current = model.verification.trim();
            if current.is_empty() || native != (current == VERIFICATION_NATIVE) {
                model.verification = if native {
                    VERIFICATION_NATIVE.to_string()
                } else {
                    inferred_verification(&agent_id, &model.status).to_string()
                };
            }
        }
    }
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

/// 旧口径的「已接入」清单（status == 已接入）。cc-switch 一律排除。
// 08b 起派发清单改列全部探测到的模型（`detected`），左栏资源 tab 走 `get_resources`
// 直接拿整库；`connected` 只留作「已接入」语义的查询入口，暂无生产调用。
#[allow(dead_code)]
pub fn connected(path: &Path) -> Result<Vec<ResourceModel>, String> {
    Ok(load(path)?
        .agents
        .into_iter()
        .flat_map(|agent| agent.models)
        .filter(|model| model.status == STATUS_CONNECTED && model.agent != "cc-switch")
        .collect())
}

/// 供界面挑模型用的清单：**不按冒烟状态过滤**，已接入 / 待冒烟 / 冒烟失败都给，
/// 只排除 cc-switch（已降级为发现提示，不产生可派发条目）。
/// 和 `connected()` 的分工：`connected()` 只是旧的「已接入」查询入口，没有生产
/// 调用；这个是「本机探测到什么」的语义，节点面板、`@` 弹层和 G1 用它，
/// 界面自己按 verification 打后缀。
/// 排序（09-15 起按 verification 而不是旧 status）：原生 / 已验证 → 未验证
/// → 验证失败，同一档内 agent → provider → id。
pub fn detected(path: &Path) -> Result<Vec<ResourceModel>, String> {
    let mut models: Vec<ResourceModel> = load(path)?
        .agents
        .into_iter()
        .flat_map(|agent| agent.models)
        .filter(|model| model.agent != "cc-switch")
        .collect();
    models.sort_by(|a, b| {
        verification_rank(&a.verification)
            .cmp(&verification_rank(&b.verification))
            .then_with(|| a.agent.cmp(&b.agent))
            .then_with(|| a.provider.cmp(&b.provider))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(models)
}

fn verification_rank(verification: &str) -> u8 {
    match verification {
        VERIFICATION_NATIVE | VERIFICATION_VERIFIED => 0,
        VERIFICATION_UNVERIFIED => 1,
        VERIFICATION_FAILED => 2,
        _ => 3,
    }
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
    ensure_verification(&mut *store);
    ensure_resolved_id(&mut *store);
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
        verification: inferred_verification(agent, STATUS_PENDING).to_string(),
        resolved_id: resolved_id_for(agent, id),
        resolved_source: RESOLVED_SOURCE_ALIAS_TABLE.to_string(),
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
            // 冒烟核对到的真身比别名静态表可信：重新探测不许把它退回静态值。
            // 判据是来源而不是「非空」——静态值也非空，按非空保护会让别名表的
            // 修正传播不到旧库（修正轮 R1，DS 7）。
            if existing.resolved_source.trim() == RESOLVED_SOURCE_IDENTITY {
                model.resolved_id.clone_from(&existing.resolved_id);
                model.resolved_source.clone_from(&existing.resolved_source);
            }
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
    let verification = inferred_verification(&request.agent_id, STATUS_PENDING).to_string();
    let resolved_id = resolved_id_for(&request.agent_id, id);
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
        verification,
        resolved_id,
        resolved_source: RESOLVED_SOURCE_ALIAS_TABLE.to_string(),
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
    // 测试构建里不读开发机的环境变量与 ~/.claude/settings.json：判据由
    // `set_claude_third_party` 显式给，默认「官方端点」。
    #[cfg(test)]
    {
        CLAUDE_THIRD_PARTY.with(|cell| cell.get())
    }
    #[cfg(not(test))]
    {
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
}

#[cfg(test)]
thread_local! {
    /// 测试里的 claude 端点判据（修正轮 R1）。
    static CLAUDE_THIRD_PARTY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
fn set_claude_third_party(value: bool) {
    CLAUDE_THIRD_PARTY.with(|cell| cell.set(value));
}

fn claude_base_url_from_settings(contents: &str) -> Option<String> {
    let value: Value = serde_json::from_str(contents).ok()?;
    value
        .get("env")?
        .get("ANTHROPIC_BASE_URL")?
        .as_str()
        .map(str::to_string)
}

/// 手动冒烟：不进进程注册表（接入页有自己的 busy 态，没有叫停入口）。
fn run_smoke(model: &ResourceModel) -> Result<SmokeSuccess, String> {
    run_smoke_registered(model, None)
}

/// 冒烟：argv 与真身核对完全复用 channels（盲跑 blind=true，prompt 固定
/// 「只回复 OK」），进程执行走 process.rs。成功返回响应里的模型真身
/// （identity），失败返回脱敏后的原因。
///
/// `registry_key` 是派发节点的取消键：首次派发自动验证时传进来，`cancel_registered`
/// 才杀得掉这次冒烟进程（修正轮 R1：原来走无注册的 `run_process`，用户点了
/// 「叫停」冒烟照跑完，随后真实请求照样发出去）。
fn run_smoke_registered(
    model: &ResourceModel,
    registry_key: Option<&str>,
) -> Result<SmokeSuccess, String> {
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
    let result = run_process_registered(
        &invocation.program,
        &invocation.args,
        invocation.input.as_deref(),
        Duration::from_secs(60),
        Some(&temp),
        registry_key,
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
            let alias_marker = route_alias_marker(&model, &output.identity);
            Ok(SmokeSuccess { identity: output.identity, meta_marker: alias_marker })
        }
        Err(error) => Err(error.message),
    }
}

fn route_alias_marker(model: &ResourceModel, identity: &str) -> Option<String> {
    crate::channels::KNOWN_MODEL_ROUTES.iter()
        .find(|(p, req, actual)| {
            *p == model.provider && *req == model.id && identity == format!("{p}/{actual}")
        })
        .map(|(_, req, actual)| format!("路由别名：{req}→{actual}"))
}

/// 把一次冒烟结果写进 store：成功 → 已接入 + 真身 + 可选 meta 标记；
/// 失败 → 冒烟失败 + 脱敏原因。
/// `verification` 跟着结果走，但原生模型恒为 native——原生模型不按冒烟
/// 结果改档（界面上也不给「验证」按钮），冒烟结果只落在 status 与详情里。
/// 「原生」按 `is_native_model` 判：claude 挂第三方端点时它是第三方，冒烟
/// 成败会真的写进 verified / failed。
fn apply_smoke_outcome(
    store: &mut ResourceStore,
    request: &ResourceModelKey,
    outcome: Result<SmokeSuccess, String>,
) -> Result<(), String> {
    let native = is_native_model(&request.agent_id);
    let target = find_model_mut(store, &request.agent_id, &request.provider, &request.id)?;
    match outcome {
        Ok(success) => {
            target.status = STATUS_CONNECTED.to_string();
            target.verification = if native {
                VERIFICATION_NATIVE
            } else {
                VERIFICATION_VERIFIED
            }
            .to_string();
            target.smoked_at = Local::now().to_rfc3339_opts(SecondsFormat::Secs, false);
            // 真身覆盖别名静态表（TASK-09b）：核对到的 model 字段才是这次真跑的
            // 模型。空真身不覆盖——个别通道拿不到 model 字段时别把已有值抹掉。
            let resolved = resolved_id_from_identity(&success.identity, &request.provider);
            if !resolved.is_empty() {
                target.resolved_id = resolved;
                target.resolved_source = RESOLVED_SOURCE_IDENTITY.to_string();
            }
            target.smoke_model = success.identity;
            target.smoke_failure.clear();
            if let Some(marker) = success.meta_marker {
                target.meta = append_note_marker(&target.meta, &marker);
            }
        }
        Err(error) => {
            target.status = STATUS_FAILED.to_string();
            target.verification = if native {
                VERIFICATION_NATIVE
            } else {
                VERIFICATION_FAILED
            }
            .to_string();
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

/// 首次派发自动验证的结果（引擎按它决定继续派发还是直接判失败）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FirstUseVerification {
    /// 不需要动手：原生模型、验过的、验失败过的。
    Skipped,
    /// 资源库里没有这一条（或库读不出来）：本次不验证，留痕写明，不挡派发。
    /// 与 `Skipped` 分开，事后才分得清「原生跳过」和「库里没这条所以没验」。
    NotInStore,
    /// 本次真发了一个短请求并核对了真身，结果已写回资源库。
    Verified,
    /// 本次验证失败（或验过了但结果写不回资源库），附脱敏后的原因。
    Failed(String),
    /// 验证窗口内用户点了叫停：不写库、不继续派发。
    Cancelled,
}

/// 第三方端点首次派发前的自动验证（09-15 定案）。只有 `verification ==
/// "unverified"` 才动手，判据与写回完全复用手动冒烟那一套。
///
/// 失败的不自动重试：这一条派发按失败返回，用户到接入页手动重验。
/// 资源库里没这一条时返回 `NotInStore` 放行——自动验证是加固，不该把「还没
/// 接入过资源」的用户挡在派发之外，但留痕里要说清没验。
///
/// `cancel_key` 是派发节点的取消键：冒烟进程按它注册，用户点「叫停」时
/// `cancel_registered` 杀得掉；冒烟返回后再查一次，取消了就不写库、按
/// `Cancelled` 返回。
///
/// 测试构建里 dispatch 走线程局部替身、不碰用户真实的 resources.json，
/// 这个入口因此在 `cargo test` 下无调用者。
#[cfg_attr(test, allow(dead_code))]
pub fn verify_on_first_use(
    agent: &str,
    provider: &str,
    model: &str,
    cancel_key: Option<&str>,
) -> FirstUseVerification {
    match store_path() {
        Ok(path) => verify_on_first_use_at(
            &path,
            agent,
            provider,
            model,
            |target| run_smoke_registered(target, cancel_key),
            || cancel_key.map(crate::dispatch::is_cancelled).unwrap_or(false),
        ),
        Err(_) => FirstUseVerification::NotInStore,
    }
}

fn key_of(model: &ResourceModel) -> ResourceModelKey {
    ResourceModelKey {
        agent_id: model.agent.clone(),
        id: model.id.clone(),
        provider: model.provider.clone(),
    }
}

/// 首次验证的目标解析结果。
enum FirstUseTarget {
    Hit(ResourceModelKey),
    Missing,
    /// provider 为空且同名模型挂在多个 provider 下，其中还有没验过的：
    /// 验哪一个都可能不是真正要派的那一个，只能报错。
    Ambiguous(Vec<String>),
}

/// 目标解析。先按 (agent, provider, id) 精确找；provider 为空时退到 agent + id
/// ——节点面板选模型不写 provider、旧图的 Target 也可能没有，精确匹配必然落空，
/// 原来会在这里静默跳过（G1 已经承诺「首次派发会先验证真身」，却一个请求都不发）。
fn resolve_first_use_target(
    store: &ResourceStore,
    agent: &str,
    provider: &str,
    id: &str,
) -> FirstUseTarget {
    let exact = ResourceModelKey {
        agent_id: agent.to_string(),
        id: id.to_string(),
        provider: provider.to_string(),
    };
    if find_model(store, &exact.agent_id, &exact.provider, &exact.id).is_ok() {
        return FirstUseTarget::Hit(exact);
    }
    if !provider.trim().is_empty() {
        return FirstUseTarget::Missing;
    }
    let candidates: Vec<&ResourceModel> = store
        .agents
        .iter()
        .flat_map(|owner| owner.models.iter())
        .filter(|model| model.agent.eq_ignore_ascii_case(agent.trim()) && model.id == id)
        .collect();
    match candidates.len() {
        0 => FirstUseTarget::Missing,
        1 => FirstUseTarget::Hit(key_of(candidates[0])),
        _ => {
            // 全都不是「未验证」时不需要动手，随便挑一条走到 Skipped 就行。
            if candidates
                .iter()
                .all(|model| model.verification != VERIFICATION_UNVERIFIED)
            {
                return FirstUseTarget::Hit(key_of(candidates[0]));
            }
            FirstUseTarget::Ambiguous(
                candidates
                    .iter()
                    .map(|model| {
                        if model.provider.trim().is_empty() {
                            "(默认)".to_string()
                        } else {
                            model.provider.clone()
                        }
                    })
                    .collect(),
            )
        }
    }
}

/// 进程级「验证中」表：同一 (agent, provider, id) 同时只允许一路在验证。
/// 两路并发首派时第二路等第一路写完库再读状态——既不重复发短请求，也不会
/// 出现一成一败按写回顺序覆盖的情况。
fn first_use_gate(key: &ResourceModelKey) -> Arc<Mutex<()>> {
    static GATES: OnceLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = OnceLock::new();
    let id = format!("{}\u{1}{}\u{1}{}", key.agent_id, key.provider, key.id);
    let table = GATES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = table.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard
        .entry(id)
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

fn verify_on_first_use_at(
    path: &Path,
    agent: &str,
    provider: &str,
    model: &str,
    smoke: impl FnOnce(&ResourceModel) -> Result<SmokeSuccess, String>,
    cancelled: impl Fn() -> bool,
) -> FirstUseVerification {
    // 读一次拿目标就放锁：一次冒烟最长 60s，整段持锁会把界面的读资源也堵住。
    let key = {
        let _guard = store_lock();
        let Ok(store) = load_or_default(path) else {
            return FirstUseVerification::NotInStore;
        };
        match resolve_first_use_target(&store, agent, provider, model) {
            FirstUseTarget::Hit(key) => key,
            FirstUseTarget::Missing => return FirstUseVerification::NotInStore,
            FirstUseTarget::Ambiguous(providers) => {
                return FirstUseVerification::Failed(format!(
                    "{agent}/{model} 在多个 provider 下都有（{}）且未全部验证，无法确定要验的端点：请在节点面板选定带 provider 的条目",
                    providers.join(" / ")
                ))
            }
        }
    };
    // 「验证中」表：拿到闸门再重读一次状态，第二路在这里看到第一路的结果。
    let gate = first_use_gate(&key);
    let _inflight = gate.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let target = {
        let _guard = store_lock();
        let Ok(store) = load_or_default(path) else {
            return FirstUseVerification::NotInStore;
        };
        match find_model(&store, &key.agent_id, &key.provider, &key.id) {
            Ok(found) => found.clone(),
            Err(_) => return FirstUseVerification::NotInStore,
        }
    };
    if target.verification != VERIFICATION_UNVERIFIED {
        return FirstUseVerification::Skipped;
    }
    let outcome = smoke(&target);
    // 冒烟期间点了叫停：进程已经被 cancel_registered 杀掉，这次结果不算数，
    // 不写库（免得把「被杀」记成验证失败），节点按「已取消」返回。
    if cancelled() {
        return FirstUseVerification::Cancelled;
    }
    let failure = outcome.as_ref().err().map(|error| safe_summary(error));
    let persisted = {
        let _guard = store_lock();
        match load_or_default(path) {
            Ok(mut store) => apply_smoke_outcome(&mut store, &key, outcome)
                .and_then(|()| save_store(path, &store)),
            Err(error) => Err(error),
        }
    };
    // 写回失败不吞：库里还是 unverified，下次派发会再冒一次烟，「失败不自动
    // 重试」在这条路径上就不成立了。让这次派发失败并写明原因。
    if let Err(error) = persisted {
        let reason = safe_summary(&error);
        return FirstUseVerification::Failed(match failure {
            Some(failure) => format!("{failure}；验证结果写回资源库也失败：{reason}"),
            None => format!("验证通过，但结果写回资源库失败：{reason}"),
        });
    }
    match failure {
        Some(reason) => FirstUseVerification::Failed(reason),
        None => FirstUseVerification::Verified,
    }
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

pub(crate) fn infer_vendor(id: &str) -> &'static str {
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
                        "验证 {}/{} 失败：{}",
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
            return Err("手动接入失败：名称和二进制名不能为空".to_string());
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
    .map_err(|error| format!("手动接入任务失败：{error}"))?
}

#[tauri::command]
pub async fn smoke_resource_model(request: ResourceModelKey) -> Result<ResourceStore, String> {
    tauri::async_runtime::spawn_blocking(move || smoke_resource_model_at(&store_path()?, request))
        .await
        .map_err(|error| format!("验证任务失败：{error}"))?
}

#[tauri::command]
pub async fn smoke_agent_models(agent_id: String) -> Result<ResourceOperation, String> {
    tauri::async_runtime::spawn_blocking(move || {
        smoke_agent_models_at(&store_path()?, &agent_id, run_smoke)
    })
    .await
    .map_err(|error| format!("验证任务失败：{error}"))?
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

    /// `detected()` 与 `connected()` 的分工：前者给界面挑模型用，不按冒烟状态过滤，
    /// 但仍然排除 cc-switch；排序（09-15 起按 verification）是原生 / 已验证 →
    /// 未验证 → 验证失败，同一档内 agent/provider/id。
    #[test]
    fn detected_keeps_unverified_drops_cc_switch_and_sorts_by_verification() {
        let root = std::env::temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        fs::write(
            &path,
            r#"{"version":1,"agents":[
                {"id":"pi","name":"pi","models":[
                    {"id":"z-b","agent":"pi","provider":"zai","status":"待冒烟"},
                    {"id":"z-a","agent":"pi","provider":"zai","status":"待冒烟"},
                    {"id":"m-a","agent":"pi","provider":"moon","status":"待冒烟"},
                    {"id":"bad","agent":"pi","provider":"moon","status":"冒烟失败"}]},
                {"id":"codex","name":"Codex","models":[
                    {"id":"gpt","agent":"codex","status":"已接入"}]},
                {"id":"cc-switch","name":"cc-switch","models":[
                    {"id":"ghost","agent":"cc-switch","status":"已接入"}]}]}"#,
        )
        .unwrap();

        let all = detected(&path).unwrap();
        let listed: Vec<(&str, &str)> = all
            .iter()
            .map(|m| (m.agent.as_str(), m.id.as_str()))
            .collect();
        // 原生（codex）在前；未验证按 provider(moon<zai) → id；验证失败垫底；cc-switch 不出现
        assert_eq!(
            listed,
            vec![
                ("codex", "gpt"),
                ("pi", "m-a"),
                ("pi", "z-a"),
                ("pi", "z-b"),
                ("pi", "bad"),
            ]
        );
        assert!(all.iter().all(|m| m.agent != "cc-switch"));

        // connected() 的语义不受影响：仍然只有已接入的那一条
        let only_connected = connected(&path).unwrap();
        assert_eq!(only_connected.len(), 1);
        assert_eq!(only_connected[0].id, "gpt");

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
                    verification: VERIFICATION_NATIVE.to_string(),
                    resolved_id: "old-model".to_string(),
                    resolved_source: RESOLVED_SOURCE_ALIAS_TABLE.to_string(),
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
                    verification: VERIFICATION_NATIVE.to_string(),
                    resolved_id: "same".to_string(),
                    resolved_source: RESOLVED_SOURCE_ALIAS_TABLE.to_string(),
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

    // ── TASK-09b：别名 → 真实模型 ID ──────────────────────────────────────

    /// 四个 claude 别名按已知表落 `resolved_id`；非别名模型 `resolved_id == id`；
    /// 同名词出现在别的 agent 下（pi 的 provider 里也可能有叫 `opus` 的）不映射。
    #[test]
    fn task09b_claude_aliases_resolve_to_known_model_ids() {
        assert_eq!(resolved_id_for("claude", "fable"), "claude-fable-5-1");
        assert_eq!(resolved_id_for("claude", "opus"), "claude-opus-5");
        assert_eq!(resolved_id_for("claude", "sonnet"), "claude-sonnet-5");
        assert_eq!(resolved_id_for("claude", "haiku"), "claude-haiku-4-5-20251001");
        // 大小写与空白不敏感（手工添加的 agent 会写成 `Claude`）
        assert_eq!(resolved_id_for("Claude", " Sonnet "), "claude-sonnet-5");
        // 非别名：claude 下的完整 ID 与别家的模型都原样返回
        assert_eq!(resolved_id_for("claude", "claude-opus-5"), "claude-opus-5");
        assert_eq!(resolved_id_for("pi", "opus"), "opus");
        assert_eq!(resolved_id_for("codex", "gpt-5.1-codex"), "gpt-5.1-codex");
    }

    /// 旧库（没有 resolved_id 字段）读进来时补齐：claude 别名按表，其余等于 id。
    /// 修正轮 R1：判据从「非空就不覆盖」换成「看 `resolved_source`」——没有来源
    /// 标记的旧值算静态值，别名表修正后跟着更新；标了 `identity` 的实测真身不动。
    #[test]
    fn task09b_legacy_store_backfills_resolved_id() {
        let root = std::env::temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        fs::write(
            &path,
            r#"{"version":1,"agents":[
                {"id":"claude","name":"Claude","models":[
                    {"id":"fable","agent":"claude","status":"待冒烟"},
                    {"id":"opus","agent":"claude","status":"待冒烟","resolved_id":"claude-opus-5-2"},
                    {"id":"sonnet","agent":"claude","status":"待冒烟",
                     "resolved_id":"claude-sonnet-5-20260901","resolved_source":"identity"}]},
                {"id":"pi","name":"pi","models":[
                    {"id":"glm-4.7","agent":"pi","provider":"zai","status":"待冒烟"}]}]}"#,
        )
        .unwrap();
        let store = load(&path).expect("legacy store");
        let model = |agent: &str, provider: &str, id: &str| {
            find_model(&store, agent, provider, id).expect(id).clone()
        };
        let fable = model("claude", "", "fable");
        assert_eq!(fable.resolved_id, "claude-fable-5-1");
        assert_eq!(fable.resolved_source, RESOLVED_SOURCE_ALIAS_TABLE);
        // 没有来源标记的旧值算静态值：按当前别名表重算，修表能传播到旧库
        let opus = model("claude", "", "opus");
        assert_eq!(opus.resolved_id, "claude-opus-5");
        assert_eq!(opus.resolved_source, RESOLVED_SOURCE_ALIAS_TABLE);
        // 标了 identity 的实测真身一个字节不动
        let sonnet = model("claude", "", "sonnet");
        assert_eq!(sonnet.resolved_id, "claude-sonnet-5-20260901");
        assert_eq!(sonnet.resolved_source, RESOLVED_SOURCE_IDENTITY);
        // 非别名模型：resolved_id == id
        assert_eq!(model("pi", "zai", "glm-4.7").resolved_id, "glm-4.7");
        let _ = fs::remove_dir_all(root);
    }

    /// 冒烟核对到的真身覆盖静态表；pi 的真身带 provider 前缀，只取模型那一半；
    /// 失败时不动 `resolved_id`（上次核对过的真身不该被一次网络故障抹掉）。
    #[test]
    fn task09b_smoke_success_overwrites_resolved_id_with_identity() {
        let mut claude = detected_model("fable", "claude", "", "Anthropic", "官方订阅", "未知", "读:fixture", "");
        assert_eq!(claude.resolved_id, "claude-fable-5-1");
        claude.verification = VERIFICATION_UNVERIFIED.to_string();
        let mut store = ResourceStore {
            version: 1,
            agents: vec![fixture_agent("claude", vec![claude])],
        };
        let key = ResourceModelKey {
            agent_id: "claude".into(),
            provider: String::new(),
            id: "fable".into(),
        };
        apply_smoke_outcome(
            &mut store,
            &key,
            Ok(SmokeSuccess { identity: "claude-fable-5-1-20260901".into(), meta_marker: None }),
        )
        .unwrap();
        let verified = find_model(&store, "claude", "", "fable").unwrap();
        assert_eq!(verified.resolved_id, "claude-fable-5-1-20260901");
        assert_eq!(verified.resolved_source, RESOLVED_SOURCE_IDENTITY);
        apply_smoke_outcome(&mut store, &key, Err("端点不可达".into())).unwrap();
        assert_eq!(
            find_model(&store, "claude", "", "fable").unwrap().resolved_id,
            "claude-fable-5-1-20260901",
            "验证失败不该抹掉上次核对出来的真身"
        );

        // pi：真身是 `provider/model`，resolved_id 只留模型那一半
        let pi_model = ResourceModel {
            id: "deepseek-v4-flash".into(),
            agent: "pi".into(),
            provider: "deepseek".into(),
            ..ResourceModel::default()
        };
        let mut pi_store = ResourceStore {
            version: 1,
            agents: vec![fixture_agent("pi", vec![pi_model])],
        };
        let pi_key = ResourceModelKey {
            agent_id: "pi".into(),
            provider: "deepseek".into(),
            id: "deepseek-v4-flash".into(),
        };
        apply_smoke_outcome(
            &mut pi_store,
            &pi_key,
            Ok(SmokeSuccess { identity: "deepseek/deepseek-flash".into(), meta_marker: None }),
        )
        .unwrap();
        let saved = find_model(&pi_store, "pi", "deepseek", "deepseek-v4-flash").unwrap();
        assert_eq!(saved.resolved_id, "deepseek-flash");
        // smoke_model 仍是完整真身，两个字段不互相替代
        assert_eq!(saved.smoke_model, "deepseek/deepseek-flash");
    }

    /// 重新探测不许把冒烟核对出来的真身退回静态表；静态值则按当前别名表刷新。
    #[test]
    fn task09b_redetect_keeps_verified_resolved_id() {
        let mut existing = detected_model("fable", "claude", "", "Anthropic", "官方订阅", "未知", "读:old", "");
        existing.resolved_id = "claude-fable-5-1-20260901".to_string();
        existing.resolved_source = RESOLVED_SOURCE_IDENTITY.to_string();
        let mut store = ResourceStore {
            version: 1,
            agents: vec![fixture_agent("claude", vec![existing])],
        };
        merge_detected_models(
            &mut store,
            "claude",
            vec![detected_model("fable", "claude", "", "Anthropic", "官方订阅", "未知", "读:new", "")],
        );
        let kept = find_model(&store, "claude", "", "fable").unwrap();
        assert_eq!(kept.resolved_id, "claude-fable-5-1-20260901");
        assert_eq!(kept.resolved_source, RESOLVED_SOURCE_IDENTITY);
    }

    /// 修正轮 R1（DS 7）：`resolved_source` 把「静态表算的」与「实测量的」分开。
    /// 静态值可以被修正后的别名表覆盖，实测真身不行——没有这个字段，
    /// 一旦写库就永久压过静态表，改了别名表也传播不到旧库。
    #[test]
    fn task09b_resolved_source_lets_alias_table_fix_reach_old_stores() {
        // 库里是上一版别名表算出来的过时值，没冒烟过
        let mut stale = detected_model("opus", "claude", "", "Anthropic", "官方订阅", "未知", "读:old", "");
        stale.resolved_id = "claude-opus-4-9".to_string();
        assert_eq!(stale.resolved_source, RESOLVED_SOURCE_ALIAS_TABLE);
        // 同一条被冒烟核对过，来源标成 identity
        let mut measured = detected_model("sonnet", "claude", "", "Anthropic", "官方订阅", "未知", "读:old", "");
        measured.resolved_id = "claude-sonnet-5-20260901".to_string();
        measured.resolved_source = RESOLVED_SOURCE_IDENTITY.to_string();
        let mut store = ResourceStore {
            version: 1,
            agents: vec![fixture_agent("claude", vec![stale, measured])],
        };

        ensure_resolved_id(&mut store);

        let opus = find_model(&store, "claude", "", "opus").unwrap();
        assert_eq!(opus.resolved_id, "claude-opus-5", "静态值该被当前别名表覆盖");
        let sonnet = find_model(&store, "claude", "", "sonnet").unwrap();
        assert_eq!(
            sonnet.resolved_id, "claude-sonnet-5-20260901",
            "实测真身不许被静态表压回去"
        );
    }

    /// 修正轮 R1（GLM「最该补的测试 1」）：手动录入这条路径原来零测试，
    /// 把 `resolved_id` / `resolved_source` 那两行删掉现有测试也全绿。
    #[test]
    fn task09b_add_model_resolves_claude_alias() {
        let store = ResourceStore {
            version: 1,
            agents: vec![fixture_agent("claude", Vec::new())],
        };
        let store = add_model(
            store,
            ResourceModelRequest {
                agent_id: "claude".to_string(),
                id: "sonnet".to_string(),
                provider: String::new(),
                vendor: "Anthropic".to_string(),
                tier: String::new(),
                note: String::new(),
            },
        )
        .expect("manual claude alias");
        let added = find_model(&store, "claude", "", "sonnet").expect("sonnet");
        assert_eq!(added.resolved_id, "claude-sonnet-5");
        assert_eq!(added.resolved_source, RESOLVED_SOURCE_ALIAS_TABLE);

        // 非别名的手动条目：resolved_id == id
        let store = add_model(
            store,
            ResourceModelRequest {
                agent_id: "claude".to_string(),
                id: "claude-opus-5".to_string(),
                provider: String::new(),
                vendor: "Anthropic".to_string(),
                tier: String::new(),
                note: String::new(),
            },
        )
        .expect("manual full id");
        let full = find_model(&store, "claude", "", "claude-opus-5").expect("opus");
        assert_eq!(full.resolved_id, "claude-opus-5");
    }

    /// 修正轮 R1（DS 10）：真身前缀剥离的边界。原来只测了「前缀与 provider
    /// 完全同形」一条，大小写不同 / 前缀不符 / provider 为空都没钉住。
    #[test]
    fn task09b_resolved_id_from_identity_strips_only_matching_prefix() {
        // 同形：剥掉
        assert_eq!(resolved_id_from_identity("zai/glm-4.7", "zai"), "glm-4.7");
        // 大小写不同：照样剥（端点回报的写法与 models-store 常不一致）
        assert_eq!(resolved_id_from_identity("Zai/glm-4.7", "zai"), "glm-4.7");
        assert_eq!(resolved_id_from_identity("zai/glm-4.7", "ZAI"), "glm-4.7");
        // 前缀不符：原样返回，不瞎切
        assert_eq!(
            resolved_id_from_identity("bigmodel/glm-4.7", "zai"),
            "bigmodel/glm-4.7"
        );
        // provider 为空（旧库 / 手写条目）：不剥
        assert_eq!(resolved_id_from_identity("zai/glm-4.7", ""), "zai/glm-4.7");
        // 根本没有前缀
        assert_eq!(resolved_id_from_identity("glm-4.7", "zai"), "glm-4.7");
        // 只有前缀没有模型名：整串留着，切完是空的更没法用
        assert_eq!(resolved_id_from_identity("zai/", "zai"), "zai/");
        // 空真身
        assert_eq!(resolved_id_from_identity("  ", "zai"), "");
    }

    // ── TASK-09：verification 三态与首次派发自动验证 ──────────────────────

    /// 旧库（没有 verification 字段）按 agent + 旧 status 推断：原生三家一律
    /// native，第三方按已接入 / 冒烟失败 / 其余映射成 verified / failed /
    /// unverified；旧字段本身不动。
    #[test]
    fn task09_legacy_store_infers_verification_three_ways() {
        let root = std::env::temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        fs::write(
            &path,
            r#"{"version":1,"agents":[
                {"id":"codex","name":"Codex","models":[{"id":"gpt","agent":"codex","status":"待冒烟"}]},
                {"id":"claude","name":"Claude","models":[{"id":"fable","agent":"claude","status":"冒烟失败"}]},
                {"id":"kimi","name":"Kimi","models":[{"id":"k3","agent":"kimi","status":"已接入"}]},
                {"id":"pi","name":"pi","models":[
                    {"id":"ok","agent":"pi","provider":"zai","status":"已接入"},
                    {"id":"bad","agent":"pi","provider":"zai","status":"冒烟失败"},
                    {"id":"new","agent":"pi","provider":"zai","status":"待冒烟"}]}]}"#,
        )
        .unwrap();
        let store = load(&path).expect("legacy store");
        let verification = |agent: &str, provider: &str, id: &str| {
            find_model(&store, agent, provider, id)
                .expect(id)
                .verification
                .clone()
        };
        assert_eq!(verification("codex", "", "gpt"), VERIFICATION_NATIVE);
        assert_eq!(verification("claude", "", "fable"), VERIFICATION_NATIVE);
        assert_eq!(verification("kimi", "", "k3"), VERIFICATION_NATIVE);
        assert_eq!(verification("pi", "zai", "ok"), VERIFICATION_VERIFIED);
        assert_eq!(verification("pi", "zai", "bad"), VERIFICATION_FAILED);
        assert_eq!(verification("pi", "zai", "new"), VERIFICATION_UNVERIFIED);
        // 旧字段保留不删
        assert_eq!(
            find_model(&store, "claude", "", "fable").expect("fable").status,
            STATUS_FAILED
        );
        let _ = fs::remove_dir_all(root);
    }

    /// 原生模型不走首次验证：冒烟函数一次都不该被调用。
    #[test]
    fn task09_verify_on_first_use_never_smokes_native_models() {
        let root = std::env::temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        fs::write(
            &path,
            r#"{"version":1,"agents":[{"id":"codex","name":"Codex","models":[
                {"id":"gpt","agent":"codex","status":"待冒烟"}]}]}"#,
        )
        .unwrap();
        let calls = std::cell::Cell::new(0u32);
        let outcome = verify_on_first_use_at(
            &path,
            "codex",
            "",
            "gpt",
            |_| {
                calls.set(calls.get() + 1);
                Ok(SmokeSuccess { identity: "gpt".into(), meta_marker: None })
            },
            || false,
        );
        assert_eq!(outcome, FirstUseVerification::Skipped);
        assert_eq!(calls.get(), 0);
        let _ = fs::remove_dir_all(root);
    }

    /// 第三方首次派发验证失败：返回 Failed 并把 failed 写回资源库；
    /// 再派发一次不重试（failed 不是 unverified），冒烟函数只被调一次。
    #[test]
    fn task09_first_use_failure_persists_and_does_not_retry() {
        let root = std::env::temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        fs::write(
            &path,
            r#"{"version":1,"agents":[{"id":"pi","name":"pi","models":[
                {"id":"glm-4.7","agent":"pi","provider":"zai","status":"待冒烟"}]}]}"#,
        )
        .unwrap();
        let calls = std::cell::Cell::new(0u32);
        let smoke = |_: &ResourceModel| -> Result<SmokeSuccess, String> {
            calls.set(calls.get() + 1);
            Err("401: invalid api key".to_string())
        };
        let first = verify_on_first_use_at(&path, "pi", "zai", "glm-4.7", smoke, || false);
        assert!(matches!(first, FirstUseVerification::Failed(ref reason) if reason.contains("401")));
        let stored = load(&path).expect("store");
        let model = find_model(&stored, "pi", "zai", "glm-4.7").expect("model");
        assert_eq!(model.verification, VERIFICATION_FAILED);
        assert_eq!(model.status, STATUS_FAILED);
        let second = verify_on_first_use_at(&path, "pi", "zai", "glm-4.7", smoke, || false);
        assert_eq!(second, FirstUseVerification::Skipped);
        assert_eq!(calls.get(), 1);
        let _ = fs::remove_dir_all(root);
    }

    /// 第三方首次派发验证通过：写 verified，第二次派发不再验证。
    #[test]
    fn task09_first_use_success_is_verified_once_only() {
        let root = std::env::temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        fs::write(
            &path,
            r#"{"version":1,"agents":[{"id":"pi","name":"pi","models":[
                {"id":"glm-4.7","agent":"pi","provider":"zai","status":"待冒烟"}]}]}"#,
        )
        .unwrap();
        let calls = std::cell::Cell::new(0u32);
        let smoke = |_: &ResourceModel| -> Result<SmokeSuccess, String> {
            calls.set(calls.get() + 1);
            Ok(SmokeSuccess { identity: "zai/glm-4.7".into(), meta_marker: None })
        };
        assert_eq!(
            verify_on_first_use_at(&path, "pi", "zai", "glm-4.7", smoke, || false),
            FirstUseVerification::Verified
        );
        let stored = load(&path).expect("store");
        let model = find_model(&stored, "pi", "zai", "glm-4.7").expect("model");
        assert_eq!(model.verification, VERIFICATION_VERIFIED);
        assert_eq!(model.smoke_model, "zai/glm-4.7");
        assert_eq!(
            verify_on_first_use_at(&path, "pi", "zai", "glm-4.7", smoke, || false),
            FirstUseVerification::Skipped
        );
        assert_eq!(calls.get(), 1);
        let _ = fs::remove_dir_all(root);
    }

    /// 修正轮 R1-1：claude 挂第三方端点就不算原生——旧库里存着的 native 要
    /// 降回 unverified，首次派发真的会验证；换回官方端点再回到 native、不验证。
    #[test]
    fn r1_claude_on_third_party_endpoint_is_not_native() {
        let root = std::env::temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        let raw = r#"{"version":1,"agents":[{"id":"claude","name":"Claude Code","models":[
            {"id":"sonnet","agent":"claude","status":"待冒烟","verification":"native"}]}]}"#;

        set_claude_third_party(true);
        fs::write(&path, raw).unwrap();
        let store = load(&path).expect("store");
        let model = find_model(&store, "claude", "", "sonnet").expect("model");
        assert_eq!(model.verification, VERIFICATION_UNVERIFIED, "第三方端点的 claude 不是原生");
        let calls = std::cell::Cell::new(0u32);
        let outcome = verify_on_first_use_at(
            &path,
            "claude",
            "",
            "sonnet",
            |_| {
                calls.set(calls.get() + 1);
                Ok(SmokeSuccess { identity: "sonnet".into(), meta_marker: None })
            },
            || false,
        );
        assert_eq!(outcome, FirstUseVerification::Verified);
        assert_eq!(calls.get(), 1);
        assert_eq!(
            find_model(&load(&path).unwrap(), "claude", "", "sonnet").unwrap().verification,
            VERIFICATION_VERIFIED,
            "第三方端点的 claude 冒烟结果要落进 verified"
        );

        set_claude_third_party(false);
        fs::write(&path, raw).unwrap();
        let store = load(&path).expect("store");
        assert_eq!(
            find_model(&store, "claude", "", "sonnet").unwrap().verification,
            VERIFICATION_NATIVE
        );
        let calls = std::cell::Cell::new(0u32);
        let outcome = verify_on_first_use_at(
            &path,
            "claude",
            "",
            "sonnet",
            |_| {
                calls.set(calls.get() + 1);
                Ok(SmokeSuccess { identity: "sonnet".into(), meta_marker: None })
            },
            || false,
        );
        assert_eq!(outcome, FirstUseVerification::Skipped);
        assert_eq!(calls.get(), 0);
        let _ = fs::remove_dir_all(root);
    }

    /// 修正轮 R1-1：agent id 大小写不影响原生判定（前端是 toLowerCase）。
    #[test]
    fn r1_native_agent_match_is_case_insensitive() {
        set_claude_third_party(false);
        assert!(is_native_model("Codex"));
        assert!(is_native_model(" KIMI "));
        assert!(!is_native_model("pi"));
    }

    /// 修正轮 R1-2：provider 为空时退到 agent + id 匹配，真的会验证（原来精确
    /// 匹配落空 → 静默 Skipped，G1 承诺了却一个请求都不发），结果写回原条目。
    #[test]
    fn r1_first_use_verifies_when_provider_is_empty() {
        let root = std::env::temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        fs::write(
            &path,
            r#"{"version":1,"agents":[{"id":"pi","name":"pi","models":[
                {"id":"glm-4.7","agent":"pi","provider":"zai","status":"待冒烟"}]}]}"#,
        )
        .unwrap();
        let calls = std::cell::Cell::new(0u32);
        let outcome = verify_on_first_use_at(
            &path,
            "pi",
            "",
            "glm-4.7",
            |target| {
                calls.set(calls.get() + 1);
                assert_eq!(target.provider, "zai", "要验的是库里那一条的端点");
                Ok(SmokeSuccess { identity: "zai/glm-4.7".into(), meta_marker: None })
            },
            || false,
        );
        assert_eq!(outcome, FirstUseVerification::Verified);
        assert_eq!(calls.get(), 1);
        assert_eq!(
            find_model(&load(&path).unwrap(), "pi", "zai", "glm-4.7").unwrap().verification,
            VERIFICATION_VERIFIED
        );
        let _ = fs::remove_dir_all(root);
    }

    /// 修正轮 R1-2：provider 为空且同名模型挂在多个 provider 下——验哪一个都
    /// 可能不是要派的那一个，返回失败并写明原因，不静默放行。
    /// 库里根本没这一条时返回 NotInStore（放行，但留痕说明没验）。
    #[test]
    fn r1_first_use_reports_ambiguous_provider_and_missing_entry() {
        let root = std::env::temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        fs::write(
            &path,
            r#"{"version":1,"agents":[{"id":"pi","name":"pi","models":[
                {"id":"glm-4.7","agent":"pi","provider":"zai","status":"待冒烟"},
                {"id":"glm-4.7","agent":"pi","provider":"bigmodel","status":"待冒烟"}]}]}"#,
        )
        .unwrap();
        let ambiguous = verify_on_first_use_at(
            &path,
            "pi",
            "",
            "glm-4.7",
            |_| panic!("provider 不确定时不该发请求"),
            || false,
        );
        match ambiguous {
            FirstUseVerification::Failed(reason) => {
                assert!(reason.contains("多个 provider"), "{reason}");
                assert!(reason.contains("zai"), "{reason}");
            }
            other => panic!("期望 Failed，实际 {other:?}"),
        }
        let missing = verify_on_first_use_at(
            &path,
            "pi",
            "",
            "没有这一条",
            |_| panic!("库里没有就不该发请求"),
            || false,
        );
        assert_eq!(missing, FirstUseVerification::NotInStore);
        let _ = fs::remove_dir_all(root);
    }

    /// 修正轮 R1-3：验证窗口内点了叫停——不写库（免得把「被杀」记成验证失败），
    /// 返回 Cancelled 让节点按「已取消」结束。
    #[test]
    fn r1_first_use_cancelled_does_not_write_store() {
        let root = std::env::temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        fs::write(
            &path,
            r#"{"version":1,"agents":[{"id":"pi","name":"pi","models":[
                {"id":"glm-4.7","agent":"pi","provider":"zai","status":"待冒烟"}]}]}"#,
        )
        .unwrap();
        let outcome = verify_on_first_use_at(
            &path,
            "pi",
            "zai",
            "glm-4.7",
            |_| Err("进程已终止".to_string()),
            || true,
        );
        assert_eq!(outcome, FirstUseVerification::Cancelled);
        let stored = load(&path).unwrap();
        let model = find_model(&stored, "pi", "zai", "glm-4.7").unwrap();
        assert_eq!(model.verification, VERIFICATION_UNVERIFIED, "取消不该改档");
        assert!(model.smoke_failure.is_empty(), "取消不该记失败原因");
        let _ = fs::remove_dir_all(root);
    }

    /// 修正轮 R1-4：同一模型两路并发首派只冒烟一次——第二路等第一路写完库
    /// 再读状态，拿到 Skipped，不会各发一次短请求、也不会按写回顺序互相覆盖。
    #[test]
    fn r1_concurrent_first_use_smokes_only_once() {
        let root = std::env::temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        fs::write(
            &path,
            r#"{"version":1,"agents":[{"id":"pi","name":"pi","models":[
                {"id":"glm-4.7","agent":"pi","provider":"zai","status":"待冒烟"}]}]}"#,
        )
        .unwrap();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let path = path.clone();
                let calls = Arc::clone(&calls);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    verify_on_first_use_at(
                        &path,
                        "pi",
                        "zai",
                        "glm-4.7",
                        |_| {
                            calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                            std::thread::sleep(Duration::from_millis(120));
                            Ok(SmokeSuccess { identity: "zai/glm-4.7".into(), meta_marker: None })
                        },
                        || false,
                    )
                })
            })
            .collect();
        let outcomes: Vec<_> = handles.into_iter().map(|handle| handle.join().unwrap()).collect();
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1, "只该冒烟一次");
        assert!(outcomes.contains(&FirstUseVerification::Verified), "{outcomes:?}");
        assert!(outcomes.contains(&FirstUseVerification::Skipped), "{outcomes:?}");
        assert_eq!(
            find_model(&load(&path).unwrap(), "pi", "zai", "glm-4.7").unwrap().verification,
            VERIFICATION_VERIFIED
        );
        let _ = fs::remove_dir_all(root);
    }

    /// 修正轮 R1-4：写回失败不吞——库里还是 unverified，下次会再冒一次烟，
    /// 「失败不自动重试」就不成立了，所以这次派发按失败返回并写明原因。
    #[test]
    fn r1_first_use_write_back_failure_fails_the_dispatch() {
        let root = std::env::temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        fs::write(
            &path,
            r#"{"version":1,"agents":[{"id":"pi","name":"pi","models":[
                {"id":"glm-4.7","agent":"pi","provider":"zai","status":"待冒烟"}]}]}"#,
        )
        .unwrap();
        let removed = path.clone();
        let outcome = verify_on_first_use_at(
            &path,
            "pi",
            "zai",
            "glm-4.7",
            move |_| {
                // 冒烟期间条目被删（用户在接入页移除 / 库被换掉）：写回会落空。
                fs::remove_file(&removed).unwrap();
                Ok(SmokeSuccess { identity: "zai/glm-4.7".into(), meta_marker: None })
            },
            || false,
        );
        match outcome {
            FirstUseVerification::Failed(reason) => {
                assert!(reason.contains("写回资源库失败"), "{reason}")
            }
            other => panic!("期望 Failed，实际 {other:?}"),
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn r3_6_route_alias_smoke_records_actual_id_and_meta_marker() {
        let model = ResourceModel {
            id: "deepseek-v4-flash".into(),
            agent: "pi".into(),
            provider: "deepseek".into(),
            ..ResourceModel::default()
        };
        let marker = route_alias_marker(&model, "deepseek/deepseek-flash").expect("known alias");
        assert_eq!(marker, "路由别名：deepseek-v4-flash→deepseek-flash");
        let key = ResourceModelKey {
            agent_id: "pi".into(),
            provider: "deepseek".into(),
            id: "deepseek-v4-flash".into(),
        };
        let mut store = ResourceStore { version: 1, agents: vec![fixture_agent("pi", vec![model])] };
        apply_smoke_outcome(&mut store, &key, Ok(SmokeSuccess {
            identity: "deepseek/deepseek-flash".into(),
            meta_marker: Some(marker),
        })).unwrap();
        let saved = find_model(&store, "pi", "deepseek", "deepseek-v4-flash").unwrap();
        assert_eq!(saved.smoke_model, "deepseek/deepseek-flash");
        assert!(saved.meta.contains("路由别名：deepseek-v4-flash→deepseek-flash"));
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
