# REPORT-42 引擎被杀不留孤儿进程；冒烟超时不再判验证失败

实现：GLM-5.3-Flash（pi，10-01）。代码与测试由它写完后进程被会话结束打断，文档补写、一处测试编译错误修正、验证与真机核由编排者完成。

## 改动

- `src/process.rs`
  - `run_process_registered_with_activity` 在 `.spawn()` 之后调用 `assign_process_to_job(child.id())`：一次性派发与冒烟的子进程挂进进程级 kill-on-close Job，孙进程按继承进入同一个 Job。非 Windows 上该函数无操作。
  - 测试辅助 `windows_job::is_pid_in_job(pid)`（仅 `cfg(test)`），用 `IsProcessInJob` 对本进程的 Job 句柄查询。
- `src/resources.rs`
  - 新取值 `VERIFICATION_TIMEOUT = "timeout"`，常量 `SMOKE_TIMEOUT_SECS = 180`，冒烟等待改用它。
  - `run_smoke_registered`：进程超时时直接返回「冒烟超时（180 秒）」，不再解析残余输出。
  - `apply_smoke_outcome`：失败原因是超时时 `verification` 记 `timeout`（原生模型仍为 `native`），其余失败照旧 `failed`。
  - `rejudge_stored_smoke_failures`：加载时不把 `timeout` 记录重判成 `failed`。
  - `verification_rank`：`timeout` 与未验证同档。
  - `model_state`：`timeout` →「冒烟超时」，note「可重试 aicross resources smoke <agent> --model <id>；不影响派发」。
- `src/engine/mod.rs`：未改。派发前检查原本就只拦 `verification == "failed"`，新增测试钉住。
- `src/console_cli.rs`、`src/engine/tests.rs`：只加测试。
- `docs/CONSOLE-CHANNEL.md`、`docs/MANUAL.md`：冒烟 180 秒上限与「冒烟超时」状态；Windows 上引擎退出时连带结束它启动的 CLI 进程，其它平台不保证。

## 测试

新增：
- `process::tests::oneshot_grandchild_lands_in_the_kill_on_close_job`（Windows）
- `resources::tests::task42_smoke_timeout_is_recorded_as_timeout_not_failed`
- `resources::tests::task42_smoke_timeout_is_180_seconds`
- `resources::tests::task42_timeout_state_shows_retry_note`
- `resources::tests::task42_timeout_record_survives_reload`
- `engine::tests::plan_dispatch_passes_smoke_timeout_and_still_rejects_failed`
- `console_cli::tests::task42_resources_lists_smoke_timeout_with_retry_note`

回退验证：临时去掉 `assign_process_to_job` 调用、去掉 `apply_smoke_outcome` 的超时分支、去掉重判里的 `timeout` 例外、把派发前检查改成连 `timeout` 一起拦，`cargo test` 结果 `412 passed; 5 failed`，失败的正是上面除 `task42_smoke_timeout_is_180_seconds` 外的 5 条（该条只钉常量值，本次回退没有改常量）。恢复后全绿。

## 命令输出

```
cargo check --all-targets   Finished `dev` profile，零 warning
cargo check --release       Finished `release` profile
cargo test                  test result: ok. 417 passed; 0 failed; 10 ignored
```

## 真机核（编排者，release 构建）

- 临时 git 项目派发经济模式，等后台引擎拉起 `pi.exe`（出方案）后 2 秒强杀引擎进程：杀前 pi 存活，杀后 2 秒内 `pi.exe` 与其 `conhost.exe` 均已结束；run 状态 `failed`，原因「引擎进程已退出」。
- `aicross resources smoke pi --model glm-5.3 --provider zai-coding-cn`：2.6 秒完成，已验证；超时路径未在真机复现，由单测覆盖。资源库事先备份，结束后已恢复。

## 未做与限制

- 首次派发时冒烟超时：这次派发仍按失败返回（原因「冒烟超时（180 秒）」）；之后该模型为 `timeout`，首次使用验证跳过，派发直接进行，真身核对推迟到手动 `resources smoke`。
- 非 Windows 平台不挂 Job，引擎被杀时子进程仍可能成孤儿。
- 被杀后图上留下的 `draft` / `pending` 节点由 TASK-44 的 `aicross recover` 处理。
