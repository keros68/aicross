//! 项目里 aicross 唯一的目录 `.aicross/`（11-02 定）：
//!
//! ```text
//! <项目>/.aicross/
//!   .gitignore      只忽略 runs/
//!   decided.md      已定事项，进版本库
//!   runs/<派发>/    一次派发一个目录：各路记录、STATE.md、裁决.md、.snapshots/
//! ```
//!
//! 派发目录名是「派发开始时间-run_id 前 8 位」，同一 run 的后续阶段按后缀
//! 找回同一目录。旧的平铺 `.dispatch/` 由 `migrate_legacy` 一次性迁入。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;

pub(crate) const DIR_NAME: &str = ".aicross";
pub(crate) const LEGACY_DIR_NAME: &str = ".dispatch";
pub(crate) const SNAPSHOTS_DIR: &str = ".snapshots";
pub(crate) const VERDICT_FILE: &str = "裁决.md";
pub(crate) const LEGACY_BUCKET: &str = "旧记录";
const GITIGNORE: &str = "runs/\n";

pub(crate) fn aicross_dir(project: &Path) -> PathBuf {
    project.join(DIR_NAME)
}

pub(crate) fn runs_dir(project: &Path) -> PathBuf {
    aicross_dir(project).join("runs")
}

pub(crate) fn decided_path(project: &Path) -> PathBuf {
    aicross_dir(project).join("decided.md")
}

/// 建 `.aicross/`，没有 `.gitignore` 时写一份只忽略 `runs/` 的。
pub(crate) fn ensure_aicross_dir(project: &Path) -> Result<PathBuf, String> {
    let dir = aicross_dir(project);
    fs::create_dir_all(&dir).map_err(|error| format!("创建 .aicross 目录失败：{error}"))?;
    let ignore = dir.join(".gitignore");
    if !ignore.exists() {
        fs::write(&ignore, GITIGNORE).map_err(|error| format!("写入 .aicross/.gitignore 失败：{error}"))?;
    }
    Ok(dir)
}

/// run_id 的短后缀：去掉不安全字符后取前 8 个字符。
pub(crate) fn short_id(run_id: &str) -> String {
    let safe: String = run_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .take(8)
        .collect();
    if safe.is_empty() { "run".into() } else { safe }
}

/// 已有的派发目录（按后缀找），没有返回 None。
pub(crate) fn find_run_dir(project: &Path, run_id: &str) -> Option<PathBuf> {
    let suffix = format!("-{}", short_id(run_id));
    let mut found: Vec<PathBuf> = fs::read_dir(runs_dir(project))
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && path.file_name().and_then(|n| n.to_str()).is_some_and(|name| name.ends_with(&suffix))
        })
        .collect();
    found.sort();
    found.pop()
}

/// 本次派发的目录：已有就复用，没有就按当前时间新建。先迁移旧 `.dispatch/`，
/// 迁移失败不阻断派发，说明放进返回的 note。
pub(crate) fn run_dir(project: &Path, run_id: &str) -> Result<(PathBuf, Option<String>), String> {
    let note = migrate_legacy(project).err();
    ensure_aicross_dir(project)?;
    if let Some(dir) = find_run_dir(project, run_id) {
        return Ok((dir, note));
    }
    let name = format!("{}-{}", Local::now().format("%Y%m%d-%H%M%S"), short_id(run_id));
    let dir = runs_dir(project).join(name);
    fs::create_dir_all(&dir).map_err(|error| format!("创建派发目录失败：{error}"))?;
    Ok((dir, note))
}

/// 派发目录里某个节点的快照目录（`.before` / `.inflight` / `.redgreen`）。
pub(crate) fn snapshot_dir(run_dir: &Path, node_file_name: &str, kind: &str) -> PathBuf {
    run_dir.join(SNAPSHOTS_DIR).join(format!("{node_file_name}.{kind}"))
}

/// 某次派发里某个节点的快照目录；派发目录不存在时返回空路径（`is_dir()` 为假）。
pub(crate) fn node_snapshot(project: &Path, run_id: &str, node_file_name: &str, kind: &str) -> PathBuf {
    find_run_dir(project, run_id)
        .map(|dir| snapshot_dir(&dir, node_file_name, kind))
        .unwrap_or_default()
}

/// 所有派发目录（含旧记录），供按节点找快照用。
pub(crate) fn all_run_dirs(project: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(runs_dir(project))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs
}

// ── 旧 `.dispatch/` 迁移 ───────────────────────────────────────────────────

fn frontmatter_value(content: &str, key: &str) -> Option<String> {
    let rest = content.strip_prefix("---\n").or_else(|| content.strip_prefix("---\r\n"))?;
    let end = rest.find("\n---")?;
    rest[..end].lines().find_map(|line| {
        let value = line.strip_prefix(key)?.strip_prefix(':')?.trim();
        let value = value.trim_matches(|c| c == '"' || c == '\'');
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn file_stamp(name: &str) -> Option<String> {
    let stamp: String = name.chars().take(15).collect();
    let bytes = stamp.as_bytes();
    (bytes.len() == 15
        && bytes[8] == b'-'
        && bytes[..8].iter().all(u8::is_ascii_digit)
        && bytes[9..].iter().all(u8::is_ascii_digit))
    .then_some(stamp)
}

fn safe_component(value: &str) -> String {
    let safe: String = value
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_') { c } else { '-' })
        .collect();
    safe.trim_matches('-').to_string()
}

fn move_path(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建 {} 失败：{error}", parent.display()))?;
    }
    let target = unique_target(to);
    fs::rename(from, &target).map_err(|error| format!("移动 {} 失败：{error}", from.display()))
}

/// 目标已存在时在名字后加 `-2`、`-3`…，迁移从不覆盖已有文件。
fn unique_target(to: &Path) -> PathBuf {
    let mut target = to.to_path_buf();
    let mut suffix = 2;
    while target.exists() {
        let stem = to.file_name().and_then(|n| n.to_str()).unwrap_or("item");
        target = to.with_file_name(format!("{stem}-{suffix}"));
        suffix += 1;
    }
    target
}

/// 把旧 `.dispatch/` 迁进 `.aicross/runs/`：md 记录按 frontmatter 的 run_id
/// 归组（目录名取该组最早文件名里的时间），`before_dir` 的旧前缀改写为新
/// 路径；`.dispatch/<run>/` 快照目录并进对应派发目录的 `.snapshots/`；其余
/// 放进 `runs/旧记录/`；最后删 `.dispatch/`。没有 `.dispatch/` 时什么都不做。
/// 出错时停下并返回原因，已移动的保持在新位置，下次接着迁。
pub(crate) fn migrate_legacy(project: &Path) -> Result<(), String> {
    let legacy = project.join(LEGACY_DIR_NAME);
    if !legacy.is_dir() {
        return Ok(());
    }
    ensure_aicross_dir(project)?;
    let runs = runs_dir(project);
    let entries: Vec<PathBuf> = fs::read_dir(&legacy)
        .map_err(|error| format!("读取 .dispatch 失败：{error}"))?
        .flatten()
        .map(|entry| entry.path())
        .collect();

    // run_id → (最早时间, 记录文件)
    let mut groups: BTreeMap<String, (String, Vec<(PathBuf, String)>)> = BTreeMap::new();
    let mut leftovers: Vec<PathBuf> = Vec::new();
    for path in &entries {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        if name == ".gitignore" {
            continue;
        }
        if path.is_file() && name.ends_with(".md") && name != "STATE.md" {
            let content = fs::read_to_string(path).unwrap_or_default();
            if let Some(run_id) = frontmatter_value(&content, "run_id") {
                let stamp = file_stamp(&name).unwrap_or_else(|| Local::now().format("%Y%m%d-%H%M%S").to_string());
                let group = groups.entry(run_id).or_insert_with(|| (stamp.clone(), Vec::new()));
                if stamp < group.0 {
                    group.0 = stamp;
                }
                group.1.push((path.clone(), content));
                continue;
            }
        }
        if path.is_dir() {
            continue; // 快照目录按 run 归位，见下
        }
        leftovers.push(path.clone());
    }

    // 每个 run 的派发目录：上次迁到一半留下的同后缀目录优先复用。
    let mut owners: BTreeMap<String, PathBuf> = BTreeMap::new();
    for (run_id, (stamp, _)) in &groups {
        let dir = find_run_dir(project, run_id)
            .unwrap_or_else(|| runs.join(format!("{stamp}-{}", short_id(run_id))));
        fs::create_dir_all(&dir).map_err(|error| format!("创建派发目录失败：{error}"))?;
        owners.insert(safe_component(run_id), dir);
    }

    // 先挪快照、再挪记录：中途失败时记录还在原处，下次照样按 run_id 归组；
    // 记录已挪走而快照还在时，按快照目录名的后缀找回已迁的派发目录。
    for path in entries.iter().filter(|p| p.is_dir()) {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        let owner = owners.get(&name).cloned().or_else(|| find_run_dir(project, &name));
        match owner {
            Some(dir) => {
                let snapshots = dir.join(SNAPSHOTS_DIR);
                fs::create_dir_all(&snapshots).map_err(|error| format!("创建快照目录失败：{error}"))?;
                for child in fs::read_dir(path).map_err(|error| format!("读取 {} 失败：{error}", path.display()))? {
                    let child = child.map_err(|error| format!("读取 {} 失败：{error}", path.display()))?;
                    move_path(&child.path(), &snapshots.join(child.file_name()))?;
                }
                fs::remove_dir(path).map_err(|error| format!("删除 {} 失败：{error}", path.display()))?;
            }
            None => move_path(path, &runs.join(LEGACY_BUCKET).join(&name))?,
        }
    }

    for (run_id, (_, files)) in &groups {
        let dir = &owners[&safe_component(run_id)];
        let legacy_prefix = format!("{LEGACY_DIR_NAME}/{}/", safe_component(run_id));
        let new_prefix = {
            let relative = dir.strip_prefix(project).unwrap_or(dir).join(SNAPSHOTS_DIR);
            format!("{}/", relative.to_string_lossy().replace('\\', "/"))
        };
        for (path, content) in files {
            let rewritten = content.replace(&legacy_prefix, &new_prefix);
            if rewritten == *content {
                move_path(path, &dir.join(path.file_name().unwrap_or_default()))?;
                continue;
            }
            let target = unique_target(&dir.join(path.file_name().unwrap_or_default()));
            fs::write(&target, rewritten).map_err(|error| format!("写入 {} 失败：{error}", target.display()))?;
            fs::remove_file(path).map_err(|error| format!("删除 {} 失败：{error}", path.display()))?;
        }
    }
    for path in leftovers {
        let name = path.file_name().unwrap_or_default().to_owned();
        move_path(&path, &runs.join(LEGACY_BUCKET).join(name))?;
    }
    // 只删空目录：还剩东西（读不出的条目）就留着，下次再迁。
    let _ = fs::remove_file(legacy.join(".gitignore"));
    fs::remove_dir(&legacy).map_err(|error| format!(".dispatch 里还有没迁完的内容：{error}"))
}

// ── 已定事项 ──────────────────────────────────────────────────────────────

pub(crate) const KIND_DECIDED: &str = "已定";
pub(crate) const KIND_REJECTED: &str = "不做";
const DECIDED_HEADER: &str = "# 已定事项\n\n由 ai-cross 在你同意后写入，可直接编辑。派发时作为约束附给各方。\n\n## 已定\n\n## 不做\n";
const ENTRY_LIMIT: usize = 200;

fn clean_line(value: &str, what: &str) -> Result<String, String> {
    let line = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.is_empty() {
        return Err(format!("{what}不能为空"));
    }
    if line.chars().any(char::is_control) {
        return Err(format!("{what}含控制字符"));
    }
    if line.chars().count() > ENTRY_LIMIT {
        return Err(format!("{what}最长 {ENTRY_LIMIT} 字"));
    }
    Ok(line)
}

/// 往 `decided.md` 的「已定」或「不做」一节末尾追加一行
/// `- YYYY-MM-DD 内容（理由）`；文件不存在时先建。返回写入的行。
pub(crate) fn add_decided(project: &Path, kind: &str, text: &str, reason: &str, date: &str) -> Result<String, String> {
    if kind != KIND_DECIDED && kind != KIND_REJECTED {
        return Err(format!("--kind 只认 {KIND_DECIDED} 或 {KIND_REJECTED}：{kind}"));
    }
    let text = clean_line(text, "内容")?;
    let reason = clean_line(reason, "理由")?;
    ensure_aicross_dir(project)?;
    let path = decided_path(project);
    let content = fs::read_to_string(&path).unwrap_or_else(|_| DECIDED_HEADER.to_string());
    let entry = format!("- {date} {text}（{reason}）");
    let heading = format!("## {kind}");
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
    let insert_at = match lines.iter().position(|line| line.trim() == heading) {
        Some(start) => {
            let end = lines[start + 1..]
                .iter()
                .position(|line| line.trim_start().starts_with("## "))
                .map_or(lines.len(), |offset| start + 1 + offset);
            // 插在本节最后一个非空行之后
            let mut at = end;
            while at > start + 1 && lines[at - 1].trim().is_empty() {
                at -= 1;
            }
            at
        }
        None => {
            if lines.last().is_some_and(|line| !line.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(heading.clone());
            lines.len()
        }
    };
    let needs_gap_before = lines.get(insert_at - 1).is_some_and(|line| line.trim() == heading);
    if needs_gap_before {
        lines.insert(insert_at, String::new());
        lines.insert(insert_at + 1, entry.clone());
        if lines.get(insert_at + 2).is_some_and(|line| !line.trim().is_empty()) {
            lines.insert(insert_at + 2, String::new());
        }
    } else {
        lines.insert(insert_at, entry.clone());
    }
    let mut out = lines.join("\n");
    out.push('\n');
    fs::write(&path, out).map_err(|error| format!("写入 decided.md 失败：{error}"))?;
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_project(tag: &str) -> PathBuf {
        let dir = crate::paths::test_temp_dir().join(format!("project-dir-{tag}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn run_dir_is_reused_for_the_same_run() {
        let project = temp_project("reuse");
        let (first, _) = run_dir(&project, "abcdef12-3456").unwrap();
        let (second, _) = run_dir(&project, "abcdef12-3456").unwrap();
        assert_eq!(first, second);
        assert!(first.file_name().unwrap().to_str().unwrap().ends_with("-abcdef12"));
        assert_eq!(fs::read_to_string(project.join(".aicross/.gitignore")).unwrap(), "runs/\n");
        let (other, _) = run_dir(&project, "99999999-0000").unwrap();
        assert_ne!(first, other);
    }

    #[test]
    fn migrate_groups_by_run_id_and_moves_snapshots() {
        let project = temp_project("migrate");
        let legacy = project.join(".dispatch");
        fs::create_dir_all(legacy.join("run-a/node-1.before/files")).unwrap();
        fs::write(legacy.join("run-a/node-1.before/files/x.txt"), "old").unwrap();
        fs::create_dir_all(legacy.join("runtime/runs")).unwrap();
        fs::write(legacy.join(".gitignore"), "*\n").unwrap();
        fs::write(legacy.join("STATE.md"), "state").unwrap();
        fs::write(legacy.join("20260913-191319-pi-b-审查.md"), "---\nrun_id: run-a\nbefore_dir: .dispatch/run-a/node-1.before\n---\n正文").unwrap();
        fs::write(legacy.join("20260913-191317-codex-a-审查.md"), "---\nschema: aicross-dispatch/1\nrun_id: run-a\n---\n正文").unwrap();
        fs::write(legacy.join("20260914-080000-kimi-c-审查.md"), "---\nrun_id: 'run-b'\n---\n").unwrap();
        fs::write(legacy.join("notes.md"), "没有 frontmatter").unwrap();

        migrate_legacy(&project).unwrap();

        assert!(!legacy.exists());
        let a = project.join(".aicross/runs/20260913-191317-runa");
        assert!(a.join("20260913-191317-codex-a-审查.md").is_file());
        let rewritten = fs::read_to_string(a.join("20260913-191319-pi-b-审查.md")).unwrap();
        assert!(rewritten.contains("before_dir: .aicross/runs/20260913-191317-runa/.snapshots/node-1.before"), "{rewritten}");
        assert_eq!(fs::read_to_string(a.join(".snapshots/node-1.before/files/x.txt")).unwrap(), "old");
        assert!(project.join(".aicross/runs/20260914-080000-runb/20260914-080000-kimi-c-审查.md").is_file());
        let old = project.join(".aicross/runs/旧记录");
        assert!(old.join("notes.md").is_file());
        assert!(old.join("STATE.md").is_file());
        assert!(old.join("runtime").is_dir());
        // 迁完后同一 run 继续写，回到同一个目录
        assert_eq!(find_run_dir(&project, "run-a").unwrap(), a);
        // 没有 .dispatch 时什么都不做
        migrate_legacy(&project).unwrap();
    }

    /// 上次迁到一半：记录已迁走，快照目录还在 .dispatch 里。重试时快照要回到
    /// 已迁的派发目录，不能进「旧记录」让 before_dir 悬空。
    #[test]
    fn migrate_retry_puts_orphan_snapshot_back_with_its_run() {
        let project = temp_project("retry");
        let done = project.join(".aicross/runs/20260913-191317-runa");
        fs::create_dir_all(&done).unwrap();
        fs::write(done.join("20260913-191317-codex-a-审查.md"), "---\nrun_id: run-a\nbefore_dir: .aicross/runs/20260913-191317-runa/.snapshots/n.before\n---\n").unwrap();
        let legacy = project.join(".dispatch");
        fs::create_dir_all(legacy.join("run-a/n.before")).unwrap();
        fs::write(legacy.join("run-a/n.before/x.txt"), "old").unwrap();
        migrate_legacy(&project).unwrap();
        assert_eq!(fs::read_to_string(done.join(".snapshots/n.before/x.txt")).unwrap(), "old");
        assert!(!project.join(".aicross/runs/旧记录").exists());
        assert!(!legacy.exists());
        // 同名记录已存在时不覆盖
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("20260913-191317-codex-a-审查.md"), "---\nrun_id: run-a\nbefore_dir: .dispatch/run-a/n.before\n---\n第二份").unwrap();
        migrate_legacy(&project).unwrap();
        assert!(fs::read_to_string(done.join("20260913-191317-codex-a-审查.md")).unwrap().contains(".snapshots/n.before"));
        assert!(fs::read_to_string(done.join("20260913-191317-codex-a-审查.md-2")).unwrap().contains("第二份"));
    }

    #[test]
    fn add_decided_creates_and_appends_to_sections() {
        let project = temp_project("decided");
        add_decided(&project, "不做", "不处理 IE 兼容", "用户只用 Chrome", "2026-11-02").unwrap();
        add_decided(&project, "已定", "接口用 snake_case", "与后端一致", "2026-11-02").unwrap();
        add_decided(&project, "不做", "不加暗色主题", "以后再说", "2026-11-03").unwrap();
        let text = fs::read_to_string(project.join(".aicross/decided.md")).unwrap();
        let decided = text.find("## 已定").unwrap();
        let rejected = text.find("## 不做").unwrap();
        let snake = text.find("接口用 snake_case").unwrap();
        let ie = text.find("不处理 IE 兼容").unwrap();
        let dark = text.find("不加暗色主题").unwrap();
        assert!(decided < snake && snake < rejected && rejected < ie && ie < dark, "{text}");
        assert!(text.contains("- 2026-11-02 不处理 IE 兼容（用户只用 Chrome）"));
        assert!(add_decided(&project, "其他", "x", "y", "2026-11-02").is_err());
        assert!(add_decided(&project, "已定", " ", "y", "2026-11-02").is_err());
        assert!(add_decided(&project, "已定", &"长".repeat(201), "y", "2026-11-02").is_err());
    }
}
