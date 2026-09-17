import type { DispatchableModel, Preset } from './graph/types';
import type { TerminalSession } from './store/app-state';
import { agentDisplayName } from './resources/agent-display';
import { verificationOf } from './resources/types';

export const CONTROL_CONSOLE_LABEL = '控制台';

export interface ConsoleTargetAgent {
  agent: string;
  provider: string;
  model: string;
}

export interface ConsoleMessage {
  id: string;
  kind: 'user' | 'assistant' | 'system';
  text: string;
  createdAt: string;
  agent?: string;
  provider?: string;
  model?: string;
  dispatchModel?: string;
  role?: string;
  durationMs?: number;
  status?: string;
  route?: string;
  /** 本次运行换算后的阶段号；模式结果消息使用。 */
  stage?: number;
  /** 关联的模式运行记录；收尾消息用它打开详情。 */
  runId?: string;
}

export interface ConsoleMentionResult {
  target: ConsoleTargetAgent | null;
  text: string;
  recognized: boolean;
}

export const NEW_TAB_MENU_ITEMS = [
  { kind: 'console' as const, label: CONTROL_CONSOLE_LABEL },
];

export const CONSOLE_APPLY_EVENT = 'aicross:console-apply-preset';

export interface ConsoleApplyDetail {
  sessionId: string;
  preset: Preset;
}

export function dispatchConsoleApply(detail: ConsoleApplyDetail, target: EventTarget = window): void {
  target.dispatchEvent(new CustomEvent<ConsoleApplyDetail>(CONSOLE_APPLY_EVENT, { detail }));
}

export function consoleResumeKey(target: ConsoleTargetAgent): string {
  return JSON.stringify([target.agent, target.provider, target.model]);
}

export function consoleResumeId(
  resumeId: Readonly<Record<string, string>> | undefined,
  target: ConsoleTargetAgent,
): string {
  return resumeId?.[consoleResumeKey(target)] ?? '';
}

export function withConsoleResumeId(
  resumeId: Readonly<Record<string, string>> | undefined,
  target: ConsoleTargetAgent,
  value: string,
): Record<string, string> {
  if (value === '') return { ...(resumeId ?? {}) };
  return { ...(resumeId ?? {}), [consoleResumeKey(target)]: value };
}

export function verifiedConsoleModels(models: DispatchableModel[]): DispatchableModel[] {
  return models.filter((model) => {
    const verification = verificationOf({
      agent: model.agent,
      status: model.status,
      verification: model.verification ?? '',
    });
    return verification === 'native' || verification === 'verified';
  });
}

export function mentionTargets(
  models: DispatchableModel[],
  recent: ConsoleTargetAgent[] = [],
): ConsoleTargetAgent[] {
  const available = verifiedConsoleModels(models);
  const preferred = new Map<string, ConsoleTargetAgent>();
  for (const target of recent) {
    const agent = target.agent.trim().toLowerCase();
    if (agent === '' || preferred.has(agent)) continue;
    const match = available.find((model) => model.agent.trim().toLowerCase() === agent
      && model.id === target.model
      && (target.provider === '' || (model.provider ?? '') === target.provider));
    if (match) {
      preferred.set(agent, { agent: match.agent, provider: match.provider ?? '', model: match.id });
    }
  }
  const seen = new Set<string>();
  return available.flatMap((model) => {
    const agent = model.agent.trim().toLowerCase();
    if (agent === '' || seen.has(agent)) return [];
    seen.add(agent);
    return [preferred.get(agent) ?? { agent: model.agent, provider: model.provider ?? '', model: model.id }];
  });
}

function mentionNames(agent: string): string[] {
  return [...new Set([agent.trim(), agentDisplayName(agent).trim()].filter(Boolean))]
    .sort((left, right) => right.length - left.length);
}

export function parseConsoleMention(
  value: string,
  targets: ConsoleTargetAgent[],
): ConsoleMentionResult {
  if (!value.startsWith('@')) return { target: null, text: value, recognized: false };
  const ordered = targets.flatMap((target) => mentionNames(target.agent).map((name) => ({ target, name })))
    .sort((left, right) => right.name.length - left.name.length);
  for (const { target, name } of ordered) {
    const prefix = `@${name}`;
    if (value.slice(0, prefix.length).toLocaleLowerCase() !== prefix.toLocaleLowerCase()) continue;
    const boundary = value.slice(prefix.length, prefix.length + 1);
    if (boundary !== '' && !/\s/.test(boundary)) continue;
    return {
      target,
      text: value.slice(prefix.length).replace(/^\s+/, ''),
      recognized: true,
    };
  }
  return { target: null, text: value, recognized: false };
}

export function completeConsoleMention(value: string, target: ConsoleTargetAgent): string {
  // 整段替换开头那个 `@词`，不看它和选中的这家是否同名（09-16 复审 Gemini 1）。
  // 原来按选中目标的词去逐词匹配：打了 `@c` 再点「pi agent」，`pi` 不以 `c` 开头，
  // 循环第一轮就断在光标 0，于是 `c` 被当成正文留下，结果是「@pi agent c 」；
  // 打了 `@kimi 帮我看` 再点「Claude Code」会变成「@Claude Code kimi 帮我看」。
  const display = agentDisplayName(target.agent);
  const head = value.match(/^@\S*/)?.[0] ?? '';
  const rest = value.slice(head.length).replace(/^\s+/, '');
  // 已经打到多词显示名的中间（如 `@pi ag`）时，把那截也一起吃掉。
  const wanted = display.toLocaleLowerCase().split(/\s+/);
  let tail = rest;
  for (let index = 1; index < wanted.length; index += 1) {
    const next = tail.match(/^\S+/)?.[0] ?? '';
    if (next === '' || !wanted[index].startsWith(next.toLocaleLowerCase())) break;
    tail = tail.slice(next.length).replace(/^\s+/, '');
  }
  return `@${display} ${tail}`;
}

export function canApplyOrchestration(
  session: TerminalSession | null | undefined,
  hasFocusNode: boolean,
  preset: Preset | null,
): boolean {
  if (!preset) return false;
  if (session?.tool === 'console' && session.targetAgent) return true;
  return session?.tool === 'graph' && hasFocusNode;
}

export function latestConsoleUserMessage(messages: ConsoleMessage[] | undefined): string {
  return [...(messages ?? [])].reverse().find((message) => message.kind === 'user')?.text ?? '';
}

export interface ConsoleApplySelection {
  text: string;
  message: ConsoleMessage | null;
  blocked: 'empty' | 'already-applied' | null;
}

export function selectConsoleApplyMessage(
  draft: string,
  messages: ConsoleMessage[] | undefined,
  lastAppliedMessageId: string | undefined,
): ConsoleApplySelection {
  const current = draft.trim();
  if (current !== '') return { text: current, message: null, blocked: null };
  const message = [...(messages ?? [])].reverse().find((item) => item.kind === 'user') ?? null;
  if (!message) return { text: '', message: null, blocked: 'empty' };
  if (message.id === lastAppliedMessageId) {
    return { text: message.text, message, blocked: 'already-applied' };
  }
  return { text: message.text, message, blocked: null };
}

export function consoleMessage(
  kind: ConsoleMessage['kind'],
  text: string,
  detail: Partial<Omit<ConsoleMessage, 'id' | 'kind' | 'text' | 'createdAt'>> = {},
): ConsoleMessage {
  return {
    id: crypto.randomUUID(),
    kind,
    text,
    createdAt: new Date().toISOString(),
    ...detail,
  };
}
