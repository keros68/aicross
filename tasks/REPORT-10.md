# REPORT-10 总指挥与快速启动

对应 `tasks/TASK-10-commander.md`，接管并复核上一位实现者留下的未提交改动后完成。

## 实现

1. 启动器
   - 空白对话图 tab 改为总指挥启动器，保留「系统终端」「对话图」两个次级入口。
   - agent 卡只显示探测为已安装的 Claude Code、Codex、Kimi Code CLI、pi，并复用 `AgentBadge`。
   - 模型从 `list_dispatchable_models` 读取并按 agent 过滤；Claude 显示别名与已核对的真实 ID，pi 显示 `provider / model`。
   - 工作区默认取当前 tab 目录，也可重新选择。有效的已保存配置会显示一键启动行；默认 agent 已卸载时不显示。

2. 启动
   - `commanderArgs` 实现四家参数映射和空模型行为；Kimi 的资源 id `kimi` 映射到 PTY 工具 id `kimicode`。
   - 本次启动参数经 `TerminalSession.extraArgs`、`TierTerminal` 和 `tier_terminal_start` 传入，追加在既有工具设置 `extra_args` 后，不改变常驻设置。
   - tab 标题使用 `agent · 模型 · 目录名`；CLI 通过 OSC 设置标题后仍以 CLI 标题为准。

3. 默认记住
   - Rust 新增 `load_commander` / `save_commander`，读写 `~/.aicross/commander.json`，版本为 1；缺字段使用默认值，错误内容和非 1 版本按空默认读取，写入保持原子替换。
   - 默认配置在 `tier_terminal_start` 成功返回后保存，不在点击启动时提前保存。
   - 工作区目录行新增「在此启动总指挥」；它用默认 agent/模型和当前目录新开 tab，不覆盖当前会话。

4. 「+」菜单
   - 标题栏「+」改为三项菜单：系统终端、对话图、总指挥。
   - 新建总指挥 tab 使用启动器；对话图只有显式选择后才进入开始视图。

5. 测试
   - 前端 `TASK-10 (1) commanderArgs 四家映射 + 空模型`：覆盖 Claude、Codex、Kimi、pi、空模型、未知 agent 和 Kimi 工具 id 映射。
   - 前端 `TASK-10 (2) 启动器纯函数`：覆盖可用 agent 过滤、模型按 agent 过滤、Claude/pi 显示文本、模型值往返、目录名、默认行、tab 标题和启动阻断文案。
   - 前端 `TASK-10 (3) 启动器组件`：覆盖无已安装 agent、agent 卡、两个次级入口、无默认以及默认 agent 已卸载。
   - Rust `orchestration::tests::task10_commander_roundtrip_and_missing_field_defaults`：覆盖文件缺失、完整往返、字段缺失、version 缺失、错误版本、损坏 JSON、非 1 版本写入拒绝。

6. 用户手册
   - `docs/MANUAL.md` 新增「总指挥」一节，说明启动方式、模型来源、原生命令行为、默认配置和对话图的分工。

## 验证

- `cargo check --all-targets`：通过；`Finished dev profile`，零 warning。
- `cargo test`：通过；324 passed，0 failed，11 ignored。
- `cd src-ui && npx vitest run --configLoader native --pool threads`：通过；27 个测试文件、380 个测试全部通过。
- `cd src-ui && npm run build`：通过；TypeScript 编译通过，Vite 处理 332 个模块并完成生产构建。

## 没做 / 部分做

- 未启动桌面窗口，未做视觉真机检查；任务书明确要求不起窗口。
- 未发任何真实 agent 或模型请求；参数映射和启动路径只做编译与自动测试验证。
- 未实现产品说明中的 Later（给总指挥安装 ai-cross skill 并同步对话图）；不在 TASK-10 范围内。
- 未改 `src/channels/`、`src/dispatch.rs`、`src/terminal.rs`，未运行 `cargo fmt`，未 commit、未 push。
