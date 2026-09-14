import { describe, expect, it, vi } from 'vitest';
import { loadTaskBoard, type TaskStorageIo } from '../task-storage';

function io(over: Partial<TaskStorageIo> = {}): TaskStorageIo {
  return {
    isTauri: true,
    loadTasks: () => Promise.resolve('[]'),
    saveTasks: () => Promise.resolve(),
    readLegacy: () => null,
    clearLegacy: () => {},
    ...over,
  };
}

const ONE = '[{"id":"a","title":"写手册","status":"todo","createdAt":1}]';

describe('loadTaskBoard', () => {
  it('reads the board the Rust side returns', async () => {
    const result = await loadTaskBoard(io({ loadTasks: () => Promise.resolve(ONE) }));
    expect(result).toEqual({ kind: 'ok', tasks: [{ id: 'a', title: '写手册', status: 'todo', createdAt: 1, description: undefined, height: undefined }] });
  });

  it('treats an empty file as an empty, writable board', async () => {
    expect(await loadTaskBoard(io())).toEqual({ kind: 'ok', tasks: [] });
  });

  // The regression this module exists for: a rejected load used to fall
  // through to localStorage and hand back a writable empty board, whose first
  // change then wrote `[]` over the real file.
  it('stops writing instead of falling back when Rust refuses the read', async () => {
    const readLegacy = vi.fn(() => ONE);
    const saveTasks = vi.fn(() => Promise.resolve());
    const result = await loadTaskBoard(
      io({
        loadTasks: () => Promise.reject(new Error('Failed to read tasks: access denied')),
        readLegacy,
        saveTasks,
      }),
    );

    expect(result).toEqual({ kind: 'damaged', reason: 'unreadable' });
    expect(readLegacy).not.toHaveBeenCalled();
    expect(saveTasks).not.toHaveBeenCalled();
  });

  it('stops writing when the file is there but unparsable', async () => {
    const saveTasks = vi.fn(() => Promise.resolve());
    const result = await loadTaskBoard(io({ loadTasks: () => Promise.resolve('{ truncated'), saveTasks }));
    expect(result).toEqual({ kind: 'damaged', reason: 'unparsable' });
    expect(saveTasks).not.toHaveBeenCalled();
  });

  it('migrates legacy localStorage tasks into the empty Rust file once', async () => {
    const saveTasks = vi.fn(() => Promise.resolve());
    const clearLegacy = vi.fn();
    const result = await loadTaskBoard(io({ readLegacy: () => ONE, saveTasks, clearLegacy }));

    expect(result.kind).toBe('ok');
    expect(result.kind === 'ok' && result.tasks).toHaveLength(1);
    expect(saveTasks).toHaveBeenCalledOnce();
    expect(clearLegacy).toHaveBeenCalledOnce();
  });

  it('survives a failed migration with an empty board rather than a damage flag', async () => {
    const result = await loadTaskBoard(
      io({ readLegacy: () => ONE, saveTasks: () => Promise.reject(new Error('disk full')) }),
    );
    expect(result).toEqual({ kind: 'ok', tasks: [] });
  });

  it('uses localStorage only outside the app', async () => {
    const loadTasks = vi.fn(() => Promise.resolve(ONE));
    const result = await loadTaskBoard(io({ isTauri: false, loadTasks, readLegacy: () => ONE }));
    expect(loadTasks).not.toHaveBeenCalled();
    expect(result.kind === 'ok' && result.tasks).toHaveLength(1);
  });

  it('outside the app, a broken localStorage value is just an empty board', async () => {
    const result = await loadTaskBoard(io({ isTauri: false, readLegacy: () => '{ truncated' }));
    expect(result).toEqual({ kind: 'ok', tasks: [] });
  });
});
