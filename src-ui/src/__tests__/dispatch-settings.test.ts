import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_DISPATCH_SETTINGS, fixRoundLimitFromInput, loadDispatchSettings, saveDispatchSettings } from '../graph/dispatch-settings';

describe('ACP dispatch settings', () => {
  const values = new Map<string, string>();

  beforeEach(() => {
    values.clear();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => values.set(key, value),
    });
    vi.stubGlobal('window', { dispatchEvent: vi.fn() });
  });

  it('旧设置缺少 ACP 字段时保持一次性运行', () => {
    values.set('aicross-dispatch-settings', JSON.stringify({ timeoutSecs: 20 }));
    expect(loadDispatchSettings().acpEnabled).toBe(false);
  });

  it('Kimi 长驻开关可保存并恢复', () => {
    saveDispatchSettings({ ...DEFAULT_DISPATCH_SETTINGS, acpEnabled: true });
    expect(loadDispatchSettings().acpEnabled).toBe(true);
  });

  // TASK-12：修正轮上限默认 1，旧设置缺字段按 1，越界夹到 0–9
  it('修正轮上限默认 1，可保存并恢复，越界夹紧', () => {
    expect(DEFAULT_DISPATCH_SETTINGS.fixRoundLimit).toBe(1);
    values.set('aicross-dispatch-settings', JSON.stringify({ timeoutSecs: 20 }));
    expect(loadDispatchSettings().fixRoundLimit).toBe(1);
    saveDispatchSettings({ ...DEFAULT_DISPATCH_SETTINGS, fixRoundLimit: 3 });
    expect(loadDispatchSettings().fixRoundLimit).toBe(3);
    values.set('aicross-dispatch-settings', JSON.stringify({ fixRoundLimit: 42 }));
    expect(loadDispatchSettings().fixRoundLimit).toBe(9);
    values.set('aicross-dispatch-settings', JSON.stringify({ fixRoundLimit: -2 }));
    expect(loadDispatchSettings().fixRoundLimit).toBe(0);
    values.set('aicross-dispatch-settings', JSON.stringify({ fixRoundLimit: 'x' }));
    expect(loadDispatchSettings().fixRoundLimit).toBe(1);
  });

  it('修正轮上限输入清空时回到默认 1，显式 0 保持 0', () => {
    expect(fixRoundLimitFromInput('')).toBe(1);
    expect(fixRoundLimitFromInput('   ')).toBe(1);
    expect(fixRoundLimitFromInput('0')).toBe(0);
  });
});
