# REPORT-32 只读问题的盲审附项目只读副本

## 结果

已完整实现 TASK-32。只读执行产物满足 `readonly_answer_material` 条件时，盲审目录除 `ANSWER.md` 外会新增 `project/`；有 diff 或 changed_files 时行为不变。未修改 `src-ui/**`，未改协议字段。

任务书与代码现状没有发现矛盾。

## 实现对应

1. 触发与材料：项目副本与 `ANSWER.md` 共用同一个 `readonly_answer_material` 分支。材料名单新增 `project/`，prompt 新增“项目只读副本见 project/，用于核对回答，不要修改”；副本不完整时紧接着写明原因。
2. git 项目：通过现有 `git_result_with_timeout` 运行 `git rev-parse` 与 `git ls-files -z`，沿用节点取消键和超时；只复制版本库跟踪文件。测试中的未跟踪文件未进入副本。
3. 非 git 项目：复用 `existing_project_files`，该函数递归时直接使用现有 `excluded_snapshot_directory`，因此沿用 `.dispatch`、`.git`、`node_modules`、`target`、`.venv`、`dist`、`build`、`coverage`、`.next`、`vendor` 等快照排除规则，没有另写第二套非 git 目录清单。
4. 固定排除：git 与非 git 路径都再次排除 `.dispatch/`、`.git/`。仓内没有现成的密钥文件名清单，因此新增一处集中判断，覆盖 `.env`、`.env.*`、`*.pem`、`*.key`、`*.p12`、`*.pfx`、`*.jks`、`id_rsa*`、`id_dsa*`、`id_ecdsa*`、`id_ed25519*`。
5. 上限：单文件超过 1 MB 时跳过；复制达到 5000 个文件或下一文件会使源文件总量超过 50 MB 时停止。跳过大文件、不可读取或非普通文件、文件数截停、总量截停都会形成“副本不完整：……”提示，不静默处理。
6. 脱敏与二进制：UTF-8 文件复用 `redact_text(text, settings.redact)`，开关与 `REQUEST.md` 一致，命中数并入派发脱敏计数。二进制沿用盲审附件的现有判断，即 UTF-8 解码失败时按原始字节复制。
7. 路径安全：每个待写路径复用 `safe_relative_path`，拒绝绝对路径、盘符前缀、根目录、`..` 与 `.` 分量；链接、junction 和非普通文件不跟随。
8. 来源时点：没有 worktree 时直接读取派发项目目录；上游产物带 worktree 时，先规范化路径并用现有 `git worktree list --porcelain` 核实它仍是该项目登记的 worktree，再从该 effective 目录复制。混合项目目录与 worktree、或多个不同 worktree 时拒绝猜测来源。
9. 重名保护：`project` 与 `ANSWER.md` 一样加入本轮材料保留名，附件同名返回“附件文件名与盲审材料重名：project”。
10. 清理：副本只写到现有盲审目录的 `project/` 下，不写回项目。备选重跑仍由 `run_fallback_attempt` 对整个盲审目录执行 `remove_dir_all`；现有重跑测试已扩展为同时物化附件、`ANSWER.md` 和 `project/`，确认清理后第二次可重新物化。

## 测试与红测

新增 7 条 `task32` 测试：

- `task32_readonly_project_copy_reaches_blind_review_through_dispatch_stage_core`：临时 git 项目经 `dispatch_stage_core` 完成“只读执行 → 盲验收”，确认跟踪的 `src/beta.py` 内容一致，未跟踪文件、跟踪的 `.env` 与 `.dispatch/` 文件均未进入副本，prompt 含指路句。
- `task32_non_git_project_copy_reuses_snapshot_directory_exclusions`：确认非 git 项目复用既有目录排除规则。
- `task32_project_copy_limit_is_reported_in_prompt`：确认超过 1 MB 的文件被跳过且 prompt 明示副本不完整。
- `task32_project_copy_stops_at_total_and_file_count_limits`：以缩小的测试阈值分别钉住总量和文件数截停。
- `task32_project_copy_redacts_text_and_preserves_binary_bytes`：确认文本密钥样式被脱敏，非 UTF-8 二进制原样复制。
- `task32_project_attachment_name_conflict_is_rejected`：确认附件名 `project` 被拒绝。
- `task32_project_copy_uses_recorded_registered_worktree`：确认副本读取上游产物记录并经 git 登记核实的 effective worktree，而非项目主目录。

反例继续由 `task28_nonempty_diff_does_not_write_answer` 钉住，并新增断言：diff 非空时没有 `project/`，prompt 也没有项目副本指路句。`fallback_blind_attempt_gets_fresh_material_dir` 已扩展到项目副本，确认盲目录随备选重跑整体清理。

红测方式：临时让项目复制函数返回空结果，并临时取消 `project/` 的材料保留名登记，然后运行：

```powershell
$env:AICROSS_TEST_DISABLE_PROJECT_COPY='1'
cargo test task32 -- --nocapture
```

结果为 0 passed、7 failed；上述 7 条逐条失败。失败点分别是副本文件不存在、上限提示不存在、脱敏/二进制文件不存在、附件同名未被拒绝、worktree 内容不存在以及真实入口没有产出 `project/src/beta.py`。随后立即恢复实现，`cargo test task32 -- --nocapture` 为 7 passed、0 failed。

## 最终验证

最终 Rust 状态完成以下验证：

```text
cargo check --all-targets
Finished `dev` profile；0 warning

cargo test --quiet  （连续第 1 次）
479 passed; 0 failed; 11 ignored

cargo test --quiet  （连续第 2 次）
479 passed; 0 failed; 11 ignored

cargo test --quiet  （连续第 3 次）
479 passed; 0 failed; 11 ignored
```

其余两项结果：

```text
cd src-ui && npx vitest run --configLoader native --pool threads
45 test files passed；656 tests passed

cd src-ui && npm run build
tsc -b && vite build 成功；350 modules transformed
```

`npm run build` 仍有既有的单个 chunk 超过 500 kB 提示，不影响退出码；没有前端源码改动。`git diff --check` 通过；`git ls-files --eol src/dispatch.rs` 为 `i/lf w/lf`，保持原有 LF。未运行 `cargo fmt`。

## 未做

- 未启动窗口。
- 未发真机请求，未调用真实模型。
- 未修改前端、修正轮、验收解析或 `ANSWER.md` 既有内容。
- 未修改 backlog。
- 未 commit，未 push。
