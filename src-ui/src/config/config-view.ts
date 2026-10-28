// 资源页与模式页的展示纯函数（TASK-47；TASK-49、fix-49-r1/r2 加概览、流程条、分组）。
// 数据契约在 ../tauri.ts（镜像 `aicross resources` 与 `aicross config show`）；
// 页面组件不掺逻辑，测试落在这层。

import type {
  ConfigShow,
  ModeEntry,
  ModeSeatNode,
  ResourceAgentEntry,
  ResourceModelEntry,
  ResourcesOutput,
  Seat,
  SeatFallback,
  SeatSource,
} from '../tauri';

// ── 状态词 ───────────────────────────────────────────────────────────

/** 需要快速扫到的异常状态用红色胶囊（视觉纪律）；「未探测」用中性灰胶囊（TASK-58），
 *  其余纯文字。 */
const ALERT_STATES = new Set(['验证失败', '未登录', '额度用尽', '未安装', '未发现']);

export function stateTone(state: string): 'alert' | 'muted' | 'plain' {
  if (ALERT_STATES.has(state)) return 'alert';
  if (state === '未探测') return 'muted';
  return 'plain';
}

/** 状态词的显示（fix-51-r3）：引擎的「冒烟超时」在界面上叫「连接超时」，其余原样。 */
export function stateLabel(state: string): string {
  return state === '冒烟超时' ? '连接超时' : state;
}

/** 第三方模型待核对或核对没通过时才给「测试连接」按钮。 */
const SMOKE_STATES = new Set(['未验证', '验证失败', '冒烟超时']);

export function canSmoke(model: Pick<ResourceModelEntry, 'state'>): boolean {
  return SMOKE_STATES.has(model.state);
}

/** 实际模型与 id 不同时才显示；相同或为空返回空串。 */
export function resolvedModelText(model: Pick<ResourceModelEntry, 'id' | 'resolved_id'>): string {
  const resolved = model.resolved_id.trim();
  return resolved === '' || resolved === model.id ? '' : resolved;
}

// ── 人选下拉 ─────────────────────────────────────────────────────────

export interface SeatOption {
  key: string;
  agent: string;
  /** 原生模型为 null。 */
  provider: string | null;
  model: string;
  /** 资源状态词；当前值已不在资源里时为「不可用」。 */
  state: string;
  disabled: boolean;
  /** 所属 agent 未实测（TASK-62）；下拉分组名后标「未实测」。 */
  untested?: boolean;
}

/** 选人时能选的状态（引擎状态词）：下拉可选项与「可用模型」统计都按这一组。
 *  连接超时、额度用尽、未验证等其余状态都不可选。 */
const SELECTABLE_STATES = new Set(['可用', '已验证']);

export function isSelectableState(state: string): boolean {
  return SELECTABLE_STATES.has(state);
}

function normalizeProvider(provider: string | null | undefined): string | null {
  const value = (provider ?? '').trim();
  return value === '' ? null : value;
}

/** 下拉选项的值：agent、provider、model 三元组，provider 空与 null 等同。 */
export function seatKey(agent: string, provider: string | null | undefined, model: string): string {
  return JSON.stringify([agent, normalizeProvider(provider) ?? '', model]);
}

/** 资源里 agent 状态为「可用」的全部模型；状态不是可用、已验证的选项禁用。 */
export function seatOptions(resources: ResourcesOutput | null | undefined): SeatOption[] {
  const options: SeatOption[] = [];
  for (const agent of resources?.agents ?? []) {
    if (agent.state !== '可用') continue;
    for (const model of agent.models) {
      const agentId = model.agent || agent.id;
      options.push({
        key: seatKey(agentId, model.provider, model.id),
        agent: agentId,
        provider: normalizeProvider(model.provider),
        model: model.id,
        state: stateLabel(model.state),
        disabled: !isSelectableState(model.state),
        untested: isUntested(agent),
      });
    }
  }
  return options;
}

// ── 位置卡片 ─────────────────────────────────────────────────────────

const SOURCE_TEXT: Record<SeatSource, string> = {
  位置: '本模式',
  角色: '角色默认',
  预设: '预设固定',
  未指定: '未指定',
};

export function sourceText(source: SeatSource): string {
  return SOURCE_TEXT[source] ?? source;
}

export function seatIsMissing(node: Pick<ModeSeatNode, 'source' | 'effective'>): boolean {
  return node.source === '未指定' || node.effective === null;
}

/** effective.state 为这些值时人选派不出去（TASK-48）。 */
const INVALID_SEAT_STATES = new Set(['未发现', '验证失败', '未登录', '不在资源库']);

/** 引擎标出的位置规则违反原因（effective.invalid）；合规或旧版引擎没有时为空串。 */
export function seatRuleViolation(node: Pick<ModeSeatNode, 'effective'>): string {
  return (node.effective?.invalid ?? '').trim();
}

/** 违反位置规则时位置行下拉上的标记；完整原因另行显示。 */
export const SEAT_RULE_BADGE = '不能放在此位置';

/** 人选失效时返回原因：违反位置规则时为引擎给的原因，否则为失效的状态词；正常（含旧数据缺
 *  state）返回空串。 */
export function seatInvalidState(node: Pick<ModeSeatNode, 'effective'>): string {
  const rule = seatRuleViolation(node);
  if (rule !== '') return rule;
  const state = node.effective?.state ?? '';
  return INVALID_SEAT_STATES.has(state) ? state : '';
}

/** 位置的展示状态：未指定或人选失效时 alert，说明文字随之换成原因。 */
export function seatStatus(node: Pick<ModeSeatNode, 'source' | 'effective'>): { alert: boolean; label: string } {
  if (seatIsMissing(node)) return { alert: true, label: sourceText('未指定') };
  const invalid = seatInvalidState(node);
  if (invalid !== '') return { alert: true, label: `人选失效：${invalid}` };
  return { alert: false, label: sourceText(node.source) };
}

// ── 页顶概览卡 ───────────────────────────────────────────────────────

export interface StatItem {
  label: string;
  value: string;
  /** 数字用异常色（不可用、失败等非 0 时）。 */
  alert: boolean;
  /** 这一格可点作筛选时的筛选名（fix-51-r2）；数字为 0 时不给。 */
  filter?: 'unavailable';
}

/** 不可用的模型状态（fix-51-r2）：最近一次探测没发现、冒烟没通过、没登录、额度用尽。 */
const UNAVAILABLE_STATES = new Set(['未发现', '验证失败', '未登录', '额度用尽']);

export function isUnavailable(model: Pick<ResourceModelEntry, 'state'>): boolean {
  return UNAVAILABLE_STATES.has(model.state);
}

// ── 已安装 / 未安装（TASK-62）────────────────────────────────────────

/** 引擎给出的 agent 状态为「未安装」的不算已安装。 */
export function isInstalled(agent: Pick<ResourceAgentEntry, 'state'>): boolean {
  return agent.state !== '未安装';
}

/** 确认装了的：探测过且不是「未安装」。从未探测的照常列卡片，但不计入 agent 数。 */
export function isConfirmedInstalled(agent: Pick<ResourceAgentEntry, 'state'>): boolean {
  return agent.state !== '未安装' && agent.state !== '未探测';
}

/** 留在主列表的 agent：装了的，以及这次没找到、但资源库里还有模型（以前探测到过）的
 *  （TASK-66），后者以卡片显示状态与原因，不折叠进「未安装」。 */
export function isListed(agent: Pick<ResourceAgentEntry, 'state' | 'models'>): boolean {
  return isInstalled(agent) || agent.models.length > 0;
}

/** 分出主列表与「未安装」折叠行，各自保持引擎给的顺序。 */
export function splitInstalled<T extends Pick<ResourceAgentEntry, 'state' | 'models'>>(
  agents: readonly T[],
): { installed: T[]; notInstalled: T[] } {
  const installed: T[] = [];
  const notInstalled: T[] = [];
  for (const agent of agents) (isListed(agent) ? installed : notInstalled).push(agent);
  return { installed, notInstalled };
}

/** 未安装折叠行的文字。 */
export function notInstalledText(count: number): string {
  return `未安装 ${count} 个`;
}

/** 旧引擎没有 untested 字段时按 false。 */
export function isUntested(agent: Pick<ResourceAgentEntry, 'untested'>): boolean {
  return agent.untested === true;
}

/** 官网地址；旧引擎没有 homepage 字段时为空串。 */
export function agentHomepage(agent: Pick<ResourceAgentEntry, 'homepage'>): string {
  return (agent.homepage ?? '').trim();
}

/** 资源页概览：已安装 agent 数 · 可用模型数（人选下拉里可选的）· 已验证 · 不可用（非 0 时可点筛选）。
 *  模型统计只算已安装的 agent。 */
export function resourcesOverview(resources: ResourcesOutput | null | undefined): StatItem[] {
  const agents = splitInstalled(resources?.agents ?? []).installed;
  const models = agents.flatMap((agent) => agent.models);
  const installedCount = agents.filter(isConfirmedInstalled).length;
  const usable = seatOptions(resources).filter((option) => !option.disabled).length;
  const verified = models.filter((model) => model.state === '已验证').length;
  const unavailable = models.filter(isUnavailable).length;
  const unavailableItem: StatItem = { label: '不可用', value: String(unavailable), alert: unavailable > 0 };
  if (unavailable > 0) unavailableItem.filter = 'unavailable';
  return [
    { label: 'agent', value: String(installedCount), alert: false },
    { label: '可用模型', value: String(usable), alert: false },
    { label: '已验证', value: String(verified), alert: false },
    unavailableItem,
  ];
}

/** 所有位置都有有效人选（不是未指定、没有失效）且至少有一个位置，算配好。 */
export function modeIsReady(mode: Pick<ModeEntry, 'stages'>): boolean {
  const seats = flowColumns(mode.stages).flatMap((column) => column.seats);
  return seats.length > 0 && seats.every((seat) => !seatStatus(seat.node).alert);
}

/** 模式页概览：默认模式名 · 已配好 / 总数 · 待处理位置（未指定或失效）。 */
export function modesOverview(config: Pick<ConfigShow, 'modes' | 'defaultMode'> | null | undefined): StatItem[] {
  const modes = config?.modes ?? [];
  const defaultName = modes.find((mode) => mode.id === config?.defaultMode)?.name ?? '未设置';
  const ready = modes.filter(modeIsReady).length;
  const pending = modes
    .flatMap((mode) => flowColumns(mode.stages).flatMap((column) => column.seats))
    .filter((seat) => seatStatus(seat.node).alert).length;
  return [
    { label: '默认模式', value: defaultName, alert: false },
    { label: '已配好', value: `${ready} / ${modes.length}`, alert: false },
    { label: '待处理位置', value: String(pending), alert: pending > 0 },
  ];
}

// ── 模式卡片的位置行（fix-51-r1） ─────────────────────────────────────

export interface SeatRowState {
  /** 未指定：模型下拉显示「未选择」并用异常色。 */
  missing: boolean;
  /** 人选失效时下拉上的标记（失效的状态词，或违反位置规则的标记），并用异常色；正常为空串。 */
  invalid: string;
  /** 违反位置规则时引擎给的原因，显示在行下；其余为空串。 */
  reason: string;
  /** 本模式人选由一键推荐填的（origin 为 recommended），下拉里加「推荐」标记。 */
  recommended: boolean;
  /** 预设写定的位置：下拉禁用，行内标「预设」。 */
  presetFixed: boolean;
}

export function seatRowState(node: Pick<ModeSeatNode, 'source' | 'seat' | 'effective'>): SeatRowState {
  const missing = seatIsMissing(node);
  const reason = missing ? '' : seatRuleViolation(node);
  return {
    missing,
    invalid: missing ? '' : reason !== '' ? SEAT_RULE_BADGE : seatInvalidState(node),
    reason,
    recommended: node.source === '位置' && node.seat?.origin === 'recommended',
    presetFixed: node.source === '预设',
  };
}

/** 资源库从未探测过时，模式页顶部提示先去探测。资源还没读到时不提示。 */
export function needsFirstDetect(resources: ResourcesOutput | null | undefined): boolean {
  if (!resources) return false;
  return (resources.detected_at ?? '').trim() === '';
}

// ── 资源卡片 ─────────────────────────────────────────────────────────

/** 列表行左侧标识块里的字：名称第一个字，拉丁字母转大写。 */
export function initialOf(name: string): string {
  const first = Array.from(name.trim())[0] ?? '';
  return first.toUpperCase();
}

/** 全是原生模型（provider 为空）的 agent，如 Claude Code、Codex、Kimi；没有模型时不算。 */
export function isNativeAgent(agent: Pick<ResourceAgentEntry, 'models'>): boolean {
  return agent.models.length > 0 && agent.models.every((model) => model.provider.trim() === '');
}

/** agent 卡片头的小统计（fix-51-r2）：原生 agent「N 个模型 · 原生」；第三方
 *  「N 个模型 · X 已验证 · Y 不可用」（Y 为 0 时省略，非 0 时 alert）；原生 agent 有
 *  不可用模型时同样补「Y 不可用」。agent 本身不可用时末尾加上它的状态（「未探测」不算异常）。 */
export function agentStats(agent: Pick<ResourceAgentEntry, 'models' | 'state'>): { text: string; alert: boolean }[] {
  const unavailable = agent.models.filter(isUnavailable).length;
  const parts = [{ text: `${agent.models.length} 个模型`, alert: false }];
  if (isNativeAgent(agent)) {
    parts.push({ text: '原生', alert: false });
  } else if (agent.models.length > 0) {
    const verified = agent.models.filter((model) => model.state === '已验证').length;
    parts.push({ text: `${verified} 已验证`, alert: false });
  }
  if (unavailable > 0) parts.push({ text: `${unavailable} 不可用`, alert: true });
  if (agent.state !== '可用') parts.push({ text: agent.state, alert: agent.state !== '未探测' });
  return parts;
}

// ── 资源卡片的折叠（fix-51-r2） ─────────────────────────────────────

export const AGENT_OPEN_STORAGE_KEY = 'aicross.resources-open';

type OpenStorage = Pick<Storage, 'getItem' | 'setItem'>;

/** 读折叠记忆：agent id → 是否展开；读不到或格式不对时为空。 */
export function readAgentOpen(storage: OpenStorage | null): Record<string, boolean> {
  try {
    const raw = storage?.getItem(AGENT_OPEN_STORAGE_KEY);
    if (!raw) return {};
    const parsed: unknown = JSON.parse(raw);
    if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) return {};
    return Object.fromEntries(Object.entries(parsed).filter((entry): entry is [string, boolean] => typeof entry[1] === 'boolean'));
  } catch {
    return {};
  }
}

export function writeAgentOpen(storage: OpenStorage | null, state: Record<string, boolean>): void {
  try {
    storage?.setItem(AGENT_OPEN_STORAGE_KEY, JSON.stringify(state));
  } catch {
    /* 存不进去只是下次回到默认折叠 */
  }
}

/** 卡片有没有可展开的内容（fix-51-r3）：有模型、有说明，或 agent 本身不可用（展开
 *  后显示它的状态）。没有时卡片头不显示展开图标、也不响应点击。 */
export function agentExpandable(agent: Pick<ResourceAgentEntry, 'models' | 'note' | 'state'>): boolean {
  return agent.models.length > 0 || agent.note.trim() !== '' || agent.state !== '可用';
}

/** 最近一次探测未发现：以引擎的 missing 字段为准；旧版引擎没有这个字段时退回看
 *  状态是否为「未发现」。 */
export function isMissing(model: Pick<ResourceModelEntry, 'state' | 'missing'>): boolean {
  return typeof model.missing === 'boolean' ? model.missing : model.state === '未发现';
}

/** 模型行下方的说明（fix-51-r3）：未发现的行、所属 agent 不可用时都不显示（胶囊与
 *  agent 状态已说明），其余原样。 */
export function modelNote(model: Pick<ResourceModelEntry, 'state' | 'note' | 'missing'>, agentState = '可用'): string {
  if (agentState !== '可用' || isMissing(model)) return '';
  return model.note.trim();
}

// ── 清理未发现（fix-51-r3，窗口命令 resources_prune） ──────────────────

/** 资源里「未发现」的模型数；给了 agent 只数这一家。 */
export function missingCount(resources: ResourcesOutput | null | undefined, agentId?: string): number {
  return (resources?.agents ?? [])
    .filter((agent) => agentId === undefined || agent.id === agentId)
    .flatMap((agent) => agent.models)
    .filter(isMissing).length;
}

export function pruneTitle(count: number): string {
  return `清理 ${count} 个未发现的模型`;
}

export function referencedText(count: number): string {
  return `${count} 个已清理的模型仍被引用`;
}

/** 卡片是否展开：记住过就按记忆，否则有不可用模型的展开、其余收起。 */
export function isAgentOpen(agent: Pick<ResourceAgentEntry, 'id' | 'models'>, saved: Record<string, boolean>): boolean {
  const remembered = saved[agent.id];
  if (typeof remembered === 'boolean') return remembered;
  return agent.models.some(isUnavailable);
}

/** 模型按 provider 分小组，组序与组内顺序都按首次出现；原生模型（provider 为空）
 *  一组，标题为「原生」。 */
export function groupByProvider(models: ResourceModelEntry[]): { provider: string; label: string; models: ResourceModelEntry[] }[] {
  const groups: { provider: string; label: string; models: ResourceModelEntry[] }[] = [];
  for (const model of models) {
    const provider = model.provider.trim();
    let group = groups.find((item) => item.provider === provider);
    if (!group) {
      group = { provider, label: provider === '' ? '原生' : provider, models: [] };
      groups.push(group);
    }
    group.models.push(model);
  }
  return groups;
}

export const MODEL_PREVIEW_LIMIT = 8;

/** 资源卡片里直接列出的模型（fix-51-r1，参照 magpie 的「Show 5 more」）：先按
 *  provider 分组排好，收起时只取前 limit 条，其余计数；展开时全部列出。 */
export function previewModels(models: ResourceModelEntry[], expanded: boolean, limit = MODEL_PREVIEW_LIMIT): {
  shown: ResourceModelEntry[];
  hidden: number;
} {
  const ordered = groupByProvider(models).flatMap((group) => group.models);
  if (expanded || ordered.length <= limit) return { shown: ordered, hidden: 0 };
  return { shown: ordered.slice(0, limit), hidden: ordered.length - limit };
}

export function moreModelsText(count: number): string {
  return `展开其余 ${count} 个`;
}

/** 模型分两组：照常列出的，与最近一次探测没发现、默认收起的。 */
export function splitMissingModels(models: ResourceAgentEntry['models']): {
  listed: ResourceModelEntry[];
  missing: ResourceModelEntry[];
} {
  return {
    listed: models.filter((model) => !isMissing(model)),
    missing: models.filter(isMissing),
  };
}

export function missingModelsText(count: number): string {
  return `${count} 个模型未发现`;
}

export const DETECT_STALE_MS = 7 * 24 * 60 * 60 * 1000;
export const DETECT_STALE_HINT = '资源超过 7 天未探测';

/** 页头「重新探测」左侧的上次探测时间（fix-49-r3）：当天只显示时分，当年显示
 *  月-日 时分，更早带年份；超过 7 天 stale 为 true。缺字段或 null 为「尚未探测」；
 *  解析不出来的时间原样显示，不提示过期。 */
export function detectStatus(detectedAt: string | null | undefined, now: number): { label: string; stale: boolean } {
  const value = (detectedAt ?? '').trim();
  if (value === '') return { label: '尚未探测', stale: false };
  const time = Date.parse(value);
  if (!Number.isFinite(time)) return { label: `上次探测 ${value}`, stale: false };
  const at = new Date(time);
  const today = new Date(now);
  const pad = (n: number) => String(n).padStart(2, '0');
  const clock = `${pad(at.getHours())}:${pad(at.getMinutes())}`;
  const day = `${pad(at.getMonth() + 1)}-${pad(at.getDate())}`;
  const text = at.toDateString() === today.toDateString()
    ? clock
    : at.getFullYear() === today.getFullYear() ? `${day} ${clock}` : `${at.getFullYear()}-${day} ${clock}`;
  return { label: `上次探测 ${text}`, stale: now - time > DETECT_STALE_MS };
}

// ── 流程图排布 ───────────────────────────────────────────────────────

export interface FlowSeat {
  /** 从 1 开始，与 config_set_seat 的参数一致。 */
  stage: number;
  index: number;
  node: ModeSeatNode;
}

export interface FlowColumn {
  stage: number;
  seats: FlowSeat[];
}

/** 阶段从左到右成列，同阶段的位置在列内上下排；空阶段不出列。 */
export function flowColumns(stages: ModeSeatNode[][]): FlowColumn[] {
  return stages
    .map((nodes, stageIndex) => ({
      stage: stageIndex + 1,
      seats: nodes.map((node, nodeIndex) => ({ stage: stageIndex + 1, index: nodeIndex + 1, node })),
    }))
    .filter((column) => column.seats.length > 0);
}

// ── 编辑 ─────────────────────────────────────────────────────────────

export const THINKING_OPTIONS: ReadonlyArray<{ value: string; label: string }> = [
  { value: '', label: '沿用预设' },
  { value: '关', label: '关' },
  { value: '低', label: '低' },
  { value: '中', label: '中' },
  { value: '高', label: '高' },
];

export interface SeatDraft {
  /** 空串表示还没选。 */
  personKey: string;
  thinking: string;
  /** 空串表示不设备选。 */
  fallbackKey: string;
  tier: string;
}

/** 编辑框初值：有本模式人选取它，否则取当前实际人选（按角色默认或预设写定）。 */
export function seatDraft(node: Pick<ModeSeatNode, 'seat' | 'effective'>): SeatDraft {
  const base = node.seat ?? node.effective;
  if (!base) return { personKey: '', thinking: '', fallbackKey: '', tier: '' };
  const fallback = base.fallback;
  return {
    personKey: seatKey(base.agent, base.provider, base.model),
    thinking: base.thinking ?? '',
    fallbackKey: fallback ? seatKey(fallback.agent, fallback.provider, fallback.model) : '',
    tier: node.seat?.tier ?? '',
  };
}

function parseSeatKey(key: string): { agent: string; provider: string | null; model: string } | null {
  try {
    const parsed: unknown = JSON.parse(key);
    if (!Array.isArray(parsed) || parsed.length !== 3 || !parsed.every((v) => typeof v === 'string')) {
      return null;
    }
    const [agent, provider, model] = parsed as string[];
    if (agent === '' || model === '') return null;
    return { agent, provider: normalizeProvider(provider), model };
  } catch {
    return null;
  }
}

/** 编辑框 → config_set_seat 的人选对象；没选人选时为 null。 */
export function seatFromDraft(draft: SeatDraft): Seat | null {
  const person = parseSeatKey(draft.personKey);
  if (!person) return null;
  const fallback: SeatFallback | null = draft.fallbackKey === '' ? null : parseSeatKey(draft.fallbackKey);
  return { ...person, tier: draft.tier, thinking: draft.thinking, fallback };
}

/** 当前值不在可选列表里时（agent 不可用了、模型被移除）补一个禁用项，
 * 让下拉如实显示现值而不是悄悄跳到第一项。 */
export function optionsWithCurrent(all: SeatOption[], currentKey: string): SeatOption[] {
  // 下拉只列可选状态（可用、已验证）的模型；当前人选即使不在其中也照常列出。
  const options = all.filter(
    (option) => option.key === currentKey || (!option.disabled && isSelectableState(option.state)),
  );
  if (currentKey === '' || options.some((option) => option.key === currentKey)) return options;
  const current = parseSeatKey(currentKey);
  if (!current) return options;
  return [
    ...options,
    {
      key: currentKey,
      ...current,
      state: '不可用',
      disabled: true,
    },
  ];
}
