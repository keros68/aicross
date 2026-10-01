# TASK-44 崩溃后核对命令 `aicross recover`（10-01 定案）

**前置：TASK-43 已合并。只改 `src/console_cli.rs`、`src/engine/mod.rs`、`src/dispatch.rs`（仅把现有回滚函数开放给引擎调用）、`docs/CONSOLE-CHANNEL.md`、`docs/MANUAL.md`。不 commit，不 push。**

## 背景

`docs/PRODUCT.md`「10-01 定案」：后台引擎意外退出后（run 被判「引擎进程已退出」或「引擎进程未能启动」），图上可能留下停在 `pending` 的节点，单写者节点可能把项目文件改了一半；窗口里原来的「崩溃后核对」已随 TASK-40 下线，命令行没有替代。现有可复用：`src/dispatch.rs` 的 `rollback_node_changes_core(project, run_id, node_id)`（按 `.dispatch/<run>/<node>.before/` 恢复派发前快照、删掉新建文件），引擎的 `read_run`（含死进程判定）、落图合并写。

## 命令

- `aicross recover <run_id>`：只读核对。输出 JSON：run 状态与原因；停在半路的节点（`pending`，以及 run 判失败时仍为 `draft` 的本 run 节点）各给 `node_id`、角色、`agent/model`、写权限（只读 / 单写者 / worktree）；单写者节点列出相对派发前快照有变化的文件（新增 / 修改 / 删除）与快照目录；worktree 节点给 worktree 路径（原样保留，不动）。run 仍在运行（进程活着）时退出 1，提示「run 仍在运行，等它结束或先 aicross cancel」。run 已是 `done` 时退出 0 并说明「无需核对」。
- `aicross recover <run_id> --rollback`：在上面的基础上，对每个停在半路的单写者节点调用现有回滚函数恢复项目文件；把这些停在半路的节点在图上标为 `failed`、正文「引擎中断，已回滚项目改动」（只读节点与 worktree 节点标 `failed`、正文「引擎中断」，不动文件），经合并写落图；刷新 run 摘要（状态保持 `failed`、原因追加「已核对」）。逐节点输出回滚结果；任一节点回滚失败时继续处理其余节点，最后退出 1 并列出失败项。
- 没有快照目录的单写者节点：不回滚，标 `failed`、正文「引擎中断，无派发前快照，项目改动未回滚，请自行核对」，并在输出里单列。

退出码：0 成功 / 1 业务失败（仍在运行、回滚有失败）/ 2 参数错 / 3 引擎不可用。

## 测试

全部用临时目录与临时 git 项目，不起真进程：
- 死进程的 run：只读核对列出 pending 单写者节点与变化文件。
- `--rollback`：文件恢复到快照、新建文件被删、节点标 `failed`、run 摘要刷新；worktree 节点与只读节点不动文件。
- 无快照的单写者节点单列且不回滚。
- 仍在运行的 run 退出 1；`done` 的 run 退出 0「无需核对」；不存在的 run 退出 1；参数错退出 2。
- 新测试在修复临时改回时必须变红。

## 文档

`docs/CONSOLE-CHANNEL.md`、`docs/MANUAL.md`：加 `recover` 一节；把「进程中断后图上的 pending 节点要开窗口对账」一类过时说法改为用 `aicross recover`；`STATE.md` 的说明同步（若本命令不读它，照旧写「引擎写入、当前无命令读取」）。

## 验证

```
cargo check --all-targets        # 零 warning
cargo check --release
cargo test
```

每个改动文件保持原有换行。写 `tasks/REPORT-44.md`。

## 编排者真机核

真派一次单写者执行，在执行中杀掉后台引擎，`aicross recover` 看核对结果，`--rollback` 后核对项目文件已恢复。
