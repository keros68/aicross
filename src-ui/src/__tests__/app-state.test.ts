import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getInitialState, reducer } from '../store/app-state';

class MemoryStorage implements Storage {
  private values = new Map<string, string>();
  writes: string[] = [];

  get length() { return this.values.size; }
  clear() { this.values.clear(); }
  getItem(key: string) { return this.values.get(key) ?? null; }
  key(index: number) { return [...this.values.keys()][index] ?? null; }
  removeItem(key: string) { this.values.delete(key); }
  setItem(key: string, value: string) {
    this.writes.push(key);
    this.values.set(key, String(value));
  }
}

describe('getInitialState personalization migration', () => {
  let storage: MemoryStorage;

  beforeEach(() => {
    storage = new MemoryStorage();
    vi.stubGlobal('localStorage', storage);
    vi.stubGlobal('crypto', { randomUUID: () => 'test-session-id' });
  });

  it('migrates a retired theme to dark and removes legacy background values', () => {
    storage.setItem('cc-theme', 'sakura');
    storage.setItem('cc-bg-path', 'C:\\wallpaper.png');
    storage.setItem('cc-bg-type', 'image');
    storage.writes = [];

    const state = getInitialState();

    expect(state.currentTheme).toBe('dark');
    expect(storage.getItem('cc-theme')).toBe('dark');
    expect(state.bgPath).toBe('');
    expect(state.bgType).toBe('none');
    expect(storage.getItem('cc-bg-path')).toBeNull();
    expect(storage.getItem('cc-bg-type')).toBeNull();
  });

  it('preserves light and does not rewrite matching shape or icon values', () => {
    storage.setItem('cc-theme', 'light');
    storage.setItem('cc-shape', 'soft');
    storage.setItem('cc-icon-theme', 'outline');
    storage.writes = [];

    const state = getInitialState();

    expect(state.currentTheme).toBe('light');
    expect(storage.writes).not.toContain('cc-theme');
    expect(storage.writes).not.toContain('cc-shape');
    expect(storage.writes).not.toContain('cc-icon-theme');
  });

  it('uses dark, soft, and outline when personalization is missing', () => {
    const state = getInitialState();

    expect(state.currentTheme).toBe('dark');
    expect(state.currentShape).toBe('soft');
    expect(state.iconTheme).toBe('outline');
    expect(storage.getItem('cc-shape')).toBe('soft');
    expect(storage.getItem('cc-icon-theme')).toBe('outline');
  });

  it('关闭图 tab 时清除指向该图的 graphFocus', () => {
    const state = getInitialState();
    const focused = { ...state, terminals: [{ ...state.terminals[0], toolData: 'graph-1' }], graphFocus: { graphId: 'graph-1', nodeId: 'n1' } };
    const next = reducer(focused, { type: 'CLOSE_TERMINAL', id: focused.terminals[0].id });
    expect(next.graphFocus).toBeNull();
  });

  it('R5-5 新开 tab 继承活动 tab 工作目录', () => {
    const state = getInitialState();
    const current = { ...state.terminals[0], folderPath: 'D:\\project' };
    const next = reducer({ ...state, terminals: [current], activeTerminalId: current.id }, { type: 'ADD_TERMINAL', session: { id: 'new', tool: 'graph', folderPath: null } });
    expect(next.terminals.at(-1)?.folderPath).toBe('D:\\project');
  });

  it('R5-6 切到没有当前图的 tab 清除 graphFocus', () => {
    const state = getInitialState();
    const terminals = [{ ...state.terminals[0], id: 'g', tool: 'graph' as const, toolData: 'graph-1' }, { id: 'plain', tool: null, folderPath: null }];
    const next = reducer({ ...state, terminals, activeTerminalId: 'g', graphFocus: { graphId: 'graph-1', nodeId: 'n1' } }, { type: 'SET_ACTIVE_TERMINAL', id: 'plain' });
    expect(next.graphFocus).toBeNull();
  });

  it('TASK-10-R1 保存后消费 commanderSettings，随后重启不恢复旧快照', () => {
    const state = getInitialState();
    const current = {
      ...state.terminals[0],
      id: 'commander-tab',
      tool: 'claude' as const,
      commanderSettings: {
        version: 1 as const,
        agent: 'claude',
        provider: '',
        model: 'fable',
        cwd: 'D:\\old',
        updated_at: '2026-09-15T10:00:00Z',
      },
    };
    const consumed = reducer(
      { ...state, terminals: [current], activeTerminalId: current.id },
      { type: 'CONSUME_COMMANDER_SETTINGS', id: current.id },
    );
    expect(consumed.terminals[0].commanderSettings).toBeUndefined();

    const restarted = reducer(consumed, {
      type: 'RESTART_TERMINAL',
      id: current.id,
      newId: 'restarted-tab',
    });
    expect(restarted.terminals[0].id).toBe('restarted-tab');
    expect(restarted.terminals[0].commanderSettings).toBeUndefined();
  });

  it('TASK-15-R1 换目录重启后保留 agent/model 快照，目录读当前 tab', () => {
    const state = getInitialState();
    const current = {
      ...state.terminals[0],
      id: 'commander-tab',
      tool: 'pi' as const,
      folderPath: 'D:\\old',
      commanderProfile: { agent: 'pi', provider: 'bigmodel', model: 'glm' },
    };
    const moved = reducer(
      { ...state, terminals: [current], activeTerminalId: current.id },
      { type: 'SET_FOLDER', path: 'D:\\new' },
    );
    const restarted = reducer(moved, {
      type: 'RESTART_TERMINAL', id: current.id, newId: 'restarted-tab',
    });
    expect(restarted.terminals[0]).toMatchObject({
      id: 'restarted-tab',
      folderPath: 'D:\\new',
      commanderProfile: { agent: 'pi', provider: 'bigmodel', model: 'glm' },
    });
  });
});
