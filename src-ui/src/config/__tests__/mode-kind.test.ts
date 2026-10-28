import { describe, expect, it } from 'vitest';
import { MODE_ICONS, groupModes, modeGuide, modeKind } from '../mode-kind';
import type { ModeEntry, ModeSeatNode } from '../../tauri';

function mode(id: string, name: string, builtin = true): ModeEntry {
  return { id, name, builtin, default: false, stages: [] };
}

function node(write: string): ModeSeatNode {
  return { role: '执行', visibility: '共享', write, seat: null, effective: null, source: '未指定' };
}

describe('模式说明（TASK-66）', () => {
  it('内置模式用写定的「适合……」，原有介绍作悬停提示', () => {
    const guide = modeGuide({ ...mode('builtin-blind', '盲审模式'), description: '同一份材料由多个厂商的模型独立审查', stages: [[node('只读')]] });
    expect(guide.text.startsWith('适合')).toBe(true);
    expect(guide.hint).toBe('同一份材料由多个厂商的模型独立审查');
    expect(guide.writes).toBe('只读');
  });

  it('六个内置模式都有说明，互不相同', () => {
    const ids = ['shared', 'economy', 'blind', 'arena', 'jury', 'swarm'];
    const texts = ids.map((id) => modeGuide(mode(`builtin-${id}`, '')).text);
    expect(texts.every((text) => text.startsWith('适合'))).toBe(true);
    expect(new Set(texts).size).toBe(ids.length);
  });

  it('写入方式：有位置不是只读就算改项目文件；内置蜂群算改项目文件', () => {
    expect(modeGuide({ ...mode('builtin-economy', '经济模式'), stages: [[node('只读')], [node('单写者')]] }).writes).toBe('改项目文件');
    expect(modeGuide({ ...mode('builtin-swarm', '蜂群模式'), stages: [[node('只读')]] }).writes).toBe('改项目文件');
    expect(modeGuide({ ...mode('user-1', '自定义模式', false), stages: [[node('worktree')]] }).writes).toBe('改项目文件');
  });

  it('自定义模式显示用户写的介绍，名字和内置撞了也不套内置说明', () => {
    const guide = modeGuide({ ...mode('user-2', '经济模式', false), description: '审稿用', stages: [[node('只读')]] });
    expect(guide).toEqual({ text: '审稿用', hint: '审稿用', writes: '只读' });
  });
});

describe('模式的种类与标识（fix-49-r2）', () => {
  it('先按 id 认，再按名称认，都认不出算自定义', () => {
    expect(modeKind('builtin-economy', '')).toBe('economy');
    expect(modeKind('builtin-jury', '评审模式')).toBe('jury');
    expect(modeKind('', '盲审模式')).toBe('blind');
    expect(modeKind('', '自定义模式 · 我的')).toBe('custom');
    expect(modeKind('user-1', '擂台模式')).toBe('arena');
  });

  it('每种模式各有一个图标，互不相同', () => {
    const icons = Object.values(MODE_ICONS);
    expect(new Set(icons).size).toBe(icons.length);
  });
});

describe('模式分组（fix-49-r2）', () => {
  it('常用（共享、经济、盲审）· 更多模式（其余内置）· 自定义，组内保持原顺序', () => {
    const groups = groupModes([
      mode('builtin-shared', '共享模式'),
      mode('builtin-arena', '擂台模式'),
      mode('builtin-economy', '经济模式'),
      mode('builtin-blind', '盲审模式'),
      mode('builtin-jury', '评审模式'),
      mode('builtin-swarm', '蜂群模式'),
      mode('user-1', '自定义模式 · 审稿', false),
    ]);
    expect(groups.map((group) => [group.title, group.modes.map((item) => item.id)])).toEqual([
      ['常用', ['builtin-shared', 'builtin-economy', 'builtin-blind']],
      ['更多模式', ['builtin-arena', 'builtin-jury', 'builtin-swarm']],
      ['自定义', ['user-1']],
    ]);
  });

  it('没有自定义模式时不出那一组；没有模式时为空', () => {
    expect(groupModes([mode('builtin-economy', '经济模式')]).map((group) => group.title)).toEqual(['常用']);
    expect(groupModes([])).toEqual([]);
  });

  it('用户自建但名字和内置撞了，仍归自定义', () => {
    expect(groupModes([mode('user-2', '经济模式', false)])[0].title).toBe('自定义');
  });
});
