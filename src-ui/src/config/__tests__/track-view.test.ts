import { describe, expect, it } from 'vitest';
import type { TrackCounts, TrackModel } from '../../tauri';
import { orderTags, trackKey, trackLine } from '../track-view';

function counts(kind: string, kindName: string, extra: Partial<TrackCounts>): TrackCounts {
  return {
    kind, kindName, runs: 0, firstPass: 0, fixedPass: 0, notPassed: 0, failed: 0,
    noAcceptance: 0, unjudged: 0, adopted: 0, judged: 0, passRate: null, firstPassRate: null,
    ...extra,
  };
}

function model(byKind: TrackCounts[]): TrackModel {
  return { model: 'codex::m', tags: [], tagNames: [], note: '', total: counts('', '未分类', {}), byKind };
}

describe('使用记录一行（TASK-69）', () => {
  it('写法与引擎一致', () => {
    expect(trackKey('claude', '', 'opus')).toBe('claude::opus');
    expect(trackKey('pi', 'opencode-go', 'm')).toBe('pi:opencode-go:m');
  });

  it('样本够才写通过率，按次数排', () => {
    const line = trackLine(model([
      counts('writing', '文案', { runs: 2, noAcceptance: 2 }),
      counts('frontend', '前端', { runs: 7, judged: 6, firstPass: 5, passRate: 83, adopted: 1 }),
    ]));
    expect(line).toBe('记录：前端 7 次，一次通过 5，通过率 83%，采纳 1；文案 2 次');
  });

  it('没有记录为空，多于三类时说明', () => {
    expect(trackLine(undefined)).toBe('');
    expect(trackLine(model([]))).toBe('');
    const many = ['a', 'b', 'c', 'd'].map((id, i) => counts(id, id, { runs: 4 - i }));
    expect(trackLine(model(many))).toBe('记录：a 4 次；b 3 次；c 2 次；另有 1 类');
  });

  it('标签按类型表排序', () => {
    expect(orderTags(['writing', 'frontend'], [{ id: 'frontend' }, { id: 'backend' }, { id: 'writing' }]))
      .toEqual(['frontend', 'writing']);
  });
});
