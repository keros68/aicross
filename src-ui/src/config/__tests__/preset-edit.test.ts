import { describe, expect, it } from 'vitest';
import {
  PRESET_SEATS_MAX,
  WRITE_OPTIONS,
  draftFromCopy,
  draftFromMode,
  emptyDraft,
  hasErrors,
  impactLines,
  newNode,
  removeImpact,
  toPresetPayload,
  validatePreset,
  writeLabel,
  type PresetDraft,
} from '../preset-edit';
import type { ModeEntry, ModeSeatNode, Seat } from '../../tauri';

function seatNode(overrides: Partial<ModeSeatNode> = {}): ModeSeatNode {
  return { role: '执行', visibility: '共享', write: '单写者', seat: null, effective: null, source: '未指定', ...overrides };
}

const seat: Seat = { agent: 'claude', provider: null, model: 'haiku', tier: '', thinking: '', fallback: null };

const economy: ModeEntry = {
  id: 'builtin-economy',
  name: '经济模式',
  description: '高档模型出方案，低档模型执行，另一家验收',
  builtin: true,
  default: true,
  stages: [
    [seatNode({ role: '出方案', write: '只读' })],
    [seatNode({ role: '执行', write: '单写者', seat })],
    [],
    [seatNode({ role: '验收', visibility: '盲', write: '只读', seat })],
  ],
};

const others = [{ id: 'builtin-economy', name: '经济模式' }, { id: 'user-1', name: '审稿' }];

function draft(overrides: Partial<PresetDraft> = {}): PresetDraft {
  return { ...emptyDraft(), name: '新模式', ...overrides };
}

describe('写权限的显示名与引擎值（TASK-55）', () => {
  it('worktree 显示为「独立工作区」，其余同名', () => {
    expect(WRITE_OPTIONS.map((option) => option.value)).toEqual(['只读', '单写者', 'worktree']);
    expect(writeLabel('worktree')).toBe('独立工作区');
    expect(writeLabel('只读')).toBe('只读');
    expect(writeLabel('其他')).toBe('其他');
  });
});

describe('编辑器初值（TASK-55）', () => {
  it('复制为新模式：名称加「副本」、没有 id、沿用介绍，空阶段去掉，位置照搬', () => {
    const copy = draftFromCopy(economy);
    expect(copy.id).toBe('');
    expect(copy.name).toBe('经济模式 副本');
    expect(copy.description).toBe(economy.description);
    expect(copy.stages.map((stage) => stage.nodes.map((node) => [node.role, node.visibility, node.write]))).toEqual([
      [['出方案', '共享', '只读']],
      [['执行', '共享', '单写者']],
      [['验收', '盲', '只读']],
    ]);
  });

  it('思考取预设本身的 presetThinking；缺失或不认时为沿用', () => {
    const mode: ModeEntry = {
      ...economy,
      stages: [[
        seatNode({ presetThinking: '高' }),
        seatNode({ presetThinking: '' }),
        seatNode(),
        seatNode({ presetThinking: '极高' }),
      ]],
    };
    expect(draftFromCopy(mode).stages[0].nodes.map((node) => node.thinking)).toEqual(['高', '', '', '']);
    expect(draftFromMode({ ...mode, id: 'user-1' }).stages[0].nodes[0].thinking).toBe('高');
  });

  it('编辑器不认的可见性按共享、写权限按只读', () => {
    const odd: ModeEntry = { ...economy, stages: [[seatNode({ visibility: '指定上游', write: '其他' })]] };
    expect(draftFromCopy(odd).stages[0].nodes[0]).toMatchObject({ visibility: '共享', write: '只读' });
  });

  it('编辑自定义模式：带 id 与原名称', () => {
    const mode: ModeEntry = { ...economy, id: 'user-1', name: '审稿', builtin: false, description: undefined };
    expect(draftFromMode(mode)).toMatchObject({ id: 'user-1', name: '审稿', description: '' });
  });

  it('新建：一个阶段一个位置', () => {
    expect(emptyDraft()).toEqual({ id: '', name: '', description: '', stages: [{ nodes: [newNode()] }] });
  });
});

describe('编辑器校验（TASK-55）', () => {
  it('合规时没有错误', () => {
    expect(hasErrors(validatePreset(draft(), others))).toBe(false);
  });

  it('名称为空、超过 40 字、与其它模式重名', () => {
    expect(validatePreset(draft({ name: '  ' }), others).name).toBe('名称为空');
    expect(validatePreset(draft({ name: '名'.repeat(41) }), others).name).toBe('名称超过 40 字');
    expect(validatePreset(draft({ name: '经济模式' }), others).name).toBe('名称已存在');
    expect(validatePreset(draft({ id: 'user-1', name: '审稿' }), others).name).toBe('');
  });

  it('同一阶段多个单写者、阶段没有位置', () => {
    const errors = validatePreset(draft({
      stages: [
        { nodes: [{ ...newNode(), write: '单写者' }, { ...newNode(), write: '单写者' }] },
        { nodes: [] },
        { nodes: [{ ...newNode(), write: '单写者' }, { ...newNode(), write: 'worktree' }] },
      ],
    }), others);
    expect(errors.stages).toEqual({ 0: '同一阶段只能有一个单写者', 1: '阶段没有位置' });
    expect(hasErrors(errors)).toBe(true);
  });

  it('没有位置、超过 12 个位置', () => {
    expect(validatePreset(draft({ stages: [] }), others).total).toBe('没有位置');
    const many = draft({ stages: [{ nodes: Array.from({ length: PRESET_SEATS_MAX + 1 }, newNode) }] });
    expect(validatePreset(many, others).total).toBe('超过 12 个位置');
    const max = draft({ stages: [{ nodes: Array.from({ length: PRESET_SEATS_MAX }, newNode) }] });
    expect(validatePreset(max, others).total).toBe('');
  });
});

describe('提交给 preset_save 的对象（TASK-55）', () => {
  it('新建不带 id；介绍空则不带；名称去首尾空白', () => {
    const payload = toPresetPayload(draft({ name: ' 新模式 ', description: '  ' }));
    expect(payload).toEqual({ name: '新模式', stages: [{ nodes: [{ role: '执行', visibility: '共享', write: '只读', thinking: '' }] }] });
  });

  it('编辑带 id；介绍压成单行', () => {
    const payload = toPresetPayload(draft({ id: 'user-1', description: '先审\n再改 ' }));
    expect(payload.id).toBe('user-1');
    expect(payload.description).toBe('先审 再改');
  });
});

describe('删除影响计数（TASK-55）', () => {
  it('本主 agent 下的人选数、是否默认、斜杠命令数', () => {
    const impact = removeImpact(economy, 'builtin-economy', [{ mode: 'builtin-economy' }, { mode: 'builtin-blind' }]);
    expect(impact).toEqual({ seats: 2, isDefault: true, slashCommands: 1 });
    expect(impactLines(impact)).toEqual(['2 个人选', '当前默认模式', '1 个斜杠命令']);
  });

  it('没有受影响项时为空', () => {
    const plain: ModeEntry = { ...economy, stages: [[seatNode()]] };
    expect(impactLines(removeImpact(plain, 'builtin-blind', []))).toEqual([]);
  });
});
