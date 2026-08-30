use crate::models::{ChangeEntry, DiffResult, FileNode};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use walkdir::{DirEntry, WalkDir};

const IGNORED_DIRS: &[&str] = &[".git", "node_modules", "target", "dist"];
const TEXT_LIMIT: u64 = 2 * 1024 * 1024;
const TEXT_TOTAL_LIMIT: usize = 16 * 1024 * 1024;

#[derive(Clone)]
enum SnapshotContent {
    Text(String),
    TextUnavailable(&'static str),
    Binary,
}

#[derive(Clone)]
struct SnapshotEntry {
    size: u64,
    modified: u128,
    hash: String,
    content: SnapshotContent,
}

type Snapshot = BTreeMap<String, SnapshotEntry>;

#[derive(Default)]
pub struct SnapshotStore {
    baseline: Snapshot,
    current: Snapshot,
}

impl SnapshotStore {
    pub fn initialize(&mut self, project: &Path) -> Result<(), String> {
        let snapshot = capture(project)?;
        self.baseline = snapshot.clone();
        self.current = snapshot;
        Ok(())
    }

    pub fn refresh(&mut self, project: &Path) -> Result<Vec<ChangeEntry>, String> {
        self.current = capture(project)?;
        Ok(compare(&self.baseline, &self.current))
    }

    pub fn reset(&mut self, project: &Path) -> Result<(), String> {
        let snapshot = capture(project)?;
        self.baseline = snapshot.clone();
        self.current = snapshot;
        Ok(())
    }

    pub fn diff(&self, relative_path: &str) -> Result<DiffResult, String> {
        let before = self.baseline.get(relative_path);
        let after = self.current.get(relative_path);
        if before.is_none() && after.is_none() {
            return Err("生成差异失败：文件不在快照中".to_string());
        }

        let binary = before.is_some_and(|entry| matches!(entry.content, SnapshotContent::Binary))
            || after.is_some_and(|entry| matches!(entry.content, SnapshotContent::Binary));
        if binary {
            let old_size = before.map(|entry| entry.size).unwrap_or(0);
            let new_size = after.map(|entry| entry.size).unwrap_or(0);
            return Ok(DiffResult {
                path: relative_path.to_string(),
                binary: true,
                text: format!("二进制文件：{old_size} 字节 → {new_size} 字节"),
            });
        }

        if let Some(reason) = before
            .and_then(unavailable_reason)
            .or_else(|| after.and_then(unavailable_reason))
        {
            return Ok(DiffResult {
                path: relative_path.to_string(),
                binary: false,
                text: format!("文本内容未保留，未生成行级差异：{reason}"),
            });
        }

        let old_text = before.and_then(text_content).unwrap_or("");
        let new_text = after.and_then(text_content).unwrap_or("");
        Ok(DiffResult {
            path: relative_path.to_string(),
            binary: false,
            text: line_diff(relative_path, old_text, new_text),
        })
    }
}

fn unavailable_reason(entry: &SnapshotEntry) -> Option<&'static str> {
    match entry.content {
        SnapshotContent::TextUnavailable(reason) => Some(reason),
        _ => None,
    }
}

fn text_content(entry: &SnapshotEntry) -> Option<&str> {
    match &entry.content {
        SnapshotContent::Text(content) => Some(content),
        _ => None,
    }
}

fn line_diff(path: &str, before: &str, after: &str) -> String {
    if before == after {
        return String::new();
    }
    let old_lines: Vec<&str> = before.lines().collect();
    let new_lines: Vec<&str> = after.lines().collect();
    let mut prefix = 0;
    while prefix < old_lines.len()
        && prefix < new_lines.len()
        && old_lines[prefix] == new_lines[prefix]
    {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < old_lines.len().saturating_sub(prefix)
        && suffix < new_lines.len().saturating_sub(prefix)
        && old_lines[old_lines.len() - suffix - 1] == new_lines[new_lines.len() - suffix - 1]
    {
        suffix += 1;
    }

    let context_start = prefix.saturating_sub(3);
    let old_change_end = old_lines.len().saturating_sub(suffix);
    let new_change_end = new_lines.len().saturating_sub(suffix);
    let context_end_old = (old_change_end + 3).min(old_lines.len());
    let context_end_new = (new_change_end + 3).min(new_lines.len());
    let old_count = context_end_old.saturating_sub(context_start);
    let new_count = context_end_new.saturating_sub(context_start);

    let mut output = format!(
        "--- a/{path}\n+++ b/{path}\n@@ -{},{} +{},{} @@\n",
        context_start + 1,
        old_count,
        context_start + 1,
        new_count
    );
    for line in &old_lines[context_start..prefix] {
        output.push_str(&format!(" {line}\n"));
    }
    for line in &old_lines[prefix..old_change_end] {
        output.push_str(&format!("-{line}\n"));
    }
    for line in &new_lines[prefix..new_change_end] {
        output.push_str(&format!("+{line}\n"));
    }
    let trailing = 3.min(suffix);
    for line in &new_lines[new_change_end..new_change_end + trailing] {
        output.push_str(&format!(" {line}\n"));
    }
    output
}

pub fn list_directory(project: &Path, relative_path: &str) -> Result<Vec<FileNode>, String> {
    let directory = safe_path(project, relative_path)?;
    if !directory.is_dir() {
        return Err("列出目录失败：目标不是目录".to_string());
    }

    let mut nodes = Vec::new();
    for result in fs::read_dir(&directory).map_err(|error| format!("读取目录失败：{error}"))?
    {
        let entry = result.map_err(|error| format!("读取目录项失败：{error}"))?;
        let path = entry.path();
        let Ok(canonical) = dunce::canonicalize(&path) else {
            continue;
        };
        if !canonical.starts_with(project) {
            continue;
        }
        let file_type = entry
            .file_type()
            .map_err(|error| format!("读取文件类型失败：{error}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        let relative = path
            .strip_prefix(project)
            .map_err(|error| format!("生成相对路径失败：{error}"))?
            .to_string_lossy()
            .to_string();
        let is_dir = canonical.is_dir();
        nodes.push(FileNode {
            name: name.clone(),
            relative_path: relative,
            absolute_path: path.to_string_lossy().to_string(),
            is_dir,
            is_symlink: file_type.is_symlink(),
            muted: is_dir && IGNORED_DIRS.contains(&name.as_str()),
        });
    }
    nodes.sort_by(|left, right| {
        right
            .is_dir
            .cmp(&left.is_dir)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    Ok(nodes)
}

pub fn safe_file(project: &Path, relative_path: &str) -> Result<PathBuf, String> {
    let path = safe_path(project, relative_path)?;
    if !path.is_file() {
        return Err("打开文件失败：目标不是文件".to_string());
    }
    Ok(path)
}

fn safe_path(project: &Path, relative_path: &str) -> Result<PathBuf, String> {
    let path = if relative_path.is_empty() {
        project.to_path_buf()
    } else {
        project.join(relative_path)
    };
    let canonical = dunce::canonicalize(&path).map_err(|error| format!("解析路径失败：{error}"))?;
    if !canonical.starts_with(project) {
        return Err("路径超出当前项目".to_string());
    }
    Ok(canonical)
}

fn capture(project: &Path) -> Result<Snapshot, String> {
    let mut snapshot = Snapshot::new();
    let mut retained_text = 0_usize;
    for result in WalkDir::new(project)
        .follow_links(true)
        .into_iter()
        .filter_entry(|entry| should_visit(project, entry))
    {
        let entry = match result {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(project)
            .map_err(|error| format!("生成快照路径失败：{error}"))?
            .to_string_lossy()
            .to_string();
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        let (hash, content) = match read_snapshot_file(entry.path(), metadata.len(), retained_text)
        {
            Ok(value) => value,
            Err(_) => continue,
        };
        if let SnapshotContent::Text(text) = &content {
            retained_text += text.len();
        }
        let modified = metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_nanos())
            .unwrap_or_default();
        snapshot.insert(
            relative,
            SnapshotEntry {
                size: metadata.len(),
                modified,
                hash,
                content,
            },
        );
    }
    Ok(snapshot)
}

fn read_snapshot_file(
    path: &Path,
    size: u64,
    retained_text: usize,
) -> Result<(String, SnapshotContent), String> {
    let file = File::open(path).map_err(|error| format!("读取快照文件失败：{error}"))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let keep_text = size <= TEXT_LIMIT;
    let mut bytes = keep_text.then(|| Vec::with_capacity(size as usize));
    let mut utf8_tail = Vec::new();
    let mut valid_utf8 = true;
    let mut has_nul = false;
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let length = reader
            .read(&mut buffer)
            .map_err(|error| format!("读取快照文件失败：{error}"))?;
        if length == 0 {
            break;
        }
        let chunk = &buffer[..length];
        hasher.update(chunk);
        has_nul |= chunk.contains(&0);
        if valid_utf8 {
            valid_utf8 = validate_utf8_chunk(&mut utf8_tail, chunk);
        }
        if let Some(bytes) = &mut bytes {
            bytes.extend_from_slice(chunk);
        }
    }
    valid_utf8 &= utf8_tail.is_empty();

    let hash = format!("{:x}", hasher.finalize());
    let content = if has_nul || !valid_utf8 {
        SnapshotContent::Binary
    } else if !keep_text {
        SnapshotContent::TextUnavailable("文件超过 2 MB")
    } else {
        let bytes = bytes.expect("small text buffer");
        if retained_text.saturating_add(bytes.len()) > TEXT_TOTAL_LIMIT {
            SnapshotContent::TextUnavailable("文本快照总量达到 16 MB")
        } else {
            SnapshotContent::Text(String::from_utf8(bytes).expect("validated UTF-8"))
        }
    };
    Ok((hash, content))
}

fn validate_utf8_chunk(tail: &mut Vec<u8>, chunk: &[u8]) -> bool {
    let mut combined = Vec::with_capacity(tail.len() + chunk.len());
    combined.extend_from_slice(tail);
    combined.extend_from_slice(chunk);
    match std::str::from_utf8(&combined) {
        Ok(_) => {
            tail.clear();
            true
        }
        Err(error) if error.error_len().is_none() => {
            *tail = combined[error.valid_up_to()..].to_vec();
            true
        }
        Err(_) => false,
    }
}

fn should_visit(project: &Path, entry: &DirEntry) -> bool {
    if entry.depth() == 0 {
        return true;
    }
    let Ok(canonical) = dunce::canonicalize(entry.path()) else {
        return false;
    };
    if !canonical.starts_with(project) {
        return false;
    }
    !entry.file_type().is_dir()
        || !IGNORED_DIRS.contains(&entry.file_name().to_string_lossy().as_ref())
}

fn compare(baseline: &Snapshot, current: &Snapshot) -> Vec<ChangeEntry> {
    let paths: BTreeSet<_> = baseline.keys().chain(current.keys()).collect();
    paths
        .into_iter()
        .filter_map(|path| {
            let before = baseline.get(path);
            let after = current.get(path);
            let kind = match (before, after) {
                (None, Some(_)) => "added",
                (Some(_), None) => "deleted",
                (Some(left), Some(right)) if snapshot_content_changed(left, right) => "modified",
                _ => return None,
            };
            Some(ChangeEntry {
                path: path.clone(),
                kind: kind.to_string(),
                binary: before
                    .is_some_and(|entry| matches!(entry.content, SnapshotContent::Binary))
                    || after.is_some_and(|entry| matches!(entry.content, SnapshotContent::Binary)),
                old_size: before.map(|entry| entry.size),
                new_size: after.map(|entry| entry.size),
            })
        })
        .collect()
}

fn snapshot_content_changed(left: &SnapshotEntry, right: &SnapshotEntry) -> bool {
    // mtime and size remain snapshot metadata, but only the content hash is authoritative.
    let _metadata_changed = left.size != right.size || left.modified != right.modified;
    left.hash != right.hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn fixture_directory(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!("aicross-{name}-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).expect("create fixture");
        dunce::canonicalize(directory).expect("canonical fixture")
    }

    fn entry(hash: &str, size: u64, modified: u128, content: SnapshotContent) -> SnapshotEntry {
        SnapshotEntry {
            size,
            modified,
            hash: hash.to_string(),
            content,
        }
    }

    #[test]
    fn safe_path_rejects_traversal_absolute_paths_and_external_symlinks() {
        let project = fixture_directory("safe-path-project");
        let outside = fixture_directory("safe-path-outside");
        fs::write(project.join("inside.txt"), "inside").expect("write inside");
        fs::write(outside.join("outside.txt"), "outside").expect("write outside");

        assert_eq!(safe_path(&project, "").expect("empty path"), project);
        let traversal = format!(
            "../{}/outside.txt",
            outside
                .file_name()
                .expect("outside fixture name")
                .to_string_lossy()
        );
        assert!(safe_path(&project, &traversal).is_err());
        assert!(safe_path(&project, outside.to_string_lossy().as_ref()).is_err());

        let link = project.join("outside-link");
        create_directory_link(&outside, &link).expect("create external directory link");
        assert!(safe_path(&project, "outside-link/outside.txt").is_err());
        fs::remove_dir(&link).expect("remove external directory link");

        fs::remove_dir_all(project).expect("remove project fixture");
        fs::remove_dir_all(outside).expect("remove outside fixture");
    }

    #[cfg(windows)]
    fn create_directory_link(source: &Path, target: &Path) -> std::io::Result<()> {
        let status = std::process::Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(target)
            .arg(source)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other(format!(
                "mklink exited with {status}"
            )))
        }
    }

    #[cfg(unix)]
    fn create_directory_link(source: &Path, target: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(source, target)
    }

    #[test]
    fn compare_and_diff_cover_added_deleted_mtime_binary_and_hunk_counts() {
        let mut baseline = Snapshot::new();
        baseline.insert(
            "same.txt".to_string(),
            entry("same", 4, 1, SnapshotContent::Text("same".to_string())),
        );
        baseline.insert(
            "deleted.txt".to_string(),
            entry("gone", 4, 1, SnapshotContent::Text("gone".to_string())),
        );
        baseline.insert(
            "binary.bin".to_string(),
            entry("old", 2, 1, SnapshotContent::Binary),
        );

        let mut current = Snapshot::new();
        current.insert(
            "same.txt".to_string(),
            entry("same", 4, 99, SnapshotContent::Text("same".to_string())),
        );
        current.insert(
            "added.txt".to_string(),
            entry("new", 3, 2, SnapshotContent::Text("new".to_string())),
        );
        current.insert(
            "binary.bin".to_string(),
            entry("new-binary", 5, 2, SnapshotContent::Binary),
        );

        let changes = compare(&baseline, &current);
        assert_eq!(
            changes
                .iter()
                .map(|change| (change.path.as_str(), change.kind.as_str()))
                .collect::<Vec<_>>(),
            [
                ("added.txt", "added"),
                ("binary.bin", "modified"),
                ("deleted.txt", "deleted")
            ]
        );
        assert!(changes[1].binary);
        assert_eq!(changes[1].old_size, Some(2));
        assert_eq!(changes[1].new_size, Some(5));

        let diff = line_diff(
            "note.txt",
            "one\ntwo\nthree\nfour\nfive\nsix\nseven\n",
            "one\ntwo\nthree\nchanged\nfive\nsix\nseven\n",
        );
        assert!(diff.contains("@@ -1,7 +1,7 @@"), "diff was {diff:?}");
    }

    #[test]
    fn large_utf8_text_is_not_reported_as_binary() {
        let directory = fixture_directory("large-text");
        let path = directory.join("large.txt");
        fs::write(&path, vec![b'a'; TEXT_LIMIT as usize + 1]).expect("write large text");
        let (_, content) = read_snapshot_file(&path, TEXT_LIMIT + 1, 0).expect("read large text");
        assert!(matches!(content, SnapshotContent::TextUnavailable(_)));
        fs::remove_dir_all(directory).expect("remove large text fixture");
    }

    #[test]
    fn snapshot_reports_modified_text_and_diff() {
        let directory = fixture_directory("workspace");
        let file = directory.join("note.txt");
        fs::write(&file, "first\nsecond\n").expect("write baseline");

        let mut store = SnapshotStore::default();
        store.initialize(&directory).expect("initialize snapshot");
        fs::write(&file, "first\nchanged\n").expect("write change");
        let changes = store.refresh(&directory).expect("refresh snapshot");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, "modified");
        let diff = store.diff("note.txt").expect("render diff");
        assert!(diff.text.contains("-second"));
        assert!(diff.text.contains("+changed"));

        fs::remove_dir_all(&directory).expect("remove fixture");
    }
}
