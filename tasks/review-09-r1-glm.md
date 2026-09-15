# TASK-09 修正轮 R1 复审

## 逐组结论

**第 1 组 原生判定 = 原生 agent 且非第三方端点（DS 1、GLM 3）— 已核实修好**
`src/resources.rs:259` `is_native_model` = `is_native_agent && !third_party_endpoint`，`third_party_endpoint`（`src/resources.rs:252`）复用 `claude_third_party_endpoint()`（`:1295`，进程环境 + settings.json 两处都读，测试构建用 `set_claude_third_party` 隔离开发机漂移）；`ensure_verification`（`:285`）把 `native ⟺ is_native_model` 做成不变式，换端点后旧库 native 确实降档；`apply_smoke_outcome`（`:1429`）同判据，第三方 claude 冒烟成败真落 verified/failed；测试 `r1_claude_on_third_party_endpoint_is_not_native`（`:2807`）两端点往返各断言一次，`r1_native_agent_match_is_case_insensitive`（`:2865`）钉住大小写。接入页按钮条件 `verificationOf !== 'native'`（`ResourcesOverlay.tsx:322/408`）自动覆盖，PRODUCT.md:232 口径已写入。

**第 2 组 provider 为空不静默跳过（DS 2）— 已核实修好**
`resolve_first_use_target`（`src/resources.rs:1538`）：provider 空时退 agent+id，唯一命中连 provider 一起做写回键；多命中且含未验证 → `Failed` 带原因（`:1619-1624`），不发请求（测试 `:2927` 用 `panic!` 钉住）；库里无条目 → 新增 `NotInStore`（`:1471`）放行但 `runner_note` 留痕（`src/dispatch.rs:1827`）。G1 文案与行为一致（`types.ts:116`「会先发一个短请求核对真身」）。

**第 3 组 可取消、不占写锁（DS 3、4、GLM 4）— 已核实修好**
验证整段在 `dispatch.rs:1812-1845`，五个下游全部加 `verified` 前置：worktree `:1852`、写锁 `:1868`、回滚 `:1876`、快照 `:1884`、备料 `:1903`；error 组装验证失败直接用 `verify_error`（`:1947`）。冒烟走 `run_smoke_registered`（`resources.rs:1351`）带 `cancel_key`，冒烟返回后查取消（`:1646`）→ `Cancelled` 不写库；dispatch 再查一次（`:1842`）。测试 `r1_first_use_verification_runs_before_write_lock_and_worktree`（`dispatch.rs:3304`）自己握写锁、断言不等到超时且 `runs/<node>/wt` 未建，是这条的硬证据。

**第 4 组 并发首派与写回（DS 5、6、GLM 7）— 已核实修好**
`first_use_gate`（`resources.rs:1591`）模型级互斥 + 拿闸门后重读（`:1630`），并发测试用 Barrier 双线程断言冒烟计数=1、一 Verified 一 Skipped（`:2981`）。写回失败并进返回值让派发失败（`:1650-1666`），测试删文件模拟（`:3029`）。替身补记 `(agent, provider, model)`（`dispatch.rs:1765`）并断言等于节点目标（`:3351`）。

**第 5 组 UI 与可访问性（DS 7–11、GLM 2、5、6）— 已核实修好**
unverified 改 `dispatch-reason`（`types.ts:116` → `DispatchBits.tsx:25` → `DispatchOverlay.tsx:450`），failed 才红；详情按钮规则 `resources.css:436-452` 且 `.agent-card-model-detail summary` 死规则已删；同名多 provider 行 `modelRowLabel` 加后缀（`types.ts:169`），点击目标经 `mention.ts:14` `#provider` 后缀 + `mutate.ts:109-114` 按最后一个 `#` 切回、`@` 弹层过滤也剔 `#` 段（`GraphTab.tsx:699/712`）；徽标字 `--text-1`、色相只留 18% 薄底/40% 描边（`resources.css:9-21`），浅色 `--verify-green: #1f7a3d`（`:41-43`，约 5.1:1）；焦点环 2px 不透明（`:163-167`）；悬停文本挂 `aria-describedby` + sr-only 节点（`ResourcesTab.tsx:180-194`）；徽标 20/24px、11/12px 字号，左栏间距 4 的倍数；厂商色统一 `vendorColor`（`AgentBadge.tsx:18`，`vendor.ts:30-31` 补 kimi/pi）。断言齐全且不是摆设（样式断言真读 css、`visibleText` 先剔 sr-only）。

**第 6 组 文档与清理（DS 12、GLM 1）— 已核实修好**
MANUAL.md:22-36 两档口径、无「一个月重验」（全仓 grep 无）；TS 死导出 `STATUS_PENDING` 已删（`types.ts` 只剩 CONNECTED/FAILED；Rust 侧同名 const 是活代码，不该删）；注释对齐（`resources.rs:1-3/43-47`、`NodePanel.tsx:58`）；测试改名 `resources.rs:2306`。`orchestration.rs` 不采纳理由成立——任务书硬约束明写 Rust 只动两个文件。

**不采纳项复核**：DS 2.7（排序收敛，展示层无行为回归）、DS 2.9（UI 已无入口，矛盾仅剩直接调命令这一条路）、DS 2.6（契约变更）、GLM 6.2（语义已由 `task09_first_use_success_is_verified_once_only` 在 resources 层钉住）四条理由均成立；DS 3.1 诚实标注「未核实」而非假装核过。

**新缺陷排查**：run_one 主路径的 error/status/runner_note 组装、回滚与快照的 `verified` 前置（验证过=true 时行为与改前一致）、心跳与对账未受波及；蜂群不在本分支。`first_use_gate` 只在锁外发短请求、无嵌套锁，无死锁面。未发现修正引入的新缺陷。

## 结论

**R1 之后 TASK-09 可交付**（本分支范围内六组全部闭合、证据与报告一致、编排者复跑 295/315/零 warning 已确认），以下是遗留项，全部低危或信息级，不阻塞：

1. （低）旧库 `status=已接入` 的 claude 条目换到第三方端点后，`inferred_verification`（`src/resources.rs:269-274`）按已接入推断为 **verified** 而非 unverified——那是旧端点下冒的烟，新端点没验过；窄边（仅 09-15 前的存量），接入页手动重验可兜底。
2. （低）资源库读不出/损坏时首验按 `NotInStore` 放行（`resources.rs:1613-1614`），严格说与「验不了的返回失败」有出入；留痕可分辨，且全应用对损坏库本就另有报错面。
3. （低）手动冒烟（`smoke_one_in_place`，`resources.rs:1463`）不走 `first_use_gate`，与首验并发可对同一条目各冒一次、写回后到者胜；影响仅限同一条目的状态覆盖。
4. （低）`first_use_gate` 的表只增不减（`resources.rs:1591`），进程生命周期每模型一把锁，量级可忽略。
5. （信息）DS 3.1（blind 对 claude `--restricted` / pi `--no-context-files`）因本轮禁读 `src/channels/` 仍未核实，下轮解除后补一眼。
6. （信息）未起窗口实看渲染（约束要求）；徽标薄底在 obsidian/cappuccino 单色主题下的观感待真机确认。
7. （信息）`orchestration.rs:562` 注释漂移与一处旧测试名，按约束留到下一轮。