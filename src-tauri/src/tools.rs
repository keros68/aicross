use crate::models::{ToolDefinition, ToolStatus};
use std::io::{Read, Result as IoResult};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

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
    let stdout = child.stdout.take()?;
    let stderr = child.stderr.take()?;
    let stdout_reader = pipe_reader(stdout);
    let stderr_reader = pipe_reader(stderr);

    let deadline = Instant::now() + Duration::from_secs(2);
    let mut timed_out = false;
    loop {
        if child.try_wait().ok()?.is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            timed_out = true;
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }

    let join_deadline = Instant::now() + Duration::from_millis(500);
    let stdout = finish_reader(stdout_reader, join_deadline);
    let stderr = finish_reader(stderr_reader, join_deadline);
    if timed_out {
        return None;
    }
    let stdout = stdout?;
    let stderr = stderr?;
    let stdout = String::from_utf8_lossy(&stdout);
    let stderr = String::from_utf8_lossy(&stderr);
    let output = if stdout.trim().is_empty() {
        stderr.as_ref()
    } else {
        stdout.as_ref()
    };
    output
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| {
            let mut value = line.trim().to_string();
            value.truncate(value.floor_char_boundary(120));
            value
        })
}

type PipeReader = (thread::JoinHandle<()>, mpsc::Receiver<IoResult<Vec<u8>>>);

fn pipe_reader<R: Read + Send + 'static>(mut reader: R) -> PipeReader {
    let (sender, receiver) = mpsc::channel();
    let handle = thread::spawn(move || {
        let mut output = Vec::new();
        let result = reader.read_to_end(&mut output).map(|_| output);
        let _ = sender.send(result);
    });
    (handle, receiver)
}

fn finish_reader(reader: PipeReader, deadline: Instant) -> Option<Vec<u8>> {
    let (handle, receiver) = reader;
    let output = receiver
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .ok()?
        .ok()?;
    handle.join().ok()?;
    Some(output)
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
