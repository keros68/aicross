// TASK-05B 修正轮 R2：fix-05b-r2.md 各条目的 TS 侧测试（编号与任务书一致）。

import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import {
  continuationSession,
  fallbackSpecOf,
  sameParentsResultCount,
  type BackendNodeResult,
} from '../dispatch.ts';
import { updateNodeFallback } from '../mutate.ts';
import { validatePremiseAttachments } from '../premise.ts';
import { newGraph, newNodeId } from '../types.ts';
import type { DispatchableModel, Graph, GraphNode, Target } from '../types.ts';
import {
  FallbackSkippedLine,
  PickOneButton,
  WritableFallbackHint,
} from '../../components/center/graph/DispatchBits.tsx';

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

describe('R2-1 备选作答的 session 不续给主通道', () => {
  function continuationGraph(parentText: string): { g: Graph; child: string } {
    const g = newGraph();
    const parent = newNodeId();
    const child = newNodeId();
    const target = mkTarget({ agent: 'pi', model: 'glm' });
    const withNodes: Graph = {
      ...g,
      nodes: {
        ...g.nodes,
        [parent]: mkTurn(parent, [g.root], {
          target,
          reply: { status: 'done', text: parentText, session_id: 'sess-123' },
        }),
        // 子节点与父同目标、共享可见性：普通回复本可续 session
        [child]: mkTurn(child, [parent], { target: mkTarget({ agent: 'pi', model: 'glm' }), reply: { status: 'draft', text: '' } }),
      },
    };
    return { g: withNodes, child };
  }

  it('父回复带「已切备选」标记时 continuationSession 返回空（下游发完整 prompt）', () => {
    const { g, child } = continuationGraph('（已切备选：主 pi/glm → 备 codex/gpt）\n\n正文');
    expect(g.nodes[g.nodes[child].parents[0]].reply?.session_id).toBe('sess-123');
    expect(continuationSession(g, child)).toBeUndefined();
  });

  it('父回复是普通作答时照常续 session', () => {
    const { g, child } = continuationGraph('正文');
    expect(continuationSession(g, child)).toBe('sess-123');
  });
});

describe('R2-2 前提附件保存前校验', () => {
  it('相对路径拒绝并提示', () => {
    expect(validatePremiseAttachments(['docs/api.md'])).toBe('附件需用绝对路径：docs/api.md');
    expect(validatePremiseAttachments(['D:/abs/ok.md', './rel.txt'])).toBe('附件需用绝对路径：./rel.txt');
  });

  it('同名 basename 提示「附件文件名重复」', () => {
    expect(validatePremiseAttachments(['D:/a/spec.json', 'C:/b/spec.json']))
      .toBe('附件文件名重复：spec.json');
  });

  it('绝对路径且不重名时通过（盘符 / UNC / POSIX 都算绝对）', () => {
    expect(validatePremiseAttachments(['D:/work/a.md', 'C:\\work\\b.md'])).toBeNull();
    expect(validatePremiseAttachments(['\\\\nas\\share\\a.md'])).toBeNull();
    expect(validatePremiseAttachments(['/home/u/a.md'])).toBeNull();
    expect(validatePremiseAttachments([])).toBeNull();
  });
});

describe('R2-4 备选下发带真实 vendor', () => {
  const models = [
    mkModel({ id: 'k3-256k', agent: 'kimi', provider: 'kimi', vendor: 'Moonshot AI' }),
    mkModel({ id: 'k3-256k', agent: 'kimi', provider: 'other', vendor: '其他渠道' }),
  ];

  it('fallbackSpecOf 用 models 里的真实 vendor（provider 精确匹配优先）', () => {
    const target = mkTarget({ fallback: { agent: 'kimi', provider: 'kimi', model: 'k3-256k' } });
    expect(fallbackSpecOf(target, models)).toEqual({
      agent: 'kimi', provider: 'kimi', model: 'k3-256k', vendor: 'Moonshot AI',
    });
  });

  it('models 查不到时 vendor 置空，由通道层推断兜底', () => {
    const target = mkTarget({ fallback: { agent: 'pi', model: 'glm-unknown' } });
    expect(fallbackSpecOf(target, models)).toEqual({
      agent: 'pi', provider: '', model: 'glm-unknown', vendor: '',
    });
  });
});

describe('R2-5 DispatchOverlay 三处组件级断言', () => {
  it('WritableFallbackHint：可写节点配备选渲染提示，只读不渲染', () => {
    const writable = mkTarget({ write: '单写者', fallback: { agent: 'pi', model: 'glm' } });
    const html = renderToStaticMarkup(<WritableFallbackHint target={writable} />);
    expect(html).toContain('额度用尽时会先回滚本次改动，再切备选。');
    expect(html).toContain('dispatch-warning');
    const readonly = mkTarget({ write: '只读', fallback: { agent: 'pi', model: 'glm' } });
    expect(renderToStaticMarkup(<WritableFallbackHint target={readonly} />)).toBe('');
  });

  it('FallbackSkippedLine：fallbackSkipped 非空渲染「主失败，未切备选」结果行', () => {
    const html = renderToStaticMarkup(<FallbackSkippedLine result={mkResult('n1', { fallbackSkipped: '可写节点' })} />);
    expect(html).toContain('主失败，未切备选（可写节点）');
    expect(renderToStaticMarkup(<FallbackSkippedLine result={mkResult('n1')} />)).toBe('');
  });

  it('PickOneButton：同父同 kind 同角色结果 ≥2 渲染「选它」，否则不渲染', () => {
    const g = newGraph();
    const a = newNodeId();
    const b = newNodeId();
    const withNodes: Graph = {
      ...g,
      nodes: {
        ...g.nodes,
        [a]: mkTurn(a, [g.root], { target: mkTarget({ role: '执行' }) }),
        [b]: mkTurn(b, [g.root], { target: mkTarget({ role: '执行' }) }),
      },
    };
    const rows = [mkResult(a), mkResult(b)];
    const visible = sameParentsResultCount(withNodes, rows, a) >= 2;
    expect(visible).toBe(true);
    const html = renderToStaticMarkup(<PickOneButton visible={visible} adopted={false} onPick={() => {}} />);
    expect(html).toContain('选它');
    expect(renderToStaticMarkup(<PickOneButton visible={false} adopted={false} onPick={() => {}} />)).toBe('');
    // 已采纳时按钮禁用且文案变「已选中」
    const adoptedHtml = renderToStaticMarkup(<PickOneButton visible adopted onPick={() => {}} />);
    expect(adoptedHtml).toContain('已选中');
    expect(adoptedHtml).toContain('disabled');
  });
});

describe('R2-10 「选它」可见性按 kind/role 收窄', () => {
  it('同父 1 擂台 + 1 顾问不出「选它」（与 pickOne 口径对齐）', () => {
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
    const rows = [mkResult(arena), mkResult(advisor)];
    // 同父但角色不同：顾问不算擂台对手，count=1 → 不显示「选它」
    expect(sameParentsResultCount(withNodes, rows, arena)).toBe(1);
    expect(sameParentsResultCount(withNodes, rows, advisor)).toBe(1);
  });
});

describe('R2-12 G1 备选 onChange 只写 fallback', () => {
  it('updateNodeFallback 不动图上的主目标（本地未确认的修改不落盘）', () => {
    const g = newGraph();
    const n = newNodeId();
    const withNodes: Graph = {
      ...g,
      nodes: {
        ...g.nodes,
        [n]: mkTurn(n, [g.root], {
          target: mkTarget({ agent: 'codex', model: 'gpt' }),
          reply: { status: 'draft', text: '' },
        }),
      },
    };
    const fallback = { agent: 'pi', provider: 'deepseek', model: 'deepseek-v4-flash' };
    const next = updateNodeFallback(withNodes, n, fallback);
    // 只写备选：主目标保持图上的 codex/gpt，不是本地改过的值
    expect(next.nodes[n].target?.agent).toBe('codex');
    expect(next.nodes[n].target?.model).toBe('gpt');
    expect(next.nodes[n].target?.fallback).toEqual(fallback);
    // 清空备选
    const cleared = updateNodeFallback(next, n, undefined);
    expect(cleared.nodes[n].target?.fallback).toBeUndefined();
    expect(cleared.nodes[n].target?.agent).toBe('codex');
  });
});
