复审完成。我逐条读了 fix-05a-r1.md 的 19 条、REPORT-05A.md 的 R1 节，并核对了 acp.rs / process.rs / terminal.rs / dispatch.rs / channels/mod.rs / orchestration.rs / server.rs / fixtures/acp/fake_server.py 及全部前端镜像文件的当前代码。

# 19 条逐条判定

| # | 判定 | 依据 |
|---|------|------|
| 1 回退丢 session id | **已核实修好** | `reset_acp_fallback` 只返全量 prompt（dispatch.rs:1569-1571），Fallback 分支换掉 session+outbound 并写 runner_note（dispatch.rs:1355-1357）；kimi `-S` 仅在 id 非空时加（channels/mod.rs:230-232）；测试断言 argv 无 `-S/-r` 且含完整 prompt（dispatch.rs:2492-2512） |
| 2 权限分类 | **已核实修好**（留一个放行方向缺口，见缺陷 C） | 只在 `allow_once/reject_once` 里选、永不 allow_always（acp.rs:376-401）；只读/盲拒写放读；策略进 runner_note（dispatch.rs:1344-1348）；假 server 实测两分支（`turn-1:no`/`turn-1:once`，fake_server.py:20 提供 allow_always 诱饵） |
| 3 Job Object + 开关清池 | **已核实修好** | spawn_interactive 子进程入 job（process.rs:86 → terminal.rs:102-104）；开关关闭调 `releaseAcpPool`（SettingsModal.tsx:208）；测试 `shutdown_terminates_fake_acp_process` 用 tasklist 验进程消失。但引入阻塞问题 B，见下 |
| 4 轮级/进程级分流 | **已核实修好** | 六类错误分类（acp.rs:13-40）；非 JSON 行 continue 且进 raw（acp.rs:330-335）；未知带 id 请求回 -32601（acp.rs:351-356）；update 按 sessionId 过滤（acp.rs:337-345）；池只对 Process/Timeout 摘除（acp.rs:146-154），Turn/ForeignSession 保留进程 |
| 5 取消 | **已核实修好**（所补测试未覆盖排队路径） | try-lock + deadline + 取消复查（acp.rs:118-135）；进行中取消只发 `session/cancel`（acp.rs:310-315），ACP 进程不进 taskkill 注册表（只有 run_process_registered 注册）。但 `queued_acp_node_observes_cancel_marker_before_acquiring_process`（dispatch.rs:2539-2543）只测了标记位，没走到排队循环 |
| 6 池键隔离位 | **已核实修好** | key 含 cwd+blind（acp.rs:92-99），盲节点强制只读且 cwd 为独立沙箱（dispatch.rs:1231-1241） |
| 7 usage 与模型校验 | **已核实修好** | 解析 `result.usage`（acp.rs:267-285）；set_model 后校验 availableModels 含 id（acp.rs:232-247）；编排者真机跑通说明 kimi 返回格式匹配 |
| 8 留痕 | **已核实修好** | stderr 每轮 take 清零（acp.rs:289 / process.rs:46-53）；stderr 读满后继续排空不断管（process.rs:111-126）；raw 8MB 封顶（acp.rs:424-435）；`#[serde(default = "oneshot")]`（dispatch.rs:244-247）+ 旧留痕读回测试（dispatch.rs:2424-2429）；「kimi 不报用量」进 runner_note 不进 error（dispatch.rs:1733-1747） |
| 9 前端 runner | **已核实修好** | Reply 加可选 `runner`（graph/types.ts:22）、写回条件展开（dispatch.ts:177）、结果卡与用量表都显示长驻/一次性（DispatchOverlay.tsx:421,448）；Rust Reply 镜像 `#[serde(default)]`（orchestration.rs:88）——只追加，旧图不受影响 |
| 10 测试补强 | **改法有问题** | ②已修好：一次性回归改本地 python stub，物理上不可能真发请求（dispatch.rs:2432-2477）；③有：vitest `长驻运行方式写回 Reply` 断言 `runner:'acp'`（dispatch.test.ts:152-159）。但①三处不足：a) 假 server 对 `session/new` 永远返回同一个 `"fake-session"`（fake_server.py:13），即使客户端错误地重新开 session 测试也照样通过——「两轮复用」不可判别；b) fix 单明确要的「杀进程后回退且留痕 runner: oneshot」端到端没有——只有纯函数喂 Err（dispatch.rs:2480-2489）和杀进程验证（acp.rs:606-623），两者没接起来，run_one 回退→落痕整段无覆盖；c) fake server 收 -32601 响应时只 `json.loads` 不校验错误码（fake_server.py:24），客户端回任何合法 JSON 都算过。REPORT 第 10 条的「已补」陈述强于实际覆盖 |
| 11 安装命令 | **已核实修好** | 只显示二进制名+「需自行安装」（SettingsModal.tsx:540-541），无 npm 包名 |
| 12 流式口径 | **已核实修好** | REPORT-05A.md:105 写死口径，配套单测存在 |
| 13 超时不回退 | **改法有问题** | 分类器层面正确（Timeout→Failed，dispatch.rs:1214, 2515-2524），排队时间也确实计入 deadline（acp.rs:91,118-132）。**但 request() 的等待循环里守卫用的是陈旧 remaining**（acp.rs:325）：deadline 前最后 ≤100ms 窗口内 recv_line 超时返回的是 `Process` 而非 `Timeout`，而静默挂死的进程恰恰总是走这条路（流式消息不断时才会走 acp.rs:317 的正确 Timeout 分支）。结果：真超时→`permits_fallback()` 为真→照旧回退 oneshot 且再给满额 timeout——GLM 原始警告的「双倍时长、写节点重复执行」在挂死场景下原样存在。测试只喂构造的 Timeout 错误，测不到这条真实路径 |
| 14 续聊换模型 | **已核实修好** | `sessions` 记录当前模型，不一致即重新 set_model 并校验（acp.rs:222-252） |
| 15 initialize 超时 | **已核实修好** | 用节点剩余时间、上限 30s（acp.rs:177，run() 传剩余时长 acp.rs:110） |
| 16 probe 一致 + 重新探测 | **已核实修好** | probe 传 `kimi_fallback()`（acp.rs:490），与 start_kimi（acp.rs:159）一致；失败显示错误不吞（SettingsModal.tsx:545，effect 172-175），「重新探测」置 null 触发重跑（547） |
| 17 shutdown_all | **已核实修好** | `acp::shutdown_all()` 存在（acp.rs:451-460），ExitRequested 调用（server.rs:4396-4397），ignored 真机测试末尾也调用（acp.rs:661） |
| 18 设置页文案 | **已核实修好** | SettingsModal.tsx:548 明确只读策略与一次性差异 |
| 19 AcpOutput 直接产 ChannelOutput | **已核实修好** | acp.rs:42-47；run_one 整体接收 `output.channel`（dispatch.rs:1339-1341） |

# 回归与契约核对（无回归）

- 第 3/4 期一次性路径未动：`run_process_registered`/`build_invocation` 行为不变，kimi `-S` 门控照旧；`runner/runner_note` 纯追加。
- 留痕契约：`aicross-dispatch/1` 名称未变，`runner` 带 serde default，旧留痕可读回（dispatch.rs:2424）；`Reply` 只加可选字段，旧图可解析（orchestration.rs:88 / types.ts:22）。
- 注册与镜像：server.rs:4243-4244 两条命令已注册；tauri.ts:375-377 命令、400-405 `AcpAdapterStatus`、476-477 `runner/runnerNote`、397 `acpEnabled` 全部对齐；dispatch-settings.ts 默认 false 且旧配置缺字段回落默认（dispatch-settings.ts:16,41）。

# 会导致误判或真机出问题的点（按严重度）

- **A（高）超时误判回退**：见第 13 条。修法很小——recv_line 超时后重算 `Instant::now() >= deadline` 再决定返回 Timeout 还是 continue（acp.rs:325 一处）。不修的话第 13 条等于没闭合，且「写节点超时后二次执行」是用户真会踩的路径。现有 188 个测试测不出它。
- **B（中高）关池阻塞**：`shutdown_all` 对每个 client 做阻塞 `lock()`（acp.rs:454），而 client 锁在整轮 prompt 期间被持有。派发进行中用户关掉 Kimi 长驻开关（SettingsModal.tsx:208，同步命令走主线程）或退出应用（server.rs:4397）都会卡到该轮结束/超时（上限 3600s）。建议 try_lock 跳过忙进程，或先对全部 registry_key 置取消标记再清池。
- **C（中低）读类判定可被 title 翻盘**：`is_read_request` 把 kind 和 title 拼在一起做词匹配（acp.rs:404-423），`kind:"edit"`、title 含 "find/search/list" 的写请求（如 "Find and replace in config.json"）会在只读节点被放行。建议 kind 命中 edit/delete/move/execute 时直接判写、不走 title。
- **D（中低）第 10 条测试缺口**：见上表——补一条「假 server 会话中途退出 → run_one 回退 → frontmatter 含 `runner: oneshot`」的端到端即可同时钉住 a/b 两处。
- **E（低）跨 runner 脏 session id**：与第 1 条互为镜像——ACP 跑过一轮后 Reply 里存的是 ACP session id；之后用户关掉 ACP 开关再派同一续聊节点，这个 id 会原样交给 `kimi -S`（dispatch.rs:1314 → mod.rs:230），表现为节点报错。用户可用「新开会话」恢复，故只记为已知问题。
- **F（低）同 key 首次并发竞态**：两线程同时未命中池各自 spawn，后插入者覆盖前者，前者进程被 Drop 杀掉、其节点白白回退 oneshot。自愈、无串话，可不修。
- **G（ cosmetic）**：append_bounded 截断标记后仍可能再追加内容（acp.rs:431-434）；initialize 阶段的 banner 不进 raw（仅 prompt 阶段记录）；第 5 条测试名与实际覆盖不符。

# 结论

**一句话：TASK-05A 已接近可交付——19 条里 17 条确实修好、契约无回归、验证链完整，但第 13 条（超时不回退）因 acp.rs:325 的陈旧 remaining 判断在真实挂死场景下仍未闭合，建议先落一个几行的修复（缺陷 A），顺手带上 B/C，再关单。**

仍未闭合项（按严重度）：A 超时误判仍触发 oneshot 回退（acp.rs:325）→ B shutdown_all/开关释放会阻塞在忙进程锁上（acp.rs:454 + SettingsModal.tsx:208/server.rs:4397）→ C title 词匹配可在只读节点放行写请求（acp.rs:404-423）→ D 杀进程→回退→留痕端到端测试缺失、假 server 复用不可判别（fake_server.py:13,24）→ E 跨 runner 脏 session id（已知问题即可）→ F/G 可随 backlog。