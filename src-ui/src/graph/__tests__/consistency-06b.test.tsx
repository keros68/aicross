// TASK-06 B 节（界面/TS）：重派与崩溃恢复的一致性。
// 编号与 tasks/TASK-06-consistency.md「B. 界面（TS）」1–7 一致。

// tsconfig.app.json 的 types 里只有 vite/client：应用代码不该碰 node 内置模块，
// 把 @types/node 拉进来会连带改写 setTimeout 的返回类型。只有这条样式断言要读
// 磁盘，所以就地压掉解析错误（与 lib/__tests__/shiki.test.ts 同一套写法）。
// @ts-expect-error node builtin, not in this project's `types`
import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import {
  applyDispatchResults,
  applyReconcile,
  buildNodeRequest,
  clampReconcileText,
  continuationSession,
  dirtyWorktreeHintOf,
  dispatchBatchKey,
  fallbackNoticeOf,
  initialDispatchSelection,
  markBatchProcessed,
  nextStageStep,
  nodeStatusLabel,
  promptWithWriteGuard,
  reconcileNeedsReview,
  reconcileUiAfter,
  reconcileUiFor,
  resetNodesToDraft,
  resetPendingToDraft,
  rollbackBeforeOf,
  rollbackHintOf,
  toggleRollbackOnReview,
  EMPTY_RECONCILE_UI,
  NO_EXTERNAL_ACTIONS,
  RECONCILE_TEXT_LIMIT,
  RECONCILE_UNKNOWN_TEXT,
  type BackendNodeResult,
  type NodeRequestParts,
  type ReconcileSummary,
} from '../dispatch.ts';
import { attachmentDraftState, parseAttachmentLines, premiseCheckBlockReason } from '../premise.ts';
import { newGraph, newNodeId } from '../types.ts';
import type { Graph, GraphNode, Reply, Target } from '../types.ts';
import type { GitFileEntry } from '../../tauri.ts';
import {
  ReviewPendingRow,
  ReviewPendingTag,
  RollbackBeforeHint,
} from '../../components/center/graph/DispatchBits.tsx';

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

function gitEntry(rel: string): GitFileEntry {
  return { path: `D:/repo/${rel}`, rel, status: 'M', added: 1, deleted: 0 };
}

// 一张「根 → 三个派发中节点」的图，覆盖对账的三种判定。
function pendingGraph(): { g: Graph; ok: string; bad: string; ghost: string } {
  const base = newGraph();
  const ok = newNodeId();
  const bad = newNodeId();
  const ghost = newNodeId();
  const pending: Reply = { status: 'pending', text: '' };
  return {
    g: {
      ...base,
      nodes: {
        ...base.nodes,
        [ok]: mkTurn(ok, [base.root], { target: mkTarget(), reply: { ...pending } }),
        [bad]: mkTurn(bad, [base.root], { target: mkTarget(), reply: { ...pending } }),
        [ghost]: mkTurn(ghost, [base.root], { target: mkTarget({ write: '单写者' }), reply: { ...pending } }),
      },
    },
    ok,
    bad,
    ghost,
  };
}

describe('06B-1 崩溃后「继续」走对账', () => {
  it('applyReconcile：completed 收回正文与产物', () => {
    const { g, ok } = pendingGraph();
    const summary: ReconcileSummary = {
      exists: true,
      runId: 'run-1',
      nodes: [{
        nodeId: ok,
        verdict: 'completed',
        recordPath: '.dispatch/2026-09-14-ok.md',
        replyText: '已按要求改完三个文件。',
        artifacts: { diff: 'diff --git a/x b/x', changed_files: [{ path: 'x', content: '1' }], stdout_summary: '' },
        error: '',
      }],
    };
    const next = applyReconcile(g, summary);
    const reply = next.nodes[ok].reply!;
    expect(reply.status).toBe('done');
    expect(reply.text).toBe('已按要求改完三个文件。');
    expect(reply.artifacts?.changed_files).toHaveLength(1);
  });

  it('applyReconcile：incomplete 回 draft 重跑', () => {
    const { g, bad } = pendingGraph();
    const next = applyReconcile(g, {
      exists: true,
      runId: 'run-1',
      nodes: [{ nodeId: bad, verdict: 'incomplete', recordPath: '.dispatch/x.md', replyText: '', artifacts: null, error: 'timeout' }],
    });
    expect(next.nodes[bad].reply).toEqual({ status: 'draft', text: '' });
  });

  it('applyReconcile：unknown 回 draft 并写「未知」说明，reconcileNeedsReview 列出它', () => {
    const { g, ghost } = pendingGraph();
    const summary: ReconcileSummary = {
      exists: true,
      runId: 'run-1',
      nodes: [{ nodeId: ghost, verdict: 'unknown', recordPath: '', replyText: '', artifacts: null, error: '' }],
    };
    const next = applyReconcile(g, summary);
    expect(next.nodes[ghost].reply?.status).toBe('draft');
    expect(next.nodes[ghost].reply?.text).toBe(RECONCILE_UNKNOWN_TEXT);
    expect(reconcileNeedsReview(summary)).toEqual([ghost]);
  });

  it('applyReconcile：exists:false 原样返回（后端未接通时的回退分支）', () => {
    const { g } = pendingGraph();
    expect(applyReconcile(g, { exists: false, runId: '', nodes: [] })).toBe(g);
    expect(applyReconcile(g, null)).toBe(g);
  });

  it('resetPendingToDraft：对账前先把 pending 放回 draft，done 的不动', () => {
    const { g, ok, bad } = pendingGraph();
    const withDone = { ...g, nodes: { ...g.nodes, [ok]: { ...g.nodes[ok], reply: { status: 'done' as const, text: '旧结果' } } } };
    const next = resetPendingToDraft(withDone);
    expect(next.nodes[ok].reply?.status).toBe('done');
    expect(next.nodes[bad].reply?.status).toBe('draft');
  });

  it('G1：待核对节点默认不勾选，标签渲染出「待核对」', () => {
    const { ok, bad, ghost } = pendingGraph();
    expect(initialDispatchSelection([ok, bad, ghost], [ghost])).toEqual([ok, bad]);
    expect(initialDispatchSelection([ok, bad, ghost])).toEqual([ok, bad, ghost]);
    expect(renderToStaticMarkup(<ReviewPendingTag visible />)).toContain('待核对');
    expect(renderToStaticMarkup(<ReviewPendingTag visible={false} />)).toBe('');
  });
});

describe('06B-2 单写者重派回滚', () => {
  it('buildRequest 的 rollbackBefore：单写者重派带，只读/worktree 不带', () => {
    expect(rollbackBeforeOf(mkTarget({ write: '单写者' }), true)).toBe(true);
    expect(rollbackBeforeOf(mkTarget({ write: '只读' }), true)).toBeUndefined();
    expect(rollbackBeforeOf(mkTarget({ write: 'worktree' }), true)).toBeUndefined();
    // 首次派发（非重派）不回滚
    expect(rollbackBeforeOf(mkTarget({ write: '单写者' }), false)).toBeUndefined();
  });

  it('G1 提示：单写者重派显示「将先回滚上次改动」', () => {
    expect(rollbackHintOf(mkTarget({ write: '单写者' }), true)).toBe('将先回滚上次改动');
    expect(rollbackHintOf(mkTarget({ write: '只读' }), true)).toBeNull();
    const html = renderToStaticMarkup(<RollbackBeforeHint target={mkTarget({ write: '单写者' })} retrying />);
    expect(html).toContain('将先回滚上次改动');
    expect(renderToStaticMarkup(<RollbackBeforeHint target={mkTarget({ write: 'worktree' })} retrying />)).toBe('');
  });

  it('G1 未提交改动提示：git 状态读不出来时不显示', () => {
    expect(dirtyWorktreeHintOf({ state: 'ok', uncommitted: [gitEntry('a'), gitEntry('b')], untracked: [gitEntry('c')] }, true))
      .toBe('项目目录有 3 处未提交改动；worktree 从 HEAD 新建，看不到这些改动');
    expect(dirtyWorktreeHintOf({ state: 'ok', uncommitted: [], untracked: [] }, true)).toBeNull();
    expect(dirtyWorktreeHintOf({ state: 'not_repo' }, true)).toBeNull();
    expect(dirtyWorktreeHintOf(null, true)).toBeNull();
  });
});

describe('06B-3 Reply.fallback 字段', () => {
  const base: BackendNodeResult = {
    nodeId: '', status: 'ok', text: '备选正文', modelReported: '', sessionId: '',
    tokensIn: 0, tokensOut: 0, durationMs: 0, error: '',
  };

  it('applyDispatchResults 写 fallback_from/to，正文不再前置标记行', () => {
    const { g, ok } = pendingGraph();
    const next = applyDispatchResults(g, [{ ...base, nodeId: ok, fallbackFrom: 'pi/glm', fallbackTo: 'codex/gpt' }]);
    const reply = next.nodes[ok].reply!;
    expect(reply.text).toBe('备选正文');
    expect(reply.text.startsWith('（已切备选')).toBe(false);
    expect(reply.fallback_from).toBe('pi/glm');
    expect(reply.fallback_to).toBe('codex/gpt');
    expect(fallbackNoticeOf(reply)).toBe('（已切备选：主 pi/glm → 备 codex/gpt）');
  });

  it('fallbackNoticeOf 兼容旧图：只有正文首行标记时仍识别', () => {
    expect(fallbackNoticeOf({ status: 'done', text: '（已切备选：主 pi/glm → 备 codex/gpt）\n\n正文' }))
      .toBe('（已切备选：主 pi/glm → 备 codex/gpt）');
    expect(fallbackNoticeOf({ status: 'done', text: '普通正文' })).toBeNull();
  });
});

describe('06B-4 G1 备选下拉溢出', () => {
  // 这条只证明规则写在 CSS 里；1568px 下四个控件不溢出卡片是视觉事实，
  // 人工核对待补（本期不起窗口）。选择器改成正则，格式化或换行不会假失败。
  it('.dispatch-controls 可折行，控件有宽度上限', () => {
    const graphTabCss = readFileSync(
      new URL('../../components/center/graph/GraphTab.css', import.meta.url),
      'utf8',
    );
    const rule = /\.dispatch-controls\s*\{[^}]*\}/.exec(graphTabCss)?.[0] ?? '';
    expect(rule).toContain('flex-wrap: wrap');
    const field = /\.dispatch-controls input,\s*\.dispatch-controls select\s*\{[^}]*\}/.exec(graphTabCss)?.[0] ?? '';
    expect(field).toContain('max-width');
    // R1-13：待核对行的间距回到 4 的倍数
    const reviewRow = /\.dispatch-review-row\s*\{[^}]*\}/.exec(graphTabCss)?.[0] ?? '';
    expect(reviewRow).toContain('margin-top: 8px');
  });
});

describe('06B-5 前提附件输入', () => {
  it('每行一条：含逗号的路径不被拆坏，空行丢掉', () => {
    expect(parseAttachmentLines('D:/a,b/x.md\n  D:/c/y.md  \n\n')).toEqual(['D:/a,b/x.md', 'D:/c/y.md']);
    expect(parseAttachmentLines('')).toEqual([]);
  });

  it('「抽查」前再校验一次：不通过就不建图并给提示', () => {
    expect(premiseCheckBlockReason({ attachments: ['docs/api.md'] }))
      .toBe('附件不合规，未建图：附件需用绝对路径：docs/api.md');
    expect(premiseCheckBlockReason({ attachments: ['D:/a/x.md'] })).toBeNull();
    expect(premiseCheckBlockReason({ attachments: [] })).toBeNull();
  });
});

describe('06B-6 盲父节点不传 session/runner', () => {
  it('父节点 visibility=盲 时 continuationSession 返回空', () => {
    const base = newGraph();
    const parent = newNodeId();
    const child = newNodeId();
    const build = (visibility: Target['visibility']): Graph => ({
      ...base,
      nodes: {
        ...base.nodes,
        [parent]: mkTurn(parent, [base.root], {
          target: mkTarget({ visibility }),
          reply: { status: 'done', text: '正文', session_id: 'sess-1' },
        }),
        [child]: mkTurn(child, [parent], { target: mkTarget(), reply: { status: 'draft', text: '' } }),
      },
    });
    expect(continuationSession(build('盲'), child)).toBeUndefined();
    expect(continuationSession(build('共享'), child)).toBe('sess-1');
  });
});

describe('06B-7 推进幂等', () => {
  it('同一批结果送达两次只推进一次', () => {
    const rows = [{ nodeId: 'b' }, { nodeId: 'a' }];
    const key = dispatchBatchKey('run-1', 1, rows);
    // 节点顺序不影响批次 id
    expect(dispatchBatchKey('run-1', 1, [{ nodeId: 'a' }, { nodeId: 'b' }])).toBe(key);
    const first = markBatchProcessed(new Set<string>(), key);
    expect(first.fresh).toBe(true);
    const second = markBatchProcessed(first.seen, key);
    expect(second.fresh).toBe(false);
    // 换阶段或换 run 是另一批，照常推进
    expect(markBatchProcessed(first.seen, dispatchBatchKey('run-1', 2, rows)).fresh).toBe(true);
    expect(markBatchProcessed(first.seen, dispatchBatchKey('run-2', 1, rows)).fresh).toBe(true);
  });
});

// ── 修正轮 R1（tasks/fix-06b-r1.md 1–13）────────────────────────────────────

function reqParts(patch: Partial<NodeRequestParts> = {}): NodeRequestParts {
  return {
    models: [],
    prompt: '任务正文',
    materials: [],
    currentMessage: '',
    originalRequest: '原始请求',
    upstreamArtifacts: [],
    sessionId: '',
    previousRunner: '',
    rollback: false,
    stage: 1,
    ...patch,
  };
}

describe('06B-R1-1 换图重置对账状态', () => {
  it('reconcileUiFor：图 id 变了就当空的，同一张图原样保留', () => {
    const ui = reconcileUiAfter('g1', {
      exists: true,
      runId: 'run-1',
      nodes: [{ nodeId: 'a', verdict: 'unknown' }],
    });
    expect(ui.needsReview).toEqual(['a']);
    expect(ui.note).toBe('已按派发记录核对 1 个节点，1 个待核对');
    expect(reconcileUiFor(ui, 'g1')).toBe(ui);
    expect(reconcileUiFor(ui, 'g2')).toBe(EMPTY_RECONCILE_UI);
    expect(reconcileUiFor(EMPTY_RECONCILE_UI, 'g1')).toBe(EMPTY_RECONCILE_UI);
  });

  it('reconcileUiAfter：对不上账时给回退说明，不列待核对', () => {
    expect(reconcileUiAfter('g1', null)).toEqual({
      graphId: 'g1',
      needsReview: [],
      note: '未能核对，按上次的待派节点重跑',
    });
  });
});

describe('06B-R1-2 收回 completed 节点的 session/用量/runner', () => {
  it('applyReconcile：completed 写回 session_id / runner / tokens，缺失与非法值不写', () => {
    const { g, ok, bad } = pendingGraph();
    const next = applyReconcile(g, {
      exists: true,
      runId: 'run-1',
      nodes: [
        {
          nodeId: ok,
          verdict: 'completed',
          replyText: '正文',
          sessionId: 'sess-9',
          runner: 'acp',
          usage: { input: 120, output: 34, cacheRead: 7 },
        },
        { nodeId: bad, verdict: 'completed', replyText: '正文', runner: '乱写的' },
      ],
    });
    const adopted = next.nodes[ok].reply!;
    expect(adopted.session_id).toBe('sess-9');
    expect(adopted.runner).toBe('acp');
    expect(adopted.tokens_in).toBe(120);
    expect(adopted.tokens_out).toBe(34);
    const missing = next.nodes[bad].reply!;
    expect(missing.session_id).toBeUndefined();
    expect('runner' in missing).toBe(false);
    expect('tokens_in' in missing).toBe(false);
  });
});

describe('06B-R1-3 重复批次不留 pending', () => {
  it('resetNodesToDraft：停止推进前把本层放回 draft，done 的不动', () => {
    const { g, ok, bad } = pendingGraph();
    const withDone = { ...g, nodes: { ...g.nodes, [ok]: { ...g.nodes[ok], reply: { status: 'done' as const, text: '结果' } } } };
    const next = resetNodesToDraft(withDone, [ok, bad]);
    expect(next.nodes[ok].reply?.status).toBe('done');
    expect(next.nodes[bad].reply?.status).toBe('draft');
    // 传进来的这一层不留 pending（图上别的层不动）
    expect([ok, bad].some((id) => next.nodes[id].reply?.status === 'pending')).toBe(false);
    // 本层没有 pending 时原样返回同一引用
    expect(resetNodesToDraft(next, [ok, bad])).toBe(next);
  });
});

describe('06B-R1-4 applyReconcile 前置条件', () => {
  it('只覆写 pending / draft；done 与 failed 的节点不被旧留痕抹掉', () => {
    const { g, ok, bad, ghost } = pendingGraph();
    const graph: Graph = {
      ...g,
      nodes: {
        ...g.nodes,
        [ok]: { ...g.nodes[ok], reply: { status: 'done', text: '重派后的新结果', session_id: 'sess-new' } },
        [bad]: { ...g.nodes[bad], reply: { status: 'failed', text: '已判失败' } },
      },
    };
    const next = applyReconcile(graph, {
      exists: true,
      runId: 'run-1',
      nodes: [
        { nodeId: ok, verdict: 'incomplete' },
        { nodeId: bad, verdict: 'unknown' },
        { nodeId: ghost, verdict: 'unknown' },
      ],
    });
    expect(next.nodes[ok].reply).toEqual({ status: 'done', text: '重派后的新结果', session_id: 'sess-new' });
    expect(next.nodes[bad].reply).toEqual({ status: 'failed', text: '已判失败' });
    expect(next.nodes[ghost].reply?.text).toBe(RECONCILE_UNKNOWN_TEXT);
  });
});

describe('06B-R1-5 buildRequest 与组合链', () => {
  it('buildNodeRequest：单写者重派带 rollbackBefore，只读 / worktree 无该键', () => {
    const writer = buildNodeRequest('n1', mkTarget({ write: '单写者' }), reqParts({ rollback: true }));
    expect(writer.rollbackBefore).toBe(true);
    expect('rollbackBefore' in JSON.parse(JSON.stringify(writer))).toBe(true);
    expect('rollbackBefore' in buildNodeRequest('n2', mkTarget({ write: '只读' }), reqParts({ rollback: true }))).toBe(false);
    expect('rollbackBefore' in buildNodeRequest('n3', mkTarget({ write: 'worktree' }), reqParts({ rollback: true }))).toBe(false);
    // 首次派发（非重派）不带
    expect('rollbackBefore' in buildNodeRequest('n4', mkTarget({ write: '单写者' }), reqParts())).toBe(false);
  });

  it('buildNodeRequest：fallback 与 rollbackBefore 并存时形状正确', () => {
    const req = buildNodeRequest(
      'n1',
      mkTarget({ write: '单写者', fallback: { agent: 'codex', model: 'gpt' } }),
      reqParts({ rollback: true, stage: 2, sessionId: 'sess-1', previousRunner: 'acp' }),
    );
    expect(req.fallback).toEqual({ agent: 'codex', provider: '', model: 'gpt', vendor: '' });
    expect(req.rollbackBefore).toBe(true);
    expect(req.stage).toBe(2);
    expect(req.sessionId).toBe('sess-1');
    expect(req.previousRunner).toBe('acp');
    expect(req.write).toBe('单写者');
  });

  it('continueUnfinished 组合链：三种 verdict 一次写回；summary 为 null 时全部 pending→draft', () => {
    const { g, ok, bad, ghost } = pendingGraph();
    const next = applyReconcile(resetPendingToDraft(g), {
      exists: true,
      runId: 'run-1',
      nodes: [
        { nodeId: ok, verdict: 'completed', replyText: '完成' },
        { nodeId: bad, verdict: 'incomplete' },
        { nodeId: ghost, verdict: 'unknown' },
      ],
    });
    expect(next.nodes[ok].reply).toEqual({ status: 'done', text: '完成' });
    expect(next.nodes[bad].reply).toEqual({ status: 'draft', text: '' });
    expect(next.nodes[ghost].reply).toEqual({ status: 'draft', text: RECONCILE_UNKNOWN_TEXT });
    const fallback = applyReconcile(resetPendingToDraft(g), null);
    expect(Object.values(fallback.nodes).every((node) => node.reply?.status !== 'pending')).toBe(true);
    expect(reconcileNeedsReview(null)).toEqual([]);
  });

  it('applyReconcile 兜底：artifacts 为 null 不写该键，非法 verdict 按 unknown', () => {
    const { g, ok, bad } = pendingGraph();
    const next = applyReconcile(g, {
      exists: true,
      runId: 'run-1',
      nodes: [
        { nodeId: ok, verdict: 'completed', replyText: '正文', artifacts: null },
        { nodeId: bad, verdict: '说不清' as ReconcileSummary['nodes'][number]['verdict'] },
      ],
    });
    expect('artifacts' in next.nodes[ok].reply!).toBe(false);
    expect(next.nodes[bad].reply).toEqual({ status: 'draft', text: RECONCILE_UNKNOWN_TEXT });
  });
});

describe('06B-R1-7 待核对节点不进实际派发集合', () => {
  it('nextStageStep：未勾选的待核对节点不进第一层，后续层也不被捡起来', () => {
    const { g, ok, bad, ghost } = pendingGraph();
    const graph = applyReconcile(resetPendingToDraft(g), {
      exists: true,
      runId: 'run-1',
      nodes: [
        { nodeId: ok, verdict: 'incomplete' },
        { nodeId: bad, verdict: 'incomplete' },
        { nodeId: ghost, verdict: 'unknown' },
      ],
    });
    const skip = new Set([ghost]);
    const first = nextStageStep(graph, [], { maxRequests: 10 }, skip);
    expect(first.kind).toBe('dispatch');
    expect(new Set(first.nodeIds)).toEqual(new Set([ok, bad]));
    expect(first.nodeIds).not.toContain(ghost);
    // 本层跑完后再算一次：待核对节点还是 draft，但仍不进集合
    const after = applyDispatchResults(graph, [ok, bad].map((nodeId) => ({
      nodeId, status: 'ok', text: '结果', modelReported: '', sessionId: '',
      tokensIn: 0, tokensOut: 0, durationMs: 0, error: '',
    })));
    const second = nextStageStep(after, [{ nodeId: ok }, { nodeId: bad }], { maxRequests: 10 }, skip);
    expect(second.kind).toBe('done');
    // 对照：不传 skip 时它会被捡起来——这正是本条要挡的
    expect(nextStageStep(after, [], { maxRequests: 10 }).nodeIds).toEqual([ghost]);
  });
});

describe('06B-R1-8 勾回待核对节点带 rollbackBefore', () => {
  it('toggleRollbackOnReview：勾上时单写者进集合，只读不进，取消勾选撤掉', () => {
    const empty = new Set<string>();
    expect([...toggleRollbackOnReview(empty, mkTarget({ write: '单写者' }), 'n1', true)]).toEqual(['n1']);
    expect([...toggleRollbackOnReview(empty, mkTarget({ write: '只读' }), 'n1', true)]).toEqual([]);
    expect([...toggleRollbackOnReview(empty, mkTarget({ write: 'worktree' }), 'n1', true)]).toEqual([]);
    expect([...toggleRollbackOnReview(new Set(['n1']), mkTarget({ write: '单写者' }), 'n1', false)]).toEqual([]);
    // 勾回来的单写者节点，请求体确实带上了这个字段
    const target = mkTarget({ write: '单写者' });
    const rollback = toggleRollbackOnReview(empty, target, 'n1', true);
    expect(buildNodeRequest('n1', target, reqParts({ rollback: rollback.has('n1') })).rollbackBefore).toBe(true);
  });
});

describe('06B-R1-9 对账清旧 session', () => {
  it('incomplete / unknown 不保留上一条的 session_id / runner，unknown 不能被子节点续会话', () => {
    const base = newGraph();
    const parent = newNodeId();
    const child = newNodeId();
    const graph: Graph = {
      ...base,
      nodes: {
        ...base.nodes,
        [parent]: mkTurn(parent, [base.root], {
          target: mkTarget(),
          reply: { status: 'pending', text: '', session_id: 'sess-1', runner: 'acp' },
        }),
        [child]: mkTurn(child, [parent], { target: mkTarget(), reply: { status: 'draft', text: '' } }),
      },
    };
    const unknown = applyReconcile(graph, {
      exists: true, runId: 'run-1', nodes: [{ nodeId: parent, verdict: 'unknown' }],
    });
    expect(unknown.nodes[parent].reply?.session_id).toBeUndefined();
    expect(unknown.nodes[parent].reply?.runner).toBeUndefined();
    expect(continuationSession(unknown, child)).toBeUndefined();
    const incomplete = applyReconcile(graph, {
      exists: true, runId: 'run-1', nodes: [{ nodeId: parent, verdict: 'incomplete' }],
    });
    expect(incomplete.nodes[parent].reply).toEqual({ status: 'draft', text: '' });
  });
});

describe('06B-R1-10 unknown 说明可见', () => {
  it('nodeStatusLabel 对待核对节点返回「待核对」，G1 未勾选也渲染路由行与标签', () => {
    const draft = mkTurn('n1', [], { target: mkTarget(), reply: { status: 'draft', text: RECONCILE_UNKNOWN_TEXT } });
    expect(nodeStatusLabel(draft)).toBe('待核对');
    expect(nodeStatusLabel(mkTurn('n2', [], { target: mkTarget(), reply: { status: 'draft', text: '' } }))).toBe('待派');
    const html = renderToStaticMarkup(
      <ReviewPendingRow line="[aicross] 执行 → pi/glm" checked={false} onToggle={() => {}} />,
    );
    expect(html).toContain('pi/glm');
    expect(html).toContain('待核对');
  });
});

describe('06B-R1-11 对账正文上限', () => {
  it('clampReconcileText：超限截断并说明，未超限原样', () => {
    expect(clampReconcileText('短正文')).toBe('短正文');
    const long = 'x'.repeat(RECONCILE_TEXT_LIMIT * 4);
    const clamped = clampReconcileText(long);
    // 留痕正文被截到上限，后面接一句说明（说明本身几十字，不随输入增长）
    expect(clamped.length).toBeLessThan(RECONCILE_TEXT_LIMIT + 100);
    expect(clamped.startsWith('x'.repeat(RECONCILE_TEXT_LIMIT))).toBe(true);
    expect(clamped).toContain('已截断');
    const { g, ok } = pendingGraph();
    const next = applyReconcile(g, {
      exists: true, runId: 'run-1', nodes: [{ nodeId: ok, verdict: 'completed', replyText: long }],
    });
    expect(next.nodes[ok].reply!.text).toBe(clamped);
  });
});

describe('06B-R1-12 禁外部动作按「可写」挂', () => {
  it('自定义角色的可写节点也带这句，只读不带，模板里已有不重复', () => {
    const custom = buildNodeRequest('n1', mkTarget({ role: '打杂', write: '单写者' }), reqParts());
    expect(custom.prompt).toContain(NO_EXTERNAL_ACTIONS);
    const readOnly = buildNodeRequest('n2', mkTarget({ role: '打杂', write: '只读' }), reqParts());
    expect(readOnly.prompt).not.toContain(NO_EXTERNAL_ACTIONS);
    const worktree = buildNodeRequest('n3', mkTarget({ role: '执行', write: 'worktree' }), reqParts());
    expect(worktree.prompt).toContain(NO_EXTERNAL_ACTIONS);
    const already = promptWithWriteGuard(`角色模板\n\n${NO_EXTERNAL_ACTIONS}`, '单写者');
    expect(already.split(NO_EXTERNAL_ACTIONS)).toHaveLength(2);
  });
});

describe('06B-R1-13 小项', () => {
  it('未提交改动提示只在本批含 worktree 节点时显示', () => {
    const changes = { state: 'ok' as const, uncommitted: [gitEntry('a')], untracked: [] };
    expect(dirtyWorktreeHintOf(changes, true)).toContain('未提交改动');
    expect(dirtyWorktreeHintOf(changes, false)).toBeNull();
  });

  it('附件输入框：草稿与已存不一致时标「未保存」', () => {
    expect(attachmentDraftState('D:/a/x.md\nD:/b/y.md', ['D:/a/x.md'])).toEqual({
      attachments: ['D:/a/x.md', 'D:/b/y.md'],
      unsaved: true,
    });
    expect(attachmentDraftState('D:/a/x.md\n', ['D:/a/x.md']).unsaved).toBe(false);
  });
});
