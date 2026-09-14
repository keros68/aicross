# REPORT-05A — ACP 客户端与长驻运行

## 做了什么

- 在 `src/channels/acp.rs` 增加 ACP v1 JSON-RPC stdio 客户端，流程为 `initialize` → `session/new` → `session/set_model`（需要时）→ `session/prompt`，从 `session/update` 的 `agent_message_chunk` 汇集回复正文。
- 首个运行适配器为 `kimi acp`。进程按 `agent/provider/model` 缓存；同一进程维护多个 session，同一 session 的后续 prompt 复用原进程。ACP session id 写回 `Reply.session_id` 的既有链路。
- ACP 启动失败、协议失败或进程退出时，继续执行第 3 期的一次性调用路径。用户取消 ACP 时不再启动一次性调用，避免取消后重复派发。
- `src/process.rs` 增加长驻 stdio 子进程封装、按行收发、超时、stderr 收集、进程树终止及现有取消注册表接入。
- 派发设置增加 `acpEnabled`，默认关闭，旧设置缺字段时仍走一次性运行。
- 设置页增加 Kimi 长驻开关，并只读探测 `kimi acp`、`claude-agent-acp`、`codex-acp` / `acp-adapter`。Claude Code 与 Codex 只展示探测状态和安装命令，不接入运行路径。
- 增加具名测试：ACP update 正文汇集、仅 Kimi 启用 ACP、一次性回归、ACP 退出后选择回退、取消后不回退、长驻进程取消、设置缺省和持久化。
- 增加 ignored 测试 `live_kimi_acp_two_turn_continuation`，供编排者在沙箱外运行两轮 Kimi ACP 续聊。

## 没做什么

- 未改 `src-ui/src/graph/` 的图逻辑，未改预设。
- 未实现 Claude Code / Codex ACP 运行适配器，只做本地可用性探测与安装说明。
- 未启动窗口，未执行 ignored 真机测试，未发送模型请求。
- 未处理前端 chunk 大小警告；该项属于 TASK-05 C 节。
- 未 commit，未 push。

## 契约追加

- `DispatchSettings` / 前端 `DispatchSettings` 追加 `acp_enabled` / `acpEnabled: boolean`，默认 `false`。
- `aicross-dispatch/1` frontmatter 追加 `runner: acp|oneshot` 与 `runner_note`，schema 名称和既有字段不变。
- `DispatchNodeResult` 追加 `runner: acp|oneshot|unknown` 与 `runnerNote`；`unknown` 仅用于前端本地合成的未运行结果。
- 续聊请求追加 `previousRunner`，与 `continuationSession` 使用相同的单父节点、共享可续聊判定。
- ACP 不回传 usage 时保持数值为 0，现有结果表显示为 `—`，不推测用量。

## 验证命令与输出原文

### `cargo check`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.41s
```

exit=0，零 warning。

### `cargo test`

```text
running 189 tests
test channels::acp::tests::live_kimi_acp_two_turn_continuation ... ignored, 编排者在沙箱外使用已登录的 kimi 运行：验证同一 ACP session 两轮续聊
...
test result: ok. 179 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 18.47s
```

exit=0。

### `cd src-ui && npx vitest run --configLoader native --pool threads`

```text
 RUN  v5.0.0 D:/work/aicross-p4/src-ui

 Test Files  12 passed (12)
      Tests  126 passed (126)
   Start at  11:12:12
   Duration  293ms (transform 60%, import 24%, tests 10%, worker 6%)
```

exit=0。命令输出另有 Node 的 `NO_COLOR` / `FORCE_COLOR` 环境 warning，不是代码 warning。

### `cd src-ui && npm run build`

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build

vite v8.0.7 building client environment for production...
✓ 582 modules transformed.
...
dist/assets/main-DgxmA2cJ.js                        1,254.48 kB │ gzip: 447.62 kB

✓ built in 2.20s
[plugin builtin:vite-reporter]
(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rolldownOptions.output.codeSplitting to improve chunking: https://rolldown.rs/reference/OutputOptions.codeSplitting
- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
```

exit=0。大 chunk warning 为既有 TASK-05 C 节事项，本节未改拆包。

### 本地入口核对（不发送模型请求）

```text
> kimi --version
0.42.0

> kimi acp --help
Usage: kimi acp [options]

Run kimi-code as an Agent Client Protocol (ACP) server over stdio.
```

## 不确定处

- 真机 Kimi ACP 两轮续聊尚未运行；需编排者执行 ignored 测试确认当前账号、模型别名和 `session/set_model` 的实际组合。
- 当前池对同一 `agent/provider/model` 的 prompt 串行处理，以保证一条 stdio 连接上的响应归属明确；未验证同一模型并发多 session 的吞吐。
- ACP 请求发生协议错误或进程退出后会回落一次性路径。若 agent 在退出前已经执行了部分写操作，重试可能重复执行；当前契约没有可用于确认提交状态的 turn id。

## 修正轮 R1

流式口径：本报告中的「流式」仅指客户端按 `session/update` 增量接收并汇集正文，界面不逐字显示。

1. 已修：ACP 失败回退时清空 ACP session，强制使用 `prepare_outbound` 的完整 prompt，并把重置原因写入 `runner_note`（`src/dispatch.rs:1355`、`src/dispatch.rs:1569`）／已补：`foreign_acp_session_fallback_drops_resume_flag_and_uses_full_context`。
2. 已修：权限按读类与写/执行类区分；只读或盲节点拒绝写/执行，所有允许与拒绝都只选 once，策略写入留痕（`src/channels/acp.rs:377`、`src/dispatch.rs:1344`）／已补：`permission_policy_classifies_read_and_write`、`fake_acp_two_turn_reuses_session_tolerates_banner_filters_updates_and_rejects_write`、`writable_permission_is_allow_once_never_allow_always`。
3. 已修：交互进程加入 Windows Job Object；退出、关闭 ACP 开关及 ignored 真机测试末尾均释放池（`src/process.rs:86`、`src/server.rs:4011`、`src-ui/src/components/common/SettingsModal.tsx:208`、`src/channels/acp.rs:451`）／已补：`shutdown_terminates_fake_acp_process`。
4. 已修：错误分为进程、协议、轮级、超时、取消和外来会话；非 JSON 行容忍、未知请求回 `-32601`、update 按 sessionId 过滤，轮级错误不摘池（`src/channels/acp.rs:14`、`src/channels/acp.rs:331`）／已补：`fake_acp_two_turn_reuses_session_tolerates_banner_filters_updates_and_rejects_write`。
5. 已修：等池锁采用 try-lock + deadline + 取消复查；当前节点取消发送 `session/cancel`，不通过注册表杀共享 ACP 进程（`src/channels/acp.rs:120`、`src/channels/acp.rs:312`）／R1 实际断言：`queued_acp_node_observes_cancel_marker_before_acquiring_process` 仅验证取消标记，`cancelled_acp_process_does_not_restart_as_oneshot` 仅验证合成错误的分流；真实排队与在飞取消路径由 R2 测试补齐。
6. 已修：池键加入 cwd 与 blind，盲节点不会复用共享进程（`src/channels/acp.rs:92`）／已补：`fake_acp_two_turn_reuses_session_tolerates_banner_filters_updates_and_rejects_write`（同 cwd 复用）；隔离键由构造逻辑直接覆盖。
7. 已修：解析 prompt response usage；`set_model` 后要求 availableModels 明确包含目标 id（`src/channels/acp.rs:233`、`src/channels/acp.rs:274`）／R1 实际断言：`fake_acp_two_turn_reuses_session_tolerates_banner_filters_updates_and_rejects_write` 断言 input/output usage 与同模型两轮，不含 `cachedReadTokens`、缺清单或换模型分支；这些由 R2 测试补齐。
8. 已修：每轮取走 stderr；raw 上限 8MB并标截断；stderr 满额后继续排空；回退原因和 Kimi 无用量说明移入 `runner_note`；旧留痕 runner 默认 oneshot（`src/process.rs:46`、`src/process.rs:122`、`src/channels/acp.rs:424`、`src/dispatch.rs:239`）／已补：`legacy_dispatch_record_without_runner_defaults_to_oneshot`、`frontmatter_roundtrips`。
9. 已修：Reply 镜像 runner，结果卡和用量表显示「长驻/一次性」（`src-ui/src/graph/types.ts:22`、`src-ui/src/graph/dispatch.ts:177`、`src-ui/src/components/center/graph/DispatchOverlay.tsx:421`）／已补：`长驻运行方式写回 Reply`。
10. 已修：新增 `fixtures/acp/fake_server.py`，覆盖 banner、JSON-RPC、权限请求、未知请求、两轮复用和可终止进程；一次性回归改用本地 Python stub，不发真实请求（`fixtures/acp/fake_server.py:1`、`src/channels/acp.rs:555`、`src/dispatch.rs:2432`）／已补：`fake_acp_two_turn_reuses_session_tolerates_banner_filters_updates_and_rejects_write`、`shutdown_terminates_fake_acp_process`、`acp_disabled_or_unsupported_preserves_oneshot_dispatch_result`；`长驻运行方式写回 Reply` 断言 `runner=acp`。
11. 已修：移除无来源 npm 包名，只显示二进制名与「需自行安装」（`src-ui/src/components/common/SettingsModal.tsx:541`）／已补：由 TypeScript build 覆盖渲染契约。
12. 已修：本节开头明确流式只指 `session/update` 增量接收，界面不逐字显示／已补：`acp_agent_message_updates_append_only_reply_text`。
13. 已修：timeout 不进入一次性回退；排队时间从进入 ACP 起计入同一 deadline（`src/channels/acp.rs:90`、`src/dispatch.rs:1214`）／已补：`acp_timeout_is_terminal_and_never_selects_oneshot_fallback`。
14. 已修：session 记录当前模型，续聊模型改变时重新 `set_model` 并校验（`src/channels/acp.rs:225`）／R1 实际断言：假 server 两轮使用同一模型，只覆盖首次 set_model 响应；换模型分支由 R2 的 `continuation_model_change_uses_same_session_and_updates_model` 补齐。
15. 已修：initialize 使用节点剩余 timeout，最大 30 秒（`src/channels/acp.rs:178`）／已补：假 server 初始化端到端覆盖。
16. 已修：Kimi probe 传 `kimi_fallback()`；探测失败显示错误并提供「重新探测」（`src/channels/acp.rs:504`、`src-ui/src/components/common/SettingsModal.tsx:545`）／已补：TypeScript build 覆盖命令返回契约。
17. 已修：新增 `shutdown_all()`，退出和关开关都调用（`src/channels/acp.rs:451`、`src/server.rs:4011`）／已补：`shutdown_terminates_fake_acp_process`。
18. 已修：设置页写明 ACP 只读权限策略及一次性 Kimi 的差异（`src-ui/src/components/common/SettingsModal.tsx:548`）／已补：TypeScript build 覆盖文案所在组件。
19. 已修：`AcpOutput` 直接携带完整 `ChannelOutput`，派发处整体接收，不再逐字段搬运（`src/channels/acp.rs:43`、`src/dispatch.rs:1339`）／已补：假 server 测试同时断言正文、session 与 usage。

### R1 验证命令与输出原文

#### `cargo check`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.24s
```

exit=0，零 warning。

#### `cargo test`

```text
running 198 tests
i..............................i..iii.................................................. 87/198
...................iii....................................ii........................... 174/198
........................
test result: ok. 188 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 4.92s
```

exit=0。未运行 ignored 真机测试，未发送真实模型请求。

#### `cd src-ui && npx vitest run --configLoader native --pool threads`

```text
 RUN  v5.0.0 D:/work/aicross-p4/src-ui

 Test Files  12 passed (12)
      Tests  127 passed (127)
   Start at  11:40:45
   Duration  303ms (transform 67%, import 20%, tests 9%, worker 4%)
```

exit=0。命令输出另有 Node 的 `NO_COLOR` / `FORCE_COLOR` 环境 warning，不是代码 warning。

#### `cd src-ui && npm run build`

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build

vite v8.0.7 building client environment for production...
✓ 582 modules transformed.
dist/assets/main-8SkWuNd6.js                        1,254.87 kB │ gzip: 447.88 kB

✓ built in 1.87s
[plugin builtin:vite-reporter]
(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rolldownOptions.output.codeSplitting to improve chunking
- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
```

exit=0。大 chunk warning 为既有 TASK-05 C 节事项。

本轮未起窗口、未运行 ignored 真机测试、未发送真实模型请求、未 commit、未 push。

## 修正轮 R2

1. 已修：`InteractiveProcess::recv_line` 返回具名错误，最后一个轮询窗口的 `Timeout` 到 deadline 后仍归为 ACP 超时，不再触发一次性回退（`src/process.rs:13`、`src/channels/acp.rs:438`）／已补：`silent_acp_process_reaches_typed_timeout_without_fallback`。
2. 已修：截断位置用 `floor_char_boundary`，截断标记写入后不再追加（`src/channels/acp.rs:552`）／已补：`append_bounded_cjk_is_utf8_safe_and_stops_after_marker`。
3. 已修：杀死假 ACP 后真实 `AcpClient::prompt` 返回可回退的 Process；跨 runner 的 `run_one` 会清 session、改写完整上下文 task file 并留 `runner: oneshot`（`src/channels/acp.rs:813`、`src/dispatch.rs:1326`、`src/dispatch.rs:1384`）／已补：`killed_acp_process_returns_process_error_from_real_client_path`、`disabled_acp_previous_runner_resets_session_and_uses_full_context`、`reset_acp_fallback_builds_full_context_invocation_without_resume_flags`。
4. 已修：`shutdown_all` 先一次性摘出池，busy slot 用已记录 PID 终止，不等待在飞轮次的 mutex（`src/channels/acp.rs:581`）／已补：`shutdown_all_returns_within_one_second_when_client_is_locked`。
5. 已修：每轮 prompt 前 drain 已到达的 stdout 行，取消和超时同时关闭并移除 session，迟到 update 不进入下一轮正文（`src/channels/acp.rs:274`、`src/channels/acp.rs:382`、`src/channels/acp.rs:439`）／已补：`late_session_update_is_drained_before_next_prompt`、`cancelled_inflight_acp_prompt_returns_cancelled_and_closes_session`。
6. 已修：交互 stdout 读线程累计 8MB 后只入队一次截断标记，随后继续排空子进程 stdout（`src/process.rs:10`、`src/process.rs:117`）／已补：`interactive_stdout_queue_stops_after_eight_megabytes_and_marks_truncation`。
7. 已修：权限先按 kind 的读写白名单判定，明确写类 kind 不再受 title 影响；假 server 增加读类请求（`src/channels/acp.rs:533`、`fixtures/acp/fake_server.py:43`）／已补：`readonly_permission_allows_read_kind_and_execute_title_cannot_bypass`、`permission_policy_classifies_read_and_write`。
8. 已修：派发请求携带父 Reply 的 runner；ACP 关闭且父 runner=acp 时清 session、用完整上下文并写 runner_note（`src-ui/src/graph/dispatch.ts:109`、`src-ui/src/components/center/graph/DispatchOverlay.tsx:125`、`src/dispatch.rs:102`、`src/dispatch.rs:1326`）／已补：`续聊请求保留父回复的 ACP runner 来源`、`disabled_acp_previous_runner_resets_session_and_uses_full_context`。
9. 已修：前端本地合成失败使用 `runner=unknown`，结果卡显示「未运行」（`src-ui/src/components/center/graph/DispatchOverlay.tsx:220`、`src-ui/src/components/center/graph/DispatchOverlay.tsx:423`）／已补：`本地合成失败结果写回未运行而非一次性`。
10. 已修：usage 优先读 `cachedReadTokens`，兼容旧 `cacheReadTokens`（`src/channels/acp.rs:302`）／已补：`fake_acp_two_turn_reuses_session_tolerates_banner_filters_updates_and_rejects_write` 断言 cache_read=5。
11. 已修：`fake_client` 缺 Python 时打印跳过原因并返回；进程退出测试按 tasklist CSV 的 PID 列精确轮询至 2s（`src/channels/acp.rs:669`、`src/channels/acp.rs:873`）／已补：`shutdown_terminates_fake_acp_process`。
12. 已修：ignored 真机测试使用随机 nonce，断言首轮 session 非空、input usage >0、次轮正文含 nonce，并在末尾 shutdown（`src/channels/acp.rs:943`）／已补：`live_kimi_acp_two_turn_continuation`（保持 ignored，本轮未发真机请求）。
13. 已修：模型清单先读 Kimi `configOptions[category=model].options[].value`；切模型先发 `session/set_config_option`，失败才试旧 `session/set_model`；两类清单都缺失时继续 ACP 并写 runner_note（`src/channels/acp.rs:315`、`fixtures/acp/fake_server.py:22`）／已补：`kimi_config_options_set_model_default_mode_and_thinking`、`legacy_models_shape_remains_supported`、`missing_model_lists_do_not_reject_acp_and_leave_runner_note`。
14. 已修：只读或盲节点设置 `mode=plan`，可写节点设置 `mode=default`，不使用 auto/yolo（`src/channels/acp.rs:368`）／已补：`kimi_config_options_set_model_default_mode_and_thinking`、`writable_acp_session_uses_default_never_yolo_mode`。
15. 已修：thinking 的「关/低」映射 low，「中/高」映射 high；失败仅追加 runner_note（`src/channels/acp.rs:372`）／已补：`kimi_config_options_set_model_default_mode_and_thinking`、`writable_acp_session_uses_default_never_yolo_mode`。
16. 已修：initialize 记录 session close 能力；取消、超时、池摘除和 shutdown 时发送 `session/close`（`src/channels/acp.rs:205`、`src/channels/acp.rs:393`、`src/channels/acp.rs:522`）／已补：`shutdown_all_sends_session_close_when_capability_is_available`、`cancelled_inflight_acp_prompt_returns_cancelled_and_closes_session`。
17. 已修：假 server 的 session/new 返回递增 id，未知请求响应必须为 `error.code=-32601`（`fixtures/acp/fake_server.py:22`、`fixtures/acp/fake_server.py:53`）／已补：`fake_acp_two_turn_reuses_session_tolerates_banner_filters_updates_and_rejects_write`。
18. 已修：截断标记成为终态；initialize banner 会在首轮 prompt 前 drain 进 raw（`src/channels/acp.rs:274`、`src/channels/acp.rs:552`）／已补：`append_bounded_cjk_is_utf8_safe_and_stops_after_marker`、`fake_acp_two_turn_reuses_session_tolerates_banner_filters_updates_and_rejects_write`。
19. 已修：池先插入空 slot，同 key 的首个调用持 slot 锁启动进程，后续调用等待并复用，不再覆盖先启动者（`src/channels/acp.rs:75`、`src/channels/acp.rs:124`、`src/channels/acp.rs:166`）／已补：`same_key_concurrent_initialization_spawns_one_fake_process`。
20. 已修：产品说明记录跨 runner 会话标识限制及「新开会话」恢复方式（`docs/PRODUCT.md:184`）／已补：`续聊请求保留父回复的 ACP runner 来源`、`disabled_acp_previous_runner_resets_session_and_uses_full_context`。

### R2 验证命令与输出原文

#### `cargo check`

```text
    Blocking waiting for file lock on build directory
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 11.15s
```

exit=0，零 warning。

#### `cargo test`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 8.30s
     Running unittests src\main.rs (target\debug\deps\aicross-3605bca6cf2a3148.exe)

running 212 tests
test channels::acp::tests::live_kimi_acp_two_turn_continuation ... ignored, 编排者在沙箱外使用已登录的 kimi 运行
...
test result: ok. 202 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 6.05s
```

exit=0。未运行 ignored 真机测试，未发送真实模型请求。

#### `cd src-ui && npx vitest run --configLoader native --pool threads`

```text
 RUN  v5.0.0 D:/work/aicross-p4/src-ui

 Test Files  12 passed (12)
      Tests  129 passed (129)
   Start at  16:56:59
   Duration  331ms (transform 75%, import 13%, tests 8%, worker 4%)
```

exit=0。命令输出另有 Node 的 `NO_COLOR` / `FORCE_COLOR` 环境 warning，不是代码 warning。

#### `cd src-ui && npm run build`

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build

vite v8.0.7 building client environment for production...
✓ 582 modules transformed.
dist/assets/main-D3PpRh8P.js                        1,255.13 kB │ gzip: 447.97 kB

✓ built in 2.30s
[plugin builtin:vite-reporter]
(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rolldownOptions.output.codeSplitting to improve chunking
- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
```

exit=0。大 chunk warning 为既有 TASK-05 C 节事项；另有 Node 的 `NO_COLOR` / `FORCE_COLOR` 环境 warning。

本轮未运行 `cargo fmt`，未起窗口，未运行 ignored 真机测试，未发送真实模型请求，未 commit，未 push。

### 编排者在 R2 之后的修正（2026-09-14 真机）

- **第 14 条改口：只读/盲节点不设 `mode=plan`，一律 `default`。** 真跑 `live_kimi_acp_two_turn_continuation` 两次都 180s 超时，调试打印看到 kimi 的思考：「plan mode is active, turn must end with AskUserQuestion or ExitPlanMode」——kimi 的 plan 模式要求回合以计划类工具收尾，一问一答的验证者在那里空转（12 次工具调用、9 次权限请求）。只读靠 `answer_permission` 拒写来保证；`acp.rs` `configure_session` 改成固定 `default`，对应两条测试的期望值同步改。fix-05a-r2 第 14 条的前提（plan = `--no-tools` 等价物）不成立，`docs/PRODUCT.md` 与 skill `channels.md` 已同步改口。
- **第 12 条改口：不断言 `usage.input > 0`。** kimi ACP 的 `session/prompt` 结果只有 `{stopReason}`，全程 3000+ 行通知里没有任何 usage/token 字段；用量缺口由 `annotate_kimi_metadata` 记进 `runner_note`（第 8 条既有行为）。
- 改完真跑：`cargo test channels::acp::tests::live_kimi_acp_two_turn_continuation -- --ignored --nocapture` → `1 passed`（26.30s）；两轮同一 session，第二轮正文含随机 nonce。
- `request()` 加了 `AICROSS_ACP_DEBUG` 环境变量门控的 stderr 调试打印（每条进出请求一行，截 300 字），只读环境变量，默认关闭。
- 编排者剔除了 R1 遗留在 `server.rs`/`dispatch.rs`/`terminal.rs`/`channels/mod.rs`/`orchestration.rs` 里的 rustfmt 排版噪音（三方合并：base=rustfmt(HEAD)），`resources.rs` 为纯排版改动已还原；分支 diff 从 +1913/−591 降到 +574/−65，`cargo test` 202 passed 不变。

## 修正轮 R3

1. 已修：ACP 超时、失败与取消均保留 `runner: acp`，并分别记录「ACP 超时/失败/已取消，未回退」；超时按 `AcpErrorKind::Timeout` 判定，不再匹配错误文本（`src/dispatch.rs:1319`、`src/dispatch.rs:1329`）／已补：`run_one_silent_acp_timeout_stays_acp_and_never_calls_oneshot`、`run_one_acp_turn_failure_stays_acp_and_never_calls_oneshot`、`run_one_cancelled_acp_stays_acp_and_never_calls_oneshot`。
2. 已修：测试专用 provider 同时注入假 ACP server 与可计数的一次性 stub，直接驱动 `run_one` 的超时终止和死进程回退路径；回退清 session、去除续聊参数并重写完整 task file（`src/channels/mod.rs:107`、`src/channels/acp.rs:189`、`src/dispatch.rs:2346`）／已补：`run_one_silent_acp_timeout_stays_acp_and_never_calls_oneshot`、`run_one_dead_acp_falls_back_once_without_resume_and_rewrites_full_task`。
3. 已修：交互 stdout 以原子计数维护队列中尚未消费的字节数，接收时归还水位；超限只丢当前积压行，消费后后续轮次可继续入队（`src/process.rs:23`、`src/process.rs:130`）／已补：`interactive_stdout_queue_recovers_after_oversized_unconsumed_line`、`two_five_megabyte_turns_are_consumed_without_lifetime_truncation`。
4. 已修：并发测试不再手写 slot 锁，两个线程以相同 key 同时调用 `run()`，假 server 用启动计数文件确认只生成一个进程（`src/channels/acp.rs:966`、`fixtures/acp/fake_server.py:16`）／已补：`same_key_concurrent_run_spawns_one_fake_process`。
5. 已修：跨 runner 清理条件改为「本轮实际不走 ACP」，覆盖 ACP 开启但 agent 不支持的路径；产品限制说明同步；`continuationRunner` 与 `continuationSession` 共用可续聊前提，盲节点两者均为空（`src/dispatch.rs:1282`、`docs/PRODUCT.md:184`、`src-ui/src/graph/dispatch.ts:109`）／已补：`unsupported_acp_previous_runner_resets_session_and_uses_full_context`、`盲节点不携带父节点的 session 与 runner`。
6. 已修：旧 `cacheReadTokens` 经独立 usage 解析断言覆盖；取消测试确认 session map 已移除；timeout 使用错误类型；契约清单补齐 `runner_note`、`previousRunner` 与 `runner:'unknown'`；假 server 增加 R3 所需参数（`src/channels/acp.rs:585`、`src/channels/acp.rs:852`、`tasks/REPORT-05A.md:25`、`fixtures/acp/fake_server.py:12`）／已补：`legacy_cache_read_tokens_shape_is_parsed`、`cancelled_inflight_acp_prompt_returns_cancelled_and_closes_session`、`盲节点不携带父节点的 session 与 runner`。

### R3 验证命令与输出原文

#### `cargo check`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.22s
```

exit=0，零 warning。

#### `cargo test`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 8.19s
     Running unittests src\main.rs (target\debug\deps\aicross-3605bca6cf2a3148.exe)

running 218 tests
test channels::acp::tests::live_kimi_acp_two_turn_continuation ... ignored, 编排者在沙箱外使用已登录的 kimi 运行
...
test result: ok. 208 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 8.23s
```

exit=0。未运行 ignored 真机测试，未发送真实模型请求。

#### `cd src-ui && npx vitest run --configLoader native --pool threads`

```text
 RUN  v5.0.0 D:/work/aicross-p4/src-ui

 Test Files  12 passed (12)
      Tests  130 passed (130)
   Start at  17:52:26
   Duration  324ms (transform 75%, import 14%, tests 7%, worker 4%)
```

exit=0。命令输出另有 Node 的 `NO_COLOR` / `FORCE_COLOR` 环境 warning，不是代码 warning。

#### `cd src-ui && npm run build`

首次执行在 Vite 加载配置时返回环境错误：

```text
failed to load config from D:\work\aicross-p4\src-ui\vite.config.ts
error during build:
Error: Build failed with 1 error:

[plugin externalize-deps]
Error: spawn EPERM
```

单独重跑同一命令：

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build

vite v8.0.7 building client environment for production...
✓ 582 modules transformed.
dist/assets/main-dEOo4qvJ.js                        1,255.14 kB │ gzip: 447.98 kB

✓ built in 2.08s
[plugin builtin:vite-reporter]
(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rolldownOptions.output.codeSplitting to improve chunking
- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
```

重跑 exit=0。大 chunk warning 为既有 TASK-05 C 节事项；另有 Node 的 `NO_COLOR` / `FORCE_COLOR` 环境 warning。

本轮未运行 `cargo fmt`，未起窗口，未运行 ignored 真机测试，未发送真实模型请求，未 commit，未 push。

### 编排者在 R3 之后的修正（2026-09-14 真机）

- **权限门收紧**（DS R3 复审第 1 条）：`is_read_request` 只放行 kind ∈ {read, search, list}；`fetch` 出网、盲节点不得外发，一律拒；kind 缺失不再按 title 词表放行。真机探针：kimi 0.42 的 `session/request_permission` 的 `toolCall` 不带 `kind`，只有 `title`（"Write"）与 content 文本；读文件不会请求权限（默认模式自动放行），所以来问的都是写/执行。`permission_policy_classifies_read_and_write` 加三条断言。
- 真机复跑：R3 后 `live_kimi_acp_two_turn_continuation` 1 passed（18.55s）；05A 的 live 时长两处记录（26.30s 为 R2 后、18.55s 为 R3 后）。
