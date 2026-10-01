# REPORT-41 智谱 coding plan 的额度上限认作额度用尽

实现：Claude 子代理（10-01）。子代理环境不允许创建报告文件，本文由编排者按其交回的全文写入。

## 改了什么

只改 `src/channels/mod.rs`（LF 换行保持不变，未运行 cargo fmt；未 commit、未 push）。

- 第 76–83 行 `classify_quota`：状态码识别新增一条，行首（允许前导空格或 tab）出现 `429:` 视同 429 状态，正则 `(?m)^[ \t]*429[ \t]*:`。用多行模式，是因为 `dispatch.rs` 把 stderr 与错误文本按行拼接后再分类。
- 第 100 行 `classify_quota`：中文规则新增两种表述，都限定在同一句内（不跨 。！？ 与换行）：
  - `(?:已达|达到)[^。！？\r\n]{0,12}上限`，覆盖「已达到 5 小时的使用上限」「达到上限」；
  - `(?:额度|限额|配额)[^。！？\r\n]*将[^。！？\r\n]*重置`，覆盖「您的限额将在 … 重置」。
- 第 166–173 行 `parse_quota_reset`：新增 `LOCAL_DATETIME` 分支，解析「将在 / 将于 YYYY-MM-DD HH:MM:SS 重置」这类不带时区的时间，按 `Local` 解释（`from_local_datetime(..).earliest()`，与 `CLOCK` 分支一致），输出 RFC 3339。位于 `CLOCK` 之后、`ISO` 之前。
- 第 1085–1125 行：两个新测试。

## reason 取值

上述原文判为 `rate_limit`：响应本身是 HTTP 429，现有规则里 429 一律映射 `rate_limit`，沿用同一规则不另开例外。下游只用「是否命中」与 `resets_at` 决定切备选、标恢复时间，没有按 reason 分支。没有状态码、只靠中文表述命中时，按现有规则给 `quota`。

## 新增测试

- `channels::tests::task41_zhipu_coding_plan_usage_cap_is_quota_hit_with_local_reset_time`：原文与「前面多一行」两种形式都判为 `rate_limit`，`resets_at` 与本地时间 2026-10-01 13:30:41 一致；只有「已达到…使用上限」或只有「限额将在…重置」时判为 `quota`，后者带正确的恢复时间。
- `channels::tests::task41_quota_classifier_rejects_cap_word_bare_1429_and_unrelated_reset`（反例）：以下文本都不判额度——「上限参数必须为正整数」「使用上限为 100 次，可在配置中修改」；`1429: unknown error`、`error at src/lib.rs:1429: unexpected token`、`line 429: syntax error`、`worker returned 429: records`；「请重置密码后再试」「密码将在 2026-10-01 13:30:41 重置」「连接将在 2026-10-01 13:30:41 重置。」

## 回退验证

三处修复分别改回后运行 `cargo test task41`，正例测试每次都失败：全部改回时「未认出」；只改回时间解析时 `resets_at` 为空；只改回行首 `429:` 时 reason 变为 `quota`。之后已恢复。反例测试在修复前后都通过，作用是防止规则放得过宽。

## 验证

- `cargo check --all-targets`：零 warning。
- `cargo check --release`：通过。
- `cargo test`：586 passed，0 failed，12 ignored。

## 没做的与不确定处

- 行首 `429:` 只认 429，不扩展到 402 / 403。
- 「超出 / 超过上限」不在新规则里，如需要后续可补。
- 恢复时间按本机时区解释，前提是智谱返回东八区时间；本机不在东八区时会有偏差。
- 未做真机复测，只按 10-01 记录的原文做了单元测试。

## 编排者追加

「已达 / 达到 … 上限」原规则会把「已达到最大重试次数上限」这类普通报错误判为额度用尽（会触发切备选并把模型标成额度用尽），收紧为 `(?:已达|达到)[^。！？\r\n]{0,12}(?:使用|用量|调用)上限`，并在反例测试里补「已达到最大重试次数上限，请检查网络后重试」。回退验证：放宽回原规则时反例测试变红；智谱原文仍判为 `rate_limit`。
