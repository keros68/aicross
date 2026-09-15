// 内置 agent 图标只沿用 fork 中 Coffee CLI 已有资产，不再内嵌第三方路径数据。
// Claude Code / Codex 使用 public 目录的 SVG；Kimi Code 与 pi 复用上游已有的
// icons-inline 资产。pi 的 SVG 依赖 currentColor，因此内联渲染。

import KIMICODE_DATA_URL from '../../icons-inline/kimicode.png?inline';
import PI_SVG from '../../icons-inline/pi.svg?raw';

export type AgentIconAsset =
  | { kind: 'image'; src: string }
  | { kind: 'inline-svg'; markup: string };

const kimiAsset: AgentIconAsset = { kind: 'image', src: KIMICODE_DATA_URL };

export const AGENT_ICON_ASSETS: Record<string, AgentIconAsset> = {
  claude: { kind: 'image', src: '/icons/tools/claude.svg' },
  codex: { kind: 'image', src: '/icons/tools/codex.svg' },
  kimi: kimiAsset,
  kimicode: kimiAsset,
  pi: { kind: 'inline-svg', markup: PI_SVG },
};

export const CSS_TONED_SLUGS: readonly string[] = ['claude', 'codex', 'kimi', 'pi'];

export function hasCssTone(slug: string): boolean {
  return CSS_TONED_SLUGS.includes(slug);
}

export function agentSlug(agentId: string): string {
  const slug = agentId.trim().toLowerCase().replace(/[^a-z0-9]/g, '') || 'unknown';
  return slug === 'kimicode' ? 'kimi' : slug;
}

export type AgentEmblem =
  | ({ slug: string } & AgentIconAsset)
  | { kind: 'letters'; slug: string; letters: string };

export function agentEmblem(agentId: string, letters: string): AgentEmblem {
  const raw = agentId.trim().toLowerCase().replace(/[^a-z0-9]/g, '') || 'unknown';
  const slug = agentSlug(agentId);
  const asset = AGENT_ICON_ASSETS[raw];
  return asset ? { slug, ...asset } : { kind: 'letters', slug, letters };
}
