import { describe, expect, it } from 'vitest';
import { filterOptions, firstEnabled, flattenGroups, groupOptions, moveActive, pickerPlacement, PICKER_LIST_MAX } from '../picker';
import type { SeatOption } from '../config-view';

function option(agent: string, provider: string | null, model: string, state = '可用', disabled = false): SeatOption {
  return { key: `${agent}/${provider ?? ''}/${model}`, agent, provider, model, state, disabled };
}

const options = [
  option('claude', null, 'sonnet'),
  option('pi', 'zai', 'glm-5.3-flash', '已验证'),
  option('claude', null, 'haiku'),
  option('pi', 'deepseek', 'deepseek-v4-flash', '未发现', true),
  option('pi', 'zai', 'glm-5.3', '未验证'),
];

describe('模型选择：搜索（fix-51-r1）', () => {
  it('空查询原样返回', () => {
    expect(filterOptions(options, '  ')).toBe(options);
  });

  it('不分大小写，按空白分词、每个词都要命中 agent / provider / 模型 / 状态之一', () => {
    expect(filterOptions(options, 'GLM').map((item) => item.model)).toEqual(['glm-5.3-flash', 'glm-5.3']);
    expect(filterOptions(options, 'pi flash').map((item) => item.model)).toEqual(['glm-5.3-flash', 'deepseek-v4-flash']);
    expect(filterOptions(options, 'zai 已验证').map((item) => item.model)).toEqual(['glm-5.3-flash']);
    expect(filterOptions(options, 'gemini')).toEqual([]);
  });
});

describe('模型选择：分组（fix-51-r1）', () => {
  it('按 agent + provider 分组，组序与组内顺序按首次出现；原生模型标题只有 agent', () => {
    const groups = groupOptions(options);
    expect(groups.map((group) => [group.label, group.items.map((item) => item.model)])).toEqual([
      ['claude', ['sonnet', 'haiku']],
      ['pi · zai', ['glm-5.3-flash', 'glm-5.3']],
      ['pi · deepseek', ['deepseek-v4-flash']],
    ]);
    expect(flattenGroups(groups).map((item) => item.model)).toEqual(['sonnet', 'haiku', 'glm-5.3-flash', 'glm-5.3', 'deepseek-v4-flash']);
  });
});

describe('模型选择：键盘上下（fix-51-r1）', () => {
  const items = [{ disabled: true }, { disabled: false }, { disabled: true }, { disabled: false }];

  it('跳过禁用项，首尾循环', () => {
    expect(moveActive(items, 1, 1)).toBe(3);
    expect(moveActive(items, 3, 1)).toBe(1);
    expect(moveActive(items, 1, -1)).toBe(3);
  });

  it('还没有选中项时向下从第一个可选、向上从最后一个可选开始', () => {
    expect(moveActive(items, -1, 1)).toBe(1);
    expect(moveActive(items, -1, -1)).toBe(3);
    expect(firstEnabled(items)).toBe(1);
  });

  it('没有可选项时为 -1', () => {
    expect(moveActive([{ disabled: true }], 0, 1)).toBe(-1);
    expect(moveActive([], -1, 1)).toBe(-1);
    expect(firstEnabled([{ disabled: true }])).toBe(-1);
  });
});

describe('pickerPlacement', () => {
  it('opens downward with the full list when there is room below', () => {
    expect(pickerPlacement(100, 136, 900)).toEqual({ up: false, listMax: PICKER_LIST_MAX });
  });

  it('opens upward near the window bottom', () => {
    expect(pickerPlacement(700, 736, 800)).toEqual({ up: true, listMax: PICKER_LIST_MAX });
  });

  it('shrinks the list to the larger side when neither side fits', () => {
    expect(pickerPlacement(200, 236, 600)).toEqual({ up: false, listMax: 288 });
    expect(pickerPlacement(330, 366, 600)).toEqual({ up: true, listMax: 254 });
  });

  it('never shrinks below the minimum', () => {
    expect(pickerPlacement(100, 136, 220).listMax).toBe(120);
  });
});

describe('模型选择：未实测分组（TASK-62）', () => {
  it('所属 agent 未实测的分组带标记，其余不带', () => {
    const groups = groupOptions([
      { ...option('agy', null, 'gemini-3-pro'), untested: true },
      option('claude', null, 'sonnet'),
    ]);
    expect(groups.map((group) => [group.label, group.untested])).toEqual([['agy', true], ['claude', false]]);
  });
});
