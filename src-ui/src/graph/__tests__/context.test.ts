import { describe, expect, it } from 'vitest';
import { contextOf } from '../context.ts';
import { stamp } from '../stamp.ts';
import type { Graph, GraphNode, NodeKind, Reply, Target, VisibilityKind } from '../types.ts';

function mkTarget(visibility: VisibilityKind, upstreams?: string[]): Target {
  return {
    agent: 'claude',
    model: 'm1',
    role: '执行',
    thinking: '中',
    visibility,
    upstreams,
    write: '只读',
  };
}

function mkNode(
  id: string,
  kind: NodeKind,
  parents: string[],
  userText: string,
  extra?: { reply?: Reply; target?: Target; attachments?: string[] },
): GraphNode {
  return {
    id,
    kind,
    parents,
    user: { text: userText, attachments: extra?.attachments ?? [] },
    reply: extra?.reply,
    target: extra?.target,
    created_at: '2026-09-13T00:00:00.000Z',
  };
}

function done(text: string): Reply {
  return { status: 'done', text };
}

function mkGraph(nodes: GraphNode[], root = 'root'): Graph {
  const rec: Record<string, GraphNode> = {};
  for (const n of nodes) rec[n.id] = n;
  return {
    version: 1,
    id: 'g',
    title: 't',
    root,
    nodes: rec,
    created_at: '2026-09-13T00:00:00.000Z',
    updated_at: '2026-09-13T00:00:00.000Z',
  };
}

describe('contextOf', () => {
  it('共享：路径上全部用户消息与模型回复', () => {
    const g = mkGraph([
      mkNode('root', 'material', [], '根消息', { attachments: ['a.txt'] }),
      mkNode('A', 'turn', ['root'], '问A', { reply: done('答A') }),
      mkNode('B', 'turn', ['A'], '问B', { target: mkTarget('共享') }),
    ]);
    const ctx = contextOf(g, 'B');
    expect(ctx).toEqual([
      { text: '根消息', attachments: ['a.txt'] },
      { text: '问A', attachments: [] },
      { text: '答A', attachments: [] },
      { text: '问B', attachments: [] },
    ]);
  });

  it('盲：只留用户消息与附件，模型回复全部剔除', () => {
    const g = mkGraph([
      mkNode('root', 'material', [], '根消息'),
      mkNode('A', 'turn', ['root'], '问A', { reply: done('答A') }),
      mkNode('B', 'turn', ['A'], '问B', {
        reply: { status: 'draft', text: '' },
        target: mkTarget('盲'),
      }),
    ]);
    const ctx = contextOf(g, 'B');
    expect(ctx).toEqual([
      { text: '根消息', attachments: [] },
      { text: '问A', attachments: [] },
      { text: '问B', attachments: [] },
    ]);
  });

  it('指定上游：用户消息全留，模型回复只留 upstreams 列表里的节点', () => {
    const g = mkGraph([
      mkNode('root', 'material', [], '根消息'),
      mkNode('A', 'turn', ['root'], '问A', { reply: done('答A') }),
      mkNode('B', 'turn', ['A'], '问B', { reply: done('答B') }),
      mkNode('C', 'turn', ['B'], '问C', { target: mkTarget('指定上游', ['A']) }),
    ]);
    const ctx = contextOf(g, 'C');
    expect(ctx).toEqual([
      { text: '根消息', attachments: [] },
      { text: '问A', attachments: [] },
      { text: '答A', attachments: [] },
      { text: '问B', attachments: [] },
      { text: '问C', attachments: [] },
    ]);
  });

  it('accept：父路径并集去重，盲之下父节点自身回复仍保留', () => {
    const g = mkGraph([
      mkNode('root', 'material', [], '根消息'),
      mkNode('A', 'turn', ['root'], '问A', { reply: done('答A') }),
      mkNode('B', 'turn', ['root'], '问B', { reply: done('答B') }),
      mkNode('X', 'accept', ['A', 'B'], '验收', { target: mkTarget('盲') }),
    ]);
    const ctx = contextOf(g, 'X');
    expect(ctx).toEqual([
      { text: '根消息', attachments: [] },
      { text: '问A', attachments: [] },
      { text: '答A', attachments: [] },
      { text: '问B', attachments: [] },
      { text: '答B', attachments: [] },
      { text: '验收', attachments: [] },
    ]);
  });

  it('accept：分叉再汇合时公共祖先只出现一次，顺序为根到叶', () => {
    const g = mkGraph([
      mkNode('root', 'material', [], '根消息'),
      mkNode('A', 'turn', ['root'], '问A', { reply: done('答A') }),
      mkNode('B', 'turn', ['A'], '问B', { reply: done('答B') }),
      mkNode('C', 'turn', ['A'], '问C', { reply: done('答C') }),
      mkNode('X', 'accept', ['B', 'C'], '', { target: mkTarget('共享') }),
    ]);
    const ctx = contextOf(g, 'X');
    expect(ctx).toEqual([
      { text: '根消息', attachments: [] },
      { text: '问A', attachments: [] },
      { text: '答A', attachments: [] },
      { text: '问B', attachments: [] },
      { text: '答B', attachments: [] },
      { text: '问C', attachments: [] },
      { text: '答C', attachments: [] },
    ]);
  });

  it('draft/pending/空文本的回复不进上下文', () => {
    const g = mkGraph([
      mkNode('root', 'material', [], '根消息'),
      mkNode('A', 'turn', ['root'], '问A', { reply: { status: 'pending', text: '半截' } }),
      mkNode('B', 'turn', ['A'], '问B', { reply: { status: 'done', text: '' } }),
      mkNode('C', 'turn', ['B'], '问C', { target: mkTarget('共享') }),
    ]);
    const ctx = contextOf(g, 'C');
    expect(ctx).toEqual([
      { text: '根消息', attachments: [] },
      { text: '问A', attachments: [] },
      { text: '问B', attachments: [] },
      { text: '问C', attachments: [] },
    ]);
  });

  it('悬空 parents id 跳过不抛异常', () => {
    const g = mkGraph([
      mkNode('root', 'material', [], '根消息'),
      mkNode('A', 'turn', ['ghost'], '问A', { target: mkTarget('共享') }),
    ]);
    expect(() => contextOf(g, 'A')).not.toThrow();
    expect(contextOf(g, 'A')).toEqual([{ text: '问A', attachments: [] }]);
  });

  it('验收节点上再分叉：后代上下文含全部执行分支的回复（祖先闭包）', () => {
    const g = mkGraph([
      mkNode('root', 'material', [], '根消息'),
      mkNode('A', 'turn', ['root'], '问A', { reply: done('答A') }),
      mkNode('E1', 'turn', ['A'], '问E1', { reply: done('答E1') }),
      mkNode('E2', 'turn', ['A'], '问E2', { reply: done('答E2') }),
      mkNode('E3', 'turn', ['A'], '问E3', { reply: done('答E3') }),
      mkNode('X', 'accept', ['E1', 'E2', 'E3'], '验收', {
        reply: done('验收结论'),
        target: mkTarget('盲'),
      }),
      mkNode('F', 'turn', ['X'], '继续干活', { target: mkTarget('共享') }),
    ]);
    const ctx = contextOf(g, 'F');
    // 祖先闭包覆盖三条执行分支，不只 parents[0] 那一条
    expect(ctx).toEqual([
      { text: '根消息', attachments: [] },
      { text: '问A', attachments: [] },
      { text: '答A', attachments: [] },
      { text: '问E1', attachments: [] },
      { text: '答E1', attachments: [] },
      { text: '验收', attachments: [] },
      { text: '验收结论', attachments: [] },
      { text: '问E2', attachments: [] },
      { text: '答E2', attachments: [] },
      { text: '问E3', attachments: [] },
      { text: '答E3', attachments: [] },
      { text: '继续干活', attachments: [] },
    ]);
  });

  it('stamp 集成：1→3→1 盖在根上填好回复后，验收（盲）上下文 = 用户消息 + 三个执行父的回复', () => {
    const preset = {
      id: 'p',
      name: 'p',
      builtin: false,
      stages: [
        { nodes: [mkTarget('共享')] },
        { nodes: [mkTarget('共享'), mkTarget('共享'), mkTarget('共享')] },
        { nodes: [{ ...mkTarget('盲'), role: '验收' }] },
      ],
    };
    let g = mkGraph([mkNode('root', 'material', [], '根消息')]);
    g = stamp(g, 'root', preset);
    // 给每个新节点填 done 回复
    const replies = ['方案', '答E1', '答E2', '答E3', '验收结论'];
    const fresh = Object.keys(g.nodes).filter((id) => id !== 'root');
    const nodes = { ...g.nodes };
    fresh.forEach((id, i) => {
      nodes[id] = { ...nodes[id], reply: done(replies[i]) };
    });
    g = { ...g, nodes };
    const accept = fresh.find((id) => g.nodes[id].kind === 'accept')!;
    const ctx = contextOf(g, accept);
    expect(ctx).toEqual([
      { text: '根消息', attachments: [] },
      { text: '答E1', attachments: [] },
      { text: '答E2', attachments: [] },
      { text: '答E3', attachments: [] },
    ]);
  });
});
