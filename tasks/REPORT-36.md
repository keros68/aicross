# REPORT-36 引擎无窗口可跑：派发编排挪进 Rust（09-30）

## 完成内容

- 新增 `src/engine/mod.rs`，承接控制台派发的建图、预设盖章、派发前检查、请求构造、逐阶段推进、修正轮、验证重试、遗留节点和 run 摘要。
- `dispatch`、`status`、`adopt`、`cancel` 改为直接调用本地引擎；`propose` 继续使用窗口 HTTP。新增隐藏子命令 `__run <run_id>`，Windows 后台进程使用 `CREATE_NO_WINDOW | DETACHED_PROCESS`。
- 命令行设置读取 `~/.aicross/dispatch-settings.json`；run 与 cancel 标记写入 `~/.aicross/runs/`。run 文件使用同目录临时文件替换，记录 `pid` 与 `updated_at`；读取时把已退出的运行进程改记为 `failed / 引擎进程已退出`。
- `ConsoleRun` 追加 `pid`、`updated_at`；`Target` 追加 `validation_command`，`ReplyArtifacts` 追加 `source_node_id`，并补了图 JSON 往返测试。
- 新增 24 个 TypeScript 金标准场景文件。时间与 UUID 按出现顺序归一化；10 条派发前拒绝分别成夹具。Rust 读取同一批夹具，并对一次通过场景的请求、最终图与 run 摘要做字段核对。
- `src/dispatch.rs` 仅向引擎开放 `dispatch_stage_core`。为使全量测试在 Windows 并行运行时稳定，TASK-13 的临时 Git 仓库测试串行化，并放宽两个只用于识别超时/启动失败的测试时间窗口；生产逻辑未改。
- `docs/CONSOLE-CHANNEL.md` 已写成最终命令行状态，包括无窗口运行、响应真实 ID、持久化位置、退出码 3、设置不同步和并发写限制。

## 改动文件

| 文件 | 内容 |
| --- | --- |
| `src/engine/mod.rs` | 无窗口引擎、持久化、后台运行与单测 |
| `src/main.rs` | 注册 `engine` 模块 |
| `src/console_cli.rs` | 四个命令改走本地引擎，接入 `__run` |
| `src/console_api.rs` | `ConsoleRun` 追加持久化字段 |
| `src/orchestration.rs` | 图类型字段对齐并开放引擎所需的现有读写函数 |
| `src/dispatch.rs` | 开放单阶段核心；稳定 Windows 并行测试 |
| `src-ui/src/__tests__/engine-golden.test.ts` | TypeScript 参考实现的逐场景金标准测试 |
| `fixtures/engine-golden/*.json` | 24 个归一化场景夹具 |
| `docs/CONSOLE-CHANNEL.md` | 命令行最终契约 |
| `tasks/REPORT-36.md` | 本报告 |

## TypeScript → Rust 移植清单

| TypeScript | Rust |
| --- | --- |
| `appendConsoleMaterial`、`fillRoot`、`latestLeaf`、`newGraph` | `append_material`、`latest_leaf`、`new_graph` |
| `consolePreset` | `start_dispatch` 中的预设或 `targets` 选择 |
| `presetWithRoleDefaults`、`applyRoleDefaults` | `apply_role_defaults` |
| `stamp` | `stamp_with`、`preset_text` |
| `graphStage` | `graph_stage` |
| `serializeContext`、`serializeCurrentMessage` | `serialize_context`、`current_message` |
| `runRequestOf`、`upstreamArtifactsOf` | `serialize_context`、`upstream_artifacts` |
| `continuationSession`、`continuationRunner` | `continuation` |
| `promptForRole`、`promptWithWriteGuard`、`buildNodeRequest` | `role_prompt`、写保护拼装、`build_request` |
| `validateNode` 与 `runConsoleDispatch` 的前置检查 | `validate_block`、`start_dispatch` |
| `nextStageStep` | `dispatchable_ids` 与 `run_background` 的阶段选择 |
| `markPending`、`resetNodesToDraft` | `mark_pending`、`reset_draft` |
| `applyDispatchResults` | `apply_results` |
| `reducePipelineStep` | `reduce_step`、`append_correction`、`append_validation_retry`、`append_leftover` |
| `dispatchBatchKey`、`markBatchProcessed` | `run_background` 的批次键集合 |
| `failedNodesWithoutResults` | `run_background` 的缺失结果补失败分支 |
| `leftoverNodes` | `leftover` |
| `runStageLoop`、`startPipelineHeadless` | `run_background` |
| `nodeSummary` | `node_summary`、`utf16_excerpt` |
| `loadDispatchSettings` | `load_settings` |

提示词继续从 `fixtures/prompts/*.txt` 用 `include_str!` 读取，没有复制模板正文。`text_excerpt` 按 UTF-16 码元截取；切在代理对中间时以替换字符表示，中文和表情均有 Rust 单测。

## 照原样移植的 TypeScript 缺陷

未发现需要单列并等待裁决的 TypeScript 缺陷。Rust 与 TypeScript 对可选请求字段的序列化默认值不同，金标准比较时只归一化 Rust 明确写出的 `null` / `false` 默认值；现有 Rust 后端协议保持不变。

## 验证

| 命令 | 实际结果 |
| --- | --- |
| `cargo check --all-targets` | 退出码 0；`Finished dev profile`；零 warning |
| `cargo test` | 退出码 0；510 passed，0 failed，11 ignored |
| `cd src-ui && npx vitest run --configLoader native --pool threads` | 退出码 0；50 files passed，722 tests passed |
| `cd src-ui && npm run build` | 退出码 0；TypeScript 构建通过，Vite 转换 578 modules；仅有既有的 500 kB chunk 提醒 |

另做了 `git diff --check`，无空白错误。所有新增文件为 LF；`server.rs` 未改。

## 未做

- 未执行任务书留给编排者的真机核：真实模型派发到 `done`、运行中取消、手工杀后台进程、重开窗口查看采纳结果。
- 未改窗口设置页与前端派发路径；窗口设置和命令行设置仍不同步。
- 未处理窗口与命令行同时写同一图的并发冲突。
- 未实现蜂群、仲裁、`pickOne`、`propose` 移植和 `preview_dispatch`，均在本期禁区或范围外。
- 未 commit，未 push。
