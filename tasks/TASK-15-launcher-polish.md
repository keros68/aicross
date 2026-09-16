# TASK-15 控制台启动器与 agent 名称统一（09-15 用户反馈）

用户看图（控制台启动器）指出四点：agent 卡大小不一很奇怪；「Kimi Code CLI」不必带 CLI，「pi」两个字母太少叫「pi agent」更合适；pi 的图标不对；参考 coffee 设置页的形态优化；以及「控制台进去是不是就该直接是原生 CLI 的状态，在里面自己选模型、思考强度、切模型」。

## 已定（编排者按用户意图定）

1. **有默认就直接起**：新开控制台 tab 时若 `~/.aicross/commander.json` 有效且默认 agent 已安装，直接起原生会话，不再停在启动器；终端上方一条细行显示 `claude · fable · aicross-demo · 更改`，点「更改」回到启动器。没有默认、默认 agent 未安装或探测未完成时才显示启动器。模型与 thinking 档在会话里用 CLI 自己的方式改（claude `/model`、codex `/model`、kimi `/model`、pi `/model`），控制台只传首次启动的 `--model`；手册写清这一点。
2. **agent 显示名一处定义、全局一致**：Claude Code / Codex / Kimi Code / pi agent。Rust 资源库的 agent 显示名（`src/resources.rs` NATIVE_AGENTS 与 pi 条目）、前端 `AgentBadge` / 启动器 / 左栏 / 接入页 / 工具设置 都从同一张表取；机器 id（claude / codex / kimi / kimicode / pi）不变。
3. **agent 卡等宽等高**：启动器的 agent 卡改成定宽网格（四张同尺寸，图标 + 名称左对齐，选中态用边框色不用变宽），窗口窄时换行仍等宽。形态参考 `D:\work\coffeecli` 设置页的工具卡（只借形态，不复制代码）。
4. **pi 的标志**：现在的「π」是占位。先查 pi 的安装包（`pi --help` 所在目录、npm 包目录）里有没有官方图标；有就按 TASK-09b 的方式内联（记录来源与许可到 `NOTICE`），没有就退回 TASK-09 的字母徽标「pi」（两字母、同色相），不用假标志。

## 实现

- `CommanderLauncher.tsx` / `commander/launcher.ts`：卡片网格与显示名；`CenterPanel.tsx`：新空白 tab 的自动启动判定（复用 `commanderDefaultAvailable`）与终端上方细行 + 「更改」；`AgentBadge.tsx` / `brand-marks.ts`：pi 标志；`src/resources.rs` 与前端显示名表；`docs/MANUAL.md`「控制台」一节补「模型与 thinking 在会话里改」与自动启动。
- 文案遵守 `AGENTS.md`「界面文案与命名规范」；改名后跑一遍 `scripts/copy-audit.py`。
- 测试：显示名表两端一致（Rust 测试 + vitest 读同一事实）；自动启动判定纯函数（有默认且已安装 → 直接起；否则启动器）；卡片等宽的 CSS 断言（grid 模板）；pi 标志来源测试或字母徽标回退测试。

验证：`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。保持文件原有换行（`src-ui/src/tauri.ts` 混合只做字节级最小改动）；不 `cargo fmt`；不起窗口、不发真机请求；不 commit、不 push。报告写 `tasks/REPORT-15.md`。
