// StatCards.tsx — 页顶数字条（fix-49-r2；fix-51-r1 改为 magpie 的 usage 样式）：
// 一张白卡里横向几格，格间竖分隔线；每格大号数字 + 下方灰色小字标签。数字从已有
// 数据算出（各页的 *Overview 纯函数）。带 filter 的格可点，作为下方列表的筛选
// （fix-51-r2，资源页「不可用」）；再点一次取消。

import type { StatItem } from '../../config/config-view';

export function StatCards({
  items,
  activeFilter,
  onFilter,
}: {
  items: StatItem[];
  activeFilter?: string | null;
  onFilter?: (filter: string | null) => void;
}) {
  return (
    <div className="stat-strip">
      {items.map((item) => {
        const body = (
          <>
            <span className={`stat-value${item.alert ? ' alert' : ''}`} title={item.value}>{item.value}</span>
            <span className="stat-label">{item.label}</span>
          </>
        );
        if (!item.filter || !onFilter) return <div key={item.label} className="stat-cell">{body}</div>;
        const selected = activeFilter === item.filter;
        return (
          <button
            key={item.label}
            type="button"
            className={`stat-cell stat-cell-button${selected ? ' selected' : ''}`}
            aria-pressed={selected}
            onClick={() => onFilter(selected ? null : item.filter ?? null)}
          >
            {body}
          </button>
        );
      })}
    </div>
  );
}
