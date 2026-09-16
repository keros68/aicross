// 左栏资源面板的展开/折叠状态。
//
// 为什么不能留在组件里（09-16 用户实测）：切到「工作区」时 Explorer 把
// ResourcesTab 整个卸载，`useState` 与 `initialized` ref 一起清零，回到「资源」
// 时首次加载分支又把四家全展开——用户刚折叠好的状态每次切标签都被推翻。
//
// 存两层：模块级缓存保证同一次运行内一定不丢；localStorage 让下次启动也记得。
// 存储不可用（隐私窗口、禁站点数据）时只掉持久化，不影响本次运行。

const STORAGE_KEY = 'aicross.resources.rail';

export interface RailState {
  /** 展开的 agent id；`null` 表示还没有用户选择，按「全展开」处理。 */
  expanded: string[] | null;
  /** 展开了未验证清单的小组，键是 `agentId/provider`。 */
  openUnverified: string[];
  /** 展开了未登录清单的小组。 */
  openLoggedOut: string[];
  /** 展开了未检查清单的小组。 */
  openUnknown: string[];
}

export const emptyRailState = (): RailState => ({
  expanded: null,
  openUnverified: [],
  openLoggedOut: [],
  openUnknown: [],
});

const stringList = (value: unknown): string[] =>
  Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : [];

/** 把存下来的任意 JSON 归一成 RailState；形状不对就当没存过。 */
export function parseRailState(raw: string | null): RailState {
  if (!raw) return emptyRailState();
  try {
    const value = JSON.parse(raw) as Partial<RailState> | null;
    if (!value || typeof value !== 'object') return emptyRailState();
    return {
      expanded: value.expanded === null || value.expanded === undefined
        ? null
        : stringList(value.expanded),
      openUnverified: stringList(value.openUnverified),
      openLoggedOut: stringList(value.openLoggedOut),
      openUnknown: stringList(value.openUnknown),
    };
  } catch {
    return emptyRailState();
  }
}

/**
 * 首屏该展开哪些 agent：用户动过就听用户的（哪怕一个都不展开），
 * 从没动过才按「全展开」。这一条是修复的核心——原来只要组件重挂载就回到全展开。
 */
export function initialExpanded(state: RailState, agentIds: string[]): string[] {
  if (state.expanded === null) return [...agentIds];
  return state.expanded.filter((id) => agentIds.includes(id));
}

export function toggleInList(list: string[], key: string): string[] {
  return list.includes(key) ? list.filter((item) => item !== key) : [...list, key];
}

interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

let cached: RailState | null = null;

export function readRailState(storage: StorageLike | null = safeStorage()): RailState {
  if (cached) return cached;
  let raw: string | null = null;
  try {
    raw = storage?.getItem(STORAGE_KEY) ?? null;
  } catch {
    raw = null;
  }
  cached = parseRailState(raw);
  return cached;
}

export function writeRailState(next: RailState, storage: StorageLike | null = safeStorage()): void {
  cached = next;
  try {
    storage?.setItem(STORAGE_KEY, JSON.stringify(next));
  } catch {
    // 本次运行仍按模块级缓存生效，只是下次启动回到默认。
  }
}

/** 测试用：清掉模块级缓存。 */
export function resetRailStateCache(): void {
  cached = null;
}

function safeStorage(): StorageLike | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage;
  } catch {
    return null;
  }
}
