import { commands, type DispatchNodeResult, type DispatchProgress, type DispatchSettings, type DispatchStageRequest } from './tauri';
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
import { subscribeProgress } from './graph/dispatch-progress';
import {
  consoleRunTotals,
  createConsoleRunRecord,
  finishConsoleRun,
  withConsoleRunProgress,
  withConsoleRunResults,
  withConsoleRunStage,
  type ConsoleRunRecord,
} from './console-run-detail';
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

export interface ConsolePresetFeedback {
  append: (message: ConsoleMessage) => void;
  now?: () => number;
  onRunUpdate?: (run: ConsoleRunRecord) => void;
  onRunProgress?: (run: ConsoleRunRecord) => void;
  subscribeProgress?: (
    runId: string,
    onEvent: (payload: DispatchProgress) => void,
  ) => { dispose: () => void };
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
  onRunUpdate?: (run: ConsoleRunRecord) => void;
  onRunProgress?: (run: ConsoleRunRecord) => void;
  now?: () => number;
  subscribeProgress?: ConsolePresetFeedback['subscribeProgress'];
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

function resultStatusLabel(result: DispatchNodeResult): string {
  if (result.timedOut || result.status === 'timeout') return '超时';
  if (result.status === 'quota_exhausted') return '额度用尽';
  if (result.status === 'identity_mismatch') return '真身不符';
  if (result.status === 'validation_failed') return '验证命令未通过';
  return result.status === 'ok' ? '完成' : '失败';
}

export function consoleResultMessage(
  target: ConsoleTargetAgent,
  result: DispatchNodeResult,
  role = '',
): ConsoleMessage {
  const fallback = result.fallbackTo.split('/');
  const actualAgent = result.fallbackTo ? (fallback.shift() || target.agent) : target.agent;
  const actualModel = result.modelReported || (result.fallbackTo ? fallback.join('/') : '') || target.model;
  const route = [
    result.fallbackTo ? `备选：${result.fallbackTo}` : '',
    result.modelReported ? `实际模型：${result.modelReported}` : '',
  ].filter(Boolean).join(' · ');
  const failed = result.status !== 'ok';
  const reason = result.error || result.runnerNote || result.text || '未返回原因';
  const text = failed
    ? [`状态：${resultStatusLabel(result)}`, `原因：${reason}`,
      result.text && result.text !== reason ? `\n${result.text}` : ''].filter(Boolean).join('\n')
    : result.text || '已完成。';
  return consoleMessage('assistant', text, {
    agent: actualAgent,
    provider: result.fallbackTo ? '' : target.provider,
    model: actualModel,
    dispatchModel: actualModel,
    role,
    durationMs: result.durationMs,
    status: result.status,
    route,
  });
}

export async function dispatchConsoleTurn(
  project: string,
  text: string,
  target: ConsoleTargetAgent,
  resumeId: string,
  io: ConsoleCommands = commands,
): Promise<ConsoleTurnResult> {
  const request = await prepareConsoleTurn(project, text, target, resumeId, io);
  return dispatchPreparedConsoleTurn(request, target, io);
}

async function prepareConsoleTurn(
  project: string,
  text: string,
  target: ConsoleTargetAgent,
  resumeId: string,
  io: ConsoleCommands,
): Promise<DispatchStageRequest> {
  const resources = await io.listDispatchableModels();
  if (resources.error) throw new Error(`读取资源失败：${resources.error}`);
  const model = exactModel(resources.models, target);
  if (!model) throw new Error('当前目标不在可派发模型清单中');
  const settings = loadDispatchSettings();
  const nodeId = crypto.randomUUID();
  return {
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
}

async function dispatchPreparedConsoleTurn(
  request: DispatchStageRequest,
  target: ConsoleTargetAgent,
  io: ConsoleCommands,
): Promise<ConsoleTurnResult> {
  const response = await io.dispatchGraphStage(request);
  const result = response.results[0];
  if (!result) throw new Error('派发没有返回结果');
  return { result, target };
}

// 外部 CLI 没有统一错误码，只能靠英文文案判断「续聊的会话已经没了」。
// 这是保守启发式：新的或本地化的供应商文案可能漏判（漏判只是多报一次失败），
// 但**绝不能误判**——误判会清掉有效会话、提示「已失效」、再重派一次多花额度。
//
// 所以要求「会话对象词」与「失效词」**贴在一起**，而不是各自出现在整句任意位置
// （09-17 编排者核对：旧写法把「Invalid API key for this session」与
// 「unknown error in thread pool」都判成了会话失效）。
const EXPIRED_RESUME_PATTERNS: RegExp[] = [
  // session not found / thread 01a0 not found / conversation does not exist / session 'abc' has expired
  // 会话词与失效词之间只允许一个 id 样的词（09-17 Gemini 复审：放任意 40 字会把
  // 「Worker thread error: task not found」判成失效）
  /\b(?:session|thread|conversation)(?:\s+id)?(?:\s+['"]?[\w-]+['"]?)?\s+(?:is\s+)?(?:not found|does not exist|no longer exists|has expired|expired)\b/i,
  // no session / unknown session / invalid thread / no such conversation
  /\b(?:no|unknown|invalid|expired|no such)\s+(?:session|thread|conversation)\b/i,
  // cannot resume / could not resume / failed to resume
  /\b(?:cannot|can't|could not|couldn't|failed to|unable to)\s+resume\b/i,
];

export function isExpiredResumeError(message: string): boolean {
  return EXPIRED_RESUME_PATTERNS.some((pattern) => pattern.test(message));
}

export async function dispatchConsoleTurnWithFallback(
  project: string,
  text: string,
  target: ConsoleTargetAgent,
  resumeId: string,
  onExpired: () => void,
  io: ConsoleCommands = commands,
): Promise<ConsoleTurnResult> {
  if (resumeId === '') return dispatchConsoleTurn(project, text, target, '', io);
  // 资源读取与目标检查在回退判断之外；发派前失败不能据此清除有效续聊 id。
  const request = await prepareConsoleTurn(project, text, target, resumeId, io);
  try {
    const resumed = await dispatchPreparedConsoleTurn(request, target, io);
    if (resumed.result.status !== 'error' || !isExpiredResumeError(resumed.result.error)) return resumed;
  } catch (error) {
    if (!isExpiredResumeError(String(error))) throw error;
  }
  onExpired();
  return dispatchConsoleTurn(project, text, target, '', io);
}

export async function dispatchConsolePreset(
  project: string,
  text: string,
  preset: Preset,
  commitGraph: (graph: Graph) => void,
  existingGraph?: Graph,
  io: ConsoleCommands = commands,
  feedback?: ConsolePresetFeedback,
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
  const now = feedback?.now ?? Date.now;
  const startedAt = now();
  let runRecord = createConsoleRunRecord(runId, preset.name, startedAt);
  const updateRun = (next: ConsoleRunRecord) => {
    runRecord = next;
    feedback?.onRunUpdate?.(next);
  };
  updateRun(runRecord);
  const feedbackFailure: { error?: unknown } = {};
  const appendFeedback = (message: ConsoleMessage) => {
    if (!feedback) return;
    try {
      feedback.append(message);
    } catch (error) {
      feedbackFailure.error = error;
      throw error;
    }
  };
  let progressSubscription: { dispose: () => void } | null = null;
  try {
    progressSubscription = feedback?.onRunUpdate || feedback?.onRunProgress
      ? (feedback.subscribeProgress ?? subscribeProgress)(runId, (payload) => {
        const next = withConsoleRunProgress(runRecord, payload, now());
        if (next === runRecord) return;
        runRecord = next;
        feedback?.onRunProgress?.(next);
      })
      : null;
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
    const announcedStages = new Set<number>();
    const announcedResults = new Map<string, string>();
    const announceResults = (collected: DispatchNodeResult[]) => {
      if (feedback?.onRunUpdate) updateRun(withConsoleRunResults(runRecord, graph, collected, now()));
      if (!feedback) return;
      const latest = [...new Map(collected.map((result) => [result.nodeId, result])).values()];
      for (const result of latest) {
        const previousStatus = announcedResults.get(result.nodeId);
        if (previousStatus === result.status) continue;
        const target = graph.nodes[result.nodeId]?.target;
        if (!target) continue;
        const message = consoleResultMessage({
          agent: target.agent,
          provider: target.provider ?? '',
          model: target.model,
        }, result, target.role);
        appendFeedback(previousStatus === undefined ? message : {
          ...message,
          text: `${target.role || '未指定角色'} · 修正后${result.status === 'ok' ? '通过' : resultStatusLabel(result)}\n${message.text}`,
        });
        announcedResults.set(result.nodeId, result.status);
      }
    };
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
        onStage: (request, ids) => {
          if (feedback?.onRunUpdate) updateRun(withConsoleRunStage(runRecord, request, now()));
          if (!feedback || announcedStages.has(request.stage)) return;
          announcedStages.add(request.stage);
          const routes = request.nodes
            .filter((node) => ids.includes(node.nodeId))
            .map((node) => `${node.role || '未指定角色'}（${node.agent} / ${node.model}）`);
          appendFeedback(consoleMessage(
            'system',
            `第 ${request.stage} 阶段 · 派出 ${ids.length} 路：${routes.join('、')}`,
            { runId },
          ));
        },
        onResults: announceResults,
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
    if ('error' in feedbackFailure) throw feedbackFailure.error;
    void io.saveGraph(graph).catch((error) => console.warn('[console] 保存对话图失败', error));
    const uniqueResults = [...new Map(pipeline.results.map((result) => [result.nodeId, result])).values()];
    runRecord = finishConsoleRun(runRecord, graph, uniqueResults, pipeline.error, now());
    feedback?.onRunUpdate?.(runRecord);
    if (feedback) {
      announceResults(uniqueResults);
      const totals = consoleRunTotals(runRecord, runRecord.finishedAt ?? now());
      const failed = uniqueResults.filter((result) => result.status !== 'ok').length;
      appendFeedback(consoleMessage(
        'system',
        `结论：${runRecord.conclusion}。\n共 ${totals.routes} 路，运行失败 ${failed} 路，用时 ${(totals.wallDurationMs / 1000).toFixed(1)} 秒。`,
        { runId },
      ));
    }
    const results = pipeline.results.flatMap((result) => {
      const target = graph.nodes[result.nodeId]?.target;
      return target ? [{
        result,
        target: { agent: target.agent, provider: target.provider ?? '', model: target.model },
      }] : [];
    });
    return { graph, results, error: pipeline.error };
  } catch (error) {
    runRecord = finishConsoleRun(runRecord, graph, [], String(error), now());
    feedback?.onRunUpdate?.(runRecord);
    throw error;
  } finally {
    progressSubscription?.dispose();
  }
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
      {
        append: options.append,
        onRunUpdate: options.onRunUpdate,
        onRunProgress: options.onRunProgress,
        now: options.now,
        subscribeProgress: options.subscribeProgress,
      },
    ).then((outcome) => {
      options.onOutcome(outcome);
    }).catch((error) => {
      options.append(consoleMessage('system', String(error).replace(/^Error:\s*/, '')));
    }).finally(() => options.setBusy(false));
  };
}
