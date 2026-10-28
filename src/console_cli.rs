//! `aicross dispatch/status/adopt/cancel/resources/config` 命令行入口。
//! `resources` 与 `config`（TASK-39）直读资源库与配置文件，不经过窗口。

use crate::channels::acp_setup::{self, AdapterReport};
use crate::orchestration::{
    FallbackTarget, ModesFile, Preset, RoleProfile, RoleSettings, SeatChoice, Target,
    THINKING_LEVELS,
};
use crate::resources::{
    self, ResourceAgent, ResourceModel, ResourceModelKey, ResourceOperation, ResourceStore,
};
use serde_json::{json, Map, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub fn run_if_subcommand(args: &[String]) -> Option<i32> {
    let command = args.get(1).map(String::as_str);
    // 顶层 `--help` / `-h` / `--version` / `-V`：命令行打印后退出 0，
    // 不落到窗口；无参数双击照旧起窗口。
    if let Some(flag @ ("--help" | "-h" | "--version" | "-V")) = command {
        attach_parent_console();
        let mut stdout = std::io::stdout();
        return Some(top_level_info(flag, &mut stdout));
    }
    if !matches!(
        command,
        Some("dispatch" | "status" | "adopt" | "cancel" | "recover" | "propose" | "resources" | "config" | "hosts" | "skill" | "slash" | "preset" | "__run")
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

/// 顶层 `--help` 与 `--version` 的输出，打印后退出 0。子命令一览与
/// docs/MANUAL.md 的命令表一致；`propose` 已删除、`__run` 是后台引擎
/// 内部入口，都不列。
fn top_level_info(flag: &str, out: &mut dyn Write) -> i32 {
    if matches!(flag, "--version" | "-V") {
        let _ = writeln!(out, "aicross {}", env!("CARGO_PKG_VERSION"));
        return 0;
    }
    let _ = write!(
        out,
        "aicross {} —— 多厂商 agent 的派发引擎与结果查看器\n\
         \n\
         用法：\n\
         \x20 aicross                打开窗口\n\
         \x20 aicross <子命令> [参数]\n\
         \n\
         子命令：\n\
         \x20 dispatch    派发一次；多行任务文本走 --text-file，不进 argv\n\
         \x20 status      看一次 run 的状态；--wait 等到结束\n\
         \x20 adopt       采纳某一路的结果\n\
         \x20 cancel      叫停一次派发\n\
         \x20 recover     崩溃后核对；--rollback 恢复单写者改动的项目文件\n\
         \x20 open        打开该 run 的查看页\n\
         \x20 resources   看资源库 / 重新探测 / 冒烟验证 / 指定路径\n\
         \x20 config      看与改角色、派发设置、模式人选\n\
         \x20 hosts       看与改主 agent\n\
         \x20 skill       skill status / install / upgrade / uninstall\n\
         \x20 slash       斜杠命令写入各家宿主，可自定义短命令\n\
         \x20 preset      自定义模式的保存与删除\n\
         \n\
         使用说明见 docs/MANUAL.md，契约全文见 docs/CONSOLE-CHANNEL.md。\n",
        env!("CARGO_PKG_VERSION")
    );
    0
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
        // TASK-40 R1-6：propose 随窗口 HTTP 通道删除；识别成已删除命令给提示，
        // 不落进「启动窗口」的默认分支。
        Some("propose") => {
            let _ = writeln!(stderr, "propose 已删除，建议在宿主对话里给出");
            return 2;
        }
        Some("resources") => return run_resources(root, args, ops, stdout, stderr),
        Some("config") => return run_config(root, args, &AcpOps::production(), stdout, stderr),
        Some("recover") => return run_recover(root, args, stdout, stderr),
        Some("hosts") => return run_hosts(root, stdout, stderr),
        Some("skill") => return run_skill(root, args, stdout, stderr),
        Some("slash") => return run_slash(root, args, stdout, stderr),
        Some("preset") => return run_preset(root, args, stdout, stderr),
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
            for note in &request.notes {
                let _ = writeln!(stderr, "{note}");
            }
            let mut input: crate::engine::DispatchInput =
                match serde_json::from_value(request.body.unwrap_or_default()) {
                    Ok(value) => value,
                    Err(error) => { let _ = writeln!(stderr, "派发请求无效：{error}"); return 2; }
                };
            // TASK-50：--host 规则（需要 --host 或非法 host 都退出 2）；
            // 解析出的 host 传给引擎，run 文件随它记录。
            input.host = match resolve_host_or_err(root, input.host.as_deref()) {
                Ok(host) => host,
                Err((code, message)) => { let _ = writeln!(stderr, "{message}"); return code; }
            };
            return match crate::engine::start_dispatch(root, input) {
                Ok(response) => {
                    let accepted = response.get("accepted").and_then(Value::as_bool) == Some(true);
                    let _ = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap_or_else(|_| "{}".into()));
                    let _ = writeln!(stderr, "{}", if accepted { "引擎已接收派发" } else { "派发被拒绝" });
                    // TASK-48：派发前检查丢掉未发现备选时的说明随响应回传，逐行写 stderr。
                    if let Some(notes) = response.get("notes").and_then(Value::as_array) {
                        for note in notes {
                            if let Some(line) = note.as_str() {
                                let _ = writeln!(stderr, "{line}");
                            }
                        }
                    }
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

/// resources 子命令的进程边界：生产走 resources.rs 的现成函数（与 `aicross resources`
/// 输出同一套），测试注入假实现，不在单测里真探测、真发模型请求。
struct ResourcesOps {
    read_resources: Box<dyn Fn(&Path) -> Result<ResourceStore, String>>,
    detect_all: Box<dyn Fn(&Path, Option<&str>) -> Result<ResourceOperation, String>>,
    smoke_agent_third_party: Box<dyn Fn(&Path, &str) -> Result<ResourceOperation, String>>,
    smoke_one: Box<dyn Fn(&Path, ResourceModelKey) -> Result<ResourceStore, String>>,
    prune_missing: Box<dyn Fn(&Path, Option<&str>, &[resources::ModelReference]) -> Result<resources::PruneOutcome, String>>,
}

impl ResourcesOps {
    fn production() -> Self {
        ResourcesOps {
            read_resources: Box::new(resources::get_resources_at),
            detect_all: Box::new(resources::detect_all_resources_at),
            smoke_agent_third_party: Box::new(resources::smoke_agent_third_party_at),
            smoke_one: Box::new(resources::smoke_resource_model_at),
            prune_missing: Box::new(resources::prune_missing_models_at),
        }
    }
}

/// 资源库路径（命令行与一键推荐共用）。
pub(crate) fn store_file(root: &Path) -> PathBuf {
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
            match resources_detect_at(root, ops) {
                Ok(outcome) => {
                    let _ = writeln!(
                        stdout,
                        "{}",
                        serde_json::to_string(&outcome.value).unwrap_or_else(|_| "{}".into())
                    );
                    for message in &outcome.messages {
                        let _ = writeln!(stderr, "{message}");
                    }
                    0
                }
                Err((code, message)) => {
                    let _ = writeln!(stderr, "{message}");
                    code
                }
            }
        }
        Some("smoke") => run_resources_smoke(root, args, ops, stdout, stderr),
        Some("prune") => run_resources_prune(root, args, ops, stdout, stderr),
        Some("set-path") => {
            if let Some(message) = extra_arg_error(args, 5) {
                let _ = writeln!(stderr, "{message}");
                return 2;
            }
            let Some(agent) = args.get(3) else {
                let _ = writeln!(stderr, "用法：aicross resources set-path <agent> [<路径>]，不给路径即清除");
                return 2;
            };
            match resources_set_path_at(root, agent, args.get(4).map(String::as_str)) {
                Ok((value, message)) => {
                    let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
                    let _ = writeln!(stderr, "{message}");
                    0
                }
                Err((code, message)) => {
                    let _ = writeln!(stderr, "{message}");
                    code
                }
            }
        }
        Some(other) => {
            let _ = writeln!(stderr, "未知子命令：{other}");
            2
        }
    }
}

/// `resources smoke` 的实现体：命令行与窗口冒烟共用。参数错（2，含
/// 「--provider 需要 --model」，fix-46-r1 R4 挪进来让窗口同一校验）与资源库
/// 读不了（3）返回 Err；业务失败（有模型冒烟失败）在 Ok 里用 exit 表达，
/// stdout 的 JSON 与 stderr 的说明行分开返回。
struct SmokeOutcome {
    exit: i32,
    value: Value,
    messages: Vec<String>,
}

fn resources_smoke_at(
    root: &Path,
    agent_arg: &str,
    model_arg: Option<&str>,
    provider_arg: Option<&str>,
    ops: &ResourcesOps,
) -> Result<SmokeOutcome, (i32, String)> {
    if model_arg.is_none() && provider_arg.is_some() {
        return Err((2, "--provider 需要 --model".into()));
    }
    let store = (ops.read_resources)(&store_file(root)).map_err(|message| (3, message))?;
    let Some(agent) = store
        .agents
        .iter()
        .find(|agent| agent.id.eq_ignore_ascii_case(agent_arg.trim()))
    else {
        let ids: Vec<&str> = store.agents.iter().map(|agent| agent.id.as_str()).collect();
        return Err((
            2,
            format!("未知 agent：{agent_arg}；可选：{}", join_or_none(&ids)),
        ));
    };
    let agent_id = agent.id.clone();
    match model_arg {
        Some(model_id) => {
            let candidates: Vec<_> = agent
                .models
                .iter()
                .filter(|model| model.id == model_id)
                .filter(|model| provider_arg.map_or(true, |value| model.provider == value))
                .collect();
            let target = match candidates.as_slice() {
                [] => {
                    return Err((
                        2,
                        format!(
                            "模型不存在：{agent_id}/{model_id}；该 agent 下可选：{}",
                            model_candidates(agent)
                        ),
                    ));
                }
                [one] => (*one).clone(),
                _ => {
                    let providers: Vec<&str> =
                        candidates.iter().map(|model| model.provider.as_str()).collect();
                    return Err((
                        2,
                        format!(
                            "该模型挂在多个 provider 下：{}；用 --provider 指定",
                            providers.join("、")
                        ),
                    ));
                }
            };
            // TASK-48：未发现的模型不发请求，直接退出 1 并给说明；判在
            // 「原生无需冒烟」之前，原生模型带标记同样适用。与引擎一致直接
            // 读标记（fix-48-r1 R7）：agent 未安装、验证失败都不改变判定。
            if resources::is_missing_from_detection(&target) {
                return Ok(SmokeOutcome {
                    exit: 1,
                    value: resources_value_filtered(&store, Some(&agent_id)),
                    messages: vec![resources::NOT_DETECTED_NOTE.to_string()],
                });
            }
            if resources::is_native_model(&target.agent)
                && !resources::native_smoke_dispatches(&target.agent)
            {
                return Ok(SmokeOutcome {
                    exit: 0,
                    value: resources_value_filtered(&store, Some(&agent_id)),
                    messages: vec!["原生模型无需冒烟".into()],
                });
            }
            let key = ResourceModelKey {
                agent_id: agent_id.clone(),
                provider: target.provider.clone(),
                id: target.id.clone(),
            };
            let smoked = (ops.smoke_one)(&store_file(root), key).map_err(|message| (3, message))?;
            let failure = smoked
                .agents
                .iter()
                .find(|agent| agent.id == agent_id)
                .and_then(|agent| {
                    agent.models.iter().find(|model| {
                        model.id == target.id && model.provider == target.provider
                    })
                })
                .filter(|model| {
                    model.verification == resources::VERIFICATION_FAILED
                        // 原生模型的测试结果不改验证档（恒为 native），失败看冒烟原因
                        || (resources::native_smoke_dispatches(&model.agent)
                            && !model.smoke_failure.is_empty())
                })
                .map(|model| model.smoke_failure.clone());
            match failure {
                Some(reason) => Ok(SmokeOutcome {
                    exit: 1,
                    value: resources_value_filtered(&smoked, Some(&agent_id)),
                    messages: vec![format!("冒烟失败：{reason}")],
                }),
                None => Ok(SmokeOutcome {
                    exit: 0,
                    value: resources_value_filtered(&smoked, Some(&agent_id)),
                    messages: vec!["冒烟完成".into()],
                }),
            }
        }
        None => {
            // R1-7：agent 下一个模型都没有时不发请求，退出 0。
            if agent.models.is_empty() {
                return Ok(SmokeOutcome {
                    exit: 0,
                    value: resources_value_filtered(&store, Some(&agent_id)),
                    messages: vec!["该 agent 下没有模型".into()],
                });
            }
            let operation = (ops.smoke_agent_third_party)(&store_file(root), &agent_id)
                .map_err(|message| (3, message))?;
            let fresh = (ops.read_resources)(&store_file(root)).map_err(|message| (3, message))?;
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
            let (exit, final_line) = if third_party == 0 {
                (0, "原生模型无需冒烟".to_string())
            } else if operation.errors.is_empty() {
                (0, "冒烟完成".to_string())
            } else {
                (1, format!("冒烟完成；{} 条失败", operation.errors.len()))
            };
            // TASK-48：跳过的未发现模型逐行说明；不算失败，不影响退出码。
            let mut messages = operation.skipped.clone();
            messages.extend(operation.errors.clone());
            messages.push(final_line);
            Ok(SmokeOutcome {
                exit,
                value: resources_value_filtered(&fresh, Some(&agent_id)),
                messages,
            })
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
    let model_arg = option_value(args, "--model");
    let provider_arg = option_value(args, "--provider");
    match resources_smoke_at(root, &agent_arg, model_arg.as_deref(), provider_arg.as_deref(), ops) {
        Ok(outcome) => {
            let _ = writeln!(stdout, "{}", serde_json::to_string(&outcome.value).unwrap_or_else(|_| "{}".into()));
            for message in &outcome.messages {
                let _ = writeln!(stderr, "{message}");
            }
            outcome.exit
        }
        Err((code, message)) => {
            let _ = writeln!(stderr, "{message}");
            code
        }
    }
}

/// `resources detect` 的实现体：命令行与窗口探测共用（fix-46-r1 R3，照
/// `resources_smoke_at` 的分法）。cc-switch 发现提示的路径取派发设置里的
/// ccSwitchPath（与 MANUAL 同一口径）。返回 `aicross resources` 形状的输出
/// 与 stderr 说明行（单个 agent 探不到或事实源读不了是探测结果不是失败，
/// 连同最后的「已探测 N 个 agent」都在 messages 里）；探测库读不了返回
/// Err((3, 说明))。
struct DetectOutcome {
    value: Value,
    messages: Vec<String>,
}

fn resources_detect_at(root: &Path, ops: &ResourcesOps) -> Result<DetectOutcome, (i32, String)> {
    let cc_switch = crate::engine::load_settings(root).cc_switch_path;
    let cc_switch = (!cc_switch.trim().is_empty()).then_some(cc_switch);
    let operation = (ops.detect_all)(&store_file(root), cc_switch.as_deref())
        .map_err(|message| (3, message))?;
    Ok(detect_outcome(operation))
}

/// 探测结果的公共尾部：stderr 说明行与资源输出。
fn detect_outcome(operation: ResourceOperation) -> DetectOutcome {
    let mut messages = operation.errors.clone();
    messages.push(format!("已探测 {} 个 agent", operation.store.agents.len()));
    DetectOutcome {
        value: resources_value(&operation.store),
        messages,
    }
}

/// 窗口资源探测（带进度回调，走查 #3）：与 `resources detect` 同一实现体。
/// 返回 `aicross resources` 形状的输出，另加 `messages` 字段（stderr 那些
/// 说明行，空时也给 `[]`）；每个 agent 探测完回调一次 `(agent, done, total)`，
/// 窗口据此发进度事件。
pub(crate) fn resources_detect_command_with_progress(
    root: &Path,
    on_done: &(dyn Fn(&str, usize, usize) + Sync),
) -> Result<Value, String> {
    let cc_switch = crate::engine::load_settings(root).cc_switch_path;
    let cc_switch = (!cc_switch.trim().is_empty()).then_some(cc_switch);
    let operation = resources::detect_all_resources_with_progress(
        &store_file(root),
        cc_switch.as_deref(),
        on_done,
    )?;
    Ok(outcome_value_with_messages(detect_outcome(operation)))
}

/// 返回值：在资源输出对象上追加 `messages`（stderr 那些说明行）。
fn value_with_messages(value: Value, messages: Vec<String>) -> Value {
    let mut value = value;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "messages".into(),
            Value::Array(messages.into_iter().map(Value::String).collect()),
        );
    }
    value
}

/// 窗口探测的返回值：在资源输出对象上追加 `messages`。
fn outcome_value_with_messages(outcome: DetectOutcome) -> Value {
    value_with_messages(outcome.value, outcome.messages)
}

/// `resources prune [--agent <a>]` 的实现体：命令行与窗口共用（照
/// `resources_detect_at` 的分法）。删除最近一次探测未发现的模型（用户申报
/// 的不带标记，天然保留；被 modes.json 人选或 roles.json 角色引用的照样
/// 删除、另列 `referenced`）。stdout 是清理后的资源（同 `aicross resources`）
/// 外加 `removed` 与 `referenced` 两个键列表；stderr 说明行随 messages
/// 返回。资源库读不了（3）与未知 agent（2）返回 Err。
struct PruneOutcome {
    value: Value,
    messages: Vec<String>,
}

fn resources_prune_at(
    root: &Path,
    agent_arg: Option<&str>,
    ops: &ResourcesOps,
) -> Result<PruneOutcome, (i32, String)> {
    let store = (ops.read_resources)(&store_file(root)).map_err(|message| (3, message))?;
    let agent_id = match agent_arg {
        Some(arg) => match store.agents.iter().find(|agent| agent.id.eq_ignore_ascii_case(arg.trim())) {
            Some(agent) => Some(agent.id.clone()),
            None => {
                let ids: Vec<&str> = store.agents.iter().map(|agent| agent.id.as_str()).collect();
                return Err((2, format!("未知 agent：{arg}；可选：{}", join_or_none(&ids))));
            }
        },
        None => None,
    };
    let referenced = referenced_model_targets(root);
    let outcome = (ops.prune_missing)(&store_file(root), agent_id.as_deref(), &referenced)
        .map_err(|message| (3, message))?;
    let mut value = resources_value(&outcome.store);
    if let Some(object) = value.as_object_mut() {
        object.insert("removed".into(), json!(outcome.removed));
        object.insert("referenced".into(), json!(outcome.referenced));
    }
    let mut messages = vec![format!("已清理 {} 个未发现的模型", outcome.removed.len())];
    if !outcome.referenced.is_empty() {
        messages.push(format!(
            "其中 {} 个仍被模式人选或角色引用：{}",
            outcome.referenced.len(),
            outcome.referenced.join("、")
        ));
    }
    Ok(PruneOutcome { value, messages })
}

/// modes.json 人选（含备选）与 roles.json 角色指向的模型清单，prune 用它
/// 区分「删了但还被引用」的条目。agent 或 model 为空（没设）的指向不算。
fn referenced_model_targets(root: &Path) -> Vec<resources::ModelReference> {
    let mut targets = Vec::new();
    let modes = crate::orchestration::modes_from(root);
    for section in modes.hosts.values() {
        for stages in section.seats.values() {
            for row in stages {
                for seat in row.iter().flatten() {
                    push_model_target(
                        &mut targets,
                        &seat.agent,
                        seat.provider.as_deref(),
                        &seat.model,
                    );
                    if let Some(fallback) = &seat.fallback {
                        push_model_target(
                            &mut targets,
                            &fallback.agent,
                            fallback.provider.as_deref(),
                            &fallback.model,
                        );
                    }
                }
            }
        }
    }
    let roles = crate::orchestration::roles_from(root);
    for role in [&roles.planning, &roles.execution, &roles.acceptance] {
        push_model_target(&mut targets, &role.agent, Some(&role.provider), &role.model);
    }
    targets
}

fn push_model_target(
    targets: &mut Vec<resources::ModelReference>,
    agent: &str,
    provider: Option<&str>,
    model: &str,
) {
    if agent.trim().is_empty() || model.trim().is_empty() {
        return;
    }
    targets.push((
        agent.trim().to_string(),
        provider.unwrap_or_default().to_string(),
        model.trim().to_string(),
    ));
}

/// 窗口资源清理：与 `resources prune` 同一实现体。返回 `aicross resources`
/// 形状的输出外加 `removed` / `referenced` 与 `messages` 字段；资源库读
/// 不了或 agent 不认识时回 Err(stderr 那行说明)。
pub(crate) fn resources_prune_command_at(root: &Path, agent: Option<&str>) -> Result<Value, String> {
    let outcome =
        resources_prune_at(root, agent, &ResourcesOps::production()).map_err(|(_, message)| message)?;
    Ok(value_with_messages(outcome.value, outcome.messages))
}

/// `resources prune [--agent <a>]`：stdout 写清理后的资源 JSON，stderr 逐行
/// 给说明；参数错退出 2，资源库读不了退出 3。
fn run_resources_prune(
    root: &Path,
    args: &[String],
    ops: &ResourcesOps,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    if let Some(message) = unknown_option_error(args, 3, &["--agent"]) {
        let _ = writeln!(stderr, "{message}");
        return 2;
    }
    let agent_arg = option_value(args, "--agent");
    match resources_prune_at(root, agent_arg.as_deref(), ops) {
        Ok(outcome) => {
            let _ = writeln!(stdout, "{}", serde_json::to_string(&outcome.value).unwrap_or_else(|_| "{}".into()));
            for message in &outcome.messages {
                let _ = writeln!(stderr, "{message}");
            }
            0
        }
        Err((code, message)) => {
            let _ = writeln!(stderr, "{message}");
            code
        }
    }
}

/// `resources set-path`：记下或清除某个 agent 的指定路径（TASK-66）。只认
/// 支持清单里的 agent；给的路径必须是已存在的文件。返回该 agent 的条目与
/// stderr 说明行；参数错退出 2，资源库写不了退出 3。
fn resources_set_path_at(root: &Path, agent_arg: &str, program: Option<&str>) -> Result<(Value, String), (i32, String)> {
    let Some(entry) = crate::agent_catalog::find(agent_arg.trim()) else {
        let ids: Vec<&str> = crate::agent_catalog::SUPPORTED_AGENTS.iter().map(|entry| entry.id).collect();
        return Err((2, format!("未知 agent：{agent_arg}；可选：{}", join_or_none(&ids))));
    };
    let program = program.map(str::trim).filter(|program| !program.is_empty()).map(Path::new);
    if let Some(program) = program {
        if !program.is_file() {
            return Err((2, format!("文件不存在：{}", program.display())));
        }
    }
    let store = resources::set_custom_path_at(&store_file(root), entry.id, program).map_err(|message| (3, message))?;
    let message = match program {
        Some(program) => format!("已指定 {} 的路径：{}；重新探测后生效", entry.id, program.display()),
        None => format!("已清除 {} 的指定路径；重新探测后生效", entry.id),
    };
    Ok((resources_value_filtered(&store, Some(entry.id)), message))
}

/// 窗口指定路径：与 `resources set-path` 同一实现体，失败回 Err(那一行说明)。
pub(crate) fn resources_set_path_command_at(root: &Path, agent: &str, program: Option<&str>) -> Result<Value, String> {
    let (value, message) = resources_set_path_at(root, agent, program).map_err(|(_, message)| message)?;
    Ok(value_with_messages(value, vec![message]))
}

/// 窗口冒烟的返回值映射：参数错与资源库读不了回 Err(那一行说明)，
/// 冒烟失败（命令行退出 1）回 Err(stderr 那几行说明)，成功才回 Ok。
fn smoke_command_value(outcome: Result<SmokeOutcome, (i32, String)>) -> Result<Value, String> {
    let outcome = outcome.map_err(|(_, message)| message)?;
    if outcome.exit != 0 {
        return Err(outcome.messages.join("\n"));
    }
    Ok(outcome.value)
}

/// 窗口冒烟：与 `resources smoke` 同一实现体。返回 `aicross resources`
/// 形状的输出；单个模型冒烟失败回 Err(stderr 那几行说明)，成功看输出里
/// 的 verification 与 smoke_failure。
pub(crate) fn resources_smoke_command_at(
    root: &Path,
    agent: &str,
    model: Option<&str>,
    provider: Option<&str>,
) -> Result<Value, String> {
    smoke_command_value(resources_smoke_at(
        root,
        agent,
        model,
        provider,
        &ResourcesOps::production(),
    ))
}

const ROLE_KEYS: [&str; 3] = ["planning", "execution", "acceptance"];

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

/// ACP 适配器的检查与安装；生产走 acp_setup，测试注入假实现，不在单测里真跑 npm。
struct AcpOps {
    install: Box<dyn Fn() -> Vec<AdapterReport>>,
    status: Box<dyn Fn() -> Vec<AdapterReport>>,
}

impl AcpOps {
    fn production() -> Self {
        AcpOps {
            install: Box::new(acp_setup::install_missing_blocking),
            status: Box::new(acp_setup::adapter_status),
        }
    }
}

fn run_config(root: &Path, args: &[String], acp: &AcpOps, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    // 家目录注入是为了测试不碰真实家目录；只有 set-host-vendor 与
    // recommend（厂商推定）会读它。claude 的进程环境变量同样注入，
    // 不在库内读真实环境（fix-50-r2）。
    match crate::paths::home() {
        Ok(home) => {
            let env_base_url = std::env::var("ANTHROPIC_BASE_URL").ok();
            run_config_at(
                root,
                &home,
                env_base_url.as_deref(),
                args,
                acp,
                stdout,
                stderr,
            )
        }
        Err(message) => {
            let _ = writeln!(stderr, "{message}");
            3
        }
    }
}

/// `run_config` 的实现体（家目录与 claude 环境变量注入）。
fn run_config_at(
    root: &Path,
    home: &Path,
    claude_base_url_env: Option<&str>,
    args: &[String],
    acp: &AcpOps,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    match args.get(2).map(String::as_str) {
        Some("show") => {
            // fix-50-r1：不再先拦位置参数——那会把 --host 当多余参数拦下；
            // 只认 --host 带值，位置参数由 unknown_option_error 拒绝。
            if let Some(message) = unknown_option_error(args, 3, &["--host"]) {
                let _ = writeln!(stderr, "{message}");
                return 2;
            }
            let host = match resolve_host_or_err(root, option_value(args, "--host").as_deref()) {
                Ok(host) => host,
                Err((code, message)) => {
                    let _ = writeln!(stderr, "{message}");
                    return code;
                }
            };
            // R1-2：roles.json 读不了或损坏时 stdout 仍输出默认值，stderr 提示一行。
            if let Err(message) = crate::orchestration::roles_from_strict(root) {
                let _ = writeln!(stderr, "{message}，已按默认值显示");
            }
            let value = config_show_value(root, host.as_deref(), &(acp.status)());
            let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
            let _ = writeln!(stderr, "已读取配置");
            0
        }
        Some("set-role") => run_config_set_role(root, args, stdout, stderr),
        Some("set") => run_config_set(root, args, acp, stdout, stderr),
        Some("set-default-mode") => run_config_set_default_mode(root, args, stdout, stderr),
        Some("set-count") => run_config_set_count(root, args, stdout, stderr),
        Some("set-focus") => run_config_set_focus(root, args, stdout, stderr),
        Some("set-seat") => run_config_set_seat(root, args, stdout, stderr),
        Some("clear-seat") => run_config_clear_seat(root, args, stdout, stderr),
        Some("set-host-vendor") => {
            run_config_set_host_vendor(root, home, claude_base_url_env, args, stdout, stderr)
        }
        Some("recommend") => {
            run_config_recommend(root, home, claude_base_url_env, args, stdout, stderr)
        }
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
    let (agent_id, chosen) =
        match resolve_resource_model(root, &agent_arg, &model_arg, provider_arg.as_deref()) {
            Ok(found) => found,
            Err((code, message)) => {
                let _ = writeln!(stderr, "{message}");
                return code;
            }
        };
    // fix-61-r1 2：planning / execution 的角色默认补的是出方案、拆分、执行
    // 这些共享位置，只接盲的 agent 不能设成这两个角色（否则角色默认会绕过
    // set-seat 的限制）；acceptance 补的验收 / 验证位是盲位置，照常允许。
    if role != "acceptance" {
        if let Some(reason) = crate::agent_catalog::blind_only_reason(&agent_id) {
            let _ = writeln!(
                stderr,
                "{reason}：{role} 角色补的是共享位置，{agent_id} 只能设为 acceptance 或放在盲位置"
            );
            return 2;
        }
    }
    // R1-2：roles_from 对损坏文件静默回默认值，整份写回会丢其余角色；
    // 严格读取遇损坏或读不了拒写。
    let mut roles = match crate::orchestration::roles_from_strict(root) {
        Ok(roles) => roles,
        Err(message) => {
            let _ = writeln!(
                stderr,
                "{message}；已停止写入，修复或删除该文件后重试"
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
        agent: agent_id,
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

// ── TASK-46：模式人选与默认模式（set-default-mode / set-seat / clear-seat）──

/// 在资源库里按 agent + model（给了 provider 时一并匹配）找模型；
/// set-role 与 set-seat 共用。agent 名大小写不敏感，返回规范 id 与模型。
/// 错误带退出码：资源库读不了 3，agent 或模型不存在、多 provider 未指定、
/// 人选或备选是未发现的模型（TASK-48）都是 2。
fn resolve_resource_model(
    root: &Path,
    agent_arg: &str,
    model_arg: &str,
    provider_arg: Option<&str>,
) -> Result<(String, ResourceModel), (i32, String)> {
    let store = match resources::get_resources_at(&store_file(root)) {
        Ok(store) => store,
        Err(message) => return Err((3, message)),
    };
    let Some(agent) = store
        .agents
        .iter()
        .find(|agent| agent.id.eq_ignore_ascii_case(agent_arg.trim()))
    else {
        let ids: Vec<&str> = store.agents.iter().map(|agent| agent.id.as_str()).collect();
        return Err((
            2,
            format!(
                "未知 agent：{agent_arg}；资源库中可选：{}",
                join_or_none(&ids)
            ),
        ));
    };
    let candidates: Vec<_> = agent
        .models
        .iter()
        .filter(|model| model.id == model_arg)
        .filter(|model| provider_arg.map_or(true, |value| model.provider == value))
        .collect();
    let chosen = match candidates.as_slice() {
        [] => Err((
            2,
            format!(
                "模型不存在：{}/{model_arg}；该 agent 下可选：{}",
                agent.id,
                model_candidates(agent)
            ),
        )),
        [one] => {
            // TASK-48：未发现的模型选了也派不出，选人选时当面拒掉，候选
            // 列表里去掉它们，免得再撞一次。与引擎一致直接读标记
            // （fix-48-r1 R7）：agent 未安装不改变判定。
            if resources::is_missing_from_detection(one) {
                Err((
                    2,
                    format!(
                        "{}/{}：{}；该 agent 下可选：{}",
                        agent.id,
                        model_arg,
                        resources::NOT_DETECTED_NOTE,
                        model_candidates_filtered(
                            agent,
                            |model| !resources::is_missing_from_detection(model)
                        )
                    ),
                ))
            } else {
                Ok((*one).clone())
            }
        }
        _ => {
            let providers: Vec<&str> =
                candidates.iter().map(|model| model.provider.as_str()).collect();
            Err((
                2,
                format!(
                    "该模型挂在多个 provider 下：{}；用 --provider 指定",
                    providers.join("、")
                ),
            ))
        }
    };
    chosen.map(|model| (agent.id.clone(), model))
}

/// `config show` 的 `defaultMode`（实际生效的预设 id）与 `modes` 两项；
/// set-default-mode / set-seat / clear-seat / recommend 写入后的输出同源。
/// 宽松读取：modes.json 缺失、损坏或版本不认都按空配置处理；资源库读不了
/// 时 `effective.state` 按「不在资源库」显示，不让整个 show 失败。
/// host 决定看哪份主 agent 配置（None = 无配置）。
pub(crate) fn modes_section(root: &Path, host: Option<&str>) -> (String, Vec<Value>) {
    let store = resource_store_for_state(root);
    let presets = crate::orchestration::raw_presets_from(root);
    let roles = crate::orchestration::roles_from(root);
    let modes = crate::orchestration::modes_from(root);
    let section = host.and_then(|host| crate::orchestration::host_section(&modes, host));
    let default_mode = crate::engine::default_preset_id(section, &presets.presets);
    let list = presets
        .presets
        .iter()
        .map(|preset| mode_entry(preset, &roles, section, &default_mode, store.as_ref()))
        .collect();
    (default_mode, list)
}

/// `effective.state` 用的资源库；读不了（缺失、损坏）返回 None，展示层
/// 不因此失败。
fn resource_store_for_state(root: &Path) -> Option<ResourceStore> {
    resources::get_resources_at(&store_file(root)).ok()
}

/// 人选在资源库里的状态（TASK-48：`config show` 的 `effective.state`）。
/// 查找用 `find_target_model`：给了 provider 只认精确匹配，没有就
/// 「不在资源库」，不再退回 agent+id（fix-48-r1 R1）；资源库读不了或
/// 找不到这条模型时同为「不在资源库」。
fn resource_state_of(store: Option<&ResourceStore>, target: &Target) -> String {
    const NOT_IN_STORE: &str = "不在资源库";
    let Some(store) = store else {
        return NOT_IN_STORE.to_string();
    };
    let Some(agent) = store
        .agents
        .iter()
        .find(|agent| agent.id.eq_ignore_ascii_case(target.agent.trim()))
    else {
        return NOT_IN_STORE.to_string();
    };
    let Some(model) = resources::find_target_model(
        &agent.models,
        &agent.id,
        target.provider.as_deref().unwrap_or_default(),
        &target.model,
    ) else {
        return NOT_IN_STORE.to_string();
    };
    let (state, _) = resources::model_state(
        model,
        agent.detected,
        chrono::Local::now().timestamp_millis(),
    );
    state.to_string()
}

/// 生效人选与它的备选是否违反位置硬规则（只接盲的 agent 落在非盲位置）。
/// 返回给 `config show` 的说明；合规为 None。
fn seat_invalid_reason(target: &Target) -> Option<String> {
    let fallback_agent = target.fallback.as_ref().map(|fallback| fallback.agent.as_str());
    std::iter::once(target.agent.as_str())
        .chain(fallback_agent)
        .find_map(|agent| {
            crate::agent_catalog::seat_violation(agent, &target.visibility).map(|reason| {
                format!("{reason}：{agent} 不能放在可见性为「{}」的位置", target.visibility)
            })
        })
}

/// `modes` 数组的一项：预设信息加每个位置的 seat（该主 agent 配置里
/// modes.json 原样）、effective（按补人顺序填好的人选，附 `state`：该人选
/// 在资源库里的状态，TASK-48）与 source（预设 / 位置 / 角色 / 未指定）；
/// 评审模式各位置另附 `focus`（生效视角，TASK-59 起按该主 agent 的配置取，
/// 缺省回默认表；其余模式为空串）与 `focusOptions`（可选的内置视角）。
/// 内置模式按该主 agent 的 counts 取实际形状（TASK-56），另附 `count`：
/// 可调模式为 `{ role, value, min, max, default }`，不可调为 null。
fn mode_entry(
    preset: &Preset,
    roles: &RoleSettings,
    section: Option<&crate::orchestration::HostModes>,
    default_mode: &str,
    store: Option<&ResourceStore>,
) -> Value {
    let shaped = crate::orchestration::shaped_preset(preset, section);
    let seat_stages = section.and_then(|section| section.seats.get(&preset.id));
    let (filled, sources, unfilled_reasons) =
        crate::engine::fill_seats_explained(&shaped, roles, seat_stages);
    // 走查 #7：评审模式各位置的视角按扁平序取生效值（TASK-59 起主 agent
    // 配置可改，缺省回安全 / 性能 / 需求覆盖的默认表；超出为空串）；取值
    // 与盖章文本共用 engine 的一张表；其余模式一律空串。
    let mut focus_flat = (preset.id == "builtin-jury").then(|| {
        shaped
            .stages
            .iter()
            .flat_map(|stage| stage.nodes.iter())
            .enumerate()
            .map(|(index, _)| crate::engine::jury_focus_effective(section, index))
            .collect::<Vec<_>>()
            .into_iter()
    });
    let focus: Vec<Vec<String>> = filled
        .stages
        .iter()
        .map(|stage| {
            (0..stage.nodes.len())
                .map(|_| {
                    focus_flat
                        .as_mut()
                        .and_then(Iterator::next)
                        .unwrap_or_default()
                })
                .collect()
        })
        .collect();
    let stages: Vec<Vec<Value>> = filled
        .stages
        .iter()
        .enumerate()
        .map(|(i, stage)| {
            stage
                .nodes
                .iter()
                .enumerate()
                .map(|(j, target)| {
                    let seat = seat_stages
                        .and_then(|stages| stages.get(i))
                        .and_then(|nodes| nodes.get(j))
                        .and_then(|seat| seat.as_ref());
                    let source = sources[i][j];
                    let effective = if source == "未指定" {
                        Value::Null
                    } else {
                        json!({
                            "agent": target.agent,
                            "provider": target.provider,
                            "model": target.model,
                            "tier": target.tier.clone().unwrap_or_default(),
                            // fix-61-r1 5：实际生效值（agy 模型自带档位时由模型决定）
                            "thinking": crate::channels::effective_thinking(
                                &target.agent,
                                &target.model,
                                &target.thinking,
                            ),
                            "fallback": target.fallback,
                            "state": resource_state_of(store, target),
                            // fix-61-r1 2：生效人选（含角色默认）或备选违反位置硬规则
                            // 时的原因；合规为 null。
                            "invalid": seat_invalid_reason(target),
                        })
                    };
                    // 预设本身给这个位置定的思考强度（人选未覆盖前的值），
                    // 复制为新模式时作为初值。
                    let preset_thinking = shaped
                        .stages
                        .get(i)
                        .and_then(|stage| stage.nodes.get(j))
                        .map(|node| node.thinking.clone())
                        .unwrap_or_default();
                    let mut node = json!({
                        "role": target.role,
                        "visibility": target.visibility,
                        "write": target.write,
                        "focus": focus.get(i).and_then(|row| row.get(j)).cloned().unwrap_or_default(),
                        "presetThinking": preset_thinking,
                        "seat": seat,
                        "effective": effective,
                        "source": source,
                        // fix-fullcheck A5：未指定是因为角色默认违反位置硬规则
                        // 而未采用时的原因；其余为 null。
                        "unfilledReason": unfilled_reasons[i][j],
                    });
                    // 评审模式的位置给出可选的内置视角（TASK-59）；自定义
                    // 文字不进清单，由用户直接输入。
                    if preset.id == "builtin-jury" {
                        node["focusOptions"] = json!(crate::engine::JURY_FOCUS_OPTIONS);
                    }
                    node
                })
                .collect()
        })
        .collect();
    let count = match crate::orchestration::count_spec(&preset.id) {
        None => Value::Null,
        Some(spec) => {
            let value = section
                .and_then(|section| section.counts.get(preset.id.as_str()))
                .copied()
                .map(|value| value as usize)
                .filter(|value| (spec.min..=spec.max).contains(value))
                .unwrap_or(spec.default);
            json!({
                "role": spec.role,
                "value": value,
                "min": spec.min,
                "max": spec.max,
                "default": spec.default,
            })
        }
    };
    // 评审模式的介绍随各位置生效视角走，改了视角不再显示默认三项。
    let description = if preset.id == "builtin-jury" {
        jury_description(&focus).unwrap_or_else(|| preset.description.clone())
    } else {
        preset.description.clone()
    };
    json!({
        "id": preset.id,
        "name": preset.name,
        "description": description,
        "builtin": preset.builtin,
        "default": preset.id == default_mode,
        "count": count,
        "stages": stages,
    })
}

/// 评审模式介绍：各位置生效视角按出现顺序去重后拼成一句；全空时为 None。
fn jury_description(focus: &[Vec<String>]) -> Option<String> {
    let mut names: Vec<&str> = Vec::new();
    for name in focus.iter().flatten().map(String::as_str) {
        if !name.is_empty() && !names.contains(&name) {
            names.push(name);
        }
    }
    (!names.is_empty()).then(|| format!("多个模型分别从{}角度审查", names.join("、")))
}

/// `config show` 的 stdout JSON；窗口 `config_snapshot` 的 config 段同此
/// 结构。roles 与 modes 按宽松读取（损坏按默认值 / 空配置显示）。
pub(crate) fn config_show_value(root: &Path, host: Option<&str>, acp: &[AdapterReport]) -> Value {
    let (default_mode, modes) = modes_section(root, host);
    json!({
        "roles": crate::orchestration::roles_from(root),
        "dispatch": crate::engine::load_settings(root),
        "acp": acp,
        "defaultMode": default_mode,
        "modes": modes,
    })
}

/// 窗口 `config_snapshot` 的实现体：`{ resources, config }`，两段分别同
/// `aicross resources` 与 `aicross config show --host` 的 stdout 结构。窗口命令
/// 直接调这里，不拉起子进程；acp 状态由调用方查好传入（生产用
/// `acp_setup::adapter_status()`，它会拉起 where.exe，须在阻塞线程上跑）。
/// host 按命令行同一套规则解析（fix-50-r1）；host 为 null 且一份配置都
/// 没有时返回资源与空配置，不报错（资源页在首次探测前就要能读到快照）；
/// 多于一份配置且未给 host 时与命令行一致报错。
pub(crate) fn config_snapshot_at(
    root: &Path,
    host: Option<&str>,
    acp: &[AdapterReport],
) -> Result<Value, String> {
    let resolved = resolve_host_or_err(root, host).map_err(|(_, message)| message)?;
    let store = resources::get_resources_at(&store_file(root))?;
    Ok(json!({
        "resources": resources_value(&store),
        "config": config_show_value(root, resolved.as_deref(), acp),
    }))
}

/// `config set-default-mode <模式>`：别名与预设 id 都认；不存在退出 2 并列出
/// 可选的 id 与名称；modes.json 损坏退出 1、不覆盖。host 为解析出的主
/// agent（写命令必须有具体 host）。
pub(crate) fn config_set_default_mode_at(
    root: &Path,
    host: &str,
    mode: &str,
) -> Result<Value, (i32, String)> {
    {
        let _guard = modes_write_lock(root)?;
        // 预设在锁内读（fix-fullcheck A4）：与 preset save / remove 串行。
        let (wanted, _) = find_preset(root, mode)?;
        let mut modes = strict_modes_for_write(root)?;
        crate::orchestration::host_section_mut(&mut modes, host).default_mode = wanted;
        crate::orchestration::save_modes_in(root, &modes).map_err(|message| (1, message))?;
    }
    let (default_mode, list) = modes_section(root, Some(host));
    Ok(json!({ "defaultMode": default_mode, "modes": list }))
}

/// 按模式名找预设：别名与 id 都认；找不到退出 2 并列出可选的 id 与名称。
/// 写 modes.json 的命令在 `modes_write_lock` 内调它（fix-fullcheck A4）：
/// 预设与 modes 在同一把锁内读，校验与写入用的是同一份快照，不会按
/// preset save / remove 之前的旧形状写回人选。
fn find_preset(root: &Path, mode: &str) -> Result<(String, Preset), (i32, String)> {
    let presets = crate::orchestration::raw_presets_from(root);
    let wanted = resolve_preset_name(mode);
    match presets.presets.iter().find(|preset| preset.id == wanted) {
        Some(found) => Ok((wanted, found.clone())),
        None => Err((2, unknown_mode_message(mode, &presets.presets))),
    }
}

fn run_config_set_default_mode(
    root: &Path,
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    if let Some(message) = unknown_option_error_allow_positionals(args, 3, &["--host"]) {
        let _ = writeln!(stderr, "{message}");
        return 2;
    }
    let positionals = positional_values(args, 3, &["--host"]);
    let Some(mode) = positionals.first().map(|value| value.to_string()) else {
        let _ = writeln!(stderr, "缺少模式");
        return 2;
    };
    if positionals.len() > 1 {
        let _ = writeln!(stderr, "不认识的参数：{}", positionals[1]);
        return 2;
    }
    let host = match resolve_host_for_write(root, option_value(args, "--host").as_deref()) {
        Ok(host) => host,
        Err((code, message)) => {
            let _ = writeln!(stderr, "{message}");
            return code;
        }
    };
    match config_set_default_mode_at(root, &host, &mode) {
        Ok(value) => {
            let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
            let _ = writeln!(stderr, "已写入默认模式");
            0
        }
        Err((code, message)) => {
            let _ = writeln!(stderr, "{message}");
            code
        }
    }
}

pub(crate) fn unknown_mode_message(mode: &str, presets: &[Preset]) -> String {
    let names: Vec<String> = presets
        .iter()
        .map(|preset| format!("{}（{}）", preset.id, preset.name))
        .collect();
    format!("模式不存在：{mode}；可选：{}", names.join("、"))
}

/// `config set-count <模式> <数量>` 的实现体；窗口命令直接调这里。
/// 校验顺序：模式不存在 → 不可调 / 越界（都退出 2，信息给出范围）。写入
/// counts 后按调整后的形状整理该模式已有人选：减少时从末尾去掉，增加时
/// 补 null。输出该模式那一项（同 `config show` 的 `modes[i]`）。
pub(crate) fn config_set_count_at(
    root: &Path,
    host: &str,
    mode: &str,
    count: u32,
) -> Result<Value, (i32, String)> {
    let (wanted, found) = find_preset(root, mode)?;
    let name = found.name.clone();
    let Some(spec) = crate::orchestration::count_spec(&wanted) else {
        return Err((2, format!("{name} 的位置数量不可调")));
    };
    let count = count as usize;
    if !(spec.min..=spec.max).contains(&count) {
        return Err((
            2,
            format!(
                "{} 的「{}」位置数量只认 {}–{}：收到 {}",
                name, spec.role, spec.min, spec.max, count
            ),
        ));
    }
    {
        let _guard = modes_write_lock(root)?;
        // 预设在锁内重读（fix-fullcheck A4），整理人选按锁内的形状。
        let (_, found) = find_preset(root, mode)?;
        let mut modes = strict_modes_for_write(root)?;
        let section = crate::orchestration::host_section_mut(&mut modes, host);
        section.counts.insert(wanted.clone(), count as u32);
        // 已有人选按调整后的形状整理：减少时裁掉超出的，增加时补空位。
        if let Some(stages) = section.seats.get_mut(&wanted) {
            let shaped = crate::orchestration::shape_builtin_preset(&found, count);
            crate::orchestration::shape_seats_to_preset(stages, &shaped);
        }
        crate::orchestration::save_modes_in(root, &modes).map_err(|message| (1, message))?;
    }
    seat_entry_after_write(root, host, &wanted)
}

fn run_config_set_count(
    root: &Path,
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    if let Some(message) = unknown_option_error_allow_positionals(args, 3, &["--host"]) {
        let _ = writeln!(stderr, "{message}");
        return 2;
    }
    let positionals = positional_values(args, 3, &["--host"]);
    let Some(mode) = positionals.first().map(|value| value.to_string()) else {
        let _ = writeln!(stderr, "缺少模式");
        return 2;
    };
    let Some(count_arg) = positionals.get(1).map(|value| value.to_string()) else {
        let _ = writeln!(stderr, "缺少数量");
        return 2;
    };
    if positionals.len() > 2 {
        let _ = writeln!(stderr, "不认识的参数：{}", positionals[2]);
        return 2;
    }
    let host = match resolve_host_for_write(root, option_value(args, "--host").as_deref()) {
        Ok(host) => host,
        Err((code, message)) => {
            let _ = writeln!(stderr, "{message}");
            return code;
        }
    };
    let Ok(count) = count_arg.parse::<u32>() else {
        let _ = writeln!(stderr, "数量必须是 0 或正整数：{count_arg}");
        return 2;
    };
    match config_set_count_at(root, &host, &mode, count) {
        Ok(value) => {
            let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
            let _ = writeln!(stderr, "已写入位置数量");
            0
        }
        Err((code, message)) => {
            let _ = writeln!(stderr, "{message}");
            code
        }
    }
}

/// 评审视角文字校验（TASK-59）：规则在 `engine::normalize_focus_text`，
/// 与读出口共用；不合格退出 2。
fn validate_focus_text(focus: &str) -> Result<String, (i32, String)> {
    crate::engine::normalize_focus_text(focus).map_err(|message| (2, message))
}

/// `config set-focus --host <h> --mode <m> --index <n> --focus <文字>` 的
/// 实现体；窗口命令直接调这里。`index` 从 1 起，是该模式里带视角的位置按
/// 阶段先后的序号（按该主 agent 调整后的形状算，与 set-count 的范围同
/// 源）；focus 为空串恢复该位置的默认。校验顺序：视角文字 → 模式不存在
/// → 只认评审模式（其它模式退出 2）→ 位置序号超出（退出 2）。读改写在
/// `modes_write_lock` 内；数量调小时多出的视角项不删，调大后继续生效。
/// 输出该模式那一项（同 `config show` 的 `modes[i]`）。
pub(crate) fn config_set_focus_at(
    root: &Path,
    host: &str,
    mode: &str,
    index: usize,
    focus: &str,
) -> Result<Value, (i32, String)> {
    let focus = validate_focus_text(focus)?;
    let _guard = modes_write_lock(root)?;
    // 预设与 modes 在同一把锁内读（fix-fullcheck A4）。
    let (wanted, found) = find_preset(root, mode)?;
    if wanted != "builtin-jury" {
        return Err((
            2,
            format!("{}的视角不可设置，目前只认评审模式", found.name),
        ));
    }
    let mut modes = strict_modes_for_write(root)?;
    let shaped = crate::orchestration::shaped_preset(
        &found,
        crate::orchestration::host_section(&modes, host),
    );
    let total: usize = shaped.stages.iter().map(|stage| stage.nodes.len()).sum();
    if index == 0 || index > total {
        return Err((
            2,
            format!("位置序号超出范围：{} 只认 1–{total}", found.name),
        ));
    }
    let section = crate::orchestration::host_section_mut(&mut modes, host);
    let list = section.focus.entry(wanted).or_default();
    if list.len() < index {
        list.resize(index, String::new());
    }
    list[index - 1] = focus;
    crate::orchestration::save_modes_in(root, &modes).map_err(|message| (1, message))?;
    seat_entry_after_write(root, host, "builtin-jury")
}

fn run_config_set_focus(
    root: &Path,
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    if let Some(message) = unknown_option_error(args, 3, &["--host", "--mode", "--index", "--focus"]) {
        let _ = writeln!(stderr, "{message}");
        return 2;
    }
    let host = match resolve_host_for_write(root, option_value(args, "--host").as_deref()) {
        Ok(host) => host,
        Err((code, message)) => {
            let _ = writeln!(stderr, "{message}");
            return code;
        }
    };
    let Some(mode) = option_value(args, "--mode") else {
        let _ = writeln!(stderr, "缺少 --mode");
        return 2;
    };
    let Some(index_arg) = option_value(args, "--index") else {
        let _ = writeln!(stderr, "缺少 --index");
        return 2;
    };
    let Some(focus) = option_value(args, "--focus") else {
        let _ = writeln!(stderr, "缺少 --focus");
        return 2;
    };
    let Ok(index) = index_arg.parse::<usize>() else {
        let _ = writeln!(stderr, "位置序号必须是正整数：{index_arg}");
        return 2;
    };
    match config_set_focus_at(root, &host, &mode, index, &focus) {
        Ok(value) => {
            let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
            let _ = writeln!(stderr, "已写入评审视角");
            0
        }
        Err((code, message)) => {
            let _ = writeln!(stderr, "{message}");
            code
        }
    }
}

// ── TASK-50：主 agent（--host / hosts / set-host-vendor / recommend） ────

/// --host 解析（所有读写配置的命令与派发共用）。错误带退出码：给了
/// --host 但资源库读不了是 3、不是检测到的 agent 是 2；没给且配置了多于
/// 一个主 agent 是 2。None = 无配置（派发走 roles.json 行为，展示按空
/// 配置）。没给 --host 时不为了校验去读资源库：资源库读不了也照常推定
/// （fix-50-r1）；推定出的主 agent 在资源库可读时仍校验是否仍被检测到。
pub(crate) fn resolve_host_or_err(
    root: &Path,
    given: Option<&str>,
) -> Result<Option<String>, (i32, String)> {
    let modes = crate::orchestration::modes_from(root);
    if given.map(str::trim).filter(|host| !host.is_empty()).is_none() {
        // 没给 --host：只按 modes.json 的结构推定；资源库读不了时传空
        // detected（表示无法校验），不退出 3。
        let detected = match resources::get_resources_at(&store_file(root)) {
            Ok(store) => crate::hosts::detected_host_ids(&store),
            Err(_) => Vec::new(),
        };
        return match crate::hosts::resolve_host(None, &modes, &detected) {
            crate::hosts::HostResolve::Detected(host) => Ok(Some(host)),
            crate::hosts::HostResolve::None => Ok(None),
            crate::hosts::HostResolve::NeedHost(configured) => {
                Err((2, format!("需要 --host，可选：{}", configured.join("、"))))
            }
            crate::hosts::HostResolve::ConfiguredNotDetected(host, detected) => Err((
                2,
                format!(
                    "modes.json 里唯一的配置 {host} 未被检测到，请用 --host 指定，可选：{}",
                    join_or_none(&detected.iter().map(String::as_str).collect::<Vec<_>>())
                ),
            )),
            crate::hosts::HostResolve::Unknown(host, detected) => Err((
                2,
                format!(
                    "未知 host：{host}；检测到：{}",
                    join_or_none(&detected.iter().map(String::as_str).collect::<Vec<_>>())
                ),
            )),
        };
    }
    // 给了 --host：必须是检测到的 agent，资源库读不了退出 3。
    let store = resources::get_resources_at(&store_file(root)).map_err(|message| (3, message))?;
    let detected = crate::hosts::detected_host_ids(&store);
    match crate::hosts::resolve_host(given, &modes, &detected) {
        crate::hosts::HostResolve::Detected(host) => Ok(Some(host)),
        crate::hosts::HostResolve::None => Ok(None),
        crate::hosts::HostResolve::NeedHost(configured) => {
            Err((2, format!("需要 --host，可选：{}", configured.join("、"))))
        }
        // ConfiguredNotDetected 只在没给 --host 时出现，与 Unknown 同文案。
        crate::hosts::HostResolve::ConfiguredNotDetected(host, detected)
        | crate::hosts::HostResolve::Unknown(host, detected) => Err((
            2,
            format!(
                "未知 host：{host}；检测到：{}",
                join_or_none(&detected.iter().map(String::as_str).collect::<Vec<_>>())
            ),
        )),
    }
}

/// 写命令的 host 解析：没有配置任何主 agent 时必须给 --host（写盘要
/// 知道写进哪份配置）。
pub(crate) fn resolve_host_for_write(
    root: &Path,
    given: Option<&str>,
) -> Result<String, (i32, String)> {
    match resolve_host_or_err(root, given)? {
        Some(host) => Ok(host),
        None => {
            let store =
                resources::get_resources_at(&store_file(root)).map_err(|message| (3, message))?;
            let detected = crate::hosts::detected_host_ids(&store);
            Err((
                2,
                format!(
                    "需要 --host，可选：{}",
                    join_or_none(&detected.iter().map(String::as_str).collect::<Vec<_>>())
                ),
            ))
        }
    }
}

/// `aicross hosts`：列出检测到的主 agent 与推定的厂商。家目录注入是
/// 为了测试不碰真实家目录。
fn run_hosts(root: &Path, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let home = match crate::paths::home() {
        Ok(home) => home,
        Err(message) => {
            let _ = writeln!(stderr, "{message}");
            return 3;
        }
    };
    let env_base_url = std::env::var("ANTHROPIC_BASE_URL").ok();
    run_hosts_at(root, &home, env_base_url.as_deref(), stdout, stderr)
}

/// `run_hosts` 的实现体（家目录与 claude 环境变量注入）。
fn run_hosts_at(
    root: &Path,
    home: &Path,
    claude_base_url_env: Option<&str>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    match crate::hosts::list_hosts_at(root, home, claude_base_url_env) {
        Ok(hosts) => {
            let _ = writeln!(
                stdout,
                "{}",
                serde_json::to_string(&hosts).unwrap_or_else(|_| "[]".into())
            );
            let _ = writeln!(stderr, "已列出主 agent");
            0
        }
        Err(message) => {
            let _ = writeln!(stderr, "{message}");
            3
        }
    }
}

/// `aicross config set-host-vendor <host> <vendor|auto>`：设定或清除
/// 厂商覆盖；写入后输出该主 agent 的 hosts 条目。
fn run_config_set_host_vendor(
    root: &Path,
    home: &Path,
    claude_base_url_env: Option<&str>,
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let Some(host) = positional(args, 3) else {
        let _ = writeln!(stderr, "缺少 host");
        return 2;
    };
    let Some(vendor) = positional(args, 4) else {
        let _ = writeln!(stderr, "缺少厂商（auto 表示清除覆盖，恢复推定）");
        return 2;
    };
    if let Some(message) = extra_arg_error(args, 5) {
        let _ = writeln!(stderr, "{message}");
        return 2;
    }
    match crate::hosts::set_vendor_override_at_home(root, home, &host, &vendor, claude_base_url_env) {
        Ok(value) => {
            let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
            let _ = writeln!(stderr, "已写入厂商设定");
            0
        }
        Err((code, message)) => {
            let _ = writeln!(stderr, "{message}");
            code
        }
    }
}

/// `aicross config recommend [--host <h>] [--mode <模式>] [--all]`。
fn run_config_recommend(
    root: &Path,
    home: &Path,
    claude_base_url_env: Option<&str>,
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    if let Some(message) = unknown_option_error_with_flags(args, 3, &["--host", "--mode", "--all"], &["--all"]) {
        let _ = writeln!(stderr, "{message}");
        return 2;
    }
    let host = option_value(args, "--host");
    let mode = option_value(args, "--mode");
    let all = args.iter().any(|arg| arg == "--all");
    match crate::recommend::recommend_at_home(
        root,
        home,
        host.as_deref(),
        mode.as_deref(),
        all,
        claude_base_url_env,
    ) {
        Ok(value) => {
            let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
            let _ = writeln!(stderr, "已写入推荐人选");
            0
        }
        Err((code, message)) => {
            let _ = writeln!(stderr, "{message}");
            code
        }
    }
}

// ── TASK-50：skill 安装（status / install / uninstall） ───────────────────

/// `aicross skill status [--project <dir>]`。
fn run_skill(root: &Path, args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let _ = root;
    let home = match crate::paths::home() {
        Ok(home) => home,
        Err(message) => {
            let _ = writeln!(stderr, "{message}");
            return 3;
        }
    };
    run_skill_at(&home, args, stdout, stderr)
}

/// `run_skill` 的实现体（家目录注入，测试用临时目录）。
fn run_skill_at(
    home: &Path,
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    match args.get(2).map(String::as_str) {
        Some("status") => {
            if let Some(message) = unknown_option_error(args, 3, &["--project"]) {
                let _ = writeln!(stderr, "{message}");
                return 2;
            }
            let project = match option_path(args, "--project") {
                Ok(project) => project,
                Err((code, message)) => {
                    let _ = writeln!(stderr, "{message}");
                    return code;
                }
            };
            match crate::skill_install::status_at(home, project.as_deref()) {
                Ok(value) => {
                    let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
                    let _ = writeln!(stderr, "已读取 skill 安装状态");
                    0
                }
                Err((code, message)) => {
                    let _ = writeln!(stderr, "{message}");
                    code
                }
            }
        }
        Some("upgrade") => {
            if let Some(message) = unknown_option_error(args, 3, &["--host", "--path"]) {
                let _ = writeln!(stderr, "{message}");
                return 2;
            }
            let host = option_value(args, "--host").unwrap_or_default();
            let Some(path) = option_value(args, "--path") else {
                let _ = writeln!(stderr, "缺少 --path：要升级的 skill 位置");
                return 2;
            };
            match crate::skill_install::upgrade_at(home, &host, Path::new(&path)) {
                Ok(value) => {
                    let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
                    let _ = writeln!(stderr, "升级完成");
                    0
                }
                Err((code, message)) => {
                    let _ = writeln!(stderr, "{message}");
                    code
                }
            }
        }
        Some("install" | "uninstall") => {
            let action = args.get(2).map(String::as_str).unwrap_or_default();
            if let Some(message) = unknown_option_error(args, 3, &["--host", "--scope", "--project"]) {
                let _ = writeln!(stderr, "{message}");
                return 2;
            }
            let host = option_value(args, "--host").unwrap_or_default();
            let scope = option_value(args, "--scope").unwrap_or_default();
            let project = match option_path(args, "--project") {
                Ok(project) => project,
                Err((code, message)) => {
                    let _ = writeln!(stderr, "{message}");
                    return code;
                }
            };
            let outcome = if action == "install" {
                crate::skill_install::install_at(home, &host, &scope, project.as_deref())
            } else {
                crate::skill_install::uninstall_at(home, &host, &scope, project.as_deref())
            };
            match outcome {
                Ok(value) => {
                    let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
                    let _ = writeln!(stderr, "{}完成", if action == "install" { "安装" } else { "卸载" });
                    0
                }
                Err((code, message)) => {
                    let _ = writeln!(stderr, "{message}");
                    code
                }
            }
        }
        None => {
            let _ = writeln!(stderr, "缺少子命令（status | install | uninstall）");
            2
        }
        Some(other) => {
            let _ = writeln!(stderr, "未知子命令：{other}");
            2
        }
    }
}

// ── TASK-53：自定义短命令（slash list / add / remove） ───────────────

/// `aicross slash <list|add|remove>`：家目录注入是为了测试不碰真实家目录。
fn run_slash(root: &Path, args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let home = match crate::paths::home() {
        Ok(home) => home,
        Err(message) => {
            let _ = writeln!(stderr, "{message}");
            return 3;
        }
    };
    run_slash_at(root, &home, args, stdout, stderr)
}

/// `run_slash` 的实现体（家目录注入，测试用临时目录）。三个子命令都写
/// modes.json 或按它定位，--host 规则同 TASK-50 的写命令：没有配置时
/// 也要有具体 host，否则不知道看哪家的家目录。
fn run_slash_at(
    root: &Path,
    home: &Path,
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    match args.get(2).map(String::as_str) {
        Some("list") => {
            if let Some(message) = unknown_option_error(args, 3, &["--host"]) {
                let _ = writeln!(stderr, "{message}");
                return 2;
            }
            let host = match resolve_host_for_write(root, option_value(args, "--host").as_deref()) {
                Ok(host) => host,
                Err((code, message)) => {
                    let _ = writeln!(stderr, "{message}");
                    return code;
                }
            };
            match crate::slash::slash_list_at(root, home, &host) {
                Ok(value) => {
                    let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
                    let _ = writeln!(stderr, "已列出短命令");
                    0
                }
                Err((code, message)) => {
                    let _ = writeln!(stderr, "{message}");
                    code
                }
            }
        }
        Some("add") => {
            // `--note=<说明>` 写法：说明以 - 开头时不会被当成下一个选项的
            // 值；先摘出来，其余参数照常校验。
            let note_index = args.iter().position(|arg| arg.starts_with("--note="));
            let note = match note_index {
                Some(index) => args[index]["--note=".len()..].to_string(),
                None => option_value(args, "--note").unwrap_or_default(),
            };
            let remaining: Vec<String> = args
                .iter()
                .enumerate()
                .filter(|(index, _)| Some(*index) != note_index)
                .map(|(_, arg)| arg.clone())
                .collect();
            if let Some(message) = unknown_option_error(&remaining, 3, &["--host", "--name", "--mode", "--note"]) {
                let _ = writeln!(stderr, "{message}");
                return 2;
            }
            let Some(name) = option_value(args, "--name") else {
                let _ = writeln!(stderr, "缺少 --name");
                return 2;
            };
            let Some(mode) = option_value(args, "--mode") else {
                let _ = writeln!(stderr, "缺少 --mode");
                return 2;
            };
            let host = match resolve_host_for_write(root, option_value(args, "--host").as_deref()) {
                Ok(host) => host,
                Err((code, message)) => {
                    let _ = writeln!(stderr, "{message}");
                    return code;
                }
            };
            match crate::slash::slash_add_at(root, home, &host, &name, &mode, &note) {
                Ok(value) => {
                    let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
                    let _ = writeln!(stderr, "已添加短命令");
                    0
                }
                Err((code, message)) => {
                    let _ = writeln!(stderr, "{message}");
                    code
                }
            }
        }
        Some("remove") => {
            if let Some(message) = unknown_option_error(args, 3, &["--host", "--name"]) {
                let _ = writeln!(stderr, "{message}");
                return 2;
            }
            let Some(name) = option_value(args, "--name") else {
                let _ = writeln!(stderr, "缺少 --name");
                return 2;
            };
            let host = match resolve_host_for_write(root, option_value(args, "--host").as_deref()) {
                Ok(host) => host,
                Err((code, message)) => {
                    let _ = writeln!(stderr, "{message}");
                    return code;
                }
            };
            match crate::slash::slash_remove_at(root, home, &host, &name) {
                Ok(value) => {
                    let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
                    let _ = writeln!(stderr, "已移除短命令");
                    0
                }
                Err((code, message)) => {
                    let _ = writeln!(stderr, "{message}");
                    code
                }
            }
        }
        Some("add-all") => {
            if let Some(message) = unknown_option_error(args, 3, &["--host", "--prefix"]) {
                let _ = writeln!(stderr, "{message}");
                return 2;
            }
            let prefix = option_value(args, "--prefix")
                .unwrap_or_else(|| crate::slash::DEFAULT_PREFIX.to_string());
            let host = match resolve_host_for_write(root, option_value(args, "--host").as_deref()) {
                Ok(host) => host,
                Err((code, message)) => {
                    let _ = writeln!(stderr, "{message}");
                    return code;
                }
            };
            match crate::slash::slash_add_all_at(root, home, &host, &prefix) {
                Ok(value) => {
                    let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
                    let _ = writeln!(stderr, "已生成短命令");
                    let skipped = value["skipped"].as_array().map(|items| items.len()).unwrap_or(0);
                    if skipped > 0 {
                        let _ = writeln!(stderr, "跳过 {skipped} 条，原因见输出");
                    }
                    0
                }
                Err((code, message)) => {
                    let _ = writeln!(stderr, "{message}");
                    code
                }
            }
        }
        None => {
            let _ = writeln!(stderr, "缺少子命令（list | add | add-all | remove）");
            2
        }
        Some(other) => {
            let _ = writeln!(stderr, "未知子命令：{other}");
            2
        }
    }
}

// ── TASK-55：preset 子命令（自定义模式的新建、更新与删除） ──────────────

fn run_preset(root: &Path, args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    match args.get(2).map(String::as_str) {
        Some("save") => run_preset_save(root, args, stdout, stderr),
        Some("remove") => run_preset_remove(root, args, stdout, stderr),
        None => {
            let _ = writeln!(stderr, "缺少子命令（save | remove）");
            2
        }
        Some(other) => {
            let _ = writeln!(stderr, "未知子命令：{other}");
            2
        }
    }
}

/// `aicross preset save --file <json> [--host <h>]`：读入一个预设对象，
/// 校验后写入 presets.json；输出保存后的预设与完整 modes（同 config show）。
/// --host 规则同 TASK-50。预设文件 UTF-8（可带 BOM）、json 无法解析退出 2。
fn run_preset_save(root: &Path, args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    if let Some(message) = unknown_option_error(args, 3, &["--file", "--host"]) {
        let _ = writeln!(stderr, "{message}");
        return 2;
    }
    let Some(path) = option_value(args, "--file") else {
        let _ = writeln!(stderr, "缺少 --file");
        return 2;
    };
    let host = match resolve_host_or_err(root, option_value(args, "--host").as_deref()) {
        Ok(host) => host,
        Err((code, message)) => {
            let _ = writeln!(stderr, "{message}");
            return code;
        }
    };
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            let _ = writeln!(stderr, "读取预设文件失败：{error}");
            return 2;
        }
    };
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
    let text = match String::from_utf8(bytes.to_vec()) {
        Ok(text) => text,
        Err(_) => {
            let _ = writeln!(stderr, "预设文件必须是 UTF-8 编码");
            return 2;
        }
    };
    let input: crate::preset::PresetInput = match serde_json::from_str(&text) {
        Ok(input) => input,
        Err(error) => {
            let _ = writeln!(stderr, "预设文件无法解析：{error}");
            return 2;
        }
    };
    match crate::preset::preset_save_at(root, host.as_deref(), &input) {
        Ok(value) => {
            let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
            let _ = writeln!(stderr, "已保存模式");
            0
        }
        Err((code, message)) => {
            let _ = writeln!(stderr, "{message}");
            code
        }
    }
}

/// `aicross preset remove <id>`：删除用户预设并级联清理人选、默认模式与
/// 短命令；输出删除结果与受影响项列表。需要家目录（删短命令文件）。
fn run_preset_remove(root: &Path, args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    if let Some(message) = unknown_option_error_allow_positionals(args, 3, &[]) {
        let _ = writeln!(stderr, "{message}");
        return 2;
    }
    let positionals = positional_values(args, 3, &[]);
    let Some(id) = positionals.first().map(|value| value.to_string()) else {
        let _ = writeln!(stderr, "缺少模式 id");
        return 2;
    };
    if positionals.len() > 1 {
        let _ = writeln!(stderr, "不认识的参数：{}", positionals[1]);
        return 2;
    }
    let home = match crate::paths::home() {
        Ok(home) => home,
        Err(message) => {
            let _ = writeln!(stderr, "{message}");
            return 3;
        }
    };
    match crate::preset::preset_remove_at(root, &home, &id) {
        Ok(value) => {
            let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
            let _ = writeln!(stderr, "已移除模式");
            0
        }
        Err((code, message)) => {
            let _ = writeln!(stderr, "{message}");
            code
        }
    }
}

/// --project 的目录参数；值必须存在。
fn option_path(args: &[String], name: &str) -> Result<Option<std::path::PathBuf>, (i32, String)> {
    match option_value(args, name) {
        Some(value) => Ok(Some(std::path::PathBuf::from(value))),
        None => Ok(None),
    }
}

/// 写入前严格读 modes.json：损坏、旧格式、版本不认或读不了时报错退出 1，
/// 不覆盖已有人选。短命令（TASK-53）同用这份。
pub(crate) fn strict_modes_for_write(root: &Path) -> Result<ModesFile, (i32, String)> {
    crate::orchestration::modes_from_strict(root).map_err(|message| {
        (1, format!("{message}；已停止写入，修复或删除该文件后重试"))
    })
}

/// 等 modes.json 写锁的上限；锁内只有读改写本地小文件，与资源库同取 40 秒。
const MODES_LOCK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(40);

/// modes.json 写锁：进程内互斥加跨进程文件锁（常驻的 `modes.json.lock`）。
/// 覆盖「严格读 → 修改 → 写盘」，窗口、命令行与不同主 agent 里的 skill
/// 是不同进程，同时改配置时互相等待，不覆盖对方的改动（一键推荐、预设、
/// 短命令同用这把锁）。presets.json 的写入也在这把锁内。
pub(crate) struct ModesGuard {
    // 字段按声明顺序释放：先放进程内互斥，再关锁文件句柄。
    _mutex: std::sync::MutexGuard<'static, ()>,
    _file: crate::file_lock::FileLock,
}

/// 先拿文件锁再拿进程内互斥：等别的进程放锁期间不占着进程内互斥。
pub(crate) fn modes_write_lock(root: &Path) -> Result<ModesGuard, (i32, String)> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    let file = crate::file_lock::FileLock::acquire(&root.join("modes.json.lock"), MODES_LOCK_TIMEOUT)
        .map_err(|message| (1, message))?;
    let mutex = LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    Ok(ModesGuard { _mutex: mutex, _file: file })
}

/// fix-46-r1 R7：写入前把该模式的 seats 数组整理成预设的形状（实现已
/// 收敛到 `orchestration::shape_seats_to_preset`，set-seat 与一键推荐共用）。

/// `config set-seat <模式> <阶段> <位置> …` 的实现体；窗口命令直接调这里。
/// 校验顺序：模式不存在 → 阶段/位置超出范围 → thinking 非法 → 人选或备选
/// 不在资源库。写入后返回该模式那一项（同 `config show` 的 modes[i]）。
pub(crate) fn config_set_seat_at(
    root: &Path,
    host: &str,
    mode: &str,
    stage: usize,
    index: usize,
    seat: SeatChoice,
) -> Result<Value, (i32, String)> {
    // 锁外先查一次，模式不存在时最先报（校验顺序不变）；写入用的预设在
    // 锁内重读（fix-fullcheck A4）。
    let (wanted, _) = find_preset(root, mode)?;
    // TASK-56：阶段与位置按该主 agent 调整后的形状算。形状在写入锁内按
    // 严格读到的 modes 重取并重新校验范围：counts 与人选在同一次读改写里
    // 保持一致，不被并发的 set-count 改动隔开。
    if !seat.thinking.is_empty() && !THINKING_LEVELS.contains(&seat.thinking.as_str()) {
        return Err((2, format!("--thinking 只认 关|低|中|高：{}", seat.thinking)));
    }
    if seat.agent.trim().is_empty() {
        return Err((2, "缺少 --agent".into()));
    }
    if seat.model.trim().is_empty() {
        return Err((2, "缺少 --model".into()));
    }
    if let Some(fallback) = &seat.fallback {
        if fallback.agent.trim().is_empty() || fallback.model.trim().is_empty() {
            return Err((2, "备选需要 agent 与 model".into()));
        }
    }
    let (agent_id, chosen) =
        resolve_resource_model(root, &seat.agent, &seat.model, seat.provider.as_deref())?;
    let fallback = match &seat.fallback {
        Some(fallback) => {
            let (agent_id, chosen) = resolve_resource_model(
                root,
                &fallback.agent,
                &fallback.model,
                fallback.provider.as_deref(),
            )?;
            Some(FallbackTarget {
                agent: agent_id,
                provider: (!chosen.provider.is_empty()).then(|| chosen.provider.clone()),
                model: chosen.id.clone(),
            })
        }
        None => None,
    };
    {
        let _guard = modes_write_lock(root)?;
        let (_, found) = find_preset(root, mode)?;
        let mut modes = strict_modes_for_write(root)?;
        let preset = {
            let section = crate::orchestration::host_section(&modes, host);
            crate::orchestration::shaped_preset(&found, section)
        };
        if stage == 0 || stage > preset.stages.len() {
            return Err((
                2,
                format!("阶段超出范围：{} 共 {} 个阶段", preset.name, preset.stages.len()),
            ));
        }
        let node_count = preset.stages[stage - 1].nodes.len();
        if index == 0 || index > node_count {
            return Err((
                2,
                format!("位置超出范围：{} 第 {stage} 阶段共 {node_count} 个位置", preset.name),
            ));
        }
        // TASK-61：只接盲位置的 agent（agy 与未实测四家）不许写进共享 /
        // 单写者位置；人选与备选都查。
        let visibility = &preset.stages[stage - 1].nodes[index - 1].visibility;
        let chosen_agents = std::iter::once(agent_id.as_str())
            .chain(fallback.as_ref().map(|fallback| fallback.agent.as_str()));
        for agent in chosen_agents {
            if let Some(reason) = crate::agent_catalog::seat_violation(agent, visibility) {
                return Err((
                    2,
                    format!(
                        "{reason}：{} 第 {stage} 阶段第 {index} 个位置可见性为「{visibility}」，{agent} 只能放在盲位置",
                        preset.name
                    ),
                ));
            }
        }
        let section = crate::orchestration::host_section_mut(&mut modes, host);
        let stages = section.seats.entry(wanted.clone()).or_default();
        // 写盘前按预设形状补齐 null（fix-46-r1 R7），前面的阶段不再落盘为空数组。
        crate::orchestration::shape_seats_to_preset(stages, &preset);
        stages[stage - 1][index - 1] = Some(SeatChoice {
            agent: agent_id,
            // provider 用资源库里的规范值；原生模型的空 provider 写 null。
            provider: (!chosen.provider.is_empty()).then(|| chosen.provider.clone()),
            model: chosen.id.clone(),
            tier: seat.tier,
            thinking: seat.thinking,
            fallback,
            // 窗口与命令行手选的人选都是 manual。
            origin: crate::orchestration::SEAT_ORIGIN_MANUAL.to_string(),
        });
        crate::orchestration::save_modes_in(root, &modes).map_err(|message| (1, message))?;
    }
    seat_entry_after_write(root, host, &wanted)
}

fn run_config_set_seat(
    root: &Path,
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let set_seat_options = [
        "--host",
        "--agent",
        "--model",
        "--provider",
        "--tier",
        "--thinking",
        "--fallback-agent",
        "--fallback-model",
        "--fallback-provider",
    ];
    if let Some(message) = unknown_option_error_allow_positionals(args, 3, &set_seat_options) {
        let _ = writeln!(stderr, "{message}");
        return 2;
    }
    let positionals = positional_values(args, 3, &set_seat_options);
    if positionals.len() > 3 {
        let _ = writeln!(stderr, "不认识的参数：{}", positionals[3]);
        return 2;
    }
    let Some(mode) = positionals.first().map(|value| value.to_string()) else {
        let _ = writeln!(stderr, "缺少模式");
        return 2;
    };
    let Some(stage_arg) = positionals.get(1).map(|value| value.to_string()) else {
        let _ = writeln!(stderr, "缺少阶段");
        return 2;
    };
    let Some(index_arg) = positionals.get(2).map(|value| value.to_string()) else {
        let _ = writeln!(stderr, "缺少位置");
        return 2;
    };
    let host = match resolve_host_for_write(root, option_value(args, "--host").as_deref()) {
        Ok(host) => host,
        Err((code, message)) => {
            let _ = writeln!(stderr, "{message}");
            return code;
        }
    };
    let Ok(stage) = stage_arg.parse::<usize>() else {
        let _ = writeln!(stderr, "阶段必须是正整数：{stage_arg}");
        return 2;
    };
    let Ok(index) = index_arg.parse::<usize>() else {
        let _ = writeln!(stderr, "位置必须是正整数：{index_arg}");
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
    let Some(agent_arg) = option_value(args, "--agent") else {
        let _ = writeln!(stderr, "缺少 --agent");
        return 2;
    };
    let Some(model_arg) = option_value(args, "--model") else {
        let _ = writeln!(stderr, "缺少 --model");
        return 2;
    };
    let fallback_agent = option_value(args, "--fallback-agent");
    let fallback_model = option_value(args, "--fallback-model");
    let fallback_provider = option_value(args, "--fallback-provider");
    // 备选三项的配对：--fallback-provider 必须跟着另两项；agent 与 model 成对。
    match (&fallback_agent, &fallback_model, &fallback_provider) {
        (Some(_), Some(_), _) | (None, None, None) => {}
        (None, None, Some(_)) => {
            let _ = writeln!(stderr, "--fallback-provider 需要 --fallback-agent 与 --fallback-model");
            return 2;
        }
        _ => {
            let _ = writeln!(stderr, "备选需要 --fallback-agent 与 --fallback-model 成对给");
            return 2;
        }
    }
    let seat = SeatChoice {
        agent: agent_arg,
        provider: provider_arg,
        model: model_arg,
        tier: tier_arg.unwrap_or_default(),
        thinking: thinking_arg.unwrap_or_default(),
        fallback: fallback_agent.zip(fallback_model).map(|(agent, model)| FallbackTarget {
            agent,
            provider: fallback_provider,
            model,
        }),
        origin: crate::orchestration::SEAT_ORIGIN_MANUAL.to_string(),
    };
    match config_set_seat_at(root, &host, &mode, stage, index, seat) {
        Ok(value) => {
            let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
            let _ = writeln!(stderr, "已写入模式人选");
            0
        }
        Err((code, message)) => {
            let _ = writeln!(stderr, "{message}");
            code
        }
    }
}

/// `config clear-seat <模式> [<阶段> <位置>]` 的实现体；给了位置只清这一个
///（置 null，本来就没设时不改盘），没给清掉该模式全部人选（删 seats 键）。
pub(crate) fn config_clear_seat_at(
    root: &Path,
    host: &str,
    mode: &str,
    stage: Option<usize>,
    index: Option<usize>,
) -> Result<Value, (i32, String)> {
    let _guard = modes_write_lock(root)?;
    // 预设与 modes 在同一把锁内读（fix-fullcheck A4）。
    let (wanted, found) = find_preset(root, mode)?;
    if let Some(stage) = stage {
        let index = index.expect("调用方保证成对");
        // TASK-56：范围按调整后的形状算，与 set-seat 同一说法；形状在写入
        // 锁内按严格读到的 modes 取，counts 与人选同一次读改写保持一致。
        let mut modes = strict_modes_for_write(root)?;
        let preset = {
            let section = crate::orchestration::host_section(&modes, host);
            crate::orchestration::shaped_preset(&found, section)
        };
        if stage == 0 || stage > preset.stages.len() {
            return Err((
                2,
                format!("阶段超出范围：{} 共 {} 个阶段", preset.name, preset.stages.len()),
            ));
        }
        let node_count = preset.stages[stage - 1].nodes.len();
        if index == 0 || index > node_count {
            return Err((
                2,
                format!("位置超出范围：{} 第 {stage} 阶段共 {node_count} 个位置", preset.name),
            ));
        }
        let slot = crate::orchestration::host_section_mut(&mut modes, host)
            .seats
            .get_mut(&wanted)
            .and_then(|stages| stages.get_mut(stage - 1))
            .and_then(|nodes| nodes.get_mut(index - 1));
        let cleared = slot.as_ref().map(|slot| slot.is_some()).unwrap_or(false);
        if cleared {
            *slot.expect("cleared 才进来，槽位必存在") = None;
            crate::orchestration::save_modes_in(root, &modes).map_err(|message| (1, message))?;
        }
    } else {
        let mut modes = strict_modes_for_write(root)?;
        if crate::orchestration::host_section_mut(&mut modes, host)
            .seats
            .shift_remove(&wanted)
            .is_some()
        {
            crate::orchestration::save_modes_in(root, &modes).map_err(|message| (1, message))?;
        }
    }
    // 读回要再拿这把锁，先放掉。
    drop(_guard);
    seat_entry_after_write(root, host, &wanted)
}

fn run_config_clear_seat(
    root: &Path,
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    if let Some(message) = unknown_option_error_allow_positionals(args, 3, &["--host"]) {
        let _ = writeln!(stderr, "{message}");
        return 2;
    }
    let positionals = positional_values(args, 3, &["--host"]);
    if positionals.len() > 3 {
        let _ = writeln!(stderr, "不认识的参数：{}", positionals[3]);
        return 2;
    }
    let Some(mode) = positionals.first().map(|value| value.to_string()) else {
        let _ = writeln!(stderr, "缺少模式");
        return 2;
    };
    let host = match resolve_host_for_write(root, option_value(args, "--host").as_deref()) {
        Ok(host) => host,
        Err((code, message)) => {
            let _ = writeln!(stderr, "{message}");
            return code;
        }
    };
    let stage_arg = positionals.get(1).map(|value| value.to_string());
    let index_arg = positionals.get(2).map(|value| value.to_string());
    let (stage, index) = match (stage_arg, index_arg) {
        (None, None) => (None, None),
        (Some(stage_arg), Some(index_arg)) => {
            let Ok(stage) = stage_arg.parse::<usize>() else {
                let _ = writeln!(stderr, "阶段必须是正整数：{stage_arg}");
                return 2;
            };
            let Ok(index) = index_arg.parse::<usize>() else {
                let _ = writeln!(stderr, "位置必须是正整数：{index_arg}");
                return 2;
            };
            (Some(stage), Some(index))
        }
        _ => {
            let _ = writeln!(stderr, "阶段与位置要成对给");
            return 2;
        }
    };
    match config_clear_seat_at(root, &host, &mode, stage, index) {
        Ok(value) => {
            let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
            let _ = writeln!(stderr, "已清除模式人选");
            0
        }
        Err((code, message)) => {
            let _ = writeln!(stderr, "{message}");
            code
        }
    }
}

/// 写盘后重新读出该模式那一项（同 `config show` 读回的 modes[i]）。
fn seat_entry_after_write(root: &Path, host: &str, preset_id: &str) -> Result<Value, (i32, String)> {
    let store = resource_store_for_state(root);
    let presets = crate::orchestration::raw_presets_from(root);
    let (default_mode, _) = modes_section(root, Some(host));
    let roles = crate::orchestration::roles_from(root);
    let modes = crate::orchestration::modes_from(root);
    let section = crate::orchestration::host_section(&modes, host);
    presets
        .presets
        .iter()
        .find(|preset| preset.id == preset_id)
        .map(|preset| mode_entry(preset, &roles, section, &default_mode, store.as_ref()))
        .ok_or_else(|| (1, format!("找不到预设：{preset_id}")))
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
    acp: &AcpOps,
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
    // 10-01 定案：打开 ACP 时当场装适配器；装不上不回滚设置，退出码 1，派发对这几家回退一次性 CLI。
    if key == "acpEnabled" && dispatch.acp_enabled {
        let reports = (acp.install)();
        let value = json!({ "dispatch": dispatch, "acp": reports });
        let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
        let described = |report: &AdapterReport| {
            let state = serde_json::to_value(report.state).ok();
            format!("{} {}", report.agent, state.as_ref().and_then(Value::as_str).unwrap_or(""))
        };
        let failed = reports.iter().filter(|report| report.not_installed()).map(described).collect::<Vec<_>>();
        if failed.is_empty() {
            let states = reports.iter().map(described).collect::<Vec<_>>();
            let _ = writeln!(stderr, "已写入派发设置；ACP 适配器：{}", states.join("，"));
            return 0;
        }
        let _ = writeln!(stderr, "已写入派发设置；ACP 适配器未装上：{}；派发对这几家回退一次性 CLI", failed.join("，"));
        return 1;
    }
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

// ── TASK-44：recover 子命令 ────────────────────────────────────────

/// aicross recover <run_id> [--rollback]：崩溃后核对。参数错退出 2；读 run、
/// 落图失败与仍在运行、回滚有失败退出 1；核对完成退出 0。
fn run_recover(root: &Path, args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let Some(run_id) = positional(args, 2) else {
        let _ = writeln!(stderr, "缺少 run_id");
        return 2;
    };
    for arg in args.iter().skip(3) {
        if arg != "--rollback" {
            let _ = writeln!(stderr, "不认识的参数：{arg}");
            return 2;
        }
    }
    let rollback = args.iter().any(|arg| arg == "--rollback");
    return match crate::engine::recover(root, &run_id, rollback) {
        Ok(outcome) => {
            let _ = writeln!(stdout, "{}", serde_json::to_string(&outcome.report).unwrap_or_else(|_| "{}".into()));
            let _ = writeln!(stderr, "{}", outcome.message);
            outcome.exit
        }
        Err(message) => {
            let _ = writeln!(stderr, "{message}");
            1
        }
    };
}

// ── resources / config 的输出与参数工具 ──────────────────────────────

/// `aicross resources` 的 stdout JSON；窗口的资源页与 config_snapshot 复用
/// 同一结构。only_agent 只看一个 agent（有且仅有一个时直接给对象）。
fn resources_value_filtered(store: &ResourceStore, only_agent: Option<&str>) -> Value {
    let now_millis = chrono::Local::now().timestamp_millis();
    // 走查 #1：detectedAt 为空说明从未探测过，agent 与其下模型都显示「未探测」。
    let ever_detected = store.detected_at.is_some();
    let entries: Vec<Value> = store
        .agents
        .iter()
        .filter(|agent| only_agent.map_or(true, |id| agent.id == id))
        .map(|agent| agent_entry(agent, ever_detected, now_millis))
        .collect();
    match (only_agent, entries.len()) {
        (Some(_), 1) => entries.into_iter().next().expect("len checked"),
        _ => json!({ "agents": entries, "detected_at": store.detected_at }),
    }
}

/// 全量资源输出（窗口用）。
pub(crate) fn resources_value(store: &ResourceStore) -> Value {
    resources_value_filtered(store, None)
}

fn output_resources(store: &ResourceStore, only_agent: Option<&str>, stdout: &mut dyn Write) {
    let value = resources_value_filtered(store, only_agent);
    let _ = writeln!(stdout, "{}", serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()));
}

fn agent_entry(agent: &ResourceAgent, ever_detected: bool, now_millis: i64) -> Value {
    let detected = agent.detected;
    let models: Vec<Value> = agent
        .models
        .iter()
        .map(|model| {
            // 走查 #1：从未探测时模型同 agent 一致显示「未探测」，note 为空。
            let (state, note) = if ever_detected {
                resources::model_state(model, detected, now_millis)
            } else {
                (resources::RESOURCE_STATE_NEVER_DETECTED, String::new())
            };
            json!({
                "id": model.id,
                "agent": model.agent,
                "provider": model.provider,
                "resolved_id": model.resolved_id,
                "state": state,
                "note": note,
                "missing": resources::is_missing_from_detection(model),
            })
        })
        .collect();
    let (state, note) = if ever_detected {
        (
            resources::agent_state(agent.detected),
            resources::agent_state_note(agent),
        )
    } else {
        (
            resources::RESOURCE_STATE_NEVER_DETECTED,
            resources::NEVER_DETECTED_NOTE.to_string(),
        )
    };
    json!({
        "id": agent.id,
        "name": agent.name,
        "version": agent.version,
        "state": state,
        "note": note,
        "models": models,
        "untested": agent.untested,
        "homepage": agent.homepage,
        "custom_path": agent.custom_path,
    })
}

/// 该 agent 下可选的模型清单（provider 非空时带 provider 前缀）；空时写「（无）」。
fn model_candidates(agent: &ResourceAgent) -> String {
    model_candidates_filtered(agent, |_| true)
}

/// 同上，`keep` 不认的条目不列（TASK-48：未发现的模型列出来也选不了）。
fn model_candidates_filtered(agent: &ResourceAgent, keep: impl Fn(&ResourceModel) -> bool) -> String {
    let items: Vec<String> = agent
        .models
        .iter()
        .filter(|model| keep(model))
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
    unknown_option_error_with_flags(args, from, allowed, &[])
}

/// 同上，flags 里是不带值的开关（如 --all）：出现即放行，不吞下一个
/// 参数（fix-50-r1：`recommend --all` 不再报「缺少 --all 的值」）。
fn unknown_option_error_with_flags(
    args: &[String],
    from: usize,
    allowed: &[&str],
    flags: &[&str],
) -> Option<String> {
    let mut index = from;
    while index < args.len() {
        let arg = args[index].as_str();
        if !arg.starts_with('-') || !allowed.contains(&arg) {
            return Some(format!("不认识的参数：{arg}"));
        }
        if flags.contains(&arg) {
            index += 1;
            continue;
        }
        match args.get(index + 1) {
            Some(value) if !value.starts_with('-') => index += 2,
            _ => return Some(format!("缺少 {arg} 的值")),
        }
    }
    None
}

/// 同上，但位置参数（非 `-` 开头）放行：带 --host 的模式写命令的选项
/// 与位置参数次序不固定，位置参数另由各命令自己校验个数。
fn unknown_option_error_allow_positionals(
    args: &[String],
    from: usize,
    allowed: &[&str],
) -> Option<String> {
    let mut index = from;
    while index < args.len() {
        let arg = args[index].as_str();
        if !arg.starts_with('-') {
            index += 1;
            continue;
        }
        if !allowed.contains(&arg) {
            return Some(format!("不认识的参数：{arg}"));
        }
        match args.get(index + 1) {
            Some(value) if !value.starts_with('-') => index += 2,
            _ => return Some(format!("缺少 {arg} 的值")),
        }
    }
    None
}

/// from 之后的位置参数（跳过 allowed 里的选项与其值）：选项与位置参数
/// 次序不固定，位置参数另按这份清单校验个数与取值。
fn positional_values<'a>(args: &'a [String], from: usize, allowed: &[&str]) -> Vec<&'a str> {
    let mut values = Vec::new();
    let mut index = from;
    while index < args.len() {
        let arg = args[index].as_str();
        if arg.starts_with('-') {
            // 带值选项连同它的值一起跳过；不认识的选项交给
            // unknown_option_error_allow_positionals 报错。
            index += if allowed.contains(&arg) { 2 } else { 1 };
            continue;
        }
        values.push(arg);
        index += 1;
    }
    values
}

#[cfg(windows)]
pub(crate) fn attach_parent_console() {
    use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    unsafe { let _ = AttachConsole(ATTACH_PARENT_PROCESS); }
}

#[cfg(not(windows))]
pub(crate) fn attach_parent_console() {}

/// 模式名解析：`--preset` 与 config 的模式类命令、一键推荐共用同一个。
/// 别名 shared / economy / blind 对应内置预设 id，其余按预设 id 原样。
pub(crate) fn resolve_preset_name(name: &str) -> String {
    match name {
        "shared" => "builtin-shared".into(),
        "economy" => "builtin-economy".into(),
        "blind" => "builtin-blind".into(),
        other => other.into(),
    }
}

struct CliRequest {
    body: Option<Value>,
    timeout: Duration,
    /// TASK-43：需要写到 stderr 的说明（如 --accept-file 过大截断）。
    notes: Vec<String>,
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
            // TASK-43：--accept-file 可选，验收条件随派发下发给盲验收与盲审节点。
            let mut notes = Vec::new();
            let accept_criteria = if args.iter().any(|arg| arg == "--accept-file") {
                let path = option_value(args, "--accept-file")
                    .ok_or_else(|| "缺少 --accept-file 的值".to_string())?;
                let (content, note) = read_accept_file(&path)?;
                if let Some(note) = note {
                    notes.push(note);
                }
                content
            } else {
                String::new()
            };
            let mut body = json!({ "project": project, "text": text });
            // TASK-46：--preset 缺省时不传，引擎按 modes.json 的默认模式解析
            // （未设默认模式时仍是经济模式）。别名解析与 config 命令共用。
            if let Some(preset) = option_value(args, "--preset") {
                body["preset"] = Value::String(resolve_preset_name(&preset));
            }
            // TASK-50：--host 决定用哪份主 agent 配置；缺省时引擎按单配置推定。
            if let Some(host) = option_value(args, "--host") {
                body["host"] = Value::String(host);
            }
            if !accept_criteria.is_empty() {
                body["accept_criteria"] = Value::String(accept_criteria);
            }
            Ok(CliRequest {
                body: Some(body),
                timeout: Duration::from_secs(30),
                notes,
            })
        }
        Some("status") => {
            positional(args, 2).ok_or_else(|| "缺少 run_id".to_string())?;
            let timeout = option_value(args, "--timeout").map(|value| value.parse::<u64>().map_err(|_| "--timeout 必须是正整数".to_string())).transpose()?.unwrap_or(600);
            if timeout == 0 { return Err("--timeout 必须是正整数".into()); }
            Ok(CliRequest { body: None, timeout: Duration::from_secs(timeout), notes: Vec::new() })
        }
        Some("adopt") => {
            positional(args, 2).ok_or_else(|| "缺少 run_id".to_string())?;
            let node_id = positional(args, 3).ok_or_else(|| "缺少 node_id".to_string())?;
            Ok(CliRequest { body: Some(json!({ "node_id": node_id })), timeout: Duration::from_secs(30), notes: Vec::new() })
        }
        Some("cancel") => {
            positional(args, 2).ok_or_else(|| "缺少 run_id".to_string())?;
            Ok(CliRequest { body: Some(json!({})), timeout: Duration::from_secs(30), notes: Vec::new() })
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

/// TASK-43：验收条件文件。UTF-8，可带 BOM（同 --text-file）；超过 64 KB
/// 截断，说明文字由调用方写到 stderr。
fn read_accept_file(path: &str) -> Result<(String, Option<String>), String> {
    const ACCEPT_FILE_LIMIT: usize = 64 * 1024;
    let bytes = fs::read(path).map_err(|error| format!("读取验收条件文件失败：{error}"))?;
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
    let mut text = String::from_utf8(bytes.to_vec())
        .map_err(|_| "验收条件文件必须是 UTF-8 编码".to_string())?;
    if text.len() > ACCEPT_FILE_LIMIT {
        let mut limit = ACCEPT_FILE_LIMIT;
        while !text.is_char_boundary(limit) {
            limit -= 1;
        }
        text.truncate(limit);
        return Ok((text, Some("验收条件文件超过 64 KB，已截断".into())));
    }
    Ok((text, None))
}

fn positional(args: &[String], index: usize) -> Option<String> {
    args.get(index).filter(|arg| !arg.starts_with('-')).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn top_level_help_and_version_print_and_exit_0() {
        // `aicross --help` / `--version`（含短形式）：命令行打印后退出 0。
        // 子命令一览不含已删除的 propose 与内部的 __run。
        for flag in ["--help", "-h"] {
            let mut out = Vec::new();
            assert_eq!(top_level_info(flag, &mut out), 0);
            let text = String::from_utf8(out).unwrap();
            for word in ["dispatch", "status", "open", "resources", "preset"] {
                assert!(text.contains(word), "{word} 不在帮助里：{text}");
            }
            assert!(!text.contains("propose"), "{text}");
            assert!(!text.contains("__run"), "{text}");
        }
        for flag in ["--version", "-V"] {
            let mut out = Vec::new();
            assert_eq!(top_level_info(flag, &mut out), 0);
            let text = String::from_utf8(out).unwrap();
            assert_eq!(text, format!("aicross {}\n", env!("CARGO_PKG_VERSION")));
        }
    }

    #[test]
    fn help_flags_route_through_cli_not_the_window() {
        // 路由：--help 系列在 run_if_subcommand 短路；空 argv 与未知参数
        // 照旧返回 None 落到窗口。
        for flag in ["--help", "-h", "--version", "-V"] {
            let args = vec!["aicross".into(), flag.into()];
            assert_eq!(run_if_subcommand(&args), Some(0), "{flag}");
        }
        assert_eq!(run_if_subcommand(&["aicross".into()]), None);
        assert_eq!(
            run_if_subcommand(&["aicross".into(), "--no-such-flag".into()]),
            None
        );
    }

    #[test]
    fn propose_is_recognized_as_removed_and_exits_2() {
        // TASK-40 R1-6：propose 随窗口 HTTP 通道删除；不再拉起窗口，
        // stderr 给去向提示，退出码 2（参数错同档）。
        let root = crate::paths::test_temp_dir().join(format!("aicross-cli-propose-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let args = vec!["aicross".into(), "propose".into(), "给个方案".into()];
        assert_eq!(run_at(&args, &root, &mut stdout, &mut stderr), 2);
        assert!(stdout.is_empty());
        let message = String::from_utf8(stderr).unwrap();
        assert!(message.contains("propose 已删除"), "{message}");
        assert!(message.contains("宿主对话"), "{message}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn missing_run_is_a_business_failure() {
        // status 直接走引擎读 run：run 不存在是业务失败（退出码 1），不再有清单一说。
        let root = crate::paths::test_temp_dir().join(format!("aicross-cli-missing-{}", Uuid::new_v4()));
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
        let root = crate::paths::test_temp_dir().join(format!("aicross-cli-text-{}", Uuid::new_v4()));
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
        let root = crate::paths::test_temp_dir().join(format!("aicross-cli-encoding-{}", Uuid::new_v4()));
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
    fn dispatch_accept_file_requires_value_and_existing_utf8_file() {
        let root = crate::paths::test_temp_dir().join(format!("aicross-cli-accept-missing-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let text_path = root.join("t.txt");
        fs::write(&text_path, "正文").unwrap();
        let text_arg: String = text_path.to_string_lossy().into();
        let no_value = vec![
            "aicross".into(), "dispatch".into(), "--project".into(), "D:/p".into(),
            "--text-file".into(), text_arg.clone(), "--accept-file".into(),
        ];
        assert!(matches!(
            build_request(&no_value),
            Err(error) if error == "缺少 --accept-file 的值"
        ));
        let missing = vec![
            "aicross".into(), "dispatch".into(), "--project".into(), "D:/p".into(),
            "--text-file".into(), text_arg.clone(),
            "--accept-file".into(), root.join("nope.md").to_string_lossy().into(),
        ];
        assert!(matches!(
            build_request(&missing),
            Err(error) if error.starts_with("读取验收条件文件失败：")
        ));
        let path = root.join("accept.md");
        fs::write(&path, [0xff, 0xfe, 0x00]).unwrap();
        let non_utf8 = vec![
            "aicross".into(), "dispatch".into(), "--project".into(), "D:/p".into(),
            "--text-file".into(), text_arg,
            "--accept-file".into(), path.to_string_lossy().into(),
        ];
        assert!(matches!(
            build_request(&non_utf8),
            Err(error) if error == "验收条件文件必须是 UTF-8 编码"
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dispatch_accept_file_goes_into_request_body_and_truncates_at_64kb() {
        let root = crate::paths::test_temp_dir().join(format!("aicross-cli-accept-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let text_path = root.join("t.txt");
        fs::write(&text_path, "正文").unwrap();
        let text_arg: String = text_path.to_string_lossy().into();
        let path = root.join("accept.md");
        fs::write(&path, [vec![0xef, 0xbb, 0xbf], "逐条核对".as_bytes().to_vec()].concat()).unwrap();
        let args = vec![
            "aicross".into(), "dispatch".into(), "--project".into(), "D:/p".into(),
            "--text-file".into(), text_arg.clone(), "--accept-file".into(), path.to_string_lossy().into(),
        ];
        let request = build_request(&args).unwrap();
        assert_eq!(request.body.unwrap()["accept_criteria"], "逐条核对");
        assert!(request.notes.is_empty());

        let big = root.join("big.md");
        fs::write(&big, "a".repeat(70 * 1024)).unwrap();
        let args = vec![
            "aicross".into(), "dispatch".into(), "--project".into(), "D:/p".into(),
            "--text-file".into(), text_arg, "--accept-file".into(), big.to_string_lossy().into(),
        ];
        let request = build_request(&args).unwrap();
        assert_eq!(request.body.unwrap()["accept_criteria"].as_str().unwrap().len(), 64 * 1024);
        assert_eq!(request.notes, vec!["验收条件文件超过 64 KB，已截断".to_string()]);

        // 第 65536 字节落在多字节字符中间：退到字符边界截断，不 panic。
        let wide = root.join("wide.md");
        fs::write(&wide, format!("ab{}", "中".repeat(30_000))).unwrap();
        let args = vec![
            "aicross".into(), "dispatch".into(), "--project".into(), "D:/p".into(),
            "--text-file".into(), root.join("t.txt").to_string_lossy().into(),
            "--accept-file".into(), wide.to_string_lossy().into(),
        ];
        let request = build_request(&args).unwrap();
        let truncated = request.body.unwrap()["accept_criteria"].as_str().unwrap().to_string();
        assert_eq!(truncated.len(), 65_534);
        assert!(truncated.ends_with('中'));

        // 未给 --accept-file 时请求体里没有该字段。
        let plain = build_request(&[
            "aicross".into(), "dispatch".into(), "--project".into(), "D:/p".into(),
            "--text-file".into(), root.join("t.txt").to_string_lossy().into(),
        ])
        .unwrap();
        assert!(plain.body.unwrap().get("accept_criteria").is_none());
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
        let root = crate::paths::test_temp_dir().join(format!("aicross-cli-run-{}", Uuid::new_v4()));
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
        let root = crate::paths::test_temp_dir().join(format!("aicross-cli-runs-dir-{}", Uuid::new_v4()));
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
        let root = crate::paths::test_temp_dir().join(format!("aicross-task39-{tag}-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    /// 资源库样板：claude 原生一条；pi 五条覆盖已验证 / 未验证 / 验证失败 /
    /// 未登录 / 额度用尽；kimi 未安装。同名 glm-4.7 挂两个 provider 供歧义用。
    /// 带 detectedAt：表示探测过，状态走 可用 / 未安装 两态（走查 #1）。
    fn write_fixture_store(root: &Path) {
        fs::write(
            root.join("resources.json"),
            r#"{"version":1,"detectedAt":"2026-10-06T09:00:00+08:00","agents":[
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

    /// TASK-66：`resources set-path` 记下、清除指定路径；未知 agent、文件不存在报参数错。
    #[test]
    fn task66_resources_set_path() {
        let root = fixture_root("set-path");
        write_fixture_store(&root);
        let program = root.join("pi.cmd");
        fs::write(&program, "@echo off\r\n").unwrap();
        let program_text = program.display().to_string();
        let ops = ResourcesOps::production();

        let (code, stdout, stderr) = run_with(&root, &["aicross", "resources", "set-path", "PI", &program_text], &ops);
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.contains("已指定 pi 的路径") && stderr.contains("重新探测后生效"), "{stderr}");
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["id"], "pi");
        assert_eq!(parsed["custom_path"], program_text.as_str());

        let (code, stdout, stderr) = run_with(&root, &["aicross", "resources", "set-path", "pi"], &ops);
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.contains("已清除 pi 的指定路径"), "{stderr}");
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["custom_path"], "");

        for (args, expected) in [
            (vec!["aicross", "resources", "set-path"], "用法"),
            (vec!["aicross", "resources", "set-path", "gemini", "x"], "未知 agent"),
            (vec!["aicross", "resources", "set-path", "pi", r"Z:\不存在\pi.cmd"], "文件不存在"),
            (vec!["aicross", "resources", "set-path", "pi", "a", "b"], "不认识的参数"),
        ] {
            let (code, _, stderr) = run_with(&root, &args, &ops);
            assert_eq!(code, 2, "{args:?}");
            assert!(stderr.contains(expected), "{args:?}: {stderr}");
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task39_resources_lists_states_without_probing() {
        let root = fixture_root("list");
        write_fixture_store(&root);
        let (code, stdout, stderr) = run_with(&root, &["aicross", "resources"], &ResourcesOps::production());
        assert_eq!(code, 0);
        assert!(stderr.contains("已读取资源库"), "{stderr}");
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        // 读取层的 ensure_agents 会补齐缺失的内置 agent（codex 不在样板里，补进来）；
        // TASK-61 起按支持清单补齐九家，每家带 untested 与 homepage。
        assert_eq!(parsed["agents"].as_array().unwrap().len(), 9);
        let agy = parsed["agents"].as_array().unwrap().iter()
            .find(|agent| agent["id"] == "agy").unwrap();
        assert_eq!(agy["untested"], false);
        assert_eq!(agy["homepage"], "https://antigravity.google");
        assert_eq!(agy["state"], "未安装");
        assert_eq!(agy["models"].as_array().unwrap().len(), 0);
        let cursor = parsed["agents"].as_array().unwrap().iter()
            .find(|agent| agent["id"] == "cursor").unwrap();
        assert_eq!(cursor["untested"], true);
        assert_eq!(cursor["homepage"], "https://cursor.com/cli");
        let kimi = parsed["agents"].as_array().unwrap().iter()
            .find(|agent| agent["id"] == "kimi").unwrap();
        assert_eq!(kimi["state"], "未安装");
        assert_eq!(kimi["note"], "没找到 kimi；安装后重新探测，或指定路径");
        assert_eq!(kimi["custom_path"], "");
        assert_eq!(kimi["version"], "未检测");
        // R1-4：agent 未安装时它下面的模型状态跟随 agent
        assert_eq!(kimi["models"].as_array().unwrap().len(), 1);
        assert_eq!(kimi["models"][0]["state"], "未安装");
        assert_eq!(kimi["models"][0]["note"], "安装后重新探测");
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
        assert_eq!(logged["note"], "未登录；在 pi 中运行 /login bigmodel 后重新探测");
        let quota = model_state_in(&stdout, "pi", "zai", "quota");
        assert_eq!(quota["state"], "额度用尽");
        assert_eq!(quota["note"], "额度恢复时间：2099-01-01T00:00:00+08:00");
        assert_eq!(model_state_in(&stdout, "claude", "", "sonnet")["state"], "可用");
        assert_eq!(model_state_in(&stdout, "claude", "", "sonnet")["resolved_id"], "claude-sonnet-5");
        // 只读：不探测、不写盘
        assert!(!stderr.contains("探测"), "{stderr}");
        let _ = fs::remove_dir_all(root);
    }

    /// TASK-42：冒烟超时的模型在 `resources` 输出里记「冒烟超时」，note 给
    /// 重试命令；不与「验证失败」混档。
    #[test]
    fn task42_resources_lists_smoke_timeout_with_retry_note() {
        let root = fixture_root("timeout-state");
        fs::write(
            root.join("resources.json"),
            r#"{"version":1,"detectedAt":"2026-10-06T09:00:00+08:00","agents":[
                {"id":"pi","name":"pi agent","kind":"内置","detected":true,"version":"0.86.1","models":[
                    {"id":"glm-5.3","agent":"pi","provider":"zai-coding-cn","status":"冒烟失败","verification":"timeout","smoke_failure":"冒烟超时（180 秒）","resolved_id":"glm-5.3","resolved_source":"alias-table"},
                    {"id":"bad","agent":"pi","provider":"zai-coding-cn","status":"冒烟失败","verification":"failed","smoke_failure":"403: denied","resolved_id":"bad","resolved_source":"alias-table"}
                ]}
            ]}"#,
        )
        .unwrap();
        let (code, stdout, stderr) =
            run_with(&root, &["aicross", "resources"], &ResourcesOps::production());
        assert_eq!(code, 0, "{stderr}");
        let timeout = model_state_in(&stdout, "pi", "zai-coding-cn", "glm-5.3");
        assert_eq!(timeout["state"], "冒烟超时");
        assert_eq!(timeout["note"], "连接测试超时，不影响派发");
        let failed = model_state_in(&stdout, "pi", "zai-coding-cn", "bad");
        assert_eq!(failed["state"], "验证失败");
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
                    skipped: Vec::new(),
                    store,
                })
            }),
            ..ResourcesOps::production()
        };
        let (code, stdout, stderr) = run_with(&root, &["aicross", "resources", "detect"], &ops);
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(*seen.lock().unwrap(), vec![Some("D:\\tools\\cc_switch.py".to_string())]);
        assert!(stderr.contains("探测 codex 失败"), "{stderr}");
        assert!(stderr.contains("已探测 9 个 agent"), "{stderr}");
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        let kimi = parsed["agents"].as_array().unwrap().iter()
            .find(|agent| agent["id"] == "kimi").unwrap();
        assert_eq!(kimi["state"], "可用");
        assert_eq!(kimi["version"], "1.2.3");
        let _ = fs::remove_dir_all(root);
    }

    /// TASK-52：`resources prune` 删掉未发现的模型，stdout 是资源形状外加
    /// removed / referenced，stderr 给说明；被 modes.json 人选引用的照样删，
    /// 另列 referenced；探测到与用户申报的都保留。
    #[test]
    fn task52_resources_prune_removes_missing_and_lists_referenced() {
        let root = fixture_root("prune");
        fs::write(
            root.join("resources.json"),
            r#"{"version":1,"agents":[
                {"id":"pi","name":"pi agent","kind":"内置","detected":true,"version":"0.86.1","models":[
                    {"id":"glm-5.3","agent":"pi","provider":"zai","status":"待冒烟","meta":"未在事实源中","resolved_id":"glm-5.3","resolved_source":"alias-table"},
                    {"id":"glm-4.7","agent":"pi","provider":"zai","status":"已接入","verification":"verified","resolved_id":"glm-4.7","resolved_source":"identity"},
                    {"id":"mine","agent":"pi","provider":"","status":"待冒烟","source":"申报(手动添加)","resolved_id":"mine","resolved_source":"alias-table"},
                    {"id":"ghost","agent":"pi","provider":"bigmodel","status":"待冒烟","meta":"未在事实源中；备注","resolved_id":"ghost","resolved_source":"alias-table"}
                ]}
            ]}"#,
        )
        .unwrap();
        // 人选引用 glm-5.3（provider null → 不限 provider）；ghost 没被引用。
        fs::write(
            root.join("modes.json"),
            r#"{"version":2,"hosts":{"pi":{"vendorOverride":"","defaultMode":"","seats":{"builtin-economy":[[{"agent":"pi","provider":null,"model":"glm-5.3","tier":"","thinking":"","fallback":null,"origin":"manual"}]]}}}}"#,
        )
        .unwrap();
        let (code, stdout, stderr) =
            run_with(&root, &["aicross", "resources", "prune"], &ResourcesOps::production());
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.contains("已清理 2 个未发现的模型"), "{stderr}");
        assert!(
            stderr.contains("其中 1 个仍被模式人选或角色引用：pi/zai/glm-5.3"),
            "{stderr}"
        );
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(
            parsed["removed"],
            serde_json::json!(["pi/zai/glm-5.3", "pi/bigmodel/ghost"])
        );
        assert_eq!(parsed["referenced"], serde_json::json!(["pi/zai/glm-5.3"]));
        let ids: Vec<&str> = parsed["agents"].as_array().unwrap().iter()
            .find(|agent| agent["id"] == "pi").unwrap()["models"].as_array().unwrap()
            .iter().map(|model| model["id"].as_str().unwrap()).collect();
        assert_eq!(ids, vec!["glm-4.7", "mine"], "探测到的与申报的都保留");
        let _ = fs::remove_dir_all(root);
    }

    /// TASK-52：`resources prune --agent` 只清这一家；未知 agent 与不认识的
    /// 选项、缺值的选项都退出 2。
    #[test]
    fn task52_resources_prune_agent_filter_and_argument_errors() {
        let root = fixture_root("prune-agent");
        fs::write(
            root.join("resources.json"),
            r#"{"version":1,"agents":[
                {"id":"pi","name":"pi agent","kind":"内置","detected":true,"version":"0.86.1","models":[
                    {"id":"glm-5.3","agent":"pi","provider":"zai","status":"待冒烟","meta":"未在事实源中","resolved_id":"glm-5.3","resolved_source":"alias-table"}
                ]},
                {"id":"codex","name":"Codex","kind":"内置","detected":true,"version":"0.2.0","models":[
                    {"id":"gpt-5.2","agent":"codex","provider":"","status":"待冒烟","meta":"未在事实源中","resolved_id":"gpt-5.2","resolved_source":"alias-table"}
                ]}
            ]}"#,
        )
        .unwrap();
        let (code, stdout, stderr) = run_with(
            &root,
            &["aicross", "resources", "prune", "--agent", "PI"],
            &ResourcesOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["removed"], serde_json::json!(["pi/zai/glm-5.3"]));
        let codex = parsed["agents"].as_array().unwrap().iter()
            .find(|agent| agent["id"] == "codex").unwrap();
        assert_eq!(codex["models"].as_array().unwrap().len(), 1, "别家的不动");
        let (code, _, stderr) = run_with(
            &root,
            &["aicross", "resources", "prune", "--agent", "gemini"],
            &ResourcesOps::production(),
        );
        assert_eq!(code, 2);
        assert!(stderr.contains("未知 agent：gemini"), "{stderr}");
        let (code, _, stderr) = run_with(
            &root,
            &["aicross", "resources", "prune", "--model", "x"],
            &ResourcesOps::production(),
        );
        assert_eq!(code, 2);
        assert!(stderr.contains("不认识的参数：--model"), "{stderr}");
        let (code, _, stderr) = run_with(
            &root,
            &["aicross", "resources", "prune", "--agent"],
            &ResourcesOps::production(),
        );
        assert_eq!(code, 2);
        assert!(stderr.contains("缺少 --agent 的值"), "{stderr}");
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
                    skipped: Vec::new(),
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
                stderr.contains("roles.json 无法解析；已停止写入，修复或删除该文件后重试"),
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

    fn run_config_acp(root: &Path, args: &[&str], acp: &AcpOps) -> (i32, String, String) {
        let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_config(root, &args, acp, &mut stdout, &mut stderr);
        (code, String::from_utf8(stdout).unwrap(), String::from_utf8(stderr).unwrap())
    }

    fn fake_acp(reports: Vec<AdapterReport>) -> (AcpOps, std::rc::Rc<std::cell::Cell<u32>>) {
        let calls = std::rc::Rc::new(std::cell::Cell::new(0));
        let counter = calls.clone();
        let installed = reports.clone();
        let ops = AcpOps {
            install: Box::new(move || {
                counter.set(counter.get() + 1);
                installed.clone()
            }),
            status: Box::new(move || reports.clone()),
        };
        (ops, calls)
    }

    fn report(agent: &'static str, state: acp_setup::AdapterState, reason: Option<&str>) -> AdapterReport {
        AdapterReport { agent, state, reason: reason.map(str::to_string) }
    }

    #[test]
    fn task45_config_set_acp_enabled_installs_and_reports() {
        let root = fixture_root("acp-on");
        let (ops, calls) = fake_acp(vec![
            report("claude", acp_setup::AdapterState::Ready, None),
            report("codex", acp_setup::AdapterState::Installed, None),
        ]);
        let (code, stdout, stderr) = run_config_acp(&root, &["aicross", "config", "set", "acpEnabled", "true"], &ops);
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(calls.get(), 1);
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["dispatch"]["acpEnabled"], true);
        assert_eq!(parsed["acp"][0], json!({"agent": "claude", "state": "已就绪"}));
        assert_eq!(parsed["acp"][1]["state"], "刚装好");
        assert_eq!(stderr.lines().count(), 1, "{stderr}");
        assert!(stderr.contains("claude 已就绪") && stderr.contains("codex 刚装好"), "{stderr}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task45_config_set_acp_enabled_failure_exits_1_and_keeps_setting() {
        let root = fixture_root("acp-fail");
        let (ops, calls) = fake_acp(vec![
            report("claude", acp_setup::AdapterState::NoNpm, Some("未检测到 npm（Node.js）")),
            report("codex", acp_setup::AdapterState::Failed, Some("npm error code ETIMEDOUT")),
        ]);
        let (code, stdout, stderr) = run_config_acp(&root, &["aicross", "config", "set", "acpEnabled", "true"], &ops);
        assert_eq!(code, 1, "{stderr}");
        assert_eq!(calls.get(), 1);
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["acp"][1]["state"], "安装失败");
        assert_eq!(parsed["acp"][1]["reason"], "npm error code ETIMEDOUT");
        assert!(stderr.contains("claude 没有 npm") && stderr.contains("codex 安装失败"), "{stderr}");
        assert!(stderr.contains("回退一次性 CLI"), "{stderr}");
        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("dispatch-settings.json")).unwrap()).unwrap();
        assert_eq!(saved["acpEnabled"], true);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task45_config_set_acp_disabled_does_not_install() {
        let root = fixture_root("acp-off");
        let (ops, calls) = fake_acp(vec![report("claude", acp_setup::AdapterState::Ready, None)]);
        let (code, stdout, stderr) = run_config_acp(&root, &["aicross", "config", "set", "acpEnabled", "false"], &ops);
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(calls.get(), 0);
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["acpEnabled"], false);
        assert!(parsed.get("acp").is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task45_config_show_lists_acp_status_without_installing() {
        let root = fixture_root("acp-show");
        let (ops, calls) = fake_acp(vec![
            report("claude", acp_setup::AdapterState::Ready, None),
            report("codex", acp_setup::AdapterState::Missing, None),
        ]);
        let (code, stdout, stderr) = run_config_acp(&root, &["aicross", "config", "show"], &ops);
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(calls.get(), 0);
        let parsed: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["acp"][0]["state"], "已就绪");
        assert_eq!(parsed["acp"][1]["state"], "未安装");
        let _ = fs::remove_dir_all(root);
    }

    // ── TASK-44：recover 子命令的参数检查与退出码 ──────────────────────

    fn recover_args(extra: &[&str]) -> Vec<String> {
        let mut args = vec!["aicross".to_string(), "recover".to_string()];
        args.extend(extra.iter().map(|arg| arg.to_string()));
        args
    }

    /// 直接把 run 文件写在临时引擎根下，不经过引擎内部函数。
    fn recover_cli_run(root: &Path, run_id: &str, status: &str, pid: u32) {
        let run = crate::engine::EngineRun {
            run_id: run_id.into(),
            status: status.into(),
            pid,
            ..crate::engine::EngineRun::default()
        };
        fs::create_dir_all(root.join("runs")).unwrap();
        fs::write(
            root.join("runs").join(format!("{run_id}.json")),
            serde_json::to_vec(&run).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn recover_requires_run_id_and_rejects_unknown_args() {
        let root = fixture_root("recover-args");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(run_at(&recover_args(&[]), &root, &mut stdout, &mut stderr), 2);
        assert!(stdout.is_empty());
        assert!(String::from_utf8(stderr).unwrap().contains("缺少 run_id"));
        // 以 -- 开头的首个参数不当 run_id：aicross recover --rollback 按缺
        // run_id 处理。
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(
            run_at(&recover_args(&["--rollback"]), &root, &mut stdout, &mut stderr),
            2
        );
        assert!(stdout.is_empty());
        assert!(String::from_utf8(stderr).unwrap().contains("缺少 run_id"));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(
            run_at(&recover_args(&["r1", "--wat"]), &root, &mut stdout, &mut stderr),
            2
        );
        assert!(String::from_utf8(stderr).unwrap().contains("不认识的参数"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recover_missing_run_exits_1() {
        let root = fixture_root("recover-missing");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(run_at(&recover_args(&["no-such-run"]), &root, &mut stdout, &mut stderr), 1);
        assert!(stdout.is_empty());
        assert!(String::from_utf8(stderr).unwrap().contains("读取 run 失败"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recover_running_run_exits_1_with_hint() {
        let root = fixture_root("recover-running");
        recover_cli_run(&root, "r1", "running", std::process::id());
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(run_at(&recover_args(&["r1"]), &root, &mut stdout, &mut stderr), 1);
        assert!(String::from_utf8(stderr).unwrap().contains("仍在运行"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recover_done_run_exits_0_with_no_need_note() {
        let root = fixture_root("recover-done");
        recover_cli_run(&root, "r1", "done", 0);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(
            run_at(&recover_args(&["r1", "--rollback"]), &root, &mut stdout, &mut stderr),
            0
        );
        let parsed: serde_json::Value = serde_json::from_str(&String::from_utf8(stdout).unwrap()).unwrap();
        assert_eq!(parsed["status"], "done");
        assert!(String::from_utf8(stderr).unwrap().contains("无需核对"));
        let _ = fs::remove_dir_all(root);
    }

    // ── TASK-46：模式人选与默认模式 ────────────────────────────────

    /// 写一个 modes.json：默认盲审，三路验证里的第一路给了人选。
    fn write_modes(root: &Path, default_mode: &str) {
        fs::write(
            root.join("modes.json"),
            format!(
                r#"{{"version":2,"hosts":{{"claude":{{"vendorOverride":"","defaultMode":"{default_mode}","seats":{{"builtin-blind":[[{{"agent":"pi","provider":"zai","model":"glm-4.7","tier":"","thinking":"中","fallback":null}},null,null]]}}}}}}}}"#
            ),
        )
        .unwrap();
    }

    fn parse_stdout(stdout: &str) -> serde_json::Value {
        serde_json::from_str(stdout.lines().next().unwrap()).unwrap()
    }

    #[test]
    fn task46_config_show_includes_default_mode_and_modes() {
        let root = fixture_root("t46-show");
        write_fixture_store(&root);
        write_modes(&root, "builtin-blind");
        let (code, stdout, stderr) = run_config_acp(&root, &["aicross", "config", "show"], &AcpOps::production());
        assert_eq!(code, 0, "{stderr}");
        let parsed = parse_stdout(&stdout);
        assert_eq!(parsed["defaultMode"], "builtin-blind");
        assert_eq!(parsed["modes"].as_array().unwrap().len(), 6);
        let blind = parsed["modes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|mode| mode["id"] == "builtin-blind")
            .unwrap();
        assert_eq!(blind["name"], "盲审模式");
        assert_eq!(blind["builtin"], true);
        assert_eq!(blind["default"], true);
        let first = &blind["stages"][0][0];
        assert_eq!(first["role"], "验收");
        assert_eq!(first["visibility"], "盲");
        assert_eq!(first["write"], "只读");
        assert_eq!(first["presetThinking"], "中");
        assert_eq!(first["source"], "位置");
        assert_eq!(first["seat"]["agent"], "pi");
        assert_eq!(first["effective"]["agent"], "pi");
        assert_eq!(first["effective"]["thinking"], "中");
        assert_eq!(first["effective"]["fallback"], serde_json::Value::Null);
        let second = &blind["stages"][0][1];
        assert_eq!(second["source"], "未指定");
        assert_eq!(second["seat"], serde_json::Value::Null);
        assert_eq!(second["effective"], serde_json::Value::Null);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task46_config_show_defaults_when_modes_missing() {
        let root = fixture_root("t46-show-empty");
        let (ops, _) = fake_acp(vec![]);
        let (code, stdout, _) = run_config_acp(&root, &["aicross", "config", "show"], &ops);
        assert_eq!(code, 0);
        let parsed = parse_stdout(&stdout);
        assert_eq!(parsed["defaultMode"], "builtin-economy");
        let economy = parsed["modes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|mode| mode["id"] == "builtin-economy")
            .unwrap();
        assert_eq!(economy["default"], true);
        // 没设角色默认，所有位置都是未指定。
        for stage in economy["stages"].as_array().unwrap() {
            for node in stage.as_array().unwrap() {
                assert_eq!(node["source"], "未指定");
                assert_eq!(node["effective"], serde_json::Value::Null);
            }
        }
        let _ = fs::remove_dir_all(root);
    }

    /// 走查 #1：资源库从未探测（detectedAt 为空）时，agent 与其下模型都
    /// 显示「未探测」，agent 的 note 为「尚未探测」、模型 note 为空；探测
    /// 过的（有 detectedAt）保持 可用 / 未安装 两态。
    #[test]
    fn task57_resources_reports_never_detected_before_first_probe() {
        // 没写过 resources.json：从未探测。
        let root = fixture_root("t57-never-detected");
        let (code, stdout, _) = run_with(&root, &["aicross", "resources"], &ResourcesOps::production());
        assert_eq!(code, 0);
        let parsed = parse_stdout(&stdout);
        assert_eq!(parsed["detected_at"], serde_json::Value::Null);
        for agent in parsed["agents"].as_array().unwrap() {
            assert_eq!(agent["state"], resources::RESOURCE_STATE_NEVER_DETECTED);
            assert_eq!(agent["note"], resources::NEVER_DETECTED_NOTE);
        }
        let _ = fs::remove_dir_all(root);

        // 有模型但没 detectedAt（旧库手搬）：同样按未探测显示。
        let root = fixture_root("t57-never-detected-models");
        fs::write(
            root.join("resources.json"),
            r#"{"version":1,"agents":[
                {"id":"pi","name":"pi agent","kind":"内置","detected":true,"version":"0.86.1","models":[
                    {"id":"glm-4.7","agent":"pi","provider":"zai","status":"已接入","verification":"verified","resolved_id":"glm-4.7","resolved_source":"identity"}
                ]}
            ]}"#,
        )
        .unwrap();
        let (code, stdout, _) = run_with(&root, &["aicross", "resources"], &ResourcesOps::production());
        assert_eq!(code, 0);
        let parsed = parse_stdout(&stdout);
        let pi = parsed["agents"].as_array().unwrap().iter()
            .find(|agent| agent["id"] == "pi").unwrap();
        assert_eq!(pi["state"], resources::RESOURCE_STATE_NEVER_DETECTED);
        assert_eq!(pi["note"], resources::NEVER_DETECTED_NOTE);
        assert_eq!(pi["models"][0]["state"], resources::RESOURCE_STATE_NEVER_DETECTED);
        assert_eq!(pi["models"][0]["note"], "");
        let _ = fs::remove_dir_all(root);
    }

    /// 走查 #7：评审模式各位置带 focus（按扁平序 安全 / 性能 / 需求覆盖，
    /// 超出为空串），数量调到 1、2 时相应变短；其余模式一律空串。
    #[test]
    fn task57_config_show_focus_marks_jury_positions() {
        let root = fixture_root("t57-focus");
        write_fixture_store(&root);
        let (ops, _) = fake_acp(vec![]);
        let focus_of = |stdout: &str, id: &str| -> Vec<String> {
            let parsed = parse_stdout(stdout);
            // config show 给 modes 数组；set-count 的输出直接就是该模式那一项。
            let mode = match parsed["modes"].as_array() {
                Some(modes) => modes
                    .iter()
                    .find(|mode| mode["id"] == id)
                    .unwrap()
                    .clone(),
                None => parsed,
            };
            mode["stages"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|stage| stage.as_array().unwrap())
                .map(|node| node["focus"].as_str().unwrap().to_string())
                .collect()
        };
        let (code, stdout, stderr) = run_config_acp(&root, &["aicross", "config", "show"], &ops);
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(focus_of(&stdout, "builtin-jury"), vec!["安全", "性能", "需求覆盖"]);
        assert_eq!(focus_of(&stdout, "builtin-shared"), vec![""]);
        assert_eq!(
            focus_of(&stdout, "builtin-blind"),
            vec!["", "", ""]
        );
        // 数量调到 2、1：focus 相应变短。
        for (count, expected) in [("2", vec!["安全", "性能"]), ("1", vec!["安全"])] {
            let (code, stdout, stderr) = run_config_acp(
                &root,
                &["aicross", "config", "set-count", "builtin-jury", count, "--host", "claude"],
                &ops,
            );
            assert_eq!(code, 0, "{stderr}");
            assert_eq!(focus_of(&stdout, "builtin-jury"), expected, "count {count}");
        }
        let _ = fs::remove_dir_all(root);
    }

    /// TASK-59：`config set-focus` 写入评审视角并在 config show 生效；
    /// 评审模式节点带 focusOptions；--focus "" 恢复该位置的默认；数量调
    /// 小后多余的视角项不删，调大后继续生效；输出为该模式那一项（同
    /// config show 的 modes[i]）。
    #[test]
    fn task59_config_set_focus_writes_and_shows() {
        let root = fixture_root("t59-set-focus");
        write_fixture_store(&root);
        let (ops, _) = fake_acp(vec![]);
        let set_focus = |index: &str, focus: &str| {
            run_config_acp(
                &root,
                &[
                    "aicross", "config", "set-focus",
                    "--host", "claude",
                    "--mode", "builtin-jury",
                    "--index", index,
                    "--focus", focus,
                ],
                &ops,
            )
        };
        let focuses_of = |stdout: &str| -> Vec<String> {
            let parsed = parse_stdout(stdout);
            // config show 给 modes 数组；set-focus 的输出直接就是该模式那一项。
            let mode = match parsed["modes"].as_array() {
                Some(modes) => modes
                    .iter()
                    .find(|mode| mode["id"] == "builtin-jury")
                    .unwrap()
                    .clone(),
                None => parsed,
            };
            mode["stages"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|stage| stage.as_array().unwrap())
                .map(|node| node["focus"].as_str().unwrap().to_string())
                .collect()
        };
        // 写入内置视角与自定义文字；输出就是该模式那一项（含 focusOptions）。
        let (code, stdout, stderr) = set_focus("1", "规范");
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(focuses_of(&stdout), vec!["规范", "性能", "需求覆盖"]);
        assert_eq!(
            parse_stdout(&stdout)["stages"][0][0]["focusOptions"],
            serde_json::json!(["安全", "性能", "需求覆盖", "规范", "正确性", "可维护性"])
        );
        let (code, stdout, stderr) = set_focus("3", "可靠性检查");
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(focuses_of(&stdout), vec!["规范", "性能", "可靠性检查"]);
        // 模式介绍随生效视角改写。
        assert_eq!(
            parse_stdout(&stdout)["description"],
            "多个模型分别从规范、性能、可靠性检查角度审查"
        );
        // config show 读到同一份。
        let (code, stdout, stderr) = run_config_acp(&root, &["aicross", "config", "show"], &ops);
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(focuses_of(&stdout), vec!["规范", "性能", "可靠性检查"]);
        // --focus "" 恢复该位置的默认。
        let (code, stdout, stderr) = run_config_acp(
            &root,
            &[
                "aicross", "config", "set-focus",
                "--host", "claude",
                "--mode", "builtin-jury",
                "--index", "1",
                "--focus", "",
            ],
            &ops,
        );
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(focuses_of(&stdout), vec!["安全", "性能", "可靠性检查"]);
        // 数量调到 2：第三位的视角项不删（显示不出），调回 3 后恢复。
        let (code, stdout, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-count", "builtin-jury", "2", "--host", "claude"],
            &ops,
        );
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(focuses_of(&stdout), vec!["安全", "性能"]);
        let (code, _, stderr) = set_focus("3", "规范");
        assert_eq!(code, 2);
        assert!(stderr.contains("位置序号超出范围"), "{stderr}");
        let (code, stdout, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-count", "builtin-jury", "3", "--host", "claude"],
            &ops,
        );
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(focuses_of(&stdout), vec!["安全", "性能", "可靠性检查"]);
        let _ = fs::remove_dir_all(root);
    }

    /// TASK-59：`config set-focus` 的参数校验：其它模式与不存在的模式、
    /// 序号越界与非数字、非法视角文字（超长、换行、制表、ANSI 转义）、
    /// 缺选项都退出 2；失败不写盘，focus 仍是默认。
    #[test]
    fn task59_config_set_focus_rejects_bad_input() {
        let root = fixture_root("t59-set-focus-args");
        write_fixture_store(&root);
        let run = |extra: &[&str]| {
            let mut args: Vec<&str> = vec!["aicross", "config", "set-focus", "--host", "claude"];
            args.extend_from_slice(extra);
            run_config_acp(&root, &args, &AcpOps::production())
        };
        let too_long = "长".repeat(21);
        for (extra, expect) in [
            (vec![], "缺少 --mode"),
            (vec!["--mode", "builtin-jury"], "缺少 --index"),
            (vec!["--mode", "builtin-jury", "--index", "1"], "缺少 --focus"),
            (vec!["--mode", "blind", "--index", "1", "--focus", "规范"], "不可设置"),
            (vec!["--mode", "ghost", "--index", "1", "--focus", "规范"], "模式不存在：ghost"),
            (vec!["--mode", "builtin-jury", "--index", "0", "--focus", "规范"], "位置序号超出范围"),
            (vec!["--mode", "builtin-jury", "--index", "4", "--focus", "规范"], "位置序号超出范围"),
            (vec!["--mode", "builtin-jury", "--index", "x", "--focus", "规范"], "位置序号必须是正整数"),
            (vec!["--mode", "builtin-jury", "--index", "1", "--focus", too_long.as_str()], "1–20 字"),
            (vec!["--mode", "builtin-jury", "--index", "1", "--focus", "两\n行"], "控制字符"),
            (vec!["--mode", "builtin-jury", "--index", "1", "--focus", "含\t制表"], "控制字符"),
            (vec!["--mode", "builtin-jury", "--index", "1", "--focus", "\u{1b}[31m红"], "控制字符"),
            // fix-59-r1：首尾的换行、制表在 trim 前就拒绝；格式字符也拒绝。
            (vec!["--mode", "builtin-jury", "--index", "1", "--focus", "规范\n"], "控制字符"),
            (vec!["--mode", "builtin-jury", "--index", "1", "--focus", "\t规范"], "控制字符"),
            (vec!["--mode", "builtin-jury", "--index", "1", "--focus", "规\u{200b}范"], "格式字符"),
            (vec!["--mode", "builtin-jury", "--index", "1", "--focus", "\u{202e}规范"], "格式字符"),
            (vec!["--mode", "builtin-jury", "--index", "1", "--focus", "规范\u{feff}"], "格式字符"),
        ] {
            let (code, stdout, stderr) = run(&extra);
            assert_eq!(code, 2, "{extra:?} → {stdout} {stderr}");
            assert!(stderr.contains(expect), "{extra:?} → {stderr}");
        }
        // 失败不写盘：focus 仍是默认。
        let (code, stdout, stderr) =
            run_config_acp(&root, &["aicross", "config", "show"], &AcpOps::production());
        assert_eq!(code, 0, "{stderr}");
        let parsed = parse_stdout(&stdout);
        let jury = parsed["modes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|mode| mode["id"] == "builtin-jury")
            .unwrap();
        let focuses: Vec<String> = jury["stages"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|stage| stage.as_array().unwrap())
            .map(|node| node["focus"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(focuses, vec!["安全", "性能", "需求覆盖"]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task46_config_set_default_mode_accepts_alias_and_id() {
        let root = fixture_root("t46-default");
        write_fixture_store(&root);
        // 别名。
        let (code, stdout, stderr) =
            run_config_acp(&root, &["aicross", "config", "set-default-mode", "blind", "--host", "claude"], &AcpOps::production());
        assert_eq!(code, 0, "{stderr}");
        let parsed = parse_stdout(&stdout);
        assert_eq!(parsed["defaultMode"], "builtin-blind");
        assert!(stderr.contains("已写入默认模式"), "{stderr}");
        // 写盘后 config show 读回一致。
        let (_, stdout, _) = run_config_acp(&root, &["aicross", "config", "show"], &AcpOps::production());
        assert_eq!(parse_stdout(&stdout)["defaultMode"], "builtin-blind");
        // 预设 id。
        let (code, stdout, _) =
            run_config_acp(&root, &["aicross", "config", "set-default-mode", "builtin-economy", "--host", "claude"], &AcpOps::production());
        assert_eq!(code, 0);
        assert_eq!(parse_stdout(&stdout)["defaultMode"], "builtin-economy");
        // 未知模式退出 2，stderr 列出可选 id 与名称。
        let (code, stdout, stderr) =
            run_config_acp(&root, &["aicross", "config", "set-default-mode", "ghost", "--host", "claude"], &AcpOps::production());
        assert_eq!(code, 2);
        assert!(stdout.is_empty());
        assert!(stderr.contains("模式不存在：ghost"), "{stderr}");
        assert!(stderr.contains("builtin-blind（盲审模式）"), "{stderr}");
        assert!(stderr.contains("builtin-economy（经济模式）"), "{stderr}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task46_config_set_seat_writes_and_show_reads_back() {
        let root = fixture_root("t46-set-seat");
        write_fixture_store(&root);
        let (code, stdout, stderr) = run_config_acp(
            &root,
            &[
                "aicross", "config", "set-seat", "blind", "1", "2", "--host", "claude",
                "--agent", "PI", "--model", "glm-4.7", "--provider", "zai",
                "--tier", "高", "--thinking", "低",
                "--fallback-agent", "claude", "--fallback-model", "sonnet",
            ],
            &AcpOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        let parsed = parse_stdout(&stdout);
        assert_eq!(parsed["id"], "builtin-blind");
        let second = &parsed["stages"][0][1];
        assert_eq!(second["source"], "位置");
        // agent 名大小写不敏感，回写规范 id；provider 取资源库规范值。
        assert_eq!(second["seat"]["agent"], "pi");
        assert_eq!(second["seat"]["provider"], "zai");
        assert_eq!(second["seat"]["model"], "glm-4.7");
        assert_eq!(second["seat"]["tier"], "高");
        assert_eq!(second["seat"]["thinking"], "低");
        let fallback = &second["seat"]["fallback"];
        assert_eq!(fallback["agent"], "claude");
        assert_eq!(fallback["provider"], serde_json::Value::Null);
        assert_eq!(fallback["model"], "sonnet");
        assert_eq!(second["effective"]["tier"], "高", "seat 非空覆盖预设");
        assert_eq!(second["effective"]["thinking"], "低");
        // 首个位置与第三个位置没动。
        assert_eq!(parsed["stages"][0][0]["seat"], serde_json::Value::Null);
        assert_eq!(parsed["stages"][0][2]["seat"], serde_json::Value::Null);
        assert!(stderr.contains("已写入模式人选"), "{stderr}");
        // config show 读回一致。
        let (_, stdout, _) = run_config_acp(&root, &["aicross", "config", "show"], &AcpOps::production());
        let show = parse_stdout(&stdout);
        let blind = show["modes"].as_array().unwrap().iter().find(|mode| mode["id"] == "builtin-blind").unwrap();
        assert_eq!(blind["stages"][0][1], parsed["stages"][0][1]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task46_config_set_seat_rejects_bad_arguments() {
        let root = fixture_root("t46-seat-args");
        write_fixture_store(&root);
        for (args, expect) in [
            (vec!["aicross", "config", "set-seat"], "缺少模式"),
            (vec!["aicross", "config", "set-seat", "blind"], "缺少阶段"),
            (vec!["aicross", "config", "set-seat", "blind", "1"], "缺少位置"),
            (vec!["aicross", "config", "set-seat", "ghost", "1", "1", "--host", "claude", "--agent", "pi", "--model", "glm-4.7"], "模式不存在：ghost"),
            (vec!["aicross", "config", "set-seat", "blind", "2", "1", "--host", "claude", "--agent", "pi", "--model", "glm-4.7", "--provider", "zai"], "阶段超出范围"),
            (vec!["aicross", "config", "set-seat", "blind", "1", "4", "--host", "claude", "--agent", "pi", "--model", "glm-4.7", "--provider", "zai"], "位置超出范围"),
            (vec!["aicross", "config", "set-seat", "blind", "0", "1", "--host", "claude", "--agent", "pi", "--model", "glm-4.7", "--provider", "zai"], "阶段超出范围"),
            (vec!["aicross", "config", "set-seat", "blind", "1", "0", "--host", "claude", "--agent", "pi", "--model", "glm-4.7", "--provider", "zai"], "位置超出范围"),
            (vec!["aicross", "config", "set-seat", "blind", "x", "1", "--host", "claude", "--agent", "pi", "--model", "glm-4.7"], "阶段必须是正整数"),
            (vec!["aicross", "config", "set-seat", "blind", "1", "x", "--host", "claude", "--agent", "pi", "--model", "glm-4.7"], "位置必须是正整数"),
            (vec!["aicross", "config", "set-seat", "blind", "1", "1", "--host", "claude", "--thinking", "超高"], "--thinking 只认"),
            (vec!["aicross", "config", "set-seat", "blind", "1", "1", "--host", "claude", "--agent", "ghost", "--model", "x"], "未知 agent：ghost"),
            (vec!["aicross", "config", "set-seat", "blind", "1", "1", "--host", "claude", "--agent", "pi", "--model", "ghost"], "模型不存在：pi/ghost"),
            (vec!["aicross", "config", "set-seat", "blind", "1", "1", "--host", "claude", "--agent", "pi", "--model", "glm-4.7"], "多个 provider"),
            (vec!["aicross", "config", "set-seat", "blind", "1", "1", "--host", "claude", "--agent", "pi", "--model", "glm-4.7", "--provider", "zai", "--fallback-agent", "ghost", "--fallback-model", "sonnet"],
                "未知 agent：ghost"),
            (
                vec!["aicross", "config", "set-seat", "blind", "1", "1", "--host", "claude", "--agent", "pi", "--model", "glm-4.7", "--fallback-agent", "claude"],
                "备选需要 --fallback-agent 与 --fallback-model 成对给",
            ),
            (
                vec!["aicross", "config", "set-seat", "blind", "1", "1", "--host", "claude", "--agent", "pi", "--model", "glm-4.7", "--fallback-model", "sonnet"],
                "备选需要 --fallback-agent 与 --fallback-model 成对给",
            ),
            (
                vec!["aicross", "config", "set-seat", "blind", "1", "1", "--host", "claude", "--agent", "pi", "--model", "glm-4.7", "--fallback-provider", "zai"],
                "--fallback-provider 需要 --fallback-agent 与 --fallback-model",
            ),
            (
                vec!["aicross", "config", "set-seat", "blind", "1", "1", "--host", "claude", "--agent", "claude", "--model", "sonnet", "--fallback-agent", "pi", "--fallback-model", "ghost"],
                "模型不存在：pi/ghost",
            ),
            (vec!["aicross", "config", "set-seat", "blind", "1", "1", "--host", "claude", "--agent", "pi", "--wat"], "不认识的参数"),
        ] {
            let (code, stdout, stderr) = run_config_acp(&root, &args, &AcpOps::production());
            assert_eq!(code, 2, "{args:?} → {stderr}");
            assert!(stdout.is_empty(), "{args:?} → {stdout}");
            assert!(stderr.contains(expect), "{args:?} → {stderr}");
        }
        // 缺 --agent / --model（有阶段位置但没有选项）。
        let (code, _, stderr) =
            run_config_acp(&root, &["aicross", "config", "set-seat", "blind", "1", "1", "--host", "claude"], &AcpOps::production());
        assert_eq!(code, 2);
        assert!(stderr.contains("缺少 --agent"), "{stderr}");
        // 失败不写盘。
        assert!(!root.join("modes.json").exists());
        let _ = fs::remove_dir_all(root);
    }

    /// 严格读取：modes.json 损坏时退出 1 且文件内容不变。
    #[test]
    fn task46_config_seat_writes_reject_broken_modes_file() {
        let root = fixture_root("t46-seat-broken");
        write_fixture_store(&root);
        fs::write(root.join("modes.json"), "{ 损坏").unwrap();
        let before = fs::read_to_string(root.join("modes.json")).unwrap();
        for args in [
            vec!["aicross", "config", "set-seat", "blind", "1", "1", "--host", "claude", "--agent", "pi", "--model", "glm-4.7", "--provider", "zai"],
            vec!["aicross", "config", "set-default-mode", "blind", "--host", "claude"],
            vec!["aicross", "config", "clear-seat", "blind", "--host", "claude"],
        ] {
            let (code, stdout, stderr) = run_config_acp(&root, &args, &AcpOps::production());
            assert_eq!(code, 1, "{args:?} → {stderr}");
            assert!(stdout.is_empty(), "{args:?} → {stdout}");
            assert!(stderr.contains("modes.json 无法解析"), "{args:?} → {stderr}");
            assert_eq!(fs::read_to_string(root.join("modes.json")).unwrap(), before);
        }
        // 展示按宽松读取：损坏照样显示空配置，不报错。
        let (code, stdout, _) = run_config_acp(&root, &["aicross", "config", "show"], &AcpOps::production());
        assert_eq!(code, 0);
        assert_eq!(parse_stdout(&stdout)["defaultMode"], "builtin-economy");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task46_config_clear_seat_single_and_all() {
        let root = fixture_root("t46-clear");
        write_fixture_store(&root);
        for index in ["1", "3"] {
            let (code, _, stderr) = run_config_acp(
                &root,
                &[
                    "aicross", "config", "set-seat", "blind", "1", index,
                    "--host", "claude", "--agent", "pi", "--model", "glm-4.7", "--provider", "zai",
                ],
                &AcpOps::production(),
            );
            assert_eq!(code, 0, "{stderr}");
        }
        // 只清一个位置（置 null）。
        let (code, stdout, stderr) =
            run_config_acp(&root, &["aicross", "config", "clear-seat", "blind", "1", "1", "--host", "claude"], &AcpOps::production());
        assert_eq!(code, 0, "{stderr}");
        let parsed = parse_stdout(&stdout);
        assert_eq!(parsed["stages"][0][0]["seat"], serde_json::Value::Null);
        assert_eq!(parsed["stages"][0][0]["source"], "未指定");
        assert_eq!(parsed["stages"][0][2]["seat"]["agent"], "pi", "其它位置不动");
        // 清掉该模式全部人选（删 seats 键）。
        let (code, stdout, stderr) =
            run_config_acp(&root, &["aicross", "config", "clear-seat", "blind", "--host", "claude"], &AcpOps::production());
        assert_eq!(code, 0, "{stderr}");
        let parsed = parse_stdout(&stdout);
        for node in parsed["stages"][0].as_array().unwrap() {
            assert_eq!(node["seat"], serde_json::Value::Null);
            assert_eq!(node["source"], "未指定");
        }
        let saved = fs::read_to_string(root.join("modes.json")).unwrap();
        assert!(!saved.contains("builtin-blind\": ["), "seats 键应删掉：{saved}");
        // 清空后再清一次仍然成功（幂等）。
        let (code, _, _) = run_config_acp(&root, &["aicross", "config", "clear-seat", "blind", "--host", "claude"], &AcpOps::production());
        assert_eq!(code, 0);
        let _ = fs::remove_dir_all(root);
    }

    /// TASK-61：只接盲位置的 agent 写进共享位置（人选或备选）时 set-seat 拦住，
    /// 退出码 2；盲位置照常写入。
    #[test]
    fn task61_set_seat_rejects_blind_only_agent_on_shared_seat() {
        let root = fixture_root("t61-seat");
        fs::write(
            root.join("resources.json"),
            r#"{"version":1,"detectedAt":"2026-10-07T09:00:00+08:00","agents":[
                {"id":"agy","name":"Antigravity","kind":"内置","detected":true,"version":"1.3.1","models":[
                    {"id":"gemini-3.8-flash-low","agent":"agy","provider":"","vendor":"Google","verification":"native"}
                ]},
                {"id":"pi","name":"pi agent","kind":"内置","detected":true,"version":"0.86.1","models":[
                    {"id":"glm-4.7","agent":"pi","provider":"zai","vendor":"Zhipu","verification":"verified"}
                ]}
            ]}"#,
        )
        .unwrap();
        let agy = || SeatChoice {
            agent: "agy".into(),
            model: "gemini-3.8-flash-low".into(),
            ..SeatChoice::default()
        };
        let (code, message) = config_set_seat_at(&root, "claude", "shared", 1, 1, agy()).unwrap_err();
        assert_eq!(code, 2);
        assert!(message.contains("纯文本盲审") && message.contains("共享"), "{message}");
        let with_fallback = SeatChoice {
            agent: "pi".into(),
            provider: Some("zai".into()),
            model: "glm-4.7".into(),
            fallback: Some(FallbackTarget {
                agent: "agy".into(),
                provider: None,
                model: "gemini-3.8-flash-low".into(),
            }),
            ..SeatChoice::default()
        };
        let (code, _) = config_set_seat_at(&root, "claude", "shared", 1, 1, with_fallback).unwrap_err();
        assert_eq!(code, 2);
        let value = config_set_seat_at(&root, "claude", "blind", 1, 1, agy()).unwrap();
        assert_eq!(value["stages"][0][0]["seat"]["agent"], "agy");
        // fix-61-r1 5：effective 的思考强度记实际生效值（模型自带档位）
        assert_eq!(value["stages"][0][0]["effective"]["thinking"], "由模型决定：low");
        assert_eq!(value["stages"][0][0]["effective"]["invalid"], Value::Null);
        let _ = fs::remove_dir_all(root);
    }

    /// fix-61-r1 2：set-role 把只接盲的 agent 设成 planning / execution 退出 2
    /// （角色默认补的是共享位置）；acceptance 照常写入。`config show` 对生效
    /// 人选违反位置硬规则的组合（如旧 roles.json 里留下的执行角色默认）标出原因。
    #[test]
    fn task61_r1_set_role_rejects_blind_only_agent_and_show_marks_invalid() {
        let root = fixture_root("t61-r1-role");
        fs::write(
            root.join("resources.json"),
            r#"{"version":1,"detectedAt":"2026-10-07T09:00:00+08:00","agents":[
                {"id":"agy","name":"Antigravity","kind":"内置","detected":true,"version":"1.3.1","models":[
                    {"id":"gemini-3.8-flash-low","agent":"agy","provider":"","vendor":"Google","verification":"native"}
                ]}
            ]}"#,
        )
        .unwrap();
        let acp = AcpOps::production();
        for role in ["planning", "execution"] {
            let (code, _, stderr) = run_config_acp(
                &root,
                &["aicross", "config", "set-role", role, "--agent", "agy", "--model", "gemini-3.8-flash-low"],
                &acp,
            );
            assert_eq!(code, 2, "{role}: {stderr}");
            assert!(stderr.contains("纯文本盲审") && stderr.contains(role), "{stderr}");
        }
        assert!(!root.join("roles.json").exists(), "拒绝时不写盘");
        let (code, _, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-role", "acceptance", "--agent", "agy", "--model", "gemini-3.8-flash-low"],
            &acp,
        );
        assert_eq!(code, 0, "{stderr}");
        // 旧 roles.json 里留下的非法执行角色默认：不补位（fix-fullcheck A5），
        // config show 显示未指定并给原因。
        let mut roles = crate::orchestration::roles_from_strict(&root).unwrap();
        roles.execution = RoleProfile {
            agent: "agy".into(),
            provider: String::new(),
            model: "gemini-3.8-flash-low".into(),
            tier: String::new(),
            thinking: "中".into(),
        };
        crate::orchestration::save_roles_in(&root, &roles).unwrap();
        let show = config_show_value(&root, Some("claude"), &[]);
        let shared = show["modes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|mode| mode["id"] == "builtin-shared")
            .unwrap();
        let node = &shared["stages"][0][0];
        assert_eq!(node["effective"], Value::Null, "{node}");
        assert_eq!(node["source"], "未指定");
        let reason = node["unfilledReason"].as_str().unwrap();
        assert!(reason.contains("agy/gemini-3.8-flash-low") && reason.contains("纯文本盲审"), "{reason}");
        let _ = fs::remove_dir_all(root);
    }

    /// fix-fullcheck A5：验收角色默认设成 agy（只看得到文字，不填验收位），
    /// 推荐凑不出合规的验收人选：保存后 `config show` 里验收位是未指定并
    /// 给原因，派发计划里验收节点也没有退回 agy。
    #[test]
    fn a5_illegal_role_default_not_used_after_recommend() {
        let root = fixture_root("a5-role-default");
        let home = fixture_root("a5-home");
        fs::write(
            root.join("resources.json"),
            r#"{"version":1,"detectedAt":"2026-10-07T09:00:00+08:00","agents":[
                {"id":"claude","name":"Claude Code","kind":"内置","detected":true,"models":[
                    {"id":"sonnet","agent":"claude","provider":"","vendor":"Anthropic","verification":"native"}
                ]},
                {"id":"agy","name":"Antigravity","kind":"内置","detected":true,"version":"1.3.1","models":[
                    {"id":"gemini-3.8-flash-low","agent":"agy","provider":"","vendor":"Google","verification":"native"}
                ]}
            ]}"#,
        )
        .unwrap();
        let (code, _, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-role", "acceptance", "--agent", "agy", "--model", "gemini-3.8-flash-low"],
            &AcpOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        let recommended =
            crate::recommend::recommend_at_home(&root, &home, Some("claude"), Some("economy"), true, None)
                .unwrap();
        let notes = recommended["notes"].as_array().unwrap();
        assert!(
            notes.iter().any(|note| note.as_str().unwrap().contains("不合规")),
            "{notes:?}"
        );
        // 保存后的 config show：验收位未指定，附原因。
        let show = config_show_value(&root, Some("claude"), &[]);
        let economy = show["modes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|mode| mode["id"] == "builtin-economy")
            .unwrap();
        let acceptance = &economy["stages"][2][0];
        assert_eq!(acceptance["role"], "验收");
        assert_eq!(acceptance["source"], "未指定", "{acceptance}");
        assert_eq!(acceptance["effective"], Value::Null);
        assert!(
            acceptance["unfilledReason"].as_str().unwrap().contains("读不到项目副本"),
            "{acceptance}"
        );
        // 派发计划：与派发同一份补人，验收节点的人选不是 agy。
        let modes = crate::orchestration::modes_from(&root);
        let presets = crate::orchestration::raw_presets_from(&root).presets;
        let roles = crate::orchestration::roles_from(&root);
        let models: Vec<_> = resources::get_resources_at(&store_file(&root))
            .unwrap()
            .agents
            .into_iter()
            .flat_map(|agent| agent.models)
            .collect();
        let settings = crate::engine::EngineSettings::default();
        let mut git_dirty_count = || None;
        let mut seq = 0usize;
        let mut ids = move || {
            seq += 1;
            format!("00000000-0000-4000-8000-{seq:012}")
        };
        let mut clock = chrono::Utc::now;
        let mut input = crate::engine::PlanInput {
            run_id: "a5",
            project: "D:/a5",
            text: "任务",
            preset: Some("builtin-economy"),
            attachments: &[],
            targets: &[],
            settings: &settings,
            models: &models,
            resources_error: None,
            presets: &presets,
            roles: &roles,
            host_modes: modes.hosts.get("claude"),
            default_preset: "builtin-economy",
            active_runs: 0,
            existing: None,
            git_dirty_count: &mut git_dirty_count,
        };
        match crate::engine::plan_dispatch(&mut input, &mut ids, &mut clock) {
            crate::engine::Plan::Ready { graph, .. } => {
                let acceptance = graph
                    .nodes
                    .values()
                    .filter_map(|node| node.target.as_ref())
                    .find(|target| target.role == "验收")
                    .expect("验收节点");
                assert_ne!(acceptance.agent, "agy", "{acceptance:?}");
                assert!(acceptance.model.trim().is_empty(), "{acceptance:?}");
            }
            crate::engine::Plan::Rejected { reason, .. } => panic!("{reason}"),
        }
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(home);
    }

    /// 窗口命令背后的函数：返回结构与命令行一致。
    #[test]
    fn task46_window_commands_match_cli_shapes() {
        let root = fixture_root("t46-window");
        write_fixture_store(&root);
        // config_snapshot：{ resources, config }，config 段同 config show。
        let snapshot = config_snapshot_at(&root, Some("claude"), &[]).unwrap();
        assert!(snapshot["resources"]["agents"].as_array().is_some());
        let (_, cli_stdout, _) = run_config_acp(&root, &["aicross", "config", "show"], &AcpOps::production());
        let config = config_show_value(&root, Some("claude"), &[]);
        assert_eq!(snapshot["config"]["defaultMode"], parse_stdout(&cli_stdout)["defaultMode"]);
        assert_eq!(config["modes"], parse_stdout(&cli_stdout)["modes"]);
        // config_set_seat：返回值同 set-seat 的 stdout（手选 origin 为 manual）。
        let seat = SeatChoice {
            agent: "pi".into(),
            provider: Some("zai".into()),
            model: "glm-4.7".into(),
            ..SeatChoice::default()
        };
        let value = config_set_seat_at(&root, "claude", "blind", 1, 1, seat).unwrap();
        assert_eq!(value["id"], "builtin-blind");
        assert_eq!(value["stages"][0][0]["seat"]["agent"], "pi");
        assert_eq!(value["stages"][0][0]["seat"]["origin"], "manual");
        assert_eq!(value["stages"][0][0]["effective"]["model"], "glm-4.7");
        // 错误返回 (退出码, stderr 说明)。
        let seat = SeatChoice { agent: "ghost".into(), model: "x".into(), ..SeatChoice::default() };
        let (code, message) = config_set_seat_at(&root, "claude", "blind", 1, 1, seat).unwrap_err();
        assert_eq!(code, 2);
        assert!(message.contains("未知 agent：ghost"), "{message}");
        // config_clear_seat 同 clear-seat。
        let value = config_clear_seat_at(&root, "claude", "blind", Some(1), Some(1)).unwrap();
        assert_eq!(value["stages"][0][0]["seat"], serde_json::Value::Null);
        // resources_detect_at / resources_smoke_command_at 的参数错误同样回消息。
        let error = resources_smoke_command_at(&root, "ghost", None, None).unwrap_err();
        assert!(error.contains("未知 agent：ghost"), "{error}");
        let _ = fs::remove_dir_all(root);
    }

    /// fix-46-r1 R9：窗口命令与命令行对同一临时目录、同一参数的输出做完整
    /// JSON 比对（不只抽查几个字段）。命令行写在后，内容相同，落盘一致。
    #[test]
    fn task46_r1_window_outputs_match_cli_json_exactly() {
        let root = fixture_root("t46-r1-window-cli");
        write_fixture_store(&root);
        // config_snapshot 的 config 段对 config show 整个 JSON。
        let reports = vec![
            report("claude", acp_setup::AdapterState::Ready, None),
            report("codex", acp_setup::AdapterState::NoCli, None),
        ];
        let (acp, _) = fake_acp(reports.clone());
        let snapshot = config_snapshot_at(&root, Some("claude"), &reports).unwrap();
        let (code, cli_stdout, stderr) = run_config_acp(&root, &["aicross", "config", "show"], &acp);
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(snapshot["config"], parse_stdout(&cli_stdout));
        // config_set_seat 对 set-seat。
        let seat = SeatChoice {
            agent: "PI".into(),
            provider: Some("zai".into()),
            model: "glm-4.7".into(),
            tier: "高".into(),
            thinking: "低".into(),
            fallback: Some(FallbackTarget {
                agent: "claude".into(),
                provider: None,
                model: "sonnet".into(),
            }),
            ..SeatChoice::default()
        };
        let window_value = config_set_seat_at(&root, "claude", "blind", 1, 2, seat).unwrap();
        let (code, cli_stdout, stderr) = run_config_acp(
            &root,
            &[
                "aicross", "config", "set-seat", "blind", "1", "2",
                "--host", "claude",
                "--agent", "PI", "--model", "glm-4.7", "--provider", "zai",
                "--tier", "高", "--thinking", "低",
                "--fallback-agent", "claude", "--fallback-model", "sonnet",
            ],
            &AcpOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(window_value, parse_stdout(&cli_stdout));
        // config_clear_seat 对 clear-seat（单个位置）。
        let window_value = config_clear_seat_at(&root, "claude", "blind", Some(1), Some(2)).unwrap();
        let (code, cli_stdout, stderr) =
            run_config_acp(&root, &["aicross", "config", "clear-seat", "blind", "1", "2", "--host", "claude"], &AcpOps::production());
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(window_value, parse_stdout(&cli_stdout));
        // config_set_default_mode 对 set-default-mode。
        let window_value = config_set_default_mode_at(&root, "claude", "blind").unwrap();
        let (code, cli_stdout, stderr) =
            run_config_acp(&root, &["aicross", "config", "set-default-mode", "blind", "--host", "claude"], &AcpOps::production());
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(window_value, parse_stdout(&cli_stdout));
        let _ = fs::remove_dir_all(root);
    }

    /// fix-46-r1 R3：窗口探测与命令行 detect 同一实现体——窗口值比掉
    /// messages 字段后等于命令行 stdout，messages 等于命令行 stderr 逐行。
    #[test]
    fn task46_r1_detect_window_value_and_messages_match_cli() {
        let root = fixture_root("t46-r1-detect");
        write_fixture_store(&root);
        let ops = ResourcesOps {
            detect_all: Box::new(|path, _| {
                let mut store = resources::get_resources_at(path)?;
                if let Some(agent) = store.agents.iter_mut().find(|agent| agent.id == "kimi") {
                    agent.detected = true;
                    agent.version = "1.2.3".to_string();
                }
                Ok(ResourceOperation {
                    errors: vec!["探测 codex 失败：读取 ~/.codex/models_cache.json 失败".to_string()],
                    cc_switch_hints: Vec::new(),
                    skipped: Vec::new(),
                    store,
                })
            }),
            ..ResourcesOps::production()
        };
        let window_value = outcome_value_with_messages(resources_detect_at(&root, &ops).unwrap());
        let (code, stdout, stderr) = run_with(&root, &["aicross", "resources", "detect"], &ops);
        assert_eq!(code, 0, "{stderr}");
        let cli_value = parse_stdout(&stdout);
        let mut window_only = window_value.clone();
        let messages = window_only.as_object_mut().unwrap().remove("messages").unwrap();
        assert_eq!(window_only, cli_value, "去掉 messages 后应与命令行 stdout 一致");
        let lines: Vec<String> = stderr.lines().map(str::to_string).collect();
        assert_eq!(messages, serde_json::to_value(lines).unwrap());
        // 空时也给 []。
        let empty = DetectOutcome { value: json!({}), messages: Vec::new() };
        assert_eq!(
            outcome_value_with_messages(empty)["messages"],
            serde_json::Value::Array(Vec::new())
        );
        let _ = fs::remove_dir_all(root);
    }

    /// fix-46-r1 R4：窗口冒烟失败回 Err(stderr 那几行说明)；参数错误同样
    /// 回 Err（校验在共用实现体里，窗口不再绕过）。
    #[test]
    fn task46_r1_window_smoke_failure_returns_stderr_lines() {
        let root = fixture_root("t46-r1-smoke-window");
        write_fixture_store(&root);
        let fail_ops = ResourcesOps {
            smoke_one: Box::new(|path, key| fake_smoke_outcome(path, &key, false)),
            ..ResourcesOps::production()
        };
        let error = smoke_command_value(resources_smoke_at(
            &root,
            "pi",
            Some("glm-5.3"),
            None,
            &fail_ops,
        ))
        .unwrap_err();
        assert_eq!(error, "冒烟失败：HTTP 429: too many requests");
        // 批量冒烟部分失败：errors 加上末行说明，一起进 Err。
        let root = fixture_root("t46-r1-smoke-window-batch");
        write_fixture_store(&root);
        let batch_ops = ResourcesOps {
            smoke_agent_third_party: Box::new(|path, _| {
                let store = resources::get_resources_at(path)?;
                Ok(ResourceOperation {
                    errors: vec!["验证 pi/bad 失败：403: denied".to_string()],
                    cc_switch_hints: Vec::new(),
                    skipped: Vec::new(),
                    store,
                })
            }),
            ..ResourcesOps::production()
        };
        let error = smoke_command_value(resources_smoke_at(&root, "pi", None, None, &batch_ops))
            .unwrap_err();
        assert!(error.contains("验证 pi/bad 失败：403: denied"), "{error}");
        assert!(error.contains("冒烟完成；1 条失败"), "{error}");
        // --provider 不带 --model：参数错，不再当成整 agent 冒烟。
        let error = smoke_command_value(resources_smoke_at(
            &root,
            "pi",
            None,
            Some("zai"),
            &ResourcesOps::production(),
        ))
        .unwrap_err();
        assert_eq!(error, "--provider 需要 --model");
        let _ = fs::remove_dir_all(root);
    }

    /// fix-46-r1 R2：非 UTF-8 的 modes.json / roles.json 按读不了拒写，
    /// 退出 1、文件内容不变。
    #[test]
    fn task46_r1_non_utf8_config_files_rejected_without_overwrite() {
        let root = fixture_root("t46-r1-non-utf8");
        write_fixture_store(&root);
        fs::write(root.join("modes.json"), [0xff, 0xfe, b'{']).unwrap();
        let before = fs::read(root.join("modes.json")).unwrap();
        let (code, stdout, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-seat", "blind", "1", "1", "--host", "claude", "--agent", "pi", "--model", "glm-4.7", "--provider", "zai"],
            &AcpOps::production(),
        );
        assert_eq!(code, 1, "{stderr}");
        assert!(stdout.is_empty());
        assert!(stderr.contains("modes.json 读不了"), "{stderr}");
        assert_eq!(fs::read(root.join("modes.json")).unwrap(), before);

        fs::write(root.join("roles.json"), [0xff, 0xfe, b'{']).unwrap();
        let before = fs::read(root.join("roles.json")).unwrap();
        let (code, stdout, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-role", "execution", "--agent", "pi", "--model", "glm-4.7", "--provider", "zai"],
            &AcpOps::production(),
        );
        assert_eq!(code, 1, "{stderr}");
        assert!(stdout.is_empty());
        assert!(stderr.contains("roles.json 读不了"), "{stderr}");
        assert_eq!(fs::read(root.join("roles.json")).unwrap(), before);
        let _ = fs::remove_dir_all(root);
    }

    /// fix-46-r1 R7：写入后 seats 按预设形状补齐 null（前面的阶段与位置
    /// 不再落盘为空数组或缺位）。
    #[test]
    fn task46_r1_seat_write_pads_seats_to_preset_shape() {
        let root = fixture_root("t46-r1-pad");
        write_fixture_store(&root);
        // 经济模式三个阶段（1 / 1 / 2 个位置），写第 2 阶段第 1 位置。
        let (code, _, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-seat", "economy", "2", "1", "--host", "claude", "--agent", "pi", "--model", "glm-4.7", "--provider", "zai"],
            &AcpOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("modes.json")).unwrap()).unwrap();
        let stages = &saved["hosts"]["claude"]["seats"]["builtin-economy"];
        assert_eq!(stages.as_array().unwrap().len(), 3);
        assert_eq!(stages[0], serde_json::Value::Array(vec![serde_json::Value::Null]));
        assert_eq!(stages[1][0]["agent"], "pi");
        assert_eq!(
            stages[2],
            serde_json::Value::Array(vec![serde_json::Value::Null, serde_json::Value::Null])
        );
        let _ = fs::remove_dir_all(root);
    }

    /// 复审追加：预设改短后留下的旧人选在下一次写入时清掉。
    #[test]
    fn task46_seat_write_trims_seats_beyond_preset_shape() {
        let root = fixture_root("t46-trim");
        write_fixture_store(&root);
        let seat = r#"{"agent":"pi","provider":"zai","model":"glm-4.7","tier":"","thinking":"","fallback":null}"#;
        fs::write(
            root.join("modes.json"),
            format!(
                r#"{{"version":2,"hosts":{{"claude":{{"vendorOverride":"","defaultMode":"","seats":{{"builtin-economy":[[null,{seat}],[null],[null,null,{seat}],[{seat}]]}}}}}}}}"#
            ),
        )
        .unwrap();
        let (code, _, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-seat", "economy", "1", "1", "--host", "claude", "--agent", "pi", "--model", "glm-4.7", "--provider", "zai"],
            &AcpOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("modes.json")).unwrap()).unwrap();
        let lens: Vec<usize> = saved["hosts"]["claude"]["seats"]["builtin-economy"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row.as_array().unwrap().len())
            .collect();
        assert_eq!(lens, vec![1, 1, 2], "经济模式的形状是 1 / 1 / 2 个位置");
        let _ = fs::remove_dir_all(root);
    }

    // ── TASK-56：位置数量可调 ─────────────────────────────────────────

    /// `config show` 报各模式的 count 字段与调整后的阶段形状：可调模式给
    /// role/value/min/max/default，不可调为 null；超出的旧人选不显示。
    #[test]
    fn task56_config_show_reports_count_and_shaped_stages() {
        let root = fixture_root("t56-show");
        write_fixture_store(&root);
        fs::write(
            root.join("modes.json"),
            r#"{"version":2,"hosts":{"claude":{"vendorOverride":"","defaultMode":"","counts":{"builtin-arena":2},"seats":{"builtin-arena":[[null,null,null]]}}}}"#,
        )
        .unwrap();
        let (code, stdout, stderr) = run_config_acp(&root, &["aicross", "config", "show", "--host", "claude"], &AcpOps::production());
        assert_eq!(code, 0, "{stderr}");
        let parsed = parse_stdout(&stdout);
        let find = |id: &str| {
            parsed["modes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|mode| mode["id"] == id)
                .unwrap()
                .clone()
        };
        // 擂台：count 设为 2，阶段只剩 2 个执行位置。
        let arena = find("builtin-arena");
        assert_eq!(
            arena["count"],
            serde_json::json!({ "role": "执行", "value": 2, "min": 2, "max": 4, "default": 3 })
        );
        assert_eq!(arena["stages"][0].as_array().unwrap().len(), 2);
        // 盲审：没设 counts，value 回默认；验收固定、两路验证。
        let blind = find("builtin-blind");
        assert_eq!(
            blind["count"],
            serde_json::json!({ "role": "验证", "value": 2, "min": 1, "max": 4, "default": 2 })
        );
        let roles: Vec<String> = blind["stages"][0]
            .as_array()
            .unwrap()
            .iter()
            .map(|node| node["role"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(roles, ["验收", "验证", "验证"]);
        // 经济：可调到 0–3，默认 1；共享不可调。
        assert_eq!(find("builtin-economy")["count"],
            serde_json::json!({ "role": "验证", "value": 1, "min": 0, "max": 3, "default": 1 }));
        assert!(find("builtin-shared")["count"].is_null());
        let _ = fs::remove_dir_all(root);
    }

    /// `config set-count`：写入 counts，减少时裁掉超出的人选，增加时补
    /// 空位；输出该模式那一项（同 config show 的 modes[i]）。
    #[test]
    fn task56_config_set_count_trims_and_pads_seats() {
        let root = fixture_root("t56-set-count");
        write_fixture_store(&root);
        let set_seat = |index: &str| {
            run_config_acp(
                &root,
                &[
                    "aicross", "config", "set-seat", "builtin-arena", "1", index,
                    "--host", "claude", "--agent", "pi", "--model", "glm-4.7", "--provider", "zai",
                ],
                &AcpOps::production(),
            )
        };
        for index in ["1", "2", "3"] {
            let (code, _, stderr) = set_seat(index);
            assert_eq!(code, 0, "{index} → {stderr}");
        }
        // 减到 2：第三个位置的人选一并去掉。
        let (code, stdout, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-count", "builtin-arena", "2", "--host", "claude"],
            &AcpOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.contains("已写入位置数量"), "{stderr}");
        let entry = parse_stdout(&stdout);
        assert_eq!(entry["count"]["value"], 2);
        assert_eq!(entry["stages"][0].as_array().unwrap().len(), 2);
        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("modes.json")).unwrap()).unwrap();
        assert_eq!(saved["hosts"]["claude"]["counts"]["builtin-arena"], 2);
        let row = &saved["hosts"]["claude"]["seats"]["builtin-arena"][0];
        assert_eq!(row.as_array().unwrap().len(), 2, "超出的人选已裁剪");
        assert_eq!(row[0]["model"], "glm-4.7");
        assert_eq!(row[1]["model"], "glm-4.7");
        // 加到 4：新位置为空位。
        let (code, stdout, _) = run_config_acp(
            &root,
            &["aicross", "config", "set-count", "builtin-arena", "4", "--host", "claude"],
            &AcpOps::production(),
        );
        assert_eq!(code, 0);
        let entry = parse_stdout(&stdout);
        assert_eq!(entry["count"]["value"], 4);
        let stages = entry["stages"][0].as_array().unwrap();
        assert_eq!(stages.len(), 4);
        assert!(stages[2]["seat"].is_null());
        assert!(stages[3]["seat"].is_null());
        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("modes.json")).unwrap()).unwrap();
        let row = &saved["hosts"]["claude"]["seats"]["builtin-arena"][0];
        assert_eq!(row.as_array().unwrap().len(), 4);
        assert!(row[2].is_null());
        assert!(row[3].is_null());
        let _ = fs::remove_dir_all(root);
    }

    /// `config set-count`：不可调的模式与越界都退出 2，信息给出范围；
    /// 失败不写盘。
    #[test]
    fn task56_config_set_count_rejects_non_adjustable_and_out_of_range() {
        let root = fixture_root("t56-set-count-args");
        write_fixture_store(&root);
        fs::write(
            root.join("modes.json"),
            r#"{"version":2,"hosts":{"claude":{"vendorOverride":"","defaultMode":""}}}"#,
        )
        .unwrap();
        for (args, expect) in [
            (vec!["aicross", "config", "set-count"], "缺少模式"),
            (vec!["aicross", "config", "set-count", "blind"], "缺少数量"),
            (vec!["aicross", "config", "set-count", "blind", "2", "extra", "--host", "claude"], "不认识的参数"),
            (vec!["aicross", "config", "set-count", "shared", "2", "--host", "claude"], "位置数量不可调"),
            (vec!["aicross", "config", "set-count", "builtin-swarm", "2", "--host", "claude"], "位置数量不可调"),
            (vec!["aicross", "config", "set-count", "blind", "5", "--host", "claude"], "只认 1–4"),
            (vec!["aicross", "config", "set-count", "blind", "0", "--host", "claude"], "只认 1–4"),
            (vec!["aicross", "config", "set-count", "economy", "4", "--host", "claude"], "只认 0–3"),
            (vec!["aicross", "config", "set-count", "blind", "x", "--host", "claude"], "数量必须"),
            (vec!["aicross", "config", "set-count", "ghost", "2", "--host", "claude"], "模式不存在：ghost"),
        ] {
            let (code, stdout, stderr) = run_config_acp(&root, &args, &AcpOps::production());
            assert_eq!(code, 2, "{args:?} → {stderr}");
            assert!(stderr.contains(expect), "{args:?} → {stderr}");
            assert!(stdout.is_empty(), "{args:?} → {stdout}");
        }
        // 失败不写盘：counts 键不存在。
        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("modes.json")).unwrap()).unwrap();
        assert!(saved["hosts"]["claude"].get("counts").is_none());
        let _ = fs::remove_dir_all(root);
    }

    /// set-seat 与 clear-seat 的阶段、位置范围按调整后的形状算：减到 1
    /// 后原第 3 位置退出范围，加到 4 后新位置可设人选。
    #[test]
    fn task56_seat_range_follows_adjusted_shape() {
        let root = fixture_root("t56-seat-range");
        write_fixture_store(&root);
        fs::write(
            root.join("modes.json"),
            r#"{"version":2,"hosts":{"claude":{"vendorOverride":"","defaultMode":"","counts":{"builtin-arena":2}}}}"#,
        )
        .unwrap();
        let (code, _, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-seat", "builtin-arena", "1", "3", "--host", "claude", "--agent", "pi", "--model", "glm-4.7", "--provider", "zai"],
            &AcpOps::production(),
        );
        assert_eq!(code, 2, "{stderr}");
        assert!(stderr.contains("位置超出范围"), "{stderr}");
        let (code, _, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-count", "builtin-arena", "4", "--host", "claude"],
            &AcpOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        let (code, stdout, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-seat", "builtin-arena", "1", "4", "--host", "claude", "--agent", "pi", "--model", "glm-4.7", "--provider", "zai"],
            &AcpOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        let entry = parse_stdout(&stdout);
        assert_eq!(entry["stages"][0][3]["seat"]["model"], "glm-4.7");
        let _ = fs::remove_dir_all(root);
    }

    /// 裁剪语义：各位置填不同模型；盲审 4 → 1 时「验收」的人选保留、
    /// 三路验证的裁掉；经济 1 → 0 → 2 时验证位置恢复、恢复的位置人选
    /// 为空，验收位置不变。
    #[test]
    fn task56_set_count_trim_keeps_fixed_seats_and_restores_shape() {
        let root = fixture_root("t56-trim");
        write_fixture_store(&root);
        // 先把盲审加到 4 路，再给 4 个位置各填不同模型（验收 + 三路验证）。
        let (code, _, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-count", "builtin-blind", "4", "--host", "claude"],
            &AcpOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        let seats = [
            ("pi", "zai", "glm-4.7"),
            ("pi", "zai", "glm-5.3"),
            ("pi", "bigmodel", "glm-4.7"),
            ("claude", "", "sonnet"),
        ];
        for (position, (agent, provider, model)) in seats.iter().enumerate() {
            let mut args = vec![
                "aicross".to_string(),
                "config".to_string(),
                "set-seat".to_string(),
                "builtin-blind".to_string(),
                "1".to_string(),
                (position + 1).to_string(),
                "--host".to_string(),
                "claude".to_string(),
                "--agent".to_string(),
                agent.to_string(),
                "--model".to_string(),
                model.to_string(),
            ];
            if !provider.is_empty() {
                args.push("--provider".into());
                args.push(provider.to_string());
            }
            let (code, _, stderr) = run_config_acp(
                &root,
                &args.iter().map(String::as_str).collect::<Vec<_>>(),
                &AcpOps::production(),
            );
            assert_eq!(code, 0, "{agent}/{model} → {stderr}");
        }
        // 盲审减到 1：验收（第 1 位）人选保留，验证位只剩一路，带的是
        // 第 2 位的人选（从末尾裁，前面的人选留在原位），第 3、4 位裁掉。
        let (code, stdout, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-count", "builtin-blind", "1", "--host", "claude"],
            &AcpOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        let entry = parse_stdout(&stdout);
        let stage = entry["stages"][0].as_array().unwrap();
        assert_eq!(stage.len(), 2, "验收 + 一路验证");
        assert_eq!(stage[0]["seat"]["model"], "glm-4.7");
        assert_eq!(stage[1]["seat"]["model"], "glm-5.3");
        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("modes.json")).unwrap()).unwrap();
        let row = saved["hosts"]["claude"]["seats"]["builtin-blind"][0]
            .as_array()
            .unwrap();
        assert_eq!(row.len(), 2);
        assert_eq!(row[0]["model"], "glm-4.7");
        assert_eq!(row[1]["model"], "glm-5.3");
        // 经济：第 3 阶段验证位填人（阶段 3 位置 2），减到 0 只剩验收，
        // 再加回 2：位置恢复、人选为空，验收不变。
        let (code, _, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-seat", "builtin-economy", "3", "2", "--host", "claude", "--agent", "kimi", "--model", "k2"],
            &AcpOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        let (code, _, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-count", "builtin-economy", "0", "--host", "claude"],
            &AcpOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        let (code, stdout, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "show", "--host", "claude"],
            &AcpOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        let economy = parse_stdout(&stdout)["modes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|mode| mode["id"] == "builtin-economy")
            .unwrap()
            .clone();
        let stage = economy["stages"][2].as_array().unwrap();
        assert_eq!(stage.len(), 1, "验证减到 0 只剩验收");
        assert_eq!(stage[0]["role"], "验收");
        let (code, stdout, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-count", "builtin-economy", "2", "--host", "claude"],
            &AcpOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        let stage = parse_stdout(&stdout)["stages"][2].as_array().unwrap().to_vec();
        assert_eq!(stage.len(), 3, "验收 + 两路验证");
        assert_eq!(stage[0]["role"], "验收");
        assert_eq!(stage[1]["role"], "验证");
        assert_eq!(stage[2]["role"], "验证");
        assert!(stage[1]["seat"].is_null() && stage[2]["seat"].is_null(), "恢复的位置人选为空");
        let _ = fs::remove_dir_all(root);
    }

    /// set-seat / clear-seat 在写入锁内按严格读到的 modes 取形状与范围：
    /// modes.json 损坏时退出 1，不落到宽松读取、不写盘。
    #[test]
    fn task56_set_seat_reads_modes_strictly_under_lock() {
        let root = fixture_root("t56-seat-strict");
        write_fixture_store(&root);
        fs::write(root.join("modes.json"), "{ not json").unwrap();
        for args in [
            vec![
                "aicross", "config", "set-seat", "builtin-arena", "1", "1",
                "--host", "claude", "--agent", "pi", "--model", "glm-4.7", "--provider", "zai",
            ],
            vec![
                "aicross", "config", "clear-seat", "builtin-arena", "1", "1", "--host", "claude",
            ],
        ] {
            let (code, stdout, stderr) = run_config_acp(&root, &args, &AcpOps::production());
            assert_eq!(code, 1, "{args:?} → {stderr}");
            assert!(stderr.contains("modes.json"), "{args:?} → {stderr}");
            assert!(stdout.is_empty(), "{args:?} → {stdout}");
        }
        let _ = fs::remove_dir_all(root);
    }

    /// fix-fullcheck A4：set-seat 在锁外先读到的预设里该位置是盲位，等锁
    /// 期间 preset save 把它改成共享位；拿到锁后按锁内重读的预设校验，
    /// agy（只接盲位）被拒，modes.json 不写。测试持有 modes 写锁、在锁内
    /// 改 presets.json，等同并发的 preset save。
    #[test]
    fn a4_set_seat_rechecks_preset_inside_lock() {
        let root = fixture_root("a4-preset-race");
        fs::write(
            root.join("resources.json"),
            r#"{"version":1,"agents":[
                {"id":"agy","name":"Antigravity","kind":"内置","detected":true,"version":"1.3.1","models":[
                    {"id":"gemini-3.8-flash-low","agent":"agy","provider":"","vendor":"Google","verification":"native"}
                ]}
            ]}"#,
        )
        .unwrap();
        let preset_with = |visibility: &str| crate::orchestration::PresetFile {
            version: 1,
            presets: vec![Preset {
                id: "u-a4".into(),
                name: "A4".into(),
                description: String::new(),
                builtin: false,
                stages: vec![crate::orchestration::Stage {
                    nodes: vec![Target {
                        role: "验证".into(),
                        thinking: "中".into(),
                        visibility: visibility.into(),
                        write: "只读".into(),
                        ..Target::default()
                    }],
                }],
            }],
        };
        crate::orchestration::save_presets_in(&root, &preset_with("盲")).unwrap();
        let guard = modes_write_lock(&root).unwrap();
        let worker_root = root.clone();
        let worker = std::thread::spawn(move || {
            config_set_seat_at(
                &worker_root,
                "claude",
                "u-a4",
                1,
                1,
                SeatChoice {
                    agent: "agy".into(),
                    model: "gemini-3.8-flash-low".into(),
                    ..SeatChoice::default()
                },
            )
        });
        std::thread::sleep(std::time::Duration::from_millis(300));
        crate::orchestration::save_presets_in(&root, &preset_with("共享")).unwrap();
        drop(guard);
        let (code, message) = worker.join().unwrap().unwrap_err();
        assert_eq!(code, 2, "{message}");
        assert!(message.contains("共享"), "{message}");
        let modes = crate::orchestration::modes_from(&root);
        assert!(
            modes.hosts.get("claude").is_none_or(|section| section.seats.get("u-a4").is_none()),
            "被拒时不写人选"
        );
        let _ = fs::remove_dir_all(root);
    }

    /// modes.json 写入跨进程互斥：测试直接持有锁文件（等同另一个主 agent
    /// 里的 aicross 进程正在写），写入要等它释放才落盘。
    #[test]
    fn modes_writes_wait_for_cross_process_lock() {
        let root = fixture_root("modes-xlock");
        let held = crate::file_lock::FileLock::acquire(
            &root.join("modes.json.lock"),
            std::time::Duration::from_secs(1),
        )
        .unwrap();
        let worker_root = root.clone();
        let worker =
            std::thread::spawn(move || config_set_default_mode_at(&worker_root, "codex", "blind"));
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(
            crate::orchestration::modes_from(&root).hosts.get("codex").is_none(),
            "别的进程持锁时不写"
        );
        drop(held);
        worker.join().unwrap().unwrap();
        let modes = crate::orchestration::modes_from(&root);
        assert!(modes.hosts.get("codex").is_some(), "放锁后写入");
        let _ = fs::remove_dir_all(root);
    }

    /// --preset 缺省时不再写死经济模式，交给引擎按默认模式解析。
    #[test]
    fn task46_dispatch_without_preset_flag_omits_preset() {
        let root = fixture_root("t46-no-preset-flag");
        let text_path = root.join("prompt.txt");
        fs::write(&text_path, "正文").unwrap();
        let args = vec![
            "aicross".into(), "dispatch".into(), "--project".into(), root.to_string_lossy().into(),
            "--text-file".into(), text_path.to_string_lossy().into(),
        ];
        let request = build_request(&args).unwrap();
        assert!(request.body.unwrap().get("preset").is_none());
        // 显式 --preset 照旧解析别名。
        let args = vec![
            "aicross".into(), "dispatch".into(), "--project".into(), root.to_string_lossy().into(),
            "--preset".into(), "blind".into(), "--text-file".into(), text_path.to_string_lossy().into(),
        ];
        let request = build_request(&args).unwrap();
        assert_eq!(request.body.unwrap()["preset"], "builtin-blind");
        let _ = fs::remove_dir_all(root);
    }

    // ── TASK-48：资源库里「未发现」的模型 ────────────────────────

    /// 给资源库里的某条模型追加「未在事实源中」标记（模拟最近一次探测
    /// 没发现它）。
    fn mark_missing(root: &Path, agent_id: &str, provider: &str, model_id: &str) {
        let path = store_file(root);
        let mut store = resources::get_resources_at(&path).unwrap();
        let model = store
            .agents
            .iter_mut()
            .find(|agent| agent.id == agent_id)
            .unwrap()
            .models
            .iter_mut()
            .find(|model| model.id == model_id && model.provider == provider)
            .unwrap();
        model.meta = if model.meta.trim().is_empty() {
            resources::MISSING_MARKER.to_string()
        } else {
            format!("{}；{}", model.meta, resources::MISSING_MARKER)
        };
        resources::save_store(&path, &store).unwrap();
    }

    #[test]
    fn task48_resources_lists_missing_state_and_detected_at() {
        let root = fixture_root("t48-list");
        write_fixture_store(&root);
        mark_missing(&root, "pi", "zai", "glm-5.3");
        // 有 detectedAt 时原样输出；写入的键名是 detectedAt（驼峰）。
        let mut store = resources::get_resources_at(&store_file(&root)).unwrap();
        store.detected_at = Some("2026-10-06T10:00:00+08:00".to_string());
        resources::save_store(&store_file(&root), &store).unwrap();
        let (code, stdout, stderr) = run_with(&root, &["aicross", "resources"], &ResourcesOps::production());
        assert_eq!(code, 0, "{stderr}");
        let parsed = parse_stdout(&stdout);
        assert_eq!(parsed["detected_at"], "2026-10-06T10:00:00+08:00");
        let missing = model_state_in(&stdout, "pi", "zai", "glm-5.3");
        assert_eq!(missing["state"], "未发现");
        assert_eq!(missing["note"], resources::NOT_DETECTED_NOTE);
        assert_eq!(missing["missing"], true);
        // 未验证 / 已验证的其它模型不受影响。
        assert_eq!(model_state_in(&stdout, "pi", "zai", "glm-4.7")["state"], "已验证");
        assert_eq!(model_state_in(&stdout, "pi", "zai", "glm-4.7")["missing"], false);
        let _ = fs::remove_dir_all(root);

        // 旧文件没有 detectedAt 时输出 null（走查 #1：此时全部显示「未探测」）。
        let root = fixture_root("t48-list-no-stamp");
        write_fixture_store(&root);
        let mut store = resources::get_resources_at(&store_file(&root)).unwrap();
        store.detected_at = None;
        resources::save_store(&store_file(&root), &store).unwrap();
        let (code, stdout, _) = run_with(&root, &["aicross", "resources"], &ResourcesOps::production());
        assert_eq!(code, 0);
        assert_eq!(parse_stdout(&stdout)["detected_at"], serde_json::Value::Null);
        let parsed = parse_stdout(&stdout);
        let kimi = parsed["agents"].as_array().unwrap().iter()
            .find(|agent| agent["id"] == "kimi").unwrap();
        assert_eq!(kimi["state"], resources::RESOURCE_STATE_NEVER_DETECTED);
        assert_eq!(kimi["note"], resources::NEVER_DETECTED_NOTE);
        let _ = fs::remove_dir_all(root);
    }

    /// 单个「未发现」模型不冒烟：不发请求、退出 1，stderr 写统一说明；
    /// 原生模型带标记同样适用。
    #[test]
    fn task48_smoke_single_missing_exits_one_without_request() {
        let root = fixture_root("t48-smoke-one");
        write_fixture_store(&root);
        mark_missing(&root, "pi", "zai", "glm-5.3");
        mark_missing(&root, "claude", "", "sonnet");
        let ops = ResourcesOps {
            smoke_one: Box::new(|_, _| panic!("未发现的模型不该发请求")),
            ..ResourcesOps::production()
        };
        let (code, stdout, stderr) = run_with(
            &root,
            &["aicross", "resources", "smoke", "pi", "--model", "glm-5.3", "--provider", "zai"],
            &ops,
        );
        assert_eq!(code, 1, "{stderr}");
        assert!(stderr.contains(resources::NOT_DETECTED_NOTE), "{stderr}");
        // 单模型输出的对象本身就是该 agent，state 应为未发现。
        let parsed = parse_stdout(&stdout);
        let model = parsed["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|model| model["id"] == "glm-5.3")
            .unwrap();
        assert_eq!(model["state"], "未发现");
        // 原生模型带标记：同样退出 1，而不是「原生模型无需冒烟」。
        let (code, _, stderr) = run_with(
            &root,
            &["aicross", "resources", "smoke", "claude", "--model", "sonnet"],
            &ops,
        );
        assert_eq!(code, 1, "{stderr}");
        assert!(stderr.contains(resources::NOT_DETECTED_NOTE), "{stderr}");
        let _ = fs::remove_dir_all(root);
    }

    /// 按 agent 冒烟：跳过的未发现模型逐行写 stderr，不算失败、退出码 0。
    #[test]
    fn task48_smoke_agent_reports_skipped_missing() {
        let root = fixture_root("t48-smoke-batch");
        write_fixture_store(&root);
        mark_missing(&root, "pi", "zai", "glm-5.3");
        let ops = ResourcesOps {
            smoke_agent_third_party: Box::new(|_, _| {
                Ok(ResourceOperation {
                    skipped: vec![format!("跳过 pi/glm-5.3：{}", resources::NOT_DETECTED_NOTE)],
                    ..ResourceOperation::default()
                })
            }),
            ..ResourcesOps::production()
        };
        let (code, _, stderr) = run_with(&root, &["aicross", "resources", "smoke", "pi"], &ops);
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.contains("跳过 pi/glm-5.3："), "{stderr}");
        assert!(stderr.contains(resources::NOT_DETECTED_NOTE), "{stderr}");
        assert!(stderr.contains("冒烟完成"), "{stderr}");
        assert!(!stderr.contains("条失败"), "{stderr}");
        let _ = fs::remove_dir_all(root);
    }

    /// set-role 指向「未发现」的模型退出 2，stderr 给原因与该 agent 下
    /// 可选模型（列表里去掉未发现的），不写 roles.json。
    #[test]
    fn task48_set_role_rejects_missing_model_and_lists_candidates() {
        let root = fixture_root("t48-set-role");
        write_fixture_store(&root);
        mark_missing(&root, "pi", "zai", "glm-5.3");
        let (code, stdout, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-role", "execution", "--agent", "pi", "--model", "glm-5.3", "--provider", "zai"],
            &AcpOps::production(),
        );
        assert_eq!(code, 2, "{stderr}");
        assert!(stdout.is_empty());
        assert!(stderr.contains("pi/glm-5.3："), "{stderr}");
        assert!(stderr.contains(resources::NOT_DETECTED_NOTE), "{stderr}");
        assert!(stderr.contains("该 agent 下可选"), "{stderr}");
        assert!(stderr.contains("zai/glm-4.7"), "{stderr}");
        assert!(!stderr.contains("zai/glm-5.3"), "候选列表应去掉未发现的：{stderr}");
        assert!(!root.join("roles.json").exists(), "被拒时不应写盘");
        let _ = fs::remove_dir_all(root);
    }

    /// set-seat 的主目标或备选指向「未发现」都退出 2，不写 modes.json。
    #[test]
    fn task48_set_seat_rejects_missing_target_and_fallback() {
        let root = fixture_root("t48-set-seat");
        write_fixture_store(&root);
        mark_missing(&root, "pi", "zai", "glm-5.3");
        let (code, _, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-seat", "blind", "1", "1", "--host", "claude", "--agent", "pi", "--model", "glm-5.3", "--provider", "zai"],
            &AcpOps::production(),
        );
        assert_eq!(code, 2, "{stderr}");
        assert!(stderr.contains(resources::NOT_DETECTED_NOTE), "{stderr}");
        // 备选未发现：同样拒绝，原因指向备选。
        let (code, _, stderr) = run_config_acp(
            &root,
            &[
                "aicross", "config", "set-seat", "blind", "1", "1", "--host", "claude",
                "--agent", "pi", "--model", "glm-4.7", "--provider", "zai",
                "--fallback-agent", "pi", "--fallback-model", "glm-5.3", "--fallback-provider", "zai",
            ],
            &AcpOps::production(),
        );
        assert_eq!(code, 2, "{stderr}");
        assert!(stderr.contains(resources::NOT_DETECTED_NOTE), "{stderr}");
        assert!(!root.join("modes.json").exists(), "被拒时不应写盘");
        let _ = fs::remove_dir_all(root);
    }

    /// `config show` 的 effective.state：人选在资源库里的状态；同名模型
    /// 挂两个 provider 时按人选的 provider 认那一条（fix-48-r1 R1），给了
    /// provider 却没精确匹配时为「不在资源库」；资源库里没有这条模型时同。
    #[test]
    fn task48_config_show_effective_state() {
        let root = fixture_root("t48-show-state");
        write_fixture_store(&root);
        mark_missing(&root, "pi", "zai", "glm-5.3");
        mark_missing(&root, "pi", "bigmodel", "glm-4.7");
        fs::write(
            root.join("presets.json"),
            r#"{"version":1,"presets":[{"id":"u48","name":"任务书预设","builtin":false,"stages":[{"nodes":[
                {"role":"执行","agent":"pi","provider":"zai","model":"glm-5.3","thinking":"中","visibility":"共享","write":"只读"},
                {"role":"执行","agent":"codex","model":"ghost","thinking":"中","visibility":"共享","write":"只读"},
                {"role":"执行","agent":"pi","provider":"zai","model":"glm-4.7","thinking":"中","visibility":"共享","write":"只读"},
                {"role":"执行","agent":"pi","provider":"bigmodel","model":"glm-4.7","thinking":"中","visibility":"共享","write":"只读"},
                {"role":"执行","agent":"pi","provider":"nope","model":"glm-4.7","thinking":"中","visibility":"共享","write":"只读"}
            ]}]}]}"#,
        )
        .unwrap();
        let (code, stdout, stderr) = run_config_acp(&root, &["aicross", "config", "show"], &AcpOps::production());
        assert_eq!(code, 0, "{stderr}");
        let parsed = parse_stdout(&stdout);
        let preset = parsed["modes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|mode| mode["id"] == "u48")
            .unwrap();
        let nodes = &preset["stages"][0];
        assert_eq!(nodes[0]["effective"]["state"], "未发现");
        assert_eq!(nodes[1]["effective"]["state"], "不在资源库");
        assert_eq!(nodes[2]["effective"]["state"], "已验证");
        // 同名模型挂在两个 provider 下，各认各的；provider 没精确匹配不回退。
        assert_eq!(nodes[3]["effective"]["state"], "未发现");
        assert_eq!(nodes[4]["effective"]["state"], "不在资源库");
        let _ = fs::remove_dir_all(root);
    }

    /// fix-48-r1 R4：注入的 detect_all 走真实写回（加载 → 盖章 → 落盘，
    /// 与生产同一份 stamp_detected_at）时，`resources detect` 输出的
    /// detected_at 非 null，写回的文件里也有 detectedAt。
    #[test]
    fn task48_detect_output_reports_detected_at_after_real_write_back() {
        let root = fixture_root("t48-detect-stamp");
        write_fixture_store(&root);
        let ops = ResourcesOps {
            detect_all: Box::new(|path, _| {
                let mut store = resources::get_resources_at(path)?;
                resources::stamp_detected_at(&mut store);
                resources::save_store(path, &store)?;
                Ok(ResourceOperation {
                    store,
                    errors: Vec::new(),
                    cc_switch_hints: Vec::new(),
                    skipped: Vec::new(),
                })
            }),
            ..ResourcesOps::production()
        };
        let (code, stdout, stderr) = run_with(&root, &["aicross", "resources", "detect"], &ops);
        assert_eq!(code, 0, "{stderr}");
        let stamp = parse_stdout(&stdout)["detected_at"]
            .as_str()
            .expect("detected_at 应有值")
            .to_string();
        chrono::DateTime::parse_from_rfc3339(&stamp).expect("detected_at 应为 RFC3339");
        let raw: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(store_file(&root)).unwrap()).unwrap();
        assert_eq!(raw["detectedAt"].as_str(), Some(stamp.as_str()), "写回文件应带 detectedAt");
        let _ = fs::remove_dir_all(root);
    }

    // ── TASK-50：主 agent、一键推荐与 skill 安装 ───────────────────

    /// --host 三种缺省情形与非法 host（写命令）：零配置要 --host；单一
    /// 配置直接用；多于一个要 --host；不是检测到的 agent 退出 2。
    #[test]
    fn task50_config_commands_follow_host_rules() {
        let root = fixture_root("t50-host-rules");
        write_fixture_store(&root);
        // 零配置：写命令要 --host。
        let (code, _, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-default-mode", "blind"],
            &AcpOps::production(),
        );
        assert_eq!(code, 2, "{stderr}");
        assert!(stderr.contains("需要 --host，可选：claude、pi"), "{stderr}");
        // 零配置：给了非法 host 退出 2。
        let (code, _, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-default-mode", "blind", "--host", "kimi"],
            &AcpOps::production(),
        );
        assert_eq!(code, 2, "{stderr}");
        assert!(stderr.contains("未知 host：kimi"), "{stderr}");
        // 单一配置：不给 --host 就用它；写盘后 modes.json 只有一份配置。
        let (code, _, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-default-mode", "blind", "--host", "claude"],
            &AcpOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("modes.json")).unwrap()).unwrap();
        assert_eq!(saved["hosts"]["claude"]["defaultMode"], "builtin-blind");
        // 多于一个配置：不给 --host 要 --host。
        fs::write(
            root.join("modes.json"),
            r#"{"version":2,"hosts":{"claude":{"vendorOverride":"","defaultMode":"","seats":{}},"pi":{"vendorOverride":"","defaultMode":"","seats":{}}}}"#,
        )
        .unwrap();
        let (code, _, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-default-mode", "blind"],
            &AcpOps::production(),
        );
        assert_eq!(code, 2, "{stderr}");
        assert!(stderr.contains("需要 --host，可选：claude、pi"), "{stderr}");
        // 多于一个配置：给了 --host 照常。
        let (code, stdout, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-default-mode", "blind", "--host", "pi"],
            &AcpOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(parse_stdout(&stdout)["defaultMode"], "builtin-blind");
        // 读命令同理：show 不给 --host 且配置多于一个 → 退出 2。
        let (code, _, stderr) = run_config_acp(&root, &["aicross", "config", "show"], &AcpOps::production());
        assert_eq!(code, 2, "{stderr}");
        assert!(stderr.contains("需要 --host"), "{stderr}");
        let _ = fs::remove_dir_all(root);
    }

    /// fix-50-r1：`config show --host claude` 跑得通，返回该主 agent 的
    /// 配置（此前被当成多余参数拦截，退出 2）。
    #[test]
    fn task50_config_show_accepts_host_option() {
        let root = fixture_root("t50-show-host");
        write_fixture_store(&root);
        fs::write(
            root.join("modes.json"),
            r#"{"version":2,"hosts":{"claude":{"vendorOverride":"","defaultMode":"builtin-blind","seats":{}},"pi":{"vendorOverride":"","defaultMode":"","seats":{}}}}"#,
        )
        .unwrap();
        let (code, stdout, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "show", "--host", "claude"],
            &AcpOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        let parsed = parse_stdout(&stdout);
        assert_eq!(parsed["defaultMode"], "builtin-blind", "返回 claude 的配置");
        assert_eq!(parsed["modes"].as_array().map(Vec::len), Some(6));
        // modes 段与实现体同构（acp 段依本机适配器实况，不比）。
        assert_eq!(
            parsed["modes"],
            config_show_value(&root, Some("claude"), &[])["modes"],
            "show --host 与 config_show_value 的 modes 同构"
        );
        // 选项与未知参数混合仍报参数错。
        let (code, _, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "show", "--host", "claude", "extra"],
            &AcpOps::production(),
        );
        assert_eq!(code, 2, "{stderr}");
        assert!(stderr.contains("不认识的参数"), "{stderr}");
        let _ = fs::remove_dir_all(root);
    }

    /// fix-50-r1：`recommend --all` 的 --all 是不带值的开关，开头、中间、
    /// 末尾都能通过（此前被当成缺值的带值选项，退出 2）。
    #[test]
    fn task50_recommend_accepts_all_flag_anywhere() {
        let root = fixture_root("t50-recommend-all-flag");
        write_recommend_store(&root);
        let home = crate::paths::test_temp_dir().join(format!("t50-all-flag-home-{}", Uuid::new_v4()));
        fs::create_dir_all(&home).unwrap();
        let fresh_modes = |root: &Path| {
            fs::write(
                root.join("modes.json"),
                r#"{"version":2,"hosts":{"claude":{"vendorOverride":"","defaultMode":"","seats":{}}}}"#,
            )
            .unwrap();
        };
        for words in [
            vec!["aicross", "config", "recommend", "--all", "--host", "claude"],
            vec!["aicross", "config", "recommend", "--host", "claude", "--all"],
            vec!["aicross", "config", "recommend", "--all"],
        ] {
            fresh_modes(&root);
            let (code, stdout, stderr) = run_config_at_args(&root, &home, &words);
            assert_eq!(code, 0, "{words:?}: {stderr}");
            let parsed = parse_stdout(&stdout);
            assert!(parsed["modes"].as_array().is_some(), "{words:?}");
            // --all 全部重填：每个位置的 seat 都是 recommended。
            for mode in parsed["modes"].as_array().unwrap() {
                for stage in mode["stages"].as_array().unwrap() {
                    for node in stage.as_array().unwrap() {
                        if node["source"] != "未指定" {
                            assert_eq!(node["seat"]["origin"], "recommended", "{words:?}");
                        }
                    }
                }
            }
        }
        // 真正的选项缺值仍要报：--host 在末尾没值。
        fresh_modes(&root);
        let (code, _, stderr) = run_config_at_args(
            &root,
            &home,
            &["aicross", "config", "recommend", "--host"],
        );
        assert_eq!(code, 2, "{stderr}");
        assert!(stderr.contains("缺少 --host 的值"), "{stderr}");
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(home);
    }

    /// fix-50-r1：只有一份配置时不给 --host 自动推定——写命令与 show 都
    /// 直接用那份；推定出的主 agent 已不在检测列表时不再自动用（退出 2）。
    #[test]
    fn task50_single_config_infers_host_without_flag() {
        let root = fixture_root("t50-single-host");
        write_fixture_store(&root);
        fs::write(
            root.join("modes.json"),
            r#"{"version":2,"hosts":{"claude":{"vendorOverride":"","defaultMode":"","seats":{}}}}"#,
        )
        .unwrap();
        // 写命令：不给 --host，自动用 claude。
        let (code, _, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-default-mode", "blind"],
            &AcpOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("modes.json")).unwrap()).unwrap();
        assert_eq!(saved["hosts"]["claude"]["defaultMode"], "builtin-blind");
        // show：不给 --host，同样自动推定。
        let (code, stdout, stderr) = run_config_acp(&root, &["aicross", "config", "show"], &AcpOps::production());
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(parse_stdout(&stdout)["defaultMode"], "builtin-blind");
        // 推定出的主 agent 已不在检测列表：不再自动用，退出 2；文案点明
        // 这是 modes.json 里唯一的配置（fix-50-r2）。
        fs::write(
            root.join("modes.json"),
            r#"{"version":2,"hosts":{"kimi":{"vendorOverride":"","defaultMode":"","seats":{}}}}"#,
        )
        .unwrap();
        let (code, _, stderr) = run_config_acp(
            &root,
            &["aicross", "config", "set-default-mode", "blind"],
            &AcpOps::production(),
        );
        assert_eq!(code, 2, "{stderr}");
        assert!(
            stderr.contains("modes.json 里唯一的配置 kimi 未被检测到，请用 --host 指定"),
            "{stderr}"
        );
        let _ = fs::remove_dir_all(root);
    }

    /// `aicross hosts`：只列检测到的 agent，带厂商推定与依据；推定读的是
    /// 注入的家目录（临时目录，不碰真实家目录）。
    #[test]
    fn task50_hosts_lists_detected_with_inferred_vendor() {
        let root = fixture_root("t50-hosts");
        write_fixture_store(&root);
        let home = crate::paths::test_temp_dir().join(format!("t50-hosts-home-{}", Uuid::new_v4()));
        fs::create_dir_all(&home).unwrap();
        // claude 指向智谱端点。
        let settings = home.join(".claude").join("settings.json");
        fs::create_dir_all(settings.parent().unwrap()).unwrap();
        fs::write(
            &settings,
            r#"{"env":{"ANTHROPIC_BASE_URL":"https://open.bigmodel.cn/api"}}"#,
        )
        .unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_hosts_at(&root, &home, None, &mut stdout, &mut stderr);
        assert_eq!(code, 0, "{}", String::from_utf8(stderr).unwrap());
        let parsed: serde_json::Value =
            serde_json::from_str(String::from_utf8(stdout).unwrap().lines().next().unwrap()).unwrap();
        let hosts = parsed.as_array().unwrap();
        assert_eq!(hosts.len(), 2, "只列 claude 与 pi（kimi 未安装、codex 未探测）");
        assert_eq!(hosts[0]["id"], "claude");
        assert_eq!(hosts[0]["vendor"], "Zhipu", "厂商统一用资源库的英文名");
        assert_eq!(hosts[0]["vendorSource"], "端点");
        assert!(hosts[0]["vendorBasis"].as_str().unwrap().contains("bigmodel.cn"));
        assert_eq!(hosts[1]["id"], "pi");
        assert_eq!(hosts[1]["vendorSource"], "未知");
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(home);
    }

    /// config set-host-vendor：设定与 auto 清除；输出与 `aicross hosts`
    /// 同结构的条目。
    #[test]
    fn task50_set_host_vendor_writes_override_and_auto_clears() {
        let root = fixture_root("t50-vendor");
        write_fixture_store(&root);
        let home = crate::paths::test_temp_dir().join(format!("t50-vendor-home-{}", Uuid::new_v4()));
        fs::create_dir_all(&home).unwrap();
        let (code, stdout, stderr) = run_config_at_args(
            &root,
            &home,
            &["aicross", "config", "set-host-vendor", "claude", "DeepSeek"],
        );
        assert_eq!(code, 0, "{stderr}");
        let parsed = parse_stdout(&stdout);
        assert_eq!(parsed["vendor"], "DeepSeek");
        assert_eq!(parsed["vendorSource"], "用户设定");
        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("modes.json")).unwrap()).unwrap();
        assert_eq!(saved["hosts"]["claude"]["vendorOverride"], "DeepSeek");
        // auto 清除。
        let (code, stdout, stderr) = run_config_at_args(
            &root,
            &home,
            &["aicross", "config", "set-host-vendor", "claude", "auto"],
        );
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(parse_stdout(&stdout)["vendor"], "Anthropic");
        assert_eq!(parse_stdout(&stdout)["vendorSource"], "推定");
        // host 不区分大小写（fix-50-r1：与 --host 同一口径）。
        let (code, stdout, stderr) = run_config_at_args(
            &root,
            &home,
            &["aicross", "config", "set-host-vendor", "Claude", "DeepSeek"],
        );
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(parse_stdout(&stdout)["vendor"], "DeepSeek");
        // 厂商只认规范化后的已知厂商名或 auto，其它值退出 2。
        let (code, _, stderr) = run_config_at_args(
            &root,
            &home,
            &["aicross", "config", "set-host-vendor", "claude", "神秘厂商"],
        );
        assert_eq!(code, 2, "{stderr}");
        assert!(stderr.contains("不认识的厂商"), "{stderr}");
        // 非法 host。
        let (code, _, stderr) = run_config_at_args(
            &root,
            &home,
            &["aicross", "config", "set-host-vendor", "ghost", "DeepSeek"],
        );
        assert_eq!(code, 2, "{stderr}");
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(home);
    }

    /// config recommend：命令行与窗口命令（recommend_at）对同一夹具的
    /// 输出一致；只填空位与 recommended，manual 不动。厂商推定读注入的
    /// 临时家目录。
    /// 推荐夹具：候选的 verification 与 vendor 全部显式（vendor 用资源库
    /// 的英文厂商名，不手写中文厂商名），不依赖原生判定（那会读真实家
    /// 目录的 claude 配置）。三家里 Anthropic 是主 agent。
    fn write_recommend_store(root: &Path) {
        fs::write(
            root.join("resources.json"),
            r#"{"version":1,"agents":[
                {"id":"claude","name":"Claude Code","kind":"内置","detected":true,"models":[
                    {"id":"sonnet","agent":"claude","provider":"","vendor":"Anthropic","verification":"verified"}
                ]},
                {"id":"pi","name":"pi agent","kind":"内置","detected":true,"models":[
                    {"id":"glm-4.7","agent":"pi","provider":"zai","vendor":"Zhipu","verification":"verified"},
                    {"id":"deepseek-chat","agent":"pi","provider":"deepseek","vendor":"DeepSeek","verification":"verified"}
                ]}
            ]}"#,
        )
        .unwrap();
    }

    #[test]
    fn task50_recommend_window_output_matches_cli() {
        let root = fixture_root("t50-recommend");
        write_recommend_store(&root);
        let home = crate::paths::test_temp_dir().join(format!("t50-recommend-home-{}", Uuid::new_v4()));
        fs::create_dir_all(&home).unwrap();
        let write_fresh_modes = |root: &Path| {
            fs::write(
                root.join("modes.json"),
                r#"{"version":2,"hosts":{"claude":{"vendorOverride":"","defaultMode":"","seats":{}}}}"#,
            )
            .unwrap();
        };
        write_fresh_modes(&root);
        let (code, cli_stdout, stderr) = run_config_at_args(
            &root,
            &home,
            &["aicross", "config", "recommend", "--host", "claude"],
        );
        assert_eq!(code, 0, "{stderr}");
        let cli_value = parse_stdout(&cli_stdout);
        assert!(cli_value["notes"].as_array().is_some());
        assert!(cli_value["modes"].as_array().is_some());
        // unverified：全库已验证时是空数组（fix-57-r1 5）。
        assert_eq!(cli_value["unverified"], json!([]));
        // 经济模式三个阶段都填了人。
        let economy = cli_value["modes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|mode| mode["id"] == "builtin-economy")
            .unwrap();
        for stage in economy["stages"].as_array().unwrap() {
            for node in stage.as_array().unwrap() {
                assert_eq!(node["seat"]["origin"], "recommended");
            }
        }
        // 重放：清掉 modes.json 再跑窗口命令，输出与命令行一致。
        write_fresh_modes(&root);
        let window_value = crate::recommend::recommend_at_home(
            &root,
            &home,
            Some("claude"),
            None,
            false,
            None,
        )
        .unwrap();
        assert_eq!(window_value, cli_value, "窗口与命令行同实现体同输出");
        // 有可用但未验证的模型时：按 agent 计数，结构 { agent, count }
        //（fix-57-r1 5）；未验证的不进候选池，推荐结果不变。
        fs::write(
            root.join("resources.json"),
            r#"{"version":1,"agents":[
                {"id":"claude","name":"Claude Code","kind":"内置","detected":true,"models":[
                    {"id":"sonnet","agent":"claude","provider":"","vendor":"Anthropic","verification":"verified"}
                ]},
                {"id":"pi","name":"pi agent","kind":"内置","detected":true,"models":[
                    {"id":"glm-4.7","agent":"pi","provider":"zai","vendor":"Zhipu","verification":"verified"},
                    {"id":"deepseek-chat","agent":"pi","provider":"deepseek","vendor":"DeepSeek","verification":"verified"},
                    {"id":"glm-5.3","agent":"pi","provider":"zai","vendor":"Zhipu","verification":"unverified"}
                ]}
            ]}"#,
        )
        .unwrap();
        let with_unverified = crate::recommend::recommend_at_home(
            &root,
            &home,
            Some("claude"),
            None,
            false,
            None,
        )
        .unwrap();
        assert_eq!(
            with_unverified["unverified"],
            json!([{ "agent": "pi", "count": 1 }])
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(home);
    }

    /// skill 命令：install → status 的 ours 判定、uninstall；全程用临时
    /// 家目录，不碰真实家目录；status 输出与实现体一致。
    #[test]
    fn task50_skill_commands_use_injected_home_and_match_impl() {
        let home = crate::paths::test_temp_dir().join(format!("t50-skill-{}", Uuid::new_v4()));
        fs::create_dir_all(&home).unwrap();
        let args = |words: &[&str]| -> Vec<String> { words.iter().map(|word| word.to_string()).collect() };
        // install：claude 写 ~/.claude/skills/ai-cross。
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_skill_at(
            &home,
            &args(&["aicross", "skill", "install", "--host", "claude", "--scope", "global"]),
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0, "{}", String::from_utf8(stderr).unwrap());
        assert!(home.join(".claude/skills/ai-cross/SKILL.md").is_file());
        // status 与实现体输出一致。
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_skill_at(&home, &args(&["aicross", "skill", "status"]), &mut stdout, &mut stderr);
        assert_eq!(code, 0, "{}", String::from_utf8(stderr).unwrap());
        let cli_value: serde_json::Value =
            serde_json::from_str(String::from_utf8(stdout).unwrap().lines().next().unwrap()).unwrap();
        let impl_value = crate::skill_install::status_at(&home, None).unwrap();
        assert_eq!(cli_value, impl_value);
        assert_eq!(cli_value["hosts"][0]["host"], "claude");
        assert_eq!(cli_value["hosts"][0]["locations"][0]["kind"], "ours");
        assert_eq!(cli_value["bundledVersion"], crate::skill_install::bundled_version());
        // external 拒绝 install；uninstall 只删 ours。
        fs::create_dir_all(home.join(".claude/skills/ai-cross")).unwrap();
        fs::write(home.join(".claude/skills/ai-cross/SKILL.md"), "别人的").unwrap();
        fs::remove_file(home.join(".claude/skills/ai-cross/.aicross-install.json")).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_skill_at(
            &home,
            &args(&["aicross", "skill", "install", "--host", "claude", "--scope", "global"]),
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 1, "{}", String::from_utf8(stderr).unwrap());
        let mut stderr_message = String::from_utf8(stderr).unwrap();
        assert!(stderr_message.contains("由其他软件管理"), "{stderr_message}");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_skill_at(
            &home,
            &args(&["aicross", "skill", "uninstall", "--host", "claude", "--scope", "global"]),
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 1, "external 不删：{}", String::from_utf8(stderr).unwrap());
        stderr_message = String::from_utf8(stderr).unwrap();
        assert!(stderr_message.contains("由其他软件管理"), "{stderr_message}");
        let _ = fs::remove_dir_all(home);
    }

    /// TASK-53 slash 命令：参数与 --host 校验、家目录注入、输出与实现体
    /// 一致；全程用临时目录，不碰真实家目录。
    #[test]
    fn task53_slash_commands_use_injected_home_and_validate_args() {
        let root = fixture_root("t53-slash");
        write_fixture_store(&root);
        let home = crate::paths::test_temp_dir().join(format!("t53-cli-home-{}", Uuid::new_v4()));
        fs::create_dir_all(&home).unwrap();
        let args = |words: &[&str]| -> Vec<String> { words.iter().map(|word| word.to_string()).collect() };
        let run = |words: &[&str]| -> (i32, String, String) {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let code = run_slash_at(&root, &home, &args(words), &mut stdout, &mut stderr);
            (
                code,
                String::from_utf8(stdout).unwrap(),
                String::from_utf8(stderr).unwrap(),
            )
        };
        // 空 config：list 输出空清单与该 host 的默认调用写法。
        let (code, stdout, stderr) = run(&["aicross", "slash", "list", "--host", "claude"]);
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.contains("已列出短命令"), "{stderr}");
        let value: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(value["host"], "claude");
        assert_eq!(value["invoke"], "/ai-cross");
        assert_eq!(value["commands"].as_array().unwrap().len(), 0);
        // 参数与 --host 校验。
        let (code, _, stderr) = run(&["aicross", "slash"]);
        assert_eq!(code, 2);
        assert!(stderr.contains("缺少子命令"), "{stderr}");
        let (code, _, stderr) = run(&["aicross", "slash", "add", "--host", "claude", "--name", "x"]);
        assert_eq!(code, 2);
        assert!(stderr.contains("缺少 --mode"), "{stderr}");
        let (code, _, stderr) = run(&["aicross", "slash", "add", "--host", "claude", "--name", "X", "--mode", "blind"]);
        assert_eq!(code, 2);
        assert!(stderr.contains("命令名"), "{stderr}");
        let (code, _, stderr) = run(&["aicross", "slash", "add", "--host", "claude", "--name", "x", "--mode", "nope"]);
        assert_eq!(code, 2);
        assert!(stderr.contains("模式不存在"), "{stderr}");
        let (code, _, stderr) = run(&["aicross", "slash", "list", "--host", "kimi"]);
        assert_eq!(code, 2, "fixture 里 kimi 未检测到");
        assert!(stderr.contains("未知 host：kimi"), "{stderr}");
        // add：文件写进注入的家目录，stdout 与实现体一致。
        let (code, stdout, stderr) = run(&[
            "aicross", "slash", "add", "--host", "claude", "--name", "x", "--mode", "blind", "--note", "只报问题",
        ]);
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.contains("已添加短命令"), "{stderr}");
        let path = home.join(".claude").join("commands").join("x.md");
        assert!(path.is_file());
        let written = fs::read_to_string(&path).unwrap();
        assert!(written.contains("aicross-managed: true") && written.contains("$ARGUMENTS"), "{written}");
        let value: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(value, crate::slash::slash_list_at(&root, &home, "claude").unwrap());
        // `--note=<说明>` 写法：以 - 开头的说明不会被当成选项值。
        let (code, _, stderr) = run(&[
            "aicross", "slash", "add", "--host", "claude", "--name", "eq", "--mode", "blind",
            "--note=-开头说明",
        ]);
        assert_eq!(code, 0, "{stderr}");
        let written = fs::read_to_string(home.join(".claude").join("commands").join("eq.md")).unwrap();
        assert!(written.contains("--host claude。-开头说明\n$ARGUMENTS"), "{written}");
        // remove：文件随定义一起删；未找到的退出 2。
        let (code, _, stderr) = run(&["aicross", "slash", "remove", "--host", "claude", "--name", "x"]);
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.contains("已移除短命令"), "{stderr}");
        assert!(!path.exists());
        let (code, _, stderr) = run(&["aicross", "slash", "remove", "--host", "claude", "--name", "x"]);
        assert_eq!(code, 2);
        assert!(stderr.contains("未找到短命令：x"), "{stderr}");
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(home);
    }

    /// TASK-54 slash add-all 命令行：默认前缀 ax-、未知选项退出 2、
    /// --prefix 覆盖缺省、已有命令的模式跳过并给原因；全程临时目录。
    #[test]
    fn task54_slash_add_all_command_line_uses_default_prefix() {
        let root = fixture_root("t54-addall");
        write_fixture_store(&root);
        let home = crate::paths::test_temp_dir().join(format!("t54-cli-home-{}", Uuid::new_v4()));
        fs::create_dir_all(&home).unwrap();
        let args = |words: &[&str]| -> Vec<String> { words.iter().map(|word| word.to_string()).collect() };
        let run = |words: &[&str]| -> (i32, String, String) {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let code = run_slash_at(&root, &home, &args(words), &mut stdout, &mut stderr);
            (
                code,
                String::from_utf8(stdout).unwrap(),
                String::from_utf8(stderr).unwrap(),
            )
        };
        // 未知选项退出 2。
        let (code, _, stderr) = run(&["aicross", "slash", "add-all", "--host", "claude", "--fast"]);
        assert_eq!(code, 2);
        assert!(stderr.contains("不认识的参数：--fast"), "{stderr}");
        // 默认前缀 ax-：六个文件落进注入的家目录。
        let (code, stdout, stderr) = run(&["aicross", "slash", "add-all", "--host", "claude"]);
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.contains("已生成短命令") && !stderr.contains("跳过"), "{stderr}");
        let value: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(value["added"].as_array().unwrap().len(), 6);
        assert_eq!(value["skipped"].as_array().unwrap().len(), 0);
        assert!(home.join(".claude").join("commands").join("ax-blind.md").is_file());
        // --prefix 覆盖缺省：模式都已有命令，全部跳过并给原因。
        let (code, stdout, stderr) =
            run(&["aicross", "slash", "add-all", "--host", "claude", "--prefix", "re-"]);
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.contains("跳过 6 条"), "{stderr}");
        let value: serde_json::Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(value["added"].as_array().unwrap().len(), 0);
        let skipped = value["skipped"].as_array().unwrap();
        assert_eq!(skipped.len(), 6);
        assert!(
            skipped[0]["reason"].as_str().unwrap().starts_with("该模式已有短命令："),
            "{}",
            skipped[0]["reason"]
        );
        // 非法前缀退出 2。
        let (code, _, stderr) =
            run(&["aicross", "slash", "add-all", "--host", "claude", "--prefix", "AX"]);
        assert_eq!(code, 2);
        assert!(stderr.contains("前缀"), "{stderr}");
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(home);
    }

    /// run_config_at 的带参包装（set-host-vendor / recommend 用）。
    fn run_config_at_args(root: &Path, home: &Path, words: &[&str]) -> (i32, String, String) {
        let args: Vec<String> = words.iter().map(|word| word.to_string()).collect();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_config_at(
            root,
            home,
            None,
            &args,
            &AcpOps::production(),
            &mut stdout,
            &mut stderr,
        );
        (
            code,
            String::from_utf8(stdout).unwrap(),
            String::from_utf8(stderr).unwrap(),
        )
    }

    /// dispatch 的 --host：不是检测到的 agent 退出 2（引擎未拉起）；
    /// 文本照纪律走 --text-file。
    #[test]
    fn task50_dispatch_rejects_unknown_host_before_spawn() {
        let root = fixture_root("t50-dispatch-host");
        write_fixture_store(&root);
        let text_file = root.join("text.txt");
        fs::write(&text_file, "任务").unwrap();
        let (code, stdout, stderr) = run_with(
            &root,
            &[
                "aicross", "dispatch", "--project", root.to_str().unwrap(),
                "--text-file", text_file.to_str().unwrap(), "--host", "kimi",
            ],
            &ResourcesOps::production(),
        );
        assert_eq!(code, 2, "{stderr}");
        assert!(stdout.is_empty());
        assert!(stderr.contains("未知 host：kimi"), "{stderr}");
        assert!(!root.join("runs").exists(), "未进引擎，run 目录不应建");
        let _ = fs::remove_dir_all(root);
    }

    // ── TASK-55：preset 命令层 ──────────────────────────────────────────

    /// `preset save` 读文件、解析、输出保存后的预设与完整 modes；参数错
    /// 与解析失败退出 2，原因写在 stderr。
    #[test]
    fn task55_cli_preset_save_reads_file_and_outputs_modes() {
        let root = fixture_root("preset-cli");
        let file = root.join("mode.json");
        fs::write(
            &file,
            r#"{"name":"界面模式","description":"介绍","stages":[{"nodes":[{"role":"执行","visibility":"共享","write":"只读"}]}]}"#,
        )
        .unwrap();
        let (code, stdout, stderr) = run_with(
            &root,
            &["aicross", "preset", "save", "--file", file.to_str().unwrap()],
            &ResourcesOps::production(),
        );
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.contains("已保存模式"), "{stderr}");
        let parsed: Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["preset"]["name"], "界面模式");
        assert_eq!(parsed["modes"].as_array().unwrap().len(), 7);
        let saved = parsed["modes"].as_array().unwrap().last().unwrap();
        assert_eq!(saved["id"], parsed["preset"]["id"]);
        assert_eq!(saved["description"], "介绍");
        assert!(root.join("presets.json").is_file());
        // 参数错与文件错：缺 --file、文件不存在、json 坏、校验不过。
        let (code, _, stderr) =
            run_with(&root, &["aicross", "preset", "save"], &ResourcesOps::production());
        assert_eq!(code, 2);
        assert!(stderr.contains("缺少 --file"), "{stderr}");
        let (code, _, stderr) = run_with(
            &root,
            &["aicross", "preset", "save", "--file", root.join("nope.json").to_str().unwrap()],
            &ResourcesOps::production(),
        );
        assert_eq!(code, 2);
        assert!(stderr.contains("读取预设文件失败"), "{stderr}");
        fs::write(&file, "{ not json").unwrap();
        let (code, _, stderr) = run_with(
            &root,
            &["aicross", "preset", "save", "--file", file.to_str().unwrap()],
            &ResourcesOps::production(),
        );
        assert_eq!(code, 2);
        assert!(stderr.contains("无法解析"), "{stderr}");
        fs::write(
            &file,
            r#"{"name":"共享模式","stages":[{"nodes":[{"role":"执行","visibility":"共享","write":"只读"}]}]}"#,
        )
        .unwrap();
        let (code, _, stderr) = run_with(
            &root,
            &["aicross", "preset", "save", "--file", file.to_str().unwrap()],
            &ResourcesOps::production(),
        );
        assert_eq!(code, 2);
        assert!(stderr.contains("重名"), "{stderr}");
        // 未知与缺少子命令。
        let (code, _, stderr) = run_with(
            &root,
            &["aicross", "preset", "frobnicate"],
            &ResourcesOps::production(),
        );
        assert_eq!(code, 2);
        assert!(stderr.contains("未知子命令"), "{stderr}");
        let (code, _, stderr) =
            run_with(&root, &["aicross", "preset"], &ResourcesOps::production());
        assert_eq!(code, 2);
        assert!(stderr.contains("缺少子命令"), "{stderr}");
        let _ = fs::remove_dir_all(root);
    }

    /// `preset remove` 的参数校验发生在读家目录之前，不碰真实家目录。
    #[test]
    fn task55_cli_preset_remove_arg_checks() {
        let root = fixture_root("preset-remove-cli");
        let (code, _, stderr) = run_with(
            &root,
            &["aicross", "preset", "remove"],
            &ResourcesOps::production(),
        );
        assert_eq!(code, 2);
        assert!(stderr.contains("缺少模式 id"), "{stderr}");
        let (code, _, stderr) = run_with(
            &root,
            &["aicross", "preset", "remove", "u1", "u2"],
            &ResourcesOps::production(),
        );
        assert_eq!(code, 2);
        assert!(stderr.contains("不认识的参数"), "{stderr}");
        let _ = fs::remove_dir_all(root);
    }
}

