// 节点显示名：材料 / agent/model / 未指定模型。界面各处统一用这一个。
// 预设显示名：内置预设用自己的名字，用户自建统一叫「自定义模式」（多个时带名字）。

import type { GraphNode, Preset } from './types.ts';

export function nodeLabel(node: GraphNode | undefined): string {
  if (!node) return '?';
  if (node.kind === 'material') return '材料';
  // 遗留节点（TASK-12）没有模型，卡片与面板标题都叫「遗留」
  if (node.leftover) return '遗留';
  const t = node.target;
  if (!t || (t.agent === '' && t.model === '')) return '未指定模型';
  if (t.agent !== '') return `${t.agent}/${t.model === '' ? '?' : t.model}`;
  return t.model;
}

export const USER_PRESET_LABEL = '自定义模式';

export type PresetGroup = '基础模式' | '自定义模式' | '更多模式';

const BASIC_PRESET_IDS = new Set(['builtin-shared', 'builtin-economy', 'builtin-blind']);

export function presetGroupOf(preset: Pick<Preset, 'id' | 'builtin'>): PresetGroup {
  if (!preset.builtin) return '自定义模式';
  return BASIC_PRESET_IDS.has(preset.id) ? '基础模式' : '更多模式';
}

export function presetDisplayName(preset: Pick<Preset, 'name' | 'builtin'>, userPresetCount: number): string {
  if (preset.builtin) return preset.name;
  return userPresetCount > 1 ? `${USER_PRESET_LABEL} · ${preset.name}` : USER_PRESET_LABEL;
}
