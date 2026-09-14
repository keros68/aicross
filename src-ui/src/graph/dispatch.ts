import type { DispatchableModel, Graph, GraphNode, Reply, ReplyArtifacts, Target } from './types.ts';
import { newNodeId } from './types.ts';
import acceptancePrompt from '../../../fixtures/prompts/acceptance.txt?raw';
import executionPrompt from '../../../fixtures/prompts/execution.txt?raw';
import planningPrompt from '../../../fixtures/prompts/planning.txt?raw';

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

export function applyDispatchResults(graph: Graph, results: BackendNodeResult[]): Graph {
  const nodes = { ...graph.nodes };
  for (const result of results) {
    const node = nodes[result.nodeId];
    if (!node) continue;
    const ok = result.status === 'ok';
    const validationText = result.status === 'validation_failed' && result.validationOutput
      ? `${result.text}${result.text ? '\n\n' : ''}[验证输出]\n${result.validationOutput}`
      : undefined;
    const reply: Reply = {
      status: ok ? 'done' : 'failed',
      text: validationText ?? (ok ? result.text : result.text || result.error),
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
  return `[aicross] ${summary}${correction ? '→ ' : ' → '}${route} ｜ ${target.tier || model?.tier || '未分档'} · thinking=${target.thinking} ｜ 可见性=${target.visibility} ｜ 写=${target.write} ｜ 隔离=${isolation}${validation}${warning}`;
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
  return prompt;
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
