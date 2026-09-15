审查完成。说明两点事实前提：本次审查只读代码、未执行 vitest/cargo（本会话无 shell），「测试通过」以 REPORT-10-R1.md 自述 + 静态核对断言可红为据；审查进行中工作区被并行追加过一处改动（`graphTabTitle`，CenterPanel.tsx/launcher.ts/commander-10.test.tsx 三处联动，行号以审查结束时状态为准），该改动不在 fix-10-r1.md 与 REPORT-10-R1.md 记录内。

## 总判

**可交付**。10 条逐一核对实现与测试，全部真实修好；新发现仅低severity 边缘项，不阻塞。

## 逐条核对

**1. 已修**。消费在保存成功之后：`CenterPanel.tsx:1225-1235` `rememberCommander` 只有 `saveCommander(...).then(saved)` 才 dispatch `CONSUME_COMMANDER_SETTINGS`（`app-state.tsx:483-489` 置 `commanderSettings: undefined`）；消费后 `commanderStartedCallback`（`launcher.ts:226-233`）返回 `undefined`，`CenterPanel.tsx:1690` 不再注册 `onStarted`，换目录（`Explorer.tsx:703-706` SET_FOLDER+RESTART_TERMINAL）与重启均不再保存。reducer 测试 `app-state.test.ts:89-117`，组件层测试 `commander-10.test.tsx:297-309`（注：「组件层」实为抽出的纯函数级测试，TierTerminal→CenterPanel 接线靠读码确认）。

**2. 已修**。清单源 = 原 launchpad 同一份 `BUILTIN_AI_CLI_FALLBACK`（`CenterPanel.tsx:1657`），过滤条件 `installed[key]===true && commanderAgentOfTool(key)===null`（`launcher.ts:162-171`），恰排除 claude/codex/kimicode/pi 十一家留十一家；点击 `openCommanderCli → onOpenTool → selectTool(tool, undefined, cwd)`（`CommanderLauncher.tsx:100-107`、`CenterPanel.tsx:1663-1667`），不传 `launch`，无 extraArgs/无 commanderSettings；「+」菜单三项未动（`CenterPanel.tsx:1592-1594`）。测试 `commander-10.test.tsx:255-274`（过滤+转发）、`:365`（第三项渲染）。

**3. 已修，不串状态**。`tool-scan.ts:8-19` 模块级快照 `{installed, scanned}` + window 事件；CenterPanel 两条扫描路径（`CenterPanel.tsx:824-851`、`:868-888`）都汇入 `applyToolsInstalled`（`:676-691`，setToolsScanned(true) + publishToolScan），Explorer 只订阅不发布（`Explorer.tsx:570, 588-594`）。共享的数据是 PATH 探测这一机器级事实，无 per-session 维度，多 tab/多 session 语义正确；`scanned` 单调不回退；发布方全仓只有 CenterPanel 一处。启动器 `toolsScanned=false` 时显式「探测中…」不出空态不出一键行（`CommanderLauncher.tsx:239-244`），`availableCommanderAgents` 仍 `=== true`（`launcher.ts:40-47`）。原固化 `{}` 的用例已改为「未扫描→探测中；已扫描且空→没探测到」（`commander-10.test.tsx:340-349`）。

**4. 已修，前后端同一规则**。前端 `isCommanderIdentifier`（`args.ts:52-54`）：空串放行，非空须无前导 `-` 且整体匹配 `^[A-Za-z0-9._:/@+-]+$`；Rust `valid_commander_identifier`（`orchestration.rs:552-558`）字节级同字符集同规则（含非 ASCII 拒绝，与无 `u` 旗标正则等价），空串放行。整项拒绝而非 trim：`'   '`、`' x '` 均不匹配；`commanderArgs` 非法时返回 `[]`（`args.ts:84`），`launchCommander` 入口直接 return（`CenterPanel.tsx:1190`），启动按钮 disabled + 显式「模型 id 含非法字符」（`CommanderLauncher.tsx:203-208, 293-307`），Rust `save_commander_in` 拒写（`orchestration.rs:535-539`）。测试：`commander-10.test.tsx:93-102` 覆盖空格/`&`/`-` 开头/纯空白/非法 provider；`orchestration.rs:1301-1319` 同集且带读回不变断言。

**5. 已修**。`append_per_launch_args`（`server.rs:870-878`）configured 在前、per-launch 在后、`!arg.trim().is_empty()` 过滤空串，仅 `server.rs:806` 一个调用点；Rust 测试 `server.rs:4338-4355` 用相等断言钉死顺序与空串过滤。文档一致：`tasks/TASK-10-commander.md:8`「先追加工具设置的 extra_args，再追加 commanderArgs；重复旗标以本次启动为准」，`docs/PRODUCT.md:241`「本次选的模型排在最后、优先生效」，与实现及 `args.ts:4-5` 注释同口径。

**6. 已修**。`graphTabTitle`（`launcher.ts:180-183`）：有 toolData 的 tab 一律按图标题（空标题回落「对话图」），只有空白 tab 按 `emptyGraphTabTitle(graphViewSelected)` 判；`CenterPanel.tsx:1401` 接线。`handleAddTab('graph')` 置 `graphViewTabs[id]=true` →「对话图」，`handleAddTab('commander')` 不置 →「总指挥」（`CenterPanel.tsx:919-931`），次级入口 `:1662` 同理。测试 `commander-10.test.tsx:244-253` 两种 tab 各有断言，且含「已开图但标题为空仍是对话图」的边缘（并修掉了过渡版本 `title || emptyGraphTabTitle(...)` 会把已开图空标题 tab 显示成「总指挥」的错位）。

**7. 已修**。Explorer 改用与启动器同一个 `commanderDefaultAvailable(commanderDefault, toolScan.installed, toolScan.scanned)`（`Explorer.tsx:595-599`，实现在 `launcher.ts:194-203`：未扫描/默认无效/默认 agent 未安装均 false），按钮渲染条件 `:825`；数据来自与 CenterPanel 同一份扫描快照。测试 `commander-10.test.tsx:290-295`。

**8. 已修**。空模型提示加 `modelsLoaded` 门（`CommanderLauncher.tsx:274` + `launcher.ts:185-192`，测试 `:276-280`）；agent 卡 onClick 经 `markCommanderSelection` 立即置位 `seeded`（`CommanderLauncher.tsx:257`、`launcher.ts:209-216`），晚到默认的 seed effect（`:159-173`）被挡住（测试 `:282-288`）；`cwdName` 对 `C:\`/`D:/` 返回盘符、`/` 返回 `/`（`launcher.ts:86-87`，测试 `:197-205`）。

**9. 已修**。模型 select 抽成 `CommanderModelSelect`（`CommanderLauncher.tsx:58-85`），选中值 `commanderModelSelectValue`（`launcher.ts:218-223`）与 option `commanderModelValue`（`:124-126`）同用固定 `{provider, model}` JSON 序列化；测试 `commander-10.test.tsx:181-195` 硬编码期望串并静态渲染 `picked !== null` 的 select 验证选中。`commanderAgentOfTool` 已有生产引用（`launcher.ts:167`）。Rust version 拒写补读回断言（`orchestration.rs:1291-1299`）。

**10. 已修**。`save_commander_in` 目标文件存在且解析失败即拒写（`orchestration.rs:540-548`），与 `save_premises_in`（`:497-512`）同口径；读侧仍空默认不改文件（`:520-526`）。测试 `orchestration.rs:1279-1289` 拒写 + 原文逐字节不变。

## 新发现的问题（按严重度）

1. **[低] 损坏 commander.json 时保存静默失败且每 tab 反复重试**（第 1、10 条的叠加效应）。`CenterPanel.tsx:1235` `.catch(() => {})` 吞掉 Rust 拒写错误 → 快照不消费 → 该 tab 每次重启重新尝试保存、再次被拒（`orchestration.rs:541-548`），用户默认永远存不上且无任何提示。复现：手工把 `~/.aicross/commander.json` 改坏 → 起一次总指挥 → 启动成功但默认未更新，重启 tab 再试再失败。拒写本身是第 10 条的既定取舍，建议后续给保存失败一个用户可见提示（记入报告即可，非本期必修）。
2. **[低] 启动器 cwd 可被晚到默认覆盖的残留窗口**。`CommanderLauncher.tsx:170-172`：`pickFolder`（`:194-201`）不置 `seeded`；若用户在默认 IPC 返回前用目录选择器选了目录且未点过 agent 卡，seed 会用 `defaults.cwd` 覆盖用户选择。实际窗口极小（defaults 是挂载即发的 IPC），且 fix 第 8 条只要求 agent 卡置位，属按规格实现，建议顺手把 pickFolder 也置位。
3. **[低] 目录行换文件夹作用在空白 graph tab 上会丢「已进对话图」标记**。`Explorer.tsx:698-706` 对 `tool==='graph'` 的 tab 同样发 RESTART_TERMINAL，tab id 变更后 `graphViewTabs` 按旧 id 索引失效（`CenterPanel.tsx:737`），用户点过「对话图」的空白 tab 会翻回启动器视图。TASK-10 引入（graphViewTabs 是新增），触发条件苛刻，影响仅视图回退。
4. **[信息] 换目录重启后 commanderTitle 仍显示旧目录名**。RESTART_TERMINAL 保留 `extraArgs`/`commanderTitle`（`app-state.tsx:495-502`），重启后 tab 标题 `agent · 模型 · 旧目录名` 直到 CLI 报 OSC7 才纠正。与 fix 清单「不改（记录）」第一条同类，属已接受口径。
5. **[信息] CenterPanel 接线层无测试**：`otherClis=BUILTIN_AI_CLI_FALLBACK×toolsInstalled`（`:1657`）与 `onOpenTool→selectTool`（`:1663-1667`）的组装只有单元级覆盖（过滤器、菜单渲染各测各的），与既有「启动链路端到端无自动化」同类缺口。
6. **[信息] 工作区在审查期间被并行追加 `graphTabTitle` 改动**（CenterPanel.tsx:1397-1403 / launcher.ts:178-183 / commander-10.test.tsx:247-252），fix 清单与 REPORT 均未记录该条；改动本身正确且修了一个真实边缘（已开图空标题 tab 的标题）。

## 核过没问题的点

- **测试可红**：全部抽到的断言为硬编码期望值（`-m` vs `--model`、非法值字面报错、`'{"provider":"a/b","model":"kimi-code/k3-256k"}'`、Rust 侧 `vec!["--model","configured","--model","picked"]`、文件读回逐字节比较），无实现镜像式同义反复；菜单点击测试直接调用元素 `onClick` 并断言转发参数，虽耦合 React 元素结构但能红。
- **回归面**：app-state 既有 reducer 用例（主题迁移、CLOSE_TERMINAL graphFocus、R5-5/5-6）原样保留且与新 case 无冲突（CONSUME 为纯新增 action，RESTART_TERMINAL 本轮未改语义）；`renderTabContent` 改动收敛在 `case 'graph'`；非总指挥 `selectTool` 经 SET_TERMINAL_TOOL 把 `extraArgs/commanderTitle/commanderSettings` 置回 undefined，无跨启动残留；外部 `coffee-cli launch`（`applyLaunchRequest`，`:967`）不带 commanderSettings，不会误触发保存。
- **白名单规则前后端逐字符等价**（字符集、前导 `-`、空串、非 ASCII 四个维度比对一致）；`saveCommander` 生产调用点唯一（`CenterPanel.tsx:1235`），入参必经 `launchCommander` 校验。
- **scanned 快照无会话维度泄漏**；发布方唯一、消费方只读；`availableCommanderAgents` 的 `=== true` 口径未被放松。
- **未动区确认**：`src/channels/`、`src/dispatch.rs`、`src/terminal.rs` 无 TASK-10 R1 触碰（未重读其内容，依据 fix 清单声明与 server.rs 调用面未变）；「+」菜单与 launchpad（含不可达的 `tool===null` 卡）保持首版原样。
- **未能独立复核项**（如实声明）：`cargo check/test`、vitest、`npm run build` 本会话未执行，REPORT-10-R1.md 的 325/390/零 warning 结论无法重放确认；TierTerminal→PTY 端到端启动无自动化覆盖（报告已声明）。