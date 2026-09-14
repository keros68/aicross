# TASK-05A 盲审（DeepSeek deepseek-v4-flash via pi，只读，2026-09-14）

## 1. 需求完整性

**1.1 `src/dispatch.rs:1305-1320` + `src/channels/acp.rs:119-121` —「异常退出回退一次性续聊、结果与第 3 期一致」不成立。**
`Reply.session_id` 现在存的是 ACP session id（`src-ui/src/graph/dispatch.ts:175`），续聊节点会把它原样回传；ACP 失败后回退分支把同一个 id 交给 `kimi -S`（`effective_session` = `node.session_id`），而 `prompt` 侧的 `sessions` 只记「本进程见过的 id」。于是「ACP session 失效」时，一次性 CLI 收到的是一个外来的 session id；同时续聊节点只发 delta 文本（`dispatch.rs:1233-1237` 用 `current_message`，不含上下文）。要么 CLI 报错、要么静默开新会话答一句无上下文的追问——两种都不是第 3 期行为。
建议：ACP 失败原因里区分「进程/会话不可用」，此时丢弃 session_id 并改用完整 `prompt`（`prepare_outbound` 的结果还在，没被消费），并把「已重置会话」写进留痕。

**1.2 `src/channels/acp.rs:190-201` — ACP 回传的 usage 完全不读。**
`session/prompt` 的响应体（ACP 的 `PromptResponse`，含 `usage.inputTokens/outputTokens`）与 `session/update` 的 usage 更新都没解析，`AcpOutput` 没有 usage 字段，`run_one` 的 `channel_output.usage` 永远是 0 → UI 恒显示「—」（`DispatchOverlay.tsx:448` 的 `|| '—'` 是对的）。要求是「若不回传才标 —」，现在是有也不读，属数据丢弃而非「不猜」。
建议：解析 `result.usage`（`inputTokens/outputTokens/totalTokens`）填进 `ChannelOutput`，拿不到才留 0。

**1.3 `src/channels/acp.rs:288-292` 与 `src-ui/src/components/common/SettingsModal.tsx:539-540` — Claude/Codex 的安装命令包名无来源。**
任务书只给了二进制名 `claude-agent-acp` / `codex-acp` / `acp-adapter`，仓库里没有任何 `@agentclientprotocol/...` 的依据（`docs/proto/resources-research.md:68` 也只按二进制名探测）。给用户的 `npm install -g @agentclientprotocol/claude-agent-acp` 与探测的二进制名对不上，照抄装完可能仍显示「未检测到」。
建议：核实包名，或只显示二进制名 + 「需自行安装」，不写死未验证的 npm 命令。

**1.4 「流式接收」只在传输层。**
`acp.rs:264-275` 把 chunk 拼进最终字符串，`request` 在 `session/prompt` 返回后才交回 `run_one`；工具调用、思考、usage 类 update 除进 `raw_stdout` 外没有任何字段，前端也不发进度（`DispatchOverlay` 只有排队/运行中/完成）。若 A 节验收把「流式」理解为界面上逐字可见，则缺；若只要求按 `session/update` 增量接收，则已满足（建议在 REPORT 里把口径写死）。

**1.5 已满足的部分**（简列，不展开）：`initialize → session/new →（需要时）session/set_model → session/prompt` 流程齐（`acp.rs:93-155`）；池键 `agent/provider/model`（`acp.rs:58`）；`runner: acp|oneshot` 进 frontmatter（`dispatch.rs:239,1429`）；claude/codex 仅探测 + 设置项；kimi 优先；用量展示走 `—`。

## 2. 逻辑正确性

**2.1 `acp.rs:35-38,83-88`（池是 `static`）+ `server.rs:4010-4026`（ExitRequested 只 drain PTY）——ACD 进程跨运行不可复用，且每次都要先失败一次。**
进程池从不跨 App 生命周期；`Reply.session_id` 是持久化的 ACP id，重启后第一个续聊节点必然命中 `Some(_) => Err("ACP 会话所属进程已退出")`（`acp.rs:121`），再落回 1.1 的坏回退。
建议：ACP 不支持 `session/load` 的前提下，把「本 id 不属于当前进程」当成「需要重建会话」而非「回退」：用完整 prompt 起新 ACP session，或明确降级为一次性且丢掉 session id。

**2.2 `acp.rs:76-80` — 一次轮级错误销毁整个进程池。**
`result.is_err()` 一律 `pool.remove` → Arc 归零 → `InteractiveProcess::drop` → taskkill。于是「正文为空」「一行坏 JSON」「某个节点的旧 session id」这类轮级问题会把同 agent/model 下所有其它节点已建立的 session 一起作废，后续每个续聊节点都要冷启动一次 kimi。
建议：区分进程级错误（stdout EOF、超时、写失败）与轮级错误（空正文、会话未知），后者保留进程。

**2.3 `acp.rs:206-207` — 任何非 JSON 行即致命。**
逐行 `serde_json::from_str` 失败直接 Err。CLI 往 stdout 打一行 banner/日志（kimi 版本提示之类）就会让 ACP 永久失效——现象只是留痕里出现 `runner: oneshot`，排查成本极高。
建议：忽略既非 JSON 对象、也无 `jsonrpc` 字段的行（仍写进 raw），只在 I/O 错误或超时上失败。

**2.4 `acp.rs:227-229` — 其它 agent→client 请求被静默吞掉。**
只认 `session/update` 与 `session/request_permission`；其它带 id 的 method（`fs/read_text_file`、`terminal/create` 等）落到 `id != 自己` 分支 `continue`，agent 会一直等回复直到超时，然后进程被杀 + 回退。
建议：对不支持的 method 回一条 `{"error":{"code":-32601,...}}` 再 `continue`。

**2.5 `acp.rs:208-213,264-275` — `session/update` 不按 sessionId 过滤。**
池内一个进程托管多个 session；上一轮响应后迟到的 update、或其它 session 的 update，都会被拼进当前节点的回复正文。建议比对 `params.sessionId` 与当前 session。

**2.6 `acp.rs:70-73` — 取消覆盖不到「在等池锁」的节点，且会误杀共享进程。**
`shared.lock()` 无超时、无取消检查；`register_process_id` 在拿到锁之后才执行，所以此时 `cancel_dispatch_node`（`dispatch.rs:275-279`）找不到 pid。两个图/两个 overlay 同时派同一 agent/model 时：第二路取消无效、锁到手照样发请求并计费；而第一路的 cancel 用 `taskkill /T` 杀掉的是**共用进程**，会把另一路正在跑的 prompt 一并打断并触发其回退重跑（有重复写副作用的风险，REPORT 的「不确定处」已经承认重试可能重复写）。
建议：`prompt` 前再查一次取消；锁加超时；跨节点共用进程时改发 `session/cancel` 而不是杀进程。

**2.7 `acp.rs:64-70` — 池键不含 workdir/project，进程 cwd 取第一个节点。**
`start_kimi(cwd)` 的 cwd 是**首个**触发该 key 的节点 workdir；盲节点 workdir 是 `run_root/<node_id>` 沙箱，若盲节点先跑，之后其它项目/共享节点共用的这个进程 cwd 就停在盲沙箱里（session 的 `cwd` 参数另说，但进程级 cwd 改不了）。
建议：池键加 project/blind 位，或至少在盲节点上禁止复用。

**2.8 `acp.rs:196`（`raw_stderr: self.process.stderr_text()`）+ `process.rs:64-70` — stderr 是全进程累计。**
每个节点的 `raw_stderr` 都包含先前所有节点的 stderr，重复写进 `.dispatch/*.md` 与前端。建议返回后清零缓冲。

**2.9 `process.rs:102-113` — stderr 只读到 4MB 就结束并关闭句柄。**
对照 `bounded_reader`（`process.rs:322-334`）里「读满后 `io::copy(..., sink())` 继续排空」的写法，长驻进程这条线程读完 4MB 就退出，管道读端被 drop；之后子进程再写 stderr 得到 broken pipe，CLI 可能直接退出。
建议：同样加持续排空。

**2.10 `process.rs:88-96` + `acp.rs:203-205` — 无界缓冲。**
`mpsc::channel` 无界、无行数/字节上限，`raw.push_str` 也不封顶；长驻进程下这是只增不减的内存（一次性路径有 `MAX_PROCESS_OUTPUT` 保护）。建议加 4–8MB 上限并标注截断。

**2.11 留痕缺回退痕迹。** `dispatch.rs:1288-1291` 的 `Err(_) => Fallback` 丢弃了 ACP 的失败原因，`runner` 停在 `oneshot`；用户在留痕里看不出「这轮试过长驻并失败，结果是一次性跑的」。取消路径同理（`dispatch.rs:1284-1287` 也不改 runner）。建议追加 `runner_fallback: <简述>`（`aicross-dispatch/1` 允许追加）或在 `error` 里带一行注记。

**2.12 `dispatch.rs:1596-1608` — ACP 成功路径也会被写 `kimi 不报用量` 进 `error`（status=ok 但 error 非空）。** 这是第 3 期既有行为，ACP 沿用后更容易误读。建议注记与 error 分字段。

**2.13 `acp.rs:137-148` — 模型只在 `currentModelId != model` 时 `set_model`，不看返回值/不校验。** 若 kimi 的 ACP model id 与图里的别名（`kimi-code/k3-256k`）格式不同，要么每次 set_model 都失败→永久回退，要么被静默忽略→跑的不是声明模型，而 ACP 路径不报真身、无从发现。建议 set_model 后校验 `models.availableModels` 含该 id，不含即报错。

## 3. 安全

**3.1 `acp.rs:241-262` — 只读/盲节点的工具权限与一次性不等价（本质缺口，无等价物）。**
一次性路径由 CLI 强制（claude `--tools`/`--restricted`、pi `--no-tools`/`--tools read,...`、codex `-s read-only`/`--ephemeral`）；ACP 只能在 agent 主动 `session/request_permission` 时作答。后果两端都错：
- agent 不请求权限的工具（或被 CLI 自身配置自动放行）在只读/盲节点照常执行；
- 反过来，`!writable` 时挑 `reject*`，会把**读**类权限请求也拒掉——而一次性只读路径是允许 read/grep/find/ls 的，盲节点要读 `REQUEST.md`/`diff.patch`/`changed/`（`dispatch.rs:1466-1500`）正好可能被自己拒掉。
建议：按节点可见性/写权限维护「读类允许、写类拒绝」的策略（权限请求的 `toolCall.title/kind` 或 `session/update` 的 tool_call 可判），并在节点开始时声明该策略、写进留痕；不要用「全有全无」。

**3.2 `acp.rs:248-256` — 可能授出 `allow_always`（持久授权），并跨节点生效。**
writable 时取第一个 `kind.starts_with("allow")` 的选项，ACP 的选项顺序里可能是 `allow_always`。进程是长驻且被同 agent/model 的其它节点（含只读/盲）复用，一条持久放行会绕过后续节点的权限判断。
建议：显式优先 `allow_once`，永不选 `allow_always`。

**3.3 `acp.rs:35-38` + `server.rs:4010-4026` + `terminal.rs:826-829` — 应用退出杀不干净长驻进程。**
池是 `static OnceLock`，永不 Drop → `InteractiveProcess::drop` 不执行；ExitRequested 只 drain PTY 会话；kill-on-close Job Object 只对 PTY 子进程 `windows_job::assign_pid`。所以 `kimi acp` 既不在优雅退出路径、也不在崩溃兜底路径里，只能指望它对 stdin EOF 自行退出；`cargo test` 里的 ignored 真机测试同理会把进程留成孤儿。
建议：`spawn_interactive` 出来的子进程也 `assign_pid`，并在 ExitRequested 里清空池（发 kill 或显式关 stdin）。

**3.4 盲节点与共享节点共用同一长驻进程（`acp.rs:58`）。**
协议层 session 是分开的，但进程级状态（cwd、CLI 读到的项目配置/记忆/技能、3.2 的持久放行）共享，与「盲审只在独立目录看到选定材料」的隔离承诺不等价。建议池键加 blind 位或盲节点专用进程。

**3.5 密钥六铁律：本路径没有新增违规读取（不读凭据文件、只继承 + `AI_CROSS_PEER`/`PYTHONUTF8`、`raw_stdout/raw_stderr/error` 均过 `safe_trace`）**，唯一问题是 2.8 的 stderr 跨节点累积留痕，属「最小留痕」而非泄密。

## 4. 代码质量与契约

**4.1 `dispatch.rs:239`（`DispatchRecord.runner`）没有 `#[serde(default)]`。**
schema 名仍是 `aicross-dispatch/1`，所以第 3 期写的留痕（无 `runner`）用现结构反序列化会直接失败（`dispatch.rs:2216` 只验证了同版本 round-trip）。「允许追加字段」的前提是旧记录仍可读。
建议：`#[serde(default = "default_runner")]`（默认 `oneshot`）或 `Option<String>`。

**4.2 `runner` 到前端就断了。** `tauri.ts:475` 有字段，`graph/dispatch.ts:171-183` 的 `applyDispatchResults` 不写进 `Reply`，UI 唯一使用点是 `DispatchOverlay.tsx:218` 给本地失败节点硬编码 `'oneshot'`。用户/结果屏无法区分长驻与一次性（只有留痕里有）。建议至少在结果行或 G1 决策行显示「长驻/一次性」。

**4.3 与第 3 期 channels 的重复。** `acp.rs:277-287` 的 probe 与 `resources.rs` 的探测/`channels/mod.rs:378-410` 的 `resolve_binary` 各自一套；`process.rs:266-280` 的 `register/unregister_process_id` 与 `run_process_registered` 的 insert/remove 重复（同一份 `running_processes`）。建议收敛到一处。

**4.4 kimi 的「必须原生 exe」只在一次性路径生效。** `channels/mod.rs:129-131` 拒绝 `.cmd/.bat`，而 `acp.rs:84-85` 经 `resolve_binary` 拿到 `kimi.cmd` 后由 `process_command`（`process.rs:117-142`）用 `cmd.exe` 包装照跑。同一个二进制两条路径判断不一致（review-07 已记过同类漂移）。

**4.5 无释放入口。** 关掉设置里的 ACP 开关（`SettingsModal.tsx:521-548` 只改 localStorage）不会释放已建的长驻进程，也没有 `release` 命令；池只能靠「下次调用出错」被动清理。建议加一个显式释放（或与 3.3 的退出清理合并）。

**4.6 已核对无误**：`server.rs:3858` 只追加一行注册；`tauri.ts:375-376, 396, 399-405, 475` 与 Rust 的 camelCase 一致（`AcpAdapterStatus`、`acpEnabled`、`runner`）；`Reply`/`Target` 契约未动；`graph/dispatch-settings.ts` 的 `acpEnabled` 缺省 `false` 与后端 `#[serde(default)]` 一致。

## 5. 测试

**5.1 报告清单逐条核对。**
- 「ACP update 正文汇集」`acp.rs:326-343`：真存在，断言目标行为（忽略 `thought_chunk`）✓
- 「仅 Kimi 启用 ACP」`acp.rs:345-350` ✓
- 「一次性回归」`dispatch.rs:2223-2239`：**基本是空断言**。`node()` 的 agent 是 `claude`（第 3 期也不支持 ACP），`settings.acp_enabled=true` 时 `supports()` 为假，ACP 分支根本没进；两次运行都走同一条路，`assert_eq` 比较的是「两次同样的失败」。更糟的是：开发机若装了 `claude`，这个测试会**真发两次模型请求**（prompt 是「分析材料」，blind=true 还带 `--restricted`），同文件其它测试都用 `missing-agent` 规避真实调用。建议改成 stub 可执行文件 + 断言 `runner == "oneshot"` / 未调用 ACP。
- 「ACP 退出后回退」`dispatch.rs:2241-2244` /「取消后不回退」`2246-2250`：只是给纯函数 `resolve_acp_attempt` 手喂一个 `Err`，没有进程、没有留痕断言。验收项写的是「进程被杀后回退路径测试（单测）」，现在没有任何测试真的杀一个长驻进程再断言「下一次走一次性且留痕为 `runner: oneshot`」。
- 「长驻进程取消」`process.rs:437-455`：真存在，但只覆盖注册表 + taskkill。
- 「设置缺省和持久化」`dispatch-settings.test.ts:11-22`：两条真实、断言正确 ✓
- 完全没覆盖：`answer_permission`（唯一的工具权限控制）、`request` 的消息分发（id/未知 method/非 JSON 行）、进程复用、`runner: acp` 进留痕（grep 全仓只有 `runner: oneshot` 的断言）。

**5.2 ignored 真机测试 `acp.rs:352-387` 构造评估。**
构造方向对：直接调 `channels::acp::run`（不会被静默回退掩盖）、两轮同 `provider=""` → 池键一致 → 确实复用同一进程、断言 `session_id` 相等 + 第二轮含代号，能验到「同一 ACP session 续聊」。误判风险有三：
1. 只断言 session_id 相等与正文含「青松」，不看第一轮是否成功、session_id 是否非空；代号是可被复述/翻译的短词（建议用随机 nonce）。
2. model 传的是图里的别名 `kimi-code/k3-256k`；若 kimi 的 `currentModelId` 不是这个字符串就会走 `set_model`，而**它若静默忽略未知 modelId，测试仍会通过**，验的却不是声明的模型（ACP 不报真身，无人能发现）→ 至少打印/断言 `models` 返回。
3. cwd 用 `std::env::current_dir()`（仓库根），且测试结束不清理池 → 测试进程退出后 `kimi acp` 可能变孤儿（与 3.3 同因）。建议用临时目录 + 结束时显式释放。

**5.3 最该补的 3 个测试。**
1. **假 ACP server 端到端**（fixture 脚本/测试二进制，说 JSON-RPC；含一行 banner、一次 `session/request_permission`、可被 kill 的会话）：覆盖 initialize→session/new→prompt→update 两轮复用、非 JSON 行容忍、只读节点权限默认拒绝、杀进程后回退且留痕 `runner: oneshot`。现在这些验收点全靠纯函数与真机测试兜，等于没有。
2. **`answer_permission` 单测**：writable=true 时必须选最窄的 allow（不得选 `allow_always`）、`!writable` 时对写类请求拒绝/对读类放行。这是 ACP 路径上唯一的权限控制，零覆盖。
3. **留痕与回退契约测试**：ACP 成功 → `runner: acp`；ACP 失败回退 → 版本字段可被旧结构解析（补 4.1 的 default 后回归第 3 期留痕样本），并断言回退时不会把外来 session id 交给一次性 CLI。

---

## 对照 A 节是否完整实现

**一句话：核心链路（initialize/session/new/prompt 流式接收、按 agent/model 复用进程、`runner` 进留痕、kimi 优先、claude/codex 只探测、用量标 —）都落地了，但「异常退出回退后与第 3 期一致」「只读/盲权限等价」「进程生命周期清理」三条做不到，因此不算完整实现。**

缺项（按严重度）：
1. ACP 失败回退时把 ACP session id 交给一次性 CLI 且只发 delta prompt → 续聊语义被静默破坏（`dispatch.rs:1305-1320`、`acp.rs:119-121`）。
2. ACP 下只读/盲没有 `--tools`/`--no-tools` 等价物，且可能授出 `allow_always` 并跨节点生效（`acp.rs:241-262`）。
3. 应用退出/崩溃不清理 ACP 长驻进程（静态池 + 只关 PTY + Job Object 只绑 PTY）。
4. 一次轮级错误销毁整个进程池、跨 App 运行无法复用 ACP session（`acp.rs:76-80,119-121`）。
5. 取消覆盖不到「等池锁」的节点；一个节点的 cancel 会杀掉共享进程并触发重跑。
6. 取消/失败时不记录 `runner` 变化与回退原因，用户从留痕看不出发生过回退（`dispatch.rs:1284-1291`）。
7. ACP 回传的 usage 不读（有也标 —）；`session/update` 不按 session 过滤；非 JSON 行即致命；未知 agent→client 请求被挂起（`acp.rs:190-229,264-275`）。
8. 跨节点共享 stderr、stdout/raw 无界、stderr 4MB 后断管（`process.rs:88-113`、`acp.rs:196`）。
9. `DispatchRecord.runner` 无 `#[serde(default)]`，旧 `aicross-dispatch/1` 留痕按现结构读不回来（`dispatch.rs:239`）。
10. 测试缺口：无 ACP 端到端与权限测试；「一次性回归」是空断言且可能真跑 `claude`；`runner: acp` 无任何断言。
11. Claude/Codex 的 npm 安装命令包名无来源，与探测的二进制名不一致（`SettingsModal.tsx:539-540`）。

---
usage: {"input": 1226, "output": 5806, "cacheRead": 112256, "cacheWrite": 0, "reasoning": 282, "totalTokens": 119288, "cost": {"input": 0.00017164, "output": 0.00162568, "cacheRead": 0.0003143168, "cacheWrite": 0, "total": 0.0021116368}}
