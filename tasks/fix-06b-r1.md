# TASK-06B 修正轮 R1（界面：对账/回滚收口）

先读 `tasks/TASK-06-consistency.md`（契约 + B 节）、`tasks/REPORT-06B.md`，再读 `tasks/review-06b-glm.md` 与 `tasks/review-06b-ds.md`（若存在）。逐条处理，每条在 `tasks/REPORT-06B.md` 末尾「修正轮 R1」一节写「已修：怎么改的（文件:行）／已补：测试名」或「不采纳：原因」。验证：`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`，命令与输出原文贴进报告。不动 `src/`；保持文件原有换行；不起窗口、不发真机请求、不 commit、不 push。

## 必修（按严重度）

1. **换图重置对账状态**（GLM 1）：`needsReview`/`reconcileNote` 在 `graph?.id` 变化时清空（与 `unfinishedRun` 的重查 effect 同处）；补测试（若组件级不便，抽成纯函数/reducer 断言）。
2. **收回 completed 节点的 session/用量/runner**（GLM 2，契约追加）：`ReconcileNodeSummary` 追加可选 `sessionId`、`runner`、`usage: { input, output, cacheRead }`；`applyReconcile` 把它们写回 `Reply.session_id`/`runner`/用量字段（按现有 Reply 字段名），缺失时不写；删掉 `keep` 里保不到东西的分支。补测试。
3. **`!batch.fresh` 分支**（GLM 3）：删掉不可达的 break；后端对同 run 重复派发的语义是「拒绝并抛错」，走现有 catch 路径。若保留幂等记账，break 前必须把本层 active 节点复位为 draft 并提示，不能悬挂 pending。补测试钉住「不会留 pending」。
4. **`applyReconcile` 前置条件**（GLM 4）：只覆写 `pending`/`draft` 状态的节点，其它状态原样保留；补测试（done 节点不被抹掉）。
5. **`buildRequest` 端到端**（GLM 5-2）：单写者重派请求体含 `rollbackBefore: true`，只读/worktree 无该键，与 `fallback` 并存时形状正确；`continueUnfinished` 组合链 `applyReconcile(resetPendingToDraft(g), summary)` 三种 verdict 一次写回 + summary 为 null 时全部 pending→draft；`applyReconcile` 的 `artifacts: null` 与非法 verdict 兜底。
6. **小质量点**（GLM 4-质量）：`dirtyWorktreeHintOf` 用 `import type { GitChanges }` 而非手写副本；CSS 断言改 `includes`/正则并注明「1568px 人工核对待补」。

## DeepSeek 补充

DeepSeek（`tasks/review-06b-ds.md`）与 GLM 在第 1、3、4、5 条上一致。DeepSeek 额外的（第 7 条最严重）：

7. **待核对节点的「不派发」必须落到实际派发集合**（DS 2.1，最严重）：`start()` 用 `nextStageStep → dispatchableNodeIds` 从图上重算，不看 G1 勾选，unknown 节点照样被派、后续层也会再捡起来。给 `nextStageStep` 加 `skip: ReadonlySet<string>` 在 `dispatchableNodeIds` 之后过滤，`start()` 传「reviewIds 中未勾选的」；过滤后为空按 done/limit 处理；「材料将发送至」按实际集合算。补测试：未勾选的待核对节点不进第一层、后续层也不被捡起。
8. **勾回待核对节点带 `rollbackBefore`**（DS 2.2）：`toggleReviewNode` 勾上时对单写者节点走 `rollbackBeforeOf(target, true)` 并加进 `rollbackNodes`；补测试。
9. **`applyReconcile` 清旧 session**（DS 2.3b，与第 4 条一起）：incomplete/unknown 不保留上一条 reply 的 `session_id`/`runner`（重跑即新会话）；unknown 不可被子节点当作可续父会话；补测试。
10. **unknown 说明可见**（DS 2.4、2.6）：`nodeStatusLabel` 对 `draft && text === RECONCILE_UNKNOWN_TEXT` 返回「待核对」，节点卡渲染该行；G1 的待核对节点即使未勾选也渲染一张只读路由卡（含 `routeLine` 与「待核对」标签）。补测试。
11. **对账正文上限**（DS 3）：`applyReconcile` 对 `replyText` 设上限（如 64KB，截断加省略与提示），防止整份留痕进 Reply 并随上下文外发。补测试。
12. **禁外部动作按「可写」挂载**（DS-06A 1.3）：`buildRequest` 对 `target.write !== '只读'` 的节点追加 `execution.txt` 里那句禁 push/发布/装包/付费接口（与角色模板解耦，避免重复追加）；补测试：自定义角色的可写节点也带这句。
13. **小项**（DS 2.7、3-附件、4-样式）：未提交改动提示只在本批含 worktree 节点时显示；附件 textarea 改受控组件，校验失败时回滚显示或标「未保存」；`.dispatch-review-row` 间距改 8px；常规提示不用 `.dispatch-warning` 红字，改 `dispatch-reason`/胶囊样式；`rollbackNodeChanges` 加注释「界面不调用，由引擎在派发前回滚」。
