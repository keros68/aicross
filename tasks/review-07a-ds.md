静态复核（本会话无 shell：只读文件，未复跑 cargo/vitest，报告的构建与测试输出按原样采信）。已读：`tasks/TASK-07-idle-and-tail.md`、`AGENTS.md`、`tasks/REPORT-07A.md`、`src/dispatch.rs`（相关段 + 全部新增测试）、`src/process.rs`、`src/channels/acp.rs`、`src/orchestration.rs`、`fixtures/acp/fake_server.py`，并对照 `tasks/review-06a-r1-ds.md` 的原始缺口。

## 1. 需求完整性（A 节 7 条 + 契约）

- 第 1 条：`DispatchProgress` 新字段在 `dispatch.rs:289-303`，`rename_all="camelCase"` + `Option` + `skip_serializing_if` → camelCase 可选 ✔；`status:"running"` 在 `2575-2582` ✔；周期硬编码 15s 在 `739` ✔；正常路径终态在后（`744` join → `745` join → 父循环 `770` 发终态）✔。唯一未兑现点：心跳线程的间隔/停发/异常路径无任何测试（见 §5）。
- 第 2 条 ✔ `dispatch.rs:1100-1147`（`previous` 清理 → rename → 写 → 成功删 `.prev` / 失败改回）。第 3 条 ✔ `1090-1093` 早退，不建目录不碰 `.prev`。第 4 条 ✔ `1173` fail-closed。第 5 条 ✔ `305`（容器级 `default`）+ `349`（`before_dir` 注释写明绝对路径例外）+ `626`（幂等 guard 注释）+ `1245`（note 带 skipped 条数）。第 6 条 ✔ Rust 镜像 `orchestration.rs:84`，`DispatchNodeResult.cacheRead`（`195`/`2118`）、`ReconcileUsage.cacheRead`（`266-271`/`484`）。第 7 条 ✔ 三个测试都真改了（`process.rs:598-618` 同步点、`dispatch.rs:3686` 的小上限断言、`3559` 走 `dispatch_stage_core`）。
- 契约「旧前端忽略未知字段与未知 status」：额外字段进载荷、`running` 是新值；TS 侧唯一监听点 `src-ui/.../DispatchOverlay.tsx:184-187` 只读 `runId/nodeId/status` 并按 runId 过滤，不炸 ✔。
- 未覆盖到契约表述的一点：终态事件（`2555-2567`）把 5 个心跳字段一律发 `None`，B 节的 `elapsed`/`lastTool` 只能靠前端缓存上一条 running。契约没写这个约定，建议在任务书/前端注释里钉死，否则 B 节很容易实现成终态清空。

## 2. 逻辑正确性

**L1（中）`src/dispatch.rs:739` — 心跳线程把 `Disconnected` 当 tick，panic 时死循环泄漏。**
`while stop_rx.recv_timeout(15s).is_err()` 对 `Timeout` 与 `Disconnected` 一视同仁。`run_one_with_activity`（`743`）一旦 panic（debug/dev 是 unwind），`stop_tx` 随栈展开销毁 → `recv_timeout` 立即返回 `Disconnected` → 循环条件恒真 → 每轮瞬间 emit 一次 `running`，永不退出，且 `heartbeat.join()` 永不执行、线程永久驻留。此时父循环 `receiver.recv()` 已报错返回，前端可能永远停在「运行中」。`Cargo.toml` 的 `[profile.release] panic="abort"` 让 release 逃过一劫，`tauri dev`/debug 不逃。
改法：`match stop_rx.recv_timeout(Duration::from_secs(15)) { Err(mpsc::RecvTimeoutError::Timeout) => emit_running_progress(...), _ => break }`。

**L2（低-中）`src/dispatch.rs:1620-1651、1672-1696` — 主/备与 ACP→一次性回退共用同一个 `ProcessActivity`，工具计数跨尝试累加。**
`run_one_with_activity` 把同一个 `activity` 传给主尝试，`run_fallback` 复用同一引用；`ProcessActivity` 只在创建时归零（`process.rs:29-36`）。后果：备选刚开始跑时 `repeatedToolCalls` 可能已 ≥3（主尝试 ACP 连续 3 次同名），B 节的「疑似空转」胶囊会在备选一启动就亮；`lastTool` 也可能是已失败的主动通道的标题。`elapsedMs`（activity 起算）与留痕 `duration_ms`（每次 attempt 起算）口径也不一致。
改法：在 `run_one_attempt` 开头重置 `ToolActivity`（保留 `started`/`last_output`），或在契约里明确写「跨尝试累加」并让 B 节据此调阈值。

**L3（低）`src/channels/acp.rs:480-485` — 只有 `session/update` 且 `sessionUpdate=="tool_call"` 被计入，权限请求里的 `toolCall` 不计。**
`session/request_permission`（`acp.rs:495` 起的分支）带 `params/toolCall.title`，是 kimi 真机上唯一稳定出现的工具信号（`is_read_request` 的注释明确说 kimi 0.42 的权限请求带 title、不带 kind）。若某通道只发权限请求或只发 `tool_call_update`，`toolCalls` 恒为 0，B 节的空转提示直接失效。补计数属于「只加计数」，不碰协议决策。
改法：在 `request_permission` 分支同样 `activity.record_tool_call(title)`（同一 activity 参数已在手边），或至少在任务书里记下这条不确定性。

**L4（低）`src/dispatch.rs:1101-1104、1138-1147` — `.prev` 的两个失败分支处理不完整。**
(a) 先 `remove_dir_all(previous)` 再校验 `root.is_dir()`（`1104-1107`）：root 是文件时直接 Err，刚删掉的 `.prev` 是仅存旧快照，白丢一份。(b) 快照已成功写完并落位后，`remove_dir_all(&previous)`（`1146`）失败会让整次 `persist_before_snapshot` 返回 Err → 调用方 `2003-2005` 把 `before_dir` 留成空串、note 写「保存派发前快照失败」。于是新快照存在但没被留痕引用；`write_project_dispatch=false` 时回滚的兜底路径（`1186` 固定拼 `project/.dispatch/...`）跟实际 `log_dir` 不同源，这次回滚就找不到它。
改法：`.prev` 清理失败只 append note（或返回 `Ok(path)` 并附带警告），不要连坐快照提交；`is_dir` 校验提到删除之前。

**L5（结论：无缺陷）`latest_before_dir` 的 fail-closed 不会误拒合法模式。** 项目模式 `workdir==project`、两侧同源 canonicalize；worktree/盲节点 `workdir≠project`，但回滚只在 `uses_single_writer` 时调用（`1706-1713`），且 worktree 建失败时 `workdir` 回落成 `project`（`1755-1763`）；项目被搬走后 canonicalize 失败会被拒，但同 run_id 的兜底路径仍在项目内可命中。正向路径有 `task06_rollback_before_runs_inside_run_one_before_redispatch`（两次不同 run_id）兜着，不是空转测试。残留 nit：`1170` 的 `project_canonical` 在 canonicalize 失败时回落原路径，这一侧仍是 fail-open（命令入口 `project_path()` 已保证 `is_dir`，实际不可达）。

**L6（无缺陷）零改动判定不误伤删除。** `changed_snapshot_paths`/`snapshot_file_unchanged`（`1046-1063`）对「before 有 / after 无」返回 false → 记改动 ✔。大文件（`bytes=None`）只比 `size + modified_ns`，同尺寸同时间戳的改动识别不到——旧逻辑同样如此，且本期早退只导致「不写快照」而非「覆盖掉旧快照」，比改前更安全。

**L7（无缺陷）结构级 `serde(default)` 保住了 runner 的 oneshot 默认。** 字段级 `#[serde(default = "oneshot")]`（`317`）优先于容器级，`2941` 与 `2934` 两条测试分别钉了「整条只剩 node」与「缺 runner」两种旧留痕。

## 3. 安全

**S1（中）`src/dispatch.rs:2575-2582`（配合 `src/process.rs:52-58`）— `lastTool` 是远端 agent 控制的原始标题，未脱敏、未截断就进事件载荷。**
留痕与结果里的 `raw_stdout`/`raw_stderr`/`error`/正文一律过 `safe_trace`（`2113-2119`、`2247-2249`），新字段绕过了这条既有约定。ACP 的 title 常见形态就是命令或路径（kimi 的 `Bash`/`Write` 类标题往往是整条命令），携带 token/`Authorization` 头并不罕见；长度也无上限，远端可以放大事件载荷。事件本身只在本地 webview，但会落到 DOM/日志。
改法：`running_progress` 里 `last_tool: Some(safe_trace(&last_tool).chars().take(120).collect())`（或截断 + 复用 `redact_text`）。

**S2（无缺陷）** 心跳载荷其余部分只有 `runId`/`nodeId`/`status`/四个数字，无正文、无密钥、无 workdir 路径（`2580`）；`toolCalls`/`repeatedToolCalls` 是计数。终态事件不带这些字段。

**S3（无缺陷）快照/回滚路径校验未被本期削弱。** 写路径仍过 `safe_relative_path`（`1119-1126`），rollback 的父链 canonicalize 校验（`1210-1216`）与符号链接先删后写（`1200-1208`）原样；`.prev` 与被替换目录同根，没引入新的可写路径，也没被 `latest_before_dir`（只认 `.md`，`1170`）或任何清理逻辑枚举。零改动早退在写路径之前，不涉及校验绕过。

## 4. 代码质量与契约

- **只追加 ✔**：`DispatchProgress` 加字段 + 新 status 值；`DispatchRecord` 只加容器属性（不改名、`serialize_frontmatter` 输出不变）；`Reply` 加一个带 `#[serde(default)]` 的可选字段；`acp::run`/`prompt_with_activity` 是 crate 内 API，唯一调用点 `dispatch.rs:1852`。
- **旧留痕可读 ✔**（`2941`、`2934`；`parse_record_file` 走同一结构体）。
- **第 2–6 期路径无回归（未发现）**：`run_one`/`run_one_attempt`/`run_fallback`（`1620-2150`）除 activity 透传与心跳包装外逐段未改；写锁、worktree、验证、备选路由、session 重置、对账（`440-490`）、回滚（`1185-1240`）语义一致。
- **`src/channels/acp.rs` 改动范围 ✔**：新增 `prompt_with_activity`（`242-260`）与 activity 参数透传（`309`、`430`、`467-485`），并把原 `prompt` 降级成测试 wrapper（`236-240`）；`answer_permission`（`377-410`）、mode/thinking（`383-395`）、超时/取消判定逐字未动，属「只加计数」。
- **Q1（低）`src/dispatch.rs:305` 的容器级 `#[serde(default)]` 顺带关掉了留痕解析的字段名校验**：今后 YAML 里字段名打错或版本改名会静默取默认值，而不是报错；`DispatchRecord` 没有 `deny_unknown_fields`，未知键本来也被忽略，所以退化是「更哑」而非「更错」。建议补一条「未知键仍被忽略、缺键取默认」的边界测试，或在 `parse_record_file` 里对已有的 `schema` 字段做值校验。

## 5. 测试

报告点名的 12 个测试**全部真实存在**，断言目标与题面一致（逐条核对：`dispatch.rs:2941/2949/2967/3444/3458/3473/3481/3493/3559/3686`、`orchestration.rs:1171`、`acp.rs:778`、`process.rs:598/606`）；`272 = 261 + 11` 自洽。

- **T1（最该补）心跳线程零覆盖。** `2949`/`2967` 都是直接调 `running_progress` 造载荷，`739-745` 的线程从没被执行过：15s 节奏、停发、终态之后不再发 running、L1 的 `Disconnected` 死循环，一条都钉不住。把循环体抽成 `fn heartbeat_loop(stop_rx, interval, emit)` 注入可测间隔与计数闭包即可。
- **T2（最具判别力的缺口）`src/process.rs:606-620` 不具判别力。** 子进程全程零输出，`last_output` 的初值就是 `ProcessActivity::new()` 的时刻，把 `mark_output` 调用全删掉这条测试照样绿——名义满足「一次性 stub 静默 2s → idleMs ≥ 2000」，实质没验证「按最后一次输出计」。改成「先输出一行 → 静默 2s」并断言 `elapsed ≥ 2000 && idle ≥ 2000`（再加一个「两次输出」变体断言 `idle < 500`）即可一句话钉死。
- **T3（自证）`src/dispatch.rs:3473-3479`。** 手工填 `cache_read: 42`、`ReconcileUsage` 由它复制，只验了 serde 字段名，不覆盖「`parse_output` 把 `cache_read` 填进 usage」也不覆盖「对账从留痕 `cache_read` 回填」。
- （补充建议第 4 条）零改动只在纯函数层测（`3481-3491`），没有走 `run_one` 的 E2E 证明「重派后旧快照仍能被 `rollback_node_changes_core` 找到」——新留痕 `before_dir` 为空、靠 `latest_before_dir` 命中旧记录这条链目前是推理而非测试。
- **T5（低）三处新测试在无 python 时静默通过**（`dispatch.rs:2970`、`process.rs:607`、`acp.rs` 的 `resolve_binary` 分支 eprintln+return）。本机有 python，别的机器上 TASK-07 的 ACP/一次性覆盖面整段消失而套件仍绿。
- **T6（flake，可接受）** `interactive_stdout_truncation_marker_is_delivered_at_full_queue_boundary`（`process.rs:598-618`）的同步点用 10s `yield_now` 忙等，失败时要空转满 10s；计数在 `send` 之前自增，理论上存在「计数到位、标记尚未入 channel」的窗口，后续 `recv_line` 的 2s 超时兜得住，判为不 flake。`task07_fake_acp_three_same_tool_calls_...`（`2967`）沿用 fixture 的 `timeout_secs=1` 跑整轮 ACP（起 python + initialize + session/new + prompt），负载高时有超时风险，与既有 ACP 测试同款、非本期引入。

## 对照 A 节是否完整实现

**基本完整：7 条都有落地代码与对应测试，无「写了报告没写代码」的条目；但第 1 条的异常路径（心跳线程生命周期）不成立，且该条的两个关键测试其中一个不具判别力。**

缺项（按严重度）：

1. `dispatch.rs:739` 心跳线程把 `Disconnected` 当 tick → panic 时死循环 + 线程泄漏（第 1 条「节点结束/取消后停止」在异常路径不成立）。
2. `dispatch.rs:2575-2582` `lastTool` 未脱敏、未截断就进事件（第 1 条新字段的安全缺口）。
3. `dispatch.rs:1620-1696` 主/备与 ACP→一次性回退共用 activity，工具计数跨尝试累加，会误触 B 节空转提示（第 1 条口径未定）。
4. `process.rs:606` 一次性 idleMs 测试无判别力（第 1 条测试要求实质空转）；心跳线程本身零覆盖。
5. `dispatch.rs:1146` `.prev` 清理失败连坐已提交的快照（第 2 条失败分支不完整）+ `1101` 删除顺序先于 `is_dir` 校验。
6. `dispatch.rs:3473` cacheRead 测试自证（第 6 条测试未钉链路）。
7. `acp.rs:480-485` 只计 `session/update/tool_call`，权限请求里的 toolCall 不计（第 1 条覆盖不全，取决于 kimi 真实通知形态）。
8. `dispatch.rs:305` 容器级 `serde(default)` 的副作用：留痕字段名打错静默降级（第 5 条的代价，建议补边界测试）。