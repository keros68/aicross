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
    /// TASK-55：模式介绍，一句话；空串不写出。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
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
        description: "单个模型执行，可读取完整对话上下文；只读".into(),
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
        description: "高档模型出方案，低档模型执行，其他厂商的模型验收".into(),
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
        description: "同一份材料由多个厂商的模型独立审查，结果互不可见".into(),
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
        description: "多个模型分别完成同一任务，从结果中选定一份采纳".into(),
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
        description: "多个模型分别从安全、性能、需求覆盖角度审查".into(),
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
        description: "任务拆分为互不重叠的子任务，由多个模型并行完成后合并".into(),
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
    // fix-46-r1 R2：只有文件缺失回默认值；存在但读不了（非 UTF-8、无权限）
    // 同样报错拒写，不能当缺失覆盖掉已有人选。
    let text = match fs::read_to_string(roles_path(root)) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(RoleSettings::default());
        }
        Err(error) => return Err(format!("roles.json 读不了：{error}")),
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

// ── TASK-46：modes.json（每个模式的位置人选与默认模式） ───────────────────

/// `thinking` 只认的四个级别；空串（沿用预设默认值）不在列，由调用方区分。
pub(crate) const THINKING_LEVELS: [&str; 4] = ["关", "低", "中", "高"];

/// 人选来源（TASK-50）：`manual`（窗口或命令行手选）/ `recommended`
/// （一键推荐填的）。旧文件没有该字段时按 `manual` 处理：只有值恰为
/// `recommended` 才算推荐填的，其余（含空）都是手选。
pub(crate) const SEAT_ORIGIN_MANUAL: &str = "manual";
pub(crate) const SEAT_ORIGIN_RECOMMENDED: &str = "recommended";

/// 一个位置的人选。`tier` / `thinking` 空串表示沿用预设的默认值；
/// `thinking` 只认 关 / 低 / 中 / 高 / 空串；`fallback` 可缺省，有值时
/// agent 与 model 必填。字段名 camelCase 是配置文件契约。
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SeatChoice {
    #[serde(default)]
    pub agent: String,
    pub provider: Option<String>,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub thinking: String,
    #[serde(default)]
    pub fallback: Option<FallbackTarget>,
    /// 人选来源；旧文件缺该字段时为空串，按 `manual` 处理（见上面常量说明）。
    #[serde(default)]
    pub origin: String,
}

impl SeatChoice {
    /// 该人选是不是一键推荐填的（空串与其它值都按手选）。
    pub(crate) fn is_recommended(&self) -> bool {
        self.origin == SEAT_ORIGIN_RECOMMENDED
    }
}

/// 一个自定义短命令的定义（modes.json v2 的 `hosts.<id>.slashCommands`，
/// TASK-53）。`name` 只允许小写字母、数字与 `-`，1–32 位；`mode` 是预设 id
/// （别名解析同 `--preset`）；`note` 为可选的固定附加说明（单行，≤200 字），
/// 原样写进命令正文。缺省为空数组。
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SlashCommand {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub note: String,
}

/// 一个主 agent 的模式配置（modes.json v2 的 `hosts.<id>`，TASK-50）。
/// `vendor_override` 为空表示不覆盖推定结果。
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HostModes {
    #[serde(default)]
    pub vendor_override: String,
    #[serde(default)]
    pub default_mode: String,
    #[serde(default)]
    pub seats: IndexMap<String, Vec<Vec<Option<SeatChoice>>>>,
    /// 自定义短命令（TASK-53）；旧文件没有该字段时按空数组处理。
    #[serde(default)]
    pub slash_commands: Vec<SlashCommand>,
    /// TASK-56：内置模式的位置数量调整，键为内置模式 id，值为可调角色的
    /// 位置数（范围见 [`count_spec`]）。缺省为空；没有条目或值越界时该
    /// 模式用默认形状。用户自定义预设不走这里。
    #[serde(default)]
    pub counts: IndexMap<String, u32>,
    /// TASK-59：评审模式各位置的视角，键为模式 id（目前只认
    /// `builtin-jury`，命令行对其它模式退出 2），值按扁平位置序存视角
    /// 文字；某项缺失或为空串时该位置用默认表（安全 → 性能 → 需求覆盖）。
    /// 位置数量调小时多出的项不删，调大后继续生效。
    #[serde(default)]
    pub focus: IndexMap<String, Vec<String>>,
}

/// modes.json（v2）：按主 agent 分组的默认模式与位置人选。`seats` 的下标
/// 与该预设 `stages[i].nodes[j]` 一一对应，`None` 为未设人选；内置预设与
/// 用户预设都可以有。与 roles.json 同目录（`AICROSS_HOME` 生效）。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModesFile {
    pub version: u32,
    #[serde(default)]
    pub hosts: IndexMap<String, HostModes>,
}

impl Default for ModesFile {
    fn default() -> Self {
        Self {
            version: 2,
            hosts: IndexMap::new(),
        }
    }
}

fn modes_path(root: &Path) -> PathBuf {
    root.join("modes.json")
}

/// 宽松读取：派发与展示用。文件缺失、损坏或 version 不认都按空配置处理
///（与 `roles_from` 同一分法）。version 1 是未发布过的旧格式，不做迁移，
/// 同样按空配置处理（TASK-50）。
pub(crate) fn modes_from(root: &Path) -> ModesFile {
    fs::read_to_string(modes_path(root))
        .ok()
        .and_then(|text| serde_json::from_str::<ModesFile>(&text).ok())
        .filter(|modes| modes.version == MODES_VERSION)
        .unwrap_or_default()
}

/// modes.json 现行版本。v1 从未发布过，不做迁移：宽松读取当空配置，
/// 严格读取报错拒写。
pub(crate) const MODES_VERSION: u32 = 2;

/// 严格读取：命令行写入用。文件缺失回默认值；损坏或 version 不认时报错，
/// 调用方拒写（与 `roles_from_strict` 同一分法，避免把已有人选静默丢掉）。
/// version 1 是旧格式，报「是旧格式」，提示删除后重新设置。
pub(crate) fn modes_from_strict(root: &Path) -> Result<ModesFile, String> {
    // fix-46-r1 R2：与 `roles_from_strict` 同一处理——只有文件缺失回默认值，
    // 存在但读不了（非 UTF-8、无权限）报错拒写。
    let text = match fs::read_to_string(modes_path(root)) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ModesFile::default());
        }
        Err(error) => return Err(format!("modes.json 读不了：{error}")),
    };
    match serde_json::from_str::<ModesFile>(&text) {
        Ok(modes) if modes.version == MODES_VERSION => Ok(modes),
        Ok(_) if modes_file_version(&text) == 1 => Err(
            "modes.json 是旧格式，删除后重新设置".to_string(),
        ),
        _ => Err("modes.json 无法解析".to_string()),
    }
}

/// 只探 version 字段，区分 v1 与其它认不得的格式（错误提示不同）。
fn modes_file_version(text: &str) -> u32 {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|value| value.get("version").and_then(serde_json::Value::as_u64))
        .unwrap_or(0) as u32
}

/// 写入 modes.json；命令行与引擎读同一份。
pub(crate) fn save_modes_in(root: &Path, modes: &ModesFile) -> Result<(), String> {
    if modes.version != MODES_VERSION {
        return Err(format!(
            "模式配置 version 只支持 {MODES_VERSION}，收到 {}",
            modes.version
        ));
    }
    write_json_atomic(&modes_path(root), modes)
}

/// 某个主 agent 的配置；没有该主 agent 的条目时给空配置（不落盘）。
pub(crate) fn host_section<'a>(modes: &'a ModesFile, host: &str) -> Option<&'a HostModes> {
    modes.hosts.get(host)
}

/// 写入前的可变访问：没有该主 agent 的条目时补一个空的（不单独落盘，
/// 由调用方改完后一并写）。
pub(crate) fn host_section_mut<'a>(modes: &'a mut ModesFile, host: &str) -> &'a mut HostModes {
    modes.hosts.entry(host.to_string()).or_default()
}

/// 写盘前把该模式的 seats 数组整理成预设的形状：阶段数与每阶段位置数都
/// 与预设一致，缺的补 null，预设改短后多出的旧人选清掉，免得预设以后
/// 再加长时旧人选对上新节点。set-seat 与一键推荐共用（fix-46-r1 R7）。
pub(crate) fn shape_seats_to_preset(
    stages: &mut Vec<Vec<Option<SeatChoice>>>,
    preset: &Preset,
) {
    stages.resize(preset.stages.len(), Vec::new());
    for (row, stage) in stages.iter_mut().zip(preset.stages.iter()) {
        row.resize(stage.nodes.len(), None);
    }
}

// ── TASK-56：内置模式的位置数量调整 ───────────────────────────────────

/// 一个内置模式可调的位置数量：`stage` 是阶段下标（0 起），该阶段里角色
/// 等于 `role` 的节点数量可调，其余节点固定不动。共享与蜂群不可调；
/// 用户自定义预设不走这里。
#[derive(Clone, Copy, Debug)]
pub(crate) struct CountSpec {
    pub preset_id: &'static str,
    pub role: &'static str,
    pub stage: usize,
    pub min: usize,
    pub max: usize,
    pub default: usize,
}

/// 可调表：数量减少从末尾去掉，增加复制该角色的模板节点。盲审另有
/// 1 个「验收」固定；经济第 3 阶段「验收」固定、可调到 0（只剩「验收」）。
pub(crate) const COUNT_SPECS: [CountSpec; 4] = [
    CountSpec { preset_id: "builtin-economy", role: "验证", stage: 2, min: 0, max: 3, default: 1 },
    CountSpec { preset_id: "builtin-blind", role: "验证", stage: 0, min: 1, max: 4, default: 2 },
    CountSpec { preset_id: "builtin-arena", role: "执行", stage: 0, min: 2, max: 4, default: 3 },
    CountSpec { preset_id: "builtin-jury", role: "验证", stage: 0, min: 1, max: 3, default: 3 },
];

/// 某个内置模式的位置数量是否可调；不可调返回 None。
pub(crate) fn count_spec(preset_id: &str) -> Option<&'static CountSpec> {
    COUNT_SPECS
        .iter()
        .find(|spec| spec.preset_id == preset_id)
}

/// 按设定值重排可调角色的位置：底版与模板节点都从 `builtin_presets()` 的
/// 原始预设取（传入的可能是已按别的数量整过形的副本，不能当模板）；在
/// 原始形状里把该角色的整段节点原位换成 `count` 个模板副本——内置模式的
/// 可调节点都连成一段靠在阶段末尾，等价于「多则从末尾去掉、少则在该角色
/// 最后一个节点之后追加」，阶段内顺序不打乱。不可调的预设原样返回；
/// `count` 越出范围时按范围夹取。派发、`config show`、补人、推荐都经
/// [`shaped_preset`] 取预设形状，不各写一份。
pub(crate) fn shape_builtin_preset(preset: &Preset, count: usize) -> Preset {
    let Some(spec) = count_spec(&preset.id) else {
        return preset.clone();
    };
    let original = builtin_presets()
        .into_iter()
        .find(|builtin| builtin.id == preset.id);
    let Some(original) = original else {
        return preset.clone();
    };
    let mut shaped = original;
    let count = count.clamp(spec.min, spec.max);
    let Some(stage) = shaped.stages.get_mut(spec.stage) else {
        return shaped;
    };
    let Some(template) = stage
        .nodes
        .iter()
        .find(|node| node.role == spec.role)
        .cloned()
    else {
        return shaped;
    };
    let mut nodes: Vec<Target> = Vec::with_capacity(stage.nodes.len());
    let mut emitted = false;
    for node in stage.nodes.drain(..) {
        if node.role == spec.role {
            // 该角色整段只出现一次：第一处换成 count 个副本，其余略过。
            if !emitted {
                nodes.extend(std::iter::repeat_n(template.clone(), count));
                emitted = true;
            }
        } else {
            nodes.push(node);
        }
    }
    stage.nodes = nodes;
    shaped
}

/// 该主 agent 配置下的实际形状：可调模式查 `counts`（缺省或越界用默认
/// 值），其余预设原样返回。
pub(crate) fn shaped_preset(preset: &Preset, section: Option<&HostModes>) -> Preset {
    let Some(spec) = count_spec(&preset.id) else {
        return preset.clone();
    };
    let count = section
        .and_then(|section| section.counts.get(preset.id.as_str()))
        .copied()
        .map(|value| value as usize)
        .filter(|value| (spec.min..=spec.max).contains(value))
        .unwrap_or(spec.default);
    if count == spec.default {
        preset.clone()
    } else {
        shape_builtin_preset(preset, count)
    }
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

/// 严格读取 presets.json（TASK-55）：`preset save` / `preset remove` 的读
/// 改写用。文件缺失回空清单；存在但读不了（非 UTF-8、无权限）、解析失败
/// 或 version != 1 时报错拒写，避免覆盖丢掉已有用户预设（与 modes.json 的
/// 严格读取同一分法）。
pub(crate) fn presets_from_strict(root: &Path) -> Result<PresetFile, String> {
    let text = match fs::read_to_string(presets_path(root)) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(PresetFile {
                version: 1,
                presets: Vec::new(),
            });
        }
        Err(error) => return Err(format!("presets.json 读不了：{error}")),
    };
    match serde_json::from_str::<PresetFile>(&text) {
        Ok(file) if file.version == 1 => Ok(file),
        _ => Err("presets.json 无法解析".to_string()),
    }
}

/// 写入 presets.json（TASK-55）：`preset save` 与 `preset remove` 用。
pub(crate) fn save_presets_in(root: &Path, file: &PresetFile) -> Result<(), String> {
    if file.version != 1 {
        return Err(format!(
            "预设文件 version 只支持 1，收到 {}",
            file.version
        ));
    }
    write_json_atomic(&presets_path(root), file)
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
            description: String::new(),
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

    // ── TASK-46：modes.json 的读写 ─────────────────────────────────────

    fn seat(agent: &str, provider: Option<&str>, model: &str) -> SeatChoice {
        SeatChoice {
            agent: agent.into(),
            provider: provider.map(str::to_string),
            model: model.into(),
            ..SeatChoice::default()
        }
    }

    #[test]
    fn task46_modes_roundtrip_keeps_camel_case_and_seat_order() {
        let root = temp_root("modes-roundtrip");
        let modes = ModesFile {
            version: MODES_VERSION,
            hosts: IndexMap::from([(
                "claude".into(),
                HostModes {
                    default_mode: "builtin-blind".into(),
                    seats: IndexMap::from([(
                        "builtin-blind".into(),
                        vec![vec![
                            Some(SeatChoice {
                                fallback: Some(FallbackTarget {
                                    agent: "claude".into(),
                                    provider: None,
                                    model: "sonnet".into(),
                                }),
                                ..seat("pi", Some("zai-coding-cn"), "glm-5.3-flash")
                            }),
                            None,
                            Some(seat("codex", None, "gpt-5.6-luna")),
                        ]],
                    )]),
                    ..HostModes::default()
                },
            )]),
        };
        save_modes_in(&root, &modes).unwrap();
        assert_eq!(modes_from(&root), modes);
        let text = fs::read_to_string(modes_path(&root)).unwrap();
        assert!(text.contains("\"defaultMode\": \"builtin-blind\""), "{text}");
        assert!(text.contains("\"vendorOverride\": \"\""), "{text}");
        assert!(text.contains("\"fallback\": {"), "{text}");
    }

    #[test]
    fn task46_modes_lenient_read_defaults_on_missing_broken_or_wrong_version() {
        let root = temp_root("modes-lenient");
        // 缺失
        assert_eq!(modes_from(&root), ModesFile::default());
        // 损坏
        fs::write(modes_path(&root), "{ 损坏").unwrap();
        assert_eq!(modes_from(&root), ModesFile::default());
        // version 不认
        fs::write(modes_path(&root), r#"{"version":9,"hosts":{}}"#).unwrap();
        assert_eq!(modes_from(&root), ModesFile::default());
    }

    /// TASK-50：v1 旧格式从未发布过，不做迁移：宽松读当空配置，严格读
    /// 报「是旧格式」拒写。
    #[test]
    fn task50_modes_v1_lenient_read_is_empty_and_strict_read_rejects() {
        let root = temp_root("modes-v1");
        let v1 = r#"{"version":1,"defaultMode":"builtin-blind","seats":{"builtin-blind":[[{"agent":"pi","model":"glm-4.7"}]]}}"#;
        fs::write(modes_path(&root), v1).unwrap();
        assert_eq!(modes_from(&root), ModesFile::default());
        assert_eq!(
            modes_from_strict(&root),
            Err("modes.json 是旧格式，删除后重新设置".into())
        );
        // 严格读取失败后写盘命令不会碰文件：内容原样。
        assert_eq!(fs::read_to_string(modes_path(&root)).unwrap(), v1);
    }

    #[test]
    fn task46_modes_strict_read_rejects_broken_and_wrong_version() {
        let root = temp_root("modes-strict");
        // 缺失回默认值（与宽松读取一致）
        assert_eq!(modes_from_strict(&root), Ok(ModesFile::default()));
        // 损坏拒写
        fs::write(modes_path(&root), "{ 损坏").unwrap();
        assert_eq!(modes_from_strict(&root), Err("modes.json 无法解析".into()));
        // version 不认拒写
        fs::write(modes_path(&root), r#"{"version":9}"#).unwrap();
        assert_eq!(modes_from_strict(&root), Err("modes.json 无法解析".into()));
        // save 校验 version
        let mut modes = ModesFile::default();
        modes.version = 1;
        assert!(save_modes_in(&root, &modes).is_err());
    }

    /// fix-46-r1 R2：文件存在但不是 UTF-8 时严格读取报错（不回默认值），
    /// 缺失仍然回默认值。
    #[test]
    fn task46_r1_strict_read_rejects_non_utf8_files() {
        let root = temp_root("modes-strict-non-utf8");
        fs::write(modes_path(&root), [0xff, 0xfe, b'{']).unwrap();
        let error = modes_from_strict(&root).unwrap_err();
        assert!(error.starts_with("modes.json 读不了："), "{error}");
        assert_eq!(modes_from(&root), ModesFile::default(), "宽松读仍按空配置");

        fs::write(roles_path(&root), [0xff, 0xfe, b'{']).unwrap();
        let error = roles_from_strict(&root).unwrap_err();
        assert!(error.starts_with("roles.json 读不了："), "{error}");
        assert_eq!(roles_from(&root), RoleSettings::default(), "宽松读仍按默认值");
    }

    // ── TASK-56：位置数量可调 ─────────────────────────────────────────

    fn preset_by(id: &str) -> Preset {
        builtin_presets()
            .into_iter()
            .find(|preset| preset.id == id)
            .unwrap()
    }

    fn roles_of(preset: &Preset, stage: usize) -> Vec<String> {
        preset.stages[stage].nodes.iter().map(|node| node.role.clone()).collect()
    }

    /// 可调表：四个可调模式的角色、阶段、范围与默认；共享、蜂群不可调。
    #[test]
    fn task56_count_specs_list_adjustable_modes() {
        let spec_of = |id| count_spec(id).map(|spec| (spec.role, spec.stage, spec.min, spec.max, spec.default));
        assert_eq!(spec_of("builtin-economy"), Some(("验证", 2, 0, 3, 1)));
        assert_eq!(spec_of("builtin-blind"), Some(("验证", 0, 1, 4, 2)));
        assert_eq!(spec_of("builtin-arena"), Some(("执行", 0, 2, 4, 3)));
        assert_eq!(spec_of("builtin-jury"), Some(("验证", 0, 1, 3, 3)));
        assert_eq!(spec_of("builtin-shared"), None, "共享不可调");
        assert_eq!(spec_of("builtin-swarm"), None, "蜂群不可调");
        // 默认值与内置预设现形状一致。
        for spec in COUNT_SPECS.iter() {
            let preset = preset_by(spec.preset_id);
            let count = preset.stages[spec.stage]
                .nodes
                .iter()
                .filter(|node| node.role == spec.role)
                .count();
            assert_eq!(count, spec.default, "{} 的默认位置数", spec.preset_id);
            assert!((spec.min..=spec.max).contains(&spec.default));
        }
    }

    /// 增减后的阶段形状：增加复制该角色的模板节点，减少从末尾去掉；
    /// 固定节点（盲审 / 经济的「验收」）保持在可调节点之前。
    #[test]
    fn task56_shape_builtin_preset_grows_and_shrinks() {
        // 盲审：1–4 个验证，验收固定在最前。
        let blind = shape_builtin_preset(&preset_by("builtin-blind"), 4);
        assert_eq!(roles_of(&blind, 0), ["验收", "验证", "验证", "验证", "验证"]);
        let blind = shape_builtin_preset(&preset_by("builtin-blind"), 1);
        assert_eq!(roles_of(&blind, 0), ["验收", "验证"]);
        // 经济：第 3 阶段 0–3 个验证，0 时只剩验收。
        let economy = shape_builtin_preset(&preset_by("builtin-economy"), 3);
        assert_eq!(roles_of(&economy, 2), ["验收", "验证", "验证", "验证"]);
        let economy = shape_builtin_preset(&preset_by("builtin-economy"), 0);
        assert_eq!(roles_of(&economy, 2), ["验收"]);
        assert_eq!(economy.stages.len(), 3, "其余阶段不动");
        assert_eq!(roles_of(&economy, 0), ["出方案"]);
        // 擂台 2–4、评审 1–3。
        let arena = shape_builtin_preset(&preset_by("builtin-arena"), 2);
        assert_eq!(roles_of(&arena, 0), ["执行", "执行"]);
        let jury = shape_builtin_preset(&preset_by("builtin-jury"), 3);
        assert_eq!(roles_of(&jury, 0), ["验证", "验证", "验证"]);
        // 新增节点复制模板：角色、盲属性与思考强度与原有节点一致。
        let blind = shape_builtin_preset(&preset_by("builtin-blind"), 4);
        let first = &blind.stages[0].nodes[1];
        let added = &blind.stages[0].nodes[4];
        assert_eq!(
            (added.role.as_str(), added.visibility.as_str(), added.write.as_str(), added.thinking.as_str()),
            (first.role.as_str(), first.visibility.as_str(), first.write.as_str(), first.thinking.as_str())
        );
        // 不可调的预设原样。
        let shared = preset_by("builtin-shared");
        assert_eq!(shape_builtin_preset(&shared, 5), shared);
        // 越界夹取。
        let blind = shape_builtin_preset(&preset_by("builtin-blind"), 99);
        assert_eq!(blind.stages[0].nodes.len(), 5);
    }

    /// 模板与底版固定从原始内置预设取：把已整过形的副本再整形，结果与
    /// 从原始预设直接整形一致（模板不丢、阶段内顺序不变）。
    #[test]
    fn task56_shape_builtin_preset_is_idempotent_from_any_input() {
        let blind = preset_by("builtin-blind");
        let grown = shape_builtin_preset(&blind, 4);
        assert_eq!(shape_builtin_preset(&grown, 1), shape_builtin_preset(&blind, 1));
        assert_eq!(shape_builtin_preset(&grown, 4), grown);
        let economy = preset_by("builtin-economy");
        let trimmed = shape_builtin_preset(&economy, 0);
        assert_eq!(shape_builtin_preset(&trimmed, 0), trimmed);
        assert_eq!(
            shape_builtin_preset(&trimmed, 2),
            shape_builtin_preset(&economy, 2)
        );
        let jury = preset_by("builtin-jury");
        assert_eq!(
            shape_builtin_preset(&shape_builtin_preset(&jury, 1), 3),
            shape_builtin_preset(&jury, 3)
        );
    }

    /// `shaped_preset`：按主 agent 的 counts 取形状；缺省用默认、越界也用
    /// 默认；不可调模式原样。
    #[test]
    fn task56_shaped_preset_reads_counts_and_falls_back_to_default() {
        let section = |counts: IndexMap<String, u32>| HostModes {
            counts,
            ..HostModes::default()
        };
        let blind = preset_by("builtin-blind");
        // 设定值生效。
        let shaped = shaped_preset(&blind, Some(&section(IndexMap::from([("builtin-blind".into(), 4)]))));
        assert_eq!(roles_of(&shaped, 0), ["验收", "验证", "验证", "验证", "验证"]);
        // 越界回默认；缺省同为默认。
        let shaped = shaped_preset(&blind, Some(&section(IndexMap::from([("builtin-blind".into(), 9)]))));
        assert_eq!(shaped, blind);
        assert_eq!(shaped_preset(&blind, None), blind);
        // 其它模式的 counts 不影响这一个。
        let shaped = shaped_preset(&blind, Some(&section(IndexMap::from([("builtin-arena".into(), 2)]))));
        assert_eq!(shaped, blind);
        // 不可调模式带 counts 也原样。
        let shared = preset_by("builtin-shared");
        assert_eq!(
            shaped_preset(&shared, Some(&section(IndexMap::from([("builtin-shared".into(), 3)])))),
            shared
        );
    }

}
