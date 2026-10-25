import { describe, expect, it } from 'vitest';
import {
  DEFAULT_PAGE_TAB,
  PAGE_TABS,
  PAGE_TAB_STORAGE_KEY,
  keepsMounted,
  readPageTab,
  writePageTab,
} from '../page-tabs';

function memoryStorage(initial: Record<string, string> = {}) {
  const data = { ...initial };
  return {
    data,
    getItem: (key: string) => (key in data ? data[key] : null),
    setItem: (key: string, value: string) => { data[key] = value; },
  };
}

const throwing = {
  getItem: () => { throw new Error('denied'); },
  setItem: () => { throw new Error('denied'); },
};

describe('页签', () => {
  it('依次是开始、资源、skill、模式、派发记录，首次打开进开始', () => {
    expect(PAGE_TABS.map((tab) => tab.label)).toEqual(['开始', '资源', 'skill', '模式', '派发记录']);
    expect(DEFAULT_PAGE_TAB).toBe('start');
  });

  it('写入后读回同一页签', () => {
    const storage = memoryStorage();
    writePageTab(storage, 'runs');
    expect(storage.data[PAGE_TAB_STORAGE_KEY]).toBe('runs');
    expect(readPageTab(storage)).toBe('runs');
    writePageTab(storage, 'modes');
    expect(readPageTab(storage)).toBe('modes');
  });

  it('没存过或值不认时用默认', () => {
    expect(readPageTab(memoryStorage())).toBe('start');
    expect(readPageTab(memoryStorage({ [PAGE_TAB_STORAGE_KEY]: 'terminal' }))).toBe('start');
    expect(readPageTab(null)).toBe('start');
    expect(readPageTab(memoryStorage({ [PAGE_TAB_STORAGE_KEY]: 'resources' }))).toBe('resources');
  });

  it('各页都常驻（派发记录只在可见时轮询）', () => {
    expect(keepsMounted('start')).toBe(true);
    expect(keepsMounted('resources')).toBe(true);
    expect(keepsMounted('modes')).toBe(true);
    expect(keepsMounted('skill')).toBe(true);
    expect(keepsMounted('runs')).toBe(true);
  });

  it('读写抛错时回默认，不向外抛', () => {
    expect(readPageTab(throwing)).toBe('start');
    expect(() => writePageTab(throwing, 'resources')).not.toThrow();
    expect(() => writePageTab(null, 'resources')).not.toThrow();
  });
});
