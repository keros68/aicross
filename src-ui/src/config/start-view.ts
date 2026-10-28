// 「开始」页的展示纯函数（TASK-63）：副 agent 各有哪些模型、审与干活各用哪个模式（与 skill 1.23.0
// 「按窗口配置派发」同一套选法）、各位置派给谁、配好没有、示例输入。

import type { ModeEntry, ResourcesOutput, SkillHostStatus } from '../tauri';
import { flowColumns, isConfirmedInstalled, isSelectableState, modeIsReady, seatStatus } from './config-view';
import { globalInstallAction, skillInstalled, staleExternal } from './skill-view';
import { modeKind } from './mode-kind';

/** 厂商 → 用户口头常说的模型系列名；表里没有的照写厂商名，「其他」「未知」不写。 */
const FAMILY: Record<string, string> = {
  anthropic: 'Claude',
  openai: 'GPT',
  google: 'Gemini',
  moonshot: 'Kimi',
  zhipu: 'GLM',
  智谱: 'GLM',
  deepseek: 'DeepSeek',
  alibaba: 'Qwen',
  阿里: 'Qwen',
};

function familyOf(vendor: string | undefined): string {
  const value = (vendor ?? '').trim();
  if (value === '' || value === '其他' || value === '未知') return '';
  return FAMILY[value.toLowerCase()] ?? value;
}

export interface WorkerAgent {
  id: string;
  name: string;
  /** 可选（可用、已验证）的模型数。 */
  models: number;
  /** 未验证、验证后可选的模型数。 */
  pending: number;
  /** 含哪几家的模型，按模型数从多到少。 */
  families: string[];
  /** 也是当前主 agent。 */
  isHost: boolean;
}

/** 副 agent：探测过、确认装了的 agent，各自的可选与未验证模型数、含哪几家的模型（按
 *  可选加未验证的算）。同一家的模型常经几个 agent 都能用（pi 里也有 Claude），写出系列名
 *  让用户知道去哪找。 */
export function workerAgents(resources: ResourcesOutput | null | undefined, hostId: string): WorkerAgent[] {
  return (resources?.agents ?? []).filter(isConfirmedInstalled).map((agent) => {
    const usable = agent.models.filter((model) => isSelectableState(model.state));
    const pending = agent.models.filter((model) => model.state === '未验证');
    const counts = new Map<string, number>();
    for (const model of [...usable, ...pending]) {
      const family = familyOf(model.vendor);
      if (family !== '') counts.set(family, (counts.get(family) ?? 0) + 1);
    }
    const families = [...counts.entries()].sort((a, b) => b[1] - a[1]).map(([family]) => family);
    return { id: agent.id, name: agent.name, models: usable.length, pending: pending.length, families, isHost: agent.id === hostId };
  });
}

/** 副 agent 一行的说明：「可用 1 个，未验证 33 个 · 含 GLM、Claude、DeepSeek」。系列名全写，
 *  排在后面的往往正是用户要找的（pi 里的 Claude），截断会把它藏掉。 */
export function workerText(worker: Pick<WorkerAgent, 'models' | 'pending' | 'families'>): string {
  const count = worker.pending > 0 ? `可用 ${worker.models} 个，未验证 ${worker.pending} 个` : `可用 ${worker.models} 个`;
  return worker.families.length === 0 ? count : `${count} · 含 ${worker.families.join('、')}`;
}

const REVIEW_KINDS = new Set(['blind', 'jury']);
const WORK_KINDS = new Set(['shared', 'economy', 'arena', 'swarm']);

function pickMode(modes: ModeEntry[], defaultMode: string, kinds: Set<string>, fallbackId: string): ModeEntry | null {
  const current = modes.find((mode) => mode.id === defaultMode);
  if (current && kinds.has(modeKind(current.id, current.name))) return current;
  return modes.find((mode) => mode.id === fallbackId) ?? null;
}

/** 审：默认模式是盲审或评审就用它，否则盲审模式。 */
export function reviewMode(modes: ModeEntry[], defaultMode: string): ModeEntry | null {
  return pickMode(modes, defaultMode, REVIEW_KINDS, 'builtin-blind');
}

/** 干活：默认模式是共享、经济、擂台、蜂群之一就用它，否则经济模式。 */
export function workMode(modes: ModeEntry[], defaultMode: string): ModeEntry | null {
  return pickMode(modes, defaultMode, WORK_KINDS, 'builtin-economy');
}

export interface SeatLine {
  role: string;
  /** 实际派给的模型；没有人选时为空串。 */
  model: string;
  /** 完整坐标（agent / provider / model），悬停显示。 */
  title: string;
  alert: boolean;
}

/** 模式里每个位置一行：角色、模型、是否有问题（未指定或人选失效）。 */
export function seatLines(mode: Pick<ModeEntry, 'stages'> | null): SeatLine[] {
  if (!mode) return [];
  return flowColumns(mode.stages).flatMap((column) => column.seats.map(({ node }) => {
    const effective = node.effective;
    const title = effective ? [effective.agent, effective.provider, effective.model].filter((part) => part).join(' / ') : '';
    return { role: node.role, model: effective?.model ?? '', title, alert: seatStatus(node).alert };
  }));
}

export type SetupState = 'need-detect' | 'need-host' | 'todo' | 'done';

/** 第 2 步的状态：先要探测、选主 agent；skill 装好（或由其他软件管理）且审与干活两个模式
 *  都配好才算完成。 */
export function setupState(input: {
  detected: boolean;
  host: string;
  skillHost: SkillHostStatus | null | undefined;
  skillLoaded: boolean;
  bundledVersion: string;
  modes: ModeEntry[];
  defaultMode: string;
}): SetupState {
  if (!input.detected) return 'need-detect';
  if (input.host === '') return 'need-host';
  if (!input.skillLoaded) return 'todo';
  const skillOk = skillInstalled(input.skillHost)
    && !needsSkillInstall(input.skillHost, input.bundledVersion)
    && staleExternal(input.skillHost) === null;
  const review = reviewMode(input.modes, input.defaultMode);
  const work = workMode(input.modes, input.defaultMode);
  const seatsOk = review !== null && work !== null && modeIsReady(review) && modeIsReady(work);
  return skillOk && seatsOk ? 'done' : 'todo';
}

/** 一键配好时要不要先装 / 更新 skill（全局写入位置未安装或是我们装的旧版）。其他软件管理的不动。 */
export function needsSkillInstall(skillHost: SkillHostStatus | null | undefined, bundledVersion: string): boolean {
  if (skillHost?.locations.some((location) => location.kind === 'external' && location.readByThisHost !== false)) return false;
  const kind = globalInstallAction(skillHost, bundledVersion).kind;
  return kind === 'install' || kind === 'update';
}

/** 各主 agent 的默认调用写法；slash_list 读不到时用。 */
const DEFAULT_INVOKE: Record<string, string> = {
  claude: '/ai-cross',
  codex: '$ai-cross',
  kimi: '/skill:ai-cross',
  pi: '/skill:ai-cross',
};

export function invokeText(host: string, fromEngine: string | null | undefined): string {
  const value = (fromEngine ?? '').trim();
  return value !== '' ? value : DEFAULT_INVOKE[host] ?? '/ai-cross';
}

/** 第 3 步的两条示例输入。 */
export function usageExamples(invoke: string): { label: string; text: string }[] {
  return [
    { label: '审', text: `${invoke} 让别家审一下 main.py` },
    { label: '干活', text: `${invoke} 写一个读取 CSV 的函数，做完让别家验收` },
  ];
}

export interface SetupHint {
  /** 审与干活两个模式里还没人选的位置数。 */
  missing: number;
  /** 缺人的原因，已去重、改成读者能懂的话。 */
  reasons: string[];
  /** 资源库里还没验证、验证后可参与推荐的模型数。 */
  unverified: number;
}

/** 「开始」页只说审与干活两个模式还缺几个人、为什么、怎么补；逐条原始说明在模式页。
 *  notes 是一键推荐返回的说明，只取属于这两个模式的条目。 */
export function setupHint(modes: Array<ModeEntry | null>, notes: string[]): SetupHint {
  const shown = modes.filter((mode): mode is ModeEntry => mode !== null);
  const seen = new Set<string>();
  const unique = shown.filter((mode) => (seen.has(mode.id) ? false : (seen.add(mode.id), true)));
  const missing = unique.reduce((sum, mode) => sum + seatLines(mode).filter((line) => line.model === '').length, 0);
  const names = unique.map((mode) => `「${mode.name}」`);
  const reasons = new Set<string>();
  let unverified = 0;
  for (const note of notes) {
    if (!names.some((name) => note.startsWith(name))) continue;
    const vendor = /需不同于主 agent 厂商 ([^，；]+)/.exec(note);
    if (vendor) reasons.add(`审查要用和主 agent（${vendor[1]}）不同厂商的模型，可用的不够`);
    else if (note.includes('候选池为空')) reasons.add('还没有可用的模型');
    else if (note.includes('厂商互不相同')) reasons.add('可用模型的厂商不够多');
    else if (note.includes('档位要求')) reasons.add('没有满足档位要求的模型');
    const count = /另有 (\d+) 个未验证模型/.exec(note);
    if (count) unverified = Math.max(unverified, Number(count[1]));
  }
  return { missing, reasons: [...reasons], unverified };
}
