# TASK-04 复审 R3（GLM-5.3-Flash via cc-switch，只读盲审，2026-09-14 09:37–09:53）

复审完成。以下为逐条结论（测试复跑按任务设定留给编排者；我已静态核对全部断言与报告声称的测试是否真实存在）。

## 22 条逐条核对

**必修 A（窗口真派实测）**

| # | 结论 | 依据 |
|---|------|------|
| A1 产物键名 | **已核实修好** | `src/dispatch.rs:108-117` 两个 struct 已去 camelCase（键恰为 `diff/changed_files/worktree/stdout_summary`、`path/content`），与 `orchestration.rs:99-116` `ReplyArtifacts` 完全同形；前端 `types.ts:27-37` 同键，访问全部防御式（`context.ts:69-73`、`DispatchOverlay.tsx:416-422`）；键名测试在 `dispatch.rs:1846`、`context.test.ts:55`。注：未按“最好复用同一类型”合并成一份 struct（两份手抄镜像），非强制要求 |
| A2 循环异常 | **已核实修好** | 纯函数 `dispatch.ts:22-41` 只标未收到结果的节点；`DispatchOverlay.tsx:212-220` catch 接 `runError`，已 ok 节点在 `:205` 已写回不被覆盖；测试 `dispatch.test.ts:56`。附带确认：preview 失败时 `activeNodes` 仍是上一层（`:197` 在 preview 之后赋值），上层节点都有结果 → 不会误标 |
| A3 未指定模型 | **已核实修好** | 前端每层 `missingTargetNodeIds` 命中即回 confirm 只列该层（`DispatchOverlay.tsx:181-188`）；后端 `dispatch.rs:368-380` 在 project_path/run 目录/write_state **之前**返回节点级 error（含“未指定”），测试 `dispatch.rs:1863` 断言不建任何运行文件、`dispatch.test.ts:63` |
| A4 角色默认 | **已核实修好** | `SettingsModal.tsx:204-206` 保存后广播 `aicross:presets-changed`；`GraphTab.tsx:69-78`、`OrchestrationPanel.tsx:87-97`（含 roles 重拉）监听重拉；盖预设兜底 `GraphTab.tsx:279`、`OrchestrationPanel.tsx:130`；测试 `dispatch.test.ts:73`（只填空目标）。小注：GraphTab 本地 `roles` 仅挂载时加载（`GraphTab.tsx:65-67`），但后端 list_presets 已按新 roles 填空目标、兜底只填仍空者，实际影响≈0 |
| A5 重派入口 | **已核实修好** | 图卡 `GraphTab.tsx:452-455`、节点面板 `NodePanel.tsx:259-261`（与 TargetEditor 同面板，可先改目标再重派）→ `redraftNode` 重置 draft → `dispatchableNodeIds`（`dispatch.ts:58` 要求 draft）包含它；测试 `dispatch.test.ts:300`（父节点不受影响） |
| A6 别名表 | **已核实修好** | `channels/mod.rs:5-11` 只放 deepseek 一条三元组；`parse_pi` `:595-602` 命中即过、identity 记响应 id；zai glm-4.7→glm-5.3-flash 仍 IdentityMismatch（测试 `:985`、`:994-1001`）；资源冒烟 `resources.rs:1151-1163` 用同一张表，`smoke_model` 记实际 id + meta「路由别名」（`:1178-1182`），测试 `resources.rs:2109`；claude 第三方 `contains` 检查只作用于 claude 通道，不拦 pi |
| A7 留痕外置 | **已核实修好** | `dispatch.rs:1061-1076` >32KB 转 `~/.aicross/runs/<run>/<node>.raw.txt` + 指针 + 末尾 4KB，`:1342` 接入留痕；测试 `:1876`（40KB/20KB 两分支）。小注：raw 写失败被 `let _ =` 吞掉，指针可能指向不存在的文件（低风险） |
| A8 空工作目录 | **已核实修好** | `dispatch.rs:1566-1575` 空串报「请先在左栏「工作区」选择项目目录」，`preview_core:324` 与 `dispatch_stage_core:394` 共用且在写盘前；G1 经 previewError 显示并禁用确认（`DispatchOverlay.tsx:375,378`）；测试 `:1891` 断言 current_dir 无新文件 |
| 9 G1 决策行 | **已核实修好** | `dispatch.ts:220,222` 有 `写=` 与 `验证=<命令>`；测试 `dispatch.test.ts:308` |
| 10 关 tab 清 focus | **已核实修好（R3b 补课）** | `app-state.tsx:408-423` CLOSE_TERMINAL 置 `graphFocus: null`；测试 `app-state.test.ts:68`；关闭按钮 `CenterPanel.tsx:961-963` 走该 action。R3 曾不采纳、R3b 已落实 |
| 11 详情点不开 | **已核实修好** | `ResourcesOverlay.tsx:341-342` 改受控 `openDetails` + button onClick，弃用原生 summary |

**必修 B（两家复审共同项）**

| # | 结论 | 依据 |
|---|------|------|
| 12 脱敏 | **已核实修好** | `dispatch.rs:1046-1059` 覆盖 diff/changed_files 内容/stdout_summary 并累加（`:1290-1292`）；盲目录 REQUEST.md/diff.patch/changed/ 与附件在 `prepare_outbound` `:1403/:1413/:1438/:1471` 逐一脱敏；测试 `:1901`（sk-… 打码 + 盲目录文件） |
| 13 git 回退 | **已核实修好** | `dispatch.rs:925` `.unwrap_or(false)` 回退内置 diff；materialize 失败清理 `:945-950`、diff 后清理 `:969`；测试 `:1929`。注：清理断言走直接调 `materialize_snapshot` 的弱路径，绕过了 scratch 创建方（`artifact_diff_for_project`），绿了不代表 guard 被执行——guard 本体我已确认存在 |
| 14 盲/验收只读 | **改法有问题（轻）** | 后端到位：`:381-383` 验收/验证非只读拒绝；`:1104-1106` 盲强制只读、`:318-321`+`:1130` 不拿写锁、`:1138-1141` 不拍快照，Rust 测试 `:1949`（但只断言谓词，比补课单“第二个盲节点不排队”弱）。**前端 `validate.ts:26-28` 只查 role，未按修正单查 `visibility=盲 且 write!=只读`**；R3c 测试名「盲验收 worktree 都报只读约束」（`validate.test.ts:59-68`）两个用例其实都是 `role=验收`，名不副实。安全无洞（后端兜底），但「执行+盲+单写者」在 G1 无提示 |
| 15 残留提示 | **已核实修好** | `dispatch.ts:221` 两条提示、`orchestration.rs:336-339` `_models` 已删；测试 `dispatch.test.ts:308` 只断言 cc-switch，kimi 盲提示无断言（轻）。尾巴：`orchestration.rs:461` 与测试 `:815` 残留未用的 `models` 绑定 → R3c 输出里那两条 unused warning，461 还每次 list_presets 白读一次 resources.json |
| 16 worktree 跳验证 | **已核实修好** | `dispatch.rs:1041-1044`+`:1259-1260` 写「worktree 节点未跑验证命令，采纳合并后再验」；测试 `:1959-1963` |
| 17 恢复/放弃 | **已核实修好** | `dispatch.rs:301-312` 改名 `STATE.abandoned-<stamp>.md`；`GraphTab.tsx:190-204` 继续时用 STATE 头 run_id 取消活跃节点；`run_one` 末尾 `:1380` 清 cancelled 键；测试 `:1959-1977`（run_id 一致 + 归档存在 + 原 STATE 删除） |
| 18 live 假绿 | **已核实修好（R3b 补课）** | `dispatch.rs:2834-2841` 缺变量即 panic 明示。`live_economy_pipeline` 未加显式 panic，但硬编码真模型 + `assert status ok`（`:2749` 等），装不上 CLI 就响亮失败，不可能静默通过，可接受 |
| 19 REPORT 措辞 | **未修完（纯文档）** | 修正单点名的旧 REPORT:272（现为 315 行）仍是「**任务书**允许的 fallback」，只改了 R3 自己的新段落（`:13`）；R1-5（`:850`）、R1-14（`:859`）未按“按现状重写”处理 |
| 20 小任务启发式 | **改法有问题（轻：实现对、测试没改）** | `dispatch.ts:233-235` 阈值 120 + 多步词/换行 ✓；但 `dispatch.test.ts:156-161` 未按修正单“改测试”补 194 字/多步词用例，仍是旧两条断言 |
| 21 diff 行头改写 | **已核实修好** | `dispatch.rs:974-978` 仅 `diff --git `/`--- `/`+++ ` 三类行头 |
| 22 maxRequests | **已核实修好** | `DispatchOverlay.tsx:84,298,378` 提示 + 禁用确认；循环内 `:189-192` 兜底。前端用原始值、后端 clamp(1,100)，maxRequests=0 时前端全禁（保守方向，无害） |

## 新缺陷排查

- **并发/写锁**：盲强制只读发生在取锁判断（`1104`→`1111`）之前，调度循环 `444-457` 未动，无新竞态。
- **留痕契约**：`DispatchRecord`（`:1310-1340`）字段未改名；文件名 agent 空时落 `unspecified`（`:1604`，现被前置校验挡住，属双保险）。
- **Reply 契约**：`orchestration.rs:70-96` 只新增 Option 字段（artifacts/adopted/error_kind），无改名无删除；`error_kind` 缺省时图卡按普通失败显示（`GraphTab.tsx:417-423`），旧图兼容。
- **命令注册与镜像**：`server.rs:3836-3857` 全量注册；`tauri.ts:432-477` 与 Rust camelCase 外壳 + snake 内层 artifacts 逐字段一致。
- **会炸/误判的点**：① `npm run build` 最后一次成功是 R3 树——R3b 改了 `app-state.tsx`、R3c 只跑了 vitest，tsc 全量验证缺失（改动极小，复跑留意即可）；② `r3_13` 的清理断言与 `r3_14` 的谓词断言都比补课单要求弱，绿不代表全路径被执行；③ 真机核对标记：冒烟 deepseek-v4-flash 成功后资源页 meta 应出现「路由别名：deepseek-v4-flash→deepseek-flash」，没有即说明窗口跑的不是这份代码。

## 结论

**R3/R3b/R3c 之后第 4 期达到可交付**——22 条中 18 条已核实修好且测试真实存在、断言对得上，其余 3 条轻瑕疵 + 1 条文档项均无阻塞性；编排者沙箱外复跑四项验证（含这次补上 `npm run build`）并在窗口真派到 G4 通过即可收口。

仍未闭合（按严重度）：
1. `validate.ts:26` 未查 `visibility=盲 且 write!=只读`（执行+盲+worktree 无提示），且 `validate.test.ts:59` 用例名不副实——补一行判断 + 真·盲用例（后端已兜底，非阻塞）。
2. `orchestration.rs:461` unused `models`（每次 list_presets 白读 resources.json）+ 测试 `:815` 死绑定——两条 cargo warning，删两行的事。
3. #20 `isSmallEconomyTask` 测试未按修正单更新（缺 194 字/多步词用例）。
4. #19 REPORT 现第 315 行仍「任务书允许的 fallback」、R1-5/R1-14 未按现状重写。
5. 观察项（不要求本轮处理）：`r3_13`/`r3_14` 断言弱于补课单、routeLine kimi 提示无断言、`record_stdout` 写 raw 失败静默、最新树未跑过成功的 `npm run build`。
---
usage: [usage] provider=Zhipu GLM model=GLM-5.3-Flash input=2920780 fresh=112140 cache_create=0 cache_read=2808640 output=29537 turns=61 ms=968860
