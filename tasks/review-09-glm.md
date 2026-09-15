# TASK-09 审查

报告与代码逐项核对过，引用的行号与断言全部属实。按维度给发现。

## 1. 需求完整性

契约 1–6 逐条对照均落地，报告描述与实现一致（抽样核对 resources.rs / dispatch.rs / 前端各处行号全对上）。两条发现：

- **`src/resources.rs:45,238-254,1452` — 原生判定只看 agent，不看端点/provider。** claude 经 cc-switch 或 `ANTHROPIC_BASE_URL` 换到第三方端点后仍判 native，`verify_on_first_use` 永不对它动手——而冒烟口径要防的静默降级恰恰高发在这条路上（zai 把 glm-4.7 换成 glm-5.3-flash 那类）。任务书契约原文就是按 agent 判，实现没错，但口径本身有洞。建议：`run_smoke` 已有 `claude_third_party_endpoint()`（`src/resources.rs:1323`）可复用——`verify_on_first_use_at` 的跳过条件对 claude 改成「官方端点才跳过」，同时 `apply_smoke_outcome`（`:1366,1381`）对 claude+第三方端点要写 verified/failed 而不是恒 native（否则失败也记不进）。改动跨两个函数，建议开 TASK-10 细化，不阻塞本期。
- **`docs/MANUAL.md:31-32` — 写了不存在的功能。**「条目超过一个月没用过，派发前重验一次」：全仓 grep 无任何实现（`src/`、`src-ui/` 均无 30 天/一个月逻辑），这是向用户承诺了幻觉行为。删掉这句，或改成「长期没用过的条目可在接入页手动重验」。

## 2. 逻辑正确性

- **`src/dispatch.rs:1903-1914` vs `:1806-1850` — 验证发生在 worktree 创建与单写者写锁之后。** 一次验证最长 60s：单写者节点期间项目写锁被占，同项目其它单写者派发陪等甚至锁超时；worktree 节点验证失败时白建一个 worktree。建议把验证块移到取消检查（`:1897`）之后、workdir/锁创建之前——验证失败时这些本不该发生。严重度低-中（共享/只读节点不受影响）。
- **同一模型两路同时首次派发**：两边都读到 unverified → 各真发一次短请求 → 先后写回 last-write-wins。判据同一套，结果一致，代价只是多发一次请求；不持锁冒烟本身是对的。可在写回前复查 verification 仍为 unverified 收口，也可不动。
- **读后写回前模型被删**（`src/resources.rs:1462-1468`）：`apply_smoke_outcome` Err → 不落盘但返回 Verified/Failed。下一轮会再验一次，语义可接受。
- 失败节点口径 ✓：status 保持 `error`、error 前缀「首次验证失败：」、runner_note「首次派发自动验证」，测试用 marker 文件证明通道未进。
- `detected()` 排序变化 ✓：`src/orchestration.rs:1241-1275` 已改并钉住新序（补了 pi 三档），`model-picker.ts:52` 明确不二次排序，无残留 `status_rank` 断言。
- 写回并发 ✓：二次加锁后重读全库再改再存（`:1461-1467`），load→apply→save 全程持 `store_lock`，与其它命令互斥成立。

## 3. 安全

未发现。核对过三点：验证请求只发固定「只回复 OK」（`resources.rs:33`），task_file=None、cwd=临时目录、blind=true，不带任何项目材料；失败原因两处均过 `safe_summary`（`:1389,:1456`），进 `.dispatch` frontmatter 的也是脱敏串；写回按 (agent, provider, id) 精确命中单条。

## 4. 代码质量与契约

未发现新问题。旧字段四个全保留（`resources.rs:101-108`）；前端删掉的 `connectedByAgent`/`resourceSummary` 是本次改动产生的死代码，符合清理纪律；`server.rs` 未动且前端确实只调用既有命令（`tauri.ts` 未改，无新 invoke）；`dispatch.rs` 生产改动仅接线块+替身，`orchestration.rs` 仅测试，第 2–8 期的 run_one 主体、对账、回滚、心跳未被触碰。备选目标经 `run_one_attempt` 自然获得首次验证，合理且零额外代码。

## 5. 视觉纪律与可访问性

- **`src-ui/src/components/center/graph/DispatchOverlay.tsx:449` — unverified 提示误用红色。** `dispatch-warning` 是 `var(--red)`（`GraphTab.css:49`），而「首次派发会先验证真身」是常规提示——新装 pi 用户每张 G1 都见红，也违反同文件 `:393` 自己写的纪律（常规走 `dispatch-reason`，红字只留异常）。改成 `dispatch-reason`；`:450` failed 保留 warning。
- **`ResourcesTab.tsx:128,140` — title 悬停键盘不可达。** title 在 WebView2 里只响应 hover，行虽可聚焦有焦点环，键盘用户读不到来源路径。最小改法：模型行加 `aria-label`（模型名+来源）或补一个 sr-only 来源文字。
- **`resources.css:40` — `font-size: 9px` 不在六级阶梯（11/12/13/14/16/18）内。** 提到 11px 或徽标加大到 18px。
- **`resources.css:157,212` — padding `5px 6px` / `4px 6px`、gap 6px，不在 4 的倍数上。** 接入页旧样式沿袭了 6px，新写的行可顺手归位，不阻塞。
- 胶囊只给 failed ✓；来源路径不进正文且有测试钉住 ✓；文案中性简短 ✓；单色主题下徽标成唯一彩色——REPORT 已自报需真机确认，同意交付前扫一眼即可。

## 6. 测试

报告所列测试全部真实存在且断言到位（Rust 5 条 + 前端三份），失败路径验到「stub 未执行」「落盘状态」「第二次 Skipped」，质量好。最该补的 3 个：

1. **G1 提示无测试**：`DispatchOverlay.tsx:428-432,449-450` 两条提示逻辑内联且零覆盖。抽成 `resources/types.ts` 纯函数 `verificationHint()` + 一条渲染断言。
2. **run_one 层「通过后第二次不再验证」**：现只测了单次派发带 runner_note。把 `FIRST_USE_STUB` 换成计数替身，断言同一目标两次 `run_one` 只记一次「首次派发已验证真身」。
3. **claude+第三方端点走自动验证**（若采纳维度 1 建议）：stub 断言挂 `ANTHROPIC_BASE_URL` 时验证发生。

---

## 是否可交付

**可交付**——契约 1–6 全部落地、验证实跑通过、安全与回归无问题；合入前顺手改掉前两条缺项更好。缺项按严重度：

1. `MANUAL.md:31` 幻影功能句（写了不存在的「一个月重验」）——文档失实，必改。
2. `DispatchOverlay.tsx:449` unverified 提示红色误用——一行改动。
3. claude 第三方端点不算第三方（`resources.rs:45`）——口径缺口，开后续任务。
4. 验证在写锁/worktree 之后执行（`dispatch.rs:1903`）——60s 占锁，建议移前。
5. title 悬停键盘不可达（`ResourcesTab.tsx:140`）。
6. 9px 字号 / 6px 间距两处小违反（`resources.css:40,157`）。
7. 并发首次派发重复冒烟一次——记录即可，可不改。