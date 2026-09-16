// 控制台启动器的纯逻辑（TASK-10 实现 1/3）。
//
// 组件只负责画，挑什么、显示成什么字都在这里，单测直接测这些函数。

import type { PickableModel } from '../resources/model-picker';
import { modelStatusSuffix } from '../resources/model-picker';
import {
  COMMANDER_AGENTS,
  commanderAgentOfTool,
  commanderIdentifierError,
  commanderToolId,
  isCommanderAgent,
  type CommanderAgent,
} from './args';
import type { CommanderSettings } from './types';
import { agentDisplayName } from '../resources/agent-display';
import { CONTROL_CONSOLE_LABEL } from '../console-session';

/** 启动器卡片网格的唯一尺寸定义，组件以 CSS 变量传给样式表。 */
export const COMMANDER_AGENT_GRID_TEMPLATE = 'repeat(auto-fit, minmax(150px, 150px))';
export const COMMANDER_AGENT_CARD_HEIGHT = 48;

/**
 * agent 显示名。与 `src/resources.rs` 的 AGENT_LABELS 一致——同一个 agent 在
 * 接入页和这里必须同名，用户才能把两处对上。
 */
export function commanderAgentLabel(agent: string): string {
  return agentDisplayName(agent);
}

/**
 * 可选的 agent：四家里本机装了的那些。
 *
 * `installed` 是 CenterPanel 的 `toolsInstalled`，按**工具 id** 存
 * （kimi 在那边叫 `kimicode`），所以要过 `commanderToolId` 再查。
 * 探测还没回来时该键是 `undefined`——按「没装」算，宁可少列一张卡，
 * 也不列一张点了起不来的。
 */
export function availableCommanderAgents(
  installed: Record<string, boolean>,
): CommanderAgent[] {
  return COMMANDER_AGENTS.filter((agent) => {
    const tool = commanderToolId(agent);
    return tool !== null && installed[tool] === true;
  });
}

/** 该 agent 名下的模型，顺序沿用后端排好的（原生/已验证在前）。 */
export function commanderModelsFor<T extends PickableModel>(
  models: T[],
  agent: string,
): T[] {
  return models.filter((model) => model.agent.trim() === agent.trim());
}

/**
 * 下拉里的模型文本。
 *
 * - pi：`provider / model`——一家挂多个 provider，只写模型名认不出是哪条。
 * - claude：别名 + 真实 ID。真实 ID 只有冒烟核对过真身才有（`smoke_model`），
 *   官方端点不核对，所以多数时候就只有别名，不编一个 ID 出来。
 * - 其余：模型 id。
 *
 * 未验证 / 验证失败的后缀沿用 `modelStatusSuffix`，与 G1 的下拉同一套口径。
 */
export function commanderModelLabel(model: PickableModel & { smoke_model?: string }): string {
  const agent = model.agent.trim();
  const suffix = modelStatusSuffix(model);
  if (agent === 'pi') {
    const provider = model.provider.trim();
    const head = provider === '' ? model.id : `${provider} / ${model.id}`;
    return `${head}${suffix}`;
  }
  if (agent === 'claude') {
    const identity = (model.smoke_model ?? '').trim();
    const head = identity === '' || identity === model.id ? model.id : `${model.id}（${identity}）`;
    return `${head}${suffix}`;
  }
  return `${model.id}${suffix}`;
}

/** 目录显示名：末级目录名。空目录返回空串，调用方自己决定占位文案。 */
export function cwdName(cwd: string | null | undefined): string {
  const original = cwd ?? '';
  if (/^[A-Za-z]:[\\/]*$/.test(original)) return original.slice(0, 2);
  if (/^[\\/]+$/.test(original)) return '/';
  const trimmed = original.replace(/[\\/]+$/, '');
  if (trimmed === '') return '';
  const cut = Math.max(trimmed.lastIndexOf('/'), trimmed.lastIndexOf('\\'));
  return cut >= 0 ? trimmed.slice(cut + 1) : trimmed;
}

/** 目录没选时的占位。 */
export const COMMANDER_NO_CWD = '未选目录';

/**
 * 有默认配置才显示的一键行文案：`启动控制台 · Claude Code · fable · aicross`。
 * agent 为空 = 还没存过默认，返回 null，首屏就不显示这一行。
 * 模型留空时省掉模型那一段，不写一个空的点。
 */
export function commanderDefaultLine(settings: CommanderSettings | null): string | null {
  if (!settings || !isCommanderAgent(settings.agent)) return null;
  const parts = ['进入控制台', commanderAgentLabel(settings.agent)];
  const model = settings.model.trim();
  if (model !== '') {
    const provider = settings.provider.trim();
    parts.push(settings.agent === 'pi' && provider !== '' ? `${provider} / ${model}` : model);
  }
  parts.push(cwdName(settings.cwd) || COMMANDER_NO_CWD);
  return parts.join(' · ');
}

/** tab 标题：`agent 显示名 · 模型 · 目录名`。pi 的模型段带 provider。 */
export function commanderTabTitle(
  agent: string,
  provider: string,
  model: string,
  cwd: string | null,
): string {
  const parts = [commanderAgentLabel(agent)];
  const trimmed = model.trim();
  if (trimmed !== '') {
    const providerName = provider.trim();
    parts.push(agent === 'pi' && providerName !== '' ? `${providerName} / ${trimmed}` : trimmed);
  }
  parts.push(cwdName(cwd) || COMMANDER_NO_CWD);
  return parts.join(' · ');
}

/**
 * 下拉 option 的 value：provider 与 id 都可能带斜杠，拼字符串会歧义，
 * 与 `model-picker` 的做法一致用 JSON 三元组。
 */
export function commanderModelValue(model: PickableModel): string {
  return JSON.stringify({ provider: model.provider, model: model.id });
}

export function parseCommanderModelValue(
  value: string,
): { provider: string; model: string } | null {
  try {
    const parsed = JSON.parse(value) as { provider?: unknown; model?: unknown };
    if (typeof parsed.provider !== 'string' || typeof parsed.model !== 'string') return null;
    return { provider: parsed.provider, model: parsed.model };
  } catch {
    return null;
  }
}

/**
 * 启动前的自检：能起返回 null，不能起返回一句说明。
 * 目录不做存在性检查——后端 `tier_terminal_start` 已经在解析 cwd 时兜底，
 * 这里再查一次要多一次 IPC，而且会和后端的判断打架。
 */
export function commanderBlockReason(
  agent: string,
  availableAgents: CommanderAgent[],
  provider = '',
  model = '',
): string | null {
  if (!isCommanderAgent(agent)) return '先选一个 agent';
  if (!availableAgents.includes(agent)) return `本机没探测到 ${commanderAgentLabel(agent)}`;
  return commanderIdentifierError(provider, model);
}

export interface CommanderCliEntry {
  key: string;
  label: string;
}

/** 已安装且不属于四家控制台的 AI CLI。 */
export function otherInstalledCommanderClis(
  catalog: { key: string | null; label: string }[],
  installed: Record<string, boolean>,
): CommanderCliEntry[] {
  return catalog.flatMap((item) => (
    item.key !== null && installed[item.key] === true && commanderAgentOfTool(item.key) === null
      ? [{ key: item.key, label: item.label }]
      : []
  ));
}

/** 空白 graph tab 在显式进入对话图前属于控制台启动器。 */
export function emptyGraphTabTitle(graphViewSelected: boolean): string {
  return graphViewSelected ? '对话图' : CONTROL_CONSOLE_LABEL;
}

// 对话图 tab 的标题（R1 复审）：已开图的按图标题，空标题仍是「对话图」；
// 只有还没进图的空白 tab 才按 启动器 / 对话图 判。
export function graphTabTitle(toolData: string | null | undefined, graphTitle: string | undefined, graphViewSelected: boolean): string {
  if (toolData) return graphTitle || '对话图';
  return emptyGraphTabTitle(graphViewSelected);
}

export function shouldShowNoCommanderModels(
  modelsLoaded: boolean,
  modelsError: string | null,
  agent: string,
  modelCount: number,
): boolean {
  return modelsLoaded && modelsError === null && agent !== '' && modelCount === 0;
}

// 空白控制台 tab 什么时候不画启动器（R1 复审 1）：只有「可能直启」时才不画——
// 默认还没读到、或默认有效且（探测未完成 / 探测完可用且该 tab 还没登记过）。
// 默认读到了但没有可用默认时立刻画启动器，探测期间由启动器自己显示「探测中…」。
/**
 * 启动器什么时候**不**显示。
 *
 * 09-16 用户裁决：**永远先让用户自己选 agent，不要用上次的默认直接把会话起起来。**
 * 原来的规则是「有可用默认就直接起」（09-15 定案），实测下来每开一个新 tab 都会
 * 无声地起一个 codex——用户想换一家得先把它关掉，等于替用户做了决定。
 *
 * 现在只在两种「还不知道该显示什么」的过渡态下隐藏：设置还没读回来（`settings === null`），
 * 或者本机 agent 还没扫完（`!toolsScanned`）。这两种都是一帧的事，不是直启。
 * 上次的选择仍然有用——启动器会把它作为预选项填好，回车即可，只是不再替用户按下去。
 */
export function commanderLauncherHidden(
  settings: CommanderSettings | null,
  // 不再参与判定（09-16 起不按默认直启），但签名保持不变：调用点与测试都按四个
  // 参数传，改签名会牵动一串无关文件。
  _installed: Record<string, boolean>,
  toolsScanned: boolean,
  tabRegistered: boolean,
): boolean {
  if (settings === null) return true;
  if (!isCommanderAgent(settings.agent)) return false;
  if (!toolsScanned) return true;
  // 这个 tab 已经起过会话了，就别再把启动器盖回去。
  return tabRegistered;
}

export function commanderDefaultAvailable(
  settings: CommanderSettings | null,
  installed: Record<string, boolean>,
  toolsScanned: boolean,
): boolean {
  if (!toolsScanned || settings === null || !isCommanderAgent(settings.agent)) return false;
  const tool = commanderToolId(settings.agent);
  return tool !== null && installed[tool] === true
    && commanderIdentifierError(settings.provider, settings.model) === null;
}

/** 新控制台 tab 的首屏判定：默认完整、探测完成且 CLI 已安装就直接启动。 */
export function commanderNewTabLaunch(
  settings: CommanderSettings | null,
  installed: Record<string, boolean>,
  toolsScanned: boolean,
  folderPath: string | null,
): { agent: CommanderAgent; provider: string; model: string; cwd: string } | null {
  if (!commanderDefaultAvailable(settings, installed, toolsScanned) || settings === null) return null;
  return {
    agent: settings.agent as CommanderAgent,
    provider: settings.provider,
    model: settings.model,
    cwd: folderPath || settings.cwd,
  };
}

export type CommanderTabStatus = 'started' | 'launcher' | 'changed';
export type CommanderTabState = Readonly<Record<string, CommanderTabStatus>>;
export type CommanderTabAction =
  | { type: 'availability'; tabId: string; available: boolean }
  | { type: 'launcher-shown'; tabId: string }
  | { type: 'changed'; tabId: string };

/**
 * 空白控制台 tab 的一次性状态机。只有尚未显示启动器的 tab
 * 能被可用默认启动；已显示启动器或点过「更改」的 tab 不再被接管。
 */
export function commanderTabTransition(
  state: CommanderTabState,
  action: CommanderTabAction,
): { state: CommanderTabState; shouldLaunch: boolean } {
  const current = state[action.tabId];
  if (action.type === 'availability') {
    if (!action.available || current !== undefined) return { state, shouldLaunch: false };
    return { state: { ...state, [action.tabId]: 'started' }, shouldLaunch: true };
  }
  if (current === 'started' && action.type === 'launcher-shown') return { state, shouldLaunch: false };
  const status = action.type === 'changed' ? 'changed' : 'launcher';
  return { state: { ...state, [action.tabId]: status }, shouldLaunch: false };
}

export function openCommanderCli(tool: string, onOpen: (tool: string) => void): void {
  onOpen(tool);
}

export function markCommanderSelection<T>(
  seeded: { current: boolean },
  setValue: (value: T) => void,
  value: T,
): void {
  seeded.current = true;
  setValue(value);
}

export function commanderModelSelectValue(
  picked: { provider: string; model: string } | null,
): string {
  if (picked === null) return '';
  return JSON.stringify({ provider: picked.provider, model: picked.model });
}

/** 只有仍带待保存快照的 tab 才注册启动成功回调；消费后重启不会再次保存。 */
export function commanderStartedCallback(
  session: { id: string; commanderSettings?: CommanderSettings },
  remember: (id: string, settings: CommanderSettings) => void,
): (() => void) | undefined {
  if (session.commanderSettings === undefined) return undefined;
  const settings = session.commanderSettings;
  return () => remember(session.id, settings);
}
