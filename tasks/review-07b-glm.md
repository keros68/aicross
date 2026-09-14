# 代码审查：TASK-07 B 节（phase/7b）

说明：本审查环境没有 shell，`git diff` 与 `vitest` 都无法执行；等价性与回归按读代码 + 既有 06b 测试核对，测试「真实存在」按读文件核对（15 条与报告逐条对上）。

## 1. 需求完整性

- **B.1 G2 提示** — 核心链路齐：`tauri.ts:530-544` 类型按契约追加；`dispatch.ts:1052-1062` 缺字段按 0/空串；`idleHint`（`dispatch.ts:1080`）阈值 ≥3 次 / ≥180000ms；胶囊用 `hint-capsule` 不是红字（`DispatchBits.tsx:51-55`）；叫停走现有 `commands.cancelDispatchNode(runId, nodeId)`（`DispatchOverlay.tsx:559`，命令在 `tauri.ts:356`）。三种输入测试齐。
- **缺项（低）**：`DispatchOverlay.tsx:548-563` — elapsed/lastTool 只上了 G2 运行屏的行；任务书原文「结果屏/**节点卡**的运行中状态显示」里的图画布节点卡没有（`dispatch-progress` state 不出弹窗）。报告已声明此边界，按任务书字面算部分实现。
- B.2/B.3/B.4/B.5 逐条对照均已做，见下各维度。

## 2. 逻辑正确性

- **发现 1（中）— 旧引擎「运行中」+ 墙上时间必然误报，报告的兼容声明不成立。** `dispatch.ts:1070-1088`：`progressRunning` 把 legacy 中文「运行中」也算在跑，而 legacy 引擎的「运行中」是一次性迁移事件（无心跳、无 `idleMs`，`progressSnapshot` 补 0）。于是 `idle = 0 + (now - receivedAt)` 只涨不清：旧引擎下任何超过 3 分钟的正常派发都会挂「疑似空转」胶囊并一直涨。REPORT-07B 第 15 行「旧引擎只发 status 时……界面不出任何空转提示」是错的——测试只断言了 `now === receivedAt` 那一刻（`idle-and-tail-07b.test.tsx:107`）。建议：无输出分支加「确有心跳」的门，如 `if (progress.elapsedMs === 0 && progress.repeatedToolCalls === 0) return null`（真心跳必带 `elapsedMs`），或只在字面 `'running'` 上算 idle。
- **发现 2（中）— `progress`/`statuses` 跨轮不清空。** `DispatchOverlay.tsx:91,194`：`setProgress` 只有事件监听一处写，`start()`/`retry()` 都不清。崩溃、叫停、重复批次停止这些路径可能没有终态 progress 事件，旧快照（status 仍是 `running`）留到下一轮：重派同一节点后，新 run 的行会在首个心跳到来前（最长 15s）直接显示上一轮的「疑似空转：N 分钟无输出」和「已运行 X 分」。建议：`start()` 和 `retry()` 里 `setProgress({}); setStatuses({});`。
- **发现 3（低-中）— `reconcileUiConverged` 会永久误删「暂不可派但仍是 draft」的待核对 id。** `dispatch.ts:1108-1116`：按 `dispatchableNodeIds` 成员资格剔除。unknown 节点被 `applyReconcile` 写回 draft（`dispatch.ts:847`），若其父节点在崩溃时也是 pending→draft 或 failed，它暂不在可派集合里——第一次任何 `onCommit`（包括确认屏改个备选下拉触发的 `updateNodeFallback`，`DispatchOverlay.tsx:501`）就把它从 `needsReview` 删掉；之后父节点跑完、它重新可派时，`initialDispatchSelection` 不再默认跳过，未核对的节点会被自动勾上派发。实现与任务书「剔除已不在 dispatchableNodeIds 的 id」字面一致，属规格自身的边界。建议：只剔除已到终态的 id（`graph.nodes[id].reply?.status === 'done' | 'failed'`），draft 的保留。
- **阈值叠加**：无误报问题。引擎 15s 心跳每次带当时 `idleMs`，`idleMs + (now - receivedAt)` 恰等于真实墙上无输出时长，下一条心跳到达即归零重算；5s 时钟只造成阈值后最迟 5s 出胶囊（报告已声明）。`elapsedMs + (now - receivedAt)` 同理不重复计。终态清除：契约保证最后一条是终态事件，`progressRunning` 对终态返回 false，胶囊消失——依赖引擎发终态 progress 事件，与契约一致。
- **runStageLoop 等价性**：结构上与原行为对齐——`nextStageStep` 推进（skip 过滤、missing-target、limit 文案）、`markPending` 先提交再派、批次幂等 `resetNodesToDraft` 后停、`reducePipelineStep` 推进 + `continueRunning`、限额文案同原文案；catch 标 failed 留在组件侧并通过就地可变的 `StageLoopState` 读到 `graph/collected/activeNodes`（`DispatchOverlay.tsx:273-283`），设计合理。保留意见：无法 diff HEAD 的原 `start()`，等价性是结构核对而非逐行核对；`missing-target` 分支组件侧用 `loop.graph.nodes[id].target!` 回填 targets（`:266`），与原实现是否一致未验证。
- **tokens_cache_read 口径**：行（`DispatchOverlay.tsx:678`）与小计（`:382`）都走 `cacheReadOf`，同一口径；`applyDispatchResults` 整体替换 reply，`cacheRead: 0` 不写图字段，回退到 `result.cacheRead ?? 0`，不会串到上一次的值；重派后 reply 变 draft，结果行回退读本行 result，显示仍正确。

## 3. 安全

未发现。心跳字段只有数字计数、状态词和 `lastTool` 标题，渲染走 React 文本节点（`IdleHint`、`dispatch-running-detail`），无 `dangerouslySetInnerHTML`，不含正文/密钥；无新增外发路径（请求体装配未动）。

## 4. 代码质量与契约

- 类型只追加：`types.ts:22`（Reply）、`dispatch.ts:24`（BackendNodeResult）、`tauri.ts:530-544` 均为纯追加，既有字段未动。未发现。
- tauri.ts 命名与契约一致（camelCase 五字段 + `status` 新增 `"running"` 注释）。未发现。
- 第 2–6 期路径无回归：`dispatchableNodeIds`、`reducePipelineStep`、`pickOne`、仲裁、对账、回滚标志均未改；`applyDispatchResults` 仅加 `tokens_cache_read` 一行（`dispatch.ts:267`）。未发现。
- `global.d.ts` 改法与 `shiki.test.ts` 一致（逐 import 上方 `@ts-expect-error`，注释同文）。未发现。
- 小瑕疵（不阻塞）：`idle-and-tail-07b.test.tsx:309-318` 按行匹配源码断 className，对行号漂移和换行敏感——但与 06b 既有做法同款，任务书也未禁止。

## 5. 测试

报告列的 15 条全部真实存在且名称一致（`idle-and-tail-07b.test.tsx`），断言目标行为；时钟相关全部用显式 `now` 参数的纯函数断言，无 `Date.now()` 依赖、无假定时器，不会 flake。未实测运行（无 shell），报告附了输出原文。

最该补的 3 个：
1. **runStageLoop 的 `limit` 分支**：`maxRequests` 触顶 → `stopped` + 上限文案且不再 dispatch——`stopped` 目前只测了重复批次，限额停止整链没测过。
2. **旧快照残留回归**：上一轮留下 `running` 快照后开新一轮，行上不得立刻出空转胶囊/旧耗时（配合发现 2 的修复）。
3. **`reconcileUiConverged` 保住 draft 的待核对 id**：unknown 节点父节点未完成（暂不可派）时不得剔除（配合发现 3 的修复）。

---

## 对照 B 节是否完整实现

**一句话**：5 条全部落地且测试真实，但第 1 条的旧引擎兼容行为与报告声明不符（>3 分钟的正常派发会误报空转），外加一个跨轮状态残留缺陷和一个收敛误删边界。

缺项（按严重度排序）：
1.（中）旧引擎无心跳时「运行中」快照叠加墙上时间必然误报「疑似空转」，且读数只涨不清（`dispatch.ts:1074-1088`；REPORT「界面不出任何空转提示」的说法不成立）。
2.（中）`start()`/`retry()` 不清 `progress`/`statuses`，上一轮快照在下一轮运行行上残留（`DispatchOverlay.tsx:91,194`）。
3.（低-中）`reconcileUiConverged` 永久剔除「暂不可派但仍是 draft」的待核对 id，之后父节点完成时该节点不再默认跳过（`dispatch.ts:1111-1112`；任务书口径如此，属规格边界，建议随修）。
4.（低）图画布节点卡无 elapsed/lastTool（报告已声明边界；按任务书「节点卡」字面算部分实现）。