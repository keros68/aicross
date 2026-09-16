// TASK-19：界面缩放档位、快捷键、持久化与 WebView/CSS 应用路径。

import { describe, expect, it, vi } from 'vitest';
// @ts-expect-error node builtin, only source assertions read local files
import { readFileSync } from 'node:fs';
import {
  UI_SCALE_LEVELS,
  applyUiScale,
  parseUiScale,
  persistUiScale,
  readUiScale,
  serializeUiScale,
  stepUiScale,
  uiScaleForShortcut,
} from '../ui-scale';

describe('TASK-19 界面缩放', () => {
  it('只使用五个档位并把越界或非档位值夹到最近档', () => {
    expect(UI_SCALE_LEVELS).toEqual([90, 100, 110, 125, 150]);
    expect(parseUiScale('80')).toBe(90);
    expect(parseUiScale('117')).toBe(110);
    expect(parseUiScale('140')).toBe(150);
    expect(parseUiScale('200')).toBe(150);
    expect(parseUiScale('bad')).toBe(100);
    expect(parseUiScale(null)).toBe(100);
  });

  it('步进在边界停住，序列化与解析可以往返', () => {
    expect(stepUiScale(90, -1)).toBe(90);
    expect(stepUiScale(100, 1)).toBe(110);
    expect(stepUiScale(125, 1)).toBe(150);
    expect(stepUiScale(150, 1)).toBe(150);
    for (const level of UI_SCALE_LEVELS) {
      expect(parseUiScale(serializeUiScale(level))).toBe(level);
    }
    let stored: string | null = null;
    const storage = {
      getItem: () => stored,
      setItem: (_key: string, value: string) => { stored = value; },
    };
    persistUiScale(125, storage);
    expect(readUiScale(storage)).toBe(125);
  });

  it('Ctrl 加减与归零走同一档位函数', () => {
    expect(uiScaleForShortcut(100, { ctrlKey: true, key: '=' })).toBe(110);
    expect(uiScaleForShortcut(125, { ctrlKey: true, key: '-' })).toBe(110);
    expect(uiScaleForShortcut(150, { ctrlKey: true, key: '0' })).toBe(100);
    expect(uiScaleForShortcut(100, { ctrlKey: false, key: '=' })).toBeNull();
  });

  it('优先调用 WebView setZoom，失败时回退根元素 CSS zoom', async () => {
    const root = { style: { zoom: '1.25' } };
    const setZoom = vi.fn(async () => {});
    await expect(applyUiScale(110, setZoom, root)).resolves.toBe('webview');
    expect(setZoom).toHaveBeenCalledWith(1.1);
    expect(root.style.zoom).toBe('');

    const fallbackRoot = { style: { zoom: '' } };
    await expect(applyUiScale(125, async () => { throw new Error('unavailable'); }, fallbackRoot)).resolves.toBe('css');
    expect(fallbackRoot.style.zoom).toBe('1.25');
  });

  it('设置页、启动应用与 Tauri 权限接到同一缩放模块', () => {
    const settings = readFileSync(new URL('../../components/common/SettingsModal.tsx', import.meta.url), 'utf8');
    const app = readFileSync(new URL('../../App.tsx', import.meta.url), 'utf8');
    const capability = readFileSync(new URL('../../../../capabilities/default.json', import.meta.url), 'utf8');
    expect(settings).toContain('UI_SCALE_LEVELS.map');
    expect(settings).toContain('updateUiScale(level)');
    expect(app).toContain('initializeUiScale()');
    expect(app).toContain('uiScaleForShortcut(readUiScale(), event)');
    expect(capability).toContain('core:webview:allow-set-webview-zoom');
  });
});
