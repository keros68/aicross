// agent 与 provider → 图标资产名（fix-49-r2）。资产在 src/icons-inline/，由
// components/common/AgentIcon.tsx 按这里的名字取；取不到时界面退回厂商色首字母。

export type AgentIconKey =
  | 'claude'
  | 'codex'
  | 'kimicode'
  | 'pi'
  | 'qwen'
  | 'antigravity'
  | 'opencode'
  | 'grok'
  | 'mimocode';

const BY_AGENT: Record<string, AgentIconKey> = {
  claude: 'claude',
  codex: 'codex',
  kimi: 'kimicode',
  kimicode: 'kimicode',
  pi: 'pi',
  qwen: 'qwen',
  antigravity: 'antigravity',
  opencode: 'opencode',
  grok: 'grok',
  mimocode: 'mimocode',
};

function slug(value: string): string {
  return value.trim().toLowerCase().replace(/[^a-z0-9]/g, '');
}

/** agent id → 图标名；没有收录的为 null。 */
export function agentIconKey(agentId: string): AgentIconKey | null {
  return BY_AGENT[slug(agentId)] ?? null;
}

/** pi 下 provider 小组标题的图标：opencode 系 → opencode，qwen 系 → qwen；
 *  其余（含原生、空串）为 null。 */
export function providerIconKey(provider: string): AgentIconKey | null {
  const value = slug(provider);
  if (value === '') return null;
  if (value.startsWith('opencode')) return 'opencode';
  if (value.startsWith('qwen')) return 'qwen';
  return null;
}
