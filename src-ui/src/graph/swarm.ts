// 蜂群（TASK-08 B 节）：拆分清单解析、按清单生成执行节点、G1 重叠闸门、
// G3 合并结果——全是纯函数，界面只负责调命令和渲染。
// 命令形状住在 tauri.ts，这里 type-only 引用（运行时擦除，不构成值依赖环）。

import type { Graph, GraphNode, Preset, Target, Thinking } from './types.ts';
import { newNodeId } from './types.ts';
import { setReplyAdopted } from './dispatch.ts';
import type {
  SwarmCheckItem,
  SwarmMergeRequestItem,
  SwarmMergeResult,
  SwarmPlanCheck,
} from '../tauri';

// 与并发上限同一处设置（dispatch-settings.ts 的 swarmLimit），默认 4。
export const SWARM_LIMIT_DEFAULT = 4;

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

// 每件一个 draft 执行节点：parents = 拆分节点，write=worktree（各自 worktree），
// 件号从 1 起决定后面的合并顺序。可见性是「指定上游 = 根材料」而不是共享——
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
  const nodes = { ...graph.nodes };
  const nodeIds: string[] = [];
  const skipped: number[] = [];
  let capped = 0;
  const nowMs = Date.now();
  items.forEach((item, index) => {
    const number = index + 1;
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

export const SWARM_CHECK_FALLBACK: SwarmPlanCheck = { dirty: false, overlaps: [], unmatched: [] };

// 核对三态（R1-1）：没要核对 / 在核对 / 核对回来了 / 核对不了。
// 「还没查」与「查不了」以前都是 null，一律放行，窗口期内闸门形同虚设。
export type SwarmCheckState =
  | { phase: 'idle' }
  | { phase: 'pending' }
  | { phase: 'ok'; check: SwarmPlanCheck }
  | { phase: 'error'; message: string };

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
// nodeId 与 node_id 两个键都发（GLM 1）：契约文本是 snake_case、其它派发类结构是
// camelCase，A 侧选哪种都能反序列化，多出来的那个键被 serde 忽略。
export function swarmMergeItems(graph: Graph, nodeIds: string[]): SwarmMergeRequestItem[] {
  return swarmPendingMergeIds(graph, nodeIds).map((id) => {
    const node = graph.nodes[id];
    return {
      nodeId: id,
      node_id: id,
      item: node.swarm!.item,
      worktree: node.reply?.artifacts?.worktree ?? '',
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

export function swarmResultLine(result: SwarmMergeResult): string {
  const head = `第 ${result.item} 件`;
  const tail = result.note ? ` · ${result.note}` : '';
  if (result.result === 'merged') return `${head}：${SWARM_MERGED_TEXT} ${result.commit || '（无 commit）'}`;
  if (result.result === 'conflict') return `${head}：冲突待裁决${tail}`;
  if (result.result === 'test_failed') return `${head}：测试红${tail}`;
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
    ['test_failed', '测试红'],
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
export function swarmCardActions(node?: GraphNode, result?: SwarmMergeResult): SwarmCardActions {
  if (!node?.swarm) return { showPickOne: true, showAdopt: true, showRetry: true, note: '' };
  const merged = result?.result === 'merged' || node.reply?.adopted === true;
  return {
    showPickOne: false,
    showAdopt: merged,
    showRetry: !merged,
    note: merged ? SWARM_MERGED_TEXT : '',
  };
}
