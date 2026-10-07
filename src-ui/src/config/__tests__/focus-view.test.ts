import { describe, expect, it } from 'vitest';
import { FOCUS_CUSTOM, focusChoices, focusError, focusIndex } from '../focus-view';
import { locationStatus, globalInstallAction, managerText } from '../skill-view';
import { staleCommands } from '../slash-view';
import type { SkillLocation, SlashCommand } from '../../tauri';

const OPTIONS = ['安全', '性能', '需求覆盖'];

describe('评审视角下拉（TASK-60）', () => {
  it('默认、可选视角、自定义…；当前值在选项里时不重复', () => {
    expect(focusChoices(OPTIONS, '安全')).toEqual([
      { value: '', label: '默认' },
      { value: '安全', label: '安全' },
      { value: '性能', label: '性能' },
      { value: '需求覆盖', label: '需求覆盖' },
      { value: FOCUS_CUSTOM, label: '自定义…' },
    ]);
  });

  it('自定义过的当前值补在「自定义…」之前', () => {
    const choices = focusChoices(OPTIONS, '可维护性');
    expect(choices.map((choice) => choice.label)).toEqual(['默认', '安全', '性能', '需求覆盖', '可维护性', '自定义…']);
  });

  it('自定义视角 1–20 字，去首尾空白后判断', () => {
    expect(focusError('  ')).toBe('视角为空');
    expect(focusError('可维护性')).toBe('');
    expect(focusError('字'.repeat(20))).toBe('');
    expect(focusError('字'.repeat(21))).toBe('视角超过 20 字');
  });

  it('config_set_focus 的序号：在带视角选项的位置里按阶段先后从 1 起数，其他位置为 0', () => {
    const plain = {};
    const focus = { focusOptions: OPTIONS };
    const stages = [[plain], [focus, focus, focus]];
    expect(focusIndex(stages, 2, 1)).toBe(1);
    expect(focusIndex(stages, 2, 3)).toBe(3);
    expect(focusIndex(stages, 1, 1)).toBe(0);
    expect(focusIndex(stages, 3, 1)).toBe(0);
  });
});

describe('skill 外部来源（TASK-60）', () => {
  const external = (manager?: string): SkillLocation => ({
    scope: 'global', path: '/home/u/.agents/skills/ai-cross', kind: 'external', version: '1.21.0', writeTarget: true, manager,
  });

  it('skills.sh、skills-manager 写出名字，空串或缺失为其他软件', () => {
    expect(managerText('skills.sh')).toBe('由 skills.sh 管理');
    expect(managerText('skills-manager')).toBe('由 skills-manager 管理');
    expect(managerText('')).toBe('由其他软件管理');
    expect(managerText(undefined)).toBe('由其他软件管理');
  });

  it('位置状态与页头标签同样写出管理软件', () => {
    expect(locationStatus(external('skills.sh'), '1.21.1')).toEqual({ text: '由 skills.sh 管理 · 1.21.0', tone: 'plain' });
    expect(locationStatus(external(''), '1.21.1').text).toBe('由其他软件管理 · 1.21.0');
    expect(globalInstallAction({ host: 'codex', locations: [external('skills-manager')], duplicate: false, updateAvailable: false }, '1.21.1'))
      .toEqual({ kind: 'external', label: '由 skills-manager 管理', disabled: true });
  });
});

describe('待更新的短命令（TASK-60）', () => {
  const command = (name: string, stale?: boolean): SlashCommand => ({
    name, mode: 'builtin-blind', modeName: '盲审模式', note: '', path: `/p/${name}.md`, state: '已写入', trigger: `/${name}`, stale,
  });

  it('只取 stale 为 true 的；字段缺失不算', () => {
    expect(staleCommands({ commands: [command('a', true), command('b', false), command('c')] }).map((item) => item.name)).toEqual(['a']);
    expect(staleCommands(null)).toEqual([]);
  });
});
