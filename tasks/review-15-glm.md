# TASK-15 审查报告

**总判：可交付**（核心行为、名称统一、网格、pi 回退、回归面均与任务书和报告一致；未发现阻断性缺陷，以下为未闭合项与观察）。

说明：本环境没有 shell 工具，我无法跑 `git diff` 与测试命令，以上是对当前工作区文件的通读核对；报告中的验证输出（cargo/vitest/build）未独立复跑。

## 按严重度排序

1. **中 — 自动直启同样命中冷启动的首个 tab，PRODUCT 244 的「一键行」在有默认时不可达**
   - 位置:`src-ui/src/store/app-state.tsx:785`（初始 state 就是一个空 graph 会话）+ `src-ui/src/components/center/CenterPanel.tsx:1235-1245`。
   - 问题：任务书与 PRODUCT.md:247 写的是「**新开**总指挥 tab 时……直接起」。实现里自动启动效果只要求「活跃会话是空 graph tab」，应用一启动、探测一完成、默认一读入，首个 tab 就无条件 spawn 一个 CLI 进程，用户零点击。同时 PRODUCT.md:244 描述的首屏「启动总指挥 · claude · fable · 目录名」一键行，在有效默认下永远看不到（启动器只剩无默认/未装/未探测三种形态）。
   - 复现：有效 `~/.aicross/commander.json` + 默认 CLI 已装 → 打开应用，等探测返回，首 tab 直接是原生会话。
   - 现状：`docs/MANUAL.md:46-47` 已按「新空白 tab 直接启动」写，说明是有意为之；但与「新开」字面和 244 行的取舍值得向用户确认一次，PRODUCT.md 的 244 行描述没有同步更新。
   - 建议：确认意图后在 PRODUCT.md 244/247 行二选一改口径；若要保留首屏一键行，给自动启动加「仅用户本次会话新开的 tab」条件。

2. **低 — 两端显示名表没有跨端一致性断言，「改一端不改另一端」不会红**
   - 位置:`src/resources.rs:47-50`（Rust 常量）与 `src-ui/src/resources/agent-display.ts:5-10`（前端表）是两张独立表；`src/resources.rs:3449-3462` 只 pin Rust 字面量，`src-ui/src/commander/__tests__/commander-10.test.tsx:126-135` 只 pin TS 字面量。
   - 复现：把 Rust 的 `AGENT_NAME_PI` 改成别的值并同时改 Rust 测试字面量 → 全部测试仍绿，前端与后端显示名分叉。
   - 建议（后续即可）：加一条跨端检查，如 Rust 测试读 `agent-display.ts` 源码断言四名一致，或用构建期生成共享表。

3. **低 — `commanderAutoStart` 是纯别名，对应测试与既有用例重复；一处死条件**
   - 位置:`src-ui/src/commander/launcher.ts:203-210`（函数体就是 `return commanderDefaultAvailable(...)`）；`commander-10.test.tsx:137-144` 与 `:324-329` 断言的是同一实现，不构成独立钉住。`CenterPanel.tsx:1238` 的 `|| commanderDefaults === null` 是死条件（`commanderAutoStart` 对 null 已返回 false）。
   - 复现：无行为影响；属同义反复风险与冗余。
   - 建议：保留别名当意图命名可以，但测试注释点明「与 commanderDefaultAvailable 同一实现」；删掉 1238 行的死条件。

4. **低 — `src/resources.rs:940` 仍有一处裸 `"Kimi Code"` 字面量**
   - 位置：kimi 模型探测路径 `detected_model(..., "Kimi Code", ...)`，未引用 `AGENT_NAME_KIMI`（值相同，显示无差，违背「一处定义」的字面要求）。
   - 复现：改 `AGENT_NAME_KIMI` 常量，此处不变。
   - 建议：改为 `AGENT_NAME_KIMI`。（其余出现处均为测试夹具，有意保留。）

5. **低 — G1 路由卡 / 结果卡及派发侧多处仍直出机器 id（本期范围外，但审查清单点到）**
   - 位置（均为未改动的既有代码):`src-ui/src/components/center/graph/DispatchOverlay.tsx:638,643,672,721,754,844,868`（`{agent}/{model}` 直出，如 `pi/glm-4.6`）、`NodePanel.tsx:123,131,273`、`OrchestrationPanel.tsx:280`、`PresetBuilder.tsx:35`、`SettingsModal.tsx:577,749`、`ResourcesOverlay.tsx:371,378`。
   - 说明:`modelOptionLabel`（`model-picker.ts:48-50`）保留机器 id 是**有测试有意钉住的**（`model-picker.test.ts:38-59` 期望 `pi/glm-4.6`），派发坐标本身是机器 id。任务书列的作用域（启动器/左栏/接入页/工具设置 + 报告补的启动卡/tab/模型分组）都已取表；G1 卡片未取表且报告未声称。
   - 建议：作为后续决定——要么明确「派发坐标行保留机器 id」写进 MANUAL 术语表，要么统一走显示名。

6. **信息 — pi 在产品里有两套图形**
   - `AgentBadge`/启动器/资源页按本期决策用字母徽标 `pi`（`brand-marks.ts:81-86`）；但 tab 图标（`CenterPanel.tsx:1403` 的 `SvgPi`）和会话记录（`HistoryBoard.tsx:30,70-78`，指向仓库自带的 `src-ui/src/icons-inline/pi.svg` 几何 Pi 图形）仍是另一套，且该 SVG 来源不在 NOTICE。不违反本期决策（决策只管品牌标志槽位），仅记录不一致。

7. **信息 — `src-ui/src/styles/global.css` 在本期改动清单里，但通读后未找到与本期相关的选择器**（该文件中无 commander / agent-badge / resources 相关规则）。无 shell 无法 diff，此项未能核对到具体改动行。

## 核过没问题的要点

- **自动直启判定与时机**：`commanderAutoStart` 要求默认有效 + 探测完成 + 默认 CLI 已装（`launcher.ts:192-210`）；探测前/默认未读入前不触发；`autoLaunchedCommanderTabs` ref 在 launch 前入集合，异步状态刷新、effect 每渲染重跑（`launchCommander` 未 memo）都不会二次建 PTY。启动自检与直启用同一套校验（identifier 白名单），不存在「直启绕过校验」路径。
- **「更改」后不被默认再起**：`markCommanderLauncherOpened` 先同步入集合再返回 graph 补丁（`launcher.ts:217-224`；`CenterPanel.tsx:1715-1722`）；`SET_TERMINAL_TOOL` 无条件清 `commanderTitle/commanderSettings/extraArgs/resumeToken/toolTitle`（`app-state.tsx:481`），细行与旧标题不留残影，tab 落到空白启动器。
- **「+」→ 对话图**:`handleAddTab('graph')` 在同一批次里 `setGraphViewTabs(id,true)`（`CenterPanel.tsx:935`），自动启动效果因 `graphViewTabs[id]` 早退（`:1236`）。
- **旧 tab 重启**:`RESTART_TERMINAL` 换 id、保 tool/extraArgs/commanderTitle（`app-state.tsx:495-502`），非空 graph 不触发直启，重启沿用原 `--model` 参数。
- **Explorer「在此启动总指挥」**：事件 → `launchCommander(..., true)` 开新 tab（`CenterPanel.tsx:1286-1304`），按钮门控与直启同一判定函数（`Explorer.tsx:595-599`）；外部 `launch --tool` 请求与直启即使撞同一空闲 tab，最终也只落一个 PTY。
- **会话细行**：内容 = 显示名 · 首次模型 · 目录名（`commanderTabTitle`，走显示名表），只在总指挥启动的 PTY tab 上渲染，符合任务书（`agent · 模型 · 目录 · 更改`，agent 用统一显示名）。
- **显示名出现处**：启动器（`launcher.ts:26-28`）、左栏（`ResourcesTab.tsx:151`）、接入页（`ResourcesOverlay.tsx:272,443`）、模型分组（`model-picker.ts:29`）、启动卡与工具设置（`AGENT_CATALOG` → `CenterPanel.tsx:600` → `ToolConfigModal` label）、tab 标题（`commanderTabTitle` + `CenterPanel.tsx:1387-1406`）都取表；`kimicode` 有别名映射；UI 源码无「Kimi Code CLI」残留（仅 `resources.rs:3484` 测试夹具有意保留）、无 `π` 残留。
- **Rust 侧**：四个 `ToolDescriptor.display_name` 引用 `resources.rs` 常量（`claude.rs:7`、`codex.rs:7`、`kimicode.rs:35`、`pi.rs:22`）；`ensure_agents` 每次加载回写 canonical 名（`resources.rs:641-643`），旧 `resources.json` 的「Kimi Code CLI」/「pi」即被迁移；机器 id 全部未动；`task15_builtin_agent_display_names_are_canonical`（`:3449-3497`）钉住表、迁移与工具注册表同源。
- **卡片网格**:`repeat(auto-fit, minmax(150px,150px))` 定宽、48px 定高、`is-picked` 只变边框色（`CenterPanel.css:349-376`），选中不变宽；CSS 断言真实读取 `CenterPanel.css` 并断言两个 CSS 变量的消费与组件输出的 style 变量（`commander-10.test.tsx:424-440`），能红。
- **pi 标志**:`BRAND_MARKS` 无 pi 条目 → 字母 `pi` 回退（两字母、`--agent-tone` 主题色，`resources.css:64-85`），无远程资产、NOTICE 未动——实现与报告声明一致；报告对安装包的核对结论（无官方 SVG）我无法复核，但实现不依赖该结论成立。
- **回归**：`graphTabTitle`/`emptyGraphTabTitle` 的 TASK-10 R1 语义有测试钉住（`commander-10.test.tsx:278-287`）；`commander.json` 读写（`orchestration.rs`）本期未改；TASK-10 R1 消费快照逻辑（`commanderStartedCallback` 只在待保存时注册回调 + `CONSUME_COMMANDER_SETTINGS` 清除）未动且有测试（`:331-343`）。
- **文案**：新增字符串（「更改」「探测中…」「本机没探测到 …」「资源库里还没有 …的模型」等）为中性说明体、全角标点、无 emoji，按钮为 2–6 字动词短语；MANUAL 术语表补了「内置 agent 显示名」行（`docs/MANUAL.md:187`），符合 AGENTS.md 对术语表维护的要求。