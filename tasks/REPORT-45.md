# REPORT-45 ACP 适配器改为打开时当场安装

实现：Claude 子代理（10-01）。子代理环境不允许创建报告文件，本文由编排者按其交回的全文写入。

## 改动

- `src/channels/acp_setup.rs`
  - 删除后台安装 `ensure_installed()`，以及它依赖的进程内状态表（`SetupState`、`states`、`set_state`、`state`、`INSTALLING`、`last_failure`）和「10 分钟内失败不重试」的节流。
  - 新增 `AdapterState` / `AdapterReport`，序列化为 `{"agent","state","reason"?}`。`state` 取值：已就绪 / 刚装好 / 未装对应 CLI / 未安装 / 没有 npm / 安装失败。
  - 新增 `adapter_status()`，只查不装。
  - 新增 `install_missing_blocking()`，前台同步执行，一次 npm 调用装完所有缺的适配器；复用 `install_args`、`prefix()`（`~/.aicross/acp`）、钉死的包名与版本和 1800 秒超时；只装「适配器不在、对应 CLI 已安装」的那几家；失败原因取 npm 输出的最后 3 行。
  - 新增 `install_missing_with(&InstallEnv)`，测试通过它注入 npm 执行；`install_missing_blocking` 在测试构建下直接 panic，防止测试真装。
  - `unavailable_reason` 只保留原来的默认文案「{name} 的 ACP 适配器尚未就绪；本轮回退一次性 CLI」。另外三种文案依赖进程内状态，后台安装去掉后不会再出现。
- `src/channels/acp.rs`：`start_agent` 不再触发安装；新增 `adapter_program()`，适配器解析失败时按原逻辑返回 Process 错误并回退一次性 CLI。ACP 协议、会话池和派发逻辑未动。
- `src/console_cli.rs`
  - 新增注入结构 `AcpOps { install, status }`，`run_config` / `run_config_set` 增加该参数。
  - `config set acpEnabled true` 写盘后执行同步安装：stdout 输出 `{"dispatch": {...}, "acp": [...]}`，stderr 一行汇总；有一家为「没有 npm」或「安装失败」时不回滚设置、退出 1，stderr 列出这几家并说明派发对它们回退一次性 CLI。设为 `false` 时不安装，输出与原来相同。
  - `config show` 追加 `acp` 字段（只查不装）。这一处超出任务书「仅 config set acpEnabled 分支」，是要求 4 所需。
- `docs/CONSOLE-CHANNEL.md`、`docs/MANUAL.md`：ACP 默认关闭、打开方式与当场安装、安装需要 npm 和对应 CLI、装不上会回退一次性 CLI，以及 `config show` 的 `acp` 字段。

## 测试

新增：
- `channels::acp_setup::tests::task45_install_skips_npm_when_all_adapters_ready`
- `channels::acp_setup::tests::task45_install_runs_npm_in_foreground_and_reports_fresh_install`
- `channels::acp_setup::tests::task45_install_skips_agents_without_their_cli`
- `channels::acp_setup::tests::task45_install_reports_missing_npm`
- `channels::acp_setup::tests::task45_install_failure_carries_last_npm_lines`
- `channels::acp::tests::task45_missing_adapter_falls_back_without_installing`
- `console_cli::tests::task45_config_set_acp_enabled_installs_and_reports`
- `console_cli::tests::task45_config_set_acp_enabled_failure_exits_1_and_keeps_setting`
- `console_cli::tests::task45_config_set_acp_disabled_does_not_install`
- `console_cli::tests::task45_config_show_lists_acp_status_without_installing`

改写：`channels::acp_setup::tests::reasons_are_plain_and_say_the_turn_falls_back` 按单一文案改写，并断言不出现「后台」。

全部测试用注入的 npm 执行，不真装。

## 回退验证

1. 在 `adapter_program` 里加回安装调用，同时让 `set acpEnabled true` 不安装：3 条新测试变红。
2. 让 `set` 无条件安装、失败原因只取 1 行、不查对应 CLI、不查装后可执行文件、没有 npm 报成安装失败：5 条新测试变红。
3. `show` 去掉 `acp` 字段、状态检查忽略「已就绪」：4 条新测试变红。

每轮都从备份恢复，恢复后 `cargo test task45` 为 10 passed。

## 验证

- `cargo check --all-targets`：零 warning。
- `cargo check --release`：通过。
- `cargo test`：420 passed，0 failed，10 ignored。

## 未做与理由

- 没有在真机执行 `aicross config set acpEnabled true` 做真实 npm 安装：测试不真装，真机安装要下载较多依赖，留给用户需要时手测。
- 没有 commit、push 或 cargo fmt；改动文件保持 LF。
