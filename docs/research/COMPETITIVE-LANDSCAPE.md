# 竞品/相邻项目定位辨析

> 2026-07-15 调研。回答"这些项目和 ai-cross 是不是一回事"。
> 一句话结论：**FreeBuddy 抢门面，pi 抢地基，两个都没碰 ai-cross 的核心（跨厂商交叉验证方法学）。**

## 2026-09-16：skill 的分发生态（`npx skills`）与「简历 skill 合集」这类竞品

来源：用户给的 `github.com/ImL1s/resume-skills` **404**（用户与仓库都查不到，可能私有、改名或手滑），下面是按同名检索到的同域项目与它们依赖的分发工具。

**`npx skills`（vercel-labs/skills）——开放的 agent skill 包管理器。**
- 拿 **GitHub 当注册表**：`owner/repo` 直接解析，也吃完整 URL、指向子目录的 tree URL、GitLab、Azure Repos、裸 git、本地路径。
- **发现规则**：根目录有 `SKILL.md` 就认；也会找 `skills/`、`skills/.curated/`、`.claude/skills/`、`.agents/skills/` 等约定位置，**向下走三层**，同时覆盖扁平布局 `skills/<name>/SKILL.md` 与带分类的目录布局。
- 安装位：默认项目级 `./<agent>/skills/`，`-g` 全局 `~/<agent>/skills/`；装完自动 symlink 到本机探测到的各家（`.claude/skills/`、`.cursor/skills/`、`.codex/skills/`）。号称支持 75+ agent。
- **没有版本锁、没有签名、没有审核**（README 未提及任何一项）。

**对我们的直接影响（已核实）**：`keros68/xiaoyu-skill` 是 `skills/<name>/SKILL.md` 布局（本机确认：`skills/ai-cross/SKILL.md`，根目录无 `SKILL.md`），**正好命中它的发现规则**，不改仓库结构就能被 `npx skills add keros68/xiaoyu-skill` 装上，也能用 tree URL 只装 ai-cross 这一个。这是一条我们此前不知道自己已经兼容的分发渠道。**注意边界**：安装与分发仍归用户的 skillmanager 管，这条只是多一个可选入口，本项目不去写任何安装位。

**同域竞品的形态（`Paramchoudhary/ResumeSkills`，★2310，2026-09-16 仍在更新）**：20 个简历/求职方向的 skill，按 `.claude/ .cursor/ .agents/ .gemini/ .windsurf/` **并排维护多份宿主目录**。README 主张是「75% 简历被 ATS 在人看到前刷掉」，卖点是 2–3 倍面试率。
- 它给的「Full Resume Optimization」五步流程是**线性提示词链**（ATS 检查 → 对岗位分析 → 改写要点 → 补关键词 → 排版），**没有任何事实核对、来源校验或人工审批门**。
- 与我们的关系：**不是同一个赛道**（简历 vs 多模型协作），但它印证了我们记过的那条判断——市面上大量 skill 合集是「把提示词分文件」，没有 verifier。我们的差异点（跨厂商交叉验证 + 用户裁决）在这类项目里看不到。
- 可借鉴的一点：多宿主并排目录是**它自己扛的成本**；走 `npx skills` 的话由 CLI 自动 symlink，不必自己维护并行目录。我们现在只有 `qoder/` 一个适配包，方向上更省。

**一条要记住的风险**：这个生态没有版本锁也没有签名，skill 供应链是裸的——`owner/repo` 改一行，下一个装的人就拿到新内容。我们自己发布时要意识到「用户怎么确认装到的是我们写的那版」这件事目前只能靠仓库本身的提交历史。

## 三层对比

| | 层级 | 是什么 | 多模型用来干嘛 | 能替代 ai-cross |
|---|---|---|---|---|
| **FreeBuddy** | 门面 · 编排壳 | Electron 桌面 App，拼装本机现成 CLI agent 并行跑 | 并行**分工**（实现/审查/测试各一 agent） | 不能，派完活就停 |
| **pi** | 地基 · harness | agent 框架（pi-ai 统一多厂商接口 SDK + 运行时）+ 自带编码 agent CLI | **抹平接口** + 会话内换模型**接力**（保留上下文） | 不能，正交，可叠着用 |
| **ai-cross** | 方法学 · 跑在 agent 里 | 注入 agent 的 skill/规则集 | **交叉验证**：给没 verifier 的任务外接盲区独立的 verifier | —— 暂无替代 |

配套定位图（本地临时文件，已发布为私有 artifact）：
`scratchpad/three-layers.html` / https://claude.ai/code/artifact/99e06672-3c08-437a-8671-92b456230cc6

## 各项目事实

### FreeBuddy — github.com/maojindao55/freebuddy
- 形态：Electron + TypeScript + Vite 桌面 workbench。"A desktop workbench for local coding agents"。
- 机制：检测/拉起本机安装的官方 CLID 二进制（Codex / ClaudeCode / OpenCode / Cursor / Kimi / Qoder / CodeBuddy + 任意 CLI），每个 agent 独立 workspace，统一界面。
- 多模型：**Workflow Teams**（模板编排，多 agent 并行，如 Codex 实现 + ClaudeCode 审查 + Kimi 测试）。附 Codex 用量卡、RSS 卡。BYOK。
- 与 ai-cross 差异：编排在 agent **外面**（GUI 模板写死）；多模型是**分工**不是验证。其"让 B 审 A"的模板把上游框架直接喂给审查者——违反盲验原则，它连这是问题都不知道。

### pi — github.com/earendil-works/pi
- 自称："agent harness"。repo 描述："AI agent toolkit: unified LLM API, agent loop, TUI, coding agent CLI"。README 首句："This is the home of the Pi agent harness project including our self extensible coding agent."
- 三包：`pi-ai`（统一多厂商 SDK：Anthropic/OpenAI/Google/Azure/Bedrock/Mistral/Groq/xAI/OpenRouter/Ollama…）、`pi-agent-core`（运行时）、`pi-coding-agent`（交互式编码 agent CLI）。
- 关系：**不是编排 Claude Code/Codex，它自己就是同类竞品**（一个 coding agent CLI）。
- "unified LLM API" = **代码层 SDK 抽象**（`npm install @earendil-works/pi-ai`，写一份代码路由到拥有该模型的 provider），**不是 OpenRouter 那种托管网关**。
- 卖点：self-extensible 不用 fork（TS 扩展/Skills/模板/主题）；供应链加固（依赖精确锁版本）；沙箱交给用户自己做（无内置权限系统）。
- org 出品，2025-08 建，活跃维护（v0.80.x）。star 数 agent 报 7 万但未核实，不采信。
- 与 ai-cross 关系：**正交**。pi 的多模型是"抹平接口 + 接力换模型"，ai-cross 是"利用盲区差异做验证"。可在 pi 上跑 ai-cross。
- 注意：pi 这个 repo 的 issue #4187 = "ZAI provider 用 SDK 方式打 GLM coding plan 端点，有封号风险"——正是本项目用 cc_switch.py + 拉起官方工具规避掉的那条线。通用框架反而更容易踩。

## 关键论证（对定位的意义）

1. **抹平接口 ≠ 改变盲区。** pi 让 GLM+Claude 跑同一套代码，该撞的共识错误照撞。同厂商共识错误 6% vs 跨厂商 1%（本项目实测）。pi 关心两家 API 长不长得一样；ai-cross 关心两家错得一不一样。正交。

2. **"会话内热切换保留上下文"技术上做不到无损**（官方文档实锤）：
   - OpenAI reasoning item 加密，跨模型报 `invalid_encrypted_content`，官方 workaround = 新开 thread 别切模型。
   - Anthropic thinking block 带签名，回传必须逐字一致，改一字符即 400。
   - prompt cache 模型级作用域，切模型即失效、冷启动重建常比不切还贵。
   - 唯一可行是**降级迁移**（丢 reasoning/thinking，纯文本重放）。pi 宣称的 "preserving thinking blocks" 跨厂商时几乎必然走这条有损路径。

3. **pi 保留上下文的方向 = 验证要切断的传染通道。** 上游模型框架一字不落灌进下游 = 确认偏误最优传播路径（对应本项目 07-11 的盲验原则/隔离四级、512 假象）。pi 做**接力**，ai-cross 要**盲验**——同一个动作、相反的上下文策略。

4. **定位教训（复用 07-09 的自我戳穿）：** 若 ai-cross 卖分层派发，FreeBuddy/pi/那 22 个编排器随时替代；卖跨厂商交叉验证，才没有替代品。别人照门面理解你——门面第一句得是"给没 verifier 的任务外接失败模式独立的 verifier"，不是"多模型分层派发省额度"。

## 赛道现状（供参考）

"并行跑多个 CLI agent"的桌面/TUI 编排器 2026 年已 22+（awesome-agent-orchestrators），且在自我淘汰（Crystal→Nimbalyst，Vibe Kanban 关停托管），Anthropic 官方 Agent Teams 从上面吃。**"多 agent 并行"本身已无护城河。**

订阅侧壁垒：Anthropic 明文禁止 + 技术封锁第三方复用订阅 OAuth（2026-04 完成执法）；Z.ai GLM 白名单制（SDK access 有封号风险）；仅 Kimi 明确欢迎第三方。→ 安全做法只有"拉起官方 CLI 进程"，这也是 FreeBuddy 和本项目 cc_switch 的共同选择。

## 补充（2026-08-30）：「群聊里 @ 多厂商 agent」已是大路货

> 触发：讨论控制台产品形态时，主线程断言"跨厂商 @ 寻址层没人做"。搜索后证伪。

| 产品 | 形态 | @ 跨厂商 | agent 之间可见性 | 备注 |
|---|---|---|---|---|
| **Bloome** bloome.im | SaaS IM（桌面+手机） | Claude Code / Codex / Gemini CLI / OpenCode | **共享**：同一 thread 互相可见（"Teammates and other agents share the same context"） | 闭源，credit 计费 |
| **OpenAgents Workspace** github.com/openagents-org/openagents | 开源 Apache 2.0，3.3k star | Claude Code + Codex 同群 | **共享**："@ 任何 Agent，它能看到同一会话里其他 Agent 的输出" | MCP/A2A；合作方含智谱、MiniMax；带共享浏览器 |
| **AgentConnect** github.com/agentconnect-md/agentconnect | 开源 Apache 2.0，Docker/K8s | ACP 兼容 agent（Claude Code/Codex…） | 同 thread 共享；有"谁能看某 agent 对话"的权限项（能否按消息隔离未核实） | 接 Slack/Telegram/Discord，自称"开源版 Claude Tag" |
| **agmsg** github.com/fujibee/agmsg | Bash + SQLite，无 UI，1.5k star | 9 种 CLI agent 互发消息 | **点名寻址**，hook 投递（Monitor/turn 两种） | 纯传输层，可作控制台的消息底座（license 待查） |
| Claude Code cross-session messaging（2.1.224+） | 官方内置 | 仅 Claude↔Claude | 点名 | 同厂商 |

向阳乔木（github.com/joeseesun）名下无此类产品（opencli skills、NotebookLM skill）。

**修正后的定位**：@ 寻址层已商品化，且四家清一色**默认共享上下文**——即 Anthropic 2026-08-13 多智能体研究里失效多发的"论坛"形态（见 SPEC v1.12）。没人把"验证者看不到其他 agent"做成一等公民，也没人带交叉验证方法学。控制台若做，差异化不是"跨厂商 @"，是"**@ 里有一种是盲的，且有章法**"：执行者 @ 默认共享上下文（效率），验证者 @ 默认盲（隔离由预设自动完成，用户只点一下）。唯一原创的 UX 问题：从共享对话里自动抠出"原始材料"而不带结论（只取附件/文件，或用户勾选消息）。

## 补充（2026-08-30）：程序化续聊——竞品怎么驱动 CLI agent，本机实测

> 触发：控制台需要"@ 一个模型并延续它的会话"。问题：三家 CLI 能否被程序化续聊？竞品用的是哪条路？

### 两条路线

| 路线 | 机制 | Claude Code | Codex | Kimi Code CLI |
|---|---|---|---|---|
| **一次性续聊**（每轮起一个进程，带会话 ID） | 进程退出即结束，靠 CLI 自己的会话持久化接上下文 | `claude -p --resume <session_id>`（`--output-format json` 返回 `session_id`；`-c` 续最近一次；`--fork-session` 分叉） | `codex exec [父级选项] resume <thread_id> -`（`--json` 的 `thread.started` 事件给 `thread_id`；`--last` 取最近；`exec fork` 分叉） | `kimi -p "…" -c`（续本目录最近会话）/ `-r <session_id>`；输出末尾打印 `To resume this session: kimi -r session_…` |
| **长驻进程 + 协议**（一个进程活着，多轮 JSON-RPC） | 客户端持有进程，逐轮发消息 | `claude -p --input-format stream-json --output-format stream-json`（Agent SDK 就是它的封装）；ACP 走官方适配器 `claude-agent-acp` | `codex app-server`（JSON-RPC 2.0 over stdio/WebSocket，`thread/start`、`thread/resume`、`turn/start`；实验性）；ACP 走社区适配器 `codex-acp` / `acp-adapter` | **原生 `kimi acp`**（ACP over stdio） |

### 本机实测（2026-08-30，两轮：先记口令、再问口令）

| CLI | 版本 | 命令 | 结果 |
|---|---|---|---|
| claude | 2.1.251 | 轮1 `echo … \| claude -p --model haiku --tools "" --output-format json` → 取 `session_id`；轮2 `claude -p --resume <id> --tools ""` | **PASS**，第二轮回 `PANDA-4417`，`session_id` 不变 |
| codex | 0.149.1 | 轮1 `codex exec --skip-git-repo-check -s read-only --json -` → `thread.started.thread_id`；轮2 `codex exec --skip-git-repo-check -s read-only --json resume <id> -` | **PASS**，回 `KOALA-8821`。**坑**：`-s`/`--json`/`--skip-git-repo-check` 是 `exec` 的父级选项，必须放在 `resume` 之前，放后面报 `unexpected argument '-s'`（exit 2） |
| kimi | 0.39.1 | 轮1 `kimi -p "…" --output-format text`；轮2 `kimi -p "…" -c --output-format text` | **PASS**，回 `LEMUR-3390`；`-c` 按工作目录找最近会话，控制台按目录隔离会话时要注意这一点，点名用 `-r <id>` |

结论：**一次性续聊三家都能用，控制台 MVP 不依赖长驻进程**。代价是每轮付一次 harness 冷启动足迹（见 SKILL.md「harness 税」）；成串对话时长驻协议更省，那是第二步。

### 竞品走的路

- **Bloome / OpenAgents / AgentConnect / OpenClaw / goose / Zed / JetBrains** 一律走 **ACP（Agent Client Protocol，agentclientprotocol.com，JSON-RPC over stdio）**：kimi 原生，Claude Code 经 `claude-agent-acp`（基于 Agent SDK），Codex 经 `codex-acp`（包 app-server）。OpenCode、Gemini/Antigravity、Qwen Code 也有 ACP 端。**ACP 是"一个客户端驱动所有 CLI agent"的公分母**，控制台第二步接 ACP 即可覆盖三家以外的 CLI。
- **agmsg** 不驱动进程，只在 agent 之间传消息（SQLite + hook），要配合各 CLI 自己的会话。
- **Conductor / Vibe Kanban / Claude Squad** 类：每任务一个 worktree + 一个交互式 CLI 进程（pty/tmux），不做会话级续聊。

### 对控制台的含义

1. MVP 用一次性续聊：`@模型` = 取该模型上次的会话 ID 再起一个进程，三家命令模板已实测。
2. 第二步接 ACP：一套客户端代码覆盖 kimi（原生）+ Claude（官方适配器）+ Codex（社区适配器），并顺带拿到 OpenCode/Qwen 等。
3. 会话 ID 是控制台要持久化的状态之一（每个 @ 一条），与 `.dispatch/` 留痕并列。

Sources: code.claude.com/docs/en/cross-session-messaging · developers.openai.com/codex/app-server · kimi.com/code/docs/en/kimi-code-cli/reference/kimi-acp.html · agentclientprotocol.com/get-started/agents · github.com/agentclientprotocol/claude-agent-acp · github.com/beyond5959/acp-adapter · docs.openclaw.ai/tools/acp-agents

## 补充（2026-08-30）：agent 资源接入怎么做

调查记录 `D:\work\aicross\docs\proto
esources-research.md`。要点：pi 0.84.2 的 `~/.pi/agent/models-store.json` 是 provider→模型两级，`pi --list-models` 可列已鉴权模型，`pi auth` 看 provider 就绪，可自动探测；coffeecli 用静态注册表 + `where/which` 探测 + T1–T3 分档 + `~/.coffee-cli/tools.json` 用户覆写，任务板条目 `{id,title,description,status}` 与资源树无关；FreeBuddy 按二进制名探测、模型是 agent 卡片下的可编辑字段；Vibe Kanban 是 executor→variant（运行预设）两级。

## 补充（2026-09-01）：NyaTerm——终端工作区里已经做完的那半个控制台

> 触发：用户发现 github.com/nyakang/nyaterm「貌似实现了一部分我们想要的功能」。核对结论：**它做完了控制台的管道层（外部 CLI agent 适配、多厂商 provider 与凭据管理、审批门），没碰交叉验证。**

事实来源：仓库代码（main 分支）、`docs-site/docs/guide/ai-assistant.md`、GitHub Releases changelog。**未运行过程序**，界面行为为代码推断。

### 项目事实

- 形态：Tauri 2 + React + Rust 桌面远程终端工作区（SSH / Local / Telnet / Serial / RDP / VNC / SFTP / 隧道 / OTP / 加密同步）。MIT，1.5k star，1641 commit，最新 v1.2.6（2026-08-31）。终端是主体，AI 是其中一个面板。
- 平台：Windows 10/11、macOS 12+、Linux；有 Homebrew / AUR。

### 与 aicross 控制台重叠的部分（均已发版，非 demo）

| 能力 | NyaTerm 的实现 | 对应 aicross 的哪块 |
|---|---|---|
| **本机 CLI agent 适配** | `src-tauri/src/core/ai/codex.rs`（56KB）、`core/ai/external/claude_code.rs`（31KB）。检测可执行文件候选路径 → 探版本 → 报登录状态 → headless 驱动 → 归一成 `ExternalAgentEvent` 流（`SessionStarted` / `TextDelta` / 审批请求 / 文件变更 / 计划项 / 用量）。Codex 走 `codex app-server` 的 stdio JSON-RPC（`CodexAppServerManager` 持长驻进程）。`AiAgentKind = Nyaterm \| Codex \| ClaudeCode` | `cc_switch.py` 那层 + TASK-03 的派发引擎 |
| **多厂商 provider 与凭据管理** | `AiProviderKind` 内置 OpenAI / Anthropic / Gemini / DeepSeek / Groq / Ollama / xAI / Cohere / MiMo / Z.ai + `OpenaiCompatible` 自定义端点；模型可拉取、可手动补录（`AiModelSource = RustGenai \| Manual`）、逐个启停、按 provider 与凭据分组、选默认。`AiApiFormat` 兼容 ChatCompletions 与 Responses | 第 1 期资源接入（TASK-01）的界面等价物 |
| **提议 → 裁决 → 执行** | 命令卡片带 `RiskLevel = Low/Medium/High/Critical`；`AiPermissionMode = Observer/Confirm/Auto/FullAccess`（默认 Confirm）；可配"允许自动执行的最高风险等级"；Critical 要求输入确认文本；高风险时给更安全的替代命令 | G1/G2 闸门 |
| **用量口径** | `AgentUsage` 只报 input/output/total token，不做美元与额度百分比 | 与本项目「界面只显示模型与 token」的定案一致（旁证：这条口径不是我们独有的洁癖） |
| **MCP 双向** | 自带 `nyaterm-mcp` crate 把终端会话作为工具暴露给外部客户端；v1.2.6（#226）加了外部 MCP 的应用内授权确认与权限模式 | 我们「不定义新协议」的另一种落法 |
| **脱敏** | `core/ai/redaction.rs`：`redact_context` / `redact_marker_values` / `redact_sensitive_text`，发送前处理 | 密钥六铁律的工程落点 |

Codex / Claude Code 集成随「文件预览 + 双栏文件管理器」那版发布（changelog：「AI 新增 Codex 集成，支持 Codex 相关设置、会话管理与动态工具调用跟踪 #47」「AI 新增 Claude Code 支持」），后续有 #389 修 Claude Code invocation。

### 它没有的部分

- **一次对话只有一个模型**。前端 `src/components/panel/ai/ModelCombobox.tsx` 是单选；输入框里的 `@` 提及的是**其他终端会话**，不是模型。没有同题多派、没有并排比对、没有交叉验证。
- **没有盲验概念**：`AiPermissionMode` 管的是"能不能动手"，不管"看不看得见"。没有可见性开关，没有空目录隔离、没有 `AI_CROSS_PEER` 一类的对等体标记。
- **没有静默降级核对**：`ClaudeCodeRuntime::auth_status` 只报 CLI 装没装（注释明说不查 OAuth token），全仓未见比对响应体 `model` 字段的动作（`verify_model.py` 干的事）。接第三方 OpenAI 兼容端点时这是真实风险。
- **没有分层派发**：`AiReasoningEffort` 有六档，但是单模型的参数，不是"强模型出方案、便宜模型执行"的编排。
- 场景差一层：它是「一个 AI 帮你操作服务器」，我们是「多个 AI 互相制衡地完成一件事」。它的主体是终端、对话是末端；我们的主体是派发编排。

### 对本项目的含义

1. **核心价值主张没被占**——多厂商 @ 已是大路货（08-30 节），管道层现在也有 MIT 参考实现，唯一没人做的仍是"@ 里有一种是盲的，且有章法"。
2. **管道层自研的必要性下降**。`codex.rs` 的 app-server JSON-RPC 用法、两家的 CLI 候选路径发现与版本探测、审批事件模型、`redaction.rs` 的脱敏点，都可以直接读代码定夺，比继续拿 `claude -p` 试参数快。注意 license：NyaTerm 是 MIT，aicross 是 AGPL-3.0 fork（上游 Coffee CLI），**MIT 代码可并入 AGPL 项目**，反向不可；若逐字复用须保留 MIT 版权声明。
3. **它是 aicross 的一个反面参照**：同样是审批门 + 多 provider + 外部 agent，它止步于"一个模型 + 一个终端"。这条边界正是本产品要越过的那条。

Sources: github.com/nyakang/nyaterm（`src-tauri/src/core/ai/`、`src-tauri/src/config/settings/ai.rs`、`src/components/panel/ai/`、`docs-site/docs/guide/ai-assistant.md`、Releases v1.2.6 及更早 changelog）· nyaterm.app/docs

---

## 2026-09-03 · EvoMap AutoResearch — 自治研究流水线，盲验 + 证据链的最完整公开参考实现

github.com/EvoMap/AutoResearch · Apache-2.0 · Python + TS（ar-runtime）· 2026-08-15 建，09-01 仍在推提交 · 312 star / 28 fork（09-03 查，增长中不采信趋势）。报告自挂 arXiv:2608.17906。定位：AI/ML 研究的自动化流水线，从 idea 到「可写论文的证据包」。

### 形态

两条可独立使用的路径：
- **Idea Pipeline**：多渠道采集（arXiv/HF/GitHub Trending/Reddit/HN/中文媒体等 11 路）→ 去重过滤 → Idea Forge（≥3 个不同模型独立构思 + 交叉评审，≥2 票过）→ 时新性检查 → 实验计划。
- **ar-runtime**：官方 Claude Code CLI + Ralph Loop 把一个 idea 推进为计划、代码、实验、评审。入口是 `claude --dangerously-skip-permissions`，README 明确要求只在一次性隔离环境跑。

### 与 aicross 相关的机制（均有代码可查）

| 机制 | 落点 |
|---|---|
| 独立性按**模型身份**计数 | `ar-critic-contract.ts`：`MIN_INDEPENDENT_CRITICS=2`、`modelIdentity` 去重、`hasIndependentIdentityPair`；README 明说「同一端点可挂多个不同模型，按底层身份计数」 |
| 盲审 | critic 与 blind review 不携带自评上下文；critic 报告绑定 unit+cycle，连同 `artifact_sha256` 记账进工作流引擎，引擎拒绝则整轮失败 |
| 完成条件归引擎不归角色 | 「角色给出的结束建议不能关闭工作流」；`verify-close` 只接受引擎记录能支持的终态；coordinator 无绕过权限 |
| 证据裁决下一步 | Django #16820 案例（其技术文档 2026-07-29 自述）：本地测试全绿但官方新增测试 0/7，沿外部反馈 2/7→4/7→7/7 推进，同时保持 203/203 回归 |
| 负结果是合法终态 | Kaggle Disaster Tweets 连续 21 次实验停在 0.8049（目标 0.8350），停止并留下三条出路；先跑低成本预实验再扩大 |
| 状态可恢复 | `workflow_queue.json` + `state.md` + append-only `decisions.log`；每轮只推进一个工作单元；supervisor 每次 attempt 留 manifest，失败记录不被后续成功覆盖 |
| 来源绑定 | idea 正文固化 + SHA256 绑定 forge 文件/序号/知识方向；项目目录拒绝复用变更后的 idea |
| 冒烟 | `preflight --live` 除可用性外，还查独立性要求与**两轮工具调用**（能发现单轮能通、多 agent 消息就挂的端点） |
| 配置 | 统一 `providers.local.json`：角色→模型别名→路由；配置只存环境变量名不存凭证值 |

### 与 aicross 的关系

- **不是竞品**：headless 自治流水线，API key 驱动，人只在事后看证据包、可接管；aicross 是用户在场裁决、驱动本机 CLI 订阅的派发控制台。决策主体不同：它的答案是「证据决定下一步」，我们是「用户 + 证据」。
- **但它是盲验 + 证据链的第一份公开工程参考**。此前 09-01 节说「唯一没人做的是 @ 里有一种是盲的且有章法」，说法要收窄：**盲审的章法在自治研究场景已有实现**（AutoResearch），没人做的剩「跨本机 CLI、用户在场裁决」这个形态里的盲审。
- 它的 critic 契约（报告绑定工作单元 + 哈希记账 + 引擎验收）可直接作为 TASK-03 验收链的参考设计；它的平台层问题（续聊、隔离、`--restricted`、宿主注入）一个都没碰，因为整体跑在一个 Claude Code 里。
- 可作文章素材：「本地测试全绿、官方 0/7」是「为什么要外接验证者」的公开实例，出自其技术文档而非我们的基准。

Sources: github.com/EvoMap/AutoResearch（main 分支 `ARCHITECTURE.md`、`ar-runtime/scripts/ar-critic-contract.ts`、`ar-runtime/ar-coordinator-startup-flow.md`、README）· evomap.ai/zh/research/autoresearch-evidence-loop（2026-07-29）

---

## 2026-09-13 · PiX — 会话是一张图：分叉即派发的形态参考

github.com/huang-sh/PiX · 无 license · Electron + Vite + TS · 2026-09-04 建，09-13 仍在推 · 55 star / 4 fork（09-13 查）· 3 位贡献者。自述：「非线性的 AI Agent 工作台——会话是一张图，随时分叉，上下文跟随分支」。底座是 pi 编码 agent（本文件 08-30 节的 earendil-works/pi），README 明说「会话就是真实的 Pi 会话，不发明新格式，离开 PiX 也能继续使用」；随包带 pi 扩展 `@injaneity/pi-computer-use`、`@ff-labs/pi-fff`，`pi-web-access` 可选装。支持 SSH / WSL 远端运行时（Node ≥ 22.19）。

### 形态

- **节点 = 一轮对话**（提问 + 回复 + 工具调用），图从左向右生长。
- **分叉是日常动作**：从任意一条用户消息 fork；克隆分支作存档点；从历史节点续跑。走不通的分支留在图上。
- **上下文跟随分支**：切分支时聊天记录立刻跟着切，不用重新配上下文，因为上下文本来就是根到分叉点的路径。
- **单击 / 双击分工**：单击只在图上高亮，图上命令（如 /fork）作用于高亮节点；双击才把主聊天面板对齐到该节点。
- **最多 3 列并排**：Ctrl+双击把分支固定到第二、三聊天列，各自独立滚动、独立回复；同一分支只保留一个面板。
- 上下文面板显示选中节点的思考过程、工具调用、耗时与步骤数。

### 与 aicross 的关系

- **不是竞品**：单 agent（pi）的会话树，一张图上只有一个模型身份在换分支；没有跨厂商、没有并排比对的判分、没有盲验概念。
- **是中栏形态的直接参照**（已采纳，`aicross/docs/PRODUCT.md` 09-13 定案「对话即图」）：扇出比对 = 同一节点 fork N 份 + 三列并排；盲审 = 选分叉点；预设 = 在节点上盖子图。它把「对话是末端」这条 08-30 定案推翻了。
- **两处借不动**：① PiX 是树，README 没有合并、没有多父节点；aicross 的验收要收多条分支的产物，必须扩成 DAG 加扇入节点。② PiX 的分支是同一个 pi 会话的分叉，「上下文跟随分支」是免费的；aicross 的分支是不同厂商的 CLI 进程，切分支等于把路径序列化成 prompt 喂新进程，有损（丢 thinking，见本文件 08-30 节对 pi 跨厂商迁移的论证）。
- 无 license，只取形态不取代码，与 coffeecli / ThinkRail 同样处理。

Sources: github.com/huang-sh/PiX（README.md main 分支，09-13 读）· api.github.com/repos/huang-sh/PiX（license null、created 2026-09-04、pushed 2026-09-13）

---

## 2026-09-14 · 研究侧：多 agent 并行、一致性与交叉验证

不是竞品，是设计依据。起因：arXiv 2502.02533（MASS）和一段流传的「多 agent 一致性」面试复盘（状态机 + 幂等 + 补偿）。下表数字除标注外均读自 arXiv 摘要页，未读全文的实验细节不采信。

| 研究 | 事实 | 边界 |
|---|---|---|
| **MASS**（Zhou 等，Google，2502.02533，ICLR 2026） | 三段优化：单 agent 提示词 → 拓扑 → 整体提示词。有效拓扑只占设计空间一小部分，且随任务变：多跳问答辩论约 +3%、其余结构掉分；数学并行汇总最有用；代码反思 + 真跑代码最有用。默认提示词下加 agent 数会饱和或变差。Gemini 1.5 Pro 平均 78.79%，多 agent 辩论基线 70.26% | 同厂商旧模型；靠验证集自动搜拓扑 |
| **Towards a Science of Scaling Agent Systems**（Kim 等，2512.08296，v3 2026-04-08） | 5 种架构 × 3 个模型家族 × 180 配置。可拆任务最高 +80.8%，串行规划最低 −70.0%；工具密集任务有多 agent 惩罚；单 agent 基线越强，加 agent 收益越小；无中心验证的架构误差传播更多（Google 博客给出独立并行 17.2 倍、中心化 4.4 倍） | 预测模型 R² 约 0.37–0.51 |
| **CAID**（Geng & Neubig，2603.21489，v2 2026-07） | 集中派发 + 异步执行 + 每人一个 git worktree，靠 commit/merge 汇合。PaperBench +25.6%、Commit0 +14.7%（相对单 agent） | 仍自述：并发编辑互扰、依赖难同步、部分进度难合并 |
| **STORM**（Liu 等，2605.20563） | 不隔离工作区，在写入时检测并解决冲突。Commit0-Lite 比 worktree 基线 +18.7，PaperBench +1.4 | 预印本 |
| **AgentRoom**（2608.23740） | CRDT 共享文件系统 + 文件级 claim/status/broadcast 作为 MCP 工具；5 个编码 CLI 模型，2 agent 比单 agent 放弃任务少、方差小 | 仅对「CLI 稳定」的模型成立；未读摘要页，取自检索摘要 |
| **Correlated Errors in LLMs**（Kim 等，2506.07962，ICML 2025） | 350+ 模型：同一榜单上两模型都错时 60% 错得一样；共享架构、同厂商会加大相关；**越强的模型错误越相关，跨架构跨厂商也一样** | 选择题为主 |
| **Beyond Self-Checking**（OpenReview，ICLR 2026 VerifAI workshop） | 跨家族验证者与生成者的相关 ρ=0.54，同家族 0.77；据此做低相关配对路由 | 未读全文，数字取自页面摘要 |
| **The Cost of Consensus**（Bertalanič & Fortuna，2605.00914） | 10 个同构模型 3 轮辩论：跟随多数最高 85.5%，同伴理由把正确推理带偏最高 70.0%，投票丢掉池中已有正确答案的差距 32.3pp；token 多 2.1–3.4 倍，准确率不高于各自独立自我修正 | 只测 7–8B 同构模型、无角色辩论 |
| **Early Diagnosis of Wasted Computation**（Li 等，2606.01365） | 失败运行里第一次预警之后平均还花 58.1% token；预警后干预把这部分从 63.8% 降到 30.4%。信号：循环、预算压力、低信息增益、工具不稳 | 165 条轨迹 |
| **MAST**（Cemri 等，2503.13657，v3 2025-10） | 1642 条轨迹、7 个框架、14 种失败模式，κ=0.88。三类约占：系统设计 44%、agent 间未对齐 32%、任务验证 24% | 早期版本比例不同，引用以 v3 为准 |

### 对 aicross 的含义

- **已被印证**：星形拓扑 + 扇入只有验收（中心验证抑制误差传播）；盲审不走辩论、G3 独有发现单列不自动取多数（跟随多数与共识塌缩）；同一目录单写者；蜂群排第 3 期（串行任务与工具密集任务多 agent 吃亏）。
- **要收窄的说法**：「失败模式独立的 verifier」→「错误相关性更低的 verifier」。跨厂商降低相关、不消除，强模型尤甚；能跑的测试、GT、原始材料核对仍是第一验证手段。
- **面试那套方案的取舍**：分布式事务表、调度中心高可用、语义指纹去重、归档与重复率大盘不适用于单用户桌面 + 用户裁决的形态；取其中两个问题——重派是否重复生效、崩溃后怎么对账——落到 `aicross/docs/PRODUCT.md`「未解的设计问题」（09-14）。异构 CLI 无统一事务协议的答案是：效果只落 worktree 里的 diff，采纳即提交。
- **蜂群做法**：先 CAID（已有 worktree），拆分时声明文件范围代替语义去重，逐个合并逐个跑测试；冲突多再看 STORM 式写时检测。
- **预设按任务类型给默认结构**（MASS）：代码 = 执行 + 真跑测试的验收；数值 = 扇出汇总；事实性 = 仲裁不辩论。扇出 ≤3 路，先调角色提示词再加路数。

Sources: arxiv.org/abs/2502.02533 · arxiv.org/abs/2512.08296 · research.google/blog/towards-a-science-of-scaling-agent-systems-when-and-why-agent-systems-work/ · arxiv.org/abs/2603.21489 · arxiv.org/abs/2605.20563 · arxiv.org/abs/2608.23740 · arxiv.org/abs/2506.07962 · openreview.net/forum?id=U19s6I8Q0u · arxiv.org/abs/2605.00914 · arxiv.org/abs/2606.01365 · arxiv.org/html/2503.13657（均 09-14 读）

---

## 2026-09-16 · Cursor Projects（beta，2026-09-10 发布）

用户转来一张中文拆解图，说「和我们的想法很相似」。以下取自官方公告、changelog、子代理文档与两篇上手文章；官方没说的标「未说明」。

| 项 | 事实 |
|---|---|
| 是什么 | 面向一整块工作（功能、迁移、整个应用）的长期工作区；一条长期线程里只跟一个**协调智能体**说话 |
| 协调者 | 「doesn't write code itself but directs other agents that do」——只规划、委派、把结果拿回给用户检查；因此「never blocked and always responsive」 |
| 执行 | 默认云端，一个 Project 一台机器，关电脑不停；需要在本机测时起本地 agent；官方称可并行「thousands of subagents」，上手文章实测是每个协调者管几十个 |
| 共享上下文 | Project 带一组文件，在它用到的所有云端与本地机器间同步；agent 往里写调研、产物、对代码库的认识和用户偏好，后续 agent 复用 |
| 订阅 | 协调者可盯 Slack 频道、定时跑、跟踪全部 PR（修 CI、开合时触发） |
| 创建 | 侧栏 Projects「+」→ 绑定 GitHub 仓库作 Workspace → 选 Model（拆解图里显示 `High`，疑为推理档）→ 创建；工作流偏 PR，不走 PR 的仓库用处小 |
| 看过程 | 可打开任一子 agent 的 tab 实时看，协调者线程同时可对话 |
| 验证 | 用户审 PR；协调者不自动推 main。官方没有内置验证者或盲审；子代理文档里 verifier 只是自定义示例 |
| 模型 | Projects 公告未说明协调者与子 agent 用什么模型、能否混用。子代理文档：`model` 默认 `inherit`，可写具体 ID（`composer-2`、`claude-opus-5[effort=high]` 等），另有 `readonly`、`is_background` |
| 计费 | 付费计划可用；协调者云机器与子 agent 另计 token，建议先设美元上限 |
| 官方数字 | 新用户合并 PR 多 30%，主要用 Projects 的用户多 6 倍（未给口径） |

### 与 aicross 的关系

- **重合的是形态，不是核心主张。**「一个对话窗 + 只派不做的主控 + 子 agent 并行 + 可点开看过程」与 `aicross/docs/PRODUCT.md` 09-15「为什么总指挥只派不做」、09-16「控制台就是对话窗」几乎同一结论——这是外部印证，不是被抄。
- **不重合的三处**：① 跨厂商——Cursor 模型全在自家计费里转，没有「验收换厂商」「盲审」这类默认值，验证交给用户审 PR；aicross 的核心是错误相关性更低的 verifier。② 资源——Cursor 用它自己的 token 计费，aicross 调用户已有的各家订阅 CLI（Claude Code / Codex / Kimi / pi）。③ 裁决——Cursor 协调者自主派发，人在 PR 处把关；aicross 是派发前、收结果时两道闸门，建议条采纳才发（09-16 定案）。
- **值得借的**：共享上下文文件（agent 写回「怎么测这个服务」一类经验，后续复用），对应 aicross 目前手写的 Runbook 与资源页模型备注（第 4 期第 7 条）；订阅触发（盯 PR / CI）目前不在 aicross 范围，记为 Later。
- **不借的**：云端常驻与「上千子 agent」——与单用户桌面、同一目录单写者、蜂群排第 3 期的定位冲突，且研究侧（本文件 09-14 节）显示串行任务与工具密集任务加 agent 吃亏。

Sources: cursor.com/blog/projects · cursor.com/changelog/projects · cursor.com/docs/subagents · flaviocopes.com/cursor-projects/ · byteiota.com/cursor-projects-the-coordinator-agent-that-ships-code-while-you-sleep/（均 09-16 读）

---

## 2026-09-18 · Yoda（lovstudio/yoda）——多 CLI 统一工作区

Electron 桌面应用，Apache 2.0，2026-05-10 建仓，09-04 仍在推送，71 star。自称「Your Orchestra of Delegated Agents」「Harness for your Agent Workspace」。

| 项 | 事实 |
|---|---|
| 接入 | 32 种编码 CLI（Claude Code、Codex、Gemini、Kimi、Pi、Qwen Code、Antigravity 等），注册表在 `src/shared/runtime-registry.ts`；另可经 ZenMux / OpenRouter 用第三方 token 跑 |
| 派发单位 | 客户端（CLI 进程，PTY + xterm 呈现）。「Agent」= System Prompt + Skills + 首选模型，不绑定客户端 |
| 运行模式 | 文档写 normal / brainstorm / compare / review / team；源码 `src/shared/paradigms/kinds.ts` 已收敛为 `single` / `team` / `compare` 三种，brainstorm、review 列为 retired run mode |
| compare | 每个变体各开一个独立任务（可各自 worktree），并排平铺窗口给人比；不做自动判分 |
| review | 现为 team 的一个预设 `review-loop`（`src/main/core/team-rooms/presets.ts`）：lead + implementer + reviewer。reviewer 提示词「Do not modify files — only inspect the worktree」，与 implementer 同一 worktree，读 implementer 的交接消息，循环到 PASS |
| team 拓扑 | Team Room：成员通过 `team-at` 脚本互相 @，conductor 把消息直接注入被 @ 成员的活会话；有跳数上限（`team-routing-limit`）。通信可选 process / message-hub（默认）/ shared-file（`.yoda/team/shared-handoff.md`）/ github issue·PR |
| 隔离 | 主分支直跑或每任务一个 git worktree |
| 其他 | Skills/Hooks/Memory 逐项查看与覆写；Linear/Jira/GitHub 等 13 种工单直接转会话；定时任务；SSH 远程（早期）；手机端经桌面网关 + 公网中继 |
| 验证 | 没有「验收换厂商」「盲审」的默认值；reviewer 选哪个客户端由用户手配 |

### 与 aicross 的关系

- **重合**：一个桌面应用接各家已装 CLI、按任务选、compare 并排、可跨厂商配 reviewer。接入面与界面形态它已做得很宽，aicross 不在「接得多」上竞争。
- **与 Coffee CLI 同一品类**（多 CLI 桌面工作区，tab 里跑原生 TUI）。Coffee 是 Tauri，重在终端体验（T1 气泡与权限卡、灵动岛、Gambit、11 种语言、面向非开发者）；Yoda 是 Electron，重在编排（worktree、compare、team room、工单）。aicross fork 自 Coffee，在它上面加的派发、盲审、蜂群、worktree 合并，正好落在 Yoda 已做的那一侧。
- **不重合**：① 验证者可见性——Yoda reviewer 共享 worktree 与执行者交接消息，是带上下文的复查；aicross 默认盲验、空目录、只给原始材料。② 拓扑——Yoda 成员互相 @，conductor 转发，靠跳数上限防循环；aicross 是星形，被派方互不可见。③ 裁决——Yoda 预设循环里 implementer ↔ reviewer 自行来回直到 PASS；aicross 派发前、收结果两道人工闸门。④ 真身核对——没看到类似 `verify_model.py` 的模型身份检查，经 MaaS 跑时存在静默降级风险（未实测）。
- **值得借的**：Skills/Hooks/Memory 透视（「agent 变笨多半出在这层」）；Agent 与客户端解耦的定义方式；compare 的平铺窗口。
- **不借的**：32 种客户端全覆盖、工单集成、手机端——与「一个引擎一个预设地加」的范围冲突。

### 补记（09-18 下午）：Yoda 怎么做到「原生界面 + 互相 @」

- 每个成员在 PTY 里跑各家原生 TUI，slash、技能、MCP、权限弹窗都是原生的。
- @ 投递：conductor 把消息以粘贴方式写进目标的 PTY，等至少 300 ms 再发回车（`src/main/core/conversations/inject-prompt.ts`，注释写明 Claude Code 的回车会抢在粘贴完成前发出）。
- 回复回流：不是 Yoda 读屏，而是成员自己用 Bash 工具跑写死 ptyId 的 `team-at` 脚本，curl 本机 hook 端点（`team-rooms/team-at-script.ts`）。
- 判断「这一轮结束」靠每家各一套运行态来源：hooks、转录文件追踪、静默对账、中断嗅探（`conversations/` 下 `claude-run-state-source`、`agent-silence-reconciler`、`claude-interrupt-sniffer`、`codex-history-projection-repair` 等）——这是该路线的维护成本。

Sources: github.com/lovstudio/yoda（README、`src/shared/paradigms/kinds.ts`、`src/main/core/team-rooms/{presets,conductor}.ts`、`src/shared/team-communication.ts`、`src/renderer/features/paradigms/kinds/compare/launch.ts`）· yoda.lovstudio.ai/docs/concepts/run-modes（均 09-18 读）

## 2026-09-21 · Ekko Studio（EKKOLearnAI/hermes-studio，原 Hermes Studio / Hermes Web UI）

本地优先的多 agent 工作台，Electron 桌面 + 自托管 Web（npm CLI、Docker）。2026-04-11 建仓，v0.7.23（09-18），09-21 仍在推送，约 1.1 万 star、1,500+ commit。**BSL-1.1**：非商业可用，商用需授权，2029-05-10 转 Apache-2.0。技术栈 Vue 3 + Naive UI / Koa 2 + Socket.IO + SQLite + node-pty。

| 项 | 事实 |
|---|---|
| 接入 | 三族 runtime：Hermes Agent；自研 Ekko；Coding 族 Claude Code、Codex、Pi、Grok、OpenCode、DSH（DeepSeek Harness）。模块按族分（`packages/server/src/modules/{studio,hermes,ekko,coding-agents}`） |
| 拉起方式 | 每轮一个进程 + 原生会话续接，与 aicross 09-01 定案同路：claude `-p --output-format stream-json` 配 `--resume` / `--session-id`；codex `exec … -` / `exec resume … <id> -`，prompt 走 stdin；pi 走 RPC JSONL，以 `agent_settled` 为轮次边界；DSH 走 ACP（`services/runtime/run-manager.ts`）。codex 带 `--dangerously-bypass-approvals-and-sandbox` |
| 两种模式 | `scoped`：Studio 生成隔离的 profile/provider/agent 配置目录，CLI 指向**本机代理**；`global`：用用户自己装好的 CLI 与其原生配置 |
| 本机代理 | scoped 下 CLI 不直连上游：Claude Code 打本地 Anthropic 兼容代理，Codex / Pi 打本地 Responses 兼容代理；代理做三种协议互转（Messages / Chat Completions / Responses），把上游 SSE **一分为二**，一路回 CLI，一路归一成 Responses 事件写库、推前端、记用量。上游 key 不写进 CLI 配置，CLI 只拿到每目标一个的本地令牌（`docs/agent-runner.md`） |
| 群聊 | `@name` 路由，agent 之间也可 @ 交接；每个 agent 看到的历史是**投影**：自己说的记 assistant，其余人与工具结果一律记 user 并加 `[发送者]: ` 前缀，@ 从正文里剥掉；超阈值用结构化摘要压缩（当前话题 / 已有结论 / 待回应 / 参与者），增量更新（`group-chat/context-projection.ts`、`agent-prompt.ts`）。防空转靠提示词（「无人需要行动就别 @」） |
| 上下文阈值 | 一个压缩阈值（默认 0.5）翻译成六家各自的原生压缩设置：Codex `model_auto_compact_token_limit`、Claude `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE` 等，并记下各家上限（Codex 封顶 90%）（`docs/chat-chain-changes/2026-09-21-coding-agent-context-policy.md`） |
| 工作流 | Vue Flow 画布：有向边、条件、成功/失败分支、循环、审批门、定时；运行历史带冻结快照可回放 |
| 审批 | 工具审批经 Socket.IO `approval.requested` / `approval.respond` 回到聊天界面；pi 的扩展 UI 请求同路 |
| 其他 | 10 个 IM 平台接入、语音、看板、Web 终端、桌面 agent 浏览器、MCP 统一管理（Studio 自带的 MCP 与用户的分开标记）、30 天用量与成本估算、多用户与角色 |
| 验证 | 没有「验收换厂商」「盲审」默认值，没有模型真身核对 |

### 与 aicross 的关系

- **重合**：一个窗口 @ 多家 agent；每轮一进程 + 原生会话续接；工具审批回到消息流。后两条是对 aicross 09-01 / 09-18 定案的外部印证。
- **不重合**：① 它是全家桶（IM、语音、看板、画布、多用户），aicross 是单用户桌面派发台；② 群聊是共享上下文、agent 互 @，aicross 把这一类标「各方结论不独立」、不作验证用，拓扑是星形；③ 它的画布是自由连线，aicross 明确不做；④ 订阅：scoped 模式经本地代理走 API key，属于 aicross「不复用第三方对订阅 OAuth 的封装、只拉官方 CLI」的另一条路；global 模式才用用户已有的 CLI 登录。
- **值得借的**：
  1. **历史投影的写法**——「自己 = assistant，别人 = user + `[名字]: ` 前缀，剥 @，工具调用截断到 4000 字符，摘要用一问一答垫在最前」。这是 PRODUCT.md「未解的设计问题 · 跨 CLI 的路径序列化」的一份现成答卷，可直接对照着定各段怎么标。盲节点在此基础上再剔模型回复即可。
  2. **摘要模板**的四栏结构，可用于长路径超长时的压缩。
  3. **一个阈值翻译成各家原生压缩设置**的对照表——aicross 单线续聊跑久了会撞各家自动压缩，这张表省一轮调查。
  4. **canonical 事件**：把各家流统一成一种内部事件再分发给界面 / 落盘 / 用量。aicross 已有 `Reply` 契约，ACP 接入后若事件种类变多可参照。
- **不借的**：本地代理夹在 CLI 与上游之间（为拿流与用量而做协议互转，维护面大；aicross 走 pi 已拿到 `responseModel` 与 token）；codex 全程 bypass 审批与沙箱（与写护栏、两道闸门相反）；自由画布、IM、语音、多用户。
- **license**：BSL-1.1 且 aicross 是 AGPL-3.0，**只读思路不抄代码**。

Sources: github.com/EKKOLearnAI/hermes-studio（README、LICENSE、`docs/agent-runner.md`、`docs/cli-chat-sessions.md`、`docs/chat-chain-changes/2026-09-21-coding-agent-context-policy.md`、`packages/server/src/modules/coding-agents/services/runtime/run-manager.ts`、`packages/server/src/modules/studio/services/group-chat/{context-projection,agent-prompt}.ts`）· GitHub API 仓库元数据（均 09-21 读，未装机实测）

## 2026-09-27 · herdr（herdrdev/herdr）——agent 的终端运行时

「the runtime your coding agents live on」。Rust 单二进制 TUI，跑在用户已有的终端里，tmux 式前缀键 + 鼠标。2026-03-27 建仓，v0.4.0，09-27 仍在推送，约 4.1 万 star。**Apache-2.0**。

| 项 | 事实 |
|---|---|
| 定位 | 不包装 agent，只托管它们的终端（「doesn't wrap or replace them; it owns their terminals」）。Claude Code、Codex、Cursor、OpenCode、Grok、Kimi Code 等 25+ 家原样跑在 pane 里 |
| 常驻 | 后台 server 持有终端，关客户端或 SSH 断线不停；重启后恢复布局，支持的 agent 按原生会话 id 续接（原进程不保留） |
| 多机 | 本地与多台 SSH 机器合进一个窗口，统一 agent 列表，各自重连 |
| 状态 | 每个 pane 标 working / blocked / idle。有完整生命周期 hook 的 agent 以 hook 为准；否则读前台进程与底部屏幕快照，用 TOML 清单匹配判定 |
| agent 互驱 | 自带 agent skill，pane 内 `HERDR_ENV=1` 时 agent 可调 CLI / socket API：`pane split`、`pane run`、`pane read`、`pane wait-output`、`agent wait`——开 pane、给别的 agent 发提示、读其输出、等它真的卡住 |
| 其他 | 插件市场；Windows 为 beta |
| 验证 | 没有角色、盲审、验收换厂商、模型真身核对；互驱是共享可见的 |

### 与 aicross 的关系

- **重合**：多家官方 CLI 原样拉起、不碰订阅 OAuth；按原生会话 id 续接；「哪一路卡住等人」一眼可见。
- **不重合**：① herdr 是终端复用器（tmux 的 agent 版），中心是 pane；aicross 的中心是自己的消息流，「不托管终端」写在「不做」里，原生 TUI 只作 agent 窗口兜底；② herdr 的编排由 agent 自己驱动（agent 读别人的屏幕、给别人发话），拓扑是网状、互相可见；aicross 是用户裁决、星形、被派方互不可见，验证者默认盲；③ herdr 没有模型层——它只认壳，不管壳里跑的是哪家模型、是否被静默降级；aicross 派发目标是模型、按厂商算独立性；④ 常驻 server、多机 SSH、屏幕状态识别是 aicross 没有也没排期的。
- **值得借的**：
  1. **状态三态 + hook 优先、屏幕兜底**的判定办法——aicross 的 agent 窗口与 ACP 单线都需要「这一路在等审批/等回答」的信号，可照这个分层做。
  2. **「真的卡住才返回」的 `agent wait` 语义**，对应流水线里「上一阶段结束才发下一阶段」的判定。
  3. 它的 TOML 屏幕清单（各家 TUI 的 idle/blocked 特征）是现成的数据，Apache-2.0 可引用。
- **不借的**：agent 互驱（同一上下文里互相读屏发话，结论不独立，与盲审相反）；常驻 server 与多机（aicross 09-01 定一次性续聊，长驻放第 5 期）。

Sources: github.com/herdrdev/herdr（README）· herdr.dev/docs/agent-skill/、herdr.dev/docs/agents/ · GitHub API 仓库元数据（均 09-27 读，未装机实测）

## 2026-09-30 · Omnigent（omnigent-ai/omnigent）——多 harness 的元编排层

「The open-source meta-harness for all your AI agents」。Python，2026-06-11 建仓，v0.16.0（09-29），alpha，约 1 万 star，**Apache-2.0**。主要贡献者 PattaraS、serena-ruan、TomeHirata、dbczumar，Databricks 是一等集成（模型源、部署、沙箱）。

| 项 | 事实 |
|---|---|
| 定位 | 在 Claude Code、Codex、Cursor、OpenCode、Hermes、Pi、Antigravity（`agy`）、Copilot、Grok Build、Devin、Kiro 之上加一层统一编排，同一会话里混用多家，换壳不改 agent 定义 |
| 形态 | 本地 server + web UI（`localhost:6767`）+ macOS 桌面壳；会话可在终端、浏览器、手机间接续；多用户、分享、共驾（`attach`）、分叉（`--fork`） |
| 凭据 | API key / Claude 与 ChatGPT 订阅（经官方 CLI）/ 任意兼容网关 base_url / Databricks；首次运行读已有环境变量和已登录 CLI |
| agent 定义 | YAML：prompt + 工具（Python 函数、MCP、子 agent）+ `executor.harness`；agent 可以写 agent |
| Polly 示例 | 编排者不写代码，拆任务派给 7 个 CLI 子 agent，各自 git worktree、各开 PR；**每个 diff 交给与作者不同厂商的审查者**，审查者只收 diff 文件、base/head SHA 和验收契约，只读；首轮用 `configured_harnesses` 做就绪预检（binary-missing / needs-auth / version-too-low）；`pi` 是只读审查/探索专员，也是唯一能跑任意网关模型的 worker |
| Debby 示例 | 一 Claude 一 GPT 两个头，同题并排作答，`/debate` 互评后收敛 |
| 治理 | policy 三级叠加（server / agent / session）：执行前暂停待批、工具调用次数上限、美元预算上限 |
| 其他 | 12 家云沙箱、定时自动化、智能路由（`sys_advise_models` 按任务推荐模型） |
| Windows | 降级：只有 server、web UI 和 SDK 类 harness；原生 `omnigent claude/codex` 终端包装（tmux/PTY）和文件/网络沙箱不可用 |
| 验证 | 没有模型真身核对：网关模型名只校验组合是否合法，未见对响应 `model` 字段的比对 |

### 与 aicross 的关系

- **重合度是目前记过的竞品里最高的**：多厂商官方 CLI 拉起、订阅走官方 CLI、pi 作网关模型通道和只读审查者、`agy` 进厂商轴、worktree 分写、**验收换厂商**、审查者只收原始材料——aicross 经济模式与盲审模式的核心机制，Polly 作为一个示例 agent 已经跑通并发版。
- **不重合**：
  1. **谁编排**：Polly 是模型当 tech lead，自主拆解、自主派发、自主选审查者，人只在最后 merge；aicross 是模型提议、用户在两道闸门裁决。
  2. **盲的程度**：Polly 的审查者材料是 diff + 契约，属于「只收指定上游」；aicross 另有空目录隔离、`--no-context-files` 挡用户级规则文件、同材料多家并排、独有发现单列。Debby 的 `/debate` 是互相可见的收敛，按 aicross 口径结论不独立。
  3. **模型层**：Omnigent 管到「哪个 harness + 哪个模型名」，不查端点是否静默降级；aicross 的冒烟判据是「回答的是不是它」。
  4. **范围**：多用户、手机、云沙箱、定时任务、美元预算是 Omnigent 的主卖点，aicross 的「不做」和「成本核算不是本产品的事」都排除了。
  5. **平台**：用户主力机是 Windows，Omnigent 在 Windows 上缺原生终端包装和沙箱；aicross 是 Tauri 桌面应用，Windows 是一等平台。
- **值得借的**：
  1. **就绪预检的状态枚举**（`true / binary-missing / needs-auth / version-too-low`）——比 aicross 接入页现在的「已接入 / 待冒烟 / 未接入」多分出了失败原因，可直接对应卡片上的提示文案。
  2. **审查派发的材料契约**：diff 存文件传绝对路径 + base/head SHA + 验收契约，审查者只读——可作为 aicross 验收节点的默认输入格式。
  3. **派发标题按任务命名、不按厂商命名**的规则，对应对话图节点标题。
- **不借的**：编排者自主派发（与用户裁决相反）；policy 的美元预算（成本核算不在范围）；多用户 server 与云沙箱。
- **对定位的影响**：「多厂商 CLI 统一编排 + 跨厂商审查」已不是空位，且有大团队、Apache-2.0、快速迭代。aicross 剩下能说清的差异是 **用户裁决而非模型自治、默认盲验与隔离分级、模型真身核对、Windows 桌面原生**。对外介绍时不再以「跨厂商审查」本身作为卖点。

Sources: github.com/omnigent-ai/omnigent（README、examples/polly/config.yaml）· GitHub API 仓库元数据、contributors、releases（均 09-30 读，未装机实测）

## 2026-09-30 · CodePilot（op7418/CodePilot，作者歸藏）——多模型聊天客户端，子 agent 在对话里派

「A multi-model AI agent desktop client」。Electron + Next.js + SQLite，2026-02-06 建仓，v0.67.17（09-22），约 6500 star，macOS / Windows / Linux 均有安装包。**BSL-1.1**：个人与非商业免费，商用需授权，2029-03-16 转 Apache-2.0。

| 项 | 事实 |
|---|---|
| 形态 | 一个聊天窗为主（Code / Plan / Ask 三种模式），侧栏放文件树、Git、agent 运行详情；可分屏两个会话 |
| 模型接入 | 17+ provider 预设（Anthropic、OpenRouter、Bedrock、Vertex、智谱、Kimi、MiniMax、火山、百炼、MiMo、Ollama、LiteLLM、任意兼容端点），填 API key 即用；自带 provider 诊断 |
| 运行时 | 三种：自研 CodePilot Runtime、Claude Code（经 Claude Agent SDK）、Codex（经 app-server）。Claude Code CLI 可选，装了才能改文件、跑命令 |
| 多模型子 agent | 父 agent 在对话中调 `codepilot_spawn_subagent`，显式给 provider + model；**只在同一运行时内**（如 Claude Code 运行时里跑 Kimi / GLM / DeepSeek），深度 1，每会话并发 2；支持声明依赖（DAG），下游等上游完成才启动，上游结果作为「不可信任务数据」注入。跨运行时调度未做 |
| 界面 | 子 agent 在聊天里显示为一行胶囊（模型图标 → 名称 → 状态 → 详情），不显示提示词；点详情在侧栏开该 run 的 tab |
| 可信性 | 终态只认真实结束信号：`is_error` 与 `api_error_status` 都查（`subtype=success` 但 403 判失败）；记录请求模型与实际模型（取 SDK init 上报），分开显示 |
| 其他 | 手机经 Telegram / 飞书 / QQ / 微信 等远程控制；助理人格与记忆文件；生成式 UI；图片生成；定时任务；token 与成本统计 |
| 验证 | 子 agent 默认继承父会话的工具与权限，是共享上下文的执行者；没有盲审、换厂商验收的默认值 |

### 与 aicross 的关系

- **形态上最接近「skill 当界面」**：用户只在一个聊天窗里说话，由模型在对话中派子 agent，结果以一行胶囊回到对话，要细看再开侧栏。这正是 aicross 09-16 定的「控制台就是对话窗」，但它没有另起一张对话图、没有编排面板，派发的全部可见界面就是胶囊 + 详情 tab。
- **不重合**：
  1. 派发由模型直接执行，没有「建议 + 用户采纳」的闸门。
  2. 子 agent 共享父会话权限与上下文，没有盲验、隔离分级和并排比对；「换模型」不等于「换验证者」。
  3. 跨厂商靠同一运行时换 provider（Claude Code 外壳跑 Kimi），不是拉起各家官方 CLI；各家订阅 OAuth 不在它的接入模型里（Codex Account 除外）。
  4. 模型接入走 API key，与 aicross「调用户已有订阅 CLI」的资源口径不同。
- **值得借的**：
  1. **胶囊 + 详情侧栏**的呈现：派发在对话里只占一行，过程与提示词进详情。对照 aicross 的对话图、右栏编排，这是更轻的一档。
  2. **请求模型与实际模型分开显示**，拿不到实际模型时标「未报告」，不回填请求值——与 aicross 真身核对口径一致，可直接对照界面文案。
  3. 依赖任务的约束：没声明依赖却要求子 agent「等待」的调用直接拒绝，不建占位 worker。
- **不借的**：代码（BSL-1.1）；助理人格、记忆、图片、远程控制等通用助理功能。

Sources: github.com/op7418/CodePilot（README、ARCHITECTURE.md、docs/handover/same-runtime-multi-model-subagents.md，后者状态行为 2026-07-25）· GitHub API 仓库元数据与 releases（均 09-30 读，未装机实测）

## 2026-10-07 · mattpocock/skills（Matt Pocock）——工程纪律类 skill 合集

| 项 | 内容 |
|---|---|
| 规模 | 约 27.8 万星，MIT，package 版本 1.3.1；promoted 两类（engineering / productivity）约 24 个 skill，另有 misc（冻结）、in-progress、deprecated |
| 定位 | 「Skills For Real Engineers」：小而可组合、与模型无关，把对齐（grill）、共享术语（GLOSSARY + ADR）、TDD、诊断、架构加深等工程纪律写成 skill；明确反对 GSD / BMAD / Spec-Kit 那种接管流程的框架 |
| 调用分轴 | 每个 skill 二选一：user-invoked（`disable-model-invocation: true` + Codex `agents/openai.yaml` 的 `policy.allow_implicit_invocation: false`，描述写给人看，负责编排）或 model-invoked（描述带触发词，承载可复用纪律）。user-invoked 可调 model-invoked，反之不行；skill 之间写「Call the Skill tool with "x"」 |
| 分发 | 两条互斥路线：Claude Code 官方 marketplace 插件（只读订阅，版本由 Anthropic 手动移 sha 钉）；skills.sh `npx skills@latest add mattpocock/skills`（拷成可改文件，跨 Codex 等）。Codex 原生插件因 manifest 只认单路径而暂缓（ADR 0002） |
| 多 agent | `code-review` 把 Standards（规范 + Fowler 坏味道基线）与 Spec（是否忠实实现需求）拆成两个并行子 agent，互不污染、不合并排序；`implement-spec` 按任务图并行派实现子 agent，每个独立 worktree，合到集成分支后统一 code-review。子 agent 都是同一个模型 |
| 治理 | `SCOPE.md` 收 issue 的门槛（真实会话里观察到的失败 + 符合理念）；`.out-of-scope/` 每条否决一个文件；ADR、changesets、CHANGELOG；每个 promoted skill 有 docs 页，固定四节 What it does / When to reach for it / Common questions / It's working if。不在 skill 里写防子 agent 递归的护栏，认为是 harness 的事 |

### 与 ai-cross / aicross 的关系

- **相同**：SKILL.md + 按需加载的参考文件；跨 Claude Code 与 Codex；用并行、隔离上下文的子 agent 做审查；`implement-spec` 的 worktree + 集成分支 + 收尾审查与 aicross 开发流程几乎同构；「护栏归 harness 不归提示词」与 aicross「规则由代码保证」同一思路。
- **不同**：
  1. 它解决「怎么把工程做对」，对模型是谁不关心；ai-cross 解决「哪个模型做哪步、谁来核查」，核心是跨厂商降低验证者的错误相关性。它的审查子 agent 与作者同模型，正是 ai-cross 要补的那一层。两者互补，不竞争。
  2. 它是二十多个小 skill 组合；ai-cross 是一个重 skill + 随附脚本 + 外部配置器 aicross，状态（清单、modes.json）在本机。
  3. 它没有基准与实测证据；ai-cross 的规则来自 bench / eval 实测。
- **值得借的**：
  1. user-invoked / model-invoked 分轴：aicross 生成的 `ax-*` 按模式命令本质是 user-invoked；kimi 下生成的是 SKILL.md，可能被模型自动触发，要核对能否标为仅用户调用。
  2. skills.sh 作为 ai-cross 的又一安装路线（`npx skills add keros68/xiaoyu-skill --skill=ai-cross`），与 aicross 安装同样要说明「二选一」。
  3. 文档页的「It's working if」节，给新用户一个可核对的成功判据。
  4. code-review 的「规范 / 需求」两轴可作为盲审提示词的分轴，和 ai-cross 的「换厂商」轴正交。
- **不借的**：grill / 术语表 / TDD 等工程纪律类 skill 本身（与 ai-cross 定位无关，用户可直接装它）。

Sources: github.com/mattpocock/skills（README、SCOPE.md、AGENTS.md、.agents/adr/0001–0002、.agents/invocation.md、.agents/install-block.md、.out-of-scope/、skills/engineering/code-review 与 implement-spec 的 SKILL.md，GitHub API 元数据，均 10-07 读，未装机实测）
