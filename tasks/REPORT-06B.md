# REPORT-06B 重派与崩溃恢复的一致性 · B. 界面（TS）

工作树 `D:\work\aicross-p5b`，分支 `phase/6b`。只动 `src-ui/` 与 `tasks/`，`src/`（Rust）一行未改。
后端两条新命令由 A 侧另一工作树实现；本期只按契约写好 TS 签名与调用，未接通时走回退分支。

## 契约

- `src-ui/src/tauri.ts:95-96` 从 `graph/dispatch.ts` type-only 引入并转出 `ReconcileSummary` / `RollbackSummary`
  （类型本体与吃它的纯函数同住 `graph/dispatch.ts`，免得 `tauri.ts` ↔ `graph/dispatch.ts` 成值依赖环）。
- `src-ui/src/tauri.ts:389-394` `reconcileDispatchState(projectDir)` → `invoke<ReconcileSummary>('reconcile_dispatch_state')`；
  `rollbackNodeChanges(projectDir, runId, nodeId)` → `invoke<RollbackSummary>('rollback_node_changes')`。
- `src-ui/src/tauri.ts:465-467` `DispatchNodeRequest` 追加可选 `rollbackBefore?: boolean`。
- `src-ui/src/graph/dispatch.ts:736-762` 契约类型 `ReconcileVerdict` / `ReconcileNodeSummary` / `ReconcileSummary`
  （camelCase：`runId`、`nodeId`、`recordPath`、`replyText`）与 `RollbackSummary`。
- `src-ui/src/graph/types.ts:26-30` `Reply` 追加 `fallback_from?` / `fallback_to?`。

`rollbackNodeChanges` 本期只定签名，界面没有调用点——回滚由引擎在派发前按 `rollbackBefore` 自己做
（契约 B-2 的口径），前端不单独发回滚请求。

## 逐条

### 1. 崩溃后「继续」走对账（backlog 13）

**已做**：
- `src-ui/src/graph/dispatch.ts:764-822` 四个纯函数：`RECONCILE_UNKNOWN_TEXT`（:764）、
  `resetPendingToDraft(graph)`（:768，回退分支，pending → draft）、
  `applyReconcile(graph, summary)`（:781，completed → `status:'done'` + `replyText` + `artifacts`；
  incomplete → draft；unknown → draft + 「上次派发结果未知，请检查项目目录后重派或放弃」）、
  `reconcileNeedsReview(summary)`（:812）。`exists:false` / `null` 原样返回同一对象引用。
- `src-ui/src/graph/dispatch.ts:818-821` `initialDispatchSelection(ids, needsReview)`：待核对节点从默认勾选剔除。
- `src-ui/src/components/center/graph/GraphTab.tsx:224-250` `continueUnfinished` 先
  `await commands.reconcileDispatchState(...)`，`try/catch` 吞掉 reject（后端未接通），再
  `commit(applyReconcile(resetPendingToDraft(graph), summary))`。`summary` 为 null 或 `exists:false` 时
  `applyReconcile` 不动图，等价于原来的「pending 全部重置为 draft」。
- `GraphTab.tsx:71-72` 新增 `needsReview` / `reconcileNote` state，工具条 `GraphTab.tsx:801` 显示一行对账说明；
  「放弃」时一并清空（`GraphTab.tsx:268-269`）。`GraphTab.tsx:833` 把 `needsReview` 传给 `DispatchOverlay`。
- `DispatchOverlay.tsx:65` 初始 `nodeIds` 走 `initialDispatchSelection`（待核对默认不派）；
  `:207-215` 勾选开关与顺序；`:410-424` G1 顶部「待核对」区（逐节点 checkbox，勾上才加回本次派发）；
  `:446` 路由行尾加「待核对」标签。
- `DispatchBits.tsx:29-33` `ReviewPendingTag`（组件级可断言）。

**已补测试**（`src-ui/src/graph/__tests__/consistency-06b.test.tsx`）：
- `06B-1 崩溃后「继续」走对账 > applyReconcile：completed 收回正文与产物`
- `06B-1 崩溃后「继续」走对账 > applyReconcile：incomplete 回 draft 重跑`
- `06B-1 崩溃后「继续」走对账 > applyReconcile：unknown 回 draft 并写「未知」说明，reconcileNeedsReview 列出它`
- `06B-1 崩溃后「继续」走对账 > applyReconcile：exists:false 原样返回（后端未接通时的回退分支）`
- `06B-1 崩溃后「继续」走对账 > resetPendingToDraft：对账前先把 pending 放回 draft，done 的不动`
- `06B-1 崩溃后「继续」走对账 > G1：待核对节点默认不勾选，标签渲染出「待核对」`

### 2. 单写者重派回滚（backlog 12）

**已做**：
- `src-ui/src/graph/dispatch.ts:826-835` `rollbackBeforeOf(target, retrying)`：`retrying` 且 `write` 既非
  「只读」也非 `worktree` 时 `true`，否则 `undefined`；`rollbackHintOf`（:832）给 G1 文案「将先回滚上次改动」。
- `DispatchOverlay.tsx:160` `buildRequest` 按 `rollbackBeforeOf(target, rollbackNodes.has(id))`
  决定带不带 `rollbackBefore: true`；只读 / worktree 不带这个字段。
- `DispatchOverlay.tsx:95-97` `rollbackNodes` 初值来自新 prop `retryNodes`（节点卡按过「重派」的节点，
  `GraphTab.tsx:74`、`:572`、`:834`）；结果屏「重派」「升级」走的 `retry()`（`DispatchOverlay.tsx:302`）
  也把这一路记进 `rollbackNodes`。
- `DispatchOverlay.tsx:448` 路由卡显示 `RollbackBeforeHint`（`DispatchBits.tsx:22-27`）。
- 未提交改动提示：仓库已有只读的 `gitChanges`，所以**没有**新增 `git_status_dirty`。
  `src-ui/src/graph/dispatch.ts:838-846` `dirtyWorktreeHintOf(changes)` 只在 `state === 'ok'` 且有改动时出
  「项目目录有 N 处未提交改动；worktree 从 HEAD 新建，看不到这些改动」；`DispatchOverlay.tsx:172-181`
  在确认屏拉一次，命令失败或不是仓库时为 `null`，界面不显示（`:409`）。

**已补测试**：
- `06B-2 单写者重派回滚 > buildRequest 的 rollbackBefore：单写者重派带，只读/worktree 不带`
- `06B-2 单写者重派回滚 > G1 提示：单写者重派显示「将先回滚上次改动」`
- `06B-2 单写者重派回滚 > G1 未提交改动提示：git 状态读不出来时不显示`

不确定处：`buildRequest` 是 `DispatchOverlay` 组件内的闭包（吃 `runId` / `selection` / `settings` 等 render 值），
没有整体外提；测试断的是它唯一的判定依据 `rollbackBeforeOf`，请求组装本身仍无测试。

### 3. Reply.fallback 字段（backlog 17）

**已做**：
- `src-ui/src/graph/types.ts:26-30` `Reply` 追加 `fallback_from?` / `fallback_to?`。
- `src-ui/src/graph/dispatch.ts:249-252` `applyDispatchResults` 把 `fallbackFrom/To` 写进字段，正文**不再**
  前置「（已切备选：…）」标记行（标记不会再随 `serializeContext` 进下游 prompt、随图落盘）。
- `src-ui/src/graph/dispatch.ts:198-208` `fallbackNoticeOf` 优先读字段拼整行，读不到再退回识别旧图的正文首行标记。
- 节点卡（`GraphTab.tsx:667`）与结果屏（`DispatchOverlay.tsx:578`）显示整行的写法没变。
- 同步改了既有断言：`src-ui/src/graph/__tests__/presets-05b.test.ts:173-193`
  （改名为「备选重跑记账：reply 记 fallback 字段，fallbackNoticeOf 能拼出整行」）。

**已补测试**：
- `06B-3 Reply.fallback 字段 > applyDispatchResults 写 fallback_from/to，正文不再前置标记行`
- `06B-3 Reply.fallback 字段 > fallbackNoticeOf 兼容旧图：只有正文首行标记时仍识别`

### 4. G1 备选下拉溢出（backlog 16）

**已做**：`src-ui/src/components/center/graph/GraphTab.css:34-39` `.dispatch-controls` 加
`flex-wrap: wrap` + `row-gap: 8px`，label 加 `min-width: 0`，input/select 加 `max-width: 240px`，
新增 `.dispatch-review-row`。四个控件（目标 / 档位 / thinking / 备选）一行放不下时折到第二行，不再溢出卡片右边。

**已补测试**：`06B-4 G1 备选下拉溢出 > .dispatch-controls 可折行，控件有宽度上限`（字符串断言）。

说明：vitest 默认把 CSS 导入（含 `?raw`）桩成空串，所以这条测试直接 `readFileSync` 读 CSS 文件；
`tsconfig.app.json` 的 `types` 只有 `vite/client`，为它在 `src-ui/src/global.d.ts:10-16` 加了一个只含
`readFileSync` 的最小 `node:fs` 声明（不引 `@types/node` 全局，免得盖掉 DOM 的定时器签名）。

### 5. 前提附件输入（backlog 19）

**已做**：
- `src-ui/src/graph/premise.ts:76-80` `parseAttachmentLines(raw)`：按行拆、去首尾空白、丢空行，不再按逗号拆。
- `src-ui/src/graph/premise.ts:95-100`（R1 在它前面插了 `attachmentDraftState`，原 :82-87）`premiseCheckBlockReason(premise)`：「抽查」按下时再跑一次
  `validatePremiseAttachments`，不通过返回提示文案。
- `src-ui/src/components/common/SettingsModal.tsx:271-275` 手输改走 `parseAttachmentLines`；
  `:737-744` 输入框改成 `textarea`（每行一条，占位文案同步改），去掉原来的 Enter 提交（Enter 现在是换行），
  保留 blur 保存。`:310-315` `runPremiseCheck` 先跑 `premiseCheckBlockReason`，不通过就不建图、只提示。
- `src-ui/src/components/common/SettingsModal.css:546-547` 多行输入框样式（`min-height` + `resize: vertical`）。

**已补测试**：
- `06B-5 前提附件输入 > 每行一条：含逗号的路径不被拆坏，空行丢掉`
- `06B-5 前提附件输入 > 「抽查」前再校验一次：不通过就不建图并给提示`

### 6. 盲父节点不传 session/runner（backlog 18）

**已做**：`src-ui/src/graph/dispatch.ts:127-130` `continuationSession` 在父节点 `target.visibility === '盲'`
时返回 `undefined`（盲分支跑在隔离目录，续聊会把它的上下文漏到共享子节点）。`continuationRunner` 依赖
`continuationSession`，随之一并返回空。

**已补测试**：`06B-6 盲父节点不传 session/runner > 父节点 visibility=盲 时 continuationSession 返回空`
（同一张图把父节点可见性换成「共享」仍能续 `sess-1`，作对照）。

### 7. 推进幂等（backlog 10，前端半边）

**已做**：
- `src-ui/src/graph/dispatch.ts:848-863` `dispatchBatchKey(runId, stage, results)`（节点集合排序后拼 key，
  与送达顺序无关）+ `markBatchProcessed(seen, key)`（返回新的 seen 与 `fresh`）。
- `DispatchOverlay.tsx:235`、`:261-267` `start()` 每层拿到结果先查批次 id；重复批次直接 `break`，
  不收进 `collected`、不推进下一层。

**已补测试**：`06B-7 推进幂等 > 同一批结果送达两次只推进一次`
（含「节点顺序不影响批次 id」「换阶段 / 换 run 照常推进」两个对照）。

## 验证

两条命令都实际跑过，输出原文（去掉终端色码）：

```
D:\work\aicross-p5b\src-ui> npx vitest run --configLoader native --pool threads

 RUN  v5.0.0 D:/work/aicross-p5b/src-ui


 Test Files  21 passed (21)
      Tests  237 passed (237)
   Start at  19:37:50
   Duration  513ms (transform 62%, import 17%, tests 17%, worker 3%)
```

新增用例（同一次运行加 `--reporter=verbose`，过滤本文件）：

```
 ✓ src/graph/__tests__/consistency-06b.test.tsx > 06B-1 崩溃后「继续」走对账 > applyReconcile：completed 收回正文与产物 5ms
 ✓ src/graph/__tests__/consistency-06b.test.tsx > 06B-1 崩溃后「继续」走对账 > applyReconcile：incomplete 回 draft 重跑 1ms
 ✓ src/graph/__tests__/consistency-06b.test.tsx > 06B-1 崩溃后「继续」走对账 > applyReconcile：unknown 回 draft 并写「未知」说明，reconcileNeedsReview 列出它 1ms
 ✓ src/graph/__tests__/consistency-06b.test.tsx > 06B-1 崩溃后「继续」走对账 > applyReconcile：exists:false 原样返回（后端未接通时的回退分支） 0ms
 ✓ src/graph/__tests__/consistency-06b.test.tsx > 06B-1 崩溃后「继续」走对账 > resetPendingToDraft：对账前先把 pending 放回 draft，done 的不动 1ms
 ✓ src/graph/__tests__/consistency-06b.test.tsx > 06B-1 崩溃后「继续」走对账 > G1：待核对节点默认不勾选，标签渲染出「待核对」 7ms
 ✓ src/graph/__tests__/consistency-06b.test.tsx > 06B-2 单写者重派回滚 > buildRequest 的 rollbackBefore：单写者重派带，只读/worktree 不带 0ms
 ✓ src/graph/__tests__/consistency-06b.test.tsx > 06B-2 单写者重派回滚 > G1 提示：单写者重派显示「将先回滚上次改动」 1ms
 ✓ src/graph/__tests__/consistency-06b.test.tsx > 06B-2 单写者重派回滚 > G1 未提交改动提示：git 状态读不出来时不显示 0ms
 ✓ src/graph/__tests__/consistency-06b.test.tsx > 06B-3 Reply.fallback 字段 > applyDispatchResults 写 fallback_from/to，正文不再前置标记行 0ms
 ✓ src/graph/__tests__/consistency-06b.test.tsx > 06B-3 Reply.fallback 字段 > fallbackNoticeOf 兼容旧图：只有正文首行标记时仍识别 0ms
 ✓ src/graph/__tests__/consistency-06b.test.tsx > 06B-4 G1 备选下拉溢出 > .dispatch-controls 可折行，控件有宽度上限 0ms
 ✓ src/graph/__tests__/consistency-06b.test.tsx > 06B-5 前提附件输入 > 每行一条：含逗号的路径不被拆坏，空行丢掉 0ms
 ✓ src/graph/__tests__/consistency-06b.test.tsx > 06B-5 前提附件输入 > 「抽查」前再校验一次：不通过就不建图并给提示 0ms
 ✓ src/graph/__tests__/consistency-06b.test.tsx > 06B-6 盲父节点不传 session/runner > 父节点 visibility=盲 时 continuationSession 返回空 1ms
 ✓ src/graph/__tests__/consistency-06b.test.tsx > 06B-7 推进幂等 > 同一批结果送达两次只推进一次 1ms
```

```
D:\work\aicross-p5b\src-ui> npm run build

> aicross-ui@0.0.0 build
> tsc -b && vite build

vite v8.0.7 building client environment for production...
transforming...✓ 322 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                                                  3.46 kB │ gzip:   1.32 kB
dist/assets/onig-Du5pRr7Y.wasm                                 466.61 kB │ gzip: 159.69 kB
...
dist/assets/main-JxxfR8Xn.js                                   426.11 kB │ gzip: 211.13 kB
dist/assets/xterm-_r9IGi5_.js                                  465.79 kB │ gzip: 123.12 kB

✓ built in 782ms
```

基线对照：改动前同样两条命令是 `20 passed (20)` / `221 passed (221)`，本期 +1 个测试文件、+16 个用例。
这次 build 没出 chunk>500kB 警告（最大 465.79 kB）。

附带核对（非任务书要求）：`npx eslint` 对本期改过的两个大文件（`SettingsModal.tsx`、`GraphTab.tsx`）的
问题数与 `HEAD` 版本一致（24 + 2 = 26），没有新增 lint 问题；`graph/dispatch.ts`、`graph/premise.ts`、
`DispatchOverlay.tsx`、`DispatchBits.tsx`、`tauri.ts`、`consistency-06b.test.tsx` 零问题。
换行风格逐文件核对过：只有 `tauri.ts` 是 CRLF/LF 混排（既有状态），新增行按所在区段的原风格插入，
其余文件仍是纯 LF；没跑格式化器。

## 没做 / 边界

- `src/`（Rust）一行未动；A 侧那 6 条（before 快照与 `rollback_node_changes`、`reconcile_dispatch_state`、
  禁外部动作提示词与 `--tools` 白名单、`dispatch_stage_core` 幂等、8MB 标记边界、Rust `Reply` 镜像）不在本期。
- 没起窗口、没发任何真机模型请求、没 commit、没 push；临时文件只用过 `%TEMP%`。
- `git_status_dirty(project_dir)` 没新增：仓库已有只读的 `gitChanges`，按任务书「调用现有 git 状态命令若有」走了它。
- `rollbackNodeChanges` 只有类型与签名，没有界面调用点。
- 后端未接通时的实际表现只由回退分支的测试与类型保证，没有在真窗口里点过「继续」。

## 合并时要对账的点

- `reconcile_dispatch_state` 的 `replyText` 取「## 原始输出」之前的回复正文段（A 侧口径）；前端整段写进
  `Reply.text`，不再二次裁剪。
- `artifacts` 期望是已脱敏的 `ReplyArtifacts`（内部字段 snake_case：`diff` / `changed_files` / `stdout_summary`）。
- `nodes[].verdict` 只认 `completed` / `incomplete` / `unknown` 三个字面量；`applyReconcile` 的三元兜底把
  其它值按 unknown 处理，A 侧别扩枚举。
- Rust `Reply` 镜像加 `fallback_from` / `fallback_to` 后，`run_fallback` 不要再往正文前塞标记行——
  前端已改成读字段，旧标记只作旧图兼容。

## 修正轮 R1

按 `tasks/fix-06b-r1.md` 13 条逐条处理（第 7、8、2、3、4 条优先）。只动 `src-ui/` 与 `tasks/`，`src/`（Rust）仍一行未改。
对账结果新增的 `sessionId` / `runner` / `usage` 按契约先写进 TS 类型，后端没给时按缺失处理。

### 1. 换图重置对账状态（GLM 1）

**已修**：`needsReview` / `reconcileNote` 两个组件级 state 合成一份带图 id 的 `ReconcileUi`。
- `src-ui/src/graph/dispatch.ts:875-899` 新增 `ReconcileUi` / `EMPTY_RECONCILE_UI`（:881）/ `reconcileUiFor(state, graphId)`（:883，id 对不上返回同一个空常量）/ `reconcileUiAfter(graphId, summary)`（:887，顺带把工具条那句说明从组件里搬出来）。
- `GraphTab.tsx:72` 换成单个 `reconcile` state；`:237` 对账后写入（带 `graph.id`），`:252`「放弃」清空，`:256` 渲染前过 `reconcileUiFor(reconcile, graphId)`，`:798` 工具条与 `:830` `needsReview` 都吃过滤后的值。换图后 id 失配即为空，不再残留上一张图的说明与「待核对」标签。

**已补测试**：
- `06B-R1-1 换图重置对账状态 > reconcileUiFor：图 id 变了就当空的，同一张图原样保留`
- `06B-R1-1 换图重置对账状态 > reconcileUiAfter：对不上账时给回退说明，不列待核对`

### 2. 收回 completed 节点的 session/用量/runner（GLM 2，契约追加）

**已修**：
- `dispatch.ts:758-762` `ReconcileNodeSummary` 追加可选 `sessionId` / `runner` / `usage: { input, output, cacheRead }`。
- `dispatch.ts:826-838` completed 分支把它们写回 `Reply.session_id` / `runner` / `tokens_in` / `tokens_out`，缺失不写；`runner` 经 `dispatch.ts:803-805` 过一道白名单（`acp` / `oneshot` / `unknown`），别的值当没有。
- 原来保不到东西的 `keep` 已删（`dispatch.ts:823`），只留 `adopted`。

**边界**：`usage.cacheRead` 没有对应的 `Reply` 字段（现有 Reply 只有 `tokens_in` / `tokens_out` / `ms`），按任务书「按现有 Reply 字段名」不新增图契约字段，这一项收不回来——结果屏的 cache_read 列只在当次派发的结果里有值。

**已补测试**：`06B-R1-2 收回 completed 节点的 session/用量/runner > applyReconcile：completed 写回 session_id / runner / tokens，缺失与非法值不写`

### 3. `!batch.fresh` 分支（GLM 3）

**已修**：保留幂等记账，但停止推进前先复位。`DispatchOverlay.tsx:264-271`：重复批次时 `resetNodesToDraft(working, ids)` 把本层刚 `markPending` 的节点放回 draft 并提交，再 `setRunError('同一批结果重复送达，已停止推进；这一层的节点已放回待派。')`，然后 break——不再留悬挂的 pending。纯函数 `resetNodesToDraft` 在 `dispatch.ts:846-858`（只动传进来的那几个 id，没有 pending 时返回同一引用）。
后端对同 run 重复派发抛错的那条路径仍走既有 catch（`DispatchOverlay.tsx:288` 起），未改。

**已补测试**：`06B-R1-3 重复批次不留 pending > resetNodesToDraft：停止推进前把本层放回 draft，done 的不动`

### 4. `applyReconcile` 前置条件（GLM 4）

**已修**：`dispatch.ts:820-822` 循环里先看节点当前状态，只覆写 `pending` / `draft`；`done`、`failed`、无 reply 的节点原样保留。崩溃后已被重派成功的节点不会被一条更旧的留痕抹掉。

**已补测试**：`06B-R1-4 applyReconcile 前置条件 > 只覆写 pending / draft；done 与 failed 的节点不被旧留痕抹掉`

### 5. `buildRequest` 端到端（GLM 5-2）

**已修**：把 `DispatchOverlay.buildRequest` 的单节点装配提成纯函数 `buildNodeRequest(nodeId, target, parts)`（`dispatch.ts:970-1003`，`NodeRequestParts` 在 :957-968）；上下文（prompt / materials / 会话 / stage）由调用方算好传入，函数本身不碰 IO。`DispatchOverlay.tsx:131-146` 改成调用它，行为不变。`DispatchNodeRequest` / `GitChanges` 走 `dispatch.ts:8` 的 type-only import（运行时擦除，不构成与 `tauri.ts` 的值依赖环）。

**已补测试**：
- `06B-R1-5 buildRequest 与组合链 > buildNodeRequest：单写者重派带 rollbackBefore，只读 / worktree 无该键`（断的是序列化后的键名与存在性）
- `06B-R1-5 buildRequest 与组合链 > buildNodeRequest：fallback 与 rollbackBefore 并存时形状正确`
- `06B-R1-5 buildRequest 与组合链 > continueUnfinished 组合链：三种 verdict 一次写回；summary 为 null 时全部 pending→draft`
- `06B-R1-5 buildRequest 与组合链 > applyReconcile 兜底：artifacts 为 null 不写该键，非法 verdict 按 unknown`

### 6. 小质量点（GLM 4-质量）

**已修**：
- `dispatch.ts:930-940` `dirtyWorktreeHintOf` 的参数类型改成从 `GitChanges` 推的 `GitChangesShape`（`Pick<GitChanges, 'state'> & Partial<Pick<Extract<GitChanges, { state: 'ok' }>, 'uncommitted' | 'untracked'>>`），不再手写结构副本；`GitChanges` 改形状时这里会编译报错。
- `consistency-06b.test.tsx:196-211` CSS 断言改成按块匹配的正则（`/\.dispatch-controls\s*\{[^}]*\}/`），格式化或选择器换行不再假失败；注释里写明「1568px 下不溢出是视觉事实，人工核对待补（本期不起窗口）」。

### 7. 待核对节点的「不派发」落到实际派发集合（DS 2.1，最严重）

**已修**：
- `dispatch.ts:111-121` `nextStageStep` 追加第 4 个参数 `skip: ReadonlySet<string>`（默认空集），在 `dispatchableNodeIds` 之后过滤；过滤后为空走既有的 `done` 分支，超限仍走 `limit`。
- `DispatchOverlay.tsx:231-236` `start()` 进循环前算一次 `skipIds =`「reviewIds 里没被勾选的」，每一层都用它——第一层不派，后续层重算时也不再捡起来。
- 「材料将发送至」（`DispatchOverlay.tsx:537`）按 `request` 算，而 `request` 来自 G1 的 `nodeIds`，现在与实际外发集合一致。

**已补测试**：`06B-R1-7 待核对节点不进实际派发集合 > nextStageStep：未勾选的待核对节点不进第一层，后续层也不被捡起来`（含「不传 skip 时它会被捡起来」的对照）

### 8. 勾回待核对节点带 `rollbackBefore`（DS 2.2）

**已修**：`dispatch.ts:914-924` 新增 `toggleRollbackOnReview(current, target, nodeId, checked)`；`DispatchOverlay.tsx:197-210` `toggleReviewNode` 勾上时对单写者节点把 id 加进 `rollbackNodes`，取消勾选时撤掉，请求体随之带 `rollbackBefore: true`。

**已补测试**：`06B-R1-8 勾回待核对节点带 rollbackBefore > toggleRollbackOnReview：勾上时单写者进集合，只读不进，取消勾选撤掉`（末尾接 `buildNodeRequest` 断请求体确实带上了）

### 9. `applyReconcile` 清旧 session（DS 2.3b）

**已修**：`dispatch.ts:837-839` incomplete / unknown 分支只保留 `adopted`，不再带上一条 reply 的 `session_id` / `runner`。unknown 节点的 session 被清掉后，`continuationSession`（`dispatch.ts:127`）对它的子节点返回空，不会续到结果未知的会话上。

**已补测试**：`06B-R1-9 对账清旧 session > incomplete / unknown 不保留上一条的 session_id / runner，unknown 不能被子节点续会话`

### 10. unknown 说明可见（DS 2.4、2.6）

**已修**：
- `dispatch.ts:354-356` `nodeStatusLabel` 对 `draft && text === RECONCILE_UNKNOWN_TEXT` 返回「待核对」——图上的节点卡（`GraphTab.tsx:560`）直接显示这个状态词。
- `GraphTab.tsx:667-669` 聊天面板的待派行对这种节点渲染那句说明，不再只写「待派」。
- `DispatchBits.tsx:35-48` 新增 `ReviewPendingRow`（checkbox + `routeLine` + 「待核对」胶囊）；`DispatchOverlay.tsx:418-425` 待核对区改用它，未勾选的节点也有路由行与标签。

**已补测试**：`06B-R1-10 unknown 说明可见 > nodeStatusLabel 对待核对节点返回「待核对」，G1 未勾选也渲染路由行与标签`

### 11. 对账正文上限（DS 3）

**已修**：`dispatch.ts:794-800` `RECONCILE_TEXT_LIMIT = 64 * 1024` + `clampReconcileText`，超限截断并在末尾加一行「正文超过 N 字符已截断，完整内容见留痕文件」；`applyReconcile` 的 completed 分支（`dispatch.ts:829`）写回前过这一道。整份留痕（可能含裸 stdout）不会再原样进 `Reply.text` 随上下文外发。

**已补测试**：`06B-R1-11 对账正文上限 > clampReconcileText：超限截断并说明，未超限原样`

### 12. 禁外部动作按「可写」挂（DS-06A 1.3）

**已修**：`dispatch.ts:945-951` 新增 `NO_EXTERNAL_ACTIONS` 常量与 `promptWithWriteGuard(prompt, write)`：`write !== '只读'` 时在 prompt 末尾追加那句禁 push / 发布 / 装依赖 / 付费接口；prompt 里已有同一句（A 侧会把它写进 `fixtures/prompts/execution.txt`）就不重复追加。挂在 `buildNodeRequest`（`dispatch.ts:990`）上而不是角色模板上，自定义角色的可写节点同样带。

**已补测试**：`06B-R1-12 禁外部动作按「可写」挂 > 自定义角色的可写节点也带这句，只读不带，模板里已有不重复`

### 13. 小项（DS 2.7、3-附件、4-样式）

**已修**：
- 未提交改动提示只在本批含 worktree 节点时显示：`dirtyWorktreeHintOf` 追加 `hasWorktreeNode` 参数（`dispatch.ts:936-938`）；`DispatchOverlay.tsx:161-170` 改成把 `gitChanges` 原始状态存进 state、渲染期按本批节点算文案。
- 附件输入框改受控：`premise.ts:82-92` `attachmentDraftState(raw, saved)`；`SettingsModal.tsx:134` 加 `attachDrafts` 草稿 state，`:747-757` textarea 走 `value` + `onChange`，保存成功清草稿，校验失败草稿留在框里并在下面显示「未保存，抽查仍按已存附件发出」（`SettingsModal.css:547` 中性灰字）。`pickPremiseAttachments` 保存成功也清草稿。
- `.dispatch-review-row` 间距 6px → 8px（`GraphTab.css:39`）。
- 常规提示不再用红字：`RollbackBeforeHint` 改 `dispatch-reason`（`DispatchBits.tsx:22-26`），未提交改动提示同（`DispatchOverlay.tsx:413`）。
- `rollbackNodeChanges` 加注释「界面不调用这条命令：回滚由引擎在派发该节点前按请求里的 rollbackBefore 自己做」（`tauri.ts:393-394`）。

**已补测试**：
- `06B-R1-13 小项 > 未提交改动提示只在本批含 worktree 节点时显示`
- `06B-R1-13 小项 > 附件输入框：草稿与已存不一致时标「未保存」`

### R1 验证

两条命令都实际跑过，输出原文（去掉终端色码）：

```
D:\work\aicross-p5b\src-ui> npx vitest run --configLoader native --pool threads

 RUN  v5.0.0 D:/work/aicross-p5b/src-ui


 Test Files  21 passed (21)
      Tests  254 passed (254)
   Start at  20:37:32
   Duration  514ms (transform 58%, import 20%, tests 18%, worker 4%)
```

```
D:\work\aicross-p5b\src-ui> npm run build

> aicross-ui@0.0.0 build
> tsc -b && vite build

vite v8.0.7 building client environment for production...
transforming...✓ 322 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                                                  3.46 kB │ gzip:   1.32 kB
dist/assets/xterm-sLQPUA_C.css                                   4.00 kB │ gzip:   1.02 kB
dist/assets/main-6sdxR3S3.css                                  126.38 kB │ gzip:  22.22 kB
dist/assets/main-DzhuC49-.js                                   426.23 kB │ gzip: 211.13 kB
dist/assets/xterm-_r9IGi5_.js                                  465.79 kB │ gzip: 123.12 kB
✓ built in 702ms
```

R1 新增 17 个用例（`consistency-06b.test.tsx` 由 16 条到 33 条），全量由 237 → 254；测试文件数不变。
`npx eslint` 对改过的文件：`dispatch.ts`、`premise.ts`、`DispatchOverlay.tsx`、`DispatchBits.tsx`、`tauri.ts`、`consistency-06b.test.tsx` 零问题；`GraphTab.tsx` + `SettingsModal.tsx` 仍是 26 个既有问题（与 R0 同数），没有新增。
换行：编辑过的文件都保持原风格（`tauri.ts` 是 CRLF/LF 混排的既有状态，逐行核对过没被整文件改写；其余纯 LF），没跑格式化器。

### R1 没做 / 边界

- 没起窗口、没发任何真机模型请求、没 commit、没 push；临时文件只用过 `%TEMP%` 下的会话目录。
- `usage.cacheRead` 没写回图（现有 `Reply` 没有这个字段，本期不扩图契约）——见第 2 条。
- 1568px 下 G1 四控件不溢出仍只有字符串/正则断言，没在真窗口里量过。
- 重复批次的复位分支（第 3 条）按后端「拒绝并抛错」的语义本就难触达，测试钉的是纯函数 `resetNodesToDraft` 的结果，不是 `start()` 的整链。

### R1 合并时要对账的点（追加）

- `ReconcileNodeSummary` 的 `sessionId` / `runner` / `usage` 是 R1 追加的契约字段：`runner` 前端只认 `acp` / `oneshot` / `unknown`，`usage` 缺失按 0 处理（不写回），`cacheRead` 前端目前丢弃。
- `replyText` 前端会按 64KB 截断（超限时在末尾追加一行说明）；A 侧仍按「## 原始输出」之前的正文段给，别指望前端做二次切分。
- `DispatchNodeRequest.prompt` 现在对 `write != 只读` 的节点在末尾多一句禁外部动作。A 侧把同一句写进 `fixtures/prompts/execution.txt` 后，前端按字符串包含去重，不会出现两遍——两处的句子必须逐字一致：
  `不得 git push、发布、安装依赖或调用付费接口；这些留给用户在验收之后做。`
