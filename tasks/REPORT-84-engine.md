# REPORT-84（引擎）：macOS 桌面端

按 `tasks/TASK-84-macos-desktop.md` 的 A 部分实施，由 Opus 在独立工作副本里完成，只改了 `src/`。合并后在主干上重跑了 `cargo check` 与 `cargo test`。

## 没验证的

- Windows 上没有重跑。要在 Windows 上重测的地方：
  - `src/agent_catalog.rs`：kimi 与 agy 的备用路径两行（只加了 `#[cfg(windows)]`，原行未改）、目录测试里 agy 的断言、`LocalAppData` 上的属性。
  - `src/channels/mod.rs`：`kimi_fallback`。
  - `src/process.rs`：`spawn_interactive` 上的 `#[allow(deprecated)]`，以及去掉的一个 `mut`。
- `blind_slot::tests::slot_that_cannot_be_cleared...` 失败过一次（「跳过的槽位要放锁」），之后 13 次都通过。原因没有确认，推测是并行测试里的子进程短暂占着锁文件；这段代码没有改。
- `aicross cancel` 没有在真实派发上试，结束进程树只有单元测试。
- 引擎自身被结束时，它拉起的子进程在 Unix 上仍会留下。这是原有的限制，没有改。
- agy、Claude 桌面版在 macOS 上的安装位置没有核对，本机没装：agy 不设备用路径，只查 PATH。

## 改了什么

- **进程号转换（原有缺陷）**：`process_alive` 与 `terminate_process_id` 把 `u32` 的进程号直接转成 `i32`。`u32::MAX` 会变成 `kill(-1, …)`：查存活时永远成功，已结束的派发一直显示在运行，`recover` 找不到卡住的节点；用于结束进程时会作用到当前用户的全部进程（现有调用方只传真实的子进程号，没有发生过）。新增 `process::unix_pid`，只接受 1 到 `i32::MAX`，两处都经过它。
- **结束进程树**：Unix 上原来只结束父进程。现在用 `pgrep -P` 逐层收集子孙进程后逐个结束。没有用进程组，否则终端里的 Ctrl-C 到不了 agent 的命令行。Windows 的 `taskkill` 分支未改。
- **后台引擎**：Unix 上在自己的进程组里启动，与 Windows 的 `CREATE_NEW_PROCESS_GROUP` 对应。
- **备用路径**：kimi 在 macOS 上是 `~/.kimi-code/bin/kimi`（`agent_catalog.rs` 与 `channels/mod.rs` 两处）；Codex 桌面版在 macOS 上找 `ChatGPT.app/Contents/Resources/codex-cli/bin/codex`（`/Applications` 与 `~/Applications`）。两个路径都对照本机实际安装核对过。
- **红绿核对的命令**：文件按 CRLF 写入时，Unix 的命令留下一个 `\r`，现在先去掉。
- **测试**：`spawn_detached_detaches_stdio` 用的 `/bin/true` 在 macOS 上不存在，改为 `/usr/bin/true`；`task07` 的快照 `mode` 为 0，还原出的文件读不了，改为 `0o644`；测试用的临时目录在非 Windows 上先解析成真实路径（macOS 的 `/var` 指向 `/private/var`）。新增 2 条：`unix_pid` 的取值范围、结束进程树时孙进程一并结束。
- **警告**：保留 `fetch_update`（替代它的 `try_update` 太新），加 `#[allow(deprecated)]`；去掉一个多余的 `mut`。

15 条失败的归因：8 条（`dead_pid_becomes_failed`、`list_runs_marks_dead_process_running_as_failed`、6 条 `recover_cmd::*`）是进程号转换的缺陷；1 条是红绿核对的 `\r`；其余 6 条是测试里的 Windows 假设。

## 验证

```
$ cargo check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.32s
$ cargo test
test result: ok. 810 passed; 0 failed; 9 ignored; 0 measured; 0 filtered out; finished in 18.89s
$ cd skills/ai-cross && python3 -m unittest discover -s tests
Ran 110 tests ... OK (skipped=1)
```

真机（调试版程序，临时 `AICROSS_HOME` 与临时项目目录）：

- `resources detect` 找到 claude 2.1.325、codex 0.169.0、kimi 2.5.2、pi 1.5.0。PATH 收窄后，kimi 仍经 `~/.kimi-code/bin/kimi` 找到，codex 经 ChatGPT.app 里的那份找到。
- `config show` 输出完整配置。
- 一次真实盲审（claude haiku 与 kimi k3，一句话的材料）状态为完成，三份记录在 `<项目>/.aicross/runs/20261116-190119-04f651a4/`，kimi 的固定目录用后已清空。
- 派发结束后没有残留进程：引擎、`claude -p`、`kimi` 以及 kimi 拉起的四个 MCP 进程都已退出。

合并后用打好的安装版窗口（临时 `AICROSS_HOME`）点「探测」：Claude Code 可用 4 个、Codex 8 个、Kimi Code 35 个、pi agent 未验证 36 个；窗口左上角有系统的关闭、最小化、缩放三个按钮。

## 没做什么

- 任务书「不做」里的各项。
- 窗口的拖动没有实测：能用的自动化工具是直接改窗口位置，不经过页面的拖动代码。
- macOS 的菜单栏是 Tauri 默认的英文菜单（File、Edit、View、Window、Help），没有改。
