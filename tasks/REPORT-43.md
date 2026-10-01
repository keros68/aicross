# REPORT-43 — 盲验收材料：项目副本、验收条件文件、提交号

按 `tasks/TASK-43-acceptance-materials.md` 实现。分支 `engine/accept43`，未 commit、未 push。

## 改了什么

### src/dispatch.rs（盲目录组装）

- 盲验收（角色「验收」、可见性「盲」）有上游产物时**一律**附项目只读副本，不再只在执行者未改文件时附；其余盲节点维持原状。多路上游按顺序各附一份 `project/`、`project-2/`…（相同来源只附一份），上游执行是 worktree 节点时取该 worktree（仍核对已登记），否则取项目目录；多副本时提示写清对应关系（`project_copy_note`）。副本的排除规则、脱敏、「副本不完整」提示不变。
- 上游全部执行的 diff 与 changed_files 都为空时，提示追加一句「本轮执行未改文件，以 project/ 当前内容为准」。
- 盲目录新增 `ACCEPT.md`：派发请求带验收条件时，盲验收与盲审节点（非「执行」角色）的盲目录写入全文（过脱敏），提示追加「本次验收条件见 ACCEPT.md，请逐条判断是否满足」；执行节点不收；未给时不生成文件、不加提示。
- 盲目录 `REQUEST.md` 头部（项目是 git 仓库时）加「项目提交：<HEAD 短 sha>」，有未提交改动时后缀「（含未提交改动）」；上游是 worktree 节点时加「执行者 worktree 提交：<该 worktree 的 HEAD 短 sha>」（多路去重后逐行列出）。非 git 项目或取不到时不写、不报错。
- `DispatchStageRequest` 追加字段 `accept_criteria`（`serde(default)` 且空串不序列化）。

### src/engine/mod.rs（验收条件随请求下发）

- `DispatchInput`、`EngineRun` 各追加 `accept_criteria`；run 文件字段空则不写出。
- `start_dispatch_with` 把验收条件写进 run 文件；`FileSink::dispatch` 在派出前把 run 文件里的验收条件盖到 `DispatchStageRequest` 上。金标准夹具走的记录型 sink 不经过这里，请求保持与夹具逐字一致。

### src/console_cli.rs（仅 `dispatch` 的 `--accept-file`）

- 可选参数 `--accept-file <路径>`：UTF-8、可带 BOM（读取与编码校验同 `--text-file`）；超过 64 KB 按 UTF-8 字符边界截断，stderr 说明「验收条件文件超过 64 KB，已截断」。缺值、文件不存在、非 UTF-8 分别返回参数错误。内容进请求体 `accept_criteria`；未给时请求体没有该字段。

### docs/CONSOLE-CHANNEL.md、docs/MANUAL.md

- 命令行契约与手册补 `--accept-file`、盲目录 `project/`、`REQUEST.md` 头部、run 文件字段的说明；术语表加 `ACCEPT.md` 与 `project/` 两行。

### src/run_view.rs（任务书清单之外，见「取舍」）

- 测试辅助函数 `run_summary` 的 `EngineRun` 字面量补一行 `accept_criteria: String::new(),`。`EngineRun` 加字段后该字面量缺字段编不过，属编译必需的机械修复。

## 新增测试

| 文件 | 测试名 | 覆盖 |
|---|---|---|
| src/dispatch.rs | `accept43_blind_acceptance_always_gets_project_copy_after_changes` | 单写者执行改了文件后的盲验收有 `project/`，内容是改后的文件；无 ANSWER.md |
| src/dispatch.rs | `accept43_project_copies_follow_upstream_worktrees` | worktree 上游：副本来自 worktree；两路 worktree 各附 `project/`、`project-2/`，提示写清对应关系 |
| src/dispatch.rs | `accept43_empty_diff_note_says_nothing_changed` | 上游 diff 全空时提示含「本轮执行未改文件，以 project/ 当前内容为准」 |
| src/dispatch.rs | `accept43_accept_file_reaches_acceptance_but_not_executor` | 盲目录有 `ACCEPT.md` 与提示；盲可见性的执行节点不收；未给时不写文件、不加提示 |
| src/dispatch.rs | `accept43_request_serialization_unchanged_without_accept_criteria` | 未给时序列化无 `acceptCriteria` 键、反序列化为空串；给了才序列化（钉住金标准） |
| src/dispatch.rs | `accept43_request_md_header_carries_commit_ids` | git 仓库干净 / 有未提交改动 / worktree 上游三种头部件；非 git 情形由既有测试 `blind_acceptance_directory_contains_only_artifact_delivery`（断言 REQUEST.md 逐字相等）覆盖 |
| src/console_cli.rs | `dispatch_accept_file_requires_value_and_existing_utf8_file` | 缺值、文件不存在、非 UTF-8 |
| src/console_cli.rs | `dispatch_accept_file_goes_into_request_body_and_truncates_at_64kb` | BOM 剥离、进请求体、64 KB 截断与 stderr 说明、未给时无该字段 |
| src/engine/mod.rs | `accept_criteria_run_file::accept_criteria_is_stored_in_the_run_file_and_omitted_when_empty` | 写进 run 文件；空则不写出 |

`fixtures/engine-golden/` 未动；`engine::golden::engine_golden_fixtures_replay` 照常通过。

## 回退验证

临时把行为层改回（保留签名与字段，保证可编译）：`prepare_outbound` 恢复「仅只读回答才附副本」、去掉「未改文件」提示、不写 `ACCEPT.md`、`REQUEST.md` 头部置空；`start_dispatch_with` 不再落 `accept_criteria`。

结果（命令原文摘录）：

```
$ cargo test accept43
test dispatch::tests::accept43_request_serialization_unchanged_without_accept_criteria ... ok
test dispatch::tests::accept43_accept_file_reaches_acceptance_but_not_executor ... FAILED
test dispatch::tests::accept43_blind_acceptance_always_gets_project_copy_after_changes ... FAILED
test dispatch::tests::accept43_empty_diff_note_says_nothing_changed ... FAILED
test dispatch::tests::accept43_project_copies_follow_upstream_worktrees ... FAILED
test dispatch::tests::accept43_request_md_header_carries_commit_ids ... FAILED
test result: FAILED. 1 passed; 5 failed; 0 ignored; 0 measured; 440 filtered out

$ cargo test accept_criteria_is_stored
test engine::accept_criteria_run_file::accept_criteria_is_stored_in_the_run_file_and_omitted_when_empty ... FAILED
test result: FAILED. 0 passed; 1 failed
```

6 个行为测试全部变红；序列化钉子测试按设计保持绿。恢复代码后复跑：`accept43` 6 passed，`accept_criteria_is_stored` 1 passed。

## 验证命令与结果

```
$ cargo check --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.95s
（grep -c warning = 0）

$ cargo check --release
    Checking tauri-plugin-single-instance v2.4.2
    Finished `release` profile [optimized] target(s) in 1m 01s

$ cargo test
test result: ok. 436 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 64.07s
```

所有改动文件保持原有 LF 换行（改动前后用 `grep -q $'\r'` 核对，均无 CR）。

## 取舍与没做的事

- **run_view.rs 的机械修复**：任务书只允许五个文件，但 `EngineRun` 加字段后 run_view.rs 测试里的全字段字面量编不过。选择了加一行空字段的最小修复并在报告里写明，而不是改动任务书要求的结构。
- **`--accept-file` 的解析测试无法参与回退变红**：参数解析本身就是这次新增的代码，回退它等于连测试一起删。回退验证只覆盖盲目录行为与 run 文件落盘两块。
- **验收条件不进执行节点提示**：任务书只要求盲目录写 `ACCEPT.md`；未给 `--accept-file` 但节点为盲验收时同样附 `project/`，此为任务书明文（「一律」）。
- **空串验收条件按未给处理**：文件存在但内容为空时，run 文件不写字段、盲目录不生成 `ACCEPT.md`，与「空则不写出」一致。
- **编排者真机核未做**：需要真实拉起各家 CLI 并消耗额度，留给编排者按任务书最后一节执行；本文档只覆盖实现与自动化验证。

## 复审与修正（编排者，10-01）

实现：GLM-5.3-Flash（pi 通道，真身未核对）。复审：Claude 子代理 + Antigravity（Gemini 3.8 Flash），均盲审。

采纳并修正：
- `prepare_outbound` 写 `ACCEPT.md` 的分支在 `if blind` 之外，且以「角色不是执行」放行：共享模式下非盲验收节点的工作目录就是项目目录，会把 `ACCEPT.md` 写进用户项目；盲的出方案等节点也会收到。改为只在盲节点、角色为验收 / 验证 / 仲裁时写。
- 多份项目副本时「本轮执行未改文件」提示只指向 `project/`：多副本时改为「以各项目副本当前内容为准」。
- 测试缺口：验收条件从请求经 `dispatch_stage_core` 到盲目录的链路没有测试；64 KB 截断只测了纯 ASCII。新增 `accept43_accept_criteria_reaches_blind_review_through_dispatch_stage_core`，`accept43_accept_file_reaches_acceptance_but_not_executor` 补出方案节点与共享验收节点两例，`dispatch_accept_file_goes_into_request_body_and_truncates_at_64kb` 补多字节边界一例。

回退验证：分别改回放行条件、把链路里的验收条件换成空串、去掉字符边界回退，各自只有对应的一条测试变红；恢复后全绿。

未采纳：
- 「ANSWER.md 提示与未改文件提示互斥」：两句都在说执行者未改文件，不冲突。
- 「`build_request` 恒为空串」：按设计由 `FileSink::dispatch` 盖章，记录型 sink 不盖章才能保持金标准夹具逐字一致。`FileSink::dispatch` 这一行没有单独的测试，记为限制。
- `run_fallback` 传空串：该函数只在测试构建存在，真实备选路径 `run_fallback_attempt` 传了验收条件。

修正后：`cargo check --all-targets` 零 warning；`cargo build --release` 通过；`cargo test` 437 passed。

## 编排者真机核

临时 git 项目（`calc.py` 的 add 写成减法，附 `test_calc.py`），经济模式，`--accept-file` 写两条条件（pytest 通过、只改 calc.py）。

- 验收节点盲目录：`ACCEPT.md`、`project/`（`calc.py` 为改后内容）、`changed/`、`diff.patch`、`REQUEST.md`；`REQUEST.md` 头部「项目提交：638b1e9（含未提交改动）」。验收（kimi）逐条判断后给 PASS。
- 执行节点（共享、单写者）：项目目录里没有 `ACCEPT.md`。
- 验证节点（盲审）同样收到 `ACCEPT.md`，但没有项目副本，本机也未配置验证命令，结论为「无法判定」。这是盲审「只看材料与验证命令输出」的设计；要让它判定测试是否通过，需配置 `validationCommand`。
