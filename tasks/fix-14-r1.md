# fix-14-r1 额度用尽识别与备选（TASK-14）复审修正轮

依据 GLM-5.3-Flash 与 DeepSeek 对 phase/14 的盲审（`tasks/review-14-glm.md`、`tasks/review-14-ds.md`）。编排者已定的取舍写在各条里。契约只追加。

## 必修（两家共同：1/2/3/5/7/10；DeepSeek 独有：3/4/6/8/9/11；GLM 独有：12/13）

1. **识别过宽（DeepSeek 1 / GLM 1、2，编排者核过）**：`classify_quota` 的 embedded status 正则前缀全可选，等于「正文里出现独立数字 402/403/429」；`resets` 单词、「额度/限额/配额」也过泛；调用方把 `raw_stdout`（整段事件流，含工具读到的文件内容）一起送进来。修法：① 数字状态只在明确上下文认（`status[: =]`、`"status":`、`HTTP <code>`、`code[: =]`、`api_error`，前缀不再可选）；② `resets` 收紧为 `resets? (at|in) ` 或 `resets \d`；「额度/限额/配额」要求同句含「用尽/不足/已达/超出/耗尽」之一；③ 输入只取 stderr + 解析出的 error 文本，不送 stdout（GLM 1：identity_mismatch 与解析失败时 stdout 是模型正文）；④ 只对 status 为 error 的结果做分类，`identity_mismatch` 一律不改写。负例测试：`src/lib.rs:429:5`、`server resets the connection`、403 无 marker、普通 error、正文里含「额度」但不是错误。
2. **备选转存覆盖主尝试（DeepSeek 2）**：`record_stdout` 只认精确 `"备"`，循环传的是 `"备 1"/"备 2"`，raw 转存 suffix 落空互相覆盖。修法：`attempt.starts_with("备")` 并把序号进文件名（`<node>.raw.备1.txt`）；留痕 frontmatter `attempt` 保持「主 / 备」两值，序号放新字段 `attempt_index`。测试：生产标签 `"备 2"` 的转存与主尝试并存。
3. **第三方首次验证撞额度（DeepSeek 3）**：`verify_on_first_use_at` 失败分支过 `classify_quota`；命中时不写 `verification=failed`（保持 unverified），把额度信息带回 `run_one_attempt` 使 status 判 `quota_exhausted` 并照常走备选。测试：unverified 模型首派 429 → quota_exhausted + 切备选 + 库中仍 unverified。
4. **可写节点也切备选（DeepSeek 4，编排者裁决：按用户「额度用尽自动备选」执行）**：单写者 / worktree 节点撞额度时，先用派发前快照回滚本次改动（既有 `rollback_before` / `rollback_node_changes_core` 通路），再切下一家；回滚失败则不切、note 写原因。非额度类失败（error/timeout/identity_mismatch）保持 05B 语义不切。G1 对「资源页配了 fallback_order 的可写节点」给一句提示。测试：单写者节点主目标 429 → 回滚后备选 merged；回滚失败 → 不切。
5. **状态词统一（DeepSeek 5 / GLM 4、5）**：`fallbackSkippedLineOf` 对额度用尽说「额度用尽，未切备选」而不是「主失败」；`ui_status`、STATE.md 分区、前端 `nodeStatusLabel` / `applyDispatchResults` 都给 `quota_exhausted` 一个「额度用尽」分支，不再落「失败」；图节点卡同词。
6. **引擎写回通知前端（DeepSeek 6）**：`mark_quota_exhausted` 成功后 Rust 发事件（复用 resources changed 的事件名或新增 `aicross:resources-changed`），前端刷新模型清单；接入页受限模型加「清除」动作（手动清 `quota_exhausted_until`）。
7. **G1 默认不勾选的时序（DeepSeek 7）**：`quotaBlockedIds` 变化时对用户未手动改过的节点收敛一次；补接线断言。
8. **`Reply.quota_resets_at` 落盘（DeepSeek 8）**：`orchestration::Reply` 加 `#[serde(default)] quota_resets_at: Option<String>`，补往返测试；结果卡从图上读它显示重置时间。
9. **重置时间解析（DeepSeek 9）**：先解析 `resets/retry` 语义再退回 ISO；补 `usage limit reached|<epoch 秒>`、`Retry-After: N` / HTTP-date、`retry[-_ ]?after`；`resets 1:30pm` 断言具体时分与「过去→次日」。
10. **被跳过候选并入最早恢复时间并写 note（DeepSeek 10）**：跳过的候选把 `quota_exhausted_until` 并入 `quota_resets`，note 写「跳过 <route>（额度用尽至 …）」。
11. **测试补课（DeepSeek 11）**：429 用纯文本不传 http_status；`retry after` 断言 ≈ now+60s 窗口；`task14_all_quota_attempts_keep_status_and_earliest_reset` 改名或补 status 断言；各条负例。

12. **术语统一（GLM 9）**：「恢复时间」「重置时间」「额度用尽至」指同一时间戳，界面与手册统一为「恢复时间」（资源页行内保留「额度用尽至 HH:MM」作为状态短语），术语表加一行。
13. **杂项（GLM 9）**：`classify_quota` 的 `exit_code` 参数要么用上（如 CLI 以特定退出码表示限额）要么在注释里写明保留原因；备选序号按实际尝试顺序编号，不按候选下标跳号。

## 保持

- 显式 `Target.fallback` 优先于 `fallback_order`、`seen` 去重、`retry.fallback=None` 不递归、resources 读写走同一写锁 + 原子写、`update_model` 不动 `quota_exhausted_until`——核过没问题，不动。

验证：`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。保持文件原有换行（`src/server.rs` CRLF；`src-ui/src/tauri.ts` 混合只做字节级最小改动）；不 `cargo fmt`；不起窗口、不发真机请求；不 commit、不 push。报告写 `tasks/REPORT-14-R1.md`（逐条对应本清单）。
