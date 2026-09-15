// TASK-11 命名统一：内置预设显示名以 docs/MANUAL.md 术语表为准。
// 引擎侧（src/orchestration.rs）与前端兜底（graph/swarm.ts）的预设名都要在表里，
// 用户自建预设显示为「自定义模式」。

import { describe, expect, it } from 'vitest';
import manual from '../../../../docs/MANUAL.md?raw';
import orchestrationSource from '../../../../src/orchestration.rs?raw';
import { presetDisplayName, USER_PRESET_LABEL } from '../label.ts';
import { BUILTIN_SWARM_PRESET } from '../swarm.ts';

function glossaryPresetNames(): string[] {
  const row = manual.split('\n').find((line: string) => line.startsWith('| 内置预设 |'));
  if (!row) throw new Error('MANUAL.md 术语表缺「内置预设」一行');
  const cell = row.split('|')[2] ?? '';
  return cell.split('/').map((name: string) => name.trim()).filter(Boolean);
}

function rustBuiltinPresetNames(): string[] {
  const start = orchestrationSource.indexOf('fn builtin_presets()');
  const end = orchestrationSource.indexOf('\n}\n', start);
  const body = orchestrationSource.slice(start, end);
  return [...body.matchAll(/name: "([^"]+)"\.into\(\)/g)].map((m) => m[1]);
}

describe('TASK-11 术语表：内置预设显示名', () => {
  const glossary = glossaryPresetNames();

  it('术语表列出六个内置预设，且全部叫「X模式」', () => {
    expect(glossary).toHaveLength(6);
    for (const name of glossary) expect(name.endsWith('模式')).toBe(true);
  });

  it('src/orchestration.rs 内置预设名与术语表一致（顺序也一致）', () => {
    expect(rustBuiltinPresetNames()).toEqual(glossary);
  });

  it('前端兜底的蜂群预设名在术语表里', () => {
    expect(glossary).toContain(BUILTIN_SWARM_PRESET.name);
  });

  it('用户自建预设显示为「自定义模式」，多个时带名字', () => {
    expect(presetDisplayName({ name: '共享模式', builtin: true }, 3)).toBe('共享模式');
    expect(presetDisplayName({ name: '手测三家流水线', builtin: false }, 1)).toBe(USER_PRESET_LABEL);
    expect(presetDisplayName({ name: '手测三家流水线', builtin: false }, 2)).toBe('自定义模式 · 手测三家流水线');
  });
});
