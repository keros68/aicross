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
  markCommanderSelection,
  otherInstalledCommanderClis,
  parseCommanderModelValue,
  shouldShowNoCommanderModels,
  commanderStartedCallback,
} from '../launcher';
import { emptyCommanderSettings } from '../types';
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
    ).toBe('启动总指挥 · claude · fable · aicross');
    // 模型留空时省掉那一段，不留一个空的点
    expect(
      commanderDefaultLine({ ...emptyCommanderSettings(), agent: 'codex', cwd: '/srv/app' }),
    ).toBe('启动总指挥 · codex · app');
    // 目录留空时给占位而不是空串
    expect(commanderDefaultLine({ ...emptyCommanderSettings(), agent: 'pi' })).toBe(
      '启动总指挥 · pi · 未选目录',
    );
    // 没存过默认（agent 空）→ null，首屏不画这一行
    expect(commanderDefaultLine(emptyCommanderSettings())).toBeNull();
    expect(commanderDefaultLine(null)).toBeNull();
  });

  it('tab 标题是 agent · 模型 · 目录名', () => {
    expect(commanderTabTitle('claude', 'fable', 'D:\\work\\aicross')).toBe('claude · fable · aicross');
    expect(commanderTabTitle('codex', '', '/srv/app')).toBe('codex · app');
    expect(commanderTabTitle('pi', 'glm', null)).toBe('pi · glm · 未选目录');
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
    expect(commanderAgentLabel('kimi')).toBe('Kimi Code CLI');
    expect(commanderAgentLabel('pi')).toBe('pi');
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
});
