import { describe, expect, it } from 'vitest';
import { runStatusLabel, runStatusTone } from '../run-view';
import {
  formatStartedAt,
  homeEmptyText,
  filterRuns,
  runRowCells,
  runsOverview,
  stageGridColumns,
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

describe('派发记录表格的文案', () => {
  it('表格的模式列与任务列', () => {
    expect(runRowCells(item())).toEqual({ mode: '经济模式', task: '给解析器加测试' });
  });

  it('任务名缺省时任务列为空；模式名缺省回「自定义模式」', () => {
    expect(runRowCells(item({ taskTitle: '' }))).toEqual({ mode: '经济模式', task: '' });
    expect(runRowCells(item({ taskTitle: '  ', presetName: '' }))).toEqual({ mode: '自定义模式', task: '' });
  });

  it('自定义模式只显示其名称，去掉「自定义模式 · 」前缀（fix-49-r3）', () => {
    expect(runRowCells(item({ presetName: '自定义模式 · 审稿' })).mode).toBe('审稿');
    expect(runRowCells(item({ presetName: '自定义模式 · ' })).mode).toBe('自定义模式');
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

describe('派发记录空状态', () => {
  it('没有 run 时写事实', () => {
    expect(homeEmptyText()).toBe('暂无派发记录');
  });
});

describe('派发记录概览与筛选（fix-49-r2）', () => {
  const runs = [
    item({ runId: 'a', status: 'done' }),
    item({ runId: 'b', status: 'failed' }),
    item({ runId: 'c', status: 'rejected' }),
    item({ runId: 'd', status: 'running' }),
    item({ runId: 'e', status: 'done' }),
  ];

  it('最近记录、已完成、失败（含已拒绝）、进行中；失败非 0 标异常', () => {
    expect(runsOverview(runs)).toEqual([
      { label: '最近记录', value: '5', alert: false },
      { label: '已完成', value: '2', alert: false },
      { label: '失败', value: '2', alert: true },
      { label: '进行中', value: '1', alert: false },
    ]);
    expect(runsOverview([item()])[2]).toEqual({ label: '失败', value: '0', alert: false });
  });

  it('全部 / 已完成 / 失败三档筛选，失败含已拒绝', () => {
    expect(filterRuns(runs, 'all')).toBe(runs);
    expect(filterRuns(runs, 'done').map((run) => run.runId)).toEqual(['a', 'e']);
    expect(filterRuns(runs, 'failed').map((run) => run.runId)).toEqual(['b', 'c']);
    expect(filterRuns([], 'failed')).toEqual([]);
  });
});

describe('查看页同阶段卡片栅格（fix-49-r4）', () => {
  it('最多并排 N 张，最小 240px，放不下时折行', () => {
    expect(stageGridColumns(1)).toBe('repeat(auto-fill, minmax(max(240px, calc((100% - 0px) / 1)), 1fr))');
    expect(stageGridColumns(3)).toBe('repeat(auto-fill, minmax(max(240px, calc((100% - 24px) / 3)), 1fr))');
  });

  it('0 或非整数按至少 1 列、向下取整', () => {
    expect(stageGridColumns(0)).toBe(stageGridColumns(1));
    expect(stageGridColumns(2.7)).toBe(stageGridColumns(2));
  });
});
