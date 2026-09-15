// `~/.aicross/commander.json` 的前端镜像（TASK-10 契约）。
// 字段名与 `src/orchestration.rs` 的 `CommanderSettings` 逐字段对应，
// snake_case 是跨端契约，改名必须两边同步。

export interface CommanderSettings {
  version: 1;
  agent: string;
  provider: string;
  model: string;
  cwd: string;
  updated_at: string;
}

/** 还没存过默认时的空值：agent 为空即「没有默认」。 */
export function emptyCommanderSettings(): CommanderSettings {
  return { version: 1, agent: '', provider: '', model: '', cwd: '', updated_at: '' };
}
