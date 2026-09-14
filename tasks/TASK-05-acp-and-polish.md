# TASK-05 ACP 接入、第 2 期预设、打包（后置，内容随前四期实测调整）

先读 `AGENTS.md`、`docs/PRODUCT.md`、`D:\work\agentteam\COMPETITIVE-LANDSCAPE.md` 2026-08-30「程序化续聊」一节。本期三块可拆开派。

## 0. 与对话图和第 3/4 期的衔接（2026-09-14，优先于下文旧说法）

前四期已经落地：中栏是 DAG（`src-ui/src/graph/`，`Graph/Node/Reply/Target`，`contextOf`/`stamp`/`dispatchableNodeIds`），派发引擎按层推进（`src/dispatch.rs`、`src-ui/src/graph/dispatch.ts`，留痕契约 `aicross-dispatch/1`，G1/G3/G4 都在 `DispatchOverlay.tsx`），资源接入与角色默认（`src/resources.rs`、`roles.json`），经济流水线 09-14 已在窗口里三层真跑通含验证失败修正轮。本期三块都长在这套东西上，不加新引擎、不定义新协议：

1. **B 节的每个「第 2 期预设」= 一个 `stamp` 图章 + 一小段结果屏动作**，不是新模式引擎：
   - 擂台 = 从同一父节点盖出 N 个共享的执行 turn 节点（目标各不同，用户在盖章时选 2–4 家）；结果屏每张卡加「选它」= 现有 `adopted` 标记 + 把被选回复作为后续列的末端（`SET_GRAPH_FOCUS`），不新增契约字段。
   - 评审团 = 盖出 ≤3 个盲 turn 节点，每个节点的**视角文本放进该节点的 `user.text`**（安全 / 性能 / 需求覆盖 / 自定义），`serializeContext` 已会把它拼在冻结模板之后；不加 `Target.perspective` 之类字段。
   - 仲裁 = G3「分歧」条目上的按钮：新建一个盲 turn 节点，`parents = [分歧涉及节点的共同材料祖先]`（`visibility=指定上游` 指向根材料），`user.text` = 该分歧的中性描述（由前端按两边正文自动拼「A 认为…；B 认为…」、不带立场词，用户可改），目标由用户选一个**与两边都不同厂商**的模型（validate 拦同厂商）。
   - 前提抽查 = 设置里「前提台账」（`~/.aicross/premises.json`：`{id, text, recorded_at, last_checked_at, last_result?}`，新文件，不动 graph 契约）；「抽查」= 新建一张图：根材料 = 前提原文 + 用户勾选的原始附件，一个盲 turn 节点发给用户选的非项目常用厂商（清单里排除 roles.json 里三个角色的厂商），prompt 用冻结模板 `fixtures/prompts/premise-check.txt`（「看到这批材料你会怎么判断」）；跑完把结论写回台账 `last_result` 并在设置页与原前提并列显示。
   - 主备/熔断 = `Target` 追加可选字段 `fallback?: { agent, provider?, model }`（**本期对图契约唯一的字段追加**，Rust `orchestration.rs::Target` 同步 `#[serde(default)]`）；`run_one` 遇 `error / timeout / identity_mismatch` 且有 fallback → 用备选重跑一次（同 prompt、同可见性），留痕两条、结果屏与节点卡显示「已切备选：主 x → 备 y」；G1 决策行显示备选。
   - 顾问 = 中栏输入 `/advise @agent/model 问题` → 盖一个 `role=顾问`、`visibility=盲`、`write=只读` 的 turn 节点，prompt 模板 `fixtures/prompts/advisor.txt`（只要方案不要执行、不接管），回复卡标「顾问意见」；`ROLES` 里加「顾问」。
2. **A 节 ACP 是通道层的一种运行方式，不碰图**：`src/channels/` 加 `runner = 一次性 | 长驻`，长驻实现 ACP 客户端（JSON-RPC over stdio：`initialize` → `session/new` → `session/prompt`，流式收 `session/update`），进程按 `agent/model` 键复用，`Reply.session_id` 存 ACP 的 session id；异常退出或 `--acp` 不可用 → 回退第 3 期一次性续聊，留痕 `isolation`/`session_id` 照旧、frontmatter 追加 `runner: acp|oneshot`（`aicross-dispatch/1` 允许追加字段，不改名不删）。先接 kimi（`kimi acp`），claude/codex 的适配器只做探测与设置项，不在本期强求真跑。
3. **C 节的任务列表 tab 用右栏现成的「任务」tab**（现在是占位），数据落 `~/.aicross/tasks.json`，与派发无耦合；首启引导用现有起始页（对话图起始视图）加三步提示条；打包走 `tauri build`，图标换 `src-tauri/icons/`（设计稿由实现者出，简洁字标 AC，不要 coffee 的杯子）。
4. **通道与模型口径**（09-14 定案）：pi 为非原生订阅模型默认通道；DeepSeek 官方 `deepseek/deepseek-v4-flash` 的响应真身是 `deepseek-flash`，已在 `channels/mod.rs::KNOWN_MODEL_ROUTES` 放行；真机验收用的模型仍按 TASK-04 第 0 节第 4 条（出方案 pi/zai-coding-cn/glm-5.3-flash、执行 codex/gpt-5.6-luna、验收 kimi/kimi-code/k3-256k）。
5. **分派**：A（ACP）、B（预设）、C（打包与体验）三块各自一个工作树、各自一份 REPORT（`REPORT-05A/B/C.md`），互不等待；改到同一文件（`src/dispatch.rs`、`DispatchOverlay.tsx`）的地方尽量只追加，合并冲突由编排者处理。每块的验收清单见各节末尾；真机请求由编排者在沙箱外跑，实现者写 ignored 测试即可。

## A. ACP 客户端（长驻进程，替代逐轮冷启动）
- 实现 Agent Client Protocol 客户端（JSON-RPC over stdio）：`initialize` → `session/new` → `session/prompt` → 流式接收。
- 先接 **kimi**（原生 `kimi acp`），再接 Claude Code（`claude-agent-acp` 适配器，npm 包，需在设置里指明安装方式）与 Codex（`codex-acp` 或 `acp-adapter`）。
- 通道执行器加一种运行方式「长驻」：同一 `agent/model` 的多轮 @ 复用一个进程；进程异常退出自动回退到 TASK-03 的一次性续聊。
- 用量：ACP 若不回传 usage，标 "—"，不猜。
- 验收：kimi 走 ACP 两轮续聊真跑一次（ignored 测试，编排者跑）；进程被杀后回退路径测试（单测）；`runner` 字段进留痕；ACP 不可用时派发结果与第 3 期完全一致（回归测试）。

## B. 第 2 期预设（都在扇出比对引擎上，不加新引擎）
- **擂台**：同任务多家做，结果屏加「选它」按钮，选中者写进对话流。
- **评审团**：盲审 + 每节点一个视角文本（安全 / 性能 / 需求覆盖 / 自定义），拼进冻结模板之后；≤3 路。
- **仲裁**：结果屏「分歧」条目上加「仲裁」→ 把原始材料 + 该分歧的中性描述（不带各方立场）盲发给用户选的第三家。
- **前提抽查**：设置里维护「前提台账」（一行一条 + 首次记录日期 + 最近抽查日期）；「抽查」= 把该条前提的原始材料零上下文发给一家非项目常用厂商，问"看到这批材料你会怎么判断"；结果并列原前提展示。
- **主备/熔断**：节点可设备选 `agent/model`；主节点失败（error/timeout/identity_mismatch）自动用备选重跑一次，G2 显示"已切备选"。
- **顾问**：中栏 `/advise @model 问题` → 单轮、盲、只要方案不要执行，回复标"顾问意见"。
- 验收：每个预设一条 `stamp` 纯函数测试（节点数、可见性、写权限、parents 正确）+ 一条 G3 动作测试（选它 / 仲裁建节点 / 备选重跑记账）；`Target.fallback` 的 Rust/TS 往返测试；`premises.json` 读写与损坏文件不覆盖测试；一条 ignored 真机测试：擂台两家（pi/glm-5.3-flash 与 codex/gpt-5.6-luna）+ 仲裁第三家（kimi/k3-256k）。

## C. 打包与体验
- `tauri build` 出 Windows 安装包；图标换成正式的；首启引导：打开目录 → 接入资源 → 选预设，三步各一句话。
- 任务列表 tab（默认关）：`{id, title, description, status}`，状态点一下循环推进，可拖动排序；与派发无耦合。
- 前端拆包（chunk 警告）。
- `docs/` 里写用户手册第一版：安装、接入、三个内置预设怎么用、`.dispatch/` 是什么、密钥怎么处理（六铁律的用户版）。说明体，每节 ≤ 10 行。
- 验收：`npm run tauri build` 在本机出 `.msi`/`.exe`（编排者跑）；任务 tab 的 reducer 与持久化测试；首启引导只在没有任何图、没有已接入资源时出现（测试）；chunk 警告消失或单 chunk < 500KB。

## 不做
- 云端、账号、遥测。
- 蜂群。
