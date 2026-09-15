# REPORT-13-R1 红绿核对复审修正

## 逐条完成情况

1. **恢复失败不再留下静默半回退**：`restore_snapshot_paths` 改为逐路径尽力恢复并汇总失败；重放失败时节点改为 `error`，`error` / `pinned_note` 写明失败路径与手工核对提示，结果卡用警告色显示。测试：`task13_restore_failure_is_best_effort_and_maps_to_error`，以及前端组件断言 `PinnedNote` 的 `dispatch-warning`。
2. **核对中断可对账**：临时回退前写 `.dispatch/<run>/<node>.redgreen`，记录 `before_dir`、`changed` 和 `red` / `green` 阶段；正常结束删除。`reconcile_dispatch_state` 发现标记时优先返回 `unknown`，说明工作区可能停在回退状态并给出快照路径。测试：`task13_interrupted_marker_makes_reconcile_request_review`、`task13_changed_function_with_regression_test_is_pinned`。
3. **未钉住文案分流**：未钉住时，等待验收写「测试未钉住，未送审」，验证兄弟与到顶节点写「上游测试未钉住」；`closeWaitingAcceptance` 接收原因。设置页补充测试未钉住也计入上限，手册补红绿核对术语。测试：`未钉住按验证未通过计入上限，修正提示词以补失败测试收尾`、`设置页完整说明共用上限与到顶已跳过`。
4. **恢复失败与验证失败分开**：恢复失败使用「红绿核对恢复本次改动失败……」并置 `error`；只有恢复成功后连续两次验证失败才置 `validation_failed`。测试：`task13_restore_failure_is_best_effort_and_maps_to_error`、`task13_restored_validation_failure_changes_status_outcome`。
5. **超时分支分开**：红跑超时恢复后直接 `skipped`，不再跑绿；绿跑超时单独写「恢复后验证命令超时，未核对」，非超时绿失败不会被红超时文案遮蔽。测试：`task13_red_green_timeout_is_skipped_and_restored`、`task13_green_timeout_is_distinct_from_red_timeout`。
6. **绿跑失败复核一次**：恢复后的第一次非零退出会原命令再跑一次，两次都失败才置 `validation_failed`；`pinned_note` 写明红、绿实际运行次数，验证输出区分绿跑第 1 / 2 次。测试：`task13_green_failure_retries_once_and_can_recover`、`task13_restored_validation_failure_changes_status_outcome`。
7. **核对锁、超时与叫停**：核对独立超时为 `min(节点超时, 300s)`；进入核对前和红绿之间检查取消状态，验证子进程带节点取消键。测试：`task13_red_green_timeout_is_capped_at_five_minutes`、`task13_cancel_during_red_run_skips_green_run`。
8. **保留原有空目录**：目录快照记录目录项，`.created` 同时登记本轮新建目录；回退只删除本轮新建目录，不再递归清理所有空父目录。测试：`task13_preexisting_empty_directory_survives_red_phase`。
9. **零改动单独说明**：本轮无文件差异时返回 `skipped` 和「本轮没有文件改动，未做核对」。测试：`task13_zero_change_reports_its_real_reason`。
10. **链接处理一致**：恢复路径与回退路径都先删除目标链接或联接点，再按快照重建；失败进入逐路径恢复清单。测试：非 Windows 的 `task13_restore_replaces_symlink_without_following_it`。
11. **前端测试补课**：增加 `DispatchOverlay` 的 `fixRound: isFixRoundExecution(sourceGraph, id)` 接线断言；三态标签与说明抽到 `DispatchBits.tsx` 的 `PinnedStatus` / `PinnedNote`，使用 `renderToStaticMarkup` 检查三态、警告类名和接线；Rust 引擎增加普通轮、空验证命令、非执行角色负例。测试：`修正轮请求追加 fixRound，普通执行请求不追加`、`红绿结论进入 Reply 供下游验收读取，三态标签固定`、`task13_trigger_conditions_have_engine_level_negative_cases`。
12. **记录与术语杂项**：派发记录的空 `pinned` 不再序列化；因第 15 条改为枚举后使用 `Option<PinnedState>` 的 `None` 跳过，仍兼容首版记录的 `pinned: ''`。保留 `[红绿核对]` 并登记术语表，前端测试文件头补 TASK-13。测试：`task13_wire_defaults_and_frontmatter_are_append_only`，以及前端手册断言。
13. **启动失败不算红**：验证结果区分正常非零退出与启动失败；不存在的命令返回 `skipped`，不再产生假 `pinned`。测试：`task13_missing_validation_program_is_not_a_red_test`。
14. **结构化超时**：`run_validation` 返回 `ValidationVerdict`，红绿逻辑按 `TimedOut` 枚举判断，不再匹配错误文本前缀。测试：`task13_validation_timeout_is_a_structured_verdict`。
15. **权限位与三值枚举**：Unix 快照记录 mode，回退和恢复重建后重新设置权限；Rust 的 `pinned` 改为 `PinnedState::{Pinned, Unpinned, Skipped}`，serde 仍输出小写字符串，非法值反序列化失败。测试：非 Windows 的 `task13_restore_preserves_executable_mode`、`task13_wire_defaults_and_frontmatter_are_append_only`。

## 验证

- `cargo check --all-targets`：退出码 0，零 warning。
- `cargo test`：402 passed，0 failed，11 ignored；忽略项均为仓库既有真机或已登录环境测试。
- `cd src-ui && npx vitest run --configLoader native --pool threads`：30 个测试文件通过，472 项通过。
- `cd src-ui && npm run build`：`tsc -b && vite build` 通过，334 modules transformed。
- `git diff --check`：通过。
- 换行复核：`src/dispatch.rs` 保持全 LF；`src/server.rs` 保持全 CRLF且未修改；`src-ui/src/tauri.ts` 保持原混合换行且未修改。

## 未做与边界

- 15 条必修均已实施，没有部分完成项。
- 当前 Windows 环境实际运行了 Windows 恢复失败负例；Unix 的链接与权限位测试已写入 `#[cfg(unix)]`，本轮未在 Unix 主机执行。
- 未修改 `src/server.rs`、`src/terminal.rs`、`src-ui/src/tauri.ts`，避免与 phase/16 交叉。
- 未运行 `cargo fmt`；未启动窗口；未发送真实模型或真机请求；未 commit、未 push。
