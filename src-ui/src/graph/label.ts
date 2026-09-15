// 节点显示名：材料 / agent/model / 未指定模型。界面各处统一用这一个。
// 预设显示名：内置预设用自己的名字，用户自建统一叫「自定义模式」（多个时带名字）。

import type { GraphNode, Preset } from './types.ts';

export function nodeLabel(node: GraphNode | undefined): string {
  if (!node) return '?';
  if (node.kind === 'material') return '材料';
  const t = node.target;
  if (!t || (t.agent === '' && t.model === '')) return '未指定模型';
  if (t.agent !== '') return `${t.agent}/${t.model === '' ? '?' : t.model}`;
  return t.model;
}

export const USER_PRESET_LABEL = '自定义模式';

export function presetDisplayName(preset: Pick<Preset, 'name' | 'builtin'>, userPresetCount: number): string {
  if (preset.builtin) return preset.name;
  return userPresetCount > 1 ? `${USER_PRESET_LABEL} · ${preset.name}` : USER_PRESET_LABEL;
}
