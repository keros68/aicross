// TASK-12：修正轮上限 + 遗留节点、验收与盲审同阶段并行、审查材料附带验证输出。
// 契约只追加：DispatchSettings.fixRoundLimit、GraphNode.leftover、
// DispatchNodeRequest.reviewMaterialValidation、两份提示词文件末尾的固定关注点。

import { describe, expect, it } from 'vitest';
import orchestrationSource from '../../../../src/orchestration.rs?raw';
import manualSource from '../../../../docs/MANUAL.md?raw';
import explainSource from '../../components/right/orchestration/ExplainOverlay.tsx?raw';
import settingsSource from '../../components/common/SettingsModal.tsx?raw';
import {
  appendCorrectionRound,
  appendLeftover,
  applyRoleDefaults,
  buildNodeRequest,
  dispatchableNodeIds,
  leftoverItems,
  leftoverLine,
  leftoverNodes,
  nodeStatusLabel,
  presetWithRoleDefaults,
  reducePipelineStep,
  reviewMaterialValidationOf,
  routeLine,
  runStageLoop,
  DEFAULT_FIX_ROUND_LIMIT,
  LEFTOVER_LINE_LIMIT,
  ROLE_PROMPT_TEMPLATES,
  type BackendNodeResult,
  type NodeRequestParts,
  type StageLoopState,
} from '../dispatch.ts';
import { BLIND_PROMPT_PREFIX } from '../context.ts';
import { PRESET_NODE_TEXTS, VERIFICATION_ROUTE_TEXT, childrenOf, stamp } from '../stamp.ts';
import { nodeLabel } from '../label.ts';
import { newGraph } from '../types.ts';
import type { Graph, GraphNode, Preset, Target } from '../types.ts';
import { continuationNodeId, latestLeaf } from '../layout.ts';
import { appendTurn, removeDraftNode } from '../mutate.ts';

const CONTRACT_FOCUS = '契约（字段、命令签名、序列化大小写）有没有变';
const RED_TEST_FOCUS = '新增的测试能不能红：把修复回退后该测试是否会失败';

const artifacts = { diff: 'x', changed_files: [], stdout_summary: 'ok' };

function target(patch: Partial<Target> = {}): Target {
  return { agent: 'codex', model: 'm', role: '执行', thinking: '中', visibility: '共享', write: '只读', ...patch };
}

function node(id: string, parents: string[], patch: Partial<GraphNode> = {}): GraphNode {
  return {
    id,
    kind: parents.length === 0 ? 'material' : parents.length > 1 ? 'accept' : 'turn',
    parents,
    user: { text: id, attachments: [] },
    created_at: id,
    ...patch,
  };
}

function graph(nodes: GraphNode[]): Graph {
  return { version: 1, id: 'g', title: '', root: 'r', nodes: Object.fromEntries(nodes.map((n) => [n.id, n])), created_at: '', updated_at: '' };
}

// r → e1 → a1（验收）+ v1（验证，同父）→ …：rounds 个「执行→验收」串起来。
function pipeline(rounds: number, acceptText = 'NEEDS_FIX\n- 测试未覆盖\n- 文案未改'): Graph {
  const nodes = [node('r', [])];
  let parent = 'r';
  for (let round = 1; round <= rounds; round += 1) {
    nodes.push(node(`e${round}`, [parent], { reply: { status: 'done', text: 'done', artifacts }, target: target({ write: '单写者' }) }));
    nodes.push(node(`a${round}`, [`e${round}`], { reply: { status: 'done', text: acceptText }, target: target({ agent: 'kimi', model: 'k', role: '验收', visibility: '盲' }) }));
    if (round === 1) {
      nodes.push(node('v1', ['e1'], { user: { text: VERIFICATION_ROUTE_TEXT, attachments: [] }, reply: { status: 'done', text: '盲审意见' }, target: target({ agent: 'pi', model: 'ds', role: '验证', visibility: '盲' }) }));
    }
    parent = `a${round}`;
  }
  return graph(nodes);
}

function acceptResult(nodeId: string, text = 'NEEDS_FIX\n- 测试未覆盖\n- 文案未改'): BackendNodeResult {
  return { nodeId, status: 'ok', text, modelReported: '', sessionId: '', tokensIn: 0, tokensOut: 0, durationMs: 0, error: '' };
}

function validationFailure(nodeId: string): BackendNodeResult {
  return { nodeId, status: 'validation_failed', text: '', modelReported: '', sessionId: '', tokensIn: 0, tokensOut: 0, durationMs: 0, error: 'pytest failed', validationOutput: '1 failed' };
}

function failedResult(nodeId: string, status: 'error' | 'timeout' = 'error'): BackendNodeResult {
  return { ...acceptResult(nodeId, ''), status, error: status };
}

describe('TASK-12 修正轮上限与遗留节点', () => {
  it('默认上限 1', () => {
    expect(DEFAULT_FIX_ROUND_LIMIT).toBe(1);
  });

  it('上限 1：第一轮 NEEDS_FIX 派修正，第二轮 NEEDS_FIX 只生成遗留节点', () => {
    const first = reducePipelineStep(pipeline(1), [acceptResult('a1')], {}, 1);
    expect(first.continueRunning).toBe(true);
    const grown = Object.values(first.graph.nodes).filter((n) => !pipeline(1).nodes[n.id]);
    expect(grown.some((n) => n.target?.role === '执行')).toBe(true);
    expect(grown.some((n) => n.target?.role === '验收')).toBe(true);
    expect(grown.some((n) => n.leftover)).toBe(false);

    const g = pipeline(2);
    const second = reducePipelineStep(g, [acceptResult('a2')], {}, 1);
    expect(second.continueRunning).toBe(false);
    const added = Object.values(second.graph.nodes).filter((n) => !g.nodes[n.id]);
    expect(added).toHaveLength(1);
    const leftover = added[0];
    expect(leftover.leftover).toEqual({ source_node_id: 'a2', items: ['- 测试未覆盖', '- 文案未改'] });
    expect(leftover.parents).toEqual(['a2']);
    expect(leftover.kind).toBe('turn');
    expect(leftover.target).toBeUndefined();
    expect(leftover.reply).toBeUndefined();
    expect(leftover.user.text).toBe('遗留：2 条未闭合，来源 验收（kimi/k）');
  });

  it('上限 3：第二轮还派修正，第四轮才生成遗留节点', () => {
    const g2 = pipeline(2);
    const second = reducePipelineStep(g2, [acceptResult('a2')], {}, 3);
    expect(second.continueRunning).toBe(true);
    expect(Object.values(second.graph.nodes).some((n) => n.leftover)).toBe(false);

    const g3 = pipeline(3);
    expect(reducePipelineStep(g3, [acceptResult('a3')], {}, 3).continueRunning).toBe(true);

    const g4 = pipeline(4);
    const fourth = reducePipelineStep(g4, [acceptResult('a4')], {}, 3);
    expect(fourth.continueRunning).toBe(false);
    const added = Object.values(fourth.graph.nodes).filter((n) => !g4.nodes[n.id]);
    expect(added).toHaveLength(1);
    expect(added[0].leftover?.source_node_id).toBe('a4');
  });

  it('上限 0：第一轮 NEEDS_FIX 直接生成遗留节点，不派修正', () => {
    const g = pipeline(1);
    const step = reducePipelineStep(g, [acceptResult('a1')], {}, 0);
    expect(step.continueRunning).toBe(false);
    const added = Object.values(step.graph.nodes).filter((n) => !g.nodes[n.id]);
    expect(added).toHaveLength(1);
    expect(added[0].leftover?.source_node_id).toBe('a1');
  });

  it('PASS 不生成遗留节点', () => {
    const g = pipeline(2);
    const step = reducePipelineStep(g, [acceptResult('a2', 'PASS')], {}, 1);
    expect(step.continueRunning).toBe(false);
    expect(Object.keys(step.graph.nodes)).toHaveLength(Object.keys(g.nodes).length);
  });

  it('遗留节点不在可派发集合里，节点卡没有派发状态', () => {
    const g = pipeline(2);
    const step = reducePipelineStep(g, [acceptResult('a2')], {}, 1);
    const leftover = Object.values(step.graph.nodes).find((n) => n.leftover)!;
    expect(dispatchableNodeIds(step.graph)).not.toContain(leftover.id);
    expect(nodeStatusLabel(leftover)).toBeNull();
    expect(nodeLabel(leftover)).toBe('遗留');
    // 即便有人给它塞了 draft reply 与 target，也不派
    const forced: Graph = { ...step.graph, nodes: { ...step.graph.nodes, [leftover.id]: { ...leftover, reply: { status: 'draft', text: '' }, target: target() } } };
    expect(dispatchableNodeIds(forced)).not.toContain(leftover.id);
    expect(leftoverNodes(step.graph).map((n) => n.id)).toEqual([leftover.id]);
  });

  it('验收 NEEDS_FIX 与验证 error 同批返回时仍生成修正轮', () => {
    const g = pipeline(1);
    const step = reducePipelineStep(g, [acceptResult('a1'), failedResult('v1')], {}, 1);
    expect(step.continueRunning).toBe(true);
    expect(Object.values(step.graph.nodes).filter((n) => !g.nodes[n.id]).map((n) => n.target?.role).sort())
      .toEqual(['执行', '验收', '验证']);
  });

  it('遗留焦点回退到来源验收，新节点不会挂在遗留下；遗留可删除', () => {
    const capped = reducePipelineStep(pipeline(2), [acceptResult('a2')], {}, 1).graph;
    const leftover = leftoverNodes(capped)[0];
    expect(latestLeaf(capped)).not.toBe(leftover.id);
    const parent = continuationNodeId(capped, leftover.id);
    expect(parent).toBe('a2');
    const appended = appendTurn(capped, parent, '继续处理');
    expect(appended.graph.nodes[appended.nodeId].parents).toEqual(['a2']);
    const removed = removeDraftNode(capped, leftover.id);
    expect(removed.nodes[leftover.id]).toBeUndefined();
    expect(leftoverNodes(removed)).toHaveLength(0);
  });

  it('验证命令未通过的重派同样读上限：上限 1 重派一次后把等待中的验收标已跳过', () => {
    const g = graph([
      node('r', []),
      node('e', ['r'], { reply: { status: 'failed', text: '' }, target: target({ write: '单写者' }) }),
      node('a', ['e'], { reply: { status: 'draft', text: '' }, target: target({ agent: 'kimi', model: 'k', role: '验收', visibility: '盲' }) }),
    ]);
    const first = reducePipelineStep(g, [validationFailure('e')], {}, 1);
    expect(first.continueRunning).toBe(true);
    const retry = Object.values(first.graph.nodes).find((n) => !g.nodes[n.id] && n.target?.role === '执行')!;
    expect(retry.user.text.startsWith('[修正 1/1] ')).toBe(true);
    expect(routeLine(retry, [], '部分')).toContain('执行（修正 1/1）');
    expect(first.validationRetries[retry.id]).toBe(1);

    const second = reducePipelineStep(g, [validationFailure('e')], { e: 1 }, 1);
    expect(second.continueRunning).toBe(false);
    expect(second.graph.nodes.a.reply?.skipped_reason).toBe('上游验证未通过');

    const third = reducePipelineStep(g, [validationFailure('e')], { e: 1 }, 3);
    expect(third.continueRunning).toBe(true);
    const retry3 = Object.values(third.graph.nodes).find((n) => !g.nodes[n.id] && n.target?.role === '执行')!;
    expect(retry3.user.text.startsWith('[修正 2/3] ')).toBe(true);
  });

  it('验证命令未通过时同父验证兄弟一并已跳过，不再可派', () => {
    const g = graph([
      node('r', []),
      node('e', ['r'], { reply: { status: 'failed', text: '' }, target: target({ write: '单写者' }) }),
      node('a', ['e'], { reply: { status: 'draft', text: '' }, target: target({ role: '验收', visibility: '盲' }) }),
      node('v', ['e'], { reply: { status: 'draft', text: '' }, target: target({ role: '验证', visibility: '盲' }) }),
    ]);
    for (const retries of [{}, { e: 1 }] as Record<string, number>[]) {
      const step = reducePipelineStep(g, [validationFailure('e')], retries, 1);
      expect(step.graph.nodes.v.reply?.skipped_reason).toBe('上游验证未通过');
      expect(nodeStatusLabel(step.graph.nodes.v)).toBe('已跳过');
      expect(dispatchableNodeIds(step.graph)).not.toContain('v');
    }
  });

  it('同一验收重复到顶只更新一张遗留卡，并随步状态传出同一个 id', () => {
    const first = reducePipelineStep(pipeline(2), [acceptResult('a2')], {}, 1);
    const id = first.leftoverIds[0];
    const secondGraph = { ...first.graph, nodes: { ...first.graph.nodes, a2: { ...first.graph.nodes.a2, reply: { status: 'done' as const, text: 'NEEDS_FIX\n- 新条目' } } } };
    const second = reducePipelineStep(secondGraph, [acceptResult('a2', 'NEEDS_FIX\n- 新条目')], {}, 1);
    expect(leftoverNodes(second.graph)).toHaveLength(1);
    expect(second.leftoverIds).toEqual([id]);
    expect(second.graph.nodes[id].leftover?.items).toEqual(['- 新条目']);
  });

  it('runStageLoop 把设置里的上限传给推进函数', async () => {
    const g = pipeline(1);
    const draft: Graph = { ...g, nodes: { ...g.nodes, a1: { ...g.nodes.a1, reply: { status: 'draft', text: '' } } } };
    const state: StageLoopState<BackendNodeResult> = { graph: draft, collected: [], activeNodes: [] };
    const stages: string[][] = [];
    const deps = {
      runId: 'run-1',
      buildRequest: (_g: Graph, ids: string[]) => ({ stage: 1, nodes: ids.map((nodeId) => ({ nodeId })) }),
      preview: async () => null,
      dispatch: async (req: { stage: number; nodes: { nodeId: string }[] }) => ({ runId: 'run-1', results: req.nodes.map((n) => acceptResult(n.nodeId, 'NEEDS_FIX\n- 未闭合')) }),
      commit: (next: Graph) => next,
      onPreview: () => undefined,
      onStage: (_req: unknown, ids: string[]) => { stages.push(ids); },
      onResults: () => undefined,
    };
    // 上限 0：验收一跑完就到顶，只生成遗留节点，不再派第二层
    const outcome = await runStageLoop(state, { maxRequests: 10, fixRoundLimit: 0 }, new Set<string>(), deps);
    expect(outcome).toEqual({ kind: 'done' });
    expect(stages).toEqual([['a1']]);
    expect(leftoverNodes(state.graph)).toHaveLength(1);
  });

  it('leftoverItems：取「- 」或编号开头的行，取不到就整段截 20 行', () => {
    expect(leftoverItems('NEEDS_FIX\n- 甲\n* 乙\n1. 丙\n2) 丁\n3、戊\n（4）己\n结论')).toEqual(['- 甲', '* 乙', '1. 丙', '2) 丁', '3、戊', '（4）己']);
    const plain = Array.from({ length: 30 }, (_, i) => `第 ${i + 1} 行`).join('\n');
    const items = leftoverItems(plain);
    expect(items).toHaveLength(LEFTOVER_LINE_LIMIT);
    expect(items[0]).toBe('第 1 行');
    expect(leftoverItems('')).toEqual([]);
    expect(leftoverItems('  \n\n')).toEqual([]);
  });

  it('appendLeftover：验收正文为空或节点不存在时不生成', () => {
    const g = graph([node('r', []), node('a', ['r'], { reply: { status: 'done', text: '' }, target: target({ role: '验收' }) })]);
    expect(appendLeftover(g, 'a').leftoverId).toBeUndefined();
    expect(appendLeftover(g, 'missing').graph).toBe(g);
    expect(leftoverLine(3, undefined)).toBe('遗留：3 条未闭合，来源 验收');
  });
});

describe('TASK-12 验收与盲审同阶段并行', () => {
  const economyShape: Preset = {
    id: 'builtin-economy',
    name: '经济模式',
    builtin: true,
    stages: [
      { nodes: [target({ agent: '', model: '', role: '出方案' })] },
      { nodes: [target({ agent: '', model: '', role: '执行', write: '单写者' })] },
      { nodes: [target({ agent: '', model: '', role: '验收', visibility: '盲' }), target({ agent: '', model: '', role: '验证', visibility: '盲' })] },
    ],
  };
  const blindShape: Preset = {
    id: 'builtin-blind',
    name: '盲审模式',
    builtin: true,
    stages: [{ nodes: ['验收', '验证', '验证'].map((role) => target({ agent: '', model: '', role, visibility: '盲' })) }],
  };

  it('引擎侧内置预设：经济模式第三段是「验收 + 验证」，盲审模式是「验收 + 验证 + 验证」', () => {
    const blocks = new Map([...orchestrationSource.matchAll(/let \w+ = Preset \{[\s\S]*?^    \};/gm)]
      .map((match) => [match[0].match(/id: "([^"]+)"/)?.[1], match[0]]));
    const economy = blocks.get('builtin-economy') ?? '';
    const stages = economy.split('Stage {').slice(1);
    expect(stages).toHaveLength(3);
    expect(stages[2]).toContain('role: "验收".into()');
    expect(stages[2]).toContain('role: "验证".into()');
    const blind = blocks.get('builtin-blind') ?? '';
    expect(blind).toContain('["验收", "验证", "验证"]');
  });

  it('经济模式盖章：第三段两路同父，验证那一路带盲审文本，验收不带', () => {
    const g = newGraph();
    const g2 = stamp(g, g.root, economyShape);
    const plan = childrenOf(g2, g.root);
    const exec = childrenOf(g2, plan[0]);
    const third = childrenOf(g2, exec[0]).map((id) => g2.nodes[id]).sort((a, b) => a.created_at.localeCompare(b.created_at));
    expect(third.map((n) => n.target?.role)).toEqual(['验收', '验证']);
    expect(third.every((n) => n.parents.length === 1 && n.parents[0] === exec[0])).toBe(true);
    expect(third[0].user.text).toBe('');
    expect(third[1].user.text).toBe(VERIFICATION_ROUTE_TEXT);
    expect(PRESET_NODE_TEXTS['builtin-economy']).toEqual(['', '', '', VERIFICATION_ROUTE_TEXT]);
  });

  it('盲审模式盖章：一段三路同父，两路验证带盲审文本', () => {
    const g = newGraph();
    const g2 = stamp(g, g.root, blindShape);
    const routes = childrenOf(g2, g.root).map((id) => g2.nodes[id]).sort((a, b) => a.created_at.localeCompare(b.created_at));
    expect(routes.map((n) => n.target?.role)).toEqual(['验收', '验证', '验证']);
    expect(routes.map((n) => n.user.text)).toEqual(['', VERIFICATION_ROUTE_TEXT, VERIFICATION_ROUTE_TEXT]);
    expect(PRESET_NODE_TEXTS['builtin-blind']).toEqual(['', VERIFICATION_ROUTE_TEXT, VERIFICATION_ROUTE_TEXT]);
  });

  it('修正轮：验收那一路与同父的验证那一路一起长出来', () => {
    const g = pipeline(1);
    const { graph: next, executionId, acceptanceId } = appendCorrectionRound(g, 'a1');
    expect(executionId).toBeDefined();
    const added = Object.values(next.nodes).filter((n) => !g.nodes[n.id]);
    expect(added.map((n) => n.target?.role).sort()).toEqual(['执行', '验收', '验证']);
    const verification = added.find((n) => n.target?.role === '验证')!;
    expect(verification.parents).toEqual([executionId]);
    expect(verification.user.text).toBe(VERIFICATION_ROUTE_TEXT);
    expect(verification.target?.visibility).toBe('盲');
    expect(verification.reply).toEqual({ status: 'draft', text: '' });
    expect(next.nodes[acceptanceId!].parents).toEqual([executionId]);
  });

  it('验证门只看验收：验证那一路的 NEEDS_FIX 不触发修正轮', () => {
    const g = pipeline(1);
    const step = reducePipelineStep(g, [acceptResult('v1', 'NEEDS_FIX\n- 盲审意见')], {}, 3);
    expect(step.continueRunning).toBe(true); // 没有验收结果 → 继续推进下一层，不长修正
    expect(Object.keys(step.graph.nodes)).toHaveLength(Object.keys(g.nodes).length);
    const both = reducePipelineStep(g, [acceptResult('a1', 'PASS'), acceptResult('v1', 'NEEDS_FIX')], {}, 3);
    expect(both.continueRunning).toBe(false);
    expect(Object.keys(both.graph.nodes)).toHaveLength(Object.keys(g.nodes).length);
  });

  it('角色默认：验证沿用验收的默认目标；盲审 / 评审预设整体不填', () => {
    const roles = {
      planning: { agent: 'codex', model: 'plan', thinking: '高' },
      execution: { agent: 'codex', model: 'exec', thinking: '低' },
      acceptance: { agent: 'kimi', model: 'k3', thinking: '中' },
    };
    expect(applyRoleDefaults(target({ agent: '', model: '', role: '验证' }), roles)).toMatchObject({ agent: 'kimi', model: 'k3' });
    const economy = presetWithRoleDefaults(economyShape, roles);
    expect(economy.stages[2].nodes.map((t) => t.model)).toEqual(['k3', 'k3']);
    const blind = presetWithRoleDefaults(blindShape, roles);
    expect(blind.stages[0].nodes.every((t) => t.model === '')).toBe(true);
    const jury = presetWithRoleDefaults({ ...blindShape, id: 'builtin-jury', name: '评审模式' }, roles);
    expect(jury.stages[0].nodes.every((t) => t.model === '')).toBe(true);
  });

  it('手册与讲解页说明验收只读判定、验证输出来源及角色默认', () => {
    for (const text of [manualSource, explainSource]) {
      expect(text).toContain('看产物给 PASS / NEEDS_FIX');
      expect(text).toContain('派盲审前由执行节点跑一次验证命令');
      expect(text).toContain('沿用验收角色的默认目标');
    }
    expect(manualSource).toContain('三路都要在确认派发页选择目标');
  });

  it('设置页完整说明共用上限与到顶已跳过', () => {
    expect(settingsSource).toContain('验证命令未通过的重派与修正轮共用此上限');
    expect(settingsSource).toContain('验证未通过到顶时该验收标「已跳过」');
  });
});

describe('TASK-12 审查材料附带验证输出', () => {
  const parts: NodeRequestParts = {
    models: [],
    prompt: '材料',
    materials: [],
    currentMessage: '',
    originalRequest: '原始需求',
    upstreamArtifacts: [artifacts],
    sessionId: '',
    previousRunner: '',
    rollback: false,
    stage: 3,
  };

  it('盲的验证节点请求带 reviewMaterialValidation: true，验收与共享验证不带', () => {
    const verification = buildNodeRequest('v', target({ role: '验证', visibility: '盲' }), parts);
    expect(verification.reviewMaterialValidation).toBe(true);
    expect(JSON.parse(JSON.stringify(verification)).reviewMaterialValidation).toBe(true);
    const acceptance = buildNodeRequest('a', target({ role: '验收', visibility: '盲' }), parts);
    expect('reviewMaterialValidation' in acceptance).toBe(false);
    const shared = buildNodeRequest('s', target({ role: '验证', visibility: '共享' }), parts);
    expect('reviewMaterialValidation' in shared).toBe(false);
    expect(reviewMaterialValidationOf(target({ role: '顾问', visibility: '盲' }))).toBe(false);
  });

  it('验收提示词与盲审前缀末尾各带两条固定关注点（读文件，不复制文本）', () => {
    for (const text of [ROLE_PROMPT_TEMPLATES['验收'], BLIND_PROMPT_PREFIX]) {
      const lines = text.trim().split('\n');
      expect(lines.at(-2)).toContain(CONTRACT_FOCUS);
      expect(lines.at(-1)).toContain(RED_TEST_FOCUS);
    }
  });
});
