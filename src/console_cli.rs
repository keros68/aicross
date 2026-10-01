//! `aicross dispatch/status/adopt/cancel/resources/config` 命令行入口。
//! `resources` 与 `config`（TASK-39）直读资源库与配置文件，不经过窗口。

use crate::orchestration::RoleProfile;
use crate::resources::{
    self, ResourceAgent, ResourceModelKey, ResourceOperation, ResourceStore,
};
use serde_json::{json, Map, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub fn run_if_subcommand(args: &[String]) -> Option<i32> {
    let command = args.get(1).map(String::as_str);
    if !matches!(
        command,
        Some("dispatch" | "status" | "adopt" | "cancel" | "resources" | "config" | "__run")
    ) {
        return None;
    }
    // R2-5：面向用户的子命令把输出挂回宿主终端；后台引擎（__run）是独立进程，
    // 不挂接，宿主终端关闭时不被波及。
    if attaches_parent_console(command) {
        attach_parent_console();
    }
    let root = crate::paths::aicross_root();
    Some(dispatch_root(args, root, &mut std::io::stdout(), &mut std::io::stderr()))
}

/// 是否挂接父进程控制台：面向用户的子命令要，后台 `__run` 不要。
fn attaches_parent_console(command: Option<&str>) -> bool {
    command.is_some_and(|command| command != "__run")
}

/// 引擎根目录不可确定（例如没有用户目录）时所有子命令一律退出码 3。
fn dispatch_root(
    args: &[String],
    root: Result<std::path::PathBuf, String>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    match root {
        Ok(root) => run_at(args, &root, stdout, stderr),
        Err(_) => {
            let _ = writeln!(stderr, "引擎不可用");
            3
        }
    }
}

fn run_at(args: &[String], root: &Path, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    run_at_with(args, root, &ResourcesOps::production(), stdout, stderr)
}

fn run_at_with(
    args: &[String],
    root: &Path,
    ops: &ResourcesOps,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    if args.get(1).map(String::as_str) == Some("__run") {
        let Some(run_id) = args.get(2) else { return 2 };
        if !crate::engine::valid_id(run_id) {
            let _ = writeln!(stderr, "run_id 无效");
            return 2;
        }
        return match crate::engine::run_background(root, run_id) {
            Ok(()) => 0,
            Err(message) => { let _ = writeln!(stderr, "{message}"); 1 }
        };
    }
    match args.get(1).map(String::as_str) {
        Some("resources") => return run_resources(root, args, ops, stdout, stderr),
        Some("config") => return run_config(root, args, stdout, stderr),
        _ => {}
    }
    let request = match build_request(args) {
        Ok(value) => value,
        Err(message) => {
            let _ = writeln!(stderr, "{message}");
            return 2;
        }
    };
    match args.get(1).map(String::as_str) {
        Some("dispatch") => {
            let input = match serde_json::from_value(request.body.unwrap_or_default()) {
                Ok(value) => value,
                Err(error) => { let _ = writeln!(stderr, "派发请求无效：{error}"); return 2; }
            };
            return match crate::engine::start_dispatch(root, input) {
                Ok(response) => {
                    let accepted = response.get("accepted").and_then(Value::as_bool) == Some(true);
                    let _ = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap_or_else(|_| "{}".into()));
                    let _ = writeln!(stderr, "{}", if accepted { "引擎已接收派发" } else { "派发被拒绝" });
                    if accepted { 0 } else { 1 }
                }
                Err(crate::engine::StartError::Unavailable(message)) => { let _ = writeln!(stderr, "{message}"); 3 }
            };
        }
        Some("status") => return run_status(root, args, request.timeout, stdout, stderr),
        Some("adopt") => {
            let Some(run_id) = positional(args, 2) else { return 2 };
            let Some(node_id) = positional(args, 3) else { return 2 };
            return match crate::engine::adopt(root, &run_id, &node_id) { Ok(run) => output_run(run, stdout, stderr), Err(message) => { let _=writeln!(stderr,"{message}"); 1 } };
        }
        Some("cancel") => {
            let Some(run_id) = positional(args, 2) else { return 2 };
            return match crate::engine::cancel(root, &run_id) { Ok(run) => output_run(run, stdout, stderr), Err(message) => { let _=writeln!(stderr,"{message}"); 1 } };
        }
        _ => {}
    }
    let _ = writeln!(stderr, "未知命令");
    2
}

fn output_run(run: crate::engine::EngineRun, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let _ = writeln!(stdout, "{}", serde_json::to_string(&run).unwrap_or_else(|_| "{}".into()));
    let _ = writeln!(stderr, "引擎请求完成");
    0
}

fn run_status(root: &Path, args: &[String], timeout: Duration, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let Some(run_id) = positional(args, 2) else { return 2 };
    let wait = args.iter().any(|arg| arg == "--wait");
    let started = Instant::now();
    loop {
        let run = match crate::engine::read_run(root, &run_id) { Ok(run) => run, Err(message) => { let _=writeln!(stderr,"{message}"); return 1 } };
        if !wait || run.status != "running" { return output_run(run, stdout, stderr); }
        if started.elapsed() >= timeout { let _=writeln!(stderr,"等待状态超时"); return 1; }
        std::thread::sleep(Duration::from_millis(500));
    }
}

// ── TASK-39：resources / config 子命令 ────────────────────────────────

/// resources 子命令的进程边界：生产走 resources.rs 的现成函数（与接入页同一
/// 套），测试注入假实现，不在单测里真探测、真发模型请求。
struct ResourcesOps {
    read_resources: Box<dyn Fn(&Path) -> Result<ResourceStore, String>>,
    detect_all: Box<dyn Fn(&Path, Option<&str>) -> Result<ResourceOperation, String>>,
    smoke_agent_third_party: Box<dyn Fn(&Path, &str) -> Result<ResourceOperation, String>>,
    smoke_one: Box<dyn Fn(&Path, ResourceModelKey) -> Result<ResourceStore, String>>,
}

impl ResourcesOps {
    fn production() -> Self {
        ResourcesOps {
            read_resources: Box::new(resources::get_resources_at),
            detect_all: Box::new(resources::detect_all_resources_at),
            smoke_agent_third_party: Box::new(resources::smoke_agent_third_party_at),
            smoke_one: Box::new(resources::smoke_resource_model_at),
        }
    }
}

fn store_file(root: &Path) -> PathBuf {
    root.join("resources.json")
}

fn run_resources(
    root: &Path,
    args: &[String],
    ops: &ResourcesOps,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    match args.get(2).map(String::as_str) {
        None => {
            let store = match (ops.read_resources)(&store_file(root)) {
                Ok(store) => store,
                Err(message) => {
                    let _ = writeln!(stderr, "{message}");
                    return 3;
                }
            };
            output_resources(&store, None, stdout);
            let _ = writeln!(stderr, "已读取资源库");
            0
        }
        Some("detect") => {
            if let Some(message) = extra_arg_error(args, 3) {
                let _ = writeln!(stderr, "{message}");
                return 2;
            }
            // cc-switch 发现提示的路径与接入页同源：派发设置里的 ccSwitchPath。
            let cc_switch = crate::engine::load_settings(root).cc_switch_path;
            let cc_switch = (!cc_switch.trim().is_empty()).then_some(cc_switch);
            let operation = match (ops.detect_all)(&store_file(root), cc_switch.as_deref()) {
                Ok(operation) => operation,
                Err(message) => {
                    let _ = writeln!(stderr, "{message}");
                    return 3;
                }
            };
            // 单个 agent 探不到或事实源读不了是探测结果，不是命令失败；逐条写 stderr。
            for error in &operation.errors {
                let _ = writeln!(stderr, "{error}");
            }
            output_resources(&operation.store, None, stdout);
            let _ = writeln!(stderr, "已探测 {} 个 agent", operation.store.agents.len());
            0
        }
        Some("smoke") => run_resources_smoke(root, args, ops, stdout, stderr),
        Some(other) => {
            let _ = writeln!(stderr, "未知子命令：{other}");
            2
        }
    }
}

fn run_resources_smoke(
    root: &Path,
    args: &[String],
    ops: &ResourcesOps,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    // args: aicross resources smoke <agent> [--model …] [--provider …]
    let Some(agent_arg) = positional(args, 3) else {
        let _ = writeln!(stderr, "缺少 agent");
        return 2;
    };
    if let Some(message) = unknown_option_error(args, 4, &["--model", "--provider"]) {
        let _ = writeln!(stderr, "{message}");
        return 2;
    }
    let store = match (ops.read_resources)(&store_file(root)) {
        Ok(store) => store,
        Err(message) => {
            let _ = writeln!(stderr, "{message}");
            return 3;
        }
    };
    let Some(agent) = store
        .agents
        .iter()
        .find(|agent| agent.id.eq_ignore_ascii_case(agent_arg.trim()))
    else {
        let ids: Vec<&str> = store.agents.iter().map(|agent| agent.id.as_str()).collect();
        let _ = writeln!(
            stderr,
            "未知 agent：{agent_arg}；可选：{}",
            join_or_none(&ids)
        );
        return 2;
    };
    let agent_id = agent.id.clone();
    match option_value(args, "--model") {
        Some(model_id) => {
            let provider = option_value(args, "--provider");
            let candidates: Vec<_> = agent
                .models
                .iter()
                .filter(|model| model.id == model_id)
                .filter(|model| provider.as_deref().map_or(true, |value| model.provider == value))
                .collect();
            let target = match candidates.as_slice() {
                [] => {
                    let _ = writeln!(
                        stderr,
                        "模型不存在：{agent_id}/{model_id}；该 agent 下可选：{}",
                        model_candidates(agent)
                    );
                    return 2;
                }
                [one] => (*one).clone(),
                _ => {
                    let providers: Vec<&str> =
                        candidates.iter().map(|model| model.provider.as_str()).collect();
                    let _ = writeln!(
                        stderr,
                        "该模型挂在多个 provider 下：{}；用 --provider 指定",
                        providers.join("、")
                    );
                    return 2;
                }
            };
            if resources::is_native_model(&target.agent) {
                let _ = writeln!(stderr, "原生模型无需冒烟");
                output_resources(&store, Some(&agent_id), stdout);
                return 0;
            }
            let key = ResourceModelKey {
                agent_id: agent_id.clone(),
                provider: target.provider.clone(),
                id: target.id.clone(),
            };
            match (ops.smoke_one)(&store_file(root), key) {
                Ok(smoked) => {
                    output_resources(&smoked, Some(&agent_id), stdout);
                    let failure = smoked
                        .agents
                        .iter()
                        .find(|agent| agent.id == agent_id)
                        .and_then(|agent| {
                            agent.models.iter().find(|model| {
                                model.id == target.id && model.provider == target.provider
                            })
                        })
                        .filter(|model| model.verification == resources::VERIFICATION_FAILED)
                        .map(|model| model.smoke_failure.clone());
                    match failure {
                        Some(reason) => {
                            let _ = writeln!(stderr, "冒烟失败：{reason}");
                            1
                        }
                        None => {
                            let _ = writeln!(stderr, "冒烟完成");
                            0
                        }
                    }
                }
                Err(message) => {
                    let _ = writeln!(stderr, "{message}");
                    3
                }
            }
        }
        None => {
            if option_value(args, "--provider").is_some() {
                let _ = writeln!(stderr, "--provider 需要 --model");
                return 2;
            }
            // R1-7：agent 下一个模型都没有时不发请求，退出 0。
            if agent.models.is_empty() {
                output_resources(&store, Some(&agent_id), stdout);
                let _ = writeln!(stderr, "该 agent 下没有模型");
                return 0;
            }
            let operation = match (ops.smoke_agent_third_party)(&store_file(root), &agent_id) {
                Ok(operation) => operation,
                Err(message) => {
                    let _ = writeln!(stderr, "{message}");
                    return 3;
                }
            };
            for error in &operation.errors {
                let _ = writeln!(stderr, "{error}");
            }
            let fresh = match (ops.read_resources)(&store_file(root)) {
                Ok(store) => store,
                Err(message) => {
                    let _ = writeln!(stderr, "{message}");
                    return 3;
                }
            };
            output_resources(&fresh, Some(&agent_id), stdout);
            let third_party = fresh
                .agents
                .iter()
                .find(|agent| agent.id == agent_id)
                .map(|agent| {
                    agent
                        .models
                        .iter()
                        .filter(|model| !resources::is_native_model(&model.agent))
                        .count()
                })
                .unwrap_or(0);
            if third_party == 0 {
                let _ = writeln!(stderr, "原生模型无需冒烟");
                return 0;
            }
            if operation.errors.is_empty() {
                let _ = writeln!(stderr, "冒烟完成");
                0
            } else {
                let _ = writeln!(stderr, "冒烟完成；{} 条失败", operation.errors.len());
                1
            }
        }
    }
}

const ROLE_KEYS: [&str; 3] = ["planning", "execution", "acceptance"];
const THINKING_LEVELS: [&str; 4] = ["关", "低", "中", "高"];

#[derive(Clone, Copy, PartialEq)]
enum SettingKind {
    Number,
    Bool,
    Text,
}

/// 派发设置的键（与 dispatch-settings.json 字段同名）及其取值类型。
fn setting_kind(key: &str) -> Option<SettingKind> {
    Some(match key {
        "timeoutSecs" | "concurrency" | "maxRequests" | "fixRoundLimit" | "swarmLimit"
        | "consoleRunLimit" => SettingKind::Number,
        "redact" | "writeProjectDispatch" | "acpEnabled" | "classifyProposals" => {
            SettingKind::Bool
        }
        "validationCommand" | "ccSwitchPath" | "classificationModel" => SettingKind::Text,
        _ => return None,
    })
}

fn run_config(root: &Path, args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    match args.get(2).map(String::as_str) {
        Some("show") => {
            if let Some(message) = extra_arg_error(args, 3) {
                let _ = writeln!(stderr, "{message}");
                return 2;
            }
            // R1-2：roles.json 损坏时 stdout 仍输出默认值，stderr 提示一行。
            let roles = match crate::orchestration::roles_from_strict(root) {
                Ok(roles) => roles,
                Err(_) => {
                    let _ = writeln!(stderr, "roles.json 无法解析，已按默认值显示");
                    crate::orchestration::roles_from(root)
                }
            };
            let value = json!({
                "roles": roles,
                "dispatch": crate::engine::load_settings(root),
            });
            let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
            let _ = writeln!(stderr, "已读取配置");
            0
        }
        Some("set-role") => run_config_set_role(root, args, stdout, stderr),
        Some("set") => run_config_set(root, args, stdout, stderr),
        None => {
            let _ = writeln!(stderr, "缺少子命令");
            2
        }
        Some(other) => {
            let _ = writeln!(stderr, "未知子命令：{other}");
            2
        }
    }
}

fn run_config_set_role(
    root: &Path,
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let Some(role) = positional(args, 3) else {
        let _ = writeln!(stderr, "缺少角色（planning|execution|acceptance）");
        return 2;
    };
    if !ROLE_KEYS.contains(&role.as_str()) {
        let _ = writeln!(stderr, "角色只认 planning|execution|acceptance：{role}");
        return 2;
    }
    if let Some(message) = unknown_option_error(
        args,
        4,
        &["--agent", "--model", "--provider", "--tier", "--thinking"],
    ) {
        let _ = writeln!(stderr, "{message}");
        return 2;
    }
    let Some(agent_arg) = option_value(args, "--agent") else {
        let _ = writeln!(stderr, "缺少 --agent");
        return 2;
    };
    let Some(model_arg) = option_value(args, "--model") else {
        let _ = writeln!(stderr, "缺少 --model");
        return 2;
    };
    let provider_arg = option_value(args, "--provider");
    let tier_arg = option_value(args, "--tier");
    let thinking_arg = option_value(args, "--thinking");
    if let Some(thinking) = thinking_arg.as_deref() {
        if !THINKING_LEVELS.contains(&thinking) {
            let _ = writeln!(stderr, "--thinking 只认 关|低|中|高：{thinking}");
            return 2;
        }
    }
    let store = match resources::get_resources_at(&store_file(root)) {
        Ok(store) => store,
        Err(message) => {
            let _ = writeln!(stderr, "{message}");
            return 3;
        }
    };
    let Some(agent) = store
        .agents
        .iter()
        .find(|agent| agent.id.eq_ignore_ascii_case(agent_arg.trim()))
    else {
        let ids: Vec<&str> = store.agents.iter().map(|agent| agent.id.as_str()).collect();
        let _ = writeln!(
            stderr,
            "未知 agent：{agent_arg}；资源库中可选：{}",
            join_or_none(&ids)
        );
        return 2;
    };
    let candidates: Vec<_> = agent
        .models
        .iter()
        .filter(|model| model.id == model_arg)
        .filter(|model| provider_arg.as_deref().map_or(true, |value| model.provider == value))
        .collect();
    let chosen = match candidates.as_slice() {
        [] => {
            let _ = writeln!(
                stderr,
                "模型不存在：{}/{model_arg}；该 agent 下可选：{}",
                agent.id,
                model_candidates(agent)
            );
            return 2;
        }
        [one] => (*one).clone(),
        _ => {
            let providers: Vec<&str> = candidates.iter().map(|model| model.provider.as_str()).collect();
            let _ = writeln!(
                stderr,
                "该模型挂在多个 provider 下：{}；用 --provider 指定",
                providers.join("、")
            );
            return 2;
        }
    };
    // R1-2：roles_from 对损坏文件静默回默认值，整份写回会丢其余角色；
    // 严格读取遇损坏拒写。
    let mut roles = match crate::orchestration::roles_from_strict(root) {
        Ok(roles) => roles,
        Err(_) => {
            let _ = writeln!(
                stderr,
                "roles.json 无法解析，为避免覆盖已停止；请先修复或删除该文件"
            );
            return 1;
        }
    };
    let profile = match role.as_str() {
        "planning" => &mut roles.planning,
        "execution" => &mut roles.execution,
        _ => &mut roles.acceptance,
    };
    // 省略的 --tier / --thinking 保持原值；agent / provider / model 用资源库里的规范值。
    let tier = tier_arg.unwrap_or_else(|| profile.tier.clone());
    let thinking = thinking_arg.unwrap_or_else(|| profile.thinking.clone());
    *profile = RoleProfile {
        agent: agent.id.clone(),
        provider: chosen.provider.clone(),
        model: chosen.id.clone(),
        tier,
        thinking,
    };
    if let Err(message) = crate::orchestration::save_roles_in(root, &roles) {
        let _ = writeln!(stderr, "{message}");
        return 1;
    }
    let _ = writeln!(stdout, "{}", serde_json::to_string(&roles).unwrap_or_else(|_| "{}".into()));
    let _ = writeln!(stderr, "已写入角色配置");
    0
}

/// 单个键的夹取：把键值塞进只有这一个键的 map，走 `settings_from_value`，
/// 取出对应字段——与 `load_settings` 读文件时用同一套区间与取整规则。
fn clamp_setting_value(key: &str, number: f64) -> Value {
    let mut map = Map::new();
    map.insert(
        key.to_string(),
        Value::Number(serde_json::Number::from_f64(number).unwrap_or_else(|| serde_json::Number::from(0))),
    );
    let settings = crate::engine::settings_from_value(&map);
    match key {
        "timeoutSecs" => json!(settings.timeout_secs),
        "concurrency" => json!(settings.concurrency),
        "maxRequests" => json!(settings.max_requests),
        "fixRoundLimit" => json!(settings.fix_round_limit),
        "swarmLimit" => json!(settings.swarm_limit),
        _ => json!(settings.console_run_limit),
    }
}

fn run_config_set(
    root: &Path,
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let Some(key) = positional(args, 3) else {
        let _ = writeln!(stderr, "缺少键");
        return 2;
    };
    // R1-5：值按原样取第 4 个参数，允许负数与以 `-` 开头的文本（负数按
    // 夹取规则夹到下限），不再把 `-` 开头当成选项前缀。
    let Some(value) = args.get(4).cloned() else {
        let _ = writeln!(stderr, "缺少值");
        return 2;
    };
    if let Some(message) = extra_arg_error(args, 5) {
        let _ = writeln!(stderr, "{message}");
        return 2;
    }
    let Some(kind) = setting_kind(&key) else {
        let _ = writeln!(stderr, "未知键：{key}");
        return 2;
    };
    let mut clamped = None;
    let json_value = match kind {
        SettingKind::Number => {
            let Ok(number) = value.parse::<f64>() else {
                let _ = writeln!(stderr, "{key} 必须是数字：{value}");
                return 2;
            };
            if !number.is_finite() {
                let _ = writeln!(stderr, "{key} 必须是有限数字：{value}");
                return 2;
            }
            let clamped_value = clamp_setting_value(&key, number);
            // 区间端点从引擎夹取结果反推（极小/极大各探一次），不另抄一份
            // 常量；越界与取整分开提示（R1-6），越界优先。
            let low = clamp_setting_value(&key, f64::MIN).as_f64().unwrap_or(f64::MIN);
            let high = clamp_setting_value(&key, f64::MAX).as_f64().unwrap_or(f64::MAX);
            if number < low || number > high {
                clamped = Some(format!("{key} 越界，已夹到 {}", clamped_value));
            } else if clamped_value.as_f64() != Some(number) {
                clamped = Some(format!("{key} 已取整为 {}", clamped_value));
            }
            clamped_value
        }
        SettingKind::Bool => match value.as_str() {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            other => {
                let _ = writeln!(stderr, "{key} 只认 true 或 false：{other}");
                return 2;
            }
        },
        SettingKind::Text => Value::String(value),
    };
    let path = root.join("dispatch-settings.json");
    let mut map = match fs::read(&path) {
        Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(Value::Object(map)) => map,
            _ => {
                let _ = writeln!(stderr, "dispatch-settings.json 解析失败");
                return 1;
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Map::new(),
        Err(error) => {
            let _ = writeln!(stderr, "读取 dispatch-settings.json 失败：{error}");
            return 1;
        }
    };
    map.insert(key.clone(), json_value);
    if let Err(message) = crate::engine::write_json_atomic(&path, &Value::Object(map)) {
        let _ = writeln!(stderr, "{message}");
        return 1;
    }
    let dispatch = crate::engine::load_settings(root);
    let _ = writeln!(stdout, "{}", serde_json::to_string(&dispatch).unwrap_or_else(|_| "{}".into()));
    match clamped {
        Some(note) => {
            let _ = writeln!(stderr, "已写入派发设置；{note}");
        }
        None => {
            let _ = writeln!(stderr, "已写入派发设置");
        }
    }
    0
}

// ── resources / config 的输出与参数工具 ──────────────────────────────

fn output_resources(store: &ResourceStore, only_agent: Option<&str>, stdout: &mut dyn Write) {
    let now_millis = chrono::Local::now().timestamp_millis();
    let entries: Vec<Value> = store
        .agents
        .iter()
        .filter(|agent| only_agent.map_or(true, |id| agent.id == id))
        .map(|agent| agent_entry(agent, now_millis))
        .collect();
    let value = match (only_agent, entries.len()) {
        (Some(_), 1) => entries.into_iter().next().expect("len checked"),
        _ => json!({ "agents": entries }),
    };
    let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
}

fn agent_entry(agent: &ResourceAgent, now_millis: i64) -> Value {
    let detected = agent.detected;
    let models: Vec<Value> = agent
        .models
        .iter()
        .map(|model| {
            let (state, note) = resources::model_state(model, detected, now_millis);
            json!({
                "id": model.id,
                "agent": model.agent,
                "provider": model.provider,
                "resolved_id": model.resolved_id,
                "state": state,
                "note": note,
            })
        })
        .collect();
    json!({
        "id": agent.id,
        "name": agent.name,
        "version": agent.version,
        "state": resources::agent_state(agent.detected),
        "note": resources::agent_state_note(agent),
        "models": models,
    })
}

/// 该 agent 下可选的模型清单（provider 非空时带 provider 前缀）；空时写「（无）」。
fn model_candidates(agent: &ResourceAgent) -> String {
    let items: Vec<String> = agent
        .models
        .iter()
        .map(|model| {
            if model.provider.trim().is_empty() {
                model.id.clone()
            } else {
                format!("{}/{}", model.provider, model.id)
            }
        })
        .collect();
    join_or_none(&items.iter().map(String::as_str).collect::<Vec<_>>())
}

fn join_or_none(items: &[&str]) -> String {
    if items.is_empty() {
        "（无）".to_string()
    } else {
        items.join("、")
    }
}

fn extra_arg_error(args: &[String], from: usize) -> Option<String> {
    args.get(from).map(|arg| format!("不认识的参数：{arg}"))
}

/// 只认 allowed 里的带值选项；出现位置参数、未知选项或选项缺值时报参数错。
/// 带值选项写了但后面没有值（到末尾，或下一个参数以 `-` 开头）时报
/// 「缺少 <选项> 的值」（fix-39-r1 R1-1：否则 `smoke pi --model` 漏写模型名
/// 会静默改成对该 agent 全部第三方模型批量冒烟，真实消耗额度）。
fn unknown_option_error(args: &[String], from: usize, allowed: &[&str]) -> Option<String> {
    let mut index = from;
    while index < args.len() {
        let arg = args[index].as_str();
        if !arg.starts_with('-') || !allowed.contains(&arg) {
            return Some(format!("不认识的参数：{arg}"));
        }
        match args.get(index + 1) {
            Some(value) if !value.starts_with('-') => index += 2,
            _ => return Some(format!("缺少 {arg} 的值")),
        }
    }
    None
}

#[cfg(windows)]
pub(crate) fn attach_parent_console() {
    use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    unsafe { let _ = AttachConsole(ATTACH_PARENT_PROCESS); }
}

#[cfg(not(windows))]
pub(crate) fn attach_parent_console() {}

struct CliRequest {
    body: Option<Value>,
    timeout: Duration,
}

/// 命令行参数校验与派发请求体的组装。status / adopt / cancel 的位置参数与
/// 选项在这里统一检查，实际执行直接走引擎函数，不再有窗口请求。
fn build_request(args: &[String]) -> Result<CliRequest, String> {
    match args.get(1).map(String::as_str) {
        Some("dispatch") => {
            if args.iter().any(|arg| arg == "--text" || arg.starts_with("--text=")) {
                return Err("派发文本必须使用 --text-file".into());
            }
            let project = option_value(args, "--project").ok_or_else(|| "缺少 --project".to_string())?;
            let text_file = option_value(args, "--text-file").ok_or_else(|| "缺少 --text-file".to_string())?;
            let bytes = fs::read(&text_file).map_err(|error| format!("读取文本文件失败：{error}"))?;
            let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
            let text = String::from_utf8(bytes.to_vec()).map_err(|_| "文本文件必须是 UTF-8 编码".to_string())?;
            let preset = option_value(args, "--preset").unwrap_or_else(|| "economy".into());
            let preset = match preset.as_str() {
                "shared" => "builtin-shared",
                "economy" => "builtin-economy",
                "blind" => "builtin-blind",
                other => other,
            };
            Ok(CliRequest {
                body: Some(json!({ "project": project, "preset": preset, "text": text })),
                timeout: Duration::from_secs(30),
            })
        }
        Some("status") => {
            positional(args, 2).ok_or_else(|| "缺少 run_id".to_string())?;
            let timeout = option_value(args, "--timeout").map(|value| value.parse::<u64>().map_err(|_| "--timeout 必须是正整数".to_string())).transpose()?.unwrap_or(600);
            if timeout == 0 { return Err("--timeout 必须是正整数".into()); }
            Ok(CliRequest { body: None, timeout: Duration::from_secs(timeout) })
        }
        Some("adopt") => {
            positional(args, 2).ok_or_else(|| "缺少 run_id".to_string())?;
            let node_id = positional(args, 3).ok_or_else(|| "缺少 node_id".to_string())?;
            Ok(CliRequest { body: Some(json!({ "node_id": node_id })), timeout: Duration::from_secs(30) })
        }
        Some("cancel") => {
            positional(args, 2).ok_or_else(|| "缺少 run_id".to_string())?;
            Ok(CliRequest { body: Some(json!({})), timeout: Duration::from_secs(30) })
        }
        _ => Err("未知命令".into()),
    }
}

fn option_value(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .filter(|value| !value.starts_with('-'))
        .cloned()
}

fn positional(args: &[String], index: usize) -> Option<String> {
    args.get(index).filter(|arg| !arg.starts_with('-')).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn missing_run_is_a_business_failure() {
        // status 直接走引擎读 run：run 不存在是业务失败（退出码 1），不再有清单一说。
        let root = std::env::temp_dir().join(format!("aicross-cli-missing-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let args = vec!["aicross".into(), "status".into(), "run-1".into()];
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(run_at(&args, &root, &mut stdout, &mut stderr), 1);
        assert!(stdout.is_empty());
        assert!(String::from_utf8(stderr).unwrap().contains("读取 run 失败"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dispatch_reads_multiline_text_from_file_only() {
        let root = std::env::temp_dir().join(format!("aicross-cli-text-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let text_path = root.join("prompt.txt");
        fs::write(&text_path, "第一行\n第二行").unwrap();
        let args = vec![
            "aicross".into(), "dispatch".into(), "--project".into(), root.to_string_lossy().into(),
            "--preset".into(), "economy".into(), "--text-file".into(), text_path.to_string_lossy().into(),
        ];
        let request = build_request(&args).unwrap();
        assert_eq!(request.body.unwrap()["text"], "第一行\n第二行");
        assert!(!args.iter().any(|arg| arg.contains("第一行")));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dispatch_rejects_text_on_argv() {
        let args = vec![
            "aicross".into(), "dispatch".into(), "--project".into(), "D:/p".into(),
            "--text".into(), "第一行\n第二行".into(),
        ];
        assert!(matches!(
            build_request(&args),
            Err(error) if error == "派发文本必须使用 --text-file"
        ));
        let equals = vec!["aicross".into(), "dispatch".into(), "--text=正文".into()];
        assert!(matches!(build_request(&equals), Err(error) if error == "派发文本必须使用 --text-file"));
    }

    #[test]
    fn dispatch_strips_utf8_bom_and_rejects_non_utf8() {
        let root = std::env::temp_dir().join(format!("aicross-cli-encoding-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("prompt.txt");
        fs::write(&path, [vec![0xef, 0xbb, 0xbf], "正文".as_bytes().to_vec()].concat()).unwrap();
        let args = vec!["aicross".into(), "dispatch".into(), "--project".into(), "D:/p".into(), "--text-file".into(), path.to_string_lossy().into()];
        assert_eq!(build_request(&args).unwrap().body.unwrap()["text"], "正文");
        fs::write(&path, [0xff, 0xfe, 0x00]).unwrap();
        assert!(matches!(build_request(&args), Err(error) if error == "文本文件必须是 UTF-8 编码"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn status_wait_timeout_defaults_to_600_and_accepts_override() {
        let default = build_request(&["aicross".into(), "status".into(), "run".into(), "--wait".into()]).unwrap();
        assert_eq!(default.timeout, Duration::from_secs(600));
        let custom = build_request(&["aicross".into(), "status".into(), "run".into(), "--wait".into(), "--timeout".into(), "7".into()]).unwrap();
        assert_eq!(custom.timeout, Duration::from_secs(7));
    }

    #[test]
    fn status_adopt_and_cancel_build_engine_requests() {
        let status = build_request(&["aicross".into(), "status".into(), "run-1".into(), "--wait".into()]).unwrap();
        assert_eq!(status.body.is_none(), true);
        let adopt = build_request(&["aicross".into(), "adopt".into(), "run-1".into(), "node-1".into()]).unwrap();
        assert_eq!(adopt.body.unwrap()["node_id"], "node-1");
        let cancel = build_request(&["aicross".into(), "cancel".into(), "run-1".into()]).unwrap();
        assert_eq!(cancel.body, Some(serde_json::json!({})));
    }

    #[test]
    fn run_subcommand_validates_run_id() {
        let root = std::env::temp_dir().join(format!("aicross-cli-run-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(run_at(&["aicross".into(), "__run".into()], &root, &mut stdout, &mut stderr), 2);
        assert_eq!(run_at(&["aicross".into(), "__run".into(), "bad/id".into()], &root, &mut stdout, &mut stderr), 2);
        assert!(String::from_utf8(stderr).unwrap().contains("run_id 无效"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn run_subcommand_skips_parent_console_attach() {
        // R2-5：面向用户的子命令要挂回宿主终端，后台 __run 不要。
        for command in [
            "dispatch", "status", "adopt", "cancel", "resources", "config",
        ] {
            assert!(attaches_parent_console(Some(command)), "{command} 应挂接父控制台");
        }
        assert!(!attaches_parent_console(Some("__run")));
        assert!(!attaches_parent_console(None));
    }

    #[test]
    fn dispatch_exit_3_when_runs_dir_not_writable() {
        // R2-12：退出码 3 的「run 目录不可写」来源。
        let root = std::env::temp_dir().join(format!("aicross-cli-runs-dir-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("runs"), "blocked").unwrap();
        let text_path = root.join("prompt.txt");
        fs::write(&text_path, "正文").unwrap();
        let args = vec![
            "aicross".into(), "dispatch".into(), "--project".into(), root.to_string_lossy().into(),
            "--text-file".into(), text_path.to_string_lossy().into(),
        ];
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(run_at(&args, &root, &mut stdout, &mut stderr), 3);
        assert!(String::from_utf8(stderr).unwrap().contains("创建 run 目录失败"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn exit_code_3_comes_from_engine_root() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        // 引擎根目录不可确定（例如没有用户目录）时一律退出码 3。
        let status_args = vec!["aicross".into(), "status".into(), "run-1".into()];
        assert_eq!(dispatch_root(&status_args, Err("没有用户目录".into()), &mut stdout, &mut stderr), 3);
        assert!(String::from_utf8(stderr).unwrap().contains("引擎不可用"));
    }

    // ── TASK-39：resources / config 命令层 ──────────────────────────

    fn fixture_root(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("aicross-task39-{tag}-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    /// 资源库样板：claude 原生一条；pi 五条覆盖已验证 / 未验证 / 验证失败 /
    /// 未登录 / 额度用尽；kimi 未安装。同名 glm-4.7 挂两个 provider 供歧义用。
    fn write_fixture_store(root: &Path) {
        fs::write(
            root.join("resources.json"),
            r#"{"version":1,"agents":[
                {"id":"claude","name":"Claude Code","kind":"内置","detected":true,"version":"2.1.270","models":[
                    {"id":"sonnet","agent":"claude","provider":"","status":"待冒烟","resolved_id":"claude-sonnet-5","resolved_source":"alias-table"}
                ]},
                {"id":"pi","name":"pi agent","kind":"内置","detected":true,"version":"0.86.1","models":[
                    {"id":"glm-4.7","agent":"pi","provider":"zai","status":"已接入","verification":"verified","resolved_id":"glm-4.7","resolved_source":"identity","smoke_model":"zai/glm-4.7"},
                    {"id":"glm-5.3","agent":"pi","provider":"zai","status":"待冒烟","verification":"unverified","resolved_id":"glm-5.3","resolved_source":"alias-table"},
                    {"id":"bad","agent":"pi","provider":"zai","status":"冒烟失败","verification":"failed","smoke_failure":"403: denied","resolved_id":"bad","resolved_source":"alias-table"},
                    {"id":"glm-4.7","agent":"pi","provider":"bigmodel","status":"待冒烟","verification":"unverified","login":"logged_out","resolved_id":"glm-4.7","resolved_source":"alias-table"},
                    {"id":"quota","agent":"pi","provider":"zai","status":"待冒烟","verification":"unverified","quota_exhausted_until":"2099-01-01T00:00:00+08:00","resolved_id":"quota","resolved_source":"alias-table"}
                ]},
                {"id":"kimi","name":"Kimi Code","kind":"内置","detected":false,"version":"未检测","models":[
                    {"id":"k2","agent":"kimi","provider":"","status":"待冒烟","resolved_id":"k2","resolved_source":"alias-table"}
                ]}
            ]}"#,
        )
        .unwrap();
    }

    fn run_with(root: &Path, args: &[&str], ops: &ResourcesOps) -> (i32, String, String) {
        let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_at_with(&args, root, ops, &mut stdout, &mut stderr);
        (code, String::from_utf8(stdout).unwrap(), String::from_utf8(stderr).unwrap())
    }

    fn model_state_in(output: &str, agent_id: &str, provider: &str, model_id: &str) -> serde_json::Value {
        let parsed: serde_json::Value = serde_json::from_str(output.lines().next().unwrap()).unwrap();
        parsed
            .get("agents")
            .and_then(|agents| agents.as_array())
            .expect("agents array")
            .iter()
            .find(|agent| agent.get("id").and_then(Value::as_str) == Some(agent_id))
            .expect(agent_id)
            .get("models")
            .and_then(|models| models.as_array())
            .expect("models array")
            .iter()
            .find(|model| {
                model.get("id").and_then(Value::as_str) == Some(model_id)
                    && model.get("provider").and_then(Value::as_str) == Some(provider)
            })
            .expect(model_id)
            .clone()
    }

    #[test]
    fn task39_resources_lists_states_without_probing() {
        let root = fixture_root("list");
        write_fixture_store(&root);
        let (code, stdout, stderr) = run_with(&root, &["aicross", "resources"], &ResourcesOps::production());
        assert_eq!(code, 0);
        assert!(stderr.contains("已读取资源库"), "{stderr}");
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        // 读取层的 ensure_agents 会补齐缺失的内置 agent（codex 不在样板里，补进来）
        assert_eq!(parsed["agents"].as_array().unwrap().len(), 4);
        let kimi = parsed["agents"].as_array().unwrap().iter()
            .find(|agent| agent["id"] == "kimi").unwrap();
        assert_eq!(kimi["state"], "未安装");
        assert_eq!(kimi["note"], "安装后运行 aicross resources detect");
        assert_eq!(kimi["version"], "未检测");
        // R1-4：agent 未安装时它下面的模型状态跟随 agent，note 与 agent 一致
        assert_eq!(kimi["models"].as_array().unwrap().len(), 1);
        assert_eq!(kimi["models"][0]["state"], "未安装");
        assert_eq!(kimi["models"][0]["note"], "安装后运行 aicross resources detect");
        let pi = parsed["agents"].as_array().unwrap().iter()
            .find(|agent| agent["id"] == "pi").unwrap();
        assert_eq!(pi["state"], "可用");
        let state = |provider: &str, id: &str| {
            model_state_in(&stdout, "pi", provider, id)["state"].clone()
        };
        assert_eq!(state("zai", "glm-4.7"), "已验证");
        assert_eq!(state("zai", "glm-5.3"), "未验证");
        assert_eq!(state("zai", "bad"), "验证失败");
        assert_eq!(state("bigmodel", "glm-4.7"), "未登录");
        let logged = model_state_in(&stdout, "pi", "bigmodel", "glm-4.7");
        assert_eq!(
            logged["note"],
            "在 pi 里执行 /login bigmodel；登录后运行 aicross resources detect 刷新"
        );
        let quota = model_state_in(&stdout, "pi", "zai", "quota");
        assert_eq!(quota["state"], "额度用尽");
        assert_eq!(quota["note"], "额度恢复时间：2099-01-01T00:00:00+08:00");
        assert_eq!(model_state_in(&stdout, "claude", "", "sonnet")["state"], "可用");
        assert_eq!(model_state_in(&stdout, "claude", "", "sonnet")["resolved_id"], "claude-sonnet-5");
        // 只读：不探测、不写盘
        assert!(!stderr.contains("探测"), "{stderr}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task39_resources_detect_uses_injected_detector_and_passes_cc_switch() {
        let root = fixture_root("detect");
        write_fixture_store(&root);
        fs::write(root.join("dispatch-settings.json"), r#"{"ccSwitchPath":"D:\\tools\\cc_switch.py"}"#).unwrap();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Option<String>>::new()));
        let recorder = std::sync::Arc::clone(&seen);
        let ops = ResourcesOps {
            detect_all: Box::new(move |path, cc_switch| {
                recorder.lock().unwrap().push(cc_switch.map(str::to_string));
                // 假探测：kimi 装上了，其余照旧；带一条探测错误验证 stderr 逐行给出。
                let mut store = resources::get_resources_at(path)?;
                if let Some(agent) = store.agents.iter_mut().find(|agent| agent.id == "kimi") {
                    agent.detected = true;
                    agent.version = "1.2.3".to_string();
                }
                Ok(ResourceOperation {
                    errors: vec!["探测 codex 失败：读取 ~/.codex/models_cache.json 失败".to_string()],
                    cc_switch_hints: Vec::new(),
                    store,
                })
            }),
            ..ResourcesOps::production()
        };
        let (code, stdout, stderr) = run_with(&root, &["aicross", "resources", "detect"], &ops);
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(*seen.lock().unwrap(), vec![Some("D:\\tools\\cc_switch.py".to_string())]);
        assert!(stderr.contains("探测 codex 失败"), "{stderr}");
        assert!(stderr.contains("已探测 4 个 agent"), "{stderr}");
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        let kimi = parsed["agents"].as_array().unwrap().iter()
            .find(|agent| agent["id"] == "kimi").unwrap();
        assert_eq!(kimi["state"], "可用");
        assert_eq!(kimi["version"], "1.2.3");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task39_smoke_single_native_model_skips_request() {
        let root = fixture_root("smoke-native");
        write_fixture_store(&root);
        let called = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let marker = std::sync::Arc::clone(&called);
        let ops = ResourcesOps {
            smoke_one: Box::new(move |_, _| {
                marker.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                panic!("原生模型不该调冒烟")
            }),
            ..ResourcesOps::production()
        };
        let (code, stdout, stderr) =
            run_with(&root, &["aicross", "resources", "smoke", "claude", "--model", "sonnet"], &ops);
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(called.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert!(stderr.contains("原生模型无需冒烟"), "{stderr}");
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        // 输出单个 agent 条目，不是 agents 数组
        assert_eq!(parsed["id"], "claude");
        assert_eq!(parsed["models"][0]["state"], "可用");
        let _ = fs::remove_dir_all(root);
    }

    /// 假冒烟：把目标模型标成指定结果并写回资源库，返回写后的 store。
    fn fake_smoke_outcome(
        path: &Path,
        key: &ResourceModelKey,
        success: bool,
    ) -> Result<ResourceStore, String> {
        let mut store = resources::get_resources_at(path)?;
        let agent = store
            .agents
            .iter_mut()
            .find(|agent| agent.id == key.agent_id)
            .expect("agent");
        let model = agent
            .models
            .iter_mut()
            .find(|model| model.id == key.id && model.provider == key.provider)
            .expect("model");
        if success {
            model.status = "已接入".into();
            model.verification = resources::VERIFICATION_VERIFIED.into();
            model.smoke_model = format!("{}/{}", key.provider, key.id);
        } else {
            model.status = "冒烟失败".into();
            model.verification = resources::VERIFICATION_FAILED.into();
            model.smoke_failure = "HTTP 429: too many requests".into();
        }
        resources::save_store(path, &store)?;
        Ok(store)
    }

    #[test]
    fn task39_smoke_single_third_party_success_and_failure() {
        let root = fixture_root("smoke-one");
        write_fixture_store(&root);
        let ok_ops = ResourcesOps {
            smoke_one: Box::new(|path, key| fake_smoke_outcome(path, &key, true)),
            ..ResourcesOps::production()
        };
        let (code, stdout, stderr) = run_with(
            &root,
            &["aicross", "resources", "smoke", "pi", "--model", "glm-5.3", "--provider", "zai"],
            &ok_ops,
        );
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.contains("冒烟完成"), "{stderr}");
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["id"], "pi");
        let model = parsed["models"].as_array().unwrap().iter()
            .find(|model| model["id"] == "glm-5.3").unwrap();
        assert_eq!(model["state"], "已验证");

        let root = fixture_root("smoke-one-fail");
        write_fixture_store(&root);
        let fail_ops = ResourcesOps {
            smoke_one: Box::new(|path, key| fake_smoke_outcome(path, &key, false)),
            ..ResourcesOps::production()
        };
        let (code, stdout, stderr) = run_with(
            &root,
            &["aicross", "resources", "smoke", "pi", "--model", "glm-5.3"],
            &fail_ops,
        );
        assert_eq!(code, 1, "{stderr}");
        assert!(stderr.contains("冒烟失败：HTTP 429"), "{stderr}");
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        let model = parsed["models"].as_array().unwrap().iter()
            .find(|model| model["id"] == "glm-5.3").unwrap();
        assert_eq!(model["state"], "验证失败");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task39_smoke_agent_batch_reports_failures() {
        let root = fixture_root("smoke-batch");
        write_fixture_store(&root);
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let recorder = std::sync::Arc::clone(&seen);
        let ops = ResourcesOps {
            smoke_agent_third_party: Box::new(move |path, agent_id| {
                recorder.lock().unwrap().push(agent_id.to_string());
                // 假批冒烟：glm-5.3 成并写回资源库，其余不动；报两条失败。
                let mut store = resources::get_resources_at(path)?;
                let agent = store
                    .agents
                    .iter_mut()
                    .find(|agent| agent.id == agent_id)
                    .expect("agent");
                let model = agent
                    .models
                    .iter_mut()
                    .find(|model| model.id == "glm-5.3" && model.provider == "zai")
                    .expect("model");
                model.status = "已接入".into();
                model.verification = resources::VERIFICATION_VERIFIED.into();
                resources::save_store(path, &store)?;
                Ok(ResourceOperation {
                    errors: vec![
                        "验证 pi/bad 失败：403: denied".to_string(),
                        "验证 pi/quota 失败：429".to_string(),
                    ],
                    cc_switch_hints: Vec::new(),
                    store,
                })
            }),
            ..ResourcesOps::production()
        };
        let (code, stdout, stderr) = run_with(&root, &["aicross", "resources", "smoke", "pi"], &ops);
        assert_eq!(code, 1, "{stderr}");
        assert_eq!(*seen.lock().unwrap(), vec!["pi".to_string()]);
        assert!(stderr.contains("验证 pi/bad 失败"), "{stderr}");
        assert!(stderr.contains("冒烟完成；2 条失败"), "{stderr}");
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["id"], "pi");
        let model = parsed["models"].as_array().unwrap().iter()
            .find(|model| model["id"] == "glm-5.3").unwrap();
        assert_eq!(model["state"], "已验证");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task39_smoke_agent_without_third_party_exits_zero() {
        let root = fixture_root("smoke-all-native");
        write_fixture_store(&root);
        let ops = ResourcesOps {
            smoke_agent_third_party: Box::new(|_, _| {
                Ok(ResourceOperation::default())
            }),
            ..ResourcesOps::production()
        };
        let (code, stdout, stderr) = run_with(&root, &["aicross", "resources", "smoke", "claude"], &ops);
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.contains("原生模型无需冒烟"), "{stderr}");
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["id"], "claude");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task39_smoke_rejects_bad_arguments() {
        let root = fixture_root("smoke-args");
        write_fixture_store(&root);
        for (args, expect) in [
            (vec!["aicross", "resources", "smoke"], "缺少 agent"),
            (vec!["aicross", "resources", "smoke", "nope"], "未知 agent：nope"),
            (vec!["aicross", "resources", "smoke", "pi", "--model", "ghost"], "模型不存在：pi/ghost"),
            (vec!["aicross", "resources", "smoke", "pi", "--model", "glm-4.7"], "多个 provider"),
            (vec!["aicross", "resources", "smoke", "pi", "--foo"], "不认识的参数"),
            (vec!["aicross", "resources", "smoke", "pi", "--provider", "zai"], "--provider 需要 --model"),
            (vec!["aicross", "resources", "bogus"], "未知子命令"),
            (vec!["aicross", "resources", "detect", "extra"], "不认识的参数"),
        ] {
            let (code, _, stderr) = run_with(&root, &args, &ResourcesOps::production());
            assert_eq!(code, 2, "{args:?} → {stderr}");
            assert!(stderr.contains(expect), "{args:?} → {stderr}");
        }
        // 候选清单里 provider 非空的带前缀，agent 不存在时列出可选 agent
        let (_, _, stderr) = run_with(
            &root,
            &["aicross", "resources", "smoke", "pi", "--model", "ghost"],
            &ResourcesOps::production(),
        );
        assert!(stderr.contains("zai/glm-4.7"), "{stderr}");
        assert!(stderr.contains("bigmodel/glm-4.7"), "{stderr}");
        let (_, _, stderr) = run_with(
            &root,
            &["aicross", "resources", "smoke", "nope"],
            &ResourcesOps::production(),
        );
        assert!(stderr.contains("claude、pi"), "{stderr}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task39_config_show_outputs_roles_and_dispatch() {
        let root = fixture_root("show");
        // 缺省：roles 与 dispatch 都给默认值
        let (code, stdout, stderr) = run_with(&root, &["aicross", "config", "show"], &ResourcesOps::production());
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.contains("已读取配置"), "{stderr}");
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["roles"]["version"], 1);
        assert_eq!(parsed["roles"]["execution"]["agent"], "");
        assert_eq!(parsed["dispatch"]["timeoutSecs"], 600);
        assert_eq!(parsed["dispatch"]["concurrency"], 3);
        assert_eq!(parsed["dispatch"]["redact"], true);

        // 有文件：roles 原样，dispatch 按夹取口径（concurrency 99 → 3）
        fs::write(
            root.join("roles.json"),
            r#"{"version":1,"execution":{"agent":"pi","provider":"zai","model":"glm-4.7","tier":"高","thinking":"低"}}"#,
        )
        .unwrap();
        fs::write(root.join("dispatch-settings.json"), r#"{"timeoutSecs":90,"concurrency":99}"#).unwrap();
        let (code, stdout, _) = run_with(&root, &["aicross", "config", "show"], &ResourcesOps::production());
        assert_eq!(code, 0);
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["roles"]["execution"]["model"], "glm-4.7");
        assert_eq!(parsed["dispatch"]["timeoutSecs"], 90);
        assert_eq!(parsed["dispatch"]["concurrency"], 3);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task39_config_set_role_writes_canonical_target() {
        let root = fixture_root("set-role");
        write_fixture_store(&root);
        fs::write(
            root.join("roles.json"),
            r#"{"version":1,"execution":{"agent":"codex","provider":"","model":"gpt","tier":"中","thinking":"高"}}"#,
        )
        .unwrap();
        // 大小写不敏感的 agent 名回写成资源库里的规范 id；provider 取资源库里的值
        let (code, stdout, stderr) = run_with(
            &root,
            &["aicross", "config", "set-role", "execution", "--agent", "PI", "--model", "glm-4.7", "--provider", "zai"],
            &ResourcesOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.contains("已写入角色配置"), "{stderr}");
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["execution"]["agent"], "pi");
        assert_eq!(parsed["execution"]["provider"], "zai");
        assert_eq!(parsed["execution"]["model"], "glm-4.7");
        // 省略 --tier / --thinking 保持原值
        assert_eq!(parsed["execution"]["tier"], "中");
        assert_eq!(parsed["execution"]["thinking"], "高");
        // 写入后再读一致
        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("roles.json")).unwrap()).unwrap();
        assert_eq!(saved, parsed);
        // 其他角色不动
        assert_eq!(parsed["planning"]["agent"], "");

        // 原生模型：provider 写空串
        let (code, stdout, stderr) = run_with(
            &root,
            &["aicross", "config", "set-role", "acceptance", "--agent", "claude", "--model", "sonnet", "--thinking", "关"],
            &ResourcesOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["acceptance"]["agent"], "claude");
        assert_eq!(parsed["acceptance"]["provider"], "");
        assert_eq!(parsed["acceptance"]["thinking"], "关");
        // tier 保持上一次 set-role 后的原值（本角色原为空）
        assert_eq!(parsed["acceptance"]["tier"], "");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task39_config_set_role_rejects_bad_arguments() {
        let root = fixture_root("set-role-args");
        write_fixture_store(&root);
        for (args, expect) in [
            (vec!["aicross", "config", "set-role"], "缺少角色"),
            (vec!["aicross", "config", "set-role", "boss", "--agent", "pi", "--model", "glm-4.7"], "角色只认"),
            (vec!["aicross", "config", "set-role", "execution"], "缺少 --agent"),
            (vec!["aicross", "config", "set-role", "execution", "--agent", "pi"], "缺少 --model"),
            (
                vec!["aicross", "config", "set-role", "execution", "--agent", "pi", "--model", "glm-4.7", "--thinking", "超高"],
                "--thinking 只认",
            ),
            (vec!["aicross", "config", "set-role", "execution", "--agent", "ghost", "--model", "x"], "未知 agent：ghost"),
            (
                vec!["aicross", "config", "set-role", "execution", "--agent", "pi", "--model", "ghost"],
                "模型不存在：pi/ghost",
            ),
            (
                vec!["aicross", "config", "set-role", "planning", "--agent", "pi", "--model", "glm-4.7"],
                "多个 provider",
            ),
            (vec!["aicross", "config", "set-role", "execution", "--agent", "pi", "--wat"], "不认识的参数"),
            (vec!["aicross", "config", "bogus"], "未知子命令"),
            (vec!["aicross", "config"], "缺少子命令"),
        ] {
            let (code, _, stderr) = run_with(&root, &args, &ResourcesOps::production());
            assert_eq!(code, 2, "{args:?} → {stderr}");
            assert!(stderr.contains(expect), "{args:?} → {stderr}");
        }
        // 目标不存在时 stderr 列出该 agent 下可选的模型；agent 名单也列出
        let (_, _, stderr) = run_with(
            &root,
            &["aicross", "config", "set-role", "execution", "--agent", "pi", "--model", "ghost"],
            &ResourcesOps::production(),
        );
        assert!(stderr.contains("该 agent 下可选"), "{stderr}");
        assert!(stderr.contains("zai/glm-4.7"), "{stderr}");
        assert!(stderr.contains("bigmodel/glm-4.7"), "{stderr}");
        let (_, _, stderr) = run_with(
            &root,
            &["aicross", "config", "set-role", "execution", "--agent", "ghost", "--model", "x"],
            &ResourcesOps::production(),
        );
        assert!(stderr.contains("资源库中可选：claude、pi、kimi、codex"), "{stderr}");
        // 失败时不写盘
        assert!(!root.join("roles.json").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task39_config_set_clamps_writes_and_roundtrips() {
        let root = fixture_root("set");
        // 越界夹取：timeoutSecs 9999 → 3600，stderr 说明
        let (code, stdout, stderr) =
            run_with(&root, &["aicross", "config", "set", "timeoutSecs", "9999"], &ResourcesOps::production());
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.contains("timeoutSecs 越界，已夹到 3600"), "{stderr}");
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["timeoutSecs"], 3600);
        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("dispatch-settings.json")).unwrap()).unwrap();
        assert_eq!(saved["timeoutSecs"], 3600);

        // 正常值 + 布尔 + 字符串：按 JSON 类型写入；未知键保留不删
        fs::write(
            root.join("dispatch-settings.json"),
            r#"{"futureKey":"keep","timeoutSecs":10}"#,
        )
        .unwrap();
        for (key, value) in [("concurrency", "2"), ("redact", "false"), ("classifyProposals", "true"), ("classificationModel", "zai/glm-5.3")] {
            let (code, _, stderr) =
                run_with(&root, &["aicross", "config", "set", key, value], &ResourcesOps::production());
            assert_eq!(code, 0, "{stderr}");
        }
        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("dispatch-settings.json")).unwrap()).unwrap();
        assert_eq!(saved["futureKey"], "keep");
        assert_eq!(saved["timeoutSecs"], 10);
        assert_eq!(saved["concurrency"], 2);
        assert_eq!(saved["redact"], false);
        assert_eq!(saved["classifyProposals"], true);
        assert_eq!(saved["classificationModel"], "zai/glm-5.3");

        // 写入后再读一致：config show 读回同一份
        let (code, stdout, _) = run_with(&root, &["aicross", "config", "show"], &ResourcesOps::production());
        assert_eq!(code, 0);
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["dispatch"]["concurrency"], 2);
        assert_eq!(parsed["dispatch"]["redact"], false);
        assert_eq!(parsed["dispatch"]["timeoutSecs"], 10);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task39_config_set_rejects_bad_input() {
        let root = fixture_root("set-args");
        for (args, expect) in [
            (vec!["aicross", "config", "set"], "缺少键"),
            (vec!["aicross", "config", "set", "timeoutSecs"], "缺少值"),
            (vec!["aicross", "config", "set", "unknownKey", "1"], "未知键"),
            (vec!["aicross", "config", "set", "timeoutSecs", "abc"], "必须是数字"),
            (vec!["aicross", "config", "set", "redact", "yes"], "只认 true 或 false"),
            (vec!["aicross", "config", "set", "timeoutSecs", "5", "extra"], "不认识的参数"),
        ] {
            let (code, _, stderr) = run_with(&root, &args, &ResourcesOps::production());
            assert_eq!(code, 2, "{args:?} → {stderr}");
            assert!(stderr.contains(expect), "{args:?} → {stderr}");
        }
        assert!(!root.join("dispatch-settings.json").exists());

        // 文件损坏：不覆盖，业务失败
        fs::write(root.join("dispatch-settings.json"), "{ 损坏").unwrap();
        let (code, _, stderr) =
            run_with(&root, &["aicross", "config", "set", "timeoutSecs", "5"], &ResourcesOps::production());
        assert_eq!(code, 1, "{stderr}");
        assert!(stderr.contains("解析失败"), "{stderr}");
        assert_eq!(fs::read_to_string(root.join("dispatch-settings.json")).unwrap(), "{ 损坏");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task39_resources_and_config_exit_3_sources() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        // 来源一：引擎根目录不可确定（resources 与 config 各验一次）。
        assert_eq!(
            dispatch_root(
                &["aicross".into(), "resources".into()],
                Err("没有用户目录".into()),
                &mut stdout,
                &mut stderr,
            ),
            3
        );
        assert_eq!(
            dispatch_root(
                &["aicross".into(), "config".into(), "show".into()],
                Err("没有用户目录".into()),
                &mut stdout,
                &mut stderr,
            ),
            3
        );
        assert!(String::from_utf8(stderr.clone()).unwrap().contains("引擎不可用"));
        // 来源二：资源库读不出来（损坏不覆盖）。
        let root = fixture_root("exit3");
        fs::write(root.join("resources.json"), "{ 损坏").unwrap();
        assert_eq!(
            run_with(&root, &["aicross", "resources"], &ResourcesOps::production()).0,
            3
        );
        assert_eq!(
            run_with(
                &root,
                &["aicross", "resources", "smoke", "pi"],
                &ResourcesOps::production(),
            )
            .0,
            3
        );
        assert_eq!(
            run_with(
                &root,
                &["aicross", "config", "set-role", "execution", "--agent", "pi", "--model", "m"],
                &ResourcesOps::production(),
            )
            .0,
            3
        );
        assert_eq!(fs::read_to_string(root.join("resources.json")).unwrap(), "{ 损坏");
        let _ = fs::remove_dir_all(root);
    }

    // ── fix-39-r1 修正轮 ──────────────────────────────────────────

    /// R1-1：带值选项写了却没给值，一律退出 2；不调冒烟、不写盘。
    #[test]
    fn task39_r1_missing_option_value_exits_2_without_side_effects() {
        let root = fixture_root("r1-missing-value");
        write_fixture_store(&root);
        let store_before = fs::read(root.join("resources.json")).unwrap();
        let roles_before = r#"{"version":1,"planning":{"agent":""}}"#;
        fs::write(root.join("roles.json"), roles_before).unwrap();
        let ops = ResourcesOps {
            smoke_one: Box::new(|_, _| panic!("缺值时不该调单条冒烟")),
            smoke_agent_third_party: Box::new(|_, _| panic!("缺值时不该调批量冒烟")),
            ..ResourcesOps::production()
        };
        for (args, expect) in [
            (vec!["aicross", "resources", "smoke", "pi", "--model"], "缺少 --model 的值"),
            (
                vec!["aicross", "resources", "smoke", "pi", "--model", "glm", "--provider"],
                "缺少 --provider 的值",
            ),
            (
                vec![
                    "aicross", "config", "set-role", "execution", "--agent", "codex",
                    "--model", "x", "--thinking",
                ],
                "缺少 --thinking 的值",
            ),
        ] {
            let (code, stdout, stderr) = run_with(&root, &args, &ops);
            assert_eq!(code, 2, "{args:?} → {stderr}");
            assert!(stdout.is_empty(), "{args:?} → {stdout}");
            assert!(stderr.contains(expect), "{args:?} → {stderr}");
        }
        assert_eq!(fs::read(root.join("resources.json")).unwrap(), store_before);
        assert_eq!(fs::read_to_string(root.join("roles.json")).unwrap(), roles_before);
        let _ = fs::remove_dir_all(root);
    }

    /// R1-2：`set-role` 遇损坏或版本不认的 roles.json 拒写（退出 1，字节
    /// 不变）；`config show` 同样情况 stdout 输出默认值、stderr 提示一行。
    #[test]
    fn task39_r1_set_role_refuses_broken_roles_and_show_hints() {
        for (tag, contents) in [("broken", "{ 损坏"), ("version", r#"{"version":2,"execution":{"agent":"pi"}}"#)] {
            let root = fixture_root(&format!("r1-roles-{tag}"));
            write_fixture_store(&root);
            fs::write(root.join("roles.json"), contents).unwrap();
            let (code, stdout, stderr) = run_with(
                &root,
                &["aicross", "config", "set-role", "execution", "--agent", "pi", "--model", "glm-5.3"],
                &ResourcesOps::production(),
            );
            assert_eq!(code, 1, "{tag} → {stderr}");
            assert!(stdout.is_empty(), "{tag} → {stdout}");
            assert!(
                stderr.contains("roles.json 无法解析，为避免覆盖已停止；请先修复或删除该文件"),
                "{tag} → {stderr}"
            );
            assert_eq!(fs::read_to_string(root.join("roles.json")).unwrap(), contents);

            let (code, stdout, stderr) =
                run_with(&root, &["aicross", "config", "show"], &ResourcesOps::production());
            assert_eq!(code, 0, "{tag} → {stderr}");
            let parsed: serde_json::Value =
                serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
            assert_eq!(parsed["roles"]["version"], 1);
            assert_eq!(parsed["roles"]["execution"]["agent"], "");
            assert!(stderr.contains("roles.json 无法解析，已按默认值显示"), "{tag} → {stderr}");
            let _ = fs::remove_dir_all(root);
        }
    }

    /// R1-5 / R1-6：值按原样取第 4 个参数（负数夹到下限，`-` 开头的文本按
    /// 原样写入）；小数取整与越界夹取分开提示。
    #[test]
    fn task39_r1_config_set_takes_leading_dash_value_and_reports_rounding() {
        let root = fixture_root("r1-set-dash");
        let (code, _, stderr) =
            run_with(&root, &["aicross", "config", "set", "timeoutSecs", "-5"], &ResourcesOps::production());
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.contains("timeoutSecs 越界，已夹到 1"), "{stderr}");

        let (code, _, stderr) = run_with(
            &root,
            &["aicross", "config", "set", "validationCommand", "--strict"],
            &ResourcesOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");

        let (code, _, stderr) =
            run_with(&root, &["aicross", "config", "set", "concurrency", "2.5"], &ResourcesOps::production());
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.contains("concurrency 已取整为 2"), "{stderr}");

        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("dispatch-settings.json")).unwrap()).unwrap();
        assert_eq!(saved["timeoutSecs"], 1);
        assert_eq!(saved["validationCommand"], "--strict");
        assert_eq!(saved["concurrency"], 2);
        // 写入后再读一致
        let (code, stdout, _) =
            run_with(&root, &["aicross", "config", "show"], &ResourcesOps::production());
        assert_eq!(code, 0);
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["dispatch"]["timeoutSecs"], 1);
        assert_eq!(parsed["dispatch"]["concurrency"], 2);
        let _ = fs::remove_dir_all(root);
    }

    /// R1-7：批量冒烟时 agent 下没有任何模型，提示并退出 0，不发请求。
    #[test]
    fn task39_r1_smoke_agent_without_models_exits_zero() {
        let root = fixture_root("r1-smoke-empty");
        write_fixture_store(&root);
        let ops = ResourcesOps {
            smoke_agent_third_party: Box::new(|_, _| panic!("没有模型时不该调冒烟")),
            ..ResourcesOps::production()
        };
        // codex 由读取层补齐，下面一个模型都没有
        let (code, stdout, stderr) = run_with(&root, &["aicross", "resources", "smoke", "codex"], &ops);
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.contains("该 agent 下没有模型"), "{stderr}");
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["id"], "codex");
        assert_eq!(parsed["state"], "未安装");
        let _ = fs::remove_dir_all(root);
    }
}

