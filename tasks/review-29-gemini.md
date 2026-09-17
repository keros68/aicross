# TASK-29 首版审查 · Antigravity / gemini-3.8-flash-high（盲审，材料内联）

**总判：可交付**

TASK-29 对应的 86、87、88 三项改动均已严格按任务书要求实现，未修改引擎（`src/**`），未破坏旧画布调用点语义，测试走真实接线且红测真实有效，未发现阻塞性缺陷。

---

### 按严重度编号清单（问题与观察项）

#### 1. [低 / 维护建议] `src-ui/src/console-chat.ts`: 约 470 行
- **现象 / 潜在隐患**：在收尾的 `announceResults(uniqueResults)` 调用中，传入的是原始未换算的 `uniqueResults`，而在更新 runRecord 时使用的是 `displayResults(uniqueResults)`。
- **现状分析**：经核实，当前节点完成时发出的逐路系统消息仅提取了角色、agent、model 与输出摘要，并不包含阶段号（阶段号仅在 `onStage` 触发的“第 X 阶段 · 派出 Y 路”中输出）；因此当前三处展示点（系统提示、详情页、收尾结论）完全一致，没有展示 bug。
- **改法建议**：为防止未来在单节点完成反馈中展示阶段号时出现分歧，可建议后续统一为 `announceResults(displayResults(uniqueResults))`。

#### 2. [低 / 时序边界] `src-ui/src/components/center/ConsoleChat.tsx`: 约 63-75 行
- **现象 / 潜在隐患**：消息流的默认触底由 `useEffect` 中的 `endRef.scrollIntoView` 驱动，而详情页返回恢复由 `useLayoutEffect` 设置 `list.scrollTop`。
- **现状分析**：在从详情页返回对话的当前 render 中，如果打开前不在底部且返回时没有新消息触发依赖变动，`useLayoutEffect` 会在绘制前精确恢复原位置；若打开前在底部且有新消息，纯函数与 effect 均指向最新底部。当前行为符合预期。
- **改法建议**：若未来详情页交互由条件渲染重构成纯模态覆盖（保留 underlying DOM），需注意确保 `scrollSnapshotRef` 在卸载和重新可见时的重置逻辑不被多次触发。

---

### 核过没问题的要点

1. **`fixRoundNodeIds` 集合完整性与计数边界**：
   - **集合闭环**：控制台以本次 `stampPreset` 新增的 `newNodeIds` 初始化集合；推进过程中在 `runStageLoop` 内通过比对 `reducePipelineStep` 前后的图节点，将生长的修正执行节点与再验收节点动态增补至集合中，不会漏计导致无限修正。
   - **隔绝旧图**：`acceptanceRound` 沿父链路回溯时只累加属于 `fixRoundNodeIds` 的验收节点，上一轮留下的验收节点被准确跳过，不会多计导致提前到顶。
   - **旧画布语义严格不变**：旧画布 `DispatchOverlay` 不传入该参数，`scopeNodeIds` 为空时走原本全图遍历计数逻辑，单元测试已同时覆盖默认全图封顶与控制台范围独立封顶。

2. **展示阶段号换算一致性**：
   - **换算基准**：通过计算本次所有新节点中带 target 的最小 `graphStage` 作为基准偏移，后端引擎请求继续保留全图绝对 `stage`，未改动引擎排序与留痕语义。
   - **三处一致**：
     - 系统提示：`onStage` 中通过 `displayStage` 换算并去重，输出“第 1 阶段...”。
     - 详情页：`withConsoleRunStage` 与 `displayResults` 将记录行中的 `stage` 全部置为换算后的 1, 2, 3, 4，表格分组完全从 1 起。
     - 收尾结论：统计范围限定于本次运行派发节点，收尾文案输出“修正 1 轮后验收再次要求修正，已到上限。共 4 路”，轮数与路数均吻合。

3. **详情页列宽统一与溢出防护**：
   - 各阶段表格统一采用 `<colgroup>` 配置固定宽度数组 `[90, 90, 180, 150, 130, 80, 100, 100, 100, 100]`，总和为 1120px，配合 `table-layout: fixed` 与 `min-width: 1120px`，各阶段列宽绝对对齐。
   - `.console-run-detail` 与 `.console-run-body` 设置 `overflow-x: hidden` 阻止页面自身横向溢出，仅 `.console-run-table-wrap` 允许 `overflow-x: auto` 横向滚动。

4. **模型上报与页头文案口径**：
   - `reportedModelLabel` 严格区分状态：`pending` / `running` 均显示 `—`；已完成无真身时才判定 kimi（`kimi 不报真身`）或其它（`未报告`）。
   - `consoleRunHeaderLabel` 彻底移除了内部 `console-mode-<uuid>`，提取本地时间精确格式化为 `模式名 · YYYY-MM-DD HH:mm`。

5. **滚动恢复纯函数与真实接线测试**：
   - 滚动逻辑提炼为 `consoleScrollSnapshot` / `consoleScrollRestoreTop` 纯函数，包含 `<= 2` 像素的亚像素容差保护，测试在 Node 环境无 DOM 依赖下稳定运行。
   - 86 测试通过 `createConsoleApplyHandler` 在同一张图上连续触发两次完整 apply 派发，且通过对 `reducePipelineStep` 范围逻辑变异证实测试具有真实否定力（会红）。
