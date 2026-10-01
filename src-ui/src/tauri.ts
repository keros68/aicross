// Tauri v2 typed invoke wrapper

// Extend Window with Tauri globals to avoid TS2339
declare global {
  interface Window {
    __TAURI_INTERNALS__?: { invoke: (cmd: string, args?: unknown) => Promise<unknown> };
    __TAURI__?: {
      invoke?: (cmd: string, args?: unknown) => Promise<unknown>;
      core?: { invoke: (cmd: string, args?: unknown) => Promise<unknown> };
    };
  }
}

// isTauri: evaluated once at module load.
// Tauri injects __TAURI_INTERNALS__ synchronously before any scripts run.
export const isTauri =
  typeof window !== 'undefined' &&
  (!!window.__TAURI_INTERNALS__ || !!window.__TAURI__);

// Resolve the invoke function across Tauri v1 / v2
function resolveInvoke(): ((cmd: string, args?: Record<string, unknown>) => Promise<unknown>) | null {
  const w = window as unknown as Record<string, unknown>;
  const internals = w.__TAURI_INTERNALS__ as Record<string, unknown> | undefined;
  if (internals && typeof internals.invoke === 'function') return internals.invoke as never;
  const tauri = w.__TAURI__ as Record<string, unknown> | undefined;
  if (tauri) {
    const core = tauri.core as Record<string, unknown> | undefined;
    if (core && typeof core.invoke === 'function') return core.invoke as never;
    if (typeof tauri.invoke === 'function') return tauri.invoke as never;
  }
  return null;
}

let _invoke = isTauri ? resolveInvoke() : null;

export function retryInvoke() {
  if (isTauri && !_invoke) _invoke = resolveInvoke();
  return _invoke;
}

// ── Git changes-panel types (mirror of src/git.rs serde output) ──
export interface GitFileEntry {
  /** Absolute, forward-slashed path (repo_root + "/" + rel). */
  path: string;
  /** Repo-relative path exactly as git reports it; diff/show specs use this. */
  rel: string;
  /** Single-letter status: M A D R C U or ? (untracked). */
  status: string;
  added: number;
  deleted: number;
}
/** A commit's metadata for the session-commits list (files fetched lazily via
 *  `gitCommitFiles` when the user expands a commit). */
export interface CommitMeta {
  /** Short hash (e.g. "abc1234"). */
  hash: string;
  /** Commit subject (first line). */
  message: string;
  author: string;
  /** Commit time, epoch seconds. */
  time: number;
}
export type GitChanges =
  | { state: 'no_git' }
  | { state: 'not_repo' }
  | {
      state: 'ok';
      repo_root: string;
      branch: string;
      /** Tracked files with uncommitted changes (staged OR unstaged, merged —
       *  numstat is HEAD↔worktree so the diff is "what changed since the last
       *  commit"). The staged/unstaged split was collapsed 2026-07-06. */
      uncommitted: GitFileEntry[];
      untracked: GitFileEntry[];
      /** Commits made since this Coffee CLI window opened (baseline..HEAD),
       *  metadata only — files fetched lazily via `gitCommitFiles`. Push-
       *  agnostic (push doesn't move HEAD). Reset on app close. */
      session_commits: CommitMeta[];
    };

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!_invoke) throw new Error('Tauri IPC not available');
  return _invoke(cmd, args) as Promise<T>;
}

// 对话图契约类型的正式来源是 graph/types.ts（与 Rust orchestration.rs 镜像）。
import type {
  DispatchableModel,
  Graph as ConversationGraph,
  Preset as GraphPreset,
} from './graph/types';

// TASK-06 对账/回滚契约：类型定义在 graph/dispatch.ts（纯函数与它同住），
// 这里只做类型引用并转出，命令签名用它们。
import type { ReconcileSummary, RollbackSummary } from './graph/dispatch';
export type { ReconcileSummary, RollbackSummary };

import type { CommanderSettings } from './commander/types';

// 资源接入契约类型的正式来源是 resources/types.ts（与 Rust resources.rs 镜像）。
import type {
  AddManualAgentRequest,
  ResourceModelKey,
  ResourceModelRequest,
  ResourceOperation,
  ResourceStore,
} from './resources/types';

// ─── Type Definitions ────────────────────────────────────────────────────────

export interface SavedSession {
  id: string;
  name: string;
  tool: string;
  cwd: string;
  session_token: string | null;
  saved_at: string;
  file_path?: string;
  turn_count?: number;
}

export interface DirEntryInfo {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
}

export interface FontInfo {
  family: string;
  monospace: boolean;
}

// ─── Typed Commands ──────────────────────────────────────────────────────────

export const commands = {
  pickFolder: () => invoke<string>('pick_folder'),

  // Window decorators
  windowMinimize: () => invoke<void>('window_minimize'),
  windowMaximize: () => invoke<void>('window_maximize'),
  windowClose: () => invoke<void>('window_close'),

  // Tier Terminal API
  tierTerminalStart: (sessionId: string, tool: string | null, cols: number, rows: number, themeMode: string, locale?: string, toolData?: string, cwd?: string, resumeToken?: string, shell?: string, extraArgs?: string[]) =>
    invoke<void>('tier_terminal_start', { sessionId, tool, toolData: toolData ?? null, cols, rows, themeMode, locale: locale ?? null, cwd: cwd ?? null, resumeToken: resumeToken ?? null, shell: shell ?? null, extraArgs: extraArgs ?? null }),
  tierTerminalInput: (sessionId: string, data: string) =>
    invoke<void>('tier_terminal_input', { sessionId, data }),
  /** Raw write to PTY — does NOT trigger agent-status detection.
   *  Used for system-generated input (auto-skip prompts, etc.). */
  tierTerminalRawWrite: (sessionId: string, data: string) =>
    invoke<void>('tier_terminal_raw_write', { sessionId, data }),
  tierTerminalKill: (sessionId: string) =>
    invoke<void>('tier_terminal_kill', { sessionId }),
  tierTerminalResize: (sessionId: string, cols: number, rows: number) =>
    invoke<void>('tier_terminal_resize', { sessionId, cols, rows }),

  /** Notify the Rust backend that the window's visibility changed.
   *  When hidden=true, every per-session worker thread (ticker, emitter)
   *  widens its sleep / coalesce window so a backgrounded Coffee CLI
   *  drops to near-zero CPU instead of running its full foreground
   *  cadence. Apple Silicon laptops in particular need this to keep
   *  the chassis cool when users leave the app open all day. */
  setBackgroundMode: (hidden: boolean) =>
    invoke<void>('set_background_mode', { hidden }),

  /** Per-tab visibility signal — narrower than setBackgroundMode. Flip when
   *  this session's terminal element enters/leaves the viewport (a tab
   *  switch, not just the whole window hiding) so its backend emitter can
   *  widen its coalesce window while nobody is looking at that tab. */
  setSessionActive: (sessionId: string, active: boolean) =>
    invoke<void>('set_session_active', { sessionId, active }),

  // Session Resume
  getNativeHistory: () => invoke<SavedSession[]>('get_native_history'),
  /** Per-session activity for the contribution heatmap.
   *  One entry per session file: { ts: epoch seconds, count: msg lines }.
   *  Frontend buckets ts into local-day boxes for the grid. */
  getMessageHeatmap: () =>
    invoke<{ ts: number; count: number }[]>('get_message_heatmap'),
  readNativeSession: (filePath: string) => invoke<string>('read_native_session', { filePath }),
  readOpencodeSession: (sessionId: string) =>
    invoke<string>('read_opencode_session', { sessionId }),
  // Hermes Agent sessions from the newer SQLite state.db (no per-session
  // file). Returns the same newline-delimited {message:{role,content}} shape
  // as readNativeSession so ChatReader's parser handles it unchanged.
  readHermesSession: (sessionToken: string) =>
    invoke<string>('read_hermes_session', { sessionToken }),
  // MiMo Code (OpenCode fork) — same SQLite schema, read from mimocode.db.
  readMimocodeSession: (sessionToken: string) =>
    invoke<string>('read_mimocode_session', { sessionToken }),
  checkNetworkPort: (host: string, port: number) => invoke<boolean>('check_network_port', { host, port }),

  // Tool availability detection
  checkToolsInstalled: () =>
    invoke<Record<string, boolean>>('check_tools_installed'),

  // External launch request (`launch --tool <id> [--cwd <dir>]`) handed to
  // the GUI at cold start. Drained exactly once by CenterPanel on mount —
  // warm-start requests arrive as 'launch-request' events instead.
  takePendingLaunch: () =>
    invoke<{ tool: string; cwd?: string } | null>('take_pending_launch'),

  // TASK-37 查看器：冷启动时 `aicross open <run_id>` 随 argv 带进来的 run id，
  // 挂载后取一次；热启动走 'open-run-request' 事件。
  takePendingOpen: () => invoke<string | null>('take_pending_open'),
  loadRunView: (runId: string) => invoke<RunView>('load_run_view', { runId }),
  adoptRunNode: (runId: string, nodeId: string) =>
    invoke<RunView>('adopt_run_node', { runId, nodeId }),

  /** Probed availability of optional shells (pwsh / Git Bash / wsl on
   *  Windows; zsh/bash/fish/sh on Unix). Fed to the SettingsModal shell
   *  picker so it only shows shells the user actually has. Inbox shells
   *  (powershell/cmd on Windows) are assumed present and NOT in this
   *  payload. Platform-specific fields are absent cross-OS — read with
   *  optional chaining. */
  detectShells: () => invoke<{
    pwsh_available?: boolean; git_bash_available?: boolean;
    zsh_available?: boolean; bash_available?: boolean;
    fish_available?: boolean; sh_available?: boolean;
    wsl_available: boolean;
  }>('detect_shells'),

  /** Static list of tools registered in the Rust src/tools/ registry —
   *  one entry per supported AI CLI with the canonical display name.
   *  Loaded once at app boot and cached; see `lib/tool-info.ts`. */
  listTools: () => invoke<{ id: string; displayName: string }[]>('list_tools'),

  /** Gambit — save a clipboard-pasted image to a temp file and return its path.
   *  The returned absolute path is inserted into the textarea so the AI CLI agent
   *  (Claude Code, etc.) can read the image via the local filesystem. */
  saveClipboardImage: (dataBase64: string, extension: string) =>
    invoke<string>('save_clipboard_image', { dataBase64, extension }),

  /** Read an image from the OS clipboard and persist it as a PNG temp file,
   *  returning its absolute path (or null when the clipboard has no image).
   *  Replaces navigator.clipboard.read() — no WebView2 permission prompt. */
  readClipboardImage: () => invoke<string | null>('read_clipboard_image'),

  listDirectory: (path: string) => invoke<DirEntryInfo[]>('list_directory', { path }),
  listSystemFonts: () => invoke<FontInfo[]>('list_system_fonts'),

  // ── Git-backed changes panel ──────────────────────────────────────
  // The right-side "修改记录" tab reads the active folder's git working
  // tree. `gitChanges` returns no_git / not_repo / ok (with uncommitted +
  // untracked groups, plus session_commits made this window); the
  // DiffPanel pulls each side's blob via `gitShowFile` and feeds the existing
  // jsdiff + Shiki pipeline.
  gitChanges: (folder: string) => invoke<GitChanges>('git_changes', { folder }),
  // Content of `<spec>` — e.g. "HEAD:src/a.ts" (committed) or ":src/a.ts"
  // (staged/index blob). null when the path doesn't exist at that revision;
  // the panel treats null as an empty side (file renders as all-additions).
  gitShowFile: (repoRoot: string, spec: string) =>
    invoke<string | null>('git_show_file', { repoRoot, spec }),
  // `git init` a folder — backs the not-a-repo state's "initialize" button.
  gitInit: (folder: string) => invoke<void>('git_init', { folder }),
  // Capture the session baseline (current HEAD) for a repo, idempotently.
  // Called at app launch + tab switch (not poll-gated — one rev-parse). Scopes
  // the "修改记录" session-commits list to commits made this window.
  gitCaptureBaseline: (folder: string) => invoke<void>('git_capture_baseline', { folder }),
  // Files changed in a single commit (lazy — on expand in the session-commits
  // list).
  gitCommitFiles: (repoRoot: string, hash: string) =>
    invoke<GitFileEntry[]>('git_commit_files', { repoRoot, hash }),
  // Current on-disk text of a file (lossy-UTF8 so GBK / latin-1 still
  // render; null = missing / binary). Used for the working-tree "new" side
  // and for untracked files, which have no git blob to `show`.
  readTextFile: (path: string) =>
    invoke<string | null>('read_text_file', { path }),

  // File system operations
  fsDelete: (path: string) => invoke<void>('fs_delete', { path }),
  fsRename: (path: string, newName: string) => invoke<void>('fs_rename', { path, newName }),
  fsPaste: (action: string, srcPath: string, targetDir: string) =>
    invoke<void>('fs_paste', { action, srcPath, targetDir }),
  showInFolder: (path: string) => invoke<void>('show_in_folder', { path }),

  // ── Skills (Coffee CLI skill store, junctioned into ~/.claude/skills + ~/.codex/skills) ──
  // Frontend does HTTP fetch (no Rust HTTP dep), pipes bytes here.
  skillsEnsureDirs: () => invoke<void>('skills_ensure_dirs'),
  skillsWriteFile: (name: string, relPath: string, bytes: number[] | Uint8Array) =>
    invoke<void>('skills_write_file', { name, relPath, bytes: Array.from(bytes) }),
  skillsList: () => invoke<{ name: string; enabled: boolean; skillMd: string | null; iconDataUrl: string | null; path: string }[]>('skills_list'),
  /** Toggle a skill on or off. On success, returns a list of per-tool
   *  warnings — usually empty. Non-empty entries describe tools whose
   *  skills dir already contained a real folder for this skill (manual
   *  install); Coffee CLI doesn't clobber those, so the user keeps
   *  their version on those tools. UI should surface these as a
   *  per-line toast alongside the success indicator. */
  skillsToggle: (name: string, enable: boolean) =>
    invoke<string[]>('skills_toggle', { name, enable }),
  skillsDelete: (name: string) => invoke<void>('skills_delete', { name }),

  // Codex-compatible plugin marketplaces (git-cloned repos under
  // ~/.aicross/marketplace). We read the marketplace.json rule, render
  // cards, and inject the plugin path into the agent prompt — nothing more.
  addMarketplace: (gitUrl: string) => invoke<void>('add_marketplace', { gitUrl }),
  listMarketplaces: () => invoke<Marketplace[]>('list_marketplaces'),
  setMarketplacePluginEnabled: (key: string, enabled: boolean) =>
    invoke<void>('set_marketplace_plugin_enabled', { key, enabled }),
  updateMarketplace: (id: string) => invoke<void>('update_marketplace', { id }),
  deleteMarketplace: (id: string) => invoke<void>('delete_marketplace', { id }),
  openMarketplaceDir: () => invoke<void>('open_marketplace_dir'),

  // Task Board persistence (~/.aicross/tasks.json)
  loadTasks: () => invoke<string>('load_tasks'),
  saveTasks: (data: string) => invoke<void>('save_tasks', { data }),

  // Credential store — passwords live in OS keychain, never in localStorage
  savePassword: (host: string, username: string, password: string) =>
    invoke<void>('save_password', { host, username, password }),
  loadPassword: (host: string, username: string) =>
    invoke<string | null>('load_password', { host, username }),
  deletePassword: (host: string, username: string) =>
    invoke<void>('delete_password', { host, username }),
  openUrl: (url: string) =>
    invoke<void>('open_url', { url }),

  // In-app self-update (Windows): download the latest installer from
  // coffeecli.com/download/<os> with streamed progress, launch it, exit.
  // Emits `self-update-progress` while it runs (see onSelfUpdateProgress).
  // Rejects on non-Windows / download failure — caller falls back to openUrl.
  downloadAndInstallUpdate: () =>
    invoke<void>('download_and_install_update'),

  // Live fs watcher — subscribes to OS-native events under `path` and
  // emits `fs-refresh` Tauri events that Explorer already listens for.
  // Calling start with a new path implicitly replaces the previous watcher.
  startFsWatcher: (path: string) =>
    invoke<void>('start_fs_watcher', { path }),
  stopFsWatcher: () =>
    invoke<void>('stop_fs_watcher'),

  // ─── Per-tool launch overrides (~/.aicross/tools.json) ─────────────
  getToolConfig: (tool: string) =>
    invoke<ToolConfigEntry>('get_tool_config', { tool }),
  getAllToolConfigs: () =>
    invoke<Record<string, ToolConfigEntry>>('get_all_tool_configs'),
  setToolConfig: (tool: string, entry: ToolConfigEntry) =>
    invoke<void>('set_tool_config', { tool, entry }),

  // ── 对话图（第 2 期，镜像 src/orchestration.rs）────────────────────
  // status == "已接入" 的可派发模型；文件缺失/解析失败时 models 为空、
  // error 带一条中文说明，不抛异常。
  listDispatchableModels: () =>
    invoke<{ models: DispatchableModel[]; error: string | null }>('list_dispatchable_models'),
  // 内置三个预设（builtin=true）+ ~/.aicross/presets.json 的用户预设。
  listPresets: () => invoke<{ version: 1; presets: GraphPreset[] }>('list_presets'),
  saveUserPresets: (presets: GraphPreset[]) =>
    invoke<void>('save_user_presets', { presets }),
  saveGraph: (graph: ConversationGraph) => invoke<void>('save_graph', { graph }),
  loadGraph: (id: string) => invoke<ConversationGraph | null>('load_graph', { id }),
  listGraphs: () => invoke<GraphMeta[]>('list_graphs'),
  loadRoles: () => invoke<RoleSettings>('load_roles'),
  saveRoles: (settings: RoleSettings) => invoke<void>('save_roles', { settings }),
  // 前提台账（TASK-05B，~/.aicross/premises.json；损坏文件读侧给空、写侧拒写）
  loadPremises: () =>
    invoke<{ version: 1; premises: import('./graph/premise').Premise[] }>('load_premises'),
  savePremises: (file: { version: 1; premises: import('./graph/premise').Premise[] }) =>
    invoke<void>('save_premises', { file }),
  loadCommander: () => invoke<CommanderSettings>('load_commander'),
  saveCommander: (settings: CommanderSettings) =>
    invoke<void>('save_commander', { settings }),
  previewDispatch: (request: DispatchStageRequest) =>
    invoke<DispatchPreview>('preview_dispatch', { request }),
  dispatchGraphStage: (request: DispatchStageRequest) =>
    invoke<DispatchStageResult>('dispatch_graph_stage', { request }),
  cancelDispatchNode: (runId: string, nodeId: string) =>
    invoke<boolean>('cancel_dispatch_node', { runId, nodeId }),
  listConsoleRuns: () => invoke<ConsoleRun[]>('list_console_runs'),
  consoleRegisterProposal: (proposalId: string, project: string, graphId: string, reason: string) => invoke<void>('console_register_proposal', { proposalId, project, graphId, reason }),
  consoleRunUpdate: (update: ConsoleRunUpdate) => invoke<void>('console_run_update', { update }),
  consoleActionAck: (requestId: string, error?: string) => invoke<void>('console_action_ack', { requestId, error }),

  // ── 蜂群（TASK-08，引擎在另一工作树实现）──────────
  // 只读核对拆分清单：脏树 / 范围重叠 / glob 没匹配到文件。
  // 后端未接通时调用会 reject，闸门进 error 态（graph/swarm.ts 的 swarmGate）。
  checkSwarmPlan: (projectDir: string, items: SwarmCheckItem[], settings: SwarmSettings) =>
    invoke<SwarmPlanCheck>('check_swarm_plan', { projectDir, items, settings }),
  // 按件号顺序 cherry-pick 各 worktree 的改动，每并一件跑一次验收命令。
  // 后端未接通时调用方回退到全 skipped（swarmMergeFallback）。
  mergeSwarmItems: (
    projectDir: string,
    runId: string,
    items: SwarmMergeRequestItem[],
    validateCommand: string,
    settings: SwarmSettings,
  ) =>
    invoke<unknown>('merge_swarm_items', { projectDir, runId, items, validateCommand, settings }),

  // ── 资源接入（TASK-07，镜像 src/resources.rs）────────────────────────
  // 契约类型统一从 resources/types.ts 引入（snake_case，与 Rust 一致）。
  getResources: () => invoke<ResourceStore>('get_resources'),
  detectResourceAgent: (agentId: string, ccSwitchPath?: string) =>
    invoke<ResourceOperation>('detect_resource_agent', {
      agentId,
      ccSwitchPath: ccSwitchPath ?? null,
    }),
  detectAllResources: (ccSwitchPath?: string) =>
    invoke<ResourceOperation>('detect_all_resources', { ccSwitchPath: ccSwitchPath ?? null }),
  checkPiProviderLogin: (provider: string) =>
    invoke<ResourceOperation>('check_pi_provider_login', { provider }),
  addResourceModel: (request: ResourceModelRequest) =>
    invoke<ResourceStore>('add_resource_model', { request }),
  updateResourceModel: (request: ResourceModelRequest) =>
    invoke<ResourceStore>('update_resource_model', { request }),
  removeResourceModel: (request: ResourceModelKey) =>
    invoke<ResourceStore>('remove_resource_model', { request }),
  addManualAgent: (request: AddManualAgentRequest) =>
    invoke<ResourceOperation>('add_manual_agent', { request }),
  smokeResourceModel: (request: ResourceModelKey) =>
    invoke<ResourceStore>('smoke_resource_model', { request }),
  smokeAgentModels: (agentId: string) =>
    invoke<ResourceOperation>('smoke_agent_models', { agentId }),
  inspectDispatchState: (projectDir: string) =>
    invoke<{ exists: boolean; unfinished: boolean; content: string; runId: string; activeNodeIds: string[] }>('inspect_dispatch_state', { projectDir }),
  abandonDispatchState: (projectDir: string) =>
    invoke<void>('abandon_dispatch_state', { projectDir }),
  // ── TASK-06 重派与崩溃恢复的一致性（镜像 src/dispatch.rs，后端另一工作树实现）──
  // 只读对账：按 STATE.md 的 run/active 节点在 .dispatch/ 找同 run/node 的留痕，
  // 判 completed / incomplete / unknown。后端未接通时调用会 reject，
  // 调用方按 exists:false 回退到原有「pending 全部重置为 draft」的行为。
  reconcileDispatchState: (projectDir: string) =>
    invoke<ReconcileSummary>('reconcile_dispatch_state', { projectDir }),
  // 按 .dispatch/<run_id>/<node_id>.before/ 回滚某节点上次派发的改动。
  // 没有快照目录时返回空列表并在 note 说明，不报错。
  // 界面不调用这条命令：回滚由引擎在派发该节点前按请求里的 rollbackBefore 自己
  // 做，这里只按契约留签名（TASK-06 R1-13）。
  rollbackNodeChanges: (projectDir: string, runId: string, nodeId: string) =>
    invoke<RollbackSummary>('rollback_node_changes', { projectDir, runId, nodeId }),
  detectAcpAdapters: () =>
    invoke<AcpAdapterStatus[]>('detect_acp_adapters'),
  releaseAcpPool: () => invoke<void>('release_acp_pool'),
  // 控制台单线聊天走 ACP（09-18）：权限卡片的回答、`/` 补全的命令清单。
  answerAcpPermission: (requestId: string, optionId: string | null) =>
    invoke<void>('answer_acp_permission', { requestId, optionId }),
  closeAcpPermissions: (runId: string) => invoke<void>('close_acp_permissions', { runId }),
  forgetAcpRun: (runId: string) => invoke<void>('forget_acp_run', { runId }),
  acpCommands: (agent: string, cwd: string) =>
    invoke<import('./console-acp').AcpCommand[]>('acp_commands', { agent, cwd }),
};

// ── 对话图契约类型：Graph / GraphNode / Target / Preset / Stage /
//    DispatchableModel 统一从 graph/types.ts 引入（见文件顶部 import）。
//    GraphMeta 只有 Rust 侧 list_graphs 返回用，留在本文件。─────────────
export interface GraphMeta {
  id: string;
  title: string;
  updated_at: string;
}

export interface DispatchSettings {
  timeoutSecs: number;
  concurrency: number;
  maxRequests: number;
  redact: boolean;
  writeProjectDispatch: boolean;
  ccSwitchPath: string;
  validationCommand: string;
  acpEnabled: boolean;
  // TASK-12 契约：修正轮上限（默认 1）。
  fixRoundLimit: number;
}

export interface AcpAdapterStatus {
  id: string;
  label: string;
  installed: boolean;
  command: string;
  detail: string;
}

export interface RoleProfile {
  agent: string;
  provider: string;
  model: string;
  tier: string;
  thinking: string;
}

export interface RoleSettings {
  version: 1;
  planning: RoleProfile;
  execution: RoleProfile;
  acceptance: RoleProfile;
}

export interface DispatchNodeRequest {
  nodeId: string;
  stage: number;
  agent: string;
  provider: string;
  model: string;
  vendor: string;
  billingSource: string;
  role: string;
  tier: string;
  thinking: string;
  visibility: string;
  prompt: string;
  currentMessage: string;
  materials: string[];
  sessionId: string;
  previousRunner: string;
  write: string;
  originalRequest: string;
  upstreamArtifacts: import('./graph/types').ReplyArtifacts[];
  // TASK-05B 主备/熔断（镜像图契约 Target.fallback；R2 追加 vendor：前端已知的真实厂商）
  fallback?: { agent: string; provider: string; model: string; vendor?: string };
  // TASK-06 契约：为真时引擎在派发该节点前先按 before 快照回滚上次改动。
  // 单写者节点重派才带；只读与 worktree 节点不带。
  rollbackBefore?: boolean;
  // TASK-12 契约：为真时引擎在盲目录物化后、派发前跑验证命令，输出落 VALIDATION.txt。
  reviewMaterialValidation?: boolean;
  // TASK-13 契约：修正轮执行节点在验证通过后自动做红绿核对。
  fixRound?: boolean;
  // 控制台单线聊天：claude / codex 也走 ACP，执行类与 MCP 的权限请求交给界面卡片。
  interactive?: boolean;
}

export interface DispatchStageRequest {
  runId: string;
  stage: number;
  projectDir: string;
  nodes: DispatchNodeRequest[];
  settings: DispatchSettings;
}

export interface DispatchPreview {
  totalRedacted: number;
  broadRedacted: number;
  nodes: { nodeId: string; redacted: number; isolation: string }[];
}

export interface DispatchNodeResult {
  nodeId: string;
  stage: number;
  status: string;
  text: string;
  rawStdout: string;
  rawStderr: string;
  modelReported: string;
  sessionId: string;
  tokensIn: number;
  tokensOut: number;
  cacheRead: number;
  durationMs: number;
  exitCode: number | null;
  timedOut: boolean;
  truncated: boolean;
  redacted: number;
  isolation: string;
  runner: 'acp' | 'oneshot' | 'unknown';
  runnerNote?: string;
  workdir: string;
  dispatchFile: string;
  frontmatter: string;
  outputPreview: string;
  error: string;
  artifacts: import('./graph/types').ReplyArtifacts | null;
  validationOutput: string;
  // TASK-13：红绿核对结论与说明；普通节点缺省。
  pinned?: 'pinned' | 'unpinned' | 'skipped';
  pinnedNote?: string;
  // TASK-05B：结果由备选重跑产生时带主/备路由（"agent/model"），否则空串
  fallbackFrom: string;
  fallbackTo: string;
  // TASK-05B R1：可写节点主失败但未切备选时带原因（"可写节点"），空/缺省 = 未跳过
  fallbackSkipped?: string;
  // TASK-14：额度响应里能解析出的恢复时间（ISO 8601）。
  quotaResetsAt?: string | null;
}

// ── 蜂群契约（TASK-08）────────────────────────
export interface SwarmCheckItem {
  item: number;
  files: string[];
}

export interface SwarmOverlap {
  a: number;
  b: number;
  path: string;
}

export interface SwarmUnmatched {
  item: number;
  glob: string;
}

export interface SwarmPlanCheck {
  dirty: boolean;
  overlaps: SwarmOverlap[];
  unmatched: SwarmUnmatched[];
  /** 引擎的说明（超上限未处理、glob 不合法）；旧引擎不发 */
  note?: string;
}

// 两条蜂群命令的 settings：派发设置加上件数上限（引擎只处理前 N 件）。
export interface SwarmSettings extends DispatchSettings {
  swarmLimit: number;
}

export interface SwarmMergeRequestItem {
  nodeId: string;
  item: number;
  worktree: string;
  /** 该件声明的文件范围（TASK-08D：合并只取范围内的 diff） */
  files: string[];
  /** 合并提交的标题：`aicross swarm item N: <title>` */
  title?: string;
  accept: string;
}

export type SwarmMergeOutcome = 'merged' | 'conflict' | 'test_failed' | 'skipped';

// 引擎按 camelCase 返回（src/dispatch.rs 的 SwarmMergeResult 是 serde
// rename_all camelCase）。旧引擎的 node_id / test_output 写法由
// normalizeSwarmMergeResults（graph/swarm.ts）在进界面前统一过一道。
export interface SwarmMergeResult {
  nodeId: string;
  item: number;
  result: SwarmMergeOutcome;
  commit: string;
  note: string;
  testOutput: string;
}

export interface DispatchStageResult {
  runId: string;
  results: DispatchNodeResult[];
  stateFile: string;
}

// TASK-07 契约：`dispatch-progress` 事件的载荷。status 除既有的中文状态词
// （排队 / 等待写锁 / 运行中 / 终态）外新增 "running"——节点跑着时每 15s 一条
// 的心跳，节点结束时最后一条仍是原有终态。后面五个字段是心跳才带的，
// 旧引擎不发；前端按 0 / 空串处理（graph/dispatch.ts 的 progressSnapshot）。
export interface DispatchProgress {
  runId: string;
  nodeId: string;
  status: string;
  /** 节点开跑到这条心跳的毫秒数 */
  elapsedMs?: number;
  /** 距上一条输出/通知的毫秒数 */
  idleMs?: number;
  /** 本轮工具调用次数，ACP 才有，一次性节点为 0 */
  toolCalls?: number;
  /** 连续同名工具调用次数 */
  repeatedToolCalls?: number;
  /** 最后一次工具调用的标题 */
  lastTool?: string;
}

export interface ConsoleRunNode {
  node_id: string;
  role: string;
  agent: string;
  model: string;
  status: string;
  text_excerpt: string;
  artifacts_dir: string;
  quota_resets_at?: string | null;
}

export interface ConsoleProposal {
  proposal_id: string;
  project: string;
  graph_id: string;
  preset_id: string;
  node_count: number;
  reason: string;
  created_at: string;
}

export interface ConsoleRun {
  run_id: string;
  graph_id: string;
  project: string;
  status: 'proposed' | 'running' | 'done' | 'failed' | 'rejected' | 'dismissed';
  nodes: ConsoleRunNode[];
  leftover?: string[];
  started_at: string;
  reason?: string;
  in_flight_node_ids?: string[];
}

export interface ConsoleRunUpdate {
  run_id: string;
  graph_id: string;
  status: ConsoleRun['status'] | 'skipped';
  nodes: ConsoleRunNode[];
  leftover: string[];
  in_flight_node_ids: string[];
  reason: string;
}

// ── TASK-37 查看器（镜像 src/run_view.rs 的 RunView / RunViewNode，
//    camelCase）──
export interface RunViewArtifacts {
  worktree: string;
  diff: string;
  changedFileCount: number;
}

export interface RunViewNode {
  nodeId: string;
  stage: number;
  role: string;
  agent: string;
  provider: string;
  /** 请求的模型 id。 */
  model: string;
  /** 图上 reply 的状态词：draft / pending / done / failed / quota_exhausted。 */
  status: string;
  errorKind: string;
  adopted: boolean;
  hasResult: boolean;
  text: string;
  skippedReason: string;
  /** 实际回答的模型；空串表示未报告。 */
  modelReported: string;
  tokensIn: number | null;
  tokensOut: number | null;
  tokensCache: number | null;
  ms: number | null;
  fallbackFrom: string;
  fallbackTo: string;
  quotaResetsAt: string;
  /** 蜂群件标题（节点正文首行）；非蜂群件为空串。 */
  swarmTitle: string;
  artifacts: RunViewArtifacts | null;
}

export interface RunView {
  runId: string;
  graphId: string;
  project: string;
  /** 推断出的模式名；派发前被拒、没建节点时为空串。 */
  presetName: string;
  graphTitle: string;
  status: 'running' | 'done' | 'failed' | 'rejected';
  reason: string;
  startedAt: string;
  updatedAt: string;
  leftover: string[];
  /** 按 run 摘要里的节点顺序。 */
  nodes: RunViewNode[];
}

export interface ConsoleDispatchEvent {
  run_id: string;
  project: string;
  text: string;
  preset?: string;
  targets?: import('./graph/types').Target[];
  attachments?: string[];
  reason?: string;
}

export interface ConsoleProposalEvent extends Omit<ConsoleDispatchEvent, 'run_id'> {
  proposal_id: string;
}

export interface ConsoleActionEvent {
  request_id: string;
  action: 'adopt' | 'cancel';
  run_id: string;
  node_id?: string;
}

export interface ConsoleRunsEvent {
  kind: string;
  graph_id: string;
  node_ids: string[];
  runs: ConsoleRun[];
}

export async function onConsoleRunsChanged(
  cb: (event: ConsoleRunsEvent) => void,
): Promise<() => void> {
  const { listen } = await import('@tauri-apps/api/event');
  return listen<ConsoleRunsEvent>('console-runs-changed', (event) => cb(event.payload));
}

export async function onConsoleDispatch(cb: (event: ConsoleDispatchEvent) => void): Promise<() => void> {
  const { listen } = await import('@tauri-apps/api/event');
  return listen<ConsoleDispatchEvent>('aicross:console-dispatch', (event) => cb(event.payload));
}

export async function onConsolePropose(cb: (event: ConsoleProposalEvent) => void): Promise<() => void> {
  const { listen } = await import('@tauri-apps/api/event');
  return listen<ConsoleProposalEvent>('aicross:console-propose', (event) => cb(event.payload));
}

export async function onConsoleAction(cb: (event: ConsoleActionEvent) => void): Promise<() => void> {
  const { listen } = await import('@tauri-apps/api/event');
  return listen<ConsoleActionEvent>('aicross:console-action', (event) => cb(event.payload));
}

// Codex-compatible plugin marketplace (a cloned git repo) + its plugins.
export interface MarketplacePlugin {
  key: string;
  name: string;
  displayName: string;
  description: string;
  /** Absolute path to the icon file; load via convertFileSrc (asset proto). */
  iconPath: string | null;
  path: string;
  enabled: boolean;
}
export interface Marketplace {
  id: string;
  displayName: string;
  manifestPath: string;
  plugins: MarketplacePlugin[];
}

// In-app self-update progress, emitted by download_and_install_update.
export interface SelfUpdateProgress {
  status: 'speed_test' | 'downloading' | 'launching' | 'error';
  percent: number;
}

// Subscribe to self-update progress while downloadAndInstallUpdate runs.
// Returns an unlisten fn. Dynamic-imports the event API (matches how the
// rest of the app subscribes to Tauri events).
export async function onSelfUpdateProgress(
  cb: (p: SelfUpdateProgress) => void,
): Promise<() => void> {
  const { listen } = await import('@tauri-apps/api/event');
  return listen<SelfUpdateProgress>('self-update-progress', (e) => cb(e.payload));
}

/**
 * One entry in `~/.aicross/tools.json`. All fields are optional —
 * empty strings / empty arrays fall through to Coffee CLI's built-in
 * defaults for that tool. Lets users say things like "always launch claude
 * with --dangerously-skip-permissions" or "run codex through
 * `docker exec mybox`" without us having to auto-detect every
 * conceivable install path.
 */
export interface ToolConfigEntry {
  /** Full launch command. Whitespace-split — first token is the binary,
   *  the rest are prepended to args. Empty falls through to default. */
  command: string;
  /** Args appended AFTER the built-in args (so tool-managed flags like
   *  --mcp-config / --append-system-prompt still come first). */
  extra_args: string[];
  /** Pre-fills the cwd selector when starting a new tab. Empty falls
   *  through to the launchpad's last-used cwd. */
  default_cwd: string;
  /** Custom directory to scan for this tool's session history files.
   *  Empty falls through to the built-in scan path. */
  history_path: string;
}
