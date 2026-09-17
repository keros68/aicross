# REPORT-32-R1 只读问题盲审附项目副本 · 复审修正轮

## 结果

已按 `fix-32-r1.md` 完成六条必修，只修改 Rust 实现与测试，并新增本报告。任务书与代码现状没有发现矛盾。

`git ls-files -z` 截断现在保留最后一个 NUL 之前的完整路径继续复制并明示副本不完整；单文件级读取、建父目录或写入失败改为跳过并计数；源文件规范化后必须仍位于规范化来源根内；git 固定排除补入 `.aicross-runtime`；原 TASK-28 真实入口测试按 `main` 恢复，TASK-32 真实入口测试单独保留；单文件上限提示按实际 `file_limit` 生成。

## 六条必修对应

1. **截断的 git 文件清单继续使用。** `readonly_project_paths` 在 `ProcessResult.truncated` 时只解析 `stdout` 中最后一个 `\0` 及以前的完整记录，丢弃末尾半条路径；正常排序、去重和复制，并附加“版本库文件清单过长，只取了前 N 个”。没有把清单截断升级为节点错误。
2. **单文件 IO 失败降级。** 副本根目录 `project/` 的 `create_dir_all` 仍是致命错误；文件规范化、元数据、读取、目标父目录创建和文件写入失败均跳过当前条目，统一计入“不可读取或非普通文件”，其它文件继续复制。测试按任务书使用读取失败注入，确认坏文件被跳过、好文件仍复制、prompt 有不完整说明。
3. **阻断中间链接与 junction 越界。** 先规范化副本来源根，再逐文件 `fs::canonicalize`，只有结果能 `strip_prefix` 于规范化来源根时才继续；否则按不可读取条目跳过。Windows 测试优先建立目录 symlink，权限不足时退回 `mklink /J`；本机成功建立链接或 junction，测试实际执行并通过，没有走跳过分支。
4. **补齐 `.aicross-runtime` 排除。** `excluded_project_copy_path` 现在统一排除路径任一组件为 `.dispatch`、`.git` 或 `.aicross-runtime`。TASK-32 的 `dispatch_stage_core` 主链测试将 `.aicross-runtime/private.txt` 纳入 git 跟踪并确认副本中不存在该目录。
5. **恢复 TASK-28 回归测试。** 从 `main` 恢复原名、原内容的 `task28_readonly_answer_reaches_blind_review_through_dispatch_stage_core`；TASK-32 的项目副本端到端测试另立为 `task32_readonly_project_copy_reaches_blind_review_through_dispatch_stage_core`，没有用改名替代旧回归。
6. **上限提示不再写死。** “已跳过 N 个超过 … MB 的文件”使用 `file_limit / (1024 * 1024)`；缩小入口允许测试传入 2 MB 并断言提示为 2 MB。

## 既有规则复用

- 非 git 路径仍走 `existing_project_files`，继续复用 `excluded_snapshot_directory` 的既有递归排除规则；本轮没有改其全量收集方式。
- git 与非 git 文件仍经过 `excluded_project_copy_path` 和 `secret_project_file`，继续排除 `.dispatch`、`.git`、密钥文件名；本轮只在固定目录排除中增加 `.aicross-runtime`。
- UTF-8 文本仍复用 `redact_text(text, settings.redact)`，脱敏命中数仍并入派发计数；非 UTF-8 文件仍按原始字节复制。
- 清单路径仍先走 `safe_relative_path`，拒绝绝对路径、盘符、根目录、`.` 与 `..`；本轮在它之后增加规范化来源根前缀检查，未放宽原检查。
- worktree 来源仍复用 `registered_worktrees` 核实登记状态，未改变来源选择规则。

## 红测

六条均先临时去掉对应实现并确认检查失败，随后立即恢复：

1. 临时恢复“`listed.truncated` 直接返回错误”，运行 `cargo test task32_r1_truncated_git_file_list_keeps_complete_paths_and_marks_incomplete -- --nocapture`。结果：退出码 1，`0 passed; 1 failed`；失败原因为 `Err("读取版本库文件清单失败：git ls-files 输出被截断")` 被 `unwrap()`。
2. 临时恢复单文件 `fs::read` 错误上抛，运行 `cargo test task32_r1_single_file_read_failure_is_skipped_without_losing_other_files -- --nocapture`。结果：退出码 1，`0 passed; 1 failed`；测试注入的读取失败使节点直接返回错误。
3. 临时取消规范化来源根的前缀拒绝，运行 `cargo test task32_r1_parent_symlink_cannot_escape_project_copy_source -- --nocapture`。结果：退出码 1，`0 passed; 1 failed`；项目外的 `linked/secret.txt` 被复制，`!workdir.join("project/linked/secret.txt").exists()` 断言失败。
4. 临时从固定排除移除 `.aicross-runtime`，运行 `cargo test task32_readonly_project_copy_reaches_blind_review_through_dispatch_stage_core -- --nocapture`。结果：退出码 1，`0 passed; 1 failed`；`!workdir.join("project/.aicross-runtime").exists()` 断言失败。
5. 临时移除原测试名（仅把函数名改为带 `_removed_for_red_check` 的名字），运行精确验收检查 `rg -n -F "fn task28_readonly_answer_reaches_blind_review_through_dispatch_stage_core()" src/dispatch.rs`。结果：退出码 1、无匹配；恢复原名后同一检查命中，且原测试单独运行通过。此条交付物本身就是被恢复的回归测试，因此使用源级存在性检查做红测。
6. 临时把提示参数恢复为常量 `1`，运行 `cargo test task32_r1_project_copy_limit_message_uses_configured_file_limit -- --nocapture`。结果：退出码 1，`0 passed; 1 failed`；实际为“超过 1 MB”，期望为“超过 2 MB”。

恢复实现后的定向结果：

```text
cargo test task32 -- --nocapture
test result: ok. 11 passed; 0 failed; 0 ignored; 484 filtered out

cargo test task28_readonly_answer_reaches_blind_review_through_dispatch_stage_core -- --nocapture
test result: ok. 1 passed; 0 failed; 0 ignored; 494 filtered out

将当前 TASK-28 函数块与 `git show main:src/dispatch.rs` 中同名函数块规范化换行后逐字比较
task28_block_matches_main=True; current_chars=3626; main_chars=3626
```

## 最终验证

```text
cargo check --all-targets
Finished `dev` profile; 0 warning

cargo test --quiet  （连续第 1 次）
test result: ok. 484 passed; 0 failed; 11 ignored

cargo test --quiet  （连续第 2 次）
test result: ok. 484 passed; 0 failed; 11 ignored

cargo test --quiet  （连续第 3 次）
test result: ok. 484 passed; 0 failed; 11 ignored

cd src-ui && npx vitest run --configLoader native --pool threads
Test Files  45 passed (45)
Tests  656 passed (656)

cd src-ui && npm run build
tsc -b && vite build 成功；350 modules transformed；built in 636ms
```

`npm run build` 仍有既有的单个 chunk 超过 500 kB 提示，不影响退出码；没有前端源码改动。最终 `cargo check --all-targets` 为零 warning。

## 按裁定保持不改

- “`project` 未并入附件保留名”不成立：保持现有 `names` 去尾斜杠生成保留名的实现与既有重名测试，不改。
- 非 git 大目录遍历仍复用 `existing_project_files`，不增加取消点，不改变先全量收集行为。
- 机器上没有 git 时只读盲审仍按现有产品依赖失败，不增加降级路径。

## 未做

- 未运行 `cargo fmt`，未启动窗口。
- 未发真机请求，未调用真实模型。
- 未修改前端源码，未扩大到六条必修以外的功能。
- 未 commit，未 push。
