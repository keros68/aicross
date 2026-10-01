# REPORT-40 窗口其余界面下线

分支 `ui/retire40`，基于 TASK-40 任务书。R0 已 commit（`ee6d0a7 TASK-40 R0`），未 push；fix-40-r1 R1 的改动在工作区，未 commit。

## 删了什么

### Rust：整文件删除（行数为 HEAD 上的行数）

| 文件 | 行数 | 原用途 |
|---|---|---|
| `src/terminal.rs` | 1579 | PTY 终端（`assign_process_to_job` 与 Job Object 一节挪进 `src/process.rs`，引擎派发仍需要） |
| `src/launch.rs` | 150 | `launch --tool` 子命令 |
| `src/hook_server.rs` | 246 | hook 回环 TCP 服务 |
| `src/hook_forwarder.rs` | 298 | `__hook` / `__codex-hook` / `__kimi-hook` 转发器（main.rs 的对应分支一并删） |
| `src/hook_installer.rs` | 1650 | 向各家 CLI 装 hook |
| `src/marketplace.rs` | 429 | 插件市场 |
| `src/skills.rs` | 472 | 技能库 |
| `src/fs_watcher.rs` | 279 | 文件树监听 |
| `src/fonts.rs` | 162 | 系统字体列表 |
| `src/shell_probe.rs` | 775 | shell 探测 |
| `src/console_api.rs` | 845 | 窗口 HTTP 通道与 `console.json` / `console.dev.json` 清单 |
| `src/tools/`（16 个文件） | 865 | 工具注册表（`resources.rs` 里引用它的测试改为本地断言） |

### Rust：只服务旧界面的命令与机器，模块保留

- `server.rs` 4722 → 229 行：只留 `window_minimize / window_maximize / window_close / show_main_window / take_pending_open`，新增 `list_runs`，发布构建的单实例转发只认 `open`，窗口揭示兜底、Windows 方角、macOS 关窗隐藏与 Dock 恢复照旧。
- `dispatch.rs` 11262 → 8953 行：删蜂群计划核对与合并（`check_swarm_plan*`、`merge_swarm_items*` 及其全部辅助函数与测试）、`preview_dispatch / dispatch_graph_stage / dispatch_stage`、`inspect_dispatch_state / reconcile_dispatch_state / rollback_node_changes / abandon_dispatch_state` 命令与结构体、脱敏计数（`redaction_count` / `broad_redaction_count`）。引擎在用的核心（`dispatch_stage_core`、`cancel_dispatch_node`、快照回滚、红绿核对、worktree 与只读项目副本路径）未动。
- `orchestration.rs` 1748 → 1069 行：删 12 条 Tauri 命令包装、commander.json 与 premises.json 读写、`presets_from` / `fill_role_defaults`（角色默认由引擎 `apply_role_defaults` 负责）、`list_graphs_in` / `GraphMeta`、`save_user_presets_in`。
- `resources.rs` 4799 → 3931 行：删 11 条 Tauri 命令包装（`get_resources`、`detect_*`、`check_pi_provider_login*`、增改删模型、手动 agent、批量冒烟）及其独占辅助与测试；CLI 在用的 `*_at` 函数（`get_resources_at`、`detect_all_resources_at`、`smoke_resource_model_at`、`smoke_agent_third_party_at` 等）原样保留。
- `channels/acp.rs` 2048 → 1723 行：删 6 条 Tauri 命令（权限卡片的应答/关闭、`/` 命令清单、池释放、适配器探测）、界面事件发送（`emit` / `install_app_handle` / 三个事件常量）、「关 tab 即拒」的 `closed_runs` 机制；权限挂起登记、超时拒绝、`test_answer_pending` 与交互会话路径保持原行为。`acp_setup.rs` 删掉界面进度事件，安装逻辑不变。
- `console_cli.rs` 1885 → 1678 行：删 `propose` 子命令（run_if_subcommand、build_request 的 propose 分支、`--reason` 校验、HTTP 客户端 `send` / 清单读取 / 进程存活核对及全部相关测试）。`dispatch / status / adopt / cancel / resources / config / __run` 行为与退出码不变。
- `git.rs` 472 → 381 行：删 `git_show_file / git_init / git_capture_baseline / git_commit_files / commit_files`；`git_changes` 去掉命令属性（引擎 `git_dirty_count` 在用），`normalize_path_key` 从 server.rs 挪进来。
- `tool_config.rs` 217 → 111 行：删 `set / save / expand_path / history_path_for / is_empty`；`get / parse_command` 保留（资源探测解析手动 agent 的二进制）。`default_cwd` / `history_path` 字段保留只作格式兼容。

### 前端：删除（src-ui/src 下 193 个文件、约 54 400 行）

`components/center|left|right|resources`、`graph/`、`store/`、`i18n/`、`icons-inline/`、`commander/`、`resources/`、`utils/`、`lib/`（`platform.ts` 保留，R0 实际留了 `IS_MACOS` 与 `IS_WINDOWS` 两个导出，`IS_WINDOWS` 已无人引用；fix-40-r1 R1-9 删掉后只剩 `IS_MACOS`）、`console-*.ts` 七个文件、`components/common` 里的 SettingsModal / FontPicker / ScrollPanel / ErrorBoundary、`__tests__/` 里 14 个旧界面测试（含 `engine-golden.test.ts`）；`public/fonts`（7 个 woff2）与 `public/icons`（Explorer/终端的图标树）。

### 依赖删除

- npm（`package.json`，`package-lock.json` 随 `npm install` 更新）：`@xterm/xterm`、`@xterm/addon-fit`、`@xterm/addon-web-links`、`@xterm/addon-webgl`、`diff`、`shiki`、`@tauri-apps/plugin-clipboard-manager`、`@tauri-apps/plugin-dialog`、`@tauri-apps/plugin-global-shortcut`。
- Cargo（`Cargo.toml`）：`portable-pty`、`tokio`、`clap`、`glob`、`log`、`tauri-plugin-dialog`、`tauri-plugin-clipboard-manager`、`tauri-plugin-global-shortcut`、`ttf-parser`、`base64`、`png`、`ureq`、`include_dir`、`notify`、`notify-debouncer-full`、`rusqlite`、`keyring`、`rmcp`、`axum`。`windows` 的 `Win32_Security` 特性一度删除后恢复——`CreateJobObjectW` / `CreateProcessW` 的签名引用 `SECURITY_ATTRIBUTES`，编译器抓回来的。
- `capabilities/default.json` 收缩为标题栏拖动、缩放、显窗与事件监听；`capabilities/desktop.json` 删除（只剩 global-shortcut）。`tauri.conf.json` 关闭 `assetProtocol`（原 `protocol-asset` 特性随 tauri 依赖一并移除）。

## 保留与搬动

- **查看器纯函数各归其位**：`ConsoleMarkdown.tsx`（原 components/center）、`vendor.ts`（原 graph/vendor.ts）、`agent-display.ts`（原 resources/agent-display.ts）搬到 `src-ui/src/viewer/`；`parseAcceptance` 与 `compactRunNumber` 内联进 `viewer/run-view.ts`（实现照抄原函数，行为不变，`run-viewer-37` 与 markdown 测试原样跑通）。
- **首页（新增）**：`viewer/Home.tsx` + `viewer/Home.css` + `viewer/run-list.ts`（纯函数：列表项文案、开始时间、空状态文案；run 级状态词与状态胶囊当时从 `run-view.ts` 复制了一份，fix-40-r1 R1-11 已收口回 `run-view.ts`）+ `viewer/__tests__/home.test.ts`（5 条）。
- **`list_runs`（新增）**：`run_view.rs` 里 `RunListItem` + `list_runs_at`，只认 `.json` 且文件名不以 `.` 开头（与引擎 `active_runs` 同口径），按 `updated_at` 倒序取 50 条，模式名与任务名复用查看页的 `preset_display_name` / `task_title`；配套 4 条测试（排序与 50 条上限、扩展名过滤、坏文件跳过与文件名兜底、取名口径、空目录）。
- 引擎（`engine/`、`dispatch.rs` 核心、`channels/`、`process.rs`）、资源探测（`resources.rs` 的 `*_at`）、`run_view.rs`、`console_cli.rs`、`git.rs`、`tool_config.rs`、`windows_path.rs`、`paths.rs` 原样保留；`process.rs` 增补从 terminal.rs 挪来的 kill-on-close Job Object（`assign_process_to_job`）。
- `LICENSE`、`NOTICE`、`TRADEMARKS.md` 未动；`windows_path.rs`、`installer.nsi`、`windows/fragments/path.wxs` 未动；README 仍写明 fork 自 Coffee CLI。
- 夹具：`fixtures/engine-golden/` 31 个夹具保留，新增 `README.md` 说明来源与改动要求；`engine::golden::engine_golden_fixtures_replay` 照常通过。

## 文档

- `docs/MANUAL.md` 改写为查看器与命令行手册（安装、命令表、资源接入、模式、首页、查看页、留痕、密钥与脱敏、术语表）。
- `docs/CONSOLE-CHANNEL.md` 删掉 `propose`、HTTP 协议、清单文件与令牌段落，只留命令行。
- `README.md` 改为现行形态（一句话、用法、开发命令）。
- 仓库 `AGENTS.md`：「技术栈」删掉 xterm/PTY，「与 coffeecli 的关系」注明上游终端等已下线。

## 验证（五条命令的实际输出）

1. `cargo check --all-targets`
   ```
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.13s
   ```
   warning/error 计数 0（`grep -cE "^warning|^error"` 输出 0）。
2. `cargo check --release`
   ```
   Finished `release` profile [optimized] target(s) in 5.00s
   ```
3. `cargo test`
   ```
   test result: ok. 406 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 44.68s
   ```
   其中含 `engine::golden::engine_golden_fixtures_replay ... ok`。
4. `cd src-ui && npx vitest run --configLoader native --pool threads`
   ```
   Test Files  3 passed (3)
        Tests  22 passed (22)
   ```
5. `cd src-ui && npm run build`
   ```
   ✓ built in 209ms
   dist/assets/main-…js 20.19 kB │ vendor 172.34 kB │ react 190.93 kB
   ```

换行核对（fix-40-r1 R1 已按文件逐个重验并改正本段）：`src/server.rs`（CRLF）与 `src/main.rs`（CRLF）逐行 CRLF 无误；`docs/PRODUCT.md` 原文件就是 CRLF，本轮未动、保持 CRLF；`src-ui/package-lock.json` 与 `README.md` 原为 CRLF，本轮被写成 LF（fix-40-r1 R1-8 改回 CRLF，内容与改前逐字节一致）；其余改动文件 LF。`src-ui/src/tauri.ts` 原是混合换行（858 行中 661 行 CRLF、197 行 LF，CRLF 段不止开头一处），整文件重写后统一为 LF（新文件 LF 约定，无法「保持」混合换行）。

改动总量：434 个文件，+6 648 / −83 024 行。

## 未做与理由

- **任务书末节的编排者真机核**（`tauri build --no-bundle` 出发布版、点首页进查看页、冷热启动、真派一次到 done）：任务书写明实现者不做，留编排者执行。
- **`docs/PRODUCT.md` 未动**：任务书第 4 节只列了 MANUAL、CONSOLE-CHANNEL、README、AGENTS 四份。PRODUCT「资源接入是命令」一节末尾「查看器里只读展示」一句与现状不符（查看页已无资源展示），留给编排者裁定。
- `docs/ACCEPTANCE-0916.md`、`docs/proto/`：历史记录，按仓库约定不作现行文档处理。
- 蜂群的命令行合并（cherry-pick 流水线）没有随窗口界面搬进引擎：PRODUCT 定了「无窗口版另起一期」，本期只删不建。
- `resize_icon.js`（一次性图标处理脚本，依赖未安装的 sharp）不属于旧界面运行时代码，保留。

## 不确定处

- **首页轮询**：查看页轮询是任务书定死的（运行中每 2 秒）；首页列表任务书未定刷新方式，我按「窗口开着时每 5 秒读一次 run 目录」实现，代价是一次目录读取。若嫌多可改成手动刷新。
- **首页错误展示**：`list_runs` 失败时保留旧列表并继续重试，从未成功过才显示空状态文案；错误详情以小字附在空状态下方。
- **`src-ui/src/tauri.ts` 换行**：原文件是混合换行（不止「前 91 行 CRLF」一处，见上面换行核对的逐文件结果），整写后统一 LF。
- **`status` 输出的 `presetName`**：首页列表的模式名直接复用查看页取名函数；旧 run 文件没有 `preset_id` 时显示「自定义模式」，与查看页一致。
- **首页状态**：R0 直接取 run 文件里的 `status`，进程已死的 running 会显示成「运行中」；fix-40-r1 R1-3 已改用引擎的只读死进程判定。
- **任务书第 5 节「取名」测试**：list_runs 的取名口径在 Rust 侧测了（`list_runs_names_preset_and_task_like_the_viewer`）；前端 `runListItemText` 只负责「模式名 · 任务名」的拼装与缺省回退，各自覆盖。

## R1（fix-40-r1，10-01）

依据 `tasks/fix-40-r1.md`。R0 已 commit（`ee6d0a7`），本轮改动在工作区，未 commit、未 push。

### 必修逐条

**R1-1 macOS 编译失败**（`src/server.rs`）
`.run(...)` 的闭包参数从 `_app_handle` 改回 `app_handle`，macOS 的 Reopen 分支照旧用它取窗口；非 macOS 分支加 `#[cfg(not(target_os = "macos"))] let _ = &app_handle;` 消未使用警告。**未能在本机编译 macOS**：本机只装了 `x86_64-pc-windows-*` target，`cargo check --target x86_64-apple-darwin` 因 target 未安装失败；已按代码核对——闭包形状与 R0 之前（5e978a5）编译过 macOS 的版本一致，仅参数名与一行 `let _` 不同。

**R1-2 无窗口时权限请求当场拒绝**（`src/channels/acp.rs`）
`ask_user_permission` 改为：生产路径不登记挂起，直接走新函数 `reject_permission_request`（有 `reject_once` 选项选它，否则 `outcome=cancelled`），不等到本轮 deadline；读类请求的自动放行（`answer_permission`）未动。原「登记挂起、等回答」整体移进 `#[cfg(test)] fn ask_user_permission_awaiting_answer`，由测试开关 `TestAnswerChannelGuard`（`#[cfg(test)] static TEST_ANSWER_CHANNEL`）按测试打开，`pending_permissions` 与 `test_answer_pending` 一并收进 `#[cfg(test)]`。对应测试：
- 重写 `interactive_turn_hands_execute_permission_to_the_user_and_honours_the_answer` → `interactive_execute_permission_is_rejected_immediately_without_waiting`：interactive 会话的执行类请求当场回 `turn-1:no`，断言耗时远小于 10 秒 deadline（实测 0.66s 全测试，即时拒绝在毫秒级），且无挂起残留。
- `late_permission_from_another_session_is_auto_answered_not_carded`：保留，用 `TestAnswerChannelGuard::enable()` 打开挂起路径钉住「回答 → 选中项」的协议形状；与本测试同持 `test_pool_guard` 互斥，开关不会串扰并行测试。
- `dispatch.rs` 的 `run_one_interactive_claude_uses_acp_even_with_acp_switch_off_and_asks_the_user` 同样加 `TestAnswerChannelGuard`。

红检：把 `ask_user_permission` 临时改回「无条件挂起等待」后，新测试等满 10 秒 deadline 超时变红；恢复后转绿。

**R1-3 首页状态做死进程判定**（`src/engine/mod.rs`、`src/run_view.rs`）
`engine` 新增 `pub(crate) fn display_status(&EngineRun) -> &str`：复用 `run_is_active`（启动窗口 + pid 复用核对同一套口径），running 且进程已死判 `failed`，不写盘。`list_runs_at` 的状态改用它。对应测试：`run_view::tests::list_runs_marks_dead_process_running_as_failed`（running + `pid: u32::MAX` → 首页显示 `failed`）。

红检：临时把 `list_runs_at` 改回直取 `run.status` 后该测试变红；恢复后转绿。

**R1-4 误删测试恢复**
- `src/resources.rs`：新增 `#[cfg(test)] fn detect_resource_agent_at_with_pi_binary`（单 agent 探测、pi 二进制可注入，对齐被删的同名函数；不用 `detect_all_resources_at` 是因为全量探测会真拉起本机 claude/codex/kimi 且长时间持资源库锁，与锁测试相撞——试过并已改掉）。恢复 `task17_pi_detect_list_auth_merge_persists_without_auth_payload`、`task17_missing_pi_detection_clears_stale_login`、`task17_pi_login_probe_runs_before_store_lock`（原断言针对已删函数的部分去掉，其余保留）。
- 额度：恢复 `task14_quota_expiry_is_cleared_and_marks_roundtrip`（原 `task14_fallback_order_roundtrips_and_quota_expiry_is_cleared` 里仍测保留代码的部分：过期清理 + `mark_quota_exhausted_at` 写入；fallback 轮转与 `update_model` 断言随那些函数不恢复）。
- `src/dispatch.rs`：恢复 `r3_8_empty_project_dir_is_rejected_without_writes`（空项目时 `dispatch_stage_core` 报「项目目录不能为空」且不写盘，preview 半边随窗口下线）、`r3_16_worktree_node_without_validation_keeps_the_hint`（worktree 节点无验证命令保留「未跑验证命令」提示，STATE inspect/abandon 半边不恢复）、`task07_dispatch_result_preserves_cache_read_usage`（`DispatchNodeResult` 的 `cacheRead` camelCase 序列化，对账半边不恢复）。
- 前端：新增 `src-ui/src/viewer/__tests__/run-view.test.ts`，恢复 `parseAcceptance` 边界用例（取最后一个判定词、只看尾部 1200 字符、窗口边缘一位之差、无判定词与空串）。

**R1-5 文档与注释**
- `docs/MANUAL.md`：删「核对（状态：待核对）」术语行；`STATE.md` 处补「引擎仍会写入，当前没有命令读取」；删「脱敏计数为 0 不等于材料干净」；`resources smoke` 两处用法补 `<agent>`；首页刷新说明改为「窗口可见时每 5 秒，不可见暂停」。
- `docs/CONSOLE-CHANNEL.md`：留痕一节同步 STATE.md 说法。
- `docs/PRODUCT.md`：机制第 6 条去掉「查看器里只读展示」。
- `NOTICE`：只动 aicross 自写段落——来源句改为只提桌面外壳（无边框窗口、自绘标题栏、单实例转发），注明上游终端、工作区、会话、修改记录不在本 fork；删「UPSTREAM TOOL ICONS REUSED BY AICROSS」块与「THIRD-PARTY ASSETS」节（所指文件已删）；「THIRD-PARTY COMPONENTS」删掉已不再打包的 portable-pty / xterm.js / rmcp 三行。上游逐字段原样未动（`git diff NOTICE` 可验）。
- `src-ui/src/styles/global.css`：字体注释改为「随系统安装提供，安装包不再带字体文件」。
- `src/engine/golden.rs`：头注释改为「前端生成器已随 TASK-40 删除，原文见 git 历史与 fixtures/engine-golden/README.md」。

### 应修逐条

- **R1-6** `src/console_cli.rs`：`propose` 加入 `run_if_subcommand` 识别表，`run_at_with` 里给出「propose 已删除，建议在宿主对话里给出」并退出 2，不再落进启动窗口的默认分支。测试：`propose_is_recognized_as_removed_and_exits_2`。
- **R1-7** 旧界面文案：`acp_setup.rs` 的 `unavailable_reason` 四句改为「……本轮回退一次性 CLI」，不再提聊天、`/` 命令、权限卡片、重启本应用（测试改写为 `reasons_are_plain_and_say_the_turn_falls_back`，附反向断言）；`dispatch.rs` 的「请先在左栏『工作区』选择项目目录」改「项目目录不能为空」，`interactive` 字段与注释、真身验证注释里的「接入页」「界面卡片」改现行说法；`orchestration.rs`、`resources.rs`、`console_cli.rs`、`acp.rs` 里指向接入页 / 左栏 / 界面 / 卡片的注释同步改。
- **R1-8** `src-ui/package-lock.json` 与 `README.md` 恢复 CRLF，内容与改前逐字节一致（转回 LF 后与 HEAD blob 比对相同）。
- **R1-9** 残留删除：根目录 `skills/`（66 个文件；NOTICE 无对应条目）；`scripts/` 下 `aicross-hook.py`、`aicross-codex-notify.py`、`aicross-hermes-plugin.py`、`aicross-opencode-plugin.js`（hook 转发器，hook_server 已删）与 `fetch-icon-themes.mjs`、`generate-icon-themes.mjs`（图标主题脚本，图标树已删）；`platform.ts` 删无人引用的 `IS_WINDOWS`；`dispatch.rs` 删引擎恒为 `None` 的 `AppHandle` 进度事件管道（`emit_progress` / `emit_running_progress` / `heartbeat_loop` / `running_progress` / `DispatchProgress`、心跳线程与 3 处 emit 调用、`dispatch_stage_core` 的 `app` 参数及其全部 29 处调用点），随之下线 7 条测试（`task07_running_progress_serializes_heartbeat_contract_fields`、`r1_heartbeat_normal_stop_emits_once_then_exits`、`r1_heartbeat_disconnected_stop_channel_exits_without_more_emits`、`r1_terminal_progress_after_heartbeat_stop_has_no_running_fields_or_late_emit`、`r1_running_progress_redacts_authorization_bearer_title`、`r1_running_progress_truncates_last_tool_to_120_characters`、`task07_fake_acp_three_same_updates_plus_permission_reach_running_event_payload`）；`cancel_dispatch_node` 删去不再注册的 `#[tauri::command]` 标注（函数保留，acp 测试在用）。`process.rs` 的 `ProcessActivity::snapshot` 标注 `#[cfg_attr(not(test), allow(dead_code))]`（现只有测试读取）。
- **R1-10** 首页轮询与读盘：`Home.tsx` 的定时器只在 `document.visibilityState === 'visible'` 时读，`visibilitychange` 恢复可见立即刷一次；`list_runs_at` 先按文件 mtime 粗排，只解析最近 50 个（与第 50 名 mtime 并列的一并解析——文件系统时间戳有粒度，同刻度写入不能凭粗排挤掉，全量套件两轮不稳定失败后加的），mtime 取不到的排最后；预设清单整页只读一次（`preset_display_name` 改为收 `&PresetFile`）。
- **R1-11** `run-list.ts` 删掉与 `run-view.ts` 重复的 `runStatusLabel` / `runStatusTone`（fix-37-r1 起就在 `run-view.ts`），`Home.tsx` 与 `home.test.ts` 改从 `run-view` 引入。

### 红检汇总（编排者抽查项）

- R1-2：回退为无条件挂起等待 → `interactive_execute_permission_is_rejected_immediately_without_waiting` 等 10s deadline 超时红；恢复 → 绿（0.66s）。
- R1-3：回退为直取 `run.status` → `list_runs_marks_dead_process_running_as_failed` 红；恢复 → 绿。

### 验证（五条命令实际输出）

1. `cargo check --all-targets`
   ```
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.11s
   ```
   warning/error 计数 0。
2. `cargo check --release`
   ```
   Finished `release` profile [optimized] target(s) in 13.68s
   ```
3. `cargo test`
   ```
   test result: ok. 408 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 47.55s
   ```
   最终版本连续 5 轮全绿。相对 R0 的 406：删 7 条随进度管道下线的测试，恢复 7 条（3 条 pi 探测 + 1 条额度 + 3 条派发核心），新增 2 条（`propose_is_recognized_as_removed_and_exits_2`、`list_runs_marks_dead_process_running_as_failed`），改写 3 条（`interactive_execute_permission_is_rejected_immediately_without_waiting`、`late_permission_from_another_session_is_auto_answered_not_carded`、`reasons_are_plain_and_say_the_turn_falls_back`）。
4. `cd src-ui && npx vitest run --configLoader native --pool threads`
   ```
   Test Files  4 passed (4)
        Tests  25 passed (25)
   ```
5. `cd src-ui && npm run build`
   ```
   ✓ built in 341ms
   ```

换行核对（逐文件）：CRLF——`src/server.rs`、`src/main.rs`（原有 CRLF 保持）；`docs/PRODUCT.md`（原有 CRLF 保持）；`README.md`、`src-ui/package-lock.json`（本轮按 R1-8 从 LF 改回 CRLF）。LF——`src/` 其余改动文件（`run_view.rs`、`dispatch.rs`、`resources.rs`、`channels/acp.rs`、`channels/acp_setup.rs`、`console_cli.rs`、`engine/mod.rs`、`engine/golden.rs`、`orchestration.rs`、`process.rs`）、`docs/MANUAL.md`、`docs/CONSOLE-CHANNEL.md`、`NOTICE`、`src-ui/src/` 全部改动与新文件（`platform.ts`、`global.css`、`Home.tsx`、`run-list.ts`、`home.test.ts`、`run-view.test.ts`）。新文件 `src-ui/src/viewer/__tests__/run-view.test.ts` 为 LF。

改动总量：95 个文件，+5 806 / −15 379 行（含 skills/ 与 scripts/ 的 72 个删除文件）。

### 未做与理由

- 任务书末节的编排者真机核不在本轮范围。
- `cargo check --target x86_64-apple-darwin` 无法执行（本机未装该 target），R1-1 按代码核对，见上。
- 积压两项（ACP 适配器改为命令行显式安装、STATE.md 崩溃后核对入口）按 fix-40-r1 说明进积压，本轮未动。

### 不确定处

- `list_runs_at` 的 mtime 粗排：与第 50 名并列时解析数会超过 50（上限是 run 目录文件总数），最坏情况（全部同 mtime）退回逐个解析。这是「不丢最新记录」与「少读盘」之间的取舍，偏向前者。
- R1-2 后 interactive 会话里执行类工具调用会被当场拒绝（kimi 单线跑写文件的活会失败并回退一次性 CLI）；这是「没有回答方」的直接后果，与 fix-40-r1 的定案一致。若今后要恢复授权通道，走积压里的「命令行显式安装 / 显式授权」另起。
- R0 报告中与事实不符的三处（换行核对、`platform.ts` 留了什么、tauri.ts 混合换行的形状）已就地改正，并在 R1 各条里写明现状。
