// 任务板的载入流程（TASK-05 C，R1 修正）。
//
// 单独一层、依赖靠参数注入，为的是能在 vitest 里跑完整的分支 —— 尤其是
// 「Rust 读不出文件」这一支：它必须停写，不能回落成一块可写的空板，否则
// 下一次改动就把空数组写回 ~/.aicross/tasks.json，把原文件盖掉。

import { parseTasks } from './task-reducer';
import type { TaskItem } from './task-types';

export const LEGACY_STORAGE_KEY = 'coffee-tasks';

export interface TaskStorageIo {
  /** 是否在应用内运行。false 时只认 localStorage。 */
  isTauri: boolean;
  /** Rust 侧 load_tasks；文件存在但读不出时 reject。 */
  loadTasks: () => Promise<string>;
  /** Rust 侧 save_tasks，只在迁移旧 localStorage 数据时用到。 */
  saveTasks: (data: string) => Promise<void>;
  readLegacy: () => string | null;
  clearLegacy: () => void;
}

/** 载不进来的两种原因，界面文案按它分。 */
export type TaskLoadDamage = 'unreadable' | 'unparsable';

export type TaskLoadResult =
  | { kind: 'ok'; tasks: TaskItem[] }
  | { kind: 'damaged'; reason: TaskLoadDamage };

export async function loadTaskBoard(io: TaskStorageIo): Promise<TaskLoadResult> {
  if (io.isTauri) {
    let raw: string;
    try {
      raw = await io.loadTasks();
    } catch {
      // 文件在、但读不出来（权限、占用、IO 错）。绝不回落到 localStorage：
      // 那会给出一块可写的空板，然后把空数组写回真文件。
      return { kind: 'damaged', reason: 'unreadable' };
    }

    const tasks = parseTasks(raw);
    if (tasks === null) return { kind: 'damaged', reason: 'unparsable' };

    // 迁移：Rust 文件是空的、旧 localStorage 里还有数据 → 搬过去。
    if (tasks.length === 0) {
      try {
        const legacy = io.readLegacy();
        if (legacy) {
          const legacyTasks = parseTasks(legacy);
          if (legacyTasks && legacyTasks.length > 0) {
            await io.saveTasks(JSON.stringify(legacyTasks));
            io.clearLegacy();
            return { kind: 'ok', tasks: legacyTasks };
          }
        }
      } catch {
        // 迁移失败不影响载入，下次启动再试。
      }
    }

    return { kind: 'ok', tasks };
  }

  // 非 Tauri（浏览器里跑前端）：只有 localStorage，坏值直接当空板，
  // 这里没有需要保护的用户文件。
  try {
    const raw = io.readLegacy();
    if (raw) return { kind: 'ok', tasks: parseTasks(raw) ?? [] };
  } catch {
    /* localStorage 不可用 */
  }
  return { kind: 'ok', tasks: [] };
}
