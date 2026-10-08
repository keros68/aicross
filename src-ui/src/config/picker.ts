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
  /** 所属 agent 未实测（TASK-62）。 */
  untested: boolean;
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
        untested: option.untested === true,
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

export interface PickerItem {
  key: string;
  disabled: boolean;
}

/** 键盘移动的顺序：额外项（如「清除人选」）只在查询为空时出现在最前，其后按分组展示顺序。 */
export function pickerItems(options: SeatOption[], query: string, extra: { key: string } | null | undefined): PickerItem[] {
  const flat = flattenGroups(groupOptions(filterOptions(options, query)))
    .map((option) => ({ key: option.key, disabled: option.disabled }));
  return extra && query.trim() === '' ? [{ key: extra.key, disabled: false }, ...flat] : flat;
}

/** 打开下拉时的选中项：打开时查询清空，所以按空查询的列表找当前值；没有当前值时取第一个可选项。 */
export function openActive(options: SeatOption[], extra: { key: string } | null | undefined, value: string): number {
  const items = pickerItems(options, '', extra);
  const selected = value === '' ? -1 : items.findIndex((item) => item.key === value);
  return selected >= 0 ? selected : firstEnabled(items);
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

export interface VerticalBounds {
  top: number;
  bottom: number;
}

/** 弹出层实际能显示的上下边界：窗口与各个会裁切内容的祖先（如滚动的页面正文）的交集。 */
export function visibleBounds(clips: VerticalBounds[], viewportHeight: number): VerticalBounds {
  return clips.reduce(
    (bounds, clip) => ({ top: Math.max(bounds.top, clip.top), bottom: Math.min(bounds.bottom, clip.bottom) }),
    { top: 0, bottom: viewportHeight },
  );
}

/** 弹出层的方向与列表最大高度：按可见边界（visibleBounds）算，下方放得下整个弹出层
 *  就向下；否则向空间更大的一侧展开，列表压到可用高度（不低于 PICKER_LIST_MIN）。 */
export const PICKER_LIST_MAX = 320;
const PICKER_LIST_MIN = 120;
/** 搜索框、内边距与离触发器 / 可见边界的间隙合计。 */
const PICKER_CHROME = 76;

export function pickerPlacement(
  triggerTop: number,
  triggerBottom: number,
  bounds: VerticalBounds,
): { up: boolean; listMax: number } {
  const below = bounds.bottom - triggerBottom - PICKER_CHROME;
  const above = triggerTop - bounds.top - PICKER_CHROME;
  if (below >= PICKER_LIST_MAX) return { up: false, listMax: PICKER_LIST_MAX };
  const up = above > below;
  const room = up ? above : below;
  return { up, listMax: Math.max(PICKER_LIST_MIN, Math.min(PICKER_LIST_MAX, room)) };
}

/** 展开期间的滚动或窗口缩放是否关闭弹层：位置按打开时算好，之后边界变了就关闭。
 *  滚动发生在弹层内部（模型列表）时不关。 */
export function closesOnViewportChange(type: 'scroll' | 'resize', insidePicker: boolean): boolean {
  return type === 'resize' || !insidePicker;
}

/** 弹层是否显示：下拉被禁用（如本行或其它位置正在保存）时，已展开的弹层也收起。 */
export function pickerShown(open: boolean, pickerDisabled: boolean): boolean {
  return open && !pickerDisabled;
}

/** 是否接受一次选择（回车或点击）：下拉被禁用时一律拒绝，避免保存进行中用旧草稿再发一次保存；
 *  未禁用时拒绝禁用项。 */
export function pickerAccepts(pickerDisabled: boolean, itemDisabled: boolean): boolean {
  return !pickerDisabled && !itemDisabled;
}
