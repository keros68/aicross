import type { Graph, GraphNode, Preset } from './types.ts';
import { cloneTarget, newNodeId } from './types.ts';

// 评审团（builtin-jury）各节点的视角文本，盖出来时写进节点 user.text；
// serializeContext 会把它拼在盲审冻结模板之后（TASK-05B，不加 Target 字段）。
// 第三条留空 = 「自定义」视角，由用户在节点面板的编辑框里填（placeholder 提示）。
// 擂台（builtin-arena）无固定文本，目标由用户在 G1 确认屏逐家选。
export const PRESET_NODE_TEXTS: Record<string, string[]> = {
  'builtin-jury': [
    '你是评审团的安全评审。只从安全视角审查这批材料：指出风险、暴露面与缓释建议。',
    '你是评审团的性能评审。只从性能视角审查这批材料：指出瓶颈、复杂度与改进建议。',
    '',
  ],
};

// 在节点 N 上展开 preset 为子图（不改原图，返回新图）。
// 第 1 阶段每个 Target 成为 N 的子节点（turn）；第 k 阶段每个 Target 以第 k-1 阶段
// 全部新节点为父节点，父节点数 ≥2 时 kind=accept，否则 turn。
// 同层节点 created_at 按阶段内索引递增 1ms，竖排顺序与预设书写顺序一致。
// PRESET_NODE_TEXTS 里的预设（评审团）按创建顺序把视角文本写进节点 user.text。
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
  const created: string[] = [];
  let prevLevel: string[] = [nodeId];
  for (const stage of preset.stages) {
    const level: string[] = [];
    stage.nodes.forEach((target, index) => {
      const id = newNodeId();
      nodes[id] = {
        id,
        kind: prevLevel.length >= 2 ? 'accept' : 'turn',
        parents: [...prevLevel],
        user: { text: target.preset_text ?? '', attachments: [] },
        reply: { status: 'draft', text: '' },
        target: cloneTarget(target),
        created_at: new Date(nowMs + index).toISOString(),
      };
      level.push(id);
      created.push(id);
    });
    prevLevel = level;
  }

  // 视角文本只认内置预设：用户预设 id 撞名（builtin-jury）不套用内置文本
  const texts = preset.builtin ? PRESET_NODE_TEXTS[preset.id] : undefined;
  if (texts) {
    created.forEach((id, index) => {
      const text = texts[index];
      if (text !== undefined) nodes[id].user.text = text;
    });
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
