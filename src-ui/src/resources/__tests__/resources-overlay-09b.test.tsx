// TASK-09b 第 5 条「接入页同步」：接入页的 agent 卡片和左栏用同一套品牌图标
// 与 provider 分组，Claude 卡片同时显示别名和真实 ID。

import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { AgentCard } from '../../components/resources/ResourcesOverlay';
import { BRAND_MARKS } from '../../components/resources/brand-marks';
import type { ResourceAgent, ResourceModel } from '../types';

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
    source: '读:fixture',
    smoked_at: '',
    smoke_model: '',
    smoke_failure: '',
    meta: '',
    note: '',
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
    expect(markup).toContain('agent-badge-md agent-badge-claude agent-badge-mark');
    expect(markup).toContain(BRAND_MARKS.claude.path);
  });

  it('Claude 卡片显示别名 + 真实 ID；两者相同的模型只显示一次', () => {
    const markup = card(
      agent('claude', 'Claude Code', [
        model({ agent: 'claude', id: 'fable', resolved_id: 'claude-fable-5-1' }),
        model({ agent: 'claude', id: 'claude-opus-5', resolved_id: 'claude-opus-5' }),
      ]),
    );
    expect(markup).toContain('>fable<');
    expect(markup).toContain('class="agent-card-model-resolved" title="claude-fable-5-1"');
    // 别名与 ID 相同的那条不长出次文本
    expect(markup).not.toContain('class="agent-card-model-resolved" title="claude-opus-5"');
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
  });
});
