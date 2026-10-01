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

## R1（2026-10-01，修正轮第二阶段：Rust 侧全部内容）

改动文件：`Cargo.toml`（加 indexmap 2 + serde）、`Cargo.lock`、`src/orchestration.rs`（`Graph.nodes` 改 `IndexMap` 保持插入序、新增 `raw_presets_from` 不套角色默认）、`src/dispatch.rs`（`DispatchNodeResult.runner_note` 补 `serde(default)`）、`src/console_cli.rs`（`__run` 入口校验 run_id、退出码 3 来源收口、新增两条测试）、`src/engine/mod.rs`（整体重写为纯函数 + 注入式推进）、新增 `src/engine/golden.rs` 与 `src/engine/tests.rs`、`docs/CONSOLE-CHANNEL.md`。

### 逐条

- **R1-1（Rust 部分）**：`start_dispatch` 拆出 `plan_dispatch`（建图 + 盖章 + 派发前检查的纯函数，模型/预设/角色/在飞数/已有图/git 全部注入）；`run_background` 的循环体拆出 `run_pipeline`（`PipelineSink` 注入落图、run 摘要、单阶段派发与在飞通知，id/时钟/取消信号由调用方给）；生产路径与测试走同一函数。惰性更新语义按 TS 复刻（commit 记数、preview 前 / dispatch 返回后 / 收尾 save 后排空）。测试：`engine::golden::engine_golden_fixtures_replay` 逐夹具重放 31 个夹具（input 驱动 → 逐阶段请求、run 摘要、最终图逐字段相等；UUID/时间戳按首现序归一化比对）。
- **R1-2**：`build_request` 的 `fix_round` 只对 `is_fix_round_execution` 口径的节点置真（角色「执行」且正文以 `[修正 N/M]` 或「根据上一轮验收意见修正实现。」开头，手写解析）。测试：`engine_golden_fixtures_replay`（needs-fix-pass、validation-retry-limit 钉住 `fixRound: true`，其余场景钉住没有该字段）；回改验证：把该字段改回恒真后 economy-pass、blind-three-way 等夹具变红，已恢复。
- **R1-3**：`serialize_context` 的用户消息只受「指定上游」过滤，盲节点带上全部祖先的 `[用户]` 段与附件；只有非直接父的模型回复受 defaultSelected 约束。测试：`engine_golden_fixtures_replay`（economy-pass、blind-three-way 的 prompt）；回改验证：把盲节点的祖先用户消息重新关掉后 economy-pass 第三阶段 prompt 变红，已恢复。
- **R1-4**：`run_pipeline` 的 dispatch Err 分支：给在飞节点补失败桩（`error`、`runner: unknown`、`isolation: 部分`、stage 取各节点 `graph_stage`）、停止推进、run 以 `failed` 收尾。测试：`engine_golden_fixtures_replay`（stage-throw）。
- **R1-5**：实测发现只设 `Stdio::null()` 不够：std 的 `Command` 在 Windows 上一律以 bInheritHandles=TRUE 创建子进程，调用方的管道写端照样漏进后台进程（中介进程 31ms 退出后 EOF 仍被拖住约 19 秒，即孙子进程存活的全程）。`spawn_detached` 改为直接 `CreateProcessW` 且 bInheritHandles=FALSE（后台进程拿不到任何继承句柄），保留 CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP；返回 pid。测试：`detached_grandchild_does_not_hold_the_pipe`（中介 + ping 孙子，EOF < 10 秒；中介为 `pipe_eof_helper`，标 `#[ignore]` 只被显式拉起）、`spawn_detached_missing_exe_is_error`；非 Windows 走 std 全 null stdio，测试 `spawn_detached_detaches_stdio`。
- **R1-6**：`start_dispatch` 在 spawn 前写一次 run（`running`、`pid: 0`），后台进程启动后自己写 pid 与 `pid_started_at`；pid 为 0 时按 30 秒启动窗口判定（`updated_at` 解析不出按过期）。`cancel` 只写标记文件；run 不在 `running` 静默成功。`adopt` 对无结果节点不改图。测试：`pid_zero_within_startup_window_stays_running`、`pid_zero_past_startup_window_becomes_failed`、`cancel_non_running_run_is_silent_ok_without_marker`、`adopt_node_without_reply_leaves_graph_untouched`。
- **R1-7**：run 文件新增 `pid_started_at`（Windows 为进程创建 FILETIME 十进制串）；存活判定 = 进程存在且创建时间一致，记录缺失或读不到时退化为存在性检查。测试：`pid_reuse_guard_checks_creation_time`（Windows）。非 Windows 只做存在性检查，已在文档注明。
- **R1-8**：`run_background` 在推进返回 Err 时先把图上仍 pending 的节点补 `failed`（error_kind `error`），run 落 `failed` 与原始错误文案，再原样返回。无单独单测（注入点是生产文件边界；逻辑与 stage-throw 的图内收尾同形）。
- **R1-9**：`latest_leaf` 按 TS `layout.ts` 口径：深度最大、同深取 created_at 严格较新者、跳过遗留节点、插入序迭代。测试：`latest_leaf_skips_leftover_and_prefers_newer_at_same_depth`。
- **R1-10**：`serialize_context` 的 materials 按首次出现序去重，不排序。测试：`engine_golden_fixtures_replay`（multi-attachment）。
- **R1-11**：`close_waiting_acceptance` 把同批 draft 的验证兄弟一并写 `skipped_reason`。测试：`close_waiting_acceptance_marks_verification_siblings`、`engine_golden_fixtures_replay`（validation-retry-limit）。
- **R1-12**：`mark_pending` 保留原 reply 全部字段（只改 status）；`reset_nodes_to_draft` 只处理 pending 节点。测试：`engine_golden_fixtures_replay`（duplicate-results、reset-draft）。
- **R1-13**：请求里每个节点的 `stage` 取各自 `graph_stage`，请求级取最小值。测试：`engine_golden_fixtures_replay`（needs-fix-pass 修正轮节点的 stage）。
- **R1-14**：`build_request` 只按 provider 精确单段查找；额度检查与备选 vendor 按两段式（先 provider 精确，再 agent+id）；备选模型判空用 trim。测试：`engine_golden_fixtures_replay`（precheck-08、precheck-order-quota-limit、fallback-continuation）。
- **R1-15**：`leftover_items` 照 TS 正则口径：`[-*•]`+空白、数字+`./)、`、`（N）`、`(N)`；无条目行时整段按行兜底（前 20 行）。测试：`leftover_items_recognize_bullet_forms`、`engine_golden_fixtures_replay`（leftover-text-bullet-forms、leftover-text-plain-fallback）。
- **R1-16**：续写已有图保留原 `source`（含空），只有新建图写 `console`。测试：`engine_golden_fixtures_replay`（continue-existing-graph）。
- **R1-17**：拆分上限行文案「本次上限 N 件。」并夹到至少 1；内置预设节点文本照 `stamp.ts` 覆盖规则（越界不覆盖）。测试：`engine_golden_fixtures_replay`（各内置预设场景的节点正文）。
- **R1-18**：`fallback_notice` 认旧图正文首行「（已切备选：」标记；`continuation` 的 provider 比较把缺省与空串视为相等。测试：`continuation_breaks_on_fallback_notice_and_tolerates_missing_provider`、`engine_golden_fixtures_replay`（fallback-continuation）。
- **R1-19**：`append_leftover` 复用同 `source_node_id` 的遗留节点（覆写、不消耗新 id）；`append_correction` 取第一个「执行」父，无产物则不生成修正轮。测试：`append_leftover_reuses_same_source_node`、`append_correction_skips_when_first_execution_parent_has_no_artifacts`、`engine_golden_fixtures_replay`（fix-limit-leftover、needs-fix-pass）。
- **R1-20**：`upstream_artifacts` 先按 DFS 序收集允许集产物，再对允许集里没走到的节点兜底（按节点 id 判重）。测试：`upstream_artifacts_falls_back_for_unreachable_allowed_nodes`。
- **R1-21**：`parse_acceptance` 先按 UTF-16 码元取尾部 1200 窗口再大写按词界取最后 PASS/NEEDS_FIX。测试：`parse_acceptance_uses_utf16_tail_window`（700 个 emoji 把 PASS 顶出窗口 → UNKNOWN）。
- **R1-22**：`apply_results` 有 `fallback_from` 时总写 `fallback_to`（可为空串）；`quota_resets_at` 滤空串。测试：`engine_golden_fixtures_replay`（fallback-continuation、quota-exhausted）。
- **R1-23**：`Graph.nodes` 改 `IndexMap` 后，run 摘要、等待验收查找、遗留节点复用等全部按插入序。测试：`engine_golden_fixtures_replay`（全部夹具的 nodes 顺序）。
- **R1-24**：`active_runs` 只数 `.json` 且文件名不以 `.` 开头的 run 文件；`__run` 入口先校验 run_id（缺失或非法退出 2）。测试：`active_runs_counts_only_json_run_files`、`console_cli::tests::run_subcommand_validates_run_id`。
- **R1-25**：`graph_stage` 单调用内记忆化（memo + 环路防御）。
- **R1-26**：`normalized` 照 TS `normalizedPath`：大小写不敏感地剥 `\\?\UNC\`→`//` 与 `\\?\`、反斜杠转正、去尾斜杠、小写。测试：`normalized_strips_extended_prefixes`。盲附件显示名 `blind_attachment_name` 取路径最后一段（空则回原路径）。
- **R1-27**：`start_dispatch` 不再 `canonicalize` 项目路径；「项目目录不存在」检查保留在最前。文档已写明路径按原文使用。
- **R1-28**：原始需求文本建图时写进 run 文件 `original_request`，后台 `drive_pipeline` 直接读 run，不从图上反推。
- **R1-29**：取消 watcher 抽为 `watch_cancel`（盯 `<run_id>.cancel`，命中置标志位并对在飞节点调 `cancel_dispatch_node`）。测试：`watch_cancel_marks_flag_and_cancels_in_flight_nodes`（真实起线程读标记）、`cancelled_pipeline_marks_in_flight_failed_and_ends_run_failed`（dispatch 后命中取消：在飞节点 failed/cancelled、run 收尾 failed/派发已取消）、`cancelled_before_first_dispatch_leaves_draft_untouched`、`console_cli::tests::exit_code_3_comes_from_engine_root_or_missing_or_stale_manifest`（退出码 3 三种来源：引擎目录不可确定、清单缺失、清单过期）。
- **R1-30**：`docs/CONSOLE-CHANNEL.md` 命令行一节补齐：run 文件含 `original_request` 与 `pid_started_at`；pid==0 的 30 秒启动窗口口径；取消只写标记、非 running 静默成功；进程中断后图上 pending 节点需开窗口核对；设置文件只认 JSON 数字（字符串数字按默认）；退出码 3 含 `propose` 找不到运行中的窗口；非 Windows 不做创建时间比对。

### 有意偏离与说明

- **R1-5 超出原建议**：审查建议只设 `Stdio::null()`；实测（Windows 11，rustc 1.97.1）这不阻断句柄继承，调用方管道仍被后台进程拖住。改用 `CreateProcessW` + bInheritHandles=FALSE 才满足「父进程返回不等流水线」。
- **派发抛错的 reason 带 `Error: ` 前缀**：仿 TS `String(Error)` 的形态（stage-throw 夹具钉住）。
- **`spawn_detached` 额外加了 CREATE_NEW_PROCESS_GROUP**（原实现只有 NO_WINDOW|DETACHED）：防止后台进程被父进程控制台的 Ctrl+C 事件波及。
- **`PipelineOutcome` 字段与 `dispatch::node_is_cancelled` 目前只有测试消费**，加了 `#[allow(dead_code)]` 并注明。
- **TS 侧缺陷照旧单列（不改 TS）**：窗口控制台路径的 `buildRequest` 从不带 `fixRound` 字段；夹具按 `isFixRoundExecution` 修复后口径钉住（R1-2，夹具 notes 有标注）。
- **窗口路径 `listConsoleRuns` 的 5 秒拒绝守卫未移植**：Rust CLI 的 run 是自己刚写的、必然 running，该守卫只在窗口侧有意义。
- **G-L15 运行中摘要保留实际遗留**：按 R1 清单裁定保留（Rust 数据更准）。

### 回改验证（会变红的证据）

- R1-2 回改（`fix_round` 恒真）：`engine_golden_fixtures_replay` 红，`blind-three-way` 首个差异 `$.stages[0].request.nodes[0].fixRound：夹具没有，实际 = true`。已恢复。
- R1-3 回改（盲节点丢掉非直接父的用户消息）：`engine_golden_fixtures_replay` 红，`economy-pass` 第三阶段验收节点 prompt 丢 `[用户]\n实现任务`。已恢复。

### 验证（本阶段实际输出）

| 命令 | 实际结果 |
| --- | --- |
| `cargo check --all-targets` | 退出码 0；零 warning（`warning`/`error` 计数为 0） |
| `cargo test` | 退出码 0；531 passed，0 failed，12 ignored |
| `cd src-ui && npx vitest run --configLoader native --pool threads` | 退出码 0；50 files passed，729 tests passed |
| `cd src-ui && npm run build` | 退出码 0；仅有既有的 500 kB chunk 提醒 |

### 未做（本阶段）

- 真机核仍归编排者（真实派发、运行中取消、杀进程、窗口对账）。
- `EngineRun` 与 `ConsoleRun` 两份同构类型的合并进积压，不在本轮。
- 未 commit，未 push。

## R2（2026-10-01，修正轮 R2）

改动文件：`Cargo.toml`、`src/engine/mod.rs`、`src/engine/tests.rs`、`src/console_cli.rs`、`docs/CONSOLE-CHANNEL.md`、`tasks/REPORT-36.md`。未改 `fixtures/engine-golden/` 与 `src-ui/`。金标准夹具仍为 31 个，重放全绿。

### 逐条

- **R2-1**：`normalized` 的 `strip_prefix_ci` 改用 `value.get(..prefix.len())` 按字符边界取前缀，切点落在多字节字符中间时不匹配（不再按字节切片 panic）。测试：`engine::tests::normalized_handles_multibyte_paths`（`D:\项目`、`D:\项`、`\\?\D:\论文\数据`、`\\?\UNC\服务器\共享\目录` 各一例，结果与 TS `normalizedPath` 口径一致）、`engine::tests::plan_dispatch_works_with_chinese_project_graph_in_dir`（图目录里有中文路径的图时，另一项目的 `plan_dispatch` 照常 Ready）。
- **R2-2**：新增 `merge_save_graph`（照 TS `mergeConsoleCommit`）：每次落图前从磁盘读回该图最新版本，只用本 run 上次提交以来改动或新增的节点覆盖，其余节点以磁盘为准，`root` 取本 run 值。`FileSink` 持有 `committed` 基准（生产落图全部走合并写），`adopt` 与失败收尾的落图同走这一份。测试：`engine::tests::merging_commits_keep_both_runs_nodes_and_results`（两个 run 交替提交，双方节点与结果都保留）、`engine::tests::adopt_during_run_survives_later_commits`（run 进行中 adopt 后，后台提交别的节点不抹掉采纳标记）。
- **R2-3**：`adopt` 只改图（合并写），不再整写 run 文件；返回值按磁盘 run 与刚改好的图现算 `nodes`。测试：`engine::tests::adopt_keeps_run_file_untouched`（adopt 前后 run 文件字节相同，图上 adopted 已落盘）。
- **R2-4**：`run_background` 读到 run 后先查 `status`，不是 `running` 直接退出、不写 pid、不派发。测试：`engine::tests::run_background_ignores_non_running_run`（run 与图文件字节不变）。
- **R2-5**：`run_if_subcommand` 只对面向用户的子命令调用 `attach_parent_console()`，`__run` 跳过；判断抽成纯函数。测试：`console_cli::tests::run_subcommand_skips_parent_console_attach`。
- **R2-6**：推进中途失败的收尾改为 `finish_failed_run`：只把本 run（run 摘要里记录的节点 id）仍 pending 的补失败，同图上别的 run 的节点不动，run 摘要 `nodes` 按收尾后的图重算；`drive_pipeline` 并入 `run_background_with`，sink 工厂可注入（生产传 FileSink）。测试：`engine::tests::pipeline_commit_failure_only_fails_this_runs_nodes`（注入提交即返回错误的 `FailingCommitSink`：本 run 的 n1 补 failed/error、他 run 的 n2 仍 pending，run 落 failed 且摘要重算为 n1）。
- **R2-7**：生产依赖恢复 `serde_json = "1"`，`preserve_order` 挪到 `[dev-dependencies]`；`Graph.nodes` 的 IndexMap 保留。验证：`cargo tree -e features,normal -i serde_json` 不再出现 preserve_order，dev 树里仍开启；金标准测试照常通过；`Cargo.lock` 无变化。改正了 Cargo.toml 里「之前只靠 schemars 传递开启」的不实注释（Cargo.lock 证实是本轮首次开启）。
- **R2-8**：取消检查点重排——登记在飞（`note_in_flight`）之后再查一次取消，命中则本阶段不起（节点保持 draft）；判定取消改到结果写回图之后，已返回的结果保留，未返回的节点由收尾统一标「派发已取消」。测试：`engine::tests::cancel_right_after_registration_skips_dispatch_and_keeps_draft`、`engine::tests::cancel_after_results_writes_them_back_and_marks_rest_cancelled`（部分返回：返回的写回 done、未返回的标 cancelled）、改写后的 `engine::tests::cancelled_pipeline_marks_in_flight_failed_and_ends_run_failed`（整批无返回时在飞标取消）。
- **R2-9**：`upstream_artifacts` 第一轮改为 `order_by_dfs(graph, allowed)` 只在允许集内 DFS（不在允许集的节点连同子树剪枝），`context_order` 用同一实现；兜底仍按允许集顺序。测试：`engine::tests::upstream_artifacts_first_pass_dfs_stays_inside_allowed_set`（指定上游 `[b, a]` 的产物顺序为 b、a，不是全祖先 DFS 序）。
- **R2-10**：`within_startup_window` 对 `updated_at` 晚于当前时间 60 秒以上（时钟回拨）同样按过期。测试：`engine::tests::pid_zero_far_future_updated_at_is_expired`（+120 秒判「引擎进程未能启动」，+5 秒仍算启动中）。
- **R2-11**：`spawn_detached`（Windows）拼命令行处加注释：实参只有 `__run` 与已过 `valid_id` 校验的 run id，朴素引号包裹即可；以后传用户可控文本必须先补引用转义。
- **R2-12**：补退出码 3 另两种来源的测试：`engine::tests::start_dispatch_reports_unavailable_when_runs_dir_not_writable`（runs 被同名文件占住）、`engine::tests::start_dispatch_reports_unavailable_when_spawn_fails`（`start_dispatch_with` 注入失败 spawn，run 落 failed 不留假 running）、`console_cli::tests::dispatch_exit_3_when_runs_dir_not_writable`（`run_at` 全链路退出码 3）。
- 文档：`docs/CONSOLE-CHANNEL.md` 补启动窗口对时钟回拨的口径（晚 60 秒以上判过期）、后台进程读到非 running 直接退出、多个命令行后台 run 交替提交同一张图按节点合并、采纳标记不被后台提交抹掉；窗口与命令行同时写同一张图仍无并发保护。

### 回改验证（会变红的证据）

- R2-1 回改（恢复 `value[..prefix.len()]` 字节切片）：`normalized_handles_multibyte_paths`、`plan_dispatch_works_with_chinese_project_graph_in_dir` 均 panic 于 `end byte index 8 is not a char boundary; it is inside '目'`。已恢复。
- R2-2 回改（`FileSink::save_graph` 改回 `save_graph_in` 整图写）：`merging_commits_keep_both_runs_nodes_and_results` 红于「另一个 run 的节点不能被抹掉」，`adopt_during_run_survives_later_commits` 红于「采纳标记不能被后台提交抹掉」。已恢复。

### 报告勘误（R1 节）

- 删去「`dispatch_stage_core` 成功后清理取消注册表」：`clear_cancelled` 在基线的同一位置已存在（before:3527 → after:3528）。
- 删去「删 `stamp`」：基线 `orchestration.rs` 本无 `fn stamp`。
- 第三处「preserve_order 之前已由 schemars 传递开启」不在报告里而在 Cargo.toml 注释，已在 R2-7 改正。

### 验证（本阶段实际输出）

| 命令 | 实际结果 |
| --- | --- |
| `cargo check --all-targets` | 退出码 0；零 warning |
| `cargo test` | 退出码 0；546 passed，0 failed，12 ignored |
| `cd src-ui && npx vitest run --configLoader native --pool threads` | 退出码 0；50 files passed，729 tests passed |
| `cd src-ui && npm run build` | 退出码 0；仅有既有的 500 kB chunk 提醒 |

### 未做（本阶段）

- 真机核仍归编排者（真实派发、运行中取消、杀进程、窗口对账）。
- 金标准输入时间按原始先后重建、检查顺序再补钉、管道捕获 `dispatch` 输出提前返回的端到端测试：按 fix-36-r2 留在积压。
- 未 commit，未 push。
