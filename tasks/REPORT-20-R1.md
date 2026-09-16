# REPORT-20-R1 派发建议条复审修正

## 结果

`fix-20-r1.md` 的 1–12 项已完成。建议仍只登记、不自动派发；采纳后才进入既有前端流水线。没有改动直接派发契约、薄中继边界或既有状态词方向。

## 逐条对应修正清单

### 1. 总指挥建议可裁决

- PTY 判定移到输入控件早退之前。xterm helper 同时是 `TEXTAREA` 时，只有 `Ctrl+Enter` 采纳；`Esc`、`Tab` 与普通回车均交还 PTY。
- 建议条增加「采纳」「忽略」按钮；模式名改为可点击按钮，点击顺序与 Tab 循环一致。
- PTY 聚焦时文案为「Ctrl+Enter 采纳 · 或点右侧按钮」，不再显示 Esc / Tab。

### 2. 单行输入框不触发采纳

- `INPUT`、`TEXTAREA`、contenteditable 及其内部元素统一视为输入目标；焦点在其中时建议条不处理任何按键。
- 普通输入控件聚焦时文案只显示「点右侧按钮」，不再提示实际不会生效的键。
- 从事件目标计算 `targetInput` / `ptyFocused` 的逻辑已抽成纯函数，并覆盖 xterm helper、单行 input、普通 textarea、普通 div 四类目标。

### 3. modal 的 Esc 优先

- `focus-enforcer.ts` 导出并复用同一条 `[aria-modal="true"]` 判定。
- 页面存在 modal 时建议条直接放行按键，设置页及其它弹层可收到 Esc。

### 4. 超时与失败建议

- 通道建议先等待 `proposed` 回执成功，再加入前端建议列表；超时或找不到 run 时不再留下幽灵建议。
- 采纳、忽略与本地建议生成均捕获失败；建议会从本地列表移除，并在原建议条位置显示可关闭的原因。
- 归类调用单独限制为最多 20 秒，低于 `/v1/propose` 的 25 秒等待档。

### 5. 在飞上限提示

- 采纳前用真实 `listConsoleRuns` 按设置中的通道上限检查；Rust 的全局上限仍作为并发兜底。
- 命中上限时不更新为 running，建议移除并显示「在飞已满，稍后再采纳」。

### 6. 本地建议进入 run 表

- 新增只登记 `proposed` 的 Tauri 命令 `console_register_proposal`，本地按钮建议显示前先登记。
- 本地与通道建议共用真实 run 查询、`console_run_update`、前端上限检查和 Rust `INFLIGHT_LIMIT`，不再使用空列表 / 空更新打桩。

### 7. 遗留与多条建议

- HTTP cancel 对 `proposed` 直接转为 `dismissed`，不向前端中继取消流水线。
- 前端通道安装时把无法恢复正文的遗留 `proposed` 翻成 `dismissed`。
- 新建议按 `proposal_id` 合并；同项目多条建议显示「还有 N 条」，处理当前条后继续显示下一条。
- 目标项目没有打开 tab 时，建议放到当前对话图；当前为总指挥时由总指挥建议条兜底显示。

### 8. 测试

- Rust 上限测试改为：先放满 `INFLIGHT_LIMIT` 条 running，再将 proposed 改为 running，断言返回 409 且原状态仍为 proposed。
- 前端补齐真实目标组合、单行输入、modal、登记顺序、本地登记、上限提示、按 id 合并、无目标 tab、遗留清理、归类判词与固定目录等用例；未引入 jsdom。

### 9. 归类判据与说明

- 只读取回复中第一个 `DISPATCH|SINGLE` 词；第一个词为 `SINGLE` 时，后文再出现 `DISPATCH` 也不派。
- 否决显示「归类模型判定不必派发」；调用失败显示「分类失败，已按不派处理：原因」。
- 用户指定的归类模型不在资源列表时明确报「所选归类模型当前不可用」，不回退到其它模型。

### 10. 名称与按钮

- 新增的输入框按钮改为「建议派发」，工具条既有「派发」保持不变。
- targets 形成的临时预设使用空名称；显示函数对空名称不追加分隔符，不再出现「自定义模式 · 自定义模式」或尾随分隔符。

### 11. 归类目录

- 归类调用固定使用 `runId/nodeId = proposal-classify`，复用同一数据目录，不再按建议 UUID 持续新增目录。

### 12. 文档

- `docs/CONSOLE-CHANNEL.md` 明确 `/v1/dispatch` 忽略 `reason`，并补充 proposed 的 cancel 结果。
- `docs/PRODUCT.md` 把入口同步为 `aicross propose`，并同步「建议派发」按钮与 PTY 实际键位。

## 关键测试先红

所有临时退化均在对应测试失败后立即恢复：

- 首轮只加测试时，PTY 的 `{ Enter, Ctrl, TEXTAREA+xterm }` 得到 `null` 而非 `accept`；终端文案仍错误显示 Esc / Tab；登记失败后列表仍存在 `late-proposal`。
- 临时移除输入目标判定后，单行 input 的回车得到 `accept`；移除 modal 判定后，Esc 得到 `dismiss`。
- 新增输入焦点文案断言时，旧实现仍返回「回车采纳 · Esc 忽略 · Tab 换模式」，测试先红后改为「点右侧按钮」。
- 临时跳过本地登记后，期望的 `local-a/local-b` 登记记录为空；把合并改回整表替换后，同项目两条建议只剩 1 条。
- 临时移除无目标 tab 的当前图回退后，结果由 `active` 变为空字符串。
- 临时移除采纳前上限检查后，`consoleRunUpdate(running)` 被调用 1 次；临时改回通用错误文案后，得到「采纳失败：同时在飞 run 已达上限 3」。
- 临时关闭遗留清理后，没有生成 `dismissed` 更新。
- 临时改回全文包含 `DISPATCH` 后，首词为 `SINGLE` 的回复被误判为 true；恢复静默模型回退后，不再抛「所选归类模型当前不可用」；合并失败与否决说明后，期望的分类失败文案不匹配。
- 临时恢复动态分类 runId、600 秒超时后，固定目录 / 20 秒断言同时失败。
- targets 显示测试在修正前实际得到「自定义模式 · 」，修正后为「自定义模式」。
- 临时移除 Rust proposed→running 上限判断后，测试在 `unwrap_err()` 处得到 `Ok(status=running)`；临时让 proposed cancel 不更新状态后，实际仍为 `proposed` 而非 `dismissed`。

恢复实现后，定向前端 21 条与 `console_api` 17 条测试均通过。

## 最终验证

未启动窗口、未连接真实模型，以下命令均在本工作树运行。

```text
> cargo check --all-targets
Compiling aicross v0.1.0 (D:\work\aicross-p5b)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.30s
```

零 warning。

```text
> cargo test
running 457 tests
test result: ok. 446 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 21.92s
```

11 项 ignored 为仓库既有的真机、真实模型或平台条件测试。

```text
> cd src-ui && npx vitest run --configLoader native --pool threads
Test Files  38 passed (38)
Tests  558 passed (558)
Duration  1.16s
```

```text
> cd src-ui && npm run build
✓ 342 modules transformed.
✓ built in 631ms
```

## 换行复核

- `src/server.rs`：4672 个 LF 全部为 CRLF；`src/main.rs`：251 个 LF 全部为 CRLF。
- `src/console_api.rs`、`src/console_cli.rs`：纯 LF。
- `CenterPanel.tsx`：2377 个 LF 全部为 CRLF。
- `src-ui/src/tauri.ts`：仍为混合换行（787 个 LF，其中 599 个 CRLF、188 个 bare LF）；只在命令表局部追加一行。
- 未运行 `cargo fmt`。

## 未做与原因

- 未执行清单开头的 `git merge main`：用户明确说明工作树已经合入 main，不需要重复合并。
- 未启动开发窗口、未发真机请求、未调用真实模型：本轮硬性禁止。
- 未运行真机交互验收：同上，仅完成纯函数、状态机、接口与构建验证。
- 未 commit、未 push。
- 未恢复 `tmp-orch-check.test.ts`：清单明确该文件已删除且不得加回。
