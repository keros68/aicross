// 模式位置数量调节的纯函数（TASK-56）：按钮可点判定、减少时会清掉的人选与确认文字。
// 契约见 tasks/TASK-56-mode-counts.md 第 2 节：减少时从末尾去掉该角色的位置。

import type { ModeCount, ModeEntry } from '../tauri';

export function canDecrease(count: Pick<ModeCount, 'value' | 'min'>): boolean {
  return count.value > count.min;
}

export function canIncrease(count: Pick<ModeCount, 'value' | 'max'>): boolean {
  return count.value < count.max;
}

/** 减少到 next 时，末尾被去掉的该角色位置里有本模式人选的个数。 */
export function seatsLostOnDecrease(mode: Pick<ModeEntry, 'stages'>, role: string, next: number): number {
  const nodes = mode.stages.flat().filter((node) => node.role === role);
  return nodes.slice(Math.max(0, next)).filter((node) => node.seat !== null).length;
}

export function decreaseConfirmText(role: string, next: number, lost: number): string {
  return `减少到 ${next} 个${role}，末尾 ${lost} 个人选将被清除`;
}
