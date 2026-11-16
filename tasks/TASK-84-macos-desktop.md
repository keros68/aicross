# TASK-84 macOS 桌面端

## 现状

2026-11-16 在一台 Apple Silicon Mac（macOS 27.2，rustc 1.99.1，node 26.5）上摸底，源码 0.3.11：

| 项 | 结果 |
|---|---|
| 前端 `npm run build` | 通过 |
| `cargo check` | 通过，3 条警告 |
| `cargo test` | 793 通过，15 失败，9 忽略 |
| `tauri dev`、`tauri build` | 没有跑过 |
| 安装包 | 只发 Windows；`scripts/build-release.ps1` 与 `latest.json` 只有 `windows-x86_64` |

失败的 15 条：

```
dispatch::tests::task07_before_snapshot_success_replaces_old_and_removes_prev
dispatch::tests::task13_red_green_restores_exact_bytes_and_leaves_unrelated_file_alone
dispatch::tests::task74_agy_1_5_6_blind_large_project_copy_single_writer_and_worktree_gate
dispatch::tests::task75_agy_blind_slots_archive_and_project_flag
dispatch::tests::task75_kimi_blind_concurrent_dispatches_use_distinct_slots_rewrite_paths_and_archive
dispatch::tests::task75_non_kimi_blind_dir_unchanged
engine::recover_cmd::corrupt_inflight_manifest_fails_rollback_and_reports
engine::recover_cmd::inflight_interruption_is_reported_and_rolled_back
engine::recover_cmd::read_only_check_lists_stuck_nodes_and_changed_files
engine::recover_cmd::rollback_restores_files_marks_nodes_and_refreshes_run
engine::recover_cmd::rollback_without_stuck_nodes_still_appends_reason
engine::recover_cmd::worktree_without_artifacts_gets_inferred_path
engine::tests::dead_pid_becomes_failed
engine::tests::spawn_detached_detaches_stdio
run_view::tests::list_runs_marks_dead_process_running_as_failed
```

生产代码里只按 Windows 写的地方（不含测试）：`src/agent_catalog.rs:106`（kimi 备用路径 `kimi.exe`）、`:132`（agy 备用路径在 `LocalAppData`）、`:334`（codex 桌面版路径）；`src/channels/mod.rs:741`（`kimi.exe`）；`src/process.rs:720`（`taskkill.exe`）；`src/dispatch.rs:1968`（`powershell.exe`）。

## 目标

在 Apple Silicon 的 macOS 上：窗口能打开、五页能用；`aicross` 命令行能探测本机 agent、读写配置、发起派发并留记录；能打出一个本机可安装的 `.dmg`。Windows 上的行为不变。

## 分工

两部分各在自己的 git worktree 里做，互不改对方的文件。

### A 引擎（`src/**`、`Cargo.toml`、`build.rs`）

1. 15 条失败的测试逐条查清原因：是生产代码在 macOS 上行为不对，还是测试里写死了 Windows 的假设。前者修生产代码；后者改测试，使它在两个系统上都验证同一件事，不用 `#[cfg(windows)]` 把测试整条跳过，除非被测的功能本身只在 Windows 上存在。
2. 上面列出的只按 Windows 写的生产代码逐处处理：macOS 上给出对应的做法（备用路径、结束进程树、不依赖 PowerShell 的替代），放在 `cfg` 分支里，Windows 分支原样保留。再全仓查一遍有没有漏的。
3. 用 `target/debug/aicross` 在本机真跑，临时 `AICROSS_HOME`：`resources` 探测到本机的 claude、codex、kimi、pi；`config show`；用最便宜的档位和一句话的材料真实派发一次盲审，核对记录写进项目的 `.aicross/runs/`、进程结束后没有残留子进程。
4. 清掉 `cargo check` 的 3 条警告，前提是改法在 Windows 用的 Rust 版本上同样能编译；做不到就留着并说明。

### B 窗口与打包（`src-ui/**`、`tauri.conf.json`、`scripts/**`、`docs/**`、新建 `macos/`）

1. `npm exec @tauri-apps/cli@2 -- tauri dev` 能打开窗口。自绘标题栏在 macOS 上：窗口按钮在左侧、可拖动、可缩放，圆角与阴影正常；`src-ui/src/lib/os.ts` 已有 `IS_MACOS`，样式里已有 `html.is-macos`，在此基础上补。五页逐页打开，截图存进任务报告指到的目录。
2. `tauri build` 打出 `.app` 与 `.dmg`（只要 `aarch64`）。本机没有更新签名私钥，也没有 Apple 开发者证书：新增 `scripts/build-macos.sh`，没有私钥时关掉更新包生成照常出 `.dmg`，有私钥时生成签名并把 `darwin-aarch64` 一项并进 `latest.json`（脚本只写这段逻辑，本机验证没有私钥的那条路）。不做公证。
3. 命令行进 PATH：macOS 的安装是拖进「应用程序」，没有安装器钩子。在 `docs/MANUAL.md` 写明用一条 `ln -s` 把 `/Applications/aicross.app/Contents/MacOS/aicross` 链到 PATH 上的目录；核对从链接启动时 `aicross --version`、`aicross config show` 正常。
4. 没有 `darwin-aarch64` 的 `latest.json` 时，窗口不报错、不显示更新提示。需要动 `src/` 才能做到的，写清楚要改哪里，交给 A 之后处理，不要自己改 `src/`。
5. `docs/MANUAL.md`、`docs/PRODUCT.md` 里关于平台的说明改成现状；未签名的应用第一次打开要在「系统设置 › 隐私与安全性」里放行，写进手册。文案按 `AGENTS.md` 的界面文案规范。

## 不做

- 不买、不配置 Apple 开发者证书，不做公证。
- 不做 Intel Mac 与通用二进制，不做 Linux。
- 不发 Release，不改应用版本号，不 push。
- 不读 `~/.pi/auth.json`、`~/.cc-switch/*.db` 等凭据文件。
- 不重构与 macOS 无关的代码。

## 验证

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cd src-ui && npm install && npm run build && npm test
cargo check && cargo test
cd skills/ai-cross && python3 -m unittest discover -s tests
```

`cargo test` 在 macOS 上 0 失败。Windows 上的结果本机验证不了，报告里列出所有动过 `cfg(windows)` 分支或共用代码路径的地方，留给 Windows 机器重跑。

## 交付

A 写 `tasks/REPORT-84-engine.md`，B 写 `tasks/REPORT-84-app.md`：改了什么、命令与输出原文、没做什么、不确定处，没做完和失败的写在最前面。Conventional Commits 英文提交到各自的分支，不 push。
