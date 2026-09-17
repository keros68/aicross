import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
// @ts-expect-error node builtin，只有静态源码测试读取本地文件
import { readFileSync } from 'node:fs';
import { createConsoleApplyHandler, dispatchConsolePreset } from '../console-chat';
import {
  CONSOLE_RUN_LIMIT,
  consoleRunTotals,
  finishConsoleRun,
  liveConsoleRunDurationMs,
  parseConsoleRunRecord,
  reportedModelLabel,
  withConsoleRunProgress,
  type ConsoleRunRecord,
  type ConsoleRunRow,
} from '../console-run-detail';
import { CONSOLE_SESSIONS_STORAGE_KEY, loadConsoleSessions, persistConsoleSessions } from '../console-persistence';
import { getInitialState, reducer, type TerminalSession } from '../store/app-state';
import type { ConsoleMessage } from '../console-session';
import type { DispatchNodeResult, DispatchProgress, DispatchStageRequest } from '../tauri';
import type { DispatchableModel, Preset, Target } from '../graph/types';

class MemoryStorage implements Storage {
  private values = new Map<string, string>();
  writeCount = 0;
  get length() { return this.values.size; }
  clear() { this.values.clear(); }
  getItem(key: string) { return this.values.get(key) ?? null; }
  key(index: number) { return [...this.values.keys()][index] ?? null; }
  removeItem(key: string) { this.values.delete(key); }
  setItem(key: string, value: string) {
    this.writeCount += 1;
    this.values.set(key, String(value));
  }
}

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

function result(request: DispatchStageRequest): DispatchNodeResult {
  const node = request.nodes[0];
  const acceptance = node.role === '验收';
  return {
    nodeId: node.nodeId,
    stage: request.stage,
    status: 'ok',
    text: acceptance ? '仍有问题\nNEEDS_FIX' : '已完成',
    rawStdout: '', rawStderr: '', modelReported: acceptance ? '' : 'backup-model', sessionId: '',
    tokensIn: acceptance ? 0 : 120, tokensOut: acceptance ? 0 : 30, cacheRead: acceptance ? 0 : 12,
    durationMs: acceptance ? 700 : 900,
    exitCode: 0, timedOut: false, truncated: false, redacted: 0, isolation: '完整', runner: 'oneshot',
    workdir: '', dispatchFile: '', frontmatter: '', outputPreview: '', error: '', artifacts: null,
    validationOutput: '',
    fallbackFrom: acceptance ? '' : `${node.agent}/${node.model}`,
    fallbackTo: acceptance ? '' : 'kimi/backup-model',
    fallbackSkipped: '',
  };
}

function row(overrides: Partial<ConsoleRunRow> = {}): ConsoleRunRow {
  return {
    nodeId: 'n1', stage: 1, role: '执行', agent: 'codex', provider: '', requestedModel: 'gpt',
    modelReported: 'gpt', status: 'ok', acceptance: '', tokensIn: 10, tokensOut: 5, cacheRead: 3,
    durationMs: 800, fallbackFrom: '', fallbackTo: '', progressElapsedMs: 0, progressAt: 0,
    toolCalls: 0, lastTool: '', ...overrides,
  };
}

function record(overrides: Partial<ConsoleRunRecord> = {}): ConsoleRunRecord {
  return {
    id: 'run-1', presetName: '经济模式', startedAt: 1000, updatedAt: 5000, finishedAt: 5000,
    wallDurationMs: 4000, status: 'completed', conclusion: '通过（第 1 轮验收通过）', error: '',
    rows: [row()], ...overrides,
  };
}

describe('TASK-27 控制台运行记录真实接线', () => {
  let storage: MemoryStorage;

  beforeEach(() => {
    storage = new MemoryStorage();
    vi.stubGlobal('localStorage', storage);
    let id = 0;
    vi.stubGlobal('crypto', { randomUUID: () => `id-${++id}` });
    class TestCustomEvent<T> extends Event {
      detail: T;
      constructor(type: string, init: CustomEventInit<T>) { super(type); this.detail = init.detail as T; }
    }
    vi.stubGlobal('CustomEvent', TestCustomEvent);
    localStorage.setItem('aicross-dispatch-settings', JSON.stringify({ fixRoundLimit: 0 }));
  });

  afterEach(() => vi.unstubAllGlobals());

  it('apply handler 驱动两阶段派发、收尾结论与持久化恢复同一份详情', async () => {
    const preset: Preset = {
      id: 'task-27', name: '自定义模式', builtin: false,
      stages: [
        { nodes: [target('codex', 'main-model', '执行')] },
        { nodes: [target('kimi', 'kimi-k3', '验收')] },
      ],
    };
    let state = getInitialState();
    const id = state.terminals[0].id;
    state = reducer(state, {
      type: 'START_CONSOLE', id, folderPath: 'D:/work',
      targetAgent: { agent: 'codex', provider: '', model: 'main-model' },
    });
    let finish!: () => void;
    const completed = new Promise<void>((resolve) => { finish = resolve; });
    const ticks = [1000, 5000];
    const handler = createConsoleApplyHandler({
      sessionId: id,
      project: 'D:/work',
      snapshot: () => ({ busy: false, draft: '检查实现', messages: state.terminals[0].messages ?? [] }),
      append: (message: ConsoleMessage) => { state = reducer(state, { type: 'APPEND_CONSOLE_MESSAGE', id, message }); },
      clearDraft: () => {}, setBusy: () => {}, markApplied: () => {}, commitGraph: () => {}, rememberGraph: () => {},
      onRunUpdate: (run: ConsoleRunRecord) => {
        state = reducer(state, { type: 'UPSERT_CONSOLE_RUN', id, run } as never);
      },
      onOutcome: () => finish(),
      io: {
        listDispatchableModels: async () => ({ models: [model('codex', 'main-model'), model('kimi', 'kimi-k3')], error: '' }),
        loadRoles: async () => ({
          version: 1,
          planning: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
          execution: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
          acceptance: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
        }),
        gitChanges: async () => null,
        previewDispatch: async () => ({ totalRedacted: 0, broadRedacted: 0, nodes: [] }),
        dispatchGraphStage: async (request: DispatchStageRequest) => ({ runId: request.runId, results: [result(request)] }),
        saveGraph: async () => {},
      } as never,
      now: () => ticks.shift() ?? 5000,
      subscribeProgress: () => ({ dispose: () => {} }),
    } as never);

    handler(new CustomEvent('aicross:console-apply-preset', { detail: { sessionId: id, preset } }));
    await completed;

    const session = state.terminals[0];
    expect(session.messages?.at(-1)).toMatchObject({
      runId: expect.stringMatching(/^console-mode-/),
      text: '结论：未通过（验收要求修正，已到上限）。\n共 2 路，运行失败 0 路，用时 4.0 秒。',
    });
    expect(session.runRecords).toHaveLength(1);
    expect(session.runRecords?.[0]).toMatchObject({ status: 'completed', wallDurationMs: 4000 });
    expect(session.runRecords?.[0].rows).toEqual(expect.arrayContaining([
      expect.objectContaining({ role: '执行', requestedModel: 'main-model', modelReported: 'backup-model', fallbackTo: 'kimi/backup-model', tokensIn: 120 }),
      expect.objectContaining({ role: '验收', acceptance: 'NEEDS_FIX', tokensIn: 0, tokensOut: 0 }),
    ]));

    const restarted = getInitialState();
    expect(restarted.terminals[0].runRecords?.[0]).toEqual(session.runRecords?.[0]);
  });
});

describe('TASK-27 运行详情口径', () => {
  it('阶段消息异常时终止运行记录并释放心跳订阅', async () => {
    const preset: Preset = {
      id: 'r1-cleanup', name: '经济模式', builtin: false,
      stages: [{ nodes: [target('codex', 'main-model', '执行')] }],
    };
    const updates: ConsoleRunRecord[] = [];
    const dispose = vi.fn();
    const io = {
      listDispatchableModels: async () => ({ models: [model('codex', 'main-model')], error: '' }),
      loadRoles: async () => ({
        version: 1,
        planning: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
        execution: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
        acceptance: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
      }),
      gitChanges: async () => null,
      previewDispatch: async () => ({ totalRedacted: 0, broadRedacted: 0, nodes: [] }),
      dispatchGraphStage: async (request: DispatchStageRequest) => ({ runId: request.runId, results: [result(request)] }),
      saveGraph: async () => {},
    };

    await expect(dispatchConsolePreset(
      'D:/work',
      '检查实现',
      preset,
      () => {},
      undefined,
      io as never,
      {
        append: (message) => {
          if (message.runId && message.text.startsWith('第 1 阶段')) throw new Error('阶段消息写入失败');
        },
        onRunUpdate: (run) => updates.push(run),
        subscribeProgress: () => ({ dispose }),
      },
    )).rejects.toThrow('阶段消息写入失败');

    expect(dispose).toHaveBeenCalledOnce();
    expect(updates.at(-1)?.status).not.toBe('running');
  });

  it('只更新本次运行的心跳且不写盘，阶段、结果与收尾仍写盘', async () => {
    const preset: Preset = {
      id: 'r1-progress', name: '经济模式', builtin: false,
      stages: [{ nodes: [target('codex', 'main-model', '执行')] }],
    };
    const storage = new MemoryStorage();
    vi.stubGlobal('localStorage', storage);
    localStorage.setItem('aicross-dispatch-settings', JSON.stringify({ fixRoundLimit: 0 }));
    let state = getInitialState();
    const id = state.terminals[0].id;
    state = reducer(state, {
      type: 'START_CONSOLE', id, folderPath: 'D:/work',
      targetAgent: { agent: 'codex', provider: '', model: 'main-model' },
    });
    storage.writeCount = 0;
    const durableUpdates: ConsoleRunRecord[] = [];
    const progressUpdates: ConsoleRunRecord[] = [];
    let emitProgress!: (payload: DispatchProgress) => void;
    let writesAroundHeartbeat: number[] = [];
    const io = {
      listDispatchableModels: async () => ({ models: [model('codex', 'main-model')], error: '' }),
      loadRoles: async () => ({
        version: 1,
        planning: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
        execution: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
        acceptance: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
      }),
      gitChanges: async () => null,
      previewDispatch: async () => ({ totalRedacted: 0, broadRedacted: 0, nodes: [] }),
      dispatchGraphStage: async (request: DispatchStageRequest) => {
        writesAroundHeartbeat = [storage.writeCount];
        const nodeId = request.nodes[0].nodeId;
        emitProgress({
          runId: 'another-run', nodeId, status: 'running', elapsedMs: 100,
          idleMs: 0, toolCalls: 1, repeatedToolCalls: 0, lastTool: 'Read',
        });
        writesAroundHeartbeat.push(storage.writeCount);
        emitProgress({
          runId: request.runId, nodeId, status: 'running', elapsedMs: 200,
          idleMs: 0, toolCalls: 2, repeatedToolCalls: 0, lastTool: 'Read',
        });
        writesAroundHeartbeat.push(storage.writeCount);
        return { runId: request.runId, results: [result(request)] };
      },
      saveGraph: async () => {},
    };

    await dispatchConsolePreset('D:/work', '检查实现', preset, () => {}, undefined, io as never, {
      append: () => {},
      onRunUpdate: (run: ConsoleRunRecord) => {
        durableUpdates.push(run);
        state = reducer(state, { type: 'UPSERT_CONSOLE_RUN', id, run });
      },
      onRunProgress: (run: ConsoleRunRecord) => {
        progressUpdates.push(run);
        state = reducer(state, { type: 'UPDATE_CONSOLE_RUN_PROGRESS', id, run });
      },
      subscribeProgress: (_runId: string, onEvent: (payload: DispatchProgress) => void) => {
        emitProgress = onEvent;
        return { dispose: () => {} };
      },
    });

    expect(writesAroundHeartbeat).toEqual([
      writesAroundHeartbeat[0], writesAroundHeartbeat[0], writesAroundHeartbeat[0],
    ]);
    expect(progressUpdates).toHaveLength(1);
    expect(progressUpdates[0].rows[0]).toMatchObject({ toolCalls: 2, lastTool: 'Read' });
    expect(durableUpdates).toEqual(expect.arrayContaining([
      expect.objectContaining({ status: 'running', rows: [expect.objectContaining({ status: 'pending' })] }),
      expect.objectContaining({ status: 'running', rows: [expect.objectContaining({ status: 'ok' })] }),
      expect.objectContaining({ status: 'completed' }),
    ]));
    expect(storage.writeCount).toBe(durableUpdates.length);
  });

  it('未报告 token 不进合计，并分开统计墙钟耗时与各路耗时和', () => {
    const value = record({
      rows: [
        row({ nodeId: 'reported', tokensIn: 100, tokensOut: 20, cacheRead: 8, durationMs: 7000 }),
        row({ nodeId: 'missing', tokensIn: 0, tokensOut: 0, cacheRead: 0, durationMs: 5000 }),
      ],
      wallDurationMs: 8000,
    });
    expect(consoleRunTotals(value, 99_000)).toEqual({
      routes: 2, tokensIn: 100, tokensOut: 20, cacheRead: 8,
      routeDurationMs: 12_000, wallDurationMs: 8000, unreported: 1,
    });
  });

  it('重启加载时把进行中记录和行冻结为已中断', () => {
    const running = record({
      status: 'running', finishedAt: undefined, wallDurationMs: undefined, updatedAt: 6000,
      rows: [row({ status: 'running', durationMs: 0, progressElapsedMs: 2500, progressAt: 5500 })],
    });
    const storage = new MemoryStorage();
    persistConsoleSessions([{
      id: 'console', tool: 'console', folderPath: 'D:/work', messages: [], resumeId: {}, runRecords: [running],
    } as TerminalSession], storage);
    const loaded = loadConsoleSessions(storage)[0].runRecords?.[0];
    expect(loaded).toMatchObject({ status: 'interrupted', finishedAt: 6000, wallDurationMs: 5000 });
    expect(loaded?.rows[0]).toMatchObject({ status: 'interrupted', durationMs: 3000 });
  });

  it('旧会话没有运行记录字段时仍正常加载', () => {
    const storage = new MemoryStorage();
    storage.setItem(CONSOLE_SESSIONS_STORAGE_KEY, JSON.stringify([{
      id: 'old', tool: 'console', folderPath: null, messages: [], resumeId: {},
    }]));
    expect(loadConsoleSessions(storage)).toEqual([expect.objectContaining({ id: 'old', runRecords: [] })]);
  });

  it('每个会话只保存最近 20 份运行记录', () => {
    const storage = new MemoryStorage();
    const runRecords = Array.from({ length: CONSOLE_RUN_LIMIT + 3 }, (_, index) => record({ id: `run-${index}` }));
    persistConsoleSessions([{
      id: 'bounded', tool: 'console', folderPath: null, messages: [], resumeId: {}, runRecords,
    } as TerminalSession], storage);
    const loaded = loadConsoleSessions(storage)[0].runRecords ?? [];
    expect(loaded).toHaveLength(CONSOLE_RUN_LIMIT);
    expect(loaded[0].id).toBe('run-3');
  });

  it('实际模型缺失与备选路由如实展示', () => {
    expect(reportedModelLabel(row({ agent: 'kimi', modelReported: '' }))).toBe('kimi 不报真身');
    expect(reportedModelLabel(row({ agent: 'pi', modelReported: '' }))).toBe('未报告');
    expect(reportedModelLabel(row({ modelReported: 'gpt-5.6-sol' }))).toBe('gpt-5.6-sol');
  });

  it('心跳落到进行中行，两次心跳之间按前端时钟继续计时', () => {
    const running = record({
      status: 'running', finishedAt: undefined, wallDurationMs: undefined,
      rows: [row({ status: 'pending', durationMs: 0, modelReported: '' })],
    });
    const next = withConsoleRunProgress(running, {
      runId: 'run-1', nodeId: 'n1', status: 'running', elapsedMs: 3000,
      idleMs: 400, toolCalls: 2, repeatedToolCalls: 1, lastTool: 'Read',
    }, 4000);
    expect(next.rows[0]).toMatchObject({
      status: 'running', progressElapsedMs: 3000, progressAt: 4000, toolCalls: 2, lastTool: 'Read',
    });
    expect(liveConsoleRunDurationMs(next.rows[0], 5200)).toBe(4200);
  });

  it('Esc 按键判定排除输入法组合与其他按键', async () => {
    const helpers = await import('../console-run-detail');
    const runDetailKeyAction = (helpers as unknown as {
      runDetailKeyAction: (event: { key: string; isComposing?: boolean }) => 'close' | null;
    }).runDetailKeyAction;
    expect(runDetailKeyAction({ key: 'Escape' })).toBe('close');
    expect(runDetailKeyAction({ key: 'Escape', isComposing: true })).toBeNull();
    expect(runDetailKeyAction({ key: 'Enter' })).toBeNull();
  });

  it('最终结论区分通过、无法识别、中途停止和无验收模式', () => {
    const graph = { nodes: {} } as never;
    expect(finishConsoleRun(record({ rows: [row({ role: '验收', acceptance: 'PASS' })] }), graph, [], '', 5000).conclusion)
      .toBe('通过（验收一次通过）');
    expect(finishConsoleRun(record({ rows: [
      row({ nodeId: 'a', role: '验收', acceptance: 'PASS' }),
      row({ nodeId: 'b', role: '验证', acceptance: 'UNKNOWN' }),
    ] }), graph, [], '', 5000).conclusion).toBe('验收结论无法识别（验收一次）');
    expect(finishConsoleRun(record({ rows: [
      row({ nodeId: 'a', role: '验收', acceptance: 'PASS' }),
      row({ nodeId: 'b', role: '验证', acceptance: 'NEEDS_FIX' }),
    ] }), graph, [], '', 5000).conclusion).toBe('未通过（验收要求修正，已到上限）');
    expect(finishConsoleRun(record(), graph, [], '用户叫停', 5000).conclusion)
      .toBe('中途停止（原因：用户叫停）');
    expect(finishConsoleRun(record({ rows: [row({ role: '执行', acceptance: '' })] }), graph, [], '', 5000).conclusion)
      .toBe('已完成（本模式不含验收）');
  });

  it('持久化解析拒绝坏记录，不让旧数据拖垮整个会话', () => {
    expect(parseConsoleRunRecord({ id: '', rows: [] })).toBeNull();
    expect(parseConsoleRunRecord(record())).toMatchObject({ id: 'run-1', rows: [{ nodeId: 'n1' }] });
  });

  it('详情页是可由 Esc 退出的满版对话区域，并保留返回入口', () => {
    const source = readFileSync(new URL('../components/center/ConsoleRunDetail.tsx', import.meta.url), 'utf8');
    const chat = readFileSync(new URL('../components/center/ConsoleChat.tsx', import.meta.url), 'utf8');
    expect(source).toContain('role="dialog"');
    expect(source).toContain('aria-modal="true"');
    expect(source).toContain('runDetailKeyAction(event)');
    expect(source).toContain('返回对话');
    expect(source).toContain('路未报告，未计入');
    expect(chat).toContain('查看进度');
    expect(chat).toContain('查看详情');
    expect(chat).toContain('message.runId');
  });

  it('运行记录构造器不保留未使用的图与节点参数', () => {
    const source = readFileSync(new URL('../console-run-detail.ts', import.meta.url), 'utf8');
    expect(source).not.toContain('_graph: Graph');
    expect(source).not.toContain('_nodeIds: string[]');
  });
});
