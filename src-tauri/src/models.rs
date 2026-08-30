use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub id: String,
    pub name: String,
    pub binary: String,
    #[serde(default)]
    pub default_args: Vec<String>,
    pub description: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    #[serde(flatten)]
    pub definition: ToolDefinition,
    pub detected: bool,
    pub executable_path: Option<String>,
    pub version: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabSettings {
    pub workspace: bool,
    pub resources: bool,
    pub sessions: bool,
    pub changes: bool,
    pub plan: bool,
    pub tasks: bool,
}

impl Default for TabSettings {
    fn default() -> Self {
        Self {
            workspace: true,
            resources: true,
            sessions: true,
            changes: true,
            plan: false,
            tasks: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub default_shell: String,
    pub tabs: TabSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_shell: if cfg!(windows) {
                "pwsh.exe".to_string()
            } else {
                "sh".to_string()
            },
            tabs: TabSettings::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialState {
    pub project_path: String,
    pub recent_projects: Vec<String>,
    pub settings: Settings,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalRequest {
    pub terminal_id: String,
    pub tool_id: String,
    pub rows: u16,
    pub cols: u16,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileNode {
    pub name: String,
    pub relative_path: String,
    pub absolute_path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub muted: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeEntry {
    pub path: String,
    pub kind: String,
    pub binary: bool,
    pub old_size: Option<u64>,
    pub new_size: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffResult {
    pub path: String,
    pub binary: bool,
    pub text: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEntry {
    pub id: String,
    pub timestamp: String,
    pub tool: String,
    pub model: String,
    pub project_path: String,
    pub first_message: String,
}
