//! 自定义短命令（TASK-53）：把「按某个模式派发」写成主 agent 认得的斜杠命令。
//!
//! 定义存在 modes.json 的 `hosts.<主 agent>.slashCommands`；文件按各家格式
//! 写进家目录：claude `~/.claude/commands/<名>.md`、codex
//! `~/.codex/prompts/<名>.md`、pi `~/.pi/agent/prompts/<名>.md`、kimi
//! `~/.kimi-code/skills/<名>/SKILL.md`（命令即 skill；kimi 还读
//! `~/.agents/skills`，那里已有同名目录时视为被占用）。我们写的文件在
//! frontmatter 里带顶格的 `aicross-managed: true`；claude 与 kimi 另带
//! `disable-model-invocation: true`，模型不会自己触发（TASK-59）。同名但
//! 无标记的文件是别人的，不覆盖、不删除。`ai-cross` 与 Windows 设备名是保留名。写入用
//! 临时文件加 rename：add 先写文件再存配置，存配置失败回滚文件（新建的
//! 删掉，更新的恢复原内容）；remove 先删带标记的文件再存定义，删文件
//! 失败配置不动。modes.json 的定义更新与文件写删放在同一把进程内锁
//! （`modes_write_lock`）里，与其它写 modes.json 的命令互不踩。host 必须
//! 是 claude / codex / pi / kimi 之一。家目录一律由调用方注入：生产用
//! `paths::home()`，测试一律用临时目录，绝不写真实家目录。

use crate::orchestration::{self, SlashCommand};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// list 输出的文件状态（state 按磁盘实际判断）。
pub(crate) const STATE_WRITTEN: &str = "已写入";
pub(crate) const STATE_ABSENT: &str = "未写入";
pub(crate) const STATE_OCCUPIED: &str = "被占用";
/// list 输出的第四种状态：modes.json 里的名称不合法（不读磁盘）。
pub(crate) const STATE_INVALID_NAME: &str = "名称无效";

/// frontmatter 里的归属标记。
const MANAGED_MARKER: &str = "aicross-managed";

/// 保留名：`ai-cross` 与 skill 本身冲突（claude 的 `/ai-cross`，kimi 读
/// `~/.agents/skills` 与 `~/.kimi-code/skills`）；其余是 Windows 设备名，
/// 作文件名会被系统占用。
const RESERVED_NAMES: [&str; 23] = [
    "ai-cross", "con", "nul", "aux", "prn",
    "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9",
    "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// 内置模式的短名与关键词（TASK-54）：每行是预设 id、短名（slug）与带
/// 参数写法里的关键词。list 输出的 `modes` 与 add-all 都按这张表的顺序；
/// 用户自定义预设没有稳定短名，不参与 add-all。
const MODE_SHORTNAMES: [(&str, &str, &str); 6] = [
    ("builtin-shared", "shared", "共享"),
    ("builtin-economy", "economy", "经济"),
    ("builtin-blind", "blind", "盲审"),
    ("builtin-arena", "arena", "擂台"),
    ("builtin-jury", "jury", "评审"),
    ("builtin-swarm", "swarm", "蜂群"),
];

/// add-all 的默认前缀。
pub(crate) const DEFAULT_PREFIX: &str = "ax-";

// 测试注入：非 0 时，第 n 次删带标记的文件模拟成失败（用后即清，每删
// 一次减一）；存 modes.json 注入不变。用来验证「删文件失败不改配置」
// 「存配置失败回滚文件」，以及 preset remove 级联删到一半失败的用例。
// 只在测试构建里存在，每个测试线程各自一份，互不影响。
#[cfg(test)]
std::thread_local! {
    pub(crate) static FAIL_REMOVE_FILE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static FAIL_SAVE_MODES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// 默认调用写法（调用 ai-cross skill 本身，各家写法不同）。
fn invoke_hint(host: &str) -> &'static str {
    match host {
        "claude" => "/ai-cross",
        "codex" => "$ai-cross",
        "pi" | "kimi" => "/skill:ai-cross",
        _ => unreachable!("host 已由 validate_host 校验"),
    }
}

/// 该 host 下这个短命令的输入写法。
fn trigger_hint(host: &str, name: &str) -> String {
    match host {
        "claude" | "pi" => format!("/{name}"),
        "codex" => format!("/prompts:{name}"),
        "kimi" => format!("/skill:{name}"),
        _ => unreachable!("host 已由 validate_host 校验"),
    }
}

/// 该 host 下这个短命令对应的文件路径（kimi 是目录里的 SKILL.md）。
pub(crate) fn command_path(home: &Path, host: &str, name: &str) -> PathBuf {
    match host {
        "claude" => home.join(".claude").join("commands").join(format!("{name}.md")),
        "codex" => home.join(".codex").join("prompts").join(format!("{name}.md")),
        "pi" => home.join(".pi").join("agent").join("prompts").join(format!("{name}.md")),
        "kimi" => home
            .join(".kimi-code")
            .join("skills")
            .join(name)
            .join("SKILL.md"),
        _ => unreachable!("host 已由 validate_host 校验"),
    }
}

/// host 必须是四家之一：路径与写法都按家分支，没有通配写法（modes.json
/// 里出现别的值也不落到某一家）。`preset remove` 的级联清理在删文件前
/// 也用它重新校验，不把 modes.json 里手改的坏值送进 `unreachable!`。
pub(crate) fn validate_host(host: &str) -> Result<(), (i32, String)> {
    if crate::hosts::HOST_IDS.contains(&host) {
        Ok(())
    } else {
        Err((
            2,
            format!("未知 host：{host}；可选：{}", crate::hosts::HOST_IDS.join("、")),
        ))
    }
}

/// 命令名：小写字母、数字与 `-`，1–32 位。`preset remove` 的级联清理
/// 删文件前同样用它过一遍 modes.json 里的定义。
pub(crate) fn validate_name(name: &str) -> Result<(), (i32, String)> {
    let ok = !name.is_empty()
        && name.chars().count() <= 32
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !ok {
        return Err((
            2,
            format!("命令名只认小写字母、数字与 -，1–32 位：{name}"),
        ));
    }
    if RESERVED_NAMES.contains(&name) {
        return Err((2, format!("命令名是保留名：{name}")));
    }
    Ok(())
}

/// add-all 的前缀：与命令名同一套字符（小写字母、数字与 `-`），可为空
///（此时名字就是模式短名）；不查保留名（前缀只是名字的前半段），拼出
/// 的名字另行逐个过 `validate_name`。
fn validate_prefix(prefix: &str) -> Result<(), (i32, String)> {
    if prefix
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        Ok(())
    } else {
        Err((2, format!("前缀只认小写字母、数字与 -：{prefix}")))
    }
}

/// 固定附加说明：单行，≤200 字。
fn validate_note(note: &str) -> Result<(), (i32, String)> {
    if note.contains('\n') || note.contains('\r') {
        return Err((2, "说明需为单行".into()));
    }
    if note.chars().count() > 200 {
        return Err((2, "说明最长 200 字".into()));
    }
    Ok(())
}

/// frontmatter（首行 `---` 到闭合 `---` 之间）里有没有顶格的
/// `aicross-managed: true`（允许行尾 `# 注释`）。开头的 BOM 先去掉；没有
/// 闭合 `---` 时不认；CRLF 由 `lines()` 处理。
fn has_managed_marker(text: &str) -> bool {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines();
    if !lines.next().is_some_and(|line| line.trim() == "---") {
        return false;
    }
    let mut found = false;
    for line in lines {
        if line.trim() == "---" {
            // 只有闭合了的 frontmatter 才认。
            return found;
        }
        if found {
            continue;
        }
        // 标记必须顶格（strip_prefix 吃不到缩进的行）；值后面允许 # 注释。
        if let Some(value) = line
            .strip_prefix(MANAGED_MARKER)
            .and_then(|rest| rest.strip_prefix(':'))
        {
            let value = value.trim();
            found = value == "true"
                || value
                    .strip_prefix("true")
                    .is_some_and(|rest| rest.trim_start().starts_with('#'));
        }
    }
    false
}

/// 路径本身是不是链接（symlink 或 Windows junction）；不跟随链接。
/// 不存在返回 Ok(false)，其它读不到元数据的情况原样返回错误。
fn is_link(path: &Path) -> std::io::Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => Ok(meta.file_type().is_symlink() || std::fs::read_link(path).is_ok()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// 磁盘实际状态，也是写入与删除共用的归属检查（list、add、remove 与
/// preset remove 的级联删除都经这里）：命令文件是普通文件且带标记 =
/// 已写入；不存在 = 未写入；其余一律被占用——存在无标记或读不了、命令
/// 目录（kimi 是 `<名>/`，其余家是 commands / prompts 目录）或文件本身是
/// symlink / junction（含悬空链接，链接指向的是外部管理的位置）、元数据
/// 读不到（除确实不存在）。更上层的祖先目录不查。
pub(crate) fn file_state(path: &Path) -> &'static str {
    let directory_linked = path.parent().map(is_link).unwrap_or(Ok(false));
    if !matches!(directory_linked, Ok(false)) {
        return STATE_OCCUPIED;
    }
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return STATE_ABSENT,
        Err(_) => return STATE_OCCUPIED,
        Ok(meta) if !meta.is_file() || meta.file_type().is_symlink() => return STATE_OCCUPIED,
        Ok(_) if std::fs::read_link(path).is_ok() => return STATE_OCCUPIED,
        Ok(_) => {}
    }
    match std::fs::read_to_string(path) {
        Ok(text) if has_managed_marker(&text) => STATE_WRITTEN,
        _ => STATE_OCCUPIED,
    }
}

/// 命令名在磁盘上的占用判定，`slash add` 与 list 的 `existing` 共用一份
/// （fix-57-r1 3）：命令文件存在且无我们的标记 = 被占用；kimi 额外两处
/// 也算被占用——`~/.agents/skills/<名>` 已存在（我们从不往那里写短命令，
/// 存在即别人的），或 `~/.kimi-code/skills/<名>/` 目录已在而我们没有带
/// 标记的 SKILL.md。返回 (状态，占用方路径)；未占用时状态为已写入或
/// 未写入，路径是命令文件本身。
fn command_occupancy(home: &Path, host: &str, name: &str) -> (&'static str, PathBuf) {
    let target = command_path(home, host, name);
    if file_state(&target) == STATE_OCCUPIED {
        return (STATE_OCCUPIED, target);
    }
    if host == "kimi" {
        let agents_dir = home.join(".agents").join("skills").join(name);
        // 不跟随链接：悬空链接也算存在。
        if std::fs::symlink_metadata(&agents_dir).is_ok() {
            return (STATE_OCCUPIED, agents_dir);
        }
        let directory = target.parent().expect("command_path 必有父目录");
        if directory.exists() && file_state(&target) != STATE_WRITTEN {
            return (STATE_OCCUPIED, directory.to_path_buf());
        }
    }
    let state = file_state(&target);
    (state, target)
}

/// 双引号字符串并做 JSON 风格转义（frontmatter 的值用）。
fn json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// 换行与控制字符替换为空格（正文是自由文本，不转义）。
fn single_line_text(value: &str) -> String {
    value.chars().map(|c| if c.is_control() { ' ' } else { c }).collect()
}

/// 写入的文件内容：frontmatter（各家字段不同）加一句固定指令。参数占位
/// claude / codex 用 `$ARGUMENTS`；pi 与 kimi 的参数会追加在指令后，不写
/// 占位。frontmatter 的值写成带引号的字符串并做 JSON 风格转义；模式名与
/// 说明在正文里的换行、控制字符替换为空格。`description` 为
/// 「ai-cross · <模式名>」。claude 与 kimi 另带
/// `disable-model-invocation: true`（TASK-59）：这两家的模型会扫命令 /
/// skill 清单自己触发，该行把触发限制为用户输入；codex 与 pi 的 prompts
/// 本来只能由用户输入，不加。list 的 `stale` 也用这份文本做比对。
fn command_file_text(host: &str, name: &str, mode_name: &str, note: &str) -> String {
    let takes_arguments = matches!(host, "claude" | "codex");
    let disables_model_invocation = matches!(host, "claude" | "kimi");
    let mut text = String::from("---\n");
    if host == "kimi" {
        text.push_str(&format!("name: {}\n", json_string(name)));
    }
    text.push_str(&format!(
        "description: {}\n",
        json_string(&format!("ai-cross · {mode_name}"))
    ));
    if takes_arguments {
        text.push_str("argument-hint: <任务>\n");
    }
    if disables_model_invocation {
        text.push_str("disable-model-invocation: true\n");
    }
    text.push_str(&format!("{MANAGED_MARKER}: true\n---\n\n"));
    let mode_name = single_line_text(mode_name);
    let note = single_line_text(note);
    text.push_str(&format!(
        "使用 ai-cross skill，按「{mode_name}」处理下面的请求；派发时带 --host {host}。"
    ));
    text.push_str(&note);
    if takes_arguments {
        text.push_str("\n$ARGUMENTS");
    }
    text.push('\n');
    text
}

/// 写文件：先写同目录临时文件再 rename（Windows 的 rename 可覆盖已存在的
/// 目标），失败时删掉临时文件，不留半成品。
fn write_command_file(
    home: &Path,
    host: &str,
    name: &str,
    mode_name: &str,
    note: &str,
) -> Result<PathBuf, (i32, String)> {
    let target = command_path(home, host, name);
    // 与删除同一份归属检查：链接与别人的文件一律不写。
    if file_state(&target) == STATE_OCCUPIED {
        return Err((1, format!("已存在同名命令（非 aicross 管理）：{}", target.display())));
    }
    let directory = target
        .parent()
        .ok_or_else(|| (1, format!("无法定位写入目录：{}", target.display())))?;
    std::fs::create_dir_all(directory).map_err(|error| {
        (1, format!("创建目录 {} 失败：{error}", directory.display()))
    })?;
    let temporary = directory.join(format!(".aicross-slash-{}.tmp", Uuid::new_v4()));
    let text = command_file_text(host, name, mode_name, note);
    let outcome = (|| {
        std::fs::write(&temporary, &text)
            .map_err(|error| (1, format!("写入临时文件失败：{error}")))?;
        std::fs::rename(&temporary, &target)
            .map_err(|error| (1, format!("替换 {} 失败：{error}", target.display())))
    })();
    if outcome.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    outcome.map(|()| target)
}

/// stale 比对用的规整文本：去掉开头的 BOM，CRLF 统一成 LF。
fn comparable_text(text: &str) -> String {
    text.strip_prefix('\u{feff}')
        .unwrap_or(text)
        .replace("\r\n", "\n")
}

/// list 的实现体；调用方持有 `modes_write_lock`。宽松读取 modes.json（与
/// `config show` 同一处理）：文件缺失或损坏按空配置显示。预设被删后
/// `modeName` 回落成预设 id；名称不合法的条目标「名称无效」，不读磁盘。
fn list_locked(root: &Path, home: &Path, host: &str) -> Result<Value, (i32, String)> {
    let modes = orchestration::modes_from(root);
    let presets = orchestration::raw_presets_from(root);
    let commands = orchestration::host_section(&modes, host)
        .map(|section| section.slash_commands.clone())
        .unwrap_or_default();
    let items = commands
        .iter()
        .map(|command| {
            let path = command_path(home, host, &command.name);
            // 名称不合法的条目不读磁盘：路径只按规则拼出来显示。
            let valid_name = validate_name(&command.name).is_ok();
            let state = if valid_name {
                file_state(&path)
            } else {
                STATE_INVALID_NAME
            };
            let mode_name = presets
                .presets
                .iter()
                .find(|preset| preset.id == command.mode)
                .map(|preset| preset.name.clone())
                .unwrap_or_else(|| command.mode.clone());
            // TASK-59：文件带我们的标记、但内容与现在会生成的不一致（如
            // 旧版没有 disable-model-invocation）时标 stale，同名 add 即
            // 重写，界面据此提示「更新」。只有已写入的状态才比对；比对前
            // 去 BOM、CRLF 统一成 LF，编辑器改了换行或加了 BOM 不算 stale。
            let stale = state == STATE_WRITTEN
                && std::fs::read_to_string(&path)
                    .map(|text| {
                        comparable_text(&text)
                            != comparable_text(&command_file_text(
                                host,
                                &command.name,
                                &mode_name,
                                &command.note,
                            ))
                    })
                    .unwrap_or(false);
            json!({
                "name": command.name,
                "mode": command.mode,
                "modeName": mode_name,
                "note": command.note,
                "path": path.to_string_lossy(),
                "state": state,
                "stale": stale,
                "trigger": trigger_hint(host, &command.name),
            })
        })
        .collect::<Vec<_>>();
    let mode_items = builtin_mode_items(home, host, &items, &presets);
    Ok(json!({
        "host": host,
        "invoke": invoke_hint(host),
        "commands": items,
        "modes": mode_items,
    }))
}

/// list 输出的 `modes`（TASK-54）：内置模式逐条给出带参数写法、建议名与
/// 已有命令。模式名取预设名，预设缺失时回落成 id；`command` 是 commands
/// 里 mode 等于该模式的第一条（同 commands[] 结构），没有为 null。
/// 走查 #14：该模式还没有登记的命令、但 `<前缀><短名>` 对应的命令文件
/// 已在磁盘上（重装或换数据目录后常见）时，另给 `existing`：`state` 为
/// 已写入（带我们的标记，add 可直接登记）或被占用（无标记，add 拒绝），
/// 文件不在时为 null。
fn builtin_mode_items(
    home: &Path,
    host: &str,
    items: &[Value],
    presets: &orchestration::PresetFile,
) -> Vec<Value> {
    MODE_SHORTNAMES
        .iter()
        .map(|(id, slug, keyword)| {
            // 模式名与介绍都取预设；预设已删时名字回落成 id，介绍留空。
            let (mode_name, description) = presets
                .presets
                .iter()
                .find(|preset| preset.id == *id)
                .map(|preset| (preset.name.clone(), preset.description.clone()))
                .unwrap_or_else(|| ((*id).to_string(), String::new()));
            let command = items
                .iter()
                .find(|item| item["mode"].as_str() == Some(*id))
                .cloned()
                .unwrap_or(Value::Null);
            let existing = if command.is_null() {
                let name = format!("{DEFAULT_PREFIX}{slug}");
                // 占用判定与 slash add 同一份（fix-57-r1 3）：kimi 的
                // ~/.agents/skills 同名目录与无标记命令目录都报被占用。
                let (state, path) = command_occupancy(home, host, &name);
                match state {
                    STATE_ABSENT => Value::Null,
                    state => json!({
                        "name": name,
                        "state": state,
                        "path": path.to_string_lossy(),
                    }),
                }
            } else {
                Value::Null
            };
            json!({
                "mode": id,
                "modeName": mode_name,
                "description": description,
                "argForm": format!("{} {keyword}", invoke_hint(host)),
                "suggestedName": format!("{DEFAULT_PREFIX}{slug}"),
                "command": command,
                "existing": existing,
            })
        })
        .collect()
}

/// `slash list` 的实现体；窗口命令同此结构。
pub(crate) fn slash_list_at(root: &Path, home: &Path, host: &str) -> Result<Value, (i32, String)> {
    // 扩展主 agent 没有短命令：只给调用写法，界面据 supported 隐藏短命令卡片。
    if let Some(extra) = crate::hosts::extra_host(host) {
        return Ok(json!({
            "host": host,
            "invoke": extra.invoke,
            "commands": [],
            "modes": [],
            "supported": false,
        }));
    }
    validate_host(host)?;
    let _guard = crate::console_cli::modes_write_lock(root)?;
    list_locked(root, home, host)
}

/// kimi 命令目录里的 SKILL.md 删掉后，目录空了才连目录一起删；还有别的
/// 文件时删不掉，不报错。
fn remove_empty_command_dir(target: &Path, host: &str) {
    if host == "kimi" {
        if let Some(directory) = target.parent() {
            let _ = std::fs::remove_dir(directory);
        }
    }
}

/// add 存配置失败时的文件回滚：新建的删掉，更新的恢复原内容。
fn rollback_command_file(target: &Path, host: &str, original: Option<&[u8]>) {
    match original {
        Some(bytes) => {
            let _ = std::fs::write(target, bytes);
        }
        None => {
            let _ = std::fs::remove_file(target);
            remove_empty_command_dir(target, host);
        }
    }
}

/// `slash add` 的入口：校验后取锁进实现体。
pub(crate) fn slash_add_at(
    root: &Path,
    home: &Path,
    host: &str,
    name: &str,
    mode: &str,
    note: &str,
) -> Result<Value, (i32, String)> {
    validate_host(host)?;
    validate_name(name)?;
    validate_note(note)?;
    let _guard = crate::console_cli::modes_write_lock(root)?;
    slash_add_locked(root, home, host, name, mode, note)
}

/// add 的实现体（调用方持有 `modes_write_lock`，add-all 逐模式调用同一
/// 份写入逻辑，不在锁内再取锁）：先写文件再存定义；同名定义已存在则
/// 更新（覆盖我们自己的文件）。别人的文件（存在无标记）退出 1 不覆盖。
/// 存配置失败时回滚文件（新建的删掉，更新的恢复原内容），配置保持原样。
fn slash_add_locked(
    root: &Path,
    home: &Path,
    host: &str,
    name: &str,
    mode: &str,
    note: &str,
) -> Result<Value, (i32, String)> {
    let wanted = crate::console_cli::resolve_preset_name(mode);
    let presets = orchestration::raw_presets_from(root);
    let Some(preset) = presets.presets.iter().find(|preset| preset.id == wanted) else {
        return Err((2, crate::console_cli::unknown_mode_message(mode, &presets.presets)));
    };
    let mode_name = preset.name.clone();
    let target = command_path(home, host, name);
    // 占用判定与 list 的 existing 共用一份（fix-57-r1 3）：文件无标记，
    // kimi 另有 ~/.agents/skills 与无标记命令目录两处。
    let (state, blocking) = command_occupancy(home, host, name);
    if state == STATE_OCCUPIED {
        return Err((
            1,
            format!("已存在同名命令（非 aicross 管理）：{}", blocking.display()),
        ));
    }
    // 严格读 modes.json：损坏或旧格式退出 1、不覆盖（不写文件也不改定义）。
    let mut modes = crate::console_cli::strict_modes_for_write(root)?;
    // 回滚用：更新时把自己的旧文件内容存下来；新建时为 None。
    let original = std::fs::read(&target).ok();
    write_command_file(home, host, name, &mode_name, note)?;
    let command = SlashCommand {
        name: name.to_string(),
        mode: wanted,
        note: note.to_string(),
    };
    let section = orchestration::host_section_mut(&mut modes, host);
    match section
        .slash_commands
        .iter()
        .position(|existing| existing.name == name)
    {
        Some(index) => section.slash_commands[index] = command,
        None => section.slash_commands.push(command),
    }
    #[cfg(test)]
    if FAIL_SAVE_MODES.with(|flag| flag.replace(false)) {
        rollback_command_file(&target, host, original.as_deref());
        return Err((1, "存 modes.json 失败（测试注入）".into()));
    }
    if let Err(message) = orchestration::save_modes_in(root, &modes) {
        rollback_command_file(&target, host, original.as_deref());
        return Err((1, message));
    }
    list_locked(root, home, host)
}

/// `slash add-all` 的实现体（TASK-54）：对每个内置模式，该模式已有命令
/// （任何名字）就跳过，否则用 `<前缀><短名>` 走 add 的同一份写入逻辑；
/// 单个失败（如被占用）不中断，连同原因记入 skipped。整个批次持同一把
/// 锁；任何写入之前先严格读 modes.json，损坏或旧格式直接退出 1，文件与
/// 定义都不动；拼出的名字过不了名称校验同样先于任何写入退出 2。
pub(crate) fn slash_add_all_at(
    root: &Path,
    home: &Path,
    host: &str,
    prefix: &str,
) -> Result<Value, (i32, String)> {
    validate_host(host)?;
    validate_prefix(prefix)?;
    for (_, slug, _) in MODE_SHORTNAMES {
        validate_name(&format!("{prefix}{slug}"))?;
    }
    let _guard = crate::console_cli::modes_write_lock(root)?;
    crate::console_cli::strict_modes_for_write(root)?;
    let presets = orchestration::raw_presets_from(root);
    let mut added: Vec<String> = Vec::new();
    let mut skipped: Vec<Value> = Vec::new();
    for (id, slug, _) in MODE_SHORTNAMES {
        let name = format!("{prefix}{slug}");
        let definitions = orchestration::modes_from(root)
            .hosts
            .get(host)
            .map(|section| section.slash_commands.clone())
            .unwrap_or_default();
        let existing = definitions
            .iter()
            .find(|command| command.mode == id)
            .map(|command| command.name.clone());
        if let Some(existing) = existing {
            skipped.push(json!({
                "name": name,
                "reason": format!("该模式已有短命令：{existing}"),
            }));
            continue;
        }
        // 生成名已被别的模式的定义占用时不改写：改写会把那条短命令换到
        // 本模式，等于动了他人的配置；记入 skipped 让用户改名或先删。
        let taken_by = definitions
            .iter()
            .find(|command| command.name == name && command.mode != id)
            .map(|command| command.mode.clone());
        if let Some(other_mode) = taken_by {
            let other_name = presets
                .presets
                .iter()
                .find(|preset| preset.id == other_mode)
                .map(|preset| preset.name.clone())
                .unwrap_or(other_mode);
            skipped.push(json!({
                "name": name,
                "reason": format!("名称已被 {other_name} 使用"),
            }));
            continue;
        }
        match slash_add_locked(root, home, host, &name, id, "") {
            Ok(_) => added.push(name),
            Err((_, message)) => skipped.push(json!({ "name": name, "reason": message })),
        }
    }
    let mut value = list_locked(root, home, host)?;
    if let Some(object) = value.as_object_mut() {
        object.insert("added".into(), json!(added));
        object.insert("skipped".into(), Value::Array(skipped));
    }
    Ok(value)
}

/// 删除我们写的带标记文件（kimi 目录空了连目录一起删）；别人的文件与
/// 不存在的文件不动，返回 false。`slash remove` 与 `preset remove` 的
/// 级联清理（TASK-55）共用同一份删除逻辑。
pub(crate) fn remove_marked_command_file(
    home: &Path,
    host: &str,
    name: &str,
) -> Result<bool, String> {
    let target = command_path(home, host, name);
    if file_state(&target) != STATE_WRITTEN {
        return Ok(false);
    }
    #[cfg(test)]
    {
        let pending = FAIL_REMOVE_FILE.with(|flag| {
            let pending = flag.get();
            if pending > 0 {
                flag.set(pending - 1);
            }
            pending
        });
        if pending == 1 {
            return Err(format!("删除 {} 失败（测试注入）", target.display()));
        }
    }
    std::fs::remove_file(&target)
        .map_err(|error| format!("删除 {} 失败：{error}", target.display()))?;
    remove_empty_command_dir(&target, host);
    Ok(true)
}

/// `slash remove` 的实现体：先删带标记的文件（kimi 只删我们写的 SKILL.md，
/// 目录空了才连目录一起删），成功后再存定义；删文件失败直接报错、配置
/// 不动。定义不在但文件带标记时也清掉文件（旧版先存配置留下的孤儿）。
/// 别人的文件与不存在的文件都不动。
pub(crate) fn slash_remove_at(
    root: &Path,
    home: &Path,
    host: &str,
    name: &str,
) -> Result<Value, (i32, String)> {
    validate_host(host)?;
    validate_name(name)?;
    let _guard = crate::console_cli::modes_write_lock(root)?;
    let mut modes = crate::console_cli::strict_modes_for_write(root)?;
    let index = modes
        .hosts
        .get(host)
        .and_then(|section| {
            section
                .slash_commands
                .iter()
                .position(|command| command.name == name)
        });
    let target = command_path(home, host, name);
    if index.is_none() && file_state(&target) != STATE_WRITTEN {
        return Err((2, format!("未找到短命令：{name}")));
    }
    remove_marked_command_file(home, host, name).map_err(|message| (1, message))?;
    if let Some(index) = index {
        modes.hosts.get_mut(host).unwrap().slash_commands.remove(index);
        orchestration::save_modes_in(root, &modes).map_err(|message| (1, message))?;
    }
    list_locked(root, home, host)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::ModesFile;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = crate::paths::test_temp_dir().join(format!("t53-{tag}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// list 输出里第一条命令的字段。
    fn first_command(value: &Value) -> &serde_json::Map<String, Value> {
        value["commands"].as_array().unwrap()[0].as_object().unwrap()
    }

    /// 四家各自的路径与完整文件内容精确比对：参数占位只有 claude / codex
    /// 有（含 `argument-hint`），kimi 另有 `name`，frontmatter 的值是带引
    /// 号的字符串。
    #[test]
    fn task53_write_paths_and_contents_per_host() {
        let root = temp_dir("paths-root");
        let home = temp_dir("paths-home");
        let cases = [
            ("claude", home.join(".claude").join("commands").join("cross-review.md")),
            ("codex", home.join(".codex").join("prompts").join("cross-review.md")),
            ("pi", home.join(".pi").join("agent").join("prompts").join("cross-review.md")),
            ("kimi", home.join(".kimi-code").join("skills").join("cross-review").join("SKILL.md")),
        ];
        for (host, path) in cases {
            let value = slash_add_at(&root, &home, host, "cross-review", "blind", "只报问题").unwrap();
            assert_eq!(value["host"], host);
            assert!(path.is_file(), "{host} 应写到 {}", path.display());
            let text = std::fs::read_to_string(&path).unwrap();
            let mut expected = String::from("---\n");
            if host == "kimi" {
                expected.push_str("name: \"cross-review\"\n");
            }
            expected.push_str("description: \"ai-cross · 盲审模式\"\n");
            if host == "claude" || host == "codex" {
                expected.push_str("argument-hint: <任务>\n");
            }
            // claude 与 kimi 带这行：模型不会自己触发（TASK-59）。
            if host == "claude" || host == "kimi" {
                expected.push_str("disable-model-invocation: true\n");
            }
            expected.push_str(&format!(
                "{MANAGED_MARKER}: true\n---\n\n使用 ai-cross skill，按「盲审模式」处理下面的请求；派发时带 --host {host}。只报问题"
            ));
            if host == "claude" || host == "codex" {
                expected.push_str("\n$ARGUMENTS");
            }
            expected.push('\n');
            assert_eq!(text, expected, "{host} 的文件内容应完全一致");
            let command = first_command(&value);
            assert_eq!(command["name"], "cross-review");
            assert_eq!(command["mode"], "builtin-blind");
            assert_eq!(command["modeName"], "盲审模式");
            assert_eq!(command["note"], "只报问题");
            assert_eq!(command["path"], path.to_string_lossy().as_ref());
            assert_eq!(command["state"], STATE_WRITTEN);
            assert_eq!(
                command["trigger"],
                match host {
                    "claude" | "pi" => "/cross-review",
                    "codex" => "/prompts:cross-review",
                    "kimi" => "/skill:cross-review",
                    _ => unreachable!(),
                }
            );
            // modes.json 里的定义。
            let modes = orchestration::modes_from(&root);
            let section = orchestration::host_section(&modes, host).unwrap();
            assert_eq!(section.slash_commands.len(), 1);
            assert_eq!(section.slash_commands[0].mode, "builtin-blind");
            // 各家的 invoke 写法。
            assert_eq!(
                value["invoke"],
                match host {
                    "claude" => "/ai-cross",
                    "codex" => "$ai-cross",
                    "pi" | "kimi" => "/skill:ai-cross",
                    _ => unreachable!(),
                }
            );
        }
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// frontmatter 的值转义、正文的换行与控制字符替换为空格。
    #[test]
    fn task53_frontmatter_escaped_and_body_single_line() {
        let text = command_file_text("claude", "x", "盲\"审\"\n模式", "注\t释");
        assert!(
            text.contains("description: \"ai-cross · 盲\\\"审\\\"\\n模式\"\n"),
            "{text}"
        );
        assert!(text.contains("按「盲\"审\" 模式」处理"), "{text}");
        assert!(text.contains("--host claude。注 释\n$ARGUMENTS\n"), "{text}");
        let kimi = command_file_text("kimi", "x", "模式", "");
        assert!(kimi.starts_with("---\nname: \"x\"\n"), "{kimi}");
        assert!(!kimi.contains("argument-hint") && !kimi.contains("$ARGUMENTS"), "{kimi}");
    }

    /// TASK-59：claude 与 kimi 的 frontmatter 带
    /// `disable-model-invocation: true`，codex 与 pi 不带；list 对带标记但
    /// 内容与现在会生成的不一致的文件标 `stale`（如旧版缺这一行），同名
    /// add 重写后回到 false；被占用与未写入不标。
    #[test]
    fn task59_disable_model_invocation_and_stale_flag() {
        let root = temp_dir("t59-stale-root");
        let home = temp_dir("t59-stale-home");
        // claude 与 kimi 带 disable-model-invocation。
        for host in ["claude", "kimi"] {
            let value = slash_add_at(&root, &home, host, "stale-x", "blind", "").unwrap();
            assert_eq!(first_command(&value)["stale"], false, "{host}");
            let path = command_path(&home, host, "stale-x");
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(text.contains("disable-model-invocation: true\n"), "{host}: {text}");
            // 旧版文件：抹掉这一行，list 标 stale（状态仍是已写入）。
            std::fs::write(&path, text.replace("disable-model-invocation: true\n", "")).unwrap();
            let value = slash_list_at(&root, &home, host).unwrap();
            assert_eq!(first_command(&value)["state"], STATE_WRITTEN, "{host}");
            assert_eq!(first_command(&value)["stale"], true, "{host}");
            // 同名 add 即重写：stale 回 false。
            let value = slash_add_at(&root, &home, host, "stale-x", "blind", "").unwrap();
            assert_eq!(first_command(&value)["stale"], false, "{host}");
            // 别的内容被手改（说明变了）同样标 stale。
            let path = command_path(&home, host, "stale-x");
            let text = std::fs::read_to_string(&path).unwrap();
            std::fs::write(&path, text.replace("盲审模式", "其它说明")).unwrap();
            let value = slash_list_at(&root, &home, host).unwrap();
            assert_eq!(first_command(&value)["stale"], true, "{host}");
        }
        // codex 与 pi 不带这行；手改正文（仍带标记）也标 stale。
        for host in ["codex", "pi"] {
            let value = slash_add_at(&root, &home, host, "stale-x", "blind", "").unwrap();
            assert_eq!(first_command(&value)["stale"], false, "{host}");
            let path = command_path(&home, host, "stale-x");
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(!text.contains("disable-model-invocation"), "{host}: {text}");
            std::fs::write(&path, format!("{text}手改的一行\n")).unwrap();
            let value = slash_list_at(&root, &home, host).unwrap();
            assert_eq!(first_command(&value)["stale"], true, "{host}");
            // 文件被换成别人的（无标记）：状态被占用，不标 stale。
            std::fs::write(&path, "别人的命令").unwrap();
            let value = slash_list_at(&root, &home, host).unwrap();
            assert_eq!(first_command(&value)["state"], STATE_OCCUPIED, "{host}");
            assert_eq!(first_command(&value)["stale"], false, "{host}");
        }
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// fix-59-r1：stale 比对前去 BOM、CRLF 统一成 LF——内容相同只是换成
    /// CRLF、加了 BOM 或两者都有的文件不算 stale；内容真变了仍算。
    #[test]
    fn task59_stale_ignores_crlf_and_bom() {
        let root = temp_dir("t59-stale-eol-root");
        let home = temp_dir("t59-stale-eol-home");
        for host in ["claude", "codex", "kimi", "pi"] {
            slash_add_at(&root, &home, host, "eol-x", "blind", "").unwrap();
            let path = command_path(&home, host, "eol-x");
            let text = std::fs::read_to_string(&path).unwrap();
            let crlf = text.replace('\n', "\r\n");
            for variant in [crlf.clone(), format!("\u{feff}{text}"), format!("\u{feff}{crlf}")] {
                std::fs::write(&path, &variant).unwrap();
                let value = slash_list_at(&root, &home, host).unwrap();
                assert_eq!(first_command(&value)["state"], STATE_WRITTEN, "{host}");
                assert_eq!(first_command(&value)["stale"], false, "{host}: {variant:?}");
            }
            std::fs::write(&path, format!("\u{feff}{crlf}手改的一行\r\n")).unwrap();
            let value = slash_list_at(&root, &home, host).unwrap();
            assert_eq!(first_command(&value)["stale"], true, "{host}");
        }
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// 标记判定：只认首行 --- 的 frontmatter 内、顶格、值为 true（允许行尾
    /// # 注释）的标记；BOM 先去掉；未闭合与 frontmatter 外的不认；CRLF 可认。
    #[test]
    fn task53_has_managed_marker_rules() {
        assert!(has_managed_marker("---\ndescription: x\naicross-managed: true\n---\n"));
        assert!(has_managed_marker("---\r\ndescription: x\r\naicross-managed: true\r\n---\r\n"));
        assert!(has_managed_marker("---\naicross-managed: true # aicross 写入\n---\n"));
        assert!(has_managed_marker("\u{feff}---\naicross-managed: true\n---\n"));
        assert!(!has_managed_marker("---\n  aicross-managed: true\n---\n"));
        assert!(!has_managed_marker("---\ndescription: x\n\taicross-managed: true\n---\n"));
        assert!(!has_managed_marker("---\n aicross-managed : true\n---\n"));
        assert!(!has_managed_marker("---\ndescription: x\n---\naicross-managed: true\n"));
        assert!(!has_managed_marker("---\naicross-managed: true\n"));
        assert!(!has_managed_marker("aicross-managed: true\n"));
        assert!(!has_managed_marker("\u{feff}aicross-managed: true\n"));
        assert!(!has_managed_marker("---\naicross-managed: false\n---\n"));
        assert!(!has_managed_marker("---\naicross-managed-别扐: true\n---\n"));
    }

    /// 名称校验：只认小写字母、数字与 `-`，1–32 位；空名与超长退出 2。
    #[test]
    fn task53_name_validation() {
        let root = temp_dir("name-root");
        let home = temp_dir("name-home");
        for bad in ["Cross", "has space", "下划线_1", "a/b", "", &"a".repeat(33)] {
            let error = slash_add_at(&root, &home, "claude", bad, "blind", "").unwrap_err();
            assert_eq!(error.0, 2, "{bad}");
            assert!(error.1.contains("命令名"), "{bad}: {}", error.1);
        }
        for good in ["a", "a-b-9", &"a".repeat(32)] {
            slash_add_at(&root, &home, "claude", good, "blind", "").unwrap();
        }
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// 模式不存在退出 2 并列出可选；别名解析同 --preset。
    #[test]
    fn task53_unknown_mode_exits_2_and_alias_resolves() {
        let root = temp_dir("mode-root");
        let home = temp_dir("mode-home");
        let error = slash_add_at(&root, &home, "claude", "x", "nope", "").unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("模式不存在：nope"), "{}", error.1);
        // 别名 blind → builtin-blind；写入后定义存预设 id。
        slash_add_at(&root, &home, "claude", "x", "blind", "").unwrap();
        let modes = orchestration::modes_from(&root);
        assert_eq!(
            orchestration::host_section(&modes, "claude").unwrap().slash_commands[0].mode,
            "builtin-blind"
        );
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// 说明校验：多行与超 200 字退出 2；合法说明原样进正文。
    #[test]
    fn task53_note_validation() {
        let root = temp_dir("note-root");
        let home = temp_dir("note-home");
        let error = slash_add_at(&root, &home, "claude", "x", "blind", "两行\n说明").unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("单行"), "{}", error.1);
        let error = slash_add_at(&root, &home, "claude", "x", "blind", &"长".repeat(201)).unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("200"), "{}", error.1);
        // 刚好 200 字合法。
        let value = slash_add_at(&root, &home, "claude", "x", "blind", &"字".repeat(200)).unwrap();
        assert_eq!(first_command(&value)["note"], "字".repeat(200));
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// 被占用（存在无标记）不覆盖退出 1：文件原样，定义也不写入。
    #[test]
    fn task53_occupied_file_not_overwritten() {
        let root = temp_dir("occupied-root");
        let home = temp_dir("occupied-home");
        let path = command_path(&home, "claude", "x");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "别人的命令").unwrap();
        let error = slash_add_at(&root, &home, "claude", "x", "blind", "").unwrap_err();
        assert_eq!(error.0, 1);
        assert!(
            error.1.contains("已存在同名命令（非 aicross 管理）") && error.1.contains(&path.display().to_string()),
            "{}",
            error.1
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "别人的命令");
        let modes = orchestration::modes_from(&root);
        assert!(orchestration::host_section(&modes, "claude")
            .is_none_or(|section| section.slash_commands.is_empty()));
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// 更新：同名定义已存在则更新（改模式与说明），覆盖我们自己的文件，
    /// 定义只有一条。
    #[test]
    fn task53_add_updates_existing_definition_and_overwrites_own_file() {
        let root = temp_dir("update-root");
        let home = temp_dir("update-home");
        slash_add_at(&root, &home, "claude", "x", "blind", "旧说明").unwrap();
        let value = slash_add_at(&root, &home, "claude", "x", "economy", "新说明").unwrap();
        let command = first_command(&value);
        assert_eq!(command["mode"], "builtin-economy");
        assert_eq!(command["modeName"], "经济模式");
        assert_eq!(command["note"], "新说明");
        assert_eq!(value["commands"].as_array().unwrap().len(), 1);
        let text = std::fs::read_to_string(command_path(&home, "claude", "x")).unwrap();
        assert!(text.contains("「经济模式」") && text.contains("新说明"), "{text}");
        assert!(!text.contains("旧说明"), "{text}");
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// remove 只删带标记的文件：别人的文件留在原地，定义照删；自己写的
    /// 连文件一起删（kimi 连空目录一起）。
    #[test]
    fn task53_remove_only_deletes_marked_files() {
        let root = temp_dir("remove-root");
        let home = temp_dir("remove-home");
        // 别人的文件：定义删掉，文件不动。
        let path = command_path(&home, "claude", "x");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "---\ndescription: other\n---\n").unwrap();
        let modes = ModesFile::default();
        orchestration::save_modes_in(&root, &modes).unwrap();
        let mut modes = orchestration::modes_from(&root);
        let section = orchestration::host_section_mut(&mut modes, "claude");
        section.slash_commands.push(SlashCommand {
            name: "x".into(),
            mode: "builtin-blind".into(),
            note: String::new(),
        });
        orchestration::save_modes_in(&root, &modes).unwrap();
        let value = slash_remove_at(&root, &home, "claude", "x").unwrap();
        assert_eq!(value["commands"].as_array().unwrap().len(), 0);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "---\ndescription: other\n---\n");
        // 自己写的：claude 删文件；kimi 连空目录一起删。
        slash_add_at(&root, &home, "claude", "y", "blind", "").unwrap();
        slash_add_at(&root, &home, "kimi", "y", "blind", "").unwrap();
        assert!(command_path(&home, "claude", "y").is_file());
        assert!(command_path(&home, "kimi", "y").is_file());
        slash_remove_at(&root, &home, "claude", "y").unwrap();
        slash_remove_at(&root, &home, "kimi", "y").unwrap();
        assert!(!command_path(&home, "claude", "y").exists());
        assert!(!command_path(&home, "kimi", "y").exists());
        assert!(!command_path(&home, "kimi", "y").parent().unwrap().exists());
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// list 的三种 state：定义在文件不在 = 未写入；文件带标记 = 已写入；
    /// 文件被别人换掉 = 被占用。
    #[test]
    fn task53_list_reports_all_three_states() {
        let root = temp_dir("state-root");
        let home = temp_dir("state-home");
        slash_add_at(&root, &home, "claude", "gone", "blind", "").unwrap();
        std::fs::remove_file(command_path(&home, "claude", "gone")).unwrap();
        slash_add_at(&root, &home, "claude", "ours", "blind", "").unwrap();
        slash_add_at(&root, &home, "claude", "taken", "blind", "").unwrap();
        std::fs::write(command_path(&home, "claude", "taken"), "别人的").unwrap();
        let value = slash_list_at(&root, &home, "claude").unwrap();
        let states: Vec<&str> = value["commands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|command| command["state"].as_str().unwrap())
            .collect();
        assert_eq!(states, [STATE_ABSENT, STATE_WRITTEN, STATE_OCCUPIED]);
        // 未写入的那条 modeName 按预设名显示。
        let gone = value["commands"]
            .as_array()
            .unwrap()
            .iter()
            .find(|command| command["name"] == "gone")
            .unwrap();
        assert_eq!(gone["modeName"], "盲审模式");
        assert_eq!(gone["trigger"], "/gone");
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// modes.json 损坏时 add / remove 退出 1、不覆盖：文件与定义都不动。
    #[test]
    fn task53_corrupt_modes_json_blocks_writes() {
        let root = temp_dir("corrupt-root");
        let home = temp_dir("corrupt-home");
        std::fs::write(root.join("modes.json"), "{ not json").unwrap();
        let error = slash_add_at(&root, &home, "claude", "x", "blind", "").unwrap_err();
        assert_eq!(error.0, 1);
        assert!(error.1.contains("已停止写入"), "{}", error.1);
        assert!(!command_path(&home, "claude", "x").exists());
        let error = slash_remove_at(&root, &home, "claude", "x").unwrap_err();
        assert_eq!(error.0, 1);
        // 旧格式同样拒写。
        std::fs::write(root.join("modes.json"), r#"{"version":1,"hosts":{}}"#).unwrap();
        let error = slash_add_at(&root, &home, "claude", "x", "blind", "").unwrap_err();
        assert_eq!(error.0, 1);
        assert!(error.1.contains("旧格式"), "{}", error.1);
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// 保留名退出 2：ai-cross 与 Windows 设备名，文件也不落盘。
    #[test]
    fn task53_reserved_names_rejected() {
        let root = temp_dir("reserved-root");
        let home = temp_dir("reserved-home");
        for bad in ["ai-cross", "con", "nul", "aux", "prn", "com1", "com9", "lpt1", "lpt9"] {
            let error = slash_add_at(&root, &home, "claude", bad, "blind", "").unwrap_err();
            assert_eq!(error.0, 2, "{bad}");
            assert!(error.1.contains("保留名"), "{bad}: {}", error.1);
            assert!(!command_path(&home, "claude", bad).exists(), "{bad}");
        }
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// 三个入口都校验 host 只能是四家之一；非法值不会落到某家的路径。
    #[test]
    fn task53_unknown_host_rejected_at_entry_points() {
        let root = temp_dir("host-root");
        let home = temp_dir("host-home");
        for host in ["_", "claude-code", ""] {
            let error = slash_list_at(&root, &home, host).unwrap_err();
            assert_eq!(error.0, 2, "{host}");
            assert!(error.1.contains("未知 host"), "{host}: {}", error.1);
            let error = slash_add_at(&root, &home, host, "x", "blind", "").unwrap_err();
            assert_eq!(error.0, 2, "{host}");
            let error = slash_remove_at(&root, &home, host, "x").unwrap_err();
            assert_eq!(error.0, 2, "{host}");
        }
        assert!(!home.join(".kimi-code").exists(), "非法 host 不应写到 kimi 路径");
        // remove 对不合法的名称也退出 2。
        let error = slash_remove_at(&root, &home, "claude", "坏名").unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("命令名"), "{}", error.1);
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// remove 删文件失败：直接报错，定义留在配置里、文件留在磁盘上；
    /// 失败用测试注入模拟。
    #[test]
    fn task53_remove_file_failure_keeps_config() {
        let root = temp_dir("rmfail-root");
        let home = temp_dir("rmfail-home");
        slash_add_at(&root, &home, "claude", "x", "blind", "").unwrap();
        FAIL_REMOVE_FILE.with(|flag| flag.set(1));
        let error = slash_remove_at(&root, &home, "claude", "x").unwrap_err();
        assert_eq!(error.0, 1);
        assert!(error.1.contains("删除"), "{}", error.1);
        assert!(command_path(&home, "claude", "x").is_file());
        let modes = orchestration::modes_from(&root);
        assert_eq!(
            orchestration::host_section(&modes, "claude").unwrap().slash_commands.len(),
            1
        );
        // 注入用后即清：再删一次恢复正常。
        slash_remove_at(&root, &home, "claude", "x").unwrap();
        assert!(!command_path(&home, "claude", "x").exists());
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// add 存配置失败：新建的文件删掉、更新的恢复原内容，配置不变；
    /// 失败用测试注入模拟。
    #[test]
    fn task53_add_save_failure_rolls_back_file() {
        let root = temp_dir("savefail-root");
        let home = temp_dir("savefail-home");
        // 新建：回滚后新文件不在，配置为空。
        FAIL_SAVE_MODES.with(|flag| flag.set(true));
        let error = slash_add_at(&root, &home, "claude", "x", "blind", "").unwrap_err();
        assert_eq!(error.0, 1);
        assert!(!command_path(&home, "claude", "x").exists());
        let modes = orchestration::modes_from(&root);
        assert!(
            orchestration::host_section(&modes, "claude")
                .is_none_or(|section| section.slash_commands.is_empty())
        );
        // 更新：回滚后恢复原文件内容与原定义。
        slash_add_at(&root, &home, "claude", "y", "blind", "旧说明").unwrap();
        FAIL_SAVE_MODES.with(|flag| flag.set(true));
        let error = slash_add_at(&root, &home, "claude", "y", "economy", "新说明").unwrap_err();
        assert_eq!(error.0, 1);
        let text = std::fs::read_to_string(command_path(&home, "claude", "y")).unwrap();
        assert!(text.contains("盲审模式") && text.contains("旧说明"), "{text}");
        let modes = orchestration::modes_from(&root);
        assert_eq!(
            orchestration::host_section(&modes, "claude").unwrap().slash_commands[0].mode,
            "builtin-blind"
        );
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// kimi 写入前：~/.agents/skills/<名> 已存在（我们从不往那里写）或
    /// ~/.kimi-code/skills/<名> 目录已在而没有我们带标记的 SKILL.md 时，
    /// 都视为被占用，退出 1 不写文件也不改定义。
    #[test]
    fn task53_kimi_agents_dir_blocks_add() {
        let root = temp_dir("agents-root");
        let home = temp_dir("agents-home");
        let agents_dir = home.join(".agents").join("skills").join("x");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(agents_dir.join("SKILL.md"), "别人的 skill").unwrap();
        let error = slash_add_at(&root, &home, "kimi", "x", "blind", "").unwrap_err();
        assert_eq!(error.0, 1);
        assert!(error.1.contains("已存在同名命令（非 aicross 管理）"), "{}", error.1);
        assert!(!command_path(&home, "kimi", "x").exists());
        let modes = orchestration::modes_from(&root);
        assert!(
            orchestration::host_section(&modes, "kimi")
                .is_none_or(|section| section.slash_commands.is_empty())
        );
        // ~/.kimi-code/skills/<名> 只有别人的文件、没有我们带标记的
        // SKILL.md：同样被占用。
        let kimi_dir = command_path(&home, "kimi", "y").parent().unwrap().to_path_buf();
        std::fs::create_dir_all(&kimi_dir).unwrap();
        std::fs::write(kimi_dir.join("references.md"), "别人的").unwrap();
        let error = slash_add_at(&root, &home, "kimi", "y", "blind", "").unwrap_err();
        assert_eq!(error.0, 1);
        assert!(error.1.contains(".kimi-code"), "{}", error.1);
        assert!(!command_path(&home, "kimi", "y").exists());
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// kimi 目录里有别的文件时：remove 只删我们写的 SKILL.md，别的文件
    /// 与目录都留在原地。
    #[test]
    fn task53_kimi_remove_keeps_extra_files() {
        let root = temp_dir("kimiextra-root");
        let home = temp_dir("kimiextra-home");
        slash_add_at(&root, &home, "kimi", "x", "blind", "").unwrap();
        let directory = command_path(&home, "kimi", "x").parent().unwrap().to_path_buf();
        std::fs::write(directory.join("references.md"), "别的文件").unwrap();
        slash_remove_at(&root, &home, "kimi", "x").unwrap();
        assert!(!command_path(&home, "kimi", "x").exists());
        assert_eq!(
            std::fs::read_to_string(directory.join("references.md")).unwrap(),
            "别的文件"
        );
        assert!(directory.is_dir());
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// 定义不在但文件带标记（旧版先存配置留下的孤儿）：remove 也能清掉
    /// 文件；文件也不在了才退出 2。
    #[test]
    fn task53_remove_cleans_orphan_file_without_definition() {
        let root = temp_dir("orphan-root");
        let home = temp_dir("orphan-home");
        slash_add_at(&root, &home, "claude", "orphan", "blind", "").unwrap();
        let mut modes = orchestration::modes_from(&root);
        let section = orchestration::host_section_mut(&mut modes, "claude");
        section.slash_commands.retain(|command| command.name != "orphan");
        orchestration::save_modes_in(&root, &modes).unwrap();
        let value = slash_remove_at(&root, &home, "claude", "orphan").unwrap();
        assert_eq!(value["commands"].as_array().unwrap().len(), 0);
        assert!(!command_path(&home, "claude", "orphan").exists());
        let error = slash_remove_at(&root, &home, "claude", "orphan").unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("未找到短命令"), "{}", error.1);
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// list：modes.json 里手改成不合法的名称标「名称无效」，不读磁盘；
    /// 合法条目照常。
    #[test]
    fn task53_list_marks_invalid_names() {
        let root = temp_dir("invalid-root");
        let home = temp_dir("invalid-home");
        orchestration::save_modes_in(&root, &ModesFile::default()).unwrap();
        let mut modes = orchestration::modes_from(&root);
        let section = orchestration::host_section_mut(&mut modes, "claude");
        section.slash_commands.push(SlashCommand {
            name: "坏名字".into(),
            mode: "builtin-blind".into(),
            note: String::new(),
        });
        section.slash_commands.push(SlashCommand {
            name: "good-name".into(),
            mode: "builtin-blind".into(),
            note: String::new(),
        });
        orchestration::save_modes_in(&root, &modes).unwrap();
        let value = slash_list_at(&root, &home, "claude").unwrap();
        let commands = value["commands"].as_array().unwrap();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0]["name"], "坏名字");
        assert_eq!(commands[0]["state"], STATE_INVALID_NAME);
        assert_eq!(commands[1]["state"], STATE_ABSENT);
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// list 输出的 modes（TASK-54）：四家 argForm 写法、suggestedName 与
    /// 模式名固定，顺序同短名表；尚无命令时 command 为 null。
    #[test]
    fn task54_list_modes_arg_forms_and_suggested_names() {
        let root = temp_dir("modes-root");
        let home = temp_dir("modes-home");
        let invocations = [
            ("claude", "/ai-cross"),
            ("codex", "$ai-cross"),
            ("pi", "/skill:ai-cross"),
            ("kimi", "/skill:ai-cross"),
        ];
        let ids = [
            "builtin-shared",
            "builtin-economy",
            "builtin-blind",
            "builtin-arena",
            "builtin-jury",
            "builtin-swarm",
        ];
        let mode_names = ["共享模式", "经济模式", "盲审模式", "擂台模式", "评审模式", "蜂群模式"];
        let slugs = ["shared", "economy", "blind", "arena", "jury", "swarm"];
        let keywords = ["共享", "经济", "盲审", "擂台", "评审", "蜂群"];
        for (host, invoke) in invocations {
            let value = slash_list_at(&root, &home, host).unwrap();
            let modes = value["modes"].as_array().unwrap();
            assert_eq!(modes.len(), 6, "{host}");
            for (index, mode) in modes.iter().enumerate() {
                assert_eq!(mode["mode"], ids[index], "{host}");
                assert_eq!(mode["modeName"], mode_names[index], "{host}");
                assert_eq!(
                    mode["argForm"],
                    format!("{invoke} {}", keywords[index]),
                    "{host}"
                );
                assert_eq!(
                    mode["suggestedName"],
                    format!("ax-{}", slugs[index]),
                    "{host}"
                );
                assert!(mode["command"].is_null(), "{host} 尚无命令时 command 为 null");
            }
        }
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// modes 的 command：取 commands 里 mode 等于该模式的第一条；同模式
    /// 后续命令不进 modes，别的模式仍为 null。
    #[test]
    fn task54_list_modes_command_matches_first_of_mode() {
        let root = temp_dir("match-root");
        let home = temp_dir("match-home");
        slash_add_at(&root, &home, "claude", "first", "blind", "").unwrap();
        slash_add_at(&root, &home, "claude", "second", "blind", "").unwrap();
        let value = slash_list_at(&root, &home, "claude").unwrap();
        let modes = value["modes"].as_array().unwrap();
        let blind = modes
            .iter()
            .find(|mode| mode["mode"] == "builtin-blind")
            .unwrap();
        assert_eq!(blind["command"]["name"], "first");
        assert_eq!(blind["command"]["trigger"], "/first");
        let shared = modes
            .iter()
            .find(|mode| mode["mode"] == "builtin-shared")
            .unwrap();
        assert!(shared["command"].is_null());
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// add-all 全新生成 6 条：added 全列、skipped 空、文件都在、modes 的
    /// command 都有了；文件内容与手写 add 一致（同一实现体）；换前缀与
    /// 空前缀照常。
    #[test]
    fn task54_add_all_generates_every_builtin_mode() {
        let root = temp_dir("addall-root");
        let home = temp_dir("addall-home");
        let value = slash_add_all_at(&root, &home, "claude", "ax-").unwrap();
        let added: Vec<String> = value["added"]
            .as_array()
            .unwrap()
            .iter()
            .map(|name| name.as_str().unwrap().to_string())
            .collect();
        assert_eq!(added.len(), 6, "{added:?}");
        assert_eq!(value["skipped"].as_array().unwrap().len(), 0);
        for slug in ["shared", "economy", "blind", "arena", "jury", "swarm"] {
            assert!(added.contains(&format!("ax-{slug}")), "{added:?}");
            assert!(command_path(&home, "claude", &format!("ax-{slug}")).is_file());
        }
        assert_eq!(value["commands"].as_array().unwrap().len(), 6);
        for mode in value["modes"].as_array().unwrap() {
            assert!(!mode["command"].is_null(), "{mode}");
        }
        let text = std::fs::read_to_string(command_path(&home, "claude", "ax-blind")).unwrap();
        assert!(text.contains("「盲审模式」"), "{text}");
        // 换一个前缀与另一个 host（互不影响）也能全量生成。
        let value = slash_add_all_at(&root, &home, "pi", "review-").unwrap();
        assert_eq!(value["added"].as_array().unwrap().len(), 6);
        assert!(command_path(&home, "pi", "review-blind").is_file());
        // 空前缀合法：名字就是短名。
        let value = slash_add_all_at(&root, &home, "codex", "").unwrap();
        assert_eq!(value["added"].as_array().unwrap().len(), 6);
        assert!(command_path(&home, "codex", "blind").is_file());
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// 该模式已有命令（任何名字）时跳过该模式，其余模式照常生成；已有
    /// 命令的名字、模式与说明都不变。
    #[test]
    fn task54_add_all_skips_modes_that_already_have_commands() {
        let root = temp_dir("skip-root");
        let home = temp_dir("skip-home");
        slash_add_at(&root, &home, "claude", "cross-review", "blind", "只报问题").unwrap();
        let value = slash_add_all_at(&root, &home, "claude", "ax-").unwrap();
        let added: Vec<String> = value["added"]
            .as_array()
            .unwrap()
            .iter()
            .map(|name| name.as_str().unwrap().to_string())
            .collect();
        assert_eq!(added.len(), 5, "{added:?}");
        assert!(!added.contains(&"ax-blind".to_string()), "{added:?}");
        let skipped = value["skipped"].as_array().unwrap();
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0]["name"], "ax-blind");
        assert_eq!(skipped[0]["reason"], "该模式已有短命令：cross-review");
        let commands = value["commands"].as_array().unwrap();
        assert_eq!(commands.len(), 6);
        let kept = commands
            .iter()
            .find(|command| command["name"] == "cross-review")
            .unwrap();
        assert_eq!(kept["mode"], "builtin-blind");
        assert_eq!(kept["note"], "只报问题");
        assert_eq!(
            commands
                .iter()
                .filter(|command| command["mode"] == "builtin-blind")
                .count(),
            1
        );
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// 生成名已被别的模式的定义占用：不改写，该模式记入 skipped，原因
    /// 写明被哪个模式占用；那条定义与文件都原样，其余模式照常生成。
    #[test]
    fn task54_add_all_skips_names_taken_by_other_mode() {
        let root = temp_dir("taken-root");
        let home = temp_dir("taken-home");
        // ax- 这个名字先给了经济模式。
        slash_add_at(&root, &home, "claude", "ax-blind", "economy", "只报问题").unwrap();
        let value = slash_add_all_at(&root, &home, "claude", "ax-").unwrap();
        let added: Vec<String> = value["added"]
            .as_array()
            .unwrap()
            .iter()
            .map(|name| name.as_str().unwrap().to_string())
            .collect();
        // 经济模式已有命令 → 跳过；盲审的生成名被它占用 → 同样跳过。
        assert_eq!(added, ["ax-shared", "ax-arena", "ax-jury", "ax-swarm"], "{added:?}");
        let skipped = value["skipped"].as_array().unwrap();
        assert_eq!(skipped.len(), 2, "{skipped:?}");
        let blind_skip = skipped
            .iter()
            .find(|item| item["name"] == "ax-blind")
            .unwrap();
        assert_eq!(blind_skip["reason"], "名称已被 经济模式 使用");
        let economy_skip = skipped
            .iter()
            .find(|item| item["name"] == "ax-economy")
            .unwrap();
        assert_eq!(economy_skip["reason"], "该模式已有短命令：ax-blind");
        // 文件与定义都原样：还是经济模式的。
        let text = std::fs::read_to_string(command_path(&home, "claude", "ax-blind")).unwrap();
        assert!(text.contains("「经济模式」") && text.contains("只报问题"), "{text}");
        let modes = orchestration::modes_from(&root);
        let definition = orchestration::host_section(&modes, "claude")
            .unwrap()
            .slash_commands
            .iter()
            .find(|command| command.name == "ax-blind")
            .unwrap();
        assert_eq!(definition.mode, "builtin-economy");
        // 盲审被跳过后没有自己的命令，modes 里 command 仍为 null。
        let blind = value["modes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|mode| mode["mode"] == "builtin-blind")
            .unwrap();
        assert!(blind["command"].is_null());
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// 目标名字被别人的文件占用时：该模式记入 skipped，其余模式照常写入，
    /// 别人的文件原样。
    #[test]
    fn task54_add_all_records_occupied_names_as_skipped() {
        let root = temp_dir("allocc-root");
        let home = temp_dir("allocc-home");
        let path = command_path(&home, "claude", "ax-arena");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "别人的命令").unwrap();
        let value = slash_add_all_at(&root, &home, "claude", "ax-").unwrap();
        let added: Vec<String> = value["added"]
            .as_array()
            .unwrap()
            .iter()
            .map(|name| name.as_str().unwrap().to_string())
            .collect();
        assert_eq!(added.len(), 5, "{added:?}");
        assert!(!added.contains(&"ax-arena".to_string()), "{added:?}");
        let skipped = value["skipped"].as_array().unwrap();
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0]["name"], "ax-arena");
        assert!(
            skipped[0]["reason"]
                .as_str()
                .unwrap()
                .contains("已存在同名命令（非 aicross 管理）"),
            "{}",
            skipped[0]["reason"]
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "别人的命令");
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// 非法前缀退出 2：字符不认，或拼出的名字超长；都发生在写入之前。
    /// 空前缀合法（名字就是短名）。
    #[test]
    fn task54_add_all_invalid_prefix_exits_2() {
        let root = temp_dir("prefix-root");
        let home = temp_dir("prefix-home");
        for bad in ["AX-", "带汉字", "带 空格", "点."] {
            let error = slash_add_all_at(&root, &home, "claude", bad).unwrap_err();
            assert_eq!(error.0, 2, "{bad}");
            assert!(error.1.contains("前缀"), "{bad}: {}", error.1);
        }
        // 前缀本身不超长，但拼出的名字超长，同样退出 2。
        let error = slash_add_all_at(&root, &home, "claude", &"a".repeat(30)).unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("命令名"), "{}", error.1);
        assert!(!home.join(".claude").exists(), "校验失败不应写入任何文件");
        // 空前缀：名字就是短名，六个都合法。
        let value = slash_add_all_at(&root, &home, "claude", "").unwrap();
        assert_eq!(value["added"].as_array().unwrap().len(), 6);
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// modes.json 损坏时 add-all 退出 1、不写任何文件（与单条 add 同一分法）。
    #[test]
    fn task54_add_all_corrupt_modes_blocks_writes() {
        let root = temp_dir("allcorrupt-root");
        let home = temp_dir("allcorrupt-home");
        std::fs::write(root.join("modes.json"), "{ not json").unwrap();
        let error = slash_add_all_at(&root, &home, "claude", "ax-").unwrap_err();
        assert_eq!(error.0, 1);
        assert!(error.1.contains("已停止写入"), "{}", error.1);
        assert!(!home.join(".claude").exists());
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// 走查 #14：该模式还没登记命令、但建议名的命令文件已在磁盘上时，
    /// modes 的 existing 给出文件状态（带标记 = 已写入，无标记 = 被占用，
    /// 不存在 = null）；登记后 existing 归 null、command 有值；add 对带
    /// 标记的已有文件照旧覆盖登记，无标记的照旧拒绝。
    #[test]
    fn task57_mode_existing_reports_unregistered_files_on_disk() {
        let root = temp_dir("exist-root");
        let home = temp_dir("exist-home");
        // ax-blind：带我们标记的遗留文件（重装 / 换数据目录后常见）。
        let blind_path = command_path(&home, "claude", "ax-blind");
        std::fs::create_dir_all(blind_path.parent().unwrap()).unwrap();
        std::fs::write(&blind_path, "---\ndescription: 旧文件\naicross-managed: true\n---\n旧正文\n").unwrap();
        // ax-arena：无标记，是别人的文件。
        let arena_path = command_path(&home, "claude", "ax-arena");
        std::fs::create_dir_all(arena_path.parent().unwrap()).unwrap();
        std::fs::write(&arena_path, "别人的命令\n").unwrap();
        let value = slash_list_at(&root, &home, "claude").unwrap();
        let mode_of = |value: &Value, id: &str| {
            value["modes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|mode| mode["mode"] == id)
                .unwrap()
                .clone()
        };
        let blind = mode_of(&value, "builtin-blind");
        assert_eq!(blind["existing"]["name"], "ax-blind");
        assert_eq!(blind["existing"]["state"], STATE_WRITTEN);
        assert_eq!(blind["existing"]["path"], blind_path.to_string_lossy().as_ref());
        let arena = mode_of(&value, "builtin-arena");
        assert_eq!(arena["existing"]["state"], STATE_OCCUPIED);
        assert_eq!(arena["existing"]["path"], arena_path.to_string_lossy().as_ref());
        for id in ["builtin-shared", "builtin-economy", "builtin-jury", "builtin-swarm"] {
            assert!(mode_of(&value, id)["existing"].is_null(), "{id} 无文件时应为 null");
        }
        // 登记后（add 覆盖带标记的已有文件）：existing 归 null，command 有值，
        // 文件内容换成我们的。
        let value = slash_add_at(&root, &home, "claude", "ax-blind", "blind", "").unwrap();
        let blind = mode_of(&value, "builtin-blind");
        assert!(blind["existing"].is_null(), "登记后 existing 应为 null：{blind}");
        assert_eq!(blind["command"]["name"], "ax-blind");
        let text = std::fs::read_to_string(&blind_path).unwrap();
        assert!(text.contains("「盲审模式」") && text.contains("aicross-managed: true"), "{text}");
        // 无标记的照旧拒绝：文件原样，定义不写入。
        let error = slash_add_at(&root, &home, "claude", "ax-arena", "builtin-arena", "").unwrap_err();
        assert_eq!(error.0, 1);
        assert!(error.1.contains("已存在同名命令（非 aicross 管理）"), "{}", error.1);
        assert_eq!(std::fs::read_to_string(&arena_path).unwrap(), "别人的命令\n");
        let value = slash_list_at(&root, &home, "claude").unwrap();
        assert_eq!(mode_of(&value, "builtin-arena")["existing"]["state"], STATE_OCCUPIED);
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// fix-57-r1 3：kimi 的 existing 与 slash add 同一套占用判定——
    /// `~/.agents/skills/<名>` 已存在，或 `~/.kimi-code/skills/<名>/` 目录
    /// 已在而没有我们带标记的 SKILL.md，都报被占用；add 对同一路径同样
    /// 拒绝。命令文件带标记时不算占用，仍是已写入。
    #[test]
    fn task57_kimi_existing_follows_add_occupancy_rule() {
        let root = temp_dir("kimiexist-root");
        let home = temp_dir("kimiexist-home");
        // ax-blind：~/.agents/skills 同名目录已存在（我们从不往那里写）。
        let agents_dir = home.join(".agents").join("skills").join("ax-blind");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(agents_dir.join("SKILL.md"), "别人的 skill\n").unwrap();
        // ax-arena：kimi 命令目录已在，只有别人的文件。
        let arena_dir = command_path(&home, "kimi", "ax-arena")
            .parent()
            .unwrap()
            .to_path_buf();
        std::fs::create_dir_all(&arena_dir).unwrap();
        std::fs::write(arena_dir.join("references.md"), "别人的\n").unwrap();
        let mode_of = |value: &Value, id: &str| {
            value["modes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|mode| mode["mode"] == id)
                .unwrap()
                .clone()
        };
        let value = slash_list_at(&root, &home, "kimi").unwrap();
        let blind = mode_of(&value, "builtin-blind");
        assert_eq!(blind["existing"]["state"], STATE_OCCUPIED);
        assert_eq!(blind["existing"]["path"], agents_dir.to_string_lossy().as_ref());
        let arena = mode_of(&value, "builtin-arena");
        assert_eq!(arena["existing"]["state"], STATE_OCCUPIED);
        assert_eq!(arena["existing"]["path"], arena_dir.to_string_lossy().as_ref());
        // add 用同一套判定：同一路径同样拒绝，别人的文件原样。
        let error = slash_add_at(&root, &home, "kimi", "ax-blind", "builtin-blind", "").unwrap_err();
        assert_eq!(error.0, 1);
        assert!(error.1.contains(&*agents_dir.to_string_lossy()), "{}", error.1);
        let error = slash_add_at(&root, &home, "kimi", "ax-arena", "builtin-arena", "").unwrap_err();
        assert_eq!(error.0, 1);
        assert!(error.1.contains(&*arena_dir.to_string_lossy()), "{}", error.1);
        assert_eq!(
            std::fs::read_to_string(agents_dir.join("SKILL.md")).unwrap(),
            "别人的 skill\n"
        );
        // 命令文件带我们的标记时不算占用：已写入，add 可直接登记。
        let shared_path = command_path(&home, "kimi", "ax-shared");
        std::fs::create_dir_all(shared_path.parent().unwrap()).unwrap();
        std::fs::write(
            &shared_path,
            "---\ndescription: 旧\naicross-managed: true\n---\n旧正文\n",
        )
        .unwrap();
        let value = slash_list_at(&root, &home, "kimi").unwrap();
        assert_eq!(mode_of(&value, "builtin-shared")["existing"]["state"], STATE_WRITTEN);
        slash_add_at(&root, &home, "kimi", "ax-shared", "builtin-shared", "").unwrap();
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// 建目录链接：Windows 用 junction（mklink /J），Unix 用 symlink；
    /// 建不了返回 false，调用方跳过用例并注明。
    #[cfg(windows)]
    fn create_dir_link(link: &Path, target: &Path) -> bool {
        std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J", &link.to_string_lossy(), &target.to_string_lossy()])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[cfg(not(windows))]
    fn create_dir_link(link: &Path, target: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    /// 建文件 symlink；Windows 没有开发者模式 / 权限时建不了，返回 false。
    fn create_file_link(link: &Path, target: &Path) -> bool {
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(target, link).is_ok()
        }
        #[cfg(not(windows))]
        {
            std::os::unix::fs::symlink(target, link).is_ok()
        }
    }

    const MARKED: &str = "---\ndescription: 外部\naicross-managed: true\n---\n外部正文\n";

    /// fix-fullcheck A2：kimi 的 `~/.kimi-code/skills/<名>/` 是指向外部目录的
    /// junction / symlink，外部目录里的 SKILL.md 带我们的标记：list 报被
    /// 占用，add 拒绝且不覆盖，remove 与 preset remove 共用的删除不删外部
    /// 文件。建不了目录链接的平台跳过。
    #[test]
    fn a2_kimi_linked_command_dir_is_external() {
        let root = temp_dir("a2-kimi-root");
        let home = temp_dir("a2-kimi-home");
        let outside = temp_dir("a2-kimi-outside");
        std::fs::write(outside.join("SKILL.md"), MARKED).unwrap();
        let skills = home.join(".kimi-code").join("skills");
        std::fs::create_dir_all(&skills).unwrap();
        if !create_dir_link(&skills.join("x"), &outside) {
            eprintln!("跳过：建不了目录链接");
            for dir in [root, home, outside] {
                let _ = std::fs::remove_dir_all(dir);
            }
            return;
        }
        let target = command_path(&home, "kimi", "x");
        assert_eq!(file_state(&target), STATE_OCCUPIED);
        let error = slash_add_at(&root, &home, "kimi", "x", "blind", "").unwrap_err();
        assert_eq!(error.0, 1, "{}", error.1);
        assert_eq!(std::fs::read_to_string(outside.join("SKILL.md")).unwrap(), MARKED);
        let error = slash_remove_at(&root, &home, "kimi", "x").unwrap_err();
        assert_eq!(error.0, 2, "{}", error.1);
        assert_eq!(remove_marked_command_file(&home, "kimi", "x"), Ok(false));
        assert_eq!(std::fs::read_to_string(outside.join("SKILL.md")).unwrap(), MARKED);
        let _ = std::fs::remove_dir(skills.join("x"));
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
        let _ = std::fs::remove_dir_all(outside);
    }

    /// fix-fullcheck A2：悬空的目录链接（指向已不存在的目录）也按占用处理，
    /// add 不沿链接新建外部目录与文件。建不了目录链接的平台跳过。
    #[test]
    fn a2_dangling_command_dir_link_is_occupied() {
        let root = temp_dir("a2-dangling-root");
        let home = temp_dir("a2-dangling-home");
        let outside = temp_dir("a2-dangling-outside");
        let skills = home.join(".kimi-code").join("skills");
        std::fs::create_dir_all(&skills).unwrap();
        if !create_dir_link(&skills.join("x"), &outside) {
            eprintln!("跳过：建不了目录链接");
            for dir in [root, home, outside] {
                let _ = std::fs::remove_dir_all(dir);
            }
            return;
        }
        std::fs::remove_dir_all(&outside).unwrap();
        let target = command_path(&home, "kimi", "x");
        assert_eq!(file_state(&target), STATE_OCCUPIED);
        let error = slash_add_at(&root, &home, "kimi", "x", "blind", "").unwrap_err();
        assert_eq!(error.0, 1, "{}", error.1);
        assert!(!outside.exists(), "不沿悬空链接建外部目录");
        // claude 的命令目录本身是悬空链接时同样按占用。
        let commands = home.join(".claude").join("commands");
        std::fs::create_dir_all(commands.parent().unwrap()).unwrap();
        let gone = temp_dir("a2-dangling-gone");
        assert!(create_dir_link(&commands, &gone));
        std::fs::remove_dir_all(&gone).unwrap();
        assert_eq!(file_state(&command_path(&home, "claude", "y")), STATE_OCCUPIED);
        let _ = std::fs::remove_dir(skills.join("x"));
        let _ = std::fs::remove_dir(&commands);
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }

    /// fix-fullcheck A2：命令文件本身是指向外部带标记文件的 symlink：
    /// 按占用处理，add 不覆盖、remove 不删。Windows 无 symlink 权限时跳过。
    #[test]
    fn a2_symlinked_command_file_is_external() {
        let root = temp_dir("a2-file-root");
        let home = temp_dir("a2-file-home");
        let outside = temp_dir("a2-file-outside");
        let external = outside.join("cross.md");
        std::fs::write(&external, MARKED).unwrap();
        let target = command_path(&home, "claude", "cross");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        if !create_file_link(&target, &external) {
            eprintln!("跳过：建不了文件 symlink");
            for dir in [root, home, outside] {
                let _ = std::fs::remove_dir_all(dir);
            }
            return;
        }
        assert_eq!(file_state(&target), STATE_OCCUPIED);
        let error = slash_add_at(&root, &home, "claude", "cross", "blind", "").unwrap_err();
        assert_eq!(error.0, 1, "{}", error.1);
        assert_eq!(remove_marked_command_file(&home, "claude", "cross"), Ok(false));
        assert_eq!(std::fs::read_to_string(&external).unwrap(), MARKED);
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
        let _ = std::fs::remove_dir_all(outside);
    }

    #[test]
    fn extra_hosts_have_invoke_but_no_short_commands() {
        let root = temp_dir("extra-root");
        let home = temp_dir("extra-home");
        let value = slash_list_at(&root, &home, "qoder").unwrap();
        assert_eq!(value["invoke"], "/ai-cross");
        assert_eq!(value["supported"], false);
        assert_eq!(slash_list_at(&root, &home, "workbuddy").unwrap()["invoke"], "用 ai-cross");
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(home);
    }
}
