# REPORT-44 — 崩溃后核对命令 `aicross recover`

按 `tasks/TASK-44-recover.md` 实现。分支 `engine/recover44`（worktree），未 commit、未 push。

## 改了什么

### src/dispatch.rs（仅开放现有回滚函数，零逻辑改动）

- `rollback_node_changes_core`、`latest_before_dir` 加 `pub(crate)`，附注释说明只开放调用给 TASK-44。后者是回滚函数内部找快照的那一个，开放它才能让核对报告与回滚认同一个快照目录（取舍见末节）。

### src/engine/mod.rs（核对与回滚的实现）

- 新增 `recover(root, run_id, rollback) -> RecoverOutcome`：`report` 写 stdout（JSON），`message` 写 stderr，`exit` 为退出码。流程：
  - `read_run`（含死进程判定，判定本身落盘，属既有行为）。仍在 `running` 时退出 1，提示「run 仍在运行，等它结束或先 aicross cancel」；`done` 退出 0，说明「无需核对」。
  - 停在半路的节点：图上 `pending` 的节点，加上 run 判 `failed` 时本 run（run 摘要记录的节点）仍为 `draft`（无 reply 或 reply 为 draft）的节点，按图插入序输出 `node_id`、角色、agent、模型、写权限。
  - 单写者节点：按回滚同一套规则找快照（`latest_before_dir`，落到 `project/.dispatch/<run>/<node>.before`）；快照存在时列出相对快照有变化的文件（`changed_files` 的 `added`/`modified`/`removed`，大小相同再比内容）与 `before_dir`；不存在时进 `no_snapshot` 单列。
  - worktree 节点：输出 `worktree` 路径（取 reply 产物），不动。
  - `--rollback`：单写者节点调 `dispatch::rollback_node_changes_core` 恢复项目文件，逐节点结果放 `nodes[].rollback`；全部停在半路的节点标 `failed`（单写者「引擎中断，已回滚项目改动」、只读与 worktree「引擎中断」、无快照「引擎中断，无派发前快照，项目改动未回滚，请自行核对」），经 `merge_save_graph` 合并写落图；run 摘要刷新（节点摘要按新图重算，状态保持 `failed`，原因追加「已核对」，重复执行不重复追加）。某节点回滚返回 Err 时继续其余节点，最后退出 1，`failures` 列出失败项。
- 新增私有辅助：`recover_safe_name`（与 dispatch.rs `safe_name` 同规则）、快照与项目的文件清单遍历、`changed_files_against_snapshot`。排除目录清单（.git/.dispatch/node_modules/target…）与链接、junction 判定复刻自 dispatch.rs 的快照口径，注释注明镜像关系。
- 新增 `#[cfg(test)] mod recover_cmd`（与既有 `accept_criteria_run_file` 同款内联测试模块）。

### src/console_cli.rs（recover 子命令入口）

- `run_if_subcommand` 识别 `recover`；`run_at_with` 路由到新增 `run_recover`。
- 参数：第 2 个位置参数为 run_id（缺则退出 2「缺少 run_id」）；其后只认 `--rollback`，其余参数退出 2「不认识的参数」。引擎 `recover` 的结果 JSON 写 stdout、一行说明写 stderr，退出码透传；引擎返回 Err（读 run、读图、落图失败）写 stderr 退出 1。
- 测试模块新增 4 个用例（见下表）。

### docs/CONSOLE-CHANNEL.md、docs/MANUAL.md

- CONSOLE-CHANNEL：开头命令清单加「核对 `aicross recover`」；死进程判定一段尾接「之后的核对与恢复见下一节」；新增「崩溃后核对」一节（只读输出、`--rollback` 行为、无快照单列、退出码、快照只存普通文件的边界）。
- MANUAL：命令表加一行；「派发由后台引擎进程执行」一段接 recover 指引；新增「崩溃后核对」一节；术语表加「崩溃后核对与恢复」一行。
- STATE.md 说明照旧（「引擎仍会写入，当前没有命令读取」）：`recover` 读 run 文件、图与派发前快照目录，不读 STATE.md。两份文档里没有找到「进程中断后要开窗口对账」的原句（应已随 TASK-40 清掉；类似说法在 PRODUCT.md，不在本任务允许清单内），已由新章节替代。

## 新增测试

| 文件 | 测试名 | 覆盖 |
|---|---|---|
| src/engine/mod.rs | `recover_cmd::read_only_check_lists_stuck_nodes_and_changed_files` | 死进程 run（pid u32::MAX）：只读核对列出 pending 单写者节点的 `added`/`modified`/`removed` 与 `before_dir`、worktree 节点的路径、无快照节点单列；图不被修改 |
| src/engine/mod.rs | `recover_cmd::rollback_restores_files_marks_nodes_and_refreshes_run` | `--rollback`：文件恢复到快照、新建文件被删、被删文件找回；无快照节点不回滚、文件保留；worktree 原样；节点标 `failed` 与各自正文；run 状态保持 `failed`、原因追加「已核对」、节点摘要重算 |
| src/engine/mod.rs | `recover_cmd::still_running_run_exits_1_without_checking` | 存活进程（当前进程 pid）：退出 1、提示仍在运行；图与项目文件不动 |
| src/engine/mod.rs | `recover_cmd::done_run_needs_no_recover` | `done` 退出 0「无需核对」 |
| src/engine/mod.rs | `recover_cmd::missing_run_is_an_error` | 不存在的 run 返回 Err（命令行退出 1） |
| src/console_cli.rs | `recover_requires_run_id_and_rejects_unknown_args` | 缺 run_id、未知参数，退出 2 |
| src/console_cli.rs | `recover_missing_run_exits_1` | 不存在的 run 退出 1，stderr「读取 run 失败」 |
| src/console_cli.rs | `recover_running_run_exits_1_with_hint` | 仍在运行退出 1，stderr 含「仍在运行」 |
| src/console_cli.rs | `recover_done_run_exits_0_with_no_need_note` | `done` 带 `--rollback` 退出 0，stderr「无需核对」 |

测试全部用临时目录，不起真进程（死进程用 pid `u32::MAX`，存活进程用当前进程自己的 pid）。用普通临时目录而非临时 git 项目：recover 链路（读 run、读图、按快照目录回滚）不调 git，与 engine 既有测试约定一致，也避免测试依赖外部进程。

## 回退验证

临时把 `recover` 开头加一行 `let rollback = rollback && false;`（回退点：核对命令的 `--rollback` 行为整体失效，只剩只读核对），其余不动：

```
$ cargo test recover
test engine::recover_cmd::rollback_restores_files_marks_nodes_and_refreshes_run ... FAILED
thread 'engine::recover_cmd::rollback_restores_files_marks_nodes_and_refreshes_run' panicked at src\engine\mod.rs:3627:9:
assertion `left == right` failed
  left: "after"
 right: "before"
test result: FAILED. 10 passed; 1 failed; 0 ignored; 0 measured; 445 filtered out
```

行为测试 `rollback_restores_files_marks_nodes_and_refreshes_run` 变红（项目文件未恢复），其余按预期保持绿。删除该行恢复代码后复跑：`cargo test recover_` 9 passed，`cargo test` 全绿。

## 验证命令与结果

```
$ cargo check --all-targets
    Compiling aicross v0.1.0 (D:\work\aicross-p44)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.82s
（grep -c '^warning' = 0）

$ cargo check --release
    Checking webview2-com v0.38.2
    Checking tauri-plugin-single-instance v2.4.2
    Finished `release` profile [optimized] target(s) in 1m 08s

$ cargo test
running 456 tests
test result: ok. 446 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 65.46s
```

换行核对：五个改动文件 HEAD 与工作区都用字节计数核对（`tr -cd '\r' | wc -c`），CR 均为 0，全 LF，改动未引入换行变化。（先用 `grep -c $'\r'` 核对得到过误导结果，MSYS2 grep 对该写法的匹配不可信，已改用字节计数。）

## 取舍与没做的事

- **dispatch.rs 开放了两个函数而非一个**：任务书写「仅把现有回滚函数开放给引擎调用」。除 `rollback_node_changes_core` 外还开放了 `latest_before_dir`（回滚函数内部找快照的那一个，只加 `pub(crate)`）。原因：核对报告要与回滚认同一个快照目录，不开放就得在引擎里复刻派发记录解析（`parse_record_file` 等），改动更大且更易错。零逻辑改动。
- **差异比对在引擎侧新写**：任务书只允许开放回滚函数，快照函数未开放，故遍历与比对在 engine 新写约 90 行；排除目录清单与链接判定复刻 dispatch.rs 的口径，注释注明镜像关系。风险：两处清单将来可能漂移，改 dispatch.rs 快照排除清单时需同步。
- **超大文件会算「新增」**：快照持久化时超过单文件上限的文件不进快照目录，核对时这些文件相对快照就是「新增」。报告按任务书口径「相对派发前快照有变化」输出，不做归因；边界已写进 CONSOLE-CHANNEL.md。
- **无快照不改变退出码**：任务书列的退出 1 条件是「仍在运行、回滚有失败」，无快照单列不算回滚失败，退出码不受影响。
- **「回滚有失败」按函数 Err 判**：`rollback_node_changes_core` 把跳过项记在 `RollbackSummary.skipped`，实际几乎不返回 Err；skipped 不算失败（否则任何带排除目录的回滚都会退出 1）。因此退出 1 的回滚失败分支现有实现下基本不可达，没有专门测试。
- **无效 run_id 退出 1 而非 2**：与 `status`/`adopt`/`cancel` 同类处理（`read_run` 的 Err 统一按业务失败）；「参数错退出 2」只用于缺 run_id 与无法识别的参数。
- **「已核对」防重复追加**：同一 run 反复执行 `--rollback` 不会在原因里叠出「已核对；已核对」。
- **编排者真机核未做**：任务书最后一节要求真派一次单写者执行、杀后台引擎后跑 `recover` / `--rollback`。需要真实拉起各家 CLI 并消耗额度，留给编排者执行；本文档只覆盖实现与自动化验证。

## R1

按 `tasks/fix-44-r1.md` 在同一工作树上逐条修正。改动只碰清单允许的文件：`src/dispatch.rs`、`src/engine/mod.rs`、`src/console_cli.rs`、`docs/CONSOLE-CHANNEL.md`、`docs/MANUAL.md` 与本报告。未 commit、未 push。六个改动文件（含本报告）换行核对均为 LF（字节计数 CR = 0），与改动前一致。

### 逐条改动

1. **执行中快照（高）**
   - `src/dispatch.rs` 新增 `persist_inflight_snapshot`：单写者拉起 CLI 之前把内存里的派发前快照写到 `项目/.dispatch/<run>/<node>.inflight/`，`manifest.json` 记每个文件的相对路径、大小、是否存了内容，`files/` 存有内容的文件原文；沿用内存快照已有的排除目录、链接跳过与单文件上限取舍，超限文件只进清单。`run_one_attempt` 在算出 `needs_oneshot` 后落盘、落盘失败只在 runner_note 记「执行中快照保存失败：…，引擎中断后无法自动回滚」；删除点放在 `.before/` 持久化之后（任何结果，含失败、取消、超时），两个快照都缺的窗口最小。原有 `.before/` 逻辑未动。
   - `src/engine/mod.rs` 的 `recover` 按磁盘状态区分停在半路的单写者节点：有 `.inflight/` →「执行中断」，变化文件按清单对比（清单有项目无 → removed；存了内容的比内容、没存的比大小 → modified；项目有清单无 → added）；没有但有 `.before/` →「执行已结束，结果未写回」，新增只认 `.created` 记录；都没有时 draft →「未发出」不查文件，pending → 无快照单列（正文照任务书）。`--rollback` 时执行中断的节点恢复存了内容的文件、删掉新增，没存内容且有变化的文件列进 `unrestorable`（不算失败，输出与节点正文都写明），完成后删除 `.inflight/`。
   - 测试：`dispatch::tests::task44_inflight_snapshot_exists_during_execution_and_removed_after`（桩命令在执行中记录 `.inflight/` 是否存在，结束后确认已删且 `.before/` 照旧落盘）、`dispatch::tests::task44_inflight_snapshot_manifest_records_size_and_stored_flag`（清单字段与超限不存内容）、`engine::recover_cmd::inflight_interruption_is_reported_and_rolled_back`（只读核对与回滚，含 added 被删、超限文件列无法恢复）。
2. **pending 限定本 run（高）**：stuck 筛选只认 run 摘要里的节点，`pending` 与 `draft` 同样处理。测试钉在 `read_only_check_lists_stuck_nodes_and_changed_files` 与 `rollback_restores_files_marks_nodes_and_refreshes_run` 里：夹具有一个不属于本 run 的 pending 单写者节点（有自己的 `.before`、改了项目文件），断言不进报告、状态保持 pending、文件不回滚。
3. **回滚失败正文（中）**：三条回滚路径（执行中快照清单损坏、执行中快照回滚出错、`rollback_node_changes_core` 返回 Err）统一改为正文「引擎中断，回滚失败：<原因>，请自行核对」、进 `failures`、退出 1，继续处理其余节点。测试 `corrupt_inflight_manifest_fails_rollback_and_reports`（清单损坏是本轮新增的、能稳定触发 Err 的回滚失败；只读模式报错误不退出非零）。
4. **标 failed 保留 reply 其余字段（中）**：只改 `status`、`text`、`error_kind`。测试钉在 `rollback_restores_files_marks_nodes_and_refreshes_run`：worktree 节点标失败后 `artifacts.worktree` 仍是原路径。
5. **worktree 路径推算（中）**：图上没有产物时按创建处同一规则推算（引擎数据目录 `runs/<run_id>/<node_id>/wt`，为此把 `dispatch_data_root` 开放为 `pub(crate)`，逻辑不改）；路径不存在时输出 `worktree_missing: true`。测试 `worktree_without_artifacts_gets_inferred_path`（推算路径正确并标不存在；目录建出后再核对不再标）。
6. **无停在半路节点也追加「已核对」（中）**：`--rollback` 时 run 摘要刷新与「已核对」追加不再以有节点为前提（防重复追加保留），图只在有节点要标时才合并写。测试 `rollback_without_stuck_nodes_still_appends_reason`。
7. **`--rollback` 不再被当成 run_id（中）**：现有实现的 `positional` 已过滤 `-` 开头参数，行为本就正确；本轮补钉住测试（`recover_requires_run_id_and_rejects_unknown_args` 扩展 `aicross recover --rollback` → 退出 2「缺少 run_id」）。
8. **输出顺序（低）**：stuck 节点改从 run 摘要的节点序取（顺带覆盖第 2 条的成员限定），不再依赖图上的键序；既有断言全部改为按 node_id 查找。测试：`read_only_check_lists_stuck_nodes_and_changed_files` 的顺序断言（夹具里 run 摘要顺序与图插入序相反）。
9. **文档（低）**：`docs/CONSOLE-CHANNEL.md` 崩溃后核对一节改为如实描述——只读核对不改项目文件与图，但读取 run 时若判定引擎进程已退出会把 run 状态更新为失败（与 `status` 相同）；补三种节点状态、`.created` 新增口径、worktree 推算与 `worktree_missing`、回滚失败正文，并补一句执行中快照（`.dispatch/` 下的 `<node>.inflight/`，执行结束即删除）。`docs/MANUAL.md` 崩溃后核对一节同步。STATE.md 的说明照旧（`recover` 不读它）。

### 回退验证（逐条）

每条修复临时改回旧实现、只跑相关测试、记录后恢复。恢复后 `grep -rn REVERT src/` 为 0。

| 条目 | 临时改回 | 变红的测试 | 结果 |
|---|---|---|---|
| 1 | dispatch 不落盘 `.inflight/`，且 `recover` 跳过执行中快照分支 | `inflight_interruption_is_reported_and_rolled_back`、`corrupt_inflight_manifest_fails_rollback_and_reports`、`task44_inflight_snapshot_exists_during_execution_and_removed_after` 三个红 | 已恢复，复跑绿 |
| 2 | stuck 改回遍历整图、`pending` 不限 run | `read_only_check_lists_stuck_nodes_and_changed_files`、`rollback_restores_files_marks_nodes_and_refreshes_run`（外来节点被标 failed，断言保持 pending 红）、`rollback_without_stuck_nodes_still_appends_reason` | 已恢复，复跑绿 |
| 3 | Err 分支正文改回「引擎中断，已回滚项目改动」 | `corrupt_inflight_manifest_fails_rollback_and_reports` | 已恢复，复跑绿 |
| 4 | 标 failed 改回整份 reply 覆写 | `rollback_restores_files_marks_nodes_and_refreshes_run`（worktree 路径变空串） | 已恢复，复跑绿 |
| 5 | 禁用 worktree 路径推算 | `worktree_without_artifacts_gets_inferred_path`（worktree 为空串） | 已恢复，复跑绿 |
| 6 | 没有节点时不刷新 run 摘要 | `rollback_without_stuck_nodes_still_appends_reason`（原因缺「已核对」） | 已恢复，复跑绿 |
| 7 | `run_recover` 的 run_id 解析换回不过滤 `--` 的形态 | `recover_requires_run_id_and_rejects_unknown_args`（`--rollback` 被当 run_id，退出 1 ≠ 2） | 已恢复，复跑绿 |
| 8 | 顺序改回图上键序（保留 run 成员限定，只回退顺序这一半） | `read_only_check_lists_stuck_nodes_and_changed_files`（顺序为图插入序，与 run 摘要序相反） | 已恢复，复跑绿 |
| 9 | 文档，无测试；逐句与实现核对 | — | — |

过程记录：第 2 条的回退验证用的是完整旧实现（含遍历来源），与修正的改动面一致；第 8 条第一次回退把成员限定也丢了（混入第 2 条的问题），重做为只回退顺序的版本，上表为最终记录。第 7 条回退时 perl 误改了 `adopt` 分支同款单行代码，当场复原，最终文件无残留、adopt 逻辑未变。

### 验证命令与结果

```
$ cargo check --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.30s
（输出里 warning 计数 = 0）

$ cargo check --release
    Finished `release` profile [optimized] target(s) in 19.83s

$ cargo test
running 462 tests
 test result: ok. 452 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 65.31s
```

新增测试 6 个（engine 4、dispatch 2），更新 2 个 engine 用例、扩展 1 个 console_cli 用例；总数从 456（446 passed + 10 ignored）变为 462（452 + 10）。

### 取舍与没做的事

- **执行中快照只为声明的单写者节点落盘**：`node.write == "单写者"`。worktree 节点创建失败退回单写者的场景（实际直接写项目）不建 `.inflight/`——图上它仍是 worktree 节点，`recover` 的 worktree 分支不读执行中快照，建了也用不上。这个场景中断后项目改动无自动恢复手段，属已知边界。
- **`rollback_node_changes_core` 现实现几乎不返回 Err**（所有差错都归入 `RollbackSummary.skipped` 并返回 Ok），第 3 条的 Err 处理测试改由执行中快照清单损坏触发——这是本轮新增的、能稳定构造 Err 的回滚路径，三条 Err 分支共用同一处理代码。`rollback_node_changes_core` 本体逻辑未改。
- **执行中回滚删新增失败算回滚失败**（Err、进 failures、退出 1）；清单损坏同理。回滚成功后删除 `.inflight/` 的失败静默忽略：节点此时已标 `failed`，下次核对不会再列出，残留目录不影响。
- **执行中回滚不清理节点新建的目录**：清单只记文件，回滚删掉新增文件后可能留下空目录（`.before` 回滚按 `.created` 的 `dir:` 行清理目录，执行中快照没有对应记录）。空目录无内容损失，不做。
- **`worktree_missing` 用布尔字段**：输出 JSON 里路径照给、存在性单独立字段，文档里写明含义；不用另一句文案。
- **draft 节点（未发出）的正文**：修正清单未给文案，按最保守处理用「引擎中断」（与只读节点一致）；报告里状态字段「未发出」，不查文件、不进 `no_snapshot`。
- **「执行已结束，结果未写回」的新增口径**：只认 `.created` 里仍存在于项目的非 `dir:` 行；`.created` 缺失或写坏时新增为空，`modified` / `removed` 不受影响。
- **编排者真机核仍未做**：与前轮同样留给编排者执行（需真实拉起各家 CLI 并消耗额度）；本轮所有验证为临时目录内的自动化测试。


## 编排者真机核与收尾（10-01）

复审（R0）：Claude 子代理 + Antigravity（Gemini 3.8 Flash）盲审，发现执行中途被杀时磁盘上没有派发前快照（任务书前提不成立）等问题，整理为 `tasks/fix-44-r1.md`，R1 由 GLM-5.3-Flash 修正。

编排者补一处：只读节点在核对输出里没有 `status` 字段，补为「执行中断」/「未发出」，`read_only_check_lists_stuck_nodes_and_changed_files` 加断言；文档同步一句。

真机（release 构建，临时 git 项目，经济模式，执行者 codex）：执行者改了 `calc.py`、新建 `notes.txt` 后强杀后台引擎。
- `aicross recover`：执行节点「执行中断」，`modified: calc.py`、`added: notes.txt`，带 `inflight_dir`；退出 0。
- `aicross recover --rollback`：`calc.py` 恢复原文、`notes.txt` 删除、`.inflight/` 删除，`git status` 干净；执行节点标「引擎中断，已回滚项目改动」，验收与验证节点标「引擎中断」；run 原因「引擎进程已退出；已核对」。再跑一次 `recover` 报「没有停在半路的节点」。
- 引擎被杀后没有残留的 codex 进程。

验证：`cargo check --all-targets` 零 warning；`cargo build --release` 通过；`cargo test` 452 passed。
