// 「开始」页的展示纯函数（TASK-63）：找到的 agent、审与干活各用哪个模式（与 skill 1.23.0
// 「按窗口配置派发」同一套选法）、各位置派给谁、配好没有、示例输入。

import type { ModeEntry, ResourcesOutput, SkillHostStatus } from '../tauri';
import { flowColumns, isConfirmedInstalled, isSelectableState, modeIsReady, seatStatus } from './config-view';
import { globalInstallAction, skillInstalled } from './skill-view';
import { modeKind } from './mode-kind';

/** 探测过、确认装了的 agent 名称与可选模型数。 */
export function foundAgents(resources: ResourcesOutput | null | undefined): { names: string[]; models: number } {
  const agents = (resources?.agents ?? []).filter(isConfirmedInstalled);
  const models = agents.reduce((sum, agent) => sum + agent.models.filter((model) => isSelectableState(model.state)).length, 0);
  return { names: agents.map((agent) => agent.name), models };
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
  const skillOk = skillInstalled(input.skillHost) && !needsSkillInstall(input.skillHost, input.bundledVersion);
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
