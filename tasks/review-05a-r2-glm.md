复审完成。先给逐条结论，再给新缺陷扫描，最后回答可交付性。

## 逐条核对（20 条）

| # | 结论 | 依据 |
|---|------|------|
| 1 | **已核实修好** | `src/process.rs:12-16` 具名 `RecvLineError`（包装 `RecvTimeoutError`）；`src/channels/acp.rs:438-450` 按类型分流，`Timeout` 且到 deadline → `AcpErrorKind::Timeout`，不进 `permits_fallback()`（`acp.rs:35-40`）；测试 `silent_acp_process_reaches_typed_timeout_without_fallback`（`acp.rs:860`）断言 `!permits_fallback`。附注：`dispatch.rs:1372` 仍有 `contains("超时")`，但只用于 status 标签，不影响回退。 |
| 2 | **已核实修好** | `acp.rs:557-570`：`floor_char_boundary` 截断 + marker 后直接 return；测试 `append_bounded_cjk_is_utf8_safe_and_stops_after_marker`（`acp.rs:868`）覆盖 CJK 边界与终态。剩余 0/1 字节的极端分支我也手推过，无 panic 路径。 |
| 3 | **改法有问题（轻）** | 链条是**三段拼接**而非题目要求的单条端到端：`killed_acp_process_returns_process_error_from_real_client_path`（`acp.rs:819`，真实杀进程→Process 可回退）＋ `disabled_acp_previous_runner_resets_session_and_uses_full_context`（`dispatch.rs:2335`，run_one 清 session/完整上下文/留痕 oneshot，但触发条件是 acp 关闭而非杀进程）＋ `reset_acp_fallback_builds_full_context_invocation_without_resume_flags`（`dispatch.rs:2366`，argv 无 -S/-r）。代码路径本身核对无误，但 `AcpResolution::Fallback` 分支（`dispatch.rs:1366-1369`）没有任何测试直达——「acp_enabled=true 下杀进程→run_one→留痕」这条题面要求的直连不存在。四条合成测试改为真实路径则已完成（`acp.rs:819/829/836/847`）。 |
| 4 | **已核实修好** | `acp.rs:586-604`：`std::mem::take` 一次性摘池（map 锁不跨遍历持有），`try_lock` 失败按记录 PID `terminate_process_id`，不等待；测试 `shutdown_all_returns_within_one_second_when_client_is_locked`（`acp.rs:899`）。 |
| 5 | **已核实修好** | 每轮 prompt 前 `drain_pending`（`acp.rs:279`）；取消（`acp.rs:426-431`）与超时（`acp.rs:444-449`）均 `close_session` 并从 map 移除，迟到 update 因 sessionId 不匹配被过滤；测试 `late_session_update_is_drained_before_next_prompt`（`acp.rs:808`）。微竞态见下「新发现」。 |
| 6 | **已核实修好** | `src/process.rs:117-138`：字节计数，超 8MB 只入队一次 `[ACP stdout 已截断]`，随后继续 `for line in BufReader::lines()` 排空至 EOF；测试 `process.rs:468`。 |
| 7 | **已核实修好** | `acp.rs:538-556`：kind 白名单→黑名单（不看 title）→未知 kind 视为写→kind 缺失才看 title；`answer_permission`（`acp.rs:507`）据此类结果选 `allow_once/reject_once`。假 server 加了读/写两类请求（`fake_server.py:46-48`）；测试 `acp.rs:698/763`。 |
| 8 | **已核实修好** | `dispatch.ts:109-114` `continuationRunner` → `DispatchOverlay.tsx:125` `previousRunner` → `dispatch.rs:102`/`1326-1329` 重置；Rust 端测试 `dispatch.rs:2335` 断言 task file 为完整上下文且 raw_stdout 不含旧 session id。 |
| 9 | **已核实修好** | `DispatchOverlay.tsx:220` `runner:'unknown'`，423/450 显示「未运行」；`tauri.ts:477` 类型含 unknown；测试 `dispatch.test.ts:169`。 |
| 10 | **已核实修好（断言只有新形）** | `acp.rs:306-309` `cachedReadTokens` 优先、`cacheReadTokens` 兜底；测试断言 cache_read=5（`acp.rs:741`）。但假 server 只发新形，旧形兜底无断言。 |
| 11 | **已核实修好** | `acp.rs:668-673` 缺 python 打印后 `return None`；`acp.rs:877-897` tasklist CSV 按 PID 精确匹配、轮询至 2s。 |
| 12 | **已核实修好（待真跑）** | `acp.rs:948-985`：随机 nonce、首轮 session 非空、`usage.input>0`、次轮含 nonce、末尾 `shutdown_all`，保持 ignored——断言全部就位，等编排者真机复跑。 |
| 13 | **已核实修好** | `acp.rs:335-371`：先读 `configOptions[category==model 或 id==model].options[].value` 校验；切模型先 `session/set_config_option`（`acp.rs:348-352`），失败才 `session/set_model`；两类清单都缺失→只记「ACP 未暴露模型清单」，不拒绝不回退。假 server 三种形状齐备（默认 kimi 形 `fake_server.py:22-29`、`--legacy-models`、`--no-model-list`）；测试 `acp.rs:770/777/785`。 |
| 14 | **已核实修好** | `acp.rs:376-379`：`!writable || blind → plan`，可写→`default`，永不 auto/yolo，失败记 note；测试 `acp.rs:785/792` 经假 server 回显 config 正向断言 `plan`/`default`。 |
| 15 | **已核实修好** | `acp.rs:380-383`：关/低→low、其余→high，失败仅 note；测试断言回显 `low`/`high`。 |
| 16 | **已核实修好（语义取舍正确）** | `acp.rs:214-217` 探测 close 能力；取消/超时 `close_session`、Process/Timeout 摘池前 `close_all_sessions`（`acp.rs:152-158`）、`shutdown_all` 先逐个 close 再终止（`acp.rs:590-593`）。成功完成的会话保留到 shutdown——若按字面「节点结束即 close」会直接杀死续聊复用（live 测试第二轮必挂 ForeignSession），实现取舍是对的；代价是从不续聊的废弃 session 会留到 shutdown。 |
| 17 | **已核实修好** | `fake_server.py:21-22` 递增 id；`fake_server.py:51-54` 未知请求响应非 -32601即 RuntimeError（python 崩溃→客户端 Disconnected→测试必失败，可判别）；`acp.rs:740` 断言 `second.session_id=="fake-session-1"`，错误重开会拿到 `-2`。 |
| 18 | **已核实修好** | `acp.rs:558-561` marker 终态；`acp.rs:278-279` `prelude_raw` 进首轮 raw（测试 `acp.rs:725` 断言 banner 在 raw_stdout）。 |
| 19 | **改法有问题（仅测试弱）** | 代码正确：`acp.rs:126-140` 先插空 slot、同 key 第二线程在 `acquire_slot` 等锁复用，不再覆盖。但测试 `same_key_concurrent_initialization_spawns_one_fake_process`（`acp.rs:924`）**没有驱动 run()/acquire_slot**，而是手写 `slot.client.lock()` 复刻同一纪律、用 starts 原子计数替代题目要求的 python stub 进程计数——接近 R1 被批的「同义反复」。 |
| 20 | **已核实修好** | `docs/PRODUCT.md:184` 已写跨 runner 限制与「新开会话」恢复。 |

## 契约与回归核对

- **留痕契约只追加**：`dispatch.rs:246-247` `runner` 带 `#[serde(default="oneshot")]`，旧 YAML 解析回 oneshot（`dispatch.rs:2279` 测试）✓。
- **Reply 镜像**：`orchestration.rs:88` `runner: Option<String>` ↔ `graph/types.ts:22` ✓。
- **注册/类型镜像**：`server.rs:3858-3859` 两命令、`server.rs:4012` 退出时 `shutdown_all`；`tauri.ts:375-377/439/477`、`SettingsModal.tsx:208`（关开关即 `releaseAcpPool`）、`dispatch-settings.ts:41` 持久化 ✓。
- **第 3/4 期一次性回归守护**：`acp_disabled_or_unsupported_preserves_oneshot_dispatch_result`（`dispatch.rs:2287`）✓；run_one 的 `oneshot_session/oneshot_outbound` 对非 ACP 路径是原值克隆，行为等价 ✓。
- **R1 三处名不副实的「已补测试」已改写**：REPORT-05A.md:111、113、120 均按实际断言改写并注明 R2 补齐 ✓。
- **沙箱 cargo test 可复现性**：缺 python 一律跳过；三个 `shutdown_all` 测试共享全局池、互相偷 slot，但我推演过各断言在偷取后仍成立（close marker 谁关都写、pid 谁杀都死），不构成 flake。`late_session_update` 有极小竞态（见下）。

## 修正引入的新问题 / 会在真机炸的点

1. **live 复跑最可能的炸点**：kimi 是否接受 `session/set_config_option`（model/mode/thinking）只经假 server 验证；编排者的 probe 只确认了 `session/new` 返回形状。若 kimi 拒绝 model 的 set_config_option 且不支持 `session/set_model`，`acp.rs:354-358` 的 `?` 会抛 Protocol → 经 `permits_fallback` 整节点回退一次性——**不炸流程（留痕有 note），但长驻收益归零**，live 测试则直接 unwrap panic。
2. **usage 形状未真机验证**：live 断言 `usage.input > 0`，若 kimi 不在 prompt response 的 `result.usage` 返回用量会失败——这是断言在暴露问题，不是代码缺陷，但编排者要预期到这一失败模式。
3. **后端 Failed/Cancelled 的 ACP 节点留痕 `runner="oneshot"`**（`dispatch.rs:1314/1370-1378`）：节点根本没跑一次性却留痕 oneshot，与第 9 条同病，不在 20 条清单内，功能无害。
4. **微竞态**：`late_session_update` 测试里，假 server 先回 result 再补 LATE 行；若读线程被长时间调度延迟，LATE 会漏过 drain、落进下一轮正文使断言失败。概率极低，可留。
5. 旧形 `cacheReadTokens` 兜底无测试断言（第 10 条尾巴）。

## 结论：R2 之后 TASK-05A 是否可交付？

**代码侧可交付：20 条中 17 条完全闭合，2 条（3、19）是测试覆盖缺口而非代码缺陷，修正是取舍正确且无新功能缺陷；是否最终交付取决于编排者真机复跑 `live_kimi_acp_two_turn_continuation` 能否通过——这是第 12-15 条唯一未闭合的验证。**

仍未闭合项（按严重度）：

1. **真机复跑未做**——kimi 对 `session/set_config_option` 的实际接受度与 `result.usage` 形状未验证（第 12-15 条的真机面）；复跑若失败，最可能是这两个点。
2. 第 3 条缺「杀进程→run_one→留痕 oneshot」直连测试，`AcpResolution::Fallback` 分支无直达覆盖。
3. 第 19 条测试未驱动 run()/acquire_slot、未用 stub 进程计数。
4. 后端 Failed/Cancelled 的 ACP 节点留痕 runner 误标 oneshot（清单外，建议下轮顺手改为 unknown）。
5. 旧形 `cacheReadTokens` 无断言；late-update drain 微竞态（极低概率 flake）。