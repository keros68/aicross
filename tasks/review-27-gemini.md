# TASK-27 首版审查 · Antigravity / gemini-3.8-flash-high（盲审，材料内联）

### 总判
**可交付但有未闭合项**。核心的详情页展示、真实接线、token 未报告与双重耗时口径、结论判定及持久化基本做通，但在中途抛错容错、焦点守卫接入和心跳高频落盘控制上存在未闭合缺陷。

---

### 问题清单（按严重度排列）

#### 1. [高] `dispatchConsolePreset` 异常时心跳订阅泄漏，运行记录永久卡在「进行中」
- **文件与位置**：[`src-ui/src/console-chat.ts:325-445`](file:///src-ui/src/console-chat.ts#L325-L445)
- **问题**：在创建 `runRecord`（初始 `status: 'running'`）并订阅 `progressSubscription` 后，读取配置 `readDispatchSettings` 或 `runPipeline` 若出现网络异常、Promise reject 或抛错，函数直接向外抛出异常。后续的 `progressSubscription?.dispose()` 与 `finishConsoleRun` 均被跳过；外层 `createConsoleApplyHandler` 的 `.catch` 仅追加了一条错误文字消息，并未更新 `runRecord`。
- **复现**：模拟 `io.dispatchGraphStage` 抛错拒绝，派发中断后查看控制台状态：虽然「处理中…」因 `finally` 消失，但持久化与内存中的该份运行记录永远停留在 `status: 'running'`，且心跳事件监听器未注销，造成事件泄漏。
- **改法**：使用 `try...catch...finally` 保护派发流程：
  - 在 `catch (error)` 中调用 `finishConsoleRun(runRecord, graph, [], message, now())`，将记录状态设为 `'stopped'` 并通知 `feedback?.onRunUpdate`；
  - 在 `finally` 块中确保执行 `progressSubscription?.dispose()`。

#### 2. [中] 未按任务书要求接入 `lib/focus-enforcer.ts` 焦点守卫，测试用读源码绕过
- **文件与位置**：[`src-ui/src/components/center/ConsoleRunDetail.tsx:62-76`](file:///src-ui/src/components/center/ConsoleRunDetail.tsx#L62-L76) 及 [`src-ui/src/__tests__/console-run-detail-27.test.ts:257-266`](file:///src-ui/src/__tests__/console-run-detail-27.test.ts#L257-L266)
- **问题**：任务书明确要求「注意 `lib/focus-enforcer.ts` 的焦点守卫，参照设置弹窗的做法」。但实现中完全未引用 `lib/focus-enforcer.ts`，仅在根元素挂载 `tabIndex={-1}` 并通过 `useLayoutEffect` 执行一次 `focus()`。
  - 用户按下 Tab 键可直接将焦点移出全屏详情页至外部侧边栏或窗口控件；
  - Esc 关闭或点击返回后未恢复原触发节点的焦点（Focus Restoration）；
  - 测试用例仅用 `readFileSync` 检查组件文件中是否包含字符串 `role="dialog"` 和 `aria-modal="true"`，掩盖了未真正接入焦点守卫的问题。
- **复现**：在详情页打开状态下持续按 Tab 键，焦点跳出中间满版区域移至外部控件。
- **改法**：引入现有 `lib/focus-enforcer.ts` 的焦点捕获/守卫逻辑（或参照设置弹窗的 FocusEnforcer 组件/hook），接管 Tab 键循环并保留打开前的 `activeElement` 用于关闭时归还焦点。

#### 3. [中] 节点心跳频繁触发同步写盘，存在主线程与 I/O 性能隐患
- **文件与位置**：[`src-ui/src/store/app-state.tsx:772`](file:///src-ui/src/store/app-state.tsx#L772) 及 [`src-ui/src/console-chat.ts:327-333`](file:///src-ui/src/console-chat.ts#L327-L333)
- **问题**：`CONSOLE_PERSIST_ACTIONS` 集合中直接加入了 `'UPSERT_CONSOLE_RUN'`。每当节点心跳（`dispatch-progress`）上报时，`withConsoleRunProgress` 都会触发 `onRunUpdate` 并 dispatch 该 action，进而无节流地直接调用 `persistConsoleSessions`，对所有会话及记录做深度序列化并同步写入 `localStorage`。多路并发派发时，心跳高频写盘会无谓占用主线程并加剧磁盘磨损。
- **复现**：多路并发运行时打印 `localStorage.setItem` 调用次数，每次心跳到达均会触发完整的 sessions 序列化写入。
- **改法**：心跳更新主要是瞬时视图状态，无需每次都同步持久化。可将心跳推进与落盘分离（如心跳只更新 state 但不加入 `CONSOLE_PERSIST_ACTIONS`），仅在阶段变更（`onStage`）、结果产出（`onResults`）、收尾完成（`finishConsoleRun`）或通过 debounce（节流）落盘。

#### 4. [低] 特殊多阶段模式下的验收轮数文案在无修正时表述稍欠精准
- **文件与位置**：[`src-ui/src/console-run-detail.ts:221-224`](file:///src-ui/src/console-run-detail.ts#L221-L224)
- **问题**：若预设模式本身定义了多个阶段的验收节点（非自动修正轮生成），`stages.length` 会大于 1。当最后一阶段判定未通过时，文案直接套用 `未通过（修正 ${round - 1} 轮后验收再次要求修正，已到上限）`，在非自动修正场景下会出现「修正 N 轮后」的措辞。
- **改法**：结合任务是否实际派发了修正阶段，或检查是否存在修正执行节点来决定是否输出「修正 X 轮后」字样。

---

### 核过没问题的要点

1. **结论判定准确且隔离前置轮次**：
   - 结论严格取最后一轮验收节点（`last = acceptanceRows.filter(row => row.stage === lastStage)`），修正轮前一轮的 `NEEDS_FIX` 不会污染最后一轮；
   - 最后一轮多验收节点时，任一 `NEEDS_FIX` 判为未通过；无 `NEEDS_FIX` 但含 `UNKNOWN` 准确判为「验收结论无法识别」；全部通过判为「通过」；
   - 无验收节点准确输出「已完成（本模式不含验收）」；中途停止优先输出停止原因；
   - 收尾消息统计改为「运行失败 N 路」，消除了与验收结论的混淆。
2. **token 口径与耗时口径完全符合要求**：
   - 仅当已完成节点 `tokensIn === 0 && tokensOut === 0` 时显式展示为「未报告」，不计入合计，且小计和合计均准确注明「N 路未报告，未计入」；
   - 墙钟耗时（整次运行时间起止差）与各路耗时合计（每一路耗时加总）同时列出，并附说明文案。
3. **心跳走表机制与中性文案**：
   - 进行中的行由心跳 `progressElapsedMs` 基准加上前端时钟继续走表，工具名称与调用次数有值才展示；
   - 备选路由标明「主 → 备」；kimi 实际模型缺失时按既有口径展示「kimi 不报真身」；无金额与额度百分比，符合 `docs/PRODUCT.md`。
4. **数据结构向后兼容与重启冻结**：
   - 旧数据缺少 `runRecords` 字段时容错加载为空数组，异常数据被安全过滤；
   - 每个会话严格截断保留最近 20 份；
   - 重启加载时通过 `interruptConsoleRun` 准确将进行中的记录和节点行冻结为「已中断」，耗时冻结在中断时间戳。
5. **禁区遵守与接线真实性**：
   - 严格未触碰 `src/**`、`tauri.ts`、`context.ts`、`types.ts` 及 `dispatch.ts`；
   - 接线测试直接通过 `createConsoleApplyHandler` 发送派发事件，验证了两阶段流水线、备选路由、token 为 0、NEEDS_FIX 及重启持久化恢复全链路。
