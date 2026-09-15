# REPORT-14 额度用尽识别与按优先级自动切备选

## 1. 契约与额度识别

- `src/channels/mod.rs` 新增纯函数 `classify_quota` 与 `QuotaHit`。支持 HTTP 429、带限额文案的 HTTP 403、`session limit`、`usage limit`、`resets`、额度、限额、配额和 `Insufficient Balance`；DeepSeek 402 归为 `balance`。
- 重置时间支持响应中的 ISO 8601、`resets 1:30pm` / `resets at 1:30pm` 和 `retry after N`（秒或分钟），成功解析后统一返回 ISO 8601。
- 一次性 CLI、ACP 与 cc-switch 的错误结果在 `run_one_attempt` 汇合后都经过同一识别函数；命中后返回 `quota_exhausted`，`DispatchNodeResult` 追加 `quota_resets_at`。
- 测试：`quota_classifier_covers_status_text_balance_and_plain_errors`、`quota_classifier_parses_retry_after`、`fallback_predicate_only_triggers_on_channel_failures`。

## 2. 按顺序切备选

- `run_one` 从一次备选改为有序循环：显式 `Target.fallback` 优先，额度仍用尽时继续读取主模型的 `fallback_order`。
- 循环跳过主目标、重复目标和 `quota_exhausted_until` 尚未到期的目标；保留原有“可写节点不自动重跑”边界。
- 每次尝试各写一条派发记录；最终结果的 `fallback_from` / `fallback_to` 记录最后一跳，`runner_note` 累积整条额度切换链。全部用尽时保持 `quota_exhausted`，`quota_resets_at` 取已知时间中最早值。
- 测试：`task14_quota_fallback_order_runs_fake_channel_through_three_attempts` 真跑 Python 假通道，覆盖显式备选优先、跳过未恢复目标、三次尝试与最后一跳；`task14_exhausted_fake_channels_return_quota_not_error` 覆盖全部用尽与最早恢复时间；`task14_all_quota_attempts_keep_status_and_earliest_reset` 覆盖时间比较。

## 3. resources.json 与资源页

- `ResourceModel` 只追加 `fallback_order` 和 `quota_exhausted_until`；旧文件缺字段时分别默认为空数组和空值。`ResourceModelRequest.fallback_order` 缺失时保留原顺序，显式空数组可清空。
- 探测刷新保留两个新字段；资源读取会清理并原子写回已过期的 `quota_exhausted_until`；引擎识别到可解析的恢复时间后写回对应模型。
- 接入页模型详情新增“备选顺序”：可从全部资源模型中加入、上移、下移、移除，并通过既有更新命令存回资源库。额度未恢复的模型行显示“额度用尽至 HH:MM”；左栏悬停说明也带同一状态。
- 测试：`task14_fallback_order_roundtrips_and_quota_expiry_is_cleared` 覆盖新旧 JSON、往返、探测字段保留、过期清理和额度写回；前端 `TASK-14 备选顺序组件` 两个用例覆盖顺序渲染与相邻移动；`TASK-14 quotaExhaustedUntil` 覆盖未来与过期时间。

## 4. 确认屏与结果卡

- G1 初始选择会排除额度恢复时间未到的目标，单列提示并保持可手动勾选；用户改选到受限模型时，路由卡同步提示。
- 结果卡追加“额度用尽”状态、恢复时间和累计切换说明；移交文字不把 `quota_exhausted` 写成普通失败。
- 前端图结果契约追加 `quota_resets_at`，旧图与旧后端缺字段时不受影响。
- 测试：`TASK-14 G1 默认排除恢复时间未到的模型`、`TASK-14 额度结果组件`。

## 5. 验证结果

```text
cargo check --all-targets
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.57s
```

零 warning。

```text
cargo test
test result: ok. 352 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out
```

忽略项均为仓库既有的真机或真实模型请求测试。

```text
cd src-ui && npx vitest run --configLoader native --pool threads
Test Files  28 passed (28)
Tests  433 passed (433)
```

```text
cd src-ui && npm run build
334 modules transformed
✓ built in 631ms
```

`NO_COLOR` / `FORCE_COLOR` 提示来自现有测试环境，不影响测试或构建结果。

## 6. 没做与边界

- 未启动窗口，未发送真实模型请求；本期通道循环用本地假脚本验证。
- 未运行 `cargo fmt`，未修改 `src/terminal.rs`，未修改 `src/server.rs`。
- `src/server.rs` 保持 4666 个 CRLF、无裸 LF；`src-ui/src/tauri.ts` 原有 576 个 CRLF 和 101 个裸 LF 保持不变，只新增 2 个裸 LF，属于字段附近的字节级追加。
- 未 commit，未 push。
