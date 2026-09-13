//! Shared `~/.aicross/resources.json` contract.
//!
//! The resource-management implementation from phase 1 still lives in the
//! archived port.  The fork only needs the persisted contract for graph
//! routing and dispatch, so this module is the single reader used by both.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ResourceStore {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub agents: Vec<ResourceAgent>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ResourceAgent {
    #[serde(default)]
    #[allow(dead_code)]
    pub id: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub name: String,
    #[serde(default)]
    pub models: Vec<ResourceModel>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ResourceModel {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub billing_source: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub thinking: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub smoked_at: String,
    #[serde(default)]
    pub smoke_model: String,
    #[serde(default)]
    pub smoke_failure: String,
    #[serde(default)]
    pub meta: String,
    #[serde(default)]
    pub note: String,
}

pub fn load(path: &Path) -> Result<ResourceStore, String> {
    let text =
        fs::read_to_string(path).map_err(|error| format!("读取 resources.json 失败：{error}"))?;
    let store: ResourceStore =
        serde_json::from_str(&text).map_err(|error| format!("resources.json 解析失败：{error}"))?;
    if store.version != 1 {
        return Err(format!("resources.json 版本不受支持：{}", store.version));
    }
    Ok(store)
}

pub fn connected(path: &Path) -> Result<Vec<ResourceModel>, String> {
    Ok(load(path)?
        .agents
        .into_iter()
        .flat_map(|agent| agent.models)
        .filter(|model| model.status == "已接入")
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connected_filters_status() {
        let root = std::env::temp_dir().join(format!("aicross-resources-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("resources.json");
        fs::write(
            &path,
            r#"{"version":1,"agents":[{"id":"codex","name":"Codex","models":[{"id":"a","agent":"codex","status":"已接入"},{"id":"b","agent":"codex","status":"待冒烟"}]}]}"#,
        )
        .unwrap();
        let models = connected(&path).unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "a");
        let _ = fs::remove_dir_all(root);
    }
}
