import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_DISPATCH_SETTINGS, loadDispatchSettings, saveDispatchSettings } from '../graph/dispatch-settings';

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
});
