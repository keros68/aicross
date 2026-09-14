# TASK-04 复审 R2（GLM-5.3-Flash via cc-switch，只读盲审，2026-09-14 08:35–08:50）

复核完毕。以下是逐条核对结果（全部基于当前 main 静态核对 + 报告比对；运行时结论采信编排者今天的复跑：cargo 156 / vitest 91 / build 通过）。

# 修正轮 R1（17 条）

| # | 结论 | 依据 |
|---|------|------|
| 1 G1 补勾方案 | **已核实修好** | `context.ts:121-123` 盲节点非父回复产出 `defaultSelected:false` 候选；`DispatchOverlay.tsx:348-357` 渲染「可选上游」复选进 `selection.messageKeys`；`serializeContext`（context.ts:192）保留勾选项；测试 `context.test.ts:297-312` 锁定默认不选/勾选后进 prompt |
| 2 parseAcceptance 单词边界 | **已核实修好** | `dispatch.ts:185-189` `\b(?:PASS|NEEDS_FIX)\b` 取末 1200 字符内最后一个；测试覆盖 `all tests passed`→UNKNOWN、同现取后、截断线外（dispatch.test.ts:115-122） |
| 3 模板可在设置里看 | **已核实修好** | `SettingsModal.tsx:442-450` 角色页只读渲染三段模板，来源与生产同一份 `ROLE_PROMPT_TEMPLATES` |
| 4 用量表按阶段合计 | **已核实修好** | `DispatchOverlay.tsx:256-268` 按 stage 归组，:430 插小计行（in/out/cache/耗时） |
| 5 单写者非 codex 提示 | **改法有问题** | R1 报告称 G1 显示「该通道无写工具」（REPORT-04:752），但该提示串在当前 src-ui 已不存在；R2#2 让 claude/pi 拿到写工具后提示对它们确实过时，可 kimi/cc-switch 配 `单写者` 仍无任何提示（cc-switch 必然空产物），角色填充约束也随 R2 撤销。影响小但报告与代码不符 |
| 6 写锁失败不无锁运行 | **已核实修好** | `dispatch.rs:1066-1073` 捕获 `write_lock_error`，:1122-1127 直接 `status=error` 且跳过执行 |
| 7 盲验收不看 kind | **已核实修好** | `context.ts:112-118` 只按 `reply.artifacts` 判定；turn 型单父回退测试 `context.test.ts:314-321` |
| 8 快照 8MB 上限 | **已核实修好** | `SNAPSHOT_FILE_LIMIT`（dispatch.rs:530）、超限只存 size+mtime（:715-716），比较走 size+mtime（:754-764），diff 按 Binary（:817-819，测试 ：2008 断言） |
| 9 继续前清孤儿 | **已核实修好（按修正单字面）** | `GraphTab.tsx:174-188` 先按 STATE 头里的 runId+activeNodeIds 逐节点 `cancelDispatchNode` 再重派。注意局限：崩溃重启后 pid 登记表随旧进程消失，`cancel_registered` 落空，只能挡新进程内的运行而非真杀孤儿——原审查担心的跨崩溃并发写场景只是缓解 |
| 10 G1 浮层视觉 | **已核实修好** | `GraphTab.css:21` 不透明 `var(--bg-app)`；vendor 去重连接（DispatchOverlay.tsx:253-255,363）；无资源时手输 agent/model（:337-340）；未指定模型阻止预览/确认/派发（:78-81,:309,:370） |
| 11 盘符前缀 | **已核实修好** | `dispatch.rs:1356-1361` 拒绝 `Component::Prefix`，Windows 测试 `blind_acceptance_rejects_drive_relative_changed_path`（:2189） |
| 12 STATE 机器可读头 | **已核实修好** | 首行 `aicross-state: {json}`（dispatch.rs:1576-1580），`inspect_dispatch_state` 只解析头部（:288-293），`validation_failed` 单列（:1601-1608）；测试 :1896/:1934 |
| 13 adopted 保留 | **已核实修好** | `dispatch.ts:116` 展开保留 + 测试 dispatch.test.ts:81-91 |
| 14 confirm 集合一致 | **已核实修好（改法变了）** | 当前 `initialIds = dispatchableNodeIds(graph)`（DispatchOverlay.tsx:40）——确认页只列真会派的节点，天然一致；报告所述「保留深层草稿+待上游完成标注」已不存在，属 R2#10 重做后的替代实现，报告文字过时但结果达标 |
| 15 is_continuing 等三小项 | **已核实修好** | `dispatch.rs:314-316` 供 preview(:338)/run_one(:1102) 共用；归类模型 input 仅 disabled 无 onChange（SettingsModal.tsx:489-495）；两套原子写互指（dispatch.rs:1637、orchestration.rs:374-376） |
| 16 reducer + diff 边界测试 | **已核实修好** | `reducePipelineStep`（dispatch.ts:355-404）覆盖重试上限/任一失败停/封顶（测试 ：169-212）；`artifact_diff_covers_deleted_binary_large_and_oversized_markers`（dispatch.rs:1993-2011）钉住删除/二进制/>200KB/8MB |
| 17 REPORT 措辞 | **已核实修好** | REPORT:972「按修正单允许的 fallback 复跑」；:1365 明确 live harness 未过前端 `serializeContext` 属过渡 |

# 修正轮 R2（16 条）

| # | 结论 | 依据 |
|---|------|------|
| 1 盲验收读得到产物 | **已核实修好** | `BLIND_ARTIFACT_NOTICE`（channels/mod.rs:6）在目录交付后追加进 prompt（dispatch.rs:1374-1375）；diff 总量 <30KB 内联（:1376-1379）；pi 盲 prompt 含提示行时 `--tools read,grep,find,ls`、否则 `--no-tools`（channels/mod.rs:240-245，与追加逻辑闭环）；测试 dispatch.rs:2151-2185（提示+内联+目录三项）、channels:844-852（argv） |
| 2 非盲可写 claude/pi 工具集 | **已核实修好** | claude `Read,Grep,Glob,Write,Edit,Bash`（channels:146-147）、pi `read,write,edit,bash,grep,find,ls`（:253-254）；argv 测试 ：924-941；角色填充不再强制 codex（orchestration.rs:336-368 + 测试 ：804） |
| 3 链式启发式 | **已核实修好** | 触发条件 `关\|低`→中（DispatchOverlay.tsx:46），确认页显示理由（:310-312）；经济预设默认「低」未改（orchestration.rs:264），主流程已活 |
| 4 G4 指定上游断链 | **已核实修好** | `appendCorrectionRound` 新执行节点 `upstreams:[acceptId, previous.id]`（dispatch.ts:251），测试断言顺序与内容（dispatch.test.ts:147） |
| 5 git diff --no-index + 回退 + kimi 裁剪 | **已核实修好** | 前后快照字节落临时区后 `git diff --no-index --binary --unified=3`（dispatch.rs:924-951），字节级保 CRLF，exit 0\|1；非 git 回退对单文件 diff 设 200KB 上限、超限「文件：…全文见 changed/」（:845-849）；kimi 30000 字节按 UTF-8 边界裁剪且保尾（channels:210、311-328，验收模板在尾部不被裁掉）；真 git 仓库集成测试 ：2014-2047 |
| 6 快照链接/上限/失败置 error | **已核实修好** | `symlink_metadata` + reparse point 检测跳链接与 junction（:650-659、684-688）；排除名单含 `.venv build coverage .next vendor`（:690-707）；8MB/128MB 双上限（:530-531、715-728）；快照或 diff 失败均 `status=error`（:1074-1084、1226-1229）；测试 ：2050/:2064 |
| 7 worktree 回退记账 | **已核实修好** | 调度前 `project_is_git_work_tree` 预探测（:398-399），`requires_single_writer`（:318-320）使非 git 的 worktree 节点按单写者排队（:427）；测试 :2118（纯函数级，偏弱，但接线真实） |
| 8 写锁公平排队/超时/可取消 | **已核实修好** | 按规范化路径的 `Mutex+Condvar+FIFO VecDeque`（:543-648）；deadline 超时报错（:634-639）；取消置标记并 notify_all（:565-574）、等锁循环响应（:623-627）；互斥与取消测试 ：2081/:2099 |
| 9 stage 逐节点 | **已核实修好** | `DispatchNodeRequest.stage` 带 default（:67-70），run_one 用节点自己的 stage 进 result 与 record（:1038、1244、1285），用量/结果分组用 `result.stage`（DispatchOverlay.tsx:256、392）；测试 `node_stage_is_used_for_result_and_record`（:2126） |
| 10 清单一致 + 总请求上限 | **已核实修好** | 首轮即 `dispatchableNodeIds`；每层重新 buildRequest 并刷 preview（:182-184）；前端 `withinRequestLimit`（:178）+ 后端拒绝单批超限（dispatch.rs:366-372）；设置默认 8、1–100（SettingsModal.tsx:477-486、dispatch-settings.ts:32-34）；纯函数测试 dispatch.test.ts:264-265 |
| 11 G4 批量缺陷 | **已核实修好** | 逐节点 `appendValidationRetry`、计数随新执行节点（dispatch.ts:360-383）；`addedCorrection ||=`（:401）；正文保留、`[验证输出]` 单独标注（:103-108）；测试 ：214-239 |
| 12 error_kind 全链路 | **已核实修好** | Rust `Reply.error_kind`（orchestration.rs:93-95）、TS（types.ts:24）、写回任意非 ok（dispatch.ts:109）、节点卡区分真身不符/验证失败（GraphTab.tsx:400-406）、浮层（DispatchOverlay.tsx:37）、STATE 单列、REPORT 契约节（:62-74）；测试 dispatch.test.ts:241-250 |
| 13 按阶段分栏 | **已核实修好** | `groupResultsByStage`（dispatch.ts:412-422）标题取首节点角色、退化「第 N 阶段」；网格列数动态（DispatchOverlay.tsx:392） |
| 14 cc-switch usage 样本 | **已核实修好** | 样本换成真实字段形态（channels:1122-1128）；live 加 `tokens_in > 0` 断言（dispatch.rs:2662）。缺环境变量仍静默 return（qwen 6.2 原诉求未全采），编排者已真跑并记录 in 7531/out 63，可接受 |
| 15 盲目录断言 | **已核实修好** | 排序后精确等于 `["REQUEST.md","changed","diff.patch"]` 且断言 `roman.py` 不在（dispatch.rs:2590-2596） |
| 16 模板共用 fixtures | **已核实修好** | `fixtures/prompts/{planning,execution,acceptance,blind-prefix}.txt`；TS `?raw`（dispatch.ts:3-5、context.ts:3）、Rust `include_str!`（dispatch.rs:2494-2497），harness 用同组文件拼 prompt（:2516/:2539/:2571） |

# 修正是否引入新缺陷

- **并发/锁**：未发现。取消标记在 run_one 末尾清理（dispatch.rs:1310），run_id 每轮新换无串扰；`mark_cancelled` 的 notify 与 100ms wait_timeout 上限配合无丢唤醒死结。
- **留痕契约**：`aicross-dispatch/1` schema 字段集未变（仅 `stage` 语义从批次改逐节点）；`Reply` 只追加可选 `artifacts`/`error_kind`，旧图兼容（REPORT:60-74 如实记载）；`DispatchSettings`/`DispatchNodeRequest` 新字段全带 serde default，旧请求兼容。
- **第 2/3 期行为**：`contextOf`/`serializeContext` 默认路径结果不变（候选 `defaultSelected:false` 不进默认上下文），旧 context 用例全保留；`dispatchableNodeIds`/续接/采纳语义未动。
- **并回注册**：server.rs:3836-3857 派发 7 命令 + orchestration + resources 全部在 `generate_handler`；tauri.ts:331-374 镜像齐全，`maxRequests` 类型在位（:389）。
- 两个 nitpick（不阻塞）：① git diff 路径改写 `replace("a/before/","a/")` 作用于整个 stdout，内容行恰含字面 `a/before/` 时会被误改（概率极低、仅影响 diff 展示）；② `maxRequests` 小于首轮节点数时确认页不预检，点确认直接进结果屏报上限错（无损失，体验突兀）。

# 结论

**两轮修正之后第 4 期达到可交付**——33 条修正里 31 条已核实修好、R1#5 改法被 R2 取代后留有缺口、R1#14 以替代方式达标，核心链路（盲验收拿到产物、git 真实 diff、写锁公平排队、逐节点 stage、请求上限、G4 批量语义）在代码与测试层面均闭合，唯一前置动作是编排者今天在窗口里真派一次。

仍未闭合项（按严重度排序）：

1. **R2 之后整条链路未再真机复跑**：pi 盲带产物的只读工具、git diff 产物、`BLIND_ARTIFACT_NOTICE` 在真实 CLI 上的效果只有单测/argv 级证据，等今天这次真派闭合。
2. **跨崩溃的孤儿进程不可杀**（R1#9 的局限）：pid 登记在内存，重启后「继续」只能防新进程内重跑，杀不掉上一实例留下的子进程，单写者互斥在该场景仍可被绕过。
3. **kimi/cc-switch + 单写者无「产物为空」提示**（R1#5 残留）：cc-switch 必然空产物、验收必 NEEDS_FIX 烧满 3 轮，界面无任何预警。
4. **自动推进仍会捎带派发本轮意外解锁的其他 draft 分支**：有总请求上限兜底，但无逐层二次确认。
5. **盲审未采纳项原样存在**：产物回传前端/图落盘未过 `safe_trace`（qwen 3.1）、验收/验证节点可配单写者拿 workspace-write（3.2）、kimi 盲隔离=部分无警示（3.3）、worktree 内验证命令几乎必失败（2.11）、恢复 run 换新 id 与放弃即删 STATE.md（2.12/2.13）、`stdout_summary` 与正文重复（2.14）。
6. **live 测试工程性残留**：缺环境变量时 `live_cc_switch_route` 静默 ok（`--ignored` 全跑会假绿）；`live_economy_pipeline` 依赖环境 `python -m pytest` 且零真身容错；多个 live 测试共享 `AICROSS_TEST_RUN_ROOT` 的 `set_var`，须 `--test-threads=1`。
7. 两个 nitpick（diff 内容行改写、上限不预检）。