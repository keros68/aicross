# REPORT-F1 — fork 后的改名换标

## 结果

本期完成了 Coffee CLI fork 基座的用户可见改名换标：

- Tauri 产品名、窗口标题、bundle identifier 与版本改为 `aicross` / `com.aicross.desktop` / `0.1.0`。
- Rust package、binary、描述与仓库地址按任务书更新，`Cargo.lock` 随 `cargo check` 更新为 `aicross 0.1.0`。
- 11 个语言文件中的产品名统一为全小写 `aicross`；用户可见的 Gambit / Sentinel 文案改为“合并输入 / Merged input”和“会话监控 / Session monitor”及对应语言的中性表述。
- 前端 HTML 标题、字体预览、标题栏无障碍标签、终端积压提示、延迟诊断标题和写入外部 hook 配置的说明文字改为 aicross。
- `icons/`、favicon、标题栏标记和终端启动遮罩改为深色圆角方块加白色等宽 `AC`。新增无第三方依赖的 `scripts/generate-aicross-icons.mjs`，生成任务书要求的 PNG、ICO、ICNS。
- `installer.nsi` 本身没有硬编码品牌名；安装目录、快捷方式、卸载项名称均使用 Tauri 注入的 `{{product_name}}` / `${PRODUCTNAME}`，因此随 `tauri.conf.json` 的 `productName` 一并变为 aicross，无需改模板。

## 边界执行

- 没有删除功能、文件或业务逻辑。
- 没有移动或修改 `port/`。
- 没有修改 `docs/` 和 `tasks/` 下已有内容；本报告是新增文件。
- 没有新增 npm 或 Cargo 依赖。
- 没有修改源码版权头或 SPDX 行。
- 没有 commit，没有 push。
- 没有进行窗口手测；按任务书标为未验证。

`LICENSE`、`NOTICE`、`TRADEMARKS.md` 在修改前后 SHA-256 完全一致：

```text
0D96A4FF68AD6D4B6F1F30F713B18D5184912BA8DD389F86AA7710DB079ABCB0  LICENSE
B17A7807CE96FFC5A529CF69B3DFE5FFF9D962E278D09CDC4585B76E9AB01565  NOTICE
C9B0D077762FF49A1177D51D9B91B6C0234A513F96F3DB28E85FD277EC554AB3  TRADEMARKS.md
```

工作区在本期开始前已有大量 fork 基座改动，包括 `NOTICE`、`README.md`、`docs/` 和 `port/` 的状态；本期保留这些既有状态，没有回退或改写。

## C 节保留项与不确定处

以下内容保守保留，没有当作用户可见品牌改写：

- `Gambit.tsx`、`ActiveGambit.tsx`、`gambit-history.ts`，以及 `gambitOpen`、`TOGGLE_GAMBIT`、CSS 类名、console 前缀等：均为任务书明确要求保留的文件名或代码标识符。
- `sentinelRef`、`changes-sentinel` 等：是 IntersectionObserver 的通用“哨点”代码标识符，不是 Sentinel 品牌。
- `src-ui/src/components/center/TierTerminal.tsx` 和 `src-ui/src/store/app-state.tsx` 中的 Gambit / Sentinel 注释：仅描述上游实现和内部协议，不会显示给用户。
- `src/server.rs:3694`、`src/server.rs:3696` 的 Hyper-Agent：只存在于已退役入口的内部历史注释；实际功能描述是“供外部编排器使用的全局匿名 MCP server”，没有当前用户可见字符串，因此保留原注释以反映上游来源。
- `Coffee 101` 卡片和 `CoffeeCLI.com` 链接文字：它们明确指向上游课程/网站，不是本产品名称。改名会误述外部目的地，且会超出本期“不改业务逻辑”的边界，因此保留。
- `src-ui/package.json` / `package-lock.json` 的 `coffee-cli`：内部 npm package 标识，不面向用户；任务书只要求修改 Cargo package 标识，故保留。
- `capabilities/default.json` 的 Coffee CLI 描述：Tauri 内部 capability 元数据，不是界面文案，保留。
- `~/.coffee-cli` 目录、`COFFEE_CLI_*` 环境变量、`[COFFEE-*]` 协议标记、`coffee-cli-*` hook/plugin 文件名与 keyring service：属于持久化和跨进程兼容标识，改动会涉及迁移与业务逻辑，留待后续专门任务。
- `coffeecli.com` 更新端点、上游仓库/issue URL 和 `install/` 脚本：属于现有更新/安装业务与上游来源链接，本期不改业务逻辑；没有把它们当作 aicross 自有地址。
- 源码中 `Coffee CLI` 的其余命中都是上游实现说明、历史行为说明或测试夹具路径，不是运行时用户可见字符串；完整等价审计输出见下文。

## 验收

### 1. `cargo check`：通过

命令：

```powershell
cargo check
```

输出原文：

```text
   Compiling aicross v0.1.0 (D:\work\aicross)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.05s
```

### 2. `cd src-ui && npm run build`：未验证

命令已实际执行三次，均在 Vite 加载配置阶段因受限环境禁止 Node 子进程执行 `net use` 而失败；错误发生在打包开始前。没有改动 Vite 配置或 `node_modules` 来绕过沙箱。

命令：

```powershell
cd src-ui && npm run build
```

最终一次输出原文：

```text

> coffee-cli@0.0.0 build
> tsc -b && vite build

(node:23084) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
failed to load config from D:\work\aicross\src-ui\vite.config.ts
error during build:
Error: Build failed with 1 error:

[plugin externalize-deps]
Error: spawn EPERM
    at ChildProcess.spawn (node:internal/child_process:420:11)
    at spawn (node:child_process:787:9)
    at Object.execFile (node:child_process:349:17)
    at exec (node:child_process:236:25)
    at optimizeSafeRealPathSync (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:1954:2)
    at windowsSafeRealPathSync (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:1940:3)
    at getRealPath (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:32413:36)
    at tryResolveRealFileOrType (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:32407:9)
    at tryCleanFsResolve (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:32151:21)
    at tryFsResolve (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:32144:14)
    at aggregateBindingErrorsIntoJsError (file:///D:/work/aicross/src-ui/node_modules/rolldown/dist/shared/error-DBGOT6sf.mjs:48:18)
    at unwrapBindingResult (file:///D:/work/aicross/src-ui/node_modules/rolldown/dist/shared/error-DBGOT6sf.mjs:18:128)
    at #build (file:///D:/work/aicross/src-ui/node_modules/rolldown/dist/shared/rolldown-build-hRnqgxyz.mjs:3313:34)
    at async bundleConfigFile (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:34633:17)
    at async bundleAndLoadConfigFile (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:34541:18)
    at async loadConfigFromFile (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:34510:42)
    at async resolveConfig (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:34134:22)
    at async createBuilder (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/chunks/node.js:33351:17)
    at async CAC.<anonymous> (file:///D:/work/aicross/src-ui/node_modules/vite/dist/node/cli.js:767:19) {
  errors: [Getter/Setter]
}
```

### 3. `cd src-ui && npx tsc --noEmit -p tsconfig.app.json`：通过

命令：

```powershell
cd src-ui && npx tsc --noEmit -p tsconfig.app.json
```

输出原文：

```text
(node:3132) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
```

### 4. 指定残留自查：未验证；等价 `rg` 已完成

两条指定 `grep` 命令均已实际执行。Git for Windows 的 `grep.exe` 在此沙箱内初始化共享内存映射时被系统拒绝，未进入搜索阶段。

命令：

```powershell
grep -rn 'Coffee CLI' src src-ui/src tauri.conf.json Cargo.toml installer.nsi
```

输出原文：

```text
      0 [main] grep (20120) C:\Program Files\Git\usr\bin\grep.exe: *** fatal error - CreateFileMapping S-1-5-21-1830235385-2720854875-1956724854-1002.1, Win32 error 5.  Terminating.
```

命令：

```powershell
grep -rn 'Gambit\|Sentinel\|Hyper-Agent' src-ui/src/i18n/
```

输出原文：

```text
      1 [main] grep (25640) C:\Program Files\Git\usr\bin\grep.exe: *** fatal error - CreateFileMapping S-1-5-21-1830235385-2720854875-1956724854-1002.1, Win32 error 5.  Terminating.
```

随后使用等价只读命令完成内容审计：

```powershell
rg -n 'Coffee CLI' src src-ui/src tauri.conf.json Cargo.toml installer.nsi
```

输出原文：

```text
Cargo.toml:95:# Prevents two Coffee CLI processes from running side-by-side. Without this,
Cargo.toml:113:#     a closed Coffee CLI and lock `~/.claude/` session files.
src\git.rs:23:/// Per-repo session baseline: the HEAD captured when Coffee CLI first saw
src\git.rs:99:        /// Coffee CLI's audience doesn't manually `git add`; the 3-way split
src\git.rs:105:        /// Commits made since this Coffee CLI window opened (baseline..HEAD),
src\git.rs:382:    // Session commits: commits made since this Coffee CLI window opened
src\hook_forwarder.rs:1:// Coffee CLI — native hook forwarder
src\hook_forwarder.rs:7:// argv). Maps the event to Coffee CLI's 3-state agent status and forwards a
src\hook_forwarder.rs:34:/// Env injected by Coffee CLI when spawning a tool in a tab (see
src\hook_forwarder.rs:37:/// Codex `notify` silent for sessions started outside Coffee CLI.
src\hook_forwarder.rs:123:    // outside Coffee CLI too — gate strictly on the tool tag.
src\hook_forwarder.rs:144:/// stays silent for Codex sessions started outside Coffee CLI.
src\hook_forwarder.rs:171:/// HookCtx::from_env: kimi sessions started outside Coffee CLI don't carry
src\fs_watcher.rs:3:// Coffee CLI's brand promise is "beautified host for AI CLIs". That means
src\fs_watcher.rs:33:/// Coffee CLI's diff/snapshot is intended for source workspaces — the
src\hook_installer.rs:1:// Coffee CLI Hook Installer
src\hook_installer.rs:6:// The forwarder is the Coffee CLI binary itself (`<exe> __hook` /
src\hook_installer.rs:53:// name silently invalidated the whole config in Coffee CLI ≤ v1.8.5.
src\hook_installer.rs:56:// Coffee CLI from starting.
src\hook_installer.rs:68:/// Argv markers for the native forwarder built into the Coffee CLI binary
src\hook_installer.rs:83:/// Events Coffee CLI listens for. Mirrors vibe-notch (ClaudeIsland)'s
src\hook_installer.rs:115:    // spawn the full Coffee CLI GUI and stall grok's TUI). One-time migration;
src\hook_installer.rs:134:/// so users who install a CLI while Coffee CLI is running don't have
src\hook_installer.rs:189:/// makes Coffee CLI's terminal bg — and the Glass theme's wallpaper blur —
src\hook_installer.rs:195:/// Theme value Coffee CLI used to write into tui.json before we discovered
src\hook_installer.rs:211:    // The hook command is the Coffee CLI binary itself: `<exe> __hook`. No
src\hook_installer.rs:234:    // Strip stale Coffee CLI entries from settings.local.json (v1.8.5 wrote
src\hook_installer.rs:251:/// top-level notify. The forwarder is the Coffee CLI binary itself (no
src\hook_installer.rs:431:/// what actually lets Coffee CLI's terminal bg (and the Glass theme's wallpaper
src\hook_installer.rs:530:/// Remove every Coffee CLI hook entry from `path` without touching any other
src\hook_installer.rs:778:/// Build a Coffee CLI hook command: `"<exe>" <subcommand>`. The agent runs
src\hook_installer.rs:1198:// Coffee CLI launched), cmd.exe holds a lock on opencode.cmd — npm renames
src\hook_installer.rs:1204:// process). But at Coffee CLI launch — when opencode is almost certainly
src\skills.rs:1://! Coffee CLI Skills — local skill store, surfaced into agent prompts.
src\skills.rs:29:/// Coffee CLI's bundled skills, baked into the binary at compile time.
src\skills.rs:31:/// `skills/` at the repo root and ship as part of Coffee CLI's
src\skills.rs:40:/// it's owned by Coffee CLI; upstream changes are not auto-tracked.
src\skills.rs:47:/// + Coffee CLI's own skills) always ships every skill, but only the
src\skills.rs:55:///   - `vibeid` — Coffee CLI's own; ships scripts/ + matrix.json,
src\skills.rs:64:///   - `playwright` — Coffee CLI's own. Browser automation skill that
src\skills.rs:103:/// Make sure the canonical Coffee CLI skill dirs exist AND the bundled
src\skills.rs:116:/// once on first install so user-side patches survive Coffee CLI
src\skills.rs:117:/// upgrades. Updating these on every launch means a Coffee CLI
src\skills.rs:169:        // a later Coffee CLI release). Refresh both library and enabled
src\skills.rs:196:/// already-seeded skill on Coffee CLI upgrade — picks up new icons,
src\skills.rs:310:/// List every skill Coffee CLI knows about, with enable status and raw
src\server.rs:116:/// CLI while Coffee CLI is running pick up tab status indicators
src\server.rs:600:    //   3. empty → spawn process inherits Coffee CLI's own cwd
src\server.rs:920:/// Coffee CLI window (other Space, app switched away, minimized) we
src\server.rs:931:/// can be inactive this way while the Coffee CLI window itself is still
src\server.rs:1014:    // titles for the history list. Coffee CLI no longer ships a
src\server.rs:1054:///   • covers any tool Coffee CLI launched inside such a worktree (they all
src\server.rs:3706:// Lets users tell Coffee CLI things like "my claude is at
src\server.rs:3738:    // plugin's init touches state. When a user double-launches Coffee CLI,
src\server.rs:3746:    // (devs working on Coffee CLI inside Coffee CLI). Both builds otherwise
src\server.rs:3965:            // Issue #28: closing Coffee CLI without first killing tabs
src\marketplace.rs:3://! Coffee CLI consumes any git repo that follows the Codex plugin-market
src-ui/src\utils\skill-meta.ts:39: * Each skill self-declares which languages it supports — Coffee CLI's
src\terminal.rs:16:/// window so a Coffee CLI window left in the background drops to near-zero
src\terminal.rs:25:// When Coffee CLI's process dies — clean exit, crash, or task-manager kill —
src\terminal.rs:27:// job. Without this, orphan `claude.exe` survived past Coffee CLI shutdown
src\terminal.rs:149:// with whatever we set on `theme.background`, which Coffee CLI flips between
src\terminal.rs:377:    // real-TUI verification. Coffee CLI launches grok raw (native OAuth via
src\terminal.rs:492:    /// Coffee CLI window itself is still focused and foreground. Defaults
src\terminal.rs:628:        // Coffee CLI's light/dark switch automatically. A no-op once the user
src\terminal.rs:756:    // leave installed even when Coffee CLI isn't the launcher.
src\terminal.rs:807:    // mutating Coffee CLI's own process-wide env.
src\terminal.rs:823:    // on child.wait() to detect process death — Coffee CLI's long-standing
src\terminal.rs:832:    // Coffee CLI's process. Belt-and-suspenders with the ExitRequested
src\terminal.rs:868:    // Coffee CLI's primary "terminal locks up after a while" failure mode:
src\terminal.rs:1170:        // can see this output right now — either the whole Coffee CLI window
src\hook_server.rs:1:// Coffee CLI Hook Server
src\launch.rs:3://! Lets launchers, context menus, and scripts open Coffee CLI directly
src\launch.rs:19://!   macOS:   open -a "Coffee CLI" --args launch --tool kimicode --cwd /work/proj
src-ui/src\App.tsx:218:  // no longer have to restart Coffee CLI to see newly-created sessions
src-ui/src\tauri.ts:75:      /** Commits made since this Coffee CLI window opened (baseline..HEAD),
src-ui/src\tauri.ts:137:   *  widens its sleep / coalesce window so a backgrounded Coffee CLI
src-ui/src\tauri.ts:201:   *  picks up tab status indicators without forcing a Coffee CLI
src-ui/src\tauri.ts:255:  // ── Skills (Coffee CLI skill store, junctioned into ~/.claude/skills + ~/.codex/skills) ──
src-ui/src\tauri.ts:264:   *  install); Coffee CLI doesn't clobber those, so the user keeps
src-ui/src\tauri.ts:356: * empty strings / empty arrays fall through to Coffee CLI's built-in
src-ui/src\main.tsx:61:// When the OS hides the Coffee CLI window (other Space, app switched
src\tools\antigravity.rs:4://! sunsets 2026-06-18. Coffee CLI swapped the slot wholesale rather than
src\tools\antigravity.rs:26://!   │                            consumed by Coffee CLI).
src\tools\antigravity.rs:29://!   │                            Coffee CLI doesn't read these.
src\tools\antigravity.rs:60://!                                            reads it. Coffee CLI does
src\tools\antigravity.rs:86://!     is a separate richer mechanism than our skills dir. Coffee CLI
src-ui/src\styles\global.css:2:   Coffee CLI — 3-Panel IDE App Styling
src-ui/src\store\app-state.tsx:1:// Coffee CLI — Global App State (React Context)
src-ui/src\store\app-state.tsx:35:// Self-authored (2): outline (line-frame), coffee (Coffee CLI brand)
src-ui/src\components\center\CenterPanel.tsx:632:  // not-installed → installed during a Coffee CLI session. `null`
src-ui/src\components\center\CenterPanel.tsx:813:  // external terminal without forcing them to restart Coffee CLI. The
src-ui/src\components\center\CenterPanel.css:275:     washed black — important because most Coffee CLI themes have a
src\tools\mod.rs:2://! per-CLI facts Coffee CLI needs (binary name, skills dir, history
src\tools\mod.rs:13:/// shape it lives in. Coffee CLI's history scanner (`server.rs`)
src\tools\mod.rs:195:    /// `true` if Coffee CLI installs a status-indicator hook for
src-ui/src\icons-inline\pi.svg:5:       Pi mark stays visible across light and dark Coffee CLI themes. */
src\tools\hermes.rs:42:    // Python plugins. Coffee CLI installs `coffee-cli-status` for the
src\tools\hermes.rs:66:            // process CWD — and Coffee CLI changes CWD per tab via the
src-ui/src\components\center\ToolConfigModal.tsx:6:// are optional — empty falls through to Coffee CLI's built-in default.
src-ui/src\components\center\ToolConfigModal.tsx:69:// Tools whose session history Coffee CLI's history scanner actually reads
src\tools\grok.rs:21://! history scanner (summary.json `info.id`), not PTY scraping. Coffee CLI
src-ui/src\components\center\TierTerminal.tsx:1045:    // "Coffee CLI_3.0.3...exe") can't be reliably bounded by a regex (would
src-ui/src\components\center\TierTerminal.tsx:1272:          // see git history — it read as Coffee CLI editorializing over the
src-ui/src\components\center\TierTerminal.tsx:1960:          generic Coffee CLI verdict on top either echoes that message
src-ui/src\icons-inline\grok.svg:3:     launchpad / history text color across light and dark Coffee CLI themes
src-ui/src\lib\shiki.ts:49: *  Coffee CLI has 9 ThemeColor values (dark/light/cappuccino/sakura/...)
src-ui/src\lib\terminal-output-scheduler.ts:13:// This module is the Coffee CLI port of Orca's `pane-terminal-output-scheduler`:
src-ui/src\lib\use-data-attr.ts:3:// where Coffee CLI parks `data-theme` and `data-shape` per App.tsx).
```

上述每一处保留原因如下：

- `Cargo.toml:95,113`：依赖/进程行为的上游源码注释。
- `src/git.rs` 全部命中：上游会话基线和提交分组行为的源码注释。
- `src/hook_forwarder.rs` 全部命中：上游 hook 协议、环境变量和迁移背景的源码注释。
- `src/fs_watcher.rs` 全部命中：上游 watcher/diff 设计意图的源码注释。
- `src/hook_installer.rs` 全部命中：上游 hook 安装、旧版本兼容和故障背景的源码注释；实际写入用户配置的三处字符串已经改为 aicross。
- `src/skills.rs` 全部命中：上游 skill store 的来源、所有权和升级语义说明。
- `src/server.rs` 全部命中：上游窗口、工作区、历史扫描、单实例和退出清理行为的源码注释。
- `src/marketplace.rs`：上游 marketplace 兼容格式说明。
- `src/terminal.rs` 全部命中：上游 PTY、主题、后台节流和子进程清理行为的源码注释。
- `src/hook_server.rs`：上游模块标题注释。
- `src/launch.rs`：上游外部启动功能示例和历史命令说明。
- `src/tools/*.rs` 全部命中：上游各 CLI 适配器的实现背景注释。
- `src-ui/src/App.tsx`、`main.tsx`、`tauri.ts`、`utils/skill-meta.ts`：上游运行时接口、会话刷新和 skill 行为的源码注释。
- `src-ui/src/styles/global.css`、`store/app-state.tsx`、`CenterPanel.css`、`icons-inline/*.svg`、`lib/*.ts`：上游样式、主题、状态和移植来源注释。
- `src-ui/src/components/center/*.tsx` 的命中：上游内部行为、旧版路径和兼容判断注释；不会渲染到界面。

第二条内容审计使用：

```powershell
rg -n 'Gambit|Sentinel|Hyper-Agent' src-ui/src/i18n/
```

stdout 为空，退出码为 1（ripgrep 的“无匹配”状态）。11 个语言文件中没有这三个大小写形式的残留。

### 5. `cargo test`：通过

命令：

```powershell
cargo test
```

输出原文：

```text
   Compiling aicross v0.1.0 (D:\work\aicross)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 6.62s
     Running unittests src\main.rs (target\debug\deps\aicross-8f73a09b6736e052.exe)

running 82 tests
test launch::tests::ignores_non_launch_argv ... ok
test server::resume_cwd_tests::does_not_split_on_a_dot_inside_a_component ... ok
test launch::tests::rejects_unknown_tool ... ok
test launch::tests::parses_tool_with_and_without_argv0 ... ok
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
test server::tests::codex_desktop_plain_block_passes_through_unchanged ... ok
test server::tests::codex_desktop_files_only_block_returns_empty ... ok
test hook_installer::tests::codex_features_enables_hooks_and_strips_legacy ... ok
test hook_installer::tests::codex_features_creates_section_if_missing ... ok
test hook_installer::tests::codex_hooks_json_unparseable_left_untouched ... ok
test hook_installer::tests::codex_hooks_json_creates_with_4_events ... ok
test hook_installer::tests::kimi_config_preserves_existing_content ... ok
test hook_installer::tests::codex_features_preserves_user_hooks_false_overwrites ... ok
test hook_installer::tests::codex_features_idempotent ... ok
test hook_installer::tests::kimi_config_blocks_have_only_allowed_fields ... ok
test hook_installer::tests::kimi_config_creates_with_9_events ... ok
test server::resume_cwd_tests::map_resolves_real_world_hyphenated_project_path ... ok
test server::resume_cwd_tests::map_resolves_second_real_world_project_path ... ok
test server::tests::strips_codex_desktop_file_preamble_to_real_request ... ok
test shell_probe::tests::auto_is_default_for_unknown_string ... ok
test shell_probe::tests::end_to_end_detects_installed_pwsh_with_version ... ignored
test shell_probe::tests::finds_msix_pwsh_alias ... ignored
test shell_probe::tests::json_mirror_hides_paths ... ok
test shell_probe::tests::parses_windows_ids ... ok
test shell_probe::tests::resolve_gitbash_falls_back_when_absent ... ok
test shell_probe::tests::resolve_cmd_uses_system_path ... ok
test server::resume_cwd_tests::map_is_empty_and_does_not_panic_on_malformed_json ... ok
test shell_probe::tests::resolve_powershell_is_absolute ... ok
test skills::tests::rel_path_validation ... ok
test skills::tests::skill_name_validation ... ok
test terminal::tests::find_preset_known_tools ... ok
test terminal::tests::find_preset_resume_program_matches_tool ... ok
test server::tests::codex_parser_drops_subagent_source_unit_variant ... ok
test terminal::tests::find_preset_unknown_returns_none ... ok
test server::tests::codex_parser_drops_subagent_source_object ... ok
test server::tests::codex_parser_drops_subagent_thread_source ... ok
test terminal::tests::claude_token_valid_uuid ... ok
test terminal::tests::claude_token_rejects_injection ... ok
test terminal::tests::osc7_absent_returns_none ... ok
test terminal::tests::osc7_basic_with_hostname ... ok
test terminal::tests::osc7_no_hostname ... ok
test terminal::tests::osc7_embedded_in_larger_buffer ... ok
test terminal::tests::osc7_percent_encoded_chinese ... ok
test terminal::tests::osc7_percent_encoded_space ... ok
test terminal::tests::osc7_windows_style_path ... ok
test hook_installer::tests::codex_hooks_json_preserves_user_hooks ... ok
test terminal::tests::osc7_st_terminator ... ok
test tool_config::tests::empty_entry_check ... ok
test tool_config::tests::parse_simple ... ok
test terminal::tests::opencode_token_valid_format ... ok
test hook_installer::tests::kimi_config_reinstall_strips_stale_keeps_user_hooks ... ok
test terminal::tests::opencode_token_rejects_invalid ... ok
test windows_path::tests::already_present_skipped_case_insensitive_and_trailing_slash ... ok
test hook_installer::tests::codex_hooks_json_idempotent_no_accumulation ... ok
test server::tests::codex_parser_keeps_legacy_session_without_thread_source ... ok
test windows_path::tests::blank_entries_in_current_dont_shadow_candidates ... ok
test windows_path::tests::duplicate_candidates_deduped ... ok
test windows_path::tests::empty_candidates_returns_current_unchanged ... ok
test server::tests::codex_parser_keeps_user_fork_session ... ok
test server::tests::codex_parser_keeps_user_main_session ... ok
test windows_path::tests::empty_current_joins_all_candidates ... ok
test windows_path::tests::mixed_present_and_new ... ok
test server::tests::keeps_real_session_with_real_user_message ... ok
test server::tests::keeps_session_that_continued_after_compaction ... ok
test hook_installer::tests::kimi_config_idempotent_no_duplicates ... ok
test server::tests::drops_pure_compaction_subtask_session ... ok
test tool_config::tests::expand_path_handles_tilde_and_passthrough ... ok
test terminal::tests::hermes_token_valid_format ... ok
test windows_path::tests::real_path_dirs_reads_registry ... ok
test server::tests::counts_array_content_user_message_with_mixed_blocks ... ok
test terminal::tests::hermes_token_rejects_invalid ... ok
test server::tests::drizzle_scanner_excludes_subagent_and_archived_sessions ... ok

test result: ok. 80 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.09s
```

### 图标检查

生成命令：

```powershell
node .\scripts\generate-aicross-icons.mjs
```

生成结果包括：

```text
32x32.png: 32x32
128x128.png: 128x128
128x128@2x.png: 256x256
256x256.png: 256x256
512x512.png: 512x512
icon.png: 512x512
icon.ico: 16, 20, 24, 32, 40, 48, 64, 96, 128, 256 px
icon.icns: 16, 32, 64, 128, 256, 512, 1024 px
```

Pillow 只读打开检查结果：

```text
32x32.png: format=PNG, size=(32, 32), frames=1
128x128.png: format=PNG, size=(128, 128), frames=1
128x128@2x.png: format=PNG, size=(256, 256), frames=1
256x256.png: format=PNG, size=(256, 256), frames=1
512x512.png: format=PNG, size=(512, 512), frames=1
icon.png: format=PNG, size=(512, 512), frames=1
icon.ico: format=ICO, size=(256, 256), frames=1
icon.icns: format=ICNS, size=(1024, 1024), frames=1
```

已打开 `icons/icon.png` 与 `icons/32x32.png` 检查：深色圆角方块、白色等宽 `AC` 可见，小尺寸可辨认。
