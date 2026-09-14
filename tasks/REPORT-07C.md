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
  **（修正轮 R1 更正：这条当时不成立。）** 引擎的终态事件按契约不带心跳字段，
  R0 的 `progressSnapshot` 把缺字段补成 0，节点一收尾读数就变 0、整行消失——
  是清空不是冻结。`07C-2` 第三条喂的是带字段的终态事件，验的是假前提。
  已在 R1-1 修好，见下方「修正轮 R1」。

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
  **（修正轮 R1 更正：「胶囊完整显示」不成立。）** 最长那句约 160px，卡片内容宽只有
  168−24−2 = 142px；`flex-shrink: 0` + 不换行的写法下，读数被压到 0 宽、胶囊自己溢出，
  尾部被 `.graph-node { overflow: hidden }` 裁掉。已在 R1-4 改成换行 + 显式行高，
  `NODE_H` 同步加高，见下方「修正轮 R1」。

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

## 修正轮 R1

来源 `tasks/fix-07c-r1.md` 的 4 条必修（DeepSeek 盲审 `tasks/review-07c-ds.md` 的 F1–F4）。
GLM 盲审 `tasks/review-07c-glm.md` 在修完这一轮之后才到，逐条比对：它的「缺项 1」＝ DS F2、
「缺项 4」的前两例 ＝ DS F2/F3 的测试，都已在本轮覆盖；不重复的两条见本节末尾。
只改 `src-ui/` 与 `tasks/`，没动 `src/`；行号是改完之后的。

### R1-1 终态事件保留上一条心跳读数（DS F1）

**已修**：`src-ui/src/graph/dispatch.ts:1065-1071` —— `progressSnapshot` 加第三个参数
`previous?: ProgressSnapshot`；payload 既无 `elapsedMs` 也无 `idleMs`（`hasHeartbeat` 为假）
且上一份快照同 `runId` 时，直接 `{ ...previous, status: payload.status }`：只换状态词，
`elapsedMs`/`idleMs`/`toolCalls`/`repeatedToolCalls`/`lastTool`/`hasHeartbeat`/`receivedAt`
全部沿用。`:1097` 的 `nextProgress` 把 `current[payload.nodeId]` 喂进去。
没有上一条心跳、或 `runId` 对不上（跨轮），仍走原来的补 0 路径，不编数。
终态耗时的冻结仍由 `runningDetailOf` 的 `progressRunning` 判据完成（非 running 时墙上时间 0）。

**已补**：`07C-R1-1 终态事件保留上一条心跳读数 > 无字段终态事件后耗时与工具仍显示且不再增长`
（只喂 `{ runId, nodeId, status: '完成' }`：读数仍是 42s / read_file，过 5 分钟不变，
不挂胶囊，`progressRunningAny` 为假；再钉两条负向——跨 runId 的旧快照不借用、
没有上一条心跳时整行不渲染）。

### R1-2 listen 取消守卫（DS F2）

**已修**：`src-ui/src/graph/dispatch-progress.ts:73-95` —— effect 体抽成
`subscribeProgress(runId, onEvent) -> { dispose }`：`runId` 为空直接返回空 `dispose`；
`disposed` 标志 + `.then((stop) => { if (disposed) stop(); else unlisten = stop; })`，
`dispose()` 里 `disposed = true; unlisten?.(); unlisten = undefined;`（重复 dispose 不重复解除）。
原来被静默吞掉的 `.catch(() => {})` 改成 `console.warn('派发进度订阅失败', error)`（`:89`）。
hook 的 effect（`:124-130`）只剩建订阅 + `dispose`，依赖仍是 `[run.runId]`，
事件名字面量仍只在共享层出现一次。

**已补**：
- `07C-R1-2 listen 取消守卫 > subscribeProgress：换 runId 与卸载各 unlisten 一次，迟到的 listen 立即解除`
  （`vi.mock('@tauri-apps/api/event')` + `vi.hoisted` 的可控桩，把 `listen` 的 resolve 攥在手里：
  空 runId 不订阅；换 runId 一次、卸载一次，各 unlisten 一次；dispose 早于 listen 解析时，
  解析出来的监听就地被 stop）
- `07C-R1-2 … > subscribeProgress：订阅失败留痕而不是静默吞掉`

### R1-3 异常收尾后时钟停（DS F3）

**已修**：`src-ui/src/graph/dispatch-progress.ts:56-70` —— 新增
`PROGRESS_ENDED_STATUS = '已结束'` 与纯函数 `endProgressRun(run, status?)`：把还在跑的快照
标成终态、读数原样保留，全终态时返回同一引用（空操作）。store 的死接口 `resetRun` 删掉，
换成 `endRun`（`:111`、`:144`、`:148`）。`src-ui/src/components/center/graph/DispatchOverlay.tsx:260-265`
给 `start()` 的 try/catch 加 `finally { progressStore.endRun(); }`——异常路径、
`missing-target` 早退、正常跑完都会经过它；`retry()` 走的是 `beginRun`，本来就把快照清空，
不需要另外收尾。全终态之后 `progressRunningAny` 为假，5s 时钟不再重建。

**已补**：`07C-R1-3 异常收尾后时钟停 > endProgressRun：收尾后 now 不再前进、不挂胶囊`
（fake timers：两个节点在跑时 `now` 每 5s 前进 4 次；`endProgressRun` 之后门控翻假、
interval 被清、再推进 10 分钟 `now` 不动；读数仍是最后一条心跳，且不挂胶囊；
重复调用返回同一引用）与 `07C-R1-3 … > 接线：start() 的 finally 调 endRun，死接口 resetRun 已删`。

### R1-4 胶囊不裁切（DS F4）

**已修**：
- `src-ui/src/components/center/graph/GraphTab.css:255-278` —— `.graph-node-progress`
  加 `flex-wrap: wrap`、`align-items: flex-start`、`row-gap: 2px`、显式 `line-height: 1.4`；
  `.graph-node-elapsed` 加 `flex: 1 1 100%`（读数单独占一行，过长仍省略、完整文本在 `title` 里）
  与显式 `line-height`；`.graph-node-progress .hint-capsule` 的 `flex-shrink: 0` 换成
  `max-width: 100%` + `white-space: normal` + `overflow-wrap: anywhere` + 显式 `line-height`，
  最长那句在 142px 内容宽里换成两行完整显示，不再溢出被裁。
- `src-ui/src/components/center/graph/GraphTab.css:223` —— `.graph-node-row` 补显式
  `line-height: 1.4`，让卡高可算。
- `src-ui/src/components/center/graph/GraphTab.tsx:35-40` —— `NODE_H` 76 → 116：
  16.8（12px 行）+ 15.4（11px 行）+ 第三行 54.2（读数 15.4 + 行距 2 + 胶囊两行 36.8）
  + 行间距 8 + 上下 padding 16 + 边框 2 = 112.4，取 116 留余量。`NODE_H` 仍是唯一常量，
  分层坐标、画布高、连线端点、按钮行位置全部派生。
- 顺手（GLM 小疵）：`DispatchBits.tsx:64` 只有胶囊没有读数时不再挂空的 `title=""`。

**已补**：`07C-R1-4 节点卡胶囊不裁切 > 最长空转文案在卡宽内换行显示完整，不靠 overflow 截断`
（样式断言：第三行 `flex-wrap: wrap` + 显式行高，胶囊 `white-space: normal` + `max-width: 100%`
且不再 `flex-shrink: 0`，读数 `flex: 1 1 100%` + `text-overflow: ellipsis`，`NODE_H >= 112`；
字符串断言：最长文案原样渲染成 `<span class="hint-capsule">疑似空转：同一工具连续 3 次</span>`，
markup 里没有省略号）。

**如实说明**：没起窗口目视。116 是按上面的行高/padding 算出来的，只验到样式规则与 DOM
字符串这一层；真机上字体度量与算出来的值有出入时，卡片是「留白多一点/少一点」，
不会再退回裁切——胶囊已经可换行、卡片高度按两行胶囊留足。

### GLM 盲审里不重复的两条

- **不采纳（本期不改，记录代价）**：GLM「缺项 2」——派发进行中切到别的图再切回来，
  `[graphId]` 复位把 `run.runId` 清成空串，监听 effect 不再挂，这一轮的读数与胶囊永久消失
  （不是晚 15s 恢复）。契约明写「换图复位」，`07C-3` 的用例也钉着这个行为；要修得把 run 按
  graphId 存成 `Record<graphId, ProgressRun>`，超出本轮范围。记在这里作为已知代价。
- **不采纳**：GLM「缺项 5」后半——store 暴露的 `runId`/`progress` 两个字段目前没有消费者。
  它们是 `snapshotOf` 的原料、也是后续把 run 按 graphId 存 map 时的入口，删了反而要再加回来；
  `resetRun` 那个真死接口已在 R1-3 删掉。
- **已覆盖**：GLM「缺项 1」＝ DS F2（R1-2）；「缺项 4」的前两例＝ R1-2 与 R1-3 的新测试。
  「缺项 3」（重挂窗口可能丢一跳）GLM 自己的结论是「不处理，观察即可」，R0 报告已如实记录。

### 验证（两条命令都实际跑过，输出原文，去掉终端色码）

```
D:\work\aicross-p5b\src-ui> npx vitest run --configLoader native --pool threads

 RUN  v5.0.0 D:/work/aicross-p5b/src-ui


 Test Files  23 passed (23)
      Tests  294 passed (294)
   Start at  22:30:21
   Duration  657ms (transform 52%, import 21%, tests 19%, worker 7%)
```

288 → 294，新增的 6 条（同一次运行加 `--reporter=verbose`，过滤本文件）：

```
 ✓ src/graph/__tests__/idle-and-tail-07c.test.tsx > 07C-R1-1 终态事件保留上一条心跳读数 > 无字段终态事件后耗时与工具仍显示且不再增长 1ms
 ✓ src/graph/__tests__/idle-and-tail-07c.test.tsx > 07C-R1-2 listen 取消守卫 > subscribeProgress：换 runId 与卸载各 unlisten 一次，迟到的 listen 立即解除 88ms
 ✓ src/graph/__tests__/idle-and-tail-07c.test.tsx > 07C-R1-2 listen 取消守卫 > subscribeProgress：订阅失败留痕而不是静默吞掉 2ms
 ✓ src/graph/__tests__/idle-and-tail-07c.test.tsx > 07C-R1-3 异常收尾后时钟停 > endProgressRun：收尾后 now 不再前进、不挂胶囊 5ms
 ✓ src/graph/__tests__/idle-and-tail-07c.test.tsx > 07C-R1-3 异常收尾后时钟停 > 接线：start() 的 finally 调 endRun，死接口 resetRun 已删 1ms
 ✓ src/graph/__tests__/idle-and-tail-07c.test.tsx > 07C-R1-4 节点卡胶囊不裁切 > 最长空转文案在卡宽内换行显示完整，不靠 overflow 截断 1ms
```

```
D:\work\aicross-p5b\src-ui> npm run build

> aicross-ui@0.0.0 build
> tsc -b && vite build

（中间 60 余个 lang-* 与样式 chunk 略）
dist/assets/lang-cpp-macro-CgwvTOCX.js                         224.24 kB │ gzip:  18.05 kB
dist/assets/lang-cpp-BAHFRj0j.js                               402.02 kB │ gzip:  31.11 kB
dist/assets/main-DGF0X_pt.js                                   427.69 kB │ gzip: 211.48 kB
dist/assets/xterm-_r9IGi5_.js                                  465.79 kB │ gzip: 123.12 kB

✓ built in 589ms
```

没起窗口、没发真机请求、没 commit、没 push；临时文件只在 `%TEMP%`。
`git diff` 里没有 CR，六个改过的文件都保持原有的 LF。
