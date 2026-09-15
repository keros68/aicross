// TASK-17：pi provider 登录状态只影响展示与 G1 默认勾选，不读取真实凭据。

// @ts-expect-error node builtin, only CSS assertions read a local file
import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { ResourceAgentList } from '../../components/resources/ResourcesTab';
import { FallbackOrderEditor } from '../../components/resources/ResourcesOverlay';
import { blockedNodePartitions, initialBlockedSelection, initialResourceBlockedSelection } from '../../components/center/graph/DispatchOverlay';
import { loggedOutNodeIds } from '../../graph/dispatch';
import type { Graph } from '../../graph/types';
import {
  loggedOutProvidersToggleLabel,
  loginLabel,
  loginOf,
  splitPiProviderGroups,
  type ResourceAgent,
  type ResourceModel,
} from '../types';

function model(partial: Partial<ResourceModel>): ResourceModel {
  return {
    id: '', agent: 'pi', provider: '', vendor: '', billing_source: '', tier: '', thinking: '',
    status: '', verification: '', resolved_id: '', resolved_source: '', source: '', smoked_at: '',
    smoke_model: '', smoke_failure: '', meta: '', note: '', ...partial,
  };
}

describe('TASK-17 pi provider 分组', () => {
  it('只有已登录正常列，未登录与未检查 provider 都合成一行且可展开', () => {
    const groups = splitPiProviderGroups([
      model({ provider: 'ready', id: 'a', login: 'logged_in' }),
      model({ provider: 'unknown', id: 'b' }),
      model({ provider: 'out-a', id: 'c', login: 'logged_out' }),
      model({ provider: 'out-a', id: 'd', login: 'logged_out' }),
      model({ provider: 'out-b', id: 'e', login: 'logged_out' }),
    ]);
    expect(groups.shown.map((group) => group.provider)).toEqual(['ready']);
    expect(groups.loggedOut.map((group) => group.provider)).toEqual(['out-a', 'out-b']);
    expect(groups.unknown.map((group) => group.provider)).toEqual(['unknown']);
    expect(loggedOutProvidersToggleLabel(2, false)).toBe('未登录 2 个 provider · 展开');
    expect(loggedOutProvidersToggleLabel(2, true)).toBe('未登录 2 个 provider · 收起');
  });

  it('旧数据与非法值按未检查显示，不会误称已登录', () => {
    expect(loginOf({})).toBe('unknown');
    expect(loginOf({ login: 'bad' })).toBe('unknown');
    expect(loginLabel(loginOf({}))).toBe('未检查');
    expect(loginLabel('logged_in')).toBe('已登录');
    expect(loginLabel('logged_out')).toBe('未登录');
  });

  it('混合 provider 保留已登录列，只给未检查的模型标记', () => {
    const models = [
      model({ provider: 'mixed', id: 'ready-model', login: 'logged_in' }),
      model({ provider: 'mixed', id: 'unknown-model', login: 'unknown' }),
    ];
    const groups = splitPiProviderGroups(models);
    expect(groups.shown.map((group) => group.provider)).toEqual(['mixed']);
    expect(groups.unknown).toEqual([]);
    const agent: ResourceAgent = { id: 'pi', name: 'pi', kind: '内置', detected: true, version: '1', models };
    const markup = renderToStaticMarkup(
      <ResourceAgentList
        agents={[agent]} expanded={['pi']} openUnverified={['pi/mixed']} graphOpen
        onToggle={() => {}} onToggleUnverified={() => {}} onPick={() => {}}
      />,
    );
    expect(markup).toContain('ready-model');
    expect(markup).toContain('unknown-model');
    expect(markup).toContain('未检查');
  });

  it('左栏展开未登录汇总后只显示 provider 和重新检查，不铺模型', () => {
    const agent: ResourceAgent = {
      id: 'pi', name: 'pi', kind: '内置', detected: true, version: '1',
      models: [
        model({ provider: 'ready', id: 'ready-model', login: 'logged_in' }),
        model({ provider: 'out', id: 'hidden-model', login: 'logged_out' }),
      ],
    };
    const markup = renderToStaticMarkup(
      <ResourceAgentList
        agents={[agent]}
        expanded={['pi']}
        openUnverified={[]}
        openLoggedOut={['pi']}
        openUnknown={[]}
        graphOpen
        onToggle={() => {}}
        onToggleUnverified={() => {}}
        onToggleLoggedOut={() => {}}
        onToggleUnknown={() => {}}
        onRecheckProvider={async () => {}}
        onPick={() => {}}
      />,
    );
    expect(markup).toContain('未登录 1 个 provider · 收起');
    expect(markup).toContain('>out<');
    expect(markup).toContain('重新检查');
    expect(markup).not.toContain('hidden-model');
  });

  it('未检查 provider 同样不铺模型，并使用未检查文案', () => {
    const agent: ResourceAgent = {
      id: 'pi', name: 'pi', kind: '内置', detected: true, version: '1',
      models: [model({ provider: 'unknown', id: 'unknown-model' })],
    };
    const markup = renderToStaticMarkup(
      <ResourceAgentList
        agents={[agent]}
        expanded={['pi']}
        openUnverified={[]}
        openUnknown={['pi']}
        graphOpen
        onToggle={() => {}}
        onToggleUnverified={() => {}}
        onToggleUnknown={() => {}}
        onPick={() => {}}
      />,
    );
    expect(markup).toContain('未检查 1 个 provider · 收起');
    expect(markup).toContain('>unknown<');
    expect(markup).not.toContain('unknown-model');
  });

  it('已执行检查但仍未知时显示登录状态未检查', () => {
    const agent: ResourceAgent = {
      id: 'pi', name: 'pi', kind: '内置', detected: false, version: '未检测',
      models: [model({ provider: 'missing', id: 'm', login: 'unknown', login_checked_at: '2026-09-15T12:00:00+08:00' })],
    };
    const markup = renderToStaticMarkup(
      <ResourceAgentList
        agents={[agent]} expanded={['pi']} openUnverified={[]} openUnknown={['pi']} graphOpen
        onToggle={() => {}} onToggleUnverified={() => {}} onToggleUnknown={() => {}} onPick={() => {}}
      />,
    );
    expect(markup).toContain('登录状态未检查');
  });
});

describe('TASK-17 G1 登录提示的默认选择', () => {
  it('已退出登录的目标与额度用尽一样默认不勾，用户触碰后可勾回', () => {
    const graph: Graph = {
      version: 1, id: 'g', title: '', root: 'root', created_at: '', updated_at: '',
      nodes: {
        root: { id: 'root', kind: 'material', parents: [], user: { text: '', attachments: [] }, created_at: '' },
        ready: {
          id: 'ready', kind: 'turn', parents: ['root'], user: { text: '', attachments: [] }, created_at: '',
          target: { agent: 'pi', provider: 'ready', model: 'm', role: '执行', thinking: '中', visibility: '共享', write: '只读' },
        },
        'logged-out': {
          id: 'logged-out', kind: 'turn', parents: ['root'], user: { text: '', attachments: [] }, created_at: '',
          target: { agent: 'pi', provider: 'out', model: 'm', role: '执行', thinking: '中', visibility: '共享', write: '只读' },
        },
      },
    };
    const models = [
      model({ provider: 'ready', id: 'm', login: 'logged_in' }),
      model({ provider: 'out', id: 'm', login: 'logged_out' }),
    ];
    expect(loggedOutNodeIds(graph, ['ready', 'logged-out'], models)).toEqual(['logged-out']);
    expect(initialBlockedSelection(['ready', 'logged-out'], [], ['logged-out'])).toEqual(['ready']);
    expect(initialBlockedSelection(['ready', 'logged-out'], [], [])).toEqual(['ready', 'logged-out']);
    const source = readFileSync(new URL('../../components/center/graph/DispatchOverlay.tsx', import.meta.url), 'utf8');
    expect(source).toContain('<div className="dispatch-stage-heading">未登录</div>');
    expect(source).toContain('该 provider 未登录，默认不派，可勾选后继续。');
  });

  it('同时未登录与额度用尽时只进未登录分区，且 G1 接线会默认取消勾选', () => {
    const graph: Graph = {
      version: 1, id: 'g', title: '', root: 'root', created_at: '', updated_at: '',
      nodes: {
        root: { id: 'root', kind: 'material', parents: [], user: { text: '', attachments: [] }, created_at: '' },
        both: {
          id: 'both', kind: 'turn', parents: ['root'], user: { text: '', attachments: [] }, created_at: '',
          target: { agent: 'pi', provider: 'out', model: 'm', role: '执行', thinking: '中', visibility: '共享', write: '只读' },
        },
      },
    };
    const models = [model({ provider: 'out', id: 'm', login: 'logged_out', quota_exhausted_until: '2099-01-01T00:00:00+08:00' })];
    expect(blockedNodePartitions(graph, ['both'], models)).toEqual({ quotaOnlyIds: [], loggedOutIds: ['both'] });
    expect(initialResourceBlockedSelection(graph, ['both'], undefined, models)).toEqual([]);
  });

  it('备选顺序编辑器给未登录模型标记', () => {
    const primary = model({ provider: 'ready', id: 'main', login: 'logged_in', fallback_order: [] });
    const loggedOut = model({ provider: 'out', id: 'backup', login: 'logged_out' });
    const markup = renderToStaticMarkup(
      <FallbackOrderEditor model={primary} models={[primary, loggedOut]} onChange={() => {}} />,
    );
    expect(markup).toContain('pi/out/backup（未登录）');
  });
});

describe('TASK-17 接入页等高布局', () => {
  it('四列等高，页面限于视口，模型列表在卡片内滚动', () => {
    const css = readFileSync(new URL('../../components/resources/resources.css', import.meta.url), 'utf8');
    const page = /\.resources-page\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
    const grid = /\.agent-card-grid\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
    const card = /\.agent-card\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
    const models = /\.agent-card-models\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
    expect(page).toContain('max-height: 100vh');
    expect(page).toContain('overflow: hidden');
    expect(grid).toContain('grid-template-columns: repeat(4, minmax(0, 1fr))');
    expect(grid).toContain('grid-auto-rows: minmax(0, 1fr)');
    expect(card).toContain('min-height: 0');
    expect(models).toContain('overflow: auto');
  });

  it('登录折叠行与重新检查按钮有键盘焦点环', () => {
    const css = readFileSync(new URL('../../components/resources/resources.css', import.meta.url), 'utf8');
    expect(css).toMatch(/\.resource-logged-out-toggle:focus-visible,[\s\S]*\.agent-card-logged-out-toggle:focus-visible\s*\{[^}]*outline:\s*2px solid var\(--accent\)/);
    expect(css).toMatch(/\.resource-logged-out-provider \.row-action:focus-visible,[\s\S]*\.agent-card-logged-out-provider \.row-action:focus-visible\s*\{[^}]*outline:\s*2px solid var\(--accent\)/);
  });
});

describe('TASK-17 R1 测试与报告卫生', () => {
  it('登录检查测试不用进程级环境变量传参', () => {
    const source = readFileSync(new URL('../../../../src/resources.rs', import.meta.url), 'utf8');
    expect(source).not.toContain('AICROSS_PI_ARGS_LOG');
  });

  it('REPORT-17 记录真机 status 形状、既有测试调整与当前清单', () => {
    const report = readFileSync(new URL('../../../../tasks/REPORT-17.md', import.meta.url), 'utf8');
    expect(report).toContain('"status":"ready"|"not_ready"');
    expect(report).toContain('resources-tab-09.test.tsx');
    expect(report).toContain('resources-overlay-09b.test.tsx');
    expect(report).toContain('task17_dirty_login_value_reads_as_unknown_instead_of_failing');
    expect(report).not.toContain('只反序列化顶层 `ready: bool`');
  });
});
