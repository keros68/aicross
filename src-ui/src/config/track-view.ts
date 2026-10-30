// TASK-69 资源页模型行的使用记录与标注。

import type { TrackCounts, TrackModel } from '../tauri';

/** 与引擎同一写法：agent:provider:model，provider 空时中间留空。 */
export function trackKey(agent: string, provider: string | null | undefined, model: string): string {
  return `${agent}:${provider ?? ''}:${model}`;
}

function kindText(counts: TrackCounts): string {
  const parts = [`${counts.kindName} ${counts.runs} 次`];
  if (counts.judged > 0) parts.push(`一次通过 ${counts.firstPass}`);
  if (counts.passRate !== null) parts.push(`通过率 ${counts.passRate}%`);
  if (counts.adopted > 0) parts.push(`采纳 ${counts.adopted}`);
  return parts.join('，');
}

/** 一行记录：按次数多少取前三类；没有记录为空串。 */
export function trackLine(model: TrackModel | undefined): string {
  if (!model) return '';
  const kinds = model.byKind
    .filter((counts) => counts.runs > 0 || counts.adopted > 0)
    .sort((a, b) => b.runs - a.runs || a.kindName.localeCompare(b.kindName));
  if (kinds.length === 0) return '';
  const shown = kinds.slice(0, 3).map(kindText).join('；');
  return kinds.length > 3 ? `记录：${shown}；另有 ${kinds.length - 3} 类` : `记录：${shown}`;
}

/** 标签按类型表的顺序排。 */
export function orderTags(tags: string[], kindList: { id: string }[]): string[] {
  const wanted = new Set(tags);
  return kindList.map((kind) => kind.id).filter((id) => wanted.has(id));
}
