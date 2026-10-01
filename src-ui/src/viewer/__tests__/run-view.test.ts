// run-view.ts 纯函数的边界用例。验收结论解析的原始断言在 TASK-40 随
// graph/dispatch.test.ts 删除，fix-40-r1 R1-4 恢复（函数现在住在 viewer/run-view.ts）。

import { describe, expect, it } from 'vitest';
import { parseAcceptance } from '../run-view';

describe('验收结论解析（parseAcceptance）', () => {
  it('取正文里最后一次出现的判定词', () => {
    expect(parseAcceptance('说明\nPASS')).toBe('PASS');
    expect(parseAcceptance('PASS 是旧值\nNEEDS_FIX')).toBe('NEEDS_FIX');
    expect(parseAcceptance('NEEDS_FIX\n修正后 PASS')).toBe('PASS');
  });

  it('只看尾部 1200 字符，更早的判定不算', () => {
    expect(parseAcceptance(`PASS${'x'.repeat(1200)}`)).toBe('UNKNOWN');
    // 判定词正好压在 1200 字符窗口的边缘：恰好收进来算，再长一位被截掉就不算。
    expect(parseAcceptance(`PASS\n${'x'.repeat(1195)}`)).toBe('PASS');
    expect(parseAcceptance(`PASS\n${'x'.repeat(1196)}`)).toBe('UNKNOWN');
  });

  it('没有判定词时未判定', () => {
    expect(parseAcceptance('all tests passed')).toBe('UNKNOWN');
    expect(parseAcceptance('需要人工判断')).toBe('UNKNOWN');
    expect(parseAcceptance('')).toBe('UNKNOWN');
  });
});
