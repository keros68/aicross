import { describe, expect, it } from 'vitest';
import {
  foundAgents,
  hostOnlyNames,
  invokeText,
  needsSkillInstall,
  reviewMode,
  seatLines,
  setupHint,
  setupState,
  usageExamples,
  workMode,
} from '../start-view';
import type { EffectiveSeat, ModeEntry, ModeSeatNode, ResourcesOutput, SkillHostStatus, SkillLocation } from '../../tauri';

const BUNDLED = '1.23.0';

function seat(model: string, overrides: Partial<EffectiveSeat> = {}): EffectiveSeat {
  return { agent: 'pi', provider: 'zai', model, tier: '', thinking: '', fallback: null, state: '可用', ...overrides };
}

function node(role: string, effective: EffectiveSeat | null): ModeSeatNode {
  return { role, visibility: '盲', write: '只读', seat: null, effective, source: effective ? '位置' : '未指定' };
}

function mode(id: string, name: string, stages: ModeSeatNode[][], isDefault = false): ModeEntry {
  return { id, name, builtin: true, default: isDefault, stages };
}

const blind = mode('builtin-blind', '盲审模式', [[node('验收', seat('gpt')), node('验证', seat('glm'))]]);
const jury = mode('builtin-jury', '评审模式', [[node('验证', seat('glm'))]]);
const economy = mode('builtin-economy', '经济模式', [[node('出方案', seat('opus'))], [node('执行', seat('sonnet'))], [node('验收', seat('glm'))]]);
const arena = mode('builtin-arena', '擂台模式', [[node('执行', seat('sonnet'))]]);
const custom = mode('custom-1', '自定义模式', [[node('执行', seat('sonnet'))]]);
const MODES = [economy, blind, arena, jury, custom];

function loc(overrides: Partial<SkillLocation> = {}): SkillLocation {
  return { scope: 'global', path: 'C:\\Users\\u\\.claude\\skills\\ai-cross', kind: 'none', version: '', linkTarget: null, readByThisHost: true, writeTarget: true, ...overrides };
}

function hostStatus(locations: SkillLocation[]): SkillHostStatus {
  return { host: 'claude', locations, duplicate: false, updateAvailable: false };
}

describe('找到的 agent', () => {
  it('只算探测过且装了的，模型只数可选的', () => {
    const resources: ResourcesOutput = {
      detected_at: '2026-10-25T10:00:00+08:00',
      agents: [
        { id: 'claude', name: 'Claude Code', version: '', state: '可用', note: '', models: [
          { id: 'opus', agent: 'claude', provider: '', resolved_id: 'opus', state: '可用', note: '' },
        ] },
        { id: 'pi', name: 'pi agent', version: '', state: '可用', note: '', models: [
          { id: 'glm', agent: 'pi', provider: 'zai', resolved_id: 'glm', state: '已验证', note: '' },
          { id: 'qwen', agent: 'pi', provider: 'q', resolved_id: 'qwen', state: '未验证', note: '' },
        ] },
        { id: 'codex', name: 'Codex', version: '', state: '未安装', note: '', models: [] },
      ],
    };
    expect(foundAgents(resources)).toEqual({ names: ['Claude Code', 'pi agent'], models: 2 });
    expect(foundAgents(null)).toEqual({ names: [], models: 0 });
  });
});

describe('审与干活各用哪个模式', () => {
  it('默认模式合适就用它', () => {
    expect(reviewMode(MODES, 'builtin-jury')?.id).toBe('builtin-jury');
    expect(workMode(MODES, 'builtin-arena')?.id).toBe('builtin-arena');
  });

  it('不合适时审用盲审模式、干活用经济模式', () => {
    expect(reviewMode(MODES, 'builtin-economy')?.id).toBe('builtin-blind');
    expect(workMode(MODES, 'builtin-blind')?.id).toBe('builtin-economy');
    expect(workMode(MODES, 'custom-1')?.id).toBe('builtin-economy');
    expect(reviewMode(MODES, '')?.id).toBe('builtin-blind');
  });

  it('模式不存在时为 null', () => {
    expect(reviewMode([economy], 'builtin-economy')).toBeNull();
  });
});

describe('位置行', () => {
  it('列出角色与模型，未选与失效标出', () => {
    const m = mode('builtin-blind', '盲审模式', [[
      node('验收', seat('gpt', { agent: 'codex', provider: null })),
      node('验证', null),
      node('验证', seat('glm', { state: '验证失败' })),
    ]]);
    expect(seatLines(m)).toEqual([
      { role: '验收', model: 'gpt', title: 'codex / gpt', alert: false },
      { role: '验证', model: '', title: '', alert: true },
      { role: '验证', model: 'glm', title: 'pi / zai / glm', alert: true },
    ]);
    expect(seatLines(null)).toEqual([]);
  });
});

describe('配好没有', () => {
  const base = {
    detected: true,
    host: 'claude',
    skillHost: hostStatus([loc({ kind: 'ours', version: BUNDLED })]),
    skillLoaded: true,
    bundledVersion: BUNDLED,
    modes: MODES,
    defaultMode: 'builtin-economy',
  };

  it('skill 装好、审与干活都有人选才算完成', () => {
    expect(setupState(base)).toBe('done');
  });

  it('先要探测、要有主 agent', () => {
    expect(setupState({ ...base, detected: false })).toBe('need-detect');
    expect(setupState({ ...base, host: '' })).toBe('need-host');
  });

  it('skill 未安装或可更新算未完成', () => {
    expect(setupState({ ...base, skillHost: hostStatus([loc()]) })).toBe('todo');
    expect(setupState({ ...base, skillHost: hostStatus([loc({ kind: 'ours', version: '1.21.3' })]) })).toBe('todo');
    expect(setupState({ ...base, skillLoaded: false })).toBe('todo');
  });

  it('其他软件管理的 skill 算装好', () => {
    const skillHost = hostStatus([loc(), loc({ kind: 'external', version: '1.22.0', path: 'C:\\x', writeTarget: false })]);
    expect(setupState({ ...base, skillHost })).toBe('done');
  });

  it('其他软件装的旧版算未完成', () => {
    const skillHost = hostStatus([loc(), loc({ kind: 'external', version: '1.12.1', outdated: true, path: 'C:/x', writeTarget: false })]);
    expect(setupState({ ...base, skillHost })).toBe('todo');
  });

  it('有位置没人选算未完成', () => {
    const emptyBlind = mode('builtin-blind', '盲审模式', [[node('验收', null)]]);
    expect(setupState({ ...base, modes: [economy, emptyBlind] })).toBe('todo');
  });
});

describe('要不要装 skill', () => {
  it('未安装或我们装的旧版要装，最新与其他软件管理的不动', () => {
    expect(needsSkillInstall(hostStatus([loc()]), BUNDLED)).toBe(true);
    expect(needsSkillInstall(null, BUNDLED)).toBe(true);
    expect(needsSkillInstall(hostStatus([loc({ kind: 'ours', version: '1.21.3' })]), BUNDLED)).toBe(true);
    expect(needsSkillInstall(hostStatus([loc({ kind: 'ours', version: BUNDLED })]), BUNDLED)).toBe(false);
    expect(needsSkillInstall(hostStatus([loc({ kind: 'external', version: '1.0.0' })]), BUNDLED)).toBe(false);
  });
});

describe('示例输入', () => {
  it('调用写法优先用引擎给的，读不到按主 agent 默认', () => {
    expect(invokeText('codex', '$ai-cross')).toBe('$ai-cross');
    expect(invokeText('codex', '')).toBe('$ai-cross');
    expect(invokeText('kimi', null)).toBe('/skill:ai-cross');
    expect(invokeText('other', undefined)).toBe('/ai-cross');
  });

  it('审与干活两条，带调用写法', () => {
    const examples = usageExamples('/ai-cross');
    expect(examples.map((example) => example.label)).toEqual(['审', '干活']);
    expect(examples.every((example) => example.text.startsWith('/ai-cross '))).toBe(true);
  });
});

describe('缺人提示', () => {
  const emptyBlind = mode('builtin-blind', '盲审模式', [[node('验收', seat('gpt')), node('验证', null), node('验证', null)]]);
  const notes = [
    '「盲审模式」第 1 阶段第 2 个位置（验证）：无可用人选：需不同于主 agent 厂商 DeepSeek，且同阶段不重复；另有 8 个未验证模型，测试连接后可参与推荐',
    '「盲审模式」第 1 阶段第 3 个位置（验证）：无可用人选：需不同于主 agent 厂商 DeepSeek，且同阶段不重复；另有 8 个未验证模型，测试连接后可参与推荐',
    '「评审模式」第 1 阶段第 1 个位置（验证）：候选池为空：先探测资源，第三方模型要先冒烟通过',
  ];

  it('只算开始页上的两个模式，原因去重并改成读者能懂的话', () => {
    expect(setupHint([emptyBlind, economy], notes)).toEqual({
      missing: 2,
      reasons: ['审查要用和主 agent（DeepSeek）不同厂商的模型，可用的不够'],
      unverified: 8,
    });
  });

  it('同一个模式出现两次只算一次；没有说明时原因为空', () => {
    expect(setupHint([emptyBlind, emptyBlind], [])).toEqual({ missing: 2, reasons: [], unverified: 0 });
  });
});

describe('只能当主 agent 的软件', () => {
  it('主 agent 里不在派发清单的单独列出', () => {
    const hosts = [{ name: 'Claude Code' }, { name: 'WorkBuddy' }, { name: 'OpenCode' }];
    expect(hostOnlyNames(hosts, ['Claude Code', 'Codex'])).toEqual(['WorkBuddy', 'OpenCode']);
    expect(hostOnlyNames(null, [])).toEqual([]);
  });
});
