// 界面侧的图变更纯函数：新建 turn 子节点、根节点填第一句。都不改原图。

import type { DispatchableModel, Graph, GraphNode, SwarmItem, Target } from './types.ts';
import { cloneTarget, defaultThinking, newNodeId, roleDefaults } from './types.ts';

// 「执行」角色的默认 Target（模型留空，由界面填）。
export function defaultTurnTarget(): Target {
  return {
    agent: '',
    model: '',
    role: '执行',
    thinking: defaultThinking(),
    ...roleDefaults('执行'),
  };
}

// 在 parentId 下新建 turn 子节点，reply=draft。target 缺省 = 执行默认值。
export function appendTurn(
  graph: Graph,
  parentId: string,
  text: string,
  target?: Target,
): { graph: Graph; nodeId: string } {
  const parent = graph.nodes[parentId];
  if (!parent) return { graph, nodeId: parentId };
  const now = new Date().toISOString();
  const id = newNodeId();
  const node: GraphNode = {
    id,
    kind: 'turn',
    parents: [parentId],
    user: { text, attachments: [] },
    reply: { status: 'draft', text: '' },
    target: target ? cloneTarget(target) : defaultTurnTarget(),
    created_at: now,
  };
  return { graph: { ...graph, nodes: { ...graph.nodes, [id]: node } }, nodeId: id };
}

// 根节点填入用户的第一句；空标题取首行前 20 字（默认名「对话图」由展示层兜底）。
export function fillRoot(graph: Graph, text: string): Graph {
  const root = graph.nodes[graph.root];
  if (!root) return graph;
  const title = graph.title === ''
    ? text.split('\n')[0].slice(0, 20) || graph.title
    : graph.title;
  return {
    ...graph,
    title,
    nodes: {
      ...graph.nodes,
      [graph.root]: { ...root, user: { text, attachments: [...root.user.attachments] } },
    },
  };
}

// 写回一个节点的 Target（节点面板逐字段编辑的出口）。
export function updateNodeTarget(graph: Graph, nodeId: string, target: Target): Graph {
  const node = graph.nodes[nodeId];
  if (!node) return graph;
  return {
    ...graph,
    nodes: { ...graph.nodes, [nodeId]: { ...node, target: cloneTarget(target) } },
  };
}

// 只写回节点的备选（G1 备选下拉用，R2-12）：不把本地改过但未确认的主目标
// 一起落盘；fallback 传 undefined 表示清空备选。
export function updateNodeFallback(graph: Graph, nodeId: string, fallback: Target['fallback']): Graph {
  const node = graph.nodes[nodeId];
  if (!node?.target) return graph;
  return updateNodeTarget(graph, nodeId, { ...node.target, fallback });
}

// 写回节点的蜂群清单条目（TASK-08：G1 判重叠后用户在节点面板改 files/accept）。
// 节点上没有 swarm 字段时不凭空补，返回原图。
export function updateNodeSwarm(graph: Graph, nodeId: string, swarm: SwarmItem): Graph {
  const node = graph.nodes[nodeId];
  if (!node?.swarm) return graph;
  return {
    ...graph,
    nodes: {
      ...graph.nodes,
      [nodeId]: { ...node, swarm: { ...swarm, files: [...swarm.files] } },
    },
  };
}

// 写回节点用户消息文本（节点面板编辑框；draft 可编辑、非 draft 只读由界面控制）。
export function updateNodeUserText(graph: Graph, nodeId: string, text: string): Graph {
  const node = graph.nodes[nodeId];
  if (!node) return graph;
  return {
    ...graph,
    nodes: { ...graph.nodes, [nodeId]: { ...node, user: { ...node.user, text } } },
  };
}

// 删除一个没有子节点的 draft 节点（擂台调路数用）。守卫：节点存在、是 draft、
// 没有任何节点以它为父；不满足返回原图。
export function removeDraftNode(graph: Graph, nodeId: string): Graph {
  const node = graph.nodes[nodeId];
  if (!node || node.reply?.status !== 'draft') return graph;
  if (Object.values(graph.nodes).some((other) => other.parents.includes(nodeId))) return graph;
  const nodes = { ...graph.nodes };
  delete nodes[nodeId];
  return { ...graph, nodes };
}

// 解析聊天输入框的发送内容：第一个 `@agent/模型`（或 `@模型`）提出来作为
// Target（唯一匹配不到就按原文手填，标不标「未接入」由 validateNode 决定），
// 其余文本是用户消息。
export function parseSend(
  raw: string,
  models: DispatchableModel[],
): { text: string; target?: Target } {
  const m = raw.match(/@([^\s@]+)/);
  const text = (m ? raw.replace(m[0], '') : raw).trim();
  if (!m) return { text };
  // `#provider` 后缀（修正轮 R1）：`@agent/模型` 表达不了 provider，pi 同名
  // 模型挂在多个 provider 下时，光靠 agent+id 会落到清单里的第一条，不一定是
  // 用户点的那一条。模型 id 里不会出现 `#`，切在最后一个 `#` 上。
  const hash = m[1].lastIndexOf('#');
  const ref = hash >= 0 ? m[1].slice(0, hash) : m[1];
  const wantProvider = hash >= 0 ? m[1].slice(hash + 1) : '';
  const slash = ref.indexOf('/');
  const matchesProvider = (mm: DispatchableModel) =>
    wantProvider === '' || mm.provider === wantProvider;
  let hit: DispatchableModel | undefined;
  if (slash >= 0) {
    const agent = ref.slice(0, slash);
    const model = ref.slice(slash + 1);
    hit = models.find((mm) => mm.agent === agent && mm.id === model && matchesProvider(mm));
  } else {
    const cands = models.filter(
      (mm) => (mm.id === ref || `${mm.agent}/${mm.id}` === ref) && matchesProvider(mm),
    );
    if (cands.length === 1) hit = cands[0];
  }
  if (hit) {
    return {
      text,
      target: {
        ...defaultTurnTarget(),
        agent: hit.agent,
        model: hit.id,
        provider: hit.provider !== '' ? hit.provider : undefined,
        tier: hit.tier !== '' ? hit.tier : undefined,
      },
    };
  }
  const target = defaultTurnTarget();
  if (slash >= 0) {
    target.agent = ref.slice(0, slash);
    target.model = ref.slice(slash + 1);
  } else {
    target.model = ref;
  }
  return { text, target };
}
