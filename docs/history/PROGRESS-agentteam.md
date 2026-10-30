# 进度

> 文中以 `bench/`、`archive/`、`articles/`、`k3-eval/`、`eval-`、`dist/` 开头的相对路径指本地资料目录 `D:\work\agentteam`，不在本仓库。

> 更新：2026-10-05。只记现状与下一步；09-30 以前的逐日流水在 `archive/PROGRESS-0707-0930.md`。
> 产品设计看 `D:\work\aicross\docs\PRODUCT.md`，skill 规格演化看 `SPEC.md`，竞品事实看 `COMPETITIVE-LANDSCAPE.md`。

## 方向（10-05 用户定）

面向其他用户的交付只有 skill。用户反馈的四个问题——不会用、本机模型每次抓取不准、不会写记忆和默认配置、每次都要指定——同一个原因：该由程序做一次并存下来的事，原先是让模型每次照说明重做。解法是把这些事做成 skill 随附的脚本，配置存用户主目录、各宿主共用。

**10-06 用户定**：aicross 重新开启，定位为编排配置器（参照 CC Switch：窗口里检测资源、按模式选人与默认模式，skill 读配置照办）。设计见其 `docs/PRODUCT.md`「状态」「编排配置器」。

## 阶段一：ai-cross skill（主线）

源仓 `D:\work\skill\xiaoyu-skill\skills\ai-cross`，当前 **v1.22.0**。`dist/ai-cross` 停在 v1.16.0，不再改。

已完成：跨厂商交叉验证与盲验四级隔离、路由模型（档位 / thinking / 亲自算铁律 / 升级阶梯）、各通道（内部 subagent / codex / claude -p / cc-switch / kimi / pi / Antigravity / 裸 API / aichat / aicross 引擎）、盘点脚本、派发脚本与三个只读桥脚本、三宿主端到端、五轮基准、三组评测与文章 01–06。

**10-05 已完成**：skill v1.19.0（xiaoyu-skill `36793dd`）——盘点脚本 `inventory.py`：只读探测本机模型入口，保存前逐个核对模型 ID，清单存 `$AICROSS_HOME/skill/manifest.json`（各宿主共用，skill 更新不丢），没点名时由脚本选默认审查者与执行者。单元测试 26 例；haiku 子代理只读文档实走盘点，8 条命令零报错。细节见 `SPEC.md` 修订 v1.15。本机还没有用它建正式清单，新会话里说「盘点」即可。

同日 skill v1.19.1（`6ca4f84`）：触发说明补日常说法；用户带着任务来时先登记到保存为止、只冒烟要用的那一条再干活；skill 在会话里用过之后，交付关键产出时主动提一句是否盲审。触发匹配 16 句全对，首句为普通请求的流程实走通过（`SPEC.md` 修订 v1.16）。

同日 skill v1.20.0（`217cb8c`）：派发脚本 `dispatch.py`——`review` 让别家审（空目录隔离、冻结模板、真身核对、留痕）、`run` 把活派给执行者、`smoke` 逐档冒烟；不带 `--go` 只出方案。六条通道真机跑通（冒烟 12 个模型、三家盲审、GLM 执行并自测通过）。单元测试全仓 57 例，CI 通过。回归检查与四例行为实测见 `SPEC.md` 修订 v1.17，其中一例首测失败（用 `review` 发实现任务），补 `run` 后复测通过。

同日 skill v1.21.0（`dcbbe7d`）：主文件 `SKILL.md` 从 54 KB 瘦到 17 KB（粗估 1.6 万 token → 4.5 千），完整规则原样搬进 `references/rules.md` 按需读；`run --read-dir` 让被派方进目录只读（三条通道真机验证改不了文件）；留痕按材料位置推断项目根。瘦身后七个场景的行为实测与一次测试事故见 `SPEC.md` 修订 v1.18。单元测试 61 例，CI 通过。

10-25 skill v1.22.0（xiaoyu-skill `22b787e`）：盘点后能发现新模型——`inventory.py diff` 与 save 时记下的各通道清单比对，报新模型、已下线的档位（附可换项）、未纳入的通道；`set --tier` 换单个档位、`set --ack` 不再提示；`pick`/`show` 超 7 天未比对时提醒。单元测试 70 例。真机首跑报出 `agy.google` 的 high 档 `gemini-3.1-pro-high` 已下线（可换 `gemini-4-argon-high`），待用户决定换档后冒烟。见 `SPEC.md` 修订 v1.19。

同日 skill v1.23.0：装了 aicross 时先读 `aicross config show --host`，按窗口里设的默认模式与人选派发、不再要求盘点，开场一行写明「按你在 aicross 里设的<模式名>」；去掉「未公开发布」。未在新会话实走；aicross 内置 skill 包仍是 1.21.3。见 `SPEC.md` 修订 v1.20。

**下一步**：

1. 在真实宿主里完整走一遍（含用户确认环节）：本机用新版建正式清单、做一次真实盲审；Codex 宿主同样走一遍。这是 v1.10 起挂着的 T3。
2. 聚合型宿主（WorkBuddy 等）做一次端到端实测；目前只在 Claude Code、Codex、Qoder 上验证过。
3. 手写派发那条路（要被派方自己改项目文件、`manual:` 通道）在规则搬到 `rules.md` 之后没有做过行为实测。
4. 没有 Python 的用户走手工清单兜底，体验没有改善；占比未知，有反馈再处理。

**本机约定**（用户 10-05）：本机已不用 cc-switch，GLM 走 pi 的 `zai-coding-cn`；盘点时不纳入 `cc-switch:` 条目。cc-switch 通道为其他用户保留。本机 cc-switch 里智谱的 sonnet / opus 档 `GLM-5.3-Flash[1M]` 已失效（`verify_model.py` 10-05 实测），因不再使用，不必修。

未收口：

- 行为实测：1.8.2 之后没在真实新会话里复测 T1–T3（SPEC v1.10 列为待办）。
- 研究欠账：跨节深缺陷 ⑥ 加 n 坐实、英文平行版、同模型跨提供商对照、受限解码下官方模型是否塌方（SPEC v1.5 五之补、`bench/RESULTS-RIGOR-20260710.md` 局限节）。

## 阶段二：aicross（2026-08-30 起；10-05 冻结，10-06 重新开启为编排配置器）

**10-25**：新人首屏「开始」页（TASK-63，`53a0419`）：探测 → 选主 agent、一键配好（装 skill + 一键推荐）→ 怎么用；内置 skill 包同步 1.23.0（`c15181d`）。vitest 240、cargo 699 通过；空数据目录真机模拟走通（REPORT-63）。发版 v0.1.2（`c6e19a3`，GitHub Release 附 NSIS 与 MSI）。

**10-25**：TASK-64（aicross `466d42a`…`bcdad77`）：仓库改公开；去掉全部 Coffee CLI 代码（窗口外壳、启动 PATH、git 统计、Job Object、安装脚本重写），许可改 MIT，版本 0.2.0；ai-cross skill 带 33 个提交并入 `skills/ai-cross`，xiaoyu-skill 移除（`9a5cc70`）。顺带查出 0.1.2 及之前安装包从未写入 PATH（内联命令超 NSIS 1024 字符上限），已改用钩子加脚本文件，真机装卸验证。vitest 240、cargo 694、skill 70 通过。

**10-25**：TASK-65（aicross `3baa5f7`…`3bbb06e`，0.3.0，未推送）：别的软件装的旧版 skill 可一键升级（备份原目录），自己装的随软件自动更新；Tauri 官方更新器（私钥 `~/.tauri/aicross-updater.key`，`scripts/build-release.ps1` 签名并生成 latest.json，每次发布都要上传）；主 agent 加 OpenCode、Gemini CLI、Antigravity、Hermes、ZCode、CodeBuddy、WorkBuddy、Qoder、Qoder CN；开始页缺人提示改写。本机更新链路 0.3.0→0.3.1 实测装上并重开。

**Current（10-10）**：aicross 已推送 GitHub 并首次发版 v0.1.0（见下）。main 上 cargo 697、vitest 227 通过。
- 10-07 合并：编排配置器全部（`6878f75`、`5fe8902`、`b1d525e`）；两行 ai / cross 图标（`57b33dd`）；内置 skill 1.21.3（`641dad7`）；支持清单与 Antigravity（TASK-61 `e6d4357` / TASK-62 `0b5428f`，Codex 只读盲审「修后合并」→ fix-61-r1 八条、界面两条）。
- 10-07 用户定：资源以软件为准，skill 盘点清单只给没装软件的人用、两份不合并（PRODUCT.md「编排配置」）；资源页按支持清单探测，未实测的标出来、不进推荐（PRODUCT.md「更多 agent」）。
- 真机：空数据目录下探测出 5 家、67 个可用模型（codex 隐藏模型已过滤）；一键推荐把 Gemini（`agy/gemini-3.8-flash-high`）填进盲审与评审第三位；agy 盲派发答对。
- ai-cross skill 1.21.3（xiaoyu-skill `9001718`）：认 aicross 宿主名 `claude` / `agy`，盲审材料按相对路径标注。
- 10-08 全面检查：Codex 分三块只读审查（22 条，无 P0）+ 窗口走查 + 真机三家盲审；Opus 修三轮、每轮 Codex 复审，合并 main（`9ce73cf`、`a13b760`、`4236b04`，未推送）。cargo 696、vitest 227。要点：skill 状态查询只读、安装与资源库改用系统文件锁、残留只清带记录 / 标记的、链接一律不动；验证失败 / 未发现的模型在派发入口对所有阶段与备选拦截；如实汇报句在最终外发文本末尾只出现一次；盲审单独用时改用独立审查模板；界面写入串行、下拉与卸载修正。窗口内操作未能复测（窗口反复被最小化），界面改动靠单元测试与命令行端到端核对。
- 10-10 首次发版 v0.1.0：仓库推送 GitHub（私有），用户提的 PR #1（10-08 三提交 + modes.json 写入跨进程锁）合并（`6819470`），合并前本地全测 cargo 697、vitest 227 通过。打包时发现自定义 installer.nsi（Coffee CLI fork 的旧 tauri 模板）缺 `Win\RestartManager.nsh` include，tauri-cli 2.12 的 utils.nsh 调它的宏导致 NSIS 打包失败，补 include 修好（`89d6534`）；顺带把 MANUAL 的 MSI 文件名更正为 zh-CN 实际产物（`60b92d5`）。tag v0.1.0 定在 `60b92d5`，GitHub Release 附 NSIS setup.exe 与 zh-CN MSI；PRODUCT.md 记渠道定为 GitHub Release（`13046fb`）。随后 `aicross --help` / `--version` 改为命令行直出即退，不再落到窗口（`b63bfa5`，cargo 699）；当天接着发 v0.1.1（`0b17547`）：版本号三处与 MANUAL 文件名对齐 0.1.1，GitHub Release 附 0.1.1 两个安装包，release 二进制 `--help` / `--version` 真机核对通过。

- TASK-46 引擎侧：GLM-5.3-Flash 实现，工作树 `D:\work\aicross-wt\t46`（分支 `task-46`）。盲审 Claude 子代理 + Gemini，修正 9 条，复审「可以合并」，编排者追加 3 条低危修复。`cargo test` 483 通过、零 warning。
- TASK-47 窗口三页：Claude 子代理实现，工作树 `D:\work\aicross-wt\t47`（分支 `task-47`）。盲审 GLM + Gemini，修正 6 条，Gemini 复审后编排者追加 2 条。vitest 49 通过，build 通过。
- 集成工作树 `D:\work\aicross-wt\int`（两期改动合在一起，未提交）里真机核对通过：命令行设盲审三位人选与默认模式后不带 `--preset` 派发，run 记 `builtin-blind`、三路按人选派出；窗口三页、改人选与清除、冒烟时切页签不卡、`aicross open` 与关闭回「最近的派发」。测试用的 `modes.json` 已删除。

**10-06 追加两期（用户：qwen 与 glm 走 pi 的验证要做好；界面重做成 CC Switch 式小窗、跟随系统配色）**：

- TASK-48 资源库「未发现」的模型（pi 升 1.0.2 后 `qwen-token-plan-cn`、`deepseek` 两个 provider 已不在 pi 里，资源库仍显示可冒烟、`deepseek/deepseek-v4-flash` 仍为已验证）：GLM-5.3-Flash，工作树 t46。重新探测后对 `opencode-go` 的 `qwen3.8-flash`、`glm-5.3-flash` 冒烟，真身核对均通过（`resolved_source: identity`）。
- TASK-49 窗口重做：Claude 子代理，工作树 t47。
- TASK-48 完成：盲审 Claude + Gemini → fix-48-r1（8 条）→ 复审「可以合并」→ 编排者追加没给 provider 时优先取未带标记的同名条目。`cargo test` 502 通过。集成工作树真机核：重新探测后 `deepseek`、`qwen-token-plan-cn` 下 27 个模型为「未发现」，设人选退出 2、冒烟退出 1，同名多 provider 未给 provider 时提示指定。
- TASK-49 外观：r1（照 CC Switch 截图：图标导航、橙色主按钮、图标按钮、生效态蓝框、资源 → 模式 → 派发记录）用户看后「单调，模块之间得处理下」；r2 进行中（页顶数字卡、模式分组与彩色标识、流程条胶囊、资源页厂商色与小统计、派发记录筛选；恢复 TASK-40 前的 agent 图标并给每个模式配图标）。

- TASK-49 外观 r3（删说明句）用户认可方向；代码盲审 GLM + Gemini → r4 修正，vitest 91 通过。
- 10-06 用户定（写进 PRODUCT.md）：定位为 skill 的伴生工具；skill 安装支持全局（推荐）/ 项目 / 其他软件管理，我们装的由我们管版本；一键推荐只填空位与推荐位、不动手选；配置按主 agent 分开存，左栏选主 agent。合并进 metrik 或向 CC Switch 提案留到以后。
- TASK-50（引擎：主 agent 与厂商推定、`modes.json` v2 按主 agent 分组、`config recommend`、`aicross skill status/install/uninstall`、内置 skill 包）：GLM，t46。
- TASK-51（窗口：左栏主 agent、skill 页、一键推荐）：Claude 子代理，t47。

- TASK-50 完成：盲审 Claude + Gemini → fix-50-r1（18 条）→ 复审 → fix-50-r2（8 条，含 pi 主 agent 厂商改写过宽）；编排者另修 kimi 厂商推定（provider 在 `[models."<default_model>"]`）。`cargo test` 546 通过。真机核：四家主 agent 厂商推定正确（pi 因默认模型已撤为「未知」）；推荐对 claude / kimi / pi（临时设为 Alibaba）都避开主 agent 厂商且同阶段互不相同；skill status 识别 skillmanager 的 junction 为 external、往其上全局安装被拒，项目安装 / 更新 / 卸载在临时目录正常。
- TASK-51 + 外观：magpie 风格（顶部分段导航、主 agent 下拉、一卡多行、行内带图标可搜索的模型下拉、数字条、绿色主按钮），前端 vitest 121 通过。应用图标换为 Leckerli One 绿色字标：128 以上全名 aicross、64 以下 ax（`icons/icon-master*.png` + `scripts/rebuild-icons.py`）。
- Later：档位表调优（推荐会选到 `codex-auto-review`、`claude/fable` 这类不宜做审查者或最贵的模型）；`detect_all_resources_at` 盖章行无直接测试。

- 10-06 晚用户真机反馈后追加：TASK-52（`resources prune` 清理未发现、模型条目 `missing` 字段、引擎文案改短）与界面 fix-51-r2/r3（「不可用」筛选、原生不显示已验证、卡片折叠含检测失败卡片、全面去口语与黑话、主 agent 固定显示与厂商行精简、清理按钮、顶栏不截断）。引擎 552、前端 133 测试通过；清理在数据副本上实测（codex 1 条、全部 27 条），真实资源库未动。设计追加：斜杠命令默认 `/ai-cross` 带参数，软件内可自定义短命令（PRODUCT.md「在主 agent 里怎么用」）。

- TASK-53 斜杠命令：默认调用写法按主 agent 显示（claude `/ai-cross`、codex `$ai-cross`、pi / kimi `/skill:ai-cross`，按官方文档核实），skill 页可自定义短命令并写入各家命令目录（带 `aicross-managed` 标记，只动自己写的）。审查 Claude（读两侧代码）+ Gemini → fix-53-r1（引擎 7 条、界面 6 条）。引擎 572、前端 141 测试通过；真机用 `aicross-selftest` 在 claude / codex / pi 三家写入、核对、删除，测试后清掉了空目录；kimi 写的是 skills 目录，未在真机写。

**交接（10-06 晚，用户：明天测试）**：全部改动未提交。改动分在两个工作树——引擎 `D:\work\aicross-wt\t46`（分支 task-46，含 TASK-46/48/50/52/53）、窗口 `D:\work\aicross-wt\t47`（分支 task-47，含 TASK-47/49/51/53 前端与新图标）；集成试用在 `D:\work\aicross-wt\int`（两边合在一起，未提交，编译目录 `D:\work\aicross-wt\int-target`）。启动试用窗口：在 int 下 `CARGO_TARGET_DIR=D:/work/aicross-wt/int-target npx --yes -p @tauri-apps/cli@2 tauri dev`（开发服务器偶尔给旧文件，先删 `D:\work\aicross\src-ui\node_modules\.vite` 再启动）。`docs/PRODUCT.md` 的最新版在主仓 `D:\work\aicross`（未提交）。资源库里 28 条未发现模型尚未清理，由用户在窗口里决定。

**Next（10-07）**：改 ai-cross skill：装了 aicross 时读 `aicross config show --host` 照办、不再自己盘点，路由行写明人选来源；未实测四家（Cursor、WorkBuddy/CodeBuddy、Copilot、Qoder）等用户装上后按官方文档核对命令并真机冒烟，再去掉「未实测」放开派发；aicross 是否推送 GitHub 待用户定。

**Later**：
- 盲审模式单独派发时审查者只收任务文本、看不到项目文件（10-06 真机派发三家都答「看不到 calc.py」）；skill 接入时要把材料写进任务文本，或给 `dispatch` 加附件参数。
- `resources detect` 输出不含 cc-switch 发现提示（原有行为）。
- 资源页冒烟只能逐个点；agent 级未登录 / 版本过低判定未做。
- `tasks/README.md` Runbook 写 `src-ui/src/tauri.ts` 是混合换行，实际为纯 LF。

以下是冻结时的状态。

**形态（09-30 定案）**：对话留在宿主，aicross 只做派发引擎（命令行）和结果查看器。理由与规则见 PRODUCT.md「为什么是这个形态」。

**已交付**：TASK-00 至 TASK-35 全部合并 main（最后 `dcaf39c`，09-21）。按主题：
- 引擎：派发执行器与各家通道、盲审隔离、真身核对、`.dispatch/` 留痕、经济流水线与产物交付、修正轮上限与遗留、红绿核对、额度切备选、崩溃对账与回滚、ACP、蜂群合并（窗口版）。
- 窗口界面：对话图、预设、接入页、控制台单线聊天、建议条、结果详情——已随 TASK-40 下线，只留首页「最近的派发」与查看页。

**进行中**：无。10-01 定案的四项（验收条件文件、ACP 打开时安装、崩溃后核对，以及积压中危清零）全部落地。

**10-01 已完成**：
- **TASK-36 引擎无窗口可跑**：合并 main（`2316fab`、`0f93482`）。R0 codex → 双盲审（GLM + kimi）→ R1 kimi（金标准逐夹具重放 31 场景）→ 复审（GLM + Claude 子代理）→ R2 GLM（中文路径崩溃、并发写合并等）→ 编排者补修落图合并两处偏差。真机核：派发经管道 1 秒返回、经济模式跑完、采纳只改图、取消 1 秒收尾；杀后台进程后 status 仍报 running → 补查退出码后复测通过。
- **TASK-37 查看器第一版（看 + 采纳）**：合并 main（`3e0f093`）。R0 GLM → 编排者发现发布构建编译失败（只在发布构建编译的单实例转发代码）→ 复审 Claude 子代理 + Antigravity（kimi 撞额度改派）→ R1 GLM（取消采纳无效、真身大面积误报、模式名改为 run 文件记 `preset_id`、冷启动 `open` 不再阻塞等 12 条）→ 发布构建窗口真机核全部通过；真机核另发现卡片标题与遗留串到别的 run（同项目共用一张图），编排者在查看器侧修好。验证命令新增 `cargo check --release`。合并后 main 全绿（cargo 564 / vitest 744）。
- **积压 94** 合并（`c33b0f1`）；**积压 100** 随 TASK-37 关闭。
- **skill v1.17.0 / v1.17.1**（xiaoyu-skill `d4d35a5`、`236ff9b`）：「aicross 引擎通道」——不需要窗口、退出码 3 的新含义、派发后不在宿主里长时间干等、`aicross open` 查看器。
- **TASK-38 每次派发新建一张图**：合并（`ce6c816`），关闭积压 103；skill v1.17.2 同步。
- **TASK-39 资源与配置命令行**（`aicross resources [detect|smoke]`、`aicross config show|set-role|set`）：合并（`a4e22e1`）。R0 GLM → 复审 Claude 子代理 + Antigravity → R1 GLM（缺值退出 2、roles.json 损坏拒写、额度先于原生等 9 条；GLM 收尾中断，编排者补验证与报告）。真机核：探测、改角色与设置均正常，配置已备份并逐字节恢复；冒烟 60 秒超时误判记积压 104。
- **TASK-40 窗口其余界面下线**：合并（`f52b42c`）。R0 GLM-5.3-Flash（删 435 个文件、约 8.3 万行；kimi 被用户叫停，半截改动已清）→ 复审 Claude 子代理（高-1 macOS 编译失败等）+ Antigravity（无窗口后 ACP 权限请求空等）→ R1 GLM-5.3-Flash（12 条）。合并后验证：cargo 410 / vitest 25，31 个金标准照常。发布版真机：首页、点入查看页、关回首页（用户点）、`open` 冷热启动、采纳写图、`propose` 报已删除、真派一次均通过；macOS 编译按代码核对，本机未能编译。
- **TASK-41 智谱额度上限认作额度用尽**（积压 102）：合并（`11029ea`）。Claude 子代理实现，编排者把「已达…上限」收紧为「使用 / 用量 / 调用上限」并补反例。
- 仓库清理：删除已合并的本地分支 49 个与已合并的远端分支。
- **TASK-45 ACP 打开时当场安装**：合并（`cf0af0a`）。Claude 子代理实现。
- **TASK-42 引擎被杀不留孤儿进程、冒烟超时不判验证失败**（积压 97、104）：合并（`f18b376`）。GLM-5.3-Flash 写完代码与测试后被会话结束打断，编排者补文档、修一处测试编译错误、补验证与报告。合并后 cargo 427 全绿。真机核：派发中强杀后台引擎，`pi.exe` 2 秒内随之结束。
- **TASK-43 盲验收材料**（积压 98）：合并（`970822f`）。GLM-5.3-Flash 实现 → 复审 Claude 子代理 + Antigravity → 编排者修正 `ACCEPT.md` 会写进共享模式项目目录、出方案节点也收到等问题并补链路测试。真机核：验收盲目录有 `ACCEPT.md`、改后的 `project/`、`REQUEST.md` 头部提交号，验收逐条判断给 PASS；盲审验证节点在未配置验证命令时如实答「无法判定」。
- **TASK-44 `aicross recover`**：合并（`90ad3ad`）。R0 GLM-5.3-Flash → 复审 Claude 子代理 + Antigravity 发现任务书前提不成立（派发前快照在执行结束后才落盘，中途被杀时无从回滚）→ `fix-44-r1.md`：单写者拉起 CLI 前写 `<node>.inflight/` 执行中快照，核对分「执行中断 / 执行已结束，结果未写回 / 未发出」，另修跨 run 误伤、回滚失败误报等 8 条 → R1 GLM。真机核：执行者改了文件后强杀引擎，`recover` 列出改动，`--rollback` 恢复原文、删新增，`git status` 干净，无残留进程。
- **skill v1.18.0**（xiaoyu-skill `622c6f8`）：`channels.md` 补 `--accept-file` 与 `aicross recover`（引擎中断后先核对、用户同意再回滚）。

**10-03 已完成**：
- **运行材料统一目录**（aicross `a18bc4a`）：数据根目录取 `AICROSS_HOME`（本机 `D:\aicross-data`），未设置时用 `~/.aicross`；run 摘要保留 30 天；测试临时文件集中到一个目录。
- **skill v1.18.1 / v1.18.2**（xiaoyu-skill `d19c13c`、`7791cd7`、`71aecb6`）：盲审与派发的临时文件共用一个目录；`AICROSS_HOME` 未设置时退回 `~/.aicross`。

**下一步**：
1. 「顺序」第 5 条「其余模式按需接入无窗口路径」按需再排。
2. 积压低危 20 条，按需挑。
3. 已知边界（记在 REPORT-42/43/44）：首次冒烟超时后真身核对推迟到手动冒烟；worktree 创建失败退回单写者时没有执行中快照；盲审验证节点在未配置 `validationCommand` 时无法判定测试是否通过。

**积压**：`aicross/tasks/fix-04-r5-backlog.md` 未修 20 条，无中危。

**Later**：
- herdr 的状态判定（hook 优先、屏幕快照兜底）与 `agent wait`「真卡住才返回」语义，供状态行参考（竞品文件 09-27 节）。
- 各家原生自动压缩阈值对照表（Ekko `2026-09-21-coding-agent-context-policy.md`）。

## 目录

| 位置 | 性质 |
|---|---|
| `D:\work\aicross\skills\ai-cross` | skill 源仓（10-25 从 xiaoyu-skill 移入，带历史） |
| `D:\work\aicross` | aicross 代码与产品文档 |
| `dist/ai-cross/` | v1.16.0 旧副本，只为保留基准材料 `.maintainer/` |
| `bench/`、`eval-*/`、`k3-eval/` | 实验记录，只读参考 |
| `articles/` | 文章 |
| `archive/` | 只读归档：旧进度流水、07-08 执行队列与研究笔记、竞品页面快照、跨宿主验证记录（`codex-e2e/`、`qoder-e2e/`）、历史现场（`test-run*/`、`e2e-run/`） |
| `D:\aicross-data` | aicross 运行材料（`AICROSS_HOME`） |
