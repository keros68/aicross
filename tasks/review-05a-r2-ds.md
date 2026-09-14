## 一、20 条逐条核对

**第 1 条（超时分类）— 改法有问题**
分流本身对了：`process.rs:35-44` 把 `RecvTimeoutError` 类型化，`acp.rs:443-450` 只在 `Instant::now() < deadline` 时 `continue`，到 deadline 返回 `AcpErrorKind::Timeout`（`permits_fallback()` 不含 Timeout，`acp.rs:35-42`）。但第 1 条同时要求「断言结果 status=timeout、**runner 为 acp**、没有第二次一次性调用」，而 `run_one` 的 `AcpResolution::Failed` 分支（`dispatch.rs:1370-1379`）只改 status/error，**不改 runner**，`runner` 仍是 `dispatch.rs:1314` 的初值 `"oneshot"` → 超时的 ACP 节点留痕写 `runner: oneshot`、`runner_note` 为空、结果卡显示「一次性」。要求的 `run_one` 级测试不存在（现测试 `acp.rs:860` 只验 `AcpClient::prompt` 的 Err 类型）。

**第 2 条（append_bounded panic）— 已核实修好**：`acp.rs:557-570` 用 `floor_char_boundary` 双向截断，`acp.rs:868` CJK 断言不 panic 且 `ends_with` 标记。

**第 3 条（杀 ACP → 回退 → 留痕 oneshot 端到端）— 改法有问题**
回退逻辑本身正确：Process→`permits_fallback`→`dispatch.rs:1366-1369` `reset_acp_fallback`→`onshot_session=""`、`needs_oneshot` 保持 true→`dispatch.rs:1385` 起一次性且无 `-S/-r`。四条同义反复测试也确已改为真实路径（`acp.rs:819` 真 `AcpClient`、`:836` 真取消、`:847` 真 `acquire_slot`、`:829` 真 state）。**但要求的 run_one 端到端（第一轮 ACP 成功→杀进程→第二轮断言 `runner: oneshot` + `runner_note` + argv + 完整上下文）不存在**：全仓只有 `dispatch.rs:1331` 一处调用 `acp::run`，dispatch 测试里没有一条走 ACP 分支（`dispatch.rs:2445` 用 cc-switch，`supports()==false`，acpEnabled=true 那半根本没进 ACP）。所以「杀进程后留痕 runner: oneshot」在派发层仍是零覆盖。

**第 4 条（shutdown_all 不阻塞）— 已核实修好**：`acp.rs:586-604` 先 `mem::take` 摘出整张 map（guard 在 `acp.rs:587` 语句末即释放），逐 slot `try_lock`，失败走 `pid` + `terminate_process_id`；测试 `acp.rs:899`。注意 ≤1s 只在 busy 分支被断言（成功分支里 `close_all_sessions` 每 session 最多 100ms，未断言）。

**第 5 条（迟到 update）— 已核实修好**：`acp.rs:387-396` 每轮 prompt 前 drain，取消/超时 `acp.rs:426-433`、`443-450` 都 `close_session` 并 `sessions.remove`（`acp.rs:534`）；测试 `acp.rs:808` 断言 `LATE` 不进正文。

**第 6 条（stdout 8MB）— 已核实修好**：`process.rs:118-138` 字节计数、只入队一次 `[ACP stdout 已截断]`、之后继续排空（`for lines()` 不 break）；测试 `process.rs:468`。残留见第四节 D。

**第 7 条（kind 优先）— 已核实修好**：`acp.rs:538-556` kind 白名单（read/search/list/fetch）→读，(edit/write/execute/delete/move)→写，未知非空 kind 当写，仅 kind 缺失看 title。title 绕过已被端到端否证：假 server 的 execute 请求 title 是 `Read misleading title`，只读节点的回复 `turn-1:no`（`acp.rs:790`）。

**第 8 条（旧 ACP session 不进 -S）— 已核实修好（有残留）**：前端 `dispatch.ts:109-114` + `DispatchOverlay.tsx:125` 传 `previousRunner`，后端 `dispatch.rs:1326` 关闭开关且父 runner=acp 时清 session + 完整上下文 + note；测试 `dispatch.test.ts:132`、`dispatch.rs:2493`。残留：ACP 开着但该节点 agent 不是 kimi（走一次性）时仍会把旧 ACP id 交给 `-S/--resume`，而 `docs/PRODUCT.md:184` 的措辞比代码严。

**第 9 条 — 已核实修好**：`DispatchOverlay.tsx:220` 用 `runner: 'unknown'`，`:423`/`:450` 显示「未运行」；测试 `dispatch.test.ts:169`。

**第 10 条 — 已核实修好（断言不全）**：`acp.rs:307` `cachedReadTokens` 优先、`or_else(cacheReadTokens)`；测试只断言 `cache_read == 5`（`acp.rs:790` 附近），旧键分支无断言（与报告自报的「兼容旧 cacheReadTokens」不完全对应）。

**第 11 条 — 已核实修好**：`acp.rs:681-694` python 缺失 eprintln+return；`acp.rs:877-897` tasklist CSV PID 精确匹配 + 2s 轮询，无 100ms sleep。

**第 12 条 — 已核实修好（一条断言有误判风险）**：`acp.rs:943-990` 随机 nonce、首轮 session 非空、次轮含 nonce、末尾 `shutdown_all`。`assert!(first.channel.usage.input > 0)` 见第四节 A。

**第 13 条（模型清单）— 已核实修好**：`acp.rs:336-374` 先 `configOptions`（category==model 或 id==model）校验 `options[].value`，切模型先 `session/set_config_option{sessionId,configId:"model",value}`（`acp.rs:349`），失败才退 `session/set_model`（`:357`）；两清单皆缺只 `notes.push("ACP 未暴露模型清单")` 不校验不回退（`:364`）；假 server 两种形状（`fake_server.py:22-31` + `--legacy-models/--no-model-list`），测试 `acp.rs:770/777/785`。R1 真机炸点（`kimi-code/k3-256k` 不在 availableModels）在 configOptions 形状下已不再触发。

**第 14 条 — 已核实修好**：`acp.rs:376-379` `!writable || blind → plan`，否则 `default`，字面量里没有 auto/yolo；测试 `acp.rs:785/792`。

**第 15 条 — 已核实修好**：`acp.rs:380-383` 关/低→low、中/高→high，失败只 push note。

**第 16 条 — 已核实修好（与字面措辞有偏差）**：能力探测 `acp.rs:205-210`；取消/超时关会话 `acp.rs:426-433/443-450/528-535`；池摘除与 shutdown 逐个 close 再终止 `acp.rs:398-408/586-604`。成功轮次刻意不关（否则续聊失效），因此不是字面的「节点结束即 close」；长驻进程上成功轮次的 session 会累积到池摘除/shutdown 为止。

**第 17 条 — 已核实修好**：`fake_server.py:21-24` `fake-session-{递增}`，`:53-54` 未知请求回包必须是 `-32601` 否则自爆；两轮复用断言 `acp.rs:790`（若重开 session 会是 `-2`，断言会失败）。

**第 18 条 — 已核实修好**：`acp.rs:559-561` 标记即终态；initialize 窗口的 banner 进 `prelude_raw`（`acp.rs:190`）并在首轮 prompt drain 前落在 raw 里，测试断言 `raw_stdout.contains("fake-acp banner")`。

**第 19 条 — 改法有问题**：代码改法对（`acp.rs:124-137` 先占位空 slot，`acquire_slot` 拿到锁后才 spawn，第二个线程只会复用）。**测试不符**：`acp.rs:924` 不调用 `run`，只在本地 `Arc<ClientSlot>` 上重演一遍 `Mutex` 语义，`starts == 1` 对任何加锁初始化都成立；python 缺失时 `fake_client()` 返回 None 也照样「通过」——既是同义反复也是空断言。

**第 20 条 — 已核实修好**：`docs/PRODUCT.md:184` 写明跨 runner 限制与「新开会话」恢复（但见第 8 条残留）。

## 二、是否引入新缺陷

- **第 3/4 期一次性路径**：无回归。`acp_enabled`/`previous_runner`/`runner_note` 全部 `#[serde(default…)]`（`dispatch.rs:37`、`102`、`249`），旧设置与旧前端仍走一次性；`dispatch.rs:1326` 的强制重置只在「父 runner=acp 且 ACP 已关」时触发。`dispatch.rs:2445` 的回归测试实际只覆盖「ACP 不支持该 agent」方向。
- **留痕契约 `aicross-dispatch/1`**：只追加（`runner`、`runner_note`），旧记录可读回（`dispatch.rs:2437`、`frontmatter_roundtrips`），`serialize_frontmatter` 仍是整串重写、无字段改名。R2 未新增字段，但 REPORT 的「契约追加」一节没补 `runner_note`/`previousRunner`/`runner:'unknown'`（文书滞后，非代码问题）。
- **Reply 契约只追加**：`orchestration.rs:88` `Option<String>` + `#[serde(default)]`，前端只在有值时写（`dispatch.ts` `applyDispatchResults`），`dispatch.test.ts:148` 的 `toEqual` 仍通过（无 runner 键）。
- **注册与类型镜像**：`server.rs:4243-4244` 注册 `detect_acp_adapters`/`release_acp_pool`，`server.rs:4397` 退出调 `shutdown_all`；`tauri.ts:375-377/397/439/477-478` 与 Rust 侧一一对应，`AcpAdapterStatus` 字段镜像一致；`SettingsModal.tsx:208` 关闭开关即 `releaseAcpPool`。
- **R1 三处名不副实**：REPORT R1 第 5/7/14 条已按实际断言改写，且改写里点名的 R2 测试（`queued_..._waiting_for_client_lock`、`continuation_model_change_...`、cache_read 断言）都真实存在。
- 新增的次级问题：`cancelled_inflight_acp_prompt_returns_cancelled_and_closes_session`（`acp.rs:836`）名字里的「closes session」并未断言；`dispatch.rs:1372` 用 `error.contains("超时")` 字符串判 timeout（ACP 协议错误里含「超时」会误标 status=timeout）。

## 三、会导致误判的断言 / 真机会炸的路径

- **A（会误判，优先级最高）**：`acp.rs:943` 附近 `assert!(first.channel.usage.input > 0)`。kimi 一次性路径已知「不报用量」（REPORT-03 记录），编排者探针只验过 `session/new`，没验 `session/prompt` 的 result。若 ACP 也不带 usage，两轮对话本身成功却因这条断言判失败。建议降级为 `eprintln!` 记录，不作为硬断言。
- **B（真机最可能炸）**：`acp.rs:202-214 append_update_text` 只认 `agent_message_chunk|agent_message` + `content.text`。R1 真机在模型校验处就挂了，这条路径从未在 kimi 上跑过；若 0.42 的 chunk 形状不同，正文为空 → `AcpErrorKind::Turn` → 不回退 → 节点直接失败（status=error）。复跑时请贴出留痕 raw_stdout 以确认形状。
- **C**：超时现在会 `run` 级摘池（`acp.rs:151-165`），同进程其他在飞节点随之变 Process→一次性；这是 R2 分级后的必然行为，不是缺陷，但会放大超时的波及面。
- **D**：`process.rs:119-137` 的 8MB 是**进程生命周期累计**，不是每轮水位。长驻会话累计输出破 8MB 后所有后续行（含下一轮响应）永久被丢 → 下一轮必然等到 deadline 超时（随后因超时摘池、重开进程自愈）。建议改为按未消费水位/每轮重置。
- **E**：`acp.rs:877/899/910` 三条 shutdown 测试共用全局池并互相 drain；分析下来各自仍能杀掉自己的 pid，但 `shutdown_terminates_fake_acp_process` 若偶发失败，先怀疑这里。
- **F**：`dispatch.ts:97-114` `continuationSession` 与 `continuationRunner` 判定条件不一致（盲父节点时 runner 有值、session 无值），会给一次性节点写一条不实的「旧 ACP 会话已重置」note；无功能副作用。

## 四、结论

**R2 之后 TASK-05A 尚未达到可交付**——kimi ACP 协议事实（13–16 条）改得对、R1 尾巴大部分收干净，但第 1/3 条要求的 `run_one` 端到端断言缺席，连带暴露出真实缺陷：ACP 超时/失败/取消节点的留痕与结果卡把 `runner` 记成 `oneshot`（实际没跑一次性），且真机 chunk 形状与 `usage.input>0` 断言仍可能造成误判。

仍未闭合项（按严重度）：
1. ACP 超时/失败/取消路径的 `runner` 标签错误：应记 `acp`（`dispatch.rs:1362-1379`，第 1 条明确要求）。
2. 第 1、3 条要求的 `run_one` 端到端断言不存在（超时 → status=timeout/runner=acp/无第二次一次性；杀 ACP → 第二轮 `runner: oneshot` + runner_note + argv 无 -S/-r + 完整上下文）。
3. 真机未验证的 `session/update` 形状（`acp.rs:202-214`）；形状不符时 Turn 错误不回退、节点直接失败。
4. live 测试 `usage.input > 0` 硬断言可能误判（`acp.rs:943`）。
5. 交互 stdout 8MB 上限按进程生命周期累计，长驻会话会静默丢响应（`process.rs:119-137`）。
6. 第 19 条测试是同义反复/空断言（`acp.rs:924`），`run` 仍无任何非 ignored 覆盖。
7. 跨 runner 脏 session id 在「ACP 开着但当前节点 agent 非 kimi」时仍会进 `-S/--resume`（`dispatch.rs:1326`；`docs/PRODUCT.md:184` 的措辞宽于代码）。
8. 第 10 条旧键 `cacheReadTokens` 无断言；`acp.rs:836` 测试名与断言不符；`dispatch.rs:1372` 用字符串判超时；REPORT「契约追加」一节未同步 R1/R2 追加的 `runner_note`/`previousRunner`/`runner:'unknown'`。