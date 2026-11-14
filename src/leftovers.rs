//! 只读统计各家 CLI 历史里指向 aicross 数据目录的遗留条目（`aicross leftovers`）。

use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader, ErrorKind};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

const CLAUDE_CLI: &str = "claude";
const KIMI_CLI: &str = "kimi";
const AGY_CLI: &str = "agy";

/// 收集默认 `~/.aicross` 与当前 `root` 下指向 aicross 数据目录但未由固定槽位接管的 CLI 历史条目。
pub(crate) fn collect(root: &Path) -> Value {
    let home = crate::paths::home();
    collect_with_home(root, home.as_deref().ok())
}

/// 指定用户目录收集遗留条目（测试用，避免读取真机家目录）。
pub(crate) fn collect_with_home(root: &Path, home: Option<&Path>) -> Value {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut root_norms: Vec<String> = Vec::new();
    push_unique_root(&mut roots, &mut root_norms, root);
    if let Some(home_dir) = home {
        let default_root = crate::paths::aicross_root_from(home_dir);
        push_unique_root(&mut roots, &mut root_norms, &default_root);
    }

    let excluded = excluded_paths(&roots);
    let excluded_norms: Vec<String> = excluded
        .iter()
        .map(|path| normalize_for_match(&path.to_string_lossy()))
        .collect();

    let (claude, kimi, agy) = match home {
        Some(home_dir) => (
            collect_claude(home_dir, &roots, &excluded, &root_norms, &excluded_norms),
            collect_kimi(home_dir, &root_norms, &excluded_norms),
            collect_agy(home_dir, &root_norms, &excluded_norms),
        ),
        None => {
            let err = Some("读不到用户目录".to_string());
            (
                cli_result(Vec::new(), err.clone()),
                cli_result(Vec::new(), err.clone()),
                cli_result(Vec::new(), err),
            )
        }
    };

    let root_strings: Vec<String> = roots
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();

    json!({
        "roots": root_strings,
        CLAUDE_CLI: claude,
        KIMI_CLI: kimi,
        AGY_CLI: agy,
    })
}

fn push_unique_root(roots: &mut Vec<PathBuf>, root_norms: &mut Vec<String>, candidate: &Path) {
    let norm = normalize_for_match(&candidate.to_string_lossy());
    if norm.is_empty() || root_norms.iter().any(|existing| existing == &norm) {
        return;
    }
    root_norms.push(norm);
    roots.push(candidate.to_path_buf());
}

fn excluded_paths(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut excluded = Vec::new();
    for root in roots {
        excluded.push(root.join("kimi-oneshot"));
        let blind = root.join("blind");
        for prefix in ["kimi", "agy"] {
            for index in 1..=crate::blind_slot::SLOT_COUNT {
                excluded.push(blind.join(format!("{prefix}-{index}")));
            }
        }
    }
    excluded
}

fn normalize_for_match(raw: &str) -> String {
    let mut norm = raw.trim().replace('\\', "/");
    while norm.ends_with('/')
        && norm.len() > 1
        && !(norm.len() == 3 && norm.as_bytes()[1] == b':')
    {
        norm.pop();
    }
    #[cfg(windows)]
    {
        norm = norm.to_lowercase();
    }
    norm
}

fn path_within(candidate_norm: &str, base_norm: &str) -> bool {
    if base_norm.is_empty() {
        return false;
    }
    if candidate_norm == base_norm {
        return true;
    }
    if base_norm.ends_with('/') {
        candidate_norm.starts_with(base_norm)
    } else {
        candidate_norm.starts_with(&format!("{base_norm}/"))
    }
}

fn is_leftover_path(candidate: &str, root_norms: &[String], excluded_norms: &[String]) -> bool {
    let cand = normalize_for_match(candidate);
    root_norms.iter().any(|root| path_within(&cand, root))
        && !excluded_norms.iter().any(|excl| path_within(&cand, excl))
}

fn cli_result(paths: Vec<String>, error: Option<String>) -> Value {
    json!({
        "count": paths.len(),
        "paths": paths,
        "error": error,
    })
}

fn encode_claude_project_name(path: &Path) -> String {
    let mut raw = path.to_string_lossy().into_owned();
    while (raw.ends_with('/') || raw.ends_with('\\'))
        && raw.len() > 1
        && !(raw.len() == 3 && raw.as_bytes()[1] == b':')
    {
        raw.pop();
    }
    let encoded: String = raw
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    #[cfg(windows)]
    {
        encoded.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        encoded
    }
}

const CLAUDE_AICROSS_SUBDIRS: &[&str] = &[
    "runs",
    "scratch",
    "blind",
    "sessions",
    "worktrees",
    "smoke",
    "tests",
];

fn claude_prefixed_match(entry_cmp: &str, base_encoded: &str) -> bool {
    !base_encoded.is_empty()
        && (entry_cmp == base_encoded || entry_cmp.starts_with(&format!("{base_encoded}-")))
}

fn claude_subdir_matches(entry_cmp: &str, base_encoded: &str) -> bool {
    if base_encoded.is_empty() {
        return false;
    }
    if entry_cmp == base_encoded {
        return true;
    }
    let Some(rest) = entry_cmp.strip_prefix(&format!("{base_encoded}-")) else {
        return false;
    };
    CLAUDE_AICROSS_SUBDIRS
        .iter()
        .any(|subdir| rest == *subdir || rest.starts_with(&format!("{subdir}-")))
}

fn read_claude_entry_cwd(entry_path: &Path) -> Option<String> {
    if let Ok(text) = fs::read_to_string(entry_path.join("sessions-index.json")) {
        if let Ok(parsed) = serde_json::from_str::<Value>(&text) {
            if let Some(orig) = parsed
                .get("originalPath")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
            {
                return Some(orig.to_string());
            }
            if let Some(items) = parsed.get("entries").and_then(Value::as_array) {
                for item in items {
                    if let Some(project_path) = item
                        .get("projectPath")
                        .and_then(Value::as_str)
                        .filter(|s| !s.trim().is_empty())
                    {
                        return Some(project_path.to_string());
                    }
                }
            }
        }
    }

    let entries = fs::read_dir(entry_path).ok()?;
    for file in entries.flatten() {
        let path = file.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") || !path.is_file() {
            continue;
        }
        let Ok(handle) = fs::File::open(&path) else {
            continue;
        };
        let reader = BufReader::new(handle);
        for line in reader.lines().take(20).flatten() {
            if let Ok(parsed) = serde_json::from_str::<Value>(&line) {
                if let Some(cwd) = parsed
                    .get("cwd")
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                {
                    return Some(cwd.to_string());
                }
            }
        }
    }
    None
}

fn collect_claude(
    home: &Path,
    roots: &[PathBuf],
    excluded: &[PathBuf],
    root_norms: &[String],
    excluded_norms: &[String],
) -> Value {
    let projects_dir = home.join(".claude").join("projects");
    let entries = match fs::read_dir(&projects_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return cli_result(Vec::new(), None);
        }
        Err(error) => {
            return cli_result(
                Vec::new(),
                Some(format!("读不到 {}：{error}", projects_dir.display())),
            );
        }
    };

    let encoded_roots: Vec<String> = roots.iter().map(|p| encode_claude_project_name(p)).collect();
    let encoded_excl: Vec<String> = excluded
        .iter()
        .map(|p| encode_claude_project_name(p))
        .collect();

    let mut paths = Vec::new();
    for entry in entries.flatten() {
        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        #[cfg(windows)]
        let name_cmp = name.to_lowercase();
        #[cfg(not(windows))]
        let name_cmp = name;

        // 先按目录名前缀预筛，避免读取无关项目下的 jsonl。
        if !encoded_roots
            .iter()
            .any(|root| claude_prefixed_match(&name_cmp, root))
        {
            continue;
        }

        let matched = if let Some(cwd) = read_claude_entry_cwd(&entry_path) {
            is_leftover_path(&cwd, root_norms, excluded_norms)
        } else {
            let under_root = encoded_roots
                .iter()
                .any(|root| claude_subdir_matches(&name_cmp, root));
            let is_excluded = encoded_excl
                .iter()
                .any(|excl| claude_prefixed_match(&name_cmp, excl));
            under_root && !is_excluded
        };
        if matched {
            paths.push(entry_path.to_string_lossy().into_owned());
        }
    }
    paths.sort();
    paths.dedup();
    cli_result(paths, None)
}

fn read_kimi_index(path: &Path, map_key: &str, deleted_key: &str) -> Result<Vec<String>, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("读不到 {}：{error}", path.display())),
    };
    let parsed: Value = serde_json::from_str(&text)
        .map_err(|_| format!("读不到 {}：格式无法识别", path.display()))?;
    let Some(obj) = parsed.as_object() else {
        return Err(format!("读不到 {}：格式无法识别", path.display()));
    };
    let Some(items) = obj.get(map_key).and_then(Value::as_object) else {
        return Err(format!("读不到 {}：格式无法识别", path.display()));
    };
    let mut deleted: HashSet<&str> = HashSet::new();
    if let Some(raw_deleted) = obj.get(deleted_key) {
        let Some(arr) = raw_deleted.as_array() else {
            return Err(format!("读不到 {}：格式无法识别", path.display()));
        };
        for item in arr {
            let Some(id) = item.as_str() else {
                return Err(format!("读不到 {}：格式无法识别", path.display()));
            };
            deleted.insert(id);
        }
    }

    let mut roots = Vec::new();
    for (id, entry) in items {
        if deleted.contains(id.as_str()) {
            continue;
        }
        let Some(entry_root) = entry.get("root").and_then(Value::as_str) else {
            return Err(format!("读不到 {}：格式无法识别", path.display()));
        };
        roots.push(entry_root.to_string());
    }
    Ok(roots)
}

fn collect_kimi(home: &Path, root_norms: &[String], excluded_norms: &[String]) -> Value {
    let kimi_dir = home.join(".kimi-code");
    let sources = [
        (
            kimi_dir.join("workspaces.json"),
            "workspaces",
            "deleted_workspace_ids",
        ),
        (
            kimi_dir.join("projects.json"),
            "projects",
            "deleted_project_ids",
        ),
    ];

    let mut seen: HashSet<String> = HashSet::new();
    let mut paths: Vec<String> = Vec::new();
    for (path, map_key, deleted_key) in &sources {
        let candidate_roots = match read_kimi_index(path, map_key, deleted_key) {
            Ok(roots) => roots,
            Err(error) => return cli_result(Vec::new(), Some(error)),
        };
        for candidate in candidate_roots {
            if is_leftover_path(&candidate, root_norms, excluded_norms) {
                let norm = normalize_for_match(&candidate);
                if seen.insert(norm) {
                    paths.push(candidate);
                }
            }
        }
    }
    paths.sort();
    cli_result(paths, None)
}

fn collect_agy(home: &Path, root_norms: &[String], excluded_norms: &[String]) -> Value {
    let path = home
        .join(".gemini")
        .join("antigravity-cli")
        .join("cache")
        .join("last_conversations.json");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return cli_result(Vec::new(), None);
        }
        Err(error) => {
            return cli_result(
                Vec::new(),
                Some(format!("读不到 {}：{error}", path.display())),
            );
        }
    };
    let Ok(parsed) = serde_json::from_str::<Value>(&text) else {
        return cli_result(
            Vec::new(),
            Some(format!("读不到 {}：格式无法识别", path.display())),
        );
    };
    let Some(map) = parsed.as_object() else {
        return cli_result(
            Vec::new(),
            Some(format!("读不到 {}：格式无法识别", path.display())),
        );
    };
    if !map.values().all(Value::is_string) {
        return cli_result(
            Vec::new(),
            Some(format!("读不到 {}：格式无法识别", path.display())),
        );
    }

    let mut seen: HashSet<String> = HashSet::new();
    let mut paths = Vec::new();
    for workspace in map.keys() {
        if is_leftover_path(workspace, root_norms, excluded_norms) {
            let norm = normalize_for_match(workspace);
            if seen.insert(norm) {
                paths.push(workspace.clone());
            }
        }
    }
    paths.sort();
    cli_result(paths, None)
}

#[cfg(test)]
mod tests {
    use super::{collect_with_home, encode_claude_project_name};
    use serde_json::json;
    use std::fs;
    use std::path::Path;
    use uuid::Uuid;

    fn temp_case(tag: &str) -> std::path::PathBuf {
        let dir = crate::paths::test_temp_dir().join(format!("aicross-leftovers-{tag}-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_history_files_return_zero_counts_without_error() {
        let dir = temp_case("missing");
        let home = dir.join("home");
        let root = dir.join("data");
        fs::create_dir_all(&home).unwrap();

        let out = collect_with_home(&root, Some(&home));
        assert_eq!(out["roots"].as_array().unwrap().len(), 2);
        for cli in ["claude", "kimi", "agy"] {
            assert_eq!(out[cli]["count"], 0, "{cli}: {out}");
            assert_eq!(out[cli]["paths"], json!([]), "{cli}: {out}");
            assert!(out[cli]["error"].is_null(), "{cli}: {out}");
        }

        let same_out = collect_with_home(&home.join(".aicross"), Some(&home));
        assert_eq!(same_out["roots"].as_array().unwrap().len(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn malformed_or_unrecognized_kimi_and_agy_files_report_error_not_failure() {
        let dir = temp_case("malformed");
        let home = dir.join("home");
        let root = dir.join("data");
        fs::create_dir_all(home.join(".kimi-code")).unwrap();
        fs::create_dir_all(home.join(".gemini/antigravity-cli/cache")).unwrap();

        fs::write(home.join(".kimi-code/workspaces.json"), "{not-json").unwrap();
        fs::write(home.join(".gemini/antigravity-cli/cache/last_conversations.json"), "[]").unwrap();

        let out = collect_with_home(&root, Some(&home));
        assert_eq!(out["kimi"]["count"], 0);
        assert!(out["kimi"]["error"].as_str().unwrap().starts_with("读不到"));
        assert_eq!(out["agy"]["count"], 0);
        assert!(out["agy"]["error"].as_str().unwrap().starts_with("读不到"));

        // workspaces.json 合法 JSON 但缺 workspaces 字段，或 projects.json 条目缺 root。
        fs::write(home.join(".kimi-code/workspaces.json"), r#"{"version":1}"#).unwrap();
        let out2 = collect_with_home(&root, Some(&home));
        assert_eq!(out2["kimi"]["count"], 0);
        assert!(out2["kimi"]["error"].as_str().unwrap().contains("格式无法识别"));

        fs::remove_file(home.join(".kimi-code/workspaces.json")).unwrap();
        fs::write(
            home.join(".kimi-code/projects.json"),
            r#"{"projects":{"p1":{"name":"x"}}}"#,
        )
        .unwrap();
        let out3 = collect_with_home(&root, Some(&home));
        assert_eq!(out3["kimi"]["count"], 0);
        assert!(out3["kimi"]["error"].as_str().unwrap().contains("格式无法识别"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn collects_dual_roots_unions_kimi_excludes_fixed_slots_and_respects_boundaries() {
        let dir = temp_case("collect");
        let home = dir.join("home");
        let custom_root = dir.join("aicross-data");
        let default_root = home.join(".aicross");
        let sibling_root = dir.join("aicross-data-other");

        // Claude 目录
        let claude_projects = home.join(".claude/projects");
        let custom_scratch_run = custom_root.join("scratch/20261105-run");
        let default_run = default_root.join("runs/r1/n1");
        let custom_slot = custom_root.join("blind/kimi-1");
        let custom_oneshot = custom_root.join("kimi-oneshot");
        let sibling_dir = sibling_root.join("runs/r1");
        for p in [
            &custom_scratch_run,
            &default_run,
            &custom_slot,
            &custom_oneshot,
            &sibling_dir,
        ] {
            let encoded = encode_claude_project_name(p);
            fs::create_dir_all(claude_projects.join(encoded)).unwrap();
        }
        // 非目录文件忽略
        fs::write(claude_projects.join("not-a-dir"), "x").unwrap();

        // Kimi workspaces.json + projects.json 并集、deleted 过滤、固定槽位排除、同路径大小写/斜杠去重
        fs::create_dir_all(home.join(".kimi-code")).unwrap();
        let custom_slash = custom_root.to_string_lossy().replace('\\', "/");
        let default_slash = default_root.to_string_lossy().replace('\\', "/");
        let sibling_slash = sibling_root.to_string_lossy().replace('\\', "/");
        fs::write(
            home.join(".kimi-code/workspaces.json"),
            json!({
                "version": 1,
                "workspaces": {
                    "w1": { "root": format!("{custom_slash}/runs/abc/n1") },
                    "w_del": { "root": format!("{custom_slash}/runs/deleted-one") },
                    "w_slot": { "root": format!("{custom_slash}/blind/kimi-3") },
                    "w_oneshot": { "root": format!("{default_slash}/kimi-oneshot") },
                    "w_sibling": { "root": format!("{sibling_slash}/runs/abc") }
                },
                "deleted_workspace_ids": ["w_del"]
            })
            .to_string(),
        )
        .unwrap();
        fs::write(
            home.join(".kimi-code/projects.json"),
            json!({
                "schema_version": 2,
                "projects": {
                    "w1_dup": { "root": custom_root.join("runs/abc/n1").to_string_lossy().into_owned() },
                    "p2": { "root": default_root.join("scratch/migrated-only").to_string_lossy().into_owned() },
                    "p_agy_slot": { "root": custom_root.join("blind/agy-2").to_string_lossy().into_owned() },
                    "p_del": { "root": custom_root.join("runs/deleted-p").to_string_lossy().into_owned() }
                },
                "deleted_project_ids": ["p_del"]
            })
            .to_string(),
        )
        .unwrap();

        // Agy last_conversations.json
        fs::create_dir_all(home.join(".gemini/antigravity-cli/cache")).unwrap();
        fs::write(
            home.join(".gemini/antigravity-cli/cache/last_conversations.json"),
            json!({
                custom_root.join("runs/run1/node1").to_string_lossy().into_owned(): "c1",
                default_root.join("scratch/s1").to_string_lossy().into_owned(): "c2",
                custom_root.join("blind/agy-1").to_string_lossy().into_owned(): "c3",
                custom_root.join("blind/kimi-8").to_string_lossy().into_owned(): "c4",
                custom_root.join("kimi-oneshot").to_string_lossy().into_owned(): "c5",
                sibling_root.join("runs/run1/node1").to_string_lossy().into_owned(): "c6",
                home.join("work/normal-project").to_string_lossy().into_owned(): "c7"
            })
            .to_string(),
        )
        .unwrap();

        let out = collect_with_home(&custom_root, Some(&home));
        assert!(out["claude"]["error"].is_null());
        assert_eq!(out["claude"]["count"], 2, "claude: {out}");
        let claude_paths: Vec<&str> = out["claude"]["paths"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(claude_paths
            .iter()
            .any(|p| Path::new(p).file_name().unwrap().to_string_lossy().eq_ignore_ascii_case(&encode_claude_project_name(&custom_scratch_run))));
        assert!(claude_paths
            .iter()
            .any(|p| Path::new(p).file_name().unwrap().to_string_lossy().eq_ignore_ascii_case(&encode_claude_project_name(&default_run))));

        assert!(out["kimi"]["error"].is_null(), "{out}");
        assert_eq!(out["kimi"]["count"], 2, "kimi: {out}");
        assert!(out["kimi"]["paths"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str().unwrap().contains("migrated-only")));

        assert!(out["agy"]["error"].is_null(), "{out}");
        assert_eq!(out["agy"]["count"], 2, "agy: {out}");
        let agy_paths: Vec<&str> = out["agy"]["paths"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(agy_paths.iter().all(|p| !p.contains("blind\\agy-1") && !p.contains("aicross-data-other")));

        let _ = fs::remove_dir_all(dir);
    }
}
