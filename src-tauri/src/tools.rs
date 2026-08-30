use crate::models::{ToolDefinition, ToolStatus};
use crate::process;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn detect_all(definitions: &[ToolDefinition]) -> Vec<ToolStatus> {
    definitions.iter().map(detect).collect()
}

pub fn resolve_binary(definition: &ToolDefinition) -> Option<PathBuf> {
    let resolved = resolve_binary_name(&definition.binary);

    if resolved.is_some() || definition.id != "kimi" || !cfg!(windows) {
        return resolved;
    }

    dirs::home_dir()
        .map(|home| home.join(".kimi-code").join("bin").join("kimi.exe"))
        .filter(|path| path.is_file())
}

pub fn resolve_binary_name(binary: &str) -> Option<PathBuf> {
    let path = Path::new(binary);
    if path.components().count() > 1 && path.is_file() {
        return dunce::canonicalize(path).ok();
    }

    let lookup = if cfg!(windows) { "where.exe" } else { "which" };
    Command::new(lookup)
        .arg(binary)
        .env("AI_CROSS_PEER", "1")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|output| preferred_lookup_result(&output))
}

fn preferred_lookup_result(output: &str) -> Option<PathBuf> {
    let paths: Vec<PathBuf> = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(PathBuf::from)
        .collect();
    if cfg!(windows) {
        return paths
            .iter()
            .find(|path| {
                path.extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|extension| {
                        matches!(
                            extension.to_ascii_lowercase().as_str(),
                            "exe" | "com" | "cmd" | "bat"
                        )
                    })
            })
            .cloned()
            .or_else(|| paths.into_iter().next());
    }
    paths.into_iter().next()
}

fn detect(definition: &ToolDefinition) -> ToolStatus {
    let path = resolve_binary(definition);
    let version = path
        .as_deref()
        .and_then(process::version_for)
        .unwrap_or_else(|| "版本未知".to_string());

    ToolStatus {
        definition: definition.clone(),
        detected: path.is_some(),
        executable_path: path.map(|value| value.to_string_lossy().to_string()),
        version,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_registry_detects_all_five_tools() {
        let definitions = crate::config::load_tools().expect("load tools").value;
        let expected = ["claude", "codex", "kimi", "pi", "powershell"];
        assert_eq!(
            definitions
                .iter()
                .map(|tool| tool.id.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        let statuses = detect_all(&definitions);
        assert_eq!(statuses.len(), expected.len());
        for status in statuses {
            println!(
                "{}: detected={}, version={}",
                status.definition.id, status.detected, status.version
            );
            assert!(!status.version.is_empty());
        }
    }
}
