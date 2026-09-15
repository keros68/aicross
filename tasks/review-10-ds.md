## 总判

**可交付但有未闭合项**：契约（`commanderArgs` 四家映射、`commander.json` 版本/缺字段/原子写）、启动链路（extraArgs 真到 argv、kimi↔kimicode、cwd、标题）都对；但“默认记住”有一条会回写旧快照的时序缺陷、启动器首屏有假空态、空白 tab 的其它 agent 入口被收窄且 launchpad 实际不可达，另有参数注入/测试缺口需要收口。

---

## 问题清单（按严重度）

**1. [中] 重启旧总指挥 tab 会用 tab 上的旧快照回写 `commander.json`，覆盖更新的默认**

- `src-ui/src/components/center/CenterPanel.tsx:1667`（`onStarted` 闭包持有 `t.commanderSettings`）、`CenterPanel.tsx:1215-1225`（`rememberCommander` 无条件写盘）、`src-ui/src/store/app-state.tsx:487-492`（`RESTART_TERMINAL` 保留 `commanderSettings`/`extraArgs`）、`src-ui/src/components/left/Explorer.tsx:687`（换目录用新 id 触发整机重挂）、`TierTerminal.tsx:1299-1300`（挂载即 `onStarted`）。
- 问题：`commanderSettings` 一旦挂上 tab 就永不消费；任何再次挂载（换目录、错误边界重试等）都会把这份旧快照重新保存，而不是当前最新默认。
- 复现：tab1 在 A 起总指挥（默认=A）→ 另开 tab2 在 B 起总指挥（默认=B）→ 回 tab1，点左栏工作区目录换到 C（触发 `RESTART_TERMINAL`，新会话确实跑在 C）→ 启动成功回调把 {agent, model, cwd:A} 写回，默认退回 A。
- 建议：启动成功保存后清掉该 tab 的 `commanderSettings`（加一个消费 action），或保存前与当前默认做 compare-and-set；若希望换目录也更新默认，则在 `RESTART_TERMINAL` 前把新 `folderPath` 写进 `commanderSettings.cwd`。

**2. [中] 首屏把“探测未返回”当成“没装”：显示“本机没探测到…”并隐藏一键行**

- `CenterPanel.tsx:501`（`toolsInstalled` 初值 `{}`）、`CenterPanel.tsx:829-834`（扫描有 300ms debounce + PATH 探测耗时）、`CommanderLauncher.tsx:55`、`CommanderLauncher.tsx:164-167`、`CommanderLauncher.tsx:136`（`defaultAvailable` 需要 `=== true`）。
- 问题：`availableCommanderAgents` 对 `undefined` 按“没装”处理，而 `{}` 正是探测返回前的真实状态。冷启动落在空白 tab 时，用户先看到“装好任意一个再回来”，一键行也消失，约 0.3–1.3s 后才出卡。launchpad 用的是乐观口径（`!== false`），两处不一致。
- 复现：装好 claude，清掉进程后首次启动 → 空白 tab 首屏即上述空态。
- 建议：给启动器传三态/加载态（`toolsInstalled === null` → “探测中…”占位），或只在扫描完成（明确 false）后才画空态；`availableCommanderAgents` 至少别把 `undefined` 与 `false` 混为一谈。
- 附：`src-ui/src/commander/__tests__/commander-10.test.tsx:218-221` 用 `render({})` 断言“本机没探测到”，恰好把这个初值状态固化成“没装”，测试无法区分加载中与真没装。

**3. [中] 空白 tab 上唯一的其它 agent 启动入口被删除，而 launchpad 不可达 → 10 个 CLI 无新开入口**

- 被删入口：基线的空白对话图底部「打开终端」菜单（`D:/work/aicross/src-ui/src/components/center/CenterPanel.tsx:1492-1517`，列出 terminal + 全部已装 AI CLI），现在只剩 `CenterPanel.tsx:1643-1644` 的「系统终端」「对话图」两个次级入口。
- launchpad：`CenterPanel.tsx:797` 的 `isLaunchpadMode` 要求 `tool === null`，但全仓没有创建 tool-null 会话的路径（初始 tab 是 graph：`app-state.tsx:777`；`ADD_TERMINAL`/`SET_TERMINAL_TOOL` 的调用方都不传 null），所以 `CenterPanel.tsx:1769` 那批 pinned agent 卡与 `CenterPanel.tsx:1717` 的“总指挥”固定卡都是死 UI。
- 后果：opencode / mimocode / openclaw / grok / antigravity / qwen / hermes / crush / aider / goose / copilot 从此只能靠历史续聊或外部 `coffee-cli launch --tool` 起，UI 里没有新开会话入口。
- 复现：全新安装后（无历史会话）无法从界面启动 opencode。
- 建议：要么启动器/「+」菜单补一个“更多 agent/已装 CLI”入口，要么在任务书/PRODUCT 里明确这是有意收窄；报告目前完全没提这一条。

**4. [中低] Windows 下 model/provider 未做字符校验，`cmd.exe /c` 层可被 `&` 等元字符拆分（Unix 安全）**

- `src-ui/src/commander/args.ts:69-79`（只 trim，无校验）、`src/server.rs:798-812`（拼进 args）、`src/terminal.rs:826` + spawn 的 Windows 分支（恒定 `COMSPEC /c program args...`）；portable-pty 的 `append_quoted`（`portable-pty-0.9.0/src/cmdbuilder.rs`）只在参数含空格/制表/换行/引号时加引号。
- 事实：Unix 直接把 argv 交给 `CommandBuilder`，无 shell，含空格是单个 argv，`-` 开头也只是单个 argv（由 CLI 决定是否当 flag）；Windows 上 `x&calc` 这种不带空格的串不会加引号，`cmd.exe` 把 `&` 当命令分隔，第二条命令会执行。
- 复现（Windows）：`~/.aicross/commander.json` 写 `"model":"x&calc"`（或 kimi config.toml 别名带 `&`），用一键行启动 → 执行 `calc`。注意这是本机用户自己的配置文件，非越权，但违背“单个 argv、不解释”的契约。
- 另：`trim()` 使“原样传入”不成立（首尾空白被吃掉；全空白时整个 flag 消失）。
- 建议：模型/provider 是标识符，落盘与启动前用 `^[A-Za-z0-9._:/@+-]+$` 之类的白名单校验（或 Windows 侧转义 `&|<>^%!`），并把校验失败显式提示，而不是静默。测试补 `commanderArgs` 对空格/`-`/`&` 的用例。

**5. [低] 模型清单未加载完就显示“资源库里还没有 X 的模型，先去接入页探测”**

- `CommanderLauncher.tsx:212` 的条件是 `modelsError === null && agentModels.length === 0 && agent !== ''`，缺 `modelsLoaded`；`modelsLoaded`（`:57`）只用在 `:121` 清 `picked`。
- 复现：默认/agent 已定、`listDispatchableModels` 在途时，该 note 先出现再被选项替换。
- 建议：条件加 `modelsLoaded`。

**6. [低] 默认值晚到会覆盖用户已做的选择**

- `CommanderLauncher.tsx:89-103`：`seeded` 只防止 effect 重复运行，用户点 agent 卡不会置位；`load_commander` 慢于点击时，seed 会把 agent/模型/目录改成默认。
- 复现：冷盘启动后 300–800ms 内先点 codex，默认 claude 到达 → 选择被改回 claude。
- 建议：用户交互时 `seeded.current = true`。

**7. [低] 测试缺口：关键路径只有“读代码验证”**

- `src/server.rs:798-812` 的“per-launch extra args 追加/空串过滤/与 tools.json extra_args 的先后”没有任何 Rust 测试（报告也承认启动路径只做编译验证）；建议抽成纯函数单测。
- `CommanderLauncher.tsx:196` 用 `JSON.stringify(picked)` 与 `launcher.ts:129` 的 `commanderModelValue` 手工对齐 key 顺序，没有任何测试锁住这两边一致（组件测试从未渲染 `picked !== null` 的 select）。
- `orchestration.rs:1266` 写侧拒 version 只断言 `is_err()`，没断言文件未被替换；可加读回校验。
- `args.ts:44` 的 `commanderAgentOfTool` 生产代码零引用（只有测试用）。

**8. [低/待确认] 参数顺序与任务书字面相反**

- `src/server.rs:798-812` 把本次启动的 `commanderArgs` 放在工具设置 `extra_args` 之后；TASK-10 字面是“先映射，**再追加**工具设置的 extra_args”。按“后者胜”的语义，后果是 tools.json 里手写的 `--model` 会被启动器选的模型覆盖（报告注释里承认这是有意）。需要产品确认优先级；至少在测试里把这个顺序钉死，避免以后被无意翻转。既有 `extra_args` 的语义本身未被改动。

**9. [nit] 细节**

- `CenterPanel.tsx:1717`：launchpad 的“总指挥”卡仍用 `SvgGraph`（对话图）图标，名实不符（该路径当前不可达，所以只是死代码里的不一致）。
- `CenterPanel.tsx:727/1636/1644`：`graphViewTabs` 一旦点了「对话图」就再也回不到同一 tab 的启动器，只能新开 tab。
- `launcher.ts:84-89`：`cwdName('C:\\')` 返回 `C:`，`cwdName('/')` 返回空 → 标题/一键行显示“未选目录”；根目录等边界没有测试。

---

## 核过没问题的要点

- **契约**：`CommanderSettings`（`orchestration.rs:268-301`）字段/版本与 `src-ui/src/commander/types.ts` 逐字段一致（snake_case）；缺 version 按 1 读、version≠1/损坏 JSON/缺字段 → 空默认（`orchestration.rs:520-526`）；写侧拒绝非 1（`:528-536`）；`write_json_atomic` 原子替换；读侧不改文件；`load_commander`/`save_commander` 已在 `server.rs:3983-3984` 注册。
- **`commanderArgs`**：claude `--model`、codex `-m`、kimi `--model`、pi `--provider` + `--model`、空模型不加模型参数、未知 agent 返回 `[]`（`args.ts:69-79`），与任务书四家映射一致；`extra_args` 为空时不发空串。
- **kimi 映射**：资源 id `kimi` ↔ PTY 工具 id `kimicode` 双向表正确（`args.ts:17-45`），资源库模型 agent 确为 `kimi`（`resources.rs:661-744`），`check_tools_installed` 返回的键是 `kimicode`，卡过滤查的就是这个键；`AgentBadge agentId="kimi"` 有对应色相。
- **启动链路**：`launchCommander`（`CenterPanel.tsx:1178-1213`）→ `SET_TERMINAL_TOOL`（`app-state.tsx:477-479`）→ `TierTerminal.tsx:1299` → `tauri.ts:145-146` → `server.rs:846`；`extra_args` 是 `Option<Vec<String>>`，Tauri camelCase→snake_case 与既有参数一致；未发现覆盖/重复：`SET_TERMINAL_TOOL` 每次重设本次启动字段，args 只在 spawn 前追加一次，工具设置 `extra_args` 的原逻辑（`server.rs:786-791`）未动，resume 路径不受影响（总指挥不带 token）。未去 `channels/`、`dispatch.rs`、`terminal.rs`。
- **cwd/标题**：launcher cwd → `SET_FOLDER`/session.folderPath → TierTerminal `folderPath` → Rust `cwd` → `dir` → `spawn_cwd`（非目录回退 home）；标题 `agent · 模型 · 目录名`（`CenterPanel.tsx:1445`，CLI 的 OSC `toolTitle` 优先，报告已声明）。
- **状态/副作用**：保存确实挂在 `tier_terminal_start` resolved 之后（`TierTerminal.tsx:1300`），失败路径不进 `onStarted`；“在此启动总指挥”走 `ADD_TERMINAL` 新 tab，不动当前会话（`CenterPanel.tsx:1195-1208`）；`load_commander` 失败被 `.catch` 兜成空默认，不阻塞（`CenterPanel.tsx:1228-1239`；`Explorer.tsx:564-580`）；`+` 菜单三项齐（`CenterPanel.tsx:1576-1580`）；「系统终端」「对话图」两个次级入口可用，`handleAddTab('graph')` 会置 `graphViewTabs` 直达开始视图。
- **安全**：`commander.json` 只有 version/agent/provider/model/cwd/updated_at，无密钥、无 token；只写 `~/.aicross/commander.json`，不碰 tools.json 或任何全局配置；Unix 无 shell 介入，模型名含空格时是单个 argv。
- **回归**：`list_dispatchable_models` 与 `model-picker` 未改，可派发清单口径不变（启动器仍展示全部已探测模型，含未验证/失败，与 G1 一致）；Explorer 目录行只在多一颗按钮时用 flex，单按钮布局与改前等价；`SET_TERMINAL_TOOL` 相比基线只是多了 extraArgs/commanderTitle/commanderSettings 三个本次启动字段，清 `toolTitle`/`resumeToken` 的既有语义保持；`applyLaunchRequest`、multi-agent/分屏 pane 的 `tierTerminalStart` 调用不受新增可选参数影响。
- **测试**：新增 `commander-10.test.tsx`（四家映射/空模型/未知 agent/工具 id 映射/过滤/显示文本/JSON 值往返/文案/阻断/静态组件）与 Rust `task10_commander_roundtrip_and_missing_field_defaults`，断言是具体字面量，能红；除第 2 条把 `{}` 固化的用例外没发现同义反复（`commanderModelValue` 往返测试偏弱，见问题 7）。
- **说明**：本次为只读审查，无 shell，无法重跑 `cargo check/test`、vitest、`npm run build`；`tasks/` 下也没有本轮（TASK-10）的日志产物，报告里的 324/380 与零 warning 结论无法独立复核。