// TASK-40 首页「最近的派发」的页面组装纯函数。数据契约（RunListItem）在
// ../tauri.ts（镜像 Rust src/run_view.rs 的 RunListItem）；这里只做展示前的
// 纯变换，Home.tsx 不掺逻辑，测试全部落在这层。

import type { RunListItem } from '../tauri';

/** 状态词：运行中给胶囊，失败与已拒绝给警示胶囊，已完成用纯文字。 */
export function runStatusLabel(status: string): string {
  const labels: Record<string, string> = {
    running: '运行中',
    done: '已完成',
    failed: '失败',
    rejected: '已拒绝',
  };
  return labels[status] ?? status;
}

export type HomeStatusTone = 'plain' | 'pill' | 'pill-alert';

/** 异常与运行中状态才给胶囊（视觉纪律）：已完成用纯文字。 */
export function runStatusTone(status: string): HomeStatusTone {
  if (status === 'running') return 'pill';
  if (status === 'failed' || status === 'rejected') return 'pill-alert';
  return 'plain';
}

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
