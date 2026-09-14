# TASK-06 B 节代码审查

读完 PRODUCT.md 相关节、TASK-06 契约+B 节、AGENTS.md、REPORT-06B，以及本期全部改动文件。

---

## 1. 需求完整性

逐条核对（文件:行 = 证据位置）：

| B 节条目 | 结论 |
|---|---|
| B-1 崩溃后「继续」走对账 | **部分**：`GraphTab.tsx:225-250`、`dispatch.ts:764-822` 在；但"默认不勾选派发"没落到实际派发集合（见 2.1） |
| B-2 单写者重派回滚 | **部分**：`dispatch.ts:826-846`、`DispatchOverlay.tsx:95-97/160/448`、`DispatchBits.tsx:22-27` 在；契约点名的 `buildRequest` 测试没测 `buildRequest`（见 5.2）；待核对勾回场景不带 `rollbackBefore`（见 2.2） |
| B-3 Reply.fallback 字段 | 在：`types.ts:26-30`、`dispatch.ts:201-207/249-252` |
| B-4 G1 备选下拉折行 | 在：`GraphTab.css:36-38` |
| B-5 前提附件逐行输入 | 在：`premise.ts:76-87`、`SettingsModal.tsx:271-285/309-315/737-744` |
| B-6 盲父节点不传 session/runner | 在：`dispatch.ts:127-129`（runner 随 `continuationSession` 走 `:141`） |
| B-7 推进幂等 | 在：`dispatch.ts:848-863`、`DispatchOverlay.tsx:259-268` |

**后端未接通的回退分支：存在且可编译。** `GraphTab.tsx:231-236` try/catch → `summary = null`；`dispatch.ts:782` `!summary?.exists` 原样返回同一引用；`tauri.ts:389-394` 命令签名与类型齐备，`ReconcileSummary`/`RollbackSummary` 由 `tauri.ts:95-96` type-only 转出，不构成 `tauri.ts` ↔ `graph/dispatch.ts` 的值依赖环。行为等价于原「pending→draft」。

---

## 2. 逻辑正确性

### 2.1（严重）待核对节点的"不派发"只存在于 state，没进请求
`DispatchOverlay.tsx:238`（`nextStageStep`）+ `:251`（`const ids = next.nodeIds`），对照 `:64/207/446` 与 `GraphTab.tsx:236-243`。

`start()` 的派发集合由 `nextStageStep → dispatchableNodeIds()` 从图上重算，全程不读 G1 的 `nodeIds`，也从不读 `reviewIds`。unknown 节点在图上就是 `{status:'draft', text:RECONCILE_UNKNOWN_TEXT}`（`dispatch.ts:806-809`），与 incomplete 的 draft 无法区分——区分二者的唯一信号是 `needsReview` state，而 `start()` 没看它。用户取消勾选后按「确认派发」：该节点照样进 `buildRequest`、`markPending`、`dispatchGraphStage`，而且循环里每一层都会再算一次。

为什么严重：这正是 PRODUCT.md 已定 ②「未知的交 G3，**不自动重跑单写者节点**」要禁的行为，单写者会在半截改动上再叠一遍；同时 G1 顶部「材料将发送至…（`DispatchOverlay.tsx:532-536`）」按 `request`（= `nodeIds`）算，与实际外发集合不一致。

改法：把排除集做成一等输入。最小改动是在循环里过滤 —— `const excluded = new Set([...reviewIds].filter((id) => !nodeIds.includes(id)))`，`const ids = next.nodeIds.filter((id) => !excluded.has(id))`，过滤后为空时按 `done`/`limit` 处理；更干净是给 `nextStageStep`（`dispatch.ts:108`）加 `skip: ReadonlySet<string>`，在 `dispatchableNodeIds` 之后过滤——这样纯函数可测（见 5.4）。

### 2.2（中）勾回待核对节点时不带 `rollbackBefore`
`DispatchOverlay.tsx:95-97`、`:160`、`dispatch.ts:826-833`。

`rollbackNodes` 只来自 `retryNodes`（节点卡重派，`GraphTab.tsx:572`）与结果屏 `retry()`（`:302`）。用户把 unknown 的**单写者**节点勾回来重派时，走的是 `toggleReviewNode`（`:209-215`），不带 `rollbackBefore` —— 上次可能已经改过文件，正是 backlog 12 要修的叠加场景。

改法：`toggleReviewNode` 勾上时对该 id 走 `rollbackBeforeOf(target, true)` 并加进 `rollbackNodes`；或让 unknown verdict 在 `applyReconcile` 里就带上"需先回滚"的语义标记。

### 2.3（中）`applyReconcile` 无状态守卫，且保留旧 session/runner
`dispatch.ts:781-810`（覆盖写在 `:799-809`；`keep` 在 `:786-790`）。

(a) 不检查节点当前状态：某节点在崩溃后被成功重派过（`done`），一条更旧的留痕会把它盖回 draft/unknown，刚拿到的结果被抹掉（长驻/多图共用同一 projectDir 时现实存在）。
(b) incomplete/unknown 把上一条 reply 的 `session_id`/`runner` 原样保留，子节点的 `continuationSession`（`:123-140`）会续上一个半途或结果未知的会话，与 B-6 的隔离口径相反。

改法：`if (node.reply?.status === 'done' && row.verdict !== 'completed') continue;`；incomplete 明确清 `session_id`/`runner`（重跑即新开会话）；unknown 至少不让它被当"可续父会话"。

### 2.4（低）unknown 的说明文字没有任何界面出口
`dispatch.ts:764`、`:806-809`；`GraphTab.tsx:669-673`；`dispatch.ts:339-347`。

`RECONCILE_UNKNOWN_TEXT` 写进 `reply.text`，但 draft 节点在聊天面板只渲染「待派」/「运行中」（`:672`），`nodeStatusLabel` 对 draft 也返回「待派」，节点卡（`GraphTab.tsx:559-568`）不带任何字。用户唯一看得到的是 G1 分区标题和工具条 `reconcileNote`（`:801`）——契约那句"请检查项目目录后重派或放弃"实际不可见。

改法：`nodeStatusLabel` 对 `status==='draft' && text===RECONCILE_UNKNOWN_TEXT` 返回「待核对」，或在节点卡上渲染这一行。

### 2.5（低）`needsReview` / `reconcileNote` 生命周期不清
`GraphTab.tsx:836`（只清 `retryNodes`）对 `:267-268`（只在「放弃」清）。

标签与说明跨多次派发长期残留；下次 G1 里一个早已处理完的节点仍被标「待核对」并默认不勾选，用户会当成新问题。改法：派发成功后剔除已不在 `dispatchableNodeIds` 里的 id，`reconcileNote` 一并清。

### 2.6（低）路由行的「待核对」标签只在勾选后才出现
`DispatchOverlay.tsx:410-424`、`:446`。路由卡列表就是 `nodeIds.map`，未勾选的待核对节点没有卡，`ReviewPendingTag` 也就没地方渲染；契约写的是"对该节点显示「待核对」标签"。改法：给 review 节点也渲染一张只读路由卡（或至少在分区里显示 `routeLine`）。

### 2.7（低）未提交改动提示与 worktree 无关照显示
`DispatchOverlay.tsx:409` + `dispatch.ts:838-846`：文案专讲 worktree，但一批全是只读节点时也显示。改为仅当本批含 `write==='worktree'` 时提示，或把文案改通用。

### 2.8 正面确认（简）
盲父 → `dispatch.ts:127-129` 返回 undefined，runner 随之；`fallbackNoticeOf`（`:201-207`）字段优先、旧首行标记兜底，兼容旧图；`applyDispatchResults`（`:236-266`）不再改正文；幂等守卫位置对（收到结果立刻判，`:259-268`，`!fresh` 直接 break 不进 `collected`）。但要说清：这条守卫挡的是"引擎把同一批结果回吐"的假想路径——本流水线里同一 `(run, stage, node 集合)` 不会二次到达（上一层节点已 done，修正/验证重试都新建 id），真正的重复回调若走事件也不会经过它（`dispatch-progress` 事件不带结果）；硬约束在 A 侧 `dispatch_stage_core` 的拒绝。

---

## 3. 安全

- **对账写回不做二次脱敏，也没有新增外发路径。** `Reply.text`/`artifacts` 会经 `context.ts:61-67`（`replyMessage` + `artifactSummary`，含整段 diff）进下游 prompt、随图落盘；外发前的脱敏由引擎按 `DispatchSettings.redact`（`tauri.ts:413`）在每次派发时统一做，前端本来就不承担脱敏。因此这是"与普通结果同一条路"，不是绕过。残留风险全在 A 侧口径：需在合并时用一条真实留痕验证「留痕正文段与 diff 已脱敏」成立，并确认 `redact=false` 时前端无兜底（既有行为）。
- **`applyReconcile` 对 `replyText` 不设上限**（`dispatch.ts:799`）：A 侧若拿不到「## 原始输出」标记，整份留痕（可能含裸 stdout）会进 `Reply.text` 并随上下文外发。建议前端加长度上限/截断。
- **前提附件校验在抽查前生效 ✓**：`SettingsModal.tsx:311-315` → `premise.ts:82-87`，不通过只 `setPremiseNote` 不建图；手输路径也先过 `validatePremiseAttachments`（`:276-280`）。
- （轻量）附件 textarea 是 `defaultValue` + `key={attachments.join('\n')}`（`SettingsModal.tsx:737-744`），校验失败被拒时 key 不变，界面继续显示那份**没被保存**的路径，而「抽查」用的是已存列表 —— 界面显示与真实外发清单不一致。改成受控组件回滚，或输入框旁标「未保存」。
- 未发现：新命令与 `gitChanges` 都是只读；无密钥回显、无日志落盘。

---

## 4. 代码质量与契约

- **类型只追加 ✓**：`Reply.fallback_from/to`（`types.ts:26-30`）、`DispatchNodeRequest.rollbackBefore`（`tauri.ts:465-467`）。**命名 ✓**：`ReconcileSummary`/`ReconcileNodeSummary`/`RollbackSummary` 与契约 camelCase 一致（`dispatch.ts:739-762`），命令名与参数名（`projectDir`/`runId`/`nodeId`）与既有 `cancelDispatchNode` 同风格（`tauri.ts:389-394`）。
- transport 类型住在纯函数模块再被 `tauri.ts` 转出：无环、可接受，但 `DispatchOverlay.tsx:4-37` 为了一个类型经 `dispatch.ts` 进口二十多个值导出。非必需项。
- `dirtyWorktreeHintOf`（`dispatch.ts:838-841`）用宽松结构类型而非 `Pick<GitChanges, 'state'|'uncommitted'|'untracked'>`：`GitChanges` 改形状时它不会编译报错，只是静默不显示。建议改成 type-only import 的 `Pick`。
- **无回归 ✓**（逐函数读过，均未改动）：`dispatchableNodeIds:82`、`nextStageStep:108`、`reducePipelineStep:662`、`pickOne:395`、`sameParentsResultCount:415`、`neutralDisputeSummary:456`、`appendArbitration:465`、`appendCorrectionRound:540`、`appendValidationRetry:576`；`applyDispatchResults` 只在 `:249-252` 追加字段，`presets-05b-r2.test.tsx:83-92` 的旧标记续聊分支仍成立。
- 样式纪律：`GraphTab.css:38` `.dispatch-review-row { margin-top: 6px }` 破坏"间距取 4 的倍数"；`.dispatch-warning`（红字，`GraphTab.css:47`）被复用于两条常规提示（`DispatchBits.tsx:25`、`DispatchOverlay.tsx:409`），按视觉纪律红色只该留给异常——建议用 `dispatch-reason` 或 `hint-capsule`。
- `rollbackNodeChanges`（`tauri.ts:393-394`）无调用点，注释没说明"界面不调用、由引擎在派发前自行回滚"。留签名符合契约，补一句注释即可。

---

## 5. 测试

- **真实存在 ✓**：`consistency-06b.test.tsx` 共 16 个 `it`，与 REPORT-06B 的 verbose 列表逐条对应；报告引用的 `dispatch.ts` 行号（764/768/781/812/818/826/832/838/848/857）与文件一致。
- **名字与断言不符 ✗**：`consistency-06b.test.tsx:146-153` 名为「buildRequest 的 rollbackBefore」，实际只断言谓词 `rollbackBeforeOf`；`DispatchOverlay.tsx:160` 的请求装配（"只读/worktree 不带该字段"这一字节级事实）零覆盖。报告自认了，但这正是契约 B-2 点名的那条测试。
- **断言强度不足**：`initialDispatchSelection`（`:137-141`）与 `ReviewPendingTag`（`:141-142`）各自都对，却没有任何测试能发现 2.1——因为没人把 `nodeIds` 与 `start()` 连起来断言。
- `exists:false`（`:122-126`）只测纯函数早退；`GraphTab.tsx:231-236` 的 catch 回退路径无测试（需 mock `commands`，或把 `continueUnfinished` 的决策抽成纯函数）。
- **最该补的 3 个**：
  1. **派发集合尊重 G1 勾选**：给 `nextStageStep` 加 `skip` 后测"未勾选的待核对节点不进第一层、也不在后续层被捡起来"（挡 2.1，本期最贵的 bug）。
  2. **rollbackBefore 的装配**：把 `buildRequest` 的节点装配抽成纯函数并测三种 write；加一条"待核对勾回的单写者应带 `rollbackBefore`"钉住 2.2。
  3. **`applyReconcile` 的边界**：节点已是 done/failed 时不被旧留痕覆盖、incomplete 清 session/runner、`summary.nodes` 含图中不存在的 nodeId 不炸 —— 现有 3 条只覆盖了 pending 起步的理想路径。

---

## 对照 B 节原始需求是否完整实现？

**不算完整**：7 条的代码都在，但 B-1 的"待核对默认不勾选派发"没有落到实际派发集合上（勾掉仍会被派），B-2 点名的 `buildRequest` 测试名不副实。

缺项（按严重度）：

1. unknown 节点在 `start()` 里仍会被自动重跑——单写者违反"不自动重跑"，G1 授权集合 ≠ 实际外发集合（`DispatchOverlay.tsx:238/251`）。
2. 勾回待核对节点时缺 `rollbackBefore`，单写者在半截改动上叠加（`DispatchOverlay.tsx:95-97/160`）。
3. `applyReconcile` 无状态守卫、incomplete/unknown 保留旧 session（`dispatch.ts:781-810`）。
4. unknown 说明文字无界面出口（`dispatch.ts:764` → `GraphTab.tsx:669-673`）。
5. `buildRequest` 的 `rollbackBefore` 无测试（`consistency-06b.test.tsx:146-153`）。
6. `needsReview`/`reconcileNote` 生命周期不清（`GraphTab.tsx:836`）。
7. 附件 textarea 校验失败后界面与已存状态不一致（`SettingsModal.tsx:737-744`）。
8. 6px 间距、红字当常规提示（`GraphTab.css:38`、`DispatchBits.tsx:25`、`DispatchOverlay.tsx:409`）。