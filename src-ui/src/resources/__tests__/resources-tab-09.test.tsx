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
  modelPickTarget,
  modelsByAgent,
  type ResourceModel,
  type ResourceStore,
} from '../types';
import { mentionRef } from '../mention';
import {
  agentEmblem,
  AGENT_ICON_ASSETS,
  CSS_TONED_SLUGS,
  hasCssTone,
} from '../../components/resources/brand-marks';
import { AgentBadge } from '../../components/resources/AgentBadge';
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
    resolved_source: '',
    source: '',
    smoked_at: '',
    smoke_model: '',
    smoke_failure: '',
    meta: '',
    note: '',
    // TASK-17：这组既有展示测试的 pi provider 明确设为已登录；旧库缺字段
    // 的「未检查」折叠行为由 resources-login-17.test.tsx 单独覆盖。
    login: 'logged_in',
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

  it('agent 行使用 fork 内已有图标与弱化版本号', () => {
    const markup = render();
    expect(markup).toContain('agent-badge-sm');
    expect(markup).toContain('src="/icons/tools/claude.svg"');
    expect(markup).toContain('src="/icons/tools/codex.svg"');
    expect(markup).toContain('agent-badge-pi agent-badge-inline-svg');
    expect(markup).not.toContain('agent-badge-letters">CO');
    expect(markup).not.toContain('agent-badge-letters">CL');
  });

  // TASK-19：左栏只显示名称；别名真身进入悬停提示，不形成第二列。
  it('claude 别名行只显示别名，真实 ID 进入悬停提示', () => {
    const markup = render();
    const text = visibleText(markup);
    expect(text).toContain('fable');
    expect(markup).toContain('模型 ID：claude-fable-5-1');
    expect(markup).not.toContain('resource-model-resolved');
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

  // 修正轮 R1（GLM「最该补的测试 3」）：两个 agent 挂同名 provider，展开其中
  // 一个的未验证清单，另一个必须还折着。原来只有 groupKey 的纯函数断言，
  // 没走渲染路径——组件里把折叠键写成 group.provider 也不会红。
  it('折叠状态不跨 agent 串：同名 provider 挂两个 agent', () => {
    const shared: ResourceStore = {
      version: 1,
      agents: [
        {
          id: 'pi',
          name: 'pi',
          kind: '内置',
          detected: true,
          version: '1.4.2',
          models: [
            model({ agent: 'pi', provider: 'zai', id: 'pi-ok', verification: 'verified' }),
            model({ agent: 'pi', provider: 'zai', id: 'pi-new', verification: 'unverified' }),
            model({ agent: 'pi', provider: 'other', id: 'pi-other', verification: 'verified' }),
          ],
        },
        {
          id: 'kimi',
          name: 'Kimi',
          kind: '内置',
          detected: true,
          version: '1.0',
          models: [
            model({ agent: 'kimi', provider: 'zai', id: 'kimi-ok', verification: 'verified' }),
            model({ agent: 'kimi', provider: 'zai', id: 'kimi-new', verification: 'unverified' }),
            model({ agent: 'kimi', provider: 'other', id: 'kimi-other', verification: 'verified' }),
          ],
        },
      ],
    };
    const text = visibleText(
      renderToStaticMarkup(
        <ResourceAgentList
          agents={modelsByAgent(shared)}
          expanded={['pi', 'kimi']}
          openUnverified={['pi/zai']}
          graphOpen
          onToggle={() => {}}
          onToggleUnverified={() => {}}
          onPick={() => {}}
        />,
      ),
    );
    expect(text).toContain('pi-new'); // pi 的 zai 组展开了
    expect(text).not.toContain('kimi-new'); // kimi 的 zai 组仍折着
    expect(text).toContain('未验证 1 个 · 收起');
    expect(text).toContain('未验证 1 个 · 展开');
  });

  // 修正轮 R1（DS 3）：单 provider 的 pi 不出组头，但 provider 名不能跟着消失。
  it('单 provider 的 agent 不出组头，provider 仍在行悬停里', () => {
    const single: ResourceStore = {
      version: 1,
      agents: [
        {
          id: 'pi',
          name: 'pi',
          kind: '内置',
          detected: true,
          version: '1.4.2',
          models: [
            model({ agent: 'pi', provider: 'bigmodel', id: 'glm-4.6', verification: 'verified' }),
            model({ agent: 'pi', provider: 'bigmodel', id: 'glm-4.5', verification: 'failed' }),
          ],
        },
      ],
    };
    const markup = renderToStaticMarkup(
      <ResourceAgentList
        agents={modelsByAgent(single)}
        expanded={['pi']}
        openUnverified={[]}
        graphOpen
        onToggle={() => {}}
        onToggleUnverified={() => {}}
        onPick={() => {}}
      />,
    );
    expect(markup).not.toContain('resource-group-head');
    expect(markup).toContain('provider：bigmodel');
    // 折叠键仍带真实 provider（抹成空串会让两个 agent 的同名组互相串）
    expect(markup).toContain('glm-4.6');
  });

  // 修正轮 R1（DS A1）：已知四家的徽标**不带**内联 --agent-tone。内联自定义
  // 属性压过样式表，只要内联还在，`[data-theme="light"]` 那套压暗色就永远
  // 不生效。上一版断言「markup 里有 vendorColor 的值」恰好证明内联存在，
  // 与「CSS 里有压暗规则」两条同时为真、功能为假——典型假绿。
  it('已知 agent 的徽标不带内联 --agent-tone，色由样式表按主题给', () => {
    const markup = render();
    expect(markup).not.toContain('--agent-tone');
    expect(markup).not.toContain(vendorColor('codex'));
    expect(markup).not.toContain(vendorColor('pi'));
    // class 上仍带 slug，样式表靠它选中
    expect(markup).toContain('agent-badge-claude');
    expect(markup).toContain('agent-badge-codex');
    expect(markup).toContain('agent-badge-pi');
    expect(markup).not.toContain('tone-codex');
  });

  it('未收录的 agent 没有主题分档，仍内联 vendor 兜底色', () => {
    const markup = renderToStaticMarkup(<AgentBadge agentId="manual-1x" />);
    expect(markup).toContain('--agent-tone');
    expect(markup).toContain(vendorColor('manual-1x'));
  });

  // 修正轮 R1（DS A5）：没开对话图时不能用 disabled——disabled 按钮不可聚焦，
  // 键盘 Tab 不到、读屏跳过，「aria-describedby 读得到说明」就不成立。
  it('没开对话图时用 aria-disabled，不用 disabled，行仍可聚焦', () => {
    const markup = renderToStaticMarkup(
      <ResourceAgentList
        agents={modelsByAgent(store)}
        expanded={EXPANDED}
        openUnverified={[]}
        graphOpen={false}
        onToggle={() => {}}
        onToggleUnverified={() => {}}
        onPick={() => {}}
      />,
    );
    expect(markup).toContain('aria-disabled="true"');
    expect(markup).not.toMatch(/class="resource-model-row"[^>]*\sdisabled/);
    expect(markup).toContain('先打开对话图');
    // 说明文本仍挂在 aria-describedby 指向的节点上
    const described = markup.match(/aria-describedby="([^"]+)"/);
    expect(described).not.toBeNull();
    expect(markup).toContain(`class="resources-sr-only" id="${described![1]}"`);
  });

  // 修正轮 R1（DS 10）：hintId 原来按 `agent-provider-id` 拼、非字母数字全换成
  // `-`，`glm-4.7` 与 `glm-4-7` 撞成同一个 DOM id，aria-describedby 指到第一条。
  it('每行的 aria-describedby 是各自的 id，不会撞', () => {
    const collide: ResourceStore = {
      version: 1,
      agents: [
        {
          id: 'pi',
          name: 'pi',
          kind: '内置',
          detected: true,
          version: '1.0',
          models: [
            model({ agent: 'pi', provider: 'zai', id: 'glm-4.7', verification: 'verified' }),
            model({ agent: 'pi', provider: 'zai', id: 'glm-4-7', verification: 'verified' }),
          ],
        },
      ],
    };
    const markup = renderToStaticMarkup(
      <ResourceAgentList
        agents={modelsByAgent(collide)}
        expanded={['pi']}
        openUnverified={[]}
        graphOpen
        onToggle={() => {}}
        onToggleUnverified={() => {}}
        onPick={() => {}}
      />,
    );
    const ids = [...markup.matchAll(/aria-describedby="([^"]+)"/g)].map((m) => m[1]);
    expect(ids).toHaveLength(2);
    expect(new Set(ids).size).toBe(2);
    for (const id of ids) expect(markup).toContain(`class="resources-sr-only" id="${id}"`);
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

// TASK-15-R1：四家复用 fork 已有图标，没收录的退回字母。
describe('agentEmblem', () => {
  it('claude / codex / kimi / pi 都走本地资产，未知项退回字母', () => {
    expect(agentEmblem('claude', 'CL')).toMatchObject({ kind: 'image', slug: 'claude', src: '/icons/tools/claude.svg' });
    expect(agentEmblem('codex', 'CO')).toMatchObject({ kind: 'image', slug: 'codex', src: '/icons/tools/codex.svg' });
    expect(agentEmblem('kimi', 'KI')).toMatchObject({ kind: 'image', slug: 'kimi' });
    expect(agentEmblem('kimicode', 'KI')).toMatchObject({ kind: 'image', slug: 'kimi' });
    expect(agentEmblem('pi', 'PI')).toMatchObject({ kind: 'inline-svg', slug: 'pi' });
    expect(agentEmblem('manual-1x', 'MA')).toEqual({
      kind: 'letters',
      slug: 'manual1x',
      letters: 'MA',
    });
  });

  it('AgentBadge 对四家都渲染已有图标，无字形分支', () => {
    for (const id of ['claude', 'codex', 'kimi', 'pi']) {
      const markup = renderToStaticMarkup(<AgentBadge agentId={id} />);
      expect(markup, id).not.toContain('agent-badge-letters');
      expect(markup, id).not.toContain('agent-badge-glyph');
      expect(markup, id).toMatch(/<img|<svg/);
    }
  });
});

describe('本地图标的字母回退', () => {
  it('删掉 codex 本地资产映射后退回字母', () => {
    const saved = AGENT_ICON_ASSETS.codex;
    try {
      delete AGENT_ICON_ASSETS.codex;
      expect(agentEmblem('codex', 'CO')).toEqual({
        kind: 'letters',
        slug: 'codex',
        letters: 'CO',
      });
      const markup = renderToStaticMarkup(<AgentBadge agentId="codex" />);
      expect(markup).toContain('agent-badge-codex agent-badge-letters');
      expect(markup).not.toContain('<svg');
      expect(markup).toContain('CO');
    } finally {
      AGENT_ICON_ASSETS.codex = saved;
    }
    expect(agentEmblem('codex', 'CO').kind).toBe('image');
  });

  it('色相由样式表给的 slug 与标志表 / 字形表对得上', () => {
    expect([...CSS_TONED_SLUGS].sort()).toEqual(['claude', 'codex', 'kimi', 'pi']);
    for (const slug of CSS_TONED_SLUGS) expect(hasCssTone(slug), slug).toBe(true);
    expect(hasCssTone('manual1x')).toBe(false);
  });
});

// 修正轮 R1（DS 2）：点一行插进对话图的 @ 引用必须带 provider，否则同名模型
// 分不清挂在哪个端点上。这套用例没有 DOM，点击不了，所以把 payload 单拎成
// 纯函数钉住；组件里只剩一行 onPick(target.agent, target.model, target.provider)。
describe('modelPickTarget', () => {
  it('把 provider 一起交出去，agent 缺省退回所在 agent 的 id', () => {
    expect(
      modelPickTarget(model({ agent: 'pi', id: 'glm-4.7', provider: 'bigmodel' }), 'pi'),
    ).toEqual({ agent: 'pi', model: 'glm-4.7', provider: 'bigmodel' });
    // 旧库里 model.agent 可能没回填
    expect(modelPickTarget(model({ id: 'glm-4.7', provider: 'zai' }), 'pi')).toEqual({
      agent: 'pi',
      model: 'glm-4.7',
      provider: 'zai',
    });
    // 原生 agent 没有 provider，仍是空串而不是 undefined
    expect(modelPickTarget(model({ agent: 'claude', id: 'fable' }), 'claude')).toEqual({
      agent: 'claude',
      model: 'fable',
      provider: '',
    });
  });

  it('拼出来的 @ 引用带 #provider', () => {
    const target = modelPickTarget(
      model({ agent: 'pi', id: 'glm-4.7', provider: 'bigmodel' }),
      'pi',
    );
    expect(mentionRef(target.agent, target.model, target.provider)).toBe('@pi/glm-4.7#bigmodel');
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
      '登录状态：已登录',
      SOURCE,
      '最近验证：2026-09-15T10:02:00+08:00（zai/glm-4.5）',
      '失败原因：401: invalid api key',
    ]);
  });

  // 修正轮 R1：provider 单列一行（原来它只是 source 的兜底），没记来源仍退到
  // 「来源未记录」——多 provider 的同名模型光看 source 分不清是哪个端点。
  it('provider 单列一行，没记来源退到「来源未记录」', () => {
    expect(modelHint(model({ id: 'a', agent: 'pi', provider: 'zai' }))).toBe(
      'a\n状态：未验证\nprovider：zai\n登录状态：已登录\n来源未记录',
    );
    expect(modelHint(model({ id: 'a', agent: 'pi' }))).toBe('a\n状态：未验证\n登录状态：已登录\n来源未记录');
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

// ── 对比度（修正轮 R1，DS A2/A3） ───────────────────────────────────────
// WCAG 2.2 的 1.4.11「非文本对比度」：图形与状态指示 ≥ 3:1。徽标与状态点
// 在左栏是唯一可见标识（状态词本轮删了），所以按 3:1 收。
// 计算基准是各主题自己的 `--bg-panel`（左栏与卡片的实际底色），从
// global.css 里读，不手抄——上一版报告正是把「对白底」的值当成了对 #202020。

/** 只对真规则做断言：注释里举的例子不该被当成规则命中。 */
function stripComments(source: string): string {
  return source.replace(/\/\*[\s\S]*?\*\//g, '');
}

function relativeLuminance(hex: string): number {
  const value = hex.replace('#', '');
  const channels = [0, 2, 4].map((offset) => {
    const part = Number.parseInt(value.slice(offset, offset + 2), 16) / 255;
    return part <= 0.03928 ? part / 12.92 : ((part + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2];
}

function contrast(foreground: string, background: string): number {
  const a = relativeLuminance(foreground);
  const b = relativeLuminance(background);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

/** 主题名 → 该主题的 token 表（只取 6 位十六进制的那些）。 */
function themeTokens(globalCss: string): Map<string, Map<string, string>> {
  const themes = new Map<string, Map<string, string>>();
  for (const block of globalCss.matchAll(/\[data-theme="([a-z]+)"\]\s*\{([^}]*)\}/g)) {
    const tokens = themes.get(block[1]) ?? new Map<string, string>();
    for (const line of block[2].matchAll(/(--[a-z0-9-]+):\s*(#[0-9a-fA-F]{6})\s*;/g)) {
      tokens.set(line[1], line[2].toLowerCase());
    }
    themes.set(block[1], tokens);
  }
  return themes;
}

/** resources.css 里某个 slug 的徽标色：`light` 取浅色覆盖，其余取基础规则。 */
function badgeTone(resourcesCss: string, slug: string, theme = 'dark'): string {
  const selector =
    theme === 'light'
      ? `\\[data-theme="light"\\] \\.agent-badge-${slug}`
      : `(?<!\\] )\\.agent-badge-${slug}`;
  const rule = new RegExp(`${selector}\\s*\\{[^}]*\\}`).exec(resourcesCss)?.[0] ?? '';
  return (/--agent-tone:\s*(#[0-9a-fA-F]{6})/.exec(rule)?.[1] ?? '').toLowerCase();
}

describe('徽标与状态点的对比度 ≥ 3:1（修正轮 R1）', () => {
  const resourcesCss: string = stripComments(
    readFileSync(new URL('../../components/resources/resources.css', import.meta.url), 'utf8'),
  );
  const globalCss: string = stripComments(
    readFileSync(new URL('../../styles/global.css', import.meta.url), 'utf8'),
  );
  const themes = [...themeTokens(globalCss)].filter(([, tokens]) => tokens.has('--bg-panel'));

  it('global.css 的主题表读得到（读不到就不是「全过」而是没测）', () => {
    expect(themes.length).toBeGreaterThanOrEqual(10);
    expect(themes.map(([name]) => name)).toContain('cappuccino');
    expect(themes.map(([name]) => name)).toContain('mint');
    expect(new Map(themes).get('light')?.get('--bg-panel')).toBe('#fafaf9');
  });

  it('四家徽标在每套主题的面板底色上都 ≥ 3:1', () => {
    for (const [name, tokens] of themes) {
      const panel = tokens.get('--bg-panel')!;
      for (const slug of CSS_TONED_SLUGS) {
        const tone = badgeTone(resourcesCss, slug, name === 'light' ? 'light' : 'dark');
        expect(tone, `${name}/${slug} 没有取到颜色`).toMatch(/^#[0-9a-f]{6}$/);
        expect(
          Number(contrast(tone, panel).toFixed(2)),
          `${name} / ${slug} ${tone} on ${panel}`,
        ).toBeGreaterThanOrEqual(3);
      }
    }
  });

  it('三色状态点在每套主题上都 ≥ 3:1', () => {
    for (const [name, tokens] of themes) {
      const panel = tokens.get('--bg-panel')!;
      // 未验证灰点 = --verify-gray = --text-2；已验证绿点浅色主题走 --verify-green
      const dots: [string, string][] = [
        ['unverified', tokens.get('--text-2')!],
        ['verified', name === 'light' ? '#1f7a3d' : tokens.get('--green')!],
        ['failed', tokens.get('--red')!],
      ];
      for (const [state, color] of dots) {
        expect(
          Number(contrast(color, panel).toFixed(2)),
          `${name} / ${state} ${color} on ${panel}`,
        ).toBeGreaterThanOrEqual(3);
      }
    }
  });
});

// 修正轮 R1-5：视觉纪律与可访问性，逐条钉在 resources.css 上。
describe('resources.css 的视觉纪律（修正轮 R1）', () => {
  const css: string = stripComments(
    readFileSync(new URL('../../components/resources/resources.css', import.meta.url), 'utf8'),
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

  it('徽标 18px；字母兜底字号仍在六级阶梯上，没有 9px', () => {
    const badge = /\.agent-badge\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
    expect(badge).toContain('width: 18px');
    expect(badge).toContain('height: 18px');
    expect(badge).toContain('color: var(--agent-tone');
    const fallback = /\.agent-badge-letters\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
    expect(fallback).toContain('font-size: 12px');
    expect(css).not.toContain('font-size: 9px');
  });

  // TASK-09b：任务书要求深浅主题各一套色。vendor.ts 的原色按深色底挑，
  // 白底上 18px 实心图形到不了 3:1。
  // 修正轮 R1：深色那一套也必须在样式表里（原来只有浅色一套，深色走内联，
  // 于是内联压过浅色规则，两套色实际只有一套）。
  it('四家徽标深浅主题各有一套色，都写在样式表里', () => {
    for (const slug of CSS_TONED_SLUGS) {
      const base = new RegExp(`(?<!\\] )\\.agent-badge-${slug}\\s*\\{[^}]*\\}`).exec(css)?.[0];
      expect(base, `${slug} 深色`).toBeTruthy();
      expect(base, `${slug} 深色`).toContain('--agent-tone:');
      const light = new RegExp(
        `\\[data-theme="light"\\] \\.agent-badge-${slug}\\s*\\{[^}]*\\}`,
      ).exec(css)?.[0];
      expect(light, `${slug} 浅色`).toBeTruthy();
      expect(light, `${slug} 浅色`).toContain('--agent-tone:');
      expect(light, `${slug} 浅色`).not.toBe(base);
    }
  });

  it('claude / codex / pi 的深色调用的就是 graph/vendor.ts 的厂商原色', () => {
    for (const slug of ['claude', 'codex', 'pi']) {
      expect(badgeTone(css, slug), slug).toBe(vendorColor(slug).toLowerCase());
    }
    // kimi 是唯一的例外：原色对最亮的深色面板到不了 3:1，提亮到同色相
    expect(badgeTone(css, 'kimi')).not.toBe(vendorColor('kimi').toLowerCase());
  });

  it('未验证灰点走 --verify-gray（--text-3 在 cappuccino 只有 2.06:1）', () => {
    expect(css).toContain('--verify-gray: var(--text-2)');
    const base = /\.verify-dot\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
    expect(base).toContain('background: var(--verify-gray)');
    expect(base).not.toContain('background: var(--text-3)');
  });

  it('未开对话图的模型行用 aria-disabled 而不是 :disabled', () => {
    expect(css).toContain(".resource-model-row[aria-disabled='true']");
    expect(css).not.toContain('.resource-model-row:disabled');
  });

  it('页标题回到六级阶梯，弹窗间隙回到 4 的倍数', () => {
    const h1 = /\.resources-page h1\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
    expect(h1).toContain('font-size: 18px');
    expect(css).not.toContain('font-size: 20px');
    const dialog = /\.res-dialog\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
    expect(dialog).toContain('gap: 12px');
    expect(dialog).not.toContain('gap: 10px');
  });

  // TASK-09b：状态点三色各有规则——失败原来只有胶囊，左栏换成点之后
  // 少一条 .verify-dot.verify-failed 就会画成灰点。
  it('状态点三色齐全：verified 绿、failed 红、unverified 灰', () => {
    expect(css).toContain('.verify-dot.verify-verified');
    expect(css).toContain('.verify-dot.verify-failed');
    const base = /\.verify-dot\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
    expect(base).toContain('background: var(--verify-gray)');
  });

  it('已验证的绿点在浅色主题有专用深绿（--green 只有 2.2:1）', () => {
    expect(css).toContain('--verify-green: #1f7a3d');
    const dot = /\.verify-dot\.verify-verified\s*\{[^}]*\}/.exec(css)?.[0] ?? '';
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
