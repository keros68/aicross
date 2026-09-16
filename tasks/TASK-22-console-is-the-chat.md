# TASK-22 控制台就是对话窗 · 第一期（09-16 用户裁决）

依据 `docs/PRODUCT.md`「控制台就是对话窗（09-16 定案，用户裁决）」。**这一期只做能用的最小一步**，不要一次把整个定案做完；后面还有第二、三期。

## 背景（编排者已核实，不用重查）

- 用户在 codex 的原生界面里打 `@kimi`，命中的是 codex 自己的文件搜索，回「no matches」。他以为控制台里能 @ 别家模型——产品主张就是这个，实现漂了。
- 右栏「应用」在 `!focusNode` 时是禁用的（`OrchestrationPanel.tsx:222`），所以从会话 tab 看过去编排永远是死的，用户「没搞懂怎么用」。
- 新 tab 自动起 codex 这条已由编排者改掉（`commanderLauncherHidden` 不再按默认直启），本期不用再动。

## 本期要做

### 1. 改名：总指挥 → 控制台

全仓一处定义、处处引用。界面文案、`docs/MANUAL.md` 术语表、`docs/PRODUCT.md` 里提到「总指挥」的地方一并改。**注意留痕与配置文件里的既有字段名不要跟着改**（`commander.json`、`source: "console"`、事件名 `aicross:console-*` 等都保持），只改用户看得见的词。

### 2. 控制台 tab 的中栏换成我们自己的消息流

- 新建 tab 的菜单只保留「控制台」一项；「系统终端」「对话图」从菜单撤掉（**代码与既有 tab 类型不要删**，它们降为次级入口，第二期再收口；本期只是不在新建菜单里出现）。
- 选完 agent 与目录后，进入的是**消息流**，不是 PTY：
  - 一个消息列表（用户消息 / 模型回复 / 系统提示三种），一个多行输入框。
  - 打字回车 → 单线发给当前选定的那一家，走既有引擎的**单节点**路径（`dispatchGraphStage` 一路），不要新起一套执行器。
  - **续聊**：同一控制台内连续对话要接上上下文，用各家已实测的续聊入口（`codex exec … resume <thread_id>`、`claude -p --resume <session_id>`、`pi --session-id <固定 id>`、kimi ACP `session/prompt`）。续聊 id 存在该 tab 的会话状态里。**续聊能力的事实源是 `references/channels.md`，不要自己猜参数。**
  - `@<家名>` 前缀：把这一条改派给那一家（家名用 `agentDisplayName` 那套显示名，大小写不敏感）。输入框要有补全提示，列出本机已接入且已验证的 agent。**`@` 后面跟不认识的名字时按普通文本发出去，不要静默吞掉。**
- **原生 TUI 保留，它是「agent 窗口」**（09-16 用户补充，定案已更新）：现有的原生 CLI tab 不删、不降级为遗留代码，它是某一家 agent 自己的窗口——`/model`、权限审批、各家快捷键都留在那里。本期**只保证不破坏**，入口在第二期收口（从派发结果里那一路打开、从左栏点模型打开）。它仍然**不在「新建 tab」菜单里**：新建只有控制台一种，agent 窗口是「为某个 agent 打开」而不是「新建一种 tab」。

### 3. 编排对当前控制台生效

- 右栏「方案与编排」在控制台 tab 激活时，**不再要求先选节点**：选模式 + 点「应用」= 把输入框里这句话（为空则用最近一条用户消息）按该模式派出去。
- 图在后台自动建（复用 TASK-16 的 `startPipelineHeadless` 与建图逻辑），派发结果以消息形式回到控制台消息流里（每一路一条，带 agent / 模型 / 角色 / 耗时）。
- 「在图上选一个节点」这句空态文案只在对话图 tab 出现；控制台 tab 的空态换成「选一个模式，或直接打字单线聊」。

## 补充（09-16 用户追加，本期照此理解）

原先估工作量时把「中栏不再托管原生 TUI」当成要付出的代价，用户澄清：**原生 TUI 不用扔，它就是 agent 窗口。**
所以本期的边界更清楚了——要新写的只有控制台的消息流与单线续聊；原生 TUI 相关代码一行都不要删，
也不要标成 deprecated。`/model`、审批弹窗这些原生交互不是「没有了」，是挪进 agent 窗口。

## 不做（本期）

- 不做对话图的降级改造（它还是现在的样子，只是不在新建菜单里）。
- 不做原生交互的转译（codex 的 `/model`、权限审批等）——它们留在 agent 窗口里，不用搬进控制台。
- 不动派发引擎、留痕格式、控制台通道协议、建议条。
- 不动 `commander.json` 的字段名与 `aicross:console-*` 事件名。

## 契约（只追加）

- 会话状态里新增每个控制台 tab 的：`resume_id`（各家续聊 id）、`messages`（消息流）、`target_agent`（当前单线发给谁）。wire 用 camelCase，与既有前端一致。
- 不新增 Rust HTTP 接口；单线执行复用既有 Tauri 命令。

## 验证

`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`；跑 `scripts/copy-audit.py`。

关键行为要有能红的测试：`@家名` 的解析（认识的 / 不认识的 / 没有 @ 的）、单线续聊 id 的存取、「应用」在控制台 tab 下不再被 `focusNode` 卡住、新建菜单只剩一项、改名后全仓没有残留的「总指挥」字样（用一条扫描测试钉住）。每条先确认去掉实现会红，并在报告里写清怎么红的。

保持文件原有换行（`src/server.rs`、`src/main.rs` CRLF，其它 Rust LF，`CenterPanel.tsx` 原有 CRLF，`src-ui/src/tauri.ts` 混合只做字节级最小改动）；不 `cargo fmt`；不起窗口、不发真机请求、不调真实模型；不 commit、不 push。报告写 `tasks/REPORT-22.md`，逐条对应上面 1–3 与「不做」。
