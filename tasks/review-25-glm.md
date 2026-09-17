**总判：可交付但有未闭合项** —— 核心接线（阶段/结果回流、去重、收尾时序）与持久化均成立且测试走真实接线；主要未闭合项是续聊失效回退把普通失败也判为失效（清掉有效会话并自动重发一次），另有三个低severity项。

说明：本会话没有 shell，我无法重跑测试/构建；以下基于源码静态审查、会话开始时的 git status 快照与 REPORT-25 的自述。测试与构建结论以报告为准，未独立复核。

---

## 问题清单（按严重度）

**1.【中】续聊回退把一切普通失败都当成「会话失效」，清掉有效 resumeId 并自动重发一次**
- 位置：`src-ui/src/console-chat.ts:189-206`（`dispatchConsoleTurnWithFallback`）
- 问题：回退条件是「抛异常 **或** `result.status === 'error'`」。后端把 CLI/供应商层失败统一记为 `status: "error"`（`src/dispatch.rs:3858` 默认 `"error"`，:2968、:4052、:4100 同）——网络断、供应商 5xx、CLI 崩溃、无效 resume 与「session 不存在」在结果上不可区分。`listDispatchableModels` 失败（`读取资源失败`）、模型不在清单（`console-chat.ts:160-162`）抛的异常同样落入 catch。
- 复现：正常续聊中供应商一次 5xx → 结果 `status:'error'` → 走 `onExpired()`：有效 resumeId 被清（`ConsoleChat.tsx:107-114`）、插入「上次的会话已失效，已开新会话」（与事实不符）、随后以空 sessionId 把同一条 prompt 全新重派一次（第二次计费请求；若真是网络问题，第二次同样失败，用户看到「已失效」+「失败」两条矛盾提示）。
- 改法：只对能确认是会话失效的信号回退——对 `result.error` 匹配 CLI 的会话不存在文案（如 "session not found"/"No session found" 类签名）再回退；其余 `error`/异常原样返回给上层按普通失败显示。

**2.【低】流水线「中途停止」的错误原因不回流，未跑节点无任何说明**
- 位置：`src-ui/src/console-chat.ts:356-363`（收尾消息不含 `pipeline.error`）、`src-ui/src/components/center/ConsoleChat.tsx:158`（`onOutcome: () => {}`）
- 问题：`startPipelineHeadless` 的 `stopped` 分支（修正轮撞总请求上限「已达总请求上限 N」、重复批次 `DUPLICATE_BATCH_TEXT`、missing-target）不产生结果条目也不走 `onError`（`graph/dispatch.ts:1506`），错误串只放进返回值；生产接线里 `onOutcome` 是空函数，收尾消息只报「成功 X 路，失败 Y 路」，停止原因对用户完全不可见——正是本期要消除的「界面无解释」的缩小版复发。修复轮触发上限是现实可发生的路径（修正轮会增加请求数）。
- 复现：设置较小 `maxRequests`，两阶段 + 修正轮派发 → 第二次 `nextStageStep` 返回 `limit` → 消息流只有各阶段消息和「成功/失败」收尾，无上限说明。
- 改法：收尾消息在 `pipeline.error` 非空时附加原因（或在 `onOutcome` 里 append 一条系统消息）。

**3.【低】收尾消息之前有一个未保护的 `await io.saveGraph(graph)`，落盘失败会吞掉收尾**
- 位置：`src-ui/src/console-chat.ts:355`
- 问题：流水线成功后先 `await io.saveGraph(graph)` 再 append 收尾消息。`saveGraph` 拒绝（磁盘错误等）时整函数抛出，收尾消息不发，用户只看到 catch 里的保存错误；同函数 `commit()` 内部对同一命令是 fire-and-forget 加 catch（:264），两处口径不一致。
- 改法：改成与 ：264 相同的 `void io.saveGraph(graph).catch(...)`，或 try/catch 后仍发收尾。

**4.【低】同一节点修正轮重跑后，消息流显示首次状态、收尾按末次状态计数，两者可能矛盾**
- 位置：`src-ui/src/console-chat.ts:357`（`new Map(...)` 去重取**末次**）对比 ：319（`announcedResults` 取**首次**）
- 问题：修正轮同 nodeId 会二次出现在 `collected`；节点消息只播首次（如「验证命令未通过」），收尾的「成功 X 路」按末次（修好后成功）统计。计数与消息流对不上。任务书只要求按节点 id 去重播报，属边角口径问题。
- 改法：收尾统计与播报同口径（按首次或按末次统一），或在末次结果与首次不同时补一条更正消息。

---

## 核过没问题的要点

1. **累积结果去重**：`onResults` 用 `announcedResults: Set<nodeId>`（`console-chat.ts:316-329`），引擎每阶段传 `[...state.collected]`（`dispatch.ts:1466`），第二阶段的累积回调不会重复插消息；定向测试里阶段 2 的累积回调真实流过，断言 `'第一阶段正文'` 全流恰一条。
2. **失败节点说明**：`resultStatusLabel`（:114-119）覆盖 超时/额度用尽/真身不符/验证命令未通过/失败，`原因：` 取 `error || runnerNote || text || '未返回原因'`（:135），无静默跳过；状态词与 MANUAL.md 词汇表（额度用尽、真身不符、验证命令未通过）一致。
3. **备选作答**：`consoleResultMessage` 读 `fallbackTo`/`modelReported` 显示实际作答方，单线与模式路径共用同一函数，口径天然一致（该函数与 `ConsoleMessage.status/route` 字段为 TASK-22 已有，`console-session.ts` 本期未改）。
4. **收尾与「处理中」时序**：收尾消息在 `dispatchConsolePreset` 返回前 append 且每次调用仅一条（:356-363）；`setBusy(false)` 在 handler 的 `.finally`（:419），promise settle 晚于收尾 append；入口 `busyRef` 拦截并发二次应用。
5. **引擎未动**：会话起始的 git status 快照中 `src-ui/src/graph/dispatch.ts` 不在改动清单；调用点按原签名 `startPipelineHeadless<DispatchNodeResult, DispatchStageRequest, unknown>` 注入 deps（:294-330）。
6. **持久化**：存 `localStorage` 键 `aicross:console-sessions:v1`，挂在与既有偏好同一个 `getInitialState` 读 + `reducer` 写链路上（`app-state.tsx:755-759, 852-856`），无新增状态容器或 Rust 存储；此前仓里本就没有整份 terminals 恢复机制，不算另起一套。读、写、`JSON.parse` 三处都 try/catch 吞掉（`console-persistence.ts:69-112`）；200 条上限在 append、落盘、恢复三层都用 `slice(-N)`，测试验证 205 条恢复后首条是 `m5`（丢最旧）。「处理中」是 `ConsoleChat` 组件本地 `useState/busyRef`（`ConsoleChat.tsx:33-34,195`），不在 `TerminalSession` 上、不落盘，重启后不会卡处理中（中途重启只是缺收尾消息，可正常续用）。
7. **失效回退的测试面**：测试走真实 `dispatchConsoleTurnWithFallback` + 假 io，断言 `['dead-thread','']` 两次派发与 `expired` 恰一次——接线真实、可红（报告记录了撤实现即抛 `session not found`）；只是它固化了第 1 条的宽口径，未覆盖误判场景。
8. **测试走真实接线**：回流用真实 `dispatchConsolePreset` + 假 io 断言 5 条消息的顺序与内容；持久化走真实 `reducer` + 真实 `getInitialState` 模拟重启（与 App 的 `useReducer(reducer, undefined, getInitialState)` 同链路）；事件层测试（`console-chat-22.test.ts:129-183`）断言第 7 参 `append` 真被传入。无同义反复；报告的 4 条「先红」记录与测试结构对得上。未覆盖项：`quota_exhausted`/`identity_mismatch` 标签、异常路径下活跃节点的合成失败消息，仅有实现无测试（低风险）。
9. **文案**：新增文案（「第 N 阶段 · 派出 M 路：…」「模式派发结束：成功 X 路，失败 Y 路，总耗时 Z 秒。」「上次的会话已失效，已开新会话」）均为中性短说明体，符合任务书指定措辞；MANUAL.md:60-62 已替换「关闭 tab 或重启后不保留」为真实行为（含 200 条上限与失效回退），表述与实现一致。