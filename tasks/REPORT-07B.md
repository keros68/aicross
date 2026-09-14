# REPORT-07B 收尾与 G2 空转提示 · B. 界面（TS）

工作树 `D:\work\aicross-p5b`，分支 `phase/7b`（基于 `main`）。只动 `src-ui/` 与 `tasks/`，`src/`（Rust）一行未改。
A 侧（引擎）心跳字段本期没接通，`tauri.ts` 按契约先写类型，缺字段按 0 / 空串处理。

## 契约（按任务书「契约」节，只追加）

- `src-ui/src/tauri.ts:526-544` 新增 `DispatchProgress`：`runId` / `nodeId` / `status` 三个既有字段 +
  可选 `elapsedMs` / `idleMs` / `toolCalls` / `repeatedToolCalls` / `lastTool`。注释里写明 `status` 除既有中文
  状态词外新增 `"running"`（15s 一条的心跳），节点结束时最后一条仍是原有终态。
- `src-ui/src/graph/types.ts:20-22` `Reply` 追加可选 `tokens_cache_read`。
- `src-ui/src/graph/dispatch.ts:23-24` `BackendNodeResult` 追加可选 `cacheRead`（`DispatchNodeResult` 早有该字段，
  两边形状对齐后 `applyDispatchResults` 才能写图）。
- 未知字段 / 未知 status 的兼容由 `progressSnapshot`（`dispatch.ts:1052`）与 `progressStatusLabel`（`:1065`）兜：
  旧引擎只发 status 时全部按 0 / 空串，界面不出任何空转提示。

## 逐条

### 1. G2「疑似空转」提示

**已做**：
- 纯函数（`src-ui/src/graph/dispatch.ts`）：
  - `ProgressSnapshot`（`:1038`）——一条心跳落到界面上的快照，多带一个 `receivedAt`（本机收到的时刻）。
    引擎整个卡死时不会再有心跳，「无输出多久」只能靠它加上墙上时间继续长，否则读数永远停在最后一条心跳上。
  - `IDLE_HINT_MS = 3 * 60 * 1000`、`REPEATED_TOOL_LIMIT = 3`（`:1049-1050`）。
  - `progressSnapshot(payload, receivedAt)`（`:1052`）缺字段补 0 / 空串；`progressStatusLabel`（`:1065`）把心跳的
    `"running"` 显示成既有的「运行中」，别的状态原样显示（否则心跳会把「运行中」冲成英文字面量）。
  - `idleHint(progress, now)`（`:1080`）：节点还在跑（status 是 `running` / 「运行中」）时，`repeatedToolCalls >= 3`
    → 「疑似空转：同一工具连续 N 次」；否则 `idleMs + (now - receivedAt) >= 180000` → 「疑似空转：N 分钟无输出」
    （阈值处正好是 3）。排队、等待写锁与终态一律不提示。
  - `runningDetailOf(progress, now)`（`:1098`）：「已运行 1 分 20 秒 · 工具 read_file」，没有工具时只报耗时，
    心跳还没到（`elapsedMs === 0`）返回空串。
- 组件：`src-ui/src/components/center/graph/DispatchBits.tsx:49-55` `IdleHint`——`hint-capsule`（既有的黄色小胶囊），
  不是 `dispatch-warning` 红字。
- 接线：`DispatchOverlay.tsx:189-196` 事件监听改吃 `DispatchProgress`，同时写 `statuses`（显示词）与
  `progress`（快照）两份 state；`:200-207` 运行屏开着时每 5s 走一格的时钟（两条心跳之间与心跳停掉之后读数继续走）；
  `:547-564` 运行中每一行左列是路由 +「已运行 / 工具」，右列是胶囊提示 + 原来那个按钮，文案「取消」→「叫停」。
- 样式：`GraphTab.css:51-54` `.dispatch-running-main` / `.dispatch-running-detail` / `.dispatch-running-actions`
  （三列栅格不变，胶囊与按钮同在最后一格）。

**已补测试**（`src-ui/src/graph/__tests__/idle-and-tail-07b.test.tsx`）：
- `07B-1 G2 疑似空转提示 > idleHint：正常运行不提示`
- `07B-1 G2 疑似空转提示 > idleHint：同一工具连续 3 次 → 疑似空转`
- `07B-1 G2 疑似空转提示 > idleHint：3 分钟无输出 → 疑似空转，心跳停了也按墙上时间继续长`
- `07B-1 G2 疑似空转提示 > progressSnapshot：后端缺字段按 0 / 空串；running 显示成「运行中」`
- `07B-1 G2 疑似空转提示 > 运行中一行：显示已运行与工具；胶囊用 hint-capsule，不是红字`

边界：`elapsed` / `lastTool` 与胶囊挂在 G2 运行屏的节点行上。图画布上的节点卡不订阅 `dispatch-progress`
（这份 state 住在派发弹窗里），那里仍只有 `nodeStatusLabel` 的「运行中」。

### 2. 对账说明随派发收敛（DS 06B-R1 3）

**已做**：`dispatch.ts:1108-1116` `reconcileUiConverged(state, graph)`：先过一道 `reconcileUiFor`（换图即空），
再按 `dispatchableNodeIds(graph)` 剔除已经跑完 / 已判失败的待核对 id；一个不剩时返回 `EMPTY_RECONCILE_UI`
（工具条那句陈旧说明一并清掉），还剩就把说明改成「还有 N 个节点待核对」。没有变化时返回同一引用。
`GraphTab.tsx:832-838` 把传给 `DispatchOverlay` 的 `onCommit` 包一层：每次写回图之后收敛一次
（`commit` 返回落盘后的图，直接拿它算）。

**已补测试**：
- `07B-2 对账说明随派发收敛 > reconcileUiConverged：派发完成的节点从待核对剔除，一个不剩时说明一起清`
- `07B-2 对账说明随派发收敛 > reconcileUiConverged：换图后当空的`

### 3. Reply.tokens_cache_read

**已做**：
- 类型见「契约」节。
- `dispatch.ts:267` `applyDispatchResults` 写 `tokens_cache_read: result.cacheRead || undefined`（0 与缺失都不写）。
- `dispatch.ts:842` `applyReconcile` 的 completed 分支写 `row.usage.cacheRead`——06B R1 申报的那个「收不回来」
  的边界闭掉了。
- `dispatch.ts:1120-1125` `cacheReadOf(reply, result)`：优先读图上的 `tokens_cache_read`，图上没有才用本次结果。
  结果屏 cache_read 列（`DispatchOverlay.tsx:678`）与阶段小计（`:382`）都走它，两处同一个口径。

**已补测试**：
- `07B-3 Reply.tokens_cache_read > applyDispatchResults 写 tokens_cache_read，0 与缺失都不写`
- `07B-3 Reply.tokens_cache_read > applyReconcile 收回 usage.cacheRead`
- `07B-3 Reply.tokens_cache_read > cacheReadOf：结果屏优先读图字段，图上没有才用本次结果`

### 4. start() 幂等停止分支整链测试（GLM 06B-R1 3 / DS 5）

**已做**：`DispatchOverlay.start()` 的整个 while 循环搬进 `dispatch.ts:1164` 的
`runStageLoop(state, settings, skip, deps)`：预览 / 派发 / 提交 / 三个回调（`onPreview` / `onStage` / `onResults`）
全部注入，函数本身不碰 IO 也不碰 React。
- `StageLoopState`（`:1134`）由调用方持有、循环就地更新——派发抛错时组件的 catch 分支还要读
  `graph` / `collected` / `activeNodes`，返回新副本那半边就拿不到了。
- `StageLoopOutcome`（`:1156`）三种：`done` / `stopped`（带提示文案）/ `missing-target`（带回确认屏的节点）。
  两句固定文案提成常量 `DUPLICATE_BATCH_TEXT`（`:1161`）、`MISSING_TARGET_TEXT`（`:1162`），测试断的是同一份字符串。
- `DispatchOverlay.tsx:236-281` 改成调用它，行为逐条对齐原实现（幂等记账、重复批次先 `resetNodesToDraft`
  再停、limit 提示、`reducePipelineStep` 推进、catch 里补 `failedNodesWithoutResults`）。

**已补测试**：
- `07B-4 start() 幂等停止分支整链 > runStageLoop：重复批次 → 本层放回 draft、给提示，且不再推进下一层`
  （两层图，第二层派 b 时引擎把第一层那批 a 原样重放回来 → 批次 id 撞上；断言 outcome 是
  stopped + DUPLICATE_BATCH_TEXT、dispatch 只被调 2 次、b 回到 draft 不悬挂 pending、重放那批不进 collected）
- `07B-4 start() 幂等停止分支整链 > runStageLoop：批次不重复时照常推进两层后收工`（对照）
- `07B-4 start() 幂等停止分支整链 > runStageLoop：下一层缺目标时回确认屏，不发这一层`（对照）

### 5. 小质量（DS 06B-R1 7）

**已做**：
- `src-ui/src/global.d.ts` 删掉手写的 node:fs 模块声明；读文件的那条断言改成与
  `lib/__tests__/shiki.test.ts` 同一套写法（`@ts-expect-error node builtin, not in this project's types`），
  改在 `consistency-06b.test.tsx:4-8`，新测试文件同样。
- 常规提示不再用红字（`.dispatch-warning` 只留给异常）：`DispatchOverlay.tsx:406`「单步小任务…」、
  `:455`「复核，非独立验证」、`:588`「已切备选：主 … → 备 …」三处改 `dispatch-reason`。
  仍是红字的：未指定模型 / 超请求上限 / thinking 已调 / 未冒烟 / 输出已截断 / 输入超 50k / 脱敏连带替换。
- `dispatch.ts:815-818` `applyReconcile` 的注释补一句「没有 reply 的节点状态取不到，按非 pending / draft 一起跳过」。
- 报告 R0 行号漂移按任务书不改。

**已补测试**：
- `07B-5 小质量 > global.d.ts 不再手写 node:fs 声明`
- `07B-5 小质量 > 常规提示不用红字：单步小任务 / 复核，非独立验证 / 已切备选 都走 dispatch-reason`
  （读 `DispatchOverlay.tsx` 源文件断这三行的 className，并反向断「未指定模型」那条仍是红的）

## 验证

两条命令都实际跑过，输出原文（去掉终端色码）：

```
D:\work\aicross-p5b\src-ui> npx vitest run --configLoader native --pool threads

 RUN  v5.0.0 D:/work/aicross-p5b/src-ui


 Test Files  22 passed (22)
      Tests  269 passed (269)
   Start at  21:11:39
   Duration  527ms (transform 65%, tests 16%, import 16%, worker 3%)

  Transform  transforming modules took 2.06s · 65% of tracked time, re-done on every run
             persist transforms across runs with fsModuleCache: true
             learn more: https://vitest.dev/guide/improving-performance#caching-between-reruns
```

本期新增的 15 个用例（同一次运行加 `--reporter=verbose`，过滤本文件）：

```
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-1 G2 疑似空转提示 > idleHint：正常运行不提示 3ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-1 G2 疑似空转提示 > idleHint：同一工具连续 3 次 → 疑似空转 1ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-1 G2 疑似空转提示 > idleHint：3 分钟无输出 → 疑似空转，心跳停了也按墙上时间继续长 0ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-1 G2 疑似空转提示 > progressSnapshot：后端缺字段按 0 / 空串；running 显示成「运行中」 1ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-1 G2 疑似空转提示 > 运行中一行：显示已运行与工具；胶囊用 hint-capsule，不是红字 10ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-2 对账说明随派发收敛 > reconcileUiConverged：派发完成的节点从待核对剔除，一个不剩时说明一起清 2ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-2 对账说明随派发收敛 > reconcileUiConverged：换图后当空的 0ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-3 Reply.tokens_cache_read > applyDispatchResults 写 tokens_cache_read，0 与缺失都不写 1ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-3 Reply.tokens_cache_read > applyReconcile 收回 usage.cacheRead 1ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-3 Reply.tokens_cache_read > cacheReadOf：结果屏优先读图字段，图上没有才用本次结果 1ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-4 start() 幂等停止分支整链 > runStageLoop：重复批次 → 本层放回 draft、给提示，且不再推进下一层 4ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-4 start() 幂等停止分支整链 > runStageLoop：批次不重复时照常推进两层后收工 0ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-4 start() 幂等停止分支整链 > runStageLoop：下一层缺目标时回确认屏，不发这一层 0ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-5 小质量 > global.d.ts 不再手写 node:fs 声明 0ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-5 小质量 > 常规提示不用红字：单步小任务 / 复核，非独立验证 / 已切备选 都走 dispatch-reason 1ms
```

```
D:\work\aicross-p5b\src-ui> npm run build

> aicross-ui@0.0.0 build
> tsc -b && vite build

vite v8.0.7 building client environment for production...
transforming...✓ 322 modules transformed.
dist/index.html                                                  3.46 kB │ gzip:   1.32 kB
dist/assets/xterm-sLQPUA_C.css                                   4.00 kB │ gzip:   1.02 kB
dist/assets/main-HZNsW-QN.css                                  126.60 kB │ gzip:  22.25 kB
dist/assets/main-BuRTlr6Q.js                                   426.51 kB │ gzip: 211.10 kB
dist/assets/xterm-_r9IGi5_.js                                  465.79 kB │ gzip: 123.12 kB
✓ built in 628ms
```

基线对照：改动前是 `21 passed (21)` / `254 passed (254)`，本期 +1 个测试文件、+15 个用例。没有 chunk>500kB 警告。

附带核对（非任务书要求）：`npx eslint` 对改过的 `dispatch.ts`、`tauri.ts`、`types.ts`、`global.d.ts`、
`DispatchOverlay.tsx`、`DispatchBits.tsx` 与两个测试文件零问题；`GraphTab.tsx` 仍是既有的 1 error + 1 warning
（roles 的 any、一条 exhaustive-deps），与 HEAD 版本同数，没有新增。
换行逐文件核对过：`tauri.ts` 仍是 CRLF/LF 混排的既有状态（95 行 LF 原样保留，新增 20 行按所在区段用 CRLF），
其余改过的文件仍是纯 LF；没跑格式化器，`git diff` 只有目标行。

## 没做 / 边界

- `src/`（Rust）一行未动；A 侧那 7 条（心跳发射、before 快照保留、零改动不覆盖快照、`latest_before_dir`
  fail-closed、serde default、`usage.cacheRead` 回填、测试补强）不在本期。
- 没起窗口、没发任何真机模型请求、没 commit、没 push；没往仓库以外写任何文件。
- 心跳字段的真实取值没有实测：后端还没发 `elapsedMs` / `idleMs` / `repeatedToolCalls`，本期只由类型与
  「缺字段按 0」的测试保证。接通后要在真窗口里看一次读数是否合理。
- 图画布上的节点卡没有 elapsed / 工具 / 空转胶囊（`dispatch-progress` 只在派发弹窗里订阅），见第 1 条边界。
- 运行屏的时钟固定 5s 一跳：胶囊最迟比真实的 3 分钟晚 5 秒出现。

## 合并时要对账的点

- `DispatchProgress` 的字段名按契约是 camelCase（`elapsedMs` / `idleMs` / `toolCalls` / `repeatedToolCalls` /
  `lastTool`），Rust 侧的 serde rename_all 已经是这个口径，新字段照加即可。
- 心跳的 `status` 前端认的是字面量 `running`；别的值原样当显示词用（现有的「排队」「等待写锁」「运行中」
  与终态都不用改）。心跳只在节点真在跑时发——前端只对 running / 「运行中」出空转提示。
- `idleMs` 的口径是「距上一条输出/通知」，前端会再加上「本机收到这条心跳之后过了多久」。所以引擎不必在
  卡死时补发心跳，但也别把 `idleMs` 归零重发。
- `Reply.tokens_cache_read` 是本期新增的图字段，Rust 镜像加 serde default；对账的
  `ReconcileNodeSummary.usage.cacheRead` 现在前端会写进图，A 侧照契约填上。

## 修正轮 R1

来源 `tasks/fix-07b-r1.md` 7 条（DeepSeek 盲审 `tasks/review-07b-ds.md`）。本轮开工后 `tasks/review-07b-glm.md`
也到了：它的发现 1 / 2 / 3 分别与 DS 2.1 / 2.2 / 2.5 同题，随那三条一并修；GLM「最该补的 3 个测试」也被
R1-1 / R1-2 / R1-5 的新用例覆盖。除此之外 GLM 没有与这 7 条不重复的必修项（它的第 4 条即 DS 1.1，按第 6 条
写回任务书）。行号是改完之后的。

### R1-1 无心跳元数据的快照不累计墙上时间（DS 2.1 高 / GLM 发现 1）

**已修**：
- `src-ui/src/graph/dispatch.ts:1051` `ProgressSnapshot` 加 `hasHeartbeat: boolean`；`:1071`
  `progressSnapshot` 按 `payload.elapsedMs !== undefined || payload.idleMs !== undefined` 计算——两个字段
  任一存在就算带心跳。
- `:1118` `idleHint` 的门槛加 `|| !progress.hasHeartbeat`：只有状态词的事件（旧引擎的一次性「运行中」、
  将来掉了字段的心跳）只显示状态，不判空转。这类事件不会被下一条心跳纠正，累计墙上时间就是必然误报。
- 说明相应改到 `:1034-1041` 的注释里。REPORT R0「旧引擎只发 status 时界面不出任何空转提示」这句话原来
  只在 `now === receivedAt` 那一刻成立，现在是真的。

**已补测试**（`src-ui/src/graph/__tests__/idle-and-tail-07b.test.tsx`）：
- `07B-1 G2 疑似空转提示 > idleHint：没有心跳元数据的快照永不判空转，3 分钟后也不判`
  （断 `hasHeartbeat === false`，再断 3 分钟、1 小时后都是 null；反向断只要带 `idleMs: 0` 一个字段就恢复按
  真读数判，3 分钟后出提示）。改之前这条会红。
- `07B-1 … > progressSnapshot：后端缺字段按 0 / 空串；running 显示成「运行中」` 的整体等值断言同步加上
  `hasHeartbeat: false` 与 `runId`。

### R1-2 新一轮复位进度状态（DS 2.2 中 / GLM 发现 2）

**已修**：两条路都堵上——快照带 runId 过滤 + 组件显式清空。
- `dispatch.ts:1044` `ProgressSnapshot` 加 `runId`；`:1078` 新纯函数
  `nextProgress(current, payload, now, runId)`：别轮次的事件原样返回 current（同一引用），本轮的才写快照；
  `:1090` 新纯函数 `progressOf(progress, nodeId, runId)`：快照 runId 对不上就当没有。
- `DispatchOverlay.tsx:195` 事件监听改调 `nextProgress`；`:561` 运行行先 `progressOf(progress, node.nodeId, runId)`
  再算 `runningDetailOf` / `IdleHint`，不再直接取 `progress[node.nodeId]`。
- `DispatchOverlay.tsx:244-246`（`start()`）与 `:307-309`（`retry()`）各加
  `setProgress({}); setStatuses({}); setCurrentNodes([]);`——`retry()` 复用同一个 nodeId，上一轮叫停/抛错/
  丢事件留下的 `running` 快照会让新一行一出现就显示上一轮耗时甚至立刻挂胶囊。

**已补测试**：
- `07B-1 G2 疑似空转提示 > nextProgress / progressOf：重派后不显示上一轮读数、不立刻出胶囊`
  （上一轮快照 `elapsedMs 900000 / idleMs 600000` 本来会报「10 分钟无输出」；换 runId 后 `progressOf` 为
  undefined、`runningDetailOf` 空串、`idleHint` null、`IdleHint` 渲染出空串；上一轮迟到的事件 `nextProgress`
  返回同一引用；本轮心跳写入后读数从 15 秒重新算）。改之前这条会红。
- `07B-1 … > 运行行接线：按钮文案是「叫停」，走 cancelDispatchNode(runId, nodeId)；start/retry 清进度`
  里断 `setProgress({})` / `setStatuses({})` / `setCurrentNodes([])` 各出现 2 次（start + retry）。

### R1-3 重复工具计数加空闲门槛（DS 2.4）

**已修**：
- `dispatch.ts:1059` 新常量 `REPEATED_TOOL_IDLE_MS = 30 * 1000`；`:1119-1121` 判定改成
  `repeatedToolCalls >= 3 && idle >= 30000`（`idle` 仍是心跳 `idleMs` 加上本机收到之后的墙上时间）。
  连续读 3 个文件几秒内就会命中计数，那时还在产出，不该提示。
- 契约同步：`tasks/TASK-07-idle-and-tail.md` B-1 改成「**同名工具连续 ≥3 且 30 秒无输出**，或 `idleMs ≥ 180000`」，
  并注明缺心跳元数据的事件一律不判。

**已补测试**：
- `07B-1 G2 疑似空转提示 > idleHint：同一工具连续 3 次且 30 秒无输出 → 疑似空转`（原
  「同一工具连续 3 次 → 疑似空转」改写：断 `repeated 3 + idle 2s` 不提示、`idle = 阈值-1` 不提示、
  `repeated 2 + idle 30s` 不提示、`repeated 3 + idle 30s` 提示、空闲由墙上时间凑满也提示）。

### R1-4 终态快照冻结耗时（DS 2.3）

**已修**：`dispatch.ts:1138` `runningDetailOf` 只在 `progressRunning(progress)` 为真时加墙上时间，终态
（完成/失败/超时…）按最后一条心跳的 `elapsedMs` 冻结。这些行还挂在本层的 `currentNodes` 上，原来读数会一直涨。

**已补测试**：
- `07B-1 G2 疑似空转提示 > runningDetailOf：终态快照冻结耗时，不再跟着墙上时间涨`
  （`status: '完成'` 的快照过 5 分钟仍是「已运行 42 秒」；`status: '失败'` 带工具名同理；还在跑的那条照旧继续走）。

### R1-5 `reconcileUiConverged` 按终态过滤（DS 2.5 / GLM 发现 3）

**已修**：`dispatch.ts:1155-1158` 判据从「是否在 `dispatchableNodeIds` 里」改成节点自己的状态：只保留仍是
`draft` 且正文还是 `RECONCILE_UNKNOWN_TEXT` 的 id，与节点卡「待核对」标签（`dispatch.ts:360`）同一个口径。
`:1145-1151` 的注释改成与代码一致。原实现会把「父节点未完成 / 目标没选 / 正在飞」的待核对节点也剔掉，
工具条计数与节点卡对不上；更麻烦的是剔除后 `initialDispatchSelection` 不再默认跳过它，父节点跑完就会被
自动勾上派出去。

**已补测试**：
- `07B-2 对账说明随派发收敛 > reconcileUiConverged：按终态过滤——在飞的剔除，暂不可派但仍待核对的 draft 保留`
  （父节点未完成、目标没选两种「暂不可派」都返回同一引用不剔除；`pending` 剔除；用户已把那句「未知」改掉的
  也剔除）。
- 既有两条同步改成用 `RECONCILE_UNKNOWN_TEXT` 建图，终态那半边改成 `done` + `failed` 各一个。

### R1-6 节点卡口径写回任务书（DS 1.1）

**已修**：`tasks/TASK-07-idle-and-tail.md` B-1 加括注「R1 收窄：本期只覆盖 G2 运行行；**节点卡的
elapsed/lastTool 下期做**，需把 `dispatch-progress` 订阅从 `DispatchOverlay` 提到 `GraphTab`」。本轮不做这个大改。

### R1-7 测试补强（DS 5.3–5.5）

**已补 / 已改**：
- `07B-1 G2 疑似空转提示 > 运行行接线：按钮文案是「叫停」，走 cancelDispatchNode(runId, nodeId)；start/retry 清进度`
  ——断 `cancelDispatchNode(runId, node.nodeId)` 接线、`>叫停</button>` 文案、快照走 `progressOf` 且源码里
  不再出现 `progress[node.nodeId]`。组件整屏渲染要 tauri 事件桥，按仓库既有先例读源文件断，正则放宽到跨空白。
- `07B-4 start() 幂等停止分支整链 > runStageLoop：达总请求上限时停在本层之前，给上限文案且不再派发`
  （`maxRequests: 1`，第二层超限：outcome 是 stopped + 字面量「已达总请求上限 1，继续派发需重新确认。」，
  `dispatch` 只调 1 次，超限那层没 markPending 仍是 draft）。
- `07B-4 … > runStageLoop：dispatch 抛错时 state 留下本层 activeNodes 与已收结果，供组件 catch 收尾`
  （第二层抛错 → 断 `state.activeNodes` 是本层、`collected` 是已收的、图上第一层 done 第二层 pending——
  正是组件 catch 分支要读的三样）。
- 重复批次那条的 outcome 改用字面量断言，另加一条 `DUPLICATE_BATCH_TEXT` 与字面量相等的断言，文案改错会红。
- `07B-2 对账说明随派发收敛 > GraphTab 接线：每次 onCommit 写回图之后收敛一次`（读 `GraphTab.tsx` 断
  `const saved = commit(next)` 与 `setReconcile((current) => reconcileUiConverged(current, saved))`）。
- 源码扫描断言改宽松：`07B-5 小质量` 两条不再按「文案与 className 同一行」找，改成
  `className="dispatch-reason"[^<]*文案` 的正则；`node:fs` 那条改成 `/declare\s+module\s+['"](node:)?fs['"]/`，
  单双引号都挡。读源文件统一走新的 `readSource()` 小工具。
- `GraphTab.css:375` 注释「校验提示」改「提示胶囊：…（校验提示与疑似空转共用）」，新测试
  `07B-5 小质量 > hint-capsule 的样式注释改成「提示胶囊」（校验提示与疑似空转共用）` 钉住。

### 不采纳

无。7 条全部处理。

## 验证（修正轮 R1）

两条命令都实际跑过，输出原文（去掉终端色码）：

```
D:\workicross-p5b\src-ui> npx vitest run --configLoader native --pool threads

 RUN  v5.0.0 D:/work/aicross-p5b/src-ui


 Test Files  22 passed (22)
      Tests  278 passed (278)
   Start at  21:36:23
   Duration  507ms (transform 63%, import 17%, tests 16%, worker 3%)

  Transform  transforming modules took 2.00s · 63% of tracked time, re-done on every run
             persist transforms across runs with fsModuleCache: true
             learn more: https://vitest.dev/guide/improving-performance#caching-between-reruns
```

本文件的 24 个用例（同一次运行加 `--reporter=verbose`，过滤本文件）：

```
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-1 G2 疑似空转提示 > idleHint：正常运行不提示 2ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-1 G2 疑似空转提示 > idleHint：同一工具连续 3 次且 30 秒无输出 → 疑似空转 0ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-1 G2 疑似空转提示 > idleHint：没有心跳元数据的快照永不判空转，3 分钟后也不判 0ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-1 G2 疑似空转提示 > idleHint：3 分钟无输出 → 疑似空转，心跳停了也按墙上时间继续长 0ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-1 G2 疑似空转提示 > progressSnapshot：后端缺字段按 0 / 空串；running 显示成「运行中」 1ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-1 G2 疑似空转提示 > 运行中一行：显示已运行与工具；胶囊用 hint-capsule，不是红字 7ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-1 G2 疑似空转提示 > runningDetailOf：终态快照冻结耗时，不再跟着墙上时间涨 0ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-1 G2 疑似空转提示 > nextProgress / progressOf：重派后不显示上一轮读数、不立刻出胶囊 0ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-1 G2 疑似空转提示 > 运行行接线：按钮文案是「叫停」，走 cancelDispatchNode(runId, nodeId)；start/retry 清进度 1ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-2 对账说明随派发收敛 > reconcileUiConverged：派发完成的节点从待核对剔除，一个不剩时说明一起清 1ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-2 对账说明随派发收敛 > reconcileUiConverged：按终态过滤——在飞的剔除，暂不可派但仍待核对的 draft 保留 0ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-2 对账说明随派发收敛 > reconcileUiConverged：换图后当空的 0ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-2 对账说明随派发收敛 > GraphTab 接线：每次 onCommit 写回图之后收敛一次 1ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-3 Reply.tokens_cache_read > applyDispatchResults 写 tokens_cache_read，0 与缺失都不写 0ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-3 Reply.tokens_cache_read > applyReconcile 收回 usage.cacheRead 0ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-3 Reply.tokens_cache_read > cacheReadOf：结果屏优先读图字段，图上没有才用本次结果 0ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-4 start() 幂等停止分支整链 > runStageLoop：重复批次 → 本层放回 draft、给提示，且不再推进下一层 1ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-4 start() 幂等停止分支整链 > runStageLoop：批次不重复时照常推进两层后收工 0ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-4 start() 幂等停止分支整链 > runStageLoop：下一层缺目标时回确认屏，不发这一层 0ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-4 start() 幂等停止分支整链 > runStageLoop：达总请求上限时停在本层之前，给上限文案且不再派发 0ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-4 start() 幂等停止分支整链 > runStageLoop：dispatch 抛错时 state 留下本层 activeNodes 与已收结果，供组件 catch 收尾 2ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-5 小质量 > global.d.ts 不再手写 node:fs 声明 1ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-5 小质量 > 常规提示不用红字：单步小任务 / 复核，非独立验证 / 已切备选 都走 dispatch-reason 1ms
 ✓ src/graph/__tests__/idle-and-tail-07b.test.tsx > 07B-5 小质量 > hint-capsule 的样式注释改成「提示胶囊」（校验提示与疑似空转共用） 0ms
```

```
D:\workicross-p5b\src-ui> npm run build

> aicross-ui@0.0.0 build
> tsc -b && vite build

vite v8.0.7 building client environment for production...

transforming...✓ 322 modules transformed.
（中间 100 余行 lang-* / model-picker / react / vendor 分块从略，无 chunk>500kB 警告）
dist/assets/main-BcRPuuGk.js                                   426.53 kB │ gzip: 211.12 kB
dist/assets/xterm-_r9IGi5_.js                                  465.79 kB │ gzip: 123.12 kB
✓ built in 622ms
```

基线对照：R1 之前是 `22 passed (22)` / `269 passed (269)`，现在 `278 passed (278)`——本轮净增 9 个用例
（R1-1 一条、R1-2 两条、R1-4 一条、R1-5 一条、R1-7 四条），另有 3 条既有用例被改写/加断言（重复工具阈值、
`progressSnapshot` 等值、重复批次文案）。

附带核对（非任务书要求）：`npx eslint src/graph/dispatch.ts src/components/center/graph/DispatchOverlay.tsx
src/components/center/graph/DispatchBits.tsx src/graph/__tests__/idle-and-tail-07b.test.tsx` 零问题（exit 0）。
换行逐文件核对：`dispatch.ts` / `DispatchOverlay.tsx` / `GraphTab.css` / 测试文件仍是纯 LF（CR 计数 0），
`tasks/TASK-07-idle-and-tail.md` 仍是全 CRLF（30/30），`tauri.ts` 本轮一个字节没动（仍 506 CR / 601 LF）。
`src/`（Rust）一行未改；没起窗口、没发真机请求、没 commit、没 push；临时文件只写在 `%TEMP%` 下。

## 没做 / 边界（R1 之后）

- 图画布节点卡仍无 elapsed / lastTool / 空转胶囊，已按 R1-6 写回任务书，留下期。
- 后端还没发心跳，`elapsedMs` / `idleMs` / `repeatedToolCalls` 的真实取值仍未实测；A 侧接通后要在真窗口里
  看一次读数是否合理，尤其是新加的 30 秒重复工具门槛。
- 运行屏时钟仍是 5s 一跳，胶囊最迟比阈值晚 5 秒出现。
- `DispatchOverlay` 里唯一调 `Date.now()` 的两处（事件监听、5s 时钟）仍无测试覆盖；判定逻辑已经全部在纯函数里。
