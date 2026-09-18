// 控制台单线聊天走 ACP 后多出的两样东西（09-18）：`/` 命令补全与权限卡片。
// 引擎推两个事件：`acp-commands`（某家 agent 的命令清单）与 `acp-permission`
// （某一路要调执行类 / MCP 工具，等用户点）；卡片关闭另推 `acp-permission-closed`。
import type { ConsoleTargetAgent } from './console-session';
import { parseConsoleMention } from './console-session';

export interface AcpCommand {
  name: string;
  description: string;
  hint: string;
}

export interface AcpPermissionOption {
  optionId: string;
  name: string;
  kind: 'allow_once' | 'reject_once' | string;
}

export interface AcpPermissionRequest {
  requestId: string;
  /** `run_id:node_id`；控制台按本轮 runId 前缀认领。 */
  dispatchKey: string;
  agent: string;
  title: string;
  kind: string;
  detail: string;
  options: AcpPermissionOption[];
}

export const ACP_COMMANDS_EVENT = 'acp-commands';
export const ACP_PERMISSION_EVENT = 'acp-permission';
export const ACP_PERMISSION_CLOSED_EVENT = 'acp-permission-closed';

/** 走 ACP 的 agent 才有命令清单；pi 等一次性通道没有。 */
export function supportsSlashCommands(agent: string): boolean {
  return ['claude', 'codex', 'kimi'].includes(agent.trim().toLowerCase());
}

export interface SlashQuery {
  target: ConsoleTargetAgent;
  /** 已打的命令词（不含开头的 `/`），可为空串。 */
  prefix: string;
  /** 命令词之前保留原样的部分（`@kimi ` 这类改派前缀）。 */
  head: string;
}

// 只在「整条输入就是一个还没打完的命令词」时弹补全：`/`、`/com`、`@kimi /sk`。
// 命令词后面一旦有空格，说明在写参数，补全收起。
export function slashQuery(
  draft: string,
  current: ConsoleTargetAgent,
  mentions: ConsoleTargetAgent[],
): SlashQuery | null {
  if (draft.includes('\n')) return null;
  const parsed = parseConsoleMention(draft, mentions);
  const body = parsed.recognized ? parsed.text : draft;
  const head = draft.slice(0, draft.length - body.length);
  const match = body.match(/^\/(\S*)$/);
  if (!match) return null;
  return { target: parsed.target ?? current, prefix: match[1], head };
}

/** 前缀匹配排前，名字里含有的排后；最多 8 条。 */
export function filterCommands(commands: AcpCommand[], prefix: string, limit = 8): AcpCommand[] {
  const wanted = prefix.toLowerCase().replace(/^\$/, '');
  const bare = (command: AcpCommand) => command.name.toLowerCase().replace(/^\$/, '');
  const starts = commands.filter((command) => bare(command).startsWith(wanted));
  const contains = wanted === ''
    ? []
    : commands.filter((command) => !bare(command).startsWith(wanted) && bare(command).includes(wanted));
  return [...starts, ...contains].slice(0, limit);
}

/** codex 的技能以 `$名字` 出现，按原样插入；其余命令插成 `/名字 `。 */
export function commandText(command: AcpCommand): string {
  return command.name.startsWith('$') ? command.name : `/${command.name}`;
}

export function completeSlashCommand(query: SlashQuery, command: AcpCommand): string {
  return `${query.head}${commandText(command)} `;
}

// 续聊失效后的重派用 `${runId}-fresh` 作 runId，同样归这一轮。
export function permissionBelongsTo(request: AcpPermissionRequest, runId: string): boolean {
  if (runId === '') return false;
  return request.dispatchKey.startsWith(`${runId}:`) || request.dispatchKey.startsWith(`${runId}-fresh:`);
}

// 与 dispatch-progress 的订阅同一写法：异步 import + listen 之间组件可能已卸载，
// 用 disposed 标志在迟到的 resolve 里就地解除。
export function subscribeAcpEvent<T>(
  event: string,
  onEvent: (payload: T) => void,
): { dispose: () => void } {
  let disposed = false;
  let unlisten: (() => void) | undefined;
  import('@tauri-apps/api/event')
    .then(({ listen }) => listen<T>(event, (message) => onEvent(message.payload)))
    .then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    })
    .catch((error) => { console.warn(`${event} 订阅失败`, error); });
  return {
    dispose: () => {
      disposed = true;
      unlisten?.();
      unlisten = undefined;
    },
  };
}
