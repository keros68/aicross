# TASK-30 首版审查 · Antigravity / gemini-3.8-flash-high（盲审，材料内联）

**总判：可交付**

---

### 问题清单（按严重度编号）

本次实现完整满足任务书的所有硬性要求与验证标准，未发现阻碍交付的缺陷。以下列出 1 项轻微的边界设计注意点：

#### [Low-1] 续写图若外部误传不存在的 `requestNodeId`，回退至根节点仍会拿到旧需求
- **文件与位置**：`src-ui/src/graph/context.ts:162` (`runRequestOf`)
- **问题**：`runRequestOf` 实现为 `graph.nodes[requestNodeId]?.user.text ?? originalRequestOf(graph)`。在多轮续写图中，若调用方传入的 `requestNodeId` 不存在，回退到 `originalRequestOf(graph)` 依然会回退到整图的根节点（即第一轮需求 A）。
- **复现**：构造包含两轮对话的图（根节点需求 A，后续节点需求 B），以非法或不存在的 `requestNodeId`（如 `""` 或 `"invalid-id"`）调用 `runRequestOf(graph, "invalid-id")`，返回值将为需求 A 而非空字符串或本次需求。
- **影响评估与事实判断**：在当前生产接线（`console-chat.ts:289` 和 `console-channel.ts:374`）中，`requestNodeId` 均紧随同作用域上一行的 `appendConsoleMaterial` 返回值 `material.nodeId`，节点在内存图中必定存在，因此**当前控制台各路径不会触发此回退**。
- **改法建议**（可选加固）：若后续有从外部恢复或持久化数据反查需求节点的扩展需求，可考虑在回退时取最近的一个有效用户材料节点，或显式返回空字符串 `""` 避免静默拿错。

---

### 核过没问题的要点

1. **控制台所有路径（草稿、未应用历史消息、续写图）的需求来源均可靠**
   - 控制台模式派发入口由 `selectConsoleApplyMessage` 确定本次需求文本（草稿非空时取去除空白后的草稿；草稿为空时取最近一条未应用消息）。
   - 该文本传入 `dispatchConsolePreset` 后，由 `appendConsoleMaterial` 在当前图上生成唯一的材料节点 `material.nodeId`，`stamp` 紧随其后挂在该节点下。
   - 外部通道派发（`runConsoleDispatch`）亦同样先走 `appendConsoleMaterial`，两处均以刚生成的 `material.nodeId` 获取本次需求，不论是草稿还是历史消息，均指向本次实际执行的用户输入。

2. **需求在盖章（`stamp`）前固定，运行中图编辑不漂移**
   - 在 `console-chat.ts` 和 `console-channel.ts` 中，均在 `stamp` 执行前通过 `const runRequest = runRequestOf(material.graph, material.nodeId)` 提取为不可变局部常量。
   - 后续所有阶段（stages）及各节点的 `buildNodeRequest` 统一注入该 `runRequest`，即使运行期间节点被编辑或图结构更新，整个运行管线中所有阶段的 `originalRequest` 始终严格一致。

3. **旧画布三处调用语义严格不变**
   - `originalRequestOf(graph)` 的既有实现保留不变（依然返回 `graph.nodes[graph.root]?.user.text ?? ''`）。
   - `DispatchOverlay.tsx` 的 3 处既有调用点（行 186 思考等级判定、行 279 派发请求参数、行 558 经济模式小任务判定）未作任何改动，保持单需求画布的原始语义。
   - `context.test.ts` 中新增了专用单测，钉住了多节点图下 `originalRequestOf` 始终取根节点需求的既有行为。

4. **共享执行上下文完整保留上一轮对话延续**
   - `serializeContext` 是根据祖先链向上追溯闭包序列化上下文，未在根节点处硬编码「原始需求」抽取逻辑。
   - 续写图上新建的材料节点作为先前叶子节点的后代，执行节点的提示词通过 `serializeContext` 能够自然包含历史轮次的用户需求与模型回复。
   - 测试通过断言 `execution?.prompt` 同时包含需求 A、第一次执行回复和需求 B，验证了对话延续性未受影响。

5. **测试走真实前端接线，且红测记录真实可信**
   - `console-request-30.test.ts` 使用 `createConsoleApplyHandler` 监听真实 CustomEvent 派发，在同一个 `existingGraph` 上执行连续两次假派发。
   - 完整拦截第二次运行中的所有阶段请求，准确断言了盲节点（验收、验证）的 `originalRequest` 为需求 B 且非需求 A；同时用假 IO 验证了 `runConsoleDispatch` 外部通道路径。
   - 报告中明确记录了未引入实现前定向跑测试时退出码为 1、盲节点期望 B 实际收到 A 的红测输出，证实了测试能有效拦截回归。
   - 代码未改动 `src/**`，零 warning、构建与既有审计脚本均通过。
