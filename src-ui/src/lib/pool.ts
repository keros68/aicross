// 限并发地逐条处理（TASK-58「全部测试」：并发 3，可取消）。取消只是不再发新请求，
// 已发出的照常等它结束；单条失败不影响其余条目。

export interface PoolResult {
  /** 已结束（成功或失败）的条数。 */
  done: number;
  failed: number;
  /** 因取消而没发出的条数大于 0 时为 true。 */
  stopped: boolean;
}

export async function runPool<T>(
  items: readonly T[],
  limit: number,
  worker: (item: T) => Promise<unknown>,
  options: { shouldStop?: () => boolean; onProgress?: (done: number) => void } = {},
): Promise<PoolResult> {
  let next = 0;
  let done = 0;
  let failed = 0;
  let stopped = false;
  const lane = async () => {
    while (next < items.length) {
      if (options.shouldStop?.()) {
        stopped = true;
        return;
      }
      const item = items[next];
      next += 1;
      try {
        await worker(item);
      } catch {
        failed += 1;
      }
      done += 1;
      options.onProgress?.(done);
    }
  };
  const lanes = Array.from({ length: Math.max(1, Math.min(limit, items.length)) }, lane);
  await Promise.all(lanes);
  return { done, failed, stopped };
}
