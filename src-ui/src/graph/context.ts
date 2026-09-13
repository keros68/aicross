import type { Graph, GraphNode, Message } from './types.ts';
import { childrenOf } from './stamp.ts';

// 全部祖先（沿所有 parents 递归）+ 自身的集合。悬空 parents 跳过，环路防御性截断。
function ancestorClosure(graph: Graph, nodeId: string): Set<string> {
  const closure = new Set<string>([nodeId]);
  const stack = [...(graph.nodes[nodeId]?.parents ?? [])];
  while (stack.length > 0) {
    const id = stack.pop() as string;
    if (closure.has(id)) continue;
    const node = graph.nodes[id];
    if (!node) continue; // 悬空 parents 跳过
    closure.add(id);
    stack.push(...node.parents);
  }
  return closure;
}

// 从根深度优先遍历，只保留在 keep 里的节点（keep 是祖先闭集，遇到不在 keep 里的
// 节点可直接剪枝）；多父节点会被多条路径到达，seen 保证只出现一次。
function orderByDfs(graph: Graph, keep: Set<string>): string[] {
  const out: string[] = [];
  const seen = new Set<string>();
  const visit = (id: string) => {
    if (!keep.has(id) || seen.has(id)) return;
    seen.add(id);
    out.push(id);
    for (const child of childrenOf(graph, id)) visit(child);
  };
  if (graph.nodes[graph.root]) visit(graph.root);
  return out;
}

function replyMessage(node: GraphNode): Message | undefined {
  const reply = node.reply;
  if (!reply) return undefined;
  if (reply.status === 'draft' || reply.status === 'pending') return undefined;
  if (reply.text === '') return undefined;
  return { text: reply.text, attachments: [] };
}

function hasUserContent(node: GraphNode): boolean {
  return node.user.text !== '' || node.user.attachments.length > 0;
}

// 上下文 = 该节点的全部祖先闭包（沿所有 parents 递归）+ 自身，祖先按从根出发
// DFS 保序去重、自身附加在末尾，按该节点 target.visibility 过滤。accept 节点
// 不再特殊算并集（闭包天然覆盖），但 accept 节点的直接父节点回复始终保留
// （验收要看产物）。节点没有 target（如根节点）时按共享处理。
export function contextOf(graph: Graph, nodeId: string): Message[] {
  const node = graph.nodes[nodeId];
  if (!node) return [];

  const closure = ancestorClosure(graph, nodeId);
  closure.delete(nodeId); // 自身附加在末尾，不参与 DFS 排序
  const pathIds = orderByDfs(graph, closure);
  pathIds.push(nodeId);

  const isAccept = node.kind === 'accept';
  const visibility = node.target?.visibility ?? '共享';
  const upstreams = new Set(node.target?.upstreams ?? []);
  const parents = new Set(node.parents);

  const out: Message[] = [];
  for (const id of pathIds) {
    const n = graph.nodes[id];
    if (!n) continue;
    if (hasUserContent(n)) {
      out.push({ text: n.user.text, attachments: [...n.user.attachments] });
    }
    const reply = replyMessage(n);
    if (!reply) continue;
    let keep: boolean;
    if (isAccept && parents.has(id)) {
      keep = true; // 父节点自身的回复始终保留（验收要看产物）
    } else if (visibility === '共享') {
      keep = true;
    } else if (visibility === '盲') {
      keep = false;
    } else {
      keep = upstreams.has(id);
    }
    if (keep) out.push(reply);
  }
  return out;
}
