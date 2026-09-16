# REPORT-20 派发建议条

## 结果

TASK-20 已实现。建议只登记、不自动派发；用户采纳后才进入 TASK-16 既有的 `runConsoleDispatch → startPipelineHeadless` 流水线。Rust 仍只负责请求校验、Tauri 事件中继和有上限的 run 摘要，没有新增派发执行路径。

## 对应任务书「实现」

### 1. 建议条

- 新增 `src-ui/src/components/center/ProposalBar.tsx` 与样式，建议条显示在当前总指挥或对话图 tab 顶部。
- 文案为「建议派发 · 模式名 · N 路 · 回车采纳 · Esc 忽略 · Tab 换模式」；PTY 聚焦时改为 `Ctrl+Enter 采纳`，`reason` 作为末尾浅色说明。
- 快捷键在输入法组字或多行输入焦点内不生效；建议条不主动取得焦点。
- 同项目的隐藏标签页不会响应当前建议条的快捷键，避免一次按键触发多次采纳或忽略。
- Tab 顺序按 `presetGroupOf` 分为基础三模式、自定义模式、更多模式；选中后只修改本次建议。直接 targets 形成的自定义模式切到内置模式后不再沿用原 targets。
- 采纳把通道 run 从 `proposed` 转为 `running`，再调用既有前端流水线；忽略转为 `dismissed`。

### 2. 用户主动触发

- 总指挥的妙手输入区和对话图输入框旁新增「派发」按钮。
- 按钮生成前端本地建议，不走 HTTP，默认 `builtin-economy`；图片路径附件从妙手输入区一并传给小任务判断。
- 本地建议与通道建议使用同一建议存储、建议条、模式切换和采纳函数。

### 3. 总指挥触发

- 新增 `POST /v1/propose` 和 `aicross propose --project <dir> [--preset <名>] [--reason <一句话>] --text-file <path>`。
- 正常响应为 `{ "proposal_id": "…", "accepted": true }`；Rust 发 `aicross:console-propose`，前端回执后才把 `status: "proposed"` 写入 run 表。
- `proposed` 不计入在飞上限；run 摘要仍受 200 条上限约束。
- `docs/CONSOLE-CHANNEL.md` 已补接口、状态、CLI 和 `/派发` / `/派发 盲审` 用法；`docs/MANUAL.md` 术语表同步新增建议与二次判断用词。
- 既有 `aicross dispatch` 仍直接派发，路径和参数行为未改。

### 4. 要不要建议

- `isSmallEconomyTask(text, attachments)` 命中时不生成建议；CLI 返回 `{ "proposal_id": "", "skipped": "单步小任务" }`，run 表不登记。
- 设置页「派发」新增「用归类模型二次判断」，默认关闭，并可选择 `classificationModel`；留空时选择已接入的低档模型。
- 开启后只在用户或总指挥请求生成建议时调用一次归类模型，并且启发式与归类模型都同意才显示建议；没有在聊天每条消息上调用。
- 归类请求复用现有 `dispatchGraphStage`，`writeProjectDispatch: false`、只读，不在 Rust 新建模型调用路径。

### 5. 测试

- Rust：覆盖 `/v1/propose` 路由与响应、假前端回执登记、`proposed → running / dismissed`、忽略后禁止恢复、proposed 不占在飞上限、小任务不登记。
- CLI：覆盖 `propose` 路径、预设别名、reason、多行 UTF-8 `--text-file`、拒绝 argv 正文、参数错误退出码 2、reason 多行拒绝。
- 前端：覆盖模式循环顺序、PTY 快捷键变体、IME/多行输入排除、reason 文案、本地按钮载荷、小任务与归类开关、`/v1/propose` 默认不调用 `dispatchGraphStage`。

## 测试先红

- 首次加入测试、尚未实现时：Rust 因缺少 `relay_proposal` 编译失败；前端因缺少 `ProposalBar` 测试套件失败。
- 临时撤掉 `dismissed` 终态保护后，`proposed_run_transitions_only_to_running_or_dismissed` 失败，表现为已忽略建议可被改回 running。
- 临时把 `propose` 路径改回 `/v1/dispatch` 后，CLI 测试按路径不符失败；临时移除 reason 单行校验后，多行 reason 测试失败。
- 临时把二次判断默认改为开启后，假前端测试实际调用了会抛出「不应派发」的 `dispatchGraphStage` 并失败。
- 上述临时改动均已恢复，定向测试重新通过。

## 最终验证

未运行窗口、未连接真实模型，以下均为本地自动验证。

```text
> cargo check --all-targets
Compiling aicross v0.1.0 (D:\work\aicross-p5b)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.97s
```

无 warning。

```text
> cargo test
running 456 tests
test result: ok. 445 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 25.57s
```

11 项 ignored 均为仓库既有真机/真实模型或本地事实源测试，本次未启用。

```text
> cd src-ui && npx vitest run --configLoader native --pool threads
Test Files  38 passed (38)
Tests  549 passed (549)
Duration  1.19s
```

```text
> cd src-ui && npm run build
✓ 342 modules transformed.
✓ built in 660ms
```

换行复核：`src/server.rs`、`src/main.rs` 仍为纯 CRLF；本次修改的 `src/console_api.rs`、`src/console_cli.rs` 为纯 LF；`src-ui/src/tauri.ts` 保持混合换行，只修改相关局部；`CenterPanel.tsx` 保持原有纯 CRLF。未运行 `cargo fmt`。

## 不做与限制

- 未实现模型自动执行、不问用户直接派发或后台静默派发：任务书明确排除。
- 未在聊天每条消息上运行归类模型：开关默认关闭，且只在显式生成建议时运行一次。
- 未启动开发窗口、未发真实模型请求、未做窗口手测：用户本次硬性要求禁止。
- 未验证开启二次判断后的真实模型响应质量：需要真实模型请求，本次只用假后端验证调用次数与结果分支。
- 未 commit、未 push；未修改 `src/server.rs`、`src/main.rs`。
- 最终核对时 `console/proposal` 的 HEAD 为 `1f7882b`，本地 `main` 已在其上前进 3 个提交；本次按要求未做 rebase 或合并。
