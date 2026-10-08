import { describe, expect, it } from 'vitest';
import {
  filterOptions,
  firstEnabled,
  flattenGroups,
  groupOptions,
  moveActive,
  openActive,
  pickerItems,
  pickerPlacement,
  PICKER_LIST_MAX,
  visibleBounds,
  closesOnViewportChange,
  pickerAccepts,
  pickerShown,
} from '../picker';
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
  const win = (height: number) => ({ top: 0, bottom: height });

  it('opens downward with the full list when there is room below', () => {
    expect(pickerPlacement(100, 136, win(900))).toEqual({ up: false, listMax: PICKER_LIST_MAX });
  });

  it('opens upward near the window bottom', () => {
    expect(pickerPlacement(700, 736, win(800))).toEqual({ up: true, listMax: PICKER_LIST_MAX });
  });

  it('shrinks the list to the larger side when neither side fits', () => {
    expect(pickerPlacement(200, 236, win(600))).toEqual({ up: false, listMax: 288 });
    expect(pickerPlacement(330, 366, win(600))).toEqual({ up: true, listMax: 254 });
  });

  it('never shrinks below the minimum', () => {
    expect(pickerPlacement(100, 136, win(220)).listMax).toBe(120);
  });

  it('按滚动正文的可见范围算空间，标题栏与工具栏不算（fix-fullcheck C7）', () => {
    // 正文从 120 开始：触发器上方只剩 330 - 120 - 76 = 134，下方 600 - 366 - 76 = 158。
    expect(pickerPlacement(330, 366, { top: 120, bottom: 600 })).toEqual({ up: false, listMax: 158 });
    // 正文从 200 开始、下方更少时向上，列表压到正文顶边以内。
    expect(pickerPlacement(500, 536, { top: 200, bottom: 640 })).toEqual({ up: true, listMax: 224 });
  });
});

describe('弹出层的可见范围（fix-fullcheck C7）', () => {
  it('取窗口与各裁切祖先的交集', () => {
    expect(visibleBounds([{ top: 120, bottom: 700 }], 800)).toEqual({ top: 120, bottom: 700 });
    expect(visibleBounds([{ top: 120, bottom: 900 }, { top: 60, bottom: 650 }], 800)).toEqual({ top: 120, bottom: 650 });
    expect(visibleBounds([{ top: -40, bottom: 900 }], 800)).toEqual({ top: 0, bottom: 800 });
  });

  it('没有裁切祖先时为整个窗口', () => {
    expect(visibleBounds([], 800)).toEqual({ top: 0, bottom: 800 });
  });
});

describe('展开期间滚动或缩放（fix-fullcheck 第二轮）', () => {
  it('窗口缩放关闭弹层', () => {
    expect(closesOnViewportChange('resize', false)).toBe(true);
  });

  it('页面正文等外部滚动关闭弹层，弹层自己的列表滚动不关', () => {
    expect(closesOnViewportChange('scroll', false)).toBe(true);
    expect(closesOnViewportChange('scroll', true)).toBe(false);
  });
});

describe('下拉被禁用时（fix-fullcheck 第三轮）', () => {
  it('已展开的弹层在禁用后不再显示', () => {
    expect(pickerShown(true, false)).toBe(true);
    expect(pickerShown(true, true)).toBe(false);
    expect(pickerShown(false, false)).toBe(false);
  });

  it('禁用期间不接受选择（回车或点击），未禁用时只拒绝禁用项', () => {
    expect(pickerAccepts(true, false)).toBe(false);
    expect(pickerAccepts(true, true)).toBe(false);
    expect(pickerAccepts(false, true)).toBe(false);
    expect(pickerAccepts(false, false)).toBe(true);
  });
});

describe('重新打开下拉时的选中项（fix-fullcheck C5）', () => {
  const extra = { key: 'clear', label: '清除人选' };

  it('列表按空查询算：额外项在最前，其后按分组顺序', () => {
    expect(pickerItems(options, '', extra).map((item) => item.key)).toEqual([
      'clear',
      ...flattenGroups(groupOptions(options)).map((item) => item.key),
    ]);
    expect(pickerItems(options, 'glm', extra).map((item) => item.key)).toEqual([
      'pi/zai/glm-5.3-flash',
      'pi/zai/glm-5.3',
    ]);
  });

  it('搜索到当前人选、关闭、再打开：选中的仍是当前人选，不是「清除人选」', () => {
    const value = 'pi/zai/glm-5.3-flash';
    // 搜索时它是第 0 项；重开后查询清空，额外项回到第 0 项。
    expect(pickerItems(options, 'glm', extra).findIndex((item) => item.key === value)).toBe(0);
    const items = pickerItems(options, '', extra);
    const active = openActive(options, extra, value);
    expect(items[active].key).toBe(value);
    expect(active).not.toBe(0);
  });

  it('没有当前值时选第一个可选项', () => {
    expect(openActive(options, extra, '')).toBe(0);
    expect(openActive(options, null, '')).toBe(0);
    expect(openActive([option('pi', 'x', 'm', '未发现', true), option('pi', 'x', 'n')], null, '')).toBe(1);
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
