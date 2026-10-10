//! ai-cross skill 的内置包与安装（TASK-50 第 4 节）。
//!
//! 内置包是 `skill-bundle/ai-cross/`（skill 源仓去掉 tests 与 .git* 的副本，
//! 由 scripts/sync-skill.py 同步），编译期嵌入可执行文件；版本号取
//! SKILL.md frontmatter 的 metadata.version。
//!
//! 位置判定只有三种：none（不存在）、ours（普通目录且含我们的标记文件
//! .aicross-install.json）、external（是链接，或没有标记文件——一律按
//! 「由其他软件管理」处理，不覆盖、不更新、不删除）。链接含 Windows 的
//! junction（skillmanager 用它）。
//!
//! 家目录与项目目录一律由调用方注入：生产用 `paths::home()`，测试一律用
//! 临时目录，绝不写真实家目录。

use include_dir::{Dir, DirEntry};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// 编译期嵌入的 skill 包。目录缺失时编译失败——同步脚本是本仓库的
/// 常规步骤（scripts/sync-skill.py），skill-bundle 随仓库提交。
static SKILL_BUNDLE: Dir = include_dir::include_dir!("$CARGO_MANIFEST_DIR/skill-bundle/ai-cross");

pub(crate) const MARKER_FILE: &str = ".aicross-install.json";
pub(crate) const KIND_NONE: &str = "none";
pub(crate) const KIND_OURS: &str = "ours";
pub(crate) const KIND_EXTERNAL: &str = "external";
const INSTALL_SOURCE: &str = "aicross";

/// 安装临时目录与备份目录名前缀。两者都建在暂存目录（见 [`staging_dir`]）
/// 里，目录下一层才是 `ai-cross/`，残留不会被 agent 当成第二份 skill。
/// 早期版本把它们直接放在 skills 目录或其上一级、目录本身就是 skill，
/// 恢复与清理时这两处也一并认（旧格式）。
const TEMP_PREFIX: &str = ".aicross-install-tmp-";
const BACKUP_PREFIX: &str = ".aicross-install-bak-";
/// 暂存目录名前缀，后接 skills 目录的真实名字（`.aicross-install-skills`）。
const STAGING_PREFIX: &str = ".aicross-install-";
/// 暂存目录里的跨进程锁文件名（系统文件锁，文件常驻）。
const INSTALL_LOCK_FILE: &str = "install.lock";
/// 新格式残留根目录里的操作记录：写明这份临时 / 备份属于哪个目标，
/// 清理与恢复只认记录里目标与本次一致的。
const OPERATION_RECORD: &str = ".aicross-op.json";
/// 安装锁的等待上限：一次安装 / 卸载只写几百 KB，正常持锁在秒级以内；
/// 内置包大到写入要几十秒时同步调大。
const INSTALL_LOCK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(40);

/// skill 写入的进程内锁（fix-50-r2）：codex、kimi、pi 的写入位置同为
/// `.agents/skills`，窗口与命令行在同一进程里并发 install / uninstall 会
/// 互相踩。跨进程由暂存目录里的锁文件互斥（[`INSTALL_LOCK_FILE`]）；状态
/// 查询只读，两把锁都不拿。
pub(crate) fn skill_write_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 一个位置的判定结果；窗口与命令行同此结构（camelCase）。
/// `write_target` 表示这个位置是不是「我们安装时写入的位置」（前端据此
/// 决定按钮状态，不按路径猜）；`outdated` 表示这个位置是别人管理的
/// （external）且版本可解析、低于内置版本（走查 #11），我们装的那份用
/// host 级的 `updateAvailable`；`manager` 表示外部位置的管理方（TASK-59）：
/// `skills.sh` / `skills-manager`，非外部与识别不了时为空串。
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillLocation {
    pub scope: String,
    pub path: String,
    pub kind: String,
    pub version: String,
    pub outdated: bool,
    pub manager: String,
    pub link_target: Option<String>,
    pub read_by_this_host: bool,
    pub write_target: bool,
}

/// manager 判定所需的上下文（TASK-59）：家目录与项目目录由调用方注入，
/// 测试一律用临时目录；`xdg_state_home` 是 XDG_STATE_HOME 的值（生产读
/// 进程环境，测试注入固定值或 None），未设或为空时全局锁文件回家目录。
#[derive(Clone, Copy)]
struct ScanContext<'a> {
    home: &'a Path,
    project: Option<&'a Path>,
    xdg_state_home: Option<&'a str>,
}

/// skills.sh 的全局锁文件：设了非空的 XDG_STATE_HOME 用
/// `$XDG_STATE_HOME/skills/.skill-lock.json`，否则回家目录
/// `~/.agents/.skill-lock.json`。
fn skills_sh_global_lock_path(context: &ScanContext) -> PathBuf {
    match context.xdg_state_home.filter(|value| !value.is_empty()) {
        Some(xdg) => Path::new(xdg).join("skills").join(".skill-lock.json"),
        None => context.home.join(".agents").join(".skill-lock.json"),
    }
}

/// 锁文件里 `skills` 对象的键（已登记的技能名）；读不了或格式不对当作
/// 未登记，不报错。
fn lock_skill_names(path: &Path) -> HashSet<String> {
    serde_json::from_str::<Value>(&std::fs::read_to_string(path).unwrap_or_default())
        .ok()
        .and_then(|value| {
            value
                .get("skills")
                .and_then(Value::as_object)
                .map(|object| object.keys().cloned().collect())
        })
        .unwrap_or_default()
}

/// `target` 是否在 `base` 目录下（按分量比，不是字符串前缀）；Windows
/// 路径不区分大小写，比较前统一小写。
fn path_starts_with(target: &Path, base: &Path) -> bool {
    #[cfg(windows)]
    {
        let lower = |path: &Path| PathBuf::from(path.to_string_lossy().to_lowercase());
        lower(target).starts_with(lower(base))
    }
    #[cfg(not(windows))]
    {
        target.starts_with(base)
    }
}

/// 外部位置的管理方（TASK-59）：位置是链接且目标在家目录 `.skills-manager`
/// 下的是 skills-manager（先把位置与 `.skills-manager` 都 canonicalize 再比，
/// 覆盖带 `..` 的相对目标与链式链接；canonicalize 失败时退回按链接文本
/// 判断，相对目标按链接所在目录解析）；否则全局位置在
/// skills.sh 全局锁文件、项目位置在项目根的 `skills-lock.json` 里登记了
/// `ai-cross` 的是 skills.sh；其余为空串。锁文件的登记只说明技能名，
/// 判不了具体路径，所以链接的指向优先于锁登记。
fn external_manager(path: &Path, is_link: bool, link_target: Option<&str>, scope: &str, context: &ScanContext) -> String {
    if is_link {
        let manager_root = context.home.join(".skills-manager");
        if let (Ok(real), Ok(real_root)) =
            (std::fs::canonicalize(path), std::fs::canonicalize(&manager_root))
        {
            if path_starts_with(&real, &real_root) {
                return "skills-manager".into();
            }
        } else if let Some(target) = link_target {
            let target = PathBuf::from(target);
            let resolved = if target.is_absolute() {
                target
            } else {
                path.parent()
                    .map(|directory| directory.join(&target))
                    .unwrap_or(target)
            };
            if path_starts_with(&resolved, &manager_root) {
                return "skills-manager".into();
            }
        }
    }
    let registered = match scope {
        "global" => lock_skill_names(&skills_sh_global_lock_path(context)),
        "project" => context
            .project
            .map(|directory| lock_skill_names(&directory.join("skills-lock.json")))
            .unwrap_or_default(),
        _ => HashSet::new(),
    };
    if registered.contains("ai-cross") {
        "skills.sh".into()
    } else {
        String::new()
    }
}

/// 一个主 agent 的 skill 安装状态。
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HostSkillStatus {
    pub host: String,
    pub locations: Vec<SkillLocation>,
    pub duplicate: bool,
    pub update_available: bool,
}

/// 读取位置：全局（家目录下）与项目（项目目录下）各会读哪些目录。
/// 另含旧位置（`~/.codex/skills`，codex 旧版）。写入位置单独一张表
/// （`install_path`）。
fn read_paths(host: &str, home: &Path, project: Option<&Path>) -> Vec<(String, PathBuf)> {
    fn push(paths: &mut Vec<(String, PathBuf)>, scope: &str, path: PathBuf) {
        paths.push((scope.to_string(), path));
    }
    let mut paths = Vec::new();
    match host {
        "claude" => {
            push(&mut paths, "global", home.join(".claude").join("skills").join("ai-cross"));
            if let Some(dir) = project {
                push(&mut paths, "project", dir.join(".claude").join("skills").join("ai-cross"));
            }
        }
        "codex" => {
            push(&mut paths, "global", home.join(".agents").join("skills").join("ai-cross"));
            // codex 旧版的位置，只扫描识别，不写入。
            push(&mut paths, "global", home.join(".codex").join("skills").join("ai-cross"));
            if let Some(dir) = project {
                push(&mut paths, "project", dir.join(".agents").join("skills").join("ai-cross"));
            }
        }
        "kimi" => {
            push(&mut paths, "global", home.join(".kimi-code").join("skills").join("ai-cross"));
            push(&mut paths, "global", home.join(".agents").join("skills").join("ai-cross"));
            if let Some(dir) = project {
                push(&mut paths, "project", dir.join(".kimi-code").join("skills").join("ai-cross"));
                push(&mut paths, "project", dir.join(".agents").join("skills").join("ai-cross"));
            }
        }
        _ => {
            push(&mut paths, "global", home.join(".pi").join("agent").join("skills").join("ai-cross"));
            push(&mut paths, "global", home.join(".agents").join("skills").join("ai-cross"));
            if let Some(dir) = project {
                push(&mut paths, "project", dir.join(".pi").join("skills").join("ai-cross"));
                push(&mut paths, "project", dir.join(".agents").join("skills").join("ai-cross"));
            }
        }
    }
    paths
}

/// 我们安装时写入的位置：claude 写 `.claude/skills/ai-cross`；codex、
/// kimi、pi 写 `.agents/skills/ai-cross`（全局在家目录下，项目在项目
/// 目录下）。拼路径逐段 join，Windows 上输出不混用正反斜杠（走查 #11）。
fn install_path(host: &str, home: &Path, project: Option<&Path>) -> PathBuf {
    let relative = match host {
        "claude" => Path::new(".claude").join("skills").join("ai-cross"),
        _ => Path::new(".agents").join("skills").join("ai-cross"),
    };
    match project {
        Some(dir) => dir.join(relative),
        None => home.join(relative),
    }
}

/// 判定一个位置（TASK-50 第 4 节）。链接（symlink / junction）按
/// external 并记下指向；普通目录含我们的标记文件为 ours，否则 external；
/// 路径存在但不是目录（普通文件）也按 external，不当 none。
/// `manager` 只对 external 有值（TASK-59），这个便捷版不带上下文，
/// install / uninstall 只看 kind 与 link_target，不判 manager。
fn inspect_location(path: &Path, scope: &str, read_by_this_host: bool) -> SkillLocation {
    let context = ScanContext {
        home: Path::new(""),
        project: None,
        xdg_state_home: None,
    };
    inspect_location_full(path, scope, read_by_this_host, false, &context)
}

fn inspect_location_full(
    path: &Path,
    scope: &str,
    read_by_this_host: bool,
    write_target: bool,
    context: &ScanContext,
) -> SkillLocation {
    let none = SkillLocation {
        scope: scope.to_string(),
        path: path.to_string_lossy().into_owned(),
        kind: KIND_NONE.to_string(),
        version: String::new(),
        outdated: false,
        manager: String::new(),
        link_target: None,
        read_by_this_host,
        write_target,
    };
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(_) => return none,
    };
    // Windows 的 junction 不是 symlink 元数据，但 read_link 认得；
    // 两个条件取或，覆盖 symlink 与 junction。
    let link = meta.file_type().is_symlink() || std::fs::read_link(path).is_ok();
    if link {
        // 链接指向可能存成 `D:/x` 形式（fix-57-r1 4）：输出前统一分隔符。
        let target = std::fs::read_link(path)
            .map(|target| native_separators(&target).to_string_lossy().into_owned())
            .ok();
        let version = std::fs::read_to_string(path.join("SKILL.md"))
            .map(|text| skill_version_from_text(&text))
            .unwrap_or_default();
        let outdated = is_outdated_version(&version);
        let manager = external_manager(
            path,
            true,
            target.as_deref(),
            scope,
            context,
        );
        return SkillLocation {
            kind: KIND_EXTERNAL.to_string(),
            version,
            outdated,
            manager,
            link_target: target,
            ..none
        };
    }
    if !meta.is_dir() {
        // 存在但是普通文件：不是我们装的，也不能当「不存在」。
        return SkillLocation {
            kind: KIND_EXTERNAL.to_string(),
            manager: external_manager(path, false, None, scope, context),
            ..none
        };
    }
    let version = std::fs::read_to_string(path.join("SKILL.md"))
        .map(|text| skill_version_from_text(&text))
        .unwrap_or_default();
    if path.join(MARKER_FILE).is_file() {
        SkillLocation {
            kind: KIND_OURS.to_string(),
            version,
            ..none
        }
    } else {
        let outdated = is_outdated_version(&version);
        SkillLocation {
            kind: KIND_EXTERNAL.to_string(),
            version,
            outdated,
            manager: external_manager(path, false, None, scope, context),
            ..none
        }
    }
}

/// 版本是否低于内置版本（走查 #11）：按点分数字逐段比；任一边解析不了
/// 视为不比较（false）。只有 external 位置会调（我们的那份用
/// updateAvailable）。
fn is_outdated_version(version: &str) -> bool {
    match (parse_version(version), parse_version(&bundled_version())) {
        (Some(found), Some(bundled)) => found < bundled,
        _ => false,
    }
}

/// 点分版本号逐段转数字；有空段或非数字段时整个视为解析不了。前缀相等
/// 时短的小（`Vec` 的字典序）：1.21 < 1.21.1。
fn parse_version(version: &str) -> Option<Vec<u64>> {
    if version.trim().is_empty() {
        return None;
    }
    version
        .trim()
        .split('.')
        .map(|segment| segment.parse::<u64>().ok())
        .collect()
}

/// Windows 上把路径分隔符统一成反斜杠；其它平台原样返回。传入的项目根
/// 路径与 `read_link` 读出的链接指向都可能带正斜杠（如 `D:/x` 形式），
/// 先统一再拼接、输出，避免正反斜杠混用（fix-57-r1 4）。
fn native_separators(path: &Path) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(path.to_string_lossy().replace('/', "\\"))
    } else {
        path.to_path_buf()
    }
}

/// 一个主 agent 的状态：列出全部读取位置（同一个物理位置只列一次，
/// `--project` 与家目录相同 directory 时去重，不误报 duplicate）；同一个
/// 主 agent 能读到两处以上 ai-cross 时 duplicate 为 true；我们装的那份
/// 版本与内置不一致时 update_available 为 true。XDG_STATE_HOME 读进程
/// 环境；测试注入用 [`host_status_at`]。
pub(crate) fn host_status(host: &str, home: &Path, project: Option<&Path>) -> HostSkillStatus {
    let xdg_state_home = std::env::var("XDG_STATE_HOME").ok();
    host_status_at(host, home, project, xdg_state_home.as_deref())
}

/// 同 [`host_status`]，XDG_STATE_HOME 由调用方注入（测试给固定值或 None，
/// 不碰进程环境）。
fn host_status_at(
    host: &str,
    home: &Path,
    project: Option<&Path>,
    xdg_state_home: Option<&str>,
) -> HostSkillStatus {
    let context = ScanContext {
        home,
        project,
        xdg_state_home,
    };
    // 写入位置按 scope 分别比对（fix-50-r2）：global 位置与全局写入位置
    // 比、project 位置与项目写入位置比，给了 --project 时全局位置的
    // writeTarget 不再算错。
    let global_target = install_path(host, home, None);
    let project_target = project.map(|dir| install_path(host, home, Some(dir)));
    // 只读：残留的临时 / 备份目录可能属于另一个进程正在进行的安装，或是
    // 中断更新后唯一能恢复的旧安装，状态查询一律不动，由下一次安装 /
    // 卸载在锁内处理（[`recover_leftovers`]）。
    let paths = read_paths(host, home, project);
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let locations = paths
        .into_iter()
        .filter_map(|(scope, path)| {
            // 同一物理目录只列一次：能规范化的用规范化路径，不能的
            // （不存在）用原路径，Windows 下再统一小写。
            let key = std::fs::canonicalize(&path).unwrap_or_else(|_| dedupe_key(&path));
            if !seen.insert(key) {
                return None;
            }
            let write_target = if scope == "global" {
                path == global_target
            } else {
                project_target.as_deref() == Some(&path)
            };
            Some(inspect_location_full(&path, &scope, true, write_target, &context))
        })
        .collect::<Vec<_>>();
    let installed = locations
        .iter()
        .filter(|location| location.kind != KIND_NONE)
        .count();
    let update_available = locations.iter().any(|location| {
        location.kind == KIND_OURS && location.version != bundled_version()
    });
    HostSkillStatus {
        host: host.to_string(),
        duplicate: installed >= 2,
        locations,
        update_available,
    }
}

/// 去重键（路径规范化失败时用）：Windows 路径不区分大小写，统一小写。
fn dedupe_key(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from(path.to_string_lossy().to_lowercase())
    }
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}

/// 路径解析成真实位置：从自身往上找第一个能规范化的祖先（沿 symlink /
/// junction 解析），再接上还不存在的尾段。整条都不存在时原样返回。
fn resolve_existing(path: &Path) -> PathBuf {
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    let mut current = path;
    loop {
        if let Ok(real) = std::fs::canonicalize(current) {
            return tail.iter().rev().fold(real, |acc, part| acc.join(part));
        }
        match (current.parent(), current.file_name()) {
            (Some(parent), Some(name)) if !parent.as_os_str().is_empty() => {
                tail.push(name.to_os_string());
                current = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// 一个写入位置的真实 skills 目录与暂存目录。暂存目录建在真实 skills 目录
/// 的上一级（父目录被 junction 重定向到别的盘时按解析后的路径），与目标
/// 同卷，换名不跨卷；名字带隐藏前缀与 skills 目录名，不在 skills 目录里，
/// 宿主不会扫描到。真实 skills 目录已是卷根时退到它自身里面。
fn staging_dir(target: &Path) -> Result<(PathBuf, PathBuf), String> {
    let skills_dir = target
        .parent()
        .ok_or_else(|| format!("无法定位安装目录：{}", target.display()))?;
    let real_skills = resolve_existing(skills_dir);
    let name = real_skills
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    let staging = match (real_skills.parent(), name) {
        (Some(parent), Some(name)) => parent.join(format!("{STAGING_PREFIX}{name}")),
        _ => real_skills.join(format!("{STAGING_PREFIX}staging")),
    };
    Ok((real_skills, staging))
}

/// 一份残留：所在目录与 skill 内容目录（新格式是其下的 `ai-cross/`，
/// 旧格式就是目录本身）。`nested` 为真是暂存目录里的新格式。
struct Leftover {
    root: PathBuf,
    content: PathBuf,
    backup: bool,
    nested: bool,
}

/// 路径本身是不是链接（symlink 或 Windows junction）；不跟随链接，不存在为假。
fn is_link_path(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .is_ok_and(|meta| meta.file_type().is_symlink() || std::fs::read_link(path).is_ok())
}

/// 写操作记录（新格式残留根目录里）：目标路径与操作令牌。
fn write_operation_record(root: &Path, target: &Path, token: &str) -> Result<(), String> {
    std::fs::create_dir_all(root)
        .map_err(|error| format!("创建目录 {} 失败：{error}", root.display()))?;
    let record = json!({
        "target": target.to_string_lossy(),
        "token": token,
    });
    std::fs::write(root.join(OPERATION_RECORD), record.to_string())
        .map_err(|error| format!("写入操作记录失败：{error}"))
}

/// 残留是不是本次目标的、可以清理或恢复：链接一律不动；新格式要有操作
/// 记录且记录里的目标与本次一致；旧格式要有我们的安装标记。不归属时
/// 返回保留原因。
fn leftover_owner_check(item: &Leftover, target: &Path) -> Result<(), String> {
    if is_link_path(&item.root) || is_link_path(&item.content) {
        return Err("是链接，可能指向别处".to_string());
    }
    if item.nested {
        let recorded = std::fs::read_to_string(item.root.join(OPERATION_RECORD))
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .and_then(|value| value["target"].as_str().map(PathBuf::from));
        match recorded {
            Some(path) if dedupe_key(&path) == dedupe_key(target) => Ok(()),
            Some(_) => Err("操作记录里的目标不是本次位置".to_string()),
            None => Err("没有操作记录".to_string()),
        }
    } else if item.content.join(MARKER_FILE).is_file() {
        Ok(())
    } else {
        Err("没有安装标记".to_string())
    }
}

/// 列出与这个写入位置有关的残留临时 / 备份目录：暂存目录里的新格式，
/// 以及 skills 目录与其上一级里的旧格式。
fn find_leftovers(target: &Path, staging: &Path) -> Vec<Leftover> {
    let mut found = Vec::new();
    let mut scanned: HashSet<PathBuf> = HashSet::new();
    let skills_dir = target.parent();
    let legacy = skills_dir
        .into_iter()
        .chain(skills_dir.and_then(Path::parent))
        .map(|dir| (dir.to_path_buf(), false));
    for (dir, nested) in std::iter::once((staging.to_path_buf(), true)).chain(legacy) {
        if !scanned.insert(dedupe_key(&dir)) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let backup = name.starts_with(BACKUP_PREFIX);
            if !backup && !name.starts_with(TEMP_PREFIX) {
                continue;
            }
            let root = entry.path();
            let content = if nested { root.join("ai-cross") } else { root.clone() };
            found.push(Leftover { root, content, backup, nested });
        }
    }
    found
}

/// 处理上一次中断的安装 / 更新留下的残留。调用方持有进程内写锁与暂存
/// 目录的跨进程锁，此刻不会有别的安装在进行。只动归属本次目标的残留
/// （[`leftover_owner_check`]），其余原样保留，原因作为说明返回。
/// `restore` 为真（安装 / 更新）且目标缺失时，先把最新一份带标记文件的
/// 备份改回目标（中断的更新只剩这份旧安装）；恢复失败返回错误，备份原样
/// 保留。其余归属本次的残留删掉。`literal_target` 用来找旧格式残留
/// （skills 目录与其上一级），`target` 是解析后的真实路径。
fn recover_leftovers(
    target: &Path,
    literal_target: &Path,
    staging: &Path,
    restore: bool,
) -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    let leftovers: Vec<Leftover> = find_leftovers(literal_target, staging)
        .into_iter()
        .filter(|item| match leftover_owner_check(item, target) {
            Ok(()) => true,
            Err(reason) => {
                notes.push(format!("保留残留目录 {}：{reason}", item.root.display()));
                false
            }
        })
        .collect();
    if leftovers.is_empty() {
        return Ok(notes);
    }
    let target_missing = matches!(
        std::fs::symlink_metadata(target),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound
    );
    let mut restored: Option<PathBuf> = None;
    if restore && target_missing {
        let newest = leftovers
            .iter()
            .filter(|item| item.backup && item.content.join(MARKER_FILE).is_file())
            .max_by_key(|item| {
                std::fs::metadata(&item.root)
                    .and_then(|meta| meta.modified())
                    .ok()
            });
        if let Some(item) = newest {
            std::fs::rename(&item.content, target).map_err(|error| {
                format!(
                    "恢复上次中断更新的备份 {} 失败：{error}；备份原样保留",
                    item.content.display()
                )
            })?;
            restored = Some(item.root.clone());
        }
    }
    for item in &leftovers {
        // 旧格式的备份目录就是被恢复的内容本身，改名后已不在原处。
        if restored.as_deref() == Some(item.root.as_path()) && item.root == item.content {
            continue;
        }
        let _ = std::fs::remove_dir_all(&item.root);
    }
    Ok(notes)
}

/// 拿安装 / 卸载的跨进程锁（暂存目录里常驻的锁文件，系统文件锁）。
fn acquire_install_lock(staging: &Path) -> Result<crate::file_lock::FileLock, (i32, String)> {
    crate::file_lock::FileLock::acquire(&staging.join(INSTALL_LOCK_FILE), INSTALL_LOCK_TIMEOUT)
        .map_err(|message| (1, message))
}

/// 安装 / 卸载的输出：该 host 的 status，保留了不归属本次的残留时另附 `notes`。
fn write_result(host: &str, home: &Path, project: Option<&Path>, notes: Vec<String>) -> Value {
    let status = host_status(host, home, project);
    let mut value = json!({
        "bundledVersion": bundled_version(),
        "hosts": [status],
    });
    if !notes.is_empty() {
        value["notes"] = json!(notes);
    }
    value
}

/// `skill status` 的实现体；输出 `{ bundledVersion, hosts: [...] }`。只读，
/// 不拿锁、不清理残留。
pub(crate) fn status_at(home: &Path, project: Option<&Path>) -> Result<Value, (i32, String)> {
    let project = project.map(native_separators);
    if let Some(project) = project.as_deref() {
        if !project.is_dir() {
            return Err((2, format!("项目目录不存在：{}", project.display())));
        }
    }
    let hosts: Vec<HostSkillStatus> = crate::hosts::HOST_IDS
        .iter()
        .map(|host| host_status(host, home, project.as_deref()))
        .collect();
    Ok(json!({
        "bundledVersion": bundled_version(),
        "hosts": hosts,
    }))
}

/// `skill install` 的实现体：写入「我们安装时写入的位置」。external 退出
/// 1 不覆盖；ours 先删后写（更新）；none 新建。写完输出该 host 的 status。
pub(crate) fn install_at(
    home: &Path,
    host: &str,
    scope: &str,
    project: Option<&Path>,
) -> Result<Value, (i32, String)> {
    let _guard = skill_write_lock();
    let project = validate_target(host, scope, project)?;
    let target = install_path(host, home, project.as_deref());
    // 先判一次：别人管理的位置不建目录、不拿锁。
    if inspect_location(&target, "", true).kind == KIND_EXTERNAL {
        return Err((
            1,
            format!("由其他软件管理：{}", target.display()),
        ));
    }
    // 新装时 skills 目录可能还不存在，先建好，暂存目录才能按真实位置定。
    let skills_dir = target
        .parent()
        .ok_or_else(|| (1, format!("无法定位安装目录：{}", target.display())))?;
    std::fs::create_dir_all(skills_dir)
        .map_err(|error| (1, format!("创建目录 {} 失败：{error}", skills_dir.display())))?;
    let (real_skills, staging) = staging_dir(&target).map_err(|message| (1, message))?;
    let real_target = real_skills.join("ai-cross");
    let lock = acquire_install_lock(&staging)?;
    let outcome = recover_leftovers(&real_target, &target, &staging, true).and_then(|notes| {
        // 锁内再判一次：等锁期间别的进程可能已换掉这个位置。
        if inspect_location(&target, "", true).kind == KIND_EXTERNAL {
            return Err(format!("由其他软件管理：{}", target.display()));
        }
        write_bundle(&real_target, &staging).map(|()| notes)
    });
    drop(lock);
    let notes = outcome.map_err(|message| (1, message))?;
    Ok(write_result(host, home, project.as_deref(), notes))
}

/// `skill uninstall` 的实现体：只删我们装的那份；external 退出 1 不动；
/// none 幂等成功。
pub(crate) fn uninstall_at(
    home: &Path,
    host: &str,
    scope: &str,
    project: Option<&Path>,
) -> Result<Value, (i32, String)> {
    let _guard = skill_write_lock();
    let project = validate_target(host, scope, project)?;
    let target = install_path(host, home, project.as_deref());
    let external = || Err((1, format!("由其他软件管理：{}", target.display())));
    if inspect_location(&target, "", true).kind == KIND_EXTERNAL {
        return external();
    }
    // skills 目录都不存在时既没有安装也没有残留，不建目录、不拿锁。
    let mut notes = Vec::new();
    if target.parent().is_some_and(Path::is_dir) {
        let (real_skills, staging) = staging_dir(&target).map_err(|message| (1, message))?;
        let real_target = real_skills.join("ai-cross");
        let lock = acquire_install_lock(&staging)?;
        let outcome =
            recover_leftovers(&real_target, &target, &staging, false).and_then(|kept| {
                match inspect_location(&target, "", true).kind.as_str() {
                    KIND_EXTERNAL => Err(format!("由其他软件管理：{}", target.display())),
                    KIND_OURS => std::fs::remove_dir_all(&target)
                        .map_err(|error| format!("删除失败：{error}")),
                    _ => Ok(()),
                }
                .map(|()| kept)
            });
        drop(lock);
        notes = outcome.map_err(|message| (1, message))?;
    }
    Ok(write_result(host, home, project.as_deref(), notes))
}

/// host / scope / project 的公共校验：host 只认四家；scope 只认
/// global / project；项目范围必须给 --project 且目录存在。返回规范化的
/// 项目路径（全局范围为 None）。
fn validate_target(
    host: &str,
    scope: &str,
    project: Option<&Path>,
) -> Result<Option<PathBuf>, (i32, String)> {
    if !crate::hosts::HOST_IDS.contains(&host) {
        return Err((2, format!("未知 host：{host}；可选：{}", crate::hosts::HOST_IDS.join("、"))));
    }
    match scope {
        "global" => Ok(None),
        "project" => {
            let Some(project) = project else {
                return Err((2, "项目范围需要 --project".to_string()));
            };
            if !project.is_dir() {
                return Err((2, format!("项目目录不存在：{}", project.display())));
            }
            // Windows 上先统一反斜杠再拼接（fix-57-r1 4）。
            Ok(Some(native_separators(project)))
        }
        other => Err((2, format!("scope 只认 global|project：{other}"))),
    }
}

/// 把内置包写到目标位置，并放标记文件（fix-50-r2：旧目录先改名成备份，
/// 临时目录换名到位，成功后删备份；换名失败时把备份改回原名——更新中途
/// 失败旧安装原样保留，不留半成品）。`target` 是解析后的真实路径，临时
/// 目录与备份目录建在 [`staging_dir`] 给的同卷暂存目录里，内容在其下一层
/// `ai-cross/`；名字带 uuid，不重名。备份改不回原名时保留备份，下一次
/// 安装先恢复它。调用方持有进程内写锁与跨进程安装锁。
fn write_bundle(target: &Path, staging: &Path) -> Result<(), String> {
    let token = uuid::Uuid::new_v4();
    let temp_root = staging.join(format!("{TEMP_PREFIX}{token}"));
    let backup_root = staging.join(format!("{BACKUP_PREFIX}{token}"));
    let temp = temp_root.join("ai-cross");
    let backup = backup_root.join("ai-cross");
    let had_old = std::fs::symlink_metadata(target).is_ok();
    let mut keep_backup = false;
    // 两个根目录先写操作记录再放内容：中断后残留也认得出属于哪个目标。
    let token_text = token.to_string();
    let outcome = write_operation_record(&temp_root, target, &token_text)
        .and_then(|()| write_bundle_into(&temp))
        .and_then(|()| {
        if had_old {
            write_operation_record(&backup_root, target, &token_text)?;
            std::fs::rename(target, &backup)
                .map_err(|error| format!("备份旧安装失败：{error}"))?;
        }
        if let Err(error) = std::fs::rename(&temp, target) {
            // 换名失败：把备份改回原名，旧安装保持原样。
            if had_old && std::fs::rename(&backup, target).is_err() {
                keep_backup = true;
                return Err(format!(
                    "替换安装目录失败：{error}；旧安装暂存在 {}，下次安装时自动恢复",
                    backup.display()
                ));
            }
            return Err(format!("替换安装目录失败：{error}"));
        }
        Ok(())
    });
    let _ = std::fs::remove_dir_all(&temp_root);
    if !keep_backup {
        let _ = std::fs::remove_dir_all(&backup_root);
    }
    outcome
}

/// 写内置包内容与标记文件到一个空目录。
fn write_bundle_into(target: &Path) -> Result<(), String> {
    std::fs::create_dir_all(target)
        .map_err(|error| format!("创建目录 {} 失败：{error}", target.display()))?;
    write_dir(&SKILL_BUNDLE, target)?;
    let marker = json!({
        "version": bundled_version(),
        "installedAt": chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "source": INSTALL_SOURCE,
    });
    std::fs::write(
        target.join(MARKER_FILE),
        serde_json::to_string_pretty(&marker).unwrap_or_default(),
    )
    .map_err(|error| format!("写入标记文件失败：{error}"))
}

/// 递归写出内置包的目录与文件；不处理链接（内置包里没有）。
fn write_dir(dir: &Dir, target: &Path) -> Result<(), String> {
    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(sub) => {
                let sub_target = target.join(sub.path().file_name().expect("子目录有名字"));
                std::fs::create_dir_all(&sub_target)
                    .map_err(|error| format!("创建目录 {} 失败：{error}", sub_target.display()))?;
                write_dir(sub, &sub_target)?;
            }
            DirEntry::File(file) => {
                let file_target = target.join(file.path().file_name().expect("文件有名字"));
                std::fs::write(&file_target, file.contents())
                    .map_err(|error| format!("写入 {} 失败：{error}", file_target.display()))?;
            }
        }
    }
    Ok(())
}

/// 内置包的版本号（SKILL.md frontmatter 的 metadata.version）。
pub(crate) fn bundled_version() -> String {
    SKILL_BUNDLE
        .get_file("SKILL.md")
        .map(|file| skill_version_from_text(&String::from_utf8_lossy(file.contents())))
        .unwrap_or_default()
}

/// 从 SKILL.md 内容取版本号：frontmatter（开头两行 --- 之间）里
/// metadata: 块下的 version。读不出为空串。
pub(crate) fn skill_version_from_text(text: &str) -> String {
    let mut lines = text.lines();
    if !lines.next().map(|line| line.trim() == "---").unwrap_or(false) {
        return String::new();
    }
    let mut in_metadata = false;
    for line in lines {
        let trimmed = line.trim_end();
        if trimmed == "---" {
            return String::new();
        }
        if trimmed == "metadata:" {
            in_metadata = true;
            continue;
        }
        if in_metadata {
            if !line.starts_with(' ') && !line.starts_with('\t') {
                return String::new();
            }
            let value = trimmed.trim().strip_prefix("version:").map(str::trim);
            if let Some(version) = value {
                if !version.is_empty() {
                    return version.trim_matches('"').to_string();
                }
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn temp_home(tag: &str) -> PathBuf {
        let home = crate::paths::test_temp_dir().join(format!("skill-{tag}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&home).unwrap();
        home
    }

    fn write_fake_skill(path: &Path, version: &str) {
        std::fs::create_dir_all(path).unwrap();
        std::fs::write(
            path.join("SKILL.md"),
            format!("---\nname: ai-cross\nmetadata:\n  version: {version}\n---\n\n# ai-cross\n"),
        )
        .unwrap();
    }

    /// 建目录链接：Windows 用 junction（mklink /J），Unix 用 symlink；
    /// 建不了返回 false，调用方跳过用例并注明。
    #[cfg(windows)]
    fn create_dir_link(link: &Path, target: &Path) -> bool {
        std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                &link.to_string_lossy(),
                &target.to_string_lossy(),
            ])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[cfg(not(windows))]
    fn create_dir_link(link: &Path, target: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    /// 路径后缀断言用：按分量比，Windows 反斜杠与 Unix 斜杠都认。
    fn ends_with_path(path: &str, suffix: &str) -> bool {
        Path::new(path).ends_with(suffix)
    }

    #[test]
    fn bundled_version_reads_frontmatter_metadata() {
        let version = bundled_version();
        assert!(!version.is_empty(), "内置包里要有 SKILL.md 的版本号");
        assert!(
            version.split('.').count() >= 2,
            "版本号形如 1.21.1：{version}"
        );
        // 解析规则本体：正常、引号包裹、缺 metadata、无 frontmatter。
        assert_eq!(skill_version_from_text("---\nmetadata:\n  version: 2.0.0\n---\n"), "2.0.0");
        assert_eq!(skill_version_from_text("---\nmetadata:\n  version: \"2.0.1\"\n---\n"), "2.0.1");
        assert_eq!(skill_version_from_text("---\nname: ai-cross\n---\n"), "");
        assert_eq!(skill_version_from_text("# 无 frontmatter"), "");
        // metadata 块结束后不再找。
        assert_eq!(
            skill_version_from_text("---\nmetadata:\n  version: 1.0.0\nname: x\nversion: 9.9.9\n---\n"),
            "1.0.0"
        );
    }

    #[test]
    fn location_kinds_none_ours_and_external() {
        let home = temp_home("kinds");
        // none：不存在。
        let location = inspect_location(&home.join(".claude/skills/ai-cross"), "global", true);
        assert_eq!(location.kind, KIND_NONE);
        // ours：普通目录 + 标记文件。
        let ours = home.join(".claude/skills/ai-cross");
        write_fake_skill(&ours, "1.0.0");
        std::fs::write(ours.join(MARKER_FILE), "{}").unwrap();
        let location = inspect_location(&ours, "global", true);
        assert_eq!(location.kind, KIND_OURS);
        assert_eq!(location.version, "1.0.0");
        assert_eq!(location.link_target, None);
        assert!(!location.write_target);
        // external：没有标记文件的普通目录。
        let external = home.join(".claude/skills/manual");
        write_fake_skill(&external, "9.9.9");
        let location = inspect_location(&external, "global", true);
        assert_eq!(location.kind, KIND_EXTERNAL);
        assert_eq!(location.version, "9.9.9");
        // 路径存在但是普通文件：按 external，不当 none（fix-50-r1）。
        let plain_file = home.join(".claude/skills/ai-cross-file");
        std::fs::write(&plain_file, "不是目录").unwrap();
        let location = inspect_location(&plain_file, "global", true);
        assert_eq!(location.kind, KIND_EXTERNAL);
        assert_eq!(location.version, "");
        let _ = std::fs::remove_dir_all(home);
    }

    #[cfg(windows)]
    #[test]
    fn junction_counts_as_external_with_target() {
        let home = temp_home("junction");
        let real = home.join("real-skill");
        write_fake_skill(&real, "3.0.0");
        let link = home.join("link-skill");
        // mklink /J 建目录 junction；cmd 转义经 PowerShell 执行。
        let output = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                &link.to_string_lossy(),
                &real.to_string_lossy(),
            ])
            .output()
            .expect("mklink 可用");
        assert!(output.status.success(), "junction 创建失败");
        let location = inspect_location(&link, "global", true);
        assert_eq!(location.kind, KIND_EXTERNAL);
        assert!(location.link_target.is_some(), "{:?}", location.link_target);
        // linkTarget 同样统一分隔符，不混用斜杠（fix-57-r1 4）。
        let target = location.link_target.as_deref().unwrap();
        assert!(!target.contains('/'), "linkTarget 应统一反斜杠：{target}");
        assert_eq!(location.version, "3.0.0");
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn status_reports_duplicate_and_update_available() {
        let home = temp_home("status");
        let bundled = bundled_version();
        // codex 能读到两处（全局 .agents 与旧位置 .codex）→ duplicate。
        // ours 先放好标记文件，版本同内置：不提示可更新。
        write_fake_skill(&home.join(".agents/skills/ai-cross"), &bundled);
        std::fs::write(home.join(".agents/skills/ai-cross").join(MARKER_FILE), "{}").unwrap();
        write_fake_skill(&home.join(".codex/skills/ai-cross"), "0.0.1");
        let status = host_status("codex", &home, None);
        assert!(status.duplicate, "{:?}", status.locations);
        assert!(!status.update_available, "ours 版本同内置：{:?}", status.locations);
        // ours 版本落后（标记文件原样保留）→ update_available。
        std::fs::write(
            home.join(".agents/skills/ai-cross").join("SKILL.md"),
            "---\nname: ai-cross\nmetadata:\n  version: 0.0.1\n---\n",
        )
        .unwrap();
        let status = host_status("codex", &home, None);
        assert!(status.update_available, "{:?}", status.locations);
        // 位置带 scope、readByThisHost 与 writeTarget；旧位置也在清单里。
        let agents = status
            .locations
            .iter()
            .find(|location| ends_with_path(&location.path, ".agents/skills/ai-cross"))
            .unwrap();
        assert_eq!(agents.scope, "global");
        assert!(agents.read_by_this_host);
        assert!(agents.write_target, "codex 写 .agents/skills");
        assert!(status
            .locations
            .iter()
            .any(|location| ends_with_path(&location.path, ".codex/skills/ai-cross")));
        // 非写入位置 writeTarget 为 false。
        assert!(status
            .locations
            .iter()
            .filter(|location| ends_with_path(&location.path, ".codex/skills/ai-cross"))
            .all(|location| !location.write_target));
        let _ = std::fs::remove_dir_all(home);
    }

    /// 走查 #11：外部安装的版本低于内置版本时 outdated 为 true；版本相同
    /// 或更高、解析不了时不提示；我们装的那份不用 outdated（用
    /// updateAvailable）。
    #[test]
    fn task57_external_location_below_bundled_is_outdated() {
        let home = temp_home("outdated");
        let bundled = bundled_version();
        let cases: [(&str, bool); 4] = [
            // 比内置低一个补丁号：走查里那条（外部 1.21.0，内置 1.21.1）。
            ("1.21.0", true),
            (&bundled, false),
            ("9.9.9", false),
            ("abc", false),
        ];
        for (index, (version, expected)) in cases.iter().enumerate() {
            let target = home
                .join(".claude")
                .join("skills")
                .join(format!("ai-cross-{index}"));
            write_fake_skill(&target, version);
            let location = inspect_location(&target, "global", true);
            assert_eq!(location.kind, KIND_EXTERNAL);
            assert_eq!(location.outdated, *expected, "version {version}");
        }
        // 普通文件（读不到版本）与不存在（none）不标 outdated。
        let plain = home.join(".claude").join("skills").join("ai-cross-file");
        std::fs::write(&plain, "不是目录").unwrap();
        assert!(!inspect_location(&plain, "global", true).outdated);
        assert!(!inspect_location(&home.join(".claude").join("skills").join("missing"), "global", true).outdated);
        // 我们的旧版本不标 outdated，走 host 级 updateAvailable。
        let ours = home.join(".claude").join("skills").join("ai-cross-ours");
        write_fake_skill(&ours, "0.0.1");
        std::fs::write(ours.join(MARKER_FILE), "{}").unwrap();
        let location = inspect_location(&ours, "global", true);
        assert_eq!(location.kind, KIND_OURS);
        assert!(!location.outdated);
        let _ = std::fs::remove_dir_all(home);
    }

    /// TASK-59：外部位置的 `manager`。skills.sh：全局锁（XDG_STATE_HOME
    /// 优先，未设回家目录）或项目锁里登记了 `ai-cross`；锁文件读不了或
    /// 格式不对当未登记；登记了别的技能不标；ours 与 none 不标。
    #[test]
    fn task59_external_manager_lock_files() {
        let home = temp_home("t59-manager");
        let project = temp_home("t59-manager-project");
        let xdg = temp_home("t59-manager-xdg");
        std::fs::create_dir_all(xdg.join("skills")).unwrap();
        std::fs::write(
            xdg.join("skills").join(".skill-lock.json"),
            r#"{"skills":{"ai-cross":{"version":"1.0.0"},"other-skill":{}}}"#,
        )
        .unwrap();
        let xdg_text = xdg.to_string_lossy().into_owned();
        let context = ScanContext {
            home: &home,
            project: Some(&project),
            xdg_state_home: Some(xdg_text.as_str()),
        };
        // 全局 external：全局锁登记了 ai-cross → skills.sh。
        let global = home.join(".claude").join("skills").join("ai-cross");
        write_fake_skill(&global, "9.9.9");
        let location = inspect_location_full(&global, "global", true, false, &context);
        assert_eq!(location.kind, KIND_EXTERNAL);
        assert_eq!(location.manager, "skills.sh");
        // 项目锁没写：项目位置不标；登记后标 skills.sh。
        let project_target = project.join(".claude").join("skills").join("ai-cross");
        write_fake_skill(&project_target, "9.9.9");
        let location = inspect_location_full(&project_target, "project", true, false, &context);
        assert_eq!(location.manager, "");
        std::fs::write(
            project.join("skills-lock.json"),
            r#"{"skills":{"ai-cross":{}}}"#,
        )
        .unwrap();
        let location = inspect_location_full(&project_target, "project", true, false, &context);
        assert_eq!(location.manager, "skills.sh");
        // 锁文件格式不对或不在了，当未登记。
        std::fs::write(xdg.join("skills").join(".skill-lock.json"), "不是 JSON").unwrap();
        let location = inspect_location_full(&global, "global", true, false, &context);
        assert_eq!(location.manager, "");
        std::fs::remove_file(xdg.join("skills").join(".skill-lock.json")).unwrap();
        let location = inspect_location_full(&global, "global", true, false, &context);
        assert_eq!(location.manager, "");
        // 登记的是别的技能时不标。
        std::fs::write(
            xdg.join("skills").join(".skill-lock.json"),
            r#"{"skills":{"other-skill":{}}}"#,
        )
        .unwrap();
        let location = inspect_location_full(&global, "global", true, false, &context);
        assert_eq!(location.manager, "");
        // XDG_STATE_HOME 未设时读家目录的 ~/.agents/.skill-lock.json。
        let fallback = ScanContext {
            home: &home,
            project: None,
            xdg_state_home: None,
        };
        std::fs::create_dir_all(home.join(".agents")).unwrap();
        let location = inspect_location_full(&global, "global", true, false, &fallback);
        assert_eq!(location.manager, "");
        std::fs::write(
            home.join(".agents").join(".skill-lock.json"),
            r#"{"skills":{"ai-cross":{}}}"#,
        )
        .unwrap();
        let location = inspect_location_full(&global, "global", true, false, &fallback);
        assert_eq!(location.manager, "skills.sh");
        // ours 与 none 不标 manager。
        std::fs::write(global.join(MARKER_FILE), "{}").unwrap();
        let location = inspect_location_full(&global, "global", true, false, &fallback);
        assert_eq!(location.kind, KIND_OURS);
        assert_eq!(location.manager, "");
        let location = inspect_location_full(&home.join("no-such"), "global", true, false, &fallback);
        assert_eq!(location.kind, KIND_NONE);
        assert_eq!(location.manager, "");
        let _ = std::fs::remove_dir_all(home);
        let _ = std::fs::remove_dir_all(project);
        let _ = std::fs::remove_dir_all(xdg);
    }

    /// TASK-59：skills-manager 的链接判定——位置是链接且目标在
    /// `~/.skills-manager/` 下；建不了链接的环境跳过并注明（如 Windows
    /// 无权限建 junction）。
    #[test]
    fn task59_external_manager_skills_manager_link() {
        let home = temp_home("t59-manager-link");
        let real = home
            .join(".skills-manager")
            .join("skills")
            .join("ai-cross");
        write_fake_skill(&real, "3.0.0");
        let link = home.join(".claude").join("skills").join("ai-cross");
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        if !create_dir_link(&link, &real) {
            println!("此环境建不了目录链接，跳过 skills-manager 链接用例");
            return;
        }
        let context = ScanContext {
            home: &home,
            project: None,
            xdg_state_home: None,
        };
        let location = inspect_location_full(&link, "global", true, false, &context);
        assert_eq!(location.kind, KIND_EXTERNAL);
        assert_eq!(location.manager, "skills-manager");
        assert!(location.link_target.is_some());
        // 链接目标不在 .skills-manager 下：不标 skills-manager；全局锁也
        // 没登记，为空串。
        let other_real = home.join("elsewhere").join("ai-cross");
        write_fake_skill(&other_real, "3.0.0");
        let other_link = home.join(".claude").join("skills").join("ai-cross-other");
        if create_dir_link(&other_link, &other_real) {
            let location = inspect_location_full(&other_link, "global", true, false, &context);
            assert_eq!(location.kind, KIND_EXTERNAL);
            assert_eq!(location.manager, "");
        } else {
            println!("此环境建不了目录链接，跳过对照用例");
        }
        let _ = std::fs::remove_dir_all(home);
    }

    /// 相对目标的符号链接；Windows 上需开发者模式或相应权限，建不了返回
    /// false 由调用方跳过。
    #[cfg(windows)]
    fn create_relative_symlink(link: &Path, target: &Path) -> bool {
        std::os::windows::fs::symlink_dir(target, link).is_ok()
    }

    #[cfg(not(windows))]
    fn create_relative_symlink(link: &Path, target: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    /// fix-59-r1：skills-manager 判定按 canonicalize 后的真实路径比对——
    /// 链式链接（位置 → 中转链接 → `~/.skills-manager/`）与带 `..` 的相对
    /// 目标都认；建不了对应链接的环境跳过该段并注明。
    #[test]
    fn task59_external_manager_canonical_chain_and_relative() {
        let home = temp_home("t59-manager-canonical");
        let real = home
            .join(".skills-manager")
            .join("skills")
            .join("ai-cross");
        write_fake_skill(&real, "3.0.0");
        let context = ScanContext {
            home: &home,
            project: None,
            xdg_state_home: None,
        };
        // 链式：位置 → home/hop/ai-cross → .skills-manager 下的真实目录。
        let hop = home.join("hop").join("ai-cross");
        std::fs::create_dir_all(hop.parent().unwrap()).unwrap();
        let chained = home.join(".claude").join("skills").join("ai-cross");
        std::fs::create_dir_all(chained.parent().unwrap()).unwrap();
        if create_dir_link(&hop, &real) && create_dir_link(&chained, &hop) {
            let location = inspect_location_full(&chained, "global", true, false, &context);
            assert_eq!(location.kind, KIND_EXTERNAL);
            assert_eq!(location.manager, "skills-manager");
        } else {
            println!("此环境建不了目录链接，跳过链式链接用例");
        }
        // 相对目标带 `..`：按链接文本拼出来的路径含 `..`，不 canonicalize
        // 时判不出。
        let relative = home.join(".codex").join("skills").join("ai-cross");
        std::fs::create_dir_all(relative.parent().unwrap()).unwrap();
        let relative_target = Path::new("..")
            .join("..")
            .join(".skills-manager")
            .join("skills")
            .join("ai-cross");
        if create_relative_symlink(&relative, &relative_target) {
            let location = inspect_location_full(&relative, "global", true, false, &context);
            assert_eq!(location.kind, KIND_EXTERNAL);
            assert_eq!(location.manager, "skills-manager");
        } else {
            println!("此环境建不了相对目标的符号链接（Windows 需开发者模式），跳过相对目标用例");
        }
        let _ = std::fs::remove_dir_all(home);
    }

    /// 走查 #11：输出的路径逐段拼接，Windows 上统一反斜杠，不混用斜杠。
    #[test]
    fn task57_status_paths_use_native_separators() {
        let home = temp_home("separators");
        let status = host_status("pi", &home, None);
        assert!(!status.locations.is_empty());
        for location in &status.locations {
            if cfg!(windows) {
                assert!(
                    !location.path.contains('/'),
                    "路径不应混用斜杠：{}",
                    location.path
                );
            }
            assert!(
                ends_with_path(&location.path, "ai-cross"),
                "每个位置都指向 ai-cross：{}",
                location.path
            );
        }
        let _ = std::fs::remove_dir_all(home);
    }

    /// fix-57-r1 4：分隔符统一只转斜杠，不改动路径的其它部分。
    #[test]
    fn task57_r1_native_separators_only_replace_slashes() {
        let converted = native_separators(Path::new("D:/x/Y/z"));
        assert_eq!(converted.to_string_lossy().replace('\\', "/"), "D:/x/Y/z");
        let already = native_separators(Path::new("D:\\x\\Y"));
        assert_eq!(already.to_string_lossy().replace('\\', "/"), "D:/x/Y");
    }

    /// fix-57-r1 4：传入 `D:/x` 形式（正斜杠）的项目根路径时，先统一
    /// 成反斜杠再拼接；安装落在项目根下，输出路径不混用斜杠。
    #[test]
    fn task57_r1_forward_slash_project_roots_are_normalized() {
        let home = temp_home("fwdslash-home");
        let project = temp_home("fwdslash-project");
        let forwarded = PathBuf::from(project.to_string_lossy().replace('\\', "/"));
        assert!(forwarded.is_dir(), "正斜杠形式应指向同一目录：{forwarded:?}");
        install_at(&home, "claude", "project", Some(&forwarded)).unwrap();
        let value = status_at(&home, Some(&forwarded)).unwrap();
        let mut project_kinds = Vec::new();
        for host in value["hosts"].as_array().unwrap() {
            for location in host["locations"].as_array().unwrap() {
                let path = location["path"].as_str().unwrap();
                if cfg!(windows) {
                    assert!(!path.contains('/'), "路径不应混用斜杠：{path}");
                }
                if location["scope"] == "project" {
                    assert!(
                        Path::new(path).starts_with(&project),
                        "project 位置应在项目根下：{path}"
                    );
                    project_kinds.push(location["kind"].as_str().unwrap().to_string());
                }
            }
        }
        // 恰好一处已安装（claude 的项目位置），其余项目位置都不存在。
        assert_eq!(
            project_kinds
                .iter()
                .filter(|kind| kind.as_str() == KIND_OURS)
                .count(),
            1,
            "{project_kinds:?}"
        );
        let _ = std::fs::remove_dir_all(home);
        let _ = std::fs::remove_dir_all(project);
    }

    /// `--project` 等于家目录时按规范化路径去重：同一物理目录只列一次，
    /// 不误报 duplicate（fix-50-r1）。
    #[test]
    fn project_same_as_home_dedupes_locations() {
        let home = temp_home("dedupe");
        install_at(&home, "kimi", "global", None).unwrap();
        // kimi 的项目位置 .agents/skills/ai-cross 与全局同名同目录。
        let status = host_status("kimi", &home, Some(&home));
        let agents_locations = status
            .locations
            .iter()
            .filter(|location| ends_with_path(&location.path, ".agents/skills/ai-cross"))
            .count();
        assert_eq!(agents_locations, 1, "同一物理目录只列一次：{:?}", status.locations);
        assert!(!status.duplicate, "{:?}", status.locations);
        // 写入位置照常标出。
        assert!(status
            .locations
            .iter()
            .filter(|location| ends_with_path(&location.path, ".agents/skills/ai-cross"))
            .all(|location| location.write_target));
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn install_creates_updates_and_refuses_external() {
        let home = temp_home("install");
        // 新建。
        let value = install_at(&home, "claude", "global", None).unwrap();
        let target = home.join(".claude/skills/ai-cross");
        assert!(target.join("SKILL.md").is_file());
        assert!(target.join(MARKER_FILE).is_file());
        let marker: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(target.join(MARKER_FILE)).unwrap())
                .unwrap();
        assert_eq!(marker["version"], bundled_version());
        assert_eq!(marker["source"], INSTALL_SOURCE);
        assert_eq!(value["hosts"][0]["host"], "claude");
        assert_eq!(value["hosts"][0]["locations"][0]["kind"], KIND_OURS);
        assert_eq!(value["hosts"][0]["locations"][0]["writeTarget"], true);
        // SKILL.md 内容与内置一致（整份复制）。
        let bundled = SKILL_BUNDLE
            .get_file("SKILL.md")
            .map(|file| file.contents().to_vec())
            .unwrap();
        assert_eq!(std::fs::read(target.join("SKILL.md")).unwrap(), bundled);
        // 更新：旧目录先改名成备份、临时目录换名到位后删备份；旧目录里
        // 的多余文件不残留，skills 目录内外都不留临时 / 备份目录（fix-50-r2）。
        std::fs::write(target.join("leftover.txt"), "旧文件").unwrap();
        install_at(&home, "claude", "global", None).unwrap();
        assert!(!target.join("leftover.txt").exists(), "更新要替换整份目录");
        // 暂存目录（常驻锁文件）以外不留 .aicross 开头的条目，暂存目录里
        // 只剩锁文件。
        let leftovers: Vec<_> = std::fs::read_dir(target.parent().unwrap())
            .unwrap()
            .chain(std::fs::read_dir(target.parent().unwrap().parent().unwrap()).unwrap())
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                name.starts_with(".aicross") && name != ".aicross-install-skills"
            })
            .collect();
        assert!(
            leftovers.is_empty(),
            "skills 目录内外都不残留临时 / 备份目录：{:?}",
            leftovers
        );
        assert!(staged_names(&home.join(".claude").join(".aicross-install-skills")).is_empty());
        // external（无标记）拒绝，文件原样保留。
        std::fs::remove_dir_all(&target).unwrap();
        write_fake_skill(&target, "9.9.9");
        let before = std::fs::read(target.join("SKILL.md")).unwrap();
        let error = install_at(&home, "claude", "global", None).unwrap_err();
        assert_eq!(error.0, 1);
        assert!(error.1.contains("由其他软件管理"), "{:?} ", error.1);
        assert_eq!(std::fs::read(target.join("SKILL.md")).unwrap(), before);
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn install_writes_agents_dir_for_codex_kimi_pi_and_project_scope() {
        let home = temp_home("install-agents");
        let project = temp_home("install-project");
        for host in ["codex", "kimi", "pi"] {
            install_at(&home, host, "global", None).unwrap();
            assert!(
                home.join(".agents/skills/ai-cross/SKILL.md").is_file(),
                "{host} 写 .agents/skills"
            );
        }
        // 项目范围写到项目目录下；目录不存在退出 2。
        install_at(&home, "pi", "project", Some(&project)).unwrap();
        assert!(project.join(".agents/skills/ai-cross/SKILL.md").is_file());
        let missing = home.join("no-such-project");
        let error = install_at(&home, "pi", "project", Some(&missing)).unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("项目目录不存在"), "{:?}", error.1);
        let error = install_at(&home, "pi", "project", None).unwrap_err();
        assert_eq!(error.0, 2);
        assert!(error.1.contains("--project"), "{:?}", error.1);
        let _ = std::fs::remove_dir_all(home);
        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn uninstall_removes_only_ours() {
        let home = temp_home("uninstall");
        // 没装过：幂等成功。
        uninstall_at(&home, "pi", "global", None).unwrap();
        // external 不动。
        let target = home.join(".agents/skills/ai-cross");
        write_fake_skill(&target, "9.9.9");
        let error = uninstall_at(&home, "pi", "global", None).unwrap_err();
        assert_eq!(error.0, 1);
        assert!(target.join("SKILL.md").is_file(), "external 不删");
        // ours 删掉。
        std::fs::write(target.join(MARKER_FILE), "{}").unwrap();
        uninstall_at(&home, "pi", "global", None).unwrap();
        assert!(!target.exists());
        let _ = std::fs::remove_dir_all(home);
    }

    /// 给了 --project 时写入位置按 scope 分别标（fix-50-r2）：global 位置
    /// 对全局写入位置、project 位置对项目写入位置，两处 `.agents` 都算
    /// writeTarget，其余读取位置不算。
    #[test]
    fn write_target_marks_global_and_project_separately() {
        let home = temp_home("write-target");
        let project = temp_home("write-target-project");
        install_at(&home, "kimi", "global", None).unwrap();
        install_at(&home, "kimi", "project", Some(&project)).unwrap();
        let status = host_status("kimi", &home, Some(&project));
        let marked = status
            .locations
            .iter()
            .filter(|location| location.write_target)
            .count();
        assert_eq!(marked, 2, "{:?}", status.locations);
        assert!(status.locations.iter().any(|location| {
            location.write_target
                && location.scope == "global"
                && ends_with_path(&location.path, ".agents/skills/ai-cross")
        }));
        assert!(status.locations.iter().any(|location| {
            location.write_target
                && location.scope == "project"
                && ends_with_path(&location.path, ".agents/skills/ai-cross")
        }));
        // 只读的两处 .kimi-code 位置不是写入位置。
        assert!(status
            .locations
            .iter()
            .filter(|location| ends_with_path(&location.path, ".kimi-code/skills/ai-cross"))
            .all(|location| !location.write_target));
        let _ = std::fs::remove_dir_all(home);
        let _ = std::fs::remove_dir_all(project);
    }

    /// fix-fullcheck A1：状态查询只读，残留的临时 / 备份目录（可能是中断
    /// 更新后唯一的旧安装，或别的进程正在用）一个都不删；下一次安装在锁内
    /// 清理，旧格式（skills 目录里与其上一级）与新格式（暂存目录）都认，
    /// 无关目录不动。
    #[test]
    fn a1_status_is_read_only_and_install_cleans_leftovers() {
        let home = temp_home("sweep");
        let skills = home.join(".claude/skills");
        let legacy_temp = skills.join(".aicross-install-tmp-4242");
        let legacy_backup = home.join(".claude/.aicross-install-bak-old");
        let (real_skills, staging) = staging_dir(&skills.join("ai-cross")).unwrap();
        let staged_temp = staging.join(".aicross-install-tmp-x/ai-cross");
        write_fake_skill(&legacy_temp, "0.1.0");
        std::fs::write(legacy_temp.join(MARKER_FILE), "{}").unwrap();
        write_fake_skill(&legacy_backup, "0.1.0");
        std::fs::write(legacy_backup.join(MARKER_FILE), "{}").unwrap();
        write_fake_skill(&staged_temp, "0.1.0");
        write_operation_record(staged_temp.parent().unwrap(), &real_skills.join("ai-cross"), "x").unwrap();
        std::fs::create_dir_all(skills.join("other-skill")).unwrap();
        let _ = status_at(&home, None).unwrap();
        assert!(legacy_temp.is_dir(), "status 不删临时目录");
        assert!(legacy_backup.join(MARKER_FILE).is_file(), "status 不删备份");
        assert!(staged_temp.join("SKILL.md").is_file(), "status 不删暂存目录内容");
        // 安装：目标缺失、旧格式备份在 → 先恢复再更新；残留全部清掉。
        install_at(&home, "claude", "global", None).unwrap();
        let target = skills.join("ai-cross");
        assert_eq!(
            skill_version_from_text(&std::fs::read_to_string(target.join("SKILL.md")).unwrap()),
            bundled_version()
        );
        assert!(!legacy_temp.exists());
        assert!(!legacy_backup.exists());
        assert!(staged_names(&staging).is_empty(), "暂存目录里只剩锁文件");
        assert!(skills.join("other-skill").is_dir(), "无关目录不受影响");
        let _ = std::fs::remove_dir_all(home);
    }

    /// fix-fullcheck A1：更新在「旧安装已改名成备份、新目录还没换到位」时
    /// 中断，目标缺失、只剩备份；下一次安装 / 更新先把备份改回目标，备份
    /// 恢复失败时不删备份。卸载不恢复，直接清掉。
    #[test]
    fn a1_interrupted_update_backup_is_restored_before_install() {
        let home = temp_home("recover");
        let target = home.join(".agents/skills/ai-cross");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        let (real_skills, staging) = staging_dir(&target).unwrap();
        let real_target = real_skills.join("ai-cross");
        let backup = staging.join(format!("{BACKUP_PREFIX}crash/ai-cross"));
        let plant_backup = || {
            write_fake_skill(&backup, "0.0.1");
            std::fs::write(backup.join(MARKER_FILE), "{}").unwrap();
            write_operation_record(backup.parent().unwrap(), &real_target, "crash").unwrap();
        };
        plant_backup();
        std::fs::write(backup.join("user-note.txt"), "旧安装里的文件").unwrap();
        let temp = staging.join(format!("{TEMP_PREFIX}crash/ai-cross"));
        write_fake_skill(&temp, "9.9.9");
        write_operation_record(temp.parent().unwrap(), &real_target, "crash").unwrap();
        let notes = recover_leftovers(&real_target, &target, &staging, true).unwrap();
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(std::fs::read_to_string(target.join("user-note.txt")).unwrap(), "旧安装里的文件");
        assert!(find_leftovers(&target, &staging).is_empty(), "恢复后残留清干净");
        // 卸载路径不恢复：目标缺失 + 备份在 → 备份删掉，目标仍缺失。
        std::fs::remove_dir_all(&target).unwrap();
        plant_backup();
        uninstall_at(&home, "codex", "global", None).unwrap();
        assert!(!target.exists());
        assert!(!backup.exists());
        // 完整一轮：中断后的安装成功，结果是内置版本。
        plant_backup();
        install_at(&home, "codex", "global", None).unwrap();
        assert_eq!(
            skill_version_from_text(&std::fs::read_to_string(target.join("SKILL.md")).unwrap()),
            bundled_version()
        );
        assert!(staged_names(&staging).is_empty());
        let _ = std::fs::remove_dir_all(home);
    }

    /// 暂存目录里除锁文件以外的条目名。
    fn staged_names(staging: &Path) -> Vec<String> {
        std::fs::read_dir(staging)
            .map(|entries| {
                entries
                    .filter_map(|entry| entry.ok())
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .filter(|name| name != INSTALL_LOCK_FILE)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// fix-fullcheck 第二轮 2：只凭名称前缀的残留不动——暂存目录里没有操作
    /// 记录或记录目标不符的、旧位置里没有安装标记的都保留并在输出 notes
    /// 里说明；残留根目录或内容目录是链接的不恢复、不删（链接目标原样）。
    #[test]
    fn r2_unowned_and_linked_leftovers_are_kept() {
        let home = temp_home("r2-keep");
        let outside = temp_home("r2-keep-outside");
        let target = home.join(".agents").join("skills").join("ai-cross");
        let skills = target.parent().unwrap().to_path_buf();
        std::fs::create_dir_all(&skills).unwrap();
        let (real_skills, staging) = staging_dir(&target).unwrap();
        // 旧位置里同名前缀、没有安装标记：别人的目录。
        let foreign_legacy = skills.join(format!("{TEMP_PREFIX}mine"));
        write_fake_skill(&foreign_legacy, "1.0.0");
        // 暂存目录里没有操作记录、记录目标不是本次位置的。
        let no_record = staging.join(format!("{BACKUP_PREFIX}norecord")).join("ai-cross");
        write_fake_skill(&no_record, "1.0.0");
        std::fs::write(no_record.join(MARKER_FILE), "{}").unwrap();
        let other_target = staging.join(format!("{BACKUP_PREFIX}other")).join("ai-cross");
        write_fake_skill(&other_target, "1.0.0");
        std::fs::write(other_target.join(MARKER_FILE), "{}").unwrap();
        write_operation_record(other_target.parent().unwrap(), &outside.join("ai-cross"), "o").unwrap();
        // 链接残留：带记录的备份根目录里，内容目录是指向外部带标记目录的链接。
        let external = outside.join("external-skill");
        write_fake_skill(&external, "2.0.0");
        std::fs::write(external.join(MARKER_FILE), "{}").unwrap();
        let linked_root = staging.join(format!("{BACKUP_PREFIX}linked"));
        write_operation_record(&linked_root, &real_skills.join("ai-cross"), "l").unwrap();
        let linked = create_dir_link(&linked_root.join("ai-cross"), &external);
        let value = install_at(&home, "codex", "global", None).unwrap();
        // 目标缺失时也没有恢复任何不归属的备份：装上的是内置版本。
        assert_eq!(
            skill_version_from_text(&std::fs::read_to_string(target.join("SKILL.md")).unwrap()),
            bundled_version()
        );
        assert!(foreign_legacy.join("SKILL.md").is_file(), "无标记的同名前缀目录不删");
        assert!(no_record.join("SKILL.md").is_file(), "没有操作记录的不删");
        assert!(other_target.join("SKILL.md").is_file(), "记录目标不符的不删");
        let notes = value["notes"].as_array().expect("保留时输出 notes");
        assert!(notes.len() >= 3, "{notes:?}");
        if linked {
            assert!(linked_root.join("ai-cross").join("SKILL.md").is_file(), "链接残留不删");
            assert!(external.join(MARKER_FILE).is_file(), "链接目标原样");
            assert!(notes.iter().any(|note| note.as_str().unwrap().contains("链接")), "{notes:?}");
            let _ = std::fs::remove_dir(linked_root.join("ai-cross"));
        } else {
            eprintln!("跳过链接部分：建不了目录链接");
        }
        let _ = std::fs::remove_dir_all(home);
        let _ = std::fs::remove_dir_all(outside);
    }

    /// fix-fullcheck A1：安装 / 卸载跨进程互斥。测试直接持有暂存目录里的
    /// 锁文件（等同另一个进程正在安装），安装要等它释放才动手。
    #[test]
    fn a1_install_waits_for_cross_process_lock() {
        let home = temp_home("xlock");
        let target = home.join(".claude/skills/ai-cross");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        let (_, staging) = staging_dir(&target).unwrap();
        let held = crate::file_lock::FileLock::acquire(
            &staging.join(INSTALL_LOCK_FILE),
            std::time::Duration::from_secs(1),
        )
        .unwrap();
        let worker_home = home.clone();
        let worker = std::thread::spawn(move || install_at(&worker_home, "claude", "global", None));
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(!target.exists(), "别的进程持锁时不写");
        drop(held);
        worker.join().unwrap().unwrap();
        assert!(target.join(MARKER_FILE).is_file());
        let _ = std::fs::remove_dir_all(home);
    }

    /// fix-fullcheck A6：skills 目录被 junction / symlink 重定向到别处时，
    /// 暂存目录按解析后的真实位置建（与目标同卷），不建在字面父目录；
    /// 新装与更新都成功，两边都不留残留。建不了目录链接的平台跳过。
    #[test]
    fn a6_staging_follows_redirected_skills_dir() {
        let home = temp_home("redirect");
        let elsewhere = temp_home("redirect-real");
        let real_skills = elsewhere.join("RealSkills");
        std::fs::create_dir_all(&real_skills).unwrap();
        std::fs::create_dir_all(home.join(".agents")).unwrap();
        if !create_dir_link(&home.join(".agents").join("skills"), &real_skills) {
            eprintln!("跳过：建不了目录链接");
            let _ = std::fs::remove_dir_all(&home);
            let _ = std::fs::remove_dir_all(&elsewhere);
            return;
        }
        let target = home.join(".agents/skills/ai-cross");
        let (resolved, staging) = staging_dir(&target).unwrap();
        let real_parent = std::fs::canonicalize(&elsewhere).unwrap();
        assert_eq!(resolved, real_parent.join("RealSkills"));
        assert_eq!(staging, real_parent.join(".aicross-install-RealSkills"));
        install_at(&home, "codex", "global", None).unwrap();
        install_at(&home, "codex", "global", None).unwrap();
        assert!(real_skills.join("ai-cross").join(MARKER_FILE).is_file());
        // 字面父目录与 skills 目录里都没有 .aicross 开头的条目；真实父目录
        // 里只有暂存目录，暂存目录里只剩锁文件。
        let stray: Vec<_> = std::fs::read_dir(home.join(".agents"))
            .unwrap()
            .chain(std::fs::read_dir(&elsewhere).unwrap())
            .chain(std::fs::read_dir(&real_skills).unwrap())
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                name.starts_with(".aicross") && name != ".aicross-install-RealSkills"
            })
            .collect();
        assert!(stray.is_empty(), "{stray:?}");
        assert!(staged_names(&staging).is_empty());
        assert!(!home.join(".agents").join(".aicross-install-skills").exists());
        let _ = std::fs::remove_dir(home.join(".agents").join("skills"));
        let _ = std::fs::remove_dir_all(home);
        let _ = std::fs::remove_dir_all(elsewhere);
    }

    #[test]
    fn status_output_structure_matches_cli_shape() {
        let home = temp_home("status-shape");
        let value = status_at(&home, None).unwrap();
        assert_eq!(value["bundledVersion"], bundled_version());
        let hosts = value["hosts"].as_array().unwrap();
        assert_eq!(hosts.len(), 4);
        for host in hosts {
            assert!(host["locations"].as_array().unwrap().len() >= 1);
            assert_eq!(host["locations"][0]["readByThisHost"], true);
            // writeTarget 每个位置都带，前端据此决定按钮状态。
            assert!(host["locations"][0]["writeTarget"].is_boolean());
        }
        let _ = std::fs::remove_dir_all(home);
    }
}
