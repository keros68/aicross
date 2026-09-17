import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  dispatchConsolePreset,
  dispatchConsoleTurnWithFallback,
} from '../console-chat';
import {
  CONSOLE_MESSAGE_LIMIT,
  CONSOLE_SESSIONS_STORAGE_KEY,
  loadConsoleSessions,
  persistConsoleSessions,
} from '../console-persistence';
import { consoleResumeKey, type ConsoleMessage, type ConsoleTargetAgent } from '../console-session';
import { getInitialState, reducer, type TerminalSession } from '../store/app-state';
import type { DispatchNodeResult, DispatchStageRequest } from '../tauri';
import type { DispatchableModel, Preset, Target } from '../graph/types';

class MemoryStorage implements Storage {
  private values = new Map<string, string>();
  get length() { return this.values.size; }
  clear() { this.values.clear(); }
  getItem(key: string) { return this.values.get(key) ?? null; }
  key(index: number) { return [...this.values.keys()][index] ?? null; }
  removeItem(key: string) { this.values.delete(key); }
  setItem(key: string, value: string) { this.values.set(key, String(value)); }
}

const pi: ConsoleTargetAgent = { agent: 'pi', provider: 'zai', model: 'glm-5.3-flash' };
const codex: ConsoleTargetAgent = { agent: 'codex', provider: '', model: 'gpt-5.6-sol' };

function target(value: ConsoleTargetAgent, role: string): Target {
  return {
    ...value,
    role,
    tier: 'pro',
    thinking: '中',
    visibility: '共享',
    write: '只读',
  };
}

function model(value: ConsoleTargetAgent): DispatchableModel {
  return {
    id: value.model,
    agent: value.agent,
    provider: value.provider,
    vendor: value.provider,
    billing_source: '',
    tier: 'pro',
    thinking: '中',
    status: 'ready',
    verification: 'verified',
    source: '',
    smoked_at: '',
    smoke_model: '',
    smoke_failure: '',
    meta: '',
    note: '',
  };
}

function result(request: DispatchStageRequest, status: 'ok' | 'timeout'): DispatchNodeResult {
  const node = request.nodes[0];
  return {
    nodeId: node.nodeId,
    stage: request.stage,
    status,
    text: status === 'ok' ? '第一阶段正文' : '',
    rawStdout: '',
    rawStderr: '',
    modelReported: status === 'ok' ? 'kimi-k2' : node.model,
    sessionId: '',
    tokensIn: 0,
    tokensOut: 0,
    cacheRead: 0,
    durationMs: status === 'ok' ? 1200 : 2500,
    exitCode: status === 'ok' ? 0 : null,
    timedOut: status === 'timeout',
    truncated: false,
    redacted: 0,
    isolation: '完整',
    runner: 'oneshot',
    workdir: '',
    dispatchFile: '',
    frontmatter: '',
    outputPreview: '',
    error: status === 'timeout' ? '等待模型响应超时' : '',
    artifacts: null,
    validationOutput: '',
    fallbackFrom: status === 'ok' ? `${node.agent}/${node.model}` : '',
    fallbackTo: status === 'ok' ? 'kimi/kimi-k2' : '',
    fallbackSkipped: '',
  };
}

describe('TASK-25 控制台进度回流', () => {
  beforeEach(() => {
    vi.stubGlobal('localStorage', new MemoryStorage());
    let id = 0;
    vi.stubGlobal('crypto', { randomUUID: () => `id-${++id}` });
  });

  afterEach(() => vi.unstubAllGlobals());

  it('真实模式接线按阶段回流，累积结果不重复，失败与收尾都有说明', async () => {
    const preset: Preset = {
      id: 'task-25-two-stage',
      name: '自定义模式',
      builtin: false,
      stages: [
        { nodes: [target(pi, '出方案')] },
        { nodes: [target(codex, '顾问')] },
      ],
    };
    const messages: ConsoleMessage[] = [];
    const stages: number[] = [];
    const ticks = [1000, 4600];
    const outcome = await dispatchConsolePreset('D:/work', '检查实现', preset, () => {}, undefined, {
      listDispatchableModels: async () => ({ models: [model(pi), model(codex)], error: '' }),
      loadRoles: async () => ({
        version: 1,
        planning: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
        execution: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
        acceptance: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
      }),
      gitChanges: async () => { throw new Error('非 git 目录'); },
      previewDispatch: async () => ({ totalRedacted: 0, broadRedacted: 0, nodes: [] }),
      dispatchGraphStage: async (request: DispatchStageRequest) => {
        stages.push(request.stage);
        return { runId: request.runId, results: [result(request, request.stage === 1 ? 'ok' : 'timeout')] };
      },
      saveGraph: async () => {},
    } as any, {
      append: (message) => messages.push(message),
      now: () => ticks.shift() ?? 4600,
    });

    expect(stages).toEqual([1, 2]);
    expect(outcome.results).toHaveLength(2);
    expect(messages.map((message) => message.kind)).toEqual([
      'system', 'assistant', 'system', 'assistant', 'system',
    ]);
    expect(messages[0].text).toBe('第 1 阶段 · 派出 1 路：出方案（pi / glm-5.3-flash）');
    expect(messages[1]).toMatchObject({
      agent: 'kimi',
      model: 'kimi-k2',
      role: '出方案',
      durationMs: 1200,
      text: '第一阶段正文',
    });
    expect(messages[2].text).toBe('第 2 阶段 · 派出 1 路：顾问（codex / gpt-5.6-sol）');
    expect(messages[3].text).toContain('状态：超时');
    expect(messages[3].text).toContain('原因：等待模型响应超时');
    expect(messages.filter((message) => message.text === '第一阶段正文')).toHaveLength(1);
    expect(messages[4].text).toBe('模式派发结束：成功 1 路，失败 1 路，总耗时 3.6 秒。');
  });

  it('修正轮达到总请求上限时，收尾消息写明停止原因', async () => {
    localStorage.setItem('aicross-dispatch-settings', JSON.stringify({ maxRequests: 2, fixRoundLimit: 1 }));
    const preset: Preset = {
      id: 'task-25-limit',
      name: '自定义模式',
      builtin: false,
      stages: [
        { nodes: [target(pi, '执行')] },
        { nodes: [target(codex, '验收')] },
      ],
    };
    const messages: ConsoleMessage[] = [];
    let dispatched = 0;
    const outcome = await dispatchConsolePreset('D:/work', '检查停止原因', preset, () => {}, undefined, {
      listDispatchableModels: async () => ({ models: [model(pi), model(codex)], error: '' }),
      loadRoles: async () => ({
        version: 1,
        planning: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
        execution: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
        acceptance: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
      }),
      gitChanges: async () => null,
      previewDispatch: async () => ({ totalRedacted: 0, broadRedacted: 0, nodes: [] }),
      dispatchGraphStage: async (request: DispatchStageRequest) => {
        dispatched += 1;
        const value = result(request, dispatched === 1 ? 'timeout' : 'ok');
        return {
          runId: request.runId,
          results: [dispatched === 1 ? {
            ...value,
            status: 'validation_failed',
            timedOut: false,
            error: '验证命令退出 1',
            validationOutput: '1 failed',
          } : value],
        };
      },
      saveGraph: async () => {},
    } as any, { append: (message) => messages.push(message) });

    expect(dispatched).toBe(2);
    expect(outcome.error).toBe('已达总请求上限 2，继续派发需重新确认。');
    expect(messages.at(-1)?.text).toContain('停止原因：已达总请求上限 2，继续派发需重新确认。');
  });

  it('最终保存失败时仍返回流水线结果并写入收尾消息', async () => {
    const preset: Preset = {
      id: 'task-25-save-failure',
      name: '自定义模式',
      builtin: false,
      stages: [{ nodes: [target(pi, '执行')] }],
    };
    const messages: ConsoleMessage[] = [];
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const outcome = await dispatchConsolePreset('D:/work', '检查保存失败', preset, () => {}, undefined, {
      listDispatchableModels: async () => ({ models: [model(pi)], error: '' }),
      loadRoles: async () => ({
        version: 1,
        planning: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
        execution: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
        acceptance: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
      }),
      gitChanges: async () => null,
      previewDispatch: async () => ({ totalRedacted: 0, broadRedacted: 0, nodes: [] }),
      dispatchGraphStage: async (request: DispatchStageRequest) => ({
        runId: request.runId,
        results: [result(request, 'ok')],
      }),
      saveGraph: async () => { throw new Error('disk full'); },
    } as any, { append: (message) => messages.push(message) });

    expect(outcome.results).toHaveLength(1);
    expect(messages.at(-1)?.text).toContain('模式派发结束：成功 1 路，失败 0 路');
    warn.mockRestore();
  });

  it('同一节点修正后的末次状态变化时补发更正消息，收尾按末次计数', async () => {
    const preset: Preset = {
      id: 'task-25-corrected-result',
      name: '自定义模式',
      builtin: false,
      stages: [
        { nodes: [target(pi, '执行')] },
        { nodes: [target(codex, '顾问')] },
      ],
    };
    const messages: ConsoleMessage[] = [];
    let firstNodeId = '';
    const outcome = await dispatchConsolePreset('D:/work', '检查末次状态', preset, () => {}, undefined, {
      listDispatchableModels: async () => ({ models: [model(pi), model(codex)], error: '' }),
      loadRoles: async () => ({
        version: 1,
        planning: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
        execution: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
        acceptance: { agent: '', provider: '', model: '', tier: '', thinking: '中' },
      }),
      gitChanges: async () => null,
      previewDispatch: async () => ({ totalRedacted: 0, broadRedacted: 0, nodes: [] }),
      dispatchGraphStage: async (request: DispatchStageRequest) => {
        if (!firstNodeId) {
          firstNodeId = request.nodes[0].nodeId;
          return { runId: request.runId, results: [result(request, 'ok')] };
        }
        return {
          runId: request.runId,
          results: [{
            ...result(request, 'timeout'),
            nodeId: firstNodeId,
            error: '修正后仍未通过',
          }],
        };
      },
      saveGraph: async () => {},
    } as any, { append: (message) => messages.push(message) });

    expect(outcome.results.map((item) => item.result.status)).toEqual(['ok', 'timeout']);
    expect(messages.some((message) => message.text.startsWith('执行 · 修正后超时'))).toBe(true);
    expect(messages.at(-1)?.text).toContain('成功 0 路，失败 1 路');
  });
});

describe('TASK-25 控制台会话持久化', () => {
  let storage: MemoryStorage;

  beforeEach(() => {
    storage = new MemoryStorage();
    vi.stubGlobal('localStorage', storage);
    let id = 0;
    vi.stubGlobal('crypto', { randomUUID: () => `restart-${++id}` });
  });

  afterEach(() => vi.unstubAllGlobals());

  it('通过实际 reducer 写入，并由初始化链路模拟重启恢复控制台负载', () => {
    let state = getInitialState();
    const id = state.terminals[0].id;
    state = reducer(state, { type: 'START_CONSOLE', id, folderPath: 'D:/work', targetAgent: pi });
    state = reducer(state, {
      type: 'APPEND_CONSOLE_MESSAGE',
      id,
      message: { id: 'm1', kind: 'user', text: '继续检查', createdAt: '2026-09-17T00:00:00Z' },
    });
    state = reducer(state, {
      type: 'SET_CONSOLE_RESUME_ID', id, key: consoleResumeKey(pi), resumeId: 'thread-25',
    });
    state = reducer(state, { type: 'SET_CONSOLE_APPLIED_MESSAGE', id, messageId: 'm1' });
    reducer(state, { type: 'SET_CONSOLE_GRAPH_ID', id, graphId: 'graph-25' });

    const restarted = getInitialState();
    expect(restarted.terminals).toHaveLength(1);
    expect(restarted.terminals[0]).toMatchObject({
      id,
      folderPath: 'D:/work',
      targetAgent: pi,
      messages: [{ id: 'm1', kind: 'user', text: '继续检查' }],
      resumeId: { [consoleResumeKey(pi)]: 'thread-25' },
      lastAppliedMessageId: 'm1',
      consoleGraphId: 'graph-25',
    });
  });

  it('存储不可用时不抛错，消息超过上限时只恢复最近 200 条', () => {
    const messages = Array.from({ length: CONSOLE_MESSAGE_LIMIT + 5 }, (_, index): ConsoleMessage => ({
      id: `m${index}`,
      kind: 'system',
      text: `消息 ${index}`,
      createdAt: '2026-09-17T00:00:00Z',
    }));
    const session: TerminalSession = {
      id: 'bounded', tool: 'console', folderPath: null, targetAgent: codex, messages, resumeId: {},
    };
    persistConsoleSessions([session]);
    expect(loadConsoleSessions()[0].messages).toHaveLength(CONSOLE_MESSAGE_LIMIT);
    expect(loadConsoleSessions()[0].messages?.[0].id).toBe('m5');
    expect(JSON.parse(storage.getItem(CONSOLE_SESSIONS_STORAGE_KEY) ?? '[]')[0].messages).toHaveLength(CONSOLE_MESSAGE_LIMIT);

    const broken = {
      getItem: () => { throw new Error('blocked'); },
      setItem: () => { throw new Error('full'); },
    } as unknown as Storage;
    expect(() => persistConsoleSessions([session], broken)).not.toThrow();
    expect(() => loadConsoleSessions(broken)).not.toThrow();
  });
});

describe('TASK-25 失效续聊回退', () => {
  beforeEach(() => {
    vi.stubGlobal('localStorage', new MemoryStorage());
    let id = 0;
    vi.stubGlobal('crypto', { randomUUID: () => `turn-${++id}` });
  });

  afterEach(() => vi.unstubAllGlobals());

  it('实际发送链路在续聊报错后清除旧会话并无 sessionId 重派一次', async () => {
    const sentResumeIds: string[] = [];
    const expired = vi.fn();
    const turn = await dispatchConsoleTurnWithFallback('D:/work', '继续', codex, 'dead-thread', expired, {
      listDispatchableModels: async () => ({ models: [model(codex)], error: '' }),
      dispatchGraphStage: async (request: DispatchStageRequest) => {
        const resumeId = request.nodes[0].sessionId;
        sentResumeIds.push(resumeId);
        if (resumeId) throw new Error('session not found');
        return { runId: request.runId, results: [result(request, 'ok')] };
      },
    } as any);

    expect(sentResumeIds).toEqual(['dead-thread', '']);
    expect(expired).toHaveBeenCalledOnce();
    expect(turn.result.status).toBe('ok');
  });

  it('错误结果明确表示会话失效时也只回退一次', async () => {
    const sentResumeIds: string[] = [];
    const expired = vi.fn();
    const turn = await dispatchConsoleTurnWithFallback('D:/work', '继续', codex, 'dead-thread', expired, {
      listDispatchableModels: async () => ({ models: [model(codex)], error: '' }),
      dispatchGraphStage: async (request: DispatchStageRequest) => {
        const resumeId = request.nodes[0].sessionId;
        sentResumeIds.push(resumeId);
        return {
          runId: request.runId,
          results: [resumeId ? {
            ...result(request, 'timeout'),
            status: 'error',
            timedOut: false,
            error: 'Unknown conversation id',
          } : result(request, 'ok')],
        };
      },
    } as any);

    expect(sentResumeIds).toEqual(['dead-thread', '']);
    expect(expired).toHaveBeenCalledOnce();
    expect(turn.result.status).toBe('ok');
  });

  it('普通错误只发送一次，保留有效续聊 id', async () => {
    const sentResumeIds: string[] = [];
    const expired = vi.fn();
    const turn = await dispatchConsoleTurnWithFallback('D:/work', '继续', codex, 'live-thread', expired, {
      listDispatchableModels: async () => ({ models: [model(codex)], error: '' }),
      dispatchGraphStage: async (request: DispatchStageRequest) => {
        sentResumeIds.push(request.nodes[0].sessionId);
        return {
          runId: request.runId,
          results: [{
            ...result(request, 'timeout'),
            status: 'error',
            timedOut: false,
            error: 'HTTP 502',
          }],
        };
      },
    } as any);

    expect(sentResumeIds).toEqual(['live-thread']);
    expect(expired).not.toHaveBeenCalled();
    expect(turn.result.status).toBe('error');
  });

  it('发派前异常原样抛出，即使消息包含会话失效词也不回退', async () => {
    const expired = vi.fn();
    const listDispatchableModels = vi.fn(async () => { throw new Error('session not found'); });
    const dispatchGraphStage = vi.fn();

    await expect(dispatchConsoleTurnWithFallback(
      'D:/work', '继续', codex, 'live-thread', expired,
      { listDispatchableModels, dispatchGraphStage } as any,
    )).rejects.toThrow('session not found');
    expect(listDispatchableModels).toHaveBeenCalledOnce();
    expect(dispatchGraphStage).not.toHaveBeenCalled();
    expect(expired).not.toHaveBeenCalled();
  });
});
