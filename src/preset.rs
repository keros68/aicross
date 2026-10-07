//! 自定义模式（TASK-55）：用户预设的新建、更新与删除。
//!
//! 用户预设存在 presets.json（`raw_presets_from` 读取，内置预设不落盘）。
//! `preset save` 校验后整份写入；`preset remove` 按「先改 modes.json，再删
//! 短命令文件，最后删预设」的顺序级联清理：各主 agent 下该模式的人选、
//! 指向它的默认模式、mode 为它的短命令定义与对应的带标记文件；任一步失败
//! 停止，已完成的部分写进错误说明。读改写与短命令、人选共用同一把进程内
//! 锁（`modes_write_lock`），窗口与命令行在同一进程里并发写不互相覆盖。
//! 家目录由调用方注入：生产用 `paths::home()`，测试一律用临时目录。

use crate::orchestration::{Preset, Stage, Target, THINKING_LEVELS};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;
use uuid::Uuid;

/// 全模式最多的位置数。
pub(crate) const MAX_NODES: usize = 12;
/// 模式名最长字数。
const MAX_NAME_CHARS: usize = 40;

/// `role` 只认的角色（顾问 / 仲裁与自定义文本是图界面动作，不进预设）。
const ROLE_VALUES: [&str; 5] = ["出方案", "执行", "验收", "验证", "拆分"];
/// `visibility` 只认的取值。
const VISIBILITY_VALUES: [&str; 2] = ["共享", "盲"];
/// `write` 只认的取值。
const WRITE_VALUES: [&str; 3] = ["只读", "单写者", "worktree"];

/// `preset save` 的输入：一个预设对象。位置（nodes）复用 orchestration 的
/// `Stage` / `Target` 反序列化，但只认 role / visibility / write / thinking：
/// 传入的 agent / model 等人选字段一律忽略（人选走 set-seat），保存时清空。
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct PresetInput {
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub stages: Vec<Stage>,
}

/// 用户给的预设 id：非空，字母、数字、`-`、`_`，1–64 位（生成的 uuid 同样
/// 符合）。id 会成为 modes.json 的 seats 键与短命令的 mode 引用，不做文件名。
fn validate_preset_id(id: &str) -> Result<(), (i32, String)> {
    let ok = !id.is_empty()
        && id.chars().count() <= 64
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err((
            2,
            format!("预设 id 只许字母、数字、-、_，1–64 位：{id}"),
        ))
    }
}

/// 换行与制表以外的控制字符。ANSI 转义序列以 ESC（一个控制字符）开头，
/// 同一判断覆盖；模式名与介绍都不收。
fn has_forbidden_control(text: &str) -> bool {
    text.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
}

/// 校验输入预设，返回整理过的模式名；不通过返回 (2, 原因)，原因写明哪一条。
/// `presets` 是现有全部预设（内置 + 用户，含正在更新的那份旧版本）；
/// `self_id` 为更新时的预设 id，重名检查跳过它自己。
fn validate_input(
    input: &PresetInput,
    presets: &[Preset],
    self_id: Option<&str>,
) -> Result<String, (i32, String)> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err((2, "模式名不能为空".into()));
    }
    if has_forbidden_control(name) {
        return Err((2, "模式名含控制字符或 ANSI 转义；换行与制表可以用".into()));
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err((2, format!("模式名最长 {MAX_NAME_CHARS} 字")));
    }
    if presets
        .iter()
        .any(|preset| Some(preset.id.as_str()) != self_id && preset.name.trim() == name)
    {
        return Err((2, format!("模式名与已有模式重名：{name}")));
    }
    if input.description.chars().count() > 200 {
        return Err((
            2,
            format!(
                "介绍最长 200 字，收到 {} 字",
                input.description.chars().count()
            ),
        ));
    }
    if has_forbidden_control(&input.description) {
        return Err((2, "介绍含控制字符或 ANSI 转义；换行与制表可以用".into()));
    }
    if input.stages.is_empty() {
        return Err((2, "至少需要一个阶段".into()));
    }
    let total: usize = input.stages.iter().map(|stage| stage.nodes.len()).sum();
    if total > MAX_NODES {
        return Err((
            2,
            format!("全模式最多 {MAX_NODES} 个位置，收到 {total} 个"),
        ));
    }
    for (stage_index, stage) in input.stages.iter().enumerate() {
        let stage_no = stage_index + 1;
        if stage.nodes.is_empty() {
            return Err((2, format!("第 {stage_no} 个阶段至少要有一个位置")));
        }
        let writers = stage
            .nodes
            .iter()
            .filter(|node| node.write == "单写者")
            .count();
        if writers > 1 {
            return Err((
                2,
                format!(
                    "第 {stage_no} 个阶段最多一个「单写者」位置（同一目录同时只有一个写者）"
                ),
            ));
        }
        for (node_index, node) in stage.nodes.iter().enumerate() {
            let position = format!("第 {stage_no} 个阶段第 {} 个位置", node_index + 1);
            if !ROLE_VALUES.contains(&node.role.as_str()) {
                return Err((
                    2,
                    format!(
                        "{position} 的 role 只认 {}：收到 {}",
                        ROLE_VALUES.join(" / "),
                        node.role
                    ),
                ));
            }
            if !VISIBILITY_VALUES.contains(&node.visibility.as_str()) {
                return Err((
                    2,
                    format!(
                        "{position} 的 visibility 只认 {}：收到 {}",
                        VISIBILITY_VALUES.join(" / "),
                        node.visibility
                    ),
                ));
            }
            if !WRITE_VALUES.contains(&node.write.as_str()) {
                return Err((
                    2,
                    format!(
                        "{position} 的 write 只认 {}：收到 {}",
                        WRITE_VALUES.join(" / "),
                        node.write
                    ),
                ));
            }
            if !node.thinking.is_empty()
                && !THINKING_LEVELS.contains(&node.thinking.as_str())
            {
                return Err((
                    2,
                    format!(
                        "{position} 的 thinking 只认 {} 或空串：收到 {}",
                        THINKING_LEVELS.join(" / "),
                        node.thinking
                    ),
                ));
            }
        }
    }
    Ok(name.to_string())
}

/// 按输入组装要保存的预设：只保留 role / visibility / write / thinking，
/// agent / model 等人选字段一律清空；`builtin` 由系统盖章，用户预设恒为
/// false。
fn normalized_preset(input: &PresetInput, id: String, name: String) -> Preset {
    Preset {
        id,
        name,
        description: input.description.clone(),
        builtin: false,
        stages: input
            .stages
            .iter()
            .map(|stage| Stage {
                nodes: stage
                    .nodes
                    .iter()
                    .map(|node| Target {
                        role: node.role.clone(),
                        thinking: node.thinking.clone(),
                        visibility: node.visibility.clone(),
                        write: node.write.clone(),
                        ..Target::default()
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// 更新前后阶段数、每阶段位置数或任一位置的角色变了：入选按旧形状存的，
/// 形状变了就对不上号，该模式的人选要清掉重选。
fn shape_changed(old: &Preset, updated: &Preset) -> bool {
    old.stages.len() != updated.stages.len()
        || old.stages.iter().zip(updated.stages.iter()).any(|(a, b)| {
            a.nodes.len() != b.nodes.len()
                || a
                    .nodes
                    .iter()
                    .zip(b.nodes.iter())
                    .any(|(x, y)| x.role != y.role)
        })
}

/// `preset save` 的实现体：没有 id 时新建（生成 uuid）；显式给了 id 时
/// 更新同 id 的用户预设，id 不存在退出 2（不静默新建）。内置预设不可
/// 修改；内置模式的别名（shared / economy / blind）不可作用户预设 id，
/// 否则同名预设永远轮不到被解析。更新时阶段数、每阶段位置数或任一位置
/// 的角色变了就清各主 agent 下该模式的人选（短命令与默认模式保留），
/// 受影响的主 agent 列在输出的 `seatsCleared`。输出保存后的预设与完整
/// modes（同 `config show` 的 modes，host 按命令行同一套规则解析）。
pub(crate) fn preset_save_at(
    root: &Path,
    host: Option<&str>,
    input: &PresetInput,
) -> Result<Value, (i32, String)> {
    let _guard = crate::console_cli::modes_write_lock();
    let wanted = input.id.trim().to_string();
    let explicit = !wanted.is_empty();
    let id = if explicit {
        validate_preset_id(&wanted)?;
        if crate::console_cli::resolve_preset_name(&wanted) != wanted {
            return Err((2, format!("预设 id 不能用内置模式的别名：{wanted}")));
        }
        if crate::orchestration::builtin_presets()
            .iter()
            .any(|preset| preset.id == wanted)
        {
            return Err((2, format!("内置模式不可修改：{wanted}")));
        }
        wanted
    } else {
        Uuid::new_v4().to_string()
    };
    let mut file = crate::orchestration::presets_from_strict(root).map_err(|message| (1, message))?;
    let position = file.presets.iter().position(|preset| preset.id == id);
    if let Some(index) = position {
        if file.presets[index].builtin {
            return Err((2, format!("内置模式不可修改：{id}")));
        }
    } else if explicit {
        return Err((2, format!("模式不存在：{id}")));
    }
    let self_id = position.map(|index| file.presets[index].id.clone());
    let all = crate::orchestration::raw_presets_from(root);
    let name = validate_input(input, &all.presets, self_id.as_deref())?;
    let preset = normalized_preset(input, id.clone(), name);
    // 人选清理先于写盘：modes.json 改完再写 presets.json，presets.json
    // 写失败时说明已完成到哪一步。
    let mut seats_cleared: Vec<String> = Vec::new();
    if let Some(index) = position {
        let old = file.presets[index].clone();
        if shape_changed(&old, &preset) {
            let mut modes = crate::console_cli::strict_modes_for_write(root)?;
            for (host_id, section) in modes.hosts.iter_mut() {
                if section.seats.shift_remove(&id).is_some() {
                    seats_cleared.push(host_id.clone());
                }
            }
            if !seats_cleared.is_empty() {
                crate::orchestration::save_modes_in(root, &modes)
                    .map_err(|message| (1, message))?;
            }
        }
    }
    match position {
        Some(index) => file.presets[index] = preset.clone(),
        None => file.presets.push(preset.clone()),
    }
    if let Err(message) = crate::orchestration::save_presets_in(root, &file) {
        if seats_cleared.is_empty() {
            return Err((1, message));
        }
        return Err((1, format!("{message}；该模式的人选已清空，预设未更新")));
    }
    let (default_mode, modes) = crate::console_cli::modes_section(root, host);
    Ok(json!({
        "preset": preset,
        "seatsCleared": seats_cleared,
        "defaultMode": default_mode,
        "modes": modes,
    }))
}

/// 级联清单里的「host / 命令名」一格，错误说明里用。
fn host_name_entries(entries: &[(String, String)]) -> String {
    if entries.is_empty() {
        return "无".into();
    }
    entries
        .iter()
        .map(|(host, name)| format!("{host}/{name}"))
        .collect::<Vec<_>>()
        .join("、")
}

/// `preset remove` 的实现体：级联清理该模式在各主 agent 下的人选、指向它
/// 的默认模式（清空）与短命令定义。顺序是先在锁内逐个删带标记的短命令
/// 文件，全部成功后一次写入 modes.json，最后删预设；任一步失败停止，
/// 错误说明里列出已删与剩下的条目（host / 命令名）。定义先过 host 与
/// 名称校验：不合法的只从定义里移除、不碰磁盘，结果里单独标出，不把
/// 坏值送进删文件的路径。输出删除结果与受影响项列表。
pub(crate) fn preset_remove_at(root: &Path, home: &Path, id: &str) -> Result<Value, (i32, String)> {
    if crate::orchestration::builtin_presets()
        .iter()
        .any(|preset| preset.id == id)
    {
        return Err((2, format!("内置模式不可删除：{id}")));
    }
    let _guard = crate::console_cli::modes_write_lock();
    let mut file = crate::orchestration::presets_from_strict(root).map_err(|message| (1, message))?;
    let Some(position) = file.presets.iter().position(|preset| preset.id == id) else {
        return Err((2, format!("未找到预设：{id}")));
    };
    if file.presets[position].builtin {
        return Err((2, format!("内置模式不可删除：{id}")));
    }
    let name = file.presets[position].name.clone();
    let mut modes = crate::console_cli::strict_modes_for_write(root)?;
    let mut seats_hosts: Vec<String> = Vec::new();
    let mut default_hosts: Vec<String> = Vec::new();
    let mut slash_targets: Vec<(String, String)> = Vec::new();
    let mut slash_invalid: Vec<Value> = Vec::new();
    let mut changed = false;
    for (host_id, section) in modes.hosts.iter_mut() {
        if section.seats.shift_remove(id).is_some() {
            seats_hosts.push(host_id.clone());
            changed = true;
        }
        if section.default_mode.trim() == id {
            section.default_mode.clear();
            default_hosts.push(host_id.clone());
            changed = true;
        }
        let names: Vec<String> = section
            .slash_commands
            .iter()
            .filter(|command| command.mode == id)
            .map(|command| command.name.clone())
            .collect();
        if !names.is_empty() {
            section
                .slash_commands
                .retain(|command| command.mode != id);
            for command_name in names {
                // host 或名称不合法的定义只从配置里移除，不拼路径不碰磁盘。
                if crate::slash::validate_host(host_id).is_ok()
                    && crate::slash::validate_name(&command_name).is_ok()
                {
                    slash_targets.push((host_id.clone(), command_name));
                } else {
                    slash_invalid.push(json!({ "host": host_id, "name": command_name }));
                }
            }
            changed = true;
        }
    }
    let mut slash_removed: Vec<Value> = Vec::new();
    for (index, (host_id, command_name)) in slash_targets.iter().enumerate() {
        match crate::slash::remove_marked_command_file(home, host_id, command_name) {
            Ok(_) => slash_removed.push(json!({ "host": host_id, "name": command_name })),
            Err(message) => {
                let done: Vec<(String, String)> = slash_targets[..index].to_vec();
                let left: Vec<(String, String)> = slash_targets[index..].to_vec();
                return Err((
                    1,
                    format!(
                        "{message}；modes.json 与 presets.json 未改动；已删：{}；未删：{}",
                        host_name_entries(&done),
                        host_name_entries(&left)
                    ),
                ));
            }
        }
    }
    if changed {
        if let Err(message) = crate::orchestration::save_modes_in(root, &modes) {
            return Err((
                1,
                format!("{message}；短命令文件已删除，modes.json 未更新"),
            ));
        }
    }
    file.presets.remove(position);
    if let Err(message) = crate::orchestration::save_presets_in(root, &file) {
        return Err((
            1,
            format!("{message}；modes.json 与短命令文件已清理，预设仍在"),
        ));
    }
    Ok(json!({
        "removed": { "id": id, "name": name },
        "affected": {
            "seats": seats_hosts,
            "defaultMode": default_hosts,
            "slash": slash_removed,
            "invalidSlash": slash_invalid,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::{
        host_section, host_section_mut, modes_from, raw_presets_from, save_modes_in,
        PresetFile, RoleSettings, SeatChoice, SlashCommand,
    };
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = crate::paths::test_temp_dir().join(format!("t55-{tag}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 单阶段单位置的输入预设。
    fn simple_input(name: &str) -> PresetInput {
        serde_json::from_value(json!({
            "name": name,
            "description": "一句话介绍",
            "stages": [
                { "nodes": [ { "role": "执行", "visibility": "共享", "write": "单写者", "thinking": "中" } ] }
            ]
        }))
        .unwrap()
    }

    #[test]
    fn task55_builtin_presets_carry_descriptions() {
        let descriptions = [
            ("builtin-shared", "单个模型执行，可读取完整对话上下文；只读"),
            ("builtin-economy", "高档模型出方案，低档模型执行，其他厂商的模型验收"),
            ("builtin-blind", "同一份材料由多个厂商的模型独立审查，结果互不可见"),
            ("builtin-arena", "多个模型分别完成同一任务，从结果中选定一份采纳"),
            ("builtin-jury", "多个模型分别从安全、性能、需求覆盖角度审查"),
            ("builtin-swarm", "任务拆分为互不重叠的子任务，由多个模型并行完成后合并"),
        ];
        let presets = crate::orchestration::builtin_presets();
        assert_eq!(presets.len(), descriptions.len());
        for (id, description) in descriptions {
            let preset = presets.iter().find(|preset| preset.id == id).unwrap();
            assert_eq!(preset.description, description, "{id}");
        }
    }

    /// config show 的 modes[] 与 slash list 的 modes[] 都带介绍。
    #[test]
    fn task55_description_shows_in_config_and_slash_list() {
        let root = temp_dir("show-root");
        let home = temp_dir("show-home");
        let (_, modes) = crate::console_cli::modes_section(&root, None);
        let shared = modes
            .iter()
            .find(|mode| mode["id"] == "builtin-shared")
            .unwrap();
        assert_eq!(shared["description"], "单个模型执行，可读取完整对话上下文；只读");
        let value = crate::slash::slash_list_at(&root, &home, "claude").unwrap();
        let blind = value["modes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|mode| mode["mode"] == "builtin-blind")
            .unwrap();
        assert_eq!(blind["description"], "同一份材料由多个厂商的模型独立审查，结果互不可见");
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// 新建（生成 uuid）、更新（同 id）与删除；输出里的 preset 与 modes
    /// 都能读到保存后的内容，介绍为空时不写出。
    #[test]
    fn task55_save_creates_updates_and_removes() {
        let root = temp_dir("crud-root");
        // 新建：无 id，生成 uuid。
        let value = preset_save_at(&root, None, &simple_input("我的模式")).unwrap();
        let preset = &value["preset"];
        assert_eq!(preset["name"], "我的模式");
        assert_eq!(preset["description"], "一句话介绍");
        assert_eq!(preset["builtin"], false);
        let id = preset["id"].as_str().unwrap().to_string();
        assert_eq!(id.len(), 36, "生成的 id 应是 uuid：{id}");
        assert_eq!(value["modes"].as_array().unwrap().len(), 7);
        assert_eq!(raw_presets_from(&root).presets.len(), 7);
        // presets.json 里落盘的形状：id 与介绍在，builtin 为 false 落盘。
        let text = std::fs::read_to_string(root.join("presets.json")).unwrap();
        assert!(text.contains(&id) && text.contains("一句话介绍"), "{text}");
        // 更新：同 id 改名，介绍留空后不写出。
        let update: PresetInput = serde_json::from_value(json!({
            "id": id,
            "name": "改名模式",
            "stages": [
                { "nodes": [ { "role": "执行", "visibility": "盲", "write": "只读" } ] }
            ]
        }))
        .unwrap();
        let value = preset_save_at(&root, None, &update).unwrap();
        assert_eq!(value["preset"]["id"], id.as_str());
        assert_eq!(value["preset"]["name"], "改名模式");
        assert!(value["preset"]["description"].is_null(), "{value}");
        let presets = raw_presets_from(&root).presets;
        assert_eq!(presets.len(), 7, "更新不新增条目");
        assert_eq!(presets.last().unwrap().name, "改名模式");
        assert!(presets.last().unwrap().description.is_empty());
        // 删除后回到内置 6 条。
        let value = preset_remove_at(&root, &temp_dir("crud-home"), &id).unwrap();
        assert_eq!(value["removed"]["id"], id.as_str());
        assert_eq!(value["removed"]["name"], "改名模式");
        assert_eq!(raw_presets_from(&root).presets.len(), 6);
        let _ = std::fs::remove_dir_all(root);
    }

    /// 内置预设不可修改、不可删除（含手写进 presets.json 的 builtin 条目）；
    /// 不存在的预设删除退出 2。
    #[test]
    fn task55_builtin_presets_are_protected() {
        let root = temp_dir("protect-root");
        // 内置 id 不可作更新目标。
        let error = preset_save_at(
            &root,
            None,
            &serde_json::from_value(json!({
                "id": "builtin-shared",
                "name": "改掉它",
                "stages": [ { "nodes": [ { "role": "执行", "visibility": "共享", "write": "只读" } ] } ]
            }))
            .unwrap(),
        )
        .unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("内置模式不可修改"), "{}", error.1);
        // 内置不可删。
        let error = preset_remove_at(&root, &temp_dir("protect-home"), "builtin-blind").unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("内置模式不可删除"), "{}", error.1);
        // 手写进文件的 builtin 条目同样受保护。
        let file = PresetFile {
            version: 1,
            presets: vec![Preset {
                id: "handmade".into(),
                name: "手工内置".into(),
                description: String::new(),
                builtin: true,
                stages: Vec::new(),
            }],
        };
        crate::orchestration::save_presets_in(&root, &file).unwrap();
        let error = preset_remove_at(&root, &temp_dir("protect-home"), "handmade").unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("内置模式不可删除"), "{}", error.1);
        // 不存在的预设退出 2。
        let error = preset_remove_at(&root, &temp_dir("protect-home"), "nope").unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("未找到预设"), "{}", error.1);
        let _ = std::fs::remove_dir_all(root);
    }

    /// 模式名规则：非空、≤40 字、不与其它预设（含内置）重名；更新时与
    /// 自己同名不算重名。
    #[test]
    fn task55_name_rules() {
        let root = temp_dir("name-root");
        let error = preset_save_at(&root, None, &simple_input("  ")).unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("模式名不能为空"), "{}", error.1);
        let error = preset_save_at(&root, None, &simple_input(&"长".repeat(41))).unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("40"), "{}", error.1);
        // 与内置重名。
        let error = preset_save_at(&root, None, &simple_input("共享模式")).unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("重名"), "{}", error.1);
        // 与其它用户预设重名。
        preset_save_at(&root, None, &simple_input("甲模式")).unwrap();
        let error = preset_save_at(&root, None, &simple_input("甲模式")).unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("重名"), "{}", error.1);
        // 更新时名字不变（与自己重名）合法。
        let id = raw_presets_from(&root)
            .presets
            .last()
            .unwrap()
            .id
            .clone();
        let update: PresetInput = serde_json::from_value(json!({
            "id": id,
            "name": "甲模式",
            "stages": [ { "nodes": [ { "role": "执行", "visibility": "共享", "write": "只读" } ] } ]
        }))
        .unwrap();
        preset_save_at(&root, None, &update).unwrap();
        // 刚好 40 字合法。
        preset_save_at(&root, None, &simple_input(&"字".repeat(40))).unwrap();
        let _ = std::fs::remove_dir_all(root);
    }

    /// 字段校验：role / visibility / write / thinking 只认列出的值，原因
    /// 写明位置与字段；传入的 agent / model 等人选字段忽略并清空。
    #[test]
    fn task55_field_validation_and_ignored_seat_fields() {
        let root = temp_dir("field-root");
        let node = |overrides: serde_json::Value| {
            let mut object = serde_json::Map::new();
            object.insert("role".into(), json!("执行"));
            object.insert("visibility".into(), json!("共享"));
            object.insert("write".into(), json!("只读"));
            if let Some(map) = overrides.as_object() {
                for (key, value) in map {
                    object.insert(key.clone(), value.clone());
                }
            }
            json!({ "stages": [ { "nodes": [ Value::Object(object) ] } ], "name": "字段模式" })
        };
        for (overrides, field) in [
            (json!({ "role": "顾问" }), "role"),
            (json!({ "role": "" }), "role"),
            (json!({ "visibility": "指定上游" }), "visibility"),
            (json!({ "write": "单写" }), "write"),
            (json!({ "thinking": "极高" }), "thinking"),
        ] {
            let input: PresetInput = serde_json::from_value(node(overrides)).unwrap();
            let error = preset_save_at(&root, None, &input).unwrap_err();
            assert_eq!(error.0, 2, "{field}: {}", error.1);
            assert!(error.1.contains(field), "{field}: {}", error.1);
            assert!(error.1.contains("第 1 个阶段第 1 个位置"), "{}", error.1);
        }
        // 空串 thinking 合法（沿用预设默认）。
        let ok: PresetInput = serde_json::from_value(node(json!({ "thinking": "" }))).unwrap();
        let value = preset_save_at(&root, None, &ok).unwrap();
        assert_eq!(value["preset"]["stages"][0]["nodes"][0]["thinking"], "");
        // 人选字段忽略：agent / model / tier / fallback 全部清空。
        let input: PresetInput = serde_json::from_value(json!({
            "name": "带人选",
            "stages": [ { "nodes": [ {
                "role": "执行", "visibility": "共享", "write": "只读", "thinking": "中",
                "agent": "claude", "model": "should-be-ignored", "provider": "x",
                "tier": "高", "fallback": { "agent": "a", "model": "b" }
            } ] } ]
        }))
        .unwrap();
        let value = preset_save_at(&root, None, &input).unwrap();
        let saved = &value["preset"]["stages"][0]["nodes"][0];
        assert_eq!(saved["role"], "执行");
        assert_eq!(saved["agent"], "");
        assert_eq!(saved["model"], "");
        assert!(saved["tier"].is_null());
        assert!(saved["fallback"].is_null());
        let _ = std::fs::remove_dir_all(root);
    }

    /// 同一阶段最多一个「单写者」；不同阶段各一个合法。同阶段两个单写者
    /// 退出 2。
    #[test]
    fn task55_single_writer_per_stage() {
        let root = temp_dir("writer-root");
        let input: PresetInput = serde_json::from_value(json!({
            "name": "两写者",
            "stages": [ { "nodes": [
                { "role": "执行", "visibility": "共享", "write": "单写者" },
                { "role": "执行", "visibility": "共享", "write": "单写者" }
            ] } ]
        }))
        .unwrap();
        let error = preset_save_at(&root, None, &input).unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("单写者"), "{}", error.1);
        // 不同阶段各一个合法。
        let input: PresetInput = serde_json::from_value(json!({
            "name": "两阶段写",
            "stages": [
                { "nodes": [ { "role": "执行", "visibility": "共享", "write": "单写者" } ] },
                { "nodes": [ { "role": "验收", "visibility": "盲", "write": "单写者" } ] }
            ]
        }))
        .unwrap();
        preset_save_at(&root, None, &input).unwrap();
        let _ = std::fs::remove_dir_all(root);
    }

    /// 位置数上限：全模式 13 个退出 2，12 个合法；空阶段与零位置退出 2。
    #[test]
    fn task55_node_limit() {
        let root = temp_dir("limit-root");
        let nodes = |count: usize| {
            (0..count)
                .map(|_| json!({ "role": "执行", "visibility": "共享", "write": "只读" }))
                .collect::<Vec<_>>()
        };
        let input: PresetInput = serde_json::from_value(json!({
            "name": "十三位置",
            "stages": [ { "nodes": nodes(13) } ]
        }))
        .unwrap();
        let error = preset_save_at(&root, None, &input).unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("12"), "{}", error.1);
        // 分两个阶段凑 12 个合法。
        let input: PresetInput = serde_json::from_value(json!({
            "name": "十二位置",
            "stages": [ { "nodes": nodes(11) }, { "nodes": nodes(1) } ]
        }))
        .unwrap();
        preset_save_at(&root, None, &input).unwrap();
        // 零位置（阶段数组为空）退出 2。
        let input: PresetInput =
            serde_json::from_value(json!({ "name": "零阶段", "stages": [] })).unwrap();
        let error = preset_save_at(&root, None, &input).unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("至少需要一个阶段"), "{}", error.1);
        // 阶段存在但没有位置退出 2。
        let input: PresetInput =
            serde_json::from_value(json!({ "name": "空阶段", "stages": [ { "nodes": [] } ] }))
                .unwrap();
        let error = preset_save_at(&root, None, &input).unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("至少要有一个位置"), "{}", error.1);
        let _ = std::fs::remove_dir_all(root);
    }

    /// 删除的级联清理：两个主 agent 下的人选、指向它的默认模式、两个主
    /// agent 的短命令定义与带标记文件都清掉；presets.json 里的预设删掉。
    #[test]
    fn task55_remove_cascades_seats_default_and_slash() {
        let root = temp_dir("cascade-root");
        let home = temp_dir("cascade-home");
        let value = preset_save_at(&root, None, &simple_input("待删模式")).unwrap();
        let id = value["preset"]["id"].as_str().unwrap().to_string();
        // claude：人选 + 默认模式 + 短命令；pi：只有人选；kimi：短命令。
        let mut modes = modes_from(&root);
        let claude = host_section_mut(&mut modes, "claude");
        claude.default_mode = id.clone();
        claude.seats.insert(
            id.clone(),
            vec![vec![Some(SeatChoice {
                agent: "claude".into(),
                model: "sonnet".into(),
                ..SeatChoice::default()
            })]],
        );
        let pi = host_section_mut(&mut modes, "pi");
        pi.seats.insert(
            id.clone(),
            vec![vec![None]],
        );
        save_modes_in(&root, &modes).unwrap();
        // 短命令走 add 的同一份写入（定义 + 带标记文件）。
        crate::slash::slash_add_at(&root, &home, "claude", "u1-cmd", &id, "").unwrap();
        crate::slash::slash_add_at(&root, &home, "kimi", "u1-kimi", &id, "").unwrap();
        // 内置模式的短命令不受影响。
        crate::slash::slash_add_at(&root, &home, "claude", "keep-blind", "blind", "").unwrap();
        let claude_file = crate::slash::command_path(&home, "claude", "u1-cmd");
        let kimi_file = crate::slash::command_path(&home, "kimi", "u1-kimi");
        let keep_file = crate::slash::command_path(&home, "claude", "keep-blind");
        assert!(claude_file.is_file() && kimi_file.is_file() && keep_file.is_file());
        let value = preset_remove_at(&root, &home, &id).unwrap();
        // 受影响项列表。
        let seats: Vec<String> = value["affected"]["seats"]
            .as_array()
            .unwrap()
            .iter()
            .map(|host| host.as_str().unwrap().to_string())
            .collect();
        assert!(seats.contains(&"claude".to_string()) && seats.contains(&"pi".to_string()), "{seats:?}");
        assert_eq!(
            value["affected"]["defaultMode"].as_array().unwrap().len(),
            1
        );
        assert_eq!(value["affected"]["defaultMode"][0], "claude");
        let slash: Vec<String> = value["affected"]["slash"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| format!("{}/{}", item["host"].as_str().unwrap(), item["name"].as_str().unwrap()))
            .collect();
        assert_eq!(slash, ["claude/u1-cmd", "kimi/u1-kimi"], "{slash:?}");
        // 磁盘与文件：预设没了，人选、默认模式与定义清掉，文件删掉，
        // 别的模式（内置）的短命令原样。
        assert!(
            !raw_presets_from(&root).presets.iter().any(|preset| preset.id == id),
            "预设应已删除"
        );
        assert!(!claude_file.exists());
        assert!(!kimi_file.exists());
        assert!(!kimi_file.parent().unwrap().exists(), "kimi 空目录一起删");
        assert!(keep_file.is_file());
        let modes = modes_from(&root);
        let claude = host_section(&modes, "claude").unwrap();
        assert!(claude.default_mode.is_empty());
        assert!(!claude.seats.contains_key(&id));
        assert_eq!(claude.slash_commands.len(), 1);
        assert_eq!(claude.slash_commands[0].name, "keep-blind");
        let pi = host_section(&modes, "pi").unwrap();
        assert!(!pi.seats.contains_key(&id));
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// 删除的顺序是先逐个删带标记的文件、全部成功后再写 modes.json、
    /// 最后删预设：第二个文件删失败时停止，错误说明列出已删与剩下的
    /// 条目，modes.json 与 presets.json 都不动；再删一次走完全部清理。
    #[test]
    fn task55_remove_file_failure_stops_before_modes_write() {
        let root = temp_dir("order-root");
        let home = temp_dir("order-home");
        let value = preset_save_at(&root, None, &simple_input("顺序模式")).unwrap();
        let id = value["preset"]["id"].as_str().unwrap().to_string();
        let mut modes = modes_from(&root);
        host_section_mut(&mut modes, "claude")
            .seats
            .insert(id.clone(), vec![vec![None]]);
        host_section_mut(&mut modes, "pi")
            .seats
            .insert(id.clone(), vec![vec![None]]);
        save_modes_in(&root, &modes).unwrap();
        crate::slash::slash_add_at(&root, &home, "claude", "order-a", &id, "").unwrap();
        crate::slash::slash_add_at(&root, &home, "kimi", "order-b", &id, "").unwrap();
        let claude_file = crate::slash::command_path(&home, "claude", "order-a");
        let kimi_file = crate::slash::command_path(&home, "kimi", "order-b");
        // 注入第 2 次删文件失败：claude 的已删，kimi 的没删。
        crate::slash::FAIL_REMOVE_FILE.with(|flag| flag.set(2));
        let error = preset_remove_at(&root, &home, &id).unwrap_err();
        assert_eq!(error.0, 1);
        assert!(
            error.1.contains("已删：claude/order-a") && error.1.contains("未删：kimi/order-b"),
            "{}",
            error.1
        );
        // 磁盘：第一个文件已删，第二个还在；配置与预设两边都没动。
        assert!(!claude_file.exists());
        assert!(kimi_file.is_file());
        let modes = modes_from(&root);
        assert!(host_section(&modes, "claude").unwrap().seats.contains_key(&id));
        assert_eq!(
            host_section(&modes, "kimi").unwrap().slash_commands[0].name,
            "order-b"
        );
        assert!(raw_presets_from(&root)
            .presets
            .iter()
            .any(|preset| preset.id == id));
        // 注入已清，再删一次：剩余的文件、配置与预设全部清理。
        let value = preset_remove_at(&root, &home, &id).unwrap();
        assert_eq!(value["affected"]["invalidSlash"].as_array().unwrap().len(), 0);
        assert!(!kimi_file.exists());
        let modes = modes_from(&root);
        assert!(!host_section(&modes, "claude").unwrap().seats.contains_key(&id));
        assert!(!raw_presets_from(&root)
            .presets
            .iter()
            .any(|preset| preset.id == id));
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// modes.json 里手改出的坏定义（host 或名称不合法）只从定义里移除，
    /// 不碰磁盘、不触发断言，结果里单独标出；好定义照常删文件。
    #[test]
    fn task55_remove_drops_invalid_slash_definitions_without_disk() {
        let root = temp_dir("invaliddef-root");
        let home = temp_dir("invaliddef-home");
        let value = preset_save_at(&root, None, &simple_input("坏定义模式")).unwrap();
        let id = value["preset"]["id"].as_str().unwrap().to_string();
        // 先写两个好定义（带标记文件落盘），再手改 modes.json 追加坏定义。
        crate::slash::slash_add_at(&root, &home, "claude", "good-name", &id, "").unwrap();
        crate::slash::slash_add_at(&root, &home, "claude", "other", "builtin-blind", "").unwrap();
        let mut modes = modes_from(&root);
        host_section_mut(&mut modes, "ghost-host")
            .slash_commands
            .push(SlashCommand {
                name: "ok-name".into(),
                mode: id.clone(),
                note: String::new(),
            });
        let claude = host_section_mut(&mut modes, "claude");
        claude.slash_commands.push(SlashCommand {
            name: "坏名".into(),
            mode: id.clone(),
            note: String::new(),
        });
        save_modes_in(&root, &modes).unwrap();
        let good_file = crate::slash::command_path(&home, "claude", "good-name");
        assert!(good_file.is_file());
        let value = preset_remove_at(&root, &home, &id).unwrap();
        let invalid = value["affected"]["invalidSlash"].as_array().unwrap();
        assert_eq!(invalid.len(), 2, "{invalid:?}");
        assert!(
            invalid
                .iter()
                .any(|item| item["host"] == "ghost-host" && item["name"] == "ok-name"),
            "{invalid:?}"
        );
        assert!(
            invalid
                .iter()
                .any(|item| item["host"] == "claude" && item["name"] == "坏名"),
            "{invalid:?}"
        );
        // 好定义的文件删了；别模式的定义与文件都不动；坏定义不再出现。
        assert!(!good_file.exists());
        let modes = modes_from(&root);
        let claude = host_section(&modes, "claude").unwrap();
        assert_eq!(claude.slash_commands.len(), 1);
        assert_eq!(claude.slash_commands[0].name, "other");
        assert!(host_section(&modes, "ghost-host")
            .unwrap()
            .slash_commands
            .is_empty());
        assert!(crate::slash::command_path(&home, "claude", "other").is_file());
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// 派发能用新建的自定义预设：引擎按 id 取到它，补人照常——有人选的
    /// 位置来源「位置」，没有的走「未指定」（角色默认为空时）。
    #[test]
    fn task55_saved_preset_dispatches_with_seat_fill() {
        let root = temp_dir("dispatch-root");
        let input: PresetInput = serde_json::from_value(json!({
            "name": "派发模式",
            "stages": [ { "nodes": [
                { "role": "执行", "visibility": "共享", "write": "单写者" },
                { "role": "验收", "visibility": "盲", "write": "只读" }
            ] } ]
        }))
        .unwrap();
        let value = preset_save_at(&root, None, &input).unwrap();
        let id = value["preset"]["id"].as_str().unwrap().to_string();
        // 引擎与派发同一份查找：raw_presets_from 里按 id 取。
        let presets = raw_presets_from(&root);
        let preset = presets
            .presets
            .iter()
            .find(|preset| preset.id == id)
            .unwrap()
            .clone();
        // claude 的第一个位置有人选：来源「位置」；第二个位置没有：未指定。
        let mut modes = modes_from(&root);
        host_section_mut(&mut modes, "claude").seats.insert(
            id,
            vec![vec![
                Some(SeatChoice {
                    agent: "pi".into(),
                    provider: Some("zai".into()),
                    model: "glm-4.7".into(),
                    ..SeatChoice::default()
                }),
                None,
            ]],
        );
        save_modes_in(&root, &modes).unwrap();
        let modes = modes_from(&root);
        let seats = host_section(&modes, "claude").unwrap().seats.get(&preset.id);
        let (filled, sources) = crate::engine::fill_seats(&preset, &RoleSettings::default(), seats);
        assert_eq!(sources[0][0], "位置");
        assert_eq!(filled.stages[0].nodes[0].agent, "pi");
        assert_eq!(filled.stages[0].nodes[0].model, "glm-4.7");
        assert_eq!(sources[0][1], "未指定");
        assert_eq!(filled.stages[0].nodes[1].model, "");
        let _ = std::fs::remove_dir_all(root);
    }

    /// presets.json 损坏或版本不认时 save / remove 退出 1、不覆盖。
    #[test]
    fn task55_corrupt_presets_json_blocks_writes() {
        let root = temp_dir("corrupt-root");
        std::fs::write(root.join("presets.json"), "{ not json").unwrap();
        let error = preset_save_at(&root, None, &simple_input("写不进")).unwrap_err();
        assert_eq!(error.0, 1);
        assert!(error.1.contains("presets.json"), "{}", error.1);
        let error = preset_remove_at(&root, &temp_dir("corrupt-home"), "u1").unwrap_err();
        assert_eq!(error.0, 1);
        assert!(error.1.contains("presets.json"), "{}", error.1);
        // 版本不认同样拒写。
        std::fs::write(
            root.join("presets.json"),
            r#"{"version":2,"presets":[]}"#,
        )
        .unwrap();
        let error = preset_save_at(&root, None, &simple_input("写不进")).unwrap_err();
        assert_eq!(error.0, 1);
        let _ = std::fs::remove_dir_all(root);
    }

    /// 非法 id 退出 2；空 id 视为新建。
    #[test]
    fn task55_preset_id_validation() {
        let root = temp_dir("id-root");
        for bad in ["带 空格", "点.", "../evil", &"a".repeat(65)] {
            let input: PresetInput = serde_json::from_value(json!({
                "id": bad,
                "name": "ID 模式",
                "stages": [ { "nodes": [ { "role": "执行", "visibility": "共享", "write": "只读" } ] } ]
            }))
            .unwrap();
            let error = preset_save_at(&root, None, &input).unwrap_err();
            assert_eq!(error.0, 2, "{bad}");
            assert!(error.1.contains("预设 id"), "{bad}: {}", error.1);
        }
        // 合法 id 但不存在：退出 2，不静默新建。
        let input: PresetInput = serde_json::from_value(json!({
            "id": "my-mode_1",
            "name": "ID 模式",
            "stages": [ { "nodes": [ { "role": "执行", "visibility": "共享", "write": "只读" } ] } ]
        }))
        .unwrap();
        let error = preset_save_at(&root, None, &input).unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("模式不存在：my-mode_1"), "{}", error.1);
        // 先无 id 新建，再按 id 更新。
        let value = preset_save_at(&root, None, &simple_input("ID 模式")).unwrap();
        let id = value["preset"]["id"].as_str().unwrap().to_string();
        let update: PresetInput = serde_json::from_value(json!({
            "id": id,
            "name": "ID 模式",
            "description": "改过的介绍",
            "stages": [ { "nodes": [ { "role": "执行", "visibility": "共享", "write": "只读" } ] } ]
        }))
        .unwrap();
        let value = preset_save_at(&root, None, &update).unwrap();
        assert_eq!(value["preset"]["id"], id.as_str());
        let _ = std::fs::remove_dir_all(root);
    }

    /// 内置模式的别名不可作用户预设 id（别名解析永远先命中内置预设，
    /// 同名用户预设轮不到被解析）。
    #[test]
    fn task55_alias_ids_are_rejected() {
        let root = temp_dir("alias-root");
        for alias in ["shared", "economy", "blind"] {
            let input: PresetInput = serde_json::from_value(json!({
                "id": alias,
                "name": "别名模式",
                "stages": [ { "nodes": [ { "role": "执行", "visibility": "共享", "write": "只读" } ] } ]
            }))
            .unwrap();
            let error = preset_save_at(&root, None, &input).unwrap_err();
            assert_eq!(error.0, 2, "{alias}");
            assert!(error.1.contains("别名"), "{alias}: {}", error.1);
        }
        assert!(!root.join("presets.json").exists(), "被拒的保存不应写盘");
        let _ = std::fs::remove_dir_all(root);
    }

    /// 介绍与模式名的字符规则：介绍最长 200 字；换行、制表以外的控制
    /// 字符与 ANSI 转义（ESC 开头）都退出 2；换行与制表可以原样保存。
    #[test]
    fn task55_description_and_name_character_rules() {
        let root = temp_dir("char-root");
        let input_with = |name: &str, description: &str| {
            serde_json::from_value::<PresetInput>(json!({
                "name": name,
                "description": description,
                "stages": [ { "nodes": [ { "role": "执行", "visibility": "共享", "write": "只读" } ] } ]
            }))
            .unwrap()
        };
        // 介绍 201 字退出 2，200 字合法。
        let error =
            preset_save_at(&root, None, &input_with("介绍模式", &"长".repeat(201))).unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("200"), "{}", error.1);
        preset_save_at(&root, None, &input_with("介绍模式", &"字".repeat(200))).unwrap();
        // 控制字符与 ANSI 转义：名字与介绍同一套规则。
        for bad_name in ["带\u{7}响铃", "\u{1b}[31m红"] {
            let error = preset_save_at(&root, None, &input_with(bad_name, "")).unwrap_err();
            assert_eq!(error.0, 2, "{bad_name:?}");
            assert!(error.1.contains("模式名"), "{}", error.1);
        }
        for bad_description in ["带\u{7}响铃", "带\u{b}垂直制表", "\u{1b}[31m红"] {
            let error =
                preset_save_at(&root, None, &input_with("另一模式", bad_description)).unwrap_err();
            assert_eq!(error.0, 2, "{bad_description:?}");
            assert!(error.1.contains("介绍"), "{}", error.1);
        }
        // 换行与制表可以用，保存原样。
        let value =
            preset_save_at(&root, None, &input_with("换行模式", "第一行\n第二行\t缩进")).unwrap();
        assert_eq!(value["preset"]["description"], "第一行\n第二行\t缩进");
        let _ = std::fs::remove_dir_all(root);
    }

    /// 更新时阶段数、每阶段位置数或任一位置的角色变了就清各主 agent 下
    /// 该模式的人选（短命令与默认模式保留），输出列出受影响的主 agent；
    /// 只改介绍不动人选。
    #[test]
    fn task55_save_update_clears_seats_when_shape_changes() {
        let root = temp_dir("shape-root");
        let home = temp_dir("shape-home");
        let value = preset_save_at(&root, None, &simple_input("变形模式")).unwrap();
        let id = value["preset"]["id"].as_str().unwrap().to_string();
        let mut modes = modes_from(&root);
        let claude = host_section_mut(&mut modes, "claude");
        claude.default_mode = id.clone();
        claude.seats.insert(
            id.clone(),
            vec![vec![Some(SeatChoice {
                agent: "claude".into(),
                model: "sonnet".into(),
                ..SeatChoice::default()
            })]],
        );
        save_modes_in(&root, &modes).unwrap();
        crate::slash::slash_add_at(&root, &home, "claude", "shape-cmd", &id, "").unwrap();

        // 只改介绍：形状没变，人选保留，seatsCleared 为空。
        let update: PresetInput = serde_json::from_value(json!({
            "id": id,
            "name": "变形模式",
            "description": "新介绍",
            "stages": [ { "nodes": [ { "role": "执行", "visibility": "共享", "write": "单写者", "thinking": "中" } ] } ]
        }))
        .unwrap();
        let value = preset_save_at(&root, None, &update).unwrap();
        assert_eq!(value["seatsCleared"].as_array().unwrap().len(), 0);
        assert!(
            host_section(&modes_from(&root), "claude")
                .unwrap()
                .seats
                .contains_key(&id),
            "形状没变时人选保留"
        );

        // 改角色：人选清掉，默认模式与短命令保留。
        let update: PresetInput = serde_json::from_value(json!({
            "id": id,
            "name": "变形模式",
            "description": "新介绍",
            "stages": [ { "nodes": [ { "role": "验收", "visibility": "盲", "write": "只读" } ] } ]
        }))
        .unwrap();
        let value = preset_save_at(&root, None, &update).unwrap();
        let cleared: Vec<String> = value["seatsCleared"]
            .as_array()
            .unwrap()
            .iter()
            .map(|host| host.as_str().unwrap().to_string())
            .collect();
        assert_eq!(cleared, ["claude"]);
        let modes = modes_from(&root);
        let claude = host_section(&modes, "claude").unwrap();
        assert!(!claude.seats.contains_key(&id), "角色变了人选应清掉");
        assert_eq!(claude.default_mode, id, "默认模式保留");
        assert_eq!(claude.slash_commands.len(), 1, "短命令保留");
        assert_eq!(claude.slash_commands[0].name, "shape-cmd");

        // 改阶段数：同样清。
        let update: PresetInput = serde_json::from_value(json!({
            "id": id,
            "name": "变形模式",
            "stages": [
                { "nodes": [ { "role": "执行", "visibility": "共享", "write": "单写者" } ] },
                { "nodes": [ { "role": "验收", "visibility": "盲", "write": "只读" } ] }
            ]
        }))
        .unwrap();
        preset_save_at(&root, None, &update).unwrap();
        let mut modes = modes_from(&root);
        let claude = host_section_mut(&mut modes, "claude");
        claude
            .seats
            .insert(id.clone(), vec![vec![None], vec![None]]);
        save_modes_in(&root, &modes).unwrap();
        let update: PresetInput = serde_json::from_value(json!({
            "id": id,
            "name": "变形模式",
            "stages": [ { "nodes": [ { "role": "执行", "visibility": "共享", "write": "单写者" } ] } ]
        }))
        .unwrap();
        let value = preset_save_at(&root, None, &update).unwrap();
        assert_eq!(value["seatsCleared"][0], "claude");
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }
}
