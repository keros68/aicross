复审完成。核对方式：通读 `fix-05b-r2.md` 12 条 → `REPORT-05B.md` 修正轮 R2 节 → 全部涉及源码与测试（未跑测试，验证结果采信编排者复跑：cargo 193 / vitest 172 / build 零 warning；live 在 R1 后 1 passed 61s）。

## 逐条核对（12 条）

**1. 备选重跑后的 session 归属 —— 已核实修好**
- `src/dispatch.rs:1302`：`run_fallback` 备选作答后 `result.session_id = String::new()`，留痕保留真实 session 仅供查账；`src/dispatch.rs:2867` 有对应 Rust 断言（备选结果不带 session_id）。
- `src-ui/src/graph/dispatch.ts:129`：`continuationSession` 对首行带「已切备选」标记的父回复直接返回 undefined，兼容修正前已落盘、Reply 里仍留备选 session 的旧图（双保险）；`applyDispatchResults`（dispatch.ts:242）`session_id: result.sessionId || undefined` 使新图 Reply 干脆无 session。两条防线都验证到，主通道（同目标共享子节点）不会再拿到备选 session。
- 测试：`presets-05b-r2.test.tsx:83-92`（带标记返回空 / 普通回复照常续）。

**2. 前提附件文件选择器 —— 已核实修好**
- `SettingsModal.tsx:279-300`：`pickPremiseAttachments` 走 `@tauri-apps/plugin-dialog` 动态 import `open({multiple:true})`（与 `CenterPanel` 同款），合并去重后保存；`capabilities/default.json:30` 已有 `dialog:default`。手输入口保留，两路保存前都过 `validatePremiseAttachments`（`premise.ts:60-74`：相对路径正则覆盖盘符/UNC/POSIX 根，basename 重名检测），不过不落盘、走 `setPremiseNote` 提示（SettingsModal.tsx:268-272、288-292）。
- 测试：R2-2 三条（相对路径、重名、三类绝对路径通过）。

**3. 主/备 raw 转存覆盖 —— 已核实修好**
- `src/dispatch.rs:1164-1182`：`record_stdout` 加 `attempt` 参数，备写 `<node_id>.fallback.raw.txt`；调用点 `:1575` 从 `run_one_attempt` 把 attempt 传进来。
- 测试：`record_stdout_keeps_primary_and_fallback_dumps_separate`（`src/dispatch.rs:3160`，两路径不同、都存在、内容完整）；`r3_7` 已改新签名并覆盖转存失败分支（`:2138/2146`）。

**4. 备选 vendor 前端下发 —— 已核实修好**
- 契约镜像：`FallbackSpec.vendor`（`src/dispatch.rs:113-115`，`#[serde(default)]`）↔ `src-ui/src/tauri.ts:436`（`vendor?`）。前端 `fallbackSpecOf(target, models)`（dispatch.ts:197-208，provider 精确匹配优先、退 agent+id）在 `buildRequest` 下发（DispatchOverlay.tsx:139）；Rust 优先用下发值、空才 `infer_vendor`（dispatch.rs:1280-1289）。
- 测试：`fallback_record_prefers_dispatched_vendor_over_inference`（dispatch.rs:3117，k3-256k 推断不出、留痕用下发的 Moonshot AI，主留痕不受影响）+ R2-4 两条 TS。

**5. 组件级断言三处 —— 已核实修好**
- `DispatchBits.tsx` 是真 JSX 组件（`<div className="dispatch-warning">`、`<button disabled>`），且 DispatchOverlay 原 JSX 条件已改由这三个组件渲染（`DispatchOverlay.tsx:387`、`:510`、`:522-529`）——测的就是生产渲染路径，不是又一层纯函数。
- 测试：R2-5 三条用 `renderToStaticMarkup` 断言渲染/不渲染/禁用态；按钮可见性部分另由 R2-10、R1-14 在纯函数层覆盖，口径一致。

**6. live 测试同义反复断言 —— 已核实修好（一处措辞见下）**
- `src/dispatch.rs:3721-3735`：两边 vendor 从擂台留痕 frontmatter 读回（`record_field`）；`:3790-3793`：仲裁留痕读回 `role: 仲裁`、`write: 只读`、vendor 与两边互异再断言；`dispute_excerpt`（`:3221-3227`，200 字+省略号）替换 `take(400)`；保持 `#[ignore]`（`:3689`）。
- 措辞瑕疵：报告称 `write`「非 helper 输入」不准确——`live_node` 请求里本就传 `write: "只读"`（`src/dispatch.rs:3262`），该断言验证的是「留痕 write 字段 = 引擎生效值」这条管线，不是盲强制只读本身的真机证据（后者由离线门禁测试覆盖）。表述问题，不改判定。

**7. 仲裁只读进 Rust 强制 —— 已核实修好**
- `src/dispatch.rs:432`：只读门禁角色集合加「仲裁」，与 `src-ui/src/graph/validate.ts:26` 同口径；错误文案按角色输出（`:439`）。测试 `arbitration_role_is_forced_readonly_by_engine`（`:3177`，仲裁+单写者被拒 / 仲裁+只读穿过门禁死在 missing agent）。

**8. 仲裁标题文案 —— 已核实修好**
- `DispatchOverlay.tsx:565`：`第三方仲裁 · 第 {group.stage} 阶段`，不再借用会显示角色名的 `group.title`；说明行不变（`:566`）。

**9. 报告降级表述 —— 已核实修好**
- `REPORT-05B.md:97、103、107` 三处均以〔R2 第 9 条降级〕注记按实际断言改写（重跑发生≠成功；词表断言仅短 fixture；fallback 下发仅纯函数级），原条目可读、未删史。

**10. 「选它」可见性收窄 —— 已核实修好**
- `src-ui/src/graph/dispatch.ts:398-413`：`sameParentsResultCount` 在同父之外追加 `kind`、`target.role` 一致，与 `pickOne`（`:378-393`）候选口径对齐；调用点 `DispatchOverlay.tsx:523`。测试 R2-10（同父擂台+顾问 count=1 不出按钮），R1-14 既有用例不受影响。

**11. `fallbackSkipped` 进留痕 frontmatter —— 已核实修好**
- `DispatchRecord.fallback_skipped`（`src/dispatch.rs:280-282`，`#[serde(default)]`，追加字段）；`run_one` 预交跳过原因（`:1219-1237`），`run_one_attempt` 在备选触发状态集合内写进结果与留痕（`:1533-1539`、`:1572`、`:1614`），validation_failed 不标。测试：`legacy_frontmatter_without_r2_fields_parses_with_empty_defaults`（`:2355`，旧格式三字段取空往返）+ 可写用例断言落盘留痕带 `fallback_skipped: 可写节点`（`:2955-2963`）+ 盲重跑反断言（`:3000`）。

**12. G1 备选 onChange 只写 fallback —— 已核实修好**
- `src-ui/src/graph/mutate.ts:69-73`：`updateNodeFallback` 只 patch 图节点 `target.fallback`；调用点 `DispatchOverlay.tsx:431` 传的是 `graph`（图上已确认状态），本地 `targets` state 里未确认的主目标修改不落盘。测试 R2-12（写/清备选主目标保持 codex/gpt 不动）。

## 回归抽查（未发现新缺陷）

- 派发路径：`dispatchableNodeIds`、`nextStageStep`、`reducePipelineStep`、`parseAcceptance`（PASS/NEEDS_FIX）均未改动；`applyDispatchResults` 仅追加 notice 前缀与备选字段；R1-19c 保证带 `fallbackFrom` 的 validation_failed 照常生长修正轮。
- 留痕契约：schema 仍 `aicross-dispatch/1`（dispatch.rs:1541），attempt/write/fallback_skipped、Request.fallback.vendor、Result.fallbackSkipped 全是追加字段；Rust/TS 镜像 `#[serde(default)]`/`?` 两侧齐（FallbackSpec.vendor、DispatchRecord 三字段、Target.fallback/FallbackTarget、Premise.attachments/last_graph_id）。
- 前提链路：`validatePremiseAttachments` 正则三种绝对形态正确；选择器返回值 `string[]|string|null` 三态都处理；图界面/节点面板/G1 的既有交互未被 DispatchBits 抽取破坏。
- 计数自洽：R2 新增 Rust 4 条（189→193）、TS 12 条（160→172），与报告一致。

## 结论

**R2 之后 TASK-05B 达到可交付**——12 条全部闭合且修法与测试对得上任务书要求，未发现修正引入的新缺陷，四项自动验证全绿。

仍未闭合的项（按严重度）：

1. **R2 改写后的 `live_arena_and_arbitration` 未真机复跑**（唯一实质开口）：真跑的是 R1 版（61s），R2 换了 frontmatter 读回与 `dispute_excerpt` 口径后未验证；读回解析依赖 YAML `key: value` 前缀格式，建议合并前或下一窗口跑一次 `cargo test -- --ignored live_arena_and_arbitration`。
2. **「已切备选」标记行机制本身**：依赖回复正文首行（Reply 无契约字段），正文恰好以「（已切备选：」开头会误判不续聊（fail-safe 方向，代价小）；标记行随 `serializeContext` 进下游上下文并随图落盘。报告「不确定处」已在案，属已知取舍，若要根治需给 Reply 开字段（下期）。
3. **仲裁同厂商校验只在建节点时拦**：建成后改目标 `validateNode` 无法回指双方，只剩通用提示（R1 起在案，未闭合）。
4. 小项：报告第 6 条「write 非 helper 输入」措辞过强（见上）；手输附件按逗号分隔，含逗号路径会被拆坏（pre-existing，非本轮引入）。