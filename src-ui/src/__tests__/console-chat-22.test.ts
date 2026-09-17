import { describe, expect, it, vi } from 'vitest';
// @ts-expect-error node builtin，只有归档防篡改扫描读本地文件
import { readdirSync, readFileSync } from 'node:fs';
import { getInitialState, reducer, type TerminalSession } from '../store/app-state';
import { fillRoot } from '../graph/mutate';
import { newGraph, type Preset } from '../graph/types';
import { createConsoleApplyHandler, dispatchConsolePreset, dispatchConsoleTurn } from '../console-chat';
import {
  NEW_TAB_MENU_ITEMS,
  canApplyOrchestration,
  completeConsoleMention,
  consoleResumeId,
  consoleResumeKey,
  dispatchConsoleApply,
  mentionTargets,
  parseConsoleMention,
  selectConsoleApplyMessage,
  type ConsoleMessage,
  type ConsoleTargetAgent,
} from '../console-session';

const codex: ConsoleTargetAgent = { agent: 'codex', provider: '', model: 'gpt-5.6-sol' };
const kimi: ConsoleTargetAgent = { agent: 'kimi', provider: '', model: 'kimi-code/k3' };
const preset: Preset = { id: 'p', name: '经济', builtin: true, stages: [] };

describe('TASK-22 控制台消息流', () => {
  it('@ 显示名只解析认识的前缀；未知名称与普通文本原样发送', () => {
    expect(parseConsoleMention('@KIMI CODE 帮我复核', [codex, kimi])).toEqual({
      target: kimi,
      text: '帮我复核',
      recognized: true,
    });
    expect(parseConsoleMention('@kimi 帮我复核', [codex, kimi])).toEqual({
      target: kimi,
      text: '帮我复核',
      recognized: true,
    });
    expect(parseConsoleMention('@CLAUDE 帮我复核', [
      codex,
      { agent: 'claude', provider: '', model: 'sonnet' },
    ])).toMatchObject({ text: '帮我复核', recognized: true });
    expect(parseConsoleMention('@pi 帮我复核', [
      codex,
      { agent: 'pi', provider: 'openai', model: 'gpt' },
    ])).toMatchObject({ text: '帮我复核', recognized: true });
    expect(parseConsoleMention('@kimicode 帮我复核', [codex, kimi]).recognized).toBe(false);
    expect(parseConsoleMention('@Unknown 帮我复核', [codex, kimi])).toEqual({
      target: null,
      text: '@Unknown 帮我复核',
      recognized: false,
    });
    expect(parseConsoleMention('帮我复核', [codex, kimi])).toEqual({
      target: null,
      text: '帮我复核',
      recognized: false,
    });
  });

  it('补全只替换开头的 @ 段并保留正文', () => {
    expect(completeConsoleMention('@Kimi 帮我重构这段', kimi)).toBe('@Kimi Code 帮我重构这段');
    expect(completeConsoleMention('@Kimi Code 帮我复核', kimi)).toBe('@Kimi Code 帮我复核');
  });

  it('@ 改派优先复用当前 tab 最近用过的模型', () => {
    const models = [
      { agent: 'kimi', provider: 'moonshot', id: 'first', status: 'ready', verification: 'verified' },
      { agent: 'kimi', provider: 'moonshot', id: 'recent', status: 'ready', verification: 'verified' },
    ] as any;
    expect(mentionTargets(models, [{ agent: 'kimi', provider: 'moonshot', model: 'recent' }]))
      .toEqual([{ agent: 'kimi', provider: 'moonshot', model: 'recent' }]);
    expect(mentionTargets(models, [{ agent: 'kimi', provider: '', model: 'recent' }]))
      .toEqual([{ agent: 'kimi', provider: 'moonshot', model: 'recent' }]);
  });

  it('同一 tab 按目标分别存取续聊 id', () => {
    vi.stubGlobal('localStorage', { getItem: () => null, setItem: () => {}, removeItem: () => {} });
    vi.stubGlobal('crypto', { randomUUID: () => 'console-tab' });
    const state = getInitialState();
    const key = consoleResumeKey(codex);
    const next = reducer(state, {
      type: 'SET_CONSOLE_RESUME_ID',
      id: state.terminals[0].id,
      key,
      resumeId: 'thread-1',
    });
    expect(consoleResumeId(next.terminals[0].resumeId, codex)).toBe('thread-1');
    expect(consoleResumeId(next.terminals[0].resumeId, kimi)).toBe('');
  });

  it('控制台 tab 应用模式不依赖 graphFocus', () => {
    const session: TerminalSession = {
      id: 'console-tab',
      tool: 'console',
      folderPath: 'D:/work',
      targetAgent: codex,
    };
    expect(canApplyOrchestration(session, false, preset)).toBe(true);
    expect(canApplyOrchestration({ ...session, tool: 'graph' }, false, preset)).toBe(false);
  });

  it('新建菜单只有控制台一项', () => {
    expect(NEW_TAB_MENU_ITEMS).toEqual([{ kind: 'console', label: '控制台' }]);
  });

  it('单线派发透传原文、续聊 id，并生成独立 runId', async () => {
    vi.stubGlobal('crypto', { randomUUID: vi.fn()
      .mockReturnValueOnce('node-1')
      .mockReturnValueOnce('run-1') });
    let request: any;
    const dispatchGraphStage = vi.fn(async (value: any) => {
      request = value;
      return { results: [{
      nodeId: 'node-1', status: 'ok', text: '完成', sessionId: 'thread-2', durationMs: 1,
      fallbackTo: '', modelReported: 'gpt-5.6-sol', error: '',
      }] };
    });
    await dispatchConsoleTurn('D:/work', '原文\n第二行', codex, 'thread-1', {
      listDispatchableModels: async () => ({ models: [{
        id: codex.model, agent: codex.agent, provider: '', vendor: 'openai', billing_source: '',
        tier: 'pro', thinking: '中', status: 'ready', verification: 'native',
      }], error: '' }),
      dispatchGraphStage,
    } as any);
    expect(request.runId).toBe('console-chat-run-1');
    expect(request.nodes).toHaveLength(1);
    expect(request.nodes[0]).toMatchObject({ prompt: '原文\n第二行', sessionId: 'thread-1' });
  });

  it('应用事件匹配 tab，拦截忙碌与空文本，并接到模式派发', async () => {
    class TestCustomEvent<T> extends Event {
      detail: T;
      constructor(type: string, init: CustomEventInit<T>) {
        super(type);
        this.detail = init.detail as T;
      }
    }
    vi.stubGlobal('CustomEvent', TestCustomEvent);
    vi.stubGlobal('crypto', { randomUUID: () => 'message-1' });
    const bus = new EventTarget();
    const appended: ConsoleMessage[] = [];
    const graph = newGraph('D:/work');
    const dispatchPreset = vi.fn(async (...args: any[]) => {
      args[3](graph);
      args[6].append({ id: 'feedback', kind: 'system', text: '进度已回流', createdAt: '' });
      return { graph, results: [], error: '' };
    });
    let busy = false;
    let draft = '';
    let remembered = '';
    const handler = createConsoleApplyHandler({
      sessionId: 'console-tab',
      project: 'D:/work',
      snapshot: () => ({ busy, draft, messages: [], existingGraph: graph }),
      append: (message) => appended.push(message),
      clearDraft: () => { draft = ''; },
      setBusy: (value) => { busy = value; },
      markApplied: vi.fn(),
      commitGraph: vi.fn(),
      rememberGraph: (graphId) => { remembered = graphId; },
      onOutcome: vi.fn(),
      dispatchPreset,
    });
    bus.addEventListener('aicross:console-apply-preset', handler);

    dispatchConsoleApply({ sessionId: 'other-tab', preset }, bus);
    expect(dispatchPreset).not.toHaveBeenCalled();
    expect(appended).toHaveLength(0);
    busy = true;
    dispatchConsoleApply({ sessionId: 'console-tab', preset }, bus);
    expect(appended.at(-1)?.text).toBe('当前消息仍在处理中。');
    busy = false;
    dispatchConsoleApply({ sessionId: 'console-tab', preset }, bus);
    expect(appended.at(-1)?.text).toBe('先输入一条消息，再应用模式。');

    draft = '按原文派发';
    dispatchConsoleApply({ sessionId: 'console-tab', preset }, bus);
    await vi.waitFor(() => expect(dispatchPreset).toHaveBeenCalledOnce());
    expect(dispatchPreset.mock.calls[0].slice(0, 5)).toEqual([
      'D:/work', '按原文派发', preset, expect.any(Function), graph,
    ]);
    expect(appended.some((message) => message.text === '进度已回流')).toBe(true);
    expect(remembered).toBe(graph.id);
  });

  it('同一条历史消息只能应用一次，模式派发续写原图', async () => {
    const message = { id: 'u1', kind: 'user', text: '继续处理', createdAt: '' } as ConsoleMessage;
    expect(selectConsoleApplyMessage('', [message], 'u1').blocked).toBe('already-applied');
    expect(selectConsoleApplyMessage('', [message], undefined)).toMatchObject({
      text: '继续处理', message, blocked: null,
    });

    vi.stubGlobal('crypto', { randomUUID: (() => {
      let index = 0;
      return () => `id-${++index}`;
    })() });
    const existing = fillRoot(newGraph('D:/work'), '第一条');
    const outcome = await dispatchConsolePreset('D:/work', '第二条', preset, () => {}, existing, {
      listDispatchableModels: async () => ({ models: [], error: '' }),
      loadRoles: async () => ({ version: 1, planning: {}, execution: {}, acceptance: {} }),
      gitChanges: async () => { throw new Error('无 git 信息'); },
      previewDispatch: async () => ({ totalRedacted: 0, broadRedacted: 0, nodes: [] }),
      dispatchGraphStage: async () => ({ results: [] }),
      saveGraph: async () => {},
    } as any);
    expect(outcome.graph.id).toBe(existing.id);
    expect(Object.values(outcome.graph.nodes).some((node) => node.user.text === '第二条')).toBe(true);
  });

  it('用户可见文案与当前文档表述不再使用旧称', async () => {
    // @ts-expect-error Node built-ins are loaded only by Vitest.
    const { readdirSync, readFileSync } = await import('node:fs');
    // @ts-expect-error Node built-ins are loaded only by Vitest.
    const { join } = await import('node:path');
    // @ts-expect-error Node built-ins are loaded only by Vitest.
    const { fileURLToPath } = await import('node:url');
    const oldName = ['总', '指挥'].join('');
    const root = fileURLToPath(new URL('../../../', import.meta.url));
    const files: string[] = [];
    const walk = (directory: string) => {
      for (const entry of readdirSync(directory, { withFileTypes: true })) {
        if (entry.isDirectory() && ['.git', '__tests__', 'dist', 'node_modules', 'target'].includes(entry.name)) continue;
        const path = join(directory, entry.name);
        if (entry.isDirectory()) walk(path);
        else if (/\.(?:css|html|js|json|md|py|rs|toml|ts|tsx|ya?ml)$/.test(entry.name)) files.push(path);
      }
    };
    walk(join(root, 'src-ui', 'src'));
    walk(join(root, 'src'));
    files.push(join(root, 'docs', 'MANUAL.md'));
    const product = readFileSync(join(root, 'docs', 'PRODUCT.md'), 'utf8');
    const currentSection = product.split('## 控制台就是对话窗（09-16 定案，用户裁决）')[1]
      ?.split('\n## ')[0] ?? '';
    const currentCopy = (file: string) => readFileSync(file, 'utf8')
      .replace(/\/\*[\s\S]*?\*\//g, '')
      .replace(/<!--\s*[\s\S]*?-->/g, '')
      .replace(/^\s*\/\/.*$/gm, '');
    const leftovers = files.filter((file) => currentCopy(file).includes(oldName));
    if (currentSection.includes(oldName)) leftovers.push('docs/PRODUCT.md#控制台就是对话窗');
    expect(leftovers).toEqual([]);
  });
});


describe('复审补漏（09-16 编排者）', () => {
  it('点补全时已打的前缀不残留，哪怕和选中的这家不同名', () => {
    // 原来按选中目标逐词匹配：打了 @kimi 再点 Claude Code 会变成
    // 「@Claude Code kimi 帮我看」，把废弃前缀当正文留下（复审 Gemini 1）。
    expect(completeConsoleMention('@c ', { agent: 'pi', model: 'glm-5.3-flash', provider: 'zai' } as never))
      .toBe('@pi agent ');
    expect(completeConsoleMention('@kimi 帮我看', { agent: 'claude', model: 'claude-sonnet-5', provider: '' } as never))
      .toBe('@Claude Code 帮我看');
    // 已经打到多词显示名中间的那截也要吃掉
    expect(completeConsoleMention('@pi ag 帮我看', { agent: 'pi', model: 'g', provider: '' } as never))
      .toBe('@pi agent 帮我看');
  });

  it('历史归档没有被本期改名动过（防篡改）', () => {
    // 改名只该动用户可见文案与当前文档；tasks/ 下的审查与报告是某个日期的记录，
    // 改了等于篡改证据（复审 Gemini 2；首版确实误改了 34 个文件）。
    const dir = new URL('../../../tasks/', import.meta.url);
    const names = (readdirSync(dir) as string[]).filter((name: string) =>
      (name.startsWith('review-') || name.startsWith('REPORT-')) && name.endsWith('.md'));
    expect(names.length).toBeGreaterThan(20);
    const hits: string[] = [];
    for (const name of names) {
      // 本期之后新写的报告可以用新词；只查本期之前就存在的归档。
      if (name === 'REPORT-22.md' || name === 'REPORT-22-R1.md') continue;
      const text = readFileSync(new URL(name, dir), 'utf8');
      if (text.includes('控制台前台') || text.includes('控制台标题')) hits.push(name);
    }
    expect(hits).toEqual([]);
  });
});


describe('控制台聊天的布局（09-16 用户反馈）', () => {
  it('聊天块占满父高，输入框才贴底', () => {
    // 外层 .graph-shell 有高度但不是 flex 容器，只写 flex:1 会落空、整块缩成
    // 内容高度，输入框就贴在标题下面、下方一大片空白。
    const css = readFileSync(new URL('../components/center/ConsoleChat.css', import.meta.url), 'utf8');
    const block = css.slice(css.indexOf('.console-chat {'), css.indexOf('}', css.indexOf('.console-chat {')));
    expect(block).toContain('height: 100%');
    expect(block).toContain('flex-direction: column');
    // 消息列表吃掉剩余空间并自己滚动
    const list = css.slice(css.indexOf('.console-message-list {'), css.indexOf('}', css.indexOf('.console-message-list {')));
    expect(list).toContain('flex: 1');
    expect(list).toContain('overflow: auto');
  });
});
