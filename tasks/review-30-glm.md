# TASK-30 首版审查 · GLM-5.3-Flash（cc-switch，只读）

## 总判

**可交付。** 任务书三条全部落实且接线正确，未发现会产生错误行为的新问题；仅有两条低severity备注（一条表述失真、一条边界行为变化），均不阻塞。验证命令本会话无法复跑（无 shell 工具），以静态核对 + 报告为据。

## 问题清单

1. **【低】`runRequestOf` 的 `??` 回退对「空文本」不回退** — `src-ui/src/graph/context.ts:162-164`
   - 问题：材料节点存在但 `user.text === ''` 时返回 `''`（`??` 不把空串当 nullish）。外部通道 `runConsoleDispatch` 的 `event.text` 若为空（`console-channel.ts:373`），`originalRequest` 会是空串；改动前是根节点旧需求（同样是错的）。
   - 复现：仅当 Rust 侧 `aicross:console-dispatch` 事件允许空 `text`（未核实，标不确定）。控制台 UI 路径不可能为空——`selectConsoleApplyMessage` 已挡 `blocked: 'empty'`（console-session.ts:178-181）。
   - 改法：不必急改。若要堵，在 `runConsoleDispatch` 入口对空 `text` reject，而不是把空串伪装成旧根需求。

2. **【备注，非缺陷】报告对回退分支的表述与实际接线不符** — `src-ui/src/graph/context.ts:161` 注释及 REPORT-30.md:13
   - `material.nodeId` 与 `material.graph` 出自同一次 `appendConsoleMaterial` 调用（console-channel.ts:66-75，两条分支都保证节点在返回图里），因此「恢复图缺该节点时退回 `originalRequestOf`」这条路径在现有两处生产调用点**不可达**，回退分支是纯防御代码。这同时意味着评审关注的「恢复图回退到根拿错需求」**不会发生**——根回退根本走不到。报告表述会误导后续维护者以为存在这条路径，建议改注释，不改行为。

3. **【备注，按任务书属预期】旧画布覆盖层用于控制台续写图时仍取根需求** — `src-ui/src/components/center/graph/DispatchOverlay.tsx:186,279,558`
   - 用户用旧画布 G1 派发一张被控制台续写过多次的图，仍按第一次需求派发。任务书明确要求旧画布语义不变，不算问题，仅指出该混用场景的残余行为。

## 核过没问题的要点

- **任务书第 1 条（两条派发路径改为本次需求）**：`dispatchConsolePreset` 在 `console-chat.ts:288-289` 于 stamp 前取 `runRequestOf(material.graph, material.nodeId)`；`runConsoleDispatch` 在 `console-channel.ts:373-374` 同样。`runRequest` 是闭包常量，`buildRequest` 全阶段复用。
- **需求来源可靠性（逐路径）**：`selectConsoleApplyMessage`（console-session.ts:173-186）草稿非空取 trim 后草稿、否则取最近一条未应用的用户消息，`selection.text` 经 handler 传入派发（console-chat.ts:530-531）；`stamp` 把第一阶段节点直接挂在材料节点下（stamp.ts:64-81），材料节点是本次全部新节点的祖先。新建图走 fillRoot 分支时 `nodeId = root` 且文本已填；根缺失的损坏图走新建材料节点分支，仍取本次文本。四种场景（草稿 / 历史消息 / 续写图 / 恢复图）都不依赖根。
- **任务书第 2 条（旧画布语义严格不变）**：`originalRequestOf` 本体未动（context.ts:156-158）；`DispatchOverlay.tsx` 不在改动清单，三处调用已逐一读过，原样保留；`context.test.ts:55-61` 新增钉住测试（后续用户消息不改变根语义）。
- **任务书第 3 条（共享上下文保留延续材料）**：`serializeContext`/`contextPartsOf` 无「原始需求」独立字段，按祖先闭包序列化（context.ts:190-228）；测试断言第二次执行节点 prompt 含 A、A 的执行回复、B（console-request-30.test.ts:130-134）。
- **盖章前固定 / 运行中编辑一致**：`runRequest` 在 stamp 前固化；修正轮经同一 `buildRequest`（dispatch.ts:1454 是流水线唯一请求构建点，循环内反复调用）；channel 侧 `mergeConsoleCommit` 合并并发编辑也只影响 context 序列化，不影响已固化的 `originalRequest`。
- **调用点无遗漏**：生产中其余 `originalRequest` 来源只有 `dispatchConsoleTurn`（console-chat.ts:206，单模型直聊，本就取本次 text）和 `classifyProposal`（console-channel.ts:264，取 event.text），均为本次需求，无需改。
- **测试走真实接线、可红**：测试未注入 `dispatchPreset`（console-chat.ts:529 的 `options.dispatchPreset ?? dispatchConsolePreset` 走真实函数），通道测试走真实 `runConsoleDispatch` + 假 io/假 store；第二次运行断言两条盲节点 `originalRequest` 为 B 且 ≠A。红测逻辑成立：当前两处装配中 `originalRequest` 唯一来源是 `runRequest` 常量，去掉实现退回根读取即第二次 = A，与报告红测输出一致（未独立复跑，此条为推理）。
- **验证复跑声明**：`cargo check/test`、vitest 全量、`npm run build`、`copy-audit`、`git diff --check` 均未在本会话重跑（无 shell 工具）；copy-audit 所述命中行「归类模型判定不必派发」确在 console-channel.ts:226 且非本次改动。

**限制**：本审查为纯静态。`context.ts` 除新增 `runRequestOf` 及注释外是否还有其他行级改动无法逐字节确证（无 git diff 手段），但其既有行为由该文件原有测试群钉住，且报告称全量 656 例通过。
