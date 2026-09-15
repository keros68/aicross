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
  ambiguousModelIds,
  modelHint,
  modelRowLabel,
  modelsByAgent,
  type ResourceModel,
  type ResourceStore,
} from '../types';
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
      id: 'pi',
      name: 'pi',
      kind: '内置',
      detected: true,
      version: '1.4.2',
      models: [
        model({ agent: 'pi', provider: 'zai', id: 'glm-4.7', source: SOURCE, verification: 'unverified' }),
        model({ agent: 'pi', provider: 'bigmodel', id: 'glm-4.7', source: SOURCE, verification: 'unverified' }),
        model({ agent: 'pi', provider: 'zai', id: 'glm-4.5', source: SOURCE, verification: 'failed' }),
      ],
    },
  ],
};

function render() {
  return renderToStaticMarkup(
    <ResourceAgentList
      agents={modelsByAgent(store)}
      expanded={['codex', 'pi']}
      graphOpen
      onToggle={() => {}}
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

  it('模型行显示模型名；原生无状态点，未验证带「未验证」，失败用胶囊', () => {
    const text = visibleText(render());
    expect(text).toContain('gpt-5.6-luna');
    expect(text).toContain('glm-4.7');
    expect(text).toContain('未验证');
    expect(text).toContain('验证失败');
    // 原生条目不占状态文字：整栏里「原生」两字不出现（那是接入页才写的词）
    expect(text).not.toContain('原生');
  });

  it('agent 行带字母徽标与弱化版本号', () => {
    const markup = render();
    expect(markup).toContain('agent-badge-sm');
    expect(visibleText(markup)).toContain('0.58.0');
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

  // 修正轮 R1-5：两个 provider 下的同名模型原来两行文字一模一样，点哪一行
  // 都插同一段 @pi/glm-4.7，最终 provider 由清单顺序决定。
  it('同名多 provider 的模型行带 provider 后缀', () => {
    const text = visibleText(render());
    expect(text).toContain('glm-4.7 · zai');
    expect(text).toContain('glm-4.7 · bigmodel');
    expect(text).toContain('glm-4.5');
    expect(text).not.toContain('glm-4.5 · zai');
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

describe('modelRowLabel / ambiguousModelIds', () => {
  it('只有同 id 挂在多个 provider 下才加后缀', () => {
    const models = [
      model({ agent: 'pi', provider: 'zai', id: 'glm-4.7' }),
      model({ agent: 'pi', provider: 'bigmodel', id: 'glm-4.7' }),
      model({ agent: 'pi', provider: 'zai', id: 'glm-4.5' }),
    ];
    const ambiguous = ambiguousModelIds(models);
    expect([...ambiguous]).toEqual(['glm-4.7']);
    expect(modelRowLabel(models[0], ambiguous)).toBe('glm-4.7 · zai');
    expect(modelRowLabel(models[2], ambiguous)).toBe('glm-4.5');
  });
});

describe('modelHint', () => {
  it('把来源、最近验证与失败原因收进同一条悬停提示', () => {
    const hint = modelHint(
      model({
        agent: 'pi',
        id: 'glm-4.5',
        source: SOURCE,
        smoked_at: '2026-09-15T10:02:00+08:00',
        smoke_model: 'zai/glm-4.5',
        smoke_failure: '401: invalid api key',
      }),
    );
    expect(hint.split('\n')).toEqual([
      'glm-4.5',
      SOURCE,
      '最近验证：2026-09-15T10:02:00+08:00（zai/glm-4.5）',
      '失败原因：401: invalid api key',
    ]);
  });

  // 修正轮 R1：provider 单列一行（原来它只是 source 的兜底），没记来源仍退到
  // 「来源未记录」——多 provider 的同名模型光看 source 分不清是哪个端点。
  it('provider 单列一行，没记来源退到「来源未记录」', () => {
    expect(modelHint(model({ id: 'a', provider: 'zai' }))).toBe('a\nprovider：zai\n来源未记录');
    expect(modelHint(model({ id: 'a' }))).toBe('a\n来源未记录');
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

  it('徽标字母走 --text-1、色相只留薄底；字号回到六级阶梯，没有 9px', () => {
    const badge = /\.agent-badge\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
    expect(badge).toContain('color: var(--text-1)');
    expect(badge).toContain('--agent-tone');
    const small = /\.agent-badge-sm\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
    expect(small).toContain('font-size: 11px');
    expect(css).not.toContain('font-size: 9px');
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
