// Tauri v2 typed invoke wrapper — TASK-40 下线其余界面后的最小形态。
// 窗口只剩标题栏控制、首页「最近的派发」与查看页，这里只保留它们的命令。

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

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!_invoke) throw new Error('Tauri IPC not available');
  return _invoke(cmd, args) as Promise<T>;
}

// ─── Typed Commands ──────────────────────────────────────────────────────────

export const commands = {
  // Window decorators（自绘标题栏）
  windowMinimize: () => invoke<void>('window_minimize'),
  windowMaximize: () => invoke<void>('window_maximize'),
  windowClose: () => invoke<void>('window_close'),

  // TASK-37 查看器：冷启动时 `aicross open <run_id>` 随 argv 带进来的 run id，
  // 挂载后取一次；热启动走 'open-run-request' 事件。
  takePendingOpen: () => invoke<string | null>('take_pending_open'),
  loadRunView: (runId: string) => invoke<RunView>('load_run_view', { runId }),
  adoptRunNode: (runId: string, nodeId: string) =>
    invoke<RunView>('adopt_run_node', { runId, nodeId }),

  // TASK-40 首页「最近的派发」：最近 50 条，按更新时间倒序。
  listRuns: () => invoke<RunListItem[]>('list_runs'),
};

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
