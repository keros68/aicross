## 总判

**可交付但有未闭合项** —— 10 条的实现主体都真实落地（读代码 + 测试逐条核对，非照抄报告）；但有 1 条判定条件比清单宽、引入了新的图 tab 标题错位（第 6 条），另有 4 处低severity 未闭合（失败探测永久「探测中」、按 id 消费的竞态、组件层测试缺失且一条断言近似同义反复、graphViewTabs 泄漏）。

---

## 10 条逐条

**1. CONSUME_COMMANDER_SETTINGS 时序 / 重启不再回写 —— 已修（测试未达清单要求）**
- 消费只在保存成功后：`CenterPanel.tsx:1225-1236`（`saveCommander(settings).then(saved).catch(()=>{})`，`saved()` 内 `dispatch(CONSUME)`）；reducer 只清该 tab 的快照 `app-state.tsx:483-489`。
- 重启/换目录不再保存：`CenterPanel.tsx:1689` 走 `commanderStartedCallback`，`launcher.ts:219-227` 在 `commanderSettings === undefined` 时返回 `undefined`；RESTART_TERMINAL 保留会话字段但不再能触发 `onStarted`（`app-state.tsx:495-502` + TierTerminal 挂载 effect 只调一次 `TierTerminal.tsx:1299-1300`）。Explorer 换目录路径 `Explorer.tsx:700-706`（SET_FOLDER + RESTART_TERMINAL）确认不再注册保存回调。
- 测试：reducer 层真断言消费+重启 `__tests__/app-state.test.ts:88-116`（可红）；清单要求的「组件层重启不再调 saveCommander」**没有**——`commander-10.test.tsx:290-302` 是纯 helper 测试，第二段还近似同义反复（见新发现 3）。
- 附带：保存失败（文件损坏/非 Tauri 外的 IPC 错误）时不消费、下次启动重试，符合「首次保存成功后清掉」的字面。

**2. 「其它 CLI」清单来源与点击路径 —— 已修**
- 来源即原「打开终端」菜单的数据源：`CenterPanel.tsx:591-595` 的 `BUILTIN_AI_CLI_FALLBACK`（15 项，与 Rust 注册表 `src/tools/mod.rs:268-293` 逐项一致）→ `CenterPanel.tsx:1656`。
- 过滤正确：`launcher.ts:162-172` 要求 `installed[key] === true` 且 `commanderAgentOfTool(key) === null`，即排除 claude/codex/kimicode/pi 四家，正好放出其余 11 家；`terminal` 不在 catalog 里，`check_tools_installed` 的 `terminal:true`（`server.rs:140`）不会漏进来。
- 点击沿原路径且不带模型参数：`CenterPanel.tsx:1662-1665` → `selectTool(tool, undefined, cwd)`，`launch` 参数缺省 → `extraArgs/commanderTitle/commanderSettings` 全 undefined（`CenterPanel.tsx:1143-1180`）。
- 「+」菜单三项未动（`CenterPanel.tsx:1592-1596`）；次级入口的 `其它 CLI` 用 `<details>` 折叠、空清单时返回 null（`CommanderLauncher.tsx:87-112, 318`）。测试 `commander-10.test.tsx:248-267, 358-368`（过滤可红；点击只到 helper 层）。

**3. scanned 共享是否串状态 —— 已修**
- 共享是「本机是否已探测」这一机器级事实，不承载 tab/session 状态：`tool-scan.ts:1-18` 是模块单例 + window 事件；CenterPanel 在扫描成功后 publish（`CenterPanel.tsx:676-690`），Explorer 订阅同一事件（`Explorer.tsx:570-571, 589-594`）。应用只有一个窗口（`tauri.conf.json:14-17`，无 `WebviewWindowBuilder`），CenterPanel 也只有一个实例；新开 tab 复用 CenterPanel 级 `toolsScanned`，不会因 tab 数变化回退到「未扫描」。
- `availableCommanderAgents` 仍只认 `=== true`（`launcher.ts:40-47`），launchpad 的乐观口径 `!== false`（`CenterPanel.tsx:1760` 附近）未被污染。
- 测试：组件层覆盖了「未扫描→探测中 / 已扫描空→没探测到」`commander-10.test.tsx:333-342`；`tool-scan.ts` 本身与 Explorer 订阅无测试（见新发现 3）。

**4. 白名单前端/Rust 同规则、整项拒绝 —— 已修**
- 前端 `args.ts:52-59`：`''` 放行，非空需 `^[A-Za-z0-9._:/@+-]+$` 且不以 `-` 开头；不 trim。`commanderArgs:83-100` 任一非法即返回 `[]`（连 provider 也整项不发）；`commanderBlockReason:145-152` 供 UI 提示；`launchCommander` 再拦一次（`CenterPanel.tsx:1189-1190`）。
- Rust `orchestration.rs:552-558` 字符集与 `-` 规则逐字符等价（允许 `. _ : / @ + -` + ascii alnum，`''` 放行），`save_commander_in:535-539` 拒写；读侧不校验但前端 `commanderDefaultAvailable:187-196` 会把非法默认的一键行/目录行按钮藏掉，argv 不会被构造。
- 测试可红：`commander-10.test.tsx:92-104`（空格/`&`/`-` 开头/纯空白/非法 provider 逐条）、`orchestration.rs:1301-1319`（拒写 + 原文不变）。JS `$` 无 `/m`，`"x\n"` 也拒——两端口径一致。

**5. 追加顺序与文档口径 —— 已修**
- 实现不变且顺序固定：tools.json `extra_args` 先（`server.rs:786-791`），本次启动参数最后（`server.rs:798-806` → `append_per_launch_args:870-878`，过滤空/纯空白）。
- 测试钉住顺序+空串过滤+`None` 不变：`server.rs:4338-4355`（硬编码期望 `["--model","configured","--model","picked"]`，可红）。
- 文档同口径：`tasks/TASK-10-commander.md:8`「先追加工具设置的 extra_args，再追加 commanderArgs；重复旗标以本次启动为准」、`docs/PRODUCT.md:241`「…照旧追加，本次选的模型排在最后、优先生效」、`args.ts:3-6` 与 `server.rs:798-805` 注释一致；`docs/MANUAL.md:47` 只说「仍会传入」，不冲突。

**6. 空白 tab 标题 —— 部分（主用例对，判定条件比清单宽，新增图 tab 错位）**
- 空白 tab：`CenterPanel.tsx:1398-1401` + `launcher.ts:174-176`，未进 graphViewTabs → 「总指挥」，点过对话图 → 「对话图」；`CenterPanel.tsx:929` / `1661` 两处置位，`1651` 决定画启动器还是 GraphTab，两者同步。测试 `commander-10.test.tsx:243-246`（只测 helper，CenterPanel 表达式无测试）。
- 问题：兜底被用在**所有** graph session 上，而不是清单写的「toolData 为空的 tab」。凡 `state.graphs[toolData].title === ''`（新建图 title 就是空串，`graph/types.ts:158-176`）或图不在 state 里时，都会掉到 `emptyGraphTabTitle(graphViewTabs[session.id])`，于是「有图但标题空」的 tab 会显示「总指挥」——见新发现 1（含 RESTART_TERMINAL 改 id 后 graphViewTabs 失配的复现）。

**7. 目录行按钮查安装 —— 已修**
- `Explorer.tsx:595-599` 读同一份扫描快照，`Explorer.tsx:825-832` 只在 `hasCommanderDefault && folderPath` 时画；`commanderDefaultAvailable`（`launcher.ts:187-196`）要求 `scanned===true` + 工具 id `=== true` + 标识符合法，与启动器一键行（`CommanderLauncher.tsx:211-212`）同一函数同一口径。未扫描/未装/默认非法 → 不画按钮，与清单一致。
- 测试 `commander-10.test.tsx:283-289`（覆盖 scanned=false、installed=false、true 三态；Explorer 接线无测试）。

**8. 启动器小项（8a/8b/8c）—— 已修**
- 空模型提示加 `modelsLoaded` 门：`CommanderLauncher.tsx:274` + `launcher.ts:178-185`，测试 `commander-10.test.tsx:269-273`（三态可红）。
- 点过 agent 卡即置位 seeded：`CommanderLauncher.tsx:257` + `launcher.ts:202-208`（`seeded.current = true` 先于 `setValue`），seed effect `CommanderLauncher.tsx:159-170` 只在 seeded 为假时跑；测试 `275-281`（但只测 helper，见新发现 3）。
- `cwdName` 根目录：`launcher.ts:84-93`（`C:\`/`D:/`→盘符，`/`→`/`，尾斜杠不额外成级），测试 `196-206` 含 `C:\`、`D:/`、`/`。

**9. 测试补课 —— 已修**
- `picked`/option 共用固定 key 顺序：抽成 `CommanderModelSelect`（`CommanderLauncher.tsx:61-85`）+ `commanderModelSelectValue`（`launcher.ts:211-217`），测试 `commander-10.test.tsx:180-195` 断言字面量 JSON 与渲染出的 `selected=""`，可红。
- `commanderAgentOfTool` 不再是死代码：`launcher.ts:162-172` 用于「其它 CLI」过滤（生产路径）。
- 写侧拒 version 补读回断言：`orchestration.rs:1291-1299`（`is_err()` + 文件字节未变）。

**10. 损坏 commander.json 拒写 —— 已修**
- `orchestration.rs:540-549`：文件存在且解析失败即 `Err`，不覆盖；读侧仍空默认且不改文件（`520-526`）。测试 `orchestration.rs:1279-1289`（先断言读回 "not json"，再断言保存 `is_err()` 且内容仍是 "not json"）。

---

## 新发现（按严重度）

**1. [低-中] graph tab 标题在「有图但图标题为空 / 图未载入」时错显「总指挥」（第 6 条引入的回归）**
- 位置：`CenterPanel.tsx:1400`（`state.graphs[session.toolData ?? '']?.title || emptyGraphTabTitle(Boolean(graphViewTabs[session.id]))`）、`CenterPanel.tsx:929/1661`、`app-state.tsx:495-502`、`graph/types.ts:160-175`（新图 `title: ''`）、`GraphTab.tsx:177-190, 504-518`。
- 复现：`+`→对话图（graphViewTabs[id]=true，标题「对话图」）→ 新建一张尚未发消息的图（title 为空，标题暂显「对话图」是对的）→ 在左栏点工作区目录换一个目录（Explorer 派发 `RESTART_TERMINAL`，session id 换成新 UUID，`graphViewTabs` 仍挂在旧 id 上）→ 该 tab 标题变「总指挥」，内容仍是这张图。基线该 case 固定返回「对话图」，不会有这个错位。次生：图文件被删（`loadFailed`）时 body 写「找不到这张图」而标题写「总指挥」。
- 建议：按清单原意判 `toolData`，例如 `session.toolData ? (state.graphs[session.toolData]?.title || '对话图') : emptyGraphTabTitle(Boolean(graphViewTabs[session.id]))`；顺手在 RESTART_TERMINAL/CLOSE_TERMINAL 时迁移或清理 `graphViewTabs`。

**2. [低] 探测失败或非 Tauri 时永久卡在「探测中…」**
- 位置：`CenterPanel.tsx:843-844, 880-882`（两处 `.catch(() => {})`）、`CenterPanel.tsx:689`（只有成功回调里 `setToolsScanned(true)`）、`CommanderLauncher.tsx:239-241`。
- 复现：浏览器里跑 UI（`!isTauri` → 扫描 effect 直接 return），或 `check_tools_installed` IPC 失败 → 启动器永远显示「探测中…」，一键行永远不出现；改动前这里显示的是「本机没探测到…」。两者都不理想，但新状态永不收敛，且把「失败」和「在途」混为一谈。
- 建议：catch 分支也 publish（scanned=true + 空/上次快照），或加超时兜底。

**3. [低] 组件层接线无测试，且一条新断言近似同义反复**
- 事实：全仓测试对 `CenterPanel.tsx`/`Explorer.tsx` 零引用（`grep` 结果为空），第 1/3/6/7 条的「组件层」要求全部落在纯函数上：`commander-10.test.tsx:290-302`（第 1 条）、`243-246`（第 6 条）、`283-289`（第 7 条）、`275-281`（8b）。其中 `290-302` 的 `afterConsume = commanderStartedCallback({ id: 'tab-b' }, …)` 传的是**不带** `commanderSettings` 的新对象，断言 `toBeUndefined()` 等于把函数实现抄了一遍——把 `onStarted` 换回旧写法（`t.commanderSettings ? () => rememberCommander(...)`）也不会红。
- 建议：至少加一条读 CenterPanel 源码/渲染的接线断言（`commanderStartedCallback(t, rememberCommander)`、`toolsScanned={toolsScanned}`），或把 `290-302` 换成「同一 tab 消费后再挂载 → 不再产生保存」。

**4. [低] 按 id 消费存在竞态：保存与 RESTART_TERMINAL 撞车时旧快照会留在新 id 上**
- 位置：`CenterPanel.tsx:1225-1235`（`dispatch(CONSUME, { id: sessionId })` 用的是闭包捕获的旧 id）、`app-state.tsx:495-502`（RESTART 改 id 但保留 `commanderSettings`）、`Explorer.tsx:700-706`。
- 复现：启动总指挥成功 → `save_commander` 尚在途中 → 用户此时换工作区目录（RESTART_TERMINAL 换 id）→ 消费落到旧 id（无匹配）→ 新 id 仍带旧快照 → 之后任何一次重启把它写回。窗口只有几毫秒（目录选择要过弹窗），实际概率低；同类后果的另一种形态：首次启动失败未消费 → 换目录后新 id 启动成功，会拿**旧目录**的 cwd 落盘。
- 建议：消费按「会话对象」而不是 id（或 RESTART 时一并迁移/清理快照）。

**5. [nit] `graphViewTabs` 只增不减**
- `CenterPanel.tsx:737, 929, 1661`；`CLOSE_TERMINAL` / `RESTART_TERMINAL` 都不清理。长时间开 tab 会累积 UUID 键（每条几十字节，量级可忽略），但正是发现 1 的根因。建议关 tab/改 id 时删除。

**6. [nit] 文档未提「其它 CLI」**
- `docs/MANUAL.md:50-54` 只写「标题栏「+」菜单可分别打开系统终端、对话图或总指挥」，新增的第 3 个次级入口没写；清单未强制，但用户面文档与 PRODUCT/TASK 的口径应一起收。

---

## 核过没问题的点

- **Rust 侧参数组装**：`extra_args` 只在 `tier_terminal_start` 内拼一次（`server.rs:806`），`None`/空数组与改动前完全一致，不落盘、不影响 resume 路径；`append_per_launch_args` 是纯函数，可单测。
- **白名单跨端等价**：字符集、`-` 前缀、空串语义、非 ASCII 拒绝、JS 无 `/m` 的 `$` 行为，逐项对齐（`args.ts:52-59` ↔ `orchestration.rs:552-558`）。
- **commander.json 契约未变**：缺字段 serde 默认、缺 version 按 1、version≠1/损坏 → 空默认且不改文件、写侧拒非 1、原子写（`orchestration.rs:268-283, 480-558`；测试 `1241-1319`）；命令已在 `server.rs:3986-3987` 注册。
- **kimi↔kimicode 映射**与四家显示顺序未动（`args.ts:19-32, 13`），`availableCommanderAgents` 仍严格 `=== true`。
- **启动链路**：`CommanderLauncher.onLaunch` → `selectTool`/`ADD_TERMINAL` → `SET_TERMINAL_TOOL`（`extraArgs/commanderTitle/commanderSettings`）→ `TierTerminal.tsx:419/1299` → `tauri.ts:145-146`（`extraArgs ?? null`）→ `server.rs:564`；重启保留 extraArgs，不带模型参数的那条路径（其它 CLI）确实不带。
- **`terminal:true` 不会伪造一条「其它 CLI」**；`BUILTIN_AI_CLI_FALLBACK` 与 Rust 注册表 15 项一一对应，没有 CLI 漏掉入口。
- **目录行按钮**：与新按钮相关的改动只在 `workspace-dir-row` 内多一颗 sibling 按钮 + flex 调整（`Explorer.css:72-101`），原目录按钮点击路径未改；点击走 `openNewTab=true` 的 `ADD_TERMINAL`（`CenterPanel.tsx:1206-1219`），不覆盖当前会话；`onLaunch` 的 cwd 用事件里的目录，标题用同一次启动的 cwd。
- **回归面**：`CONSUME_COMMANDER_SETTINGS` 是纯新增 action（`app-state.tsx:347, 483-489`），`SET_TERMINAL_TOOL` 的既有清空语义（`toolTitle`）未动；既有 reducer 测试与新 action 不冲突；没有任何既有测试断言过 graph tab 的旧标题，因此第 6 条的错位不会被现有测试拦住（这也说明测试面偏纯函数）。
- **无死代码残留**：旧「打开终端」折叠菜单的 JSX/CSS 已整块替换，`grep` 无残留类名；`commanderAgentOfTool` 已在生产路径使用。