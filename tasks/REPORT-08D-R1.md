# REPORT-08D-R1 蜂群合并 08D 复审修正轮

范围：完整实施 `tasks/fix-08d-r1.md`。代码只修改 `src/dispatch.rs`（含底部 `mod tests`），未修改 `src/process.rs`、前端或其他实现文件。未运行 `cargo fmt`，`src/dispatch.rs` 保持 LF；未启动窗口、未发真机请求、未 commit、未 push。

## 11 条必修对应

### 1. 未跟踪新目录与暂存后二次范围核验

三处项目/worktree 状态读取统一使用 `status --porcelain=v1 -z --untracked-files=all`，使文件级声明能看到未跟踪目录中的具体文件，排除目录也按具体路径生效。`git add -A` 后再读取 `git diff --cached --name-only --no-renames -z <base>`；命中排除目录或声明范围外的路径会用 `<base>` 定向移出 index，并写入“范围外改动未并入”。

测试：

- `task08d_r1_file_declaration_finds_new_file_in_untracked_directory`
- `task08d_r1_directory_declaration_excludes_nested_cache_from_patch`
- 既有 `task08d_out_of_range_and_excluded_files_are_not_applied`

### 2. 非零 apply 的冲突分级

`git apply` 非零后读取 `git ls-files -u -z`。只有真实 unmerged 条目才返回 `conflict` 并列冲突文件；没有 unmerged 条目时返回 `skipped`，保留 Git 原始错误与 worktree 路径。

测试：`task08d_r1_non_conflict_apply_failure_is_skipped`。该用例在真实仓库中把已跟踪二进制文件替换为同名目录，使二进制补丁 3-way apply 失败且不产生 unmerged 条目。

### 3. 幂等重送

apply 成功后先执行 `git diff --cached --quiet`。无暂存差异时返回 `skipped：范围内改动已在项目中`，不再进入 commit 的 `nothing to commit` 失败分支。

测试：`task08d_r1_repeated_item_is_idempotently_skipped`。

### 4. 失效路径的未跟踪残留与定向恢复

apply 失败/超时、提交失败和验证红回退都改为按本件实际暂存路径定向 reset/checkout；验证红另外恢复验证后实际报告的脏路径。恢复后用全量 porcelain 找出本件补丁新增的未跟踪文件并定点删除，再次全量复核；仍有改动时在 note 给出人工核对指引并停止后续合并，不静默继续。整树 `checkout -- .` 与 `reset --hard` 已从该流程移除。

测试：

- `task08d_r1_conflict_cleans_new_file_and_later_item_merges`
- `task08d_validation_project_change_fails_and_rolls_back_item`
- `task08d_apply_timeout_is_skipped_and_later_item_continues`
- `task08d_restore_failure_stops_batch_with_manual_recovery_guidance`

### 5. porcelain rename/copy 解析

`parse_git_status_paths` 同时检查 X/Y 两列的 `R`/`C`，保留首个记录中的当前路径，并消费 `-z` 后随的原路径字段。

测试：`task08d_r1_status_parser_consumes_worktree_rename_origin`。

### 6. 输出截断、差集复杂度与 note 上限

worktree 改动 diff/status、暂存清单 diff、项目入口/逐件/验证后/恢复后 status 均检查 `truncated`，被截断就安全跳过或停止，不按不完整清单继续。范围外差集改为 `HashSet` 查询；路径 note 最多列 20 条，超过时写“等 N 项”。补丁本体走文件，不受进程 stdout 上限影响。

测试：

- `task08d_r1_truncated_changed_status_skips_item`
- `task08d_r1_outside_note_is_bounded`

### 7. 超过 4 MiB 的补丁

补丁生成改为 `git diff --binary --full-index --cached ... --output=<临时文件>`，项目侧以 `git apply --3way --index <临时文件>` 读取；各分支清理临时补丁文件。补丁不再经过 `ProcessResult.stdout`。

测试：`task08d_r1_large_binary_patch_is_fully_applied`，真实生成并逐字节核对 5 MiB 伪随机二进制文件。

### 8. 非 UTF-8 字节保真

与第 7 条共用文件级补丁通道，不再把补丁经 `from_utf8_lossy` 解码后作为字符串 stdin 回传。

测试：`task08d_r1_non_utf8_file_bytes_are_preserved`，断言 GBK 字节并入前后完全一致。

### 9. 逐件 apply 前脏树保护

每件生成补丁后、apply 前重新读取项目全量 porcelain。排除目录外出现改动时，本件及余下件均 `skipped`；不做回退，保留用户改动。状态读取失败或截断也停止余下合并。

测试：`task08d_r1_dirty_between_items_stops_without_losing_user_edit`。

### 10. note 分级与目录口径

合并 note 区分：`glob 无效：<pattern>`、`声明范围未匹配到改动`、`范围内没有改动`。合并侧不再用 worktree 的 `is_dir()` 判断裸声明；裸名按自身或其后代匹配，因此目录从 worktree 中删光后仍能覆盖基线树中的删除路径。

测试：

- `task08d_r1_invalid_unmatched_and_empty_ranges_have_distinct_notes`
- `task08d_r1_bare_directory_declaration_merges_full_deletion`

### 11. 裸目录删除与非法 glob

裸声明同时覆盖字面路径和 `path/**` 后代；非法 glob 单独写 note，不再静默退化成“范围内没有改动”。

测试：

- `task08d_r1_bare_directory_declaration_merges_full_deletion`
- `task08d_r1_invalid_unmatched_and_empty_ranges_have_distinct_notes`

## 测试补课

- `task08d_merge_wire_defaults_missing_declared_files_to_safe_skip` 已从单纯 serde 默认值断言扩为真实 repo/worktree 合并，确认返回 `skipped：未声明文件范围` 且文件未并入。
- 既有 `task08d_in_range_conflict_restores_clean_project_and_keeps_first_item` 新增冲突文件名、worktree 路径和全量 porcelain 为空的断言。
- `task08d_r1_conflict_cleans_new_file_and_later_item_merges` 覆盖“新建文件 + 冲突文件同补丁”、恢复后无未跟踪残留、item1 merged / item2 conflict / item3 merged，并在下一批继续并入 item4，证明入口未被残留卡住。
- `task08d_validation_cleans_tracked_excluded_changes`、`task08d_validation_project_change_fails_and_rolls_back_item` 与上述冲突测试均使用 `--untracked-files=all` 断言项目干净，不再隐藏未跟踪残留。

## 没做与边界

- 没有修改 `src/process.rs`：文件补丁通道已在 `dispatch.rs` 内通过 Git 的 `--output` 与文件参数完成，不需要扩大进程层接口。
- 没有更改既有验证命令超时语义；仍按任务“保持”部分归 `test_failed`。
- 没有运行前端测试或前端构建：本轮明确只改 Rust 合并实现与其测试，硬性验证项是后端 `cargo check --all-targets` 和 `cargo test`。
- 没有运行 `cargo fmt`，没有启动桌面窗口或调用真实模型。

## 验证命令与结果原文

### `cargo check --all-targets`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.24s
```

输出中没有 warning。

### `cargo test --quiet`

```text
running 347 tests
...
test result: ok. 336 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 16.14s
```

11 个 ignored 均为仓库既有的真机/真实模型或显式环境验收项；本轮未启用这些外部请求测试。

### `git diff --check`

```text
```

退出码 0，无空白错误。另以字节检查确认 `src/dispatch.rs` 为 0 个 CRLF、仅 LF，未发生换行风格翻面。

## 复审后补记（编排者，763b21b）

DeepSeek 复审第 1 条：恢复后复核未过滤排除目录，验证命令留下的 `__pycache__` 之类被当成「恢复失败」，`test_failed` 被降成 `skipped` 并整批停下。编排者在 `restore_swarm_paths` 的复核里加 `!excluded_swarm_path` 过滤（与逐件脏检查同口径），新增 `task08d_r1_validation_red_with_cache_residue_stays_test_failed`（真造 `__pycache__` 残留 + exit 9，断言 `test_failed` 且后件 `merged`）。回退该过滤后此测试红，恢复后绿；`cargo test` 337 passed / 0 failed / 11 ignored。GLM 复审（`tasks/review-08d-r1-glm.md`）核过该提交判正确。两家复审剩余低危项记入 `tasks/fix-04-r5-backlog.md` 第 21–26 条。
