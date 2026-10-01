# 控制台通道

本文供 ai-cross skill 的 `channels.md` 维护者使用。aicross 的执行面是命令行：
派发 `aicross dispatch/status/adopt/cancel`，资源 `aicross resources`，
配置 `aicross config`，查看 `aicross open`。命令直接使用无窗口引擎与本机文件，
不需要 aicross 窗口在跑；窗口只承载首页与查看页（TASK-40 已删除 HTTP 通道与
`propose` 子命令，G1 在宿主对话里完成）。

## 派发

多行派发文本必须写入文件，不放进 argv（Windows 在第一个换行截断）：

```powershell
aicross dispatch --project D:\work\project --preset economy --text-file D:\tmp\dispatch.txt
aicross status <run_id>
aicross status <run_id> --wait --timeout 600
aicross adopt <run_id> <node_id>
aicross cancel <run_id>
aicross open <run_id>
```

`--preset` 缺省为 `economy`；别名 `shared` / `economy` / `blind` 分别对应
共享 / 经济 / 盲审模式，也可以直接写预设 id。`--text-file` 接受 UTF-8，
可带 UTF-8 BOM；其它编码返回参数错误。不接受 `--text` 或 `--text=...`。

`--accept-file` 可选，给验收条件文件：UTF-8、可带 BOM，超过 64 KB 截断并在
stderr 说明。内容随派发存进 run 文件（`accept_criteria`，未给时不写出），
盲验收与盲审的隔离目录写成 `ACCEPT.md`，执行节点不收；未给时不生成这个
文件。盲验收的隔离目录一律附项目只读副本 `project/`（上游执行在自己
worktree 时取该 worktree；多路上游各附一份，`project-2/` 起递增），
`REQUEST.md` 头部写项目与执行者 worktree 的提交号（非 git 项目不写）。

首个响应带真实的 `graph_id` 与本次新建的 `node_ids`：

```json
{ "run_id": "…", "accepted": true, "graph_id": "…", "node_ids": ["…"] }
```

## 状态、采纳与取消

`status` 输出 run 摘要（与 `~/.aicross/runs/<run_id>.json` 同一口径，含死进程
判定）。`--wait` 等到 run 不再是 `running`，`--timeout` 单位为秒，默认 600，
到时以退出码 1 结束。`adopt <run_id> <node_id>` 只写图上的采纳标记；worktree
那路的改动要按查看页提示手动合并。`cancel <run_id>` 写取消标记，不改写 run
文件；run 已不在 `running` 时取消静默成功（退出 0，打印当前 run）。

run 摘要记录本次派发的原始需求文本（`original_request`）、验收条件全文
（`accept_criteria`，未给 `--accept-file` 时不写出）、所用预设 id
（`preset_id`，按 targets 直给时为空）、后台进程 pid 与进程创建时间
（`pid_started_at`）。后台引擎逐阶段更新；取消标记存放在同目录的
`<run_id>.cancel`，处理完成后删除。

后台引擎启动的各家 CLI 子进程（含它们再拉起的进程）在 Windows 上挂进
kill-on-close Job：引擎进程退出或被杀时随之结束，不留继续消耗额度的孤儿进程。
其它平台不做这项保证。

存活判定：run 为 `running` 且 pid 非 0 时，Windows 核对进程存在且创建时间与
`pid_started_at` 一致（防止 pid 复用误判）；macOS 与 Linux 只核对进程存在。
派发刚接收、后台进程尚未报到时 pid 为 0：`updated_at` 在 30 秒内视为启动中；
超过 30 秒仍为 0，或 `updated_at` 比当前时间晚 60 秒以上（时钟回拨），判
`failed`，原因是「引擎进程未能启动」。进程中途被杀死时 run 判 `failed`、
原因是「引擎进程已退出」。

项目路径按命令行给的原文使用（不做规范化改写）；项目目录不存在时派发前拒绝。

## 每次 dispatch 一张图

每次 `aicross dispatch` 新建一张图，派发之间不共享上下文：后一次派发的执行者
读不到前一次的材料与回复，run 摘要的遗留项只来自本次的图，图标题取本次的任务。
追问同一件事时，由调用方把前情写进任务文本。

## 资源与配置

```powershell
aicross resources
aicross resources detect
aicross resources smoke pi --model glm-4.7 --provider zai-coding-cn
aicross config show
aicross config set-role execution --agent pi --model glm-4.7 --provider zai-coding-cn
aicross config set timeoutSecs 900
```

`resources` 只读资源库（`~/.aicross/resources.json`），不探测、不发模型请求。
`resources detect` 重新探测全部 agent（含 pi provider 登录检查）并写回资源库；
cc-switch 发现提示的路径取派发设置里的 `ccSwitchPath`。单个 agent 探不到或事实
源读取失败时逐行写 stderr，退出码仍为 0。`resources smoke <agent>` 对该 agent
的第三方模型各发一次短请求并核对真身；`--model` 只冒烟这一个，`--provider`
在同名模型挂多个 provider 时指定端点；原生模型不冒烟，退出 0。冒烟失败退出 1，
stderr 给脱敏后的原因。

单次冒烟最长等 180 秒。超时记「冒烟超时」（资源库 `verification` 为 `timeout`），
不算验证失败，派发前检查不拦，可再跑一次 `resources smoke` 重试；真身核对不通过
才记「验证失败」并拦下派发。

`config show` 输出 `{ "roles": …, "dispatch": …, "acp": … }`，`acp` 列出每家
ACP 适配器当前是否就绪，只查不装。`config set-role` 把角色指
到资源库里的模型，目标不在资源库时退出 2 并列出可选模型；`roles.json` 损坏时
退出 1，不写盘。`config set` 只认 `dispatch-settings.json` 的键
（`timeoutSecs`、`concurrency`、`maxRequests`、`fixRoundLimit`、`swarmLimit`、
`consoleRunLimit`、`validationCommand`、`redact`、`writeProjectDispatch`、
`acpEnabled`、`ccSwitchPath`、`classificationModel`、`classifyProposals`）；
越界夹取与小数取整在 stderr 说明，未知键退出 2；文件损坏时退出 1，不覆盖。

ACP 默认关闭，派发走各家 CLI 自带的续聊。`config set acpEnabled true` 写入设置
后当场检查并安装 claude、codex 的 ACP 适配器（npm 装进 `~/.aicross/acp`，版本
固定），装完才返回；安装需要 npm 与对应的 CLI，没装对应 CLI 的那家跳过。stdout
输出 `{ "dispatch": …, "acp": [{ "agent": "claude", "state": "已就绪" }, …] }`，
`state` 为 已就绪 / 刚装好 / 未装对应 CLI / 没有 npm / 安装失败，后两种带
`reason`（失败时为 npm 输出的最后几行）。有一家没装上时设置照样保留，退出 1，
stderr 写明是哪几家；派发对它们回退一次性 CLI。派发途中不安装。设为 `false`
时不安装。

## 查看

`aicross open <run_id>` 打开该 run 的查看页：窗口没开时另起窗口进程并直接进入
该页，命令随即返回，不等窗口关闭；窗口已开时切到该页。打开动作只读，不派发、
不改文件。首页「最近的派发」列出最近 50 条 run，点一条进查看页。

## 退出码

| 退出码 | 含义 |
| --- | --- |
| 0 | 请求成功 |
| 1 | 业务失败、等待超时（`status --wait` 到时）或找不到 run（`open`） |
| 2 | 命令参数、文本文件或编码错误 |
| 3 | 无法确定引擎目录、run 目录不可写、后台进程无法启动、资源库不可读写，或查看窗口起不来 |

skill 仅在退出码 3 时回退到直接调各家 CLI。

## 留痕与摘要

派发记录写在 `<项目>/.dispatch/`（一次调用一个 Markdown 文件，`STATE.md` 记
未跑完的那一轮；引擎仍会写入，当前没有命令读取）；run 摘要在
`~/.aicross/runs/<run_id>.json`；图在
`~/.aicross/graphs/`；派发设置在 `~/.aicross/dispatch-settings.json`。命令把
JSON 写到 stdout，把一行说明写到 stderr。
