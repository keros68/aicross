// PageHeader.tsx — 页面工具行（fix-51-r1，参照 magpie）：页名与窗口按钮在顶栏，
// 这里只放页面自己的东西，左边是筛选或数据（如上次探测时间、实际厂商），右边是
// 页面动作。两边都没有时不渲染。

import type { ReactNode } from 'react';

export function PageHeader({ left, actions }: { left?: ReactNode; actions?: ReactNode }) {
  if (!left && !actions) return null;
  return (
    <div className="page-toolbar">
      <div className="page-toolbar-left">{left}</div>
      {actions && <div className="page-toolbar-actions">{actions}</div>}
    </div>
  );
}
