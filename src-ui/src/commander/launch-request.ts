// 「在这个目录起总指挥」这个请求的收发（TASK-10 实现 3）。
//
// 发起方是左栏工作区的目录行，执行方是中栏 CenterPanel（tab 与 PTY 都归它
// 管）。两边没有父子关系，走一个 window 事件，与 `open-orchestration.ts`
// 同样的写法。这里不需要一次性标记：CenterPanel 一直挂着，不存在「事件发
// 出去时收方还没 mount」的情况。

export const COMMANDER_LAUNCH_EVENT = 'aicross:commander-launch';

export interface CommanderLaunchDetail {
  /** 在哪个目录起。空串 = 用默认里记着的目录。 */
  cwd: string;
}

/** 请求用已存的默认 agent/模型，在 `cwd` 起一个总指挥会话。 */
export function requestCommanderLaunch(cwd: string): void {
  if (typeof window === 'undefined') return;
  window.dispatchEvent(
    new CustomEvent<CommanderLaunchDetail>(COMMANDER_LAUNCH_EVENT, { detail: { cwd } }),
  );
}

export const COMMANDER_DEFAULTS_EVENT = 'aicross:commander-defaults';

/**
 * 默认变了（刚起过一次总指挥）。左栏的目录行只在有默认时才画那颗按钮，
 * 没有这个通知它要等到下次开应用才看得见。
 */
export function announceCommanderDefaults(): void {
  if (typeof window === 'undefined') return;
  window.dispatchEvent(new CustomEvent(COMMANDER_DEFAULTS_EVENT));
}
