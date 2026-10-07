// skill 页「斜杠命令」卡片的纯函数（TASK-53、TASK-54）：名称校验、触发写法预览、
// 状态的显示、按模式列出与「其它」的划分。契约见 tasks/TASK-53-slash-commands.md
// 第 1、3 节与 tasks/TASK-54-slash-per-mode.md 第 2、3 节。

import type { SlashCommand, SlashList, SlashModeEntry } from '../tauri';

/** 名称：小写字母、数字、-，1–32 位（与引擎校验一致）。 */
const SLASH_NAME_RE = /^[a-z0-9-]{1,32}$/;

export const SLASH_NAME_HINT = '只能用小写字母、数字和 -';
export const SLASH_NOTE_MAX = 200;

/** Windows 保留的设备名，不能作文件名。 */
const WINDOWS_RESERVED_RE = /^(con|nul|aux|prn|com[1-9]|lpt[1-9])$/;

/** 名称不合规时的提示；合规为空串（与引擎校验一致）。 */
export function slashNameError(name: string): string {
  if (!SLASH_NAME_RE.test(name)) return SLASH_NAME_HINT;
  if (name === 'ai-cross') return 'ai-cross 为保留名';
  if (WINDOWS_RESERVED_RE.test(name)) return '系统保留名';
  return '';
}

export function slashNameValid(name: string): boolean {
  return slashNameError(name) === '';
}

/** 各主 agent 下自定义命令的输入前缀：claude、pi 为「/」，codex 为「/prompts:」，
 *  kimi 为「/skill:」；未知的按「/」。 */
export function triggerPrefix(host: string): string {
  if (host === 'codex') return '/prompts:';
  if (host === 'kimi') return '/skill:';
  return '/';
}

export function triggerPreview(host: string, name: string): string {
  return `${triggerPrefix(host)}${name}`;
}

export type SlashTone = 'ok' | 'alert' | 'plain';

/** 状态胶囊：已写入常规（生效色），未写入、被占用、名称无效用异常色，其余原样中性。 */
export function slashStateTone(state: string): SlashTone {
  if (state === '已写入') return 'ok';
  if (state === '未写入' || state === '被占用' || state === SLASH_INVALID_STATE) return 'alert';
  return 'plain';
}

/** 引擎对名称不合规的定义给的状态；这一行只给「删除」。 */
export const SLASH_INVALID_STATE = '名称无效';

/** 附加说明只取单行，去掉首尾空白、换行换成空格。 */
export function normalizeSlashNote(note: string): string {
  return note.replace(/\s*[\r\n]+\s*/g, ' ').trim();
}

// ── 按模式列出（TASK-54） ─────────────────────────────────────────────

export const SLASH_DEFAULT_PREFIX = 'ax-';

export interface SlashModeRow extends SlashModeEntry {
  /** 没有命令时建议名的触发写法预览，如 /prompts:ax-blind；有命令时为空串。 */
  suggestedTrigger: string;
}

/** 卡片里每个内置模式一行；旧版引擎没有 modes 时为空。 */
export function slashModeRows(list: Pick<SlashList, 'host' | 'modes'> | null | undefined): SlashModeRow[] {
  return (list?.modes ?? []).map((entry) => ({
    ...entry,
    suggestedTrigger: entry.command ? '' : triggerPreview(list?.host ?? '', entry.suggestedName),
  }));
}

/** 「全部生成」可点：有内置模式行，且至少一个模式还没有命令。 */
export function canAddAll(list: Pick<SlashList, 'modes'> | null | undefined): boolean {
  const modes = list?.modes ?? [];
  return modes.length > 0 && modes.some((entry) => entry.command === null);
}

/** 「其它」小节：不是任何模式行所显示的那条命令（自定义模式的命令、同一模式的额外名称）。 */
export function otherSlashCommands(list: Pick<SlashList, 'commands' | 'modes'> | null | undefined): SlashCommand[] {
  const shown = new Set((list?.modes ?? []).flatMap((entry) => (entry.command ? [entry.command.name] : [])));
  return (list?.commands ?? []).filter((command) => !shown.has(command.name));
}

/** 文件内容旧了、需要重写的命令（TASK-60，引擎的 stale 字段）。 */
export function staleCommands(list: Pick<SlashList, 'commands'> | null | undefined): SlashCommand[] {
  return (list?.commands ?? []).filter((command) => command.stale === true);
}
