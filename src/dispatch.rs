use crate::channels::{self, ChannelErrorKind, ChannelOutput};
use crate::paths::aicross_root;
use crate::process::{cancel_registered, run_process_registered, run_process_registered_with_activity, ProcessActivity};
use chrono::{DateTime, Local, SecondsFormat};
use glob::{MatchOptions, Pattern};
use regex::Regex;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Condvar, Mutex, OnceLock};
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
    #[serde(default)]
    pub validation_command: String,
    #[serde(default = "default_max_requests")]
    pub max_requests: usize,
    #[serde(default = "default_swarm_limit")]
    pub swarm_limit: usize,
    #[serde(default)]
    pub acp_enabled: bool,
    /// TASK-12：修正轮上限（验收 NEEDS_FIX 与验证命令未通过的重派次数），到顶后生成遗留节点。
    #[serde(default = "default_fix_round_limit")]
    pub fix_round_limit: usize,
}

impl Default for DispatchSettings {
    fn default() -> Self {
        Self {
            timeout_secs: default_timeout(),
            concurrency: default_concurrency(),
            redact: true,
            write_project_dispatch: true,
            cc_switch_path: String::new(),
            validation_command: String::new(),
            max_requests: default_max_requests(),
            swarm_limit: default_swarm_limit(),
            acp_enabled: false,
            fix_round_limit: default_fix_round_limit(),
        }
    }
}

fn default_fix_round_limit() -> usize {
    1
}
fn default_timeout() -> u64 {
    600
}
fn default_concurrency() -> usize {
    3
}
fn default_max_requests() -> usize {
    8
}
fn default_swarm_limit() -> usize {
    4
}
fn default_true() -> bool {
    true
}

fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

fn oneshot() -> String {
    "oneshot".into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchNodeRequest {
    pub node_id: String,
    #[serde(default = "default_stage")]
    pub stage: u32,
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
    #[serde(default)]
    pub previous_runner: String,
    #[serde(default)]
    pub write: String,
    #[serde(default)]
    pub original_request: String,
    #[serde(default)]
    pub upstream_artifacts: Vec<DispatchArtifacts>,
    /// TASK-05B 主备/熔断：主目标 error/timeout/identity_mismatch 时用备选重跑一次。
    #[serde(default)]
    pub fallback: Option<FallbackSpec>,
    #[serde(default)]
    pub rollback_before: bool,
    /// TASK-12：为真时引擎在盲目录物化后、派发前跑一遍验证命令，输出落 VALIDATION.txt。
    #[serde(default)]
    pub review_material_validation: bool,
    /// TASK-13：修正轮执行节点在验证通过后自动做红绿核对。
    #[serde(default)]
    pub fix_round: bool,
}

/// 备选目标（图契约 Target.fallback 的三字段 + R2 追加的下发 vendor；camelCase 由外层结构继承）。
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct FallbackSpec {
    pub agent: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    /// TASK-05B R2：前端已知的真实厂商随请求下发；空则按模型 ID 推断。
    #[serde(default)]
    pub vendor: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ChangedFile {
    pub path: String,
    pub content: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DispatchArtifacts {
    #[serde(default)]
    pub diff: String,
    #[serde(default)]
    pub changed_files: Vec<ChangedFile>,
    #[serde(default)]
    pub worktree: Option<String>,
    #[serde(default)]
    pub stdout_summary: String,
    // R1 复审：本次尝试落下的派发前快照目录（没改动时为空）。额度用尽切备选前的回滚只认它，不认历史快照。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub before_dir: String,
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
    pub broad_redacted: usize,
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
    pub stage: u32,
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
    pub runner: String,
    pub runner_note: String,
    pub workdir: String,
    pub dispatch_file: String,
    pub frontmatter: String,
    pub output_preview: String,
    pub error: String,
    pub artifacts: Option<DispatchArtifacts>,
    pub validation_output: String,
    /// TASK-13：红绿核对结论；普通节点不返回这两个追加字段。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_note: Option<String>,
    /// TASK-05B：本结果由备选重跑产生时，主目标与备选目标的 "agent/model"。
    #[serde(default)]
    pub fallback_from: String,
    #[serde(default)]
    pub fallback_to: String,
    /// 主失败但跳过备选的原因（空串 = 未发生跳过）；可写节点为「可写节点」。
    #[serde(default)]
    pub fallback_skipped: String,
    /// 额度用尽时能从通道响应解析出的恢复时间（ISO 8601）。
    #[serde(default)]
    pub quota_resets_at: Option<String>,
    /// 仅供引擎在本轮结束后通知前端刷新资源，不进入 IPC 契约。
    #[serde(skip)]
    resources_changed: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchStageResult {
    pub run_id: String,
    pub results: Vec<DispatchNodeResult>,
    pub state_file: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchStateSummary {
    pub exists: bool,
    pub unfinished: bool,
    pub content: String,
    pub run_id: String,
    pub active_node_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RollbackSummary {
    pub restored: Vec<String>,
    pub removed: Vec<String>,
    pub skipped: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileNode {
    pub node_id: String,
    pub verdict: String,
    pub record_path: String,
    pub reply_text: String,
    pub artifacts: Option<DispatchArtifacts>,
    pub error: String,
    pub session_id: String,
    pub runner: String,
    pub usage: ReconcileUsage,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileSummary {
    pub exists: bool,
    pub run_id: String,
    pub nodes: Vec<ReconcileNode>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct DispatchStateHeader {
    schema: String,
    run_id: String,
    stage: u32,
    unfinished: bool,
    active_node_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DispatchProgress {
    run_id: String,
    node_id: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    elapsed_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    idle_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repeated_tool_calls: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_tool: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
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
    #[serde(default = "oneshot")]
    runner: String,
    #[serde(default)]
    runner_note: String,
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
    /// TASK-05B：「主」或「备」（备选重跑的那条留痕）。追加字段，不改名不删。
    #[serde(default)]
    attempt: String,
    /// 备选的实际尝试顺序；主尝试为 0。
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    attempt_index: u32,
    /// TASK-05B R2：生效的写权限（盲节点强制只读之后的值）。
    #[serde(default)]
    write: String,
    /// TASK-05B R2：主失败但未切备选的原因（空串 = 未发生）；可写节点为「可写节点」。
    #[serde(default)]
    fallback_skipped: String,
    /// 项目内快照记相对路径；write_project_dispatch=false 时例外为绝对路径。
    #[serde(default)]
    before_dir: String,
    /// TASK-12：盲目录里是否附了 VALIDATION.txt（审查材料附带验证输出）。
    #[serde(default)]
    validation_attached: bool,
    /// TASK-13：红绿核对结论；非修正轮为空。
    #[serde(default)]
    pinned: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    swarm_item: Option<u32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    merge_result: String,
}

fn excluded_snapshot_directory(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".dispatch"
            | ".aicross-runtime"
            | ".pytest_cache"
            | "__pycache__"
            | "node_modules"
            | "target"
            | "dist"
            | ".venv"
            | "build"
            | "coverage"
            | ".next"
            | "vendor"
    )
}

fn git_result(
    args: &[&str],
    cwd: &Path,
    registry_key: Option<&str>,
) -> Result<crate::process::ProcessResult, String> {
    run_process_registered(
        Path::new("git"),
        &args.iter().map(|value| (*value).to_string()).collect::<Vec<_>>(),
        None,
        Duration::from_secs(120),
        Some(cwd),
        registry_key,
    )
}

fn git_result_with_timeout(
    args: &[&str],
    cwd: &Path,
    registry_key: Option<&str>,
    timeout: Duration,
) -> Result<crate::process::ProcessResult, String> {
    #[cfg(test)]
    if args.first() == Some(&"apply")
        && SWARM_TEST_TIMEOUT_NEXT_APPLY.with(|flag| flag.replace(false))
    {
        return Ok(crate::process::ProcessResult {
            code: None,
            stdout: String::new(),
            stderr: String::new(),
            timed_out: true,
            truncated: false,
            timeout,
        });
    }
    run_process_registered(
        Path::new("git"),
        &args.iter().map(|value| (*value).to_string()).collect::<Vec<_>>(),
        None,
        timeout,
        Some(cwd),
        registry_key,
    )
}

#[cfg(test)]
thread_local! {
    static SWARM_TEST_TIMEOUT_NEXT_APPLY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SWARM_TEST_FAIL_NEXT_RESTORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SWARM_TEST_FAIL_NEXT_RESET: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SWARM_TEST_FAIL_PROJECT_HEAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SWARM_TEST_TRUNCATE_NEXT_CHANGED_STATUS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SWARM_TEST_DIRTY_BEFORE_ITEM: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

fn process_succeeded(result: &crate::process::ProcessResult) -> bool {
    !result.timed_out && result.code == Some(0)
}

fn process_note(result: &crate::process::ProcessResult, fallback: &str) -> String {
    if result.timed_out {
        return format!("{fallback}：命令超时");
    }
    let detail = [result.stderr.trim(), result.stdout.trim()]
        .into_iter()
        .find(|value| !value.is_empty())
        .unwrap_or(fallback);
    safe_trace(detail)
}

fn reset_swarm_item(
    project: &Path,
    baseline: &str,
    selected: &[String],
    cancel_key: &str,
    timeout: Duration,
) -> Result<(), String> {
    if baseline.is_empty() {
        return Err("无法确定回退基线".into());
    }
    #[cfg(test)]
    if SWARM_TEST_FAIL_NEXT_RESET.with(|flag| flag.replace(false)) {
        return Err("回退失败：测试注入的 git reset 失败".into());
    }
    let reset = git_result_with_timeout(&["reset", "--soft", baseline], project, Some(cancel_key), timeout)
        .map_err(|error| format!("回退失败：{error}"))?;
    if !process_succeeded(&reset) {
        return Err(format!("回退失败：{}", process_note(&reset, "git reset --soft 未成功")));
    }
    restore_swarm_paths(project, selected, cancel_key, timeout)
}

fn restore_after_apply_conflict(
    project: &Path,
    selected: &[String],
    cancel_key: &str,
    timeout: Duration,
) -> Result<(), String> {
    #[cfg(test)]
    if SWARM_TEST_FAIL_NEXT_RESTORE.with(|flag| flag.replace(false)) {
        return Err("测试注入的定向恢复失败".into());
    }
    restore_swarm_paths(project, selected, cancel_key, timeout)
}

fn restore_swarm_paths(
    project: &Path,
    selected: &[String],
    cancel_key: &str,
    timeout: Duration,
) -> Result<(), String> {
    if selected.is_empty() {
        return Ok(());
    }
    let mut reset_args = vec!["reset", "-q", "HEAD", "--"];
    reset_args.extend(selected.iter().map(String::as_str));
    let reset = git_result_with_timeout(&reset_args, project, Some(cancel_key), timeout)
        .map_err(|error| format!("恢复失败：{error}"))?;
    if !process_succeeded(&reset) {
        return Err(format!("恢复失败：{}", process_note(&reset, "git reset 路径未成功")));
    }
    let mut tracked = Vec::new();
    for path in selected {
        let listed = git_result(&["ls-files", "--error-unmatch", "--", path], project, Some(cancel_key))?;
        if process_succeeded(&listed) {
            tracked.push(path.as_str());
        }
    }
    if !tracked.is_empty() {
        let mut checkout_args = vec!["checkout", "--"];
        checkout_args.extend(tracked);
        let checkout = git_result_with_timeout(&checkout_args, project, Some(cancel_key), timeout)
            .map_err(|error| format!("恢复失败：{error}"))?;
        if !process_succeeded(&checkout) {
            return Err(format!("恢复失败：{}", process_note(&checkout, "git checkout 路径未成功")));
        }
    }
    let status = git_result(&["status", "--porcelain=v1", "-z", "--untracked-files=all"], project, Some(cancel_key))
        .map_err(|error| format!("恢复后检查项目状态失败：{error}"))?;
    if !process_succeeded(&status) || status.truncated {
        return Err(if status.truncated { "恢复后项目状态输出被截断".into() } else { process_note(&status, "恢复后检查项目状态失败") });
    }
    let selected_set = selected.iter().map(String::as_str).collect::<HashSet<_>>();
    for (path, untracked) in parse_git_status_paths(&status.stdout) {
        if untracked && selected_set.contains(path.as_str()) {
            let target = project.join(&path);
            let metadata = fs::symlink_metadata(&target).map_err(|error| format!("清理残留未跟踪文件失败 {}：{error}", target.display()))?;
            if metadata.is_file() || metadata.file_type().is_symlink() {
                fs::remove_file(&target).map_err(|error| format!("清理残留未跟踪文件失败 {}：{error}", target.display()))?;
            }
        }
    }
    let status = git_result(&["status", "--porcelain=v1", "-z", "--untracked-files=all"], project, Some(cancel_key))
        .map_err(|error| format!("恢复后复核项目状态失败：{error}"))?;
    if !process_succeeded(&status) || status.truncated {
        return Err(if status.truncated { "恢复后项目状态输出被截断".into() } else { process_note(&status, "恢复后复核项目状态失败") });
    }
    // 复核只看排除目录之外（R1 复审）：验证命令留下的 __pycache__ 之类是被容忍的残留，
    // 与逐件脏检查同一口径，不能把「验证红」误判成「恢复失败」而整批停下。
    let remaining = parse_git_status_paths(&status.stdout).into_iter()
        .map(|(path, _)| path)
        .filter(|path| !excluded_swarm_path(path))
        .collect::<Vec<_>>();
    if !remaining.is_empty() {
        return Err(format!("恢复后项目仍有改动：{}", summarize_paths(&remaining)));
    }
    Ok(())
}

fn existing_project_files(root: &Path) -> Result<Vec<String>, String> {
    fn walk(root: &Path, current: &Path, files: &mut Vec<String>) -> Result<(), String> {
        for entry in fs::read_dir(current)
            .map_err(|error| format!("读取项目目录失败 {}：{error}", current.display()))?
        {
            let entry = entry.map_err(|error| format!("读取项目目录项失败：{error}"))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("读取项目文件元数据失败 {}：{error}", path.display()))?;
            if is_link_or_junction(&metadata) {
                continue;
            }
            if metadata.is_dir() {
                if !excluded_snapshot_directory(&entry.file_name().to_string_lossy()) {
                    walk(root, &path, files)?;
                }
            } else if metadata.is_file() {
                files.push(
                    path.strip_prefix(root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    walk(root, root, &mut files)?;
    files.sort();
    Ok(files)
}

fn compile_glob(pattern: &str) -> Result<Pattern, String> {
    Pattern::new(pattern).map_err(|error| format!("glob {pattern:?} 无效：{error}"))
}

fn normalized_swarm_glob(pattern: &str, project: &Path) -> String {
    let mut normalized = pattern.trim().replace('\\', "/");
    while let Some(rest) = normalized.strip_prefix("./") {
        normalized = rest.to_string();
    }
    while normalized.ends_with('/') {
        normalized.pop();
    }
    if !normalized.contains(['*', '?', '[']) && project.join(&normalized).is_dir() {
        normalized.push_str("/**");
    }
    normalized
}

fn check_swarm_plan_core(project: &Path, items: &[SwarmPlanItem], limit: usize) -> Result<SwarmPlanCheck, String> {
    let status = git_result(&["status", "--porcelain"], project, None)?;
    if !process_succeeded(&status) {
        return Err(process_note(&status, "读取 git 状态失败"));
    }
    let files = existing_project_files(project)?;
    let mut matched = Vec::new();
    let mut unmatched = Vec::new();
    let mut notes = Vec::new();
    if items.len() > limit {
        notes.push(format!("超过蜂群件数上限，仅检查前 {limit} 件"));
    }
    for item in items.iter().take(limit) {
        let mut item_matches = HashSet::new();
        for pattern in &item.files {
            let normalized = normalized_swarm_glob(pattern, project);
            let matcher = match compile_glob(&normalized) {
                Ok(matcher) => matcher,
                Err(error) => {
                    unmatched.push(SwarmUnmatched { item: item.item, glob: pattern.clone() });
                    notes.push(error);
                    continue;
                }
            };
            let paths = files
                .iter()
                .filter(|path| matcher.matches_with(path, MatchOptions {
                    case_sensitive: false,
                    require_literal_separator: true,
                    require_literal_leading_dot: false,
                }))
                .cloned()
                .collect::<Vec<_>>();
            if paths.is_empty() {
                unmatched.push(SwarmUnmatched { item: item.item, glob: pattern.clone() });
            }
            item_matches.extend(paths);
        }
        matched.push((item.item, item_matches));
    }
    let mut overlaps = Vec::new();
    for left in 0..matched.len() {
        for right in (left + 1)..matched.len() {
            if let Some(path) = matched[left].1.intersection(&matched[right].1).min().cloned() {
                overlaps.push(SwarmOverlap { a: matched[left].0, b: matched[right].0, path });
            }
        }
    }
    Ok(SwarmPlanCheck { dirty: !status.stdout.trim().is_empty(), overlaps, unmatched, note: notes.join("；") })
}

#[tauri::command]
pub async fn check_swarm_plan(project_dir: String, items: Vec<SwarmPlanItem>, settings: DispatchSettings) -> Result<SwarmPlanCheck, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let project = project_path(&project_dir)?;
        check_swarm_plan_core(&project, &items, settings.swarm_limit.max(1))
    })
    .await
    .map_err(|error| format!("蜂群计划检查线程失败：{error}"))?
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SwarmPlanItem {
    pub item: u32,
    #[serde(default)]
    pub files: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SwarmOverlap {
    pub a: u32,
    pub b: u32,
    pub path: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SwarmUnmatched {
    pub item: u32,
    pub glob: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SwarmPlanCheck {
    pub dirty: bool,
    pub overlaps: Vec<SwarmOverlap>,
    pub unmatched: Vec<SwarmUnmatched>,
    pub note: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SwarmMergeItem {
    pub node_id: String,
    pub item: u32,
    pub worktree: String,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub accept: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SwarmMergeResult {
    pub node_id: String,
    pub item: u32,
    pub result: String,
    pub commit: String,
    pub note: String,
    pub test_output: String,
}

fn truncate_utf8(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        return value.to_string();
    }
    let mut end = limit;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

fn skipped_swarm_results(items: &[SwarmMergeItem], note: &str) -> Vec<SwarmMergeResult> {
    items
        .iter()
        .map(|item| SwarmMergeResult {
            node_id: item.node_id.clone(),
            item: item.item,
            result: "skipped".into(),
            note: note.into(),
            ..SwarmMergeResult::default()
        })
        .collect()
}

fn append_swarm_record(project: &Path, log_dir: &Path, run_id: &str, result: &SwarmMergeResult) -> Result<(), String> {
    ensure_log_dir(&log_dir)?;
    let now = Local::now().to_rfc3339_opts(SecondsFormat::Secs, false);
    let record = DispatchRecord {
        schema: "aicross-dispatch/1".into(),
        run_id: run_id.into(),
        node: result.node_id.clone(),
        agent: "git".into(),
        role: "合并".into(),
        workdir: project.to_string_lossy().into_owned(),
        status: if result.result == "merged" { "ok".into() } else { "error".into() },
        started_at: now.clone(),
        ended_at: now,
        error: result.note.clone(),
        swarm_item: Some(result.item),
        merge_result: result.result.clone(),
        ..DispatchRecord::default()
    };
    let frontmatter = serialize_frontmatter(&record)?;
    write_record(
        &log_dir,
        &record,
        &frontmatter,
        "",
        &result.note,
        None,
        &result.test_output,
        "",
    )?;
    Ok(())
}

fn append_swarm_records(project: &Path, log_dir: &Path, run_id: &str, results: &mut [SwarmMergeResult]) {
    for result in results {
        if let Err(error) = append_swarm_record(project, log_dir, run_id, result) {
            if result.note.is_empty() {
                result.note = format!("写入合并记录失败：{error}");
            } else {
                result.note.push_str(&format!("；写入合并记录失败：{error}"));
            }
        }
    }
}

fn registered_worktrees(project: &Path, cancel_key: &str) -> Result<HashSet<PathBuf>, String> {
    let listed = git_result(&["worktree", "list", "--porcelain"], project, Some(cancel_key))?;
    if !process_succeeded(&listed) {
        return Err(process_note(&listed, "读取项目 worktree 清单失败"));
    }
    Ok(listed.stdout.lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .filter_map(|path| match fs::canonicalize(path) {
            Ok(path) => Some(path),
            Err(error) => {
                eprintln!("跳过无法规范化的登记 worktree {path}：{error}");
                None
            }
        })
        .collect())
}

fn excluded_swarm_path(path: &str) -> bool {
    path.replace('\\', "/")
        .split('/')
        .any(excluded_snapshot_directory)
}

fn parse_git_status_paths(status: &str) -> Vec<(String, bool)> {
    let fields = status.split_terminator('\0').collect::<Vec<_>>();
    let mut paths = Vec::new();
    let mut index = 0;
    while index < fields.len() {
        let field = fields[index];
        if field.len() < 3 {
            index += 1;
            continue;
        }
        let state = &field[..2];
        let path = field[3..].replace('\\', "/");
        let bytes = state.as_bytes();
        if (matches!(bytes[0], b'R' | b'C') || matches!(bytes[1], b'R' | b'C')) && index + 1 < fields.len() {
            index += 1;
        }
        paths.push((path, state == "??"));
        index += 1;
    }
    paths
}

fn swarm_changed_paths(
    worktree: &Path,
    base: &str,
    cancel_key: &str,
) -> Result<Vec<String>, String> {
    let committed = git_result(&["diff", "--name-only", "-z", base, "--"], worktree, Some(cancel_key))?;
    if !process_succeeded(&committed) {
        return Err(process_note(&committed, "读取 worktree 改动范围失败"));
    }
    if committed.truncated {
        return Err("读取 worktree 改动范围失败：git diff 输出被截断".into());
    }
    let status = git_result(&["status", "--porcelain=v1", "-z", "--untracked-files=all"], worktree, Some(cancel_key))?;
    #[cfg(test)]
    let status = {
        let mut status = status;
        if SWARM_TEST_TRUNCATE_NEXT_CHANGED_STATUS.with(|flag| flag.replace(false)) {
            status.truncated = true;
        }
        status
    };
    if !process_succeeded(&status) {
        return Err(process_note(&status, "读取 worktree 状态失败"));
    }
    if status.truncated {
        return Err("读取 worktree 状态失败：输出被截断".into());
    }
    let mut paths = committed.stdout.split_terminator('\0')
        .filter(|path| !path.is_empty())
        .map(|path| path.replace('\\', "/"))
        .chain(parse_git_status_paths(&status.stdout).into_iter().map(|(path, _)| path))
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn swarm_path_matches(path: &str, patterns: &[String]) -> bool {
    let mut matched = false;
    for pattern in patterns {
        let mut normalized = pattern.trim().replace('\\', "/");
        while let Some(rest) = normalized.strip_prefix("./") {
            normalized = rest.to_string();
        }
        while normalized.ends_with('/') {
            normalized.pop();
        }
        if !normalized.contains(['*', '?', '[']) {
            matched |= path == normalized || path.strip_prefix(&normalized).is_some_and(|rest| rest.starts_with('/'));
            continue;
        }
        match compile_glob(&normalized) {
            Ok(matcher) => matched |= matcher.matches_with(path, MatchOptions {
                case_sensitive: false,
                require_literal_separator: true,
                require_literal_leading_dot: false,
            }),
            Err(_) => {}
        }
    }
    matched
}

fn invalid_swarm_globs(patterns: &[String]) -> Vec<String> {
    patterns.iter().filter_map(|pattern| {
        let mut normalized = pattern.trim().replace('\\', "/");
        while let Some(rest) = normalized.strip_prefix("./") {
            normalized = rest.to_string();
        }
        while normalized.ends_with('/') {
            normalized.pop();
        }
        if normalized.contains(['*', '?', '[']) && compile_glob(&normalized).is_err() {
            Some(pattern.clone())
        } else {
            None
        }
    }).collect()
}

fn append_note(note: &mut String, addition: &str) {
    if addition.is_empty() {
        return;
    }
    if !note.is_empty() {
        note.push('；');
    }
    note.push_str(addition);
}

fn summarize_paths(paths: &[String]) -> String {
    let shown = paths.iter().take(20).cloned().collect::<Vec<_>>().join(", ");
    if paths.len() > 20 {
        format!("{shown} 等 {} 项", paths.len())
    } else {
        shown
    }
}

fn restore_validation_exclusions(
    project: &Path,
    cancel_key: &str,
    timeout: Duration,
) -> Result<Vec<String>, String> {
    let status = git_result(&["status", "--porcelain=v1", "-z", "--untracked-files=all"], project, Some(cancel_key))
        .map_err(|error| format!("检查验证后项目状态失败：{error}"))?;
    if !process_succeeded(&status) {
        return Err(process_note(&status, "检查验证后项目状态失败"));
    }
    if status.truncated {
        return Err("检查验证后项目状态失败：输出被截断".into());
    }
    let paths = parse_git_status_paths(&status.stdout);
    let outside = paths.iter()
        .filter(|(path, _)| !excluded_swarm_path(path))
        .map(|(path, _)| path.clone())
        .collect::<Vec<_>>();
    if !outside.is_empty() {
        return Ok(outside);
    }
    let tracked_excluded = paths.into_iter()
        .filter(|(path, untracked)| !*untracked && excluded_swarm_path(path))
        .map(|(path, _)| path)
        .collect::<Vec<_>>();
    if !tracked_excluded.is_empty() {
        let mut args = vec!["checkout", "--"];
        args.extend(tracked_excluded.iter().map(String::as_str));
        let checkout = git_result_with_timeout(&args, project, Some(cancel_key), timeout)
            .map_err(|error| format!("清理验证生成文件失败：{error}"))?;
        if !process_succeeded(&checkout) {
            return Err(format!("清理验证生成文件失败：{}", process_note(&checkout, "git checkout 未成功")));
        }
    }
    Ok(Vec::new())
}

fn project_dirty_paths(project: &Path, cancel_key: &str) -> Result<Vec<String>, String> {
    let status = git_result(&["status", "--porcelain=v1", "-z", "--untracked-files=all"], project, Some(cancel_key))?;
    if !process_succeeded(&status) {
        return Err(process_note(&status, "读取 git 状态失败"));
    }
    if status.truncated {
        return Err("读取 git 状态失败：输出被截断".into());
    }
    Ok(parse_git_status_paths(&status.stdout).into_iter().map(|(path, _)| path).collect())
}

fn audit_staged_swarm_paths(
    worktree: &Path,
    patterns: &[String],
    base: &str,
    cancel_key: &str,
) -> Result<(Vec<String>, Vec<String>), String> {
    let staged = git_result(&["diff", "--cached", "--name-only", "--no-renames", "-z", base, "--"], worktree, Some(cancel_key))?;
    if !process_succeeded(&staged) {
        return Err(process_note(&staged, "复核暂存清单失败"));
    }
    if staged.truncated {
        return Err("复核暂存清单失败：git diff 输出被截断".into());
    }
    let mut allowed = Vec::new();
    let mut rejected = Vec::new();
    for path in staged.stdout.split_terminator('\0').filter(|path| !path.is_empty()).map(|path| path.replace('\\', "/")) {
        let in_scope = swarm_path_matches(&path, patterns);
        if excluded_swarm_path(&path) || !in_scope {
            rejected.push(path);
        } else {
            allowed.push(path);
        }
    }
    if !rejected.is_empty() {
        let mut reset_args = vec!["reset", "-q", base, "--"];
        reset_args.extend(rejected.iter().map(String::as_str));
        let reset = git_result(&reset_args, worktree, Some(cancel_key))?;
        if !process_succeeded(&reset) {
            return Err(process_note(&reset, "从暂存清单移除范围外改动失败"));
        }
    }
    Ok((allowed, rejected))
}

fn write_swarm_patch(
    worktree: &Path,
    base: &str,
    selected: &[String],
    cancel_key: &str,
) -> Result<PathBuf, String> {
    let patch_path = std::env::temp_dir().join(format!("aicross-swarm-{}.patch", Uuid::new_v4()));
    let output_arg = format!("--output={}", patch_path.to_string_lossy());
    let mut args = vec!["diff", "--binary", "--full-index", "--cached", base, output_arg.as_str(), "--"];
    args.extend(selected.iter().map(String::as_str));
    let diff = git_result(&args, worktree, Some(cancel_key));
    match diff {
        Err(error) => {
            let _ = fs::remove_file(&patch_path);
            Err(format!("生成声明范围补丁失败：{error}"))
        }
        Ok(diff) if !process_succeeded(&diff) => {
            let _ = fs::remove_file(&patch_path);
            Err(process_note(&diff, "生成声明范围补丁失败"))
        }
        Ok(diff) if diff.truncated => {
            let _ = fs::remove_file(&patch_path);
            Err("生成声明范围补丁失败：git diff 输出被截断".into())
        }
        Ok(_) => Ok(patch_path),
    }
}

fn unmerged_swarm_paths(project: &Path, cancel_key: &str) -> Vec<String> {
    let listed = match git_result(&["ls-files", "-u", "-z"], project, Some(cancel_key)) {
        Ok(listed) if process_succeeded(&listed) && !listed.truncated => listed,
        _ => return Vec::new(),
    };
    let mut paths = listed.stdout.split_terminator('\0')
        .filter_map(|record| record.split_once('\t').map(|(_, path)| path.replace('\\', "/")))
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn merge_swarm_items_core(
    project: &Path,
    run_id: &str,
    mut items: Vec<SwarmMergeItem>,
    validate_command: &str,
    settings: &DispatchSettings,
) -> Result<Vec<SwarmMergeResult>, String> {
    items.sort_by_key(|item| item.item);
    let limit = settings.swarm_limit.max(1);
    let mut overflow = if items.len() > limit {
        skipped_swarm_results(&items.split_off(limit), "超过蜂群件数上限")
    } else {
        Vec::new()
    };
    let cancel_key = format!("swarm-merge:{run_id}");
    let log_dir = if settings.write_project_dispatch {
        project.join(".dispatch")
    } else {
        dispatch_data_root()?.join("runs").join(run_id).join("dispatch")
    };
    let timeout = Duration::from_secs(settings.timeout_secs.max(1));
    let _write_guard = match acquire_project_write(project, timeout, &cancel_key) {
        Ok(guard) => guard,
        Err(error) => {
            let mut results = skipped_swarm_results(&items, &error);
            results.append(&mut overflow);
            append_swarm_records(project, &log_dir, run_id, &mut results);
            return Ok(results);
        }
    };
    let dirty_paths = match project_dirty_paths(project, &cancel_key) {
        Ok(paths) => paths,
        Err(error) => {
            let mut results = skipped_swarm_results(&items, &error);
            results.append(&mut overflow);
            append_swarm_records(project, &log_dir, run_id, &mut results);
            return Ok(results);
        }
    };
    if !dirty_paths.is_empty() {
        let mut results = skipped_swarm_results(&items, &format!("项目目录有未提交改动：{}", summarize_paths(&dirty_paths)));
        results.append(&mut overflow);
        append_swarm_records(project, &log_dir, run_id, &mut results);
        return Ok(results);
    }

    let registered = match registered_worktrees(project, &cancel_key) {
        Ok(worktrees) => worktrees,
        Err(error) => {
            let mut results = skipped_swarm_results(&items, &error);
            results.append(&mut overflow);
            append_swarm_records(project, &log_dir, run_id, &mut results);
            return Ok(results);
        }
    };
    #[cfg(test)]
    let forced_head_failure = SWARM_TEST_FAIL_PROJECT_HEAD.with(|flag| flag.replace(false));
    #[cfg(not(test))]
    let forced_head_failure = false;
    let project_head = match if forced_head_failure {
        Err("测试注入的项目 HEAD 读取失败".into())
    } else {
        git_result(&["rev-parse", "HEAD"], project, Some(&cancel_key))
    } {
        Ok(head) if process_succeeded(&head) => head.stdout.trim().to_string(),
        Ok(head) => {
            let mut results = skipped_swarm_results(&items, &process_note(&head, "读取项目 HEAD 失败"));
            results.append(&mut overflow);
            append_swarm_records(project, &log_dir, run_id, &mut results);
            return Ok(results);
        }
        Err(error) => {
            let mut results = skipped_swarm_results(&items, &format!("读取项目 HEAD 失败：{error}"));
            results.append(&mut overflow);
            append_swarm_records(project, &log_dir, run_id, &mut results);
            return Ok(results);
        }
    };

    let mut results = Vec::new();
    let mut halt_after_item = false;
    let mut halt_note = String::new();
    for (index, item) in items.iter().enumerate() {
        let worktree = PathBuf::from(&item.worktree);
        let mut result = SwarmMergeResult {
            node_id: item.node_id.clone(),
            item: item.item,
            ..SwarmMergeResult::default()
        };
        'merge_item: {
        if !worktree.is_dir() {
            result.result = "skipped".into();
            result.note = format!("worktree 不存在：{}", worktree.display());
        } else if fs::canonicalize(&worktree).ok().is_none_or(|path| !registered.contains(&path)) {
            result.result = "skipped".into();
            result.note = format!("worktree 未登记在本项目：{}", worktree.display());
        } else if item.files.is_empty() {
            result.result = "skipped".into();
            result.note = "未声明文件范围".into();
        } else {
            let base = git_result(&["merge-base", &project_head, "HEAD"], &worktree, Some(&cancel_key));
            match base {
                Err(error) => {
                    result.result = "skipped".into();
                    result.note = format!("读取 worktree 合并基线失败：{error}");
                }
                Ok(base) if !process_succeeded(&base) => {
                    result.result = "skipped".into();
                    result.note = process_note(&base, "读取 worktree 合并基线失败");
                }
                Ok(base) => {
                    let base = base.stdout.trim().to_string();
                    match swarm_changed_paths(&worktree, &base, &cancel_key) {
                        Err(error) => {
                            result.result = "skipped".into();
                            result.note = error;
                        }
                        Ok(changed_paths) => {
                            for pattern in invalid_swarm_globs(&item.files) {
                                append_note(&mut result.note, &format!("glob 无效：{pattern}"));
                            }
                            let selected = changed_paths.iter()
                                .filter(|path| !excluded_swarm_path(path) && swarm_path_matches(path, &item.files))
                                .cloned()
                                .collect::<Vec<_>>();
                            let selected_set = selected.iter().collect::<HashSet<_>>();
                            let outside = changed_paths.iter()
                                .filter(|path| !selected_set.contains(path))
                                .cloned()
                                .collect::<Vec<_>>();
                            if !outside.is_empty() {
                                append_note(&mut result.note, &format!("范围外改动未并入：{}", summarize_paths(&outside)));
                            }
                            if selected.is_empty() {
                                result.result = "skipped".into();
                                if changed_paths.is_empty() {
                                    append_note(&mut result.note, "范围内没有改动");
                                } else {
                                    append_note(&mut result.note, "声明范围未匹配到改动");
                                }
                            } else {
                                let mut add_args = vec!["add", "-A", "--"];
                                add_args.extend(selected.iter().map(String::as_str));
                                let add = git_result(&add_args, &worktree, Some(&cancel_key));
                                match add {
                                    Err(error) => {
                                        result.result = "skipped".into();
                                        append_note(&mut result.note, &format!("暂存声明范围改动失败：{error}"));
                                    }
                                    Ok(add) if !process_succeeded(&add) => {
                                        result.result = "skipped".into();
                                        append_note(&mut result.note, &process_note(&add, "暂存声明范围改动失败"));
                                    }
                                    Ok(_) => {
                                        let audit = audit_staged_swarm_paths(&worktree, &item.files, &base, &cancel_key);
                                        match audit {
                                            Err(error) => {
                                                result.result = "skipped".into();
                                                append_note(&mut result.note, &error);
                                            }
                                            Ok((staged, rejected)) => {
                                                if !rejected.is_empty() {
                                                    append_note(&mut result.note, &format!("范围外改动未并入：{}", summarize_paths(&rejected)));
                                                }
                                                if staged.is_empty() {
                                                    result.result = "skipped".into();
                                                    append_note(&mut result.note, "范围内没有改动");
                                                    break 'merge_item;
                                                }
                                                let patch = match write_swarm_patch(&worktree, &base, &staged, &cancel_key) {
                                                    Ok(path) => path,
                                                    Err(error) => {
                                                        result.result = "skipped".into();
                                                        append_note(&mut result.note, &error);
                                                        break 'merge_item;
                                                    }
                                                };
                                                if fs::metadata(&patch).map(|metadata| metadata.len()).unwrap_or(0) == 0 {
                                                    let _ = fs::remove_file(&patch);
                                                    result.result = "skipped".into();
                                                    append_note(&mut result.note, "范围内没有改动");
                                                    break 'merge_item;
                                                }
                                                #[cfg(test)]
                                                if SWARM_TEST_DIRTY_BEFORE_ITEM.with(|target| {
                                                    let matches = target.get() == item.item;
                                                    if matches { target.set(0); }
                                                    matches
                                                }) {
                                                    let _ = fs::write(project.join("shared.txt"), "user edit during merge\n");
                                                }
                                                match project_dirty_paths(project, &cancel_key) {
                                                    Err(error) => {
                                                        let _ = fs::remove_file(&patch);
                                                        result.result = "skipped".into();
                                                        append_note(&mut result.note, &error);
                                                        halt_after_item = true;
                                                        halt_note = "项目状态无法确认，已停止后续合并".into();
                                                        break 'merge_item;
                                                    }
                                                    Ok(paths) => {
                                                        let outside_exclusions = paths.into_iter().filter(|path| !excluded_swarm_path(path)).collect::<Vec<_>>();
                                                        if !outside_exclusions.is_empty() {
                                                            let _ = fs::remove_file(&patch);
                                                            result.result = "skipped".into();
                                                            append_note(&mut result.note, &format!("项目目录在合并期间出现未提交改动：{}", summarize_paths(&outside_exclusions)));
                                                            halt_after_item = true;
                                                            halt_note = "项目目录在合并期间出现未提交改动，已停止后续合并".into();
                                                            break 'merge_item;
                                                        }
                                                    }
                                                }
                                                let baseline = git_result(&["rev-parse", "HEAD"], project, Some(&cancel_key)).ok()
                                                    .filter(process_succeeded).map(|value| value.stdout.trim().to_string()).unwrap_or_default();
                                                let patch_arg = patch.to_string_lossy().into_owned();
                                                let apply = git_result_with_timeout(&["apply", "--3way", "--index", &patch_arg], project, Some(&cancel_key), timeout);
                                                let _ = fs::remove_file(&patch);
                                                match apply {
                                                    Err(error) => {
                                                        result.result = "skipped".into();
                                                        append_note(&mut result.note, &format!("无法启动 git apply：{error}；worktree：{}", worktree.display()));
                                                    }
                                                    Ok(apply) if apply.timed_out => {
                                                        result.result = "skipped".into();
                                                        append_note(&mut result.note, &format!("应用声明范围补丁失败：命令超时；worktree：{}", worktree.display()));
                                                        if let Err(error) = restore_after_apply_conflict(project, &staged, &cancel_key, timeout) {
                                                            append_note(&mut result.note, &format!("{error}，项目状态不确定，请手动恢复工作区与索引"));
                                                            halt_after_item = true;
                                                            halt_note = "上一件恢复失败，项目状态不确定，已停止后续合并；请手动恢复工作区与索引".into();
                                                        }
                                                    }
                                                    Ok(apply) if !process_succeeded(&apply) => {
                                                        let conflicts = unmerged_swarm_paths(project, &cancel_key);
                                                        if conflicts.is_empty() {
                                                            result.result = "skipped".into();
                                                            append_note(&mut result.note, &format!("应用声明范围补丁失败：{}；worktree：{}", process_note(&apply, "git apply 未成功"), worktree.display()));
                                                        } else {
                                                            result.result = "conflict".into();
                                                            append_note(&mut result.note, &format!("范围内文件冲突：{}；worktree：{}", summarize_paths(&conflicts), worktree.display()));
                                                        }
                                                        if let Err(error) = restore_after_apply_conflict(project, &staged, &cancel_key, timeout) {
                                                            result.result = "skipped".into();
                                                            append_note(&mut result.note, &format!("{error}，项目状态不确定，请手动恢复工作区与索引"));
                                                            halt_after_item = true;
                                                            halt_note = "上一件恢复失败，项目状态不确定，已停止后续合并；请手动恢复工作区与索引".into();
                                                        }
                                                    }
                                                    Ok(_) => {
                                                        let quiet = git_result(&["diff", "--cached", "--quiet"], project, Some(&cancel_key));
                                                        if quiet.as_ref().is_ok_and(|quiet| quiet.code == Some(0) && !quiet.timed_out) {
                                                            result.result = "skipped".into();
                                                            append_note(&mut result.note, "范围内改动已在项目中");
                                                            break 'merge_item;
                                                        }
                                                        let title = if item.title.trim().is_empty() { &item.node_id } else { &item.title };
                                                        let message = format!("aicross swarm item {}: {title}", item.item);
                                                        let commit = git_result(&["commit", "-m", &message], project, Some(&cancel_key));
                                                        match commit {
                                                            Err(error) => {
                                                                result.result = "skipped".into();
                                                                append_note(&mut result.note, &format!("提交声明范围补丁失败：{error}"));
                                                                if let Err(reset_error) = reset_swarm_item(project, &baseline, &staged, &cancel_key, timeout) {
                                                                    append_note(&mut result.note, &format!("{reset_error}，项目状态不确定"));
                                                                    halt_after_item = true;
                                                                    halt_note = "上一件回退失败，项目状态不确定，已停止后续合并".into();
                                                                }
                                                            }
                                                            Ok(commit) if !process_succeeded(&commit) => {
                                                                result.result = "skipped".into();
                                                                append_note(&mut result.note, &process_note(&commit, "提交声明范围补丁失败"));
                                                                if let Err(reset_error) = reset_swarm_item(project, &baseline, &staged, &cancel_key, timeout) {
                                                                    append_note(&mut result.note, &format!("{reset_error}，项目状态不确定"));
                                                                    halt_after_item = true;
                                                                    halt_note = "上一件回退失败，项目状态不确定，已停止后续合并".into();
                                                                }
                                                            }
                                                            Ok(_) => {
                                                                let command = if item.accept.trim().is_empty() { validate_command } else { &item.accept };
                                                                let validation = run_validation(command, project, timeout);
                                                                let dirty_after_validation = restore_validation_exclusions(project, &cancel_key, timeout);
                                                                match (validation, dirty_after_validation) {
                                                                    (Ok(output), Ok(dirty)) if dirty.is_empty() => {
                                                                        result.result = "merged".into();
                                                                        result.test_output = truncate_utf8(&safe_trace(&output), 8 * 1024);
                                                                        result.commit = git_result(&["rev-parse", "HEAD"], project, Some(&cancel_key)).ok()
                                                                            .filter(process_succeeded).map(|value| value.stdout.trim().to_string()).unwrap_or_default();
                                                                    }
                                                                    (validation, dirty) => {
                                                                        result.result = "test_failed".into();
                                                                        if let Err(error) = validation {
                                                                            append_note(&mut result.note, &format!("验证命令未通过；worktree：{}", worktree.display()));
                                                                            result.test_output = truncate_utf8(&safe_trace(&error), 8 * 1024);
                                                                        }
                                                                        let cleanup_failed = dirty.is_err();
                                                                        let mut rollback_paths = staged.clone();
                                                                        if let Ok(paths) = &dirty {
                                                                            rollback_paths.extend(paths.iter().cloned());
                                                                            rollback_paths.sort();
                                                                            rollback_paths.dedup();
                                                                        }
                                                                        match dirty {
                                                                            Ok(paths) if !paths.is_empty() => append_note(&mut result.note, &format!("验证命令改动了项目文件：{}", paths.join(", "))),
                                                                            Err(error) => append_note(&mut result.note, &error),
                                                                            _ => {}
                                                                        }
                                                                        if let Err(reset_error) = reset_swarm_item(project, &baseline, &rollback_paths, &cancel_key, timeout) {
                                                                            result.result = "skipped".into();
                                                                            append_note(&mut result.note, &format!("{reset_error}，项目状态不确定"));
                                                                            halt_after_item = true;
                                                                            halt_note = "上一件回退失败，项目状态不确定，已停止后续合并".into();
                                                                        } else if cleanup_failed {
                                                                            result.result = "skipped".into();
                                                                            append_note(&mut result.note, "清理失败，已停止后续合并，请人工核对项目状态");
                                                                            halt_after_item = true;
                                                                            halt_note = "上一件清理失败，已停止后续合并，请人工核对项目状态".into();
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        }
        if let Err(error) = append_swarm_record(project, &log_dir, run_id, &result) {
            if result.note.is_empty() {
                result.note = format!("写入合并记录失败：{error}");
            } else {
                result.note.push_str(&format!("；写入合并记录失败：{error}"));
            }
        }
        results.push(result);
        if halt_after_item {
            let note = if halt_note.is_empty() {
                "上一件恢复失败，项目状态不确定，已停止后续合并；请手动恢复工作区与索引"
            } else {
                &halt_note
            };
            let mut remaining = skipped_swarm_results(
                &items[(index + 1)..],
                note,
            );
            append_swarm_records(project, &log_dir, run_id, &mut remaining);
            results.append(&mut remaining);
            break;
        }
    }
    for result in &mut overflow {
        if let Err(error) = append_swarm_record(project, &log_dir, run_id, result) {
            result.note.push_str(&format!("；写入合并记录失败：{error}"));
        }
    }
    results.append(&mut overflow);
    Ok(results)
}

#[tauri::command]
pub async fn merge_swarm_items(
    project_dir: String,
    run_id: String,
    items: Vec<SwarmMergeItem>,
    validate_command: String,
    settings: DispatchSettings,
) -> Result<Vec<SwarmMergeResult>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let project = project_path(&project_dir)?;
        merge_swarm_items_core(&project, &run_id, items, &validate_command, &settings)
    })
    .await
    .map_err(|error| format!("蜂群合并线程失败：{error}"))?
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
    let key = registry_key(&run_id, &node_id);
    mark_cancelled(&key);
    let _ = cancel_registered(&key);
    true
}

#[tauri::command]
pub fn inspect_dispatch_state(project_dir: String) -> Result<DispatchStateSummary, String> {
    let path = project_path(&project_dir)?
        .join(".dispatch")
        .join("STATE.md");
    if !path.is_file() {
        return Ok(DispatchStateSummary::default());
    }
    let content =
        fs::read_to_string(&path).map_err(|error| format!("读取派发状态失败：{error}"))?;
    let header = content
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("aicross-state: "))
        .and_then(|json| serde_json::from_str::<DispatchStateHeader>(json).ok())
        .unwrap_or_default();
    Ok(DispatchStateSummary {
        exists: true,
        unfinished: header.unfinished,
        content,
        run_id: header.run_id,
        active_node_ids: header.active_node_ids,
    })
}

fn parse_record_file(path: &Path) -> Result<(DispatchRecord, String, Option<DispatchArtifacts>), String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("读取派发记录失败 {}：{error}", path.display()))?;
    let rest = content.strip_prefix("---\n").ok_or_else(|| "派发记录缺少 frontmatter".to_string())?;
    let (yaml, body) = rest.split_once("\n---\n").ok_or_else(|| "派发记录 frontmatter 未结束".to_string())?;
    let record: DispatchRecord = serde_yaml::from_str(yaml)
        .map_err(|error| format!("解析派发记录 frontmatter 失败：{error}"))?;
    if record.schema != "aicross-dispatch/1" {
        return Err(format!("不支持的派发记录 schema：{}", record.schema));
    }
    let section = |heading: &str| {
        let marker = format!("## {heading}");
        let start = body.match_indices(&marker)
            .filter(|(index, _)| *index == 0 || body.as_bytes().get(index - 1) == Some(&b'\n'))
            .map(|(index, _)| index + marker.len())
            .last()?;
        let tail = body[start..].trim_start_matches(['\r', '\n']);
        let end = tail.match_indices("## ")
            .find(|(index, _)| *index == 0 || tail.as_bytes().get(index - 1) == Some(&b'\n'))
            .map_or(tail.len(), |(index, _)| index);
        Some(tail[..end].trim_end())
    };
    let reply_text = section("回复正文").unwrap_or_default().to_string();
    let artifacts = section("产物 JSON")
        .filter(|json| !json.is_empty() && *json != "null")
        .and_then(|json| serde_json::from_str(json).ok());
    Ok((record, reply_text, artifacts))
}

fn record_order_key(ended_at: &str, status: &str, path: &Path) -> (i64, bool, PathBuf) {
    let ended_at = DateTime::parse_from_rfc3339(ended_at)
        .map(|value| value.timestamp_millis())
        .unwrap_or_default();
    (ended_at, status == "ok", path.to_path_buf())
}

#[tauri::command]
pub fn reconcile_dispatch_state(project_dir: String) -> Result<ReconcileSummary, String> {
    let project = project_path(&project_dir)?;
    let dispatch = project.join(".dispatch");
    let state = inspect_dispatch_state(project_dir)?;
    if !state.exists {
        return Ok(ReconcileSummary::default());
    }
    let mut nodes = Vec::new();
    for node_id in state.active_node_ids {
        let mut matched = None;
        let mut matched_key = None;
        if dispatch.is_dir() {
            let mut paths = fs::read_dir(&dispatch)
                .map_err(|error| format!("读取派发记录目录失败：{error}"))?
                .map(|entry| entry.map(|value| value.path()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("读取派发记录目录项失败：{error}"))?;
            paths.sort();
            for path in paths {
                if path.file_name().and_then(|name| name.to_str()) == Some("STATE.md")
                    || path.extension().and_then(|value| value.to_str()) != Some("md")
                {
                    continue;
                }
                if let Ok((record, reply_text, artifacts)) = parse_record_file(&path) {
                    if record.run_id == state.run_id && record.node == node_id && record.merge_result.is_empty() {
                        let key = record_order_key(&record.ended_at, &record.status, &path);
                        if matched_key.as_ref().is_none_or(|current| &key > current) {
                            matched_key = Some(key);
                            matched = Some((record, reply_text, artifacts, path));
                        }
                    }
                }
            }
        }
        nodes.push(match matched {
            Some((record, reply_text, artifacts, path)) => ReconcileNode {
                node_id,
                verdict: if record.status == "ok" { "completed" } else { "incomplete" }.into(),
                record_path: path.to_string_lossy().into_owned(),
                reply_text,
                artifacts,
                error: record.error,
                session_id: record.session_id,
                runner: record.runner,
                usage: ReconcileUsage { input: record.tokens_in, output: record.tokens_out, cache_read: record.cache_read },
            },
            None => ReconcileNode { node_id, verdict: "unknown".into(), ..ReconcileNode::default() },
        });
    }
    Ok(ReconcileSummary { exists: true, run_id: state.run_id, nodes })
}

#[tauri::command]
pub fn rollback_node_changes(project_dir: String, run_id: String, node_id: String) -> Result<RollbackSummary, String> {
    let project = project_path(&project_dir)?;
    let _guard = acquire_project_write(&project, Duration::from_secs(30), &format!("rollback:{run_id}:{node_id}"))?;
    rollback_node_changes_core(&project, &run_id, &node_id)
}

#[tauri::command]
pub fn abandon_dispatch_state(project_dir: String) -> Result<(), String> {
    let path = project_path(&project_dir)?
        .join(".dispatch")
        .join("STATE.md");
    if path.is_file() {
        let stamp = Local::now().format("%Y%m%d-%H%M%S");
        let abandoned = path.with_file_name(format!("STATE.abandoned-{stamp}.md"));
        fs::rename(&path, &abandoned).map_err(|error| format!("放弃派发状态失败：{error}"))?;
    }
    Ok(())
}

fn is_continuing(node: &DispatchNodeRequest) -> bool {
    !node.session_id.is_empty() && node.visibility == "共享"
}

fn requires_single_writer(node: &DispatchNodeRequest, worktree_falls_back: bool) -> bool {
    node.visibility != "盲"
        && (node.write == "单写者" || (node.write == "worktree" && worktree_falls_back))
}

// 节点运行时占用的 agent 集合：主 agent + 有效备选 agent。同一 agent 同时最多
// 1 路的串行约束对备选尝试同样成立，调度按整个集合记账。
fn node_agents(node: &DispatchNodeRequest) -> Vec<String> {
    let mut agents = vec![channels::normalize_agent(&node.agent).to_string()];
    if let Some(fallback) = node
        .fallback
        .as_ref()
        .filter(|f| !f.agent.trim().is_empty() && !f.model.trim().is_empty())
    {
        agents.push(channels::normalize_agent(&fallback.agent).to_string());
    }
    agents
}

fn preview_core(request: &DispatchStageRequest) -> Result<DispatchPreview, String> {
    let _ = project_path(&request.project_dir)?;
    if request
        .nodes
        .iter()
        .any(|node| channels::normalize_agent(&node.agent) == "cc-switch")
    {
        let path = request.settings.cc_switch_path.trim();
        if path.is_empty() {
            return Err("未配置 cc_switch.py 路径".into());
        }
        if !Path::new(path).is_file() {
            return Err(format!("cc_switch.py 路径不存在：{path}"));
        }
    }
    let mut preview = DispatchPreview::default();
    for node in &request.nodes {
        let prompt = if is_continuing(node) && !node.current_message.is_empty() {
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
        preview.broad_redacted += broad_redaction_count(prompt);
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
    if let Some(node) = request.nodes.iter().find(|node| node.agent.trim().is_empty() || node.model.trim().is_empty()) {
        return Ok(DispatchStageResult {
            run_id: request.run_id,
            results: vec![DispatchNodeResult {
                node_id: node.node_id.clone(),
                stage: node.stage,
                status: "error".into(),
                error: format!("节点 {} 未指定 agent/model，不能派发", node.node_id),
                ..DispatchNodeResult::default()
            }],
            state_file: String::new(),
        });
    }
    let mut rejected = Vec::new();
    request.nodes.retain(|node| {
        // 与前端 validateNode 同口径：验收/验证/仲裁角色只读（R2-7 补仲裁）。
        let readonly_role = matches!(node.role.as_str(), "验收" | "验证" | "仲裁" | "拆分");
        let invalid = readonly_role && node.write != "只读";
        if invalid {
            rejected.push(DispatchNodeResult {
                node_id: node.node_id.clone(),
                stage: node.stage,
                status: "error".into(),
                error: format!("节点 {} 的「{}」角色写权限必须为只读", node.node_id, node.role),
                ..DispatchNodeResult::default()
            });
        }
        !invalid
    });
    if request.nodes.is_empty() {
        return Ok(DispatchStageResult {
            run_id: request.run_id,
            results: rejected,
            state_file: String::new(),
        });
    }
    if request.nodes.len() > request.settings.max_requests.clamp(1, 100) {
        return Err(format!(
            "本次派发 {} 路，超过总请求上限 {}",
            request.nodes.len(),
            request.settings.max_requests.clamp(1, 100)
        ));
    }
    if request.run_id.is_empty() {
        request.run_id = Uuid::new_v4().to_string();
    }
    let _active_run = {
        // 进程内按 run_id 幂等互斥；RAII guard 在成功、失败和提前返回时都会释放。
        let mut active = active_runs().lock().map_err(|_| "派发运行状态不可用".to_string())?;
        if !active.insert(request.run_id.clone()) {
            return Err(format!("该 run 已在进行：{}", request.run_id));
        }
        ActiveRunGuard(request.run_id.clone())
    };
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
    let timeout = Duration::from_secs(request.settings.timeout_secs.clamp(1, 3600));
    let worktree_falls_back = request.nodes.iter().any(|node| node.write == "worktree")
        && !project_is_git_work_tree(&project, timeout).unwrap_or(false);
    let state_stage = request
        .nodes
        .iter()
        .map(|node| node.stage)
        .min()
        .unwrap_or(request.stage);
    write_state(
        &state_file,
        &request.run_id,
        state_stage,
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
    let mut results = rejected;
    let (sender, receiver) = mpsc::channel::<(Vec<String>, bool, DispatchNodeResult)>();
    let mut active_single_writer = false;

    while !queued.is_empty() || active > 0 {
        while active < limit {
            let next_index = queued.iter().position(|node| {
                let requires_single_writer = requires_single_writer(node, worktree_falls_back);
                node_agents(node)
                    .iter()
                    .all(|agent| !active_agents.contains(agent))
                    && !(requires_single_writer && active_single_writer)
            });
            if active_single_writer {
                for node in queued.iter().filter(|node| {
                    requires_single_writer(node, worktree_falls_back)
                }) {
                    if status.get(&node.node_id).map(String::as_str) != Some("等待写锁") {
                        status.insert(node.node_id.clone(), "等待写锁".into());
                        emit_progress(&app, &request.run_id, &node.node_id, "等待写锁");
                    }
                }
            }
            let Some(index) = next_index else { break };
            let node = queued.remove(index).expect("queued index");
            let single_writer = requires_single_writer(&node, worktree_falls_back);
            if single_writer {
                active_single_writer = true;
            }
            let agents = node_agents(&node);
            for agent in &agents {
                active_agents.insert(agent.clone());
            }
            active += 1;
            status.insert(node.node_id.clone(), "运行中".into());
            emit_progress(&app, &request.run_id, &node.node_id, "运行中");
            write_state(
                &state_file,
                &request.run_id,
                state_stage,
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
            let app = app.clone();
            thread::spawn(move || {
                let node_id = node.node_id.clone();
                let activity = ProcessActivity::new();
                let (stop_tx, stop_rx) = mpsc::channel();
                let heartbeat_app = app.clone();
                let heartbeat_run = run_id.clone();
                let heartbeat_node = node_id.clone();
                let heartbeat_activity = activity.clone();
                let heartbeat = thread::spawn(move || heartbeat_loop(stop_rx, Duration::from_secs(15), || {
                    emit_running_progress(&heartbeat_app, &heartbeat_run, &heartbeat_node, &heartbeat_activity);
                }));
                let result = run_one_with_activity(&run_id, &project, &run_root, &log_dir, &settings, node, activity);
                let _ = stop_tx.send(());
                let _ = heartbeat.join();
                let _ = tx.send((agents, single_writer, result));
            });
        }
        if active == 0 {
            return Err("同一 agent 并发调度未能推进".into());
        }
        let (agents, single_writer, result) = receiver
            .recv()
            .map_err(|error| format!("接收派发结果失败：{error}"))?;
        active -= 1;
        for agent in &agents {
            active_agents.remove(agent);
        }
        if single_writer {
            active_single_writer = false;
        }
        let ui_status = match result.status.as_str() {
            "ok" => "完成",
            "timeout" => "超时",
            "validation_failed" => "验证命令未通过",
            "identity_mismatch" => "真身不符",
            "quota_exhausted" => "额度用尽",
            _ => "失败",
        };
        status.insert(result.node_id.clone(), ui_status.into());
        emit_progress(&app, &request.run_id, &result.node_id, ui_status);
        if result.resources_changed {
            if let Some(app) = &app {
                let _ = app.emit("aicross:resources-changed", ());
            }
        }
        results.push(result);
        write_state(
            &state_file,
            &request.run_id,
            state_stage,
            limit,
            &status,
            &results,
        )?;
    }

    clear_review_validation_cache(&request.run_id, &project);

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
    #[cfg(test)]
    return Ok(std::env::temp_dir().join(format!("aicross-test-runtime-{}", std::process::id())));
    #[cfg(not(test))]
    aicross_root()
}

const SNAPSHOT_FILE_LIMIT: u64 = 8 * 1024 * 1024;
const SNAPSHOT_TOTAL_LIMIT: u64 = 128 * 1024 * 1024;
#[cfg(test)]
const ROLLBACK_TEST_TOTAL_LIMIT: u64 = 512;
const FALLBACK_FILE_DIFF_LIMIT: usize = 200 * 1024;

#[derive(Clone, Debug, Default)]
struct SnapshotFile {
    bytes: Option<Vec<u8>>,
    size: u64,
    modified_ns: u128,
}

type DirectorySnapshot = HashMap<String, SnapshotFile>;

#[derive(Default)]
struct ProjectLockState {
    held: bool,
    queue: VecDeque<String>,
}

#[derive(Default)]
struct ProjectLock {
    state: Mutex<ProjectLockState>,
    changed: Condvar,
}

fn project_write_locks() -> &'static Mutex<HashMap<PathBuf, Arc<ProjectLock>>> {
    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<ProjectLock>>>> = OnceLock::new();
    LOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cancelled_dispatches() -> &'static Mutex<HashSet<String>> {
    static CANCELLED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    CANCELLED.get_or_init(|| Mutex::new(HashSet::new()))
}

fn active_runs() -> &'static Mutex<HashSet<String>> {
    static ACTIVE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    ACTIVE.get_or_init(|| Mutex::new(HashSet::new()))
}

struct ActiveRunGuard(String);

impl Drop for ActiveRunGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = active_runs().lock() {
            active.remove(&self.0);
        }
    }
}

fn mark_cancelled(key: &str) {
    if let Ok(mut cancelled) = cancelled_dispatches().lock() {
        cancelled.insert(key.to_string());
    }
    if let Ok(locks) = project_write_locks().lock() {
        for lock in locks.values() {
            lock.changed.notify_all();
        }
    }
}

pub(crate) fn is_cancelled(key: &str) -> bool {
    cancelled_dispatches()
        .lock()
        .is_ok_and(|cancelled| cancelled.contains(key))
}

pub(crate) fn clear_cancelled(key: &str) {
    if let Ok(mut cancelled) = cancelled_dispatches().lock() {
        cancelled.remove(key);
    }
}

struct ProjectWriteGuard {
    lock: Arc<ProjectLock>,
}

impl Drop for ProjectWriteGuard {
    fn drop(&mut self) {
        if let Ok(mut state) = self.lock.state.lock() {
            state.held = false;
            self.lock.changed.notify_all();
        }
    }
}

fn acquire_project_write(
    project: &Path,
    timeout: Duration,
    cancel_key: &str,
) -> Result<ProjectWriteGuard, String> {
    let project = project
        .canonicalize()
        .unwrap_or_else(|_| project.to_path_buf());
    let lock = {
        let mut locks = project_write_locks()
            .lock()
            .map_err(|_| "写锁状态不可用".to_string())?;
        locks.entry(project).or_default().clone()
    };
    let waiter = Uuid::new_v4().to_string();
    let deadline = Instant::now() + timeout;
    let mut state = lock
        .state
        .lock()
        .map_err(|_| "写锁状态不可用".to_string())?;
    state.queue.push_back(waiter.clone());
    loop {
        if is_cancelled(cancel_key) {
            state.queue.retain(|entry| entry != &waiter);
            lock.changed.notify_all();
            return Err("等待写锁已取消".into());
        }
        if !state.held && state.queue.front() == Some(&waiter) {
            state.queue.pop_front();
            state.held = true;
            drop(state);
            return Ok(ProjectWriteGuard { lock });
        }
        let now = Instant::now();
        if now >= deadline {
            state.queue.retain(|entry| entry != &waiter);
            lock.changed.notify_all();
            return Err(format!("等待写锁超过 {} 秒", timeout.as_secs()));
        }
        let remaining = deadline.saturating_duration_since(now);
        let wait = remaining.min(Duration::from_millis(100));
        let (next, _) = lock
            .changed
            .wait_timeout(state, wait)
            .map_err(|_| "写锁状态不可用".to_string())?;
        state = next;
    }
}

#[cfg(windows)]
fn is_link_or_junction(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_link_or_junction(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn snapshot_directory(root: &Path) -> Result<DirectorySnapshot, String> {
    snapshot_directory_with_limits(root, SNAPSHOT_FILE_LIMIT, SNAPSHOT_TOTAL_LIMIT)
}

fn snapshot_directory_with_limits(
    root: &Path,
    file_limit: u64,
    total_limit: u64,
) -> Result<DirectorySnapshot, String> {
    fn walk(
        root: &Path,
        current: &Path,
        out: &mut DirectorySnapshot,
        file_limit: u64,
        total_limit: u64,
        total_read: &mut u64,
    ) -> Result<(), String> {
        for entry in fs::read_dir(current)
            .map_err(|error| format!("读取目录快照失败 {}：{error}", current.display()))?
        {
            let entry = entry.map_err(|error| format!("读取目录项失败：{error}"))?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("读取快照元数据失败 {}：{error}", path.display()))?;
            if is_link_or_junction(&metadata) {
                continue;
            }
            if metadata.is_dir() {
                if excluded_snapshot_directory(&name) {
                    continue;
                }
                walk(root, &path, out, file_limit, total_limit, total_read)?;
            } else if metadata.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let bytes = if metadata.len() > file_limit {
                    None
                } else {
                    if total_read.saturating_add(metadata.len()) > total_limit {
                        return Err(format!(
                            "目录快照超过总字节上限 {} MB",
                            total_limit / (1024 * 1024)
                        ));
                    }
                    *total_read += metadata.len();
                    Some(fs::read(&path).map_err(|error| {
                        format!("读取快照文件失败 {}：{error}", path.display())
                    })?)
                };
                let modified_ns = metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|duration| duration.as_nanos())
                    .unwrap_or_default();
                out.insert(relative, SnapshotFile { bytes, size: metadata.len(), modified_ns });
            }
        }
        Ok(())
    }

    let mut snapshot = HashMap::new();
    let mut total_read = 0;
    walk(
        root,
        root,
        &mut snapshot,
        file_limit,
        total_limit,
        &mut total_read,
    )?;
    Ok(snapshot)
}

fn snapshot_file_unchanged(old_file: Option<&SnapshotFile>, new_file: Option<&SnapshotFile>) -> bool {
    match (old_file, new_file) {
        (Some(old), Some(new)) => match (&old.bytes, &new.bytes) {
            (Some(old), Some(new)) => old == new,
            (None, None) => old.size == new.size && old.modified_ns == new.modified_ns,
            _ => false,
        },
        (None, None) => true,
        _ => false,
    }
}

fn changed_snapshot_paths(before: &DirectorySnapshot, after: &DirectorySnapshot) -> Vec<String> {
    let mut paths: Vec<String> = before.keys().chain(after.keys()).cloned().collect();
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .filter(|path| !snapshot_file_unchanged(before.get(path), after.get(path)))
        .collect()
}

fn safe_relative_path(value: &str) -> Option<&Path> {
    let path = Path::new(value);
    (!path.is_absolute()
        && !path.components().any(|part| matches!(part, std::path::Component::Prefix(_) | std::path::Component::RootDir | std::path::Component::ParentDir | std::path::Component::CurDir)))
        .then_some(path)
}

#[derive(Debug, PartialEq, Eq)]
struct PersistedBeforeSnapshot {
    path: String,
    note: String,
}

fn finish_before_snapshot<F>(previous: &Path, had_previous: bool, remove: F) -> String
where
    F: FnOnce(&Path) -> std::io::Result<()>,
{
    if !had_previous {
        return String::new();
    }
    remove(previous)
        .err()
        .map(|error| format!("清理 before.prev 快照失败：{error}"))
        .unwrap_or_default()
}

fn persist_before_snapshot_with_note(
    project: &Path,
    log_dir: &Path,
    run_id: &str,
    node_id: &str,
    before: &DirectorySnapshot,
    after: &DirectorySnapshot,
) -> Result<PersistedBeforeSnapshot, String> {
    let changed = changed_snapshot_paths(before, after);
    if changed.is_empty() {
        return Ok(PersistedBeforeSnapshot { path: String::new(), note: String::new() });
    }
    let project_dispatch = project.join(".dispatch");
    let root = if log_dir == project_dispatch {
        log_dir.join(safe_name(run_id)).join(format!("{}.before", safe_name(node_id)))
    } else {
        log_dir.join(format!("{}.before", safe_name(node_id)))
    };
    let previous = PathBuf::from(format!("{}.prev", root.to_string_lossy()));
    if root.exists() && !root.is_dir() {
        return Err("旧 before 快照不是目录".into());
    }
    if previous.exists() {
        fs::remove_dir_all(&previous).map_err(|error| format!("清理旧 before.prev 快照失败：{error}"))?;
    }
    let had_previous = root.exists();
    if had_previous {
        fs::rename(&root, &previous).map_err(|error| format!("保留旧 before 快照失败：{error}"))?;
    }
    let written = (|| {
        fs::create_dir_all(&root).map_err(|error| format!("创建 before 快照失败：{error}"))?;
        let mut created = Vec::new();
        let mut skipped = Vec::new();
        for path in changed {
            if before.get(&path).is_none() {
                created.push(path);
                continue;
            }
            let Some(bytes) = before.get(&path).and_then(|file| file.bytes.as_deref()) else {
                skipped.push(path);
                continue;
            };
            let relative = safe_relative_path(&path).ok_or_else(|| format!("快照路径不安全：{path}"))?;
            let target = root.join(relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| format!("创建 before 子目录失败：{error}"))?;
            }
            fs::write(&target, bytes).map_err(|error| format!("写入 before 快照失败 {}：{error}", target.display()))?;
        }
        created.sort();
        fs::write(root.join(".created"), created.join("\n"))
            .map_err(|error| format!("写入新建文件清单失败：{error}"))?;
        skipped.sort();
        fs::write(root.join(".skipped"), skipped.join("\n"))
            .map_err(|error| format!("写入跳过文件清单失败：{error}"))
    })();
    if let Err(error) = written {
        let _ = fs::remove_dir_all(&root);
        if had_previous {
            fs::rename(&previous, &root).map_err(|restore| format!("{error}；恢复旧 before 快照失败：{restore}"))?;
        }
        return Err(error);
    }
    let note = finish_before_snapshot(&previous, had_previous, |path| fs::remove_dir_all(path));
    let path = if root.starts_with(project) {
        root.strip_prefix(project).unwrap_or(&root).to_string_lossy().replace('\\', "/")
    } else {
        root.to_string_lossy().into_owned()
    };
    Ok(PersistedBeforeSnapshot { path, note })
}

#[cfg(test)]
fn persist_before_snapshot(
    project: &Path,
    log_dir: &Path,
    run_id: &str,
    node_id: &str,
    before: &DirectorySnapshot,
    after: &DirectorySnapshot,
) -> Result<String, String> {
    persist_before_snapshot_with_note(project, log_dir, run_id, node_id, before, after)
        .map(|saved| saved.path)
}

fn latest_before_dir(project: &Path, node_id: &str) -> Option<PathBuf> {
    let mut matched: Option<(i64, bool, PathBuf, PathBuf)> = None;
    let mut paths = fs::read_dir(project.join(".dispatch")).into_iter().flatten().flatten()
        .map(|entry| entry.path()).collect::<Vec<_>>();
    if let Ok(root) = dispatch_data_root() {
        if let Ok(runs) = fs::read_dir(root.join("runs")) {
            for run in runs.flatten() {
                if let Ok(records) = fs::read_dir(run.path().join("dispatch")) {
                    paths.extend(records.flatten().map(|entry| entry.path()));
                }
            }
        }
    }
    let project_canonical = project.canonicalize().unwrap_or_else(|_| project.to_path_buf());
    for path in paths {
        if path.extension().and_then(|value| value.to_str()) != Some("md") { continue; }
        let Ok((record, _, _)) = parse_record_file(&path) else { continue };
        if record.node != node_id || record.before_dir.trim().is_empty() { continue; }
        if Path::new(&record.workdir).canonicalize().ok().as_ref() != Some(&project_canonical) { continue; }
        let before = PathBuf::from(&record.before_dir);
        let before = if before.is_absolute() { before } else { project.join(before) };
        let (ended, ok, record_path) = record_order_key(&record.ended_at, &record.status, &path);
        let key = (ended, ok, record_path, before);
        if matched.as_ref().is_none_or(|current| (&key.0, &key.1, &key.2) > (&current.0, &current.1, &current.2)) {
            matched = Some(key);
        }
    }
    matched.map(|(_, _, _, before)| before).filter(|path| path.is_dir())
}

fn rollback_node_changes_core(project: &Path, run_id: &str, node_id: &str) -> Result<RollbackSummary, String> {
    let fallback = project.join(".dispatch").join(safe_name(run_id)).join(format!("{}.before", safe_name(node_id)));
    let root = latest_before_dir(project, node_id).unwrap_or(fallback);
    if !root.is_dir() {
        return Ok(RollbackSummary { note: "没有找到派发前快照（首次派发或快照已清理）；排除目录（.git/node_modules/target/dist…）与链接内的改动不在快照内".into(), ..RollbackSummary::default() });
    }
    restore_before_dir(project, &root)
}

// 按指定的快照目录恢复（重派回滚用历史最新快照；额度回滚只用本次尝试的快照）。
fn restore_before_dir(project: &Path, root: &Path) -> Result<RollbackSummary, String> {
    let root = root.to_path_buf();
    let mut summary = RollbackSummary::default();
    summary.skipped = fs::read_to_string(root.join(".skipped"))
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect();
    fn collect(root: &Path, current: &Path, out: &mut Vec<(String, Vec<u8>)>, skipped: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(current) else { skipped.push(current.to_string_lossy().into_owned()); return };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(metadata) = fs::symlink_metadata(&path) else { skipped.push(path.to_string_lossy().into_owned()); continue };
            let relative = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            if matches!(relative.as_str(), ".created" | ".skipped") { continue; }
            if is_link_or_junction(&metadata) { skipped.push(relative); }
            else if metadata.is_dir() { collect(root, &path, out, skipped); }
            else if metadata.is_file() {
                match fs::read(&path) { Ok(bytes) => out.push((relative, bytes)), Err(_) => skipped.push(relative) }
            }
        }
    }
    let mut restore_plan = Vec::new();
    collect(&root, &root, &mut restore_plan, &mut summary.skipped);
    restore_plan.sort_by(|a, b| a.0.cmp(&b.0));
    let project_canonical = project.canonicalize().unwrap_or_else(|_| project.to_path_buf());
    for (path, bytes) in restore_plan {
        let Some(relative) = safe_relative_path(&path) else { summary.skipped.push(path); continue };
        let target = project.join(relative);
        if let Ok(metadata) = fs::symlink_metadata(&target) {
            if is_link_or_junction(&metadata) {
                let removed = if metadata.is_dir() { fs::remove_dir(&target) } else { fs::remove_file(&target) };
                if removed.is_err() { summary.skipped.push(path); continue; }
            }
        }
        if let Some(parent) = target.parent() {
            if fs::create_dir_all(parent).is_err()
                || parent.canonicalize().map_or(true, |value| !value.starts_with(&project_canonical)) {
                summary.skipped.push(path); continue;
            }
        }
        if fs::write(&target, bytes).is_ok() { summary.restored.push(path); } else { summary.skipped.push(path); }
    }
    let created = fs::read_to_string(root.join(".created")).unwrap_or_default();
    for path in created.lines().filter(|line| !line.trim().is_empty()) {
        let Some(relative) = safe_relative_path(path) else { summary.skipped.push(path.into()); continue };
        let target = project.join(relative);
        let Ok(metadata) = fs::symlink_metadata(&target) else { continue };
        let removed = if metadata.is_dir() { fs::remove_dir(&target) } else { fs::remove_file(&target) };
        if removed.is_ok() { summary.removed.push(path.into()); } else { summary.skipped.push(path.into()); }
    }
    summary.restored.sort();
    summary.removed.sort();
    summary.skipped.sort();
    summary.skipped.dedup();
    summary.note = format!("已回滚上次改动 {} 文件，跳过 {} 项；排除目录（.git/node_modules/target/dist…）与链接内的改动不在快照内", summary.restored.len() + summary.removed.len(), summary.skipped.len());
    Ok(summary)
}

fn changed_file_contents(after: &DirectorySnapshot, paths: &[String]) -> Vec<ChangedFile> {
    paths
        .iter()
        .filter_map(|path| {
            let bytes = after.get(path)?.bytes.as_deref()?;
            if bytes.len() > 200 * 1024 {
                return None;
            }
            let content = std::str::from_utf8(bytes).ok()?.to_string();
            Some(ChangedFile {
                path: path.clone(),
                content,
            })
        })
        .collect()
}

fn fallback_diff_for_paths(
    before: &DirectorySnapshot,
    after: &DirectorySnapshot,
    paths: &[String],
) -> String {
    let mut diff = String::new();
    for path in paths {
        let old_file = before.get(path);
        let new_file = after.get(path);
        let old = old_file.and_then(|file| file.bytes.as_deref());
        let new = new_file.and_then(|file| file.bytes.as_deref());
        let old_text = old.and_then(|bytes| std::str::from_utf8(bytes).ok());
        let new_text = new.and_then(|bytes| std::str::from_utf8(bytes).ok());
        let old_label = if old_file.is_some() {
            format!("a/{path}")
        } else {
            "/dev/null".into()
        };
        let new_label = if new_file.is_some() {
            format!("b/{path}")
        } else {
            "/dev/null".into()
        };
        let mut file_diff = format!("--- {old_label}\n+++ {new_label}\n");
        if (old_file.is_some() && old_text.is_none()) || (new_file.is_some() && new_text.is_none()) {
            file_diff.push_str(&format!("Binary files {old_label} and {new_label} differ\n"));
            diff.push_str(&file_diff);
            continue;
        }
        let old_text = old_text.unwrap_or("");
        let new_text = new_text.unwrap_or("");
        let old_lines = if old_text.is_empty() {
            0
        } else {
            old_text.lines().count()
        };
        let new_lines = if new_text.is_empty() {
            0
        } else {
            new_text.lines().count()
        };
        let old_start = usize::from(old_file.is_some());
        let new_start = usize::from(new_file.is_some());
        file_diff.push_str(&format!(
            "@@ -{old_start},{old_lines} +{new_start},{new_lines} @@\n"
        ));
        for line in old_text.lines() {
            file_diff.push_str(&format!("-{line}\n"));
        }
        for line in new_text.lines() {
            file_diff.push_str(&format!("+{line}\n"));
        }
        if file_diff.len() > FALLBACK_FILE_DIFF_LIMIT {
            diff.push_str(&format!("文件：{path}\n全文见 changed/\n"));
        } else {
            diff.push_str(&file_diff);
        }
    }
    diff
}

fn artifact_diff(before: &DirectorySnapshot, after: &DirectorySnapshot) -> DispatchArtifacts {
    let paths = changed_snapshot_paths(before, after);
    DispatchArtifacts {
        diff: fallback_diff_for_paths(before, after, &paths),
        changed_files: changed_file_contents(after, &paths),
        ..DispatchArtifacts::default()
    }
}

fn materialize_snapshot(
    root: &Path,
    snapshot: &DirectorySnapshot,
    paths: &[String],
) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|error| format!("创建 diff 临时目录失败：{error}"))?;
    for path in paths {
        let Some(bytes) = snapshot.get(path).and_then(|file| file.bytes.as_deref()) else {
            continue;
        };
        let target = root.join(Path::new(path));
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("创建 diff 临时子目录失败：{error}"))?;
        }
        fs::write(&target, bytes)
            .map_err(|error| format!("写入 diff 临时文件失败 {}：{error}", target.display()))?;
    }
    Ok(())
}

fn project_is_git_work_tree_with(program: &Path, project: &Path, timeout: Duration) -> Result<bool, String> {
    let probe = run_process_registered(
        program,
        &["rev-parse".into(), "--is-inside-work-tree".into()],
        None,
        timeout,
        Some(project),
        None,
    )?;
    Ok(!probe.timed_out && probe.code == Some(0) && probe.stdout.trim() == "true")
}

fn project_is_git_work_tree(project: &Path, timeout: Duration) -> Result<bool, String> {
    project_is_git_work_tree_with(Path::new("git"), project, timeout)
}

fn artifact_diff_for_project(
    project: &Path,
    scratch_parent: &Path,
    before: &DirectorySnapshot,
    after: &DirectorySnapshot,
    timeout: Duration,
) -> Result<DispatchArtifacts, String> {
    let is_git = project_is_git_work_tree(project, timeout).unwrap_or(false);
    artifact_diff_for_project_with_git(Path::new("git"), is_git, scratch_parent, before, after, timeout)
}

fn artifact_diff_for_project_with_git(
    git: &Path,
    is_git: bool,
    scratch_parent: &Path,
    before: &DirectorySnapshot,
    after: &DirectorySnapshot,
    timeout: Duration,
) -> Result<DispatchArtifacts, String> {
    let paths = changed_snapshot_paths(before, after);
    if paths.is_empty() {
        return Ok(DispatchArtifacts::default());
    }
    if !is_git {
        return Ok(artifact_diff(before, after));
    }

    let materialized = paths
        .iter()
        .filter(|path| {
            before.get(*path).is_none_or(|file| file.bytes.is_some())
                && after.get(*path).is_none_or(|file| file.bytes.is_some())
        })
        .cloned()
        .collect::<Vec<_>>();
    let oversized = paths
        .iter()
        .filter(|path| !materialized.contains(path))
        .cloned()
        .collect::<Vec<_>>();
    let scratch = scratch_parent.join(format!("diff-{}", Uuid::new_v4()));
    let before_root = scratch.join("before");
    let after_root = scratch.join("after");
    if let Err(error) = materialize_snapshot(&before_root, before, &materialized)
        .and_then(|_| materialize_snapshot(&after_root, after, &materialized))
    {
        let _ = fs::remove_dir_all(&scratch);
        return Err(error);
    }
    let result = run_process_registered(
        git,
        &[
            "diff".into(),
            "--no-index".into(),
            "--binary".into(),
            "--unified=3".into(),
            "--no-ext-diff".into(),
            "--src-prefix=a/".into(),
            "--dst-prefix=b/".into(),
            "before".into(),
            "after".into(),
        ],
        None,
        timeout,
        Some(&scratch),
        None,
    );
    let _ = fs::remove_dir_all(&scratch);
    let result = match result {
        Ok(result) => result,
        Err(_) => return Ok(artifact_diff(before, after)),
    };
    if result.timed_out || !matches!(result.code, Some(0 | 1)) {
        return Ok(artifact_diff(before, after));
    }
    let mut diff = normalize_git_diff(&result.stdout);
    if !diff.is_empty() { diff.push('\n'); }
    diff.push_str(&fallback_diff_for_paths(before, after, &oversized));
    Ok(DispatchArtifacts {
        diff,
        changed_files: changed_file_contents(after, &paths),
        ..DispatchArtifacts::default()
    })
}

fn normalize_git_diff(raw: &str) -> String {
    raw.lines().map(|line| {
        if line.starts_with("diff --git ") || line.starts_with("--- ") || line.starts_with("+++ ") {
            line.replace("a/before/", "a/")
                .replace("b/after/", "b/")
                .replace("\"before/", "\"a/")
                .replace("\"after/", "\"b/")
        } else { line.to_string() }
    }).collect::<Vec<_>>().join("\n")
}

fn create_worktree(project: &Path, path: &Path, timeout: Duration) -> Result<(), String> {
    if !project_is_git_work_tree(project, timeout)? {
        return Err("项目不是 git 仓库".into());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建 worktree 目录失败：{error}"))?;
    }
    let result = run_process_registered(
        Path::new("git"),
        &[
            "worktree".into(),
            "add".into(),
            "--detach".into(),
            path.to_string_lossy().into_owned(),
            "HEAD".into(),
        ],
        None,
        timeout,
        Some(project),
        None,
    )?;
    if result.timed_out || result.code != Some(0) {
        return Err(nonempty_process_error(&result, "创建 worktree 失败"));
    }
    Ok(())
}

fn run_validation(command: &str, cwd: &Path, timeout: Duration) -> Result<String, String> {
    if command.trim().is_empty() {
        return Ok(String::new());
    }
    #[cfg(windows)]
    let (program, args) = (
        Path::new("powershell.exe"),
        vec!["-NoProfile".into(), "-Command".into(), command.into()],
    );
    #[cfg(not(windows))]
    let (program, args) = (Path::new("sh"), vec!["-lc".into(), command.into()]);
    let result = run_process_registered(program, &args, None, timeout, Some(cwd), None)?;
    let output = [result.stdout.trim(), result.stderr.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let output = strip_ansi(&output);
    if result.timed_out {
        return Err(format!("验证命令超时\n{output}"));
    }
    if result.code != Some(0) {
        return Err(validation_failure(result.code, &output));
    }
    Ok(output)
}

fn successful_validation_output(node: &DispatchNodeRequest, used_worktree: bool) -> Option<&'static str> {
    (node.role == "执行" && used_worktree && !node.fix_round)
        .then_some("worktree 节点未跑验证命令，采纳合并后再验")
}

#[derive(Debug, Default, PartialEq, Eq)]
struct RedGreenOutcome {
    pinned: String,
    note: String,
    validation_output: String,
    validation_failed: bool,
}

fn remove_empty_snapshot_parents(root: &Path, target: &Path) {
    let mut current = target.parent();
    while let Some(directory) = current {
        if directory == root || !directory.starts_with(root) {
            break;
        }
        let empty = fs::read_dir(directory).is_ok_and(|mut entries| entries.next().is_none());
        if !empty || fs::remove_dir(directory).is_err() {
            break;
        }
        current = directory.parent();
    }
}

fn restore_snapshot_paths(
    root: &Path,
    snapshot: &DirectorySnapshot,
    paths: &[String],
) -> Result<(), String> {
    let root_canonical = root.canonicalize().map_err(|error| format!("核对目录不可用：{error}"))?;
    for path in paths {
        let relative = safe_relative_path(path).ok_or_else(|| format!("核对路径不安全：{path}"))?;
        let target = root.join(relative);
        if let Ok(metadata) = fs::symlink_metadata(&target) {
            if is_link_or_junction(&metadata) {
                return Err(format!("核对路径是链接，未恢复：{path}"));
            }
        }
        match snapshot.get(path) {
            Some(file) => {
                let bytes = file.bytes.as_deref().ok_or_else(|| format!("核对快照未保存文件内容：{path}"))?;
                let parent = target.parent().ok_or_else(|| format!("核对路径没有父目录：{path}"))?;
                fs::create_dir_all(parent).map_err(|error| format!("创建核对恢复目录失败：{error}"))?;
                let parent_canonical = parent.canonicalize().map_err(|error| format!("核对恢复目录不可用：{error}"))?;
                if !parent_canonical.starts_with(&root_canonical) {
                    return Err(format!("核对恢复路径超出工作目录：{path}"));
                }
                fs::write(&target, bytes).map_err(|error| format!("恢复核对后文件失败 {path}：{error}"))?;
            }
            None => {
                if target.is_file() {
                    fs::remove_file(&target).map_err(|error| format!("移除核对后新增文件失败 {path}：{error}"))?;
                    remove_empty_snapshot_parents(root, &target);
                }
            }
        }
    }
    Ok(())
}

fn red_green_check(
    project: &Path,
    workdir: &Path,
    before_dir: &str,
    before: &DirectorySnapshot,
    after: &DirectorySnapshot,
    command: &str,
    timeout: Duration,
) -> RedGreenOutcome {
    let changed = changed_snapshot_paths(before, after);
    if before_dir.trim().is_empty() {
        return RedGreenOutcome {
            pinned: "skipped".into(),
            note: "缺少派发前快照，未做红绿核对".into(),
            ..RedGreenOutcome::default()
        };
    }
    if changed.iter().any(|path| after.get(path).is_some_and(|file| file.bytes.is_none())) {
        return RedGreenOutcome {
            pinned: "skipped".into(),
            note: "本次改动含未保存内容的文件，未做红绿核对".into(),
            ..RedGreenOutcome::default()
        };
    }
    let before_root = PathBuf::from(before_dir);
    let before_root = if before_root.is_absolute() { before_root } else { project.join(before_root) };
    if !before_root.is_dir() {
        return RedGreenOutcome {
            pinned: "skipped".into(),
            note: "派发前快照不存在，未做红绿核对".into(),
            ..RedGreenOutcome::default()
        };
    }
    if fs::read_to_string(before_root.join(".skipped")).is_ok_and(|value| !value.trim().is_empty()) {
        return RedGreenOutcome {
            pinned: "skipped".into(),
            note: "派发前快照不完整，未做红绿核对".into(),
            ..RedGreenOutcome::default()
        };
    }

    let rollback = match restore_before_dir(workdir, &before_root) {
        Ok(summary) if summary.skipped.is_empty() => {
            for path in changed.iter().filter(|path| !before.contains_key(*path)) {
                remove_empty_snapshot_parents(workdir, &workdir.join(path));
            }
            Ok(())
        }
        Ok(summary) => Err(format!("{}，未做红绿核对", summary.note)),
        Err(error) => Err(format!("临时回退修复失败：{error}")),
    };
    if let Err(note) = rollback {
        let _ = restore_snapshot_paths(workdir, after, &changed);
        return RedGreenOutcome { pinned: "skipped".into(), note, ..RedGreenOutcome::default() };
    }

    let red = run_validation(command, workdir, timeout);
    let restored = restore_snapshot_paths(workdir, after, &changed);
    if let Err(error) = restored {
        return RedGreenOutcome {
            pinned: "skipped".into(),
            note: "红绿核对恢复后验证未通过".into(),
            validation_output: error,
            validation_failed: true,
        };
    }
    let green = run_validation(command, workdir, timeout);
    let final_restore = restore_snapshot_paths(workdir, after, &changed);
    if let Err(error) = final_restore {
        return RedGreenOutcome {
            pinned: "skipped".into(),
            note: "红绿核对恢复后验证未通过".into(),
            validation_output: error,
            validation_failed: true,
        };
    }

    let red_timed_out = red.as_ref().is_err_and(|error| error.starts_with("验证命令超时"));
    let green_timed_out = green.as_ref().is_err_and(|error| error.starts_with("验证命令超时"));
    let validation_output = format!(
        "[回退修复后]\n{}\n\n[恢复修复后]\n{}",
        red.as_ref().map(|value| value.as_str()).unwrap_or_else(|error| error.as_str()),
        green.as_ref().map(|value| value.as_str()).unwrap_or_else(|error| error.as_str()),
    );
    if red_timed_out || green_timed_out {
        return RedGreenOutcome {
            pinned: "skipped".into(),
            note: "红绿核对验证命令超时，未核对".into(),
            validation_output,
            validation_failed: false,
        };
    }
    if green.is_err() {
        return RedGreenOutcome {
            pinned: "skipped".into(),
            note: "红绿核对恢复后验证未通过".into(),
            validation_output,
            validation_failed: true,
        };
    }
    if red.is_ok() {
        return RedGreenOutcome {
            pinned: "unpinned".into(),
            note: "回退修复后测试仍通过：新增测试没有钉住这个修复".into(),
            validation_output,
            validation_failed: false,
        };
    }
    RedGreenOutcome {
        pinned: "pinned".into(),
        note: "回退修复后测试失败，恢复后测试通过".into(),
        validation_output,
        validation_failed: false,
    }
}

fn redact_artifacts(value: &mut DispatchArtifacts) -> usize {
    let mut redacted = 0;
    let (diff, count) = redact_text(&value.diff, true);
    value.diff = diff;
    redacted += count;
    for file in &mut value.changed_files {
        let (content, count) = redact_text(&file.content, true);
        file.content = content;
        redacted += count;
    }
    let (summary, count) = redact_text(&value.stdout_summary, true);
    value.stdout_summary = summary;
    redacted + count
}

// 主/备尝试各自的原始输出转存：备选按实际尝试序号加后缀，互不覆盖。
fn record_stdout(raw_stdout: &str, run_root: &Path, node_id: &str, attempt: &str) -> String {
    if raw_stdout.len() <= 32 * 1024 {
        return raw_stdout.to_string();
    }
    let suffix = attempt.strip_prefix("备").map(|value| {
        let index = value.trim().parse::<u32>().unwrap_or(1);
        format!(".备{index}")
    }).unwrap_or_default();
    let raw_path = run_root.join(format!("{}{}.raw.txt", safe_name(node_id), suffix));
    let tail = raw_stdout
        .chars()
        .rev()
        .take(4096)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    match fs::write(&raw_path, raw_stdout.as_bytes()) {
        Ok(()) => format!("原始输出已转存：{}\n末尾 4KB：\n{}", raw_path.display(), tail),
        Err(error) => format!("原始输出转存失败：{error}\n末尾 4KB：\n{tail}"),
    }
}

fn nonempty_process_error(result: &crate::process::ProcessResult, fallback: &str) -> String {
    let text = if result.stderr.trim().is_empty() {
        result.stdout.trim()
    } else {
        result.stderr.trim()
    };
    if text.is_empty() {
        fallback.into()
    } else {
        text.into()
    }
}

enum AcpResolution<T> {
    Use(T),
    Fallback(String),
    Failed(channels::acp::AcpError),
    Cancelled(String),
}

fn resolve_acp_attempt<T>(
    result: Result<T, channels::acp::AcpError>,
    cancelled: bool,
) -> AcpResolution<T> {
    match result {
        Ok(output) => AcpResolution::Use(output),
        Err(error) if cancelled || error.kind == channels::acp::AcpErrorKind::Cancelled => {
            AcpResolution::Cancelled(error.message)
        }
        Err(error) if error.permits_fallback() => AcpResolution::Fallback(error.message),
        Err(error) => AcpResolution::Failed(error),
    }
}

// 显式备选保留既有的 error/timeout/identity_mismatch 语义；额度用尽也会触发，
// 并可继续走资源库里的有序备选。validation_failed 仍交给修正轮。
fn should_try_fallback(status: &str, has_fallback: bool) -> bool {
    has_fallback && matches!(status, "error" | "timeout" | "identity_mismatch" | "quota_exhausted")
}

#[cfg(test)]
fn run_one(
    run_id: &str,
    project: &Path,
    run_root: &Path,
    log_dir: &Path,
    settings: &DispatchSettings,
    node: DispatchNodeRequest,
) -> DispatchNodeResult {
    run_one_with_activity(run_id, project, run_root, log_dir, settings, node, ProcessActivity::new())
}

fn run_one_with_activity(
    run_id: &str,
    project: &Path,
    run_root: &Path,
    log_dir: &Path,
    settings: &DispatchSettings,
    node: DispatchNodeRequest,
    activity: ProcessActivity,
) -> DispatchNodeResult {
    let cancel_key = registry_key(run_id, &node.node_id);
    // 可写节点只在额度用尽时切备选，并在每次切换前回滚该次尝试的改动。
    // 其它 error/timeout/identity_mismatch 保持 05B 语义，不自动换通道。
    let explicit = node
        .fallback
        .as_ref()
        .map(|f| !f.agent.trim().is_empty() && !f.model.trim().is_empty())
        .unwrap_or(false);
    let configured = resource_fallback_models(&node.agent, &node.provider, &node.model);
    let has_fallback = explicit || !configured.is_empty();
    let writable = node.visibility != "盲" && node.write != "只读";
    let primary = run_one_attempt(
        run_id,
        project,
        run_root,
        log_dir,
        settings,
        node.clone(),
        "主",
        // 05B：可写节点非额度失败不切备选，留痕与结果都标原因；额度用尽的情况在下面回滚后切，不标。
        if writable && has_fallback { "可写节点" } else { "" },
        &activity,
    );
    let mut candidates = Vec::new();
    if let Some(fallback) = node.fallback.clone().filter(|item| !item.agent.trim().is_empty() && !item.model.trim().is_empty()) {
        let exhausted_until = resource_quota_until(&fallback.agent, &fallback.provider, &fallback.model);
        candidates.push((fallback, true, exhausted_until));
    }
    for model in configured {
        candidates.push((FallbackSpec {
            agent: model.agent.clone(),
            provider: model.provider.clone(),
            model: model.id.clone(),
            vendor: model.vendor.clone(),
        }, false, model.quota_exhausted_until.clone()));
    }
    let primary_key = crate::resources::resource_model_key(&node.agent, &node.provider, &node.model);
    let mut seen = HashSet::from([primary_key]);
    candidates.retain(|(candidate, _, _)| seen.insert(crate::resources::resource_model_key(&candidate.agent, &candidate.provider, &candidate.model)));

    let mut result = primary;
    let mut resources_changed = result.resources_changed;
    let mut previous_route = route_name(&node.agent, &node.provider, &node.model);
    let mut quota_notes = Vec::new();
    let mut quota_resets = Vec::new();
    let mut attempt_index = 0_u32;
    let mut current_attempt_rolled_back = false;
    if result.status == "quota_exhausted" {
        quota_notes.push(format!("主 {previous_route} 额度用尽"));
        if let Some(value) = result.quota_resets_at.clone() { quota_resets.push(value); }
    }
    for (candidate, is_explicit, exhausted_until) in candidates {
        let eligible = if writable {
            result.status == "quota_exhausted"
        } else if is_explicit {
            should_try_fallback(&result.status, true)
        } else {
            result.status == "quota_exhausted"
        };
        if !eligible || is_cancelled(&cancel_key) {
            break;
        }
        if writable && !current_attempt_rolled_back {
            match rollback_quota_attempt(project, &result, Duration::from_secs(settings.timeout_secs.clamp(1, 3600))) {
                Ok(note) => {
                    quota_notes.push(note);
                    current_attempt_rolled_back = true;
                }
                Err(error) => {
                    result.fallback_skipped = "回滚失败".into();
                    append_runner_note(&mut result.runner_note, &format!("额度用尽，回滚本次改动失败：{error}"));
                    current_attempt_rolled_back = true;
                    break;
                }
            }
        }
        let next_route = route_name(&candidate.agent, &candidate.provider, &candidate.model);
        if quota_until_active(exhausted_until.as_deref()) {
            if let Some(value) = exhausted_until {
                quota_resets.push(value.clone());
                quota_notes.push(format!("跳过 {next_route}（额度用尽至 {value}）"));
            }
            continue;
        }
        attempt_index += 1;
        let mut next = run_fallback_attempt(
            run_id, project, run_root, log_dir, settings, &node, &candidate,
            &format!("备 {attempt_index}"), &activity,
        );
        next.fallback_from = previous_route.clone();
        next.fallback_to = next_route.clone();
        previous_route = next_route.clone();
        if next.status == "quota_exhausted" {
            quota_notes.push(format!("备 {next_route} 额度用尽"));
            if let Some(value) = next.quota_resets_at.clone() { quota_resets.push(value); }
        } else if !quota_notes.is_empty() {
            quota_notes.push(format!("备 {next_route}"));
        }
        result = next;
        resources_changed |= result.resources_changed;
        current_attempt_rolled_back = false;
    }
    if writable && has_fallback && result.status == "quota_exhausted" && !current_attempt_rolled_back {
        match rollback_quota_attempt(project, &result, Duration::from_secs(settings.timeout_secs.clamp(1, 3600))) {
            Ok(note) => quota_notes.push(note),
            Err(error) => {
                result.fallback_skipped = "回滚失败".into();
                append_runner_note(&mut result.runner_note, &format!("额度用尽，回滚本次改动失败：{error}"));
            }
        }
    }
    if !quota_notes.is_empty() {
        append_runner_note(&mut result.runner_note, &quota_notes.join(" → "));
    }
    if result.status == "quota_exhausted" {
        result.quota_resets_at = earliest_reset(quota_resets);
    }
    result.resources_changed = resources_changed;
    clear_cancelled(&cancel_key);
    result
}

fn rollback_quota_attempt(project: &Path, result: &DispatchNodeResult, timeout: Duration) -> Result<String, String> {
    #[cfg(test)]
    if QUOTA_ROLLBACK_FAILURE.with(|cell| cell.get()) {
        return Err("测试注入的回滚失败".into());
    }
    if let Some(worktree) = result.artifacts.as_ref().and_then(|artifacts| artifacts.worktree.as_deref()) {
        let removed = run_process_registered(
            Path::new("git"),
            &["worktree".into(), "remove".into(), "--force".into(), worktree.into()],
            None,
            timeout,
            Some(project),
            None,
        )?;
        if removed.timed_out || removed.code != Some(0) {
            return Err(nonempty_process_error(&removed, "清理本次 worktree 失败"));
        }
        return Ok(format!("已回滚 {} 本次改动", result.node_id));
    }
    // 只认本次尝试的快照：本次没改动就无需回滚；绝不拿历史快照把上一次的产物或用户手改一起还原。
    let before_dir = result.artifacts.as_ref().map(|artifacts| artifacts.before_dir.trim()).unwrap_or("");
    if before_dir.is_empty() {
        return Ok(format!("{} 本次没有改动，无需回滚", result.node_id));
    }
    let root = PathBuf::from(before_dir);
    let root = if root.is_absolute() { root } else { project.join(root) };
    if !root.is_dir() {
        return Err("本次派发前快照不存在，无法回滚".into());
    }
    let summary = restore_before_dir(project, &root)?;
    if !summary.skipped.is_empty() {
        return Err(format!("{}，无法确认全部改动已恢复", summary.note));
    }
    Ok(format!("已回滚 {} 本次改动", result.node_id))
}

fn route_name(agent: &str, _provider: &str, model: &str) -> String {
    format!("{agent}/{model}")
}

fn quota_until_active(value: Option<&str>) -> bool {
    value
        .and_then(|item| DateTime::parse_from_rfc3339(item).ok())
        .map(|item| item.timestamp_millis() > Local::now().timestamp_millis())
        .unwrap_or(false)
}

fn earliest_reset(values: Vec<String>) -> Option<String> {
    values.into_iter().filter_map(|value| {
        DateTime::parse_from_rfc3339(&value).ok().map(|parsed| (parsed.timestamp_millis(), value))
    }).min_by_key(|(timestamp, _)| *timestamp).map(|(_, value)| value)
}

// 备选重跑：同 prompt、同可见性、同写权限，只换 agent/provider/model；
// 留痕由 run_one_attempt 各写一条（attempt: 主 / 备），结果带上主备路由供界面显示。
#[cfg(test)]
fn run_fallback(
    run_id: &str,
    project: &Path,
    run_root: &Path,
    log_dir: &Path,
    settings: &DispatchSettings,
    node: &DispatchNodeRequest,
    activity: &ProcessActivity,
) -> DispatchNodeResult {
    let fallback = node.fallback.clone().unwrap_or_default();
    run_fallback_attempt(run_id, project, run_root, log_dir, settings, node, &fallback, "备", activity)
}

fn run_fallback_attempt(
    run_id: &str,
    project: &Path,
    run_root: &Path,
    log_dir: &Path,
    settings: &DispatchSettings,
    node: &DispatchNodeRequest,
    fallback: &FallbackSpec,
    attempt: &str,
    activity: &ProcessActivity,
) -> DispatchNodeResult {
    // 盲节点的隔离目录在第一次尝试时已铺过材料；重跑前清掉，否则 prepare_outbound
    // 会因目录非空而拒绝。
    if node.visibility == "盲" {
        let dir = run_root.join(&node.node_id);
        if dir.exists() {
            let _ = fs::remove_dir_all(&dir);
        }
    }
    let mut retry = node.clone();
    retry.agent = fallback.agent.clone();
    retry.provider = fallback.provider.clone();
    retry.model = fallback.model.clone();
    // 计费口径按备选目标记：vendor 优先用前端随请求下发的真实值（R2-4），
    // 没下发再按模型 ID 推断（同 resources 的既有口径）；都拿不到就置空，
    // 不抄主目标的。billing_source 取备选 provider。
    retry.vendor = if !fallback.vendor.trim().is_empty() {
        fallback.vendor.clone()
    } else {
        let inferred_vendor = crate::resources::infer_vendor(&fallback.model);
        if inferred_vendor == "其他" {
            String::new()
        } else {
            inferred_vendor.into()
        }
    };
    retry.billing_source = if fallback.provider.trim().is_empty() {
        String::new()
    } else {
        fallback.provider.clone()
    };
    retry.fallback = None; // 备选不再套备选，最多重跑一次
    retry.session_id = String::new(); // 换通道不续接旧会话
    let mut result = run_one_attempt(run_id, project, run_root, log_dir, settings, retry, attempt, "", activity);
    // R2-1：这个 session 属于备选通道，而节点 target 仍是主目标；写回会让下游
    // 同目标「共享」子节点经 continuationSession 把它续到主通道（-S/-r 一个
    // 主通道不存在的会话）。备选作答 = 另起会话不续聊：结果 session 置空，
    // 下游节点发完整 prompt；留痕里保留真实 session 仅供查账。
    result.session_id = String::new();
    result.fallback_from = route_name(&node.agent, &node.provider, &node.model);
    result.fallback_to = route_name(&fallback.agent, &fallback.provider, &fallback.model);
    result
}

#[cfg(test)]
thread_local! {
    /// 首次验证的测试替身。测试里默认不碰用户真实的 resources.json：没装替身
    /// 就按 Skipped 放行，要验接线的那条测试自己装。
    static FIRST_USE_STUB: std::cell::RefCell<Option<crate::resources::FirstUseVerification>> =
        const { std::cell::RefCell::new(None) };
    /// 替身收到的 (agent, provider, model)，按调用顺序记下来：钉住「键传对了」
    /// 和「验过的第二次不再验」（修正轮 R1，原来替身把三个参数直接丢掉）。
    static FIRST_USE_CALLS: std::cell::RefCell<Vec<(String, String, String)>> =
        const { std::cell::RefCell::new(Vec::new()) };
    /// TASK-14：资源路由测试替身。None 表示测试不读用户真实 resources.json。
    static RESOURCE_MODELS_STUB: std::cell::RefCell<Option<Vec<crate::resources::ResourceModel>>> =
        const { std::cell::RefCell::new(None) };
    static QUOTA_MARKS: std::cell::RefCell<Vec<(String, String, String, Option<String>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static QUOTA_ROLLBACK_FAILURE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn resource_fallback_models(agent: &str, provider: &str, model: &str) -> Vec<crate::resources::ResourceModel> {
    #[cfg(test)]
    {
        return RESOURCE_MODELS_STUB.with(|cell| {
            let borrowed = cell.borrow();
            let Some(models) = borrowed.as_ref() else { return Vec::new() };
            let Some(current) = models.iter().find(|item| item.agent == agent && item.provider == provider && item.id == model) else { return Vec::new() };
            current.fallback_order.iter().filter_map(|key| {
                models.iter().find(|item| crate::resources::resource_model_key(&item.agent, &item.provider, &item.id) == *key).cloned()
            }).collect()
        });
    }
    #[cfg(not(test))]
    crate::resources::fallback_models(agent, provider, model).unwrap_or_default()
}

fn resource_quota_until(agent: &str, provider: &str, model: &str) -> Option<String> {
    #[cfg(test)]
    {
        return RESOURCE_MODELS_STUB.with(|cell| cell.borrow().as_ref().and_then(|models| {
            models.iter().find(|item| item.agent == agent && item.provider == provider && item.id == model)
                .and_then(|item| item.quota_exhausted_until.clone())
        }));
    }
    #[cfg(not(test))]
    crate::resources::quota_exhausted_until(agent, provider, model)
}

fn record_quota_hit(agent: &str, provider: &str, model: &str, resets_at: Option<&str>) -> bool {
    #[cfg(test)]
    {
        QUOTA_MARKS.with(|cell| cell.borrow_mut().push((agent.into(), provider.into(), model.into(), resets_at.map(str::to_string))));
        resets_at.is_some()
    }
    #[cfg(not(test))]
    {
        resets_at.is_some() && crate::resources::mark_quota_exhausted(agent, provider, model, resets_at).is_ok()
    }
}

fn classify_attempt_quota(status: &str, raw_stderr: &str, error: &str, exit_code: Option<i32>) -> Option<channels::QuotaHit> {
    if status != "error" {
        return None;
    }
    channels::classify_quota(&[raw_stderr, error].join("\n"), exit_code, None)
}

fn first_use_verification(
    agent: &str,
    provider: &str,
    model: &str,
    cancel_key: &str,
) -> crate::resources::FirstUseVerification {
    #[cfg(test)]
    {
        let _ = cancel_key;
        FIRST_USE_CALLS.with(|cell| {
            cell.borrow_mut()
                .push((agent.to_string(), provider.to_string(), model.to_string()))
        });
        FIRST_USE_STUB
            .with(|cell| cell.borrow_mut().take())
            .unwrap_or(crate::resources::FirstUseVerification::Skipped)
    }
    #[cfg(not(test))]
    crate::resources::verify_on_first_use(agent, provider, model, Some(cancel_key))
}

fn run_one_attempt(
    run_id: &str,
    project: &Path,
    run_root: &Path,
    log_dir: &Path,
    settings: &DispatchSettings,
    mut node: DispatchNodeRequest,
    attempt: &str,
    fallback_skip_reason: &str,
    activity: &ProcessActivity,
) -> DispatchNodeResult {
    activity.reset_attempt();
    let stage = node.stage.max(1);
    let started = Local::now();
    let timer = Instant::now();
    let cancel_key = registry_key(run_id, &node.node_id);
    let blind = node.visibility == "盲";
    if blind {
        node.write = "只读".into();
    }
    let isolation = channels::isolation_for(&node.agent, blind).to_string();
    let timeout = Duration::from_secs(settings.timeout_secs.clamp(1, 3600));
    // 09-15 口径：第三方端点的模型第一次被派发到时，先真发一个短请求核对真身，
    // 结果记进资源库。失败即本节点失败、不自动重试（用户到接入页手动重验）；
    // 原生 CLI（官方端点的 claude / codex / kimi）与验过 / 验失败过的直接跳过。
    //
    // 修正轮 R1：整段移到取项目写锁、建 worktree、回滚、快照、备料之前——
    // 一次验证最长 60s，期间不该占着项目写锁让同项目其它单写者节点陪等，
    // 验证失败也不该留下一个白建的 worktree 和一份白铺的隔离目录。
    let mut verify_error = String::new();
    let mut verify_note = String::new();
    let mut verify_quota_resets_at = None;
    if is_cancelled(&cancel_key) {
        verify_error = "派发已取消".to_string();
    } else {
        match first_use_verification(&node.agent, &node.provider, &node.model, &cancel_key) {
            crate::resources::FirstUseVerification::Skipped => {}
            crate::resources::FirstUseVerification::NotInStore => {
                append_runner_note(&mut verify_note, "资源库无此条目，未做首次验证");
            }
            crate::resources::FirstUseVerification::Verified => {
                append_runner_note(&mut verify_note, "首次派发已验证真身");
            }
            crate::resources::FirstUseVerification::Failed(reason) => {
                append_runner_note(&mut verify_note, "首次派发自动验证");
                verify_error = format!("首次验证失败：{reason}");
            }
            crate::resources::FirstUseVerification::QuotaExhausted { resets_at } => {
                append_runner_note(&mut verify_note, "首次派发自动验证命中额度限制");
                verify_error = "首次验证请求额度用尽".to_string();
                verify_quota_resets_at = resets_at;
            }
            crate::resources::FirstUseVerification::Cancelled => {
                append_runner_note(&mut verify_note, "首次派发自动验证，验证中已取消");
                verify_error = "派发已取消".to_string();
            }
        }
        // 验证窗口内点了叫停：这次不派发（原来验证完照样往下走）。
        if verify_error.is_empty() && is_cancelled(&cancel_key) {
            verify_error = "派发已取消".to_string();
        }
    }
    let verified = verify_error.is_empty();
    let mut worktree_warning = String::new();
    let mut worktree_path = None;
    let mut uses_single_writer = node.write == "单写者";
    let workdir = if blind {
        run_root.join(&node.node_id)
    } else if node.write == "worktree" && verified {
        let path = run_root.join(&node.node_id).join("wt");
        match create_worktree(project, &path, timeout) {
            Ok(()) => {
                worktree_path = Some(path.to_string_lossy().into_owned());
                path
            }
            Err(error) => {
                uses_single_writer = true;
                worktree_warning = format!("worktree 不可用，已退回单写者：{error}");
                project.to_path_buf()
            }
        }
    } else {
        project.to_path_buf()
    };
    let (_write_guard, write_lock_error) = if uses_single_writer && verified {
        match acquire_project_write(project, timeout, &cancel_key) {
            Ok(guard) => (Some(guard), None),
            Err(error) => (None, Some(error)),
        }
    } else {
        (None, None)
    };
    let (rollback_note, rollback_error) = if verified && write_lock_error.is_none() && node.rollback_before && uses_single_writer {
        match rollback_node_changes_core(project, run_id, &node.node_id) {
            Ok(summary) => (summary.note, None),
            Err(error) => (String::new(), Some(format!("回滚上次改动失败：{error}"))),
        }
    } else {
        (String::new(), None)
    };
    let (before_snapshot, snapshot_error) = if verified
        && write_lock_error.is_none()
        && !blind
        && node.write != "只读"
    {
        match snapshot_directory(&workdir) {
            Ok(snapshot) => (Some(snapshot), None),
            Err(error) => (None, Some(error)),
        }
    } else {
        (None, None)
    };
    let task_dir = run_root.join("tasks");
    let task_file = task_dir.join(format!("{}.txt", safe_name(&node.node_id)));
    let session_dir = dispatch_data_root()
        .map(|root| root.join("sessions").join(safe_name(&node.agent)))
        .ok();
    // 首次验证没过就不铺材料：REQUEST.md / diff.patch / changed 白铺一份，
    // 节点还是 error。
    let prepared = if verified {
        prepare_outbound(&node, settings.redact, &workdir, blind)
            .map(|(prompt, names, count)| OutboundPreparation::Materialized(prompt, names, count))
    } else {
        Ok(OutboundPreparation::Unsent)
    };
    // TASK-12：审查材料附带验证输出（盲目录物化后、派发前；一处调用）。
    let (prepared, validation_attached) =
        attach_review_validation(run_id, project, settings, &node, &workdir, blind, timeout, prepared);
    let effective_session =
        if channels::normalize_agent(&node.agent) == "pi" && node.session_id.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            node.session_id.clone()
        };

    let (prompt, material_names, prepared_redacted, preparation_error) = match prepared {
        Ok(OutboundPreparation::Materialized(prompt, names, count)) => (prompt, names, count, String::new()),
        Ok(OutboundPreparation::Unsent) => ("(未发送)".into(), Vec::new(), 0, String::new()),
        Err(error) => ("(未发送)".into(), Vec::new(), 0, error),
    };
    let continuing = is_continuing(&node);
    let (outbound, mut redacted) = if continuing {
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
    let mut entered_channel = false;
    let mut status = "error".to_string();
    if verify_quota_resets_at.is_some() || verify_error == "首次验证请求额度用尽" {
        status = "quota_exhausted".to_string();
    }
    let mut runner = "oneshot".to_string();
    let mut runner_note = rollback_note;
    if !verify_note.is_empty() {
        append_runner_note(&mut runner_note, &verify_note);
    }
    let mut oneshot_session = effective_session.clone();
    let mut oneshot_outbound = outbound.clone();
    let mut error = if verified {
        write_lock_error
            .or(rollback_error)
            .or(snapshot_error)
            .unwrap_or(preparation_error)
    } else {
        verify_error
    };
    if error.is_empty() && is_cancelled(&cancel_key) {
        error = "派发已取消".into();
    }

    let mut needs_oneshot = error.is_empty();
    let uses_acp = settings.acp_enabled && channels::acp::supports(&node.agent);
    if needs_oneshot && !uses_acp && node.previous_runner == "acp" {
        (oneshot_session, oneshot_outbound) = reset_acp_fallback(&prompt);
        append_runner_note(&mut runner_note, "本轮不走 ACP，旧 ACP 会话已重置，改一次性完整上下文");
    }
    if needs_oneshot && uses_acp {
        entered_channel = true;
        let attempt = channels::acp::run(
            &node.agent,
            &node.provider,
            &node.model,
            (!node.session_id.is_empty()).then_some(node.session_id.as_str()),
            &outbound,
            &workdir,
            timeout,
            &node.thinking,
            node.write != "只读",
            blind,
            &cancel_key,
            activity.clone(),
        );
        match resolve_acp_attempt(attempt, is_cancelled(&cancel_key)) {
            AcpResolution::Use(output) => {
                channel_output = output.channel;
                raw_stdout = output.raw_stdout;
                raw_stderr = output.raw_stderr;
                status = "ok".into();
                runner = "acp".into();
                let permission_note = if node.write == "只读" || blind {
                    "ACP 权限：读类放行，写/执行类拒绝，仅本次授权"
                } else {
                    "ACP 权限：仅本次授权，永不持久授权"
                };
                append_runner_note(&mut runner_note, &permission_note);
                if !output.runner_note.is_empty() {
                    runner_note.push_str("；");
                    runner_note.push_str(&output.runner_note);
                }
                needs_oneshot = false;
            }
            AcpResolution::Cancelled(reason) => {
                error = reason;
                runner = "acp".into();
                append_runner_note(&mut runner_note, "ACP 已取消，未回退");
                needs_oneshot = false;
            }
            AcpResolution::Fallback(reason) => {
                if channels::classify_quota(&reason, None, None).is_some() {
                    raw_stderr = reason.clone();
                    error = reason;
                    runner = "acp".into();
                    needs_oneshot = false;
                } else {
                    (oneshot_session, oneshot_outbound) = reset_acp_fallback(&prompt);
                    append_runner_note(&mut runner_note, &format!("ACP 会话已重置，改一次性完整上下文：{reason}"));
                    activity.reset_attempt();
                }
            }
            AcpResolution::Failed(acp_error) => {
                timed_out = acp_error.kind == channels::acp::AcpErrorKind::Timeout;
                error = acp_error.message;
                status = if timed_out {
                    "timeout".into()
                } else {
                    "error".into()
                };
                runner = "acp".into();
                let failure_note = if timed_out {
                    "ACP 超时，未回退"
                } else {
                    "ACP 失败，未回退"
                };
                append_runner_note(&mut runner_note, &failure_note);
                needs_oneshot = false;
            }
        }
    }

    if needs_oneshot {
        let _ = fs::write(&task_file, oneshot_outbound.as_bytes());
        match channels::build_invocation(
            &node.agent,
            &node.provider,
            &node.model,
            &node.tier,
            &node.thinking,
            blind,
            (!oneshot_session.is_empty()).then_some(oneshot_session.as_str()),
            &oneshot_outbound,
            Some(&task_file),
            session_dir.as_deref(),
            (!settings.cc_switch_path.trim().is_empty())
                .then(|| Path::new(settings.cc_switch_path.trim())),
            node.write != "只读",
        ) {
            Ok(invocation) => {
                entered_channel = true;
                match run_process_registered_with_activity(
                &invocation.program,
                &invocation.args,
                invocation.input.as_deref(),
                timeout,
                Some(&workdir),
                Some(&cancel_key),
                Some(activity.clone()),
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
                                if parse_error.kind == ChannelErrorKind::IdentityMismatch {
                                    status = "identity_mismatch".into();
                                }
                                channel_output.identity = parse_error.identity;
                                error = parse_error.message;
                            }
                        }
                    }
                }
                Err(run_error) => error = run_error,
                }
            },
            Err(build_error) => error = build_error,
        }
    }

    let mut quota_resets_at = verify_quota_resets_at;
    let mut resources_changed = false;
    if status == "quota_exhausted" && !entered_channel {
        resources_changed = record_quota_hit(&node.agent, &node.provider, &node.model, quota_resets_at.as_deref());
    } else if entered_channel {
        if let Some(hit) = classify_attempt_quota(&status, &raw_stderr, &error, exit_code) {
            status = "quota_exhausted".into();
            quota_resets_at = hit.resets_at;
            resources_changed = record_quota_hit(&node.agent, &node.provider, &node.model, quota_resets_at.as_deref());
        }
    }

    let mut validation_output = String::new();
    if status == "ok" && successful_validation_output(&node, worktree_path.is_some()).is_some() {
        validation_output = successful_validation_output(&node, true).unwrap().into();
    } else if status == "ok" && node.role == "执行" && node.write != "只读" {
        match run_validation(&settings.validation_command, &workdir, timeout) {
            Ok(output) => validation_output = output,
            Err(validation_error) => {
                validation_output = validation_error.clone();
                status = "validation_failed".into();
                error = validation_error;
            }
        }
    }
    let mut artifacts = None;
    let mut before_dir = String::new();
    let mut snapshots_for_red_green = None;
    if let Some(before) = before_snapshot {
        let built = snapshot_directory(&workdir).and_then(|after| {
            if entered_channel {
                match persist_before_snapshot_with_note(project, log_dir, run_id, &node.node_id, &before, &after) {
                    Ok(saved) => {
                        before_dir = saved.path;
                        append_runner_note(&mut runner_note, &saved.note);
                    }
                    Err(error) => append_runner_note(&mut runner_note, &format!("保存派发前快照失败：{error}")),
                }
            }
            let built = artifact_diff_for_project(project, run_root, &before, &after, timeout);
            snapshots_for_red_green = Some((before, after));
            built
        });
        match built {
            Ok(mut value) => {
                value.worktree = worktree_path.clone();
                value.before_dir = before_dir.clone();
                let summary = safe_trace(&channel_output.text)
                    .chars()
                    .take(2000)
                    .collect::<String>();
                value.stdout_summary = if worktree_warning.is_empty() {
                    summary
                } else if summary.is_empty() {
                    worktree_warning.clone()
                } else {
                    format!("{worktree_warning}\n{summary}")
                };
                if settings.redact {
                    redacted += redact_artifacts(&mut value);
                }
                artifacts = Some(value);
            }
            Err(snapshot_error) => {
                status = "error".into();
                error = snapshot_error;
            }
        }
    }

    let mut pinned = None;
    let mut pinned_note = None;
    let should_check_red_green = status == "ok"
        && node.fix_round
        && node.role == "执行"
        && matches!(node.write.as_str(), "单写者" | "worktree")
        && !settings.validation_command.trim().is_empty();
    if should_check_red_green {
        let red_green_guard = if worktree_path.is_some() {
            acquire_project_write(&workdir, timeout, &cancel_key).ok()
        } else {
            None
        };
        let outcome = if worktree_path.is_some() && red_green_guard.is_none() {
            RedGreenOutcome {
                pinned: "skipped".into(),
                note: "红绿核对未取得节点写锁，未核对".into(),
                ..RedGreenOutcome::default()
            }
        } else if let Some((before, after)) = snapshots_for_red_green.as_ref() {
            red_green_check(
                project,
                &workdir,
                &before_dir,
                before,
                after,
                &settings.validation_command,
                timeout,
            )
        } else {
            RedGreenOutcome {
                pinned: "skipped".into(),
                note: "缺少派发前快照，未做红绿核对".into(),
                ..RedGreenOutcome::default()
            }
        };
        if !outcome.validation_output.is_empty() {
            validation_output = outcome.validation_output.clone();
        }
        if outcome.validation_failed {
            status = "validation_failed".into();
            error = outcome.note.clone();
        }
        pinned = Some(outcome.pinned);
        pinned_note = Some(outcome.note);
    }

    annotate_kimi_metadata(&node.agent, &mut channel_output, &mut runner_note);

    // CLI 输出可能回显请求头或配置片段。无论外发脱敏开关如何，留痕与
    // 返回给前端的错误文本都先做安全过滤。
    let raw_stdout = safe_trace(&raw_stdout);
    let raw_stderr = safe_trace(&raw_stderr);
    let error = safe_trace(&error);

    let ended = Local::now();
    let duration_ms = timer.elapsed().as_millis().min(u64::MAX as u128) as u64;
    // R2-11：主失败且可写节点跳过备选时，把原因写进留痕 frontmatter 与结果；
    // validation_failed 不触发备选（走修正轮），不标。
    let fallback_skipped = if !fallback_skip_reason.is_empty()
        && matches!(status.as_str(), "error" | "timeout" | "identity_mismatch")
    {
        fallback_skip_reason.to_string()
    } else {
        String::new()
    };
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
        runner: runner.clone(),
        runner_note: runner_note.clone(),
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
        attempt: if attempt.starts_with("备") { "备".into() } else { "主".into() },
        attempt_index: attempt.strip_prefix("备").and_then(|value| value.trim().parse().ok()).unwrap_or(0),
        write: node.write.clone(),
        fallback_skipped: fallback_skipped.clone(),
        before_dir,
        validation_attached,
        pinned: pinned.clone().unwrap_or_default(),
        swarm_item: None,
        merge_result: String::new(),
    };
    let frontmatter = serialize_frontmatter(&record).unwrap_or_else(|e| format!("error: {e}"));
    let record_stdout = record_stdout(&raw_stdout, run_root, &node.node_id, attempt);
    let dispatch_file = write_record(
        log_dir,
        &record,
        &frontmatter,
        &outbound,
        &channel_output.text,
        artifacts.as_ref(),
        &record_stdout,
        &raw_stderr,
    )
    .unwrap_or_default();

    let result = DispatchNodeResult {
        node_id: node.node_id,
        stage,
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
        runner,
        runner_note,
        workdir: workdir.to_string_lossy().into_owned(),
        dispatch_file,
        frontmatter,
        output_preview: raw_stdout.chars().take(2000).collect(),
        error,
        artifacts,
        validation_output: safe_trace(&validation_output),
        pinned,
        pinned_note: pinned_note.map(|note| safe_trace(&note)),
        fallback_from: String::new(),
        fallback_to: String::new(),
        fallback_skipped,
        quota_resets_at,
        resources_changed,
    };
    result
}

// ── TASK-12 审查材料附带验证输出 ─────────────────────────────────────────
// 审查员只读、不能跑测试：派盲审节点前引擎先跑一遍验证命令，输出（脱敏、截断
// 8 KB）写进盲目录 VALIDATION.txt，prompt 末尾补一句指路。只对盲节点生效；
// 验证命令为空时写一行「未配置验证命令」，同样算附上。

pub const REVIEW_VALIDATION_FILE: &str = "VALIDATION.txt";
pub const REVIEW_VALIDATION_LIMIT: usize = 8 * 1024;
const REVIEW_VALIDATION_PROMPT_NOTE: &str = "验证命令输出见 VALIDATION.txt";
const REVIEW_VALIDATION_UNSET: &str = "未配置验证命令";

#[derive(Debug)]
enum OutboundPreparation {
    Materialized(String, Vec<String>, usize),
    Unsent,
}

type ReviewValidationCell = Arc<OnceLock<String>>;

fn review_validation_cache() -> &'static Mutex<HashMap<String, ReviewValidationCell>> {
    static CACHE: OnceLock<Mutex<HashMap<String, ReviewValidationCell>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn review_validation_cache_key(run_id: &str, project: &Path) -> String {
    let project = fs::canonicalize(project).unwrap_or_else(|_| project.to_path_buf());
    format!("{run_id}\0{}", project.to_string_lossy())
}

fn clear_review_validation_cache(run_id: &str, project: &Path) {
    let key = review_validation_cache_key(run_id, project);
    review_validation_cache().lock().unwrap().remove(&key);
}

#[cfg(test)]
type ReviewValidationHook = Arc<
    dyn Fn(&str, &Path, Duration) -> Option<Result<String, String>> + Send + Sync,
>;

#[cfg(test)]
fn review_validation_hook() -> &'static Mutex<Option<ReviewValidationHook>> {
    static HOOK: OnceLock<Mutex<Option<ReviewValidationHook>>> = OnceLock::new();
    HOOK.get_or_init(|| Mutex::new(None))
}

fn run_review_validation(command: &str, cwd: &Path, timeout: Duration) -> Result<String, String> {
    #[cfg(test)]
    if let Some(result) = review_validation_hook()
        .lock()
        .unwrap()
        .clone()
        .and_then(|hook| hook(command, cwd, timeout))
    {
        return result;
    }
    run_validation(command, cwd, timeout)
}

/// 验证命令在「材料来源目录」跑：上游产物带 worktree 且目录还在就用它，否则项目目录。
fn review_material_dir(project: &Path, node: &DispatchNodeRequest) -> PathBuf {
    node.upstream_artifacts
        .iter()
        .filter_map(|artifact| artifact.worktree.as_deref())
        .map(PathBuf::from)
        .find(|path| path.is_dir())
        .unwrap_or_else(|| project.to_path_buf())
}

/// VALIDATION.txt 正文：命令行 + 结果；先脱敏（不受外发开关影响）再截断，
/// 截断落在字符边界上并注明。
fn review_validation_text(command: &str, outcome: Result<String, String>) -> String {
    let command = command.trim();
    let body = if command.is_empty() {
        REVIEW_VALIDATION_UNSET.to_string()
    } else {
        match outcome {
            Ok(output) if output.trim().is_empty() => "验证命令通过（exit 0）".to_string(),
            Ok(output) => format!("验证命令通过（exit 0）
{output}"),
            Err(error) => error,
        }
    };
    let text = if command.is_empty() {
        body
    } else {
        format!("验证命令：{command}

{body}")
    };
    let text = safe_trace(&text);
    if text.len() <= REVIEW_VALIDATION_LIMIT {
        return text;
    }
    let mut cut = REVIEW_VALIDATION_LIMIT;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}
…（输出超过 8 KB 已截断）", &text[..cut])
}

/// 同一 run、同一项目目录只执行一次验证命令；同批各盲验证节点复用同一正文。
fn cached_review_validation_text(
    run_id: &str,
    project: &Path,
    settings: &DispatchSettings,
    node: &DispatchNodeRequest,
    timeout: Duration,
) -> String {
    let key = review_validation_cache_key(run_id, project);
    let cell = {
        let mut cache = review_validation_cache().lock().unwrap();
        cache.entry(key).or_insert_with(|| Arc::new(OnceLock::new())).clone()
    };
    cell.get_or_init(|| {
        let command = settings.validation_command.trim();
        let outcome = if command.is_empty() {
            Ok(String::new())
        } else {
            let half = Duration::from_secs((timeout.as_secs() / 2).max(1));
            run_review_validation(command, &review_material_dir(project, node), half)
        };
        review_validation_text(command, outcome)
    }).clone()
}

/// 盲目录物化成功后附 VALIDATION.txt：返回改写后的 prompt / 材料名与「是否附上」。
/// 非盲节点、未开启、物化失败的一律原样返回且不附。
fn attach_review_validation(
    run_id: &str,
    project: &Path,
    settings: &DispatchSettings,
    node: &DispatchNodeRequest,
    workdir: &Path,
    blind: bool,
    timeout: Duration,
    prepared: Result<OutboundPreparation, String>,
) -> (Result<OutboundPreparation, String>, bool) {
    if !blind || !node.review_material_validation {
        return (prepared, false);
    }
    let (mut prompt, mut names, count) = match prepared {
        Ok(OutboundPreparation::Materialized(prompt, names, count)) => (prompt, names, count),
        Ok(OutboundPreparation::Unsent) => return (Ok(OutboundPreparation::Unsent), false),
        Err(error) => return (Err(error), false),
    };
    let text = cached_review_validation_text(run_id, project, settings, node, timeout);
    if let Err(error) = fs::write(workdir.join(REVIEW_VALIDATION_FILE), text.as_bytes()) {
        return (Err(format!("写入 {REVIEW_VALIDATION_FILE} 失败：{error}")), false);
    }
    names.push(REVIEW_VALIDATION_FILE.to_string());
    prompt.push_str("

");
    prompt.push_str(REVIEW_VALIDATION_PROMPT_NOTE);
    prompt.push_str("\n\n验证命令输出：\n");
    prompt.push_str(&text);
    (Ok(OutboundPreparation::Materialized(prompt, names, count)), true)
}

fn reset_acp_fallback(full_prompt: &str) -> (String, String) {
    (String::new(), full_prompt.to_string())
}

fn prepare_outbound(
    node: &DispatchNodeRequest,
    redact: bool,
    workdir: &Path,
    blind: bool,
) -> Result<(String, Vec<String>, usize), String> {
    let (mut prompt, mut count) = redact_text(&node.prompt, redact);
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
        if !node.upstream_artifacts.is_empty() {
            let (request, hits) = redact_text(&node.original_request, redact);
            count += hits;
            fs::write(workdir.join("REQUEST.md"), request)
                .map_err(|error| format!("写入 REQUEST.md 失败：{error}"))?;
            let diffs = node
                .upstream_artifacts
                .iter()
                .map(|artifact| artifact.diff.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let (diffs, hits) = redact_text(&diffs, redact);
            count += hits;
            fs::write(workdir.join("diff.patch"), &diffs)
                .map_err(|error| format!("写入 diff.patch 失败：{error}"))?;
            let changed = workdir.join("changed");
            fs::create_dir_all(&changed)
                .map_err(|error| format!("创建 changed 目录失败：{error}"))?;
            for file in node
                .upstream_artifacts
                .iter()
                .flat_map(|artifact| artifact.changed_files.iter())
            {
                let relative = Path::new(&file.path);
                if relative.is_absolute()
                    || relative
                        .components()
                        .any(|part| matches!(part, std::path::Component::Prefix(_) | std::path::Component::ParentDir))
                {
                    return Err(format!("产物路径不是安全相对路径：{}", file.path));
                }
                let target = changed.join(relative);
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|error| format!("创建产物目录失败：{error}"))?;
                }
                let (content, hits) = redact_text(&file.content, redact);
                count += hits;
                fs::write(&target, content)
                    .map_err(|error| format!("写入产物文件失败 {}：{error}", target.display()))?;
            }
            names.extend(["REQUEST.md".into(), "diff.patch".into(), "changed/".into()]);
            prompt.push_str("\n\n");
            prompt.push_str(channels::BLIND_ARTIFACT_NOTICE);
            if diffs.len() < 30 * 1024 {
                prompt.push_str("\n\n[diff.patch 全文]\n");
                prompt.push_str(&diffs);
            }
        }
        let mut used = HashSet::new();
        let mut inline = Vec::new();
        let mut inline_bytes = 0_usize;
        let mut all_text = true;
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
                inline_bytes += safe.len();
                inline.push((name.to_string(), safe));
            } else {
                all_text = false;
                fs::write(&target, bytes)
                    .map_err(|error| format!("复制附件失败 {}：{error}", target.display()))?;
            }
            names.push(name.to_string());
        }
        if node.upstream_artifacts.is_empty() && all_text && inline_bytes < 30 * 1024 {
            for (name, text) in inline {
                prompt.push_str(&format!("\n\n[附件全文 {name}]\n{text}"));
            }
            if channels::normalize_agent(&node.agent) == "codex" && !node.materials.is_empty() {
                prompt.push_str("\n\n材料已在正文，勿再用工具读取。");
            }
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
    redact_text(&strip_ansi(text), true).0
}

fn validation_failure(code: Option<i32>, output: &str) -> String {
    let exit = code.map_or_else(|| "未知".into(), |value| value.to_string());
    format!("验证命令失败（exit={exit}）\n{}", strip_ansi(output))
}

fn annotate_kimi_metadata(agent: &str, output: &mut ChannelOutput, runner_note: &mut String) {
    if agent != "kimi" { return; }
    if output.identity.is_empty() {
        output.identity = "kimi 不报真身".into();
    }
    if output.usage.input == 0 && output.usage.output == 0 && output.usage.cache_read == 0 {
        *runner_note = if runner_note.is_empty() { "kimi 不报用量".into() } else { format!("{runner_note}
kimi 不报用量") };
    }
}

fn append_runner_note(target: &mut String, note: &str) {
    if note.is_empty() { return; }
    if !target.is_empty() { target.push_str("；"); }
    target.push_str(note);
}

fn strip_ansi(text: &str) -> String {
    static ANSI: OnceLock<Regex> = OnceLock::new();
    ANSI.get_or_init(|| Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]").expect("ansi regex"))
        .replace_all(text, "")
        .into_owned()
}

fn broad_redaction_count(text: &str) -> usize {
    static BROAD: OnceLock<Regex> = OnceLock::new();
    BROAD
        .get_or_init(|| {
            Regex::new(
                r#"(?im)^\s*(?:password|passwd|pwd|token|api[_-]?key|secret[_-]?key|access[_-]?key)\s*[:=]\s*[^"'\r\n]+$"#,
            )
            .expect("broad redaction regex")
        })
        .find_iter(text)
        .count()
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
    if value.trim().is_empty() {
        return Err("请先在左栏「工作区」选择项目目录".into());
    }
    let path = PathBuf::from(value);
    if !path.is_dir() {
        return Err(format!("项目目录不存在：{}", path.display()));
    }
    Ok(path)
}

fn ensure_log_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|error| format!("创建派发记录目录失败：{error}"))?;
    let ignore = path.join(".gitignore");
    if !ignore.exists() {
        fs::write(&ignore, "*\n")
            .map_err(|error| format!("写入 .dispatch/.gitignore 失败：{error}"))?;
    }
    Ok(())
}

fn serialize_frontmatter(record: &DispatchRecord) -> Result<String, String> {
    let yaml = serde_yaml::to_string(record).map_err(|error| format!("序列化派发记录失败：{error}"))?;
    Ok(format!("---\n{yaml}---"))
}

fn write_record(
    log_dir: &Path,
    record: &DispatchRecord,
    frontmatter: &str,
    prompt: &str,
    reply_text: &str,
    artifacts: Option<&DispatchArtifacts>,
    stdout: &str,
    stderr: &str,
) -> Result<String, String> {
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let base = format!(
        "{}-{}-{}-{}",
        stamp,
        if record.agent.trim().is_empty() { "unspecified".into() } else { safe_name(&record.agent) },
        safe_name(&record.model),
        safe_name(&record.role)
    );
    let mut path = log_dir.join(format!("{base}.md"));
    let mut suffix = 2;
    while path.exists() {
        path = log_dir.join(format!("{base}-{suffix}.md"));
        suffix += 1;
    }
    let artifacts_json = serde_json::to_string(&artifacts)
        .map_err(|error| format!("序列化派发产物失败：{error}"))?;
    let mut body = format!("{frontmatter}\n\n## 任务全文\n\n{prompt}\n\n## 回复正文\n\n{}\n\n## 产物 JSON\n\n{artifacts_json}\n\n## 原始输出\n\n{stdout}\n", safe_trace(reply_text));
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
    let mut active_node_ids = status
        .iter()
        .filter(|(_, state)| matches!(state.as_str(), "运行中" | "排队" | "等待写锁"))
        .map(|(node, _)| node.clone())
        .collect::<Vec<_>>();
    active_node_ids.sort();
    let header = DispatchStateHeader {
        schema: "aicross-state/1".into(),
        run_id: run_id.into(),
        stage,
        unfinished: !active_node_ids.is_empty(),
        active_node_ids,
    };
    let mut body = format!(
        "aicross-state: {}\n## 当前状态\n阶段: 派发第 {stage} 阶段 ｜ run: {run_id} ｜ 更新: {}\n## 已完成\n",
        serde_json::to_string(&header).map_err(|error| format!("生成状态头失败：{error}"))?,
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
        .filter(|(_, state)| *state == "运行中" || *state == "排队" || *state == "等待写锁")
    {
        body.push_str(&format!("- [ ] {node}: {state}\n"));
    }
    body.push_str("## 验证命令未通过\n");
    for (node, state) in entries.iter().filter(|(_, state)| *state == "验证命令未通过") {
        let file = by_id
            .get(node.as_str())
            .map(|result| result.dispatch_file.as_str())
            .unwrap_or("");
        body.push_str(&format!("- [ ] {node}: {state} → {file}\n"));
    }
    body.push_str("## 额度用尽\n");
    for (node, state) in entries.iter().filter(|(_, state)| *state == "额度用尽") {
        let file = by_id
            .get(node.as_str())
            .map(|result| result.dispatch_file.as_str())
            .unwrap_or("");
        body.push_str(&format!("- [ ] {node}: {state} → {file}\n"));
    }
    body.push_str("## 失败与超时\n");
    for (node, state) in entries
        .iter()
        .filter(|(_, state)| *state == "失败" || *state == "超时" || *state == "真身不符")
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
    if status.values().all(|state| matches!(state.as_str(), "完成" | "额度用尽")) {
        body.push_str("- 查看并排结果，采纳、重派、升级或交人裁决\n");
    } else if status
        .values()
        .any(|state| state == "运行中" || state == "排队" || state == "等待写锁")
    {
        body.push_str("- 等待本阶段各路完成\n");
    } else {
        body.push_str("- 检查失败原因后重派或更换通道\n");
    }
    write_atomic(path, body.as_bytes())
}

// JSON 持久化的同类实现见 `orchestration::write_json_atomic`；统一收敛留待第 5 期。
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
                elapsed_ms: None,
                idle_ms: None,
                tool_calls: None,
                repeated_tool_calls: None,
                last_tool: None,
            },
        );
    }
}

fn emit_running_progress(app: &Option<AppHandle>, run_id: &str, node_id: &str, activity: &ProcessActivity) {
    if let Some(app) = app {
        let _ = app.emit("dispatch-progress", running_progress(run_id, node_id, activity));
    }
}

fn heartbeat_loop<F>(stop_rx: mpsc::Receiver<()>, interval: Duration, mut emit: F)
where
    F: FnMut(),
{
    loop {
        match stop_rx.recv_timeout(interval) {
            Err(mpsc::RecvTimeoutError::Timeout) => emit(),
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn running_progress(run_id: &str, node_id: &str, activity: &ProcessActivity) -> DispatchProgress {
    let (elapsed_ms, idle_ms, tool_calls, repeated_tool_calls, last_tool) = activity.snapshot();
    let last_tool = safe_trace(&last_tool).chars().take(120).collect();
    DispatchProgress {
        run_id: run_id.into(), node_id: node_id.into(), status: "running".into(),
        elapsed_ms: Some(elapsed_ms), idle_ms: Some(idle_ms), tool_calls: Some(tool_calls),
        repeated_tool_calls: Some(repeated_tool_calls), last_tool: Some(last_tool),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_available() -> bool {
        std::process::Command::new("git").arg("--version").output().is_ok_and(|output| output.status.success())
    }

    fn require_git(test: &str) -> bool {
        if git_available() { true } else { eprintln!("skipping {test}: git 不存在"); false }
    }

    fn test_git(cwd: &Path, args: &[&str]) -> String {
        let result = git_result(args, cwd, None).unwrap();
        assert!(process_succeeded(&result), "git {args:?} failed: {}", process_note(&result, ""));
        result.stdout.trim().to_string()
    }

    fn swarm_repository(tag: &str) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("aicross-swarm-{tag}-{}", Uuid::new_v4()));
        let project = root.join("project");
        let first = root.join("item-1");
        let second = root.join("item-2");
        fs::create_dir_all(&project).unwrap();
        test_git(&project, &["init"]);
        test_git(&project, &["config", "user.name", "aicross test"]);
        test_git(&project, &["config", "user.email", "aicross@example.invalid"]);
        fs::write(project.join("shared.txt"), "base\n").unwrap();
        test_git(&project, &["add", "shared.txt"]);
        test_git(&project, &["commit", "-m", "base"]);
        let first_arg = first.to_string_lossy().into_owned();
        let second_arg = second.to_string_lossy().into_owned();
        test_git(&project, &["worktree", "add", "-b", "swarm-item-1", &first_arg, "HEAD"]);
        test_git(&project, &["worktree", "add", "-b", "swarm-item-2", &second_arg, "HEAD"]);
        (root, project, first, second)
    }

    fn merge_item(node_id: &str, item: u32, worktree: &Path, accept: &str) -> SwarmMergeItem {
        SwarmMergeItem {
            node_id: node_id.into(),
            item,
            worktree: worktree.to_string_lossy().into_owned(),
            files: vec!["**".into()],
            title: String::new(),
            accept: accept.into(),
        }
    }

    #[test]
    fn task08a_swarm_plan_reports_overlap_unmatched_and_dirty_tree() {
        if !require_git("task08a_swarm_plan_reports_overlap_unmatched_and_dirty_tree") { return; }
        let (root, project, _, _) = swarm_repository("plan");
        fs::create_dir_all(project.join("src/nested")).unwrap();
        fs::write(project.join("src/a.rs"), "a").unwrap();
        fs::write(project.join("src/nested/b.rs"), "b").unwrap();
        test_git(&project, &["add", "src"]);
        test_git(&project, &["commit", "-m", "sources"]);
        let clean = check_swarm_plan_core(&project, &[
            SwarmPlanItem { item: 1, files: vec!["./src/".into()] },
            SwarmPlanItem { item: 2, files: vec!["SRC/a.rs".into()] },
            SwarmPlanItem { item: 3, files: vec!["docs/**".into()] },
            SwarmPlanItem { item: 4, files: vec!["shared.txt".into()] },
        ], 4).unwrap();
        assert!(!clean.dirty);
        assert_eq!(clean.overlaps, [SwarmOverlap { a: 1, b: 2, path: "src/a.rs".into() }]);
        assert_eq!(clean.unmatched, [SwarmUnmatched { item: 3, glob: "docs/**".into() }]);
        let limited = check_swarm_plan_core(&project, &[
            SwarmPlanItem { item: 1, files: vec!["[broken".into()] },
            SwarmPlanItem { item: 2, files: vec!["shared.txt".into()] },
        ], 1).unwrap();
        assert_eq!(limited.unmatched, [SwarmUnmatched { item: 1, glob: "[broken".into() }]);
        assert!(limited.note.contains("glob") && limited.note.contains("仅检查前 1 件"));
        fs::write(project.join("shared.txt"), "dirty\n").unwrap();
        assert!(check_swarm_plan_core(&project, &[], 4).unwrap().dirty);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08a_r1_split_role_is_rejected_when_not_readonly() {
        let root = std::env::temp_dir().join(format!("aicross-split-guard-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let mut split = node(Vec::new());
        split.role = "拆分".into();
        split.write = "worktree".into();
        let result = dispatch_stage_core(stage_request(root.to_string_lossy().into_owned(), vec![split]), None).unwrap();
        assert_eq!(result.results[0].status, "error");
        assert!(result.results[0].error.contains("拆分") && result.results[0].error.contains("只读"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08a_merge_two_worktrees_in_item_order_with_one_commit_each() {
        if !require_git("task08a_merge_two_worktrees_in_item_order_with_one_commit_each") { return; }
        let (root, project, first, second) = swarm_repository("success");
        fs::write(first.join("one.txt"), "one\n").unwrap();
        fs::write(second.join("two.txt"), "two\n").unwrap();
        let results = merge_swarm_items_core(&project, "run-success", vec![
            merge_item("second", 2, &second, ""),
            merge_item("first", 1, &first, ""),
        ], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results.iter().map(|result| result.item).collect::<Vec<_>>(), [1, 2]);
        assert!(results.iter().all(|result| result.result == "merged"));
        assert!(project.join("one.txt").exists() && project.join("two.txt").exists());
        assert_eq!(test_git(&project, &["rev-list", "--count", "HEAD"]), "3");
        assert!(test_git(&project, &["log", "-2", "--format=%s"]).lines().next().unwrap().contains("item 2"));
        let records = fs::read_dir(project.join(".dispatch"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("md"))
            .map(|entry| parse_record_file(&entry.path()).unwrap().0)
            .collect::<Vec<_>>();
        assert_eq!(records.len(), 2);
        assert!(records.iter().all(|record| record.swarm_item.is_some() && record.merge_result == "merged"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08d_in_range_conflict_restores_clean_project_and_keeps_first_item() {
        if !require_git("task08d_in_range_conflict_restores_clean_project_and_keeps_first_item") { return; }
        let (root, project, first, second) = swarm_repository("conflict");
        fs::write(first.join("shared.txt"), "first\n").unwrap();
        fs::write(second.join("shared.txt"), "second\n").unwrap();
        let results = merge_swarm_items_core(&project, "run-conflict", vec![
            merge_item("first", 1, &first, ""),
            merge_item("second", 2, &second, ""),
        ], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results.iter().map(|result| result.result.as_str()).collect::<Vec<_>>(), ["merged", "conflict"], "{results:?}");
        assert!(results[1].note.contains("shared.txt") && results[1].note.contains(&second.to_string_lossy().to_string()));
        assert_eq!(fs::read_to_string(project.join("shared.txt")).unwrap(), "first\n");
        assert_eq!(test_git(&project, &["rev-list", "--count", "HEAD"]), "2");
        assert!(test_git(&project, &["status", "--porcelain", "--untracked-files=all"]).is_empty());
        assert!(test_git(&project, &["ls-files", "-u"]).is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08a_failed_validation_resets_only_the_failed_item() {
        if !require_git("task08a_failed_validation_resets_only_the_failed_item") { return; }
        let (root, project, first, second) = swarm_repository("validation");
        fs::write(first.join("one.txt"), "one\n").unwrap();
        fs::write(second.join("two.txt"), "two\n").unwrap();
        let results = merge_swarm_items_core(&project, "run-validation", vec![
            merge_item("first", 1, &first, "exit 0"),
            merge_item("second", 2, &second, ""),
        ], "exit 9", &DispatchSettings::default()).unwrap();
        assert_eq!(results.iter().map(|result| result.result.as_str()).collect::<Vec<_>>(), ["merged", "test_failed"]);
        assert!(project.join("one.txt").exists());
        assert!(!project.join("two.txt").exists());
        assert_eq!(test_git(&project, &["rev-list", "--count", "HEAD"]), "2");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08a_dirty_project_skips_entire_merge_batch() {
        if !require_git("task08a_dirty_project_skips_entire_merge_batch") { return; }
        let (root, project, first, second) = swarm_repository("dirty");
        fs::write(project.join("shared.txt"), "dirty\n").unwrap();
        let results = merge_swarm_items_core(&project, "run-dirty", vec![
            merge_item("first", 1, &first, ""),
            merge_item("second", 2, &second, ""),
        ], "", &DispatchSettings::default()).unwrap();
        assert!(results.iter().all(|result| result.result == "skipped"));
        assert!(results.iter().all(|result| result.note.contains("未提交改动")));
        assert_eq!(test_git(&project, &["rev-list", "--count", "HEAD"]), "1");
        assert_eq!(fs::read_dir(project.join(".dispatch")).unwrap().filter_map(Result::ok)
            .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("md")).count(), 2);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08a_r1_agent_committed_worktree_head_is_merged() {
        if !require_git("task08a_r1_agent_committed_worktree_head_is_merged") { return; }
        let (root, project, first, _) = swarm_repository("committed");
        fs::write(first.join("committed.txt"), "agent commit\n").unwrap();
        test_git(&first, &["add", "committed.txt"]);
        test_git(&first, &["commit", "-m", "agent committed"]);
        let results = merge_swarm_items_core(&project, "run-committed", vec![merge_item("first", 1, &first, "")], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results[0].result, "merged");
        assert_eq!(fs::read_to_string(project.join("committed.txt")).unwrap(), "agent commit\n");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08d_apply_timeout_is_skipped_and_later_item_continues() {
        if !require_git("task08d_apply_timeout_is_skipped_and_later_item_continues") { return; }
        let (root, project, first, second) = swarm_repository("timeout-continues");
        fs::write(first.join("one.txt"), "one\n").unwrap();
        fs::write(second.join("two.txt"), "two\n").unwrap();
        SWARM_TEST_TIMEOUT_NEXT_APPLY.with(|flag| flag.set(true));
        let results = merge_swarm_items_core(&project, "run-timeout", vec![
            merge_item("one", 1, &first, ""),
            merge_item("two", 2, &second, ""),
        ], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results.iter().map(|result| result.result.as_str()).collect::<Vec<_>>(), ["skipped", "merged"]);
        assert!(results[0].note.contains("命令超时"));
        assert!(!project.join("one.txt").exists() && project.join("two.txt").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08d_restore_failure_stops_batch_with_manual_recovery_guidance() {
        if !require_git("task08d_restore_failure_stops_batch_with_manual_recovery_guidance") { return; }
        let (root, project, first, second) = swarm_repository("abort-failure");
        fs::write(project.join("shared.txt"), "project\n").unwrap();
        test_git(&project, &["add", "shared.txt"]);
        test_git(&project, &["commit", "-m", "project change"]);
        fs::write(first.join("shared.txt"), "first\n").unwrap();
        fs::write(second.join("later.txt"), "later\n").unwrap();
        SWARM_TEST_FAIL_NEXT_RESTORE.with(|flag| flag.set(true));
        let results = merge_swarm_items_core(&project, "run-abort-failure", vec![
            merge_item("conflict", 1, &first, ""),
            merge_item("later", 2, &second, ""),
        ], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results.iter().map(|result| result.result.as_str()).collect::<Vec<_>>(), ["skipped", "skipped"], "{results:?}");
        assert!(results.iter().all(|result| result.note.contains("手动恢复工作区与索引")));
        let _ = fs::remove_dir_all(root);
    }

    fn track_in_project_and_worktrees(project: &Path, worktrees: &[&Path], path: &str, content: &[u8]) {
        let target = project.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, content).unwrap();
        test_git(project, &["add", "--", path]);
        test_git(project, &["commit", "-m", "tracked fixture"]);
        let head = test_git(project, &["rev-parse", "HEAD"]);
        for worktree in worktrees {
            test_git(worktree, &["merge", "--ff-only", &head]);
        }
    }

    #[test]
    fn task08d_disjoint_ranges_sharing_tracked_pyc_both_merge() {
        if !require_git("task08d_disjoint_ranges_sharing_tracked_pyc_both_merge") { return; }
        let (root, project, first, second) = swarm_repository("disjoint-pyc");
        let pyc = "src/__pycache__/__init__.cpython-312.pyc";
        track_in_project_and_worktrees(&project, &[&first, &second], pyc, b"base-pyc");
        fs::write(first.join(pyc), b"first-pyc").unwrap();
        fs::write(second.join(pyc), b"second-pyc").unwrap();
        fs::write(first.join("one.txt"), "one\n").unwrap();
        fs::write(second.join("two.txt"), "two\n").unwrap();
        let mut one = merge_item("one", 1, &first, "");
        one.files = vec!["one.txt".into()];
        let mut two = merge_item("two", 2, &second, "");
        two.files = vec!["two.txt".into()];
        let results = merge_swarm_items_core(&project, "run-disjoint-pyc", vec![one, two], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results.iter().map(|result| result.result.as_str()).collect::<Vec<_>>(), ["merged", "merged"], "{results:?}");
        assert_eq!(fs::read(project.join(pyc)).unwrap(), b"base-pyc");
        assert!(project.join("one.txt").exists() && project.join("two.txt").exists());
        assert!(results.iter().all(|result| result.note.contains(pyc)));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08d_out_of_range_and_excluded_files_are_not_applied() {
        if !require_git("task08d_out_of_range_and_excluded_files_are_not_applied") { return; }
        let (root, project, first, second) = swarm_repository("scope-excludes");
        let pyc = "src/__pycache__/tracked.pyc";
        track_in_project_and_worktrees(&project, &[&first, &second], pyc, b"base-pyc");
        fs::create_dir_all(first.join("src/feature")).unwrap();
        fs::write(first.join("src/feature/new.rs"), "new\n").unwrap();
        fs::write(first.join("shared.txt"), "outside\n").unwrap();
        fs::write(first.join(pyc), b"garbage").unwrap();
        let mut item = merge_item("scope", 1, &first, "");
        item.files = vec!["src/**".into()];
        let results = merge_swarm_items_core(&project, "run-scope-excludes", vec![item], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results[0].result, "merged", "{results:?}");
        assert!(project.join("src/feature/new.rs").exists());
        assert_eq!(fs::read_to_string(project.join("shared.txt")).unwrap(), "base\n");
        assert_eq!(fs::read(project.join(pyc)).unwrap(), b"base-pyc");
        assert!(results[0].note.contains("shared.txt") && results[0].note.contains(pyc));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08d_r1_file_declaration_finds_new_file_in_untracked_directory() {
        if !require_git("task08d_r1_file_declaration_finds_new_file_in_untracked_directory") { return; }
        let (root, project, first, _) = swarm_repository("r1-untracked-file");
        fs::create_dir_all(first.join("src/feature")).unwrap();
        fs::write(first.join("src/feature/new.rs"), "new\n").unwrap();
        let mut item = merge_item("new-file", 1, &first, "");
        item.files = vec!["src/feature/new.rs".into()];
        let results = merge_swarm_items_core(&project, "run-r1-untracked-file", vec![item], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results[0].result, "merged", "{results:?}");
        assert_eq!(fs::read_to_string(project.join("src/feature/new.rs")).unwrap(), "new\n");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08d_r1_directory_declaration_excludes_nested_cache_from_patch() {
        if !require_git("task08d_r1_directory_declaration_excludes_nested_cache_from_patch") { return; }
        let (root, project, first, _) = swarm_repository("r1-untracked-cache");
        fs::create_dir_all(first.join("src/feature/__pycache__")).unwrap();
        fs::write(first.join("src/feature/new.py"), "print('ok')\n").unwrap();
        fs::write(first.join("src/feature/__pycache__/new.pyc"), b"cache").unwrap();
        let mut item = merge_item("directory", 1, &first, "");
        item.files = vec!["src/feature".into()];
        let results = merge_swarm_items_core(&project, "run-r1-untracked-cache", vec![item], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results[0].result, "merged", "{results:?}");
        assert!(project.join("src/feature/new.py").exists());
        assert!(!project.join("src/feature/__pycache__/new.pyc").exists());
        assert!(results[0].note.contains("__pycache__"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08d_r1_non_conflict_apply_failure_is_skipped() {
        if !require_git("task08d_r1_non_conflict_apply_failure_is_skipped") { return; }
        let (root, project, first, second) = swarm_repository("r1-binary-failure");
        track_in_project_and_worktrees(&project, &[&first, &second], "image.bin", &[0_u8, 1, 2, 3, 4]);
        fs::write(first.join("image.bin"), [0_u8, 5, 6, 7, 8]).unwrap();
        test_git(&project, &["rm", "image.bin"]);
        fs::create_dir_all(project.join("image.bin")).unwrap();
        fs::write(project.join("image.bin/child.txt"), "replacement\n").unwrap();
        test_git(&project, &["add", "image.bin/child.txt"]);
        test_git(&project, &["commit", "-m", "replace binary with directory"]);
        let mut item = merge_item("binary", 1, &first, "");
        item.files = vec!["image.bin".into()];
        let results = merge_swarm_items_core(&project, "run-r1-binary-failure", vec![item], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results[0].result, "skipped", "{results:?}");
        assert!(results[0].note.contains("应用声明范围补丁失败"));
        assert!(!results[0].note.contains("文件冲突"));
        assert!(test_git(&project, &["ls-files", "-u"]).is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08d_r1_repeated_item_is_idempotently_skipped() {
        if !require_git("task08d_r1_repeated_item_is_idempotently_skipped") { return; }
        let (root, project, first, _) = swarm_repository("r1-idempotent");
        fs::write(first.join("once.txt"), "once\n").unwrap();
        let item = merge_item("once", 1, &first, "");
        let first_result = merge_swarm_items_core(&project, "run-r1-idempotent-1", vec![item.clone()], "", &DispatchSettings::default()).unwrap();
        let second_result = merge_swarm_items_core(&project, "run-r1-idempotent-2", vec![item], "", &DispatchSettings::default()).unwrap();
        assert_eq!(first_result[0].result, "merged", "{first_result:?}");
        assert_eq!(second_result[0].result, "skipped", "{second_result:?}");
        assert!(second_result[0].note.contains("范围内改动已在项目中"), "{second_result:?}");
        assert_eq!(test_git(&project, &["rev-list", "--count", "HEAD"]), "2");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08d_r1_conflict_cleans_new_file_and_later_item_merges() {
        if !require_git("task08d_r1_conflict_cleans_new_file_and_later_item_merges") { return; }
        let (root, project, first, second) = swarm_repository("r1-conflict-new");
        let third = root.join("item-3");
        let third_arg = third.to_string_lossy().into_owned();
        test_git(&project, &["worktree", "add", "-b", "swarm-item-3", &third_arg, "HEAD"]);
        fs::write(first.join("shared.txt"), "first\n").unwrap();
        fs::write(second.join("shared.txt"), "second\n").unwrap();
        fs::write(second.join("new.txt"), "must be cleaned\n").unwrap();
        fs::write(third.join("third.txt"), "third\n").unwrap();
        let results = merge_swarm_items_core(&project, "run-r1-conflict-new", vec![
            merge_item("first", 1, &first, ""),
            merge_item("second", 2, &second, ""),
            merge_item("third", 3, &third, ""),
        ], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results.iter().map(|result| result.result.as_str()).collect::<Vec<_>>(), ["merged", "conflict", "merged"], "{results:?}");
        assert!(results[1].note.contains("shared.txt") && results[1].note.contains(&second.to_string_lossy().to_string()));
        assert!(!project.join("new.txt").exists());
        assert!(project.join("third.txt").exists());
        assert!(test_git(&project, &["status", "--porcelain", "--untracked-files=all"]).is_empty());
        let fourth = root.join("item-4");
        let fourth_arg = fourth.to_string_lossy().into_owned();
        test_git(&project, &["worktree", "add", "-b", "swarm-item-4", &fourth_arg, "HEAD"]);
        fs::write(fourth.join("fourth.txt"), "fourth\n").unwrap();
        let next = merge_swarm_items_core(&project, "run-r1-after-conflict", vec![merge_item("fourth", 4, &fourth, "")], "", &DispatchSettings::default()).unwrap();
        assert_eq!(next[0].result, "merged", "{next:?}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08d_r1_status_parser_consumes_worktree_rename_origin() {
        if !require_git("task08d_r1_status_parser_consumes_worktree_rename_origin") { return; }
        let (root, project, first, second) = swarm_repository("r1-rename");
        track_in_project_and_worktrees(&project, &[&first, &second], "old.txt", b"old\n");
        test_git(&first, &["mv", "old.txt", "new.txt"]);
        let status = git_result(&["status", "--porcelain=v1", "-z", "--untracked-files=all"], &first, None).unwrap();
        let paths = parse_git_status_paths(&status.stdout);
        assert!(paths.iter().any(|(path, _)| path == "new.txt"), "{paths:?}");
        assert!(paths.iter().all(|(path, _)| path != "old.txt" && !path.starts_with('/')), "{paths:?}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08d_r1_truncated_changed_status_skips_item() {
        if !require_git("task08d_r1_truncated_changed_status_skips_item") { return; }
        let (root, project, first, _) = swarm_repository("r1-truncated-status");
        fs::write(first.join("one.txt"), "one\n").unwrap();
        SWARM_TEST_TRUNCATE_NEXT_CHANGED_STATUS.with(|flag| flag.set(true));
        let results = merge_swarm_items_core(&project, "run-r1-truncated-status", vec![merge_item("one", 1, &first, "")], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results[0].result, "skipped", "{results:?}");
        assert!(results[0].note.contains("截断"));
        assert!(!project.join("one.txt").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08d_r1_outside_note_is_bounded() {
        if !require_git("task08d_r1_outside_note_is_bounded") { return; }
        let (root, project, first, _) = swarm_repository("r1-bounded-note");
        fs::write(first.join("inside.txt"), "inside\n").unwrap();
        for index in 0..25 {
            fs::write(first.join(format!("outside-{index:02}.txt")), "outside\n").unwrap();
        }
        let mut item = merge_item("bounded", 1, &first, "");
        item.files = vec!["inside.txt".into()];
        let results = merge_swarm_items_core(&project, "run-r1-bounded-note", vec![item], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results[0].result, "merged", "{results:?}");
        assert!(results[0].note.contains("等 25 项"), "{}", results[0].note);
        assert!(!results[0].note.contains("outside-24.txt"), "{}", results[0].note);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08d_r1_large_binary_patch_is_fully_applied() {
        if !require_git("task08d_r1_large_binary_patch_is_fully_applied") { return; }
        let (root, project, first, _) = swarm_repository("r1-large-patch");
        let mut state = 0x1234_5678_u32;
        let bytes = (0..5 * 1024 * 1024).map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 24) as u8
        }).collect::<Vec<_>>();
        fs::write(first.join("large.bin"), &bytes).unwrap();
        let mut item = merge_item("large", 1, &first, "");
        item.files = vec!["large.bin".into()];
        let results = merge_swarm_items_core(&project, "run-r1-large-patch", vec![item], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results[0].result, "merged", "{results:?}");
        assert_eq!(fs::read(project.join("large.bin")).unwrap(), bytes);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08d_r1_non_utf8_file_bytes_are_preserved() {
        if !require_git("task08d_r1_non_utf8_file_bytes_are_preserved") { return; }
        let (root, project, first, _) = swarm_repository("r1-gbk");
        let gbk = [0xd6, 0xd0, 0xce, 0xc4, b'\n'];
        fs::write(first.join("gbk.txt"), gbk).unwrap();
        let mut item = merge_item("gbk", 1, &first, "");
        item.files = vec!["gbk.txt".into()];
        let results = merge_swarm_items_core(&project, "run-r1-gbk", vec![item], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results[0].result, "merged", "{results:?}");
        assert_eq!(fs::read(project.join("gbk.txt")).unwrap(), gbk);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08d_r1_dirty_between_items_stops_without_losing_user_edit() {
        if !require_git("task08d_r1_dirty_between_items_stops_without_losing_user_edit") { return; }
        let (root, project, first, second) = swarm_repository("r1-mid-batch-dirty");
        fs::write(first.join("one.txt"), "one\n").unwrap();
        fs::write(second.join("two.txt"), "two\n").unwrap();
        SWARM_TEST_DIRTY_BEFORE_ITEM.with(|target| target.set(2));
        let results = merge_swarm_items_core(&project, "run-r1-mid-batch-dirty", vec![
            merge_item("one", 1, &first, ""),
            merge_item("two", 2, &second, ""),
        ], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results.iter().map(|result| result.result.as_str()).collect::<Vec<_>>(), ["merged", "skipped"], "{results:?}");
        assert_eq!(fs::read_to_string(project.join("shared.txt")).unwrap(), "user edit during merge\n");
        assert!(project.join("one.txt").exists() && !project.join("two.txt").exists());
        assert!(results[1].note.contains("合并期间出现未提交改动"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08d_r1_invalid_unmatched_and_empty_ranges_have_distinct_notes() {
        if !require_git("task08d_r1_invalid_unmatched_and_empty_ranges_have_distinct_notes") { return; }
        let (root, project, first, second) = swarm_repository("r1-note-levels");
        fs::write(first.join("changed.txt"), "changed\n").unwrap();
        let mut invalid = merge_item("invalid", 1, &first, "");
        invalid.files = vec!["[broken".into()];
        let mut empty = merge_item("empty", 2, &second, "");
        empty.files = vec!["missing.txt".into()];
        let results = merge_swarm_items_core(&project, "run-r1-note-levels", vec![invalid, empty], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results.iter().map(|result| result.result.as_str()).collect::<Vec<_>>(), ["skipped", "skipped"], "{results:?}");
        assert!(results[0].note.contains("glob 无效：[broken") && results[0].note.contains("声明范围未匹配到改动"));
        assert!(results[1].note.contains("范围内没有改动") && !results[1].note.contains("未匹配到改动"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08d_r1_bare_directory_declaration_merges_full_deletion() {
        if !require_git("task08d_r1_bare_directory_declaration_merges_full_deletion") { return; }
        let (root, project, first, second) = swarm_repository("r1-directory-delete");
        track_in_project_and_worktrees(&project, &[&first, &second], "olddir/a.rs", b"a\n");
        track_in_project_and_worktrees(&project, &[&first, &second], "olddir/b.rs", b"b\n");
        fs::remove_dir_all(first.join("olddir")).unwrap();
        let mut item = merge_item("delete-dir", 1, &first, "");
        item.files = vec!["olddir".into()];
        let results = merge_swarm_items_core(&project, "run-r1-directory-delete", vec![item], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results[0].result, "merged", "{results:?}");
        assert!(!project.join("olddir").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08d_validation_cleans_tracked_excluded_changes() {
        if !require_git("task08d_validation_cleans_tracked_excluded_changes") { return; }
        let (root, project, first, second) = swarm_repository("validation-excluded");
        let pyc = "src/__pycache__/tracked.pyc";
        track_in_project_and_worktrees(&project, &[&first, &second], pyc, b"base-pyc");
        fs::write(first.join("one.txt"), "one\n").unwrap();
        let mut item = merge_item("clean", 1, &first, "");
        item.files = vec!["one.txt".into()];
        #[cfg(windows)]
        { item.accept = "[IO.File]::WriteAllBytes('src/__pycache__/tracked.pyc',[byte[]](1,2,3))".into(); }
        #[cfg(not(windows))]
        { item.accept = "printf dirty > src/__pycache__/tracked.pyc".into(); }
        let results = merge_swarm_items_core(&project, "run-validation-excluded", vec![item], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results[0].result, "merged", "{results:?}");
        assert_eq!(fs::read(project.join(pyc)).unwrap(), b"base-pyc");
        assert!(test_git(&project, &["status", "--porcelain", "--untracked-files=all"]).is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08d_validation_project_change_fails_and_rolls_back_item() {
        if !require_git("task08d_validation_project_change_fails_and_rolls_back_item") { return; }
        let (root, project, first, _) = swarm_repository("validation-project-change");
        fs::write(first.join("one.txt"), "one\n").unwrap();
        let mut item = merge_item("dirty-validation", 1, &first, "");
        item.files = vec!["one.txt".into()];
        #[cfg(windows)]
        { item.accept = "[IO.File]::WriteAllText('shared.txt','validation dirty')".into(); }
        #[cfg(not(windows))]
        { item.accept = "printf 'validation dirty' > shared.txt".into(); }
        let results = merge_swarm_items_core(&project, "run-validation-project-change", vec![item], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results[0].result, "test_failed", "{results:?}");
        assert!(results[0].note.contains("验证命令改动了项目文件：shared.txt"));
        assert!(!project.join("one.txt").exists());
        assert_eq!(fs::read_to_string(project.join("shared.txt")).unwrap(), "base\n");
        assert!(test_git(&project, &["status", "--porcelain", "--untracked-files=all"]).is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08d_r1_validation_red_with_cache_residue_stays_test_failed() {
        if !require_git("task08d_r1_validation_red_with_cache_residue_stays_test_failed") { return; }
        let (root, project, first, second) = swarm_repository("validation-cache-residue");
        fs::write(first.join("one.txt"), "one
").unwrap();
        let mut item = merge_item("cache-residue", 1, &first, "");
        item.files = vec!["one.txt".into()];
        #[cfg(windows)]
        { item.accept = "New-Item -ItemType Directory -Force __pycache__ | Out-Null; [IO.File]::WriteAllText('__pycache__/m.pyc','x'); exit 9".into(); }
        #[cfg(not(windows))]
        { item.accept = "mkdir -p __pycache__ && printf x > __pycache__/m.pyc && exit 9".into(); }
        fs::write(second.join("two.txt"), "two
").unwrap();
        let mut later = merge_item("after-residue", 2, &second, "");
        later.files = vec!["two.txt".into()];
        let results = merge_swarm_items_core(&project, "run-validation-cache-residue", vec![item, later], "", &DispatchSettings::default()).unwrap();
        // 验证红只回退本件，排除目录里的验证残留不算恢复失败，后件照常并入
        assert_eq!(results.iter().map(|row| row.result.as_str()).collect::<Vec<_>>(), vec!["test_failed", "merged"], "{results:?}");
        assert!(!results[0].note.contains("手动恢复"), "{}", results[0].note);
        assert!(!project.join("one.txt").exists());
        assert!(project.join("two.txt").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08a_r2_failed_ahead_item_retries_all_commits_and_merges() {
        if !require_git("task08a_r2_failed_ahead_item_retries_all_commits_and_merges") { return; }
        let (root, project, first, _) = swarm_repository("retry-ahead");
        fs::write(first.join("one.txt"), "one\n").unwrap();
        test_git(&first, &["add", "one.txt"]);
        test_git(&first, &["commit", "-m", "agent first"]);
        fs::write(first.join("two.txt"), "two\n").unwrap();
        test_git(&first, &["add", "two.txt"]);
        test_git(&first, &["commit", "-m", "agent second"]);
        let failed = merge_swarm_items_core(&project, "run-retry-1", vec![merge_item("retry", 1, &first, "exit 9")], "", &DispatchSettings::default()).unwrap();
        assert_eq!(failed[0].result, "test_failed");
        assert_eq!(test_git(&project, &["rev-list", "--count", "HEAD"]), "1");
        assert!(!project.join("one.txt").exists() && !project.join("two.txt").exists());
        let merged = merge_swarm_items_core(&project, "run-retry-2", vec![merge_item("retry", 1, &first, "exit 0")], "", &DispatchSettings::default()).unwrap();
        assert_eq!(merged[0].result, "merged");
        assert_eq!(test_git(&project, &["rev-list", "--count", "HEAD"]), "2");
        assert!(project.join("one.txt").exists() && project.join("two.txt").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08a_r2_merge_commit_subject_contains_title() {
        if !require_git("task08a_r2_merge_commit_subject_contains_title") { return; }
        let (root, project, first, _) = swarm_repository("title");
        fs::write(first.join("title.txt"), "title\n").unwrap();
        let mut item = merge_item("node-title", 1, &first, "");
        item.title = "named title".into();
        let results = merge_swarm_items_core(&project, "run-title", vec![item], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results[0].result, "merged");
        assert!(test_git(&project, &["log", "-1", "--format=%s"]).contains("named title"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08a_r1_missing_unregistered_and_unchanged_worktrees_are_skipped() {
        if !require_git("task08a_r1_missing_unregistered_and_unchanged_worktrees_are_skipped") { return; }
        let (root, project, first, _) = swarm_repository("skip-paths");
        let foreign = root.join("foreign");
        fs::create_dir_all(&foreign).unwrap();
        test_git(&foreign, &["init"]);
        let missing = root.join("missing");
        let results = merge_swarm_items_core(&project, "run-skip", vec![
            merge_item("unchanged", 1, &first, ""),
            merge_item("missing", 2, &missing, ""),
            merge_item("foreign", 3, &foreign, ""),
        ], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results.iter().map(|result| result.result.as_str()).collect::<Vec<_>>(), ["skipped", "skipped", "skipped"]);
        assert!(results[0].note.contains("没有改动"));
        assert!(results[1].note.contains("不存在"));
        assert!(results[2].note.contains("未登记"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08a_r2_stale_registered_worktree_does_not_block_valid_item() {
        if !require_git("task08a_r2_stale_registered_worktree_does_not_block_valid_item") { return; }
        let (root, project, first, second) = swarm_repository("stale-registration");
        fs::remove_dir_all(&first).unwrap();
        fs::write(second.join("valid.txt"), "valid\n").unwrap();
        let results = merge_swarm_items_core(&project, "run-stale", vec![
            merge_item("stale", 1, &first, ""),
            merge_item("valid", 2, &second, ""),
        ], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results.iter().map(|result| result.result.as_str()).collect::<Vec<_>>(), ["skipped", "merged"]);
        assert!(results[0].note.contains("不存在") && project.join("valid.txt").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08a_r2_missing_project_head_is_skipped_not_reported_unchanged() {
        if !require_git("task08a_r2_missing_project_head_is_skipped_not_reported_unchanged") { return; }
        let (root, project, first, _) = swarm_repository("missing-head");
        SWARM_TEST_FAIL_PROJECT_HEAD.with(|flag| flag.set(true));
        let results = merge_swarm_items_core(&project, "run-missing-head", vec![merge_item("head", 1, &first, "")], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results[0].result, "skipped");
        assert!(results[0].note.contains("HEAD") && !results[0].note.contains("没有改动"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08a_r2_reset_failure_stops_batch_and_is_not_swallowed() {
        if !require_git("task08a_r2_reset_failure_stops_batch_and_is_not_swallowed") { return; }
        let (root, project, first, second) = swarm_repository("reset-failure");
        fs::write(first.join("one.txt"), "one\n").unwrap();
        fs::write(second.join("two.txt"), "two\n").unwrap();
        SWARM_TEST_FAIL_NEXT_RESET.with(|flag| flag.set(true));
        let results = merge_swarm_items_core(&project, "run-reset-failure", vec![
            merge_item("failed", 1, &first, "exit 9"),
            merge_item("later", 2, &second, "exit 0"),
        ], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results.iter().map(|result| result.result.as_str()).collect::<Vec<_>>(), ["skipped", "skipped"]);
        assert!(results[0].note.contains("回退失败") && results[0].note.contains("状态不确定"));
        assert!(results[1].note.contains("停止后续合并"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08a_r1_middle_validation_failure_does_not_stop_later_item() {
        if !require_git("task08a_r1_middle_validation_failure_does_not_stop_later_item") { return; }
        let (root, project, first, second) = swarm_repository("continue");
        let third = root.join("item-3");
        let third_arg = third.to_string_lossy().into_owned();
        test_git(&project, &["worktree", "add", "-b", "swarm-item-3", &third_arg, "HEAD"]);
        fs::write(first.join("one.txt"), "one\n").unwrap();
        fs::write(second.join("two.txt"), "two\n").unwrap();
        fs::write(third.join("three.txt"), "three\n").unwrap();
        let results = merge_swarm_items_core(&project, "run-continue", vec![
            merge_item("one", 1, &first, "exit 0"),
            merge_item("two", 2, &second, "exit 9"),
            merge_item("three", 3, &third, "exit 0"),
        ], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results.iter().map(|result| result.result.as_str()).collect::<Vec<_>>(), ["merged", "test_failed", "merged"]);
        assert!(project.join("one.txt").exists() && !project.join("two.txt").exists() && project.join("three.txt").exists());
        assert!(results[1].note.contains("验证命令未通过"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08a_r1_swarm_limit_processes_only_first_n_items() {
        if !require_git("task08a_r1_swarm_limit_processes_only_first_n_items") { return; }
        let (root, project, first, second) = swarm_repository("limit");
        fs::write(first.join("one.txt"), "one\n").unwrap();
        fs::write(second.join("two.txt"), "two\n").unwrap();
        let settings = DispatchSettings { swarm_limit: 1, ..DispatchSettings::default() };
        let results = merge_swarm_items_core(&project, "run-limit", vec![merge_item("one", 1, &first, ""), merge_item("two", 2, &second, "")], "", &settings).unwrap();
        assert_eq!(results.iter().map(|result| result.result.as_str()).collect::<Vec<_>>(), ["merged", "skipped"]);
        assert!(results[1].note.contains("超过蜂群件数上限"));
        assert!(!project.join("two.txt").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08a_r1_merge_record_uses_status_enum_and_reconcile_ignores_it() {
        if !require_git("task08a_r1_merge_record_uses_status_enum_and_reconcile_ignores_it") { return; }
        let (root, project, first, _) = swarm_repository("record-status");
        fs::write(first.join("one.txt"), "one\n").unwrap();
        let results = merge_swarm_items_core(&project, "run-record", vec![merge_item("node-1", 1, &first, "")], "", &DispatchSettings::default()).unwrap();
        let record_path = fs::read_dir(project.join(".dispatch")).unwrap().filter_map(Result::ok)
            .map(|entry| entry.path()).find(|path| path.extension().and_then(|value| value.to_str()) == Some("md")).unwrap();
        let record = parse_record_file(&record_path).unwrap().0;
        assert_eq!((record.status.as_str(), record.merge_result.as_str()), ("ok", "merged"));
        let status = HashMap::from([("node-1".into(), "运行中".into())]);
        write_state(&project.join(".dispatch/STATE.md"), "run-record", 1, 1, &status, &[]).unwrap();
        let reconciled = reconcile_dispatch_state(project.to_string_lossy().into_owned()).unwrap();
        assert_eq!(reconciled.nodes[0].verdict, "unknown");
        assert_eq!(results[0].result, "merged");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08a_tauri_swarm_contract_serializes_camel_case_and_merge_results() {
        let request = SwarmMergeItem { node_id: "n1".into(), item: 1, worktree: "w".into(), files: vec!["src/**".into()], title: "title".into(), accept: "test".into() };
        let value = serde_json::to_value(request).unwrap();
        assert_eq!(value["nodeId"], "n1");
        assert_eq!(value["files"][0], "src/**");
        assert!(value.get("node_id").is_none());
        let result = SwarmMergeResult { node_id: "n1".into(), item: 1, result: "merged".into(), ..SwarmMergeResult::default() };
        let result_value = serde_json::to_value(result).unwrap();
        assert_eq!(result_value["result"], "merged");
        assert!(result_value.get("testOutput").is_some());
        assert!(result_value.get("test_output").is_none());
        let settings: DispatchSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings.swarm_limit, 4);
    }

    #[test]
    fn task08d_merge_wire_defaults_missing_declared_files_to_safe_skip() {
        if !require_git("task08d_merge_wire_defaults_missing_declared_files_to_safe_skip") { return; }
        let (root, project, first, _) = swarm_repository("r1-missing-files-wire");
        fs::write(first.join("undeclared.txt"), "must not merge\n").unwrap();
        let item: SwarmMergeItem = serde_json::from_value(serde_json::json!({
            "nodeId": "legacy",
            "item": 1,
            "worktree": first.to_string_lossy(),
            "accept": ""
        })).unwrap();
        assert!(item.files.is_empty());
        let results = merge_swarm_items_core(&project, "run-r1-missing-files-wire", vec![item], "", &DispatchSettings::default()).unwrap();
        assert_eq!(results[0].result, "skipped", "{results:?}");
        assert!(results[0].note.contains("未声明文件范围"));
        assert!(!project.join("undeclared.txt").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08a_server_registers_both_swarm_commands() {
        let server = include_str!("server.rs");
        assert!(server.contains("crate::dispatch::check_swarm_plan,"));
        assert!(server.contains("crate::dispatch::merge_swarm_items,"));
    }

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
            stage: 1,
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
            previous_runner: String::new(),
            write: "只读".into(),
            original_request: "分析材料".into(),
            upstream_artifacts: Vec::new(),
            fallback: None,
            rollback_before: false,
            review_material_validation: false,
            fix_round: false,
        }
    }

    fn stage_request(project_dir: String, nodes: Vec<DispatchNodeRequest>) -> DispatchStageRequest {
        DispatchStageRequest {
            run_id: "r3-test-run".into(),
            stage: 1,
            project_dir,
            nodes,
            settings: DispatchSettings::default(),
        }
    }

    #[test]
    fn r3_1_artifacts_serialize_with_snake_case_keys() {
        let value = serde_json::to_value(DispatchArtifacts {
            diff: "d".into(),
            changed_files: vec![ChangedFile { path: "a.txt".into(), content: "c".into() }],
            worktree: Some("wt".into()),
            stdout_summary: "s".into(),
            before_dir: String::new(),
        }).unwrap();
        let mut keys = value.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
        keys.sort();
        assert_eq!(keys, ["changed_files", "diff", "stdout_summary", "worktree"]);
        let changed = &value["changed_files"][0];
        let mut changed_keys = changed.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
        changed_keys.sort();
        assert_eq!(changed_keys, ["content", "path"]);
    }

    #[test]
    fn task06_dispatch_request_reads_rollback_before_camel_case() {
        let mut value = serde_json::to_value(node(Vec::new())).unwrap();
        value.as_object_mut().unwrap().insert("rollbackBefore".into(), true.into());
        let restored: DispatchNodeRequest = serde_json::from_value(value).unwrap();
        assert!(restored.rollback_before);
    }

    #[test]
    fn task06_execution_prompt_forbids_external_side_effects() {
        let prompt = include_str!("../fixtures/prompts/execution.txt");
        assert!(prompt.contains("不得 git push、发布、安装依赖或调用付费接口；这些留给用户在验收之后做。"));
    }

    #[test]
    fn r3_3_empty_target_returns_node_error_without_creating_run_files() {
        let root = std::env::temp_dir().join(format!("aicross-r3-empty-target-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let mut empty = node(Vec::new());
        empty.agent.clear();
        let result = dispatch_stage_core(stage_request(root.to_string_lossy().into_owned(), vec![empty]), None).unwrap();
        assert_eq!(result.results[0].status, "error");
        assert!(result.results[0].error.contains("未指定"));
        assert!(fs::read_dir(&root).unwrap().next().is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn r3_7_large_stdout_is_externalized_but_small_stdout_is_inlined() {
        let root = std::env::temp_dir().join(format!("aicross-r3-stdout-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let large = "x".repeat(40 * 1024);
        let recorded = record_stdout(&large, &root, "node-large", "主");
        assert!(recorded.len() <= 32 * 1024 + 512);
        assert!(recorded.contains("原始输出已转存"));
        assert_eq!(fs::read_to_string(root.join("node-large.raw.txt")).unwrap(), large);
        let small = "y".repeat(20 * 1024);
        assert_eq!(record_stdout(&small, &root, "node-small", "主"), small);
        let failed_target = root.join("node-failed.备1.raw.txt");
        fs::create_dir_all(&failed_target).unwrap();
        let failed = record_stdout(&large, &root, "node-failed", "备");
        assert!(failed.contains("原始输出转存失败："));
        assert!(failed.contains("末尾 4KB"));
        assert!(!root.join("node-small.raw.txt").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn r3_8_empty_project_dir_rejects_preview_and_dispatch_without_writes() {
        let before = fs::read_dir(std::env::current_dir().unwrap()).unwrap().count();
        let request = stage_request(String::new(), vec![node(Vec::new())]);
        assert!(preview_core(&request).unwrap_err().contains("工作区"));
        assert!(dispatch_stage_core(request, None).unwrap_err().contains("工作区"));
        let after = fs::read_dir(std::env::current_dir().unwrap()).unwrap().count();
        assert_eq!(after, before);
    }

    #[test]
    fn r3_12_artifacts_and_blind_delivery_redact_secrets() {
        let secret = "sk-abcdef0123456789abcdef";
        let mut artifacts = DispatchArtifacts {
            diff: format!("+api_key={secret}"),
            changed_files: vec![ChangedFile { path: "a.txt".into(), content: format!("api_key={secret}") }],
            ..DispatchArtifacts::default()
        };
        let hits = redact_artifacts(&mut artifacts);
        assert!(hits >= 1);
        assert!(!artifacts.diff.contains(secret));
        assert!(!artifacts.changed_files[0].content.contains(secret));

        let root = std::env::temp_dir().join(format!("aicross-r3-blind-redact-{}", Uuid::new_v4()));
        let mut blind = node(Vec::new());
        blind.original_request = "review".into();
        blind.upstream_artifacts = vec![DispatchArtifacts {
            diff: format!("+api_key={secret}"),
            changed_files: vec![ChangedFile { path: "a.txt".into(), content: format!("api_key={secret}") }],
            ..DispatchArtifacts::default()
        }];
        let (_, _, count) = prepare_outbound(&blind, true, &root, true).unwrap();
        assert!(count >= 1);
        assert!(!fs::read_to_string(root.join("diff.patch")).unwrap().contains(secret));
        assert!(!fs::read_to_string(root.join("changed/a.txt")).unwrap().contains(secret));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn r3_13_missing_git_probe_falls_back_and_failed_materialization_leaves_no_diff_dir() {
        let root = std::env::temp_dir().join(format!("aicross-r3-git-fallback-{}", Uuid::new_v4()));
        let missing = root.join("missing-project");
        let fake_git = root.join("git-does-not-exist.exe");
        assert!(!project_is_git_work_tree_with(&fake_git, &missing, Duration::from_millis(50)).unwrap_or(false));
        let before = HashMap::from([("a.txt".into(), SnapshotFile { bytes: Some(b"old".to_vec()), ..SnapshotFile::default() })]);
        let after = HashMap::from([("a.txt".into(), SnapshotFile { bytes: Some(b"new".to_vec()), ..SnapshotFile::default() })]);
        fs::create_dir_all(&root).unwrap();
        let fallback = artifact_diff_for_project(&missing, &root, &before, &after, Duration::from_millis(50)).unwrap();
        assert!(!fallback.diff.is_empty());
        let diff_failed = artifact_diff_for_project_with_git(&fake_git, true, &root, &before, &after, Duration::from_millis(50)).unwrap();
        assert_eq!(diff_failed.diff, artifact_diff(&before, &after).diff);
        let blocked = root.join("blocked");
        fs::write(&blocked, "file").unwrap();
        assert!(artifact_diff_for_project_with_git(&fake_git, true, &blocked, &before, &after, Duration::from_millis(50)).is_err());
        let diff_dirs = fs::read_dir(&root).unwrap().filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("diff-")).count();
        assert_eq!(diff_dirs, 0);
        let normalized = normalize_git_diff("diff --git a/before/a.txt b/after/a.txt\n--- a/before/a.txt\n+++ b/after/a.txt\n+a/before/content");
        assert!(normalized.contains("+a/before/content"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn r3_14_blind_single_writer_nodes_do_not_take_the_project_write_lock() {
        let project = std::env::temp_dir().join(format!("aicross-r4-blind-lock-{}", Uuid::new_v4()));
        fs::create_dir_all(&project).unwrap();
        let _held = acquire_project_write(&project, Duration::from_secs(1), "writer").unwrap();
        let mut blind = node(Vec::new());
        blind.write = "单写者".into();
        blind.visibility = "盲".into();
        assert!(!requires_single_writer(&blind, true));
        assert!(!requires_single_writer(&blind, false));
        blind.visibility = "共享".into();
        assert!(requires_single_writer(&blind, true));
        let _ = fs::remove_dir_all(project);
    }

    #[test]
    fn r4_2_invalid_acceptance_write_returns_node_error() {
        let root = std::env::temp_dir().join(format!("aicross-r4-readonly-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let mut invalid = node(Vec::new());
        invalid.node_id = "invalid-review".into();
        invalid.write = "worktree".into();
        let result = dispatch_stage_core(stage_request(root.to_string_lossy().into_owned(), vec![invalid]), None).unwrap();
        assert_eq!(result.results.len(), 1);
        assert_eq!(result.results[0].node_id, "invalid-review");
        assert_eq!(result.results[0].status, "error");
        assert!(result.state_file.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn r4_1_worktree_fallback_runs_validation_in_effective_directory() {
        let root = std::env::temp_dir().join(format!("aicross-r4-validation-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        std::env::set_var("AICROSS_TEST_RUN_ROOT", root.join("runtime"));
        let mut worktree = node(Vec::new());
        worktree.role = "执行".into();
        worktree.visibility = "共享".into();
        worktree.write = "worktree".into();
        assert!(successful_validation_output(&worktree, false).is_none());
        let marker = root.join("validated.txt");
        #[cfg(windows)]
        let command = format!("Set-Content -LiteralPath '{}' -Value validated", marker.display());
        #[cfg(not(windows))]
        let command = format!("printf validated > '{}'", marker.display());
        let output = run_validation(&command, &root, Duration::from_secs(5)).unwrap();
        assert!(!output.contains("未跑验证命令"));
        assert!(marker.exists());
        std::env::remove_var("AICROSS_TEST_RUN_ROOT");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn r5_3_validation_error_strips_ansi_and_formats_exit_code() {
        assert_eq!(validation_failure(Some(1), "\x1b[33mfailed\x1b[0m"), "验证命令失败（exit=1）\nfailed");
    }

    #[test]
    fn r5_8_quoted_new_file_diff_headers_are_normalized() {
        let raw = "diff --git \"before/新 文件.txt\" \"after/新 文件.txt\"\n--- \"before/新 文件.txt\"\n+++ \"after/新 文件.txt\"";
        let normalized = normalize_git_diff(raw);
        assert!(normalized.contains("--- \"a/新 文件.txt\""));
        assert!(normalized.contains("+++ \"b/新 文件.txt\""));
        assert!(!normalized.contains("\"after/"));
    }

    #[test]
    fn r5_9_kimi_missing_identity_and_usage_are_explicit_in_trace_fields() {
        let mut output = ChannelOutput::default();
        let mut error = String::new();
        annotate_kimi_metadata("kimi", &mut output, &mut error);
        assert_eq!(output.identity, "kimi 不报真身");
        assert_eq!(error, "kimi 不报用量");
    }

    #[test]
    fn r3_16_worktree_validation_and_r3_17_abandoned_state_keep_run_identity() {
        let mut worktree = node(Vec::new());
        worktree.role = "执行".into();
        worktree.write = "worktree".into();
        assert!(successful_validation_output(&worktree, true).unwrap().contains("未跑验证命令"));

        let root = std::env::temp_dir().join(format!("aicross-r3-state-{}", Uuid::new_v4()));
        let dispatch = root.join(".dispatch");
        fs::create_dir_all(&dispatch).unwrap();
        let status = HashMap::from([("a".into(), "排队".into())]);
        write_state(&dispatch.join("STATE.md"), "run-from-state", 2, 1, &status, &[]).unwrap();
        let inspected = inspect_dispatch_state(root.to_string_lossy().into_owned()).unwrap();
        assert_eq!(inspected.run_id, "run-from-state");
        abandon_dispatch_state(root.to_string_lossy().into_owned()).unwrap();
        assert!(!dispatch.join("STATE.md").exists());
        assert!(fs::read_dir(&dispatch).unwrap().filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().starts_with("STATE.abandoned-")));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task08a_dispatch_record_swarm_fields_roundtrip() {
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
            runner: "oneshot".into(),
            runner_note: String::new(),
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
            attempt: "主".into(),
            attempt_index: 0,
            write: "只读".into(),
            fallback_skipped: String::new(),
            before_dir: String::new(),
            validation_attached: false,
            pinned: String::new(),
            swarm_item: Some(2),
            merge_result: "merged".into(),
        };
        let yaml = serde_yaml::to_string(&record).unwrap();
        let restored: DispatchRecord = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(restored, record);
        assert!(serialize_frontmatter(&record).unwrap().starts_with("---\n"));
        assert!(serialize_frontmatter(&record)
            .unwrap()
            .contains("runner: oneshot"));
        assert!(serialize_frontmatter(&record).unwrap().contains("swarm_item: 2"));
        assert!(serialize_frontmatter(&record).unwrap().contains("merge_result: merged"));
    }

    #[test]
    fn legacy_dispatch_record_without_runner_defaults_to_oneshot() {
        let yaml = "schema: aicross-dispatch/1\nrun_id: old\nstage: 1\nnode: n\nagent: kimi\nprovider: ''\nmodel: m\nvendor: v\nbilling_source: b\nrole: r\ntier: t\nthinking: 中\nvisibility: 共享\nisolation: 部分\nworkdir: x\nmaterials: []\nredacted: 0\ntruncated: false\nstatus: ok\nstarted_at: s\nended_at: e\nduration_ms: 1\ntokens_in: 0\ntokens_out: 0\ncache_read: 0\nidentity: ''\nsession_id: ''\nexit_code: 0\nerror: ''\n";
        let restored: DispatchRecord = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(restored.runner, "oneshot");
        assert!(restored.runner_note.is_empty());
    }

    #[test]
    fn task07_dispatch_record_all_legacy_fields_default_instead_of_unknown() {
        let restored: DispatchRecord = serde_yaml::from_str("node: legacy\n").unwrap();
        assert_eq!(restored.node, "legacy");
        assert_eq!(restored.runner, "oneshot");
        assert_eq!((restored.stage, restored.tokens_in, restored.truncated), (0, 0, false));
    }

    #[test]
    fn r1_record_unknown_key_is_ignored_and_missing_keys_use_defaults() {
        let root = std::env::temp_dir().join(format!("aicross-r1-record-defaults-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("record.md");
        fs::write(&path, "---\nschema: aicross-dispatch/1\nnode: legacy\nunknown_key: ignored\n---\n\n## 回复正文\n\nok\n").unwrap();
        let (record, reply, _) = parse_record_file(&path).unwrap();
        assert_eq!((record.node.as_str(), record.runner.as_str(), record.stage), ("legacy", "oneshot", 0));
        assert_eq!(reply, "ok");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn r1_record_schema_mismatch_is_rejected() {
        let root = std::env::temp_dir().join(format!("aicross-r1-record-schema-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("record.md");
        fs::write(&path, "---\nschema: aicross-dispatch/999\nnode: legacy\n---\n\n## 回复正文\n\nok\n").unwrap();
        assert!(parse_record_file(&path).unwrap_err().contains("不支持的派发记录 schema"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task07_running_progress_serializes_heartbeat_contract_fields() {
        let activity = ProcessActivity::new();
        activity.record_tool_call("Read file");
        activity.record_tool_call("Read file");
        activity.record_tool_call("Read file");
        let (elapsed_ms, idle_ms, tool_calls, repeated_tool_calls, last_tool) = activity.snapshot();
        let value = serde_json::to_value(DispatchProgress {
            run_id: "run".into(), node_id: "node".into(), status: "running".into(),
            elapsed_ms: Some(elapsed_ms), idle_ms: Some(idle_ms), tool_calls: Some(tool_calls),
            repeated_tool_calls: Some(repeated_tool_calls), last_tool: Some(last_tool),
        }).unwrap();
        assert_eq!(value["status"], "running");
        assert_eq!((value["toolCalls"].as_u64(), value["repeatedToolCalls"].as_u64()), (Some(3), Some(3)));
        assert_eq!(value["lastTool"], "Read file");
        assert!(value.get("elapsedMs").is_some() && value.get("idleMs").is_some());
    }

    #[test]
    fn r1_heartbeat_normal_stop_emits_once_then_exits() {
        let (stop_tx, stop_rx) = mpsc::channel();
        let (emit_tx, emit_rx) = mpsc::channel();
        let heartbeat = thread::spawn(move || heartbeat_loop(stop_rx, Duration::from_millis(10), || {
            emit_tx.send(()).unwrap();
        }));
        emit_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        stop_tx.send(()).unwrap();
        heartbeat.join().unwrap();
        assert!(emit_rx.recv_timeout(Duration::from_millis(30)).is_err());
    }

    #[test]
    fn r1_heartbeat_disconnected_stop_channel_exits_without_more_emits() {
        let (stop_tx, stop_rx) = mpsc::channel();
        let (emit_tx, emit_rx) = mpsc::channel();
        let heartbeat = thread::spawn(move || heartbeat_loop(stop_rx, Duration::from_millis(10), || {
            emit_tx.send(()).unwrap();
        }));
        emit_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        drop(stop_tx);
        heartbeat.join().unwrap();
        assert!(emit_rx.recv_timeout(Duration::from_millis(30)).is_err());
    }

    #[test]
    fn r1_terminal_progress_after_heartbeat_stop_has_no_running_fields_or_late_emit() {
        let (stop_tx, stop_rx) = mpsc::channel();
        let (emit_tx, emit_rx) = mpsc::channel();
        let heartbeat = thread::spawn(move || heartbeat_loop(stop_rx, Duration::from_millis(10), || {
            emit_tx.send(running_progress("run", "node", &ProcessActivity::new())).unwrap();
        }));
        emit_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        stop_tx.send(()).unwrap();
        heartbeat.join().unwrap();
        let terminal = DispatchProgress {
            run_id: "run".into(), node_id: "node".into(), status: "完成".into(),
            elapsed_ms: None, idle_ms: None, tool_calls: None,
            repeated_tool_calls: None, last_tool: None,
        };
        let value = serde_json::to_value(terminal).unwrap();
        assert!(value.get("elapsedMs").is_none() && value.get("lastTool").is_none());
        assert!(emit_rx.recv_timeout(Duration::from_millis(30)).is_err());
    }

    #[test]
    fn r1_running_progress_redacts_authorization_bearer_title() {
        let activity = ProcessActivity::new();
        activity.record_tool_call("Authorization: Bearer secret-token");
        let value = serde_json::to_value(running_progress("run", "node", &activity)).unwrap();
        assert!(!value["lastTool"].as_str().unwrap().contains("secret-token"));
        assert!(value["lastTool"].as_str().unwrap().contains("[REDACTED]"));
    }

    #[test]
    fn r1_running_progress_truncates_last_tool_to_120_characters() {
        let activity = ProcessActivity::new();
        activity.record_tool_call(&"工".repeat(121));
        let value = serde_json::to_value(running_progress("run", "node", &activity)).unwrap();
        assert_eq!(value["lastTool"].as_str().unwrap().chars().count(), 120);
    }

    #[test]
    fn task07_fake_acp_three_same_updates_plus_permission_reach_running_event_payload() {
        let _pool_guard = channels::acp::test_pool_guard();
        if channels::resolve_binary("python", None).is_err() { eprintln!("跳过：PATH 中没有 python"); return; }
        let root = std::env::temp_dir().join(format!("aicross-task07-acp-progress-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let (request, mut settings, _) = acp_run_one_fixture(&root, &["--repeat-tool-calls"]);
        settings.timeout_secs = 10;
        let activity = ProcessActivity::new();
        let result = run_one_with_activity("run", &project, &root.join("runs"), &log_dir, &settings, request, activity.clone());
        assert_eq!(result.status, "ok", "{}", result.error);
        let event = serde_json::to_value(running_progress("run", "node-1", &activity)).unwrap();
        assert_eq!(event["repeatedToolCalls"], 4);
        assert_eq!(event["toolCalls"], 4);
        assert_eq!(event["lastTool"], "Read file");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn r1_acp_to_oneshot_fallback_resets_attempt_tool_counts() {
        let _pool_guard = channels::acp::test_pool_guard();
        if channels::resolve_binary("python", None).is_err() { eprintln!("跳过：PATH 中没有 python"); return; }
        let root = std::env::temp_dir().join(format!("aicross-r1-acp-fallback-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let (request, mut settings, _) = acp_run_one_fixture(&root, &["--repeat-tool-calls", "--fail-prompt-after-tools"]);
        settings.timeout_secs = 10;
        let activity = ProcessActivity::new();
        let result = run_one_with_activity("run", &project, &root.join("runs"), &log_dir, &settings, request, activity.clone());
        assert_eq!((result.status.as_str(), result.runner.as_str()), ("ok", "oneshot"), "{}", result.error);
        let (_, _, calls, repeated, last_tool) = activity.snapshot();
        assert_eq!((calls, repeated, last_tool), (0, 0, String::new()));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn r1_backup_attempt_does_not_inherit_primary_tool_counts() {
        if channels::resolve_binary("python", None).is_err() { eprintln!("跳过：PATH 中没有 python"); return; }
        let root = std::env::temp_dir().join(format!("aicross-r1-backup-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let stub = root.join("stub.py");
        fs::write(&stub, "print('backup')\nprint('input=1 cache_read=0 output=1')\n").unwrap();
        let mut request = node(Vec::new());
        request.visibility = "盲".into();
        request.fallback = Some(FallbackSpec {
            agent: "cc-switch".into(), provider: "stub".into(), model: "stub".into(), vendor: String::new(),
        });
        let settings = DispatchSettings { cc_switch_path: stub.to_string_lossy().into_owned(), ..DispatchSettings::default() };
        let activity = ProcessActivity::new();
        activity.record_tool_call("Primary tool");
        let result = run_fallback("run", &project, &root.join("runs"), &log_dir, &settings, &request, &activity);
        assert_eq!(result.status, "ok", "{}", result.error);
        let (_, _, calls, repeated, last_tool) = activity.snapshot();
        assert_eq!((calls, repeated, last_tool), (0, 0, String::new()));
        let _ = fs::remove_dir_all(root);
    }

    /// TASK-09：`verification == "unverified"` 的第三方目标首次派发前先验证；
    /// 验证失败的节点直接判 error，通道一次都不该跑（stub 不留痕迹）。
    #[test]
    fn task09_first_use_verification_failure_fails_node_before_channel() {
        let root = std::env::temp_dir().join(format!("aicross-task09-verify-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let marker = root.join("channel_ran.txt");
        let stub = root.join("cc_switch_stub.py");
        fs::write(
            &stub,
            format!(
                "import pathlib\npathlib.Path(r'{}').write_text('ran')\nprint('stub answer')\n",
                marker.display()
            ),
        )
        .unwrap();
        let mut request = node(Vec::new());
        request.agent = "cc-switch".into();
        request.provider = "stub".into();
        let settings = DispatchSettings {
            cc_switch_path: stub.to_string_lossy().into_owned(),
            ..DispatchSettings::default()
        };
        FIRST_USE_STUB.with(|cell| {
            *cell.borrow_mut() = Some(crate::resources::FirstUseVerification::Failed(
                "响应模型不符：请求 glm-4.7，响应 zai/glm-4.5".into(),
            ))
        });
        let failed = run_one("verify-fail", &project, &root.join("runs"), &log_dir, &settings, request.clone());
        FIRST_USE_STUB.with(|cell| *cell.borrow_mut() = None);
        assert_eq!(failed.status, "error");
        assert!(failed.error.starts_with("首次验证失败："), "{}", failed.error);
        assert!(failed.runner_note.contains("首次派发自动验证"), "{}", failed.runner_note);
        assert!(!marker.exists(), "验证失败的节点不该进通道");

        // 同一条路径验证通过时照常派发，runner_note 记一笔。
        if channels::resolve_binary("python", None).is_ok() {
            FIRST_USE_STUB.with(|cell| {
                *cell.borrow_mut() = Some(crate::resources::FirstUseVerification::Verified)
            });
            let ok = run_one("verify-ok", &project, &root.join("runs"), &log_dir, &settings, request);
            FIRST_USE_STUB.with(|cell| *cell.borrow_mut() = None);
            assert_eq!(ok.status, "ok", "{}", ok.error);
            assert!(ok.runner_note.contains("首次派发已验证真身"), "{}", ok.runner_note);
            assert!(marker.exists());
        }
        let _ = fs::remove_dir_all(root);
    }

    /// 修正轮 R1-3/4：首次验证跑在取项目写锁、建 worktree、备料之前。
    /// 测试握着同一项目的写锁，验证失败的节点必须直接返回「首次验证失败」，
    /// 而不是先在写锁上等到超时；worktree 目录也不该被建出来。
    #[test]
    fn r1_first_use_verification_runs_before_write_lock_and_worktree() {
        let root = std::env::temp_dir().join(format!("aicross-r1-lock-{}", Uuid::new_v4()));
        let project = root.join("project");
        let run_root = root.join("runs");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let marker = root.join("channel_ran.txt");
        let stub = root.join("cc_switch_stub.py");
        fs::write(
            &stub,
            format!(
                "import pathlib\npathlib.Path(r'{}').write_text('ran')\nprint('stub answer')\n",
                marker.display()
            ),
        )
        .unwrap();
        let mut request = node(Vec::new());
        request.agent = "cc-switch".into();
        request.provider = "stub".into();
        request.model = "glm-4.7".into();
        request.visibility = "共享".into();
        request.write = "worktree".into();
        let settings = DispatchSettings {
            cc_switch_path: stub.to_string_lossy().into_owned(),
            timeout_secs: 2,
            ..DispatchSettings::default()
        };
        // 同项目的写锁被别人占着：没有「验证在前」这条，单写者退路会先在这里等到超时。
        let held = acquire_project_write(&project, Duration::from_secs(2), "holder").expect("lock");
        FIRST_USE_STUB.with(|cell| {
            *cell.borrow_mut() = Some(crate::resources::FirstUseVerification::Failed(
                "响应模型不符：请求 glm-4.7，响应 zai/glm-4.5".into(),
            ))
        });
        FIRST_USE_CALLS.with(|cell| cell.borrow_mut().clear());
        let failed = run_one("r1-lock", &project, &run_root, &log_dir, &settings, request.clone());
        FIRST_USE_STUB.with(|cell| *cell.borrow_mut() = None);
        drop(held);
        assert_eq!(failed.status, "error");
        assert!(failed.error.starts_with("首次验证失败："), "{}", failed.error);
        assert!(!marker.exists(), "验证失败的节点不该进通道");
        assert!(
            !run_root.join(&request.node_id).join("wt").exists(),
            "验证失败不该白建 worktree"
        );
        // 替身收到的是节点的真实目标三元组（原来替身把三个参数直接丢掉）。
        let calls = FIRST_USE_CALLS.with(|cell| cell.borrow().clone());
        assert_eq!(
            calls,
            vec![("cc-switch".to_string(), "stub".to_string(), "glm-4.7".to_string())]
        );
        let _ = fs::remove_dir_all(root);
    }

    /// 修正轮 R1-3：验证窗口内点了叫停——该节点按「已取消」返回，不进通道。
    #[test]
    fn r1_cancel_during_first_use_verification_stops_the_node() {
        let root = std::env::temp_dir().join(format!("aicross-r1-cancel-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let marker = root.join("channel_ran.txt");
        let stub = root.join("cc_switch_stub.py");
        fs::write(
            &stub,
            format!(
                "import pathlib\npathlib.Path(r'{}').write_text('ran')\nprint('stub answer')\n",
                marker.display()
            ),
        )
        .unwrap();
        let mut request = node(Vec::new());
        request.agent = "cc-switch".into();
        request.provider = "stub".into();
        let settings = DispatchSettings {
            cc_switch_path: stub.to_string_lossy().into_owned(),
            ..DispatchSettings::default()
        };
        FIRST_USE_STUB.with(|cell| {
            *cell.borrow_mut() = Some(crate::resources::FirstUseVerification::Cancelled)
        });
        let cancelled = run_one("r1-cancel", &project, &root.join("runs"), &log_dir, &settings, request);
        FIRST_USE_STUB.with(|cell| *cell.borrow_mut() = None);
        assert_eq!(cancelled.status, "error");
        assert_eq!(cancelled.error, "派发已取消");
        assert!(
            cancelled.runner_note.contains("验证中已取消"),
            "{}",
            cancelled.runner_note
        );
        assert!(!marker.exists(), "取消的节点不该进通道");
        let _ = fs::remove_dir_all(root);
    }

    /// 修正轮 R1-2：资源库里没有这一条时照常派发，但留痕写明「没验」——
    /// 和「原生跳过」分得开。
    #[test]
    fn r1_first_use_not_in_store_dispatches_with_note() {
        if channels::resolve_binary("python", None).is_err() {
            return;
        }
        let root = std::env::temp_dir().join(format!("aicross-r1-nostore-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let stub = root.join("cc_switch_stub.py");
        fs::write(&stub, "print('stub answer')\nprint('input=1 cache_read=0 output=1')\n").unwrap();
        let mut request = node(Vec::new());
        request.agent = "cc-switch".into();
        request.provider = "stub".into();
        let settings = DispatchSettings {
            cc_switch_path: stub.to_string_lossy().into_owned(),
            ..DispatchSettings::default()
        };
        FIRST_USE_STUB.with(|cell| {
            *cell.borrow_mut() = Some(crate::resources::FirstUseVerification::NotInStore)
        });
        let result = run_one("r1-nostore", &project, &root.join("runs"), &log_dir, &settings, request);
        FIRST_USE_STUB.with(|cell| *cell.borrow_mut() = None);
        assert_eq!(result.status, "ok", "{}", result.error);
        assert!(
            result.runner_note.contains("资源库无此条目，未做首次验证"),
            "{}",
            result.runner_note
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn acp_disabled_or_unsupported_preserves_oneshot_dispatch_result() {
        let root = std::env::temp_dir().join(format!("aicross-acp-regression-{}", Uuid::new_v4()));
        let project = root.join("project");
        let run_root = root.join("runs");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let stub = root.join("cc_switch_stub.py");
        fs::write(
            &stub,
            "print('stub answer')\nprint('input=2 cache_read=0 output=1')\n",
        )
        .unwrap();
        let mut request = node(Vec::new());
        request.agent = "cc-switch".into();
        request.provider = "stub".into();
        let settings = DispatchSettings {
            cc_switch_path: stub.to_string_lossy().into_owned(),
            ..DispatchSettings::default()
        };
        let disabled = run_one(
            "off",
            &project,
            &run_root,
            &log_dir,
            &settings,
            request.clone(),
        );
        let enabled = run_one(
            "on",
            &project,
            &run_root,
            &log_dir,
            &DispatchSettings {
                acp_enabled: true,
                ..settings
            },
            request,
        );
        assert_eq!(enabled.status, disabled.status);
        assert_eq!(enabled.error, disabled.error);
        assert_eq!(enabled.runner, "oneshot");
        assert!(enabled.text.starts_with("stub answer"));
        assert!(enabled.frontmatter.contains("runner: oneshot"));
        let _ = fs::remove_dir_all(root);
    }

    fn acp_run_one_fixture(root: &Path, acp_args: &[&str]) -> (DispatchNodeRequest, DispatchSettings, PathBuf) {
        let fake_server = std::env::current_dir().unwrap().join("fixtures/acp/fake_server.py");
        let oneshot = root.join("kimi_oneshot_stub.py");
        fs::write(&oneshot, "import json, pathlib, sys\npathlib.Path(__file__).with_suffix('.count').open('a', encoding='utf-8').write('run\\n')\nprint(json.dumps({'role':'assistant','content':'oneshot answer'}))\nprint('ARGV=' + repr(sys.argv))\n").unwrap();
        let provider = format!("__aicross_test__|{}|{}{}", fake_server.display(), oneshot.display(), acp_args.iter().map(|arg| format!("|{arg}")).collect::<String>());
        let mut request = node(Vec::new());
        request.agent = "kimi".into();
        request.provider = provider;
        request.model = "fake-model/model".into();
        request.visibility = "共享".into();
        request.prompt = "完整祖先上下文\n当前追问".into();
        request.current_message = "当前追问".into();
        let settings = DispatchSettings { acp_enabled: true, timeout_secs: 1, ..DispatchSettings::default() };
        (request, settings, oneshot.with_extension("count"))
    }

    #[test]
    fn run_one_silent_acp_timeout_stays_acp_and_never_calls_oneshot() {
        let _pool_guard = channels::acp::test_pool_guard();
        let Some(_) = channels::resolve_binary("python", None).ok() else { eprintln!("跳过：PATH 中没有 python"); return };
        let root = std::env::temp_dir().join(format!("aicross-run-one-timeout-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let (request, settings, count) = acp_run_one_fixture(&root, &["--hang-after-prompt"]);
        let result = run_one("acp-timeout", &project, &root.join("runs"), &log_dir, &settings, request);
        assert_eq!(result.status, "timeout");
        assert_eq!(result.runner, "acp");
        assert!(result.runner_note.contains("ACP 超时，未回退"));
        assert!(!count.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn run_one_cancelled_acp_stays_acp_and_never_calls_oneshot() {
        let _pool_guard = channels::acp::test_pool_guard();
        let Some(_) = channels::resolve_binary("python", None).ok() else { eprintln!("跳过：PATH 中没有 python"); return };
        let root = std::env::temp_dir().join(format!("aicross-run-one-cancel-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let (request, mut settings, count) = acp_run_one_fixture(&root, &["--hang-after-prompt"]);
        settings.timeout_secs = 5;
        let run_root = root.join("runs");
        let worker = thread::spawn(move || run_one("acp-cancel", &project, &run_root, &log_dir, &settings, request));
        thread::sleep(Duration::from_millis(300));
        cancel_dispatch_node("acp-cancel".into(), "node-1".into());
        let result = worker.join().unwrap();
        assert_eq!(result.runner, "acp");
        assert!(result.runner_note.contains("ACP 已取消，未回退"));
        assert!(!count.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn run_one_acp_turn_failure_stays_acp_and_never_calls_oneshot() {
        let _pool_guard = channels::acp::test_pool_guard();
        let Some(_) = channels::resolve_binary("python", None).ok() else { eprintln!("跳过：PATH 中没有 python"); return };
        let root = std::env::temp_dir().join(format!("aicross-run-one-failed-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let (request, mut settings, count) = acp_run_one_fixture(&root, &["--late-update"]);
        settings.timeout_secs = 10;
        let result = run_one("acp-failed", &project, &root.join("runs"), &log_dir, &settings, request);
        assert_eq!(result.status, "error");
        assert_eq!(result.runner, "acp");
        assert!(result.runner_note.contains("ACP 失败，未回退"));
        assert!(!count.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn run_one_dead_acp_falls_back_once_without_resume_and_rewrites_full_task() {
        let _pool_guard = channels::acp::test_pool_guard();
        let Some(_) = channels::resolve_binary("python", None).ok() else { eprintln!("跳过：PATH 中没有 python"); return };
        let root = std::env::temp_dir().join(format!("aicross-run-one-fallback-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let (mut request, settings, count) = acp_run_one_fixture(&root, &["--exit-after-first-turn"]);
        request.current_message = request.prompt.clone();
        let first = run_one("acp-first", &project, &root.join("runs-1"), &log_dir, &settings, request.clone());
        assert_eq!(first.runner, "acp");
        thread::sleep(Duration::from_millis(100));
        request.session_id = first.session_id;
        request.previous_runner = "acp".into();
        request.current_message = "当前追问".into();
        let second = run_one("acp-second", &project, &root.join("runs-2"), &log_dir, &settings, request);
        assert_eq!(second.status, "ok");
        assert_eq!(second.runner, "oneshot");
        assert!(second.runner_note.contains("ACP 会话已重置"));
        assert_eq!(fs::read_to_string(count).unwrap().lines().count(), 1);
        assert!(!second.raw_stdout.contains("'-S'"));
        assert!(!second.raw_stdout.contains("'-r'"));
        assert_eq!(fs::read_to_string(root.join("runs-2/tasks/node-1.txt")).unwrap(), "完整祖先上下文\n当前追问");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unsupported_acp_previous_runner_resets_session_and_uses_full_context() {
        let root = std::env::temp_dir().join(format!("aicross-acp-off-resume-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let stub = root.join("cc_switch_stub.py");
        fs::write(&stub, "import sys\nprint('stub full context')\nprint('input=1 cache_read=0 output=1')\nprint('ARGV=' + repr(sys.argv))\nprint('STDIN=' + sys.stdin.read())\n").unwrap();
        let mut request = node(Vec::new());
        request.agent = "cc-switch".into();
        request.provider = "stub".into();
        request.visibility = "共享".into();
        request.session_id = "old-acp-session".into();
        request.previous_runner = "acp".into();
        request.prompt = "完整祖先上下文\n当前追问".into();
        request.current_message = "当前追问".into();
        let result = run_one("acp-unsupported", &project, &root.join("runs"), &log_dir, &DispatchSettings {
            acp_enabled: true,
            cc_switch_path: stub.to_string_lossy().into_owned(),
            ..DispatchSettings::default()
        }, request);
        assert_eq!(result.runner, "oneshot");
        assert!(result.runner_note.contains("旧 ACP 会话已重置"));
        assert!(result.session_id.is_empty());
        assert_eq!(fs::read_to_string(root.join("runs/tasks/node-1.txt")).unwrap(), "完整祖先上下文\n当前追问");
        assert!(result.text.contains("stub full context"));
        assert!(result.raw_stdout.contains("--task-file"));
        assert!(!result.raw_stdout.contains("old-acp-session"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reset_acp_fallback_builds_full_context_invocation_without_resume_flags() {
        let full = "完整祖先上下文\n当前追问";
        let (session, outbound) = reset_acp_fallback(full);
        let invocation = channels::build_invocation(
            "kimi",
            "",
            "kimi-code/model",
            "",
            "中",
            false,
            (!session.is_empty()).then_some(session.as_str()),
            &outbound,
            None,
            None,
            None,
            false,
        )
        .unwrap();
        assert!(!invocation.args.iter().any(|arg| arg == "-S" || arg == "-r"));
        assert!(invocation.args.iter().any(|arg| arg == full));
    }

    // 旧留痕没有后续追加字段也照常解析，新字段取默认空。
    #[test]
    fn legacy_frontmatter_without_r2_fields_parses_with_empty_defaults() {
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
            attempt: "主".into(),
            attempt_index: 0,
            runner: "oneshot".into(),
            runner_note: String::new(),
            write: "只读".into(),
            fallback_skipped: "可写节点".into(),
            before_dir: String::new(),
            validation_attached: false,
            pinned: String::new(),
            swarm_item: Some(2),
            merge_result: "merged".into(),
        };
        let yaml = serde_yaml::to_string(&record).unwrap();
        let legacy = yaml
            .lines()
            .filter(|line| {
                !line.starts_with("attempt:")
                    && !line.starts_with("write:")
                    && !line.starts_with("fallback_skipped:")
                    && !line.starts_with("swarm_item:")
                    && !line.starts_with("merge_result:")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let restored: DispatchRecord = serde_yaml::from_str(&legacy).unwrap();
        assert_eq!(restored.attempt, "");
        assert_eq!(restored.write, "");
        assert_eq!(restored.fallback_skipped, "");
        assert_eq!(restored.swarm_item, None);
        assert_eq!(restored.merge_result, "");
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
        let status = HashMap::from([
            ("a".into(), "运行中".into()),
            ("b".into(), "失败".into()),
            ("c".into(), "验证命令未通过".into()),
            ("d".into(), "额度用尽".into()),
        ]);
        let failed = [
            DispatchNodeResult {
                node_id: "b".into(),
                dispatch_file: "b.md".into(),
                ..DispatchNodeResult::default()
            },
            DispatchNodeResult {
                node_id: "c".into(),
                dispatch_file: "c.md".into(),
                ..DispatchNodeResult::default()
            },
            DispatchNodeResult {
                node_id: "d".into(),
                dispatch_file: "d.md".into(),
                ..DispatchNodeResult::default()
            },
        ];
        write_state(&path, "run", 2, 2, &status, &failed).unwrap();
        let state = fs::read_to_string(&path).unwrap();
        assert!(state.contains("同阶段并发上限 2"));
        let header: DispatchStateHeader = serde_json::from_str(
            state.lines().next().unwrap().strip_prefix("aicross-state: ").unwrap(),
        )
        .unwrap();
        assert_eq!(header.run_id, "run");
        assert!(header.unfinished);
        assert_eq!(header.active_node_ids, ["a"]);
        assert!(state.contains("## 进行中\n- [ ] a: 运行中"));
        assert!(state.contains("## 验证命令未通过\n- [ ] c: 验证命令未通过 → c.md"));
        assert!(state.contains("## 额度用尽\n- [ ] d: 额度用尽 → d.md"));
        assert!(state.contains("## 失败与超时\n- [ ] b: 失败 → b.md"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn inspect_state_uses_machine_header_instead_of_status_prose() {
        let root = std::env::temp_dir().join(format!("aicross-state-header-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join(".dispatch")).unwrap();
        fs::write(
            root.join(".dispatch/STATE.md"),
            "aicross-state: {\"schema\":\"aicross-state/1\",\"run_id\":\"run-1\",\"stage\":2,\"unfinished\":false,\"active_node_ids\":[]}\n- old: 运行中\n",
        )
        .unwrap();
        let summary = inspect_dispatch_state(root.to_string_lossy().into_owned()).unwrap();
        assert!(!summary.unfinished);
        assert_eq!(summary.run_id, "run-1");
        assert!(summary.active_node_ids.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task06_reconcile_classifies_completed_incomplete_unknown_and_ignores_other_run() {
        let root = std::env::temp_dir().join(format!("aicross-task06-reconcile-{}", Uuid::new_v4()));
        let dispatch = root.join(".dispatch");
        fs::create_dir_all(&dispatch).unwrap();
        let status = HashMap::from([
            ("done".into(), "运行中".into()),
            ("failed".into(), "运行中".into()),
            ("missing".into(), "运行中".into()),
        ]);
        write_state(&dispatch.join("STATE.md"), "run-6", 1, 1, &status, &[]).unwrap();
        let record = |run: &str, node: &str, status: &str, error: &str| format!(
            "---\nschema: aicross-dispatch/1\nrun_id: {run}\nstage: 1\nnode: {node}\nagent: codex\nprovider: ''\nmodel: gpt\nvendor: OpenAI\nbilling_source: official\nrole: execute\ntier: low\nthinking: low\nvisibility: shared\nisolation: partial\nrunner: oneshot\nrunner_note: ''\nworkdir: x\nmaterials: []\nredacted: 0\ntruncated: false\nstatus: {status}\nstarted_at: s\nended_at: e\nduration_ms: 1\ntokens_in: 1\ntokens_out: 1\ncache_read: 0\nidentity: gpt\nsession_id: s\nexit_code: 0\nerror: '{error}'\n---\n\n## 任务全文\n\ntask\n\n## 回复正文\n\nreply-{node}\n\n## 产物 JSON\n\n{{\"diff\":\"d\",\"changed_files\":[],\"worktree\":null,\"stdout_summary\":\"s\"}}\n\n## 原始输出\n\nraw\n"
        );
        fs::write(dispatch.join("done.md"), record("run-6", "done", "ok", "")).unwrap();
        fs::write(dispatch.join("failed.md"), record("run-6", "failed", "error", "boom")).unwrap();
        fs::write(dispatch.join("wrong-run.md"), record("other", "missing", "ok", "")).unwrap();
        let summary = reconcile_dispatch_state(root.to_string_lossy().into_owned()).unwrap();
        assert_eq!(summary.run_id, "run-6");
        assert_eq!(summary.nodes.iter().find(|item| item.node_id == "done").unwrap().verdict, "completed");
        assert_eq!(summary.nodes.iter().find(|item| item.node_id == "done").unwrap().reply_text, "reply-done");
        assert!(summary.nodes.iter().find(|item| item.node_id == "done").unwrap().artifacts.is_some());
        let done = summary.nodes.iter().find(|item| item.node_id == "done").unwrap();
        assert_eq!((done.session_id.as_str(), done.runner.as_str()), ("s", "oneshot"));
        assert_eq!(done.usage, ReconcileUsage { input: 1, output: 1, cache_read: 0 });
        assert_eq!(summary.nodes.iter().find(|item| item.node_id == "failed").unwrap().verdict, "incomplete");
        assert_eq!(summary.nodes.iter().find(|item| item.node_id == "missing").unwrap().verdict, "unknown");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task06_single_writer_rollback_restores_changed_deleted_and_created_files() {
        let root = std::env::temp_dir().join(format!("aicross-task06-rollback-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("changed.txt"), "before").unwrap();
        fs::write(root.join("deleted.txt"), "restore me").unwrap();
        let before = snapshot_directory(&root).unwrap();
        fs::write(root.join("changed.txt"), "after").unwrap();
        fs::remove_file(root.join("deleted.txt")).unwrap();
        fs::write(root.join("created.txt"), "remove me").unwrap();
        let after = snapshot_directory(&root).unwrap();
        persist_before_snapshot(&root, &root.join(".dispatch"), "run", "node", &before, &after).unwrap();
        let summary = rollback_node_changes_core(&root, "run", "node").unwrap();
        assert_eq!(fs::read_to_string(root.join("changed.txt")).unwrap(), "before");
        assert_eq!(fs::read_to_string(root.join("deleted.txt")).unwrap(), "restore me");
        assert!(!root.join("created.txt").exists());
        assert_eq!(summary.restored, ["changed.txt", "deleted.txt"]);
        assert_eq!(summary.removed, ["created.txt"]);
        assert!(summary.note.contains("排除目录"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task07_before_snapshot_write_failure_restores_previous_directory() {
        let root = std::env::temp_dir().join(format!("aicross-task07-snapshot-restore-{}", Uuid::new_v4()));
        let old = root.join(".dispatch/run/node.before");
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("kept.txt"), "old snapshot").unwrap();
        let before = HashMap::from([("../unsafe".into(), SnapshotFile { bytes: Some(b"old".to_vec()), ..SnapshotFile::default() })]);
        let after = HashMap::from([("../unsafe".into(), SnapshotFile { bytes: Some(b"new".to_vec()), ..SnapshotFile::default() })]);
        assert!(persist_before_snapshot(&root, &root.join(".dispatch"), "run", "node", &before, &after).is_err());
        assert_eq!(fs::read_to_string(old.join("kept.txt")).unwrap(), "old snapshot");
        assert!(!root.join(".dispatch/run/node.before.prev").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task07_before_snapshot_success_replaces_old_and_removes_prev() {
        let root = std::env::temp_dir().join(format!("aicross-task07-snapshot-commit-{}", Uuid::new_v4()));
        let old = root.join(".dispatch/run/node.before");
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("stale.txt"), "stale").unwrap();
        let before = HashMap::from([("changed.txt".into(), SnapshotFile { bytes: Some(b"old".to_vec()), ..SnapshotFile::default() })]);
        let after = HashMap::from([("changed.txt".into(), SnapshotFile { bytes: Some(b"new".to_vec()), ..SnapshotFile::default() })]);
        persist_before_snapshot(&root, &root.join(".dispatch"), "run", "node", &before, &after).unwrap();
        assert_eq!(fs::read_to_string(old.join("changed.txt")).unwrap(), "old");
        assert!(!old.join("stale.txt").exists());
        assert!(!root.join(".dispatch/run/node.before.prev").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task07_dispatch_and_reconcile_results_preserve_cache_read_usage() {
        let dispatch = DispatchNodeResult { cache_read: 42, ..DispatchNodeResult::default() };
        let reconcile = ReconcileUsage { cache_read: dispatch.cache_read, ..ReconcileUsage::default() };
        assert_eq!(serde_json::to_value(&dispatch).unwrap()["cacheRead"], 42);
        assert_eq!(serde_json::to_value(&reconcile).unwrap()["cacheRead"], 42);
    }

    #[test]
    fn r1_parse_output_populates_dispatch_cache_read() {
        let process = crate::process::ProcessResult {
            code: Some(0), stdout: "answer".into(), stderr: "[usage] input=2 cache_read=42 output=1".into(),
            timed_out: false, truncated: false, timeout: Duration::from_secs(1),
        };
        let output = channels::parse_output("cc-switch", "", "", &process).unwrap();
        let result = DispatchNodeResult { cache_read: output.usage.cache_read, ..DispatchNodeResult::default() };
        assert_eq!(result.cache_read, 42);
    }

    #[test]
    fn r1_reconcile_refills_cache_read_from_record() {
        let root = std::env::temp_dir().join(format!("aicross-r1-reconcile-cache-{}", Uuid::new_v4()));
        let dispatch = root.join(".dispatch");
        fs::create_dir_all(&dispatch).unwrap();
        fs::write(dispatch.join("STATE.md"), "aicross-state: {\"schema\":\"aicross-state/1\",\"run_id\":\"run\",\"stage\":1,\"unfinished\":true,\"active_node_ids\":[\"node\"]}\n").unwrap();
        let record = DispatchRecord {
            schema: "aicross-dispatch/1".into(), run_id: "run".into(), node: "node".into(),
            status: "ok".into(), cache_read: 42, ended_at: "2026-01-01T00:00:00+00:00".into(),
            ..DispatchRecord::default()
        };
        fs::write(dispatch.join("record.md"), format!("{}\n\n## 回复正文\n\nok\n", serialize_frontmatter(&record).unwrap())).unwrap();
        let summary = reconcile_dispatch_state(root.to_string_lossy().into_owned()).unwrap();
        assert_eq!(summary.nodes[0].usage.cache_read, 42);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task07_zero_change_redispatch_preserves_nonempty_snapshot() {
        let root = std::env::temp_dir().join(format!("aicross-task07-zero-change-{}", Uuid::new_v4()));
        let old = root.join(".dispatch/run/node.before");
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("kept.txt"), "old snapshot").unwrap();
        let same = HashMap::from([("same.txt".into(), SnapshotFile { bytes: Some(b"same".to_vec()), ..SnapshotFile::default() })]);
        assert_eq!(persist_before_snapshot(&root, &root.join(".dispatch"), "run", "node", &same, &same).unwrap(), "");
        assert_eq!(fs::read_to_string(old.join("kept.txt")).unwrap(), "old snapshot");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn r1_before_root_file_error_preserves_existing_prev_directory() {
        let root = std::env::temp_dir().join(format!("aicross-r1-before-file-{}", Uuid::new_v4()));
        let before_root = root.join(".dispatch/run/node.before");
        let previous = root.join(".dispatch/run/node.before.prev");
        fs::create_dir_all(&previous).unwrap();
        fs::write(previous.join("kept.txt"), "kept").unwrap();
        fs::write(&before_root, "conflict").unwrap();
        let before = HashMap::from([("a.txt".into(), SnapshotFile { bytes: Some(b"old".to_vec()), ..SnapshotFile::default() })]);
        let after = HashMap::from([("a.txt".into(), SnapshotFile { bytes: Some(b"new".to_vec()), ..SnapshotFile::default() })]);
        assert!(persist_before_snapshot(&root, &root.join(".dispatch"), "run", "node", &before, &after).is_err());
        assert_eq!(fs::read_to_string(previous.join("kept.txt")).unwrap(), "kept");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn r1_committed_snapshot_cleanup_failure_returns_note_not_error() {
        let previous = Path::new("before.prev");
        let note = finish_before_snapshot(previous, true, |_| Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "locked")));
        assert!(note.contains("清理 before.prev 快照失败"));
        assert!(finish_before_snapshot(previous, false, |_| unreachable!()).is_empty());
    }

    #[test]
    fn r1_zero_change_redispatch_keeps_old_snapshot_discoverable_for_rollback() {
        if channels::resolve_binary("python", None).is_err() { eprintln!("跳过：PATH 中没有 python"); return; }
        let root = std::env::temp_dir().join(format!("aicross-r1-zero-e2e-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = project.join(".dispatch");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("a.txt"), "old").unwrap();
        let stub = root.join("stub.py");
        fs::write(&stub, "import pathlib\np=pathlib.Path.cwd()\n(p/'a.txt').write_text('new', encoding='utf-8')\nprint('changed')\nprint('input=1 cache_read=0 output=1')\n").unwrap();
        let mut request = node(Vec::new());
        request.agent = "cc-switch".into(); request.provider = "stub".into(); request.model = "stub".into();
        request.role = "执行".into(); request.visibility = "共享".into(); request.write = "单写者".into();
        let settings = DispatchSettings { cc_switch_path: stub.to_string_lossy().into_owned(), ..DispatchSettings::default() };
        let first = run_one("run-a", &project, &root.join("runs"), &log_dir, &settings, request.clone());
        assert_eq!(first.status, "ok", "{}", first.error);
        assert!(first.frontmatter.contains("before_dir: .dispatch/run-a/node-1.before"));
        fs::write(&stub, "print('unchanged')\nprint('input=1 cache_read=0 output=1')\n").unwrap();
        let second = run_one("run-b", &project, &root.join("runs"), &log_dir, &settings, request);
        assert_eq!(second.status, "ok", "{}", second.error);
        assert!(second.frontmatter.contains("before_dir: ''"));
        fs::write(project.join("a.txt"), "dirty").unwrap();
        let summary = rollback_node_changes_core(&project, "run-b", "node-1").unwrap();
        assert_eq!(fs::read_to_string(project.join("a.txt")).unwrap(), "old");
        assert_eq!(summary.restored, ["a.txt"]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task07_latest_before_dir_rejects_uncanonicalizable_workdir() {
        let root = std::env::temp_dir().join(format!("aicross-task07-fail-closed-{}", Uuid::new_v4()));
        let dispatch = root.join(".dispatch");
        let before = dispatch.join("run/node.before");
        fs::create_dir_all(&before).unwrap();
        let record = DispatchRecord {
            node: "node".into(), workdir: root.join("missing").to_string_lossy().into_owned(),
            before_dir: ".dispatch/run/node.before".into(), ended_at: "2026-01-01T00:00:00+00:00".into(),
            status: "ok".into(), ..DispatchRecord::default()
        };
        fs::write(dispatch.join("record.md"), format!("{}\n", serialize_frontmatter(&record).unwrap())).unwrap();
        assert_eq!(latest_before_dir(&root, "node"), None);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task06_rollback_before_runs_inside_run_one_before_redispatch() {
        if channels::resolve_binary("python", None).is_err() {
            eprintln!("跳过：PATH 中没有 python");
            return;
        }
        let root = std::env::temp_dir().join(format!("aicross-task06-run-one-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = project.join(".dispatch");
        let run_root = root.join("runs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        fs::write(project.join("tracked.txt"), "before").unwrap();
        let stub = root.join("cc_switch_stub.py");
        fs::write(&stub, "import pathlib\np=pathlib.Path.cwd()\n(p/'tracked.txt').write_text('first', encoding='utf-8')\n(p/'created.txt').write_text('new', encoding='utf-8')\nprint('first reply')\nprint('input=1 cache_read=0 output=1')\n").unwrap();
        let mut request = node(Vec::new());
        request.agent = "cc-switch".into();
        request.provider = "stub".into();
        request.model = "stub".into();
        request.role = "执行".into();
        request.visibility = "共享".into();
        request.write = "单写者".into();
        let settings = DispatchSettings { cc_switch_path: stub.to_string_lossy().into_owned(), ..DispatchSettings::default() };
        request.write = "worktree".into();
        let first = run_one("run-a", &project, &run_root, &log_dir, &settings, request.clone());
        assert_eq!(first.status, "ok", "{}", first.error);
        assert!(first.frontmatter.contains("before_dir: .dispatch/run-a/node-1.before"));
        fs::write(project.join("tracked.txt"), "half-finished").unwrap();
        fs::write(&stub, "import pathlib\np=pathlib.Path.cwd()\nassert (p/'tracked.txt').read_text(encoding='utf-8') == 'before'\nassert not (p/'created.txt').exists()\nprint('rollback observed')\nprint('input=1 cache_read=0 output=1')\n").unwrap();
        request.rollback_before = true;
        let second = run_one("run-b", &project, &run_root, &log_dir, &settings, request);
        assert_eq!(second.status, "ok", "{}", second.error);
        assert!(second.runner_note.contains("已回滚上次改动 2 文件"));
        assert_eq!(fs::read_to_string(project.join("tracked.txt")).unwrap(), "before");
        assert!(!project.join("created.txt").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task06_rollback_reports_over_eight_megabyte_snapshot_entries_as_skipped() {
        let root = std::env::temp_dir().join(format!("aicross-task06-large-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let before = HashMap::from([("large.bin".into(), SnapshotFile { bytes: None, size: SNAPSHOT_FILE_LIMIT + 1, modified_ns: 1 })]);
        let after = HashMap::from([("large.bin".into(), SnapshotFile { bytes: None, size: SNAPSHOT_FILE_LIMIT + 1, modified_ns: 2 })]);
        persist_before_snapshot(&root, &root.join(".dispatch"), "run", "node", &before, &after).unwrap();
        let summary = rollback_node_changes_core(&root, "run", "node").unwrap();
        assert_eq!(summary.skipped, ["large.bin"]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task06_duplicate_active_run_is_rejected_before_dispatch() {
        let root = std::env::temp_dir().join(format!("aicross-task06-active-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        struct Reset(&'static str);
        impl Drop for Reset { fn drop(&mut self) { if let Ok(mut active) = active_runs().lock() { active.remove(self.0); } } }
        active_runs().lock().unwrap().insert("same-run".into());
        let _reset = Reset("same-run");
        let mut request = stage_request(root.to_string_lossy().into_owned(), vec![node(Vec::new())]);
        request.run_id = "same-run".into();
        let error = dispatch_stage_core(request.clone(), None).unwrap_err();
        assert!(error.contains("该 run 已在进行"));
        drop(_reset);
        request.project_dir = root.join("missing").to_string_lossy().into_owned();
        let released = dispatch_stage_core(request.clone(), None).unwrap_err();
        assert!(!released.contains("该 run 已在进行"));
        let released_again = dispatch_stage_core(request, None).unwrap_err();
        assert!(!released_again.contains("该 run 已在进行"), "提前返回也应由 guard 释放 run_id");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task06_safe_relative_path_rejects_root_and_current_components() {
        assert!(safe_relative_path("/x").is_none());
        assert!(safe_relative_path("./x").is_none());
        assert!(safe_relative_path("ok/x").is_some());
    }

    #[test]
    fn task06_rollback_created_root_path_does_not_escape_project() {
        let root = std::env::temp_dir().join(format!("aicross-task06-rooted-{}", Uuid::new_v4()));
        let before = root.join(".dispatch/run/node.before");
        fs::create_dir_all(&before).unwrap();
        fs::write(before.join(".created"), "/x").unwrap();
        fs::write(before.join(".skipped"), "").unwrap();
        let summary = rollback_node_changes_core(&root, "run", "node").unwrap();
        assert_eq!(summary.skipped, ["/x"]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task06_snapshot_outside_project_is_absolute_and_colocated_with_records() {
        let root = std::env::temp_dir().join(format!("aicross-task06-external-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = root.join("runtime/dispatch");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let before = HashMap::from([("a.txt".into(), SnapshotFile { bytes: Some(b"old".to_vec()), ..SnapshotFile::default() })]);
        let after = HashMap::from([("a.txt".into(), SnapshotFile { bytes: Some(b"new".to_vec()), ..SnapshotFile::default() })]);
        let saved = persist_before_snapshot(&project, &log_dir, "run", "node", &before, &after).unwrap();
        assert!(Path::new(&saved).is_absolute());
        assert!(Path::new(&saved).starts_with(&log_dir));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task06_reconcile_parses_embedded_heading_and_legacy_record() {
        let root = std::env::temp_dir().join(format!("aicross-task06-parse-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let base = "---\nschema: aicross-dispatch/1\nrun_id: r\nstage: 1\nnode: n\nagent: codex\nprovider: ''\nmodel: m\nvendor: v\nbilling_source: b\nrole: r\ntier: t\nthinking: t\nvisibility: shared\nisolation: partial\nrunner: acp\nrunner_note: ''\nworkdir: x\nmaterials: []\nredacted: 0\ntruncated: false\nstatus: ok\nstarted_at: '2026-01-01T00:00:00+00:00'\nended_at: '2026-01-01T00:00:01+00:00'\nduration_ms: 1\ntokens_in: 2\ntokens_out: 3\ncache_read: 4\nidentity: i\nsession_id: s\nexit_code: 0\nerror: ''\n---\n\n## 任务全文\n\n含 ## 产物 JSON 字样\n\n## 回复正文\n\n正文";
        let legacy = root.join("legacy.md");
        fs::write(&legacy, base).unwrap();
        let (record, reply, artifacts) = parse_record_file(&legacy).unwrap();
        assert_eq!(reply, "正文");
        assert!(artifacts.is_none());
        assert_eq!((record.session_id.as_str(), record.runner.as_str(), record.tokens_in, record.tokens_out, record.cache_read), ("s", "acp", 2, 3, 4));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task06_acp_runner_note_append_preserves_rollback_note() {
        let mut note = "已回滚上次改动 1 文件".to_string();
        append_runner_note(&mut note, "ACP 会话已重置");
        assert_eq!(note, "已回滚上次改动 1 文件；ACP 会话已重置");
    }

    #[test]
    fn task06_reconcile_order_parses_timestamps_and_prefers_ok_on_tie() {
        assert!(record_order_key("2026-01-01T02:00:00+00:00", "error", Path::new("a.md"))
            > record_order_key("2026-01-01T09:00:00+08:00", "ok", Path::new("z.md")));
        assert!(record_order_key("2026-01-01T02:00:00+00:00", "ok", Path::new("a.md"))
            > record_order_key("2026-01-01T02:00:00+00:00", "error", Path::new("z.md")));
    }

    #[test]
    fn task06_snapshot_persist_failure_warns_without_flipping_success() {
        if channels::resolve_binary("python", None).is_err() { eprintln!("跳过：PATH 中没有 python"); return; }
        let root = std::env::temp_dir().join(format!("aicross-task06-persist-warning-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        fs::write(project.join("a.txt"), "old").unwrap();
        fs::write(log_dir.join("node-1.before"), "conflict").unwrap();
        let stub = root.join("stub.py");
        fs::write(&stub, "import pathlib\np=pathlib.Path.cwd()\n(p/'a.txt').write_text('new')\nprint('ok')\nprint('input=1 cache_read=0 output=1')\n").unwrap();
        let mut request = node(Vec::new());
        request.agent = "cc-switch".into(); request.provider = "stub".into(); request.model = "stub".into();
        request.role = "执行".into(); request.visibility = "共享".into(); request.write = "单写者".into();
        let settings = DispatchSettings { cc_switch_path: stub.to_string_lossy().into_owned(), ..DispatchSettings::default() };
        let result = run_one("run", &project, &root.join("runs"), &log_dir, &settings, request);
        assert_eq!(result.status, "ok", "{}", result.error);
        assert!(result.runner_note.contains("保存派发前快照失败"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task06_early_failure_does_not_replace_previous_before_snapshot() {
        if channels::resolve_binary("python", None).is_err() { eprintln!("跳过：PATH 中没有 python"); return; }
        let root = std::env::temp_dir().join(format!("aicross-task06-preserve-{}", Uuid::new_v4()));
        let project = root.join("project"); let log_dir = project.join(".dispatch");
        fs::create_dir_all(&log_dir).unwrap(); fs::write(project.join("a.txt"), "old").unwrap();
        let stub = root.join("stub.py");
        fs::write(&stub, "import pathlib\n(pathlib.Path.cwd()/'a.txt').write_text('new')\nprint('ok')\nprint('input=1 cache_read=0 output=1')\n").unwrap();
        let mut request = node(Vec::new());
        request.agent = "cc-switch".into(); request.provider = "stub".into(); request.model = "stub".into();
        request.role = "执行".into(); request.visibility = "共享".into(); request.write = "单写者".into();
        let settings = DispatchSettings { cc_switch_path: stub.to_string_lossy().into_owned(), ..DispatchSettings::default() };
        assert_eq!(run_one("run", &project, &root.join("runs"), &log_dir, &settings, request.clone()).status, "ok");
        request.agent = "not-a-real-agent".into();
        assert_eq!(run_one("run", &project, &root.join("runs"), &log_dir, &settings, request).status, "error");
        let summary = rollback_node_changes_core(&project, "run", "node-1").unwrap();
        assert_eq!(fs::read_to_string(project.join("a.txt")).unwrap(), "old");
        assert_eq!(summary.restored, ["a.txt"]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task06_rollback_reads_before_directory_beyond_snapshot_total_limit() {
        let root = std::env::temp_dir().join(format!("aicross-task06-total-{}", Uuid::new_v4()));
        let before = root.join(".dispatch/run/node.before"); fs::create_dir_all(&before).unwrap();
        let file = fs::File::create(before.join("large.bin")).unwrap(); file.set_len(ROLLBACK_TEST_TOTAL_LIMIT + 1).unwrap(); drop(file);
        fs::write(before.join(".created"), "").unwrap(); fs::write(before.join(".skipped"), "").unwrap();
        assert!(snapshot_directory_with_limits(&before, ROLLBACK_TEST_TOTAL_LIMIT * 2, ROLLBACK_TEST_TOTAL_LIMIT).is_err());
        let summary = rollback_node_changes_core(&root, "run", "node").unwrap();
        assert_eq!(fs::metadata(root.join("large.bin")).unwrap().len(), ROLLBACK_TEST_TOTAL_LIMIT + 1);
        assert_eq!(summary.restored, ["large.bin"]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task06_rollback_command_waits_for_project_write_lock() {
        let root = std::env::temp_dir().join(format!("aicross-task06-command-lock-{}", Uuid::new_v4()));
        let before = root.join(".dispatch/run/node.before"); fs::create_dir_all(&before).unwrap();
        fs::write(before.join("a.txt"), "old").unwrap(); fs::write(before.join(".created"), "").unwrap(); fs::write(before.join(".skipped"), "").unwrap();
        fs::write(root.join("a.txt"), "new").unwrap();
        let guard = acquire_project_write(&root, Duration::from_secs(1), "holder").unwrap();
        let path = root.to_string_lossy().into_owned();
        let handle = std::thread::spawn(move || rollback_node_changes(path, "run".into(), "node".into()).unwrap());
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(fs::read_to_string(root.join("a.txt")).unwrap(), "new");
        drop(guard);
        assert_eq!(handle.join().unwrap().restored, ["a.txt"]);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn task06_rollback_replaces_target_symlink_without_following_it() {
        use std::os::windows::fs::symlink_file;
        let root = std::env::temp_dir().join(format!("aicross-task06-link-{}", Uuid::new_v4()));
        let outside = root.with_extension("outside"); let before = root.join(".dispatch/run/node.before");
        fs::create_dir_all(&before).unwrap(); fs::write(before.join("a.txt"), "inside").unwrap();
        fs::write(before.join(".created"), "").unwrap(); fs::write(before.join(".skipped"), "").unwrap(); fs::write(&outside, "outside").unwrap();
        if let Err(error) = symlink_file(&outside, root.join("a.txt")) { eprintln!("跳过：无法创建符号链接：{error}"); let _ = fs::remove_dir_all(root); let _ = fs::remove_file(outside); return; }
        let summary = rollback_node_changes_core(&root, "run", "node").unwrap();
        assert_eq!(fs::read_to_string(&outside).unwrap(), "outside"); assert_eq!(fs::read_to_string(root.join("a.txt")).unwrap(), "inside");
        assert_eq!(summary.restored, ["a.txt"]);
        let _ = fs::remove_dir_all(root); let _ = fs::remove_file(outside);
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

    #[test]
    fn snapshot_builds_diff_and_changed_file_content() {
        let before = HashMap::from([(
            "a.py".into(),
            SnapshotFile {
                bytes: Some(b"old\n".to_vec()),
                ..SnapshotFile::default()
            },
        )]);
        let after = HashMap::from([
            (
                "a.py".into(),
                SnapshotFile {
                    bytes: Some(b"new\n".to_vec()),
                    ..SnapshotFile::default()
                },
            ),
            (
                "b.py".into(),
                SnapshotFile {
                    bytes: Some(b"added\n".to_vec()),
                    ..SnapshotFile::default()
                },
            ),
        ]);
        let artifacts = artifact_diff(&before, &after);
        assert!(artifacts.diff.contains("--- a/a.py"));
        assert!(artifacts.diff.contains("+++ b/b.py"));
        assert!(artifacts.diff.contains("@@ -0,0 +1,1 @@\n+added"));
        assert_eq!(artifacts.changed_files.len(), 2);
    }

    #[test]
    fn artifact_diff_covers_deleted_binary_large_and_oversized_markers() {
        let before = HashMap::from([
            ("deleted.txt".into(), SnapshotFile { bytes: Some(b"gone\n".to_vec()), ..SnapshotFile::default() }),
            ("binary.bin".into(), SnapshotFile { bytes: Some(vec![0xff, 0x00]), ..SnapshotFile::default() }),
            ("large.txt".into(), SnapshotFile { bytes: Some(vec![b'a'; 200 * 1024 + 1]), ..SnapshotFile::default() }),
            ("huge.dat".into(), SnapshotFile { bytes: None, size: SNAPSHOT_FILE_LIMIT + 1, modified_ns: 1 }),
        ]);
        let after = HashMap::from([
            ("binary.bin".into(), SnapshotFile { bytes: Some(vec![0xfe, 0x00]), ..SnapshotFile::default() }),
            ("large.txt".into(), SnapshotFile { bytes: Some(vec![b'b'; 200 * 1024 + 1]), ..SnapshotFile::default() }),
            ("huge.dat".into(), SnapshotFile { bytes: None, size: SNAPSHOT_FILE_LIMIT + 1, modified_ns: 2 }),
        ]);
        let artifacts = artifact_diff(&before, &after);
        assert!(artifacts.diff.contains("--- a/deleted.txt\n+++ /dev/null"));
        assert!(artifacts.diff.contains("Binary files a/binary.bin and b/binary.bin differ"));
        assert!(artifacts.diff.contains("Binary files a/huge.dat and b/huge.dat differ"));
        assert!(artifacts.diff.contains("文件：large.txt\n全文见 changed/"));
        assert!(!artifacts.changed_files.iter().any(|file| file.path == "large.txt"));
    }

    #[test]
    fn git_repository_uses_three_line_unified_diff() {
        let root = std::env::temp_dir().join(format!("aicross-git-diff-{}", Uuid::new_v4()));
        let project = root.join("project");
        let scratch = root.join("scratch");
        fs::create_dir_all(&project).unwrap();
        let init = run_process_registered(
            Path::new("git"),
            &["init".into()],
            None,
            Duration::from_secs(5),
            Some(&project),
            None,
        )
        .unwrap();
        assert_eq!(init.code, Some(0));
        let before_text = "unchanged-1\r\nunchanged-2\r\nunchanged-3\r\nold\r\nunchanged-5\r\nunchanged-6\r\nunchanged-7\r\nunchanged-8\r\n";
        let after_text = before_text.replace("old", "new");
        let before = HashMap::from([("a.txt".into(), SnapshotFile { bytes: Some(before_text.as_bytes().to_vec()), ..SnapshotFile::default() })]);
        let after = HashMap::from([("a.txt".into(), SnapshotFile { bytes: Some(after_text.into_bytes()), ..SnapshotFile::default() })]);
        let artifacts = artifact_diff_for_project(
            &project,
            &scratch,
            &before,
            &after,
            Duration::from_secs(5),
        )
        .unwrap();
        assert!(artifacts.diff.contains("@@ -1,7 +1,7 @@"));
        assert!(artifacts.diff.contains(" unchanged-1"));
        assert!(!artifacts.diff.contains(" unchanged-8"));
        assert!(artifacts.diff.contains("-old"));
        assert!(artifacts.diff.contains("+new"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn snapshot_marks_files_over_eight_megabytes_without_reading_content() {
        let root = std::env::temp_dir().join(format!("aicross-large-snapshot-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("large.bin");
        let file = fs::File::create(&path).unwrap();
        file.set_len(SNAPSHOT_FILE_LIMIT + 1).unwrap();
        let snapshot = snapshot_directory(&root).unwrap();
        let entry = snapshot.get("large.bin").unwrap();
        assert!(entry.bytes.is_none());
        assert_eq!(entry.size, SNAPSHOT_FILE_LIMIT + 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn snapshot_skips_excluded_directories_and_enforces_total_bytes() {
        let root = std::env::temp_dir().join(format!("aicross-limited-snapshot-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join(".venv")).unwrap();
        fs::create_dir_all(root.join("coverage")).unwrap();
        fs::write(root.join(".venv/secret.txt"), b"skip").unwrap();
        fs::write(root.join("coverage/report.txt"), b"skip").unwrap();
        fs::write(root.join("a.txt"), b"1234").unwrap();
        let snapshot = snapshot_directory_with_limits(&root, 16, 16).unwrap();
        assert_eq!(snapshot.keys().cloned().collect::<Vec<_>>(), vec!["a.txt"]);
        fs::write(root.join("b.txt"), b"5678").unwrap();
        assert!(snapshot_directory_with_limits(&root, 16, 7)
            .unwrap_err()
            .contains("总字节上限"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_write_lock_is_mutually_exclusive() {
        let root = std::env::temp_dir().join(format!("aicross-lock-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let first = acquire_project_write(&root, Duration::from_secs(1), "lock-first").unwrap();
        let path = root.clone();
        let (sender, receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            let _second = acquire_project_write(&path, Duration::from_secs(1), "lock-second").unwrap();
            sender.send(()).unwrap();
        });
        assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
        drop(first);
        receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        worker.join().unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_write_lock_wait_can_be_cancelled() {
        let root = std::env::temp_dir().join(format!("aicross-lock-cancel-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let first = acquire_project_write(&root, Duration::from_secs(2), "cancel-first").unwrap();
        let path = root.clone();
        let worker = thread::spawn(move || {
            acquire_project_write(&path, Duration::from_secs(2), "cancel-second")
                .map(|_| ())
        });
        thread::sleep(Duration::from_millis(50));
        mark_cancelled("cancel-second");
        let error = worker.join().unwrap().unwrap_err();
        assert!(error.contains("已取消"));
        clear_cancelled("cancel-second");
        drop(first);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn non_git_worktree_is_accounted_as_single_writer() {
        let mut request = node(Vec::new());
        request.write = "worktree".into();
        request.visibility = "共享".into();
        assert!(requires_single_writer(&request, true));
        assert!(!requires_single_writer(&request, false));
    }

    #[test]
    fn node_stage_is_used_for_result_and_record() {
        let root = std::env::temp_dir().join(format!("aicross-node-stage-{}", Uuid::new_v4()));
        let project = root.join("project");
        let run_root = root.join("runs");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let mut request = node(Vec::new());
        request.agent = "missing-agent".into();
        request.visibility = "共享".into();
        request.stage = 7;
        let result = run_one(
            "stage-run",
            &project,
            &run_root,
            &log_dir,
            &DispatchSettings::default(),
            request,
        );
        assert_eq!(result.stage, 7);
        assert!(result.frontmatter.contains("stage: 7"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn blind_acceptance_directory_contains_only_artifact_delivery() {
        let root = std::env::temp_dir().join(format!("aicross-artifact-{}", Uuid::new_v4()));
        let mut request = node(Vec::new());
        request.original_request = "逐字原始需求".into();
        request.upstream_artifacts = vec![DispatchArtifacts {
            diff: "--- a/a.py\n+++ b/a.py\n".into(),
            changed_files: vec![ChangedFile {
                path: "a.py".into(),
                content: "print('ok')\n".into(),
            }],
            stdout_summary: "done".into(),
            worktree: None,
            before_dir: String::new(),
        }];
        let (prompt, _, _) = prepare_outbound(&request, true, &root, true).unwrap();
        let mut entries = fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        entries.sort();
        for required in ["REQUEST.md", "changed", "diff.patch"] {
            assert!(entries.iter().any(|entry| entry == required));
        }
        for project_entry in ["AGENTS.md", "CLAUDE.md", ".dispatch", "fixtures"] {
            assert!(!entries.iter().any(|entry| entry == project_entry));
        }
        assert!(prompt.contains(channels::BLIND_ARTIFACT_NOTICE));
        assert!(prompt.contains("[diff.patch 全文]"));
        assert!(prompt.contains("--- a/a.py"));
        assert_eq!(
            fs::read_to_string(root.join("REQUEST.md")).unwrap(),
            "逐字原始需求"
        );
        assert!(root.join("changed/a.py").is_file());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn blind_acceptance_rejects_drive_relative_changed_path() {
        let root = std::env::temp_dir().join(format!("aicross-artifact-prefix-{}", Uuid::new_v4()));
        let mut request = node(Vec::new());
        request.upstream_artifacts = vec![DispatchArtifacts {
            changed_files: vec![ChangedFile { path: "C:escape.txt".into(), content: "no".into() }],
            ..DispatchArtifacts::default()
        }];
        let error = prepare_outbound(&request, true, &root, true).unwrap_err();
        assert!(error.contains("产物路径不是安全相对路径"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn broad_redaction_warning_only_counts_unquoted_whole_lines() {
        assert_eq!(
            broad_redaction_count("token=ordinary prose\n{\"token\":\"secret\"}"),
            1
        );
    }

    // ── TASK-05B 主备/熔断 ────────────────────────────────────────────────

    #[test]
    fn fallback_predicate_only_triggers_on_channel_failures() {
        for status in ["error", "timeout", "identity_mismatch", "quota_exhausted"] {
            assert!(should_try_fallback(status, true), "{status} 应触发备选");
        }
        for status in ["ok", "validation_failed"] {
            assert!(!should_try_fallback(status, true), "{status} 不应触发备选");
        }
        assert!(!should_try_fallback("error", false));
    }

    #[test]
    fn task14_r1_quota_classification_only_accepts_error_fields_and_never_identity_mismatch() {
        assert!(classify_attempt_quota("error", "HTTP 429 too many requests", "", Some(1)).is_some());
        assert!(classify_attempt_quota("identity_mismatch", "HTTP 429 too many requests", "", Some(1)).is_none());
        assert!(classify_attempt_quota("error", "ordinary command error", "", Some(1)).is_none());
        // 函数没有 stdout 参数：模型正文无法进入额度分类。
        assert!(classify_attempt_quota("error", "", "output parse failed", Some(1)).is_none());
    }

    #[test]
    fn task14_quota_fallback_order_runs_fake_channel_through_three_attempts() {
        if channels::resolve_binary("python", None).is_err() { return; }
        let root = std::env::temp_dir().join(format!("aicross-quota-fallback-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let marker = root.join("calls.txt");
        let stub = root.join("cc_switch_stub.py");
        fs::write(&stub, format!(
            "import pathlib,sys\np=sys.argv[sys.argv.index('--provider')+1]\nf=pathlib.Path(r'{}')\nf.write_text((f.read_text() if f.exists() else '')+p+'\\n')\nif p != 'p2':\n print('session limit; resets at 2099-01-01T00:00:00+08:00', file=sys.stderr)\n sys.exit(1)\nprint('final answer')\n",
            marker.display()
        )).unwrap();

        let mut primary = crate::resources::ResourceModel { agent: "cc-switch".into(), provider: "p0".into(), id: "main".into(), ..Default::default() };
        primary.fallback_order = vec![
            "cc-switch/skip/skipped".into(),
            "cc-switch/p2/backup-b".into(),
        ];
        let skipped = crate::resources::ResourceModel {
            agent: "cc-switch".into(), provider: "skip".into(), id: "skipped".into(),
            quota_exhausted_until: Some("2099-02-01T00:00:00+08:00".into()), ..Default::default()
        };
        let backup_a = crate::resources::ResourceModel { agent: "cc-switch".into(), provider: "p1".into(), id: "backup-a".into(), ..Default::default() };
        let backup_b = crate::resources::ResourceModel { agent: "cc-switch".into(), provider: "p2".into(), id: "backup-b".into(), ..Default::default() };
        RESOURCE_MODELS_STUB.with(|cell| *cell.borrow_mut() = Some(vec![primary, skipped, backup_a, backup_b]));
        QUOTA_MARKS.with(|cell| cell.borrow_mut().clear());

        let mut request = node(Vec::new());
        request.agent = "cc-switch".into();
        request.provider = "p0".into();
        request.model = "main".into();
        request.visibility = "共享".into();
        request.write = "只读".into();
        request.fallback = Some(FallbackSpec { agent: "cc-switch".into(), provider: "p1".into(), model: "backup-a".into(), vendor: String::new() });
        let settings = DispatchSettings { cc_switch_path: stub.to_string_lossy().into_owned(), ..Default::default() };
        let result = run_one("quota-three", &project, &root.join("runs"), &log_dir, &settings, request);
        RESOURCE_MODELS_STUB.with(|cell| *cell.borrow_mut() = None);

        assert_eq!(result.status, "ok", "{}", result.error);
        assert_eq!(fs::read_to_string(&marker).unwrap().lines().collect::<Vec<_>>(), ["p0", "p1", "p2"]);
        assert_eq!(result.fallback_from, "cc-switch/backup-a");
        assert_eq!(result.fallback_to, "cc-switch/backup-b");
        assert!(result.runner_note.contains("主 cc-switch/main 额度用尽 → 备 cc-switch/backup-a 额度用尽"), "{}", result.runner_note);
        assert!(result.runner_note.contains("跳过 cc-switch/skipped（额度用尽至 2099-02-01T00:00:00+08:00）"), "{}", result.runner_note);
        assert!(result.runner_note.ends_with("备 cc-switch/backup-b"), "{}", result.runner_note);
        assert_eq!(QUOTA_MARKS.with(|cell| cell.borrow().len()), 2);
        assert_eq!(fs::read_dir(&log_dir).unwrap().filter_map(Result::ok).count(), 3);
        let records = fs::read_dir(&log_dir).unwrap().filter_map(Result::ok)
            .map(|entry| fs::read_to_string(entry.path()).unwrap()).collect::<Vec<_>>();
        assert!(records.iter().any(|body| body.contains("attempt: 备\nattempt_index: 1")));
        assert!(records.iter().any(|body| body.contains("attempt: 备\nattempt_index: 2")));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task14_reset_helpers_choose_earliest_active_time() {
        let earliest = earliest_reset(vec![
            "2099-01-02T00:00:00+08:00".into(),
            "2099-01-01T12:00:00+08:00".into(),
        ]);
        assert_eq!(earliest.as_deref(), Some("2099-01-01T12:00:00+08:00"));
        assert!(quota_until_active(Some("2099-01-01T00:00:00+08:00")));
        assert!(!quota_until_active(Some("2000-01-01T00:00:00+08:00")));
    }

    #[test]
    fn task14_r1_skipped_candidate_reset_is_returned_when_no_attempt_has_one() {
        if channels::resolve_binary("python", None).is_err() { return; }
        let root = std::env::temp_dir().join(format!("aicross-quota-skipped-reset-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let marker = root.join("calls.txt");
        let stub = root.join("cc_switch_stub.py");
        fs::write(&stub, format!(
            "import pathlib,sys\np=pathlib.Path(r'{}')\np.write_text((p.read_text() if p.exists() else '')+'called\\n')\nprint('usage limit reached', file=sys.stderr)\nsys.exit(1)\n",
            marker.display()
        )).unwrap();
        let mut primary = crate::resources::ResourceModel {
            agent: "cc-switch".into(), provider: "p0".into(), id: "main".into(), ..Default::default()
        };
        primary.fallback_order = vec!["cc-switch/p1/blocked".into()];
        let blocked = crate::resources::ResourceModel {
            agent: "cc-switch".into(), provider: "p1".into(), id: "blocked".into(),
            quota_exhausted_until: Some("2099-03-01T00:00:00+08:00".into()), ..Default::default()
        };
        RESOURCE_MODELS_STUB.with(|cell| *cell.borrow_mut() = Some(vec![primary, blocked]));
        let mut request = node(Vec::new());
        request.agent = "cc-switch".into(); request.provider = "p0".into(); request.model = "main".into();
        request.visibility = "共享".into(); request.write = "只读".into();
        let settings = DispatchSettings { cc_switch_path: stub.to_string_lossy().into_owned(), ..Default::default() };
        let result = run_one("quota-skipped-reset", &project, &root.join("runs"), &log_dir, &settings, request);
        RESOURCE_MODELS_STUB.with(|cell| *cell.borrow_mut() = None);
        assert_eq!(result.status, "quota_exhausted");
        assert_eq!(result.quota_resets_at.as_deref(), Some("2099-03-01T00:00:00+08:00"));
        assert!(result.runner_note.contains("跳过 cc-switch/blocked（额度用尽至 2099-03-01T00:00:00+08:00）"), "{}", result.runner_note);
        assert_eq!(fs::read_to_string(marker).unwrap().lines().collect::<Vec<_>>(), ["called"]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task14_exhausted_fake_channels_return_quota_not_error() {
        if channels::resolve_binary("python", None).is_err() { return; }
        let root = std::env::temp_dir().join(format!("aicross-quota-terminal-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let stub = root.join("cc_switch_stub.py");
        fs::write(&stub, "import sys\np=sys.argv[sys.argv.index('--provider')+1]\nreset='2099-01-02T00:00:00+08:00' if p == 'p0' else '2099-01-01T00:00:00+08:00'\nprint('usage limit; resets at '+reset, file=sys.stderr)\nsys.exit(1)\n").unwrap();
        let mut primary = crate::resources::ResourceModel { agent: "cc-switch".into(), provider: "p0".into(), id: "main".into(), ..Default::default() };
        primary.fallback_order = vec!["cc-switch/p1/backup".into()];
        let backup = crate::resources::ResourceModel { agent: "cc-switch".into(), provider: "p1".into(), id: "backup".into(), ..Default::default() };
        RESOURCE_MODELS_STUB.with(|cell| *cell.borrow_mut() = Some(vec![primary, backup]));
        let mut request = node(Vec::new());
        request.agent = "cc-switch".into(); request.provider = "p0".into(); request.model = "main".into();
        request.visibility = "共享".into(); request.write = "只读".into();
        let settings = DispatchSettings { cc_switch_path: stub.to_string_lossy().into_owned(), ..Default::default() };
        let result = run_one("quota-terminal", &project, &root.join("runs"), &log_dir, &settings, request);
        RESOURCE_MODELS_STUB.with(|cell| *cell.borrow_mut() = None);
        assert_eq!(result.status, "quota_exhausted");
        assert_eq!(result.quota_resets_at.as_deref(), Some("2099-01-01T00:00:00+08:00"));
        assert_eq!(result.fallback_from, "cc-switch/main");
        assert_eq!(result.fallback_to, "cc-switch/backup");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task14_r1_writable_quota_rolls_back_before_fallback() {
        if channels::resolve_binary("python", None).is_err() { return; }
        let root = std::env::temp_dir().join(format!("aicross-quota-writable-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = project.join(".dispatch");
        fs::create_dir_all(&log_dir).unwrap();
        fs::write(project.join("state.txt"), "original").unwrap();
        let marker = root.join("calls.txt");
        let stub = root.join("cc_switch_stub.py");
        fs::write(&stub, format!(
            "import pathlib,sys\np=sys.argv[sys.argv.index('--provider')+1]\nf=pathlib.Path(r'{}')\nf.write_text((f.read_text() if f.exists() else '')+p+'\\n')\ns=pathlib.Path('state.txt')\nold=s.read_text()\ns.write_text(p+'|saw='+old)\nif p == 'p0':\n print('HTTP 429 too many requests; resets at 2099-01-01T00:00:00+08:00', file=sys.stderr)\n sys.exit(1)\nprint('fallback answer')\n",
            marker.display()
        )).unwrap();
        let mut request = node(Vec::new());
        request.agent = "cc-switch".into(); request.provider = "p0".into(); request.model = "main".into();
        request.visibility = "共享".into(); request.write = "单写者".into();
        request.fallback = Some(FallbackSpec { agent: "cc-switch".into(), provider: "p1".into(), model: "backup".into(), vendor: String::new() });
        let settings = DispatchSettings { cc_switch_path: stub.to_string_lossy().into_owned(), ..Default::default() };
        let result = run_one("quota-writable", &project, &root.join("runs"), &log_dir, &settings, request);
        assert_eq!(result.status, "ok", "{}", result.error);
        assert_eq!(fs::read_to_string(project.join("state.txt")).unwrap(), "p1|saw=original");
        assert_eq!(fs::read_to_string(marker).unwrap().lines().collect::<Vec<_>>(), ["p0", "p1"]);
        assert!(result.runner_note.contains("已回滚 node-1 本次改动"), "{}", result.runner_note);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task14_r2_writable_quota_without_changes_falls_back_without_rollback() {
        if channels::resolve_binary("python", None).is_err() { return; }
        let root = std::env::temp_dir().join(format!("aicross-quota-nochange-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = project.join(".dispatch");
        fs::create_dir_all(&log_dir).unwrap();
        fs::write(project.join("state.txt"), "original").unwrap();
        let stub = root.join("cc_switch_stub.py");
        // 主目标第一句就撞额度、没碰任何文件；备选正常写。
        fs::write(&stub, "import pathlib,sys\np=sys.argv[sys.argv.index('--provider')+1]\nif p == 'p0':\n print('HTTP 429 too many requests; resets at 2099-01-01T00:00:00+08:00', file=sys.stderr)\n sys.exit(1)\ns=pathlib.Path('state.txt')\ns.write_text(p+'|saw='+s.read_text())\nprint('fallback answer')\n").unwrap();
        let mut request = node(Vec::new());
        request.agent = "cc-switch".into(); request.provider = "p0".into(); request.model = "main".into();
        request.visibility = "共享".into(); request.write = "单写者".into();
        request.fallback = Some(FallbackSpec { agent: "cc-switch".into(), provider: "p1".into(), model: "backup".into(), vendor: String::new() });
        let settings = DispatchSettings { cc_switch_path: stub.to_string_lossy().into_owned(), ..Default::default() };
        let result = run_one("quota-nochange", &project, &root.join("runs"), &log_dir, &settings, request);
        assert_eq!(result.status, "ok", "{} / {}", result.error, result.runner_note);
        assert_eq!(result.fallback_to, "cc-switch/backup");
        assert!(result.fallback_skipped.is_empty(), "{}", result.fallback_skipped);
        assert!(result.runner_note.contains("本次没有改动，无需回滚"), "{}", result.runner_note);
        assert_eq!(fs::read_to_string(project.join("state.txt")).unwrap(), "p1|saw=original");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task14_r2_quota_rollback_ignores_stale_snapshot_from_earlier_run() {
        if channels::resolve_binary("python", None).is_err() { return; }
        let root = std::env::temp_dir().join(format!("aicross-quota-stale-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = project.join(".dispatch");
        fs::create_dir_all(&log_dir).unwrap();
        fs::write(project.join("state.txt"), "original").unwrap();
        let flag = root.join("first-done.flag");
        let stub = root.join("cc_switch_stub.py");
        // 第一轮：主目标先改文件再撞额度（落下快照）；第二轮：主目标不碰文件直接撞额度。
        fs::write(&stub, format!(
            "import pathlib,sys\np=sys.argv[sys.argv.index('--provider')+1]\nflag=pathlib.Path(r'{}')\ns=pathlib.Path('state.txt')\nif p == 'p0':\n if not flag.exists():\n  s.write_text('p0-dirty')\n  flag.write_text('1')\n print('HTTP 429 too many requests; resets at 2099-01-01T00:00:00+08:00', file=sys.stderr)\n sys.exit(1)\ns.write_text(p+'|saw='+s.read_text())\nprint('fallback answer')\n",
            flag.display()
        )).unwrap();
        let mut request = node(Vec::new());
        request.agent = "cc-switch".into(); request.provider = "p0".into(); request.model = "main".into();
        request.visibility = "共享".into(); request.write = "单写者".into();
        request.fallback = Some(FallbackSpec { agent: "cc-switch".into(), provider: "p1".into(), model: "backup".into(), vendor: String::new() });
        let settings = DispatchSettings { cc_switch_path: stub.to_string_lossy().into_owned(), ..Default::default() };
        let first = run_one("quota-stale-1", &project, &root.join("runs"), &log_dir, &settings, request.clone());
        assert_eq!(first.status, "ok", "{} / {}", first.error, first.runner_note);
        assert!(first.runner_note.contains("已回滚 node-1 本次改动"), "{}", first.runner_note);
        assert_eq!(fs::read_to_string(project.join("state.txt")).unwrap(), "p1|saw=original");
        // 用户在两次派发之间手改了文件；第二轮本次无改动，历史快照绝不能把它还原成 original。
        fs::write(project.join("state.txt"), "user-edit").unwrap();
        let second = run_one("quota-stale-2", &project, &root.join("runs"), &log_dir, &settings, request);
        assert_eq!(second.status, "ok", "{} / {}", second.error, second.runner_note);
        assert!(second.runner_note.contains("本次没有改动，无需回滚"), "{}", second.runner_note);
        assert_eq!(fs::read_to_string(project.join("state.txt")).unwrap(), "p1|saw=user-edit");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task14_r1_first_use_quota_switches_to_fallback_without_entering_primary_channel() {
        if channels::resolve_binary("python", None).is_err() { return; }
        let root = std::env::temp_dir().join(format!("aicross-first-use-quota-dispatch-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let marker = root.join("calls.txt");
        let stub = root.join("cc_switch_stub.py");
        fs::write(&stub, format!(
            "import pathlib,sys\np=sys.argv[sys.argv.index('--provider')+1]\npathlib.Path(r'{}').write_text(p)\nprint('fallback answer')\n",
            marker.display()
        )).unwrap();
        let mut request = node(Vec::new());
        request.agent = "cc-switch".into(); request.provider = "p0".into(); request.model = "main".into();
        request.visibility = "共享".into(); request.write = "只读".into();
        request.fallback = Some(FallbackSpec { agent: "cc-switch".into(), provider: "p1".into(), model: "backup".into(), vendor: String::new() });
        FIRST_USE_STUB.with(|cell| *cell.borrow_mut() = Some(crate::resources::FirstUseVerification::QuotaExhausted {
            resets_at: Some("2099-01-01T00:00:00+08:00".into()),
        }));
        QUOTA_MARKS.with(|cell| cell.borrow_mut().clear());
        let settings = DispatchSettings { cc_switch_path: stub.to_string_lossy().into_owned(), ..Default::default() };
        let result = run_one("first-use-quota", &project, &root.join("runs"), &log_dir, &settings, request);
        assert_eq!(result.status, "ok", "{}", result.error);
        assert_eq!(result.fallback_to, "cc-switch/backup");
        assert_eq!(fs::read_to_string(&marker).unwrap(), "p1");
        assert_eq!(QUOTA_MARKS.with(|cell| cell.borrow().len()), 1);
        assert!(result.runner_note.contains("主 cc-switch/main 额度用尽"), "{}", result.runner_note);
        let records = fs::read_dir(&log_dir).unwrap().filter_map(Result::ok)
            .map(|entry| fs::read_to_string(entry.path()).unwrap()).collect::<Vec<_>>();
        assert!(records.iter().any(|body| body.contains("首次派发自动验证命中额度限制")));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task14_r1_writable_quota_stops_when_rollback_fails() {
        if channels::resolve_binary("python", None).is_err() { return; }
        let root = std::env::temp_dir().join(format!("aicross-quota-rollback-fail-{}", Uuid::new_v4()));
        let project = root.join("project");
        let log_dir = project.join(".dispatch");
        fs::create_dir_all(&log_dir).unwrap();
        fs::write(project.join("state.txt"), "original").unwrap();
        let marker = root.join("calls.txt");
        let stub = root.join("cc_switch_stub.py");
        fs::write(&stub, format!(
            "import pathlib,sys\np=sys.argv[sys.argv.index('--provider')+1]\nf=pathlib.Path(r'{}')\nf.write_text((f.read_text() if f.exists() else '')+p+'\\n')\npathlib.Path('state.txt').write_text(p)\nprint('HTTP 429 too many requests; resets at 2099-01-01T00:00:00+08:00', file=sys.stderr)\nsys.exit(1)\n",
            marker.display()
        )).unwrap();
        let mut request = node(Vec::new());
        request.agent = "cc-switch".into(); request.provider = "p0".into(); request.model = "main".into();
        request.visibility = "共享".into(); request.write = "单写者".into();
        request.fallback = Some(FallbackSpec { agent: "cc-switch".into(), provider: "p1".into(), model: "backup".into(), vendor: String::new() });
        let settings = DispatchSettings { cc_switch_path: stub.to_string_lossy().into_owned(), ..Default::default() };
        QUOTA_ROLLBACK_FAILURE.with(|cell| cell.set(true));
        let result = run_one("quota-rollback-fail", &project, &root.join("runs"), &log_dir, &settings, request);
        QUOTA_ROLLBACK_FAILURE.with(|cell| cell.set(false));
        assert_eq!(result.status, "quota_exhausted");
        assert_eq!(result.fallback_skipped, "回滚失败");
        assert!(result.fallback_to.is_empty());
        assert_eq!(fs::read_to_string(marker).unwrap().lines().collect::<Vec<_>>(), ["p0"]);
        assert!(result.runner_note.contains("回滚本次改动失败"), "{}", result.runner_note);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task06_fallback_reruns_once_uses_fields_without_reply_marker() {
        let root = std::env::temp_dir().join(format!("aicross-fallback-{}", Uuid::new_v4()));
        let project = root.join("project");
        let run_root = root.join("runs");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let mut request = node(Vec::new());
        request.agent = "missing-agent".into();
        request.model = "m-main".into();
        request.visibility = "共享".into();
        request.fallback = Some(FallbackSpec {
            agent: "missing-agent-b".into(),
            provider: String::new(),
            model: "m-back".into(),
            ..FallbackSpec::default()
        });
        let result = run_one(
            "fb-run",
            &project,
            &run_root,
            &log_dir,
            &DispatchSettings::default(),
            request,
        );
        // 主备都不可用（agent 不存在，构建调用即失败），但最终结果带备选路由
        assert_eq!(result.status, "error");
        assert_eq!(result.fallback_from, "missing-agent/m-main");
        assert_eq!(result.fallback_to, "missing-agent-b/m-back");
        assert!(!result.text.starts_with("（已切备选："));
        assert!(result.frontmatter.contains("attempt: 备"));
        // R2-1：备选作答的 session 属于备选通道，不写回结果（节点 target 仍是主目标）
        assert!(result.session_id.is_empty(), "备选作答结果不得带回 session_id");
        // 留痕两条：主一条、备一条
        let records: Vec<String> = fs::read_dir(&log_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().and_then(|e| e.to_str()) == Some("md"))
            .map(|entry| fs::read_to_string(entry.path()).unwrap())
            .collect();
        assert_eq!(records.len(), 2, "应各留一条主/备记录");
        assert!(records.iter().any(|body| body.contains("attempt: 主")));
        assert!(records.iter().any(|body| body.contains("attempt: 备")));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fallback_blind_attempt_gets_fresh_material_dir() {
        let root = std::env::temp_dir().join(format!("aicross-fallback-blind-{}", Uuid::new_v4()));
        let project = root.join("project");
        let run_root = root.join("runs");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let material = project.join("m.txt");
        fs::write(&material, "材料").unwrap();
        let mut request = node(vec![material.to_string_lossy().into_owned()]);
        request.agent = "missing-agent".into();
        request.model = "m-main".into();
        // node() 默认 visibility=盲：第一次尝试会把材料铺进隔离目录，
        // 备选重跑前必须清掉，否则第二次 prepare_outbound 拒非空目录。
        request.fallback = Some(FallbackSpec {
            agent: "missing-agent-b".into(),
            provider: String::new(),
            model: "m-back".into(),
            ..FallbackSpec::default()
        });
        let result = run_one(
            "fb-blind-run",
            &project,
            &run_root,
            &log_dir,
            &DispatchSettings::default(),
            request,
        );
        assert_eq!(result.status, "error");
        assert!(!result.error.contains("盲审目录不是空目录"), "备选不应死在隔离目录上：{}", result.error);
        let records = fs::read_dir(&log_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().and_then(|e| e.to_str()) == Some("md"))
            .count();
        assert_eq!(records, 2);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writable_non_quota_failure_does_not_switch_fallback() {
        let root = std::env::temp_dir().join(format!("aicross-fallback-writable-{}", Uuid::new_v4()));
        let project = root.join("project");
        let run_root = root.join("runs");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let mut request = node(Vec::new());
        request.agent = "missing-agent".into();
        request.model = "m-main".into();
        request.visibility = "共享".into();
        request.write = "单写者".into();
        request.fallback = Some(FallbackSpec {
            agent: "missing-agent-b".into(),
            provider: String::new(),
            model: "m-back".into(),
            ..FallbackSpec::default()
        });
        let result = run_one(
            "fb-writable-run",
            &project,
            &run_root,
            &log_dir,
            &DispatchSettings::default(),
            request,
        );
        // 非额度类失败保持 05B 语义：可写节点不切备选。
        assert_eq!(result.status, "error");
        assert_eq!(result.fallback_skipped, "可写节点");
        assert!(result.fallback_from.is_empty());
        assert!(result.fallback_to.is_empty());
        assert!(result.frontmatter.contains("attempt: 主"));
        assert!(result.frontmatter.contains("fallback_skipped: 可写节点"));
        let bodies: Vec<String> = fs::read_dir(&log_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().and_then(|e| e.to_str()) == Some("md"))
            .map(|entry| fs::read_to_string(entry.path()).unwrap())
            .collect();
        assert_eq!(bodies.len(), 1, "非额度失败只能有主尝试一条留痕");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fallback_reruns_for_blind_node_and_marks_no_skip() {
        let root = std::env::temp_dir().join(format!("aicross-fallback-blind-rerun-{}", Uuid::new_v4()));
        let project = root.join("project");
        let run_root = root.join("runs");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let material = project.join("m.txt");
        fs::write(&material, "材料").unwrap();
        // node() 默认 visibility=盲 / write=只读：主失败照常切备选
        let mut request = node(vec![material.to_string_lossy().into_owned()]);
        request.agent = "missing-agent".into();
        request.model = "m-main".into();
        request.fallback = Some(FallbackSpec {
            agent: "missing-agent-b".into(),
            provider: String::new(),
            model: "m-back".into(),
            ..FallbackSpec::default()
        });
        let result = run_one(
            "fb-blind-rerun",
            &project,
            &run_root,
            &log_dir,
            &DispatchSettings::default(),
            request,
        );
        assert_eq!(result.status, "error");
        assert!(result.fallback_skipped.is_empty(), "盲节点不得标可写跳过");
        assert_eq!(result.fallback_from, "missing-agent/m-main");
        assert_eq!(result.fallback_to, "missing-agent-b/m-back");
        assert!(result.frontmatter.contains("attempt: 备"));
        assert!(!result.frontmatter.contains("fallback_skipped: 可写节点"), "切了备选不得标跳过");
        let records = fs::read_dir(&log_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().and_then(|e| e.to_str()) == Some("md"))
            .count();
        assert_eq!(records, 2, "盲节点备选重跑应各留一条主/备记录");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fallback_record_uses_fallback_billing_and_vendor() {
        let root = std::env::temp_dir().join(format!("aicross-fallback-billing-{}", Uuid::new_v4()));
        let project = root.join("project");
        let run_root = root.join("runs");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let mut request = node(Vec::new());
        request.agent = "missing-agent".into();
        request.model = "m-main".into();
        request.visibility = "共享".into();
        request.write = "只读".into();
        request.vendor = "Anthropic".into();
        request.billing_source = "官方订阅".into();
        request.fallback = Some(FallbackSpec {
            agent: "missing-agent-b".into(),
            provider: "deepseek".into(),
            model: "deepseek-v4-flash".into(),
            ..FallbackSpec::default()
        });
        let result = run_one(
            "fb-billing-run",
            &project,
            &run_root,
            &log_dir,
            &DispatchSettings::default(),
            request,
        );
        assert_eq!(result.fallback_to, "missing-agent-b/deepseek-v4-flash");
        let bodies: Vec<String> = fs::read_dir(&log_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().and_then(|e| e.to_str()) == Some("md"))
            .map(|entry| fs::read_to_string(entry.path()).unwrap())
            .collect();
        assert_eq!(bodies.len(), 2);
        let main = bodies.iter().find(|body| body.contains("attempt: 主")).unwrap();
        let backup = bodies.iter().find(|body| body.contains("attempt: 备")).unwrap();
        // 主留痕用主目标口径；备选留痕用备选目标的 vendor/billing，不抄主目标
        assert!(main.contains("vendor: Anthropic"));
        assert!(main.contains("billing_source: 官方订阅"));
        assert!(backup.contains("vendor: DeepSeek"));
        assert!(backup.contains("billing_source: deepseek"));
        assert!(!backup.contains("官方订阅"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fallback_agent_blocks_same_agent_node_from_running_concurrently() {
        let root = std::env::temp_dir().join(format!("aicross-fallback-serial-{}", Uuid::new_v4()));
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        // A 的备选 agent 与 B 的主 agent 相同：B 必须等 A 整个结束才能开始。
        // A 是单写者且写锁被本测试持有，会阻塞到写锁超时（1 秒），
        // 若 B 与 A 并发，B（missing agent 秒败）的开始时间会早于 A 的结束时间。
        let _guard = acquire_project_write(&project, Duration::from_secs(10), "serial-hold").unwrap();
        let mut a = node(Vec::new());
        a.node_id = "node-a".into();
        a.agent = "missing-a".into();
        a.model = "m-a".into();
        a.role = "执行".into();
        a.visibility = "共享".into();
        a.write = "单写者".into();
        a.fallback = Some(FallbackSpec {
            agent: "missing-b".into(),
            provider: String::new(),
            model: "m-b".into(),
            ..FallbackSpec::default()
        });
        let mut b = node(Vec::new());
        b.node_id = "node-b".into();
        b.agent = "missing-b".into();
        b.model = "m-b".into();
        b.visibility = "共享".into();
        b.write = "只读".into();
        let mut settings = DispatchSettings::default();
        settings.timeout_secs = 1;
        settings.concurrency = 3;
        let result = dispatch_stage_core(
            DispatchStageRequest {
                run_id: format!("serial-run-{}", Uuid::new_v4()),
                stage: 1,
                project_dir: project.to_string_lossy().into_owned(),
                nodes: vec![a, b],
                settings,
            },
            None,
        )
        .unwrap();
        assert_eq!(result.results.len(), 2);
        let time_of = |node_id: &str, key: &str| {
            let item = result.results.iter().find(|item| item.node_id == node_id).unwrap();
            item.frontmatter
                .lines()
                .find_map(|line| line.strip_prefix(&format!("{key}: ")))
                .unwrap_or_else(|| panic!("{node_id} 留痕缺 {key}：\n{}", item.frontmatter))
                .to_string()
        };
        assert!(time_of("node-b", "started_at") >= time_of("node-a", "ended_at"),
            "B 与 A 的备选 agent 相同，不得在 A 结束前开始");
        let _ = fs::remove_dir_all(root);
    }

    // R2-4：前端随请求下发真实 vendor 时，备选留痕直接用下发值，不靠 infer_vendor 猜
    // （k3-256k 之类模型 ID 推断不出厂商）。
    #[test]
    fn fallback_record_prefers_dispatched_vendor_over_inference() {
        let root = std::env::temp_dir().join(format!("aicross-fallback-vendor-{}", Uuid::new_v4()));
        let project = root.join("project");
        let run_root = root.join("runs");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let mut request = node(Vec::new());
        request.agent = "missing-agent".into();
        request.model = "m-main".into();
        request.visibility = "共享".into();
        request.write = "只读".into();
        request.fallback = Some(FallbackSpec {
            agent: "missing-agent-b".into(),
            provider: "kimi".into(),
            model: "k3-256k".into(),
            vendor: "Moonshot AI".into(),
        });
        let result = run_one(
            "fb-vendor-run",
            &project,
            &run_root,
            &log_dir,
            &DispatchSettings::default(),
            request,
        );
        assert_eq!(result.fallback_to, "missing-agent-b/k3-256k");
        let bodies: Vec<String> = fs::read_dir(&log_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().and_then(|e| e.to_str()) == Some("md"))
            .map(|entry| fs::read_to_string(entry.path()).unwrap())
            .collect();
        assert_eq!(bodies.len(), 2);
        let backup = bodies.iter().find(|body| body.contains("attempt: 备")).unwrap();
        assert!(backup.contains("vendor: Moonshot AI"), "备选留痕用下发的 vendor：{backup}");
        let main = bodies.iter().find(|body| body.contains("attempt: 主")).unwrap();
        assert!(!main.contains("Moonshot AI"), "主留痕不受备选 vendor 影响");
        let _ = fs::remove_dir_all(root);
    }

    // R2-3：主/备两次尝试的原始输出转存路径不同且都存在（备选不再覆盖主尝试）。
    #[test]
    fn task14_r1_production_backup_labels_keep_all_raw_dumps_separate() {
        let root = std::env::temp_dir().join(format!("aicross-rawdump-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let large = "x".repeat(40 * 1024);
        let primary = record_stdout(&large, &root, "node-1", "主");
        let backup_one = record_stdout(&large, &root, "node-1", "备 1");
        let backup_two = record_stdout(&large, &root, "node-1", "备 2");
        assert_ne!(primary, backup_one);
        assert_ne!(backup_one, backup_two);
        assert!(primary.contains("node-1.raw.txt"));
        assert!(backup_one.contains("node-1.备1.raw.txt"));
        assert!(backup_two.contains("node-1.备2.raw.txt"));
        assert_eq!(fs::read_to_string(root.join("node-1.raw.txt")).unwrap(), large);
        assert_eq!(fs::read_to_string(root.join("node-1.备1.raw.txt")).unwrap(), large);
        assert_eq!(fs::read_to_string(root.join("node-1.备2.raw.txt")).unwrap(), large);
        let _ = fs::remove_dir_all(root);
    }

    // R2-7：仲裁角色与验收/验证一样由 Rust 强制只读（此前只有前端 validateNode 提示）。
    #[test]
    fn arbitration_role_is_forced_readonly_by_engine() {
        let root = std::env::temp_dir().join(format!("aicross-arb-readonly-{}", Uuid::new_v4()));
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        let mut arb = node(Vec::new());
        arb.agent = "missing-agent".into();
        arb.model = "m-arb".into();
        arb.role = "仲裁".into();
        arb.visibility = "共享".into();
        arb.write = "单写者".into();
        let rejected = dispatch_stage_core(
            stage_request(project.to_string_lossy().into_owned(), vec![arb]),
            None,
        )
        .unwrap();
        assert_eq!(rejected.results[0].status, "error");
        assert!(
            rejected.results[0].error.contains("「仲裁」") && rejected.results[0].error.contains("只读"),
            "仲裁可写应被拒：{}",
            rejected.results[0].error
        );
        // 对照：仲裁 + 只读 通过这扇门（随后死在 missing agent 的构建/执行上，不是门禁）
        let mut ok = node(Vec::new());
        ok.agent = "missing-agent".into();
        ok.model = "m-arb".into();
        ok.role = "仲裁".into();
        ok.visibility = "共享".into();
        ok.write = "只读".into();
        let passed = dispatch_stage_core(
            stage_request(project.to_string_lossy().into_owned(), vec![ok]),
            None,
        )
        .unwrap();
        assert_eq!(passed.results[0].status, "error");
        assert!(
            !passed.results[0].error.contains("写权限必须为只读"),
            "只读仲裁不该被门禁拦：{}",
            passed.results[0].error
        );
        let _ = fs::remove_dir_all(root);
    }

    // 与前端 disputeExcerpt（graph/dispatch.ts）同口径的摘录：折叠空白，
    // 超 200 字截断加省略号。仅供 live 测试构造中性描述。
    fn dispute_excerpt(text: &str, limit: usize) -> String {
        let clean = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if clean.chars().count() <= limit {
            return clean;
        }
        format!("{}…", clean.chars().take(limit).collect::<String>())
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
            stage: 1,
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
            previous_runner: String::new(),
            write: "只读".into(),
            original_request: prompt.into(),
            upstream_artifacts: Vec::new(),
            fallback: None,
            rollback_before: false,
            review_material_validation: false,
            fix_round: false,
        }
    }

    fn live_settings(concurrency: usize) -> DispatchSettings {
        DispatchSettings {
            timeout_secs: 600,
            concurrency,
            redact: true,
            write_project_dispatch: true,
            cc_switch_path: String::new(),
            validation_command: String::new(),
            max_requests: 8,
            swarm_limit: 4,
            acp_enabled: false,
            fix_round_limit: 1,
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

    #[test]
    #[ignore = "three real model requests; run once outside the sandbox for TASK-04 acceptance"]
    fn live_economy_pipeline() {
        const BLIND_PREFIX: &str = include_str!("../fixtures/prompts/blind-prefix.txt");
        const PLAN_TEMPLATE: &str = include_str!("../fixtures/prompts/planning.txt");
        const EXECUTION_TEMPLATE: &str = include_str!("../fixtures/prompts/execution.txt");
        const ACCEPTANCE_TEMPLATE: &str = include_str!("../fixtures/prompts/acceptance.txt");
        let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let project = std::env::temp_dir().join(format!("aicross-pipeline-{}", Uuid::new_v4()));
        fs::create_dir_all(project.join("fixtures/pipeline")).unwrap();
        fs::copy(
            repository.join("fixtures/pipeline/roman.py"),
            project.join("fixtures/pipeline/roman.py"),
        )
        .unwrap();
        std::env::set_var("AICROSS_TEST_RUN_ROOT", project.join(".aicross-runtime"));
        let original = "给 fixtures/pipeline/roman.py 补一个把整数转罗马数字的函数并写 3 个测试";

        let mut plan = live_node(
            "pipeline-plan",
            "pi",
            "zai-coding-cn",
            "glm-5.3-flash",
            "出方案",
            "共享",
            &format!("{original}\n\n{}", PLAN_TEMPLATE.trim()),
            original,
            Vec::new(),
            "",
        );
        plan.thinking = "高".into();
        plan.stage = 1;
        let plan_result = dispatch_stage_core(
            DispatchStageRequest {
                run_id: format!("task04-pipeline-{}", Local::now().format("%Y%m%d-%H%M%S")),
                stage: 1,
                project_dir: project.to_string_lossy().into_owned(),
                nodes: vec![plan],
                settings: live_settings(1),
            },
            None,
        )
        .unwrap();
        let plan_item = &plan_result.results[0];
        assert_eq!(plan_item.status, "ok", "{}", plan_item.error);

        let mut execute = live_node(
            "pipeline-execute", "codex", "", "gpt-5.6-luna", "执行", "共享",
            &format!("{}\n\n原始需求：{original}\n\n方案：{}", EXECUTION_TEMPLATE.trim(), plan_item.text),
            original, Vec::new(), "",
        );
        execute.write = "单写者".into();
        execute.stage = 2;
        let mut execute_settings = live_settings(1);
        execute_settings.validation_command = "python -m pytest".into();
        let execute_result = dispatch_stage_core(
            DispatchStageRequest {
                run_id: plan_result.run_id.clone(),
                stage: 2,
                project_dir: project.to_string_lossy().into_owned(),
                nodes: vec![execute],
                settings: execute_settings,
            },
            None,
        )
        .unwrap();
        let execute_item = &execute_result.results[0];
        assert_eq!(execute_item.status, "ok", "{}", execute_item.error);
        assert!(
            !execute_item.validation_output.trim().is_empty(),
            "执行节点没有保存验证命令输出"
        );
        let artifacts = execute_item
            .artifacts
            .clone()
            .expect("执行节点必须返回产物");
        assert!(!artifacts.diff.trim().is_empty(), "执行产物 diff 为空");

        let mut accept = live_node(
            "pipeline-accept", "kimi", "kimi", "kimi-code/k3-256k", "验收", "盲",
            &format!("{}\n\n[用户]\n{original}\n\n{}", BLIND_PREFIX.trim(), ACCEPTANCE_TEMPLATE.trim()),
            original, Vec::new(), "",
        );
        accept.stage = 3;
        accept.original_request = original.into();
        accept.upstream_artifacts = vec![artifacts];
        let accept_result = dispatch_stage_core(
            DispatchStageRequest {
                run_id: plan_result.run_id,
                stage: 3,
                project_dir: project.to_string_lossy().into_owned(),
                nodes: vec![accept],
                settings: live_settings(1),
            },
            None,
        )
        .unwrap();
        let accept_item = &accept_result.results[0];
        assert_eq!(accept_item.status, "ok", "{}", accept_item.error);
        let mut blind_entries = fs::read_dir(&accept_item.workdir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        blind_entries.sort();
        assert_eq!(blind_entries, ["REQUEST.md", "changed", "diff.patch"]);
        assert!(!Path::new(&accept_item.workdir).join("roman.py").exists());
        assert!(project.join(".dispatch/STATE.md").is_file());
        let dispatch_records = fs::read_dir(project.join(".dispatch"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.path().extension().and_then(|value| value.to_str()) == Some("md")
                    && entry.file_name() != "STATE.md"
            })
            .count();
        assert_eq!(dispatch_records, 3, "每个阶段应各有一份派发记录");
        let request_count = plan_result.results.len()
            + execute_result.results.len()
            + accept_result.results.len();
        assert!(request_count <= 8, "请求总数超过 8：{request_count}");
        println!(
            "PIPELINE_RUN={} REQUESTS=3 STATE={}",
            accept_result.run_id, accept_result.state_file
        );
        let _ = fs::remove_dir_all(project);
    }

    #[test]
    #[ignore = "one real cc-switch request; requires AICROSS_CC_SWITCH_PATH and provider"]
    fn live_cc_switch_route() {
        let Ok(script) = std::env::var("AICROSS_CC_SWITCH_PATH") else {
            panic!("需要 AICROSS_CC_SWITCH_PATH 与 AICROSS_CC_SWITCH_PROVIDER");
        };
        let provider = std::env::var("AICROSS_CC_SWITCH_PROVIDER").unwrap_or_default();
        if provider.is_empty() {
            panic!("需要 AICROSS_CC_SWITCH_PATH 与 AICROSS_CC_SWITCH_PROVIDER");
        }
        let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut request = live_node(
            "cc-switch-live",
            "cc-switch",
            &provider,
            "cc-switch",
            "验证",
            "共享",
            "只回答 OK",
            "只回答 OK",
            Vec::new(),
            "",
        );
        request.tier = "低".into();
        let mut settings = live_settings(1);
        settings.cc_switch_path = script;
        let result = dispatch_stage_core(
            DispatchStageRequest {
                run_id: format!("task04-cc-switch-{}", Local::now().format("%Y%m%d-%H%M%S")),
                stage: 1,
                project_dir: project.to_string_lossy().into_owned(),
                nodes: vec![request],
                settings,
            },
            None,
        )
        .unwrap();
        assert_eq!(
            result.results[0].status, "ok",
            "{}",
            result.results[0].error
        );
        assert!(result.results[0].tokens_in > 0, "cc-switch tokens_in 为 0");
        println!(
            "CC_SWITCH_REQUESTS=1 RECORD={}",
            result.results[0].dispatch_file
        );
    }

    #[test]
    #[ignore = "three real model requests (擂台两家 + 仲裁一家); run outside the sandbox for TASK-05B acceptance"]
    fn live_arena_and_arbitration() {
        const BLIND_PREFIX: &str = include_str!("../fixtures/prompts/blind-prefix.txt");
        let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let enforce_live_success = user_data_root_is_writable();
        if !enforce_live_success {
            println!("ARENA_ASSERTIONS_SKIPPED=用户数据目录不可写，当前为受限环境");
            return;
        }
        std::env::set_var("AICROSS_TEST_RUN_ROOT", project.join(".dispatch/runtime"));
        let prompt = "估算一个三口之家一个月的饮用水需求量（升）。只给数字区间与一条依据，不要展开。";
        // 擂台：同一问题两家共享作答（对应内置预设 builtin-arena 的目标组合，
        // 由编排者在 G1 确认屏选定；这里直接按定案模型落死）。
        let first = live_node("arena-pi-glm", "pi", "zai-coding-cn", "glm-5.3-flash", "执行", "共享", prompt, prompt, Vec::new(), "");
        let second = live_node("arena-codex-luna", "codex", "", "gpt-5.6-luna", "执行", "共享", prompt, prompt, Vec::new(), "");
        let arena = dispatch_stage_core(
            DispatchStageRequest {
                run_id: format!("task05b-arena-{}", Local::now().format("%Y%m%d-%H%M%S")),
                stage: 1,
                project_dir: project.to_string_lossy().into_owned(),
                nodes: vec![first, second],
                settings: live_settings(2),
            },
            None,
        )
        .unwrap();
        println!("ARENA_RUN={} STATE={}", arena.run_id, arena.state_file);
        for item in &arena.results {
            println!("ARENA_RESULT node={} status={} record={}", item.node_id, item.status, item.dispatch_file);
            assert_eq!(item.status, "ok", "{}: {}", item.node_id, item.error);
        }
        // R2-6：厂商与写权限不写死，从真实结果的留痕 frontmatter 读回再断言。
        let record_field = |node_id: &str, key: &str| {
            arena
                .results
                .iter()
                .find(|item| item.node_id == node_id)
                .and_then(|item| {
                    item.frontmatter
                        .lines()
                        .find_map(|line| line.strip_prefix(&format!("{key}: ")))
                })
                .unwrap_or_else(|| panic!("{node_id} 留痕缺 {key}"))
                .to_string()
        };
        let first_vendor = record_field("arena-pi-glm", "vendor");
        let second_vendor = record_field("arena-codex-luna", "vendor");
        // 仲裁：与前端 appendArbitration 的产物同形——盲、只读、role=仲裁，
        // 材料只带双方共同材料祖先（根，这里即擂台题目本身），中性描述按
        // node_id 映射取两边正文，标签「甲方/乙方」，不写双方 agent/model。
        let body_of = |node_id: &str| {
            arena
                .results
                .iter()
                .find(|item| item.node_id == node_id)
                .map(|item| item.text.clone())
                .unwrap_or_else(|| panic!("擂台缺 {node_id} 的结果"))
        };
        // R2-6：与产品 disputeExcerpt 同口径（200 字 + 省略号），不再用 take(400)。
        let dispute = format!(
            "【甲方】认为：{}；【乙方】认为：{}。请独立裁决，给出你的结论与理由。",
            dispute_excerpt(&body_of("arena-pi-glm"), 200),
            dispute_excerpt(&body_of("arena-codex-luna"), 200)
        );
        assert!(dispute.contains("【甲方】") && dispute.contains("【乙方】"));
        for leaked in ["pi/", "codex/", "glm-5.3-flash", "gpt-5.6-luna"] {
            assert!(!dispute.contains(leaked), "中性描述不得写双方 agent/model：{leaked}");
        }
        let material = std::env::temp_dir().join(format!("aicross-arb-root-{}.md", Uuid::new_v4()));
        fs::write(&material, prompt).unwrap();
        let arb_prompt = format!(
            "{}\n\n[用户] {prompt}\n\n[附件] {}\n\n{dispute}",
            BLIND_PREFIX.trim(),
            material.file_name().unwrap().to_string_lossy()
        );
        let arb = live_node(
            "arb-kimi", "kimi", "kimi", "kimi-code/k3-256k", "仲裁", "盲",
            &arb_prompt, &arb_prompt, vec![material.to_string_lossy().into_owned()], "",
        );
        let arbitration = dispatch_stage_core(
            DispatchStageRequest {
                run_id: format!("task05b-arb-{}", Local::now().format("%Y%m%d-%H%M%S")),
                stage: 2,
                project_dir: project.to_string_lossy().into_owned(),
                nodes: vec![arb],
                settings: live_settings(1),
            },
            None,
        )
        .unwrap();
        let arb = &arbitration.results[0];
        println!("ARBITRATION status={} record={} isolation={}", arb.status, arb.dispatch_file, arb.isolation);
        assert_eq!(arb.status, "ok", "{}", arb.error);
        // R2-6：仲裁留痕读回的 vendor 与两边不同、write 是引擎强制后的「只读」。
        let arb_field = |key: &str| {
            arb.frontmatter
                .lines()
                .find_map(|line| line.strip_prefix(&format!("{key}: ")))
                .unwrap_or_else(|| panic!("仲裁留痕缺 {key}：\n{}", arb.frontmatter))
                .to_string()
        };
        assert_eq!(arb_field("role"), "仲裁");
        assert_eq!(arb_field("write"), "只读", "盲仲裁的生效写权限必须是只读");
        assert_ne!(arb_field("vendor"), first_vendor, "仲裁厂商不得与甲方相同");
        assert_ne!(arb_field("vendor"), second_vendor, "仲裁厂商不得与乙方相同");
        let arb_workdir = Path::new(&arb.workdir);
        assert_ne!(arb_workdir, project.as_path(), "盲仲裁的 workdir 不得是项目目录");
        assert!(arb_workdir.join(material.file_name().unwrap()).is_file(),
            "隔离目录只应收到共同材料祖先");
        assert!(!arb_workdir.join("Cargo.toml").exists(), "隔离目录不得含项目文件");
        let _ = fs::remove_file(material);
    }

    // ── TASK-12 审查材料附带验证输出 / 修正轮上限 / 提示词固定关注点 ──────────

    fn task12_temp(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("aicross-task12-{name}-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn task12_blind_node(flag: bool) -> DispatchNodeRequest {
        let mut request = node(Vec::new());
        request.review_material_validation = flag;
        request
    }

    fn task12_prepared(
        request: &DispatchNodeRequest,
        workdir: &Path,
        blind: bool,
    ) -> Result<OutboundPreparation, String> {
        prepare_outbound(request, true, workdir, blind)
            .map(|(prompt, names, count)| OutboundPreparation::Materialized(prompt, names, count))
    }

    #[test]
    fn task12_settings_fix_round_limit_defaults_to_one_and_reads_camel_case() {
        let settings: DispatchSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings.fix_round_limit, 1);
        assert_eq!(DispatchSettings::default().fix_round_limit, 1);
        let settings: DispatchSettings = serde_json::from_str(r#"{"fixRoundLimit":3}"#).unwrap();
        assert_eq!(settings.fix_round_limit, 3);
        let json = serde_json::to_value(DispatchSettings::default()).unwrap();
        assert_eq!(json["fixRoundLimit"], 1);
    }

    #[test]
    fn task12_request_reads_review_material_validation_camel_case_default_false() {
        let mut value = serde_json::to_value(node(Vec::new())).unwrap();
        assert_eq!(value["reviewMaterialValidation"], false);
        value.as_object_mut().unwrap().remove("reviewMaterialValidation");
        let restored: DispatchNodeRequest = serde_json::from_value(value.clone()).unwrap();
        assert!(!restored.review_material_validation);
        value.as_object_mut().unwrap().insert("reviewMaterialValidation".into(), true.into());
        let restored: DispatchNodeRequest = serde_json::from_value(value).unwrap();
        assert!(restored.review_material_validation);
    }

    #[test]
    fn task12_validation_txt_materialized_for_exit_zero_command() {
        let root = task12_temp("exit0");
        let project = root.join("project");
        let workdir = root.join("blind");
        fs::create_dir_all(&project).unwrap();
        let settings = DispatchSettings { validation_command: "echo review-ok".into(), ..DispatchSettings::default() };
        let request = task12_blind_node(true);
        let prepared = task12_prepared(&request, &workdir, true);
        let (prepared, attached) = attach_review_validation("exit0", &project, &settings, &request, &workdir, true, Duration::from_secs(60), prepared);
        let OutboundPreparation::Materialized(prompt, names, _) = prepared.unwrap() else { panic!("应已物化") };
        assert!(attached);
        let text = fs::read_to_string(workdir.join("VALIDATION.txt")).unwrap();
        assert!(text.contains("验证命令：echo review-ok"), "{text}");
        assert!(text.contains("exit 0"), "{text}");
        assert!(text.contains("review-ok"), "{text}");
        assert!(prompt.contains("验证命令输出见 VALIDATION.txt\n\n验证命令输出：\n"), "{prompt}");
        assert!(prompt.contains("review-ok"), "{prompt}");
        assert_eq!(names.last().map(String::as_str), Some("VALIDATION.txt"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task12_validation_txt_materialized_for_exit_nine_command_and_still_attached() {
        let root = task12_temp("exit9");
        let project = root.join("project");
        let workdir = root.join("blind");
        fs::create_dir_all(&project).unwrap();
        let settings = DispatchSettings { validation_command: "echo boom; exit 9".into(), ..DispatchSettings::default() };
        let request = task12_blind_node(true);
        let prepared = task12_prepared(&request, &workdir, true);
        let (prepared, attached) = attach_review_validation("exit9", &project, &settings, &request, &workdir, true, Duration::from_secs(60), prepared);
        assert!(prepared.is_ok());
        assert!(attached);
        let text = fs::read_to_string(workdir.join("VALIDATION.txt")).unwrap();
        assert!(text.contains("exit=9"), "{text}");
        assert!(text.contains("boom"), "{text}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task12_validation_txt_says_unset_when_command_empty() {
        let root = task12_temp("unset");
        let project = root.join("project");
        let workdir = root.join("blind");
        fs::create_dir_all(&project).unwrap();
        let request = task12_blind_node(true);
        let prepared = task12_prepared(&request, &workdir, true);
        let (_, attached) = attach_review_validation("unset", &project, &DispatchSettings::default(), &request, &workdir, true, Duration::from_secs(60), prepared);
        assert!(attached);
        assert_eq!(fs::read_to_string(workdir.join("VALIDATION.txt")).unwrap(), "未配置验证命令");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task12_validation_txt_not_written_when_flag_off_or_not_blind() {
        let root = task12_temp("off");
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        let settings = DispatchSettings { validation_command: "echo x".into(), ..DispatchSettings::default() };
        let off = root.join("off");
        let prepared = task12_prepared(&task12_blind_node(false), &off, true);
        let (prepared, attached) = attach_review_validation("off", &project, &settings, &task12_blind_node(false), &off, true, Duration::from_secs(60), prepared);
        assert!(!attached);
        let OutboundPreparation::Materialized(prompt, _, _) = prepared.unwrap() else { panic!("应已物化") };
        assert!(!prompt.contains("VALIDATION.txt"));
        assert!(!off.join("VALIDATION.txt").exists());
        let shared = root.join("shared");
        fs::create_dir_all(&shared).unwrap();
        let mut request = task12_blind_node(true);
        request.visibility = "共享".into();
        let prepared = task12_prepared(&request, &shared, false);
        let (_, attached) = attach_review_validation("shared", &project, &settings, &request, &shared, false, Duration::from_secs(60), prepared);
        assert!(!attached);
        assert!(!shared.join("VALIDATION.txt").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task12_unsent_does_not_run_validation_or_write_blind_material() {
        let root = task12_temp("unsent");
        let project = root.join("project");
        let workdir = root.join("blind");
        let marker = root.join("validation-ran.txt");
        fs::create_dir_all(&project).unwrap();
        #[cfg(windows)]
        let command = format!("Set-Content -LiteralPath '{}' -Value ran", marker.display());
        #[cfg(not(windows))]
        let command = format!("printf ran > '{}'", marker.display());
        let settings = DispatchSettings { validation_command: command, ..DispatchSettings::default() };
        let request = task12_blind_node(true);
        let (prepared, attached) = attach_review_validation(
            "unsent",
            &project,
            &settings,
            &request,
            &workdir,
            true,
            Duration::from_secs(60),
            Ok(OutboundPreparation::Unsent),
        );
        assert!(matches!(prepared, Ok(OutboundPreparation::Unsent)));
        assert!(!attached);
        assert!(!marker.exists());
        assert!(!workdir.join(REVIEW_VALIDATION_FILE).exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task12_same_run_and_project_executes_review_validation_once() {
        let root = task12_temp("cache");
        let project = root.join("project");
        let marker = root.join("validation-count.txt");
        fs::create_dir_all(&project).unwrap();
        #[cfg(windows)]
        let command = format!("Add-Content -LiteralPath '{}' -Value x; Write-Output shared-output", marker.display());
        #[cfg(not(windows))]
        let command = format!("printf x >> '{}'; echo shared-output", marker.display());
        let settings = DispatchSettings { validation_command: command, ..DispatchSettings::default() };
        clear_review_validation_cache("same-run", &project);
        for name in ["blind-a", "blind-b"] {
            let workdir = root.join(name);
            let request = task12_blind_node(true);
            let prepared = task12_prepared(&request, &workdir, true);
            let (prepared, attached) = attach_review_validation(
                "same-run",
                &project,
                &settings,
                &request,
                &workdir,
                true,
                Duration::from_secs(60),
                prepared,
            );
            assert!(attached);
            let OutboundPreparation::Materialized(prompt, _, _) = prepared.unwrap() else { panic!("应已物化") };
            assert!(prompt.contains("shared-output"), "{prompt}");
        }
        assert_eq!(fs::read_to_string(&marker).unwrap().lines().count(), 1);
        clear_review_validation_cache("same-run", &project);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task12_run_one_materializes_validation_and_uses_half_node_timeout() {
        if channels::resolve_binary("python", None).is_err() {
            eprintln!("跳过：PATH 中没有 python");
            return;
        }
        let root = task12_temp("run-one");
        let project = root.join("project");
        let run_root = root.join("runs");
        let log_dir = root.join("logs");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&log_dir).unwrap();
        let stub = root.join("cc-switch-stub.py");
        fs::write(&stub, "print('review complete')\nprint('input=1 cache_read=0 output=1')\n").unwrap();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let captured = observed.clone();
        *review_validation_hook().lock().unwrap() = Some(Arc::new(move |command, cwd, timeout| {
            if command != "task12-injected-validation" {
                return None;
            }
            captured.lock().unwrap().push((cwd.to_path_buf(), timeout));
            Some(Ok("injected-validation-output".into()))
        }));
        let mut request = task12_blind_node(true);
        request.agent = "cc-switch".into();
        request.provider = "stub".into();
        request.model = "stub".into();
        let settings = DispatchSettings {
            cc_switch_path: stub.to_string_lossy().into_owned(),
            validation_command: "task12-injected-validation".into(),
            timeout_secs: 10,
            ..DispatchSettings::default()
        };
        let result = run_one("run-one", &project, &run_root, &log_dir, &settings, request);
        *review_validation_hook().lock().unwrap() = None;
        clear_review_validation_cache("run-one", &project);
        assert_eq!(result.status, "ok", "{}", result.error);
        assert!(run_root.join("node-1").join(REVIEW_VALIDATION_FILE).exists());
        assert!(result.frontmatter.contains("validation_attached: true"), "{}", result.frontmatter);
        assert_eq!(*observed.lock().unwrap(), vec![(project.clone(), Duration::from_secs(5))]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task12_validation_text_truncates_at_8kb_on_char_boundary() {
        let text = review_validation_text("cargo test", Ok("测".repeat(20_000)));
        assert!(text.len() <= REVIEW_VALIDATION_LIMIT + "\n…（输出超过 8 KB 已截断）".len(), "{}", text.len());
        assert!(text.ends_with("…（输出超过 8 KB 已截断）"));
        assert!(text.starts_with("验证命令：cargo test\n\n验证命令通过（exit 0）\n测"));
        let short = review_validation_text("cargo test", Ok("fine".into()));
        assert!(!short.contains("已截断"));
    }

    #[test]
    fn task12_validation_text_is_redacted() {
        let text = review_validation_text("npm test", Err("验证命令失败（exit=1）\ntoken=sk-live-abc123\nAuthorization: Bearer xyz".into()));
        assert!(!text.contains("abc123"), "{text}");
        assert!(!text.contains("Bearer xyz"), "{text}");
        assert!(text.contains("[REDACTED]"));
        assert!(text.contains("exit=1"));
    }

    #[test]
    fn task12_review_material_dir_prefers_existing_upstream_worktree() {
        let root = task12_temp("dir");
        let project = root.join("project");
        let worktree = root.join("wt");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&worktree).unwrap();
        let mut request = task12_blind_node(true);
        assert_eq!(review_material_dir(&project, &request), project);
        request.upstream_artifacts.push(DispatchArtifacts { worktree: Some(worktree.to_string_lossy().into_owned()), ..DispatchArtifacts::default() });
        assert_eq!(review_material_dir(&project, &request), worktree);
        request.upstream_artifacts[0].worktree = Some(root.join("gone").to_string_lossy().into_owned());
        assert_eq!(review_material_dir(&project, &request), project);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task12_record_frontmatter_carries_validation_attached() {
        let record = DispatchRecord { validation_attached: true, ..DispatchRecord::default() };
        assert!(serialize_frontmatter(&record).unwrap().contains("validation_attached: true"));
        let record = DispatchRecord::default();
        assert!(serialize_frontmatter(&record).unwrap().contains("validation_attached: false"));
    }

    #[test]
    fn task12_review_prompt_files_end_with_fixed_focus_points() {
        const CONTRACT: &str = "契约（字段、命令签名、序列化大小写）有没有变";
        const RED_TEST: &str = "新增的测试能不能红：把修复回退后该测试是否会失败";
        for text in [
            include_str!("../fixtures/prompts/acceptance.txt"),
            include_str!("../fixtures/prompts/blind-prefix.txt"),
        ] {
            let lines: Vec<&str> = text.trim_end().lines().collect();
            let tail = &lines[lines.len() - 2..];
            assert!(tail[0].contains(CONTRACT), "{text}");
            assert!(tail[1].contains(RED_TEST), "{text}");
        }
    }

    fn task13_repo(tag: &str) -> (PathBuf, PathBuf, DirectorySnapshot) {
        let root = std::env::temp_dir().join(format!("aicross-red-green-{tag}-{}", Uuid::new_v4()));
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        test_git(&project, &["init"]);
        test_git(&project, &["config", "user.name", "aicross test"]);
        test_git(&project, &["config", "user.email", "aicross@example.invalid"]);
        fs::write(project.join("function.txt"), b"1\n").unwrap();
        fs::write(project.join("baseline.test"), b"baseline\n").unwrap();
        test_git(&project, &["add", "function.txt", "baseline.test"]);
        test_git(&project, &["commit", "-m", "base"]);
        let before = snapshot_directory(&project).unwrap();
        (root, project, before)
    }

    fn task13_persist_before(project: &Path, before: &DirectorySnapshot, after: &DirectorySnapshot, node: &str) -> String {
        let log = project.join(".dispatch");
        persist_before_snapshot(project, &log, "run-13", node, before, after).unwrap()
    }

    #[cfg(windows)]
    fn task13_strict_command() -> String {
        "if ((Get-Content function.txt -Raw).Trim() -ne '2' -or -not (Test-Path regression.test)) { exit 1 }".into()
    }

    #[cfg(not(windows))]
    fn task13_strict_command() -> String {
        "test \"$(cat function.txt)\" = 2 && test -f regression.test".into()
    }

    #[cfg(windows)]
    fn task13_baseline_command() -> String {
        "if (-not (Test-Path baseline.test)) { exit 1 }".into()
    }

    #[cfg(not(windows))]
    fn task13_baseline_command() -> String {
        "test -f baseline.test".into()
    }

    #[cfg(windows)]
    fn task13_timeout_command() -> String {
        "Start-Sleep -Seconds 2".into()
    }

    #[cfg(not(windows))]
    fn task13_timeout_command() -> String {
        "sleep 2".into()
    }

    fn task13_failure_command() -> String {
        "exit 1".into()
    }

    fn task13_file_bytes(root: &Path) -> HashMap<String, Vec<u8>> {
        snapshot_directory(root).unwrap().into_iter()
            .filter_map(|(path, file)| file.bytes.map(|bytes| (path, bytes)))
            .collect()
    }

    #[test]
    fn task13_changed_function_with_regression_test_is_pinned() {
        if !require_git("task13_changed_function_with_regression_test_is_pinned") { return; }
        let (root, project, before) = task13_repo("pinned");
        fs::write(project.join("function.txt"), b"2\n").unwrap();
        fs::write(project.join("regression.test"), b"expects 2\n").unwrap();
        let after = snapshot_directory(&project).unwrap();
        let before_dir = task13_persist_before(&project, &before, &after, "pinned");
        let outcome = red_green_check(&project, &project, &before_dir, &before, &after, &task13_strict_command(), Duration::from_secs(5));
        assert_eq!(outcome.pinned, "pinned", "{outcome:?}");
        assert!(!outcome.validation_failed, "{outcome:?}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task13_changed_function_without_regression_test_is_unpinned() {
        if !require_git("task13_changed_function_without_regression_test_is_unpinned") { return; }
        let (root, project, before) = task13_repo("unpinned");
        fs::write(project.join("function.txt"), b"2\n").unwrap();
        let after = snapshot_directory(&project).unwrap();
        let before_dir = task13_persist_before(&project, &before, &after, "unpinned");
        let outcome = red_green_check(&project, &project, &before_dir, &before, &after, &task13_baseline_command(), Duration::from_secs(5));
        assert_eq!(outcome.pinned, "unpinned", "{outcome:?}");
        assert_eq!(outcome.note, "回退修复后测试仍通过：新增测试没有钉住这个修复");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task13_missing_before_snapshot_is_skipped() {
        if !require_git("task13_missing_before_snapshot_is_skipped") { return; }
        let (root, project, before) = task13_repo("missing");
        fs::write(project.join("function.txt"), b"2\n").unwrap();
        let after = snapshot_directory(&project).unwrap();
        let outcome = red_green_check(&project, &project, "missing.before", &before, &after, &task13_baseline_command(), Duration::from_secs(5));
        assert_eq!(outcome.pinned, "skipped", "{outcome:?}");
        assert!(!outcome.validation_failed);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task13_red_green_restores_exact_bytes_and_leaves_unrelated_file_alone() {
        if !require_git("task13_red_green_restores_exact_bytes_and_leaves_unrelated_file_alone") { return; }
        let (root, project, _) = task13_repo("restore");
        fs::write(project.join("unrelated.bin"), [0_u8, 1, 2, 255]).unwrap();
        let before = snapshot_directory(&project).unwrap();
        fs::write(project.join("function.txt"), b"2\r\n").unwrap();
        fs::write(project.join("regression.test"), b"expects 2\n").unwrap();
        let after = snapshot_directory(&project).unwrap();
        let before_dir = task13_persist_before(&project, &before, &after, "restore");
        let expected = task13_file_bytes(&project);
        let outcome = red_green_check(&project, &project, &before_dir, &before, &after, &task13_strict_command(), Duration::from_secs(5));
        assert_eq!(outcome.pinned, "pinned", "{outcome:?}");
        assert_eq!(task13_file_bytes(&project), expected);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task13_red_green_timeout_is_skipped_and_restored() {
        if !require_git("task13_red_green_timeout_is_skipped_and_restored") { return; }
        let (root, project, before) = task13_repo("timeout");
        fs::write(project.join("function.txt"), b"2\n").unwrap();
        let after = snapshot_directory(&project).unwrap();
        let before_dir = task13_persist_before(&project, &before, &after, "timeout");
        let expected = task13_file_bytes(&project);
        let outcome = red_green_check(&project, &project, &before_dir, &before, &after, &task13_timeout_command(), Duration::from_millis(50));
        assert_eq!(outcome.pinned, "skipped", "{outcome:?}");
        assert!(!outcome.validation_failed);
        assert_eq!(task13_file_bytes(&project), expected);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task13_restored_validation_failure_changes_status_outcome() {
        if !require_git("task13_restored_validation_failure_changes_status_outcome") { return; }
        let (root, project, before) = task13_repo("green-failed");
        fs::write(project.join("function.txt"), b"2\n").unwrap();
        let after = snapshot_directory(&project).unwrap();
        let before_dir = task13_persist_before(&project, &before, &after, "green-failed");
        let expected = task13_file_bytes(&project);
        let outcome = red_green_check(&project, &project, &before_dir, &before, &after, &task13_failure_command(), Duration::from_secs(5));
        assert_eq!(outcome.pinned, "skipped", "{outcome:?}");
        assert!(outcome.validation_failed, "{outcome:?}");
        assert_eq!(outcome.note, "红绿核对恢复后验证未通过");
        assert_eq!(task13_file_bytes(&project), expected);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task13_wire_defaults_and_frontmatter_are_append_only() {
        let mut value = serde_json::to_value(node(Vec::new())).unwrap();
        assert_eq!(value["fixRound"], false);
        value.as_object_mut().unwrap().remove("fixRound");
        assert!(!serde_json::from_value::<DispatchNodeRequest>(value).unwrap().fix_round);
        let result = DispatchNodeResult { pinned: Some("pinned".into()), pinned_note: Some("ok".into()), ..DispatchNodeResult::default() };
        let result_json = serde_json::to_value(result).unwrap();
        assert_eq!(result_json["pinned"], "pinned");
        assert_eq!(result_json["pinnedNote"], "ok");
        let record = DispatchRecord { pinned: "unpinned".into(), ..DispatchRecord::default() };
        assert!(serialize_frontmatter(&record).unwrap().contains("pinned: unpinned"));
    }

    fn task13_engine_run(tag: &str, write: &str, add_test: bool, validation_command: String) -> (PathBuf, PathBuf, DispatchNodeResult) {
        let (root, project, _) = task13_repo(tag);
        let stub = root.join("cc_switch_stub.py");
        let regression = if add_test {
            "pathlib.Path('regression.test').write_text('expects 2\\n', encoding='utf-8')\n"
        } else {
            ""
        };
        fs::write(
            &stub,
            format!("import pathlib\npathlib.Path('function.txt').write_text('2\\n', encoding='utf-8')\n{regression}print('stub answer')\nprint('input=2 cache_read=0 output=1')\n"),
        ).unwrap();
        let mut request = node(Vec::new());
        request.agent = "cc-switch".into();
        request.provider = "stub".into();
        request.role = "执行".into();
        request.visibility = "共享".into();
        request.write = write.into();
        request.fix_round = true;
        let settings = DispatchSettings {
            cc_switch_path: stub.to_string_lossy().into_owned(),
            validation_command,
            timeout_secs: 10,
            write_project_dispatch: true,
            ..DispatchSettings::default()
        };
        let run_root = root.join("runs");
        let log_dir = project.join(".dispatch");
        let result = run_one("run-13-engine", &project, &run_root, &log_dir, &settings, request);
        (root, project, result)
    }

    #[test]
    fn task13_engine_single_writer_fix_without_test_returns_unpinned_record() {
        if !require_git("task13_engine_single_writer_fix_without_test_returns_unpinned_record") { return; }
        let (root, project, result) = task13_engine_run("engine-unpinned", "单写者", false, task13_baseline_command());
        assert_eq!((result.status.as_str(), result.pinned.as_deref()), ("ok", Some("unpinned")), "{}", result.error);
        assert_eq!(fs::read_to_string(project.join("function.txt")).unwrap().trim(), "2");
        assert!(result.frontmatter.contains("pinned: unpinned"), "{}", result.frontmatter);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task13_engine_worktree_fix_with_test_runs_and_restores_inside_worktree() {
        if !require_git("task13_engine_worktree_fix_with_test_runs_and_restores_inside_worktree") { return; }
        let (root, project, result) = task13_engine_run("engine-worktree", "worktree", true, task13_strict_command());
        assert_eq!((result.status.as_str(), result.pinned.as_deref()), ("ok", Some("pinned")), "{}", result.error);
        let worktree = PathBuf::from(result.artifacts.as_ref().and_then(|value| value.worktree.as_deref()).expect("应返回 worktree"));
        assert_eq!(fs::read_to_string(worktree.join("function.txt")).unwrap().trim(), "2");
        assert!(worktree.join("regression.test").is_file());
        assert_eq!(fs::read(project.join("function.txt")).unwrap(), b"1\n");
        assert!(result.frontmatter.contains("pinned: pinned"), "{}", result.frontmatter);
        let _ = run_process_registered(
            Path::new("git"),
            &["worktree".into(), "remove".into(), "--force".into(), worktree.to_string_lossy().into_owned()],
            None,
            Duration::from_secs(10),
            Some(&project),
            None,
        );
        let _ = fs::remove_dir_all(root);
    }
}
