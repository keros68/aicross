import catchupPrefix from '../../fixtures/prompts/console-catchup-prefix.txt?raw';
import {
  consoleResumeKey,
  parseConsoleMention,
  type ConsoleMessage,
  type ConsoleTargetAgent,
} from './console-session';

export interface ConsoleCatchup {
  /** 发给目标的全文；没有可补内容时等于 outbound 原文 */
  text: string;
  /** 补进去的消息条数（截掉的不算） */
  included: number;
  /** 因总长上限被丢掉的更早消息条数 */
  omitted: number;
}

export const CONSOLE_CATCHUP_PREFIX = catchupPrefix.trim();

// 单条正文上限与头尾保留量；各段合计上限。只在本文件用，不进设置页。
const MESSAGE_CHAR_LIMIT = 4000;
const MESSAGE_HEAD_CHARS = 3000;
const MESSAGE_TAIL_CHARS = 800;
const TOTAL_CHAR_LIMIT = 24000;

function isHighSurrogate(code: number): boolean {
  return code >= 0xd800 && code <= 0xdbff;
}

function isLowSurrogate(code: number): boolean {
  return code >= 0xdc00 && code <= 0xdfff;
}

// 超长正文留头尾。切点不落在代理对中间：半个 emoji 序列化后引擎那侧解析不了。
function clipBody(body: string): string {
  if (body.length <= MESSAGE_CHAR_LIMIT) return body;
  let headEnd = MESSAGE_HEAD_CHARS;
  if (isHighSurrogate(body.charCodeAt(headEnd - 1))) headEnd -= 1;
  let tailStart = body.length - MESSAGE_TAIL_CHARS;
  if (isLowSurrogate(body.charCodeAt(tailStart))) tailStart += 1;
  return `${body.slice(0, headEnd)}…[省略 ${tailStart - headEnd} 字]…${body.slice(tailStart)}`;
}

// 成功的回复：status 为 ok，或旧消息没带 status。
function isOkReply(message: ConsoleMessage): boolean {
  return message.kind === 'assistant' && (message.status === undefined || message.status === 'ok');
}

// 目标会话自己说的。消息上的 model 是显示用的（可能是引擎报回的真实型号），不能拿来认会话，
// 只按 resumeKey 判；没有 resumeKey 的（旧消息、模式派发结果、没成功或走了备选的那一轮）都不算。
function spokenByTarget(message: ConsoleMessage, targetKey: string): boolean {
  return message.kind === 'assistant' && message.resumeKey === targetKey;
}

// 单线聊天的回复该记到哪个续聊会话键上；不该记的返回空串。
// 走了备选或没成功的那一轮，目标会话未必收到内容，不能当「它说过」；
// 没回会话 id 的那一轮没有可续的会话；原生命令（/compact 等）那一轮没补对话，也都不能当。
export function consoleReplyResumeKey(
  target: ConsoleTargetAgent,
  result: { status: string; fallbackTo: string; sessionId: string },
  outbound: string,
): string {
  if (result.status !== 'ok' || result.fallbackTo !== '' || result.sessionId === '') return '';
  if (outbound.startsWith('/')) return '';
  return consoleResumeKey(target);
}

// 一条消息对应的段落；不收的返回空串。标记口径同 graph/context.ts。
function segmentOf(message: ConsoleMessage, mentions: ConsoleTargetAgent[]): string {
  if (message.kind === 'user') {
    const body = parseConsoleMention(message.text, mentions).text;
    // 剥完 @ 后为空的跳过；以 / 开头的是发给某一家的原生命令，别家不用看。
    if (body.trim() === '' || body.trim().startsWith('/')) return '';
    return `[用户]\n${clipBody(body)}`;
  }
  // 没成功的回复正文是「状态：…」提示，不收。
  if (!isOkReply(message) || message.text.trim() === '') return '';
  return `[模型回复 ${routeOf(message)}]\n${clipBody(message.text)}`;
}

// 标记里的 agent/model 取派发目标（同 graph/context.ts）：消息上的 model 是引擎报回的，
// 可能是「kimi 不报真身」这类占位文字。resumeKey 里存着派发目标，有就用它。
function routeOf(message: ConsoleMessage): string {
  let agent = message.agent;
  let model = message.model;
  if (message.resumeKey) {
    try {
      const parsed: unknown = JSON.parse(message.resumeKey);
      if (Array.isArray(parsed) && typeof parsed[0] === 'string' && typeof parsed[2] === 'string') {
        [agent, , model] = parsed as [string, string, string];
      }
    } catch {
      // 键坏了就用消息上的字段。
    }
  }
  return [agent, model].filter(Boolean).join('/') || '模型';
}

// 单线聊天里各家各有续聊会话：把目标没看到的那一段拼在当前消息前面。
// 纯函数，不碰 store、不发请求；messages 不含本次刚输入的这条。
export function projectConsoleCatchup(
  messages: ConsoleMessage[],
  target: ConsoleTargetAgent,
  outbound: string,
  options: { fresh: boolean; mentions: ConsoleTargetAgent[] },
): ConsoleCatchup {
  const untouched: ConsoleCatchup = { text: outbound, included: 0, omitted: 0 };
  // 各家原生命令前面加字会失效。
  if (outbound.startsWith('/')) return untouched;
  const targetKey = consoleResumeKey(target);
  let start = 0;
  if (!options.fresh) {
    for (let index = messages.length - 1; index >= 0; index -= 1) {
      if (!spokenByTarget(messages[index], targetKey)) continue;
      start = index + 1;
      break;
    }
  }
  const segments = messages.slice(start)
    .map((message) => segmentOf(message, options.mentions))
    .filter((segment) => segment !== '');
  let total = segments.reduce((sum, segment) => sum + segment.length, 0);
  let omitted = 0;
  while (total > TOTAL_CHAR_LIMIT && omitted < segments.length) {
    total -= segments[omitted].length;
    omitted += 1;
  }
  const kept = segments.slice(omitted);
  if (kept.length === 0) return { ...untouched, omitted };
  const text = [
    CONSOLE_CATCHUP_PREFIX,
    ...(omitted > 0 ? [`[更早的 ${omitted} 条对话已省略]`] : []),
    ...kept,
    `[当前消息]\n${outbound}`,
  ].join('\n\n');
  return { text, included: kept.length, omitted };
}

// 消息流里的提示：实际发出的那一份带了几条、丢了几条。
export function consoleCatchupNotice(catchup: ConsoleCatchup, displayName: string): string {
  if (catchup.included === 0) return '';
  const dropped = catchup.omitted > 0 ? `（更早的 ${catchup.omitted} 条已省略）` : '';
  return `已附带此前 ${catchup.included} 条对话给 ${displayName}${dropped}`;
}
