# 控制台通道

本文供 ai-cross skill 的 `channels.md` 维护者使用。aicross 的执行面是命令行：
派发 `aicross dispatch/status/adopt/cancel`，核对 `aicross recover`，
资源 `aicross resources`，配置 `aicross config`，主 agent `aicross hosts`，
skill 安装 `aicross skill`，自定义模式 `aicross preset`，自定义短命令
`aicross slash`，查看 `aicross open`。命令直接使用无窗口引擎与本机文件，
不需要 aicross 窗口在跑；窗口承载资源、模式与最近的派发三页。

## 主 agent 与 --host

主 agent 是用户直接对话、装着 ai-cross skill 的那个 agent。可作主 agent 的只有
claude、codex、kimi、pi 四家，且只列资源库里 `detected` 为 true 的：

```powershell
aicross hosts
```

输出 `[{ "id", "name", "vendor", "vendorSource", "vendorBasis" }]`。厂商按各
家配置推定（不读密钥）：claude 看 `~/.claude/settings.json` 的
`env.ANTHROPIC_BASE_URL`（先解析出 URL 主机名再比对，进程环境变量同口径），
codex 看 `~/.codex/config.toml` 的 `model_provider`，kimi 看
`~/.kimi-code/config.toml` 的 `provider`（`managed:kimi-code` 开头 → Moonshot），
pi 拿默认 provider + 默认模型到资源库里查厂商，查不到或该模型未发现则为
「未知」，`vendorBasis` 写明依据。`vendorSource` 取 推定 / 端点 / 用户设定 /
未知。

厂商统一用资源库的英文名：Anthropic、OpenAI、Google、Moonshot、Zhipu、
DeepSeek、Alibaba、MiniMax、StepFun、OpenRouter。常见端点：`anthropic.com` →
Anthropic，`bigmodel.cn`、`z.ai` → Zhipu，`moonshot`、`kimi.com` → Moonshot，
`dashscope`、`aliyun` → Alibaba。比较不区分大小写与写法（中文旧写法
智谱 / 月之暗面 / 阿里也认）。

用户可以覆盖推定：`aicross config set-host-vendor <host> <vendor|auto>`
写入 modes.json 的 `vendorOverride`，只认上面这些英文名或 `auto`，其它值
退出 2；`auto` 清除覆盖。输出该主 agent 的条目（同 `aicross hosts` 项）。
非法 host 退出 2。

所有读写配置的命令与派发都认 `--host <agent>`：

- 给了就用，值必须是检测到的 agent，否则退出 2「未知 host：…；检测到：…」。
- 没给时，modes.json 里只有一个主 agent 的配置就用它，但它在资源库可读时
  仍要校验是否仍被检测到（已停用的不再自动用，退出 2「modes.json 里唯一
  的配置 … 未被检测到，请用 --host 指定，可选：…」）；资源库读不了时
  不为校验去读它，照常推定、不退出 3；一个配置都没有则按无配置处理
  （派发走原有 roles.json 行为，展示按空配置）；多于一个则退出 2
  「需要 --host，可选：…」。

窗口命令 `config_snapshot` 同一套规则；host 为 null 且一份配置都没有时
返回资源与空配置，不报错（资源页在首次探测前就要能读到快照）；多于一份
配置且未给 host 时与命令行一致报错。

写命令（set-default-mode / set-count / set-focus / set-seat / clear-seat /
set-host-vendor）在没有任何配置时必须有 --host。run 文件追加 `host` 字段
记录本次派发的主 agent（无配置时为空、不写出）。

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

`--preset` 缺省时用 `modes.json` 里的默认模式，未设默认模式时为经济模式；
别名 `shared` / `economy` / `blind` 分别对应
共享 / 经济 / 盲审模式，也可以直接写预设 id。`--text-file` 接受 UTF-8，
可带 UTF-8 BOM；其它编码返回参数错误。不接受 `--text` 或 `--text=...`。

`--seat <阶段>.<位置>=<agent>:<provider>:<model>` 可重复，只对这一次派发换人，不写
`modes.json`；provider 没有就空着（`1.1=claude::opus`），写法与 `resources find` 输出的
`target` 一致。模式取 `--preset`，没给时取该主 agent 的默认模式；需要 `--host`。校验与
`config set-seat` 相同（模型在资源库里、阶段与位置在范围内、可见性允许该 agent，模式里
写定了模型的位置不能换）；不通过退出 2，不派发。通过后 stderr 每个位置写一行
「这次临时换人（不改配置）：…」。

```powershell
aicross dispatch --host claude --project D:\work\project --preset blind --seat 1.1=pi:opencode-go:claude-opus-5-5 --text-file D:\tmp\dispatch.txt
```

`--accept-file` 可选，给验收条件文件：UTF-8、可带 BOM，超过 64 KB 截断并在
stderr 说明。内容随派发存进 run 文件（`accept_criteria`，未给时不写出），
盲验收与盲审的隔离目录写成 `ACCEPT.md`，执行节点不收；未给时不生成这个
文件。盲验收的隔离目录一律附项目只读副本 `project/`（上游执行在自己
worktree 时取该 worktree；多路上游各附一份，`project-2/` 起递增），
`REQUEST.md` 头部写项目与执行者 worktree 的提交号（非 git 项目不写）。

`--kind` 可选，给任务类型：`frontend` / `backend` / `writing` / `data` / `docs` /
`other`，中文名（前端 / 后端 / 文案 / 数据分析 / 文档 / 其他）也认；别的值或缺值
退出 2。解析成英文 id 后存进 run 文件（`task_kind`，未给时不写出），供使用记录
按类型汇总，派发流程不因它改变。

每个派出的节点，最终外发文本末尾固定追加一句如实汇报要求：`Be honest in your
response. 没做完、没运行、失败或与预期不符的结果要写明，放在回答开头。`
所有角色与模式都一样，没有开关；派发原文在提示词里原样保留。派发层铺材料前
先摘掉引擎拼在提示词末尾的那一段，定下最终外发文本（续接会话时发的当前消息、
盲审材料说明与 diff、agy 的内联材料之后）再在最末尾追加一次，文本已以它结尾
时不再追加；ACP 与一次性 CLI 两条通道都一样。用户原文、diff、内联源码里引用
的同样文字不改动。Kimi 一次性通道超过 30000 字节时保留首尾裁中间，收尾句留在
末尾。派给
agy 的节点在这句之后再追加 `不要使用任何工具，直接用文字回答。`（agy 无头
模式下工具全被拒）。

审查位的模板看有没有上游产物（直接父节点里有模型节点）：有时验收位用验收
模板，审查位附两条固定关注点（契约变没变、回退修复后测试红不红）；没有时
（盲审、评审模式单独派发，第一阶段就是审查位）验收位改用「独立审查这份
原始材料」的模板（`fixtures/prompts/independent-review.txt`），固定关注点
不附；评审模式的视角句照常。

agy、cursor、codebuddy、copilot、qoder 只接盲位置：落在共享或单写者位置时
节点派发前直接失败（不做首次验证、不建 worktree），错误写明原因。agy 在盲
目录里跑 `agy --model <m> --input-format stream-json --output-format stream-json
--print-timeout <超时>s [--effort low|medium|high]`（模型 id 自带 `-low` /
`-medium` / `-high` 等档位后缀时不传 `--effort`，两者冲突 agy 直接报错），
材料是 stdin 的一行 NDJSON；
最后一行 `{"event":"result"}` 的 `status` 不是 SUCCESS 或 `response` 为空即失败。
agy 读不了文件：盲目录里的材料（REQUEST.md、diff.patch、changed/、ANSWER.md、
ACCEPT.md、VALIDATION.txt、附件）全部以文本内联进提示词（提示词里已有的不重复），
提示词与材料合计上限 200 KB；超上限、有二进制材料、或节点要读项目只读副本
（如有上游产物的盲验收）时派发前失败，不发出。模型 id 自带档位时思考强度由
模型决定，留痕的 `thinking`、run 摘要节点的 `thinking`（仅此时写出）与
`config show` 的 `effective.thinking` 都记「由模型决定：low」这类实际生效值。
cursor、codebuddy、copilot、qoder 是未实测通道，派发直接失败并提示「未实测通道」。

首个响应带真实的 `graph_id` 与本次新建的 `node_ids`：

```json
{ "run_id": "…", "accepted": true, "graph_id": "…", "node_ids": ["…"], "notes": [] }
```

派发前检查拦下不能再派的目标：目标模型在最近一次探测中未发现或验证失败时
拒绝派发，原因写在响应的 `reason` 里，前者为「<agent>/<model> 在最近一次
探测中未发现，请重新探测或换人选」。「未发现」按资源库 `meta` 的「未在事实
源中」标记判断，新建的各阶段都查（验证失败同样各阶段都查）；同名模型挂在
多个 provider 下时，按人选的 provider 认那一条，人选不带 provider 时优先取
没有未发现、没有验证失败的那条。执行时派发层每次尝试（任一阶段的主目标、
显式备选、资源库 `fallback_order` 里的有序备选）再按同一口径查一次：主目标
未发现或验证失败时不进通道、节点失败并写明原因；备选未发现或验证失败时
跳过它，跳过原因写进 runner 说明；冒烟超时（`timeout`）照常放行。备选未发现时把它从该节点移除（写进本次 run 的图，
同 run 重试不再带它），主目标照常派出，说明一行一条随响应的 `notes` 给出
（响应总有该字段，空时为 `[]`）；run 文件的 `notes` 同样记录，为空时不
写出。

## 状态、采纳与取消

`status` 输出 run 摘要（与 `~/.aicross/runs/<run_id>.json` 同一口径，含死进程
判定）。`--wait` 等到 run 不再是 `running`，`--timeout` 单位为秒，默认 600，
到时以退出码 1 结束。`adopt <run_id> <node_id>` 只写图上的采纳标记；worktree
那路的改动要按查看页提示手动合并。`cancel <run_id>` 写取消标记，不改写 run
文件；run 已不在 `running` 时取消静默成功（退出 0，打印当前 run）。

run 摘要记录本次派发的原始需求文本（`original_request`）、验收条件全文
（`accept_criteria`，未给 `--accept-file` 时不写出）、所用预设 id
（`preset_id`，按 targets 直给时为空）、主 agent（`host`，无配置时不写出）、
任务类型（`task_kind`，未给 `--kind` 时不写出）、从节点移除未发现备选的说明（`notes`，
无则不写出）、后台进程 pid 与进程创建时间（`pid_started_at`）。后台引擎逐阶段更新；取消标记存放在同目录的
`<run_id>.cancel`，处理完成后删除。

后台引擎启动的各家 CLI 子进程（含它们再拉起的进程）在 Windows 上挂进
kill-on-close Job：引擎进程退出或被杀时随之结束，不留继续消耗额度的孤儿进程。
其它平台不做这项保证。

存活判定：run 为 `running` 且 pid 非 0 时，Windows 核对进程存在且创建时间与
`pid_started_at` 一致（防止 pid 复用误判）；macOS 与 Linux 只核对进程存在。
派发刚接收、后台进程尚未报到时 pid 为 0：`updated_at` 在 30 秒内视为启动中；
超过 30 秒仍为 0，或 `updated_at` 比当前时间晚 60 秒以上（时钟回拨），判
`failed`，原因是「引擎进程未能启动」。进程中途被杀死时 run 判 `failed`、
原因是「引擎进程已退出」；之后的核对与恢复见下一节 `aicross recover`。

## 崩溃后核对

后台引擎意外退出后，图上可能留下停在半路的节点（本 run 的节点里图上
`pending` 的，以及 run 判失败时仍为 `draft` 的），单写者节点可能把项目文件
改了一半。`aicross recover <run_id>` 做只读核对，stdout 输出 JSON：run 状态
与原因；每个停在半路的节点给 `node_id`、角色、agent、模型、写权限（只读 /
单写者 / worktree）。图上不属于这个 run 的节点不进报告。

单写者节点按磁盘上的快照区分状态。有执行中快照（`inflight_dir`）→「执行
中断」：`changed_files`（`added` / `modified` / `removed`）拿项目现状对比
清单，超过单文件上限、只记路径没存内容的文件列在 `unrestorable`。没有执行
中快照但有派发前快照（`before_dir`）→「执行已结束，结果未写回」：
`changed_files` 相对快照，新增只认快照里 `.created` 的记录，项目里多出来的
其它文件不算新增。两者都没有时：`draft` 节点「未发出」，不查文件；
`pending` 节点在 `no_snapshot` 里单列：项目改动回滚不了，需自行核对。
worktree 节点给 `worktree` 路径，原样保留；图上没有产物时按创建时的固定规则
推算路径，推算出的路径不存在时 `worktree_missing` 为 true（worktree 不存在）。
只读节点不查文件，状态按是否已发出记「执行中断」或「未发出」。

只读核对不改项目文件、不改图；读取 run 时若判定引擎进程已退出，会把 run
状态更新为失败、原因「引擎进程已退出」（与 `aicross status` 相同）。

`aicross recover <run_id> --rollback` 在核对之后恢复项目文件：「执行中断」的
节点按执行中快照恢复存了内容的文件、删掉新增，没存内容且有变化的文件不动，
在输出与节点正文里列明（不算失败）；「执行已结束，结果未写回」的节点恢复到
派发前快照、删掉新建文件；逐节点结果在输出 JSON 的 `nodes[].rollback`。全部
停在半路的节点在图上标 `failed`，只改状态、正文与失败类型，原 reply 的其余
字段（产物、worktree 路径等）保留：已回滚的单写者正文「引擎中断，已回滚项目
改动」（有无法恢复的文件时在正文里列明），只读与 worktree 正文「引擎中断」，
未发出的正文「引擎中断」，无快照的正文「引擎中断，无派发前快照，项目改动未
回滚，请自行核对」；经合并写落图。run 摘要同步刷新，状态保持 `failed`，原因
追加「已核对」，没有停在半路的节点也刷新。某个节点回滚失败时继续处理其余
节点，该节点正文「引擎中断，回滚失败：<原因>，请自行核对」，最后退出 1 并在
`failures` 里列出失败项。

退出码：0 成功；1 run 仍在运行（提示等它结束或先 `aicross cancel`）或回滚有
失败；run 已是 `done` 时退出 0，说明「无需核对」。快照只存普通文件：排除目录
（.git/node_modules/target/dist…）与链接里的改动不在快照内；超过单文件上限
的文件只记路径不存内容，这些位置的变化无法自动恢复，会列在 `unrestorable`。

单写者执行期间，项目目录的 `.dispatch/` 下会有 `<node>.inflight/`
（执行中快照），执行结束即删除；看到它残留说明引擎在执行中途退出。

项目路径按命令行给的原文使用（不做规范化改写）；项目目录不存在时派发前拒绝。

## 每次 dispatch 一张图

每次 `aicross dispatch` 新建一张图，派发之间不共享上下文：后一次派发的执行者
读不到前一次的材料与回复，run 摘要的遗留项只来自本次的图，图标题取本次的任务。
追问同一件事时，由调用方把前情写进任务文本。

## 使用记录（stats）与用途标注（resources note）

后台引擎跑完一次派发（成功或失败）时，按最终的图给每条执行链往
`$AICROSS_HOME/track/outcomes.jsonl` 追加一行 `{"type":"outcome", run_id, node_id,
host, preset_id, task_kind, agent, provider, model, role, result, rounds, error_kind}`。
执行链是第一轮执行节点加它后面的修正轮（验收要求修正后长出的执行，以及同父、
同人选、任务文本以「[修正」开头或含「请修正后重新运行」的验证重试）。`result`：

| 值 | 条件 |
|---|---|
| `first_pass` | 链只有一轮，最后一个验收结论 PASS |
| `fixed_pass` | 修正轮之后验收结论 PASS |
| `not_passed` | 验收结论 NEEDS_FIX（修正轮到顶），或最后一轮 `error_kind` 为 `validation_failed` |
| `failed` | 最后一轮 `failed`（非验证失败）或 `quota_exhausted` |
| `no_acceptance` | 最后一轮完成，没有验收节点 |
| `unjudged` | 有验收，但验收没完成或结论读不出 |

叫停（`cancelled`）、从没跑过（draft / pending）、链上任一轮切了备选的不写。
`adopt` 成功时追加 `{"type":"adopt", …, role}`。写失败只在 stderr 说明，不影响
派发与采纳。读的时候同一（type、run_id、node_id）只取最后一行。

`aicross stats [--kind <类型>]` 输出 `{ minJudged, kindList, models, kinds }`：
`models[]` 每项 `model`（`agent:provider:model`）、`usable`（按资源库状态，资源库
读不了时为 null）、`tags` / `tagNames` / `note`、`total` 与 `byKind[]`（`runs`、
各结果数、`adopted`、`judged` = 一次通过 + 修正后通过 + 未通过、`passRate` /
`firstPassRate`：`judged` ≥ 5 时为 0–100 的整数，否则 null）；`kinds[]` 每项
`ranked`（`passRate` 不为 null 的模型按通过率、样本数降序）与 `tagged`。退出码：
0 成功，1 标注文件读不了，2 参数错。

`aicross resources note <agent>:<provider>:<model> [--tags <类型,…>] [--note <文本>]`
读写 `$AICROSS_HOME/model-notes.json`（`{ version: 1, models: { <模型>: { note, tags } } }`，
标签存英文 id、按类型表顺序、去重；两项都空时删掉该条）。不给选项只读；
`--tags ""` / `--note ""` 清空。输出 `{ model, tags, tagNames, note }`；模型写法
不对、标签不在类型表里、未知选项退出 2，文件读写失败退出 1。

## 资源与配置

```powershell
aicross resources
aicross resources detect
aicross resources smoke pi --model glm-4.7 --provider zai-coding-cn
aicross resources prune [--agent <a>]
aicross resources find claude 5.5
aicross resources set-path pi D:\tools\pi\pi.cmd
aicross resources set-path pi
aicross config show --host claude
aicross config set-role execution --agent pi --model glm-4.7 --provider zai-coding-cn
aicross config set timeoutSecs 900
aicross config set-default-mode blind --host claude
aicross config set-count blind 3 --host claude
aicross config set-focus --host claude --mode builtin-jury --index 2 --focus 规范
aicross config set-seat blind 1 1 --agent pi --model glm-4.7 --provider zai-coding-cn --host claude
aicross config clear-seat blind 1 1 --host claude
aicross config clear-seat blind --host claude
aicross config set-host-vendor claude DeepSeek
aicross config recommend --host claude
aicross config recommend --host claude --mode blind --all
```

`resources` 只读资源库（`~/.aicross/resources.json`），不探测、不发模型请求，
顶层 `detected_at` 是最近一次成功写回的探测时间（RFC3339；从未探测过为
null）。从未探测过时（`detected_at` 为 null）每个 agent 与其下模型都显示
「未探测」，agent 的 note 为「尚未探测」；探测过后只有 可用 / 未安装 两态。
agent 按支持清单列出（claude、codex、kimi、pi、agy、cursor、codebuddy、
copilot、qoder），没装的也列（`state` 未安装、`models` 为空）。每个 agent
另带 `untested`（布尔：命令模板只照官方文档写、没在真机跑通，一键推荐不选）
与 `homepage`（官网链接）、`custom_path`（用户指定的可执行文件路径，没指定为空串）。
未安装的 agent 的 note 写明找的是哪个命令（「没找到 pi；安装后重新探测，或指定路径」），
指定的路径不存在时为「指定的路径不存在：<路径>」。每个模型带 `vendor`（厂商，如
Anthropic、OpenAI，认不出为「其他」），开始页据此写出副 agent 含哪几家的模型。
`resources detect` 重新探测全部 agent（含 pi provider 登录检查）并写
回资源库，同时更新 `detected_at`；各 agent 的探测在资源库锁外并行跑，
全部完成后一次合并写盘。窗口命令 `resources_detect` 同一实现体，每个
agent 探测完发一次 `resources-detect-progress` 事件
（`{ "agent": …, "done": …, "total": … }`）。cc-switch 发现提示的路径取派发设置里的
`ccSwitchPath`。单个 agent 探不到或事实源读取失败时逐行写 stderr，退出码仍
为 0；claude、codex、kimi、pi 以外的 agent 没装时只记未安装，不写 stderr。
模型清单：agy 读 `agy models`（30 秒超时），cursor 读 `cursor-agent models`，
copilot、codebuddy 读随版本发布的内置小表，qoder 固定五档
（lite / efficient / auto / performance / ultimate）。`cursor-agent models` 解析失败时
写一条错误，cursor 已有模型标「未在事实源中」、不再可选。这几家的模型厂商按模型名推，
认不出的记「未知」，只能当执行位（出方案、拆分、顾问与审查位都不进推荐）。`resources smoke <agent>` 对该 agent 的第三方模型各发一次短请求并核对
真身；`--model` 只冒烟这一个，`--provider` 在同名模型挂多个 provider 时指定
端点；原生模型不冒烟，退出 0。agy 例外：`resources smoke agy --model <m>`
真发一次最小派发，回答非空即通过，失败退出 1（验证档仍为原生）；它在本次调用
专属的空目录 `$AICROSS_HOME/scratch/smoke-agy-<uuid>` 里跑，结束（含失败、超时）即删除。模型状态为「未发现」（资源库 `meta` 带
「未在事实源中」标记，即最近一次探测没有发现它；原生模型同样适用）时不发
请求：单个模型冒烟退出 1 并说明；按 agent 冒烟跳过它，逐行写 stderr，不算
失败。可冒烟的模型全被跳过时，输出仍保留该 agent 的条目与跳过说明。冒烟、
选人选与派发对「未发现」的判断都按这个标记，agent 未安装、验证失败都不
改变判定。冒烟失败退出 1，stderr 给脱敏后的原因。

单次冒烟最长等 180 秒。超时记「冒烟超时」（资源库 `verification` 为 `timeout`），
不算验证失败，派发前检查不拦，可再跑一次 `resources smoke` 重试；真身核对不通过
才记「验证失败」并拦下派发。

资源库的全部写入（探测、冒烟、清理、额度标记、首次派发的验证结果）走同一把
跨进程锁：资源库旁常驻的 `resources.json.lock` 上的系统文件锁（Windows
`LockFileEx`、Unix `flock`；句柄关闭或进程崩溃时由系统释放，等锁最多 40 秒，
超时报错）。窗口、命令行与后台派发进程都
拿它。冒烟、各 agent 的探测与 cc-switch 提示都在锁外跑；写回时重新读取最新
资源库、只合并本次结果，期间别的进程写入的额度标记与验证结果不丢。codex
模型缓存里的模型全部隐藏时按成功读出的空清单合并（旧条目标未发现），缓存
解析失败或没有任何带 slug 的模型才按探测失败处理、保留旧条目。

`resources prune [--agent <a>]` 从资源库删除最近一次探测未发现的模型（`meta`
带「未在事实源中」标记）；给了 `--agent` 只清这一家，用户申报的模型没有标记，
不会删。stdout 是清理后的资源（同 `resources`）外加两个键列表：`removed` 是被
删的 `agent/provider/model`，`referenced` 是其中仍被 modes.json 人选或
roles.json 角色引用的——这些照样删除，相关位置在 `config show` 里的
`effective.state` 变成「不在资源库」，需重新指定人选；stderr 写
「已清理 N 个未发现的模型」，有被引用的另说明一行。窗口命令 `resources_prune`
返回同一结构。删掉的模型再次探测发现后会重新加入资源库。

`resources find <名字>` 按用户口头说的名字在资源库里找模型，范围是所有 agent 下的模型，
不只看同名的 agent：「claude 5.5」可以对上 pi 下的 `claude-opus-5-5`。名字拆成字母段与
数字段，与模型编号、`resolved_id`、提供方、厂商、agent 比对；大小写、空格、横线、点号
不影响；claude ↔ anthropic / opus / sonnet / haiku、gpt ↔ openai、gemini ↔ google、
kimi ↔ moonshot、glm ↔ zhipu / zai 互认；code、cli、agent、model 这类词忽略；版本号从头
对上才算（5.5 对上 5-5，6 对不上 5.6）。stdout 为
`{ "query", "matches": [...], "related": [...] }`，每项有 `agent`、`provider`、`model`、
`resolved_id`、`vendor`、`state`、`note`、`usable`（可用 / 已验证 / 未验证 / 冒烟超时为
true）与 `target`（`<agent>:<provider>:<model>`，可直接给 `dispatch --seat`）。
`matches` 按能派发的在前、与说法多出的段少的在前，最多 20 个；一个都没对上时为空，
`related` 列出只按名字（不看版本号）对上的模型。退出码：有对上的 0，没有 1，名字里没有
字母或数字 2，资源库读不了 3。

`resources set-path <agent> [<路径>]` 记下某个 agent 的可执行文件路径（资源库该 agent 的
`customPath`），不给路径即清除；只认支持清单里的 agent，路径必须是已存在的文件，否则
退出 2。探测、冒烟、派发找程序时先用它，文件不存在时退回内置位置与 PATH。stdout 是该
agent 的条目（同 `resources`），stderr 提示重新探测后生效。窗口命令 `resources_set_path`
同一实现体。

`config show [--host <h>]` 输出 `{ "roles": …, "dispatch": …, "acp": …, "defaultMode": …,
"modes": … }`，`acp` 列出每家 ACP 适配器当前是否就绪，只查不装。`--host` 规则
见「主 agent 与 --host」；modes 段按该主 agent 的配置显示。`config set-role` 把角色指
到资源库里的模型，目标不在资源库时退出 2 并列出可选模型；目标是「未发现」的
模型（最近一次探测没有发现）时同样退出 2，原因与可选模型一并写在 stderr
（列表里去掉未发现的）；只接盲位置的 agent（agy 与未实测四家）设成
planning / execution 时退出 2（这两个角色默认补的是共享位置），acceptance 照常写入；
`roles.json` 损坏时退出 1，不写盘。`config show` 每个位置的 `effective` 另带
`invalid`：生效人选或其备选违反位置硬规则（只接盲的 agent 落在非盲位置）时为
原因，合规为 null。角色默认违反位置硬规则（只接盲的 agent 落在非盲位置、只看
得到文字的 agent 落在验收位）时补人不采用它：该位置 `source` 为 `未指定`、
`effective` 为 null，节点另带 `unfilledReason` 写明原因（其余情况为 null），
派发计划里同样不补。一键推荐凑不出合规人选时同样不退回它，notes 写明该位置
需手动指定；角色默认只是不在推荐范围（如厂商未知、未实测通道）时派发照样
补位，notes 写「使用角色默认 X（不在推荐范围：原因）」。`config set` 只认
`dispatch-settings.json` 的键
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
时不安装。窗口进程把适配器状态缓存 60 秒，`config set acpEnabled true` 装完
适配器后清缓存；命令行一次性调用不缓存、每次实查。

### 模式人选与默认模式（modes.json）

模式配置在 `~/.aicross/modes.json`（设了 `AICROSS_HOME` 时换成它，与
`roles.json` 同目录），按主 agent 分组（v2 格式）：

```json
{
  "version": 2,
  "hosts": {
    "claude": {
      "vendorOverride": "",
      "defaultMode": "builtin-blind",
      "counts": {},
      "seats": {
        "builtin-blind": [
          [
            { "agent": "pi", "provider": "zai-coding-cn", "model": "glm-4.7", "tier": "", "thinking": "中",
              "fallback": null, "origin": "manual" },
            null
          ]
        ]
      },
      "slashCommands": []
    }
  }
}
```

`hosts.<主 agent>` 各有一份 `vendorOverride`（空为不覆盖推定）、`defaultMode`
与 `seats`，另有一份 `slashCommands`（自定义短命令，见「自定义短命令」一节，
缺省为空数组）与一份 `counts`（内置模式的位置数量调整，键为内置模式 id、
值为可调角色的位置数，缺省为空对象；没有条目或值越界时该模式用默认形状）。`seats[<预设 id>]` 是「阶段数组 → 位置数组」，下标与该预设
`stages[i].nodes[j]` 一一对应，`null` 表示这个位置没设人选；内置预设与
`presets.json` 里的用户预设都可以有。人选字段 `agent`、`model` 必填，
`provider` 可为 null；`tier`、`thinking` 空串表示沿用预设的默认值，
`thinking` 只认 关 / 低 / 中 / 高 / 空串；`fallback` 可为 null，有值时
`agent`、`model` 必填；`origin` 是人选来源：`manual`（窗口或命令行手选）/
`recommended`（一键推荐填的），缺省视为 `manual`。agent 或 model 为空（含空
白）的人选按没设处理，继续走下面的补人顺序；非法的 `thinking` 不覆盖预设值。

version 1 的旧格式不做迁移（从未发布过）：宽松读取当空配置，严格读取报
「modes.json 是旧格式，删除后重新设置」并退出 1、不覆盖。

派发补人按顺序取第一个成立的：预设节点写死了模型，不动；主 agent 配置里
这个位置有人选，用它（`tier`、`thinking` 非空时覆盖预设值，`fallback`
写进目标）；否则按 `roles.json` 的三个角色补（与现有行为一致，擂台 / 盲审 /
评审模式整份跳过这一步）；都没有则保持空，派发前以「未指定模型」拒绝。
预设被改过、长度对不上时，派发与展示对多出的忽略、缺的按 null；下一次写入该模式的人选时，seats 整理成预设的形状，多出的旧人选清掉。`aicross dispatch` 不带
`--preset` 时用该主 agent 的 `defaultMode`，它为空或指向不存在的预设时用经济
模式；run 文件记录实际用的预设 id 与主 agent（`host`，无配置时不写出）。

内置模式的位置数量可调，每个模式只调一个角色的位置数：

| 模式 | 可调的位置 | 范围 | 默认 |
|---|---|---|---|
| 经济模式 | 第 3 阶段的「验证」 | 0–3 | 1 |
| 盲审模式 | 「验证」（另有 1 个「验收」固定） | 1–4 | 2 |
| 擂台模式 | 「执行」 | 2–4 | 3 |
| 评审模式 | 「验证」 | 1–3 | 3 |

共享模式与蜂群模式不可调；用户自定义模式的结构由编辑决定，不提供数量调整。
数量写入 `counts` 后，派发与 `config show` 按调整后的形状取预设：增加的位置
复制该角色的模板节点，减少的从末尾去掉；`modes.json` 里该模式超出的人选在
写入 counts 时一并去掉，增加的位置留空，由用户选或一键推荐填。

评审模式每个位置的视角可按主 agent 另设：modes.json 的 host 段存
`focus: { "builtin-jury": ["安全", "性能", …] }`，按扁平位置序存视角文字。
某项缺失或为空串时该位置用默认表「安全 → 性能 → 需求覆盖」；位置数量调小
后多出的项不删，调大后继续生效。视角文字可以是内置的六个（安全、性能、
需求覆盖、规范、正确性、可维护性，各有固定评审文本）或任意自定义文字
（派发时套统一模板「X评审：只从X视角审查这批材料，指出问题与改进建议。」）；
写入前校验：原串含换行或制表即拒绝（首尾也算），再去首尾空白后须为
1–20 字，含其它控制字符（含 ANSI 转义）或 Unicode 格式字符（零宽、双向
覆盖与隔离、BOM 等）拒绝。读取用同一套校验：手改 modes.json 写进的不合格
视角按默认表处理，不报错。视角与派发文本由同一张表给出，`config show` 与
盖章共用。

- `config show` 的 `modes` 按 `raw_presets_from` 的顺序列出全部预设，每项含
  `id`、`name`、`description`（模式介绍；内置预设的介绍写死，用户预设随
  预设保存）、`builtin`、`default`（是否该主 agent 实际生效的默认模式）、
  `count`（可调模式为 `{ "role", "value", "min", "max", "default" }`，`value`
  是当前生效的数量；不可调的模式为 null）与 `stages`；每个位置给 `role`、
  `visibility`、`write`、`focus`（评审模式各位置的生效视角：该主 agent 设了
  非空串用设的，否则按默认表取，超出的位置与其它模式为空串；评审模式
  位置另附 `focusOptions` 为可选的内置视角清单
  `["安全","性能","需求覆盖","规范","正确性","可维护性"]`，自定义文字
  不进清单）、`seat`（该主 agent 配置
  里 modes.json 原样，没设为 null）、`effective`（按补人顺序填好的人选，未指
  定时为 null；附 `state` 为该人选在资源库里的状态，资源库里没有这条模型时
  为「不在资源库」）与 `source`（`预设` / `位置` / `角色` / `未指定`）。
- `config set-default-mode <模式> [--host <h>]` 认别名与预设 id（与
  `--preset` 同一个解析）；不存在退出 2，stderr 列出可选的 id 与名称；写入后
  输出 `{ "defaultMode": …, "modes": […] }`。
- `config set-count <模式> <数量> [--host <h>]`：设定上表可调角色的位置数量，
  别名解析同上；模式不存在、模式不可调、数量越界都退出 2（越界时给出范围，
  如「只认 1–4」）；写入后输出该模式那一项（同 `config show` 的 `modes[i]`）。
- `config set-focus --host <h> --mode <模式> --index <n> --focus <文字>`：
  设定评审模式第 `n` 个带视角位置的视角（`n` 从 1 起，按该主 agent 调整后
  的形状算；目前只认 `builtin-jury`，其它模式退出 2）。`--focus` 给内置
  视角名或自定义文字（校验见上），`--focus ""` 恢复该位置的默认；模式
  不存在、序号超出当前位置数（如「只认 1–3」）或视角文字非法都退出 2。
  读改写与其它 modes.json 写命令同一把进程内锁。写入后输出该模式那一项
  （同 `config show` 的 `modes[i]`）。窗口命令
  `config_set_focus(host, mode, index, focus)` 返回同一结构。
- `config set-seat <模式> <阶段> <位置> … [--host <h>]`：`<阶段>`、`<位置>`
  从 1 开始数，范围按该主 agent 调整后的形状算；形状在写入锁内按严格读到的
  modes.json 取（与人选同一次读改写，不被并发的 set-count 改动隔开）；选项 `--agent`、`--model` 必给，
  `--provider`、`--tier`、
  `--thinking` 可选，备选 `--fallback-agent` 与 `--fallback-model` 成对给、
  `--fallback-provider` 要跟着另两项。模式不存在、阶段或位置超出范围、
  `--thinking` 非法、备选项缺配对退出 2（模式不存在时列出可选的 id 与名称）；
  人选或备选不在资源库、或指向「未发现」的模型，都退出 2 并列出该 agent 下可
  选的模型（列表里去掉未发现的）；人选或备选是只接盲位置的 agent（agy 与
  cursor、codebuddy、copilot、qoder），而该位置可见性不是「盲」时退出 2；
  资源库读不了退出 3。写入的人选 `origin`
  一律为 `manual`。写入后输出该模式那一项（同 `config show` 的 `modes[i]`）。
- `config clear-seat <模式> [<阶段> <位置>] [--host <h>]`：给了位置只清这一个
  （置 null），没给清掉该模式全部人选；输出同 `set-seat`。
- `config set-host-vendor <host> <vendor|auto>`：见「主 agent 与 --host」。
- `config recommend [--host <h>] [--mode <模式>] [--all]`：一键推荐，见下一节。
- 这些写命令遇 `modes.json` 损坏、旧格式、版本不认或读不了（如非 UTF-8）退
  出 1、不覆盖；`config show` 与派发按宽松读取，文件缺失、损坏或版本不认都
  按空配置处理。读改写全程持有 `modes.json.lock` 跨进程文件锁：窗口、命令行与不同主
  agent 同时改配置时依次写入，不互相覆盖；等锁超过 40 秒退出 1。
  `set-seat`、`clear-seat`、`set-count`、`set-focus`、`set-default-mode` 与
  一键推荐在同一把进程内锁里重新读取预设与 modes.json 再校验和写入，不按
  锁外读到的旧预设形状写回人选（与 `preset save` / `preset remove` 串行）。

### 一键推荐（config recommend）

```powershell
aicross config recommend --host claude
aicross config recommend --host claude --mode blind --all
```

不带 `--mode` 时对该主 agent 的全部模式做；带了只做这一个（认别名与预设
id，不存在退出 2）。不带 `--all`：只填空位置与 `origin` 为 `recommended` 的
位置，`manual` 不动；带 `--all`：全部重填（窗口里需用户确认后才传）。

候选池只有资源状态为「可用」（原生）或「已验证」的模型；`未验证`、`未发现`、
`验证失败`、`未登录`、`额度用尽` 都不进。档位与计费方式查随版本发布的小表
（`src/data/model-tiers.json`，agent 精确匹配、provider 前缀匹配（真实
provider 名可带后缀，如 `zai-coding-cn` 命中 `zai`）、模型名包含匹配）；表里
没有的档位为「未知」，排在已知档位之后。

规则：出方案、拆分优先高档，同档内订阅优先、按次排最后；执行优先中、低
档；执行与规划位在同一阶段内分散：已选过的同一模型往后排、已有过的厂商
也往后排，都是软条件，没有别的候选时允许重复（有多个执行位的阶段因此
不会全填同一个模型，只有一个执行位的阶段结果不变）；验收、验证的厂商必须
不同于主 agent 的实际厂商（主 agent 厂商「未知」时
不设这条限制，并在 notes 里说明「主 agent 厂商未知，无法保证跨厂商」），同
一阶段的验收 / 验证位置之间厂商互不相同，尽量也不同于该模式里执行位置的厂
商，已验证的第三方与订阅优先。主 agent 的原生模型（provider 为空，claude /
codex / kimi）与主 agent 走同一配置端点，厂商按主 agent 的实际厂商算，不会
被当成另一家选为审查者；pi 下的模型各带 provider，一律按资源库厂商算。认
不出厂商（「其他」）与没有厂商信息的模型不参与同阶段去重与主 agent 避让，
同一阶段也不重复选同一个模型。凑不出满足条件的人选时该位置留空，`notes`
写明原因，不降级硬填；有可用但未验证的第三方模型时，原因后面另加一句
「另有 N 个未验证模型，测试连接后可参与推荐」（N 为全部这类模型的总数）；
该位置的角色默认仍会补位时，另写一条「使用角色默认
X」；由此仍同阶段厂商重复或撞上主 agent 厂商的，写盘前再补一条冲突说明。结果确定：同输入同输出，候选同分时按资源库顺序。推荐计算与写
盘在配置写锁内完成：先严格读取 modes.json（损坏或旧格式退出 1、不覆盖），
算完只写本轮动过的模式，期间其它手选不会丢。

推荐过的人选变成未发现等状态后，下次推荐会替换它（因为它是
`recommended`）；`manual` 的只在 `effective.state` 里体现失效，不替换。

输出 `{ "defaultMode": …, "modes": […], "notes": […], "unverified": […] }
（`modes` 同
`config show`，含写盘后的人选与 `origin`；`notes` 为凑不出的位置与说明，
空时为 `[]`；`unverified` 是各 agent 下可用但未验证、因此没进候选池的第三
方模型数 `[ { "agent": "pi", "count": 33 }, … ]`，不含未发现的与原生的，没
有时为 `[]`，窗口 `config_recommend` 同结构）。modes.json 损坏或旧格式退出 1、不覆盖。

## skill 安装

```powershell
aicross skill status
aicross skill status --project D:\work\project
aicross skill install --host claude --scope global
aicross skill install --host pi --scope project --project D:\work\project
aicross skill uninstall --host claude --scope global
```

内置包随软件发布（skill 源仓去掉测试的副本），版本号取 `SKILL.md`
frontmatter 的 `metadata.version`。每个主 agent 会读取的位置：

| agent | 全局 | 项目 |
|---|---|---|
| claude | `~/.claude/skills` | `.claude/skills` |
| codex | `~/.agents/skills`，另扫描旧位置 `~/.codex/skills` | `.agents/skills` |
| kimi | `~/.kimi-code/skills`、`~/.agents/skills` | `.kimi-code/skills`、`.agents/skills` |
| pi | `~/.pi/agent/skills`、`~/.agents/skills` | `.pi/skills`、`.agents/skills` |

安装时写入的位置：claude 写 `.claude/skills/ai-cross`；codex、kimi、pi 写
`.agents/skills/ai-cross`（全局在家目录下，项目在项目目录下）。装好的目录里有
标记文件 `.aicross-install.json`（版本、安装时间、来源）。

每个位置的判定：不存在为 `none`；是链接（symlink / junction，记下指向）或没
有标记文件、或路径存在但不是目录为 `external`（由其他软件管理）；普通目录且
含标记文件为 `ours`。各位置都读出 `SKILL.md` 的版本号（读不出为空）。

`skill status` 输出 `{ "bundledVersion": …, "hosts": [{ "host", "locations":
[{ "scope", "path", "kind", "version", "outdated", "manager", "linkTarget",
"readByThisHost", "writeTarget" }], "duplicate", "updateAvailable" }] }。
路径逐段拼接，Windows 上统一反斜杠。`writeTarget` 表示该位置是不是「我们安
装时写入的位置」，前端据此决定按钮状态，不按路径猜。`outdated` 表示该位置
由其他软件管理（`kind` 为 `external`）且版本可解析、低于内置版本；我们装
的那份不用它，用 host 级的 `updateAvailable`。外部位置另附 `manager` 标明
管理方：位置是链接且目标在 `~/.skills-manager/` 下为 `"skills-manager"`
（位置与该目录都解析成真实路径再比，带 `..` 的相对目标与链式链接都认；
解析不了时按链接文本判断）；
否则全局位置在 skills.sh 全局锁文件（设了 XDG_STATE_HOME 用
`$XDG_STATE_HOME/skills/.skill-lock.json`，未设用 `~/.agents/.skill-lock.json`）、
项目位置在项目根的 `skills-lock.json` 里登记了 `ai-cross` 的为
`"skills.sh"`；其余为空串。锁文件读不了或格式不对当作未登记，不报错；
链接指向与锁登记同时命中时按链接指向算。同一物理目录只列一次
（`--project` 与家目录相同时不重复）；同一主 agent 能读到两处以上 ai-cross
时 `duplicate` 为 true；我们装的那份版本与内置不一致时 `updateAvailable` 为
true。`--project` 给的目录不存在时退出 2。

`skill install --host <h> --scope global|project [--project <dir>]`：写入「我
们安装时写入的位置」。目标为 `external` 时退出 1「由其他软件管理：<路径>」，
不覆盖；为 `ours` 与 `none` 时先写到临时目录（含标记文件），再把旧目录改名
成备份、临时目录换名到位，成功后删备份，换名失败时把备份改回原名——更新中
途失败时旧安装原样保留，不留半成品；备份改不回原名时保留备份，下一次安装
先恢复它。临时目录与备份目录建在暂存目录里：skills 目录解析成真实路径
（父目录被 junction / symlink 重定向到别的盘时按解析后的位置）后，取它上一
级的 `.aicross-install-<skills 目录名>/`，与目标同卷；内容在其下一层
`ai-cross/`，名字带 uuid，宿主不会把它当成 skill。安装、更新与卸载跨进程
互斥：暂存目录里常驻的 `install.lock` 上的系统文件锁（句柄关闭或进程崩溃时由
系统释放，等锁最多 40 秒），进程内另有一把写锁（codex、kimi、pi 的写入位置
同为 `.agents/skills`）。临时与备份目录的根目录里先写操作记录
`.aicross-op.json`（目标路径与操作令牌）再放内容。拿到锁后先处理上一次中断
留下的残留，只动归属本次目标的：暂存目录里要有操作记录且记录的目标与本次
一致；早期版本放在 skills 目录与其上一级的旧格式要带我们的安装标记；残留
根目录或内容目录是 symlink / junction 的一律不动。归属的残留里，目标缺失而
有带标记的备份时（安装 / 更新）先把最新一份改回目标，其余删掉；卸载不恢复，
直接清掉。不归属的保留，写进输出的 `notes`（没有时不写出该字段）。
`skill status` 只读，不拿锁、不清理任何残留。项目范围必须给 `--project`，
目录不存在退出 2。写完输出该 host 的 status 项。

`skill uninstall` 只删我们装的那份（`ours`）；`external` 退出 1 不动；没装过
时静默成功。

## 自定义模式

```powershell
aicross preset save --file D:\tmp\mode.json [--host claude]
aicross preset remove <id>
```

用户预设存在 `~/.aicross/presets.json`（设了 `AICROSS_HOME` 时换成它），
`{ "version": 1, "presets": [...] }`；内置预设不落盘，读取时合并在前。
`preset save` 的 json 是一个预设对象：

```json
{
  "id": "my-mode",
  "name": "我的模式",
  "description": "一句话介绍",
  "stages": [
    { "nodes": [ { "role": "执行", "visibility": "共享", "write": "单写者", "thinking": "中" } ] }
  ]
}
```

没有 `id` 时新建（生成 uuid）；显式给了 `id` 时更新同 id 的用户预设，id
不存在退出 2「模式不存在：<id>」，不静默新建。内置模式的别名
（shared / economy / blind，与 `--preset` 同一张表）不能作用户预设 id，
否则同名预设永远轮不到被解析。校验不过退出 2，原因写明是哪一条：

- `name` 非空，最长 40 字，不与其它预设（含内置）重名；`builtin: true` 的
  预设不可修改、不可删除。
- `name` 与 `description` 不收换行、制表以外的控制字符与 ANSI 转义；
  `description` 最长 200 字。
- 至少一个阶段，每个阶段至少一个位置，全模式最多 12 个位置。
- `role` 只认 出方案 / 执行 / 验收 / 验证 / 拆分；`visibility` 只认
  共享 / 盲；`write` 只认 只读 / 单写者 / worktree；`thinking` 只认
  关 / 低 / 中 / 高 / 空串。
- 同一阶段最多一个「单写者」位置（同一目录同时只有一个写者）。
- 位置的 agent / model 不在这里设（人选走 `set-seat`），传入的一律忽略，
  保存时清空。

写盘用原子写；读改写与短命令、人选共用同一把进程内锁。`presets.json`
损坏、读不了或版本不认时退出 1、不覆盖。更新已有预设时，阶段数、每阶段
位置数或任一位置的角色变了，各主 agent 下该模式的人选一并清掉（短命令与
默认模式保留），受影响的主 agent 列在输出的 `seatsCleared`。保存后输出
`{ "preset": …, "seatsCleared": [主 agent…], "defaultMode": …, "modes": […] }`，
preset 为保存后的预设（新建时返回生成的 id），modes 同 `config show`；
`--host` 规则同 TASK-50，可缺省。

`preset remove <id>` 删除用户预设，并级联清理：各主 agent 下该模式的人选、
指向它的默认模式（清空）、`slashCommands` 里 mode 为它的定义与对应的带
标记短命令文件。顺序是先在锁内逐个删带标记的文件，全部成功后一次写入
modes.json，最后删预设；任一步失败停止，错误说明里列出已删与剩下的条目
（host / 命令名）。定义的 host 或名称不合法时只从定义里移除、不碰磁盘，
在输出的 `affected.invalidSlash` 里单独标出。输出
`{ "removed": { "id", "name" }, "affected": { "seats": [主 agent…],
"defaultMode": [主 agent…], "slash": [{ "host", "name" }],
"invalidSlash": [{ "host", "name" }] } }`，slash 列出删了定义与文件的短
命令，invalidSlash 列出只从定义里移除的坏定义。

窗口命令 `preset_save(preset)`、`preset_remove(id)` 返回同一结构。派发对
用户预设的支持不变：`aicross dispatch --preset <id>`。

## 自定义短命令

```powershell
aicross slash list --host claude
aicross slash add --host claude --name cross-review --mode blind [--note <说明>]
aicross slash add-all --host claude [--prefix <前缀>]
aicross slash remove --host claude --name cross-review
```

用户给某个模式（及可选的固定说明）起一个短名，软件按主 agent 的格式把一句
固定指令写进家目录，之后在主 agent 里输入短命令即可按该模式派发。定义存在
modes.json 的 `hosts.<主 agent>.slashCommands`：

```json
"slashCommands": [
  { "name": "cross-review", "mode": "builtin-blind", "note": "" }
]
```

`name` 只允许小写字母、数字与 `-`，1–32 位；`ai-cross`（与 skill 本身的
命令冲突）与 Windows 设备名（con、nul、aux、prn、com1–com9、lpt1–lpt9）
是保留名，不能作命令名，退出 2。`mode` 是预设 id（别名解析同
`--preset`），不存在退出 2；`note` 为可选的固定附加说明，单行且不超过 200
字，原样接在指令后面；说明以 `-` 开头时写成 `--note=<说明>`，否则会被
当成另一个选项。校验不过的原因写在 stderr。

各家写入的路径与输入写法：

| agent | 文件 | 输入写法（trigger） | 默认调用（invoke） |
|---|---|---|---|
| claude | `~/.claude/commands/<名>.md` | `/<名>` | `/ai-cross` |
| codex | `~/.codex/prompts/<名>.md` | `/prompts:<名>` | `$ai-cross` |
| pi | `~/.pi/agent/prompts/<名>.md` | `/<名>` | `/skill:ai-cross` |
| kimi | `~/.kimi-code/skills/<名>/SKILL.md` | `/skill:<名>` | `/skill:ai-cross` |

文件内容是 frontmatter 加一句固定指令：`使用 ai-cross skill，按「<模式名>」
处理下面的请求；派发时带 --host <主 agent>。<说明>`。frontmatter 各家
不同：claude 与 codex 有 `description`、`argument-hint: <任务>`，pi 只有
`description`，kimi 另有 `name`；claude 与 kimi 另带
`disable-model-invocation: true`，这两家的模型不会自己触发短命令，只能
由用户输入（codex 与 pi 的 prompts 本来只能由用户输入，不加）。参数占位
claude 与 codex 在指令后另起一行写 `$ARGUMENTS`，pi 与 kimi 的参数会追加
在指令后，不写占位。`description`
为「ai-cross · <模式名>」，与 kimi 的 `name` 一样写成带引号的字符串并做
转义；模式名与说明在正文里的换行、控制字符替换为空格。我们写的文件都在
frontmatter 里带 `aicross-managed: true` 标记（判定标记时先去掉开头的
BOM；标记须顶格、在闭合 `---` 之前，行尾允许 `# 注释`）。

写入用临时文件加 rename；同名定义已存在则更新（覆盖我们自己的文件）。
目标文件存在但 frontmatter 里没有 `aicross-managed: true` 时视为别人的，
不覆盖，退出 1「已存在同名命令（非 aicross 管理）：<路径>」；kimi 还会读
`~/.agents/skills` 与 `~/.kimi-code/skills`：同名目录已存在而我们没有带
标记的 SKILL.md 时同样退出 1。add 先写文件再存定义，
存定义失败时回滚文件（新建的删掉，更新的恢复原内容）。`remove` 先删带
标记的文件（kimi 只删我们写的 SKILL.md，目录空了才连目录一起删）再存
定义，删文件失败时定义不动；定义不在但文件带标记时也清掉文件；别人的
文件与不存在的文件都不动，定义与文件都不在时退出 2。
modes.json 损坏、旧格式或读不了时 add、add-all 与 remove 退出 1、不覆盖（不写文件也
不改定义）。定义与文件的同一次变更放在同一把进程内锁里，窗口与命令行共用。

内置六个模式的短名与关键词固定，按此顺序列出：shared 共享、economy 经济、
blind 盲审、arena 擂台、jury 评审、swarm 蜂群。`slash list` 输出
`{ "host": …, "invoke": …, "commands": [...], "modes": [...] }`，commands
每条含 `name`、`mode`、`modeName`（预设名；预设已删时回落成 id）、`note`、
`path`、`state`、`stale` 与 `trigger`。`state` 按磁盘实际判断：文件存在且带
标记为「已写入」；存在无标记为「被占用」；不存在为「未写入」。命令目录
（kimi 是 `~/.kimi-code/skills/<名>/`，其余家是 commands / prompts 目录）或
命令文件本身是 symlink / junction（含悬空链接）时一律按「被占用」，add 不写、
remove 与 `preset remove` 的级联删除不删；元数据读不到（确实不存在除外）同样
按「被占用」。写入与删除共用这一个判定；modes.json 里的
名称不合法时为「名称无效」（不读磁盘）。`stale` 表示文件带我们的标记、但
内容与现在会生成的不一致（如旧版没有 `disable-model-invocation`，或模式
改名后文件里仍是旧名）：同名 add 即重写，界面据此提示更新；只有状态为
「已写入」的条目才比对，比对前去掉 BOM、CRLF 统一成 LF，其余为 false。add 与 remove 之后输出同 list。
`modes` 逐个列出内置模式，每条含 `mode`（预设 id）、`modeName`、
`description`（模式介绍）、`argForm`（默认调用写法加关键词，如 claude 的
`/ai-cross 盲审`）、`suggestedName`（`ax-<短名>`）、`command`（`commands`
里 mode 等于该模式的第一条，没有为 null）与 `existing`：该模式还没有登记
的命令、但建议名对应的命令文件已在磁盘上时给出
`{ "name": "ax-blind", "state": "已写入" | "被占用", "path": … }`（带标记的
遗留文件重装或换数据目录后常见，`slash add` 可直接登记覆盖；无标记的
`slash add` 照旧拒绝），文件不在或已登记时为 null。

`slash add-all` 一键生成内置模式的短命令；用户自定义预设没有稳定短名，
不参与，仍可手动添加。`--prefix` 缺省 `ax-`，只认小写字母、数字与 `-`；
拼出的每个名字都要过命令名校验，不过退出 2，不写任何文件。对每个模式：
该模式已有命令（任何名字）则跳过；生成名已被别的模式的定义使用时不改写，
连同原因记入 skipped（「名称已被 <模式名> 使用」）；否则按 `<前缀><短名>`
走 add 的同一份写入流程。单个失败不中断，连同原因记入输出。输出同
`slash list`，另加 `added`（已生成的名字）与 `skipped`（`{ name, reason }`）。
窗口命令 `slash_list(host)`、`slash_add(host, name, mode, note?)`、
`slash_add_all(host, prefix?)`、`slash_remove(host, name)` 返回同一结构。
`--host` 规则同 TASK-50：必须是检测到的 agent，且各子命令都要有具体 host
（文件按 host 分路径写家目录）。

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
`~/.aicross/graphs/`；用户预设在 `~/.aicross/presets.json`；派发设置在
`~/.aicross/dispatch-settings.json`。设了
`AICROSS_HOME` 时以上 `~/.aicross` 都换成它。命令把
JSON 写到 stdout，把一行说明写到 stderr。
