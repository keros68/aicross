import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createConsoleApplyHandler } from '../console-chat';
import {
  consoleRunHeaderLabel,
  consoleScrollRestoreTop,
  consoleScrollSnapshot,
  reportedModelLabel,
  type ConsoleRunRecord,
  type ConsoleRunRow,
} from '../console-run-detail';
import type { ConsoleMessage } from '../console-session';
import type { DispatchNodeResult, DispatchStageRequest } from '../tauri';
import type { DispatchableModel, Graph, Preset, Target } from '../graph/types';

function target(agent: string, model: string, role: string): Target {
  return { agent, model, role, thinking: '中', visibility: '共享', write: '只读' };
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
    text: node.role === '验收' ? '还有问题\nNEEDS_FIX' : '已完成',
    rawStdout: '', rawStderr: '', modelReported: node.model, sessionId: '',
    tokensIn: 10, tokensOut: 5, cacheRead: 0, durationMs: 100,
    exitCode: 0, timedOut: false, truncated: false, redacted: 0, isolation: '完整', runner: 'oneshot',
    workdir: '', dispatchFile: '', frontmatter: '', outputPreview: '', error: '',
    artifacts: node.role === '执行' ? { diff: 'x', changed_files: [], stdout_summary: '已完成' } : null,
    validationOutput: '', fallbackFrom: '', fallbackTo: '', fallbackSkipped: '',
  }));
}

function row(overrides: Partial<ConsoleRunRow> = {}): ConsoleRunRow {
  return {
    nodeId: 'n1', stage: 1, role: '执行', agent: 'codex', provider: '', requestedModel: 'gpt',
    modelReported: '', status: 'ok', acceptance: '', tokensIn: 1, tokensOut: 1, cacheRead: 0,
    durationMs: 100, fallbackFrom: '', fallbackTo: '', progressElapsedMs: 0, progressAt: 0,
    toolCalls: 0, lastTool: '', ...overrides,
  };
}

describe('TASK-29 控制台每次应用模式', () => {
  beforeEach(() => {
    let id = 0;
    vi.stubGlobal('crypto', { randomUUID: () => `id-${++id}` });
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => key === 'aicross-dispatch-settings'
        ? JSON.stringify({ fixRoundLimit: 1 })
        : null,
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

  it('同一张图连续应用两次时，第二次从第 1 阶段开始并仍可跑一轮修正', async () => {
    const preset: Preset = {
      id: 'task-29', name: '经济模式', builtin: false,
      stages: [
        { nodes: [target('codex', 'main-model', '执行')] },
        { nodes: [target('kimi', 'review-model', '验收')] },
      ],
    };
    const messages: ConsoleMessage[] = [];
    const requests: DispatchStageRequest[] = [];
    const completedRuns: ConsoleRunRecord[] = [];
    const outcomes: { graph: Graph }[] = [];
    let graph: Graph | undefined;
    let draft = '第一次处理';
    let busy = false;
    const io = {
      listDispatchableModels: async () => ({
        models: [model('codex', 'main-model'), model('kimi', 'review-model')], error: '',
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
      onOutcome: (outcome) => {
        outcomes.push(outcome);
      },
      onRunUpdate: (run) => {
        if (run.status !== 'completed') return;
        const index = completedRuns.findIndex((item) => item.id === run.id);
        if (index < 0) completedRuns.push(run);
        else completedRuns[index] = run;
      },
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
    const firstGraphId = graph?.id;
    const secondMessageStart = messages.length;
    draft = '第二次处理';
    await apply();

    expect(outcomes).toHaveLength(2);
    expect(outcomes[1].graph.id).toBe(firstGraphId);
    const secondMessages = messages.slice(secondMessageStart);
    const stageMessages = secondMessages.filter((message) => message.runId && message.text.startsWith('第 '));
    expect(stageMessages.map((message) => message.text.match(/\d+/)?.[0])).toEqual(['1', '2', '3', '4']);
    expect(secondMessages.filter((message) => message.kind === 'assistant').map((message) => message.stage)).toEqual([1, 2, 3, 4]);
    const secondRunId = stageMessages[0].runId;
    expect(requests.filter((request) => request.runId === secondRunId)).toHaveLength(4);
    expect(secondMessages.at(-1)?.text).toContain('结论：未通过（修正 1 轮后验收仍要求修正，已到上限）。');
    expect(secondMessages.at(-1)?.text).toContain('共 4 路');
    expect(completedRuns).toHaveLength(2);
    expect(completedRuns[1].rows.map((item) => item.stage)).toEqual([1, 2, 3, 4]);
  });

  it('运行中的实际模型显示占位，完成后才报告缺失', () => {
    expect(reportedModelLabel(row({ status: 'pending' }))).toBe('—');
    expect(reportedModelLabel(row({ status: 'running', agent: 'kimi' }))).toBe('—');
    expect(reportedModelLabel(row({ status: 'ok' }))).toBe('未报告');
    expect(reportedModelLabel(row({ status: 'ok', agent: 'kimi' }))).toBe('kimi 不报真身');
  });

  it('页头只显示模式名与本地开始时间', () => {
    const startedAt = new Date(2026, 8, 17, 14, 5, 42).getTime();
    const record = { presetName: '经济模式', startedAt } as ConsoleRunRecord;
    expect(consoleRunHeaderLabel(record)).toBe('经济模式 · 2026-09-17 14:05');
    expect(consoleRunHeaderLabel(record)).not.toContain('console-mode-');
  });

  it('返回对话时恢复原滚动位置，原先在底部且有新消息时跟到最新', () => {
    const middle = consoleScrollSnapshot({
      scrollTop: 240, scrollHeight: 1000, clientHeight: 400, messageCount: 6,
    });
    expect(consoleScrollRestoreTop(middle, {
      scrollHeight: 1300, clientHeight: 400, messageCount: 8,
    })).toBe(240);

    const bottom = consoleScrollSnapshot({
      scrollTop: 600, scrollHeight: 1000, clientHeight: 400, messageCount: 6,
    });
    expect(consoleScrollRestoreTop(bottom, {
      scrollHeight: 1300, clientHeight: 400, messageCount: 8,
    })).toBe(900);
  });
});
