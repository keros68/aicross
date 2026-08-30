import { invoke } from "@tauri-apps/api/core";
import type {
  ChangeEntry,
  DiffResult,
  FileNode,
  InitialState,
  AddManualAgentRequest,
  AddResourceModelRequest,
  ConnectionOverview,
  ResourceModelKey,
  ResourceOperation,
  ResourceStore,
  SessionEntry,
  Settings,
  SmokeSelectionRequest,
  Tool,
  UpdateResourceModelRequest,
} from "./types";

export const api = {
  initialState: () => invoke<InitialState>("get_initial_state"),
  tools: () => invoke<Tool[]>("detect_tools"),
  setProject: (projectPath: string) =>
    invoke<InitialState>("set_project", { projectPath }),
  saveSettings: (settings: Settings) =>
    invoke<Settings>("save_settings", { settings }),
  listDirectory: (relativePath: string) =>
    invoke<FileNode[]>("list_directory", { relativePath }),
  openFile: (relativePath: string) =>
    invoke<void>("open_file", { relativePath }),
  refreshChanges: () => invoke<ChangeEntry[]>("refresh_changes"),
  resetBaseline: () => invoke<void>("reset_baseline"),
  diff: (relativePath: string) =>
    invoke<DiffResult>("get_file_diff", { relativePath }),
  sessions: () => invoke<SessionEntry[]>("list_sessions"),
  openTerminal: (request: {
    terminalId: string;
    toolId: string;
    rows: number;
    cols: number;
    env: Record<string, string>;
  }) => invoke<void>("open_terminal", { request }),
  writeTerminal: (terminalId: string, data: string) =>
    invoke<void>("write_terminal", { terminalId, data }),
  resizeTerminal: (terminalId: string, rows: number, cols: number) =>
    invoke<void>("resize_terminal", { terminalId, rows, cols }),
  closeTerminal: (terminalId: string) =>
    invoke<void>("close_terminal", { terminalId }),
  resources: () => invoke<ResourceStore>("get_resources"),
  detectResourceAgent: (agentId: string) =>
    invoke<ResourceOperation>("detect_resource_agent", { agentId }),
  detectAllResources: (agentIds: string[]) =>
    invoke<ResourceOperation>("detect_all_resources", { agentIds }),
  addResourceModel: (request: AddResourceModelRequest) =>
    invoke<ResourceStore>("add_resource_model", { request }),
  updateResourceModel: (request: UpdateResourceModelRequest) =>
    invoke<ResourceStore>("update_resource_model", { request }),
  removeResourceModel: (request: ResourceModelKey) =>
    invoke<ResourceStore>("remove_resource_model", { request }),
  addManualAgent: (request: AddManualAgentRequest) =>
    invoke<ResourceStore>("add_manual_agent", { request }),
  smokeResourceModel: (request: ResourceModelKey) =>
    invoke<ResourceStore>("smoke_resource_model", { request }),
  smokeSelectedResources: (request: SmokeSelectionRequest) =>
    invoke<ResourceOperation>("smoke_selected_resources", { request }),
  connectionOverview: () =>
    invoke<ConnectionOverview>("get_connection_overview"),
};
