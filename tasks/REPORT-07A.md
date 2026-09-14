# REPORT-07A — 契约与 A. 引擎（Rust）

## 逐条交付

1. 已做：`dispatch-progress` 增加 camelCase 可选心跳字段；每个运行节点建立共享活动状态并每 15 秒发 `status="running"`，完成或取消后用停止通道立即结束心跳线程；一次性 stdout/stderr 读线程更新最后输出时间，ACP `session/update` 统计工具总数、连续同名次数和标题（`src/dispatch.rs:289`、`src/dispatch.rs:732`、`src/dispatch.rs:2569`、`src/process.rs:14`、`src/process.rs:456`、`src/channels/acp.rs:465`）／已补：`task07_running_progress_serializes_heartbeat_contract_fields`、`task07_fake_acp_three_same_tool_calls_reach_running_event_payload`、`fake_acp_three_same_tool_calls_count_as_three_repeated_calls`、`oneshot_silent_two_seconds_reports_idle_at_least_two_seconds`。

2. 已做：写新 before 快照前将旧目录改名为 `.before.prev`；成功后删除 prev，写失败则清理半成品并恢复旧目录（`src/dispatch.rs:1082`）／已补：`task07_before_snapshot_write_failure_restores_previous_directory`、`task07_before_snapshot_success_replaces_old_and_removes_prev`。

3. 已做：before/after 没有变化时在触碰现存目录之前返回，不写空快照也不移动旧目录（`src/dispatch.rs:1091`）／已补：`task07_zero_change_redispatch_preserves_nonempty_snapshot`。

4. 已做：`latest_before_dir` 仅接受 workdir 成功 canonicalize 且等于当前项目的记录，失败时不匹配（`src/dispatch.rs:1173`）／已补：`task07_latest_before_dir_rejects_uncanonicalizable_workdir`。

5. 已做：`DispatchRecord` 使用结构级 `#[serde(default)]`，保留 runner 的 oneshot 特殊默认；`before_dir` 注明运行目录模式的绝对路径例外；`dispatch_stage_core` 的 run 级 RAII 幂等互斥补代码注释；回滚 note 写入 skipped 数量（`src/dispatch.rs:305`、`src/dispatch.rs:349`、`src/dispatch.rs:626`、`src/dispatch.rs:1245`）／已补：`task07_dispatch_record_all_legacy_fields_default_instead_of_unknown`。

6. 已做：Rust `Reply` 镜像追加带 `#[serde(default)]` 的 `tokens_cache_read`；派发结果和对账结果继续从同一 `cache_read` 用量写出 camelCase `cacheRead`（`src/orchestration.rs:83`、`src/dispatch.rs:484`、`src/dispatch.rs:2118`）／已补：`task07_reply_cache_read_roundtrips_and_defaults_for_old_graphs`、`task07_dispatch_and_reconcile_results_preserve_cache_read_usage`。

7. 已做：8MB 截断测试先等待生产者把普通行与截断标记入队再消费；回滚测试先用 512 B 总上限证明快照器会拒绝同一目录，再证明回滚路径完整读取；幂等释放测试在手工占用释放后两次调用 `dispatch_stage_core`，同时覆盖提前返回后的 RAII 再释放（`src/process.rs:604`、`src/dispatch.rs:3559`、`src/dispatch.rs:3686`）／已补：强化既有 `interactive_stdout_truncation_marker_is_delivered_at_full_queue_boundary`、`task06_rollback_reads_before_directory_beyond_snapshot_total_limit`、`task06_duplicate_active_run_is_rejected_before_dispatch`。

没做：B. 界面（TS）；未启动窗口、未发真机请求、未运行 `cargo fmt`、未 commit、未 push。`src/server.rs` 未修改并保持 CRLF；其余修改的 Rust 文件保持 LF。`src/channels/acp.rs` 只增加工具调用/活动计数的透传、采集和对应测试，没有改变 ACP 协议处理决策。

## 验证命令与输出原文

### `cargo check`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.42s
```

Rust warning 为 0。

### `cargo test -- --nocapture`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 6.93s
     Running unittests src\main.rs (target\debug\deps\aicross-3605bca6cf2a3148.exe)

running 272 tests
test channels::acp::tests::live_kimi_acp_two_turn_continuation ... ignored, 编排者在沙箱外使用已登录的 kimi 运行
test dispatch::tests::live_arena_and_arbitration ... ignored, three real model requests (擂台两家 + 仲裁一家); run outside the sandbox for TASK-05B acceptance
test dispatch::tests::live_blind_review ... ignored, real model requests; run once for TASK-03 acceptance
test dispatch::tests::live_cc_switch_route ... ignored, one real cc-switch request; requires AICROSS_CC_SWITCH_PATH and provider
test dispatch::tests::live_economy_pipeline ... ignored, three real model requests; run once outside the sandbox for TASK-04 acceptance
test dispatch::tests::live_shared_continuation ... ignored, two real model requests; run once for TASK-03 acceptance
test resources::tests::live_codex_and_kimi_fact_sources_parse ... ignored, explicit local fact-source check
test resources::tests::live_detect_four_agents ... ignored, 真机验收：探测四个内置 agent（内存 store，不写 resources.json）
test resources::tests::live_smoke_pi_models ... ignored, 真机验收：冒烟两条 pi 模型（2 次模型请求；内存 store，不写盘）
test shell_probe::tests::end_to_end_detects_installed_pwsh_with_version ... ignored
test shell_probe::tests::finds_msix_pwsh_alias ... ignored
test result: ok. 261 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 9.62s
```

11 条 ignored 均为既有真机、登录态或环境探测测试；没有发起真机模型请求。

### `cd src-ui && npx vitest run --configLoader native --pool threads`

```text
 RUN  v5.0.0 D:/work/aicross-p4/src-ui

 Test Files  21 passed (21)
      Tests  254 passed (254)
   Start at  21:09:31
   Duration  511ms (transform 63%, import 18%, tests 15%, worker 4%)
```

命令同时打印 Node 环境提示：`Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.`；测试本身全通过。

### `cd src-ui && npm run build`

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build

vite v8.0.7 building client environment for production...
transforming...✓ 322 modules transformed.
rendering chunks...
computing gzip size...
✓ built in 757ms
```

命令同时打印同一条 `NO_COLOR/FORCE_COLOR` Node 环境提示；TypeScript 与 Vite 构建成功。

### `git diff --check`

```text
```

退出码 0，无空白错误。

## 修正轮 R1

1. 已修：心跳循环抽为可注入间隔与闭包的 `heartbeat_loop`，只在 `Timeout` 发 `running`，收到停止信号或 `Disconnected` 立即退出；终态仍在心跳线程停止后发送（`src/dispatch.rs:741`、`src/dispatch.rs:2612`）／已补：`r1_heartbeat_normal_stop_emits_once_then_exits`、`r1_heartbeat_disconnected_stop_channel_exits_without_more_emits`、`r1_terminal_progress_after_heartbeat_stop_has_no_running_fields_or_late_emit`。

2. 已修：`running_progress` 的 `lastTool` 先走 `safe_trace` 脱敏，再按 Unicode 字符截到 120 字符（`src/dispatch.rs:2624`）／已补：`r1_running_progress_redacts_authorization_bearer_title`、`r1_running_progress_truncates_last_tool_to_120_characters`。

3. 已修：`ProcessActivity::reset_attempt` 重置本次尝试起点与工具计数、保留最后输出时间；每次 `run_one_attempt` 以及 ACP 转一次性回退前调用，主／备和 ACP／一次性不跨尝试累计，`elapsedMs` 与该次留痕 `duration_ms` 同为本次尝试起算（`src/process.rs:61`、`src/dispatch.rs:1768`、`src/dispatch.rs:1927`）／已补：`r1_reset_attempt_clears_tool_counts_and_restarts_elapsed_only`、`r1_backup_attempt_does_not_inherit_primary_tool_counts`、`r1_acp_to_oneshot_fallback_resets_attempt_tool_counts`。

4. 已修：当前 session 的 `session/request_permission.params.toolCall` 也计数；同时补齐 `session/update` 的 sessionId 隔离和无 title 仍递增计数，未改变权限决策（`src/channels/acp.rs:480`、`src/channels/acp.rs:503`）／已补：`r1_permission_request_tool_call_is_counted`、`r1_other_session_tool_call_is_not_counted`、`r1_untitled_tool_call_still_increments_count`。

5. 已修：旧 `.before` 是否目录的校验移到删除 `.prev` 前；新快照写成后清理 `.prev` 失败降级为 runner note，仍返回有效 `before_dir`（`src/dispatch.rs:1084`、`src/dispatch.rs:1102`、`src/dispatch.rs:1165`、`src/dispatch.rs:2037`）／已补：`r1_before_root_file_error_preserves_existing_prev_directory`、`r1_committed_snapshot_cleanup_failure_returns_note_not_error`。

6. 已修：一次性 idle 测试改为先输出后静默 2 秒，并增加第二次输出后 idle 小于 500 ms；cacheRead 分别钉住 `parse_output` 与从留痕对账回填；零改动重派走两次 `run_one` 后验证旧快照仍被回滚查找命中（`src/process.rs:633`、`src/process.rs:647`、`src/dispatch.rs:3661`、`src/dispatch.rs:3672`、`src/dispatch.rs:3724`）／已补：`oneshot_output_then_silent_two_seconds_reports_idle_from_last_output`、`oneshot_second_output_resets_idle_below_half_second`、`r1_parse_output_populates_dispatch_cache_read`、`r1_reconcile_refills_cache_read_from_record`、`r1_zero_change_redispatch_keeps_old_snapshot_discoverable_for_rollback`。

7. 已修：`parse_record_file` 校验 `schema == aicross-dispatch/1`；未知键仍忽略，缺键仍按兼容默认值读取（`src/dispatch.rs:405`）／已补：`r1_record_unknown_key_is_ignored_and_missing_keys_use_defaults`、`r1_record_schema_mismatch_is_rejected`。

8. 已修：契约明确工具计数按本次尝试统计，并写明终态事件不带心跳字段、前端保留上一条 running 的 `elapsedMs`／`lastTool`（`tasks/TASK-07-idle-and-tail.md:7`）／已补：`r1_terminal_progress_after_heartbeat_stop_has_no_running_fields_or_late_emit`。

GLM 非重复必修项已并入第 4、6 条：跨 session 工具通知不计、无 title 仍计、ACP 完整链测试单独放宽为 10 秒。未启动窗口、未发真机请求、未运行 `cargo fmt`、未 commit、未 push。

### R1 验证命令与输出原文

#### `cargo check`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.49s
```

warning 为 0。

#### `cargo test`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 8.10s
     Running unittests src\main.rs (target\debug\deps\aicross-3605bca6cf2a3148.exe)

running 291 tests
test channels::acp::tests::live_kimi_acp_two_turn_continuation ... ignored, 编排者在沙箱外使用已登录的 kimi 运行
test dispatch::tests::live_arena_and_arbitration ... ignored, three real model requests (擂台两家 + 仲裁一家); run outside the sandbox for TASK-05B acceptance
test dispatch::tests::live_blind_review ... ignored, real model requests; run once for TASK-03 acceptance
test dispatch::tests::live_cc_switch_route ... ignored, one real cc-switch request; requires AICROSS_CC_SWITCH_PATH and provider
test dispatch::tests::live_economy_pipeline ... ignored, three real model requests; run once outside the sandbox for TASK-04 acceptance
test dispatch::tests::live_shared_continuation ... ignored, two real model requests; run once for TASK-03 acceptance
test resources::tests::live_codex_and_kimi_fact_sources_parse ... ignored, explicit local fact-source check
test resources::tests::live_detect_four_agents ... ignored, 真机验收：探测四个内置 agent（内存 store，不写 resources.json）
test resources::tests::live_smoke_pi_models ... ignored, 真机验收：冒烟两条 pi 模型（2 次模型请求；内存 store，不写盘）
test shell_probe::tests::end_to_end_detects_installed_pwsh_with_version ... ignored
test shell_probe::tests::finds_msix_pwsh_alias ... ignored
test result: ok. 280 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 10.39s
```

11 条 ignored 均为既有真机、登录态或环境探测测试；没有发起真机模型请求。

#### `cd src-ui && npx vitest run --configLoader native --pool threads`

```text
 RUN  v5.0.0 D:/work/aicross-p4/src-ui

 Test Files  21 passed (21)
      Tests  254 passed (254)
   Start at  21:40:01
   Duration  497ms (transform 61%, import 19%, tests 16%, worker 4%)
```

命令另原样打印多次 Node 环境提示：`Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.`；测试全部通过。

#### `cd src-ui && npm run build`

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build

vite v8.0.7 building client environment for production...
transforming...✓ 322 modules transformed.
rendering chunks...
computing gzip size...
✓ built in 602ms
```

命令另原样打印 Node 环境提示：`Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.`；TypeScript 与 Vite 构建成功。
