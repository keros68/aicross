# TASK-25 控制台：编排进度回流 + 消息持久化（09-17）

补齐 TASK-22 第一期的两个缺口，都是 09-16 夜编排者窗口实测发现的（backlog 75、76）。用户今天实测，这两条是他**第一个会撞上**的。

## 背景（编排者已核实根因，直接采信）

### 缺口一：编排派发时消息流里没有任何结果（backlog 75）

实测：控制台里输入一句话、选经济模式点「应用」，流水线在后台正常推进（`.dispatch/STATE.md` 显示跑到第 5 阶段、留痕落了出方案 / 执行 / 验收 / 验证等六七条），**界面自始至终只有一行「处理中…」**。

**根因已定位**：`src-ui/src/console-chat.ts` 的 `dispatchConsolePreset` 调 `startPipelineHeadless` 时，传进去的回调是：

```ts
onPreview: () => {},
onStage: () => {},
onResults: () => {},
```

而引擎 `src-ui/src/graph/dispatch.ts` 在每个阶段开始时调 `deps.onStage(stageRequest, ids)`（:1451），结果到了调 `deps.onResults([...state.collected])`（:1466、:1513）。**引擎一直在报进度，只是控制台没接。** 这正是 TASK-22 任务书第 3 条写明要做（「派发结果以消息形式回到控制台消息流里，每一路一条，带 agent / 模型 / 角色 / 耗时」）却没落地的部分。

### 缺口二：消息与续聊 id 不持久化（backlog 76）

实测：改一次样式触发 dev 重建，控制台里刚聊的对话全没了。应用重启同理。`docs/MANUAL.md` 目前写着「关闭 tab 或重启后不保留」。

## 要做的

### 1. 接上 `onStage` 与 `onResults`

- `onStage(request, ids)`：插一条**系统消息**，形如「第 N 阶段 · 派出 M 路：出方案（pi / glm-5.3-flash）」。同一阶段只插一次。
- `onResults(collected)`：`collected` 是**累积**的全量结果，不是增量——**必须按节点 id 去重**，每个节点只在它第一次出现时插一条回复消息，带 agent、模型、角色、耗时、正文。失败 / 超时 / 额度用尽的节点也要插一条，写明状态与原因，不要静默跳过。
- 备选作答时（结果带 `fallbackTo`）显示实际作答方，与单线路径的做法一致。
- 流水线结束时插一条收尾系统消息（成功几路 / 失败几路 / 总耗时），然后才解除「处理中」。
- **不要改引擎**（`graph/dispatch.ts` 的 `startPipelineHeadless` 及其回调签名），只在控制台这一侧接线。

### 2. 控制台会话持久化

- 每个控制台 tab 的 `messages`、各家续聊 id（`resumeId`）、`targetAgent`、已派发标记、`graphId` 存下来，应用重启或 dev 热更后恢复。
- 存储位：先看现有 tab / 会话是怎么跨重启恢复的（09-16 实测应用重启后 demo tab 是恢复的），**沿用同一机制**，不要另起一套。若现有机制只存 tab 骨架，就在同一处把控制台负载一起存。
- 存储不可用时（隐私窗口、配额满）只掉持久化，不影响本次运行，不抛错。
- 消息条数设上限（例如每 tab 最近 200 条），超出丢最旧的，避免无限增长。
- 恢复后续聊 id 可能已失效（对方 CLI 会话被清理）：下一条消息若续聊报错，**不要让用户卡住**——退回全新会话并插一条系统提示「上次的会话已失效，已开新会话」。
- 更新 `docs/MANUAL.md` 里「关闭 tab 或重启后不保留」那句为新的真实行为。

## 不做

- 不改派发引擎、不改 `startPipelineHeadless` 签名、不改留痕格式。
- 不做对话图降级、不做 agent 窗口入口（第二期其它条目）。
- 不做跨设备同步。

## 验证

`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`；跑 `scripts/copy-audit.py`。

**本仓连吃了三次「只测纯函数漏接线」**，所以关键测试必须走真实接线：
- 用假 io 驱动 `dispatchConsolePreset` 跑一条两阶段流水线，断言消息流里依次出现：阶段系统消息、每个节点**恰好一条**回复（`onResults` 被调多次时不重复）、失败节点有状态说明、收尾消息。
- 持久化：写入 → 模拟重启（清内存态后从存储恢复）→ 断言 messages / resumeId / targetAgent 都回来；存储抛错时不抛；超过上限丢最旧。
- 续聊 id 失效的回退：假 io 让续聊报错 → 断言退回新会话并插了提示。

每条先确认去掉实现会红，并在报告里写清怎么红的。

保持文件原有换行（`src/server.rs`、`src/main.rs` CRLF，其它 Rust LF，`CenterPanel.tsx` 原有 CRLF，`src-ui/src/tauri.ts` 混合只做字节级最小改动）；不 `cargo fmt`；不起窗口、不发真机请求、不调真实模型；不 commit、不 push。报告写 `tasks/REPORT-25.md`，逐条对应 1、2。
