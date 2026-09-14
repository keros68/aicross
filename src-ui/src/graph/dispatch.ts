import type { DispatchableModel, Graph, GraphNode, Preset, Reply, ReplyArtifacts, Target } from './types.ts';
import { newNodeId } from './types.ts';
import { ancestorsOf } from './layout.ts';
import { defaultTurnTarget, parseSend } from './mutate.ts';
import { arbitrationVendorConflict } from './validate.ts';
// TASK-06 R1：请求体与 git 状态的契约形状住在 tauri.ts，这里 type-only 引用
// （运行时擦除，不构成与 tauri.ts 的值依赖环）。
import type { DispatchNodeRequest, DispatchProgress, GitChanges } from '../tauri';
import acceptancePrompt from '../../../fixtures/prompts/acceptance.txt?raw';
import executionPrompt from '../../../fixtures/prompts/execution.txt?raw';
import planningPrompt from '../../../fixtures/prompts/planning.txt?raw';
import advisorPrompt from '../../../fixtures/prompts/advisor.txt?raw';

export interface BackendNodeResult {
  nodeId: string;
  stage?: number;
  status: string;
  text: string;
  modelReported: string;
  sessionId: string;
  tokensIn: number;
  tokensOut: number;
  // TASK-07-3：缓存命中的输入 token，旧结果没有这个字段时按 0 处理。
  cacheRead?: number;
  durationMs: number;
  error: string;
  runner?: 'acp' | 'oneshot' | 'unknown';
  artifacts?: ReplyArtifacts | null;
  validationOutput?: string;
  // TASK-05B：结果由备选重跑产生时带主/备路由（"agent/model"），否则缺省
  fallbackFrom?: string;
  fallbackTo?: string;
  // TASK-05B R1：可写节点主失败但未切备选时带原因（"可写节点"），空/缺省 = 未跳过
  fallbackSkipped?: string;
}

export function failedNodesWithoutResults(
  active: Pick<BackendNodeResult, 'nodeId'>[],
  results: BackendNodeResult[],
  error: string,
): BackendNodeResult[] {
  const received = new Set(results.map((result) => result.nodeId));
  return active.filter((node) => !received.has(node.nodeId)).map((node) => ({
    nodeId: node.nodeId,
    stage: 1,
    status: 'error',
    text: '',
    modelReported: '',
    sessionId: '',
    tokensIn: 0,
    tokensOut: 0,
    durationMs: 0,
    error,
    artifacts: null,
  }));
}

type RoleTarget = { agent: string; provider?: string; model: string; tier?: string; thinking: string };

export function applyRoleDefaults<T extends RoleTarget & { role: string }>(
  target: T,
  roles: { planning: RoleTarget; execution: RoleTarget; acceptance: RoleTarget },
): T {
  if (target.model.trim()) return target;
  const profile = target.role === '出方案' ? roles.planning : target.role === '执行' ? roles.execution : target.role === '验收' ? roles.acceptance : undefined;
  if (!profile || !profile.model.trim()) return target;
  return { ...target, agent: profile.agent, provider: profile.provider, model: profile.model, tier: profile.tier, thinking: profile.thinking };
}

// 盖章前把 roles.json 的角色默认填进空目标的预设节点。擂台（builtin-arena）跳过：
// 三路目标必须留空由用户在 G1 逐家选，否则会被填成同一家（GraphTab 与
// OrchestrationPanel 两处调用点共用）。
export function presetWithRoleDefaults(
  preset: Preset,
  roles: { planning: RoleTarget; execution: RoleTarget; acceptance: RoleTarget },
): Preset {
  if (preset.id === 'builtin-arena') return preset;
  return {
    ...preset,
    stages: preset.stages.map((stage) => ({
      ...stage,
      nodes: stage.nodes.map((target) => applyRoleDefaults(target, roles)),
    })),
  };
}

export function dispatchableNodeIds(graph: Graph): string[] {
  return Object.values(graph.nodes)
    .filter((node) => {
      if (node.reply?.status !== 'draft' || !node.target) return false;
      return node.parents.every((parentId) => {
        const parent = graph.nodes[parentId];
        return !!parent && (parent.kind === 'material' || parent.reply?.status === 'done');
      });
    })
    .sort((a, b) => a.created_at.localeCompare(b.created_at) || a.id.localeCompare(b.id))
    .map((node) => node.id);
}

export function missingTargetNodeIds(graph: Graph, nodeIds: string[]): string[] {
  return nodeIds.filter((id) => {
    const target = graph.nodes[id]?.target;
    return !target?.agent.trim() || !target.model.trim();
  });
}

export type NextStageStep =
  | { kind: 'dispatch'; nodeIds: string[] }
  | { kind: 'missing-target'; nodeIds: string[] }
  | { kind: 'limit'; nodeIds: string[] }
  | { kind: 'done'; nodeIds: [] };

// skip：用户在 G1 取消勾选的「待核对」节点（TASK-06 R1-7）。派发集合是从图上
// 重算的，不过滤的话这些节点第一层就被派出去，后续层还会再捡起来——与确认屏
// 授权的集合不一致，单写者还会在半截改动上再叠一遍。过滤后为空按 done 处理。
export function nextStageStep(
  graph: Graph,
  collected: Pick<BackendNodeResult, 'nodeId'>[],
  settings: { maxRequests: number },
  skip: ReadonlySet<string> = new Set<string>(),
): NextStageStep {
  const nodeIds = dispatchableNodeIds(graph).filter((id) => !skip.has(id));
  if (nodeIds.length === 0) return { kind: 'done', nodeIds: [] };
  const missing = missingTargetNodeIds(graph, nodeIds);
  if (missing.length > 0) return { kind: 'missing-target', nodeIds: missing };
  if (!withinRequestLimit(collected.length, nodeIds.length, settings.maxRequests)) {
    return { kind: 'limit', nodeIds };
  }
  return { kind: 'dispatch', nodeIds };
}

export function continuationSession(graph: Graph, nodeId: string): string | undefined {
  const node = graph.nodes[nodeId];
  if (!node?.target || node.target.visibility !== '共享' || node.parents.length !== 1) return undefined;
  const parent = graph.nodes[node.parents[0]];
  if (!parent?.target || !parent.reply?.session_id) return undefined;
  // TASK-06-6：父节点是盲节点时，它的会话跑在隔离目录、只看得到外发材料，
  // 续到共享子节点上等于把盲分支的上下文漏过来——发完整 prompt，不续聊。
  if (parent.target.visibility === '盲') return undefined;
  // R2-1：父回复是备选作答（fallback 字段或旧图的首行标记）时，session 属于
  // 备选通道，不能续到本节点的主通道上——发完整 prompt，不续聊。
  if (fallbackNoticeOf(parent.reply) !== null) return undefined;
  const sameTarget =
    parent.target.agent === node.target.agent &&
    (parent.target.provider ?? '') === (node.target.provider ?? '') &&
    parent.target.model === node.target.model;
  return sameTarget ? parent.reply.session_id : undefined;
}

export function continuationRunner(graph: Graph, nodeId: string): 'acp' | 'oneshot' | undefined {
  const node = graph.nodes[nodeId];
  if (!continuationSession(graph, nodeId) || !node || node.parents.length !== 1) return undefined;
  const runner = graph.nodes[node.parents[0]]?.reply?.runner;
  return runner === 'acp' || runner === 'oneshot' ? runner : undefined;
}

export function clearTargetSession(graph: Graph, nodeId: string): Graph {
  const node = graph.nodes[nodeId];
  if (!node?.target) return graph;
  const nodes = { ...graph.nodes };
  const pending = [...node.parents];
  const seen = new Set<string>();
  while (pending.length > 0) {
    const id = pending.pop() as string;
    if (seen.has(id)) continue;
    seen.add(id);
    const ancestor = nodes[id];
    if (!ancestor) continue;
    const sameTarget = ancestor.target
      && ancestor.target.agent === node.target.agent
      && (ancestor.target.provider ?? '') === (node.target.provider ?? '')
      && ancestor.target.model === node.target.model;
    if (sameTarget && ancestor.reply?.session_id) {
      nodes[id] = {
        ...ancestor,
        reply: { ...ancestor.reply, session_id: undefined },
      };
    }
    pending.push(...ancestor.parents);
  }
  return { ...graph, nodes };
}

export function setReplyAdopted(graph: Graph, nodeId: string, adopted: boolean): Graph {
  const node = graph.nodes[nodeId];
  if (!node?.reply) return graph;
  return {
    ...graph,
    nodes: {
      ...graph.nodes,
      [nodeId]: { ...node, reply: { ...node.reply, adopted } },
    },
  };
}

export function markPending(graph: Graph, nodeIds: string[]): Graph {
  const ids = new Set(nodeIds);
  const nodes = { ...graph.nodes };
  for (const id of ids) {
    const node = nodes[id];
    if (!node) continue;
    nodes[id] = { ...node, reply: { ...(node.reply ?? { text: '' }), status: 'pending' } };
  }
  return { ...graph, nodes };
}

// 「已切备选」整行。TASK-06 起首选 Reply.fallback_from/to 两个字段（正文不再
// 前置标记行，标记不会随 serializeContext 进下游 prompt）；旧图里只有正文首行
// 标记的，退回按标记识别。
export function fallbackNoticeOf(reply?: Reply): string | null {
  if (reply?.fallback_from) {
    return `（已切备选：主 ${reply.fallback_from} → 备 ${reply.fallback_to ?? ''}）`;
  }
  const first = reply?.text.split('\n', 1)[0] ?? '';
  return first.startsWith('（已切备选：') ? first : null;
}

// G1 请求体里的备选段：节点 target.fallback 存在且 model 非空时下发，否则不带。
// R2-4：vendor 用 models 里的真实值随请求下发（先按 provider 精确匹配，再退
// agent+id，与 DispatchOverlay 决策行的口径一致），通道层留痕不再靠模型 ID 猜。
export function fallbackSpecOf(
  target: Target,
  models: DispatchableModel[] = [],
): { agent: string; provider: string; model: string; vendor: string } | undefined {
  const fallback = target.fallback;
  if (!fallback || !fallback.model.trim()) return undefined;
  const picked = models.find((item) =>
    item.agent === fallback.agent && item.id === fallback.model &&
    (item.provider ?? '') === (fallback.provider ?? ''))
    ?? models.find((item) => item.agent === fallback.agent && item.id === fallback.model);
  return { agent: fallback.agent, provider: fallback.provider ?? '', model: fallback.model, vendor: picked?.vendor ?? '' };
}

// G1 提示：可写节点（write != 只读）配了备选时提示——备选重跑只对只读/盲节点
// 触发（引擎侧保证），可写节点主失败即终态，不自动切备选。
export function writableFallbackHintOf(target: Target): string | null {
  return target.write !== '只读' && !!target.fallback?.model.trim() ? '可写节点不自动切备选' : null;
}

// 结果卡：可写节点主失败但未切备选时的说明行；fallbackSkipped 空/缺省 = 未跳过。
export function fallbackSkippedLineOf(result: Pick<BackendNodeResult, 'fallbackSkipped'>): string | null {
  return result.fallbackSkipped ? `主失败，未切备选（${result.fallbackSkipped}）` : null;
}

export function applyDispatchResults(graph: Graph, results: BackendNodeResult[]): Graph {
  const nodes = { ...graph.nodes };
  for (const result of results) {
    const node = nodes[result.nodeId];
    if (!node) continue;
    const ok = result.status === 'ok';
    const validationText = result.status === 'validation_failed' && result.validationOutput
      ? `${result.text}${result.text ? '\n\n' : ''}[验证输出]\n${result.validationOutput}`
      : undefined;
    const body = validationText ?? (ok ? result.text : result.text || result.error);
    const reply: Reply = {
      status: ok ? 'done' : 'failed',
      text: body,
      // TASK-06-3：主/备路由进字段，不再前置标记行（正文干净，不污染下游上下文）
      ...(result.fallbackFrom
        ? { fallback_from: result.fallbackFrom, fallback_to: result.fallbackTo ?? '' }
        : {}),
      ...(ok ? {} : { error_kind: result.status }),
      model_reported: result.modelReported || undefined,
      tokens_in: result.tokensIn || undefined,
      tokens_out: result.tokensOut || undefined,
      // TASK-07-3：cache_read 进图，结果屏与下次打开都读得到
      tokens_cache_read: result.cacheRead || undefined,
      ms: result.durationMs,
      session_id: result.sessionId || undefined,
      ...(result.runner ? { runner: result.runner } : {}),
      artifacts: result.artifacts ?? undefined,
      ...(node.reply?.adopted === undefined ? {} : { adopted: node.reply.adopted }),
    };
    nodes[result.nodeId] = { ...node, reply };
  }
  return { ...graph, nodes };
}

export function redraftNode(graph: Graph, nodeId: string, upgrade = false): Graph {
  const node = graph.nodes[nodeId];
  if (!node?.target) return graph;
  const thinking = upgrade ? nextThinking(node.target.thinking) : node.target.thinking;
  const tier = upgrade ? nextTier(node.target.tier) : node.target.tier;
  return {
    ...graph,
    nodes: {
      ...graph.nodes,
      [nodeId]: {
        ...node,
        target: { ...node.target, tier, thinking },
        reply: { status: 'draft', text: '' },
      },
    },
  };
}

function nextTier(value: Target['tier']): Target['tier'] {
  if (!value || value === '低') return '中';
  if (value === '中') return '高';
  return value;
}

function nextThinking(value: Target['thinking']): Target['thinking'] {
  if (value === '关') return '低';
  if (value === '低') return '中';
  return '高';
}

export function appendDecision(graph: Graph, resultNodeIds: string[], text: string): Graph {
  const parents = [...new Set(resultNodeIds)].filter((id) => !!graph.nodes[id]);
  if (parents.length === 0) return graph;
  const id = newNodeId();
  const node: GraphNode = {
    id,
    kind: parents.length > 1 ? 'accept' : 'turn',
    parents,
    user: { text, attachments: [] },
    created_at: new Date().toISOString(),
  };
  return { ...graph, nodes: { ...graph.nodes, [id]: node } };
}

export function routeLine(node: GraphNode, models: DispatchableModel[], isolation: string): string {
  const target = node.target;
  if (!target) return '';
  const model = models.find(
    (item) =>
      item.agent === target.agent &&
      item.id === target.model &&
      (item.provider ?? '') === (target.provider ?? ''),
  );
  const correction = node.user.text.match(/^\[修正 (\d+)\/3\]\s*/);
  const summary = correction ? `执行（修正 ${correction[1]}/3）` : node.user.text.replace(/\s+/g, ' ').trim().slice(0, 24) || target.role || '任务';
  const route = target.agent.trim() && target.model.trim()
    ? `${target.agent}/${target.model}`
    : '未指定模型';
  const validation = (target as Target & { validation_command?: string }).validation_command?.trim() ? ` ｜ 验证=${(target as Target & { validation_command?: string }).validation_command?.trim()}` : '';
  const warning = target.agent === 'cc-switch' && ['单写者', 'worktree'].includes(target.write) ? ' ｜ cc-switch 单轮无工具，产物将为空' : target.agent === 'kimi' && target.visibility === '盲' ? ' ｜ kimi 无只读档，隔离=部分' : '';
  const fallback = target.fallback?.model.trim()
    ? ` ｜ 备选=${target.fallback!.agent}/${target.fallback!.model}`
    : '';
  return `[aicross] ${summary}${correction ? '→ ' : ' → '}${route} ｜ ${target.tier || model?.tier || '未分档'} · thinking=${target.thinking} ｜ 可见性=${target.visibility} ｜ 写=${target.write} ｜ 隔离=${isolation}${validation}${warning}${fallback}`;
}

export function routeReason(node: GraphNode): string | undefined {
  const text = node.user.text.replace(/^\[修正 \d+\/3\]\s*/, '').trim();
  return text.startsWith('验证命令未通过') ? text : undefined;
}

export function nodeStatusLabel(node: GraphNode): string | null {
  if (node.kind === 'material') return null;
  if (node.reply?.skipped_reason) return '已跳过';
  if (node.reply?.status === 'failed') {
    if (node.reply.error_kind === 'identity_mismatch') return '真身不符';
    if (node.reply.error_kind === 'validation_failed') return '验证失败';
    return '失败';
  }
  // 对账判「未知」的节点（R1-10）：draft 但正文是那句说明，节点卡要看得出来，
  // 否则用户只在 G1 分区里见过一次，回到图上与普通「待派」没有区别。
  if (node.reply?.status === 'draft' && node.reply.text === RECONCILE_UNKNOWN_TEXT) return '待核对';
  return ({ draft: '待派', pending: '运行中', done: '完成' } as const)[node.reply?.status ?? 'draft'];
}

export type AcceptanceVerdict = 'PASS' | 'NEEDS_FIX' | 'UNKNOWN';

export function parseAcceptance(text: string): AcceptanceVerdict {
  const tail = text.slice(-1200).toUpperCase();
  const matches = [...tail.matchAll(/\b(?:PASS|NEEDS_FIX)\b/g)];
  return (matches.at(-1)?.[0] as AcceptanceVerdict | undefined) ?? 'UNKNOWN';
}

export function isSmallEconomyTask(text: string, attachments: string[]): boolean {
  return text.length < 120 && attachments.length === 0 && !/[\r\n]|测试|验证|新建|实现|并/.test(text);
}

export function needsChainThinking(text: string): boolean {
  const numbers = text.match(/\d+(?:\.\d+)?/g)?.length ?? 0;
  return numbers >= 3 || /(计算|统计|推导|递推)/.test(text);
}

export const ROLE_PROMPT_TEMPLATES = {
  '执行': executionPrompt.trim(),
  '验收': acceptancePrompt.trim(),
  '出方案': planningPrompt.trim(),
  '顾问': advisorPrompt.trim(),
} as const;

export function promptForRole(role: string, prompt: string): string {
  if (role === '执行') {
    return `${ROLE_PROMPT_TEMPLATES['执行']}\n\n${prompt}`;
  }
  if (role === '验收') {
    return `${prompt}\n\n${ROLE_PROMPT_TEMPLATES['验收']}`;
  }
  if (role === '出方案') {
    return `${prompt}\n\n${ROLE_PROMPT_TEMPLATES['出方案']}`;
  }
  if (role === '顾问') {
    return `${prompt}\n\n${ROLE_PROMPT_TEMPLATES['顾问']}`;
  }
  return prompt;
}

// ── TASK-05B 第 2 期预设的结果屏动作与输入解析 ────────────────────────────

// 擂台「选它」：被选节点 adopted=true，同父、同 kind、同角色的其它并行候选清掉
// adopted（不同角色的节点——如同父的 /advise 顾问——不算候选，不动它）。
// 「设为后续列末端」是 UI 动作（SET_GRAPH_FOCUS），不在这个纯函数里。
export function pickOne(graph: Graph, nodeId: string): Graph {
  const node = graph.nodes[nodeId];
  if (!node?.reply) return graph;
  const rivals = Object.values(graph.nodes)
    .filter((other) =>
      other.id !== nodeId &&
      other.kind === node.kind &&
      (other.target?.role ?? '') === (node.target?.role ?? '') &&
      other.parents.length === node.parents.length &&
      other.parents.every((p) => node.parents.includes(p)) &&
      other.reply?.adopted === true)
    .map((other) => other.id);
  let next = setReplyAdopted(graph, nodeId, true);
  for (const id of rivals) next = setReplyAdopted(next, id, false);
  return next;
}

// 「选它」按钮的可见条件：同阶段结果里与本节点 parents 集合相同（同父竞争）且
// kind、target.role 一致（R2-10，与 pickOne 的候选口径对齐：同父顾问不算擂台
// 对手）的结果数（含自身）≥2 才显示；同阶段但不同分支的结果不算候选。
export function sameParentsResultCount(
  graph: Graph,
  results: Pick<BackendNodeResult, 'nodeId'>[],
  nodeId: string,
): number {
  const node = graph.nodes[nodeId];
  if (!node) return 0;
  return results.filter((result) => {
    const other = graph.nodes[result.nodeId];
    return !!other &&
      other.kind === node.kind &&
      (other.target?.role ?? '') === (node.target?.role ?? '') &&
      other.parents.length === node.parents.length &&
      other.parents.every((p) => node.parents.includes(p));
  }).length;
}

// 仲裁的「分歧中性描述」：标签用甲方/乙方（不出现双方 agent/model，避免厂商锚定），
// 不带立场词。用户在结果屏可改这段文本再建节点。
const TURNING_WORDS = /但|然而|不过|却|反之|相反/;

// 摘录：超限时优先保留含转折词的句子，再按原序补到预算内，末尾加省略号。
function disputeExcerpt(text: string, limit = 200): string {
  const clean = text.replace(/\s+/g, ' ').trim();
  if (clean.length <= limit) return clean;
  const sentences = clean.match(/[^。！？；.!?;]+[。！？；.!?;]?/g) ?? [clean];
  const chosen = new Set<number>();
  let used = 0;
  const take = (index: number) => {
    if (chosen.has(index) || used + sentences[index].length > limit) return;
    chosen.add(index);
    used += sentences[index].length;
  };
  sentences.forEach((sentence, index) => { if (TURNING_WORDS.test(sentence)) take(index); });
  sentences.forEach((_, index) => take(index));
  const out = chosen.size > 0
    ? [...chosen].sort((a, b) => a - b).map((index) => sentences[index]).join('')
    : clean.slice(0, limit);
  return `${out.slice(0, limit)}…`;
}

export function neutralDisputeSummary(graph: Graph, aId: string, bId: string): string {
  const take = (id: string) => disputeExcerpt(graph.nodes[id]?.reply?.text ?? '');
  return `【甲方】认为：${take(aId)}；【乙方】认为：${take(bId)}。请独立裁决，给出你的结论与理由。`;
}

// 仲裁建节点：盲发第三家（隔离目录、只拷共同材料祖先的用户消息与附件、强制只读，
// 引擎盲分支语义）。parents = 分歧双方的共同材料祖先（一般是根），kind 按 parents
// 数定（≥2 → accept）；role=仲裁、write=只读 强制。
// 目标厂商与任一方相同则拒绝（validate 拦同厂商）。
export function appendArbitration(
  graph: Graph,
  aId: string,
  bId: string,
  text: string,
  target: Target,
  models: DispatchableModel[],
): { graph: Graph; nodeId?: string; error?: string } {
  if (!graph.nodes[aId] || !graph.nodes[bId] || aId === bId) {
    return { graph, error: '仲裁需要两个不同的分歧节点' };
  }
  if (arbitrationVendorConflict(graph, aId, bId, target, models)) {
    return { graph, error: '仲裁应与双方不同厂商' };
  }
  const bAncestors = new Set(ancestorsOf(graph, bId));
  const materials = ancestorsOf(graph, aId).filter(
    (id) => bAncestors.has(id) && graph.nodes[id]?.kind === 'material',
  );
  if (materials.length === 0) {
    return { graph, error: '双方没有共同的材料祖先，无法仲裁' };
  }
  const id = newNodeId();
  const node: GraphNode = {
    id,
    kind: materials.length >= 2 ? 'accept' : 'turn',
    parents: [...materials],
    user: { text, attachments: [] },
    reply: { status: 'draft', text: '' },
    target: {
      ...target,
      role: '仲裁',
      visibility: '盲',
      upstreams: undefined,
      write: '只读',
    },
    created_at: new Date().toISOString(),
  };
  return { graph: { ...graph, nodes: { ...graph.nodes, [id]: node } }, nodeId: id };
}

// 中栏输入 `/advise @agent/model 问题`：盖 role=顾问、盲、只读 的 turn 节点。
// 不带 @ 时目标留空，G1 确认屏再选。`/advisefoo` 之类前缀撞名不识别。
export function parseAdvise(
  raw: string,
  models: DispatchableModel[],
): { question: string; target: Target } | null {
  if (!/^\/advise(?:\s|$)/.test(raw)) return null;
  const rest = raw.slice('/advise'.length).trim();
  const parsed = parseSend(rest, models);
  return {
    question: parsed.text,
    target: {
      ...(parsed.target ?? defaultTurnTarget()),
      role: '顾问',
      visibility: '盲',
      write: '只读',
    },
  };
}

export function graphStage(graph: Graph, nodeId: string): number {
  const memo = new Map<string, number>();
  const visit = (id: string, seen = new Set<string>()): number => {
    if (memo.has(id)) return memo.get(id) as number;
    if (seen.has(id)) return 0;
    const node = graph.nodes[id];
    if (!node || node.parents.length === 0) return 0;
    const nextSeen = new Set(seen).add(id);
    const depth = 1 + Math.max(0, ...node.parents.map((parent) => visit(parent, nextSeen)));
    memo.set(id, depth);
    return depth;
  };
  return visit(nodeId);
}

export function appendCorrectionRound(graph: Graph, acceptId: string): { graph: Graph; executionId?: string; acceptanceId?: string } {
  const accept = graph.nodes[acceptId];
  if (!accept?.target || accept.target.role !== '验收') return { graph };
  const previous = accept.parents.map((id) => graph.nodes[id]).find((node) => node?.target?.role === '执行');
  if (!previous?.target || !previous.reply?.artifacts) return { graph };
  const executionId = newNodeId();
  const acceptanceId = newNodeId();
  const now = Date.now();
  const execution: GraphNode = {
    id: executionId,
    kind: 'turn',
    parents: [acceptId],
    user: {
      text: `根据上一轮验收意见修正实现。\n\n${accept.reply?.text ?? ''}`,
      attachments: [],
    },
    reply: { status: 'draft', text: '' },
    target: { ...previous.target, upstreams: [acceptId, previous.id] },
    created_at: new Date(now).toISOString(),
  };
  const acceptance: GraphNode = {
    id: acceptanceId,
    kind: 'turn',
    parents: [executionId],
    user: { text: '', attachments: [] },
    reply: { status: 'draft', text: '' },
    target: { ...accept.target, upstreams: accept.target.upstreams ? [...accept.target.upstreams] : undefined },
    created_at: new Date(now + 1).toISOString(),
  };
  return {
    graph: { ...graph, nodes: { ...graph.nodes, [executionId]: execution, [acceptanceId]: acceptance } },
    executionId,
    acceptanceId,
  };
}

export function appendValidationRetry(
  graph: Graph,
  executionId: string,
  detail: string,
  retryRound = 1,
): { graph: Graph; executionId?: string; acceptanceId?: string } {
  const execution = graph.nodes[executionId];
  if (!execution?.target) return { graph };
  const waitingAccept = Object.values(graph.nodes).find(
    (node) => node.parents.includes(executionId) && node.target?.role === '验收' && node.reply?.status === 'draft',
  );
  if (!waitingAccept?.target) return { graph };
  const nextExecutionId = newNodeId();
  const nextAcceptanceId = newNodeId();
  const now = Date.now();
  return {
    graph: {
      ...graph,
      nodes: {
      ...graph.nodes,
      [waitingAccept.id]: { ...waitingAccept, reply: { status: 'failed', text: '验证命令未通过，未送审' } },
      [nextExecutionId]: {
        id: nextExecutionId,
        kind: execution.parents.length > 1 ? 'accept' : 'turn',
        parents: [...execution.parents],
        user: { text: `[修正 ${retryRound}/3] 验证命令未通过，请修正后重新运行。\n\n${detail}`, attachments: [] },
        reply: { status: 'draft', text: '' },
        target: { ...execution.target },
        created_at: new Date(now).toISOString(),
      },
      [nextAcceptanceId]: {
        id: nextAcceptanceId,
        kind: 'turn',
        parents: [nextExecutionId],
        user: { text: '', attachments: [] },
        reply: { status: 'draft', text: '' },
        target: { ...waitingAccept.target },
        created_at: new Date(now + 1).toISOString(),
      },
      },
    },
    executionId: nextExecutionId,
    acceptanceId: nextAcceptanceId,
  };
}

function closeWaitingAcceptance(graph: Graph, executionId: string): Graph {
  const waitingAccept = Object.values(graph.nodes).find(
    (node) => node.parents.includes(executionId) && node.target?.role === '验收' && node.reply?.status === 'draft',
  );
  if (!waitingAccept) return graph;
  return {
    ...graph,
    nodes: {
      ...graph.nodes,
      [waitingAccept.id]: {
        ...waitingAccept,
        reply: { status: 'draft', text: '', skipped_reason: '上游验证未通过' },
      },
    },
  };
}

export function acceptanceRound(graph: Graph, nodeId: string): number {
  let count = 0;
  const pending = [nodeId];
  const seen = new Set<string>();
  while (pending.length) {
    const id = pending.pop() as string;
    if (seen.has(id)) continue;
    seen.add(id);
    const node = graph.nodes[id];
    if (!node) continue;
    if (node.target?.role === '验收') count += 1;
    pending.push(...node.parents);
  }
  return count;
}

export interface PipelineStep {
  graph: Graph;
  validationRetries: Record<string, number>;
  continueRunning: boolean;
}

// DispatchOverlay.start 只负责 IO；封顶、失败停止与修正轮生长由这个纯函数决定。
export function reducePipelineStep(
  graph: Graph,
  results: BackendNodeResult[],
  validationRetries: Record<string, number>,
): PipelineStep {
  const validationFailures = results.filter((result) => result.status === 'validation_failed');
  if (validationFailures.length > 0) {
    let next = graph;
    let addedRetry = false;
    const retries = { ...validationRetries };
    for (const failure of validationFailures) {
      const count = retries[failure.nodeId] ?? 0;
      if (count >= 3) {
        next = closeWaitingAcceptance(next, failure.nodeId);
        continue;
      }
      const appended = appendValidationRetry(
        next,
        failure.nodeId,
        failure.validationOutput || failure.error,
        count + 1,
      );
      next = appended.graph;
      if (appended.executionId) {
        retries[appended.executionId] = count + 1;
        addedRetry = true;
      }
    }
    return { graph: next, validationRetries: retries, continueRunning: addedRetry };
  }
  if (results.some((result) => result.status !== 'ok')) {
    return { graph, validationRetries, continueRunning: false };
  }

  const acceptanceResults = results.filter(
    (result) => graph.nodes[result.nodeId]?.target?.role === '验收',
  );
  if (acceptanceResults.length === 0) {
    return { graph, validationRetries, continueRunning: true };
  }

  let next = graph;
  let addedCorrection = false;
  for (const result of acceptanceResults) {
    if (parseAcceptance(result.text) !== 'NEEDS_FIX' || acceptanceRound(next, result.nodeId) > 3) continue;
    const appended = appendCorrectionRound(next, result.nodeId);
    next = appended.graph;
    addedCorrection ||= !!appended.executionId;
  }
  return { graph: next, validationRetries, continueRunning: addedCorrection };
}

export interface ResultStageGroup<T extends BackendNodeResult = BackendNodeResult> {
  stage: number;
  title: string;
  rows: T[];
}

export function groupResultsByStage<T extends BackendNodeResult>(graph: Graph, results: T[]): ResultStageGroup<T>[] {
  return [...new Set(results.map((result) => result.stage ?? 1))]
    .sort((a, b) => a - b)
    .map((stage) => {
      const rows = results.filter((result) => (result.stage ?? 1) === stage);
      const role = rows
        .map((result) => graph.nodes[result.nodeId]?.target?.role?.trim())
        .find(Boolean);
      return { stage, title: role || `第 ${stage} 阶段`, rows };
    });
}

export function withinRequestLimit(dispatched: number, next: number, limit: number): boolean {
  return dispatched + next <= Math.max(1, limit);
}

// ── TASK-06 重派与崩溃恢复的一致性 ────────────────────────────────────────

// reconcile_dispatch_state 的返回（camelCase，与 Rust 侧契约镜像）。
export type ReconcileVerdict = 'completed' | 'incomplete' | 'unknown';

export interface ReconcileNodeSummary {
  nodeId: string;
  verdict: ReconcileVerdict;
  recordPath?: string;
  replyText?: string;
  artifacts?: ReplyArtifacts | null;
  error?: string;
  // R1 追加（契约）：崩溃前那次实际跑完的会话、运行方式与用量，从留痕
  // frontmatter 取，缺失为空/0。不收回的话下游子节点续不上会话、结果也没用量。
  sessionId?: string;
  runner?: string;
  usage?: { input: number; output: number; cacheRead: number };
}

export interface ReconcileSummary {
  exists: boolean;
  runId: string;
  nodes: ReconcileNodeSummary[];
}

// rollback_node_changes 的返回（camelCase 契约；本期前端只展示 note）。
export interface RollbackSummary {
  restored: string[];
  removed: string[];
  skipped: string[];
  note: string;
}

export const RECONCILE_UNKNOWN_TEXT = '上次派发结果未知，请检查项目目录后重派或放弃';

// 崩溃后「继续」的回退路径：把还挂在 pending 的节点放回 draft 重跑。
// 对账接通时先跑这一步，再由 applyReconcile 覆盖留痕说得清的节点。
export function resetPendingToDraft(graph: Graph): Graph {
  const nodes = { ...graph.nodes };
  let touched = false;
  for (const [id, node] of Object.entries(nodes)) {
    if (node.reply?.status !== 'pending') continue;
    nodes[id] = { ...node, reply: { ...node.reply, status: 'draft' } };
    touched = true;
  }
  return touched ? { ...graph, nodes } : graph;
}

// 留痕正文进 Reply 的上限（R1-11）：留痕若没被切干净，整份（可能含裸 stdout）
// 会进 Reply.text，再随 serializeContext 外发给下游。超限截断并说明。
export const RECONCILE_TEXT_LIMIT = 64 * 1024;

export function clampReconcileText(text: string): string {
  if (text.length <= RECONCILE_TEXT_LIMIT) return text;
  return `${text.slice(0, RECONCILE_TEXT_LIMIT)}\n…（正文超过 ${RECONCILE_TEXT_LIMIT} 字符已截断，完整内容见留痕文件）`;
}

function reconcileRunner(runner?: string): Reply['runner'] | undefined {
  return runner === 'acp' || runner === 'oneshot' || runner === 'unknown' ? runner : undefined;
}

// 对账结果写回图：completed 收回留痕正文、产物与会话/用量（status done）；
// incomplete 回 draft 重跑；unknown 回 draft 并写一句说明，由 G1 的「待核对」
// 区决定派不派。
// 前置条件（R1-4）：只覆写 pending / draft 的节点。崩溃后已经重派成功的节点
// （done）、已判失败的节点不能被一条更旧的留痕抹掉。没有 reply 的节点（材料节点
// 和刚建出来还没写过 reply 的节点）状态取不到，按「非 pending / draft」一起跳过
// ——留痕本来也不会点到它们（07-5）。
// 会话（R1-9）：incomplete / unknown 不保留上一条 reply 的 session_id / runner
// ——重跑即新会话，半途或结果未知的会话不能被子节点当作可续父会话。
export function applyReconcile(graph: Graph, summary: ReconcileSummary | null | undefined): Graph {
  if (!summary?.exists || summary.nodes.length === 0) return graph;
  const nodes = { ...graph.nodes };
  let touched = false;
  for (const row of summary.nodes) {
    const node = nodes[row.nodeId];
    if (!node) continue;
    const status = node.reply?.status;
    if (status !== 'pending' && status !== 'draft') continue;
    const adopted = node.reply?.adopted === undefined ? {} : { adopted: node.reply.adopted };
    const runner = reconcileRunner(row.runner);
    const reply: Reply =
      row.verdict === 'completed'
        ? {
            status: 'done',
            text: clampReconcileText(row.replyText ?? ''),
            ...(row.artifacts ? { artifacts: row.artifacts } : {}),
            ...(row.sessionId ? { session_id: row.sessionId } : {}),
            ...(runner ? { runner } : {}),
            ...(row.usage?.input ? { tokens_in: row.usage.input } : {}),
            ...(row.usage?.output ? { tokens_out: row.usage.output } : {}),
            // TASK-07-3：R1 收不回的 cacheRead 现在有图字段了
            ...(row.usage?.cacheRead ? { tokens_cache_read: row.usage.cacheRead } : {}),
            ...adopted,
          }
        : row.verdict === 'incomplete'
          ? { status: 'draft', text: '', ...adopted }
          : { status: 'draft', text: RECONCILE_UNKNOWN_TEXT, ...adopted };
    nodes[row.nodeId] = { ...node, reply };
    touched = true;
  }
  return touched ? { ...graph, nodes } : graph;
}

// 重复批次守卫用（R1-3）：本层节点已 markPending 并提交过，停止推进前把它们
// 放回 draft，不能悬挂在 pending 上。
export function resetNodesToDraft(graph: Graph, nodeIds: string[]): Graph {
  const nodes = { ...graph.nodes };
  let touched = false;
  for (const id of nodeIds) {
    const node = nodes[id];
    if (node?.reply?.status !== 'pending') continue;
    nodes[id] = { ...node, reply: { ...node.reply, status: 'draft' } };
    touched = true;
  }
  return touched ? { ...graph, nodes } : graph;
}

// 「待核对」节点：留痕找不到、结果未知的那些。G1 对它们显示标签且默认不勾选。
export function reconcileNeedsReview(summary: ReconcileSummary | null | undefined): string[] {
  if (!summary?.exists) return [];
  return summary.nodes.filter((row) => row.verdict === 'unknown').map((row) => row.nodeId);
}

// G1 初始勾选：待核对节点默认不派（用户核对完项目目录再手动勾回来）。
export function initialDispatchSelection(ids: string[], needsReview: string[] = []): string[] {
  const review = new Set(needsReview);
  return ids.filter((id) => !review.has(id));
}

// 对账后工具条上的那点状态（R1-1）：它属于某一张图，不属于组件。带上 graphId，
// 渲染前过一次 reconcileUiFor——换图后 id 对不上就当空的，不会把上一张图的
// 「已按留痕对账 N 个节点」和「待核对」标签带过去。
export interface ReconcileUi {
  graphId: string;
  needsReview: string[];
  note: string;
}

export const EMPTY_RECONCILE_UI: ReconcileUi = { graphId: '', needsReview: [], note: '' };

export function reconcileUiFor(state: ReconcileUi, graphId: string): ReconcileUi {
  return state.graphId !== '' && state.graphId === graphId ? state : EMPTY_RECONCILE_UI;
}

export function reconcileUiAfter(
  graphId: string,
  summary: ReconcileSummary | null | undefined,
): ReconcileUi {
  const needsReview = reconcileNeedsReview(summary);
  return {
    graphId,
    needsReview,
    note: summary?.exists
      ? `已按留痕对账 ${summary.nodes.length} 个节点${needsReview.length > 0 ? `，${needsReview.length} 个待核对` : ''}`
      : '未能对账，按上次的待派节点重跑',
  };
}

// 请求体里的 rollbackBefore：单写者（write != 只读 且非 worktree）节点重派时
// 为真，引擎在写锁内先按 before 快照回滚上次改动再派发。只读节点没有改动可滚，
// worktree 节点每次从 HEAD 新建、天然幂等，两者都不带这个字段。
export function rollbackBeforeOf(target: Target | undefined, retrying: boolean): true | undefined {
  if (!retrying || !target) return undefined;
  return target.write !== '只读' && target.write !== 'worktree' ? true : undefined;
}

// G1 提示：单写者重派会先回滚。
export function rollbackHintOf(target: Target | undefined, retrying: boolean): string | null {
  return rollbackBeforeOf(target, retrying) ? '将先回滚上次改动' : null;
}

// 待核对节点被勾回本次派发时（R1-8）：单写者的那一路要带 rollbackBefore，否则
// 会在上次留下的半截改动上再叠一遍；取消勾选时撤掉。
export function toggleRollbackOnReview(
  current: ReadonlySet<string>,
  target: Target | undefined,
  nodeId: string,
  checked: boolean,
): Set<string> {
  const next = new Set(current);
  if (!checked) next.delete(nodeId);
  else if (rollbackBeforeOf(target, true)) next.add(nodeId);
  return next;
}

// G1 提示：项目目录有未提交改动时说一句——worktree 节点从 HEAD 新建，看不到
// 这些改动。git 状态读不出来（未接通 / 不是仓库）时不显示；本批没有 worktree
// 节点时也不显示（R1-13，这句只与 worktree 有关）。
// 形状取自 tauri.ts 的 GitChanges（R1-6），不再手写副本。
type GitChangesShape =
  Pick<GitChanges, 'state'>
  & Partial<Pick<Extract<GitChanges, { state: 'ok' }>, 'uncommitted' | 'untracked'>>;

export function dirtyWorktreeHintOf(
  changes: GitChangesShape | null | undefined,
  hasWorktreeNode: boolean,
): string | null {
  if (!hasWorktreeNode || !changes || changes.state !== 'ok') return null;
  const count = (changes.uncommitted?.length ?? 0) + (changes.untracked?.length ?? 0);
  return count > 0 ? `项目目录有 ${count} 处未提交改动；worktree 从 HEAD 新建，看不到这些改动` : null;
}

// 禁外部动作（R1-12）：按「可写」挂，不挂在角色模板上——自定义角色的可写节点
// 同样要带这句。fixtures/prompts/execution.txt 里已有同一句时不重复追加。
export const NO_EXTERNAL_ACTIONS = '不得 git push、发布、安装依赖或调用付费接口；这些留给用户在验收之后做。';

export function promptWithWriteGuard(prompt: string, write: string): string {
  if (write === '只读' || prompt.includes(NO_EXTERNAL_ACTIONS)) return prompt;
  return `${prompt}\n\n${NO_EXTERNAL_ACTIONS}`;
}

// G1 请求体里单个节点的装配（R1-5）：从 DispatchOverlay.buildRequest 提出来，
// rollbackBefore / fallback / 写护栏这些契约字段才能按序列化后的形状断言。
// 上下文（prompt / materials / 会话）由调用方算好传进来，这里不碰 IO。
export interface NodeRequestParts {
  models: DispatchableModel[];
  prompt: string; // serializeContext 的正文；角色模板与写护栏在这里加
  materials: string[];
  currentMessage: string;
  originalRequest: string;
  upstreamArtifacts: ReplyArtifacts[];
  sessionId: string;
  previousRunner: string;
  rollback: boolean; // 本节点这一轮是重派 / 从待核对勾回
  stage: number;
}

export function buildNodeRequest(
  nodeId: string,
  target: Target,
  parts: NodeRequestParts,
): DispatchNodeRequest {
  const model = parts.models.find((item) =>
    item.agent === target.agent && item.id === target.model &&
    (item.provider ?? '') === (target.provider ?? ''));
  return {
    nodeId,
    stage: parts.stage,
    agent: target.agent,
    provider: target.provider ?? '',
    model: target.model,
    vendor: model?.vendor ?? '',
    billingSource: model?.billing_source ?? '',
    role: target.role,
    tier: target.tier ?? model?.tier ?? '',
    thinking: target.thinking,
    visibility: target.visibility,
    prompt: promptWithWriteGuard(promptForRole(target.role, parts.prompt), target.write),
    currentMessage: parts.currentMessage,
    materials: parts.materials,
    sessionId: parts.sessionId,
    previousRunner: parts.previousRunner,
    write: target.write,
    originalRequest: parts.originalRequest,
    upstreamArtifacts: parts.upstreamArtifacts,
    // TASK-05B 主备/熔断：备选随请求下发（带真实 vendor）
    fallback: fallbackSpecOf(target, parts.models),
    // TASK-06-2：单写者重派先回滚上次改动；只读与 worktree 节点不带这个字段
    ...(rollbackBeforeOf(target, parts.rollback) ? { rollbackBefore: true } : {}),
  };
}

// 推进幂等（backlog 10 前端半边）：同一批结果送达两次只推进一层。
// 批次 id = run + 阶段 + 本批节点集合（排序后），与送达顺序无关。
export function dispatchBatchKey(
  runId: string,
  stage: number,
  results: Pick<BackendNodeResult, 'nodeId'>[],
): string {
  const ids = [...new Set(results.map((result) => result.nodeId))].sort();
  return `${runId}#${stage}#${ids.join(',')}`;
}

export function markBatchProcessed(
  seen: ReadonlySet<string>,
  key: string,
): { seen: Set<string>; fresh: boolean } {
  if (seen.has(key)) return { seen: new Set(seen), fresh: false };
  return { seen: new Set(seen).add(key), fresh: true };
}

// ── TASK-07 疑似空转提示与 06B 收尾 ──────────────────────────────────────

// G2 运行中状态（TASK-07-1）：一条 dispatch-progress 落到界面上的快照。
// 引擎不发的字段按 0 / 空串补齐（旧引擎只发 status）。receivedAt 是本机收到
// 这条心跳的时刻——引擎整个卡死时不会再有新心跳，idle 要用它加上墙上时间
// 才会继续长，否则界面永远停在最后一条心跳的读数上。
// hasHeartbeat 区分「带心跳元数据」与「只有一个状态词」（R1-1）：缺字段补 0
// 在显示上安全，在判定上不安全——只发 status 的旧引擎事件如果也累计墙上时间，
// 任何跑满 3 分钟的节点都会被误判成空转，而且再没有新事件能纠正它。
// runId 用于渲染时过滤上一轮留下的快照（R1-2）。
export interface ProgressSnapshot {
  runId: string;
  status: string;
  elapsedMs: number;
  idleMs: number;
  toolCalls: number;
  repeatedToolCalls: number;
  lastTool: string;
  receivedAt: number;
  hasHeartbeat: boolean;
}

// 无输出多久算「疑似空转」，以及同名工具连续多少次算「疑似空转」。
// 重复工具还要配一个较小的空闲门槛（R1-3）：连续读 3 个文件几秒内就会命中
// 计数，只有「重复且这段时间没有产出」才值得提示。
export const IDLE_HINT_MS = 3 * 60 * 1000;
export const REPEATED_TOOL_LIMIT = 3;
export const REPEATED_TOOL_IDLE_MS = 30 * 1000;

// 终态事件不带心跳字段（TASK-07 契约）：只换状态词，`elapsedMs`/`idleMs`/
// `toolCalls`/`repeatedToolCalls`/`lastTool`/`hasHeartbeat` 与 `receivedAt` 都沿用
// 上一条 running 的快照（R1-1）。补成 0 的话节点一收尾读数就变 0、
// `runningDetailOf` 返回空串，整行消失——那是清空，不是冻结。
export function progressSnapshot(
  payload: DispatchProgress,
  receivedAt: number,
  previous?: ProgressSnapshot,
): ProgressSnapshot {
  const hasHeartbeat = payload.elapsedMs !== undefined || payload.idleMs !== undefined;
  if (!hasHeartbeat && previous && previous.runId === payload.runId) {
    return { ...previous, status: payload.status };
  }
  return {
    runId: payload.runId,
    status: payload.status,
    elapsedMs: payload.elapsedMs ?? 0,
    idleMs: payload.idleMs ?? 0,
    toolCalls: payload.toolCalls ?? 0,
    repeatedToolCalls: payload.repeatedToolCalls ?? 0,
    lastTool: payload.lastTool ?? '',
    receivedAt,
    hasHeartbeat,
  };
}

// 进度 state 的更新（R1-2）：整段逻辑放纯函数里，组件只负责 setState。
// 别轮次的事件直接丢掉——retry() 复用同一个 nodeId，不过滤的话上一轮的读数
// 会落到新一轮的行上。
export function nextProgress(
  current: Record<string, ProgressSnapshot>,
  payload: DispatchProgress,
  now: number,
  runId: string,
): Record<string, ProgressSnapshot> {
  if (payload.runId !== runId) return current;
  return { ...current, [payload.nodeId]: progressSnapshot(payload, now, current[payload.nodeId]) };
}

// 渲染时取快照（R1-2）：state 以 nodeId 为键、跨轮存活，上一轮没收到终态的
// 快照（叫停、抛错、丢事件）不能直接拿来渲染新一轮的行。
export function progressOf(
  progress: Record<string, ProgressSnapshot>,
  nodeId: string,
  runId: string,
): ProgressSnapshot | undefined {
  const snapshot = progress[nodeId];
  return snapshot && snapshot.runId === runId ? snapshot : undefined;
}

// 心跳的 status 是 "running"，界面上还是既有的那个词；别的状态原样显示。
export function progressStatusLabel(status: string): string {
  return status === 'running' ? '运行中' : status;
}

// 节点还在跑（心跳或「运行中」）才算得上空转；排队、等待写锁与终态不算。
export function progressRunning(progress: ProgressSnapshot): boolean {
  return progress.status === 'running' || progress.status === '运行中';
}

// 本轮还有没有节点在跑（TASK-07C）：共享层的 5s 时钟只在这时候走——全部到
// 终态之后读数已经冻结，再 tick 只是白重渲染整张图。
export function progressRunningAny(progress: Record<string, ProgressSnapshot>): boolean {
  return Object.values(progress).some(progressRunning);
}

function idleMsOf(progress: ProgressSnapshot, now: number): number {
  return progress.idleMs + Math.max(0, now - progress.receivedAt);
}

// G2「疑似空转」判定（TASK-07-1）：只提示不阻止，也不下结论说它坏了。
// 同名工具连续 3 次且 30 秒无输出，或 3 分钟没有任何输出/通知，就在节点旁挂一句。
// 没有心跳元数据的快照（旧引擎只发状态词）一律不判——那样的事件永远不会被
// 下一条心跳纠正，累计墙上时间就是必然误报（R1-1）。
export function idleHint(progress: ProgressSnapshot | undefined, now: number): string | null {
  if (!progress || !progressRunning(progress) || !progress.hasHeartbeat) return null;
  const idle = idleMsOf(progress, now);
  if (progress.repeatedToolCalls >= REPEATED_TOOL_LIMIT && idle >= REPEATED_TOOL_IDLE_MS) {
    return `疑似空转：同一工具连续 ${progress.repeatedToolCalls} 次`;
  }
  if (idle >= IDLE_HINT_MS) return `疑似空转：${Math.floor(idle / 60_000)} 分钟无输出`;
  return null;
}

function elapsedLabel(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const minutes = Math.floor(total / 60);
  return minutes > 0 ? `${minutes} 分 ${total % 60} 秒` : `${total} 秒`;
}

// G2 运行中那一行的细节：已运行多久、最后一次调的什么工具。心跳没到之前
// （只有「排队」）返回空串，界面就只显示状态词。节点已经收尾（终态）的行还
// 挂在本层列表里，读数按最后一条心跳冻结，不再跟着墙上时间涨（R1-4）。
export function runningDetailOf(progress: ProgressSnapshot | undefined, now: number): string {
  if (!progress || progress.elapsedMs === 0) return '';
  const wall = progressRunning(progress) ? Math.max(0, now - progress.receivedAt) : 0;
  const elapsed = elapsedLabel(progress.elapsedMs + wall);
  return progress.lastTool ? `已运行 ${elapsed} · 工具 ${progress.lastTool}` : `已运行 ${elapsed}`;
}

// 对账说明随派发收敛（TASK-07-2）：一轮派发之后，已经跑完或已判失败的节点
// 不在待派集合里了，把它们从「待核对」剔除；一个不剩时连工具条那句说明一起
// 清掉——否则说明会一直挂着上一次对账的陈旧计数。换图后同样清空（口径与
// reconcileUiFor 一致）。
// 判据是节点自己的终态，不是「这一刻可不可派」（R1-5）：父节点还没跑完、
// 目标没选、正在飞的节点都暂时不可派，但它仍然没核对过。按可派集合剔除会让
// 工具条的计数与节点卡的「待核对」标签对不上，还会让这些节点在父节点跑完后
// 被 initialDispatchSelection 默认勾上派出去。口径与节点卡一致：只留仍是
// draft 且正文还是 RECONCILE_UNKNOWN_TEXT 的那些。
export function reconcileUiConverged(state: ReconcileUi, graph: Graph): ReconcileUi {
  const current = reconcileUiFor(state, graph.id);
  if (current.needsReview.length === 0) return current;
  const needsReview = current.needsReview.filter((id) => {
    const reply = graph.nodes[id]?.reply;
    return reply?.status === 'draft' && reply.text === RECONCILE_UNKNOWN_TEXT;
  });
  if (needsReview.length === current.needsReview.length) return current;
  if (needsReview.length === 0) return EMPTY_RECONCILE_UI;
  return { ...current, needsReview, note: `还有 ${needsReview.length} 个节点待核对` };
}

// 结果屏 cache_read 列（TASK-07-3）：优先读图上的 tokens_cache_read——对账
// 收回来的那一路只有图字段，没有本次派发的结果行。
export function cacheReadOf(
  reply: Reply | undefined,
  result: Pick<BackendNodeResult, 'cacheRead'>,
): number {
  return reply?.tokens_cache_read ?? result.cacheRead ?? 0;
}

// ── 派发推进循环（TASK-07-4） ────────────────────────────────────────────
// 原来整段写在 DispatchOverlay.start() 里，重复批次那条停止分支只能靠纯函数
// 间接验证。这里把循环体原样搬出来，预览/派发/提交都由调用方注入，整链
// （本层复位 draft + 提示 + 不再推进下一层）才测得到。
// 状态对象由调用方持有并就地更新：派发抛错时组件的 catch 分支还要读
// graph / collected / activeNodes，返回新副本的话那半边就拿不到了。

export interface StageLoopState<R extends BackendNodeResult> {
  graph: Graph;
  collected: R[];
  activeNodes: { nodeId: string }[];
}

export interface StageRequestShape {
  stage: number;
  nodes: { nodeId: string }[];
}

export interface StageLoopDeps<R extends BackendNodeResult, Req extends StageRequestShape, P> {
  runId: string;
  buildRequest: (graph: Graph, nodeIds: string[]) => Req;
  preview: (request: Req) => Promise<P>;
  dispatch: (request: Req) => Promise<{ runId: string; results: R[] }>;
  commit: (graph: Graph) => Graph;
  onPreview: (preview: P) => void;
  onStage: (request: Req, nodeIds: string[]) => void;
  onResults: (collected: R[]) => void;
}

export type StageLoopOutcome =
  | { kind: 'done' }
  | { kind: 'stopped'; error: string }
  | { kind: 'missing-target'; nodeIds: string[]; error: string };

export const DUPLICATE_BATCH_TEXT = '同一批结果重复送达，已停止推进；这一层的节点已放回待派。';
export const MISSING_TARGET_TEXT = '下一阶段有节点未指定模型，请先选择目标。';

export async function runStageLoop<R extends BackendNodeResult, Req extends StageRequestShape, P>(
  state: StageLoopState<R>,
  settings: { maxRequests: number },
  skip: ReadonlySet<string>,
  deps: StageLoopDeps<R, Req, P>,
): Promise<StageLoopOutcome> {
  let validationRetries: Record<string, number> = {};
  let processedBatches: Set<string> = new Set();
  for (;;) {
    const next = nextStageStep(state.graph, state.collected, settings, skip);
    if (next.kind === 'done') return { kind: 'done' };
    if (next.kind === 'missing-target') {
      return { kind: 'missing-target', nodeIds: next.nodeIds, error: MISSING_TARGET_TEXT };
    }
    if (next.kind === 'limit') {
      return { kind: 'stopped', error: `已达总请求上限 ${settings.maxRequests}，继续派发需重新确认。` };
    }
    const ids = next.nodeIds;
    const stageRequest = deps.buildRequest(state.graph, ids);
    deps.onPreview(await deps.preview(stageRequest));
    state.activeNodes = stageRequest.nodes;
    deps.onStage(stageRequest, ids);
    state.graph = deps.commit(markPending(state.graph, ids));
    const output = await deps.dispatch(stageRequest);
    const batch = markBatchProcessed(
      processedBatches,
      dispatchBatchKey(output.runId || deps.runId, stageRequest.stage, output.results),
    );
    processedBatches = batch.seen;
    if (!batch.fresh) {
      // 同一批结果重复送达：本层节点刚 markPending 过，停下前放回 draft，
      // 不能悬挂在 pending 上（06B R1-3）。
      state.graph = deps.commit(resetNodesToDraft(state.graph, ids));
      return { kind: 'stopped', error: DUPLICATE_BATCH_TEXT };
    }
    state.collected.push(...output.results);
    deps.onResults([...state.collected]);
    state.graph = deps.commit(applyDispatchResults(state.graph, output.results));
    const step = reducePipelineStep(state.graph, output.results, validationRetries);
    validationRetries = step.validationRetries;
    if (step.graph !== state.graph) state.graph = deps.commit(step.graph);
    if (!step.continueRunning) return { kind: 'done' };
  }
}
