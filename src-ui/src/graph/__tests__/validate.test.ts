import { describe, expect, it } from 'vitest';
import { validateNode } from '../validate.ts';
import type { DispatchableModel, Graph, GraphNode, NodeKind, Target } from '../types.ts';

function mkModel(id: string, agent: string, vendor: string): DispatchableModel {
  return {
    id,
    agent,
    provider: 'p',
    vendor,
    billing_source: 'b',
    tier: 't',
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

function mkTarget(agent: string, model: string, role: string): Target {
  return { agent, model, role, thinking: '中', visibility: '共享', write: '只读' };
}

function mkNode(id: string, kind: NodeKind, parents: string[], target?: Target): GraphNode {
  return {
    id,
    kind,
    parents,
    user: { text: '', attachments: [] },
    target,
    created_at: '2026-09-13T00:00:00.000Z',
  };
}

function mkGraph(nodes: GraphNode[]): Graph {
  const rec: Record<string, GraphNode> = {};
  for (const n of nodes) rec[n.id] = n;
  return {
    version: 1,
    id: 'g',
    title: 't',
    root: 'root',
    nodes: rec,
    created_at: '2026-09-13T00:00:00.000Z',
    updated_at: '2026-09-13T00:00:00.000Z',
  };
}

const MODELS: DispatchableModel[] = [
  mkModel('m1', 'claude', 'anthropic'),
  mkModel('m2', 'codex', 'openai'),
];

describe('validateNode', () => {
  it('同厂商验收：标 复核，非独立验证', () => {
    const g = mkGraph([
      mkNode('root', 'material', []),
      mkNode('exec', 'turn', ['root'], mkTarget('claude', 'm1', '执行')),
      mkNode('review', 'turn', ['exec'], mkTarget('claude', 'm1', '验收')),
    ]);
    expect(validateNode(g, 'review', MODELS)).toEqual(['复核，非独立验证']);
  });

  it('换厂商验收：不标', () => {
    const g = mkGraph([
      mkNode('root', 'material', []),
      mkNode('exec', 'turn', ['root'], mkTarget('claude', 'm1', '执行')),
      mkNode('review', 'turn', ['exec'], mkTarget('codex', 'm2', '验证')),
    ]);
    expect(validateNode(g, 'review', MODELS)).toEqual([]);
  });

  it('vendor 查不到（模型未接入）时不标复核', () => {
    const g = mkGraph([
      mkNode('root', 'material', []),
      mkNode('exec', 'turn', ['root'], mkTarget('claude', 'm1', '执行')),
      mkNode('review', 'turn', ['exec'], mkTarget('ghost', 'gx', '验收')),
    ]);
    const hints = validateNode(g, 'review', MODELS);
    expect(hints).not.toContain('复核，非独立验证');
    expect(hints).toContain('未接入');
  });

  it('未接入：模型不在列表里，含 models 为空的情况；model 为空不标', () => {
    const g = mkGraph([
      mkNode('root', 'material', []),
      mkNode('a', 'turn', ['root'], mkTarget('x', 'nope', '执行')),
      mkNode('b', 'turn', ['a'], mkTarget('', '', '执行')),
    ]);
    expect(validateNode(g, 'a', MODELS)).toEqual(['未接入']);
    expect(validateNode(g, 'a', [])).toEqual(['未接入']);
    expect(validateNode(g, 'b', [])).toEqual([]);
  });

  it('accept 父节点少于 2：标 扇入不足', () => {
    const g = mkGraph([
      mkNode('root', 'material', []),
      mkNode('x', 'accept', ['root'], mkTarget('claude', 'm1', '验收')),
    ]);
    expect(validateNode(g, 'x', MODELS)).toContain('扇入不足');
  });

  it('无 target 的根节点：无提示', () => {
    const g = mkGraph([mkNode('root', 'material', [])]);
    expect(validateNode(g, 'root', MODELS)).toEqual([]);
  });
});
