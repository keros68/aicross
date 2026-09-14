# TASK-07C 修正轮 R1 复审

## 逐条核对

**1. 终态事件保留上一条心跳读数（DS F1）——已核实修好**
`progressSnapshot` 加了 `previous` 参数，无心跳字段且同 runId 时 `{ ...previous, status }` 只换状态词（`src-ui/src/graph/dispatch.ts:1070-1073`），`nextProgress` 喂入 `current[payload.nodeId]`（`dispatch.ts:1097`）；冻结由 `runningDetailOf` 的非 running 墙上时间 0（`dispatch.ts:1156`）与 `idleHint` 的 running 判据（`dispatch.ts:1136`）完成；测试钉住了沿用、5 分钟不变、跨 runId 不借用、无 previous 不编数四格（`idle-and-tail-07c.test.tsx:268-310`）。同 runId 守卫在 `nextProgress` 过滤后是冗余但无害的防御。

**2. listen 取消守卫（DS F2）——已核实修好**
`subscribeProgress` 的 `disposed` 标志 + 迟到 resolve 就地 `stop()` + 幂等 dispose（`dispatch-progress.ts:78-96`），hook effect 依赖 `[run.runId]`、cleanup dispose（`dispatch-progress.ts:124-131`）；可控桩测试覆盖了空 runId 不订阅、换 runId/卸载各 unlisten 一次、迟到 listen 立即解除、重复 dispose 不重复解除（`test.tsx:314-354`），失败路径从静默吞改成 `console.warn` 留痕（`dispatch-progress.ts:89`，`test.tsx:356-365`）。

**3. 异常收尾后时钟停（DS F3）——已核实修好**
`endProgressRun` 把 running 快照标 `'已结束'`、读数原样保留，全终态返回同一引用（`dispatch-progress.ts:56-68`）；`start()` 的 `finally { progressStore.endRun() }` 覆盖异常、missing-target 早退、正常跑完三条路（`DispatchOverlay.tsx:260-265`），正常路是同引用空操作不触发重渲染；`retry()` 走 `beginRun` 清空（`DispatchOverlay.tsx:283`）；`resetRun` 全仓已无引用，测试用 fake timers 钉了时钟停、10 分钟 `now` 不动、不挂胶囊（`test.tsx:369-425`）。

**4. 胶囊不裁切（DS F4）——已核实修好（样式/DOM 字符串层）**
第三行 `flex-wrap: wrap` + 显式 `line-height: 1.4`，读数 `flex: 1 1 100%` 独占一行（过长省略、全文在 title），胶囊 `max-width: 100%` + `white-space: normal` + `overflow-wrap: anywhere`，`flex-shrink: 0` 已移除（`GraphTab.css:255-278`）；高度账对得上：16.8+4+15.4+4+(15.4+2+36.8)+16+2 = 112.4 ≤ `NODE_H = 116`（`GraphTab.tsx:35-40`），胶囊内宽约 124px、最长文案约 140px 恰好两行；`NODE_H` 仍是唯一常量，连线端点（`GraphTab.tsx:527,529`）、布局（`:415-421`）、按钮行（`:576`）全部派生一致；断言含精确胶囊 markup 无省略号（`test.tsx:429-465`）。报告如实写明「未起窗口目视」。

## 新缺陷排查（第 2–7 期路径）

未发现回归：`statuses` state 删除后「排队」改为「无本轮快照」的默认值，在既有流程下等价（报告已记录语义边界）；`beginRun` 两处、`nextRunId` 先生成后写（`DispatchOverlay.tsx:279-283`）时序正确；07B 旧接线用例同步改到共享层入口（`idle-and-tail-07b.test.tsx:204-216`）。一个理论性残留：`endRun` 之后若再有迟到心跳仍会把快照翻回 running 并重启时钟——引擎事件先于命令返回到达，实际不触发，不构成回归。

## 结论

**R1 之后 TASK-07C 可交付**——4 条必修全部修好且各有对应测试，修正本身未引入新缺陷，vitest 294 / build 绿。

仍未闭合的项（按严重度）：

1. **派发进行中切图再切回，本轮读数与胶囊永久丢失**（GLM 缺项 2，已记录为不采纳的已知代价；要修需把 run 按 graphId 存 map，超出本轮）。
2. **`NODE_H=116` 与胶囊换行未经真机目视**（真实字体度量可能有出入；胶囊已可换行、高度留了 3.6px 余量，退化为留白偏差而非裁切）。
3. **订阅重挂窗口可能丢一跳心跳**（读数晚 15s 更新，两轮盲审一致认为观察即可）。
4. `endRun` 后迟到心跳翻回 running 的理论边界（实际事件顺序不触发）。