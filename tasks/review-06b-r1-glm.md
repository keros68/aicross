# TASK-06B 修正轮 R1 复审

13 条逐条核对完毕（读了 fix 清单、契约追加、报告 R1 节，以及全部涉及文件的现行代码与测试）。

## 逐条判定

| # | 判定 | 依据 |
|---|---|---|
| 1 换图重置对账状态 | **已核实修好** | `dispatch.ts:883-885` `reconcileUiFor` 按 graphId 过滤，`GraphTab.tsx:72/256/798/830` 全部吃过滤后的值，工具条与 G1 不再跨图残留；测试 `consistency-06b.test.tsx:302-323` |
| 2 收回 session/用量/runner | **已核实修好** | 类型 `dispatch.ts:758-762`，completed 分支写回 `:826-836`，runner 白名单 `:803-805`，`keep` 已删只留 adopted（`:823`）；`usage.cacheRead` 无 Reply 字段不写回，与任务书「按现有 Reply 字段名」一致且报告已申报 |
| 3 `!batch.fresh` 不留 pending | **已核实修好** | `DispatchOverlay.tsx:264-271` break 前 `resetNodesToDraft(working, ids)` 并提交 + 提示；纯函数 `dispatch.ts:848-858` 有测试钉「本层无 pending」 |
| 4 applyReconcile 前置条件 | **已核实修好** | `dispatch.ts:820-822` 只覆写 pending/draft，done/failed/无 reply 原样保留；测试 `:369-393` 钉 done/failed 不被抹 |
| 5 buildNodeRequest 端到端 | **已核实修好** | 纯函数 `dispatch.ts:970-1003`；测试断序列化后的键：`'rollbackBefore' in JSON.parse(JSON.stringify(writer))`（test `:399`）、fallback 并存形状 `:406-418`、组合链三 verdict 一次写回 + null 全回 draft `:420-437`、artifacts:null/非法 verdict 兜底 `:439-451` |
| 6 小质量点 | **已核实修好** | `GitChangesShape` 用 `Pick<GitChanges,…>`（`dispatch.ts:932-934`），形状漂移会编译报错；CSS 断言改正则 + 注明「1568px 人工核对待补」（test `:215-230`） |
| 7 skip 过滤（最严重） | **已核实修好** | 过滤在 `dispatchableNodeIds` **之后**（`dispatch.ts:120`），过滤后为空走 done（`:121`）；`start()` 进循环前算一次 `skipIds`（`DispatchOverlay.tsx:233`）且每层都传（`:236`），后续层不会捡起；「材料将发送至」按 `request`（= nodeIds，与实际首层集合一致）算（`:150-155/:537`）；测试含「不传 skip 会被捡起来」的对照（test `:454-481`） |
| 8 勾回带 rollbackBefore | **已核实修好** | `toggleRollbackOnReview`（`dispatch.ts:916-926`），`toggleReviewNode` 勾上加、取消勾选撤（`DispatchOverlay.tsx:197-208`），并链到 `buildNodeRequest` 断请求体真带上（test `:483-495`） |
| 9 清旧 session | **已核实修好** | incomplete/unknown 分支重建 reply 只带 adopted，不带 `session_id`/`runner`（`dispatch.ts:837-839`），子节点 `continuationSession` 随之返回空；测试两种 verdict + 续会话断言（test `:497-524`） |
| 10 unknown 说明可见 | **已核实修好** | `nodeStatusLabel` 返回「待核对」（`dispatch.ts:354-356`）→ 节点卡（`GraphTab.tsx:560`）；聊天面板待派行渲染那句说明（`GraphTab.tsx:667-669`）；G1 未勾选也渲染 `ReviewPendingRow`（`DispatchBits.tsx:37-47`，`DispatchOverlay.tsx:414-427`） |
| 11 对账正文上限 | **已核实修好** | `clampReconcileText` 64K 上限 + 截断说明（`dispatch.ts:794-801`），completed 写回前过这道（`:829`）；测试钉「applyReconcile 走同一道」（test `:539-554`）。小注：上限按 JS 字符数不是 UTF-8 字节，超限实际可能到 256KB——够用，不是缺陷 |
| 12 禁外部动作按可写挂载 | **已核实修好** | `NO_EXTERNAL_ACTIONS` + `promptWithWriteGuard`（`dispatch.ts:945-952`），挂在 `buildNodeRequest`（`:990`）不挂角色模板，`includes` 去重；自定义角色可写/worktree 带、只读不带均有断言（test `:556-567`） |
| 13 小项 | **已核实修好** | worktree 才提示（`DispatchOverlay.tsx:169-170/:413`）；textarea 受控 + 「未保存」标记（`SettingsModal.tsx:747-757`、`premise.ts:85-91`）；review-row 8px（`GraphTab.css:39`）；常规提示改 `dispatch-reason`（`DispatchBits.tsx:23-26`）；`rollbackNodeChanges` 注明界面不调用（`tauri.ts:391-396`）；测试 `:569-583` |

## 无回归核对

- `dispatchableNodeIds`、`reducePipelineStep`、`pickOne`、`appendArbitration`、备选重跑（`fallbackSpecOf`/`continuationSession` 的 fallback 分支）逐函数读过，均未改动；`applyDispatchResults` 仅追加 fallback 字段。
- `nextStageStep` 加第 4 参带默认值，`dispatch.test.ts:230-235` 的 3 参旧调用不受影响。
- 类型只追加：`ReconcileNodeSummary` 新增可选项、`DispatchNodeRequest.rollbackBefore`、`Reply.fallback_*` 均未改既有字段。
- `tauri.ts` 命令名与 camelCase 参数（`projectDir`/`runId`/`nodeId`）与契约及既有 `inspectDispatchState` 风格一致；`ReconcileSummary`/`RollbackSummary` type-only 转出，无值依赖环。
- 唯一的行为面新增是可写节点 prompt 末尾多一句禁外部动作——这是 R1-12 本身，254 个测试全绿说明未破坏既有 prompt 断言。

## 结论：R1 之后 TASK-06B 是否可交付？

**可交付**——13 条全部「已核实修好」，无改法引入的新缺陷，两条遗留都以「已申报的边界」而非缺陷存在。

仍未闭合项（按严重度，均不阻断）：

1. **合并对账点**：A 侧把禁外部动作写进 `fixtures/prompts/execution.txt` 时必须与 `NO_EXTERNAL_ACTIONS` 逐字一致，否则可写节点会出现两遍（当前 fixtures 里还没有这句；报告已列）。
2. `usage.cacheRead` 收不回（`Reply` 无对应字段，本期不扩图契约，已申报）——需要时随下一次契约变更补。
3. 重复批次复位分支只有 `resetNodesToDraft` 纯函数测试，`start()` 整链无覆盖，且该路径按 A 侧「拒绝并抛错」语义实际难触达（报告已申报）。
4. 1568px 折行仍是 CSS 字符串断言，真窗口人工核对待补（报告已申报）。