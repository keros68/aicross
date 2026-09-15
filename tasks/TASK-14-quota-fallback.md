# TASK-14 额度用尽识别与按优先级自动切备选（09-15 定案）

依据 `docs/PRODUCT.md`「第 4 期草案」第 4 条与用户裁决：额度用尽自动切备选，备选顺序由用户设定。改动在通道层、引擎备选循环、资源页；契约只追加。

## 契约（只追加）

- 通道结果 status 追加一个值 `quota_exhausted`（与 `error` / `timeout` / `identity_mismatch` 并列）；`BackendNodeResult` 追加 `quota_resets_at?: string`（ISO 8601，解析得到才填）。识别规则集中在 `src/channels/mod.rs` 一个纯函数 `classify_quota(stderr_or_body: &str, exit_code, http_status) -> Option<QuotaHit>`：HTTP 429；HTTP 403 且正文含 rate limit / quota / usage limit；文本含「session limit」「usage limit」「resets」「额度」「限额」「配额」「Insufficient Balance」（DeepSeek 402 也算，标 `reason: balance`）。能解析出重置时间（「resets 1:30pm」「resets at」「retry after N」）就填 `resets_at`。
- `ResourceModel` 追加 `fallback_order: Vec<String>`（serde default 空）：该模型额度用尽时按顺序尝试的模型 key 列表（`agent/provider/model` 三元组字符串化），用户在资源页排；追加 `quota_exhausted_until: Option<String>`。
- `Target.fallback` 语义不变（显式备选优先）；没设显式备选时，引擎按 `fallback_order` 逐个尝试。

## 实现

1. **识别**：三家一次性 CLI 与 ACP、cc-switch 路径的错误输出都过 `classify_quota`；命中即 status `quota_exhausted`，不算 error。`should_try_fallback` 把 `quota_exhausted` 加进触发条件。
2. **按顺序切**：`run_one` 的备选循环改成列表：先显式 `Target.fallback`，再 `fallback_order` 里的每一项（跳过与主目标相同的、跳过资源页标记为额度用尽且未到重置时间的）；每切一次结果里 `fallback_from/to` 记最后一跳，note 累积「主 X 额度用尽 → 备 Y 额度用尽 → 备 Z」。全部用尽：status 保持 `quota_exhausted`，`quota_resets_at` 取最早的重置时间，不标失败。
3. **资源页**：每个模型行悬停/详情加「备选顺序」编辑（从同 agent 或全部可派发模型里多选并排序，存回 `resources.json`）；额度用尽的模型显示「额度用尽至 HH:MM」（读 `quota_exhausted_until`，过期自动清）；引擎命中时写回该字段。
4. **界面**：结果卡状态「额度用尽」+ 重置时间；G1 确认屏对额度用尽且未到时间的模型给提示并默认不勾选。
5. **测试**：`classify_quota` 对 429 / 403 文案 / 「session limit · resets 1:30pm」/ DeepSeek 402 / 普通 error 各一条；备选循环真跑假通道（fixtures 里的 fake server 或 mock）走完三跳；`fallback_order` 往返与旧 `resources.json` 缺字段默认；资源页排序组件测试。

验证同 TASK-12。不动 `src/terminal.rs`；不 `cargo fmt`；保持文件原有换行；不起窗口、不发真机请求；不 commit、不 push。报告写 `tasks/REPORT-14.md`。
