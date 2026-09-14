## 逐条核对（12 条）

**1. 备选重跑后的 session 归属 —— 已核实修好**
`src/dispatch.rs:1302` 在 `run_fallback` 内 `result.session_id = String::new()`（仅备选分支；留痕 `DispatchRecord.session_id` 仍留真实值查账，:1570）；TS 侧 `src-ui/src/graph/dispatch.ts:129-134` 对 `fallbackNoticeOf(parent.reply) !== null` 的父回复直接返回 undefined（兼容 R2 前落盘、Reply 里还留备选 session 的旧图）。`applyDispatchResults` 取 `session_id: result.sessionId || undefined`（dispatch.ts:242），而派发请求里 session 的唯一来源是 `continuationSession`（DispatchOverlay.tsx:135）→ 主通道拿不到备选 session。测试：`R2-1`（presets-05b-r2.test.tsx:60-80，带标记/普通两条）+ Rust `fallback_reruns_once_and_records_both_attempts`（dispatch.rs:2867 断言备选结果 session 为空）。

**2. 前提附件 —— 已核实修好**
文件选择器 `SettingsModal.tsx:279-300`（`open({multiple:true})`，`dialog:default` 在 capabilities/default.json:30，与 CenterPanel.tsx:1143 同款）；手输入口保留但保存前校验 `validatePremiseAttachments`（premise.ts:60-75，相对路径/同名 basename），选择器路径同样校验（SettingsModal.tsx:288）；不过不落盘、走 `setPremiseNote`（:270/:290 → 渲染于 :716）。测试 `R2-2` 三条（r2.test.tsx:83-104）。

**3. raw 转存互相覆盖 —— 已核实修好**
`record_stdout` 加 `attempt`，`<node>.raw.txt` / `<node>.fallback.raw.txt`（dispatch.rs:1164-1180）；调用点 :1575 传 `attempt`（"主"/"备" 分别来自 run_one :1236 与 run_fallback :1298）。留痕 `.md` 另有 `while path.exists()` 去重（:1865-1872），两处都不覆盖。测试 :3160（路径不同、内容各自完整），既有 `r3_7` 改用新签名并覆盖转存失败分支（:2133-2151）。

**4. 备选 vendor —— 已核实修好**
TS 镜像 `tauri.ts:436` `fallback.vendor?`；Rust `FallbackSpec.vendor` 带 `#[serde(default)]`（dispatch.rs:113-115）；`run_fallback` 优先下发值、空才 `infer_vendor`（:1280-1289，"其他"→空）；前端 `fallbackSpecOf(target, models)`（dispatch.ts:197-208）在 `DispatchOverlay.tsx:139` 传 models。测试 :3117（下发 Moonshot AI）+ `R2-4` 两条。

**5. 三处组件级断言 —— 已核实修好**
`DispatchBits.tsx:8/15/22` 三个真元件，`DispatchOverlay.tsx:387/510/522` 确实渲染它们（原 JSX 条件已换掉）；测试用 `renderToStaticMarkup` 渲染元件 JSX（r2.test.tsx:136-174），不是又一层纯函数。（保留：这是薄封装，断言的是元件而非 DispatchOverlay 整树——任务书允许的抽法；未引入 @testing-library。）

**6. live 同义反复断言 —— 已修（有保留，见未闭合项 2）**
`record_field`（dispatch.rs:3721）从擂台留痕读回 vendor（:3734-3735），仲裁侧 `arb_field` 从仲裁留痕读回 role/write/vendor 再互异断言（:3790-3793）；`dispute_excerpt` :3221 改「折叠空白 + 200 字 + 省略号」，`#[ignore]` 保持。保留：`live_node` 仍由 helper 写死 `write:"只读"` 与 agent→vendor 映射（:3267-3273），故 vendor/write 断言实质仍由 helper 输入决定（vendor 落空为 "" 也能通过 `assert_ne`）。

**7. 仲裁只读进 Rust 强制 —— 已核实修好**
`dispatch.rs:431-439` 门禁角色集合加「仲裁」，错误文案按角色输出，与 `validate.ts:26` 同口径；测试 `arbitration_role_is_forced_readonly_by_engine`（:3177）双向（可写拒、只读放行到 missing agent）。

**8. 仲裁标题文案 —— 已核实修好**
`DispatchOverlay.tsx:565`「第三方仲裁 · 第 {group.stage} 阶段」，说明行 :566 未动。

**9. 报告降级表述 —— 已核实修好（口径）**
REPORT-05B.md 第 1/7/11 条内各有〔R2 第 9 条降级〕注记，按实际断言改写（重跑发生、词表只对短 fixture、下发仅纯函数级）。

**10. 「选它」按 kind/role 收窄 —— 已核实修好**
`sameParentsResultCount` 加 `kind` 与 `target.role` 一致（dispatch.ts:398-411），与 `pickOne`（:378-393）完全对齐；`R2-10`（1 擂台 + 1 顾问两侧 count=1）；无回归——内置擂台三节点同 role=执行（orchestration.rs:359-368）→ count=3，按钮不消失，`R1-14` 同角色用例仍成立。

**11. `fallbackSkipped` 进留痕 —— 已核实修好**
`DispatchRecord` 追加 `write`/`fallback_skipped`（dispatch.rs:279/:282，均 `#[serde(default)]`，追加在 `attempt` 之后 → 只追加）；写入在 `run_one_attempt` :1533-1539（主失败且属触发集合才标）/ :1572；Reply 无新字段（types.ts:14-26，`applyDispatchResults` 不写）；旧格式往返测试 :2355，落盘断言 :2955-2963、反断言 :3000。

**12. G1 备选 onChange 只写 fallback —— 已核实修好**
`updateNodeFallback`（mutate.ts:69-73）只 patch `target.fallback`（`cloneTarget` 深拷贝，types.ts:126-131）；`DispatchOverlay.tsx:431` 已改用它，本地未确认的主目标留到 `start` 时的 `markPending(graphWithTargets)` 才落盘；`R2-12` 测试覆盖写/清空两态。

## 回归扫描（R2 新增缺陷）

- **派发路径**：R2 对 `dispatch.ts` 只动了 `continuationSession`/`fallbackSpecOf`/`sameParentsResultCount`，`dispatchableNodeIds`、`nextStageStep`、`reducePipelineStep`、`applyDispatchResults`（含 `parseAcceptance` 的 PASS/NEEDS_FIX 尾巴读法）未动；`R1-3`、`R1-19c` 用例仍在，无回归。`fallbackSpecOf` 第二参可省，R1-6 已同步补 `vendor` 键。
- **留痕契约**：`aicross-dispatch/1` 未改名，新增字段全部追加在结构尾部，解析侧全 `#[serde(default)]`；`DispatchNodeResult.fallback_skipped`（camelCase → `fallbackSkipped`）与 tauri.ts:482 对齐。
- **镜像**：`FallbackSpec.vendor`、`DispatchRecord.write/fallback_skipped`、`DispatchNodeResult.fallback_skipped` 三处 Rust 侧均 `#[serde(default)]`，TS 侧 `vendor?` 对齐。
- 未发现阻断性新缺陷；仅文档与测试强度类瑕疵（见下）。

## 结论

**R2 之后 TASK-05B 可交付**（12 条均已按任务书落地，193/172/build 与 R1 真跑记录一致，无阻断性回归）。

仍未闭合项（按严重度）：
1. `live_arena_and_arbitration` 的 **R2 版断言未真机复跑**（上次真跑是 R1 版；R2 只保证编译与离线路径，`write`/`vendor` 读回路径未在真机验证一次）。
2. 第 6 条**只治标**：vendor/write 仍由 helper 写死输入（vendor 落空也能过 `assert_ne`），真机断言对引擎行为的判别力仍弱。
3. Rust `dispute_excerpt` 缺前端「转折句优先」规则，严格说不是「同一个口径」（任务书允许降级为 200 字+省略号）。
4. 「已切备选」靠回复正文首行标记识别：标记行会随 `serializeContext` 进下游 prompt，且模型正文恰好以该前缀开头会误判（已在「不确定处」记录，根治要 Reply 契约字段）。
5. 文档/边界小疵：REPORT-05B R1 第 6 条仍写 `updateNodeTarget` 写回（与 R2-12 冲突）；前提附件校验只在设置页保存路径，手改 `premises.json` 或遗留相对路径不会在「抽查」前拦。