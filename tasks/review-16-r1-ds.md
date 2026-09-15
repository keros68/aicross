# TASK-16 R1 复审

**总判：可交付但有未闭合项。** 9 组必修在实现层都落地了（薄中继、常量时间、CLI、NSIS、契约、在飞上限、GLM 补充项均能在实现里对上）；未闭合项集中在前端侧：控制台 run 没接图进度 store，且后台 run 用整图快照回写，运行期间的用户图编辑会被覆盖。

---

## 9 组逐条

**1. Rust 薄中继 —— 已修**
- `src/console_api.rs` 全文无 `run_prepared` / `build_node_request` / `prepare_dispatch_at` / `adopt_graph_node` / `DispatchSettings` / `save_graph`（grep 全 src 仅 dispatch.rs 自身测试命中 `DispatchSettings`）。
- 中继：`LiveBackend::dispatch` → `relay_dispatch`（`src/console_api.rs:129-148`、`233-273`）；回执 5 s 超时 → 503 且 run 置 rejected（`233-273`、`541-558` 测试）；emit 失败 → 503（`262-268`）。
- 摘要表容量 200、淘汰最旧（`19`、`284-294`、测试 `519-527`）；`GET /v1/runs/<id>` 只读摘要（`302-320`）。
- 事件 `aicross:console-dispatch`/`aicross:console-action`（`131`、`265`），前端命令 `console_run_update`/`console_action_ack`（`323`、`330`），注册于 `src/server.rs:3988-3990`。

**2. 前端复用既有流水线 —— 已修（两点残留见新发现 1/2）**
- `src-ui/src/console-channel.ts:60-79` 图选择：`graph.project` 优先，旧图按 tab `folderPath` 回退；`src-ui/src/graph/types.ts:174-194` `newGraph(project)` 真把 project 写在 Graph 上（测试 `src-ui/src/__tests__/console-channel.test.ts:29-37`）。
- `console-channel.ts:123-135` commit（SET_GRAPH+saveGraph+回报）、`149-174` 调 `startPipelineHeadless`，与 `DispatchOverlay.tsx:316-321` 同一函数（`dispatch.ts:1444-1478`）；`console_run_update` 在首帧/每阶段/落图/终态回报（`128`、`163-166`、`171`）。
- adopt/cancel 转发（`console-channel.ts:175-194`）；结果卡采纳已撤回前端 `setReplyAdopted`（`DispatchOverlay.tsx:766`），Rust 无采纳入口。
- G1：额度 `quotaBlockedNodeIds`（`110-115`）、验证失败（`106-109`）、只读/未接入（`104-105`）、请求上限（`116`）、脏 worktree 用同一 `dirtyWorktreeHintOf`（`120-121`）；设置经 `buildRequest` 的 `settings` 全量带入引擎（`139-147`，含 `validationCommand`/`fixRoundLimit`，修正轮与红绿照跑）。
- 焦点：`consoleTabPlan`（`13-19`）+ `CenterPanel.tsx:519-523` 总指挥前台只后台开图。

**3. HTTP 与清单安全 —— 已修**
- 常量时间比较：`console_api.rs:186-197`（逐字节异或累计 + 长度异或）。
- 403：`src/hook_server.rs:195-202`（任意 `Origin` 或 `Sec-Fetch-Site: cross-site`）；测试 `229-232`。
- body 前 401：头读完先 `early_http_response`（`147`），401/403 在读 body/413 之前；测试 `235-238`。
- `HEAD`/`OPTIONS` 进 HTTP 分流不进 JSON 行（`104`）。
- 清单失败只告警（`hook_server.rs:59-62`），hook server 照常启动。
- `%SystemRoot%\System32`（`console_api.rs:410-418`）；临时文件先收紧再写正文（`353-373`），测试 `560-574`。

**4. CLI —— 已修**
- `--timeout` 默认 600、正整数校验（`src/console_cli.rs:122-123`），到时 exit 1（`67-69`）。
- 每轮重读清单（`37-38`）；pid 判活 → 删清单并报「清单已过期」exit 3（`40-43`、`145-167`、`173-186`）。
- dev 独立清单（`console_api.rs:340-341`，读写同源；测试 `291-292`）。
- BOM 剥离、非 UTF-8 明确报错（`106-108`）；`--text`/`--text=` 都拒（`100-102`）。
- 30 s 总超时（`202`）；200 非 JSON → exit 1 + 原文首行（`60-63`、`211-212`、`222-223`）。
- 参数/文本先于清单检查（`33-35`），本地输入错误不被「未运行」遮盖。

**5. 安装包 PATH —— 已修（测试同源性问题见新发现 4）**
- 逐项解析两段 `installer.nsi` 内嵌 `-EncodedCommand`（`679-690`、`777-785`）：`DoNotExpandEnvironmentNames` 保 `%VAR%`、`GetValueKind` 保 `REG_SZ`/`REG_EXPAND_SZ`、`TrimEnd('\')`+`OrdinalIgnoreCase` 比较、只增删本项、`Path` 不存在时安装写非空本项/卸载不写、卸载计数不变不回写、只有 exit 10 才 `WM_SETTINGCHANGE`。
- `windows/fragments/path.wxs` 仍为 item 级 `Part="last" Permanent="no" System="no"`，只写自己的值；`tauri.conf.json:55-56` 已接线。
- 旧源码字符串断言已删；新增 `console-channel.ts:204-218` 纯函数测试（`console-channel.test.ts:39-46`）。

**6. 契约与文档 —— 已修**
- run 状态含 `rejected`（`console_api.rs:44-58`、`tauri.ts:646`）；节点状态原样保存（`306-320`），`quota_exhausted` 不改写并带 `quota_resets_at`（`65-77`、前端 `console-channel.ts:57-67`），测试 `536-547`。
- `leftover` 真填（`console-channel.ts:128`、`171`，测试 `quota_status...` 与前端流水线）。
- `docs/CONSOLE-CHANNEL.md` 补齐状态码表、退出码表、预设别名、平台可用性、dev 清单名、30 s 超时、200 非 JSON 行为；`docs/MANUAL.md:195` 有「通道在飞上限」。

**7. 在飞上限与 G1 —— 已修**
- Rust 硬上限 3 超限 409（`console_api.rs:20`、`284-294`，测试 `528-534`）；前端设置 1–3 默认 3（`dispatch-settings.ts:17,55-57`、`SettingsModal.tsx:636-643`、`console-channel.ts:85-88`）。
- 脏 worktree 命中写进 rejected 原因（`console-channel.ts:120-121`）。

**8. 测试整理 —— 部分**
- 清单里点名的测试都在：往返 `478-500`、rejected `502-517`、容量 `519-527`、常量时间 `455-461`、Origin/401 `229-238`、pid `295-302`、PATH `39-46`、共享启动函数行为与接线 `console-channel.test.ts:48-72`、`idle-and-tail-07c.test.tsx:419-427`。
- 但新增的 `hook_server.rs:241-245 head_and_options_are_http_methods` 是硬编码列表自证（不调用生产分流），属清单第 8 条要求清掉的同义反复；另外 PATH「纯函数」不是安装包执行的代码（见新发现 4）。

**9. GLM 补充项 —— 已修**
- 已结束 run 的 cancel 不改状态（`console_api.rs:142-150`，测试 `548-553`）。
- 200 非 JSON → exit 1 原文首行（`console_cli.rs:60-63`）。
- HTTP 总超时（`console_cli.rs:202`）。
- Windows 清单先收紧空临时文件 ACL 再写正文、再原子替换（`console_api.rs:353-373`、`380-392`；测试 `560-574` 断言 harden 早于正文）。
- 额度状态词与恢复时间保留（同上第 6 组）。
- NSIS 卸载确有本项才回写（解码后 `if($b.Count -ne $a.Count)`，`installer.nsi:779`）。

---

## 新发现的问题（按严重度）

**1【中】后台控制台 run 持有整图快照、每个阶段整图回写，运行期间的用户图编辑会被下次 commit 覆盖**
- 位置：`src-ui/src/console-channel.ts:123-135`（`commit(next)` 整图 `SET_GRAPH`+`saveGraph`）、`149-174`；`src-ui/src/components/center/graph/GraphTab.tsx:866-869`；`src-ui/src/graph/persist.ts:10-16`。
- 复现：控制台派一个多阶段预设 → 第一阶段执行期间（或结果已落图后），在同一张图里改某节点 prompt / 对上一节点点「采纳」→ 下一次 `commit`（markPending 或 applyDispatchResults）用流水线自己的闭包快照整图写回，界面与磁盘上的这次编辑消失。
- 建议：每次 commit 前从 store/磁盘重读并按 nodeId 合并 Reply（或加 `updated_at` 版本守卫，冲突即停止并回报 run rejected），否则「图的单写者」只在 Rust 侧成立。

**2【中低】控制台 run 未接图进度 store，图上节点卡没有心跳**
- 位置：`src-ui/src/console-channel.ts:149-174` 未传 `begin/end`；`startPipelineHeadless` 支持但可选（`src-ui/src/graph/dispatch.ts:1427-1442`）；`useDispatchProgress` 只在 runId 被 `beginRun` 设置后才订阅（`src-ui/src/graph/dispatch-progress.ts:114-151`）；对照 `DispatchOverlay.tsx:320-321`。
- 复现：总指挥前台时控制台派发，切到该图：节点卡没有「已运行 … · 工具 …」与空转胶囊（右栏任务板因自行 `subscribeProgress(run.run_id)` 仍有，`TaskBoard.tsx:101-108`）。
- 建议：由 CenterPanel 把 run_id 转给 GraphTab 的 store（或 GraphTab 监听 running console run 自动 `beginRun/endRun`）。

**3【低】每个 progress 事件都 `loadGraph`+`SET_GRAPH`，可能把内存图回退到盘上旧快照**
- 位置：`CenterPanel.tsx:506-521`（不看 `kind`）；`onStage` 的回报（`console-channel.ts:163-166`）早于同一阶段 commit 的 save（`123-131`）。
- 复现：派发进行中，图 tab 节点状态可能短暂从 pending 退回 draft（下一条结果 commit 又纠正）；此时用户编辑/采纳基于旧图，放大新发现 1。
- 建议：`kind === 'progress'` 只更新运行摘要不重载图，或仅在终态/`graph_id` 首次出现时 `loadGraph`。

**4【低】PATH 纯函数是镜像死代码，装包逻辑零测试**
- 位置：`console-channel.ts:204-218` 仅被 `console-channel.test.ts:39-46` 调用；实际生效的是 `installer.nsi:684`/`779` 的内嵌 PowerShell。
- 复现：改坏 PowerShell 脚本里的匹配/写回逻辑，测试不会红。
- 建议：把脚本抽成仓库内 `.ps1`（NSIS 安装时读取或生成 EncodedCommand），对同一份文本做解析测试；否则在测试注释里注明是镜像而非被装代码。

**5【低】同义反复测试未删**
- 位置：`src/hook_server.rs:241-245`（断言测试内硬编码的前缀列表，不调用 `handle_conn`/生产判定）。
- 建议：删掉或改为对分流纯函数（如抽出 `is_http_request_line`）的行为断言。

**6【低】`--timeout` 非法值静默回退默认值**
- 位置：`console_cli.rs:139-142`（`option_value` 过滤 `-` 前缀）+ `122-123`。`--timeout -1`、`--timeout`（缺值）都返回 `None` → 600，而不是退出码 2。
- 建议：区分「选项缺席」与「值非法」。

**7【低】console G1 比界面严，文档未写**
- 位置：`console-channel.ts:104`（`未接入` 直接 409）对照 `DispatchOverlay.tsx:571-595`（只提示不阻断）；上限用整份预设节点数（`console-channel.ts:116`）而界面是单层数（`DispatchOverlay.tsx:181`）。
- 影响：资源库暂未同步的模型在界面可派、控制台被 409；`docs/CONSOLE-CHANNEL.md` 只列了 409 的 G1 项未提「未接入」。
- 建议：文档写明更严口径，或与界面统一。

**8【低】在飞 run 无回收**
- 位置：`console_api.rs:284-294`：run 卡在 `running`（前端崩溃、窗口关闭、丢回报）会永久占 1/3 名额，没有 TTL/reaper。
- 建议：给 running run 加过期清理，或按 manifest/前端心跳判死。

**9【低】回执超时后事件已送达的慢前端仍会启动该 run**
- 位置：`console_api.rs:255-270`（超时置 rejected 并 503）；`apply_run_update`（`302-320`）不校验当前状态，后续进度会把 rejected 改回 running/done。
- 影响：CLI 收到 503 若按「未受理」重试，可能重复派发。
- 建议：前端开始执行前先查 run 状态（非 running 即放弃并 ack 错误），或 Rust 对 rejected run 拒绝后续非 rejected 更新。

---

## 核过没问题的点

- **Rust 侧已无第二写者**：无 prompt 拼装、无整图 `save_graph`、无默认设置；HTTP 只校验/转发/记摘要；`GET` 的 `leftover`/`quota_resets_at`/`rejected` 与前端回报字段逐一对齐。
- **鉴权与清单**：常量时间比较正确（长度不等直接 false）；401 先于 body、413 先于读体；403 覆盖任意 Origin/cross-site；`Content-Length` 解析失败按过大；清单 `create_new` 临时文件 → 收紧 → 同步 → 原子替换（Windows `MoveFileExW`），失败清临时文件；`ExitRequested` 删清单；401/404/409/413/503 状态词与文档一致。
- **CLI**：退出码 3 只留给「拿不到控制台/清单过期/pid 已死/端点不通」；`--wait` 的 `status` 每轮重读清单并能捡起重启后的新端点；`console.dev.json` 与 release 互不覆盖；ureq `.timeout()` 是总超时。
- **NSIS**：两段 PowerShell 逐字符解码核对——安装幂等、卸载条件回写、保留值类型与 `%VAR%`、大小写与尾反斜杠归一、`Path` 缺失不写空值；WiX 侧本就 item 级、只动自己一项。
- **前端接线**：`newGraph` 写 `Graph.project`、`source=console` 覆盖图/根/材料/新节点；采纳路径已撤回到前端 `setReplyAdopted`（无 `adopt_graph_node`/`console_adopt` 残留）；`startPipelineHeadless` 的 begin/end/finally、`missing-target`/`limit`/重复批次的收尾与 `DispatchOverlay` 原行为一致（`dispatch.ts:1466-1478`、`DispatchOverlay.tsx:326-360`）；`consoleTabPlan` 不抢总指挥焦点；设置（含 `validationCommand`、`fixRoundLimit`、`ccSwitchPath`、`acpEnabled`）整份传入引擎，TASK-12/13 的修正轮与红绿在 headless 路径上会照跑（引擎侧逻辑未改，测试集未因此变红）。
- **契约追加而不破坏**：`accepted` 为新增字段、`graph_id`/`node_ids` 保留空值兼容；`console-runs-changed`、任务板行、「来自总指挥」标记、hook 首行分流均保持。