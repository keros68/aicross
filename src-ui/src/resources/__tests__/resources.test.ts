import { describe, expect, it } from 'vitest';
import {
  agentMark,
  connectedByAgent,
  groupModelsByProvider,
  parseCommandArgs,
  resourceSummary,
  resourceSummaryText,
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

describe('connectedByAgent', () => {
  it('只保留已接入模型，空组不出现', () => {
    const s = store([
      model({ agent: 'pi', id: 'a', status: '已接入' }),
      model({ agent: 'pi', id: 'b', status: '待冒烟' }),
      model({ agent: 'codex', id: 'c', status: '冒烟失败' }),
    ]);
    const groups = connectedByAgent(s);
    expect(groups).toHaveLength(1);
    expect(groups[0].id).toBe('pi');
    expect(groups[0].models.map((m) => m.id)).toEqual(['a']);
  });
});

describe('resourceSummary', () => {
  it('统计待冒烟与未接入', () => {
    const s = store([
      model({ agent: 'pi', id: 'a', status: '待冒烟' }),
      model({ agent: 'pi', id: 'b', status: '待冒烟' }),
      model({ agent: 'pi', id: 'c', status: '未接入' }),
      model({ agent: 'pi', id: 'd', status: '' }),
      model({ agent: 'pi', id: 'e', status: '已接入' }),
      model({ agent: 'pi', id: 'f', status: '冒烟失败' }),
    ]);
    expect(resourceSummary(s)).toEqual({ pending: 2, disconnected: 2 });
    expect(resourceSummaryText(s)).toBe('2 个待冒烟 · 2 个未接入 · 管理 →');
  });

  it('没有待办时只显示管理入口', () => {
    const s = store([model({ agent: 'pi', id: 'a', status: '已接入' })]);
    expect(resourceSummaryText(s)).toBe('管理 →');
  });
});

describe('groupModelsByProvider', () => {
  it('按 provider 分组，无 provider 并成空组', () => {
    const groups = groupModelsByProvider([
      model({ id: 'a', provider: 'zai' }),
      model({ id: 'b', provider: 'zai' }),
      model({ id: 'c', provider: '' }),
    ]);
    expect(groups).toHaveLength(2);
    expect(groups.find((g) => g.label === 'zai')?.models).toHaveLength(2);
    expect(groups.find((g) => g.label === '')?.models).toHaveLength(1);
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
