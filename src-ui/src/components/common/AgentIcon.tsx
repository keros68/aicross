// AgentIcon.tsx — agent 图标（fix-49-r2）。资产沿用 TASK-40 之前窗口里的同一套
// （src/icons-inline/）：用 currentColor 的 SVG（pi、grok）内联渲染以跟随文字色，
// 其余 SVG 与 PNG 作为图片放在底板上渲染（多次渲染时不会出现重复的 SVG id）。没有收录的
// agent 退回厂商色首字母。

import type { CSSProperties } from 'react';
import { agentIconKey, providerIconKey, type AgentIconKey } from '../../lib/agent-icon-key';
import { vendorColor } from '../../viewer/vendor';
import { initialOf } from '../../config/config-view';
import CLAUDE_URL from '../../icons-inline/claude.svg';
import CODEX_URL from '../../icons-inline/codex.svg';
import QWEN_URL from '../../icons-inline/qwen.svg';
import ANTIGRAVITY_URL from '../../icons-inline/antigravity.svg';
import MIMOCODE_URL from '../../icons-inline/mimocode.svg';
import KIMICODE_URL from '../../icons-inline/kimicode.png?inline';
import OPENCODE_URL from '../../icons-inline/opencode.png?inline';
import PI_SVG from '../../icons-inline/pi.svg?raw';
import GROK_SVG from '../../icons-inline/grok.svg?raw';

type Asset = { kind: 'image'; src: string } | { kind: 'inline-svg'; markup: string };

const ASSETS: Record<AgentIconKey, Asset> = {
  claude: { kind: 'image', src: CLAUDE_URL },
  codex: { kind: 'image', src: CODEX_URL },
  qwen: { kind: 'image', src: QWEN_URL },
  antigravity: { kind: 'image', src: ANTIGRAVITY_URL },
  mimocode: { kind: 'image', src: MIMOCODE_URL },
  kimicode: { kind: 'image', src: KIMICODE_URL },
  opencode: { kind: 'image', src: OPENCODE_URL },
  pi: { kind: 'inline-svg', markup: PI_SVG },
  grok: { kind: 'inline-svg', markup: GROK_SVG },
};

function IconAsset({ iconKey, size }: { iconKey: AgentIconKey; size: number }) {
  const asset = ASSETS[iconKey];
  const style = { width: size, height: size } as CSSProperties;
  if (asset.kind === 'image') {
    // 图片图标放在有底色的圆角底板上（--icon-plate），深色图标在深色主题下也可辨。
    return (
      <span className="agent-icon agent-icon-plate" style={style} aria-hidden="true">
        <img src={asset.src} alt="" draggable={false} />
      </span>
    );
  }
  return (
    <span
      className="agent-icon"
      style={style}
      aria-hidden="true"
      dangerouslySetInnerHTML={{ __html: asset.markup }}
    />
  );
}

/** agent 图标；没有收录时显示厂商色首字母（字号随 size）。 */
export function AgentIcon({ agent, size = 16 }: { agent: string; size?: number }) {
  const key = agentIconKey(agent);
  if (key) return <IconAsset iconKey={key} size={size} />;
  return (
    <span
      className="agent-icon agent-icon-letter"
      style={{ width: size, height: size, fontSize: Math.round(size * 0.75), color: vendorColor(agent) }}
      aria-hidden="true"
    >
      {initialOf(agent)}
    </span>
  );
}

/** provider 小组标题的图标；对不上的不显示。 */
export function ProviderIcon({ provider, size = 14 }: { provider: string; size?: number }) {
  const key = providerIconKey(provider);
  return key ? <IconAsset iconKey={key} size={size} /> : null;
}
