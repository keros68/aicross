import { describe, expect, it } from 'vitest';
import { childrenOf, descendantsOf, stamp } from '../stamp.ts';
import { newGraph, newNodeId } from '../types.ts';
import type { Preset, Target } from '../types.ts';

function mkTarget(role: string, model = ''): Target {
  return {
    agent: '',
    model,
    role,
    thinking: '中',
    visibility: '共享',
    write: '只读',
  };
}

function mkPreset(stages: Target[][]): Preset {
  return { id: newNodeId(), name: 'p', builtin: false, stages: stages.map((nodes) => ({ nodes })) };
}

describe('stamp', () => {
  it('1→3→1 在根上展开：末端 1 个 accept 带 3 个父节点', () => {
    const g = newGraph();
    const preset = mkPreset([
      [mkTarget('出方案')],
      [mkTarget('执行', 'a'), mkTarget('执行', 'b'), mkTarget('执行', 'c')],
      [mkTarget('验收')],
    ]);
    const g2 = stamp(g, g.root, preset);
    expect(Object.keys(g2.nodes)).toHaveLength(1 + 1 + 3 + 1);

    const stage1 = childrenOf(g2, g.root);
    expect(stage1).toHaveLength(1);
    expect(g2.nodes[stage1[0]].kind).toBe('turn');
    expect(g2.nodes[stage1[0]].parents).toEqual([g.root]);

    const stage2 = childrenOf(g2, stage1[0]);
    expect(stage2).toHaveLength(3);
    for (const id of stage2) {
      expect(g2.nodes[id].kind).toBe('turn');
      expect(g2.nodes[id].parents).toEqual([stage1[0]]);
    }

    const stage3 = stage2.flatMap((id) => childrenOf(g2, id));
    expect(new Set(stage3).size).toBe(1);
    const accept = g2.nodes[stage3[0]];
    expect(accept.kind).toBe('accept');
    expect(accept.parents).toEqual(stage2);
    expect(accept.target?.role).toBe('验收');

    // 同层节点 created_at 按预设书写顺序递增，竖排顺序随之固定
    const ts = stage2.map((id) => g2.nodes[id].created_at);
    expect(ts[0] < ts[1]).toBe(true);
    expect(ts[1] < ts[2]).toBe(true);
  });

  it('1→1→1 链式全部 turn，父指针逐级衔接', () => {
    const g = newGraph();
    const preset = mkPreset([[mkTarget('出方案')], [mkTarget('执行')], [mkTarget('验收')]]);
    const g2 = stamp(g, g.root, preset);
    expect(Object.keys(g2.nodes)).toHaveLength(4);

    const n1 = childrenOf(g2, g.root);
    const n2 = childrenOf(g2, n1[0]);
    const n3 = childrenOf(g2, n2[0]);
    expect(n1).toHaveLength(1);
    expect(n2).toHaveLength(1);
    expect(n3).toHaveLength(1);
    for (const id of [n1[0], n2[0], n3[0]]) {
      expect(g2.nodes[id].kind).toBe('turn');
    }
    expect(g2.nodes[n2[0]].parents).toEqual([n1[0]]);
    expect(g2.nodes[n3[0]].parents).toEqual([n2[0]]);
    expect(descendantsOf(g2, g.root).sort()).toEqual([n1[0], n2[0], n3[0]].sort());
  });

  it('在非根节点上盖：父指针指向该节点，原图不被修改', () => {
    const g = newGraph();
    const first = stamp(g, g.root, mkPreset([[mkTarget('执行', 'm')]]));
    const child = childrenOf(first, first.root)[0];
    const beforeUpdatedAt = first.updated_at;

    const g2 = stamp(first, child, mkPreset([[mkTarget('验收')], [mkTarget('验证')]]));
    const added = childrenOf(g2, child);
    expect(added).toHaveLength(1);
    expect(g2.nodes[added[0]].parents).toEqual([child]);
    const last = childrenOf(g2, added[0]);
    expect(g2.nodes[last[0]].parents).toEqual([added[0]]);
    expect(Object.keys(g2.nodes)).toHaveLength(4);

    // 原图不被修改
    expect(Object.keys(first.nodes)).toHaveLength(2);
    expect(first.updated_at).toBe(beforeUpdatedAt);
    expect(childrenOf(first, child)).toHaveLength(0);
    expect(Number.isNaN(Date.parse(g2.updated_at))).toBe(false);
  });

  it('新节点是 draft 空消息，target 深拷贝（改新节点不影响 preset）', () => {
    const g = newGraph();
    const preset = mkPreset([[mkTarget('执行', 'm')]]);
    const g2 = stamp(g, g.root, preset);
    const id = childrenOf(g2, g.root)[0];
    const node = g2.nodes[id];
    expect(node.user).toEqual({ text: '', attachments: [] });
    expect(node.reply).toEqual({ status: 'draft', text: '' });
    node.target!.model = 'changed';
    expect(preset.stages[0].nodes[0].model).toBe('m');
  });
});
