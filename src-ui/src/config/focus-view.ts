// 评审模式位置视角的纯函数（TASK-60）：下拉选项与自定义视角的校验。契约见
// tasks/TASK-60-focus-ui.md（config_set_focus，空串恢复默认）。

/** 下拉里「自定义…」的值（不会和真实视角重名）。 */
export const FOCUS_CUSTOM = '\u0000custom';
export const FOCUS_MAX = 20;

/** 视角下拉的选项：默认（空串）、引擎给的可选视角；当前值不在其中（自定义过）时补在末尾；
 *  最后是「自定义…」。 */
export function focusChoices(options: readonly string[], current: string): { value: string; label: string }[] {
  const choices = [{ value: '', label: '默认' }, ...options.map((option) => ({ value: option, label: option }))];
  const value = current.trim();
  if (value !== '' && !options.includes(value)) choices.push({ value, label: value });
  choices.push({ value: FOCUS_CUSTOM, label: '自定义…' });
  return choices;
}

/** 自定义视角：去首尾空白后 1–20 字；不合规时返回原因，合规为空串。 */
export function focusError(text: string): string {
  const value = text.trim();
  if (value === '') return '视角为空';
  if (Array.from(value).length > FOCUS_MAX) return `视角超过 ${FOCUS_MAX} 字`;
  return '';
}

/** config_set_focus 的 index：该位置在本模式所有带视角选项的位置里排第几（从 1 起，按阶段
 *  先后）；这个位置不带视角选项时为 0。 */
export function focusIndex(
  stages: readonly (readonly { focusOptions?: readonly string[] }[])[],
  stage: number,
  index: number,
): number {
  let position = 0;
  for (let s = 0; s < stages.length; s += 1) {
    for (let n = 0; n < stages[s].length; n += 1) {
      if ((stages[s][n].focusOptions ?? []).length === 0) continue;
      position += 1;
      if (s === stage - 1 && n === index - 1) return position;
    }
  }
  return 0;
}
