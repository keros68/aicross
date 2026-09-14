## 复审结论（TASK-07B 修正轮 R1）

**1. 无心跳元数据的快照不累计墙上时间（DS 2.1 高 / GLM 1）— 已核实修好**
`src-ui/src/graph/dispatch.ts:1071` 按 `elapsedMs !== undefined || idleMs !== undefined` 算 `hasHeartbeat`（字段在 `:1051`），`:1118` `idleHint` 对 `!hasHeartbeat` 直接返回 null；测试 `__tests__/idle-and-tail-07b.test.tsx:103-118` 断 3 分钟/1 小时后仍为 null，且补一个 `idleMs:0` 就恢复判定，改前必红。判定分支三条都在（终态/排队/无元数据都不判），无误报面。

**2. 新一轮复位进度 + 按 runId 取快照（DS 2.2 中 / GLM 2）— 已核实修好**
双保险：`DispatchOverlay.tsx:244-246`（start）与 `:307-309`（retry）各清 `progress`/`statuses`/`currentNodes`；`dispatch.ts:1078-1086` `nextProgress` 丢别轮事件（同引用返回）、`:1090-1096` `progressOf` 按 `runId` 过滤，渲染点 `DispatchOverlay.tsx:561` 已无 `progress[node.nodeId]`（测试 `:204-215` 反向断言）。快照带 `runId`（`dispatch.ts:1043`）而 `runId` 只在 `retry()` 换新、`start()` 期间不变，过滤不会误杀本轮。

**3. 重复工具 ≥3 且 30s 无输出才提示（DS 2.4）— 已核实修好**
`dispatch.ts:1059` `REPEATED_TOOL_IDLE_MS = 30s`，`:1120` 条件为 `>= 3 && idle >= 30000`（`idle` 含墙上时间）；契约已同步 `tasks/TASK-07-idle-and-tail.md` B-1「同名工具连续 ≥3 且 30 秒无输出」；测试 `:88-101` 覆盖 4 个边界（3 次+2s 不报、阈值-1 不报、2 次+30s 不报、3 次+30s 报）。

**4. 终态冻结耗时（DS 2.3）— 已核实修好**
`dispatch.ts:1138` 只有 `progressRunning` 为真才加 `now - receivedAt`，终态按最后一条心跳的 `elapsedMs` 冻结；测试 `:167-176` 断 `完成`/`失败` 过 5 分钟不变、仍在跑的继续走。

**5. `reconcileUiConverged` 按终态过滤（DS 2.5 / GLM 3）— 已核实修好**
`dispatch.ts:1155-1158` 判据改为「仍是 `draft` 且正文 `RECONCILE_UNKNOWN_TEXT`」，与节点卡 `:360` 同口径；注释 `:1143-1151` 已与代码一致。这样在飞（pending）剔除、暂不可派（父未完成/无 target）的 draft 保留（`:266/:270` 断同引用），不会再让 `initialDispatchSelection` 之后默认勾选派发；无变化返回同引用，不会触发 setState 抖动。

**6. 节点卡口径写回任务书（DS 1.1）— 已核实修好**
`tasks/TASK-07-idle-and-tail.md` B-1 括注「R1 收窄：本期只覆盖 G2 运行行；节点卡的 elapsed/lastTool 下期做，需把 dispatch-progress 订阅从 DispatchOverlay 提到 GraphTab」在位，本轮未擅自大改。

**7. 测试补强（DS 5.3–5.5）— 已核实（基本修好，一处残留弱点）**
`runStageLoop` limit 分支 `:431-448`、dispatch 抛错路径 state 形状 `:450-470`、`DUPLICATE_BATCH_TEXT` 字面量双向断言 `:390-394`、GraphTab 收敛接线源断言 `:288-292`、正则放宽 `:478-489`/`:472-476`、`hint-capsule` 注释 `GraphTab.css:375` 均落实。唯一残留：任务书要「运行行**渲染**断言」，实际做成读源文件断言（`:204-215`）——文案与 `cancelDispatchNode(runId, node.nodeId)`（`DispatchOverlay.tsx:572`）都钉住了，但整行渲染路径（`currentNodes` 映射 + `progressOf` 取值）仍无组件级覆盖；这是覆盖弱点不是错修。

**新缺陷 / 第 2–6 期回归 — 未发现**
R1 的生产代码改动只落在新增/新增门槛上：`idleHint`/`runningDetailOf`/`progressOf` 的调用点只有 `DispatchBits.tsx:52`、`DispatchOverlay.tsx:562`，`reconcileUiConverged` 只在 `GraphTab.tsx:836`；`applyDispatchResults` 只加 `tokens_cache_read`（`dispatch.ts:267`）、`applyReconcile` 只加一个键（`:842`），擂台/仲裁/采纳/交接、`dispatchableNodeIds`、`nextStageStep`、`reducePipelineStep`、回滚标志均未动；`ProgressSnapshot` 无其他消费者，第 2–6 期无影响。编排者复跑 278/build 绿与报告一致（净增 9 条、本文件 24 条，逐条对得上）。

## R1 之后 TASK-07B 是否可交付？

**可交付**：7 条必修（含 GLM 同题发现）全部真实落地且测试转红为绿，未引入新缺陷。

仍未闭合项（按严重度）：
1. **中** — 画布节点卡的 elapsed/lastTool/空转胶囊仍未做（原始 B-1 的一半），已由 R1-6 写回任务书，需另开一期把 `dispatch-progress` 订阅提到 `GraphTab`。
2. **低** — 运行行「叫停」/接线只有源文件断言，缺组件级渲染覆盖；`DispatchOverlay` 两处 `Date.now()`（事件监听、5s 时钟）零测试。
3. **低** — A 侧心跳未接通，`elapsedMs`/`idleMs`/`repeatedToolCalls` 与 30s 门槛只能接通后在真窗口核对；`lastTool` 若含命令行/路径会原样显示（A 侧建议只放工具名）。
4. **低** — 运行屏时钟 5s 一跳，胶囊最迟晚 5 秒出现（报告已声明）。
5. **提示级** — 报告个别行号漂移（`dispatch.ts:1044` 实为 1043、`1119-1121` 实为 1120），`tauri.ts`/`src/` 未动的换行与「零改动」声明只能用 diff 复核（只读工具无法核验），不影响交付结论。