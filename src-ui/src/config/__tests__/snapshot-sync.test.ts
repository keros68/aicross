import { describe, expect, it } from 'vitest';
import { createSnapshotSync } from '../snapshot-sync';

/** 可手动放行的假请求。 */
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

function setup() {
  const applied: string[] = [];
  const failed: string[] = [];
  const busy: boolean[] = [];
  const reads: ReturnType<typeof deferred<string>>[] = [];
  const sync = createSnapshotSync<string>({
    apply: (value) => applied.push(value),
    fail: (error) => failed.push(String(error)),
    busy: (value) => busy.push(value),
  });
  const fetcher = () => {
    const request = deferred<string>();
    reads.push(request);
    return request.promise;
  };
  return { sync, applied, failed, busy, reads, fetcher };
}

describe('快照读取与写入的交错（fix-fullcheck C3）', () => {
  it('写入开始前发出的旧读取晚于写入返回：旧结果不落地，写入结果落地，写完重读', async () => {
    const { sync, applied, reads, fetcher } = setup();
    const refresh = sync.read(fetcher);
    const write = deferred<string>();
    const saved = sync.write(() => write.promise, (value) => applied.push(`write:${value}`));

    reads[0].resolve('A');
    await flush();
    expect(applied).toEqual([]);

    write.resolve('B');
    await saved;
    expect(applied).toEqual(['write:B']);
    expect(reads).toHaveLength(2);

    reads[1].resolve('B');
    expect(await refresh).toBe(true);
    expect(applied).toEqual(['write:B', 'B']);
  });

  it('写入进行中的刷新推迟到写完，多次刷新只读一次', async () => {
    const { sync, applied, reads, fetcher } = setup();
    const write = deferred<string>();
    const saved = sync.write(() => write.promise, (value) => applied.push(`write:${value}`));
    const first = sync.read(fetcher);
    const second = sync.read(fetcher);
    expect(reads).toHaveLength(0);

    write.resolve('B');
    await saved;
    expect(reads).toHaveLength(1);
    reads[0].resolve('B');
    expect(await Promise.all([first, second])).toEqual([true, true]);
    expect(applied).toEqual(['write:B', 'B']);
  });

  it('写入排队执行：后发的写入等前一个返回才发出，结果按顺序落地，全部写完后重读一次', async () => {
    const { sync, applied, reads, fetcher } = setup();
    void sync.read(fetcher);
    reads[0].resolve('A');
    await flush();

    const one = deferred<string>();
    const two = deferred<string>();
    let secondSent = false;
    const first = sync.write(() => one.promise, (value) => applied.push(`write:${value}`));
    const second = sync.write(() => { secondSent = true; return two.promise; }, (value) => applied.push(`write:${value}`));
    await flush();
    expect(secondSent).toBe(false);

    one.resolve('B');
    await first;
    await flush();
    expect(secondSent).toBe(true);
    expect(applied).toEqual(['A', 'write:B']);
    expect(reads).toHaveLength(1);

    two.resolve('C');
    await second;
    expect(applied).toEqual(['A', 'write:B', 'write:C']);
    expect(reads).toHaveLength(2);
    reads[1].resolve('C');
    await flush();
    expect(applied.at(-1)).toBe('C');
  });

  it('后发写入不会先返回：旧的整份响应先落地，新人选随后落地，重读延迟期间一直忙（第二轮 P1）', async () => {
    const { sync, applied, busy, reads, fetcher } = setup();
    void sync.read(fetcher);
    reads[0].resolve('modes:A');
    await flush();

    // 设默认返回整份 modes（含旧人选 A），人选写入返回新人选 B。
    const setDefault = deferred<string>();
    const setSeat = deferred<string>();
    const sent: string[] = [];
    const first = sync.write(() => { sent.push('default'); return setDefault.promise; }, (value) => applied.push(`write:${value}`));
    const second = sync.write(() => { sent.push('seat'); return setSeat.promise; }, (value) => applied.push(`write:${value}`));
    await flush();
    expect(sent).toEqual(['default']);
    expect(busy).toEqual([true]);

    setDefault.resolve('modes:A+default');
    await first;
    await flush();
    expect(sent).toEqual(['default', 'seat']);
    setSeat.resolve('seat:B');
    await second;
    expect(applied).toEqual(['modes:A', 'write:modes:A+default', 'write:seat:B']);

    // 重读还没回来：仍然忙，编辑保持锁定。
    await flush();
    expect(busy).toEqual([true]);
    reads[1].resolve('modes:B+default');
    await flush();
    expect(applied.at(-1)).toBe('modes:B+default');
    expect(busy).toEqual([true, false]);
  });

  it('重读失败：交给 fail 并结束忙碌，之后可手动刷新', async () => {
    const { sync, failed, busy, reads, fetcher } = setup();
    void sync.read(fetcher);
    reads[0].resolve('A');
    await flush();

    const write = deferred<string>();
    const saved = sync.write(() => write.promise, () => undefined);
    write.resolve('B');
    await saved;
    reads[1].reject('读不到');
    await flush();
    expect(failed).toEqual(['读不到']);
    expect(busy).toEqual([true, false]);

    const retry = sync.read(fetcher);
    reads[2].resolve('B');
    expect(await retry).toBe(true);
  });

  it('自定义模式保存 / 删除期间发起的读取不落地旧快照（第二轮 P2）', async () => {
    const { sync, applied, reads, fetcher } = setup();
    const before = sync.read(fetcher);
    const save = deferred<string>();
    const saved = sync.write(() => save.promise, (value) => applied.push(`preset:${value}`));
    const during = sync.read(fetcher);
    expect(reads).toHaveLength(1);
    reads[0].resolve('old');
    await flush();
    expect(applied).toEqual([]);

    save.resolve('saved');
    await saved;
    reads[1].resolve('new');
    expect(await Promise.all([before, during])).toEqual([true, true]);
    expect(applied).toEqual(['preset:saved', 'new']);

    const remove = deferred<null>();
    const removed = sync.write(() => remove.promise, () => applied.push('removed'));
    const duringRemove = sync.read(fetcher);
    expect(reads).toHaveLength(2);
    remove.resolve(null);
    await removed;
    reads[2].resolve('after-remove');
    expect(await duringRemove).toBe(true);
    expect(applied.slice(-2)).toEqual(['removed', 'after-remove']);
  });

  it('还没读过快照时写完即结束忙碌', async () => {
    const { sync, busy } = setup();
    await sync.write(() => Promise.resolve('B'), () => undefined);
    expect(busy).toEqual([true, false]);
  });

  it('写入失败照常抛出，并且写完后重读', async () => {
    const { sync, applied, reads, fetcher } = setup();
    void sync.read(fetcher);
    reads[0].resolve('A');
    await flush();

    const write = deferred<string>();
    const saved = sync.write(() => write.promise, (value) => applied.push(`write:${value}`));
    write.reject(new Error('写入失败'));
    await expect(saved).rejects.toThrow('写入失败');
    expect(reads).toHaveLength(2);
  });

  it('没有写入时后发的读取为准，先发的读取等到后发的结果', async () => {
    const { sync, applied, failed, reads, fetcher } = setup();
    const first = sync.read(fetcher);
    const second = sync.read(fetcher);
    reads[1].resolve('new');
    reads[0].resolve('old');
    expect(await Promise.all([first, second])).toEqual([true, true]);
    expect(applied).toEqual(['new']);
    expect(failed).toEqual([]);
  });

  it('读取失败交给 fail，结果为 false', async () => {
    const { sync, applied, failed, reads, fetcher } = setup();
    const load = sync.read(fetcher);
    reads[0].reject('读不到');
    expect(await load).toBe(false);
    expect(applied).toEqual([]);
    expect(failed).toEqual(['读不到']);
  });
});
