// TASK-37 查看器的页面组装纯函数。数据契约（RunView / RunViewNode）在
// ../tauri.ts（镜像 Rust src/run_view.rs），这里只做展示前的纯变换；
// RunViewer.tsx 不掺逻辑，测试全部落在这层。
//
// 原先复用的三个纯函数（TASK-40）：验收结论解析 parseAcceptance（原
// graph/dispatch.ts）、token 数缩写 compactRunNumber（原
// components/center/ConsoleRunDetail.tsx）内联在本文件，agent/model 显示
// 坐标 agentModelDisplay 搬到 ./agent-display，行为不变。

import { agentModelDisplay } from './agent-display';
import type { RunView, RunViewNode } from '../tauri';

export type AcceptanceVerdictParsed = 'PASS' | 'NEEDS_FIX' | 'UNKNOWN';

/** 验收 / 验证结论取正文尾部 1200 字符里最后一次出现的判定词。 */
export function parseAcceptance(text: string): AcceptanceVerdictParsed {
  const tail = text.slice(-1200).toUpperCase();
  const matches = [...tail.matchAll(/\b(?:PASS|NEEDS_FIX)\b/g)];
  return (matches.at(-1)?.[0] as AcceptanceVerdictParsed | undefined) ?? 'UNKNOWN';
}

/** token 数缩写：1.2k / 3.4m，行为与原 ConsoleRunDetail 的同名函数一致。 */
export function compactRunNumber(value: number): string {
  if (value < 10_000) return String(value);
  if (value < 1_000_000) return `${Number((value / 1000).toFixed(1))}k`;
  return `${Number((value / 1_000_000).toFixed(1))}m`;
}

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

/** 卡片主标题「角色 · 任务」：任务名依次取蜂群条目标题、本次派发的原始需求、
 * 图标题，都没有才退回 agent/model。同项目的派发共用一张图，图标题只是该项目
 * 第一次派发的任务，所以排在原始需求之后。 */
export function cardTitle(node: RunViewNode, view: Pick<RunView, 'graphTitle' | 'taskTitle'>): string {
  const role = node.role.trim() || '未命名角色';
  const task = node.swarmTitle.trim()
    || view.taskTitle.trim()
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
  if (node.status === 'draft') return '未开始';
  if (node.status === 'pending') return '运行中';
  if (node.status === 'done') return '完成';
  if (node.status === 'quota_exhausted' || node.errorKind === 'quota_exhausted') return '额度用尽';
  if (node.errorKind === 'identity_mismatch') return '实际模型不符';
  if (node.errorKind === 'validation_failed') return '验证命令未通过';
  if (node.errorKind === 'timeout') return '超时';
  if (node.errorKind === 'cancelled') return '已取消';
  return '失败';
}

/** 异常状态才给胶囊（视觉纪律）：完成与未开始用纯文字。 */
export function nodeStatusTone(node: RunViewNode): 'alert' | 'plain' {
  const label = nodeStatusLabel(node);
  return label === '完成' || label === '未开始' ? 'plain' : 'alert';
}

/** 实际回答的模型：拿不到时标「未返回」（kimi 不返回实际模型，标「未返回实际模型」），运行中给「—」，
 * 不回填请求值。 */
export function reportedModelLabel(node: RunViewNode): string {
  if (node.modelReported !== '') return node.modelReported;
  if (node.status === 'pending' || node.status === 'draft') return '—';
  const agent = (node.fallbackTo.split('/')[0] || node.agent).trim();
  return agent === 'kimi' ? '未返回实际模型' : '未返回';
}

/** 真身不符的标记只认引擎判定（errorKind === 'identity_mismatch'，
 * fix-37-r1 R1-3）；前端不做字符串比较——claude 请求常写别名、pi 的真身
 * 带 provider 前缀，比较必然误报。实际模型原样显示。 */
export function identityMismatch(node: RunViewNode): boolean {
  return node.errorKind === 'identity_mismatch';
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

/** 有结果的卡才有采纳动作的判据：图上有 reply 且已到终态。 */
export function canAdopt(node: RunViewNode): boolean {
  return node.hasResult && node.status !== 'draft' && node.status !== 'pending';
}

/** 采纳动作（fix-37-r1 R1-2）：第一版不做取消——引擎 adopt 只写采纳标记。
 * 已采纳的卡显示「已采纳」不可点；未采纳且结果落定的卡显示「采纳」；
 * 没有结果返回 null。 */
export type AdoptAction = { label: string; clickable: boolean } | null;

export function adoptAction(node: RunViewNode): AdoptAction {
  if (!canAdopt(node)) return null;
  return node.adopted
    ? { label: '已采纳', clickable: false }
    : { label: '采纳', clickable: true };
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

/** run 状态的展示（fix-37-r1 R1-11 视觉纪律）：运行中、失败、已拒绝这类要
 * 快速扫到的异常状态用胶囊（失败与拒绝带警示色）；已完成用纯文字。 */
export function runStatusTone(status: string): 'plain' | 'pill' | 'pill-alert' {
  if (status === 'running') return 'pill';
  if (status === 'failed' || status === 'rejected') return 'pill-alert';
  return 'plain';
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
