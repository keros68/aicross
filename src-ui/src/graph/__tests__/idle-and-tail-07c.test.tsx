// TASK-07C（B-1 的另一半）：`dispatch-progress` 的订阅从 DispatchOverlay 提到
// GraphTab 层的共享 store，图上的节点卡与运行屏读同一份快照、用同一套判定。
// 判定逻辑本身在 07B 的用例里已经钉死，这里只测「同源」「节点卡渲染」「复位」。

// tsconfig.app.json 的 types 里只有 vite/client：应用代码不该碰 node 内置模块。
// 只有读源文件断言的那几条要读磁盘，就地压掉解析错误（同 07B 的写法）。
// @ts-expect-error node builtin, not in this project's `types`
import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import {
  idleHint,
  progressRunningAny,
  progressStatusLabel,
  runningDetailOf,
  REPEATED_TOOL_IDLE_MS,
} from '../dispatch.ts';
import {
  applyProgressEvent,
  beginProgressRun,
  endProgressRun,
  progressRunOf,
  subscribeProgress,
  EMPTY_PROGRESS_RUN,
  PROGRESS_ENDED_STATUS,
  PROGRESS_TICK_MS,
  type ProgressRun,
} from '../dispatch-progress.ts';
import type { DispatchProgress } from '../../tauri.ts';
import { NodeProgressRow } from '../../components/center/graph/DispatchBits.tsx';

// R1-2：订阅生命周期要在 node 环境里断言，`listen` 换成可控的桩——每次调用记下
// 事件名与回调，并把 resolve 攥在手里，好造「listen 迟到解析」这一格。
const eventMock = vi.hoisted(() => {
  const state = {
    names: [] as string[],
    handlers: [] as Array<(event: { payload: unknown }) => void>,
    pending: [] as Array<(stop: () => void) => void>,
    failNext: false,
    reset() {
      state.names = [];
      state.handlers = [];
      state.pending = [];
      state.failNext = false;
    },
    listen(name: string, handler: (event: { payload: unknown }) => void) {
      state.names.push(name);
      state.handlers.push(handler);
      if (state.failNext) return Promise.reject(new Error('no window'));
      return new Promise<() => void>((resolve) => { state.pending.push(resolve); });
    },
  };
  return state;
});
vi.mock('@tauri-apps/api/event', () => ({ listen: eventMock.listen }));

// 动态 import + 两级 then，等够微任务队列排空；不用假时钟这一格。
function flushMicrotasks(): Promise<void> {
  return new Promise((resolve) => { setTimeout(resolve, 0); });
}

function readSource(relative: string): string {
  return readFileSync(new URL(relative, import.meta.url), 'utf8');
}

function heartbeat(patch: Partial<DispatchProgress> = {}): DispatchProgress {
  return {
    runId: 'run-1',
    nodeId: 'n1',
    status: 'running',
    elapsedMs: 42_000,
    idleMs: 1_000,
    toolCalls: 3,
    repeatedToolCalls: 1,
    lastTool: 'read_file',
    ...patch,
  };
}

describe('07C-1 进度状态提到共享层', () => {
  it('applyProgressEvent / progressRunOf：运行屏与节点卡读到同一份快照', () => {
    const run = applyProgressEvent(beginProgressRun('run-1'), heartbeat(), 1_000_000);
    // 两个消费者都走 progressRunOf（组件里是 store.snapshotOf），拿到同一个对象
    const forOverlay = progressRunOf(run, 'n1');
    const forNodeCard = progressRunOf(run, 'n1');
    expect(forOverlay).toBeDefined();
    expect(forOverlay).toBe(forNodeCard);
    expect(forOverlay).toBe(run.progress.n1);
    // 同一份快照 → 同一套文案与判定，两边不会各说各的
    expect(runningDetailOf(forOverlay, 1_000_000)).toBe('已运行 42 秒 · 工具 read_file');
    expect(renderToStaticMarkup(<NodeProgressRow progress={forNodeCard} now={1_000_000} />))
      .toContain('已运行 42 秒 · 工具 read_file');
    expect(progressStatusLabel(forOverlay!.status)).toBe('运行中');
    expect(idleHint(forOverlay, 1_000_000)).toBeNull();
  });

  it('订阅只剩共享层一处：DispatchOverlay 不再自己 listen，两个消费者都走 snapshotOf', () => {
    const shared = readSource('../dispatch-progress.ts');
    const overlay = readSource('../../components/center/graph/DispatchOverlay.tsx');
    const tab = readSource('../../components/center/graph/GraphTab.tsx');
    // 事件名与 5s 时钟只在共享层出现一次（两边的 import 路径不算，断的是事件名字面量）
    expect(shared.match(/'dispatch-progress'/g)?.length).toBe(1);
    expect(shared).toContain('PROGRESS_TICK_MS');
    expect(overlay).not.toContain("'dispatch-progress'");
    expect(tab).not.toContain("'dispatch-progress'");
    expect(overlay).not.toContain('@tauri-apps/api/event');
    expect(tab).not.toContain('@tauri-apps/api/event');
    // GraphTab 建 store、传给运行屏、也喂给节点卡
    expect(tab).toMatch(/useDispatchProgress\(graphId\)/);
    expect(tab).toMatch(/progressStore=\{progressStore\}/);
    expect(tab).toMatch(/<NodeProgressRow\s+progress=\{progressStore\.snapshotOf\(id\)\}\s+now=\{progressStore\.now\}\s*\/>/);
    // 运行屏只消费，不再自己存进度
    expect(overlay).toMatch(/progressStore\.snapshotOf\(\s*node\.nodeId\s*\)/);
    expect(overlay).not.toContain('setProgress(');
    expect(overlay).not.toContain('setNow(');
  });

  it('progressRunningAny：还有节点在跑才走时钟，全到终态就停', () => {
    expect(progressRunningAny({})).toBe(false);
    const running = applyProgressEvent(beginProgressRun('run-1'), heartbeat(), 1_000_000);
    expect(progressRunningAny(running.progress)).toBe(true);
    const done = applyProgressEvent(running, heartbeat({ status: '完成' }), 1_010_000);
    expect(progressRunningAny(done.progress)).toBe(false);
    // 两个节点，一个跑完一个还在跑 → 时钟继续
    const mixed = applyProgressEvent(done, heartbeat({ nodeId: 'n2' }), 1_010_001);
    expect(progressRunningAny(mixed.progress)).toBe(true);
    expect(PROGRESS_TICK_MS).toBe(5000);
  });
});

describe('07C-2 节点卡运行中显示耗时与工具', () => {
  it('NodeProgressRow：运行中渲染「已运行 … · 工具 …」，完整文本进 title', () => {
    const run = applyProgressEvent(
      beginProgressRun('run-1'),
      heartbeat({ elapsedMs: 80_000, lastTool: 'edit_file' }),
      1_000_000,
    );
    const html = renderToStaticMarkup(
      <NodeProgressRow progress={progressRunOf(run, 'n1')} now={1_000_000 + 10_000} />,
    );
    // 心跳读数 + 本机墙上时间：80s + 10s
    expect(html).toContain('已运行 1 分 30 秒 · 工具 edit_file');
    expect(html).toContain('class="graph-node-row graph-node-progress"');
    expect(html).toContain('title="已运行 1 分 30 秒 · 工具 edit_file"');
    expect(html).toContain('class="graph-node-elapsed"');
    // 常规读数不挂胶囊、不用红字
    expect(html).not.toContain('hint-capsule');
    expect(html).not.toContain('dispatch-warning');
  });

  it('NodeProgressRow：同一套疑似空转胶囊，没快照/没读数时不占行', () => {
    const idle = applyProgressEvent(
      beginProgressRun('run-1'),
      heartbeat({ repeatedToolCalls: 3, idleMs: REPEATED_TOOL_IDLE_MS }),
      1_000_000,
    );
    const html = renderToStaticMarkup(
      <NodeProgressRow progress={progressRunOf(idle, 'n1')} now={1_000_000} />,
    );
    // 与运行屏的 IdleHint 同一个组件、同一句文案
    expect(html).toContain('疑似空转：同一工具连续 3 次');
    expect(html).toContain('hint-capsule');
    expect(html).toContain('已运行 42 秒');
    expect(html).not.toContain('dispatch-warning');
    // 没派发过 / 心跳还没到：整行不渲染
    expect(renderToStaticMarkup(<NodeProgressRow progress={undefined} now={1_000_000} />)).toBe('');
    const queued = applyProgressEvent(
      beginProgressRun('run-1'),
      { runId: 'run-1', nodeId: 'n1', status: '排队' },
      1_000_000,
    );
    expect(renderToStaticMarkup(
      <NodeProgressRow progress={progressRunOf(queued, 'n1')} now={1_000_000} />,
    )).toBe('');
  });

  it('NodeProgressRow：终态冻结，节点卡不跟着墙上时间涨', () => {
    const run = applyProgressEvent(
      applyProgressEvent(beginProgressRun('run-1'), heartbeat(), 1_000_000),
      heartbeat({ status: '完成' }),
      1_000_100,
    );
    const later = renderToStaticMarkup(
      <NodeProgressRow progress={progressRunOf(run, 'n1')} now={1_000_100 + 5 * 60_000} />,
    );
    expect(later).toContain('已运行 42 秒 · 工具 read_file');
    expect(later).not.toContain('hint-capsule');
  });
});

describe('07C-3 复位：换图与重派', () => {
  it('beginProgressRun：重派后节点卡与运行屏都不显示上一轮读数', () => {
    const stale = applyProgressEvent(
      beginProgressRun('run-1'),
      heartbeat({ elapsedMs: 900_000, idleMs: 600_000 }),
      1_000_000,
    );
    expect(idleHint(progressRunOf(stale, 'n1'), 1_000_000)).toBe('疑似空转：10 分钟无输出');

    // 新一轮：同一个 nodeId、新的 runId
    const fresh = beginProgressRun('run-2');
    expect(fresh.progress).toEqual({});
    expect(progressRunOf(fresh, 'n1')).toBeUndefined();
    expect(renderToStaticMarkup(
      <NodeProgressRow progress={progressRunOf(fresh, 'n1')} now={1_000_000 + 14_000} />,
    )).toBe('');

    // 上一轮迟到的事件不写入，state 原样返回（同一引用，不触发重渲染）
    const late = applyProgressEvent(fresh, heartbeat({ idleMs: 999_000 }), 1_000_001);
    expect(late).toBe(fresh);

    // 本轮心跳照常写入，读数从新一轮算起
    const next = applyProgressEvent(fresh, heartbeat({ runId: 'run-2', elapsedMs: 15_000, idleMs: 0 }), 1_000_002);
    expect(next.runId).toBe('run-2');
    expect(runningDetailOf(progressRunOf(next, 'n1'), 1_000_002)).toBe('已运行 15 秒 · 工具 read_file');
    expect(idleHint(progressRunOf(next, 'n1'), 1_000_002)).toBeNull();
  });

  it('换图：复位成 EMPTY_PROGRESS_RUN，之后连本轮事件也不再落进来', () => {
    const run: ProgressRun = applyProgressEvent(beginProgressRun('run-1'), heartbeat(), 1_000_000);
    expect(progressRunOf(run, 'n1')).toBeDefined();
    // hook 里换图走的就是这一步（useEffect 依赖 graphId）
    expect(EMPTY_PROGRESS_RUN).toEqual({ runId: '', progress: {} });
    expect(progressRunOf(EMPTY_PROGRESS_RUN, 'n1')).toBeUndefined();
    expect(applyProgressEvent(EMPTY_PROGRESS_RUN, heartbeat(), 1_000_001)).toBe(EMPTY_PROGRESS_RUN);
    expect(renderToStaticMarkup(
      <NodeProgressRow progress={progressRunOf(EMPTY_PROGRESS_RUN, 'n1')} now={1_000_000} />,
    )).toBe('');
  });

  it('共享层接线：换图按 graphId 复位，start/retry 各换一次轮次', () => {
    const shared = readSource('../dispatch-progress.ts');
    const overlay = readSource('../../components/center/graph/DispatchOverlay.tsx');
    // 换图复位挂在 graphId 上
    expect(shared).toMatch(/setRun\(EMPTY_PROGRESS_RUN\);\s*\}, \[graphId\]\)/);
    // 没派发过不挂监听；重派换 runId 时重挂
    expect(shared).toMatch(/if \(run\.runId === ''\) return;/);
    expect(shared).toMatch(/\}, \[run\.runId\]\)/);
    // start() 与 retry() 各调一次 beginRun；retry 用的是新生成的 runId
    expect(overlay.match(/progressStore\.beginRun\(/g)?.length).toBe(2);
    expect(overlay).toMatch(/const nextRunId = crypto\.randomUUID\(\);/);
    expect(overlay).toMatch(/progressStore\.beginRun\(nextRunId\)/);
  });
});

describe('07C-4 不复制判定逻辑', () => {
  it('节点卡与运行屏共用 runningDetailOf / idleHint / IdleHint，没有第二份实现', () => {
    const bits = readSource('../../components/center/graph/DispatchBits.tsx');
    const tab = readSource('../../components/center/graph/GraphTab.tsx');
    const shared = readSource('../dispatch-progress.ts');
    // 节点卡那一行只是排版，读数与提示都来自 graph/dispatch.ts 的纯函数
    expect(bits).toMatch(/runningDetailOf\(progress, now\)/);
    expect(bits).toMatch(/idleHint\(progress, now\) === null/);
    expect(bits).toMatch(/<IdleHint progress=\{progress\} now=\{now\} \/>/);
    // 阈值与文案没有在别处重写（断的是实现里的字面量，不是注释里提到的词）
    for (const literal of ['疑似空转：', '已运行 ${', 'IDLE_HINT_MS', 'REPEATED_TOOL_LIMIT']) {
      expect(tab).not.toContain(literal);
      expect(shared).not.toContain(literal);
    }
    // 共享层只做 state 形状，判定仍然从 dispatch.ts 引进来
    expect(shared).toMatch(/from '\.\/dispatch'/);
  });
});

// ── 修正轮 R1（tasks/fix-07c-r1.md，来自 tasks/review-07c-ds.md 的盲审）────────

describe('07C-R1-1 终态事件保留上一条心跳读数', () => {
  it('无字段终态事件后耗时与工具仍显示且不再增长', () => {
    const running = applyProgressEvent(beginProgressRun('run-1'), heartbeat(), 1_000_000);
    // 引擎发的终态事件只有 runId/nodeId/status，不带任何心跳字段（TASK-07 契约）
    const done = applyProgressEvent(
      running,
      { runId: 'run-1', nodeId: 'n1', status: '完成' },
      1_000_100,
    );
    const snapshot = progressRunOf(done, 'n1');
    expect(snapshot?.status).toBe('完成');
    expect(snapshot?.elapsedMs).toBe(42_000);
    expect(snapshot?.lastTool).toBe('read_file');
    expect(snapshot?.toolCalls).toBe(3);
    expect(snapshot?.hasHeartbeat).toBe(true);
    // 冻结：过 5 分钟读数仍是最后一条心跳的值，也不挂空转胶囊
    expect(runningDetailOf(snapshot, 1_000_100)).toBe('已运行 42 秒 · 工具 read_file');
    expect(runningDetailOf(snapshot, 1_000_100 + 5 * 60_000)).toBe('已运行 42 秒 · 工具 read_file');
    expect(idleHint(snapshot, 1_000_100 + 5 * 60_000)).toBeNull();
    const html = renderToStaticMarkup(
      <NodeProgressRow progress={snapshot} now={1_000_100 + 5 * 60_000} />,
    );
    expect(html).toContain('已运行 42 秒 · 工具 read_file');
    expect(html).not.toContain('hint-capsule');
    // 时钟也随之停（终态不算「在跑」）
    expect(progressRunningAny(done.progress)).toBe(false);
    // 上一轮的旧快照不会被借用：runId 对不上就按新事件原样补 0
    const crossRun = applyProgressEvent(
      { runId: 'run-2', progress: { n1: snapshot! } },
      { runId: 'run-2', nodeId: 'n1', status: '完成' },
      1_000_200,
    );
    expect(progressRunOf(crossRun, 'n1')?.elapsedMs).toBe(0);
    expect(progressRunOf(crossRun, 'n1')?.hasHeartbeat).toBe(false);
    // 没有上一条心跳时同样不编数：整行不渲染
    const bare = applyProgressEvent(
      beginProgressRun('run-3'),
      { runId: 'run-3', nodeId: 'n9', status: '完成' },
      1_000_300,
    );
    expect(renderToStaticMarkup(
      <NodeProgressRow progress={progressRunOf(bare, 'n9')} now={1_000_300} />,
    )).toBe('');
  });
});

describe('07C-R1-2 listen 取消守卫', () => {
  it('subscribeProgress：换 runId 与卸载各 unlisten 一次，迟到的 listen 立即解除', async () => {
    eventMock.reset();
    const unlisten = vi.fn();
    const seen: DispatchProgress[] = [];

    // 没派发过（runId 空）不订阅
    subscribeProgress('', () => {}).dispose();
    await flushMicrotasks();
    expect(eventMock.pending).toHaveLength(0);

    // 第一轮：解析后换 runId（effect 重挂）→ unlisten 一次
    const first = subscribeProgress('run-1', (payload) => seen.push(payload));
    await flushMicrotasks();
    expect(eventMock.names).toEqual(['dispatch-progress']);
    eventMock.pending[0](unlisten);
    await flushMicrotasks();
    eventMock.handlers[0]({ payload: heartbeat() });
    expect(seen).toHaveLength(1);
    first.dispose();
    expect(unlisten).toHaveBeenCalledTimes(1);

    // 第二轮：解析后卸载 → 再 unlisten 一次（换 runId 与卸载各一次）
    const second = subscribeProgress('run-2', (payload) => seen.push(payload));
    await flushMicrotasks();
    eventMock.pending[1](unlisten);
    await flushMicrotasks();
    second.dispose();
    expect(unlisten).toHaveBeenCalledTimes(2);
    // 拆过之后不再重复解除
    second.dispose();
    expect(unlisten).toHaveBeenCalledTimes(2);

    // 迟到 resolve：dispose 发生在 listen 解析之前，解析出来的监听就地解除
    const late = subscribeProgress('run-3', () => {});
    await flushMicrotasks();
    late.dispose();
    expect(unlisten).toHaveBeenCalledTimes(2);
    eventMock.pending[2](unlisten);
    await flushMicrotasks();
    expect(unlisten).toHaveBeenCalledTimes(3);
  });

  it('subscribeProgress：订阅失败留痕而不是静默吞掉', async () => {
    eventMock.reset();
    eventMock.failNext = true;
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const sub = subscribeProgress('run-1', () => {});
    await flushMicrotasks();
    expect(warn).toHaveBeenCalledTimes(1);
    sub.dispose();
    warn.mockRestore();
  });
});

describe('07C-R1-3 异常收尾后时钟停', () => {
  it('endProgressRun：收尾后 now 不再前进、不挂胶囊', () => {
    vi.useFakeTimers();
    try {
      const started = 1_000_000;
      vi.setSystemTime(started);
      let run = applyProgressEvent(beginProgressRun('run-1'), heartbeat(), started);
      run = applyProgressEvent(run, heartbeat({ nodeId: 'n2' }), started);

      // 共享层的 5s 时钟：门控就是 progressRunningAny（hook 的 effect 依赖它）
      let now = started;
      let timer: ReturnType<typeof setInterval> | undefined;
      const gate = () => {
        if (timer !== undefined) {
          clearInterval(timer);
          timer = undefined;
        }
        if (!progressRunningAny(run.progress)) return;
        now = Date.now();
        timer = setInterval(() => { now = Date.now(); }, PROGRESS_TICK_MS);
      };
      gate();
      expect(timer).toBeDefined();
      vi.advanceTimersByTime(4 * PROGRESS_TICK_MS);
      expect(now).toBe(started + 4 * PROGRESS_TICK_MS);

      // 派发抛错收尾：没收到终态事件的节点在这里补成终态
      run = endProgressRun(run);
      expect(progressRunOf(run, 'n1')?.status).toBe(PROGRESS_ENDED_STATUS);
      expect(progressRunOf(run, 'n2')?.status).toBe(PROGRESS_ENDED_STATUS);
      expect(progressRunningAny(run.progress)).toBe(false);
      gate();
      expect(timer).toBeUndefined();

      const frozen = now;
      vi.advanceTimersByTime(10 * 60_000);
      expect(now).toBe(frozen);
      // 读数冻结在最后一条心跳上，3 分钟之后也不挂空转胶囊
      const html = renderToStaticMarkup(
        <NodeProgressRow progress={progressRunOf(run, 'n1')} now={frozen + 10 * 60_000} />,
      );
      expect(html).toContain('已运行 42 秒 · 工具 read_file');
      expect(html).not.toContain('hint-capsule');

      // 全终态时是空操作，返回同一引用
      expect(endProgressRun(run)).toBe(run);
    } finally {
      vi.useRealTimers();
    }
  });

  it('接线：start() 的 finally 调 endRun，死接口 resetRun 已删', () => {
    const shared = readSource('../dispatch-progress.ts');
    const overlay = readSource('../../components/center/graph/DispatchOverlay.tsx');
    expect(shared).not.toContain('resetRun');
    expect(shared).toMatch(/endRun: \(\) => void;/);
    expect(overlay).toMatch(/\} finally \{[\s\S]*?progressStore\.endRun\(\);[\s\S]*?\}/);
  });
});

describe('07C-R1-4 节点卡胶囊不裁切', () => {
  it('最长空转文案在卡宽内换行显示完整，不靠 overflow 截断', () => {
    const css = readSource('../../components/center/graph/GraphTab.css');
    const tab = readSource('../../components/center/graph/GraphTab.tsx');
    const progressRule = css.slice(
      css.indexOf('.graph-node-progress {'),
      css.indexOf('.graph-node-actions'),
    );

    // 第三行可换行、行高显式
    expect(progressRule).toContain('flex-wrap: wrap;');
    expect(progressRule).toContain('line-height: 1.4;');
    // 胶囊自己也换行，不再是 flex-shrink: 0 + nowrap 的溢出写法
    const capsuleRule = progressRule.slice(progressRule.indexOf('.graph-node-progress .hint-capsule'));
    expect(capsuleRule).toContain('white-space: normal;');
    expect(capsuleRule).toContain('max-width: 100%;');
    expect(capsuleRule).not.toContain('flex-shrink: 0;');
    // 读数单独占一行，过长时才省略；完整文本仍在 title 里
    expect(progressRule).toMatch(/\.graph-node-elapsed \{[^}]*flex: 1 1 100%;/);
    expect(progressRule).toMatch(/\.graph-node-elapsed \{[^}]*text-overflow: ellipsis;/);
    // 卡高按三行重算，容得下两行胶囊
    const nodeH = Number(tab.match(/const NODE_H = (\d+);/)?.[1]);
    expect(nodeH).toBeGreaterThanOrEqual(112);

    // 最长文案原样渲染在胶囊里，没有省略号、没有截断
    const idle = applyProgressEvent(
      beginProgressRun('run-1'),
      heartbeat({ repeatedToolCalls: 3, idleMs: REPEATED_TOOL_IDLE_MS }),
      1_000_000,
    );
    const hint = idleHint(progressRunOf(idle, 'n1'), 1_000_000);
    expect(hint).toBe('疑似空转：同一工具连续 3 次');
    const html = renderToStaticMarkup(
      <NodeProgressRow progress={progressRunOf(idle, 'n1')} now={1_000_000} />,
    );
    expect(html).toContain('<span class="hint-capsule">' + hint + '</span>');
    expect(html).not.toContain('…');
  });
});
