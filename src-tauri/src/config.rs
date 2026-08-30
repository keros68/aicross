use crate::models::{Settings, ToolDefinition};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

fn config_dir() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|path| path.join(".aicross"))
        .ok_or_else(|| "无法确定用户目录".to_string())
}

fn read_json<T: DeserializeOwned>(name: &str) -> Option<T> {
    let path = config_dir().ok()?.join(name);
    let contents = fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

fn write_json<T: Serialize>(name: &str, value: &T) -> Result<(), String> {
    let directory = config_dir()?;
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let contents = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    fs::write(directory.join(name), contents).map_err(|error| error.to_string())
}

pub fn load_settings() -> Settings {
    read_json("settings.json").unwrap_or_default()
}

pub fn save_settings(settings: &Settings) -> Result<(), String> {
    write_json("settings.json", settings)
}

pub fn load_recent() -> Vec<String> {
    read_json("recent.json").unwrap_or_default()
}

pub fn save_recent(recent: &[String]) -> Result<(), String> {
    write_json("recent.json", &recent)
}

pub fn load_tools() -> Result<Vec<ToolDefinition>, String> {
    let mut tools: Vec<ToolDefinition> =
        serde_json::from_str(include_str!("../tools.json")).map_err(|error| error.to_string())?;

    if let Some(overrides) = read_json::<Vec<ToolDefinition>>("tools.json") {
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
