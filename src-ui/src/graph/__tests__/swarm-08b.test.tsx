// TASK-08 B 节（蜂群 · 界面）：契约字段、内置预设、清单解析与建节点、
// G1 重叠闸门、G3 逐件合并、手册一节。用例编号与任务书 B1–B6 一致。

// tsconfig.app.json 的 types 里只有 vite/client：应用代码不该碰 node 内置模块。
// 只有读手册断言的那一条要读磁盘，就地压掉解析错误（同 07B/07C 的写法）。
// @ts-expect-error node builtin, not in this project's `types`
import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import {
  appendSwarmNodes,
  applySwarmMerge,
  arbitrationCandidates,
  BUILTIN_SWARM_PRESET,
  normalizeSwarmMergeResults,
  parseSwarmPlan,
  swarmCardActions,
  swarmChildItems,
  swarmCheckItems,
  swarmLimitLine,
  swarmWireSettings,
  SWARM_SPLIT_PROMPT,
  swarmGate,
  swarmGenerateNotice,
  swarmMergeErrorResults,
  swarmMergeFallback,
  swarmMergeItems,
  swarmMergeReady,
  swarmMergeSummary,
  swarmNodeText,
  swarmOverlapItems,
  swarmPendingMergeIds,
  swarmPlanNotice,
  swarmReplyNote,
  swarmResultLine,
  withSwarmPreset,
  SWARM_BACKEND_MISSING_NOTE,
  SWARM_CHECK_FAILED_TEXT,
  SWARM_CHECKING_TEXT,
  SWARM_DIRTY_TEXT,
  SWARM_LIMIT_DEFAULT,
  SWARM_MERGE_EMPTY_TEXT,
  SWARM_MERGE_MARK,
  swarmNodeIdsOf,
  SWARM_PROCEED_TEXT,
  type SwarmCheckState,
} from '../swarm.ts';
import { PRESET_NODE_TEXTS, stamp } from '../stamp.ts';
import { contextPartsOf, serializeContext } from '../context.ts';
import { applyRoleDefaults, presetWithRoleDefaults } from '../dispatch.ts';
import { updateNodeSwarm } from '../mutate.ts';
import { newGraph, roleDefaults } from '../types.ts';
import type { Graph, GraphNode, Preset, SwarmItem } from '../types.ts';
import type { SwarmMergeResult, SwarmPlanCheck } from '../../tauri.ts';
import { commands } from '../../tauri.ts';
import { DEFAULT_DISPATCH_SETTINGS, loadDispatchSettings } from '../dispatch-settings.ts';
import { SwarmGateNotice, SwarmMergeRow } from '../../components/center/graph/DispatchBits.tsx';
import { SwarmEditor } from '../../components/right/orchestration/NodePanel.tsx';

const ROLE_PROFILES = {
  planning: { agent: 'claude', provider: '', model: 'opus', tier: '高', thinking: '高' },
  execution: { agent: 'codex', provider: '', model: 'gpt-5', tier: '中', thinking: '中' },
  acceptance: { agent: 'kimi', provider: '', model: 'k2', tier: '低', thinking: '低' },
};

function mkSwarmNode(id: string, item: number, patch: Partial<GraphNode> = {}): GraphNode {
  return {
    id,
    kind: 'turn',
    parents: ['root'],
    user: { text: '', attachments: [] },
    reply: { status: 'done', text: '', artifacts: { diff: '', changed_files: [], worktree: `D:/wt/${item}`, stdout_summary: '' } },
    target: { agent: 'codex', model: 'gpt-5', role: '执行', thinking: '中', visibility: '共享', write: 'worktree' },
    swarm: { item, files: [`src/${item}/**`], accept: `pytest tests/test_${item}.py` },
    created_at: `2026-09-15T10:0${item}:00Z`,
    ...patch,
  };
}

function mkGraph(nodes: GraphNode[]): Graph {
  const graph = newGraph();
  const root: GraphNode = { ...graph.nodes[graph.root], id: 'root' };
  const map: Record<string, GraphNode> = { root };
  for (const node of nodes) map[node.id] = node;
  return { ...graph, root: 'root', nodes: map };
}

describe('B1 契约：Node.swarm 与两条命令', () => {
  it('B1 swarm 字段跟着节点走过一次盖章与一次写回', () => {
    const graph = mkGraph([mkSwarmNode('a', 1)]);
    const preset: Preset = { id: 'p', name: 'p', builtin: false, stages: [] };
    const stamped = stamp(graph, 'a', preset);
    expect(stamped.nodes.a.swarm).toEqual({ item: 1, files: ['src/1/**'], accept: 'pytest tests/test_1.py' });

    const next = updateNodeSwarm(graph, 'a', { item: 1, files: ['src/x/**', 'docs/y.md'], accept: '' });
    expect(next.nodes.a.swarm).toEqual({ item: 1, files: ['src/x/**', 'docs/y.md'], accept: '' });
    // 原图不动，files 数组是拷贝
    expect(graph.nodes.a.swarm!.files).toEqual(['src/1/**']);
  });

  it('B1 没有 swarm 字段的节点不被凭空补上', () => {
    const graph = mkGraph([mkSwarmNode('a', 1, { swarm: undefined })]);
    expect(updateNodeSwarm(graph, 'a', { item: 9, files: [], accept: '' })).toBe(graph);
  });

  it('B1 后端未接通时两条命令 reject，回退值就是契约里那个形状', async () => {
    const wire = swarmWireSettings(DEFAULT_DISPATCH_SETTINGS);
    await expect(commands.checkSwarmPlan('D:/p', [{ item: 1, files: ['a/**'] }], wire)).rejects.toThrow();
    await expect(commands.mergeSwarmItems('D:/p', 'run', [], '', wire)).rejects.toThrow();
    expect(swarmMergeFallback([{ nodeId: 'a', item: 1, worktree: 'D:/wt/1', files: [], accept: '' }])).toEqual([
      { nodeId: 'a', item: 1, result: 'skipped', commit: '', note: SWARM_BACKEND_MISSING_NOTE, testOutput: '' },
    ]);
  });
});

describe('B2 内置预设「蜂群模式」', () => {
  it('B2 盖在根材料上长出一个只读、共享的拆分节点', () => {
    const graph = newGraph();
    const stamped = stamp(graph, graph.root, BUILTIN_SWARM_PRESET);
    const created = Object.values(stamped.nodes).filter((node) => node.id !== graph.root);
    expect(created).toHaveLength(1);
    expect(created[0].kind).toBe('turn');
    expect(created[0].parents).toEqual([graph.root]);
    expect(created[0].target).toMatchObject({ role: '拆分', visibility: '共享', write: '只读' });
    // 节点文本就是冻结的拆分提示词（split.txt）加一行本次上限，不在 TS 里另写一份
    expect(created[0].user.text.startsWith(SWARM_SPLIT_PROMPT)).toBe(true);
    expect(created[0].user.text).toContain('不修改文件');
    expect(created[0].user.text).toContain('```');
    expect(created[0].user.text.endsWith(swarmLimitLine(4))).toBe(true);
    expect(PRESET_NODE_TEXTS['builtin-swarm']).toBeUndefined();
  });

  it('B2 拆分角色的默认是共享 / 只读，目标按「出方案」填', () => {
    expect(roleDefaults('拆分')).toEqual({ visibility: '共享', write: '只读' });
    const filled = applyRoleDefaults(
      { agent: '', model: '', role: '拆分', thinking: '中' },
      ROLE_PROFILES,
    );
    expect(filled).toMatchObject({ agent: 'claude', model: 'opus', tier: '高', thinking: '高' });
    const preset = presetWithRoleDefaults(BUILTIN_SWARM_PRESET, ROLE_PROFILES);
    expect(preset.stages[0].nodes[0]).toMatchObject({ agent: 'claude', model: 'opus', role: '拆分' });
  });

  it('B2 引擎没有同 id 预设时前端兜一份，有则让位', () => {
    expect(withSwarmPreset([]).map((p) => p.id)).toEqual(['builtin-swarm']);
    const backend: Preset = { id: 'builtin-swarm', name: '蜂群模式', builtin: true, stages: [] };
    expect(withSwarmPreset([backend])).toEqual([backend]);
    expect(BUILTIN_SWARM_PRESET.name).toBe('蜂群模式');
  });

  it('B2 件数上限与并发上限同一处设置，默认 4', () => {
    expect(SWARM_LIMIT_DEFAULT).toBe(4);
    expect(DEFAULT_DISPATCH_SETTINGS.swarmLimit).toBe(4);
    const store: Record<string, string> = { 'aicross-dispatch-settings': JSON.stringify({ swarmLimit: 99 }) };
    vi.stubGlobal('localStorage', { getItem: (k: string) => store[k] ?? null });
    expect(loadDispatchSettings().swarmLimit).toBe(4);
    vi.unstubAllGlobals();
  });
});

describe('B3 清单解析与按清单建节点', () => {
  const fence = (body: string) => ['前言', '```json', body, '```', '后记'].join('\n');

  it('B3 取最后一个 json 围栏', () => {
    const text = [
      fence('{"items":[{"title":"旧","files":["old/**"],"accept":""}]}'),
      fence('{"items":[{"title":"新","files":["new/**"],"accept":"pytest"}]}'),
    ].join('\n');
    expect(parseSwarmPlan(text)!.items).toEqual([{ title: '新', files: ['new/**'], accept: 'pytest' }]);
  });

  it('B3 缺围栏 / JSON 坏 / 字段不合格都返回 null', () => {
    expect(parseSwarmPlan('没有围栏的普通回复')).toBeNull();
    expect(parseSwarmPlan(fence('{ 这不是 json }'))).toBeNull();
    expect(parseSwarmPlan(fence('{"items":[]}'))).toBeNull();
    expect(parseSwarmPlan(fence('{"items":[{"title":"","files":["a/**"]}]}'))).toBeNull();
    expect(parseSwarmPlan(fence('{"items":[{"title":"甲","files":[]}]}'))).toBeNull();
    expect(parseSwarmPlan(fence('{"items":[{"title":"甲","files":"a/**"}]}'))).toBeNull();
    expect(parseSwarmPlan(fence('{"items":[{"title":"甲","files":["a/**"],"accept":3}]}'))).toBeNull();
  });

  it('B3 accept 可缺省，超上限截断并给一句提示', () => {
    const plan = parseSwarmPlan(fence('{"items":[{"title":"甲","files":["a/**"]}]}'))!;
    expect(plan).toEqual({ items: [{ title: '甲', files: ['a/**'], accept: '' }], dropped: 0 });
    expect(swarmPlanNotice(plan)).toBeNull();

    const five = JSON.stringify({ items: [1, 2, 3, 4, 5].map((n) => ({ title: `第${n}`, files: [`s${n}/**`] })) });
    const capped = parseSwarmPlan(fence(five), 4)!;
    expect(capped.items).toHaveLength(4);
    expect(capped.dropped).toBe(1);
    expect(swarmPlanNotice(capped)).toBe('清单 5 件，超过上限 4 件，只取前 4 件');
    expect(parseSwarmPlan(fence(five), 2)!.items.map((i) => i.title)).toEqual(['第1', '第2']);
  });

  it('B3 每件建一个 draft 执行节点：worktree、件号、清单文本', () => {
    const graph = mkGraph([]);
    const split: GraphNode = {
      id: 'split', kind: 'turn', parents: ['root'],
      user: { text: '', attachments: [] },
      reply: { status: 'done', text: '' },
      target: { agent: 'claude', model: 'opus', role: '拆分', thinking: '高', visibility: '共享', write: '只读' },
      created_at: '2026-09-15T10:00:00Z',
    };
    const base = { ...graph, nodes: { ...graph.nodes, split } };
    const items = [
      { title: '改解析器', files: ['src/a/**', 'docs/x.md'], accept: 'pytest tests/test_a.py' },
      { title: '补测试', files: ['tests/**'], accept: '' },
    ];
    const outcome = appendSwarmNodes(base, 'split', items, ROLE_PROFILES.execution);
    expect(outcome.nodeIds).toHaveLength(2);
    const first = outcome.graph.nodes[outcome.nodeIds[0]];
    expect(first.parents).toEqual(['split']);
    expect(first.reply).toEqual({ status: 'draft', text: '' });
    // R1-8：可见性是「指定上游 = 根材料」，不是共享——共享会把整份清单带进每件
    expect(first.target).toMatchObject({ agent: 'codex', model: 'gpt-5', role: '执行', write: 'worktree', visibility: '指定上游', thinking: '中' });
    expect(first.target!.upstreams).toEqual(['root']);
    expect(first.swarm).toEqual({ item: 1, files: ['src/a/**', 'docs/x.md'], accept: 'pytest tests/test_a.py' });
    expect(first.user.text).toBe(swarmNodeText(items[0]));
    expect(first.user.text).toBe('改解析器\n文件范围：src/a/**、docs/x.md\n验收命令：pytest tests/test_a.py');
    expect(outcome.graph.nodes[outcome.nodeIds[1]].swarm!.item).toBe(2);
    // accept 为空时文本里不出现验收命令行
    expect(outcome.graph.nodes[outcome.nodeIds[1]].user.text).toBe('补测试\n文件范围：tests/**');
    // 空清单 / 找不到拆分节点：原图不动
    expect(appendSwarmNodes(base, 'split', []).graph).toBe(base);
    expect(appendSwarmNodes(base, 'missing', items).graph).toBe(base);
    // 没有角色默认时目标留空，G1 再选；写权限仍是 worktree
    const blank = appendSwarmNodes(base, 'split', items);
    expect(blank.graph.nodes[blank.nodeIds[0]].target).toMatchObject({
      agent: '', model: '', role: '执行', write: 'worktree', thinking: '中',
    });
  });
});

describe('B4 G1 重叠闸门', () => {
  const check = (patch: Partial<SwarmPlanCheck>): SwarmPlanCheck => ({ dirty: false, overlaps: [], unmatched: [], ...patch });
  const ok = (patch: Partial<SwarmPlanCheck>): SwarmCheckState => ({ phase: 'ok', check: check(patch) });

  it('B4 swarmGate：干净且不重叠放行', () => {
    expect(swarmGate(ok({}))).toEqual({ blocked: false, reasons: [], hints: [], canProceed: false });
    // 没有蜂群节点 / 没有项目目录：不核对也不拦
    expect(swarmGate({ phase: 'idle' })).toEqual({ blocked: false, reasons: [], hints: [], canProceed: false });
  });

  it('B4 swarmGate：重叠阻断并点名两件与撞上的路径', () => {
    const gate = swarmGate(ok({ overlaps: [{ a: 1, b: 2, path: 'src/a/x.ts' }] }));
    expect(gate.blocked).toBe(true);
    expect(gate.reasons).toEqual(['第 1 件与第 2 件都会改 src/a/x.ts，改到不重叠或合并成一件']);
    expect(swarmOverlapItems(check({ overlaps: [{ a: 2, b: 1, path: 'p' }, { a: 1, b: 3, path: 'q' }] }))).toEqual([1, 2, 3]);
  });

  it('B4 swarmGate：脏树阻断，glob 没匹配只提示', () => {
    expect(swarmGate(ok({ dirty: true }))).toEqual({ blocked: true, reasons: [SWARM_DIRTY_TEXT], hints: [], canProceed: false });
    const gate = swarmGate(ok({ unmatched: [{ item: 2, glob: 'docs/none/**' }] }));
    expect(gate.blocked).toBe(false);
    expect(gate.hints).toEqual(['第 2 件的范围 docs/none/** 在项目里没有匹配到文件']);
  });

  it('B4 送给引擎的核对项按件号排序、只取带 swarm 的节点', () => {
    const graph = mkGraph([mkSwarmNode('b', 2), mkSwarmNode('a', 1), mkSwarmNode('c', 3, { swarm: undefined })]);
    expect(swarmCheckItems(graph, ['b', 'a', 'c', 'root'])).toEqual([
      { item: 1, files: ['src/1/**'] },
      { item: 2, files: ['src/2/**'] },
    ]);
  });

  it('B4 SwarmGateNotice：阻断走警告样式，提示走常规灰字', () => {
    const html = renderToStaticMarkup(
      <SwarmGateNotice gate={{ blocked: true, reasons: ['重叠了'], hints: ['没匹配到'], canProceed: false }} />,
    );
    expect(html).toContain('dispatch-warning');
    expect(html).toContain('重叠了');
    expect(html).toContain('dispatch-reason');
    expect(html).toContain('没匹配到');
    expect(renderToStaticMarkup(<SwarmGateNotice gate={{ blocked: false, reasons: [], hints: [], canProceed: false }} />)).toBe('');
  });

  it('B4 SwarmEditor：draft 给编辑框，非 draft 只读', () => {
    const swarm: SwarmItem = { item: 2, files: ['src/a/**', 'docs/x.md'], accept: 'pytest' };
    const editable = renderToStaticMarkup(<SwarmEditor swarm={swarm} editable onChange={() => {}} />);
    expect(editable).toContain('<textarea');
    expect(editable).toContain('src/a/**\ndocs/x.md');
    expect(editable).toContain('第 2 件');
    const readonly = renderToStaticMarkup(<SwarmEditor swarm={swarm} editable={false} onChange={() => {}} />);
    expect(readonly).not.toContain('<textarea');
    expect(readonly).toContain('src/a/**、docs/x.md');
  });
});

describe('B5 G3 逐件合并', () => {
  const result = (patch: Partial<SwarmMergeResult>): SwarmMergeResult => ({
    nodeId: 'a', item: 1, result: 'merged', commit: '', note: '', testOutput: '', ...patch,
  });

  it('B5 全部 swarm 节点终态才出合并按钮', () => {
    const done = mkGraph([mkSwarmNode('a', 1), mkSwarmNode('b', 2)]);
    expect(swarmMergeReady(done, ['a', 'b'])).toBe(true);
    const failed = mkGraph([mkSwarmNode('a', 1), mkSwarmNode('b', 2, { reply: { status: 'failed', text: '' } })]);
    expect(swarmMergeReady(failed, ['a', 'b'])).toBe(true);
    const quota = mkGraph([mkSwarmNode('a', 1), mkSwarmNode('b', 2, { reply: { status: 'quota_exhausted', text: '' } })]);
    expect(swarmMergeReady(quota, ['a', 'b'])).toBe(true);
    const pending = mkGraph([mkSwarmNode('a', 1), mkSwarmNode('b', 2, { reply: { status: 'pending', text: '' } })]);
    expect(swarmMergeReady(pending, ['a', 'b'])).toBe(false);
    expect(swarmMergeReady(done, [])).toBe(false);
  });

  it('B5 送给引擎的件按件号排序，带 worktree 与本件验收命令', () => {
    const graph = mkGraph([mkSwarmNode('b', 2), mkSwarmNode('a', 1)]);
    expect(swarmMergeItems(graph, ['b', 'a'])).toEqual([
      { nodeId: 'a', item: 1, worktree: 'D:/wt/1', files: graph.nodes.a.swarm!.files, title: '', accept: 'pytest tests/test_1.py' },
      { nodeId: 'b', item: 2, worktree: 'D:/wt/2', files: graph.nodes.b.swarm!.files, title: '', accept: 'pytest tests/test_2.py' },
    ]);
  });

  it('B5 结果行：已并入 / 冲突待裁决 / 测试红 / 已跳过', () => {
    expect(swarmResultLine(result({ commit: 'abc1234' }))).toBe('第 1 件：已并入 abc1234');
    expect(swarmResultLine(result({ item: 2, result: 'conflict', note: 'D:/wt/2' }))).toBe('第 2 件：冲突待裁决 · D:/wt/2');
    expect(swarmResultLine(result({ item: 3, result: 'test_failed', note: '已回滚' }))).toBe('第 3 件：验收未通过 · 已回滚');
    expect(swarmResultLine(result({ item: 4, result: 'skipped', note: '无改动' }))).toBe('第 4 件：已跳过 · 无改动');
  });

  it('B5 只有 merged 的节点自动 adopted，冲突与测试红不动', () => {
    const graph = mkGraph([mkSwarmNode('a', 1), mkSwarmNode('b', 2), mkSwarmNode('c', 3)]);
    const next = applySwarmMerge(graph, [
      result({ nodeId: 'a', item: 1, commit: 'aaa' }),
      result({ nodeId: 'b', item: 2, result: 'conflict' }),
      result({ nodeId: 'c', item: 3, result: 'test_failed' }),
      result({ nodeId: 'missing', item: 4 }),
    ]);
    expect(next.nodes.a.reply?.adopted).toBe(true);
    expect(next.nodes.b.reply?.adopted).toBeUndefined();
    expect(next.nodes.c.reply?.adopted).toBeUndefined();
  });

  it('B5 引擎返回 node_id / test_output 或 camelCase 都收得住（旧引擎兼容）', () => {
    const rows = normalizeSwarmMergeResults([
      { node_id: 'a', item: 1, result: 'merged', commit: 'aaa', note: '', test_output: '' },
      { nodeId: 'b', item: 2, result: 'test_failed', commit: '', note: '已回滚', testOutput: 'FAILED' },
      { nodeId: 'c', item: 3, result: '莫名其妙' },
      '不是对象',
    ]);
    expect(rows).toEqual([
      { nodeId: 'a', item: 1, result: 'merged', commit: 'aaa', note: '', testOutput: '' },
      { nodeId: 'b', item: 2, result: 'test_failed', commit: '', note: '已回滚', testOutput: 'FAILED' },
      { nodeId: 'c', item: 3, result: 'skipped', commit: '', note: '', testOutput: '' },
    ]);
    expect(normalizeSwarmMergeResults(null)).toEqual([]);
  });

  it('B5 SwarmMergeRow：并入走灰字，其它走警告，验收输出折起来', () => {
    expect(renderToStaticMarkup(<SwarmMergeRow result={result({ commit: 'aaa' })} />)).toContain('dispatch-reason');
    const red = renderToStaticMarkup(<SwarmMergeRow result={result({ result: 'test_failed', note: '已回滚', testOutput: 'FAILED test_a' })} />);
    expect(red).toContain('dispatch-warning');
    expect(red).toContain('<details>');
    expect(red).toContain('FAILED test_a');
    expect(renderToStaticMarkup(<SwarmMergeRow result={result({ commit: 'aaa' })} />)).not.toContain('<details>');
  });
});

describe('B6 手册', () => {
  it('B6 docs/MANUAL.md 有「蜂群模式」一节，写清怎么盖、清单、合并、冲突', () => {
    const manual = readFileSync(new URL('../../../../docs/MANUAL.md', import.meta.url), 'utf-8') as string;
    expect(manual).toContain('## 蜂群模式');
    for (const word of ['拆分', 'glob', 'cherry-pick', '冲突', '验收命令', '干净']) {
      expect(manual).toContain(word);
    }
  });
});

// 修正轮 R1（tasks/fix-08b-r1.md 的 9 条）：编号与那份清单一致。
describe('R1 修正轮', () => {
  const check = (patch: Partial<SwarmPlanCheck>): SwarmPlanCheck => ({ dirty: false, overlaps: [], unmatched: [], ...patch });
  const mergeRow = (patch: Partial<SwarmMergeResult>): SwarmMergeResult => ({
    nodeId: 'a', item: 1, result: 'merged', commit: '', note: '', testOutput: '', ...patch,
  });
  const splitGraph = (rootText = '根材料', replyText = ''): Graph => {
    const graph = mkGraph([]);
    const root: GraphNode = { ...graph.nodes.root, user: { text: rootText, attachments: [] } };
    const split: GraphNode = {
      id: 'split',
      kind: 'turn',
      parents: ['root'],
      user: { text: [SWARM_SPLIT_PROMPT, swarmLimitLine()].join('\n\n'), attachments: [] },
      reply: { status: 'done', text: replyText },
      target: { agent: 'claude', model: 'opus', role: '拆分', thinking: '高', visibility: '共享', write: '只读' },
      created_at: '2026-09-15T10:00:00Z',
    };
    return { ...graph, nodes: { root, split } };
  };

  it('R1-1 核对没回来时闸门拦住确认派发，核对不了时可以选择继续', () => {
    const pending = swarmGate({ phase: 'pending' });
    expect(pending.blocked).toBe(true);
    expect(pending.hints).toEqual([SWARM_CHECKING_TEXT]);
    expect(pending.canProceed).toBe(false);

    const state: SwarmCheckState = { phase: 'error', message: '读不到项目目录' };
    const failed = swarmGate(state);
    expect(failed.blocked).toBe(true);
    expect(failed.canProceed).toBe(true);
    expect(failed.reasons).toEqual([`${SWARM_CHECK_FAILED_TEXT}：读不到项目目录`]);

    const proceed = swarmGate(state, true);
    expect(proceed.blocked).toBe(false);
    expect(proceed.hints).toEqual([SWARM_PROCEED_TEXT]);
    // 核对回来了就按核对结果说话，不再问要不要继续
    expect(swarmGate({ phase: 'ok', check: check({}) }).canProceed).toBe(false);
  });

  it('R1-1 SwarmGateNotice：核对中出灰字，核对不了才出「仍要派发」', () => {
    const checking = renderToStaticMarkup(
      <SwarmGateNotice gate={swarmGate({ phase: 'pending' })} proceed={false} onProceed={() => {}} />,
    );
    expect(checking).toContain(SWARM_CHECKING_TEXT);
    expect(checking).not.toContain('仍要派发');
    const failed = renderToStaticMarkup(
      <SwarmGateNotice gate={swarmGate({ phase: 'error', message: '命令不可用' })} proceed={false} onProceed={() => {}} />,
    );
    expect(failed).toContain(SWARM_CHECK_FAILED_TEXT);
    expect(failed).toContain('仍要派发');
    expect(failed).toContain('type="checkbox"');
  });

  it('R1-2 同一拆分节点再点一次不重复建件，件数上限在生成时也生效', () => {
    const base = splitGraph();
    const items = [1, 2, 3].map((n) => ({ title: `第${n}`, files: [`src/${n}/**`], accept: '' }));
    const first = appendSwarmNodes(base, 'split', items, undefined, 4);
    expect(first.nodeIds).toHaveLength(3);
    expect(swarmChildItems(first.graph, 'split')).toEqual([1, 2, 3]);
    expect(swarmGenerateNotice(first, 4)).toBe('已生成 3 个执行节点');

    const again = appendSwarmNodes(first.graph, 'split', items, undefined, 4);
    expect(again.nodeIds).toEqual([]);
    expect(again.graph).toBe(first.graph);
    expect(again.skipped).toEqual([1, 2, 3]);
    expect(swarmGenerateNotice(again, 4)).toBe('第 1、2、3 件已生成，未重复建');

    const capped = appendSwarmNodes(base, 'split', items, undefined, 2);
    expect(capped.nodeIds).toHaveLength(2);
    expect(capped.capped).toBe(1);
    expect(swarmGenerateNotice(capped, 2)).toBe('已生成 2 个执行节点；件数上限 2 件，1 件未生成');
  });

  it('R1-3 逐件结果写回节点正文：并入标采纳，冲突与测试红只记 note', () => {
    const graph = mkGraph([
      mkSwarmNode('a', 1, { reply: { status: 'done', text: '做完了' } }),
      mkSwarmNode('b', 2, { reply: { status: 'done', text: '做完了' } }),
    ]);
    const results = [
      mergeRow({ nodeId: 'a', item: 1, commit: 'abc1234' }),
      mergeRow({ nodeId: 'b', item: 2, result: 'conflict', note: 'D:/wt/2' }),
    ];
    const next = applySwarmMerge(graph, results);
    expect(next.nodes.a.reply!.text).toContain(`${SWARM_MERGE_MARK} 第 1 件：已并入 abc1234`);
    expect(next.nodes.a.reply!.adopted).toBe(true);
    expect(next.nodes.b.reply!.text).toContain('第 2 件：冲突待裁决 · D:/wt/2');
    expect(next.nodes.b.reply!.adopted).toBeUndefined();
    expect(swarmReplyNote(results[0])).toBe(`${SWARM_MERGE_MARK} 第 1 件：已并入 abc1234`);
    // 再写一次不重复追加
    expect(applySwarmMerge(next, results).nodes.a.reply!.text).toBe(next.nodes.a.reply!.text);
    // 引擎未接通那批不写进正文
    const missing = applySwarmMerge(graph, swarmMergeFallback([{ nodeId: 'a', item: 1, worktree: '', files: [], accept: '' }]));
    expect(missing.nodes.a.reply!.text).toBe('做完了');
  });

  it('R1-3 已并入的件不进下一次合并请求', () => {
    const graph = mkGraph([mkSwarmNode('a', 1), mkSwarmNode('b', 2)]);
    const merged = applySwarmMerge(graph, [mergeRow({ nodeId: 'a', item: 1, commit: 'aaa' })]);
    expect(swarmPendingMergeIds(merged, ['a', 'b'])).toEqual(['b']);
    expect(swarmMergeItems(merged, ['a', 'b']).map((row) => row.item)).toEqual([2]);
    const all = applySwarmMerge(merged, [mergeRow({ nodeId: 'b', item: 2, commit: 'bbb' })]);
    expect(swarmMergeItems(all, ['a', 'b'])).toEqual([]);
  });

  it('R1-3 失败分级：只有引擎未接通才说未接通，部分并入如实报数', () => {
    const items = [{ nodeId: 'a', item: 1, worktree: 'D:/wt/1', files: [], accept: '' }];
    expect(swarmMergeErrorResults(items, new Error('Tauri IPC not available'))[0].note).toBe(SWARM_BACKEND_MISSING_NOTE);
    expect(swarmMergeErrorResults(items, new Error('git cherry-pick 失败'))[0].note).toBe('合并未完成：git cherry-pick 失败');
    expect(swarmMergeSummary(swarmMergeFallback(items))).toBe(SWARM_BACKEND_MISSING_NOTE);
    expect(swarmMergeSummary([])).toBe(SWARM_MERGE_EMPTY_TEXT);
    expect(swarmMergeSummary([
      mergeRow({ nodeId: 'a', item: 1, commit: 'aaa' }),
      mergeRow({ nodeId: 'b', item: 2, result: 'conflict', note: 'D:/wt/2' }),
      mergeRow({ nodeId: 'c', item: 3, result: 'test_failed', note: '已回滚' }),
    ])).toBe('已并入 1 件，冲突 1 件，验收未通过 1 件');
  });

  it('R1-4 蜂群卡片不出「选它」，未并入不给「采纳」，已并入不给「重派」', () => {
    const graph = mkGraph([mkSwarmNode('a', 1)]);
    const plain: GraphNode = { ...graph.nodes.a, swarm: undefined };
    expect(swarmCardActions(plain)).toEqual({ showPickOne: true, showAdopt: true, showRetry: true, note: '' });
    expect(swarmCardActions(graph.nodes.a)).toEqual({ showPickOne: false, showAdopt: false, showRetry: true, note: '' });
    const merged = mergeRow({ nodeId: 'a', item: 1, commit: 'aaa' });
    expect(swarmCardActions(graph.nodes.a, merged)).toEqual({ showPickOne: false, showAdopt: true, showRetry: false, note: '已并入' });
    // 合并写回 adopted 之后，不带本次结果也认得出已并入；已采纳的件不再给按钮（08C-6）
    expect(swarmCardActions(applySwarmMerge(graph, [merged]).nodes.a)).toMatchObject({ showAdopt: false, showRetry: false, note: '已并入' });
    expect(swarmCardActions(graph.nodes.a, mergeRow({ result: 'test_failed', note: '已回滚' })))
      .toMatchObject({ showAdopt: false, showRetry: true });
  });

  it('R1-5 重叠件改完文件范围，重核对的入参跟着变', () => {
    const graph = mkGraph([
      mkSwarmNode('a', 1),
      mkSwarmNode('b', 2, { swarm: { item: 2, files: ['src/1/**'], accept: '' } }),
    ]);
    expect(swarmCheckItems(graph, ['a', 'b'])).toEqual([
      { item: 1, files: ['src/1/**'] },
      { item: 2, files: ['src/1/**'] },
    ]);
    const edited = updateNodeSwarm(graph, 'b', { item: 2, files: ['src/2/**'], accept: 'pytest' });
    expect(swarmCheckItems(edited, ['a', 'b'])).toEqual([
      { item: 1, files: ['src/1/**'] },
      { item: 2, files: ['src/2/**'] },
    ]);
    expect(swarmGate({ phase: 'ok', check: check({ overlaps: [{ a: 1, b: 2, path: 'src/1/x.ts' }] }) }).reasons[0])
      .toContain('改到不重叠或合并成一件');
  });

  it('R1-6 图上「盖预设」菜单也套 withSwarmPreset', () => {
    const source = readFileSync(
      new URL('../../components/center/graph/GraphTab.tsx', import.meta.url),
      'utf-8',
    ) as string;
    expect(source).not.toMatch(/setPresets\(f\.presets\)/);
    expect(source.match(/withSwarmPreset\(f\.presets\)/g)).toHaveLength(2);
  });

  it('R1-7 拆分提示词按设置的件数上限补「本次上限 N 件」', () => {
    expect(swarmLimitLine(2)).toBe('本次上限 2 件。');
    expect(swarmLimitLine()).toBe(`本次上限 ${SWARM_LIMIT_DEFAULT} 件。`);
    const graph = newGraph();
    const stamped = stamp(graph, graph.root, BUILTIN_SWARM_PRESET, 2);
    const created = Object.values(stamped.nodes).filter((node) => node.id !== graph.root);
    expect(created[0].user.text).toContain('本次上限 2 件');
    // 同一个上限也是解析时的截断线与提示里的数
    const five = ['```json', JSON.stringify({ items: [1, 2, 3].map((n) => ({ title: `第${n}`, files: [`s${n}/**`] })) }), '```'].join('\n');
    const plan = parseSwarmPlan(five, 2)!;
    expect(plan.items).toHaveLength(2);
    expect(swarmPlanNotice(plan, 2)).toBe('清单 3 件，超过上限 2 件，只取前 2 件');
  });

  it('R1-8 每件只带自己的清单条目，上下文不带整份清单', () => {
    const plan = ['```json', JSON.stringify({
      items: [
        { title: '改解析器', files: ['src/a/**'], accept: 'pytest a' },
        { title: '补覆盖率', files: ['tests/**'], accept: 'pytest b' },
      ],
    }), '```'].join('\n');
    const base = splitGraph('根材料：把解析器改一改', plan);
    const items = parseSwarmPlan(plan)!.items;
    const outcome = appendSwarmNodes(base, 'split', items, undefined, 4);
    const first = outcome.graph.nodes[outcome.nodeIds[0]];
    expect(first.user.text).toBe('改解析器\n文件范围：src/a/**\n验收命令：pytest a');
    expect(first.user.text).not.toContain('补覆盖率');
    const { prompt } = serializeContext(outcome.graph, first.id);
    expect(prompt).toContain('根材料：把解析器改一改');
    expect(prompt).not.toContain('补覆盖率');
    expect(prompt).not.toContain('tests/**');
    // 拆分回复（整份清单）不进上下文
    expect(contextPartsOf(outcome.graph, first.id).some((part) => part.source === 'reply')).toBe(false);
  });

  it('R1-GLM4 蜂群件不进第三方仲裁的候选', () => {
    const graph = mkGraph([mkSwarmNode('a', 1), mkSwarmNode('b', 2), mkSwarmNode('c', 3, { swarm: undefined })]);
    const rows = [
      { nodeId: 'a', status: 'ok' },
      { nodeId: 'b', status: 'ok' },
      { nodeId: 'c', status: 'ok' },
      { nodeId: 'root', status: 'error' },
    ];
    expect(arbitrationCandidates(graph, rows).map((row) => row.nodeId)).toEqual(['c']);
  });

  it('R1-9 旧图节点没有 swarm 字段时按无蜂群处理', () => {
    const graph = mkGraph([mkSwarmNode('a', 1, { swarm: undefined }), mkSwarmNode('b', 2, { swarm: undefined })]);
    expect(swarmNodeIdsOf(graph, ['a', 'b'])).toEqual([]);
    expect(swarmCheckItems(graph, ['a', 'b'])).toEqual([]);
    expect(swarmMergeReady(graph, ['a', 'b'])).toBe(false);
    expect(swarmMergeItems(graph, ['a', 'b'])).toEqual([]);
    expect(swarmChildItems(graph, 'root')).toEqual([]);
    expect(swarmCardActions(graph.nodes.a)).toMatchObject({ showPickOne: true, showAdopt: true, showRetry: true });
  });
});
