# REPORT-85：引擎被结束时带走子进程，并发记录不互相覆盖

按 `tasks/TASK-85-engine-termination-and-records.md` 实施，只改了 `src/`。本机 macOS（Apple Silicon）。

## 没验证的

- Windows 上没有运行。`write_record` 与报错文案两处改动两个系统共用；结束信号的处理只在 Unix 编译。
- 结束信号只真机试了 SIGTERM，SIGHUP 与 SIGINT 走同一段代码，没有单独试。
- 引擎被 SIGKILL 或崩溃时，子孙进程仍会留下。
- `blind_slot` 那条偶发失败的测试这次没有再出现，原因仍未确认。

## 改了什么

- **结束信号（`src/process.rs`、`src/console_cli.rs`）**：新增 `kill_children_on_termination`，命令行子命令启动时调用一次。处理函数只记下信号编号，一个线程每 100 ms 查一次，命中后用 `pgrep -P` 收集自己的子孙进程逐个结束，再以 128 + 信号编号退出。收集与结束两步从 `terminate_process_id` 里拆成 `unix_descendants` 与 `kill_pids`，两处共用。
- **记录文件名（`src/dispatch.rs`）**：`write_record` 用 `create_new` 占名，已存在时换下一个后缀。
- **报错（`src/hosts.rs`、`src/console_cli.rs`、`src/recommend.rs`）**：新增 `hosts::unknown_host_message`，四处共用。没有检测到主 agent 时为「未知 host：claude；本机还没有检测到主 agent，先在资源页点「探测」或运行 aicross resources detect」；有检测结果时文案不变。
- **强制转换排查**：`src/` 里的 `as i32` 只剩 `paths.rs` 一处（Windows 的错误码，与进程号无关）。`libc::kill` 共四处调用，进程号都经过 `unix_pid` 或来自 `pgrep` 输出并已过滤为正数。没有发现第二处。
- **测试**：新增 2 条，`unknown_host_message` 的两种文案、只结束子孙进程时根进程保留。

## 验证

```
$ cargo check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.19s
$ cargo test
test result: ok. 812 passed; 0 failed; 9 ignored; 0 measured; 0 filtered out; finished in 18.30s
```

并发记录那条测试单独运行：改动前（未改的代码）8 次里失败 4 次，改动后连跑 25 次全部通过。

真机（调试版程序，临时 `AICROSS_HOME` 与临时项目目录）：

- 未探测时 `aicross config show --host claude` 输出新的报错。
- 探测后做一次盲审派发（claude haiku、kimi k3、claude sonnet，一句话的材料）。引擎拉起子进程后对引擎发 SIGTERM，2 秒后查看：引擎、`claude -p`、`kimi-code` 与 kimi 拉起的 4 个 MCP 进程都已退出；`aicross status` 把这次派发报为 failed。

## 没做什么

- 任务书「不做」里的各项。
- 没有改 skill；skill 转述的是引擎的报错原文。

## 不确定处

- 处理函数登记在所有命令行子命令上，包括很快就结束的查询命令；多一个每 100 ms 醒一次的线程，没有观察到影响。
- `pgrep` 收集与结束之间新拉起的进程、已脱离会话的进程收不到，与 `aicross cancel` 的限制相同。
