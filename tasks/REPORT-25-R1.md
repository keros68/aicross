# REPORT-25-R1 控制台进度回流与持久化修正轮

## 结果

`fix-25-r1` 的四项必修已完成。续聊只在明确的会话失效信号下回退；模式派发会补齐未播报结果、显示停止原因和末次状态更正；最终保存失败不再吞掉收尾消息。

保持项未改：累积结果仍按节点去重，失败节点仍显示状态与原因，备选作答仍显示实际作答方，收尾仍先于 `setBusy(false)`；未修改派发引擎、持久化链路、200 条上限或“处理中”状态。

未启动窗口，未发送真机请求，未调用真实模型；未运行 `cargo fmt`，未 commit，未 push。

## 1. 续聊回退只识别会话失效

- 新增带局限说明的纯函数 `isExpiredResumeError`。错误文本必须同时包含会话类词 `session / thread / conversation / resume` 和失效类词 `not found / no such / expired / invalid / unknown / does not exist / cannot resume / could not resume` 才视为会话失效。
- 将资源读取、目标检查和请求构造移到回退判断之外。发派前异常原样抛出，不清除续聊 id，也不重派。
- 已发派后的普通错误、HTTP 502、超时、额度用尽、真身不符及其他非会话失效错误保持原结果；明确的会话失效错误结果或异常才清除旧 id，并以空 `sessionId` 重派一次。
- 正向用例同时覆盖抛出 `session not found` 和返回 `Unknown conversation id`；第二次新会话失败不会再次回退。

先红证据：先加入真实 `dispatchConsoleTurnWithFallback` 接线测试，再改实现。定向运行退出码 1；普通 `HTTP 502` 实际发送 `['live-thread', '']`，期望只发送 `['live-thread']`；发派前抛出 `session not found` 时 `listDispatchableModels` 实际调用 2 次，期望 1 次，证明旧实现误清会话并重派。

## 2. 补齐流水线结果与停止原因

- `onResults` 仍接收累积结果，但每次按节点取末次结果；收尾前再遍历 `pipeline.results` 的节点末次结果，补发未播报节点。
- `pipeline.error` 非空时，收尾消息追加“停止原因：……”。
- 测试通过真实 `dispatchConsolePreset` 和假 IO 触发验证失败、修正轮及总请求上限 2；消息流最终写明“停止原因：已达总请求上限 2，继续派发需重新确认。”。

先红证据：未实现时该测试退出码 1；`outcome.error` 已有上限原因，但实际收尾只有“模式派发结束：成功 1 路，失败 1 路，总耗时 0.0 秒。”，缺少停止原因。

## 3. 保存失败不吞收尾

- 最终 `saveGraph` 改为与 `commit()` 一致的异步保存并捕获错误。保存失败只写警告，不中断结果返回或收尾消息。
- 测试通过真实 `dispatchConsolePreset` 让 `saveGraph` 抛出 `disk full`，断言仍返回 1 条结果并写入“成功 1 路，失败 0 路”的收尾。

先红证据：未实现时该测试退出码 1，`dispatchConsolePreset` 在最终 `await io.saveGraph(graph)` 直接抛出 `Error: disk full`，没有执行收尾断言。

## 4. 修正后状态与末次统计一致

- 已播报状态由集合改为“节点 id → 最近状态”映射。同一节点状态变化时补发一条带正文的更正消息，例如“执行 · 修正后超时”；状态不变时仍不重复。
- 收尾继续按每个节点的末次结果统计。测试让同一节点先成功、后超时，断言出现更正消息，收尾为“成功 0 路，失败 1 路”。

先红证据：未实现时该测试退出码 1；更正消息存在性实际为 `false`，期望 `true`，而收尾已按末次结果统计，复现了消息与计数不一致。

以上四项的先红检查在同一次定向运行中得到：

```text
Test Files  1 failed (1)
Tests  5 failed | 4 passed (9)
```

实现后相关接线测试：

```text
Test Files  2 passed (2)
Tests  23 passed (23)
```

## 最终验证

以下命令均由本修正轮直接运行。

### `cargo check --all-targets`

```text
Compiling aicross v0.1.0 (D:\work\aicross-p6)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.86s
```

退出码 0，零 warning。

### `cargo test`

```text
running 470 tests
test result: ok. 459 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 26.82s
```

退出码 0。11 项 ignored 为仓库既有的真机、真实模型或本地事实源测试，本轮按要求未启用。

### `cd src-ui && npx vitest run --configLoader native --pool threads`

```text
Test Files  41 passed (41)
Tests  616 passed (616)
Duration  1.13s
```

退出码 0。`NO_COLOR` / `FORCE_COLOR` 提示为既有环境提示，不是测试失败。

### `cd src-ui && npm run build`

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build
✓ 348 modules transformed.
✓ built in 591ms
```

退出码 0。Vite 仍提示主 chunk 为 502.47 kB、超过 500 kB；这是构建警告，本轮未扩大范围处理。

### `python scripts/copy-audit.py`

```text
共 6 处命中，5 个文件（含误报，需人工复核）
```

退出码 0。命中仍在 `DispatchOverlay.tsx`、`SettingsModal.tsx`、`console-channel.ts`、`stamp.ts` 和 `resources/types.ts`，均不是本轮新增文案。

## 文件与边界

- 修正：`src-ui/src/console-chat.ts`、`src-ui/src/__tests__/console-feedback-persist-25.test.ts`。
- 新增：`tasks/REPORT-25-R1.md`。
- 上一轮未提交的其余修改全部保留；未修改 `src-ui/src/graph/dispatch.ts`、`src/server.rs`、`src/main.rs`、`src-ui/src/tauri.ts` 或 `CenterPanel.tsx`。
- 指定文件换行保持原状：`src/server.rs`、`src/main.rs`、`CenterPanel.tsx` 为 CRLF，`src-ui/src/tauri.ts` 保持原有混合换行；本轮修改的 TypeScript 与 Markdown 文件保持 LF。
