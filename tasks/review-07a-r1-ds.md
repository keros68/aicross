已读完 `tasks/fix-07a-r1.md`、`REPORT-07A.md`「修正轮 R1」、`dispatch.rs`/`process.rs`/`acp.rs`/`orchestration.rs`/`fake_server.py`/`TASK-07-idle-and-tail.md`，并逐条对照了两份 R0 审查。

## 逐条核对（10 条）

1. **已核实修好** — `heartbeat_loop` 只在 `Timeout` 发 running，`Ok(()) | Disconnected` 一律 break（`src/dispatch.rs:2612-2621`）；worker 侧 `send(())+join()` 在 `tx.send` 前、终态 emit 在 `recv()` 之后，running 不可能晚于终态（`src/dispatch.rs:741-747`、774-776）。三条测试 `3042/3055/3068` 真起线程并 join。
2. **已核实修好** — `safe_trace` 先脱敏再 `chars().take(120)`（`src/dispatch.rs:2626`），顺序正确（先截断会漏半个 token）；测试 `3085-3098` 钉住 `[REDACTED]` 与 120 字符。
3. **已核实修好** — `run_one_attempt` 开头 `reset_attempt()`（`src/dispatch.rs:1768`）、ACP→一次性回退前再重置（`1927`）；`started` 与 `timer` 同一行区间起算，`elapsedMs` 与 `duration_ms` 同口径（`2082`）；`ToolActivity` 整体归零、`last_output` 保留（`src/process.rs:59-66`）。主/备不跨尝试累加（测试 `3152-3162`）。
4. **已核实修好** — 权限请求的 `params/toolCall.title` 同样计数（`src/channels/acp.rs:505-513`），且不影响 `answer_permission` 决策；测试 `acp.rs:805` 用无 `--repeat-tool-calls` 的假 server，`calls==1` 只能来自权限请求，判别力足。
5. **已核实修好** — `root` 是否目录的校验在删 `.prev` 之前（`src/dispatch.rs:1121` 先于 `1124`）；写成功后 `.prev` 清理失败只产 note（`finish_before_snapshot` `1089-1098`、`1165`），调用方仍拿到有效 `before_dir` 并把 note 并入 `runner_note`（`2036-2043`）。测试 `3701`（root 是文件时旧 `.prev` 不被删）判别力强。
6. **已核实修好（一处偏弱）** — idle 改成「先输出→静默 2.1s」`idle ≥ 2000` +「二次输出」`idle < 500`（`src/process.rs:631-656`，读线程在 `finish_bounded_reader` 里 join，无竞态）；零改动重派走两次 `run_one` 再由 `rollback_node_changes_core` 命中 run-a 旧快照（`src/dispatch.rs:3713-3746`），真 E2E；对账链路走真命令 `reconcile_dispatch_state`（`3672-3686`）。弱项见末尾第 2 条。
7. **已核实修好** — `parse_record_file` 值校验 `schema == aicross-dispatch/1`（`src/dispatch.rs:412-414`），未知键忽略/缺键取默认（`2999-3006`）、不匹配报错（`3010-3017`）。全仓留痕写出的 schema 都是同一值，无回归面。
8. **已核实修好** — 契约节补了「本次尝试」与「终态事件不带心跳字段，前端保留上一条 running 的 elapsedMs/lastTool」（`tasks/TASK-07-idle-and-tail.md:7`）。
9. **已核实修好** — `session/update` 分支的计数被 `sessionId == 当前会话` 包住（`src/channels/acp.rs:480-492`），权限分支同样过滤（`505-513`）；无 title 走 `record_untitled_tool_call` 递增 `calls`。三条测试 `814/823` 对「跨 session 不计」「无 title 仍计」均有反向判别力。
10. **已核实修好** — 两条整链 ACP 测试 `settings.timeout_secs = 10`（`src/dispatch.rs:3111`、`3132`），acp 层测试用 5s deadline（`acp.rs:798/807/816/825`），不再沿用 fixture 的 1s。报告未单列第 9、10 条编号条目（并入第 4、6 条尾注），属报告体例问题。

**新增缺陷扫描**：主/备、ACP→一次性、worktree/写锁/验证/对账/回滚路径逐段未动；`acp.rs` 改动只在 `request()` 的两个通知分支加计数（`mark_output`/`answer_permission`/取消判定逐字未变）；`persist_before_snapshot` 降级为 `#[cfg(test)]` wrapper，生产路径走 `_with_note`，无死代码。未发现产品级新缺陷。

## R1 之后 TASK-07A 是否可交付

**可交付**：A 节硬需求 10 条全部闭合、无回归与新缺陷，剩余缺口都在「测试判别力/文档/跨工作树」这一层，不阻塞合并。

仍未闭合（按严重度）：

1. **（中-低）第 4 条的真机形态未验收**：`live_kimi_acp_two_turn_continuation` 传独立 `ProcessActivity` 且不断言 `toolCalls`（`src/channels/acp.rs:1064-1097`），「kimi 只在权限请求里带 title」目前只有假 server 证据；真机只证明了 ACP 两轮链路没被改坏。
2. **（低）cacheRead 派发链路测试自证**：`r1_parse_output_populates_dispatch_cache_read` 手工把 `output.usage.cache_read` 拷进 `DispatchNodeResult`（`src/dispatch.rs:3667`），删掉生产赋值 `2119`/`2155` 也无测试变红；建议补一条 stub 打印 `cache_read=42` 的 `run_one` 断言 `result.cache_read`。
3. **（低）计时敏感断言有假失败余量**：心跳测试用 10ms 间隔，`recv_timeout(1s)` 返回后到 `stop_tx.send` 之间若被抢占 >10ms，会多出一条已入 channel 的 emit 使 `recv_timeout(30ms).is_err()` 翻车（`src/dispatch.rs:3042-3070`）；`idle < 500` 同理（`src/process.rs:655`）。建议把这些断言改成「不再新增（允许缓冲差一条）」或把间隔放大。
4. **（低）契约与实现已不一致**：`TASK-07-idle-and-tail.md:11` 仍写「toolCalls 从 `session/update` 的 tool_call 通知累计」和「连发 3 次同名 → `repeatedToolCalls == 3`」，而 R1 后权限请求也计数、对应测试与假 server 现为 4（`dispatch.rs:3102-3116`）；契约没同步，07B/后续期只看契约会误判阈值来源。
5. **（低）旧前端把新 status 原样显示**：`DispatchOverlay.tsx:553` 直接渲染 `statuses[nodeId]`，07A 单独合到 main 时运行中行会短暂显示英文 `running`（契约的「旧前端忽略未知 status」只保证不崩，不保证不显示）。
6. **（低）`schema` 缺键被拒与 `serde(default)` 口径相抵**：`src/dispatch.rs:412` 使无 `schema` 行的旧留痕从对账/回滚查找中消失，而容器级 `#[serde(default)]` 声称「全部字段可缺省」；本仓留痕都带 schema，影响面小，建议注释点明或对缺键放宽。
7. **（低）第 5 条后半只到单元粒度**：`r1_committed_snapshot_cleanup_failure_returns_note_not_error`（`3716-3721`）直接调 `finish_before_snapshot`，「persist 不返回 Err 且 `before_dir` 不落空」这半句仍靠读码。