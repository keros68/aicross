//! TASK-69 模型的使用记录与用途标签。
//!
//! - 任务类型：固定小表，派发时 `--kind` 给，记进 run 文件。
//! - 结果记录：每次派发跑完，按最终的图给每条执行链追加一行到
//!   `track/outcomes.jsonl`；`adopt` 追加采纳行。这份文件不随 `runs/` 的
//!   30 天清理删除。
//! - 标签与备注：`model-notes.json`，按 `agent:provider:model` 存，重新探测
//!   不影响它。
//! - 汇总：`aicross stats` 现读上面两份文件算。

use crate::orchestration::{Graph, GraphNode};
use crate::resources::ResourceStore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 任务类型表：(id, 中文名)。改表要同步 skill 的 channels.md。
pub(crate) const TASK_KINDS: &[(&str, &str)] = &[
    ("frontend", "前端"),
    ("backend", "后端"),
    ("writing", "文案"),
    ("data", "数据分析"),
    ("docs", "文档"),
    ("other", "其他"),
];

/// 有验收结论的次数不少于这个数才给通过率。
pub(crate) const MIN_JUDGED: u32 = 5;

pub(crate) const RESULT_FIRST_PASS: &str = "first_pass";
pub(crate) const RESULT_FIXED_PASS: &str = "fixed_pass";
pub(crate) const RESULT_NOT_PASSED: &str = "not_passed";
pub(crate) const RESULT_FAILED: &str = "failed";
pub(crate) const RESULT_NO_ACCEPTANCE: &str = "no_acceptance";
pub(crate) const RESULT_UNJUDGED: &str = "unjudged";

const NOTES_LOCK_TIMEOUT: Duration = Duration::from_secs(10);

/// 中文名或英文 id（不分大小写）解析成 id。
pub(crate) fn resolve_kind(raw: &str) -> Result<&'static str, String> {
    let wanted = raw.trim();
    TASK_KINDS
        .iter()
        .find(|(id, name)| id.eq_ignore_ascii_case(wanted) || *name == wanted)
        .map(|(id, _)| *id)
        .ok_or_else(|| {
            let list: Vec<String> = TASK_KINDS.iter().map(|(id, name)| format!("{name}（{id}）")).collect();
            format!("任务类型只能是：{}", list.join("、"))
        })
}

pub(crate) fn kind_name(id: &str) -> &str {
    TASK_KINDS.iter().find(|(kind, _)| *kind == id).map(|(_, name)| *name).unwrap_or("未分类")
}

/// `agent:provider:model`，provider 空时中间留空（与 `--seat` 同一写法）。
pub(crate) fn model_key(agent: &str, provider: &str, model: &str) -> String {
    format!("{}:{}:{}", agent.trim(), provider.trim(), model.trim())
}

/// 解析 `agent:provider:model`。
pub(crate) fn parse_model_key(raw: &str) -> Result<String, String> {
    let mut parts = raw.splitn(3, ':');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(agent), Some(provider), Some(model)) if !agent.trim().is_empty() && !model.trim().is_empty() => {
            Ok(model_key(agent, provider, model))
        }
        _ => Err(format!(
            "模型的写法是 <agent>:<provider>:<model>，provider 没有就空着，如 claude::opus：{raw}"
        )),
    }
}

fn track_dir(root: &Path) -> PathBuf {
    root.join("track")
}

fn outcomes_path(root: &Path) -> PathBuf {
    track_dir(root).join("outcomes.jsonl")
}

fn notes_path(root: &Path) -> PathBuf {
    root.join("model-notes.json")
}

// ── 结果判定 ────────────────────────────────────────────────────────────

/// 一条执行链的结果。
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ChainOutcome {
    pub node_id: String,
    pub agent: String,
    pub provider: String,
    pub model: String,
    pub result: &'static str,
    pub rounds: usize,
    pub error_kind: String,
}

fn role(node: &GraphNode) -> &str {
    node.target.as_ref().map(|t| t.role.as_str()).unwrap_or("")
}

fn status(node: &GraphNode) -> &str {
    node.reply.as_ref().map(|r| r.status.as_str()).unwrap_or("")
}

/// 验证重试的任务文本：现行写法以「[修正 N/M]」开头；早期版本没有这个前缀，
/// 只有「请修正后重新运行」。
fn is_retry_text(node: &GraphNode) -> bool {
    let text = &node.user.text;
    text.starts_with("[修正") || text.contains("请修正后重新运行")
}

fn same_target(a: &GraphNode, b: &GraphNode) -> bool {
    match (&a.target, &b.target) {
        (Some(x), Some(y)) => x.agent == y.agent && x.model == y.model && x.provider == y.provider,
        _ => false,
    }
}

/// 修正轮：父节点里有验收（验收要求修正后长出的），或任务文本以「[修正」
/// 开头（验证命令未通过后的重试）。
fn is_correction(graph: &Graph, node: &GraphNode) -> bool {
    is_retry_text(node)
        || node
            .parents
            .iter()
            .filter_map(|id| graph.nodes.get(id))
            .any(|parent| role(parent) == "验收")
}

fn acceptances_of<'a>(graph: &'a Graph, exec_id: &str) -> Vec<&'a GraphNode> {
    let mut list: Vec<&GraphNode> = graph
        .nodes
        .values()
        .filter(|n| role(n) == "验收" && n.parents.iter().any(|p| p == exec_id))
        .collect();
    list.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    list
}

/// 链上的下一轮执行：验收要求修正后长出的执行，或同父、同人选、文本以
/// 「[修正」开头的验证重试；取最早建的、没被别的链占用的那个。
/// 上限：蜂群里同一人选的多条执行同父时，验证重试按建立先后分给各链，
/// 可能分错；要准确需要重试节点带上蜂群条目号。
fn next_round<'a>(graph: &'a Graph, current: &GraphNode, used: &HashSet<String>) -> Option<&'a GraphNode> {
    let accept_ids: HashSet<&str> = acceptances_of(graph, &current.id).iter().map(|n| n.id.as_str()).collect();
    let mut candidates: Vec<&GraphNode> = graph
        .nodes
        .values()
        .filter(|n| role(n) == "执行" && n.id != current.id && !used.contains(&n.id))
        .filter(|n| {
            let from_acceptance = n.parents.iter().any(|p| accept_ids.contains(p.as_str()));
            let validation_retry = is_retry_text(n)
                && n.parents == current.parents
                && same_target(n, current)
                && n.created_at > current.created_at;
            from_acceptance || validation_retry
        })
        .collect();
    candidates.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    candidates.into_iter().next()
}

/// 按最终的图算每条执行链的结果。叫停的、从没跑过的、切了备选的链不记。
pub(crate) fn chain_outcomes(graph: &Graph) -> Vec<ChainOutcome> {
    let mut firsts: Vec<&GraphNode> = graph
        .nodes
        .values()
        .filter(|n| role(n) == "执行" && !is_correction(graph, n))
        .collect();
    firsts.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    let mut used: HashSet<String> = firsts.iter().map(|n| n.id.clone()).collect();
    let mut outcomes = Vec::new();
    for first in firsts {
        let mut chain = vec![first];
        while let Some(next) = next_round(graph, chain[chain.len() - 1], &used) {
            used.insert(next.id.clone());
            chain.push(next);
        }
        let switched = chain
            .iter()
            .any(|n| n.reply.as_ref().is_some_and(|r| r.fallback_from.as_deref().is_some_and(|v| !v.is_empty())));
        if switched {
            continue;
        }
        let last = chain[chain.len() - 1];
        let Some(result) = judge(graph, last, chain.len()) else {
            continue;
        };
        let target = first.target.as_ref().expect("执行节点有人选");
        outcomes.push(ChainOutcome {
            node_id: first.id.clone(),
            agent: target.agent.clone(),
            provider: target.provider.clone().unwrap_or_default(),
            model: target.model.clone(),
            result,
            rounds: chain.len(),
            error_kind: last
                .reply
                .as_ref()
                .and_then(|r| r.error_kind.clone())
                .unwrap_or_default(),
        });
    }
    outcomes
}

fn judge(graph: &Graph, last: &GraphNode, rounds: usize) -> Option<&'static str> {
    let error_kind = last.reply.as_ref().and_then(|r| r.error_kind.as_deref()).unwrap_or("");
    if error_kind == "cancelled" {
        return None;
    }
    match status(last) {
        "done" => {
            let acceptances = acceptances_of(graph, &last.id);
            let Some(accept) = acceptances.last() else {
                return Some(RESULT_NO_ACCEPTANCE);
            };
            let accept_error = accept.reply.as_ref().and_then(|r| r.error_kind.as_deref()).unwrap_or("");
            if accept_error == "cancelled" {
                return None;
            }
            if status(accept) != "done" {
                return Some(RESULT_UNJUDGED);
            }
            let text = accept.reply.as_ref().map(|r| r.text.as_str()).unwrap_or("");
            Some(match crate::engine::parse_acceptance(text) {
                "PASS" if rounds == 1 => RESULT_FIRST_PASS,
                "PASS" => RESULT_FIXED_PASS,
                "NEEDS_FIX" => RESULT_NOT_PASSED,
                _ => RESULT_UNJUDGED,
            })
        }
        "failed" if error_kind == "validation_failed" => Some(RESULT_NOT_PASSED),
        "failed" | "quota_exhausted" => Some(RESULT_FAILED),
        _ => None,
    }
}

// ── 记录文件 ────────────────────────────────────────────────────────────

/// 记录里的一行：结果行或采纳行。
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct TrackLine {
    #[serde(rename = "type")]
    pub kind_of_line: String,
    pub at: String,
    pub run_id: String,
    pub node_id: String,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub preset_id: String,
    #[serde(default)]
    pub task_kind: String,
    pub agent: String,
    #[serde(default)]
    pub provider: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub role: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub result: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rounds: usize,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub error_kind: String,
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

/// 追加若干行。一行一次 write，追加模式下几个后台进程同时收尾也不会把行
/// 写穿插；写失败只返回说明，不影响派发本身。
fn append_lines(root: &Path, lines: &[TrackLine]) -> Result<(), String> {
    if lines.is_empty() {
        return Ok(());
    }
    let dir = track_dir(root);
    fs::create_dir_all(&dir).map_err(|e| format!("创建记录目录失败：{e}"))?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(outcomes_path(root))
        .map_err(|e| format!("打开使用记录失败：{e}"))?;
    for line in lines {
        let mut text = serde_json::to_string(line).map_err(|e| format!("生成使用记录失败：{e}"))?;
        text.push('\n');
        file.write_all(text.as_bytes()).map_err(|e| format!("写使用记录失败：{e}"))?;
    }
    Ok(())
}

/// 派发跑完时调：按最终的图写结果行。
pub(crate) fn record_run(root: &Path, run: &crate::engine::EngineRun, graph: &Graph, at: &str) -> Result<(), String> {
    let lines: Vec<TrackLine> = chain_outcomes(graph)
        .into_iter()
        .map(|o| TrackLine {
            kind_of_line: "outcome".into(),
            at: at.into(),
            run_id: run.run_id.clone(),
            node_id: o.node_id,
            host: run.host.clone(),
            preset_id: run.preset_id.clone(),
            task_kind: run.task_kind.clone(),
            agent: o.agent,
            provider: o.provider,
            model: o.model,
            role: "执行".into(),
            result: o.result.into(),
            rounds: o.rounds,
            error_kind: o.error_kind,
        })
        .collect();
    append_lines(root, &lines)
}

/// 采纳成功时调。
pub(crate) fn record_adopt(root: &Path, run: &crate::engine::EngineRun, node: &GraphNode, at: &str) -> Result<(), String> {
    let Some(target) = node.target.as_ref() else {
        return Ok(());
    };
    append_lines(
        root,
        &[TrackLine {
            kind_of_line: "adopt".into(),
            at: at.into(),
            run_id: run.run_id.clone(),
            node_id: node.id.clone(),
            host: run.host.clone(),
            preset_id: run.preset_id.clone(),
            task_kind: run.task_kind.clone(),
            agent: target.agent.clone(),
            provider: target.provider.clone().unwrap_or_default(),
            model: target.model.clone(),
            role: target.role.clone(),
            ..TrackLine::default()
        }],
    )
}

/// 读全部行；坏行跳过。同一 (类型, run, 节点) 只留最后一行。
fn read_lines(root: &Path) -> Vec<TrackLine> {
    let Ok(text) = fs::read_to_string(outcomes_path(root)) else {
        return Vec::new();
    };
    let mut order: Vec<(String, String, String)> = Vec::new();
    let mut latest: HashMap<(String, String, String), TrackLine> = HashMap::new();
    for raw in text.lines() {
        let Ok(line) = serde_json::from_str::<TrackLine>(raw) else {
            continue;
        };
        let key = (line.kind_of_line.clone(), line.run_id.clone(), line.node_id.clone());
        if !latest.contains_key(&key) {
            order.push(key.clone());
        }
        latest.insert(key, line);
    }
    order.into_iter().filter_map(|key| latest.remove(&key)).collect()
}

// ── 标签与备注 ──────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct ModelNote {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct NotesFile {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub models: BTreeMap<String, ModelNote>,
}

pub(crate) fn load_notes(root: &Path) -> Result<NotesFile, String> {
    match fs::read(notes_path(root)) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!("model-notes.json 读不了：{e}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(NotesFile { version: 1, ..NotesFile::default() }),
        Err(error) => Err(format!("读取 model-notes.json 失败：{error}")),
    }
}

/// 改一个模型的标签或备注；`None` 表示不动。标签按表里的顺序存、去重，
/// 两项都空时删掉这一条。
pub(crate) fn set_note(
    root: &Path,
    key: &str,
    tags: Option<&str>,
    note: Option<&str>,
) -> Result<ModelNote, String> {
    let parsed_tags = match tags {
        Some(raw) => {
            let mut wanted: HashSet<&str> = HashSet::new();
            for part in raw.split([',', '，', '、']).map(str::trim).filter(|s| !s.is_empty()) {
                wanted.insert(resolve_kind(part)?);
            }
            Some(
                TASK_KINDS
                    .iter()
                    .map(|(id, _)| *id)
                    .filter(|id| wanted.contains(id))
                    .map(String::from)
                    .collect::<Vec<_>>(),
            )
        }
        None => None,
    };
    let _lock = crate::file_lock::FileLock::acquire(&root.join("model-notes.json.lock"), NOTES_LOCK_TIMEOUT)?;
    let mut file = load_notes(root)?;
    file.version = 1;
    let mut entry = file.models.get(key).cloned().unwrap_or_default();
    if let Some(tags) = parsed_tags {
        entry.tags = tags;
    }
    if let Some(note) = note {
        entry.note = note.trim().to_string();
    }
    if entry == ModelNote::default() {
        file.models.remove(key);
    } else {
        file.models.insert(key.to_string(), entry.clone());
    }
    crate::engine::write_json_atomic(&notes_path(root), &file)?;
    Ok(entry)
}

pub(crate) fn note_value(key: &str, note: &ModelNote) -> Value {
    json!({
        "model": key,
        "tags": note.tags,
        "tagNames": note.tags.iter().map(|t| kind_name(t)).collect::<Vec<_>>(),
        "note": note.note,
    })
}

// ── 汇总 ────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq)]
struct Counts {
    runs: u32,
    first_pass: u32,
    fixed_pass: u32,
    not_passed: u32,
    failed: u32,
    no_acceptance: u32,
    unjudged: u32,
    adopted: u32,
}

impl Counts {
    fn add(&mut self, result: &str) {
        self.runs += 1;
        match result {
            RESULT_FIRST_PASS => self.first_pass += 1,
            RESULT_FIXED_PASS => self.fixed_pass += 1,
            RESULT_NOT_PASSED => self.not_passed += 1,
            RESULT_FAILED => self.failed += 1,
            RESULT_NO_ACCEPTANCE => self.no_acceptance += 1,
            _ => self.unjudged += 1,
        }
    }

    fn judged(&self) -> u32 {
        self.first_pass + self.fixed_pass + self.not_passed
    }

    /// 样本够时的通过率（0–100，取整）。
    fn pass_rate(&self) -> Option<u32> {
        let judged = self.judged();
        (judged >= MIN_JUDGED).then(|| ((self.first_pass + self.fixed_pass) * 100 + judged / 2) / judged)
    }

    fn first_pass_rate(&self) -> Option<u32> {
        let judged = self.judged();
        (judged >= MIN_JUDGED).then(|| (self.first_pass * 100 + judged / 2) / judged)
    }

    /// `name` 给了就用它（合计写「全部」），否则按类型表取名，空类型为「未分类」。
    fn value(&self, kind: &str, name: Option<&str>) -> Value {
        json!({
            "kind": kind,
            "kindName": name.unwrap_or_else(|| kind_name(kind)),
            "runs": self.runs,
            "firstPass": self.first_pass,
            "fixedPass": self.fixed_pass,
            "notPassed": self.not_passed,
            "failed": self.failed,
            "noAcceptance": self.no_acceptance,
            "unjudged": self.unjudged,
            "adopted": self.adopted,
            "judged": self.judged(),
            "passRate": self.pass_rate(),
            "firstPassRate": self.first_pass_rate(),
        })
    }
}

/// 资源库里这个模型能不能派（与 `resources find` 同一套状态）。
fn usable_in(store: Option<&ResourceStore>, key: &str, now_millis: i64) -> Option<bool> {
    let store = store?;
    let ever = store.detected_at.is_some();
    for agent in &store.agents {
        for model in &agent.models {
            if model_key(&agent.id, &model.provider, &model.id) == key {
                if !ever {
                    return Some(false);
                }
                let (state, _) = crate::resources::model_state(model, agent.detected, now_millis);
                return Some(crate::model_find::USABLE_STATES.contains(&state));
            }
        }
    }
    Some(false)
}

/// `aicross stats` 的输出。`only_kind` 给了就只算这一类。
pub(crate) fn stats_value(
    root: &Path,
    only_kind: Option<&str>,
    store: Option<&ResourceStore>,
    now_millis: i64,
) -> Result<Value, String> {
    let notes = load_notes(root)?;
    let lines = read_lines(root);
    // 模型 → 类型 → 计数
    let mut table: BTreeMap<String, BTreeMap<String, Counts>> = BTreeMap::new();
    for line in &lines {
        if only_kind.is_some_and(|k| k != line.task_kind) {
            continue;
        }
        let key = model_key(&line.agent, &line.provider, &line.model);
        let counts = table.entry(key).or_default().entry(line.task_kind.clone()).or_default();
        match line.kind_of_line.as_str() {
            "outcome" => counts.add(&line.result),
            "adopt" => counts.adopted += 1,
            _ => {}
        }
    }
    for key in notes.models.keys() {
        table.entry(key.clone()).or_default();
    }
    let mut models = Vec::new();
    for (key, kinds) in &table {
        let note = notes.models.get(key).cloned().unwrap_or_default();
        let mut total = Counts::default();
        let mut by_kind = Vec::new();
        for (kind, counts) in kinds {
            total.runs += counts.runs;
            total.first_pass += counts.first_pass;
            total.fixed_pass += counts.fixed_pass;
            total.not_passed += counts.not_passed;
            total.failed += counts.failed;
            total.no_acceptance += counts.no_acceptance;
            total.unjudged += counts.unjudged;
            total.adopted += counts.adopted;
            by_kind.push(counts.value(kind, None));
        }
        let mut parts = key.splitn(3, ':');
        models.push(json!({
            "model": key,
            "agent": parts.next().unwrap_or_default(),
            "provider": parts.next().unwrap_or_default(),
            "id": parts.next().unwrap_or_default(),
            "usable": usable_in(store, key, now_millis),
            "tags": note.tags,
            "tagNames": note.tags.iter().map(|t| kind_name(t)).collect::<Vec<_>>(),
            "note": note.note,
            "total": total.value("", Some("全部")),
            "byKind": by_kind,
        }));
    }
    let wanted: Vec<&str> = match only_kind {
        Some(kind) => vec![kind],
        None => TASK_KINDS.iter().map(|(id, _)| *id).collect(),
    };
    let kinds: Vec<Value> = wanted
        .iter()
        .map(|kind| {
            let mut ranked: Vec<(String, u32, u32)> = table
                .iter()
                .filter_map(|(key, kinds)| {
                    let counts = kinds.get(*kind)?;
                    Some((key.clone(), counts.pass_rate()?, counts.judged()))
                })
                .collect();
            ranked.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)).then(a.0.cmp(&b.0)));
            let tagged: Vec<&String> = notes
                .models
                .iter()
                .filter(|(_, note)| note.tags.iter().any(|t| t == kind))
                .map(|(key, _)| key)
                .collect();
            json!({
                "kind": kind,
                "kindName": kind_name(kind),
                "ranked": ranked.iter().map(|(key, rate, judged)| json!({
                    "model": key,
                    "passRate": rate,
                    "judged": judged,
                    "usable": usable_in(store, key, now_millis),
                })).collect::<Vec<_>>(),
                "tagged": tagged,
            })
        })
        .collect();
    Ok(json!({
        "minJudged": MIN_JUDGED,
        "kindList": TASK_KINDS.iter().map(|(id, name)| json!({ "id": id, "name": name })).collect::<Vec<_>>(),
        "models": models,
        "kinds": kinds,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::{Message, Reply, Target};

    fn temp(name: &str) -> PathBuf {
        let dir = crate::paths::test_temp_dir().join(format!("track-{name}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn node(id: &str, parents: &[&str], role: &str, model: &str, status: &str, text: &str, at: &str) -> GraphNode {
        GraphNode {
            id: id.into(),
            kind: "turn".into(),
            parents: parents.iter().map(|s| s.to_string()).collect(),
            user: Message::default(),
            reply: Some(Reply {
                status: status.into(),
                text: text.into(),
                ..Reply::default()
            }),
            target: Some(Target {
                agent: "codex".into(),
                model: model.into(),
                role: role.into(),
                ..Target::default()
            }),
            created_at: at.into(),
            ..GraphNode::default()
        }
    }

    fn graph(nodes: Vec<GraphNode>) -> Graph {
        let mut g = Graph { version: 1, id: "g".into(), ..Graph::default() };
        for n in nodes {
            g.nodes.insert(n.id.clone(), n);
        }
        g
    }

    fn results(g: &Graph) -> Vec<(&'static str, usize)> {
        chain_outcomes(g).into_iter().map(|o| (o.result, o.rounds)).collect()
    }

    #[test]
    fn kinds_accept_chinese_and_ids() {
        assert_eq!(resolve_kind("前端").unwrap(), "frontend");
        assert_eq!(resolve_kind(" Data ").unwrap(), "data");
        let error = resolve_kind("视频").unwrap_err();
        assert!(error.contains("前端（frontend）"), "{error}");
    }

    #[test]
    fn model_key_needs_three_parts() {
        assert_eq!(parse_model_key("claude::opus").unwrap(), "claude::opus");
        assert!(parse_model_key("claude:opus").is_err());
        assert!(parse_model_key(":p:m").is_err());
    }

    #[test]
    fn first_pass_and_no_acceptance() {
        let pass = graph(vec![
            node("e", &["root"], "执行", "m", "done", "ok", "1"),
            node("a", &["e"], "验收", "k", "done", "结论：PASS", "2"),
        ]);
        assert_eq!(results(&pass), vec![(RESULT_FIRST_PASS, 1)]);
        let shared = graph(vec![node("e", &["root"], "执行", "m", "done", "ok", "1")]);
        assert_eq!(results(&shared), vec![(RESULT_NO_ACCEPTANCE, 1)]);
    }

    #[test]
    fn correction_round_then_pass_counts_as_fixed() {
        let g = graph(vec![
            node("e1", &["root"], "执行", "m", "done", "", "1"),
            node("a1", &["e1"], "验收", "k", "done", "NEEDS_FIX", "2"),
            node("e2", &["a1"], "执行", "m", "done", "", "3"),
            node("a2", &["e2"], "验收", "k", "done", "PASS", "4"),
        ]);
        assert_eq!(results(&g), vec![(RESULT_FIXED_PASS, 2)]);
    }

    #[test]
    fn needs_fix_at_limit_is_not_passed() {
        let g = graph(vec![
            node("e1", &["root"], "执行", "m", "done", "", "1"),
            node("a1", &["e1"], "验收", "k", "done", "NEEDS_FIX", "2"),
        ]);
        assert_eq!(results(&g), vec![(RESULT_NOT_PASSED, 1)]);
    }

    #[test]
    fn validation_retry_joins_the_chain() {
        let mut first = node("e1", &["root"], "执行", "m", "failed", "", "1");
        first.reply.as_mut().unwrap().error_kind = Some("validation_failed".into());
        let mut retry = node("e2", &["root"], "执行", "m", "failed", "", "3");
        retry.user.text = "[修正 1/1] 验证命令未通过，请修正后重新运行。".into();
        retry.reply.as_mut().unwrap().error_kind = Some("validation_failed".into());
        let g = graph(vec![
            first,
            node("a1", &["e1"], "验收", "k", "failed", "验证命令未通过，未送审", "2"),
            retry,
        ]);
        assert_eq!(results(&g), vec![(RESULT_NOT_PASSED, 2)]);

        // 早期版本的重试文本没有「[修正 N/M]」前缀。
        let mut old = g.clone();
        old.nodes.get_mut("e2").unwrap().user.text = "验证命令未通过，请修正后重新运行。".into();
        assert_eq!(results(&old), vec![(RESULT_NOT_PASSED, 2)]);
    }

    #[test]
    fn errors_count_as_failed_and_cancel_or_fallback_or_draft_are_skipped() {
        let mut error = node("e", &["root"], "执行", "m", "failed", "", "1");
        error.reply.as_mut().unwrap().error_kind = Some("timeout".into());
        assert_eq!(results(&graph(vec![error])), vec![(RESULT_FAILED, 1)]);

        let mut cancelled = node("e", &["root"], "执行", "m", "failed", "", "1");
        cancelled.reply.as_mut().unwrap().error_kind = Some("cancelled".into());
        assert!(results(&graph(vec![cancelled])).is_empty());

        let mut switched = node("e", &["root"], "执行", "m", "done", "", "1");
        switched.reply.as_mut().unwrap().fallback_from = Some("codex/m".into());
        assert!(results(&graph(vec![switched])).is_empty());

        assert!(results(&graph(vec![node("e", &["root"], "执行", "m", "draft", "", "1")])).is_empty());
    }

    #[test]
    fn failed_acceptance_is_unjudged() {
        let g = graph(vec![
            node("e", &["root"], "执行", "m", "done", "", "1"),
            node("a", &["e"], "验收", "k", "failed", "超时", "2"),
        ]);
        assert_eq!(results(&g), vec![(RESULT_UNJUDGED, 1)]);
    }

    #[test]
    fn notes_set_clear_and_reject_unknown_tags() {
        let root = temp("notes");
        let entry = set_note(&root, "claude::opus", Some("文案,前端,文案"), Some(" 审美好 ")).unwrap();
        assert_eq!(entry.tags, vec!["frontend", "writing"]);
        assert_eq!(entry.note, "审美好");
        let entry = set_note(&root, "claude::opus", Some(""), None).unwrap();
        assert!(entry.tags.is_empty());
        assert_eq!(entry.note, "审美好");
        set_note(&root, "claude::opus", None, Some("")).unwrap();
        assert!(load_notes(&root).unwrap().models.is_empty());
        assert!(set_note(&root, "claude::opus", Some("视频"), None).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    fn outcome(run: &str, kind: &str, model: &str, result: &str) -> TrackLine {
        TrackLine {
            kind_of_line: "outcome".into(),
            at: "t".into(),
            run_id: run.into(),
            node_id: "n".into(),
            task_kind: kind.into(),
            agent: "codex".into(),
            model: model.into(),
            result: result.into(),
            rounds: 1,
            ..TrackLine::default()
        }
    }

    #[test]
    fn stats_rate_needs_enough_samples_and_ranks() {
        let root = temp("stats");
        let mut lines = Vec::new();
        for i in 0..5 {
            lines.push(outcome(&format!("a{i}"), "frontend", "a", if i < 4 { RESULT_FIRST_PASS } else { RESULT_NOT_PASSED }));
            lines.push(outcome(&format!("b{i}"), "frontend", "b", if i < 2 { RESULT_FIXED_PASS } else { RESULT_NOT_PASSED }));
        }
        lines.push(outcome("c0", "frontend", "c", RESULT_FIRST_PASS));
        // 同一 run 同一节点重复写只算最后一行。
        lines.push(outcome("c0", "frontend", "c", RESULT_FIRST_PASS));
        append_lines(&root, &lines).unwrap();
        let mut adopt = outcome("a0", "frontend", "a", "");
        adopt.kind_of_line = "adopt".into();
        append_lines(&root, &[adopt.clone(), adopt]).unwrap();
        set_note(&root, "codex::c", Some("前端"), None).unwrap();

        let value = stats_value(&root, Some("frontend"), None, 0).unwrap();
        let ranked = value["kinds"][0]["ranked"].as_array().unwrap();
        assert_eq!(ranked.len(), 2, "c 只有 1 次，不排");
        assert_eq!(ranked[0]["model"], "codex::a");
        assert_eq!(ranked[0]["passRate"], 80);
        assert_eq!(ranked[1]["passRate"], 40);
        assert_eq!(value["kinds"][0]["tagged"][0], "codex::c");
        let models = value["models"].as_array().unwrap();
        let a = models.iter().find(|m| m["model"] == "codex::a").unwrap();
        assert_eq!(a["byKind"][0]["adopted"], 1);
        assert_eq!(a["byKind"][0]["firstPassRate"], 80);
        let c = models.iter().find(|m| m["model"] == "codex::c").unwrap();
        assert_eq!(c["byKind"][0]["runs"], 1);
        assert!(c["byKind"][0]["passRate"].is_null());
        assert_eq!(c["tags"][0], "frontend");
        let _ = fs::remove_dir_all(&root);
    }
}
