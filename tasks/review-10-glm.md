## 总判

**可交付但有未闭合项** —— 契约、启动链路、状态副作用、安全四条主线都核过且成立，测试非同义反复；存在 2 个应处理的行为问题（旧 tab 重启回写默认、extra_args 追加顺序与两份文档字面相悖）和若干低severity UI/一致性问题。

## 问题清单（按严重度）

1. **旧总指挥 tab 重启会把过期设置回写为当前默认**
   - 位置：`src-ui/src/components/center/CenterPanel.tsx:1667`（`onStarted={t.commanderSettings ? () => rememberCommander(...) : undefined}`）、`app-state.tsx:487-494`（`RESTART_TERMINAL` 保留 `commanderSettings`）。
   - 问题：`rememberCommander` 在 TierTerminal 每次 mount 成功后都会调 `saveCommander`，重启也不例外。复现：起 tab A（claude/fable，默认=fable）→ 再起 tab B（codex/opus，默认=opus）→ 重启 tab A → `~/.aicross/commander.json` 被改回 claude/fable，一键行、目录行按钮全部跟着变。用户没有做任何「设为默认」的动作。
   - 建议：首次保存成功后清掉该 tab 的 `commanderSettings`（新增一个小 action），或给 `onStarted` 加一次性标记，使重启不再触发保存。

2. **per-launch 追加顺序与任务书/PRODUCT 字面相悖（review 点 1 的顺序项）**
   - 位置：`src/server.rs:786-813` —— tools.json `extra_args` 先追加（790），本次启动参数最后追加（806-813）。
   - 问题：`tasks/TASK-10-commander.md` 契约写「commanderArgs(...)；**再追加**工具设置的 extra_args」，`docs/PRODUCT.md:241` 写「工具设置里的额外参数照旧追加」——字面上 extra_args 应在最后。实现里 extra_args 不再是最后一个旗标：用户在工具设置里常驻了 `--model X`，用总指挥选模型 Y 启动，argv 为 `--model X --model Y`，按「后者胜」Y 赢，常驻配置被本次选择静默压掉。这个语义本身（本次启动优先）更合理，且 `REPORT-10.md:15` 与代码注释都已按实现口径写明，但与设计文档字面冲突，验收口径未闭合。
   - 建议：二选一收口——改 PRODUCT/TASK 一句话（「常驻 extra_args 在前，本次启动参数最后，重复旗标以本次启动为准」），或把追加顺序对调。改文档是更便宜的收口。

3. **空白启动器 tab 的 tab 标题显示「对话图」**
   - 位置：`src-ui/src/components/center/CenterPanel.tsx:1387`（`case 'graph'` 固定返回 `'对话图'`）。
   - 问题：「+」→ 总指挥新建的 tab 内容是总指挥启动器，tab 标题却是「对话图」；PRODUCT 要求总指挥 tab 标题是 `agent · 模型 · 目录名`（启动后才生效，启动前无标题）。复现：点「+」→ 总指挥，看 tab 条。
   - 建议：在渲染点（`CenterPanel.tsx:1636` 处能拿到 `graphViewTabs`）给空白未进对话图的 tab 一个「总指挥」标题；注意「+」→ 对话图但还没点开的 tab（toolData 同样为空）应仍显示「对话图」，所以不能只判 `toolData === ''`。

4. **目录行「在此启动总指挥」不检查默认 agent 是否已安装**
   - 位置：`src-ui/src/components/left/Explorer.tsx:563-580, 807-814`（只判 `isCommanderAgent(settings.agent)`）。
   - 问题：启动器的一键行有安装检查（`CommanderLauncher.tsx:136-137`），目录行没有。复现：默认存了 pi → 卸载 pi → 目录行仍显示按钮 → 点击新开 tab → `tier_terminal_start` 找不到二进制，终端里红字报错。任务书只说「用默认 agent/模型」，未规定检查，但两个快速启动入口口径不一致。
   - 建议：与启动器同口径（读 `toolsInstalled` 或后端探测），或明确接受「点了在终端里看到报错」并记录。

5. **启动器冷启动闪现「本机没探测到…」空态**
   - 位置：`src-ui/src/components/center/CommanderLauncher.tsx:164-167`；探测来源 `CenterPanel.tsx:814-841`（300ms debounce + 30s TTL + IPC 扫描），`toolsInstalled` 初始为 `{}`。
   - 问题：打开空白 tab 的前 ~0.3-1s 内 `agents === []`，空态文案先出现，扫描返回后才换成 agent 卡/一键行；期间一键行也不显示。探测未返回按「没装」算是有意取舍（`launcher.ts:30-37`），但空态文案在「还没探测」和「真没装」两种情况下无法区分。
   - 建议：加一个「已扫描过」信号传入启动器，扫描完成前显示「探测中…」或不显示空态。

6. **标题与实际 spawn 目录可能不一致（信息项）**
   - 位置：`src-ui/src/commander/launcher.ts:135-139`（有意不预检目录）+ `src/server.rs:831-835`（目录不存在 → 回退 home）。复现：一键行保存的目录被删后点一键启动 → tab 标题仍写旧目录名，进程实际在 home。CLI 报 OSC7 后 folderPath 会被纠正。可接受，但用户第一眼会看到错位。

7. **`save_commander_in` 缺「损坏文件拒写」护栏（nit）**
   - 位置：`src/orchestration.rs:528-536`，对照姊妹实现 `save_premises_in`（497-512，损坏文件拒写防静默丢数据）。损坏的 commander.json 会在下次启动成功时被静默覆盖。该文件只是便利默认、读侧已按空默认处理，覆盖无实害；仅与同类模式不一致，留个心眼即可。

8. **Windows 经 `cmd.exe /c` 起 PTY 的既有暴露面（信息项，非本期引入）**
   - 位置：`src/terminal.rs:593-599`。argv 全程是 `Vec<String>` 单元素传递（前端 JSON 数组 → `extra_args: Option<Vec<String>>` → `CommandBuilder.arg`），含空格/以 `-` 开头的模型名保持单个 argv，portable-pty 会按 MSVCRT 规则加引号；但 cmd.exe 重解析意味着 `%VAR%` 展开等边缘情形存在。这与既有 tools.json `extra_args`（用户自填自由文本）同一暴露面，不跨信任边界，TASK-10 未扩大它。

## 核过没问题的要点

- **契约**：`CommanderSettings`（orchestration.rs:270-283）与 `commander/types.ts` 逐字段对应；缺字段落 serde 默认、version 缺失按 1 读、读侧 version≠1/损坏 JSON → 空默认且不改文件、写侧拒绝非 1、原子写（临时文件+rename）；只存 agent/provider/model/cwd/updated_at，无密钥，不碰任何全局配置。
- **commanderArgs**：四家映射与契约逐字一致（claude/kimi `--model`、codex `-m`、pi `--provider` 在 `--model` 前），空模型不加参数（纯空白也算空），未知 agent 返回 `[]`；pi 空 provider 只发 `--model` 是对契约的合理补充且有注释说明。
- **启动链路**：`CommanderLauncher.onLaunch` → `launchCommander`（CenterPanel.tsx:1178-1213）→ `SET_TERMINAL_TOOL`/`ADD_TERMINAL` 携带 `extraArgs` → TierTerminal prop（:1666）→ `tierTerminalStart(..., extraArgs)`（TierTerminal.tsx:1299）→ invoke `extraArgs`（tauri.ts:145-146）→ Rust `extra_args`（server.rs:564）。只追加一次，无重复；`None` 时与改动前完全一致（server.rs:561-563 注释与代码相符）；不落盘。空白 graph tab 首次 SET_TERMINAL_TOOL 才挂载 TierTerminal（key 含 id+restartKey），`[]` 依赖的启动 effect 不会拿到旧参数；重启经 `RESTART_TERMINAL` 保留 extraArgs 后重放。
- **kimi 映射**：资源 id `kimi` ↔ 工具 id `kimicode` 双向表（args.ts:19-32）；`kimicode`、`pi` 均在 Rust 工具注册表（tools/mod.rs:288,291）；`check_tools_installed` 按注册表 id 返回，`availableCommanderAgents` 经 `commanderToolId` 查表正确。
- **工作目录**：launch 显式传 cwd，后端解析顺序（前端 cwd → tools.json default_cwd → home）未被改动。
- **标题**：`toolTitle（CLI OSC）> commanderTitle > baseTitle`（CenterPanel.tsx:1445），CLI 报标题后接管，符合报告所述。
- **状态与副作用**：`saveCommander` 只在 `tier_terminal_start` 成功 resolve 后经 `onStarted` 调用（TierTerminal.tsx:1298-1300）；「在此启动总指挥」走 `openNewTab=true` 新开 tab，不覆盖当前会话；「+」菜单三项（1577-1579）；对话图仅在显式选择后进入（graphViewTabs）；`load_commander` Rust 侧不返回错误、前端另有 `.catch`，启动不阻塞；`isLaunchpadMode` 判定未动，启动器不会误触发 launchpad UI。
- **回归**：`list_dispatchable_models` 未改，启动器与 G1 下拉共用 `modelStatusSuffix` 口径；Explorer 目录行按钮是目录按钮的兄弟节点，不影响原点击；旧「打开终端」折叠菜单样式被整块替换（CenterPanel.css:285-287），无残留死代码；GraphTab「返回」按钮（图被删时）现在落到启动器——是「空白=启动器」的连带结果，与产品口径一致，但语义有变，留意即可。
- **测试**：前端 38 项断言全部硬编码期望值（`-m` vs `--model`、pi 参数顺序、JSON value 往返、`cwdName` 尾斜杠、空态/一键行 gating），非实现镜像；Rust 测试走临时目录真实文件 I/O，覆盖缺失/往返/缺字段/缺 version/错 version/损坏 JSON/写侧拒绝。启动链路的端到端（TierTerminal→PTY）无自动化覆盖，报告已如实声明。