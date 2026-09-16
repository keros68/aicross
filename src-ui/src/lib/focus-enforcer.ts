// 终端焦点守卫的判定。CenterPanel 有一对全局 focusin / mouseup 监听，任何
// 点击之后 10ms 会把焦点抢回当前激活的终端——这是为了让用户点完标签栏、
// 点完空白处还能直接打字。
//
// 模态弹窗是例外（09-16 窗口实测）：设置弹窗打开后，点弹窗里的任何按钮，
// 焦点都会被守卫抢回 xterm，于是 Esc 走进了 PTY（codex 把它当成「编辑上一
// 条」），弹窗自己挂在 document 上的 Esc 监听永远收不到，弹窗只能用 ✕ 关。
// 判定改成：焦点落在输入控件上、或落在任何对话框内部、或页面上存在 modal
// 弹窗时，都不抢。
//
// 入参写成结构化类型而不是 Element / Document：本仓的 vitest 跑在 node 环境
// 下（没有 jsdom），判定要能脱开真实 DOM 测。
export interface FocusCandidate {
  tagName: string;
  closest?(selector: string): unknown;
}

export interface DialogRoot {
  querySelector(selector: string): unknown;
}

export function hasOpenModal(root: DialogRoot): boolean {
  return Boolean(root.querySelector('[aria-modal="true"]'));
}

export function shouldEnforceTerminalFocus(
  active: FocusCandidate | null | undefined,
  root: DialogRoot,
): boolean {
  if (active && (active.tagName === 'INPUT' || active.tagName === 'TEXTAREA')) return false;
  if (active?.closest?.('[role="dialog"]')) return false;
  if (hasOpenModal(root)) return false;
  return true;
}
