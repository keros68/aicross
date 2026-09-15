## 总判

**可交付，但有未闭合项**。契约追加、备选循环、资源页写回、G1/结果卡主体都对得上任务书与 PRODUCT.md 已定 ③；循环无死循环/重复计费，写节点不切备选故无回滚问题。未闭合的是：识别函数在 oneshot 汇合点把 stdout 一起喂进去、裸数字状态码即可命中，都构成误判路径；备选尝试的原始输出转存会覆盖主尝试的转存。以下按严重度排列。本次为纯静态审查（本会话无 shell 工具），未实际运行 cargo test / vitest，测试结论引自 REPORT-14 并核对了测试代码本身。

## 问题清单

**1.（高）oneshot 汇合点把 stdout 喂进 classify_quota，违反该函数自己的调用契约，可把带正文的结果误判成额度用尽**
- 位置：`src/dispatch.rs:3302-3304`，对照函数注释 `src/channels/mod.rs:57-58`（「调用方只把错误输出传进来，避免正常回复谈到 quota / 限额时被误判」）。
- 问题：`quota_text = [raw_stderr, raw_stdout, error].join("\n")`。凡 `status` 落在非 ok / 非 timeout 都会跑：包括 `identity_mismatch`（第三方端点真身不符，stdout 里是模型完整回复）和输出解析失败（正文仍在 stdout）。
- 复现：盲节点派到第三方端点，回复正文里出现「resets 1:30pm」「额度」「usage limit」或裸 `429`（比如任务本身就是讨论限额方案——正是本仓库自己的用法），同时真身核对失败或解析失败 → status 被改写为 `quota_exhausted`；若正文里还能解析出时间（ISO / `resets 1:30pm` / `retry after`），`record_quota_hit`（dispatch.rs:3307）把该模型写进 resources.json 的 `quota_exhausted_until`，G1 从此默认不派它。后果正是审查关心的两种：错切备选掩盖真因（identity_mismatch 被覆盖）＋错误标额度。
- 建议：此处只传 `raw_stderr + error`；或仅当 stdout 里没有可解析回复时才纳入 stdout。

**2.（中）内嵌状态码正则前缀全可选，正文里任何裸 402/403/429 都算数；429 单独即触发**
- 位置：`src/channels/mod.rs:65-73, 82`。
- 问题：`(?:http(?: status)?|status|code)?\s*[:=]?\s*\b(402|403|429)\b` 三个前缀都可省略，等价于「文本含裸数字」。`status != Some(429)` 判定下，裸 `429` 不需要任何限额措辞就命中。任务书写的是「HTTP 429」。
- 复现：普通错误文本含「main.rs:429」或 CLI 打印「exit code 429」→ `quota_exhausted`（reason=rate_limit），无任何限额文案。
- 建议：把前缀组改为必选（如 `(?i)(?:https?|http|status|code)\s*[:=]?\s*\b(402|403|429)\b`），或 429 须与限额措辞同现。补一条 `classify_quota("error at main.rs:429", Some(1), None).is_none()` 的阴性测试（现状下该断言会红，恰好钉住此问题）。

**3.（中）备选尝试的原始输出转存覆盖主尝试的转存文件**
- 位置：`src/dispatch.rs:2653`（`let suffix = if attempt == "备" {...}`），与注释 2647-2648（「备选加 .fallback 后缀，不再覆盖主尝试的转存」）矛盾；本期 attempt 标签改成了 `备 {index+1}`（dispatch.rs:2799）。
- 复现：只读/盲节点配 fallback_order，主尝试 stdout >32KB 转存到 `{node}.raw.txt`；任一备选尝试 stdout 也 >32KB → 同名文件被覆盖，主尝试原始输出丢失。旧单备选路径（attempt 恰为「备」，dispatch.rs:2853）不触发，所以既有测试 `record_stdout_keeps_primary_and_fallback_dumps_separate`（dispatch.rs:6916）照旧绿——测试盲区。
- 建议：判据改成 `attempt.starts_with("备")`，后缀可带序号（`.fallback-1`）把多次备选也分开；补一条走新循环路径的转存分离断言。

**4.（低）图上的节点卡把额度用尽显示为「失败」**
- 位置：`src-ui/src/graph/dispatch.ts:357-361`（nodeStatusLabel 只映射 identity_mismatch / validation_failed，error_kind=`quota_exhausted` 落到兜底「失败」；该 error_kind 在 dispatch.ts:266 写入）。
- 后果：同一结果在 G3 结果卡是「额度用尽」（DispatchOverlay.tsx:92-93），回到图上变「失败」，与 MANUAL.md 术语表「一个概念一个词」和「不算失败」的口径不完全一致（结果卡本身符合）。
- 建议：nodeStatusLabel 增补 `error_kind === 'quota_exhausted' → '额度用尽'`。

**5.（低）可写节点额度用尽时的「主失败」措辞**
- 位置：`src-ui/src/graph/dispatch.ts:244-246`（fallbackSkippedLineOf 固定文案「主失败，未切备选（…）」；quota_exhausted 也会带 fallback_skipped=「可写节点」，dispatch.rs:3377-3383）。
- 建议：按 status/error_kind 分支措辞，额度用尽时说「额度用尽，未切备选」。

**6.（低）G1 的「可写节点不自动切备选」提示只看显式 Target.fallback，不看 fallback_order**
- 位置：`src-ui/src/graph/dispatch.ts:237-241`（`!!target.fallback?.model.trim()` 才提示）；引擎侧跳过条件是 `explicit || !configured.is_empty()`（dispatch.rs:2740-2746）。
- 复现：用户在资源页给模型排了 fallback_order，派一个单写者节点（没设显式备选）→ 引擎不会切备选，G1 无任何提示，要到结果卡才看到「未切备选（可写节点）」。
- 建议：提示条件纳入该模型在资源页的 fallback_order（models 已在 G1 手上，DispatchableModel 带 fallback_order 字段，types.ts:131）。

**7.（低）备选因「额度未恢复」被跳过时不留任何说明**
- 位置：`src/dispatch.rs:2793-2795`（静默 `continue`）；对比可写节点跳过有 fallback_skipped + 留痕 frontmatter（R2-11）。
- 后果：runner_note 链条里看不出「排在后面的备选为何没试」。纯可观测性。
- 建议：跳过时往 quota_notes 追加一句（如「备 X 额度未恢复，跳过」）。

**8.（低）重置时间解析的时区边界**
- 位置：`src/channels/mod.rs:94`（ISO 正则强制要求 `Z` 或 `±HH:MM`，无时区的裸 ISO 直接解析失败 → resets_at=None → 不标资源库，行为保守可接受）；`111-129`（`resets 1:30pm` 一律按本机时区解释）。
- 事实：Claude 会话额度的本地时间表述没问题；但若某 CLI 打印的是 UTC 钟点（无时区字样），会按本地解读，标出的恢复时间偏差数小时；12 小时制含 12am/12pm 边界已正确处理（118-119）；`retry after N` 无单位默认秒（108）。
- 建议：属已知取舍可接受；若要收紧，识别文本里的 `UTC`/`Z` 钟点并换算。

**9.（提示）杂项**
- `src/channels/mod.rs:61`：`_exit_code` 参数按任务书签名保留但完全未用，DeepSeek 402 目前纯靠「insufficient balance」文案命中；后续要么用起来要么删参。
- `src/dispatch.rs:2799`：备选序号用的是候选列表原始下标，候选被跳过时留痕/说明里出现「备 2」而无「备 1」，观感跳号。
- 术语：「恢复时间」（QuotaResetLine，DispatchBits.tsx:42）、「额度用尽至 HH:MM」（quotaExhaustedLabel）、任务书里的「重置时间」是同一时间戳的三种说法；MANUAL 术语表未钉「恢复时间」这个词。建议统一为一个。

## 核过没问题的要点

1. **契约只追加**：`status` 新值 `quota_exhausted` 与 error/timeout/identity_mismatch 并列（dispatch.rs:3305）；`BackendNodeResult.quota_resets_at` 带 serde default（dispatch.rs:226-228，camelCase 经 `#[serde(rename_all)]` 到前端 `tauri.ts:540`，旧后端缺字段为 undefined）；`ResourceModel.fallback_order`/`quota_exhausted_until` 均 serde default（resources.rs:128-133），旧 JSON 缺字段可读有测试（resources.rs:2639-2646）；`Target.fallback` 三字段与语义未动（graph/types.ts:51-70），仅触发条件按任务书追加 quota（dispatch.rs:2703-2707）。
2. **识别覆盖面**：claude/codex/kimi/pi/cc-switch 一次性路径全部汇合于 run_one_attempt 的 3302 过 classify_quota；ACP 在 3200（Fallback 分支）与 3302（Failed 分支 status=error）都过；timeout 明确排除（3302）；build 失败（entered_channel=false）不识别。分类器测试覆盖 429 / 403+文案 / session limit+12 小时制 / 402 balance / 普通 error 阴性 / retry-after。
3. **备选循环**：显式 fallback 优先、其后按 fallback_order（2758-2773）；跳过与主目标相同项（seen 集合含主目标）、跳过 quota_exhausted_until 未到期项、跨字段去重，候选列表有限且每项至多跑一次——无死循环、无重复计费；非 quota 失败立即停止列表（2787-2791）；每跳 fallback_from/to 记最后一跳、note 累积全链（2796-2814，有真跑 Python 假通道的三跳测试 6503-6550）；全部用尽保持 quota_exhausted、resets_at 取最早（2816-2818, 2834-2838，有测试）；写节点（单写者/worktree）整体跳过备选（2742-2746），故「备选换通道不换产物」的既有语义未被破坏，无需新回滚；fallback 尝试仍置空 session 防下游续错会话（2897-2904）。
4. **resources.json**：update_model 全量 load→改→save，vendor/tier/note 由前端随当前值回传（ResourcesOverlay.tsx:169-177），quota_exhausted_until 不被 UI 编辑覆盖（resources.rs:1461-1478）；探测刷新保留 fallback_order 与 quota_exhausted_until（1367-1368，有测试）；过期清理在 get_resources_at 持锁清理并原子写回（2071-2078，有测试 2775-2779）；引擎 mark_quota_exhausted 只在时间可解析时写（343-345），与 UI 写命令同用进程级 store_lock + 原子写（517-567），无并发覆盖窗口。
5. **G1 与结果卡**：恢复时间未到的目标默认不勾（DispatchOverlay.tsx:98-99, quotaBlockedNodeIds dispatch.ts:886-899，固定 now 测试）、单列「额度用尽」分区可手动勾回（532-548）、路由卡 QuotaHint（587）；结果卡「额度用尽」状态 + 恢复时间 + 链式说明（92-93, 729, 733-735），移交文字对 quota 有独立分支不写成失败（371）。
6. **测试与文案**：新增测试均为行为断言（真跑假通道、marker 文件核对调用顺序、留痕条数、临时目录 resources.json 往返），未见同义反复；前端测试用固定 now，不受时钟影响。界面词汇「额度用尽」「备选顺序」与 MANUAL.md 术语表一致，无内部词外泄（除上面第 9 条的「恢复时间」措辞分叉）。