// TASK-05B 第 2 期预设：图章纯函数 + G3 动作 + Target.fallback 往返 + 前提抽查图。

import { describe, expect, it } from 'vitest';
import { PRESET_NODE_TEXTS, childrenOf, stamp } from '../stamp.ts';
import {
  applyDispatchResults,
  appendArbitration,
  fallbackNoticeOf,
  neutralDisputeSummary,
  parseAdvise,
  pickOne,
  routeLine,
  setReplyAdopted,
  type BackendNodeResult,
} from '../dispatch.ts';
import { arbitrationVendorConflict, validateNode } from '../validate.ts';
import { cloneTarget, newGraph, newNodeId } from '../types.ts';
import type { DispatchableModel, Graph, GraphNode, Preset, Target } from '../types.ts';
import { buildPremiseCheckGraph, premiseCheckOutcome, premiseGraphTitle, PREMISE_CHECK_PROMPT } from '../premise.ts';

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

function mkPreset(id: string, nodes: Target[]): Preset {
  return { id, name: id, builtin: true, stages: [{ nodes }] };
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

describe('TASK-05B 内置预设图章', () => {
  it('擂台：同父 3 个共享执行 turn 节点', () => {
    const g = newGraph();
    const preset = mkPreset('builtin-arena', [0, 1, 2].map(() => mkTarget({ role: '执行' })));
    const g2 = stamp(g, g.root, preset);
    const ids = childrenOf(g2, g.root);
    expect(ids).toHaveLength(3);
    for (const id of ids) {
      const node = g2.nodes[id];
      expect(node.kind).toBe('turn');
      expect(node.parents).toEqual([g.root]);
      expect(node.target?.visibility).toBe('共享');
      expect(node.target?.write).toBe('只读');
      expect(node.reply?.status).toBe('draft');
    }
  });

  it('评审团：3 个盲验证节点，视角文本按序写进 user.text（第三条留空 = 自定义）', () => {
    const g = newGraph();
    const preset = mkPreset('builtin-jury', [0, 1, 2].map(() => mkTarget({ role: '验证', visibility: '盲' })));
    const g2 = stamp(g, g.root, preset);
    const ids = childrenOf(g2, g.root);
    expect(ids).toHaveLength(3);
    const texts = PRESET_NODE_TEXTS['builtin-jury'];
    ids.forEach((id, index) => {
      const node = g2.nodes[id];
      expect(node.parents).toEqual([g.root]);
      expect(node.target?.visibility).toBe('盲');
      expect(node.target?.write).toBe('只读');
      expect(node.target?.role).toBe('验证');
      expect(node.user.text).toBe(texts[index]);
    });
    expect(texts.join('')).toContain('安全');
    expect(texts.join('')).toContain('性能');
    // R1-4：第三条留空，由节点面板编辑框填「自定义视角」
    expect(texts[2]).toBe('');
  });
});

describe('TASK-05B G3 动作', () => {
  function arenaGraph(): { g: Graph; a: string; b: string } {
    const g = newGraph();
    const preset = mkPreset('builtin-arena', [
      mkTarget({ role: '执行', agent: 'pi', model: 'glm' }),
      mkTarget({ role: '执行', agent: 'codex', model: 'gpt' }),
    ]);
    const g2 = stamp(g, g.root, preset);
    const [a, b] = childrenOf(g2, g.root);
    const done = applyDispatchResults(g2, [a, b].map((nodeId) => ({
      nodeId, status: 'ok', text: `${nodeId} 正文`, modelReported: '', sessionId: '',
      tokensIn: 1, tokensOut: 1, durationMs: 1, error: '',
    })));
    return { g: done, a, b };
  }

  it('选它：被选节点 adopted=true，同父的其它候选清掉 adopted', () => {
    const { g, a, b } = arenaGraph();
    let next = pickOne(g, a);
    expect(next.nodes[a].reply?.adopted).toBe(true);
    next = setReplyAdopted(next, b, true);
    expect(next.nodes[b].reply?.adopted).toBe(true);
    // 改选另一家：旧的被清掉
    next = pickOne(next, b);
    expect(next.nodes[b].reply?.adopted).toBe(true);
    expect(next.nodes[a].reply?.adopted).toBe(false);
    // 原图不被修改
    expect(g.nodes[a].reply?.adopted).toBeUndefined();
  });

  it('仲裁建节点：parents=共同材料祖先，盲+只读+role=仲裁', () => {
    const { g, a, b } = arenaGraph();
    const models = [
      mkModel({ id: 'glm', agent: 'pi', vendor: '智谱' }),
      mkModel({ id: 'gpt', agent: 'codex', vendor: 'OpenAI' }),
      mkModel({ id: 'k3', agent: 'kimi', vendor: 'Moonshot AI' }),
    ];
    const text = neutralDisputeSummary(g, a, b);
    expect(text).toContain('认为');
    expect(text).toContain('甲方');
    expect(text).toContain('乙方');
    const outcome = appendArbitration(
      g, a, b, text,
      mkTarget({ agent: 'kimi', model: 'k3' }),
      models,
    );
    expect(outcome.error).toBeUndefined();
    const node = outcome.graph.nodes[outcome.nodeId!];
    expect(node.parents).toEqual([g.root]);
    expect(node.kind).toBe('turn');
    expect(node.user.text).toBe(text);
    expect(node.target?.role).toBe('仲裁');
    expect(node.target?.visibility).toBe('盲');
    expect(node.target?.write).toBe('只读');
    expect(node.reply?.status).toBe('draft');
  });

  it('仲裁拦同厂商：目标与任一方同厂商时拒绝建节点', () => {
    const { g, a, b } = arenaGraph();
    const models = [
      mkModel({ id: 'glm', agent: 'pi', vendor: '智谱' }),
      mkModel({ id: 'gpt', agent: 'codex', vendor: 'OpenAI' }),
      mkModel({ id: 'glm-x', agent: 'pi', vendor: '智谱' }),
      mkModel({ id: 'k3', agent: 'kimi', vendor: 'Moonshot AI' }),
    ];
    const conflict = mkTarget({ agent: 'pi', model: 'glm-x' });
    expect(arbitrationVendorConflict(g, a, b, conflict, models)).toBe(true);
    const outcome = appendArbitration(g, a, b, 't', conflict, models);
    expect(outcome.nodeId).toBeUndefined();
    expect(outcome.error).toContain('不同厂商');
    // 第三家厂商放行（k3 在 models 里，findModel 找得到，真走 conflict 判定路径）
    const third = mkTarget({ agent: 'kimi', model: 'k3' });
    expect(arbitrationVendorConflict(g, a, b, third, models)).toBe(false);
    const ok = appendArbitration(g, a, b, 't', third, models);
    expect(ok.error).toBeUndefined();
  });

  it('备选重跑记账：reply 正文带「已切备选」标记行，fallbackNoticeOf 能取出', () => {
    const { g, a } = arenaGraph();
    const result: BackendNodeResult = {
      nodeId: a, status: 'ok', text: '备选正文', modelReported: '', sessionId: '',
      tokensIn: 1, tokensOut: 1, durationMs: 1, error: '',
      fallbackFrom: 'pi/glm-5.3-flash', fallbackTo: 'codex/gpt-5.6-luna',
    };
    const next = applyDispatchResults(g, [result]);
    const reply = next.nodes[a].reply!;
    expect(reply.status).toBe('done');
    expect(reply.text.startsWith('（已切备选：主 pi/glm-5.3-flash → 备 codex/gpt-5.6-luna）')).toBe(true);
    expect(reply.text).toContain('备选正文');
    // R1-9：fallbackNoticeOf 取整行，节点卡直接渲染主/备路由
    expect(fallbackNoticeOf(reply)).toBe('（已切备选：主 pi/glm-5.3-flash → 备 codex/gpt-5.6-luna）');
    // 没有备选字段的结果不带标记
    const plain = applyDispatchResults(g, [{ ...result, fallbackFrom: undefined, fallbackTo: undefined }]);
    expect(fallbackNoticeOf(plain.nodes[a].reply)).toBeNull();
  });
});

describe('TASK-05B Target.fallback 与解析', () => {
  it('fallback 的 TS 往返：JSON 序列化/还原与 cloneTarget 深拷贝', () => {
    const target = mkTarget({
      agent: 'codex',
      model: 'gpt-5.6-luna',
      fallback: { agent: 'pi', provider: 'deepseek', model: 'deepseek-v4-flash' },
    });
    // JSON 往返（与 Rust orchestration.rs::Target 的 serde 形状一致）
    const restored = JSON.parse(JSON.stringify(target)) as Target;
    expect(restored).toEqual(target);
    expect(restored.fallback?.provider).toBe('deepseek');
    // cloneTarget 深拷贝：改副本不影响原值
    const cloned = cloneTarget(target);
    cloned.fallback!.model = 'changed';
    expect(target.fallback?.model).toBe('deepseek-v4-flash');
  });

  it('routeLine 的 G1 决策行显示备选', () => {
    const node = mkTurn('n', ['root'], {
      target: mkTarget({
        agent: 'codex', model: 'gpt-5.6-luna',
        fallback: { agent: 'pi', model: 'deepseek-v4-flash' },
      }),
    });
    expect(routeLine(node, [], '全')).toContain('备选=pi/deepseek-v4-flash');
    const plain = mkTurn('n', ['root'], { target: mkTarget({ agent: 'codex', model: 'gpt' }) });
    expect(routeLine(plain, [], '全')).not.toContain('备选=');
  });

  it('parseAdvise：/advise @agent/model 问题 → 顾问、盲、只读', () => {
    const models = [mkModel({ id: 'k3-256k', agent: 'kimi', provider: 'kimi' })];
    const parsed = parseAdvise('/advise @kimi/k3-256k 这个方案有什么坑', models);
    expect(parsed).not.toBeNull();
    expect(parsed!.question).toBe('这个方案有什么坑');
    expect(parsed!.target.role).toBe('顾问');
    expect(parsed!.target.visibility).toBe('盲');
    expect(parsed!.target.write).toBe('只读');
    expect(parsed!.target.agent).toBe('kimi');
    expect(parsed!.target.model).toBe('k3-256k');
    // 非 /advise 输入不拦
    expect(parseAdvise('@kimi/k3-256k 干活', models)).toBeNull();
    // 不带 @ 目标留空，G1 确认屏再选
    const blank = parseAdvise('/advise 怎么看', models);
    expect(blank!.target.agent).toBe('');
  });

  it('validateNode：仲裁节点只能只读', () => {
    const g = newGraph();
    const nodeId = newNodeId();
    const node = mkTurn(nodeId, [g.root], {
      target: mkTarget({ role: '仲裁', write: '单写者', visibility: '指定上游', upstreams: [g.root] }),
    });
    const g2: Graph = { ...g, nodes: { ...g.nodes, [nodeId]: node } };
    expect(validateNode(g2, nodeId, [])).toContain('仲裁节点只能只读');
    const ok: Graph = {
      ...g2,
      nodes: { ...g2.nodes, [nodeId]: { ...node, target: { ...node.target!, write: '只读' } } },
    };
    expect(validateNode(ok, nodeId, [])).not.toContain('仲裁节点只能只读');
  });
});

describe('TASK-05B 前提抽查图', () => {
  it('根材料=前提原文，一个盲验证节点带冻结模板，结论可回读', () => {
    const premise = { id: 'abcd1234-5678', text: '上游 API 保持 v2 兼容' };
    const g = buildPremiseCheckGraph(premise, { agent: 'kimi', model: 'k3' });
    expect(g.title).toBe(premiseGraphTitle(premise));
    expect(g.title).toContain('前提抽查 #abcd1234');
    expect(g.nodes[g.root].user.text).toBe(premise.text);
    const ids = childrenOf(g, g.root);
    expect(ids).toHaveLength(1);
    const check = g.nodes[ids[0]];
    expect(check.user.text).toBe(PREMISE_CHECK_PROMPT);
    expect(check.target?.role).toBe('验证');
    expect(check.target?.visibility).toBe('盲');
    expect(check.target?.write).toBe('只读');
    expect(check.target?.agent).toBe('kimi');
    // 未跑完 → null；跑完 → 回读正文
    expect(premiseCheckOutcome(g)).toBeNull();
    const done = applyDispatchResults(g, [{
      nodeId: ids[0], status: 'ok', text: '结论：仍成立', modelReported: '', sessionId: '',
      tokensIn: 1, tokensOut: 1, durationMs: 1, error: '',
    }]);
    expect(premiseCheckOutcome(done)).toBe('结论：仍成立');
  });
});
