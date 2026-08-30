use crate::models::{ResourceStore, Settings, ToolDefinition};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use uuid::Uuid;

pub struct Loaded<T> {
    pub value: T,
    pub warning: Option<String>,
}

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
    write_json_in(&directory, name, value)
}

fn write_json_in<T: Serialize>(directory: &Path, name: &str, value: &T) -> Result<(), String> {
    fs::create_dir_all(directory).map_err(|error| format!("创建配置目录失败：{error}"))?;
    let contents = serde_json::to_string_pretty(value)
        .map_err(|error| format!("生成 {name} 失败：{error}"))?;
    let path = directory.join(name);
    let temporary = directory.join(format!(".{name}.{}.tmp", Uuid::new_v4()));
    let write_result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| format!("创建 {name} 临时文件失败：{error}"))?;
        file.write_all(contents.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|error| format!("写入 {name} 临时文件失败：{error}"))?;
        replace_file(&temporary, &path, name)
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    write_result
}

fn replace_file(temporary: &Path, path: &Path, name: &str) -> Result<(), String> {
    match fs::rename(temporary, path) {
        Ok(()) => return Ok(()),
        Err(error) if !path.exists() => return Err(format!("替换 {name} 失败：{error}")),
        Err(_) => {}
    }

    let directory = path
        .parent()
        .ok_or_else(|| format!("替换 {name} 失败：目标没有父目录"))?;
    let backup = directory.join(format!(".{name}.{}.bak", Uuid::new_v4()));
    fs::rename(path, &backup).map_err(|error| format!("备份现有 {name} 失败：{error}"))?;
    if let Err(error) = fs::rename(temporary, path) {
        let restore = fs::rename(&backup, path);
        return match restore {
            Ok(()) => Err(format!("替换 {name} 失败：{error}")),
            Err(restore_error) => Err(format!(
                "替换 {name} 失败：{error}；恢复旧文件失败：{restore_error}"
            )),
        };
    }
    fs::remove_file(&backup).map_err(|error| format!("清理 {name} 备份失败：{error}"))
}

pub fn load_settings() -> Result<Loaded<Settings>, String> {
    load_settings_from(&config_dir()?)
}

fn load_settings_from(directory: &Path) -> Result<Loaded<Settings>, String> {
    Ok(match read_json(directory, "settings.json") {
        Ok(value) => Loaded {
            value: value.unwrap_or_default(),
            warning: None,
        },
        Err(error) => Loaded {
            value: Settings::default(),
            warning: Some(format!("{error}；已使用默认设置")),
        },
    })
}

pub fn save_settings(settings: &Settings) -> Result<(), String> {
    write_json("settings.json", settings)
}

pub fn load_recent() -> Result<Loaded<Vec<String>>, String> {
    let directory = config_dir()?;
    Ok(match read_json::<Vec<String>>(&directory, "recent.json") {
        Ok(value) => Loaded {
            value: value
                .unwrap_or_default()
                .into_iter()
                .map(strip_windows_verbatim_prefix)
                .collect(),
            warning: None,
        },
        Err(error) => Loaded {
            value: Vec::new(),
            warning: Some(format!("{error}；已清空最近项目")),
        },
    })
}

pub fn save_recent(recent: &[String]) -> Result<(), String> {
    write_json("recent.json", &recent)
}

pub fn load_resources() -> Result<Loaded<ResourceStore>, String> {
    let directory = config_dir()?;
    Ok(
        match read_json::<ResourceStore>(&directory, "resources.json") {
            Ok(Some(value)) if value.version == 1 => Loaded {
                value,
                warning: None,
            },
            Ok(Some(value)) => Loaded {
                value: ResourceStore::default(),
                warning: Some(format!(
                    "resources.json 版本不受支持：{}；已使用空资源清单",
                    value.version
                )),
            },
            Ok(None) => Loaded {
                value: ResourceStore::default(),
                warning: None,
            },
            Err(error) => Loaded {
                value: ResourceStore::default(),
                warning: Some(format!("{error}；已使用空资源清单")),
            },
        },
    )
}

pub fn save_resources(resources: &ResourceStore) -> Result<(), String> {
    write_json("resources.json", resources)
}

pub fn load_tools() -> Result<Loaded<Vec<ToolDefinition>>, String> {
    load_tools_from(&config_dir()?)
}

pub fn add_tool_override(definition: ToolDefinition) -> Result<Vec<ToolDefinition>, String> {
    let directory = config_dir()?;
    let mut overrides =
        read_json::<Vec<ToolDefinition>>(&directory, "tools.json")?.unwrap_or_default();
    if overrides.iter().any(|tool| tool.id == definition.id) {
        return Err(format!("工具标识已存在：{}", definition.id));
    }
    overrides.push(definition);
    write_json("tools.json", &overrides)?;
    load_tools().map(|loaded| loaded.value)
}

fn load_tools_from(directory: &Path) -> Result<Loaded<Vec<ToolDefinition>>, String> {
    let mut tools: Vec<ToolDefinition> = serde_json::from_str(include_str!("../tools.json"))
        .map_err(|error| format!("解析内置 tools.json 失败：{error}"))?;

    match read_json::<Vec<ToolDefinition>>(directory, "tools.json") {
        Ok(Some(overrides)) => {
            for definition in overrides {
                if let Some(existing) = tools.iter_mut().find(|tool| tool.id == definition.id) {
                    *existing = definition;
                } else {
                    tools.push(definition);
                }
            }
        }
        Ok(None) => {}
        Err(error) => {
            return Ok(Loaded {
                value: tools,
                warning: Some(format!("{error}；已使用内置工具注册表")),
            });
        }
    }

    Ok(Loaded {
        value: tools,
        warning: None,
    })
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

        let loaded = load_tools_from(&directory).expect("merge overrides");
        assert!(loaded.warning.is_none());
        let tools = loaded.value;
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
        let loaded = load_tools_from(&directory).expect("invalid overrides fall back");
        let warning = loaded.warning.expect("invalid overrides must be visible");
        assert!(warning.contains("解析"));
        assert!(warning.contains(directory.join("tools.json").to_string_lossy().as_ref()));
        assert_eq!(loaded.value[0].id, "claude");

        fs::remove_dir_all(&directory).expect("remove fixture");
    }

    #[test]
    fn damaged_settings_do_not_prevent_loading() {
        let directory = std::env::temp_dir().join(format!("aicross-config-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).expect("create fixture");
        fs::write(directory.join("settings.json"), "{damaged json")
            .expect("write damaged settings");

        let loaded = load_settings_from(&directory).expect("fall back from damaged settings");
        assert_eq!(
            loaded.value.default_shell,
            Settings::default().default_shell
        );
        let warning = loaded.warning.expect("damaged settings warning");
        assert!(warning.contains("解析"));
        assert!(warning.contains(directory.join("settings.json").to_string_lossy().as_ref()));

        fs::remove_dir_all(&directory).expect("remove fixture");
    }

    #[test]
    fn json_config_can_be_replaced_repeatedly() {
        let directory = std::env::temp_dir().join(format!("aicross-config-{}", Uuid::new_v4()));
        write_json_in(
            &directory,
            "resources.json",
            &serde_json::json!({"version": 1}),
        )
        .expect("first write");
        write_json_in(
            &directory,
            "resources.json",
            &serde_json::json!({"version": 2}),
        )
        .expect("replacement write");
        let value: serde_json::Value = read_json(&directory, "resources.json")
            .expect("read replacement")
            .expect("resources exists");
        assert_eq!(value["version"], 2);
        fs::remove_dir_all(&directory).expect("remove fixture");
    }
}
