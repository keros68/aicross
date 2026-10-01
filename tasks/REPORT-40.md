# REPORT-40 窗口其余界面下线

分支 `ui/retire40`，基于 TASK-40 任务书。未 commit、未 push（改动已暂存在工作区）。

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

`components/center|left|right|resources`、`graph/`、`store/`、`i18n/`、`icons-inline/`、`commander/`、`resources/`、`utils/`、`lib/`（仅留 `platform.ts` 的 `IS_MACOS`）、`console-*.ts` 七个文件、`components/common` 里的 SettingsModal / FontPicker / ScrollPanel / ErrorBoundary、`__tests__/` 里 14 个旧界面测试（含 `engine-golden.test.ts`）；`public/fonts`（7 个 woff2）与 `public/icons`（Explorer/终端的图标树）。

### 依赖删除

- npm（`package.json`，`package-lock.json` 随 `npm install` 更新）：`@xterm/xterm`、`@xterm/addon-fit`、`@xterm/addon-web-links`、`@xterm/addon-webgl`、`diff`、`shiki`、`@tauri-apps/plugin-clipboard-manager`、`@tauri-apps/plugin-dialog`、`@tauri-apps/plugin-global-shortcut`。
- Cargo（`Cargo.toml`）：`portable-pty`、`tokio`、`clap`、`glob`、`log`、`tauri-plugin-dialog`、`tauri-plugin-clipboard-manager`、`tauri-plugin-global-shortcut`、`ttf-parser`、`base64`、`png`、`ureq`、`include_dir`、`notify`、`notify-debouncer-full`、`rusqlite`、`keyring`、`rmcp`、`axum`。`windows` 的 `Win32_Security` 特性一度删除后恢复——`CreateJobObjectW` / `CreateProcessW` 的签名引用 `SECURITY_ATTRIBUTES`，编译器抓回来的。
- `capabilities/default.json` 收缩为标题栏拖动、缩放、显窗与事件监听；`capabilities/desktop.json` 删除（只剩 global-shortcut）。`tauri.conf.json` 关闭 `assetProtocol`（原 `protocol-asset` 特性随 tauri 依赖一并移除）。

## 保留与搬动

- **查看器纯函数各归其位**：`ConsoleMarkdown.tsx`（原 components/center）、`vendor.ts`（原 graph/vendor.ts）、`agent-display.ts`（原 resources/agent-display.ts）搬到 `src-ui/src/viewer/`；`parseAcceptance` 与 `compactRunNumber` 内联进 `viewer/run-view.ts`（实现照抄原函数，行为不变，`run-viewer-37` 与 markdown 测试原样跑通）。
- **首页（新增）**：`viewer/Home.tsx` + `viewer/Home.css` + `viewer/run-list.ts`（纯函数：列表项文案、状态词与胶囊、开始时间、空状态文案）+ `viewer/__tests__/home.test.ts`（5 条）。
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

换行核对：`src/server.rs`（229 行）与 `src/main.rs`（273 行）逐行 CRLF；其余改动文件 LF。`src-ui/src/tauri.ts` 原本是前 91 行 CRLF、后段 LF 的混合文件，整文件重写后统一为 LF（新文件 LF 约定，无法「保持」混合换行）。

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
- **`src-ui/src/tauri.ts` 换行**：见上，混合换行无法原样保留，已统一 LF。
- **`status` 输出的 `presetName`**：首页列表的模式名直接复用查看页取名函数；旧 run 文件没有 `preset_id` 时显示「自定义模式」，与查看页一致。
- **任务书第 5 节「取名」测试**：list_runs 的取名口径在 Rust 侧测了（`list_runs_names_preset_and_task_like_the_viewer`）；前端 `runListItemText` 只负责「模式名 · 任务名」的拼装与缺省回退，各自覆盖。
