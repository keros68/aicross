# TASK-37 查看器第一版：一次 run 一页，看 + 采纳（10-01）

**前置：TASK-36 合并到 main 之后再开工**（本期读 TASK-36 产出的 run 文件、调用它的 `adopt`，并改同一批命令行入口）。

**允许改：** `src/**`（新增命令与 Tauri 命令）、`src-ui/src/**` 中新增的查看器文件与入口接线、`docs/CONSOLE-CHANNEL.md`、`docs/MANUAL.md` 新增一节。**不删**现有窗口界面（下线是「顺序」第 4 步）。不 commit，不 push。

## 背景

`docs/PRODUCT.md`「机制」第 5 条与「待验证 3」（10-01 用户定）：G3 在查看器里做；沿用现有 Tauri 窗口；第一版只做「看 + 采纳」，重派与放弃仍在宿主对话里由 skill 调命令行。

## 1. 打开方式

- 新增命令 `aicross open <run_id>`：窗口没开时启动窗口并直接进入该 run 的查看页；窗口已开时把请求转给已开的窗口，切到查看页。照现有 `launch` 的做法：冷启动经 `AppState::pending_launch` 与 `take_pending_launch`，热启动经单实例转发与 `launch-request` 事件（`src/launch.rs`、`src/main.rs`、`src-ui/src/components/center/CenterPanel.tsx`）。`open` 的参数解析与 `launch` 并列，不改 `launch` 的现有行为。
- `run_id` 先过 TASK-36 的 `valid_id`；run 文件不存在时查看页显示「找不到这次派发」，命令行退出 1。
- 打开查看页时不派发、不改任何文件。

## 2. 数据

- 新增 Tauri 命令 `load_run_view(run_id)`：读 `~/.aicross/runs/<run_id>.json`（经 TASK-36 的 `read_run`，会顺带完成死进程判定）与该 run 的图文件，返回 run 摘要 + 本 run 涉及的节点（按 run 摘要里的节点顺序）。不读 `.dispatch/` 原文；节点上已有的 `reply`（正文、产物 diff、验证输出、模型真身、token、耗时、备选、跳过原因）足够。
- run 状态为 `running` 时，查看页每 2 秒重新调用一次；转为终态后停止。

## 3. 页面

一页，自上而下：

1. **页头**：模式名（「X 模式」）、run 状态、开始时间、总耗时、失败或拒绝原因；遗留条目（若有）。
2. **按阶段分组，组内各路并排**（最多 3 列，超出横向滚动）。每路一张卡：
   - 主标题「角色 · 任务」，次标题 `agent/model`（借 Omnigent 第 3 条：任务名取蜂群条目标题，否则取图标题，都没有才退回 `agent/model`）；厂商色点。
   - 请求的模型与实际回答的模型（不一致时标出；拿不到真身标「未报告」，不回填请求值）。
   - 状态（`quota_exhausted` 显示恢复时间）、token（输入 / 输出 / 缓存）、耗时、备选切换说明、跳过原因。
   - 验收节点显示结论（PASS / NEEDS_FIX / 未判定）。
   - 正文（Markdown）；产物 diff（等宽、可折叠）；worktree 节点显示「采纳前请手动合并：<路径>」。
3. **动作**：每张有结果的卡上一个「采纳 / 取消采纳」，调用 TASK-36 的 `adopt`（经其合并写，不整写 run 文件）。没有其它按钮。

能复用的现成部件：`src-ui/src/console-run-detail.ts`（请求模型与实际模型、token、耗时、验收结论的取值与文案）、`src-ui/src/components/center/graph/DispatchOverlay.tsx` 结果屏里的产物与 worktree 展示。复用方式是抽出纯函数或小组件，不改原组件行为。视觉遵守 PRODUCT.md「视觉纪律（查看器）」。

## 4. 契约（只追加）

- 命令行：`aicross open <run_id>`；退出码 0 已打开 / 1 找不到 run / 2 参数错 / 3 窗口起不来。
- Tauri 命令：`load_run_view`、`adopt_run_node`（后者包 TASK-36 的 `adopt`）。
- `docs/CONSOLE-CHANNEL.md` 补 `open` 一条；`docs/MANUAL.md` 加「查看器」一节，写打开方式与页面上每块是什么。

## 5. 测试

- Rust：`open` 参数解析（含 `--flag=value` 形式与非法 run id）；`load_run_view` 对 running / done / failed / 找不到 四种 run 的返回；`adopt_run_node` 经合并写、不动 run 文件。
- 前端：页面数据组装做成纯函数测（分组与列顺序、标题取名顺序、真身显示规则、quota 显示、验收结论）；轮询在终态停止。
- 不为通过测试改 TASK-36 的引擎行为；发现引擎问题写进报告。

## 6. 验证命令

```
cargo check --all-targets        # 零 warning
cargo test
cd src-ui && npx vitest run --configLoader native --pool threads
cd src-ui && npm run build
```

换行：每个改动文件保持原有换行；新文件 LF。写报告 `tasks/REPORT-37.md`：改动文件、复用了哪些现成部件、测试名、四条命令实际输出、未做与理由。

## 编排者真机核（实现者不做）

用 `aicross dispatch` 真派一次经济模式（窗口关闭）→ `aicross open <run_id>` 冷启动打开 → 运行中看到逐阶段刷新 → 结束后采纳一路 → 关掉窗口再 `open` 同一个 run，仍显示已采纳；窗口已开时再 `open` 另一个 run，确认切到新页。
