复审完成。说明两点前提：我无 shell（无法跑 `git diff` / 重跑测试，验证输出只能采信报告）；审查过程中工作区有一次新增改动（`commanderLauncherHidden` 及其测试、CenterPanel 门控改写），我以下全部结论以**改动后的当前盘上状态**为准。

# 总判

**可交付但有未闭合项** —— 10 条修正本身逐条核对均已落实且相互一致，未发现新引入的行为回归；未闭合项不在代码：最后一笔改动（启动器隐藏门控抽成 `commanderLauncherHidden`）发生在 REPORT-15-R1.md 定稿之后，报告未覆盖（含测试总数），且该改动后的全量验证无证据。

# 10 条逐条

**1. 自动直启 cwd 工作区优先、不反向 SET_FOLDER —— 已修**
- `commanderNewTabLaunch` 返回 `cwd: folderPath || settings.cwd`（`src-ui/src/commander/launcher.ts:241`），两个调用点都用它：`handleAddTab`（`CenterPanel.tsx:1225`）与自动启动 effect（`CenterPanel.tsx:1246`，用 `activeSession.folderPath`）。
- 「+ → 总指挥」有默认时走 `launchCommander(launch, true)` → 直接 `ADD_TERMINAL`，**不经 `selectTool`，全程无 `SET_FOLDER`**（`CenterPanel.tsx:1202-1216`），`cc-folder` / recent folders 不会被存量默认污染（`app-state.tsx:387-394` 的 SET_FOLDER 只在冷启动无工作区、以解析出的 cwd 调用时触达）。
- 测试 `commander-10.test.tsx:136-144`（A 优先于 B、null 时用 B、未装/未探测返回 null）；文档 `docs/MANUAL.md:46` 写明同序。

**2. 启动器登记 + 晚到默认不接管 + 建 tab 处判定 —— 已修**
- 一次性状态机 `commanderTabTransition`（`launcher.ts:256-268`）：`availability` 仅在 tab 未登记时可启动一次；`launcher-shown` / `changed` 登记后不再被接管。
- 启动器挂载即登记：`CommanderLauncher.tsx:131` 的 `useLayoutEffect(onShown)`，由 `CenterPanel.tsx:1707-1713` 写入状态机。
- 关键配套：门控 `commanderLauncherHidden`（`launcher.ts:206-216`，消费于 `CenterPanel.tsx:1699`）——「默认有效且 tab 未登记」时不画启动器，等 availability effect 直启；因此不会先画一帧启动器再替换，也扫掉了「探测返回瞬间顶掉启动器」的路径（DS 复现 B）；「另一 tab 写了默认」场景下已登记 tab 恒为 `launcher` 状态，availability 不再触发（DS 复现 A）。
- 「+ → 总指挥」建 tab 处判定：`CenterPanel.tsx:1222-1231`，有默认直接建带工具 PTY tab，启动器不挂载，`listDispatchableModels` 不会白跑（`CommanderLauncher.tsx:141-161` 仅挂载时请求）。
- 测试：`commander-10.test.tsx:147-168` 三条 reducer 路径、`:489-503` 门控两例（含「已登记则画」）。

**3. 细行目录段随 folderPath；9. pi 带 provider —— 均已修**
- 细行与 tab 标题渲染时都从 `commanderProfile + t.folderPath` 现算：`CenterPanel.tsx:1733-1737`、`:1499-1507`（优先级 `toolTitle > 现算标题 > cwd 名` 不变）；`RESTART_TERMINAL` 保 profile、换 id（`app-state.tsx:497-504`），换目录后标题即显新目录。
- `commanderTabTitle` 收 provider，pi 输出 `provider / model`（`launcher.ts:112-127`）；默认行同口径（`:100-110`）。
- 测试：reducer 级 `app-state.test.ts:119-140`（SET_FOLDER + RESTART 保快照、目录读当前 tab）；`commander-10.test.tsx:284-288`（pi 带 provider）。

**4. G1 显示名只改渲染层 —— 已修**
- 新增 `agentModelDisplay`（`agent-display.ts:24-28`），仅用于渲染坐标：`DispatchOverlay.tsx:649/654/683/688/732/765/855/879/899` 及 `displayRouteLine` 包装（`:74-82`，用于 `:562/579/613/750`）、`NodePanel.tsx:124/131/273/280`、`OrchestrationPanel.tsx:281`、`PresetBuilder.tsx:36`。
- 数据契约未动：`dispatch.ts:352-353` 的 `route = ${target.agent}/${target.model}` 原样；裁决上下文仍是机器 id（`DispatchOverlay.tsx:404` handoff 摘要）；目标下拉的 option **value** 仍是机器 JSON 三元组、只有文案转显示名（`:636/:649/:653-654`）。
- 测试 `agent-display-r1.test.ts:23-33` 钉住 `routeLine` 仍含 `pi/glm` + 显示名转换。

**5. 核心接线测试 —— 已修（按清单口径；有一个残留缺口）**
- 状态机纯函数三条路径有测试（见第 2 条）；`commanderLauncherToolPatch`、`commanderAutoStart`、`markCommanderLauncherOpened` 全部删除（全仓 grep 仅剩测试里的否定断言，`commander-10.test.tsx:485`）。
- 残留缺口：`commander-10.test.tsx:477-486` 的「CenterPanel 真实接入」是**源码字符串断言**（含四个接线标记、不含旧函数名），不是行为测试；effect 内部的逻辑错误（例如 `available` 传错值）不会有测试变红。纯逻辑三层（状态机 / 门控 / 纯函数）已各自钉住，这属于清单要求之外的超额缺口，非未完成项。

**6. 两端同源断言 —— 已修**
- `commander-10.test.tsx:466-475` 用 `node:fs` 读 `src/resources.rs`，正则抽出四个 `AGENT_NAME_*` 与 `AGENT_DISPLAY_NAMES` 全等比较；正则与实际常量书写形式匹配（`resources.rs:47-50`）。任一侧单方改名/改值都会红。

**7. 图标统一 —— 已修（一处已文档化的偏差）**
- `brand-marks.ts` 重写（:1-43）：simple-icons 路径、`BRAND_MARKS`、`glyph` 分支全部不存在（grep 全仓仅测试否定断言）；`AgentBadge` 只剩 `image / inline-svg / letters` 三态（`AgentBadge.tsx:8-36`）。
- 五个面同源接入 AgentBadge：启动器卡（`CommanderLauncher.tsx:273`）、左栏资源（`ResourcesTab.tsx:150`）、接入页（`ResourcesOverlay.tsx:442`）、启动卡（`CenterPanel.tsx:576-588`）、历史行（`HistoryBoard.tsx:64-66`）。
- pi 用 `icons-inline/pi.svg`（`brand-marks.ts:6,19`），与 tab 图标 `SvgPi` 同一文件（`CenterPanel.tsx:152/271`）——顺带把 GLM 上轮「pi 两套图形」的观察也合掉了。
- 偏差：清单点名的 `public/icons/tools/kimicode.svg` 仓库里不存在（实际目录只有 claude/codex/mimocode/qwen/antigravity.svg）；Kimi 改用已有 `icons-inline/kimicode.png`，REPORT「没做与边界」有说明，符合「只用仓库已有文件」约束。
- NOTICE：aicross 自有段改为「reused from the forked Coffee CLI codebase」并列出四个文件（`NOTICE:26-34`），上游 NOTICE 原文保留。
- 测试：`resources-tab-09.test.tsx:382-404`（真实渲染断言四家无 letters/glyph、未知退字母）、`resources-overlay-09b.test.tsx:62-66`、`agent-display-r1.test.ts:35-50`。

**8. 网格测试与文档 —— 已修**
- CSS 断言真实读 `CenterPanel.css`：grid 消费变量、卡片高度变量、`.is-picked` 只改 `border-color` 且无 `width/padding/border-width`（`commander-10.test.tsx:454-458`；CSS 事实在 `CenterPanel.css:349-353/359/374-376`，断言有实义）。
- 文档：`PRODUCT.md:234`（图标决定）、`:243/:247`（四个显示名）、`:244`（冷启动直启）；`README.md:3`、`AGENTS.md:7`；`docs/proto/index.html:2` 文件头「历史快照」。测试 `agent-display-r1.test.ts:52-60` 逐项钉住。

**9. 小项 —— 已修**
- pi provider（见第 3 条）；effect 死条件已不存在（现 effect `CenterPanel.tsx:1244-1261` 无 `commanderDefaults === null` 冗余判断，且先登记再启动、登记前判定与 `launchCommander` 的早退条件同一套校验，不存在「记了集合却没启动」的死端）；glyph 类型/分支/选择器删除；冷启动直启写入 `MANUAL.md:46`。

**10. Kimi 常量、冷启动、global.css —— 已修**
- `resources.rs:940` 改引 `AGENT_NAME_KIMI`（测试 `agent-display-r1.test.ts:62-64` 用正则钉住该调用点）；其余裸显名只在 `#[cfg(test)]` 夹具（旧库迁移用例有意保留）。
- 冷启动直启写入手册/PRODUCT（见第 8 条）；`global.css` 不在本轮改动文件列表（git status 快照无该文件），与 REPORT 声明一致。

# 新发现的问题（按严重度）

1.〔中〕**REPORT-15-R1.md 与最终代码脱节**：`commanderLauncherHidden`（`launcher.ts:206-216`）、其测试（`commander-10.test.tsx:489-503`）和 CenterPanel 门控改写（`:1699`）是报告定稿后新进盘的改动，报告通篇未提（grep 无命中），报告里的 vitest 总数（482）与「测试先红」描述对应的是旧形态。复现：对照 `tasks/REPORT-15-R1.md` 与现文件即可见。建议：commit 前补报告（含门控的设计取舍，见下条），并对最终状态重跑一次全量验证——fix 清单要求报告逐条对应，且我无法替验最终状态的 cargo/vitest/build。

2.〔低〕**有存量默认时探测期间启动器区是空白**：`commanderLauncherHidden` 对「已存默认 agent 但探测未回」返回 true → 渲染 `null`（`launcher.ts:214` + `CenterPanel.tsx:1699`）；`commanderDefaults === null`（默认文件未读回）同理。这是「可能直启时不画」的有意取舍（无默认时启动器立即可见并显示「探测中…」，`launcher.ts:213-214` 注释写明了），但「有默认的首屏短暂空白」这一状态没有任何手册/报告记载，探测慢时用户看到的是白板。建议：报告里记一句即可，行为可接受。

3.〔低〕**`onShown` 每次渲染重复登记**：`CommanderLauncher.tsx:131` 依赖 `[onShown]`，而 CenterPanel 每次渲染传新闭包 → 启动器存续期间每次渲染都重跑 transition。幂等（状态停留 `launcher`），无功能影响，仅与「首次进入布局时登记」的意图表达不符；回调可用稳定 ref 或空依赖。

4.〔信息〕**一键行仍是「默认目录优先」的例外**：启动器里的一键行（`CommanderLauncher.tsx:229-244`）点击后以 `defaults.cwd` 启动并经 `selectTool → SET_FOLDER` 把工作区/cc-folder 切到默认目录。按钮文案本身就写明该目录，属 TASK-10 既有显式行为，不在第 1 条「自动直启」的修正范围；记录在案以免被当成漏改。

5.〔信息〕**范围外的机器 id 渲染残留**：接入页模型分组标签 `ResourcesOverlay.tsx:378`、`SettingsModal` 相关处仍直出机器 id——与上轮 GLM #5 的「后续决定」一致，本清单未要求，非回归。

# 核过没问题的点

- 状态机与门控互洽：启动器可见 ⇔ 「不可直启或已登记」，不存在「可见的启动器被 availability 抢走」的窗口；子组件 `useLayoutEffect` 登记先于父 effect 判定。
- 「更改」回路：登记 `changed` → `SET_TERMINAL_TOOL{graph,''}` 清 profile/title/settings（`app-state.tsx:480-484`）→ 回启动器且不再被默认接管（`CenterPanel.tsx:1741-1755`）。
- 旧 tab 重启：`RESTART_TERMINAL` 保 tool/extraArgs/profile，换 id 不触发直启（非空 graph 早退）。
- 外部 `launch --tool` 复用空闲 tab：同批次 dispatch，无与直启撞车的中间态。
- tab 逻辑回归：`emptyGraphTabTitle` / `graphTabTitle` 语义与 `case 'graph'` 未变，测试钉住（`commander-10.test.tsx:298-307`）；标题优先级保持 TASK-10 口径。
- commander.json Rust 读写未动（`orchestration.rs` 不在本轮改动列表）；消费快照逻辑未动且有两层测试（`commander-10.test.tsx:352-363`、`app-state.test.ts:89-117`）。
- 跨端显示名断言、CSS 断言、图标断言、文档断言都是「读真实文件/真实渲染」的断言，具备变红能力；正则与实际文件形式逐一比对过。
- 显示名机器 id（claude/codex/kimi/kimicode/pi）全链路未改；Rust 常量一处定义、四个 ToolDescriptor 引用、旧库迁移测试夹具保留。