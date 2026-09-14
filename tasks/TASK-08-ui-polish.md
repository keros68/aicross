# TASK-08 界面细节修整 + Claude 模型清单

工作树 `D:\work\aicross-ui`，分支 `ui/polish`（从 main `0f511fd` 起）。前端在 `src-ui/`（React + TS，样式在各组件同目录 `.css` 与 `src-ui/src/styles/global.css`），Rust 在根目录 `src/`。不动 `src/dispatch.rs`、`src/channels/`、`src-ui/src/graph/` 的逻辑；`src/resources.rs` 只改第 3 条要求的部分。不要 commit，不要 push，不起 Tauri 窗口。

## 用户 09-14 指出的三处（截图）

1. **图顶栏「可派 N 路 ｜ 派发」**：「派发」按钮里的文字位置不对（文字相对按钮框偏上/偏下，和左边「可派 0 路」不在一条基线上）。对应 `src-ui/src/components/center/graph/GraphTab.tsx` 的 `.graph-primary-btn` 与其容器（`GraphTab.css`）。
2. **右栏「方案与编排」头部**：「方案与编排 / 任务」两个 tab、预设下拉、「应用 / 编辑编排 / 讲解」三个按钮之间的上下间距不对（挤在一起、没有节奏）。对应 `src-ui/src/components/right/orchestration/OrchestrationPanel.tsx` + `OrchestrationPanel.css`，以及右栏 tab 的样式来源。
3. **接入资源页 Claude Code 卡片没有 fable**：现在 `src/resources.rs` `claude_presets()` 硬编码 `["opus","sonnet","haiku"]`，而 `claude --help` 的 `--model` 说明里别名是 `'fable', 'opus', or 'sonnet'`（本机 2.1.270）。改法：探测时运行 `claude --help`，用正则从 `--model` 一段抓所有 `'xxx'` 单引号别名，加上 `haiku`；抓不到就退回静态表 `["fable","opus","sonnet","haiku"]`；`source` 写成 `claude --help 别名 (时间)` 或 `静态表(2026-09-14)`。冒烟不用改（已按响应 `model` 字段核对真身，`identity.contains(model.id)` 对 `fable`→`claude-fable-5-1` 成立）。补 Rust 测试：给定一段 `--help` 文本能抓出 fable/opus/sonnet；空文本退回静态表。

## 全面过一遍界面

用已安装的 skill `web-design-guidelines`（逐条检查清单）和 `design-taste-frontend`（审美取舍）把整个前端过一遍：对话图起始页与图页、G1 派发浮层、结果屏、左栏三个 tab、右栏两个 tab、接入资源页、设置各页。重点是**间距节奏、对齐、字号层级、按钮内文字垂直居中、暗/亮两套主题下的对比度**。原则：

- 只修样式与布局，不改交互逻辑、不改文案、不加功能。
- 沿用现有 token（`--bg-*`、`--text-*`、`--accent` 等在 `global.css`），不引入新色。
- 继续「去 coffee 味」的方向（第 6 期已做第一刀），但别推翻既有布局。
- 每处改动在报告里写「哪里、原来怎样、改成怎样、为什么」，按截图三处 + 其余发现两组列。

## 验证

`cd src-ui && npx vitest run`、`cd src-ui && npm run build`、根目录 `cargo test resources`（第 3 条）。命令与输出原文贴进 `tasks/REPORT-08.md`。报告末尾列「没改的」（你看到但觉得该留给编排者决定的）。
