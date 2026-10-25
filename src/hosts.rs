//! 主 agent 检测与厂商推定（TASK-50 第 1 节）。
//!
//! 可作主 agent 的只有四个：claude、codex、kimi、pi，且只列资源库里
//! `detected` 为 true 的。厂商推定只读各家配置文件，不读任何密钥：
//! claude 看 ANTHROPIC_BASE_URL（settings.json 与进程环境变量），codex 看
//! config.toml 的 model_provider，kimi 看 config.toml 的 provider，pi 拿
//! 默认模型到资源库里查厂商。每个结果带推定来源（推定 / 端点 / 用户设定 /
//! 未知）与一行依据；用户在 modes.json 里写的 vendorOverride 优先。
//!
//! 家目录一律由调用方注入：生产用 `paths::home()`，测试用临时目录，
//! 绝不碰真实家目录。

use crate::orchestration::ModesFile;
use crate::resources::{self, ResourceStore};
use serde::Serialize;
use std::path::Path;

pub(crate) const HOST_IDS: [&str; 4] = ["claude", "codex", "kimi", "pi"];

/// 只当主 agent 用的软件：不在派发通道里，派发仍由四家与支持清单里的 agent
/// 承担。点「探测」时按配置目录或命令是否存在记进资源库（`extraHosts`），
/// 之后与四家一样可选为主 agent、各存一份配置。skill 目录取各家官方文档或本机
/// 实测；显式调用写法核实不了的用自然语言，靠 skill 描述触发。
pub(crate) struct ExtraHost {
    pub id: &'static str,
    pub name: &'static str,
    /// 任一命令在 PATH 里即算装了。
    pub commands: &'static [&'static str],
    /// 任一目录（相对家目录，`/` 分隔）存在即算装了。
    pub markers: &'static [&'static str],
    /// 全局 skill 目录（相对家目录），ai-cross 装在它下面。
    pub skills_dir: &'static str,
    /// 调用 ai-cross 的写法，用在示例输入开头。
    pub invoke: &'static str,
    /// 只用一家模型的软件写厂商；能换模型的为 None，由用户在模式页设定。
    pub vendor: Option<(&'static str, &'static str)>,
}

pub(crate) const EXTRA_HOSTS: &[ExtraHost] = &[
    ExtraHost { id: "opencode", name: "OpenCode", commands: &["opencode"], markers: &[".config/opencode"],
        skills_dir: ".config/opencode/skills", invoke: "用 ai-cross", vendor: None },
    ExtraHost { id: "gemini", name: "Gemini CLI", commands: &["gemini"], markers: &[],
        skills_dir: ".gemini/skills", invoke: "用 ai-cross",
        vendor: Some(("Google", "Gemini CLI 只用 Google 的模型")) },
    ExtraHost { id: "agy", name: "Antigravity", commands: &["agy", "antigravity"],
        markers: &[".gemini/antigravity", ".gemini/antigravity-cli"],
        skills_dir: ".gemini/antigravity/skills", invoke: "用 ai-cross", vendor: None },
    ExtraHost { id: "hermes", name: "Hermes Agent", commands: &["hermes"], markers: &[".hermes"],
        skills_dir: ".hermes/skills", invoke: "/ai-cross", vendor: None },
    // ZCode 读 ~/.agents/skills，不读 ~/.zcode/skills（skillmanager 08-16 本机实测）。
    ExtraHost { id: "zcode", name: "ZCode", commands: &[], markers: &[".zcode"],
        skills_dir: ".agents/skills", invoke: "用 ai-cross", vendor: None },
    ExtraHost { id: "codebuddy", name: "CodeBuddy", commands: &["codebuddy"], markers: &[".codebuddy"],
        skills_dir: ".codebuddy/skills", invoke: "用 ai-cross", vendor: None },
    ExtraHost { id: "workbuddy", name: "WorkBuddy", commands: &[], markers: &[".workbuddy"],
        skills_dir: ".workbuddy/skills", invoke: "用 ai-cross", vendor: None },
    ExtraHost { id: "qoder", name: "Qoder", commands: &["qodercli"], markers: &[".qoder"],
        skills_dir: ".qoder/skills", invoke: "/ai-cross", vendor: None },
    ExtraHost { id: "qoder-cn", name: "Qoder CN", commands: &["qoderclicn"], markers: &[".qoder-cn"],
        skills_dir: ".qoder-cn/skills", invoke: "/ai-cross", vendor: None },
];

pub(crate) fn extra_host(id: &str) -> Option<&'static ExtraHost> {
    EXTRA_HOSTS.iter().find(|host| host.id == id)
}

/// 四家与扩展主 agent 的全部 id（固定顺序）。
pub(crate) fn all_host_ids() -> Vec<&'static str> {
    HOST_IDS.iter().copied().chain(EXTRA_HOSTS.iter().map(|host| host.id)).collect()
}

pub(crate) fn is_host_id(id: &str) -> bool {
    HOST_IDS.contains(&id) || extra_host(id).is_some()
}

/// 本机装了哪些扩展主 agent：配置目录存在或命令在 PATH 里。只查文件系统，
/// 不起进程；`path_var` 由调用方给（生产传 PATH 环境变量，测试注入）。
pub(crate) fn detect_extra_hosts(home: &Path, path_var: Option<&str>) -> Vec<String> {
    EXTRA_HOSTS
        .iter()
        .filter(|host| {
            host.markers.iter().any(|marker| home.join(marker).is_dir())
                || host.commands.iter().any(|command| on_path(command, path_var))
        })
        .map(|host| host.id.to_string())
        .collect()
}

fn on_path(command: &str, path_var: Option<&str>) -> bool {
    let Some(path_var) = path_var else { return false };
    let names: Vec<String> = if cfg!(windows) {
        ["exe", "cmd", "bat", "ps1"].iter().map(|ext| format!("{command}.{ext}")).collect()
    } else {
        vec![command.to_string()]
    };
    std::env::split_paths(path_var)
        .any(|dir| names.iter().any(|name| dir.join(name).is_file()))
}

/// 推定来源：默认按官方端点猜的。
pub(crate) const VENDOR_SOURCE_INFERRED: &str = "推定";
/// 推定来源：按第三方端点 / provider 名称对出来的。
pub(crate) const VENDOR_SOURCE_ENDPOINT: &str = "端点";
/// 推定来源：用户在配置里指定的。
pub(crate) const VENDOR_SOURCE_OVERRIDE: &str = "用户设定";
/// 推定来源：认不出。
pub(crate) const VENDOR_SOURCE_UNKNOWN: &str = "未知";

/// `aicross hosts` 的一项；窗口 `hosts_list` 同此结构。
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HostInfo {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub vendor_source: String,
    pub vendor_basis: String,
}

/// 全仓共用的厂商取值：统一用资源库的英文名（resources::infer_vendor 的
/// 输出），比较与写盘都先过 normalize_vendor，比较不区分大小写。
pub(crate) const KNOWN_VENDORS: &[&str] = &[
    "Anthropic", "OpenAI", "Google", "Moonshot", "Zhipu", "DeepSeek",
    "Alibaba", "MiniMax", "StepFun", "OpenRouter",
];

/// 厂商规范化：认得的写法（英文别名、中文旧写法、大小写变体）统一成
/// 资源库的英文名；认不出的原样返回（trim 后）。写 vendorOverride 与推荐
/// 里的厂商比较都先过这里。
pub(crate) fn normalize_vendor(value: &str) -> String {
    let lowered = value.trim().to_lowercase();
    let canonical = match lowered.as_str() {
        "anthropic" => "Anthropic",
        "openai" => "OpenAI",
        "google" | "gemini" => "Google",
        "moonshot" | "月之暗面" | "kimi" | "moonshot ai" => "Moonshot",
        "zhipu" | "智谱" | "zai" | "bigmodel" | "glm" | "zhipu ai" => "Zhipu",
        "deepseek" => "DeepSeek",
        "alibaba" | "阿里" | "阿里云" | "aliyun" | "dashscope" | "qwen" => "Alibaba",
        "minimax" => "MiniMax",
        "stepfun" | "step" => "StepFun",
        "openrouter" => "OpenRouter",
        _ => return value.trim().to_string(),
    };
    canonical.to_string()
}

/// 是否认得的厂商写法（规范化后命中 KNOWN_VENDORS 之一）。
pub(crate) fn is_known_vendor(value: &str) -> bool {
    let normalized = normalize_vendor(value);
    KNOWN_VENDORS.iter().any(|vendor| vendor.eq_ignore_ascii_case(&normalized))
}

/// 端点 URL 主机名 → 厂商（claude 的 ANTHROPIC_BASE_URL：先解析出主机名，
/// 再按主机名包含关键字小写比对）。表随版本更新。
const URL_VENDOR_RULES: &[(&str, &str)] = &[
    ("anthropic.com", "Anthropic"),
    ("bigmodel.cn", "Zhipu"),
    ("minimaxi.com", "MiniMax"),
    ("z.ai", "Zhipu"),
    ("moonshot", "Moonshot"),
    ("kimi.com", "Moonshot"),
    ("deepseek", "DeepSeek"),
    ("dashscope", "Alibaba"),
    ("aliyun", "Alibaba"),
    ("minimax", "MiniMax"),
    ("stepfun", "StepFun"),
    ("openrouter", "OpenRouter"),
];

/// provider 名称关键字 → 厂商（codex 的 model_provider、kimi 的 provider；
/// 包含匹配，小写比对）。表随版本更新。
const PROVIDER_VENDOR_RULES: &[(&str, &str)] = &[
    ("openai", "OpenAI"),
    ("anthropic", "Anthropic"),
    ("moonshot", "Moonshot"),
    ("kimi", "Moonshot"),
    ("bigmodel", "Zhipu"),
    ("zhipu", "Zhipu"),
    ("zai", "Zhipu"),
    ("glm", "Zhipu"),
    ("deepseek", "DeepSeek"),
    ("dashscope", "Alibaba"),
    ("aliyun", "Alibaba"),
    ("qwen", "Alibaba"),
    ("minimax", "MiniMax"),
    ("stepfun", "StepFun"),
    ("openrouter", "OpenRouter"),
    ("google", "Google"),
    ("gemini", "Google"),
];

/// 各家显示名；与资源库内置 agent 的 name 一致。
fn host_display_name(id: &str) -> &'static str {
    match id {
        "claude" => resources::AGENT_NAME_CLAUDE,
        "codex" => resources::AGENT_NAME_CODEX,
        "kimi" => resources::AGENT_NAME_KIMI,
        "pi" => resources::AGENT_NAME_PI,
        other => extra_host(other).map_or("未知软件", |host| host.name),
    }
}

/// 资源库里检测到的主 agent id：四家看 agent 的 detected，扩展主 agent 看最近
/// 一次探测记下的 `extraHosts`；顺序同 [`all_host_ids`]。
pub(crate) fn detected_host_ids(store: &ResourceStore) -> Vec<String> {
    all_host_ids()
        .into_iter()
        .filter(|id| {
            if HOST_IDS.contains(id) {
                store.agents.iter().any(|agent| agent.id == *id && agent.detected)
            } else {
                store.extra_hosts.iter().any(|host| host == id)
            }
        })
        .map(str::to_string)
        .collect()
}

/// 主 agent 清单（`aicross hosts`）。资源库读不了时报错，由调用方定退出码。
/// `claude_base_url_env` 是进程环境变量 ANTHROPIC_BASE_URL 的值，由调用方
/// 注入（生产传 `std::env::var` 的结果，测试传 None），不在库里读真实环境。
pub(crate) fn list_hosts_at(
    root: &Path,
    home: &Path,
    claude_base_url_env: Option<&str>,
) -> Result<Vec<HostInfo>, String> {
    let store = resources::get_resources_at(&root.join("resources.json"))?;
    let modes = crate::orchestration::modes_from(root);
    Ok(collect_hosts(&store, &modes, home, claude_base_url_env))
}

/// 主 agent 清单的实现体；资源库、modes 与 claude 的环境变量值都由调用方
/// 给（测试注入）。
pub(crate) fn collect_hosts(
    store: &ResourceStore,
    modes: &ModesFile,
    home: &Path,
    claude_base_url_env: Option<&str>,
) -> Vec<HostInfo> {
    detected_host_ids(store)
        .iter()
        .map(String::as_str)
        .map(|id| {
            let (vendor, source, basis) = infer_vendor(id, home, store, claude_base_url_env);
            let mut info = HostInfo {
                id: id.to_string(),
                name: host_display_name(id).to_string(),
                vendor,
                vendor_source: source,
                vendor_basis: basis,
            };
            if let Some(section) = modes.hosts.get(id) {
                apply_override(&mut info, &section.vendor_override);
            }
            info
        })
        .collect()
}

/// 用户覆盖：override 非空（trim 后）时生效，规范化成资源库的英文名后
/// 采用（写入口已校验，手工改过 modes.json 的认不得的写法原样保留，
/// 推荐比较时同样不区分大小写）。
fn apply_override(info: &mut HostInfo, vendor_override: &str) {
    let wanted = vendor_override.trim();
    if wanted.is_empty() {
        return;
    }
    info.vendor = normalize_vendor(wanted);
    info.vendor_source = VENDOR_SOURCE_OVERRIDE.to_string();
    info.vendor_basis = "用户在配置里指定".to_string();
}

/// 单家厂商推定。依据（vendorBasis）是一句事实，如
/// 「端点 open.bigmodel.cn」「pi 默认模型 x/y 未发现」。
/// claude 的进程环境变量由调用方注入（生产传 `std::env::var` 的结果，
/// 测试传固定值，不依赖测试进程的真实环境）。
pub(crate) fn infer_vendor(
    host: &str,
    home: &Path,
    store: &ResourceStore,
    claude_base_url_env: Option<&str>,
) -> (String, String, String) {
    match host {
        "claude" => infer_claude(home, claude_base_url_env),
        "codex" => infer_codex(home),
        "kimi" => infer_kimi(home),
        "pi" => infer_pi(home, store),
        other => match extra_host(other).and_then(|host| host.vendor) {
            Some((vendor, basis)) => (vendor.to_string(), VENDOR_SOURCE_INFERRED.to_string(), basis.to_string()),
            None => (
                VENDOR_SOURCE_UNKNOWN.to_string(),
                VENDOR_SOURCE_UNKNOWN.to_string(),
                "这个软件可以换用不同厂商的模型，请在模式页设定".to_string(),
            ),
        },
    }
}

/// claude：settings.json 的 env.ANTHROPIC_BASE_URL 为空 → Anthropic；
/// 非空按 URL 主机名匹配。进程环境变量同口径（与 inventory.py 的
/// host_vendor_of 一致，值由调用方注入），settings.json 优先。
fn infer_claude(home: &Path, env_base_url: Option<&str>) -> (String, String, String) {
    let from_settings = read_json_field(
        &home.join(".claude").join("settings.json"),
        &["env", "ANTHROPIC_BASE_URL"],
    );
    let base_url = match from_settings {
        Some(url) if !url.trim().is_empty() => url,
        _ => match env_base_url.map(str::trim).filter(|url| !url.is_empty()) {
            Some(url) => url.to_string(),
            None => {
                return (
                    "Anthropic".to_string(),
                    VENDOR_SOURCE_INFERRED.to_string(),
                    "官方端点".to_string(),
                );
            }
        },
    };
    finish_endpoint_match(&base_url, URL_VENDOR_RULES)
}

/// codex：config.toml 没有 model_provider 或为 openai → OpenAI；其它按
/// 名称匹配，匹配不上「未知」。
fn infer_codex(home: &Path) -> (String, String, String) {
    let provider = read_toml_field(&home.join(".codex").join("config.toml"), "model_provider");
    match provider.as_deref().map(str::trim) {
        None | Some("") | Some("openai") => (
            "OpenAI".to_string(),
            VENDOR_SOURCE_INFERRED.to_string(),
            "官方端点".to_string(),
        ),
        Some(name) => finish_provider_match(name),
    }
}

/// kimi：provider 以 managed:kimi-code 开头 → Moonshot；其它按名称匹配。
fn infer_kimi(home: &Path) -> (String, String, String) {
    let provider = kimi_default_provider(&home.join(".kimi-code").join("config.toml"))
        .unwrap_or_default();
    let provider = provider.trim();
    if provider.starts_with("managed:kimi-code") {
        return (
            "Moonshot".to_string(),
            VENDOR_SOURCE_INFERRED.to_string(),
            format!("provider {provider}"),
        );
    }
    if provider.is_empty() {
        return (
            "未知".to_string(),
            VENDOR_SOURCE_UNKNOWN.to_string(),
            "未设 provider".to_string(),
        );
    }
    finish_provider_match(provider)
}

/// pi：默认 provider + 默认模型到资源库里查厂商；找不到或该模型未发现时
/// 「未知」，依据里写明（本机实情：默认 provider 可能已不在 pi 里）。
fn infer_pi(home: &Path, store: &ResourceStore) -> (String, String, String) {
    let settings = home.join(".pi").join("agent").join("settings.json");
    let provider = read_json_field(&settings, &["defaultProvider"]).unwrap_or_default();
    let model = read_json_field(&settings, &["defaultModel"]).unwrap_or_default();
    if provider.trim().is_empty() || model.trim().is_empty() {
        return (
            "未知".to_string(),
            VENDOR_SOURCE_UNKNOWN.to_string(),
            "pi 未设默认模型".to_string(),
        );
    }
    let basis = format!("pi 默认模型 {}/{}", provider.trim(), model.trim());
    let Some(agent) = store.agents.iter().find(|agent| agent.id == "pi") else {
        return (
            "未知".to_string(),
            VENDOR_SOURCE_UNKNOWN.to_string(),
            format!("{basis} 不在资源库"),
        );
    };
    match resources::find_target_model(&agent.models, "pi", provider.trim(), model.trim()) {
        Some(model) if resources::is_missing_from_detection(model) => {
            ("未知".to_string(), VENDOR_SOURCE_UNKNOWN.to_string(), format!("{basis} 未发现"))
        }
        Some(model) if model.vendor.trim().is_empty() => (
            "未知".to_string(),
            VENDOR_SOURCE_UNKNOWN.to_string(),
            format!("{basis} 没有厂商信息"),
        ),
        Some(model) => (
            model.vendor.clone(),
            VENDOR_SOURCE_ENDPOINT.to_string(),
            basis,
        ),
        None => (
            "未知".to_string(),
            VENDOR_SOURCE_UNKNOWN.to_string(),
            format!("{basis} 不在资源库"),
        ),
    }
}

/// 端点匹配的公共收尾：解析出 URL 的主机名，按规则表（小写比对）给厂商
/// （来源「端点」），解析失败或匹配不上「未知」。依据写成
/// 「端点 <主机名或原文>」，厂商认不出时补一句「厂商未识别」。
fn finish_endpoint_match(value: &str, rules: &[(&str, &str)]) -> (String, String, String) {
    let hostname = url_hostname(value);
    let matched = match &hostname {
        Some(hostname) => rules
            .iter()
            .find(|(keyword, _)| host_matches_rule(hostname, keyword)),
        // 不是能解析的 URL（如只写了主机名）：按原文小写包含比对，口径与
        // provider 匹配一致。
        None => rules
            .iter()
            .find(|(keyword, _)| value.to_lowercase().contains(keyword)),
    };
    let basis = format!("端点 {}", hostname.unwrap_or_else(|| value.to_string()));
    match matched {
        Some((_, vendor)) => (
            vendor.to_string(),
            VENDOR_SOURCE_ENDPOINT.to_string(),
            basis,
        ),
        None => (
            "未知".to_string(),
            VENDOR_SOURCE_UNKNOWN.to_string(),
            format!("{basis} · 厂商未识别"),
        ),
    }
}

/// 从 URL 里取主机名（小写）：去掉 scheme、路径、端口与 userinfo。
/// 不是 `scheme://…` 形状的输入返回 None，由调用方回退包含比对。
fn url_hostname(value: &str) -> Option<String> {
    let (scheme, rest) = value.split_once("://")?;
    if scheme.is_empty() || rest.is_empty() {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?.split(':').next()?;
    let trimmed = host.trim().to_lowercase();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// 主机名是否命中规则关键字：带点的关键字按标签后缀比对
/// （`api.z.ai` 命中 `z.ai`，`buzz.ai` 不命中）；单个标签的关键字
/// 只要主机名里有这个标签就算命中（`api.moonshot.ai` 命中 `moonshot`）。
fn host_matches_rule(hostname: &str, keyword: &str) -> bool {
    let key_labels: Vec<&str> = keyword.split('.').filter(|label| !label.is_empty()).collect();
    let host_labels: Vec<&str> = hostname.split('.').filter(|label| !label.is_empty()).collect();
    if key_labels.len() > 1 {
        return host_labels.len() >= key_labels.len()
            && host_labels[host_labels.len() - key_labels.len()..] == key_labels[..];
    }
    let Some(keyword) = key_labels.first() else {
        return false;
    };
    host_labels.iter().any(|label| *label == *keyword)
}

fn finish_provider_match(name: &str) -> (String, String, String) {
    let lowered = name.to_lowercase();
    for (keyword, vendor) in PROVIDER_VENDOR_RULES {
        if lowered.contains(keyword) {
            return (
                vendor.to_string(),
                VENDOR_SOURCE_ENDPOINT.to_string(),
                format!("端点 {name}"),
            );
        }
    }
    (
        "未知".to_string(),
        VENDOR_SOURCE_UNKNOWN.to_string(),
        format!("端点 {name} · 厂商未识别"),
    )
}

/// 读 JSON 里按路径给的一个字符串字段；文件不存在、损坏或类型不对都算没有。
fn read_json_field(path: &Path, field_path: &[&str]) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let mut current = &value;
    for key in field_path {
        current = current.get(key)?;
    }
    current.as_str().map(str::to_string)
}

/// 读 TOML 顶层的一个字符串字段；文件不存在、损坏或类型不对都算没有。
/// Kimi Code 的 provider 写在默认模型那一节：`default_model = "x"` →
/// `[models."x"] provider = "…"`；旧写法顶层 `provider` 作兜底。
fn kimi_default_provider(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: toml::Value = toml::from_str(&text).ok()?;
    let from_model = value
        .get("default_model")
        .and_then(|model| model.as_str())
        .and_then(|model| value.get("models")?.get(model)?.get("provider")?.as_str());
    from_model
        .or_else(|| value.get("provider")?.as_str())
        .map(str::to_string)
}

fn read_toml_field(path: &Path, field: &str) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: toml::Value = toml::from_str(&text).ok()?;
    value.get(field)?.as_str().map(str::to_string)
}

/// `config set-host-vendor <host> <vendor|auto>`：设定或清除厂商覆盖，
/// 写入后返回该主 agent 的清单条目。
pub(crate) fn set_vendor_override_at(
    root: &Path,
    host: &str,
    vendor: &str,
) -> Result<HostInfo, (i32, String)> {
    let home = crate::paths::home().map_err(|message| (3, message))?;
    let env_base_url = std::env::var("ANTHROPIC_BASE_URL").ok();
    set_vendor_override_at_home(root, &home, host, vendor, env_base_url.as_deref())
}

/// 同上，家目录注入（测试用临时目录）。host 与厂商写法都不区分大小写：
/// host 先转小写再比对；厂商只认规范化后的已知厂商名或 auto，其它值退出
/// 2（窗口下拉传的就是这些英文名）。
pub(crate) fn set_vendor_override_at_home(
    root: &Path,
    home: &Path,
    host: &str,
    vendor: &str,
    claude_base_url_env: Option<&str>,
) -> Result<HostInfo, (i32, String)> {
    let store = resources::get_resources_at(&root.join("resources.json"))
        .map_err(|message| (3, message))?;
    let host = host.trim().to_lowercase();
    let detected = detected_host_ids(&store);
    if !detected.iter().any(|id| id.eq_ignore_ascii_case(&host)) {
        return Err((
            2,
            format!(
                "未知 host：{host}；检测到：{}",
                if detected.is_empty() {
                    "（无）".to_string()
                } else {
                    detected.join("、")
                }
            ),
        ));
    }
    let override_value = if vendor.eq_ignore_ascii_case("auto") {
        String::new()
    } else if is_known_vendor(vendor) {
        normalize_vendor(&vendor)
    } else {
        return Err((
            2,
            format!(
                "不认识的厂商：{vendor}；可选：auto 或 {}",
                KNOWN_VENDORS.join("、")
            ),
        ));
    };
    {
        // 与 modes.json 其它写命令同一把锁，覆盖「严格读 → 改 → 写盘」。
        let _guard = crate::console_cli::modes_write_lock(root)?;
        let mut modes = crate::orchestration::modes_from_strict(root).map_err(|message| {
            (
                1,
                format!("{message}；已停止写入，修复或删除该文件后重试"),
            )
        })?;
        crate::orchestration::host_section_mut(&mut modes, &host).vendor_override = override_value;
        crate::orchestration::save_modes_in(root, &modes).map_err(|message| (1, message))?;
    }
    let modes = crate::orchestration::modes_from(root);
    collect_hosts(&store, &modes, home, claude_base_url_env)
        .into_iter()
        .find(|info| info.id == host)
        .ok_or_else(|| (1, format!("找不到主 agent：{host}")))
}

// ── --host 解析（TASK-50 第 2 节） ────────────────────────────────────────

/// --host 的解析结果。
pub(crate) enum HostResolve {
    /// 没给 --host 且 modes.json 里没有主 agent 配置：按无配置处理
    /// （派发走原有 roles.json 行为，展示按空配置）。
    None,
    /// 解析出的主 agent id。
    Detected(String),
    /// 没给 --host 且配置了多于一个主 agent：需要 --host。
    NeedHost(Vec<String>),
    /// 给了 --host 但不是检测到的 agent。
    Unknown(String, Vec<String>),
    /// 没给 --host，modes.json 里唯一的配置是推定出的主 agent，但它已
    /// 不被检测到（fix-50-r2：与给了非法 --host 的文案分开）。
    ConfiguredNotDetected(String, Vec<String>),
}

/// --host 规则（所有读写配置的命令与派发共用）：给了就用（必须是检测到
/// 的 agent，不区分大小写）；没给时 modes.json 里只有一个主 agent 的配置
/// 就用它，但它在 detected 非空时仍要校验是否仍被检测到（推定出的主
/// agent 停用后不再自动用），一个都没有则按无配置处理，多于一个则要
/// --host。detected 传空切片表示资源库读不了、无法校验（fix-50-r1：
/// 没给 --host 时不为校验读资源库）。
pub(crate) fn resolve_host(
    given: Option<&str>,
    modes: &ModesFile,
    detected: &[String],
) -> HostResolve {
    let Some(host) = given.map(str::trim).filter(|host| !host.is_empty()) else {
        let configured: Vec<String> = modes.hosts.keys().cloned().collect();
        return match configured.len() {
            0 => HostResolve::None,
            1 => {
                let host = configured[0].clone();
                if !detected.is_empty() && !detected.iter().any(|id| id == &host) {
                    HostResolve::ConfiguredNotDetected(host, detected.to_vec())
                } else {
                    HostResolve::Detected(host)
                }
            }
            _ => HostResolve::NeedHost(configured),
        };
    };
    let lowered = host.to_lowercase();
    if detected.iter().any(|id| id.eq_ignore_ascii_case(&lowered)) {
        HostResolve::Detected(lowered)
    } else {
        HostResolve::Unknown(host.to_string(), detected.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::HostModes;
    use crate::resources::ResourceAgent;

    fn store_with(detected: &[&str]) -> ResourceStore {
        ResourceStore {
            extra_hosts: Vec::new(),
            version: 1,
            agents: HOST_IDS
                .iter()
                .map(|id| ResourceAgent {
                    id: id.to_string(),
                    name: host_display_name(id).to_string(),
                    kind: "内置".to_string(),
                    detected: detected.contains(id),
                    ..ResourceAgent::default()
                })
                .collect(),
            detected_at: None,
        }
    }

    fn modes_with_hosts(hosts: &[&str]) -> ModesFile {
        ModesFile {
            version: crate::orchestration::MODES_VERSION,
            hosts: hosts
                .iter()
                .map(|id| (id.to_string(), HostModes::default()))
                .collect(),
        }
    }

    fn detected_list(store: &ResourceStore) -> Vec<String> {
        detected_host_ids(store)
    }

    #[test]
    fn resolve_host_follows_given_single_configured_and_need_host_rules() {
        let store = store_with(&["claude", "codex"]);
        let detected = detected_list(&store);
        // 给了就用，大小写不敏感。
        assert!(matches!(
            resolve_host(Some(" Claude "), &modes_with_hosts(&[]), &detected),
            HostResolve::Detected(ref id) if id == "claude"
        ));
        // 给了但不是检测到的 agent。
        assert!(matches!(
            resolve_host(Some("kimi"), &modes_with_hosts(&[]), &detected),
            HostResolve::Unknown(_, _)
        ));
        // 没给：零配置 → None；一个 → 用它；多于一个 → 要 --host。
        assert!(matches!(
            resolve_host(None, &modes_with_hosts(&[]), &detected),
            HostResolve::None
        ));
        assert!(matches!(
            resolve_host(None, &modes_with_hosts(&["codex"]), &detected),
            HostResolve::Detected(ref id) if id == "codex"
        ));
        assert!(matches!(
            resolve_host(None, &modes_with_hosts(&["claude", "codex"]), &detected),
            HostResolve::NeedHost(_)
        ));
        // 推定出的主 agent 已不在检测列表：不再自动用，且与给了非法
        // --host 的情形分开报（fix-50-r2）。
        assert!(matches!(
            resolve_host(None, &modes_with_hosts(&["kimi"]), &detected),
            HostResolve::ConfiguredNotDetected(host, _) if host == "kimi"
        ));
        // detected 为空表示资源库读不了、无法校验：单一配置照样用。
        assert!(matches!(
            resolve_host(None, &modes_with_hosts(&["kimi"]), &[]),
            HostResolve::Detected(ref id) if id == "kimi"
        ));
    }

    #[test]
    fn detected_host_ids_keeps_fixed_order_and_skips_undetected() {
        let store = store_with(&["pi", "claude"]);
        assert_eq!(detected_host_ids(&store), vec!["claude", "pi"]);
    }

    #[test]
    fn claude_vendor_infers_from_settings_env_and_injected_env() {
        let home = crate::paths::test_temp_dir().join("hosts-claude");
        // 没有配置文件、没注入环境变量 → Anthropic 推定。
        let (vendor, source, basis) = infer_claude(&home, None);
        assert_eq!((vendor.as_str(), source.as_str()), ("Anthropic", VENDOR_SOURCE_INFERRED));
        assert_eq!(basis, "官方端点");

        // settings.json 指向智谱端点 → Zhipu（统一用资源库的英文厂商名）。
        let settings = home.join(".claude").join("settings.json");
        std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
        std::fs::write(
            &settings,
            r#"{"env":{"ANTHROPIC_BASE_URL":"https://open.bigmodel.cn/api/anthropic"}}"#,
        )
        .unwrap();
        let (vendor, source, basis) = infer_claude(&home, None);
        assert_eq!((vendor.as_str(), source.as_str()), ("Zhipu", VENDOR_SOURCE_ENDPOINT));
        assert!(basis.contains("open.bigmodel.cn"), "{basis}");

        // settings.json 未设时用注入的进程环境变量；settings.json 优先。
        std::fs::remove_file(&settings).unwrap();
        let (vendor, source, basis) = infer_claude(&home, Some("https://api.moonshot.ai/anthropic"));
        assert_eq!((vendor.as_str(), source.as_str()), ("Moonshot", VENDOR_SOURCE_ENDPOINT));
        assert_eq!(basis, "端点 api.moonshot.ai");
        std::fs::write(
            &settings,
            r#"{"env":{"ANTHROPIC_BASE_URL":"https://api.z.ai/v1"}}"#,
        )
        .unwrap();
        let (vendor, _, _) = infer_claude(&home, Some("https://api.moonshot.ai/anthropic"));
        assert_eq!(vendor, "Zhipu", "settings.json 优先于进程环境变量");

        // 认不出的主机 → 未知；官方域名 → Anthropic。
        std::fs::write(
            &settings,
            r#"{"env":{"ANTHROPIC_BASE_URL":"https://relay.example.example/v1"}}"#,
        )
        .unwrap();
        let (vendor, source, _) = infer_claude(&home, None);
        assert_eq!((vendor.as_str(), source.as_str()), ("未知", VENDOR_SOURCE_UNKNOWN));
        std::fs::write(
            &settings,
            r#"{"env":{"ANTHROPIC_BASE_URL":"https://api.anthropic.com/v1"}}"#,
        )
        .unwrap();
        let (vendor, source, _) = infer_claude(&home, None);
        assert_eq!((vendor.as_str(), source.as_str()), ("Anthropic", VENDOR_SOURCE_ENDPOINT));
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn codex_and_kimi_vendors_follow_provider_names() {
        let home = crate::paths::test_temp_dir().join("hosts-codex-kimi");
        // 没有配置文件 → codex 推定 OpenAI，kimi 未知。
        let (vendor, source, _) = infer_codex(&home);
        assert_eq!((vendor.as_str(), source.as_str()), ("OpenAI", VENDOR_SOURCE_INFERRED));
        let (vendor, source, _) = infer_kimi(&home);
        assert_eq!((vendor.as_str(), source.as_str()), ("未知", VENDOR_SOURCE_UNKNOWN));

        let codex_config = home.join(".codex").join("config.toml");
        std::fs::create_dir_all(codex_config.parent().unwrap()).unwrap();
        std::fs::write(&codex_config, "model_provider = \"zai-coding\"\n").unwrap();
        let (vendor, source, basis) = infer_codex(&home);
        assert_eq!((vendor.as_str(), source.as_str()), ("Zhipu", VENDOR_SOURCE_ENDPOINT));
        assert!(basis.contains("zai-coding"), "{basis}");

        let kimi_config = home.join(".kimi-code").join("config.toml");
        std::fs::create_dir_all(kimi_config.parent().unwrap()).unwrap();
        std::fs::write(&kimi_config, "provider = \"managed:kimi-code/subscription\"\n").unwrap();
        let (vendor, source, basis) = infer_kimi(&home);
        assert_eq!((vendor.as_str(), source.as_str()), ("Moonshot", VENDOR_SOURCE_INFERRED));
        assert!(basis.contains("managed:kimi-code"), "{basis}");

        // 真机写法：provider 在默认模型那一节，顶层没有 provider。
        std::fs::write(
            &kimi_config,
            "default_model = \"kimi-code/k3\"\n\n[providers.\"managed:kimi-code\"]\n\n[models.\"kimi-code/k3\"]\nprovider = \"managed:kimi-code\"\nmodel = \"k3\"\n",
        )
        .unwrap();
        let (vendor, source, _) = infer_kimi(&home);
        assert_eq!((vendor.as_str(), source.as_str()), ("Moonshot", VENDOR_SOURCE_INFERRED));
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn pi_vendor_reads_resource_store_and_reports_missing() {
        let home = crate::paths::test_temp_dir().join("hosts-pi");
        let settings = home.join(".pi").join("agent").join("settings.json");
        std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
        std::fs::write(
            &settings,
            r#"{"defaultProvider":"qwen-token-plan-cn","defaultModel":"qwen3.8-max"}"#,
        )
        .unwrap();
        // 资源库里没有这条默认模型 → 未知，依据写明。
        let store = store_with(&["pi"]);
        let (vendor, source, basis) = infer_pi(&home, &store);
        assert_eq!((vendor.as_str(), source.as_str()), ("未知", VENDOR_SOURCE_UNKNOWN));
        assert_eq!(basis, "pi 默认模型 qwen-token-plan-cn/qwen3.8-max 不在资源库");

        // 资源库里有且带厂商 → 端点；带未发现标记 → 未知。
        let mut store = store_with(&["pi"]);
        let vendor_value = {
            let agent = store.agents.iter_mut().find(|a| a.id == "pi").unwrap();
            agent.models.push(crate::resources::ResourceModel {
                id: "qwen3.8-max".into(),
                agent: "pi".into(),
                provider: "qwen-token-plan-cn".into(),
                vendor: "Alibaba".into(),
                ..crate::resources::ResourceModel::default()
            });
            agent.models[0].vendor.clone()
        };
        assert_eq!(vendor_value, "Alibaba");
        let (vendor, source, _) = infer_pi(&home, &store);
        assert_eq!((vendor.as_str(), source.as_str()), ("Alibaba", VENDOR_SOURCE_ENDPOINT));
        {
            let agent = store.agents.iter_mut().find(|a| a.id == "pi").unwrap();
            agent.models[0].meta = format!("{}；其它", resources::MISSING_MARKER);
        }
        let (vendor, source, basis) = infer_pi(&home, &store);
        assert_eq!((vendor.as_str(), source.as_str()), ("未知", VENDOR_SOURCE_UNKNOWN));
        assert_eq!(basis, "pi 默认模型 qwen-token-plan-cn/qwen3.8-max 未发现");
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn vendor_override_wins_and_auto_clears() {
        let store = store_with(&["claude"]);
        let home = crate::paths::test_temp_dir().join("hosts-override");
        let mut modes = ModesFile::default();
        modes.hosts.insert(
            "claude".to_string(),
            HostModes {
                vendor_override: "DeepSeek".into(),
                ..HostModes::default()
            },
        );
        let hosts = collect_hosts(&store, &modes, &home, None);
        assert_eq!(hosts.len(), 1);
        assert_eq!((hosts[0].vendor.as_str(), hosts[0].vendor_source.as_str()), ("DeepSeek", VENDOR_SOURCE_OVERRIDE));
        // 中文旧写法与大小写变体规范化成英文名。
        modes.hosts.get_mut("claude").unwrap().vendor_override = "智谱".into();
        let hosts = collect_hosts(&store, &modes, &home, None);
        assert_eq!(hosts[0].vendor, "Zhipu");
        modes.hosts.get_mut("claude").unwrap().vendor_override = " deepseek ".into();
        let hosts = collect_hosts(&store, &modes, &home, None);
        assert_eq!(hosts[0].vendor, "DeepSeek");
        // 空覆盖不生效。
        modes.hosts.get_mut("claude").unwrap().vendor_override = String::new();
        let hosts = collect_hosts(&store, &modes, &home, None);
        assert_eq!(hosts[0].vendor, "Anthropic");
        assert_eq!(hosts[0].vendor_source, VENDOR_SOURCE_INFERRED);
        let _ = std::fs::remove_dir_all(home);
    }

    /// 厂商规范化表：中文旧写法、别名、大小写变体都归到资源库的英文名；
    /// 认不得的原样返回。
    #[test]
    fn normalize_vendor_unifies_writings() {
        assert_eq!(normalize_vendor("智谱"), "Zhipu");
        assert_eq!(normalize_vendor("月之暗面"), "Moonshot");
        assert_eq!(normalize_vendor("Moonshot AI"), "Moonshot");
        assert_eq!(normalize_vendor("Zhipu AI"), "Zhipu");
        assert_eq!(normalize_vendor("阿里"), "Alibaba");
        assert_eq!(normalize_vendor("阿里云"), "Alibaba");
        assert_eq!(normalize_vendor("deepseek"), "DeepSeek");
        assert_eq!(normalize_vendor(" ANTHROPIC "), "Anthropic");
        assert_eq!(normalize_vendor("kimi"), "Moonshot");
        assert_eq!(normalize_vendor("dashscope"), "Alibaba");
        assert_eq!(normalize_vendor("神秘厂商"), "神秘厂商");
        assert!(is_known_vendor("Zhipu"));
        assert!(is_known_vendor("zhipu"));
        assert!(!is_known_vendor("神秘厂商"));
    }

    #[test]
    fn collect_hosts_lists_only_detected_with_names() {
        let store = store_with(&["pi", "kimi"]);
        let modes = ModesFile::default();
        let home = crate::paths::test_temp_dir().join("hosts-list");
        let hosts = collect_hosts(&store, &modes, &home, None);
        assert_eq!(
            hosts.iter().map(|host| host.id.as_str()).collect::<Vec<_>>(),
            vec!["kimi", "pi"]
        );
        assert_eq!(hosts[0].name, "Kimi Code");
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn extra_hosts_detected_by_config_dir_or_command() {
        let home = crate::paths::test_temp_dir().join(format!("extra-hosts-{}", uuid::Uuid::new_v4()));
        let bin = home.join("bin");
        std::fs::create_dir_all(home.join(".config/opencode")).unwrap();
        std::fs::create_dir_all(home.join(".gemini/antigravity-cli")).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        let hermes = if cfg!(windows) { "hermes.cmd" } else { "hermes" };
        std::fs::write(bin.join(hermes), "").unwrap();
        // 只有 ~/.gemini（Antigravity 也用它）不算装了 Gemini CLI。
        std::fs::create_dir_all(home.join(".gemini")).unwrap();
        let path_var = bin.to_string_lossy().into_owned();
        assert_eq!(detect_extra_hosts(&home, Some(&path_var)), vec!["opencode", "agy", "hermes"]);
        assert_eq!(detect_extra_hosts(&home, None), vec!["opencode", "agy"]);

        // 记进资源库后进主 agent 清单，排在四家之后；Gemini CLI 以外厂商未知。
        let mut store = store_with(&["claude"]);
        store.extra_hosts = vec!["hermes".into(), "gemini".into()];
        assert_eq!(detected_host_ids(&store), vec!["claude", "gemini", "hermes"]);
        let hosts = collect_hosts(&store, &ModesFile::default(), &home, None);
        assert_eq!(hosts[1].name, "Gemini CLI");
        assert_eq!(hosts[1].vendor, "Google");
        assert_eq!(hosts[2].vendor, VENDOR_SOURCE_UNKNOWN);
        let _ = std::fs::remove_dir_all(home);
    }
}
