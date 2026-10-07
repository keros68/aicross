import { describe, expect, it } from 'vitest';
import {
  SLASH_DEFAULT_PREFIX,
  SLASH_INVALID_STATE,
  canAddAll,
  otherSlashCommands,
  slashModeRows,
  SLASH_NAME_HINT,
  slashNameError,
  normalizeSlashNote,
  slashNameValid,
  slashStateTone,
  triggerPrefix,
  triggerPreview,
} from '../slash-view';
import type { SlashCommand, SlashList } from '../../tauri';

describe('斜杠命令：名称校验（TASK-53）', () => {
  it('小写字母、数字、-，1–32 位', () => {
    expect(slashNameValid('cross-review')).toBe(true);
    expect(slashNameValid('a1')).toBe(true);
    expect(slashNameValid('a'.repeat(32))).toBe(true);
  });

  it('空、超长、大写、空格、下划线、斜杠、中文都不行', () => {
    for (const name of ['', 'a'.repeat(33), 'Cross', 'cross review', 'cross_review', '/cross', '审一下']) {
      expect(slashNameValid(name)).toBe(false);
    }
    expect(SLASH_NAME_HINT).toBe('只能用小写字母、数字和 -');
    expect(slashNameError('Cross')).toBe(SLASH_NAME_HINT);
  });

  it('ai-cross 为保留名（与 skill 本身冲突）', () => {
    expect(slashNameValid('ai-cross')).toBe(false);
    expect(slashNameError('ai-cross')).toBe('ai-cross 为保留名');
    expect(slashNameValid('ai-cross-2')).toBe(true);
  });

  it('Windows 保留名：con、nul、aux、prn、com1–com9、lpt1–lpt9', () => {
    for (const name of ['con', 'nul', 'aux', 'prn', 'com1', 'com9', 'lpt1', 'lpt9']) {
      expect(slashNameError(name)).toBe('系统保留名');
    }
    for (const name of ['com0', 'com10', 'console', 'lpt', 'nul-review']) {
      expect(slashNameError(name)).toBe('');
    }
  });
});

describe('斜杠命令：触发写法预览（TASK-53）', () => {
  it('claude、pi 为「/名称」，codex 为「/prompts:名称」，kimi 为「/skill:名称」', () => {
    expect(triggerPreview('claude', 'cross-review')).toBe('/cross-review');
    expect(triggerPreview('pi', 'cross-review')).toBe('/cross-review');
    expect(triggerPreview('codex', 'cross-review')).toBe('/prompts:cross-review');
    expect(triggerPreview('kimi', 'cross-review')).toBe('/skill:cross-review');
  });

  it('未知的主 agent 按「/」', () => {
    expect(triggerPrefix('hermes')).toBe('/');
  });
});

describe('斜杠命令：状态与附加说明（TASK-53）', () => {
  it('已写入为生效色，未写入、被占用、名称无效为异常色，其余中性', () => {
    expect(slashStateTone('已写入')).toBe('ok');
    expect(slashStateTone('未写入')).toBe('alert');
    expect(slashStateTone('被占用')).toBe('alert');
    expect(SLASH_INVALID_STATE).toBe('名称无效');
    expect(slashStateTone(SLASH_INVALID_STATE)).toBe('alert');
    expect(slashStateTone('其他')).toBe('plain');
  });

  it('附加说明压成单行并去掉首尾空白', () => {
    expect(normalizeSlashNote('  只看安全问题\n 不改代码  ')).toBe('只看安全问题 不改代码');
    expect(normalizeSlashNote('   ')).toBe('');
  });
});

describe('斜杠命令：按模式列出（TASK-54）', () => {
  const command = (name: string, mode: string): SlashCommand => ({
    name, mode, modeName: mode, note: '', path: `/p/${name}.md`, state: '已写入', trigger: `/prompts:${name}`,
  });
  const blindCmd = command('ax-blind', 'builtin-blind');
  const extra = command('my-review', 'builtin-blind');
  const custom = command('paper', 'user-1');
  const list: Pick<SlashList, 'host' | 'commands' | 'modes'> = {
    host: 'codex',
    commands: [blindCmd, extra, custom],
    modes: [
      { mode: 'builtin-economy', modeName: '经济模式', argForm: '$ai-cross 经济', suggestedName: 'ax-economy', command: null },
      { mode: 'builtin-blind', modeName: '盲审模式', argForm: '$ai-cross 盲审', suggestedName: 'ax-blind', command: blindCmd },
    ],
  };

  it('模式行：有命令时不给建议写法；无命令时给建议名的触发写法预览', () => {
    const rows = slashModeRows(list);
    expect(rows.map((row) => [row.mode, row.suggestedTrigger, row.command?.name ?? null])).toEqual([
      ['builtin-economy', '/prompts:ax-economy', null],
      ['builtin-blind', '', 'ax-blind'],
    ]);
    expect(rows[0].argForm).toBe('$ai-cross 经济');
    expect(SLASH_DEFAULT_PREFIX).toBe('ax-');
  });

  it('旧版引擎没有 modes：没有模式行，全部命令归入「其它」', () => {
    const old: Pick<SlashList, 'host' | 'commands' | 'modes'> = { host: 'codex', commands: [blindCmd, custom] };
    expect(slashModeRows(old)).toEqual([]);
    expect(canAddAll(old)).toBe(false);
    expect(otherSlashCommands(old).map((item) => item.name)).toEqual(['ax-blind', 'paper']);
  });

  it('全部生成：还有模式没有命令时可点，都有时不可点', () => {
    expect(canAddAll(list)).toBe(true);
    const done = { modes: list.modes!.map((entry) => ({ ...entry, command: entry.command ?? command(entry.suggestedName, entry.mode) })) };
    expect(canAddAll(done)).toBe(false);
    expect(canAddAll({ modes: [] })).toBe(false);
    expect(canAddAll(null)).toBe(false);
  });

  it('「其它」：自定义模式的命令与同一模式的额外名称，不含模式行显示的那条', () => {
    expect(otherSlashCommands(list).map((item) => item.name)).toEqual(['my-review', 'paper']);
    expect(otherSlashCommands(null)).toEqual([]);
  });
});
