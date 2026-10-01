import { describe, expect, it } from 'vitest';
import {
  acceptanceVerdict,
  acceptanceVerdictLabel,
  canAdopt,
  cardSubtitle,
  cardTitle,
  fallbackText,
  formatDuration,
  formatTokens,
  groupNodesByStage,
  identityMismatch,
  nodeStatusLabel,
  nodeStatusTone,
  pollIntervalMs,
  quotaResetText,
  reportedModelLabel,
  runElapsedMs,
  stageColumnCount,
} from '../viewer/run-view';
import type { RunView, RunViewNode } from '../tauri';

function node(overrides: Partial<RunViewNode> = {}): RunViewNode {
  return {
    nodeId: 'n1',
    stage: 1,
    role: '执行',
    agent: 'claude',
    provider: '',
    model: 'sonnet',
    status: 'done',
    errorKind: '',
    adopted: false,
    hasResult: true,
    text: '',
    skippedReason: '',
    modelReported: '',
    tokensIn: 1000,
    tokensOut: 200,
    tokensCache: 50,
    ms: 1500,
    fallbackFrom: '',
    fallbackTo: '',
    quotaResetsAt: '',
    swarmTitle: '',
    artifacts: null,
    ...overrides,
  };
}

function view(overrides: Partial<RunView> = {}): RunView {
  return {
    runId: 'run-1',
    graphId: 'g1',
    project: 'D:/p',
    presetName: '经济模式',
    graphTitle: '给解析器加测试',
    status: 'done',
    reason: '',
    startedAt: '2026-10-01T08:00:00.000Z',
    updatedAt: '2026-10-01T08:01:00.000Z',
    leftover: [],
    nodes: [],
    ...overrides,
  };
}

describe('run viewer data assembly (TASK-37)', () => {
  it('groups nodes by stage in summary order with role titles', () => {
    const nodes = [
      node({ nodeId: 'plan', stage: 1, role: '出方案' }),
      node({ nodeId: 'exec', stage: 2, role: '执行' }),
      node({ nodeId: 'accept', stage: 3, role: '验收' }),
      node({ nodeId: 'verify', stage: 3, role: '验证' }),
      // 修正轮补的执行与验收：深度与上一轮相同，归入同一阶段。
      node({ nodeId: 'exec-fix', stage: 2, role: '执行' }),
    ];
    const groups = groupNodesByStage({ nodes });
    expect(groups.map((group) => group.stage)).toEqual([1, 2, 3]);
    expect(groups.map((group) => group.title)).toEqual(['出方案', '执行', '验收']);
    // 组内保持 run 摘要顺序。
    expect(groups[1].nodes.map((n) => n.nodeId)).toEqual(['exec', 'exec-fix']);
    expect(groups[2].nodes.map((n) => n.nodeId)).toEqual(['accept', 'verify']);
    // 没有角色的阶段退回「第 N 阶段」。
    expect(groupNodesByStage({ nodes: [node({ role: '', stage: 4 })] })[0].title).toBe('第 4 阶段');
  });

  it('caps stage columns at 3', () => {
    expect(stageColumnCount(1)).toBe(1);
    expect(stageColumnCount(3)).toBe(3);
    expect(stageColumnCount(5)).toBe(3);
    expect(stageColumnCount(0)).toBe(1);
  });

  it('titles take swarm item title, then graph title, then agent/model', () => {
    const swarm = node({ role: '执行', swarmTitle: '改解析器' });
    expect(cardTitle(swarm, view())).toBe('执行 · 改解析器');
    const plain = node({ role: '执行', swarmTitle: '' });
    expect(cardTitle(plain, view())).toBe('执行 · 给解析器加测试');
    const bare = node({ role: '执行', swarmTitle: '' });
    expect(cardTitle(bare, view({ graphTitle: '' }))).toBe('执行 · claude/sonnet');
    // 图标题与 agent/model 都没有时给占位，不落成「角色 · /」。
    expect(cardTitle(bare, view({ graphTitle: '' })) === '执行 · claude/sonnet').toBe(true);
    expect(cardSubtitle(node({ agent: 'kimi', provider: 'kimi', model: 'k3' }))).toBe('Kimi Code/kimi/k3');
    expect(cardSubtitle(node({ agent: '', model: '' }))).toBe('未指定模型');
  });

  it('labels node statuses including skipped and error kinds', () => {
    expect(nodeStatusLabel(node({ status: 'done' }))).toBe('完成');
    expect(nodeStatusLabel(node({ status: 'pending' }))).toBe('运行中');
    expect(nodeStatusLabel(node({ status: 'draft' }))).toBe('待派');
    expect(nodeStatusLabel(node({ status: 'quota_exhausted' }))).toBe('额度用尽');
    expect(nodeStatusLabel(node({ status: 'failed', errorKind: 'timeout' }))).toBe('超时');
    expect(nodeStatusLabel(node({ status: 'failed', errorKind: 'identity_mismatch' }))).toBe('真身不符');
    expect(nodeStatusLabel(node({ status: 'failed', errorKind: 'validation_failed' }))).toBe('验证命令未通过');
    expect(nodeStatusLabel(node({ status: 'failed', errorKind: 'cancelled' }))).toBe('已取消');
    expect(nodeStatusLabel(node({ status: 'failed', errorKind: 'error' }))).toBe('失败');
    expect(nodeStatusLabel(node({ status: 'done', skippedReason: '上游验证未通过' }))).toBe('已跳过');
    // 胶囊只给异常态；完成与待派用纯文字。
    expect(nodeStatusTone(node({ status: 'done' }))).toBe('plain');
    expect(nodeStatusTone(node({ status: 'draft' }))).toBe('plain');
    expect(nodeStatusTone(node({ status: 'running' as string }))).toBe('alert');
    expect(nodeStatusTone(node({ status: 'failed', errorKind: 'timeout' }))).toBe('alert');
  });

  it('reports identity without backfilling the requested model', () => {
    expect(reportedModelLabel(node({ modelReported: 'claude-sonnet-4-6' }))).toBe('claude-sonnet-4-6');
    // 运行中给「—」，不回填请求值。
    expect(reportedModelLabel(node({ status: 'pending' }))).toBe('—');
    // 终态拿不到真身：kimi 说明不报，其余标未报告。
    expect(reportedModelLabel(node({ agent: 'kimi' }))).toBe('kimi 不报真身');
    expect(reportedModelLabel(node({ agent: 'claude' }))).toBe('未报告');
    // 切过备选时按备选的 agent 判断。
    expect(reportedModelLabel(node({ agent: 'claude', fallbackFrom: 'claude/sonnet', fallbackTo: 'kimi/k3' }))).toBe('kimi 不报真身');
  });

  it('flags identity mismatch against the effective requested model', () => {
    expect(identityMismatch(node({ model: 'sonnet', modelReported: 'claude-sonnet-4-6' }))).toBe(true);
    expect(identityMismatch(node({ model: 'claude-sonnet-4-6', modelReported: 'claude-sonnet-4-6' }))).toBe(false);
    // 没有真身不比。
    expect(identityMismatch(node({ modelReported: '' }))).toBe(false);
    // 切过备选：与备选的模型比。
    expect(identityMismatch(node({
      model: 'sonnet',
      modelReported: 'glm-5.3',
      fallbackFrom: 'claude/sonnet',
      fallbackTo: 'pi/glm-5.3',
    }))).toBe(false);
  });

  it('shows quota reset time only for quota states', () => {
    expect(quotaResetText(node({
      status: 'quota_exhausted',
      quotaResetsAt: '2026-10-02T18:00:00Z',
    }))).toBe('恢复时间 2026-10-02T18:00:00Z');
    expect(quotaResetText(node({ status: 'quota_exhausted', quotaResetsAt: '' }))).toBe('');
    expect(quotaResetText(node({ status: 'done', quotaResetsAt: '2026-10-02T18:00:00Z' }))).toBe('');
  });

  it('formats fallback switch text', () => {
    expect(fallbackText(node({ fallbackFrom: 'claude/sonnet', fallbackTo: 'pi/glm' })))
      .toBe('已切备选：claude/sonnet → pi/glm');
    expect(fallbackText(node({}))).toBe('');
  });

  it('derives acceptance verdict for review roles only', () => {
    const accept = (text: string) => node({ role: '验收', text });
    expect(acceptanceVerdict(accept('结论：PASS'))).toBe('PASS');
    expect(acceptanceVerdict(accept('仍需修正：NEEDS_FIX'))).toBe('NEEDS_FIX');
    expect(acceptanceVerdict(accept('没给结论'))).toBe('UNKNOWN');
    // 未完成的验收按未判定。
    expect(acceptanceVerdict(node({ role: '验收', status: 'failed' }))).toBe('UNKNOWN');
    expect(acceptanceVerdict(node({ role: '验证', status: 'pending' }))).toBe('UNKNOWN');
    // 执行角色不显示结论。
    expect(acceptanceVerdict(node({ role: '执行', text: 'PASS' }))).toBeNull();
    expect(acceptanceVerdictLabel('PASS')).toBe('通过');
    expect(acceptanceVerdictLabel('NEEDS_FIX')).toBe('需修正');
    expect(acceptanceVerdictLabel('UNKNOWN')).toBe('未判定');
    expect(acceptanceVerdictLabel(null)).toBe('—');
  });

  it('offers adopt only on nodes with settled results', () => {
    expect(canAdopt(node({ hasResult: true, status: 'done' }))).toBe(true);
    expect(canAdopt(node({ hasResult: true, status: 'failed' }))).toBe(true);
    expect(canAdopt(node({ hasResult: true, status: 'quota_exhausted' }))).toBe(true);
    expect(canAdopt(node({ hasResult: true, status: 'pending' }))).toBe(false);
    expect(canAdopt(node({ hasResult: true, status: 'draft' }))).toBe(false);
    expect(canAdopt(node({ hasResult: false, status: 'done' }))).toBe(false);
  });

  it('stops polling once the run reaches a terminal state', () => {
    expect(pollIntervalMs('running')).toBe(2000);
    expect(pollIntervalMs('done')).toBeNull();
    expect(pollIntervalMs('failed')).toBeNull();
    expect(pollIntervalMs('rejected')).toBeNull();
  });

  it('computes elapsed time from started_at to updatedAt for terminal runs', () => {
    const base = view({ startedAt: '2026-10-01T08:00:00.000Z', updatedAt: '2026-10-01T08:01:00.000Z' });
    expect(runElapsedMs(base, Date.parse('2026-10-01T09:00:00.000Z'))).toBe(60_000);
    // 运行中按当前时刻。
    expect(runElapsedMs(view({ ...base, status: 'running' }), Date.parse('2026-10-01T08:00:30.000Z'))).toBe(30_000);
    // 时间解析不出来给 null。
    expect(runElapsedMs(view({ startedAt: '不是时间' }), 0)).toBeNull();
  });

  it('formats durations and token numbers', () => {
    expect(formatDuration(1500)).toBe('1.5 秒');
    expect(formatDuration(null)).toBe('—');
    expect(formatTokens(null)).toBe('—');
    expect(formatTokens(0)).toBe('—');
    expect(formatTokens(9500)).toBe('9500');
    expect(formatTokens(95000)).toBe('95k');
    expect(formatTokens(1_234_567)).toBe('1.2m');
  });
});
