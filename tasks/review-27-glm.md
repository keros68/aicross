# TASK-27 首版审查 · GLM-5.3-Flash（cc-switch，只读）

## 总判

**可交付但有未闭合项**。任务书两条主线（详情页、收尾结论）已真实接线且主干判定正确；遗留 1 条结构性隐患（订阅/收尾无兜底）、1 条心跳写盘放大、1 处结论「轮」口径的边界偏差，均不阻塞主流程。

## 问题清单

**1.（中低 · 结构隐患）`console-chat.ts:329-459` — 心跳订阅与运行记录收尾没有 try/finally 兜底**
订阅在 329 行建立，`dispose()` 在 436 行、`finishConsoleRun` 在 439 行，中间约 110 行代码无异常兜底。一旦这段抛错：订阅泄漏（`subscribeProgress` 挂的是全局 `dispatch-progress` 监听，`dispatch-progress.ts:80-96`，泄漏到重启为止），运行记录永远停在 `running`——本会话内阶段/收尾消息按钮一直显示「查看进度」、详情页行一直走表，直到重启才被 `interruptConsoleRun` 标「已中断」。
**现实触发面**（核实后）：比预想窄。`startPipelineHeadless` 把 `runStageLoop` 的抛错全部折进 `pipeline.error`（`dispatch.ts:1504-1515`）；`io.saveGraph`、`persistConsoleSessions` 各自 catch；剩余可抛点只有 `commitGraph`/`append` 两个 React dispatch（reducer 抛错才传播）以及 `startPipelineHeadless` 自身 catch 块里的 commit/onResults。当前代码我没有找到必然触发的路径，属于「现在难触发、后续改动一脚踩空」的脆弱结构。
**复现**：人为让 `commitGraph` 或 `feedback.append` 抛错即可观察记录卡在「进行中」。
**改法**：329 行起包 try/finally，finally 里 `progressSubscription?.dispose()`；catch 里先 `finishConsoleRun(record, graph, [], String(error), now())` 并 `onRunUpdate` 后再 rethrow（`createConsoleApplyHandler` 的 `.catch` 已负责展示错误消息）。

**2.（中低 · 写盘放大）`console-chat.ts:329-333` + `app-state.tsx:775-779` — 每条进度事件都触发全量同步持久化**
进度回调无条件 `updateRun(...)`：即使 `withConsoleRunProgress` 因 runId 不匹配/节点不在记录而原样返回同一引用，仍会 dispatch `UPSERT_CONSOLE_RUN`；该 action 在 `CONSOLE_PERSIST_ACTIONS` 里，每次触发对**全部**控制台会话做同步 `JSON.stringify` + `localStorage.setItem`（`app-state.tsx:775-779`、`console-persistence.ts:76-85`）。消息正文含完整模型输出，单会话可达 200 条，序列化体积可能到 MB 级。本路心跳 15s/节点已是稳定写盘（并发 C 路 ≈ 每 15s 写 C 次）；旧画布同时在跑派发时，其心跳也被这台订阅接住、每条白写一次。
**改法**：回调里先判 `next === runRecord` 则跳过 dispatch；或把 `UPSERT_CONSOLE_RUN` 改为防抖持久化。实际卡顿幅度未实测（本环境无 shell），严重度标注为推测成分。

**3.（低 · 边界口径）`console-run-detail.ts:211-227` — 结论的「轮」按验收行去重 stage 数计，两类场景会偏**
- 续聊场景（`existingGraph` 非空）新节点挂在深度不同的旧节点上，同批 `request.stage` 取 min（`console-chat.ts:343`），行 stage 可能失真，进而轮数计数失真（仅影响详情页分组与「第 N 轮」措辞）。
- 更实质的：两条**不等深**分支各有验收、浅分支 NEEDS_FIX 且修正轮已到顶（`appendCorrectionRound` 不再生长，`dispatch.ts:758-828`）、深分支 PASS——最后一 stage 只含深分支行 → 判「通过」，浅分支的 NEEDS_FIX 被无视。与任务书「多个验收类节点任一 NEEDS_FIX 即未通过」的宽读法相悖；若按「最后一轮内任一」的窄读法则实现相符。**「轮」的口径本身在任务书里有歧义，此项标不确定**；触发前提苛刻（需不等深并行验收分支）。
**改法**：未通过判定不限定在最大 stage，改为「全部验收行中存在 NEEDS_FIX 且该验收链无更晚的复判结果」。

**4.（低 · 测试）`console-run-detail-27.test.ts:99-160, 255-266`**
- 真实接线格用 2 行覆盖了任务书要求的 3 个条件（token 为 0 的那路就是 NEEDS_FIX 那路），条件覆盖是全的，但与「一路 token 为 0、**一路**切备选、**一路** NEEDS_FIX」的三路读法不完全一致。
- Esc/满版格是 `readFileSync` + `toContain` 的静态源码断言，不验证实际行为。本仓 vitest 跑 node 环境无 jsdom（`focus-enforcer.ts` 注释可证），静态检查是既有折中，但「Esc 能退出」这件事实际未被测试证明。
- 报告的红测清单与测试结构相容（删掉实现模块时 import 即整文件红），但红性我无法独立复跑（本环境无 shell），**此点未验证**。

**5.（nit）`console-run-detail.ts:98-104`** — `createConsoleRunRecord` 的 `_graph`、`_nodeIds` 是占位死参，签名里没有用。
**6.（nit · 口径）`console-run-detail.ts:134 + 259-265`** — 阶段开始到首条心跳之间，进行中行计时从 `progressAt`（阶段派发时刻）起算，含排队/等锁时间；首条心跳后被引擎 `elapsedMs` 纠正。轻微波动，可接受。

## 核过没问题的要点

- **订阅与串扰**：订阅仅在 `feedback.onRunUpdate` 存在时建立；`withConsoleRunProgress` 按 runId 挡外部事件，不会把画布派发的进度写进控制台记录（写盘副作用见问题 2）。
- **前置校验不留半截记录**：资源/目标/额度/请求数/脏工作区等 throw 全部发生在建记录（`console-chat.ts:320-328`）之前。
- **正常失败路径都会收尾**：`startPipelineHeadless` 把 dispatch/preview reject、duplicate batch、missing-target、请求上限都折成 `pipeline.error`，最终走到 dispose + `finishConsoleRun` →「中途停止（原因：…）」。
- **结论判定主干正确**（`console-run-detail.ts:211-253`）：只取最大 stage 的验收行；NEEDS_FIX 优先 → 未通过；非 PASS（含 UNKNOWN、验收节点本身失败）→「验收结论无法识别」；全 PASS →「通过（第 N 轮…）」；无验收行 →「已完成（本模式不含验收）」；有 error → 中途停止优先于验收结论。修正轮走 `appendCorrectionRound` 生成**新**节点、深度 +2（`dispatch.ts:596-642`），前一轮的 NEEDS_FIX 不会污染最后一轮判定，轮数计法与之一致；测试 232-248 钉了通过/无法识别/未通过/中途停止/无验收五种。
- **收尾文案**：两句结构、`共 N 路，运行失败 M 路，用时 X 秒`与任务书示例一致；「运行失败」按 nodeId 去重后统计（`console-chat.ts:443-449`）。
- **token 口径**：完成/中断行 0/0 →「未报告」，不计入输入/输出/缓存合计，全局与阶段小计都注明「N 路未报告，未计入」（`console-run-detail.ts:255-257`、`ConsoleRunDetail.tsx:23-27,116-118,142`）；测试 164-176 钉住。
- **两种耗时**：墙钟 = finishedAt − startedAt，各路耗时和 = 逐行 `liveConsoleRunDurationMs` 之和，并列展示且有一句口径说明（`ConsoleRunDetail.tsx:114-120`）；进行中行两次心跳间由前端 1s 时钟走表（`ConsoleRunDetail.tsx:78-83` + `liveConsoleRunDurationMs`），测试 217-230。
- **持久化**：同一 localStorage 键加字段；旧数据无 `runRecords` → `[]`（测试 192-198）；20 份上限在 `persistedSession`/`loadConsoleSessions`/`UPSERT_CONSOLE_RUN` reducer 三处生效（测试 200-209）；重启加载经 `interruptConsoleRun` 把 running 记录与行冻结为「已中断」并补齐墙钟/耗时（测试 178-190，且重启断言走真链路 `getInitialState → loadConsoleSessions`，`app-state.tsx:872-876`）；坏记录被拒但不拖垮会话（测试 250-253）；消息 `runId` 随持久化保留（`console-persistence.ts:52`），被 20 份上限淘汰后按钮自动隐藏（`ConsoleChat.tsx:201`）。
- **详情页交互**：整体替换 ConsoleChat 根节点、`height:100%`，占满中间区域；Esc 在 document 捕获阶段 `preventDefault + stopPropagation`；`role="dialog"` + `aria-modal="true"` 命中焦点守卫的两条豁免（`focus-enforcer.ts:22-33`），终端焦点守卫不会抢焦点；「返回对话」与 Esc 同走 `onClose` 复位。
- **范围与文案**：git status 快照中无 `src/**`、`tauri.ts`、`graph/context.ts`、`graph/types.ts`、`graph/dispatch.ts`、`DispatchOverlay.tsx`，禁区未碰；详情页与结论文案无美元、无额度百分比，口径中性简短。
- **无法核实的报告项**（如实说明）：`cargo check/test`、vitest 全量、`npm run build`、`copy-audit` 零新增、换行保真——本审查环境无 shell，未复跑，仅确认报告描述与代码现状不矛盾。
