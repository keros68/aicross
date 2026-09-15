// agent 图标徽标（TASK-09）：统一风格的字母徽标，不用任何厂商商标图。
//
// 形状与字母对所有 agent 一样（圆角方形 + agentMark 的 1–2 个字母），只有色相
// 按家区分，符合 PRODUCT.md「视觉纪律」的厂商色一条：色相只做小面积标识。
//
// 修正轮 R1：色相不再另立一套，统一从 `graph/vendor.ts` 取——同一个 agent 在
// 对话图上的色点与这里的徽标必须同色。字母本身用 `--text-1`，色相只留在薄底与
// 描边上：全饱和色相在浅色主题上做小号文字达不到 4.5:1。

import type { CSSProperties } from 'react';
import { vendorColor } from '../../graph/vendor';
import { agentMark } from '../../resources/types';

export function AgentBadge({ agentId, size = 'sm' }: { agentId: string; size?: 'sm' | 'md' }) {
  return (
    <span
      className={`agent-badge agent-badge-${size}`}
      style={{ '--agent-tone': vendorColor(agentId) } as CSSProperties}
      aria-hidden="true"
    >
      {agentMark(agentId)}
    </span>
  );
}
