## 总判

**可交付但有未闭合项**——10 条的功能面都落地了（第 1/2/3/4/6/7/8/9/10 条实现可核，无反向 SET_FOLDER、无接管、图标与显示名同源、跨端比对真读 Rust 文件）；未闭项是第 5 条的「接线」断言只是源码文本比对，以及本轮 gate 新带来的一处首屏空白 / 新增死分支。以下均为静态核对（环境无 shell，未能跑 git diff 与 cargo/vitest/build）。

---

## 逐条核对

| # | 结论 | 依据 |
|---|---|---|
| 1 自动直启 cwd 顺序 | **已修**（测试只覆盖纯函数） | `commander/launcher.ts:215-228` `cwd: folderPath || settings.cwd`；`CenterPanel.tsx:1226-1231`「+→总指挥」走 `launchCommander(launch,true)` → `ADD_TERMINAL`（`app-state.tsx:416-428` 新 tab 取 cwd、activeId 设为新 tab），不经过 `selectTool` 的 `SET_FOLDER`；自动直启路径 `CenterPanel.tsx:1247` 传 `activeSession.folderPath`，`selectTool`（`:1157-1176`）里 `SET_FOLDER` 的值就是本 tab 自己的 folderPath → 旧实现的「默认目录反向切走工作区」已不存在。测试 `commander-10.test.tsx:136-144` 只断言 `cwd === 'D:\workspace-a'`，**没有**「左栏仍是 A」的断言（全仓无 CenterPanel 渲染测试）。 |
| 2 启动器登记 / + 建 tab 处判定 | **已修** | 状态机 `launcher.ts:230-253`；`CenterPanel.tsx:1711-1717` 经 `onShown` 登记 `launcher`，`CommanderLauncher.tsx:131` 是 `useLayoutEffect`（先于父级 passive effect，同一 commit 内启动器先登记）；`CenterPanel.tsx:1244-1254` `availability` 只在 `state[id] === undefined` 时启动；`:1746-1750` 登记 `changed`；「+→总指挥」在 `:1226-1231` 直接建 PTY tab → 不画空 graph、不挂启动器、不发 `listDispatchableModels`。测试 `:146-168` 三条路径齐。 |
| 3 细行目录段随 folderPath | **已修** | `app-state.tsx:158-159` 新增 `commanderProfile`，`:481-484` 写入、`:497-503` `RESTART_TERMINAL` 只换 id/清 toolTitle（profile 与 folderPath 保留）；`CenterPanel.tsx:1737-1741` 细行 = `commanderTabTitle(profile…, t.folderPath)`、`:1500-1508` tab 标题同源；测试 `__tests__/app-state.test.ts:119-142`（SET_FOLDER→RESTART→profile 在、folderPath 新）。 |
| 4 只改渲染层显示名 | **已修** | `DispatchOverlay.tsx:74-82` 新增 `displayRouteLine`（对 `routeLine` 结果做串替换），用于 `:562/579/613/750`；`:405-407` 的 `handoff()` 裁决上下文仍写 `${agent}/${model}`；`graph/dispatch.ts:341-359` 未动，`dispatch.test.ts:428-434`、`presets-05b.test.ts:215-224` 仍钉机器 id；`NodePanel.tsx:124/131/273/280`、`OrchestrationPanel.tsx:281`、`PresetBuilder.tsx:36` 走 `agentModelDisplay`；测试 `agent-display-r1.test.ts:22-31`（新实现前无 `agentModelDisplay(` → 会红）。 |
| 5 核心接线测试 | **已修（接线断言弱）** | 三条状态机用例 `commander-10.test.tsx:146/156/163`；`commanderAutoStart` / `commanderLauncherToolPatch` / `markCommanderLauncherOpened` 全仓只剩 `commander-10.test.tsx:484` 的 `not.toContain`；红绿描述自洽（该文件恰好 37 个 `it`，与报告「6 failed \| 31 passed (37)」吻合，失败点正是 6 条 R1 用例/改签名的 `:284`）。缺口：`TASK-15-R1：CenterPanel 真实接入`（`:476-486`）只 grep CenterPanel 源码含 4 个字面串，`available: true`、`tabId` 传错、不调 `launchCommander` 都不会红。 |
| 6 两端同源 | **已修** | `commander-10.test.tsx:465-474` 用 `node:fs` 读 `../../../../src/resources.rs`，正则抽四个 `AGENT_NAME_*` 与 `AGENT_DISPLAY_NAMES` `toEqual`；单边改名即红。 |
| 7 四家图标换 coffee 那套 | **已修（资产按仓库实际调整，报告已声明）** | `brand-marks.ts:1-41`：无 `BRAND_MARKS`、无 `glyph`；claude/codex=`/icons/tools/*.svg`、kimi/kimicode=`icons-inline/kimicode.png`、pi=`icons-inline/pi.svg`（内联）；五面接入 `CommanderLauncher.tsx:272`、`ResourcesTab.tsx:150`、`ResourcesOverlay.tsx:442`、`CenterPanel.tsx:577-589`、`HistoryBoard.tsx:65`；`NOTICE` 新增段只列这四个文件，`UPSTREAM NOTICE` 段原文未动；测试 `resources-tab-09.test.tsx:382-412`、`agent-display-r1.test.ts:43-51`。偏差：`src-ui/public/icons/tools/` 里**没有** `kimicode.svg`/`pi.svg`（`ls` 只有 antigravity/claude/codex/mimocode/qwen），实现改用仓库已有的 `icons-inline/kimicode.png` + `icons-inline/pi.svg`，`REPORT-15-R1`「没做与边界」已写明。 |
| 8 网格测试与文档 | **已修** | `commander-10.test.tsx:444-463` 新增 `.is-picked` 规则只含 `border-color`、不含 width/padding/border-width（`CenterPanel.css:374-376` 事实相符）；`PRODUCT.md:233/243/244`、`README.md:3`、`AGENTS.md:7`、`docs/proto/index.html:2`（历史快照）已同步；`agent-display-r1.test.ts:53-60` 钉住这些文本。残留（清单外）：用例名「窄窗口可换行」仍大于断言。 |
| 9 pi 带 provider / 删死分支 | **已修（新引入一处死分支）** | `launcher.ts:110-121` `commanderTabTitle` 收 provider，pi 输出 `provider / model`；`CenterPanel.tsx:1198/1500-1507/1739-1741` 都传；`glyph` 类型/渲染/CSS 选择器已删（`brand-marks.ts:33-41`、`AgentBadge.tsx:9-38`、`resources.css:15-96` 无 glyph）；旧 effect 里的 `commanderDefaults === null` 死条件随旧实现消失（`CenterPanel.tsx:1245-1254`）；`MANUAL.md:46` 写入冷启动直启。注意 `CommanderLauncher.tsx:246-247` 的「探测中…」在 CenterPanel 里已不可达（见新发现 1）。 |
| 10 GLM 补充 | **已修** | `src/resources.rs:940` 改用 `AGENT_NAME_KIMI`（定义 `:49`），全 `src/` 里四个显示名的非测试字面量只剩常量定义（其余命中都是测试夹具，如 `:3484`）；冷启动直启写进 `MANUAL.md:46`、`PRODUCT.md:244` 与报告；启动器「更改」回路保留（`CenterPanel.tsx:1742-1758`）；`global.css` 内容中无 commander/agent-badge/simple-icons 规则（仅字体注释里出现 glyph 一词），与「本轮不必改」一致——字节级 diff 无 shell 无法核。 |

---

## 新发现的问题

**1〔中〕冷启动首屏空白 + 「探测中…」变成不可达分支（本轮 gate 引入）**
- 位置：`CenterPanel.tsx:1699-1703`（`commanderDefaults === null || !toolsScanned || (commanderDefaultAvailable(...) && state[id] === undefined) ? null : <CommanderLauncher/>`）、`CommanderLauncher.tsx:246-247`、`docs/MANUAL.md:47`。
- 复现：没有 `~/.aicross/commander.json`（或默认 agent 未装）时冷启动 → gate 的 `!toolsScanned` 分支先命中，整屏空白，直到 `checkToolsInstalled()`（`CenterPanel.tsx:833-858`：300ms 防抖 + `server.rs:133-142` 对 15 个工具各起一次 `where`，文件注释自称每项 200ms–1s）回来才画出启动器；期间看不到原设计的「探测中…」，也不能选 agent。同时 `CommanderLauncher.tsx:246-247` 这个分支在 CenterPanel 路径下永远不可达，与第 9 条「删无生产者分支」的口径相反；`MANUAL.md:47`「…或探测未完成时才显示总指挥启动器」与新行为不符。
- 建议：gate 收窄为「只有可能直启时才不画启动器」（默认已读入且无可用默认时直接画启动器，探测期显示「探测中…」），或给启动器一个「画壳但不登记 launcher」的占位态；顺带改 `MANUAL.md:47` 措辞。

**2〔低〕第 5 条的「真实接入」断言是源码文本比对**
- 位置：`commander-10.test.tsx:476-486`。
- 复现：把 `CenterPanel.tsx:1249-1251` 改成 `available: true`（或 `tabId: 'x'`），或删掉 `:1254` 的 `launchCommander(launch)` → 三个字面串仍在，测试全绿，自动直启已失效。
- 建议：把「effect 判定」抽成纯函数（commanderDefaults/toolsScanned/installed/activeSession/graphViewTabs → launch|null）再单测，或补一次 CenterPanel 渲染断言（mock `tauri` 的 IPC）。

**3〔低〕启动器「一键行」不走「当前工作区优先」**
- 位置：`CommanderLauncher.tsx:229-243`（`cwd: defaults.cwd`）对比 `:186-199`（工作区非空时字段显示 folderPath）。
- 复现：默认 `{claude, cwd=B}`、左栏工作区 A → 会话里点「更改」回到启动器 → 目录字段显示 A，点一键行却在 B 起会话（按钮文案带 B，属口径不一致）。
- 建议：一键行改传 `(folderPath ?? '') !== '' ? folderPath : defaults.cwd`，与 `commanderNewTabLaunch` 同序。

**4〔低〕tab 条上的 claude/codex 仍是内联字面量副本**
- 位置：`CenterPanel.tsx:204-205`（`CLAUDE_SVG`/`CODEX_SVG` 与 `public/icons/tools/*.svg` 字节同源但各存一份）、使用点 `:1396/1404`。
- 复现：只改 `public/icons/tools/claude.svg` → 启动器/左栏/接入页/启动卡/历史行都变，tab 条不变（kimi/pi 的 tab 图标已复用同一资产）。
- 建议：`SvgClaude`/`SvgCodex` 改为读 `brand-marks` 的 asset 或改成 `<img src="/icons/tools/…svg">`。

**5〔低〕显示名渲染仍有两处机器 id（第 4 条名单外）**
- 位置：`SettingsModal.tsx:577/749`、`ResourcesOverlay.tsx:371/378`（`{agent}/{provider}/{id}`）。
- 说明：`resources-overlay-09b.test.tsx:161` 正钉着这些文案（`1. codex/gpt`），要改需同时改测试；第 4 条点名的四处已完成，这里只是同一类残留。

**6〔信息〕`agent-display-r1.test.ts:35-40` 的「都接入 AgentBadge」用 `toContain('AgentBadge')` 判定**，仅留 import 即可通过（HistoryBoard 现状就是如此），建议断到使用点（如 `<AgentBadge agentId={tool} />`）。

---

## 核过没问题的点

- **一次性语义与 StrictMode**：`launcher.ts:245-248` 对 `current !== undefined` 早退，`launcher.ts:250` 不让 `launcher-shown` 覆盖 `started`；`CenterPanel.tsx:1253-1254` 先写 ref 再启动 → 双跑 effect 只起一次。
- **「更改」回路无残影**：`CenterPanel.tsx:1745-1758` 登记 `changed` + `graphViewTabs=false` + `SET_TERMINAL_TOOL{graph,''}`,`app-state.tsx:481-484` 清 `commanderTitle/profile/settings/toolTitle`；此后该 tab 不再被默认接管。
- **「+→对话图」不受影响**：`CenterPanel.tsx:1236` 置 `graphViewTabs[id]=true`，`:1246` 与 `:1699` 两处都早退，`graphTabTitle`/`emptyGraphTabTitle`（`launcher.ts:172-186`）语义与 TASK-10 R1 一致（`commander-10.test.tsx:298-307`）。
- **消费快照**：`launcher.ts:275-284` 只在有 `commanderSettings` 时注册回调，`TierTerminal.tsx:1298-1300` 只在 `tierTerminalStart` 成功后调用；`app-state.tsx:485-491` 消费清空；`commander-10.test.tsx:351-363`、`app-state.test.ts:89-117` 未动。
- **commander.json**：`orchestration.rs:543-587` 读侧不改文件、`version!=1` 落空默认、非法 provider/model 拒写，均未动。
- **显示名单一事实源**：`resources.rs:47-50` → 四个 `ToolDescriptor` 引用 → `ensure_agents` 迁移旧库，`resources.rs:3449-3497` 钉住；前端 `agent-display.ts:5-10` 一处；`src/` 内非测试字面量已清零。
- **无 simple-icons / π 残留**：全仓 grep 只命中 `tasks/` 文档与 `agent-display-r1.test.ts:55` 的断言字符串；`/icons/tools/{claude,codex}.svg`、`icons-inline/{kimicode.png,pi.svg}` 四文件都在。
- **验证数字自洽**：仓库实际 31 个测试文件与报告一致；`commander-10.test.tsx` 恰好 37 个 `it`，与「6 failed | 31 passed (37)」吻合（说明红绿描述不是编的）。
- **未能独立复核**：`cargo check/test`、`vitest`、`npm run build`、`scripts/copy-audit.py` 的输出；`global.css` / `server.rs` / `terminal.rs` / `tauri.ts`「未修改」属字节级结论，无 shell 无法确认。