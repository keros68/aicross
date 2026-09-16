// 资源接入契约类型（TASK-07）：与 src/resources.rs 的 serde 输出一一对应，
// 字段保持 snake_case，不改名。

import { agentDisplayName } from './agent-display';

export type Login = 'unknown' | 'logged_in' | 'logged_out';

export interface ResourceModel {
  id: string;
  agent: string;
  provider: string;
  vendor: string;
  billing_source: string;
  tier: string;
  thinking: string;
  status: string;
  // 09-15 口径的四态。旧库里没有这个字段，读到空串时由 verificationOf 推断。
  verification: string;
  // 别名对应的真实模型 ID（TASK-09b）。claude 的 fable/opus/sonnet/haiku 是
  // 别名，只看别名看不出跑的是哪个模型。后端补齐；旧 fixture 读到空串时由
  // resolvedIdOf 退回 id。
  resolved_id: string;
  // `resolved_id` 是哪来的（修正轮 R1）：`alias-table` = 本地静态别名表算的，
  // `identity` = 冒烟 / 首次派发核对到的真身。空串按 `alias-table` 处理。
  // 界面不显示它，它的用处是「别名表修正后旧库能不能被覆盖」。
  resolved_source: string;
  source: string;
  smoked_at: string;
  smoke_model: string;
  smoke_failure: string;
  meta: string;
  note: string;
  fallback_order?: string[];
  quota_exhausted_until?: string | null;
  // pi provider 登录检查结果。旧 resources.json 没有字段时按 unknown 处理。
  login?: Login;
  login_checked_at?: string;
}

export interface ResourceAgent {
  id: string;
  name: string;
  kind: string;
  detected: boolean;
  version: string;
  models: ResourceModel[];
}

export interface ResourceStore {
  version: number;
  agents: ResourceAgent[];
}

export interface CcSwitchHint {
  name: string;
  has_token: boolean;
  models: string[];
}

export interface ResourceOperation {
  store: ResourceStore;
  errors: string[];
  cc_switch_hints: CcSwitchHint[];
}

export interface ResourceModelRequest {
  agent_id: string;
  id: string;
  provider: string;
  vendor: string;
  tier: string;
  note: string;
  fallback_order: string[];
  clear_quota_exhausted?: boolean;
}

export interface ResourceModelKey {
  agent_id: string;
  id: string;
  provider: string;
}

export interface AddManualAgentRequest {
  name: string;
  binary: string;
  args: string[];
}

// ── 纯函数（vitest 覆盖） ────────────────────────────────────────────────

export const STATUS_CONNECTED = '已接入';
export const STATUS_FAILED = '冒烟失败';

/** `--version` 原样输出中供行内显示的版本号；无法识别时保留原文。 */
export function cliVersionNumber(raw: string): string {
  const value = raw.trim();
  return value.match(/\b\d+(?:\.\d+)+(?:[-+][0-9A-Za-z.-]+)?\b/)?.[0] ?? value;
}

export function resourceModelKey(model: Pick<ResourceModel, 'agent' | 'provider' | 'id'>): string {
  return `${model.agent}/${model.provider}/${model.id}`;
}

export function quotaExhaustedUntil(
  model: Pick<ResourceModel, 'quota_exhausted_until'>,
  now = Date.now(),
): string | null {
  const value = model.quota_exhausted_until?.trim();
  if (!value) return null;
  const timestamp = Date.parse(value);
  return Number.isFinite(timestamp) && timestamp > now ? value : null;
}

export function quotaExhaustedLabel(
  model: Pick<ResourceModel, 'quota_exhausted_until'>,
  now = Date.now(),
): string {
  const value = quotaExhaustedUntil(model, now);
  if (!value) return '';
  const time = new Intl.DateTimeFormat('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false }).format(new Date(value));
  return `额度用尽至 ${time}`;
}

/** 09-15 口径的四态，与 Rust 的 `ResourceModel::verification` 一一对应。 */
export type Verification = 'native' | 'unverified' | 'verified' | 'failed';

/**
 * 原生 CLI：自家模型清单说有就是有，不冒烟。与 Rust 的 NATIVE_AGENTS 对齐。
 * 只用于旧库 / fixture 里 `verification` 为空时的兜底：端点判定（claude 挂
 * `ANTHROPIC_BASE_URL` 就不算原生）只在后端做，后端填进来的字段优先。
 */
const NATIVE_AGENTS = ['claude', 'codex', 'kimi'];

const VERIFICATIONS: Verification[] = ['native', 'unverified', 'verified', 'failed'];

/**
 * 模型的验证档位。后端会把这个字段补全，这里再兜一次底：旧库或手写 fixture
 * 里 `verification` 为空时按 agent + 旧 `status` 推断，映射与 Rust 的
 * `inferred_verification` 一致。
 */
export function verificationOf(
  model: Pick<ResourceModel, 'agent' | 'status' | 'verification'>,
): Verification {
  const value = (model.verification || '').trim() as Verification;
  if (VERIFICATIONS.includes(value)) return value;
  if (NATIVE_AGENTS.includes(model.agent.trim().toLowerCase())) return 'native';
  if (model.status === STATUS_CONNECTED) return 'verified';
  if (model.status === STATUS_FAILED) return 'failed';
  return 'unverified';
}

/** 状态词。原生模型不给词——常态不该占一行文字（视觉纪律）。 */
export function verificationLabel(verification: Verification): string {
  if (verification === 'unverified') return '未验证';
  if (verification === 'verified') return '已验证';
  if (verification === 'failed') return '验证失败';
  return '原生';
}

/**
 * G1 里给这个目标的一句话提示；常态返回空串（不占一行）。
 * 未验证是第三方模型的初始状态（本机 pi 几十条都是），是常态不是异常——
 * 走 `dispatch-reason`（text-3），红字只留给 failed（修正轮 R1）。
 */
export function verificationHint(
  verification: Verification | undefined,
): { text: string; tone: 'reason' | 'warning' } | null {
  if (verification === 'unverified') {
    return { text: '该模型未验证，首次派发会先发一个短请求核对真身。', tone: 'reason' };
  }
  if (verification === 'failed') {
    return { text: '该模型上次验证失败，可在接入页重新验证。', tone: 'warning' };
  }
  return null;
}

const VERIFICATION_ORDER: Record<Verification, number> = {
  native: 0,
  verified: 1,
  unverified: 2,
  failed: 3,
};

/**
 * 左栏列全部已探测模型（09-15 起不再只列冒烟通过的），按 agent 分组；
 * 组内排序：原生 → 已验证 → 未验证 → 验证失败，同档按模型名。
 * 没有模型的 agent 不出现——未检测到的 CLI 在接入页里管。
 */
export function modelsByAgent(store: ResourceStore): ResourceAgent[] {
  return store.agents
    .filter((agent) => agent.models.length > 0)
    .map((agent) => ({
      ...agent,
      models: [...agent.models].sort(
        (a, b) =>
          VERIFICATION_ORDER[verificationOf(a)] - VERIFICATION_ORDER[verificationOf(b)] ||
          a.provider.localeCompare(b.provider) ||
          a.id.localeCompare(b.id),
      ),
    }));
}

/**
 * 模型的真实 ID。后端会补齐 `resolved_id`；旧库与手写 fixture 读到空串时退回
 * `id`（非别名模型两者本来就相同）。
 */
export function resolvedIdOf(model: Pick<ResourceModel, 'id' | 'resolved_id'>): string {
  return (model.resolved_id || '').trim() || model.id;
}

/**
 * 行内次文本：别名与真实 ID 不同时才显示真身，相同就只显示一次（TASK-09b）。
 * claude 的四个别名是唯一会命中的一组。
 */
export function modelSecondaryId(model: Pick<ResourceModel, 'id' | 'resolved_id'>): string {
  const resolved = resolvedIdOf(model);
  return resolved === model.id ? '' : resolved;
}

/**
 * 行内模型名仍是派发时使用的 `id`，只去掉与 agent 显示名同义的厂商前缀。
 * 比较时忽略空格、连字符等分隔符，因此 `kimi-code` 与 `Kimi Code` 同义。
 */
export function modelDisplayName(
  model: Pick<ResourceModel, 'agent' | 'id'>,
  displayName = agentDisplayName(model.agent, model.agent),
): string {
  const slash = model.id.indexOf('/');
  if (slash <= 0 || slash === model.id.length - 1) return model.id;
  const normalize = (value: string) => value.toLowerCase().replace(/[^a-z0-9]+/g, '');
  return normalize(model.id.slice(0, slash)) === normalize(displayName)
    ? model.id.slice(slash + 1)
    : model.id;
}

function shortenHomePath(value: string): string {
  return value
    .replace(/[A-Za-z]:[\\/]Users[\\/][^\\/]+(?=[\\/]|$)/i, '~')
    .replace(/\/(?:Users|home)\/[^/]+(?=\/|$)/, '~');
}

/** 把新旧来源字段统一成「来源：说明 · 日期」，旧库无需重新探测。 */
export function normalizeModelSource(source: string): string {
  const value = source.trim();
  if (!value) return '来源：未记录 · 日期未知';
  // 幂等：已经带「来源：」前缀但日期不是标准形状的串（比如上面那条「日期未知」）
  // 再过一次会掉进末尾兜底，被拼成「来源：来源：未记录 · 日期未知 · 日期未知」
  // （09-16 复审 Gemini 2）。
  if (value.startsWith('来源：') && !/·\s*\d{4}-\d{2}-\d{2}$/.test(value)) return value;

  const current = /^来源：(.+?)\s*·\s*(\d{4}-\d{2}-\d{2})$/.exec(value);
  if (current) return `来源：${shortenHomePath(current[1].trim())} · ${current[2]}`;

  const read = /^读:(.+?)\s*\((\d{4}-\d{2}-\d{2})(?:T[^)]*)?\)$/.exec(value);
  if (read) return `来源：读取 ${shortenHomePath(read[1].trim())} · ${read[2]}`;

  const declared = /^申报\((\d{4}-\d{2}-\d{2})(?:T[^)]*)?\)$/.exec(value);
  if (declared) return `来源：用户申报 · ${declared[1]}`;

  const dated = /^(.+?)\s*\((\d{4}-\d{2}-\d{2})(?:T[^)]*)?\)$/.exec(value);
  if (dated) return `来源：${shortenHomePath(dated[1].trim())} · ${dated[2]}`;

  const legacyRead = value.startsWith('读:') ? `读取 ${value.slice(2).trim()}` : value;
  return `来源：${shortenHomePath(legacyRead)} · 日期未知`;
}

/** 接入页统计：只数未验证的第三方条目，原生不计。 */
export function unverifiedCount(store: ResourceStore): number {
  let count = 0;
  for (const agent of store.agents) {
    for (const model of agent.models) {
      if (verificationOf(model) === 'unverified') count += 1;
    }
  }
  return count;
}

export function unverifiedSummaryText(store: ResourceStore): string {
  const count = unverifiedCount(store);
  return count ? `${count} 个未验证（首次派发时自动验证）` : '';
}

export interface ProviderGroup {
  /** 这一组的 provider 名（原生 agent 是空串）。是否渲染组头看 `showsGroupHeads`。 */
  provider: string;
  models: ResourceModel[];
}

const LOGINS: Login[] = ['unknown', 'logged_in', 'logged_out'];

/** pi provider 的登录状态；缺字段、异常值都按「未检查」显示。 */
export function loginOf(model: { login?: string }): Login {
  const value = (model.login || '').trim() as Login;
  return LOGINS.includes(value) ? value : 'unknown';
}

export function loginLabel(login: Login): string {
  return login === 'logged_in' ? '已登录' : login === 'logged_out' ? '未登录' : '未检查';
}

/**
 * 按 provider 分小组（TASK-09b）。pi 底下几十条模型混在一起没法看，分组之后
 * 组内不再需要 `· provider` 后缀——组头已经说了是哪个端点。
 *
 * 组按 provider 名排序（固定 `'en'`，不跟随系统 locale——后端 `detected()` 用
 * 的是字节序，跟着系统走会让左栏与下拉对不上），输入顺序在组内保留
 * （`modelsByAgent` 已按 验证档 → provider → id 排过）。
 *
 * 修正轮 R1（DS 3）：只有一个 provider 时**照样**带上它的名字，不再抹成空串。
 * 抹空会让单 provider 的 pi 在左栏和接入页都看不到自己挂在哪个端点上；
 * 「不出组头」是渲染判据（`showsGroupHeads`），不是数据判据。
 */
export function providerGroups(models: ResourceModel[]): ProviderGroup[] {
  const groups = new Map<string, ResourceModel[]>();
  for (const model of models) {
    const provider = model.provider.trim();
    groups.set(provider, [...(groups.get(provider) ?? []), model]);
  }
  return [...groups.entries()]
    .sort(([left], [right]) => left.localeCompare(right, 'en'))
    .map(([provider, grouped]) => ({ provider, models: grouped }));
}

/**
 * pi 的 provider 分组：只有已登录正常显示；未登录和未检查的 provider 都合成
 * 一行，避免其下大批未验证模型把资源页和左栏拉长。一个 provider 的模型共用
 * 登录状态。组内只要有已登录模型就保留正常列，其余行单独标记；
 * 全部未检查时才收入「未检查」桶。
 */
export function splitPiProviderGroups(models: ResourceModel[]): {
  shown: ProviderGroup[];
  loggedOut: ProviderGroup[];
  unknown: ProviderGroup[];
} {
  const shown: ProviderGroup[] = [];
  const loggedOut: ProviderGroup[] = [];
  const unknown: ProviderGroup[] = [];
  for (const group of providerGroups(models)) {
    const states = group.models.map(loginOf);
    if (states.includes('logged_in')) shown.push(group);
    else if (states.every((state) => state === 'unknown')) unknown.push(group);
    else loggedOut.push(group);
  }
  return { shown, loggedOut, unknown };
}

export function providerLoginOf(models: ResourceModel[]): Login {
  const states = models.map(loginOf);
  if (states.includes('logged_in')) return 'logged_in';
  if (states.every((state) => state === 'unknown')) return 'unknown';
  return 'logged_out';
}

export function loginCheckHint(models: ResourceModel[]): string {
  return models.some((model) => loginOf(model) === 'unknown' && !!model.login_checked_at)
    ? '登录状态未检查'
    : '';
}

/** 未登录 provider 的折叠行文本。这一行本身就是展开按钮。 */
export function loggedOutProvidersToggleLabel(count: number, expanded: boolean): string {
  return `未登录 ${count} 个 provider · ${expanded ? '收起' : '展开'}`;
}

export function unknownProvidersToggleLabel(count: number, expanded: boolean): string {
  return `未检查 ${count} 个 provider · ${expanded ? '收起' : '展开'}`;
}

/**
 * 要不要渲染组头。一个组挂一个组头是纯噪声（原生 agent 更是一个空名组），
 * 单组时 provider 改走行悬停（左栏）与详情块（接入页）。
 */
export function showsGroupHeads(groups: ProviderGroup[]): boolean {
  return groups.length > 1;
}

/**
 * 小组内拆成「常显」与「默认折叠」两段：已验证 / 验证失败 / 原生常显，
 * 未验证折叠（pi 一个端点几十条未验证，全部铺开就是用户说的「很乱」）。
 */
export function splitUnverified(models: ResourceModel[]): {
  shown: ResourceModel[];
  collapsed: ResourceModel[];
} {
  const shown: ResourceModel[] = [];
  const collapsed: ResourceModel[] = [];
  for (const model of models) {
    (verificationOf(model) === 'unverified' ? collapsed : shown).push(model);
  }
  return { shown, collapsed };
}

/** 折叠行的文字。这一行本身就是展开按钮。 */
export function unverifiedToggleLabel(count: number, expanded: boolean): string {
  return `未验证 ${count} 个 · ${expanded ? '收起' : '展开'}`;
}

/**
 * 点一行模型时交给 `insertMention` 的三段（修正轮 R1，DS 2）。
 * provider 是 `@agent/模型#provider` 里的 `#provider`，掉了就分不清同名模型
 * 挂在哪个端点上——这是 TASK-09b 要保住的旧行为，单拎成纯函数好钉住。
 * `model.agent` 可能没回填（旧库），用所在 agent 的 id 兜底。
 */
export function modelPickTarget(
  model: Pick<ResourceModel, 'agent' | 'id' | 'provider'>,
  agentId: string,
): { agent: string; model: string; provider: string } {
  return { agent: model.agent || agentId, model: model.id, provider: model.provider };
}

/** 小组的折叠键：同一个 provider 名可能挂在两个 agent 下，必须带 agent。 */
export function groupKey(agentId: string, provider: string): string {
  return `${agentId}/${provider}`;
}

/** 字母徽标上的字：agent id 前两位字母数字大写。 */
export function agentMark(id: string): string {
  return id.replace(/[^A-Za-z0-9]/g, '').slice(0, 2).toUpperCase() || 'AI';
}

/**
 * 模型行的悬停提示：provider、登录状态、来源路径、最近验证时间与真身、失败原因
 * 都收在这里。09-15 起这些不再进行内正文——左栏宽度有限，每行顶一条
 * `读:C:\Users\…` 会把模型名挤没。
 *
 * 修正轮 R1：同一段文本也挂进行的 `aria-describedby`，键盘用户读得到
 * （title 在 WebView2 里只响应 hover）。
 */
export function modelDetailLines(model: ResourceModel): string[] {
  const lines = [
    modelDisplayName(model),
    `模型 ID：${resolvedIdOf(model)}`,
    `状态：${verificationLabel(verificationOf(model))}`,
    normalizeModelSource(model.source),
  ];
  const quota = quotaExhaustedLabel(model);
  if (quota) lines.push(quota);
  if (model.provider.trim() !== '') lines.push(`provider：${model.provider}`);
  if (model.agent === 'pi' || model.login_checked_at) {
    lines.push(`登录状态：${loginLabel(loginOf(model))}`);
  }
  if (model.login_checked_at) lines.push(`最近登录检查：${model.login_checked_at}`);
  if (model.smoked_at) {
    lines.push(`最近验证：${model.smoked_at}${model.smoke_model ? `（${model.smoke_model}）` : ''}`);
  }
  if (model.smoke_failure) lines.push(`失败原因：${model.smoke_failure}`);
  // meta 与 note 必须留在末尾（09-16 复审 Gemini 1）：meta 是「真实模型 ID：…；
  // 上下文：…；effort：…」这样的键值串，合并层还会往里追加「未在事实源中」标记；
  // note 承载用户手写备注和「按已知路由别名重判（日期）」的翻案留痕。前一版为了
  // 统一形状把这两行整个删了，等于把这些信息从界面上抹掉。
  if (model.meta.trim() !== '') lines.push(model.meta.trim());
  if (model.note.trim() !== '') lines.push(`备注：${model.note.trim()}`);
  return lines;
}

export function modelHint(model: ResourceModel): string {
  return modelDetailLines(model).join('\n');
}

/** 启动参数文本 → argv（支持单双引号；引号未闭合抛错）。 */
export function parseCommandArgs(text: string): string[] {
  const args: string[] = [];
  let current = '';
  let quote: "'" | '"' | null = null;
  let started = false;
  for (let index = 0; index < text.length; index += 1) {
    const character = text[index];
    if (quote) {
      if (character === quote) {
        quote = null;
      } else if (quote === '"' && character === '\\' && (text[index + 1] === '"' || text[index + 1] === '\\')) {
        current += text[index + 1];
        index += 1;
      } else {
        current += character;
      }
      started = true;
    } else if (/\s/.test(character)) {
      if (started) {
        args.push(current);
        current = '';
        started = false;
      }
    } else if (character === '"' || character === "'") {
      quote = character;
      started = true;
    } else {
      current += character;
      started = true;
    }
  }
  if (quote) throw new Error('启动参数引号未闭合');
  if (started) args.push(current);
  return args;
}

export function errorText(value: unknown): string {
  const message = value instanceof Error ? value.message : String(value);
  return message.replace(/^Error:\s*/, '');
}
