// 派发设置对话框的展示与校验纯函数（TASK-89）：对话框一页改完
// dispatch-settings.json 里桌面端要管的部分。数值键的区间镜像
// src/engine/mod.rs 的 settings_from_value；保存走 dispatch_save（与
// `aicross config set` 同一实现体），引擎仍按同一套规则夹取，这里先拦住，
// 让输入框里就见不到非法值。

import type { DispatchSettings } from '../tauri';

// ── 字段定义 ───────────────────────────────────────────────

type NumberKey =
  | 'concurrency'
  | 'consoleRunLimit'
  | 'maxRequests'
  | 'fixRoundLimit'
  | 'swarmLimit'
  | 'timeoutSecs'
  | 'execTimeoutSecs'
  | 'idleTimeoutSecs';
type BoolKey = 'redact' | 'writeProjectDispatch' | 'acpEnabled';
type TextKey = 'validationCommand';

/** 小区间的数值键用分段选择，天然不越界。 */
export interface SegmentedField {
  kind: 'segmented';
  key: NumberKey;
  label: string;
  hint: string;
  choices: number[];
}

/** 数值键：scale 是显示值到存储值的倍率（超时按分钟显示，存储秒）。min/max
 *  按引擎区间换算到显示单位（超时 1–3600 秒 → 1/60–60 分钟），已存的合法值
 *  不会在对话框里报错。 */
export interface NumberField {
  kind: 'number';
  key: NumberKey;
  label: string;
  hint: string;
  min: number;
  max: number;
  /** 输入框右侧的单位。 */
  unit: string;
  /** 显示值 × scale = 存储值。 */
  scale: number;
  integer: boolean;
}

export interface BoolField {
  kind: 'bool';
  key: BoolKey;
  label: string;
  hint: string;
}

export interface TextField {
  kind: 'text';
  key: TextKey;
  label: string;
  hint: string;
}

export type SettingsField = SegmentedField | NumberField | BoolField | TextField;

export const SETTINGS_SECTIONS: ReadonlyArray<{ title: string; fields: ReadonlyArray<SettingsField> }> = [
  {
    title: '上限',
    fields: [
      { kind: 'segmented', key: 'concurrency', label: '并发上限', hint: '同一阶段同时运行的位置数', choices: [1, 2, 3] },
      { kind: 'segmented', key: 'consoleRunLimit', label: '同时派发上限', hint: '同时进行的派发次数，到顶后新的派发被拒绝', choices: [1, 2, 3] },
      { kind: 'number', key: 'maxRequests', label: '单次派发上限', hint: '一次派发包含的位置数', min: 1, max: 100, unit: '路', scale: 1, integer: true },
      { kind: 'segmented', key: 'fixRoundLimit', label: '修正轮上限', hint: '验收或验证未通过时最多重派的次数', choices: [0, 1, 2, 3] },
      { kind: 'segmented', key: 'swarmLimit', label: '蜂群上限', hint: '蜂群模式一次最多拆出的件数', choices: [1, 2, 3, 4] },
    ],
  },
  {
    title: '超时',
    fields: [
      { kind: 'number', key: 'timeoutSecs', label: '只读位置超时', hint: '只读与盲审位置的总时长', min: 1 / 60, max: 60, unit: '分钟', scale: 60, integer: false },
      { kind: 'number', key: 'execTimeoutSecs', label: '可写位置超时', hint: '可写位置的总时长', min: 1 / 60, max: 60, unit: '分钟', scale: 60, integer: false },
      { kind: 'number', key: 'idleTimeoutSecs', label: '连续无输出超时', hint: '连续没有输出到这个时长按超时结束', min: 1 / 60, max: 60, unit: '分钟', scale: 60, integer: false },
    ],
  },
  {
    title: '其它',
    fields: [
      { kind: 'bool', key: 'redact', label: '派发内容脱敏', hint: '发送前把密钥、令牌等常见形态替换为占位符' },
      { kind: 'bool', key: 'writeProjectDispatch', label: '派发记录写进项目目录', hint: '不勾时只写 aicross 的数据目录' },
      { kind: 'text', key: 'validationCommand', label: '验证命令', hint: '可写执行位置跑完后在项目里运行的命令；留空不验证' },
      { kind: 'bool', key: 'acpEnabled', label: 'ACP 续聊', hint: '开启后保存时联网安装适配器；装不上的派发回退一次性 CLI' },
    ],
  },
];

// ── 草稿 ─────────────────────────────────────────────────

/** 数值键存字符串（输入中可能不完整），分段与布尔存原生值，文本存字符串。 */
export type SettingsDraft = Record<string, string | number | boolean>;

/** 从读到的设置生成草稿初值：数值键按 scale 换算成显示值。 */
export function initialDraft(settings: DispatchSettings): SettingsDraft {
  const draft: SettingsDraft = {};
  for (const { fields } of SETTINGS_SECTIONS) {
    for (const field of fields) {
      if (field.kind === 'number') {
        draft[field.key] = scaleText(settings[field.key], field.scale);
      } else {
        draft[field.key] = settings[field.key];
      }
    }
  }
  return draft;
}

/** 数值输入的校验：合法返回空串，否则返回一句说明（上限按显示单位，下限按
 *  存储单位，如「最长 60 分钟」「最短 1 秒」）。 */
export function numberInputError(field: NumberField, raw: string): string {
  const text = raw.trim();
  if (text === '') return '请填写';
  const value = Number(text);
  if (!Number.isFinite(value)) return '要填数字';
  if (field.integer && !Number.isInteger(value)) return '要填整数';
  if (value > field.max) return `最长 ${field.max} ${field.unit}`;
  if (value < field.min) {
    return field.scale === 1
      ? `最少 ${field.min} ${field.unit}`
      : `最短 ${Math.max(1, Math.round(field.min * field.scale))} 秒`;
  }
  return '';
}

/** 数值草稿换算成存储值（×scale 取整）。调用前先用 numberInputError 校验。 */
function draftNumber(field: NumberField, raw: string): number {
  return Math.round(Number(raw) * field.scale);
}

export type SettingsPatch = Record<string, number | boolean | string>;

/** 草稿 → 保存的补丁：只带改动的键；任一数值键不合法时返回 null（保存按钮停用）。
 *  文本键按去掉首尾空白后比较与写入。 */
export function settingsPatch(draft: SettingsDraft, settings: DispatchSettings): SettingsPatch | null {
  const patch: SettingsPatch = {};
  for (const { fields } of SETTINGS_SECTIONS) {
    for (const field of fields) {
      const current = draft[field.key];
      if (field.kind === 'number') {
        const raw = typeof current === 'string' ? current : String(current);
        if (numberInputError(field, raw) !== '') return null;
        const value = draftNumber(field, raw);
        if (value !== settings[field.key]) patch[field.key] = value;
      } else if (field.kind === 'text') {
        const value = (typeof current === 'string' ? current : '').trim();
        if (value !== settings[field.key]) patch[field.key] = value;
      } else if (current !== settings[field.key]) {
        patch[field.key] = current;
      }
    }
  }
  return patch;
}

/** 草稿里有改动的键数（补丁为 null 时视为不可保存）。 */
export function changedCount(patch: SettingsPatch | null): number {
  return patch === null ? 0 : Object.keys(patch).length;
}

// ── ACP 适配器状态 ─────────────────────────────────────

/** 适配器状态一行：如「claude 已就绪，codex 未安装」；空清单为空串。 */
export function acpStatusText(reports: { agent: string; state: string }[] | null | undefined): string {
  const parts = (reports ?? []).map((report) => `${report.agent} ${report.state}`);
  return parts.length === 0 ? '' : `适配器：${parts.join('，')}`;
}

// ── 单位换算（超时按分钟展示） ───────────────────────

/** 存储值 → 显示值文本：尽量写短（600 秒 → 「10」），除不尽时保留三位小数
 *  （610 秒 → 「10.167」），保证显示值换算回去仍是原值，不造成假改动。 */
export function scaleText(value: number, scale: number): string {
  return String(Number((value / scale).toFixed(3)));
}
