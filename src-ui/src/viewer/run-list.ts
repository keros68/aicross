// TASK-40 首页「最近的派发」的页面组装纯函数。数据契约（RunListItem）在
// ../tauri.ts（镜像 Rust src/run_view.rs 的 RunListItem）；这里只做展示前的
// 纯变换，Home.tsx 不掺逻辑，测试全部落在这层。run 级状态词与状态胶囊在
// ./run-view（fix-40-r1 R1-11），这里只引不复制。

import type { RunListItem } from '../tauri';

/** 开始时间的本地展示（精确到分钟）；解析不出来原样返回。 */
export function formatStartedAt(iso: string): string {
  const time = Date.parse(iso);
  if (!Number.isFinite(time)) return iso;
  const date = new Date(time);
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} `
    + `${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

/** 列表项主文案：模式名 + 任务名；任务名缺省时只留模式名。 */
export function runListItemText(item: Pick<RunListItem, 'presetName' | 'taskTitle'>): string {
  const preset = item.presetName.trim() || '自定义模式';
  const task = item.taskTitle.trim();
  return task === '' ? preset : `${preset} · ${task}`;
}

/** 空状态说明：没有 run 时首页显示这一句。 */
export function homeEmptyText(): string {
  return '还没有派发记录；在宿主里经 ai-cross skill 派发，或运行 aicross dispatch';
}
