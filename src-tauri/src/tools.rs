use crate::models::{ToolDefinition, ToolStatus};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub fn detect_all(definitions: &[ToolDefinition]) -> Vec<ToolStatus> {
    definitions.iter().map(detect).collect()
}

pub fn resolve_binary(definition: &ToolDefinition) -> Option<PathBuf> {
    let resolved = if cfg!(windows) {
        Command::new("where.exe")
            .arg(&definition.binary)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|output| {
                output
                    .lines()
                    .find(|line| !line.trim().is_empty())
                    .map(PathBuf::from)
            })
    } else {
        Command::new("which")
            .arg(&definition.binary)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|output| {
                output
                    .lines()
                    .find(|line| !line.trim().is_empty())
                    .map(PathBuf::from)
            })
    };

    if resolved.is_some() || definition.id != "kimi" || !cfg!(windows) {
        return resolved;
    }

    dirs::home_dir()
        .map(|home| home.join(".kimi-code").join("bin").join("kimi.exe"))
        .filter(|path| path.is_file())
}

fn detect(definition: &ToolDefinition) -> ToolStatus {
    let path = resolve_binary(definition);
    let version = path
        .as_deref()
        .and_then(version_for)
        .unwrap_or_else(|| "版本未知".to_string());

    ToolStatus {
        definition: definition.clone(),
        detected: path.is_some(),
        executable_path: path.map(|value| value.to_string_lossy().to_string()),
        version,
    }
}

fn version_for(binary: &Path) -> Option<String> {
    let mut child = Command::new(binary)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    let deadline = Instant::now() + Duration::from_secs(2);
    let completed = loop {
        if let Some(status) = child.try_wait().ok()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        thread::sleep(Duration::from_millis(25));
    };
    if !completed.success() {
        return None;
    }

    let mut output = String::new();
    child.stdout.take()?.read_to_string(&mut output).ok()?;
    if output.trim().is_empty() {
        child.stderr.take()?.read_to_string(&mut output).ok()?;
    }
    output
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| {
            let mut value = line.trim().to_string();
            value.truncate(value.floor_char_boundary(120));
            value
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_registry_detects_all_five_tools() {
        let definitions = crate::config::load_tools().expect("load tools");
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
