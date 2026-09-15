// TASK-09：左栏资源 tab 的展示层。用户提的第一条是「每行塞着 读:C:\Users\…
// 来源路径，很乱」，所以这里钉住：来源路径只进 title 悬停，不进正文。

// tsconfig.app.json 的 types 里只有 vite/client：应用代码不该碰 node 内置模块，
// 只有样式断言要读磁盘，就地压掉解析错误（与 consistency-06b.test.tsx 同一写法）。
// @ts-expect-error node builtin, not in this project's `types`
import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { ResourceAgentList } from '../../components/resources/ResourcesTab';
import {
  modelHint,
  modelsByAgent,
  type ResourceModel,
  type ResourceStore,
} from '../types';
import { agentEmblem, BRAND_MARKS } from '../../components/resources/brand-marks';
import { verificationHint } from '../types';
import { VerificationHint } from '../../components/center/graph/DispatchBits';
import { vendorColor } from '../../graph/vendor';

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
    source: '',
    smoked_at: '',
    smoke_model: '',
    smoke_failure: '',
    meta: '',
    note: '',
    ...partial,
  };
}

const SOURCE = '读:C:\\Users\\keros68\\.codex\\models_cache.json (2026-09-15T10:00:00+08:00)';

const store: ResourceStore = {
  version: 1,
  agents: [
    {
      id: 'codex',
      name: 'Codex',
      kind: '内置',
      detected: true,
      version: '0.58.0',
      models: [model({ agent: 'codex', id: 'gpt-5.6-luna', source: SOURCE, verification: 'native' })],
    },
    {
      id: 'claude',
      name: 'Claude Code',
      kind: '内置',
      detected: true,
      version: '2.1.0',
      models: [
        model({
          agent: 'claude',
          id: 'fable',
          resolved_id: 'claude-fable-5-1',
          source: SOURCE,
          verification: 'native',
        }),
        model({
          agent: 'claude',
          id: 'claude-opus-5',
          resolved_id: 'claude-opus-5',
          source: SOURCE,
          verification: 'native',
        }),
      ],
    },
    {
      id: 'pi',
      name: 'pi',
      kind: '内置',
      detected: true,
      version: '1.4.2',
      models: [
        model({ agent: 'pi', provider: 'zai', id: 'glm-4.7', source: SOURCE, verification: 'unverified' }),
        model({ agent: 'pi', provider: 'bigmodel', id: 'glm-4.7', source: SOURCE, verification: 'unverified' }),
        model({ agent: 'pi', provider: 'zai', id: 'glm-4.5', source: SOURCE, verification: 'failed' }),
        model({ agent: 'pi', provider: 'zai', id: 'glm-4.6', source: SOURCE, verification: 'verified' }),
      ],
    },
  ],
};

const EXPANDED = ['codex', 'claude', 'pi'];

function render(openUnverified: string[] = []) {
  return renderToStaticMarkup(
    <ResourceAgentList
      agents={modelsByAgent(store)}
      expanded={EXPANDED}
      openUnverified={openUnverified}
      graphOpen
      onToggle={() => {}}
      onToggleUnverified={() => {}}
      onPick={() => {}}
    />,
  );
}

/**
 * 去掉标签，只留用户在栏里真正看到的文字。`.resources-sr-only` 的内容是给
 * 读屏与 aria-describedby 用的，视觉上不出现（CSS 里 1px + clip-path），
 * 先整块剔掉再剥标签（修正轮 R1）。
 */
function visibleText(markup: string): string {
  return markup
    .replace(/<span class="resources-sr-only"[^>]*>[\s\S]*?<\/span>/g, '')
    .replace(/<[^>]*>/g, '');
}

describe('ResourceAgentList', () => {
  it('来源路径只在 title 悬停里，不渲染进行内正文', () => {
    const markup = render();
    expect(markup).toContain('models_cache.json'); // title 属性里还在
    expect(visibleText(markup)).not.toContain('models_cache.json');
    expect(visibleText(markup)).not.toContain('读:C:');
  });

  // TASK-09b：状态只留一个点，文字进 aria-label / title。原来 pi 底下几十行
  // 每行都跟着「未验证」三个字，正是用户说的「很乱」。
  it('状态点不跟文字：状态词只在 aria-label 与 title 里', () => {
    const markup = render(['pi/zai', 'pi/bigmodel']);
    const text = visibleText(markup);
    expect(text).toContain('gpt-5.6-luna');
    expect(text).toContain('glm-4.7');
    // 常显行的状态词一个都不进正文
    expect(text).not.toContain('已验证');
    expect(text).not.toContain('验证失败');
    expect(text).not.toContain('原生');
    // 点本身带状态词，给读屏与鼠标悬停
    expect(markup).toContain('class="verify-dot verify-failed" role="img" aria-label="验证失败"');
    expect(markup).toContain('class="verify-dot verify-verified" role="img" aria-label="已验证"');
    // 原生条目连点都没有
    expect(markup).not.toContain('verify-native');
  });

  it('agent 行带品牌标志与弱化版本号；徽标不再用字母', () => {
    const markup = render();
    expect(markup).toContain('agent-badge-sm');
    // claude → Anthropic 标志，codex → OpenAI 标志，pi → π 字形
    expect(markup).toContain(BRAND_MARKS.claude.path);
    expect(markup).toContain(BRAND_MARKS.codex.path);
    expect(markup).toContain('agent-badge-pi agent-badge-glyph');
    expect(visibleText(markup)).toContain('π');
    // 原来的字母徽标（CO / CL / PI）不再出现
    expect(visibleText(markup)).not.toContain('CO');
    expect(visibleText(markup)).not.toContain('CL');
    expect(visibleText(markup)).not.toContain('PI');
  });

  // TASK-09b：用户原话「claude 没有模型 id」。别名行里补真身，别名与 ID 相同
  // 的模型只显示一次。
  it('claude 别名行显示别名 + 真实 ID，非别名模型只显示一次', () => {
    const text = visibleText(render());
    expect(text).toContain('fable');
    expect(text).toContain('claude-fable-5-1');
    // claude-opus-5 的 resolved_id 与 id 相同：整栏里只出现一次
    expect(text.split('claude-opus-5')).toHaveLength(2);
  });

  // TASK-09b：多 provider 的 agent 按 provider 分小组，组内不再带后缀；
  // 未验证默认折成一行。
  it('pi 按 provider 分小组，未验证默认折叠，组内不带 provider 后缀', () => {
    const text = visibleText(render());
    expect(text).toContain('zai');
    expect(text).toContain('bigmodel');
    // 后缀写法没了
    expect(text).not.toContain('glm-4.7 · zai');
    // zai 组：已验证的 glm-4.6 与失败的 glm-4.5 常显，未验证的 glm-4.7 折叠
    expect(text).toContain('glm-4.6');
    expect(text).toContain('glm-4.5');
    expect(text).not.toContain('glm-4.7');
    expect(text).toContain('未验证 1 个 · 展开');
    // 原生 agent 没有小组头
    expect(render()).not.toContain('class="resource-group-head"><span class="resource-group-name"></span>');
  });

  it('展开某个小组后，只有该组的未验证模型出现', () => {
    const text = visibleText(render(['pi/zai']));
    expect(text).toContain('glm-4.7');
    expect(text).toContain('未验证 1 个 · 收起');
    expect(text).toContain('未验证 1 个 · 展开'); // bigmodel 那组仍然折着
  });

  // 修正轮 R1-5：徽标色相与对话图上的厂商色点同源（graph/vendor.ts），
  // 不再是资源页自己的一套 --agent-* 变量。
  it('徽标色相从 graph/vendor.ts 取，不另立一套', () => {
    const markup = render();
    expect(markup).toContain(vendorColor('codex'));
    expect(markup).toContain(vendorColor('pi'));
    expect(markup).not.toContain('tone-codex');
    expect(markup).not.toContain('tone-pi');
  });

  // 修正轮 R1-5：title 在 WebView2 里只响应 hover，键盘用户读不到来源与
  // 最近验证时间；同一段文本挂进 aria-describedby 指向的 sr-only 节点。
  it('悬停提示同时挂进 aria-describedby，键盘可达', () => {
    const markup = render();
    const described = markup.match(/aria-describedby="([^"]+)"/);
    expect(described).not.toBeNull();
    expect(markup).toContain(`class="resources-sr-only" id="${described![1]}"`);
    expect(markup).toContain('models_cache.json');
  });
});

// TASK-09b：品牌标志表本身。路径数据来自 simple-icons（CC0-1.0），
// 这里钉住「四家各画什么」与「没收录的退回字母」。
describe('agentEmblem', () => {
  it('claude / codex / kimi 走品牌标志，pi 走 π 字形，未知退回字母', () => {
    expect(agentEmblem('claude', 'CL')).toMatchObject({ kind: 'mark', slug: 'claude' });
    expect(agentEmblem('codex', 'CO')).toMatchObject({ kind: 'mark', slug: 'codex' });
    expect(agentEmblem('kimi', 'KI')).toMatchObject({ kind: 'mark', slug: 'kimi' });
    expect(agentEmblem('pi', 'PI')).toEqual({ kind: 'glyph', slug: 'pi', glyph: 'π' });
    expect(agentEmblem('manual-1x', 'MA')).toEqual({
      kind: 'letters',
      slug: 'manual1x',
      letters: 'MA',
    });
  });

  it('标志名与路径数据都在（漏抄一条会渲染成空徽标）', () => {
    for (const [slug, mark] of Object.entries(BRAND_MARKS)) {
      expect(mark.viewBox, slug).toBe('0 0 24 24');
      expect(mark.path.length, slug).toBeGreaterThan(40);
      expect(mark.title, slug).not.toBe('');
    }
    expect(BRAND_MARKS.claude.title).toBe('Anthropic');
    expect(BRAND_MARKS.codex.title).toBe('OpenAI');
  });
});

describe('modelHint', () => {
  it('把来源、最近验证与失败原因收进同一条悬停提示', () => {
    const hint = modelHint(
      model({
        agent: 'pi',
        id: 'glm-4.5',
        verification: 'failed',
        source: SOURCE,
        smoked_at: '2026-09-15T10:02:00+08:00',
        smoke_model: 'zai/glm-4.5',
        smoke_failure: '401: invalid api key',
      }),
    );
    expect(hint.split('\n')).toEqual([
      'glm-4.5',
      '状态：验证失败',
      SOURCE,
      '最近验证：2026-09-15T10:02:00+08:00（zai/glm-4.5）',
      '失败原因：401: invalid api key',
    ]);
  });

  // 修正轮 R1：provider 单列一行（原来它只是 source 的兜底），没记来源仍退到
  // 「来源未记录」——多 provider 的同名模型光看 source 分不清是哪个端点。
  it('provider 单列一行，没记来源退到「来源未记录」', () => {
    expect(modelHint(model({ id: 'a', agent: 'pi', provider: 'zai' }))).toBe(
      'a\n状态：未验证\nprovider：zai\n来源未记录',
    );
    expect(modelHint(model({ id: 'a', agent: 'pi' }))).toBe('a\n状态：未验证\n来源未记录');
  });

  // TASK-09b：状态词与真身都进悬停——行内只剩一个点，这是唯一的读法。
  it('别名的真身与状态词都进悬停提示', () => {
    const hint = modelHint(
      model({ agent: 'claude', id: 'fable', resolved_id: 'claude-fable-5-1', source: SOURCE }),
    );
    expect(hint.split('\n')).toEqual([
      'fable',
      '模型 ID：claude-fable-5-1',
      '状态：原生',
      SOURCE,
    ]);
  });
});

// 修正轮 R1-5：G1 里「未验证」是第三方模型的常态（本机 pi 几十条都是），
// 原来画成红字（dispatch-warning），红字贬值、真正的 failed 被淹没。
describe('verificationHint / VerificationHint', () => {
  it('未验证走常规提示，验证失败才是红字，其余不出提示', () => {
    expect(verificationHint('unverified')).toEqual({
      text: '该模型未验证，首次派发会先发一个短请求核对真身。',
      tone: 'reason',
    });
    expect(verificationHint('failed')?.tone).toBe('warning');
    expect(verificationHint('native')).toBeNull();
    expect(verificationHint('verified')).toBeNull();
    expect(verificationHint(undefined)).toBeNull();
  });

  it('渲染出来的 class 与 tone 对得上', () => {
    expect(renderToStaticMarkup(<VerificationHint verification="unverified" />)).toContain(
      'class="dispatch-reason"',
    );
    expect(renderToStaticMarkup(<VerificationHint verification="failed" />)).toContain(
      'class="dispatch-warning"',
    );
    expect(renderToStaticMarkup(<VerificationHint verification="native" />)).toBe('');
  });
});

// 修正轮 R1-5：视觉纪律与可访问性，逐条钉在 resources.css 上。
describe('resources.css 的视觉纪律（修正轮 R1）', () => {
  const css: string = readFileSync(
    new URL('../../components/resources/resources.css', import.meta.url),
    'utf8',
  );

  it('「详情」按钮有自己的规则，旧的 details/summary 死规则删掉', () => {
    expect(css).toContain('.agent-card-detail-toggle {');
    const rule = /\.agent-card-detail-toggle\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
    expect(rule).toContain('border: none');
    expect(rule).toContain('background: none');
    expect(css).not.toContain('.agent-card-model-detail summary');
  });

  it('焦点环 2px 不透明强调色，不用半透明的 --border-focus', () => {
    const rule =
      /\.resource-agent-summary:focus-visible,\s*\.resource-model-row:focus-visible\s*\{[^}]*\}/.exec(css)?.[0] ??
      '';
    expect(rule).toContain('outline: 2px solid var(--accent)');
    expect(rule).not.toContain('--border-focus');
  });

  // TASK-09b：徽标换成品牌标志，18px、色随 --agent-tone；字母兜底才留薄底。
  it('徽标 18px 单色随 --agent-tone；字号仍在六级阶梯上，没有 9px', () => {
    const badge = /\.agent-badge\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
    expect(badge).toContain('width: 18px');
    expect(badge).toContain('height: 18px');
    expect(badge).toContain('color: var(--agent-tone');
    const fallback =
      /\.agent-badge-glyph,\s*\.agent-badge-letters\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
    expect(fallback).toContain('font-size: 11px');
    expect(css).not.toContain('font-size: 9px');
  });

  // TASK-09b：任务书要求深浅主题各一套色。vendor.ts 的原色按深色底挑，
  // 白底上 18px 实心图形到不了 3:1。
  it('四家徽标在浅色主题各有一套压暗色', () => {
    for (const slug of ['claude', 'codex', 'kimi', 'pi']) {
      const rule = new RegExp(
        `\\[data-theme="light"\\] \\.agent-badge-${slug}\\s*\\{[^}]*\\}`,
      ).exec(css)?.[0];
      expect(rule, slug).toBeTruthy();
      expect(rule, slug).toContain('--agent-tone:');
    }
  });

  // TASK-09b：状态点三色各有规则——失败原来只有胶囊，左栏换成点之后
  // 少一条 .verify-dot.verify-failed 就会画成灰点。
  it('状态点三色齐全：verified 绿、failed 红、unverified 灰', () => {
    expect(css).toContain('.verify-dot.verify-verified');
    expect(css).toContain('.verify-dot.verify-failed');
    const base = /\.verify-dot\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
    expect(base).toContain('background: var(--text-3)');
  });

  it('已验证的绿点在浅色主题有专用深绿（--green 只有 2.2:1）', () => {
    expect(css).toContain('--verify-green: #1f7a3d');
    const dot = /\.verify-verified \.verify-dot\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
    expect(dot).toContain('var(--verify-green)');
  });

  it('左栏的内边距与间隙都是 4 的倍数', () => {
    for (const selector of [
      '.resource-list',
      '.resource-agent-summary',
      '.resource-model-row',
      '.resource-group-head',
      '.resource-unverified-toggle',
      '.resource-manage-link',
    ]) {
      const rule = new RegExp(`\\${selector}\\s*\\{[^}]*\\}`).exec(css)?.[0] ?? '';
      expect(rule, selector).not.toBe('');
      for (const [, value] of rule.matchAll(/(?:padding|gap):\s*([^;]+);/g)) {
        for (const size of value.trim().split(/\s+/)) {
          const px = /^(\d+)px$/.exec(size);
          if (px) expect(Number(px[1]) % 4, `${selector} ${size}`).toBe(0);
        }
      }
    }
  });
});
