## 逐条核对（13 条）

| # | 判定 | 证据 + 理由 |
|---|---|---|
| 1 换图重置对账状态 | **已核实修好** | `dispatch.ts:875-899` `ReconcileUi{graphId,…}`+`reconcileUiFor`（id 失配返回同一 `EMPTY_RECONCILE_UI`）、`reconcileUiAfter`；`GraphTab.tsx:237/252/256` 写入·清空·渲染前过滤，`:798/:830` 都吃过滤值。做法是「渲染期按 id 过滤」而非 effect 里清 state，功能等价且不残留到别的图（仅内存留着，无害）。 |
| 2 收回 completed 的 session/用量/runner | **已核实修好（有 1 处自认边界）** | 类型 `dispatch.ts:749-755`；写回 `dispatch.ts:826-836`：`session_id`、`runner`（`:803-805` 白名单只在 `acp/oneshot/unknown` 落盘）、`tokens_in/out`；`keep` 已删（grep `\bkeep\b` 零命中）。`usage.cacheRead` 因 `Reply` 无对应字段被丢弃，报告「边界」已写明。 |
| 3 `!batch.fresh` 分支 | **已核实修好（不可达性未消除，任务书允许）** | `DispatchOverlay.tsx:265-272`：重复批次先 `onCommit(resetNodesToDraft(working, ids))`（`:268`）再 `setRunError('…已放回待派')` 后 break；纯函数 `dispatch.ts:848-858` 只动传进来的 id、无 pending 返回同引用。后端「拒绝并抛错」仍走既有 catch（`:288` 起）。测试只钉纯函数，`start()` 整链未测（报告已认）。 |
| 4 `applyReconcile` 前置条件 | **已核实修好** | `dispatch.ts:814-822`：`if (status !== 'pending' && status !== 'draft') continue;`，`done/failed` 不被旧留痕抹掉；测试 `consistency-06b.test.tsx:370`。注：`reply` 为 undefined 的节点同样被跳过（报告表述为「原样保留」，实际流程不可达）。 |
| 5 `buildRequest` 端到端 | **已核实修好** | `buildNodeRequest` 提到纯函数（`dispatch.ts:970-1003`，`NodeRequestParts:957-968`），`DispatchOverlay.tsx:131-146` 调用；测试 `:399` 断的是 `JSON.parse(JSON.stringify(req))` 里的键（负例断对象无此键，随之序列化也不会出现）；`:406` fallback+rollbackBefore 并存；`:420` 组合链三 verdict / null 全 pending→draft；`:439` `artifacts:null` 不写键 + 非法 verdict 按 unknown。 |
| 6 小质量点 | **已核实修好** | `dispatch.ts:932-940` `GitChangesShape = Pick<GitChanges,'state'> & Partial<Pick<Extract<GitChanges,{state:'ok'}>,'uncommitted'|'untracked'>>`（`GitChanges` 来自 `:8` type-only import，改形状会编译报错）；CSS 断言改正则块匹配 + 注释「1568px 人工核对待补」`test:216-233`。无新用例（沿用 06B-4）。 |
| 7 待核对不派发落到实际集合 | **已核实修好** | `dispatch.ts:114-124`：`skip` 为第 4 参，`const nodeIds = dispatchableNodeIds(graph).filter((id) => !skip.has(id))`（`:120`）在 missing-target/limit **之前**，空集→`done`；`DispatchOverlay.tsx:233` 算一次 `skipIds`、`:236` **循环内每层都传**；后续层重算时该节点仍是 draft 但被过滤（测试 `:455-479` 含「不传 skip 就会被捡起来」对照）。「材料将发送至」(`:537`) 的 `vendorNames` 取自 `request.nodes`（`:150` 由 `nodeIds` 组装），与实际首层外发集合一致。 |
| 8 勾回带 rollbackBefore | **已核实修好** | `dispatch.ts:916-923` `toggleRollbackOnReview`；`DispatchOverlay.tsx:197-208` `toggleReviewNode` 勾上时 `setRollbackNodes(...)`（单写者进、只读/worktree 不进、取消撤掉），请求经 `buildNodeRequest`（`dispatch.ts:1001`）带键；测试 `:484-494` 末尾接请求体断言。 |
| 9 清旧 session | **已核实修好** | `dispatch.ts:837-839` incomplete/unknown 只保留 `adopted`，不带上一条 `session_id/runner`；`continuationSession`(`:129-140`) 随之返回空（测试 `:498-523` 含子节点续聊对照）。 |
| 10 unknown 说明可见 | **已核实修好** | `dispatch.ts:356` `nodeStatusLabel` 对 `draft && text===RECONCILE_UNKNOWN_TEXT` → 「待核对」（节点卡 `GraphTab.tsx:560`、聊天面板 `:668` 渲染整句）；`DispatchBits.tsx:35-48` `ReviewPendingRow`（checkbox+routeLine+胶囊），未勾选也渲染于 `DispatchOverlay.tsx:419-425`。是「行」不是「卡」，信息面等价。 |
| 11 正文上限 | **已核实修好** | `dispatch.ts:796-801` `RECONCILE_TEXT_LIMIT=64KB` + `clampReconcileText`（截断+说明行），completed 分支 `:829` 写回前过滤；测试 `:540-553`（含经 `applyReconcile` 的端到端断言）。 |
| 12 禁外部动作按可写挂 | **已核实修好** | `dispatch.ts:947-951` `NO_EXTERNAL_ACTIONS`/`promptWithWriteGuard`（`write==='只读'` 直接返回，`prompt.includes(...)` 去重），挂在 `dispatch.ts:990` 的 `buildNodeRequest` 上——`DispatchOverlay` 是唯一组装 `DispatchNodeRequest` 的地方（grep 确认），自定义角色同样带；测试 `:557-565`。 |
| 13 小项 | **已核实修好** | ①`dirtyWorktreeHintOf(changes, hasWorktreeNode)`（`dispatch.ts:936-940`）+ `DispatchOverlay.tsx:169-170/413` 用 `dispatch-reason`；②附件受控：`premise.ts:82-92` `attachmentDraftState` + `SettingsModal.tsx:134/277-293/747-757`（保存成功清草稿、失败留框并标「未保存，抽查仍按已存附件发出」，`SettingsModal.css:547` 中性灰）；③`.dispatch-review-row` `margin-top: 8px`（`GraphTab.css:39`，测试正则钉住）；④`RollbackBeforeHint` 改 `dispatch-reason`（`DispatchBits.tsx:22-26`）；⑤`tauri.ts:393-394` 注释「界面不调用，引擎在派发前回滚」。 |

## 特别核对项结论

- **第 7 条**：skip 确实在 `dispatchableNodeIds` 之后过滤（`dispatch.ts:120`）、`start()` 进循环前算一次且在循环体内每层传（`DispatchOverlay.tsx:233/236`），后续层不会再捡起未勾选节点（测试对照项证实）；「材料将发送至」按 `request.nodes`＝实际首层集合算。
- **第 8 条**：勾回的单写者进 `rollbackNodes`，请求体带 `rollbackBefore: true`（`DispatchOverlay.tsx:207` → `dispatch.ts:1001`）。
- **第 3 条**：停止前 `resetNodesToDraft` 已提交并提示，本层不留 pending。
- **第 2/4/9 条**：`applyReconcile` 只覆写 pending/draft；completed 写回 session/runner/tokens；incomplete/unknown 清 session/runner（不写旧值）。
- **第 12 条**：挂在可写判断上，`includes` 去重，无重复追加。
- **第 5 条**：正例断序列化后的键（`JSON.parse(JSON.stringify(...))`），负例断对象无键（等价于序列化无键）。
- **回归**：`dispatchableNodeIds`/`reducePipelineStep`/`pickOne`/`sameParentsResultCount`/仲裁/`appendValidationRetry` 均未被 R1 触碰；`nextStageStep` 只是加默认参（`dispatch.test.ts:230-235` 三参调用照旧）；`applyDispatchResults` 仅 fallback 字段（R0 改动，`presets-05b.test.ts:174-195` 已同步、`presets-05b-r2.test.tsx:83-92` 旧标记续聊仍成立）。类型只追加（`types.ts:29-30`、`tauri.ts:469`），`tauri.ts` 命令名/参数与契约 camelCase 一致（`:389-396`）。**未发现 R1 引入的新缺陷。**

## R1 之后 TASK-06B 是否可交付

**可以说达到了 B 侧可交付：13 条全部落地且最严重的 7/8/2/3/4/9 在代码层已核实，类型只追加、构建与 254 条测试经编排者复跑全绿；只余少量文案/契约对账项，无阻断。**

仍未闭合项（按严重度）：

1. **合并前必核**：`fixtures/prompts/execution.txt`（A 侧）那句禁外部动作必须与 `dispatch.ts:947` 逐字一致，否则 执行 角色可写节点 prompt 会出现两遍。
2. **合并前必核**：A 侧 `reconcile_dispatch_state` 是否真给 `sessionId/runner/usage`（前端已按缺失兜底，缺失即「续聊断、无 token 数」）；`replyText` 段口径（「## 原始输出」之前）与前端 64KB 截断叠加后不再二次切分。
3. 同图内对账说明不随派发收敛：工具条仍显示「已按留痕对账 N 个节点，M 个待核对」（DS 2.5 后半条未做；标签因 `initialIds` 过滤不会误标已完成节点，仅文案陈旧）。
4. `usage.cacheRead` 未写回图（`Reply` 无该字段），结果屏 cache_read 只有当次派发有值——需在契约对账里确认可接受。
5. 幂等停止分支仍是不可达路径，只测了纯函数 `resetNodesToDraft`，`start()` 整链（复位提交顺序 + 提示文案）无测试。
6. G1 四控件 1568px 下不溢出仍只有正则断言，未起窗口人工核对。
7. 小质量：`global.d.ts` 手写 `declare module 'node:fs'`（仓库内 `shiki.test.ts` 用的是 `@ts-expect-error` 另一套写法，将来引入 `@types/node` 会遮蔽真实类型）；`.dispatch-warning` 仍用于「单步小任务」「复核，非独立验证」「已切备选」等常规提示（本轮只改了被点名的两条）；`applyReconcile` 对无 `reply` 节点按非 draft 跳过，与报告「原样保留」的注释口径宜补一句；报告 R0 段行号已随 R1 插入漂移约 15 行。