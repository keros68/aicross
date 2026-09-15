## 总判

**可交付但有未闭合项** —— 契约、备选循环、资源页写回与锁的主干都成立且测试能红，但额度识别过宽（真失败会被误判成额度用尽并写回资源库）、备选原始输出会覆盖主尝试转存、第三方首次验证这条路径不过 `classify_quota`，另有若干状态/字段/文案一致性问题。下列 11 条按严重度排序。

---

### 1. 高：`classify_quota` 过宽，普通错误会被误判成额度用尽并写回资源库

- **文件:行**：`src/channels/mod.rs:65-82`（判定）；`src/dispatch.rs:3302-3308`（调用与写回）
- **问题**：embedded status 正则 `(?:http(?: status)?|status|code)?\s*[:=]?\s*\b(402|403|429)\b` 的所有前缀与分隔符都是可选的，等价于「正文里出现独立数字 402/403/429」；`text_limit` 又含泛化词 `resets` 与 `额度/限额/配额`，且调用方把 `raw_stderr + raw_stdout + error` 一起送进来——`raw_stdout` 对 codex/kimi/pi 是整段事件流，含工具读到的仓库文件内容。命中即 status=quota_exhausted、`record_quota_hit` 写 `quota_exhausted_until`（G1 之后默认不勾该模型），真失败被掩盖且会错切备选。
- **复现**：纯函数直接调用 `classify_quota("error: unmatched delimiter at src/lib.rs:429:5", Some(1), None)` → `Some(QuotaHit{reason:"rate_limit"})`；`classify_quota("server resets the connection", Some(1), None)` → `Some{resets_at:None}`。真实场景：一个编译/测试失败的 codex 节点，raw stdout 里任何行号 429 或「限额」「resets」字样都会中。
- **建议**：数字状态只在明确上下文里认（`api_error_status`/`"status":`/`HTTP <code>` 等前缀不再全部可选）；`resets` 收紧为 `resets (at|in|\d)`；对 raw stdout 只取「错误相关行/最后若干行」或先经 `parse_output` 已有 error 文本，不要把工具输出整段当额度证据；补负例测试（`:429:`、`resets the connection`、403 无 marker、普通 error）。

### 2. 高：备选尝试的原始输出转存会覆盖主尝试（`attempt` 值对不上）

- **文件:行**：`src/dispatch.rs:2653`（`if attempt == "备"`）；`:2797-2799`（生产传 `&format!("备 {}", index + 1)`）
- **问题**：`record_stdout` 只认精确 `"备"`，TASK-14 的循环改传 `"备 1"/"备 2"`，suffix 落空 → 备选尝试 >32KB 的 raw 输出写进 `<node>.raw.txt`，覆盖主尝试的转存；多次备选互相覆盖，只剩最后一次。同时留痕 frontmatter 的 `attempt` 值从 TASK-05B 契约的「主/备」漂成「备 1/备 2」。既有测试 `record_stdout_keeps_primary_and_fallback_dumps_separate`（`dispatch.rs:6914`）只传字面量 `"备"`，红不了。
- **复现**：跑 `task14_quota_fallback_order_runs_fake_channel_through_three_attempts` 的桩，让每次 stdout >32KB；结束后 `runs/node-1.raw.txt` 是最后一次备选的输出，主尝试输出丢失。
- **建议**：判据改成 `attempt.starts_with("备")`（或传结构化枚举）；不改 `attempt` 的落盘取值，或在 REPORT/契约里显式记录取值变化；测试补一条生产标签 `"备 1"` 的断言。

### 3. 中：第三方「首次派发自动验证」失败不过 `classify_quota`，额度被记成验证失败

- **文件:行**：`src/dispatch.rs:3012-3030`（verify 失败直接短路，`entered_channel` 保持 false）；`:3302`（分类只看 `entered_channel`）；`src/resources.rs:1855-1895`（`verify_on_first_use_at` → `apply_smoke_outcome` 写 `verification=failed`）
- **问题**：未验证的第三方模型首派先发一次真请求核对真身，若这次请求撞 429/额度，结果是 `error`「首次验证失败：…」，不切备选、不写 `quota_exhausted_until`，还会把该模型在资源库标成「验证失败」（需人工重验）。与本期「限额即换家」正相反。
- **复现**：resources.json 里 pi 模型 `verification:"unverified"`，通道对首个请求返回 429；结果 `status=="error"`、`fallback_from/to` 空、库中 `verification=="failed"`。
- **建议**：在 `run_smoke_registered`/`verify_on_first_use_at` 失败分支过 `channels::classify_quota`；命中时（a）不写 verification=failed（保持 unverified），（b）把额度信息带回 `run_one_attempt`（新 `FirstUseVerification` 变体或 error 前缀约定），使 status 判 quota_exhausted 并照常走备选。

### 4. 中（需用户裁决）：可写节点（单写者/worktree）仍完全不切备选，任务书只列了两个跳过条件

- **文件:行**：`src/dispatch.rs:2736-2748`（`skip_reason = "可写节点"`）、`:2783`（`if skip_reason.is_empty()`）；`src-ui/src/graph/dispatch.ts:237-241`（G1 提示只看 `Target.fallback`）
- **问题**：TASK-14 写的是「跳过与主目标相同、跳过已标额度用尽且未到时间的」，未提可写节点；实现沿用 TASK-05B 的「可写节点不重跑」边界。后果：执行类节点（最耗额度的角色）撞额度时不会换家，只报 quota_exhausted；用户在资源页给执行模型排的 `fallback_order` 在 G1 没有任何提示（`writableFallbackHintOf` 判 `target.fallback`），失败后结果卡才出现「主失败，未切备选（可写节点）」。
- **复现**：只读节点配 `fallback_order` → 会切；同一目标改成 `write="单写者"` → 不切，`fallback_skipped="可写节点"`。
- **建议**：二选一并写进 REPORT/文案：①确认保持跳过（则在 G1 对「资源页有 fallback_order 的可写节点」也给提示，并把 `fallback_skipped` 文案与「不算失败」对齐）；②按任务书语义切，切前用既有 `before` 快照回滚（引擎已有 `rollback_node_changes_core`/`rollback_before` 通路）。

### 5. 中低：`quota_exhausted` 在 STATE.md／进度事件／图节点卡仍被写成「失败」

- **文件:行**：`src/dispatch.rs:1850-1856`（`ui_status` 无 quota 分支 → `_ => "失败"`，写入 `STATE.md`「## 失败与超时」）；`src-ui/src/graph/dispatch.ts:258-260`（`applyDispatchResults` → `status:'failed'`）、`:354-361`（`nodeStatusLabel` 无 quota 分支 → 「失败」）
- **问题**：结果卡/用量表显示「额度用尽」，同一次派发的 STATE.md 和图上节点卡显示「失败」，与「全部用尽…不算失败」「一个概念一个词」冲突。
- **复现**：任一 quota_exhausted 节点跑完后看 `.dispatch/STATE.md` 与图节点卡状态词。
- **建议**：`ui_status` 与 `nodeStatusLabel` 各加 `quota_exhausted => "额度用尽"` 分支；`write_state` 的「失败与超时」分区也决定是否单列。

### 6. 低：引擎写回 `quota_exhausted_until` 不通知前端，界面停留旧值

- **文件:行**：`src/resources.rs:354-361`（`mark_quota_exhausted` 静默写盘）；`src-ui/src/resources/mention.ts:32`（`RESOURCES_CHANGED_EVENT` 只由前端动作触发）
- **问题**：Rust 全仓没有对应事件；引擎命中额度后，已打开的接入页与 GraphTab 的 `models`（G1 的 `quotaBlockedIds` 数据源）不会刷新，模型行不显示「额度用尽至」、G1 也不会把它默认排除，直到用户做一次会触发刷新的事件或重挂载。另外误标后除等过期外没有手动清除入口。
- **复现**：开着接入页派发一个会撞额度的模型，回到接入页看该行状态。
- **建议**：`mark_quota_exhausted` 成功后从 Rust 发一个事件（或复用现有事件名）让前端 `notifyResourcesChanged`；接入页给受限模型加「已恢复／清除」动作。

### 7. 低：G1「默认不勾选」只在挂载那一刻算一次，且无接线测试

- **文件:行**：`src-ui/src/components/center/graph/DispatchOverlay.tsx:98-99`
- **问题**：`nodeIds` 初值用挂载时的 `quotaBlockedIds` 过滤一次；`models` 是异步拉的（GraphTab `useState([])` + effect），若开屏瞬间进确认屏、或确认屏开着时收到 `RESOURCES_CHANGED_EVENT`，被标额度用尽的节点仍保持勾选，分区标题却写「默认不派」。测试只钉了纯函数 `quotaBlockedNodeIds`（`graph/__tests__/dispatch.test.ts:61-72`），没有钉组件接线。
- **复现**：把 `listDispatchableModels` 延迟 1s，模型加载前打开确认屏 → 受限节点默认勾选。
- **建议**：`quotaBlockedIds` 变化时对「仍未用户手动改过」的节点做一次收敛（或按 `modelsRead` 延后初始化），并补一条读源码/渲染的接线断言。

### 8. 低：`Reply.quota_resets_at` 被前端写入、被 Rust `save_graph` 静默丢弃（当前无读取方）

- **文件:行**：`src-ui/src/graph/dispatch.ts:265`、`src-ui/src/graph/types.ts:34`；`src/orchestration.rs:82-119`（Rust `Reply` 无该字段）、`:817-820`（`save_graph(graph: Graph)` 反序列化后重新序列化）
- **问题**：派发结果写进 `Reply.quota_resets_at`，但 `save_graph` 经 Rust 结构体往返，未知字段落盘即丢；全仓也没有任何读取该字段的代码。REPORT-14「图结果契约追加 quota_resets_at，旧图与旧后端缺字段时不受影响」的说法与实际不符（写入即丢）。
- **复现**：派发后采纳/提交图 → 重新 `load_graph`，`reply.quota_resets_at` 不存在（`error_kind:"quota_exhausted"` 在，因为 Rust 有 `error_kind`）。
- **建议**：要么给 `orchestration::Reply` 加 `#[serde(default)] quota_resets_at: Option<String>` 并补往返测试，要么去掉前端这次写入，避免死字段。

### 9. 低：重置时间解析有两处缺口（贪婪 ISO / 真实 Claude 格式与 `Retry-After:` 头）

- **文件:行**：`src/channels/mod.rs:91-110`
- **问题**：①ISO 分支取整段文本里**第一个** ISO 时间戳，且优先于 `resets/retry after`；错误正文里若带无关时间戳（某些流式事件字段），会被当成恢复时间显示/写回。②只认 `retry after N`，不认 HTTP 头式 `Retry-After: N` 与 `Retry-After: <HTTP-date>`；③仓库内没有实测证据，但按外部经验 Claude Code 的会话额度原文是 `… usage limit reached|<epoch 秒>`，现有规则会命中「usage limit」（status 正确）却解析不出秒级时间戳 → `resets_at` 为空，于是资源页不显示「额度用尽至」、备选循环也不会跳过该模型。
- **复现**：`classify_quota("Claude AI usage limit reached|1757970000", …)` → `Some{resets_at:None}`；`classify_quota("usage limit; Retry-After: 120", …)` → `resets_at:None`。
- **建议**：优先解析 `resets/retry` 语义再退回 ISO；补 `\|\d{10}` 秒级时间戳与 `retry[-_ ]?after`（含 HTTP-date）；补对应单测。

### 10. 低：候选被「已知额度未恢复」跳过时静默且不计入最早恢复时间

- **文件:行**：`src/dispatch.rs:2793-2794`（`continue` 无 note）、`:2816-2818`（`earliest_reset` 只收本次尝试的结果）
- **问题**：全部候选都因 `quota_exhausted_until` 未到期被跳过、且主目标没解析出时间时，`quota_resets_at` 为 `None`，即使资源库里明明知道更早的恢复时间；用户也看不到「跳过了哪些、因为什么」。
- **复现**：主模型 quota_exhausted 无 resets；`fallback_order` 里唯一候选带未来 `quota_exhausted_until` → 结果 `quota_resets_at == None`。
- **建议**：把被跳过候选的 `quota_exhausted_until` 一并并入 `quota_resets`，并在 note 里写「跳过 <route>（额度用尽至 …）」。

### 11. 低：测试质量——429 分支与重置时间语义基本没被钉住，个别用例名不副实

- **文件:行**：`src/channels/mod.rs:938-956`（`quota_classifier_covers_status_text_balance_and_plain_errors`、`quota_classifier_parses_retry_after`）；`src/dispatch.rs:6553-6561`（`task14_all_quota_attempts_keep_status_and_earliest_reset`）
- **问题**：①429 用例显式传 `http_status=Some(429)`，`embedded_status` 正则分支（风险最大的那条）完全未测；②`retry after` 只断言「结果是可解析 RFC3339」，断言的是函数自己生成的值，接近同义反复，没断言 ≈now+60s；③`resets 1:30pm` 只断言可解析，没断言 13:30/跨日/时区语义；④无任何负例（403 无 marker、数字 429 行号、泛化 `resets`）；⑤`task14_all_quota_attempts_keep_status_and_earliest_reset` 名为「全部用尽保持 status」却只测 `earliest_reset`/`quota_until_active` 两个 helper（status 行为实际由另一条假通道用例覆盖）。
- **复现**：把 `embedded_status` 整块删掉、把 clock 分支的小时改错，以上用例仍全绿。
- **建议**：429 用纯文本（`"too many requests (429)"`，不传 http_status）断言；retry 断言时间窗口；clock 断言具体时分与「过去→次日」；各补负例。

---

## 核过没问题的要点

- **契约只追加且向后兼容**：`DispatchNodeResult.quota_resets_at`（`dispatch.rs:226-228`，前端 `tauri.ts:540`）、`ResourceModel.fallback_order`/`quota_exhausted_until`（`resources.rs:128-133`）均 `#[serde(default)]`；旧 `resources.json` 缺字段可读有测试（`resources.rs:2638-2646`）；`Target.fallback` 形状与 `fallbackSpecOf` 未动，Rust 侧往返测试仍在（`orchestration.rs:1057-1083`）。
- **备选循环主干正确**：显式 `Target.fallback` 先于 `fallback_order`（`dispatch.rs:2759-2772`）；`seen` 去重含主目标与重复项；非显式候选只在 `quota_exhausted` 时继续（`dispatch.rs:2785-2790`）；`quota_until_active` 跳过未恢复目标；候选列表有限、每个只跑一次、`retry.fallback=None` 不递归，无死循环/重复计费；`fallback_from/to` 记最后一跳、`runner_note` 累积整链、全部用尽保持 `quota_exhausted` 且 `quota_resets_at` 取最早——三跳假通道用例真起 Python 子进程（`dispatch.rs:6503-6548`），删逻辑即红。
- **写路径与锁**：所有 resources 读写（引擎写回、资源页更新、过期清理）都走同一 `store_lock()` + 原子写（`resources.rs:556-567`、`:2071-2079`、`:336-351`）；`update_model` 不动 `quota_exhausted_until`，`fallback_order: None` 保留原序（`:1474-1476`，有测试）；`merge_detected_models` 保留两个新字段（`:1366-1368`）；资源页 `updateModel` 回传 vendor/tier/note 全量值（`ResourcesOverlay.tsx:169-176`），接入页编辑不会丢别的字段；手动添加模型拒绝重复，不会覆盖既有 `fallback_order`。
- **识别覆盖面**：claude/codex/kimi/pi/cc-switch 的一次性调用在 `entered_channel` 失败后统一过 `classify_quota`（`dispatch.rs:3302-3308`）；ACP 额度命中时不回退 oneshot、直接进同一分类（`dispatch.rs:3199-3210`）；`timeout` 与 `build_invocation` 失败被有意排除。
- **G1 与结果屏**：`quotaBlockedNodeIds` 支持 provider 精确/退化匹配，默认排除 + 单列分区 + 可手动勾选（`DispatchOverlay.tsx:98-99, 532-547`）；改选受限模型时路由卡有 `QuotaHint`（`:587`）；结果卡「额度用尽」、`QuotaResetLine`、移交文字「额度用尽：…」（`:93, 371, 729-735`）；资源页行内与悬停提示都用 `quotaExhaustedLabel`（`ResourcesOverlay.tsx:470-472`、`types.ts:311-318`）。
- **文案规范**：新增可见词「备选顺序／额度用尽／额度用尽至 HH:MM」已进 `docs/MANUAL.md:164-165` 术语表，与 PRODUCT「已定 ③」用词一致；「移除／上移／下移」为动词短语，「默认不派」沿用既有写法；未见内部词、emoji 或半角标点；PRODUCT 的「同角色可用模型」因资源模型不带角色而落成「全部可派发模型」，与任务书第 3 条一致。
- **前端契约与旧图**：`BackendNodeResult.quotaResetsAt` 可选，`applyDispatchResults` 缺字段时不影响旧图（`graph/dispatch.ts:265`）；`quotaExhaustedUntil` 对无法解析/过期值返回 null（`resources/types.ts:90-98`，有测试）。

说明：本次为只读审查且环境无 shell，无法运行 `git diff`/`cargo test`/`vitest`，以上按工作区当前文件核实；REPORT-14 里「未改 `src/terminal.rs`/`src/server.rs`」「未 cargo fmt」「CRLF 计数」这类 diff 事实我无法复核。