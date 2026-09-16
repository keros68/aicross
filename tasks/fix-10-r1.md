# fix-10-r1 控制台与快速启动（TASK-10）复审修正轮

依据 GLM-5.3-Flash 与 DeepSeek 对 phase/10 的盲审（`tasks/review-10-glm.md`、`tasks/review-10-ds.md`）。编排者已定的取舍写在各条里。仍不动 `src/channels/`、`src/dispatch.rs`、`src/terminal.rs`。

## 必修

1. **旧控制台 tab 重启回写过期默认**（两家共同）：`CenterPanel.tsx:1667` 的 `onStarted` 每次挂载都 `saveCommander(t.commanderSettings)`，换目录/重启会把旧快照写回。修法：首次保存成功后清掉该 tab 的 `commanderSettings`（加一个消费 action），`RESTART_TERMINAL` 不再触发保存。测试：reducer 层「保存后清空」+ 组件层「重启不再调 saveCommander」。
2. **其它已装 CLI 失去新开入口**（DeepSeek 3）：基线空白 tab 底部「打开终端」菜单列出全部已装 AI CLI，现在只剩 系统终端/对话图 两个次级入口，opencode/qwen/aider/goose 等十一家没有界面入口（launchpad 路径 `tool === null` 实际不可达）。修法：启动器次级入口加第三项「其它 CLI」，展开列出已装且不属于四家控制台的工具（沿用原菜单的数据源与点击路径，起 PTY 不带模型参数）；不改「+」菜单。测试：已装 opencode 时次级入口出现且点击走 `SET_TERMINAL_TOOL`。
3. **首屏假空态**（两家共同）：`toolsInstalled` 初值 `{}` 被当「没装」，冷启动先闪「本机没探测到…」且一键行消失。修法：CenterPanel 传入「已扫描」信号（首次探测返回后为 true），未扫描时启动器显示「探测中…」占位、不出空态也不出一键行；`availableCommanderAgents` 保持只认 `=== true`。把 `commander-10.test.tsx:218-221` 那条用 `render({})` 断「没探测到」的用例改成「未扫描 → 探测中；已扫描且空 → 没探测到」。
4. **模型/provider 白名单**（DeepSeek 4）：Windows PTY 经 `cmd.exe /c` 起进程，不带空格的 `x&calc` 不会被加引号。修法：`commanderArgs` 与 `saveCommander` 前用 `^[A-Za-z0-9._:/@+-]+$` 校验 model 与 provider（去掉 trim 语义：不合规就整个拒绝，不静默吃字符），启动器上显式提示「模型 id 含非法字符」；Rust `save_commander` 同样校验拒写。测试：空格、`&`、`-` 开头、纯空白各一条。
5. **追加顺序定案**（GLM 2 / DeepSeek 8）：保持实现（工具设置 `extra_args` 在前、本次启动参数最后，重复旗标以本次启动为准），改文档口径：`tasks/TASK-10-commander.md` 契约那句改成「先追加工具设置的 extra_args，再追加 commanderArgs；重复旗标以本次启动为准」，`docs/PRODUCT.md`「控制台与快速启动」一节里「工具设置里的额外参数照旧追加」后补半句「本次选的模型排在最后、优先生效」（PRODUCT.md 是 LF，只改这一句）。把 `server.rs` 的参数组装抽成纯函数并加 Rust 测试钉住顺序与空串过滤。
6. **空白启动器 tab 的标题**（GLM 3）：「+」→ 控制台 新建的 tab 标题显示「对话图」。修法：未进对话图（不在 `graphViewTabs`）且 `toolData` 为空的 tab 标题为「控制台」；显式选了对话图的仍是「对话图」。测试：两种 tab 各一条。
7. **目录行「在此启动控制台」不查安装**（GLM 4）：与启动器一键行同口径，默认 agent 未安装则不显示按钮（读同一份 `toolsInstalled`/扫描信号）。
8. **启动器小项**（DeepSeek 5/6/9）：「资源库里还没有 X 的模型」加 `modelsLoaded` 门；用户点过 agent 卡后 `seeded` 置位，晚到的默认不覆盖选择；`cwdName` 对 `C:\`、`/` 等根目录返回盘符或 `/` 而不是空串，补边界测试。
9. **测试补课**（DeepSeek 7）：`picked` 的 `JSON.stringify` 与 `commanderModelValue` 的 key 顺序用一条往返测试锁住（渲染 `picked !== null` 的 select）；`commanderAgentOfTool` 生产代码零引用——要么在标题/重启路径用上，要么删掉；`orchestration.rs:1266` 写侧拒 version 补读回断言文件未变。
10. **`save_commander_in` 与 `save_premises_in` 同口径**（GLM 7）：损坏的 commander.json 拒写而不是静默覆盖（读侧仍按空默认）。

## 不改（记录）

- 标题目录名与实际 spawn 目录可能错位（目录被删时回退 home，OSC7 后纠正）：接受。
- Windows `cmd.exe` 重解析是既有暴露面，第 4 条的白名单已把控制台这条路封住；tools.json `extra_args` 是用户自填自由文本，本期不动。

验证：`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。`src-ui/src/tauri.ts` 混合换行只做字节级最小改动；其它文件保持原有换行；不 `cargo fmt`；不起窗口、不发真机请求；不 commit、不 push。报告写 `tasks/REPORT-10-R1.md`（逐条对应本清单）。
