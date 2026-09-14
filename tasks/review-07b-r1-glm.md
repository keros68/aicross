复审完成。7 条逐项核对（均已对照源码与测试，非仅报告自述）：

## 逐条结论

**1. 无心跳元数据的快照不判空转（DS 2.1 高）— 已核实修好**
`dispatch.ts:1071` 用 `elapsedMs !== undefined || idleMs !== undefined` 算 `hasHeartbeat`，`dispatch.ts:1118` `idleHint` 门槛加 `|| !progress.hasHeartbeat`；测试 `idle-and-tail-07b.test.tsx:103-118` 断 3 分钟、1 小时后均为 null，反向断带 `idleMs: 0` 即恢复判定——改前会红，符合要求。

**2. start/retry 复位进度 + 按 runId 取快照（DS 2.2 中）— 已核实修好**
双保险都到位：`dispatch.ts:1078-1097` `nextProgress`（别轮次原样返回同引用）与 `progressOf`（runId 不符当没有），渲染走 `DispatchOverlay.tsx:561`；`start()` 在 `DispatchOverlay.tsx:244-246`、`retry()` 在 `:307-309` 各清一次三份 state。测试 `:178-202`（上一轮 600s idle 快照换 runId 后 detail 空串、hint null、迟到事件同引用）与 `:213-215`（三个清空各出现 2 次）覆盖。

**3. 重复工具 ≥3 且 30s 无输出（DS 2.4）— 已核实修好**
`dispatch.ts:1059` + `:1120-1121` `repeatedToolCalls >= 3 && idle >= REPEATED_TOOL_IDLE_MS`；契约 `tasks/TASK-07-idle-and-tail.md:24` 已改成「同名工具连续 ≥3 且 30 秒无输出」并注明缺元数据不判。测试 `:88-101` 五个断点（含阈值 ±1 与墙上时间凑满）齐全。

**4. 终态冻结耗时（DS 2.3）— 已核实修好**
`dispatch.ts:1138` 非 running 时 `wall = 0`，按最后一条心跳的 `elapsedMs` 冻结；测试 `:167-176` 断「完成」快照 +5 分钟仍是 42 秒、「失败」带工具名同理、在飞的照旧走。

**5. reconcileUiConverged 按终态过滤（DS 2.5）— 已核实修好**
`dispatch.ts:1155-1158` 只留 `status === 'draft' && text === RECONCILE_UNKNOWN_TEXT`，与节点卡「待核对」标签（`dispatch.ts:360`）同口径；注释 `:1147-1151` 与代码一致；无变化返回同引用（`:1159`）。测试 `:253-279` 覆盖「暂不可派仍保留 / pending 剔除 / 用户已改的剔除」。

**6. 节点卡口径写回任务书（DS 1.1）— 已核实修好**
`TASK-07-idle-and-tail.md:24` B-1 已加「R1 收窄……节点卡的 elapsed/lastTool 下期做，需把 dispatch-progress 订阅从 DispatchOverlay 提到 GraphTab」。

**7. 测试补强（DS 5.3–5.5）— 已核实修好**
叫停文案 + `cancelDispatchNode(runId, nodeId)` 接线（`:204-216`，且断无 `progress[node.nodeId]`）；limit 分支（`:431-448`，字面量文案、只派 1 次、未 markPending）；dispatch 抛错 state 形状（`:450-468`，正是组件 catch 读的三样）；`DUPLICATE_BATCH_TEXT` 字面量（`:393-397`）；GraphTab 收敛接线读源断言（`:288-292`）；源码扫描放宽正则与 `node:fs` 双引号（`:475-488`）；hint-capsule 注释（`GraphTab.css:375` + 测试 `:491-494`）。

## 回归核对（第 2–6 期路径）

无回归迹象：`runStageLoop`（`dispatch.ts:1210-1253`）保留了 skip 过滤与空集→done（`:122`）、重复批次先 `resetNodesToDraft` 再停（`:1239-1244`）、limit 在 markPending 之前停（测试钉住）；抛错时 state 就地更新供组件 catch（`DispatchOverlay.tsx:280-290`）读 graph/collected/activeNodes。GraphTab 包一层 `onCommit` 后仍返回落盘图（`GraphTab.tsx:834-838`），`commitGraph` 返回值契约成立；`dispatchableNodeIds` 在工具条（`:799-800`）仍有使用，无死导入。事件监听 runId 先守卫（`DispatchOverlay.tsx:193`）。vitest 278 / build 绿与代码状态相符。

## 最终结论

**TASK-07B 在 R1 之后可交付**——7 条全部闭合、修正未引入功能性新缺陷，遗留项均为文档债或计划内边界。仍未闭合的项（按严重度）：

1. （低）`tasks/TASK-07-idle-and-tail.md:25` B-2 仍写「剔除已不在 `dispatchableNodeIds` 的 id」，与 R1-5 实现的终态口径相反，下个读任务书的人会踩——纯文档债，无代码影响。
2. （低，计划内）心跳字段真实取值未实测：A 侧接通后需在真窗口验证一次读数，尤其是新加的 30 秒重复工具门槛（报告已声明）。
3. （nit）`progressSnapshot`（`dispatch.ts:1071`）把 `elapsedMs: null` 也算「有心跳」（`!== undefined`），若后端哪天序列化出显式 null 会退回误报；serde 按契约省略字段时不触发。
4. （nit）REPORT R1 验证命令行路径写成 `D:\workicross-p5b`（`REPORT-07B.md:316`、`:361`），`\a` 被吞的笔误，不影响结论。
5. （nit，计划内）节点卡 elapsed/lastTool 留下期（已按第 6 条写回任务书）；运行屏时钟 5s 一跳、胶囊最迟晚 5 秒。