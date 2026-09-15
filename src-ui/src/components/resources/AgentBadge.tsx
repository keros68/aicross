// agent 图标徽标（TASK-09；TASK-09b 换成品牌标志）。
//
// 09-15 用户反馈：字母徽标不如官方标志好认。标志路径数据来自 simple-icons
// （CC0-1.0），来源与许可写在 `brand-marks.ts` 的文件头；SVG 内联，不引任何
// 图标运行时依赖。没有收录标志的 agent 退回字形（pi → π）或字母。
//
// 色相仍从 `graph/vendor.ts` 取——同一个 agent 在对话图上的色点与这里的徽标
// 必须同色。深浅主题各一套：深色主题用 vendor 原色（内联 --agent-tone），
// 浅色主题在 resources.css 里按 `.agent-badge-<slug>` 压暗，小图形在白底上
// 才够 3:1（WCAG 非文字对比度）。

import type { CSSProperties } from 'react';
import { vendorColor } from '../../graph/vendor';
import { agentMark } from '../../resources/types';
import { agentEmblem } from './brand-marks';

export function AgentBadge({ agentId, size = 'sm' }: { agentId: string; size?: 'sm' | 'md' }) {
  const emblem = agentEmblem(agentId, agentMark(agentId));
  const className = `agent-badge agent-badge-${size} agent-badge-${emblem.slug} agent-badge-${emblem.kind}`;
  const style = { '--agent-tone': vendorColor(agentId) } as CSSProperties;
  if (emblem.kind === 'mark') {
    return (
      <span className={className} style={style} aria-hidden="true">
        <svg viewBox={emblem.mark.viewBox} fill="currentColor" focusable="false" role="presentation">
          <path d={emblem.mark.path} />
        </svg>
      </span>
    );
  }
  return (
    <span className={className} style={style} aria-hidden="true">
      {emblem.kind === 'glyph' ? emblem.glyph : emblem.letters}
    </span>
  );
}
