import { describe, expect, it, vi } from 'vitest';
import {
  commandListKey,
  commandText,
  completeSlashCommand,
  filterCommands,
  needsFallbackReject,
  normalizeCwd,
  permissionBelongsTo,
  slashQuery,
  supportsSlashCommands,
  type AcpCommand,
  type AcpPermissionRequest,
} from '../console-acp';
import { dispatchConsoleTurn, dispatchConsoleTurnWithFallback } from '../console-chat';
import type { ConsoleTargetAgent } from '../console-session';

const claude: ConsoleTargetAgent = { agent: 'claude', provider: '', model: 'claude-opus-5' };
const kimi: ConsoleTargetAgent = { agent: 'kimi', provider: '', model: 'kimi-code/k3' };
const list: AcpCommand[] = [
  { name: 'compact', description: '压缩上下文', hint: '' },
  { name: 'anthropic-skills:pdf', description: 'PDF', hint: '' },
  { name: 'context', description: '', hint: '' },
  { name: '$imagegen', description: 'codex 技能', hint: '' },
];

function request(dispatchKey: string): AcpPermissionRequest {
  return { requestId: 'r', dispatchKey, agent: 'claude', title: 't', kind: 'other', detail: '', options: [] };
}

describe('控制台 ACP：/ 补全与权限卡片', () => {
  it('只在输入就是一个没打完的命令词时弹补全，@ 改派前缀照样认', () => {
    expect(slashQuery('/', claude, [claude, kimi])).toEqual({ target: claude, prefix: '', head: '' });
    expect(slashQuery('/com', claude, [claude, kimi])).toEqual({ target: claude, prefix: 'com', head: '' });
    expect(slashQuery('@kimi /sk', claude, [claude, kimi])).toEqual({ target: kimi, prefix: 'sk', head: '@kimi ' });
    expect(slashQuery('/compact 保留最近', claude, [claude])).toBeNull();
    expect(slashQuery('看一下 /tmp', claude, [claude])).toBeNull();
    expect(slashQuery('/a\n/b', claude, [claude])).toBeNull();
  });

  it('前缀匹配在前、包含匹配在后；codex 的 $技能 按原样插入', () => {
    expect(filterCommands(list, 'co').map((c) => c.name)).toEqual(['compact', 'context']);
    expect(filterCommands(list, 'pdf').map((c) => c.name)).toEqual(['anthropic-skills:pdf']);
    expect(filterCommands(list, 'image').map((c) => c.name)).toEqual(['$imagegen']);
    expect(filterCommands(list, '', 2)).toHaveLength(2);
    expect(commandText(list[0])).toBe('/compact');
    expect(commandText(list[3])).toBe('$imagegen');
    const query = slashQuery('@kimi /com', claude, [claude, kimi])!;
    expect(completeSlashCommand(query, list[0])).toBe('@kimi /compact ');
  });

  it('只有走 ACP 的三家有命令清单', () => {
    expect(supportsSlashCommands('claude')).toBe(true);
    expect(supportsSlashCommands('Codex')).toBe(true);
    expect(supportsSlashCommands('kimi')).toBe(true);
    expect(supportsSlashCommands('pi')).toBe(false);
  });

  it('权限卡片按本轮 runId 认领，续聊失效重派的那一路也算', () => {
    expect(permissionBelongsTo(request('console-chat-a:node'), 'console-chat-a')).toBe(true);
    expect(permissionBelongsTo(request('console-chat-a-fresh:node'), 'console-chat-a')).toBe(true);
    expect(permissionBelongsTo(request('console-chat-ab:node'), 'console-chat-a')).toBe(false);
    expect(permissionBelongsTo(request('console-chat-a:node'), '')).toBe(false);
  });

  it('命令清单按 agent + 目录分开缓存，目录写法差异不影响', () => {
    expect(normalizeCwd('C:\\Users\\Me\\Proj\\')).toBe('c:/users/me/proj');
    expect(commandListKey('Claude', 'C:\\Users\\Me\\Proj')).toBe(commandListKey('claude', 'c:/users/me/proj/'));
    expect(commandListKey('claude', 'D:/a')).not.toBe(commandListKey('claude', 'D:/b'));
    expect(normalizeCwd('/work/Foo/')).toBe('/work/Foo');
    expect(commandListKey('claude', '/work/Foo')).not.toBe(commandListKey('claude', '/work/foo'));
  });

  it('适配器没给拒绝选项时卡片补一个拒绝', () => {
    expect(needsFallbackReject([{ optionId: 'a', name: '允许一次', kind: 'allow_once' }])).toBe(true);
    expect(needsFallbackReject([
      { optionId: 'a', name: '允许一次', kind: 'allow_once' },
      { optionId: 'r', name: '拒绝', kind: 'reject_once' },
    ])).toBe(false);
  });

  it('单线聊天请求带 interactive，runId 用调用方给的', async () => {
    let captured: any;
    const io = {
      listDispatchableModels: async () => ({ models: [{
        id: claude.model, agent: 'claude', provider: '', vendor: 'anthropic', billing_source: '',
        tier: 'pro', thinking: '中', status: 'ready', verification: 'native',
      }], error: '' }),
      dispatchGraphStage: vi.fn(async (value: any) => {
        captured = value;
        return { results: [{ nodeId: value.nodes[0].nodeId, status: 'ok', text: '好', sessionId: 's', durationMs: 1,
          fallbackTo: '', modelReported: '', error: '' }] };
      }),
    } as any;
    await dispatchConsoleTurn('D:/work', '/compact', claude, '', io, 'console-chat-fixed');
    expect(captured.runId).toBe('console-chat-fixed');
    expect(captured.nodes[0].interactive).toBe(true);
    expect(captured.nodes[0].currentMessage).toBe('/compact');
  });

  it('续聊失效重派沿用同一 runId 前缀', async () => {
    const runIds: string[] = [];
    const io = {
      listDispatchableModels: async () => ({ models: [{
        id: claude.model, agent: 'claude', provider: '', vendor: 'anthropic', billing_source: '',
        tier: 'pro', thinking: '中', status: 'ready', verification: 'native',
      }], error: '' }),
      dispatchGraphStage: vi.fn(async (value: any) => {
        runIds.push(value.runId);
        const expired = runIds.length === 1;
        return { results: [{ nodeId: value.nodes[0].nodeId, status: expired ? 'error' : 'ok', text: '',
          sessionId: '', durationMs: 1, fallbackTo: '', modelReported: '',
          error: expired ? 'No conversation found with session ID: old' : '' }] };
      }),
    } as any;
    await dispatchConsoleTurnWithFallback('D:/work', '你好', claude, 'old', () => {}, io, 'console-chat-x');
    expect(runIds).toEqual(['console-chat-x', 'console-chat-x-fresh']);
  });

  it('带 freshText 时会话失效重开发出的是 freshText，不带则沿用 text', async () => {
    const sent: any[] = [];
    const io = {
      listDispatchableModels: async () => ({ models: [{
        id: claude.model, agent: 'claude', provider: '', vendor: 'anthropic', billing_source: '',
        tier: 'pro', thinking: '中', status: 'ready', verification: 'native',
      }], error: '' }),
      dispatchGraphStage: vi.fn(async (value: any) => {
        sent.push(value.nodes[0]);
        const expired = value.nodes[0].sessionId !== '';
        return { results: [{ nodeId: value.nodes[0].nodeId, status: expired ? 'error' : 'ok', text: '',
          sessionId: '', durationMs: 1, fallbackTo: '', modelReported: '',
          error: expired ? 'No conversation found with session ID: old' : '' }] };
      }),
    } as any;
    await dispatchConsoleTurnWithFallback(
      'D:/work', '补一段\n\n[当前消息]\n你好', claude, 'old', () => {}, io, 'console-chat-y',
      '补全部\n\n[当前消息]\n你好', '你好',
    );
    expect(sent.map((node) => node.sessionId)).toEqual(['old', '']);
    expect(sent[0].currentMessage).toBe('补一段\n\n[当前消息]\n你好');
    expect(sent[0].prompt).toBe('补全部\n\n[当前消息]\n你好');
    expect(sent[1].prompt).toBe('补全部\n\n[当前消息]\n你好');
    expect(sent[1].currentMessage).toBe('补全部\n\n[当前消息]\n你好');
    expect(sent[1].originalRequest).toBe('你好');

    sent.length = 0;
    await dispatchConsoleTurnWithFallback('D:/work', '你好', claude, 'old', () => {}, io, 'console-chat-z');
    expect(sent.map((node) => node.prompt)).toEqual(['你好', '你好']);
    expect(sent[1].originalRequest).toBe('你好');
  });

  it('续聊请求里 currentMessage 是只补上次之后的那份，prompt 是补全部的那份，originalRequest 是原话', async () => {
    let node: any;
    const io = {
      listDispatchableModels: async () => ({ models: [{
        id: kimi.model, agent: 'kimi', provider: '', vendor: 'moonshot', billing_source: '',
        tier: 'pro', thinking: '中', status: 'ready', verification: 'native',
      }], error: '' }),
      dispatchGraphStage: vi.fn(async (value: any) => {
        node = value.nodes[0];
        return { results: [{ nodeId: node.nodeId, status: 'ok', text: '好', sessionId: 's', durationMs: 1,
          fallbackTo: '', modelReported: '', error: '' }] };
      }),
    } as any;
    const resumed = '前缀\n\n[用户]\n补上回收\n\n[当前消息]\n上面的方案有什么问题';
    const fresh = '前缀\n\n[用户]\n给个方案\n\n[用户]\n补上回收\n\n[当前消息]\n上面的方案有什么问题';
    await dispatchConsoleTurnWithFallback(
      'D:/work', resumed, kimi, 'kimi-session', () => {}, io, 'console-chat-k', fresh, '上面的方案有什么问题',
    );
    expect(node.sessionId).toBe('kimi-session');
    // 引擎续聊发 currentMessage；ACP 失败清会话改走一次性 CLI 时发 prompt。
    expect(node.currentMessage).toBe(resumed);
    expect(node.prompt).toBe(fresh);
    expect(node.originalRequest).toBe('上面的方案有什么问题');

    // 没有续聊 id 的那条路同样带两份；不传 freshText 时两项相同。
    await dispatchConsoleTurnWithFallback('D:/work', resumed, kimi, '', () => {}, io, 'console-chat-k2', fresh, '原话');
    expect([node.sessionId, node.currentMessage, node.prompt, node.originalRequest]).toEqual(['', resumed, fresh, '原话']);
    await dispatchConsoleTurn('D:/work', '你好', kimi, 's', io, 'console-chat-k3');
    expect([node.currentMessage, node.prompt, node.originalRequest]).toEqual(['你好', '你好', '你好']);
  });
});
