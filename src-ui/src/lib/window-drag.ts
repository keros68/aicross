// 无系统边框窗口的拖动：页头行与左侧导航顶部共用。按下左键（不在可点元素上）
// 调 startDragging()，双击切换最大化。不要再同时设 data-tauri-drag-region 或
// -webkit-app-region: drag：同一元素上两套拖动机制会与 Tauri 的拖动抢事件，
// 反向拖动时卡住重绘。

import type React from 'react';
import { commands, isTauri } from '../tauri';

/** 落在这些元素（或其内部）上的按下与双击交给元素自己，不拖窗口。 */
export const INTERACTIVE_SELECTOR = 'button, a, input, select, textarea, label, [role="button"]';

/** 按下的目标能否开始拖窗口：不在可点元素里才拖（标题文字、空白处可拖）。 */
export function startsWindowDrag(target: Pick<Element, 'closest'> | null): boolean {
  return target !== null && target.closest(INTERACTIVE_SELECTOR) === null;
}

export async function onWindowDragMouseDown(e: React.MouseEvent<HTMLElement>): Promise<void> {
  if (e.button !== 0) return;
  if (!startsWindowDrag(e.target as Element)) return;
  e.preventDefault();
  if (!isTauri) return;
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().startDragging();
  } catch { /* 拖动不可用时忽略 */ }
}

export function onWindowDragDoubleClick(e: React.MouseEvent<HTMLElement>): void {
  if (!startsWindowDrag(e.target as Element)) return;
  if (isTauri) commands.windowMaximize().catch(() => {});
}
