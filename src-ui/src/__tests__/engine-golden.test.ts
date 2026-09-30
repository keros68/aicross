// @ts-expect-error Vitest provides Node built-ins; application types intentionally do not.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
// @ts-expect-error Vitest provides Node built-ins; application types intentionally do not.
import { dirname, resolve } from 'node:path';
// @ts-expect-error Vitest provides Node built-ins; application types intentionally do not.
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  applyDispatchResults, applyRoleDefaults, buildNodeRequest, dispatchBatchKey,
  markBatchProcessed, nextStageStep, presetWithRoleDefaults, reducePipelineStep,
  resetNodesToDraft,
} from '../graph/dispatch';
import { serializeContext } from '../graph/context';
import type { BackendNodeResult } from '../graph/dispatch';
import type { Graph, Target } from '../graph/types';

declare const process: { env: Record<string, string | undefined> };
const fixtureDir = resolve(dirname(fileURLToPath(import.meta.url)), '../../../fixtures/engine-golden');

const target = (role = '执行', visibility: Target['visibility'] = role === '执行' ? '共享' : '盲'): Target => ({
  agent: 'codex', model: 'gpt-test', role, thinking: '中', visibility, write: '只读',
});
const graph = (role = '执行', visibility?: Target['visibility']): Graph => ({
  version: 1, id: 'graph-1', title: '契约', root: 'root', project: 'D:/project', source: 'console',
  created_at: '2026-09-30T00:00:00.000Z', updated_at: '2026-09-30T00:00:00.000Z',
  nodes: {
    root: { id: 'root', kind: 'material', parents: [], user: { text: '实现任务', attachments: [] }, created_at: '2026-09-30T00:00:00.000Z', source: 'console' },
    node: { id: 'node', kind: 'turn', parents: ['root'], user: { text: '', attachments: [] }, reply: { status: 'draft', text: '' }, target: target(role, visibility), created_at: '2026-09-30T00:00:01.000Z', source: 'console' },
  },
});
const result = (overrides: Partial<BackendNodeResult> = {}): BackendNodeResult => ({
  nodeId: 'node', stage: 1, status: 'ok', text: '完成', modelReported: 'gpt-test', sessionId: '',
  tokensIn: 1, tokensOut: 2, cacheRead: 0, durationMs: 3, error: '', runner: 'oneshot', artifacts: null,
  ...overrides,
});
function requestOf(value: Graph, nodeId: string) {
  const context = serializeContext(value, nodeId);
  return buildNodeRequest(nodeId, value.nodes[nodeId].target as Target, {
    models: [], prompt: context.prompt, materials: context.materials, currentMessage: '', originalRequest: '实现任务',
    upstreamArtifacts: [], sessionId: '', previousRunner: '', rollback: false, stage: 1,
  });
}
function summaryOf(value: Graph) {
  return Object.values(value.nodes).filter((node) => node.target).map((node) => ({
    node_id: node.id, role: node.target?.role ?? '', agent: node.target?.agent ?? '', model: node.target?.model ?? '',
    status: node.reply?.status ?? 'draft', text_excerpt: (node.reply?.text ?? '').slice(0, 160),
    artifacts_dir: node.reply?.artifacts?.worktree ?? '',
  }));
}
function fixture(name: string, inputGraph: Graph, batches: BackendNodeResult[][], finalGraph: Graph, observation?: unknown) {
  return {
    name, inputGraph, settings: { maxRequests: 8, fixRoundLimit: 1 }, injectedResults: batches,
    requests: Object.values(inputGraph.nodes).filter((node) => node.target).map((node) => requestOf(inputGraph, node.id)),
    finalGraph, runSummary: summaryOf(finalGraph), observation,
  };
}
function normalize<T>(value: T): T {
  const ids = new Map<string, string>();
  const times = new Map<string, string>();
  const visit = (item: unknown): unknown => {
    if (typeof item === 'string') {
      if (/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(item)) {
        if (!ids.has(item)) ids.set(item, `UUID_${ids.size + 1}`);
        return ids.get(item);
      }
      if (/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{3}Z$/.test(item)) {
        if (!times.has(item)) times.set(item, `TIME_${times.size + 1}`);
        return times.get(item);
      }
      return item;
    }
    if (Array.isArray(item)) return item.map(visit);
    if (item && typeof item === 'object') return Object.fromEntries(Object.entries(item).map(([key, child]) => [visit(key), visit(child)]));
    return item;
  };
  return visit(JSON.parse(JSON.stringify(value))) as T;
}
function scenarios(): Array<[string, ReturnType<typeof fixture>]> {
  const base = graph();
  const ok = result();
  const done = applyDispatchResults(base, [ok]);
  const acceptance = graph('验收', '盲');
  const needsFix = result({ text: 'NEEDS_FIX\n- 补回归测试' });
  const accepted = applyDispatchResults(acceptance, [needsFix]);
  const correction = reducePipelineStep(accepted, [needsFix], {}, 1);
  const exhausted = reducePipelineStep(accepted, [needsFix], {}, 0);
  const failed = result({ status: 'validation_failed', text: '', error: '验证失败', validationOutput: 'exit 1' });
  const retried = reducePipelineStep(applyDispatchResults(base, [failed]), [failed], { node: 1 }, 1);
  const quota = result({ status: 'quota_exhausted', text: '', error: '额度用尽', quotaResetsAt: '2026-10-01T00:00:00Z' });
  const key = dispatchBatchKey('run-1', 1, [ok]);
  const once = markBatchProcessed(new Set<string>(), key);
  const twice = markBatchProcessed(once.seen, key);
  const missing = graph();
  missing.nodes.node = { ...missing.nodes.node, target: { ...target(), model: '' } };
  const existing = graph();
  existing.nodes.old = { id: 'old', kind: 'material', parents: [], user: { text: '既有材料', attachments: [] }, created_at: '2026-09-29T00:00:00.000Z' };
  const roles = {
    planning: { agent: 'claude', model: 'plan', thinking: '中' },
    execution: { agent: 'codex', model: 'exec', thinking: '高' },
    acceptance: { agent: 'kimi', model: 'review', thinking: '低' },
  };
  const direct = presetWithRoleDefaults({ id: 'direct', name: '', builtin: false, stages: [{ nodes: [{ ...target(), agent: '', model: '' }] }] }, roles);
  const blind = graph('验收', '盲');
  blind.nodes.verify1 = { ...blind.nodes.node, id: 'verify1', target: target('验证', '盲'), created_at: '2026-09-30T00:00:02.000Z' };
  blind.nodes.verify2 = { ...blind.nodes.node, id: 'verify2', target: target('验证', '盲'), created_at: '2026-09-30T00:00:03.000Z' };
  const rows: Array<[string, ReturnType<typeof fixture>]> = [
    ['economy-pass', fixture('经济模式一次通过', base, [[ok]], done)],
    ['needs-fix-pass', fixture('验收 NEEDS_FIX → 修正轮 → 通过', acceptance, [[needsFix], [ok]], correction.graph, correction.continueRunning)],
    ['fix-limit-leftover', fixture('修正轮到上限 → 遗留节点', acceptance, [[needsFix]], exhausted.graph, exhausted.leftoverIds)],
    ['validation-retry-limit', fixture('验证命令失败 → 重试到顶', base, [[failed]], retried.graph, retried.validationRetries)],
    ['blind-three-way', fixture('盲审模式三路', blind, [], blind)],
    ['shared', fixture('共享模式', base, [], base)],
    ['quota-exhausted', fixture('某路 quota_exhausted', base, [[quota]], applyDispatchResults(base, [quota]))],
    ['duplicate-results', fixture('同一批结果重复送达', base, [[ok], [ok]], done, { first: once.fresh, duplicate: twice.fresh })],
    ['missing-next-target', fixture('下一阶段缺目标', missing, [], missing, nextStageStep(missing, [], { maxRequests: 8 }))],
    ['request-limit', fixture('超过总请求上限', base, [], base, nextStageStep(base, Array.from({ length: 8 }, (_, i) => ({ nodeId: `old-${i}` })), { maxRequests: 8 }))],
    ['targets-direct', fixture('targets 直给', base, [], base, direct)],
    ['continue-existing-graph', fixture('续写已有图', existing, [[ok]], applyDispatchResults(existing, [ok]), serializeContext(existing, 'node'))],
    ['role-defaults', fixture('角色默认值', base, [], base, applyRoleDefaults({ ...target(), agent: '', model: '' }, roles))],
    ['reset-draft', fixture('恢复待派发节点', base, [], resetNodesToDraft({ ...base, nodes: { ...base.nodes, node: { ...base.nodes.node, reply: { status: 'pending', text: '' } } } }, ['node']))],
  ];
  const reasons = [
    '同时在飞 run 已达上限 3', '读取资源失败：示例', '找不到预设：missing', '角色「执行」没有可派发目标',
    '未接入', '验收/验证节点只能只读', '目标验证失败：codex/gpt-test',
    '目标额度用尽：codex/gpt-test；恢复时间 2026-10-01T00:00:00Z', '本次派发 9 路，超过总请求上限 8',
    '项目目录有 1 处未提交改动；worktree 从 HEAD 新建，看不到这些改动',
  ];
  reasons.forEach((reason, i) => rows.push([`precheck-${String(i + 1).padStart(2, '0')}`, fixture(`派发前拒绝 ${i + 1}`, base, [], base, { reason, precedingChecks: reasons.slice(0, i) })]));
  return rows;
}

describe('TASK-36 TypeScript reference golden', () => {
  for (const [slug, raw] of scenarios()) {
    it(slug, () => {
      const actual = normalize(raw);
      const path = resolve(fixtureDir, `${slug}.json`);
      if (process.env.AICROSS_WRITE_GOLDEN === '1') {
        mkdirSync(fixtureDir, { recursive: true });
        writeFileSync(path, `${JSON.stringify(actual, null, 2)}\n`, 'utf8');
      }
      expect(actual).toEqual(JSON.parse(readFileSync(path, 'utf8')));
    });
  }
});
