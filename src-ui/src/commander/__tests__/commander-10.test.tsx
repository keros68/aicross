// TASK-10 总指挥与快速启动：编号与任务书「实现 5」的三组一致。
// 1) commanderArgs 四家映射 + 空模型
// 2) 启动器纯函数：可用 agent 过滤、模型清单按 agent 过滤与显示文本、默认行文案
// 3) 启动器组件：没装任何一家时的兜底、次级入口两条
//
// Rust 侧的 commander.json 往返与缺字段默认在
// `src/orchestration.rs::tests::task10_commander_roundtrip_and_missing_field_defaults`。

import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import {
  commanderAgentOfTool,
  commanderArgs,
  commanderIdentifierError,
  commanderToolId,
  COMMANDER_AGENTS,
  isCommanderAgent,
} from '../args';
import {
  availableCommanderAgents,
  commanderAgentLabel,
  commanderBlockReason,
  commanderDefaultAvailable,
  commanderDefaultLine,
  commanderModelSelectValue,
  commanderModelLabel,
  commanderModelsFor,
  commanderModelValue,
  commanderTabTitle,
  cwdName,
  emptyGraphTabTitle,
  graphTabTitle,
  markCommanderSelection,
  otherInstalledCommanderClis,
  parseCommanderModelValue,
  shouldShowNoCommanderModels,
  commanderStartedCallback,
  commanderNewTabLaunch,
  commanderTabTransition,
  COMMANDER_AGENT_CARD_HEIGHT,
  COMMANDER_AGENT_GRID_TEMPLATE, commanderLauncherHidden,
} from '../launcher';
import { emptyCommanderSettings } from '../types';
import type { CommanderSettings } from '../types';
import { AGENT_DISPLAY_NAMES, agentDisplayName } from '../../resources/agent-display';
import {
  CommanderLauncher,
  CommanderModelSelect,
  CommanderOtherCliMenu,
} from '../../components/center/CommanderLauncher';
import type { PickableModel } from '../../resources/model-picker';
import type { DispatchableModel } from '../../graph/types';


const model = (over: Partial<PickableModel>): PickableModel => ({
  id: 'm',
  agent: 'claude',
  provider: '',
  status: '已接入',
  verification: 'native',
  ...over,
});

describe('TASK-10 (1) commanderArgs 四家映射 + 空模型', () => {
  it('claude / kimi 用 --model，codex 用 -m', () => {
    expect(commanderArgs('claude', '', 'fable')).toEqual(['--model', 'fable']);
    expect(commanderArgs('kimi', '', 'kimi-code/k3-256k')).toEqual([
      '--model',
      'kimi-code/k3-256k',
    ]);
    expect(commanderArgs('codex', '', 'gpt-5.6-luna')).toEqual(['-m', 'gpt-5.6-luna']);
  });

  it('pi 先 --provider 再 --model', () => {
    expect(commanderArgs('pi', 'zai-coding-cn', 'glm-5.3-flash')).toEqual([
      '--provider',
      'zai-coding-cn',
      '--model',
      'glm-5.3-flash',
    ]);
  });

  it('空模型 → 不加模型参数', () => {
    for (const agent of COMMANDER_AGENTS) {
      expect(commanderArgs(agent, '', '')).toEqual([]);
      expect(commanderArgs(agent, '', '   ')).toEqual([]);
    }
    // pi 的 provider 还在时只剩 provider 那一段
    expect(commanderArgs('pi', 'zai', '')).toEqual(['--provider', 'zai']);
    // provider 为空的 pi 不塞空串出去（pi 会按参数错拒绝）
    expect(commanderArgs('pi', '', 'glm-5.3-flash')).toEqual(['--model', 'glm-5.3-flash']);
  });

  it('不认识的 agent 返回空数组，绝不猜参数名', () => {
    expect(commanderArgs('grok', '', 'grok-4')).toEqual([]);
    expect(commanderArgs('', '', 'x')).toEqual([]);
    expect(isCommanderAgent('grok')).toBe(false);
  });

  it('模型与 provider 非法值整项拒绝，不 trim 或当成旗标传出', () => {
    expect(commanderIdentifierError('', 'model id')).toBe('模型 id 含非法字符');
    expect(commanderIdentifierError('', 'x&calc')).toBe('模型 id 含非法字符');
    expect(commanderIdentifierError('', '-danger')).toBe('模型 id 含非法字符');
    expect(commanderIdentifierError('', '   ')).toBe('模型 id 含非法字符');
    expect(commanderIdentifierError('bad provider', 'model')).toBe('provider id 含非法字符');
    expect(commanderArgs('claude', '', 'model id')).toEqual([]);
    expect(commanderArgs('claude', '', 'x&calc')).toEqual([]);
    expect(commanderArgs('claude', '', '-danger')).toEqual([]);
    expect(commanderArgs('claude', '', '   ')).toEqual([]);
    expect(commanderArgs('pi', 'bad provider', 'model')).toEqual([]);
  });

  it('agent id ↔ 工具 id：只有 kimi 两边不同名', () => {
    expect(commanderToolId('claude')).toBe('claude');
    expect(commanderToolId('codex')).toBe('codex');
    expect(commanderToolId('kimi')).toBe('kimicode');
    expect(commanderToolId('pi')).toBe('pi');
    expect(commanderToolId('grok')).toBeNull();
    expect(commanderAgentOfTool('kimicode')).toBe('kimi');
    expect(commanderAgentOfTool('terminal')).toBeNull();
    expect(commanderAgentOfTool(null)).toBeNull();
  });
});

describe('TASK-10 (2) 启动器纯函数', () => {
  it('TASK-15：四个 agent 显示名从同一张前端表取', () => {
    expect(AGENT_DISPLAY_NAMES).toEqual({
      claude: 'Claude Code',
      codex: 'Codex',
      kimi: 'Kimi Code',
      pi: 'pi agent',
    });
    expect(agentDisplayName('kimicode')).toBe('Kimi Code');
    expect(commanderAgentLabel('pi')).toBe('pi agent');
  });

  it('TASK-15-R1：新总指挥 tab 有默认时直接生成启动参数，当前工作区优先于存量默认目录', () => {
    const defaults = { ...emptyCommanderSettings(), agent: 'claude', model: 'fable', cwd: 'D:\\default-b' };
    expect(commanderNewTabLaunch(defaults, { claude: true }, true, 'D:\\workspace-a')).toEqual({
      agent: 'claude', provider: '', model: 'fable', cwd: 'D:\\workspace-a',
    });
    expect(commanderNewTabLaunch(defaults, { claude: true }, true, null)?.cwd).toBe('D:\\default-b');
    expect(commanderNewTabLaunch(defaults, { claude: false }, true, null)).toBeNull();
    expect(commanderNewTabLaunch(defaults, { claude: true }, false, null)).toBeNull();
  });

  it('TASK-15-R1：探测 false→true 只启动一次', () => {
    let state = {};
    let step = commanderTabTransition(state, { type: 'availability', tabId: 'tab-1', available: false });
    expect(step.shouldLaunch).toBe(false);
    step = commanderTabTransition(step.state, { type: 'availability', tabId: 'tab-1', available: true });
    expect(step.shouldLaunch).toBe(true);
    step = commanderTabTransition(step.state, { type: 'availability', tabId: 'tab-1', available: true });
    expect(step.shouldLaunch).toBe(false);
  });

  it('TASK-15-R1：另一 tab 写入默认不接管已显示启动器的 tab', () => {
    const shown = commanderTabTransition({}, { type: 'launcher-shown', tabId: 'tab-1' });
    const lateDefault = commanderTabTransition(shown.state, { type: 'availability', tabId: 'tab-1', available: true });
    expect(lateDefault.shouldLaunch).toBe(false);
    expect(lateDefault.state['tab-1']).toBe('launcher');
  });

  it('TASK-15-R1：点“更改”后不再自动启动', () => {
    const changed = commanderTabTransition({}, { type: 'changed', tabId: 'tab-1' });
    const next = commanderTabTransition(changed.state, { type: 'availability', tabId: 'tab-1', available: true });
    expect(next.shouldLaunch).toBe(false);
    expect(next.state['tab-1']).toBe('changed');
  });

  it('可用 agent 只列探测到的，按工具 id 查（kimi → kimicode）', () => {
    expect(
      availableCommanderAgents({ claude: true, codex: false, kimicode: true, pi: true }),
    ).toEqual(['claude', 'kimi', 'pi']);
    // 探测还没回来（undefined）按没装算
    expect(availableCommanderAgents({ claude: true })).toEqual(['claude']);
    expect(availableCommanderAgents({})).toEqual([]);
    // 顺序固定，不随传入 record 的键序变
    expect(
      availableCommanderAgents({ pi: true, kimicode: true, codex: true, claude: true }),
    ).toEqual(['claude', 'codex', 'kimi', 'pi']);
  });

  it('模型清单按 agent 过滤，顺序沿用后端排好的', () => {
    const models = [
      model({ id: 'fable', agent: 'claude' }),
      model({ id: 'glm-5.3-flash', agent: 'pi', provider: 'zai' }),
      model({ id: 'opus', agent: 'claude' }),
    ];
    expect(commanderModelsFor(models, 'claude').map((m) => m.id)).toEqual(['fable', 'opus']);
    expect(commanderModelsFor(models, 'pi').map((m) => m.id)).toEqual(['glm-5.3-flash']);
    expect(commanderModelsFor(models, 'codex')).toEqual([]);
  });

  it('显示文本：pi 带 provider，claude 有真身才显示别名 + 真实 ID', () => {
    expect(commanderModelLabel(model({ id: 'fable', agent: 'claude' }))).toBe('fable');
    expect(
      commanderModelLabel({ ...model({ id: 'fable', agent: 'claude' }), smoke_model: 'claude-fable-5' }),
    ).toBe('fable（claude-fable-5）');
    // 真身与别名一样时不重复写一遍
    expect(
      commanderModelLabel({ ...model({ id: 'fable', agent: 'claude' }), smoke_model: 'fable' }),
    ).toBe('fable');
    expect(
      commanderModelLabel(model({ id: 'glm-5.3-flash', agent: 'pi', provider: 'zai' })),
    ).toBe('zai / glm-5.3-flash');
    expect(commanderModelLabel(model({ id: 'gpt-5.6-luna', agent: 'codex' }))).toBe('gpt-5.6-luna');
  });

  it('显示文本带未验证 / 验证失败后缀，与 G1 同一套口径', () => {
    expect(
      commanderModelLabel(
        model({ id: 'glm', agent: 'pi', provider: 'zai', status: '待冒烟', verification: 'unverified' }),
      ),
    ).toBe('zai / glm（未验证）');
    expect(
      commanderModelLabel(
        model({ id: 'ds', agent: 'pi', provider: 'ds', status: '冒烟失败', verification: 'failed' }),
      ),
    ).toBe('ds / ds（验证失败）');
  });

  it('option value 走 JSON 三元组，provider 与 id 带斜杠也认得回来', () => {
    const m = model({ id: 'kimi-code/k3-256k', agent: 'kimi', provider: 'a/b' });
    expect(parseCommanderModelValue(commanderModelValue(m))).toEqual({
      provider: 'a/b',
      model: 'kimi-code/k3-256k',
    });
    expect(parseCommanderModelValue('')).toBeNull();
    expect(parseCommanderModelValue('{"provider":1}')).toBeNull();
  });

  it('picked 与 option 共用固定 key 顺序，select 能选中 provider/model 往返值', () => {
    const picked = { provider: 'a/b', model: 'kimi-code/k3-256k' };
    expect(commanderModelSelectValue(picked)).toBe(
      '{"provider":"a/b","model":"kimi-code/k3-256k"}',
    );
    const fullModel: DispatchableModel = {
      ...model({ id: picked.model, agent: 'kimi', provider: picked.provider }),
      vendor: '', billing_source: '', tier: '', thinking: '', source: '', smoked_at: '',
      smoke_model: '', smoke_failure: '', meta: '', note: '',
    };
    const html = renderToStaticMarkup(
      <CommanderModelSelect picked={picked} models={[fullModel]} onChange={() => {}} />,
    );
    expect(html).toContain('value="{&quot;provider&quot;:&quot;a/b&quot;,&quot;model&quot;:&quot;kimi-code/k3-256k&quot;}" selected=""');
  });

  it('目录名取末级，尾斜杠不算一级', () => {
    expect(cwdName('D:\\work\\aicross')).toBe('aicross');
    expect(cwdName('D:\\work\\aicross\\')).toBe('aicross');
    expect(cwdName('/home/k/aicross/')).toBe('aicross');
    expect(cwdName('')).toBe('');
    expect(cwdName(null)).toBe('');
    expect(cwdName('C:\\')).toBe('C:');
    expect(cwdName('D:/')).toBe('D:');
    expect(cwdName('/')).toBe('/');
  });

  it('默认行文案：启动总指挥 · agent · 模型 · 目录名', () => {
    expect(
      commanderDefaultLine({
        ...emptyCommanderSettings(),
        agent: 'claude',
        model: 'fable',
        cwd: 'D:\\work\\aicross',
      }),
    ).toBe('启动总指挥 · Claude Code · fable · aicross');
    // 模型留空时省掉那一段，不留一个空的点
    expect(
      commanderDefaultLine({ ...emptyCommanderSettings(), agent: 'codex', cwd: '/srv/app' }),
    ).toBe('启动总指挥 · Codex · app');
    // 目录留空时给占位而不是空串
    expect(commanderDefaultLine({ ...emptyCommanderSettings(), agent: 'pi' })).toBe(
      '启动总指挥 · pi agent · 未选目录',
    );
    expect(commanderDefaultLine({
      ...emptyCommanderSettings(), agent: 'pi', provider: 'bigmodel', model: 'glm', cwd: '/work',
    })).toBe('启动总指挥 · pi agent · bigmodel / glm · work');
    // 没存过默认（agent 空）→ null，首屏不画这一行
    expect(commanderDefaultLine(emptyCommanderSettings())).toBeNull();
    expect(commanderDefaultLine(null)).toBeNull();
  });

  it('tab 标题是 agent · 模型 · 目录名，pi 带 provider', () => {
    expect(commanderTabTitle('claude', '', 'fable', 'D:\\work\\aicross')).toBe('Claude Code · fable · aicross');
    expect(commanderTabTitle('codex', '', '', '/srv/app')).toBe('Codex · app');
    expect(commanderTabTitle('pi', 'bigmodel', 'glm', null)).toBe('pi agent · bigmodel / glm · 未选目录');
  });

  it('启动前自检：没选 agent / 选了没装的都拦住', () => {
    expect(commanderBlockReason('claude', ['claude', 'pi'])).toBeNull();
    expect(commanderBlockReason('', ['claude'])).toBe('先选一个 agent');
    expect(commanderBlockReason('codex', ['claude'])).toBe('本机没探测到 Codex');
    expect(commanderBlockReason('grok', ['claude'])).toBe('先选一个 agent');
    expect(commanderBlockReason('claude', ['claude'], '', 'x&calc')).toBe('模型 id 含非法字符');
  });

  it('空白 tab 只有显式进入对话图后才显示“对话图”标题', () => {
    expect(emptyGraphTabTitle(false)).toBe('总指挥');
    expect(emptyGraphTabTitle(true)).toBe('对话图');
    // 已开图的 tab 不受启动器影响：空标题仍是「对话图」，有标题用图标题
    expect(graphTabTitle('g1', '', false)).toBe('对话图');
    expect(graphTabTitle('g1', undefined, false)).toBe('对话图');
    expect(graphTabTitle('g1', '两件小活', false)).toBe('两件小活');
    expect(graphTabTitle('', undefined, false)).toBe('总指挥');
    expect(graphTabTitle(null, undefined, true)).toBe('对话图');
  });

  it('其它 CLI 只列已安装的非总指挥工具，点击沿原工具路径转发', () => {
    const catalog = [
      { key: 'claude', label: 'Claude Code' },
      { key: 'opencode', label: 'OpenCode' },
      { key: 'aider', label: 'Aider' },
    ];
    expect(otherInstalledCommanderClis(catalog, { claude: true, opencode: true, aider: false }))
      .toEqual([{ key: 'opencode', label: 'OpenCode' }]);
    const onOpen = vi.fn();
    const menu = CommanderOtherCliMenu({
      otherClis: [{ key: 'opencode', label: 'OpenCode' }],
      onOpenTool: onOpen,
    });
    const menuBody = Array.isArray(menu?.props.children) ? menu.props.children[1] : null;
    const button = Array.isArray(menuBody?.props.children)
      ? menuBody.props.children[0]
      : menuBody?.props.children;
    button.props.onClick();
    expect(onOpen).toHaveBeenCalledWith('opencode');
  });

  it('资源库空模型提示必须等模型清单加载完成', () => {
    expect(shouldShowNoCommanderModels(false, null, 'claude', 0)).toBe(false);
    expect(shouldShowNoCommanderModels(true, null, 'claude', 0)).toBe(true);
    expect(shouldShowNoCommanderModels(true, '读取失败', 'claude', 0)).toBe(false);
  });

  it('用户点 agent 卡立即锁定初值，晚到默认不能再 seed', () => {
    const seeded = { current: false };
    let selected = '';
    markCommanderSelection(seeded, (value) => { selected = value; }, 'codex');
    expect(seeded.current).toBe(true);
    expect(selected).toBe('codex');
  });

  it('目录快捷入口只在扫描完成且默认 CLI 已安装时可用', () => {
    const defaults = { ...emptyCommanderSettings(), agent: 'pi', model: 'glm' };
    expect(commanderDefaultAvailable(defaults, {}, false)).toBe(false);
    expect(commanderDefaultAvailable(defaults, { pi: false }, true)).toBe(false);
    expect(commanderDefaultAvailable(defaults, { pi: true }, true)).toBe(true);
  });

  it('保存成功消费快照后，组件重启路径不再生成 saveCommander 回调', () => {
    const settings = { ...emptyCommanderSettings(), agent: 'claude', model: 'fable' };
    const saveCommander = vi.fn();
    const first = commanderStartedCallback(
      { id: 'tab-a', commanderSettings: settings },
      (_id, value) => saveCommander(value),
    );
    first?.();
    expect(saveCommander).toHaveBeenCalledOnce();
    const afterConsume = commanderStartedCallback({ id: 'tab-b' }, (_id, value) => saveCommander(value));
    expect(afterConsume).toBeUndefined();
    expect(saveCommander).toHaveBeenCalledOnce();
  });

  it('agent 显示名与接入页一致', () => {
    expect(commanderAgentLabel('claude')).toBe('Claude Code');
    expect(commanderAgentLabel('kimi')).toBe('Kimi Code');
    expect(commanderAgentLabel('pi')).toBe('pi agent');
    expect(commanderAgentLabel('grok')).toBe('grok');
  });
});

describe('TASK-10 (3) 启动器组件', () => {
  const render = (
    installed: Record<string, boolean>,
    defaults = emptyCommanderSettings(),
    toolsScanned = true,
    otherClis: { key: string; label: string }[] = [],
  ) =>
    renderToStaticMarkup(
      <CommanderLauncher
        installed={installed}
        toolsScanned={toolsScanned}
        otherClis={otherClis}
        defaults={defaults}
        folderPath="D:\\work\\aicross"
        onLaunch={() => {}}
        onOpenTerminal={() => {}}
        onOpenGraph={() => {}}
        onOpenTool={() => {}}
      />,
    );

  it('未扫描显示探测中；扫描完成且为空才显示没探测到', () => {
    const loading = render({}, emptyCommanderSettings(), false);
    expect(loading).toContain('探测中…');
    expect(loading).not.toContain('本机没探测到');
    expect(loading).not.toContain('启动总指挥 ·');

    const html = render({}, emptyCommanderSettings(), true);
    expect(html).toContain('本机没探测到');
    expect(html).not.toContain('启动总指挥');
  });

  it('装了就画 agent 卡与启动按钮', () => {
    const html = render({ claude: true, pi: true });
    expect(html).toContain('Claude Code');
    expect(html).toContain('启动总指挥');
    // 没装的那两家不出现
    expect(html).not.toContain('Codex');
  });

  it('次级入口保留系统终端与对话图', () => {
    const html = render({ claude: true });
    expect(html).toContain('系统终端');
    expect(html).toContain('对话图');
  });

  it('有其它已装 CLI 时出现第三项与工具名', () => {
    const html = render(
      { claude: true, opencode: true },
      emptyCommanderSettings(),
      true,
      [{ key: 'opencode', label: 'OpenCode' }],
    );
    expect(html).toContain('其它 CLI');
    expect(html).toContain('OpenCode');
  });

  it('没存过默认时不画一键行', () => {
    expect(render({ claude: true })).not.toContain('启动总指挥 ·');
  });

  it('默认 agent 已卸载时不画一键行', () => {
    const defaults = {
      ...emptyCommanderSettings(),
      agent: 'claude',
      model: 'fable',
      cwd: 'D:\\work\\aicross',
    };
    expect(render({ codex: true }, defaults)).not.toContain('启动总指挥 ·');
  });

  it('TASK-15：卡片网格固定列宽和行高，窄窗口可换行', async () => {
    // tsconfig 未引入 Node 类型；Vitest 本身运行在 Node，动态读取避免把测试工具
    // 变成产品构建依赖，同时能真正钉住 CSS 是否消费网格变量。
    // @ts-expect-error Node built-in is only loaded by Vitest.
    const { readFileSync } = await import('node:fs');
    const centerPanelCss = readFileSync(
      new URL('../../components/center/CenterPanel.css', import.meta.url),
      'utf8',
    );
    expect(centerPanelCss).toMatch(/\.commander-agents\s*\{[^}]*display:\s*grid[^}]*grid-template-columns:\s*var\(--commander-agent-grid\)/s);
    expect(centerPanelCss).toMatch(/\.commander-agent-card\s*\{[^}]*height:\s*var\(--commander-agent-card-height\)/s);
    const pickedRule = centerPanelCss.match(/\.commander-agent-card\.is-picked\s*\{([^}]*)\}/s)?.[1] ?? '';
    expect(pickedRule).toMatch(/border-color:\s*var\(--accent\)/);
    expect(pickedRule).not.toMatch(/(?:^|[;\s])(width|padding|border-width)\s*:/);
    expect(COMMANDER_AGENT_GRID_TEMPLATE).toBe('repeat(auto-fit, minmax(150px, 150px))');
    expect(COMMANDER_AGENT_CARD_HEIGHT).toBe(48);
    const html = render({ claude: true, codex: true, kimi: true, pi: true });
    expect(html).toContain('--commander-agent-grid:repeat(auto-fit, minmax(150px, 150px))');
    expect(html).toContain('--commander-agent-card-height:48px');
  });

  it('TASK-15-R1：前端显示名与 Rust 常量逐项一致', async () => {
    // @ts-expect-error Node built-in is only loaded by Vitest.
    const { readFileSync } = await import('node:fs');
    const rust = readFileSync(new URL('../../../../src/resources.rs', import.meta.url), 'utf8');
    const names = Object.fromEntries(
      [...rust.matchAll(/const AGENT_NAME_(CLAUDE|CODEX|KIMI|PI):\s*&str\s*=\s*"([^"]+)"/g)]
        .map((match) => [match[1].toLowerCase(), match[2]]),
    );
    expect(names).toEqual(AGENT_DISPLAY_NAMES);
  });

  it('TASK-15-R1：CenterPanel 真实接入 tab 状态机与建 tab 直启', async () => {
    // @ts-expect-error Node built-in is only loaded by Vitest.
    const { readFileSync } = await import('node:fs');
    const source = readFileSync(new URL('../../components/center/CenterPanel.tsx', import.meta.url), 'utf8');
    expect(source).toContain('commanderNewTabLaunch(');
    expect(source).toContain("type: 'launcher-shown'");
    expect(source).toContain("type: 'availability'");
    expect(source).toContain("type: 'changed'");
    expect(source).not.toContain('commanderLauncherToolPatch');
  });
});

describe('TASK-15-R1 复审：启动器只在可能直启时才不画', () => {
  const ok: CommanderSettings = { version: 1, agent: 'claude', provider: '', model: 'fable', cwd: 'C:/x', updated_at: '' };
  const none: CommanderSettings = { ...ok, agent: '' };
  it('默认未读到 → 不画（等一拍）；读到但没有默认 → 立刻画，探测期间也画', () => {
    expect(commanderLauncherHidden(null, {}, false, false)).toBe(true);
    expect(commanderLauncherHidden(none, {}, false, false)).toBe(false);
    expect(commanderLauncherHidden(none, { claude: true }, true, false)).toBe(false);
  });
  it('默认有效：探测未完成不画；探测完可用且 tab 未登记不画；已登记或不可用则画', () => {
    expect(commanderLauncherHidden(ok, {}, false, false)).toBe(true);
    expect(commanderLauncherHidden(ok, { claude: true }, true, false)).toBe(true);
    expect(commanderLauncherHidden(ok, { claude: true }, true, true)).toBe(false);
    expect(commanderLauncherHidden(ok, { claude: false }, true, false)).toBe(false);
  });
});
