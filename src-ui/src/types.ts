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
  tabs: TabSettings;
};

export type InitialState = {
  projectPath: string;
  recentProjects: string[];
  settings: Settings;
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
