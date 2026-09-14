审查完毕。说明：本会话没有 shell 工具，未重跑 vitest/构建，`237 passed` 以报告原文为准；我逐条读了 16 个新用例与全部改动代码，确认测试真实存在且断言目标行为。

## 1. 需求完整性

对照 B 节 7 条与契约，逐条核对均落地：

| 条目 | 实现位置 | 判定 |
|---|---|---|
| 1 对账 | `tauri.ts:389-390`、`dispatch.ts:781-815`、`GraphTab.tsx:224-247`、待核对区 `DispatchOverlay.tsx:410-424` | ✓ |
| 2 回滚 | `dispatch.ts:826-844`、`DispatchOverlay.tsx:95-97,160,302,448`、`tauri.ts:467` | ✓ |
| 3 fallback 字段 | `types.ts:29-30`、`dispatch.ts:201-207,249-252` | ✓ |
| 4 下拉折行 | `GraphTab.css:35-39` | ✓ |
| 5 附件逐行 | `premise.ts:78-87`、`SettingsModal.tsx:271-285,309-315,737-744` | ✓ |
| 6 盲父 | `dispatch.ts:127-130,141-146` | ✓ |
| 7 幂等 | `dispatch.ts:848-863`、`DispatchOverlay.tsx:262-267` | ✓ |

回退分支存在：`reconcileDispatchState` reject → `GraphTab.tsx:231-235` 置 null → `applyReconcile`（`dispatch.ts:782`）原样返回同引用 → 等价旧的「pending 重置为 draft」；`npm run build` 报告通过，`rollbackBefore?`/`fallback_from?` 均可选，不依赖后端即可编译。

发现：

- **`GraphTab.tsx:71-72` vs `GraphTab.tsx:154-161,307-317`** — `needsReview`/`reconcileNote` 在同 session 内 `openGraph` 换图后不重置，而 `unfinishedRun` 有按 `graph?.id` 重查的 effect。为什么：这两个 state 是组件级的，语义却属于某张图；换图后工具条（`GraphTab.tsx:801`）仍显示上一张图的「已按留痕对账 N 个节点」，旧 id 虽被 `initialIds` 过滤掉、不至于误标，但文案残留误导。建议：在 `graph?.id` 变化的 effect 里 `setNeedsReview([])`、`setReconcileNote('')`。
- **契约缺口：收回的 completed 节点没有 session_id/用量** — `dispatch.ts:789-793` 的 `keep` 保留 `adopted/session_id/runner`，但「继续」时节点 reply 是 pending，`markPending`（`dispatch.ts:187-196`）展开的旧 reply 不含 session，`keep` 实际永远保不到东西；契约 `ReconcileNodeSummary` 也不携带这两样。为什么：结果是崩溃前那次实际跑完的节点，session 在留痕里有，收回后下游子节点续不了它的会话、结果卡无 token 数——「收回产物」收得不完整。属 A/B 契约层面，报告「合并时对账」清单没列这条。建议：契约给 `ReconcileNodeSummary` 追加可选 `session_id`/tokens（A 侧留痕有据可取），或删掉 `keep` 里保不到的分支免得误导后续维护。

## 2. 逻辑正确性

核对项全部正确：三种 verdict 写回（`dispatch.ts:794-804`，非三值字面量兜底按 unknown ✓）；待核对默认不勾选（`dispatch.ts:818-821` + `DispatchOverlay.tsx:65`）且勾回后按 `initialIds` 保序（`:208-213`）；`rollbackBeforeOf` 只认非只读非 worktree 的重派（`dispatch.ts:826-829`），请求侧 `DispatchOverlay.tsx:160`、节点卡与结果屏两条重派入口（`GraphTab.tsx:572`、`DispatchOverlay.tsx:302`）都覆盖；`fallbackNoticeOf` 字段优先、旧图首行标记兼容（`dispatch.ts:201-207`）；盲父 session/runner 均空（`dispatch.ts:130`，runner 经 `:143` 依赖随之空）；重复批次 `break`（`DispatchOverlay.tsx:262-267`）。

发现：

- **`DispatchOverlay.tsx:267` `if (!batch.fresh) break;` — 该分支不可达，且触发时行为有害**。为什么：一次 `start()` 内每个 stage 只 invoke 一次，批次 key 每轮新增一个，相邻两层节点集合不相交，key 不可能重复；它能防的唯一情形是后端对重复调用「返回同一批成功结果」，而此时本层节点已在 `:259-260` 被 `markPending` 并 commit，break 后既不写回结果也不复位，节点永久挂 pending。A 侧契约实际是「第二次拒绝（抛错）」，走 catch 会把未收到的节点标 failed，不经过这里。建议：要么删掉（后端语义已由 A 侧幂等覆盖），要么 break 前把本层 active 节点复位为 draft 并提示，别留悬挂状态。
- **`dispatch.ts:785-807` `applyReconcile` 不看节点当前状态一律覆写**。为什么：现在唯一调用点靠「continueUnfinished 时 pending 节点动不了」这个隐含前提才安全（pending 不可派也不可重派）；作为导出的纯函数，未来任何别的调用点传入含 done/failed 节点的 summary 都会把现成的 `done` 回复整条抹掉（incomplete→draft 是丢数据）。建议：循环内加一行 `if (node.reply && node.reply.status !== 'pending' && node.reply.status !== 'draft') continue;`，把前提钉进函数本身。

## 3. 安全

未发现。写回 as-is：`dispatch.ts:794-804` 直接写 `replyText`/`artifacts`，无二次脱敏也无任何 invoke 外发；completed 的 artifacts 落图盘依赖 A 侧写留痕时已脱敏的契约前提，与报告「合并时对账」第二条一致，非本期缺陷。附件校验在抽查前生效：`SettingsModal.tsx:311-315` 先于 `:319` 建图与 `:323` saveGraph。

## 4. 代码质量与契约

类型只追加（`types.ts:29-30`、`tauri.ts:467`，`DispatchNodeResult` 未动）；命令与参数 camelCase（`tauri.ts:389-394`，与既有 `inspect_dispatch_state` 同款转换）；第 2/3/4/5 期路径无回归——`dispatchableNodeIds`/`reducePipelineStep`/`pickOne`/仲裁/擂台逻辑未动，`applyDispatchResults` 仅加 fallback spread（`dispatch.ts:249-252`），受影响的既有断言已同步（`presets-05b.test.ts:174-195`），r1/r2 用标记文本直测 `continuationSession` 不受影响。

发现：

- **`dispatch.ts:838-844` `dirtyWorktreeHintOf` 手写结构类型** `{ state?: string; uncommitted?: unknown[]; ... }` 而非复用 `GitChanges`。为什么：`GitChanges`（`tauri.ts:63-79`）是字面量联合，演化时这份副本不会编译报错（如 `state` 换字面量值，`!== 'ok'` 静默成立）。建议 `import type { GitChanges } from '../tauri'`——type-only 引用不构成报告担心的值依赖环。
- **`consistency-06b.test.tsx:202,204` CSS 断言按「单行以 `.dispatch-controls {` 开头」匹配**。为什么：CSS 一旦格式化或选择器换行就假失败；且该断言只证明字符串存在，不证明 1568px 下不溢出（任务书允许字符串断言，报告也如实承认没起窗口）。建议改用 `includes`/正则，注释里记下「1568px 人工核对待补」。

## 5. 测试

报告 16 条与 `consistency-06b.test.tsx` 一一对应，断言的是目标行为而非实现细节（06B-2 的 `buildRequest` 缺口报告已如实申报）。

最该补的 3 个：

1. **`continueUnfinished` 的组合行为**：`applyReconcile(resetPendingToDraft(g), summary)` 混合三种 verdict 一次写回 + summary=null 时全部 pending→draft 且 `reconcileNeedsReview` 为空——B-1 主链路目前只有单元件，回退整链没钉。
2. **`buildRequest` 端到端**：单写者重派的请求体含 `rollbackBefore: true`（钉住 camelCase 键名，serialize 才是契约面）、只读/worktree 节点请求体无该字段；顺带钉 `fallback` 与 `rollbackBefore` 并存时的形状。
3. **`applyReconcile` 边界**：completed 且 `artifacts: null`（只写正文、不写 artifacts 键）+ verdict 传非法字面量按 unknown 兜底——两个契约边角现无测试。

---

**对照 B 节原始需求是否完整实现？** 已完整实现，7 条全部落地且回退分支可编译，无阻断缺项。

缺项（按严重度排序）：
1. `needsReview`/`reconcileNote` 换图不重置（UI 状态跨图泄漏）；
2. 收回的 completed 节点无 session_id/用量——契约不携带，下游续聊断、无 token 数，需合并时与 A 侧对账定夺；
3. `!batch.fresh` 分支不可达且触发时节点悬挂 pending；
4. `applyReconcile` 不设节点状态前置条件，安全性靠调用点隐含前提；
5. `buildRequest` 组装无端到端测试；`dirtyWorktreeHintOf` 手写类型副本与 CSS 断言脆弱两个小质量点。