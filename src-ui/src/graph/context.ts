import type { Graph, GraphNode, Message, ReplyArtifacts } from './types.ts';
import { childrenOf } from './stamp.ts';
import blindPromptPrefix from '../../../fixtures/prompts/blind-prefix.txt?raw';

export interface ContextPart {
  key: string;
  nodeId: string;
  source: 'user' | 'reply';
  label: string;
  message: Message;
  defaultSelected?: boolean;
}

export interface ContextSelection {
  messageKeys: string[];
  attachments: string[];
}

export const BLIND_PROMPT_PREFIX = blindPromptPrefix.trim();

export function serializeCurrentMessage(message: Message): string {
  const blocks: string[] = [];
  if (message.text !== '') blocks.push(`[用户]\n${message.text}`);
  if (message.attachments.length > 0) {
    blocks.push(`[附件]\n${message.attachments.join('\n')}`);
  }
  return blocks.join('\n\n');
}

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
  if (reply.status !== 'done') return undefined;
  if (reply.text === '') return undefined;
  const artifact = reply.artifacts ? artifactSummary(reply.artifacts) : '';
  return { text: [reply.text, artifact].filter(Boolean).join('\n\n'), attachments: [] };
}

export function artifactSummary(artifacts: ReplyArtifacts): string {
  const files = artifacts.changed_files.map((file) => file.path).join('、') || '无改后全文';
  const worktree = artifacts.worktree ? `\nworktree: ${artifacts.worktree}` : '';
  return `[执行产物]\n改动文件：${files}${worktree}\nstdout 摘要：${artifacts.stdout_summary || '—'}\n\n${artifacts.diff}`;
}

function hasUserContent(node: GraphNode): boolean {
  return node.user.text !== '' || node.user.attachments.length > 0;
}

// 上下文 = 该节点的全部祖先闭包（沿所有 parents 递归）+ 自身，祖先按从根出发
// DFS 保序去重、自身附加在末尾，按该节点 target.visibility 过滤。
// 盲节点把被隔离的上游回复作为默认不勾选候选项产出；直接父回复有
// artifacts 时由目录交付、不进 prompt，无 artifacts 时则回退到 prompt。
// 节点没有 target（如根节点）时按共享处理。
export function contextPartsOf(graph: Graph, nodeId: string): ContextPart[] {
  const node = graph.nodes[nodeId];
  if (!node) return [];

  const closure = ancestorClosure(graph, nodeId);
  closure.delete(nodeId); // 自身附加在末尾，不参与 DFS 排序
  const pathIds = orderByDfs(graph, closure);
  pathIds.push(nodeId);

  const visibility = node.target?.visibility ?? '共享';
  const upstreams = new Set(node.target?.upstreams ?? []);
  const parents = new Set(node.parents);

  const out: ContextPart[] = [];
  for (const id of pathIds) {
    const n = graph.nodes[id];
    if (!n) continue;
    if (hasUserContent(n)) {
      out.push({
        key: `${id}:user`,
        nodeId: id,
        source: 'user',
        label: '用户',
        message: { text: n.user.text, attachments: [...n.user.attachments] },
      });
    }
    const reply = replyMessage(n);
    if (!reply) continue;
    if (visibility === '盲' && parents.has(id) && n.reply?.artifacts) {
      continue; // 盲验收的结构化产物走目录，不把执行正文或 diff 塞进 prompt。
    }
    let keep: boolean;
    let defaultSelected = true;
    if (visibility === '盲' && parents.has(id)) {
      keep = true; // 无 artifacts 时的直接父回复是唯一可验材料。
    } else if (visibility === '共享') {
      keep = true;
    } else if (visibility === '盲') {
      keep = true;
      defaultSelected = false;
    } else {
      keep = upstreams.has(id);
    }
    if (keep) {
      const target = n.target;
      const route = target
        ? [target.agent, target.model].filter(Boolean).join('/') || '模型'
        : '模型';
      out.push({
        key: `${id}:reply`,
        nodeId: id,
        source: 'reply',
        label: `模型回复 ${route}`,
        message: reply,
        defaultSelected,
      });
    }
  }
  return out;
}

export function contextOf(graph: Graph, nodeId: string): Message[] {
  return contextPartsOf(graph, nodeId).filter((part) => part.defaultSelected !== false).map((part) => ({
    text: part.message.text,
    attachments: [...part.message.attachments],
  }));
}

export function originalRequestOf(graph: Graph): string {
  return graph.nodes[graph.root]?.user.text ?? '';
}

export function upstreamArtifactsOf(graph: Graph, nodeId: string): ReplyArtifacts[] {
  const node = graph.nodes[nodeId];
  if (!node) return [];
  const visibility = node.target?.visibility ?? '共享';
  const allowed = visibility === '盲'
    ? new Set(node.parents)
    : visibility === '指定上游'
      ? new Set(node.target?.upstreams ?? [])
      : ancestorClosure(graph, nodeId);
  const out: ReplyArtifacts[] = [];
  for (const id of orderByDfs(graph, allowed)) {
    const artifacts = graph.nodes[id]?.reply?.artifacts;
    if (artifacts) out.push(artifacts);
  }
  // Direct parents may not be reachable from graph.root in a damaged/recovered graph.
  for (const id of allowed) {
    const artifacts = graph.nodes[id]?.reply?.artifacts;
    if (artifacts && !out.includes(artifacts)) out.push(artifacts);
  }
  return out;
}

// contextOf 的稳定文本形式。组件只负责勾选，不自行拼 prompt；共享节点按
// 角色标记每段，盲节点在同一份已过滤上下文前套冻结模板。
export function serializeContext(
  graph: Graph,
  nodeId: string,
  selection?: ContextSelection,
): { prompt: string; materials: string[]; parts: ContextPart[] } {
  const all = contextPartsOf(graph, nodeId);
  const selectedMessages = selection ? new Set(selection.messageKeys) : null;
  const selectedAttachments = selection ? new Set(selection.attachments) : null;
  const parts = all
    .map((part) => ({
      ...part,
      message: {
        text: (selectedMessages === null ? part.defaultSelected !== false : selectedMessages.has(part.key))
          ? part.message.text
          : '',
        attachments: part.message.attachments.filter(
          (path) => selectedAttachments === null || selectedAttachments.has(path),
        ),
      },
    }))
    .filter((part) => part.message.text !== '' || part.message.attachments.length > 0);

  const visibility = graph.nodes[nodeId]?.target?.visibility ?? '共享';
  const blocks: string[] = [];
  for (const part of parts) {
    if (part.message.text !== '') blocks.push(`[${part.label}]\n${part.message.text}`);
    if (part.message.attachments.length > 0) {
      const paths = visibility === '盲'
        ? part.message.attachments.map((path) => path.split(/[\\/]/).pop() || path)
        : part.message.attachments;
      blocks.push(`[附件]\n${paths.join('\n')}`);
    }
  }
  const prompt = visibility === '盲'
    ? [BLIND_PROMPT_PREFIX, ...blocks].join('\n\n')
    : blocks.join('\n\n');
  const materials = parts.flatMap((part) => part.message.attachments);
  return { prompt, materials: [...new Set(materials)], parts };
}
