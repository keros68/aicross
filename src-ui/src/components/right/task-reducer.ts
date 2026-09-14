// Pure list operations behind the 任务 tab (TASK-05 C 节).
//
// The board itself is upstream React with the mutations inlined in
// `setTasks(prev => ...)` callbacks. Those bodies moved here unchanged so each
// one is directly testable: every function takes the current array and returns
// a new one, and none of them touch React, the DOM, or the clock.

import { NEXT_STATUS, STATUS_ORDER, type TaskItem, type TaskStatus } from './task-types';

/** Where a dragged card was released. `id` null = an empty status section. */
export interface DropTarget {
  id: string | null;
  status: TaskStatus | null;
  position: 'before' | 'after';
}

/** New tasks land on top of the list. */
export function addTask(tasks: TaskItem[], task: TaskItem): TaskItem[] {
  return [task, ...tasks];
}

export function removeTask(tasks: TaskItem[], id: string): TaskItem[] {
  return tasks.filter((t) => t.id !== id);
}

/** Checkbox click: 待办 → 进行中 → 完成 → 待办. */
export function cycleStatus(tasks: TaskItem[], id: string): TaskItem[] {
  return tasks.map((t) => (t.id === id ? { ...t, status: NEXT_STATUS[t.status] } : t));
}

/** Direct status set, used by the note view's status dots. */
export function setTaskStatus(tasks: TaskItem[], id: string, status: TaskStatus): TaskItem[] {
  return tasks.map((t) => (t.id === id ? { ...t, status } : t));
}

/** Field write-through for title / description / note height. */
export function patchTask(
  tasks: TaskItem[],
  id: string,
  patch: Partial<Omit<TaskItem, 'id'>>,
): TaskItem[] {
  return tasks.map((t) => (t.id === id ? { ...t, ...patch } : t));
}

/** Prompt view: retitle every item sharing `oldTitle` (the group moves as one). */
export function renameCategory(tasks: TaskItem[], oldTitle: string, newTitle: string): TaskItem[] {
  return tasks.map((t) => (t.title === oldTitle ? { ...t, title: newTitle } : t));
}

/** Copies a task in place, right after the original. Ids/timestamps injected. */
export function duplicateTask(
  tasks: TaskItem[],
  id: string,
  newId: string,
  createdAt: number,
): TaskItem[] {
  const index = tasks.findIndex((t) => t.id === id);
  if (index < 0) return tasks;
  const copy: TaskItem = { ...tasks[index], id: newId, createdAt };
  const next = [...tasks];
  next.splice(index + 1, 0, copy);
  return next;
}

/**
 * Drag-reorder. The board renders tasks grouped by status in `STATUS_ORDER`,
 * so a drop first flattens to that render order, then re-inserts the dragged
 * task at the drop point — which may also move it to another status section.
 */
export function moveTask(tasks: TaskItem[], id: string, drop: DropTarget): TaskItem[] {
  if (!drop.id && !drop.status) return tasks;

  const ordered = STATUS_ORDER.flatMap((s) => tasks.filter((t) => t.status === s));
  const draggedIdx = ordered.findIndex((t) => t.id === id);
  if (draggedIdx === -1) return tasks;

  const dragged = ordered[draggedIdx];
  const newStatus = drop.status ?? dragged.status;
  const updated: TaskItem = { ...dragged, status: newStatus };
  const without = ordered.filter((t) => t.id !== id);

  if (drop.id) {
    const targetIdx = without.findIndex((t) => t.id === drop.id);
    if (targetIdx !== -1) {
      without.splice(drop.position === 'before' ? targetIdx : targetIdx + 1, 0, updated);
    } else {
      without.push(updated);
    }
    return without;
  }

  // Dropped on a section header: append at the end of that section, or at the
  // section's would-be position when the section is currently empty.
  const sectionSize = without.filter((t) => t.status === newStatus).length;
  let insertAt = 0;
  if (sectionSize === 0) {
    const sIdx = STATUS_ORDER.indexOf(newStatus);
    for (let si = 0; si < sIdx; si += 1) {
      insertAt += without.filter((t) => t.status === STATUS_ORDER[si]).length;
    }
  } else {
    for (let i = 0; i < without.length; i += 1) {
      if (without[i].status === newStatus) insertAt = i + 1;
    }
  }
  without.splice(insertAt, 0, updated);
  return without;
}

const STATUSES: TaskStatus[] = ['todo', 'working', 'done'];

/**
 * Parses `~/.aicross/tasks.json`'s payload.
 *
 * Returns `null` when the text is not a task array — a damaged file, or one
 * written by something else. The board treats `null` as "leave the file
 * alone": it neither shows tasks nor saves over them, so a parse failure can
 * never turn into data loss. An empty array is a valid, empty board.
 */
export function parseTasks(raw: string): TaskItem[] | null {
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    return null;
  }
  if (!Array.isArray(value)) return null;

  const out: TaskItem[] = [];
  for (const entry of value) {
    if (typeof entry !== 'object' || entry === null) return null;
    const item = entry as Record<string, unknown>;
    if (typeof item.id !== 'string' || item.id === '') return null;
    if (typeof item.title !== 'string') return null;
    const status = STATUSES.includes(item.status as TaskStatus)
      ? (item.status as TaskStatus)
      : 'todo';
    out.push({
      id: item.id,
      title: item.title,
      description: typeof item.description === 'string' ? item.description : undefined,
      status,
      createdAt: typeof item.createdAt === 'number' ? item.createdAt : 0,
      height: typeof item.height === 'number' ? item.height : undefined,
    });
  }
  return out;
}
