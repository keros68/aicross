## 总判

**可交付但有未闭合项** —— 13 条里 11 条已修、1 条部分（第 3 条，被回滚路径截断）、1 条部分（第 4 条：回滚改为隐式后有两个未覆盖分支，其中「回滚到范围外文件」有数据风险）。本轮为只读静态审查（无 shell），未能复跑 `cargo test` / `vitest`；「测试在旧实现下会红」的结论按 review-14-ds/glm 引述的旧实现推断并逐条核对了新代码与断言。

## 逐条

**1. 已修。** `classify_quota` 仍收 `(stderr_or_body, exit_code, http_status)`，唯一生产调用点 `src/dispatch.rs:3043-3047` 只拼 `raw_stderr + error`，不再有 stdout（`raw_stdout` 无任何调用点，grep 全仓仅 3 处调用：`dispatch.rs:3047`、`dispatch.rs:3297`（ACP 错误文本）、`resources.rs:1883`（冒烟错误文本））。数字状态前缀不再可选（`src/channels/mod.rs:68-75`），`resets` 收紧为 `at|in|\d`（`:82-86`），中文额度词要求同句用尽语义（`:88-92`）。`classify_attempt_quota` 先判 `status != "error"`（`dispatch.rs:3044-3046`），identity_mismatch 不改写。负例测试 `src/channels/mod.rs:995-1004` 含 `error at src/lib.rs:429:5`、`server resets the connection`、`403 forbidden`、`额度配置已加载`、`配额字段不能为空`；按旧正则（裸 `429`、裸 `resets`、裸额度词），前三条之外的 429 行号 / resets the connection / 两条中文在旧实现下会命中 → 是真红测试（`403 forbidden` 旧实现本就返回 None，属回归护栏）。接线侧另有 `dispatch.rs:6621-6627`。

**2. 已修。** `record_stdout` 改 `strip_prefix("备")` 并带序号（`src/dispatch.rs:2664-2672`）；留痕 `attempt` 归一为「主/备」、序号进新字段 `attempt_index`（`dispatch.rs:3515-3516`，`#[serde(default, skip_serializing_if)]` 于 `:361-363`）。测试 `dispatch.rs:7175-7191`（直接断言 `node-1.备1/备2.raw.txt` 三份并存，旧实现下会红）与 `:6680-6681`（三次真实尝试的 `attempt: 备\nattempt_index: 1/2`）。注意：实际文件名是 `<node>.备N.raw.txt`，与本条写的 `<node>.raw.备N.txt`、报告写的同名约定不一致（详见新发现 3）。

**3. 部分。** 验证语义已修：`verify_on_first_use_at` 在 `apply_smoke_outcome` 之前判额度并返回 `QuotaExhausted`（`src/resources.rs:1882-1886`），未写 `verification=failed`；派发侧映射为 `status=quota_exhausted` 并回写资源（`dispatch.rs:3118-3123`、`:3227-3229`、`:3400-3402`）。测试 `resources.rs:3316-3341`（库中仍 unverified、非 failed、smoke_failure 空）与 `dispatch.rs:6787-6820`（不进主通道、切备选、QUOTA_MARKS=1）。**未闭合**：该测试用的是只读节点；可写节点首派撞额度时，回滚分支会以「没有找到派发前快照」判失败而**不切备选**（见新发现 1），与「照常走备选」不符。

**4. 部分。** 主路径成立：可写节点额度失败先 `rollback_quota_attempt` 再切（`dispatch.rs:2809-2821`），回滚失败写 `fallback_skipped="回滚失败"` 并 `break`（`:2815-2820`），非额度失败 `eligible=false` 不切（`:2797-2805`，测试 `:6939-6972`），worktree 走 `git worktree remove --force`、单写者走 `rollback_node_changes_core`（`:2869-2895`）；G1 提示含资源页 `fallback_order`（`src-ui/src/graph/dispatch.ts:236-244`，测试 `presets-05b-r1.test.ts:73-90`）；测试 `dispatch.rs:6760-6785`（有改动 → 回滚后备选 merged）、`:6822-6848`（注入回滚失败 → 不切）。回滚写文件的范围本身是安全的（`safe_relative_path` + 父目录 canonical 必须在项目内 + 快照不含 .git/node_modules 等，`dispatch.rs:2318-2360`）。**未闭合**两点见新发现 1。

**5. 已修。** 后端 `ui_status` 增「额度用尽」（`dispatch.rs:1861-1866`）、STATE.md 单列 `## 额度用尽`（`:3913-3922`，测试 `:5909`）；前端 `nodeStatusLabel`（`dispatch.ts:364-372`）、`applyDispatchResults`（`:267`）、`resultStatus`/结果卡（`DispatchOverlay.tsx:93,387`）、蜂群终态（`graph/swarm.ts:380`）、重派入口（`GraphTab.tsx:583`、`NodePanel.tsx:348`）、`fallbackSkippedLineOf`（`dispatch.ts:249-254`）均有分支；测试 `graph/__tests__/dispatch.test.ts:82-97`、`presets-05b-r1.test.ts:92-96`。未见落「失败」的残留路径。

**6. 已修。** `mark_quota_exhausted` 成功后按 `result.resources_changed` 发 `aicross:resources-changed`（`dispatch.rs:1870-1874`）；前端单一桥接 `subscribeResourcesChanged`（`resources/mention.ts:36-46`，`App.tsx:134`），GraphTab/ResourcesTab/OrchestrationPanel 监听刷新；接入页受限行加「清除」（`ResourcesOverlay.tsx:522-532`），Rust `clear_quota_exhausted` 只清该字段（`resources.rs:173,1480-1482`，`update_model` 其余字段语义未动）。测试 `resources-overlay-09b.test.tsx:183-198`。小事实：`ResourcesOverlay` 自身不订阅该事件（只在自己操作后 setStore），但它是全屏弹层、重开即挂载重拉，未构成实际不刷新。

**7. 已修。** `quotaBlockedIds` 变化时用 effect 收敛、手动改过（`toggleQuotaNode` 记 `quotaSelectionTouched`）的保留（`DispatchOverlay.tsx:159-162, 282-290`），`convergeQuotaSelection` 语义正确；测试含源码接线断言（`dispatch.test.ts:76-81`）。小边界见新发现 5。

**8. 已修。** `orchestration::Reply` 追加 `#[serde(default)] quota_resets_at`（`orchestration.rs:119-121`），Graph 持久化路径 `GraphNode.reply: Option<Reply>`（`:50`）经 `save_graph` 往返不再丢；往返 + 旧图默认测试 `:1462-1470`；图上节点卡 `<QuotaResetLine resetsAt={node.reply.quota_resets_at}>`（`GraphTab.tsx:684`）与结果卡 `result.quotaResetsAt`（`DispatchOverlay.tsx:751`）都显示。

**9. 已修。** `parse_quota_reset` 顺序为 epoch → retry 秒/分 → Retry-After HTTP-date → `resets HH:MM` → ISO（`channels/mod.rs:108-171`）；测试 `:1023-1049` 覆盖 `usage limit reached|<epoch>`（断言秒值）、`Retry-After: 120` 与 HTTP-date、`retry_after=60` 优先于 ISO、`resets 1:30pm` 断言 13:30 与「过去→次日」；60/120 秒用上下界窗口断言（`:1009-1021`）。未见旧行为回归。

**10. 已修。** 被跳过的候选把 `quota_exhausted_until` 并入 `quota_resets` 且写 note「跳过 <route>（额度用尽至 …）」（`dispatch.rs:2823-2829`），终态取最早（`:2861-2863, 2902-2915`）。测试 `dispatch.rs:6697-6731`（主无时间、唯一候选被跳过 → 返回库中时间 + note + 未调用）与 `:6666-6685`。

**11. 已修。** 429 正例用 `"HTTP 429 too many requests"` 不传 `http_status`（`channels/mod.rs:977-979`）；retry 窗口断言（`:1011-1021`）；旧 `task14_all_quota_attempts_...` 已改名/拆分，status 与最早时间由 `dispatch.rs:6733-6758` 断言；负例见第 1 条。残余盲区见新发现 4。

**12. 已修。** 界面与手册统一「恢复时间」（`DispatchBits.tsx:43`、`DispatchOverlay.tsx:551`、`MANUAL.md:165` 术语表新增行），资源页行内保留「额度用尽至 HH:MM」（`resources/types.ts:101-108`）；测试 `resources-overlay-09b.test.tsx:177-181,200-203`。事实补充：`docs/PRODUCT.md:256,266` 仍写「重置时间」，测试只钉 MANUAL。

**13. 已修。** `exit_code` 在注释里写明保留原因、明确不据此分类（`channels/mod.rs:62-66`，实现 `let _ = exit_code;`）；备选序号按实际尝试递增、被跳过的候选不占号（`dispatch.rs:2831-2834`），测试 `:6680-6681`、`:6697-6731`。

## 新发现（按严重度）

**1.（高）可写节点额度失败的回滚判定用了「历史最新快照」，两个分支都错。** 位置 `src/dispatch.rs:2809-2821`（循环内回滚 + 失败 break）、`:2869-2895`（`rollback_quota_attempt`）、`:2198-2200`（本次尝试无改动时 `persist_before_snapshot_with_note` 提前返回、`before_dir` 为空）、`:2275-2303`（`latest_before_dir` 按 node 取 ended_at 最新记录，不区分 run/本次尝试）、`:2305-2311`。
- 症状 A（回滚失败误判 → 不切备选）：单写者节点本次尝试**没有产生任何文件改动**（最典型的就是首个请求即 429；首次验证撞额度同理）→ 没有落新快照 → `latest_before_dir` 找不到（首次派发）或找到**历史**记录 → `rollback_quota_attempt` 返回 `Err("没有找到派发前快照…")` → `fallback_skipped="回滚失败"`、`break`，备选一次都不跑。复现：临时项目 + 单写者 + 显式/资源页备选，桩只往 stderr 打 `HTTP 429 ...; resets at …` 后 `exit 1`、不写文件 → 期望切备选且 `status=ok`，实际 `status=quota_exhausted`、`fallback_to` 空。现有测试 `:6760-6785` 的桩每次都写 `state.txt`，正好绕开了这条；`:6787-6820` 用的又是只读节点。
- 症状 B（回滚到范围外文件）：同一节点此前作为单写者跑过且留痕有 `before_dir`（例如先成功改过文件），之后从节点面板「重派」（`NodePanel.tsx:348`，该入口不写 `retryNodes`，因此不下发 `rollbackBefore`）或任何不带 `rollbackBefore` 的重派，本次撞额度且无改动 → `latest_before_dir` 命中**上一次派发**的快照 → 把上一次产物和其后的用户手改一起还原，属于回滚范围外内容（写文件本身有 `safe_relative_path`/canonical 项目内护栏，越界的是「回滚到哪个版本」）。
- 建议：让额度回滚只认本次尝试的快照（把 run_id/本次 record 传进查找，或校验 `result.artifacts.before_dir`/本 run 记录），「没有本次快照/本次无改动」按“无需回滚”走 Ok 并照常切备选；补两条测试：单写者 429 不写文件 → 切备选；存在旧快照 + 本次无改动 → 旧文件内容不被改动。

**2.（低）非额度失败的可写节点不再标 `fallback_skipped="可写节点"`。** `dispatch.rs:2797-2805` 的 `eligible` 对可写节点只认 `quota_exhausted`，非额度失败直接 `break`，而主/备尝试的 `fallback_skip_reason` 现在恒为 `""`（`:2759-2771`、`:2974`），`fallback_skipped` 全仓只可能取「回滚失败」或空（`:3474-3480`）。结果是 05B 的「主失败，未切备选（可写节点）」在结果卡消失（前端分支 `dispatch.ts:249-254` 成引擎侧死代码），新测试 `:6939-6972` 还把它钉成空。若是有意收紧，建议同步改 05B 文档/前端文案；若否，建议对 `writable && has_fallback && 非额度失败` 恢复该标记。

**3.（低）报告与实现的 raw 转存文件名不符。** 实际 `<node>.备N.raw.txt`（`dispatch.rs:2672`，测试 `:7185-7189` 也按此钉），`tasks/fix-14-r1.md` 写 `<node>.raw.备1.txt`、`tasks/REPORT-14-R1.md` 写 `<node>.raw.备N.txt`。功能无影响（无读取方，后缀仍是 `.raw.txt`），属文档不实。

**4.（低）「stdout 不进分类」缺端到端负例。** `dispatch.rs:6621-6627` 只断言 wrapper 在 `identity_mismatch`/非额度文本下返回 None，证明不了调用点没把 stdout 拼进来（该行为只靠代码阅读）。建议补一条假通道用例：stdout 输出「本月额度已用尽 / resets at …」、stderr 无额度标记、失败退出 → 结果必须不是 `quota_exhausted`。

**5.（低）G1 收敛的 touched 集不含「待核对」分区。** `quotaSelectionTouched` 只在 `toggleQuotaNode` 记（`DispatchOverlay.tsx:282-283`）；`toggleReviewNode`（`:270-281`）不记。一个节点同时属于「待核对」和「额度用尽」分区时，用户在待核对行手勾后若 `models` 异步刷新，effect 会把它收敛掉（`:159-162`）。属边角，但与本条「用户未手动改过的节点」的字面语义有出入。

**6.（低，疑似既有）provider 为空的目标写回/读取额度会静默落空。** `record_quota_hit` → `mark_quota_exhausted` → `find_model_mut(agent, "", model)` 精确匹配 provider（`resources.rs:339-360`），旧图/未选 provider 的 Target 在库里 provider 非空时 `Err` 被吞成 `resources_changed=false`；首次验证的 `resolve_first_use_target` 却能按 agent+id 命中（`resources.rs:1795-1830`），两条路径口径不一致。

## 核过没问题的点

- 契约只追加、向后兼容：`attempt_index`（`dispatch.rs:361-363`，0 不序列化）、`Reply.quota_resets_at`（`orchestration.rs:119-121`）、`ResourceModel.fallback_order/quota_exhausted_until`（`resources.rs:128-135`）、前端可选字段（`tauri.ts:539`、`graph/types.ts:33`）；旧图/旧 JSON 有默认值与测试。
- 备选循环主干：显式 `Target.fallback` 先于 `fallback_order`、`seen` 含主目标去重、非显式候选只在 `quota_exhausted` 继续、`retry.fallback=None` 不递归、`session_id` 置空、`fallback_from/to` 记最后一跳、`runner_note` 累积、无死循环/重复计费（`dispatch.rs:2749-2863`，三跳假通道测试 `:6630-6685`）。
- 资源写路径：`mark_quota_exhausted`/`clear_expired_quota`/`update_model` 与资源页同走 `store_lock` + 原子写；`update_model` 不动 `quota_exhausted_until`（除显式 `clear_quota_exhausted`）；过期清理只清过期值。ACP 额度分支、`timeout` 与 `build_invocation` 失败不分类，符合既定取舍。
- 回滚的文件范围护栏：快照只含本次 changed 路径、跳过链接/排除目录，恢复时 `safe_relative_path` + 父目录 canonical 必须在项目内（`dispatch.rs:2318-2360`）；问题只在「选用了哪份快照」（新发现 1）。
- 前端：`nodeStatusLabel`/`applyDispatchResults`/`resultStatus`/蜂群终态/重派入口对 `quota_exhausted` 全覆盖；G1 默认不勾 + 分区 + 可勾回 + `QuotaHint` + `WritableFallbackHint` 的 `fallback_order` 判定与测试；`fallbackSkippedLineOf` 按 status 分措辞。
- 术语：界面与 `MANUAL.md` 统一「恢复时间」，资源页状态短语仍「额度用尽至 HH:MM」。

说明：本轮不能执行 `cargo check/test`、`vitest`、`npm run build`、`git diff`，报告中的构建/测试结论未由我复核；CRLF/仅追加等 diff 事实同样未复核。