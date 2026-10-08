import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

// tauri.ts 在加载时取 invoke，所以先放好假的窗口对象再动态导入。
const invoke = vi.fn((cmd: string, args?: unknown) => Promise.resolve({ cmd, args }));
let commands: typeof import('../tauri').commands;

beforeAll(async () => {
  vi.stubGlobal('window', { __TAURI_INTERNALS__: { invoke } });
  ({ commands } = await import('../tauri'));
});

afterAll(() => {
  vi.unstubAllGlobals();
});

describe('自定义模式命令的参数（fix-fullcheck C4）', () => {
  const preset = { name: '我的模式', stages: [{ nodes: [{ role: '执行', visibility: '共享', write: '单写者' }] }] };

  it('保存带上当前主 agent', async () => {
    invoke.mockClear();
    await commands.presetSave('codex', preset);
    expect(invoke).toHaveBeenCalledWith('preset_save', { host: 'codex', preset });
  });

  it('删除只传 id：引擎按 id 清理各主 agent 下的引用', async () => {
    invoke.mockClear();
    await commands.presetRemove('custom-1');
    expect(invoke).toHaveBeenCalledWith('preset_remove', { id: 'custom-1' });
  });
});
