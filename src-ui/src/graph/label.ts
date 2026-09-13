// 节点显示名：材料 / agent/model / 未指定模型。界面各处统一用这一个。

import type { GraphNode } from './types.ts';

export function nodeLabel(node: GraphNode | undefined): string {
  if (!node) return '?';
  if (node.kind === 'material') return '材料';
  const t = node.target;
  if (!t || (t.agent === '' && t.model === '')) return '未指定模型';
  if (t.agent !== '') return `${t.agent}/${t.model === '' ? '?' : t.model}`;
  return t.model;
}
