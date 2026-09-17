# REPORT-30 续写对话图时盲审拿到本次需求

## 结果

控制台连续运行改为从本次 `stamp` 所挂的材料节点读取 `originalRequest`。同一张图先运行需求 A、再运行需求 B 时，第二次的执行、验收和验证请求均收到 B；共享执行上下文仍保留 A、第一次执行回复和 B。

外部通道使用同一来源。旧画布继续按根节点读取原始需求，既有语义不变。

## 实现

- `src-ui/src/graph/context.ts`
  - 保留 `originalRequestOf(graph)` 的根节点语义。
  - 新增 `runRequestOf(graph, requestNodeId)`。`requestNodeId` 来自 `appendConsoleMaterial` 的返回值，即本次 `stamp` 的挂载节点；恢复图缺少该节点时才退回 `originalRequestOf`。
- `src-ui/src/console-chat.ts`
  - `dispatchConsolePreset` 在盖章前用 `runRequestOf(material.graph, material.nodeId)` 固定本次需求，`buildRequest` 的所有阶段复用该值。
- `src-ui/src/console-channel.ts`
  - `runConsoleDispatch` 同样在盖章前固定本次需求，避免运行期间图节点编辑导致不同阶段漂移。
- `src-ui/src/__tests__/console-request-30.test.ts`
  - 通过 `createConsoleApplyHandler` 和真实 `dispatchConsolePreset` 接线，在同一 `existingGraph` 上连续两次假派发。
  - 第二次两条盲节点分别为验收、验证，均断言 `originalRequest` 是 B、不是 A。
  - 第二次共享执行节点断言 `originalRequest` 是 B，同时 `prompt` 仍含 A、第一次执行回复和 B。
  - 另以假 IO 运行真实 `runConsoleDispatch`，钉住外部通道的本次需求来源。
- `src-ui/src/graph/__tests__/context.test.ts`
  - 钉住旧画布 `originalRequestOf` 仍取根节点，后续用户消息不改变该语义。

`selectConsoleApplyMessage` 的核实结果：草稿非空时取去除首尾空白后的草稿；草稿为空时取最近一条尚未应用的用户消息。该正文传入 `dispatchConsolePreset`，由 `appendConsoleMaterial` 写入 `material.nodeId`，`stamp` 再挂到这个节点下。因此 `material.nodeId` 是本次需求的直接来源，比从后代节点反向搜索稳定。

`serializeContext` 没有单独从根节点生成“原始需求”字段。它按祖先闭包序列化用户消息和模型回复，因此未改动；共享执行节点继续获得上一轮对话作为延续材料。

## `originalRequestOf` 调用点核对

任务开始时的五个生产调用点均已核对：

1. `src-ui/src/console-chat.ts` 的 `dispatchConsolePreset` 请求装配：控制台续写路径，已改为本次材料节点读取并固定的 `runRequest`。
2. `src-ui/src/console-channel.ts` 的 `runConsoleDispatch` 请求装配：外部通道续写路径，已改为本次材料节点读取并固定的 `runRequest`。
3. `src-ui/src/components/center/graph/DispatchOverlay.tsx:186`：旧画布按根需求决定是否提高执行节点的思考等级；一张图一个需求，保留 `originalRequestOf(graph)`。
4. `src-ui/src/components/center/graph/DispatchOverlay.tsx:279`：旧画布派发请求的 `originalRequest`；一张图一个需求，保留 `originalRequestOf(sourceGraph)`。
5. `src-ui/src/components/center/graph/DispatchOverlay.tsx:558`：旧画布判断经济模式小任务；根需求和根附件仍是该画布的任务输入，保留 `originalRequestOf(graph)`。

当前另有两处非生产路径调用：`runRequestOf` 在材料节点缺失时调用 `originalRequestOf` 作为旧图兼容回退；`context.test.ts` 直接调用它验证旧画布语义。

## 红测

先只加入测试，不改实现，运行：

```text
cd src-ui
npx vitest run src/__tests__/console-request-30.test.ts src/graph/__tests__/context.test.ts --configLoader native --pool threads
```

结果：退出码 1；1 个测试失败、18 个通过。失败断言显示第二次两条盲节点期望 B，实际均为 A：

```text
Expected: ["需求 B：说明 alpha.py", "需求 B：说明 alpha.py"]
Received: ["需求 A：说明 beta.py", "需求 A：说明 beta.py"]
```

加入实现后同一组定向测试为 2 个文件、20 个测试全部通过。

## 验证

- `cargo check --all-targets`
  - 退出码 0。
  - `Finished dev profile`，无 warning。
- `cargo test`
  - 退出码 0。
  - `473 passed; 0 failed; 11 ignored`。
- `cd src-ui && npx vitest run --configLoader native --pool threads`
  - 退出码 0。
  - `45 passed` 测试文件，`656 passed` 测试。
- `cd src-ui && npm run build`
  - 退出码 0。
  - TypeScript 编译和 Vite 构建完成；仍有既有的 500 kB chunk 提示。
- `python scripts/copy-audit.py`
  - 退出码 0。
  - 共 6 处命中、5 个文件；均为既有命中，本次没有新增用户可见文案。唯一处于本次修改文件中的命中是 `console-channel.ts` 原有注释“归类模型判定不必派发”，该行未改动。
- `git diff --check`
  - 退出码 0。

## 范围与限制

- 未改动 `src/**`，未运行 `cargo fmt`，未引入 `jsdom`。
- 未打开窗口，未发送真机请求，未调用真实模型。
- 未修改 backlog，未 commit，未 push。
- 外部通道通过真实前端接线加假 IO 验证；按任务书限制，未做真实 CLI 或模型验收。
