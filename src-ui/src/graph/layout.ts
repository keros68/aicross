// 会话图界面布局的纯函数：深度分层、同层排序、根→节点路径、默认列末端。
// 与渲染无关，可单测。

import type { Graph, GraphNode } from './types.ts';

// 深度 = 到根的最长路径（父节点最大深度 + 1，根为 0）。accept 节点多父时取最大，
// 这样扇入节点排在最深的父分支右侧。环路防御：visiting 截断为 0。
export function depthMap(graph: Graph): Map<string, number> {
  const memo = new Map<string, number>();
  const visiting = new Set<string>();
  const depth = (id: string): number => {
    const cached = memo.get(id);
    if (cached !== undefined) return cached;
    if (visiting.has(id)) return 0;
    const node = graph.nodes[id];
    if (!node) return 0;
    visiting.add(id);
    let d = 0;
    for (const p of node.parents) {
      if (!graph.nodes[p]) continue; // 悬空 parents 跳过
      d = Math.max(d, depth(p) + 1);
    }
    visiting.delete(id);
    memo.set(id, d);
    return d;
  };
  for (const id of Object.keys(graph.nodes)) depth(id);
  return memo;
}

// 按深度分层，同层内按 created_at（再按 id）稳定排序，保证兄弟节点竖排顺序固定。
export function layersOf(graph: Graph): string[][] {
  const depths = depthMap(graph);
  const ids = Object.keys(graph.nodes);
  let maxDepth = 0;
  for (const id of ids) maxDepth = Math.max(maxDepth, depths.get(id) ?? 0);
  const layers: string[][] = Array.from({ length: maxDepth + 1 }, () => []);
  const sorted = [...ids].sort((a, b) => {
    const na = graph.nodes[a];
    const nb = graph.nodes[b];
    if (na.created_at !== nb.created_at) return na.created_at < nb.created_at ? -1 : 1;
    return a < b ? -1 : a > b ? 1 : 0;
  });
  for (const id of sorted) layers[depths.get(id) ?? 0].push(id);
  return layers;
}

// 根到节点的展示路径（含两端，根→叶顺序），沿 parents[0] 回溯；accept 节点的
// 展示路径走第一父。聊天列只显示一条链用；上下文计算见 context.ts（祖先闭包）。
// 悬空/环路防御性截断。
export function pathTo(graph: Graph, nodeId: string): string[] {
  const path: string[] = [];
  const seen = new Set<string>();
  let cur: string | undefined = nodeId;
  while (cur !== undefined) {
    if (seen.has(cur)) break;
    seen.add(cur);
    const node: GraphNode | undefined = graph.nodes[cur];
    if (!node) break;
    path.unshift(cur);
    cur = node.parents[0];
    if (cur !== undefined && !graph.nodes[cur]) break;
  }
  return path;
}

// 默认列末端：深度最大的节点，同深度取最新创建。图只有根时返回根。
export function latestLeaf(graph: Graph): string {
  const depths = depthMap(graph);
  let best = graph.root;
  let bestDepth = -1;
  for (const id of Object.keys(graph.nodes)) {
    const d = depths.get(id) ?? 0;
    if (d > bestDepth) {
      best = id;
      bestDepth = d;
    } else if (d === bestDepth && graph.nodes[id].created_at > graph.nodes[best].created_at) {
      best = id;
    }
  }
  return best;
}

// 全部祖先（BFS，去重，悬空 parents 跳过）——节点面板「指定上游」的候选列表。
export function ancestorsOf(graph: Graph, nodeId: string): string[] {
  const out: string[] = [];
  const seen = new Set<string>([nodeId]);
  const queue = [...(graph.nodes[nodeId]?.parents ?? [])];
  while (queue.length > 0) {
    const id = queue.shift() as string;
    if (seen.has(id)) continue;
    seen.add(id);
    const node = graph.nodes[id];
    if (!node) continue;
    out.push(id);
    queue.push(...node.parents);
  }
  return out;
}

// 两节点是否在同一条祖先链上（一方是另一方的祖先，或相同）。
// 「同一分支只保留一个面板」的判定（C1）：同链即同分支。
export function sameBranch(graph: Graph, a: string, b: string): boolean {
  if (a === b) return true;
  return ancestorsOf(graph, a).includes(b) || ancestorsOf(graph, b).includes(a);
}

// 加载时校验 kind 与 parents 数一致（脏数据防御）：根节点 material 不动，
// 非根节点 parents ≥ 2 应为 accept、否则 turn；不一致按 parents 数修正并记
// console.warn。不改原图；无需修正时返回原图。
export function normalizeNodeKinds(graph: Graph): Graph {
  let fixed: { id: string; from: string; to: string }[] = [];
  let nodes = graph.nodes;
  for (const [id, node] of Object.entries(graph.nodes)) {
    if (id === graph.root) continue;
    const expected = node.parents.length >= 2 ? 'accept' : 'turn';
    if (node.kind === expected) continue;
    if (nodes === graph.nodes) nodes = { ...graph.nodes };
    nodes[id] = { ...node, kind: expected };
    fixed.push({ id, from: node.kind, to: expected });
  }
  if (fixed.length > 0) {
    console.warn(
      `[graph] kind 与 parents 数不一致，已按 parents 数修正：${fixed
        .map((f) => `${f.id} ${f.from}→${f.to}`)
        .join('、')}`,
    );
    return { ...graph, nodes };
  }
  return graph;
}
