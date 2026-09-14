# TASK-05A 修正轮 R1（ACP 长驻）

先读 `tasks/TASK-05-acp-and-polish.md` 第 0 节第 2 条与 A 节、`tasks/REPORT-05A.md`，再读 `tasks/review-05a-ds.md`（DeepSeek 盲审）与 `tasks/review-05a-glm.md`（GLM 盲审，若存在）。逐条处理，每条在 `tasks/REPORT-05A.md` 末尾「修正轮 R1」一节写「已修：怎么改的（文件:行）／已补：测试名」或「不采纳：原因」。验证：`cargo check` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`，命令与输出原文贴进报告。不起窗口、不发真机请求（ignored 真机测试留给编排者）、不 commit、不 push。

编排者已在沙箱外真跑 `channels::acp::tests::live_kimi_acp_two_turn_continuation`：1 passed，26.3s——协议链路是通的，本轮修的是回退语义、权限、生命周期与测试强度。

## 必修（按严重度）

1. **回退时不得把 ACP session id 交给一次性 CLI**（DS 1.1、2.1）：ACP 失败/进程不在/会话不属于当前进程时，丢弃 `session_id`，改用 `prepare_outbound` 已经准备好的**完整 prompt**（不是 delta），并在留痕 `error` 或新字段 `runner_note` 写「ACP 会话已重置，改一次性完整上下文」。补测试：给定带外来 session_id 的续聊节点 + ACP 不可用 → 一次性 argv 不含 `-S/-r`，prompt 含完整上下文。
2. **ACP 下的工具权限策略**（DS 3.1、3.2）：`answer_permission` 按节点的写权限与可见性判定——读类请求（read/grep/find/ls/list 等，按 `toolCall.kind`/`title`）一律放行，写/执行类在 `write=只读` 或盲节点时拒绝；**永不选 `allow_always`**，只选 `allow_once`（拒绝同理选 `reject_once`）。节点开始时把策略写进留痕（`runner_note`）。补测试：只读节点对写请求拒绝、对读请求放行；writable 节点永不返回 `allow_always`。
3. **进程生命周期**（DS 3.3、4.5）：`spawn_interactive` 的子进程也进 Windows Job Object（`windows_job::assign_pid`），`ExitRequested` 时清空 ACP 池（关 stdin + kill）；设置里关掉 ACP 开关时调用一个新命令 `release_acp_pool` 释放；ignored 真机测试末尾显式释放。补测试：池释放后进程不在（用假 server）。
4. **轮级错误不销毁整个池**（DS 2.2、2.3、2.4、2.5）：区分进程级错误（stdout EOF、写失败、超时）与轮级错误（空正文、坏 JSON 行、未知会话）；非 JSON 行忽略（写进 raw）；未知的 agent→client 请求回 `-32601` 错误而不是挂着；`session/update` 按 `sessionId` 过滤，只拼当前会话的 chunk。补测试（假 server 脚本）。
5. **取消**（DS 2.6）：等池锁的节点也能被取消（锁加超时 + 拿到锁后再查一次取消标记）；多路共用一个进程时取消当前节点发 `session/cancel`，不 `taskkill` 共享进程；只有进程级错误才杀。补测试。
6. **池键加隔离位**（DS 2.7、3.4）：盲节点不复用共享进程（池键含 `blind`），进程 cwd 与节点 workdir 一致；同一项目的共享节点才复用。
7. **usage 与真身**（DS 1.2、2.13）：解析 `session/prompt` 响应里的 `usage`（inputTokens/outputTokens）填进 `ChannelOutput`，没有才留 0；`set_model` 后校验 `models.availableModels` 含该 id，不含即报错回退，不静默。
8. **留痕**（DS 2.8、2.10、2.11、2.12、4.1）：`raw_stderr` 每轮清零、raw 缓冲加 8MB 上限并标截断；stderr 读满后继续排空不断管；回退时 `runner` 仍记实际方式并在 `error`/`runner_note` 写回退原因；`DispatchRecord.runner` 加 `#[serde(default = oneshot)]`，补一条「旧 aicross-dispatch/1 留痕（无 runner）能读回」的测试；status=ok 时的「kimi 不报用量」注记不要放在 `error` 字段（放 `runner_note` 或 meta）。
9. **前端**（DS 4.2）：`applyDispatchResults` 把 `runner` 写进 `Reply`（可选字段，Rust 镜像同步 `#[serde(default)]`），结果屏/G1 决策行显示「长驻/一次性」。
10. **测试补强**（DS 5）：① 写一个假 ACP server（`fixtures/acp/fake_server.py` 或 Rust 测试二进制：说 JSON-RPC，先打一行 banner，发一次 `session/request_permission`，支持被 kill）做端到端：两轮复用、非 JSON 行容忍、只读节点拒写、杀进程后回退且留痕 `runner: oneshot`；② 把「一次性回归」那条空断言（`dispatch.rs` 用 `claude` 的那条）改成 stub 可执行 + 断言 `runner == "oneshot"`，绝不能真发请求；③ `runner: acp` 至少一条断言。
11. **安装命令包名**（DS 1.3）：Claude/Codex 的 ACP 适配器 npm 包名没有来源，设置页只显示二进制名 +「需自行安装」，不写死未核实的 `npm install` 命令。
12. **口径**：REPORT 里把「流式」定义写死为「按 `session/update` 增量接收，界面不逐字显示」。

## GLM 补充

GLM 与 DeepSeek 在第 1–5、8、10 条上结论一致（回退 session id、cwd/盲隔离、退出清杀、失败零留痕、取消/排队、测试同义反复、非 JSON 行与未知请求）。GLM 额外的：

13. **超时不回退**（GLM 2）：ACP 跑满 timeout 后不再跑一次 oneshot（最坏双倍时长、`write != 只读` 节点可能重复写）；超时直接判 `timeout`，只有「进程异常退出/协议错误」才回退。排队等锁的时间计入节点 deadline。补测试。
14. **续聊时模型变更**（GLM 2）：同一 session 中途 `target.model` 变了要重新 `set_model` 并校验，不静默沿用旧模型。
15. `initialize` 超时从写死 5s 改为复用节点 timeout（上限 30s），冷启动慢盘不误回退。
16. `probe` 也传 `kimi_fallback()`（`~/.kimi-code/bin`），设置页探测结论与派发一致；探测结果加「重新探测」按钮，失败不吞成空数组。
17. 新增 `acp::shutdown_all()`，`ExitRequested` 与关闭 ACP 开关都调用它；第 3 条的测试用它。
18. 设置页文案写明：ACP 下只读节点会拒绝写/执行类权限请求（一次性 kimi 无此限制），读类请求放行。
19. `AcpOutput` 直接产出 `ChannelOutput`（含 usage），去掉 `run_one` 里的逐字段搬运。
