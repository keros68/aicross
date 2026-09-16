// TASK-19：四家 agent 的资源行共用同一 DOM 形态，字号与行高同步上调。

// @ts-expect-error node builtin, only CSS assertions read a local file
import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { AgentCard, AgentCardModelMeta } from '../../components/resources/ResourcesOverlay';
import { ResourceAgentList } from '../../components/resources/ResourcesTab';
import type { ResourceAgent, ResourceModel } from '../types';

function model(partial: Partial<ResourceModel>): ResourceModel {
  return {
    id: '', agent: '', provider: '', vendor: '', billing_source: '', tier: '', thinking: '',
    status: '', verification: 'native', resolved_id: '', resolved_source: '', source: '',
    smoked_at: '', smoke_model: '', smoke_failure: '', meta: '', note: '', ...partial,
  };
}

function agent(id: string, item: ResourceModel): ResourceAgent {
  return { id, name: id, kind: '内置', detected: true, version: '1.0.0', models: [item] };
}

const agents = [
  agent('claude', model({ agent: 'claude', id: 'sonnet', resolved_id: 'claude-sonnet-5' })),
  agent('codex', model({ agent: 'codex', id: 'gpt-5.6', resolved_id: 'gpt-5.6' })),
  agent('kimi', model({ agent: 'kimi', id: 'kimi-k2', resolved_id: 'kimi-k2' })),
  agent('pi', model({ agent: 'pi', provider: 'zai', id: 'glm-5', resolved_id: 'glm-5', verification: 'verified', login: 'logged_in' })),
];

describe('TASK-19 左栏资源行', () => {
  it('四家模型行只有名称槽与状态槽，HTML 结构一致且没有第二列', () => {
    const markup = renderToStaticMarkup(
      <ResourceAgentList
        agents={agents}
        expanded={agents.map((item) => item.id)}
        openUnverified={[]}
        graphOpen
        onToggle={() => {}}
        onToggleUnverified={() => {}}
        onPick={() => {}}
      />,
    );
    const rows = [...markup.matchAll(/<button class="resource-model-row"[\s\S]*?<\/button>/g)].map((match) => match[0]);
    expect(rows).toHaveLength(4);
    for (const row of rows) {
      expect(row).toMatch(/<span class="resource-model-name">[^<]+<\/span><span class="resource-model-status">[\s\S]*<\/span><\/button>$/);
      const visibleChildren = row.slice(row.indexOf('>') + 1);
      expect(visibleChildren).not.toMatch(/已登录|未登录|未检查/);
    }
  });

  it('接入页模型主行只显示名称和状态点，真实 ID 与登录检查进入详情', () => {
    const claude = agents[0];
    const card = renderToStaticMarkup(
      <AgentCard
        agent={claude}
        busy={null}
        note=""
        onDetect={async () => {}}
        onVerifyAgent={async () => {}}
        onVerifyModel={async () => {}}
        onUpdate={async () => {}}
        onRemove={async () => {}}
        onManual={() => {}}
      />,
    );
    const main = /<div class="agent-card-model-main"[\s\S]*?<\/div>/.exec(card)?.[0] ?? '';
    expect(main).toContain('class="agent-card-model-name"');
    expect(main).toContain('class="agent-card-model-state"');
    expect(main).not.toContain('agent-card-model-resolved');
    expect(main).not.toContain('class="verify-text"');

    const detail = renderToStaticMarkup(
      <AgentCardModelMeta model={model({
        agent: 'pi', provider: 'zai', id: 'alias', resolved_id: 'glm-5', login: 'logged_in',
        login_checked_at: '2026-09-16T08:00:00+08:00',
      })} />,
    );
    expect(detail).toContain('模型 ID：glm-5');
    expect(detail).toContain('provider：zai');
    expect(detail).toContain('登录状态：已登录');
    expect(detail).toContain('最近登录检查：2026-09-16T08:00:00+08:00');
  });

  it('接入页详情显示验证状态与未来额度恢复时间', () => {
    const detail = renderToStaticMarkup(
      <AgentCardModelMeta model={model({
        agent: 'pi', provider: 'zai', id: 'glm-5', verification: 'verified',
        quota_exhausted_until: '2099-01-01T15:45:00+08:00',
      })} />,
    );
    expect(detail).toContain('状态：已验证');
    expect(detail).toContain('额度用尽至 15:45');
  });

  it('关闭对话图时悬停提示仍保留模型详情', () => {
    const markup = renderToStaticMarkup(
      <ResourceAgentList
        agents={[agents[0]]}
        expanded={['claude']}
        openUnverified={[]}
        graphOpen={false}
        onToggle={() => {}}
        onToggleUnverified={() => {}}
        onPick={() => {}}
      />,
    );
    expect(markup).toContain('先打开对话图\nsonnet');
    expect(markup).toContain('模型 ID：claude-sonnet-5');
    expect(markup).toContain('状态：原生');
  });

  it('模型行是 14px / 30px，agent 名是 14px，版本是 12px', () => {
    const css = readFileSync(new URL('../../components/resources/resources.css', import.meta.url), 'utf8');
    const row = /\.resource-model-row\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
    const name = /\.resource-model-name\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
    const agentName = /\.resource-agent-name\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
    const version = /\.resource-agent-version\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
    expect(row).toContain('height: 30px');
    expect(name).toContain('font-size: 14px');
    expect(agentName).toContain('font-size: 14px');
    expect(version).toContain('font-size: 12px');
  });

  it('左栏与接入页未验证折叠按钮共用样式和键盘焦点样式', () => {
    const css = readFileSync(new URL('../../components/resources/resources.css', import.meta.url), 'utf8');
    expect(css).toMatch(/\.resource-unverified-toggle,\s*\.agent-card-unverified-toggle\s*\{/);
    expect(css).toMatch(/\.resource-unverified-toggle:focus-visible,\s*\.agent-card-unverified-toggle:focus-visible\s*\{/);
  });
});
