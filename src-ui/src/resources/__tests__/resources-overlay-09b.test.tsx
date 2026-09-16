// TASK-09b 第 5 条「接入页同步」：接入页的 agent 卡片和左栏用同一套品牌图标
// 与 provider 分组，Claude 卡片同时显示别名和真实 ID。

import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import dispatchRustSource from '../../../../src/dispatch.rs?raw';
import manualSource from '../../../../docs/MANUAL.md?raw';
import appSource from '../../App.tsx?raw';
import { AgentCard, AgentCardModelMeta, FallbackOrderEditor, moveFallbackOrder, quotaClearVisible } from '../../components/resources/ResourcesOverlay';
import resourcesOverlaySource from '../../components/resources/ResourcesOverlay.tsx?raw';
import resourcesBridgeSource from '../mention.ts?raw';
import type { ResourceAgent, ResourceModel } from '../types';
import { QuotaResetLine } from '../../components/center/graph/DispatchBits';
import { resultStatus } from '../../components/center/graph/DispatchOverlay';

function model(partial: Partial<ResourceModel>): ResourceModel {
  return {
    id: '',
    agent: '',
    provider: '',
    vendor: '',
    billing_source: '',
    tier: '',
    thinking: '',
    status: '',
    verification: '',
    resolved_id: '',
    resolved_source: '',
    source: '读:fixture',
    smoked_at: '',
    smoke_model: '',
    smoke_failure: '',
    meta: '',
    note: '',
    login: 'logged_in',
    ...partial,
  };
}

function agent(id: string, name: string, models: ResourceModel[]): ResourceAgent {
  return { id, name, kind: '内置', detected: true, version: '1.0.0', models };
}

const noop = async () => {};

function card(value: ResourceAgent) {
  return renderToStaticMarkup(
    <AgentCard
      agent={value}
      busy={null}
      note=""
      onDetect={noop}
      onVerifyAgent={noop}
      onVerifyModel={noop}
      onUpdate={noop}
      onRemove={noop}
      onManual={() => {}}
    />,
  );
}

describe('AgentCard（接入页）', () => {
  it('卡片用与左栏同一套品牌标志', () => {
    const markup = card(agent('claude', 'Claude Code', [model({ agent: 'claude', id: 'fable' })]));
    expect(markup).toContain('agent-badge-md agent-badge-claude agent-badge-image');
    expect(markup).toContain('src="/icons/tools/claude.svg"');
  });

  it('Claude 卡片主行只显示别名，真实 ID 进入悬停与详情', () => {
    const markup = card(
      agent('claude', 'Claude Code', [
        model({ agent: 'claude', id: 'fable', resolved_id: 'claude-fable-5-1' }),
        model({ agent: 'claude', id: 'claude-opus-5', resolved_id: 'claude-opus-5' }),
      ]),
    );
    expect(markup).toContain('>fable<');
    expect(markup).toContain('模型 ID：claude-fable-5-1');
    expect(markup).not.toContain('agent-card-model-resolved');
  });

  it('多 provider 的卡片按 provider 分组，组头带计数；单 provider 不出组头', () => {
    const many = card(
      agent('pi', 'pi', [
        model({ agent: 'pi', provider: 'zai', id: 'glm-4.7' }),
        model({ agent: 'pi', provider: 'zai', id: 'glm-4.6' }),
        model({ agent: 'pi', provider: 'deepseek', id: 'deepseek-v4' }),
      ]),
    );
    expect(many).toContain('<span class="agent-card-group-name">deepseek</span>');
    expect(many).toContain('<span class="agent-card-group-name">zai</span>');
    expect(many).toContain('<span class="agent-card-group-count">2</span>');

    const single = card(agent('claude', 'Claude Code', [model({ agent: 'claude', id: 'fable' })]));
    expect(single).not.toContain('agent-card-group-name');

    // 修正轮 R1（DS 3）：单 provider 的 pi 也不出组头
    const onePi = card(
      agent('pi', 'pi', [
        model({ agent: 'pi', provider: 'bigmodel', id: 'glm-4.6' }),
        model({ agent: 'pi', provider: 'bigmodel', id: 'glm-4.5' }),
      ]),
    );
    expect(onePi).not.toContain('agent-card-group-name');
  });
});

// 修正轮 R1（DS 3）：单 provider 不出组头，provider 名不能跟着消失——
// 详情块里单列一行。详情默认收着，所以直接渲染这一段。
describe('AgentCardModelMeta（接入页详情）', () => {
  it('provider 单列一行，原生 agent（provider 为空）不出这一行', () => {
    const third = renderToStaticMarkup(
      <AgentCardModelMeta model={model({ agent: 'pi', provider: 'bigmodel', id: 'glm-4.6' })} />,
    );
    expect(third).toContain('provider：bigmodel');
    expect(third).toContain('读:fixture');

    const native = renderToStaticMarkup(
      <AgentCardModelMeta model={model({ agent: 'claude', id: 'fable' })} />,
    );
    expect(native).not.toContain('provider：');
  });

  it('来源、最近验证、失败原因与备注仍在', () => {
    const markup = renderToStaticMarkup(
      <AgentCardModelMeta
        model={model({
          agent: 'pi',
          provider: 'zai',
          id: 'glm-4.5',
          smoked_at: '2026-09-15T10:02:00+08:00',
          smoke_model: 'zai/glm-4.5',
          smoke_failure: '401: invalid api key',
          note: '手动加的',
        })}
      />,
    );
    expect(markup).toContain('最近验证：2026-09-15T10:02:00+08:00（zai/glm-4.5）');
    expect(markup).toContain('验证失败：401: invalid api key');
    expect(markup).toContain('备注：手动加的');
  });
});

describe('TASK-14 备选顺序组件', () => {
  it('按保存顺序渲染，并提供上移、下移、移除', () => {
    const primary = model({
      agent: 'pi', provider: 'p0', id: 'main',
      fallback_order: ['codex//gpt', 'pi/p1/glm'],
    });
    const markup = renderToStaticMarkup(
      <FallbackOrderEditor
        model={primary}
        models={[
          primary,
          model({ agent: 'codex', id: 'gpt' }),
          model({ agent: 'pi', provider: 'p1', id: 'glm' }),
          model({ agent: 'kimi', id: 'k2' }),
        ]}
        onChange={() => {}}
      />,
    );
    expect(markup.indexOf('1. codex/gpt')).toBeLessThan(markup.indexOf('2. pi/p1/glm'));
    expect(markup).toContain('>上移</button>');
    expect(markup).toContain('>下移</button>');
    expect(markup).toContain('>移除</button>');
    expect(markup).toContain('value="kimi//k2"');
  });

  it('移动只交换相邻项，不修改输入数组', () => {
    const order = ['a//1', 'b//2', 'c//3'];
    expect(moveFallbackOrder(order, 2, -1)).toEqual(['a//1', 'c//3', 'b//2']);
    expect(order).toEqual(['a//1', 'b//2', 'c//3']);
  });
});

describe('TASK-14 额度结果组件', () => {
  it('显示独立状态与恢复时间', () => {
    expect(resultStatus('quota_exhausted')).toBe('额度用尽');
    const markup = renderToStaticMarkup(<QuotaResetLine resetsAt="2099-01-01T00:00:00+08:00" />);
    expect(markup).toContain('恢复时间：');
    expect(renderToStaticMarkup(<QuotaResetLine />)).toBe('');
  });

  it('Rust 写回额度后通过单一桥接通知现有资源监听方', () => {
    expect(dispatchRustSource).toContain('app.emit("aicross:resources-changed", ())');
    expect(resourcesBridgeSource).toContain('listen(RESOURCES_CHANGED_EVENT, notifyResourcesChanged)');
    expect(appSource).toContain('useEffect(() => subscribeResourcesChanged(), [])');
  });

  it('受限模型提供清除动作，普通模型不显示', () => {
    const limited = model({
      agent: 'pi', provider: 'zai', id: 'glm', quota_exhausted_until: '2099-01-01T00:00:00+08:00',
    });
    expect(quotaClearVisible(limited)).toBe(true);
    expect(quotaClearVisible(model({ agent: 'pi', provider: 'zai', id: 'glm' }))).toBe(false);
    expect(resourcesOverlaySource).toMatch(/quotaClearVisible\(model\)[\s\S]*?>\s*清除\s*<\/button>/);
    expect(resourcesOverlaySource).toContain('clear_quota_exhausted: true');
  });

  it('TASK-14 R1 统一使用恢复时间术语', () => {
    expect(manualSource).toContain('| 模型额度重新可用的时间 | 恢复时间 |');
    expect(manualSource).not.toContain('重置时间');
  });
});
