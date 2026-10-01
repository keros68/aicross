// TASK-37 查看器的页面组装纯函数。数据契约（RunView / RunViewNode）在
// ../tauri.ts（镜像 Rust src/run_view.rs），这里只做展示前的纯变换；
// RunViewer.tsx 不掺逻辑，测试全部落在这层。
//
// 复用的现成纯函数：graph/dispatch.ts 的 parseAcceptance（验收结论）、
// resources/agent-display.ts 的 agentModelDisplay（次标题）、
// ConsoleRunDetail.tsx 的 compactRunNumber（token 数缩写）。

import { parseAcceptance } from '../graph/dispatch';
import { agentModelDisplay } from '../resources/agent-display';
import { compactRunNumber } from '../components/center/ConsoleRunDetail';
import type { RunView, RunViewNode } from '../tauri';

/** run 运行中时查看页的轮询间隔；终态返回 null，轮询停止。 */
export const RUN_VIEW_POLL_INTERVAL_MS = 2000;

export function pollIntervalMs(status: string): number | null {
  return status === 'running' ? RUN_VIEW_POLL_INTERVAL_MS : null;
}

export interface RunStageGroup {
  stage: number;
  title: string;
  nodes: RunViewNode[];
}

/** 按阶段分组：阶段号升序，组内保持 run 摘要里的节点顺序；组标题取组内
 * 第一个非空角色，都没有时退回「第 N 阶段」。 */
export function groupNodesByStage(view: Pick<RunView, 'nodes'>): RunStageGroup[] {
  const stages = [...new Set(view.nodes.map((node) => node.stage))].sort((a, b) => a - b);
  return stages.map((stage) => {
    const nodes = view.nodes.filter((node) => node.stage === stage);
    const role = nodes.map((node) => node.role.trim()).find(Boolean);
    return { stage, title: role || `第 ${stage} 阶段`, nodes };
  });
}

/** 组内最多 3 列，超出换行；列数至少 1。 */
export function stageColumnCount(count: number): number {
  return Math.max(1, Math.min(3, count));
}

/** 卡片主标题「角色 · 任务」：任务名依次取蜂群条目标题、图标题，都没有才
 * 退回 agent/model。 */
export function cardTitle(node: RunViewNode, view: Pick<RunView, 'graphTitle'>): string {
  const role = node.role.trim() || '未命名角色';
  const task = node.swarmTitle.trim()
    || view.graphTitle.trim()
    || (node.agent.trim() && node.model.trim() ? `${node.agent}/${node.model}` : '未指定模型');
  return `${role} · ${task}`;
}

/** 卡片次标题：用户可见的 agent/model 坐标。 */
export function cardSubtitle(node: RunViewNode): string {
  if (!node.agent.trim() && !node.model.trim()) return '未指定模型';
  return agentModelDisplay(node.agent, node.model, node.provider);
}

export function nodeStatusLabel(node: RunViewNode): string {
  if (node.skippedReason !== '') return '已跳过';
  if (node.status === 'draft') return '待派';
  if (node.status === 'pending') return '运行中';
  if (node.status === 'done') return '完成';
  if (node.status === 'quota_exhausted' || node.errorKind === 'quota_exhausted') return '额度用尽';
  if (node.errorKind === 'identity_mismatch') return '真身不符';
  if (node.errorKind === 'validation_failed') return '验证命令未通过';
  if (node.errorKind === 'timeout') return '超时';
  if (node.errorKind === 'cancelled') return '已取消';
  return '失败';
}

/** 异常状态才给胶囊（视觉纪律）：完成与待派用纯文字。 */
export function nodeStatusTone(node: RunViewNode): 'alert' | 'plain' {
  const label = nodeStatusLabel(node);
  return label === '完成' || label === '待派' ? 'plain' : 'alert';
}

/** 实际回答的模型：拿不到真身标「未报告」（kimi 不报真身），运行中给「—」，
 * 不回填请求值。 */
export function reportedModelLabel(node: RunViewNode): string {
  if (node.modelReported !== '') return node.modelReported;
  if (node.status === 'pending' || node.status === 'draft') return '—';
  const agent = (node.fallbackTo.split('/')[0] || node.agent).trim();
  return agent === 'kimi' ? 'kimi 不报真身' : '未报告';
}

/** 真身与请求不一致：有真身才比；切过备选时按备选的模型比。 */
export function identityMismatch(node: RunViewNode): boolean {
  if (node.modelReported === '') return false;
  const requested = node.fallbackFrom !== ''
    ? (node.fallbackTo.split('/')[1] ?? node.model)
    : node.model;
  return node.modelReported !== requested;
}

/** 额度用尽的恢复时间；不是额度态或没记时间为空串。 */
export function quotaResetText(node: RunViewNode): string {
  const exhausted = node.status === 'quota_exhausted' || node.errorKind === 'quota_exhausted';
  return exhausted && node.quotaResetsAt !== '' ? `恢复时间 ${node.quotaResetsAt}` : '';
}

/** 备选切换说明。 */
export function fallbackText(node: RunViewNode): string {
  return node.fallbackFrom !== '' && node.fallbackTo !== ''
    ? `已切备选：${node.fallbackFrom} → ${node.fallbackTo}`
    : '';
}

/** 有结果的卡才给「采纳 / 取消采纳」：图上有 reply 且已到终态。 */
export function canAdopt(node: RunViewNode): boolean {
  return node.hasResult && node.status !== 'draft' && node.status !== 'pending';
}

export type AcceptanceVerdict = 'PASS' | 'NEEDS_FIX' | 'UNKNOWN' | null;

/** 验收 / 验证那两路的结论：完成看正文尾部判定，其余按未判定；别的角色为
 * null（不显示）。 */
export function acceptanceVerdict(node: RunViewNode): AcceptanceVerdict {
  if (node.role !== '验收' && node.role !== '验证') return null;
  return node.status === 'done' ? parseAcceptance(node.text) : 'UNKNOWN';
}

export function acceptanceVerdictLabel(verdict: AcceptanceVerdict): string {
  if (verdict === 'PASS') return '通过';
  if (verdict === 'NEEDS_FIX') return '需修正';
  if (verdict === 'UNKNOWN') return '未判定';
  return '—';
}

export function runStatusLabel(status: string): string {
  const labels: Record<string, string> = {
    running: '运行中',
    done: '已完成',
    failed: '失败',
    rejected: '已拒绝',
  };
  return labels[status] ?? status;
}

/** 总耗时：运行中按当前时刻，终态按最后更新时刻；时间解析不出来返回 null。 */
export function runElapsedMs(
  view: Pick<RunView, 'startedAt' | 'updatedAt' | 'status'>,
  nowMs: number,
): number | null {
  const start = Date.parse(view.startedAt);
  if (!Number.isFinite(start)) return null;
  const end = view.status === 'running' ? nowMs : Date.parse(view.updatedAt);
  if (!Number.isFinite(end)) return null;
  return Math.max(0, end - start);
}

export function formatDuration(ms: number | null): string {
  return ms === null ? '—' : `${(ms / 1000).toFixed(1)} 秒`;
}

export function formatTokens(value: number | null): string {
  return value === null || value <= 0 ? '—' : compactRunNumber(value);
}

/** 开始时间的本地展示（精确到秒）。 */
export function formatStartedAt(iso: string): string {
  const time = Date.parse(iso);
  if (!Number.isFinite(time)) return iso;
  const date = new Date(time);
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} `
    + `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}
