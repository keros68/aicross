# TASK-07A 修正轮 R1（引擎：心跳/快照收口）

先读 `tasks/TASK-07-idle-and-tail.md`（契约 + A 节）、`tasks/REPORT-07A.md`，再读 `tasks/review-07a-ds.md` 与 `tasks/review-07a-glm.md`（若存在）。逐条处理，每条在 `tasks/REPORT-07A.md` 末尾「修正轮 R1」一节写「已修：怎么改的（文件:行）／已补：测试名」或「不采纳：原因」。验证：`cargo check` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`，命令与输出原文贴进报告。不跑 `cargo fmt`、保持文件原有换行（`server.rs` CRLF、其余 Rust LF）；`src/channels/acp.rs` 只允许改工具计数相关的行；不起窗口、不发真机请求、不 commit、不 push。

## 必修（按严重度）

1. **心跳线程 `Disconnected` 即退出**（DS L1）：`recv_timeout` 只有 `Timeout` 才发 `running`，`Disconnected` 直接 `break`；把循环体抽成 `heartbeat_loop(stop_rx, interval, emit)` 可注入间隔与计数闭包。补测试：正常停发一次即停；`stop_tx` 被 drop（模拟 panic）后线程退出且不再 emit；终态之后不再有 running。
2. **`lastTool` 脱敏 + 截断**（DS S1）：`running_progress` 里 `last_tool` 过 `safe_trace`/`redact_text` 并截到 120 字符（按字符边界）。补测试：标题含 `Authorization: Bearer …` 时事件里被遮盖，超长被截。
3. **工具计数按尝试归零**（DS L2）：`run_one_attempt` 开头重置 `ToolActivity`（保留 `started`/`last_output`），主/备与 ACP→一次性回退不跨尝试累加；`elapsedMs` 与留痕 `duration_ms` 同口径（按本次 attempt 起算）。契约文件补一句「按尝试计」。补测试。
4. **权限请求里的 toolCall 也计数**（DS L3）：`session/request_permission` 分支同样 `record_tool_call(title)`（kimi 0.42 真机只在权限请求里带 title）。补测试：假 server 只发权限请求时 `toolCalls` 仍递增。
5. **`.prev` 失败分支**（DS L4）：`is_dir` 校验提前到删除 `.prev` 之前；写成功后清理 `.prev` 失败只追加 note，不让整次 persist 返回 Err、不让 `before_dir` 落空。补测试两条。
6. **测试判别力**（DS T2、T3、补充）：一次性 idleMs 测试改「先输出一行 → 静默 2s」断言 `idle ≥ 2000`，加「两次输出」变体断言 `idle < 500`；cacheRead 走 `parse_output` 与对账回填两条链路断言；零改动重派后 `rollback_node_changes_core` 仍按 `latest_before_dir` 命中旧快照的 `run_one` E2E。
7. **留痕字段名边界测试**（DS Q1）：补「未知键忽略、缺键取默认、`schema` 值不匹配报错」的测试；`parse_record_file` 对 `schema` 做值校验。
8. **契约补一句**（DS §1）：终态事件不带心跳字段，前端保留上一条 running 的 elapsed/lastTool（写进 `tasks/TASK-07-idle-and-tail.md` 契约节，07B 已按此实现）。

## GLM 补充

GLM（`tasks/review-07a-glm.md`）与 DeepSeek 在第 1–5 条一致。GLM 额外的：

9. **tool_call 计数按 sessionId 过滤**（GLM 2.3/2.4）：只计当前 session 的 `tool_call` 通知；无 title 的也计入 `toolCalls`（`lastTool` 置空），不要漏计。补测试。
10. **新 ACP 测试的 1s 超时易 flake**（GLM 5.1）：`task07_fake_acp_three_same_tool_calls_*` 的 timeout 放宽到 10s 以上（假 server 本地起，实际耗时不受影响）。
