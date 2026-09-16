// 内置 agent 的用户可见名称。资源页、控制台、启动卡和工具设置都从这里取；
// Rust `src/resources.rs` 的 NATIVE_AGENTS / PI_AGENT 与这张表保持一致。
// 机器 id（claude / codex / kimi / kimicode / pi）不是显示名，不能直接画到界面。

export const AGENT_DISPLAY_NAMES = {
  claude: 'Claude Code',
  codex: 'Codex',
  kimi: 'Kimi Code',
  pi: 'pi agent',
} as const;

const DISPLAY_NAME_ALIASES: Record<string, keyof typeof AGENT_DISPLAY_NAMES> = {
  kimicode: 'kimi',
};

/** 内置 agent / 工具 id → 用户可见名称；未知项沿用调用方给的名称。 */
export function agentDisplayName(agentId: string, fallback?: string): string {
  const id = agentId.trim().toLowerCase();
  const canonical = DISPLAY_NAME_ALIASES[id] ?? id;
  return AGENT_DISPLAY_NAMES[canonical as keyof typeof AGENT_DISPLAY_NAMES] ?? fallback ?? agentId;
}

/** 用户可见的 agent/model 坐标；机器 id 仍保留在数据与裁决上下文中。 */
export function agentModelDisplay(agentId: string, model: string, provider = ''): string {
  const agent = agentDisplayName(agentId, agentId);
  const route = provider === '' ? model : `${provider}/${model}`;
  return `${agent}/${route}`;
}
