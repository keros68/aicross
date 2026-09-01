# REPORT-F2 — 切断与上游的运行时冲突

## 结果

F2 已完成。aicross 不再在应用启动、启动台工具重扫或 Claude 终端启动时自动写外部 CLI 配置。

**是否还存在任何在用户没有显式触发的情况下写外部 CLI 配置的代码路径：否。**

当前可达的外部 CLI 配置写入逻辑全部位于 `hook_installer` 内，只能从未接前端入口的显式 Tauri command `install_hooks` 到达。前端没有 `install_hooks`、`install_hook_for_tool` 或 `installHookForTool` 调用。旧的 Claude `theme=auto` 写入辅助函数仍保留，但没有调用方，终端启动不再调用它。

本期没有运行 hook 安装入口，没有启动应用窗口，也没有读、写或删除用户真实的 `~/.coffee-cli`、`~/.claude`、`~/.codex`、`~/.kimi-code`、`~/.config/opencode` 或 Hermes 目录。

## 改动

### A. 停止自动写外部 CLI 配置

- 删除 Tauri `setup` 中的 `hook_installer::install_all()` 启动调用。
- 删除启动台工具重扫后的前端 `installHookForTool` IPC 调用及前端封装。
- 保留 `hook_installer` 的函数与测试；新增并注册显式 `install_hooks` Tauri command，本期前端没有入口。
- 在 `src/hook_installer.rs` 顶部注明：本 fork 不自动安装 hook，必须由用户显式触发，原因是全局 CLI 配置会与上游 Coffee CLI 冲突。
- 额外审计发现并断开 Claude 终端启动时自动写 `~/.claude/settings.json` 的 `theme=auto` 调用。辅助函数保留但不可从启动路径到达。

### B. 独立运行时标识

- 在 `src/paths.rs` 新增 `aicross_root()` 与供隔离路径使用的 `aicross_root_from()`，七处持久化路径统一改为 `~/.aicross`：hooks、marketplace、heatmap cache、tasks、skills、skills-library、tools.json。
- keyring service 从 `coffee-cli` 改为 `aicross`；没有读取或迁移上游凭据。
- hook 环境变量改为 `AICROSS_TAB_ID`、`AICROSS_HOOK_PORT`、`AICROSS_TOOL`，脚本中的判断同步修改；调试变量改为 `AICROSS_DEBUG`。
- hook 参考脚本和安装文件统一改为 `aicross-` 前缀：
  - `aicross-hook.py`
  - `aicross-codex-notify.py`
  - `aicross-opencode-plugin.js`
  - `aicross-hermes-plugin.py`
  - 安装名 `aicross-island.js`
  - Hermes 插件名 `aicross-status`
  - Grok 历史状态文件名 `aicross-status.json`
- `src-ui/package.json` 与 `src-ui/package-lock.json` 的包名改为 `aicross-ui`。
- 没有迁移、读取、复制或删除 `~/.coffee-cli`。`~/.aicross` 首次运行为空是预期行为。

### C. 删除上游发布脚本

已删除整个 `install/` 目录。该目录的 PowerShell、shell 安装脚本与发布说明模板指向上游 `coffeecli.com` 发布渠道，不属于 aicross。本项目确定自己的分发方式后再单独实现。

`installer.nsi` 保留且未改。

## 验收

### 1. `cargo check`：通过

命令：

```powershell
cargo check
```

输出原文：

```text
   Compiling aicross v0.1.0 (D:\work\aicross)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.31s
```

### 2. `cargo test`：通过

上游测试数量未减少：共运行 82 个测试，80 passed，2 ignored。

命令：

```powershell
cargo test
```

输出原文：

```text
   Compiling aicross v0.1.0 (D:\work\aicross)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 6.04s
     Running unittests src\main.rs (target\debug\deps\aicross-8f73a09b6736e052.exe)

running 82 tests
test server::resume_cwd_tests::does_not_split_on_a_dot_inside_a_component ... ok
test launch::tests::ignores_non_launch_argv ... ok
test launch::tests::parses_tool_with_and_without_argv0 ... ok
test launch::tests::rejects_unknown_tool ... ok
test launch::tests::parses_equals_form_and_validates_cwd ... ok
test server::resume_cwd_tests::does_not_touch_an_ordinary_hidden_dir_without_worktrees ... ok
test server::resume_cwd_tests::handles_mixed_separators_and_non_ascii ... ok
test server::resume_cwd_tests::leaves_a_plain_project_dir_unchanged ... ok
test server::resume_cwd_tests::leaves_a_real_subdir_unchanged ... ok
test server::resume_cwd_tests::map_is_empty_when_claude_json_is_missing ... ok
test server::resume_cwd_tests::requires_worktrees_to_be_a_whole_next_component ... ok
test server::resume_cwd_tests::map_has_no_entry_for_an_unregistered_folder ... ok
test server::resume_cwd_tests::strips_only_dot_claude_worktrees_not_other_dot_dir_worktrees ... ok
test server::resume_cwd_tests::strips_unix_worktree_cwd_to_project_root ... ok
test server::resume_cwd_tests::strips_windows_worktree_cwd_to_project_root ... ok
test server::tests::codex_desktop_files_only_block_returns_empty ... ok
test server::resume_cwd_tests::map_is_empty_and_does_not_panic_on_malformed_json ... ok
test server::resume_cwd_tests::map_resolves_second_real_world_project_path ... ok
test server::tests::codex_desktop_plain_block_passes_through_unchanged ... ok
test server::resume_cwd_tests::map_resolves_real_world_hyphenated_project_path ... ok
test hook_installer::tests::codex_hooks_json_unparseable_left_untouched ... ok
test hook_installer::tests::codex_features_creates_section_if_missing ... ok
test hook_installer::tests::kimi_config_creates_with_9_events ... ok
test hook_installer::tests::codex_hooks_json_creates_with_4_events ... ok
test hook_installer::tests::kimi_config_preserves_existing_content ... ok
test hook_installer::tests::codex_features_preserves_user_hooks_false_overwrites ... ok
test hook_installer::tests::kimi_config_blocks_have_only_allowed_fields ... ok
test server::tests::strips_codex_desktop_file_preamble_to_real_request ... ok
test shell_probe::tests::auto_is_default_for_unknown_string ... ok
test hook_installer::tests::codex_hooks_json_preserves_user_hooks ... ok
test hook_installer::tests::codex_features_enables_hooks_and_strips_legacy ... ok
test shell_probe::tests::end_to_end_detects_installed_pwsh_with_version ... ignored
test hook_installer::tests::kimi_config_reinstall_strips_stale_keeps_user_hooks ... ok
test shell_probe::tests::finds_msix_pwsh_alias ... ignored
test shell_probe::tests::json_mirror_hides_paths ... ok
test shell_probe::tests::parses_windows_ids ... ok
test shell_probe::tests::resolve_gitbash_falls_back_when_absent ... ok
test skills::tests::rel_path_validation ... ok
test shell_probe::tests::resolve_cmd_uses_system_path ... ok
test skills::tests::skill_name_validation ... ok
test shell_probe::tests::resolve_powershell_is_absolute ... ok
test terminal::tests::find_preset_known_tools ... ok
test terminal::tests::find_preset_resume_program_matches_tool ... ok
test terminal::tests::find_preset_unknown_returns_none ... ok
test server::tests::codex_parser_drops_subagent_source_unit_variant ... ok
test terminal::tests::claude_token_valid_uuid ... ok
test server::tests::codex_parser_drops_subagent_source_object ... ok
test terminal::tests::osc7_absent_returns_none ... ok
test terminal::tests::claude_token_rejects_injection ... ok
test terminal::tests::osc7_basic_with_hostname ... ok
test terminal::tests::osc7_embedded_in_larger_buffer ... ok
test terminal::tests::osc7_no_hostname ... ok
test terminal::tests::osc7_percent_encoded_chinese ... ok
test terminal::tests::osc7_percent_encoded_space ... ok
test terminal::tests::osc7_st_terminator ... ok
test terminal::tests::osc7_windows_style_path ... ok
test server::tests::codex_parser_keeps_legacy_session_without_thread_source ... ok
test terminal::tests::opencode_token_valid_format ... ok
test tool_config::tests::empty_entry_check ... ok
test server::tests::codex_parser_keeps_user_fork_session ... ok
test tool_config::tests::parse_simple ... ok
test windows_path::tests::already_present_skipped_case_insensitive_and_trailing_slash ... ok
test windows_path::tests::blank_entries_in_current_dont_shadow_candidates ... ok
test server::tests::codex_parser_keeps_user_main_session ... ok
test hook_installer::tests::codex_features_idempotent ... ok
test windows_path::tests::duplicate_candidates_deduped ... ok
test server::tests::codex_parser_drops_subagent_thread_source ... ok
test windows_path::tests::empty_current_joins_all_candidates ... ok
test windows_path::tests::empty_candidates_returns_current_unchanged ... ok
test windows_path::tests::mixed_present_and_new ... ok
test terminal::tests::opencode_token_rejects_invalid ... ok
test server::tests::counts_array_content_user_message_with_mixed_blocks ... ok
test tool_config::tests::expand_path_handles_tilde_and_passthrough ... ok
test hook_installer::tests::codex_hooks_json_idempotent_no_accumulation ... ok
test hook_installer::tests::kimi_config_idempotent_no_duplicates ... ok
test server::tests::keeps_session_that_continued_after_compaction ... ok
test server::tests::keeps_real_session_with_real_user_message ... ok
test windows_path::tests::real_path_dirs_reads_registry ... ok
test terminal::tests::hermes_token_valid_format ... ok
test terminal::tests::hermes_token_rejects_invalid ... ok
test server::tests::drops_pure_compaction_subtask_session ... ok
test server::tests::drizzle_scanner_excludes_subagent_and_archived_sessions ... ok

test result: ok. 80 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.09s
```

### 3. `npm install`：通过，有 npm 清理警告

在仓库内临时 HOME `D:\work\aicross\.f2-npm-home` 下执行，随后已删除该临时目录。没有使用用户真实 HOME。

命令：

```powershell
cd src-ui
npm install
```

输出原文：

```text
(node:39500) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
npm warn cleanup Failed to remove some directories [
npm warn cleanup   [
npm warn cleanup     'D:\\work\\aicross\\src-ui\\node_modules\\@rolldown\\binding-wasm32-wasi\\node_modules\\@emnapi\\wasi-threads',
npm warn cleanup     [Error: EPERM: operation not permitted, rmdir 'D:\work\aicross\src-ui\node_modules\@rolldown\binding-wasm32-wasi\node_modules\@emnapi\wasi-threads'] {
npm warn cleanup       errno: -4048,
npm warn cleanup       code: 'EPERM',
npm warn cleanup       syscall: 'rmdir',
npm warn cleanup       path: 'D:\\work\\aicross\\src-ui\\node_modules\\@rolldown\\binding-wasm32-wasi\\node_modules\\@emnapi\\wasi-threads'
npm warn cleanup     }
npm warn cleanup   ]
npm warn cleanup ]

up to date, audited 297 packages in 13s

136 packages are looking for funding
  run `npm fund` for details

7 vulnerabilities (2 low, 5 high)

To address issues that do not require attention, run:
  npm audit fix

To address all issues (including breaking changes), run:
  npm audit fix --force

Run `npm audit` for details.
npm notice
npm notice New major version of npm available! 10.9.8 -> 12.0.2
npm notice Changelog: https://github.com/npm/cli/releases/tag/v12.0.2
npm notice To update run: npm install -g npm@12.0.2
npm notice
```

退出码为 0。没有执行 `npm audit fix`，因为不属于本期范围。

### 4. `cd src-ui && npm run build`：通过

命令已实际完整通过两次。Vite 转换 554 个模块并生成 `dist/`；大 chunk 警告不影响退出码。当前执行环境存在间歇性 `spawn EPERM`：同一命令的部分重跑在 Vite 配置加载阶段被系统拒绝，但最终完整重跑通过。没有修改 Vite 配置或依赖来绕过执行环境。

命令：

```powershell
cd src-ui
npm run build
```

关键输出原文（逐资产明细共 300 余行，此处保留命令头、构建统计、主要产物与完整结尾）：

```text

> aicross-ui@0.0.0 build
> tsc -b && vite build

(node:31296) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
vite v8.0.7 building client environment for production...
transforming...✓ 554 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                                         2.51 kB │ gzip:   1.13 kB
dist/assets/main-DWy_JxXA.css                         130.93 kB │ gzip:  23.02 kB
dist/assets/main-lbtBKVDG.js                        1,192.68 kB │ gzip: 426.48 kB

✓ built in 2.79s
[plugin builtin:vite-reporter]
(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rolldownOptions.output.codeSplitting to improve chunking: https://rolldown.rs/reference/OutputOptions.codeSplitting
- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
```

补充执行 TypeScript 检查：

```powershell
cd src-ui
.\node_modules\.bin\tsc.cmd --noEmit -p tsconfig.app.json
```

stdout 为空，退出码为 0。

### 5. 指定残留自查：指定 `grep` 未验证；等价 `rg` 已完成

四条指定 `grep` 均已实际执行，但 Git for Windows 的 `grep.exe` 在当前环境创建共享内存映射时被系统拒绝，未进入搜索阶段。

命令：

```powershell
grep -rn '\.coffee-cli' src/ --include='*.rs'
```

输出原文：

```text
      0 [main] grep (3132) C:\Program Files\Git\usr\bin\grep.exe: *** fatal error - CreateFileMapping S-1-5-21-1830235385-2720854875-1956724854-1002.1, Win32 error 5.  Terminating.
```

命令：

```powershell
grep -rn 'COFFEE_CLI_' src/ --include='*.rs'
```

输出原文：

```text
      0 [main] grep (32308) C:\Program Files\Git\usr\bin\grep.exe: *** fatal error - CreateFileMapping S-1-5-21-1830235385-2720854875-1956724854-1002.1, Win32 error 5.  Terminating.
```

命令：

```powershell
grep -rn 'coffee-cli' src/server.rs
```

输出原文：

```text
      0 [main] grep (34324) C:\Program Files\Git\usr\bin\grep.exe: *** fatal error - CreateFileMapping S-1-5-21-1830235385-2720854875-1956724854-1002.1, Win32 error 5.  Terminating.
```

命令：

```powershell
grep -rn 'install_all()\|install_for_tool(' src/
```

输出原文：

```text
      0 [main] grep (19880) C:\Program Files\Git\usr\bin\grep.exe: *** fatal error - CreateFileMapping S-1-5-21-1830235385-2720854875-1956724854-1002.1, Win32 error 5.  Terminating.
```

随后使用等价只读命令完成内容审计。

命令：

```powershell
rg -n --glob '*.rs' '\.coffee-cli' src
```

输出原文：

```text
src\paths.rs:5://! Coffee CLI's `~/.coffee-cli/` tree.
```

唯一残留是 `paths.rs` 中“不得回退到上游目录”的历史边界注释；没有代码路径使用该目录。

命令：

```powershell
rg -n --glob '*.rs' 'COFFEE_CLI_' src
```

stdout 为空，退出码为 1（ripgrep 的“无匹配”状态）。

命令：

```powershell
rg -n 'coffee-cli' src/server.rs
```

输出原文：

```text
322:    let tmp_dir = std::env::temp_dir().join("coffee-cli").join("pasted-images");
389:        let tmp_dir = std::env::temp_dir().join("coffee-cli").join("pasted-images");
3635:    let out_path = std::env::temp_dir().join(format!("coffee-cli-update-setup.{ext}"));
4027:            project_root_from_cwd("/home/eben/coffee-cli/.claude/worktrees/foo"),
4028:            "/home/eben/coffee-cli"
4035:        assert_eq!(project_root_from_cwd("/home/eben/coffee-cli"), "/home/eben/coffee-cli");
4096:        let dir = std::env::temp_dir().join(format!("coffee-cli-test-{}", test_name));
4191:            "coffee-cli-drizzle-test-{}.db",
4293:            "coffee-cli-codex-test-{}-{}.jsonl",
4422:        let dir = std::env::temp_dir().join(format!("coffee-cli-compact-test-{}", std::process::id()));
4438:        let dir = std::env::temp_dir().join(format!("coffee-cli-real-test-{}", std::process::id()));
4456:        let dir = std::env::temp_dir().join(format!("coffee-cli-cont-test-{}", std::process::id()));
4479:        let dir = std::env::temp_dir().join(format!("coffee-cli-arr-test-{}", std::process::id()));
```

keyring 的 `coffee-cli` service 已消失。其余命中是既有临时文件夹名、更新下载临时文件名或测试夹具路径，不是本期列出的持久化目录、keyring、环境变量或 hook 标识。

命令：

```powershell
rg -n 'install_all\(\)|install_for_tool\(' src
```

输出原文：

```text
src\hook_installer.rs:98:pub fn install_all() {
src\hook_installer.rs:136:pub fn install_for_tool(tool: &str) {
src\server.rs:120:    crate::hook_installer::install_all();
```

前两处是保留的函数定义；第三处只在显式 `install_hooks` Tauri command 内。`setup`、工具重扫和终端启动路径中没有调用。前端等价搜索 stdout 为空：

```powershell
rg -n 'install_hooks|installHookForTool|install_hook_for_tool' src-ui/src
```

退出码为 1（无匹配）。

### 6. `install/`：符合预期，不存在

命令：

```powershell
ls install/
```

输出原文：

```text
Get-ChildItem:
Line |
   2 |  ls install/
     |  ~~~~~~~~~~~
     | Cannot find path 'D:\work\aicross\install\' because it does not exist.
```

补充检查：

```powershell
Test-Path -LiteralPath '.\install'
```

输出原文：

```text
False
```

## 未做与不确定处

- 没有启动应用窗口；任务书要求本期完成前不要启动，窗口手测留给沙箱外编排者。
- 没有执行任何 hook 安装函数或显式 `install_hooks` command。
- 没有迁移上游配置、凭据、技能、市场或会话数据。
- 没有修改 `LICENSE`、`NOTICE`、`TRADEMARKS.md`、`port/`、`docs/` 或 `installer.nsi`。
- 没有新增 npm 或 Cargo 依赖。
- 没有 commit，没有 push。
- 前端完整 Vite build 已通过，但当前环境中的 Vite 配置加载存在间歇性 `spawn EPERM`；成功重跑的产物与统计已记录。
- `npm install` 报告 7 个依赖漏洞与一个既有 `node_modules` 清理警告；未自动修复，避免超出本期范围或引入破坏性依赖升级。
