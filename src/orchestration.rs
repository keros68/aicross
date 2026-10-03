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

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::resources::{detected as detected_resources, ResourceModel};

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
    /// 节点表用 IndexMap 保持插入序，对齐前端 JS 对象的键序语义：
    /// 找等待中的验收、复用遗留节点、latest_leaf 同深平局都按插入序取第一个。
    #[serde(default)]
    pub nodes: IndexMap<String, GraphNode>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    /// TASK-16：图所属工作区。旧图缺失时保持为空，不改变既有契约。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub project: String,
    /// TASK-16：`console` 表示图由控制台通道创建或继续。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
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
    pub swarm: Option<SwarmItem>,
    /// TASK-12：遗留节点（修正轮到顶后未闭合的条目清单，只读、不可派发）。
    #[serde(default)]
    pub leftover: Option<Leftover>,
    #[serde(default)]
    pub created_at: String,
    /// TASK-16：`console` 表示节点来自控制台通道。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
}

/// 遗留清单（前端 `GraphNode.leftover` 镜像）：来源验收节点与未闭合的条目。
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Leftover {
    #[serde(default)]
    pub source_node_id: String,
    #[serde(default)]
    pub items: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SwarmItem {
    #[serde(default)]
    pub item: u32,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub accept: String,
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
    pub tokens_cache_read: Option<u64>,
    #[serde(default)]
    pub ms: Option<u64>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub runner: Option<String>,
    /// 结果屏「采纳」标记，随图持久化（前端 Reply.adopted）
    #[serde(default)]
    pub adopted: Option<bool>,
    /// 执行节点交给后续共享/盲验收节点的结构化产物。
    #[serde(default)]
    pub artifacts: Option<ReplyArtifacts>,
    /// 失败类型；用于让真身不符等状态在图持久化后仍可区分。
    #[serde(default)]
    pub error_kind: Option<String>,
    /// R5：验证重试到顶后未派出的验收节点标「已跳过」的原因（前端 Reply.skipped_reason）。
    #[serde(default)]
    pub skipped_reason: Option<String>,
    #[serde(default)]
    pub fallback_from: Option<String>,
    #[serde(default)]
    pub fallback_to: Option<String>,
    /// 额度用尽结果的恢复时间；旧图缺失时为空。
    #[serde(default)]
    pub quota_resets_at: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ReplyArtifacts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_node_id: Option<String>,
    #[serde(default)]
    pub diff: String,
    #[serde(default)]
    pub changed_files: Vec<ReplyChangedFile>,
    #[serde(default)]
    pub worktree: Option<String>,
    #[serde(default)]
    pub stdout_summary: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ReplyChangedFile {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub content: String,
}

/// TASK-05B 主备/熔断：主目标失败（error/timeout/identity_mismatch）时的备选。
/// 本期对图契约唯一的字段追加；镜像前端 graph/types.ts 的 FallbackTarget。
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct FallbackTarget {
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Target {
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: String,
    /// 出方案 | 执行 | 验收 | 验证 | 顾问 | 仲裁 | 自定义文本
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_command: Option<String>,
    /// 备选目标；不设则主目标失败即终态
    #[serde(default)]
    pub fallback: Option<FallbackTarget>,
    /// 内置预设盖章时写入节点 user.text；由 Rust 侧冻结材料提供。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub preset_text: String,
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
pub struct DispatchableModels {
    pub models: Vec<ResourceModel>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoleProfile {
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub thinking: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoleSettings {
    pub version: u32,
    #[serde(default)]
    pub planning: RoleProfile,
    #[serde(default)]
    pub execution: RoleProfile,
    #[serde(default)]
    pub acceptance: RoleProfile,
}

impl Default for RoleSettings {
    fn default() -> Self {
        Self {
            version: 1,
            planning: RoleProfile::default(),
            execution: RoleProfile::default(),
            acceptance: RoleProfile::default(),
        }
    }
}

// ── 内置预设（不落盘；builtin=true 不可删不可改） ─────────────────────────

fn blank_target() -> Target {
    Target::default()
}

pub(crate) fn builtin_presets() -> Vec<Preset> {
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
                    write: "单写者".into(),
                    ..blank_target()
                }],
            },
            // TASK-12：验收阶段两路同阶段——验收（盲、只读，看产物给 PASS / NEEDS_FIX，
            // 修正轮只看它）与验证（派盲审前由执行节点跑一次验证命令并附输出），结果并排。
            Stage {
                nodes: vec![
                    Target {
                        role: "验收".into(),
                        thinking: "中".into(),
                        visibility: "盲".into(),
                        write: "只读".into(),
                        ..blank_target()
                    },
                    Target {
                        role: "验证".into(),
                        thinking: "中".into(),
                        visibility: "盲".into(),
                        write: "只读".into(),
                        ..blank_target()
                    },
                ],
            },
        ],
    };
    // 盲审模式（TASK-12）：一段三路——一路验收看产物给 PASS / NEEDS_FIX，两路验证
    // 盲审；派盲审前由执行节点跑一次验证命令并附输出，同阶段并行。
    let blind = Preset {
        id: "builtin-blind".into(),
        name: "盲审模式".into(),
        builtin: true,
        stages: vec![Stage {
            nodes: ["验收", "验证", "验证"]
                .into_iter()
                .map(|role| Target {
                    role: role.into(),
                    thinking: "中".into(),
                    visibility: "盲".into(),
                    write: "只读".into(),
                    ..blank_target()
                })
                .collect(),
        }],
    };
    // TASK-05B 第 2 期预设（图章形态；仲裁/顾问/前提抽查是动作不是图章，见 TASK-05 0.1）。
    // 擂台：同一父节点盖出 3 个共享执行节点，目标由用户在 G1 确认屏逐家选（2–4 家，
    // 删增节点在图界面进行）。结果屏每张卡有「选它」= adopted + 设为后续列末端。
    let arena = Preset {
        id: "builtin-arena".into(),
        name: "擂台模式".into(),
        builtin: true,
        stages: vec![Stage {
            nodes: (0..3)
                .map(|_| Target {
                    role: "执行".into(),
                    thinking: "中".into(),
                    visibility: "共享".into(),
                    write: "只读".into(),
                    ..blank_target()
                })
                .collect(),
        }],
    };
    // 评审团：3 个盲验证节点，视角文本（安全/性能/需求覆盖）由前端 stamp 时
    // 写进各节点 user.text（PRESET_NODE_TEXTS，不加 Target 字段）。
    let jury = Preset {
        id: "builtin-jury".into(),
        name: "评审模式".into(),
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
    let swarm = Preset {
        id: "builtin-swarm".into(),
        name: "蜂群模式".into(),
        builtin: true,
        stages: vec![Stage {
            nodes: vec![Target {
                role: "拆分".into(),
                visibility: "共享".into(),
                write: "只读".into(),
                preset_text: include_str!("../fixtures/prompts/split.txt").trim().into(),
                ..blank_target()
            }],
        }],
    };
    vec![shared, economy, blind, arena, jury, swarm]
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

fn roles_path(root: &Path) -> PathBuf {
    root.join("roles.json")
}

pub(crate) fn roles_from(root: &Path) -> RoleSettings {
    fs::read_to_string(roles_path(root))
        .ok()
        .and_then(|text| serde_json::from_str::<RoleSettings>(&text).ok())
        .filter(|settings| settings.version == 1)
        .unwrap_or_default()
}

/// fix-39-r1 R1-2：命令行的严格读取。`roles_from` 对损坏或版本不认的文件
/// 静默回默认值，`config set-role` 改一个角色后整份写回会把其余角色丢掉；
/// 这里文件存在但解析失败或 version != 1 时返回 Err 让调用方拒写（遇损坏
/// 拒写，避免把其余角色静默丢掉）。文件缺失与 `roles_from` 一致回默认值。
pub(crate) fn roles_from_strict(root: &Path) -> Result<RoleSettings, String> {
    let Ok(text) = fs::read_to_string(roles_path(root)) else {
        return Ok(RoleSettings::default());
    };
    match serde_json::from_str::<RoleSettings>(&text) {
        Ok(roles) if roles.version == 1 => Ok(roles),
        _ => Err("roles.json 无法解析".to_string()),
    }
}

/// 写入 roles.json；命令行（TASK-39 的 `config set-role`）与引擎读同一份。
pub(crate) fn save_roles_in(root: &Path, settings: &RoleSettings) -> Result<(), String> {
    if settings.version != 1 {
        return Err(format!(
            "角色配置 version 只支持 1，收到 {}",
            settings.version
        ));
    }
    write_json_atomic(&roles_path(root), settings)
}

/// 原子写：先写同目录临时文件（flush + sync）再 rename。Windows 上
/// `std::fs::rename` 走 MoveFileExW(MOVEFILE_REPLACE_EXISTING)，可以覆盖
/// 已存在的目标；失败时删目标重试一次只是锁占用等偶发情形的兜底。
/// 做法参考 port/src-tauri-old/src/config.rs 的 write_json_in（我们自己的 fork）。
/// 注意：resources.rs 里有这份函数的整份复制（write_json_atomic），两边
/// 改动要互相同步；收敛成共享 util 留第 5 期。
/// 派发 Markdown/STATE 的同类实现见 `dispatch::write_atomic`；统一收敛留待第 5 期。
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

/// 注意用的是 `detected` 不是 `connected`：节点面板与派发前检查要看到本机探测到的
/// 全部模型（本机 pi 一家三个 provider 就有 31 条），未冒烟的由 `aicross resources`
/// 输出按状态标注。预设角色默认值仍走 `connected`（可派发清单语义），不受影响。
pub(crate) fn dispatchable_from(root: &Path) -> DispatchableModels {
    let path = resources_path(root);
    match detected_resources(&path) {
        Ok(models) => DispatchableModels {
            models,
            error: None,
        },
        Err(_) if !path.exists() => {
            return DispatchableModels {
                models: Vec::new(),
                error: Some("未找到 resources.json".to_string()),
            }
        }
        Err(error) => DispatchableModels {
            models: Vec::new(),
            error: Some(error),
        },
    }
}

/// 内置预设 + presets.json 里的用户预设；文件不存在、解析失败或
/// version != 1 时拒绝该文件、只返回内置（与 graphs 的版本校验对齐）。
/// 角色默认不在这里填：引擎建图盖章时自己套，避免填两遍。
pub(crate) fn raw_presets_from(root: &Path) -> PresetFile {
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

pub(crate) fn save_graph_in(root: &Path, graph: &Graph) -> Result<(), String> {
    if graph.version != 1 {
        return Err(format!("图 version 只支持 1，收到 {}", graph.version));
    }
    validate_graph_id(&graph.id)?;
    write_json_atomic(&graphs_dir(root).join(format!("{}.json", graph.id)), graph)
}

pub(crate) fn load_graph_in(root: &Path, id: &str) -> Result<Option<Graph>, String> {
    validate_graph_id(id)?;
    let path = graphs_dir(root).join(format!("{id}.json"));
    let contents = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("读取 {} 失败：{e}", path.display())),
    };
    let graph: Graph = serde_json::from_str(&contents)
        .map_err(|e| format!("解析 {} 失败：{e}", path.display()))?;
    if graph.version != 1 {
        return Err(format!(
            "图 {} version 只支持 1，收到 {}",
            id, graph.version
        ));
    }
    Ok(Some(graph))
}

// ── 单元测试 ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let dir = crate::paths::test_temp_dir().join(format!("aicross-orch-test-{tag}-{}", Uuid::new_v4()));
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
            swarm: None,
            leftover: None,
            created_at: "2026-09-13T10:00:00Z".into(),
            source: String::new(),
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
                adopted: Some(true),
                skipped_reason: Some("上游验证未通过".into()),
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
            swarm: Some(SwarmItem {
                item: 1,
                files: vec!["src/a/**".into()],
                accept: "cargo test a".into(),
            }),
            leftover: None,
            created_at: "2026-09-13T10:01:00Z".into(),
            source: String::new(),
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
            swarm: None,
            leftover: None,
            created_at: "2026-09-13T10:02:00Z".into(),
            source: String::new(),
        };
        let nodes: IndexMap<String, GraphNode> = [root, turn, accept]
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
            project: String::new(),
            source: String::new(),
        }
    }

    #[test]
    fn presets_missing_file_returns_builtins() {
        let root = temp_root("presets-missing");
        let file = raw_presets_from(&root);
        assert_eq!(file.presets.len(), 6);
        assert!(file.presets.iter().all(|p| p.builtin));
        assert_eq!(file.presets[0].id, "builtin-shared");
        assert_eq!(file.presets[1].id, "builtin-economy");
        assert_eq!(file.presets[2].id, "builtin-blind");
        assert_eq!(file.presets[3].id, "builtin-arena");
        assert_eq!(file.presets[4].id, "builtin-jury");
        assert_eq!(file.presets[5].id, "builtin-swarm");
        assert_eq!(file.presets[0].name, "共享模式");
        assert_eq!(file.presets[1].stages.len(), 3);
        assert_eq!(file.presets[2].stages[0].nodes.len(), 3);
    }

    #[test]
    fn builtin_arena_and_jury_shapes() {
        let presets = builtin_presets();
        let arena = presets.iter().find(|p| p.id == "builtin-arena").unwrap();
        // 擂台：一层 3 个共享执行节点，目标留空由用户在 G1 选 2–4 家
        assert_eq!(arena.stages.len(), 1);
        assert_eq!(arena.stages[0].nodes.len(), 3);
        for target in &arena.stages[0].nodes {
            assert_eq!(target.role, "执行");
            assert_eq!(target.visibility, "共享");
            assert_eq!(target.write, "只读");
            assert!(target.model.is_empty());
        }
        let jury = presets.iter().find(|p| p.id == "builtin-jury").unwrap();
        // 评审团：一层 3 个盲验证节点，视角文本由前端 stamp 时写进 user.text
        assert_eq!(jury.stages.len(), 1);
        assert_eq!(jury.stages[0].nodes.len(), 3);
        for target in &jury.stages[0].nodes {
            assert_eq!(target.role, "验证");
            assert_eq!(target.visibility, "盲");
            assert_eq!(target.write, "只读");
        }
    }

    #[test]
    fn target_fallback_json_roundtrip() {
        // Rust/TS 往返：前端 graph/types.ts 的 fallback 形状（snake_case 契约内
        // 唯一追加字段）必须原样过 Rust serde。
        let json = serde_json::json!({
            "agent": "codex",
            "model": "gpt-5.6-luna",
            "role": "执行",
            "thinking": "中",
            "visibility": "共享",
            "write": "单写者",
            "fallback": { "agent": "pi", "provider": "deepseek", "model": "deepseek-v4-flash" }
        });
        let target: Target = serde_json::from_value(json.clone()).unwrap();
        let fallback = target.fallback.clone().unwrap();
        assert_eq!(fallback.agent, "pi");
        assert_eq!(fallback.provider.as_deref(), Some("deepseek"));
        assert_eq!(fallback.model, "deepseek-v4-flash");
        // 往返：序列化再解析不丢字段（Option 字段序列化为 null，与 TS undefined 等价）
        let restored: Target = serde_json::from_value(serde_json::to_value(&target).unwrap()).unwrap();
        assert_eq!(restored, target);
        assert_eq!(serde_json::to_value(&target).unwrap()["fallback"], json["fallback"]);
        // 不带 fallback 的旧图照常解析
        let mut without = json.clone();
        without.as_object_mut().unwrap().remove("fallback");
        let target: Target = serde_json::from_value(without).unwrap();
        assert_eq!(target.fallback, None);
    }

    #[test]
    fn presets_broken_file_returns_builtins() {
        let root = temp_root("presets-broken");
        fs::write(root.join("presets.json"), "{ not json").unwrap();
        let file = raw_presets_from(&root);
        assert_eq!(file.presets.len(), 6);
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
        let merged = raw_presets_from(&root);
        assert_eq!(merged.presets.len(), 6);
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
        let merged = raw_presets_from(&root);
        assert_eq!(merged.presets.len(), 7);
    }

    #[test]
    fn roles_roundtrip() {
        let root = temp_root("roles");
        let roles = RoleSettings {
            version: 1,
            planning: RoleProfile {
                agent: "pi".into(),
                provider: "zai-coding-cn".into(),
                model: "glm-5.3-flash".into(),
                tier: "高".into(),
                thinking: "高".into(),
            },
            execution: RoleProfile {
                agent: "codex".into(),
                provider: String::new(),
                model: "gpt-5.6-luna".into(),
                tier: "低".into(),
                thinking: "低".into(),
            },
            acceptance: RoleProfile {
                agent: "kimi".into(),
                provider: "kimi".into(),
                model: "kimi-code/k3-256k".into(),
                tier: "中".into(),
                thinking: "中".into(),
            },
        };
        save_roles_in(&root, &roles).unwrap();
        assert_eq!(roles_from(&root), roles);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn task36_graph_roundtrip_preserves_validation_and_source_node_fields() {
        let root = temp_root("task36-roundtrip");
        let mut graph = sample_graph();
        let node = graph.nodes.get_mut(&graph.root).unwrap();
        node.target = Some(Target {
            validation_command: Some("cargo test".into()),
            ..Target::default()
        });
        node.reply = Some(Reply {
            artifacts: Some(ReplyArtifacts {
                source_node_id: Some("source-1".into()),
                ..ReplyArtifacts::default()
            }),
            ..Reply::default()
        });
        save_graph_in(&root, &graph).unwrap();
        let loaded = load_graph_in(&root, &graph.id).unwrap().unwrap();
        let loaded_node = loaded.nodes.get(&loaded.root).unwrap();
        assert_eq!(loaded_node.target.as_ref().unwrap().validation_command.as_deref(), Some("cargo test"));
        assert_eq!(loaded_node.reply.as_ref().unwrap().artifacts.as_ref().unwrap().source_node_id.as_deref(), Some("source-1"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn graph_roundtrip() {
        let root = temp_root("graph-rt");
        let graph = sample_graph();
        save_graph_in(&root, &graph).unwrap();
        let loaded = load_graph_in(&root, "g-1").unwrap().unwrap();
        assert_eq!(loaded, graph);
        // R5：skipped_reason 是前端追加的可选字段，Rust 镜像必须原样保住。
        let kept = loaded.nodes.values().find_map(|n| n.reply.as_ref().and_then(|r| r.skipped_reason.clone()));
        assert_eq!(kept.as_deref(), Some("上游验证未通过"));
    }

    #[test]
    fn task08a_graph_node_swarm_roundtrips_and_old_graph_defaults_to_none() {
        let graph = sample_graph();
        let restored: Graph = serde_json::from_value(serde_json::to_value(&graph).unwrap()).unwrap();
        assert_eq!(restored.nodes["n1"].swarm, Some(SwarmItem {
            item: 1,
            files: vec!["src/a/**".into()],
            accept: "cargo test a".into(),
        }));
        let legacy: GraphNode = serde_json::from_str(r#"{"id":"old","kind":"turn"}"#).unwrap();
        assert_eq!(legacy.swarm, None);
    }

    #[test]
    fn task06_reply_fallback_fields_roundtrip_and_default_for_old_graphs() {
        let reply = Reply { fallback_from: Some("claude/main".into()), fallback_to: Some("codex/backup".into()), ..Reply::default() };
        let restored: Reply = serde_json::from_value(serde_json::to_value(&reply).unwrap()).unwrap();
        assert_eq!(restored.fallback_from.as_deref(), Some("claude/main"));
        assert_eq!(restored.fallback_to.as_deref(), Some("codex/backup"));
        let legacy: Reply = serde_json::from_str(r#"{"status":"done","text":"old"}"#).unwrap();
        assert_eq!(legacy.fallback_from, None);
        assert_eq!(legacy.fallback_to, None);
    }

    #[test]
    fn task14_r1_reply_quota_reset_roundtrips_and_defaults_for_old_graphs() {
        let reply = Reply { quota_resets_at: Some("2099-01-01T00:00:00+08:00".into()), ..Reply::default() };
        let restored: Reply = serde_json::from_value(serde_json::to_value(&reply).unwrap()).unwrap();
        assert_eq!(restored.quota_resets_at, reply.quota_resets_at);
        let legacy: Reply = serde_json::from_str(r#"{"status":"failed","text":"old"}"#).unwrap();
        assert_eq!(legacy.quota_resets_at, None);
    }

    #[test]
    fn task07_reply_cache_read_roundtrips_and_defaults_for_old_graphs() {
        let reply = Reply { tokens_cache_read: Some(42), ..Reply::default() };
        let restored: Reply = serde_json::from_value(serde_json::to_value(&reply).unwrap()).unwrap();
        assert_eq!(restored.tokens_cache_read, Some(42));
        let legacy: Reply = serde_json::from_str(r#"{"status":"done","text":"old"}"#).unwrap();
        assert_eq!(legacy.tokens_cache_read, None);
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
    fn dispatchable_missing_file_reports_error() {
        let root = temp_root("models-missing");
        let result = dispatchable_from(&root);
        assert!(result.models.is_empty());
        assert_eq!(
            result.error.as_deref(),
            Some("未找到 resources.json")
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
    fn dispatchable_lists_all_detected_with_connected_first() {
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
            }, {
                "id": "pi", "name": "pi", "kind": "cli",
                "detected": true, "version": "1.0",
                "models": [
                    {"id": "p-new", "agent": "pi", "provider": "zai", "status": "待冒烟"},
                    {"id": "p-bad", "agent": "pi", "provider": "zai", "status": "冒烟失败"},
                    {"id": "p-ok", "agent": "pi", "provider": "zai", "status": "已接入"}
                ]
            }]
        });
        fs::write(root.join("resources.json"), store.to_string()).unwrap();
        let result = dispatchable_from(&root);
        assert_eq!(result.error, None);
        // 08b 起派发清单列出全部探测到的模型；09-15 起排序按 verification：
        // 原生（claude 三条不论旧 status）与已验证在前，未验证次之，验证失败垫底。
        let ids: Vec<&str> = result.models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["m1", "m2", "m3", "p-ok", "p-new", "p-bad"]);
        assert_eq!(result.models[1].status, "未接入");
        assert_eq!(result.models[1].verification, "native");
        assert_eq!(result.models[4].verification, "unverified");
        // m1/m2 缺了一大堆字段也能解析，缺字段落成空串
        assert_eq!(result.models[0].vendor, "");
    }

    // ── TASK-12 ──────────────────────────────────────────────────────────

    #[test]
    fn task12_economy_acceptance_stage_has_acceptance_and_verification_routes() {
        let presets = builtin_presets();
        let economy = presets.iter().find(|p| p.id == "builtin-economy").unwrap();
        assert_eq!(economy.stages.len(), 3);
        let roles: Vec<&str> = economy.stages[2].nodes.iter().map(|t| t.role.as_str()).collect();
        assert_eq!(roles, ["验收", "验证"]);
        for target in &economy.stages[2].nodes {
            assert_eq!((target.visibility.as_str(), target.write.as_str()), ("盲", "只读"));
        }
    }

    #[test]
    fn task12_blind_preset_has_one_acceptance_and_two_verification_routes() {
        let presets = builtin_presets();
        let blind = presets.iter().find(|p| p.id == "builtin-blind").unwrap();
        assert_eq!(blind.stages.len(), 1);
        let roles: Vec<&str> = blind.stages[0].nodes.iter().map(|t| t.role.as_str()).collect();
        assert_eq!(roles, ["验收", "验证", "验证"]);
        for target in &blind.stages[0].nodes {
            assert_eq!((target.visibility.as_str(), target.write.as_str()), ("盲", "只读"));
            assert!(target.model.is_empty());
        }
    }

    #[test]
    fn task12_graph_node_leftover_roundtrip_and_default_none() {
        let json = serde_json::json!({
            "id": "l1",
            "kind": "turn",
            "parents": ["a2"],
            "user": { "text": "遗留：2 条未闭合，来源 验收", "attachments": [] },
            "leftover": { "source_node_id": "a2", "items": ["- 测试未覆盖", "- 文案未改"] },
            "created_at": "2026-09-15T00:00:00Z"
        });
        let node: GraphNode = serde_json::from_value(json).unwrap();
        let leftover = node.leftover.as_ref().unwrap();
        assert_eq!(leftover.source_node_id, "a2");
        assert_eq!(leftover.items, ["- 测试未覆盖", "- 文案未改"]);
        let back = serde_json::to_value(&node).unwrap();
        assert_eq!(back["leftover"]["items"][1], "- 文案未改");
        let plain: GraphNode = serde_json::from_value(serde_json::json!({ "id": "x", "kind": "turn" })).unwrap();
        assert!(plain.leftover.is_none());
        assert_eq!(serde_json::to_value(&plain).unwrap()["leftover"], serde_json::Value::Null);
    }

}
