# TASK-03 修正轮 R1 复审

核对基础：两份盲审、修正单、REPORT 的「修正轮 R1」与「编排者补记」逐条对照了 src/channels/mod.rs、src/dispatch.rs、src/process.rs、src/resources.rs、前端 graph/ 四件与测试、DispatchOverlay、GraphTab、tauri.ts、SettingsModal、orchestration.rs、server.rs、Cargo.toml，并抽查了补记引用的 `.dispatch/` 留痕（203400/203420/203631 均实存且 status ok，与补记一致）。

## 23 条逐条结论

| # | 结论 | 依据（文件:行号） |
|---|---|---|
| 1 pi 真身 | **已核实修好** | `src/channels/mod.rs:522-526` 优先 `responseModel` 回退 `message.model`，`:535-539` 不符判 identity_mismatch 且 identity 写真身，`src/dispatch.rs:484-486` 映射 status；fixture `fixtures/channels/pi-response-model-mismatch.jsonl` 是真实事件（请求 glm-4.7、responseModel glm-5.3-flash、cacheRead 1472、正文“青松-731”，与 194041 留痕一致），测试 `mod.rs:838-843` |
| 2 pi 错误事件 | **已核实修好** | `mod.rs:528-534` 取 `errorMessage`；fixture 为真实 403 事件，测试 `mod.rs:846-852` 断言 AccessDenied.Unpurchased |
| 3 pi 用量键名 | **已核实修好** | `mod.rs:558-577` 认 `input/output/cacheRead`（带 snake_case 回退），测试 `mod.rs:855-864` 断言 115/41/1472 |
| 4 kimi 别名+`-S`+锚定解析+parse_kimi | **已核实修好** | 别名：`mod.rs:178-180` 无 `/` 即报错（测试 `:922-938`）；发送 `-S`：`mod.rs:195-197`（测试 `:898-919`）；解析只锚定 `To resume this session:` 行并兼容四种 flag（`mod.rs:690-705`，正文含 `-r abc12345678` 不误取，测试 `:941-948`）；编排者改的 `parse_kimi`：跳过 role=tool（`:598-601`）、收 assistant 纯字符串 content（`:602-607`）、`session.resume_hint` 优先（`:609-613`），与 text 块路径互斥不重复计数，测试 `:951-964` 用真实事件形状；补记真机实证 `-S` 与 `-r` 均可续 |
| 5 脱敏 | **已核实修好** | `dispatch.rs:635-636` 键/值支持可选引号、引号值整体吞、裸值吞到行尾，测试 `:963-980` 含 JSON 两形态且不再命中；失败路径写「(未发送)」（`:409-412`，测试 `:1025-1051` 断言不含原文）；stdout/stderr/error 落盘与返回前 `safe_trace`（`:499-503`，测试 `:983-995`） |
| 6 kimi/cc-switch 用量 | **已核实修好** | kimi 递归收集 + max 合并（`mod.rs:639-688`，测试 `:967-978`）；cc-switch 从 stderr `[usage]` 行解析（`:712-724`，测试 `:981-988`）；「单轮」标注在 G1/结果卡/用量表三处（`DispatchOverlay.tsx:237,289,303`） |
| 7 codex --ephemeral | **已核实修好（文档项）** | flag 保留在 `mod.rs:163`，REPORT R1 第 7 条补记 help 出处与「部分」理由。help 输出本环境无 shell 无法独立复核，属可接受转述 |
| 8 codex thinking | **已核实修好** | `mod.rs:324-330` 关/低→low、中→medium、高→high，测试 `:1026-1031` 明确 never xhigh |
| 9 串行键归一化 | **已核实修好** | `dispatch.rs:293-298` 调度 position 过滤与 active_agents 均用 `normalize_agent` 后的键，`:322` 回传的也是归一化名；测试 `mod.rs:1010-1014`（只覆盖映射本身，循环路径靠读码确认） |
| 10 claude null | **已核实修好** | `mod.rs:400-404` `is_some_and(\|s\| !s.is_null())`，测试 `:1017-1023` |
| 11 E 节两项 | **已核实修好** | 角标 `GraphTab.tsx:421-423`；「新开会话」`DispatchOverlay.tsx:236` + `startNewSession :185-188`，`dispatch.ts:41-66` 清同目标父链 session_id，测试 `dispatch.test.ts:61-68` |
| 12 附件路径 | **已核实修好** | `context.ts:164-169` 共享全路径、盲 basename，materials 仍存全路径供复制（`:174`）；测试双向断言（`context.test.ts:235-264`） |
| 13 STATE.md | **已核实修好** | `dispatch.rs:783` 写实际 limit，失败/超时独立小节（`:771-781`），测试 `:1054-1070` |
| 14 live 断言 | **已核实修好** | `dispatch.rs:1153,1225-1236` 可写用户目录时断言 `status=="ok"` 且 tokens>0（kimi 豁免 tokens 并注释引任务书 A 表），沙箱打印原因；编排者沙箱外真跑三路 ok、续聊两轮 ok，留痕已核对 |
| 15 argv 测试 | **已核实修好** | channels 测试全部直调 `build_invocation_spec`（`mod.rs:764,773,787,806,869,899,923,992`），`resolve_binary` 只在生产入口 `build_invocation`（`:95`），无任何静默跳过护栏 |
| 16 cc_switch 路径贯通 | **已核实修好** | Rust：`dispatch.rs:31` 设置字段、`mod.rs:249-253` 空值报错、`dispatch.rs:447-448` 传入；前端：`tauri.ts:353` → `dispatch-settings.ts:13,33` → `DispatchOverlay.tsx:73` → `SettingsModal.tsx:593-600` 输入框；全仓 grep `agentteam` 零残留 |
| 17 非 Windows 取消 | **已核实修好** | `process.rs:239-249` unix 走 `libc::kill(SIGKILL)` 返回真实结果、其他平台 false；`Cargo.toml:132-133` cfg(unix) libc 已声明 |
| 18 状态词/阶段列 | **已核实修好** | `DispatchOverlay.tsx:28-30` 中文 resultStatus 与卡片头同源；`:303` 阶段列取 `request.stage` |
| 19 G2/采纳/running | **改法有问题（一处）** | G2 行带角色摘要+agent/model（`:269`）、running 禁背景关闭且隐藏关闭钮（`:191,:198`）、采纳写 `Reply.adopted`（`dispatch.ts:68-78`、`types.ts:22`）都做了；**但 `orchestration.rs:70-87` 的 Rust Reply 镜像没有 `adopted` 字段**，serde 丢未知字段 → save_graph/load 往返后标记消失，REPORT「随图持久化」端到端不成立（会话内活着，重启即丢） |
| 20 上下文/裁决 | **已核实修好** | `context.ts:59-65` 仅 done 进上下文（failed/pending/空文本全排除，测试 `context.test.ts:146-162`）；`appendDecision` 生成普通决策节点无 `[系统]` 前缀（`dispatch.ts:141-153`，测试 `dispatch.test.ts:97-102`） |
| 21 Overlay 整理 | **已核实修好** | 真换行 `{'\n\n'}`（`:306`）、settings 一次读取（`:54`）、前端不再复制隔离判定、回退「核对中」（`:208`） |
| 22 测试与注释 | **已核实修好** | `parse_classification` 只在 `#[cfg(test)]`（`dispatch.rs:862-902`，生产区无残留）；`OrchestrationPanel.tsx:1-2` 注释已更新；盲目录测试断言 CLAUDE.md/AGENTS.md/.dispatch 三者均未拷入（`dispatch.rs:1018-1020`） |
| 23 OnceLock | **已核实修好** | 脱敏 `dispatch.rs:630`、kimi session `mod.rs:691`、cc-switch usage `mod.rs:713` |

## 「不采纳」三条理由

三条均成立：G2 形态偏差功能等价且已记录在 REPORT；机器归类任务书标可选、容错解析测试保留、第 4 期接线；codex 389k 是路由策略而非执行器缺陷（复跑仍 20 万+ in，问题尚在，按观察记录第 4 期处理是诚实的）。

## 修正引入的新缺陷排查

除第 19 条的 `adopted` 持久化断链外未发现实质新缺陷：parse_kimi 两条收文路径互斥、tool 事件先跳过；失败路径与续聊组合不会泄漏（prepare 只有盲分支可出错，而盲节点不可能 continuing）；调度循环无死锁；派发命令已在 `server.rs:3842-3844` 注册；进度事件 payload 与前端监听匹配。两处可忽略 nit：`kimi_session_id` 的行过滤大小写敏感而正则 `(?i)`（CLI 输出格式固定，实际无影响）；补记未提的 203021/203023/203259 三份中间轮留痕与最终轮结论一致，不构成矛盾。

---

## 第 3 期能否提交并进入第 4 期

**可以提交进入第 4 期**：23 条中 22 条已核实修好、0 条未修、唯一保留是第 19 条「采纳标记跨重启丢失」这一字段级小缺陷，不阻塞交付。

仍未闭合的项：
1. **`adopted` 未进 Rust Reply 镜像**（`orchestration.rs:70-87`）——采纳标记重启即丢，REPORT 第 19 条「随图持久化」表述不实；一行修复，建议提交前顺手补上，否则改 REPORT 措辞；
2. codex 盲审输入仍 20 万+ token——已声明第 4 期路由策略（不采纳 3）；
3. 机器归类未接线——任务书可选项，第 4 期（不采纳 2）；
4. live 测试 prompt 是手写 concat，未走生产 `serializeContext`——修正单范围外的已知缺口；
5. `npx vitest run` 原命令需在普通 shell/CI 复核一次（沙箱 spawn EPERM，REPORT 不确定处 4 已自我声明）；
6. `codex exec --help` 的 `--ephemeral` 证据为 REPORT 转述，本次审查无法独立复核（低风险）；
7. kimi 用量恒为「—」——契约内行为（stream-json 无 usage），属已知限制。