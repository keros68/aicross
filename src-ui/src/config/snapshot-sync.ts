// 模式页快照的读写协调（fix-fullcheck C3 与第二轮）：配置写入（保存 / 清除人选、改思考、
// 改备选、一键推荐、设默认、改视角、改数量、保存 / 删除自定义模式）排队一个接一个执行，
// 后发的写入等前一个返回才发出，结果按发出顺序落地，迟回的整份响应不会盖掉后写的人选；
// 写入期间读取推迟，写入开始时作废还在路上的读取（它可能读到写入前的旧配置）；全部写完后
// 统一重读一次，以引擎的配置为准。从第一个写入开始到这次重读结束（成功或失败）都算忙碌，
// 页面据此锁定编辑。

export interface SnapshotSyncHooks<T> {
  /** 读取结果落地。 */
  apply: (value: T) => void;
  /** 最新一次读取失败。 */
  fail: (error: unknown) => void;
  /** 忙碌状态变化：第一个写入开始时为 true，全部写完且重读结束后为 false。 */
  busy?: (busy: boolean) => void;
}

export interface SnapshotSync<T> {
  /** 读一次快照；写入进行中时推迟到写完。返回最终落地的那次读取是否成功。 */
  read: (fetcher: () => Promise<T>) => Promise<boolean>;
  /** 排队执行一次写入，成功后用 apply 并入结果；失败照常抛出。全部写完（含失败）后重读。 */
  write: <R>(action: () => Promise<R>, apply: (result: R) => void) => Promise<R>;
}

export function createSnapshotSync<T>(hooks: SnapshotSyncHooks<T>): SnapshotSync<T> {
  // 已排队或执行中的写入数。
  let writes = 0;
  // 写入队列的尾部；新写入接在它后面。
  let queue: Promise<unknown> = Promise.resolve();
  // 每次发起读取或写入都加一；读取返回时序号已变就不落地。
  let seq = 0;
  let fetcher: (() => Promise<T>) | null = null;
  // 等下一次落地读取的调用方（被作废或推迟的读取也在这里等）。
  let waiters: ((ok: boolean) => void)[] = [];
  let busy = false;

  const setBusy = (next: boolean) => {
    if (busy === next) return;
    busy = next;
    hooks.busy?.(next);
  };

  const settle = (ok: boolean) => {
    const pending = waiters;
    waiters = [];
    for (const resolve of pending) resolve(ok);
    setBusy(false);
  };

  const start = () => {
    if (fetcher === null) {
      setBusy(false);
      return;
    }
    const current = ++seq;
    fetcher().then(
      (value) => {
        if (current !== seq || writes > 0) return;
        hooks.apply(value);
        settle(true);
      },
      (error: unknown) => {
        if (current !== seq || writes > 0) return;
        hooks.fail(error);
        settle(false);
      },
    );
  };

  return {
    read(next) {
      fetcher = next;
      const done = new Promise<boolean>((resolve) => { waiters.push(resolve); });
      if (writes === 0) start();
      return done;
    },
    write(action, apply) {
      writes += 1;
      seq += 1;
      setBusy(true);
      const run = queue.then(async () => {
        const result = await action();
        apply(result);
        return result;
      });
      const done = run.finally(() => {
        writes -= 1;
        if (writes === 0) start();
      });
      queue = done.catch(() => undefined);
      return done;
    },
  };
}
