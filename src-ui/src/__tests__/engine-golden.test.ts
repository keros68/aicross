// TASK-36 R1（fix-36-r1.md R1-1）：金标准改为直接驱动 runConsoleDispatch。
// 每个场景用假 io（资源 / 预设 / 角色 / 在飞 run / git 返回场景数据；
// dispatchGraphStage 按调用次序返回注入结果；saveGraph 与 consoleRunUpdate
// 全程记录）和假 context（内存 graphs + SET_GRAPH）跑一遍完整派发。夹具形状：
//   name        场景一句话说明
//   input       event（派发事件）、settings（localStorage 原文，未夹取）、
//               models / resourcesError（listDispatchableModels 返回）、presets、
//               roles、runs（listConsoleRuns 两次调用都返回它）、
//               existingGraphs（store 里已有的图）、git（gitChanges 返回）
//   stages      每一次 dispatchGraphStage 实际收到的请求（逐字段）与注入结果：
//               { results: [...] } 本次返回的结果（nodeId 与请求里的节点一一对应）；
//               { replay: N }       原样返回 stages[N] 的结果（模拟重复送达）；
//               { throw: "..." }    本次派发抛错
//   runUpdates  每一次 consoleRunUpdate 收到的摘要（逐字段，含拒绝）
//   finalGraph  saveGraph 最后一次收到的图；派发前拒绝时从未到过 saveGraph，为 null
//   notes       场景级说明（含 fixRound 偏离标注）
//
// 唯一有意偏离（fix-36-r1.md R1-2）：请求里的 fixRound 字段按
// isFixRoundExecution(graph, nodeId) 计算后写入夹具。窗口控制台路径的
// buildRequest 从不带该字段（TS 侧缺陷，本期不改 TS 行为），夹具钉的是
// Rust 按 R1-2 修复后的口径。命中的场景在 notes 里统一标注。
//
// 确定性：crypto.randomUUID 换成计数器（UUID v4 形状），Date 冻结在
// 2026-09-30T00:00:00.000Z；写盘 / 比较前统一把 UUID 与 ISO 时间戳按出现
// 顺序归一化成 UUID_n / TIME_n。AICROSS_WRITE_GOLDEN=1 写夹具，否则读夹具
// 逐项断言。每个场景另带 expect / assert 钉点，防止夹具写成自洽的废品。
// @ts-expect-error Vitest provides Node built-ins; application types intentionally do not.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
// @ts-expect-error Vitest provides Node built-ins; application types intentionally do not.
import { dirname, resolve } from 'node:path';
// @ts-expect-error Vitest provides Node built-ins; application types intentionally do not.
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { runConsoleDispatch, type ConsoleChannelContext } from '../console-channel';
import { isFixRoundExecution } from '../graph/dispatch';
import { DEFAULT_DISPATCH_SETTINGS, type UiDispatchSettings } from '../graph/dispatch-settings';
import type { DispatchableModel, Graph, Preset, ReplyArtifacts, Target } from '../graph/types';
import type {
  ConsoleDispatchEvent, ConsoleRun, ConsoleRunUpdate, DispatchNodeRequest,
  DispatchNodeResult, DispatchStageRequest, GitChanges, RoleSettings,
} from '../tauri';

declare const process: { env: Record<string, string | undefined> };
const fixtureDir = resolve(dirname(fileURLToPath(import.meta.url)), '../../../fixtures/engine-golden');

type ChannelIo = Parameters<typeof runConsoleDispatch>[2];

// ── 场景素材 ─────────────────────────────────────────────────────────────

const target = (overrides: Partial<Target> = {}): Target => ({
  agent: 'codex', model: 'exec', role: '执行', thinking: '中', visibility: '共享', write: '只读',
  ...overrides,
});

const model = (agent: string, id: string, overrides: Partial<DispatchableModel> = {}): DispatchableModel => ({
  id, agent, provider: '', vendor: `${agent}-vendor`, billing_source: '订阅', tier: '', thinking: '',
  status: '已接入', verification: 'native', source: '', smoked_at: '', smoke_model: '',
  smoke_failure: '', meta: '', note: '', ...overrides,
});

const MODELS: DispatchableModel[] = [
  model('claude', 'plan', { tier: '高', thinking: '高' }),
  model('codex', 'exec', { tier: '低', thinking: '低' }),
  model('kimi', 'review', { tier: '中', thinking: '中' }),
];

const ROLES: RoleSettings = {
  version: 1,
  planning: { agent: 'claude', provider: '', model: 'plan', tier: '高', thinking: '高' },
  execution: { agent: 'codex', provider: '', model: 'exec', tier: '低', thinking: '低' },
  acceptance: { agent: 'kimi', provider: '', model: 'review', tier: '中', thinking: '中' },
};

const EMPTY_ROLES: RoleSettings = {
  version: 1,
  planning: { agent: '', provider: '', model: '', tier: '', thinking: '' },
  execution: { agent: '', provider: '', model: '', tier: '', thinking: '' },
  acceptance: { agent: '', provider: '', model: '', tier: '', thinking: '' },
};

// 与 src/orchestration.rs 的 builtin_presets() 同形状（手写镜像，改动一起改）。
const PRESETS: Preset[] = [
  {
    id: 'builtin-shared', name: '共享模式', builtin: true,
    stages: [{ nodes: [{ agent: '', model: '', role: '执行', thinking: '中', visibility: '共享', write: '只读' }] }],
  },
  {
    id: 'builtin-economy', name: '经济模式', builtin: true,
    stages: [
      { nodes: [{ agent: '', model: '', role: '出方案', tier: '高', thinking: '高', visibility: '共享', write: '只读' }] },
      { nodes: [{ agent: '', model: '', role: '执行', tier: '低', thinking: '低', visibility: '共享', write: '单写者' }] },
      {
        nodes: [
          { agent: '', model: '', role: '验收', thinking: '中', visibility: '盲', write: '只读' },
          { agent: '', model: '', role: '验证', thinking: '中', visibility: '盲', write: '只读' },
        ],
      },
    ],
  },
  {
    id: 'builtin-blind', name: '盲审模式', builtin: true,
    stages: [{
      nodes: [
        { agent: '', model: '', role: '验收', thinking: '中', visibility: '盲', write: '只读' },
        { agent: '', model: '', role: '验证', thinking: '中', visibility: '盲', write: '只读' },
        { agent: '', model: '', role: '验证', thinking: '中', visibility: '盲', write: '只读' },
      ],
    }],
  },
];

const SELF_RUN: ConsoleRun = {
  run_id: 'run-1', graph_id: '', project: 'D:/project', status: 'running',
  nodes: [], leftover: [], started_at: '2026-09-30T00:00:00.000Z', in_flight_node_ids: [],
};

const runningRun = (id: string): ConsoleRun => ({
  run_id: id, graph_id: 'g', project: 'D:/project', status: 'running',
  nodes: [], leftover: [], started_at: '2026-09-29T00:00:00.000Z', in_flight_node_ids: [],
});

const ARTIFACTS: ReplyArtifacts = {
  diff: 'diff --git a/src/a.ts b/src/a.ts\n@@ -0,0 +1 @@\n+export const a = 1;',
  changed_files: [{ path: 'src/a.ts', content: 'export const a = 1;' }],
  worktree: 'D:/project/.aicross/worktrees/exec-1',
  stdout_summary: '构建通过',
};

const ARTIFACTS_ROUND2: ReplyArtifacts = {
  diff: 'diff --git a/src/a.test.ts b/src/a.test.ts\n@@ -0,0 +1 @@\n+test("a", () => {});',
  changed_files: [{ path: 'src/a.test.ts', content: 'test("a", () => {});' }],
  worktree: 'D:/project/.aicross/worktrees/exec-2',
  stdout_summary: '测试通过',
};

const DIRTY_GIT: GitChanges = {
  state: 'ok', repo_root: 'D:/project', branch: 'main',
  uncommitted: [{ path: 'D:/project/src/a.ts', rel: 'src/a.ts', status: 'M', added: 3, deleted: 1 }],
  untracked: [], session_commits: [],
};

const res = (node: DispatchNodeRequest, overrides: Partial<DispatchNodeResult> = {}): DispatchNodeResult => ({
  nodeId: node.nodeId, stage: node.stage, status: 'ok', text: '完成', rawStdout: '', rawStderr: '',
  modelReported: node.model, sessionId: '', tokensIn: 11, tokensOut: 22, cacheRead: 0, durationMs: 33,
  exitCode: 0, timedOut: false, truncated: false, redacted: 0, isolation: '完全', runner: 'oneshot',
  workdir: '', dispatchFile: '', frontmatter: '', outputPreview: '', error: '', artifacts: null,
  validationOutput: '', fallbackFrom: '', fallbackTo: '', ...overrides,
});

const validationFailed = (node: DispatchNodeRequest): DispatchNodeResult =>
  res(node, { status: 'validation_failed', text: '', error: '验证失败', validationOutput: 'exit 1', exitCode: 1 });

// ── 场景定义 ─────────────────────────────────────────────────────────────

interface ScenarioInput {
  event: ConsoleDispatchEvent;
  settings: UiDispatchSettings;
  models: DispatchableModel[];
  resourcesError: string | null;
  presets: Preset[];
  roles: RoleSettings;
  runs: ConsoleRun[];
  existingGraphs: Graph[];
  git: GitChanges;
}

type Batch =
  | { results: (request: DispatchStageRequest) => DispatchNodeResult[] }
  | { replay: number }
  | { throw: string };

interface StageRecord {
  request: DispatchStageRequest;
  injected: { results: DispatchNodeResult[] } | { replay: number } | { throw: string };
}

interface RunOutput {
  name: string;
  input: ScenarioInput;
  stages: StageRecord[];
  runUpdates: ConsoleRunUpdate[];
  finalGraph: Graph | null;
  notes: string[];
}

interface ScenarioExpect {
  // 最终一次 consoleRunUpdate 的 status 与 reason（拒绝场景就是 rejected 那条）。
  status: string;
  reason: string;
  // dispatchGraphStage 被调用的次数（拒绝类场景为 0）。
  stages: number;
}

interface Scenario {
  name: string;
  input: ScenarioInput;
  batches: Batch[];
  expect: ScenarioExpect;
  // 场景级钉点（在未归一化的输出上断言），防止夹具写成自洽的废品。
  assert?: (out: RunOutput) => void;
  notes?: string[];
}

const baseInput = (overrides: Partial<ScenarioInput> = {}): ScenarioInput => ({
  event: { run_id: 'run-1', project: 'D:/project', text: '实现任务', preset: 'builtin-economy' },
  settings: { ...DEFAULT_DISPATCH_SETTINGS },
  models: MODELS,
  resourcesError: null,
  presets: PRESETS,
  roles: ROLES,
  runs: [SELF_RUN],
  existingGraphs: [],
  git: { state: 'no_git' },
  ...overrides,
});

const settingsWith = (overrides: Partial<UiDispatchSettings>): UiDispatchSettings => ({
  ...DEFAULT_DISPATCH_SETTINGS, ...overrides,
});

const ok = (text: string, extra: Partial<DispatchNodeResult> = {}): Batch => ({
  results: (request) => request.nodes.map((node) => res(node, { text, ...extra })),
});

const inject = (results: (request: DispatchStageRequest) => DispatchNodeResult[]): Batch => ({ results });

const existingGraph: Graph = {
  version: 1, id: 'existing-graph', title: '既有图', root: 'old-root', project: 'D:/project',
  created_at: '2026-09-28T00:00:00.000Z', updated_at: '2026-09-28T00:00:04.000Z',
  nodes: {
    'old-root': {
      id: 'old-root', kind: 'material', parents: [],
      user: { text: '既有需求', attachments: [] }, created_at: '2026-09-28T00:00:00.000Z',
    },
    'old-exec-a': {
      id: 'old-exec-a', kind: 'turn', parents: ['old-root'],
      user: { text: '', attachments: [] }, reply: { status: 'done', text: '甲路完成' },
      target: target(), created_at: '2026-09-28T00:00:01.000Z',
    },
    'old-exec-b': {
      id: 'old-exec-b', kind: 'turn', parents: ['old-root'],
      user: { text: '', attachments: [] }, reply: { status: 'done', text: '乙路完成' },
      target: target({ agent: 'kimi', model: 'review' }), created_at: '2026-09-28T00:00:02.000Z',
    },
    'old-accept': {
      id: 'old-accept', kind: 'turn', parents: ['old-exec-a'],
      user: { text: '', attachments: [] }, reply: { status: 'done', text: 'NEEDS_FIX\n- 旧账未清' },
      target: target({ agent: 'kimi', model: 'review', role: '验收', visibility: '盲' }),
      created_at: '2026-09-28T00:00:03.000Z',
    },
    'old-leftover': {
      id: 'old-leftover', kind: 'turn', parents: ['old-accept'],
      user: { text: '遗留：1 条未闭合，来源 验收（kimi/review）', attachments: [] },
      leftover: { source_node_id: 'old-accept', items: ['- 旧账未清'] },
      created_at: '2026-09-28T00:00:04.000Z',
    },
  },
};

const advisorPreset: Preset = {
  id: 'custom-advisor', name: '', builtin: false,
  stages: [
    { nodes: [target()] },
    { nodes: [{ agent: '', model: '', role: '顾问', thinking: '中', visibility: '盲', write: '只读' }] },
  ],
};

const chainPreset: Preset = {
  id: 'custom-chain', name: '', builtin: false,
  stages: [
    { nodes: [target({ tier: '中' })] },
    { nodes: [target({ tier: '中' })] },
    { nodes: [target({ tier: '中' })] },
  ],
};

function scenarios(): Array<[string, Scenario]> {
  const rows: Array<[string, Scenario]> = [
    ['economy-pass', {
      name: '经济模式三阶段一次通过',
      input: baseInput(),
      batches: [
        ok('方案：分两步实现。'),
        ok('实现完成', { artifacts: ARTIFACTS }),
        inject((request) => [res(request.nodes[0], { text: 'PASS' }), res(request.nodes[1], { text: '核对一致' })]),
      ],
      expect: { status: 'done', reason: '', stages: 3 },
      assert: (out) => {
        // 第三阶段一个请求两路并发：验收 + 验证。
        expect(out.stages[2].request.nodes.map((node) => node.role)).toEqual(['验收', '验证']);
        // 盲验收不看执行正文（产物走目录），但带上全部祖先的用户消息。
        expect(out.stages[2].request.nodes[0].prompt).toContain('[用户]\n实现任务');
        expect(out.stages[2].request.nodes[0].prompt).not.toContain('实现完成');
        expect(out.stages[2].request.nodes[0].upstreamArtifacts).toEqual([ARTIFACTS]);
      },
    }],
    ['needs-fix-pass', {
      name: '执行（带产物）→ 验收 NEEDS_FIX → 修正轮 → 通过',
      input: baseInput(),
      batches: [
        ok('方案：分两步实现。'),
        ok('实现完成', { artifacts: ARTIFACTS }),
        inject((request) => [res(request.nodes[0], { text: 'NEEDS_FIX\n- 补回归测试' }), res(request.nodes[1], { text: '核对一致' })]),
        ok('已补回归测试', { artifacts: ARTIFACTS_ROUND2 }),
        inject((request) => [res(request.nodes[0], { text: 'PASS' }), res(request.nodes[1], { text: '核对一致' })]),
      ],
      expect: { status: 'done', reason: '', stages: 5 },
      assert: (out) => {
        const correction = out.stages[3].request.nodes[0];
        expect(correction.role).toBe('执行');
        // R1-2 偏离：修正轮执行节点的 fixRound 按 isFixRoundExecution 计算写入。
        expect((correction as { fixRound?: boolean }).fixRound).toBe(true);
        // 修正轮正文以「根据上一轮验收意见修正实现。」开头。
        const execNode = Object.values(out.finalGraph!.nodes).find((node) => node.id === correction.nodeId);
        expect(execNode?.user.text.startsWith('根据上一轮验收意见修正实现。')).toBe(true);
        // 修正轮的验证兄弟跟着长出来，与修正验收同批派发。
        expect(out.stages[4].request.nodes.map((node) => node.role)).toEqual(['验收', '验证']);
      },
    }],
    ['fix-limit-leftover', {
      name: '修正轮到上限 → 遗留节点',
      input: baseInput(),
      batches: [
        ok('方案：分两步实现。'),
        ok('实现完成', { artifacts: ARTIFACTS }),
        inject((request) => [res(request.nodes[0], { text: 'NEEDS_FIX\n- 第一项\n- 第二项' }), res(request.nodes[1], { text: '核对一致' })]),
        ok('已修一轮', { artifacts: ARTIFACTS_ROUND2 }),
        inject((request) => [res(request.nodes[0], { text: 'NEEDS_FIX\n- 仍未闭合' }), res(request.nodes[1], { text: '核对一致' })]),
      ],
      expect: { status: 'done', reason: '', stages: 5 },
      assert: (out) => {
        const leftover = Object.values(out.finalGraph!.nodes).find((node) => node.leftover);
        expect(leftover?.leftover?.items).toEqual(['- 仍未闭合']);
        expect(out.runUpdates.at(-1)?.leftover).toEqual(['- 仍未闭合']);
      },
    }],
    ['validation-retry-limit', {
      name: '验证命令失败重试到上限，等待中的验收与验证兄弟一并标跳过',
      input: baseInput(),
      batches: [
        ok('方案：分两步实现。'),
        inject((request) => [validationFailed(request.nodes[0])]),
        inject((request) => [validationFailed(request.nodes[0])]),
      ],
      expect: { status: 'failed', reason: '', stages: 3 },
      assert: (out) => {
        const nodes = Object.values(out.finalGraph!.nodes);
        // 第一次重试：等待中的验收标「验证命令未通过，未送审」，同批 draft 的
        // 验证兄弟标跳过（appendValidationRetry 语义）。
        expect(nodes.find((node) => node.target?.role === '验收')?.reply?.text).toBe('验证命令未通过，未送审');
        expect(nodes.find((node) => node.target?.role === '验证')?.reply?.skipped_reason).toBe('上游验证未通过');
        // 重试到上限：新一轮等待中的验收也标跳过（closeWaitingAcceptance 语义）。
        const skipped = nodes.filter((node) => node.reply?.skipped_reason === '上游验证未通过');
        expect(skipped.length).toBe(2);
        // 重试节点正文以 [修正 1/1] 开头 → 请求带 fixRound（R1-2 偏离口径）。
        expect((out.stages[2].request.nodes[0] as { fixRound?: boolean }).fixRound).toBe(true);
      },
    }],
    ['blind-three-way', {
      name: '盲审模式三路并发（一路验收 + 两路验证，同一阶段一个请求）',
      input: baseInput({
        event: {
          run_id: 'run-1', project: 'D:/project', text: '审查这批改动',
          targets: [
            target({ agent: 'claude', model: 'plan', role: '验收', visibility: '盲' }),
            target({ agent: 'codex', model: 'exec', role: '验证', visibility: '盲' }),
            target({ agent: 'kimi', model: 'review', role: '验证', visibility: '盲' }),
          ],
        },
      }),
      batches: [
        inject((request) => request.nodes.map((node, index) => res(node, { text: index === 0 ? 'PASS' : '核对一致' }))),
      ],
      expect: { status: 'done', reason: '', stages: 1 },
      assert: (out) => {
        const nodes = out.stages[0].request.nodes;
        expect(nodes.map((node) => node.role)).toEqual(['验收', '验证', '验证']);
        expect(nodes.map((node) => node.visibility)).toEqual(['盲', '盲', '盲']);
        // 盲的「验证」那一路带审查材料验证标记；验收不带。
        expect(nodes.map((node) => node.reviewMaterialValidation ?? false)).toEqual([false, true, true]);
      },
    }],
    ['shared', {
      name: '共享模式一路一次通过',
      input: baseInput({ event: { run_id: 'run-1', project: 'D:/project', text: '实现任务', preset: 'builtin-shared' } }),
      batches: [ok('完成')],
      expect: { status: 'done', reason: '', stages: 1 },
    }],
    ['quota-exhausted', {
      name: '执行路 quota_exhausted，节点记额度用尽，run 仍收尾 done',
      input: baseInput({ event: { run_id: 'run-1', project: 'D:/project', text: '实现任务', preset: 'builtin-shared' } }),
      batches: [
        inject((request) => [res(request.nodes[0], {
          status: 'quota_exhausted', text: '', error: '额度用尽', quotaResetsAt: '2026-10-01T00:00:00Z', exitCode: 1,
        })]),
      ],
      expect: { status: 'done', reason: '', stages: 1 },
      assert: (out) => {
        const node = Object.values(out.finalGraph!.nodes).find((value) => value.target);
        expect(node?.reply?.status).toBe('quota_exhausted');
        expect(node?.reply?.quota_resets_at).toBe('2026-10-01T00:00:00Z');
        expect(out.runUpdates.at(-1)?.nodes[0].quota_resets_at).toBe('2026-10-01T00:00:00Z');
      },
    }],
    ['duplicate-results', {
      name: '同一批结果重复送达 → 停止推进，本层节点放回待派',
      input: baseInput(),
      batches: [
        ok('方案：分两步实现。'),
        inject((request) => [validationFailed(request.nodes[0])]),
        { replay: 1 },
      ],
      expect: { status: 'failed', reason: '同一批结果重复送达，已停止推进；这一层的节点已放回待派。', stages: 3 },
      assert: (out) => {
        // 重试节点（第三次派发的对象）在最终图上是 draft（从 pending 放回）。
        const retryId = out.stages[2].request.nodes[0].nodeId;
        expect(out.finalGraph!.nodes[retryId]?.reply?.status).toBe('draft');
      },
    }],
    ['missing-next-target', {
      name: '下一阶段节点缺目标 → 停止并收尾 failed',
      input: baseInput({
        event: { run_id: 'run-1', project: 'D:/project', text: '实现任务', preset: 'custom-advisor' },
        presets: [...PRESETS, advisorPreset],
      }),
      batches: [ok('执行完成')],
      expect: { status: 'failed', reason: '下一阶段有节点未指定模型，请先选择目标。', stages: 1 },
    }],
    ['request-limit', {
      name: '阶段间累计请求超总上限 → 停止并收尾 failed',
      input: baseInput({ settings: settingsWith({ maxRequests: 4 }) }),
      batches: [
        ok('方案：分两步实现。'),
        ok('实现完成', { artifacts: ARTIFACTS }),
        inject((request) => [res(request.nodes[0], { text: 'NEEDS_FIX\n- 补回归测试' }), res(request.nodes[1], { text: '核对一致' })]),
      ],
      expect: { status: 'failed', reason: '已达总请求上限 4，继续派发需重新确认。', stages: 3 },
      assert: (out) => {
        // 修正轮已经长出来但没有派出去（上限停在下一次派发之前）。
        const correction = Object.values(out.finalGraph!.nodes)
          .find((node) => node.user.text.startsWith('根据上一轮验收意见修正实现。'));
        expect(correction?.reply?.status).toBe('draft');
      },
    }],
    ['targets-direct', {
      name: 'targets 直给，不查预设文件',
      input: baseInput({
        event: {
          run_id: 'run-1', project: 'D:/project', text: '实现任务',
          targets: [target({ write: '单写者', tier: '低', thinking: '低' })],
        },
      }),
      batches: [ok('实现完成', { artifacts: ARTIFACTS })],
      expect: { status: 'done', reason: '', stages: 1 },
    }],
    ['continue-existing-graph', {
      name: '续写已有图（带分叉与遗留节点，source 原样保留为空）',
      input: baseInput({
        event: { run_id: 'run-1', project: 'D:/project', text: '实现任务', preset: 'builtin-shared' },
        existingGraphs: [existingGraph],
      }),
      batches: [ok('新需求完成')],
      expect: { status: 'done', reason: '', stages: 1 },
      assert: (out) => {
        expect(out.finalGraph!.id).toBe('existing-graph');
        // 续写已有图：source 不被改写成 console（对象上可能挂着 undefined，落盘即消失）。
        expect(out.finalGraph!.source).toBeUndefined();
        // 新材料挂在列末端（遗留节点不算末端，挂在它来源的验收节点下）。
        const material = Object.values(out.finalGraph!.nodes).find((node) => node.user.text === '实现任务');
        expect(material?.parents).toEqual(['old-accept']);
        // 既有节点原样保留；旧遗留节点的条目照常进 run 摘要。
        expect(out.finalGraph!.nodes['old-leftover'].leftover?.items).toEqual(['- 旧账未清']);
        expect(out.runUpdates.at(-1)?.leftover).toEqual(['- 旧账未清']);
      },
    }],
    ['role-defaults', {
      name: '盲审预设在角色默认跳过名单 → 角色没有可派发目标',
      input: baseInput({ event: { run_id: 'run-1', project: 'D:/project', text: '审查这批改动', preset: 'builtin-blind' } }),
      batches: [],
      expect: { status: 'rejected', reason: '角色「验收」没有可派发目标', stages: 0 },
    }],
    ['reset-draft', {
      name: '修正轮重试批重复送达 → 在飞的重试节点放回待派',
      input: baseInput(),
      batches: [
        ok('方案：分两步实现。'),
        ok('实现完成', { artifacts: ARTIFACTS }),
        inject((request) => [res(request.nodes[0], { text: 'NEEDS_FIX\n- 改进入口校验' }), res(request.nodes[1], { text: '核对一致' })]),
        inject((request) => [validationFailed(request.nodes[0])]),
        { replay: 3 },
      ],
      expect: { status: 'failed', reason: '同一批结果重复送达，已停止推进；这一层的节点已放回待派。', stages: 5 },
      assert: (out) => {
        const retryId = out.stages[4].request.nodes[0].nodeId;
        expect(out.finalGraph!.nodes[retryId]?.reply?.status).toBe('draft');
        expect(out.finalGraph!.nodes[retryId]?.user.text.startsWith('[修正 1/1]')).toBe(true);
      },
    }],
    ['multi-attachment', {
      name: '多附件按首次出现顺序保留（根材料顺序非字母序）',
      input: baseInput({
        event: {
          run_id: 'run-1', project: 'D:/project', text: '参考三份材料实现任务', preset: 'builtin-shared',
          attachments: ['D:/project/zeta.md', 'D:/project/alpha.md', 'D:/project/mu.md'],
        },
      }),
      batches: [ok('完成')],
      expect: { status: 'done', reason: '', stages: 1 },
      assert: (out) => {
        expect(out.stages[0].request.nodes[0].materials).toEqual([
          'D:/project/zeta.md', 'D:/project/alpha.md', 'D:/project/mu.md',
        ]);
      },
    }],
    ['stage-throw', {
      name: '单阶段派发抛错 → 在飞节点补失败桩、停止推进、run 收尾 failed',
      input: baseInput(),
      batches: [
        ok('方案：分两步实现。'),
        { throw: '引擎进程崩溃' },
      ],
      expect: { status: 'failed', reason: 'Error: 引擎进程崩溃', stages: 2 },
      assert: (out) => {
        const failedId = out.stages[1].request.nodes[0].nodeId;
        const reply = out.finalGraph!.nodes[failedId]?.reply;
        expect(reply?.status).toBe('failed');
        expect(reply?.runner).toBe('unknown');
        expect(reply?.error_kind).toBe('error');
      },
    }],
    ['fallback-continuation', {
      name: '备选作答后不续会话；正常作答后下一阶段带会话与运行方式',
      input: baseInput({
        event: { run_id: 'run-1', project: 'D:/project', text: '实现任务', preset: 'custom-chain' },
        presets: [...PRESETS, chainPreset],
      }),
      batches: [
        inject((request) => [res(request.nodes[0], { text: '第一棒', sessionId: 'sess-1', fallbackFrom: 'codex/exec', fallbackTo: 'claude/plan' })]),
        inject((request) => [res(request.nodes[0], { text: '第二棒', sessionId: 'sess-2', runner: 'acp' })]),
        inject((request) => [res(request.nodes[0], { text: '第三棒' })]),
      ],
      expect: { status: 'done', reason: '', stages: 3 },
      assert: (out) => {
        // 走了备选的那一路：下一阶段不续会话（会话属于备选通道）。
        expect(out.stages[1].request.nodes[0].sessionId).toBe('');
        expect(out.stages[1].request.nodes[0].previousRunner).toBe('');
        // 正常作答：下一阶段续上会话与运行方式。
        expect(out.stages[2].request.nodes[0].sessionId).toBe('sess-2');
        expect(out.stages[2].request.nodes[0].previousRunner).toBe('acp');
      },
    }],
    ['leftover-text-bullet-forms', {
      name: '遗留条目识别：（1）与 (2) 与 - 形态是条目，「3 处需要修改」不是',
      input: baseInput({ settings: settingsWith({ fixRoundLimit: 0 }) }),
      batches: [
        ok('方案：分两步实现。'),
        ok('实现完成', { artifacts: ARTIFACTS }),
        inject((request) => [
          res(request.nodes[0], { text: 'NEEDS_FIX\n3 处需要修改\n（1）补入口校验\n(2) 补错误处理\n- 更新文档' }),
          res(request.nodes[1], { text: '核对一致' }),
        ]),
      ],
      expect: { status: 'done', reason: '', stages: 3 },
      assert: (out) => {
        expect(out.runUpdates.at(-1)?.leftover).toEqual(['（1）补入口校验', '(2) 补错误处理', '- 更新文档']);
      },
    }],
    ['leftover-text-plain-fallback', {
      name: '遗留条目无条目行 → 整段按行兜底（含「3 处需要修改」一行）',
      input: baseInput({ settings: settingsWith({ fixRoundLimit: 0 }) }),
      batches: [
        ok('方案：分两步实现。'),
        ok('实现完成', { artifacts: ARTIFACTS }),
        inject((request) => [
          res(request.nodes[0], { text: 'NEEDS_FIX\n3 处需要修改\n入口校验缺失\n错误处理不一致' }),
          res(request.nodes[1], { text: '核对一致' }),
        ]),
      ],
      expect: { status: 'done', reason: '', stages: 3 },
      assert: (out) => {
        expect(out.runUpdates.at(-1)?.leftover).toEqual(['NEEDS_FIX', '3 处需要修改', '入口校验缺失', '错误处理不一致']);
      },
    }],
    ['precheck-order-inflight-preset', {
      name: '在飞上限与找不到预设同时成立 → 先报在飞上限',
      input: baseInput({
        event: { run_id: 'run-1', project: 'D:/project', text: '实现任务', preset: 'missing' },
        runs: [runningRun('run-a'), runningRun('run-b'), runningRun('run-c'), SELF_RUN],
      }),
      batches: [],
      expect: { status: 'rejected', reason: '同时在飞 run 已达上限 3', stages: 0 },
    }],
    ['precheck-order-quota-limit', {
      name: '额度用尽与超过总请求上限同时成立 → 先报额度用尽',
      input: baseInput({
        event: {
          run_id: 'run-1', project: 'D:/project', text: '实现任务',
          targets: [target(), target({ agent: 'kimi', model: 'review' })],
        },
        settings: settingsWith({ maxRequests: 1 }),
        models: [
          MODELS[0],
          model('codex', 'exec', { tier: '低', thinking: '低', quota_exhausted_until: '2026-10-01T00:00:00Z' }),
          model('kimi', 'review', { tier: '中', thinking: '中', quota_exhausted_until: '2026-10-01T00:00:00Z' }),
        ],
      }),
      batches: [],
      expect: { status: 'rejected', reason: '目标额度用尽：codex/exec；恢复时间 2026-10-01T00:00:00Z', stages: 0 },
    }],
  ];

  const prechecks: Array<[string, string, Partial<ScenarioInput>, string]> = [
    ['precheck-01', '同时在飞 run 已达上限', {
      runs: [runningRun('run-a'), runningRun('run-b'), runningRun('run-c'), SELF_RUN],
    }, '同时在飞 run 已达上限 3'],
    ['precheck-02', '读取资源失败', { resourcesError: '示例' }, '读取资源失败：示例'],
    ['precheck-03', '找不到预设', {
      event: { run_id: 'run-1', project: 'D:/project', text: '实现任务', preset: 'missing' },
    }, '找不到预设：missing'],
    ['precheck-04', '角色没有可派发目标（角色默认全空）', {
      event: { run_id: 'run-1', project: 'D:/project', text: '实现任务', preset: 'builtin-shared' },
      roles: EMPTY_ROLES,
    }, '角色「执行」没有可派发目标'],
    ['precheck-05', '目标未接入', {
      event: { run_id: 'run-1', project: 'D:/project', text: '实现任务', preset: 'builtin-shared' },
      models: [MODELS[0]],
    }, '未接入'],
    ['precheck-06', '验收节点只能只读', {
      event: {
        run_id: 'run-1', project: 'D:/project', text: '实现任务',
        targets: [target({ agent: 'kimi', model: 'review', role: '验收', visibility: '盲', write: '单写者' })],
      },
    }, '验收/验证节点只能只读'],
    ['precheck-07', '目标验证失败', {
      event: { run_id: 'run-1', project: 'D:/project', text: '实现任务', preset: 'builtin-shared' },
      models: [MODELS[0], model('codex', 'exec', { tier: '低', thinking: '低', verification: 'failed' }), MODELS[2]],
    }, '目标验证失败：codex/exec'],
    ['precheck-08', '目标额度用尽（含恢复时间）', {
      event: { run_id: 'run-1', project: 'D:/project', text: '实现任务', preset: 'builtin-shared' },
      models: [MODELS[0], model('codex', 'exec', { tier: '低', thinking: '低', quota_exhausted_until: '2026-10-01T00:00:00Z' }), MODELS[2]],
    }, '目标额度用尽：codex/exec；恢复时间 2026-10-01T00:00:00Z'],
    ['precheck-09', '本次派发超过总请求上限', {
      event: {
        run_id: 'run-1', project: 'D:/project', text: '实现任务',
        targets: [target({ agent: 'claude', model: 'plan' }), target(), target({ agent: 'kimi', model: 'review' })],
      },
      settings: settingsWith({ maxRequests: 2 }),
    }, '本次派发 3 路，超过总请求上限 2'],
    ['precheck-10', '项目目录有未提交改动且有 worktree 节点', {
      event: {
        run_id: 'run-1', project: 'D:/project', text: '实现任务',
        targets: [target({ write: 'worktree' })],
      },
      git: DIRTY_GIT,
    }, '项目目录有 1 处未提交改动；worktree 从 HEAD 新建，看不到这些改动'],
  ];
  for (const [slug, name, overrides, reason] of prechecks) {
    rows.push([slug, {
      name: `派发前拒绝：${name}`,
      input: baseInput(overrides),
      batches: [],
      expect: { status: 'rejected', reason, stages: 0 },
    }]);
  }
  return rows;
}

// ── 假 io / context 驱动 runConsoleDispatch ──────────────────────────────

const FIXROUND_NOTE = '请求里的 fixRound 按 isFixRoundExecution 计算写入夹具（R1-2 口径）；'
  + '窗口控制台路径的 buildRequest 从不带该字段，属 TS 侧缺陷，本期不改 TS 行为。';

async function runScenario(scenario: Scenario): Promise<RunOutput> {
  const { input } = scenario;
  vi.stubGlobal('localStorage', { getItem: () => JSON.stringify(input.settings) });
  const stages: StageRecord[] = [];
  const runUpdates: ConsoleRunUpdate[] = [];
  let lastSaved: Graph | null = null;
  let dispatchCalls = 0;
  const state: { graphs: Record<string, Graph>; terminals: never[]; activeTerminalId: null } = {
    graphs: Object.fromEntries(input.existingGraphs.map((graph) => [graph.id, graph])),
    terminals: [],
    activeTerminalId: null,
  };
  const context = {
    getState: () => state,
    dispatch: (action: { type: string; graph?: Graph }) => {
      if (action.type === 'SET_GRAPH' && action.graph) state.graphs[action.graph.id] = action.graph;
    },
  } as unknown as ConsoleChannelContext;
  const io: ChannelIo = {
    listDispatchableModels: async () => ({ models: input.models, error: input.resourcesError }),
    listPresets: async () => ({ version: 1, presets: input.presets }),
    loadRoles: async () => input.roles,
    listConsoleRuns: async () => input.runs,
    gitChanges: async () => input.git,
    previewDispatch: async () => ({ totalRedacted: 0, broadRedacted: 0, nodes: [] }),
    dispatchGraphStage: async (request: DispatchStageRequest) => {
      const batch = scenario.batches[dispatchCalls];
      dispatchCalls += 1;
      if (!batch) throw new Error(`场景未提供第 ${dispatchCalls} 次派发的注入结果`);
      // 落进夹具的是线上真实发出的形状（undefined 字段在 IPC 边界被剥掉）。
      const recorded = JSON.parse(JSON.stringify(request)) as DispatchStageRequest;
      // 唯一有意偏离（fix-36-r1.md R1-2）：fixRound 按 isFixRoundExecution 计算
      // 后写入夹具；窗口控制台路径实际从不带该字段（TS 侧缺陷，不改 TS 行为）。
      if (lastSaved) {
        for (const node of recorded.nodes) {
          if (isFixRoundExecution(lastSaved, node.nodeId)) {
            (node as DispatchNodeRequest & { fixRound?: boolean }).fixRound = true;
          }
        }
      }
      if ('throw' in batch) {
        stages.push({ request: recorded, injected: { throw: batch.throw } });
        throw new Error(batch.throw);
      }
      if ('replay' in batch) {
        const source = stages[batch.replay].injected;
        if (!('results' in source)) throw new Error(`stages[${batch.replay}] 没有可重放的结果`);
        stages.push({ request: recorded, injected: { replay: batch.replay } });
        return { runId: request.runId, results: JSON.parse(JSON.stringify(source.results)) as DispatchNodeResult[], stateFile: '' };
      }
      const results = batch.results(request);
      stages.push({ request: recorded, injected: { results } });
      return { runId: request.runId, results, stateFile: '' };
    },
    saveGraph: async (graph: Graph) => { lastSaved = graph; },
    consoleRunUpdate: async (update: ConsoleRunUpdate) => {
      runUpdates.push(JSON.parse(JSON.stringify(update)) as ConsoleRunUpdate);
    },
    consoleRegisterProposal: async () => {},
    cancelDispatchNode: async () => true,
    consoleActionAck: async () => {},
  };
  await runConsoleDispatch(input.event, context, io);
  const hasFixRound = stages.some((stage) => stage.request.nodes.some((node) => 'fixRound' in node));
  return {
    name: scenario.name,
    input,
    stages,
    runUpdates,
    finalGraph: lastSaved,
    notes: [...(scenario.notes ?? []), ...(hasFixRound ? [FIXROUND_NOTE] : [])],
  };
}

// ── 归一化：UUID 与时间戳按出现顺序替换 ──────────────────────────────────

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

// ── 确定性替身 ───────────────────────────────────────────────────────────

let uuidSeq = 0;

beforeEach(() => {
  uuidSeq = 0;
  vi.useFakeTimers({ toFake: ['Date'] });
  vi.setSystemTime(new Date('2026-09-30T00:00:00.000Z'));
  vi.stubGlobal('crypto', {
    randomUUID: () => `00000000-0000-4000-8000-${String(++uuidSeq).padStart(12, '0')}`,
  });
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe('TASK-36 R1 引擎金标准（直接驱动 runConsoleDispatch）', () => {
  for (const [slug, scenario] of scenarios()) {
    it(slug, async () => {
      const out = await runScenario(scenario);
      const last = out.runUpdates.at(-1);
      expect(out.stages.length).toBe(scenario.expect.stages);
      expect(last?.status).toBe(scenario.expect.status);
      expect(last?.reason ?? '').toBe(scenario.expect.reason);
      if (scenario.expect.stages === 0 && scenario.expect.status === 'rejected') {
        expect(out.finalGraph).toBeNull();
      }
      scenario.assert?.(out);
      const actual = normalize(out);
      const path = resolve(fixtureDir, `${slug}.json`);
      if (process.env.AICROSS_WRITE_GOLDEN === '1') {
        mkdirSync(fixtureDir, { recursive: true });
        writeFileSync(path, `${JSON.stringify(actual, null, 2)}\n`, 'utf8');
      }
      expect(actual).toEqual(JSON.parse(readFileSync(path, 'utf8')));
    });
  }
});
