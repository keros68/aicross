//! 一键推荐（TASK-50 第 3 节）。
//!
//! 只填空位置与来源为 recommended 的位置（`--all` 全部重填），manual 不动。
//! 候选池只有资源状态为「可用」（原生）或「已验证」的模型；档位与计费查
//! `src/data/model-tiers.json`（随版本发布的小表），表里没有的档位按
//! 「未知」排最后。规则沿用 skill `inventory.py` 选审查者的思路：出方案、
//! 拆分优先高档；执行优先中低档；验收、验证避开主 agent 厂商、同阶段互不
//! 相同、尽量不同于执行厂商；执行与规划位在同一阶段内分散（同一模型不
//! 重复、优先没出现过的厂商，软条件，走查 #6）；凑不出就留空并写 notes，
//! 不降级硬填。同输入同输出：全部按资源库顺序稳定排序，不引入随机。

use crate::orchestration::{self, ModesFile, Preset, RoleSettings, SeatChoice, Target};
use crate::resources::{self, ResourceStore};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// 内置档位表：编译期嵌入，随版本更新（同别名表的做法）。
const MODEL_TIERS_TEXT: &str = include_str!("data/model-tiers.json");

const TIER_HIGH: &str = "高";
const TIER_MID: &str = "中";
const TIER_LOW: &str = "低";
const TIER_UNKNOWN: &str = "未知";
const BILLING_SUBSCRIPTION: &str = "订阅";
const BILLING_PER_CALL: &str = "按次";
const BILLING_UNKNOWN: &str = "未知";

/// 候选池里的一条模型。
#[derive(Clone, Debug, PartialEq)]
struct Candidate {
    agent: String,
    provider: Option<String>,
    model: String,
    vendor: String,
    tier: String,
    billing: String,
    /// true = 第三方且已验证；false = 原生可用。
    verified: bool,
}

/// 档位表解析结果；文件损坏时按空表处理（全部「未知」）。
struct TierTable {
    tiers: Vec<TierRule>,
    billing: Vec<TierRule>,
}

#[derive(Clone, Debug)]
struct TierRule {
    agent: String,
    provider: String,
    model: String,
    value: String,
}

impl TierTable {
    fn load() -> TierTable {
        let parsed: Value = serde_json::from_str(MODEL_TIERS_TEXT).unwrap_or(Value::Null);
        TierTable {
            tiers: parse_rules(&parsed, "tiers", "tier"),
            billing: parse_rules(&parsed, "billing", "billing"),
        }
    }

    fn tier_of(&self, agent: &str, provider: &str, model: &str) -> String {
        self.tiers
            .iter()
            .find(|rule| rule.matches(agent, provider, model))
            .map(|rule| rule.value.clone())
            .unwrap_or_else(|| TIER_UNKNOWN.to_string())
    }

    fn billing_of(&self, agent: &str, provider: &str, model: &str) -> String {
        self.billing
            .iter()
            .find(|rule| rule.matches(agent, provider, model))
            .map(|rule| rule.value.clone())
            .unwrap_or_else(|| BILLING_UNKNOWN.to_string())
    }
}

fn parse_rules(table: &Value, key: &str, field: &str) -> Vec<TierRule> {
    table
        .get(key)
        .and_then(Value::as_array)
        .map(|rules| {
            rules
                .iter()
                .filter_map(|rule| {
                    let value = rule.get(field)?.as_str()?.to_string();
                    Some(TierRule {
                        agent: string_field(rule, "agent"),
                        provider: string_field(rule, "provider"),
                        model: string_field(rule, "model"),
                        value,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn string_field(rule: &Value, key: &str) -> String {
    rule.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_lowercase()
}

impl TierRule {
    fn matches(&self, agent: &str, provider: &str, model: &str) -> bool {
        (self.agent.is_empty() || self.agent == agent.to_lowercase())
            // provider 按前缀匹配：真实 provider 名带后缀（zai-coding-cn、
            // opencode-go），表里写短名（fix-50-r1）。
            && (self.provider.is_empty() || provider.to_lowercase().starts_with(&self.provider))
            && (self.model.is_empty() || model.to_lowercase().contains(&self.model))
    }
}

/// 候选池：全部已探测 agent 下状态为「可用」或「已验证」的模型，按资源
/// 库顺序。
fn candidate_pool(store: &ResourceStore, table: &TierTable) -> Vec<Candidate> {
    let now_millis = chrono::Local::now().timestamp_millis();
    let mut pool = Vec::new();
    for agent in &store.agents {
        // 未实测的 agent（TASK-61）一律不进推荐；手动选仍可。
        if !agent.detected || crate::agent_catalog::is_untested(&agent.id) {
            continue;
        }
        for model in &agent.models {
            let (state, _) = resources::model_state(model, agent.detected, now_millis);
            let verified = match state {
                "可用" => false,
                "已验证" => true,
                _ => continue,
            };
            pool.push(Candidate {
                agent: agent.id.clone(),
                provider: (!model.provider.trim().is_empty()).then(|| model.provider.clone()),
                model: model.id.clone(),
                vendor: model.vendor.trim().to_string(),
                tier: table.tier_of(&agent.id, &model.provider, &model.id),
                billing: table.billing_of(&agent.id, &model.provider, &model.id),
                verified,
            });
        }
    }
    pool
}

/// 各 agent 下「可用但未验证」的第三方模型数（走查 #4、#5）：agent 已探测、
/// 模型不带「未在事实源中」标记、不是原生、没挂额度也没登出，验证还没过
/// ——这些模型因此没进候选池。没有的 agent 不列，全部没有时为空表。
pub(crate) fn unverified_counts(store: &ResourceStore) -> Vec<(String, usize)> {
    let now_millis = chrono::Local::now().timestamp_millis();
    store
        .agents
        .iter()
        .filter_map(|agent| {
            let count = agent
                .models
                .iter()
                .filter(|model| {
                    resources::model_state(model, agent.detected, now_millis).0
                        == resources::RESOURCE_STATE_UNVERIFIED
                })
                .count();
            (count > 0).then(|| (agent.id.clone(), count))
        })
        .collect()
}

/// 档位排序值。出方案 / 拆分 / 顾问：高 → 中 → 低 → 未知；
/// 执行及其它角色：中 → 低 → 未知 → 高（高档浪费，排最后）。
fn tier_rank(tier: &str, planning: bool) -> u8 {
    match (planning, tier) {
        (true, TIER_HIGH) | (false, TIER_MID) => 0,
        (true, TIER_MID) | (false, TIER_LOW) => 1,
        (true, TIER_LOW) | (false, TIER_UNKNOWN) => 2,
        (false, TIER_HIGH) => 3,
        _ => 3,
    }
}

fn billing_rank(billing: &str) -> u8 {
    match billing {
        BILLING_SUBSCRIPTION => 0,
        BILLING_PER_CALL => 2,
        _ => 1,
    }
}

/// 角色归类：出方案 / 拆分 / 顾问按高档选；验收 / 验证 / 仲裁按跨厂商选；
/// 其余（执行及用户预设的其它角色）按中低档选。
fn is_planning_role(role: &str) -> bool {
    matches!(role, "出方案" | "拆分" | "顾问")
}

fn is_reviewer_role(role: &str) -> bool {
    matches!(role, "验收" | "验证" | "仲裁")
}

/// 生效人选的厂商（资源库里查不到或没有厂商信息时为空串）。
fn effective_vendor(store: &ResourceStore, target: &Target) -> String {
    effective_vendor_of(store, &target.agent, target.provider.as_deref(), &target.model)
}

fn effective_vendor_of(
    store: &ResourceStore,
    agent: &str,
    provider: Option<&str>,
    model: &str,
) -> String {
    let models = store
        .agents
        .iter()
        .find(|candidate_agent| candidate_agent.id == agent)
        .map(|candidate_agent| candidate_agent.models.as_slice())
        .unwrap_or(&[]);
    resources::find_target_model(models, agent, provider.unwrap_or_default(), model)
        .map(|found| found.vendor.trim().to_string())
        .unwrap_or_default()
}

/// 一次推荐 run 里对单个预设的处理结果。
pub(crate) struct PresetPicks {
    preset_id: String,
    seats: Vec<Vec<Option<SeatChoice>>>,
    changed: bool,
}

/// 这个位置经本轮推荐后是否仍由现在的生效人选跑（它的厂商要占位，
/// 限制同阶段其它审查者与执行厂商判断）。只有预设写死与保留的手选
/// 先行占位；空位与 recommended 位不算——推荐会重新填它们，角色默认
/// 位要等推荐结果：凑不出人、维持角色默认时才占位（见推荐主流程）。
fn kept_effective(source: &str, seat: Option<&SeatChoice>, fill_all: bool) -> bool {
    if source == "预设" {
        return true;
    }
    match seat {
        // 手选的人选不动；recommended 会被换掉，现厂商不占位。
        Some(seat) => !fill_all && !seat.is_recommended(),
        // 空位（含角色默认补位的位置）不预先占位。
        None => false,
    }
}

/// 比较用的厂商：只有主 agent 的原生模型（provider 为空，且主 agent 是
/// claude / codex / kimi）与主 agent 走同一配置端点，厂商按主 agent 实际
/// 厂商算，不按资源库的原生厂商算，避免被当成另一家选为审查者。pi 例外：
/// pi 下的模型（含默认模型）各自带 provider、各走各的端点，一律用资源库
/// 的厂商（fix-50-r2：此前整组 pi 模型都被改记成主 agent 厂商，审查位
/// 凑不出人）。推定未知时不改写（此时审查者本就不做厂商限制）。
fn is_host_native(agent: &str, provider: Option<&str>, host: &str) -> bool {
    agent == host
        && host != "pi"
        && provider.unwrap_or_default().trim().is_empty()
}

fn comparison_vendor_of_target(
    store: &ResourceStore,
    target: &Target,
    host: &str,
    host_vendor: &str,
    host_vendor_known: bool,
) -> String {
    if host_vendor_known && is_host_native(&target.agent, target.provider.as_deref(), host) {
        return host_vendor.to_string();
    }
    effective_vendor(store, target)
}

fn candidate_comparison_vendor(
    candidate: &Candidate,
    host: &str,
    host_vendor: &str,
    host_vendor_known: bool,
) -> String {
    if host_vendor_known && is_host_native(&candidate.agent, candidate.provider.as_deref(), host) {
        return host_vendor.to_string();
    }
    candidate.vendor.clone()
}

/// 厂商相同判断：两边先规范化（中文旧写法、别名、大小写），再比较。
fn normalized_vendor(value: &str) -> String {
    crate::hosts::normalize_vendor(value)
}

/// 厂商是否参与同阶段去重与主 agent 避让：「其他」（认不出）与空厂商
/// 不参与——既不挡同阶段的其它位置，也不被挡（fix-50-r2）。
fn participates_in_vendor_rules(vendor: &str) -> bool {
    !vendor.is_empty() && vendor != "其他" && vendor != resources::VENDOR_UNKNOWN
}

/// 位置硬规则（TASK-61 / fix-61-r1）：违反时返回原因，候选与角色默认都按它筛。
/// - 只接盲的 agent 不进非盲位置；
/// - 未实测的 agent 不进推荐；
/// - 只看得到文字的 agent（agy）不填验收位（验收要读项目副本、跑验证）；
/// - 厂商「未知」的模型只当执行位（出方案 / 拆分 / 顾问 / 审查位都不进）。
fn seat_rule_violation(agent: &str, vendor: &str, role: &str, visibility: &str) -> Option<String> {
    if let Some(reason) = crate::agent_catalog::position_violation(agent, role, visibility) {
        return Some(reason);
    }
    if crate::agent_catalog::is_untested(agent) {
        return Some(format!("{agent} 是未实测通道，不进推荐"));
    }
    if normalized_vendor(vendor) == resources::VENDOR_UNKNOWN && role != "执行" {
        return Some("厂商未知，只当执行位".to_string());
    }
    None
}

/// 某个位置的候选：候选池里不违反位置硬规则的那些（保持原顺序）。
fn seat_pool(pool: &[Candidate], role: &str, visibility: &str) -> Vec<Candidate> {
    pool.iter()
        .filter(|candidate| {
            seat_rule_violation(&candidate.agent, &candidate.vendor, role, visibility).is_none()
        })
        .cloned()
        .collect()
}

/// 审查位同人判断用的键：agent/provider/model，provider 空时为空串。
fn seat_model_key(agent: &str, provider: Option<&str>, model: &str) -> (String, String, String) {
    (
        agent.to_string(),
        provider.unwrap_or_default().trim().to_string(),
        model.to_string(),
    )
}

/// 人选的一行描述（agent/provider/model，provider 空时省略）。
fn target_desc(target: &Target) -> String {
    let provider = target.provider.as_deref().unwrap_or_default().trim();
    if provider.is_empty() {
        format!("{}/{}", target.agent, target.model)
    } else {
        format!("{}/{}/{}", target.agent, provider, target.model)
    }
}

/// 推荐实现体。store / modes / presets / roles 由调用方注入，便于测试；
/// 命令行与窗口共用。`host_vendor_known` 为 false 时审查者不做厂商限制，
/// notes 里说明「无法保证跨厂商」。
pub(crate) fn recommend_into(
    store: &ResourceStore,
    modes: &ModesFile,
    host: &str,
    host_vendor: &str,
    host_vendor_known: bool,
    presets: &[Preset],
    roles: &RoleSettings,
    wanted_mode: Option<&str>,
    fill_all: bool,
) -> (Vec<PresetPicks>, Vec<String>) {
    let table = TierTable::load();
    let pool = candidate_pool(store, &table);
    let unverified_total: usize = unverified_counts(store)
        .iter()
        .map(|(_, count)| count)
        .sum();
    let mut notes: Vec<String> = Vec::new();
    if !host_vendor_known {
        notes.push("主 agent 厂商未知，无法保证跨厂商".to_string());
    }
    let section = modes.hosts.get(host);
    let mut picks = Vec::new();
    for preset in presets {
        if let Some(wanted) = wanted_mode {
            if preset.id != wanted {
                continue;
            }
        }
        // TASK-56：按该主 agent 调整后的形状推荐，写出的人选与派发同形。
        let preset = &crate::orchestration::shaped_preset(preset, section);
        let (filled, sources) =
            crate::engine::fill_seats(preset, roles, section.and_then(|s| s.seats.get(&preset.id)));
        // 角色默认模板：推荐后仍空着的位置若它会补位，那个人照样跑，
        // 厂商要占位并在 notes 写明。
        // 违反位置硬规则的角色默认不进模板（与派发同一份补人），原因在
        // role_reasons。
        let (role_template, _, role_reasons) =
            crate::engine::fill_seats_explained(preset, roles, None);
        let old_seats = section.and_then(|s| s.seats.get(&preset.id));
        let seat_at = |i: usize, j: usize| -> Option<&SeatChoice> {
            old_seats
                .and_then(|stages| stages.get(i))
                .and_then(|nodes| nodes.get(j))
                .and_then(Option::as_ref)
        };
        // 推荐后仍会跑的执行厂商，与各阶段审查者已占的厂商（都存规范化
        // 后的写法，比较不区分大小写）；「其他」与空厂商不占位。审查位
        // 另记已占的具体模型，同阶段不重复选同一个人（「其他」厂商退出
        // 去重后靠这个抵挡，fix-50-r2）。
        let mut execution_vendors: HashSet<String> = HashSet::new();
        let mut stage_reviewer_vendors: HashMap<usize, HashSet<String>> = HashMap::new();
        let mut stage_reviewer_models: HashMap<usize, HashSet<(String, String, String)>> =
            HashMap::new();
        // 走查 #6：执行与规划位的同阶段已选清单——同一个模型不重复、优先
        // 尚未在本阶段出现过的厂商，都是软条件（没有别的候选时允许重复）。
        // 两类角色合用一份清单是有意的：内置模式同一阶段只有执行位，合并
        // 与分开无区别；只有自定义模式在一个阶段里混排规划位与执行位时，
        // 两类位置才会互相去重（同阶段的规划位占掉的模型，执行位不再选）。
        let mut stage_used_vendors: HashMap<usize, HashSet<String>> = HashMap::new();
        let mut stage_used_models: HashMap<usize, HashSet<(String, String, String)>> =
            HashMap::new();
        for (i, stage) in sources.iter().enumerate() {
            for (j, source) in stage.iter().enumerate() {
                let target = &filled.stages[i].nodes[j];
                if !kept_effective(source, seat_at(i, j), fill_all) {
                    continue;
                }
                let vendor = normalized_vendor(&comparison_vendor_of_target(
                    store, target, host, host_vendor, host_vendor_known,
                ));
                if target.role == "执行" && participates_in_vendor_rules(&vendor) {
                    execution_vendors.insert(vendor.clone());
                }
                if is_reviewer_role(&target.role) {
                    if participates_in_vendor_rules(&vendor) {
                        stage_reviewer_vendors.entry(i).or_default().insert(vendor);
                    }
                    stage_reviewer_models
                        .entry(i)
                        .or_default()
                        .insert(seat_model_key(
                            &target.agent,
                            target.provider.as_deref(),
                            &target.model,
                        ));
                } else {
                    if participates_in_vendor_rules(&vendor) {
                        stage_used_vendors.entry(i).or_default().insert(vendor);
                    }
                    stage_used_models
                        .entry(i)
                        .or_default()
                        .insert(seat_model_key(
                            &target.agent,
                            target.provider.as_deref(),
                            &target.model,
                        ));
                }
            }
        }
        let mut seats: Vec<Vec<Option<SeatChoice>>> = old_seats.cloned().unwrap_or_default();
        let mut changed = false;
        for (i, stage) in filled.stages.iter().enumerate() {
            for (j, target) in stage.nodes.iter().enumerate() {
                let existing = seat_at(i, j);
                // 预设写死了模型的位置不用位置人选，推荐也不填。
                if sources[i][j] == "预设" {
                    continue;
                }
                // manual 不动；recommended 与空位可填；--all 全部重填。
                if let Some(seat) = existing {
                    if !fill_all && !seat.is_recommended() {
                        continue;
                    }
                }
                let empty_models = HashSet::new();
                // 位置硬规则（TASK-61 / fix-61-r1）筛掉不合规的候选。
                let seat_pool = seat_pool(&pool, &target.role, &target.visibility);
                let pick = choose_candidate(
                    &seat_pool,
                    &target.role,
                    host,
                    host_vendor,
                    host_vendor_known,
                    &execution_vendors,
                    stage_reviewer_vendors.get(&i).cloned().unwrap_or_default(),
                    stage_reviewer_models.get(&i).unwrap_or(&empty_models),
                    stage_used_vendors.get(&i).cloned().unwrap_or_default(),
                    stage_used_models.get(&i).unwrap_or(&empty_models),
                );
                if seats.len() <= i {
                    seats.resize(filled.stages.len(), Vec::new());
                }
                if seats[i].len() <= j {
                    seats[i].resize(stage.nodes.len(), None);
                }
                match pick {
                    Some(candidate) => {
                        let seat = SeatChoice {
                            agent: candidate.agent.clone(),
                            provider: candidate.provider.clone(),
                            model: candidate.model.clone(),
                            tier: String::new(),
                            thinking: String::new(),
                            fallback: None,
                            origin: orchestration::SEAT_ORIGIN_RECOMMENDED.to_string(),
                        };
                        if seats[i][j].as_ref() != Some(&seat) {
                            changed = true;
                        }
                        // 新选的人占厂商位：同阶段审查者与执行厂商判断都用它。
                        let vendor = normalized_vendor(&candidate_comparison_vendor(
                            &candidate, host, host_vendor, host_vendor_known,
                        ));
                        if is_reviewer_role(&target.role) {
                            if participates_in_vendor_rules(&vendor) {
                                stage_reviewer_vendors
                                    .entry(i)
                                    .or_default()
                                    .insert(vendor.clone());
                            }
                            stage_reviewer_models
                                .entry(i)
                                .or_default()
                                .insert(seat_model_key(
                                    &candidate.agent,
                                    candidate.provider.as_deref(),
                                    &candidate.model,
                                ));
                        } else {
                            if participates_in_vendor_rules(&vendor) {
                                stage_used_vendors.entry(i).or_default().insert(vendor.clone());
                            }
                            stage_used_models
                                .entry(i)
                                .or_default()
                                .insert(seat_model_key(
                                    &candidate.agent,
                                    candidate.provider.as_deref(),
                                    &candidate.model,
                                ));
                        }
                        if target.role == "执行" && participates_in_vendor_rules(&vendor) {
                            execution_vendors.insert(vendor);
                        }
                        seats[i][j] = Some(seat);
                    }
                    None => {
                        notes.push(format!(
                            "「{}」第 {} 阶段第 {} 个位置（{}）：{}",
                            preset.name,
                            i + 1,
                            j + 1,
                            target.role,
                            unfilled_reason(
                                &pool,
                                host_vendor,
                                host_vendor_known,
                                &target.role,
                                unverified_total,
                            ),
                        ));
                        // 凑不出人时若角色默认会补位，那个人照样跑：厂商占位
                        // （同阶段其它审查者与执行厂商的判断把它算进去），并在
                        // notes 写明（fix-50-r1：空位不再预先当作占位）。
                        // 角色默认本身违反位置硬规则时不退回它（fix-61-r1 2）：
                        // 补人的共同路径 fill_seats 已不补（fix-fullcheck A5），
                        // 模板里这一位为空、附原因；不占位，写明需手动指定。
                        if let Some(reason) = &role_reasons[i][j] {
                            notes.push(reason.clone());
                        } else if !role_template.stages[i].nodes[j].model.trim().is_empty() {
                            let default_target = &role_template.stages[i].nodes[j];
                            // 推荐自己的挑人规则（未实测、厂商未知等）只管推荐
                            // 不选它；派发时角色默认照样补位，照实写明。
                            let soft = seat_rule_violation(
                                &default_target.agent,
                                &effective_vendor(store, default_target),
                                &target.role,
                                &target.visibility,
                            );
                            let vendor = normalized_vendor(&comparison_vendor_of_target(
                                store, default_target, host, host_vendor, host_vendor_known,
                            ));
                            if is_reviewer_role(&target.role) {
                                if participates_in_vendor_rules(&vendor) {
                                    stage_reviewer_vendors
                                        .entry(i)
                                        .or_default()
                                        .insert(vendor.clone());
                                }
                                stage_reviewer_models
                                    .entry(i)
                                    .or_default()
                                    .insert(seat_model_key(
                                        &default_target.agent,
                                        default_target.provider.as_deref(),
                                        &default_target.model,
                                    ));
                            } else {
                                if participates_in_vendor_rules(&vendor) {
                                    stage_used_vendors.entry(i).or_default().insert(vendor.clone());
                                }
                                stage_used_models
                                    .entry(i)
                                    .or_default()
                                    .insert(seat_model_key(
                                        &default_target.agent,
                                        default_target.provider.as_deref(),
                                        &default_target.model,
                                    ));
                            }
                            if target.role == "执行" && participates_in_vendor_rules(&vendor) {
                                execution_vendors.insert(vendor);
                            }
                            notes.push(match soft {
                                Some(reason) => format!(
                                    "使用角色默认 {}（不在推荐范围：{reason}）",
                                    target_desc(default_target),
                                ),
                                None => format!("使用角色默认 {}", target_desc(default_target)),
                            });
                        }
                        if seats[i][j].is_some() {
                            changed = true;
                        }
                        seats[i][j] = None;
                    }
                }
            }
        }
        orchestration::shape_seats_to_preset(&mut seats, preset);
        if !changed {
            changed = match old_seats {
                Some(old) => old != &seats,
                None => seats.iter().any(|row| row.iter().any(Option::is_some)),
            };
        }
        // 写盘前补一次检查（fix-50-r2）：维持角色默认等情况下，最终配置的
        // 同阶段审查厂商可能重复或撞上主 agent 厂商，把冲突写进 notes。
        append_stage_conflict_notes(
            preset,
            &seats,
            &role_template,
            store,
            host,
            host_vendor,
            host_vendor_known,
            &mut notes,
        );
        picks.push(PresetPicks {
            preset_id: preset.id.clone(),
            seats,
            changed,
        });
    }
    (picks, notes)
}

/// 终检（fix-50-r2）：按最终 seats 逐阶段核对审查位（含角色默认补位的
/// 空位）——同阶段厂商重复、或撞上主 agent 厂商时追加 notes。推荐过程中
/// 的占位判断只能挡住它之后的选择，维持角色默认的位置可能和更早定下的
/// 人选冲突，这里在写盘前统一补一次。「其他」与空厂商不参与。
fn append_stage_conflict_notes(
    preset: &Preset,
    seats: &[Vec<Option<SeatChoice>>],
    role_template: &Preset,
    store: &ResourceStore,
    host: &str,
    host_vendor: &str,
    host_vendor_known: bool,
    notes: &mut Vec<String>,
) {
    let host_vendor_norm = normalized_vendor(host_vendor);
    for (i, stage) in seats.iter().enumerate() {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for (j, seat) in stage.iter().enumerate() {
            let Some(node) = role_template.stages.get(i).and_then(|stages| stages.nodes.get(j))
            else {
                continue;
            };
            if !is_reviewer_role(&node.role) {
                continue;
            }
            let vendor = match seat {
                Some(seat) => {
                    let base = if host_vendor_known
                        && is_host_native(&seat.agent, seat.provider.as_deref(), host)
                    {
                        host_vendor.to_string()
                    } else {
                        effective_vendor_of(
                            store,
                            &seat.agent,
                            seat.provider.as_deref(),
                            &seat.model,
                        )
                    };
                    normalized_vendor(&base)
                }
                None => {
                    let default_target = &role_template.stages[i].nodes[j];
                    if default_target.model.trim().is_empty() {
                        continue;
                    }
                    normalized_vendor(&comparison_vendor_of_target(
                        store,
                        default_target,
                        host,
                        host_vendor,
                        host_vendor_known,
                    ))
                }
            };
            if !participates_in_vendor_rules(&vendor) {
                continue;
            }
            *counts.entry(vendor).or_insert(0) += 1;
        }
        for (vendor, count) in &counts {
            if *count >= 2 {
                notes.push(format!(
                    "「{}」第 {} 阶段 {count} 个审查位置同为 {vendor}",
                    preset.name,
                    i + 1
                ));
            }
            if host_vendor_known && *vendor == host_vendor_norm {
                notes.push(format!(
                    "「{}」第 {} 阶段审查位置与主 agent 同为 {vendor}",
                    preset.name,
                    i + 1
                ));
            }
        }
    }
}

fn unfilled_reason(
    pool: &[Candidate],
    host_vendor: &str,
    host_vendor_known: bool,
    role: &str,
    unverified_total: usize,
) -> String {
    let base = if pool.is_empty() {
        "候选池为空：先探测资源，第三方模型要先冒烟通过".to_string()
    } else if is_reviewer_role(role) {
        if host_vendor_known {
            format!("无可用人选：需不同于主 agent 厂商 {host_vendor}，且同阶段不重复")
        } else {
            "候选池里凑不出厂商互不相同的模型".to_string()
        }
    } else {
        "候选池里没有满足档位要求的模型".to_string()
    };
    // 走查 #4、#5：有可用但未验证的模型时给出路——去测连接，测过就能进
    // 候选池。
    if unverified_total > 0 {
        return format!(
            "{base}；另有 {unverified_total} 个未验证模型，测试连接后可参与推荐"
        );
    }
    base
}

/// 为一个位置挑人。先按硬条件过滤（审查者的厂商约束，主 agent 自己的
/// 模型按端点厂商算），再按软偏好稳定排序（同分保持资源库顺序），
/// 取第一个。厂商比较都先规范化，不区分大小写与写法。执行与规划位的
/// 排序键是（本阶段已选同一模型，本阶段已有该厂商，档位，计费）——都是
/// 软条件，没有别的候选时允许重复（走查 #6）。
#[allow(clippy::too_many_arguments)]
fn choose_candidate(
    pool: &[Candidate],
    role: &str,
    host: &str,
    host_vendor: &str,
    host_vendor_known: bool,
    execution_vendors: &HashSet<String>,
    stage_reviewer_vendors: HashSet<String>,
    stage_reviewer_models: &HashSet<(String, String, String)>,
    stage_used_vendors: HashSet<String>,
    stage_used_models: &HashSet<(String, String, String)>,
) -> Option<Candidate> {
    let planning = is_planning_role(role);
    let reviewer = is_reviewer_role(role);
    let host_vendor_norm = normalized_vendor(host_vendor);
    let vendor_of = |candidate: &Candidate| -> String {
        normalized_vendor(&candidate_comparison_vendor(
            candidate, host, host_vendor, host_vendor_known,
        ))
    };
    let mut candidates: Vec<&Candidate> = pool
        .iter()
        .filter(|candidate| {
            if !reviewer {
                return true;
            }
            let vendor = vendor_of(candidate);
            // 同一阶段不重复选同一个人；「其他」与空厂商不参与厂商去重与
            // 避让（fix-50-r2），但同人去重不受此限。
            if stage_reviewer_models.contains(&seat_model_key(
                &candidate.agent,
                candidate.provider.as_deref(),
                &candidate.model,
            )) {
                return false;
            }
            if !participates_in_vendor_rules(&vendor) {
                return true;
            }
            if host_vendor_known && vendor == host_vendor_norm {
                return false;
            }
            !stage_reviewer_vendors.contains(&vendor)
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    candidates.sort_by_key(|candidate| {
        let vendor = vendor_of(candidate);
        if reviewer {
            // 尽量不同于执行厂商；已验证的第三方优先；订阅优先、按次最后。
            [
                u8::from(
                    participates_in_vendor_rules(&vendor)
                        && execution_vendors.contains(&vendor),
                ),
                u8::from(!candidate.verified),
                billing_rank(&candidate.billing),
                0,
            ]
        } else {
            // 同阶段已选过同一模型、已有同一厂商的往后排（软条件），
            // 再按档位与计费排。
            [
                u8::from(stage_used_models.contains(&seat_model_key(
                    &candidate.agent,
                    candidate.provider.as_deref(),
                    &candidate.model,
                ))),
                u8::from(
                    participates_in_vendor_rules(&vendor)
                        && stage_used_vendors.contains(&vendor),
                ),
                tier_rank(&candidate.tier, planning),
                billing_rank(&candidate.billing),
            ]
        }
    });
    candidates.first().map(|candidate| (*candidate).clone())
}

/// `config recommend` 的实现体；窗口 `config_recommend` 同此（返回与命令行
/// stdout 同结构）。不带 --mode 时对该主 agent 的全部模式做。写入后输出
/// `{ defaultMode, modes, notes }`（modes 同 `config show`）。
pub(crate) fn recommend_at(
    root: &Path,
    host_given: Option<&str>,
    mode: Option<&str>,
    fill_all: bool,
) -> Result<Value, (i32, String)> {
    let home = crate::paths::home().map_err(|message| (3, message))?;
    let env_base_url = std::env::var("ANTHROPIC_BASE_URL").ok();
    recommend_at_home(
        root,
        &home,
        host_given,
        mode,
        fill_all,
        env_base_url.as_deref(),
    )
}

/// 同上，家目录与 claude 环境变量都注入（测试用临时目录，不读真实环境）。
/// 整个「严格读 → 计算 → 写盘」在 modes 写锁内完成（fix-50-r1：不在锁外
/// 拿旧快照、写盘时整份覆盖——期间手选的其它模式不会丢；modes.json 损坏
/// 或旧格式时先报错，不再按空配置计算）。输出前的读回（modes_section 要
/// 再拿这把锁）在出锁之后做（fix-50-r2）。
pub(crate) fn recommend_at_home(
    root: &Path,
    home: &Path,
    host_given: Option<&str>,
    mode: Option<&str>,
    fill_all: bool,
    claude_base_url_env: Option<&str>,
) -> Result<Value, (i32, String)> {
    let store = resources::get_resources_at(&crate::console_cli::store_file(root))
        .map_err(|message| (3, message))?;
    let detected = crate::hosts::detected_host_ids(&store);
    let roles = orchestration::roles_from(root);
    let (host, notes) = {
        let _guard = crate::console_cli::modes_write_lock(root)?;
        // 预设在锁内读（fix-fullcheck A4）：按 preset save / remove 之后的
        // 形状推荐与写盘，不拿锁外的旧快照。
        let presets = orchestration::raw_presets_from(root).presets;
        let wanted = match mode {
            Some(mode) => {
                let wanted = crate::console_cli::resolve_preset_name(mode);
                if !presets.iter().any(|preset| preset.id == wanted) {
                    return Err((2, crate::console_cli::unknown_mode_message(mode, &presets)));
                }
                Some(wanted)
            }
            None => None,
        };
        let modes = orchestration::modes_from_strict(root).map_err(|message| {
            (
                1,
                format!("{message}；已停止写入，修复或删除该文件后重试"),
            )
        })?;
        let host = match crate::hosts::resolve_host(host_given, &modes, &detected) {
            crate::hosts::HostResolve::Detected(host) => host,
            crate::hosts::HostResolve::None => return Err((2, need_host_message(&detected))),
            crate::hosts::HostResolve::NeedHost(configured) => {
                return Err((2, format!("需要 --host，可选：{}", configured.join("、"))))
            }
            crate::hosts::HostResolve::ConfiguredNotDetected(host, detected) => {
                return Err((2, configured_not_detected_message(&host, &detected)))
            }
            crate::hosts::HostResolve::Unknown(host, detected) => {
                return Err((
                    2,
                    format!(
                        "未知 host：{host}；检测到：{}",
                        join_or_empty(&detected)
                    ),
                ))
            }
        };
        let override_vendor = modes
            .hosts
            .get(&host)
            .map(|section| section.vendor_override.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(|value| crate::hosts::normalize_vendor(&value));
        let (vendor, vendor_known) = match override_vendor {
            Some(vendor) => (vendor, true),
            None => crate::hosts::collect_hosts(&store, &modes, home, claude_base_url_env)
                .into_iter()
                .find(|info| info.id == host)
                .map(|info| {
                    let known = info.vendor_source != crate::hosts::VENDOR_SOURCE_UNKNOWN;
                    (info.vendor, known)
                })
                .unwrap_or_else(|| ("未知".to_string(), false)),
        };
        let (picks, notes) = recommend_into(
            &store,
            &modes,
            &host,
            &vendor,
            vendor_known,
            &presets,
            &roles,
            wanted.as_deref(),
            fill_all,
        );
        if picks.iter().any(|pick| pick.changed) {
            // 写盘用同一份严格读到的快照改出来，只动本轮推荐算过的模式。
            let mut updated = modes;
            for pick in &picks {
                if !pick.changed {
                    continue;
                }
                let preset = presets
                    .iter()
                    .find(|preset| preset.id == pick.preset_id)
                    .expect("pick 来自 presets");
                // TASK-56：按调整后的形状整理，减掉的位置不因这里回填。
                let shaped =
                    crate::orchestration::shaped_preset(preset, updated.hosts.get(&host));
                let section = orchestration::host_section_mut(&mut updated, &host);
                let stages = section.seats.entry(pick.preset_id.clone()).or_default();
                *stages = pick.seats.clone();
                orchestration::shape_seats_to_preset(stages, &shaped);
            }
            orchestration::save_modes_in(root, &updated).map_err(|message| (1, message))?;
        }
        (host, notes)
    };
    // 读回不占 modes 锁：modes_section 自己要拿这把锁（fix-50-r2）。
    let (default_mode, modes_list) = crate::console_cli::modes_section(root, Some(&host));
    // 走查 #4、#5：可用但未验证、因此没进候选池的第三方模型数，给出路
    // （去测试连接）；没有时为空数组。
    let unverified = unverified_counts(&store)
        .into_iter()
        .map(|(agent, count)| json!({ "agent": agent, "count": count }))
        .collect::<Vec<_>>();
    Ok(json!({
        "defaultMode": default_mode,
        "modes": modes_list,
        "notes": notes,
        "unverified": unverified,
    }))
}

fn need_host_message(detected: &[String]) -> String {
    if detected.is_empty() {
        return "需要 --host；本机没有检测到的主 agent，先在资源页探测".to_string();
    }
    format!("需要 --host，可选：{}", detected.join("、"))
}

/// modes.json 里唯一的配置是推定出的主 agent，但它已不被检测到（fix-50-r2）。
fn configured_not_detected_message(host: &str, detected: &[String]) -> String {
    format!(
        "modes.json 里唯一的配置 {host} 未被检测到，请用 --host 指定，可选：{}",
        join_or_empty(detected)
    )
}

fn join_or_empty(items: &[String]) -> String {
    if items.is_empty() {
        "（无）".to_string()
    } else {
        items.join("、")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::{
        builtin_presets, HostModes, ModesFile, SeatChoice, SEAT_ORIGIN_MANUAL,
        SEAT_ORIGIN_RECOMMENDED,
    };
    use crate::resources::{ResourceAgent, ResourceModel};

    fn model(
        agent: &str,
        provider: &str,
        id: &str,
        vendor: &str,
        verification: &str,
    ) -> ResourceModel {
        ResourceModel {
            id: id.into(),
            agent: agent.into(),
            provider: provider.into(),
            vendor: vendor.into(),
            verification: verification.into(),
            ..ResourceModel::default()
        }
    }

    /// 候选池夹具：claude 原生一条（vendor 按官方算）；pi 三条第三方已验证
    /// （Zhipu 订阅 ×2、DeepSeek 按次）加一条验证失败、一条未发现；codex 原生
    /// 一条。全部 detected。vendor 用资源库的英文厂商名（infer_vendor 的真实
    /// 输出），不手写中文厂商名。
    fn fixture_store() -> ResourceStore {
        let claude_models = vec![model("claude", "", "sonnet", "Anthropic", "native")];
        let pi_models = vec![
            model("pi", "zai", "glm-4.7", "Zhipu", "verified"),
            model("pi", "zai", "glm-4.7-flash", "Zhipu", "verified"),
            model("pi", "deepseek", "deepseek-chat", "DeepSeek", "verified"),
            model("pi", "zai", "bad", "Zhipu", "failed"),
            {
                let mut missing = model("pi", "zai", "ghost", "Zhipu", "verified");
                missing.meta = crate::resources::MISSING_MARKER.into();
                missing
            },
        ];
        let codex_models = vec![model("codex", "", "exec", "OpenAI", "native")];
        ResourceStore {
            version: 1,
            agents: vec![
                ResourceAgent {
                    id: "claude".into(),
                    name: "Claude Code".into(),
                    detected: true,
                    models: claude_models,
                    ..ResourceAgent::default()
                },
                ResourceAgent {
                    id: "codex".into(),
                    name: "Codex".into(),
                    detected: true,
                    models: codex_models,
                    ..ResourceAgent::default()
                },
                ResourceAgent {
                    id: "pi".into(),
                    name: "pi agent".into(),
                    detected: true,
                    models: pi_models,
                    ..ResourceAgent::default()
                },
            ],
            detected_at: None,
        }
    }

    fn fixture_modes() -> ModesFile {
        ModesFile {
            version: crate::orchestration::MODES_VERSION,
            hosts: indexmap::IndexMap::from([(
                "claude".into(),
                HostModes::default(),
            )]),
        }
    }

    fn preset(name: &str) -> Preset {
        builtin_presets()
            .into_iter()
            .find(|preset| preset.id == name)
            .unwrap()
    }

    fn pick_of<'a>(picks: &'a [PresetPicks], preset_id: &str) -> &'a PresetPicks {
        picks.iter().find(|pick| pick.preset_id == preset_id).unwrap()
    }

    fn seat_desc(seat: &SeatChoice) -> String {
        format!("{}/{}/{}", seat.agent, seat.provider.clone().unwrap_or_default(), seat.model)
    }

    /// 空位与 recommended 位置都填 manual 不动；recommended 指向的模型后来
    /// 变成未发现时整体换人。
    #[test]
    fn task50_recommend_fills_empty_and_recommended_but_not_manual() {
        let store = fixture_store();
        let mut modes = fixture_modes();
        let section = modes.hosts.get_mut("claude").unwrap();
        // 经济模式第 3 阶段第 1 位（验收）：manual 不动；第 2 位（验证）：
        // recommended 指向已失效的模型（未发现），下次推荐要换人。
        section.seats.insert(
            "builtin-economy".into(),
            vec![
                vec![],
                vec![],
                vec![
                    Some(SeatChoice {
                        agent: "pi".into(),
                        provider: Some("zai".into()),
                        model: "glm-4.7".into(),
                        origin: SEAT_ORIGIN_MANUAL.into(),
                        ..SeatChoice::default()
                    }),
                    Some(SeatChoice {
                        agent: "pi".into(),
                        provider: Some("zai".into()),
                        model: "ghost".into(),
                        origin: SEAT_ORIGIN_RECOMMENDED.into(),
                        ..SeatChoice::default()
                    }),
                ],
            ],
        );
        let presets = vec![preset("builtin-economy")];
        let (picks, notes) = recommend_into(
            &store,
            &modes,
            "claude",
            "Anthropic",
            true,
            &presets,
            &crate::orchestration::RoleSettings::default(),
            None,
            false,
        );
        let pick = pick_of(&picks, "builtin-economy");
        let stages = &pick.seats;
        // 出方案：高档没有，中档里订阅的 claude/sonnet 先（资源库顺序）。
        let planning = stages[0][0].as_ref().unwrap();
        assert_eq!(seat_desc(planning), "claude//sonnet");
        assert!(planning.is_recommended());
        // 执行：中低档优先，同上取 claude/sonnet。
        assert_eq!(seat_desc(stages[1][0].as_ref().unwrap()), "claude//sonnet");
        // 验收：manual 原样。
        let acceptance = stages[2][0].as_ref().unwrap();
        assert_eq!(seat_desc(acceptance), "pi/zai/glm-4.7");
        assert!(!acceptance.is_recommended());
        // 验证：recommended 且已失效 → 换成与主 agent（Anthropic）不同厂商、
        // 已验证优先的第三方（DeepSeek，验证过；codex 是原生的排后）。
        let verification = stages[2][1].as_ref().unwrap();
        assert_eq!(seat_desc(verification), "pi/deepseek/deepseek-chat");
        assert!(verification.is_recommended());
        assert!(notes.is_empty(), "{notes:?}");
    }

    /// --all：manual 也重填。
    #[test]
    fn task50_recommend_all_refills_manual() {
        let store = fixture_store();
        let mut modes = fixture_modes();
        let section = modes.hosts.get_mut("claude").unwrap();
        section.seats.insert(
            "builtin-blind".into(),
            vec![vec![Some(SeatChoice {
                agent: "claude".into(),
                provider: None,
                model: "sonnet".into(),
                origin: SEAT_ORIGIN_MANUAL.into(),
                ..SeatChoice::default()
            }), None, None]],
        );
        let presets = vec![preset("builtin-blind")];
        let (picks, _) = recommend_into(
            &store,
            &modes,
            "claude",
            "Anthropic",
            true,
            &presets,
            &crate::orchestration::RoleSettings::default(),
            None,
            true,
        );
        let seats = &pick_of(&picks, "builtin-blind").seats;
        // 全部重填：三个位置都是 recommended，同阶段厂商互不相同、避开
        // Anthropic。可用厂商：智谱、OpenAI、DeepSeek 恰好三家。
        let vendor_of = |seat: &SeatChoice| -> String {
            store
                .agents
                .iter()
                .find(|agent| agent.id == seat.agent)
                .unwrap()
                .models
                .iter()
                .find(|model| model.id == seat.model && model.provider == seat.provider.as_deref().unwrap_or_default())
                .unwrap()
                .vendor
                .clone()
        };
        let vendors: Vec<String> = seats[0].iter().flatten().map(&vendor_of).collect();
        let mut unique = vendors.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 3, "三个位置三家不同厂商：{vendors:?}");
        for seat in seats[0].iter().flatten() {
            assert!(seat.is_recommended());
            // 资源库里 claude/sonnet 的厂商是 Anthropic（主 agent），不入选。
            assert_ne!(seat_desc(seat), "claude//sonnet");
        }
    }

    /// 主 agent 厂商未知：不做厂商限制，notes 说明无法保证跨厂商；同阶段
    /// 仍要求互不相同。
    #[test]
    fn task50_recommend_unknown_host_vendor_notes_and_still_varies_stage() {
        let store = fixture_store();
        let modes = fixture_modes();
        // 只留智谱的第三方候选：bad 与 ghost 不进池（验证失败 / 未发现），
        // glm-4.7 与 flash 同厂商 → 盲审三路只有一路凑得出。
        let mut store = store;
        store.agents.retain(|agent| agent.id != "codex");
        store.agents.retain(|agent| agent.id != "claude");
        let presets = vec![preset("builtin-blind")];
        let (picks, notes) = recommend_into(
            &store,
            &modes,
            "claude",
            "未知",
            false,
            &presets,
            &crate::orchestration::RoleSettings::default(),
            None,
            false,
        );
        let seats = &pick_of(&picks, "builtin-blind").seats;
        // 主 agent 未知：智谱（订阅）与 DeepSeek（按次）都能入选，第三路
        // 没有第三家厂商 → 留空。
        assert_eq!(seat_desc(seats[0][0].as_ref().unwrap()), "pi/zai/glm-4.7");
        assert_eq!(seat_desc(seats[0][1].as_ref().unwrap()), "pi/deepseek/deepseek-chat");
        assert!(seats[0][2].is_none(), "第三路没有第三家厂商：{seats:?}");
        assert!(
            notes.iter().any(|note| note.contains("主 agent 厂商未知，无法保证跨厂商")),
            "{notes:?}"
        );
        assert_eq!(
            notes.iter().filter(|note| note.contains("第 1 阶段")).count(),
            1,
            "凑不出的那个位置有一条 notes：{notes:?}"
        );
    }

    /// 候选池为空：全部留空，notes 写明原因。
    #[test]
    fn task50_recommend_empty_pool_leaves_empty_with_note() {
        let mut store = fixture_store();
        store.agents.clear();
        let modes = fixture_modes();
        let presets = vec![preset("builtin-blind")];
        let (picks, notes) = recommend_into(
            &store,
            &modes,
            "claude",
            "Anthropic",
            true,
            &presets,
            &crate::orchestration::RoleSettings::default(),
            None,
            false,
        );
        let seats = &pick_of(&picks, "builtin-blind").seats;
        assert!(seats[0].iter().all(Option::is_none));
        assert!(notes.iter().any(|note| note.contains("候选池为空")), "{notes:?}");
    }

    /// 按次计费在同档位时排订阅后面：按次模型在资源库里排前、同档，
    /// 订阅的仍然入选（fix-50-r1：夹具不再靠资源库顺序侥幸通过）；
    /// 结果确定（两次同输出）。
    #[test]
    fn task50_recommend_subscription_before_per_call_and_deterministic() {
        // 夹具：deepseek（按次，中档）排在 glm（订阅，中档）前面；
        // 主 agent 厂商未知，两家都可选。共享模式只有一个执行位置。
        let store = ResourceStore {
            version: 1,
            agents: vec![ResourceAgent {
                id: "pi".into(),
                name: "pi agent".into(),
                detected: true,
                models: vec![
                    model("pi", "deepseek", "deepseek-chat", "DeepSeek", "verified"),
                    model("pi", "zai", "glm-4.7", "Zhipu", "verified"),
                ],
                ..ResourceAgent::default()
            }],
            detected_at: None,
        };
        let modes = fixture_modes();
        let presets = vec![preset("builtin-shared")];
        let roles = crate::orchestration::RoleSettings::default();
        let (picks, _) = recommend_into(
            &store,
            &modes,
            "claude",
            "未知",
            false,
            &presets,
            &roles,
            None,
            false,
        );
        let first = pick_of(&picks, "builtin-shared").seats[0][0]
            .as_ref()
            .unwrap()
            .clone();
        // 同档内订阅优先：DeepSeek（按次）虽然排前也不入选。
        assert_eq!(seat_desc(&first), "pi/zai/glm-4.7", "订阅优先于按次");
        let (picks_again, _) = recommend_into(
            &store,
            &modes,
            "claude",
            "未知",
            false,
            &presets,
            &roles,
            None,
            false,
        );
        assert_eq!(
            pick_of(&picks, "builtin-shared").seats,
            pick_of(&picks_again, "builtin-shared").seats,
            "同输入同输出"
        );
    }

    /// --mode 只做一个模式；其余模式不动。
    #[test]
    fn task50_recommend_single_mode_leaves_other_modes_untouched() {
        let store = fixture_store();
        let modes = fixture_modes();
        let presets = vec![preset("builtin-shared"), preset("builtin-blind")];
        let (picks, _) = recommend_into(
            &store,
            &modes,
            "claude",
            "Anthropic",
            true,
            &presets,
            &crate::orchestration::RoleSettings::default(),
            Some("builtin-blind"),
            false,
        );
        assert!(pick_of(&picks, "builtin-blind").seats[0][0].is_some());
        // 只对指定的模式做：没有给共享模式产出 pick。
        assert_eq!(picks.len(), 1);
        assert_eq!(picks[0].preset_id, "builtin-blind");
    }

    /// 主 agent 是 claude 且推定为第三方端点（Zhipu）时，claude 原生模型
    /// （同一后端）按端点厂商算，不能被当成另一家选为审查者（fix-50-r1：
    /// 此前按资源库里的 Anthropic 算，会漏进审查者）。
    #[test]
    fn task50_recommend_host_native_models_count_as_endpoint_vendor() {
        let store = fixture_store();
        let modes = fixture_modes();
        let presets = vec![preset("builtin-economy")];
        let (picks, _) = recommend_into(
            &store,
            &modes,
            "claude",
            "Zhipu",
            true,
            &presets,
            &crate::orchestration::RoleSettings::default(),
            None,
            false,
        );
        let seats = &pick_of(&picks, "builtin-economy").seats;
        // 出方案与执行没有厂商限制：claude/sonnet（中档订阅）先。
        assert_eq!(seat_desc(seats[0][0].as_ref().unwrap()), "claude//sonnet");
        assert_eq!(seat_desc(seats[1][0].as_ref().unwrap()), "claude//sonnet");
        // 审查者避开端点厂商 Zhipu：claude 原生 sonnet（同一后端）与 pi 的
        // Zhipu 模型都不入选，验收取 DeepSeek、验证取 OpenAI（codex 原生）。
        assert_eq!(seat_desc(seats[2][0].as_ref().unwrap()), "pi/deepseek/deepseek-chat");
        assert_eq!(seat_desc(seats[2][1].as_ref().unwrap()), "codex//exec");
        assert!(
            !seats[2].iter().flatten().any(|seat| seat.model == "sonnet"),
            "claude 原生模型不能当审查者：{:?}",
            seats[2]
        );
    }

    /// kimi 主 agent（Moonshot）时，厂商为 Moonshot 的模型（含 pi 上的
    /// moonshot provider 与 kimi 原生模型）不会被选为审查者（fix-50-r1）。
    #[test]
    fn task50_recommend_kimi_host_excludes_moonshot_reviewers() {
        let store = ResourceStore {
            version: 1,
            agents: vec![
                ResourceAgent {
                    id: "kimi".into(),
                    name: "Kimi Code".into(),
                    detected: true,
                    models: vec![model("kimi", "", "k2", "Moonshot", "native")],
                    ..ResourceAgent::default()
                },
                ResourceAgent {
                    id: "pi".into(),
                    name: "pi agent".into(),
                    detected: true,
                    models: vec![
                        model("pi", "zai", "glm-4.7", "Zhipu", "verified"),
                        model("pi", "moonshot", "kimi-k2", "Moonshot", "verified"),
                        model("pi", "deepseek", "deepseek-chat", "DeepSeek", "verified"),
                    ],
                    ..ResourceAgent::default()
                },
            ],
            detected_at: None,
        };
        let modes = fixture_modes();
        let presets = vec![preset("builtin-blind")];
        let (picks, _) = recommend_into(
            &store,
            &modes,
            "kimi",
            "Moonshot",
            true,
            &presets,
            &crate::orchestration::RoleSettings::default(),
            None,
            false,
        );
        let seats = &pick_of(&picks, "builtin-blind").seats;
        // 三路审查者：Zhipu、DeepSeek 各占一路；Moonshot 的两处（kimi 原生
        // 与 pi 的 moonshot provider）都不入选，第三路没有第三家 → 留空。
        assert_eq!(seat_desc(seats[0][0].as_ref().unwrap()), "pi/zai/glm-4.7");
        assert_eq!(seat_desc(seats[0][1].as_ref().unwrap()), "pi/deepseek/deepseek-chat");
        assert!(seats[0][2].is_none(), "{:?}", seats[0]);
        assert!(
            !seats[0].iter().flatten().any(|seat| seat.model == "kimi-k2" || seat.model == "k2"),
            "Moonshot 模型不能当审查者：{:?}",
            seats[0]
        );
    }

    /// 角色默认补位的位置不预先占厂商位：推荐先尽力填，填不进、维持角色
    /// 默认时才占位并在 notes 写明（fix-50-r1；此前默认厂商提前占位，挤掉
    /// 同阶段其它位置的合理人选）。
    #[test]
    fn task50_recommend_role_default_not_placeholder_until_final() {
        // 夹具只有两家：claude 原生（主 agent 厂商）与 pi 的 Zhipu 模型。
        let store = ResourceStore {
            version: 1,
            agents: vec![
                ResourceAgent {
                    id: "claude".into(),
                    name: "Claude Code".into(),
                    detected: true,
                    models: vec![model("claude", "", "sonnet", "Anthropic", "native")],
                    ..ResourceAgent::default()
                },
                ResourceAgent {
                    id: "pi".into(),
                    name: "pi agent".into(),
                    detected: true,
                    models: vec![model("pi", "zai", "glm-4.7", "Zhipu", "verified")],
                    ..ResourceAgent::default()
                },
            ],
            detected_at: None,
        };
        let mut modes = fixture_modes();
        // modes.json 里验收、验证两个位置都空着：来源是角色默认。
        modes.hosts.get_mut("claude").unwrap().seats.insert(
            "builtin-economy".into(),
            vec![vec![], vec![], vec![None, None]],
        );
        // roles.json 默认值：验收、验证都由 pi 的 Zhipu 模型补。
        let roles = crate::orchestration::RoleSettings {
            acceptance: crate::orchestration::RoleProfile {
                agent: "pi".into(),
                provider: "zai".into(),
                model: "glm-4.7".into(),
                ..crate::orchestration::RoleProfile::default()
            },
            ..crate::orchestration::RoleSettings::default()
        };
        let presets = vec![preset("builtin-economy")];
        let (picks, notes) = recommend_into(
            &store,
            &modes,
            "claude",
            "Anthropic",
            true,
            &presets,
            &roles,
            None,
            false,
        );
        let seats = &pick_of(&picks, "builtin-economy").seats;
        // 验收（第 1 位）：角色默认的 Zhipu 不再提前占位，推荐照常选出它。
        assert_eq!(seat_desc(seats[2][0].as_ref().unwrap()), "pi/zai/glm-4.7");
        // 验证（第 2 位）：同阶段厂商不能重复，池里只剩主 agent 厂商
        // （Anthropic）的 claude/sonnet → 凑不出，留空、维持角色默认。
        assert!(seats[2][1].is_none(), "{:?}", seats[2]);
        assert!(
            notes.iter().any(|note| note.contains("使用角色默认 pi/zai/glm-4.7")),
            "{notes:?}"
        );
        // 出方案、执行照常由推荐填（角色默认只在验收、验证上生效）。
        assert_eq!(seat_desc(seats[0][0].as_ref().unwrap()), "claude//sonnet");
        assert_eq!(seat_desc(seats[1][0].as_ref().unwrap()), "claude//sonnet");
    }

    /// 出方案优先高档：高档模型在资源库里排后也先入选（fix-50-r1 补用例）。
    #[test]
    fn task50_recommend_planning_prefers_high_tier() {
        // sonnet（中档）在前，opus（高档）在后。
        let store = ResourceStore {
            version: 1,
            agents: vec![ResourceAgent {
                id: "claude".into(),
                name: "Claude Code".into(),
                detected: true,
                models: vec![
                    model("claude", "", "sonnet", "Anthropic", "native"),
                    model("claude", "", "opus", "Anthropic", "native"),
                ],
                ..ResourceAgent::default()
            }],
            detected_at: None,
        };
        let modes = fixture_modes();
        let presets = vec![preset("builtin-economy")];
        let (picks, _) = recommend_into(
            &store,
            &modes,
            "claude",
            "Anthropic",
            true,
            &presets,
            &crate::orchestration::RoleSettings::default(),
            None,
            false,
        );
        let seats = &pick_of(&picks, "builtin-economy").seats;
        // 出方案：高档 opus 先于中档 sonnet，无视资源库顺序。
        assert_eq!(seat_desc(seats[0][0].as_ref().unwrap()), "claude//opus");
        // 执行优先中低档：opus（高档）排最后，取 sonnet。
        assert_eq!(seat_desc(seats[1][0].as_ref().unwrap()), "claude//sonnet");
    }

    /// 档位表按真实 provider 名前缀命中：zai-coding-cn、opencode-go；
    /// claude 别名 opus / sonnet / haiku / fable；表里没有的为未知
    /// （fix-50-r1：provider 从精确匹配改为前缀匹配）。
    #[test]
    fn task50_tier_table_matches_real_provider_prefixes() {
        let table = TierTable::load();
        assert_eq!(table.tier_of("pi", "zai-coding-cn", "glm-4.7"), "中");
        assert_eq!(table.billing_of("pi", "zai-coding-cn", "glm-4.7"), "订阅");
        assert_eq!(table.tier_of("pi", "zai-coding-cn", "glm-4.7-flash"), "低");
        assert_eq!(table.tier_of("pi", "opencode-go", "grok-code"), "中");
        assert_eq!(table.billing_of("pi", "opencode-go", "grok-code"), "按次");
        assert_eq!(table.tier_of("claude", "", "claude-opus-4-5"), "高");
        assert_eq!(table.tier_of("claude", "", "claude-sonnet-4-5"), "中");
        assert_eq!(table.tier_of("claude", "", "claude-haiku-4-5"), "低");
        assert_eq!(table.tier_of("claude", "", "fable"), "高");
        assert_eq!(table.billing_of("claude", "", "claude-opus-4-5"), "订阅");
        // 表里没有 → 未知。
        assert_eq!(table.tier_of("pi", "unknown-provider", "x-model"), "未知");
        assert_eq!(table.billing_of("pi", "unknown-provider", "x-model"), "未知");
    }

    /// pi 主 agent（厂商推定自默认模型，Moonshot）时，pi 下的模型一律按
    /// 资源库厂商参与审查位选拔，不会被整组改记成主 agent 厂商（fix-50-r2：
    /// 此前所有 pi 模型都被改写，审查位凑不出人）。
    #[test]
    fn task50_recommend_pi_host_models_keep_resource_vendors() {
        let store = ResourceStore {
            version: 1,
            agents: vec![ResourceAgent {
                id: "pi".into(),
                name: "pi agent".into(),
                detected: true,
                models: vec![
                    model("pi", "moonshot", "kimi-k2", "Moonshot", "verified"),
                    model("pi", "zai", "glm-4.7", "Zhipu", "verified"),
                    model("pi", "deepseek", "deepseek-chat", "DeepSeek", "verified"),
                ],
                ..ResourceAgent::default()
            }],
            detected_at: None,
        };
        let modes = ModesFile::default();
        let presets = vec![preset("builtin-blind")];
        let (picks, notes) = recommend_into(
            &store,
            &modes,
            "pi",
            "Moonshot",
            true,
            &presets,
            &crate::orchestration::RoleSettings::default(),
            None,
            false,
        );
        let seats = &pick_of(&picks, "builtin-blind").seats;
        // 三路审查位：智谱、DeepSeek 各占一路；主 agent 厂商 Moonshot（含
        // 默认模型 kimi-k2）不入选，第三路留空。
        assert_eq!(seat_desc(seats[0][0].as_ref().unwrap()), "pi/zai/glm-4.7");
        assert_eq!(seat_desc(seats[0][1].as_ref().unwrap()), "pi/deepseek/deepseek-chat");
        assert!(seats[0][2].is_none(), "{:?}", seats[0]);
        assert!(
            !seats[0].iter().flatten().any(|seat| seat.model == "kimi-k2"),
            "主 agent 厂商的模型不能当审查者：{:?}",
            seats[0]
        );
        // 只有第三路（没有第三家厂商）写 notes；前两路照常入选。
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].contains("第 3 个位置"), "{notes:?}");
    }

    /// 「其他」与空厂商不参与同阶段去重与主 agent 避让（fix-50-r2）：手选
    /// 审查者厂商是「其他」时，其余位置照常能从池里选人（包括另一个认不
    /// 出厂商的模型），同阶段也不重复选同一个模型。
    #[test]
    fn task50_recommend_unrecognized_vendors_do_not_block_reviewers() {
        let store = ResourceStore {
            version: 1,
            agents: vec![ResourceAgent {
                id: "pi".into(),
                name: "pi agent".into(),
                detected: true,
                models: vec![
                    model("pi", "router", "model-a", "其他", "verified"),
                    model("pi", "relay", "model-b", "其他", "verified"),
                    model("pi", "zai", "glm-4.7", "Zhipu", "verified"),
                ],
                ..ResourceAgent::default()
            }],
            detected_at: None,
        };
        let mut modes = ModesFile::default();
        // 盲审第 1 路手选了厂商「其他」的 model-a。
        modes.hosts.insert(
            "claude".into(),
            HostModes {
                seats: indexmap::IndexMap::from([(
                    "builtin-blind".into(),
                    vec![vec![
                        Some(SeatChoice {
                            agent: "pi".into(),
                            provider: Some("router".into()),
                            model: "model-a".into(),
                            origin: SEAT_ORIGIN_MANUAL.into(),
                            ..SeatChoice::default()
                        }),
                        None,
                        None,
                    ]],
                )]),
                ..HostModes::default()
            },
        );
        let presets = vec![preset("builtin-blind")];
        let (picks, notes) = recommend_into(
            &store,
            &modes,
            "claude",
            "Anthropic",
            true,
            &presets,
            &crate::orchestration::RoleSettings::default(),
            None,
            false,
        );
        let seats = &pick_of(&picks, "builtin-blind").seats;
        // 第 1 路手选不动；第 2 路取智谱（订阅优先），第 3 路取另一个
        // 「其他」厂商的 model-b（认不出的厂商不挡人也不被挡；同阶段的
        // model-a 已被手选占位，不重复选）。
        assert_eq!(seat_desc(seats[0][0].as_ref().unwrap()), "pi/router/model-a");
        assert_eq!(seat_desc(seats[0][1].as_ref().unwrap()), "pi/zai/glm-4.7");
        assert_eq!(seat_desc(seats[0][2].as_ref().unwrap()), "pi/relay/model-b");
        assert!(
            !notes.iter().any(|note| note.contains("第 1 阶段")),
            "三个位置都该有入选或保留的人选：{notes:?}"
        );
    }

    /// 维持角色默认后的同阶段冲突在写盘前补一次检查（fix-50-r2）：角色
    /// 默认与更早定下的人选同厂商时，notes 写明冲突。
    #[test]
    fn task50_recommend_stage_conflict_note_when_role_default_kept() {
        // 夹具只有两家：claude 原生（主 agent 厂商 Anthropic）与 pi 的智谱。
        let store = ResourceStore {
            version: 1,
            agents: vec![
                ResourceAgent {
                    id: "claude".into(),
                    name: "Claude Code".into(),
                    detected: true,
                    models: vec![model("claude", "", "sonnet", "Anthropic", "native")],
                    ..ResourceAgent::default()
                },
                ResourceAgent {
                    id: "pi".into(),
                    name: "pi agent".into(),
                    detected: true,
                    models: vec![model("pi", "zai", "glm-4.7", "Zhipu", "verified")],
                    ..ResourceAgent::default()
                },
            ],
            detected_at: None,
        };
        let mut modes = fixture_modes();
        // 验收位置手选 pi 的智谱模型（manual，推荐不动）；验证位置空着。
        modes.hosts.get_mut("claude").unwrap().seats.insert(
            "builtin-economy".into(),
            vec![
                vec![],
                vec![],
                vec![
                    Some(SeatChoice {
                        agent: "pi".into(),
                        provider: Some("zai".into()),
                        model: "glm-4.7".into(),
                        origin: SEAT_ORIGIN_MANUAL.into(),
                        ..SeatChoice::default()
                    }),
                    None,
                ],
            ],
        );
        // 角色默认：验收 / 验证都用 pi 的智谱模型。
        let roles = crate::orchestration::RoleSettings {
            acceptance: crate::orchestration::RoleProfile {
                agent: "pi".into(),
                provider: "zai".into(),
                model: "glm-4.7".into(),
                ..crate::orchestration::RoleProfile::default()
            },
            ..crate::orchestration::RoleSettings::default()
        };
        let presets = vec![preset("builtin-economy")];
        let (picks, notes) = recommend_into(
            &store,
            &modes,
            "claude",
            "Anthropic",
            true,
            &presets,
            &roles,
            None,
            false,
        );
        let seats = &pick_of(&picks, "builtin-economy").seats;
        // 验证位凑不出（智谱被同阶段手选挡、claude 撞主 agent 厂商）→
        // 维持角色默认；终检发现同阶段两个智谱，追加冲突 notes。
        assert!(seats[2][1].is_none(), "{:?}", seats[2]);
        assert!(
            notes.iter().any(|note| {
                note.contains("第 3 阶段 2 个审查位置同为 Zhipu")
            }),
            "{notes:?}"
        );
    }

    /// TASK-56：按调整后的形状推荐。擂台减到 2 只填两个位置，多出的旧
    /// 人选一并裁掉；盲审加到 4 后按 5 个位置（验收固定加四路验证）填，
    /// 同阶段厂商互不相同，凑不出的留空。
    #[test]
    fn task56_recommend_fills_adjusted_shape() {
        let store = fixture_store();
        let mut modes = fixture_modes();
        let section = modes.hosts.get_mut("claude").unwrap();
        section.counts.insert("builtin-arena".into(), 2);
        section.counts.insert("builtin-blind".into(), 4);
        // 擂台原有 3 路手选：减到 2 后第三路裁掉。
        let manual = |model: &str| SeatChoice {
            agent: "claude".into(),
            provider: None,
            model: model.into(),
            origin: SEAT_ORIGIN_MANUAL.into(),
            ..SeatChoice::default()
        };
        section.seats.insert(
            "builtin-arena".into(),
            vec![vec![Some(manual("sonnet")), None, Some(manual("extra"))]],
        );
        let presets = vec![preset("builtin-arena"), preset("builtin-blind")];
        let (picks, notes) = recommend_into(
            &store,
            &modes,
            "claude",
            "Anthropic",
            true,
            &presets,
            &crate::orchestration::RoleSettings::default(),
            None,
            false,
        );
        let arena_pick = pick_of(&picks, "builtin-arena");
        let arena = &arena_pick.seats;
        assert_eq!(arena[0].len(), 2, "减到 2 个位置：{arena:?}");
        assert_eq!(arena[0][0].as_ref().unwrap().model, "sonnet", "手选不动");
        let filled = arena[0][1].as_ref().unwrap();
        assert!(filled.is_recommended(), "空位被填：{arena:?}");
        assert!(arena_pick.changed, "形状变化算有改动");
        // 盲审 5 个位置全是审查位：可用厂商只有智谱、DeepSeek、OpenAI
        // 三家，同阶段互不相同 → 恰好填三路，其余留空并写 notes。
        let blind = &pick_of(&picks, "builtin-blind").seats;
        assert_eq!(blind[0].len(), 5, "验收固定加 4 路验证：{blind:?}");
        assert_eq!(blind[0].iter().flatten().count(), 3);
        for seat in blind[0].iter().flatten() {
            assert!(seat.is_recommended());
        }
        assert_eq!(
            notes.iter().filter(|note| note.contains("盲审模式")).count(),
            2,
            "凑不出的两个位置各有 notes：{notes:?}"
        );
    }

    /// 走查 #6：擂台 3 个执行位在候选池含三家时选出三家不同模型，不再
    /// 全填同一个。
    #[test]
    fn task57_arena_positions_spread_across_models() {
        let store = fixture_store();
        let modes = fixture_modes();
        let presets = vec![preset("builtin-arena")];
        let (picks, notes) = recommend_into(
            &store,
            &modes,
            "claude",
            "Anthropic",
            true,
            &presets,
            &crate::orchestration::RoleSettings::default(),
            None,
            false,
        );
        assert!(notes.is_empty(), "{notes:?}");
        let seats = &pick_of(&picks, "builtin-arena").seats;
        let models: Vec<String> = seats[0]
            .iter()
            .flatten()
            .map(|seat| seat_desc(seat))
            .collect();
        assert_eq!(models.len(), 3, "{models:?}");
        let mut unique = models.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 3, "三个执行位三个不同模型：{models:?}");
        // 按档位与计费依次选出三家（fixture 池：claude/sonnet 中档订阅、
        // codex/exec 中档订阅、pi 两家第三方），不再是同一家连填三席。
        assert_eq!(
            models,
            vec![
                "claude//sonnet".to_string(),
                "codex//exec".to_string(),
                "pi/zai/glm-4.7".to_string(),
            ],
            "{models:?}"
        );
        for seat in seats[0].iter().flatten() {
            assert!(seat.is_recommended());
        }
    }

    /// 走查 #6：候选池只有一个模型时，软条件让位，三个执行位都填它。
    #[test]
    fn task57_arena_positions_repeat_when_pool_has_single_model() {
        let store = ResourceStore {
            version: 1,
            agents: vec![ResourceAgent {
                id: "pi".into(),
                name: "pi agent".into(),
                detected: true,
                models: vec![model("pi", "zai", "glm-4.7", "Zhipu", "verified")],
                ..ResourceAgent::default()
            }],
            detected_at: None,
        };
        let modes = fixture_modes();
        let presets = vec![preset("builtin-arena")];
        let (picks, notes) = recommend_into(
            &store,
            &modes,
            "claude",
            "Anthropic",
            true,
            &presets,
            &crate::orchestration::RoleSettings::default(),
            None,
            false,
        );
        assert!(notes.is_empty(), "没有别的候选时允许重复：{notes:?}");
        let seats = &pick_of(&picks, "builtin-arena").seats;
        for seat in seats[0].iter().flatten() {
            assert_eq!(seat_desc(seat), "pi/zai/glm-4.7");
        }
        assert_eq!(seats[0].iter().flatten().count(), 3);
    }

    /// 走查 #4、#5：可用但未验证的第三方模型不进候选池，但按 agent 计数
    /// 进 unverified；凑不出的 notes 末尾给出路（另有 N 个未验证模型，
    /// 测试连接后可参与推荐）。原生与未发现的不计。
    #[test]
    fn task57_unverified_models_counted_and_unfilled_reason_points_the_way() {
        let mut store = ResourceStore {
            version: 1,
            agents: vec![ResourceAgent {
                id: "pi".into(),
                name: "pi agent".into(),
                detected: true,
                models: vec![
                    model("pi", "zai", "glm-4.7", "Zhipu", "verified"),
                    model("pi", "zai", "glm-5.3", "Zhipu", "unverified"),
                    model("pi", "bigmodel", "glm-4.6", "Zhipu", "unverified"),
                    // 验证失败的不算「可用但未验证」。
                    model("pi", "zai", "bad", "Zhipu", "failed"),
                    // 额度用尽的不算。
                    {
                        let mut quota = model("pi", "zai", "quota", "Zhipu", "unverified");
                        quota.quota_exhausted_until = Some("2099-01-01T00:00:00+08:00".into());
                        quota
                    },
                ],
                ..ResourceAgent::default()
            }],
            detected_at: None,
        };
        // 未发现的不计。
        let mut ghost = model("pi", "zai", "ghost", "Zhipu", "unverified");
        ghost.meta = crate::resources::MISSING_MARKER.into();
        store.agents[0].models.push(ghost);
        assert_eq!(
            unverified_counts(&store),
            vec![("pi".to_string(), 2)]
        );
        // 盲审三路只有智谱一家已验证：后两路凑不出，notes 给出路。
        let modes = fixture_modes();
        let presets = vec![preset("builtin-blind")];
        let (picks, notes) = recommend_into(
            &store,
            &modes,
            "claude",
            "Anthropic",
            true,
            &presets,
            &crate::orchestration::RoleSettings::default(),
            None,
            false,
        );
        let seats = &pick_of(&picks, "builtin-blind").seats;
        assert_eq!(seats[0].iter().flatten().count(), 1);
        let pointing = notes
            .iter()
            .filter(|note| note.contains("另有 2 个未验证模型，测试连接后可参与推荐"))
            .count();
        assert_eq!(pointing, 2, "凑不出的两个位置各带一句出路：{notes:?}");
        // 没有未验证模型时不追加。
        let clean = fixture_store();
        assert!(unverified_counts(&clean).is_empty());
    }

    /// TASK-61：未实测 agent 的模型一律不进推荐（即使已验证）；厂商「未知」
    /// 的模型只当执行位、不进验收 / 验证位；只接盲位置的 agy 不进共享位置。
    #[test]
    fn task61_recommend_skips_untested_unknown_vendor_reviewers_and_blind_only_shared() {
        let store = ResourceStore {
            version: 1,
            agents: vec![
                ResourceAgent {
                    id: "claude".into(),
                    name: "Claude Code".into(),
                    detected: true,
                    models: vec![model("claude", "", "sonnet", "Anthropic", "native")],
                    ..ResourceAgent::default()
                },
                ResourceAgent {
                    id: "cursor".into(),
                    name: "Cursor".into(),
                    detected: true,
                    untested: true,
                    models: vec![model("cursor", "", "gpt-5", "OpenAI", "verified")],
                    ..ResourceAgent::default()
                },
                ResourceAgent {
                    id: "pi".into(),
                    name: "pi agent".into(),
                    detected: true,
                    models: vec![model("pi", "x", "mystery", "未知", "verified")],
                    ..ResourceAgent::default()
                },
                ResourceAgent {
                    id: "agy".into(),
                    name: "Antigravity".into(),
                    detected: true,
                    models: vec![
                        model("agy", "", "gemini-3.8-flash-high", "Google", "native"),
                        model("agy", "", "claude-opus-5-5-high", "Anthropic", "native"),
                    ],
                    ..ResourceAgent::default()
                },
            ],
            detected_at: None,
        };
        let presets = vec![preset("builtin-economy")];
        let (picks, _) = recommend_into(
            &store,
            &fixture_modes(),
            "claude",
            "Anthropic",
            true,
            &presets,
            &crate::orchestration::RoleSettings::default(),
            None,
            false,
        );
        let seats = &pick_of(&picks, "builtin-economy").seats;
        let all: Vec<String> = seats.iter().flatten().flatten().map(seat_desc).collect();
        assert!(!all.iter().any(|seat| seat.starts_with("cursor/")), "{all:?}");
        // 出方案、执行是共享位置：agy 不进
        for stage in &seats[..2] {
            for seat in stage.iter().flatten() {
                assert_ne!(seat.agent, "agy", "{all:?}");
            }
        }
        // 验收：agy 读不到项目副本不填（fix-61-r1 1），未知厂商不进 → 留空；
        // 验证：主 agent 厂商 Anthropic 避开（含 agy 的 claude 档）→ agy 的 Gemini
        assert!(seats[2][0].is_none(), "{:?}", seats[2]);
        assert_eq!(seat_desc(seats[2][1].as_ref().unwrap()), "agy//gemini-3.8-flash-high");
        // 未知厂商只当执行位：出方案 / 拆分 / 顾问 / 审查位都筛掉（fix-61-r1 6）
        let only_unknown = vec![Candidate {
            agent: "pi".into(),
            provider: Some("x".into()),
            model: "mystery".into(),
            vendor: "未知".into(),
            tier: TIER_UNKNOWN.into(),
            billing: BILLING_UNKNOWN.into(),
            verified: true,
        }];
        assert_eq!(seat_pool(&only_unknown, "执行", "共享").len(), 1);
        for role in ["出方案", "拆分", "顾问", "验收", "验证", "仲裁"] {
            assert!(seat_pool(&only_unknown, role, "盲").is_empty(), "{role}");
        }
    }

    /// fix-61-r1 6：自定义模式里同时有规划位与执行位时，厂商「未知」的模型
    /// 只填执行位，规划位凑不出人留空。
    #[test]
    fn task61_r1_unknown_vendor_skips_planning_seat_in_custom_mode() {
        let store = ResourceStore {
            version: 1,
            agents: vec![ResourceAgent {
                id: "pi".into(),
                name: "pi agent".into(),
                detected: true,
                models: vec![model("pi", "x", "mystery", "未知", "verified")],
                ..ResourceAgent::default()
            }],
            detected_at: None,
        };
        let shared = |role: &str| Target {
            role: role.into(),
            visibility: "共享".into(),
            write: "只读".into(),
            ..Target::default()
        };
        let custom = Preset {
            id: "custom-plan-exec".into(),
            name: "自定义".into(),
            stages: vec![
                crate::orchestration::Stage { nodes: vec![shared("出方案")] },
                crate::orchestration::Stage { nodes: vec![shared("顾问"), shared("执行")] },
            ],
            ..Preset::default()
        };
        let (picks, notes) = recommend_into(
            &store,
            &fixture_modes(),
            "claude",
            "Anthropic",
            true,
            &[custom],
            &crate::orchestration::RoleSettings::default(),
            None,
            true,
        );
        let seats = &pick_of(&picks, "custom-plan-exec").seats;
        assert!(seats[0][0].is_none(), "出方案不填未知厂商：{seats:?}");
        assert!(seats[1][0].is_none(), "顾问不填未知厂商：{seats:?}");
        assert_eq!(seat_desc(seats[1][1].as_ref().unwrap()), "pi/x/mystery");
        assert!(notes.iter().any(|note| note.contains("出方案")), "{notes:?}");
    }

    /// fix-61-r1 2：凑不出合规候选时，不退回违反硬规则的角色默认（agy 设成
    /// 执行角色默认、落在共享执行位）：不写「使用角色默认」，写明需手动指定。
    #[test]
    fn task61_r1_recommend_does_not_fall_back_to_illegal_role_default() {
        let store = ResourceStore {
            version: 1,
            agents: vec![ResourceAgent {
                id: "agy".into(),
                name: "Antigravity".into(),
                detected: true,
                models: vec![model("agy", "", "gemini-3.8-flash-low", "Google", "native")],
                ..ResourceAgent::default()
            }],
            detected_at: None,
        };
        let roles = crate::orchestration::RoleSettings {
            execution: crate::orchestration::RoleProfile {
                agent: "agy".into(),
                model: "gemini-3.8-flash-low".into(),
                ..crate::orchestration::RoleProfile::default()
            },
            ..crate::orchestration::RoleSettings::default()
        };
        for fill_all in [false, true] {
            let (picks, notes) = recommend_into(
                &store,
                &fixture_modes(),
                "claude",
                "Anthropic",
                true,
                &[preset("builtin-shared")],
                &roles,
                None,
                fill_all,
            );
            let seats = &pick_of(&picks, "builtin-shared").seats;
            assert!(seats[0][0].is_none(), "{seats:?}");
            assert!(!notes.iter().any(|note| note.starts_with("使用角色默认")), "{notes:?}");
            assert!(
                notes.iter().any(|note| note.contains("不合规") && note.contains("需手动指定")),
                "{notes:?}"
            );
        }
    }
}
