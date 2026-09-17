import type { ConsoleMessage, ConsoleTargetAgent } from './console-session';
import {
  CONSOLE_RUN_LIMIT,
  interruptConsoleRun,
  parseConsoleRunRecord,
} from './console-run-detail';
import type { TerminalSession } from './store/app-state';

export const CONSOLE_MESSAGE_LIMIT = 200;
export const CONSOLE_SESSIONS_STORAGE_KEY = 'aicross:console-sessions:v1';

function storageOrDefault(storage?: Storage): Storage | null {
  if (storage) return storage;
  try {
    return typeof localStorage === 'undefined' ? null : localStorage;
  } catch {
    return null;
  }
}

function targetFrom(value: unknown): ConsoleTargetAgent | undefined {
  if (!value || typeof value !== 'object') return undefined;
  const target = value as Record<string, unknown>;
  if (typeof target.agent !== 'string' || typeof target.provider !== 'string' || typeof target.model !== 'string') {
    return undefined;
  }
  return { agent: target.agent, provider: target.provider, model: target.model };
}

function messageFrom(value: unknown): ConsoleMessage | null {
  if (!value || typeof value !== 'object') return null;
  const message = value as Record<string, unknown>;
  if (typeof message.id !== 'string'
    || (message.kind !== 'user' && message.kind !== 'assistant' && message.kind !== 'system')
    || typeof message.text !== 'string'
    || typeof message.createdAt !== 'string') return null;
  return {
    id: message.id,
    kind: message.kind,
    text: message.text,
    createdAt: message.createdAt,
    ...(typeof message.agent === 'string' ? { agent: message.agent } : {}),
    ...(typeof message.provider === 'string' ? { provider: message.provider } : {}),
    ...(typeof message.model === 'string' ? { model: message.model } : {}),
    ...(typeof message.dispatchModel === 'string' ? { dispatchModel: message.dispatchModel } : {}),
    ...(typeof message.role === 'string' ? { role: message.role } : {}),
    ...(typeof message.durationMs === 'number' && Number.isFinite(message.durationMs)
      ? { durationMs: message.durationMs }
      : {}),
    ...(typeof message.status === 'string' ? { status: message.status } : {}),
    ...(typeof message.route === 'string' ? { route: message.route } : {}),
    ...(typeof message.stage === 'number' && Number.isFinite(message.stage)
      ? { stage: message.stage }
      : {}),
    ...(typeof message.runId === 'string' && message.runId !== '' ? { runId: message.runId } : {}),
  };
}

function resumeIdsFrom(value: unknown): Record<string, string> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
  return Object.fromEntries(Object.entries(value)
    .filter(([key, resumeId]) => key !== '' && typeof resumeId === 'string' && resumeId !== '')) as Record<string, string>;
}

function persistedSession(session: TerminalSession): TerminalSession {
  return {
    id: session.id,
    tool: 'console',
    folderPath: session.folderPath,
    messages: (session.messages ?? []).slice(-CONSOLE_MESSAGE_LIMIT),
    resumeId: resumeIdsFrom(session.resumeId),
    runRecords: (session.runRecords ?? []).slice(-CONSOLE_RUN_LIMIT),
    ...(session.targetAgent ? { targetAgent: { ...session.targetAgent } } : {}),
    ...(session.lastAppliedMessageId ? { lastAppliedMessageId: session.lastAppliedMessageId } : {}),
    ...(session.consoleGraphId ? { consoleGraphId: session.consoleGraphId } : {}),
  };
}

export function persistConsoleSessions(sessions: TerminalSession[], storage?: Storage): void {
  try {
    storageOrDefault(storage)?.setItem(
      CONSOLE_SESSIONS_STORAGE_KEY,
      JSON.stringify(sessions.filter((session) => session.tool === 'console').map(persistedSession)),
    );
  } catch {
    // 隐私窗口或存储配额不足时只放弃跨重启恢复，本次会话继续使用内存状态。
  }
}

export function loadConsoleSessions(storage?: Storage): TerminalSession[] {
  try {
    const raw = storageOrDefault(storage)?.getItem(CONSOLE_SESSIONS_STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed.flatMap((value): TerminalSession[] => {
      if (!value || typeof value !== 'object') return [];
      const session = value as Record<string, unknown>;
      if (typeof session.id !== 'string' || session.id === '') return [];
      const messages = Array.isArray(session.messages)
        ? session.messages.map(messageFrom).filter((message): message is ConsoleMessage => message !== null)
        : [];
      const targetAgent = targetFrom(session.targetAgent);
      const runRecords = Array.isArray(session.runRecords)
        ? session.runRecords
          .map(parseConsoleRunRecord)
          .filter((record) => record !== null)
          .slice(-CONSOLE_RUN_LIMIT)
          .map(interruptConsoleRun)
        : [];
      return [{
        id: session.id,
        tool: 'console',
        folderPath: typeof session.folderPath === 'string' ? session.folderPath : null,
        messages: messages.slice(-CONSOLE_MESSAGE_LIMIT),
        resumeId: resumeIdsFrom(session.resumeId),
        runRecords,
        ...(targetAgent ? { targetAgent } : {}),
        ...(typeof session.lastAppliedMessageId === 'string' && session.lastAppliedMessageId !== ''
          ? { lastAppliedMessageId: session.lastAppliedMessageId }
          : {}),
        ...(typeof session.consoleGraphId === 'string' && session.consoleGraphId !== ''
          ? { consoleGraphId: session.consoleGraphId }
          : {}),
      }];
    });
  } catch {
    return [];
  }
}
