import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createConsoleApplyHandler } from '../console-chat';
import { appendConsoleMaterial, runConsoleDispatch } from '../console-channel';
import type { ConsoleMessage } from '../console-session';
import type { DispatchNodeResult, DispatchStageRequest } from '../tauri';
import { newGraph, type DispatchableModel, type Graph, type Preset, type Target } from '../graph/types';

function target(agent: string, model: string, role: string, visibility: Target['visibility']): Target {
  return { agent, model, role, thinking: '中', visibility, write: '只读' };
}

function model(agent: string, id: string): DispatchableModel {
  return {
    id, agent, provider: '', vendor: agent, billing_source: '', tier: 'pro', thinking: '中',
    status: 'ready', verification: 'verified', source: '', smoked_at: '', smoke_model: '',
    smoke_failure: '', meta: '', note: '',
  };
}

function resultsFor(request: DispatchStageRequest): DispatchNodeResult[] {
  return request.nodes.map((node) => ({
    nodeId: node.nodeId,
    stage: request.stage,
    status: 'ok',
    text: node.role === '执行' ? '第一次执行回复也应留在续写上下文' : 'APPROVED',
    rawStdout: '', rawStderr: '', modelReported: node.model, sessionId: '',
    tokensIn: 10, tokensOut: 5, cacheRead: 0, durationMs: 100,
    exitCode: 0, timedOut: false, truncated: false, redacted: 0, isolation: '完整', runner: 'oneshot',
    workdir: '', dispatchFile: '', frontmatter: '', outputPreview: '', error: '',
    artifacts: null, validationOutput: '', fallbackFrom: '', fallbackTo: '', fallbackSkipped: '',
  }));
}

describe('TASK-30 控制台每次运行的原始需求', () => {
  beforeEach(() => {
    let id = 0;
    vi.stubGlobal('crypto', { randomUUID: () => `id-${++id}` });
    vi.stubGlobal('localStorage', {
      getItem: () => null,
      setItem: () => {},
      removeItem: () => {},
    });
    class TestCustomEvent<T> extends Event {
      detail: T;
      constructor(type: string, init: CustomEventInit<T>) {
        super(type);
        this.detail = init.detail as T;
      }
    }
    vi.stubGlobal('CustomEvent', TestCustomEvent);
  });

  afterEach(() => vi.unstubAllGlobals());

  it('同一张图连续假派发时，第二次盲审收到需求 B，执行上下文保留第一次对话', async () => {
    const preset: Preset = {
      id: 'task-30', name: '双盲模式', builtin: false,
      stages: [
        { nodes: [target('codex', 'exec-model', '执行', '共享')] },
        { nodes: [
          target('kimi', 'accept-model', '验收', '盲'),
          target('pi', 'verify-model', '验证', '盲'),
        ] },
      ],
    };
    const messages: ConsoleMessage[] = [];
    const requests: DispatchStageRequest[] = [];
    const outcomes: { graph: Graph }[] = [];
    let graph: Graph | undefined;
    let draft = '需求 A：说明 beta.py';
    let busy = false;
    const io = {
      listDispatchableModels: async () => ({
        models: [
          model('codex', 'exec-model'),
          model('kimi', 'accept-model'),
          model('pi', 'verify-model'),
        ],
        error: '',
      }),
      loadRoles: async () => ({
        version: 1,
        planning: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
        execution: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
        acceptance: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
      }),
      gitChanges: async () => null,
      previewDispatch: async () => ({ totalRedacted: 0, broadRedacted: 0, nodes: [] }),
      dispatchGraphStage: async (request: DispatchStageRequest) => {
        requests.push(request);
        return { runId: request.runId, results: resultsFor(request) };
      },
      saveGraph: async () => {},
    };
    const handler = createConsoleApplyHandler({
      sessionId: 'console-tab',
      project: 'D:/work',
      snapshot: () => ({ busy, draft, messages, existingGraph: graph }),
      append: (message) => messages.push(message),
      clearDraft: () => { draft = ''; },
      setBusy: (value) => { busy = value; },
      markApplied: () => {},
      commitGraph: (next) => { graph = next; },
      rememberGraph: () => {},
      onOutcome: (outcome) => outcomes.push(outcome),
      io: io as never,
      subscribeProgress: () => ({ dispose: () => {} }),
    });
    const apply = async () => {
      const previousOutcomes = outcomes.length;
      handler(new CustomEvent('aicross:console-apply-preset', { detail: { sessionId: 'console-tab', preset } }));
      await vi.waitFor(() => expect(busy).toBe(false));
      if (outcomes.length === previousOutcomes) throw new Error(messages.at(-1)?.text || '模式派发未完成');
    };

    await apply();
    draft = '需求 B：说明 alpha.py';
    await apply();

    const runIds = [...new Set(requests.map((request) => request.runId))];
    expect(runIds).toHaveLength(2);
    const secondNodes = requests.filter((request) => request.runId === runIds[1]).flatMap((request) => request.nodes);
    const blindNodes = secondNodes.filter((node) => node.role === '验收' || node.role === '验证');
    expect(blindNodes).toHaveLength(2);
    expect(blindNodes.map((node) => node.originalRequest)).toEqual([
      '需求 B：说明 alpha.py',
      '需求 B：说明 alpha.py',
    ]);
    expect(blindNodes.every((node) => node.originalRequest !== '需求 A：说明 beta.py')).toBe(true);
    const execution = secondNodes.find((node) => node.role === '执行');
    expect(execution?.originalRequest).toBe('需求 B：说明 alpha.py');
    expect(execution?.prompt).toContain('需求 A：说明 beta.py');
    expect(execution?.prompt).toContain('第一次执行回复也应留在续写上下文');
    expect(execution?.prompt).toContain('需求 B：说明 alpha.py');
  });

  it('外部通道续写现有图时也把本次材料节点作为原始需求', async () => {
    const preset: Preset = {
      id: 'task-30-channel', name: '盲审模式', builtin: false,
      stages: [{ nodes: [target('kimi', 'accept-model', '验收', '盲')] }],
    };
    const initial = appendConsoleMaterial(newGraph('D:/work'), '需求 A：说明 beta.py', []).graph;
    let state = {
      graphs: { [initial.id]: initial },
      terminals: [],
      activeTerminalId: null,
    };
    const requests: DispatchStageRequest[] = [];
    const io = {
      listDispatchableModels: async () => ({ models: [model('kimi', 'accept-model')], error: '' }),
      listPresets: async () => ({ version: 1 as const, presets: [preset] }),
      loadRoles: async () => ({
        version: 1,
        planning: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
        execution: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
        acceptance: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
      }),
      listConsoleRuns: async () => [],
      gitChanges: async () => null,
      previewDispatch: async () => ({ totalRedacted: 0, broadRedacted: 0, nodes: [] }),
      dispatchGraphStage: async (request: DispatchStageRequest) => {
        requests.push(request);
        return { runId: request.runId, results: resultsFor(request) };
      },
      saveGraph: async () => {},
      consoleRunUpdate: async () => {},
    };
    const context = {
      getState: () => state,
      dispatch: (action: { graph?: Graph }) => {
        if (action.graph) state = { ...state, graphs: { ...state.graphs, [action.graph.id]: action.graph } };
      },
    };

    await runConsoleDispatch({
      run_id: 'channel-run', project: 'D:/work', text: '需求 B：说明 alpha.py',
      preset: preset.id, attachments: [], reason: '',
    }, context as never, io as never);

    expect(requests).toHaveLength(1);
    expect(requests[0].nodes[0].originalRequest).toBe('需求 B：说明 alpha.py');
    expect(requests[0].nodes[0].originalRequest).not.toBe('需求 A：说明 beta.py');
  });
});
