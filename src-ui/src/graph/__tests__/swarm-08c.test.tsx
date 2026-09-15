// TASK-08C 蜂群联调收口：编号与任务书的 1–8 条一致。
// 08A/08B 并入 main 之后的对账项——拆分提示词单一来源、合并入口按图派生、
// 跨拆分节点续号、G1 首帧与空目录、只发 camelCase、已并入件的保护、
// 每件上下文只带根材料、前端只读角色集合。

// tsconfig.app.json 的 types 里只有 vite/client：应用代码不该碰 node 内置模块。
// 只有读源码/冻结文件断言的那几条要读磁盘，就地压掉解析错误（同 08B 的写法）。
// @ts-expect-error node builtin, not in this project's `types`
import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import {
  appendSwarmNodes,
  applySwarmMerge,
  BUILTIN_SWARM_PRESET,
  swarmCardActions,
  swarmCheckInput,
  swarmGate,
  swarmItemBase,
  swarmLimitLine,
  swarmMergeEntry,
  swarmPendingMergeStage,
  swarmMergeItems,
  swarmRecordedResult,
  swarmStageOf,
  swarmWireSettings,
  SWARM_LIMIT_DEFAULT,
  SWARM_NO_PROJECT_TEXT,
  SWARM_SPLIT_PROMPT,
  type SwarmCheckState,
} from '../swarm.ts';
import { PRESET_NODE_TEXTS, stamp } from '../stamp.ts';
import { pickOne, setReplyAdopted } from '../dispatch.ts';
import { serializeContext } from '../context.ts';
import { READONLY_ROLES, validateNode } from '../validate.ts';
import { newGraph } from '../types.ts';
import type { Graph, GraphNode, Preset, Target } from '../types.ts';
import type { SwarmMergeResult } from '../../tauri.ts';
import { DEFAULT_DISPATCH_SETTINGS } from '../dispatch-settings.ts';
import { SwarmGateNotice } from '../../components/center/graph/DispatchBits.tsx';

const src = (path: string) => readFileSync(new URL(path, import.meta.url), 'utf-8') as string;

const mkTarget = (patch: Partial<Target> = {}): Target => ({
  agent: 'codex', model: 'gpt-5', role: '执行', thinking: '中', visibility: '共享', write: '只读', ...patch,
});

function mkGraph(nodes: GraphNode[]): Graph {
  const root: GraphNode = {
    id: 'root', kind: 'material', parents: [],
    user: { text: '根材料：把解析器改一改', attachments: [] },
    created_at: '2026-09-15T09:00:00Z',
  };
  return {
    version: 1, id: 'g', title: 't', root: 'root',
    nodes: Object.fromEntries([root, ...nodes].map((node) => [node.id, node])),
    created_at: '2026-09-15T09:00:00Z', updated_at: '2026-09-15T09:00:00Z',
  };
}

function mkSplit(id: string): GraphNode {
  return {
    id, kind: 'turn', parents: ['root'],
    user: { text: SWARM_SPLIT_PROMPT, attachments: [] },
    reply: { status: 'done', text: '清单在此' },
    target: mkTarget({ role: '拆分', write: '只读' }),
    created_at: '2026-09-15T10:00:00Z',
  };
}

function mkItem(id: string, parent: string, item: number, patch: Partial<GraphNode> = {}): GraphNode {
  return {
    id, kind: 'turn', parents: [parent],
    user: { text: `第${item}件标题\n文件范围：src/${item}/**`, attachments: [] },
    reply: {
      status: 'done',
      text: '做完了',
      artifacts: { diff: '', changed_files: [], worktree: `D:/wt/${item}`, stdout_summary: '' },
    },
    target: mkTarget({ write: 'worktree', visibility: '指定上游', upstreams: ['root'] }),
    swarm: { item, files: [`src/${item}/**`], accept: `pytest tests/test_${item}.py` },
    created_at: '2026-09-15T11:00:00Z',
    ...patch,
  };
}

const mergeRow = (patch: Partial<SwarmMergeResult>): SwarmMergeResult => ({
  nodeId: 'a', item: 1, result: 'merged', commit: '', note: '', testOutput: '', ...patch,
});

describe('TASK-08C 蜂群联调收口', () => {
  it('08C-1 拆分提示词只有一份来源：盖章消费引擎的 preset_text，前端不另写文本', () => {
    // 唯一来源是冻结文件，前端拿的就是它
    expect(SWARM_SPLIT_PROMPT).toBe(src('../../../../fixtures/prompts/split.txt').trim());
    expect(PRESET_NODE_TEXTS['builtin-swarm']).toBeUndefined();
    expect(src('../stamp.ts')).not.toContain('swarmSplitText');

    // 引擎清单里的那份（preset_text 由 Rust 读 split.txt 填）盖出来就是它加一行上限
    const engine: Preset = {
      id: 'builtin-swarm', name: '蜂群模式', builtin: true,
      stages: [{ nodes: [mkTarget({ agent: '', model: '', role: '拆分', visibility: '共享', preset_text: '引擎给的拆分要求' })] }],
    };
    const graph = newGraph();
    const stamped = stamp(graph, graph.root, engine, 3);
    const created = Object.values(stamped.nodes).filter((node) => node.id !== graph.root);
    expect(created[0].user.text).toBe(`引擎给的拆分要求\n\n${swarmLimitLine(3)}`);

    // 兜底预设（引擎清单里没有 builtin-swarm）用同一份冻结文件，不重复维护文本
    expect(BUILTIN_SWARM_PRESET.stages[0].nodes[0].preset_text).toBe(SWARM_SPLIT_PROMPT);
    const fallback = stamp(graph, graph.root, BUILTIN_SWARM_PRESET, SWARM_LIMIT_DEFAULT);
    const fallbackNode = Object.values(fallback.nodes).filter((node) => node.id !== graph.root)[0];
    expect(fallbackNode.user.text).toBe(`${SWARM_SPLIT_PROMPT}\n\n${swarmLimitLine()}`);
    // 非拆分节点不补这行
    const jury: Preset = { id: 'p', name: 'p', builtin: false, stages: [{ nodes: [mkTarget({ preset_text: '照常' })] }] };
    const other = stamp(graph, graph.root, jury, 3);
    expect(Object.values(other.nodes).filter((node) => node.id !== graph.root)[0].user.text).toBe('照常');
  });

  it('08C-2 合并入口按图派生：可并件 / 已并件 / 待裁决件都从图上算', () => {
    const graph = mkGraph([
      mkSplit('split'),
      mkItem('n1', 'split', 1),
      mkItem('n2', 'split', 2),
      mkItem('n3', 'split', 3),
    ]);
    // 件在拆分节点下一层
    expect(swarmStageOf(graph, ['root', 'split', 'n2'])).toBe(2);
    expect(swarmStageOf(graph, ['root', 'split'])).toBeNull();

    const after = applySwarmMerge(graph, [
      mergeRow({ nodeId: 'n1', item: 1, commit: 'aaa' }),
      mergeRow({ nodeId: 'n2', item: 2, result: 'conflict', note: 'D:/wt/2' }),
    ]);
    const entry = swarmMergeEntry(after, 2);
    expect(entry.nodeIds).toEqual(['n1', 'n2', 'n3']);
    expect(entry.ready).toBe(true);
    expect(entry.mergedIds).toEqual(['n1']);
    expect(entry.disputedIds).toEqual(['n2']);
    expect(entry.pendingIds).toEqual(['n2', 'n3']);
    expect(entry.rows.map((row) => row.state)).toEqual(['merged', 'conflict', 'pending']);
    expect(entry.rows[0].line).toBe('第 1 件：已并入 aaa');
    expect(entry.rows[2].line).toBe('第 3 件：待合并');
    expect(swarmRecordedResult(after.nodes.n3)).toBe('pending');

    // 单件重派之后（那件回 draft、本次 results 只有它）清单仍是全部三件
    const redrafted: Graph = {
      ...after,
      nodes: { ...after.nodes, n3: { ...after.nodes.n3, reply: { status: 'draft', text: '' } } },
    };
    const reopened = swarmMergeEntry(redrafted, 2);
    expect(reopened.nodeIds).toEqual(['n1', 'n2', 'n3']);
    expect(reopened.ready).toBe(false);
    expect(swarmMergeItems(redrafted, reopened.nodeIds).map((row) => row.item)).toEqual([2, 3]);
    // 别的阶段没有件
    expect(swarmMergeEntry(after, 1).nodeIds).toEqual([]);
  });

  it('08E 工具条「合并」入口：件全到终态且有未并入的才给，重派中或全并入则不给', () => {
    const graph = mkGraph([mkSplit('split'), mkItem('n1', 'split', 1), mkItem('n2', 'split', 2)]);
    expect(swarmPendingMergeStage(graph)).toBe(2);
    const partly = applySwarmMerge(graph, [
      mergeRow({ nodeId: 'n1', item: 1, commit: 'aaa' }),
      mergeRow({ nodeId: 'n2', item: 2, result: 'conflict', note: 'D:/wt/2' }),
    ]);
    expect(swarmPendingMergeStage(partly)).toBe(2);
    const all = applySwarmMerge(graph, [
      mergeRow({ nodeId: 'n1', item: 1, commit: 'aaa' }),
      mergeRow({ nodeId: 'n2', item: 2, commit: 'bbb' }),
    ]);
    expect(swarmPendingMergeStage(all)).toBeNull();
    const redrafted: Graph = { ...partly, nodes: { ...partly.nodes, n2: { ...partly.nodes.n2, reply: { status: 'draft', text: '' } } } };
    expect(swarmPendingMergeStage(redrafted)).toBeNull();
    expect(swarmPendingMergeStage(newGraph())).toBeNull();
  });

  it('08C-3 跨拆分节点件号续号：第二个拆分从最大件号 +1 起编，重复生成仍幂等', () => {
    const base = mkGraph([mkSplit('s1'), mkItem('n1', 's1', 1), mkItem('n2', 's1', 2), mkSplit('s2')]);
    expect(swarmItemBase(base, 's2')).toBe(2);
    expect(swarmItemBase(base, 's1')).toBe(0);

    const items = [1, 2].map((n) => ({ title: `乙${n}`, files: [`docs/${n}/**`], accept: '' }));
    const first = appendSwarmNodes(base, 's2', items, undefined, 4);
    expect(first.nodeIds.map((id) => first.graph.nodes[id].swarm!.item)).toEqual([3, 4]);
    // 再点一次不重复建（件号锚在自己那批的起点上）
    const again = appendSwarmNodes(first.graph, 's2', items, undefined, 4);
    expect(again.nodeIds).toEqual([]);
    expect(again.skipped).toEqual([3, 4]);
    // 合并排序按件号，两批不交错
    const entry = swarmMergeEntry(first.graph, 2);
    expect(entry.rows.map((row) => row.item)).toEqual([1, 2, 3, 4]);
  });

  it('08C-4 G1 首帧与空目录：没选工作目录时阻断并说原因，首帧按 pending 处理', () => {
    const pending: SwarmCheckState = { phase: 'pending' };
    // 没有蜂群件：不核对也不拦
    expect(swarmCheckInput(0, '', pending)).toEqual({ phase: 'idle' });
    expect(swarmGate(swarmCheckInput(0, '', pending)).blocked).toBe(false);
    // 有蜂群件但没选工作目录：命令发不出去，直接阻断
    expect(swarmCheckInput(2, '   ', pending)).toEqual({ phase: 'noProject' });
    const gate = swarmGate(swarmCheckInput(2, '', pending));
    expect(gate.blocked).toBe(true);
    expect(gate.reasons).toEqual([SWARM_NO_PROJECT_TEXT]);
    expect(gate.canProceed).toBe(false);
    expect(renderToStaticMarkup(<SwarmGateNotice gate={gate} />)).toContain(SWARM_NO_PROJECT_TEXT);
    // 有工作目录时照命令状态走；首帧（effect 还没跑）的初值就是 pending，不留可点窗口
    expect(swarmCheckInput(2, 'D:/p', pending)).toBe(pending);
    expect(swarmGate(swarmCheckInput(2, 'D:/p', pending)).blocked).toBe(true);
    expect(src('../../components/center/graph/DispatchOverlay.tsx'))
      .toContain("useState<SwarmCheckState>({ phase: 'pending' })");
  });

  it('08C-5 合并与核对只发 camelCase，且都带 settings', () => {
    const graph = mkGraph([mkSplit('split'), mkItem('n1', 'split', 1)]);
    const sent = swarmMergeItems(graph, ['n1'])[0];
    expect(Object.keys(sent).sort()).toEqual(['accept', 'files', 'item', 'nodeId', 'title', 'worktree']);
    expect(sent.title).toBe('第1件标题');
    expect(swarmWireSettings({ ...DEFAULT_DISPATCH_SETTINGS, swarmLimit: 2 })).toEqual({
      timeoutSecs: 600, concurrency: 3, maxRequests: 8, redact: true, writeProjectDispatch: true,
      ccSwitchPath: '', validationCommand: '', acpEnabled: false, swarmLimit: 2,
    });
    // 界面设置里的 classificationModel 不发
    expect(swarmWireSettings(DEFAULT_DISPATCH_SETTINGS)).not.toHaveProperty('classificationModel');
    // 双写与死常量、过期注释都清掉了
    const wire = src('../../tauri.ts');
    expect(wire).not.toContain('node_id?: string');
    expect(wire).toContain("invoke<SwarmPlanCheck>('check_swarm_plan', { projectDir, items, settings })");
    expect(wire).toContain("{ projectDir, runId, items, validateCommand, settings }");
    expect(src('../swarm.ts')).not.toContain('SWARM_CHECK_FALLBACK');
    expect(wire).not.toContain('SWARM_CHECK_FALLBACK');
  });

  it('08C-6 已并入的件不给「取消采纳」，兄弟卡的「选它」也清不掉它的采纳', () => {
    // m1 是同一个拆分底下手动加的普通执行节点（没有 swarm 字段），它的卡片有「选它」
    const sibling = { ...mkItem('m1', 'split', 9), swarm: undefined };
    const graph = mkGraph([mkSplit('split'), mkItem('n1', 'split', 1), mkItem('n2', 'split', 2), sibling]);
    const merged = applySwarmMerge(graph, [mergeRow({ nodeId: 'n1', item: 1, commit: 'aaa' })]);
    expect(merged.nodes.n1.reply!.adopted).toBe(true);
    const actions = swarmCardActions(merged.nodes.n1);
    expect(actions).toEqual({ showPickOne: false, showAdopt: false, showRetry: false, note: '已并入' });
    // 图上留痕认得出已并入：不带本次结果、adopted 被手动清掉也不给重派
    const cleared = setReplyAdopted(merged, 'n1', false);
    expect(swarmCardActions(cleared.nodes.n1)).toMatchObject({ showRetry: false, note: '已并入' });
    // 同父的普通兄弟按「选它」不清已并入件的 adopted，已并入的件也不会重回待合并清单
    const picked = pickOne(merged, 'm1');
    expect(picked.nodes.m1.reply!.adopted).toBe(true);
    expect(picked.nodes.n1.reply!.adopted).toBe(true);
    expect(swarmMergeItems(picked, ['n1', 'n2']).map((row) => row.item)).toEqual([2]);
  });

  it('08C-7 每件上下文只带根材料，不带拆分节点的用户消息', () => {
    const base = mkGraph([mkSplit('split')]);
    const items = [
      { title: '改解析器', files: ['src/a/**'], accept: 'pytest a' },
      { title: '补覆盖率', files: ['tests/**'], accept: 'pytest b' },
    ];
    const outcome = appendSwarmNodes(base, 'split', items, undefined, 4);
    const first = outcome.graph.nodes[outcome.nodeIds[0]];
    expect(first.target!.visibility).toBe('指定上游');
    expect(first.target!.upstreams).toEqual(['root']);
    const { prompt } = serializeContext(outcome.graph, first.id);
    expect(prompt).toContain('根材料：把解析器改一改');
    expect(prompt).toContain('改解析器');
    // 拆分要求（拆分节点的用户消息）与拆分回复、别的件都不进来
    expect(prompt).not.toContain(SWARM_SPLIT_PROMPT);
    expect(prompt).not.toContain('清单在此');
    expect(prompt).not.toContain('补覆盖率');
  });

  it('08C-8 前端只读角色集合含「拆分」，与 NodePanel 和引擎护栏同口径', () => {
    expect(READONLY_ROLES).toEqual(['验收', '验证', '仲裁', '拆分']);
    const graph = mkGraph([
      { ...mkSplit('split'), target: mkTarget({ role: '拆分', write: 'worktree' }) },
    ]);
    expect(validateNode(graph, 'split', [])).toContain('拆分节点只能只读');
    const readonly: Graph = {
      ...graph,
      nodes: { ...graph.nodes, split: { ...graph.nodes.split, target: mkTarget({ role: '拆分', write: '只读' }) } },
    };
    expect(validateNode(readonly, 'split', [])).not.toContain('拆分节点只能只读');
    // 与节点面板的角色名单、引擎的只读护栏对齐
    const panel = src('../../components/right/orchestration/NodePanel.tsx');
    for (const role of READONLY_ROLES) expect(panel).toContain(`'${role}'`);
    expect(src('../../../../src/dispatch.rs'))
      .toContain('matches!(node.role.as_str(), "验收" | "验证" | "仲裁" | "拆分")');
  });
});
