# TASK-33 首版审查 · Antigravity / gemini-3.8-flash-high（盲审，材料内联）

### 总判
**不可交付**。核心的验收轮数计算在多阶段流水线预设（执行1→验收1→执行2→验收2）下仍会将预设自带的后续执行节点误判为修正节点，导致 0 轮修正被误报为「修正 1 轮后通过」（backlog 81 未完全闭合）；且父角色为「验证」时存在漏计、多分支并发修正存在轮数膨胀；收尾播报测试使用了读源码匹配字符串的静态弱测试。

---

### 严重度编号清单

#### [P1 - 严重] `src-ui/src/console-run-detail.ts:285-294` 与 `src-ui/src/console-channel.ts:444`
- **问题**：预设自带多阶段流水线（如：阶段1执行 → 阶段2验收 → 阶段3执行 → 阶段4验收）且无任何修正时，阶段3的正常执行节点会被判定为修正节点，结论被误报为「通过（修正 1 轮后通过）」，未彻底闭合 backlog 81。
- **原因**：
  1. `console-channel.ts` 调用 `startPipelineHeadless` 时传入 `fixRoundNodeIds: new Set(created)`，其中 `created` 是本次预设初始新建的**全部**节点集合。
  2. `correctionRoundCount` 中仅检查 `row.role === '执行' && fixRoundNodeIds.has(row.nodeId) && parent.role === '验收'`。由于阶段3执行节点在预设创建时就在 `created` 集合内，且其父节点正是阶段2验收节点，条件全部命中，被强行计入修正轮。
  3. 测试中的 `预设自带两次验收但没有修正时仍写验收一次通过` 用例只构造了 `exec -> accept-1 -> accept-2`（两个连续验收之间无执行节点），绕开了典型的多阶段执行流水线，导致假阳性漏测。
- **复现**：
  定义预设：阶段1为执行，阶段2为验收，阶段3为执行，阶段4为验收。正常运行全 PASS、无修正派生。`finishConsoleRun` 传入带该预设节点的 `fixRoundNodeIds`，输出结论为 `通过（修正 1 轮后通过）`，期望应为 `通过（验收一次通过）`。
- **改法**：
  `startPipelineHeadless` 内部应单独收集**调度循环中实际新生成的修正执行节点 ID 集合**（或从 `pipeline.fixRoundNodeIds` 中排除初始预设静态节点 `created`），`finishConsoleRun` 只比对该动态修正节点集合；不要将包含预设初始静态节点的 `new Set(created)` 直接当作修正节点范围。

---

#### [P2 - 中等] `src-ui/src/console-run-detail.ts:292`
- **问题**：修正执行节点的父节点角色判定遗漏了系统支持的「验证」角色，导致若验收阶段使用「验证」角色，其后生成的修正执行节点无法计入修正轮数。
- **原因**：
  在 `acceptanceConclusion` 开头明确兼顾了两种角色 `row.role === '验收' || row.role === '验证'`，但在 `correctionRoundCount` 判定父节点时硬编码为：
  `graph.nodes[parentId]?.target?.role === '验收'`。
  当父节点角色为「验证」时，判定返回 `false`，导致轮数统计为 0。
- **复现**：
  构造验收节点 `role: '验证'`，并在修正轮派出修正执行节点接在该节点之后。调用 `finishConsoleRun`，结论输出为 `通过（验收一次通过）` 而非 `通过（修正 1 轮后通过）`。
- **改法**：
  改为 `const parentRole = graph.nodes[parentId]?.target?.role; parentRole === '验收' || parentRole === '验证'`。

---

#### [P3 - 中等] `src-ui/src/console-run-detail.ts:285-294`
- **问题**：多分支并行修正时，轮数直接按修正执行节点的行数（数量）累加，导致单轮多路并发修正的轮数被成倍虚标。
- **原因**：
  `correctionRoundCount` 实现为 `return rows.filter(...).length;`。若同一次运行中有两条并行分支各自产出了 1 个修正执行节点，属于同 1 轮并发修正，但两行均命中过滤条件，`.length` 返回 2，导致结论显示为「修正 2 轮后通过」。
- **复现**：
  构造两个并行的执行+验收分支，第一轮两路均返回 `NEEDS_FIX`，分别派出 1 个修正执行节点（共 2 个执行节点），随后均通过。此时实际只修正了 1 轮，但判定给出 `通过（修正 2 轮后通过）`。
- **改法**：
  修正轮数应按修正节点的迭代层级（或去重后的 stage 阶段数）计算，或按同一链上的最大修正深度统计，而不是直接 `filter.length` 数节点个数。

---

#### [P4 - 较低] `src-ui/src/__tests__/console-polish-33.test.ts:296-300`
- **问题**：任务书第 7 条（收尾播报换算阶段号）的断言采用 `readFileSync` 检查源码文本中是否包含特定变量语句，属于无效弱测试。
- **原因**：
  测试直接读 `console-chat.ts` 源码匹配字符串 `const displayed = displayResults(collected)`，未通过调用执行函数验证播报输出是否真切使用了换算后的阶段号。若后续有重命名或局部重构会报虚警，若参数使用有偏差此断言也无法拦截。
- **复现**：
  静态代码审查事实，在 `src-ui/src/__tests__/console-polish-33.test.ts` 296-300 行可见。
- **改法**：
  在既有的 `createConsoleApplyHandler` 真实接线测试中追加断言：断言多次派发或包含多阶段时，播报消息文本或 `onRunUpdate` 携带的 stage 确实为经过 `displayResults` 换算后的展示阶段号。

---

#### [P5 - 较低] `src-ui/src/components/center/ConsoleRunDetail.tsx:75-80`
- **问题**：当页内无可聚焦元素（`focusable.length === 0`）时，`runDetailTabTarget` 返回 `null`，键盘事件未调用 `preventDefault()`，焦点将脱离详情页溢出至外层左右栏；且根容器 `<div ref={rootRef}>` 无 `tabIndex`，在关闭按钮不可用时无法承接初始焦点。
- **说明**：
  当前硬编码实现中头部存在未禁用的「返回对话」按钮，因此目前不会立即触发；但纯函数边界缺少兜底保护，属于防御性缺陷。
- **改法**：
  当 `runDetailTabTarget` 返回 `null` 时，若仍处于详情页内，应同样拦截默认事件；同时给根容器增加 `tabIndex={-1}` 作为聚焦兜底。

---

### 核过没问题的要点

1. **验收链末次判定（祖先链遍历）**：
   `hasAncestor` 通过 parents 广度优先向上回溯祖先，`acceptanceRows.filter` 成功过滤掉同链上存在更晚验收节点的旧节点。不等深并行分支（浅分支 NEEDS_FIX、深分支 PASS）下，浅分支与深分支末次节点均被正确保留，任一 NEEDS_FIX 即判未通过，消除了旧版仅看最大 stage 带来的误判。
2. **焦点恢复与打开按钮消失处理**：
   `openRunDetail` 正确捕获打开前的 `activeElement`；`ConsoleChat` 的 `useLayoutEffect` 中通过 `runDetailFocusTarget(Boolean(previous?.isConnected))` 判断，若原打开按钮因消息列表重渲染已脱离文档，焦点稳定回退至 `composerRef`（输入框），未出现死焦点。
3. **首条心跳前的 pending 状态与持久化记录兼容**：
   - 阶段开始时行置为 `pending`，状态显示「排队中」，耗时固定为「—」；心跳到达后才置为 `running` 并走表。
   - 对持久化历史记录（已有 20 份），既有 `running` 状态在 `withConsoleRunStage` 做了保护（`previous.status === 'running' ? 'running' : 'pending'`），不会倒退回 pending；旧记录在详情页中正常显示「进行中」与对应耗时，向下完全兼容。
4. **一次性跳过自动滚底标记机制**：
   - `scrollSnapshotRef.current` 恢复滚动后设置 `skipAutoScrollRef.current = true`。
   - 自动滚底的 `useEffect` 将 `openRunId` 纳入依赖，在关闭详情页的同帧必被调度；一旦检测到跳过标记，立即将其清理回 `false` 并中止当次滚动，后续新消息到达时正常滚底，标记绝不会卡死。
5. **表格 900px 列宽与紧凑数字悬停展示**：
   - 十列宽度合计精准为 900px（`min-width: 900px`），在默认 940px 容器下完整展示，消除了中间栏默认宽度下的横向滚动条；单元格横向 padding 收窄为 6px，模型列保留断行。
   - `>= 10,000` 数字转为 `k/m` 紧凑展示，在行数据、底部小计、总计三处均通过 `title={String(...)}` 完整提供了原生 hover 全数展示。
6. **startPipelineHeadless 接口兼容**：
   返回对象中增加 `fixRoundNodeIds: ReadonlySet<string>`，正常与异常路径均有返回值（默认空 Set），旧调用方（如画布相关派发）不受任何影响。
7. **核心测试的真实接线与变异有效性**：
   - 第 2 条针对 `createConsoleApplyHandler` 补齐了真实接线测试，覆盖一轮修正到 PASS 的完整事件派发与回调；变异空集合确认了红测。
   - 第 5 条针对 `runConsoleDispatch` 补齐了连续两次事件派发的真实接线测试，验证了第二次 NEEDS_FIX 派发 4 次请求的修正闭环。
