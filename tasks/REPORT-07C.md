# REPORT-07C 进度状态提到共享层，节点卡也显示运行中读数

TASK-07 B-1 的另一半（`tasks/TASK-07-idle-and-tail.md` B-1 的括注、`tasks/REPORT-07B.md`
「修正轮 R1」R1-6 写回的那条）：把 `dispatch-progress` 的订阅从 `DispatchOverlay` 提到
`GraphTab` 层，图上的节点卡在运行中也显示「已运行 … · 工具 …」和同一套「疑似空转」胶囊。
只改 `src-ui/` 与 `tasks/`，没动 `src/`。行号是改完之后的。

## 已做

### 1. 共享层：`dispatch-progress` 的订阅、5s 时钟、复位时机

**已做**（新文件 `src-ui/src/graph/dispatch-progress.ts:1-106`）：

- `ProgressRun`（`:22-25`）= `{ runId, progress }`，一轮派发的全部快照；
  `EMPTY_PROGRESS_RUN`（`:28`）、`PROGRESS_TICK_MS = 5000`（`:31`）。
- 三个纯函数，测试不必渲染组件：
  - `beginProgressRun(runId)`（`:34-36`）——新一轮清快照、换 runId；
  - `applyProgressEvent(run, payload, now)`（`:39-47`）——转调既有的 `nextProgress`，
    别轮次的事件返回**同一引用**（不触发重渲染）；
  - `progressRunOf(run, nodeId)`（`:49-51`）——转调既有的 `progressOf`，runId 对不上当没有。
    运行屏与节点卡都只走这一个入口。
- `useDispatchProgress(graphId)`（`:68-105`）返回 `DispatchProgressStore`
  （`:54-67`：`runId` / `progress` / `now` / `snapshotOf` / `beginRun` / `resetRun`）。
  三个 effect：换图按 `graphId` 复位（`:73-75`）、`runId` 非空才挂事件监听且 runId 变时重挂
  （`:78-86`）、`progressRunningAny` 为真时才走 5s 时钟（`:90-96`）。
- 判定逻辑**没有第二份**：耗时/工具文案仍是 `graph/dispatch.ts` 的 `runningDetailOf`，
  空转判定仍是 `idleHint`，按 runId 取快照仍是 `progressOf`；本文件只做 state 形状与 React 接线。

**已做**（`src-ui/src/graph/dispatch.ts:1109-1113`）：新增纯函数 `progressRunningAny(progress)`，
复用既有的私有 `progressRunning`。共享层的时钟只在「还有节点在跑」时走——全部到终态之后读数
已经按 R1-4 冻结，再 tick 只是白重渲染整张图。

**已补测试**（`src-ui/src/graph/__tests__/idle-and-tail-07c.test.tsx`）：
- `07C-1 进度状态提到共享层 > progressRunningAny：还有节点在跑才走时钟，全到终态就停`
- `07C-3 复位：换图与重派 > 共享层接线：换图按 graphId 复位，start/retry 各换一次轮次`
- `07C-4 不复制判定逻辑 > 节点卡与运行屏共用 runningDetailOf / idleHint / IdleHint，没有第二份实现`

### 2. DispatchOverlay 改为消费同一份进度状态

**已做**（`src-ui/src/components/center/graph/DispatchOverlay.tsx`）：

- `:55` Props 加 `progressStore: DispatchProgressStore`；`:65` 解构。
- 删掉本地的 `progress` / `now` / `statuses` 三份 state、`dispatch-progress` 的
  `listen` effect、5s 时钟 effect（原 `:89-93`、`:187-209`）。整个文件不再出现事件名
  `'dispatch-progress'`，也不再 import `@tauri-apps/api/event`。
- `:218` `start()`、`:274-278` `retry()` 各调一次 `progressStore.beginRun(...)`；
  `retry()` 改成先生成 `nextRunId` 再一起写（原来是 `setRunId(crypto.randomUUID())`，
  拿不到新值喂给共享层）。
- `:530-547` 运行行改走 `progressStore.snapshotOf(node.nodeId)` + `progressStore.now`，
  状态词从**同一份快照**读（`progressStatusLabel(snapshot.status)`，没有快照显示「排队」）。
  `statuses` 那份 state 原来只被事件与 `onStage` 的「排队」写，删掉后口径反而合一，
  `onStage` 回调（`:238`）简化成只写 `currentNodes`。

**已改测试**（`idle-and-tail-07b.test.tsx:209-215`，原 R1-2 那条接线用例）：
`progressOf(progress, node.nodeId, runId)` 改成 `progressStore.snapshotOf(node.nodeId)`，
`setProgress({})` / `setStatuses({})` 各 2 次改成 `progressStore.beginRun(` 2 次。
「叫停」文案与 `cancelDispatchNode(runId, nodeId)` 的断言原样保留。

**已补测试**：
- `07C-1 … > applyProgressEvent / progressRunOf：运行屏与节点卡读到同一份快照`
  （断两个消费者拿到的是**同一个对象**、就是 `run.progress.n1`，再断同一份快照喂给
  `runningDetailOf` 与 `NodeProgressRow` 得到同一句文案）
- `07C-1 … > 订阅只剩共享层一处：DispatchOverlay 不再自己 listen，两个消费者都走 snapshotOf`
  （事件名字面量在共享层出现且只出现 1 次；overlay 与 GraphTab 都不含事件名、不含
  `@tauri-apps/api/event`；GraphTab 建 store、传给运行屏、也喂给节点卡）

### 3. 节点卡上的运行中读数与疑似空转胶囊

**已做**（`src-ui/src/components/center/graph/DispatchBits.tsx:57-69`）：新增
`NodeProgressRow`——复用 `runningDetailOf` 与既有的 `IdleHint` 组件，自己只决定排版：
既没有读数也没有提示时不占行；读数过长时省略，完整文本进 `title`。

**已做**（`GraphTab.tsx`）：`:81` `const progressStore = useDispatchProgress(graphId)`；
`:570` 节点卡里渲染 `<NodeProgressRow progress={progressStore.snapshotOf(id)} now={progressStore.now} />`；
`:840` 把同一个 store 传给 `DispatchOverlay`。

**已做**（`GraphTab.tsx:35-37`）：`NODE_H` 56 改成 76。`.graph-node` 是 `overflow: hidden` 的定高卡，
不留出这一行的话运行中那行会被裁掉；`NODE_H` 只在本文件的布局计算里用（分层坐标、画布高度、
连线端点、按钮行位置），一处改全都跟着走。

**已做**（`GraphTab.css:250-266`）：`.graph-node-progress`（行内 gap 6px）、
`.graph-node-elapsed`（11px、`--text-3`、nowrap + 省略号）、
`.graph-node-progress .hint-capsule` 的 `flex-shrink: 0`——卡片只有 168px 宽，
读数让位给胶囊。不用红色（胶囊沿用既有的 `.hint-capsule` 黄色）。

**已补测试**：
- `07C-2 节点卡运行中显示耗时与工具 > NodeProgressRow：运行中渲染「已运行 … · 工具 …」，完整文本进 title`
  （心跳 80s + 本机墙上 10s 得到「已运行 1 分 30 秒 · 工具 edit_file」，断 class 与 title，
  断不含 `hint-capsule` 与 `dispatch-warning`）
- `07C-2 … > NodeProgressRow：同一套疑似空转胶囊，没快照/没读数时不占行`
  （重复工具 3 次 + 30s 无输出，与运行屏同一句「疑似空转：同一工具连续 3 次」；
  没快照、只有「排队」两种情况渲染出空串）
- `07C-2 … > NodeProgressRow：终态冻结，节点卡不跟着墙上时间涨`（终态心跳后过 5 分钟仍是
  「已运行 42 秒 · 工具 read_file」，且不挂胶囊）

### 4. 复位语义与 R1 一致

**已做 / 已补测试**：
- **按 runId 取快照**：`progressRunOf` 转调既有 `progressOf`，两个消费者共用。
- **重派/新一轮**：`beginRun` 在 `start()` 与 `retry()` 各一次；迟到的上一轮事件被
  `applyProgressEvent` 挡回同一引用。
  用例 `07C-3 复位：换图与重派 > beginProgressRun：重派后节点卡与运行屏都不显示上一轮读数`
  （上一轮 `elapsedMs 900000 / idleMs 600000` 本来会报「10 分钟无输出」；换轮次后节点卡渲染空串，
  迟到事件返回同一引用，本轮心跳写入后读数从 15 秒重新算）。
- **换图**：hook 里按 `graphId` 的 effect 复位成 `EMPTY_PROGRESS_RUN`。
  用例 `07C-3 … > 换图：复位成 EMPTY_PROGRESS_RUN，之后连本轮事件也不再落进来`
  （复位后 `snapshotOf` 为 undefined、节点卡渲染空串、本轮事件也不再写入，因为 runId 已是空串）。
- **终态冻结**：不清快照，`runningDetailOf` 自己冻结（R1-4 已有的行为），
  由 `07C-2` 第三条在节点卡这一路再钉一次。

## 没做

- **节点卡上的「叫停」**：按任务书不加（结果屏已有）。
- **hook 本身的渲染级测试**：仓库没装 jsdom / testing-library（`src-ui/package.json` devDependencies
  里只有 vitest，既有组件测试一律走 `react-dom/server` 的 `renderToStaticMarkup` 或读源文件断言）。
  所以 `useDispatchProgress` 的三个 effect 用「纯函数 + 源码断言」两头夹：状态转移全在
  `beginProgressRun` / `applyProgressEvent` / `progressRunOf` 里直接测，effect 的挂法
  （按 `graphId` 复位、runId 为空不挂监听、按 `run.runId` 重挂）按仓库既有做法读源文件断。
  没起窗口，所以「节点卡实际显示成什么样」只验到 DOM 字符串这一层。
- **卡片高度的目视核对**：`NODE_H` 76 是按 11px 行高 + 既有 8px padding 算的，没起窗口看。
  读数与长胶囊同时出现的极端情况靠 nowrap + 省略号 + `flex-shrink: 0` 兜底：胶囊完整显示，
  读数被省略号截断，完整文本在 `title` 里。

## 不确定处

- `retry()` 之后新一轮的监听要拆掉旧的再挂新的（effect 依赖 `run.runId`），
  这中间有一个极短的窗口可能丢事件。改之前 `DispatchOverlay` 的 effect 依赖 `[phase, runId]`，
  行为相同，没有新增风险；丢一条心跳的后果也只是读数晚 15s 更新。
- 状态词改成从快照读之后，「排队」不再由 `onStage` 主动写入，而是「还没有本轮快照」的默认值。
  两者在既有流程下等价（`onStage` 与第一条心跳之间节点确实在排队），但如果将来引擎在派发
  之外的时机发 `dispatch-progress`，这一格的语义会跟着变。

## 验证

两条命令都实际跑过，输出原文（去掉终端色码）：

```
D:\work\aicross-p5b\src-ui> npx vitest run --configLoader native --pool threads

 RUN  v5.0.0 D:/work/aicross-p5b/src-ui


 Test Files  23 passed (23)
      Tests  288 passed (288)
   Start at  22:06:56
   Duration  535ms (transform 59%, import 21%, tests 17%, worker 3%)

  Transform  transforming modules took 2.12s · 59% of tracked time, re-done on every run
             persist transforms across runs with fsModuleCache: true
             learn more: https://vitest.dev/guide/improving-performance#caching-between-reruns
```

本文件的 10 个用例（同一次运行加 `--reporter=verbose`，过滤本文件）：

```
 ✓ src/graph/__tests__/idle-and-tail-07c.test.tsx > 07C-1 进度状态提到共享层 > applyProgressEvent / progressRunOf：运行屏与节点卡读到同一份快照 7ms
 ✓ src/graph/__tests__/idle-and-tail-07c.test.tsx > 07C-1 进度状态提到共享层 > 订阅只剩共享层一处：DispatchOverlay 不再自己 listen，两个消费者都走 snapshotOf 1ms
 ✓ src/graph/__tests__/idle-and-tail-07c.test.tsx > 07C-1 进度状态提到共享层 > progressRunningAny：还有节点在跑才走时钟，全到终态就停 2ms
 ✓ src/graph/__tests__/idle-and-tail-07c.test.tsx > 07C-2 节点卡运行中显示耗时与工具 > NodeProgressRow：运行中渲染「已运行 … · 工具 …」，完整文本进 title 0ms
 ✓ src/graph/__tests__/idle-and-tail-07c.test.tsx > 07C-2 节点卡运行中显示耗时与工具 > NodeProgressRow：同一套疑似空转胶囊，没快照/没读数时不占行 1ms
 ✓ src/graph/__tests__/idle-and-tail-07c.test.tsx > 07C-2 节点卡运行中显示耗时与工具 > NodeProgressRow：终态冻结，节点卡不跟着墙上时间涨 1ms
 ✓ src/graph/__tests__/idle-and-tail-07c.test.tsx > 07C-3 复位：换图与重派 > beginProgressRun：重派后节点卡与运行屏都不显示上一轮读数 1ms
 ✓ src/graph/__tests__/idle-and-tail-07c.test.tsx > 07C-3 复位：换图与重派 > 换图：复位成 EMPTY_PROGRESS_RUN，之后连本轮事件也不再落进来 0ms
 ✓ src/graph/__tests__/idle-and-tail-07c.test.tsx > 07C-3 复位：换图与重派 > 共享层接线：换图按 graphId 复位，start/retry 各换一次轮次 1ms
 ✓ src/graph/__tests__/idle-and-tail-07c.test.tsx > 07C-4 不复制判定逻辑 > 节点卡与运行屏共用 runningDetailOf / idleHint / IdleHint，没有第二份实现 1ms
```

```
D:\work\aicross-p5b\src-ui> npm run build

> aicross-ui@0.0.0 build
> tsc -b && vite build

vite v8.0.7 building client environment for production...
transforming...✓ 323 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                                                  3.46 kB │ gzip:   1.32 kB
dist/assets/onig-Du5pRr7Y.wasm                                 466.61 kB │ gzip: 159.69 kB
dist/assets/xterm-sLQPUA_C.css                                   4.00 kB │ gzip:   1.02 kB
dist/assets/model-picker-Dx7sti2H.css                            4.75 kB │ gzip:   1.40 kB
（中间 45 个 lang-* 与样式 chunk 略）
dist/assets/main-Cujws9Tv.js                                   427.29 kB │ gzip: 211.33 kB
dist/assets/xterm-_r9IGi5_.js                                  465.79 kB │ gzip: 123.12 kB

✓ built in 725ms
```

顺手跑的 `npx eslint`（4 个改过的组件/模块文件）只剩 `GraphTab.tsx` 两条改动之前就有的
（`:82` 的 `useState<any>`、`:168` 的 exhaustive-deps 警告），新文件干净。

没起窗口、没发真机请求、没 commit、没 push；临时文件只在 `%TEMP%`。
改过的文件全是 LF，`git diff` 里没有 CR。
