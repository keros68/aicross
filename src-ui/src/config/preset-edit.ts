// 自定义模式编辑器的纯函数（TASK-55）：初值（新建 / 复制为新模式 / 编辑）、与引擎一致
// 的校验、写权限显示名与引擎值的映射、提交给 preset_save 的对象、删除影响计数。
// 契约见 tasks/TASK-55-custom-modes.md 第 2 节。

import type { ModeEntry, PresetPayload, SlashCommand } from '../tauri';

export const ROLE_OPTIONS: readonly string[] = ['出方案', '执行', '验收', '验证', '拆分'];
export const VISIBILITY_OPTIONS: readonly string[] = ['共享', '盲'];
/** 写权限：value 是传给引擎的值，label 是界面显示名。 */
export const WRITE_OPTIONS: ReadonlyArray<{ value: string; label: string }> = [
  { value: '只读', label: '只读' },
  { value: '单写者', label: '单写者' },
  { value: 'worktree', label: '独立工作区' },
];
export const PRESET_THINKING_OPTIONS: ReadonlyArray<{ value: string; label: string }> = [
  { value: '', label: '沿用' },
  { value: '关', label: '关' },
  { value: '低', label: '低' },
  { value: '中', label: '中' },
  { value: '高', label: '高' },
];

export const PRESET_NAME_MAX = 40;
export const PRESET_SEATS_MAX = 12;

export interface DraftNode {
  role: string;
  visibility: string;
  write: string;
  thinking: string;
}

export interface PresetDraft {
  /** 编辑已有自定义模式时为它的 id；新建为空串。 */
  id: string;
  name: string;
  description: string;
  stages: { nodes: DraftNode[] }[];
}

export function writeLabel(value: string): string {
  return WRITE_OPTIONS.find((option) => option.value === value)?.label ?? value;
}

export function newNode(): DraftNode {
  return { role: '执行', visibility: '共享', write: '只读', thinking: '' };
}

/** 「添加阶段」时新阶段的第一个位置：验证 · 盲（TASK-58）。 */
export function newStageNode(): DraftNode {
  return { role: '验证', visibility: '盲', write: '只读', thinking: '' };
}

export function emptyDraft(): PresetDraft {
  return { id: '', name: '', description: '', stages: [{ nodes: [newNode()] }] };
}

/** 模式里的位置 → 编辑器里的位置；编辑器不认的可见性（如「指定上游」）按共享、写权限按只读；
 *  思考取预设本身给该位置定的值（presetThinking），缺失或不认时为沿用。 */
function nodeFromMode(node: ModeEntry['stages'][number][number]): DraftNode {
  const thinking = (node.presetThinking ?? '').trim();
  return {
    role: ROLE_OPTIONS.includes(node.role) ? node.role : '执行',
    visibility: VISIBILITY_OPTIONS.includes(node.visibility) ? node.visibility : '共享',
    write: WRITE_OPTIONS.some((option) => option.value === node.write) ? node.write : '只读',
    thinking: PRESET_THINKING_OPTIONS.some((option) => option.value === thinking) ? thinking : '',
  };
}

function stagesFromMode(mode: Pick<ModeEntry, 'stages'>): PresetDraft['stages'] {
  return mode.stages
    .filter((nodes) => nodes.length > 0)
    .map((nodes) => ({ nodes: nodes.map(nodeFromMode) }));
}

/** 复制为新模式：沿用阶段与位置，名称为「<原名> 副本」，不带 id。 */
export function draftFromCopy(mode: Pick<ModeEntry, 'name' | 'stages' | 'description'>): PresetDraft {
  return { id: '', name: `${mode.name} 副本`, description: mode.description ?? '', stages: stagesFromMode(mode) };
}

/** 编辑自定义模式：带 id 与原名称、介绍。 */
export function draftFromMode(mode: Pick<ModeEntry, 'id' | 'name' | 'stages' | 'description'>): PresetDraft {
  return { id: mode.id, name: mode.name, description: mode.description ?? '', stages: stagesFromMode(mode) };
}

export interface PresetErrors {
  name: string;
  /** 阶段下标（从 0 起）→ 该阶段的问题。 */
  stages: Record<number, string>;
  /** 整个模式的问题（没有位置、超过上限）。 */
  total: string;
}

export function hasErrors(errors: PresetErrors): boolean {
  return errors.name !== '' || errors.total !== '' || Object.keys(errors.stages).length > 0;
}

/** 与引擎一致的校验：名称非空、≤40 字、不与其它模式重名；每个阶段至少一个位置、
 *  最多一个单写者；全模式至少一个位置、最多 12 个。 */
export function validatePreset(draft: PresetDraft, others: Pick<ModeEntry, 'id' | 'name'>[]): PresetErrors {
  const errors: PresetErrors = { name: '', stages: {}, total: '' };
  const name = draft.name.trim();
  if (name === '') errors.name = '名称为空';
  else if (Array.from(name).length > PRESET_NAME_MAX) errors.name = `名称超过 ${PRESET_NAME_MAX} 字`;
  else if (others.some((mode) => mode.id !== draft.id && mode.name.trim() === name)) errors.name = '名称已存在';

  draft.stages.forEach((stage, index) => {
    if (stage.nodes.length === 0) errors.stages[index] = '阶段没有位置';
    else if (stage.nodes.filter((node) => node.write === '单写者').length > 1) errors.stages[index] = '同一阶段只能有一个单写者';
  });

  const total = draft.stages.reduce((sum, stage) => sum + stage.nodes.length, 0);
  if (total === 0) errors.total = '没有位置';
  else if (total > PRESET_SEATS_MAX) errors.total = `超过 ${PRESET_SEATS_MAX} 个位置`;
  return errors;
}

/** 提交给 preset_save 的对象：名称、介绍去首尾空白（介绍压成单行，空则不带）；新建不带 id。 */
export function toPresetPayload(draft: PresetDraft): PresetPayload {
  const description = draft.description.replace(/\s*[\r\n]+\s*/g, ' ').trim();
  const payload: PresetPayload = {
    name: draft.name.trim(),
    stages: draft.stages.map((stage) => ({
      nodes: stage.nodes.map((node) => ({
        role: node.role,
        visibility: node.visibility,
        write: node.write,
        thinking: node.thinking,
      })),
    })),
  };
  if (draft.id !== '') payload.id = draft.id;
  if (description !== '') payload.description = description;
  return payload;
}

export interface RemoveImpact {
  /** 本主 agent 下该模式里设了人选的位置数。 */
  seats: number;
  isDefault: boolean;
  /** 本主 agent 下指向该模式的斜杠命令数。 */
  slashCommands: number;
}

/** 删除前按当前快照数出受影响项。 */
export function removeImpact(
  mode: Pick<ModeEntry, 'id' | 'stages'>,
  defaultMode: string,
  slashCommands: Pick<SlashCommand, 'mode'>[],
): RemoveImpact {
  return {
    seats: mode.stages.flat().filter((node) => node.seat !== null).length,
    isDefault: mode.id === defaultMode,
    slashCommands: slashCommands.filter((command) => command.mode === mode.id).length,
  };
}

/** 确认框里列出的受影响项；没有则为空。 */
export function impactLines(impact: RemoveImpact): string[] {
  const lines: string[] = [];
  if (impact.seats > 0) lines.push(`${impact.seats} 个人选`);
  if (impact.isDefault) lines.push('当前默认模式');
  if (impact.slashCommands > 0) lines.push(`${impact.slashCommands} 个斜杠命令`);
  return lines;
}
