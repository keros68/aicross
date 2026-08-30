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
    #[serde(default)]
    pub ai_cross_dir: String,
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
            ai_cross_dir: String::new(),
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ResourceStore {
    pub version: u32,
    pub agents: Vec<ResourceAgent>,
}

impl Default for ResourceStore {
    fn default() -> Self {
        Self {
            version: 1,
            agents: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ResourceAgent {
    pub id: String,
    pub kind: String,
    pub detected: bool,
    pub version: String,
    pub models: Vec<ResourceModel>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ResourceModel {
    pub id: String,
    pub agent: String,
    pub provider: String,
    pub vendor: String,
    pub billing_source: String,
    pub tier: String,
    pub thinking: String,
    pub status: String,
    pub source: String,
    pub smoked_at: String,
    pub smoke_model: String,
    pub note: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AddResourceModelRequest {
    pub agent_id: String,
    pub id: String,
    #[serde(default)]
    pub provider: String,
    pub vendor: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UpdateResourceModelRequest {
    pub agent_id: String,
    pub id: String,
    #[serde(default)]
    pub provider: String,
    pub vendor: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ResourceModelKey {
    pub agent_id: String,
    pub id: String,
    #[serde(default)]
    pub provider: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AddManualAgentRequest {
    pub name: String,
    pub binary: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SmokeSelectionRequest {
    #[serde(default)]
    pub agent_ids: Vec<String>,
    #[serde(default)]
    pub cc_switch_providers: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResourceOperation {
    pub store: ResourceStore,
    pub errors: Vec<String>,
    pub cc_switch_providers: Vec<CcSwitchProvider>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ConnectionOverview {
    pub cc_switch_providers: Vec<CcSwitchProvider>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CcSwitchProvider {
    pub name: String,
    pub has_token: bool,
    pub models: Vec<String>,
}
