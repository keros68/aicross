# TASK-14 R1 完成报告

## 结果

`fix-14-r1.md` 的 13 条必修均已完成。修正覆盖额度识别、首次验证、可写节点回滚与备选、恢复时间解析和持久化、资源刷新与手动清除、G1 收敛、状态文案以及派发留痕。未启动应用窗口，未请求真实模型，未修改 `src/terminal.rs`，未提交或推送。

## 逐条修正

1. **收紧额度识别。** 数字状态只接受 `HTTP`、明确的 `status` / `code` 字段或 `api_error` 上下文；`resets` 必须带时间语义；中文额度词必须与用尽语义同句。派发层只在 `status=error` 时检查 stderr 与解析错误，不读取 stdout，`identity_mismatch` 不改写。测试：`quota_classifier_rejects_unmarked_status_reset_prose_and_loose_chinese_words`、`quota_classifier_covers_status_text_balance_and_plain_errors`、`task14_r1_quota_classification_only_accepts_error_fields_and_never_identity_mismatch`。
2. **分开保存每次 raw 输出。** `record_stdout` 接受生产标签“备 N”，生成 `<node>.raw.备N.txt`；frontmatter 的 `attempt` 只写“主／备”，新增 `attempt_index` 表示实际尝试序号。测试：`task14_r1_production_backup_labels_keep_all_raw_dumps_separate`、`task14_quota_fallback_order_runs_fake_channel_through_three_attempts`。
3. **首次验证识别额度。** 第三方未验证模型的首次验证遇到额度错误时保持 `unverified`，把恢复时间交给派发层并继续备选，不进入主通道调用。测试：`task14_r1_first_use_quota_keeps_model_unverified`、`task14_r1_first_use_quota_switches_to_fallback_without_entering_primary_channel`。
4. **可写节点安全切换备选。** 单写者与 worktree 的额度失败先回滚本次改动再切备选；回滚失败时停止切换并写明原因；非额度失败仍不切。G1 在显式备选或资源页 `fallback_order` 生效时显示回滚提示。测试：`task14_r1_writable_quota_rolls_back_before_fallback`、`task14_r1_writable_quota_stops_when_rollback_fails`、`writable_non_quota_failure_does_not_switch_fallback`、`writableFallbackHintOf：可写节点配了备选才提示`。
5. **统一额度状态。** 后端 UI 状态、`STATE.md`、图数据、图节点卡、结果卡、蜂群终态与重派入口均单列“额度用尽”；额度回滚失败显示“额度用尽，未切备选（回滚失败）”。测试：`state_uses_actual_limit_and_separates_failures`、`TASK-14 R1 图结果保留额度状态、恢复时间与节点状态词`、`fallbackSkippedLineOf：fallbackSkipped 非空时给「主失败，未切备选（可写节点）」`、`B5 全部 swarm 节点终态才出合并按钮`。
6. **通知资源刷新并支持清除。** 额度状态成功写回资源库后发出 `aicross:resources-changed`；应用级桥接复用现有前端刷新事件。接入页受限模型新增“清除”，通过原子写通路清空 `quota_exhausted_until`。测试：`Rust 写回额度后通过单一桥接通知现有资源监听方`、`受限模型提供清除动作，普通模型不显示`、`task14_fallback_order_roundtrips_and_quota_expiry_is_cleared`。
7. **修正 G1 异步时序。** 模型清单或受限节点集合变化后，重新收敛默认选中项；用户手动操作过的受限节点保持其选择。测试：`TASK-14 R1 G1 模型异步更新后只收敛用户未手动改过的节点`、`TASK-14 G1 默认排除恢复时间未到的模型`。
8. **持久化恢复时间。** `orchestration::Reply` 追加带默认值的 `quota_resets_at`，兼容旧图；派发结果写回图后，持久化结果卡直接从 Reply 显示恢复时间。测试：`task14_r1_reply_quota_reset_roundtrips_and_defaults_for_old_graphs`、`TASK-14 R1 图结果保留额度状态、恢复时间与节点状态词`、`显示独立状态与恢复时间`。
9. **扩展恢复时间解析。** 先解析 reset/retry 语义，再使用 ISO 回退；支持管道后的 epoch 秒、`Retry-After` 秒数和 HTTP-date、`retry-after` / `retry_after` / `retry after`，以及 `resets 1:30pm` 的本地时刻和跨日。测试：`quota_classifier_parses_retry_after_with_real_time_window`、`quota_classifier_parses_epoch_http_date_and_prefers_reset_semantics_over_iso`、`quota_classifier_resets_clock_uses_local_1330_and_rolls_past_time_to_tomorrow`。
10. **纳入被跳过候选。** 已处于额度限制期的备选不调用，但其时间进入最早恢复时间计算，runner note 写明跳过的 route 与时间。测试：`task14_r1_skipped_candidate_reset_is_returned_when_no_attempt_has_one`、`task14_quota_fallback_order_runs_fake_channel_through_three_attempts`、`task14_reset_helpers_choose_earliest_active_time`。
11. **补足有效负例和时间窗口断言。** 429 正例使用纯文本而非 `http_status` 参数；60 秒和 120 秒 retry 使用上下界验证；终态测试同时断言 `quota_exhausted` 和最早时间。相关测试即第 1、9、10 条所列测试，以及 `task14_exhausted_fake_channels_return_quota_not_error`。这些测试分别约束生产分支，回退对应修复会失败。
12. **统一术语。** 界面与手册统一使用“恢复时间”；资源页行内状态仍为“额度用尽至 HH:MM”；术语表新增对应行。测试：`TASK-14 R1 统一使用恢复时间术语`、`显示独立状态与恢复时间`。
13. **说明退出码并按实际尝试编号。** `classify_quota` 注明保留 `exit_code` 但不据此分类，因为各 CLI 退出码不稳定；`attempt_index` 只在实际调用时递增，被跳过候选不造成断号。测试：`task14_quota_fallback_order_runs_fake_channel_through_three_attempts` 验证实际三次调用及两个连续备选序号，`task14_r1_skipped_candidate_reset_is_returned_when_no_attempt_has_one` 验证跳过项不产生调用。

## 验证

- `cargo check --all-targets`：通过；无 warning。
- `cargo test`：通过；362 passed，0 failed，11 ignored。忽略项均为仓库原有真机或真实模型测试。
- `cd src-ui && npx vitest run --configLoader native --pool threads`：通过；28 个测试文件、438 个测试全部通过。
- `cd src-ui && npm run build`：通过；TypeScript 构建与 Vite 生产构建成功。控制台只有环境变量 `NO_COLOR` / `FORCE_COLOR` 的 Node 提示，不是编译 warning。
- `git diff --check`：通过。
- 换行检查：`src/server.rs` 仍为 4666 个 CRLF、0 个裸 LF；`src-ui/src/tauri.ts` 仍为 576 个 CRLF、103 个裸 LF。两文件均未修改。

## 未做与边界

- 没有未完成或部分完成项。
- 未运行被忽略的真机／真实模型测试；本轮按要求只使用本地假通道和单元／集成测试。
- 未运行 `cargo fmt`，未启动 Tauri 窗口，未修改 `src/terminal.rs`，未 commit，未 push。

## 复审后补记（编排者，aa81270）

两家复审（`tasks/review-14-r1-*.md`）都判可交付。DeepSeek 新发现第 1 条（高）编排者直接修：可写节点撞额度时的回滚原本复用 `rollback_node_changes_core`，它按节点取「历史最新快照」——本次尝试没改动时判「回滚失败」不切备选，且可能把上一次派发的产物和用户手改一起还原。改成只认本次尝试的快照（`DispatchArtifacts.before_dir` 记录本次落下的快照目录；没有就「无需回滚」照常切；有就按该目录恢复，抽出 `restore_before_dir`），新增 `task14_r2_writable_quota_without_changes_falls_back_without_rollback` 与 `task14_r2_quota_rollback_ignores_stale_snapshot_from_earlier_run`，回退到旧查找后两条都红。顺带恢复 05B 的「可写节点」跳过标记（非额度失败）。改后全树 `cargo test` 364 passed、vitest 438、build 通过。其余低危记入 `tasks/fix-04-r5-backlog.md` 第 33–36 条。
