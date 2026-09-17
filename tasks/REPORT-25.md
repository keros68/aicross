# REPORT-25 控制台进度回流与会话持久化

## 结果

TASK-25 已按任务书第 1、2 节完成。模式派发的阶段与节点结果会在运行期间进入控制台消息流，收尾消息写入后才解除“处理中”；未关闭的控制台 tab 会在 dev 热更或应用重启后恢复消息、续聊 id、当前目标、已派发标记和对话图 id。

未修改派发引擎、`startPipelineHeadless` 签名或派发记录格式；未启动窗口、发送真机请求或调用真实模型。

## 1. 接上 `onStage` 与 `onResults`

- `dispatchConsolePreset` 的 `onStage` 现在按阶段号去重，并写入“第 N 阶段 · 派出 M 路：角色（agent / model）”系统消息。
- `onResults` 按节点 id 处理累积结果；一个节点只在第一次出现时写入一条回复。回复携带实际 agent、模型、角色、耗时和正文。
- 备选结果读取 `fallbackTo` 与 `modelReported`，显示实际作答方；主目标信息仍保留在备选说明中。
- 超时、额度用尽、真身不符、验证失败与普通失败均写明状态和原因，不静默跳过。
- 流水线结束后写入成功路数、失败路数和总耗时，再由既有 `finally` 解除“处理中”。
- 应用模式事件把控制台的 `append` 接到 `dispatchConsolePreset`，测试同时覆盖事件层到反馈参数的真实接线。

## 2. 控制台会话持久化

- 沿用 `app-state` 初始化、reducer 与 localStorage 的现有恢复链路，新增控制台负载键；没有新增 Rust 存储或另一套状态容器。
- 每个控制台 tab 保存 `messages`、按 agent/provider/model 区分的 `resumeId`、`targetAgent`、`lastAppliedMessageId` 和 `consoleGraphId`；恢复图 id 后通过现有 `loadGraph` 命令载入图。
- reducer 在控制台负载或 tab 骨架变化后同步更新存储。存储读取失败、写入失败或 JSON 损坏时退回内存状态，不向当前运行抛错。
- 内存与持久化消息都限制为每个 tab 最近 200 条，超过后丢弃最旧消息。
- 单线续聊使用旧 id 报错或返回普通错误状态时，先清除该目标的旧 id，插入“上次的会话已失效，已开新会话”，随后以空 `sessionId` 向同一目标重派一次。
- `docs/MANUAL.md` 已更新为当前恢复、关闭 tab 删除记录、200 条上限与失效续聊回退行为。

## 关键测试先红

定向命令：

```text
cd src-ui && npx vitest run --configLoader native --pool threads src/__tests__/console-feedback-persist-25.test.ts src/__tests__/console-chat-22.test.ts
```

实现后逐项临时撤掉对应接线并运行定向用例，得到以下真实失败；每次记录后立即恢复实现：

| 范围 | 临时撤掉的实现 | 实际失败 |
|---|---|---|
| 阶段与结果回流 | 把 `dispatchConsolePreset` 的 `onStage` / `onResults` 恢复为空函数 | 消息种类实际只有 `['system']`，期望阶段、两条节点回复和收尾共 5 条；退出码 1。 |
| 事件到反馈参数 | 不把 `{ append: options.append }` 传给模式派发 | “进度已回流”断言实际为 `false`，期望 `true`；退出码 1。 |
| 持久化 | reducer 不调用 `persistConsoleSessions` | 模拟重启后得到新的空控制台，`messages` / `resumeId` / `targetAgent` / 标记 / 图 id 均未恢复；退出码 1。 |
| 失效续聊回退 | 续聊失败后不清除并以空 id 重派 | 定向测试直接抛出 `Error: session not found`，没有第二次空 `sessionId` 请求；退出码 1。 |

全部临时故障已恢复。恢复后定向结果：

```text
Test Files  2 passed (2)
Tests  17 passed (17)
```

## 最终验证

以下命令均由本次实现直接运行。

### `cargo check --all-targets`

```text
Compiling aicross v0.1.0 (D:\work\aicross-p6)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 23.18s
```

退出码 0，零 warning。

### `cargo test`

```text
running 470 tests
test result: ok. 459 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 23.71s
```

11 项 ignored 为仓库既有的真机、真实模型或本地事实源测试，本次按任务要求未启用。

### `cd src-ui && npx vitest run --configLoader native --pool threads`

```text
Test Files  41 passed (41)
Tests  610 passed (610)
Duration  1.20s
```

退出码 0。控制台的 `NO_COLOR` / `FORCE_COLOR` 提示为既有环境提示，不是测试失败。

### `cd src-ui && npm run build`

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build
✓ 348 modules transformed.
✓ built in 628ms
```

退出码 0。Vite 仍提示主 chunk 超过 500 kB，为仓库既有构建提示，本期未扩大处理范围。

### `python scripts/copy-audit.py`

```text
共 6 处命中，5 个文件（含误报，需人工复核）
```

退出码 0。命中仍位于 `DispatchOverlay.tsx`、`SettingsModal.tsx`、`console-channel.ts`、`stamp.ts` 和 `resources/types.ts`，均不是本期新增文案。

## 文件与边界

- 修改：`src-ui/src/console-chat.ts`、`src-ui/src/components/center/ConsoleChat.tsx`、`src-ui/src/store/app-state.tsx`、`src-ui/src/__tests__/console-chat-22.test.ts`、`docs/MANUAL.md`。
- 新增：`src-ui/src/console-persistence.ts`、`src-ui/src/__tests__/console-feedback-persist-25.test.ts`、`tasks/REPORT-25.md`。
- 未修改 `src-ui/src/graph/dispatch.ts`、`src/server.rs`、`src/main.rs`、`src-ui/src/tauri.ts` 或 `CenterPanel.tsx`。指定文件仍分别保持 CRLF、LF 或原有混合换行。
- 未运行 `cargo fmt`，未 commit，未 push。
