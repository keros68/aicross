# fix-22-r1 控制台就是对话窗 · 第一期复审修正轮

依据 GLM-5.3-Flash 与 Antigravity/Gemini 3.8 Flash 对 `console/phase22` 的盲审（`tasks/review-22-glm.md`、`tasks/review-22-gemini.md`），以及编排者自己的核实。Gemini 判**不可交付**，GLM 判「可交付但有未闭合项」。契约只追加。

## 先说两条**不要改**的（两家的头号问题都不成立，编排者已验伪）

- **Gemini 1「引擎不回传 sessionId、续聊接线断裂」不成立**：`src/dispatch.rs` 的结果结构本来就有 `session_id`（:118/:219/:317），前端经 `DispatchStageRequest.nodes[].sessionId` 传入、从 `turn.result.sessionId` 读回，各家的 resume 参数与返回解析在既有引擎里都齐（codex 选项前置于 `resume` 且有 Rust 测试 `codex_resume_follows_parent_options`、claude `--resume`、pi `--session-id`）。**不要去 Rust 里再写一套 resume 拼装。**
- **GLM 1「备选作答的会话 id 被写进主目标的续聊槽」不成立**：引擎在备选作答时会主动把 `session_id` 清空（`dispatch.rs:3511`，注释写明理由），并有测试钉住（`assert!(result.session_id.is_empty(), "备选作答结果不得带回 session_id")`）。前端 `if (turn.result.sessionId)` 又挡了一层，写不进去。**这条只保留它的附带项**，见必修 7。

## 必修

1. **`@kimi` / `@claude` / `@pi` 不认，只认完整显示名（Gemini 2 / GLM 5；编排者实测确认）**：`parseConsoleMention` 只用 `agentDisplayName` 作前缀，于是 `@Kimi Code` 能命中而 `@kimi` 不能。**这正是本期要解决的那件事本身**——用户原话就是在会话里打 `@kimi`。编排者写了两条临时用例实测：`@kimi 帮我看一下`、`@claude 帮我看一下` 的 `recognized` 都是 `false`。修法：显示名与机器名（`kimi` / `claude` / `codex` / `pi`）双向映射，两种写法都认，仍然大小写不敏感、仍然要求后随空白或串尾、仍然「不认识的名字按普通文本原样发出不吞」。
2. **改名误伤了历史归档（编排者发现）**：改名把 `tasks/` 下 34 个**已归档的历史审查与报告**里的「总指挥」也改成了「控制台」——包括别家审查员当时写下的原文（如 `tasks/review-16-glm.md`）。那些是某个日期的记录，改了等于篡改证据。**把 `tasks/` 下所有历史审查（`review-*.md`）与报告（`REPORT-*.md`）的改名全部还原**；改名只动用户可见文案、`docs/MANUAL.md`、`docs/PRODUCT.md` 的当前正文，以及本期新写的文件。
3. **最大一块新逻辑零测试（GLM 2）**：`console-chat.ts` 与 `ConsoleChat.tsx` 全仓没有任何测试引用，现有的 `console-chat-22.test.ts` 实际只测了 `console-session.ts` 的纯函数与 reducer。**这正是本仓这两天连吃两次的「只测纯函数漏接线」形态**。补：`dispatchConsoleTurn` 的请求形状断言（`sessionId` 透传、`runId`、`nodes[0].prompt` 是原文）、`CONSOLE_APPLY_EVENT` 的 sessionId 匹配与 busy/空文本拦截、「应用」按钮 → `dispatchConsoleApply` → `dispatchConsolePreset` 整条链。用假 io 打桩，不要引 jsdom。
4. **聊天被硬编码成只读，而三份文档都没写（GLM 3）**：现在所有聊天轮次按只读沙箱派发，用户在聊天里让 codex 改文件会静默不生效。**编排者裁决：第一期就保持只读**（控制台没有审批 UI，放开写权限要连审批路径一起做，留第二期），但必须**说出来**——`docs/MANUAL.md` 与 `tasks/REPORT-22.md` 写明，并在控制台空态或输入框提示里给一句「本期聊天只读，要改文件用模式派发」。不要偷偷放开写权限。
5. **空输入连点「应用」会重复派发历史消息；每次都新建孤立图（Gemini 4）**：(a) 空输入取最近一条用户消息时，要记住该消息已被编排派过，重复点给系统提示而不是再派一次；(b) `dispatchConsolePreset` 每次 `newGraph(project)` 造出一张谁也看不到的孤立图——复用 TASK-16 的续图逻辑（`selectConsoleGraph` 那一路），并把 `graphId` 记到该 tab 上，让用户能去对话图里看这次派发。
6. **`@` 改派只取该 agent 清单里的第一个模型（GLM 5）**：`mentionTargets` 每家只取第一个，用户无法指定，也未必是本 tab 用过的那个。改成优先复用该 agent 在本 tab 最近一次用过的 `(provider, model)`，没有记录再退回清单第一个。
7. **补全点击会清掉已打的正文（Gemini 3 / GLM 5）**：`setDraft('@名 ')` 整体替换草稿，用户打了「@Kimi 帮我重构这段」再点补全就只剩「@Kimi Code 」。改成只替换开头的 `@` 段，保留后面的文本。
8. **备选作答时消息归属标错（GLM 1 的附带项，这条成立）**：回复卡用请求的 `selected` 显示 agent 与模型，但引擎切了备选时实际作答的是另一家。把 `fallbackTo` / `modelReported` 显示进消息元信息，别让用户以为是主目标答的。
9. **多个控制台 tab 标题都叫「控制台」（GLM 8）**：开两个就分不清。`session.targetAgent` 就在 tab 上，复用 `commanderTabTitle` 那套显示「agent · 模型 · 目录名」。
10. **启动器态就显示了聊天的空态文案（GLM 9）**：还没选 agent 时右栏已经写「选一个模式，或直接打字单线聊」，与实际状态不符。判据加上「已经选完 agent」。

## 低危，进 backlog 不在本轮做

- 单线路径没有额度预检，额度用尽的 `@` 目标照样派出，错误等引擎回来才显示（GLM 6）。
- 续聊 id 与消息流只在内存，关 tab / 重启即失；第二期接持久化，本轮只在 `docs/MANUAL.md` 写明现状（GLM 4）。
- 直启路径的遗留辅助函数（`commanderNewTabLaunch` / `commanderTabTransition` / `commanderArgs`）已无生产调用方，第二期收口时随直启路径一并删（GLM 7）。

## 保持

改名主路径、新建菜单只剩控制台、`@` 解析的「不认识不吞」契约、「应用」解卡后不派空文本、原生 TUI 与既有 tab 类型完好、`commander.json` 字段名与 `aicross:console-*` 事件名未动——两家都核过，不要动。

## 验证

`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`；跑 `scripts/copy-audit.py`。

必修 1、3、5、7 每条都要有能红的测试；第 2 条用一条扫描测试钉住「`tasks/review-*.md` 与 `tasks/REPORT-*.md` 里不出现本期改名」。每条先确认去掉实现会红，并在报告里写清怎么红的。

**动手前先 `git merge main`**：main 已经前进（backlog 61/65 的修复、启动器参数的类型修正都在上面）。保持文件原有换行；不 `cargo fmt`；不起窗口、不发真机请求、不调真实模型；不 commit、不 push。报告写 `tasks/REPORT-22-R1.md`，逐条对应本清单。
