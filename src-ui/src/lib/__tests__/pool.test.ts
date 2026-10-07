import { describe, expect, it } from 'vitest';
import { runPool } from '../pool';

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((r) => { resolve = r; });
  return { promise, resolve };
}

describe('限并发逐条处理（TASK-58「全部测试」）', () => {
  it('同时在跑的不超过上限，全部跑完并报进度', async () => {
    let running = 0;
    let peak = 0;
    const progress: number[] = [];
    const result = await runPool([1, 2, 3, 4, 5, 6, 7], 3, async () => {
      running += 1;
      peak = Math.max(peak, running);
      await new Promise((r) => setTimeout(r, 5));
      running -= 1;
    }, { onProgress: (done) => progress.push(done) });
    expect(peak).toBe(3);
    expect(result).toEqual({ done: 7, failed: 0, stopped: false });
    expect(progress).toEqual([1, 2, 3, 4, 5, 6, 7]);
  });

  it('单条失败不影响其余，计入 failed', async () => {
    const result = await runPool([1, 2, 3], 2, async (n) => {
      if (n === 2) throw new Error('boom');
    });
    expect(result).toEqual({ done: 3, failed: 1, stopped: false });
  });

  it('取消后不再发新请求，已发出的照常结束', async () => {
    const gates = [deferred(), deferred(), deferred(), deferred(), deferred()];
    const started: number[] = [];
    let stop = false;
    const run = runPool([0, 1, 2, 3, 4], 2, async (n) => {
      started.push(n);
      await gates[n].promise;
    }, { shouldStop: () => stop });
    await Promise.resolve();
    expect(started).toEqual([0, 1]);
    stop = true;
    gates[0].resolve();
    gates[1].resolve();
    const result = await run;
    expect(started).toEqual([0, 1]);
    expect(result).toEqual({ done: 2, failed: 0, stopped: true });
  });

  it('没有条目时直接结束', async () => {
    expect(await runPool([], 3, async () => {})).toEqual({ done: 0, failed: 0, stopped: false });
  });
});
