import { commands, onConsoleAction, onConsoleDispatch, onConsolePropose, type ConsoleActionEvent, type ConsoleDispatchEvent, type ConsoleProposal, type ConsoleProposalEvent, type ConsoleRun, type ConsoleRunNode, type DispatchNodeResult, type DispatchStageRequest } from './tauri';
import type { AppState, TerminalSession } from './store/app-state';
import type { DispatchableModel, Graph, GraphNode, Preset } from './graph/types';
import { newGraph, newNodeId } from './graph/types';
import { latestLeaf } from './graph/layout';
import { fillRoot } from './graph/mutate';
import { stamp } from './graph/stamp';
import { loadDispatchSettings } from './graph/dispatch-settings';
import { runRequestOf, serializeContext, serializeCurrentMessage, upstreamArtifactsOf } from './graph/context';
import { buildNodeRequest, continuationRunner, continuationSession, dirtyWorktreeHintOf, dispatchableNodeIds, failedNodesWithoutResults, graphStage, isSmallEconomyTask, leftoverNodes, presetWithRoleDefaults, quotaBlockedNodeIds, setReplyAdopted, startPipelineHeadless } from './graph/dispatch';
import { validateNode } from './graph/validate';

export type ConsoleTabPlan =
  | { kind: 'focus'; sessionId: string }
  | { kind: 'background' }
  | { kind: 'open' };

export function consoleTabPlan(sessions: TerminalSession[], activeId: string | null, graphId: string): ConsoleTabPlan {
  const existing = sessions.find((session) => session.tool === 'graph' && session.toolData === graphId);
  const active = sessions.find((session) => session.id === activeId);
  const consoleActive = active?.tool === 'console' || Boolean(active?.commanderTitle);
  if (existing) return consoleActive ? { kind: 'background' } : { kind: 'focus', sessionId: existing.id };
  return consoleActive ? { kind: 'background' } : { kind: 'open' };
}

export function consoleInflightCount(runs: ConsoleRun[]): number {
  return runs.filter((run) => run.status === 'running')
    .reduce((count, run) => count + (run.in_flight_node_ids?.length ?? run.nodes.filter((node) => node.status === 'running').length), 0);
}

// Windows 扩展长度前缀要先剥掉（09-16 真机）：Rust 侧 canonicalize 给出的是
// `\\?\C:\...`，而标签页的目录是 `C:\...`，不剥就永远认不出用户已开的那张图。
export const normalizedPath = (value: string | null | undefined) => (value ?? '')
  .replace(/^\\\\\?\\UNC\\/i, '//')
  .replace(/^\\\\\?\\/, '')
  .replace(/\\/g, '/')
  .replace(/\/+$/, '')
  .toLocaleLowerCase();

export function selectConsoleGraph(state: Pick<AppState, 'graphs' | 'terminals' | 'activeTerminalId'>, project: string): Graph | undefined {
  const wanted = normalizedPath(project);
  const tabs = state.terminals.filter((tab) => tab.tool === 'graph' && normalizedPath(tab.folderPath) === wanted);
  const active = tabs.find((tab) => tab.id === state.activeTerminalId);
  const ordered = active ? [active, ...tabs.filter((tab) => tab !== active)] : tabs;
  for (const tab of ordered) {
    const graph = tab.toolData ? state.graphs[tab.toolData] : undefined;
    if (graph && (!graph.project || normalizedPath(graph.project) === wanted)) return graph;
  }
  return Object.values(state.graphs).filter((graph) => normalizedPath(graph.project) === wanted)
    .sort((a, b) => b.updated_at.localeCompare(a.updated_at))[0];
}

export function proposalDisplayGraphId(state: Pick<AppState, 'graphs' | 'terminals' | 'activeTerminalId'>, project: string, requested = ''): string {
  if (requested) return requested;
  const wanted = normalizedPath(project);
  const matching = state.terminals.find((tab) => {
    if (tab.tool !== 'graph' || !tab.toolData) return false;
    const graph = state.graphs[tab.toolData];
    return normalizedPath(tab.folderPath) === wanted || normalizedPath(graph?.project) === wanted;
  });
  if (matching?.toolData) return matching.toolData;
  const active = state.terminals.find((tab) => tab.id === state.activeTerminalId);
  return active?.tool === 'graph' ? (active.toolData ?? '') : '';
}

export function appendConsoleMaterial(graph: Graph, text: string, attachments: string[]): { graph: Graph; nodeId: string } {
  const root = graph.nodes[graph.root];
  if (root && root.user.text === '' && root.parents.length === 0) {
    const filled = fillRoot(graph, text);
    return { graph: { ...filled, nodes: { ...filled.nodes, [filled.root]: { ...filled.nodes[filled.root], user: { text, attachments }, source: 'console' } } }, nodeId: graph.root };
  }
  const nodeId = newNodeId();
  const node: GraphNode = { id: nodeId, kind: 'material', parents: [latestLeaf(graph)], user: { text, attachments }, created_at: new Date().toISOString(), source: 'console' };
  return { graph: { ...graph, nodes: { ...graph.nodes, [nodeId]: node } }, nodeId };
}

function consolePreset(event: ConsoleDispatchEvent, presets: Preset[]): Preset | undefined {
  if (event.targets?.length) return { id: `console-${event.run_id}`, name: '', builtin: false, stages: [{ nodes: event.targets }] };
  return presets.find((preset) => preset.id === (event.preset ?? 'builtin-economy'));
}

function nodeSummary(graph: Graph, ids: Iterable<string>): ConsoleRunNode[] {
  return [...new Set(ids)].map((id) => graph.nodes[id]).filter((node): node is GraphNode => Boolean(node?.target)).map((node) => ({
    node_id: node.id,
    role: node.target!.role,
    agent: node.target!.agent,
    model: node.target!.model,
    status: node.reply?.status ?? 'draft',
    text_excerpt: (node.reply?.text || node.reply?.skipped_reason || '').slice(0, 160),
    artifacts_dir: node.reply?.artifacts?.worktree ?? '',
    quota_resets_at: node.reply?.quota_resets_at,
  }));
}

export interface ConsoleChannelContext {
  getState: () => AppState;
  dispatch: (action: any) => void;
}

type ChannelCommands = Pick<typeof commands, 'listDispatchableModels' | 'listPresets' | 'loadRoles' | 'gitChanges' | 'previewDispatch' | 'dispatchGraphStage' | 'saveGraph' | 'consoleRegisterProposal' | 'consoleRunUpdate' | 'listConsoleRuns' | 'cancelDispatchNode' | 'consoleActionAck'>;

interface ProposalRuntime extends ConsoleProposal {
  text: string;
  attachments: string[];
  targets?: ConsoleDispatchEvent['targets'];
  customTargets?: ConsoleDispatchEvent['targets'];
  local: boolean;
  presets: Preset[];
}

interface LocalProposalInput {
  project: string;
  graphId?: string;
  text: string;
  attachments?: string[];
}

type ProposalInputEvent = ConsoleProposalEvent & { graph_id?: string; local?: boolean };

export interface ConsoleProposalNotice {
  id: string;
  project: string;
  graph_id: string;
  text: string;
}

let proposalContext: ConsoleChannelContext | null = null;
let proposalItems: ProposalRuntime[] = [];
let proposalSnapshot: ConsoleProposal[] = [];
let proposalNoticeSnapshot: ConsoleProposalNotice[] = [];
const proposalListeners = new Set<() => void>();

function publishProposals(next: ProposalRuntime[]): void {
  proposalItems = next;
  proposalSnapshot = next.map(({ presets: _presets, text: _text, attachments: _attachments, targets: _targets, customTargets: _customTargets, local: _local, ...proposal }) => proposal);
  proposalListeners.forEach((listener) => listener());
}

function mergeProposal(proposal: ProposalRuntime): void {
  proposalNoticeSnapshot = proposalNoticeSnapshot.filter((notice) =>
    notice.graph_id !== proposal.graph_id && normalizedPath(notice.project) !== normalizedPath(proposal.project));
  publishProposals([proposal, ...proposalItems.filter((item) => item.proposal_id !== proposal.proposal_id)]);
}

function proposalNotice(project: string, graphId: string, text: string): void {
  proposalNoticeSnapshot = [
    { id: crypto.randomUUID(), project, graph_id: graphId, text },
    ...proposalNoticeSnapshot.filter((notice) => notice.graph_id !== graphId || normalizedPath(notice.project) !== normalizedPath(project)),
  ];
  proposalListeners.forEach((listener) => listener());
}

export function clearProposalNotice(id: string): void {
  proposalNoticeSnapshot = proposalNoticeSnapshot.filter((notice) => notice.id !== id);
  proposalListeners.forEach((listener) => listener());
}

export function subscribeConsoleProposals(listener: () => void): () => void {
  proposalListeners.add(listener);
  return () => proposalListeners.delete(listener);
}

export function getConsoleProposals(): ConsoleProposal[] { return proposalSnapshot; }
export function getConsoleProposalNotices(): ConsoleProposalNotice[] { return proposalNoticeSnapshot; }

export function proposalPresets(proposalId: string): Preset[] {
  return proposalItems.find((item) => item.proposal_id === proposalId)?.presets ?? [];
}

export function updateProposalPreset(proposalId: string, presetId: string): void {
  const next = proposalItems.map((item) => {
    if (item.proposal_id !== proposalId) return item;
    const preset = item.presets.find((value) => value.id === presetId);
    return preset ? {
      ...item,
      preset_id: preset.id,
      node_count: preset.stages.reduce((count, stage) => count + stage.nodes.length, 0),
      targets: preset.id === `console-${item.proposal_id}` ? item.customTargets : undefined,
    } : item;
  });
  publishProposals(next);
}

export function proposalsForTab(proposals: ConsoleProposal[], project: string, graphId = '', commander = false): ConsoleProposal[] {
  const wanted = normalizedPath(project);
  const matching = proposals.filter((proposal) =>
    (graphId !== '' && proposal.graph_id === graphId) || (wanted !== '' && normalizedPath(proposal.project) === wanted));
  return matching.length > 0 || !commander ? matching : proposals;
}

// 兜底显示（目标项目没有打开的 tab，或控制台条代显）时，条上要带出目标目录，
// 否则项目 X 的建议画在项目 Y 的 tab 上，用户无从分辨就采纳了（09-16 复审 GLM 4）。
export function proposalIsForeign(proposal: ConsoleProposal, project: string, graphId = ''): boolean {
  if (graphId !== '' && proposal.graph_id === graphId) return false;
  const wanted = normalizedPath(project);
  return wanted === '' || normalizedPath(proposal.project) !== wanted;
}

export function proposalForTab(proposals: ConsoleProposal[], project: string, graphId = '', commander = false): ConsoleProposal | undefined {
  return proposalsForTab(proposals, project, graphId, commander)[0];
}

export function proposalNoticeForTab(notices: ConsoleProposalNotice[], project: string, graphId = '', commander = false): ConsoleProposalNotice | undefined {
  const wanted = normalizedPath(project);
  return notices.find((notice) =>
    (graphId !== '' && notice.graph_id === graphId) || (wanted !== '' && normalizedPath(notice.project) === wanted))
    ?? (commander ? notices[0] : undefined);
}

export function buildLocalProposalEvent(input: LocalProposalInput, idFactory: () => string = () => crypto.randomUUID()): ProposalInputEvent {
  return {
    proposal_id: idFactory(), project: input.project, graph_id: input.graphId ?? '', text: input.text,
    attachments: input.attachments ?? [], preset: 'builtin-economy', reason: '', local: true,
  };
}

export async function proposalDecision(
  text: string,
  attachments: string[],
  useClassifier: boolean,
  classify: () => Promise<boolean>,
): Promise<{ propose: true } | { propose: false; skipped: string }> {
  if (isSmallEconomyTask(text, attachments)) return { propose: false, skipped: '单步小任务' };
  if (!useClassifier) return { propose: true };
  try {
    return await classify() ? { propose: true } : { propose: false, skipped: '归类模型判定不必派发' };
  } catch (error) {
    const reason = String(error).replace(/^Error:\s*/, '');
    return { propose: false, skipped: `分类失败，已按不派处理${reason ? `：${reason}` : ''}` };
  }
}

export function classificationTarget(value: string, models: DispatchableModel[]): DispatchableModel | undefined {
  if (value) {
    try {
      const [agent, provider, model] = JSON.parse(value) as string[];
      const selected = models.find((item) => item.agent === agent && item.provider === provider && item.id === model);
      if (selected) return selected;
    } catch {}
    throw new Error('所选归类模型当前不可用');
  }
  return models.find((model) => /low|haiku|flash|mini/i.test(`${model.tier} ${model.id}`)) ?? models[0];
}

export function proposalClassificationVerdict(text: string): boolean {
  return text.match(/\b(DISPATCH|SINGLE)\b/i)?.[1].toUpperCase() === 'DISPATCH';
}

export async function classifyProposal(event: ProposalInputEvent, settings: ReturnType<typeof loadDispatchSettings>, io: ChannelCommands): Promise<boolean> {
  const resources = await io.listDispatchableModels();
  const model = classificationTarget(settings.classificationModel, resources.models);
  if (!model) throw new Error('没有可用的归类模型');
  const prompt = `只判断下面任务是否值得交给多个模型或多个阶段并行处理。只输出 DISPATCH 或 SINGLE。\n\n任务：${event.text}`;
  const classifyId = `proposal-classify-${event.proposal_id}`;
  const result = await io.dispatchGraphStage({
    runId: classifyId,
    stage: 1,
    projectDir: event.project,
    settings: { timeoutSecs: Math.min(settings.timeoutSecs, 20), concurrency: 1, maxRequests: 1, redact: settings.redact, writeProjectDispatch: false, ccSwitchPath: settings.ccSwitchPath, validationCommand: '', acpEnabled: false, fixRoundLimit: 0 },
    nodes: [{
      nodeId: classifyId, stage: 1, agent: model.agent, provider: model.provider, model: model.id,
      vendor: model.vendor, billingSource: model.billing_source, role: '顾问', tier: model.tier, thinking: model.thinking,
      visibility: '共享', prompt, currentMessage: event.text, materials: event.attachments ?? [], sessionId: '', previousRunner: '', write: '只读',
      originalRequest: event.text, upstreamArtifacts: [],
    }],
  });
  const answer = result.results[0];
  if (answer?.status !== 'ok') throw new Error(answer?.text || '归类调用失败');
  return proposalClassificationVerdict(answer.text);
}

export async function runConsoleProposal(event: ProposalInputEvent, context: ConsoleChannelContext, io: ChannelCommands = commands): Promise<void> {
  const settings = loadDispatchSettings();
  const decision = await proposalDecision(event.text, event.attachments ?? [], settings.classifyProposals, () => classifyProposal(event, settings, io));
  if (!decision.propose) {
    if (!event.local) await io.consoleRunUpdate({ run_id: event.proposal_id, graph_id: '', status: 'skipped', nodes: [], leftover: [], in_flight_node_ids: [], reason: decision.skipped });
    else proposalNotice(event.project, event.graph_id ?? '', decision.skipped);
    return;
  }
  const [presetFile, roles] = await Promise.all([io.listPresets(), io.loadRoles()]);
  const selected = event.targets?.length
    ? { id: `console-${event.proposal_id}`, name: '', builtin: false, stages: [{ nodes: event.targets }] }
    : presetFile.presets.find((preset) => preset.id === (event.preset ?? 'builtin-economy'));
  if (!selected) {
    if (!event.local) await io.consoleRunUpdate({ run_id: event.proposal_id, graph_id: '', status: 'rejected', nodes: [], leftover: [], in_flight_node_ids: [], reason: `找不到预设：${event.preset ?? 'builtin-economy'}` });
    return;
  }
  const preset = presetWithRoleDefaults(selected, roles);
  const graphId = proposalDisplayGraphId(context.getState(), event.project, event.graph_id);
  const proposal: ProposalRuntime = {
    proposal_id: event.proposal_id, project: event.project, graph_id: graphId, preset_id: preset.id,
    node_count: preset.stages.reduce((count, stage) => count + stage.nodes.length, 0), reason: event.reason ?? '', created_at: new Date().toISOString(),
    text: event.text, attachments: event.attachments ?? [], targets: event.targets, customTargets: event.targets, local: event.local === true,
    presets: event.targets?.length ? [preset, ...presetFile.presets] : presetFile.presets.map((value) => presetWithRoleDefaults(value, roles)),
  };
  if (event.local) {
    await io.consoleRegisterProposal(proposal.proposal_id, proposal.project, proposal.graph_id, proposal.reason);
  } else {
    await io.consoleRunUpdate({ run_id: event.proposal_id, graph_id: graphId, status: 'proposed', nodes: [], leftover: [], in_flight_node_ids: [], reason: proposal.reason });
  }
  mergeProposal(proposal);
}

export async function requestLocalProposal(input: LocalProposalInput): Promise<void> {
  if (!proposalContext || input.text.trim() === '') return;
  try {
    await runConsoleProposal(buildLocalProposalEvent(input), proposalContext);
  } catch (error) {
    proposalNotice(input.project, input.graphId ?? '', `建议生成失败：${String(error).replace(/^Error:\s*/, '')}`);
  }
}

export async function acceptConsoleProposal(proposalId: string, io: ChannelCommands = commands, contextOverride: ConsoleChannelContext | null = proposalContext): Promise<void> {
  const proposal = proposalItems.find((item) => item.proposal_id === proposalId);
  const context = contextOverride;
  if (!proposal || !context) return;
  try {
    const limit = loadDispatchSettings().consoleRunLimit;
    const activeRuns = await io.listConsoleRuns();
    if (activeRuns.filter((run) => run.status === 'running' && run.run_id !== proposal.proposal_id).length >= limit) {
      throw new Error(`同时在飞 run 已达上限 ${limit}`);
    }
    await io.consoleRunUpdate({ run_id: proposal.proposal_id, graph_id: proposal.graph_id, status: 'running', nodes: [], leftover: [], in_flight_node_ids: [], reason: proposal.reason });
    publishProposals(proposalItems.filter((item) => item.proposal_id !== proposalId));
    const event: ConsoleDispatchEvent = {
      run_id: proposal.proposal_id, project: proposal.project, text: proposal.text, preset: proposal.preset_id,
      attachments: proposal.attachments, targets: proposal.targets, reason: proposal.reason,
    };
    await runConsoleDispatch(event, context, io);
  } catch (error) {
    publishProposals(proposalItems.filter((item) => item.proposal_id !== proposalId));
    const reason = String(error).replace(/^Error:\s*/, '');
    proposalNotice(proposal.project, proposal.graph_id, reason.includes('同时在飞 run 已达上限') ? '在飞已满，建议已撤回' : `采纳失败：${reason}`);
  }
}

export async function dismissConsoleProposal(proposalId: string, io: Pick<ChannelCommands, 'consoleRunUpdate'> = commands): Promise<void> {
  const proposal = proposalItems.find((item) => item.proposal_id === proposalId);
  if (!proposal) return;
  try {
    await io.consoleRunUpdate({ run_id: proposal.proposal_id, graph_id: proposal.graph_id, status: 'dismissed', nodes: [], leftover: [], in_flight_node_ids: [], reason: proposal.reason });
  } catch (error) {
    const reason = String(error).replace(/^Error:\s*/, '');
    proposalNotice(proposal.project, proposal.graph_id, `忽略失败：${reason}`);
  }
  publishProposals(proposalItems.filter((item) => item.proposal_id !== proposalId));
}

// 把流水线这次落图（previous → next 的差）合到 store 里的最新图 latest 上：
// 只带走本次改动或新增的节点，其余节点以 latest 为准（用户的并发编辑保留）。纯函数，可测。
export function mergeConsoleCommit(latest: Graph, previous: Graph, next: Graph): Graph {
  const changed = Object.keys(next.nodes).filter((id) => JSON.stringify(next.nodes[id]) !== JSON.stringify(previous.nodes[id]));
  const nodes = { ...latest.nodes, ...Object.fromEntries(changed.map((id) => [id, next.nodes[id]])) };
  return { ...latest, root: next.root, nodes };
}

export async function runConsoleDispatch(event: ConsoleDispatchEvent, context: ConsoleChannelContext, io: ChannelCommands = commands): Promise<void> {
  const settings = loadDispatchSettings();
  const [resources, presetFile, roles, activeRuns] = await Promise.all([io.listDispatchableModels(), io.listPresets(), io.loadRoles(), io.listConsoleRuns()]);
  const reject = async (reason: string, graphId = '') => io.consoleRunUpdate({ run_id: event.run_id, graph_id: graphId, status: 'rejected', nodes: [], leftover: [], in_flight_node_ids: [], reason });
  if (activeRuns.filter((run) => run.status === 'running' && run.run_id !== event.run_id).length >= settings.consoleRunLimit) {
    await reject(`同时在飞 run 已达上限 ${settings.consoleRunLimit}`);
    return;
  }
  if (resources.error) { await reject(`读取资源失败：${resources.error}`); return; }
  const selected = consolePreset(event, presetFile.presets);
  if (!selected) { await reject(`找不到预设：${event.preset ?? 'builtin-economy'}`); return; }
  const preset = presetWithRoleDefaults(selected, roles);
  const existing = selectConsoleGraph(context.getState(), event.project);
  const base = existing ?? newGraph(event.project);
  // R1 复审 N3：只有新建的图标「来自控制台」，续写用户自己的图保留原来源。
  const graphSource = existing ? existing.source : ('console' as const);
  const material = appendConsoleMaterial({ ...base, project: event.project, source: graphSource }, event.text, event.attachments ?? []);
  const runRequest = runRequestOf(material.graph, material.nodeId);
  let graph = stamp(material.graph, material.nodeId, preset, settings.swarmLimit);
  const before = new Set(Object.keys(material.graph.nodes));
  const created = Object.keys(graph.nodes).filter((id) => !before.has(id));
  graph = { ...graph, nodes: Object.fromEntries(Object.entries(graph.nodes).map(([id, node]) => [id, created.includes(id) ? { ...node, source: 'console' as const } : node])) };
  const allRunIds = new Set(created);
  const first = dispatchableNodeIds(graph).filter((id) => created.includes(id));
  const missing = first.filter((id) => !graph.nodes[id].target?.agent.trim() || !graph.nodes[id].target?.model.trim());
  if (missing.length) { await reject(`角色「${graph.nodes[missing[0]].target?.role ?? ''}」没有可派发目标`, graph.id); return; }
  const blockingHint = first.flatMap((id) => validateNode(graph, id, resources.models)).find((hint) => hint === '未接入' || hint.includes('只能只读'));
  if (blockingHint) { await reject(blockingHint, graph.id); return; }
  const failedVerification = first.find((id) => {
    const target = graph.nodes[id].target!;
    return resources.models.find((model) => model.agent === target.agent && model.id === target.model)?.verification === 'failed';
  });
  if (failedVerification) { const target = graph.nodes[failedVerification].target!; await reject(`目标验证失败：${target.agent}/${target.model}`, graph.id); return; }
  const quota = quotaBlockedNodeIds(graph, first, resources.models);
  if (quota.length) {
    const target = graph.nodes[quota[0]].target!;
    const reset = resources.models.find((model) => model.agent === target.agent && model.id === target.model)?.quota_exhausted_until;
    await reject(`目标额度用尽：${target.agent}/${target.model}${reset ? `；恢复时间 ${reset}` : ''}`, graph.id);
    return;
  }
  if (created.length > settings.maxRequests) { await reject(`本次派发 ${created.length} 路，超过总请求上限 ${settings.maxRequests}`, graph.id); return; }
  const dirty = await io.gitChanges(event.project).catch(() => null);
  const dirtyReason = dirtyWorktreeHintOf(dirty, first.some((id) => graph.nodes[id].target?.write === 'worktree'));
  if (dirtyReason) { await reject(dirtyReason, graph.id); return; }

  // R1 复审 1：流水线持有的是自己的快照，每次落图先合到 store 里的最新图上，
  // 只覆盖本次真正改动的节点——运行期间用户在同一张图上的编辑不会被整图回写抹掉。
  let committed: Graph = graph;
  // R1 复审 N1：在飞清单在阶段开始时记下、结果落图后清空；每次落图回报都带着它，
  // 否则执行期间摘要里的在飞恒为空，`aicross cancel` 与「N 路在飞」都不工作。
  let inFlight: string[] = [];
  const commit = (next: Graph) => {
    const latest = context.getState().graphs[graph.id] ?? committed;
    graph = { ...mergeConsoleCommit(latest, committed, next), project: event.project, source: graphSource };
    graph = { ...graph, updated_at: new Date().toISOString() };
    committed = graph;
    context.dispatch({ type: 'SET_GRAPH', graph });
    void io.saveGraph(graph)
      .then(() => io.consoleRunUpdate({ run_id: event.run_id, graph_id: graph.id, status: 'running', nodes: nodeSummary(graph, allRunIds), leftover: leftoverNodes(graph).flatMap((node) => node.leftover?.items ?? []), in_flight_node_ids: inFlight, reason: '' }))
      .catch((error) => console.warn('[graph] 保存失败', error));
    return graph;
  };
  // R1 复审 N2：准备阶段超过 5 秒时 Rust 已把 run 判成 rejected 并给 CLI 回了 503，
  // 这时不能再起流水线（模型会真跑而 skill 侧已按失败回退）。
  const current = (await io.listConsoleRuns()).find((run) => run.run_id === event.run_id);
  if (current && current.status !== 'running') return;
  graph = { ...graph, updated_at: new Date().toISOString() };
  context.dispatch({ type: 'SET_GRAPH', graph });
  await io.saveGraph(graph);
  await io.consoleRunUpdate({ run_id: event.run_id, graph_id: graph.id, status: 'running', nodes: nodeSummary(graph, allRunIds), leftover: [], in_flight_node_ids: [], reason: '' });

  const buildRequest = (sourceGraph: Graph, ids: string[]): DispatchStageRequest => ({
    runId: event.run_id,
    stage: ids.length ? Math.min(...ids.map((id) => graphStage(sourceGraph, id))) : 1,
    projectDir: event.project,
    settings: { timeoutSecs: settings.timeoutSecs, concurrency: settings.concurrency, maxRequests: settings.maxRequests, redact: settings.redact, writeProjectDispatch: settings.writeProjectDispatch, ccSwitchPath: settings.ccSwitchPath, validationCommand: settings.validationCommand, acpEnabled: settings.acpEnabled, fixRoundLimit: settings.fixRoundLimit },
    nodes: ids.map((id) => {
      const node = sourceGraph.nodes[id];
      const selected = serializeContext(sourceGraph, id);
      return buildNodeRequest(id, node.target!, { models: resources.models, prompt: selected.prompt, materials: selected.materials, currentMessage: serializeCurrentMessage(node.user), originalRequest: runRequest, upstreamArtifacts: upstreamArtifactsOf(sourceGraph, id), sessionId: continuationSession(sourceGraph, id) ?? '', previousRunner: continuationRunner(sourceGraph, id) ?? '', rollback: false, stage: graphStage(sourceGraph, id) });
    }),
  });

  const pipeline = await startPipelineHeadless<DispatchNodeResult, DispatchStageRequest, unknown>({
    graph,
    settings,
    skip: new Set(Object.keys(graph.nodes).filter((id) => graph.nodes[id].target && !created.includes(id))),
    deps: {
      runId: event.run_id,
      buildRequest,
      preview: (request) => io.previewDispatch(request),
      dispatch: (request) => io.dispatchGraphStage(request),
      commit,
      onPreview: () => {},
      onStage: (request) => {
        request.nodes.forEach((node) => allRunIds.add(node.nodeId));
        inFlight = request.nodes.map((node) => node.nodeId);
        void io.consoleRunUpdate({ run_id: event.run_id, graph_id: graph.id, status: 'running', nodes: nodeSummary(graph, allRunIds), leftover: [], in_flight_node_ids: inFlight, reason: '' });
      },
      onResults: (results) => { results.forEach((result) => allRunIds.add(result.nodeId)); inFlight = []; },
    },
    onError: (active, collected, message, working) => failedNodesWithoutResults(active, collected, message).map((node) => ({ ...node, stage: graphStage(working, node.nodeId), rawStdout: '', rawStderr: '', cacheRead: 0, exitCode: null, timedOut: false, truncated: false, redacted: 0, isolation: '部分', runner: 'unknown', workdir: '', dispatchFile: '', frontmatter: '', outputPreview: '', validationOutput: '', artifacts: null, fallbackFrom: '', fallbackTo: '', fallbackSkipped: '' })),
  });
  graph = pipeline.graph;
  const statuses = nodeSummary(graph, allRunIds).map((node) => node.status);
  const status = pipeline.error || statuses.includes('failed') ? 'failed' : 'done';
  await io.saveGraph(graph);
  await io.consoleRunUpdate({ run_id: event.run_id, graph_id: graph.id, status, nodes: nodeSummary(graph, allRunIds), leftover: leftoverNodes(graph).flatMap((node) => node.leftover?.items ?? []), in_flight_node_ids: [], reason: pipeline.error });
}

// 取消的目标 = 摘要里的在飞节点 ∪ 图上仍 pending 的节点（R1 复审 N1）：
// 摘要可能落后一拍，图上的 pending 才是引擎正在跑的。纯函数，可测。
export function consoleCancelTargets(run: { in_flight_node_ids?: string[] }, graph: Graph | undefined): string[] {
  const ids = new Set(run.in_flight_node_ids ?? []);
  if (graph) {
    for (const node of Object.values(graph.nodes)) {
      if (node.reply?.status === 'pending') ids.add(node.id);
    }
  }
  return [...ids];
}

export async function runConsoleAction(event: ConsoleActionEvent, context: ConsoleChannelContext, io: ChannelCommands = commands): Promise<void> {
  try {
    const run = (await io.listConsoleRuns()).find((item) => item.run_id === event.run_id);
    if (!run) throw new Error(`找不到 run：${event.run_id}`);
    if (event.action === 'adopt') {
      if (!event.node_id) throw new Error('缺少 node_id');
      const graph = context.getState().graphs[run.graph_id];
      if (!graph) throw new Error('通道图未打开');
      const next = { ...setReplyAdopted(graph, event.node_id, true), updated_at: new Date().toISOString() };
      context.dispatch({ type: 'SET_GRAPH', graph: next });
      await io.saveGraph(next);
    } else if (run.status === 'running') {
      const targets = consoleCancelTargets(run, context.getState().graphs[run.graph_id]);
      await Promise.all(targets.map((nodeId) => io.cancelDispatchNode(run.run_id, nodeId)));
      await io.consoleRunUpdate({ run_id: run.run_id, graph_id: run.graph_id, status: 'failed', nodes: run.nodes.map((node) => node.status === 'running' ? { ...node, status: 'failed', text_excerpt: '派发已取消' } : node), leftover: run.leftover ?? [], in_flight_node_ids: [], reason: '派发已取消' });
    }
    await io.consoleActionAck(event.request_id);
  } catch (error) {
    await io.consoleActionAck(event.request_id, String(error));
  }
}

export async function dismissStaleConsoleProposals(io: Pick<ChannelCommands, 'listConsoleRuns' | 'consoleRunUpdate'> = commands): Promise<void> {
  const stale = (await io.listConsoleRuns()).filter((run) => run.status === 'proposed');
  await Promise.all(stale.map((run) => io.consoleRunUpdate({
    run_id: run.run_id,
    graph_id: run.graph_id,
    status: 'dismissed',
    nodes: run.nodes,
    leftover: run.leftover ?? [],
    in_flight_node_ids: [],
    reason: run.reason ?? '',
  }).catch(() => {})));
}

export async function installConsoleChannel(context: ConsoleChannelContext): Promise<() => void> {
  proposalContext = context;
  if (proposalItems.length === 0) await dismissStaleConsoleProposals();
  const [stopDispatch, stopProposal, stopAction] = await Promise.all([
    onConsoleDispatch((event) => { void runConsoleDispatch(event, context).catch((error) => commands.consoleRunUpdate({ run_id: event.run_id, graph_id: '', status: 'rejected', nodes: [], leftover: [], in_flight_node_ids: [], reason: String(error) }).catch(() => {})); }),
    onConsolePropose((event) => { void runConsoleProposal(event, context).catch((error) => commands.consoleRunUpdate({ run_id: event.proposal_id, graph_id: '', status: 'rejected', nodes: [], leftover: [], in_flight_node_ids: [], reason: String(error) }).catch(() => {})); }),
    onConsoleAction((event) => { void runConsoleAction(event, context); }),
  ]);
  return () => {
    if (proposalContext === context) proposalContext = null;
    stopDispatch(); stopProposal(); stopAction();
  };
}

export function addWindowsPathEntry(value: string | null, entry: string): string | null {
  if (value === null) return entry;
  const wanted = normalizedPath(entry);
  const items = value.split(';').filter(Boolean);
  return items.some((item) => normalizedPath(item) === wanted) ? value : [...items, entry].join(';');
}

export function removeWindowsPathEntry(value: string | null, entry: string): string | null {
  if (value === null) return null;
  const wanted = normalizedPath(entry);
  const kept = value.split(';').filter((item) => item !== '' && normalizedPath(item) !== wanted);
  return kept.length === value.split(';').filter(Boolean).length ? value : kept.join(';');
}
