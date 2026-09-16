// 控制台的启动参数映射（TASK-10 契约）。
//
// 纯函数，不碰 IPC、不读全局状态：给一个 agent + provider + model，算出这次
// 启动要额外追加的 argv。后端把它接在工具设置的 `extra_args` 之后（见
// `src/server.rs` 的 per-launch extra args 段），所以这里只管模型这一段。
//
// 「壳 ≠ 模型」：agent 决定用哪个参数名，模型才是派发目标。四家参数名各不
// 相同且会漂，改这里之前先看各家 `--help`，别照抄记忆。

/** 资源库口径的 agent id（`src/resources.rs` 的 NATIVE_AGENTS + pi）。 */
export type CommanderAgent = 'claude' | 'codex' | 'kimi' | 'pi';

export const COMMANDER_AGENTS: CommanderAgent[] = ['claude', 'codex', 'kimi', 'pi'];

/**
 * 资源库 agent id → 启动 PTY 用的工具 id（`ToolType` / Rust 工具注册表）。
 * 只有 kimi 两边不同名：资源库叫 `kimi`，工具注册表叫 `kimicode`。
 */
const TOOL_BY_AGENT: Record<CommanderAgent, string> = {
  claude: 'claude',
  codex: 'codex',
  kimi: 'kimicode',
  pi: 'pi',
};

/** 反向表，从 tab 的工具 id 找回资源库 agent id。 */
const AGENT_BY_TOOL: Record<string, CommanderAgent> = {
  claude: 'claude',
  codex: 'codex',
  kimicode: 'kimi',
  pi: 'pi',
};

export function isCommanderAgent(value: string): value is CommanderAgent {
  return (COMMANDER_AGENTS as string[]).includes(value);
}

/** agent id → 启动用的工具 id；不认识的 agent 返回 null（不猜）。 */
export function commanderToolId(agent: string): string | null {
  return isCommanderAgent(agent) ? TOOL_BY_AGENT[agent] : null;
}

/** 工具 id → agent id；不是控制台支持的四家则返回 null。 */
export function commanderAgentOfTool(tool: string | null | undefined): CommanderAgent | null {
  return AGENT_BY_TOOL[(tool ?? '').trim()] ?? null;
}

/**
 * 模型 / provider 都是 CLI 标识符，不是自由文本。空串表示不指定；非空值必须
 * 原样满足白名单，不能靠 trim 悄悄修正。前导 `-` 也拒绝，避免值被 CLI 当旗标。
 */
export function isCommanderIdentifier(value: string): boolean {
  return value === '' || (!value.startsWith('-') && /^[A-Za-z0-9._:/@+-]+$/.test(value));
}

export function commanderIdentifierError(provider: string, model: string): string | null {
  if (!isCommanderIdentifier(model)) return '模型 id 含非法字符';
  if (!isCommanderIdentifier(provider)) return 'provider id 含非法字符';
  return null;
}

/** 各家的模型参数名。claude / kimi 用 `--model`，codex 用 `-m`。 */
const MODEL_FLAG: Record<CommanderAgent, string> = {
  claude: '--model',
  codex: '-m',
  kimi: '--model',
  pi: '--model',
};

/**
 * 这次启动要追加的 argv。
 *
 * - claude → `["--model", model]`
 * - codex  → `["-m", model]`
 * - kimi   → `["--model", model]`
 * - pi     → `["--provider", provider, "--model", model]`
 *
 * 模型为空 → 不加模型参数（起一个不指定模型的原生会话，CLI 用它自己的默认）。
 * pi 的 provider 为空时只发 `--model`：provider 是 pi 独有的一层，空着就让
 * pi 自己按它的 models-store 解析，硬塞一个空串会让它报参数错。
 * 不认识的 agent 返回空数组，绝不猜一个参数名出去。
 */
export function commanderArgs(agent: string, provider: string, model: string): string[] {
  if (!isCommanderAgent(agent) || commanderIdentifierError(provider, model) !== null) return [];
  const args: string[] = [];
  if (agent === 'pi' && provider !== '') {
    args.push('--provider', provider);
  }
  if (model !== '') {
    args.push(MODEL_FLAG[agent], model);
  }
  return args;
}
