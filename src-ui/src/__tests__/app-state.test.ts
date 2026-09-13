import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getInitialState } from '../store/app-state';

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
});
