import { describe, expect, it } from 'vitest';
import { MODE_ICONS, groupModes, modeKind } from '../mode-kind';
import type { ModeEntry } from '../../tauri';

function mode(id: string, name: string, builtin = true): ModeEntry {
  return { id, name, builtin, default: false, stages: [] };
}

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
