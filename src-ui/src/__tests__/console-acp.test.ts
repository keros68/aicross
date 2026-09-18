import { describe, expect, it, vi } from 'vitest';
import {
  commandText,
  completeSlashCommand,
  filterCommands,
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
});
