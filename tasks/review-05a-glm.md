# TASK-05A 盲审（GLM-5.3-Flash via cc-switch，只读，2026-09-14 11:15–11:24）

# Code Review — TASK-05 A 节（ACP 客户端）

## 1. 需求完整性

- `src/channels/acp.rs:124-152` — **回退后的续聊链路断裂**：ACP 成功一轮后 `Reply.session_id` 存的是 ACP session id；下一轮若 ACP 失败回退，`dispatch.rs:1303` 把同一个 id 作为 `effective_session` 传给一次性路径，即 `kimi -S <acp-session-id>`（`channels/mod.rs:229-231`）。ACP session id 不是 CLI 的 session id，要么报错要么静默丢上下文——违背「回退后结果与第 3 期一致」。反之（先 oneshot 后 ACP）也会因 `acp.rs:126` 的 `sessions.contains` 永久降级为 oneshot。建议：留痕里区分两种 id（如记录 runner 连同 id 来源），回退时若 `runner` 上一次是 acp 则清空 session 按首轮处理，或在 frontmatter 追加 `session_kind: acp|cli`。
- `src/dispatch.rs:1288-1291` — **ACP 失败原因零留痕**：Fallback 分支把 `attempt` 的 Err 字符串直接丢弃，oneshot 成功后整条留痕看不出 ACP 试过且为何失败。`aicross-dispatch/1` 允许追加，建议把失败原因追加进 error 前缀或 frontmatter（如 `acp_fallback_reason:`）。
- 需求项「claude/codex 只探测与设置项」「用量拿不到标 —」已满足：`acp.rs:287-327` 只探测；`DispatchOverlay.tsx:448` 的 `result.tokensIn || '—'` 把 0 显示为 —（已核实）。
- `initialize`（`acp.rs:104`）超时写死 5s，慢盘/首启冷启动会直接回退。建议复用入参 timeout 或放宽到 15s。

## 2. 逻辑正确性

- `src/channels/acp.rs:58` — **进程池 key 不含 cwd**：池按 `agent/provider/model` 复用，进程 cwd 在首次 spawn 时固定（`process.rs:79-81`），而盲节点的 `workdir` 是每节点独立的 `run_root/node_id`（`dispatch.rs:1179-1180`），材料写在该目录、提示词说「材料已在工作目录」。若进程由先前的非盲轮次（cwd=项目目录）建立，盲节点复用进程后 agent 按自身 cwd 找 REQUEST.md 会落空——盲隔离等价性被破坏。建议：key 加入 cwd，或盲节点强制新开进程（不进池）。
- `src/channels/acp.rs:200-201` — **单条非 JSON stdout 行即全路失败**：`serde_json::from_str` 失败直接 Err → 回退。ACP 服务器往 stdout 打一行 banner/日志就废掉整条长驻路径。建议：非 JSON 行跳过（可存入 raw）而非报错。
- `src/channels/acp.rs:208-213` — **未应答未知 server→client 请求**：只处理 `session/update` 和 `session/request_permission`；其他带 id 的服务器请求（fs/terminal 类，即便 capability 声明 false 也不能假设对方永不发）会落入 id 不匹配被跳过，agent 永久等待 → 整个 prompt 超时。建议：对带 `method`+`id` 的消息回 `method not found` error。
- `src/dispatch.rs:1262-1274` + 报告自述 — **超时也回退**：ACP 已跑满 timeout 后再跑一次完整 oneshot，最坏 2×timeout、双倍模型调用，且如报告所言可能重复执行写操作。timeout 与「进程异常退出」性质不同，建议 timeout 不回退直接判失败，或至少对 `write != 只读` 节点不回退。
- `src/channels/acp.rs:74-79` — **排队等待期取消无效**：并发两路同 model 时第二路阻塞在 `shared.lock()`，此时取消只置 flag；获得锁后不复查 `is_cancelled` 照样发完整 prompt。另外节点的 timeout 只从拿到锁起算，排队时间不受约束，N 路同 model 串行最坏 N×timeout。建议：拿锁后复查取消标志；排队也计入 deadline。
- `src/channels/acp.rs:140-151` — **续聊 session 忽略模型变更**：set_model 只在新 session 分支；同一线程中途换 `target.model` 会被静默用旧模型（oneshot 每轮都传 `-m`）。建议：续聊时也校验/设置模型。
- 半包/粘包：`process.rs:98-106` 用 `BufReader::lines()` 按行收，JSON 字符串内换行被转义，处理正确；`recv_line` 按 deadline 计时（`acp.rs:189-195`）正确；错误路径 `run()` 从池中摘除（`acp.rs:81-85`）、Drop 兜底杀树（`process.rs:59-65`）正确。锁序 pool→client 单向，无死锁。但注意竞态：超时摘池时另一等待者还持有 Arc，Drop 不执行，它会在濒死进程上继续 prompt——低概率，可接受。
- `process.rs:98-106` — 长驻 stdout 无 4MB 上限（oneshot 路径有 `bounded_reader`），失控输出会无限堆在 channel 里。建议：读线程同样 `take(MAX_PROCESS_OUTPUT)`。

## 3. 安全

- **应用退出不清杀池内进程**：`server.rs:4010-4025` 的 ExitRequested 只 drain 终端 PTY session，不触及 `acp.rs:35-38` 的静态池；`InteractiveProcess::drop`（`process.rs:59-65`）因池永久持有 Arc 永不触发。`server.rs:3982-3985` 上游注释自己写明了 Windows 子进程不随父进程死——`kimi acp` 会在应用退出后滞留后台。建议：ExitRequested 时遍历池 `terminate()`（暴露一个 `acp::shutdown_all()`），并顺带在 `acp_enabled` 关闭时清池。
- `acp.rs:228-247` vs `channels/mod.rs:216-238` — **只读不等价（偏严方向）**：oneshot 的 kimi 只读节点没有任何工具限制（CLI 默认全开、可真写盘）；ACP 只读则对所有 `request_permission` 自动 reject。行为不等价但方向安全；需在文档/设置文案写明差异，否则用户会觉得「ACP 下只读节点工具全失败」。盲节点因 `dispatch.rs:1171-1172` 强制只读，reject 是对的，但受上面 cwd 问题影响材料可能读不到。
- 环境变量：`spawn_interactive`（`process.rs:73-78`）与 oneshot 同样只加 `AI_CROSS_PEER`/`PYTHONUTF8`，密钥沿用子进程继承链，stderr 进留痕前过 `safe_trace`（`dispatch.rs:1408-1410`）——与第 3 期一致，未发现新问题。

## 4. 代码质量与契约

- 契约：`aicross-dispatch/1` 名称未动（`dispatch.rs:1415`），`runner` 为纯追加（`dispatch.rs:239`），序列化往返有测试；`Target`/`Reply` 图契约未触碰（`graph/types.ts` 不在改动列表）；`server.rs:3858` 注册、`tauri.rs:375-376/388-405/475` 类型镜像齐全。未发现问题。
- `src/channels/acp.rs:12-17` — `AcpOutput` 与 `ChannelOutput` 字段近乎重复（text/session_id/raw_*），且 `run_one` 里手工逐字段搬运（`dispatch.rs:1276-1279`）。现在可接受；等 claude/codex 接入前建议让 ACP 产出 `ChannelOutput`（加 usage 字段），减少第二处搬运。
- `src/channels/acp.rs:264` — `probe` 用 `resolve_binary(binary, None)`，而 `start_kimi`（`acp.rs:90`）带 `kimi_fallback()`：kimi 只装在 `~/.kimi-code/bin` 不在 PATH 时，设置页显示「未检测到」但派发实际能用，探测结论失真。建议 probe 也传 fallback。
- `SettingsModal.tsx:171-174` — 探测结果缓存为 `acpAdapters !== null` 后本会话不再刷新，探测失败被吞成空数组；低危，可加手动刷新按钮。

## 5. 测试

- `dispatch.rs:2241-2250` — 两条「进程被杀后回退」测试只直接调用 `resolve_acp_attempt(Err(...))`，该函数不看错误内容、不做任何事；验收要求的「进程被杀后回退路径测试」字面满足、实质未覆盖 `run()` 的摘池和 `run_one` 的回退执行。`dispatch.rs:2223-2238` 回归测试因 `node()` 用 agent=claude（`supports=false`）只覆盖「不支持」分支，「支持但 ACP 失败→回退结果一致」无端到端测试。
- `acp.rs:360-388` — ignored 真机测试构造基本正确（同进程两轮、断言记忆内容），但：① `kimi-code/k3-256k` 直接当 ACP `modelId` 用，若 ACP 侧模型 id 格式不同，`session/set_model` 硬失败会造成假阴性（建议 set_model 失败降级为警告而非 abort，测试单独断言）；② `assert_eq!(second.session_id, first.session_id)` 是同义反复（传入什么返回什么），真正的续聊证据只有文本断言；③ 测试结束进程留在静态池里，cargo test 进程退出后成孤儿（同第 3 节问题）。
- 最该补的 3 个：
  1. **回退端到端**：acp_enabled=true + kimi 节点 + 令 ACP 必败（如传一个不存在的 session id 或指向死进程），断言 runner=oneshot、status/error 与 acp_enabled=false 完全一致、frontmatter 含 `runner: oneshot`，且 ACP 失败原因出现在留痕——同时钉住第 1、2 节两个缺口。
  2. **framing/健壮性**：用 pwsh 假 ACP server（分行写 JSON、半行 sleep、夹一条非 JSON 行、发一条未知 server→client 请求），钉住 recv 半包/粘包与非 JSON 容错的期望行为。
  3. **退出清杀**：spawn 交互进程入池 → 调用 `shutdown_all()`（新增）→ 断言子进程退出；防回归第 3 节的生命周期问题。

## 对照 A 节是否完整实现

**主体完整（kimi 长驻、进程复用、session 回写、失败回退、runner 留痕、探测与设置项、usage 标 — 均在），但「回退结果与第 3 期一致」在跨 runner 续聊场景下不成立，真机验收未跑。** 缺项按严重度：

1. 回退轮把 ACP session id 当 CLI session id 传给 `kimi -S`，续聊上下文丢失或报错（acp.rs:126 / dispatch.rs:1303）。
2. 进程池 key 不含 cwd，复用进程使盲节点材料目录错位（acp.rs:58）。
3. 应用退出不清杀 ACP 池进程，Windows 孤儿（server.rs:4010）。
4. ACP 失败原因在回退时零留痕（dispatch.rs:1288）。
5. 超时也回退 → 双倍调用/双倍时长、可能重复写操作；排队等待期取消不生效（dispatch.rs:1288 / acp.rs:74）。
6. 「支持但失败→回退一致」无端到端回归测试；回退单测为同义反复（dispatch.rs:2241）。
7. 续聊 session 忽略模型变更；非 JSON stdout 行、未知 server request 会让整路失败/挂起（acp.rs:140/200/208）。
---
usage: [usage] provider=Zhipu GLM model=GLM-5.3-Flash input=650128 fresh=56976 cache_create=0 cache_read=593152 output=15790 turns=24 ms=521913
