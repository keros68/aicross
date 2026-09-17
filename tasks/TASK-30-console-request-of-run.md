# TASK-30 续写对话图时盲审拿到本次需求（09-17）

对应 backlog 92。**只做这一件事。** 不碰 `src/**`。

## 问题（编排者 09-17 窗口实测，已核实原因）

同一控制台先问「用一句话说明 beta.py 的用途」，再问「用一句话说明 alpha.py 的用途」。第二次运行的验收原文：「REQUEST.md 写 beta.py 而 ANSWER.md 答 alpha.py，两者对象不一致」。

原因：`src-ui/src/graph/context.ts:156` `originalRequestOf(graph)` 返回 `graph.nodes[graph.root].user.text`，即整张图的第一条用户消息。控制台每次应用模式都续写同一张图（TASK-29 刻意保留），于是第二次起每次运行的盲审 `REQUEST.md`、以及其它用到 `originalRequestOf` 的地方，拿的都是第一次的需求。旧画布一张图一个需求，所以一直没暴露。

## 要做

1. 控制台模式派发（`console-chat.ts` 的 `dispatchConsolePreset` buildRequest）与外部通道派发（`console-channel.ts` 约 435 行）传给 `buildNodeRequest` 的 `originalRequest`，改为**本次运行对应的用户需求**：本次 stamp 所挂的那条用户消息（即本次新建节点往上最近的、非本次新建的用户消息节点，或本次应用时选中的消息正文——先核实 `selectConsoleApplyMessage` 选的是哪条、stamp 挂在哪个节点下，选最可靠的来源，报告里写清）。
2. **所有**用到 `originalRequestOf` 的调用点逐个列出并判断：旧画布（一图一需求）保持原语义不变；控制台续写路径改为本次需求。优先新增一个明确的函数（例如 `runRequestOf(graph, nodeId, runNodeIds)`），不要改 `originalRequestOf` 的既有语义。
3. 若 `serializeContext` / 共享上下文里也有「原始需求」一节取自根节点，同样核实并按本次需求处理；执行者（共享上下文）仍能看到上一轮对话作为延续材料，这一点不能丢。

## 验证

`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`、`python scripts/copy-audit.py` 不新增命中。

- **必须走真实接线**：用 `createConsoleApplyHandler` / `dispatchConsolePreset` 在同一 `existingGraph` 上连续两次假派发（第一次需求 A、第二次需求 B），捕获第二次发往引擎的请求，断言所有盲节点（验收 / 验证）的 `originalRequest` 是 B 不是 A；执行节点的上下文仍包含第一次的对话延续。
- 旧画布调用点行为用测试钉住不变。
- 先确认去掉实现会红，在报告里写清怎么红的。

保持文件原有换行；不 `cargo fmt`；不引入 jsdom；不起窗口、不发真机请求、不调真实模型；不 commit、不 push。报告写 `tasks/REPORT-30.md`。backlog 不用改。
