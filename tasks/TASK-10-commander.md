# TASK-10 控制台与快速启动（09-15 用户提出）

依据 `docs/PRODUCT.md`「控制台与快速启动（09-15 定案）」。前端为主；Rust 只允许新增一个读写 `~/.aicross/commander.json` 的小命令对（`load_commander` / `save_commander`，放 `src/orchestration.rs`，`server.rs` 注册），不动 `src/channels/`、`src/dispatch.rs`、`src/terminal.rs`。

## 契约（只追加）

- `~/.aicross/commander.json`：`{ version: 1, agent: "claude"|"codex"|"kimi"|"pi", provider: string, model: string, cwd: string, updated_at: string }`。
- 启动参数映射（前端纯函数 `commanderArgs(agent, provider, model) -> string[]`）：claude → `["--model", model]`；codex → `["-m", model]`；kimi → `["--model", model]`；pi → `["--provider", provider, "--model", model]`。先追加工具设置的 `extra_args`，再追加 `commanderArgs`；重复旗标以本次启动为准。空模型 → 不加模型参数。

## 实现

1. **启动器**：空白 tab（`isEmptyGraphSession`）改成「控制台」启动器，替代底部「打开终端」折叠菜单：左侧 agent 卡（只列已安装，复用 `AgentBadge` 品牌标志），中间模型下拉（读 `listDispatchableModels` 按 agent 过滤，Claude 显示别名 + 真实 ID，pi 显示 provider/model），右侧工作区目录（默认当前工作区，可选文件夹），主按钮「启动控制台」；下方保留「系统终端」「对话图」两个次级入口。有默认配置时首屏显示一键行「启动控制台 · claude · fable · <目录名>」。
2. **启动**：走现有 `selectTool(tool, toolData, cwd)` / TierTerminal 的 PTY 路径，把 `commanderArgs` 作为该次启动的附加参数传入（找到 TierTerminal/tierTerminalStart 接收额外参数的口子；没有就加一个可选 `extraArgs` 透传，保持既有工具设置 `extra_args` 语义不变）；tab 标题 `agent · 模型 · 目录名`。
3. **默认记住**：启动成功即 `save_commander`；应用启动 `load_commander` 填充一键行；工作区 tab 目录行加「在此启动控制台」按钮（用默认 agent/模型，目录换当前）。
4. **「+」菜单**：保留 系统终端 / 对话图 / 控制台（新）。对话图不再是默认新 tab。
5. **测试**：`commanderArgs` 四家映射 + 空模型；启动器纯函数（可用 agent 过滤、模型清单按 agent 过滤与显示文本、默认行文案）；Rust `commander.json` 往返与缺字段默认。
6. `docs/MANUAL.md` 加「控制台」一节（怎么起、模型从哪来、原生命令照用、和对话图的关系）。

验证：`cargo check` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。保持文件原有换行；不起窗口、不发真机请求、不 commit、不 push。报告写 `tasks/REPORT-10.md`。
