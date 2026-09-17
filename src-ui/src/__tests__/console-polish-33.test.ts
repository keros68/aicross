// @ts-expect-error node builtin，只有静态源码测试读取本地文件
import { readFileSync } from 'node:fs';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createConsoleApplyHandler } from '../console-chat';
import { runConsoleDispatch } from '../console-channel';
import {
  finishConsoleRun,
  liveConsoleRunDurationMs,
  runRowStatusLabel,
  withConsoleRunStage,
  type ConsoleRunRecord,
  type ConsoleRunRow,
} from '../console-run-detail';
import { newGraph, type DispatchableModel, type Graph, type GraphNode, type Preset, type Target } from '../graph/types';
import type { ConsoleMessage } from '../console-session';
import type { DispatchNodeResult, DispatchStageRequest } from '../tauri';

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

function row(nodeId: string, stage: number, role: string, acceptance: ConsoleRunRow['acceptance']): ConsoleRunRow {
  return {
    nodeId, stage, role, agent: 'codex', provider: '', requestedModel: 'model', modelReported: 'model',
    status: 'ok', acceptance, tokensIn: 1, tokensOut: 1, cacheRead: 0, durationMs: 100,
    fallbackFrom: '', fallbackTo: '', progressElapsedMs: 0, progressAt: 0, toolCalls: 0, lastTool: '',
  };
}

function record(rows: ConsoleRunRow[]): ConsoleRunRecord {
  return {
    id: 'run', presetName: '自定义模式', startedAt: 1000, updatedAt: 2000,
    status: 'running', conclusion: '', error: '', rows,
  };
}

function graph(nodes: Array<{ id: string; parents: string[]; role?: string }>): Graph {
  const base = newGraph('D:/work');
  const entries = nodes.map(({ id, parents, role }): [string, GraphNode] => [id, {
    id, kind: 'turn', parents, user: { text: '', attachments: [] },
    reply: { status: 'done', text: '' },
    ...(role ? { target: target('codex', 'model', role) } : {}),
    created_at: '2026-09-17T00:00:00.000Z',
  }]);
  return { ...base, nodes: { ...base.nodes, ...Object.fromEntries(entries) } };
}

const finishWithScope = finishConsoleRun as unknown as (
  value: ConsoleRunRecord,
  valueGraph: Graph,
  results: DispatchNodeResult[],
  error: string,
  now: number,
  scope: ReadonlySet<string>,
) => ConsoleRunRecord;

function resultFor(request: DispatchStageRequest, text: string): DispatchNodeResult {
  const node = request.nodes[0];
  return {
    nodeId: node.nodeId, stage: request.stage, status: 'ok', text,
    rawStdout: '', rawStderr: '', modelReported: node.model, sessionId: '',
    tokensIn: 1, tokensOut: 1, cacheRead: 0, durationMs: 10,
    exitCode: 0, timedOut: false, truncated: false, redacted: 0, isolation: '完整', runner: 'oneshot',
    workdir: '', dispatchFile: '', frontmatter: '', outputPreview: '', error: '',
    artifacts: node.role === '执行' ? { diff: 'x', changed_files: [], stdout_summary: '完成' } : null,
    validationOutput: '', fallbackFrom: '', fallbackTo: '', fallbackSkipped: '',
  };
}

describe('TASK-33 控制台遗留小问题', () => {
  beforeEach(() => {
    let id = 0;
    vi.stubGlobal('crypto', { randomUUID: () => `task-33-${++id}` });
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

  it('十列表格默认宽度不超过 880px，token 使用紧凑显示并保留全数', async () => {
    const detail = await import('../components/center/ConsoleRunDetail');
    const widths = (detail as unknown as { RUN_TABLE_COLUMN_WIDTHS: number[] }).RUN_TABLE_COLUMN_WIDTHS;
    const compact = (detail as unknown as { compactRunNumber: (value: number) => string }).compactRunNumber;
    expect(widths).toHaveLength(10);
    expect(widths.reduce((sum, width) => sum + width, 0)).toBeLessThanOrEqual(880);
    expect(compact(49_400)).toBe('49.4k');
    const source = readFileSync(new URL('../components/center/ConsoleRunDetail.tsx', import.meta.url), 'utf8');
    const css = readFileSync(new URL('../components/center/ConsoleChat.css', import.meta.url), 'utf8');
    expect(source).toContain('title={String(row.tokensIn)}');
    expect(css).toContain('min-width: 880px');
    expect(css).not.toContain('min-width: 1120px');
  });

  it('不等深并行分支按每条链最后一次验收判定', () => {
    const valueGraph = graph([
      { id: 'shallow-exec', parents: [], role: '执行' },
      { id: 'shallow-accept', parents: ['shallow-exec'], role: '验收' },
      { id: 'deep-exec', parents: [], role: '执行' },
      { id: 'deep-accept-1', parents: ['deep-exec'], role: '验收' },
      { id: 'deep-fix', parents: ['deep-accept-1'], role: '执行' },
      { id: 'deep-accept-2', parents: ['deep-fix'], role: '验收' },
    ]);
    const rows = [
      row('shallow-accept', 2, '验收', 'NEEDS_FIX'),
      row('deep-accept-1', 2, '验收', 'NEEDS_FIX'),
      row('deep-fix', 3, '执行', ''),
      row('deep-accept-2', 4, '验收', 'PASS'),
    ];
    expect(finishWithScope(record(rows), valueGraph, [], '', 3000, new Set(rows.map((item) => item.nodeId))).conclusion)
      .toBe('未通过（修正 1 轮后验收仍要求修正，已到上限）');
  });

  it('预设自带验收后的执行阶段但没有流水线修正时仍写验收一次通过', () => {
    const valueGraph = graph([
      { id: 'exec-1', parents: [], role: '执行' },
      { id: 'accept-1', parents: ['exec-1'], role: '验收' },
      { id: 'exec-2', parents: ['accept-1'], role: '执行' },
      { id: 'accept-2', parents: ['exec-2'], role: '验收' },
    ]);
    const rows = [
      row('exec-1', 1, '执行', ''), row('accept-1', 2, '验收', 'PASS'),
      row('exec-2', 3, '执行', ''), row('accept-2', 4, '验收', 'PASS'),
    ];
    expect(finishWithScope(record(rows), valueGraph, [], '', 3000, new Set()).conclusion)
      .toBe('通过（验收一次通过）');
  });

  it('一轮修正后通过与到上限未通过使用实际修正执行节点数', () => {
    const valueGraph = graph([
      { id: 'exec', parents: [], role: '执行' },
      { id: 'accept-1', parents: ['exec'], role: '验收' },
      { id: 'fix', parents: ['accept-1'], role: '执行' },
      { id: 'accept-2', parents: ['fix'], role: '验收' },
    ]);
    const scope = new Set(['fix', 'accept-2']);
    const shared = [row('accept-1', 2, '验收', 'NEEDS_FIX'), row('fix', 3, '执行', '')];
    expect(finishWithScope(record([...shared, row('accept-2', 4, '验收', 'PASS')]), valueGraph, [], '', 3000, scope).conclusion)
      .toBe('通过（修正 1 轮后通过）');
    expect(finishWithScope(record([...shared, row('accept-2', 4, '验收', 'NEEDS_FIX')]), valueGraph, [], '', 3000, scope).conclusion)
      .toBe('未通过（修正 1 轮后验收仍要求修正，已到上限）');
  });

  it('两条并行验收链各修正一次只算一轮', () => {
    const valueGraph = graph([
      { id: 'exec-a', parents: [], role: '执行' },
      { id: 'accept-a-1', parents: ['exec-a'], role: '验收' },
      { id: 'fix-a', parents: ['accept-a-1'], role: '执行' },
      { id: 'accept-a-2', parents: ['fix-a'], role: '验收' },
      { id: 'exec-b', parents: [], role: '执行' },
      { id: 'accept-b-1', parents: ['exec-b'], role: '验收' },
      { id: 'fix-b', parents: ['accept-b-1'], role: '执行' },
      { id: 'accept-b-2', parents: ['fix-b'], role: '验收' },
    ]);
    const rows = [
      row('accept-a-1', 2, '验收', 'NEEDS_FIX'), row('fix-a', 3, '执行', ''),
      row('accept-a-2', 4, '验收', 'PASS'), row('accept-b-1', 2, '验收', 'NEEDS_FIX'),
      row('fix-b', 3, '执行', ''), row('accept-b-2', 4, '验收', 'PASS'),
    ];
    expect(finishWithScope(record(rows), valueGraph, [], '', 3000, new Set(['fix-a', 'accept-a-2', 'fix-b', 'accept-b-2'])).conclusion)
      .toBe('通过（修正 1 轮后通过）');
  });

  it('createConsoleApplyHandler 真实接线在一轮修正后写出通过结论', async () => {
    const preset: Preset = {
      id: 'task-33-apply', name: '经济模式', builtin: false,
      stages: [
        { nodes: [target('codex', 'exec-model', '执行')] },
        { nodes: [target('kimi', 'accept-model', '验收')] },
      ],
    };
    const messages: ConsoleMessage[] = [];
    let valueGraph: Graph | undefined;
    let busy = false;
    let acceptanceCalls = 0;
    let completed: ConsoleRunRecord | undefined;
    const handler = createConsoleApplyHandler({
      sessionId: 'console-tab', project: 'D:/work',
      snapshot: () => ({ busy, draft: '检查实现', messages, existingGraph: valueGraph }),
      append: (message) => messages.push(message),
      clearDraft: () => {},
      setBusy: (value) => { busy = value; },
      markApplied: () => {},
      commitGraph: (next) => { valueGraph = next; },
      rememberGraph: () => {},
      onOutcome: () => {},
      onRunUpdate: (run) => { if (run.status === 'completed') completed = run; },
      subscribeProgress: () => ({ dispose: () => {} }),
      io: {
        listDispatchableModels: async () => ({ models: [model('codex', 'exec-model'), model('kimi', 'accept-model')], error: '' }),
        loadRoles: async () => ({
          version: 1, planning: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
          execution: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
          acceptance: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
        }),
        gitChanges: async () => null,
        previewDispatch: async () => ({ totalRedacted: 0, broadRedacted: 0, nodes: [] }),
        dispatchGraphStage: async (request: DispatchStageRequest) => {
          const isAcceptance = request.nodes[0].role === '验收';
          if (isAcceptance) acceptanceCalls += 1;
          return {
            runId: request.runId,
            results: [resultFor(request, isAcceptance && acceptanceCalls === 1 ? 'NEEDS_FIX' : 'PASS')],
          };
        },
        saveGraph: async () => {},
      } as never,
    });

    handler(new CustomEvent('aicross:console-apply-preset', { detail: { sessionId: 'console-tab', preset } }));
    await vi.waitFor(() => expect(busy).toBe(false));

    expect(completed?.conclusion).toBe('通过（修正 1 轮后通过）');
    expect(messages.at(-1)?.text).toContain('结论：通过（修正 1 轮后通过）。');
  });

  it('详情页 Tab 与 Shift+Tab 的纯判定始终留在页内', async () => {
    const helpers = await import('../console-run-detail') as unknown as {
      runDetailTabTarget: (activeIndex: number, count: number, shiftKey: boolean) => number | null;
      runDetailTabPlan: (activeIndex: number, count: number, shiftKey: boolean) => { kind: 'item'; index: number } | { kind: 'root' };
      runDetailFocusTarget: (previousConnected: boolean) => 'previous' | 'composer';
    };
    expect(helpers.runDetailTabTarget(-1, 2, false)).toBe(0);
    expect(helpers.runDetailTabTarget(0, 2, false)).toBe(1);
    expect(helpers.runDetailTabTarget(1, 2, false)).toBe(0);
    expect(helpers.runDetailTabTarget(0, 2, true)).toBe(1);
    expect(helpers.runDetailTabTarget(0, 0, false)).toBeNull();
    expect(helpers.runDetailTabPlan(0, 0, false)).toEqual({ kind: 'root' });
    expect(helpers.runDetailFocusTarget(true)).toBe('previous');
    expect(helpers.runDetailFocusTarget(false)).toBe('composer');
  });

  it('首条心跳前保持排队中且不累计耗时', () => {
    const running = record([]);
    const staged = withConsoleRunStage(running, {
      runId: 'run', stage: 1, projectDir: 'D:/work', settings: {} as never,
      nodes: [{ nodeId: 'queued', role: '执行', agent: 'codex', provider: '', model: 'model' } as never],
    }, 2000);
    expect(staged.rows[0].status).toBe('pending');
    expect(runRowStatusLabel(staged.rows[0].status)).toBe('排队中');
    expect(liveConsoleRunDurationMs(staged.rows[0], 9000)).toBe(0);
  });

  it('外部通道连续两次事件时，第二次 NEEDS_FIX 后派出修正执行', async () => {
    const preset: Preset = {
      id: 'task-33-channel', name: '经济模式', builtin: false,
      stages: [
        { nodes: [target('codex', 'exec-model', '执行')] },
        { nodes: [target('kimi', 'accept-model', '验收')] },
      ],
    };
    let state = { graphs: {} as Record<string, Graph>, terminals: [], activeTerminalId: null };
    const requests: DispatchStageRequest[] = [];
    const acceptanceCalls = new Map<string, number>();
    const io = {
      listDispatchableModels: async () => ({ models: [model('codex', 'exec-model'), model('kimi', 'accept-model')], error: '' }),
      listPresets: async () => ({ version: 1 as const, presets: [preset] }),
      loadRoles: async () => ({
        version: 1, planning: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
        execution: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
        acceptance: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
      }),
      listConsoleRuns: async () => [],
      gitChanges: async () => null,
      previewDispatch: async () => ({ totalRedacted: 0, broadRedacted: 0, nodes: [] }),
      dispatchGraphStage: async (request: DispatchStageRequest) => {
        requests.push(request);
        const node = request.nodes[0];
        if (node.role !== '验收') return { runId: request.runId, results: [resultFor(request, '完成')] };
        const count = (acceptanceCalls.get(request.runId) ?? 0) + 1;
        acceptanceCalls.set(request.runId, count);
        const needsFix = request.runId === 'channel-2' && count === 1;
        return { runId: request.runId, results: [resultFor(request, needsFix ? 'NEEDS_FIX' : 'APPROVED')] };
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

    await runConsoleDispatch({ run_id: 'channel-1', project: 'D:/work', text: '第一次', preset: preset.id, attachments: [], reason: '' }, context as never, io as never);
    await runConsoleDispatch({ run_id: 'channel-2', project: 'D:/work', text: '第二次', preset: preset.id, attachments: [], reason: '' }, context as never, io as never);

    const second = requests.filter((request) => request.runId === 'channel-2');
    expect(second.map((request) => request.nodes[0].role)).toEqual(['执行', '验收', '执行', '验收']);
  });

  it('滚动恢复标记只跳过下一次自动滚底并随即清除', async () => {
    const helpers = await import('../console-run-detail') as unknown as {
      consoleAutoScrollStep: (skipNext: boolean) => { shouldScroll: boolean; skipNext: boolean };
    };
    const skipped = helpers.consoleAutoScrollStep(true);
    expect(skipped).toEqual({ shouldScroll: false, skipNext: false });
    expect(helpers.consoleAutoScrollStep(skipped.skipNext)).toEqual({ shouldScroll: true, skipNext: false });
    const source = readFileSync(new URL('../components/center/ConsoleChat.tsx', import.meta.url), 'utf8');
    expect(source).toContain('skipAutoScrollRef.current = true');
  });

  it('正在查看的记录被裁剪后主动关闭详情并恢复聊天', async () => {
    const helpers = await import('../console-run-detail') as unknown as {
      runDetailAvailabilityAction: (openRunId: string, recordAvailable: boolean) => 'close' | null;
    };
    expect(helpers.runDetailAvailabilityAction('', false)).toBeNull();
    expect(helpers.runDetailAvailabilityAction('run-1', true)).toBeNull();
    expect(helpers.runDetailAvailabilityAction('run-1', false)).toBe('close');
    const source = readFileSync(new URL('../components/center/ConsoleChat.tsx', import.meta.url), 'utf8');
    expect(source).toContain("runDetailAvailabilityAction(openRunId, Boolean(openRun)) === 'close'");
  });
});
