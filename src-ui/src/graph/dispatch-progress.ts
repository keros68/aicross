// 派发进度的共享层（TASK-07C）：`dispatch-progress` 的订阅原来在
// DispatchOverlay 里，运行屏一关、或者用户切回图上看节点卡，就什么读数都没有。
// 这里把「订阅 + 5s 时钟 + 复位时机」提到 GraphTab 层，运行屏与图上的节点卡
// 读同一份快照。
//
// 判定逻辑不在这里：耗时/工具文案是 graph/dispatch.ts 的 runningDetailOf，
// 空转判定是同一个文件的 idleHint，按 runId 取快照是 progressOf。本文件只做
// state 形状与 React 接线，纯函数部分（beginProgressRun / applyProgressEvent /
// progressRunOf）单独导出，测试不必渲染组件。
//
// 复位语义与 REPORT-07B 的 R1 一致：
//   - 新一轮（确认派发 / 重派）：beginProgressRun 清空快照并换 runId；
//   - 迟到的上一轮事件：applyProgressEvent 原样返回（nextProgress 按 runId 挡）；
//   - 换图：GraphTab 按 graphId 复位成 EMPTY_PROGRESS_RUN；
//   - 终态：不清快照；终态事件不带心跳字段，progressSnapshot 沿用上一条 running
//     的读数（R1-1），runningDetailOf 把它冻结在最后一条心跳上；
//   - 异常收尾：endProgressRun 把没收到终态的节点标成终态，5s 时钟随之停（R1-3）。

import { useCallback, useEffect, useMemo, useState } from 'react';
import type { DispatchProgress } from '../tauri';
import { nextProgress, progressOf, progressRunning, progressRunningAny, type ProgressSnapshot } from './dispatch';

/** 一轮派发的进度：本轮 runId + 每个节点最后一条心跳的快照。 */
export interface ProgressRun {
  runId: string;
  progress: Record<string, ProgressSnapshot>;
}

/** 还没有派发过，或者已经换图。 */
export const EMPTY_PROGRESS_RUN: ProgressRun = { runId: '', progress: {} };

/** 心跳之间让「已运行」「无输出多久」继续走的时钟步长。引擎心跳是 15s。 */
export const PROGRESS_TICK_MS = 5000;

/** 新一轮开始：清掉上一轮的快照，记下本轮 runId。 */
export function beginProgressRun(runId: string): ProgressRun {
  return { runId, progress: {} };
}

/** 一条心跳落进 state。别轮次的事件不写入，返回同一引用。 */
export function applyProgressEvent(
  run: ProgressRun,
  payload: DispatchProgress,
  now: number,
): ProgressRun {
  const progress = nextProgress(run.progress, payload, now, run.runId);
  return progress === run.progress ? run : { runId: run.runId, progress };
}

/** 取某个节点本轮的快照；runId 对不上就当没有。运行屏与节点卡都走这一个入口。 */
export function progressRunOf(run: ProgressRun, nodeId: string): ProgressSnapshot | undefined {
  return progressOf(run.progress, nodeId, run.runId);
}

/** 本轮结束时没收到终态事件的状态词。异常收尾、丢事件都走它。 */
export const PROGRESS_ENDED_STATUS = '已结束';

// 本轮收尾（R1-3）：派发抛错、命令 reject 的路径不会给每个节点补终态事件，
// 快照会一直停在 running——5s 时钟永不停，3 分钟后还会挂出「疑似空转」，
// 而这一层早就判失败了。收尾时把还在跑的快照标成终态，读数按最后一条心跳冻结。
export function endProgressRun(run: ProgressRun, status: string = PROGRESS_ENDED_STATUS): ProgressRun {
  if (!progressRunningAny(run.progress)) return run;
  const progress: Record<string, ProgressSnapshot> = {};
  for (const [nodeId, snapshot] of Object.entries(run.progress)) {
    progress[nodeId] = progressRunning(snapshot) ? { ...snapshot, status } : snapshot;
  }
  return { runId: run.runId, progress };
}

// `dispatch-progress` 的订阅（R1-2）：effect 体抽出来单测。异步 import + listen
// 之间组件可能已经卸载或换了 runId，那时 cleanup 里的 unlisten 还是 undefined，
// 监听装上就再也拆不掉；用 disposed 标志在迟到的 resolve 里就地解除。
export function subscribeProgress(
  runId: string,
  onEvent: (payload: DispatchProgress) => void,
): { dispose: () => void } {
  if (runId === '') return { dispose: () => {} };
  let disposed = false;
  let unlisten: (() => void) | undefined;
  import('@tauri-apps/api/event')
    .then(({ listen }) => listen<DispatchProgress>(
      'dispatch-progress',
      (event) => onEvent(event.payload),
    ))
    .then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    })
    .catch((error) => { console.warn('派发进度订阅失败', error); });
  return {
    dispose: () => {
      disposed = true;
      unlisten?.();
      unlisten = undefined;
    },
  };
}

/** 共享给运行屏与节点卡的进度状态。 */
export interface DispatchProgressStore {
  /** 本轮 runId；没派发过是空串。 */
  runId: string;
  progress: Record<string, ProgressSnapshot>;
  /** 时钟当前值，传给 runningDetailOf / idleHint。 */
  now: number;
  /** 取某个节点本轮的快照。 */
  snapshotOf: (nodeId: string) => ProgressSnapshot | undefined;
  /** 新一轮开始（确认派发 / 重派）。 */
  beginRun: (runId: string) => void;
  /** 本轮收尾：没收到终态事件的节点标成终态，时钟随之停。 */
  endRun: () => void;
}

export function useDispatchProgress(graphId: string): DispatchProgressStore {
  const [run, setRun] = useState<ProgressRun>(EMPTY_PROGRESS_RUN);
  const [now, setNow] = useState(() => Date.now());

  // 换图复位：上一张图的读数不能落到新图的节点卡上（口径同 reconcileUiFor）。
  useEffect(() => {
    setRun(EMPTY_PROGRESS_RUN);
  }, [graphId]);

  // 一轮开始才订阅，没派发过不挂监听。runId 变（重派）时重挂一次。
  useEffect(() => {
    if (run.runId === '') return;
    const subscription = subscribeProgress(
      run.runId,
      (payload) => setRun((current) => applyProgressEvent(current, payload, Date.now())),
    );
    return () => subscription.dispose();
  }, [run.runId]);

  // 引擎卡死时不会再有心跳，只有这个时钟能让「无输出多久」继续长。全部到终态
  // 之后读数已冻结，不再 tick。
  const running = progressRunningAny(run.progress);
  useEffect(() => {
    if (!running) return;
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), PROGRESS_TICK_MS);
    return () => clearInterval(timer);
  }, [running]);

  const beginRun = useCallback((runId: string) => setRun(beginProgressRun(runId)), []);
  const endRun = useCallback(() => setRun((current) => endProgressRun(current)), []);
  const snapshotOf = useCallback((nodeId: string) => progressRunOf(run, nodeId), [run]);

  return useMemo(
    () => ({ runId: run.runId, progress: run.progress, now, snapshotOf, beginRun, endRun }),
    [beginRun, endRun, now, run.progress, run.runId, snapshotOf],
  );
}
