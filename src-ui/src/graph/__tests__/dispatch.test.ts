import { describe, expect, it } from 'vitest';
import {
  appendDecision,
  acceptanceRound,
  appendCorrectionRound,
  appendValidationRetry,
  applyDispatchResults,
  applyRoleDefaults,
  clearTargetSession,
  failedNodesWithoutResults,
  continuationSession,
  continuationRunner,
  dispatchableNodeIds,
  graphStage,
  groupResultsByStage,
  isSmallEconomyTask,
  missingTargetNodeIds,
  nextStageStep,
  needsChainThinking,
  parseAcceptance,
  promptForRole,
  quotaBlockedNodeIds,
  reducePipelineStep,
  redraftNode,
  setReplyAdopted,
  withinRequestLimit,
  routeLine,
  routeReason,
  nodeStatusLabel,
} from '../dispatch.ts';
import type { Graph, GraphNode, Target } from '../types.ts';

const target = (agent = 'codex', model = 'm'): Target => ({
  agent,
  model,
  role: '执行',
  thinking: '中',
  visibility: '共享',
  write: '只读',
});
const node = (id: string, parents: string[], reply?: GraphNode['reply']): GraphNode => ({
  id,
  kind: parents.length === 0 ? 'material' : parents.length > 1 ? 'accept' : 'turn',
  parents,
  user: { text: id, attachments: [] },
  target: parents.length ? target() : undefined,
  reply,
  created_at: id,
});
const graph = (nodes: GraphNode[]): Graph => ({
  version: 1,
  id: 'g',
  title: '',
  root: 'r',
  nodes: Object.fromEntries(nodes.map((n) => [n.id, n])),
  created_at: '',
  updated_at: '',
});

describe('graph dispatch helpers', () => {
  it('TASK-14 G1 默认排除恢复时间未到的模型', () => {
    const g = graph([node('r', []), node('a', ['r'], { status: 'draft', text: '' }), node('b', ['r'], { status: 'draft', text: '' })]);
    g.nodes.b.target = target('kimi', 'k');
    const models = [
      { agent: 'codex', provider: '', id: 'm', vendor: '', billing_source: '', tier: '', thinking: '', status: '', source: '', smoked_at: '', smoke_model: '', smoke_failure: '', meta: '', note: '', quota_exhausted_until: '2026-09-15T11:00:00+08:00' },
      { agent: 'kimi', provider: '', id: 'k', vendor: '', billing_source: '', tier: '', thinking: '', status: '', source: '', smoked_at: '', smoke_model: '', smoke_failure: '', meta: '', note: '', quota_exhausted_until: '2026-09-15T09:00:00+08:00' },
    ];
    expect(quotaBlockedNodeIds(g, ['a', 'b'], models, Date.parse('2026-09-15T10:00:00+08:00'))).toEqual(['a']);
  });

  it('R5-1 验证重试到顶把未派验收标为已跳过', () => {
    const g = graph([
      node('r', []),
      { ...node('e', ['r'], { status: 'failed', text: '' }), target: { ...target(), role: '执行', write: '单写者' } },
      { ...node('a', ['e'], { status: 'draft', text: '' }), target: { ...target('kimi', 'k'), role: '验收' } },
    ]);
    const failure = { nodeId: 'e', status: 'validation_failed', text: '', modelReported: '', sessionId: '', tokensIn: 0, tokensOut: 0, durationMs: 0, error: 'failed' };
    const next = reducePipelineStep(g, [failure], { e: 3 }).graph;
    expect(next.nodes.a.reply).toEqual({ status: 'draft', text: '', skipped_reason: '上游验证未通过' });
    expect(nodeStatusLabel(next.nodes.a)).toBe('已跳过');
  });

  it('R5-2 验证重试路由显示修正轮且理由独立', () => {
    const g = graph([node('r', []), { ...node('e', ['r']), target: { ...target(), role: '执行' } }, { ...node('a', ['e'], { status: 'draft', text: '' }), target: { ...target('kimi', 'k'), role: '验收' } }]);
    const retried = appendValidationRetry(g, 'e', 'pytest failed', 2).graph;
    const retry = Object.values(retried.nodes).find(item => !g.nodes[item.id] && item.target?.role === '执行')!;
    expect(routeLine(retry, [], '部分')).toContain('执行（修正 2/3）→ codex/m');
    expect(routeReason(retry)).toContain('验证命令未通过');
  });

  it('R5-7 根列默认执行目标采用 roles.json 的执行角色', () => {
    const roles = { planning: target(), acceptance: target(), execution: { agent: 'codex', provider: 'openai', model: 'gpt-5', tier: '中', thinking: '高' } };
    expect(applyRoleDefaults(target('', ''), roles)).toMatchObject(roles.execution);
  });
  it('R3-2 异常只标记未收到结果的节点', () => {
    const active = [{ nodeId: 'a' }, { nodeId: 'b' }];
    const ok = [{ nodeId: 'a', status: 'ok', text: 'done', modelReported: '', sessionId: '', tokensIn: 0, tokensOut: 0, durationMs: 1, error: '' }];
    expect(failedNodesWithoutResults(active, ok, '连接中断').map((item) => item.nodeId)).toEqual(['b']);
    expect(failedNodesWithoutResults(active, ok, '连接中断')[0]).toMatchObject({ status: 'error', error: '连接中断' });
  });

  it('R3-3 推进层遇到空 agent 或 model 时停在对应节点', () => {
    const g = graph([
      node('r', []),
      { ...node('a', ['r'], { status: 'draft', text: '' }), target: target('', 'm') },
      { ...node('b', ['r'], { status: 'draft', text: '' }), target: target('codex', '') },
    ]);
    const ids = dispatchableNodeIds(g);
    expect(missingTargetNodeIds(g, ids)).toEqual(['a', 'b']);
  });

  it('R3-4 角色默认值只填空目标并携带提供者档位与 thinking', () => {
    const roles = {
      planning: { agent: 'pi', provider: 'deepseek', model: 'plan', tier: '低', thinking: '高' },
      execution: { agent: 'codex', provider: 'openai', model: 'exec', tier: '中', thinking: '中' },
      acceptance: { agent: 'claude', provider: 'anthropic', model: 'review', tier: '高', thinking: '低' },
    };
    const empty = { ...target('', ''), role: '执行', provider: '', tier: '低' };
    expect(applyRoleDefaults(empty, roles)).toMatchObject(roles.execution);
    const filled = { ...target('kimi', 'chosen'), role: '执行', provider: 'moonshot', tier: '高' };
    expect(applyRoleDefaults(filled, roles)).toEqual(filled);
  });
  it('只返回父节点完成或为材料的 draft 节点', () => {
    const g = graph([
      node('r', []),
      node('a', ['r'], { status: 'draft', text: '' }),
      node('b', ['a'], { status: 'draft', text: '' }),
      node('c', ['r'], { status: 'done', text: 'ok' }),
      node('d', ['c'], { status: 'draft', text: '' }),
    ]);
    expect(dispatchableNodeIds(g)).toEqual(['a', 'd']);
  });

  it('同目标共享父节点带 session_id 时续聊', () => {
    const parent = node('a', ['r'], { status: 'done', text: 'one', session_id: 'thread-1' });
    const child = node('b', ['a'], { status: 'draft', text: '' });
    const g = graph([node('r', []), parent, child]);
    expect(continuationSession(g, 'b')).toBe('thread-1');
    g.nodes.b.target = target('pi', 'other');
    expect(continuationSession(g, 'b')).toBeUndefined();
  });

  it('续聊请求保留父回复的 ACP runner 来源', () => {
    const parent = node('a', ['r'], { status: 'done', text: 'one', session_id: 'acp-1', runner: 'acp' });
    const child = node('b', ['a'], { status: 'draft', text: '' });
    const g = graph([node('r', []), parent, child]);
    expect(continuationRunner(g, 'b')).toBe('acp');
  });

  it('盲节点不携带父节点的 session 与 runner', () => {
    const parent = node('a', ['r'], { status: 'done', text: 'one', session_id: 'acp-1', runner: 'acp' });
    const child = node('b', ['a'], { status: 'draft', text: '' });
    child.target!.visibility = '盲';
    const g = graph([node('r', []), parent, child]);
    expect(continuationSession(g, 'b')).toBeUndefined();
    expect(continuationRunner(g, 'b')).toBeUndefined();
  });

  it('新开会话清除同目标父链 session_id', () => {
    const parent = node('a', ['r'], { status: 'done', text: 'one', session_id: 'thread-1' });
    const child = node('b', ['a'], { status: 'draft', text: '' });
    const g = graph([node('r', []), parent, child]);
    const next = clearTargetSession(g, 'b');
    expect(next.nodes.a.reply?.session_id).toBeUndefined();
    expect(continuationSession(next, 'b')).toBeUndefined();
  });

  it('结果写回 Reply 的正文、用量、耗时与 session_id，保留采纳状态', () => {
    const g = graph([node('r', []), node('a', ['r'], { status: 'pending', text: '', adopted: true })]);
    const next = applyDispatchResults(g, [{
      nodeId: 'a', status: 'ok', text: 'answer', modelReported: 'openai/m',
      sessionId: 'thread-2', tokensIn: 12, tokensOut: 4, durationMs: 80, error: '',
    }]);
    expect(next.nodes.a.reply).toEqual({
      status: 'done', text: 'answer', model_reported: 'openai/m', tokens_in: 12,
      tokens_out: 4, ms: 80, session_id: 'thread-2', adopted: true,
    });
  });

  it('长驻运行方式写回 Reply', () => {
    const g = graph([node('r', []), node('a', ['r'], { status: 'pending', text: '' })]);
    const next = applyDispatchResults(g, [{
      nodeId: 'a', status: 'ok', text: 'answer', modelReported: '', sessionId: 'acp-1',
      tokensIn: 0, tokensOut: 0, durationMs: 20, error: '', runner: 'acp',
    }]);
    expect(next.nodes.a.reply?.runner).toBe('acp');
  });

  it('本地合成失败结果写回未运行而非一次性', () => {
    const g = graph([node('r', []), node('a', ['r'], { status: 'pending', text: '' })]);
    const next = applyDispatchResults(g, [{
      nodeId: 'a', status: 'error', text: '', modelReported: '', sessionId: '',
      tokensIn: 0, tokensOut: 0, durationMs: 0, error: 'frontend failure', runner: 'unknown',
    }]);
    expect(next.nodes.a.reply?.runner).toBe('unknown');
  });

  it('升级后提高档位和 thinking 并回到 draft', () => {
    const g = graph([node('r', []), node('a', ['r'], { status: 'failed', text: '失败' })]);
    const next = redraftNode(g, 'a', true);
    expect(next.nodes.a.target?.tier).toBe('中');
    expect(next.nodes.a.target?.thinking).toBe('高');
    expect(next.nodes.a.reply).toEqual({ status: 'draft', text: '' });
  });

  it('采纳状态写入 Reply 并可取消', () => {
    const g = graph([node('r', []), node('a', ['r'], { status: 'done', text: 'answer' })]);
    const adopted = setReplyAdopted(g, 'a', true);
    expect(adopted.nodes.a.reply?.adopted).toBe(true);
    expect(setReplyAdopted(adopted, 'a', false).nodes.a.reply?.adopted).toBe(false);
  });

  it('交人裁决节点不把系统标签塞进用户正文', () => {
    const g = graph([node('r', []), node('a', ['r'], { status: 'done', text: 'answer' })]);
    const next = appendDecision(g, ['a'], '请裁决');
    const added = Object.values(next.nodes).find((item) => !g.nodes[item.id]);
    expect(added?.user.text).toBe('请裁决');
  });

  it('验收结论解析 PASS / NEEDS_FIX / 未知', () => {
    expect(parseAcceptance('说明\nPASS')).toBe('PASS');
    expect(parseAcceptance('PASS 是旧值\nNEEDS_FIX')).toBe('NEEDS_FIX');
    expect(parseAcceptance('all tests passed')).toBe('UNKNOWN');
    expect(parseAcceptance('NEEDS_FIX\n修正后 PASS')).toBe('PASS');
    expect(parseAcceptance(`PASS${'x'.repeat(1200)}`)).toBe('UNKNOWN');
    expect(parseAcceptance('需要人工判断')).toBe('UNKNOWN');
  });

  it('小任务与链式任务启发式', () => {
    expect(isSmallEconomyTask('改一个按钮', [])).toBe(true);
    expect(isSmallEconomyTask('改一个按钮', ['spec.md'])).toBe(false);
    expect(isSmallEconomyTask('这'.repeat(194), [])).toBe(false);
    expect(isSmallEconomyTask('先修改按钮并补测试', [])).toBe(false);
    expect(isSmallEconomyTask('修改按钮\n补充说明', [])).toBe(false);
    expect(needsChainThinking('计算 1、2、3 的统计值')).toBe(true);
    expect(needsChainThinking('只改标题')).toBe(false);
  });

  it('R4-5 推进函数覆盖派发、未指定、上限和结束四态', () => {
    const ready = graph([node('r', []), node('a', ['r'], { status: 'draft', text: '' })]);
    expect(nextStageStep(ready, [], { maxRequests: 3 })).toEqual({ kind: 'dispatch', nodeIds: ['a'] });
    const missing = { ...ready, nodes: { ...ready.nodes, a: { ...ready.nodes.a, target: target('', '') } } };
    expect(nextStageStep(missing, [], { maxRequests: 3 })).toEqual({ kind: 'missing-target', nodeIds: ['a'] });
    expect(nextStageStep(ready, [{ nodeId: 'used' }], { maxRequests: 1 })).toEqual({ kind: 'limit', nodeIds: ['a'] });
    const done = { ...ready, nodes: { ...ready.nodes, a: { ...ready.nodes.a, reply: { status: 'done' as const, text: 'ok' } } } };
    expect(nextStageStep(done, [], { maxRequests: 3 })).toEqual({ kind: 'done', nodeIds: [] });
  });

  it('角色模板固定加入数值、任务契约与验收维度', () => {
    expect(promptForRole('执行', '正文')).toContain('不得心算');
    expect(promptForRole('出方案', '正文')).toContain('任务契约');
    expect(promptForRole('验收', '正文')).toContain('是否完整实现');
  });

  it('NEEDS_FIX 在图上追加执行与验收节点，最多轮次可数', () => {
    const artifacts = { diff: 'x', changed_files: [], stdout_summary: 'ok' };
    const g = graph([
      node('r', []),
      { ...node('e', ['r'], { status: 'done', text: 'done', artifacts }), target: { ...target(), role: '执行', write: '单写者' } },
      { ...node('a', ['e'], { status: 'done', text: 'NEEDS_FIX' }), target: { ...target('kimi', 'k'), role: '验收', visibility: '盲' } },
    ]);
    const next = appendCorrectionRound(g, 'a');
    expect(Object.keys(next.graph.nodes)).toHaveLength(5);
    expect(next.graph.nodes[next.executionId!].parents).toEqual(['a']);
    expect(next.graph.nodes[next.executionId!].target?.upstreams).toEqual(['a', 'e']);
    expect(next.graph.nodes[next.acceptanceId!].parents).toEqual([next.executionId!]);
    expect(acceptanceRound(next.graph, next.acceptanceId!)).toBe(2);
    expect(graphStage(next.graph, next.acceptanceId!)).toBe(4);
  });

  it('验证失败跳过原验收并追加下一次执行与验收', () => {
    const g = graph([
      node('r', []),
      { ...node('e', ['r'], { status: 'failed', text: '验证失败' }), target: { ...target(), role: '执行', write: '单写者' } },
      { ...node('a', ['e'], { status: 'draft', text: '' }), target: { ...target('kimi', 'k'), role: '验收', visibility: '盲' } },
    ]);
    const next = appendValidationRetry(g, 'e', 'pytest failed').graph;
    const added = Object.values(next.nodes).filter((item) => !g.nodes[item.id]);
    const retry = added.find((item) => item.target?.role === '执行')!;
    const accept = added.find((item) => item.target?.role === '验收')!;
    expect(next.nodes.a.reply).toEqual({ status: 'failed', text: '验证命令未通过，未送审' });
    expect(retry.user.text).toContain('pytest failed');
    expect(accept.parents).toEqual([retry.id]);
    expect(dispatchableNodeIds(next)).toContain(retry.id);
  });

  it('推进 reducer 在验证重试上限和任一失败时停止', () => {
    const g = graph([
      node('r', []),
      { ...node('e', ['r'], { status: 'failed', text: '验证失败' }), target: { ...target(), role: '执行', write: '单写者' } },
      { ...node('a', ['e'], { status: 'draft', text: '' }), target: { ...target('kimi', 'k'), role: '验收', visibility: '盲' } },
    ]);
    const validation = { nodeId: 'e', status: 'validation_failed', text: '', modelReported: '', sessionId: '', tokensIn: 0, tokensOut: 0, durationMs: 0, error: 'pytest failed' };
    expect(reducePipelineStep(g, [validation], { e: 3 }).continueRunning).toBe(false);
    const retried = reducePipelineStep(g, [validation], { e: 2 });
    expect(retried.continueRunning).toBe(true);
    const retryId = Object.keys(retried.validationRetries).find((id) => id !== 'e')!;
    expect(retried.validationRetries[retryId]).toBe(3);
    expect(reducePipelineStep(g, [{ ...validation, status: 'error' }], {}).continueRunning).toBe(false);
  });

  it('推进 reducer 在 NEEDS_FIX 轮次内生长，PASS 时停止', () => {
    const artifacts = { diff: 'x', changed_files: [], stdout_summary: 'ok' };
    const g = graph([
      node('r', []),
      { ...node('e', ['r'], { status: 'done', text: 'done', artifacts }), target: { ...target(), role: '执行', write: '单写者' } },
      { ...node('a', ['e'], { status: 'done', text: 'NEEDS_FIX' }), target: { ...target('kimi', 'k'), role: '验收', visibility: '盲' } },
    ]);
    const base = { nodeId: 'a', status: 'ok', modelReported: '', sessionId: '', tokensIn: 0, tokensOut: 0, durationMs: 0, error: '' };
    expect(reducePipelineStep(g, [{ ...base, text: 'NEEDS_FIX' }], {}).continueRunning).toBe(true);
    expect(reducePipelineStep(g, [{ ...base, text: 'PASS' }], {}).continueRunning).toBe(false);
  });

  it('推进 reducer 超过验收修正上限时不再生长', () => {
    const artifacts = { diff: 'x', changed_files: [], stdout_summary: 'ok' };
    const nodes = [node('r', [])];
    let parent = 'r';
    for (let round = 1; round <= 4; round += 1) {
      const executionId = `e${round}`;
      const acceptanceId = `a${round}`;
      nodes.push({ ...node(executionId, [parent], { status: 'done', text: 'done', artifacts }), target: { ...target(), role: '执行', write: '单写者' } });
      nodes.push({ ...node(acceptanceId, [executionId], { status: 'done', text: 'NEEDS_FIX' }), target: { ...target('kimi', 'k'), role: '验收', visibility: '盲' } });
      parent = acceptanceId;
    }
    const g = graph(nodes);
    const result = { nodeId: 'a4', status: 'ok', text: 'NEEDS_FIX', modelReported: '', sessionId: '', tokensIn: 0, tokensOut: 0, durationMs: 0, error: '' };
    const reduced = reducePipelineStep(g, [result], {});
    expect(reduced.continueRunning).toBe(false);
    expect(Object.keys(reduced.graph.nodes)).toHaveLength(Object.keys(g.nodes).length);
  });

  it('多个验证失败节点分别追加重试', () => {
    const g = graph([
      node('r', []),
      { ...node('e1', ['r'], { status: 'failed', text: '' }), target: { ...target(), role: '执行', write: '单写者' } },
      { ...node('a1', ['e1'], { status: 'draft', text: '' }), target: { ...target('kimi', 'k'), role: '验收', visibility: '盲' } },
      { ...node('e2', ['r'], { status: 'failed', text: '' }), target: { ...target('pi', 'p'), role: '执行', write: '单写者' } },
      { ...node('a2', ['e2'], { status: 'draft', text: '' }), target: { ...target('claude', 'c'), role: '验收', visibility: '盲' } },
    ]);
    const failed = (nodeId: string) => ({ nodeId, status: 'validation_failed', text: '模型正文', modelReported: '', sessionId: '', tokensIn: 0, tokensOut: 0, durationMs: 0, error: 'failed', validationOutput: 'pytest failed' });
    const reduced = reducePipelineStep(g, [failed('e1'), failed('e2')], {});
    expect(reduced.continueRunning).toBe(true);
    expect(Object.values(reduced.graph.nodes).filter((item) => !g.nodes[item.id] && item.target?.role === '执行')).toHaveLength(2);
    expect(Object.keys(reduced.validationRetries)).toHaveLength(2);
    expect(reduced.graph.nodes.a1.reply?.status).toBe('failed');
    expect(reduced.graph.nodes.a2.reply?.status).toBe('failed');
  });

  it('验证失败保留模型正文并标注验证输出', () => {
    const g = graph([node('r', []), node('e', ['r'], { status: 'pending', text: '' })]);
    const next = applyDispatchResults(g, [{
      nodeId: 'e', status: 'validation_failed', text: '已修改文件', modelReported: '', sessionId: '',
      tokensIn: 0, tokensOut: 0, durationMs: 1, error: 'pytest failed', validationOutput: '1 failed',
    }]);
    expect(next.nodes.e.reply?.text).toBe('已修改文件\n\n[验证输出]\n1 failed');
    expect(next.nodes.e.reply?.error_kind).toBe('validation_failed');
  });

  it('真身不符作为独立失败类型写回 Reply', () => {
    const g = graph([node('r', []), node('e', ['r'], { status: 'pending', text: '' })]);
    const next = applyDispatchResults(g, [{
      nodeId: 'e', status: 'identity_mismatch', text: '', modelReported: 'other/model', sessionId: '',
      tokensIn: 0, tokensOut: 0, durationMs: 1, error: '真身不符',
    }]);
    expect(next.nodes.e.reply).toMatchObject({
      status: 'failed', text: '真身不符', error_kind: 'identity_mismatch', model_reported: 'other/model',
    });
  });

  it('结果按 stage 分组并以首个节点角色命名', () => {
    const g = graph([
      node('r', []),
      { ...node('p', ['r']), target: { ...target(), role: '出方案' } },
      { ...node('e', ['p']), target: { ...target(), role: '' } },
    ]);
    const base = { status: 'ok', text: '', modelReported: '', sessionId: '', tokensIn: 0, tokensOut: 0, durationMs: 0, error: '' };
    const groups = groupResultsByStage(g, [
      { ...base, nodeId: 'e', stage: 2 },
      { ...base, nodeId: 'p', stage: 1 },
    ]);
    expect(groups.map((group) => [group.stage, group.title])).toEqual([[1, '出方案'], [2, '第 2 阶段']]);
    expect(withinRequestLimit(6, 2, 8)).toBe(true);
    expect(withinRequestLimit(7, 2, 8)).toBe(false);
  });

  it('R3-5 失败节点重派后可再次派发且有产物父节点不受影响', () => {
    const parent = { ...node('p', ['r'], { status: 'done', text: 'parent', artifacts: { diff: 'd', changed_files: [], stdout_summary: 's' } }), target: target() };
    const g = graph([node('r', []), parent, { ...node('e', ['p'], { status: 'failed', text: 'x' }), target: target() }]);
    const next = redraftNode(g, 'e');
    expect(dispatchableNodeIds(next)).toEqual(['e']);
    expect(next.nodes.p).toEqual(parent);
  });

  it('G1 路由行显示验证命令和必要的通道提示', () => {
    const n = { ...node('e', ['r']), target: { ...target('cc-switch', 'm'), write: '单写者' as const, validation_command: 'npm test' } };
    const line = routeLine(n, [], '部分');
    expect(line).toContain('验证=npm test');
    expect(line).toContain('cc-switch 单轮无工具，产物将为空');
    const kimi = { ...node('k', ['r']), target: { ...target('kimi', 'kimi-code/k3'), visibility: '盲' as const } };
    expect(routeLine(kimi, [], '部分')).toContain('隔离=部分');
  });
});
