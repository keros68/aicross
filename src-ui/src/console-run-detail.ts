import type { DispatchNodeResult, DispatchProgress, DispatchStageRequest } from './tauri';
import { parseAcceptance } from './graph/dispatch';
import type { Graph } from './graph/types';

export const CONSOLE_RUN_LIMIT = 20;

export type ConsoleRunStatus = 'running' | 'completed' | 'stopped' | 'interrupted';
export type ConsoleAcceptance = 'PASS' | 'NEEDS_FIX' | 'UNKNOWN' | '';

export interface ConsoleRunRow {
  nodeId: string;
  stage: number;
  role: string;
  agent: string;
  provider: string;
  requestedModel: string;
  modelReported: string;
  status: string;
  acceptance: ConsoleAcceptance;
  tokensIn: number;
  tokensOut: number;
  cacheRead: number;
  durationMs: number;
  fallbackFrom: string;
  fallbackTo: string;
  progressElapsedMs: number;
  progressAt: number;
  toolCalls: number;
  lastTool: string;
}

export interface ConsoleRunRecord {
  id: string;
  presetName: string;
  startedAt: number;
  updatedAt: number;
  finishedAt?: number;
  wallDurationMs?: number;
  status: ConsoleRunStatus;
  conclusion: string;
  error: string;
  rows: ConsoleRunRow[];
}

export interface ConsoleRunTotals {
  routes: number;
  tokensIn: number;
  tokensOut: number;
  cacheRead: number;
  routeDurationMs: number;
  wallDurationMs: number;
  unreported: number;
}

export interface ConsoleScrollMetrics {
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
  messageCount: number;
}

export interface ConsoleScrollSnapshot extends ConsoleScrollMetrics {
  atBottom: boolean;
}

function finite(value: unknown, fallback = 0): number {
  return typeof value === 'number' && Number.isFinite(value) ? Math.max(0, value) : fallback;
}

function emptyRow(
  nodeId: string,
  stage: number,
  role: string,
  agent: string,
  provider: string,
  requestedModel: string,
): ConsoleRunRow {
  return {
    nodeId,
    stage,
    role,
    agent,
    provider,
    requestedModel,
    modelReported: '',
    status: 'pending',
    acceptance: '',
    tokensIn: 0,
    tokensOut: 0,
    cacheRead: 0,
    durationMs: 0,
    fallbackFrom: '',
    fallbackTo: '',
    progressElapsedMs: 0,
    progressAt: 0,
    toolCalls: 0,
    lastTool: '',
  };
}

function replaceRow(record: ConsoleRunRecord, row: ConsoleRunRow, now: number): ConsoleRunRecord {
  const index = record.rows.findIndex((item) => item.nodeId === row.nodeId);
  const rows = index < 0
    ? [...record.rows, row]
    : record.rows.map((item, rowIndex) => rowIndex === index ? row : item);
  return { ...record, updatedAt: now, rows };
}

export function createConsoleRunRecord(
  id: string,
  presetName: string,
  startedAt: number,
): ConsoleRunRecord {
  return {
    id,
    presetName,
    startedAt,
    updatedAt: startedAt,
    status: 'running',
    conclusion: '',
    error: '',
    rows: [],
  };
}

export function runDetailKeyAction(
  event: { key: string; isComposing?: boolean },
): 'close' | null {
  return event.key === 'Escape' && !event.isComposing ? 'close' : null;
}

export function runDetailTabTarget(
  activeIndex: number,
  focusableCount: number,
  shiftKey: boolean,
): number | null {
  if (focusableCount <= 0) return null;
  if (activeIndex < 0) return shiftKey ? focusableCount - 1 : 0;
  return (activeIndex + (shiftKey ? -1 : 1) + focusableCount) % focusableCount;
}

export function runDetailTabPlan(
  activeIndex: number,
  focusableCount: number,
  shiftKey: boolean,
): { kind: 'item'; index: number } | { kind: 'root' } {
  const target = runDetailTabTarget(activeIndex, focusableCount, shiftKey);
  return target === null ? { kind: 'root' } : { kind: 'item', index: target };
}

export function runDetailFocusTarget(previousConnected: boolean): 'previous' | 'composer' {
  return previousConnected ? 'previous' : 'composer';
}

export function consoleRunHeaderLabel(
  record: Pick<ConsoleRunRecord, 'presetName' | 'startedAt'>,
): string {
  const date = new Date(record.startedAt);
  const pad = (value: number) => String(value).padStart(2, '0');
  const minute = [
    date.getFullYear(),
    pad(date.getMonth() + 1),
    pad(date.getDate()),
  ].join('-');
  return `${record.presetName} · ${minute} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

export function consoleScrollSnapshot(metrics: ConsoleScrollMetrics): ConsoleScrollSnapshot {
  return {
    ...metrics,
    atBottom: metrics.scrollHeight - metrics.clientHeight - metrics.scrollTop <= 2,
  };
}

export function consoleScrollRestoreTop(
  snapshot: ConsoleScrollSnapshot,
  current: Pick<ConsoleScrollMetrics, 'scrollHeight' | 'clientHeight' | 'messageCount'>,
): number {
  const maximum = Math.max(0, current.scrollHeight - current.clientHeight);
  if (snapshot.atBottom && current.messageCount > snapshot.messageCount) return maximum;
  return Math.min(Math.max(0, snapshot.scrollTop), maximum);
}

export function consoleAutoScrollStep(skipNext: boolean): { shouldScroll: boolean; skipNext: boolean } {
  return { shouldScroll: !skipNext, skipNext: false };
}

export function runDetailAvailabilityAction(
  openRunId: string,
  recordAvailable: boolean,
): 'close' | null {
  return openRunId !== '' && !recordAvailable ? 'close' : null;
}

export function withConsoleRunStage(
  record: ConsoleRunRecord,
  request: DispatchStageRequest,
  now: number,
): ConsoleRunRecord {
  let next = record;
  for (const node of request.nodes) {
    const previous = next.rows.find((row) => row.nodeId === node.nodeId)
      ?? emptyRow(node.nodeId, request.stage, node.role, node.agent, node.provider, node.model);
    next = replaceRow(next, {
      ...previous,
      stage: request.stage,
      role: node.role,
      agent: node.agent,
      provider: node.provider,
      requestedModel: node.model,
      status: previous.status === 'running' ? 'running' : 'pending',
      progressAt: previous.status === 'running' ? previous.progressAt : 0,
    }, now);
  }
  return next;
}

export function withConsoleRunProgress(
  record: ConsoleRunRecord,
  progress: DispatchProgress,
  now: number,
): ConsoleRunRecord {
  if (progress.runId !== record.id) return record;
  const previous = record.rows.find((row) => row.nodeId === progress.nodeId);
  if (!previous || previous.status !== 'pending' && previous.status !== 'running') return record;
  return replaceRow(record, {
    ...previous,
    status: 'running',
    progressElapsedMs: finite(progress.elapsedMs, previous.progressElapsedMs),
    progressAt: now,
    toolCalls: finite(progress.toolCalls, previous.toolCalls),
    lastTool: typeof progress.lastTool === 'string' ? progress.lastTool : previous.lastTool,
  }, now);
}

function rowFromResult(
  previous: ConsoleRunRow | undefined,
  graph: Graph,
  result: DispatchNodeResult,
): ConsoleRunRow | null {
  const target = graph.nodes[result.nodeId]?.target;
  if (!previous && !target) return null;
  const base = previous ?? emptyRow(
    result.nodeId,
    result.stage,
    target?.role ?? '',
    target?.agent ?? '',
    target?.provider ?? '',
    target?.model ?? '',
  );
  const role = target?.role ?? base.role;
  const acceptance = role === '验收' || role === '验证'
    ? (result.status === 'ok' ? parseAcceptance(result.text) : 'UNKNOWN')
    : '';
  return {
    ...base,
    stage: result.stage,
    role,
    agent: target?.agent ?? base.agent,
    provider: target?.provider ?? base.provider,
    requestedModel: target?.model ?? base.requestedModel,
    modelReported: result.modelReported,
    status: result.timedOut || result.status === 'timeout' ? 'timeout' : result.status,
    acceptance,
    tokensIn: finite(result.tokensIn),
    tokensOut: finite(result.tokensOut),
    cacheRead: finite(result.cacheRead),
    durationMs: finite(result.durationMs),
    fallbackFrom: result.fallbackFrom ?? '',
    fallbackTo: result.fallbackTo ?? '',
  };
}

export function withConsoleRunResults(
  record: ConsoleRunRecord,
  graph: Graph,
  results: DispatchNodeResult[],
  now: number,
): ConsoleRunRecord {
  let next = record;
  const latest = [...new Map(results.map((result) => [result.nodeId, result])).values()];
  for (const result of latest) {
    const row = rowFromResult(next.rows.find((item) => item.nodeId === result.nodeId), graph, result);
    if (row) next = replaceRow(next, row, now);
  }
  return next;
}

function hasAncestor(graph: Graph, nodeId: string, ancestorId: string): boolean {
  const pending = [...(graph.nodes[nodeId]?.parents ?? [])];
  const seen = new Set<string>();
  while (pending.length > 0) {
    const id = pending.pop() as string;
    if (id === ancestorId) return true;
    if (seen.has(id)) continue;
    seen.add(id);
    pending.push(...(graph.nodes[id]?.parents ?? []));
  }
  return false;
}

function correctionRoundCount(
  graph: Graph,
  lastAcceptanceRows: ConsoleRunRow[],
  correctionNodeIds: ReadonlySet<string>,
): number {
  const memo = new Map<string, number>();
  const depth = (nodeId: string, visiting = new Set<string>()): number => {
    const cached = memo.get(nodeId);
    if (cached !== undefined) return cached;
    if (visiting.has(nodeId)) return 0;
    const node = graph.nodes[nodeId];
    if (!node) return 0;
    const nextVisiting = new Set(visiting).add(nodeId);
    const parentDepth = Math.max(0, ...node.parents.map((parentId) => depth(parentId, nextVisiting)));
    const value = parentDepth + (node.target?.role === '执行' && correctionNodeIds.has(nodeId) ? 1 : 0);
    memo.set(nodeId, value);
    return value;
  };
  return Math.max(0, ...lastAcceptanceRows.map((row) => depth(row.nodeId)));
}

function acceptanceConclusion(
  graph: Graph,
  rows: ConsoleRunRow[],
  correctionNodeIds: ReadonlySet<string>,
): string {
  const acceptanceRows = rows.filter((row) => row.role === '验收' || row.role === '验证');
  if (acceptanceRows.length === 0) return '已完成（本模式不含验收）';
  const last = acceptanceRows.filter((row) => !acceptanceRows.some((later) =>
    later.nodeId !== row.nodeId && hasAncestor(graph, later.nodeId, row.nodeId)));
  const round = correctionRoundCount(graph, last, correctionNodeIds);
  if (last.some((row) => row.acceptance === 'NEEDS_FIX')) {
    return round > 0
      ? `未通过（修正 ${round} 轮后验收仍要求修正，已到上限）`
      : '未通过（验收要求修正，已到上限）';
  }
  if (last.some((row) => row.acceptance !== 'PASS')) {
    return round > 0
      ? `验收结论无法识别（修正 ${round} 轮后）`
      : '验收结论无法识别（验收一次）';
  }
  return round > 0
    ? `通过（修正 ${round} 轮后通过）`
    : '通过（验收一次通过）';
}

export function finishConsoleRun(
  record: ConsoleRunRecord,
  graph: Graph,
  results: DispatchNodeResult[],
  error: string,
  now: number,
  correctionNodeIds: ReadonlySet<string> = new Set(),
): ConsoleRunRecord {
  const updated = withConsoleRunResults(record, graph, results, now);
  const stoppedReason = error.replace(/[。.!！？?]+$/u, '');
  const settledRows = updated.rows.map((row) => row.status === 'pending' || row.status === 'running' ? {
    ...row,
    status: 'interrupted',
    durationMs: liveConsoleRunDurationMs(row, now),
  } : row);
  return {
    ...updated,
    rows: settledRows,
    updatedAt: now,
    finishedAt: now,
    wallDurationMs: Math.max(0, now - record.startedAt),
    status: error ? 'stopped' : 'completed',
    conclusion: error ? `中途停止（原因：${stoppedReason}）` : acceptanceConclusion(graph, settledRows, correctionNodeIds),
    error,
  };
}

export function tokenUnreported(row: ConsoleRunRow): boolean {
  return row.status !== 'pending' && row.status !== 'running' && row.tokensIn === 0 && row.tokensOut === 0;
}

export function liveConsoleRunDurationMs(row: ConsoleRunRow, now: number): number {
  if (row.status !== 'running') return row.durationMs;
  return Math.max(
    row.durationMs,
    row.progressElapsedMs + (row.progressAt > 0 ? Math.max(0, now - row.progressAt) : 0),
  );
}

export function consoleRunTotals(record: ConsoleRunRecord, now: number): ConsoleRunTotals {
  return record.rows.reduce<ConsoleRunTotals>((totals, row) => {
    const unreported = tokenUnreported(row);
    return {
      routes: totals.routes + 1,
      tokensIn: totals.tokensIn + (unreported ? 0 : row.tokensIn),
      tokensOut: totals.tokensOut + (unreported ? 0 : row.tokensOut),
      cacheRead: totals.cacheRead + (unreported ? 0 : row.cacheRead),
      routeDurationMs: totals.routeDurationMs + liveConsoleRunDurationMs(row, now),
      wallDurationMs: record.wallDurationMs ?? Math.max(0, now - record.startedAt),
      unreported: totals.unreported + (unreported ? 1 : 0),
    };
  }, {
    routes: 0,
    tokensIn: 0,
    tokensOut: 0,
    cacheRead: 0,
    routeDurationMs: 0,
    wallDurationMs: record.wallDurationMs ?? Math.max(0, now - record.startedAt),
    unreported: 0,
  });
}

export function reportedModelLabel(row: ConsoleRunRow): string {
  if (row.modelReported) return row.modelReported;
  if (row.status === 'pending' || row.status === 'running') return '—';
  const actualAgent = row.fallbackTo.split('/')[0] || row.agent;
  return actualAgent === 'kimi' ? 'kimi 不报真身' : '未报告';
}

export function runRowStatusLabel(status: string): string {
  if (status === 'pending') return '排队中';
  if (status === 'running') return '进行中';
  if (status === 'interrupted') return '已中断';
  if (status === 'timeout') return '超时';
  return status === 'ok' ? '成功' : '失败';
}

export function acceptanceLabel(value: ConsoleAcceptance): string {
  if (value === 'PASS') return '通过';
  if (value === 'NEEDS_FIX') return '需修正';
  if (value === 'UNKNOWN') return '未知';
  return '—';
}

function rowFrom(value: unknown): ConsoleRunRow | null {
  if (!value || typeof value !== 'object') return null;
  const row = value as Record<string, unknown>;
  if (typeof row.nodeId !== 'string' || row.nodeId === ''
    || typeof row.role !== 'string' || typeof row.agent !== 'string'
    || typeof row.provider !== 'string' || typeof row.requestedModel !== 'string'
    || typeof row.modelReported !== 'string' || typeof row.status !== 'string'
    || typeof row.fallbackFrom !== 'string' || typeof row.fallbackTo !== 'string'
    || typeof row.lastTool !== 'string') return null;
  const acceptance = row.acceptance;
  if (acceptance !== '' && acceptance !== 'PASS' && acceptance !== 'NEEDS_FIX' && acceptance !== 'UNKNOWN') return null;
  return {
    nodeId: row.nodeId,
    stage: finite(row.stage, 1) || 1,
    role: row.role,
    agent: row.agent,
    provider: row.provider,
    requestedModel: row.requestedModel,
    modelReported: row.modelReported,
    status: row.status,
    acceptance,
    tokensIn: finite(row.tokensIn),
    tokensOut: finite(row.tokensOut),
    cacheRead: finite(row.cacheRead),
    durationMs: finite(row.durationMs),
    fallbackFrom: row.fallbackFrom,
    fallbackTo: row.fallbackTo,
    progressElapsedMs: finite(row.progressElapsedMs),
    progressAt: finite(row.progressAt),
    toolCalls: finite(row.toolCalls),
    lastTool: row.lastTool,
  };
}

export function parseConsoleRunRecord(value: unknown): ConsoleRunRecord | null {
  if (!value || typeof value !== 'object') return null;
  const record = value as Record<string, unknown>;
  if (typeof record.id !== 'string' || record.id === ''
    || typeof record.presetName !== 'string'
    || typeof record.conclusion !== 'string' || typeof record.error !== 'string'
    || !Array.isArray(record.rows)
    || !['running', 'completed', 'stopped', 'interrupted'].includes(String(record.status))) return null;
  const rows = record.rows.map(rowFrom);
  if (rows.some((row) => row === null)) return null;
  const startedAt = finite(record.startedAt);
  const updatedAt = finite(record.updatedAt, startedAt);
  if (startedAt <= 0 || updatedAt < startedAt) return null;
  return {
    id: record.id,
    presetName: record.presetName,
    startedAt,
    updatedAt,
    ...(typeof record.finishedAt === 'number' && Number.isFinite(record.finishedAt)
      ? { finishedAt: finite(record.finishedAt) }
      : {}),
    ...(typeof record.wallDurationMs === 'number' && Number.isFinite(record.wallDurationMs)
      ? { wallDurationMs: finite(record.wallDurationMs) }
      : {}),
    status: record.status as ConsoleRunStatus,
    conclusion: record.conclusion,
    error: record.error,
    rows: rows as ConsoleRunRow[],
  };
}

export function interruptConsoleRun(record: ConsoleRunRecord): ConsoleRunRecord {
  if (record.status !== 'running') return record;
  return {
    ...record,
    status: 'interrupted',
    finishedAt: record.updatedAt,
    wallDurationMs: Math.max(0, record.updatedAt - record.startedAt),
    conclusion: '中途停止（应用已重启）',
    error: '应用已重启',
    rows: record.rows.map((row) => row.status === 'pending' || row.status === 'running' ? {
      ...row,
      status: 'interrupted',
      durationMs: liveConsoleRunDurationMs(row, record.updatedAt),
    } : row),
  };
}
