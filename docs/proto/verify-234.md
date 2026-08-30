# verify-234 只读调查记录

调查时间：2026-08-30。三项互相独立的只读调查，方法为 WebFetch/WebSearch + 本地文件核对。

---

## ② agmsg 的 license

**结论：MIT License。**

- 依据：`https://raw.githubusercontent.com/fujibee/agmsg/main/LICENSE` 首行 "MIT License / Copyright (c) 2026 fujibee / Permission is hereby granted, free of charge, to any person obtaining a copy"（WebFetch 原文引用）。仓库首页（`https://github.com/fujibee/agmsg`）License 标签同样显示 MIT。
- 不确定处：未直接打开仓库网页版的 LICENSE 渲染页核对文件名/路径是否恰为 `LICENSE`（用的是 raw.githubusercontent 直链，内容与仓库首页展示一致，可信度高，但未逐字节比对 SPDX 头）。

**消息存储（SQLite 路径）**

- 默认路径：`~/.agents/skills/<cmd>/db/messages.db`，可用环境变量 `AGMSG_STORAGE_PATH` 覆盖。
- 依据（README 原文引用）：`"The store is WAL-mode SQLite, so multiple readers and a single writer coexist without conflicts."`（来源：`https://raw.githubusercontent.com/fujibee/agmsg/main/README.md`）
- 不确定处：`<cmd>` 具体取值（是否等于宿主 CLI 名如 claude/codex）未在摘要中给出原文定义，需要另开 README 全文核对。

**投递机制（hook）**

- 依据（README 原文引用）：
  - Monitor 模式（Claude Code 为主）：`"SessionStart hook → Monitor tool → blocking SQLite stream"`，约 5 秒延迟。
  - Turn 模式（Codex/Copilot 默认）：`"Stop hook fires check-inbox.sh between assistant turns"`。
  - Both 模式：Monitor 为主、Turn 兜底。
- 不同宿主 agent 用不同 hook（Claude Code 用 Monitor 工具，Codex 靠 Stop hook 触发查收件箱）。

**寻址语义："只发给某个 agent，别的 agent 看不到"**

- **结论：没有严格的一对一私信语义；是"按名字广播"，不是按 agent 实例隔离。**
- 依据（README 原文引用）：`"If two agents are subscribed to the same name, both see the same inbound message."`（同一个名字下的多个 agent 都会看到同一条消息）
- 补充机制：`actas <name>` 提供"排他性绑定角色"——`"actas <name> is exclusive across sessions: it switches both sending and receiving to <name>, claims a lock that stops peer sessions from subscribing to the same name."` 这只是防止同一个名字被多个 session 同时占用（互斥锁），并不构成"某条消息只有指定 agent 能看到、团队里其他 agent 看不到"的按消息级隔离。
- send.sh 的调用形态是 `send.sh <team> <from> <to> "<message>"`，`<to>` 是按名字寻址，但一旦多个 agent 订阅同一个名字，消息对它们都可见——即隔离粒度是"名字"而非"唯一 agent 身份"。
- 不确定处：未找到 agmsg 是否提供"广播给全组 vs 私发给单个已订阅者"的显式区分命令；上述结论基于 README 摘要转述，未逐字通读全文档（如有 wiki/docs 目录未检索）。

---

## ③ AgentConnect 的可见性粒度

**结论：AgentConnect 的 visibility 机制是"人 → 资源/会话/agent"的整体访问控制，不提供"同一 thread 内某个 agent 看不到另一个 agent 的部分消息"这种按消息或按 agent 的细粒度隔离。可见性是会话级（整份 transcript 要么可读要么不可读），不是消息级过滤。**

- 依据（`https://docs.agentconnect.md/docs/permissions-overview` 原文引用，经 WebFetch 摘要转述）：
  - "Three independent axes then decide who sees a resource, who reads a session, and which agents may call one another." 三个轴分别是：
    1. Team visibility：`"A person → an agent, daemon, schedule, MCP provider, or skill source"`
    2. Session visibility：`"A person → one session and its transcript"`
    3. Agent visibility：`"An agent → another agent"`（这一条实际控制的是"哪些 agent 可以互相调用"，而不是"消息对谁可见"）
  - 会话可读性是二元的整体授权："Someone allowed to read a session does not gain the Agent's page, configuration, workspace, or invocation controls—the console may show the Agent's name as plain session context, never as a link."（这段说明的是"能读 transcript 的人不因此获得对该 Agent 本体的配置权限"，而非"能过滤 transcript 里的部分消息"）
- 仓库首页（`https://github.com/agentconnect-md/agentconnect`）功能描述：团队可以 `"Set clear boundaries"`，决定 `"who can see each agent and session, which repositories and tools it may use, and which other agents it may call"`——同样是 agent/session 级别的边界，未提及消息级可见性。
- **License：Apache License 2.0**（仓库首页 badge + LICENSE 文件引用）。
- 不确定处：
  - 未能直接抓到 `docs.agentconnect.md` 首页导航（该次 WebFetch 报错 "Socket is closed"），页面列表来自后续对 `/docs/permissions-overview` 的单独抓取和仓库首页转述，未逐页核对是否存在专门讲"消息级/thread 内分区可见性"的隐藏页面（如某篇 how-it-works 或 sessions 页可能有更细节的例外条款，未逐页通读）。
  - "Agent visibility：An agent → another agent" 这一轴的官方原文只摘要到"决定哪些 agent 可以互相调用"，没有查到它是否附带"调用时看不到调用方历史消息"这类隐式效果的原文说明——如果需要精确回答"能不能做到 agent A 看不到 agent B 在同一 thread 里发的某条消息"，现有材料只能回答到会话级，消息级问题原文未直接提及，按现有证据判断为**不支持**，但不是"文档明确否定"，而是"文档没有提供这个粒度的机制"。

---

## ④ 各 CLI 的用量能否折算成订阅额度百分比

### 本地代码现状

**`usage_probe.py`（`D:\work\agentteam\dist\ai-cross\references\usage_probe.py`）目前能拿到的字段：**

- Claude Code（第 94-100 行）：从 `~/.claude/projects/**/*.jsonl` 里筛 `"type":"assistant"` 记录，取 `message.model`（响应侧真实模型值）+ `timestamp`。**没有 token 数字段，也没有额度/费用字段**，只统计"某模型出现的次数 + 首末次时间戳"。
- Codex（第 102-108 行）：从 `~/.codex/sessions/**/*.jsonl` 筛 `"type":"turn_context"`，取 `payload.model`（请求侧值）+ `timestamp`。同样**无 token 数、无额度字段**。
- Kimi Code（第 110-116 行）：从 `~/.kimi-code/sessions/**/wire.jsonl` 筛 `"type":"llm.request"`，取 `model` + `modelAlias`（请求侧）+ `time`。同样**无 token 数、无额度字段**。
- 另有一段"已知 CLI 数据目录存在性 + 最近活动时间"的浅层扫描（第 118-137 行），只看目录 mtime，不解析内容。
- 输出仅为：`{source, model, alias?, count, first_seen, last_seen}` 的聚合列表 + installs 存在性列表（第 139-151 行）。**代码注释里明确写了隐私铁律：只输出模型ID/次数/时间戳/目录存在性，不读 token、不读费用、不读对话内容**（第 9 行）。

**结论：`usage_probe.py` 现在完全不产出 token 数或额度百分比，只能回答"某模型/某 CLI 最近是否在用、用了几次"，不能回答"用了多少配额"。**

**`channels.md` 里搜 "usage"/"额度"/"限流" 命中的段落**（`D:\work\agentteam\dist\ai-cross\references\channels.md`）：

- 第 35 行：额度耗尽时的**症状**是"挂住不返回、不报错"（GLM 实测 180s 超时），不是能读到的数值。
- 第 132 行：`--output-format json` 里检查 `is_error`/`api_error_status`，这是**错误检测**不是余量读取。
- 第 159 行、167 行附近的实测表：`--tools` 参数对**固定 input token 足迹**的省法（12k vs 31k），这是"派发时怎么省token"，不是"读剩余额度"。
- 第 179-180 行：千问 Token Plan 额度**不接入本 skill 派发**（风险不明确），未提供额度读取方式。
- 第 251 行："免费续期与漂移预警"用的是 usage_probe 的**新鲜度信号**（近 7 天日志里出现过 = 免冒烟续期），同样不是额度百分比。

**结论：channels.md 全文没有任何"读取额度上限/剩余量并折算百分比"的方案或字段，现状停留在"检测是否被限流（挂住/报错）"和"用日志新鲜度做续期判断"两类代理指标。**

### 三家是否有可编程读取的额度上限/余量

- **Claude（Claude Code）**：`/usage` 是**交互式斜杠命令**，展示 5 小时滚动窗口 + 每周上限的百分比；`claude --help` 顶层命令列表（本机 2026-08-30 实测）中**没有** `usage`/`status` 子命令或对应 CLI flag，只在交互会话内可用。未找到官方文档的编程接口（API）可直接查询 Claude Code 的用量百分比。
  - 依据：WebSearch 摘要引用 "the only reliable source of truth for your current numbers is the /usage command"；本机 `claude --help` 实测输出的顶层 Commands 列表（agents/attach/auth/auto-mode/doctor/gateway/import/install/logs/mcp/plugin/project/respawn/rm/setup-token/stop|kill/ultrareview/update）中无 usage 相关项。
  - 不确定处：搜索结果为第三方博客转述，未找到 Anthropic 官方文档原文逐字确认"无编程接口"；不排除企业版 Gateway（`claude gateway`，本机 help 有列出）里有用量遥测接口，本次未展开调查。

- **Codex**：交互式有 `/status`，展示 5 小时窗口与每周窗口的**剩余**百分比；`codex login status` 是登录状态检查，非本次核实到用量字段。**关键发现**：`codex exec --json`（app-server / exec 模式的程序化输出）里的 `TokenCount` 事件本应带 `rate_limits`（含 `primary.used_percent` / `secondary.used_percent` / `window_minutes` / `resets_in_seconds` 等），但据 GitHub Issue **openai/codex#14728**（已关闭）原文引用："codex exec mode always yields `rate_limits: null` in rollout JSONL and in `TokenCount` events, making it impossible to display real-time usage percentages in tooling"。即：**字段设计上存在，但 exec/非交互模式下实测恒为 null**，只有 VSCode 交互模式才拿得到（该 issue 提到的 `x-codex-*` 响应头据推测只在非 exec 会话下被处理）。
  - 依据：`https://github.com/openai/codex/issues/14728`（WebFetch 原文引用如上）；WebSearch 结果标题确认存在该 issue 及另一相关 issue "feat(exec): emit rate_limits in exec mode JSONL output"。
  - 不确定处：issue 状态为 closed，但摘要未明确是"已修复关闭"还是"wontfix 关闭"，需要再开 issue 页看 close 原因/是否有后续 PR 合并；本机未实跑 `codex exec --json` 验证当前版本是否仍为 null（任务要求不发模型请求，只读 `--help`，故未做此项实测）。

- **Kimi**：Kimi Code 平台的用量是**独立的请求配额**（不是 token 余量）："roughly 300 to 1,200 calls per rolling five-hour window depending on plan"，与 Kimi Open Platform（`platform.moonshot.cn`）的按量计费是两套**互相独立**的系统；官方有一个用量查询接口 `POST https://www.kimi.com/apiv2/kimi.gateway.billing.v1.BillingService/GetUsages`，但据检索到的社区反馈（MoonshotAI/kimi-cli issue #2150 标题："API usage display is confusing: two independent quota systems, inverted semantics, and poor discoverability"），这块**文档混乱、口径不透明**，未核实该 GetUsages 接口是否面向第三方开放调用（认证方式、是否需要网页 session cookie 而非 API key）。本机 `kimi --help` 顶层只有 `Usage: kimi [options] [command]` 提示，grep "usage/status" 未命中具体子命令名。
  - 依据：WebSearch 摘要转述自 geotoolbox.ai、GitHub issue 标题（MoonshotAI/kimi-cli #2150、#2278）；GetUsages 端点路径来自 WebSearch 摘要（转述自 `steipete/CodexBar` 项目文档 `docs/kimi.md`），**未直接 WebFetch 该文档原文核实**。
  - 不确定处：GetUsages 接口的调用方式（认证、返回字段是否含"百分比"或只有原始用量数）未核实，需要另外直接抓取 `github.com/steipete/CodexBar/blob/main/docs/kimi.md` 原文。

### 回答"要显示本周额度用了百分之几，还缺什么"

- 三家都**没有**一个稳定、文档化、可编程（非交互 TUI）直接返回"额度用了百分之几"的官方接口：
  - Claude：只有交互式 `/usage`，无 CLI/API 对应物（本次调查未找到反例）。
  - Codex：字段（`rate_limits.primary/secondary.used_percent`）在协议里存在，但 exec/非交互模式下实测输出恒为 null（#14728），只有交互式 `/status` 或 VSCode 扩展能看到真实值。
  - Kimi：有一个 `GetUsages` HTTP 接口，但认证方式、字段口径（两套独立配额系统）未核实清楚，社区反馈其可用性/口径存在混乱。
- 所以要在本项目里做"额度百分比"仪表盘，现状缺的是：① Codex 端等官方把 exec 模式的 `rate_limits` 字段修好（或找到不依赖 exec 模式的替代读取路径，如企业 Gateway/网页 session）；② Kimi 端需要先确认 `GetUsages` 的认证与字段含义，并厘清它到底对应哪一套配额（Coding Plan 的 5 小时请求数 vs Open Platform 的按量余额）；③ Claude 端目前完全没有找到官方非交互读取路径，只能退回 `usage_probe.py` 现有的"日志新鲜度/请求计数"做代理指标，这本质上不是"额度百分比"，只是"最近有没有在用、用了几次"。

---

## 备注

本文件所有网络类结论均基于 WebFetch/WebSearch 工具返回的**摘要转述**（工具本身会用小模型压缩原文再返回），未对每一句都做二次逐字核实原始 HTML；已在各节标出"依据"处尽量保留工具返回的原文引号片段。凡标"不确定处"的均为证据链上仍薄弱、建议后续用户自行打开原始链接复核的点。
