// 「切到右栏『方案与编排』」这个请求的收发（TASK-05 C，R1 修正）。
//
// 原本只发一个 window 事件就够：右栏一直挂着。但首启引导的「选预设」那一步
// 可能先要把收起的右栏展开 —— 那一刻 RightPanel 还没 mount，事件没人听，
// 用户就停在 localStorage 记着的上一个 tab 上。
//
// 所以事件之外再留一个一次性标记：RightPanel mount 时读一次，读完即清。
// 已经挂着的 RightPanel 在事件回调里也清一次，免得它下次重新 mount 时
// 又莫名其妙跳到「方案与编排」。

export const OPEN_ORCHESTRATION_EVENT = 'aicross:open-orchestration';

export type RightTab = 'tasks' | 'orchestration';

let pending = false;

/** 请求右栏切到「方案与编排」。 */
export function requestOrchestrationPanel(): void {
  pending = true;
  if (typeof window !== 'undefined') {
    window.dispatchEvent(new CustomEvent(OPEN_ORCHESTRATION_EVENT));
  }
}

/** 取走待办请求；没有则返回 false。读完即清，重复调用不会再为真。 */
export function consumeOrchestrationRequest(): boolean {
  const was = pending;
  pending = false;
  return was;
}

/** 右栏 mount 时的初始 tab：待办请求优先于上次记住的 tab。 */
export function initialRightTab(saved: string | null, hasPendingRequest: boolean): RightTab {
  if (hasPendingRequest) return 'orchestration';
  if (saved === 'tasks' || saved === 'orchestration') return saved;
  return 'orchestration';
}
