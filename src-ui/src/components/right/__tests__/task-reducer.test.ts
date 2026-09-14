import { describe, expect, it } from 'vitest';
import type { TaskItem } from '../task-types';
import {
  addTask,
  cycleStatus,
  duplicateTask,
  moveTask,
  parseTasks,
  patchTask,
  removeTask,
  renameCategory,
  setTaskStatus,
} from '../task-reducer';

function task(id: string, status: TaskItem['status'] = 'todo', title = id): TaskItem {
  return { id, title, status, createdAt: 1 };
}

const ids = (list: TaskItem[]) => list.map((t) => t.id).join(',');

describe('task-reducer', () => {
  it('adds on top and removes by id without touching the rest', () => {
    const base = [task('a'), task('b')];
    expect(ids(addTask(base, task('c')))).toBe('c,a,b');
    expect(ids(removeTask(base, 'a'))).toBe('b');
    expect(ids(removeTask(base, 'nope'))).toBe('a,b');
    expect(ids(base)).toBe('a,b'); // input untouched
  });

  it('cycles status 待办 → 进行中 → 完成 → 待办', () => {
    let list = [task('a')];
    expect(list[0].status).toBe('todo');
    list = cycleStatus(list, 'a');
    expect(list[0].status).toBe('working');
    list = cycleStatus(list, 'a');
    expect(list[0].status).toBe('done');
    list = cycleStatus(list, 'a');
    expect(list[0].status).toBe('todo');
  });

  it('sets status directly and patches fields', () => {
    const base = [task('a'), task('b')];
    expect(setTaskStatus(base, 'b', 'done')[1].status).toBe('done');
    expect(setTaskStatus(base, 'b', 'done')[0].status).toBe('todo');

    const patched = patchTask(base, 'a', { title: '写手册', description: '第一版' });
    expect(patched[0].title).toBe('写手册');
    expect(patched[0].description).toBe('第一版');
    expect(patched[1]).toBe(base[1]); // untouched items keep identity
  });

  it('renames every item in a prompt category at once', () => {
    const base = [task('a', 'todo', '翻译'), task('b', 'todo', '翻译'), task('c', 'todo', '改写')];
    const renamed = renameCategory(base, '翻译', '译文');
    expect(renamed.map((t) => t.title)).toEqual(['译文', '译文', '改写']);
  });

  it('duplicates a task right after the original with a fresh id', () => {
    const base = [task('a'), task('b')];
    const next = duplicateTask(base, 'a', 'a2', 99);
    expect(ids(next)).toBe('a,a2,b');
    expect(next[1].createdAt).toBe(99);
    expect(next[1].title).toBe(next[0].title);
    expect(duplicateTask(base, 'missing', 'x', 0)).toBe(base);
  });

  describe('moveTask', () => {
    // Render order is 进行中 → 待办 → 完成, so the flattened list is w1,w2,t1,t2,d1.
    const board = [
      task('t1', 'todo'),
      task('d1', 'done'),
      task('w1', 'working'),
      task('t2', 'todo'),
      task('w2', 'working'),
    ];

    it('reorders inside one section', () => {
      const next = moveTask(board, 't2', { id: 't1', status: 'todo', position: 'before' });
      expect(ids(next)).toBe('w1,w2,t2,t1,d1');
    });

    it('moves a task to another section and takes that status', () => {
      const next = moveTask(board, 't1', { id: 'w1', status: 'working', position: 'after' });
      expect(ids(next)).toBe('w1,t1,w2,t2,d1');
      expect(next.find((t) => t.id === 't1')!.status).toBe('working');
    });

    it('appends at the end of a section when dropped on its header', () => {
      const next = moveTask(board, 'd1', { id: null, status: 'todo', position: 'after' });
      expect(ids(next)).toBe('w1,w2,t1,t2,d1');
      expect(next.find((t) => t.id === 'd1')!.status).toBe('todo');
    });

    it('places the task correctly when the target section is empty', () => {
      const twoTodos = [task('t1', 'todo'), task('t2', 'todo')];
      const next = moveTask(twoTodos, 't2', { id: null, status: 'working', position: 'after' });
      expect(ids(next)).toBe('t2,t1');
      expect(next[0].status).toBe('working');
    });

    it('is a no-op without a drop target or for an unknown id', () => {
      expect(moveTask(board, 't1', { id: null, status: null, position: 'after' })).toBe(board);
      expect(moveTask(board, 'ghost', { id: 't1', status: 'todo', position: 'after' })).toBe(board);
    });
  });

  describe('parseTasks', () => {
    it('reads a well-formed board and fills in optional fields', () => {
      const parsed = parseTasks(
        '[{"id":"a","title":"写手册","status":"working","createdAt":7,"description":"x","height":120}]',
      );
      expect(parsed).toEqual([
        { id: 'a', title: '写手册', status: 'working', createdAt: 7, description: 'x', height: 120 },
      ]);
    });

    it('accepts an empty board', () => {
      expect(parseTasks('[]')).toEqual([]);
    });

    it('falls back to 待办 for an unknown status', () => {
      expect(parseTasks('[{"id":"a","title":"t","status":"archived"}]')![0].status).toBe('todo');
    });

    it('returns null for anything that is not a task array', () => {
      expect(parseTasks('')).toBeNull();
      expect(parseTasks('{ truncated')).toBeNull();
      expect(parseTasks('null')).toBeNull();
      expect(parseTasks('{"tasks":[]}')).toBeNull();
      expect(parseTasks('[1,2]')).toBeNull();
      expect(parseTasks('[{"title":"no id"}]')).toBeNull();
      expect(parseTasks('[{"id":"","title":"blank id"}]')).toBeNull();
      expect(parseTasks('[{"id":"a"}]')).toBeNull();
    });
  });
});
