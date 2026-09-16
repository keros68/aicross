# TASK-22 第一期盲审 · Antigravity / Gemini 3.8 Flash (High)

2026-09-16。材料全部内联（任务书 + 实现报告 + src/src-ui/docs 的完整 diff 与新增文件全文，约 12 万字符），prompt 明写不使用任何工具。用量：80,873 token / 92.2 秒 / 订阅额度。

**编排者注**：本文第 1 条（引擎不回传 sessionId）经核实不成立，理由见 `tasks/fix-22-r1.md` 开头。

---

总判：**不可交付**。

---

### 问题清单（按严重度排序）

#### 1. 单线续聊接线断裂，底层通道未接入实测参数且无 sessionId 回传
- **文件:行**：`src-ui/src/console-chat.ts:98`、`src-ui/src/components/center/ConsoleChat.tsx:112`
- **问题**：
  1. `dispatchConsoleTurn` 发起单节点派发时，`previousRunner` 被硬编码为空字符串 `''`，`stage` 硬编码为 `1`，既有引擎在缺少 runner 标识时无法接续上一轮上下文。
  2. `ConsoleChat.tsx` 中写了 `if (turn.result.sessionId)` 保存续聊 id，但 `src/dispatch.rs` 与派发通道本期完全未做改动；既有的 `dispatchGraphStage` 与 `DispatchNodeResult` 未对接 `references/channels.md` 规定的实测参数（`codex exec ... resume <id>` 选项前置、`claude -p --resume`、`pi --session-id`、`kimi ACP`），返回结果中根本没有回传 `sessionId`。
  3. `resumeId` 仅作为前端内存 state 存在，无任何持久化机制，tab 关闭或应用重启后状态完全清空。
- **复现条件**：在控制台中与某模型进行第一轮对话，收到回复后发送第二条消息。审查发往通道的请求载荷，`sessionId` 始终为空，无法接上历史上下文。
- **建议改法**：在 Rust 通道层按 `references/channels.md` 规范补齐各家 CLI 的 resume 参数拼装与输出捕获，使 `DispatchNodeResult` 能够回传会话 ID；`dispatchConsoleTurn` 携带真实的 `previousRunner`。

---

#### 2. `@家名` 仅匹配长显示名，不识别机器名与常用简称（`@kimi` 命中失败）
- **文件:行**：`src-ui/src/console-session.ts:88`
- **问题**：`parseConsoleMention` 只以 `@${agentDisplayName(target.agent)}` 作为识别前缀（即只识别 `@Claude Code`、`@Kimi Code`、`@pi agent`、`@Codex`）。任务书与产品定案中用户最典型触发的 `@kimi`、`@claude`、`@pi` 因与长显示名不完全一致，全部返回 `recognized: false`，被当作普通文本原样发给当前默认 agent。这与本次修改的出发点（解决用户输入 `@kimi` 命中失败）直接冲突。
- **复现条件**：在默认 agent 为 Codex 的控制台中输入 `@kimi 帮我分析`，回车发送。
- **建议改法**：在 `parseConsoleMention` 中建立显示名与机器标识（`kimi`、`claude`、`pi`、`codex`）的双向映射，允许 `@kimi` 与 `@Kimi Code` 均能正确匹配并改派。

---

#### 3. 补全列表点击会直接清空并覆盖用户已输入的全部内容
- **文件:行**：`src-ui/src/components/center/ConsoleChat.tsx:187`
- **问题**：补全浮层中按钮的点击行为是 `onClick={() => setDraft(`@${agentDisplayName(item.agent)} `)}`。若用户先输入了 `@Kimi 帮我重构这段代码`，再点击弹出的补全项，`setDraft` 会直接将输入框全部重置为 `@Kimi Code `，导致后面已经打好的内容全部被冲掉。
- **复现条件**：在输入框打字 `@Kimi 请帮我检查测试`，点击弹出的 `@Kimi Code` 补全按钮。
- **建议改法**：点击补全时仅替换开头的 `@` 标识符片段，保留后续用户文本（如 `setDraft(draft.replace(/^@\S*/, `@${display} `))`）。

---

#### 4. 输入框为空时点击「应用」会无限重复派发历史最后一条消息，且后台建图产生孤立图
- **文件:行**：`src-ui/src/components/center/ConsoleChat.tsx:135`、`src-ui/src/console-chat.ts:133`
- **问题**：
  1. 当输入框为空时，`ConsoleChat` 会取 `latestConsoleUserMessage(messages)` 作为派发内容。但系统对该历史消息没有任何「已应用」标记或消费锁，若用户连续点击右栏「应用」，同一条历史消息会被重复多次派发到后台并发起执行，造成额外开销与回复轰炸。
  2. `dispatchConsolePreset` 每次都调用 `newGraph(project)`，未复用 TASK-16 已有的 `selectConsoleGraph` 累加逻辑，导致每次编排都生成一张完全独立的孤立图，且未把新图的 `graphId` 记录到 tab 上，用户切换到「对话图」次级入口无法查看或裁决刚刚跑完的编排图。
- **复现条件**：单线发送一条消息并收到回复后，保持输入框为空，连续点击右栏「应用」按钮。
- **建议改法**：记录最后一条编排已派发的消息 ID 或状态，空输入时若上一条已编排过则提示用户输入新内容；编排派发复用现有图节点连接逻辑，并与对话图视图打通。

---

#### 5. 测试同义反复，全测纯函数，接线零覆盖
- **文件:行**：`src-ui/src/__tests__/console-chat-22.test.ts:33-54`、`src-ui/src/commander/__tests__/commander-10.test.tsx:477`
- **问题**：
  1. 测试用例 2 仅直接向 `reducer` 手动派发带 `'thread-1'` 的 `SET_CONSOLE_RESUME_ID`，只验证了 reducer 字典赋值，未测试 `ConsoleChat` 收到返回后存入状态的实际链路。
  2. 测试用例 3 仅测试纯函数 `canApplyOrchestration` 的返回值，未测试 `OrchestrationPanel` 点击「应用」派发 `CONSOLE_APPLY_EVENT` 及 `ConsoleChat` 监听触发派发的接线。
  3. 测试用例 4 仅断言静态常量数组 `NEW_TAB_MENU_ITEMS` 的长度为 1。
  4. `commander-10.test.tsx:477` 用 `readFileSync` 读源码检查字符串包含来替代行为测试。
  整个 `ConsoleChat.tsx` 及其与 Tauri IPC、右栏事件、状态机的交互在前端自动化测试中处于 0 接线覆盖状态。
- **复现条件**：人为阻断 `CONSOLE_APPLY_EVENT` 监听，运行 `npx vitest run src/__tests__/console-chat-22.test.ts`，测试依然全部通过。
- **建议改法**：补充 `ConsoleChat` 组件挂载与交互测试，模拟输入、改派与收到事件的真实调用。

---

#### 6. 进入控制台后无法切换 Agent 或工作区目录
- **文件:行**：`src-ui/src/components/center/ConsoleChat.tsx:156`
- **问题**：原原生 CLI 会话顶部提供「更改」按钮回到启动器。新中栏换成 `ConsoleChat` 后，头部仅渲染了三个静态 `span`，没有提供返回启动器、重新选择单线 Agent 或更改工作区目录的任何交互入口。用户一旦进入控制台，该 tab 就被永久绑定在此 agent 上，要换 agent 只能关闭 tab 重新新建。
- **复现条件**：进入控制台后，尝试在该 tab 将默认单线对话切换为其他已接入的 Agent。
- **建议改法**：在 `ConsoleChat` 顶部保留「更改」操作按钮，支持清空当前 `targetAgent` 回到启动器，或弹出下拉菜单变更单线目标。

---

### 核过没问题的要点

1. **改名边界与留痕契约保持严密**：`commander.json`、`source: "console"`、事件名 `aicross:console-*`、`CommanderSettings` 结构体字段名完全未动，仅改动了用户可见的文案与注释；旧称扫描测试保证了无残留。
2. **新建入口与主 Tab 收口彻底**：新建菜单仅保留「控制台」单项，全部关闭后的首个 tab 兜底逻辑对齐为 `createDefaultConsoleSession`。
3. **原生 TUI 与 PTY 代码原样保留**：`TierTerminal`、PTY 启动逻辑与会话管理未被删除或标记废弃，次级入口与启动器内的其他 CLI 依然能正常唤起原生终端。
4. **界面文案严格遵循 AGENTS.md 规范**：界面所有模型均通过 `agentDisplayName` 显示规范名（Claude Code / Codex / Kimi Code / pi agent），未泄露底层裸机器 ID；全角中文标点与中英文空格符合规范。
5. **@ 句中出现不误判**：非以 `@` 开头的普通文本（如句中包含 `@`）不会触发改派判定，文本完整原样发出，未被静默吞掉。
