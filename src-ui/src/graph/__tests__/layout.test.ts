// layout.ts / mutate.ts 的界面纯函数测试

import { describe, expect, it } from 'vitest';
import type { DispatchableModel, Graph, GraphNode, NodeKind } from '../types.ts';
import { depthMap, latestLeaf, layersOf, pathTo } from '../layout.ts';
import { appendTurn, fillRoot, parseSend } from '../mutate.ts';

let clock = 0;
function ts(): string {
  clock += 1;
  return `2026-09-13T00:00:${String(clock).padStart(2, '0')}.000Z`;
}

function mkNode(
  id: string,
  kind: NodeKind,
  parents: string[],
  text = '',
  target?: GraphNode['target'],
): GraphNode {
  return {
    id,
    kind,
    parents,
    user: { text, attachments: [] },
    ...(target ? { target } : {}),
    created_at: ts(),
  };
}

function mkGraph(nodes: GraphNode[], root = 'root'): Graph {
  const rec: Record<string, GraphNode> = {};
  for (const n of nodes) rec[n.id] = n;
  return {
    version: 1,
    id: 'g1',
    title: '',
    root,
    nodes: rec,
    created_at: '2026-09-13T00:00:00.000Z',
    updated_at: '2026-09-13T00:00:00.000Z',
  };
}

describe('depthMap', () => {
  it('链式深度递增', () => {
    const g = mkGraph([
      mkNode('root', 'material', []),
      mkNode('a', 'turn', ['root']),
      mkNode('b', 'turn', ['a']),
    ]);
    const d = depthMap(g);
    expect(d.get('root')).toBe(0);
    expect(d.get('a')).toBe(1);
    expect(d.get('b')).toBe(2);
  });

  it('accept 取最深父节点 + 1（最长路径）', () => {
    const g = mkGraph([
      mkNode('root', 'material', []),
      mkNode('a', 'turn', ['root']),
      mkNode('b', 'turn', ['a']),
      mkNode('c', 'turn', ['root']),
      mkNode('acc', 'accept', ['b', 'c']),
    ]);
    expect(depthMap(g).get('acc')).toBe(3); // root→a→b→acc
  });
});

describe('layersOf', () => {
  it('按深度分层，同层按创建时间排序', () => {
    const g = mkGraph([
      mkNode('root', 'material', []),
      mkNode('a', 'turn', ['root']),
      mkNode('b', 'turn', ['root']),
      mkNode('c', 'turn', ['a']),
    ]);
    expect(layersOf(g)).toEqual([['root'], ['a', 'b'], ['c']]);
  });
});

describe('pathTo', () => {
  it('沿第一父回溯到根', () => {
    const g = mkGraph([
      mkNode('root', 'material', []),
      mkNode('a', 'turn', ['root']),
      mkNode('b', 'turn', ['a']),
    ]);
    expect(pathTo(g, 'b')).toEqual(['root', 'a', 'b']);
  });

  it('accept 节点走第一父路径', () => {
    const g = mkGraph([
      mkNode('root', 'material', []),
      mkNode('a', 'turn', ['root']),
      mkNode('b', 'turn', ['root']),
      mkNode('acc', 'accept', ['a', 'b']),
    ]);
    expect(pathTo(g, 'acc')).toEqual(['root', 'a', 'acc']);
  });
});

describe('latestLeaf', () => {
  it('取最深节点，同深取最新', () => {
    const g = mkGraph([
      mkNode('root', 'material', []),
      mkNode('a', 'turn', ['root']),
      mkNode('b', 'turn', ['root']),
    ]);
    expect(latestLeaf(g)).toBe('b');
  });
});

describe('appendTurn', () => {
  it('新建 draft 子节点，默认执行 Target', () => {
    const g = mkGraph([mkNode('root', 'material', [], '材料')]);
    const { graph: g2, nodeId } = appendTurn(g, 'root', '你好');
    const node = g2.nodes[nodeId];
    expect(node.kind).toBe('turn');
    expect(node.parents).toEqual(['root']);
    expect(node.user.text).toBe('你好');
    expect(node.reply?.status).toBe('draft');
    expect(node.target?.role).toBe('执行');
    expect(node.target?.visibility).toBe('共享');
    expect(g.nodes[nodeId]).toBeUndefined(); // 不改原图
  });
});

describe('fillRoot', () => {
  it('填根消息，空标题取首行前 20 字', () => {
    const g = mkGraph([mkNode('root', 'material', [])]);
    const long = '这是一条很长很长很长很长的第一句话会被截断掉';
    const g2 = fillRoot(g, long);
    expect(g2.nodes.root.user.text).toBe(long);
    expect(g2.title).toBe(long.slice(0, 20));
    expect(g2.title.length).toBe(20);
  });

  it('已有标题不被覆盖', () => {
    const g = { ...mkGraph([mkNode('root', 'material', [])]), title: '我的图' };
    const g2 = fillRoot(g, '第一句');
    expect(g2.title).toBe('我的图');
  });
});

function mkModel(id: string, agent: string): DispatchableModel {
  return {
    id,
    agent,
    provider: 'p1',
    vendor: 'v',
    billing_source: 'b',
    tier: '高',
    thinking: '中',
    status: '已接入',
    source: 's',
    smoked_at: '',
    smoke_model: '',
    smoke_failure: '',
    meta: '',
    note: '',
  };
}

const MODELS: DispatchableModel[] = [
  mkModel('k3', 'kimi'),
  mkModel('dup', 'a1'),
  mkModel('dup', 'a2'),
];

describe('parseSend', () => {
  it('@agent/模型 提取为 Target（带上 provider/tier），其余文本是用户消息', () => {
    const r = parseSend('@kimi/k3 帮我看看这段', MODELS);
    expect(r.text).toBe('帮我看看这段');
    expect(r.target).toMatchObject({
      agent: 'kimi',
      model: 'k3',
      provider: 'p1',
      tier: '高',
      role: '执行',
    });
  });

  it('裸 @模型 唯一匹配成功', () => {
    const r = parseSend('@k3 干活', MODELS);
    expect(r.target).toMatchObject({ agent: 'kimi', model: 'k3' });
    expect(r.text).toBe('干活');
  });

  it('裸 @模型 消歧失败落手填：agent 留空、model 按原文', () => {
    const r = parseSend('@dup 干活', MODELS);
    expect(r.target).toMatchObject({ agent: '', model: 'dup' });
    expect(r.text).toBe('干活');
  });

  it('@ 未接入的 agent/模型 按原文手填', () => {
    const r = parseSend('@foo/bar-9 干活', MODELS);
    expect(r.target).toMatchObject({ agent: 'foo', model: 'bar-9' });
    expect(r.target?.provider).toBeUndefined();
    expect(r.target?.tier).toBeUndefined();
  });

  it('没写 @：target 为 undefined（调用侧沿用列末端 Target）', () => {
    const r = parseSend('就这句话', MODELS);
    expect(r.text).toBe('就这句话');
    expect(r.target).toBeUndefined();
  });
});
