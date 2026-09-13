import { describe, expect, it } from 'vitest';
import {
  appendDecision,
  applyDispatchResults,
  clearTargetSession,
  continuationSession,
  dispatchableNodeIds,
  redraftNode,
  setReplyAdopted,
} from '../dispatch.ts';
import type { Graph, GraphNode, Target } from '../types.ts';

const target = (agent = 'codex', model = 'm'): Target => ({
  agent,
  model,
  role: '执行',
  thinking: '中',
  visibility: '共享',
  write: '只读',
});
const node = (id: string, parents: string[], reply?: GraphNode['reply']): GraphNode => ({
  id,
  kind: parents.length === 0 ? 'material' : parents.length > 1 ? 'accept' : 'turn',
  parents,
  user: { text: id, attachments: [] },
  target: parents.length ? target() : undefined,
  reply,
  created_at: id,
});
const graph = (nodes: GraphNode[]): Graph => ({
  version: 1,
  id: 'g',
  title: '',
  root: 'r',
  nodes: Object.fromEntries(nodes.map((n) => [n.id, n])),
  created_at: '',
  updated_at: '',
});

describe('graph dispatch helpers', () => {
  it('只返回父节点完成或为材料的 draft 节点', () => {
    const g = graph([
      node('r', []),
      node('a', ['r'], { status: 'draft', text: '' }),
      node('b', ['a'], { status: 'draft', text: '' }),
      node('c', ['r'], { status: 'done', text: 'ok' }),
      node('d', ['c'], { status: 'draft', text: '' }),
    ]);
    expect(dispatchableNodeIds(g)).toEqual(['a', 'd']);
  });

  it('同目标共享父节点带 session_id 时续聊', () => {
    const parent = node('a', ['r'], { status: 'done', text: 'one', session_id: 'thread-1' });
    const child = node('b', ['a'], { status: 'draft', text: '' });
    const g = graph([node('r', []), parent, child]);
    expect(continuationSession(g, 'b')).toBe('thread-1');
    g.nodes.b.target = target('pi', 'other');
    expect(continuationSession(g, 'b')).toBeUndefined();
  });

  it('新开会话清除同目标父链 session_id', () => {
    const parent = node('a', ['r'], { status: 'done', text: 'one', session_id: 'thread-1' });
    const child = node('b', ['a'], { status: 'draft', text: '' });
    const g = graph([node('r', []), parent, child]);
    const next = clearTargetSession(g, 'b');
    expect(next.nodes.a.reply?.session_id).toBeUndefined();
    expect(continuationSession(next, 'b')).toBeUndefined();
  });

  it('结果写回 Reply 的正文、用量、耗时与 session_id', () => {
    const g = graph([node('r', []), node('a', ['r'], { status: 'pending', text: '' })]);
    const next = applyDispatchResults(g, [{
      nodeId: 'a', status: 'ok', text: 'answer', modelReported: 'openai/m',
      sessionId: 'thread-2', tokensIn: 12, tokensOut: 4, durationMs: 80, error: '',
    }]);
    expect(next.nodes.a.reply).toEqual({
      status: 'done', text: 'answer', model_reported: 'openai/m', tokens_in: 12,
      tokens_out: 4, ms: 80, session_id: 'thread-2',
    });
  });

  it('升级后提高档位和 thinking 并回到 draft', () => {
    const g = graph([node('r', []), node('a', ['r'], { status: 'failed', text: '失败' })]);
    const next = redraftNode(g, 'a', true);
    expect(next.nodes.a.target?.tier).toBe('中');
    expect(next.nodes.a.target?.thinking).toBe('高');
    expect(next.nodes.a.reply).toEqual({ status: 'draft', text: '' });
  });

  it('采纳状态写入 Reply 并可取消', () => {
    const g = graph([node('r', []), node('a', ['r'], { status: 'done', text: 'answer' })]);
    const adopted = setReplyAdopted(g, 'a', true);
    expect(adopted.nodes.a.reply?.adopted).toBe(true);
    expect(setReplyAdopted(adopted, 'a', false).nodes.a.reply?.adopted).toBe(false);
  });

  it('交人裁决节点不把系统标签塞进用户正文', () => {
    const g = graph([node('r', []), node('a', ['r'], { status: 'done', text: 'answer' })]);
    const next = appendDecision(g, ['a'], '请裁决');
    const added = Object.values(next.nodes).find((item) => !g.nodes[item.id]);
    expect(added?.user.text).toBe('请裁决');
  });
});
