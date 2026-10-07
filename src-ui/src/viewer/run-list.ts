// TASK-40「派发记录」页（fix-49-r1 起为表格）的页面组装纯函数。数据契约（RunListItem）在
// ../tauri.ts（镜像 Rust src/run_view.rs 的 RunListItem）；这里只做展示前的
// 纯变换，Home.tsx 不掺逻辑，测试全部落在这层。run 级状态词与状态胶囊在
// ./run-view（fix-40-r1 R1-11），这里只引不复制。

import type { RunListItem } from '../tauri';
import type { StatItem } from '../config/config-view';

/** 开始时间的本地展示（精确到分钟）；解析不出来原样返回。 */
export function formatStartedAt(iso: string): string {
  const time = Date.parse(iso);
  if (!Number.isFinite(time)) return iso;
  const date = new Date(time);
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} `
    + `${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

const CUSTOM_PREFIX = '自定义模式 ·';

/** 表格一行的「模式」「任务」两列（fix-49-r3）：模式列只放模式名，自定义模式去掉
 *  「自定义模式 · 」前缀只留其名称；模式名缺省回「自定义模式」，任务名缺省为空串。 */
export function runRowCells(item: Pick<RunListItem, 'presetName' | 'taskTitle'>): { mode: string; task: string } {
  const preset = item.presetName.trim();
  const mode = preset.startsWith(CUSTOM_PREFIX) ? preset.slice(CUSTOM_PREFIX.length).trim() : preset;
  return {
    mode: mode || '自定义模式',
    task: item.taskTitle.trim(),
  };
}

/** 空状态：没有 run 时显示。 */
export function homeEmptyText(): string {
  return '暂无派发记录';
}

/** 失败（含派发前被拒）。 */
const RUN_FAILED: ReadonlySet<string> = new Set(['failed', 'rejected']);

/** 派发记录页概览（fix-49-r2）：最近记录 · 已完成 · 失败（含已拒绝）· 进行中。 */
export function runsOverview(runs: Pick<RunListItem, 'status'>[]): StatItem[] {
  const done = runs.filter((run) => run.status === 'done').length;
  const failed = runs.filter((run) => RUN_FAILED.has(run.status)).length;
  const running = runs.filter((run) => run.status === 'running').length;
  return [
    { label: '最近记录', value: String(runs.length), alert: false },
    { label: '已完成', value: String(done), alert: false },
    { label: '失败', value: String(failed), alert: failed > 0 },
    { label: '进行中', value: String(running), alert: false },
  ];
}


export type RunFilter = 'all' | 'done' | 'failed';

export const RUN_FILTERS: ReadonlyArray<{ id: RunFilter; label: string }> = [
  { id: 'all', label: '全部' },
  { id: 'done', label: '已完成' },
  { id: 'failed', label: '失败' },
];

/** 表格上方分段切换的前端筛选；「失败」含已拒绝。 */
export function filterRuns<T extends Pick<RunListItem, 'status'>>(runs: T[], filter: RunFilter): T[] {
  if (filter === 'done') return runs.filter((run) => run.status === 'done');
  if (filter === 'failed') return runs.filter((run) => RUN_FAILED.has(run.status));
  return runs;
}

/** 查看页同阶段卡片的栅格（TASK-49）：最多并排 columns 张；内容区窄到放不下
 *  240px 一张时自动折行，不出现横向滚动。12px 与 .run-viewer-cards 的 gap 一致。 */
export function stageGridColumns(columns: number): string {
  const count = Math.max(1, Math.floor(columns));
  const gaps = (count - 1) * 12;
  return `repeat(auto-fill, minmax(max(240px, calc((100% - ${gaps}px) / ${count})), 1fr))`;
}
