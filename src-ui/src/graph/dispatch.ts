import type { DispatchableModel, Graph, GraphNode, Preset, Reply, ReplyArtifacts, Target } from './types.ts';
import { newNodeId } from './types.ts';
import { ancestorsOf } from './layout.ts';
import { defaultTurnTarget, parseSend } from './mutate.ts';
import { arbitrationVendorConflict } from './validate.ts';
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

export function nextStageStep(
  graph: Graph,
  collected: Pick<BackendNodeResult, 'nodeId'>[],
  settings: { maxRequests: number },
): NextStageStep {
  const nodeIds = dispatchableNodeIds(graph);
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
  // R2-1：父回复是备选作答（首行「已切备选」标记）时，session 属于备选通道，
  // 不能续到本节点的主通道上——发完整 prompt，不续聊。
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

// 「已切备选」在回复正文里的标记行。Reply 契约本期不加字段，节点卡靠这行识别
// （fallbackNoticeOf 提取；正文其余部分不变，下游上下文仍可用）。
export function fallbackNoticeOf(reply?: Reply): string | null {
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
    const notice = result.fallbackFrom
      ? `（已切备选：主 ${result.fallbackFrom} → 备 ${result.fallbackTo}）\n\n`
      : '';
    const reply: Reply = {
      status: ok ? 'done' : 'failed',
      text: `${notice}${body}`,
      ...(ok ? {} : { error_kind: result.status }),
      model_reported: result.modelReported || undefined,
      tokens_in: result.tokensIn || undefined,
      tokens_out: result.tokensOut || undefined,
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
