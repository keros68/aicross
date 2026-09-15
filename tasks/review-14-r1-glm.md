# TASK-14 fix-14-r1 复审（只读，13 条逐项）

**前提声明**：本会话无 shell，未运行 cargo/vitest，全部结论来自对当前工作区文件的静态核读。审查过程中 `src/dispatch.rs` 行号发生了约 +78 行的漂移并被继续编辑（出现了 REPORT 未记载的 `task14_r2_*` 测试与改写后的 `rollback_quota_attempt`），下列行号以最后一次读取的文件状态为准；REPORT-14-R1 中的验证结论（cargo test 362 / vitest 438）早于这批 R2 改动，我无法复核当前树是否仍全绿。

## 总判

**可交付，但有未闭合项** —— 13 条全部已修且负例测试在旧实现下会红；未闭合项不在 13 条本身，而在：REPORT-14-R1 第 4 条描述的实现（复用 `rollback_node_changes_core`）已被更新的一版（按本次尝试快照 + `restore_before_dir`）取代，报告与验证结论落后于工作区现状。

## 逐条核对

1. **已修**。① 输入只剩 stderr + error：`classify_attempt_quota`（`src/dispatch.rs:3060-3065`）没有 stdout 参数（结构上杜绝），汇合点 `src/dispatch.rs:3415-3425` 只在 `entered_channel` 时用该函数；identity_mismatch 因 `status != "error"`（`:3061-3063`）永不改写，timeout 同理。② 数字状态码必须带前缀：`src/channels/mod.rs:71` 的正则四条前缀（`http `、`status:`/`"status":`、`code:`、`api_error`）均不可省。③ `resets` 收紧为 `\bresets?\s+(?:at\b|in\b|\d)`（`mod.rs:85`），中文额度词要求同句含用尽语义（`mod.rs:88-93`）。④ 负例测试 `mod.rs:995-1006`（`src/lib.rs:429:5`、`server resets the connection`、`403 forbidden`、`额度配置已加载`、`配额字段不能为空`、普通 error）——旧实现下裸 `429`、`resets` 子串、`额度` 子串分别命中，四条全红；派发层负例 `dispatch.rs:6641-6647`（identity_mismatch 不改写、解析失败文本不中）。
2. **已修**。`record_stdout` 按 `备` 前缀解析序号生成 `<node>.raw.备N.txt`（`src/dispatch.rs:2677-2681`）；留痕 `attempt` 归一为主/备两值，新字段 `attempt_index` 记实际序号（`dispatch.rs:3533-3534`，字段定义 `:364-366`，serde default + 0 值省略向后兼容）。测试：`task14_r1_production_backup_labels_keep_all_raw_dumps_separate`（`:7253-7268`，40KB×3 份互不覆盖，旧实现下三份同名全红）；`attempt_index: 1/2` 断言（`:6700-6701`）。
3. **已修**。`verify_on_first_use_at` 在写库之前先 `classify_quota`，命中直接返回 `QuotaExhausted`，不走 `apply_smoke_outcome`（`src/resources.rs:1882-1885`，枚举 `:1713`）；派发层状态置 `quota_exhausted`（`dispatch.rs:3244-3246`）、`record_quota_hit`（`:3415-3418`）、照常切备选。测试：`task14_r1_first_use_quota_keeps_model_unverified`（`resources.rs:3316-3340`，断言 unverified / 非 failed / smoke_failure 空，旧实现下 3336 行红）；`task14_r1_first_use_quota_switches_to_fallback_without_entering_primary_channel`（`dispatch.rs:6882-6896`，marker 只有 p1、QUOTA_MARKS=1、备注含额度用尽）。
4. **已修**（且在审查期间又被加固了一轮）。可写节点仅在额度用尽时进备选循环（`dispatch.rs:2809-2818`），切换前回滚（worktree → `git worktree remove --force` 只删本次 worktree；单写者 → 只认本次尝试 `artifacts.before_dir` 的 `restore_before_dir`，`rollback_quota_attempt` `:2879-2913`）；本次无改动视为无需回滚照常切（`:2900-2902`）；绝不取历史快照；回滚失败置 `fallback_skipped="回滚失败"` 并中断不切（`:2825-2830`、`:2862-2866`）；非额度失败标「可写节点」不切（`:2776-2777`、`:3494-3500`）；G1 提示 `writableFallbackHintOf` 同时看显式备选与资源页 fallback_order（`src-ui/src/graph/dispatch.ts:237-246`，经 `DispatchBits.tsx:13` 接线）。测试：回滚后备选 saw=original（`dispatch.rs:6780-6803`，不回滚即红）、无改动直接切（`:6807-6829`）、历史快照不还原用户手改（`:6832-6862`）、回滚失败不切（`:6900-6927`）、非额度不切（`:7017` 起）、提示四态（`presets-05b-r1.test.ts:78-90`）。**回滚路径范围**：`restore_before_dir`（`:2318-2369`）只写项目 canonical 前缀内的路径、目标为链接时先删链接、只删 `.created` 清单内文件、跳过链接/超大/不可读；`before_dir` 在快照持久化时写入 artifacts（`:3444-3448, 3458`），不存在空 run_id 搜历史快照的调用（grep 无命中）。单写者并发由调度器 `active_single_writer` 串行（`:1789/1807/1860`），回滚发生在该节点仍活跃窗口内，无并发写者。
5. **已修**。`ui_status` 额度分支（`dispatch.rs:1863-1870`）；STATE.md 单列「## 额度用尽」区（`:3923-3930`，测试 `:5888-5929`）；前端 `resultStatus`（`DispatchOverlay.tsx:93`）、移交文字（`:387`）、G1 分区（`:549-551`）、`fallbackSkippedLineOf` 额度措辞（`dispatch.ts:249-254`）、`applyDispatchResults` 写 `quota_exhausted` 状态（`:267`）、`nodeStatusLabel`（`:370`）、`ReplyStatus` 类型（`types.ts:4`）、swarm 终态含 quota（`swarm.ts:380`）、图卡/节点弹窗重派入口（`GraphTab.tsx:583-587`、`NodePanel.tsx:348-350`）。测试 `dispatch.test.ts:82-97`、`presets-05b-r1.test.ts:92-96`。
6. **已修**。结果带 `resources_changed` 时 Rust emit `aicross:resources-changed`（`dispatch.rs:1873-1877`）；前端桥 `subscribeResourcesChanged`（`mention.ts:37-51`）在 App 挂载（`App.tsx:134`），ResourcesTab/GraphTab/OrchestrationPanel 既有监听生效；接入页受限模型「清除」按钮（`ResourcesOverlay.tsx:521-531`，`quotaClearVisible :396`）走 `clear_quota_exhausted` 原子写（`resources.rs:167-173, 1480-1482`）。测试：`resources-overlay-09b.test.tsx:183-197`、`resources.rs:2765+/2802-2810`。
7. **已修**。`convergeQuotaSelection` 纯函数 + effect 随 `quotaBlockedIds` 变化收敛，用户手勾过的节点保持（`DispatchOverlay.tsx:95-104, 109-111, 160-162`，touched 记于 `:283`）；接线测试 `dispatch.test.ts:75-80`（含源码级断言）。
8. **已修**。`orchestration::Reply.quota_resets_at` 带 `#[serde(default)]`（`src/orchestration.rs:119-121`），往返+旧图缺字段测试（`:1463-1469`）；图侧写入（`dispatch.ts:273`，`types.ts:34`）、图结果卡从 Reply 显示 `QuotaResetLine`（`GraphTab.tsx:684`）。
9. **已修**。解析顺序 epoch → Retry-After 数值 → Retry-After HTTP-date → `resets HH:MM` → ISO 兜底（`src/channels/mod.rs:108-166`）；测试覆盖 `usage limit reached|1757970000`、`Retry-After: 120` 与 HTTP-date、`retry_after=60` 优先于无关 ISO（`mod.rs:1009-1032`）、`resets 1:30pm` 断言本地 13:30 与过去→次日（`:1035-1042`）、60s/120s 上下界窗口（`:1009-1019`）。
10. **已修**。跳过候选把 `quota_exhausted_until` 并入 `quota_resets` 并写「跳过 route（额度用尽至 …）」（`dispatch.rs:2833-2839`，最终取最早 `:2871-2873`）；测试 `:6717-6749`（主无 resets 时返回被跳候选时间、marker 只调一次）与三跳用例 `:6650-6702`。
11. **已修**。429 正例纯文本不传 http_status（`mod.rs:977`，走 embedded 正则）；`task14_all_quota_attempts_keep_status_and_earliest_reset` 已改名拆分为 `task14_reset_helpers_choose_earliest_active_time` + `task14_exhausted_fake_channels_return_quota_not_error`（后者经假通道断言 `quota_exhausted` 与最早时间，`dispatch.rs:6753-6777`）；各负例见第 1 条。
12. **已修**。MANUAL 术语表新增「恢复时间」行（`docs/MANUAL.md:165`）；资源页行内保留「额度用尽至 HH:MM」（`src-ui/src/resources/types.ts:102-108`）；界面统一「恢复时间」（`DispatchOverlay.tsx:551`、`DispatchBits.tsx:43`）；测试 `resources-overlay-09b.test.tsx:200-201`。
13. **已修**。`exit_code` 注明保留原因且不参与判定（`channels/mod.rs:64-66`）；备选序号在实际调用时递增、跳过项不断号（`dispatch.rs:2841-2844`，三跳用例断言连续 1/2）。

## 新发现的问题（按严重度）

1. **中低（流程）**：REPORT-14-R1 落后于工作区。第 4 条写「复用既有 rollback_before / rollback_node_changes_core 通路」，但当前实现已改为按本次尝试快照回滚（`rollback_quota_attempt` + 新抽取的 `restore_before_dir`，`dispatch.rs:2318-2369, 2879-2913`），并新增了报告未记载的两条 `task14_r2_*` 测试；报告「验证」节的 cargo/vitest 计数无法证明覆盖当前树。建议：补记 R2 变更并重跑 `cargo test` / vitest / build 后再交付。审查期间文件仍在被编辑（行号漂移），也需确认没有半途状态。
2. **低**：回滚失败不落留痕。`fallback_skipped="回滚失败"` 与备注是在 `run_one_attempt` 已写完留痕之后才改到 result 上（`dispatch.rs:2825-2830, 2862-2866`，留痕写于 `:3543-3553`），该次尝试的 `.md` frontmatter/runner_note 里查不到「回滚失败」，只有返回前端的结果带。复现：`QUOTA_ROLLBACK_FAILURE` 场景后读留痕文件。建议：留痕补记或在该次尝试后追加写。
3. **低（既定取舍内的残留，非本轮缺陷）**：`resets \d` 仍会命中「server resets 3 connections」类非额度文本（`channels/mod.rs:85`）；ISO 兜底在无 reset/retry 语义时仍取正文第一个 ISO 时间戳（`:157-165`），理论上可能采信无关时间戳。均为 fix-14-r1 第 1/9 条选定规则的边界，如要收紧需再立规则。
4. **提示**：`status 429`（空格无冒号）与 `HTTP/1.1 429` 不被识别（`channels/mod.rs:71` 要求 `[:=]` 或 `http` 后接空白），与清单第 1 条写的 `status[: =]` 相比偏保守，方向是漏判不是误判。
5. **提示**：`dispatch.rs:230、370` 字段注释未列「回滚失败」这一取值。

## 核过没问题的点

- 负例四件套在旧实现下确实全红（裸数字行号、`resets the connection`、无 marker 403、含「额度」非错误）；`classify_attempt_quota` 无 stdout 入参属结构性保证。
- 首次验证只送冒烟 error 文本进分类（`resources.rs:1883`），命中分支在写库前返回；ACP fallback 分类只喂错误 reason（`dispatch.rs:3314`）且命中时不回退 oneshot。
- 备选循环保持：显式优先、seen 去重、候选有限、每候选至多一次、全部用尽保持 quota_exhausted 且取最早 resets；跳过候选不占 `attempt_index`。
- 回滚路径不碰项目外文件、不碰链接、不碰排除目录；stale 快照还原被 R2 测试钉死；单写者节点调度期串行使回滚无并发写者。
- 事件链完整（Rust emit → tauri listen → window event → 三处页面刷新），清除动作走同一原子写通路且有正反测试。
- G1 时序收敛不影响用户手勾节点；旧图/旧记录缺新字段均 serde default，`attempt_index` 为 0 时不序列化。
- 术语在手册、资源页、确认屏、结果卡四处一致。