# TASK-45 ACP 适配器改为打开时当场安装（10-01）

**只改 `src/channels/acp_setup.rs`、`src/channels/acp.rs`（仅安装触发处）、`src/console_cli.rs`（仅 `config set acpEnabled` 分支）、`docs/CONSOLE-CHANNEL.md`、`docs/MANUAL.md`。不 commit，不 push。**

## 背景

`docs/PRODUCT.md`「10-01 定案」：ACP 默认关闭，派发走各家 CLI 自带的续聊。现在的安装方式是 `acp.rs` 约 299 行在需要 ACP 时调 `acp_setup::ensure_installed()`，后者在后台线程里跑 npm；窗口下线后它只会在寿命很短的 `aicross __run` 进程里触发，引擎结束时安装多半来不及完成或记录，用户也看不到结果。

## 要求

1. **后台不再安装**：`acp.rs` 不再调用 `ensure_installed()`；适配器不在就按现有逻辑回退一次性 CLI（`unavailable_reason` 文案照旧）。
2. **同步安装函数**：`acp_setup` 提供同步版本（例如 `install_missing_blocking() -> Vec<AdapterReport>`），对「适配器不在、但对应 CLI 已安装」的那几家，在前台跑 npm 安装（复用现有 `install_args`、`prefix`、包名与版本、超时），返回每家的结果：已就绪 / 刚装好 / 未装对应 CLI（跳过）/ 没有 npm / 安装失败（附 npm 最后几行原因）。不保留「10 分钟内失败不重试」的节流（用户主动打开时就该真装）。原后台版本若已无调用方，删掉。
3. **打开时触发**：`aicross config set acpEnabled true` 写入设置后，调用上面的同步安装并把每家结果写进 stdout 的 JSON（追加字段 `acp`，例如 `{ "dispatch": {...}, "acp": [{ "agent": "claude", "state": "已就绪" }, ...] }`），stderr 一行汇总。安装失败不回滚设置，但退出码为 1 并说明哪几家没装上、派发将对它们回退一次性 CLI。设为 `false` 时不安装。
4. **查看状态**：`aicross config show` 输出里追加 `acp`：每家适配器当前是否就绪（只查，不装）。
5. 不改 ACP 协议、会话池与派发逻辑。

## 测试

- 同步安装函数：注入 npm 执行（不真装），覆盖 已就绪 / 刚装好 / 跳过未装 CLI / 没有 npm / 安装失败 各一例。
- `config set acpEnabled true` 调用安装、输出 `acp` 字段、失败时退出 1 且设置已写入；`false` 不调用安装。
- `acp.rs` 不再触发安装：去掉调用后，原有触发安装的测试按新行为改写。
- 新测试在修复临时改回时必须变红。

## 文档

`docs/CONSOLE-CHANNEL.md`、`docs/MANUAL.md`：ACP 默认关闭；打开方式与当场安装；安装需要 npm 与对应 CLI；装不上的会回退一次性 CLI。

## 验证

```
cargo check --all-targets        # 零 warning
cargo check --release
cargo test
```

每个改动文件保持原有换行。完成后写报告（不能建文件时写在回复里，由编排者落盘到 `tasks/REPORT-45.md`）：改动、测试名、回退验证、三条命令输出、未做与理由。
