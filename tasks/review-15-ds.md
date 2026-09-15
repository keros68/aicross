## 总判

**可交付但有未闭合项** —— 自动直启、显示名统一、卡片网格、pi 回退字母、文档这几件任务书要求的主路径都对（一次性集合确实挡住了探测返回后的二次启动，「更改」也确实不会被默认立刻顶回去）；未闭项集中在：自动直启用**存量默认目录**覆盖当前工作区并顺手切走工作区（会写 `cc-folder`）、默认值晚到会把用户正在用的启动器顶掉、细行目录段在左栏换目录后不更新、G1 路由卡/结果卡仍画机器 id，以及本期核心接线（effect + 更改）没有任何测试钉住。

---

## 按严重度排序

### 1〔高〕自动直启用默认里记的目录，并把工作区切过去

- **位置**：`src-ui/src/components/center/CenterPanel.tsx:1244`（`cwd: commanderDefaults.cwd || activeSession.folderPath || ''`）；副作用在 `CenterPanel.tsx:1170`（`selectTool` 里 `dispatch SET_FOLDER`）→ `store/app-state.tsx:385-392`（同时写 `localStorage['cc-folder']`）。
- **对比**：启动器自己是「当前工作区赢」——`components/center/CommanderLauncher.tsx:180`（`folderPath` 非空就覆盖 cwd）、`:186-192`（仅在 folderPath 为空时才用默认 cwd）；`docs/PRODUCT.md:243` 也写「选工作区目录（默认当前工作区）」。两条入口口径相反。
- **复现**：默认 `{agent: claude, cwd: B}` → 左栏把工作区切到 A（SET_FOLDER(A)，`cc-folder=A`）→ 点「+ → 总指挥」。新 tab 的 `folderPath` 是 A（`app-state.tsx:414-420`），但 effect 用 B 起会话；`selectTool` 再 dispatch SET_FOLDER(B)，于是 Explorer（读 `activeSession.folderPath`，`Explorer.tsx:563-564`）、`cc-folder`、recent folders 全变成 B。
- **建议**：改成 `cwd: activeSession.folderPath || commanderDefaults.cwd || ''`（与启动器同序），或自动直启路径不 `SET_FOLDER`（但那样会话目录与左栏会不一致，不推荐）。

### 2〔中〕默认值晚到时，自动直启会把用户正在操作的启动器顶掉

- **位置**：`CenterPanel.tsx:1235-1246`。集合只在「这个 tab 自动起过」或「点过更改」（`CenterPanel.tsx:1720`）时登记；「该 tab 当时显示启动器是因为默认还不成立」这条路径没有登记。
- **复现 A（确定可复现）**：首启无默认 → 打开一个空白总指挥 tab（启动器）→ 在另一个 tab 用「在此启动总指挥」或启动器起一次（`CenterPanel.tsx:1249-1262` 的 `rememberCommander` → `setCommanderDefaults`）→ 回头看那个空白 tab：`commanderAutoStart` 转真，界面当场被 PTY 替换，启动器里已选的内容丢失。
- **复现 B（任务书句面如此，但结果是同上）**：「探测未完成时才显示启动器」——在启动器里挑 agent/模型的这一秒内探测返回，`toolsScanned` false→true，同一 effect 重跑并把启动器换掉。
- **附带**：effect 在 paint 之后跑，所以每次「+ → 总指挥」都会先画一帧启动器再被替换，并且白跑一次 `listDispatchableModels`（`CommanderLauncher.tsx:139-160`）。
- **建议**：二选一并在任务书/手册写明——要么探测期间不画启动器（判定移到 tab 创建处，`handleAddTab`），要么启动器一渲染就登记该 tab「用户已看到启动器」。至少把「另一个 tab 写了默认」这条排除掉。

### 3〔中〕细行的目录段是启动时的快照，左栏换目录/重启后不更新

- **位置**：`CenterPanel.tsx:1208`（title 由 `commanderTabTitle(..., cwd)` 生成）、`CenterPanel.tsx:1710-1711`（细行直接渲染 `t.commanderTitle`）；而 `Explorer.tsx:703-706` 换目录只做 `SET_FOLDER` + `RESTART_TERMINAL`，两者都不动 `commanderTitle`（`app-state.tsx:385-392`、`:495-502`）。
- **复现**：起一次总指挥会话 → 左栏目录行换到另一个目录（CLI 会在新目录里重启）→ 顶部细行仍写旧目录名；tab 标题也保留旧值，直到 CLI 自己报 OSC 标题（`CenterPanel.tsx:1491` 的优先级）。
- **建议**：细行固定 agent/模型，目录一段改读 `t.folderPath`；或在 `SET_FOLDER`/`RESTART_TERMINAL` 时清掉 `commanderTitle`。

### 4〔中〕G1 路由卡与结果卡仍把机器 id 画到界面

- **位置**：`src-ui/src/graph/dispatch.ts:352-353`（`route` = `${target.agent}/${target.model}`）→ `DispatchOverlay.tsx:602`（路由卡）、`:753-754`（结果卡头）、`:888`（用量表）；同类还有 `right/orchestration/NodePanel.tsx:123`、`:273`、`OrchestrationPanel.tsx:280`、`PresetBuilder.tsx:35`。
- **依据**：`resources/agent-display.ts:2-3` 自己声明「机器 id …不能直接画到界面」，任务书 §2 的名单只列到「工具设置」，所以这是「未闭合/需裁决」，不是实现写错。注意 `routeLine` 的同一串还会进裁决上下文（`DispatchOverlay.tsx:391-394`）并被测试钉住（`graph/__tests__/dispatch.test.ts:428-431`、`presets-05b.test.ts:187-190`）——要改只能在渲染层加一层 `agentDisplayName()`，别动 `agent/model` 这个标识串。
- **建议**：明确裁决要么保留（写清「路由行是标识串」），要么只改渲染层。

### 5〔中〕新增测试钉不住本期核心行为，且报告的红绿结论不可复现

- **事实**：
  - `commander/launcher.ts:213-215` 的 `commanderLauncherToolPatch` 产品代码没调用（只有 `launcher.ts:223` 与测试 `commander-10.test.tsx:147`），`expect(commanderLauncherToolPatch()).toEqual({tool:'graph',toolData:''})` 是复述一个字面量。
  - `commanderAutoStart`（`launcher.ts:204-210`）是 `commanderDefaultAvailable`（`launcher.ts:192-202`）的纯转发，两者各有断言同组事实的测试（`commander-10.test.tsx:137-143` 与 `:322-328`）。
  - 全仓没有渲染 `CenterPanel` 的测试（`grep CenterPanel` 只有 `App.tsx:19`），所以「探测返回后不会二次启动」「点更改后不会被默认立刻再起」改坏了测试不会红；`commander-10.test.tsx:146-150` 只测了辅助函数。
- **报告问题**：`tasks/REPORT-15.md`「测试先红」写「临时…关闭自动启动…定向 Vitest 如期出现 5 个失败」。按现有测试，只有改 `commanderAutoStart` 本体才会红，改 `CenterPanel.tsx:1235-1246` 不会红，因此该红绿核对不能证明接线被钉住。
- **建议**：把判定抽成可测状态机（tab id → 已启动过/已看过启动器）或用 reducer + 一次 render 断言「探测 false→true 只产生一次启动」「点更改后不再产生」；`commanderLauncherToolPatch` 删掉或让 `CenterPanel` 直接调用。

### 6〔中低〕「两端同源」是两份手抄字符串，没有任何测试比对 Rust 与前端

- **位置**：`src/resources.rs:47-50` 与 `src-ui/src/resources/agent-display.ts:5-10` 各写一份；Rust 侧 `resources.rs:3449-3458` 的 `assert_eq!(NATIVE_AGENTS, [...])` / `assert_eq!(PI_AGENT, ("pi","pi agent"))` 是复述常量定义，`:3459-3462` 的 `tools::find(...).display_name == name` 是常量比常量；前端 `commander-10.test.tsx:127-134` 同理。
- **后果**：单边改名（例如只把 `AGENT_NAME_PI` 改成别的）两端会不一致且测试全绿；任务书「vitest 读同一事实」没做到。
- **建议**：像 CSS 断言那样用 `node:fs` 读 `src/resources.rs` 抽出四个 `AGENT_NAME_*` 与前端表比对（`commander-10.test.tsx:424-437` 已有先例）。

### 7〔低〕网格测试的名字大于断言，选中态不变宽没被钉住

- **位置**：`commander-10.test.tsx:424-437`。只断言了两个常量、CSS 的两条正则、渲染 HTML 里的两个变量赋值；既没断言换行行为，也没断言 `.is-picked` 不改宽度（`CenterPanel.css:374-376` 事实是只改 `border-color`）。
- **建议**：补 `.commander-agent-card.is-picked` 规则存在且不含 `width`/`padding`/`border-width` 的断言，并把用例名改成与断言一致（`窄窗口可换行` 目前只由常量里的 `auto-fit` 间接体现）。

### 8〔低〕文档/原型里的旧名与旧标志残留

- `docs/PRODUCT.md:234` 仍写「pi 无公开标志，**用 π 字形**」——本期已改成字母徽标 `pi`，文档没同步。
- `docs/PRODUCT.md:243` 写「Claude Code / Codex / **Kimi** / **pi**」，与同文件 `:247` 自定的四个显示名不一致。
- `docs/proto/index.html:1046/1052/1057`：`Codex CLI` / `Kimi Code CLI` / `pi`（界面基准原型）。
- `AGENTS.md:7`、`README.md:3` 的产品简介也有「Kimi Code CLI / pi」。
- **建议**：至少同步 `PRODUCT.md:234` 与 `:243`；原型要么一起改，要么在文件头注明是历史快照。app 内没有残留（见下）。

### 9〔低〕几处小冗余/小缺口

- `CenterPanel.tsx:1238` 的 `commanderDefaults === null` 在 `commanderAutoStart` 已保证非空后是死条件；`:1239` 先把 tab id 记进集合再调用可能空转的 `launchCommander`（`:1194-1196` 早退、`selectTool` 在 `activeTerminalId` 为空时什么都不做），没有回滚——当前 auto-start 路径上这两种失败不可达，属防御性问题。
- pi 的细行/标题只写模型 id 不写 provider（`launcher.ts:110-116`），与启动器下拉的 `provider / model` 口径（`launcher.ts:76-79`）不一致：同名模型挂两个 provider 时分不清跑的是哪个。建议 `commanderTabTitle` 收 provider。
- `brand-marks.ts:67-70`/`81-86` 的 `kind: 'glyph'` 分支已无生产者（π 时代残留），`AgentBadge.tsx:35` 与 `resources.css:32` 还在兜它；删掉或注明保留原因。

---

## 我核过没问题的要点

1. **一次性判定与时机**：`CenterPanel.tsx:1235-1246` 先记 id 再启动，`toolsScanned`/`toolsInstalled`/`commanderDefaults` 晚到导致的 effect 重跑都在 `:1238` 挡住；`main.tsx:25` 开着 StrictMode，双跑 effect 因 ref 复用也只会起一次。三个前置条件对「旧 tab 重启」（`app-state.tsx:495-502` 不改 tool）与「+ → 对话图」（`CenterPanel.tsx:935` 置 `graphViewTabs[id]=true`，`:1236` 提前返回）天然不适用。
2. **「在此启动总指挥」**（`Explorer.tsx:824-832` → `commander/launch-request.ts` → `CenterPanel.tsx:1282-1300`）：走 `ADD_TERMINAL` 建**带工具**的新 tab（`CenterPanel.tsx:1211-1223`），不是空白 graph tab，不会被自动直启二次接管；`commanderSettings` 挂在 tab 上，PTY 起来后才写盘（`TierTerminal.tsx:1299-1300`）。
3. **「更改」**：`CenterPanel.tsx:1712-1725` 的 `SET_TERMINAL_TOOL{graph,''+toolData:''}` 会清掉 `commanderTitle`/`commanderSettings`/`toolTitle`（`app-state.tsx:478-482`），细行消失、待存快照作废；同时把该 id 记进集合，所以回到启动器后不会被默认立刻再起。自动起的 tab / 手动起的 tab / 起过又改过的 tab 三条分支我逐个走过，都只剩一次启动。顺带：PTY 起失败时 `commanderTitle` 仍在，用户有「更改」这条退路。
4. **tab 逻辑回归**：`emptyGraphTabTitle`/`graphTabTitle`（`launcher.ts:172-179`）与 `case 'graph'`（`CenterPanel.tsx:1431-1434`）语义未变——未点对话图的空白 tab = 「总指挥」，点了 = 「对话图」，已开图的空标题仍是「对话图」；标题优先级 `toolTitle > commanderTitle > cwd 名`（`:1491`）保持 TASK-10 口径；启动卡的「总指挥」入口（`:1782-1796`）未受影响。
5. **初始 tab**：`app-state.tsx:785` 建的就是空白 graph tab，会被同一条 effect 覆盖 → 有默认时**开应用即进会话，首屏再也看不到启动器**。这超出任务书「新开总指挥 tab」的字面范围，但与 `PRODUCT.md:247` 意图一致；报告和手册都没写这条，建议补一句。
6. **显示名一致（Rust 侧）**：`resources.rs:47-56` 一处定义 → 四个 ToolDescriptor 引用常量（`claude.rs:7`、`codex.rs:7`、`kimicode.rs:35`、`pi.rs:22`）；`ensure_agents`（`resources.rs:627-655`）对四个 id 无条件重写 name，所以旧 `resources.json` 的 `Kimi Code CLI`/`pi` 一读就迁移，测试用真实旧值钉住（`resources.rs:3481-3496`）；机器 id `claude/codex/kimi/kimicode/pi` 全未改。
7. **显示名一致（前端 8 处）**：启动器（`CommanderLauncher.tsx:262`、`launcher.ts:26-28`）、左栏资源（`ResourcesTab.tsx:151`）、左栏历史（`HistoryBoard.tsx:101` 走 Rust 注册表）、接入页（`ResourcesOverlay.tsx:443`、`:272-275`）、模型分组（`model-picker.ts:27-31`）、tab 标题（`CenterPanel.tsx:1387/1395/1403/1406`）、工具设置（`CenterPanel.tsx:2112-2118` → `ToolConfigModal.tsx:155`）、启动卡（`CenterPanel.tsx:596-601`）全部取表；只有 #4 那两处例外。app 内没有裸 `Kimi Code CLI`（仅 `resources.rs:3484` 的旧库测试数据）与裸 `pi` 标签。
8. **pi 标志结论与实现一致**：`C:\Users\keros68\.local\bin\pi.cmd` → `D:\pi-official\pi.exe`；包目录 `package.json` 是 `@earendil-works/pi-coding-agent` 0.85.1 / MIT；全目录 `**/*.{ico,png,jpg,svg,gif}` 只有 `assets/clankolas.png`（吉祥物）+ `docs/images/` 4 张截图，无可用标志；`README.md:3` 的标志是远程 `https://pi.dev/logo-auto.svg`。实现走字母回退（`brand-marks.ts:85` 特判 `pi`，`AgentBadge.tsx:33-36` 渲染），色相由样式表给（`CSS_TONED_SLUGS` 含 pi，`resources.css:64-66`/`83-85`），未新增任何远程或图片资产，`NOTICE` 现条目只覆盖 claude/codex/kimi，确无需新增。（注：启动卡/历史行的 pi 仍是既有的单色几何 `icons-inline/pi.svg`，与本期的字母徽标并存，属 T2 遗留，不在本期范围。）
9. **卡片网格事实成立**：`launcher.ts:19-20` 常量 + `CommanderLauncher.tsx:253-267` 以 CSS 变量下发，`CenterPanel.css:349-353` 消费；`repeat(auto-fit, minmax(150px,150px))` 是定宽轨道（窄窗口掉列、不拉伸），卡片 `height:48px` + `box-sizing:border-box`（`:355-368`），选中只改 `border-color`（`:374-376`）→ 等宽等高、换行仍等宽、选中不变宽；只是最后一条没进断言（见 #7）。
10. **回归面**：`commander.json` 读写在 Rust 侧未动（`orchestration.rs:543-587`、`879-888`），`version != 1` 落空默认（`:553`），非法 provider/model 拒写（`:564-568`）；TASK-10 R1 的消费快照逻辑未动（`launcher.ts:245-253`、`CenterPanel.tsx:1249-1262`、`app-state.tsx:483-489`，测试 `__tests__/app-state.test.ts:89-117`）。
11. **文案**：本期新增的用户可见串（`CommanderLauncher.tsx:246`、`:257-268`、`:305`；`CenterPanel.tsx:1625`、`:1683`、`:1724`、`:1795`；`launcher.ts:88-90`、`:110-116`）我按 `scripts/copy-audit.py` 的规则逐条比对（双否/第二人称/第一人称/内部词/冒烟/添加/期次/破折号），均未命中；`docs/MANUAL.md:44-63` 补了自动直启、细行+「更改」、CLI 内 `/model`，术语表 `:187` 加了「内置 agent 显示名」一行。

## 未能核实（只读环境限制）

- 无 shell、`.git` 是文件（worktree 链接），所以 `git status/diff`、`git diff --check`、`server.rs` 4666 CRLF 这类 git 级结论我无法确认，也不知道本期精确改动面。
- 未复跑 `cargo check/test`、`vitest`、`npm run build`，也没跑 `scripts/copy-audit.py`（只按规则核了本期新增串；报告里那 4 条既有命中的归属未复核）。