import { afterEach, describe, expect, it, vi } from 'vitest';
import { DARK_QUERY, followSystemTheme, themeFromMedia } from '../theme';

describe('配色跟随系统', () => {
  it('系统为深色时用 dark，浅色时用 light', () => {
    expect(themeFromMedia({ matches: true })).toBe('dark');
    expect(themeFromMedia({ matches: false })).toBe('light');
  });

  it('拿不到 matchMedia 结果时用深色', () => {
    expect(themeFromMedia(null)).toBe('dark');
    expect(themeFromMedia(undefined)).toBe('dark');
  });
});

describe('followSystemTheme（fix-49-r4）', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  function fakeMedia(matches: boolean) {
    const listeners = new Set<(event: { matches: boolean }) => void>();
    const media = {
      matches,
      media: DARK_QUERY,
      addEventListener: vi.fn((_type: string, fn: (event: { matches: boolean }) => void) => { listeners.add(fn); }),
      removeEventListener: vi.fn((_type: string, fn: (event: { matches: boolean }) => void) => { listeners.delete(fn); }),
      fire(next: boolean) { listeners.forEach((fn) => fn({ matches: next })); },
      listeners,
    };
    return media;
  }

  function fakeRoot() {
    const attrs: Record<string, string> = {};
    return {
      attrs,
      setAttribute: (name: string, value: string) => { attrs[name] = value; },
    };
  }

  it('初始按系统配色赋值，系统切换时跟着变，注销后不再响应', () => {
    const media = fakeMedia(false);
    const matchMedia = vi.fn(() => media);
    vi.stubGlobal('window', { matchMedia });
    const root = fakeRoot();

    const stop = followSystemTheme(root as unknown as HTMLElement);
    expect(matchMedia).toHaveBeenCalledWith(DARK_QUERY);
    expect(root.attrs['data-theme']).toBe('light');

    media.fire(true);
    expect(root.attrs['data-theme']).toBe('dark');
    media.fire(false);
    expect(root.attrs['data-theme']).toBe('light');

    stop();
    expect(media.removeEventListener).toHaveBeenCalledTimes(1);
    expect(media.listeners.size).toBe(0);
    media.fire(true);
    expect(root.attrs['data-theme']).toBe('light');
  });

  it('没有 matchMedia 时设为深色，注销函数可安全调用', () => {
    vi.stubGlobal('window', {});
    const root = fakeRoot();
    const stop = followSystemTheme(root as unknown as HTMLElement);
    expect(root.attrs['data-theme']).toBe('dark');
    expect(() => stop()).not.toThrow();
  });
});
