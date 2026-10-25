// 窗口命令的类型化封装：各页只通过这里调 Rust 侧命令。

type TauriInvoke = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

/** Tauri 在页面脚本运行前注入 `__TAURI_INTERNALS__`；浏览器里单开前端时没有。 */
function tauriInvoke(): TauriInvoke | null {
  if (typeof window === 'undefined') return null;
  const internals = (window as { __TAURI_INTERNALS__?: { invoke?: TauriInvoke } }).__TAURI_INTERNALS__;
  return typeof internals?.invoke === 'function' ? internals.invoke : null;
}

export const isTauri = tauriInvoke() !== null;

export function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const call = tauriInvoke();
  if (!call) return Promise.reject(new Error('不在 aicross 窗口中运行'));
  return call(cmd, args) as Promise<T>;
}

export type WindowAction = 'minimize' | 'toggle-maximize' | 'close';

export interface AppUpdateInfo {
  version: string;
  current: string;
  notes: string;
}

export const commands = {
  windowAction: (action: WindowAction) => invoke<void>('window_action', { action }),
  appUpdateCheck: () => invoke<AppUpdateInfo | null>('app_update_check'),
  appUpdateInstall: () => invoke<void>('app_update_install'),

  // TASK-37 查看器：冷启动时 `aicross open <run_id>` 随 argv 带进来的 run id，
  // 挂载后取一次；热启动走 'open-run-request' 事件。
  takePendingOpen: () => invoke<string | null>('take_pending_open'),
  loadRunView: (runId: string) => invoke<RunView>('load_run_view', { runId }),
  adoptRunNode: (runId: string, nodeId: string) =>
    invoke<RunView>('adopt_run_node', { runId, nodeId }),

  // TASK-40「派发记录」：最近 50 条，按更新时间倒序。
  listRuns: () => invoke<RunListItem[]>('list_runs'),

  // TASK-47 资源页与模式页（契约见 tasks/TASK-46-modes-config.md 第 4 节）；
  // TASK-51 起配置按主 agent 分开，读写配置的命令都带 host（契约见
  // tasks/TASK-50-host-skill-recommend.md 第 5 节）。
  // 返回值与对应命令行的 stdout JSON 同一结构；失败时 reject 的是 stderr 那行说明。
  /** host 为 null 时由引擎按 --host 缺省规则取；没有任何主 agent 配置时返回资源与
   *  空配置（资源页在还没有主 agent 时用）。 */
  configSnapshot: (host: string | null) => invoke<ConfigSnapshot>('config_snapshot', { host }),
  resourcesDetect: () => invoke<ResourcesOutput>('resources_detect'),
  /** 只给 agent 时冒烟该 agent 全部第三方模型（输出 { agents }）；给了 model
   *  时输出该 agent 那一项。 */
  resourcesSmoke: (agent: string, model?: string, provider?: string) =>
    invoke<ResourcesOutput | ResourceAgentEntry>('resources_smoke', {
      agent,
      model: model ?? null,
      provider: provider ?? null,
    }),
  configSetDefaultMode: (host: string, mode: string) =>
    invoke<DefaultModeResult>('config_set_default_mode', { host, mode }),
  /** stage、index 从 1 开始。写入的人选由 Rust 侧记为 origin: manual。 */
  configSetSeat: (host: string, mode: string, stage: number, index: number, seat: Seat) =>
    invoke<ModeEntry>('config_set_seat', { host, mode, stage, index, seat }),
  /** 不给 stage、index 时清掉该模式全部人选。 */
  configClearSeat: (host: string, mode: string, stage?: number, index?: number) =>
    invoke<ModeEntry>('config_clear_seat', { host, mode, stage: stage ?? null, index: index ?? null }),
  /** 设定评审模式某个位置的视角（TASK-59）；focus 为空串恢复默认，index 从 1 起。 */
  configSetFocus: (host: string, mode: string, index: number, focus: string) =>
    invoke<ModeEntry>('config_set_focus', { host, mode, index, focus }),
  /** 设定可调模式的位置数量（TASK-56）；返回该模式条目。 */
  configSetCount: (host: string, mode: string, count: number) =>
    invoke<ModeEntry>('config_set_count', { host, mode, count }),
  /** 一键推荐：all 为 false 只填空位置与 origin 为 recommended 的位置；为 true 全部重填。 */
  configRecommend: (host: string, mode: string | null, all: boolean) =>
    invoke<RecommendResult>('config_recommend', { host, mode, all }),

  // TASK-51 主 agent 与 skill 安装。
  hostsList: () => invoke<HostEntry[]>('hosts_list'),
  /** vendor 为 'auto' 时取消用户设定，回到自动推定。 */
  hostSetVendor: (host: string, vendor: string) => invoke<unknown>('host_set_vendor', { host, vendor }),
  skillStatus: (project?: string) => invoke<SkillStatus>('skill_status', { project: project ?? null }),
  skillInstall: (host: string, scope: SkillScope, project?: string) =>
    invoke<SkillHostStatus>('skill_install', { host, scope, project: project ?? null }),
  /** 把一份旧的外部安装换成内置版本；原文件备份，之后由 aicross 管理。 */
  skillUpgrade: (host: string, path: string) => invoke<unknown>('skill_upgrade', { host, path }),
  skillUninstall: (host: string, scope: SkillScope, project?: string) =>
    invoke<unknown>('skill_uninstall', { host, scope, project: project ?? null }),
  /** 从资源库删除最近一次探测未发现的模型；给了 agent 只清这一家（TASK-52 第 1 节）。 */
  resourcesPrune: (agent: string | null) => invoke<PruneResult>('resources_prune', { agent }),
  // TASK-53 斜杠命令（契约见 tasks/TASK-53-slash-commands.md 第 3 节）。
  slashList: (host: string) => invoke<SlashList>('slash_list', { host }),
  /** 同名已存在则更新。 */
  slashAdd: (host: string, name: string, mode: string, note: string | null) =>
    invoke<SlashList>('slash_add', { host, name, mode, note }),
  slashRemove: (host: string, name: string) => invoke<SlashList>('slash_remove', { host, name }),
  // TASK-55 自定义模式（契约见 tasks/TASK-55-custom-modes.md 第 2 节）。
  /** 没有 id 时新建，有 id 时更新同 id 的自定义模式；host 为当前主 agent。 */
  presetSave: (host: string, preset: PresetPayload) => invoke<PresetSaveResult>('preset_save', { host, preset }),
  /** 删除自定义模式，并清理各主 agent 下的人选、默认模式与斜杠命令；不分主 agent，不带 host。 */
  presetRemove: (id: string) => invoke<unknown>('preset_remove', { id }),
  /** 给每个还没有命令的内置模式生成 <prefix><短名>（TASK-54 第 3 节）；prefix 缺省 ax-。 */
  slashAddAll: (host: string, prefix: string | null) =>
    invoke<SlashAddAllResult>('slash_add_all', { host, prefix }),
  /** 系统选目录对话框；取消时为 null。 */
  pickFolder: () => invoke<string | null>('pick_folder'),
};

// ── TASK-55 自定义模式（镜像 `aicross preset save` 的输入与输出）──
export interface PresetPayload {
  /** 没有时新建。 */
  id?: string;
  name: string;
  description?: string;
  stages: { nodes: { role: string; visibility: string; write: string; thinking?: string }[] }[];
}

export interface PresetSaveResult {
  preset: unknown;
  modes: ModeEntry[];
}

// ── TASK-53 斜杠命令（镜像 `aicross slash list`）──
export interface SlashCommand {
  name: string;
  /** 预设 id。 */
  mode: string;
  modeName: string;
  note: string;
  path: string;
  /** 已写入 / 未写入 / 被占用。 */
  state: string;
  /** 该主 agent 下的输入写法，如 /prompts:cross-review。 */
  trigger: string;
  /** 文件内容旧于当前版本，需要重写（TASK-59）；旧版引擎没有。 */
  stale?: boolean;
}

/** 每个内置模式一行（TASK-54 第 2 节）。 */
export interface SlashModeEntry {
  mode: string;
  modeName: string;
  /** 模式介绍（TASK-55）；旧版引擎没有。 */
  description?: string;
  /** 带参数的调用写法，如 /ai-cross 盲审。 */
  argForm: string;
  /** 建议的短命令名，如 ax-blind。 */
  suggestedName: string;
  /** commands 里该模式的第一条；没有为 null。 */
  command: SlashCommand | null;
  /** 还没登记、但对应命令文件已存在（TASK-57）；旧版引擎没有。 */
  existing?: { name: string; state: string; path: string } | null;
}

export interface SlashList {
  host: string;
  /** 默认调用写法，如 /ai-cross、$ai-cross。 */
  invoke: string;
  commands: SlashCommand[];
  /** 旧版引擎没有这个字段。 */
  modes?: SlashModeEntry[];
}

export interface SlashAddAllResult extends SlashList {
  added?: string[];
  skipped?: { name: string; reason: string }[];
}

// ── TASK-52 清理未发现（镜像 `aicross resources prune`）──
export interface PruneResult extends ResourcesOutput {
  /** 被删的 agent/provider/model。 */
  removed: string[];
  /** 被删的模型里仍被人选或角色引用的那些；界面只用条数。 */
  referenced: unknown[];
}

// ── TASK-51 主 agent（镜像 `aicross hosts`）──
export interface HostEntry {
  id: string;
  name: string;
  /** 实际厂商；推定不出时为「未知」。 */
  vendor: string;
  /** 推定 / 端点 / 用户设定 / 未知。 */
  vendorSource: string;
  /** 推定依据，一行。 */
  vendorBasis: string;
}

// ── TASK-51 skill 安装（镜像 `aicross skill status`）──
export type SkillScope = 'global' | 'project';

export interface SkillLocation {
  scope: SkillScope;
  path: string;
  /** none：不存在；ours：我们装的；external：链接或没有我们的标记文件。 */
  kind: 'none' | 'ours' | 'external';
  /** SKILL.md 的版本号；读不出为空串。 */
  version: string;
  /** external 且是链接时的指向目标；其余为空串或 null。 */
  linkTarget?: string | null;
  readByThisHost?: boolean;
  /** 我们安装时写入的位置（fix-50-r1）；旧版引擎没有这个字段。 */
  writeTarget?: boolean;
  /** 外部安装且版本低于内置（TASK-57）；旧版引擎没有。 */
  outdated?: boolean;
  /** 外部位置的管理软件：skills.sh / skills-manager / 空串（TASK-59）；旧版引擎没有。 */
  manager?: string;
}

export interface SkillHostStatus {
  host: string;
  locations: SkillLocation[];
  duplicate: boolean;
  updateAvailable: boolean;
}

export interface SkillStatus {
  bundledVersion: string;
  hosts: SkillHostStatus[];
}

// ── TASK-51 一键推荐（镜像 `aicross config recommend`）──
export interface RecommendResult {
  defaultMode: string;
  modes: ModeEntry[];
  /** 凑不出人选的位置等说明；按条列出。 */
  notes: string[] | string;
  /** 各 agent 下可用但未验证、没进推荐候选池的第三方模型数（TASK-57）；旧版引擎没有。 */
  unverified?: { agent: string; count: number }[];
}

/** 资源探测进度事件 resources-detect-progress 的载荷（TASK-57）：每个 agent 探测完发一次。 */
export interface DetectProgress {
  agent: string;
  done: number;
  total: number;
}

// ── TASK-47 资源（镜像 `aicross resources` 的输出，snake_case 原样）──
export interface ResourceModelEntry {
  id: string;
  agent: string;
  /** 原生模型为空串。 */
  provider: string;
  /** 实际模型；与 id 相同时界面不重复显示。 */
  resolved_id: string;
  /** 状态词：未探测 / 可用 / 已验证 / 未验证 / 验证失败 / 冒烟超时 / 未登录 / 额度用尽 / 未安装 /
   *  未发现（最近一次探测没发现，TASK-48）。 */
  state: string;
  note: string;
  /** 最近一次探测未发现的标记（不看 agent 是否可用）；旧版引擎没有这个字段。 */
  missing?: boolean;
}

export interface ResourceAgentEntry {
  id: string;
  name: string;
  version: string;
  /** 可用 / 未安装。 */
  state: string;
  note: string;
  models: ResourceModelEntry[];
  /** 命令模板还没在真机上跑通（TASK-61）；旧引擎缺这个字段时按 false。 */
  untested?: boolean;
  /** 官网地址（TASK-61）；旧引擎缺这个字段时按空串。 */
  homepage?: string;
}

export interface ResourcesOutput {
  agents: ResourceAgentEntry[];
  /** 上次探测时间（RFC3339）；从未探测为 null，旧数据缺这个字段时同样视为尚未探测（TASK-48）。 */
  detected_at?: string | null;
}

// ── TASK-47 模式（镜像 `aicross config show`；人选对象同 modes.json，camelCase）──
export interface SeatFallback {
  agent: string;
  provider: string | null;
  model: string;
}

export interface Seat {
  agent: string;
  provider: string | null;
  model: string;
  /** 空串表示沿用预设。 */
  tier: string;
  /** 关 / 低 / 中 / 高；空串表示沿用预设。 */
  thinking: string;
  fallback: SeatFallback | null;
  /** 人选来源（TASK-50）：manual 手选 / recommended 一键推荐填的；缺省视为 manual。 */
  origin?: 'manual' | 'recommended';
}

/** 派发时实际用的人选，附该人选在资源库里的状态（TASK-48）：资源状态词，
 *  资源库里没有这条模型时为「不在资源库」；旧数据缺这个字段时视为正常。 */
export interface EffectiveSeat extends Seat {
  state?: string;
  /** 人选或备选违反位置规则时的原因（如只能盲审的 agent 放在共享位置）；合规为 null，旧版引擎没有。 */
  invalid?: string | null;
}

/** 人选来源：预设写定 / 本模式指定 / 按角色默认 / 未指定。 */
export type SeatSource = '预设' | '位置' | '角色' | '未指定';

export interface ModeSeatNode {
  role: string;
  /** 盲 / 共享 / 指定上游。 */
  visibility: string;
  /** 只读 / 单写者 / worktree。 */
  write: string;
  /** 评审模式各位置的视角（生效值）：默认依次为安全 / 性能 / 需求覆盖，其余为空串（TASK-57）；
   *  旧版引擎没有。 */
  focus?: string;
  /** 评审模式位置可选的视角（TASK-59）；非评审模式或旧版引擎没有。 */
  focusOptions?: string[];
  /** 预设本身给这个位置定的思考强度：关 / 低 / 中 / 高 / 空串（TASK-55）；旧版引擎没有。 */
  presetThinking?: string;
  /** modes.json 里的原样；没设为 null。 */
  seat: Seat | null;
  /** 派发时实际用的人选；未指定时为 null。 */
  effective: EffectiveSeat | null;
  source: SeatSource;
}

/** 模式里可调数量的那个角色（TASK-56 第 2 节）。 */
export interface ModeCount {
  role: string;
  value: number;
  min: number;
  max: number;
  default: number;
}

export interface ModeEntry {
  id: string;
  name: string;
  /** 模式介绍（TASK-55）；旧版引擎没有，空串不显示。 */
  description?: string;
  /** 可调的位置数量（TASK-56）；不可调的模式为 null，旧版引擎没有。 */
  count?: ModeCount | null;
  builtin: boolean;
  default: boolean;
  /** 阶段数组 → 位置数组。 */
  stages: ModeSeatNode[][];
}

export interface ConfigShow {
  roles: unknown;
  dispatch: unknown;
  acp: unknown;
  /** 实际生效的默认模式 id。 */
  defaultMode: string;
  modes: ModeEntry[];
}

export interface ConfigSnapshot {
  resources: ResourcesOutput;
  config: ConfigShow;
}

export interface DefaultModeResult {
  defaultMode: string;
  modes: ModeEntry[];
}

// ── TASK-40 首页列表（镜像 src/run_view.rs 的 RunListItem，camelCase）──
export interface RunListItem {
  runId: string;
  /** 模式名：内置预设用自己的名字，用户自建为「自定义模式 · 名称」。 */
  presetName: string;
  /** running / done / failed / rejected。 */
  status: string;
  startedAt: string;
  /** 本次派发的任务名（原始需求第一行）。 */
  taskTitle: string;
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
  /** 本次派发的任务名（原始需求第一行）。 */
  taskTitle: string;
  status: 'running' | 'done' | 'failed' | 'rejected';
  reason: string;
  startedAt: string;
  updatedAt: string;
  leftover: string[];
  /** 按 run 摘要里的节点顺序。 */
  nodes: RunViewNode[];
}
