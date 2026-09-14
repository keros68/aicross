// TASK-07C（B-1 的另一半）：`dispatch-progress` 的订阅从 DispatchOverlay 提到
// GraphTab 层的共享 store，图上的节点卡与运行屏读同一份快照、用同一套判定。
// 判定逻辑本身在 07B 的用例里已经钉死，这里只测「同源」「节点卡渲染」「复位」。

// tsconfig.app.json 的 types 里只有 vite/client：应用代码不该碰 node 内置模块。
// 只有读源文件断言的那几条要读磁盘，就地压掉解析错误（同 07B 的写法）。
// @ts-expect-error node builtin, not in this project's `types`
import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
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
  progressRunOf,
  EMPTY_PROGRESS_RUN,
  PROGRESS_TICK_MS,
  type ProgressRun,
} from '../dispatch-progress.ts';
import type { DispatchProgress } from '../../tauri.ts';
import { NodeProgressRow } from '../../components/center/graph/DispatchBits.tsx';

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
