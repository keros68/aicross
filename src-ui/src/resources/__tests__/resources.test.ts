import { describe, expect, it } from 'vitest';
import {
  agentMark,
  groupKey,
  modelSecondaryId,
  modelsByAgent,
  parseCommandArgs,
  providerGroups,
  resolvedIdOf,
  splitUnverified,
  unverifiedCount,
  unverifiedSummaryText,
  unverifiedToggleLabel,
  verificationOf,
  type ResourceStore,
} from '../types';
import { appendMention } from '../mention';

function model(partial: Partial<import('../types').ResourceModel>): import('../types').ResourceModel {
  return {
    id: '',
    agent: '',
    provider: '',
    vendor: '',
    billing_source: '',
    tier: '',
    thinking: '',
    status: '',
    verification: '',
    resolved_id: '',
    source: '',
    smoked_at: '',
    smoke_model: '',
    smoke_failure: '',
    meta: '',
    note: '',
    ...partial,
  };
}

function store(models: import('../types').ResourceModel[]): ResourceStore {
  const agents = new Map<string, import('../types').ResourceModel[]>();
  for (const m of models) {
    agents.set(m.agent, [...(agents.get(m.agent) ?? []), m]);
  }
  return {
    version: 1,
    agents: [...agents.entries()].map(([id, agentModels]) => ({
      id,
      name: id,
      kind: '内置',
      detected: true,
      version: '1.0',
      models: agentModels,
    })),
  };
}

// TASK-09：verification 三态（+ 原生）。后端会写这个字段，这里钉住「字段为空
// 时按 agent + 旧 status 推断」这条兜底，映射与 Rust 的 inferred_verification 一致。
describe('verificationOf', () => {
  it('显式字段优先，四个合法值原样返回', () => {
    expect(verificationOf({ agent: 'pi', status: '待冒烟', verification: 'verified' })).toBe('verified');
    expect(verificationOf({ agent: 'claude', status: '已接入', verification: 'failed' })).toBe('failed');
    expect(verificationOf({ agent: 'pi', status: '', verification: 'unverified' })).toBe('unverified');
    expect(verificationOf({ agent: 'pi', status: '', verification: 'native' })).toBe('native');
  });

  it('原生三家（claude / codex / kimi）一律 native，不看旧 status', () => {
    for (const agent of ['claude', 'codex', 'kimi', 'CODEX']) {
      expect(verificationOf({ agent, status: '待冒烟', verification: '' })).toBe('native');
      expect(verificationOf({ agent, status: '冒烟失败', verification: '' })).toBe('native');
    }
  });

  it('第三方按旧 status 映射：已接入 → verified，冒烟失败 → failed，其余 → unverified', () => {
    expect(verificationOf({ agent: 'pi', status: '已接入', verification: '' })).toBe('verified');
    expect(verificationOf({ agent: 'pi', status: '冒烟失败', verification: '' })).toBe('failed');
    expect(verificationOf({ agent: 'pi', status: '待冒烟', verification: '' })).toBe('unverified');
    expect(verificationOf({ agent: 'pi', status: '', verification: '' })).toBe('unverified');
    expect(verificationOf({ agent: 'pi', status: '', verification: '乱写' })).toBe('unverified');
  });
});

describe('modelsByAgent', () => {
  it('列全部探测模型，组内按 原生 → 已验证 → 未验证 → 验证失败 排', () => {
    const s = store([
      model({ agent: 'pi', id: 'z-new', provider: 'zai', verification: 'unverified' }),
      model({ agent: 'pi', id: 'a-new', provider: 'zai', verification: 'unverified' }),
      model({ agent: 'pi', id: 'bad', provider: 'zai', verification: 'failed' }),
      model({ agent: 'pi', id: 'ok', provider: 'zai', verification: 'verified' }),
      model({ agent: 'codex', id: 'gpt', verification: 'native' }),
    ]);
    const groups = modelsByAgent(s);
    expect(groups.map((g) => g.id)).toEqual(['pi', 'codex']);
    expect(groups[0].models.map((m) => m.id)).toEqual(['ok', 'a-new', 'z-new', 'bad']);
    expect(groups[1].models.map((m) => m.id)).toEqual(['gpt']);
  });

  it('没有模型的 agent 不出现（未检测到的 CLI 在接入页管）', () => {
    const s: ResourceStore = {
      version: 1,
      agents: [{ id: 'kimi', name: 'Kimi', kind: '内置', detected: false, version: '', models: [] }],
    };
    expect(modelsByAgent(s)).toEqual([]);
  });
});

describe('unverifiedSummaryText', () => {
  it('只数未验证的第三方条目，原生与已验证不计', () => {
    const s = store([
      model({ agent: 'pi', id: 'a', verification: 'unverified' }),
      model({ agent: 'pi', id: 'b', verification: 'unverified' }),
      model({ agent: 'pi', id: 'c', verification: 'verified' }),
      model({ agent: 'pi', id: 'd', verification: 'failed' }),
      model({ agent: 'codex', id: 'gpt', status: '待冒烟', verification: '' }),
    ]);
    expect(unverifiedCount(s)).toBe(2);
    expect(unverifiedSummaryText(s)).toBe('2 个未验证（首次派发时自动验证）');
  });

  it('全部验过时不出这一行', () => {
    const s = store([model({ agent: 'pi', id: 'a', verification: 'verified' })]);
    expect(unverifiedSummaryText(s)).toBe('');
  });
});

// TASK-09b：pi 那几十行按 provider 分小组，原生 agent 不分组。
describe('providerGroups', () => {
  it('多 provider 按名排序分组，组内保留输入顺序', () => {
    const groups = providerGroups([
      model({ id: 'a', provider: 'zai' }),
      model({ id: 'b', provider: 'zai' }),
      model({ id: 'c', provider: 'bigmodel' }),
    ]);
    expect(groups.map((g) => g.provider)).toEqual(['bigmodel', 'zai']);
    expect(groups[1].models.map((m) => m.id)).toEqual(['a', 'b']);
  });

  it('只有一个 provider（原生 agent 的 provider 全是空串）时不分组', () => {
    const groups = providerGroups([
      model({ id: 'fable', provider: '' }),
      model({ id: 'opus', provider: '' }),
    ]);
    expect(groups).toHaveLength(1);
    expect(groups[0].provider).toBe('');
    expect(groups[0].models).toHaveLength(2);
    // 同一个 provider 的多条也不该长出组头
    expect(providerGroups([model({ id: 'a', provider: 'zai' })])[0].provider).toBe('');
  });

  it('没有模型时不出空组', () => {
    expect(providerGroups([])).toEqual([]);
  });

  it('组的折叠键带 agent：同名 provider 可能挂在两个 agent 下', () => {
    expect(groupKey('pi', 'zai')).toBe('pi/zai');
    expect(groupKey('claude', '')).toBe('claude/');
  });
});

describe('splitUnverified / unverifiedToggleLabel', () => {
  it('未验证折叠，已验证 / 验证失败 / 原生常显', () => {
    const { shown, collapsed } = splitUnverified([
      model({ agent: 'pi', id: 'ok', verification: 'verified' }),
      model({ agent: 'pi', id: 'new1', verification: 'unverified' }),
      model({ agent: 'pi', id: 'bad', verification: 'failed' }),
      model({ agent: 'pi', id: 'new2', verification: 'unverified' }),
      model({ agent: 'codex', id: 'gpt', verification: 'native' }),
    ]);
    expect(shown.map((m) => m.id)).toEqual(['ok', 'bad', 'gpt']);
    expect(collapsed.map((m) => m.id)).toEqual(['new1', 'new2']);
  });

  it('折叠行的文字带计数与展开 / 收起', () => {
    expect(unverifiedToggleLabel(37, false)).toBe('未验证 37 个 · 展开');
    expect(unverifiedToggleLabel(37, true)).toBe('未验证 37 个 · 收起');
  });
});

// TASK-09b：claude 的 fable/opus/sonnet/haiku 是别名，行内要看得到真身。
describe('resolvedIdOf / modelSecondaryId', () => {
  it('后端给的真身优先，空串退回 id', () => {
    expect(resolvedIdOf({ id: 'fable', resolved_id: 'claude-fable-5-1' })).toBe('claude-fable-5-1');
    expect(resolvedIdOf({ id: 'glm-4.7', resolved_id: '' })).toBe('glm-4.7');
    expect(resolvedIdOf({ id: 'glm-4.7', resolved_id: '   ' })).toBe('glm-4.7');
  });

  it('别名与 ID 相同的模型不重复显示', () => {
    expect(modelSecondaryId({ id: 'fable', resolved_id: 'claude-fable-5-1' })).toBe('claude-fable-5-1');
    expect(modelSecondaryId({ id: 'glm-4.7', resolved_id: 'glm-4.7' })).toBe('');
    expect(modelSecondaryId({ id: 'glm-4.7', resolved_id: '' })).toBe('');
  });
});

describe('agentMark', () => {
  it('取 id 前两位字母数字大写', () => {
    expect(agentMark('claude')).toBe('CL');
    expect(agentMark('pi')).toBe('PI');
    expect(agentMark('manual-1x')).toBe('MA');
  });
});

describe('parseCommandArgs', () => {
  it('空白拆分并支持引号', () => {
    expect(parseCommandArgs('--foo bar  --baz')).toEqual(['--foo', 'bar', '--baz']);
    expect(parseCommandArgs('--name "a b" \'c d\'')).toEqual(['--name', 'a b', 'c d']);
    expect(parseCommandArgs('')).toEqual([]);
  });

  it('引号未闭合抛错', () => {
    expect(() => parseCommandArgs('--name "abc')).toThrow('引号未闭合');
  });
});

describe('appendMention', () => {
  it('空草稿直接写入', () => {
    expect(appendMention('', '@pi/glm-4.7')).toBe('@pi/glm-4.7 ');
  });

  it('已有文本补空格分隔', () => {
    expect(appendMention('你好', '@pi/glm-4.7')).toBe('你好 @pi/glm-4.7 ');
    expect(appendMention('你好 ', '@pi/glm-4.7')).toBe('你好 @pi/glm-4.7 ');
  });
});
