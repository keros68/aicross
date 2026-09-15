// 蜂群（TASK-08 B 节）：拆分清单解析、按清单生成执行节点、G1 重叠闸门、
// G3 合并结果——全是纯函数，界面只负责调命令和渲染。
// 命令形状住在 tauri.ts，这里 type-only 引用（运行时擦除，不构成值依赖环）。

import type { Graph, GraphNode, Preset, Target, Thinking } from './types.ts';
import { newNodeId } from './types.ts';
import { graphStage, setReplyAdopted } from './dispatch.ts';
import type { UiDispatchSettings } from './dispatch-settings.ts';
import type {
  SwarmCheckItem,
  SwarmMergeRequestItem,
  SwarmMergeResult,
  SwarmPlanCheck,
  SwarmSettings,
} from '../tauri';
import splitPrompt from '../../../fixtures/prompts/split.txt?raw';

// 与并发上限同一处设置（dispatch-settings.ts 的 swarmLimit），默认 4。
export const SWARM_LIMIT_DEFAULT = 4;

// 拆分提示词只有一份来源：冻结文件 fixtures/prompts/split.txt。引擎把它放在
// 内置预设的 Target.preset_text 上，盖章时直接写进节点消息（stamp.ts）；这里
// 只给「引擎清单里没有 builtin-swarm」的兜底预设用同一份文件，不另写一份文本。
export const SWARM_SPLIT_PROMPT = splitPrompt.trim();

// split.txt 写的是「不超过 N 件」，N 是本机设置里的上限：盖章时在模板后补这一行。
export function swarmLimitLine(limit: number = SWARM_LIMIT_DEFAULT): string {
  return `本次上限 ${Math.max(1, limit)} 件。`;
}

// 两条蜂群命令都要 settings（引擎按 swarmLimit 只处理前 N 件、按 timeoutSecs
// 给验收命令设超时、按 writeProjectDispatch 决定留痕落在哪）。界面设置多出来的
// classificationModel 不发。
export function swarmWireSettings(settings: UiDispatchSettings): SwarmSettings {
  return {
    timeoutSecs: settings.timeoutSecs,
    concurrency: settings.concurrency,
    maxRequests: settings.maxRequests,
    redact: settings.redact,
    writeProjectDispatch: settings.writeProjectDispatch,
    ccSwitchPath: settings.ccSwitchPath,
    validationCommand: settings.validationCommand,
    acpEnabled: settings.acpEnabled,
    swarmLimit: settings.swarmLimit,
    fixRoundLimit: settings.fixRoundLimit,
  };
}

export interface SwarmPlanItem {
  title: string;
  files: string[];
  accept: string;
}

export interface SwarmPlan {
  items: SwarmPlanItem[];
  /** 超出上限被截掉的件数；>0 时界面出一句提示 */
  dropped: number;
}

// 内置预设「蜂群模式」：盖在根材料上长出一个拆分节点（只读、共享，目标按
// 「出方案」角色默认填，见 dispatch.ts 的 applyRoleDefaults）。引擎侧
// builtin_presets() 补上同 id 的预设后，withSwarmPreset 自动让位给后端那份。
export const BUILTIN_SWARM_PRESET: Preset = {
  id: 'builtin-swarm',
  name: '蜂群模式',
  builtin: true,
  stages: [
    {
      nodes: [
        {
          agent: '',
          model: '',
          role: '拆分',
          thinking: '中',
          visibility: '共享',
          write: '只读',
          preset_text: SWARM_SPLIT_PROMPT,
        },
      ],
    },
  ],
};

export function withSwarmPreset(presets: Preset[]): Preset[] {
  return presets.some((preset) => preset.id === BUILTIN_SWARM_PRESET.id)
    ? presets
    : [...presets, BUILTIN_SWARM_PRESET];
}

// ── 1. 清单解析 ────────────────────────────────────────────────────────────

export const SWARM_PARSE_FAILED_TEXT = '未解析到清单';

const JSON_FENCE = /```json[^\n]*\n([\s\S]*?)```/g;

function cleanStrings(value: unknown): string[] | null {
  if (!Array.isArray(value)) return null;
  const out: string[] = [];
  for (const entry of value) {
    if (typeof entry !== 'string') return null;
    const text = entry.trim();
    if (text !== '') out.push(text);
  }
  return out.length > 0 ? out : null;
}

// 取正文里最后一个 ```json 围栏，校验字段；任何一件不合格 → null（按普通回复展示）。
// 超过 limit 件时截断，dropped 记截掉几件。
export function parseSwarmPlan(text: string, limit = SWARM_LIMIT_DEFAULT): SwarmPlan | null {
  const blocks = [...text.matchAll(JSON_FENCE)].map((match) => match[1]);
  if (blocks.length === 0) return null;
  let parsed: unknown;
  try {
    parsed = JSON.parse(blocks[blocks.length - 1]);
  } catch {
    return null;
  }
  const raw = (parsed as { items?: unknown } | null)?.items;
  if (!Array.isArray(raw) || raw.length === 0) return null;
  const items: SwarmPlanItem[] = [];
  for (const entry of raw) {
    if (typeof entry !== 'object' || entry === null) return null;
    const row = entry as { title?: unknown; files?: unknown; accept?: unknown };
    if (typeof row.title !== 'string' || row.title.trim() === '') return null;
    const files = cleanStrings(row.files);
    if (!files) return null;
    if (row.accept !== undefined && typeof row.accept !== 'string') return null;
    items.push({ title: row.title.trim(), files, accept: (row.accept ?? '').toString().trim() });
  }
  const capped = Math.max(1, limit);
  return { items: items.slice(0, capped), dropped: Math.max(0, items.length - capped) };
}

// 上限直接传进来（R1-7）：不再拿截断后的件数反推，设置改成 2 时这句话也对。
export function swarmPlanNotice(plan: SwarmPlan, limit = SWARM_LIMIT_DEFAULT): string | null {
  if (plan.dropped <= 0) return null;
  const total = plan.items.length + plan.dropped;
  return `清单 ${total} 件，超过上限 ${Math.max(1, limit)} 件，只取前 ${plan.items.length} 件`;
}

// ── 2. 按清单生成执行节点 ──────────────────────────────────────────────────

const THINKINGS: Thinking[] = ['关', '低', '中', '高'];

function asThinking(value: string | undefined): Thinking {
  return THINKINGS.includes((value ?? '') as Thinking) ? (value as Thinking) : '中';
}

// 节点消息只写本件（R1-8）：整份清单不进任何一件的 prompt。
export function swarmNodeText(item: SwarmPlanItem): string {
  const lines = [item.title, `文件范围：${item.files.join('、')}`];
  if (item.accept !== '') lines.push(`验收命令：${item.accept}`);
  return lines.join('\n');
}

export interface SwarmExecutionProfile {
  agent: string;
  provider?: string;
  model: string;
  tier?: string;
  thinking?: string;
}

export interface SwarmAppendOutcome {
  graph: Graph;
  nodeIds: string[];
  /** 已经生成过、这次没重复建的件号（R1-2 幂等） */
  skipped: number[];
  /** 因件数上限没生成的件数 */
  capped: number;
}

// 某个拆分节点底下已经生成过的件号。
export function swarmChildItems(graph: Graph, splitNodeId: string): number[] {
  return Object.values(graph.nodes)
    .filter((node) => node.parents.includes(splitNodeId) && !!node.swarm)
    .map((node) => node.swarm!.item)
    .sort((a, b) => a - b);
}

// 件号续号（08C-3）：同图内第二个拆分节点的件从别处已用掉的最大件号 +1 起编，
// 否则两批都从 1 起，G1 会说「第 1 件与第 1 件都会改…」、合并按件号排序两批交错。
// 这个拆分节点自己已经生成过件时锚在它原来的起点上，重复点「生成」仍然幂等。
export function swarmItemBase(graph: Graph, splitNodeId: string): number {
  const own = swarmChildItems(graph, splitNodeId);
  if (own.length > 0) return own[0] - 1;
  let max = 0;
  for (const node of Object.values(graph.nodes)) {
    if (!node.swarm || node.parents.includes(splitNodeId)) continue;
    max = Math.max(max, node.swarm.item);
  }
  return max;
}

// 每件一个 draft 执行节点：parents = 拆分节点，write=worktree（各自 worktree），
// 件号决定后面的合并顺序，从 swarmItemBase +1 起编（08C-3 跨拆分节点续号）。
// 可见性是「指定上游 = 根材料」而不是共享——
// 共享会把拆分回复的整份清单带进每件的上下文（R1-8）。
// 同件号已经有节点时不重复建（R1-2），总件数不超过 limit。
export function appendSwarmNodes(
  graph: Graph,
  splitNodeId: string,
  items: SwarmPlanItem[],
  execution?: SwarmExecutionProfile,
  limit = SWARM_LIMIT_DEFAULT,
): SwarmAppendOutcome {
  const unchanged: SwarmAppendOutcome = { graph, nodeIds: [], skipped: [], capped: 0 };
  if (!graph.nodes[splitNodeId] || items.length === 0) return unchanged;
  const allowed = Math.max(1, limit);
  const existing = new Set(swarmChildItems(graph, splitNodeId));
  const base = swarmItemBase(graph, splitNodeId);
  const nodes = { ...graph.nodes };
  const nodeIds: string[] = [];
  const skipped: number[] = [];
  let capped = 0;
  const nowMs = Date.now();
  items.forEach((item, index) => {
    const number = base + index + 1;
    if (existing.has(number)) {
      skipped.push(number);
      return;
    }
    if (existing.size + nodeIds.length >= allowed) {
      capped += 1;
      return;
    }
    const id = newNodeId();
    const target: Target = {
      agent: execution?.agent ?? '',
      provider: execution?.provider ? execution.provider : undefined,
      model: execution?.model ?? '',
      role: '执行',
      tier: execution?.tier ? execution.tier : undefined,
      thinking: asThinking(execution?.thinking),
      visibility: '指定上游',
      upstreams: [graph.root],
      write: 'worktree',
    };
    const node: GraphNode = {
      id,
      kind: 'turn',
      parents: [splitNodeId],
      user: { text: swarmNodeText(item), attachments: [] },
      reply: { status: 'draft', text: '' },
      target,
      swarm: { item: number, files: [...item.files], accept: item.accept },
      created_at: new Date(nowMs + index).toISOString(),
    };
    nodes[id] = node;
    nodeIds.push(id);
  });
  if (nodeIds.length === 0) return { ...unchanged, skipped, capped };
  return {
    graph: { ...graph, nodes, updated_at: new Date(nowMs).toISOString() },
    nodeIds,
    skipped,
    capped,
  };
}

// 「按清单生成执行节点」按钮下面的那一行反馈。
export function swarmGenerateNotice(outcome: SwarmAppendOutcome, limit = SWARM_LIMIT_DEFAULT): string {
  const parts: string[] = [];
  if (outcome.nodeIds.length > 0) parts.push(`已生成 ${outcome.nodeIds.length} 个执行节点`);
  if (outcome.skipped.length > 0) parts.push(`第 ${outcome.skipped.join('、')} 件已生成，未重复建`);
  if (outcome.capped > 0) parts.push(`件数上限 ${Math.max(1, limit)} 件，${outcome.capped} 件未生成`);
  return parts.join('；') || '未生成节点';
}

// ── 3. G1 闸门 ─────────────────────────────────────────────────────────────

export const SWARM_DIRTY_TEXT = '项目目录有未提交改动，蜂群合并要求干净';
export const SWARM_CHECKING_TEXT = '正在核对文件范围…';
export const SWARM_CHECK_FAILED_TEXT = '无法核对文件范围';
export const SWARM_PROCEED_TEXT = '无法核对文件范围，已选择继续派发';
export const SWARM_NO_PROJECT_TEXT = '未选工作目录，无法核对文件范围';

// 核对状态（R1-1）：没要核对 / 没有工作目录 / 在核对 / 核对回来了 / 核对不了。
// 「还没查」与「查不了」以前都是 null，一律放行，窗口期内闸门形同虚设。
export type SwarmCheckState =
  | { phase: 'idle' }
  | { phase: 'noProject' }
  | { phase: 'pending' }
  | { phase: 'ok'; check: SwarmPlanCheck }
  | { phase: 'error'; message: string };

// 闸门读的状态由「本批有几件 + 有没有工作目录 + 命令状态」推出来（08C-4）：
// 没有蜂群件不拦；有蜂群件但没选工作目录时命令发不出去，直接阻断并说清原因；
// 其余看命令状态。命令状态的初值是 pending，确认屏第一帧（effect 还没跑）也不
// 留可点窗口。
export function swarmCheckInput(
  swarmCount: number,
  projectDir: string,
  state: SwarmCheckState,
): SwarmCheckState {
  if (swarmCount === 0) return { phase: 'idle' };
  if (projectDir.trim() === '') return { phase: 'noProject' };
  return state;
}

export interface SwarmGateState {
  blocked: boolean;
  reasons: string[];
  hints: string[];
  /** 界面是否给「仍要派发」的选择（只在核对不了时给） */
  canProceed: boolean;
}

export function swarmCheckOf(state: SwarmCheckState): SwarmPlanCheck | null {
  return state.phase === 'ok' ? state.check : null;
}

// blocked = 核对未回 / 核对失败且未选择继续 / 脏树 / 范围重叠；
// unmatched 只提示，进 hints。
export function swarmGate(state: SwarmCheckState, proceedAnyway = false): SwarmGateState {
  if (state.phase === 'idle') return { blocked: false, reasons: [], hints: [], canProceed: false };
  if (state.phase === 'noProject') {
    return { blocked: true, reasons: [SWARM_NO_PROJECT_TEXT], hints: [], canProceed: false };
  }
  if (state.phase === 'pending') {
    return { blocked: true, reasons: [], hints: [SWARM_CHECKING_TEXT], canProceed: false };
  }
  if (state.phase === 'error') {
    const detail = state.message.trim();
    const line = detail === '' ? SWARM_CHECK_FAILED_TEXT : `${SWARM_CHECK_FAILED_TEXT}：${detail}`;
    return proceedAnyway
      ? { blocked: false, reasons: [], hints: [SWARM_PROCEED_TEXT], canProceed: true }
      : { blocked: true, reasons: [line], hints: [], canProceed: true };
  }
  const check = state.check;
  const overlaps = check.overlaps ?? [];
  const unmatched = check.unmatched ?? [];
  const reasons: string[] = [];
  if (check.dirty) reasons.push(SWARM_DIRTY_TEXT);
  for (const overlap of overlaps) {
    reasons.push(`第 ${overlap.a} 件与第 ${overlap.b} 件都会改 ${overlap.path}，改到不重叠或合并成一件`);
  }
  const hints = unmatched.map((row) => `第 ${row.item} 件的范围 ${row.glob} 在项目里没有匹配到文件`);
  // 引擎的说明（超上限未处理、glob 不合法）只提示，不阻断。
  if ((check.note ?? '').trim() !== '') hints.push(check.note!.trim());
  return { blocked: check.dirty || overlaps.length > 0, reasons, hints, canProceed: false };
}

export function swarmNodeIdsOf(graph: Graph, nodeIds: string[]): string[] {
  return nodeIds
    .filter((id) => !!graph.nodes[id]?.swarm)
    .sort((a, b) => (graph.nodes[a].swarm!.item - graph.nodes[b].swarm!.item));
}

export function swarmCheckItems(graph: Graph, nodeIds: string[]): SwarmCheckItem[] {
  return swarmNodeIdsOf(graph, nodeIds).map((id) => ({
    item: graph.nodes[id].swarm!.item,
    files: [...graph.nodes[id].swarm!.files],
  }));
}

// 标红哪几件：重叠对涉及的件号。
export function swarmOverlapItems(check: SwarmPlanCheck | null | undefined): number[] {
  const out = new Set<number>();
  for (const overlap of check?.overlaps ?? []) {
    out.add(overlap.a);
    out.add(overlap.b);
  }
  return [...out].sort((a, b) => a - b);
}

// ── 4. G3 合并 ─────────────────────────────────────────────────────────────

export const SWARM_BACKEND_MISSING_NOTE = '引擎未接通蜂群合并，本次未改动项目目录';
export const SWARM_MERGE_EMPTY_TEXT = '合并结果为空，项目目录状态未知，请自行核对';
export const SWARM_MERGE_MARK = '[蜂群合并]';
export const SWARM_MERGED_TEXT = '已并入';

// 本阶段全部 swarm 节点终态（done / failed）才出「合并」按钮。
export function swarmMergeReady(graph: Graph, nodeIds: string[]): boolean {
  const ids = swarmNodeIdsOf(graph, nodeIds);
  if (ids.length === 0) return false;
  return ids.every((id) => {
    const status = graph.nodes[id].reply?.status;
    return status === 'done' || status === 'failed';
  });
}

// 已并入的件（adopted 由合并写上）不再进下一次合并请求（R1-3 幂等）。
export function swarmPendingMergeIds(graph: Graph, nodeIds: string[]): string[] {
  return swarmNodeIdsOf(graph, nodeIds).filter((id) => graph.nodes[id].reply?.adopted !== true);
}

// 按件号排序送给引擎；worktree 取本节点产物里的路径，accept 空则引擎回退到验证命令。
// wire 一律 camelCase（08C-5，A 侧 SwarmMergeItem 是 serde rename_all camelCase）。
// title 是合并提交的标题（`aicross swarm item N: <title>`），取本件消息的第一行。
export function swarmMergeItems(graph: Graph, nodeIds: string[]): SwarmMergeRequestItem[] {
  return swarmPendingMergeIds(graph, nodeIds).map((id) => {
    const node = graph.nodes[id];
    return {
      nodeId: id,
      item: node.swarm!.item,
      worktree: node.reply?.artifacts?.worktree ?? '',
      files: node.swarm!.files,
      title: (node.user.text.split('\n')[0] ?? '').trim(),
      accept: node.swarm!.accept,
    };
  });
}

export function swarmMergeFallback(
  items: SwarmMergeRequestItem[],
  note = SWARM_BACKEND_MISSING_NOTE,
): SwarmMergeResult[] {
  return items.map((item) => ({
    nodeId: item.nodeId,
    item: item.item,
    result: 'skipped' as const,
    commit: '',
    note,
    testOutput: '',
  }));
}

// 命令 reject 的两种情形要分开（R1-3）：引擎没这条命令是「未接通」，
// 其它错误照原文说，别谎称项目目录没动过。
export function swarmMergeErrorResults(
  items: SwarmMergeRequestItem[],
  error: unknown,
): SwarmMergeResult[] {
  const message = String((error as { message?: unknown } | null)?.message ?? error ?? '').trim();
  const missing = message === '' || message.includes('Tauri IPC not available');
  return swarmMergeFallback(items, missing ? SWARM_BACKEND_MISSING_NOTE : `合并未完成：${message}`);
}

const OUTCOMES = ['merged', 'conflict', 'test_failed', 'skipped'] as const;

// 引擎在另一工作树并行做，node_id/test_output 与 nodeId/testOutput 两种写法都收。
export function normalizeSwarmMergeResults(raw: unknown): SwarmMergeResult[] {
  if (!Array.isArray(raw)) return [];
  const out: SwarmMergeResult[] = [];
  for (const entry of raw) {
    if (typeof entry !== 'object' || entry === null) continue;
    const row = entry as Record<string, unknown>;
    const outcome = String(row.result ?? '');
    out.push({
      nodeId: String(row.nodeId ?? row.node_id ?? ''),
      item: Number(row.item ?? 0),
      result: (OUTCOMES as readonly string[]).includes(outcome)
        ? (outcome as SwarmMergeResult['result'])
        : 'skipped',
      commit: String(row.commit ?? ''),
      note: String(row.note ?? ''),
      testOutput: String(row.testOutput ?? row.test_output ?? ''),
    });
  }
  return out;
}

// 合并入口从图上派生（08C-2）：结果屏、重开的结果屏、单件重派之后都按图算，
// 不再绑当次 overlay 会话的 results。每件的状态取节点正文里最后一条合并留痕
// （applySwarmMerge 写的那行），没有留痕就是还没并过。
export type SwarmItemState = 'pending' | 'merged' | 'conflict' | 'test_failed' | 'skipped';

export function swarmRecordedResult(node?: GraphNode): SwarmItemState {
  const marked = (node?.reply?.text ?? '')
    .split('\n')
    .filter((line) => line.startsWith(SWARM_MERGE_MARK));
  const last = marked[marked.length - 1] ?? '';
  if (last === '') return node?.reply?.adopted === true ? 'merged' : 'pending';
  if (last.includes(SWARM_MERGED_TEXT)) return 'merged';
  if (last.includes('冲突待裁决')) return 'conflict';
  // 09-15 改名前的图写的是「测试红」，旧记录照样识别
  if (last.includes('验收未通过') || last.includes('测试红')) return 'test_failed';
  if (last.includes('已跳过')) return 'skipped';
  return 'pending';
}

export interface SwarmEntryRow {
  nodeId: string;
  item: number;
  state: SwarmItemState;
  line: string;
}

export interface SwarmMergeEntry {
  stage: number;
  /** 本阶段全部蜂群件，按件号 */
  nodeIds: string[];
  /** 全部件终态才能合并 */
  ready: boolean;
  /** 可并件（还没并入的） */
  pendingIds: string[];
  /** 已并件 */
  mergedIds: string[];
  /** 待裁决件（冲突 / 测试红） */
  disputedIds: string[];
  rows: SwarmEntryRow[];
}

// 某一阶段上的蜂群件；stage 由 swarmStageOf 从本次相关节点推出来。
export function swarmMergeEntry(graph: Graph, stage: number): SwarmMergeEntry {
  const nodeIds = Object.values(graph.nodes)
    .filter((node) => !!node.swarm && graphStage(graph, node.id) === stage)
    .sort((a, b) => a.swarm!.item - b.swarm!.item)
    .map((node) => node.id);
  const rows = nodeIds.map((id) => {
    const node = graph.nodes[id];
    const state = swarmRecordedResult(node);
    const marked = (node.reply?.text ?? '')
      .split('\n')
      .filter((line) => line.startsWith(SWARM_MERGE_MARK));
    const last = marked[marked.length - 1] ?? '';
    return {
      nodeId: id,
      item: node.swarm!.item,
      state,
      line: last === '' ? `第 ${node.swarm!.item} 件：待合并` : last.slice(SWARM_MERGE_MARK.length).trim(),
    };
  });
  return {
    stage,
    nodeIds,
    ready: swarmMergeReady(graph, nodeIds),
    pendingIds: swarmPendingMergeIds(graph, nodeIds),
    mergedIds: rows.filter((row) => row.state === 'merged').map((row) => row.nodeId),
    disputedIds: rows
      .filter((row) => row.state === 'conflict' || row.state === 'test_failed')
      .map((row) => row.nodeId),
    rows,
  };
}

// 图上还有没并完的蜂群阶段（08E）：件全部跑到终态、却有件没并入时，
// 图工具条给一个「合并」入口重开结果屏；多阶段取最深的那个。没有则 null。
export function swarmPendingMergeStage(graph: Graph): number | null {
  const stages = [...new Set(Object.values(graph.nodes)
    .filter((node) => !!node.swarm)
    .map((node) => graphStage(graph, node.id)))]
    .sort((a, b) => b - a);
  for (const stage of stages) {
    const entry = swarmMergeEntry(graph, stage);
    if (entry.ready && entry.pendingIds.length > 0) return stage;
  }
  return null;
}

// 这一屏该看哪一阶段的蜂群件：本次结果与本次可派节点里第一个蜂群件所在的阶段。
export function swarmStageOf(graph: Graph, nodeIds: string[]): number | null {
  for (const id of nodeIds) {
    if (graph.nodes[id]?.swarm) return graphStage(graph, id);
  }
  return null;
}

export function swarmResultLine(result: SwarmMergeResult): string {
  const head = `第 ${result.item} 件`;
  const tail = result.note ? ` · ${result.note}` : '';
  if (result.result === 'merged') return `${head}：${SWARM_MERGED_TEXT} ${result.commit || '（无 commit）'}`;
  if (result.result === 'conflict') return `${head}：冲突待裁决${tail}`;
  if (result.result === 'test_failed') return `${head}：验收未通过${tail}`;
  return `${head}：已跳过${tail}`;
}

// 合并这一批整体怎么说（R1-3）：只有命令 reject 或全部跳过且注明引擎未接通
// 才说「引擎未接通」；并进去一部分时按件如实报数。
export function swarmMergeSummary(results: SwarmMergeResult[]): string {
  if (results.length === 0) return SWARM_MERGE_EMPTY_TEXT;
  const allMissing = results.every(
    (row) => row.result === 'skipped' && row.note === SWARM_BACKEND_MISSING_NOTE,
  );
  if (allMissing) return SWARM_BACKEND_MISSING_NOTE;
  const labels: [SwarmMergeResult['result'], string][] = [
    ['merged', SWARM_MERGED_TEXT],
    ['conflict', '冲突'],
    ['test_failed', '验收未通过'],
    ['skipped', '跳过'],
  ];
  return labels
    .map(([kind, label]) => [label, results.filter((row) => row.result === kind).length] as const)
    .filter(([, count]) => count > 0)
    .map(([label, count]) => `${label} ${count} 件`)
    .join('，');
}

// 写进节点正文的那一行（带标记，重复合并不重复追加）。
export function swarmReplyNote(result: SwarmMergeResult): string {
  return `${SWARM_MERGE_MARK} ${swarmResultLine(result)}`;
}

// 逐件结果落到对应节点的 Reply（R1-3）：merged 追加 commit 并标采纳；
// conflict / test_failed / skipped 只追加 note，不采纳。引擎未接通的那批不写。
export function applySwarmMerge(graph: Graph, results: SwarmMergeResult[]): Graph {
  let next = graph;
  for (const result of results) {
    if (result.note === SWARM_BACKEND_MISSING_NOTE) continue;
    const node = next.nodes[result.nodeId];
    if (!node?.reply) continue;
    const line = swarmReplyNote(result);
    if (!node.reply.text.includes(line)) {
      const text = node.reply.text === '' ? line : `${node.reply.text}\n\n${line}`;
      next = {
        ...next,
        nodes: { ...next.nodes, [result.nodeId]: { ...node, reply: { ...node.reply, text } } },
      };
    }
    if (result.result === 'merged') next = setReplyAdopted(next, result.nodeId, true);
  }
  return next;
}

// 仲裁双方的候选（GLM 4）：蜂群件之间是分工不是分歧，同阶段两件都成功也不该
// 当成两路对立结果推给第三家仲裁。
export function arbitrationCandidates<T extends { nodeId: string; status: string }>(
  graph: Graph,
  rows: T[],
): T[] {
  return rows.filter((row) => row.status === 'ok' && !graph.nodes[row.nodeId]?.swarm);
}

export interface SwarmCardActions {
  showPickOne: boolean;
  showAdopt: boolean;
  showRetry: boolean;
  note: string;
}

// 蜂群件的结果卡动作（R1-4）：件与件不是擂台对手，不出「选它」；
// 「采纳」只对已并入的件有意义；已并入的件不给「重派 / 升级」，
// 否则 worktree 从已含该件的 HEAD 重开，再合一次就是重复应用。
// 已并入的件也不给「取消采纳」（08C-6）：改动已经在项目目录里，清掉 adopted
// 只会让它重回待合并清单，再并一次。
export function swarmCardActions(node?: GraphNode, result?: SwarmMergeResult): SwarmCardActions {
  if (!node?.swarm) return { showPickOne: true, showAdopt: true, showRetry: true, note: '' };
  const merged = result?.result === 'merged'
    || swarmRecordedResult(node) === 'merged'
    || node.reply?.adopted === true;
  return {
    showPickOne: false,
    showAdopt: merged && node.reply?.adopted !== true,
    showRetry: !merged,
    note: merged ? SWARM_MERGED_TEXT : '',
  };
}
