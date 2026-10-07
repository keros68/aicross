# TASK-48 资源库里「未发现」的模型（10-06）

**只改 `src/**`、`docs/CONSOLE-CHANNEL.md`。不改前端。不 commit，不 push。** 在 TASK-46 的未提交改动之上继续（同一工作树）。

## 背景

10-06 真机：pi 升到 1.0.2 后，`qwen-token-plan-cn` 与 `deepseek` 两个 provider 已不在 `pi --list-models` 里，但资源库还保留着它们的模型，状态显示「未验证」，`deepseek/deepseek-v4-flash` 甚至仍是「已验证」。窗口里对它们冒烟只得到「No API key」，派发与选人选也不会拦。

现有代码其实已经知道：`src/resources.rs` 的 `merge_detected_models` 会给最近一次探测没发现的模型在 `meta` 里追加 `MISSING_MARKER`，只是状态、冒烟、选人选、派发都没用它。重新探测发现了它，合并时整条替换，标记随之消失。

## 1. 状态

`resources::model_state` 增加状态 **`未发现`**：`meta` 含 `MISSING_MARKER` 时返回，`note` 为「最近一次探测没有发现这个模型；在对应 agent 里确认登录后重新探测」。优先级：在 `未安装`（agent 不可用）之后，在额度用尽、原生、未登录、验证结果之前。原生模型同样适用。用户申报的模型（`source` 以「申报(」或「来源：用户申报 · 」开头）本来就不打标记，不受影响。

## 2. 用到状态的地方

1. **冒烟**：`resources smoke <agent> --model <m>` 指向 `未发现` 的模型时不发请求，退出 1，stderr 写上面的 note；按 agent 冒烟全部第三方模型时跳过 `未发现` 的，stderr 逐个说明跳过。窗口命令 `resources_smoke` 同口径（共用函数）。
2. **选人选**：`config set-role` 与 `config set-seat` 的人选或备选是 `未发现` 时退出 2，stderr 写原因，并列出该 agent 下可选的模型（列表里去掉 `未发现` 的）。
3. **派发**：派发前检查遇到目标或备选模型是 `未发现` 时拒绝，原因写「<agent>/<model> 在最近一次探测中未发现，请重新探测或换人选」。找到现有对「验证失败」做派发前拦截的地方，加在同一处，不另写一套。备选未发现时：只拒绝这个备选（视为未设备选），主目标照常；在 run 文件或 stderr 里留一句说明。
4. **展示**：`config show` 的 `modes[].stages[][].effective` 追加 `state` 字段（该人选在资源库里的状态；资源库里没有这条模型时为 `不在资源库`），窗口据此提示失效的人选。同一结构的窗口命令跟着变。

## 3. 探测时间

`resources.json` 顶层追加 `detectedAt`（RFC3339，`resources detect` 成功写回时更新；旧文件没有该字段时为空）。`aicross resources` 的输出顶层追加 `detected_at`（与其它字段同为 snake_case；从未探测过为 `null`）。窗口用它显示「上次探测」时间。这是对资源库格式的追加字段，不改已有字段。

## 4. 约束

- 不改 `merge_detected_models` 的合并规则，只读它打的标记。
- 不读凭据、不输出任何 token。

## 5. 测试

- `model_state`：带标记的第三方 / 原生 / 原为「已验证」的模型都返回 `未发现`；agent 不可用时仍是 `未安装`；去掉标记（重新探测发现）后恢复原状态。
- 合并：先探测到 A、B，再探测只有 A → B 为 `未发现`；再探测到 B → B 恢复。
- 冒烟：单个 `未发现` 退出 1 且不调用冒烟函数（冒烟函数可注入）；按 agent 冒烟跳过它。
- `set-seat` / `set-role` 指向 `未发现` 退出 2，候选列表里没有它。
- 派发：目标 `未发现` 被拒并给出原因；备选 `未发现` 时主目标照常派出。
- `config show` 的 `effective.state`；资源库没有这条模型时为 `不在资源库`。
- `detectedAt` 写入与 `detected_at` 输出；旧文件无该字段时为 `null`。

## 6. 文档

`docs/CONSOLE-CHANNEL.md`「资源与配置」：状态取值加 `未发现` 及其含义；冒烟、`set-seat`、派发对它的处理；`detected_at`；`effective.state`。

## 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # 零 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
```

换行规则同 TASK-46（`src/server.rs`、`src/main.rs` CRLF，其余 Rust LF）。写 `tasks/REPORT-48.md`：改动文件、复用了哪些现成函数、测试名、验证结果、未做与理由。
