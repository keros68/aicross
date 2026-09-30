// TASK-19：界面缩放档位、快捷键、持久化与 WebView/CSS 应用路径。

import { describe, expect, it, vi } from 'vitest';
// @ts-expect-error node builtin, only source assertions read local files
import { readFileSync } from 'node:fs';
import {
  UI_SCALE_LEVELS,
  applyUiScale,
  createUiScaleShortcutController,
  initializeUiScale,
  parseUiScale,
  persistUiScale,
  readUiScale,
  serializeUiScale,
  stepUiScale,
  uiScaleForShortcut,
  updateUiScale,
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

  it('成功、回退、再成功时始终只有一处倍率生效', async () => {
    const root = { style: { zoom: '' } };
    let webviewFactor = 1;
    const setZoom = vi.fn(async (factor: number) => {
      if (factor === 1.25) throw new Error('temporary failure');
      webviewFactor = factor;
    });

    await expect(applyUiScale(150, setZoom, root)).resolves.toBe('webview');
    await expect(applyUiScale(125, setZoom, root)).resolves.toBe('css');
    expect(webviewFactor).toBe(1);
    expect(root.style.zoom).toBe('1.25');

    await expect(applyUiScale(110, setZoom, root)).resolves.toBe('webview');
    expect(webviewFactor).toBe(1.1);
    expect(root.style.zoom).toBe('');
    expect(setZoom.mock.calls.map(([factor]) => factor)).toEqual([1.5, 1.25, 1, 1.1]);
  });

  it('updateUiScale 持久化、应用并发送带 detail 的事件', () => {
    const storage = { setItem: vi.fn() };
    const apply = vi.fn();
    const dispatched: Event[] = [];
    const eventTarget = { dispatchEvent: vi.fn((event: Event) => { dispatched.push(event); return true; }) };

    updateUiScale(125, { storage, apply, eventTarget });

    expect(storage.setItem).toHaveBeenCalledWith('aicross-ui-scale', '125');
    expect(apply).toHaveBeenCalledWith(125);
    const event = dispatched[0] as CustomEvent<number>;
    expect(event.type).toBe('aicross:ui-scale-changed');
    expect(event.detail).toBe(125);
  });

  it('initializeUiScale 从注入存储读取并调用应用函数', () => {
    const storage = { getItem: vi.fn(() => '110') };
    const apply = vi.fn();

    expect(initializeUiScale({ storage, apply })).toBe(110);
    expect(storage.getItem).toHaveBeenCalledWith('aicross-ui-scale');
    expect(apply).toHaveBeenCalledWith(110);
  });

  it('快捷键用内存当前档递进，存储不可用不影响连续步进', () => {
    const update = vi.fn();
    const controller = createUiScaleShortcutController(100, update);
    const shortcut = () => ({
      key: '=', ctrlKey: true, metaKey: false, altKey: false, repeat: false,
      preventDefault: vi.fn(), stopPropagation: vi.fn(),
    });

    controller.handleKeyDown(shortcut());
    controller.handleKeyDown(shortcut());
    expect(update.mock.calls.map(([scale]) => scale)).toEqual([110, 125]);

    controller.sync(150);
    controller.handleKeyDown({ ...shortcut(), key: '-' });
    expect(update).toHaveBeenLastCalledWith(125);
  });

  it('首帧前初始化缩放，WebView 原生缩放热键保持关闭', () => {
    const settings = readFileSync(new URL('../../components/common/SettingsModal.tsx', import.meta.url), 'utf8');
    const app = readFileSync(new URL('../../App.tsx', import.meta.url), 'utf8');
    const capability = readFileSync(new URL('../../../../capabilities/default.json', import.meta.url), 'utf8');
    const tauriConfig = JSON.parse(readFileSync(new URL('../../../../tauri.conf.json', import.meta.url), 'utf8'));
    expect(settings).toContain('UI_SCALE_LEVELS.map');
    expect(app).toMatch(/useLayoutEffect\(\(\) => \{\s*const initialScale = initializeUiScale\(\)/);
    expect(capability).toContain('core:webview:allow-set-webview-zoom');
    expect(tauriConfig.app.windows[0].zoomHotkeysEnabled).toBe(false);
  });

  it('非终端与非代码块界面不再保留 11px 辅助文字', () => {
    const paths = [
      '../../components/common/SettingsModal.css',
      '../../components/center/CenterPanel.css',
      '../../components/center/ContributionHeatmap.css',
      '../../components/center/Gambit.css',
      '../../styles/global.css',
    ];
    for (const path of paths) {
      const css = readFileSync(new URL(path, import.meta.url), 'utf8');
      expect(css, path).not.toMatch(/font-size:\s*11px/);
    }
  });

  it('快捷键文案、术语表、定案编号与 Explorer 字号遗留记录一致', () => {
    const settings = readFileSync(new URL('../../components/common/SettingsModal.tsx', import.meta.url), 'utf8');
    const manual = readFileSync(new URL('../../../../docs/MANUAL.md', import.meta.url), 'utf8');
    const product = readFileSync(new URL('../../../../docs/history/PRODUCT-0830-0930.md', import.meta.url), 'utf8');
    const backlog = readFileSync(new URL('../../../../tasks/fix-04-r5-backlog.md', import.meta.url), 'utf8');
    expect(settings).toContain('Ctrl + = / - / 0');
    expect(manual).toContain('Ctrl + = / - / 0');
    expect(manual).toMatch(/\| 界面整体显示比例 \| 界面缩放 \| 设置页「外观」、本手册 \|/);
    // 切到**下一个二级标题**为止，不要一路切到「## 顺序」：中间再插一节定案
    // （09-16 就插了「控制台就是对话窗」）会把别节的编号也算进来（backlog 62 同型）。
    const start = product.indexOf('## 用户 09-15 晚四点反馈的定案');
    const rest = product.slice(start + 1);
    const nextHeading = rest.search(/^## /m);
    const decisions = nextHeading < 0 ? rest : rest.slice(0, nextHeading);
    expect([...decisions.matchAll(/^(\d+)\. /gm)].map((match) => Number(match[1]))).toEqual([1, 2, 3, 4, 5]);
    expect(backlog).toContain('Explorer.css');
    expect(backlog).toContain('12.5px');
  });

  it('修正报告保留窗口核对项与多窗口缩放限制', () => {
    const report = readFileSync(new URL('../../../../tasks/REPORT-19-R1.md', import.meta.url), 'utf8');
    expect(report).toContain('待窗口核对');
    expect(report).toContain('90% / 100% / 150%');
    expect(report).toContain('多窗口缩放不联动');
    expect(report).toContain('detached');
  });
});

describe('缩放回退跨页面重载也幂等（backlog 61）', () => {
  const fakeStore = (seed: Record<string, string> = {}) => {
    const map = new Map(Object.entries(seed));
    return {
      getItem: (key: string) => map.get(key) ?? null,
      setItem: (key: string, value: string) => { map.set(key, value); },
      dump: () => Object.fromEntries(map),
    };
  };

  it('上一次是 webview 倍率、重载后首个 setZoom 失败：先把 webview 归 1，不叠 CSS', async () => {
    // 重载后内存里的 WeakMap 是空的，只能靠存档知道上次生效的是 webview 倍率。
    const store = fakeStore({ 'aicross.ui-scale.mode': 'webview' });
    const root = { style: { zoom: '' } };
    const calls: number[] = [];
    const setZoom = async (factor: number) => {
      calls.push(factor);
      if (factor !== 1) throw new Error('setZoom 不可用');
    };
    const mode = await applyUiScale(125, setZoom, root as never, store);
    expect(calls).toEqual([1.25, 1]); // 失败后先把 webview 归 1
    expect(mode).toBe('css');
    expect(root.style.zoom).toBe('1.25'); // 只有一处倍率
  });

  it('没有存档（真的第一次用）时不会去调归 1', async () => {
    const store = fakeStore();
    const root = { style: { zoom: '' } };
    const calls: number[] = [];
    const setZoom = async (factor: number) => { calls.push(factor); throw new Error('nope'); };
    const mode = await applyUiScale(125, setZoom, root as never, store);
    expect(calls).toEqual([1.25]);
    expect(mode).toBe('css');
  });

  it('成功走 webview 时把模式记进存档，供下次重载读', async () => {
    const store = fakeStore();
    const root = { style: { zoom: '9' } };
    const mode = await applyUiScale(150, async () => {}, root as never, store);
    expect(mode).toBe('webview');
    expect(root.style.zoom).toBe('');
    expect(store.dump()['aicross.ui-scale.mode']).toBe('webview');
  });

  it('存储不可用时不抛错，退回只靠内存', async () => {
    const broken = {
      getItem: () => { throw new Error('blocked'); },
      setItem: () => { throw new Error('blocked'); },
    };
    const root = { style: { zoom: '' } };
    await expect(applyUiScale(110, async () => {}, root as never, broken)).resolves.toBe('webview');
  });
});
