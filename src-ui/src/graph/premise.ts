// 前提抽查（TASK-05B）：台账条目 → 抽查图的构建与结果回读，都是纯函数。
// 台账持久化在 Rust 侧（~/.aicross/premises.json，orchestration.rs），这里不管 IO。

import type { Graph, Target } from './types.ts';
import { newGraph } from './types.ts';
import { appendTurn, fillRoot } from './mutate.ts';
import premiseCheckPrompt from '../../../fixtures/prompts/premise-check.txt?raw';

export const PREMISE_CHECK_PROMPT = premiseCheckPrompt.trim();

// 镜像 Rust orchestration.rs::Premise（snake_case 契约）
export interface Premise {
  id: string;
  text: string;
  recorded_at: string;
  last_checked_at?: string | null;
  last_result?: string | null;
  attachments: string[];
  last_graph_id?: string | null;
}

export function premiseGraphTitle(premise: Pick<Premise, 'id' | 'text'>): string {
  return `前提抽查 #${premise.id.slice(0, 8)}：${premise.text.replace(/\s+/g, ' ').slice(0, 16)}`;
}

// 抽查图：根材料 = 前提原文 + 台账勾选的附件；一个盲验证节点，user.text = 冻结模板
// （serializeContext 把它拼在盲审前缀之后）。target 由设置页选好
// （非项目常用厂商），留空则到 G1 确认屏再选。
export function buildPremiseCheckGraph(
  premise: Pick<Premise, 'id' | 'text'> & { attachments?: string[] },
  target?: Partial<Target>,
): Graph {
  let graph = newGraph();
  graph = fillRoot(graph, premise.text);
  const root = graph.nodes[graph.root];
  graph = {
    ...graph,
    title: premiseGraphTitle(premise),
    nodes: {
      ...graph.nodes,
      [graph.root]: { ...root, user: { ...root.user, attachments: [...(premise.attachments ?? [])] } },
    },
  };
  const check: Target = {
    agent: target?.agent ?? '',
    provider: target?.provider,
    model: target?.model ?? '',
    tier: target?.tier,
    thinking: target?.thinking ?? '中',
    role: '验证',
    visibility: '盲',
    write: '只读',
  };
  return appendTurn(graph, graph.root, PREMISE_CHECK_PROMPT, check).graph;
}

// R2-2：前提附件保存前校验。相对路径拒绝（派发时按绝对路径读文件，相对路径
// 会让抽查节点整体失败）；同名 basename 拒绝（盲分支按文件名拷进隔离目录，
// 重名会被拒）。返回错误文案，通过返回 null。
export function validatePremiseAttachments(paths: string[]): string | null {
  for (const path of paths) {
    // Windows 盘符 / UNC（\\）/ POSIX 根 都算绝对路径
    if (!/^(?:[A-Za-z]:[\\/]|\\\\|\/)/.test(path)) {
      return `附件需用绝对路径：${path}`;
    }
  }
  const seen = new Set<string>();
  for (const path of paths) {
    const base = path.split(/[\\/]/).pop() ?? path;
    if (seen.has(base)) return `附件文件名重复：${base}`;
    seen.add(base);
  }
  return null;
}

// 手输附件按「每行一条」拆（TASK-06-5）：路径里可以含逗号、空格。
// 首尾空白去掉，空行丢掉，重复保留给 validatePremiseAttachments 判。
export function parseAttachmentLines(raw: string): string[] {
  return raw.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
}

// 附件手输框（TASK-06 R1-13）：框是受控的，草稿与已存列表不一致时界面标
// 「未保存」——校验没通过时框里还留着那几行路径，不标一下会被当成已经生效，
// 而「抽查」用的是已存列表。
export function attachmentDraftState(
  raw: string,
  saved: string[],
): { attachments: string[]; unsaved: boolean } {
  const attachments = parseAttachmentLines(raw);
  return { attachments, unsaved: attachments.join('\n') !== saved.join('\n') };
}

// 「抽查」按钮按下时再校验一次（TASK-06-5）：台账文件被手改过时，设置页保存
// 那道校验拦不住；不通过就不建图，返回提示文案。
export function premiseCheckBlockReason(premise: Pick<Premise, 'attachments'>): string | null {
  const invalid = validatePremiseAttachments(premise.attachments ?? []);
  return invalid ? `附件不合规，未建图：${invalid}` : null;
}

// 回读抽查结论：图里 role=验证 且回复已完成的节点正文；没跑完返回 null。
export function premiseCheckOutcome(graph: Graph): string | null {
  const node = Object.values(graph.nodes).find(
    (n) => n.target?.role === '验证' && n.reply?.status === 'done' && n.reply.text !== '',
  );
  return node?.reply?.text ?? null;
}

// 设置页回写：按 last_graph_id 读图（不按标题找，同名图各归各），把跑完的验证
// 节点结论写回台账。结论为空不覆盖；与已存结论相同（按截断 500 字比较）不重写；
// last_checked_at 用回写时刻，建图时不写。
export async function matchPremiseResults(
  premises: Premise[],
  loadGraph: (id: string) => Promise<Graph | null>,
): Promise<{ premises: Premise[]; changed: boolean }> {
  let changed = false;
  const next = [...premises];
  for (let i = 0; i < next.length; i++) {
    const premise = next[i];
    if (!premise.last_graph_id) continue;
    const graph = await loadGraph(premise.last_graph_id).catch(() => null);
    if (!graph) continue;
    const outcome = premiseCheckOutcome(graph);
    if (!outcome) continue;
    const sliced = outcome.slice(0, 500);
    if (sliced === premise.last_result) continue;
    next[i] = { ...premise, last_result: sliced, last_checked_at: new Date().toISOString() };
    changed = true;
  }
  return { premises: next, changed };
}
