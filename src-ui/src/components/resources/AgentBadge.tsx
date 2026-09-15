// agent 图标徽标（TASK-09；TASK-09b 换成品牌标志）。
//
// 09-15 用户反馈：字母徽标不如官方标志好认。标志路径数据来自 simple-icons
// （CC0-1.0），来源与许可写在 `brand-marks.ts` 的文件头；SVG 内联，不引任何
// 图标运行时依赖。没有收录标志的 agent 退回字母（pi → pi）。
//
// 色相与对话图上的厂商色点同源（`graph/vendor.ts`）。已知四家的色由
// `resources.css` 按 `.agent-badge-<slug>` 给，深浅主题各一套——**不能**内联，
// 内联自定义属性在层叠里压过样式表，浅色主题的压暗规则会整条失效（修正轮
// R1，DS A1）。未收录的 agent 没有主题分档，仍内联 vendor 原色兜底。

import type { CSSProperties } from 'react';
import { vendorColor } from '../../graph/vendor';
import { agentMark } from '../../resources/types';
import { agentEmblem, hasCssTone } from './brand-marks';

export function AgentBadge({ agentId, size = 'sm' }: { agentId: string; size?: 'sm' | 'md' }) {
  const emblem = agentEmblem(agentId, agentMark(agentId));
  const className = `agent-badge agent-badge-${size} agent-badge-${emblem.slug} agent-badge-${emblem.kind}`;
  const style = hasCssTone(emblem.slug)
    ? undefined
    : ({ '--agent-tone': vendorColor(agentId) } as CSSProperties);
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
