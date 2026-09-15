import type { TerminalSession } from './store/app-state';
import type { ConsoleRun } from './tauri';

export type ConsoleTabPlan =
  | { kind: 'focus'; sessionId: string }
  | { kind: 'background' }
  | { kind: 'open' };

export function consoleTabPlan(
  sessions: TerminalSession[],
  activeId: string | null,
  graphId: string,
): ConsoleTabPlan {
  const existing = sessions.find((session) => session.tool === 'graph' && session.toolData === graphId);
  const active = sessions.find((session) => session.id === activeId);
  const commanderActive = Boolean(active?.commanderTitle);
  if (existing) return commanderActive ? { kind: 'background' } : { kind: 'focus', sessionId: existing.id };
  return commanderActive ? { kind: 'background' } : { kind: 'open' };
}

export function consoleInflightCount(runs: ConsoleRun[]): number {
  return runs.filter((run) => run.status === 'running')
    .reduce((count, run) => count + run.nodes.filter((node) => node.status === 'running').length, 0);
}
