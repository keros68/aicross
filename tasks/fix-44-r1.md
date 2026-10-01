# fix-44 R1：recover 的修正（10-01 复审：Claude 子代理 + Antigravity）

在本工作树现有改动上继续修，不重写。**本轮允许改 `src/dispatch.rs`（第 1 条需要）、`src/engine/mod.rs`、`src/console_cli.rs`、`docs/CONSOLE-CHANNEL.md`、`docs/MANUAL.md`、`tasks/REPORT-44.md`。不 commit，不 push。**

## 1. 【高】执行中途被杀的单写者节点没有可回滚的快照（任务书前提不成立）

编排者已核：`run_one_attempt` 在执行**结束后**才调 `persist_before_snapshot_with_note` 把 `.before/` 写盘，且只存改动过的文件（`dispatch.rs` 约 2818 行、1109 行）。执行中途引擎被杀时磁盘上没有快照，`recover --rollback` 无从恢复；反过来，有 `.before/` 却仍是 `pending` 的节点其实已执行完、只是结果没写回图。

要求：
- 单写者（非 worktree、非只读）执行在**拉起 CLI 之前**，把内存里已有的派发前快照（`before_snapshot`）落盘为「执行中快照」：与 `.before` 同一根目录规则（`latest_before_dir` / `persist_before_snapshot_with_note` 用的那套），目录名 `<node>.inflight/`，内含 `manifest.json`（每个文件：相对路径、大小、是否存了内容）与 `files/`（有内容的文件原样存）。沿用现有快照的排除目录、链接跳过、单文件 8 MB / 总 128 MB 上限；超限文件只进清单不存内容。
- 该次执行结束（任何结果，含失败、取消、超时）后删除 `.inflight/`；原有的 `.before/` 持久化逻辑不变。落盘失败不阻断派发，在 runner_note 里记「执行中快照保存失败：…，引擎中断后无法自动回滚」。
- `recover` 按磁盘状态区分停在半路的单写者节点：
  - 有 `.inflight/`：状态「执行中断」。变化文件 = 项目当前内容对比清单：清单里有、项目里没有 → `removed`；内容不同（存了内容的比内容，没存内容的比大小）→ `modified`；项目里有、清单里没有（同一套排除规则）→ `added`。`--rollback`：恢复存了内容的文件、删除 `added`、没存内容且有变化的文件列为「无法恢复」（不算失败，但在输出与节点正文里写明）；完成后删除 `.inflight/`。
  - 没有 `.inflight/` 但有 `.before/`：状态「执行已结束，结果未写回」。新增文件取快照里的 `.created` 记录，**不得**把「项目里有、快照里没有」都算新增（`.before/` 只存改动过的文件）。`--rollback` 照旧调 `rollback_node_changes_core`。
  - 都没有：节点是 `draft`（从未发出）→ 状态「未发出」，不查文件；`pending` → 「无派发前快照」单列，正文照任务书。
- 测试：`.inflight` 在拉起前存在、结束后被删（可用会写一个标记文件并读取 `.inflight` 是否存在的桩命令）；执行中断的核对与回滚（含 `added` 被删、超限文件列为无法恢复）；「执行已结束，结果未写回」时项目里未改动的文件不被报成 `added`。

## 2. 【高】`pending` 节点没有限定在本 run

`recover` 的筛选里 `status == "pending"` 没限定 `run_node_ids`；同图别的 run 的节点会被卷入，`--rollback` 时被回滚、标 `failed`。`pending` 与 `draft` 都只认本 run 的节点。测试：图上放一个不属于本 run 的 `pending` 单写者节点，核对与回滚都不碰它。

## 3. 【中】回滚失败时节点正文仍写「已回滚」

`rollback_node_changes_core` 返回 Err 时正文改为「引擎中断，回滚失败：<原因>，请自行核对」，并进 `failures`、退出 1。测试：构造回滚失败（例如快照目录是文件而不是目录，或其它能稳定触发 Err 的方式），断言继续处理其余节点、正文、`failures`、退出 1。

## 4. 【中】标 `failed` 时整份 reply 被覆写

只改 `status`、`text`、`error_kind`，保留原 reply 的其余字段（`artifacts`、worktree 路径等）。测试钉住 worktree 节点标失败后路径仍在。

## 5. 【中】worktree 节点中途中断时路径为空

图上没有产物时按 `dispatch.rs` 创建 worktree 的固定规则推算路径（找到创建处，复用同一规则；路径不存在时写明「worktree 不存在」）。

## 6. 【中】`--rollback` 且没有停在半路的节点时没追加「已核对」

run 判 `failed` 时，`--rollback` 无论有没有节点要处理，都刷新 run 摘要并追加「已核对」（防重复追加保留）。

## 7. 【中】命令行把 `--rollback` 当成 run_id

`aicross recover --rollback`（缺 run_id）退出 2「缺少 run_id」：第一个位置参数以 `--` 开头时按缺 run_id 处理。补测试。

## 8. 【低】输出顺序

`nodes` 按 run 摘要里的节点顺序输出（不依赖 HashMap 顺序）；测试按 node_id 查找而不是按下标。

## 9. 【低】文档

`docs/CONSOLE-CHANNEL.md` 里「核对不改 run」改为如实描述：只读核对不改项目文件与图；读取时若判定引擎进程已退出，会把 run 状态更新为失败（与 `status` 相同）。补「执行中快照」一句：单写者执行期间项目目录的 `.dispatch/` 下会有 `<node>.inflight/`，执行结束即删除。

## 验证

```
cargo check --all-targets        # 零 warning
cargo check --release
cargo test
```

每条修复的新测试在该修复临时改回时必须变红，逐条做回退验证并记进报告（不是只回退一处）。每个改动文件保持原有换行。在 `tasks/REPORT-44.md` 末尾追加「R1」一节：逐条改动、测试名、回退验证结果、三条命令输出、未做与理由。
