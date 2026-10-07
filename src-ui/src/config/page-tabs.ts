// 窗口各页的导航项与记忆（TASK-47；TASK-49 改为左侧导航，fix-49-r1 改顺序与默认页，
// TASK-51 加 skill 页，fix-51-r1 改为顶栏分段导航）。选中的页签存在 localStorage；读写都包 try/catch（隐私模式、
// 存储被禁时访问器本身会抛错），读不到或值不认就用默认。

export type PageTab = 'resources' | 'skill' | 'modes' | 'runs';

/** 按使用先后排：先探测资源，再给主 agent 装 skill、给模式选人选，最后看派发记录。
 *  skill 与模式两页按当前主 agent 显示。 */
export const PAGE_TABS: ReadonlyArray<{ id: PageTab; label: string }> = [
  { id: 'resources', label: '资源' },
  { id: 'skill', label: 'skill' },
  { id: 'modes', label: '模式' },
  { id: 'runs', label: '派发记录' },
];

/** 首次打开（没存过页签）进「资源」。 */
export const DEFAULT_PAGE_TAB: PageTab = 'resources';
export const PAGE_TAB_STORAGE_KEY = 'aicross.page-tab';

type TabStorage = Pick<Storage, 'getItem' | 'setItem'>;

/** 取 window.localStorage；访问器抛错或没有 window 时为 null。 */
export function pageTabStorage(): TabStorage | null {
  try {
    return typeof window === 'undefined' ? null : window.localStorage;
  } catch {
    return null;
  }
}

/** 各页挂载后常驻、切页签或打开查看页只隐藏：探测、测试连接、安装进行中切走再切回，
 * 页内的执行状态仍在，同一时间只跑一个不被绕过；切回时先显示已有数据、后台静默刷新，
 * 不再整页「读取中…」（TASK-58）。「派发记录」只在可见时轮询。 */
export function keepsMounted(tab: PageTab): boolean {
  return tab === 'resources' || tab === 'skill' || tab === 'modes' || tab === 'runs';
}

export function isPageTab(value: unknown): value is PageTab {
  return PAGE_TABS.some((tab) => tab.id === value);
}

export function readPageTab(storage: TabStorage | null): PageTab {
  try {
    const value = storage?.getItem(PAGE_TAB_STORAGE_KEY);
    return isPageTab(value) ? value : DEFAULT_PAGE_TAB;
  } catch {
    return DEFAULT_PAGE_TAB;
  }
}

export function writePageTab(storage: TabStorage | null, tab: PageTab): void {
  try {
    storage?.setItem(PAGE_TAB_STORAGE_KEY, tab);
  } catch {
    /* 存不进去只是下次回默认页签，不影响当前页面 */
  }
}
