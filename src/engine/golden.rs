//! 逐夹具金标准重放测试（fixtures/engine-golden/*.json）。
//!
//! 夹具录自 TypeScript 参照实现（`runConsoleDispatch`）。前端生成器
//! 已随 TASK-40 删除，原文见 git 历史与 `fixtures/engine-golden/README.md`；
//! 本模块把 input 段驱动给 Rust 引擎（`plan_dispatch` + `run_pipeline` +
//! 记录型 PipelineSink），产出按 UUID / 时间戳首现序归一化后与夹具逐字段
//! 比较。夹具是契约，不一致时改 Rust，不改夹具。

use super::*;
use serde_json::{json, Map, Value};

/// TS 侧冻结的 Date（vi.setSystemTime）；引擎产出的全部时间戳都等于它。
const FROZEN_TIME: &str = "2026-09-30T00:00:00.000Z";

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/engine-golden")
}

// ── 归一化：UUID v4 与 ISO 毫秒时间戳按首现序换成 UUID_n / TIME_n ──────────

fn is_uuid_v4(text: &str) -> bool {
    let b = text.as_bytes();
    if b.len() != 36 {
        return false;
    }
    let hex = |c: u8| c.is_ascii_hexdigit();
    for (i, &c) in b.iter().enumerate() {
        match i {
            8 | 13 | 18 | 23 => {
                if c != b'-' {
                    return false;
                }
            }
            14 => {
                if c != b'4' {
                    return false;
                }
            }
            19 => {
                if !matches!(c, b'8' | b'9' | b'a' | b'b' | b'A' | b'B') {
                    return false;
                }
            }
            _ => {
                if !hex(c) {
                    return false;
                }
            }
        }
    }
    true
}

fn is_iso_ms(text: &str) -> bool {
    let b = text.as_bytes();
    if b.len() != 24 || b[23] != b'Z' {
        return false;
    }
    for (i, &c) in b.iter().enumerate().take(23) {
        let digit = matches!(i, 0..=3 | 5..=6 | 8..=9 | 11..=12 | 14..=15 | 17..=18 | 20..=22);
        if digit {
            if !c.is_ascii_digit() {
                return false;
            }
        } else {
            let want = match i {
                4 | 7 => b'-',
                10 => b'T',
                13 | 16 => b':',
                19 => b'.',
                _ => unreachable!(),
            };
            if c != want {
                return false;
            }
        }
    }
    true
}

/// 与 TS 测试里的 normalize 同规则：对象键也参与替换，遍序 = JSON 键序。
fn normalize(value: &Value) -> Value {
    let mut ids: HashMap<String, String> = HashMap::new();
    let mut times: HashMap<String, String> = HashMap::new();
    fn visit(
        item: &Value,
        ids: &mut HashMap<String, String>,
        times: &mut HashMap<String, String>,
    ) -> Value {
        match item {
            Value::String(text) => {
                if is_uuid_v4(text) {
                    let next = format!("UUID_{}", ids.len() + 1);
                    return Value::String(ids.entry(text.clone()).or_insert(next).clone());
                }
                if is_iso_ms(text) {
                    let next = format!("TIME_{}", times.len() + 1);
                    return Value::String(times.entry(text.clone()).or_insert(next).clone());
                }
                item.clone()
            }
            Value::Array(items) => Value::Array(items.iter().map(|i| visit(i, ids, times)).collect()),
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(k, v)| {
                        (
                            match visit(&Value::String(k.clone()), ids, times) {
                                Value::String(s) => s,
                                _ => unreachable!(),
                            },
                            visit(v, ids, times),
                        )
                    })
                    .collect(),
            ),
            _ => item.clone(),
        }
    }
    visit(value, &mut ids, &mut times)
}

/// 序列化等值规则：TS 的 undefined 在 IPC 边界被剥掉，Rust 的 None → null，
/// 比较前递归剥掉 null。
fn strip_nulls(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k.clone(), strip_nulls(v)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(strip_nulls).collect()),
        _ => value.clone(),
    }
}

/// TS 的条件 spread：这四个布尔字段为 false 时键不存在。
fn strip_false_flags(node: &mut Map<String, Value>) {
    for key in [
        "rollbackBefore",
        "reviewMaterialValidation",
        "fixRound",
        "interactive",
    ] {
        if node.get(key) == Some(&Value::Bool(false)) {
            node.remove(key);
        }
    }
}

/// 录制我侧实际发出的请求，整成夹具形状（剥 null、剥 false 键、剥 settings.swarmLimit）。
fn record_request(request: &DispatchStageRequest) -> Value {
    let mut value = strip_nulls(&serde_json::to_value(request).expect("序列化请求"));
    if let Some(settings) = value.get_mut("settings").and_then(Value::as_object_mut) {
        settings.remove("swarmLimit");
    }
    if let Some(nodes) = value.get_mut("nodes").and_then(Value::as_array_mut) {
        for node in nodes {
            if let Some(map) = node.as_object_mut() {
                strip_false_flags(map);
            }
        }
    }
    value
}

// ── input 段的 TIME_n 反归一化 ────────────────────────────────────────────

/// 夹具 input 里的 TIME_n 换回真实时间戳：本 run 的 started_at 那个 token 是
/// 冻结时间（引擎产出的所有时间戳也等于它，归一化后回到同一 token），其余
/// token 按编号序给互不相同的合成时间（2026-09-28T00:00:00.NNNZ）。input 段
/// 不允许出现 UUID_n（既有图的 id 都是字面量）。
fn denormalize_input(fixture: &Value) -> Value {
    let input = &fixture["input"];
    let run_id = input["event"]["run_id"].as_str().expect("event.run_id");
    let mut frozen_token = String::new();
    for run in input["runs"].as_array().expect("input.runs") {
        if run["run_id"].as_str() == Some(run_id) {
            frozen_token = run["started_at"].as_str().expect("本 run started_at").to_string();
        }
    }
    assert!(
        frozen_token.starts_with("TIME_"),
        "input.runs 里找不到本 run 的时间 token"
    );
    let mut tokens: Vec<String> = vec![];
    fn collect(item: &Value, tokens: &mut Vec<String>) {
        match item {
            Value::String(text) => {
                if text.starts_with("UUID_") {
                    panic!("input 段不允许出现 UUID token：{text}");
                }
                if text.starts_with("TIME_") && !tokens.contains(text) {
                    tokens.push(text.clone());
                }
            }
            Value::Array(items) => items.iter().for_each(|i| collect(i, tokens)),
            Value::Object(map) => {
                for (k, v) in map {
                    assert!(!k.starts_with("TIME_") && !k.starts_with("UUID_"), "input 键含 token：{k}");
                    collect(v, tokens);
                }
            }
            _ => {}
        }
    }
    collect(input, &mut tokens);
    // token 编号 = 夹具首现序；按编号序分配合成时间，保证互不相同且不等于冻结时间。
    let mut by_token: HashMap<String, String> = HashMap::new();
    let mut seq = 0usize;
    let mut numbered = tokens.clone();
    numbered.sort_by_key(|t| t.trim_start_matches("TIME_").parse::<usize>().unwrap());
    for token in numbered {
        if token == frozen_token {
            by_token.insert(token, FROZEN_TIME.to_string());
        } else {
            by_token.insert(token, format!("2026-09-28T00:00:00.{seq:03}Z"));
            seq += 1;
        }
    }
    fn replace(item: &Value, by_token: &HashMap<String, String>) -> Value {
        match item {
            Value::String(text) => Value::String(
                by_token.get(text).cloned().unwrap_or_else(|| text.clone()),
            ),
            Value::Array(items) => Value::Array(items.iter().map(|i| replace(i, by_token)).collect()),
            Value::Object(map) => Value::Object(
                map.iter().map(|(k, v)| (k.clone(), replace(v, by_token))).collect(),
            ),
            _ => item.clone(),
        }
    }
    replace(input, &by_token)
}

// ── 记录型 PipelineSink：dispatch 按夹具 stages 注入 ──────────────────────

struct RecordingSink<'a> {
    fixture_stages: &'a [Value],
    cursor: usize,
    /// 夹具 nodeId token → 我侧实际 id（跨阶段保持，重复出现须一致）。
    id_tokens: HashMap<String, String>,
    recorded: Vec<Value>,
    updates: Vec<Value>,
    last_saved: Option<Graph>,
}

impl RecordingSink<'_> {
    fn parse_results(&self, fixture_results: &[Value]) -> Vec<DispatchNodeResult> {
        fixture_results
            .iter()
            .map(|result| {
                let mut result = result.clone();
                let token = result["nodeId"].as_str().expect("结果 nodeId");
                let actual = self
                    .id_tokens
                    .get(token)
                    .unwrap_or_else(|| panic!("结果引用了未见过的节点 token：{token}"));
                result["nodeId"] = Value::String(actual.clone());
                serde_json::from_value::<DispatchNodeResult>(result).expect("解析注入结果")
            })
            .collect()
    }
}

impl PipelineSink for RecordingSink<'_> {
    fn save_graph(&mut self, graph: &Graph) -> Result<Option<Graph>, String> {
        self.last_saved = Some(graph.clone());
        Ok(None)
    }

    fn update(&mut self, update: &RunUpdate) -> Result<(), String> {
        self.updates.push(serde_json::to_value(update).expect("序列化 run 摘要"));
        Ok(())
    }

    fn dispatch(&mut self, request: &DispatchStageRequest) -> Result<DispatchStageResult, String> {
        let Some(fixture_stage) = self.fixture_stages.get(self.cursor) else {
            panic!("场景未提供第 {} 次派发的注入结果", self.cursor + 1);
        };
        self.cursor += 1;
        let fixture_nodes = fixture_stage["request"]["nodes"]
            .as_array()
            .expect("夹具请求 nodes");
        assert_eq!(
            fixture_nodes.len(),
            request.nodes.len(),
            "第 {} 次派发的节点数与夹具不一致",
            self.cursor
        );
        for (fixture_node, node) in fixture_nodes.iter().zip(&request.nodes) {
            let token = fixture_node["nodeId"].as_str().expect("夹具 nodeId");
            match self.id_tokens.get(token) {
                Some(existing) => assert_eq!(
                    existing, &node.node_id,
                    "token {token} 再次出现时对应了不同节点"
                ),
                None => {
                    self.id_tokens.insert(token.to_string(), node.node_id.clone());
                }
            }
        }
        self.recorded.push(json!({
            "request": record_request(request),
            "injected": fixture_stage["injected"].clone(),
        }));
        let injected = &fixture_stage["injected"];
        if let Some(message) = injected["throw"].as_str() {
            return Err(message.to_string());
        }
        let results = if let Some(replay) = injected["replay"].as_u64() {
            let source = &self.fixture_stages[replay as usize]["injected"]["results"];
            self.parse_results(source.as_array().expect("replay 源结果"))
        } else {
            self.parse_results(injected["results"].as_array().expect("注入结果"))
        };
        Ok(DispatchStageResult {
            run_id: request.run_id.clone(),
            results,
            state_file: String::new(),
        })
    }
}

// ── 场景驱动 ──────────────────────────────────────────────────────────────

/// TS `selectConsoleGraph` 的无窗口半边：同项目（归一化路径）的图里取
/// updated_at 最新的一张。夹具录制自续写同项目图的窗口口径，`aicross
/// dispatch` 本身总是新建图。
fn pick_project_graph(graphs: Vec<Graph>, project: &str) -> Option<Graph> {
    let wanted = normalized(project);
    let mut graphs: Vec<Graph> = graphs
        .into_iter()
        .filter(|g| normalized(&g.project) == wanted)
        .collect();
    graphs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    graphs.into_iter().next()
}

fn run_fixture(fixture: &Value) -> Value {
    let input = denormalize_input(fixture);
    let event = &input["event"];
    let run_id = event["run_id"].as_str().unwrap().to_string();
    let project = event["project"].as_str().unwrap().to_string();
    let text = event["text"].as_str().unwrap().to_string();
    let preset = event["preset"].as_str().map(str::to_string);
    let attachments: Vec<String> = event
        .get("attachments")
        .map(|v| serde_json::from_value(v.clone()).expect("attachments"))
        .unwrap_or_default();
    let targets: Vec<Target> = event
        .get("targets")
        .map(|v| serde_json::from_value(v.clone()).expect("targets"))
        .unwrap_or_default();
    let settings = settings_from_value(input["settings"].as_object().expect("settings"));
    let models: Vec<ResourceModel> =
        serde_json::from_value(input["models"].clone()).expect("models");
    let presets: Vec<Preset> = serde_json::from_value(input["presets"].clone()).expect("presets");
    let roles: RoleSettings = serde_json::from_value(input["roles"].clone()).expect("roles");
    let resources_error = input["resourcesError"].as_str().map(str::to_string);
    let active_runs = input["runs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|run| {
            run["status"].as_str() == Some("running") && run["run_id"].as_str() != Some(run_id.as_str())
        })
        .count();
    let graphs: Vec<Graph> =
        serde_json::from_value(input["existingGraphs"].clone()).expect("existingGraphs");
    let existing = pick_project_graph(graphs, &project);
    let git = input["git"].clone();
    let mut git_dirty_count = move || {
        if git["state"].as_str() == Some("ok") {
            Some(
                git["uncommitted"].as_array().map_or(0, Vec::len)
                    + git["untracked"].as_array().map_or(0, Vec::len),
            )
        } else {
            None
        }
    };

    // 与 TS 测试的替身一致：UUID v4 形状的计数器 + 冻结时间。
    let mut seq = 0usize;
    let mut ids = move || {
        seq += 1;
        format!("00000000-0000-4000-8000-{seq:012}")
    };
    let frozen = DateTime::parse_from_rfc3339(FROZEN_TIME)
        .unwrap()
        .with_timezone(&Utc);
    let mut clock = move || frozen;

    // TASK-46：金标准夹具不涉及 modes.json，按空配置与经济缺省重放。
    let mut plan_input = PlanInput {
        run_id: &run_id,
        project: &project,
        text: &text,
        preset: preset.as_deref(),
        attachments: &attachments,
        targets: &targets,
        settings: &settings,
        models: &models,
        resources_error: resources_error.as_deref(),
        presets: &presets,
        roles: &roles,
        host_modes: None,
        default_preset: "builtin-economy",
        active_runs,
        existing,
        git_dirty_count: &mut git_dirty_count,
    };

    let fixture_stages = fixture["stages"].as_array().expect("stages");
    let (stages, run_updates, final_graph) = match plan_dispatch(&mut plan_input, &mut ids, &mut clock) {
        Plan::Rejected { reason, graph_id } => {
            let update = json!({
                "run_id": run_id,
                "graph_id": graph_id,
                "status": "rejected",
                "nodes": [],
                "leftover": [],
                "in_flight_node_ids": [],
                "reason": reason,
            });
            (vec![], vec![update], Value::Null)
        }
        Plan::Ready {
            graph,
            created,
            original,
            notes: _,
        } => {
            let mut sink = RecordingSink {
                fixture_stages,
                cursor: 0,
                id_tokens: HashMap::new(),
                recorded: vec![],
                updates: vec![],
                last_saved: None,
            };
            let outcome = run_pipeline(
                &run_id,
                &project,
                &settings,
                graph,
                &created,
                &models,
                &original,
                &mut sink,
                &mut ids,
                &mut clock,
                &|| false,
            )
            .expect("run_pipeline 不应失败（sink 全部注入成功）");
            assert_eq!(sink.cursor, fixture_stages.len(), "注入结果没有用完");
            let last_saved = sink.last_saved.expect("save_graph 至少被调一次");
            // 收尾落图就是 run_pipeline 返回的图；返回的 error 就是终态摘要的 reason。
            assert_eq!(
                serde_json::to_value(&last_saved).unwrap(),
                serde_json::to_value(&outcome.graph).unwrap()
            );
            assert_eq!(
                outcome.error,
                sink.updates
                    .last()
                    .and_then(|u| u["reason"].as_str().map(str::to_string))
                    .unwrap_or_default()
            );
            (
                sink.recorded,
                sink.updates,
                strip_nulls(&serde_json::to_value(&last_saved).expect("序列化最终图")),
            )
        }
    };
    json!({
        "name": fixture["name"],
        "input": input,
        "stages": stages,
        "runUpdates": run_updates,
        "finalGraph": final_graph,
        "notes": fixture["notes"],
    })
}

#[test]
fn engine_golden_fixtures_replay() {
    let dir = fixture_dir();
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("读取夹具目录")
        .map(|entry| entry.expect("读取夹具条目").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    assert_eq!(files.len(), 31, "金标准夹具数量变了：{}", dir.display());
    for path in files {
        let slug = path.file_stem().unwrap().to_string_lossy().to_string();
        let fixture: Value = serde_json::from_slice(
            &fs::read(&path).unwrap_or_else(|e| panic!("读取夹具 {slug} 失败：{e}")),
        )
        .unwrap_or_else(|e| panic!("解析夹具 {slug} 失败：{e}"));
        let actual = normalize(&run_fixture(&fixture));
        if actual != fixture {
            let diff = first_diff(&fixture, &actual, "$".to_string())
                .unwrap_or_else(|| "（顺序差异？）".to_string());
            let actual_path = crate::paths::test_temp_dir().join(format!("aicross-golden-{slug}.actual.json"));
            fs::write(&actual_path, serde_json::to_string_pretty(&actual).unwrap()).unwrap();
            panic!("夹具 {slug} 重放不一致：{diff}\n实际输出已写到 {}", actual_path.display());
        }
    }
}

/// 递归找第一处不一致，返回路径与两侧值。
fn first_diff(expected: &Value, actual: &Value, path: String) -> Option<String> {
    match (expected, actual) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in a {
                let Some(other) = b.get(k) else {
                    return Some(format!("{path}.{k}：实际没有，夹具 = {v}"));
                };
                if let Some(found) = first_diff(v, other, format!("{path}.{k}")) {
                    return Some(found);
                }
            }
            for k in b.keys() {
                if !a.contains_key(k) {
                    return Some(format!("{path}.{k}：夹具没有，实际 = {}", b[k]));
                }
            }
            None
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                return Some(format!("{path}：长度 {} != {}", a.len(), b.len()));
            }
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                if let Some(found) = first_diff(x, y, format!("{path}[{i}]")) {
                    return Some(found);
                }
            }
            None
        }
        _ => {
            if expected == actual {
                None
            } else {
                Some(format!("{path}：夹具 = {expected}，实际 = {actual}"))
            }
        }
    }
}
