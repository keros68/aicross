# TASK-05A 修正轮 R2（ACP 长驻，复审 R1 的收尾）

先读 `tasks/fix-05a-r1.md`、`tasks/REPORT-05A.md` 的「修正轮 R1」一节，再读 `tasks/review-05a-r1-ds.md`（DeepSeek 复审）与 `tasks/review-05a-r1-glm.md`（GLM 复审，若存在）。两家对 R1 的 19 条大多核实修好，本轮只收下面的尾巴。每条在 `tasks/REPORT-05A.md` 末尾「修正轮 R2」一节写「已修：怎么改的（文件:行）／已补：测试名」；报告里 R1 的「已补测试」有三处名不副实（第 5、7、14 条），按实际断言改写。验证：`cargo check` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`，命令与输出原文贴进报告。**不要跑 `cargo fmt`**（上一轮把 10 个无关文件格式化了，编排者已剔除）；只改本轮需要的行。不起窗口、不发真机请求、不 commit、不 push。

## 必修（按严重度）

1. **超时分类错误 → 超时仍回退**（DS 13）：`acp.rs` 轮询循环把「最后一窗口（remaining ≤ 100ms）的接收超时」判成 `Process`，`permits_fallback()` 为真，于是静默超时照样跑一次一次性（最坏 2×timeout，写节点重复执行）。改为按 `RecvTimeoutError` 类型（不是 `e.contains("超时")` 字符串）分流：仍剩时间 → 继续轮询；到 deadline → `Timeout`，不回退。补测试：用假 server 起一个「收到 prompt 后不输出任何行直到 deadline」的真进程，断言结果 status=timeout、`runner` 为 acp、没有第二次一次性调用（用 python stub 计数）。
2. **`append_bounded` 截断 panic**（DS 8）：`String::truncate` 落在非字符边界会 panic 打死派发线程。改用字符边界（`floor_char_boundary` 或按 `char_indices` 找边界）；补测试：CJK 内容凑到上限附近不 panic 且标注截断。
3. **「杀 ACP 进程 → 回退 → 留痕 runner: oneshot」端到端**（DS 10①）：用假 server + python stub 走 `run_one`：第一轮 ACP 成功，杀掉假 server 进程，第二轮断言走一次性（`runner: oneshot`、`runner_note` 含回退原因、argv 不含 -S/-r、prompt 为完整上下文）。同时把 `foreign_acp_session_fallback_*`、`killed_acp_process_*`、`cancelled_acp_process_*`、`queued_acp_node_observes_cancel_marker_*` 四条合成 Err/同义反复的测试改成驱动真实路径（至少 `run_one` 或 `AcpClient` 的等锁分支）。
4. **`shutdown_all` 不得阻塞等 in-flight 轮次**（DS 新风险①）：对每个进程 `try_lock`，拿不到就直接终止进程（Job Object/taskkill）并从池中移除；map 锁不持有超过一次遍历。补测试：一轮在飞时调用 `shutdown_all` 在 1s 内返回。
5. **取消后的迟到 `session/update` 不进下一轮**（DS 新风险②）：取消或超时后把该 session 标为 stale，后续同 sessionId 的 update 丢弃直到新 prompt 开始；或每轮 prompt 前 drain 已到达的行。补测试。
6. **stdout mpsc 无上限**（DS 新风险③）：读线程按字节计数，超过 8MB 停止入队并记「已截断」，进程仍排空。补测试。
7. **权限判定不得被 title 绕过**（DS 2）：`is_read_request` 先看 `kind`：`kind ∈ {read, search, list, fetch}` 视为读；`kind ∈ {edit, write, execute, delete, move}` 一律视为写，不看 title；只有 kind 缺失时才看 title。补测试：`kind=execute` + title 含 read → 只读节点拒绝；`kind=read` → 放行。假 server 加一次读类权限请求，断言只读节点放行。
8. **ACP 关掉后旧 ACP session id 不进 `-S`**（DS 1 残留）：`Reply.runner` 已随图落盘，续聊判定时若父回复 `runner == "acp"` 且本轮走一次性 → 视为无 session，发完整 prompt 并记 `runner_note`。补测试。
9. **本地合成失败结果不硬编码 `runner=oneshot`**（DS 9）：用 `'unknown'`/空并在结果卡显示「未运行」。
10. **`cachedReadTokens`**（DS 7）：usage 解析同时接受 `cachedReadTokens` 与 `cacheReadTokens`，补断言。
11. **测试健壮性**（DS 新风险④）：`fake_client()` 找不到 python 时 `return`（跳过并 eprintln），不 unwrap；`shutdown_terminates_fake_acp_process` 用 pid 精确匹配（`tasklist /FI "PID eq N"` 或 `OpenProcess`）并轮询等待 ≤2s，不用 100ms sleep。
12. **真机复跑**由编排者做：R1 加的 `set_model` 必发 + `availableModels` 校验、`cachedReadTokens` 都在真机上没跑过；编排者会在 R2 后再跑一次 `live_kimi_acp_two_turn_continuation`。把该 ignored 测试的断言加强：第一轮 `session_id` 非空、`usage.input > 0`、第二轮正文含随机 nonce（不是固定词）、结束调 `shutdown_all`。

13. **真机已炸：kimi ACP 没有 `models.availableModels`**（编排者 R1 后真跑 `live_kimi_acp_two_turn_continuation`：`AcpError { kind: Protocol, message: "ACP 不支持声明模型 kimi-code/k3-256k" }`，0 passed）。编排者用原始 JSON-RPC 探了一次 kimi 0.42 `kimi acp`：`session/new` 的 result 是 `{ sessionId, configOptions: [ {id:"model", category:"model", currentValue:"kimi-code/k3-256k", options:[{value:"kimi-code/kimi-for-coding"},{value:"kimi-code/kimi-for-coding-highspeed"},{value:"kimi-code/k3"},{value:"kimi-code/k3-256k"}]}, {id:"thinking", category:"thought_level", currentValue:"high", options:[low,high,max]}, {id:"mode", category:"mode", currentValue:"default", options:[default,plan,auto,yolo]} ], modes: {currentModeId:"default", availableModes:[...]} }`，`initialize` 的 `agentCapabilities` 含 `loadSession:true`、`sessionCapabilities.{list,resume,close,delete,fork}`。改法：模型校验先读 `configOptions[category=="model"]`（`options[].value`），没有再退到 `models.availableModels`；两者都没有时**不校验、不回退**，只在 `runner_note` 记「ACP 未暴露模型清单」；切模型用 ACP 规范的 `session/set_config_option {sessionId, configId:"model", value}`，失败再试 `session/set_model`。补测试：假 server 改成 kimi 这种 `configOptions` 形状（另保留一条旧 `models` 形状）。
14. **只读/盲节点用 kimi 的 `plan` 模式**：`mode` 选项里 `plan = Read-only planning; no tool execution`——这就是 ACP 下只读/盲节点的 `--no-tools` 等价物（第 2 条的权限拒绝仍保留作兜底）。会话建立后按节点写权限 `session/set_config_option {configId:"mode", value:"plan"}`（可写节点用 `default`，永不 `auto`/`yolo`），失败记 `runner_note`。补测试。
15. **thinking 映射**：`thinking` 选项 low/high/max 对应我们的 关·低→low、中·高→high；用 `set_config_option` 设置，失败只记 note。
16. **session/close**：`sessionCapabilities.close` 存在时，节点结束（含取消）发 `session/close`，不让会话在长驻进程里堆积；`shutdown_all` 前逐个 close 再终止。

## GLM 补充

GLM 复审（`tasks/review-05a-r1-glm.md`）与 DeepSeek 结论一致：GLM 的 A=第 1 条、B=第 4 条、C=第 7 条、D=第 3 条、E=第 8 条，按上面做即可。GLM 额外的：

17. **假 server 的会话复用要可判别**（GLM 10a/10c）：`fixtures/acp/fake_server.py` 的 `session/new` 每次返回递增 id（`fake-session-1`、`-2`…），客户端错误地重开 session 时「两轮复用」测试必须失败；假 server 收到对未知请求的响应时校验 `error.code == -32601`，不是任何合法 JSON 都放过。
18. **`append_bounded` 标记后不得再追加**（GLM G）：截断标记写入后置一个「已截断」位，后续内容直接丢；`initialize` 阶段的 banner/非 JSON 行也进 raw（与 prompt 阶段一致）。
19. **同 key 首次并发竞态**（GLM F，低）：两线程同时未命中池各自 spawn、后插入者覆盖前者，前者进程被 Drop 杀掉、其节点白白回退。改成先占位（map 里放 `Arc<Mutex<Option<Client>>>` 或按 key 的启动锁），第二个线程等第一个启动完再复用。补测试：两线程同 key 同时 `run`，只 spawn 一个进程（python stub 计数）。
20. **跨 runner 脏 session id 记为已知问题**（GLM E）：第 8 条修完后在 `docs/PRODUCT.md`「已知限制」或 REPORT 里写一句；用户可用「新开会话」恢复。
