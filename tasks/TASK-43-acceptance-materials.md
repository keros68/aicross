# TASK-43 盲验收材料：项目副本、验收条件文件、提交号（10-01，积压 98 + 定案「验收条件文件」）

**前置：TASK-42、TASK-45 已合并。只改 `src/dispatch.rs`（盲目录组装）、`src/engine/mod.rs`（仅把验收条件随请求下发）、`src/console_cli.rs`（仅 `dispatch` 的 `--accept-file`）、`docs/CONSOLE-CHANNEL.md`、`docs/MANUAL.md`。不改 `fixtures/engine-golden/`。不 commit，不 push。**

## 1. 盲验收一律附项目只读副本（积压 98）

10-01 真机（run `9900e4e6`）：单写者执行改了文件后，首轮验收只拿到 `REQUEST.md`、`diff.patch`、`changed/`，没有项目副本，跑不了测试，判了 NEEDS_FIX；修正轮执行面对已改好的工作区，本轮 diff 为空，第二轮验收据此判「交付为零」，最后进遗留。

现状（`src/dispatch.rs` 约 3576–3650 行）：只有执行者没改文件时（`readonly_answer_material` 命中）才调 `copy_readonly_project` 附 `project/`。

要求：
- 有上游产物的盲验收，**一律**附 `project/` 只读副本（同一套排除规则、脱敏与「副本不完整」提示）。副本来源：上游执行是 worktree 节点时取该 worktree 目录（`artifacts.worktree`），否则取项目目录。多路上游各有 worktree 时按上游顺序各附一份（`project/`、`project-2/` …），在提示里写清对应关系。
- 上游全部执行的 diff 都为空、但带产物时（修正轮在已改好的工作区上什么也没改），提示里加一句「本轮执行未改文件，以 project/ 当前内容为准」，不再让验收按「交付为零」理解。
- 原有「执行者未改文件 → ANSWER.md」逻辑保留。

## 2. 验收条件文件 `ACCEPT.md`（10-01 定案）

- `aicross dispatch` 追加可选参数 `--accept-file <路径>`（UTF-8，可带 BOM；读取与编码校验同 `--text-file`；文件过大按 64 KB 截断并在 stderr 说明）。
- 内容随派发进入 run 文件（追加字段，空则不写出）与本次派发的请求（`DispatchStageRequest` 追加字段，`serde(default)` 且空串不序列化——保证金标准夹具的比对不受影响）。
- 盲验收与盲审节点的盲目录写 `ACCEPT.md`，提示里补一句「本次验收条件见 ACCEPT.md，请逐条判断是否满足」；没给 `--accept-file` 时不生成文件、不加提示。
- 执行节点不收到 `ACCEPT.md`（验收条件只给验收方；执行者从任务文本里获得要求）。

## 3. 提交号写进 `REQUEST.md` 头部（10-01 定案）

盲目录的 `REQUEST.md` 开头加两行（项目是 git 仓库时）：
- `项目提交：<HEAD 短 sha>`，项目目录有未提交改动时后缀「（含未提交改动）」；
- 上游是 worktree 节点时加 `执行者 worktree 提交：<该 worktree 的 HEAD 短 sha>`。
不是 git 仓库或取不到时不写这两行，不报错。

## 测试

- 单写者执行改了文件后的盲验收：盲目录里有 `project/`，内容是改后的文件。
- worktree 上游：副本来自 worktree 目录。
- 上游 diff 全空：提示里有「本轮执行未改文件」那句。
- `--accept-file`：参数解析（缺值、文件不存在、非 UTF-8）、写进 run 文件、盲目录有 `ACCEPT.md`、执行节点没有、未给参数时请求序列化与原来逐字相同（钉住金标准不受影响）。
- `REQUEST.md` 头部两行：git 仓库干净 / 有未提交改动 / 非 git 仓库三例。
- 新测试在修复临时改回时必须变红；`engine::golden::engine_golden_fixtures_replay` 照常通过。

## 验证

```
cargo check --all-targets        # 零 warning
cargo check --release
cargo test
```

每个改动文件保持原有换行。写 `tasks/REPORT-43.md`。

## 编排者真机核

在临时 git 项目里真派一次经济模式（修一行 bug，附 `--accept-file`），看盲验收目录里的 `project/`、`ACCEPT.md`、`REQUEST.md` 头部，确认首轮验收能跑测试并给出合理结论。
