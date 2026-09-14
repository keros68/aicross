## 逐条核对（对照 `tasks/fix-05a-r3.md` 六条）

### 1. ACP 超时/失败/取消不得记 oneshot — **已核实修好**
- `src/dispatch.rs:1319-1323`：`AcpResolution::Cancelled` → `runner="acp"`、`runner_note="ACP 已取消，未回退"`、`needs_oneshot=false`。
- `src/dispatch.rs:1329-1342`：`Failed` → `timed_out = acp_error.kind == AcpErrorKind::Timeout`（按类型，不匹配文本）、`runner="acp"`、note 分「ACP 超时，未回退」/「ACP 失败，未回退」。
- 留痕取同一变量：`src/dispatch.rs:1483-1484`；结果卡用 `result.runner==='acp'` 显示「长驻」（`DispatchOverlay.tsx:423`）；全仓 grep 无 `"unknown"` 字面量，后端不冒充 `unknown`。三条单测 `dispatch.rs:2360/2378/2400` 与实测一致（`Failed` 只覆盖 Turn+Timeout，Process/Protocol/ForeignSession 才走 Fallback，语义正确）。

### 2. 两条 `run_one` 端到端 — **已核实修好（两处小瑕疵）**
- 真驱动：`dispatch.rs:2360-2375`、`2418-2443` 直接调 `run_one`，fixture 同时注入假 ACP server 与可计数 stub（`dispatch.rs:2341-2358`；`channels/mod.rs:111-121`、`acp.rs:189-196`）。
- (a) 超时：断言 `status=timeout / runner=acp / note 含「ACP 超时，未回退」 / !count.exists()`（stub 执行才追加计数，缺失=零调用），并覆盖「一次性路径不被调用」。
- (b) 死进程：首轮 `runner=acp` → 次轮 `runner=oneshot`、note 含「ACP 会话已重置」、计数文件 1 行、`raw_stdout` 无 `'-S'`/`'-r'`、task file == 完整提示词 `dispatch.rs:2438-2441`。
- 生产路径未被改坏：两处注入都在 `#[cfg(test)]` 下（`channels/mod.rs:111`、`acp.rs:190`），发布编译剔除；ACP 池键含 provider，测试 provider 不可能与真实键相撞。
- 瑕疵：python 缺失时静默跳过（`dispatch.rs:2361` 一类），本轮「已验证」依赖运行环境；死进程用例断言的是结果字段 `runner`，frontmatter 文本形态只由 `dispatch.rs:2330` 的 cc-switch 回归覆盖。

### 3. 8MB 按未消费水位 — **已核实修好（一个边界）**
- `src/process.rs:23/130-131` 原子水位；`138-141` CAS 预留（`current+item ≤ MAX` 才入队），`45`/`58` 在 `recv_line`/`try_recv_line` 归还；只有已预留的行会被 send，故不会下溢，消费与预留无竞态。
- 超限只丢该行 + 每段一次截断标记（`148-157`），有行入队即复位 `marked_truncated`（`143`）→ 可恢复。
- `acp.rs:903-911` + `fake_server.py:14/70-71` 两轮各 5MB：断言两轮正文都到、第二轮 `raw_stdout` 无 `[ACP stdout 已截断]`。旧「累计一生」实现下 5+5>8 会丢第二轮 update 行，故该用例有区分力。
- 边界：队列恰好满 8MB 时，截断标记自身也过不了尺寸检查（`process.rs:148-157`）→ 这一角下丢行不留任何痕迹（消费端通常活跃，且丢响应最终变超时），低严重度。

### 4. 第 19 条驱动真实路径 — **已核实修好**
- `acp.rs:966-985`：`thread::scope` 两线程同 key 同时 `run()`；假 server 启动即向 `--start-file` 追加（`fake_server.py:16-20`）；断言行数 == 1。手写 `slot.client.lock()` 的复刻版已删（grep 只剩 R2 报告文本）。
- 瑕疵：成功路径不进池摘除，测试结束后池里留一个活 python（下次 `shutdown_all` 或测试进程退出时清），属测试卫生，非产品缺陷；`dispatch.rs:2378` 取消用例同理。

### 5. 跨 runner 清理另一半 + 前端对齐 — **已核实修好（盲父语义需澄清）**
- `dispatch.rs:1282-1286`：条件已是「本轮实际不走 ACP」= `!uses_acp && previous_runner=="acp"`，`uses_acp = acp_enabled && supports(agent)`，覆盖「开关开着但 agent 非 kimi」；`dispatch.rs:2446-2476` 用 cc-switch + `acp_enabled=true` 断言 session 清空、note、`--task-file` 完整上下文、argv 无旧 session。
- `docs/PRODUCT.md:184` 措辞与代码一致（含「当前 agent 不支持 ACP」）。
- 前端：`dispatch.ts:109-114` `continuationRunner` 直接复用 `continuationSession`，两者严格同前提；`DispatchOverlay.tsx:124-125` 同源同 `restartNodes` 守卫；`dispatch.test.ts:139-146` 断言盲节点两者皆空。
- 澄清：判定看的是**被派发节点**的 visibility，盲**父**节点的 session/runner 仍会被同目标的共享子节点带出（`dispatch.ts:99` 只查子节点）。后果被兜住（池键含 blind → ForeignSession → Fallback 重置；一次性路径续的是共享节点本来就能看到的内容），但「盲父节点两者都为空」这层语义既未实现也无测试。

### 6. 小尾巴 — **已核实修好（两项仅「够用」）**
- a) `acp.rs:898-901` 直接喂旧 JSON（题目允许此路）；但为它加的 fixture 开关 `--legacy-cache-usage`（`fake_server.py:15/72`）无任何调用者，成了死代码。
- b) `acp.rs:852` 用例已加 `assert!(client.sessions.is_empty())`，名实相符。
- c) `dispatch.rs:1330` 按类型判定；grep 无残留 `contains("超时")` 式分流（`dispatch.rs:1094` 是验证命令超时，无关）。
- d) `REPORT-05A.md:20-25`「契约追加」已含 `runner_note`、`previousRunner`、`runner:'unknown'`。
- e) 对齐见第 5 条；盲父语义同上。

## 编排者三项改动的副作用核查
- **mode 固定 default**（`acp.rs:373-376`，注释 `371-372`）：全仓无残留 plan/yolo 期望（含 `SettingsModal.tsx:548` 文案）。只读保护现在**只剩权限门**（`acp.rs:500-522`：读类 allow_once，写/执行或未知 kind → reject_once）；`is_read_request` 把 `fetch` 当读、且 kind 缺失时回退标题关键词（`acp.rs:529-547`）——写类工具若省略 kind 且标题含 read 会被放行，plan 模式这条第二道防线已不存在（该门是 R2 既有设计，非本轮新引入，但严重度因改口上升）。
- **live 测试不断言 usage**（`acp.rs:1008-1014`）：仍断言 session 非空、nonce 续聊、同 session、末尾 `shutdown_all`；kimi 只回 `stopReason` 的解释已写进注释与报告，属如实降级。
- **AICROSS_ACP_DEBUG**（`acp.rs:422/456`）：`eprintln!` 在父进程侧，**不会**落入 `raw_stderr`（那是子进程 stderr，经 `take_stderr_text`）或留痕；默认关闭，300 字节截断做了 char boundary。唯一小点：开启时协议行（含模型正文）未经 `safe_trace` 直出控制台——调试开关的预期行为。

## 是否引入新缺陷
- 一次性路径：无回归——新钩子全部 `#[cfg(test)]`（`channels/mod.rs:111/149`、`acp.rs:190`），`run_one` 只改标签与重置条件；`acp_disabled_or_unsupported_preserves_oneshot_dispatch_result`（`dispatch.rs:2300-2335`）等仍过。
- 留痕契约 `aicross-dispatch/1`：只追加——`runner` 有 `#[serde(default="oneshot")]`、`runner_note` `#[serde(default)]`（`dispatch.rs:246-249`），旧记录可反序列化（`2288-2293`），roundtrip（`2278-2282`）通过，schema 名与既有键未动。
- Reply 契约：只追加——`types.ts:22` 可选 `runner`，`dispatch.ts:184` 条件展开，`redraftNode` 会清掉旧 runner，旧图不受影响。
- 注册与镜像：`server.rs:3858-3859` 登记 `detect_acp_adapters`/`release_acp_pool`；`tauri.ts:439`（请求 `previousRunner`）与 `477-478`（结果 `runner`/`runnerNote?`）与 Rust camelCase 镜像一致。
- 文档漂移（低）：R2 节仍引用 R3 已删除的 `interactive_stdout_queue_stops_after_eight_megabytes_and_marks_truncation`（`REPORT-05A.md:193`）与 `same_key_concurrent_initialization_spawns_one_fake_process`（`:206`）；8MB 语义从「终生截断」改为「按水位可恢复」这一契约变化只在 R3 测试名里体现；R3 节写「本轮未运行 ignored 真机测试」，而编排者实际跑了 live（26.30s / 你给的 18.6s，两数未对齐）。

## R3 之后 TASK-05A 是否达到可交付

**达到可交付**——6 条必修全部核实修好，剩余问题均为低严重度边界/记录类，不阻塞验收。

仍未闭合项（按严重度）：
1. 只读/盲节点的写保护只剩权限门这一层：`fetch` 视为读类、kind 缺失时按标题放行（`acp.rs:529-547`），plan 模式这道防线已被移除——建议下轮把 `fetch`/未知 kind 收紧为拒绝。
2. `--legacy-cache-usage` fixture 开关无调用者（`fake_server.py:15/72`），旧 usage 形只由直接喂 JSON 的单测覆盖。
3. 盲**父**节点 → 共享子节点仍会继承 session/runner（`dispatch.ts:99` 只查子节点），靠池键+ForeignSession 兜住，无测试。
4. 队列恰满 8MB 时截断标记自身也无法入队，丢行不留痕（`process.rs:148-157`）。
5. 新 `run_one` 用例 python 缺失即静默跳过；取消/并发用例在池中留活进程（测试卫生）。
6. 报告漂移：R2 节引用已删除的测试名、8MB 契约变化未落文、R3 节未记录编排者的 live 复跑（时长数字两处不一致）。
7. `AICROSS_ACP_DEBUG` 开启时协议行未经 `safe_trace` 直出控制台（仅调试态）。
8. 真机证据仍全压在 `live_kimi_acp_two_turn_continuation`（ignored）上，本环境无法复跑，只能凭报告采信。