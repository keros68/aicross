import { describe, expect, it } from 'vitest';
import { agentIconKey, providerIconKey } from '../agent-icon-key';

describe('agent 图标（fix-49-r2）', () => {
  it('收录的 agent 按 id 取图标，kimi 与 kimicode 同一个', () => {
    expect(agentIconKey('claude')).toBe('claude');
    expect(agentIconKey('Codex')).toBe('codex');
    expect(agentIconKey('kimi')).toBe('kimicode');
    expect(agentIconKey('kimicode')).toBe('kimicode');
    expect(agentIconKey('pi')).toBe('pi');
    expect(agentIconKey('agy')).toBe('antigravity');
  });

  it('没收录的为 null，界面退回首字母', () => {
    expect(agentIconKey('hermes')).toBeNull();
    for (const id of ['cursor', 'codebuddy', 'copilot', 'qoder']) expect(agentIconKey(id)).toBeNull();
    expect(agentIconKey('')).toBeNull();
  });

  it('provider 小组：opencode 系与 qwen 系能对上，其余不配', () => {
    expect(providerIconKey('opencode-go')).toBe('opencode');
    expect(providerIconKey('qwen-token-plan-cn')).toBe('qwen');
    expect(providerIconKey('zai')).toBeNull();
    expect(providerIconKey('')).toBeNull();
  });
});
