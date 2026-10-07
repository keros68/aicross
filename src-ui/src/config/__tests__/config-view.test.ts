import { describe, expect, it } from 'vitest';
import {
  DETECT_STALE_HINT,
  DETECT_STALE_MS,
  canSmoke,
  detectStatus,
  flowColumns,
  missingModelsText,
  needsFirstDetect,
  initialOf,
  AGENT_OPEN_STORAGE_KEY,
  agentExpandable,
  agentHomepage,
  agentStats,
  isMissing,
  missingCount,
  modelNote,
  pruneTitle,
  referencedText,
  stateLabel,
  isAgentOpen,
  isNativeAgent,
  isUnavailable,
  isInstalled,
  isUntested,
  notInstalledText,
  readAgentOpen,
  writeAgentOpen,
  MODEL_PREVIEW_LIMIT,
  moreModelsText,
  previewModels,
  seatRowState,
  groupByProvider,
  modeIsReady,
  modesOverview,
  resourcesOverview,
  optionsWithCurrent,
  resolvedModelText,
  seatDraft,
  seatFromDraft,
  seatInvalidState,
  seatKey,
  seatOptions,
  seatStatus,
  sourceText,
  splitInstalled,
  isConfirmedInstalled,
  splitMissingModels,
  stateTone,
} from '../config-view';
import type { ModeEntry, ModeSeatNode, ResourcesOutput, Seat } from '../../tauri';

const resources: ResourcesOutput = {
  agents: [
    {
      id: 'claude', name: 'Claude Code', version: '2.1.270', state: '可用', note: '',
      models: [
        { id: 'sonnet', agent: 'claude', provider: '', resolved_id: 'claude-sonnet-5', state: '可用', note: '' },
      ],
    },
    {
      id: 'pi', name: 'pi agent', version: '0.86.1', state: '可用', note: '',
      models: [
        { id: 'glm-4.7', agent: 'pi', provider: 'zai', resolved_id: 'glm-4.7', state: '已验证', note: '' },
        { id: 'bad', agent: 'pi', provider: 'zai', resolved_id: 'bad', state: '验证失败', note: '403' },
        { id: 'glm-4.7', agent: 'pi', provider: 'bigmodel', resolved_id: 'glm-4.7', state: '未登录', note: '' },
        { id: 'glm-5.3', agent: 'pi', provider: 'zai', resolved_id: 'glm-5.3', state: '未验证', note: '' },
      ],
    },
    {
      id: 'kimi', name: 'Kimi Code', version: '未检测', state: '未安装', note: '安装后运行 aicross resources detect',
      models: [
        { id: 'k2', agent: 'kimi', provider: '', resolved_id: 'k2', state: '未安装', note: '' },
      ],
    },
  ],
};

function node(overrides: Partial<ModeSeatNode> = {}): ModeSeatNode {
  return {
    role: '验收', visibility: '盲', write: '只读', seat: null, effective: null, source: '未指定',
    ...overrides,
  };
}

const glmSeat: Seat = {
  agent: 'pi', provider: 'zai', model: 'glm-4.7', tier: '', thinking: '中',
  fallback: { agent: 'claude', provider: null, model: 'sonnet' },
};

describe('人选下拉的选项', () => {
  it('只列 agent 状态为可用的模型', () => {
    const options = seatOptions(resources);
    expect(options.map((o) => o.agent)).not.toContain('kimi');
    expect(options).toHaveLength(5);
  });

  it('验证失败、未登录的选项禁用，其余可选', () => {
    const disabled = Object.fromEntries(seatOptions(resources).map((o) => [`${o.model}@${o.provider}:${o.state}`, o.disabled]));
    expect(disabled['bad@zai:验证失败']).toBe(true);
    expect(disabled['glm-4.7@bigmodel:未登录']).toBe(true);
    expect(disabled['glm-4.7@zai:已验证']).toBe(false);
    expect(disabled['glm-5.3@zai:未验证']).toBe(false);
  });

  it('原生模型 provider 为 null，第三方带 provider，状态照资源', () => {
    const [sonnet] = seatOptions(resources);
    expect(sonnet).toMatchObject({ agent: 'claude', model: 'sonnet', provider: null, state: '可用' });
    const zai = seatOptions(resources).find((o) => o.model === 'glm-4.7' && o.provider === 'zai');
    expect(zai).toMatchObject({ agent: 'pi', state: '已验证' });
  });

  it('同模型挂在不同 provider 下是不同选项', () => {
    const keys = seatOptions(resources).map((o) => o.key);
    expect(new Set(keys).size).toBe(keys.length);
  });

  it('没有资源时为空', () => {
    expect(seatOptions(null)).toEqual([]);
    expect(seatOptions({ agents: [] })).toEqual([]);
  });

  it('当前值不在列表里时补一个禁用项', () => {
    const options = seatOptions(resources);
    const key = seatKey('kimi', null, 'k2');
    const shown = optionsWithCurrent(options, '');
    const merged = optionsWithCurrent(options, key);
    expect(merged).toHaveLength(shown.length + 1);
    expect(merged.at(-1)).toMatchObject({ key, agent: 'kimi', provider: null, model: 'k2', state: '不可用', disabled: true });
    expect(optionsWithCurrent(options, options[0].key)).toEqual(shown);
  });

  it('下拉不列未验证与选不上的模型，当前人选照常列出', () => {
    const option = (model: string, state: string, disabled = false) => ({
      key: seatKey('pi', 'zai', model), agent: 'pi', provider: 'zai', model, state, disabled,
    });
    const all = [option('a', '已验证'), option('b', '未验证'), option('c', '未发现', true), option('d', '可用')];
    expect(optionsWithCurrent(all, '').map((o) => o.model)).toEqual(['a', 'd']);
    expect(optionsWithCurrent(all, all[1].key).map((o) => o.model)).toEqual(['a', 'b', 'd']);
    expect(optionsWithCurrent(all, all[1].key)[1]).toMatchObject({ state: '未验证' });
    expect(optionsWithCurrent(all, all[2].key).map((o) => o.model)).toEqual(['a', 'c', 'd']);
  });
});

describe('位置卡片的来源文字', () => {
  it('四种来源各有说明', () => {
    expect(sourceText('位置')).toBe('本模式');
    expect(sourceText('角色')).toBe('角色默认');
    expect(sourceText('预设')).toBe('预设固定');
    expect(sourceText('未指定')).toBe('未指定');
  });
});

describe('流程图排布', () => {
  it('阶段成列、位置成行，编号从 1 开始', () => {
    const a = node({ role: '执行' });
    const b = node({ role: '验收' });
    const c = node({ role: '验证' });
    const d = node({ role: '仲裁' });
    const columns = flowColumns([[a], [b, c], [d]]);
    expect(columns.map((col) => col.stage)).toEqual([1, 2, 3]);
    expect(columns[1].seats.map((s) => [s.stage, s.index, s.node.role])).toEqual([
      [2, 1, '验收'],
      [2, 2, '验证'],
    ]);
    expect(columns[2].seats[0]).toMatchObject({ stage: 3, index: 1 });
  });

  it('空阶段不出列，后面阶段的编号不变', () => {
    const columns = flowColumns([[node()], [], [node()]]);
    expect(columns.map((col) => col.stage)).toEqual([1, 3]);
  });

  it('没有阶段时为空', () => {
    expect(flowColumns([])).toEqual([]);
  });
});

describe('状态词的展示', () => {
  it('验证失败、未登录、额度用尽、未安装用异常胶囊', () => {
    for (const state of ['验证失败', '未登录', '额度用尽', '未安装']) {
      expect(stateTone(state)).toBe('alert');
    }
  });

  it('其余状态纯文字', () => {
    for (const state of ['可用', '已验证', '未验证', '冒烟超时', '']) {
      expect(stateTone(state)).toBe('plain');
    }
  });

  it('未验证、验证失败、冒烟超时给测试连接按钮', () => {
    expect(canSmoke({ state: '未验证' })).toBe(true);
    expect(canSmoke({ state: '验证失败' })).toBe(true);
    expect(canSmoke({ state: '冒烟超时' })).toBe(true);
    expect(canSmoke({ state: '已验证' })).toBe(false);
    expect(canSmoke({ state: '可用' })).toBe(false);
    expect(canSmoke({ state: '未登录' })).toBe(false);
  });

  it('实际模型与 id 不同才显示', () => {
    expect(resolvedModelText({ id: 'sonnet', resolved_id: 'claude-sonnet-5' })).toBe('claude-sonnet-5');
    expect(resolvedModelText({ id: 'glm-4.7', resolved_id: 'glm-4.7' })).toBe('');
    expect(resolvedModelText({ id: 'x', resolved_id: '' })).toBe('');
  });
});

describe('编辑框与人选对象', () => {
  it('初值取本模式人选，往返后与原人选一致', () => {
    const draft = seatDraft(node({ seat: glmSeat, effective: glmSeat, source: '位置' }));
    expect(seatFromDraft(draft)).toEqual(glmSeat);
  });

  it('没有本模式人选时取实际人选，档位为空串', () => {
    const effective: Seat = { ...glmSeat, tier: 'sonnet', fallback: null };
    const draft = seatDraft(node({ effective, source: '角色' }));
    expect(draft.tier).toBe('');
    expect(seatFromDraft(draft)).toEqual({ ...effective, tier: '' });
  });

  it('未指定时没选人选，生成不出人选对象', () => {
    const draft = seatDraft(node());
    expect(draft.personKey).toBe('');
    expect(seatFromDraft(draft)).toBeNull();
  });

  it('选了下拉项：原生模型 provider 为 null，备选不设为 null', () => {
    const [sonnet] = seatOptions(resources);
    expect(seatFromDraft({ personKey: sonnet.key, thinking: '', fallbackKey: '', tier: '' })).toEqual({
      agent: 'claude', provider: null, model: 'sonnet', tier: '', thinking: '', fallback: null,
    });
  });
});

describe('模式卡片的位置行（TASK-49、TASK-51、fix-51-r1）', () => {
  const effective = (model: string, state?: string, agent = 'claude'): ModeSeatNode['effective'] => (
    state === undefined ? { ...glmSeat, agent, model } : { ...glmSeat, agent, model, state }
  );

  it('未指定：下拉显示未选择并标异常，没有失效状态', () => {
    expect(seatRowState(node({ role: '验收' }))).toEqual({ missing: true, invalid: '', recommended: false, presetFixed: false });
  });

  it('人选失效：未发现、验证失败、未登录、不在资源库标出状态', () => {
    for (const state of ['未发现', '验证失败', '未登录', '不在资源库']) {
      const seatNode = node({ role: '验收', effective: effective('deepseek-v4-flash', state, 'pi'), source: '位置' });
      expect(seatInvalidState(seatNode)).toBe(state);
      expect(seatStatus(seatNode)).toEqual({ alert: true, label: `人选失效：${state}` });
      expect(seatRowState(seatNode)).toMatchObject({ missing: false, invalid: state });
    }
  });

  it('正常状态与缺省 state 不算失效；位置卡片的来源文字照旧', () => {
    for (const state of ['已验证', '可用', '未验证', undefined]) {
      const seatNode = node({ effective: effective('haiku', state), source: '角色' });
      expect(seatInvalidState(seatNode)).toBe('');
      expect(seatStatus(seatNode)).toEqual({ alert: false, label: '角色默认' });
      expect(seatRowState(seatNode).invalid).toBe('');
    }
    expect(seatStatus(node())).toEqual({ alert: true, label: '未指定' });
  });

  it('本模式人选由一键推荐填的带「推荐」标记，手选、旧数据与角色默认的不带（TASK-51）', () => {
    const recommended = node({ effective: effective('haiku'), seat: { ...glmSeat, origin: 'recommended' }, source: '位置' });
    const manual = node({ effective: effective('haiku'), seat: { ...glmSeat, origin: 'manual' }, source: '位置' });
    const legacy = node({ effective: effective('haiku'), seat: { ...glmSeat }, source: '位置' });
    const byRole = node({ effective: effective('haiku'), source: '角色' });
    expect([recommended, manual, legacy, byRole].map((item) => seatRowState(item).recommended)).toEqual([true, false, false, false]);
  });

  it('预设固定的位置锁定', () => {
    const preset = node({ effective: effective('haiku'), seat: { ...glmSeat, origin: 'recommended' }, source: '预设' });
    expect(seatRowState(preset)).toEqual({ missing: false, invalid: '', recommended: false, presetFixed: true });
  });
});

describe('不可用与折叠（fix-51-r2）', () => {
  it('不可用 = 未发现 + 验证失败 + 未登录 + 额度用尽', () => {
    for (const state of ['未发现', '验证失败', '未登录', '额度用尽']) expect(isUnavailable({ state })).toBe(true);
    for (const state of ['可用', '已验证', '未验证', '冒烟超时', '未安装']) expect(isUnavailable({ state })).toBe(false);
  });

  it('默认：有不可用模型的卡片展开，其余收起；记住过就按记忆', () => {
    expect(isAgentOpen(resources.agents[1], {})).toBe(true);
    expect(isAgentOpen(resources.agents[0], {})).toBe(false);
    expect(isAgentOpen(resources.agents[1], { pi: false })).toBe(false);
    expect(isAgentOpen(resources.agents[0], { claude: true })).toBe(true);
  });

  it('折叠记忆写入后读回；读到坏数据或存储抛错时为空，不向外抛', () => {
    const data: Record<string, string> = {};
    const storage = { getItem: (key: string) => data[key] ?? null, setItem: (key: string, value: string) => { data[key] = value; } };
    writeAgentOpen(storage, { claude: true, pi: false });
    expect(data[AGENT_OPEN_STORAGE_KEY]).toBe('{"claude":true,"pi":false}');
    expect(readAgentOpen(storage)).toEqual({ claude: true, pi: false });
    data[AGENT_OPEN_STORAGE_KEY] = '{"claude":"yes","pi":false}';
    expect(readAgentOpen(storage)).toEqual({ pi: false });
    data[AGENT_OPEN_STORAGE_KEY] = 'not json';
    expect(readAgentOpen(storage)).toEqual({});
    expect(readAgentOpen(null)).toEqual({});
    const throwing = { getItem: () => { throw new Error('denied'); }, setItem: () => { throw new Error('denied'); } };
    expect(readAgentOpen(throwing)).toEqual({});
    expect(() => writeAgentOpen(throwing, {})).not.toThrow();
  });
});

describe('资源卡片直接列模型（fix-51-r1）', () => {
  const many = Array.from({ length: 11 }, (_, i) => (
    { id: `m${i}`, agent: 'pi', provider: i % 2 === 0 ? 'zai' : 'opencode-go', resolved_id: '', state: '可用', note: '' }
  ));

  it('超过 8 条时收起其余并计数；顺序先按 provider 分组', () => {
    const { shown, hidden } = previewModels(many, false);
    expect(shown).toHaveLength(MODEL_PREVIEW_LIMIT);
    expect(hidden).toBe(3);
    expect(shown.slice(0, 6).every((model) => model.provider === 'zai')).toBe(true);
    expect(moreModelsText(hidden)).toBe('展开其余 3 个');
  });

  it('展开或不超过 8 条时全部列出', () => {
    expect(previewModels(many, true)).toMatchObject({ hidden: 0 });
    expect(previewModels(many, true).shown).toHaveLength(11);
    expect(previewModels(many.slice(0, 8), false)).toMatchObject({ hidden: 0 });
  });
});

describe('未探测时模式页提示（fix-49-r1）', () => {
  it('资源从未探测（null、缺字段、空串）时提示', () => {
    expect(needsFirstDetect({ agents: [], detected_at: null })).toBe(true);
    expect(needsFirstDetect({ agents: [] })).toBe(true);
    expect(needsFirstDetect({ agents: [], detected_at: ' ' })).toBe(true);
  });

  it('探测过或资源还没读到时不提示', () => {
    expect(needsFirstDetect({ agents: [], detected_at: '2026-10-06T08:00:00Z' })).toBe(false);
    expect(needsFirstDetect(null)).toBe(false);
    expect(needsFirstDetect(undefined)).toBe(false);
  });
});

describe('概览卡（fix-49-r2）', () => {
  it('资源：已安装 agent 数、可用模型（下拉里可选的）、已验证、不可用（非 0 时可点筛选）', () => {
    expect(resourcesOverview(resources)).toEqual([
      { label: 'agent', value: '2', alert: false },
      { label: '可用模型', value: '3', alert: false },
      { label: '已验证', value: '1', alert: false },
      { label: '不可用', value: '2', alert: true, filter: 'unavailable' },
    ]);
  });

  it('资源：不可用为 0 时常规色且不可点；没有资源时全 0', () => {
    const clean: ResourcesOutput = { agents: [resources.agents[0]] };
    expect(resourcesOverview(clean).at(-1)).toEqual({ label: '不可用', value: '0', alert: false });
    expect(resourcesOverview(null).map((item) => item.value)).toEqual(['0', '0', '0', '0']);
  });

  it('模式：默认模式名、已配好 / 总数、待处理位置', () => {
    const ok = (id: string, name: string, stages: ModeSeatNode[][]): ModeEntry => ({ id, name, builtin: true, default: false, stages });
    const config = {
      defaultMode: 'builtin-economy',
      modes: [
        ok('builtin-economy', '经济模式', [[node({ effective: glmSeat, source: '位置' })]]),
        ok('builtin-blind', '盲审模式', [[node({ effective: { ...glmSeat, state: '未发现' }, source: '位置' }), node()]]),
        ok('builtin-arena', '擂台模式', []),
      ],
    };
    expect(modeIsReady(config.modes[0])).toBe(true);
    expect(modeIsReady(config.modes[1])).toBe(false);
    expect(modeIsReady(config.modes[2])).toBe(false);
    expect(modesOverview(config)).toEqual([
      { label: '默认模式', value: '经济模式', alert: false },
      { label: '已配好', value: '1 / 3', alert: false },
      { label: '待处理位置', value: '2', alert: true },
    ]);
  });

  it('模式：没有待处理时常规色；读不到默认时写「未设置」', () => {
    expect(modesOverview({ defaultMode: 'x', modes: [] })).toEqual([
      { label: '默认模式', value: '未设置', alert: false },
      { label: '已配好', value: '0 / 0', alert: false },
      { label: '待处理位置', value: '0', alert: false },
    ]);
  });
});

describe('资源卡片的标识与小统计（fix-49-r1/r2）', () => {
  it('首字母取名称第一个字，拉丁字母转大写', () => {
    expect(initialOf('pi agent')).toBe('P');
    expect(initialOf('  Claude Code')).toBe('C');
    expect(initialOf('经济模式')).toBe('经');
    expect(initialOf('')).toBe('');
  });

  it('第三方：「N 个模型 · X 已验证 · Y 不可用」，Y 非 0 时标异常（fix-51-r2）', () => {
    expect(agentStats({ models: resources.agents[1].models, state: '可用' })).toEqual([
      { text: '4 个模型', alert: false },
      { text: '1 已验证', alert: false },
      { text: '2 不可用', alert: true },
    ]);
  });

  it('第三方没有不可用时省略那一段', () => {
    const ok = resources.agents[1].models.filter((model) => model.state === '已验证' || model.state === '未验证');
    expect(agentStats({ models: ok, state: '可用' })).toEqual([
      { text: '2 个模型', alert: false },
      { text: '1 已验证', alert: false },
    ]);
  });

  it('原生：「N 个模型 · 原生」，不写已验证（fix-51-r2）', () => {
    expect(isNativeAgent(resources.agents[0])).toBe(true);
    expect(agentStats({ models: resources.agents[0].models, state: '可用' })).toEqual([
      { text: '1 个模型', alert: false },
      { text: '原生', alert: false },
    ]);
    expect(isNativeAgent({ models: [] })).toBe(false);
  });

  it('agent 不可用时末尾加上它的状态并标异常', () => {
    expect(agentStats({ models: [], state: '未安装' }).at(-1)).toEqual({ text: '未安装', alert: true });
  });

  it('模型按 provider 分组，组序按首次出现，原生一组叫「原生」', () => {
    const groups = groupByProvider([
      ...resources.agents[1].models,
      { id: 'big-pickle', agent: 'pi', provider: 'opencode-go', resolved_id: '', state: '未验证', note: '' },
      { id: 'x', agent: 'pi', provider: '', resolved_id: '', state: '可用', note: '' },
    ]);
    expect(groups.map((group) => [group.label, group.models.map((model) => model.id)])).toEqual([
      ['zai', ['glm-4.7', 'bad', 'glm-5.3']],
      ['bigmodel', ['glm-4.7']],
      ['opencode-go', ['big-pickle']],
      ['原生', ['x']],
    ]);
    expect(groupByProvider([])).toEqual([]);
  });
});

describe('资源卡片（TASK-49）', () => {
  const models = [
    { id: 'a', agent: 'pi', provider: 'zai', resolved_id: 'a', state: '已验证', note: '' },
    { id: 'b', agent: 'pi', provider: 'deepseek', resolved_id: 'b', state: '未发现', note: '' },
    { id: 'c', agent: 'pi', provider: 'qwen', resolved_id: 'c', state: '未发现', note: '' },
    { id: 'd', agent: 'pi', provider: 'zai', resolved_id: 'd', state: '未验证', note: '' },
  ];

  it('未发现的模型单独一组并计数，其余照常列出', () => {
    const { listed, missing } = splitMissingModels(models);
    expect(listed.map((m) => m.id)).toEqual(['a', 'd']);
    expect(missing.map((m) => m.id)).toEqual(['b', 'c']);
    expect(missingModelsText(missing.length)).toBe('2 个模型未发现');
    expect(splitMissingModels([]).missing).toEqual([]);
  });

  it('未发现按异常状态显示，不给测试连接按钮', () => {
    expect(stateTone('未发现')).toBe('alert');
    expect(canSmoke({ state: '未发现' })).toBe(false);
  });

  it('上次探测：缺字段或 null 为尚未探测，不提示', () => {
    expect(detectStatus(undefined, Date.now())).toEqual({ label: '尚未探测', stale: false });
    expect(detectStatus(null, Date.now())).toEqual({ label: '尚未探测', stale: false });
    expect(detectStatus('', Date.now())).toEqual({ label: '尚未探测', stale: false });
  });

  // 预期值都按本地时区算：用本地时间构造探测时刻，避免 UTC 与本地时区差出时分或日期。
  const pad = (n: number) => String(n).padStart(2, '0');
  const local = (time: number) => {
    const d = new Date(time);
    return { day: `${pad(d.getMonth() + 1)}-${pad(d.getDate())}`, clock: `${pad(d.getHours())}:${pad(d.getMinutes())}`, year: d.getFullYear() };
  };

  it('上次探测：7 天内不提示，超过 7 天提示重新探测', () => {
    const base = new Date(2026, 9, 1, 8, 30).getTime();
    const detectedAt = new Date(base).toISOString();
    const recent = detectStatus(detectedAt, base + DETECT_STALE_MS - 1000);
    expect(recent.stale).toBe(false);
    expect(recent.label).toBe(`上次探测 ${local(base).day} ${local(base).clock}`);
    expect(detectStatus(detectedAt, base + DETECT_STALE_MS + 1000).stale).toBe(true);
    expect(DETECT_STALE_HINT).toBe('资源超过 7 天未探测');
  });

  it('上次探测：当天只显示时分，跨年带年份（fix-49-r3）', () => {
    const at = new Date(2026, 9, 6, 9, 5).getTime();
    const iso = new Date(at).toISOString();
    const { day, clock, year } = local(at);
    expect(detectStatus(iso, at + 60 * 60 * 1000).label).toBe(`上次探测 ${clock}`);
    expect(detectStatus(iso, new Date(2026, 9, 8, 9, 0).getTime()).label).toBe(`上次探测 ${day} ${clock}`);
    expect(detectStatus(iso, new Date(2027, 0, 2).getTime()).label).toBe(`上次探测 ${year}-${day} ${clock}`);
  });

  it('上次探测时间解析不出来时原样显示，不提示', () => {
    expect(detectStatus('不是时间', Date.now())).toEqual({ label: '上次探测 不是时间', stale: false });
  });
});

describe('人选下拉禁用未发现（TASK-49）', () => {
  it('未发现的模型在下拉里禁用', () => {
    const withMissing: ResourcesOutput = {
      agents: [{
        id: 'pi', name: 'pi agent', version: '1.0.2', state: '可用', note: '',
        models: [
          { id: 'deepseek-v4-flash', agent: 'pi', provider: 'deepseek', resolved_id: '', state: '未发现', note: '' },
          { id: 'glm-4.7', agent: 'pi', provider: 'zai', resolved_id: '', state: '已验证', note: '' },
        ],
      }],
      detected_at: null,
    };
    const disabled = Object.fromEntries(seatOptions(withMissing).map((o) => [o.model, o.disabled]));
    expect(disabled['deepseek-v4-flash']).toBe(true);
    expect(disabled['glm-4.7']).toBe(false);
  });
});

describe('文案、折叠与清理（fix-51-r3）', () => {
  it('引擎的「冒烟超时」显示为「连接超时」，其余原样；下拉的状态也用显示名', () => {
    expect(stateLabel('冒烟超时')).toBe('连接超时');
    expect(stateLabel('已验证')).toBe('已验证');
    const timeout: ResourcesOutput = {
      agents: [{ id: 'pi', name: 'pi', version: '', state: '可用', note: '', models: [
        { id: 'm', agent: 'pi', provider: 'zai', resolved_id: '', state: '冒烟超时', note: '' },
      ] }],
    };
    expect(seatOptions(timeout)[0]).toMatchObject({ state: '连接超时' });
  });

  it('未发现的模型行、agent 不可用时的模型行不显示说明，其余原样', () => {
    expect(modelNote({ state: '未发现', note: '最近一次探测没有发现这个模型' })).toBe('');
    expect(modelNote({ state: '未安装', note: '安装后重新探测', missing: true })).toBe('');
    expect(modelNote({ state: '未安装', note: '安装后重新探测' }, '未安装')).toBe('');
    expect(modelNote({ state: '验证失败', note: ' 403 ' })).toBe('403');
    expect(modelNote({ state: '验证失败', note: ' 403 ' }, '可用')).toBe('403');
  });

  it('未发现以 missing 字段为准；字段缺失时退回看状态', () => {
    expect(isMissing({ state: '未安装', missing: true })).toBe(true);
    expect(isMissing({ state: '未发现', missing: false })).toBe(false);
    expect(isMissing({ state: '未发现' })).toBe(true);
    expect(isMissing({ state: '可用' })).toBe(false);
  });

  it('未发现计数与分组按 missing 字段（agent 不可用时状态是未安装也算）', () => {
    const models = [
      { id: 'a', agent: 'kimi', provider: '', resolved_id: '', state: '未安装', note: '', missing: true },
      { id: 'b', agent: 'kimi', provider: '', resolved_id: '', state: '未安装', note: '', missing: false },
    ];
    const res: ResourcesOutput = { agents: [{ id: 'kimi', name: 'Kimi Code', version: '', state: '未安装', note: '', models }] };
    expect(missingCount(res)).toBe(1);
    expect(missingCount(res, 'kimi')).toBe(1);
    const { listed, missing } = splitMissingModels(models);
    expect(listed.map((model) => model.id)).toEqual(['b']);
    expect(missing.map((model) => model.id)).toEqual(['a']);
  });

  it('有模型、有说明或 agent 不可用时卡片可展开；都没有时不可展开', () => {
    expect(agentExpandable({ models: resources.agents[0].models, note: '', state: '可用' })).toBe(true);
    expect(agentExpandable({ models: [], note: '', state: '未安装' })).toBe(true);
    expect(agentExpandable({ models: [], note: '探测失败：超时', state: '可用' })).toBe(true);
    expect(agentExpandable({ models: [], note: ' ', state: '可用' })).toBe(false);
  });

  it('未发现计数：全部或指定 agent', () => {
    const withMissing: ResourcesOutput = {
      agents: [
        { id: 'pi', name: 'pi', version: '', state: '可用', note: '', models: [
          { id: 'a', agent: 'pi', provider: 'deepseek', resolved_id: '', state: '未发现', note: '' },
          { id: 'b', agent: 'pi', provider: 'qwen', resolved_id: '', state: '未发现', note: '' },
          { id: 'c', agent: 'pi', provider: 'zai', resolved_id: '', state: '已验证', note: '' },
        ] },
        { id: 'claude', name: 'Claude Code', version: '', state: '可用', note: '', models: [
          { id: 'x', agent: 'claude', provider: '', resolved_id: '', state: '未发现', note: '' },
        ] },
      ],
    };
    expect(missingCount(withMissing)).toBe(3);
    expect(missingCount(withMissing, 'pi')).toBe(2);
    expect(missingCount(withMissing, 'codex')).toBe(0);
    expect(missingCount(null)).toBe(0);
  });

  it('清理确认框标题与引用提示', () => {
    expect(pruneTitle(3)).toBe('清理 3 个未发现的模型');
    expect(referencedText(2)).toBe('2 个已清理的模型仍被引用');
  });
});

describe('更多 agent（TASK-62）', () => {
  const agent = (id: string, state: string, extra: { untested?: boolean; homepage?: string } = {}) => ({
    id, name: id, version: '', state, note: '', models: [], ...extra,
  });

  it('按状态分出已安装与未安装，各自保持引擎给的顺序', () => {
    const list = [
      agent('claude', '可用'),
      agent('cursor', '未安装'),
      agent('agy', '可用', { untested: true }),
      agent('qoder', '未安装'),
      agent('codex', '未登录'),
    ];
    const { installed, notInstalled } = splitInstalled(list);
    expect(installed.map((a) => a.id)).toEqual(['claude', 'agy', 'codex']);
    expect(notInstalled.map((a) => a.id)).toEqual(['cursor', 'qoder']);
    expect(isInstalled(list[1])).toBe(false);
    expect(splitInstalled([])).toEqual({ installed: [], notInstalled: [] });
  });

  it('未安装折叠行文字', () => {
    expect(notInstalledText(5)).toBe('未安装 5 个');
  });

  it('旧引擎缺 untested、homepage 时按 false 与空串', () => {
    expect(isUntested(agent('claude', '可用'))).toBe(false);
    expect(isUntested(agent('agy', '可用', { untested: true }))).toBe(true);
    expect(agentHomepage(agent('claude', '可用'))).toBe('');
    expect(agentHomepage(agent('cursor', '未安装', { homepage: ' https://cursor.com ' }))).toBe('https://cursor.com');
  });

  it('统计卡的 agent 数只算已安装的', () => {
    const data: ResourcesOutput = {
      agents: [agent('claude', '可用'), agent('cursor', '未安装'), agent('copilot', '未安装')],
    };
    expect(resourcesOverview(data)[0]).toEqual({ label: 'agent', value: '1', alert: false });
  });

  it('从未探测的 agent 留在主列表，但不计入 agent 数', () => {
    const data: ResourcesOutput = { agents: [agent('claude', '未探测'), agent('agy', '未探测')] };
    expect(splitInstalled(data.agents).installed).toHaveLength(2);
    expect(isConfirmedInstalled(agent('claude', '未探测'))).toBe(false);
    expect(resourcesOverview(data)[0]).toEqual({ label: 'agent', value: '0', alert: false });
  });

  it('下拉选项带上 agent 的未实测标记', () => {
    const data: ResourcesOutput = {
      agents: [
        {
          ...agent('agy', '可用', { untested: true }),
          models: [{ id: 'gemini-3-pro', agent: 'agy', provider: '', resolved_id: '', state: '可用', note: '' }],
        },
        resources.agents[0],
      ],
    };
    expect(seatOptions(data).map((o) => [o.agent, o.untested])).toEqual([['agy', true], ['claude', false]]);
  });
});
