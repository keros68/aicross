import { commands, onConsoleAction, onConsoleDispatch, type ConsoleActionEvent, type ConsoleDispatchEvent, type ConsoleRun, type ConsoleRunNode, type DispatchNodeResult, type DispatchStageRequest } from './tauri';
import type { AppState, TerminalSession } from './store/app-state';
import type { Graph, GraphNode, Preset } from './graph/types';
import { newGraph, newNodeId } from './graph/types';
import { latestLeaf } from './graph/layout';
import { fillRoot } from './graph/mutate';
import { stamp } from './graph/stamp';
import { loadDispatchSettings } from './graph/dispatch-settings';
import { originalRequestOf, serializeContext, serializeCurrentMessage, upstreamArtifactsOf } from './graph/context';
import { buildNodeRequest, continuationRunner, continuationSession, dirtyWorktreeHintOf, dispatchableNodeIds, failedNodesWithoutResults, graphStage, leftoverNodes, presetWithRoleDefaults, quotaBlockedNodeIds, setReplyAdopted, startPipelineHeadless } from './graph/dispatch';
import { validateNode } from './graph/validate';

export type ConsoleTabPlan =
  | { kind: 'focus'; sessionId: string }
  | { kind: 'background' }
  | { kind: 'open' };

export function consoleTabPlan(sessions: TerminalSession[], activeId: string | null, graphId: string): ConsoleTabPlan {
  const existing = sessions.find((session) => session.tool === 'graph' && session.toolData === graphId);
  const active = sessions.find((session) => session.id === activeId);
  const commanderActive = Boolean(active?.commanderTitle);
  if (existing) return commanderActive ? { kind: 'background' } : { kind: 'focus', sessionId: existing.id };
  return commanderActive ? { kind: 'background' } : { kind: 'open' };
}

export function consoleInflightCount(runs: ConsoleRun[]): number {
  return runs.filter((run) => run.status === 'running')
    .reduce((count, run) => count + (run.in_flight_node_ids?.length ?? run.nodes.filter((node) => node.status === 'running').length), 0);
}

const normalizedPath = (value: string | null | undefined) => (value ?? '').replace(/\\/g, '/').replace(/\/+$/, '').toLocaleLowerCase();

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

function appendConsoleMaterial(graph: Graph, text: string, attachments: string[]): { graph: Graph; nodeId: string } {
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
  if (event.targets?.length) return { id: `console-${event.run_id}`, name: '自定义模式', builtin: false, stages: [{ nodes: event.targets }] };
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

type ChannelCommands = Pick<typeof commands, 'listDispatchableModels' | 'listPresets' | 'loadRoles' | 'gitChanges' | 'previewDispatch' | 'dispatchGraphStage' | 'saveGraph' | 'consoleRunUpdate' | 'listConsoleRuns' | 'cancelDispatchNode' | 'consoleActionAck'>;

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
  const base = selectConsoleGraph(context.getState(), event.project) ?? newGraph(event.project);
  const material = appendConsoleMaterial({ ...base, project: event.project, source: 'console' }, event.text, event.attachments ?? []);
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
  const commit = (next: Graph) => {
    const latest = context.getState().graphs[graph.id] ?? committed;
    graph = { ...mergeConsoleCommit(latest, committed, next), project: event.project, source: 'console' };
    graph = { ...graph, updated_at: new Date().toISOString() };
    committed = graph;
    context.dispatch({ type: 'SET_GRAPH', graph });
    void io.saveGraph(graph)
      .then(() => io.consoleRunUpdate({ run_id: event.run_id, graph_id: graph.id, status: 'running', nodes: nodeSummary(graph, allRunIds), leftover: leftoverNodes(graph).flatMap((node) => node.leftover?.items ?? []), in_flight_node_ids: [], reason: '' }))
      .catch((error) => console.warn('[graph] 保存失败', error));
    return graph;
  };
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
      return buildNodeRequest(id, node.target!, { models: resources.models, prompt: selected.prompt, materials: selected.materials, currentMessage: serializeCurrentMessage(node.user), originalRequest: originalRequestOf(sourceGraph), upstreamArtifacts: upstreamArtifactsOf(sourceGraph, id), sessionId: continuationSession(sourceGraph, id) ?? '', previousRunner: continuationRunner(sourceGraph, id) ?? '', rollback: false, stage: graphStage(sourceGraph, id) });
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
        void io.consoleRunUpdate({ run_id: event.run_id, graph_id: graph.id, status: 'running', nodes: nodeSummary(graph, allRunIds), leftover: [], in_flight_node_ids: request.nodes.map((node) => node.nodeId), reason: '' });
      },
      onResults: (results) => results.forEach((result) => allRunIds.add(result.nodeId)),
    },
    onError: (active, collected, message, working) => failedNodesWithoutResults(active, collected, message).map((node) => ({ ...node, stage: graphStage(working, node.nodeId), rawStdout: '', rawStderr: '', cacheRead: 0, exitCode: null, timedOut: false, truncated: false, redacted: 0, isolation: '部分', runner: 'unknown', workdir: '', dispatchFile: '', frontmatter: '', outputPreview: '', validationOutput: '', artifacts: null, fallbackFrom: '', fallbackTo: '', fallbackSkipped: '' })),
  });
  graph = pipeline.graph;
  const statuses = nodeSummary(graph, allRunIds).map((node) => node.status);
  const status = pipeline.error || statuses.includes('failed') ? 'failed' : 'done';
  await io.saveGraph(graph);
  await io.consoleRunUpdate({ run_id: event.run_id, graph_id: graph.id, status, nodes: nodeSummary(graph, allRunIds), leftover: leftoverNodes(graph).flatMap((node) => node.leftover?.items ?? []), in_flight_node_ids: [], reason: pipeline.error });
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
      await Promise.all((run.in_flight_node_ids ?? []).map((nodeId) => io.cancelDispatchNode(run.run_id, nodeId)));
      await io.consoleRunUpdate({ run_id: run.run_id, graph_id: run.graph_id, status: 'failed', nodes: run.nodes.map((node) => node.status === 'running' ? { ...node, status: 'failed', text_excerpt: '派发已取消' } : node), leftover: run.leftover ?? [], in_flight_node_ids: [], reason: '派发已取消' });
    }
    await io.consoleActionAck(event.request_id);
  } catch (error) {
    await io.consoleActionAck(event.request_id, String(error));
  }
}

export async function installConsoleChannel(context: ConsoleChannelContext): Promise<() => void> {
  const [stopDispatch, stopAction] = await Promise.all([
    onConsoleDispatch((event) => { void runConsoleDispatch(event, context).catch((error) => commands.consoleRunUpdate({ run_id: event.run_id, graph_id: '', status: 'rejected', nodes: [], leftover: [], in_flight_node_ids: [], reason: String(error) })); }),
    onConsoleAction((event) => { void runConsoleAction(event, context); }),
  ]);
  return () => { stopDispatch(); stopAction(); };
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
