# REPORT-06A — 重派与崩溃恢复的一致性（契约 + Rust 引擎）

## 交付对照

### 契约

- 已做：新增 `reconcile_dispatch_state`、`ReconcileSummary`/节点明细，并注册 Tauri 命令；新增 `rollback_node_changes`、`RollbackSummary` 并注册命令（`src/dispatch.rs:241`、`src/dispatch.rs:261`、`src/dispatch.rs:402`、`src/dispatch.rs:453`、`src/server.rs:3952`）。／已补：`task06_reconcile_classifies_completed_incomplete_unknown_and_ignores_other_run`、`task06_single_writer_rollback_restores_changed_deleted_and_created_files`
- 已做：`aicross-dispatch/1` frontmatter 追加可缺省的 `before_dir`，`DispatchNodeRequest` 追加 camelCase 输入 `rollbackBefore`（`src/dispatch.rs:113`、`src/dispatch.rs:329`）。／已补：`task06_dispatch_request_reads_rollback_before_camel_case`、`frontmatter_roundtrips`
- 已做：Rust 图契约 `Reply` 追加可选 `fallback_from`/`fallback_to`，旧图缺字段仍取 `None`（`src/orchestration.rs:102`）。／已补：`task06_reply_fallback_fields_roundtrip_and_default_for_old_graphs`

### A. 引擎（Rust）

1. 已做：单写者尝试完成后比较 before/after，把旧内容写到 `.dispatch/<run>/<node>.before/`，新建文件写入 `.created`，超 8MB 无内容项写入 `.skipped`；回滚恢复旧文件、删除新建文件，无快照时返回空结果和说明；`rollback_before` 在写锁内、派发前执行，worktree 不执行，结果 note 记录回滚数（`src/dispatch.rs:1023`、`src/dispatch.rs:1062`、`src/dispatch.rs:1612`、`src/dispatch.rs:1827`）。／已补：`task06_single_writer_rollback_restores_changed_deleted_and_created_files`、`task06_rollback_before_runs_inside_run_one_before_redispatch`、`task06_rollback_reports_over_eight_megabyte_snapshot_entries_as_skipped`
2. 已做：按 `STATE.md` 的 `run_id` 与 active 节点扫描、确定性选择同节点最新留痕；ok/incomplete/无匹配分别返回 completed/incomplete/unknown；新留痕追加回复正文和脱敏后产物 JSON，旧留痕仍可解析且产物为空（`src/dispatch.rs:383`、`src/dispatch.rs:402`、`src/dispatch.rs:2227`）。／已补：`task06_reconcile_classifies_completed_incomplete_unknown_and_ignores_other_run`
3. 已做：执行提示追加禁止 push、发布、安装依赖和付费接口；Claude 写节点白名单仍只含 `Read,Grep,Glob,Write,Edit,Bash`，明确排除 WebFetch/WebSearch（`fixtures/prompts/execution.txt:2`、`src/channels/mod.rs:970`）。／已补：`task06_execution_prompt_forbids_external_side_effects`、`writable_claude_excludes_web_tools_from_allowlist`
4. 已做：进程内登记 active run，同一 `run_id` 尚在进行时第二次调用返回“该 run 已在进行”，RAII 保证所有退出路径释放登记（`src/dispatch.rs:586`、`src/dispatch.rs:790`）。／已补：`task06_duplicate_active_run_is_rejected_before_dispatch`
5. 已做：交互输出队列为截断标记额外预留标记自身字节，普通输出仍受 8MB 上限约束（`src/process.rs:13`、`src/process.rs:157`）。／已补：`interactive_stdout_truncation_marker_has_reserved_capacity_beyond_eight_megabytes`
6. 已做：Rust 侧只提供并保存 `fallback_from`/`fallback_to` 字段；标记行由前端在 B.3 停写（`src/orchestration.rs:102`、`src/dispatch.rs:1638`）。／已补：`task06_reply_fallback_fields_roundtrip_and_default_for_old_graphs`、`task06_fallback_reruns_once_uses_fields_without_reply_marker`

## 没做

- 没做：TASK-06 的 B. 界面 1–7；本报告只覆盖用户指定的“契约”与“A. 引擎（Rust）”。
- 没做：未改 `src/channels/acp.rs`，未启动窗口，未发真机请求，未运行 `cargo fmt`，未 commit，未 push。
- 没做：未处理工作区原有/外部生成的未跟踪文件 `tasks/review-06a-prompt.txt`，避免越界覆盖。

## 验证命令与输出原文

### `cargo check`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.49s
```

零 warning。

### `cargo test`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 20.13s
     Running unittests src\main.rs (target\debug\deps\aicross-3605bca6cf2a3148.exe)

running 250 tests
...
test result: ok. 239 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 12.31s
```

上框保留完整命令的起止原文；逐项名称较长。另以 `cargo test -q` 对同一全量测试作最终复核，其完整输出原文如下：

```text
running 250 tests
i....................................................iiiii............................. 87/250
..................................................................iii.................. 174/250
.......................ii...................................................
test result: ok. 239 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 8.64s
```

说明：此前全测中的既有测试 `fallback_agent_blocks_same_agent_node_from_running_concurrently` 因测试态默认运行目录落在沙箱外而报一次 `创建运行目录失败：拒绝访问。 (os error 5)`。将 `cfg(test)` 下未显式设置 `AICROSS_TEST_RUN_ROOT` 的默认目录收窄到系统临时目录后，最终全测如上通过；生产构建路径未改变（`src/dispatch.rs:744`）。

### `cd src-ui && npx vitest run --configLoader native --pool threads`

```text
 RUN  v5.0.0 D:/work/aicross-p4/src-ui

 Test Files  20 passed (20)
      Tests  221 passed (221)
   Start at  19:23:11
   Duration  492ms (transform 65%, import 16%, tests 15%, worker 4%)
```

### `cd src-ui && npm run build`

第一次运行：

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build

failed to load config from D:\work\aicross-p4\src-ui\vite.config.ts
error during build:
Error: Build failed with 1 error:

[plugin externalize-deps]
Error: spawn EPERM
```

原命令直接复跑：

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build

vite v8.0.7 building client environment for production...
transforming...
✓ 322 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                                                  3.46 kB │ gzip:   1.32 kB
...
✓ built in 645ms
```

### `git -c core.whitespace=cr-at-eol diff --check`

```text
```

无输出，退出码 0。`server.rs` 检查为 4618 个 LF、4618 个 CRLF（纯 CRLF）；其余本次修改的 Rust 文件 CRLF 为 0（纯 LF）。

## 修正轮 R1

1. 已修：回滚与 before 快照按 `uses_single_writer` 生效模式执行，覆盖非 git worktree 降级（`src/dispatch.rs:1678`、`src/dispatch.rs:1705`、`src/dispatch.rs:1930`）。／已补：`task06_rollback_before_runs_inside_run_one_before_redispatch`
2. 已修：按 `node_id` 扫项目及运行目录留痕，以最新非空 `before_dir` 定位，失败再退当前 run 路径；无快照 note 明确首次派发/已清理（`src/dispatch.rs:1101`、`src/dispatch.rs:1131`；`tasks/TASK-06-consistency.md:8`）。／已补：`task06_rollback_before_runs_inside_run_one_before_redispatch`
3. 已修：拒绝 RootDir/CurDir；恢复前移除目标链接/junction，并 canonicalize 校验父目录仍在项目内（`src/dispatch.rs:1045`、`src/dispatch.rs:1161`）。／已补：`task06_safe_relative_path_rejects_root_and_current_components`、`task06_rollback_created_root_path_does_not_escape_project`、`task06_rollback_replaces_target_symlink_without_following_it`
4. 已修：关闭项目留痕时快照与留痕同置运行目录并记绝对路径；项目 `.gitignore` 覆盖子目录；持久化失败只追加 warning（`src/dispatch.rs:1052`、`src/dispatch.rs:1090`、`src/dispatch.rs:1930`）。／已补：`task06_snapshot_outside_project_is_absolute_and_colocated_with_records`、`task06_snapshot_persist_failure_warns_without_flipping_success`
5. 已修：ACP 各结果分支统一追加 `runner_note`，不覆盖回滚说明（`src/dispatch.rs:1778`、`src/dispatch.rs:2241`）。／已补：`task06_acp_runner_note_append_preserves_rollback_note`
6. 已修：对账按标题行取最后一个正文/产物段，兼容内嵌字样和无产物旧留痕（`src/dispatch.rs:393`）。／已补：`task06_reconcile_parses_embedded_heading_and_legacy_record`
7. 已修：真压 8MB 队列边界并断言标记入队、普通行不入队；active run 用 Drop 清理并验证释放；Python 缺失显式 eprintln（`src/process.rs:513`、`src/dispatch.rs:3355`）。／已补：`interactive_stdout_truncation_marker_is_delivered_at_full_queue_boundary`、`task06_duplicate_active_run_is_rejected_before_dispatch`
8. 已修：`ReconcileNode` 增加 `sessionId`、`runner`、`usage.input/output/cacheRead` 并从 frontmatter 回填（`src/dispatch.rs:250`、`src/dispatch.rs:467`；`tasks/TASK-06-consistency.md:8`）。／已补：`task06_reconcile_classifies_completed_incomplete_unknown_and_ignores_other_run`
9. 已修：只在真正进入 ACP/oneshot 通道后落 before，准备、取消、构建早退不覆盖旧快照（`src/dispatch.rs:1760`、`src/dispatch.rs:1930`）。／已补：`task06_early_failure_does_not_replace_previous_before_snapshot`
10. 已修：回滚直接遍历 before，先收集、逐条恢复、最后删 `.created`；单项失败进 skipped 并返回 summary（`src/dispatch.rs:1141`）。／已补：`task06_rollback_reads_before_directory_beyond_snapshot_total_limit`
11. 已修：命令先取项目写锁，与派发互斥（`src/dispatch.rs:486`；`tasks/TASK-06-consistency.md:9`）。／已补：`task06_rollback_command_waits_for_project_write_lock`
12. 已修：`ended_at` 按 RFC3339 时间戳排序，同刻 ok 优先、再按文件名；解析取最后标题（`src/dispatch.rs:419`、`src/dispatch.rs:451`）。／已补：`task06_reconcile_order_parses_timestamps_and_prefers_ok_on_tie`、`task06_reconcile_parses_embedded_heading_and_legacy_record`
13. 已修：明确为进程内、run 级互斥；RAII 释放后允许相同 run 再进入（`src/dispatch.rs:610`；`tasks/TASK-06-consistency.md:13`）。／已补：`task06_duplicate_active_run_is_rejected_before_dispatch`
14. 已修：无快照和正常回滚 note 均说明排除目录与链接内改动不在快照内（`src/dispatch.rs:1135`、`src/dispatch.rs:1191`）。／已补：`task06_single_writer_rollback_restores_changed_deleted_and_created_files`
15. 已修：process 标记测试改为 `recv_line` 行为断言；active run 测试用 Drop 守卫（`src/process.rs:513`、`src/dispatch.rs:3355`）。／已补：`interactive_stdout_truncation_marker_is_delivered_at_full_queue_boundary`、`task06_duplicate_active_run_is_rejected_before_dispatch`
16. 已修：A.6 改为“Rust 侧只提供字段；标记行由前端在 B.3 停写”；禁外部动作按可写挂载归 06B R1，本轮不动前端（`tasks/REPORT-06A.md:18`）。／已补：`task06_reply_fallback_fields_roundtrip_and_default_for_old_graphs`

### R1 验证命令与输出原文

#### `cargo check`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.18s
```

零 warning。

#### `cargo test -q -- --test-threads=1`

```text
running 261 tests
...........i..........................................................iiiii............ 87/261
....................................................................................... 174/261
.......iii.........................................ii.................................. 261/261

test result: ok. 250 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 22.60s
```

补充：默认并行全测曾两次触发禁止修改的 `src/channels/acp.rs` 既有 1 秒计时断言抖动；该用例单独运行通过（0.30s），最终以单线程全量测试消除用例间资源竞争并全部通过。

#### `cd src-ui && npx vitest run --configLoader native --pool threads`

```text
 RUN  v5.0.0 D:/work/aicross-p4/src-ui

 Test Files  20 passed (20)
      Tests  221 passed (221)
   Start at  20:30:44
   Duration  515ms (transform 59%, import 19%, tests 18%, worker 4%)
```

#### `cd src-ui && npm run build`

前两次复跑在 Vite `externalize-deps` 的 `spawn EPERM` 处失败；第三次原命令成功。成功输出原文较长，逐资产行照录关键首尾如下：

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build

(node:32912) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
vite v8.0.7 building client environment for production...
transforming...
✓ 322 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                                                3.46 kB │ gzip:   1.32 kB
dist/assets/onig-Du5pRr7Y.wasm                               466.61 kB │ gzip: 159.69 kB
dist/assets/main-CiCPcvJb.js                                 424.05 kB │ gzip: 210.44 kB
dist/assets/xterm-_r9IGi5_.js                                465.79 kB │ gzip: 123.12 kB

✓ built in 593ms
```

#### `git -c core.whitespace=cr-at-eol diff --check`

```text
```

无输出，退出码 0。最终检查：`src/server.rs` 为 LF=4618、CRLF=4618（纯 CRLF）；`src/dispatch.rs`、`src/process.rs` 的 CRLF 均为 0（纯 LF）。未运行 `cargo fmt`。
