export type Tool = {
  id: string;
  name: string;
  binary: string;
  defaultArgs: string[];
  description: string;
  detected: boolean;
  executablePath: string | null;
  version: string;
};

export type TabSettings = {
  workspace: boolean;
  resources: boolean;
  sessions: boolean;
  changes: boolean;
  plan: boolean;
  tasks: boolean;
};

export type Settings = {
  defaultShell: string;
  aiCrossDir: string;
  tabs: TabSettings;
};

export type InitialState = {
  projectPath: string;
  recentProjects: string[];
  settings: Settings;
  warnings: string[];
};

export type FileNode = {
  name: string;
  relativePath: string;
  absolutePath: string;
  isDir: boolean;
  isSymlink: boolean;
  muted: boolean;
};

export type ChangeEntry = {
  path: string;
  kind: "added" | "modified" | "deleted";
  binary: boolean;
  oldSize: number | null;
  newSize: number | null;
};

export type DiffResult = {
  path: string;
  binary: boolean;
  text: string;
};

export type SessionEntry = {
  id: string;
  timestamp: string;
  tool: string;
  model: string;
  projectPath: string;
  firstMessage: string;
};

export type TerminalTab = {
  id: string;
  toolId: string;
  title: string;
  running: boolean;
  env: Record<string, string>;
};

export type EnvEntry = { key: string; value: string };

export type ResourceStore = {
  version: number;
  agents: ResourceAgent[];
};

export type ResourceAgent = {
  id: string;
  kind: string;
  detected: boolean;
  version: string;
  models: ResourceModel[];
};

export type ResourceModel = {
  id: string;
  agent: string;
  provider: string;
  vendor: string;
  billing_source: string;
  tier: string;
  thinking: string;
  status: string;
  source: string;
  smoked_at: string;
  smoke_model: string;
  meta: string;
  note: string;
};

export type CcSwitchProvider = {
  name: string;
  has_token: boolean;
  models: string[];
};

export type ResourceOperation = {
  store: ResourceStore;
  errors: string[];
  cc_switch_providers: CcSwitchProvider[] | null;
};

export type ConnectionOverview = {
  cc_switch_providers: CcSwitchProvider[];
};

export type ResourceModelRequest = {
  agent_id: string;
  id: string;
  provider: string;
  vendor: string;
  tier: string;
  note: string;
};

export type ResourceModelKey = {
  agent_id: string;
  id: string;
  provider: string;
};

export type AddManualAgentRequest = {
  name: string;
  binary: string;
  args: string[];
};

export type SmokeSelectionRequest = {
  agent_ids: string[];
  cc_switch_providers: string[];
};
