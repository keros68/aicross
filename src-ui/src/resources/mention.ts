// 资源 → 对话图 / 接入页 的跨组件事件（TASK-07）。
// GraphTab 监听 insert-mention 把文本追加进主面板（列 0）输入框；
// TitleBar 监听 open-resources 打开接入页覆盖层。

export const INSERT_MENTION_EVENT = 'aicross:insert-mention';
export const OPEN_RESOURCES_EVENT = 'aicross:open-resources';
export const RESOURCES_CHANGED_EVENT = 'aicross:resources-changed';

/** 把 `@agent/模型` 插进当前对话图的主面板输入框。 */
export function insertMention(agent: string, model: string): void {
  window.dispatchEvent(
    new CustomEvent(INSERT_MENTION_EVENT, { detail: { text: `@${agent}/${model}` } }),
  );
}

/** 打开接入页（全屏覆盖层）。 */
export function openResourcesPage(): void {
  window.dispatchEvent(new CustomEvent(OPEN_RESOURCES_EVENT));
}

/** 资源数据变了，通知左栏「资源」tab 刷新。 */
export function notifyResourcesChanged(): void {
  window.dispatchEvent(new Event(RESOURCES_CHANGED_EVENT));
}

/** 把 mention 文本追加到既有草稿后面（补空格分隔），GraphTab 与测试共用。 */
export function appendMention(draft: string, text: string): string {
  const base = draft && !draft.endsWith(' ') ? `${draft} ` : draft;
  return `${base}${text} `;
}
