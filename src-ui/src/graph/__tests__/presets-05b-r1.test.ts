// TASK-05B 修正轮 R1：fix-05b-r1.md 各条目的 TS 侧测试（编号与任务书一致）。

import { describe, expect, it } from 'vitest';
import { PRESET_NODE_TEXTS, childrenOf, stamp } from '../stamp.ts';
import {
  appendArbitration,
  applyDispatchResults,
  fallbackSkippedLineOf,
  fallbackSpecOf,
  neutralDisputeSummary,
  parseAdvise,
  pickOne,
  presetWithRoleDefaults,
  reducePipelineStep,
  sameParentsResultCount,
  setReplyAdopted,
  writableFallbackHintOf,
  type BackendNodeResult,
} from '../dispatch.ts';
import { serializeContext } from '../context.ts';
import { fillRoot, removeDraftNode, updateNodeUserText } from '../mutate.ts';
import { buildPremiseCheckGraph, matchPremiseResults, type Premise } from '../premise.ts';
import { newGraph, newNodeId, roleDefaults } from '../types.ts';
import type { DispatchableModel, Graph, GraphNode, Preset, Target } from '../types.ts';

function mkTarget(patch: Partial<Target>): Target {
  return {
    agent: '',
    model: '',
    role: '执行',
    thinking: '中',
    visibility: '共享',
    write: '只读',
    ...patch,
  };
}

function mkPreset(id: string, nodes: Target[], builtin = true): Preset {
  return { id, name: id, builtin, stages: [{ nodes }] };
}

function mkModel(patch: Partial<DispatchableModel>): DispatchableModel {
  return {
    id: '', agent: '', provider: '', vendor: '', billing_source: '', tier: '',
    thinking: '', status: '已接入', source: '', smoked_at: '', smoke_model: '',
    smoke_failure: '', meta: '', note: '',
    ...patch,
  };
}

function mkTurn(id: string, parents: string[], patch: Partial<GraphNode> = {}): GraphNode {
  return {
    id,
    kind: 'turn',
    parents,
    user: { text: '', attachments: [] },
    reply: { status: 'done', text: `${id} 的回复` },
    created_at: '2026-09-14T10:00:00Z',
    ...patch,
  };
}

function mkResult(nodeId: string, patch: Partial<BackendNodeResult> = {}): BackendNodeResult {
  return {
    nodeId, status: 'ok', text: `${nodeId} 正文`, modelReported: '', sessionId: '',
    tokensIn: 1, tokensOut: 1, durationMs: 1, error: '',
    ...patch,
  };
}

const ROLES = {
  planning: { agent: 'pi', model: 'glm-plan', thinking: '高' },
  execution: { agent: 'pi', model: 'glm-exec', thinking: '低' },
  acceptance: { agent: 'codex', model: 'gpt-acc', thinking: '中' },
};

describe('R1-1 可写节点不自动切备选', () => {
  it('writableFallbackHintOf：可写节点配了备选才提示', () => {
    const writable = mkTarget({ write: '单写者', fallback: { agent: 'pi', model: 'glm' } });
    expect(writableFallbackHintOf(writable)).toBe('可写节点不自动切备选');
    const worktree = mkTarget({ write: 'worktree', fallback: { agent: 'pi', model: 'glm' } });
    expect(writableFallbackHintOf(worktree)).toBe('可写节点不自动切备选');
    const readonly = mkTarget({ write: '只读', fallback: { agent: 'pi', model: 'glm' } });
    expect(writableFallbackHintOf(readonly)).toBeNull();
    const noFallback = mkTarget({ write: '单写者' });
    expect(writableFallbackHintOf(noFallback)).toBeNull();
  });

  it('fallbackSkippedLineOf：fallbackSkipped 非空时给「主失败，未切备选（可写节点）」', () => {
    expect(fallbackSkippedLineOf({ fallbackSkipped: '可写节点' })).toBe('主失败，未切备选（可写节点）');
    expect(fallbackSkippedLineOf({ fallbackSkipped: '' })).toBeNull();
    expect(fallbackSkippedLineOf({})).toBeNull();
  });
});

describe('R1-2 pickOne：同父不同角色节点互不清 adopted', () => {
  it('擂台候选 pickOne 不动同父顾问节点的 adopted', () => {
    const g = newGraph();
    const arena = newNodeId();
    const advisor = newNodeId();
    const withNodes: Graph = {
      ...g,
      nodes: {
        ...g.nodes,
        [arena]: mkTurn(arena, [g.root], { target: mkTarget({ role: '执行', agent: 'pi', model: 'glm' }) }),
        [advisor]: mkTurn(advisor, [g.root], { target: mkTarget({ role: '顾问', visibility: '盲', write: '只读' }) }),
      },
    };
    const adopted = setReplyAdopted(withNodes, advisor, true);
    expect(adopted.nodes[advisor].reply?.adopted).toBe(true);
    const next = pickOne(adopted, arena);
    expect(next.nodes[arena].reply?.adopted).toBe(true);
    // 顾问节点与擂台候选同父但角色不同，不该被清
    expect(next.nodes[advisor].reply?.adopted).toBe(true);
  });
});

describe('R1-3 擂台「2–4 家」可达', () => {
  it('presetWithRoleDefaults：roles 配置下擂台各节点目标留空，其它预设照常填充', () => {
    const arena = mkPreset('builtin-arena', [0, 1, 2].map(() => mkTarget({ role: '执行' })));
    const skipped = presetWithRoleDefaults(arena, ROLES);
    for (const target of skipped.stages[0].nodes) {
      expect(target.agent).toBe('');
      expect(target.model).toBe('');
    }
    const economy = mkPreset('builtin-economy', [mkTarget({ role: '执行' })]);
    const filled = presetWithRoleDefaults(economy, ROLES);
    expect(filled.stages[0].nodes[0].model).toBe('glm-exec');
  });

  it('擂台盖章后（roles 配置下）各节点目标为空，G1 逐路选', () => {
    const g = newGraph();
    const preset = presetWithRoleDefaults(
      mkPreset('builtin-arena', [0, 1, 2].map(() => mkTarget({ role: '执行' }))),
      ROLES,
    );
    const g2 = stamp(g, g.root, preset);
    const ids = childrenOf(g2, g.root);
    expect(ids).toHaveLength(3);
    for (const id of ids) {
      expect(g2.nodes[id].target?.agent).toBe('');
      expect(g2.nodes[id].target?.model).toBe('');
    }
  });

  it('removeDraftNode：没有子节点的 draft 节点可删', () => {
    const g = newGraph();
    const preset = mkPreset('builtin-arena', [0, 1, 2].map(() => mkTarget({ role: '执行' })));
    const g2 = stamp(g, g.root, preset);
    const ids = childrenOf(g2, g.root);
    const g3 = removeDraftNode(g2, ids[2]);
    expect(Object.keys(g3.nodes)).toHaveLength(3);
    expect(g3.nodes[ids[2]]).toBeUndefined();
    // 原图不变
    expect(g2.nodes[ids[2]]).toBeDefined();
  });

  it('removeDraftNode：有子节点的 draft 拒删', () => {
    const g = newGraph();
    const preset = mkPreset('builtin-arena', [0, 1].map(() => mkTarget({ role: '执行' })));
    let g2 = stamp(g, g.root, preset);
    const [a] = childrenOf(g2, g.root);
    g2 = stamp(g2, a, mkPreset('child-preset', [mkTarget({ role: '执行' })]));
    expect(childrenOf(g2, a)).toHaveLength(1);
    const g3 = removeDraftNode(g2, a);
    expect(g3).toBe(g2);
    expect(g3.nodes[a]).toBeDefined();
  });

  it('removeDraftNode：非 draft 节点拒删', () => {
    const g = newGraph();
    const preset = mkPreset('builtin-arena', [mkTarget({ role: '执行' })]);
    let g2 = stamp(g, g.root, preset);
    const [a] = childrenOf(g2, g.root);
    g2 = applyDispatchResults(g2, [mkResult(a)]);
    expect(g2.nodes[a].reply?.status).toBe('done');
    expect(removeDraftNode(g2, a)).toBe(g2);
    // 不存在的节点同样原样返回
    expect(removeDraftNode(g2, 'no-such')).toBe(g2);
  });
});

describe('R1-4 节点 user.text 编辑', () => {
  it('updateNodeUserText 写回节点 user.text，原图不变', () => {
    const g = newGraph();
    const preset = mkPreset('builtin-jury', [0, 1, 2].map(() => mkTarget({ role: '验证', visibility: '盲' })));
    const g2 = stamp(g, g.root, preset);
    const ids = childrenOf(g2, g.root);
    // 第三条是「自定义」空文本，面板编辑框写回
    expect(g2.nodes[ids[2]].user.text).toBe('');
    const g3 = updateNodeUserText(g2, ids[2], '你是评审团的自定义评审。');
    expect(g3.nodes[ids[2]].user.text).toBe('你是评审团的自定义评审。');
    expect(g2.nodes[ids[2]].user.text).toBe('');
    // 附件不动
    expect(g3.nodes[ids[2]].user.attachments).toEqual([]);
  });
});

describe('R1-5 前提抽查带附件', () => {
  it('buildPremiseCheckGraph：台账 attachments 进根材料 user.attachments', () => {
    const premise: Premise = {
      id: 'abcd1234-5678',
      text: '上游 API 保持 v2 兼容',
      recorded_at: '2026-09-14T10:00:00Z',
      attachments: ['docs/api-v2.md', 'fixtures/spec.json'],
    };
    const g = buildPremiseCheckGraph(premise, { agent: 'kimi', model: 'k3' });
    expect(g.nodes[g.root].user.text).toBe(premise.text);
    expect(g.nodes[g.root].user.attachments).toEqual(premise.attachments);
    // 无附件的台账条目根材料附件为空
    const bare = buildPremiseCheckGraph({ ...premise, attachments: [] });
    expect(bare.nodes[bare.root].user.attachments).toEqual([]);
  });
});

describe('R1-6 / R1-11c 备选下发与写回', () => {
  it('fallbackSpecOf：节点 target.fallback 存在时请求体带备选段', () => {
    const withFallback = mkTarget({
      agent: 'codex', model: 'gpt',
      fallback: { agent: 'pi', provider: 'deepseek', model: 'deepseek-v4-flash' },
    });
    expect(fallbackSpecOf(withFallback)).toEqual({ agent: 'pi', provider: 'deepseek', model: 'deepseek-v4-flash', vendor: '' });
    // provider 缺省补空串（与 Rust FallbackSpec.provider: String 对齐）
    const noProvider = mkTarget({ fallback: { agent: 'pi', model: 'glm' } });
    expect(fallbackSpecOf(noProvider)).toEqual({ agent: 'pi', provider: '', model: 'glm', vendor: '' });
    // 没设备选 / 备选 model 空白 → 请求不带
    expect(fallbackSpecOf(mkTarget({}))).toBeUndefined();
    expect(fallbackSpecOf(mkTarget({ fallback: { agent: 'pi', model: '  ' } }))).toBeUndefined();
  });
});

describe('R1-7 / R1-11b 仲裁中性描述脱敏', () => {
  function disputeGraph(): { g: Graph; a: string; b: string } {
    const g = newGraph();
    const a = newNodeId();
    const b = newNodeId();
    const withNodes: Graph = {
      ...g,
      nodes: {
        ...g.nodes,
        [a]: mkTurn(a, [g.root], {
          target: mkTarget({ role: '执行', agent: 'pi', model: 'glm' }),
          reply: { status: 'done', text: '方案一更快。' },
        }),
        [b]: mkTurn(b, [g.root], {
          target: mkTarget({ role: '执行', agent: 'codex', model: 'gpt' }),
          reply: { status: 'done', text: '方案二更稳。' },
        }),
      },
    };
    return { g: withNodes, a, b };
  }

  it('标签是甲方/乙方，不含双方 agent/model 名与立场词', () => {
    const { g, a, b } = disputeGraph();
    const text = neutralDisputeSummary(g, a, b);
    expect(text).toContain('【甲方】');
    expect(text).toContain('【乙方】');
    expect(text).not.toContain('pi');
    expect(text).not.toContain('glm');
    expect(text).not.toContain('codex');
    expect(text).not.toContain('gpt');
    for (const word of ['更好', '更差', '同意', '反对', '应该']) {
      expect(text).not.toContain(word);
    }
  });

  it('超长截断带省略号，优先保留含转折词的句子', () => {
    const g = newGraph();
    const a = newNodeId();
    const b = newNodeId();
    const filler = '这部分是铺垫句，没有实质分歧。'.repeat(20); // 远超 200 字
    const turning = '但内存占用会翻倍，这是关键分歧。';
    const withNodes: Graph = {
      ...g,
      nodes: {
        ...g.nodes,
        [a]: mkTurn(a, [g.root], { reply: { status: 'done', text: `${filler}${turning}` } }),
        [b]: mkTurn(b, [g.root], { reply: { status: 'done', text: '短回复。' } }),
      },
    };
    const text = neutralDisputeSummary(withNodes, a, b);
    expect(text).toContain('…');
    expect(text).toContain(turning);
    // 短回复不截断、不加省略号（全文只有甲方那处一个省略号）
    expect(text).toContain('【乙方】认为：短回复。');
    expect((text.match(/…/g) ?? [])).toHaveLength(1);
  });
});

describe('R1-7 / R1-13 appendArbitration 盲语义与 kind', () => {
  const models = [
    mkModel({ id: 'glm', agent: 'pi', vendor: '智谱' }),
    mkModel({ id: 'gpt', agent: 'codex', vendor: 'OpenAI' }),
    mkModel({ id: 'k3', agent: 'kimi', vendor: 'Moonshot AI' }),
  ];

  function sidesUnder(parents: string[], g: Graph): { g: Graph; a: string; b: string } {
    const a = newNodeId();
    const b = newNodeId();
    return {
      g: {
        ...g,
        nodes: {
          ...g.nodes,
          [a]: mkTurn(a, parents, { target: mkTarget({ role: '执行', agent: 'pi', model: 'glm' }) }),
          [b]: mkTurn(b, parents, { target: mkTarget({ role: '执行', agent: 'codex', model: 'gpt' }) }),
        },
      },
      a,
      b,
    };
  }

  it('产物 visibility=盲、只读、role=仲裁，不带 upstreams', () => {
    const base = newGraph();
    const { g, a, b } = sidesUnder([base.root], base);
    const root = g.root;
    const outcome = appendArbitration(g, a, b, '中性描述', mkTarget({ agent: 'kimi', model: 'k3' }), models);
    expect(outcome.error).toBeUndefined();
    const node = outcome.graph.nodes[outcome.nodeId!];
    expect(node.target?.visibility).toBe('盲');
    expect(node.target?.upstreams).toBeUndefined();
    expect(node.target?.write).toBe('只读');
    expect(node.target?.role).toBe('仲裁');
    expect(node.parents).toEqual([root]);
  });

  it('共同材料祖先 ≥2 时 kind=accept，1 个时 kind=turn', () => {
    const base = newGraph();
    const m2 = newNodeId();
    const withM2: Graph = {
      ...base,
      nodes: {
        ...base.nodes,
        [m2]: {
          id: m2, kind: 'material', parents: [base.root],
          user: { text: '第二份材料', attachments: [] },
          created_at: '2026-09-14T10:00:01Z',
        },
      },
    };
    const two = sidesUnder([m2], withM2);
    const acceptOutcome = appendArbitration(two.g, two.a, two.b, 't', mkTarget({ agent: 'kimi', model: 'k3' }), models);
    expect(acceptOutcome.error).toBeUndefined();
    const acceptNode = acceptOutcome.graph.nodes[acceptOutcome.nodeId!];
    expect(acceptNode.parents.length).toBeGreaterThanOrEqual(2);
    expect(acceptNode.kind).toBe('accept');

    const one = sidesUnder([base.root], base);
    const turnOutcome = appendArbitration(one.g, one.a, one.b, 't', mkTarget({ agent: 'kimi', model: 'k3' }), models);
    expect(turnOutcome.graph.nodes[turnOutcome.nodeId!].kind).toBe('turn');
  });

  it('R1-11d：双方没有共同材料祖先时拒绝建节点', () => {
    const base = newGraph();
    const island = newNodeId(); // 与根不相连的材料岛
    const withIsland: Graph = {
      ...base,
      nodes: {
        ...base.nodes,
        [island]: {
          id: island, kind: 'material', parents: [],
          user: { text: '孤岛材料', attachments: [] },
          created_at: '2026-09-14T10:00:01Z',
        },
      },
    };
    const a = newNodeId();
    const b = newNodeId();
    const g: Graph = {
      ...withIsland,
      nodes: {
        ...withIsland.nodes,
        [a]: mkTurn(a, [island], { target: mkTarget({ role: '执行', agent: 'pi', model: 'glm' }) }),
        [b]: mkTurn(b, [base.root], { target: mkTarget({ role: '执行', agent: 'codex', model: 'gpt' }) }),
      },
    };
    const outcome = appendArbitration(g, a, b, 't', mkTarget({ agent: 'kimi', model: 'k3' }), models);
    expect(outcome.nodeId).toBeUndefined();
    expect(outcome.error).toContain('没有共同的材料祖先');
  });
});

describe('R1-19b 仲裁产物过 serializeContext 只含根材料与中性描述', () => {
  it('prompt 不含双方 reply 正文', () => {
    const base = fillRoot(newGraph(), '原始需求正文');
    const a = newNodeId();
    const b = newNodeId();
    const g: Graph = {
      ...base,
      nodes: {
        ...base.nodes,
        [a]: mkTurn(a, [base.root], {
          target: mkTarget({ role: '执行', agent: 'pi', model: 'glm' }),
          reply: { status: 'done', text: '甲方的机密推导过程甲甲甲' },
        }),
        [b]: mkTurn(b, [base.root], {
          target: mkTarget({ role: '执行', agent: 'codex', model: 'gpt' }),
          reply: { status: 'done', text: '乙方的机密推导过程乙乙乙' },
        }),
      },
    };
    const models = [
      mkModel({ id: 'glm', agent: 'pi', vendor: '智谱' }),
      mkModel({ id: 'gpt', agent: 'codex', vendor: 'OpenAI' }),
      mkModel({ id: 'k3', agent: 'kimi', vendor: 'Moonshot AI' }),
    ];
    const neutral = '【甲方】认为：…；【乙方】认为：…。请独立裁决。';
    const outcome = appendArbitration(g, a, b, neutral, mkTarget({ agent: 'kimi', model: 'k3' }), models);
    expect(outcome.error).toBeUndefined();
    const { prompt } = serializeContext(outcome.graph, outcome.nodeId!);
    expect(prompt).toContain('原始需求正文');
    expect(prompt).toContain(neutral);
    expect(prompt).not.toContain('甲甲甲');
    expect(prompt).not.toContain('乙乙乙');
  });
});

describe('R1-8 / R1-15 matchPremiseResults 按 id 回写', () => {
  function mkPremise(patch: Partial<Premise>): Premise {
    return {
      id: 'p-1', text: '前提', recorded_at: '2026-09-14T10:00:00Z',
      attachments: [],
      ...patch,
    };
  }

  function checkedGraph(premise: Premise, outcome: string | null): Graph {
    const g = buildPremiseCheckGraph(premise, { agent: 'kimi', model: 'k3' });
    if (outcome === null) return g;
    const [checkId] = childrenOf(g, g.root);
    return applyDispatchResults(g, [mkResult(checkId, { text: outcome })]);
  }

  it('两次抽查同名图按 last_graph_id 各归各', async () => {
    const premise = mkPremise({ id: 'same-name', last_graph_id: 'g2' });
    const other = mkPremise({ id: 'other', last_graph_id: 'g1' });
    // 同一前提两次抽查产生两张同名图；另一前提的图标题相同前缀也不同名，这里直接按 id 给
    const g1 = checkedGraph(premise, '旧结论');
    const g2raw = checkedGraph(premise, '新结论');
    const g2: Graph = { ...g2raw, updated_at: '2020-01-01T00:00:00Z' };
    const loader = (id: string) => Promise.resolve(id === 'g1' ? g1 : id === 'g2' ? g2 : null);
    const { premises, changed } = await matchPremiseResults([premise, other], loader);
    expect(changed).toBe(true);
    expect(premises[0].last_result).toBe('新结论');
    expect(premises[1].last_result).toBe('旧结论');
    // last_checked_at 用回写时刻，不是 graph.updated_at
    expect(premises[0].last_checked_at).toBeTruthy();
    expect(premises[0].last_checked_at).not.toBe('2020-01-01T00:00:00Z');
  });

  it('结论为空不覆盖已有结果', async () => {
    const premise = mkPremise({ last_graph_id: 'g1', last_result: '已有结论' });
    const g1 = checkedGraph(premise, null); // 未跑完
    const { premises, changed } = await matchPremiseResults([premise], () => Promise.resolve(g1));
    expect(changed).toBe(false);
    expect(premises[0].last_result).toBe('已有结论');
  });

  it('结论未变不重写（按截断 500 字比较）', async () => {
    const long = '长'.repeat(600);
    const premise = mkPremise({ last_graph_id: 'g1', last_result: long.slice(0, 500), last_checked_at: '2026-09-01T00:00:00Z' });
    const g1 = checkedGraph(premise, long);
    const { premises, changed } = await matchPremiseResults([premise], () => Promise.resolve(g1));
    expect(changed).toBe(false);
    expect(premises[0]).toBe(premise);
    // 没有 last_graph_id 的旧条目不参与回写
    const legacy = mkPremise({ id: 'legacy' });
    const idle = await matchPremiseResults([legacy], () => Promise.resolve(g1));
    expect(idle.changed).toBe(false);
  });
});

describe('R1-10 角色清单与解析边界', () => {
  it('parseAdvise：/advisefoo 不识别，裸 /advise 识别，正常形态不变', () => {
    const models = [mkModel({ id: 'k3-256k', agent: 'kimi', provider: 'kimi' })];
    expect(parseAdvise('/advisefoo 你好', models)).toBeNull();
    const bare = parseAdvise('/advise', models);
    expect(bare).not.toBeNull();
    expect(bare!.question).toBe('');
    expect(bare!.target.role).toBe('顾问');
    const normal = parseAdvise('/advise @kimi/k3-256k 这个方案有什么坑', models);
    expect(normal!.question).toBe('这个方案有什么坑');
    expect(normal!.target.model).toBe('k3-256k');
  });

  it('roleDefaults：顾问与仲裁都是盲+只读', () => {
    expect(roleDefaults('顾问')).toEqual({ visibility: '盲', write: '只读' });
    expect(roleDefaults('仲裁')).toEqual({ visibility: '盲', write: '只读' });
    expect(roleDefaults('执行')).toEqual({ visibility: '共享', write: '只读' });
  });
});

describe('R1-12a 预设弱键', () => {
  it('builtin:false 且 id 撞名 builtin-jury 的用户预设不写视角文本', () => {
    const g = newGraph();
    const fake = mkPreset('builtin-jury', [0, 1, 2].map(() => mkTarget({ role: '验证', visibility: '盲' })), false);
    const g2 = stamp(g, g.root, fake);
    const ids = childrenOf(g2, g.root);
    expect(ids).toHaveLength(3);
    for (const id of ids) expect(g2.nodes[id].user.text).toBe('');
    // 真内置预设仍写（第三条为「自定义」空串）
    const real = stamp(g, g.root, mkPreset('builtin-jury', [0, 1, 2].map(() => mkTarget({ role: '验证', visibility: '盲' }))));
    const realIds = childrenOf(real, g.root);
    expect(real.nodes[realIds[0]].user.text).toBe(PRESET_NODE_TEXTS['builtin-jury'][0]);
    expect(real.nodes[realIds[0]].user.text).not.toBe('');
  });
});

describe('R1-14 「选它」按钮同父条件', () => {
  it('sameParentsResultCount 只数 parents 集合相同的结果', () => {
    const g = newGraph();
    const a = newNodeId();
    const b = newNodeId();
    const c = newNodeId(); // c 在 a 下面，与 a/b 不同父
    const withNodes: Graph = {
      ...g,
      nodes: {
        ...g.nodes,
        [a]: mkTurn(a, [g.root], { target: mkTarget({ role: '执行' }) }),
        [b]: mkTurn(b, [g.root], { target: mkTarget({ role: '执行' }) }),
        [c]: mkTurn(c, [a], { target: mkTarget({ role: '执行' }) }),
      },
    };
    const rows = [mkResult(a), mkResult(b), mkResult(c)];
    // a/b 同父 → 2；c 独占一支 → 1（不显示「选它」）
    expect(sameParentsResultCount(withNodes, rows, a)).toBe(2);
    expect(sameParentsResultCount(withNodes, rows, b)).toBe(2);
    expect(sameParentsResultCount(withNodes, rows, c)).toBe(1);
  });
});

describe('R1-19c fallbackFrom 结果过 reducePipelineStep', () => {
  it('备选重跑后的 validation_failed 照常生长修正轮', () => {
    const g = newGraph();
    const exec = newNodeId();
    const accept = newNodeId();
    const withNodes: Graph = {
      ...g,
      nodes: {
        ...g.nodes,
        [exec]: mkTurn(exec, [g.root], {
          reply: { status: 'pending', text: '' },
          target: mkTarget({ role: '执行', write: '单写者' }),
        }),
        [accept]: mkTurn(accept, [exec], {
          reply: { status: 'draft', text: '' },
          target: mkTarget({ role: '验收' }),
        }),
      },
    };
    const failed = mkResult(exec, {
      status: 'validation_failed',
      text: '',
      validationOutput: 'pytest failed',
      fallbackFrom: 'pi/glm',
      fallbackTo: 'codex/gpt',
    });
    const step = reducePipelineStep(withNodes, [failed], {});
    expect(step.continueRunning).toBe(true);
    // 长出新执行节点 + 新验收节点，原验收标失败
    expect(Object.keys(step.graph.nodes)).toHaveLength(5);
    expect(step.graph.nodes[accept].reply?.status).toBe('failed');
    const retries = Object.values(step.validationRetries);
    expect(retries).toEqual([1]);
  });
});
