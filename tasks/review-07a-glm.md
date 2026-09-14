# TASK-07A 审查

以下发现基于通读五个改动文件全文（dispatch.rs 4984 行、process.rs、acp.rs、orchestration.rs、fake_server.py），报告引用的全部行号已逐一核对属实。本会话无 shell 工具，未能重跑 `cargo test`，报告中的验证输出只能读码佐证（`run_one` 加了 `#[cfg(test)]`（`src/dispatch.rs:1608`），cargo check 零 warning 的声明与代码状态一致）。

## 1. 需求完整性

7 条逐条核对均有实现与对应测试，报告与代码一致。契约细节：

- `DispatchProgress`（`src/dispatch.rs:287-303`）：camelCase ✓、Option + `skip_serializing_if` 可选 ✓、终态事件字段省略 ✓。
- 心跳 15s 一次 ✓（`src/dispatch.rs:739` `recv_timeout(15s)` 循环）；终态前 `heartbeat.join()` 先于 `tx.send`，running 不会晚于终态 ✓（常规路径）。
- **发现 1.1**：`src/dispatch.rs:738-745` — 「节点结束/取消后停止」对 panic 分支不成立且行为更糟，见 2.1。需求原文写「心跳线程在节点结束/取消后停止」，panic 属于未覆盖的结束方式。
- **发现 1.2**：A 节要求的两条指定测试都存在且断言正确，但「心跳」本身（线程、间隔、停止）零测试覆盖——三个 task07 测试全是纯函数/直接构造 payload。见 5。

## 2. 逻辑正确性

- **2.1（中）** `src/dispatch.rs:739` — `while stop_rx.recv_timeout(15s).is_err()`：worker 线程 panic 时 unwind 会 drop `stop_tx`，此后 `recv_timeout` 对已断开通道**立即**返回 `Err(Disconnected)`，`is_err()` 仍为 true → 心跳线程变成无间隔忙等死循环，持续 emit `status="running"` 事件刷屏。为什么：`Disconnected` 和 `Timeout` 被同等对待。建议改 `loop { match stop_rx.recv_timeout(15s) { Err(Timeout) => emit, _ => break } }`。
- **2.2（中）** `src/dispatch.rs:1659,1714` — 主、备两次尝试共享同一个 `activity`：`toolCalls`/`repeatedToolCalls` 跨主备累计，「连续同名」的连续性跨 agent 延续（主以 "Read file" 结束、备以 "Read file" 开始即计为连续），idleMs 也从主尝试最后输出起算。为什么：B 节 G2 阈值 `repeatedToolCalls ≥ 3` 会把「主失败→备重跑」误报为「同一工具连转 3 次」，而主备本就是两次独立运行。建议：进入 `run_fallback` 时重置工具计数（给 `ProcessActivity` 加 `reset_tools()`），或备选新建 activity、elapsed 基准保持不变。
- **2.3（中）** `src/channels/acp.rs:479-486` — tool_call 计数不校验 `params.sessionId`，而同函数里正文拼接（487-492）是校验的。共享 ACP 进程上其它 session 的迟到 tool_call 通知会计入当前节点（fake_server 自己就在每轮发一条 "other" session 的通知，只是恰好不是 tool_call）。建议加与正文一致的 sessionId 判断。
- **2.4（低）** `src/channels/acp.rs:482` — 无 `title` 的 tool_call 完全不计数。ACP 规范 title 可缺（同文件 `is_read_request` 已处理过 kimi 真机缺字段的形状），此时 `toolCalls` 低估、`repeatedToolCalls` 漏判。建议无 title 时 `calls += 1`、`last_tool` 保持不变。
- **2.5（低）** `src/dispatch.rs:1100-1108` — 先删 stale `.prev`（1101-1103）再检查旧 `root` 是否目录（1106）。若 root 是文件，报错时 prev 已被删。顺序对调（先验 root 再清 prev）更稳，影响极小。
- **2.6（低）** `src/dispatch.rs:1145-1147` — 新快照写成功后删 `.prev` 失败 → 整体返回 Err → 调用方记「保存快照失败」且 `before_dir` 置空，但新快照实际有效且恰好位于回滚默认路径；此后 `latest_before_dir` 会按**其它** record 的 before_dir 命中旧快照，两目录并存语义混乱。建议 prev 清理失败降级为 runner_note，不推翻写成功的事实。
- **2.7（低，确认项回答）** `src/dispatch.rs:1168,1173` — fail-closed 实现正确（canonicalize 失败即 `continue`）。误拒面：合法运行目录模式（workdir == project）只有在 **project 本身** canonicalize 失败时才被误拒（1168 的 `unwrap_or_else` 用原始路径兜底，与 1173 解析出的真实路径不等）——方向保守，可接受，建议注释点明。worktree 记录（workdir ≠ project）被拒是设计内：rollback_before 只对 single-writer 生效，其 workdir 就是 project。
- 零改动判定 ✓：删除文件在 `changed_snapshot_paths` 中 before 有/after 无 → 计入改动（`src/dispatch.rs:1053-1073`），端到端有恢复被删文件的测试（`src/dispatch.rs:3435`）。>8MB 文件按 size+mtime 判等是既有口径，非本期引入。
- 结构级 `#[serde(default)]` 保住 runner oneshot ✓：字段级 `default = "oneshot"` 优先于结构级默认，`task07_dispatch_record_all_legacy_fields_default_instead_of_unknown` 钉住（`src/dispatch.rs:2941`）。
- idleMs 语义符合定义：ACP 只在 `session/prompt` 循环传 activity（`acp.rs:301-310`，session/new/config 不计）= 按最后通知计；oneshot 由 `bounded_reader` 两个读线程 mark（`src/process.rs:462-477`）。prepare/快照/验证阶段无子进程输出也计入 idle——与「距上一条输出」定义一致，靠 B 节阈值消化。

## 3. 安全

- 心跳载荷仅 run_id/node_id/status/时延/计数/工具标题，无正文、无密钥 ✓。
- **3.1（低）** `src/dispatch.rs:2580` + `src/channels/acp.rs:483` — `lastTool` 是 agent CLI 通知里的模型可控文本，未经 `redact_text`/`safe_trace` 也未限长，直达前端。不落盘、本地事件，危害低，但与「留痕与错误文本一律先安全过滤」的既有口径不一致。建议过 `safe_trace` 并截断（如 200 字符）。
- 回滚与快照路径校验未被削弱 ✓：`persist_before_snapshot` 仍走 `safe_relative_path`（`src/dispatch.rs:1124`），rollback 仍走 safe_relative_path + parent canonicalize starts_with + symlink 替换（`src/dispatch.rs:1216-1231`）。

## 4. 代码质量与契约

- 只追加 ✓；`acp.rs` 改动限于 activity 透传、逐行 mark、tool_call 计数与测试，协议决策未动 ✓；run_fallback 加 `&activity` 参数是必要最小改动 ✓。
- **4.1（低）** `src/dispatch.rs:738-745` — 心跳线程 8 行内联在调度循环里，15s 硬编码，无法在测试中缩短或观测停止时机（也是 5.1 缺测试的根因）。建议抽 `fn heartbeat_loop(app, run_id, node_id, activity, stop_rx, interval)`，测试传 50ms。

## 5. 测试

报告所列测试全部真实存在，断言目标行为（我逐一读过测试体）。flake 评估：`oneshot_silent_two_seconds` 不易 flake（子进程零输出 → idle ≥ 总耗时，python 启动只会加大余量）；8MB 测试的同步点（`src/process.rs:603-607`）把竞态改成 10s deadline 的确定性等待，成立。

- **5.1（中）** `src/dispatch.rs:3046` — fixture 固定 `timeout_secs: 1`，被新测试 `task07_fake_acp_three_same_tool_calls_reach_running_event_payload`（happy path）使用。慢机上 initialize + session/new + 3 次配置 + prompt 超 1s 即误报 timeout，`assert status=="ok"` 翻车。该 fixture 的 1s 是为 hang 测试设计的。建议此测试改用更宽 timeout。
- 最该补的 3 个：
  1. **心跳生命周期**（配合 4.1 抽函数）：50ms interval + 快速 stub，断言运行期收到 ≥2 条 running、终态后 100ms 内无新 running——同时覆盖 2.1 的 Disconnected 分支。
  2. **跨 session tool_call 隔离**：fake_server 加旗标对其它 sessionId 发 tool_call，断言不计数（当前实现会失败，正好钉住 2.3）。
  3. **零改动重派的集成路径**：run_one 跑两次、第二次 stub 不改文件，断言第二条留痕 `before_dir` 为空且 rollback 仍恢复第一次快照——现有测试只测了 `persist_before_snapshot` 纯函数，没测 record 与回滚查找的组合行为。

---

## 对照 A 节是否完整实现

**一句话：A 节 7 条全部交付，指定测试齐全、报告行号与验证输出与代码相符，属可合并状态；但心跳线程的 panic 分支存在忙等死循环缺陷，且主备共享 activity 会让 repeatedToolCalls 跨尝试误报，建议合并前修 2.1/2.2。**

缺项（按严重度排序）：
1. 心跳线程遇 worker panic 变无间隔忙等刷事件（`dispatch.rs:739`，2.1）
2. 主/备共享 activity，toolCalls/repeated/idle 跨尝试累计，污染 G2 判据（2.2）
3. tool_call 计数不按 sessionId 过滤、无 title 不计数（2.3/2.4）
4. lastTool 未过滤未限长（3.1）
5. 心跳 15s/生命周期不可测无测试；新 ACP 测试 1s 超时易 flake（4.1/5.1）
6. `.prev` 两处失败分支顺序与报错语义小疵（2.5/2.6）