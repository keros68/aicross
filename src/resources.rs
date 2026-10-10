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

use crate::agent_catalog::ModelSource;
use crate::channels;
use crate::paths::aicross_root;
use crate::process::{run_process, run_process_registered, version_with_arg, ProcessResult};
use chrono::{DateTime, Local, SecondsFormat};
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
/// 「未在事实源中」标记：merge_detected_models 打，model_state / 引擎派发前
/// 检查读；crate 内可见是为引擎测试构造夹具。
pub(crate) const MISSING_MARKER: &str = "未在事实源中";
const SMOKE_PROMPT: &str = "只回复 OK";

/// 「未发现」状态的统一说明（TASK-48）：冒烟、选人选、派发的拒绝与跳过
/// 都用这一句，不再各写各的。
pub(crate) const NOT_DETECTED_NOTE: &str = "最近一次探测未发现";

/// 09-15 口径的四态（写进 `ResourceModel::verification`，`aicross resources` 按它显示状态）。
/// TASK-42 起追加 `timeout`：冒烟超时单独记档，不算验证失败、不拦派发。
/// 旧的 `status` 字段保留不删：`resources` 输出的详情里还要看「最近冒烟」那一段。
pub const VERIFICATION_NATIVE: &str = "native";
pub const VERIFICATION_UNVERIFIED: &str = "unverified";
pub const VERIFICATION_VERIFIED: &str = "verified";
pub const VERIFICATION_FAILED: &str = "failed";
pub const VERIFICATION_TIMEOUT: &str = "timeout";

/// 冒烟单次请求的等待上限（TASK-42）：60 秒对慢端点太短，放宽到 180 秒。
pub(crate) const SMOKE_TIMEOUT_SECS: u64 = 180;

const PI_LOGIN_CHECK_TIMEOUT: Duration = Duration::from_secs(10);
const PI_LOGIN_CHECK_CONCURRENCY: usize = 4;

/// pi provider 的本机登录状态。它只记录 `pi auth check` 的就绪结论，
/// 不包含、不读取也不持久化任何凭据字段。
#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResourceLogin {
    #[default]
    Unknown,
    LoggedIn,
    LoggedOut,
}

// 读盘宽松（R1 复审 3）：resources.json 里出现认不得的值（旧版、手改、将来新增）时
// 按「未检查」读，不让整个资源库读取失败；前端口径相同。
impl<'de> Deserialize<'de> for ResourceLogin {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Ok(match raw.as_str() {
            "logged_in" => ResourceLogin::LoggedIn,
            "logged_out" => ResourceLogin::LoggedOut,
            _ => ResourceLogin::Unknown,
        })
    }
}

/// 原生 CLI：自家模型清单说有就是有，不冒烟，只在 agent 级别探一次。
/// 「原生」= 原生 agent **且**没走第三方端点（修正轮 R1）：claude 挂了
/// `ANTHROPIC_BASE_URL`（或 settings.json 的 env）就按第三方走自动验证，
/// 静默降级正是在这条路上高发。判定见 `is_native_model`。
pub(crate) const AGENT_NAME_CLAUDE: &str = "Claude Code";
pub(crate) const AGENT_NAME_CODEX: &str = "Codex";
pub(crate) const AGENT_NAME_KIMI: &str = "Kimi Code";
pub(crate) const AGENT_NAME_PI: &str = "pi agent";

/// 资源库里 agent 的骨架、显示名、是否原生都按支持清单
/// （`agent_catalog::SUPPORTED_AGENTS`）来；上面四个显示名常量与清单一致，
/// 由 `task61_catalog_names_match_resource_constants` 钉住。
const AGENT_KIND_BUILTIN: &str = "内置";

/// 新 agent 的模型厂商按模型名推（TASK-61）：认不出的记「未知」——
/// 不进验收 / 验证位，只能当执行位。
pub(crate) const VENDOR_UNKNOWN: &str = "未知";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ResourceStore {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub agents: Vec<ResourceAgent>,
    /// 最近一次成功写回的探测时间（RFC3339；TASK-48 的追加字段）。旧库没有
    /// 该字段时为 None，`aicross resources` 的 `detected_at` 输出 null。
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "detectedAt")]
    pub detected_at: Option<String>,
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
    /// 命令模板只照官方文档写、没在真机跑通（TASK-61）；按支持清单填。
    #[serde(default)]
    pub untested: bool,
    /// 官网（TASK-61）；资源页「未安装」里给链接。按支持清单填。
    #[serde(default)]
    pub homepage: String,
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
    /// 09-15 口径：native / unverified / verified / failed；TASK-42 起冒烟超时
    /// 另记 timeout。旧库缺这个字段，
    /// `ensure_verification` 按 agent + 旧 `status` 推断后补上。
    #[serde(default)]
    pub verification: String,
    /// pi provider 的登录状态；旧 resources.json 缺失时为 `unknown`。
    #[serde(default)]
    pub login: ResourceLogin,
    /// 最近一次只读登录检查的时间；不写入任何凭据或检查输出。
    #[serde(default)]
    pub login_checked_at: String,
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
    /// 本模型额度用尽时按顺序尝试的 `agent/provider/model` 键。
    #[serde(default)]
    pub fallback_order: Vec<String>,
    /// 已知的额度恢复时间。过期值在读取资源库时自动清掉。
    #[serde(default)]
    pub quota_exhausted_until: Option<String>,
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
    /// 按条目批量操作时跳过未执行的条目与原因（TASK-48：未发现的模型
    /// 不发请求）。跳过不算失败，不进 errors、不影响退出码。
    #[serde(default)]
    pub skipped: Vec<String>,
}

/// TASK-52 `resources prune` 的结果：清理后的资源库、被删的
/// `agent/provider/model` 键，其中被 modes.json 人选或 roles.json 角色
/// 引用的单列在 `referenced`（照样删除，输出里另列说明）。
pub(crate) struct PruneOutcome {
    pub store: ResourceStore,
    pub removed: Vec<String>,
    pub referenced: Vec<String>,
}

/// prune 时判断「被引用」的参照：modes.json 人选 / roles.json 角色里的一个
/// 模型指向（agent, provider, model）。provider 为空表示不限 provider。
pub(crate) type ModelReference = (String, String, String);

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ResourceModelKey {
    pub agent_id: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub provider: String,
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
    /// `hide` 是 codex 自己不在选择器里列出的模型（自动审批审查、旧版保留等）。
    #[serde(default)]
    visibility: String,
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
    rejudge_stored_smoke_failures(&mut store);
    ensure_resolved_id(&mut store);
    Ok(store)
}

fn clear_expired_quota(store: &mut ResourceStore) -> bool {
    let now = Local::now().timestamp_millis();
    let mut changed = false;
    for model in store.agents.iter_mut().flat_map(|agent| &mut agent.models) {
        let expired = model
            .quota_exhausted_until
            .as_deref()
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.timestamp_millis() <= now)
            .unwrap_or(false);
        if expired {
            model.quota_exhausted_until = None;
            changed = true;
        }
    }
    changed
}

pub(crate) fn resource_model_key(agent: &str, provider: &str, model: &str) -> String {
    format!("{agent}/{provider}/{model}")
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn dispatch_models() -> Result<Vec<ResourceModel>, String> {
    let path = store_path()?;
    let _guard = store_lock(&path)?;
    let mut store = load_or_default(&path)?;
    clear_expired_quota(&mut store);
    Ok(store
        .agents
        .into_iter()
        .flat_map(|owner| owner.models)
        .collect())
}

fn mark_quota_exhausted_at(
    path: &Path,
    agent: &str,
    provider: &str,
    model: &str,
    resets_at: Option<&str>,
) -> Result<(), String> {
    let Some(resets_at) = resets_at.filter(|value| DateTime::parse_from_rfc3339(value).is_ok()) else {
        return Ok(());
    };
    let _guard = store_lock(path)?;
    let mut store = load_or_default(path)?;
    let item = find_model_mut(&mut store, agent, provider, model)?;
    item.quota_exhausted_until = Some(resets_at.to_string());
    save_store(path, &store)
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn mark_quota_exhausted(
    agent: &str,
    provider: &str,
    model: &str,
    resets_at: Option<&str>,
) -> Result<(), String> {
    mark_quota_exhausted_at(&store_path()?, agent, provider, model, resets_at)
}

/// agent id 是不是原生三家。大小写不敏感——手工添加的 agent 写成 `Codex`
/// 时，前端（`toLowerCase`）和这里必须判成同一档。
fn is_native_agent(agent: &str) -> bool {
    crate::agent_catalog::find(agent).is_some_and(|entry| entry.native)
}

/// 第三方端点判定。目前只覆盖 claude（进程环境 `ANTHROPIC_BASE_URL` 或
/// `~/.claude/settings.json` 的 `env.ANTHROPIC_BASE_URL`）——codex 的
/// `model_provider` 与 kimi 的 `config.toml` base_url 本期没有事实源读法，
/// 暂不覆盖，手册里写明这一点。
fn third_party_endpoint(agent: &str) -> bool {
    agent.trim().eq_ignore_ascii_case("claude") && claude_third_party_endpoint()
}

/// 「原生」的完整判据：原生 agent 且没走第三方端点。
/// 这条同时是 `verification == "native"` 的不变式——引擎按它决定跳不跳过首次验证，
/// `aicross resources` 按它标注原生模型不冒烟。
pub(crate) fn is_native_model(agent: &str) -> bool {
    is_native_agent(agent) && !third_party_endpoint(agent)
}

/// 原生模型一般不冒烟；agy 例外（TASK-61）：资源页「测试」走一次最小派发，
/// 回答非空即通过——无头下工具全被拒时回答为空，读模型清单看不出来。
/// 结果只写 status 与冒烟详情，验证档仍是 native。
pub(crate) fn native_smoke_dispatches(agent: &str) -> bool {
    agent.trim().eq_ignore_ascii_case("agy")
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

/// 旧库里可能保留着静态路由表加入前的「真身不符」。读取时只按当前静态表
/// 重判已有结果，不启动 CLI：命中已知别名就翻成已验证；其余有失败原因的
/// 第三方记录统一归为 failed，不再与从未验证共用 unverified。
fn rejudge_stored_smoke_failures(store: &mut ResourceStore) {
    static IDENTITY_MISMATCH: OnceLock<Regex> = OnceLock::new();
    let mismatch = IDENTITY_MISMATCH.get_or_init(|| {
        Regex::new(
            r"^真身不符：请求\s+([^/\s，]+)/([^，\s]+)，响应\s+([^/\s，]+)/([^，\s]+)\s*$",
        )
        .expect("identity mismatch regex")
    });
    let date = Local::now().format("%Y-%m-%d").to_string();

    for agent in &mut store.agents {
        let owner = agent.id.clone();
        for model in &mut agent.models {
            if model.smoke_failure.trim().is_empty() {
                continue;
            }
            let agent_id = if model.agent.trim().is_empty() { &owner } else { &model.agent };
            if is_native_model(agent_id) {
                continue;
            }

            let known_alias = mismatch.captures(model.smoke_failure.trim()).and_then(|captures| {
                let requested_provider = captures.get(1)?.as_str();
                let requested_model = captures.get(2)?.as_str();
                let actual_provider = captures.get(3)?.as_str();
                let actual_model = captures.get(4)?.as_str();
                crate::channels::KNOWN_MODEL_ROUTES
                    .iter()
                    .any(|(provider, requested, actual)| {
                        *provider == requested_provider
                            && *requested == requested_model
                            && *provider == actual_provider
                            && *actual == actual_model
                            && model.provider == requested_provider
                            && model.id == requested_model
                    })
                    .then(|| (actual_provider.to_string(), actual_model.to_string()))
            });

            if let Some((actual_provider, actual_model)) = known_alias {
                model.status = STATUS_CONNECTED.to_string();
                model.verification = VERIFICATION_VERIFIED.to_string();
                model.resolved_id = actual_model.clone();
                model.resolved_source = RESOLVED_SOURCE_IDENTITY.to_string();
                model.smoke_model = format!("{actual_provider}/{actual_model}");
                model.smoke_failure.clear();
                model.note = append_note_marker(
                    &model.note,
                    &format!("按已知路由别名重判（{date}）"),
                );
            } else if model.verification != VERIFICATION_VERIFIED
                && model.verification != VERIFICATION_TIMEOUT
            {
                // 已验证的记录不因为留着旧失败原因被每次加载静默降档
                // （09-16 复审 GLM 2；当前写路径造不出这个状态，纯消隐患）；
                // 超时档同理不按失败降档（TASK-42）。
                model.status = STATUS_FAILED.to_string();
                model.verification = VERIFICATION_FAILED.to_string();
            }
        }
    }
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
            detected_at: None,
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

/// 写回资源库（原子写）。命令行与测试同用。
pub(crate) fn save_store(path: &Path, store: &ResourceStore) -> Result<(), String> {
    write_json_atomic(path, store)
}

/// 资源库锁的等待上限（fix-fullcheck B1）。持锁区段只做读 → 改 → 原子写，
/// 毫秒级；冒烟、探测、cc-switch 提示等外部进程一律在锁外跑。持锁区段里
/// 加入秒级操作时同步调大。
const STORE_LOCK_TIMEOUT: Duration = Duration::from_secs(40);

/// 资源库写锁：进程内互斥加跨进程系统文件锁（锁住常驻的
/// `resources.json.lock`）。窗口、命令行与后台 `__run` 是不同进程，所有
/// load → 改 → save 序列都必须持有它，防止读改写互相覆盖（前端按钮 busy
/// 只是 UI 纪律，不是后端保证）。
struct StoreGuard {
    // 字段按声明顺序释放：先放进程内互斥，再关锁文件句柄。
    _mutex: MutexGuard<'static, ()>,
    _file: crate::file_lock::FileLock,
}

/// 先拿文件锁再拿进程内互斥：等别的进程放锁期间不占着进程内互斥。
fn store_lock(path: &Path) -> Result<StoreGuard, String> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "resources.json".into());
    let file = crate::file_lock::FileLock::acquire(
        &path.with_file_name(format!("{name}.lock")),
        STORE_LOCK_TIMEOUT,
    )?;
    let mutex = store_mutex()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    Ok(StoreGuard {
        _file: file,
        _mutex: mutex,
    })
}

fn store_mutex() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// 旧口径的「已接入」清单（status == 已接入）。cc-switch 一律排除。
// 08b 起派发清单改列全部探测到的模型（`detected`）；`aicross resources` 走
// `get_resources` 直接拿整库；`connected` 只留作「已接入」语义的查询入口，暂无生产调用。
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
/// → 验证失败（冒烟超时可重试，随未验证同档，TASK-42），同一档内
/// agent → provider → id。
pub fn detected(path: &Path) -> Result<Vec<ResourceModel>, String> {
    let mut store = load(path)?;
    clear_expired_quota(&mut store);
    let mut models: Vec<ResourceModel> = store
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
        // 超时可重试，不是坏档：与未验证同档（TASK-42）
        VERIFICATION_UNVERIFIED | VERIFICATION_TIMEOUT => 1,
        VERIFICATION_FAILED => 2,
        _ => 3,
    }
}

/// 保证支持清单里每个 agent 的骨架存在（没装的也列，`detected: false`、
/// 无模型，界面据此折叠）；显示名、`untested`、`homepage` 按清单刷新；
/// 迁移掉历史 store 里的 cc-switch agent（已降级为发现提示）；回填
/// model.agent 冗余字段。
pub fn ensure_agents(store: &mut ResourceStore) {
    store.version = 1;
    store.agents.retain(|agent| agent.id != "cc-switch");
    for entry in crate::agent_catalog::SUPPORTED_AGENTS {
        if store.agents.iter().all(|agent| agent.id != entry.id) {
            store.agents.push(ResourceAgent {
                id: entry.id.to_string(),
                name: entry.name.to_string(),
                kind: AGENT_KIND_BUILTIN.to_string(),
                detected: false,
                version: "未检测".to_string(),
                models: Vec::new(),
                untested: entry.untested,
                homepage: entry.homepage.to_string(),
            });
        }
        if let Some(agent) = store.agents.iter_mut().find(|agent| agent.id == entry.id) {
            agent.name = entry.name.to_string();
            agent.untested = entry.untested;
            agent.homepage = entry.homepage.to_string();
            if agent.kind.trim().is_empty() {
                agent.kind = AGENT_KIND_BUILTIN.to_string();
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
    // 支持清单里的 agent 统一走 `agent_catalog::resolve_program`（兜底路径
    // 优先，再查 PATH）：kimi 要原生 exe，agy 在 Windows 上 PATH 里常常没有；
    // 派发与冒烟对 agy 用同一个函数，三处结果一致。
    if let Some(entry) = crate::agent_catalog::SUPPORTED_AGENTS
        .iter()
        .find(|entry| entry.command == name)
    {
        return crate::agent_catalog::resolve_program(entry.id);
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
    detect_version_with(binary, "--version")
}

fn detect_version_with(binary: Option<&PathBuf>, version_arg: &str) -> (bool, String) {
    match binary {
        Some(path) => (
            true,
            version_with_arg(path, version_arg).unwrap_or_else(|| "已检测".to_string()),
        ),
        None => (false, "未检测".to_string()),
    }
}

// ── 探测 ──────────────────────────────────────────────────────────────────

/// pi 的探测（版本、模型清单、provider 登录检查）必须先在资源库锁外
/// 完成（探测一次最长几十秒，不能把其他写方挡在锁外），走 Pi 专用准备；
/// 其余 agent 走通用准备（TASK-57 走查 #3 起全部 agent 都在锁外准备）。
fn uses_pi_prepared_detection(agent_id: &str) -> bool {
    agent_id == "pi"
}

/// 一个 agent 的锁外探测准备结果：跑版本与模型清单子进程，不碰资源库；
/// 锁内由 `apply_prepared_detection` 合并。pi 与其余 agent 各走一档。
/// `Failed` 是探测线程 panic 的占位结果（fix-57-r1 1）：锁内只记一条
/// 错误，不合并该 agent，其余 agent 照常。
enum PreparedDetection {
    Pi(PreparedPiDetection),
    Plain(PreparedAgentDetection),
    Failed(String),
}

impl PreparedDetection {
    fn agent_id(&self) -> &str {
        match self {
            PreparedDetection::Pi(_) => "pi",
            PreparedDetection::Plain(prepared) => &prepared.agent_id,
            PreparedDetection::Failed(agent_id) => agent_id,
        }
    }
}

/// 非 pi agent 的锁外探测准备：版本检测结果、失败说明与模型清单。
/// `models` 为 None 表示不合并（手动 agent 没有事实源可读）。
struct PreparedAgentDetection {
    agent_id: String,
    detected: bool,
    version: String,
    missing_error: Option<String>,
    models: Option<Result<Vec<ResourceModel>, String>>,
}

fn prepare_agent_detection(agent_id: &str) -> PreparedDetection {
    if uses_pi_prepared_detection(agent_id) {
        return PreparedDetection::Pi(prepare_pi_detection(resolve_cli("pi")));
    }
    PreparedDetection::Plain(prepare_plain_detection(agent_id))
}

fn prepare_plain_detection(agent_id: &str) -> PreparedAgentDetection {
    // 手动 agent：二进制来自 tools.json 里的手动配置，没有事实源可读。
    if agent_id.starts_with("manual-") {
        let entry = crate::tool_config::get(agent_id);
        let (binary, _) = crate::tool_config::parse_command(&entry.command);
        let resolved = binary.as_deref().and_then(resolve_cli);
        let (detected, version) = detect_version(resolved.as_ref());
        return PreparedAgentDetection {
            agent_id: agent_id.to_string(),
            detected,
            version,
            missing_error: (!detected).then(|| {
                format!(
                    "探测 {agent_id} 失败：未检测到 {}",
                    binary.as_deref().unwrap_or("（未配置二进制）")
                )
            }),
            models: None,
        };
    }

    let entry = crate::agent_catalog::find(agent_id);
    let command = entry.map_or(agent_id, |entry| entry.command);
    let resolved = resolve_cli(command);
    let (detected, version) =
        detect_version_with(resolved.as_ref(), entry.map_or("--version", |entry| entry.version_arg));
    // 支持清单里固定四家以外的 agent 没装是常态：只记未安装，不报错。
    let mut missing_error = (!detected && entry.map_or(true, |entry| entry.core))
        .then(|| format!("探测 {agent_id} 失败：未检测到 {command}"));
    let source = entry.map(|entry| entry.models);
    let models = match (source, resolved.as_deref()) {
        // claude 的模型是静态预设，未检测时没有事实源可读；仍走一次
        // 空合并，只给既有条目刷「未在事实源中」标记（与 codex/kimi
        // 走同一条合并路径），不新增预设条目。新增的 agent 同理：
        // 没装就没有清单可读。
        (Some(ModelSource::ClaudeHelp), _) if !detected => Some(Ok(Vec::new())),
        (Some(ModelSource::ClaudeHelp), _) => Some(Ok(claude_presets(resolved.as_ref()))),
        (Some(ModelSource::CodexCache), _) => Some(detect_codex()),
        (Some(ModelSource::KimiConfig), _) => Some(detect_kimi()),
        (Some(ModelSource::AgyModels), Some(binary)) => Some(detect_agy(binary)),
        (Some(ModelSource::CursorModels), Some(binary)) => {
            let (models, error) = cursor_models_or_mark_missing(detect_cursor(binary));
            if error.is_some() {
                missing_error = error;
            }
            models
        }
        (Some(ModelSource::StaticTable), Some(_)) => Some(static_table_models(agent_id)),
        (Some(ModelSource::QoderTiers), Some(_)) => Some(Ok(qoder_tier_models())),
        _ => Some(Ok(Vec::new())),
    };
    PreparedAgentDetection {
        agent_id: agent_id.to_string(),
        detected,
        version,
        missing_error,
        models,
    }
}

/// 锁内应用一个 agent 的探测结果（TASK-57 走查 #3）。
fn apply_prepared_detection(
    store: &mut ResourceStore,
    prepared: PreparedDetection,
    errors: &mut Vec<String>,
) {
    match prepared {
        PreparedDetection::Pi(prepared) => apply_prepared_pi_detection(store, prepared, errors),
        PreparedDetection::Plain(prepared) => apply_prepared_plain_detection(store, prepared, errors),
        PreparedDetection::Failed(agent_id) => {
            errors.push(format!("探测 {agent_id} 失败：线程异常"));
        }
    }
}

fn apply_prepared_plain_detection(
    store: &mut ResourceStore,
    prepared: PreparedAgentDetection,
    errors: &mut Vec<String>,
) {
    if store.agents.iter().all(|agent| agent.id != prepared.agent_id) {
        errors.push(format!("探测资源失败：未知 agent：{}", prepared.agent_id));
        return;
    }
    if let Some(error) = prepared.missing_error {
        errors.push(error);
    }
    set_agent_detection(store, &prepared.agent_id, prepared.detected, &prepared.version);
    match prepared.models {
        Some(Ok(models)) => merge_detected_models(store, &prepared.agent_id, models),
        Some(Err(error)) => errors.push(format!("探测 {} 失败：{error}", prepared.agent_id)),
        None => {}
    }
}

struct PreparedPiDetection {
    detected: bool,
    version: String,
    result: Option<Result<(Vec<ResourceModel>, Vec<String>), String>>,
}

fn prepare_pi_detection(binary: Option<PathBuf>) -> PreparedPiDetection {
    let (detected, version) = detect_version(binary.as_ref());
    let result = binary.as_deref().map(detect_pi);
    PreparedPiDetection { detected, version, result }
}

fn apply_prepared_pi_detection(
    store: &mut ResourceStore,
    prepared: PreparedPiDetection,
    errors: &mut Vec<String>,
) {
    set_agent_detection(store, "pi", prepared.detected, &prepared.version);
    match prepared.result {
        Some(Ok((models, provider_errors))) => {
            merge_detected_models(store, "pi", models);
            errors.extend(provider_errors);
        }
        Some(Err(error)) => errors.push(format!("探测 pi 失败：{error}")),
        None => {
            errors.push("探测 pi 失败：未检测到 pi".to_string());
            if let Some(agent) = store.agents.iter_mut().find(|agent| agent.id == "pi") {
                let checked_at = Local::now().to_rfc3339_opts(SecondsFormat::Secs, false);
                for model in &mut agent.models {
                    model.login = ResourceLogin::Unknown;
                    model.login_checked_at.clone_from(&checked_at);
                }
            }
        }
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
    let named = cache
        .models
        .into_iter()
        .filter(|model| !model.slug.trim().is_empty())
        .collect::<Vec<_>>();
    if named.is_empty() {
        return Err("模型缓存没有有效模型".to_string());
    }
    // 隐藏的不列；全部隐藏时是成功读出的空清单（fix-fullcheck B6），合并时
    // 旧条目标未发现，不当解析失败保留旧清单。
    let mut listed = named
        .into_iter()
        .filter(|model| model.visibility.trim() != "hide")
        .collect::<Vec<_>>();
    listed.sort_by_key(|model| model.priority.unwrap_or(i64::MAX));
    let models = listed
        .into_iter()
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
    Ok(models)
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
            AGENT_NAME_KIMI,
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

// ── 支持清单里的新 agent（TASK-61）────────────────────────────────────────

/// 列模型命令的等待上限：`agy models` 要先联网拉清单。
const MODEL_LIST_TIMEOUT: Duration = Duration::from_secs(30);

/// 列模型命令跑一次：超时或非零退出都算失败，返回 stdout。
fn run_model_list(binary: &Path, args: &[&str], label: &str) -> Result<String, String> {
    let args = args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
    let listed = run_process(binary, &args, None, MODEL_LIST_TIMEOUT, None)?;
    if listed.timed_out || listed.code.is_some_and(|code| code != 0) {
        return Err(format!("{label} {}", process_error_summary(&listed)));
    }
    Ok(listed.stdout)
}

/// 新 agent 的模型厂商按模型名推：复用 `infer_vendor`，认不出的记「未知」
/// （cursor 的 `auto`、qoder 的档位名）。
pub(crate) fn vendor_for_listed_model(id: &str) -> &'static str {
    match infer_vendor(id) {
        "其他" => VENDOR_UNKNOWN,
        vendor => vendor,
    }
}

fn listed_model(id: &str, agent: &str, thinking: &str, source: &str, meta: &str) -> ResourceModel {
    detected_model(
        id,
        agent,
        "",
        vendor_for_listed_model(id),
        "官方订阅",
        thinking,
        source,
        meta,
    )
}

fn detect_agy(binary: &Path) -> Result<Vec<ResourceModel>, String> {
    let stdout = run_model_list(binary, &["models"], "agy models")?;
    parse_agy_models(&stdout, &source_read("agy models"))
}

/// `agy models`（1.3.1 实测）：首行 `Fetching available models...`，其后每行
/// 「id<TAB>显示名」。不含 TAB 的行一律跳过。
fn parse_agy_models(stdout: &str, source: &str) -> Result<Vec<ResourceModel>, String> {
    let models = stdout
        .lines()
        .filter_map(|line| {
            let (id, name) = line.split_once('\t')?;
            let id = id.trim();
            (!id.is_empty() && !id.contains(' ')).then(|| {
                listed_model(id, "agy", "支持", source, name.trim())
            })
        })
        .collect::<Vec<_>>();
    if models.is_empty() {
        Err("agy models 没有列出模型".to_string())
    } else {
        Ok(models)
    }
}

/// fix-61-r1 7：`cursor-agent models` 解析失败时按「事实源里没有」合并空清单，
/// 已有模型由 `merge_detected_models` 标「未在事实源中」、不再可选，同时写一条错误。
/// 固定四家的失败处理不走这里（它们失败时不合并，保留原状）。
fn cursor_models_or_mark_missing(
    listed: Result<Vec<ResourceModel>, String>,
) -> (Option<Result<Vec<ResourceModel>, String>>, Option<String>) {
    match listed {
        Ok(models) => (Some(Ok(models)), None),
        Err(error) => (Some(Ok(Vec::new())), Some(format!("探测 cursor 失败：{error}"))),
    }
}

fn detect_cursor(binary: &Path) -> Result<Vec<ResourceModel>, String> {
    let stdout = run_model_list(binary, &["models"], "cursor-agent models")?;
    parse_cursor_models(&stdout, &source_read("cursor-agent models"))
}

/// `cursor-agent models`（未实测，按官方文档的「id - 显示名」行解析，去掉
/// `(current)` / `(default)` 标注）。一行都解析不出就报错、清单留空。
fn parse_cursor_models(stdout: &str, source: &str) -> Result<Vec<ResourceModel>, String> {
    let models = stdout
        .lines()
        .filter_map(|line| {
            let (id, name) = line.trim().split_once(" - ")?;
            let id = id.trim();
            if id.is_empty() || id.contains(char::is_whitespace) {
                return None;
            }
            let name = name
                .replace("(current)", "")
                .replace("(default)", "")
                .trim()
                .to_string();
            Some(listed_model(id, "cursor", "未知", source, &name))
        })
        .collect::<Vec<_>>();
    if models.is_empty() {
        Err("cursor-agent models 的输出解析不出模型".to_string())
    } else {
        Ok(models)
    }
}

/// 没有可靠列模型命令的 agent 用的小表：随版本发布，每家注明来源与日期。
const AGENT_MODELS_TEXT: &str = include_str!("data/agent-models.json");

fn static_table_models(agent_id: &str) -> Result<Vec<ResourceModel>, String> {
    parse_static_table(AGENT_MODELS_TEXT, agent_id)
}

fn parse_static_table(text: &str, agent_id: &str) -> Result<Vec<ResourceModel>, String> {
    let table: Value =
        serde_json::from_str(text).map_err(|error| format!("内置模型表解析失败：{error}"))?;
    let entry = table
        .get("agents")
        .and_then(|agents| agents.get(agent_id))
        .ok_or_else(|| format!("内置模型表里没有 {agent_id}"))?;
    let source = source_value(
        &format!(
            "内置模型表（{}）",
            entry.get("source").and_then(Value::as_str).unwrap_or("未注明")
        ),
        entry.get("date").and_then(Value::as_str).unwrap_or("未注明"),
    );
    let models = entry
        .get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .map(|id| listed_model(id.trim(), agent_id, "未知", &source, ""))
        .collect::<Vec<_>>();
    if models.is_empty() {
        Err(format!("内置模型表里 {agent_id} 没有模型"))
    } else {
        Ok(models)
    }
}

/// Qoder 只能选档位，不能点名模型：固定五档，厂商一律「未知」。
const QODER_TIERS: [&str; 5] = ["lite", "efficient", "auto", "performance", "ultimate"];

fn qoder_tier_models() -> Vec<ResourceModel> {
    let source = source_value("Qoder 固定档位", "2026-10-07");
    QODER_TIERS
        .iter()
        .map(|tier| {
            let mut model = listed_model(tier, "qoder", "未知", &source, "档位，不对应具体模型");
            model.vendor = VENDOR_UNKNOWN.to_string();
            model
        })
        .collect()
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
        .map(|model| model.provider.as_str())
        .filter(|provider| !provider.trim().is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    for (provider, login) in check_pi_provider_logins(binary, providers) {
        apply_pi_provider_login(&mut models, &provider, login);
    }
    Ok((models, Vec::new()))
}

/// 对同一 provider 只运行一次检查。每一批至多四个子进程，避免 provider
/// 较多时把本机进程槽占满；批内结果按 provider 归并，所以调用方不依赖线程顺序。
fn check_pi_provider_logins(binary: &Path, providers: Vec<String>) -> Vec<(String, ResourceLogin)> {
    check_pi_provider_logins_with(binary, providers, PI_LOGIN_CHECK_TIMEOUT)
}

fn check_pi_provider_logins_with(
    binary: &Path,
    providers: Vec<String>,
    timeout: Duration,
) -> Vec<(String, ResourceLogin)> {
    let binary = binary.to_path_buf();
    check_pi_provider_logins_with_runner(
        providers,
        Arc::new(move |provider| check_pi_provider_login_with(&binary, provider, timeout)),
    )
}

fn check_pi_provider_logins_with_runner(
    mut providers: Vec<String>,
    runner: Arc<dyn Fn(&str) -> ResourceLogin + Send + Sync>,
) -> Vec<(String, ResourceLogin)> {
    providers.sort();
    providers.dedup();
    let mut checked = Vec::with_capacity(providers.len());
    for group in providers.chunks(PI_LOGIN_CHECK_CONCURRENCY) {
        let handles = group
            .iter()
            .cloned()
            .map(|provider| {
                let runner = Arc::clone(&runner);
                std::thread::spawn(move || {
                    let login = runner(&provider);
                    (provider, login)
                })
            })
            .collect::<Vec<_>>();
        checked.extend(handles.into_iter().filter_map(|handle| handle.join().ok()));
    }
    checked.sort_by(|left, right| left.0.cmp(&right.0));
    checked
}

/// 只读 pi 的 exit code 与顶层 `status`（`ready` 布尔仅作兼容）。不要改成读取
/// `--credentials` 或把整份 JSON 收进 `Value`：鉴权资料从不属于资源库契约。
fn check_pi_provider_login_with(binary: &Path, provider: &str, timeout: Duration) -> ResourceLogin {
    let result = run_process(
        binary,
        &[
            "auth".to_string(),
            "check".to_string(),
            "--provider".to_string(),
            provider.to_string(),
            "--json".to_string(),
            "--no-refresh".to_string(),
        ],
        None,
        timeout,
        None,
    );
    pi_login_from_check(result)
}

/// `Result` 的错误（命令不存在等）和超时都是未知，不把错误输出保存或上送；
/// 有明确 `status: not_ready`（或兼容形状 `ready: false`）才判未登录。
/// 退出码是就绪判据的一部分：即使正文是 `status: ready`，非 0 退出码仍判
/// 未登录，因为 provider 已返回可解析的认证结论，但检查命令没有成功完成；
/// 「未检查」只留给命令不可用、超时或没有可解析结论的情况。
fn pi_login_from_check(result: Result<ProcessResult, String>) -> ResourceLogin {
    let Ok(result) = result else { return ResourceLogin::Unknown };
    if result.timed_out {
        return ResourceLogin::Unknown;
    }
    match pi_auth_ready(&result) {
        Some(true) if result.code == Some(0) => ResourceLogin::LoggedIn,
        Some(_) => ResourceLogin::LoggedOut,
        None => ResourceLogin::Unknown,
    }
}

/// 反序列化时只声明顶层 readiness，不构造或访问其他 JSON 字段（包括
/// `credentials`）。pi 有时输出 JSONL，逐行时也沿用同一窄结构。
// 真机（pi 0.85，09-15 实测）输出的是 `{"status":"ready"|"not_ready","provider":…,"authType":…}`，
// `ready` 布尔只作兼容形态；`credentials` 与 `reason` 字段不声明、不读取。
#[derive(Deserialize)]
struct PiAuthReady {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    ready: Option<bool>,
}

fn pi_auth_ready(result: &ProcessResult) -> Option<bool> {
    let ready = |text: &str| {
        let parsed = serde_json::from_str::<PiAuthReady>(text.trim()).ok()?;
        match parsed.status.as_deref() {
            Some("ready") => Some(true),
            Some("not_ready") => Some(false),
            Some(_) => None,
            None => parsed.ready,
        }
    };
    ready(&result.stdout).or_else(|| result.stdout.lines().find_map(ready))
}

fn apply_pi_provider_login(models: &mut [ResourceModel], provider: &str, login: ResourceLogin) {
    let checked_at = Local::now().to_rfc3339_opts(SecondsFormat::Secs, false);
    for model in models.iter_mut().filter(|model| model.provider == provider) {
        model.login = login.clone();
        model.login_checked_at.clone_from(&checked_at);
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
                        source_value("claude --help 别名", &Local::now().format("%Y-%m-%d").to_string()),
                    );
                }
            }
        }
    }
    (
        CLAUDE_FALLBACK_ALIASES.iter().map(|a| a.to_string()).collect(),
        source_value("静态别名表", CLAUDE_STATIC_TABLE_DATE),
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
    source_value(
        &format!("读取 {}", shorten_home_path(source)),
        &Local::now().format("%Y-%m-%d").to_string(),
    )
}

fn source_value(description: &str, date: &str) -> String {
    format!("来源：{description} · {date}")
}

fn shorten_home_path(source: &str) -> String {
    static WINDOWS_HOME: OnceLock<Regex> = OnceLock::new();
    static UNIX_HOME: OnceLock<Regex> = OnceLock::new();
    let shortened = WINDOWS_HOME
        .get_or_init(|| Regex::new(r"(?i)[A-Z]:[\\/]Users[\\/][^\\/]+([\\/]|$)").expect("Windows home regex"))
        .replace(source, "~$1")
        .into_owned();
    UNIX_HOME
        .get_or_init(|| Regex::new(r"/(?:Users|home)/[^/]+(/|$)").expect("Unix home regex"))
        .replace(&shortened, "~$1")
        .into_owned()
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
        login: ResourceLogin::Unknown,
        login_checked_at: String::new(),
        resolved_id: resolved_id_for(agent, id),
        resolved_source: RESOLVED_SOURCE_ALIAS_TABLE.to_string(),
        source: source.to_string(),
        smoked_at: String::new(),
        smoke_model: String::new(),
        smoke_failure: String::new(),
        meta: meta.to_string(),
        note: String::new(),
        fallback_order: Vec::new(),
        quota_exhausted_until: None,
    }
}

fn model_key(model: &ResourceModel) -> (&str, &str, &str) {
    (&model.agent, &model.provider, &model.id)
}

/// 合并事实源条目：已存在的保留用户字段（tier/status/冒烟结果/vendor/note），
/// 新条目进「待冒烟」；消失的标「未在事实源中」（申报条目不标）。
/// 同一 provider 内按事实源给出的顺序排列（codex 缓存按 priority 排，推荐在
/// 同分时取靠前者）；事实源里没有的条目排在后面、按 id。
fn merge_detected_models(store: &mut ResourceStore, agent_id: &str, discovered: Vec<ResourceModel>) {
    let Some(agent) = store.agents.iter_mut().find(|agent| agent.id == agent_id) else {
        return;
    };
    let discovered_keys = discovered
        .iter()
        .enumerate()
        .map(|(index, model)| {
            (
                (
                    model.agent.clone(),
                    model.provider.clone(),
                    model.id.clone(),
                ),
                index,
            )
        })
        .collect::<HashMap<_, _>>();

    for mut model in discovered {
        if let Some(existing) = agent
            .models
            .iter_mut()
            .find(|existing| model_key(existing) == model_key(&model))
        {
            model.tier.clone_from(&existing.tier);
            model.status.clone_from(&existing.status);
            model.verification.clone_from(&existing.verification);
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
            model.fallback_order.clone_from(&existing.fallback_order);
            model.quota_exhausted_until.clone_from(&existing.quota_exhausted_until);
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
        if !discovered_keys.contains_key(&key)
            && !existing.source.starts_with("申报(")
            && !existing.source.starts_with("来源：用户申报 · ")
        {
            existing.meta = append_note_marker(&existing.meta, MISSING_MARKER);
        }
    }
    let order = |model: &ResourceModel| {
        discovered_keys
            .get(&(model.agent.clone(), model.provider.clone(), model.id.clone()))
            .copied()
            .unwrap_or(usize::MAX)
    };
    agent.models.sort_by(|left, right| {
        (&left.provider, order(left), &left.id).cmp(&(&right.provider, order(right), &right.id))
    });
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

/// 手动冒烟：不进进程注册表（冒烟是一发短请求，无需叫停入口）。
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
        Duration::from_secs(SMOKE_TIMEOUT_SECS),
    )?;
    // agy 在本次调用专属的空目录里跑（fix-61-r1 3），返回前（含失败、超时）
    // 由 SmokeScratch 的 Drop 删掉；其余 agent 照旧在系统临时目录跑。
    let scratch = match aicross_root() {
        Ok(root) => SmokeScratch::create_in(&root, &model.agent)?,
        Err(_) => None,
    };
    let temp = std::env::temp_dir();
    let cwd = scratch.as_ref().map_or(temp.as_path(), |scratch| scratch.path.as_path());
    let result = run_process_registered(
        &invocation.program,
        &invocation.args,
        invocation.input.as_deref(),
        Duration::from_secs(SMOKE_TIMEOUT_SECS),
        Some(cwd),
        registry_key,
    )?;
    drop(scratch);
    // 超时先于输出解析拦截（TASK-42）：进程已被终止，解析残余输出只会造出
    // 看不懂的失败原因。这条错误由 apply_smoke_outcome 记成 timeout 档。
    if result.timed_out {
        return Err(smoke_timeout_error());
    }
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

/// 冒烟专属空目录（fix-61-r1 3）：只给只看得到文字的 agent（agy）建，位置
/// `$AICROSS_HOME/scratch/smoke-<agent>-<uuid>`；离开作用域即删除。
struct SmokeScratch {
    path: PathBuf,
}

impl SmokeScratch {
    fn create_in(root: &Path, agent: &str) -> Result<Option<SmokeScratch>, String> {
        if !crate::agent_catalog::is_text_only(agent) {
            return Ok(None);
        }
        let path = root
            .join("scratch")
            .join(format!("smoke-{}-{}", agent.trim().to_ascii_lowercase(), Uuid::new_v4()));
        fs::create_dir_all(&path)
            .map_err(|error| format!("创建冒烟目录失败 {}：{error}", path.display()))?;
        Ok(Some(SmokeScratch { path }))
    }
}

impl Drop for SmokeScratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn route_alias_marker(model: &ResourceModel, identity: &str) -> Option<String> {
    crate::channels::KNOWN_MODEL_ROUTES.iter()
        .find(|(p, req, actual)| {
            *p == model.provider && *req == model.id && identity == format!("{p}/{actual}")
        })
        .map(|(_, req, actual)| format!("路由别名：{req}→{actual}"))
}

/// 冒烟超时的失败正文（TASK-42）。`apply_smoke_outcome` 按它把 `verification`
/// 记成 `timeout` 而不是 `failed`——超时不算验证失败，不拦派发。
fn smoke_timeout_error() -> String {
    format!("冒烟超时（{SMOKE_TIMEOUT_SECS} 秒）")
}

fn is_smoke_timeout(error: &str) -> bool {
    error == smoke_timeout_error()
}

/// 把一次冒烟结果写进 store：成功 → 已接入 + 真身 + 可选 meta 标记；
/// 失败 → 冒烟失败 + 脱敏原因；超时 → 冒烟失败 + 脱敏原因，但 `verification`
/// 记 `timeout`（原生模型恒为 native，TASK-42）。
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
            } else if is_smoke_timeout(&error) {
                // 超时单独记档（TASK-42）：派发前检查只拦 failed，timeout 放行
                VERIFICATION_TIMEOUT
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

/// 首次派发自动验证的结果（引擎按它决定继续派发还是直接判失败）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FirstUseVerification {
    /// 不需要动手：原生模型、验过的、验证超时过的。
    Skipped,
    /// 资源库里这一条已验证失败或最近一次探测未发现：不验证、不派发，附原因
    /// （[`dispatch_block_reason`]，fix-fullcheck B2 / B3）。
    Rejected(String),
    /// 资源库里没有这一条（或库读不出来）：本次不验证，留痕写明，不挡派发。
    /// 与 `Skipped` 分开，事后才分得清「原生跳过」和「库里没这条所以没验」。
    NotInStore,
    /// 本次真发了一个短请求并核对了真身，结果已写回资源库。
    Verified,
    /// 本次验证失败（或验过了但结果写不回资源库），附脱敏后的原因。
    Failed(String),
    /// 首次验证请求命中额度限制；不把模型写成验证失败，由派发层切备选。
    QuotaExhausted { resets_at: Option<String> },
    /// 验证窗口内用户点了叫停：不写库、不继续派发。
    Cancelled,
}

/// 第三方端点首次派发前的自动验证（09-15 定案）。只有 `verification ==
/// "unverified"` 才动手，判据与写回完全复用手动冒烟那一套。
///
/// 失败的不自动重试：这一条派发按失败返回，用户可用 `aicross resources smoke` 手动重验。
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
            // 全都不是「未验证」时不需要动手：与 find_target_model 同一优先序
            // 挑一条（没未发现、没验证失败的在前），一条失败的同名条目不牵连
            // 可用的。
            if candidates
                .iter()
                .all(|model| model.verification != VERIFICATION_UNVERIFIED)
            {
                let preferred = candidates
                    .iter()
                    .find(|m| !is_missing_from_detection(m) && m.verification != VERIFICATION_FAILED)
                    .or_else(|| candidates.iter().find(|m| !is_missing_from_detection(m)))
                    .unwrap_or(&candidates[0]);
                return FirstUseTarget::Hit(key_of(preferred));
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
    // 读一次拿目标就放锁：一次冒烟最长 180 秒，整段持锁会把界面的读资源也堵住。
    let key = {
        // 只读：原子写保证读到整份文件，拿不到锁也照读。
        let _guard = store_lock(path).ok();
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
        let _guard = store_lock(path).ok();
        let Ok(store) = load_or_default(path) else {
            return FirstUseVerification::NotInStore;
        };
        match find_model(&store, &key.agent_id, &key.provider, &key.id) {
            Ok(found) => found.clone(),
            Err(_) => return FirstUseVerification::NotInStore,
        }
    };
    if let Some(reason) = dispatch_block_reason(&target) {
        return FirstUseVerification::Rejected(reason);
    }
    if target.verification != VERIFICATION_UNVERIFIED {
        return FirstUseVerification::Skipped;
    }
    let outcome = smoke(&target);
    // 冒烟期间点了叫停：进程已经被 cancel_registered 杀掉，这次结果不算数，
    // 不写库（免得把「被杀」记成验证失败），节点按「已取消」返回。
    if cancelled() {
        return FirstUseVerification::Cancelled;
    }
    if let Err(error) = &outcome {
        if let Some(hit) = crate::channels::classify_quota(error, None, None) {
            return FirstUseVerification::QuotaExhausted { resets_at: hit.resets_at };
        }
    }
    let failure = outcome.as_ref().err().map(|error| safe_summary(error));
    let persisted = store_lock(path).and_then(|_guard| match load_or_default(path) {
        Ok(mut store) => apply_smoke_outcome(&mut store, &key, outcome)
            .and_then(|()| save_store(path, &store)),
        Err(error) => Err(error),
    });
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
    } else if normalized.contains("gemini") || normalized.contains("google") {
        "Google"
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

// ── TASK-39：命令行状态映射（`aicross resources` 输出用，纯函数） ──────

/// agent 两态：探测到 → 可用；未探测到 → 未安装。
pub(crate) const RESOURCE_STATE_AVAILABLE: &str = "可用";
pub(crate) const RESOURCE_STATE_MISSING: &str = "未安装";
/// 模型态：第三方按验证结果，pi 另有登录与额度两态；TASK-48 起最近一次
/// 探测没发现的模型另有「未发现」一态。
pub(crate) const RESOURCE_STATE_UNVERIFIED: &str = "未验证";
pub(crate) const RESOURCE_STATE_VERIFIED: &str = "已验证";
pub(crate) const RESOURCE_STATE_FAILED: &str = "验证失败";
pub(crate) const RESOURCE_STATE_TIMEOUT: &str = "冒烟超时";
pub(crate) const RESOURCE_STATE_LOGGED_OUT: &str = "未登录";
pub(crate) const RESOURCE_STATE_QUOTA: &str = "额度用尽";
pub(crate) const RESOURCE_STATE_NOT_DETECTED: &str = "未发现";

/// 走查 #1：资源库从未探测（`detectedAt` 为空）时 agent 与其下模型的第三
/// 种态；探测过后回到 可用 / 未安装 两态。
pub(crate) const RESOURCE_STATE_NEVER_DETECTED: &str = "未探测";
/// 未探测时 agent 的下一步说明；同态的模型 note 为空。
pub(crate) const NEVER_DETECTED_NOTE: &str = "尚未探测";

pub(crate) fn agent_state(detected: bool) -> &'static str {
    if detected {
        RESOURCE_STATE_AVAILABLE
    } else {
        RESOURCE_STATE_MISSING
    }
}

/// agent 状态的下一步说明；可用时为空（无下一步）。
pub(crate) fn agent_state_note(agent: &ResourceAgent) -> String {
    if agent.detected {
        String::new()
    } else {
        install_note().to_string()
    }
}

/// 未安装时的下一步说明；agent 与它下面模型的 note 用同一句。
fn install_note() -> &'static str {
    "安装后重新探测"
}

/// meta 是否带着「未在事实源中」标记（TASK-48）。标记由
/// `merge_detected_models` 在最近一次探测没发现该模型时追加，这里只读，
/// 不改合并规则；重新探测发现后整条替换，标记随 meta 自然消失。分段与
/// `append_note_marker` 同一口径：按「；」分段后 trim 比对。
pub(crate) fn is_missing_from_detection(model: &ResourceModel) -> bool {
    model
        .meta
        .split('；')
        .any(|part| part.trim() == MISSING_MARKER)
}

/// 按 agent + provider + id 查模型（fix-48-r1 R1）：派发前检查与 `config show`
/// 的人选状态共用这一个函数，不再各写各的查找。provider 非空时只做精确
/// 匹配——同一模型 id 可以挂在多个 provider 下，其中一个带「未在事实源中」
/// 标记时，按 agent+id 取第一条会看错条目。provider 为空时实际派发由 agent
/// 自己选端点，这里优先取没带「未发现」标记的那条，全部带标记才返回带标记
/// 的，避免一个已撤掉的同名条目把可用的人选判成未发现；没带标记的里面
/// 再优先没验证失败的（fix-fullcheck B4），一条失败的同名条目不牵连可用的。
/// 派发前检查（引擎的第一阶段检查与派发层每次尝试的 [`dispatch_block_reason`]）
/// 都经这里，provider 精确匹配。
pub(crate) fn find_target_model<'a>(
    models: &'a [ResourceModel],
    agent: &str,
    provider: &str,
    id: &str,
) -> Option<&'a ResourceModel> {
    if provider.trim().is_empty() {
        let same_id = || models.iter().filter(|m| m.agent == agent && m.id == id);
        let first = same_id().next()?;
        same_id()
            .find(|m| !is_missing_from_detection(m) && m.verification != VERIFICATION_FAILED)
            .or_else(|| same_id().find(|m| !is_missing_from_detection(m)))
            .or(Some(first))
    } else {
        models
            .iter()
            .find(|m| m.agent == agent && m.provider == provider && m.id == id)
    }
}

/// 派发前的资源库拦截（fix-fullcheck B2 / B3）：最近一次探测未发现、或已
/// 验证失败的模型不派发，返回原因；`timeout` 与其它状态放行。派发层每次
/// 尝试（任一阶段的主目标、显式备选、资源库有序备选）与首次验证入口共用。
pub(crate) fn dispatch_block_reason(model: &ResourceModel) -> Option<String> {
    if is_missing_from_detection(model) {
        return Some(format!(
            "{}/{} 在最近一次探测中未发现，请重新探测或换人选",
            model.agent, model.id
        ));
    }
    if model.verification == VERIFICATION_FAILED {
        let detail = model.smoke_failure.trim();
        let detail = if detail.is_empty() {
            String::new()
        } else {
            format!("（{detail}）")
        };
        return Some(format!(
            "目标验证失败：{}/{}{detail}；用 aicross resources smoke 重验后再派发",
            model.agent, model.id
        ));
    }
    None
}

/// 探测成功写回前盖时间戳（fix-48-r1 R4）：生产与测试两个探测入口共用，
/// console_cli 测试的注入实现也走它，「探测成功写回即更新 detectedAt」
/// 只有这一处。
pub(crate) fn stamp_detected_at(store: &mut ResourceStore) {
    store.detected_at = Some(Local::now().to_rfc3339_opts(SecondsFormat::Secs, false));
}

/// 模型状态与下一步说明。优先级：agent 未安装 → 未发现 → 未过期的额度用尽
/// → 原生 → pi 未登录 → 验证结果。agent 未探测到时它下面的模型一并无从使用，
/// 状态跟随 agent（fix-39-r1 R1-4）；原生模型也可能挂额度标记，引擎派发前拒掉
/// 额度未恢复的目标时分原生与否，额度因此先于原生判（R1-3）；探测没发现的
/// 模型不管验证到哪一档都不能再派，未发现因此压在额度与验证结果之上，
/// 原生模型同样适用（TASK-48）。
/// `now_millis` 注入当前时间：过期的额度标记按验证结果展示
/// （读取层的 `clear_expired_quota` 会顺带清掉库里的过期值）。
pub(crate) fn model_state(
    model: &ResourceModel,
    agent_available: bool,
    now_millis: i64,
) -> (&'static str, String) {
    if !agent_available {
        return (RESOURCE_STATE_MISSING, install_note().to_string());
    }
    if is_missing_from_detection(model) {
        return (RESOURCE_STATE_NOT_DETECTED, NOT_DETECTED_NOTE.to_string());
    }
    let quota_active = model
        .quota_exhausted_until
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .is_some_and(|moment| moment.timestamp_millis() > now_millis);
    if quota_active {
        return (
            RESOURCE_STATE_QUOTA,
            format!(
                "额度恢复时间：{}",
                model.quota_exhausted_until.as_deref().unwrap_or_default()
            ),
        );
    }
    if is_native_model(&model.agent) {
        return (RESOURCE_STATE_AVAILABLE, String::new());
    }
    if model.agent.trim().eq_ignore_ascii_case("pi") && model.login == ResourceLogin::LoggedOut {
        return (RESOURCE_STATE_LOGGED_OUT, pi_login_note(&model.provider));
    }
    match model.verification.as_str() {
        VERIFICATION_VERIFIED => (RESOURCE_STATE_VERIFIED, String::new()),
        // 超时不算验证失败（TASK-42）：状态插在「验证结果」一档内，
        // note 说明不影响派发。
        VERIFICATION_TIMEOUT => (
            RESOURCE_STATE_TIMEOUT,
            "连接测试超时，不影响派发".to_string(),
        ),
        VERIFICATION_FAILED => (RESOURCE_STATE_FAILED, String::new()),
        _ => (RESOURCE_STATE_UNVERIFIED, String::new()),
    }
}

/// 未登录时的登录提示。pi 0.86 的 `pi --help` 没有 `auth login` 子命令：
/// 订阅类 provider 在 pi 会话里用 `/login` 登录，API key 类走环境变量；
/// 登录后要重新探测才会刷新登录态（fix-39-r1 R1-8）。
fn pi_login_note(provider: &str) -> String {
    let provider = provider.trim();
    let login = if provider.is_empty() {
        "/login".to_string()
    } else {
        format!("/login {provider}")
    };
    format!("未登录；在 pi 中运行 {login} 后重新探测")
}

// ── Tauri 命令 ────────────────────────────────────────────────────────────
// 所有写命令都按「持 store_lock(path) → load → 改 → 原子写」的序列执行，
// 锁保证并发命令不互相覆盖（审查 2.4）。命令体拆成 *_at(path, …) 自由函数，
// 方便测试用临时目录钉住行为。

pub(crate) fn get_resources_at(path: &Path) -> Result<ResourceStore, String> {
    let _guard = store_lock(path)?;
    let mut store = load_or_default(path)?;
    ensure_agents(&mut store);
    if clear_expired_quota(&mut store) && path.is_file() {
        save_store(path, &store)?;
    }
    Ok(store)
}

pub(crate) fn detect_all_resources_at(
    path: &Path,
    cc_switch_path: Option<&str>,
) -> Result<ResourceOperation, String> {
    detect_all_resources_with_progress(path, cc_switch_path, &|_, _, _| {})
}

/// 探测全部 agent，每个 agent 探测完回调一次 `(agent_id, done, total)`
/// （走查 #3）：窗口据此发进度事件，命令行传空回调。
pub(crate) fn detect_all_resources_with_progress(
    path: &Path,
    cc_switch_path: Option<&str>,
    on_done: &(dyn Fn(&str, usize, usize) + Sync),
) -> Result<ResourceOperation, String> {
    detect_all_resources_inner(path, cc_switch_path, on_done, &|agent_id| {
        prepare_agent_detection(agent_id)
    })
}

/// 探测主流程：各 agent 的探测（版本、模型清单、pi 的登录探测）在 store
/// 锁外并行跑（每个 agent 一个线程），全部完成后在锁内一次性合并、盖
/// `detectedAt`、写盘；合并按 agent 清单顺序进行，结果与逐个顺序跑一致。
/// `prepare` 注入锁外探测，测试用假实现，不真跑本机 CLI。
fn detect_all_resources_inner(
    path: &Path,
    cc_switch_path: Option<&str>,
    on_done: &(dyn Fn(&str, usize, usize) + Sync),
    prepare: &(dyn Fn(&str) -> PreparedDetection + Sync),
) -> Result<ResourceOperation, String> {
    // agent 清单先在锁内取齐（ensure_agents 可能补内置骨架），探测全部
    // 放到锁外，不把其他写方挡在锁外几十秒。
    let ids = {
        let _guard = store_lock(path)?;
        let mut store = load_or_default(path)?;
        ensure_agents(&mut store);
        store
            .agents
            .iter()
            .map(|agent| agent.id.clone())
            .collect::<Vec<_>>()
    };
    let total = ids.len();
    // 进度计数与回调在同一把锁里，事件里的 done 严格递增（fix-57-r1 2）。
    // 锁可能因回调自身 panic 而中毒，拿回数据继续计数。
    let finished = &std::sync::Mutex::new(0usize);
    let prepared: Vec<PreparedDetection> = std::thread::scope(|scope| {
        let handles: Vec<_> = ids
            .iter()
            .map(|agent_id| {
                let agent_id = agent_id.clone();
                let spawned = agent_id.clone();
                let handle = scope.spawn(move || {
                    let prepared = prepare(&spawned);
                    let mut done =
                        finished.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    *done += 1;
                    on_done(&spawned, *done, total);
                    prepared
                });
                (agent_id, handle)
            })
            .collect();
        // 探测线程 panic 不作废整次探测：join 失败转成该 agent 的 Failed
        // 占位，锁内合并时记一条错误，其余 agent 照常盖时间、写盘
        // （fix-57-r1 1）。
        handles
            .into_iter()
            .map(|(agent_id, handle)| {
                handle
                    .join()
                    .unwrap_or_else(|_| PreparedDetection::Failed(agent_id))
            })
            .collect()
    });
    merge_prepared_detections(path, cc_switch_path, &ids, prepared)
}

/// 锁内合并：按 agent 清单顺序应用全部探测结果，查 cc-switch 发现提示，
/// 盖 `detectedAt` 并写盘。并行与顺序两个入口共用这一步。
fn merge_prepared_detections(
    path: &Path,
    cc_switch_path: Option<&str>,
    ids: &[String],
    mut prepared: Vec<PreparedDetection>,
) -> Result<ResourceOperation, String> {
    // cc-switch 提示要拉起 Python（最长 20 秒），放在锁外。
    let (cc_switch_hints, hint_error) = detect_cc_switch_hints(cc_switch_path);
    let _guard = store_lock(path)?;
    let mut store = load_or_default(path)?;
    ensure_agents(&mut store);
    let mut errors = Vec::new();
    for agent_id in ids {
        let index = prepared
            .iter()
            .position(|item| item.agent_id() == agent_id)
            .expect("每个 agent 恰有一份探测结果");
        let item = prepared.swap_remove(index);
        apply_prepared_detection(&mut store, item, &mut errors);
    }
    if let Some(error) = hint_error {
        errors.push(error);
    }
    stamp_detected_at(&mut store);
    save_store(path, &store)?;
    Ok(ResourceOperation {
        store,
        errors,
        cc_switch_hints,
        skipped: Vec::new(),
    })
}

/// 顺序参照实现（测试用）：逐个探测后走同一份合并，结果应与并行版一致。
#[cfg(test)]
fn detect_all_resources_sequential_with(
    path: &Path,
    cc_switch_path: Option<&str>,
    prepare: &(dyn Fn(&str) -> PreparedDetection + Sync),
) -> Result<ResourceOperation, String> {
    let ids = {
        let _guard = store_lock(path)?;
        let mut store = load_or_default(path)?;
        ensure_agents(&mut store);
        store
            .agents
            .iter()
            .map(|agent| agent.id.clone())
            .collect::<Vec<_>>()
    };
    let prepared = ids.iter().map(|agent_id| prepare(agent_id)).collect();
    merge_prepared_detections(path, cc_switch_path, &ids, prepared)
}

/// TASK-52：删除最近一次探测未发现（meta 带「未在事实源中」标记）的模型。
/// 用户申报的模型不带标记，天然保留；被 modes.json 人选或 roles.json 角色
/// 引用的照样删除，删掉的里面被引用的经 `referenced` 返回，调用方单列
/// 说明。`agent_filter` 只清这一家（大小写不敏感，与其它 resources 命令
/// 同一写法）；`referenced` 是调用方从 modes.json 与 roles.json 收集的
/// 模型指向。在 store_lock 内读改写；没有要删的模型时不写盘。
pub(crate) fn prune_missing_models_at(
    path: &Path,
    agent_filter: Option<&str>,
    referenced: &[ModelReference],
) -> Result<PruneOutcome, String> {
    let _guard = store_lock(path)?;
    let mut store = load_or_default(path)?;
    ensure_agents(&mut store);
    let mut removed = Vec::new();
    let mut referenced_removed = Vec::new();
    for agent in &mut store.agents {
        if agent_filter.is_some_and(|wanted| !agent.id.eq_ignore_ascii_case(wanted.trim())) {
            continue;
        }
        agent.models.retain(|model| {
            if !is_missing_from_detection(model) {
                return true;
            }
            let key = resource_model_key(&model.agent, &model.provider, &model.id);
            if reference_matches(referenced, model) {
                referenced_removed.push(key.clone());
            }
            removed.push(key);
            false
        });
    }
    if !removed.is_empty() {
        save_store(path, &store)?;
    }
    Ok(PruneOutcome {
        store,
        removed,
        referenced: referenced_removed,
    })
}

/// 人选 / 角色是否指向这条模型：agent 与 model 要一致（agent 名大小写
/// 不敏感），provider 为空表示不限，非空要相等。
fn reference_matches(referenced: &[ModelReference], model: &ResourceModel) -> bool {
    referenced.iter().any(|(agent, provider, id)| {
        agent.trim().eq_ignore_ascii_case(model.agent.trim())
            && id.trim() == model.id.trim()
            && (provider.trim().is_empty() || provider.trim() == model.provider.trim())
    })
}

/// 单 agent 探测（fix-40-r1 R1-4 恢复测试用，同名函数原随 Tauri 命令下线）：
/// 只探测指定 agent，pi 二进制可注入，不真探测本机其它 CLI。生产走
/// `detect_all_resources_at`。登录探测仍先于 store 锁——探测一次最长
/// 几十秒，不能把其他写方挡在锁外。
#[cfg(test)]
fn detect_resource_agent_at_with_pi_binary(
    path: &Path,
    agent_id: &str,
    cc_switch_path: Option<&str>,
    pi_binary: Option<PathBuf>,
) -> Result<ResourceOperation, String> {
    let prepared_pi = (agent_id == "pi").then(|| prepare_pi_detection(pi_binary));
    let _guard = store_lock(path)?;
    let mut store = load_or_default(path)?;
    ensure_agents(&mut store);
    let mut errors = Vec::new();
    if let Some(prepared) = prepared_pi {
        apply_prepared_pi_detection(&mut store, prepared, &mut errors);
    } else {
        apply_prepared_detection(&mut store, prepare_agent_detection(agent_id), &mut errors);
    }
    let (cc_switch_hints, hint_error) = detect_cc_switch_hints(cc_switch_path);
    if let Some(error) = hint_error {
        errors.push(error);
    }
    stamp_detected_at(&mut store);
    save_store(path, &store)?;
    Ok(ResourceOperation {
        store,
        errors,
        cc_switch_hints,
        skipped: Vec::new(),
    })
}

pub(crate) fn smoke_resource_model_at(path: &Path, request: ResourceModelKey) -> Result<ResourceStore, String> {
    smoke_resource_model_with(path, &request, run_smoke)
}

/// 单模型冒烟（fix-fullcheck B1）：锁内取目标即放锁，冒烟（最长 180 秒）
/// 在锁外跑；写回时重新加锁、重读最新库，只合并这一条的结果，期间别的
/// 进程写入的额度标记、验证结果、探测结果都保留。冒烟期间目标被删掉时
/// 报错，不写回。
fn smoke_resource_model_with(
    path: &Path,
    request: &ResourceModelKey,
    smoke: impl FnOnce(&ResourceModel) -> Result<SmokeSuccess, String>,
) -> Result<ResourceStore, String> {
    let target = {
        let _guard = store_lock(path)?;
        let store = load_or_default(path)?;
        find_model(&store, &request.agent_id, &request.provider, &request.id)?.clone()
    };
    let outcome = smoke(&target);
    let _guard = store_lock(path)?;
    let mut store = load_or_default(path)?;
    apply_smoke_outcome(&mut store, request, outcome)?;
    save_store(path, &store)?;
    Ok(store)
}

/// TASK-39：命令行按 agent 冒烟只挑第三方模型；原生模型不发请求
/// （`aicross resources smoke <agent>` 的口径）。TASK-48 起最近一次探测
/// 没发现的模型同样不发请求，逐个给出跳过说明。过滤与跳过说明提成具名
/// 函数（fix-48-r1 R5），生产入口与测试注入入口共用。
fn smoke_agent_third_party_filter(model: &ResourceModel) -> bool {
    !is_native_model(&model.agent) && !is_missing_from_detection(model)
}

/// 被排除条目的跳过说明：未发现的第三方模型逐个给一句；原生模型不说明。
fn smoke_agent_third_party_skip_reason(model: &ResourceModel) -> Option<String> {
    (!is_native_model(&model.agent) && is_missing_from_detection(model))
        .then(|| format!("跳过 {}/{}：{}", model.agent, model.id, NOT_DETECTED_NOTE))
}

pub(crate) fn smoke_agent_third_party_at(
    path: &Path,
    agent_id: &str,
) -> Result<ResourceOperation, String> {
    smoke_agent_models_filtered_at(
        path,
        agent_id,
        smoke_agent_third_party_filter,
        smoke_agent_third_party_skip_reason,
        run_smoke,
    )
}

/// 同上、冒烟函数可注入（只服务测试）：走与生产同一条过滤与跳过说明，
/// 不在单测里真发模型请求。
#[cfg(test)]
fn smoke_agent_third_party_with(
    path: &Path,
    agent_id: &str,
    smoke: impl FnMut(&ResourceModel) -> Result<SmokeSuccess, String>,
) -> Result<ResourceOperation, String> {
    smoke_agent_models_filtered_at(
        path,
        agent_id,
        smoke_agent_third_party_filter,
        smoke_agent_third_party_skip_reason,
        smoke,
    )
}

/// 冒烟某个 agent 的模型（`filter` 挑条目，`skip_reason` 给被排除条目的跳过
/// 说明，返回 None 的不说明）；失败汇总进 errors（单条失败不中断整批），
/// 跳过说明汇总进 skipped，不算失败、不影响退出码。每冒烟一条都重新
/// load → 改 → 写盘，不整批长持旧 store——一次
/// 冒烟最长 180 秒，整批持旧数据会把期间其他写方的更新整体抹掉（审查 2.4）。
fn smoke_agent_models_filtered_at(
    path: &Path,
    agent_id: &str,
    filter: impl Fn(&ResourceModel) -> bool,
    skip_reason: impl Fn(&ResourceModel) -> Option<String>,
    mut smoke: impl FnMut(&ResourceModel) -> Result<SmokeSuccess, String>,
) -> Result<ResourceOperation, String> {
    // 第一次加锁读到的库作为返回值；全部条目被跳过时循环体不执行，
    // 不能落回空库，否则命令行与窗口的输出会丢掉这个 agent（fix-48-r1 R3）。
    let (keys, skipped, latest) = {
        let _guard = store_lock(path)?;
        let mut store = load_or_default(path)?;
        ensure_agents(&mut store);
        let Some(agent) = store.agents.iter().find(|agent| agent.id == agent_id) else {
            return Ok(ResourceOperation {
                store,
                errors: Vec::new(),
                cc_switch_hints: Vec::new(),
                skipped: Vec::new(),
            });
        };
        let keys = agent
            .models
            .iter()
            .filter(|model| filter(model))
            .map(|model| ResourceModelKey {
                agent_id: agent_id.to_string(),
                id: model.id.clone(),
                provider: model.provider.clone(),
            })
            .collect::<Vec<_>>();
        let skipped = agent
            .models
            .iter()
            .filter(|model| !filter(model))
            .filter_map(skip_reason)
            .collect::<Vec<_>>();
        (keys, skipped, store)
    };
    let mut errors = Vec::new();
    let mut latest = latest;
    for key in &keys {
        // 同 smoke_resource_model_with（fix-fullcheck B1）：冒烟在锁外跑，
        // 写回时重读最新库、只合并这一条。
        let target = {
            let _guard = store_lock(path)?;
            let store = load_or_default(path)?;
            find_model(&store, &key.agent_id, &key.provider, &key.id).cloned()
        };
        let model = match target {
            Ok(model) => model,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let outcome = smoke(&model);
        let _guard = store_lock(path)?;
        let mut store = load_or_default(path)?;
        match apply_smoke_outcome(&mut store, key, outcome) {
            Ok(()) => {
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
        skipped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_model(id: &str) -> ResourceModel {
        detected_model(id, "codex", "", "OpenAI", "官方订阅", "支持", "读:fixture", "")
    }

    /// 厂商按模型 id 关键字推定（fix-50-r2：补 gemini / google → Google）；
    /// 认不出的为「其他」（不参与推荐的厂商去重与主 agent 避让）。
    #[test]
    fn infer_vendor_maps_model_id_keywords() {
        assert_eq!(infer_vendor("gemini-2.5-pro"), "Google");
        assert_eq!(infer_vendor("Gemini-Flash"), "Google");
        assert_eq!(infer_vendor("google/gemma-3"), "Google");
        assert_eq!(infer_vendor("claude-sonnet-4-5"), "Anthropic");
        assert_eq!(infer_vendor("gpt-5.2"), "OpenAI");
        assert_eq!(infer_vendor("glm-4.7"), "Zhipu");
        assert_eq!(infer_vendor("deepseek-chat"), "DeepSeek");
        assert_eq!(infer_vendor("minimax-text"), "MiniMax");
        assert_eq!(infer_vendor("qwen3.8-max"), "Alibaba");
        assert_eq!(infer_vendor("mystery-model"), "其他");
    }

    fn fixture_agent(id: &str, models: Vec<ResourceModel>) -> ResourceAgent {
        ResourceAgent {
            id: id.to_string(),
            name: id.to_string(),
            kind: "内置".to_string(),
            detected: true,
            version: "test".to_string(),
            models,
            ..ResourceAgent::default()
        }
    }

    fn write_fake_pi(root: &Path, body: &str) -> PathBuf {
        #[cfg(windows)]
        let script = root.join("pi.cmd");
        #[cfg(not(windows))]
        let script = root.join("pi");
        fs::write(&script, body).expect("write fake pi");
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("make fake pi executable");
        }
        script
    }

    /// fix-40-r1 R1-4 恢复（原 task17_pi_detect_list_auth_merge_persists_without_auth_payload，
    /// 改用可注入 pi 路径的全量探测入口）：探测合并后登录态落盘，但凭据本体不进资源库。
    #[test]
    fn task17_pi_detect_list_auth_merge_persists_without_auth_payload() {
        let root = crate::paths::test_temp_dir().join(format!("aicross-pi-detect-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        #[cfg(windows)]
        let body = r#"@echo off
if /I "%~1"=="--version" goto version
if /I "%~1"=="--list-models" goto list
if /I "%~1"=="auth" goto auth
exit /b 2
:version
echo pi test
exit /b 0
:list
echo ready model-a
exit /b 0
:auth
echo {"status":"ready","provider":"ready","authType":"api_key","credentials":{"access_token":"do-not-read"}}
exit /b 0
"#;
        #[cfg(not(windows))]
        let body = r#"#!/bin/sh
if [ "$1" = --version ]; then printf '%s\n' 'pi test'; exit 0; fi
if [ "$1" = --list-models ]; then printf '%s\n' 'ready model-a'; exit 0; fi
if [ "$1" = auth ]; then printf '%s\n' '{"status":"ready","provider":"ready","authType":"api_key","credentials":{"access_token":"do-not-read"}}'; exit 0; fi
exit 2
"#;
        let script = write_fake_pi(&root, body);
        let path = root.join("resources.json");
        save_store(&path, &ResourceStore { detected_at: None, version: 1, agents: vec![fixture_agent("pi", Vec::new())] }).unwrap();
        let operation = detect_resource_agent_at_with_pi_binary(&path, "pi", None, Some(script)).unwrap();
        assert!(operation.errors.is_empty(), "{:?}", operation.errors);
        let persisted = load(&path).unwrap();
        let model = find_model(&persisted, "pi", "ready", "model-a").expect("detected model");
        assert_eq!(model.login, ResourceLogin::LoggedIn);
        assert!(!model.login_checked_at.is_empty());
        let raw = fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("do-not-read"));
        assert!(!raw.contains("credentials"));
        let _ = fs::remove_dir_all(root);
    }

    /// fix-40-r1 R1-4 恢复（原 task17_missing_pi_detection_clears_stale_login）：
    /// pi 缺席时探测照常写回，残留的旧登录态清成未知。
    #[test]
    fn task17_missing_pi_detection_clears_stale_login() {
        let root = crate::paths::test_temp_dir().join(format!("aicross-pi-stale-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        let model = ResourceModel {
            id: "m".into(), agent: "pi".into(), provider: "p".into(),
            login: ResourceLogin::LoggedIn, ..Default::default()
        };
        save_store(&path, &ResourceStore { detected_at: None, version: 1, agents: vec![fixture_agent("pi", vec![model])] }).unwrap();
        let operation = detect_resource_agent_at_with_pi_binary(&path, "pi", None, None).unwrap();
        let pi = operation.store.agents.iter().find(|agent| agent.id == "pi").unwrap();
        assert!(!pi.detected);
        assert_eq!(pi.models[0].login, ResourceLogin::Unknown);
        assert!(!pi.models[0].login_checked_at.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    /// fix-40-r1 R1-4 恢复（原 task17_pi_login_probe_runs_before_store_lock）：
    /// pi 登录探测在取资源库锁之前跑，不把其他写方挡在锁外。
    #[test]
    fn task17_pi_login_probe_runs_before_store_lock() {
        let root = crate::paths::test_temp_dir().join(format!("aicross-pi-lock-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let marker = root.join("auth-started");
        #[cfg(windows)]
        let body = r#"@echo off
if /I "%~1"=="--version" goto version
if /I "%~1"=="--list-models" goto list
if /I "%~1"=="auth" goto auth
exit /b 2
:version
echo pi test
exit /b 0
:list
echo p model-a
exit /b 0
:auth
type nul > "%~dp0auth-started"
ping -n 3 127.0.0.1 > nul
echo {"status":"ready","provider":"p"}
exit /b 0
"#;
        #[cfg(not(windows))]
        let body = r#"#!/bin/sh
if [ "$1" = --version ]; then printf '%s\n' 'pi test'; exit 0; fi
if [ "$1" = --list-models ]; then printf '%s\n' 'p model-a'; exit 0; fi
if [ "$1" = auth ]; then : > "$(dirname "$0")/auth-started"; sleep 1; printf '%s\n' '{"status":"ready","provider":"p"}'; exit 0; fi
exit 2
"#;
        let script = write_fake_pi(&root, body);
        let path = root.join("resources.json");
        save_store(&path, &ResourceStore { detected_at: None, version: 1, agents: vec![fixture_agent("pi", Vec::new())] }).unwrap();
        let thread_path = path.clone();
        let handle = std::thread::spawn(move || {
            detect_resource_agent_at_with_pi_binary(&thread_path, "pi", None, Some(script))
        });
        for _ in 0..200 {
            if marker.is_file() { break; }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(marker.is_file(), "fake pi auth check did not start");
        let mut available = false;
        for _ in 0..20 {
            if let Ok(guard) = store_mutex().try_lock() {
                drop(guard);
                available = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(available, "pi auth check must not hold the resource store lock");
        handle.join().unwrap().unwrap();
        let _ = fs::remove_dir_all(root);
    }

    /// fix-40-r1 R1-4 恢复（原 task14_fallback_order_roundtrips_and_quota_expiry_is_cleared
    /// 中仍测保留代码的部分）：已过期的额度标记在读时清掉并写回，恢复时间标记可写入。
    /// 原测试里 fallback 清单轮转与 update_model 的断言随那些函数一并删除。
    #[test]
    fn task14_quota_expiry_is_cleared_and_marks_roundtrip() {
        let root = crate::paths::test_temp_dir().join(format!("aicross-quota-store-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        let mut primary = fixture_model("main");
        primary.agent = "pi".into();
        primary.provider = "p0".into();
        primary.quota_exhausted_until = Some("2000-01-01T00:00:00+08:00".into());
        let store = ResourceStore {
            detected_at: None,
            version: 1,
            agents: vec![fixture_agent("pi", vec![primary])],
        };
        save_store(&path, &store).unwrap();

        // 已过期的额度标记：读资源库时清掉并写回。
        let visible = get_resources_at(&path).unwrap();
        let main = find_model(&visible, "pi", "p0", "main").unwrap();
        assert!(main.quota_exhausted_until.is_none());

        // 未过期的恢复时间标记：原样保留，可写入。
        mark_quota_exhausted_at(&path, "pi", "p0", "main", Some("2099-01-01T00:00:00+08:00")).unwrap();
        assert_eq!(find_model(&load(&path).unwrap(), "pi", "p0", "main").unwrap().quota_exhausted_until.as_deref(), Some("2099-01-01T00:00:00+08:00"));
        let visible = get_resources_at(&path).unwrap();
        assert_eq!(find_model(&visible, "pi", "p0", "main").unwrap().quota_exhausted_until.as_deref(), Some("2099-01-01T00:00:00+08:00"));
        let _ = fs::remove_dir_all(root);
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
    fn codex_cache_skips_hidden_and_keeps_priority_order() {
        let cache = r#"{"models":[
            {"slug":"codex-auto-review","visibility":"hide","priority":43},
            {"slug":"gpt-b","visibility":"list","priority":2},
            {"slug":"gpt-legacy","visibility":"hide","priority":13},
            {"slug":"gpt-c","visibility":"list","priority":9},
            {"slug":"gpt-a","visibility":"list","priority":1}
        ]}"#;
        let models = parse_codex_cache(cache, "读:fixture").expect("codex cache");
        let ids = models.iter().map(|model| model.id.as_str()).collect::<Vec<_>>();
        assert_eq!(ids, ["gpt-a", "gpt-b", "gpt-c"]);

        // 合并后资源库保持事实源顺序，而不是按 id 字母序；事实源里没有的排最后。
        let mut store = ResourceStore {
            detected_at: None,
            version: 1,
            agents: vec![fixture_agent("codex", vec![fixture_model("codex-auto-review")])],
        };
        merge_detected_models(&mut store, "codex", models);
        let merged = store.agents[0]
            .models
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(merged, ["gpt-a", "gpt-b", "gpt-c", "codex-auto-review"]);
        assert!(store.agents[0].models[3].meta.contains(MISSING_MARKER));
    }

    /// fix-fullcheck B6：缓存解析成功、但模型全部 `visibility: "hide"`：这是
    /// 「过滤后为空」，不是解析失败——返回空清单，合并后旧条目标未发现，
    /// 派发前检查据此拦下；缓存解析失败与没有任何带 slug 的模型仍报错。
    #[test]
    fn fullcheck_b6_codex_cache_all_hidden_marks_old_models_missing() {
        let cache = r#"{"models":[
            {"slug":"gpt-old","visibility":"hide","priority":1},
            {"slug":"codex-auto-review","visibility":"hide","priority":2}
        ]}"#;
        let models = parse_codex_cache(cache, "读:fixture").expect("全部隐藏是空清单，不是失败");
        assert!(models.is_empty());
        let mut old = fixture_model("gpt-old");
        old.agent = "codex".into();
        let mut store = ResourceStore {
            detected_at: None,
            version: 1,
            agents: vec![fixture_agent("codex", vec![old])],
        };
        let mut errors = Vec::new();
        apply_prepared_plain_detection(
            &mut store,
            PreparedAgentDetection {
                agent_id: "codex".into(),
                detected: true,
                version: "0.2.0".into(),
                missing_error: None,
                models: Some(Ok(models)),
            },
            &mut errors,
        );
        assert!(errors.is_empty(), "{errors:?}");
        let old = &store.agents[0].models[0];
        assert!(is_missing_from_detection(old), "{}", old.meta);
        assert!(dispatch_block_reason(old).is_some());
        // 解析失败与没有任何带 slug 的模型照旧报错，旧条目不动。
        assert!(parse_codex_cache("{ not json", "读:fixture").is_err());
        assert!(parse_codex_cache(r#"{"models":[{"slug":" ","visibility":"list"}]}"#, "读:fixture").is_err());
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

        let root = crate::paths::test_temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
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
        let root = crate::paths::test_temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
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
        let root = crate::paths::test_temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
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
            detected_at: None,
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
                    login: ResourceLogin::Unknown,
                    login_checked_at: String::new(),
                    resolved_id: "old-model".to_string(),
                    resolved_source: RESOLVED_SOURCE_ALIAS_TABLE.to_string(),
                    source: "读:old".to_string(),
                    smoked_at: "2026-01-01".to_string(),
                    smoke_model: "old-model".to_string(),
                    smoke_failure: "旧失败；摘要保留完整".to_string(),
                    meta: "旧元数据".to_string(),
                    note: "保留备注".to_string(),
                    fallback_order: vec!["pi/zai/glm-4.7".to_string()],
                    quota_exhausted_until: Some("2099-01-01T00:00:00+08:00".to_string()),
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
        assert_eq!(old.fallback_order, ["pi/zai/glm-4.7"]);
        assert!(old.quota_exhausted_until.is_some());
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
        assert!(legacy.fallback_order.is_empty());
        assert!(legacy.quota_exhausted_until.is_none());

        let mut store = ResourceStore {
            detected_at: None,
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
                    login: ResourceLogin::Unknown,
                    login_checked_at: String::new(),
                    resolved_id: "same".to_string(),
                    resolved_source: RESOLVED_SOURCE_ALIAS_TABLE.to_string(),
                    source: "读:old".to_string(),
                    smoked_at: String::new(),
                    smoke_model: String::new(),
                    smoke_failure: String::new(),
                    meta: "priority：1".to_string(),
                    note: "用户备注".to_string(),
                    fallback_order: Vec::new(),
                    quota_exhausted_until: None,
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
    fn pi_source_matching_uses_exact_tokens() {
        let store = r#"{
          "p": {"models": [
            {"id":"glm-4","provider":"p"},
            {"id":"glm-4.7","provider":"p"}
          ]}
        }"#;
        let models = parse_pi_sources(store, "p glm-4.7", "读:fixture").expect("exact match");
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "glm-4.7");

    }

    /// TASK-17：假 pi 只接受精确的只读 argv。返回 JSON 中刻意放入凭据字段，
    /// 断言资源状态与落盘 JSON 均不含它，防止实现退回整份认证对象的解析/持久化。
    #[test]
    fn task17_pi_status_fixtures_match_live_output_shape() {
        let result = |stdout: &str, code| ProcessResult {
            code: Some(code), stdout: stdout.to_string(), stderr: String::new(), timed_out: false,
            truncated: false, timeout: Duration::from_secs(1),
        };
        assert_eq!(
            pi_login_from_check(Ok(result(include_str!("../fixtures/resources/pi_auth_check_ready.json"), 0))),
            ResourceLogin::LoggedIn
        );
        assert_eq!(
            pi_login_from_check(Ok(result(include_str!("../fixtures/resources/pi_auth_check_not_ready.json"), 1))),
            ResourceLogin::LoggedOut
        );
    }

    #[test]
    fn task26_pi_is_detected_outside_the_store_lock() {
        assert!(uses_pi_prepared_detection("pi"));
        for agent in ["claude", "codex", "kimi", "manual-local"] {
            assert!(!uses_pi_prepared_detection(agent), "{agent}");
        }
    }

    /// 走查 #3 的假探测：每家固定结果，不真跑本机 CLI；kimi 故意探不到
    /// 且模型清单读取失败，pi 带一条 provider 提示，覆盖错误路径。
    fn task57_fake_prepare() -> impl Fn(&str) -> PreparedDetection + Sync {
        let claude = detected_model("sonnet", "claude", "", "Anthropic", "官方订阅", "支持", "读:fixture", "");
        let codex = vec![fixture_model("a"), fixture_model("b")];
        let pi = detected_model("glm-4.7", "pi", "zai", "Zhipu", "按次", "支持", "读:fixture", "");
        move |agent_id: &str| match agent_id {
            "claude" => PreparedDetection::Plain(PreparedAgentDetection {
                agent_id: agent_id.to_string(),
                detected: true,
                version: "2.1.0".to_string(),
                missing_error: None,
                models: Some(Ok(vec![claude.clone()])),
            }),
            "codex" => PreparedDetection::Plain(PreparedAgentDetection {
                agent_id: agent_id.to_string(),
                detected: true,
                version: "0.2.0".to_string(),
                missing_error: None,
                models: Some(Ok(codex.clone())),
            }),
            "kimi" => PreparedDetection::Plain(PreparedAgentDetection {
                agent_id: agent_id.to_string(),
                detected: false,
                version: "未检测".to_string(),
                missing_error: Some(format!("探测 {agent_id} 失败：未检测到 {agent_id}")),
                models: Some(Err("读取 ~/.kimi-code/config.toml 失败".to_string())),
            }),
            "pi" => PreparedDetection::Pi(PreparedPiDetection {
                detected: true,
                version: "0.86.0".to_string(),
                result: Some(Ok((vec![pi.clone()], vec!["provider x 读不了".to_string()]))),
            }),
            // TASK-61：支持清单里其余的 agent 按未安装处理（不报错）。
            other if crate::agent_catalog::find(other).is_some_and(|entry| !entry.core) => {
                PreparedDetection::Plain(PreparedAgentDetection {
                    agent_id: agent_id.to_string(),
                    detected: false,
                    version: "未检测".to_string(),
                    missing_error: None,
                    models: Some(Ok(Vec::new())),
                })
            }
            other => panic!("假探测不认识 agent：{other}"),
        }
    }

    fn task57_temp_root(tag: &str) -> PathBuf {
        let root = crate::paths::test_temp_dir()
            .join(format!("aicross-task57-{tag}-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    /// 走查 #3：注入假探测，并行版与顺序版在同一份输入下合并出同一份
    /// store（除 detectedAt 时间戳外逐字段一致），错误清单也一致。
    #[test]
    fn task57_parallel_detection_matches_sequential_merge() {
        let prepare = task57_fake_prepare();
        let parallel_path = task57_temp_root("parallel").join("resources.json");
        let sequential_path = task57_temp_root("sequential").join("resources.json");
        let parallel =
            detect_all_resources_inner(&parallel_path, None, &|_, _, _| {}, &prepare).unwrap();
        let sequential = detect_all_resources_sequential_with(&sequential_path, None, &prepare).unwrap();
        assert_eq!(parallel.errors, sequential.errors);
        assert!(parallel.store.detected_at.is_some());
        assert!(sequential.store.detected_at.is_some());
        let mut left = serde_json::to_value(&parallel.store).unwrap();
        let mut right = serde_json::to_value(&sequential.store).unwrap();
        left.as_object_mut().unwrap().remove("detectedAt");
        right.as_object_mut().unwrap().remove("detectedAt");
        assert_eq!(left, right);
        // 四家都探完：三家可用，kimi 按假结果未安装。
        let detected: Vec<(&str, bool)> = parallel
            .store
            .agents
            .iter()
            .map(|agent| (agent.id.as_str(), agent.detected))
            .collect();
        // TASK-61：支持清单里其余五家按未安装列出。
        assert_eq!(
            detected,
            vec![
                ("claude", true), ("codex", true), ("kimi", false), ("pi", true),
                ("agy", false), ("cursor", false), ("codebuddy", false), ("copilot", false),
                ("qoder", false),
            ]
        );
        let _ = fs::remove_dir_all(parallel_path.parent().unwrap());
        let _ = fs::remove_dir_all(sequential_path.parent().unwrap());
    }

    /// 走查 #3：每个 agent 探测完回调一次，done 从 1 递增到 total。
    #[test]
    fn task57_progress_callback_fires_once_per_agent() {
        let prepare = task57_fake_prepare();
        let path = task57_temp_root("progress").join("resources.json");
        let calls = std::sync::Mutex::new(Vec::<(String, usize, usize)>::new());
        let operation = detect_all_resources_inner(&path, None, &|agent, done, total| {
            calls.lock().unwrap().push((agent.to_string(), done, total));
        }, &prepare).unwrap();
        let calls = calls.into_inner().unwrap();
        assert_eq!(calls.len(), operation.store.agents.len());
        // done 严格递增（fix-57-r1 2）：不排序，直接断言回调序列就是
        // 1..=total；计数加一与回调在同一把锁里，顺序才成立。
        let dones: Vec<usize> = calls.iter().map(|(_, done, _)| *done).collect();
        assert_eq!(dones, (1..=calls.len()).collect::<Vec<_>>());
        assert!(calls.iter().all(|(_, _, total)| *total == calls.len()));
        let agents: HashSet<&str> = calls.iter().map(|(agent, _, _)| agent.as_str()).collect();
        assert_eq!(agents.len(), calls.len(), "每个 agent 恰回调一次：{calls:?}");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    /// fix-57-r1 1：某个 agent 的探测线程 panic 不再作废整次探测，
    /// 转成该 agent 的一条「线程异常」错误，其余 agent 照常合并、盖时间、写盘。
    #[test]
    fn task57_panicking_probe_becomes_one_agent_error() {
        let prepare = task57_fake_prepare();
        let panicking = move |agent_id: &str| {
            if agent_id == "codex" {
                panic!("注入的探测 panic");
            }
            prepare(agent_id)
        };
        let path = task57_temp_root("panic").join("resources.json");
        let operation =
            detect_all_resources_inner(&path, None, &|_, _, _| {}, &panicking).unwrap();
        // codex 的探测线程 panic 转成一条线程异常；其余 agent 的错误与
        // 假探测的基线一致（kimi 未检测与清单读取失败、pi 的 provider 提示）。
        assert_eq!(
            operation.errors,
            vec![
                "探测 codex 失败：线程异常",
                "探测 kimi 失败：未检测到 kimi",
                "探测 kimi 失败：读取 ~/.kimi-code/config.toml 失败",
                "provider x 读不了",
            ]
        );
        // 其余 agent 照常合并：claude、pi 可用；codex 停在骨架的未检测状态。
        let detected: Vec<(&str, bool)> = operation
            .store
            .agents
            .iter()
            .map(|agent| (agent.id.as_str(), agent.detected))
            .collect();
        // TASK-61：支持清单里其余五家按未安装列出。
        assert_eq!(
            detected,
            vec![
                ("claude", true), ("codex", false), ("kimi", false), ("pi", true),
                ("agy", false), ("cursor", false), ("codebuddy", false), ("copilot", false),
                ("qoder", false),
            ]
        );
        assert!(operation.store.detected_at.is_some());
        // 写盘：重新读盘核对结果真的落了盘。
        let reloaded = get_resources_at(&path).unwrap();
        assert!(reloaded.detected_at.is_some());
        let codex = reloaded
            .agents
            .iter()
            .find(|agent| agent.id == "codex")
            .unwrap();
        assert!(!codex.detected);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn task26_ready_status_with_nonzero_exit_is_logged_out_by_policy() {
        let result = ProcessResult {
            code: Some(7),
            stdout: r#"{"status":"ready","provider":"p"}"#.into(),
            stderr: String::new(),
            timed_out: false,
            truncated: false,
            timeout: Duration::from_secs(1),
        };
        assert_eq!(pi_login_from_check(Ok(result)), ResourceLogin::LoggedOut);
    }
    /// TASK-17：检查分批并行，但同一时刻最多四个 provider 子进程。
    #[test]
    fn task17_pi_login_checks_cap_concurrency_at_four() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let runner = {
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak);
            Arc::new(move |_provider: &str| {
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(current, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(50));
                active.fetch_sub(1, Ordering::SeqCst);
                ResourceLogin::LoggedIn
            })
        };
        let providers = (0..9).map(|index| format!("p{index}")).collect();
        let checked = check_pi_provider_logins_with_runner(providers, runner);
        assert_eq!(checked.len(), 9);
        assert_eq!(peak.load(Ordering::SeqCst), 4);
    }

    /// TASK-17：命令缺失和超时都不是「已退出」，只能留下 unknown。
    #[test]
    fn task17_pi_login_missing_command_and_timeout_are_unknown() {
        let missing = crate::paths::test_temp_dir().join(format!("aicross-no-pi-{}", Uuid::new_v4()));
        assert_eq!(
            check_pi_provider_login_with(&missing, "missing", Duration::from_millis(30)),
            ResourceLogin::Unknown
        );

        let root = crate::paths::test_temp_dir().join(format!("aicross-pi-timeout-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        #[cfg(windows)]
        let body = "@echo off\r\nping -n 3 127.0.0.1 > nul\r\necho {\"ready\":true}\r\n";
        #[cfg(not(windows))]
        let body = "#!/bin/sh\nsleep 1\nprintf '%s\\n' '{\"ready\":true}'\n";
        let script = write_fake_pi(&root, body);
        assert_eq!(
            check_pi_provider_login_with(&script, "slow", Duration::from_millis(30)),
            ResourceLogin::Unknown
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task17_dirty_login_value_reads_as_unknown_instead_of_failing() {
        let raw = r#"{"agent":"pi","id":"m","provider":"p","login":"expired"}"#;
        let model: ResourceModel = serde_json::from_str(raw).expect("dirty login value must not fail the whole store");
        assert_eq!(model.login, ResourceLogin::Unknown);
        let ok: ResourceModel = serde_json::from_str(r#"{"agent":"pi","id":"m","provider":"p","login":"logged_out"}"#).unwrap();
        assert_eq!(ok.login, ResourceLogin::LoggedOut);
    }

    #[test]
    fn task17_old_resource_json_defaults_login_to_unknown() {
        let root = crate::paths::test_temp_dir().join(format!("aicross-legacy-login-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        fs::write(&path, r#"{"version":1,"agents":[{"id":"pi","models":[{"id":"legacy","agent":"pi","provider":"p"}]}]}"#).unwrap();
        let store = load(&path).expect("legacy resources.json");
        let old = find_model(&store, "pi", "p", "legacy")
            .expect("legacy model");
        assert_eq!(old.login, ResourceLogin::Unknown);
        assert!(old.login_checked_at.is_empty());
        let _ = fs::remove_dir_all(root);
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
    fn declared_models_are_not_marked_missing() {
        // 申报 source 的消失条目不标「未在事实源中」（修正轮 R1 补测）。
        let mut store = ResourceStore {
            detected_at: None,
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
        let root = crate::paths::test_temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
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
            detected_at: None,
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
            detected_at: None,
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
            detected_at: None,
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
            detected_at: None,
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

    /// TASK-21：旧库中的真身不符记录要按当前路由别名表重判。命中的翻案，
    /// 未命中的仍是失败；整个过程只读旧数据与静态表，不运行冒烟。
    #[test]
    fn task21_known_route_alias_rejudges_old_smoke_failure() {
        let mut matched = ResourceModel {
            id: "deepseek-v4-flash".into(),
            agent: "pi".into(),
            provider: "deepseek".into(),
            status: STATUS_FAILED.into(),
            verification: VERIFICATION_UNVERIFIED.into(),
            smoke_failure: "真身不符：请求 deepseek/deepseek-v4-flash，响应 deepseek/deepseek-flash".into(),
            ..ResourceModel::default()
        };
        matched.smoked_at = "2026-09-14T08:45:02+08:00".into();
        let unmatched = ResourceModel {
            id: "other-model".into(),
            agent: "pi".into(),
            provider: "deepseek".into(),
            status: STATUS_FAILED.into(),
            verification: VERIFICATION_UNVERIFIED.into(),
            smoke_failure: "真身不符：请求 deepseek/other-model，响应 deepseek/unexpected".into(),
            ..ResourceModel::default()
        };
        let original = ResourceStore {
            detected_at: None,
            version: 1,
            agents: vec![fixture_agent("pi", vec![matched, unmatched])],
        };
        let root = crate::paths::test_temp_dir().join(format!("aicross-task21-rejudge-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        save_store(&path, &original).unwrap();

        let mut store = load(&path).expect("load must rejudge stored failures");

        let fixed = find_model(&store, "pi", "deepseek", "deepseek-v4-flash").unwrap();
        assert_eq!(fixed.verification, VERIFICATION_VERIFIED);
        assert_eq!(fixed.status, STATUS_CONNECTED);
        assert_eq!(fixed.resolved_id, "deepseek-flash");
        assert_eq!(fixed.resolved_source, RESOLVED_SOURCE_IDENTITY);
        assert_eq!(fixed.smoke_model, "deepseek/deepseek-flash");
        assert!(fixed.smoke_failure.is_empty());
        assert!(fixed.note.contains("按已知路由别名重判（"));

        merge_detected_models(
            &mut store,
            "pi",
            vec![detected_model(
                "deepseek-v4-flash",
                "pi",
                "deepseek",
                "DeepSeek",
                "deepseek",
                "未知",
                "来源：读取 fixture · 2026-09-16",
                "",
            )],
        );
        let redetected = find_model(&store, "pi", "deepseek", "deepseek-v4-flash").unwrap();
        assert_eq!(redetected.verification, VERIFICATION_VERIFIED);
        assert_eq!(redetected.resolved_id, "deepseek-flash");
        assert!(redetected.smoke_failure.is_empty());
        assert!(redetected.note.contains("按已知路由别名重判（"));

        let kept = find_model(&store, "pi", "deepseek", "other-model").unwrap();
        assert_eq!(kept.verification, VERIFICATION_FAILED);
        assert_eq!(kept.status, STATUS_FAILED);
        assert!(!kept.smoke_failure.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    /// TASK-21：一次普通冒烟失败必须直接进入 failed，不能和从未验证共用档位。
    #[test]
    fn task21_smoke_failure_is_recorded_as_failed() {
        let model = ResourceModel {
            id: "m".into(),
            agent: "pi".into(),
            provider: "p".into(),
            verification: VERIFICATION_UNVERIFIED.into(),
            ..ResourceModel::default()
        };
        let mut store = ResourceStore {
            detected_at: None,
            version: 1,
            agents: vec![fixture_agent("pi", vec![model])],
        };
        let key = ResourceModelKey { agent_id: "pi".into(), provider: "p".into(), id: "m".into() };

        apply_smoke_outcome(&mut store, &key, Err("端点拒绝".into())).unwrap();

        let saved = find_model(&store, "pi", "p", "m").unwrap();
        assert_eq!(saved.verification, VERIFICATION_FAILED);
        assert_eq!(saved.status, STATUS_FAILED);
        assert_eq!(saved.smoke_failure, "端点拒绝");
    }

    // ── TASK-42：冒烟超时单独记档，不拦派发 ──────────────────────────

    /// 超时的冒烟结果记 `timeout`，不与验证失败共用档位；原生模型恒为
    /// native；普通失败照旧 `failed`（对照）。
    #[test]
    fn task42_smoke_timeout_is_recorded_as_timeout_not_failed() {
        let model = ResourceModel {
            id: "m".into(),
            agent: "pi".into(),
            provider: "p".into(),
            verification: VERIFICATION_UNVERIFIED.into(),
            ..ResourceModel::default()
        };
        let mut store = ResourceStore {
            detected_at: None,
            version: 1,
            agents: vec![fixture_agent("pi", vec![model])],
        };
        let key = ResourceModelKey { agent_id: "pi".into(), provider: "p".into(), id: "m".into() };

        apply_smoke_outcome(&mut store, &key, Err(smoke_timeout_error())).unwrap();

        let saved = find_model(&store, "pi", "p", "m").unwrap();
        assert_eq!(saved.verification, VERIFICATION_TIMEOUT, "超时不应记成验证失败");
        assert_eq!(saved.status, STATUS_FAILED);
        assert_eq!(saved.smoke_failure, format!("冒烟超时（{SMOKE_TIMEOUT_SECS} 秒）"));

        // 原生模型的 verification 恒为 native，冒烟超时也不改档
        let native = ResourceModel {
            id: "n".into(),
            agent: "codex".into(),
            ..ResourceModel::default()
        };
        let mut store = ResourceStore {
            detected_at: None,
            version: 1,
            agents: vec![fixture_agent("codex", vec![native])],
        };
        let key = ResourceModelKey { agent_id: "codex".into(), provider: String::new(), id: "n".into() };
        apply_smoke_outcome(&mut store, &key, Err(smoke_timeout_error())).unwrap();
        assert_eq!(find_model(&store, "codex", "", "n").unwrap().verification, VERIFICATION_NATIVE);

        // 对照：普通失败照旧记 failed（task21 已有，这里就近再钉一遍）
        let model = ResourceModel {
            id: "m".into(),
            agent: "pi".into(),
            provider: "p".into(),
            verification: VERIFICATION_UNVERIFIED.into(),
            ..ResourceModel::default()
        };
        let mut store = ResourceStore {
            detected_at: None,
            version: 1,
            agents: vec![fixture_agent("pi", vec![model])],
        };
        let key = ResourceModelKey { agent_id: "pi".into(), provider: "p".into(), id: "m".into() };
        apply_smoke_outcome(&mut store, &key, Err("端点拒绝".into())).unwrap();
        assert_eq!(find_model(&store, "pi", "p", "m").unwrap().verification, VERIFICATION_FAILED);
    }

    /// 冒烟等待上限常量是 180 秒；超时错误正文可被 is_smoke_timeout 认出。
    #[test]
    fn task42_smoke_timeout_is_180_seconds() {
        assert_eq!(SMOKE_TIMEOUT_SECS, 180);
        assert_eq!(Duration::from_secs(SMOKE_TIMEOUT_SECS), Duration::from_secs(180));
        assert!(is_smoke_timeout(&smoke_timeout_error()));
        assert_eq!(smoke_timeout_error(), "冒烟超时（180 秒）");
        assert!(!is_smoke_timeout("端点拒绝"));
    }

    /// `resources` 状态映射：timeout → 冒烟超时 + 重试说明；没有 timeout 值的
    /// 旧数据不受影响（认不得的值仍按未验证展示）。
    #[test]
    fn task42_timeout_state_shows_retry_note() {
        let now = 2_000_000_000_000_i64;
        let model = ResourceModel {
            agent: "pi".into(),
            provider: "zai-coding-cn".into(),
            id: "glm-5.3".into(),
            verification: VERIFICATION_TIMEOUT.into(),
            ..ResourceModel::default()
        };
        let (state, note) = model_state(&model, true, now);
        assert_eq!(state, RESOURCE_STATE_TIMEOUT);
        assert_eq!(state, "冒烟超时");
        assert_eq!(note, "连接测试超时，不影响派发");
        let legacy = ResourceModel { verification: "legacy".into(), ..model };
        assert_eq!(model_state(&legacy, true, now).0, RESOURCE_STATE_UNVERIFIED);
    }

    /// 旧库加载不动超时档：有失败原因的 timeout 记录不被重判成 failed。
    #[test]
    fn task42_timeout_record_survives_reload() {
        let root = crate::paths::test_temp_dir().join(format!("aicross-task42-reload-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        fs::write(
            &path,
            r#"{"version":1,"agents":[
                {"id":"pi","name":"pi agent","kind":"内置","detected":true,"version":"0.86.1","models":[
                    {"id":"glm-5.3","agent":"pi","provider":"zai","status":"冒烟失败","verification":"timeout","smoke_failure":"冒烟超时（180 秒）","resolved_id":"glm-5.3","resolved_source":"alias-table"}
                ]}
            ]}"#,
        )
        .unwrap();
        let store = load(&path).expect("store");
        let saved = find_model(&store, "pi", "zai", "glm-5.3").unwrap();
        assert_eq!(saved.verification, VERIFICATION_TIMEOUT, "超时档不应被重判成 failed");
        assert_eq!(saved.smoke_failure, "冒烟超时（180 秒）");
        let _ = fs::remove_dir_all(root);
    }

    /// 复审补漏（09-16 Gemini 3）：路径正好以用户主目录结尾时也要缩成 `~`，
    /// 原来的正则强制捕获后面那个分隔符，没有分隔符就整串不替换。
    #[test]
    fn task21_home_shortening_also_handles_paths_ending_at_home() {
        assert_eq!(shorten_home_path(r"C:\Users\alice"), "~");
        assert_eq!(shorten_home_path(r"C:\Users\alice\.codex\models_cache.json"), r"~\.codex\models_cache.json");
        assert_eq!(shorten_home_path("/home/alice"), "~");
        assert_eq!(shorten_home_path("/Users/alice/.codex/x.json"), "~/.codex/x.json");
    }

    /// TASK-21：新探测写入统一来源形状，并在落库前缩短用户主目录。
    #[test]
    fn task21_source_writer_uses_uniform_shape_and_short_home() {
        let date = Local::now().format("%Y-%m-%d").to_string();
        assert_eq!(
            source_read(r"C:\Users\alice\.codex\models_cache.json"),
            format!(r"来源：读取 ~\.codex\models_cache.json · {date}")
        );
        assert_eq!(
            source_value("用户申报", "2026-09-15"),
            "来源：用户申报 · 2026-09-15"
        );
        assert_eq!(
            source_value("claude --help 别名", "2026-09-15"),
            "来源：claude --help 别名 · 2026-09-15"
        );
    }

    /// 修正轮 R1（GLM「最该补的测试 1」）：手动录入这条路径原来零测试，
    /// 把 `resolved_id` / `resolved_source` 那两行删掉现有测试也全绿。
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
        let root = crate::paths::test_temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
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
        let root = crate::paths::test_temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
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
        let root = crate::paths::test_temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
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
        // fix-fullcheck B2：验证失败过的不重试，也不放行，按 Rejected 拒绝派发。
        let second = verify_on_first_use_at(&path, "pi", "zai", "glm-4.7", smoke, || false);
        assert!(
            matches!(second, FirstUseVerification::Rejected(ref reason) if reason.contains("目标验证失败") && reason.contains("401")),
            "{second:?}"
        );
        assert_eq!(calls.get(), 1);
        let _ = fs::remove_dir_all(root);
    }

    /// fix-fullcheck B2 / B3 / B4：首次验证入口按资源库状态拦截——验证失败与
    /// 未发现返回 Rejected，冒烟超时放行（Skipped），都不调冒烟；provider
    /// 精确匹配，同名模型挂两个 provider（一失败一已验证）时各认各的，
    /// 不给 provider 时两种排列顺序都取可用的那条。
    #[test]
    fn fullcheck_first_use_rejects_failed_and_missing_by_exact_provider() {
        let root = crate::paths::test_temp_dir().join(format!("aicross-first-use-block-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        let never = |_: &ResourceModel| -> Result<SmokeSuccess, String> { panic!("不该冒烟") };
        let write = |models: &str| {
            fs::write(
                &path,
                format!(r#"{{"version":1,"agents":[{{"id":"pi","name":"pi","models":[{models}]}}]}}"#),
            )
            .unwrap();
        };
        let failed = r#"{"id":"glm","agent":"pi","provider":"bad","verification":"failed","smoke_failure":"403: denied"}"#;
        let verified = r#"{"id":"glm","agent":"pi","provider":"good","verification":"verified"}"#;
        for order in [format!("{failed},{verified}"), format!("{verified},{failed}")] {
            write(&order);
            assert!(matches!(
                verify_on_first_use_at(&path, "pi", "bad", "glm", never, || false),
                FirstUseVerification::Rejected(ref reason) if reason.contains("403")
            ));
            assert_eq!(
                verify_on_first_use_at(&path, "pi", "good", "glm", never, || false),
                FirstUseVerification::Skipped,
                "{order}"
            );
            assert_eq!(
                verify_on_first_use_at(&path, "pi", "", "glm", never, || false),
                FirstUseVerification::Skipped,
                "{order}"
            );
        }
        write(r#"{"id":"slow","agent":"pi","provider":"zai","verification":"timeout"}"#);
        assert_eq!(
            verify_on_first_use_at(&path, "pi", "zai", "slow", never, || false),
            FirstUseVerification::Skipped
        );
        write(&format!(
            r#"{{"id":"gone","agent":"pi","provider":"zai","verification":"verified","meta":"{MISSING_MARKER}"}}"#
        ));
        assert!(matches!(
            verify_on_first_use_at(&path, "pi", "zai", "gone", never, || false),
            FirstUseVerification::Rejected(ref reason) if reason.contains("未发现")
        ));
        let _ = fs::remove_dir_all(root);
    }

    fn b1_store(tag: &str) -> PathBuf {
        let root = crate::paths::test_temp_dir().join(format!("aicross-b1-{tag}-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        fs::write(
            &path,
            r#"{"version":1,"agents":[{"id":"pi","name":"pi","models":[
                {"id":"a","agent":"pi","provider":"zai","verification":"unverified"},
                {"id":"b","agent":"pi","provider":"zai","verification":"unverified"},
                {"id":"c","agent":"pi","provider":"zai","verification":"unverified"}]}]}"#,
        )
        .unwrap();
        path
    }

    /// fix-fullcheck B1：冒烟在锁外跑，期间另一个写方（这里在冒烟回调里
    /// 直接写，等同另一个进程交错写入）给别的模型打的额度标记与验证结果，
    /// 冒烟写回后都还在；写回只合并目标模型。单模型冒烟与按 agent 批量冒烟
    /// 同一口径。旧实现冒烟时持锁，回调里的写入会卡死在同一把锁上。
    #[test]
    fn fullcheck_b1_smoke_write_back_keeps_interleaved_writes() {
        let path = b1_store("single");
        let key = ResourceModelKey { agent_id: "pi".into(), id: "a".into(), provider: "zai".into() };
        let store = smoke_resource_model_with(&path, &key, |_| {
            mark_quota_exhausted_at(&path, "pi", "zai", "b", Some("2099-01-01T00:00:00+08:00")).unwrap();
            let _guard = store_lock(&path).unwrap();
            let mut store = load_or_default(&path).unwrap();
            find_model_mut(&mut store, "pi", "zai", "c").unwrap().verification = VERIFICATION_FAILED.into();
            save_store(&path, &store).unwrap();
            Ok(SmokeSuccess { identity: "zai/a".into(), meta_marker: None })
        })
        .unwrap();
        let stored = load(&path).unwrap();
        for view in [&store, &stored] {
            assert_eq!(find_model(view, "pi", "zai", "a").unwrap().verification, VERIFICATION_VERIFIED);
            assert_eq!(
                find_model(view, "pi", "zai", "b").unwrap().quota_exhausted_until.as_deref(),
                Some("2099-01-01T00:00:00+08:00")
            );
            assert_eq!(find_model(view, "pi", "zai", "c").unwrap().verification, VERIFICATION_FAILED);
        }
        let _ = fs::remove_dir_all(path.parent().unwrap());

        let path = b1_store("batch");
        let mut round = 0;
        smoke_agent_third_party_with(&path, "pi", |model| {
            round += 1;
            if round == 1 {
                // 冒烟 a 的期间，另一个写方把 c 标成额度用尽。
                mark_quota_exhausted_at(&path, "pi", "zai", "c", Some("2099-01-01T00:00:00+08:00")).unwrap();
            }
            Ok(SmokeSuccess { identity: format!("zai/{}", model.id), meta_marker: None })
        })
        .unwrap();
        let stored = load(&path).unwrap();
        assert_eq!(
            find_model(&stored, "pi", "zai", "c").unwrap().quota_exhausted_until.as_deref(),
            Some("2099-01-01T00:00:00+08:00")
        );
        assert_eq!(find_model(&stored, "pi", "zai", "a").unwrap().verification, VERIFICATION_VERIFIED);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    /// fix-fullcheck B1：资源库写入跨进程互斥。测试直接持有锁文件（等同另
    /// 一个进程正在写），写入要等它释放才落盘。
    #[test]
    fn fullcheck_b1_store_writes_wait_for_cross_process_lock() {
        let path = b1_store("xlock");
        let held = crate::file_lock::FileLock::acquire(
            &path.with_file_name("resources.json.lock"),
            Duration::from_secs(1),
        )
        .unwrap();
        let worker_path = path.clone();
        let worker = std::thread::spawn(move || {
            mark_quota_exhausted_at(&worker_path, "pi", "zai", "a", Some("2099-01-01T00:00:00+08:00"))
        });
        std::thread::sleep(Duration::from_millis(300));
        assert!(
            find_model(&load(&path).unwrap(), "pi", "zai", "a").unwrap().quota_exhausted_until.is_none(),
            "别的进程持锁时不写"
        );
        drop(held);
        worker.join().unwrap().unwrap();
        assert!(find_model(&load(&path).unwrap(), "pi", "zai", "a").unwrap().quota_exhausted_until.is_some());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn task14_r1_first_use_quota_keeps_model_unverified() {
        let root = crate::paths::test_temp_dir().join(format!("aicross-first-use-quota-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        fs::write(
            &path,
            r#"{"version":1,"agents":[{"id":"pi","name":"pi","models":[
                {"id":"glm-4.7","agent":"pi","provider":"zai","status":"待冒烟"}]}]}"#,
        ).unwrap();
        let outcome = verify_on_first_use_at(
            &path,
            "pi",
            "zai",
            "glm-4.7",
            |_| Err("HTTP 429 too many requests; Retry-After: 60".into()),
            || false,
        );
        assert!(matches!(outcome, FirstUseVerification::QuotaExhausted { resets_at: Some(_) }));
        let stored = load(&path).unwrap();
        let model = find_model(&stored, "pi", "zai", "glm-4.7").unwrap();
        assert_eq!(model.verification, VERIFICATION_UNVERIFIED);
        assert_ne!(model.status, STATUS_FAILED);
        assert!(model.smoke_failure.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    /// 第三方首次派发验证通过：写 verified，第二次派发不再验证。
    #[test]
    fn task09_first_use_success_is_verified_once_only() {
        let root = crate::paths::test_temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
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
        let root = crate::paths::test_temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
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

    #[test]
    fn task15_builtin_agent_display_names_are_canonical() {
        let mut store = ResourceStore::default();
        ensure_agents(&mut store);
        let names: Vec<(&str, &str)> = store
            .agents
            .iter()
            .map(|agent| (agent.id.as_str(), agent.name.as_str()))
            .collect();
        // TASK-61：骨架按支持清单补齐，固定四家在前、名称不变
        assert_eq!(
            names,
            vec![
                ("claude", "Claude Code"),
                ("codex", "Codex"),
                ("kimi", "Kimi Code"),
                ("pi", "pi agent"),
                ("agy", "Antigravity"),
                ("cursor", "Cursor"),
                ("codebuddy", "WorkBuddy / CodeBuddy"),
                ("copilot", "GitHub Copilot CLI"),
                ("qoder", "Qoder"),
            ]
        );
        for (id, name) in [
            ("claude", AGENT_NAME_CLAUDE),
            ("codex", AGENT_NAME_CODEX),
            ("kimi", AGENT_NAME_KIMI),
            ("pi", AGENT_NAME_PI),
        ] {
            assert_eq!(crate::agent_catalog::find(id).unwrap().name, name);
        }

        let mut old_store = ResourceStore {
            detected_at: None,
            version: 1,
            agents: vec![
                ResourceAgent { id: "kimi".into(), name: "Kimi Code CLI".into(), ..Default::default() },
                ResourceAgent { id: "pi".into(), name: "pi".into(), ..Default::default() },
            ],
        };
        ensure_agents(&mut old_store);
        assert_eq!(
            old_store.agents.iter().find(|agent| agent.id == "kimi").unwrap().name,
            "Kimi Code"
        );
        assert_eq!(
            old_store.agents.iter().find(|agent| agent.id == "pi").unwrap().name,
            "pi agent"
        );
    }

    /// 修正轮 R1-2：provider 为空时退到 agent + id 匹配，真的会验证（原来精确
    /// 匹配落空 → 静默 Skipped，G1 承诺了却一个请求都不发），结果写回原条目。
    #[test]
    fn r1_first_use_verifies_when_provider_is_empty() {
        let root = crate::paths::test_temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
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
        let root = crate::paths::test_temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
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
        let root = crate::paths::test_temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
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
        let root = crate::paths::test_temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
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
        let root = crate::paths::test_temp_dir().join(format!("aicross-resources-{}", Uuid::new_v4()));
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
                // 冒烟期间条目被删（资源库被改写 / 换掉）：写回会落空。
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
        let mut store = ResourceStore { detected_at: None, version: 1, agents: vec![fixture_agent("pi", vec![model])] };
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

    // ── TASK-39：命令行状态映射 ──────────────────────────────────────

    #[test]
    fn task39_agent_state_maps_detected_flag() {
        assert_eq!(agent_state(true), RESOURCE_STATE_AVAILABLE);
        assert_eq!(agent_state(false), RESOURCE_STATE_MISSING);
        let mut agent = fixture_agent("kimi", Vec::new());
        agent.detected = false;
        assert_eq!(agent_state_note(&agent), "安装后重新探测");
        agent.detected = true;
        assert_eq!(agent_state_note(&agent), "");
    }

    #[test]
    fn task39_model_state_covers_native_verification_login_and_quota() {
        let now = 2_000_000_000_000_i64;
        // agent 未安装（R1-4）：状态跟随 agent，note 与 agent 一致
        let undetected = ResourceModel {
            id: "k2".into(),
            agent: "kimi".into(),
            ..ResourceModel::default()
        };
        assert_eq!(
            model_state(&undetected, false, now),
            (RESOURCE_STATE_MISSING, "安装后重新探测".to_string())
        );
        // 原生：可用，无说明
        let native = detected_model("sonnet", "claude", "", "Anthropic", "官方订阅", "未知", "读:fixture", "");
        assert_eq!(model_state(&native, true, now), (RESOURCE_STATE_AVAILABLE, String::new()));
        // 第三方三档：已验证 / 验证失败 / 未验证
        let third = |verification: &str| ResourceModel {
            agent: "pi".into(),
            provider: "zai".into(),
            verification: verification.into(),
            ..ResourceModel::default()
        };
        assert_eq!(model_state(&third(VERIFICATION_VERIFIED), true, now).0, RESOURCE_STATE_VERIFIED);
        assert_eq!(model_state(&third(VERIFICATION_FAILED), true, now).0, RESOURCE_STATE_FAILED);
        assert_eq!(model_state(&third(VERIFICATION_UNVERIFIED), true, now).0, RESOURCE_STATE_UNVERIFIED);
        // 认不得的 verification 值按未验证展示，不让整个清单失效
        assert_eq!(model_state(&third("legacy"), true, now).0, RESOURCE_STATE_UNVERIFIED);
        // pi 未登录：note 给登录提示与刷新提示（R1-8）
        let logged_out = ResourceModel {
            login: ResourceLogin::LoggedOut,
            ..third(VERIFICATION_UNVERIFIED)
        };
        let (state, note) = model_state(&logged_out, true, now);
        assert_eq!(state, RESOURCE_STATE_LOGGED_OUT);
        assert_eq!(note, "未登录；在 pi 中运行 /login zai 后重新探测");
        assert_eq!(
            model_state(
                &ResourceModel { provider: String::new(), ..logged_out.clone() },
                true,
                now
            ).1,
            "未登录；在 pi 中运行 /login 后重新探测"
        );
        // 额度用尽：未过期显示恢复时间；已过期按验证结果展示。
        // 原生模型也挂额度标记，额度先于原生判（R1-3，与引擎拒源口径一致）。
        let mut quota = third(VERIFICATION_UNVERIFIED);
        quota.quota_exhausted_until = Some("2099-01-01T00:00:00+08:00".into());
        let (state, note) = model_state(&quota, true, now);
        assert_eq!(state, RESOURCE_STATE_QUOTA);
        assert_eq!(note, "额度恢复时间：2099-01-01T00:00:00+08:00");
        quota.quota_exhausted_until = Some("2000-01-01T00:00:00+08:00".into());
        assert_eq!(model_state(&quota, true, now).0, RESOURCE_STATE_UNVERIFIED);
        let mut native_quota = detected_model("sonnet", "claude", "", "Anthropic", "官方订阅", "未知", "读:fixture", "");
        native_quota.quota_exhausted_until = Some("2099-01-01T00:00:00+08:00".into());
        assert_eq!(model_state(&native_quota, true, now).0, RESOURCE_STATE_QUOTA);
        native_quota.quota_exhausted_until = Some("2000-01-01T00:00:00+08:00".into());
        assert_eq!(model_state(&native_quota, true, now).0, RESOURCE_STATE_AVAILABLE);
        // 额度未过期优先于未登录；agent 未安装优先于额度（状态跟随 agent）
        let quota_login = ResourceModel {
            quota_exhausted_until: Some("2099-01-01T00:00:00+08:00".into()),
            ..logged_out
        };
        assert_eq!(model_state(&quota_login, true, now).0, RESOURCE_STATE_QUOTA);
        assert_eq!(model_state(&quota_login, false, now).0, RESOURCE_STATE_MISSING);
    }


    /// TASK-48：meta 带「未在事实源中」标记的模型状态为「未发现」，第三方、
    /// 原生、原为「已验证」都一样；agent 不可用时仍是「未安装」；去掉标记
    /// （重新探测发现）后恢复原状态。
    #[test]
    fn task48_model_state_reports_not_detected() {
        let now = 2_000_000_000_000_i64;
        let marked = MISSING_MARKER.to_string();
        // 第三方带标记：未发现，note 给下一步
        let mut third = ResourceModel {
            agent: "pi".into(),
            provider: "zai".into(),
            verification: VERIFICATION_VERIFIED.into(),
            ..ResourceModel::default()
        };
        third.meta = marked.clone();
        assert_eq!(
            model_state(&third, true, now),
            (RESOURCE_STATE_NOT_DETECTED, NOT_DETECTED_NOTE.to_string())
        );
        // 原生带标记同样适用；额度、验证结果都被压在下面
        let mut native = detected_model("sonnet", "claude", "", "Anthropic", "官方订阅", "未知", "读:fixture", "");
        native.verification = VERIFICATION_VERIFIED.into();
        native.meta = marked.clone();
        assert_eq!(model_state(&native, true, now).0, RESOURCE_STATE_NOT_DETECTED);
        // agent 不可用时仍是未安装（状态跟随 agent）
        assert_eq!(model_state(&third, false, now).0, RESOURCE_STATE_MISSING);
        // 去掉标记（重新探测发现）后恢复原状态
        third.meta = String::new();
        assert_eq!(model_state(&third, true, now).0, RESOURCE_STATE_VERIFIED);
    }

    /// 复审追加：没给 provider 时，同名模型挂在多个 provider 下，优先取没带
    /// 「未发现」标记的那条；全部带标记才返回带标记的；给了 provider 只认那条。
    #[test]
    fn task48_find_target_model_prefers_detected_entry_without_provider() {
        let entry = |provider: &str, missing: bool| ResourceModel {
            agent: "pi".into(),
            provider: provider.into(),
            id: "deepseek-v4-flash".into(),
            meta: if missing { MISSING_MARKER.to_string() } else { String::new() },
            ..ResourceModel::default()
        };
        let models = vec![entry("deepseek", true), entry("opencode-go", false)];
        let found = find_target_model(&models, "pi", "", "deepseek-v4-flash").unwrap();
        assert_eq!(found.provider, "opencode-go");
        let exact = find_target_model(&models, "pi", "deepseek", "deepseek-v4-flash").unwrap();
        assert!(is_missing_from_detection(exact));
        let all_missing = vec![entry("deepseek", true), entry("old", true)];
        let found = find_target_model(&all_missing, "pi", "", "deepseek-v4-flash").unwrap();
        assert!(is_missing_from_detection(found));
        assert!(find_target_model(&models, "pi", "", "ghost").is_none());
    }

    /// TASK-48：合并只改标记不打扰原有规则——先探测到 A、B，再探测只有 A
    /// 时 B 变「未发现」；再探测到 B 后 B 恢复。
    #[test]
    fn task48_merge_marks_missing_then_recovers() {
        let now = 2_000_000_000_000_i64;
        let mut store = ResourceStore {
            detected_at: None,
            version: 1,
            agents: vec![fixture_agent(
                "codex",
                vec![fixture_model("a"), fixture_model("b")],
            )],
        };
        merge_detected_models(&mut store, "codex", vec![fixture_model("a"), fixture_model("b")]);
        let state = |store: &ResourceStore, id: &str| {
            let model = find_model(store, "codex", "", id).expect("model");
            model_state(model, true, now).0
        };
        assert_eq!(state(&store, "b"), RESOURCE_STATE_AVAILABLE);

        merge_detected_models(&mut store, "codex", vec![fixture_model("a")]);
        assert_eq!(state(&store, "b"), RESOURCE_STATE_NOT_DETECTED);

        merge_detected_models(&mut store, "codex", vec![fixture_model("a"), fixture_model("b")]);
        assert_eq!(state(&store, "b"), RESOURCE_STATE_AVAILABLE);
        let b = find_model(&store, "codex", "", "b").expect("b");
        assert!(!b.meta.contains(MISSING_MARKER), "重新发现后标记应随合并消失");
    }

    /// TASK-48：按 agent 冒烟跳过未发现的模型，不发请求，逐个给出跳过
    /// 说明；跳过不算失败。（fix-48-r1 R5：经冒烟函数可注入的入口走生产
    /// 同一条过滤与跳过说明。）
    #[test]
    fn task48_smoke_agent_skips_missing_models() {
        let root = crate::paths::test_temp_dir().join(format!("aicross-task48-smoke-skip-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        let mut missing = fixture_model("gone");
        missing.meta = MISSING_MARKER.to_string();
        save_store(
            &path,
            &ResourceStore {
                detected_at: None,
                version: 1,
                agents: vec![fixture_agent("pi", vec![fixture_model("glm-5.3"), missing])],
            },
        )
        .unwrap();
        let smoked = std::cell::RefCell::new(Vec::<String>::new());
        let operation = smoke_agent_third_party_with(&path, "pi", |model| {
            smoked.borrow_mut().push(model.id.clone());
            Ok(SmokeSuccess { identity: model.id.clone(), meta_marker: None })
        })
        .unwrap();
        assert_eq!(*smoked.borrow(), vec!["glm-5.3".to_string()], "只冒烟未带标记的");
        assert!(operation.errors.is_empty(), "跳过不算失败");
        assert_eq!(
            operation.skipped,
            vec![format!("跳过 pi/gone：{}", NOT_DETECTED_NOTE)]
        );
        let _ = fs::remove_dir_all(root);
    }

    /// fix-48-r1 R3：某 agent 下第三方模型全部未发现时，可冒烟的清单为空，
    /// 返回值仍是读到的那个库（含完整条目），跳过说明逐条在案；输出不能
    /// 变成空库、丢掉这个 agent。
    #[test]
    fn task48_smoke_agent_all_missing_keeps_store_entries() {
        let root = crate::paths::test_temp_dir().join(format!("aicross-task48-smoke-all-missing-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        let mut gone = fixture_model("gone");
        gone.meta = MISSING_MARKER.to_string();
        save_store(
            &path,
            &ResourceStore {
                detected_at: None,
                version: 1,
                agents: vec![fixture_agent("pi", vec![gone])],
            },
        )
        .unwrap();
        let operation = smoke_agent_third_party_with(&path, "pi", |_: &ResourceModel| {
            panic!("全部未发现时不该发请求")
        })
        .unwrap();
        let agent = operation.store.agents.iter().find(|agent| agent.id == "pi").expect("agent 应保留");
        assert_eq!(agent.models.len(), 1, "完整条目应保留");
        assert_eq!(agent.models[0].id, "gone");
        assert_eq!(
            operation.skipped,
            vec![format!("跳过 pi/gone：{}", NOT_DETECTED_NOTE)]
        );
        assert!(operation.errors.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    /// TASK-48：探测成功写回时更新 detectedAt；旧文件没有该字段时读出 None。
    #[test]
    fn task48_detect_updates_detected_at_and_old_files_read_none() {
        let root = crate::paths::test_temp_dir().join(format!("aicross-task48-detected-at-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        save_store(&path, &ResourceStore { detected_at: None, version: 1, agents: vec![fixture_agent("pi", Vec::new())] }).unwrap();
        let body = r#"#!/bin/sh
if [ "$1" = --version ]; then printf '%s\n' 'pi test'; exit 0; fi
if [ "$1" = --list-models ]; then printf '%s\n' 'ready model-a'; exit 0; fi
if [ "$1" = auth ]; then printf '%s\n' '{"status":"ready","provider":"ready","authType":"api_key","credentials":{"access_token":"do-not-read"}}'; exit 0; fi
exit 2
"#;
        let script = write_fake_pi(&root, body);
        let operation = detect_resource_agent_at_with_pi_binary(&path, "pi", None, Some(script)).unwrap();
        assert!(operation.errors.is_empty(), "{:?}", operation.errors);
        let stamp = operation.store.detected_at.clone().expect("detectedAt 应写入");
        chrono::DateTime::parse_from_rfc3339(&stamp).expect("detectedAt 应为 RFC3339");
        // 落盘的字段名是 detectedAt，旧文件没有该字段时读出 None。
        let raw: serde_json::Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(raw["detectedAt"].as_str(), Some(stamp.as_str()));
        fs::write(&path, r#"{"version":1,"agents":[]}"#).unwrap();
        assert_eq!(load(&path).unwrap().detected_at, None);
        let _ = fs::remove_dir_all(root);
    }

    /// TASK-39：命令行按 agent 冒烟只挑第三方模型。claude 走官方端点时是
    /// 原生（不冒烟）；挂了 ANTHROPIC_BASE_URL 就是第三方（要冒烟）。
    #[test]
    fn task39_smoke_agent_third_party_respects_endpoint_kind() {
        let root = crate::paths::test_temp_dir().join(format!("aicross-task39-smoke-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        let claude_model = ResourceModel {
            id: "sonnet".into(),
            agent: "claude".into(),
            status: STATUS_PENDING.into(),
            ..ResourceModel::default()
        };
        save_store(
            &path,
            &ResourceStore { detected_at: None, version: 1, agents: vec![fixture_agent("claude", vec![claude_model])] },
        )
        .unwrap();

        set_claude_third_party(false);
        let calls = std::cell::Cell::new(0u32);
        let operation = smoke_agent_models_filtered_at(
            &path,
            "claude",
            |model| !is_native_model(&model.agent),
            |_| None,
            |_| {
                calls.set(calls.get() + 1);
                Ok(SmokeSuccess { identity: "sonnet".into(), meta_marker: None })
            },
        )
        .unwrap();
        assert!(operation.errors.is_empty(), "{:?}", operation.errors);
        assert_eq!(calls.get(), 0, "官方端点的 claude 是原生，不冒烟");
        assert_eq!(
            find_model(&load(&path).unwrap(), "claude", "", "sonnet").unwrap().status,
            STATUS_PENDING
        );

        set_claude_third_party(true);
        let operation = smoke_agent_models_filtered_at(
            &path,
            "claude",
            |model| !is_native_model(&model.agent),
            |_| None,
            |_| {
                calls.set(calls.get() + 1);
                Ok(SmokeSuccess { identity: "claude/sonnet".into(), meta_marker: None })
            },
        )
        .unwrap();
        assert!(operation.errors.is_empty(), "{:?}", operation.errors);
        assert_eq!(calls.get(), 1, "第三方端点的 claude 要冒烟");
        let smoked_store = load(&path).unwrap();
        let smoked = find_model(&smoked_store, "claude", "", "sonnet").unwrap();
        assert_eq!(smoked.status, STATUS_CONNECTED);
        assert_eq!(smoked.verification, VERIFICATION_VERIFIED);
        set_claude_third_party(false);
        let _ = fs::remove_dir_all(root);
    }

    /// TASK-52：prune 只删带「未在事实源中」标记的模型；探测到、用户申报的
    /// 都保留；删掉的以 `agent/provider/model` 键列在 removed 并落盘。
    #[test]
    fn task52_prune_removes_only_missing_models() {
        let root = crate::paths::test_temp_dir().join(format!("aicross-task52-prune-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        let mut missing = detected_model("gone", "pi", "zai", "Zhipu", "按次", "支持", "读:探测", "");
        missing.meta = MISSING_MARKER.to_string();
        let declared = detected_model("mine", "pi", "", "OpenAI", "官方订阅", "支持", "申报(手动添加)", "");
        save_store(
            &path,
            &ResourceStore {
                detected_at: None,
                version: 1,
                agents: vec![fixture_agent("pi", vec![fixture_model("kept"), missing, declared])],
            },
        )
        .unwrap();
        let outcome = prune_missing_models_at(&path, None, &[]).unwrap();
        assert_eq!(outcome.removed, vec!["pi/zai/gone".to_string()], "只删带标记的");
        assert!(outcome.referenced.is_empty());
        let persisted = load(&path).unwrap();
        let ids: Vec<&str> = persisted.agents[0]
            .models
            .iter()
            .map(|model| model.id.as_str())
            .collect();
        assert_eq!(ids, vec!["kept", "mine"], "探测到的与用户申报的都保留");
        let _ = fs::remove_dir_all(root);
    }

    /// TASK-52：给了 agent 只清这一家（大小写不敏感），别家的未发现模型
    /// 不动；不给则清全库。
    #[test]
    fn task52_prune_with_agent_filter_only_cleans_that_agent() {
        let root = crate::paths::test_temp_dir().join(format!("aicross-task52-prune-agent-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        let mut pi_missing = detected_model("gone", "pi", "zai", "Zhipu", "按次", "支持", "读:探测", "");
        pi_missing.meta = MISSING_MARKER.to_string();
        let mut codex_missing = detected_model("ghost", "codex", "", "OpenAI", "官方订阅", "支持", "读:探测", "");
        codex_missing.meta = MISSING_MARKER.to_string();
        save_store(
            &path,
            &ResourceStore {
                detected_at: None,
                version: 1,
                agents: vec![
                    fixture_agent("pi", vec![pi_missing]),
                    fixture_agent("codex", vec![codex_missing]),
                ],
            },
        )
        .unwrap();
        let outcome = prune_missing_models_at(&path, Some("PI"), &[]).unwrap();
        assert_eq!(outcome.removed, vec!["pi/zai/gone".to_string()]);
        let persisted = load(&path).unwrap();
        let codex = persisted.agents.iter().find(|agent| agent.id == "codex").unwrap();
        assert_eq!(codex.models.len(), 1, "别家的未发现模型不动");
        let outcome = prune_missing_models_at(&path, None, &[]).unwrap();
        assert_eq!(outcome.removed, vec!["codex//ghost".to_string()]);
        let _ = fs::remove_dir_all(root);
    }

    /// TASK-52：被引用的未发现模型照样删除，另列在 referenced；provider
    /// 空的指向不限 provider；被引用但没带标记的不进这两个列表。
    #[test]
    fn task52_prune_lists_referenced_but_still_removes() {
        let root = crate::paths::test_temp_dir().join(format!("aicross-task52-prune-ref-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        let mut seated = detected_model("gone", "pi", "zai", "Zhipu", "按次", "支持", "读:探测", "");
        seated.meta = MISSING_MARKER.to_string();
        let mut roled = detected_model("ghost", "pi", "bigmodel", "Zhipu", "按次", "支持", "读:探测", "");
        roled.meta = MISSING_MARKER.to_string();
        let healthy = detected_model("kept", "pi", "zai", "Zhipu", "按次", "支持", "读:探测", "");
        save_store(
            &path,
            &ResourceStore {
                detected_at: None,
                version: 1,
                agents: vec![fixture_agent("pi", vec![seated, roled, healthy])],
            },
        )
        .unwrap();
        let referenced = vec![
            ("pi".to_string(), "zai".to_string(), "gone".to_string()),
            // provider 为空：不限 provider，同一模型挂在哪家都算被引用
            ("pi".to_string(), String::new(), "ghost".to_string()),
            ("pi".to_string(), "zai".to_string(), "kept".to_string()),
        ];
        let outcome = prune_missing_models_at(&path, None, &referenced).unwrap();
        assert_eq!(
            outcome.removed,
            vec!["pi/zai/gone".to_string(), "pi/bigmodel/ghost".to_string()]
        );
        assert_eq!(outcome.referenced, outcome.removed, "删掉的都被引用，单列在 referenced");
        let persisted = load(&path).unwrap();
        assert_eq!(persisted.agents[0].models.len(), 1, "只留下没带标记的那条");
        let _ = fs::remove_dir_all(root);
    }

    /// TASK-52：prune 删掉的模型，再次探测发现后会重新加入资源库。
    #[test]
    fn task52_prune_then_detect_readds_rediscovered_model() {
        let root = crate::paths::test_temp_dir().join(format!("aicross-task52-prune-redetect-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        let mut gone = detected_model("gone", "pi", "zai", "Zhipu", "按次", "支持", "读:探测", "");
        gone.meta = MISSING_MARKER.to_string();
        save_store(
            &path,
            &ResourceStore {
                detected_at: None,
                version: 1,
                agents: vec![fixture_agent("pi", vec![gone])],
            },
        )
        .unwrap();
        let outcome = prune_missing_models_at(&path, None, &[]).unwrap();
        assert_eq!(outcome.removed.len(), 1);
        assert!(load(&path).unwrap().agents[0].models.is_empty());

        // 再次探测发现：merge 把这条重新加回，标记随合并消失。
        let mut store = load(&path).unwrap();
        merge_detected_models(
            &mut store,
            "pi",
            vec![detected_model("gone", "pi", "zai", "Zhipu", "按次", "支持", "读:探测", "")],
        );
        save_store(&path, &store).unwrap();
        let reloaded = load(&path).unwrap();
        let model = find_model(&reloaded, "pi", "zai", "gone").unwrap();
        assert!(!is_missing_from_detection(model));
        let _ = fs::remove_dir_all(root);
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
            detected_at: None,
            version: 1,
            agents: Vec::new(),
        };
        ensure_agents(&mut store);
        let mut errors = Vec::new();
        for id in ["claude", "codex", "kimi"] {
            apply_prepared_detection(&mut store, prepare_agent_detection(id), &mut errors);
        }
        apply_prepared_pi_detection(&mut store, prepare_pi_detection(resolve_cli("pi")), &mut errors);
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
            detected_at: None,
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
            let target = find_model(&store, "pi", provider, id).expect("model").clone();
            apply_smoke_outcome(&mut store, &key, run_smoke(&target)).expect("apply_smoke_outcome");
            let model = find_model(&store, "pi", provider, id).expect("model");
            println!(
                "smoke pi/{provider}/{id}: status={} smoke_model={} failure={}",
                model.status, model.smoke_model, model.smoke_failure
            );
        }
    }

    /// TASK-61：新 agent 的厂商按模型名推，认不出的记「未知」。
    #[test]
    fn task61_listed_model_vendor_by_model_name() {
        assert_eq!(vendor_for_listed_model("gemini-3.8-flash-low"), "Google");
        assert_eq!(vendor_for_listed_model("claude-opus-5-5-high"), "Anthropic");
        assert_eq!(vendor_for_listed_model("gpt-oss-120b-medium"), "OpenAI");
        assert_eq!(vendor_for_listed_model("gpt-5"), "OpenAI");
        assert_eq!(vendor_for_listed_model("o3"), "OpenAI");
        assert_eq!(vendor_for_listed_model("auto"), VENDOR_UNKNOWN);
        for tier in QODER_TIERS {
            assert_eq!(vendor_for_listed_model(tier), VENDOR_UNKNOWN, "{tier}");
        }
    }

    /// TASK-61：`agy models` 的「id<TAB>显示名」行（1.3.1 真机输出节选），
    /// 跳过 `Fetching available models...`；原生、官方订阅、厂商按模型分开。
    #[test]
    fn task61_parse_agy_models_skips_banner_and_splits_vendors() {
        set_claude_third_party(false);
        let stdout = "Fetching available models...\n\
gemini-3.8-flash-high\tGemini 3.8 Flash (High)\n\
gemini-3.8-flash-low\tGemini 3.8 Flash (Low)\n\
claude-opus-5-5-low\tClaude Opus 5.5 (Low)\n\
gpt-oss-120b-medium\tGPT-OSS 120B (Medium)\n\
\n";
        let models = parse_agy_models(stdout, "来源：读取 agy models · 2026-10-07").unwrap();
        let summary: Vec<(&str, &str)> =
            models.iter().map(|model| (model.id.as_str(), model.vendor.as_str())).collect();
        assert_eq!(
            summary,
            vec![
                ("gemini-3.8-flash-high", "Google"),
                ("gemini-3.8-flash-low", "Google"),
                ("claude-opus-5-5-low", "Anthropic"),
                ("gpt-oss-120b-medium", "OpenAI"),
            ]
        );
        for model in &models {
            assert_eq!(model.agent, "agy");
            assert_eq!(model.provider, "");
            assert_eq!(model.verification, VERIFICATION_NATIVE);
            assert_eq!(model.billing_source, "官方订阅");
        }
        assert_eq!(models[0].meta, "Gemini 3.8 Flash (High)");
        assert!(parse_agy_models("Fetching available models...\n", "s").is_err());
    }

    /// TASK-61：cursor-agent models 按「id - 显示名」解析（假输出）；解析不出报错。
    #[test]
    fn task61_parse_cursor_models_with_fake_output() {
        let stdout = "Available models\n\nauto - Auto  (current)\ngpt-5 - GPT-5\nsonnet-4.5 - Claude 4.5 Sonnet\n\nTip: use --model <id>\n";
        let models = parse_cursor_models(stdout, "s").unwrap();
        let summary: Vec<(&str, &str, &str)> = models
            .iter()
            .map(|model| (model.id.as_str(), model.vendor.as_str(), model.meta.as_str()))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("auto", VENDOR_UNKNOWN, "Auto"),
                ("gpt-5", "OpenAI", "GPT-5"),
                ("sonnet-4.5", VENDOR_UNKNOWN, "Claude 4.5 Sonnet"),
            ]
        );
        // 未实测 agent 不是原生：首次派发前要验证
        assert!(models.iter().all(|model| model.verification == VERIFICATION_UNVERIFIED));
        assert!(parse_cursor_models("Error: not logged in\n", "s").is_err());
    }

    /// TASK-61：copilot / codebuddy 读随版本发布的小表（注明来源与日期）；
    /// qoder 固定五档，厂商全「未知」。
    #[test]
    fn task61_static_table_and_qoder_tiers() {
        for agent in ["copilot", "codebuddy"] {
            let models = static_table_models(agent).unwrap();
            assert!(!models.is_empty(), "{agent}");
            assert!(models.iter().all(|model| model.agent == agent));
            assert!(models[0].source.contains("内置模型表"), "{}", models[0].source);
            assert!(models[0].source.contains("2026-10-07"), "{}", models[0].source);
        }
        assert!(parse_static_table("{\"agents\":{}}", "copilot").is_err());
        let qoder = qoder_tier_models();
        assert_eq!(
            qoder.iter().map(|model| model.id.as_str()).collect::<Vec<_>>(),
            vec!["lite", "efficient", "auto", "performance", "ultimate"]
        );
        assert!(qoder.iter().all(|model| model.vendor == VENDOR_UNKNOWN));
    }

    /// fix-61-r1 3：agy 冒烟在 `$AICROSS_HOME/scratch/` 下的专属空目录里跑，
    /// 离开作用域（成功、失败、超时都一样）即删除；其余 agent 不建。
    #[test]
    fn task61_r1_agy_smoke_scratch_is_private_and_removed() {
        let root = crate::paths::test_temp_dir().join(format!("aicross-t61-smoke-{}", Uuid::new_v4()));
        let first = SmokeScratch::create_in(&root, "agy").unwrap().unwrap();
        let second = SmokeScratch::create_in(&root, "agy").unwrap().unwrap();
        assert_ne!(first.path, second.path, "每次调用专属");
        assert!(first.path.starts_with(root.join("scratch")));
        assert!(fs::read_dir(&first.path).unwrap().next().is_none(), "空目录");
        let (first_path, second_path) = (first.path.clone(), second.path.clone());
        drop(first);
        assert!(!first_path.exists());
        // 提前返回（失败、超时）同样经 Drop 清理
        let failed = || -> Result<(), String> {
            let _scratch = second;
            Err("冒烟超时（180 秒）".into())
        };
        assert!(failed().is_err());
        assert!(!second_path.exists());
        for agent in ["claude", "codex", "kimi", "pi"] {
            assert!(SmokeScratch::create_in(&root, agent).unwrap().is_none(), "{agent}");
        }
        let _ = fs::remove_dir_all(root);
    }

    /// fix-61-r1 7：cursor-agent models 解析失败时合并空清单，已有模型标
    /// 「未在事实源中」（不再可选），并写一条错误。
    #[test]
    fn task61_r1_cursor_parse_failure_marks_existing_models_missing() {
        let mut store = ResourceStore::default();
        ensure_agents(&mut store);
        let existing = parse_cursor_models("gpt-5 - GPT-5\n", "s").unwrap();
        merge_detected_models(&mut store, "cursor", existing);
        let (models, error) =
            cursor_models_or_mark_missing(parse_cursor_models("Error: not logged in\n", "s"));
        assert!(error.as_deref().is_some_and(|error| error.starts_with("探测 cursor 失败")));
        let mut errors = Vec::new();
        apply_prepared_plain_detection(
            &mut store,
            PreparedAgentDetection {
                agent_id: "cursor".into(),
                detected: true,
                version: "test".into(),
                missing_error: error,
                models,
            },
            &mut errors,
        );
        assert_eq!(errors.len(), 1, "{errors:?}");
        let cursor = store.agents.iter().find(|agent| agent.id == "cursor").unwrap();
        assert_eq!(cursor.models.len(), 1);
        assert!(is_missing_from_detection(&cursor.models[0]));
        // 解析成功时照常合并、不写错误
        let (models, error) = cursor_models_or_mark_missing(parse_cursor_models("gpt-5 - GPT-5\n", "s"));
        assert!(error.is_none());
        assert_eq!(models.unwrap().unwrap().len(), 1);
    }

    /// fix-61-r1 4：探测与派发 / 冒烟找 agy 用同一个解析函数，结果一致。
    #[test]
    fn task61_r1_agy_program_resolution_is_shared() {
        assert_eq!(resolve_cli("agy"), crate::agent_catalog::resolve_program("agy"));
        assert_eq!(resolve_cli("kimi"), crate::agent_catalog::resolve_program("kimi"));
    }

    /// TASK-61：骨架按支持清单补齐并刷新 untested / homepage（旧库缺字段或
    /// 写错都按清单改正）；agy 是原生，未实测四家不是。
    #[test]
    fn task61_ensure_agents_fills_untested_and_homepage_from_catalog() {
        set_claude_third_party(false);
        let mut store: ResourceStore = serde_json::from_str(
            r#"{"version":1,"agents":[{"id":"cursor","name":"x","untested":false}]}"#,
        )
        .unwrap();
        ensure_agents(&mut store);
        let cursor = store.agents.iter().find(|agent| agent.id == "cursor").unwrap();
        assert!(cursor.untested);
        assert_eq!(cursor.name, "Cursor");
        assert_eq!(cursor.homepage, "https://cursor.com/cli");
        let agy = store.agents.iter().find(|agent| agent.id == "agy").unwrap();
        assert!(!agy.untested);
        assert!(!agy.detected);
        assert!(agy.models.is_empty());
        assert!(is_native_model("agy"));
        assert!(!is_native_model("cursor"));
        assert!(native_smoke_dispatches("agy"));
        assert!(!native_smoke_dispatches("claude"));
        let json = serde_json::to_value(&store).unwrap();
        assert_eq!(json["agents"][0]["untested"], true);
        assert!(json["agents"][0]["homepage"].is_string());
    }
}
