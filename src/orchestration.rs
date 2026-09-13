//! 第 2 期「对话图」Rust 侧：模型清单 + 预设与图的持久化命令。
//!
//! 契约见 tasks/TASK-02-orchestration.md A 节。字段名（snake_case）是跨期
//! 契约，前端 graph/types.ts 与之逐字段对应，改名必须两期同步。
//!
//! 持久化位置（全部在 `~/.aicross` 下，见 paths.rs）：
//! - `resources.json`：第 0/1 期资源接入写的模型清单，本期只读。
//! - `presets.json`：用户预设 `{ version: 1, presets: [...] }`；内置预设
//!   不落盘，由 `list_presets` 每次合并。
//! - `graphs/<id>.json`：一图一文件，原子写。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::paths::aicross_root;

// ── 对话图数据模型（A 节契约） ─────────────────────────────────────────────

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Graph {
    pub version: u32,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub root: String,
    #[serde(default)]
    pub nodes: HashMap<String, GraphNode>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct GraphNode {
    #[serde(default)]
    pub id: String,
    /// "material" | "turn" | "accept"
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub parents: Vec<String>,
    #[serde(default)]
    pub user: Message,
    #[serde(default)]
    pub reply: Option<Reply>,
    #[serde(default)]
    pub target: Option<Target>,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Message {
    #[serde(default)]
    pub text: String,
    /// 附件文件路径
    #[serde(default)]
    pub attachments: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Reply {
    /// "draft" | "pending" | "done" | "failed"（本期只产生 draft）
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub model_reported: Option<String>,
    #[serde(default)]
    pub tokens_in: Option<u64>,
    #[serde(default)]
    pub tokens_out: Option<u64>,
    #[serde(default)]
    pub ms: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Target {
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: String,
    /// 出方案 | 执行 | 验收 | 验证 | 自定义文本
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub tier: Option<String>,
    /// 关 | 低 | 中 | 高
    #[serde(default)]
    pub thinking: String,
    /// 共享 | 盲 | 指定上游
    #[serde(default)]
    pub visibility: String,
    /// visibility == "指定上游" 时的上游节点 id 列表
    #[serde(default)]
    pub upstreams: Option<Vec<String>>,
    /// 只读 | 单写者 | worktree
    #[serde(default)]
    pub write: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Preset {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub builtin: bool,
    #[serde(default)]
    pub stages: Vec<Stage>,
}

/// 同阶段并行，阶段间串行。
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Stage {
    #[serde(default)]
    pub nodes: Vec<Target>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PresetFile {
    pub version: u32,
    #[serde(default)]
    pub presets: Vec<Preset>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GraphMeta {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub updated_at: String,
}

// ── resources.json 只读镜像（shape 见 port/src-tauri-old/src/models.rs） ───

#[derive(Clone, Debug, Default, Deserialize)]
struct ResourceStore {
    #[serde(default)]
    #[allow(dead_code)]
    version: u32,
    #[serde(default)]
    agents: Vec<ResourceAgent>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct ResourceAgent {
    #[serde(default)]
    #[allow(dead_code)]
    id: String,
    #[serde(default)]
    models: Vec<ResourceModel>,
}

/// 与 resources.json 的 ResourceModel 逐字段对应；全部加 default 容忍缺字段。
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DispatchableModels {
    pub models: Vec<ResourceModel>,
    pub error: Option<String>,
}

// ── 内置预设（不落盘；builtin=true 不可删不可改） ─────────────────────────

fn blank_target() -> Target {
    Target::default()
}

fn builtin_presets() -> Vec<Preset> {
    let shared = Preset {
        id: "builtin-shared".into(),
        name: "共享模式".into(),
        builtin: true,
        stages: vec![Stage {
            nodes: vec![Target {
                role: "执行".into(),
                thinking: "中".into(),
                visibility: "共享".into(),
                write: "只读".into(),
                ..blank_target()
            }],
        }],
    };
    let economy = Preset {
        id: "builtin-economy".into(),
        name: "经济模式".into(),
        builtin: true,
        stages: vec![
            Stage {
                nodes: vec![Target {
                    role: "出方案".into(),
                    tier: Some("高".into()),
                    thinking: "高".into(),
                    visibility: "共享".into(),
                    write: "只读".into(),
                    ..blank_target()
                }],
            },
            Stage {
                nodes: vec![Target {
                    role: "执行".into(),
                    tier: Some("低".into()),
                    thinking: "低".into(),
                    visibility: "共享".into(),
                    write: "只读".into(),
                    ..blank_target()
                }],
            },
            Stage {
                nodes: vec![Target {
                    role: "验收".into(),
                    thinking: "中".into(),
                    visibility: "盲".into(),
                    write: "只读".into(),
                    ..blank_target()
                }],
            },
        ],
    };
    let blind = Preset {
        id: "builtin-blind".into(),
        name: "盲审模式".into(),
        builtin: true,
        stages: vec![Stage {
            nodes: (0..3)
                .map(|_| Target {
                    role: "验证".into(),
                    thinking: "中".into(),
                    visibility: "盲".into(),
                    write: "只读".into(),
                    ..blank_target()
                })
                .collect(),
        }],
    };
    vec![shared, economy, blind]
}

// ── 内部读写函数（接受 root 路径，命令函数只包一层 aicross_root()） ────────

fn resources_path(root: &Path) -> PathBuf {
    root.join("resources.json")
}

fn presets_path(root: &Path) -> PathBuf {
    root.join("presets.json")
}

fn graphs_dir(root: &Path) -> PathBuf {
    root.join("graphs")
}

/// 原子写：先写同目录临时文件（flush + sync）再 rename。Windows 上
/// `std::fs::rename` 走 MoveFileExW(MOVEFILE_REPLACE_EXISTING)，可以覆盖
/// 已存在的目标；失败时删目标重试一次只是锁占用等偶发情形的兜底。
/// 做法参考 port/src-tauri-old/src/config.rs 的 write_json_in（我们自己的 fork）。
fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let directory = path
        .parent()
        .ok_or_else(|| format!("写入 {} 失败：目标没有父目录", path.display()))?;
    fs::create_dir_all(directory)
        .map_err(|e| format!("创建目录 {} 失败：{e}", directory.display()))?;
    let contents =
        serde_json::to_string_pretty(value).map_err(|e| format!("生成 JSON 失败：{e}"))?;
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
                // rename 一般可覆盖目标；失败多是目标被占用，删目标后兜底重试一次。
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

fn validate_graph_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("图 id 不能为空".to_string());
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!("图 id 只许字母、数字、-、_：{id}"));
    }
    Ok(())
}

fn dispatchable_from(root: &Path) -> DispatchableModels {
    let path = resources_path(root);
    let contents = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return DispatchableModels {
                models: Vec::new(),
                error: Some("未找到 resources.json，尚未接入资源".to_string()),
            }
        }
        Err(e) => {
            return DispatchableModels {
                models: Vec::new(),
                error: Some(format!("读取 resources.json 失败：{e}")),
            }
        }
    };
    match serde_json::from_str::<ResourceStore>(&contents) {
        Ok(store) => DispatchableModels {
            models: store
                .agents
                .into_iter()
                .flat_map(|agent| agent.models)
                .filter(|model| model.status == "已接入")
                .collect(),
            error: None,
        },
        Err(e) => DispatchableModels {
            models: Vec::new(),
            error: Some(format!("resources.json 解析失败：{e}")),
        },
    }
}

/// 内置三个预设 + presets.json 里的用户预设；文件不存在、解析失败或
/// version != 1 时拒绝该文件、只返回内置（与 graphs 的版本校验对齐）。
fn presets_from(root: &Path) -> PresetFile {
    let mut presets = builtin_presets();
    let path = presets_path(root);
    if let Ok(contents) = fs::read_to_string(&path) {
        if let Ok(file) = serde_json::from_str::<PresetFile>(&contents) {
            if file.version == 1 {
                presets.extend(file.presets);
            }
        }
    }
    PresetFile {
        version: 1,
        presets,
    }
}

fn save_user_presets_in(root: &Path, presets: Vec<Preset>) -> Result<(), String> {
    // 防御性剔除 builtin=true 条目：内置不可改不可删，由 list_presets
    // 永远合并内置保证。
    let presets: Vec<Preset> = presets.into_iter().filter(|p| !p.builtin).collect();
    let file = PresetFile {
        version: 1,
        presets,
    };
    write_json_atomic(&presets_path(root), &file)
}

fn save_graph_in(root: &Path, graph: &Graph) -> Result<(), String> {
    if graph.version != 1 {
        return Err(format!("图 version 只支持 1，收到 {}", graph.version));
    }
    validate_graph_id(&graph.id)?;
    write_json_atomic(&graphs_dir(root).join(format!("{}.json", graph.id)), graph)
}

fn load_graph_in(root: &Path, id: &str) -> Result<Option<Graph>, String> {
    validate_graph_id(id)?;
    let path = graphs_dir(root).join(format!("{id}.json"));
    let contents = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("读取 {} 失败：{e}", path.display())),
    };
    let graph: Graph =
        serde_json::from_str(&contents).map_err(|e| format!("解析 {} 失败：{e}", path.display()))?;
    if graph.version != 1 {
        return Err(format!("图 {} version 只支持 1，收到 {}", id, graph.version));
    }
    Ok(Some(graph))
}

fn list_graphs_in(root: &Path) -> Vec<GraphMeta> {
    let dir = graphs_dir(root);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    let mut metas: Vec<GraphMeta> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        // 解析失败的跳过，不让一张坏图拖垮整个列表。
        let Ok(contents) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(graph) = serde_json::from_str::<Graph>(&contents) else {
            continue;
        };
        metas.push(GraphMeta {
            id: graph.id,
            title: graph.title,
            updated_at: graph.updated_at,
        });
    }
    // 最近更新的排前面（updated_at 是 ISO 字符串，字典序即时间序）。
    metas.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    metas
}

// ── Tauri 命令 ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_dispatchable_models() -> DispatchableModels {
    match aicross_root() {
        Ok(root) => dispatchable_from(&root),
        Err(e) => DispatchableModels {
            models: Vec::new(),
            error: Some(format!("无法确定用户目录：{e}")),
        },
    }
}

#[tauri::command]
pub fn list_presets() -> PresetFile {
    match aicross_root() {
        Ok(root) => presets_from(&root),
        Err(_) => PresetFile {
            version: 1,
            presets: builtin_presets(),
        },
    }
}

#[tauri::command]
pub fn save_user_presets(presets: Vec<Preset>) -> Result<(), String> {
    save_user_presets_in(&aicross_root()?, presets)
}

#[tauri::command]
pub fn save_graph(graph: Graph) -> Result<(), String> {
    save_graph_in(&aicross_root()?, &graph)
}

#[tauri::command]
pub fn load_graph(id: String) -> Result<Option<Graph>, String> {
    load_graph_in(&aicross_root()?, &id)
}

#[tauri::command]
pub fn list_graphs() -> Vec<GraphMeta> {
    match aicross_root() {
        Ok(root) => list_graphs_in(&root),
        Err(_) => Vec::new(),
    }
}

// ── 单元测试 ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("aicross-orch-test-{tag}-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn user_preset(id: &str) -> Preset {
        Preset {
            id: id.into(),
            name: format!("预设 {id}"),
            builtin: false,
            stages: vec![Stage {
                nodes: vec![Target {
                    role: "执行".into(),
                    thinking: "中".into(),
                    visibility: "共享".into(),
                    write: "只读".into(),
                    ..blank_target()
                }],
            }],
        }
    }

    fn sample_graph() -> Graph {
        let root = GraphNode {
            id: "root".into(),
            kind: "material".into(),
            parents: Vec::new(),
            user: Message {
                text: "第一句话".into(),
                attachments: vec!["D:/tmp/a.png".into()],
            },
            reply: None,
            target: None,
            created_at: "2026-09-13T10:00:00Z".into(),
        };
        let turn = GraphNode {
            id: "n1".into(),
            kind: "turn".into(),
            parents: vec!["root".into()],
            user: Message {
                text: "干活".into(),
                attachments: Vec::new(),
            },
            reply: Some(Reply {
                status: "draft".into(),
                ..Reply::default()
            }),
            target: Some(Target {
                agent: "claude".into(),
                model: "claude-x".into(),
                role: "执行".into(),
                thinking: "中".into(),
                visibility: "共享".into(),
                write: "只读".into(),
                ..blank_target()
            }),
            created_at: "2026-09-13T10:01:00Z".into(),
        };
        let accept = GraphNode {
            id: "n2".into(),
            kind: "accept".into(),
            parents: vec!["n1".into(), "n1b".into()],
            user: Message::default(),
            reply: Some(Reply {
                status: "draft".into(),
                ..Reply::default()
            }),
            target: Some(Target {
                role: "验收".into(),
                thinking: "中".into(),
                visibility: "盲".into(),
                write: "只读".into(),
                ..blank_target()
            }),
            created_at: "2026-09-13T10:02:00Z".into(),
        };
        let nodes: HashMap<String, GraphNode> = [root, turn, accept]
            .into_iter()
            .map(|n| (n.id.clone(), n))
            .collect();
        Graph {
            version: 1,
            id: "g-1".into(),
            title: "测试图".into(),
            root: "root".into(),
            nodes,
            created_at: "2026-09-13T10:00:00Z".into(),
            updated_at: "2026-09-13T10:02:00Z".into(),
        }
    }

    #[test]
    fn presets_roundtrip() {
        let root = temp_root("presets-rt");
        save_user_presets_in(&root, vec![user_preset("u1"), user_preset("u2")]).unwrap();
        let file = presets_from(&root);
        assert_eq!(file.version, 1);
        // 3 个内置 + 2 个用户
        assert_eq!(file.presets.len(), 5);
        let user: Vec<&Preset> = file.presets.iter().filter(|p| !p.builtin).collect();
        assert_eq!(user.len(), 2);
        assert_eq!(*user[0], user_preset("u1"));
        assert_eq!(*user[1], user_preset("u2"));
    }

    #[test]
    fn presets_missing_file_returns_builtins() {
        let root = temp_root("presets-missing");
        let file = presets_from(&root);
        assert_eq!(file.presets.len(), 3);
        assert!(file.presets.iter().all(|p| p.builtin));
        assert_eq!(file.presets[0].id, "builtin-shared");
        assert_eq!(file.presets[1].id, "builtin-economy");
        assert_eq!(file.presets[2].id, "builtin-blind");
        assert_eq!(file.presets[0].name, "共享模式");
        assert_eq!(file.presets[1].stages.len(), 3);
        assert_eq!(file.presets[2].stages[0].nodes.len(), 3);
    }

    #[test]
    fn presets_broken_file_returns_builtins() {
        let root = temp_root("presets-broken");
        fs::write(root.join("presets.json"), "{ not json").unwrap();
        let file = presets_from(&root);
        assert_eq!(file.presets.len(), 3);
    }

    #[test]
    fn presets_wrong_version_returns_builtins() {
        let root = temp_root("presets-version");
        // version != 1 拒绝整个文件、只回内置，与 graphs 的版本校验对齐
        let file = PresetFile {
            version: 2,
            presets: vec![user_preset("u1")],
        };
        fs::write(
            root.join("presets.json"),
            serde_json::to_string_pretty(&file).unwrap(),
        )
        .unwrap();
        let merged = presets_from(&root);
        assert_eq!(merged.presets.len(), 3);
        assert!(merged.presets.iter().all(|p| p.builtin));
        // version 合法的同内容文件正常合并
        let file = PresetFile {
            version: 1,
            presets: vec![user_preset("u1")],
        };
        fs::write(
            root.join("presets.json"),
            serde_json::to_string_pretty(&file).unwrap(),
        )
        .unwrap();
        let merged = presets_from(&root);
        assert_eq!(merged.presets.len(), 4);
    }

    #[test]
    fn save_strips_builtin_entries() {
        let root = temp_root("presets-strip");
        let mut forged = user_preset("forged");
        forged.builtin = true;
        save_user_presets_in(&root, vec![forged, user_preset("u1")]).unwrap();
        // 磁盘上只剩非内置条目
        let contents = fs::read_to_string(root.join("presets.json")).unwrap();
        let file: PresetFile = serde_json::from_str(&contents).unwrap();
        assert_eq!(file.presets.len(), 1);
        assert_eq!(file.presets[0].id, "u1");
        // 读回来仍是 3 内置 + 1 用户，伪造的内置没进去
        let merged = presets_from(&root);
        assert_eq!(merged.presets.iter().filter(|p| p.builtin).count(), 3);
    }

    #[test]
    fn graph_roundtrip() {
        let root = temp_root("graph-rt");
        let graph = sample_graph();
        save_graph_in(&root, &graph).unwrap();
        let loaded = load_graph_in(&root, "g-1").unwrap().unwrap();
        assert_eq!(loaded, graph);
    }

    #[test]
    fn load_missing_graph_returns_none() {
        let root = temp_root("graph-missing");
        assert_eq!(load_graph_in(&root, "no-such").unwrap(), None);
    }

    #[test]
    fn save_rejects_bad_ids() {
        let root = temp_root("graph-badid");
        for bad in ["../evil", "a/b", "a\\b", "", "a b", "a.b"] {
            let mut graph = sample_graph();
            graph.id = bad.into();
            assert!(save_graph_in(&root, &graph).is_err(), "id {bad:?} 应被拒");
        }
    }

    #[test]
    fn save_rejects_wrong_version() {
        let root = temp_root("graph-version");
        let mut graph = sample_graph();
        graph.version = 2;
        assert!(save_graph_in(&root, &graph).is_err());
    }

    #[test]
    fn list_graphs_skips_broken_files() {
        let root = temp_root("graph-list");
        save_graph_in(&root, &sample_graph()).unwrap();
        fs::write(graphs_dir(&root).join("broken.json"), "{ nope").unwrap();
        let metas = list_graphs_in(&root);
        assert_eq!(metas.len(), 1);
        assert_eq!(
            metas[0],
            GraphMeta {
                id: "g-1".into(),
                title: "测试图".into(),
                updated_at: "2026-09-13T10:02:00Z".into(),
            }
        );
    }

    #[test]
    fn dispatchable_missing_file_reports_error() {
        let root = temp_root("models-missing");
        let result = dispatchable_from(&root);
        assert!(result.models.is_empty());
        assert_eq!(
            result.error.as_deref(),
            Some("未找到 resources.json，尚未接入资源")
        );
    }

    #[test]
    fn dispatchable_broken_file_reports_error() {
        let root = temp_root("models-broken");
        fs::write(root.join("resources.json"), "{ nope").unwrap();
        let result = dispatchable_from(&root);
        assert!(result.models.is_empty());
        assert!(result.error.unwrap().starts_with("resources.json 解析失败"));
    }

    #[test]
    fn dispatchable_filters_connected_only() {
        let root = temp_root("models-filter");
        let store = serde_json::json!({
            "version": 1,
            "agents": [{
                "id": "claude", "name": "Claude", "kind": "cli",
                "detected": true, "version": "1.0",
                "models": [
                    {"id": "m1", "agent": "claude", "status": "已接入"},
                    {"id": "m2", "agent": "claude", "status": "未接入"},
                    {"id": "m3", "agent": "claude", "status": "已接入", "note": "缺字段容忍"}
                ]
            }]
        });
        fs::write(root.join("resources.json"), store.to_string()).unwrap();
        let result = dispatchable_from(&root);
        assert_eq!(result.error, None);
        let ids: Vec<&str> = result.models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["m1", "m3"]);
        // m1/m2 缺了一大堆字段也能解析，缺字段落成空串
        assert_eq!(result.models[0].vendor, "");
    }
}
