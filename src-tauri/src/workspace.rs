use crate::models::{ChangeEntry, DiffResult, FileNode};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use walkdir::{DirEntry, WalkDir};

const IGNORED_DIRS: &[&str] = &[".git", "node_modules", "target", "dist"];
const TEXT_LIMIT: u64 = 2 * 1024 * 1024;

#[derive(Clone)]
struct SnapshotEntry {
    size: u64,
    modified: u128,
    hash: String,
    content: Option<String>,
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
        let binary = before.and_then(|entry| entry.content.as_ref()).is_none() && before.is_some()
            || after.and_then(|entry| entry.content.as_ref()).is_none() && after.is_some();

        if binary {
            let old_size = before.map(|entry| entry.size).unwrap_or(0);
            let new_size = after.map(|entry| entry.size).unwrap_or(0);
            return Ok(DiffResult {
                path: relative_path.to_string(),
                binary: true,
                text: format!("二进制文件：{old_size} 字节 → {new_size} 字节"),
            });
        }

        let old_text = before
            .and_then(|entry| entry.content.as_deref())
            .unwrap_or("");
        let new_text = after
            .and_then(|entry| entry.content.as_deref())
            .unwrap_or("");
        let text = line_diff(relative_path, old_text, new_text);
        Ok(DiffResult {
            path: relative_path.to_string(),
            binary: false,
            text,
        })
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
        return Err("目标不是目录".to_string());
    }

    let mut nodes = Vec::new();
    for result in fs::read_dir(&directory).map_err(|error| error.to_string())? {
        let entry = result.map_err(|error| error.to_string())?;
        let path = entry.path();
        let Ok(canonical) = path.canonicalize() else {
            continue;
        };
        if !canonical.starts_with(project) {
            continue;
        }
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let relative = path
            .strip_prefix(project)
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .to_string();
        nodes.push(FileNode {
            name: name.clone(),
            relative_path: relative,
            absolute_path: path.to_string_lossy().to_string(),
            is_dir: file_type.is_dir(),
            muted: file_type.is_dir() && IGNORED_DIRS.contains(&name.as_str()),
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
        return Err("目标不是文件".to_string());
    }
    Ok(path)
}

fn safe_path(project: &Path, relative_path: &str) -> Result<PathBuf, String> {
    let path = if relative_path.is_empty() {
        project.to_path_buf()
    } else {
        project.join(relative_path)
    };
    let canonical = path.canonicalize().map_err(|error| error.to_string())?;
    if !canonical.starts_with(project) {
        return Err("路径超出当前项目".to_string());
    }
    Ok(canonical)
}

fn capture(project: &Path) -> Result<Snapshot, String> {
    let mut snapshot = Snapshot::new();
    for result in WalkDir::new(project)
        .follow_links(false)
        .into_iter()
        .filter_entry(should_visit)
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
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .to_string();
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        let bytes = match fs::read(entry.path()) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        let hash = format!("{:x}", Sha256::digest(&bytes));
        let modified = metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_nanos())
            .unwrap_or_default();
        let content =
            if metadata.len() <= TEXT_LIMIT && !bytes.iter().take(8192).any(|byte| *byte == 0) {
                String::from_utf8(bytes).ok()
            } else {
                None
            };
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

fn should_visit(entry: &DirEntry) -> bool {
    entry.depth() == 0
        || !entry.file_type().is_dir()
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
                (Some(left), Some(right))
                    if left.hash != right.hash
                        || left.size != right.size
                        || left.modified != right.modified =>
                {
                    "modified"
                }
                _ => return None,
            };
            Some(ChangeEntry {
                path: path.clone(),
                kind: kind.to_string(),
                binary: before.map(|entry| entry.content.is_none()).unwrap_or(false)
                    || after.map(|entry| entry.content.is_none()).unwrap_or(false),
                old_size: before.map(|entry| entry.size),
                new_size: after.map(|entry| entry.size),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn snapshot_reports_modified_text_and_diff() {
        let directory = std::env::temp_dir().join(format!("aicross-workspace-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).expect("create fixture");
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
