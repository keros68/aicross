// 派发设置对话框的纯函数测试（TASK-89）：区间镜像引擎、分钟换算、补丁只带
// 改动键、非法输入拦在保存前。

import { describe, expect, it } from 'vitest';
import {
  SETTINGS_SECTIONS,
  acpStatusText,
  changedCount,
  initialDraft,
  numberInputError,
  scaleText,
  settingsPatch,
  type NumberField,
} from '../settings-view';
import type { DispatchSettings } from '../../tauri';

function settings(overrides: Partial<DispatchSettings> = {}): DispatchSettings {
  return {
    timeoutSecs: 600,
    execTimeoutSecs: 1800,
    idleTimeoutSecs: 300,
    concurrency: 3,
    maxRequests: 8,
    swarmLimit: 4,
    consoleRunLimit: 3,
    fixRoundLimit: 1,
    redact: true,
    writeProjectDispatch: true,
    ccSwitchPath: '',
    validationCommand: '',
    acpEnabled: false,
    classificationModel: '',
    classifyProposals: false,
    ...overrides,
  };
}

/** 数值字段区间要镜像 src/engine/mod.rs 的 settings_from_value：改区间时两边一起改。 */
const ENGINE_RANGES: Partial<Record<string, [number, number]>> = {
  concurrency: [1, 3],
  consoleRunLimit: [1, 3],
  maxRequests: [1, 100],
  fixRoundLimit: [0, 3],
  swarmLimit: [1, 4],
  timeoutSecs: [1, 3600],
  execTimeoutSecs: [1, 3600],
  idleTimeoutSecs: [1, 3600],
};

describe('派发设置字段定义（TASK-89）', () => {
  it('数值字段区间镜像引擎夹取（换算到显示单位）', () => {
    for (const { fields } of SETTINGS_SECTIONS) {
      for (const field of fields) {
        if (field.kind === 'segmented') {
          const [low, high] = ENGINE_RANGES[field.key] ?? [NaN, NaN];
          expect(Math.min(...field.choices), field.key).toBe(low);
          expect(Math.max(...field.choices), field.key).toBe(high);
        } else if (field.kind === 'number') {
          const [low, high] = ENGINE_RANGES[field.key] ?? [NaN, NaN];
          expect(field.min * field.scale, field.key).toBe(low);
          expect(field.max * field.scale, field.key).toBe(high);
        }
      }
    }
  });

  it('字段键不重复', () => {
    const keys = SETTINGS_SECTIONS.flatMap(({ fields }) => fields.map((field) => field.key));
    expect(new Set(keys).size).toBe(keys.length);
  });
});

describe('草稿初值与换算', () => {
  it('默认值按分钟显示，秒整除时不带小数', () => {
    const draft = initialDraft(settings());
    expect(draft.timeoutSecs).toBe('10');
    expect(draft.execTimeoutSecs).toBe('30');
    expect(draft.idleTimeoutSecs).toBe('5');
    expect(draft.concurrency).toBe(3);
    expect(draft.redact).toBe(true);
    expect(draft.validationCommand).toBe('');
  });

  it('除不尽的秒保留三位小数，换算回秒仍是原值（不造成假改动）', () => {
    expect(scaleText(630, 60)).toBe('10.5');
    expect(scaleText(610, 60)).toBe('10.167');
    const draft = initialDraft(settings({ timeoutSecs: 610 }));
    expect(settingsPatch(draft, settings({ timeoutSecs: 610 }))).toEqual({});
  });

  it('分钟以下的秒数照常显示，不算非法', () => {
    const base = settings({ timeoutSecs: 30 });
    const draft = initialDraft(base);
    expect(draft.timeoutSecs).toBe('0.5');
    expect(settingsPatch(draft, base)).toEqual({});
  });
});

describe('数值输入校验', () => {
  const field: NumberField = {
    kind: 'number',
    key: 'timeoutSecs',
    label: '只读位置超时',
    hint: '',
    min: 1 / 60,
    max: 60,
    unit: '分钟',
    scale: 60,
    integer: false,
  };
  const integerField: NumberField = {
    kind: 'number',
    key: 'maxRequests',
    label: '单次派发上限',
    hint: '',
    min: 1,
    max: 100,
    unit: '路',
    scale: 1,
    integer: true,
  };

  it('空与非数字报错', () => {
    expect(numberInputError(field, '')).toBe('请填写');
    expect(numberInputError(field, '  ')).toBe('请填写');
    expect(numberInputError(field, 'abc')).toBe('要填数字');
  });

  it('越界报错并按上下限分别说明；分钟以下的小数合法', () => {
    expect(numberInputError(field, '0')).toBe('最短 1 秒');
    expect(numberInputError(field, '61')).toBe('最长 60 分钟');
    expect(numberInputError(field, '10.5')).toBe('');
    // 命令行设过分钟以下的秒数：展示与保存都照常。
    expect(numberInputError(field, '0.5')).toBe('');
  });

  it('整数键不接受小数', () => {
    expect(numberInputError(integerField, '7.5')).toBe('要填整数');
    expect(numberInputError(integerField, '0')).toBe('最少 1 路');
    expect(numberInputError(integerField, '7')).toBe('');
  });
});

describe('保存补丁', () => {
  it('只带改动的键；超时按分钟换算回秒', () => {
    const base = settings();
    const draft = initialDraft(base);
    draft.concurrency = 2;
    draft.timeoutSecs = '15';
    draft.validationCommand = '  npm test  ';
    expect(settingsPatch(draft, base)).toEqual({
      concurrency: 2,
      timeoutSecs: 900,
      validationCommand: 'npm test',
    });
  });

  it('没有改动时补丁为空', () => {
    const base = settings();
    expect(settingsPatch(initialDraft(base), base)).toEqual({});
    expect(changedCount(settingsPatch(initialDraft(base), base))).toBe(0);
  });

  it('任一数值不合法时补丁为 null（保存停用）', () => {
    const base = settings();
    const draft = initialDraft(base);
    draft.concurrency = 2;
    draft.timeoutSecs = '';
    expect(settingsPatch(draft, base)).toBeNull();
    expect(changedCount(null)).toBe(0);
  });

  it('改动被算成路数：changedCount 数键', () => {
    const base = settings();
    const draft = initialDraft(base);
    draft.fixRoundLimit = 0;
    draft.redact = false;
    expect(changedCount(settingsPatch(draft, base))).toBe(2);
  });
});

describe('ACP 适配器状态行', () => {
  it('拼接各家状态；没有报告为空串', () => {
    expect(acpStatusText([{ agent: 'claude', state: '已就绪' }, { agent: 'codex', state: '未安装' }]))
      .toBe('适配器：claude 已就绪，codex 未安装');
    expect(acpStatusText([])).toBe('');
    expect(acpStatusText(null)).toBe('');
  });
});
