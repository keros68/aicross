// TASK-07 B 节（界面/TS）：G2 疑似空转提示 + TASK-06B 两轮复审的收尾项。
// 编号与 tasks/TASK-07-idle-and-tail.md「B. 界面（TS）」1–5 一致。

// tsconfig.app.json 的 types 里只有 vite/client：应用代码不该碰 node 内置模块。
// 只有读源文件断言的那两条要读磁盘，就地压掉解析错误（同 shiki.test.ts 的写法）。
// @ts-expect-error node builtin, not in this project's `types`
import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import {
  applyDispatchResults,
  applyReconcile,
  cacheReadOf,
  idleHint,
  nextProgress,
  progressOf,
  progressSnapshot,
  progressStatusLabel,
  reconcileUiConverged,
  runningDetailOf,
  runStageLoop,
  DUPLICATE_BATCH_TEXT,
  EMPTY_RECONCILE_UI,
  IDLE_HINT_MS,
  RECONCILE_UNKNOWN_TEXT,
  REPEATED_TOOL_IDLE_MS,
  type BackendNodeResult,
  type ProgressSnapshot,
  type ReconcileSummary,
  type ReconcileUi,
  type StageLoopState,
} from '../dispatch.ts';
import { newGraph, newNodeId } from '../types.ts';
import type { Graph, GraphNode, Target } from '../types.ts';
import { IdleHint } from '../../components/center/graph/DispatchBits.tsx';

function mkTarget(patch: Partial<Target> = {}): Target {
  return {
    agent: 'pi',
    model: 'glm',
    role: '执行',
    thinking: '中',
    visibility: '共享',
    write: '只读',
    ...patch,
  };
}

function mkTurn(id: string, parents: string[], patch: Partial<GraphNode> = {}): GraphNode {
  return {
    id,
    kind: 'turn',
    parents,
    user: { text: '', attachments: [] },
    created_at: '2026-09-14T00:00:00.000Z',
    ...patch,
  };
}

function snapshot(patch: Partial<ProgressSnapshot> = {}): ProgressSnapshot {
  return {
    runId: 'run-1',
    status: 'running',
    elapsedMs: 20_000,
    idleMs: 1_000,
    toolCalls: 2,
    repeatedToolCalls: 1,
    lastTool: '',
    receivedAt: 1_000_000,
    hasHeartbeat: true,
    ...patch,
  };
}

function readSource(relative: string): string {
  return readFileSync(new URL(relative, import.meta.url), 'utf8');
}

describe('07B-1 G2 疑似空转提示', () => {
  it('idleHint：正常运行不提示', () => {
    expect(idleHint(snapshot(), 1_000_000)).toBeNull();
    // 心跳还没到（只有「排队」）、节点已经收尾，都不算空转
    expect(idleHint(undefined, 1_000_000)).toBeNull();
    expect(idleHint(snapshot({ status: '排队' }), 1_000_000)).toBeNull();
    expect(idleHint(snapshot({ status: '完成', repeatedToolCalls: 5 }), 1_000_000)).toBeNull();
  });

  it('idleHint：同一工具连续 3 次且 30 秒无输出 → 疑似空转', () => {
    // R1-3：重复计数单独用会误报——连续读 3 个文件几秒内就命中，那时还在产出。
    expect(idleHint(snapshot({ repeatedToolCalls: 3, idleMs: 2_000 }), 1_000_000)).toBeNull();
    expect(idleHint(snapshot({ repeatedToolCalls: 3, idleMs: REPEATED_TOOL_IDLE_MS - 1 }), 1_000_000))
      .toBeNull();
    // 次数不够，光空闲 30 秒也不提示（要到 3 分钟才走另一条）
    expect(idleHint(snapshot({ repeatedToolCalls: 2, idleMs: REPEATED_TOOL_IDLE_MS }), 1_000_000))
      .toBeNull();
    expect(idleHint(snapshot({ repeatedToolCalls: 3, idleMs: REPEATED_TOOL_IDLE_MS, lastTool: 'read_file' }), 1_000_000))
      .toBe('疑似空转：同一工具连续 3 次');
    // 空闲也可以由墙上时间凑满
    expect(idleHint(snapshot({ repeatedToolCalls: 7, idleMs: 1_000 }), 1_000_000 + 30_000))
      .toBe('疑似空转：同一工具连续 7 次');
  });

  it('idleHint：没有心跳元数据的快照永不判空转，3 分钟后也不判', () => {
    // R1-1：旧引擎只发一条状态词，没有 elapsedMs/idleMs，也不会再发第二条。
    // 这种快照如果累计墙上时间，任何跑满 3 分钟的节点都会被误判成空转。
    const bare = progressSnapshot({ runId: 'run-1', nodeId: 'n1', status: '运行中' }, 1_000_000);
    expect(bare.hasHeartbeat).toBe(false);
    expect(idleHint(bare, 1_000_000)).toBeNull();
    expect(idleHint(bare, 1_000_000 + 3 * 60_000)).toBeNull();
    expect(idleHint(bare, 1_000_000 + 60 * 60_000)).toBeNull();
    // 只要带上任一心跳字段就重新按真读数判
    const beat = progressSnapshot(
      { runId: 'run-1', nodeId: 'n1', status: 'running', idleMs: 0 },
      1_000_000,
    );
    expect(beat.hasHeartbeat).toBe(true);
    expect(idleHint(beat, 1_000_000 + 3 * 60_000)).toBe('疑似空转：3 分钟无输出');
  });

  it('idleHint：3 分钟无输出 → 疑似空转，心跳停了也按墙上时间继续长', () => {
    // 心跳自己报的 idleMs 就够
    expect(idleHint(snapshot({ idleMs: IDLE_HINT_MS }), 1_000_000)).toBe('疑似空转：3 分钟无输出');
    // 差一点点不提示
    expect(idleHint(snapshot({ idleMs: IDLE_HINT_MS - 1 }), 1_000_000)).toBeNull();
    // 引擎卡死、心跳也不发了：最后一条心跳的 idleMs 加上墙上时间
    expect(idleHint(snapshot({ idleMs: 60_000 }), 1_000_000 + 125_000)).toBe('疑似空转：3 分钟无输出');
    expect(idleHint(snapshot({ idleMs: 0 }), 1_000_000 + 8 * 60_000)).toBe('疑似空转：8 分钟无输出');
  });

  it('progressSnapshot：后端缺字段按 0 / 空串；running 显示成「运行中」', () => {
    const bare = progressSnapshot({ runId: 'r1', nodeId: 'n1', status: 'running' }, 1_000_000);
    expect(bare).toEqual({
      runId: 'r1',
      status: 'running',
      elapsedMs: 0,
      idleMs: 0,
      toolCalls: 0,
      repeatedToolCalls: 0,
      lastTool: '',
      receivedAt: 1_000_000,
      hasHeartbeat: false,
    });
    expect(idleHint(bare, 1_000_000)).toBeNull();
    expect(progressStatusLabel('running')).toBe('运行中');
    expect(progressStatusLabel('等待写锁')).toBe('等待写锁');
  });

  it('运行中一行：显示已运行与工具；胶囊用 hint-capsule，不是红字', () => {
    expect(runningDetailOf(undefined, 1_000_000)).toBe('');
    expect(runningDetailOf(snapshot({ elapsedMs: 0 }), 1_000_000)).toBe('');
    expect(runningDetailOf(snapshot({ elapsedMs: 42_000 }), 1_000_000)).toBe('已运行 42 秒');
    expect(runningDetailOf(snapshot({ elapsedMs: 80_000, lastTool: 'read_file' }), 1_000_000))
      .toBe('已运行 1 分 20 秒 · 工具 read_file');
    // 两条心跳之间也接着走
    expect(runningDetailOf(snapshot({ elapsedMs: 42_000 }), 1_000_000 + 5_000)).toBe('已运行 47 秒');

    const quiet = renderToStaticMarkup(<IdleHint progress={snapshot()} now={1_000_000} />);
    expect(quiet).toBe('');
    const loud = renderToStaticMarkup(
      <IdleHint progress={snapshot({ repeatedToolCalls: 3, idleMs: REPEATED_TOOL_IDLE_MS })} now={1_000_000} />,
    );
    expect(loud).toContain('疑似空转：同一工具连续 3 次');
    expect(loud).toContain('hint-capsule');
    expect(loud).not.toContain('dispatch-warning');
  });

  it('runningDetailOf：终态快照冻结耗时，不再跟着墙上时间涨', () => {
    // R1-4：节点跑完了，行还挂在本层列表里，读数应停在最后一条心跳上。
    const done = snapshot({ status: '完成', elapsedMs: 42_000 });
    expect(runningDetailOf(done, 1_000_000)).toBe('已运行 42 秒');
    expect(runningDetailOf(done, 1_000_000 + 5 * 60_000)).toBe('已运行 42 秒');
    expect(runningDetailOf(snapshot({ status: '失败', elapsedMs: 80_000, lastTool: 'edit_file' }), 1_000_000 + 60_000))
      .toBe('已运行 1 分 20 秒 · 工具 edit_file');
    // 还在跑的照旧继续走
    expect(runningDetailOf(snapshot({ elapsedMs: 42_000 }), 1_000_000 + 60_000)).toBe('已运行 1 分 42 秒');
  });

  it('nextProgress / progressOf：重派后不显示上一轮读数、不立刻出胶囊', () => {
    // R1-2：retry() 复用同一个 nodeId。上一轮叫停/抛错时可能收不到终态事件，
    // 快照 status 还停在 running、idleMs 很大。
    const stale: Record<string, ProgressSnapshot> = {
      n1: snapshot({ runId: 'run-1', status: 'running', elapsedMs: 900_000, idleMs: 600_000 }),
    };
    expect(idleHint(stale.n1, 1_000_000)).toBe('疑似空转：10 分钟无输出');

    // 新一轮：同一个 nodeId、新的 runId，第一条心跳还没到
    expect(progressOf(stale, 'n1', 'run-2')).toBeUndefined();
    expect(runningDetailOf(progressOf(stale, 'n1', 'run-2'), 1_000_000 + 14_000)).toBe('');
    expect(idleHint(progressOf(stale, 'n1', 'run-2'), 1_000_000 + 14_000)).toBeNull();
    expect(renderToStaticMarkup(
      <IdleHint progress={progressOf(stale, 'n1', 'run-2')} now={1_000_000 + 14_000} />,
    )).toBe('');

    // 上一轮迟到的事件直接丢掉，不会污染这一轮
    const late = nextProgress(stale, { runId: 'run-1', nodeId: 'n1', status: 'running', idleMs: 999_000 }, 1_000_001, 'run-2');
    expect(late).toBe(stale);
    // 本轮的心跳照常写入，读数从新一轮算起
    const fresh = nextProgress(stale, { runId: 'run-2', nodeId: 'n1', status: 'running', elapsedMs: 15_000, idleMs: 0 }, 1_000_002, 'run-2');
    expect(fresh.n1.runId).toBe('run-2');
    expect(runningDetailOf(progressOf(fresh, 'n1', 'run-2'), 1_000_002)).toBe('已运行 15 秒');
    expect(idleHint(progressOf(fresh, 'n1', 'run-2'), 1_000_002)).toBeNull();
  });

  it('运行行接线：按钮文案是「叫停」，走 cancelDispatchNode(runId, nodeId)；start/retry 清进度', () => {
    // 组件测要 tauri 事件桥，这里按仓库既有做法读源文件断接线（宽松正则）。
    const overlay = readSource('../../components/center/graph/DispatchOverlay.tsx');
    expect(overlay).toMatch(/cancelDispatchNode\(\s*runId\s*,\s*node\.nodeId\s*\)/);
    expect(overlay).toMatch(/>\s*叫停\s*<\/button>/);
    // 快照按 runId 过滤后再渲染，不直接取 progress[nodeId]。过滤本身搬进了
    // 共享层的 snapshotOf（TASK-07C），这里只断不再自己取。
    expect(overlay).toMatch(/progressStore\.snapshotOf\(\s*node\.nodeId\s*\)/);
    expect(overlay).not.toContain('progress[node.nodeId]');
    // start() 与 retry() 各清一次（R1-2）；清的是共享层的一轮（TASK-07C）
    expect(overlay.match(/progressStore\.beginRun\(/g)?.length).toBe(2);
    expect(overlay.match(/setCurrentNodes\(\[\]\)/g)?.length).toBe(2);
  });
});

describe('07B-2 对账说明随派发收敛', () => {
  // 根 → a、b，两个都被对账判 unknown（applyReconcile 写成 draft + 那句正文）
  function reviewGraph(): { g: Graph; a: string; b: string; ui: ReconcileUi } {
    const base = newGraph();
    const a = newNodeId();
    const b = newNodeId();
    const unknown = { status: 'draft' as const, text: RECONCILE_UNKNOWN_TEXT };
    const g: Graph = {
      ...base,
      nodes: {
        ...base.nodes,
        [a]: mkTurn(a, [base.root], { target: mkTarget(), reply: { ...unknown } }),
        [b]: mkTurn(b, [base.root], { target: mkTarget(), reply: { ...unknown } }),
      },
    };
    return { g, a, b, ui: { graphId: g.id, needsReview: [a, b], note: '已按留痕对账 2 个节点，2 个待核对' } };
  }

  it('reconcileUiConverged：派发完成的节点从待核对剔除，一个不剩时说明一起清', () => {
    const { g, a, b, ui } = reviewGraph();
    // 都还没派：原样返回，说明不动
    expect(reconcileUiConverged(ui, g)).toBe(ui);

    // b 跑完了：只剩 a，说明改成当前计数
    const afterB: Graph = { ...g, nodes: { ...g.nodes, [b]: { ...g.nodes[b], reply: { status: 'done', text: '好了' } } } };
    const one = reconcileUiConverged(ui, afterB);
    expect(one.needsReview).toEqual([a]);
    expect(one.note).toBe('还有 1 个节点待核对');

    // a 判失败：一个不剩，工具条那句陈旧说明一并清掉
    const afterAll: Graph = { ...afterB, nodes: { ...afterB.nodes, [a]: { ...afterB.nodes[a], reply: { status: 'failed', text: '' } } } };
    expect(reconcileUiConverged(ui, afterAll)).toBe(EMPTY_RECONCILE_UI);
  });

  it('reconcileUiConverged：按终态过滤——在飞的剔除，暂不可派但仍待核对的 draft 保留', () => {
    // R1-5：口径从「是否在可派集合里」改成节点自己的终态。父节点没跑完、
    // 目标没选的 unknown 节点暂时不可派，但它仍然没核对过，不能被永久剔除
    // ——否则父节点跑完后 initialDispatchSelection 会把它默认勾上派出去。
    const { g, a, b, ui } = reviewGraph();
    const blocked: Graph = {
      ...g,
      nodes: {
        ...g.nodes,
        // a 的父节点还没跑完：a 不在 dispatchableNodeIds 里
        [a]: { ...g.nodes[a], parents: [b] },
      },
    };
    expect(reconcileUiConverged(ui, blocked)).toBe(ui);

    // 目标没选（同样不可派）也保留
    const noTarget: Graph = { ...g, nodes: { ...g.nodes, [a]: { ...g.nodes[a], target: mkTarget({ model: '' }) } } };
    expect(reconcileUiConverged(ui, noTarget)).toBe(ui);

    // 正在飞（pending）的剔除
    const flying: Graph = { ...g, nodes: { ...g.nodes, [b]: { ...g.nodes[b], reply: { status: 'pending', text: '' } } } };
    expect(reconcileUiConverged(ui, flying).needsReview).toEqual([a]);

    // 用户已把那句「未知」改掉（核对过了）：也剔除，与节点卡的「待核对」同口径
    const edited: Graph = { ...g, nodes: { ...g.nodes, [b]: { ...g.nodes[b], reply: { status: 'draft', text: '已核对' } } } };
    expect(reconcileUiConverged(ui, edited).needsReview).toEqual([a]);
  });

  it('reconcileUiConverged：换图后当空的', () => {
    const { g, ui } = reviewGraph();
    const other: Graph = { ...g, id: 'another-graph' };
    expect(reconcileUiConverged(ui, other)).toBe(EMPTY_RECONCILE_UI);
    expect(reconcileUiConverged(EMPTY_RECONCILE_UI, g)).toBe(EMPTY_RECONCILE_UI);
  });

  it('GraphTab 接线：每次 onCommit 写回图之后收敛一次', () => {
    const graphTab = readSource('../../components/center/graph/GraphTab.tsx');
    expect(graphTab).toMatch(/setReconcile\(\s*\(current\)\s*=>\s*reconcileUiConverged\(current,\s*saved\)\s*\)/);
    expect(graphTab).toMatch(/const\s+saved\s*=\s*commit\(next\)/);
  });
});

describe('07B-3 Reply.tokens_cache_read', () => {
  const base: BackendNodeResult = {
    nodeId: '', status: 'ok', text: '正文', modelReported: '', sessionId: '',
    tokensIn: 10, tokensOut: 20, durationMs: 5, error: '',
  };

  function oneDraft(): { g: Graph; id: string } {
    const g0 = newGraph();
    const id = newNodeId();
    return {
      g: { ...g0, nodes: { ...g0.nodes, [id]: mkTurn(id, [g0.root], { target: mkTarget(), reply: { status: 'pending', text: '' } }) } },
      id,
    };
  }

  it('applyDispatchResults 写 tokens_cache_read，0 与缺失都不写', () => {
    const { g, id } = oneDraft();
    expect(applyDispatchResults(g, [{ ...base, nodeId: id, cacheRead: 4096 }]).nodes[id].reply!.tokens_cache_read)
      .toBe(4096);
    expect(applyDispatchResults(g, [{ ...base, nodeId: id, cacheRead: 0 }]).nodes[id].reply!.tokens_cache_read)
      .toBeUndefined();
    expect(applyDispatchResults(g, [{ ...base, nodeId: id }]).nodes[id].reply!.tokens_cache_read)
      .toBeUndefined();
  });

  it('applyReconcile 收回 usage.cacheRead', () => {
    const { g, id } = oneDraft();
    const summary: ReconcileSummary = {
      exists: true,
      runId: 'run-1',
      nodes: [{ nodeId: id, verdict: 'completed', replyText: '跑完了', usage: { input: 10, output: 20, cacheRead: 512 } }],
    };
    expect(applyReconcile(g, summary).nodes[id].reply!.tokens_cache_read).toBe(512);
  });

  it('cacheReadOf：结果屏优先读图字段，图上没有才用本次结果', () => {
    expect(cacheReadOf({ status: 'done', text: '', tokens_cache_read: 512 }, { cacheRead: 0 })).toBe(512);
    expect(cacheReadOf({ status: 'done', text: '' }, { cacheRead: 64 })).toBe(64);
    expect(cacheReadOf(undefined, { cacheRead: 64 })).toBe(64);
    expect(cacheReadOf(undefined, {})).toBe(0);
  });
});

describe('07B-4 start() 幂等停止分支整链', () => {
  // 根 → a → b：两层，每层一路。
  function twoStages(): { g: Graph; a: string; b: string } {
    const base = newGraph();
    const a = newNodeId();
    const b = newNodeId();
    return {
      g: {
        ...base,
        nodes: {
          ...base.nodes,
          [a]: mkTurn(a, [base.root], { target: mkTarget(), reply: { status: 'draft', text: '' } }),
          [b]: mkTurn(b, [a], { target: mkTarget(), reply: { status: 'draft', text: '' } }),
        },
      },
      a,
      b,
    };
  }

  function okResult(nodeId: string): BackendNodeResult {
    return { nodeId, stage: 1, status: 'ok', text: '好了', modelReported: '', sessionId: '', tokensIn: 0, tokensOut: 0, durationMs: 1, error: '' };
  }

  // 每层一个请求：stage 固定 1，模拟引擎把同一批结果重放一次时的批次 id。
  const deps = (dispatch: (req: { stage: number; nodes: { nodeId: string }[] }) => Promise<{ runId: string; results: BackendNodeResult[] }>) => {
    const previews: number[] = [];
    const stages: string[][] = [];
    const results: BackendNodeResult[][] = [];
    return {
      previews,
      stages,
      results,
      deps: {
        runId: 'run-1',
        buildRequest: (_graph: Graph, ids: string[]) => ({ stage: 1, nodes: ids.map((nodeId) => ({ nodeId })) }),
        preview: async (req: { nodes: { nodeId: string }[] }) => { previews.push(req.nodes.length); return req.nodes.length; },
        dispatch,
        commit: (graph: Graph) => graph,
        onPreview: () => {},
        onStage: (_req: unknown, ids: string[]) => { stages.push(ids); },
        onResults: (collected: BackendNodeResult[]) => { results.push([...collected]); },
      },
    };
  };

  it('runStageLoop：重复批次 → 本层放回 draft、给提示，且不再推进下一层', async () => {
    const { g, a, b } = twoStages();
    const state: StageLoopState<BackendNodeResult> = { graph: g, collected: [], activeNodes: [] };
    let calls = 0;
    // 第二层派 b，引擎却把第一层那批（a）原样重放回来：批次 id 与第一层相同。
    const harness = deps(async () => { calls += 1; return { runId: 'run-1', results: [okResult(a)] }; });
    const outcome = await runStageLoop(state, { maxRequests: 10 }, new Set<string>(), harness.deps);

    // 文案用字面量断，改错不会被同一个常量自证
    expect(outcome).toEqual({
      kind: 'stopped',
      error: '同一批结果重复送达，已停止推进；这一层的节点已放回待派。',
    });
    expect(DUPLICATE_BATCH_TEXT).toBe('同一批结果重复送达，已停止推进；这一层的节点已放回待派。');
    expect(calls).toBe(2);
    expect(harness.stages).toEqual([[a], [b]]);
    // 本层刚 markPending 的 b 放回 draft，不留悬挂的 pending
    expect(state.graph.nodes[b].reply!.status).toBe('draft');
    expect(state.graph.nodes[a].reply!.status).toBe('done');
    // 重复那批不收进结果
    expect(state.collected.map((result) => result.nodeId)).toEqual([a]);
  });

  it('runStageLoop：批次不重复时照常推进两层后收工', async () => {
    const { g, a, b } = twoStages();
    const state: StageLoopState<BackendNodeResult> = { graph: g, collected: [], activeNodes: [] };
    const harness = deps(async (req) => ({ runId: 'run-1', results: req.nodes.map((node) => okResult(node.nodeId)) }));
    const outcome = await runStageLoop(state, { maxRequests: 10 }, new Set<string>(), harness.deps);

    expect(outcome).toEqual({ kind: 'done' });
    expect(harness.stages).toEqual([[a], [b]]);
    expect(state.collected.map((result) => result.nodeId)).toEqual([a, b]);
    expect(state.graph.nodes[b].reply!.status).toBe('done');
    expect(state.activeNodes).toEqual([{ nodeId: b }]);
  });

  it('runStageLoop：下一层缺目标时回确认屏，不发这一层', async () => {
    const { g, a, b } = twoStages();
    const noTarget: Graph = { ...g, nodes: { ...g.nodes, [b]: { ...g.nodes[b], target: mkTarget({ model: '' }) } } };
    const state: StageLoopState<BackendNodeResult> = { graph: noTarget, collected: [], activeNodes: [] };
    const harness = deps(async (req) => ({ runId: 'run-1', results: req.nodes.map((node) => okResult(node.nodeId)) }));
    const outcome = await runStageLoop(state, { maxRequests: 10 }, new Set<string>(), harness.deps);

    expect(outcome).toEqual({ kind: 'missing-target', nodeIds: [b], error: '下一阶段有节点未指定模型，请先选择目标。' });
    expect(harness.stages).toEqual([[a]]);
  });

  it('runStageLoop：达总请求上限时停在本层之前，给上限文案且不再派发', async () => {
    const { g, a, b } = twoStages();
    const state: StageLoopState<BackendNodeResult> = { graph: g, collected: [], activeNodes: [] };
    let calls = 0;
    const harness = deps(async (req) => {
      calls += 1;
      return { runId: 'run-1', results: req.nodes.map((node) => okResult(node.nodeId)) };
    });
    // 上限 1：第一层派得出去，第二层（b）超限
    const outcome = await runStageLoop(state, { maxRequests: 1 }, new Set<string>(), harness.deps);

    expect(outcome).toEqual({ kind: 'stopped', error: '已达总请求上限 1，继续派发需重新确认。' });
    expect(calls).toBe(1);
    expect(harness.stages).toEqual([[a]]);
    // 超限那层一次都没 markPending，b 仍是 draft
    expect(state.graph.nodes[b].reply!.status).toBe('draft');
    expect(state.collected.map((result) => result.nodeId)).toEqual([a]);
  });

  it('runStageLoop：dispatch 抛错时 state 留下本层 activeNodes 与已收结果，供组件 catch 收尾', async () => {
    const { g, a, b } = twoStages();
    const state: StageLoopState<BackendNodeResult> = { graph: g, collected: [], activeNodes: [] };
    let calls = 0;
    const harness = deps(async (req) => {
      calls += 1;
      if (calls === 2) throw new Error('引擎挂了');
      return { runId: 'run-1', results: req.nodes.map((node) => okResult(node.nodeId)) };
    });
    await expect(runStageLoop(state, { maxRequests: 10 }, new Set<string>(), harness.deps))
      .rejects.toThrow('引擎挂了');

    // 抛错那层的节点留在 activeNodes 上（组件据此补 failed 结果行）
    expect(state.activeNodes).toEqual([{ nodeId: b }]);
    // 已收的结果与已写回的图都还在
    expect(state.collected.map((result) => result.nodeId)).toEqual([a]);
    expect(state.graph.nodes[a].reply!.status).toBe('done');
    expect(state.graph.nodes[b].reply!.status).toBe('pending');
  });
});

describe('07B-5 小质量', () => {
  it('global.d.ts 不再手写 node:fs 声明', () => {
    const globals = readSource('../../global.d.ts');
    // 引号写法不限（R1-7：别只挡单引号）
    expect(globals).not.toMatch(/declare\s+module\s+['"](node:)?fs['"]/);
  });

  it('常规提示不用红字：单步小任务 / 复核，非独立验证 / 已切备选 都走 dispatch-reason', () => {
    const overlay = readSource('../../components/center/graph/DispatchOverlay.tsx');
    // R1-7：不再按「同一行」找，允许 className 与文案之间换行/重排
    for (const text of ['单步小任务', '复核，非独立验证', '已切备选']) {
      const near = new RegExp(`className="dispatch-reason"[^<]*${text}`);
      const red = new RegExp(`className="dispatch-warning"[^<]*${text}`);
      expect(overlay).toMatch(near);
      expect(overlay).not.toMatch(red);
    }
    // 异常仍然是红的
    expect(overlay).toMatch(/className="dispatch-warning"\s*>\s*未指定模型，不能确认派发。/);
  });

  it('hint-capsule 的样式注释改成「提示胶囊」（校验提示与疑似空转共用）', () => {
    const css = readSource('../../components/center/graph/GraphTab.css');
    expect(css).toMatch(/\/\*\s*提示胶囊[^*]*\*\/\s*\.hint-capsule\s*\{/);
  });
});
