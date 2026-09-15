审查完毕。以下为对工作区当前状态（9 组修正）的逐条核对结果。

说明两点事实前提：① 本会话无 shell 工具，无法执行 git diff / cargo test / vitest，REPORT 中的验证数字按报告采信、未复跑；② 审查进行中工作区仍在变动——`console-channel.ts` 在我两次读取之间新增了 `mergeConsoleCommit`（标注「R1 复审 1」），下述结论以最后一次读取的文件内容为准。

---

## 总判：可交付但有未闭合项

薄中继机制本身属实且两条盲审的高危项全部消除；但新机制里有一个功能性缺口：**执行期间的在飞清单被 commit 回报清空，导致 `aicross cancel` 在真正的执行期基本取消不掉、且「N 路在飞」显示恒为 0**（新发现 N1），另有若干低severity 项。

---

## 9 组逐条

**1. Rust 改薄中继 —— 已修（取消的实际效果见 N1）**
- `run_prepared` / `build_node_request` / `save_graph` / `DispatchSettings::default()` 在 `src/console_api.rs` 全文零出现；全仓 grep 仅剩 `src/dispatch.rs` 引擎自身及其测试命中；v1 的 `orchestration::adopt_graph_node` 已删（全仓 0 命中）。
- HTTP → 事件 → 回执：`relay_dispatch` 校验文本/项目目录 → 插入 running 摘要 → `emit("aicross:console-dispatch")` → 5 秒 `recv_timeout`（`src/console_api.rs:233-258`）；emit 失败或超时 → `reject_run` + 503（:246-257）；前端 `status:'rejected'` 回执经 `apply_run_update` 转成 HTTP 409+原因（:302-320）。
- 同一启动函数：`startPipelineHeadless`（`src-ui/src/graph/dispatch.ts:1444`）被 `DispatchOverlay.start`（`DispatchOverlay.tsx:316`）与 `runConsoleDispatch`（`console-channel.ts:162`）共同调用；测试钉住接线（`console-channel.test.ts:48-52`）。
- 图选择：`newGraph` 把 `project` 写在 Graph 上（`src-ui/src/graph/types.ts:109,174-194`，根节点不再带）；旧图按 tab `folderPath` 匹配、大小写/斜杠归一、active tab 优先（`console-channel.ts:33-44`）。
- 采纳/取消转发：`relay_action` + `console_action_ack` 往返（`console_api.rs:260-272,329-336`）；结果卡采纳回到前端 `setReplyAdopted`（`DispatchOverlay.tsx:766`），仅蜂群 `pickOne` 保持原样（:761）——首版 Rust 采纳入口确已撤掉。

**2. 前端通道 —— 已修**
- 事件处理、图选择、`stamp()`（与 `GraphTab.tsx:372` 同一函数、同参来源 `loadDispatchSettings().swarmLimit`）、`console_run_update` 回报（每阶段 commit / onStage / 终态，`console-channel.ts:141,175,185`）、adopt/cancel 转发（:188-207）。
- 总指挥 tab 前台不抢焦点保留（`consoleTabPlan` :18-24 + `CenterPanel.tsx:509-521`，`graph_id` 为空时不开 tab :509）。
- prompt 不再另造：`buildRequest` 与 DispatchOverlay 同构（`serializeContext`/`buildNodeRequest`/`originalRequestOf`/`upstreamArtifactsOf`/`continuation*` 同一来源，`console-channel.ts:150-160` 对照 `DispatchOverlay.tsx:183-215`）。

**3. HTTP 与清单安全 —— 已修**
- 常量时间比较：XOR 累计含长度差（`console_api.rs:186-195`）。
- Origin / `Sec-Fetch-Site: cross-site` → 403，且在鉴权与读 body 之前（`hook_server.rs:147-149,195-203`）；401 也在读 body 前（body 只在 else 分支读，:150-172）。
- `HEAD`/`OPTIONS` 进 HTTP 分支、不落 JSON 行、不回 `{}` 行、不刷日志（`hook_server.rs:104-107`，`handle_http` 无日志）。
- 清单失败只告警「控制台通道不可用」，hook server 照常启动（`hook_server.rs:60-62`；`server.rs:4003-4012` 仅记录端口）。
- `%SystemRoot%` 定位 whoami/icacls（`console_api.rs:410`）；ACL 时序改为空临时文件先收紧、写正文、`MoveFileExW` 原子替换（:353-371,378-387），并有注入式顺序断言（测试 :562-576：harden 时文件非空即失败）。

**4. CLI —— 已修**
- `--timeout` 默认 600、可覆盖、0 拒绝，到点退出 1（`console_cli.rs:122-124,72-75`）。
- 每轮循环重读清单（:37-48）；pid 判活（`OpenProcess`/`kill 0`，:172-185），死进程删清单并报「控制台未运行（清单已过期）」退出 3（:40-43,165-168）。
- dev 用 `console.dev.json`，写/读/删三处同源（`console_api.rs:340-342` + `console_cli.rs:157`）。
- BOM 剥离、非 UTF-8 明确报错（:105-107）；`--text` 与 `--text=` 都按用法错误退出 2（:100-102）；参数解析先于清单检查（:28 vs :38）。

**5. 安装包 PATH —— 已修（NSIS 本体无自动化测试，见 N5 备注）**
- 两段 `-EncodedCommand` 解码核对（UTF-16LE PowerShell）：读值用 `DoNotExpandEnvironmentNames`（保留 `%VAR%`）、回写用 `GetValueKind` 原类型（保 `REG_EXPAND_SZ`/`REG_SZ`）、匹配 `TrimEnd('\')+OrdinalIgnoreCase`（大小写+尾反斜杠）、安装时 Path 缺失只写非空本项、卸载仅在确有删除（`$b.Count -ne $a.Count`）时回写、`WM_SETTINGCHANGE` 仅 exit 10 时广播；安装目录经环境变量传递避免引号注入（`installer.nsi:683-690,777-785`）。
- WiX 原有 item-level `Environment` 未动（`windows/fragments/path.wxs:6-14`），语义同口径。
- 纯函数测试存在（`console-channel.test.ts:39-46`，`console-channel.ts:217-229`）——但测的是 TS 镜像，不是 NSIS 脚本本体，NSIS 正确性只靠本次人工解码。

**6. 契约与文档 —— 已修**
- `rejected` 状态（`console_api.rs:296-300`；TS 联合类型 `tauri.ts:646`）；leftover 真填自遗留节点（`console-channel.ts:141,185`）；节点状态词原样保留 + `quota_resets_at`（:62-73；`ConsoleRunNode.quota_resets_at` `console_api.rs:58`）。
- `docs/CONSOLE-CHANNEL.md` 四表齐全且逐条对得上：状态码 400/401/403/404/409/413/503（:90-100）、退出码 0/1/2/3（:132-137）、预设别名（:116-123，与 `console_cli.rs:109-114` 一致）、平台 PATH（:139）、5 秒 503（:57）、30 秒 HTTP 超时/`--timeout`/重读清单/200 非 JSON（:130,114）。

**7. 在飞上限与 G1 —— 已修**
- Rust 硬上限 3、超限 409（`console_api.rs:20,284-293`，测试 `fourth_inflight_run_is_rejected`）；前端设置 1–3 默认 3（`dispatch-settings.ts:55-57`，`SettingsModal.tsx:635-643`）。
- G1 判定走界面同一套函数：缺目标、`validateNode` 的未接入/只读、verification=failed、`quotaBlockedNodeIds`、`maxRequests`、`dirtyWorktreeHintOf`（`console-channel.ts:110-129`），拒绝原因写进 run 摘要；「未验证」不放行拦截、交引擎冒烟，与既有口径一致。`dispatchableNodeIds` 本身不预筛这些状态（`dispatch.ts:94-107`），前置检查能真拦到。

**8. 测试整理 —— 已修（两处近似同义反复残留，见 N5）**
- Rust 侧有假前端回执往返（测试线程内调 `apply_run_update`）、rejected 409、200 容量淘汰、在飞 409、额度保留、终态 cancel 不转发、超时 503、清单权限顺序、401 先于路由、404 先于 emit（`console_api.rs:454-576`）；CLI 侧 stale pid/超时/BOM/`--text=`/200 非 JSON/退出码 3 单行（`console_cli.rs:231-332`）；前端有共享函数行为测试（`console-channel.test.ts:54-78`）与接线断言（:48-52）、PATH 纯函数（:39-46）、mergeConsoleCommit（:81-94）。v1 的源码字符串断言大部分已删。

**9. GLM 补充 —— 已修**
- cancel 对已结束 run 不转发不改状态（`should_relay_cancel`，`console_api.rs:142-150`，测试 :541-547）；200 非 JSON 退出 1 并打首行（`console_cli.rs:60-64,222-224`）；HTTP 总超时 30s（:10,203）；ACL 先收紧后写正文（见第 3 组）；额度保留原状态词与恢复时间（测试 `quota_status_and_reset_time_are_preserved` :529-539）；NSIS 卸载条件回写（见第 5 组）。

**回归面（问题 6）**：`runStageLoop` 循环体原样保留重复批次护栏、上限、缺目标、修正轮（`reducePipelineStep`+`validationRetries`+`fixRoundLimit`，`dispatch.ts:1413-1418`）；TASK-12 测试仍直接驱动 `runStageLoop`（`review-defaults-12.test.ts:236-252`）；红绿/验证命令在引擎侧，本轮未触碰引擎派发路径；07C 的 finally 接线断言与实现匹配（`dispatch.ts:1461-1463`）。未发现抽函数造成的 DispatchOverlay 行为分叉（deps 一一对应，begin/end 由 startPipelineHeadless 调用）。

---

## 新发现的问题

**N1【中高】commit 回报把在飞清单清空：执行期 in_flight 恒为空，cancel 基本不生效、在飞计数恒 0**
- 位置：`src-ui/src/console-channel.ts:141`（commit 回报 `in_flight_node_ids: []`）对照 ：175（onStage 回报真实列表）；循环顺序 `src-ui/src/graph/dispatch.ts:1396-1398`（onStage → `commit(markPending)` → `dispatch` 长执行）；Rust 整字段覆盖 `console_api.rs:302-320`。
- 复现：派一个长任务 → `GET /v1/runs/<id>`：onStage 瞬间可见 in_flight，随后整个模型执行期间都是 `[]`，节点状态是 `pending`（`markPending` `dispatch.ts:205-211`，`nodeSummary` 原样上报）。此时 `aicross cancel <id>`：前端读到 `in_flight=[]`，`cancelDispatchNode` 一个都不调（`console-channel.ts:200`），仅把摘要置 failed「派发已取消」；流水线继续跑，下一次 commit 又把状态盖回 running（:141），最终照常出结果——取消返回成功但什么都没取消，状态还反复。同时总指挥 tab「N 路在飞」与任务板行数在整个执行期显示 0（`console-channel.ts:26-29`；`TaskBoard.tsx:629-630` 数 `status==='running'`，而摘要里永远是 `pending`）。
- 建议：commit 回报携带当前活动阶段节点（把 ids 传入 commit，或 Rust 端仅终态更新清空 in_flight）；或取消改为对 `run.graph_id` 图上 `reply.status==='pending'` 的节点逐个 `cancelDispatchNode`（UI 的叫停不依赖摘要，故不受影响）。

**N2【低中】前端超过 5 秒才首次回执时产生 ghost run**
- 位置：`console-channel.ts:90-129`（四个资源命令 + `gitChanges` 都完成后才发第一次 update）对照 `console_api.rs:251-257`（超时即 `reject_run`+503）与 ：302-320（`apply_run_update` 不校验当前状态，任意覆盖）。
- 复现：大仓库上 `gitChanges`/`listDispatchableModels` 慢于 5 秒 → CLI 已收到 503/rejected，前端随后照常盖章、派发，并把 rejected 覆盖回 running——模型实际在跑，skill 侧按失败回退。
- 建议：`apply_run_update` 拒绝对终态（rejected/done/failed）run 的非终态覆盖；或前端首发 update 前确认 run 仍为 running。

**N3【低】通道续写用户旧图时把整图 `source` 盖成 `'console'`**
- 位置：`console-channel.ts:103`（`{ ...base, project: event.project, source: 'console' }`，base 可能是用户自建图）；`CenterPanel.tsx:1447` 按 `graph.source==='console'` 把 tab 标成「· 来自总指挥」。
- 后果：用户自己建的图被通道续写后，tab 被误标为总指挥来源。
- 建议：仅新建图标 `source:'console'`，续写时保留原值。

**N4【低】adopt 要求图在内存 state 中**
- 位置：`console-channel.ts:194-195`（`graphs[run.graph_id]` 不存在即抛「通道图未打开」→ 409）。
- 复现：run 执行中用户关掉该图 tab → `aicross adopt <run> <node>` 得 409，尽管图在盘上（v1 的 Rust 入口可从盘读，现入口做不到）。
- 建议：先 `loadGraph` 兜底，或文档标注该限制。

**N5【低】两处近似同义反复的测试残留**
- `src/hook_server.rs:240-245`：`head_and_options_are_http_methods` 断言字面量字符串匹配同一份字面量前缀数组，改坏 `handle_conn:104` 的分支不会红；
- `src/console_cli.rs:312-314`：断言 `HTTP_TIMEOUT` 常量等于自身。
- 真实行为目前只由代码本身保证。建议分别改为驱动 `handle_conn` 的分流函数与断言 `send()` 构造的 request 带 timeout。

**N6【info】报告与工作区不同步**
- `console-channel.ts:84-88,131-144` 的 `mergeConsoleCommit`（并发编辑合并落图，配套测试 `console-channel.test.ts:81-94`）是本轮审查期间新出现的改动，`tasks/REPORT-16-R1.md` 未记载；工作区在审查时点仍在变动，REPORT 的验证结论对应的是较早快照。

**N7【info】文档示例里节点 `status: "running"` 不可出现**
- `docs/CONSOLE-CHANNEL.md:79` 示例节点状态写 `running`；实际 `nodeSummary` 只会产生 `draft/pending/done/failed/quota_exhausted`（`console-channel.ts:68`）。同页 ：88 的状态词列表本身是对的。

---

## 核过没问题的点

- **中继链路**：401 先于路由先于 body；emit 失败/超时双路径都 `reject_run`+503；rejected 回执转 409 且摘要保留原因；run 摘要容量 200 淘汰（生产插入路径 `insert_inflight_run` 同样带淘汰）；`list_console_runs` 按 started_at 倒序。
- **鉴权**：常量时间比较把长度差纳入累计、无短路；未知路径也先过令牌；413 在鉴权后、读 body 前；Content-Length 解析失败按 413、读不满按 400。
- **清单生命周期**：`create_new` → 收紧 ACL → 写正文 → sync → 原子替换，失败清临时文件；Unix `chmod 0600` 在写前；退出删除且 `NotFound` 视为成功（`server.rs:4143`）；SID 解析有 `S-` 前缀校验；`%SystemRoot%`。
- **NSIS 语义**（解码核对）：只增删本项、保型、保留 `%VAR%`、大小写/尾反斜杠、Path 缺失不写空值、卸载仅在确有删除时回写并广播；安装目录经环境变量传递。
- **CLI**：退出码 0/1/2/3 与文档一致；参数错误先于清单检查；`--wait` 每轮重读清单可跟随控制台重启换端口/令牌；stdout 仅 JSON、stderr 单行。
- **前端流水线复用**：`stamp`、`presetWithRoleDefaults`、`serializeContext`/`buildNodeRequest`、`runStageLoop`（修正轮/红绿/重复批次/上限/缺目标全保留）、`dirtyWorktreeHintOf`、`leftoverNodes` 均为界面同一来源；`skip` 把非本次创建的已盖章节点排除在派发外（授权范围=本次派发）。
- **接线**：命令与事件注册（`server.rs:3988-3990`）；`tauri.ts` 契约类型与 Rust serde 字段逐一对齐；CenterPanel 在 `graph_id` 为空时不开 tab、background/focus/open 三态与测试一致；设置项 1–3 归一化钳位。
- **文档**：状态码表、退出码表、别名表、平台说明、5 秒/30 秒/600 秒超时、清单权限与 PID 判活描述均与实现一致。
- **未新增依赖**：`Cargo.toml` 无新 crate（`Win32_System_Threading` feature 已有，仅注释补充用途）。