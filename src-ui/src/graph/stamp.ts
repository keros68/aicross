import type { Graph, GraphNode, Preset } from './types.ts';
import { cloneTarget, newNodeId } from './types.ts';

// 在节点 N 上展开 preset 为子图（不改原图，返回新图）。
// 第 1 阶段每个 Target 成为 N 的子节点（turn）；第 k 阶段每个 Target 以第 k-1 阶段
// 全部新节点为父节点，父节点数 ≥2 时 kind=accept，否则 turn。
// 同层节点 created_at 按阶段内索引递增 1ms，竖排顺序与预设书写顺序一致。
export function stamp(graph: Graph, nodeId: string, preset: Preset): Graph {
  const base = graph.nodes[nodeId];
  if (!base) return graph;

  const nodes: Record<string, GraphNode> = {};
  for (const [id, n] of Object.entries(graph.nodes)) {
    nodes[id] = {
      ...n,
      parents: [...n.parents],
      user: { text: n.user.text, attachments: [...n.user.attachments] },
      reply: n.reply ? { ...n.reply } : undefined,
      target: n.target ? cloneTarget(n.target) : undefined,
    };
  }

  const nowMs = Date.now();
  const now = new Date(nowMs).toISOString();
  let prevLevel: string[] = [nodeId];
  for (const stage of preset.stages) {
    const level: string[] = [];
    stage.nodes.forEach((target, index) => {
      const id = newNodeId();
      nodes[id] = {
        id,
        kind: prevLevel.length >= 2 ? 'accept' : 'turn',
        parents: [...prevLevel],
        user: { text: '', attachments: [] },
        reply: { status: 'draft', text: '' },
        target: cloneTarget(target),
        created_at: new Date(nowMs + index).toISOString(),
      };
      level.push(id);
    });
    prevLevel = level;
  }

  return { ...graph, nodes, updated_at: now };
}

export function childrenOf(graph: Graph, nodeId: string): string[] {
  const out: string[] = [];
  for (const id of Object.keys(graph.nodes)) {
    if (graph.nodes[id].parents.includes(nodeId)) out.push(id);
  }
  return out;
}

// 全部后代，DFS 前序，去重（accept 汇合处一个节点可能经多条路径到达）
export function descendantsOf(graph: Graph, nodeId: string): string[] {
  const out: string[] = [];
  const seen = new Set<string>([nodeId]);
  const visit = (id: string) => {
    for (const child of childrenOf(graph, id)) {
      if (seen.has(child)) continue;
      seen.add(child);
      out.push(child);
      visit(child);
    }
  };
  visit(nodeId);
  return out;
}
