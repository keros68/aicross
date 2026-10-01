# TASK-40 窗口其余界面下线（10-01）

**前置**：TASK-36 至 TASK-39 已合并 main（无窗口引擎、查看器、每次派发一张图、资源与配置命令行）。不 commit，不 push。

## 背景

`docs/PRODUCT.md`「为什么是这个形态」「机制」与「不做」：对话留在宿主，aicross 只做命令行引擎和一页查看器。窗口里从 Coffee CLI 继承和此前自建的其余界面（终端 tab、文件树、控制台对话窗、对话图、右栏编排、接入页、设置页、启动器、讲解页、建议条）不再需要；它们的功能已由命令行替代：派发 `aicross dispatch/status/adopt/cancel`，资源 `aicross resources [detect|smoke]`，配置 `aicross config show|set-role|set`，查看 `aicross open`。

## 1. 窗口最终只剩

- **自绘标题栏**（最小化 / 最大化 / 关闭、拖动）与窗口缩放边条。
- **首页「最近的派发」**：直接启动窗口（不带 `open`）时显示。新增 Tauri 命令 `list_runs`：读 `~/.aicross/runs/*.json`（只认 `.json` 且不以 `.` 开头，与引擎 `active_runs` 同口径），按 `updated_at` 倒序取最近 50 条，每条给 `run_id`、模式名（同查看器的 `preset_id` 取名规则）、状态、开始时间、任务名（`original_request` 第一行，同查看器 `task_title`）。点一条进入查看页；查看页「关闭」回到首页。没有任何 run 时显示一句「还没有派发记录；在宿主里经 ai-cross skill 派发，或运行 aicross dispatch」。
- **查看页**（TASK-37）：保持现有行为与测试。

## 2. 删除

以编译器与测试为准，删掉所有只为上述旧界面存在的代码、样式、测试与资源，包括但不限于：

- 前端：`src-ui/src/components/` 下除查看器所需之外的全部组件（center、left、right、resources、common 里只留标题栏、缩放边条、错误边界等查看器与首页仍在用的）；`console-*.ts`、`commander/`、`settings/`、`store/` 中只服务旧界面的部分；`graph/` 整个目录。查看器还在用的纯函数（如 `parseAcceptance`、`compactRunNumber`、`agentModelDisplay`、`vendorColor`、`ConsoleMarkdown`）搬到 `src-ui/src/viewer/` 或保留原位但删掉旁边无关的代码，行为不变。
- 前端金标准生成器 `src-ui/src/__tests__/engine-golden.test.ts` 随前端参考实现一起删除；**`fixtures/engine-golden/` 的 31 个夹具保留**，作为引擎行为契约，Rust 侧 `engine::golden::engine_golden_fixtures_replay` 照常重放。在 `fixtures/engine-golden/` 放一个 `README.md`，说明夹具来自已删除的前端参考实现（最后一版见 git 历史），今后改动须有理由并同步改 Rust 测试。
- Rust：只服务旧界面的模块与 Tauri 命令——终端与 PTY（`terminal.rs`）、`launch` 子命令（`launch.rs`）、Coffee 的 hook 机制（`hook_server.rs`、`hook_forwarder.rs`、`hook_installer.rs`）、`marketplace.rs`、`skills.rs`、`fs_watcher.rs`、`fonts.rs`、`tool_config.rs`、`src/tools/`、`shell_probe.rs`、窗口 HTTP 通道与 `propose`（`console_api.rs`、`console.json` / `console.dev.json` 清单的写入），以及 `server.rs` 里对应的注册与状态。**删除前先确认引擎、资源探测、查看器、命令行没有用到**；被用到的部分保留或挪到使用方。
- `aicross propose` 子命令删除（G1 已在宿主对话里做）；退出码表与文档同步。

## 3. 必须保持

- 命令行全部子命令行为不变：`dispatch / status / adopt / cancel / open / resources [detect|smoke] / config show|set-role|set`，退出码不变。
- 查看器与 TASK-37 的冷热启动、单实例转发。
- 资源库、`roles.json`、`dispatch-settings.json`、`graphs/`、`runs/` 的格式不变。
- AGPL 与上游义务：`LICENSE`、`NOTICE`（上游那段逐字保留）、`TRADEMARKS.md` 不动；`README.md` 仍写明 fork 自 Coffee CLI。
- Windows 安装包把安装目录加入 PATH 的逻辑（`windows_path.rs` 与 NSIS 脚本）保留。

## 4. 文档

- `docs/MANUAL.md` 改写为查看器与命令行手册：安装、各命令、查看器两页（首页与查看页）、配置文件位置、退出码。删掉旧界面的说明与术语。
- `docs/CONSOLE-CHANNEL.md`：删掉 `propose`、HTTP 接口、清单文件与令牌相关段落，只留命令行。
- `README.md`：项目一句话与用法改为现行形态。
- `AGENTS.md`（仓库内）：「技术栈」「目录」里删掉终端、PTY、xterm 等已不存在的部分。

## 5. 测试与验证

- 删掉被删代码的测试；保留并跑通其余全部测试。新增：`list_runs` 的 Rust 测试（排序、只认 `.json`、取名、空目录）；首页的前端纯函数测试（列表项文案、空状态）。
- `src-ui/package.json` 里只为旧界面引入的依赖（如 xterm 系列）删掉，`package-lock.json` 随之更新；保持原换行。

```
cargo check --all-targets        # 零 warning
cargo check --release
cargo test
cd src-ui && npx vitest run --configLoader native --pool threads
cd src-ui && npm run build
```

每个改动文件保持原有换行（`src/server.rs`、`src/main.rs` 是 CRLF）；新文件 LF。写 `tasks/REPORT-40.md`：删了哪些文件与模块（附行数）、保留并搬动了哪些、删依赖清单、五条命令实际输出、未做与理由。

## 编排者真机核（实现者不做）

`tauri build --no-bundle` 出发布版：直接启动窗口看到「最近的派发」并能点进查看页、关回首页；`aicross open <run_id>` 冷热启动；`aicross dispatch` 真派一次到 `done`；`resources`、`config show` 照常。
