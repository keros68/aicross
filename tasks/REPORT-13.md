# REPORT-13 红绿核对

## 完成内容

1. 契约只追加
   - `DispatchNodeRequest` 追加 `fix_round`，IPC 使用既有 camelCase 规则传为 `fixRound`，缺失时为 `false`。
   - `DispatchNodeResult` 追加可选的 `pinned`、`pinned_note`，前端对应 `pinned`、`pinnedNote`。
   - `aicross-dispatch/1` 派发记录 frontmatter 追加 `pinned`；旧记录缺失时按空值读取。

2. 自动红绿核对
   - 仅在修正轮、执行角色、`单写者` 或 `worktree`、验证命令非空且第一次验证通过时触发。
   - 根据本次 `before_dir` 快照和前后目录快照确定本次 diff 文件；临时回退、恢复和最终恢复只处理这些文件。
   - 回退后失败且恢复后通过标 `pinned`；回退后仍通过标 `unpinned`；快照缺失、不完整、文件内容未保存、写锁未取得或验证超时标 `skipped`，不改变节点成功状态。
   - 恢复后验证失败标 `validation_failed`，说明为「红绿核对恢复后验证未通过」。核对结束再次恢复本次改动的字节内容。
   - 单写者沿用整次派发已持有的项目写锁；worktree 在自己的目录内取得节点写锁后核对。普通 worktree 节点仍保留“合并后再验”的原行为。

3. 流水线与结果显示
   - `reducePipelineStep` 将 `unpinned` 与 `validation_failed` 一起计入 `fixRoundLimit`；未到上限时生成修正节点，到上限时停止继续派发并关闭等待中的验收。
   - 未钉住产生的修正提示词以「上一轮修复回退后测试仍通过，请补能失败的测试」收尾。
   - 红绿结论写入执行节点 Reply，后续验收上下文可收到该结论。
   - 结果卡状态旁显示「测试已钉住 / 测试未钉住 / 未核对」；`unpinned` 使用警告色并显示 `pinnedNote`。

## 新增测试

Rust：

- `task13_changed_function_with_regression_test_is_pinned`
- `task13_changed_function_without_regression_test_is_unpinned`
- `task13_missing_before_snapshot_is_skipped`
- `task13_red_green_restores_exact_bytes_and_leaves_unrelated_file_alone`
- `task13_red_green_timeout_is_skipped_and_restored`
- `task13_restored_validation_failure_changes_status_outcome`
- `task13_wire_defaults_and_frontmatter_are_append_only`
- `task13_engine_single_writer_fix_without_test_returns_unpinned_record`
- `task13_engine_worktree_fix_with_test_runs_and_restores_inside_worktree`

其中引擎级测试真实创建临时 git 仓库；worktree 测试真实创建登记 worktree，并确认改动与核对均发生在 worktree 内，项目目录保持原值。

前端：

- `修正轮请求追加 fixRound，普通执行请求不追加`
- `两种 reducePipelineStep 修正节点都识别为修正轮`
- `未钉住按验证未通过计入上限，修正提示词以补失败测试收尾`
- `红绿结论进入 Reply 供下游验收读取，三态标签固定`

## 验证

- `cargo check --all-targets`
  - `Finished dev profile [unoptimized + debuginfo] target(s)`，退出码 0，零 warning。
- `cargo test`
  - `test result: ok. 391 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out`。
- `cd src-ui && npx vitest run --configLoader native --pool threads`
  - `Test Files 30 passed (30)`；`Tests 472 passed (472)`。
- `cd src-ui && npm run build`
  - `tsc -b && vite build`；`334 modules transformed`；`built in 512ms`，退出码 0。
- `git diff --check`
  - 退出码 0，无空白错误。
- 换行复核
  - `src/dispatch.rs`：CRLF 0，LF 8842。
  - `src/server.rs`：CRLF 4666，LF 0，未修改。
  - `src-ui/src/tauri.ts`：CRLF 576 保持不变；LF 从 107 增至 112，仅新增 5 行。

## 未做与边界

- 未启动 Tauri 窗口，未做窗口手测；任务明确要求不起窗口。
- 未发送真实模型或真机请求；测试使用本地假通道和临时仓库。
- 未运行 `cargo fmt`，未修改 `src/terminal.rs`。
- 未 commit，未 push。
