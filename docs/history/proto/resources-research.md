# 资源区调查：agent / 模型 二级组织方式

只读调查，2026-08-30。

## A. pi agent（本机 0.84.2，`~/.pi/agent/`）

**结论**：pi 本身就是「provider → 模型数组」两级结构（等价于 aicross 想要的「agent → 模型」二级），且提供只读命令 `pi --list-models` 可以自动探测——**可以自动探测**，不需要额外解析配置文件也能拿到清单；但若要展示 baseUrl/成本等元数据，仍需读 `models-store.json`。

依据：

1. **实际文件结构**（本机路径是 `~/.pi/agent/`，不是 `~/.pi/` 根目录；`~/.pi/agent/settings.json` 与 `~/.pi/agent/models-store.json` 是两个独立文件）：

   - `settings.json`（顶层扁平字段，非嵌套）：
     ```json
     {
       "lastChangelogVersion": "0.84.2",
       "defaultProvider": "qwen-token-plan-cn",
       "defaultModel": "qwen3.8-max",
       "defaultThinkingLevel": "xhigh",
       "theme": "dark"
     }
     ```
     只记「当前默认 provider + 默认 model」，不含 provider 清单本身。

   - `models-store.json`：以 **provider id 为顶层 key**，每个 provider 下是 `models` 数组，数组每项是一个模型对象；provider 级还带 `checkedAt`/`lastModified`/`etag`（远程模型目录的缓存元数据）。本机实测下唯一 provider `zai-coding-cn` 下有 5 个模型（glm-4.7 / glm-5-turbo / glm-5.2 / glm-5.2-highspeed / glm-5.3），每个模型对象字段：`id`、`name`、`api`（如 `"openai-completions"`）、`provider`、`baseUrl`、`reasoning`（bool）、`input`（如 `["text"]`）、`cost`（input/output/cacheRead/cacheWrite）、`compat`（一组能力标志，如 `supportsReasoningEffort`/`thinkingFormat`/`maxTokensField`）、`contextWindow`、`maxTokens`。**未见 apiKey/token 类字段**（凭据分开存在 `auth.json`，本次未读）。

   这证实 pi 的组织方式正是「provider（可映射为 aicross 的『第三方 coding plan 端点』）→ 该 provider 下多个模型」，与 aicross 想要的「agent 下挂多个模型」二级结构天然吻合——只是 pi 语境里第一级是 provider 不是 CLI 壳（pi 本身只是壳，provider 才是「资源」）。

2. **官方文档**（WebFetch 摘录，原文见 [custom-provider.md](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/custom-provider.md)、[models.md](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/models.md)）：
   - 加自定义端点用 `pi.registerProvider("my-llm", { baseUrl, apiKey, api: "openai-completions" | "anthropic-messages", models: [...] })`；Anthropic 兼容端点把 `api` 换成 `"anthropic-messages"` 即可。
   - 一个 provider 下**可以挂多个模型**：`models` 数组，"When `models` is provided, it **replaces** all existing models for that provider."
   - `~/.pi/agent/models.json`（用户可写的覆盖文件,注意与本机实际生成的缓存文件 `models-store.json` 是两个不同文件——前者是用户手改的配置输入,后者像是探测/缓存结果落盘,本次未深究二者读写关系）可以加 Ollama/vLLM/LM Studio/任意 OpenAI 或 Anthropic 兼容代理;模型对象最简只需要 `id`。
   - 有列出当前可用模型的命令：`pi --list-models [search]`（本机 `pi --help` 实测确认存在，模糊搜索可选）；文档还提到未配置鉴权的模型会"load 但在 `/model` 和 `--list-models` 里不可用"，即该命令只报**已鉴权可用**的模型,不是把 catalog 全量吐出来。
   - `pi auth <command>`（本机 `pi --help` 确认）可"Print credentials or check provider readiness"——这是另一个只读探测点，可能比 `--list-models` 更适合做"这个 provider 通不通"的检测,但两者都还没有实跑（遵照指示未发起模型请求）。

3. **对 aicross 的结论**：pi 作为一个 agent 资源接入时,**可以自动探测**其下模型清单,有两条路径——① 跑只读命令 `pi --list-models`（拿到的是当前已鉴权可用的模型,更贴近"能不能用"）;② 直接读 `~/.pi/agent/models-store.json`（拿到完整 provider→模型元数据,包括 baseUrl/cost/contextWindow,但可能包含未鉴权、不可用的条目）。两者互补,若要"资源区"展示丰富信息（成本、上下文窗口）宜读文件,若只要"能否派发"的判定宜跑命令。

## B. coffeecli 三栏结构 + 任务列表

**结论**：coffeecli 已经有一套「一级 CLI 工具 registry（`ToolDescriptor` + `TOOLS` 静态表）+ PATH 探测（`where`/`which`）+ 分档（T1/T2/T3）」的成熟实现,可以直接参考其分层与探测方式；它的「任务板」是纯本地 to-do/便签,不含模型概念,与 aicross 想要的"agent→模型"资源树是两回事,不能照搬数据模型,但探测层非常值得抄。

依据：

- **三栏职责**（各目录文件清单 + 顶部注释，`D:\work\coffeecli\src-ui\src\components\{left,center,right}`）：
  - `left/Explorer.tsx`：注释自述"Left panel: file tree synced from terminal CWD"——左栏是文件树,跟随当前终端的工作目录。
  - `center/`：`CenterPanel.tsx`（承载 `TierTerminal` 终端 + `SkillsPanel` + `FourSplitGrid` 分屏 + `ToolConfigModal` + `ContributionHeatmap`）是主工作区,`AGENT_CATALOG`（`VALID_PIN_KEYS` 常量里能看到全部支持的 CLI id：`claude, opencode, mimocode, openclaw, codex, grok, antigravity, qwen, hermes, pi, crush, aider, kimicode, goose, copilot` + `terminal`/分屏/`installer`）是启动台卡片来源。
  - `right/`：`ChangesBoard.tsx`（git 工作区改动,未提交/未跟踪/已提交三态）、`HistoryBoard.tsx`（历史会话列表）、`TaskBoard.tsx`+`TaskNoteView.tsx`+`TaskPromptView.tsx`（任务板,todo 列表 / 便签视图 / 提示词库视图三种展示模式共享同一份数据）、`DiffPanel.tsx`（选中文件的统一 diff,只读审计视图,作为 ChangesBoard 点击后的浮层）。

- **任务板字段与交互**（`D:\work\coffeecli\src\components\right\task-types.ts` + `TaskBoard.tsx`）：
  - `TaskItem { id, title, description?, status: 'todo'|'working'|'done', createdAt, height? }`——字段很薄,没有模型/agent 关联字段。
  - 说明文字：`description` 是纯文本;在"prompt 库"视图模式下,`title` 被复用为分类名、`description` 存提示词正文。
  - 可勾选：点击状态圆点按 `NEXT_STATUS`（todo→working→done→todo）循环推进,不是复选框语义。
  - 可拖动：`TaskBoard.tsx` 里有 `dropTargetRef`（`{id, status, position}`）,说明支持拖拽改状态/排序。
  - 持久化：既存 `localStorage`（旧键 `coffee-tasks`）又经 Tauri 后端 `commands.loadTasks/saveTasks`（新版本以后端为准,localStorage 仅做一次性迁移）,还监听 `tasks-changed` 事件做多窗口同步。

- **Agent 检测/安装器逻辑**（`D:\work\coffeecli\src\tools\mod.rs`、`src\tool_config.rs`、`src\server.rs`）：
  - 核心是一个静态注册表 `ToolDescriptor`（`src/tools/mod.rs`）：每个受支持 CLI 一个文件（`src/tools/{claude,codex,pi,...}.rs`）,字段含 `id`、`display_name`、`binary_name`（探测用的可执行文件名）、`skill_dir_relative`、`has_hook_surface`、`history_shape`（历史记录落盘的目录结构,定义了 7 种 shape 如 `GenericJsonl`/`CodexRollout`/`KimiIndex` 等）、`default_args`。所有工具汇总进 `pub static TOOLS: &[&ToolDescriptor]`,遍历它而不是各处硬编码列表。
  - **探测本机装了哪些 CLI** 就是最朴素的 PATH 查找（`src/server.rs` 第 90-111 行）：Windows 用 `where <bin>`、Unix 用 `which <bin>`,取子进程退出码判断有无,不解析版本、不发请求。
  - **分档**：注释里明确写了三档——T1（hook-wired,有实时状态)：claude/codex/opencode/mimocode/hermes/kimicode；T2（有历史扫描+热力图+resume,但没有 hook 状态点）：pi；T3（仅探测+启动,没有历史/hook)：crush/aider/goose/copilot。这是一个可以直接借鉴的"档位"划分方式,对应 aicross 想区分"能拿到丰富信息的资源"和"只知道装了但拿不到细节的资源"。
  - 用户可覆写：`~/.coffee-cli/tools.json`（`tool_config.rs`）允许每个工具自定义 `command`（含前缀参数,如 `docker exec mybox codex`）、`extra_args`、`default_cwd`、`history_path`,空字段回退内置默认——这是"用户手动补充探测不到的信息"的范式,可供 aicross 参考（本机没装的第三方端点让用户自己把 baseUrl/command 写一遍)。

## C. 竞品的 agent 资源接入方式

**结论**：三家都是"一级 = agent/CLI 家族,二级 = 该 agent 下的具体配置变体"的两层结构,但二级语义不完全等同"模型"——Vibe Kanban 二级是「variant/preset」（可以包含模型选择,但更偏"运行参数组合"）,FreeBuddy 二级是"该 agent 的模型下拉+参数",都不是像 pi 那样天然的 provider→模型数组。

依据（均为 WebFetch/WebSearch 摘录,未做二次核实,标注来源）：

- **FreeBuddy**（[github.com/maojindao55/freebuddy](https://github.com/maojindao55/freebuddy)）：README 称"automatically detects locally installed agents"——按二进制名在 PATH 上查（如 Codex 对应 `codex-acp`、ClaudeCode 对应 `claude-agent-acp`、OpenCode 对应 `opencode`）,与 coffeecli 的 `where`/`which` 探测思路一致。配置入口在 Settings → Coding Agents,可"Check installed runtimes"「Run recommended install commands」并"Customize binary paths, models, and extra arguments"——即模型是每个 agent 卡片下的一个可编辑字段/列表,而不是独立探测出来的。DeepSeek 等个别 agent 支持通过环境变量（如 `DEEPSEEK_API_KEY`)配置,workspace 内 `cordis.yml` 可覆盖打包自带的默认配置。

- **Vibe Kanban**（[BloopAI/vibe-kanban](https://github.com/BloopAI/vibe-kanban)，文档：[Agent Profiles & Configuration](https://vibekanban.com/docs/configuration-customisation/agent-configurations)）：确认是两级——顶层 `executors` 对象,key 是 agent 家族（`CLAUDE_CODE`/`GEMINI`/`CODEX`/`AMP` 等）,每个 agent 下是若干具名 **variant**（如 `DEFAULT`、`PLAN`）,每个 variant 装的是该 agent 的运行参数（示例摘录：`"DEFAULT": {"CLAUDE_CODE": {"dangerously_skip_permissions": true}}`,`"PLAN": {"CLAUDE_CODE": {"plan": true}}`）。变体名规则："Case-insensitive and normalised to SCREAMING_SNAKE_CASE"。运行时按"Identity & Preset + Runtime Overrides"两层解析。项目状态：官方仓库说明其"is sunsetting",但会转为社区维护的开源项目。

- **Conductor**：本次 WebSearch 未专门检索到（时间限制内优先查了 Vibe Kanban）,未查到,不写结论。

## 备注

- 全程未读取 `~/.pi/agent/auth.json`。`settings.json`/`models-store.json` 本身未见任何 apiKey/token 字段（凭据显然存在别处）,故本报告没有需要脱敏的字段。
- 未修改除本文件外的任何文件。
