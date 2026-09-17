# TASK-29 首版审查 · GLM-5.3-Flash（cc-switch，只读）

# TASK-29 首版审查

**总判：可交付** —— 86/87/88 三条的实现在代码层面均核实成立，未发现会导致漏计/多计或行为回归的缺陷；下列编号项均为低严重度或报告/范围外事项，不阻塞交付。

（说明：本环境只读、无 shell，我未能独立重跑 vitest/cargo/build，报告中的运行结果未复核；以下结论全部来自对当前代码的静态核实。也无法对 HEAD 做逐行 diff，以下「未改动」判断基于调用点与签名核查。）

## 问题清单（按严重度）

1. **【低·报告准确性】REPORT-29.md:38 — 红测观察值与按现有代码推算不符。**
   报告称未加实现时第二次系统提示是全图阶段 `['4', '5']`。但按现有测试与图结构推算：第一次 apply 在新图上会正常跑一轮修正（`acceptanceRound` 对新图首个验收计 1 ≤ 1，`dispatch.ts:834`），第一次运行占深度 1–4；第二次 apply 的材料节点挂在最深节点下（`console-channel.ts:73` latestLeaf），新执行/验收节点全图深度应为 **6、7**，红态消息应为 `['6','7']`。`['4','5']` 恰好对应「第一次运行没跑修正轮（验收 PASS 即收尾，深度只到 2）」的测试变体。疑似报告记录的是早期版本的测试观察。复现：按当前测试文件重跑红态即可对证。改法：核对后更正报告此段；不影响实现本身，也不影响「能红」的结论（阶段号、请求数 2≠4、结论文案、缺函数四处都足以红）。

2. **【低·边界】ConsoleChat.tsx:69-71 与 73-82 — 关闭详情与新消息同一渲染帧到达时，恢复会被 scrollIntoView 覆盖。**
   恢复在 `useLayoutEffect`（先于 paint）执行；line 69 的 `scrollIntoView` 是 passive effect，若「返回」点击与新消息 append 恰好被 React 18 合批到同一渲染（`messages.length` 在关闭帧变化），它会在恢复之后执行并把消息流拉到底。复现条件苛刻（消息到达与点击同帧），后果是一次性跳底、用户可自行滚动。改法（可选）：恢复时置一个短暂标记，让 line 69 的 effect 在下一帧内跳过一次。

3. **【低·既有行为继承，非本次引入】console-chat.ts:389-392/427-428 — 同批节点跨图深度时展示阶段取批内最小值。**
   `nextStageStep`（dispatch.ts:128-142）不按深度分组，一批可能含不同深度的节点；`onStage` 给整批节点写同一个展示阶段。经济类预设不会触发（各批同深度）。改动前的 `withConsoleRunStage` 同样用 `request.stage`（全图最小深度）写整批行，粗糙度一致，不属回归。仅记录，不需本期处理。

4. **【范围外观察，建议记 backlog】console-channel.ts:439-442 — 外部通道路径未传 `fixRoundNodeIds`，仍按全图计修正轮。**
   该路径同样用 `skip` + 续写图的模式；若外部连续两次事件续写同一张图，会复现 86 的「一轮修正不跑直接到顶」。TASK-29 明确只做三条、未列此路径，属范围外；是否真的存在跨事件续写同一图的场景我未核实（不确定）。

5. **【说明，判定为符合意图】console-run-detail.ts:334-339 — `interrupted`/`failed`/`timeout` 行的「实际模型」也显示「未报告」。**
   任务书字面是「只有已完成且没拿到实际模型才显示未报告」；已结束但失败的行显示「未报告」语义上成立（确实没拿到），进行中/排队显示「—」已满足。kimi 口径（含 fallback 后的 agent 判定）保留正确。仅指出字面与实现略有出入。

## 核过没问题的要点

- **fixRoundNodeIds 集合完整性（审查重点 1）**：
  - 初始集合 = `newNodeIds`（console-chat.ts:290,418），即本次 `stamp` 新建的全部节点。`stamp`（graph/stamp.ts:66-82）只新建、从不复用旧节点，材料节点产生于 stamp 之前故不在集合内——初始派发集与集合严格一致。
  - 推进中生长：`runStageLoop` 在每次 `reducePipelineStep` 前快照、之后把新增节点补入集合（dispatch.ts:1474-1487）。修正轮的执行+验收+验证兄弟节点（dispatch.ts:596-642）、验证重试的执行+验收（dispatch.ts:655-714）、遗留节点全部经此路径补入，无遗漏。
  - 重派/恢复：再次 apply 重新 stamp，旧节点（含上次遗留的 draft 验收）全部进 `skip`（console-chat.ts:417），既不会被派发（不会产生新的 NEEDS_FIX），也不在本次集合内（不占额度）。上限=1 时第二次 apply 的实际序列：执行→验收(计1)→派修正执行→验收(计2>1)→遗留+收尾，与 86 要求一致。
  - 无多计路径：`acceptanceRound` 只数「验收」角色祖先（dispatch.ts:749），集合里的执行/验证/遗留节点不计数；同批两个 NEEDS_FIX 的互不污染（祖先链不相交于待计节点）。
- **旧画布语义严格不变**：`DispatchOverlay.tsx:392` 与 `console-channel.ts:439` 均不传新参数；`acceptanceRound`/`reducePipelineStep`/`runStageLoop`/`startPipelineHeadless` 的新参数全部可选，undefined 时逐分支与原逻辑等价（dispatch.ts:749 条件在无 scope 时恒真；`startPipelineHeadless:1522` 仅在有参时复制集合）。dispatch.test.ts:344-368 成对断言「不传=全图封顶 / 传=本次独立」两种语义；console-channel.test.ts:48-50 钉住接线。
- **三处展示阶段号一致（审查重点 2）**：系统提示 `onStage` 用 `displayStage(request.stage)`（console-chat.ts:427）；详情页 rows 全部经 `displayResults` 换算——`withConsoleRunStage` 收到的是已换算的 request（:429），结果与收尾都过 `displayResults`（:394,470）；收尾结论 `acceptanceConclusion` 用 row.stage 数轮次（console-run-detail.ts:255-271）。引擎请求 stage 保持全图深度（console-chat.ts:366,382），符合任务书「只在展示层换算」的选项且报告已写明。首次 apply（firstRunStage=1）展示号与原行为恒等。控制台 UI 无其他展示全图深度的遗漏点（DispatchOverlay 结果屏的原始 stage 属旧画布，应保留）。
- **详情页（审查重点 3）**：单一 `RUN_TABLE_COLUMN_WIDTHS`（和=1120=min-width）经 colgroup 应用到所有阶段表，`table-layout: fixed`；`.console-run-table-wrap` 独享横向滚动，`.console-run-body` overflow-x:hidden、根 overflow:hidden（ConsoleRunDetail.tsx:21,136-140；ConsoleChat.css:181-348）。页头 `consoleRunHeaderLabel` 为「模式名 · 本地日期 时间：分」，无 runId（console-run-detail.ts:132-143）。视觉效果未做窗口验收——报告已如实声明，任务书对此条只要求说明做法。
- **滚动恢复时序（审查重点 4）**：纯函数语义正确（底部判定 2px 容差；底部+新消息→最大值；否则夹取到当前最大，console-run-detail.ts:145-159）。关闭详情时 chat 树在同一渲染内重挂载，`messageListRef` 在 layout effect 执行前已就位、先于 paint；详情页打开期间到达的消息不会触发中途恢复（openRunId 真值时早退）。主路径时序成立（同帧竞态见第 2 条）。
- **测试真实性与可红性**：console-per-apply-29.test.ts 经 `createConsoleApplyHandler` 进入**真实** `dispatchConsolePreset`（未注入 `dispatchPreset`），同一 `existingGraph` 连续两次假派发，断言同图 id（续写保留，满足要求 3）、第二次阶段消息 1–4、secondRunId 下 4 次请求（证明修正确实派出）、结论文案与 rows 阶段；另有三条单测覆盖「—」/「未报告」、页头、滚动纯函数。node 环境无 jsdom。测试断言在缺实现时不红则不可能通过报告所述的最终断言，红的方向均可复核（唯报告记录的红态数值存疑，见第 1 条）。
- **约束遵守**：改动文件清单与任务书范围一致，`src/**`、backlog、docs/PRODUCT.md 未动（与 git status 吻合）；报告如实声明未做窗口手测、未 commit/push。
