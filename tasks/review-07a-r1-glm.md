# TASK-07A 修正轮 R1 复审

先说限制：本会话没有 shell 工具，无法跑 `git diff` 逐行核对改动范围；「acp.rs 仍限计数」一项基于全文阅读并对照 DS R0 复审的逐行基线描述。其余结论均来自对源码与测试的逐条核对。

## 逐条核对（10 条）

1. **心跳 Disconnected 即退出 — 已核实修好**。`src/dispatch.rs:2612-2622` 只有 `Timeout` 才 emit，`Ok(())`/`Disconnected` 都 `break`；调度处 `src/dispatch.rs:744-747` 先 `stop_tx.send` + `heartbeat.join()`，终态事件在 `receiver.recv()` 之后才发（`src/dispatch.rs:771`），worker panic 时 `stop_tx` 随栈展开 drop → 线程自退出。三条测试（`src/dispatch.rs:3039/3052/3065`）分别钉住正常停、drop 停不emit、终态无心跳字段且无晚发。

2. **lastTool 脱敏 + 截断 — 已核实修好**。`src/dispatch.rs:2626` 先 `safe_trace` 再 `chars().take(120)`（字符边界安全）；`src/dispatch.rs:3085`（Bearer → `[REDACTED]`）、`3094`（121 个「工」→ 恰 120 字符）两条测试判别力明确。

3. **工具计数按尝试归零 — 已核实修好**。`src/process.rs:59-66` 重置 `started`+`tools`、保留 `last_output`；`src/dispatch.rs:1768`（每次 attempt 开头）与 `:1927`（ACP→一次性回退前）各调一次；`elapsedMs` 与留痕 `duration_ms` 同在 attempt 内起算（`src/dispatch.rs:1770-1771`、`:2082`）。测试 `src/process.rs:659`（elapsed<100 且 idle≥20，证明 last_output 未被伪造）、`src/dispatch.rs:3142`（备选不继承主计数）、`:3123`（ACP 回退后归零，不重置则会是 4，判别力足）；契约句在 `tasks/TASK-07-idle-and-tail.md:7`。

4. **权限请求计数 + sessionId 过滤 — 已核实修好**。`src/channels/acp.rs:503-514` 权限分支先按 `params.sessionId` 匹配当前 session 再计数，带 title 走 `record_tool_call`、无 title 走 `record_untitled_tool_call`；`session/update` 侧 `src/channels/acp.rs:479-493` 同样过滤。测试 `src/channels/acp.rs:805/814/823` 分别钉住「只发权限→计 1」「other session 不计」「无 title 仍计 2」，`fixtures/acp/fake_server.py:62-70` 的三个旗标精确支撑。

5. **`.prev` 失败分支 — 已核实修好**。`src/dispatch.rs:1121-1123` root 是否目录的校验在删 `.prev`（`:1124-1126`）之前；写成功后清理失败只经 `finish_before_snapshot`（`:1089-1100`）降级为 note，persist 仍返回 Ok 且 `before_dir` 有效，note 进 runner_note（`:2040`）。测试 `src/dispatch.rs:3701`（root 是文件时 `.prev` 原样保留）、`:3716`（清理失败→note、无 prev→空串）。

6. **测试判别力 — 已核实修好**。`src/process.rs:631`（先输出一行→静默 2.1s，idle≥2000）与 `:645`（第二次输出后 idle<500）——把 R0「删掉 `mark_output` 照样绿」的空洞钉死；cacheRead 两条链 `src/dispatch.rs:3661`（parse_output）与 `:3672`（对账从真实序列化留痕回填）；零改动 E2E `src/dispatch.rs:3724` 走两次 `run_one`，第二条 `before_dir: ''` 且 `rollback_node_changes_core("run-b")` 经 `latest_before_dir` 恢复第一次的旧值——正是 R0 指出的「推理而非测试」那条链。

7. **留痕字段名边界 — 已核实修好**。`src/dispatch.rs:412-414` `parse_record_file` 校验 `schema == "aicross-dispatch/1"`；`src/dispatch.rs:2999`（未知键忽略+缺键默认）、`:3011`（schema 不匹配报错）。

8. **契约补句 — 已核实修好**。`tasks/TASK-07-idle-and-tail.md:7` 同时含「本次尝试」口径与「终态事件不带心跳字段，前端保留上一条 `running` 的 `elapsedMs`/`lastTool`」。

9. **（GLM 2.3/2.4）— 已核实修好**，已并入第 4 条。细节差异：fix 条目写「`lastTool` 置空」，实现是「保持不变」（`src/process.rs:52-57` 不碰 `last_tool`/`repeated`）——这与 GLM 2.4 的原始建议（`last_tool` 保持不变）一致，测试场景（从未出现 title）两种读法都满足，仅措辞不一致，不算改法问题。

10. **timeout 放宽 — 已核实修好**。`src/dispatch.rs:3111` 与 `:3132` 显式 `timeout_secs = 10`（1s 只留给 hang 类测试）；「10 秒以上」按含本数理解成立。acp.rs 单元层测试维持 5s deadline，与全部既有 ACP 测试同款，GLM 条目本就只点名 dispatch 侧。

## 特别核对

- **终态之后不再发 running**：`emit_running_progress` 唯一调用点是心跳闭包（`src/dispatch.rs:742`），心跳线程在终态 emit 前已 join；终态载荷五个心跳字段全 `None`（`skip_serializing_if` 不序列化，`src/dispatch.rs:293-302`）。✓
- **第 2–6 期无回归**：`run_one_attempt`/`run_fallback` 除 `reset_attempt` 两行与快照 note 降级外结构未变，写锁、worktree 回退、验证、备选路由、session 重置、对账、回滚语义一致；快照改动被既有 task06/task07 快照测试群（`src/dispatch.rs:3624-3765`）+ R1 新测试覆盖；ACP→一次性回退既有测试 `src/dispatch.rs:3289` 未破坏。✓
- **acp.rs 限改计数**：`answer_permission`/`is_read_request`/`configure_session`/超时取消判定与 DS R0 描述逐字一致，新增行为仅 mark_output、带 sessionId 过滤的 tool_call 计数与 activity 透传（基于全文阅读，非 git diff）。✓
- 编排者复跑 cargo 280 / vitest 254 / build 零 warning 及 live 真跑，按题面与 REPORT R1 节采信。

## 结论

**R1 之后 TASK-07A 可交付**——10 条全部闭合，无阻塞项。

仍未闭合的项（均为低危，不阻交付，按严重度排序）：

1. （低）`r1_heartbeat_normal_stop/disconnected`（`src/dispatch.rs:3042/3055`）存在 ~10ms 调度竞态：主线程若在首个 emit 后超过 10ms 才发出 stop，worker 会多 emit 一次导致断言失败——极端负载下的低概率 flake，可留到下轮顺手把间隔放宽观察窗口。
2. （低）`record_untitled_tool_call`（`src/process.rs:52`）不清 `repeated`/`last_tool`：titled→untitled→同名 titled 序列会把中间无名调用也计进「连续」，`repeatedToolCalls` 至多偏大 1，对 G2 阈值 ≥3 影响可忽略。
3. （极低）`fix-07a-r1.md` 第 9 条「`lastTool` 置空」与实现（保持不变）措辞不一致，契约未定义「先有 title 后有无 title」组合的行为，建议在契约里补一句或改 fix 记录。
4. （备忘）缺 `schema` 字段的更老留痕现在会被 `parse_record_file` 拒绝（fail-closed，符合第 7 条意图）；留痕均由本应用写出，实际不可达。