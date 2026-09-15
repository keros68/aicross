// agent 图标徽标。内置四家复用 Coffee CLI fork 已有图标，未收录的 agent 退回字母。

import type { CSSProperties } from 'react';
import { vendorColor } from '../../graph/vendor';
import { agentMark } from '../../resources/types';
import { agentEmblem } from './brand-marks';

export function AgentBadge({ agentId, size = 'sm' }: { agentId: string; size?: 'sm' | 'md' | 'lg' }) {
  const emblem = agentEmblem(agentId, agentMark(agentId));
  const className = `agent-badge agent-badge-${size} agent-badge-${emblem.slug} agent-badge-${emblem.kind}`;
  if (emblem.kind === 'image') {
    return (
      <span className={className} aria-hidden="true">
        <img src={emblem.src} alt="" />
      </span>
    );
  }
  if (emblem.kind === 'inline-svg') {
    return (
      <span
        className={className}
        aria-hidden="true"
        dangerouslySetInnerHTML={{ __html: emblem.markup }}
      />
    );
  }
  return (
    <span
      className={className}
      style={{ '--agent-tone': vendorColor(agentId) } as CSSProperties}
      aria-hidden="true"
    >
      {emblem.letters}
    </span>
  );
}
