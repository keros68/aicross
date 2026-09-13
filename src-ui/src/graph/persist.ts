// 图变更的统一出口：全局状态 + 落盘（fire-and-forget），以及
// 「节点由哪个预设盖出来」的运行期记忆（不进契约字段，重启后显示「手动」）。

import type { Graph } from './types.ts';
import { commands } from '../tauri';

type SetGraph = (action: { type: 'SET_GRAPH'; graph: Graph }) => void;

// 每次图变更走这里：刷新 updated_at → SET_GRAPH → save_graph 落盘。
export function commitGraph(dispatch: SetGraph, graph: Graph): Graph {
  const next: Graph = { ...graph, updated_at: new Date().toISOString() };
  dispatch({ type: 'SET_GRAPH', graph: next });
  commands.saveGraph(next).catch((err) => console.warn('[graph] 保存失败', err));
  return next;
}

const stampNames = new Map<string, string>(); // `${graphId}:${nodeId}` → 预设名

export function recordStamp(graphId: string, nodeIds: string[], presetName: string): void {
  for (const id of nodeIds) stampNames.set(`${graphId}:${id}`, presetName);
}

export function presetNameOf(graphId: string, nodeId: string): string | null {
  return stampNames.get(`${graphId}:${nodeId}`) ?? null;
}
