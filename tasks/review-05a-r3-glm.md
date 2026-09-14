复审完成。逐条结论如下。

## 6 条逐项核对

**第 1 条（超时/失败/取消留痕 runner=acp）— 已核实修好**
`src/dispatch.rs:1319-1344`：Cancelled/Failed 分支均置 `runner="acp"`，note 分别为「ACP 已取消/超时/失败，未回退」；超时按 `acp_error.kind == AcpErrorKind::Timeout` 判定（`src/dispatch.rs:1330`），全文件已无 `contains("超时")` 文本匹配。结果卡对 `runner==='acp'` 显示「长驻」（`DispatchOverlay.tsx:423`）。

**第 2 条（run_one 端到端两条）— 已核实修好**
两条测试直接调用 `run_one`（`src/dispatch.rs:2360`、`src/dispatch.rs:2418`），非复刻。超时用例断言 `!count.exists()`（stub 零调用，比「零二次调用」更强）；死进程用例断言 `runner=oneshot`、note 含「ACP 会话已重置」、stub 恰好 1 次调用、ARGV repr 无 `'-S'`/`'-r'`、task file 重写为完整上下文（`src/dispatch.rs:2436-2441`）。测试注入走 `#[cfg(test)]` 的 `test_provider_parts`（`src/channels/mod.rs:111-122`、`src/channels/acp.rs:189-197`），生产构建下该分支不编译，生产路径未改坏。回退驱动真实：第二轮 ACP 尝试对新进程传旧 session id → `ForeignSession` → 可回退，走的是生产分流。

**第 3 条（8MB 水位）— 已核实修好**
`src/process.rs:23`、`src/process.rs:130-164`：水位为「队列中未消费字节」的 `AtomicUsize`，入队前 `fetch_update`（CAS，预留失败即丢行并只发一次标记），消费时 `fetch_sub` 归还（`recv_line`/`try_recv_line`）。递增只在唯一读线程，递减只对应已入队行，CAS 自旋见到最新值——无竞态、无下溢可能。`two_five_megabyte_turns_are_consumed_without_lifetime_truncation`（`acp.rs:903`）覆盖两轮各 5MB。

**第 4 条（并发真实 run）— 已核实修好**
`src/channels/acp.rs:966-984`：两线程同 key 并发 `run()`，fake server 用 `--start-file` 计数启动（`fixtures/acp/fake_server.py:16-20`），断言恰好 1 行；旧的手写 `slot.client.lock()` 复刻测试已删除（grep 无 `same_key_concurrent_initialization`）。

**第 5 条（跨 runner 另一半 + 前端对齐）— 已核实修好**
判定改为 `!uses_acp`，其中 `uses_acp = acp_enabled && supports()`（`src/dispatch.rs:1282-1286`），覆盖「ACP 开着但 agent 不支持」；测试 `unsupported_acp_previous_runner_resets_session_and_uses_full_context`（`src/dispatch.rs:2446`，cc-switch + acp_enabled=true + previous_runner=acp）。前端 `continuationRunner` 以 `continuationSession` 为前提（`src-ui/src/graph/dispatch.ts:111`），盲节点测试两者都断言 undefined（`dispatch.test.ts:139-146`）；`docs/PRODUCT.md:184` 措辞与代码一致。一个小保留：`!uses_acp` 分支里池中旧 ACP session 并未服务端 close（「已重置」只是本地不再续用），系 R2 既有口径，正确性不受影响。

**第 6 条（小尾巴）— 已核实修好**
a) `legacy_cache_read_tokens_shape_is_parsed`（`acp.rs:898`）+ `parse_usage` 兜底（`acp.rs:590`）；b) 取消测试断言 `client.sessions.is_empty()`（`acp.rs:864`）；c) 超时按类型（见第 1 条）；d) REPORT 契约清单含 `runner_note`/`previousRunner`/`runner:'unknown'`（`REPORT-05A.md:25-27`）；e) 见第 5 条。

## 编排者改动评估（无副作用）

- `configure_session` 固定 `mode=default`（`acp.rs:371-376`）：只读保护不靠 plan 模式，靠 `answer_permission` 对写/执行类 `reject_once`（`acp.rs:506-510`），已由真机 live 两轮验证；无残留 plan 声明（src 全 grep 无），`SettingsModal.tsx:548` 文案与之一致。
- live 测试不断言 usage（`acp.rs:1024-1026`）与真机观测（prompt 结果只有 `stopReason`）相符。
- `AICROSS_ACP_DEBUG` 打印（`acp.rs:422`、`acp.rs:456`）走宿主进程 stderr，留痕的 `raw_stdout/raw_stderr` 只采子进程管道，**不会落入留痕**；截断有 char-boundary 保护；默认关闭。

## 新缺陷扫描

未发现。一次性回归测试仍在且通过（`acp_disabled_or_unsupported_preserves_oneshot_dispatch_result`）；`DispatchRecord` 只追加且带 serde default（旧留痕解析有测试）；Reply 类型只追加可选 `runner`（`types.ts:22`）；`server.rs:3858-3859`/`4012` 注册完整；`tauri.ts:397/439/477-478` 类型镜像齐全。仅一处**引用偏差**（非代码问题）：REPORT R3 第 2 条写 `src/channels/mod.rs:107`，实际 cfg(test) 注入分支在 111-122。

## 结论

**TASK-05A 在 R3 后达到可交付**——6 条全部修好且验证真实（208/130/零 warning、live 真机 1 passed），未引入新缺陷。

仍未闭合的项（按严重度，均为既有已知项、非阻塞）：
1. ACP 失败回退重试无 turn id 契约，agent 已部分执行写操作时可能重复执行（REPORT「不确定处」已登记，需后续协议层设计）。
2. kimi ACP 不回 usage/真身，结果表用量恒为「—」，只能靠 runner_note 说明（适配 kimi 侧开放）。
3. `!uses_acp` 跨 runner 分支不主动 close 池内旧 session，依赖 shutdown/进程退出回收（低危资源项）。
4. 同模型并发多 session 吞吐未验证（池内 prompt 串行，性能项）。
5. 文档小尾巴：REPORT `mod.rs:107` 行号引用偏差；超大积压后标记行预留失败时不补发标记（`process.rs:152`，极边缘）。