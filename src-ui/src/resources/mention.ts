// 资源 → 对话图 / 接入页 的跨组件事件（TASK-07）。
// GraphTab 监听 insert-mention 把文本追加进主面板（列 0）输入框；
// TitleBar 监听 open-resources 打开接入页覆盖层。

export const INSERT_MENTION_EVENT = 'aicross:insert-mention';
export const OPEN_RESOURCES_EVENT = 'aicross:open-resources';
export const RESOURCES_CHANGED_EVENT = 'aicross:resources-changed';

/**
 * mention 引用文本。`@agent/模型` 表达不了 provider，pi 一家挂三个 provider
 * 时同名模型解析不出点的是哪一行，所以 provider 非空时补一个 `#provider`
 * 后缀（模型 id 里不会出现 `#`），由 `parseSend` 切回来（修正轮 R1）。
 */
export function mentionRef(agent: string, model: string, provider = ''): string {
  const tail = provider.trim() === '' ? '' : `#${provider.trim()}`;
  return `@${agent}/${model}${tail}`;
}

/** 把 `@agent/模型` 插进当前对话图的主面板输入框。 */
export function insertMention(agent: string, model: string, provider = ''): void {
  window.dispatchEvent(
    new CustomEvent(INSERT_MENTION_EVENT, { detail: { text: mentionRef(agent, model, provider) } }),
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

/** 把 Rust 引擎写回额度状态的 Tauri 事件转成现有页面使用的 window 事件。 */
export function subscribeResourcesChanged(): () => void {
  let stopped = false;
  let unlisten: (() => void) | undefined;
  import('@tauri-apps/api/event')
    .then(({ listen }) => listen(RESOURCES_CHANGED_EVENT, notifyResourcesChanged))
    .then((stop) => {
      if (stopped) stop();
      else unlisten = stop;
    })
    .catch(() => {});
  return () => {
    stopped = true;
    unlisten?.();
  };
}

/** 把 mention 文本追加到既有草稿后面（补空格分隔），GraphTab 与测试共用。 */
export function appendMention(draft: string, text: string): string {
  const base = draft && !draft.endsWith(' ') ? `${draft} ` : draft;
  return `${base}${text} `;
}
