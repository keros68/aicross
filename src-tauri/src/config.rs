use crate::models::{Settings, ToolDefinition};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

fn config_dir() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|path| path.join(".aicross"))
        .ok_or_else(|| "无法确定用户目录".to_string())
}

fn read_json<T: DeserializeOwned>(directory: &Path, name: &str) -> Result<Option<T>, String> {
    let path = directory.join(name);
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("读取 {} 失败：{error}", path.display())),
    };
    serde_json::from_str(&contents)
        .map(Some)
        .map_err(|error| format!("解析 {} 失败：{error}", path.display()))
}

fn write_json<T: Serialize>(name: &str, value: &T) -> Result<(), String> {
    let directory = config_dir()?;
    fs::create_dir_all(&directory).map_err(|error| format!("创建配置目录失败：{error}"))?;
    let contents = serde_json::to_string_pretty(value)
        .map_err(|error| format!("生成 {name} 失败：{error}"))?;
    fs::write(directory.join(name), contents).map_err(|error| format!("写入 {name} 失败：{error}"))
}

pub fn load_settings() -> Result<Settings, String> {
    Ok(read_json(&config_dir()?, "settings.json")?.unwrap_or_default())
}

pub fn save_settings(settings: &Settings) -> Result<(), String> {
    write_json("settings.json", settings)
}

pub fn load_recent() -> Result<Vec<String>, String> {
    Ok(read_json::<Vec<String>>(&config_dir()?, "recent.json")?
        .unwrap_or_default()
        .into_iter()
        .map(strip_windows_verbatim_prefix)
        .collect())
}

pub fn save_recent(recent: &[String]) -> Result<(), String> {
    write_json("recent.json", &recent)
}

pub fn load_tools() -> Result<Vec<ToolDefinition>, String> {
    load_tools_from(&config_dir()?)
}

fn load_tools_from(directory: &Path) -> Result<Vec<ToolDefinition>, String> {
    let mut tools: Vec<ToolDefinition> = serde_json::from_str(include_str!("../tools.json"))
        .map_err(|error| format!("解析内置 tools.json 失败：{error}"))?;

    if let Some(overrides) = read_json::<Vec<ToolDefinition>>(directory, "tools.json")? {
        for definition in overrides {
            if let Some(existing) = tools.iter_mut().find(|tool| tool.id == definition.id) {
                *existing = definition;
            } else {
                tools.push(definition);
            }
        }
    }

    Ok(tools)
}

fn strip_windows_verbatim_prefix(path: String) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    path.strip_prefix(r"\\?\").unwrap_or(&path).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn tools_overrides_merge_existing_and_new_entries_and_report_invalid_json() {
        let directory = std::env::temp_dir().join(format!("aicross-config-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).expect("create fixture");
        fs::write(
            directory.join("tools.json"),
            r#"[
              {"id":"codex","name":"Codex Local","binary":"codex-local","defaultArgs":["--safe"],"description":"override"},
              {"id":"extra","name":"Extra","binary":"extra","defaultArgs":[],"description":"new"}
            ]"#,
        )
        .expect("write valid overrides");

        let tools = load_tools_from(&directory).expect("merge overrides");
        let codex = tools.iter().find(|tool| tool.id == "codex").expect("codex");
        assert_eq!(codex.name, "Codex Local");
        assert_eq!(codex.binary, "codex-local");
        assert_eq!(codex.default_args, ["--safe"]);
        assert!(tools.iter().any(|tool| tool.id == "extra"));

        fs::write(
            directory.join("tools.json"),
            r#"[{"id":"codex","name":"missing fields"}]"#,
        )
        .expect("write invalid overrides");
        let error = load_tools_from(&directory).expect_err("invalid overrides must be visible");
        assert!(error.contains("解析"));
        assert!(error.contains("tools.json"));

        fs::remove_dir_all(&directory).expect("remove fixture");
    }
}
