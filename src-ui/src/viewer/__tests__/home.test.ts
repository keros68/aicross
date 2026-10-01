import { describe, expect, it } from 'vitest';
import {
  formatStartedAt,
  homeEmptyText,
  runListItemText,
  runStatusLabel,
  runStatusTone,
} from '../run-list';
import type { RunListItem } from '../../tauri';

function item(overrides: Partial<RunListItem> = {}): RunListItem {
  return {
    runId: 'run-1',
    presetName: '经济模式',
    status: 'done',
    startedAt: '2026-10-01T08:30:00Z',
    taskTitle: '给解析器加测试',
    ...overrides,
  };
}

describe('首页列表项文案', () => {
  it('主文案是「模式名 · 任务名」', () => {
    expect(runListItemText(item())).toBe('经济模式 · 给解析器加测试');
  });

  it('任务名缺省时只留模式名；模式名也缺省回「自定义模式」', () => {
    expect(runListItemText(item({ taskTitle: '' }))).toBe('经济模式');
    expect(runListItemText(item({ taskTitle: '  ', presetName: '' }))).toBe('自定义模式');
  });

  it('状态词与状态展示成对：异常与运行中给胶囊，已完成用纯文字', () => {
    expect(runStatusLabel('running')).toBe('运行中');
    expect(runStatusLabel('done')).toBe('已完成');
    expect(runStatusLabel('failed')).toBe('失败');
    expect(runStatusLabel('rejected')).toBe('已拒绝');
    expect(runStatusTone('running')).toBe('pill');
    expect(runStatusTone('done')).toBe('plain');
    expect(runStatusTone('failed')).toBe('pill-alert');
    expect(runStatusTone('rejected')).toBe('pill-alert');
    // 未知状态原样展示，不给胶囊。
    expect(runStatusLabel('weird')).toBe('weird');
    expect(runStatusTone('weird')).toBe('plain');
  });

  it('开始时间按本地时区展示到分钟', () => {
    // 固定时区避免平台差异：用 UTC 时间戳构造，断言只看格式。
    const text = formatStartedAt('2026-10-01T00:00:00Z');
    expect(text).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/);
    expect(formatStartedAt('不是时间')).toBe('不是时间');
  });
});

describe('首页空状态', () => {
  it('没有 run 时说明派发从哪里来', () => {
    expect(homeEmptyText()).toBe(
      '还没有派发记录；在宿主里经 ai-cross skill 派发，或运行 aicross dispatch',
    );
  });
});
