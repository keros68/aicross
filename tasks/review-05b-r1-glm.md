# TASK-05B 修正轮 R1 复审

先说明验证方式：本会话没有可用的 shell 工具，以下为逐文件静态核对（19 条的代码落点与测试断言全部实读）；`cargo test` / `vitest` / `build` 由编排者复跑。测试计数我按文件实点核对过：`presets-05b-r1.test.ts` 恰 25 条 `it`，`presets-05b.test.ts` 恰 11 条，与报告口径一致。

## 逐条裁定

| # | 裁定 | 依据（文件：行） |
|---|---|---|
| 1 | **已核实修好** | 门在 `src/dispatch.rs:1207`：`visibility != "盲" && write != "只读"` 直接返回主失败并填 `fallback_skipped`；盲节点备选重跑前清隔离目录（`dispatch.rs:1234-1239`，目录在 `run_root` 下不碰项目），只读节点重跑走 `build_invocation` 的只读档（`dispatch.rs:1382`）不会重复写。G1 提示 `writableFallbackHintOf`（`dispatch.ts:200`）渲染于 `DispatchOverlay.tsx:388`；结果行 `fallbackSkippedLineOf`（`dispatch.ts:205`）渲染于 `DispatchOverlay.tsx:510`。Rust 双测试（`dispatch.rs:2807` 断言仅 1 条留痕、`:2848` 断言盲节点双留痕且无跳过标）+ TS `R1-1` 两条真实存在 |
| 2 | **已核实修好** | `pickOne` 候选过滤要求 `kind` 与 `target.role` 双一致（`dispatch.ts:370-376`）；`R1-2` 用例擂台候选 + 同父顾问并存，断言顾问 adopted 不被清 |
| 3 | **已核实修好** | TS 侧 `presetWithRoleDefaults`（`dispatch.ts:67`）两个盖章调用点都已换用（`GraphTab.tsx:327`、`OrchestrationPanel.tsx:133`）；Rust 侧 `fill_role_defaults` 同步跳过（`orchestration.rs:463`，堵住 `list_presets` 返回已填目标的另一半）。`removeDraftNode` 双重守卫（`mutate.ts:79-82` + UI 条件 `GraphTab.tsx:510`）。测试：`R1-3` 5 条 + `arena_role_defaults_stay_empty`（`orchestration.rs:1098`） |
| 4 | **已核实修好** | `NodePanel.tsx:320-334`（draft 可编辑、非 draft 只读）、`updateNodeUserText`（`mutate.ts:68`）、第三条空串（`stamp.ts:12`）+ placeholder「自定义视角」（`NodePanel.tsx:327`）；测试 `R1-4` + 评审团用例 `texts[2]===''` |
| 5 | **已核实修好（口径偏离已注明）** | `Premise.attachments` 双侧（`orchestration.rs:257` 带 `#[serde(default)]`、`premise.ts:18`），进根材料（`premise.ts:41`），往返 + 旧格式解析测试（`orchestration.rs:945/968`）。实现是路径输入框非「勾选文件」（`SettingsModal.tsx:670-678`），报告已说明原因（界面上本无附件数据源），可接受 |
| 6 | **已核实修好** | `TargetEditor` 备选行（`NodePanel.tsx:232-257`，写回经 `cloneTarget` 深拷贝）；G1 onChange 即时 `updateNodeTarget` 写回（`DispatchOverlay.tsx:431`）；测试 `R1-6` |
| 7 | **已核实修好** | `appendArbitration`：`visibility:'盲'`、`upstreams:undefined`、强制只读、parents=共同材料祖先（`dispatch.ts:455-468`）；G1 盲节点自动走「本次外发」分支（`DispatchOverlay.tsx:449`）；中性描述「甲方/乙方」无 agent/model（`dispatch.ts:424-427`），截断省略号 + 转折词优先（`:405-422`）。测试：`R1-7` 两条词表断言 + 盲语义用例 + `R1-19b`（serializeContext 产物不含双方正文，与 `context.ts` 盲分支实读一致——仲裁节点祖先闭包只含材料节点，双方 reply 根本不在闭包里） |
| 8 | **已核实修好** | `last_graph_id` 双侧（`orchestration.rs:259`、`premise.ts:19`），`matchPremiseResults` 纯函数按 id 读图（`premise.ts:68-87`），`last_checked_at` 用回写时刻；设置页建图只记 id（`SettingsModal.tsx:280-287`），回写失败走 `setPremiseNote`（`:200`）。测试 `R1-8/15` 3 条 |
| 9 | **已核实修好** | 备选留痕 vendor/billing 取备选口径（`dispatch.rs:1246-1256`，`infer_vendor` 改 `pub(crate)` 是 `resources.rs:1367` 唯一改动），测试 `fallback_record_uses_fallback_billing_and_vendor`（`dispatch.rs:2889`，主/备各归各）；节点卡整行 `fallbackNoticeOf`（`GraphTab.tsx:601`）；报告文案与代价说明已改 |
| 10 | **已核实修好** | `ROLES` 加仲裁（`NodePanel.tsx:31`）；`roleDefaults` 补顾问/仲裁（`types.ts:112`）；`parseAdvise` 用 `/^\/advise(?:\s|$)/`（`dispatch.ts:479`）；台账写回失败提示不吞（`SettingsModal.tsx:200`）；测试 `R1-10` 2 条 |
| 11 | **已核实修好（纯函数口径）** | `k3` 进 `models` 且真走 conflict 判定（`presets-05b.test.ts:167-172`）；`R1-11b/11d` 在；live 测试重写见第 16 条。唯一折扣：G1 下发 fallback 的测试停在 `fallbackSpecOf` 层而非完整请求体——`buildRequest` 是组件内闭包不可导出，接线点 `DispatchOverlay.tsx:140` 仅一行，可接受 |
| 12 | **已核实修好（标题字面有偏差）** | `PRESET_NODE_TEXTS` 要求 `preset.builtin`（`stamp.ts:60`）；标题 + 说明行在（`DispatchOverlay.tsx:568-569`）。但标题用 `group.title`，带角色阶段显示「第三方仲裁 · 执行」而非「第 N 阶段」字面——识别目的达成，算可接受偏差 |
| 13 | **已核实修好** | `kind: materials.length >= 2 ? 'accept' : 'turn'`（`dispatch.ts:457`），与 `normalizeNodeKinds` 的加载口径（`layout.ts:116`）一致不冲突；测试 `R1-13` 双向断言 |
| 14 | **已核实修好** | `sameParentsResultCount`（`dispatch.ts:385-398`）+ 按钮条件（`DispatchOverlay.tsx:522`）；`R1-14` 断言同阶段不同父不出。小瑕疵见下「新缺陷扫描」a |
| 15 | **已核实修好** | `outcome.slice(0,500)` 比较（`premise.ts:81`）；建图不写 `last_checked_at`（`SettingsModal.tsx:280-287` 只有 `last_graph_id`）；测试含「未变不重写」用 `toBe(premise)` 断言对象未换 |
| 16 | **已核实修好** | `enforce_live_success` 移到测试最开头（`dispatch.rs:3454-3458`，受限环境整体早退，不存在跳过仲裁段的路径）；dispute 按 `node_id` 映射（`:3486-3493`）；甲方/乙方 + 泄漏词断言（`:3500-3503`）；仲裁节点按盲形态构造且断言厂商三方互异（`:3511-3517`）。`#[ignore]` 保持，未真跑 |
| 17 | **已核实修好** | `node_agents`（`dispatch.rs:350-360`）在挑选/登记/释放三处生效（`:501-507`、`:524-526`、`:556-560`）；且 `run_one` 是线程内整体执行后才发结果，主+备全程占坑，记账无洞。测试 `dispatch.rs:2936` 判别力可信（时间戳是秒级截断，但截断单调，`B.started ≥ A.ended` 恒成立，不飘）。worktree 显式标记「不采纳」理由成立：第 1 条后可写节点（含 worktree）根本不进备选，降级路径不可达 |
| 18 | **已核实（口径）** | 实点核对：初版 7 条（orchestration 4 + dispatch 3）+ R1 新增 7 条 = 14 非 ignored + 1 ignored，与报告一致；全套计数 189/10 待编排者复跑确认 |
| 19 | **已补** | a) `R1-3` + `arena_role_defaults_stay_empty`；b) `R1-19b` 断言 prompt 含根材料与中性描述、不含双方正文；c) `R1-19c` 断言 5 节点、原验收 failed、`retries==[1]`、`continueRunning` 为真——断言的都是目标行为本身 |

## 修正是否引入新缺陷（回归扫描）

- **第 2/3/4 期主路径无回归**：`dispatchableNodeIds`（draft + 父材料/done）、`reducePipelineStep`（验证失败→修正轮、NEEDS_FIX→纠正轮、封顶 3）、`applyDispatchResults`、`parseAcceptance` 语义未动；备选标记行只在 `fallbackFrom` 非空时前置，`validation_failed` 仍不触发备选（`should_try_fallback` 只认 error/timeout/identity_mismatch，`dispatch.rs:1186`）。仲裁节点 draft+target+材料父，天然可派发，无死路。
- **契约镜像**：`Target.fallback` / `DispatchNodeRequest.fallback` / `DispatchNodeResult.fallbackFrom/To/Skipped` 双侧对齐，Rust 侧全带 `#[serde(default)]`，TS 侧 optional；留痕契约只有 `attempt` 追加（`dispatch.rs:271-273`），旧记录可解析——只追加成立。
- **真机不会炸的路径复核**：盲节点备选重跑清目录后 `prepare_outbound` 的非空拒绝（`dispatch.rs:1573-1577`）不再触发；并发记账最坏情形是等当前节点结束（有 timeout 兜底），无死锁路径；前提附件经盲分支材料拷贝，路径合法。

**瑕疵（均非阻断）**：
- a) `sameParentsResultCount` 不按 kind/role 过滤而 `pickOne` 过滤：同父「1 擂台 + 1 顾问」都完成时擂台卡会出「选它」，点了也只 adopted 自己、不误清顾问——行为安全，仅按钮语义略宽。
- b) `fallbackSkipped` 只活在结果屏：Reply 正文与留痕 frontmatter 都不含「未切备选（可写节点）」，事后只看图/留痕无法与普通失败区分（对照：切备选那条是进正文的）。
- c) G1 备选 onChange 写回的是整个本地 `target`（`DispatchOverlay.tsx:431`），若用户先改了主目标再动备选，主目标编辑也被落盘且取消弹窗不回滚——与第 6 条意图同向，属轻微口径扩大。

## 结论

**R1 之后 TASK-05B 达到可交付**：19 条全部闭合（18 修 + 1 项合理不采纳），静态核对未发现回归或会在真机崩溃的路径。

仍未闭合项（按严重度降序，全部为低及以下，可作后续轮次/Later）：
1. `fallbackSkipped` 不进 Reply/留痕，图上事后无法区分「可写节点未切备选」（需 Reply 或 frontmatter 追加字段，本期明确不加）。
2. 「选它」按钮可见性未按 kind/role 收窄，与 `pickOne` 口径存在无害的宽边界。
3. 前提附件入口是路径文本输入，「勾选文件」原口径未做（已注明原因，待界面有附件数据源时补）。
4. 仲裁区标题字面（`group.title` 可能是角色名）与「第 N 阶段」措辞不完全一致。
5. `live_arena_and_arbitration` 仍为 `#[ignore]`，真实三机形态未经真跑验证——这是编排者验收动作，不是代码缺口。