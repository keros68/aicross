// 模型选择下拉的纯逻辑（fix-51-r1，参照 magpie 的 picker）：搜索过滤、按
// agent / provider 分组、键盘上下移动（跳过禁用项、首尾循环）。组件在
// ModelPicker.tsx，测试落在这层。

import type { SeatOption } from './config-view';

export interface PickerGroup {
  key: string;
  /** 组标题：原生模型为 agent，有 provider 时为「agent · provider」。 */
  label: string;
  agent: string;
  provider: string | null;
  items: SeatOption[];
}

/** 按空白分词，每个词都要命中 agent、provider、模型或状态之一（不分大小写）。 */
export function filterOptions(options: SeatOption[], query: string): SeatOption[] {
  const terms = query.trim().toLowerCase().split(/\s+/).filter((term) => term !== '');
  if (terms.length === 0) return options;
  return options.filter((option) => {
    const haystack = [option.agent, option.provider ?? '', option.model, option.state].join(' ').toLowerCase();
    return terms.every((term) => haystack.includes(term));
  });
}

/** 按 agent + provider 分组，组序与组内顺序都按首次出现。 */
export function groupOptions(options: SeatOption[]): PickerGroup[] {
  const groups: PickerGroup[] = [];
  for (const option of options) {
    const key = `${option.agent}\u0000${option.provider ?? ''}`;
    let group = groups.find((item) => item.key === key);
    if (!group) {
      group = {
        key,
        label: option.provider ? `${option.agent} · ${option.provider}` : option.agent,
        agent: option.agent,
        provider: option.provider,
        items: [],
      };
      groups.push(group);
    }
    group.items.push(option);
  }
  return groups;
}

/** 分组后的展示顺序，键盘移动按这个顺序走。 */
export function flattenGroups(groups: PickerGroup[]): SeatOption[] {
  return groups.flatMap((group) => group.items);
}

/** 第一个可选项的下标；都不可选时为 -1。 */
export function firstEnabled(items: { disabled: boolean }[]): number {
  return items.findIndex((item) => !item.disabled);
}

/** 从 current 按 delta（1 下 / -1 上）移到下一个可选项，首尾循环；没有可选项时为 -1。
 *  current 为 -1 时向下从第一个、向上从最后一个开始找。 */
export function moveActive(items: { disabled: boolean }[], current: number, delta: 1 | -1): number {
  const count = items.length;
  if (count === 0) return -1;
  let index = current < 0 ? (delta > 0 ? -1 : count) : current;
  for (let step = 0; step < count; step += 1) {
    index = (index + delta + count) % count;
    if (!items[index].disabled) return index;
  }
  return -1;
}

/** 弹出层的方向与列表最大高度：下方放得下整个弹出层就向下；否则向空间
 *  更大的一侧展开，列表压到可用高度（不低于 PICKER_LIST_MIN）。 */
export const PICKER_LIST_MAX = 320;
const PICKER_LIST_MIN = 120;
/** 搜索框、内边距与离触发器 / 窗口边缘的间隙合计。 */
const PICKER_CHROME = 76;

export function pickerPlacement(
  triggerTop: number,
  triggerBottom: number,
  viewportHeight: number,
): { up: boolean; listMax: number } {
  const below = viewportHeight - triggerBottom - PICKER_CHROME;
  const above = triggerTop - PICKER_CHROME;
  if (below >= PICKER_LIST_MAX) return { up: false, listMax: PICKER_LIST_MAX };
  const up = above > below;
  const room = up ? above : below;
  return { up, listMax: Math.max(PICKER_LIST_MIN, Math.min(PICKER_LIST_MAX, room)) };
}
