import { describe, expect, it } from 'vitest';
import { parseNotation, printNotation } from '../notation.ts';
import { roleDefaults } from '../types.ts';
import type { DispatchableModel, Preset, Stage, Target } from '../types.ts';

function mkModel(id: string, agent: string, vendor = 'v'): DispatchableModel {
  return {
    id,
    agent,
    provider: 'p',
    vendor,
    billing_source: 'b',
    tier: 't',
    thinking: '中',
    status: '已接入',
    source: 's',
    smoked_at: '',
    smoke_model: '',
    smoke_failure: '',
    meta: '',
    note: '',
  };
}

const MODELS: DispatchableModel[] = [
  mkModel('fable-5', 'claude', 'anthropic'),
  mkModel('gpt-5.6-terra', 'codex', 'openai'),
  mkModel('k3', 'kimi', 'moonshot'),
  mkModel('glm-5.2', 'zhipu', 'zhipu'),
  mkModel('glm-5.3', 'zhipu', 'zhipu'),
];

function mkPreset(stages: Target[][]): Preset {
  return { id: 'p1', name: 'p', builtin: false, stages: stages.map((nodes) => ({ nodes })) };
}

function mkTarget(partial: Partial<Target>): Target {
  return {
    agent: '',
    model: '',
    role: '执行',
    thinking: '中',
    visibility: '共享',
    write: '只读',
    ...partial,
  };
}

function roundtrip(preset: Preset, models: DispatchableModel[]): Stage[] {
  const { text, lossy } = printNotation(preset);
  expect(lossy).toEqual([]); // 这些用例不含记法外字段
  const res = parseNotation(text, models);
  expect(res.error).toBeUndefined();
  expect(res.preset).toBeDefined();
  return res.preset!.stages;
}

describe('parseNotation', () => {
  it('任务书示例：分阶段、并行组、组后缀 @blind、裸模型名唯一匹配', () => {
    const res = parseNotation('fable-5 → [gpt-5.6-terra | k3 | glm-5.2]@blind → glm-5.3', MODELS);
    expect(res.error).toBeUndefined();
    const stages = res.preset!.stages;
    expect(stages).toHaveLength(3);
    expect(stages[0].nodes).toHaveLength(1);
    expect(stages[0].nodes[0]).toMatchObject({ agent: 'claude', model: 'fable-5', role: '执行', thinking: '中', visibility: '共享', write: '只读' });
    expect(stages[1].nodes).toHaveLength(3);
    for (const n of stages[1].nodes) expect(n.visibility).toBe('盲');
    expect(stages[1].nodes.map((n) => `${n.agent}/${n.model}`)).toEqual([
      'codex/gpt-5.6-terra',
      'kimi/k3',
      'zhipu/glm-5.2',
    ]);
    expect(stages[2].nodes[0]).toMatchObject({ agent: 'zhipu', model: 'glm-5.3' });
    expect(res.unresolved).toBeUndefined();
  });

  it('消歧失败：裸模型名不唯一时报错并列出候选', () => {
    const models = [mkModel('dup', 'a1'), mkModel('dup', 'a2')];
    const res = parseNotation('dup', models);
    expect(res.preset).toBeUndefined();
    expect(res.error).toBeDefined();
    expect(res.error!.pos).toBe(0);
    expect(res.error!.message).toContain('不唯一');
    expect(res.error!.message).toContain('a1/dup');
    expect(res.error!.message).toContain('a2/dup');
  });

  it('非法后缀：报错带位置与原因', () => {
    const line = 'fable-5 @blur';
    const res = parseNotation(line, MODELS);
    expect(res.preset).toBeUndefined();
    expect(res.error).toBeDefined();
    expect(res.error!.pos).toBe(line.indexOf('@'));
    expect(res.error!.message).toContain('未知可见性后缀');
  });

  it('组后缀被节点自己的后缀覆盖', () => {
    const res = parseNotation('[k3 | glm-5.2@shared #high]@blind :wt', MODELS);
    expect(res.error).toBeUndefined();
    const [a, b] = res.preset!.stages[0].nodes;
    expect(a.visibility).toBe('盲');
    expect(a.write).toBe('worktree');
    expect(a.thinking).toBe('中');
    expect(b.visibility).toBe('共享');
    expect(b.thinking).toBe('高');
    expect(b.write).toBe('worktree');
  });

  it('空模型 ?：agent 与 model 留空', () => {
    const res = parseNotation('? → k3', MODELS);
    expect(res.error).toBeUndefined();
    expect(res.preset!.stages[0].nodes[0]).toMatchObject({ agent: '', model: '' });
    expect(res.unresolved).toBeUndefined();
  });

  it('自定义角色（含中文），未识别的角色取共享+只读缺省', () => {
    const res = parseNotation('k3!红队对抗 → glm-5.3!验收', MODELS);
    expect(res.error).toBeUndefined();
    const [s1, s2] = res.preset!.stages;
    expect(s1.nodes[0].role).toBe('红队对抗');
    expect(s1.nodes[0].visibility).toBe('共享');
    expect(s1.nodes[0].write).toBe('只读');
    expect(s2.nodes[0].role).toBe('验收');
    expect(s2.nodes[0].visibility).toBe('盲');
  });

  it('空 models 列表：按原样接受并标 unresolved，不算解析错误', () => {
    const res = parseNotation('foo-bar → codex/baz-1', []);
    expect(res.error).toBeUndefined();
    expect(res.preset).toBeDefined();
    expect(res.unresolved).toEqual(['foo-bar', 'codex/baz-1']);
    expect(res.preset!.stages[0].nodes[0]).toMatchObject({ agent: '', model: 'foo-bar' });
    expect(res.preset!.stages[1].nodes[0]).toMatchObject({ agent: 'codex', model: 'baz-1' });
  });

  it('已接入列表里匹配不到的模型标 unresolved', () => {
    const res = parseNotation('ghost-9', MODELS);
    expect(res.error).toBeUndefined();
    expect(res.unresolved).toEqual(['ghost-9']);
  });

  it('接受 ASCII -> 作为阶段分隔', () => {
    const res = parseNotation('k3 -> glm-5.3', MODELS);
    expect(res.error).toBeUndefined();
    expect(res.preset!.stages).toHaveLength(2);
  });
});

describe('printNotation 往返', () => {
  it('内置预设：共享对话', () => {
    const preset = mkPreset([[mkTarget({ role: '执行' })]]);
    expect(roundtrip(preset, MODELS)).toEqual(preset.stages);
  });

  it('内置预设：经济流水线（空模型、角色缺省联动）', () => {
    const preset = mkPreset([
      [mkTarget({ role: '出方案', thinking: '高' })],
      [mkTarget({ role: '执行', thinking: '低' })],
      [mkTarget({ role: '验收', ...roleDefaults('验收') })],
    ]);
    expect(roundtrip(preset, MODELS)).toEqual(preset.stages);
  });

  it('内置预设：盲审（一段三节点并行）', () => {
    const preset = mkPreset([
      [
        mkTarget({ role: '验证', visibility: '盲' }),
        mkTarget({ role: '验证', visibility: '盲' }),
        mkTarget({ role: '验证', visibility: '盲' }),
      ],
    ]);
    expect(roundtrip(preset, MODELS)).toEqual(preset.stages);
  });

  it('多阶段多并行混合（已接入模型解析回 agent/id）', () => {
    const preset = mkPreset([
      [mkTarget({ agent: 'claude', model: 'fable-5', role: '出方案', thinking: '高' })],
      [
        mkTarget({ agent: 'codex', model: 'gpt-5.6-terra', role: '执行', thinking: '低', write: '单写者' }),
        mkTarget({ agent: 'kimi', model: 'k3', role: '执行', thinking: '关', write: 'worktree' }),
      ],
      [mkTarget({ agent: 'zhipu', model: 'glm-5.3', role: '验证', visibility: '盲' })],
    ]);
    expect(roundtrip(preset, MODELS)).toEqual(preset.stages);
  });

  it('未接入模型（空列表）打印后仍可往返', () => {
    const preset = mkPreset([[mkTarget({ model: 'hand-1', role: '执行' })]]);
    const stages = roundtrip(preset, []);
    expect(stages).toEqual(preset.stages);
  });

  it('打印的正则形式：显式全部后缀，agent/model 带前缀', () => {
    const preset = mkPreset([
      [mkTarget({ agent: 'kimi', model: 'k3', role: '验收', thinking: '高', visibility: '盲', write: '只读' })],
    ]);
    expect(printNotation(preset).text).toBe('kimi/k3!验收#high@blind:ro');
  });
});

describe('printNotation lossy 标记', () => {
  it('provider/档位/指定上游列表不进记法，lossy 列出且可表达字段往返相等', () => {
    const preset = mkPreset([
      [
        mkTarget({
          agent: 'kimi',
          model: 'k3',
          role: '出方案',
          thinking: '高',
          provider: 'official',
          tier: '高',
          visibility: '指定上游',
          upstreams: ['node-a', 'node-b'],
        }),
      ],
    ]);
    const { text, lossy } = printNotation(preset);
    expect(lossy).toEqual(['provider', '档位', '指定上游']);
    const res = parseNotation(text, MODELS);
    expect(res.error).toBeUndefined();
    const t = res.preset!.stages[0].nodes[0];
    // 可表达字段逐一相等
    expect(t).toMatchObject({
      agent: 'kimi',
      model: 'k3',
      role: '出方案',
      thinking: '高',
      visibility: '指定上游',
      write: '只读',
    });
    // 记法外字段在解析结果里不存在
    expect(t.provider).toBeUndefined();
    expect(t.tier).toBeUndefined();
    expect(t.upstreams).toBeUndefined();
  });

  it('@upstream 解析回「指定上游」且 upstreams 为空时同样计入 lossy', () => {
    const res = parseNotation('k3@upstream', MODELS);
    expect(res.error).toBeUndefined();
    const t = res.preset!.stages[0].nodes[0];
    expect(t.visibility).toBe('指定上游');
    expect(t.upstreams).toBeUndefined();
    expect(printNotation(res.preset!).lossy).toEqual(['指定上游']);
  });

  it('空字符串的 provider/tier 不算 lossy', () => {
    const preset = mkPreset([[mkTarget({ provider: '', tier: '' })]]);
    expect(printNotation(preset).lossy).toEqual([]);
  });
});
