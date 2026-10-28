// 新用户走查修正的展示纯函数（TASK-58）：未探测状态、探测按钮文字、全部测试的目标
// 与文字、推荐后的未验证提示、评审视角、模型下拉的状态文字、已存在的命令文件。
// 引擎字段见 tasks/TASK-58-newuser-ui.md「引擎契约」。

import type { ResourceAgentEntry, ResourceModelEntry, SlashModeEntry } from '../tauri';
import { canSmoke } from './config-view';

/** 从未探测时 agent 与模型的状态（中性，不算异常）。 */
export const NOT_DETECTED_STATE = '未探测';

/** 版本号的显示：引擎写「未检测」或为空时不显示。 */
export function versionText(version: string): string {
  const value = version.trim();
  return value === '' || value === '未检测' ? '' : value;
}

/** 资源页主按钮：探测中显示进度（拿不到进度事件时「探测中…」）；从未探测为「探测」，
 *  探测过为「重新探测」。 */
export function detectButtonLabel(input: {
  detecting: boolean;
  progress: { done: number; total: number } | null;
  everDetected: boolean;
}): string {
  if (input.detecting) {
    return input.progress && input.progress.total > 0
      ? `探测中 ${input.progress.done}/${input.progress.total}`
      : '探测中…';
  }
  return input.everDetected ? '重新探测' : '探测';
}

/** 「全部测试」的目标：这个 agent 下有「测试连接」按钮的第三方模型（provider 不为空），
 *  即未验证、验证失败、连接超时的，与单条测试一致。 */
export function batchSmokeTargets(agent: Pick<ResourceAgentEntry, 'models'>): ResourceModelEntry[] {
  return agent.models.filter((model) => canSmoke(model) && model.provider.trim() !== '');
}

/** 「全部测试」按钮（TASK-66）：有第三方模型的 agent 始终显示；没有可测的模型时置灰，
 *  reason 作悬停说明。没有第三方模型的 agent 不显示（show 为 false）。 */
export function batchSmokeButton(agent: Pick<ResourceAgentEntry, 'state' | 'models'>): { show: boolean; reason: string } {
  const show = agent.models.some((model) => model.provider.trim() !== '');
  if (!show) return { show, reason: '' };
  if (agent.state === '未安装') return { show, reason: '未安装，无法测试' };
  return { show, reason: batchSmokeTargets(agent).length === 0 ? '没有待测的模型' : '' };
}

export function batchSmokeLabel(running: { done: number; total: number } | null): string {
  return running ? `测试中 ${running.done}/${running.total}` : '全部测试';
}

/** 推荐后的提示：各 agent 未验证、没进候选池的模型数（0 的不列）。 */
export function unverifiedLines(unverified: { agent: string; count: number }[] | null | undefined): { agent: string; text: string }[] {
  return (unverified ?? [])
    .filter((item) => item.count > 0)
    .map((item) => ({ agent: item.agent, text: `${item.agent} 有 ${item.count} 个模型未验证` }));
}

/** 位置行的角色名：有视角时写「验证 · 安全」。 */
export function seatRoleText(role: string, focus: string | null | undefined): string {
  const value = (focus ?? '').trim();
  return value === '' ? role : `${role} · ${value}`;
}

/** 模型下拉里的状态文字：可用、已验证不写，其余（未验证、未发现等）照写。 */
export function optionStateText(state: string): string {
  return state === '可用' || state === '已验证' ? '' : state;
}

export type SlashRowAction =
  | { kind: 'command' }
  | { kind: 'generate'; name: string }
  | { kind: 'register'; name: string }
  | { kind: 'occupied' };

/** 斜杠命令模式行的动作：已登记 → 显示命令；对应文件已存在且是我们写的 → 「登记」
 *  （名称取已有文件）；被占用 → 只显示「文件已被占用」；都没有 → 「生成」。 */
export function slashRowAction(row: Pick<SlashModeEntry, 'command' | 'existing' | 'suggestedName'>): SlashRowAction {
  if (row.command) return { kind: 'command' };
  const existing = row.existing ?? null;
  if (existing && existing.state === '已写入') return { kind: 'register', name: existing.name };
  if (existing && existing.state === '被占用') return { kind: 'occupied' };
  return { kind: 'generate', name: row.suggestedName };
}
