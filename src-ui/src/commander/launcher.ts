// 总指挥启动器的纯逻辑（TASK-10 实现 1/3）。
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

/**
 * agent 显示名。与 `src/resources.rs` 的 AGENT_LABELS 一致——同一个 agent 在
 * 接入页和这里必须同名，用户才能把两处对上。
 */
const AGENT_LABEL: Record<CommanderAgent, string> = {
  claude: 'Claude Code',
  codex: 'Codex',
  kimi: 'Kimi Code CLI',
  pi: 'pi',
};

export function commanderAgentLabel(agent: string): string {
  return isCommanderAgent(agent) ? AGENT_LABEL[agent] : agent;
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
 * 有默认配置才显示的一键行文案：`启动总指挥 · claude · fable · aicross`。
 * agent 为空 = 还没存过默认，返回 null，首屏就不显示这一行。
 * 模型留空时省掉模型那一段，不写一个空的点。
 */
export function commanderDefaultLine(settings: CommanderSettings | null): string | null {
  if (!settings || !isCommanderAgent(settings.agent)) return null;
  const parts = ['启动总指挥', settings.agent];
  const model = settings.model.trim();
  if (model !== '') parts.push(model);
  parts.push(cwdName(settings.cwd) || COMMANDER_NO_CWD);
  return parts.join(' · ');
}

/** tab 标题：`agent · 模型 · 目录名`。模型留空时省掉中间那段。 */
export function commanderTabTitle(agent: string, model: string, cwd: string | null): string {
  const parts = [agent];
  const trimmed = model.trim();
  if (trimmed !== '') parts.push(trimmed);
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

/** 已安装且不属于四家总指挥的 AI CLI。 */
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

/** 空白 graph tab 在显式进入对话图前属于总指挥启动器。 */
export function emptyGraphTabTitle(graphViewSelected: boolean): string {
  return graphViewSelected ? '对话图' : '总指挥';
}

export function shouldShowNoCommanderModels(
  modelsLoaded: boolean,
  modelsError: string | null,
  agent: string,
  modelCount: number,
): boolean {
  return modelsLoaded && modelsError === null && agent !== '' && modelCount === 0;
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
