# TASK-05A 复审 R1（DeepSeek deepseek-v4-flash via pi，只读，2026-09-14）

## R1 复审（19 条逐条）

前提下的事实：`src/channels/acp.rs` 全文读过；`dispatch.rs` 相关段（1198–1571、2380–2545）读过；`process.rs`/`terminal.rs`/`server.rs`/`SettingsModal.tsx`/`dispatch.ts`/`types.ts`/`DispatchOverlay.tsx`/`tauri.ts`/`fake_server.py`/`orchestration.rs:71-101` 读过。编排者给的 cargo 188 / vitest 127 / build 与报告一致。

| # | 结论 | 依据 + 理由 |
|---|---|---|
| 1 | 已核实修好 | `dispatch.rs:1355-1357` Fallback 调 `reset_acp_fallback(&prompt)`（`1569-1571` 返回空 session + 完整 prompt），一次性调用处 `1380` 用 `oneshot_session`，空即不传 id → `channels/mod.rs:230-231` 的 `-S` 不会出现；测试 `dispatch.rs:2492-2512` 断言无 `-S/-r` 且 argv 含全文。残留：设置里 ACP 关掉后，图里持久化的**旧 ACP session id** 仍会走 `-S`（`is_continuation` 链路未区分 id 来源），所以「关开关 → 下一轮」仍可能报错。 |
| 2 | 已核实修好 | `acp.rs:382` `wanted = is_read_request(...) || (writable && !blind)`，只在 options 里找 `allow_once`/`reject_once`，找不到回 `{"outcome":"cancelled"}`，**不可能选 allow_always**（`387-400`）；策略语写入留痕 `dispatch.rs:1344-1348`。残留：`is_read_request`（`404`）只按 kind/title token 判，`kind=execute` 但 title 含 read/list 的写类请求会被放行——这是安全方向的漏放。 |
| 3 | 已核实修好 | `process.rs:86` `assign_process_to_job` → `terminal.rs:102-107` → `windows_job::assign_pid`；`server.rs:4396-4397` ExitRequested 调 `shutdown_all`。 |
| 4 | 已核实修好 | 错误分 6 类（`acp.rs:14-32`），只有 `Process|Timeout` 摘池（`148-153`），Turn/ForeignSession/Protocol 保池；非 JSON 行 `continue` 且已进 raw（`331-334`）；未知带 id 请求回 `-32601`（`343-348`）；`session/update` 按 `params.sessionId` 比对（`336-342`）。 |
| 5 | 已核实修好 | 等锁 `try_lock` + deadline + 取消复查、拿到锁后再查一次（`acp.rs:118-135`）；取消发 `session/cancel`（`310-315`）且 ACP 进程从不进 `running_processes` 注册表，`cancel_dispatch_node` 的 taskkill 打不到它。**但随附测试是同义反复**（见 B 节）。 |
| 6 | 已核实修好 | 池键含 `cwd.display()` 与 `blind`（`acp.rs:92-99`），进程 cwd = 节点 workdir（`start_kimi(cwd)` → `spawn_interactive(.., Some(cwd))`）。REPORT 自认「隔离键由构造逻辑直接覆盖」——无断言。 |
| 7 | 已核实修好（1 处疑误） | usage 取 `/result/usage` → input/output 正确（`276-288`，`fake_server` 断言 11/7）；`set_model` 后强制 `availableModels` 含该 id，否则 Protocol 错误回退（`232-246`）。疑误：`282` 读的是 `cacheReadTokens`，ACP 规范字段应为 `cachedReadTokens` → 真机上 cache_read 会恒 0（建议双键回退），无测试覆盖。 |
| 8 | **改法有问题** | 每轮 `take_stderr_text()` 清零（`289`）✓、stderr 读满继续排空 ✓（`process.rs:107-126`）、旧留痕默认 oneshot ✓（`dispatch.rs:244`，测试 `2424-2429`）、`kimi 不报用量` 移进 `runner_note` ✓（`1733-1746`）。但 `append_bounded`（`424-437`）在 8MB 分支执行 `raw.truncate(MAX_RAW-20)`：`String::truncate` 非字符边界会 **panic**，而这里正是按字符截断后刚好凑满字节数的场景（CJK 内容约 1/3 概率命中）→ 派发线程 panic、`clear_cancelled` 不执行。另：出错轮不取 stderr，残余会挂到下一轮 `raw_stderr`。 |
| 9 | 已核实修好 | `types.ts:22` `runner?`、`dispatch.ts:177` 写入 Reply、`DispatchOverlay.tsx:421` 结果卡 + `448` 用量表显示「长驻/一次性」；Rust 镜像 `orchestration.rs:87-88` `#[serde(default)] runner: Option<String>`（这是随图落盘的那份 Reply）。G1 的 `route-line`（`323`/`408`）没显示（派发前不知道 runner，可接受）；本地合成的失败结果硬编码 `'oneshot'`（`DispatchOverlay.tsx:218`）会把没跑过的节点显示成一次性。 |
| 10 | **改法有问题** | ② 一次性回归确实改成了 python stub（`dispatch.rs:2432-2477`，断言 `stub answer` 前缀 + `runner: oneshot` + frontmatter），不会再真发请求 ✓；③ `runner=acp` 有前端断言（`dispatch.test.ts:152-159`）✓；① 的假 server 覆盖了两轮复用、banner 容忍、拒写、未知请求、sessionId 过滤 ✓（`acp.rs:558-598`）。**但「杀进程后回退且留痕 runner: oneshot」的端到端没交付**：只有合成 Err 的纯函数断言（`dispatch.rs:2480-2489`）。 |
| 11 | 已核实修好 | `SettingsModal.tsx:541` 只显示二进制名 +「需自行安装」，全仓无未核实的 npm 包名。 |
| 12 | 已核实修好 | REPORT「修正轮 R1」首段写死口径。 |
| 13 | **改法有问题** | 见 B 节：`acp.rs:325-327` 把「最后一轮轮询（remaining ≤100ms）的超时」归成 `Process`，而**静默超时必然落在这一支** → `permits_fallback()` 为真 → 仍然回退一次性。新增测试只喂合成 `AcpErrorKind::Timeout`，测不到真实路径。 |
| 14 | 已核实修好 | `acp.rs:222` 用 `self.sessions` 记当前模型，续聊换 model 会重新 `set_model` + 校验（`232-247`）。REPORT 说「假 server 两轮测试覆盖」不准：两轮同 model，换模型那一支无测试。 |
| 15 | 已核实修好 | `acp.rs:177` initialize 用 `timeout.min(30s)`，其中 timeout = 节点剩余 deadline（`103-106`）。 |
| 16 | 已核实修好 | `probe("kimi", super::kimi_fallback(), ...)`（`acp.rs:504`）；失败时既 `setAcpAdapters([])` 也 `setAcpProbeError` 且显示「探测失败」+「重新探测」按钮（`SettingsModal.tsx:174,545-547`）——不是静默吞掉，但与「不吞成空数组」字面有差。 |
| 17 | 已核实修好 | `acp.rs:451-464` `shutdown_all` + `release_acp_pool`；`server.rs:4244` 注册、`4397` 退出调用、`SettingsModal.tsx:208` 关开关调用、`acp.rs:660` 真机测试末尾调用。 |
| 18 | 已核实修好 | `SettingsModal.tsx:548` 文案写明只读/盲拒写、读类放行、一次性 kimi 无此限制。 |
| 19 | 已核实修好 | `AcpOutput.channel: ChannelOutput`（`acp.rs:40-44`），`run_one` 一句 `channel_output = output.channel`（`1339`）。 |

## B. 特别核对项

**第 1 条**：丢弃 session id ✓（`reset_acp_fallback` 返回 `("", full_prompt)`），完整 prompt ✓（用的是 `prepare_outbound` 的 `prompt`，不是 `outbound` delta），argv 不含 `-S/-r` ✓ 有测试。唯一缺陷是测试只把两个 helper 拼起来验，没驱动 `run_one`（`dispatch.rs:1355` 若被改回 `node.session_id`，测试仍绿）。

**第 2 条**：读/写分类 ✓、只读+盲拒写 ✓、永不 `allow_always` ✓、策略进留痕 ✓。缺一支测试：`fake_server` 只发 `kind=execute`，没有「只读节点收到读类请求 → 放行」的断言（只有纯函数 `permission_policy_classifies_read_and_write`）。

**第 3/17 条**：`spawn_interactive` 进 Job Object ✓（`process.rs:86`）；`ExitRequested` 清池 ✓（`server.rs:4396`）；关开关清池 ✓（`SettingsModal.tsx:208` → `release_acp_pool`）。

**第 4/5 条**：轮级错误保池 ✓；非 JSON 行忽略并进 raw ✓；未知 agent→client 请求回 `-32601`（带 id 才回，通知不回，符合 JSON-RPC）✓；`sessionId` 过滤 ✓；等锁节点可取消 + 拿锁后复查 ✓；共享进程发 `session/cancel` 不 taskkill ✓。**第 5 条的「补测试」是空的**：`queued_acp_node_observes_cancel_marker_before_acquiring_process`（`dispatch.rs:2539-2543`）只断言 `cancel_dispatch_node` 与 `is_cancelled` 两个既有函数，完全没进 `acp.rs:118-135` 的等锁分支——把 R1 的 try-lock/deadline/复查整段回退，测试依旧全绿。

**第 10 条**：「一次性回归」不可能再真发请求（agent=cc-switch + `cc_switch_path` 指向 python stub，`channels/mod.rs:292-299` 直接以 stub 为 program，测试断言 stub 输出前缀）✓；`runner: acp` 有断言（前端）✓；假 server 不支持 `session/cancel` 是唯一覆盖缺口。缺 10① 的第四小项（杀进程 → 回退 → 留痕 `runner: oneshot` 端到端）。

**第 13 条（本轮最实质的缺陷）**：`acp.rs:322-327`
```rust
let remaining = deadline.saturating_duration_since(Instant::now());
if remaining.is_zero() { return Err(Timeout) }        // 几乎打不到
let line = match self.process.recv_line(remaining.min(100ms)) {
    Err(e) if e.contains("超时") && remaining > 100ms => continue,   // 只有还剩 >100ms 才继续轮询
    Err(e) => return Err(AcpError::new(Process, e)),                  // ← 静默超时走这里
};
```
静默的 agent（无任何 stdout 行）在 deadline 前的最后一次 `recv_line` 必然是 `remaining ≤ 100ms`，于是「超时」被判成 `Process`；`permits_fallback()` 对 `Process` 为真 → `run_one` 仍然跑一次完整一次性（最坏 2×timeout；写节点重复写）。只有「恰好有行在最后一窗口到达、下一轮循环 top 处 `remaining == 0`」才判 `Timeout`，也就是说行为取决于 agent 是否恰好吐过一行。建议按 `RecvTimeoutError` 类型（而不是 `e.contains("超时")` 字符串）分流：仍剩时间 → continue；已到 deadline → `Timeout`。配套测试应喂「无输出直到 deadline」的真进程，而不是合成枚举。

**新引入缺陷扫描**：
- 与 3/4 期一次性路径：无回归 ✓（`oneshot_session/outbound` 只在 Fallback 分支被改写，其他路径取值与旧代码相同；`DispatchSettings.acp_enabled` 有 default，旧设置仍走一次性）。
- 留痕只追加 ✓：`aicross-dispatch/1` 名未改，只加 `runner`/`runner_note`（`dispatch.rs:244-247`），旧记录可读有测试。
- Reply 只追加 runner ✓；`server.rs:4243-4244` 注册、`tauri.ts:375-377,397,400-405,476-477` 类型镜像齐全 ✓。
- 新风险：① `shutdown_all`（`acp.rs:451-460`）持 map 锁 + 阻塞式 `shared.lock()`——有在飞轮次时会把退出/关开关卡到该轮 deadline（默认 600s，上限 3600s），期间所有新 ACP 查找也被 map 锁挡住；建议 `try_lock` 跳过或用有界等待。② 取消后的迟到 `session/update`（同一 sessionId）会被拼进**下一轮**正文（`request` 只丢 id 不匹配的响应，不丢 update）。③ `spawn_interactive` 的 stdout `mpsc` 仍无上限（`process.rs:96-103`），池常驻 + 轮间不排空 → 失控 agent 可无界堆积（R1 只封了 raw 字符串）。④ `fake_client()` 用 `resolve_binary("python", None).unwrap()`：没有 python 的机器上这 4 条测试**硬失败**而非跳过；`shutdown_terminates_fake_acp_process` 用 `tasklist` 输出 + `contains(pid)`（pid 可能是别的 pid 的子串 → 假失败）且靠 100ms sleep 竞态。

**会导致误判的断言**（按危险度）：
1. `acp_timeout_is_terminal_and_never_selects_oneshot_fallback`（`dispatch.rs:2515-2524`）——绿着掩盖第 13 条的真实行为；同类的 `killed_acp_process_selects_oneshot_fallback`、`cancelled_acp_process_does_not_restart_as_oneshot` 也只是合成 Err。
2. `queued_acp_node_observes_cancel_marker_before_acquiring_process`——名字指向 R1 新逻辑，实际什么都没测。
3. `foreign_acp_session_fallback_drops_resume_flag_and_uses_full_context`——把 `reset_acp_fallback` + `build_invocation` 再拼一遍，不是 `run_one` 的真实接线（REPORT 也引作「已补测试」）。
4. 真机 ignored 测试（`acp.rs:630+`）的 `assert_eq!(second.session_id, first.session_id)` 与 `contains("青松")` 都太软（GLM 早指出是「同义反复」）；它靠 `.unwrap()` 不 panic 才间接证明 ACP 没报错——而**这一次真机跑发生在 R1 之前**，R1 新增的 `set_model` 校验（新建 session 时必定发送，且要求 `/result/models/availableModels`）从未在真机上验证过：若 kimi 的 `session/set_model` 响应是 `{}` 或路径不同，ACP 会整体静默退化为一次性（留痕只留一句 `runner_note`），而现有单测/前端测试全绿。这是当前最需要一次 26s 复跑闭掉的口子（同时能顺便验 `cachedReadTokens`）。

## C. R1 之后 TASK-05A 是否可交付

**未达到可交付**：R1 的 17 条实现改动方向正确、契约与类型镜像干净，但第 13 条（静默超时仍回退、写节点可能重复执行）是真实缺陷且随附测试测不到它，第 10① 的「杀进程→回退→留痕 oneshot」端到端未交付，且 R1 改动（尤其 `set_model` 校验）在真机上还没跑过一次。

仍未闭合项（按严重度）：

1. **【高】超时分类错误 → 超时仍回退一次性**（`acp.rs:325-327`）；修完后补「无输出直到 deadline 的真进程」测试（第 13 条）。
2. **【高】R1 后未真机复跑 ignored 测试**：`set_model` 必发 + `availableModels` 校验（`acp.rs:222-246`）未经真机确认，失败形态是「静默退化为一次性」；同批验 `cachedReadTokens`（第 7/14/15 条的真机面）。
3. **【中】`append_bounded` 截断分支可 panic**（非字符边界 `truncate`，`acp.rs:435`），8MB 级输出会打死派发线程（第 8 条）。
4. **【中】缺「杀 ACP 进程 → 回退 → 留痕 `runner: oneshot`」端到端测试**，现有相关断言全是合成 Err（第 10①）。
5. **【中】`shutdown_all` 阻塞等 in-flight 轮次**，退出/关开关可能卡到 600s（上限 3600s），且期间阻塞所有 ACP 查找（第 3/17 条硬化）。
6. **【中低】取消后迟到 `session/update` 污染下一轮正文；stdout mpsc 无上限跨轮堆积**（第 4/8 条硬化）。
7. **【中低】权限读类判定可被 title 里的 read/list 字样绕过**（`kind=execute` 也放行），且「只读节点放行读请求」无测试（第 2 条）。
8. **【低】ACP 关掉后旧 ACP session id 仍进 `-S`；本地合成失败结果硬编码 `runner=oneshot`**（第 1/9 条 UI/语义）。
9. **【低】`fake_client()` 依赖 PATH 有 python 且 `unwrap`；shutdown 测试用 `tasklist`+pid 子串+100ms 竞态**（测试健壮性）。
10. **【低】REPORT R1 的「已补测试」有 3 处名不副实**（第 5、7/14 条），需按实际断言改写留痕，避免下一轮误判。

---
usage: {"input": 754, "output": 4780, "cacheRead": 147328, "cacheWrite": 0, "reasoning": 313, "totalTokens": 152862, "cost": {"input": 0.00010556000000000001, "output": 0.0013384, "cacheRead": 0.0004125184, "cacheWrite": 0, "total": 0.0018564784}}
