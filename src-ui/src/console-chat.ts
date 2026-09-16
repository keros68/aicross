import { commands, type DispatchNodeResult, type DispatchSettings, type DispatchStageRequest } from './tauri';
import type { DispatchableModel, Graph, Preset, Target, Thinking } from './graph/types';
import { newGraph } from './graph/types';
import { stamp } from './graph/stamp';
import { loadDispatchSettings } from './graph/dispatch-settings';
import {
  buildNodeRequest,
  continuationRunner,
  continuationSession,
  dirtyWorktreeHintOf,
  dispatchableNodeIds,
  failedNodesWithoutResults,
  graphStage,
  presetWithRoleDefaults,
  quotaBlockedNodeIds,
  startPipelineHeadless,
} from './graph/dispatch';
import {
  originalRequestOf,
  serializeContext,
  serializeCurrentMessage,
  upstreamArtifactsOf,
} from './graph/context';
import { validateNode } from './graph/validate';
import { appendConsoleMaterial } from './console-channel';
import {
  consoleMessage,
  selectConsoleApplyMessage,
  type ConsoleApplyDetail,
  type ConsoleMessage,
  type ConsoleTargetAgent,
} from './console-session';

type ConsoleCommands = Pick<typeof commands,
  'listDispatchableModels' | 'loadRoles' | 'gitChanges' | 'previewDispatch' |
  'dispatchGraphStage' | 'saveGraph'>;

export interface ConsoleTurnResult {
  result: DispatchNodeResult;
  target: ConsoleTargetAgent;
}

export interface ConsolePresetResult {
  graph: Graph;
  results: ConsoleTurnResult[];
  error: string;
}

export interface ConsoleApplySnapshot {
  busy: boolean;
  draft: string;
  messages: ConsoleMessage[];
  lastAppliedMessageId?: string;
  existingGraph?: Graph;
}

export interface ConsoleApplyHandlerOptions {
  sessionId: string;
  project: string;
  snapshot: () => ConsoleApplySnapshot;
  append: (message: ConsoleMessage) => void;
  clearDraft: () => void;
  setBusy: (busy: boolean) => void;
  markApplied: (messageId: string) => void;
  commitGraph: (graph: Graph) => void;
  rememberGraph: (graphId: string) => void;
  onOutcome: (outcome: ConsolePresetResult) => void;
  dispatchPreset?: typeof dispatchConsolePreset;
  io?: ConsoleCommands;
}

function requestSettings(settings: ReturnType<typeof loadDispatchSettings>): DispatchSettings {
  return {
    timeoutSecs: settings.timeoutSecs,
    concurrency: settings.concurrency,
    maxRequests: settings.maxRequests,
    redact: settings.redact,
    writeProjectDispatch: settings.writeProjectDispatch,
    ccSwitchPath: settings.ccSwitchPath,
    validationCommand: settings.validationCommand,
    acpEnabled: settings.acpEnabled,
    fixRoundLimit: settings.fixRoundLimit,
  };
}

function thinkingOf(value: string): Thinking {
  return value === '关' || value === '低' || value === '高' ? value : '中';
}

function modelTarget(model: DispatchableModel): Target {
  return {
    agent: model.agent,
    provider: model.provider || undefined,
    model: model.id,
    role: '',
    tier: model.tier,
    thinking: thinkingOf(model.thinking),
    visibility: '共享',
    write: '只读',
  };
}

function exactModel(models: DispatchableModel[], target: ConsoleTargetAgent): DispatchableModel | undefined {
  return models.find((model) => model.agent === target.agent
    && model.id === target.model
    && (model.provider ?? '') === target.provider);
}

export async function dispatchConsoleTurn(
  project: string,
  text: string,
  target: ConsoleTargetAgent,
  resumeId: string,
  io: ConsoleCommands = commands,
): Promise<ConsoleTurnResult> {
  const resources = await io.listDispatchableModels();
  if (resources.error) throw new Error(`读取资源失败：${resources.error}`);
  const model = exactModel(resources.models, target);
  if (!model) throw new Error('当前目标不在可派发模型清单中');
  const settings = loadDispatchSettings();
  const nodeId = crypto.randomUUID();
  const request: DispatchStageRequest = {
    runId: `console-chat-${crypto.randomUUID()}`,
    stage: 1,
    projectDir: project,
    settings: requestSettings(settings),
    nodes: [buildNodeRequest(nodeId, modelTarget(model), {
      models: resources.models,
      prompt: text,
      materials: [],
      currentMessage: text,
      originalRequest: text,
      upstreamArtifacts: [],
      sessionId: resumeId,
      previousRunner: '',
      rollback: false,
      stage: 1,
    })],
  };
  const response = await io.dispatchGraphStage(request);
  const result = response.results[0];
  if (!result) throw new Error('派发没有返回结果');
  return { result, target };
}

export async function dispatchConsolePreset(
  project: string,
  text: string,
  preset: Preset,
  commitGraph: (graph: Graph) => void,
  existingGraph?: Graph,
  io: ConsoleCommands = commands,
): Promise<ConsolePresetResult> {
  const settings = loadDispatchSettings();
  const [resources, roles] = await Promise.all([
    io.listDispatchableModels(),
    io.loadRoles(),
  ]);
  if (resources.error) throw new Error(`读取资源失败：${resources.error}`);
  const effective = presetWithRoleDefaults(preset, roles);
  const base = existingGraph ?? newGraph(project);
  const graphSource = existingGraph ? existingGraph.source : ('console' as const);
  const material = appendConsoleMaterial({ ...base, project, source: graphSource }, text, []);
  let graph = stamp(material.graph, material.nodeId, effective, settings.swarmLimit);
  const newNodeIds = Object.keys(graph.nodes).filter((id) => !material.graph.nodes[id]);
  graph = {
    ...graph,
    nodes: Object.fromEntries(Object.entries(graph.nodes).map(([id, node]) => [
      id,
      newNodeIds.includes(id) ? { ...node, source: 'console' as const } : node,
    ])),
  };
  const created = dispatchableNodeIds(graph).filter((id) => newNodeIds.includes(id));
  const missing = created.find((id) => {
    const target = graph.nodes[id]?.target;
    return !target?.agent.trim() || !target.model.trim();
  });
  if (missing) throw new Error(`角色「${graph.nodes[missing].target?.role ?? ''}」没有可派发目标`);
  const blocked = created.flatMap((id) => validateNode(graph, id, resources.models))
    .find((hint) => hint === '未接入' || hint.includes('只能只读'));
  if (blocked) throw new Error(blocked);
  const quota = quotaBlockedNodeIds(graph, created, resources.models);
  if (quota.length > 0) {
    const target = graph.nodes[quota[0]].target!;
    throw new Error(`目标额度用尽：${target.agent}/${target.model}`);
  }
  if (created.length > settings.maxRequests) {
    throw new Error(`本次派发 ${created.length} 路，超过总请求上限 ${settings.maxRequests}`);
  }
  const dirty = await io.gitChanges(project).catch(() => null);
  const dirtyReason = dirtyWorktreeHintOf(
    dirty,
    created.some((id) => graph.nodes[id].target?.write === 'worktree'),
  );
  if (dirtyReason) throw new Error(dirtyReason);

  const runId = `console-mode-${crypto.randomUUID()}`;
  const commit = (next: Graph) => {
    graph = { ...next, project, source: graphSource, updated_at: new Date().toISOString() };
    commitGraph(graph);
    void io.saveGraph(graph).catch((error) => console.warn('[console] 保存对话图失败', error));
    return graph;
  };
  commit(graph);
  const buildRequest = (sourceGraph: Graph, ids: string[]): DispatchStageRequest => ({
    runId,
    stage: ids.length ? Math.min(...ids.map((id) => graphStage(sourceGraph, id))) : 1,
    projectDir: project,
    settings: requestSettings(settings),
    nodes: ids.map((id) => {
      const node = sourceGraph.nodes[id];
      const selected = serializeContext(sourceGraph, id);
      return buildNodeRequest(id, node.target!, {
        models: resources.models,
        prompt: selected.prompt,
        materials: selected.materials,
        currentMessage: serializeCurrentMessage(node.user),
        originalRequest: originalRequestOf(sourceGraph),
        upstreamArtifacts: upstreamArtifactsOf(sourceGraph, id),
        sessionId: continuationSession(sourceGraph, id) ?? '',
        previousRunner: continuationRunner(sourceGraph, id) ?? '',
        rollback: false,
        stage: graphStage(sourceGraph, id),
      });
    }),
  });
  const pipeline = await startPipelineHeadless<DispatchNodeResult, DispatchStageRequest, unknown>({
    graph,
    settings,
    skip: new Set(Object.keys(graph.nodes).filter((id) => graph.nodes[id].target && !newNodeIds.includes(id))),
    deps: {
      runId,
      buildRequest,
      preview: (request) => io.previewDispatch(request),
      dispatch: (request) => io.dispatchGraphStage(request),
      commit,
      onPreview: () => {},
      onStage: () => {},
      onResults: () => {},
    },
    onError: (active, collected, message, working) => failedNodesWithoutResults(active, collected, message).map((node) => ({
      ...node,
      stage: graphStage(working, node.nodeId),
      rawStdout: '',
      rawStderr: '',
      cacheRead: 0,
      exitCode: null,
      timedOut: false,
      truncated: false,
      redacted: 0,
      isolation: '部分',
      runner: 'unknown',
      workdir: '',
      dispatchFile: '',
      frontmatter: '',
      outputPreview: '',
      validationOutput: '',
      artifacts: null,
      fallbackFrom: '',
      fallbackTo: '',
      fallbackSkipped: '',
    })),
  });
  graph = pipeline.graph;
  await io.saveGraph(graph);
  const results = pipeline.results.flatMap((result) => {
    const target = graph.nodes[result.nodeId]?.target;
    return target ? [{
      result,
      target: { agent: target.agent, provider: target.provider ?? '', model: target.model },
    }] : [];
  });
  return { graph, results, error: pipeline.error };
}

export function createConsoleApplyHandler(options: ConsoleApplyHandlerOptions): (event: Event) => void {
  return (event: Event) => {
    const detail = (event as CustomEvent<ConsoleApplyDetail>).detail;
    if (!detail || detail.sessionId !== options.sessionId) return;
    const snapshot = options.snapshot();
    if (snapshot.busy) {
      options.append(consoleMessage('system', '当前消息仍在处理中。'));
      return;
    }
    const selection = selectConsoleApplyMessage(
      snapshot.draft,
      snapshot.messages,
      snapshot.lastAppliedMessageId,
    );
    if (selection.blocked === 'empty') {
      options.append(consoleMessage('system', '先输入一条消息，再应用模式。'));
      return;
    }
    if (selection.blocked === 'already-applied') {
      options.append(consoleMessage('system', '最近一条消息已应用过模式，请输入新内容。'));
      return;
    }
    const message = selection.message ?? consoleMessage('user', selection.text);
    if (!selection.message) {
      options.append(message);
      options.clearDraft();
    }
    options.markApplied(message.id);
    options.setBusy(true);
    const run = options.dispatchPreset ?? dispatchConsolePreset;
    void run(
      options.project,
      selection.text,
      detail.preset,
      (graph) => {
        options.commitGraph(graph);
        options.rememberGraph(graph.id);
      },
      snapshot.existingGraph,
      options.io ?? commands,
    ).then((outcome) => {
      options.onOutcome(outcome);
    }).catch((error) => {
      options.append(consoleMessage('system', String(error).replace(/^Error:\s*/, '')));
    }).finally(() => options.setBusy(false));
  };
}
