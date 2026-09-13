Now let me read the actual source files.



Now the frontend files.



核对完成。以下是逐条结论（文件:行号均为当前工作区实际行号）。

## 必修 1–17

| # | 结论 | 依据 |
|---|---|---|
| 1 pi 真身 | **已核实修好** | `channels/mod.rs:520-527` 优先 `responseModel` 再回退 `message.model`，`:536-540` 不符返回带 identity 的错误；`dispatch.rs:484-486` 映射 `identity_mismatch`；fixture `fixtures/channels/pi-response-model-mismatch.jsonl` 是 `.dispatch/20260913-194041` 的真实事件行（含 `responseId`/`timestamp`/`responseModel:glm-5.3-flash`），测试 `channels/mod.rs:836-843` 断言真身；真跑留痕 `.dispatch/20260913-203420:30` `identity: zai-coding-cn/glm-5.3-flash` 佐证链路生效 |
| 2 pi 错误事件 | **已核实修好** | `channels/mod.rs:528-534` 先判 `stopReason=="error"` 取 `errorMessage`；fixture `pi-error-event.jsonl` 取自 `20260913-194024`，测试 `:846-852` 断言 `AccessDenied.Unpurchased` 进 error |
| 3 pi 用量键名 | **已核实修好** | `channels/mod.rs:565-578` `input/output/cacheRead` + 旧键回退；测试 `:855-866` 断言 115/41/1472 |
| 4 kimi | **已核实修好**（含补记 3） | 别名校验 `channels/mod.rs:178-180`＋测试 `:925-938`；`-S` 发送 `:195-197`＋测试 `:905-921`，补记 2 真机确认 `-S`/`-r` 均可续；提示行锚定 `:690-706`（先 `lines().rev().find(含 "To resume this session:")` 再正则）＋测试 `:940-948` 正文含 `-r abc12345678` 不误取。补记 3 的 `parse_kimi`（`:614-660`）正确：`role=="tool"` 跳过 `:622-624`、assistant 纯字符串 content 收取 `:626-632`、`session.resume_hint` 优先 `:634-638,646-650`；测试 `:950-966` 的事件形状与真跑留痕 `.dispatch/20260913-203400:45-52` 逐字同构，该轮 `status: ok` + `session_id: session_2af67d1c…` 是实测通过证据 |
| 5 脱敏 | **已核实修好** | `dispatch.rs:635-636` 键/值均可选引号、引号值整体吞、无引号吞到行尾；测试 `:940-960` 含 `{"password": "a b,c"}`、`{"api_key":"sk-test value,tail"}` 两例 JSON 形态且断言脱敏后不再命中；失败路径 `:411` 写 `(未发送)`＋测试 `:1030-1052` 断言留痕不含原文；stdout/stderr/error 落盘与返回前一律 `safe_trace` `:498-503,659-661`＋测试 `:983-995` |
| 6 kimi/cc-switch 用量、单轮 | **已核实修好** | `channels/mod.rs:663-690` 递归收 usage（`merge_usage` 取 max，适配累计计数）＋测试 `:968-979`；cc-switch 从 stderr `[usage]` 解析 `:707-733`＋测试 `:981-989`；「单轮」标注 `DispatchOverlay.tsx:237,289,303` |
| 7 `--ephemeral` | **已核实修好**（报告层） | flag 保留 `channels/mod.rs:162-164`，出处与 isolation 仍「部分」的理由写进 `REPORT-03.md:325`；仓库内不可复核 `--help`，但 codex 真跑两轮 ok（`20260913-203631`）说明 flag 被接受 |
| 8 codex thinking | **已核实修好** | `channels/mod.rs:324-330` 关/低→low、中→medium、高→high；测试 `:1026-1031` 显式断言 high 不出 xhigh |
| 9 串行键归一化 | **已核实修好** | `dispatch.rs:294,297` 调度键与 active 集合都过 `normalize_agent`；测试 `channels/mod.rs:1010-1015` |
| 10 claude null | **已核实修好** | `channels/mod.rs:400-404` `is_some_and(\|s\| !s.is_null())`；测试 `:1017-1024` |
| 11 E 节两项 | **已核实修好** | 角标 `GraphTab.tsx:421-423`；「新开会话」`DispatchOverlay.tsx:210,236`＋`clearTargetSession`（`graph/dispatch.ts:41-68`，沿父链清同目标 session）＋`:96` `restartNodes` 强制本次不续接；测试 `dispatch.test.ts`「新开会话清除同目标父链 session_id」 |
| 12 附件全路径 | **已核实修好** | `context.ts:163-169` 盲取 basename、共享留全路径；`context.test.ts` 两例分别断言含 `C:\data\a.csv` 与只含 `raw.csv` 且不含全路径；复制仍用 `materials` 全路径 `dispatch.rs:534-560` |
| 13 STATE.md | **已核实修好** | `dispatch.rs:268-272` 实际 limit 传入 `:275-282`；`:771-780` 失败/超时单列，`:783` 写真实上限；测试 `:1054-1070`；真跑 `.dispatch/STATE.md` 已是新格式（三路进「已完成」、「失败与超时」空节） |
| 14 live 断言 | **已核实修好** | `dispatch.rs:1136-1155` 写权限探针，`:1206-1214` 断言 `status=="ok"` 且 in/out>0（kimi 例外，补记 4 给出任务书 A 表依据），`:1216-1218` 受限环境打印原因；补记 5/6 两条 ignored 真跑 `1 passed` |
| 15 argv 测试 | **已核实修好** | `build_invocation` 拆为 `build_invocation_spec`（`channels/mod.rs:100-118`）＋`resolve_binary`（`:95`）；六个 argv/校验测试全部直调 spec（`:869-938`、`:905-921`、`:991-1007`）；全仓 grep `resolve_binary` 只剩两处生产调用点，静默跳过护栏已消失 |
| 16 cc_switch 路径 | **已核实修好（贯通）** | `SettingsModal.tsx:593-600` 输入框 → `dispatch-settings.ts:13,33` 持久化/校验 → `DispatchOverlay.tsx:73` 进 request → `tauri.ts:353` 契约 → `dispatch.rs:31,447-448` → `channels/mod.rs:249-254` 空值先报「未配置 cc_switch.py 路径」再核文件；测试 `:991-1007`；`grep agentteam src/` 无命中，硬编码已清除 |
| 17 非 Windows 取消 | **已核实修好** | `process.rs:238-249` unix 走 `libc::kill(pid, SIGKILL)` 并返回实际结果、其他平台如实 false；`Cargo.toml:132-133` 已有 `cfg(unix)` libc。仅静态核对（Windows 上不编译该分支），无 unix 实测 |

## 次要 18–23

| # | 结论 | 依据 |
|---|---|---|
| 18 状态词/阶段 | **已核实修好（阶段值仍恒为 1）** | `DispatchOverlay.tsx:28-29` 中文状态、`:290` 卡片头与 `:303` 表格同源；阶段列取 `request.stage`，但 `:70` 请求里 stage 本身硬编码 1（本期单阶段），列值真实但不会变；另 `identity_mismatch` 被折叠为「失败」，只靠卡片正文 error 文案区分 |
| 19 G2/采纳 | **改法有问题** | G2 行带角色＋路由 `:269` ✓、running 禁背景关闭 `:191` ✓、采纳写 `Reply.adopted`（`graph/dispatch.ts:70-81`、`types.ts:22`、`GraphTab.tsx:424`、测试在 `dispatch.test.ts`）——但 Rust `Reply`（`src/orchestration.rs:70-87`）无 `adopted` 字段，`save_graph` 走 Rust 结构体反序列化后再序列化（`orchestration.rs:353-359,437-439`），未知字段被静默丢弃 ⇒ 重启或 `loadGraph` 后「已采纳」消失，REPORT 第 19 条「随图持久化」不成立，且违反 `types.ts:1` 声明的字段镜像契约 |
| 20 上下文/裁决 | **已核实修好** | `context.ts:60-64` 仅 `done` 进上下文＋`context.test.ts`「draft/pending/failed/空文本不进上下文」；`appendDecision`（`graph/dispatch.ts:143-157`）产普通用户节点，`DispatchOverlay.tsx:180` 文案无 `[系统]` 前缀＋对应测试 |
| 21 Overlay 整理 | **已核实修好** | `:306` 用 `{'\n\n'}` 真换行；`:54` `useState(loadDispatchSettings)` 惰性只读一次；`isolationOf` 已删（`:208` 用 preview 的 isolation，缺省「核对中」），全仓无残留 |
| 22 测试/注释 | **已核实修好** | `dispatch.rs:863,873` `parse_classification` 在 `#[cfg(test)] mod tests` 内；`OrchestrationPanel.tsx:1-2` 注释改为「派发执行由中栏 DispatchOverlay 承担」；盲目录测试断言 CLAUDE.md/AGENTS.md/.dispatch 均未拷入 `dispatch.rs:1020-1022` |
| 23 正则缓存 | **已核实修好** | `dispatch.rs:629-630`、`channels/mod.rs:691`、`:712` 三处 `OnceLock` |

## 修正是否引入新缺陷

- **实质回归只有 1 处**：第 19 条 `adopted` 不落盘（见上）。
- **可用性代价（两条决策叠加）**：不采纳「G2 改中栏系统行」＋新增「running 禁背景关闭」（`:191`）⇒ 长任务期间（codex 真跑 197.8s）用户被锁在模态里，回不到中栏看图，只能逐路取消。建议 running 态给「收起」。
- **耦合脆弱**：`dispatch.rs:484` 用 `starts_with("真身不符")` 判 `identity_mismatch`，channels 侧改文案即静默降级为 error；应给 `ChannelParseError` 加 kind 枚举。
- **留痕契约外的口子**：stdout/stderr/error 都过 `safe_trace`，但 `channel_output.text`（写进 Reply 并存到 `~/.aicross/graphs/*.json`）未过滤；模型若回显凭据，`.dispatch` 干净而图文件不干净。
- **未闭合的验证缺口**：cc-switch usage 正则（`channels/mod.rs:713-716`）假定 `input=… cache_read=… output=…` 固定键序，`cc_switch.py` 不在仓库、本轮 cc-switch 路无真跑；建议改成逐键独立捕获。kimi usage 恒 0 已由真跑留痕解释（stream-json 不带 usage），符合任务书「否则 —」。
- **过度脱敏面扩大**：`:635-636` 无引号分支吞到行尾，盲审材料正文里出现 `token:`/`pwd:` 等常见词会整行替换。是修正单明示取舍，但 G1 只报「已脱敏 N 处」，未提示可能连带吃掉正常内容。
- **并发/超时/取消未见新问题**：`limit=clamp(1,3).min(n)`＋归一化串行键＋`taskkill /T /F` 杀树＋4MiB 截断后探针 drain 均在位；`write_state` 只在主循环调用，无跨线程写竞争；`active==0` 兜底分支实际不可达但无害。live 测试仍用 `set_var`（进程级），两条 ignored 同跑时值相同，风险可控。
- **第 2 期图与面板行为未破坏**：`dispatchableNodeIds`/`continuationSession`/`stamp`/三列/@ 弹层逻辑未改，`context.test.ts` 既有用例（stamp 集成、accept 并集、悬空 parents、祖先闭包）全在；frontmatter 29 字段与 C 节逐字同序未动。

## 「不采纳」三条

1. **G2 保持 overlay**：理由部分成立（信息等价、偏差已记录），但代价见上（running 期锁死中栏）。可接受，建议第 4 期给收起入口。
2. **D 节机器归类**：成立，任务书 `TASK-03-dispatch.md:83` 明标「可选」。附带问题：`SettingsModal.tsx:585-591` 的「归类模型」输入框现在是悬空控件（填了无人使用），应置灰或注明第 4 期。
3. **codex 389k 输入**：成立，属路由策略而非执行器正确性。但 GLM 提的「盲节点按 input tokens 阈值告警」未做，结果卡对 389k 无任何提示，第 4 期应与内联/走 pi 的路由策略一起处理。

---

## 最终回答

**可以提交并进入第 4 期——23 条中 22 条已核实修好、验收 3/4 有真跑留痕与 `1 passed` 支撑，唯一实质回归是 `Reply.adopted` 在 Rust 契约里缺字段导致采纳标记不落盘（一行即可补），建议提交前修掉。**

仍未闭合的项：

1. `src/orchestration.rs:70-87` `Reply` 缺 `adopted` 字段 ⇒ 采纳标记重启即丢，REPORT 第 19 条「随图持久化」不成立（**建议本轮补**）。
2. `dispatch.rs:484` 靠中文文案前缀判 `identity_mismatch`，应改为类型化 kind。
3. `channel_output.text` 未过 `safe_trace`，模型回显凭据会进 `~/.aicross/graphs/*.json`。
4. cc-switch 路零真跑：`channels/mod.rs:713-716` usage 正则键序未与真实 `cc_switch.py` 核对；`--usage`、`--task-file`、`python` 解析整条链路无实测；preview 也不校验 `ccSwitchPath`。
5. `process.rs:238-249` unix 取消分支无实机验证（Windows 上不编译）。
6. running 期模态锁死中栏（不采纳 G2 中栏行 ＋ 禁背景关闭的叠加代价）。
7. `DispatchOverlay.tsx:70` `stage` 硬编码 1，多阶段派发与「阶段」列的真实变化留到第 4 期；`identity_mismatch` 在 UI 折叠为「失败」。
8. 盲节点大输入（codex 389k）无阈值告警；小体量材料内联/改走 pi 的路由策略未做。
9. 「归类模型」设置项悬空（D 节第 4 期再接）。
10. G1 决策行缺任务书 D 节要求的「理由：角色默认/用户指定」段（`graph/dispatch.ts:160-171`），两家审查均未提，属残留契约偏差。
11. 无引号脱敏吞整行的过度替换未在 G1 向用户提示。
12. live 测试 prompt 仍是手写 concat（`dispatch.rs:1180-1184`），未走生产 `serializeContext`/`BLIND_PROMPT_PREFIX`，真跑形态与 UI 外发形态不完全同源（qwen 5.2，未列入修正单）。