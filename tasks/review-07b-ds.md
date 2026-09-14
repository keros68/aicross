# TASK-07 B 节审查（phase/7b）

## 1. 需求完整性

**1.1 [中] 「节点卡」半边没做** — `DispatchOverlay.tsx:548-561` 只在 G2 运行行显示 `已运行 … · 工具 …`；图画布节点卡仍只有 `nodeStatusLabel` 的「运行中」（`GraphTab.tsx:537,560` ← `dispatch.ts:361`）。
问题：B-1 字面是「结果屏/节点卡的运行中状态显示 elapsed、lastTool」，实现只覆盖前者。
为什么：`progress` state 住在 DispatchOverlay 内部，节点卡拿不到；报告已把它写成边界，但任务书没授权收窄。
建议：要么在本期把口径写回任务书（「G2 运行行」），要么把 `dispatch-progress` 的订阅提到 GraphTab/一个 hook（大改，建议另开一期）。其余 B-1 项（纯函数、阈值、胶囊、叫停、不用红色）齐。

**1.2 契约逐条：无缺项** — `DispatchProgress` 五字段可选 camelCase + `status` 新增 `"running"` 的注释（`tauri.ts:526-542`）；`Reply.tokens_cache_read?`（`types.ts:20-22`）；缺字段按 0/空串（`dispatch.ts:1052-1063`）；叫停走既有 `commands.cancelDispatchNode(runId, node.nodeId)`（`DispatchOverlay.tsx:559`，无新命令）；B-2 `GraphTab.tsx:834-838`；B-3 `dispatch.ts:267/842` + `DispatchOverlay.tsx:678,382`；B-4 `dispatch.ts:1164-1210`；B-5 三处提示改 `dispatch-reason`（`:406/:455/:588`）、`global.d.ts` 清理、`applyReconcile` 注释（`:815-818`）。

## 2. 逻辑正确性

**2.1 [高] 不带心跳字段的 status 事件也会启动 3 分钟倒计时 → 误报** — `dispatch.ts:1070-1072`（`progressRunning` 认「运行中」）+ `:1080-1088`（`idle = idleMs(0) + 墙上时间`）。
问题：一条只有 `status` 的「运行中」事件，`idleMs` 补 0、`receivedAt` 记为收到时刻，180s 后必然返回「疑似空转：3 分钟无输出」，而且这条永远不会被新心跳纠正（引擎不发就不会有新事件）。
为什么：B 节现状（报告自述 A 侧心跳未接通）下，任何跑满 3 分钟的节点都会挂胶囊；即便 A 接通，只要引擎还有一条不带字段的「运行中」事件（或将来某条心跳掉了字段），同样误报。「缺字段按 0 处理」在显示上安全，在**判定**上不安全——测试 `idle-and-tail-07b.test.tsx:107` 只断言同一时刻不提示，正好把这个坑盖住。
建议：把「有心跳元数据」与「只有状态」分开——`ProgressSnapshot` 加 `hasHeartbeat = payload.elapsedMs !== undefined || payload.idleMs !== undefined`（或判定只认 `status === 'running'`），`idleHint` 对无元数据的快照只显示状态、不累计墙上时间；补一条会红的测试（缺字段快照 + 3 分钟后仍为 null）。

**2.2 [中] 新一轮不复位 `progress` / `statuses` / `currentNodes`** — `DispatchOverlay.tsx:88-92`（state）、`:236-247`（`start()` 只清 runError）、`retry()`（`:288-302` 重置了 results/runId/restartNodes，没重置 progress/statuses/currentNodes）。
问题：state 以 nodeId 为键、跨 run 存活，而 `retry()` 复用同一个 nodeId。
为什么：① 重派后到第一条新心跳之间（最长 15s）`runningDetailOf` 显示的是**上一轮**的耗时且继续按墙上时间涨（如「已运行 15 分 3 秒」）；② 若上一轮该节点没收到终态事件（叫停、catch、引擎丢事件），快照 status 还是 `running`，新一行**一出现就报疑似空转**；③ `currentNodes` 也要等 `onStage` 才更新，进 running 的瞬间渲染的是上一批行。
建议：`start()` 与 `retry()` 里 `setProgress({}); setStatuses({}); setCurrentNodes([]);`（或把 runId 存进快照，渲染时过滤 `snapshot.runId !== runId`）。

**2.3 [低] 终态快照的耗时继续走** — `dispatch.ts:1098-1102`。
问题：`runningDetailOf` 只看 `elapsedMs !== 0`，不看 status；同一批里已完成的节点仍挂在 `currentNodes` 上，读数继续增长。
建议：`progressRunning(progress)` 为假时返回空串，或按终态事件冻结 `receivedAt`。胶囊本身在终态会消失 ✓（`:1081`）。

**2.4 [中低] `repeatedToolCalls ≥ 3` 与 idle 无关联 → 常见误报** — `dispatch.ts:1082-1084`。
问题：连续读 3 个文件（同一 `read_file`）在几秒内就会命中，胶囊出现且一直挂到换工具。
为什么：契约写的是「≥3 **或** idleMs ≥ 180000」，实现忠实于契约；但 `docs/PRODUCT.md:211` 的产品意图是「同一工具调用重复 **+** N 分钟无文件变化而 token 在涨」，现在少了「不产出」这一半。
建议：向前端契约方提一个修正——要么重复计数由引擎在「有输出/换工具」时复位（A 侧语义），要么前端要求 `repeatedToolCalls ≥ 3 && idle ≥ 某个较小值`。不改也要在报告里写明这是已知误报面。

**2.5 [低] `reconcileUiConverged` 按「可派」过滤而非按「终态」** — `dispatch.ts:1108-1116`。
问题：`dispatchableNodeIds` 要求 draft + 父节点 done + 有 target。因此在飞（pending）的 id、父节点未完成或无 target 的 id 都会被剔除，工具条「还有 N 个待核对」随派发开始就消失，而节点卡仍显示「待核对」（`dispatch.ts:360`）——两个指示器口径不一致；`needsReview` 同时喂 `initialDispatchSelection`（默认不勾选），剔除后该节点若再回到 draft 就会被默认派出去。
为什么：任务书写的是「剔除已不在 dispatchableNodeIds 的 id」，实现照做；但注释（`:1105-1107`）说成「已经跑完或已判失败」，与代码不符。
建议：过滤条件收紧为「终态（done/failed）或在飞（pending）」，或改为保留 `reply.text === RECONCILE_UNKNOWN_TEXT` 的 id；注释改成与代码一致。

**2.6 runStageLoop 与原 `start()` 等价：未发现偏差** — `dispatch.ts:1164-1210` 与 `DispatchOverlay.tsx:236-281` 逐点对齐：`skipIds` 循环外算一次并每层传入 ✓；`nextStageStep` → missing-target（回确认屏、用 `loop.graph` 取 target）→ limit（文案 + 落 results 屏）→ done ✓；`activeNodes = stageRequest.nodes` 后 `onStage` 再 `commit(markPending)` ✓；重复批次先 `markBatchProcessed` → `commit(resetNodesToDraft(state.graph, ids))` → 返回 stopped，不收进 `collected`、不推进 ✓；`reducePipelineStep` 的 `step.graph !== state.graph` 恒等比较保留 ✓；`commit` 返回落盘图（`persist.ts:10-15`）与原 `working = onCommit(...)` 一致 ✓；catch 分支读的 `loop.graph/collected/activeNodes` 形状与旧实现相同 ✓。唯一新增副作用是 `commit` 现在会顺带收敛对账说明（B-2 要的）。`limit` 分支无测试，见 5.4。

**2.7 `tokens_cache_read` 与小计口径：一致** — `dispatch.ts:1120-1125` 同时用于逐行（`DispatchOverlay.tsx:678`）与小计（`:382`）；`applyDispatchResults` 先写图（`:267`）、`applyReconcile` 写对账那一路（`:842`），所以「图上优先」不会读到别轮的陈旧值（重派走 `redraftNode`，reply 整个被替换）。注：`|| undefined` 让真 0 与缺失不可分，两处都渲染成 `—`，可接受，但报告别写成「0 会写进去」。

**2.8 终态后提示清除：成立**（`dispatch.ts:1070-1071`，终态不在 running 集合），前提是引擎确实发终态事件；若终态事件丢失，胶囊会挂到本层结束（行随 phase 切换消失）。

## 3. 安全

未发现。心跳载荷只进组件 state（`DispatchOverlay.tsx:193-194`），不写图、不下发、不进日志；`lastTool` 以 React 文本节点渲染（转义，无 HTML 注入）；监听按 `payload.runId !== runId` 过滤（`:192`）；胶囊只是提示，从不自动取消。一条给 A 侧的提醒（非本期缺陷）：ACP 的 `title` 可能带命令行/路径，`lastTool` 会在窗口里原样显示，建议引擎侧只放工具名（`tauri.ts:541` 的注释已经这么写了）。

## 4. 代码质量与契约

**4.1 类型只追加：✓** — `tauri.ts:530-542`（新接口）、`dispatch.ts:23-24`（`cacheRead?`）、`types.ts:20-22`（`tokens_cache_read?`）；`Reply`/`BackendNodeResult` 既有字段一个没删，签名没改。

**4.2 `tauri.ts` 命名：✓ 与契约一致**（camelCase、全可选、`status: string` 不收紧成 union，旧值/新值都能过）。

**4.3 第 2–6 期路径：未发现回归** — 逐个读过 `dispatchableNodeIds:87`、`nextStageStep:116`、`pickOne:409`、`sameParentsResultCount:427`、`appendCorrectionRound/appendValidationRetry:520-660`、`applyReconcile:827`、`resetPendingToDraft/resetNodesToDraft:846-866`、`reconcileUiFor:891`、`rollbackBeforeOf/toggleRollbackOnReview:903-926`、`reducePipelineStep:676`、`dispatchBatchKey/markBatchProcessed:1005-1023`：除 `BackendNodeResult` 加字段、`applyDispatchResults` 加一个键、`applyReconcile` 加一个键 + 注释外无改动；`DispatchOverlay` 的擂台/仲裁/采纳/交接路径（`:330-400`）逐行未动。

**4.4 `global.d.ts`：✓ 与 `shiki.test.ts` 同一套写法** — 手写 `declare module 'node:fs'` 已删；`idle-and-tail-07b.test.tsx:7-8` 与 `consistency-06b.test.tsx:7-8` 用同款 `@ts-expect-error node builtin, not in this project's \`types\``；`tsconfig.app.json.include=["src"]` 覆盖测试，`tsc -b` 会校验该指令（多余时报 TS2578），因此这条断言是自检的。既有风险（非本期引入）：将来真把 `@types/node` 加进 `types`，三个文件的 `expect-error` 会一起变成错误。

**4.5 [低] 两处源码扫描断言脆** — `idle-and-tail-07b.test.tsx:145-160`：要求提示文案与 `className` 同行（重排/换行即红），且 `not.toContain("declare module 'node:fs'")` 只挡单引号写法（双引号漏网）。

**4.6 [低] `hint-capsule` 语义** — `GraphTab.css:375` 注释是「校验提示」，现在兼作疑似空转提示（颜色写死 `#d9a13b`，非红色 ✓）；建议顺手把注释改成「提示胶囊」。

## 5. 测试

**5.1 用例真实存在、名字与报告一致 ✓**（15 条逐条对上），断言的是目标行为：三种 idleHint 输入、缺字段补 0、`runStageLoop` 的 outcome/推进序列/draft 复位/collected 不重复、`applyReconcile` 写 cacheRead、`cacheReadOf` 优先级、收敛剔除与换图清空。

**5.2 时钟相关测试不会 flake ✓** — `now`/`receivedAt` 全部显式传参，被测纯函数里没有 `Date.now()`。代价：唯一的 `Date.now()` 调用点（事件监听 + 5s 时钟，`DispatchOverlay.tsx:189-207`）零覆盖。

**5.3 [中] 「运行中一行」名不副实** — `idle-and-tail-07b.test.tsx:112-127` 只 `renderToStaticMarkup(<IdleHint …>)` + 纯函数断言，没有渲染运行行，也没断言按钮文案「叫停」或 `cancelDispatchNode` 的接线。

**5.4 [中] `runStageLoop` 覆盖不全** — 缺 `limit` 分支（限额文案 + 不再推进）与 `dispatch` 抛错路径（抽 `StageLoopState` 的动机就是 catch 要读 `loop.graph/collected/activeNodes`，恰恰没测）；`DUPLICATE_BATCH_TEXT` 用实现同一常量自证，文案改错不会红（missing-target 那条用了字面量，是好的）。

**5.5 [低] B-2 只测了纯函数** — `GraphTab.tsx:834-838` 的 `onCommit` 收敛接线没有断言（删掉包装测试仍全绿）；仓库已有读源文件断言的先例，可低成本补一条。

**最该补的 3 个测试**
1. 跨轮次复位：把 `progress` 更新抽成纯函数（`nextProgress(current, payload, now, runId)`）或用假时间做组件测试，钉「重派后不显示上一轮读数、不立刻出胶囊」——对应 2.2，现在会红。
2. `runStageLoop` 的 `limit` 分支 + `dispatch` 抛错后 `state` 的形状（`activeNodes` = 本层、`collected` = 已收结果）。
3. 缺字段/旧「运行中」快照在 3 分钟后不得提示——对应 2.1，现在会红。

---

## 对照 B 节原始需求是否完整实现

**基本完整：5 条都落了地，但 B-1 的「节点卡显示 elapsed/lastTool」只做了 G2 运行行，且空转判定在心跳字段缺失时会必然误报。**

缺项（按严重度）：
1. **高** — 缺心跳字段的快照仍会累计墙上时间，3 分钟后误报「疑似空转」（`dispatch.ts:1080-1088`）；B 节现状下每个长节点都会命中。
2. **中** — 新一轮（重派）不复位 `progress`/`statuses`/`currentNodes`，显示上一轮读数、可能立刻误报（`DispatchOverlay.tsx:236-302`）。
3. **中** — B-1 的「节点卡」半边未做（`GraphTab.tsx:537,560`；报告已声明为边界）。
4. **中低** — 重复工具计数无 idle 门槛，连续同名工具即误报（`dispatch.ts:1082-1084`，契约使然，建议回报契约方）。
5. **低** — `reconcileUiConverged` 按「可派」而非「终态」过滤，节点卡与工具条口径可能不一致（`dispatch.ts:1108-1116`）。
6. **低** — 覆盖缺口：`limit` 分支、抛错路径、跨轮次复位、运行行/「叫停」接线、GraphTab 收敛接线均无测试。