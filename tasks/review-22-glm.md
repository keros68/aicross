# TASK-22 第一期审查报告

**总判：可交付但有未闭合项**——主路径（改名、消息流、@改派、编排放行、原生 TUI 保留、协议字段不动）与任务书相符，但有一个条件触发的续聊 id 污染缺陷、一块零测试的新逻辑、一个未披露的「聊天只读」硬约束，以及若干低危项。本环境无 shell，未能独立复跑测试，以下均为静态核对结论。

---

## 问题清单（按严重度）

**1.（中）备选切换后，别家会话 id 被存进当前 agent 的续聊槽**
- 位置：`src-ui/src/components/center/ConsoleChat.tsx:95-101`；配合 `src/dispatch.rs:3263-3296`
- 问题：`send()` 把 `turn.result.sessionId` 无条件存到**请求目标**的 resume key 下，不看 `result.fallbackTo`。而 Rust 引擎在请求不带显式 fallback 时仍会用资源库的 `fallback_order`（`resource_fallback_models`）在 error/timeout/identity_mismatch/quota_exhausted 时切备选重跑；备选成功后返回的结果里 `sessionId` 是**备选那家**的会话 id。单线路径 `modelTarget`（`console-chat.ts:61-72`）不设 `Target.fallback`，但挡不住资源库配置的备选。
- 复现条件：给聊天目标模型在资源页配过备选；一次聊天轮次主目标额度用尽/超时 → 引擎切备选成功 → 备选的 session id 被写成主目标的 resume id → 此后每条消息都以 `codex exec resume <别家id>` / `claude -p --resume <别家id>` 续聊报错，且 UI 无任何提示，只能重开 tab 恢复（`START_CONSOLE` 才清 `resumeId`，`app-state.tsx:520`）。同场景下 `responseMessage`（`ConsoleChat.tsx:26-38`）用 `selected` 显示 agent/模型，回答其实来自备选，归属标错。
- 建议：`result.fallbackTo` 非空时不写 `SET_CONSOLE_RESUME_ID`（或写到备选目标的 key 下），并把 `fallbackTo`/`modelReported` 显示进消息 meta。

**2.（中）最大一块新逻辑零测试，红测试全是纯函数/常量级**
- 位置：`src-ui/src/console-chat.ts`（全文件）、`src-ui/src/components/center/ConsoleChat.tsx`（组件）
- 问题：`console-chat-22.test.ts` 实际只测 `console-session.ts` 的纯函数 + app-state reducer。全仓没有任何测试引用 `dispatchConsoleTurn`/`dispatchConsolePreset`/`ConsoleChat`（grep 证实仅生产代码互相引用）。未覆盖的接线包括：单线请求里 `sessionId = resumeId` 的透传（`console-chat.ts:105`）、`CONSOLE_APPLY_EVENT` 的 sessionId 匹配与 busy/空文本拦截（`ConsoleChat.tsx:111-149`）、「应用」按钮 disabled 到 `dispatchConsoleApply` 再到 `dispatchConsolePreset` 的整条链。`canApplyOrchestration` 只测了纯函数，`OrchestrationPanel.tsx:230` 的接线没测。REPORT-22 的「先红」五条与实现一一对应这点写得如实，但覆盖面就是这五条。
- 建议：给 `dispatchConsoleTurn` 补请求形状断言（尤其 sessionId、runId、nodes[0].prompt 原文），给 ConsoleChat 的事件链补一条渲染/act 级测试。这正是任务书点名的「只测纯函数漏接线」形态。

**3.（中）控制台单线聊天被硬编码成只读，且三份文档都没写**
- 位置：`src-ui/src/console-chat.ts:70-71`（`visibility: '共享', write: '只读'`）
- 问题：所有聊天轮次按只读沙箱派发（codex `-s read-only`、claude `--tools Read,Grep,Glob`、pi `--tools read,grep,find,ls`）。任务书、PRODUCT.md 09-16 定案、REPORT-22 都没有「控制台聊天只读」这条定案；产品定案说的是中栏没有交互式审批，不等于禁写。用户在聊天里让 codex 改文件，会静默按只读执行。
- 建议：要么在 REPORT/MANUAL 写明这是第一期默认并留能力位，要么给出按目录/按模型放开的口径，交用户裁决。

**4.（低中）续聊 id 与消息流只存在于内存 tab 状态，关 tab/重启即失**
- 位置：`src-ui/src/store/app-state.tsx:136-176,165`；`src-ui/src/store/app-state.tsx:520`
- 问题：`TerminalSession` 不持久化（全仓无 terminals 相关 localStorage/后端保存）。任务书「续聊 id 存在该 tab 的会话状态里」字面满足；但 `docs/MANUAL.md:58-59` 只说「切换 tab 不会混用上下文」，没提重启后下一条消息从全新会话开始、消息流清空。换 agent/换目录的重置是对的（`START_CONSOLE` 清 `resumeId:{} messages:[]`，chat tab 无「更改」入口，只能新开 tab）。
- 建议：第二期接持久化或 HistoryBoard 式恢复前，至少在 MANUAL 补一句现状。

**5.（低）@改派的模型选取不可控 + 补全点击清空已打正文**
- 位置：`src-ui/src/console-session.ts:78-86`；`src-ui/src/components/center/ConsoleChat.tsx:192`
- 问题：`mentionTargets` 每个 agent 只取清单第一个模型，`@Kimi Code` 永远派给资源清单排序后的第一个 kimi 模型，用户无法指定，也未必是本 tab 用过的那个；补全按钮 `setDraft('@名 ')` 整体替换草稿，用户已打的正文会被清掉。另外 `@kimi`/`@pi` 单独打不全名（`Kimi Code`/`pi agent`）不命中解析，会按普通文本发出——符合「不吞」的契约，但靠补全列表兜底。
- 建议：改派目标优先复用该 agent 最近一次的 (provider,model)；补全按钮只替换 `@` 起始段。

**6.（低）单线路径没有额度/验证预检**
- 位置：`src-ui/src/console-chat.ts:87-94`
- 问题：`dispatchConsoleTurn` 只查 `exactModel` 存在；模式路径有的 `quotaBlockedNodeIds`/`validateNode`/verification 检查都没做。补全列表（`verifiedConsoleModels`）也不过滤 `quota_exhausted_until`。额度用尽的 @目标照样派出，错误等引擎回来才以消息形式出现。属体验问题，不是静默错误。
- 建议：发送前查一次额度，命中直接给系统提示。

**7.（低）本次改动造成的启动器死代码仍被测试供养**
- 位置：`src-ui/src/commander/launcher.ts:243,269`、`src-ui/src/commander/args.ts:83`
- 问题：`commanderNewTabLaunch`、`commanderTabTransition`、`commanderArgs` 生产代码已无调用方（grep 证实只剩测试与定义），`commander-10.test.tsx:477-486` 还用「CenterPanel 源码不含 `commanderNewTabLaunch(`」的字符串断言钉住"不用"。任务书「代码不删」针对原生 TUI/tab 类型；这三个是直启路径的遗留辅助函数。
- 建议：第二期收口时随直启路径一并删除（含对应测试段），或现在标注废弃原因。

**8.（低）控制台聊天 tab 标题全是「控制台」，多 tab 不可分辨**
- 位置：`src-ui/src/components/center/CenterPanel.tsx:1444`
- 问题：聊天 tab 没有 `commanderProfile`/`commanderTitle`（`START_CONSOLE` 显式清掉），标题落到固定 `CONTROL_CONSOLE_LABEL`；开两个控制台 tab 无法区分 agent/目录。`session.targetAgent` 就在 tab 上，可直接复用 `commanderTabTitle`。

**9.（低）启动器态的右栏空态文案提前出现**
- 位置：`src-ui/src/components/right/orchestration/OrchestrationPanel.tsx:48,246-250`
- 问题：`consoleActive` 判据是 `tool==='console'`，未选 agent 的启动器态也显示「选一个模式，或直接打字单线聊」，与实际状态（还没进入聊天）不符。「应用」此时是禁用的（`targetAgent` 为空），无功能问题。

---

## 核过没问题的要点

- **续聊参数与 channels.md 一致（既有引擎，本期未动）**：codex 选项全部在 `resume` 之前、prompt 走 stdin `-`（`src/channels/mod.rs:324-346`，有 Rust 测试 `codex_resume_follows_parent_options`）；claude `--resume <id>` + stdin（:313-314）；pi `--session-id`（:419-420）；kimi CLI `-S` / ACP 走引擎既有路径。前端只经 `DispatchStageRequest.nodes[].sessionId` 透传（`graph/dispatch.ts:1192`），各家返回侧解析（codex `thread.started`、claude `session_id`、pi `session` 事件、kimi resume_hint）齐全。
- **@解析契约**：认识的前缀大小写不敏感、显示名长者优先、后随字符须为空白或串尾（`console-session.ts:88-108`）；不认识/句中 @/无 @ 都按原文发出不吞（有测试）；补全只列 native/verified（`verifiedConsoleModels`，后端已填 verification 时 failed/unverified 不入选）。
- **「应用」解卡无反向问题**：console+`targetAgent` 直通（`console-session.ts:111-119`）；空输入且无历史 → 系统提示拦截，不派空文本；draft 非空才追加为新 user 消息，空 draft 复用最近一条属任务书设计；busy 拦截双击重复派发；graph tab 仍要求 focusNode；切到非图 tab 时 `graphFocus` 被清（`app-state.tsx:505`）。
- **建图复用无冲突**：`newGraph/fillRoot/stamp/startPipelineHeadless` 全部复用（`console-chat.ts:131-222`），`source:'console'` 是 TASK-16 既有字段；每次 apply 新建图、id 为 uuid，与 skill 通道 `selectConsoleGraph`「按 project+updated_at 续写最新图」的逻辑兼容，无 id 冲突。
- **原生 TUI 未破坏**：TierTerminal/PTY/多分屏分支与 `tier_terminal_start` 全链原样（TierTerminal.tsx 本期仅注释行变动）；graph tab 保留且可从启动器次级进入；外部 `launch --tool` 复用空闲 tab 的判定正确排除了聊天 tab（`CenterPanel.tsx:986-989`）；HistoryBoard 恢复路径未动。
- **改名只改该改的**：全仓「总指挥」仅剩任务书（含历史 tasks/*.md 已一并清洗，属扫描测试的必然结果）；`commander.json` snake_case 字段、`source:"console"`、`aicross:console-dispatch/propose/action`、`aicross:commander-*` 事件名全部未动；新增 `aicross:console-apply-preset` 是同前缀追加事件，非改名。
- **文案**：新增界面文案（「进入控制台」「直接输入可单线聊；输入 @ 可临时改派给另一家。」等）为中性简短说明体；MANUAL 术语表已加「主对话 tab → 控制台」。
- **声明与代码一致性**：REPORT-22 的「未新增执行器/HTTP 接口、未动 dispatch.rs 派发逻辑、未动建议条」与代码核对相符；其红测试对应关系写得具体。测试数字（590/462）本次未能独立复跑，未见明显会导致红的问题。