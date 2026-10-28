# aicross 用户手册

aicross 是多厂商 agent 的派发引擎与结果查看器。说话的地方是宿主（Claude Code、
Codex、Kimi 等装了 ai-cross skill 的对话窗）：模型提议派发、用户同意，aicross
拉起各家 CLI、隔离盲审、核对真身、留痕，并把各路结果并排交给用户裁决。窗口
有五页：开始、资源、skill、模式、派发记录；派发记录里点一条进查看页。装了
aicross 时，ai-cross skill（1.23.0 起）按窗口里设的默认模式与人选派发。

## 安装

Windows 用 `aicross_0.2.0_x64-setup.exe` 或 `aicross_0.2.0_x64_zh-CN.msi`，
两个装的是同一个程序，装一个就行。运行需要 WebView2，缺失时安装器会补装。
安装包把安装目录加入当前用户 PATH，卸载时只删除这一项。程序数据都在
`~/.aicross/`（资源库、预设、图、run 摘要、配置），卸载不删。设了环境变量
`AICROSS_HOME` 时数据目录改为它指向的位置，下文的 `~/.aicross/` 都按它读。
`runs/` 下超过 30 天未改动的运行记录在下次派发时删除。
macOS 与 Linux 目前只保证能编译，安装包未做验证。

## 命令行

`aicross --help` 列子命令一览，`aicross --version` 看版本；两者打印后即退出，不起窗口。不带参数运行 aicross 是打开窗口。

| 命令 | 用途 |
|---|---|
| `aicross dispatch --project <目录> --preset <模式> --text-file <文件> [--accept-file <文件>]` | 派发一次；多行任务文本走文件，不进 argv |
| `aicross status <run_id> [--wait] [--timeout 秒]` | 看一次 run 的状态；`--wait` 等到结束 |
| `aicross adopt <run_id> <node_id>` | 采纳某一路的结果 |
| `aicross cancel <run_id>` | 叫停一次派发 |
| `aicross recover <run_id> [--rollback]` | 崩溃后核对；`--rollback` 恢复单写者改动的项目文件 |
| `aicross open <run_id>` | 打开该 run 的查看页 |
| `aicross resources [detect \| smoke <agent> \| set-path <agent> [<路径>]]` | 看资源库 / 重新探测 / 冒烟验证 / 指定可执行文件路径 |
| `aicross config show \| set-role \| set` | 看与改角色、派发设置 |

`--preset` 认 `shared` / `economy` / `blind`（共享 / 经济 / 盲审），也可以直接
写预设 id；缺省经济模式。`--text-file` 要 UTF-8。`--accept-file` 可选，给
验收条件文件：内容随派发存进 run 文件，盲验收与盲审的隔离目录写成
`ACCEPT.md`，执行节点不收；超过 64 KB 截断。命令把 JSON 写到 stdout、
一行说明写到 stderr；退出码：0 成功，1 业务失败或等待超时，2 参数错误，
3 引擎不可用（skill 只在 3 时回退到直接调各家 CLI）。契约全文见
`docs/CONSOLE-CHANNEL.md`。

派发由后台引擎进程执行。Windows 上引擎退出或被杀时，它启动的各家 CLI 进程随之
结束；其它平台不保证。引擎意外退出后用 `aicross recover` 核对，见下文
「崩溃后核对」。

派发设置存放在 `~/.aicross/dispatch-settings.json`（超时、并发、修正轮上限、
验证命令、脱敏、派发记录位置等），用 `aicross config show` 看全量，`config set`
逐项改。角色（出方案 / 执行 / 验收）在 `~/.aicross/roles.json`，用
`config set-role` 改；三个角色一次定好，之后每次派发复用。

ACP 默认关闭，派发走各家 CLI 自带的续聊。要用 ACP 时执行
`aicross config set acpEnabled true`：写入设置后当场安装 claude、codex 的 ACP
适配器并输出每家结果。安装需要 npm 与对应的 CLI；装不上的，命令退出 1 并说明
原因，派发对它们回退一次性 CLI。`aicross config show` 的 `acp` 一项显示各家
适配器是否就绪。

## 崩溃后核对

后台引擎意外退出后，图上可能留下停在半路的节点，单写者节点可能把项目文件改了
一半。`aicross recover <run_id>` 只读核对：列出这些节点、各自的写权限与状态
（执行中断 / 执行已结束，结果未写回 / 未发出），单写者节点给出有变化的文件与
对应快照目录，worktree 节点给 worktree 路径（原样保留，没有产物时按创建规则
推算）。单写者执行期间，项目目录的 `.dispatch/` 下会有 `<node>.inflight/`
执行中快照，执行结束即删除；看到它残留说明引擎在执行中途退出。

`aicross recover <run_id> --rollback` 把单写者节点的项目改动恢复到对应快照：
执行中断的按执行中快照恢复，执行已结束的恢复到派发前快照，并把这些停在半路
的节点标为失败；没存内容的大文件无法自动恢复，报告与节点正文里列明；没有
快照的节点不回滚，报告里单列，需要自行核对。run 仍在运行时命令退出 1，等它
结束或先 `aicross cancel`；run 已是 `done` 时无需核对。

## 资源接入

接入是申报制：aicross 不扫描本机，资源清单在 `~/.aicross/resources.json`。

- **原生**（claude / codex / kimi 的自家模型，且没走第三方端点）：清单里有的
  就可用，不验证，冒烟也跳过。
- **第三方**（pi 的各 provider、cc-switch 端点、挂了 `ANTHROPIC_BASE_URL` 的
  claude）：记「未验证」。第一次派发前引擎发一个短请求、比对响应体里的模型
  真身，通过记「已验证」，不通过记「验证失败」并把这次派发按失败返回，不自动
  重试。也可以 `aicross resources smoke <agent>` 提前验证某一家。
  冒烟最长等 180 秒，超时记「冒烟超时」，不算验证失败，不拦派发，可重试。

claude 的 `fable` / `opus` / `sonnet` / `haiku` 是别名，真身记在
`resolved_id`。额度用尽的模型标「额度用尽」并写恢复时间；有备选时按备选顺序
改派，结果注明「主 X 额度用尽，已切备 Y」。

agent 装在 PATH 以外的目录时探测不到，资源页写明「没找到 <命令>」。在该 agent 的卡片或「未安装」行点「指定路径」，填可执行文件的完整路径，保存后重新探测；探测、测试、派发都先用这个路径。命令行：`aicross resources set-path <agent> [<路径>]`，不给路径即清除。以前探测到过、这次没找到的 agent 不折叠进「未安装」，留在主列表里显示原因。

## 模式

模式 = 给某个引擎换一套预设，一律叫「X模式」：

| 模式 | 适合 | 怎么分工 | 改文件 |
|---|---|---|---|
| 共享模式 | 问答、读代码、出主意 | 一个模型看完整对话后作答 | 只读 |
| 经济模式 | 要改代码的日常任务 | 高档模型出方案，低档模型改，另一家验收，验收旁并一路盲验证 | 改项目文件 |
| 盲审模式 | 核对方案、改动、结论等关键产出 | 三家各自审查，互相看不到 | 只读 |
| 擂台模式 | 想要几个版本挑一个 | 几家各做一份，选定一份采纳 | 只读 |
| 评审模式 | 分头查问题 | 几家分别从安全、性能、需求覆盖角度审查 | 只读 |
| 蜂群模式 | 能拆成互不重叠几块的大任务 | 拆成至多 4 件，各家在自己的 worktree 里做，再依次合并（命令行合并待另起一期） | 改项目文件 |
「适合……」与「只读 / 改项目文件」；自定义模式显示自己的介绍，改不改文件按各位置的写权限推出。

执行者带上下文是效率，验证者带上下文是污染：验收与盲审默认是盲的。验证角色
未指定目标时，除盲审模式外沿用验收角色的默认目标。修正轮上限默认 1；到上限
未闭合的问题生成「遗留」清单，不再派模型。

## 每次派发一张图

每次 `aicross dispatch` 新建一张图，派发之间不共享上下文。追问同一件事时，
把前情写进这次的任务文本（skill 会做）。run 摘要的遗留、查看页的标题与遗留
只属于本次派发。

## 开始

第一次打开窗口进这一页，三步：

1. **本机的 AI**：「探测」查出装了哪些 agent 与可用模型。
2. **配好**：选在哪个软件里用（主 agent），「一键配好」把 ai-cross skill 装到全局，
   并按推荐填好各模式空着的位置（手选的不动）。随后列出审与干活各用哪个模式、每个
   位置派给谁；还有位置没选到人时说明原因，并给「去验证」「手选人选」；「改人选」
   进模式页。
3. **怎么用**：回到主 agent 里输入的两条示例，可复制。

### 主 agent

Claude Code、Codex、Kimi Code、pi agent 之外，OpenCode、Gemini CLI、Antigravity、
Hermes Agent、ZCode、CodeBuddy、WorkBuddy、Qoder、Qoder CN 也可当主 agent：「探测」时
按配置目录或命令是否存在发现，记在资源库的 `extraHosts`。它们只是说话的地方，派发
仍由资源页里的 agent 承担；skill 只装全局，没有短命令，示例输入用「用 ai-cross …」
（Hermes、Qoder 用 `/ai-cross`）。除 Gemini CLI（Google）外，这些软件能换用不同厂商的
模型，厂商在模式页设定后推荐才能避开同厂商。

| 主 agent | skill 位置 |
|---|---|
| OpenCode | `~/.config/opencode/skills` |
| Gemini CLI | `~/.gemini/skills` |
| Antigravity | `~/.gemini/antigravity/skills` |
| Hermes Agent | `~/.hermes/skills` |
| ZCode | `~/.agents/skills` |
| CodeBuddy | `~/.codebuddy/skills` |
| WorkBuddy | `~/.workbuddy/skills` |
| Qoder / Qoder CN | `~/.qoder/skills` / `~/.qoder-cn/skills` |

### skill 升级

- aicross 自己装的 skill 随软件更新：启动时发现版本与内置不同，自动换成内置版本。
- 其他软件装的旧版（读不到 aicross 的设置）标「需要升级到 <版本>」，点「升级」确认后
  换成内置版本，原文件移到同一配置目录下的 `.aicross-skill-backup/`，之后由 aicross
  管理。命令行：`aicross skill upgrade --host <主 agent> --path <位置>`。
- skills-manager 管理的不在这里替换（它同步时会改回去），到 skills-manager 里更新。

## 软件更新

启动时查 GitHub 最新一版的 `latest.json`，有新版时顶栏出现「更新到 X」；确认后下载安装包、
校验签名并安装，装完自动重新打开，数据目录不动。查不到（离线等）时不提示。0.3.0 起有此
功能，更早的版本需手动安装一次新版。发布时用 `scripts/build-release.ps1` 打包并生成
`latest.json`，与安装包一起上传。

## 派发记录

最近 50 条派发，按更新时间倒序，每条给
模式名、任务名、状态和开始时间。点一条进入查看页；没有任何派发时提示从宿主
派发或运行 `aicross dispatch`。窗口可见时列表每 5 秒刷新一次，不可见时暂停。

## 查看页

派发结束后看结果的地方：一次 run 一页，只做「看 + 采纳」；重派与放弃仍在宿主
对话里说，由 skill 调命令行。

打开方式：`aicross open <run_id>`（run id 在派发响应与 `status` 的输出里，也
可以从派发记录点进去）。窗口没开时另起窗口进程并直接进入该页，命令随即返回；窗口
已开时切到该页。打开动作只读，不派发、不改文件。run 运行中时页面每 2 秒自动
刷新，结束后停止；首次读取不到 run 文件时页面显示「找不到这次派发」。Esc 或
「关闭」退出查看页；Esc 只关最上层。

页面上每一块：

- **页头**：模式名、run 状态、开始时间、总耗时；失败或拒绝的原因、遗留条目
  （若有）也在这里。
- **中部**按阶段分组，同一阶段各路并排（最多 3 列，超出换行）。每张卡的标题
  是「角色 · 任务」，副标题是 agent 与模型。卡上依次是：请求的模型与实际回答
  的模型（引擎判定不符时标「真身不符」，拿不到真身标「未报告」）、状态（额度
  用尽附恢复时间）、token 与耗时、备选切换与跳过原因；验收与验证那路显示结论
  （通过 / 需修正 / 未判定）；正文、产物 diff 与 worktree 提示在卡内展开看。
- **采纳**：每张有结果的卡上一个「采纳」；已采纳的卡只显示「已采纳」。
  采纳只改图上的标记；worktree 那路的改动要先按卡上给的路径手动合并。

## `.dispatch/` 与留痕

每次派发在项目目录下写一份派发记录：`<项目>/.dispatch/`，一次调用一个
Markdown 文件，头部 YAML 记着模型真身、用量、耗时、可见性；`STATE.md` 记未
跑完的那一轮；引擎仍会写入，当前没有命令读取。目录自带内容为 `*` 的
`.gitignore`，默认不进版本库。不想写进
项目，用 `aicross config set writeProjectDispatch false` 改写到
`~/.aicross/runs/`。

## 密钥与脱敏

密钥只在本机读、只按子进程环境变量传给 CLI、不回显、不落盘到 aicross 自己的
文件、不进模型上下文。发出去的任务全文默认过一遍脱敏：认 PEM 私钥块、
`Authorization: Bearer`、password / token / api_key 等赋值、AWS `AKIA…` 访问
密钥和三种数据库连接串；别的形态（裸写的 `sk-…`、`https://用户:令牌@主机`）
认不出来，会原样发出去。规则以 `src/dispatch.rs` 的 `redaction_patterns()`
为准。

## 术语表

| 概念 | 唯一用词 | 出现位置 |
|---|---|---|
| 内置预设 | 共享模式 / 经济模式 / 盲审模式 / 擂台模式 / 评审模式 / 蜂群模式 | `src/orchestration.rs` 内置预设名、`--preset` 别名、本手册 |
| 用户自建预设 | 自定义模式（多个时「自定义模式 · 名称」） | 查看页模式名 |
| 角色 | 出方案 / 执行 / 验收 / 验证 / 拆分 / 顾问 / 仲裁 | 角色配置、`src/orchestration.rs` |
| 内置 agent 显示名 | Claude Code / Codex / Kimi Code / pi agent | 资源输出、查看页副标题 |
| 可见性 | 共享 / 盲 | 图文件、查看页卡片 |
| 写权限 | 只读 / 单写者 / worktree | 图文件 |
| thinking 档 | 关 / 低 / 中 / 高 | 角色配置 |
| 把任务发给模型 | 派发 | 命令行、本手册 |
| 认可一路结果 | 采纳（状态：已采纳） | 查看页卡片 |
| 派发结果查看页 | 查看页（命令 `aicross open`） | 本手册、`docs/CONSOLE-CHANNEL.md` |
| 再派一次 | 重派 | 宿主对话 |
| 中途停止 | 叫停（命令 `aicross cancel`） | 宿主对话、本手册 |
| 崩溃后核对与恢复 | 核对 / 回滚（命令 `aicross recover [--rollback]`） | 本手册、`docs/CONSOLE-CHANNEL.md` |
| 把资源纳入清单 | 接入（动作成对：接入 ↔ 移除） | `aicross resources` |
| 用户给 agent 填的可执行文件位置 | 指定路径（动作成对：指定路径 ↔ 清除） | 资源页、`aicross resources set-path` |
| 核对模型真身 | 验证（状态：未验证 / 已验证 / 验证失败 / 冒烟超时 / 原生） | 资源输出、首次派发 |
| 执行后跑验证命令不通过 | 验证命令未通过（与真身「验证失败」分开） | 查看页状态 |
| 修正轮自动回退并恢复改动后核对测试 | 红绿核对（结果：测试已钉住 / 测试未钉住） | 查看页卡片 |
| 修正轮到顶后未闭合的条目 | 遗留 | 查看页页头 |
| 派盲审前附上的验证命令输出 | VALIDATION.txt | 盲目录、派发记录 |
| `--accept-file` 给出的验收条件 | ACCEPT.md | 盲目录、run 摘要 |
| 供验收核对的项目只读副本 | project/（多路上游时 project-2/ 起递增） | 盲目录 |
| 上游结果未进入验收的节点 | 已跳过 | 查看页卡片 |
| 模型限额已触发 | 额度用尽 | 资源输出、查看页卡片 |
| 模型额度重新可用的时间 | 恢复时间 | 资源输出、查看页卡片 |
| pi provider 登录状态 | 已登录 / 未登录 / 未检查 | 资源输出 |

不在界面上出现的词：留痕、对账、口径、判定、闸门、门禁、落库、wire、G1–G4、修正轮编号与任务编号。
