# TASK-22 R1 复审 · Antigravity / Gemini 3.8 Flash (High)

2026-09-16，材料全部内联。判「可交付但有未闭合项」，三条编排者当场修掉。**因时间关系本轮只跑了这一家**（GLM 一轮要 15-25 分钟，当晚要收尾），编排者另做了独立复核与红绿。

---

总判：可交付但有未闭合项

### 按严重度排序的问题清单

1. **`src-ui/src/console-session.ts:133-149`**
   - **问题**：`completeConsoleMention` 在草稿已有输入与所选补全目标不匹配时未替换前缀，导致残留废弃前缀。
   - **复现条件**：用户在输入框输入 `@c `，补全弹出后点击 `@pi agent`；或输入 `@kimi ` 后点击 `@Claude Code`。
   - **原因**：函数使用所选目标显示名的单词（如 `['pi', 'agent']`）按词匹配 `source`。当输入为 `c` 时，`'pi'.startsWith('c')` 为 `false`，循环立即在 `cursor = 0` 处中断，导致整个原有 token 被当作 `rest` 正文保留，补全结果变成 `@pi agent c ` 或 `@Claude Code kimi `。
   - **建议改法**：将补全逻辑规范化为正则替换开头的 `@` 词法单元：以 `^@\S*` 匹配开头的 mention 片段，无论是否与 target 匹配，点击补全时统一替换为 `@${display} `，仅将其后的正文保留为 `rest`。

2. **`src-ui/src/__tests__/console-chat-22.test.ts:205-236` 与 `tasks/REPORT-22-R1.md`**
   - **问题**：缺少必修 2 明确要求的历史归档防篡改测试，且实现者报告遗漏了该条的红测试说明。
   - **复现条件**：查看测试文件第 205 行起的用例以及报告中的「必修 1、3、5、7 的红测试」段落。
   - **原因**：清单必修 2 及验证部分明确要求：「第 2 条用一条扫描测试钉住『`tasks/review-*.md` 与 `tasks/REPORT-*.md` 里不出现本期改名』。每条先确认去掉实现会红，并在报告里写清怎么红的。」当前测试仅反向扫描了代码库与当前定案文档无旧称，完全未扫描 `tasks/review-*.md` 与 `tasks/REPORT-*.md`，测试覆盖未闭合，报告中也完全漏写该条红测试。
   - **建议改法**：在测试文件中补加对 `tasks/review-*.md` 与 `tasks/REPORT-*.md` 的扫描用例，断言历史归档文件中未被替换为「控制台」，并在报告中补齐该用例的反向红测试说明。

3. **`tasks/REPORT-22.md`（未在 diff 中）**
   - **问题**：`tasks/REPORT-22.md` 遗漏补充「聊天当前只读」说明，与实现者报告不符。
   - **复现条件**：审查本次提交的 git diff 清单，无 `tasks/REPORT-22.md` 文件。
   - **原因**：必修 4 明确要求 `docs/MANUAL.md` 与 `tasks/REPORT-22.md` 均写明聊天只读，实现者报告声称「`tasks/REPORT-22.md` 同步补充」，但在还原必修 2 的历史归档时误将该文件一同还原/丢弃，未形成实际 diff。
   - **建议改法**：在 `tasks/REPORT-22.md` 中补齐控制台第一期聊天只读的说明。

4. **`src-ui/src/components/center/ConsoleChat.tsx:181-239`**
   - **问题**：控制台会话界面内缺失切入次级「对话图」视图的交互入口。
   - **复现条件**：在控制台会话中应用模式派发完成后，观察当前界面。
   - **原因**：必修 5 明确目标为「把 `graphId` 记到该 tab 上，让用户能去对话图里看这次派发」。当前实现虽然在 tab 上记录了 `consoleGraphId` 并完成了原图更新，但 `ConsoleChat` 界面内未提供任何跳转到对应 Graph 视图的按钮或操作项，新建菜单亦移除了对话图，用户在该 tab 上无法直达已生成的对话图。
   - **建议改法**：在控制台头部或派发结果卡片旁增加「查看对话图」次级入口，点击后将 tab 的视图状态切到对应 `consoleGraphId` 的 `GraphTab`。

---

### 核验无问题的要点

1. **两条「不要改」完全守住**：未在 Rust `src/dispatch.rs` 另造 resume 参数拼装；未触碰引擎备选作答主动清空 `session_id` 的机制。
2. **`@` 短名解析闭合**：显示名与机器名（`kimi` / `claude` / `codex` / `pi`）均能正确解析；大小写不敏感；严格执行后随空白或串尾边界；未知名称不吞字，连同 `@` 原样交由默认目标；句中 `@` 正常按普通正文发出。
3. **改派模型优先级**：按 tab 历史消息自近至远优先复用该 agent 实际使用过的 `(provider, model)`，无记录时正确回退至可用列表首位。
4. **接线测试有效性**：`dispatchConsoleTurn` 真实校验了 `runId`、prompt 原文与 `sessionId` 透传；`createConsoleApplyHandler` 真实测试了 tabId 匹配、busy 拦截与空文本拦截；无同义反复打桩。
5. **空输入重复应用拦截**：通过 `lastAppliedMessageId` 记录草稿派发与历史消息派发，再次空输入点击时准确识别 `already-applied` 并提示系统消息，拦截重复派发。
6. **续图复用逻辑**：复用 TASK-16 选图与 `appendConsoleMaterial`，首轮生成图即记录 `consoleGraphId`，后续派发在原图追加 material 节点，并由 `skip` 过滤已执行的历史节点，未与既有建图冲突。
7. **备选作答显示**：`responseMessage` 解析 `fallbackTo` 与 `modelReported`，备选作答时卡片顶部与元信息按实际代理显示，不再错标为主目标。
8. **多 tab 标题与空态判据**：启动器态与已选 agent 会话态标题分离；右栏方案空态加上 `Boolean(activeSession.targetAgent)` 判据，未选 agent 前不提前透出聊天引导。
9. **只读说明到位**：`docs/MANUAL.md` 与控制台界面空态提示均明确标注「控制台聊天当前只读，要改文件用模式派发」。
