# REPORT-05B — 第 2 期预设（TASK-05 B 节）

日期：2026-09-14

## 结果

TASK-05 B 节六个预设全部落地，形态按任务书第 0 节第 1 条：每个预设 = 一个 `stamp` 图章或一小段结果屏/输入动作，没有新引擎、没有新协议。图契约本期只追加一个字段：`Target.fallback`。

四项自动验证最终结果：

- `cargo check`：通过，0 warning（`grep -cE "^(warning|error)"` 输出 `0`）。
- `cargo test`：`test result: ok. 182 passed; 0 failed; 10 ignored`。
- `cd src-ui && npx vitest run`：`Test Files 12 passed (12)` / `Tests 135 passed (135)`。
- `cd src-ui && npm run build`：通过（chunk > 500 kB 警告是既有项，归 C 节）。

未起窗口、未发真机请求、未 commit、未 push。真机擂台+仲裁已写成 ignored 测试（见下）。

## 做了什么

### 契约追加（本期唯一）

- `Target.fallback?: { agent, provider?, model }`：`src-ui/src/graph/types.ts` 与 `src/orchestration.rs::Target`（`FallbackTarget`，`#[serde(default)]`）双侧镜像；`cloneTarget` 深拷贝。
- 非图契约的伴随追加（`aicross-dispatch/1` 允许追加字段、不改名不删）：
  - `DispatchNodeRequest.fallback`（备选随请求下发）、`DispatchNodeResult.fallback_from/fallback_to`（备选重跑产生时带主/备路由）。
  - 留痕 frontmatter 追加 `attempt: 主|备`。
  - 新文件 `~/.aicross/premises.json`（前提台账），不动图契约。

### 六个预设的落点

- **擂台**：内置预设 `builtin-arena`（一层 3 个共享/执行/只读节点，目标留空）。结果屏每张卡在同阶段 ≥2 路时出现「选它」：`pickOne`（`graph/dispatch.ts`）把被选节点 `adopted=true`、同父其它候选清 `adopted`，并经新 prop `onFocusNode` 让 GraphTab 把它设为第 1 列末端 + 焦点。不新增契约字段。
- **评审团**：内置预设 `builtin-jury`（一层 3 个盲/验证/只读节点）。视角文本（安全/性能/需求覆盖）在 `stamp` 时按 `PRESET_NODE_TEXTS`（`graph/stamp.ts`，以预设 id 索引）写进各节点 `user.text`；`serializeContext` 原样把它拼在盲审冻结模板之后。未加 `Target.perspective` 之类字段。
- **仲裁**：结果屏每个 ≥2 路成功的阶段一组「分歧仲裁」编排区：选甲方/乙方（默认前两路）、中性描述自动生成（`neutralDisputeSummary` 拼「A 认为…；B 认为…」，不带立场词）且可改、目标下拉排除双方厂商；「仲裁」按钮走 `appendArbitration` 建盲 turn 节点：`parents = 双方共同材料祖先`（kind=material 的共同祖先，一般即根）、`visibility=指定上游` 且 `upstreams` 指向同批材料、`role=仲裁`、`write=只读` 强制。同厂商由 `validate.ts::arbitrationVendorConflict` 拦（UI 排除 + 纯函数拒绝双保险）；`validateNode` 追加「仲裁节点只能只读」。
- **前提抽查**：SettingsModal 新「前提」页：`premises.json` 台账（`{id, text, recorded_at, last_checked_at?, last_result?}`，Rust 命令 `load_premises`/`save_premises`），一行一条、增删、记录/最近抽查日期并列展示。「抽查」按 `graph/premise.ts::buildPremiseCheckGraph` 建图：根材料 = 前提原文，一个盲验证节点，`user.text` = 冻结模板 `fixtures/prompts/premise-check.txt`；目标下拉排除 roles.json 三个角色所用厂商，可不选（到 G1 再选）。建图后更新 `last_checked_at`；下次打开本页时按标题 `前提抽查 #<id前8>` 匹配已跑完的图，把验证节点结论写回 `last_result` 并列展示（截 500 字）。
- **主备/熔断**：`run_one` 拆成外壳 + `run_one_attempt`（原函数体不动，签名加 `attempt`）；主目标以 `error/timeout/identity_mismatch` 收场且有备选时 `run_fallback` 重跑一次——同可见性、同写权限，只换 agent/provider/model；清 session_id（换通道不续旧会话，发完整 prompt，费用与暴露面按全量算）、备选不再套备选；盲节点重跑前清隔离目录（否则 prepare_outbound 拒非空）。留痕两条（attempt 主/备），结果带 `fallbackFrom/fallbackTo`。G1 确认屏每路加「备选」下拉，`routeLine` 决策行追加 `｜ 备选=a/m`；结果卡和图节点卡显示「已切备选：主 x → 备 y」（节点卡靠回复正文首行标记 `（已切备选：…）` 识别，`fallbackNoticeOf` 提取；Reply 不加字段）。`validation_failed` 不触发备选（备选换通道不换产物，验证失败走既有修正轮）。
- **顾问**：中栏输入 `/advise @agent/model 问题` → `parseAdvise` 盖 `role=顾问`、`visibility=盲`、`write=只读` 的 turn 节点；`promptForRole` 追加 `fixtures/prompts/advisor.txt` 模板（只要方案不要执行、不接管）；回复卡标「顾问意见」（GraphTab meta 行）；`NodePanel` 的 `ROLES` 加「顾问」。
- `OrchestrationPanel` 的 FUTURE_PRESETS 移除已落地的六个，剩 红蓝/群聊/蜂群。

### 测试

- TS（`src-ui/src/graph/__tests__/presets-05b.test.ts`，11 条）：擂台图章（节点数/可见性/写权限/parents）、评审团图章（含视角文本按序进 `user.text`）、「选它」adopted 记账、仲裁建节点字段、仲裁拦同厂商、备选重跑记账（标记行 + `fallbackNoticeOf`）、`Target.fallback` 的 JSON 往返与 `cloneTarget` 深拷贝、`routeLine` 显示备选、`parseAdvise`、`validateNode` 仲裁只读、前提抽查图构建与结论回读。（初稿写「10 条」，修正轮 R1 按实数改 11。）
- Rust：`orchestration.rs` 追加 `builtin_arena_and_jury_shapes`、`target_fallback_json_roundtrip`、`premises_roundtrip`、`premises_broken_file_loads_empty_and_save_refuses_to_overwrite`；既有断言的内置预设数 3→5 同步更新。`dispatch.rs` 追加 `fallback_predicate_only_triggers_on_channel_failures`、`fallback_reruns_once_and_records_both_attempts`（主备各一条留痕 + 结果带路由）、`fallback_blind_attempt_gets_fresh_material_dir`。
- 真机（ignored，留编排者）：`dispatch::tests::live_arena_and_arbitration`——擂台 pi/zai-coding-cn/glm-5.3-flash 与 codex/gpt-5.6-luna 两家共享作答，再把两边正文拼成中性描述盲发 kimi/kimi-code/k3-256k 仲裁。跑法：`cargo test -- --ignored live_arena_and_arbitration --nocapture`。（构造在修正轮 R1 按真实仲裁形态重写，见末节第 16 条。）

## 没做什么

- A 节（ACP）、C 节（打包与体验）不在本块范围。
- 仲裁的「分歧」检测：归类模型尚未接入，结果屏不做自动分歧判定；改为每个 ≥2 路成功的阶段都给一组仲裁编排区，由用户选双方。这是对本节「分歧条目上的按钮」的落地口径调整。
- 擂台「盖章时选 2–4 家」没有做成盖章弹窗：盖出 3 个空目标节点，在 G1 确认屏逐家选（那里本就有每路目标下拉）。初稿写「增删路数走图界面」不成立——图界面当时删不了节点；draft 节点删除在修正轮 R1 补上（见末节第 3 条）。
- 前提抽查的「用户勾选的原始附件」未做：台账条目按任务书 schema 无附件字段，根材料只放前提原文。（修正轮 R1 已补，见末节第 5 条。）
- 设置页建抽查图后不自动开 tab（开 tab 要动 CenterPanel/app-state，超出本块范围）；页面提示用户到对话图起始页打开。
- 顾问/仲裁没有进 `fill_role_defaults` 的角色默认填充（roles.json 只有三个角色，任务书未要求加）。

## 契约追加清单（供合并与下游对账）

| 位置 | 追加 | 说明 |
|---|---|---|
| 图契约 `Target`（两侧） | `fallback?: {agent, provider?, model}` | 本期唯一图契约字段 |
| `DispatchNodeRequest` | `fallback?: {agent, provider, model}` | camelCase，可省 |
| `DispatchNodeResult` | `fallbackFrom`/`fallbackTo: string` | 空串 = 未切备选 |
| 留痕 frontmatter（`aicross-dispatch/1`） | `attempt: 主\|备` | 追加字段，旧记录解析不受影响 |
| `~/.aicross/premises.json` | `{version:1, premises:[{id,text,recorded_at,last_checked_at?,last_result?}]}` | 新文件；损坏文件读侧给空、写侧拒写 |
| 新 Tauri 命令 | `load_premises` / `save_premises` | 已注册进 `server.rs` |
| 新冻结模板 | `fixtures/prompts/advisor.txt`、`premise-check.txt` | 与既有四份模板同目录 |
| 内置预设 | `builtin-arena`（擂台）、`builtin-jury`（评审团） | 内置总数 3→5 |

（修正轮 R1 在其上又追加：`DispatchNodeResult.fallbackSkipped`、`Premise.attachments`、`Premise.last_graph_id`，均为追加字段，见末节。修正轮 R2 再追加：`DispatchNodeRequest.fallback.vendor`、留痕 frontmatter `write` 与 `fallback_skipped`，同为追加字段，旧记录解析不受影响，见末节。）

## 验证命令与输出原文

```
$ cargo check 2>&1 | grep -cE "^(warning|error)"
0

$ cargo test 2>&1 | grep "test result"
test result: ok. 182 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 4.42s

$ cd src-ui && npx vitest run 2>&1 | grep -E "Test Files|Tests "
 Test Files  12 passed (12)
      Tests  135 passed (135)

$ cd src-ui && npm run build 2>&1 | tail -3
(!) Some chunks are larger than 500 kB after minification. ...   （既有警告，归 C 节）
✓ built in 4.34s
```

## 不确定处

- 「已切备选」在节点卡上的呈现依赖回复正文首行标记（Reply 不加字段的约束下最简做法）。代价写明白：该标记行会随 `serializeContext` 进入所有下游节点的 prompt，并随图落盘长期留存；某模型回复正文若恰好以 `（已切备选：` 开头，`fallbackNoticeOf` 会误报。若后续觉得污染上下文，需要给 Reply 开契约字段，本期未加。
- 擂台的 3 个执行节点原会被 `fill_role_defaults` 填上同一个「执行」角色默认模型——修正轮 R1 已修：Rust 侧 `fill_role_defaults` 与前端 `applyRoleDefaults` 都跳过 `builtin-arena`，盖出即空目标，G1 逐家选（见末节第 3 条）。
- 备选重跑对 worktree/单写者等可写节点：修正轮 R1 起可写节点不再触发备选（结果标「主失败，未切备选（可写节点）」），原「worktree 第二次 `create_worktree` 失败、退回单写者直写项目」的静默降级路径随之不会再走到（见末节第 1、17 条）。
- 仲裁节点的 vendor 校验只在创建时拦；建好后用户改目标成同厂商，validateNode 无法回指双方（分歧双方不进节点字段），只有通用只读提示。

## 修正轮 R1

依据 `tasks/fix-05b-r1.md`（DeepSeek `tasks/review-05b-ds.md` 与 GLM `tasks/review-05b-glm.md` 两家盲审，19 条）。逐条如下；测试文件：Rust 在 `src/dispatch.rs` / `src/orchestration.rs` 的 `#[cfg(test)]`，TS 新增 `src-ui/src/graph/__tests__/presets-05b-r1.test.ts`（25 条）并扩充 `presets-05b.test.ts`（11 条）。未动 `src/channels/`。

1. **备选重跑不得对可写节点重复写项目 —— 已修**：`run_one` 在主失败且配了备选时判 `visibility != "盲" && write != "只读"` 的直接返回主失败结果，填 `fallback_skipped = "可写节点"`（`src/dispatch.rs:1209`；`DispatchNodeResult.fallback_skipped` 追加于 `src/dispatch.rs:202`，camelCase 镜像 `fallbackSkipped` 在 `src-ui/src/tauri.ts:482`、`src-ui/src/graph/dispatch.ts:28`）；盲节点（引擎强制只读）与只读节点照旧走 `run_fallback`。G1 确认屏对「可写节点 + 已选备选」显示「可写节点不自动切备选」（`writableFallbackHintOf`，`src-ui/src/graph/dispatch.ts:200`，渲染于 `DispatchOverlay.tsx:388`）；结果卡显示「主失败，未切备选（可写节点）」（`fallbackSkippedLineOf`，`dispatch.ts:205`，渲染于 `DispatchOverlay.tsx:510`）。worktree 节点同属可写，不再走备选，原静默降级路径失效。已补：`fallback_skipped_for_writable_node_returns_primary_failure`（`src/dispatch.rs:2807`，单写者主失败 → 仅 1 条留痕、`fallback_skipped == "可写节点"`）、`fallback_reruns_for_blind_node_and_marks_no_skip`（`src/dispatch.rs:2848`，盲节点主失败 → 备选重跑、双留痕、无跳过标记）、`R1-1 可写节点不自动切备选`（presets-05b-r1.test.ts，2 条）。〔R2 第 9 条降级：离线环境主备 agent 都不可用，造不出 ok 应答；上述用例断言的是「备选重跑发生」（双留痕 + 路由字段），不是任务书字面要求的「备选重跑成功」。〕
2. **`pickOne` 误清同父不同角色节点 —— 已修**：清候选时要求 `other.kind` 与 `other.target?.role` 都和被选节点一致（`src-ui/src/graph/dispatch.ts:366`）。按任务书先写失败用例确认红（`expect(adopted).toBe(true)` 收到 `false`，`Tests 1 failed`），修后转绿。已补：`R1-2 pickOne：同父不同角色节点互不清 adopted`（根下擂台候选 + `/advise` 顾问并存，选它不动顾问的 adopted）。
3. **擂台「2–4 家」可达 —— 已修**：a) 盖 `builtin-arena` 跳过角色默认填充——TS 侧抽 `presetWithRoleDefaults`（`src-ui/src/graph/dispatch.ts:67`），`GraphTab.tsx:328` 与 `OrchestrationPanel.tsx:133` 两处盖章调用点改用它；Rust 侧 `fill_role_defaults` 同步跳过（`src/orchestration.rs:463`，否则 `list_presets` 返回的内置擂台仍带已填目标，GLM 1.1 点名的另一半）。b) 图界面给 draft 节点加「删除」：`removeDraftNode` 纯函数（`src-ui/src/graph/mutate.ts:79`，守卫：节点存在、是 draft、无任何节点以它为 parent），图界面高亮 draft 节点出「删除」按钮（`GraphTab.tsx:512`）。已补：`R1-3`（4 条：roles 配置下盖章后三路目标为空、`removeDraftNode` 可删/有子拒删/非 draft 拒删）、`arena_role_defaults_stay_empty`（`src/orchestration.rs:1098`）。
4. **评审团「自定义」视角与 `user.text` 编辑入口 —— 已修**：NodePanel 加「节点消息」编辑框，draft 节点可编辑、非 draft 只读展示（`NodePanel.tsx:296-311`；纯函数 `updateNodeUserText`，`src-ui/src/graph/mutate.ts:68`）；`PRESET_NODE_TEXTS['builtin-jury']` 第三条视角文本改空串（`src-ui/src/graph/stamp.ts:11`），空 `user.text` 的 draft 节点编辑框 placeholder 显示「自定义视角」。仲裁中性描述建在 `user.text`，建节点后同样可在面板改。已补：`R1-4 updateNodeUserText 写回`；评审团既有用例加 `texts[2] === ''` 断言。
5. **前提抽查带附件 —— 已修**：`Premise` 双侧追加 `attachments`（`src/orchestration.rs:257`，`#[serde(default)]` 旧台账兼容；`src-ui/src/graph/premise.ts:18`）；设置页每条前提可加附件（`SettingsModal.tsx:715-724`，`setPremiseAttachments`；界面本无附件勾选数据源——`user.attachments` 在界面上没有既有入口——按任务书备选口径用路径输入实现，未新造协议）；`buildPremiseCheckGraph` 把 `premise.attachments` 塞进 `root.user.attachments`（`premise.ts:41`），盲节点引擎会把勾选材料拷进隔离目录。已补：`premises_attachments_and_last_graph_id_roundtrip`（`src/orchestration.rs:945`）、`premises_legacy_format_without_new_fields_parses_empty`（`src/orchestration.rs:968`，旧格式解析两字段为空）、`R1-5 台账 attachments 进根材料`。
6. **备选可在节点上设置并持久化 —— 已修**：`TargetEditor` 加「备选」下拉（`NodePanel.tsx:232-255`，与 G1 同一套分组数据源，可清空，写回 `target.fallback`）；G1 备选下拉改为 onChange 即时 `updateNodeTarget` 写回图节点（`DispatchOverlay.tsx:428-441`），不再只在点「确认派发」时随 `markPending` 落盘。已补：`R1-6 / R1-11c fallbackSpecOf`（节点 `target.fallback` 存在时请求体带 fallback；provider 空串补齐、空白不下发）。
7. **仲裁节点走盲语义 —— 已修**：`appendArbitration` 建的节点 `visibility: '盲'`、不再设 `upstreams`（`src-ui/src/graph/dispatch.ts:433-472`；`context.ts`/`validate.ts` 盲分支不读 upstreams，无冲突），隔离目录、强制只读、盲审前缀、只拷共同材料都走引擎既有盲分支；G1 自动走既有「本次外发」渲染分支（`DispatchOverlay.tsx:455` 条件即 `visibility==='盲'`，无需新代码）；`createArbitration` 目标字面量同步（`DispatchOverlay.tsx:302`）。`neutralDisputeSummary` 重写（`dispatch.ts:424`）：标签「甲方/乙方」、不写双方 agent/model；`disputeExcerpt` 截断加省略号且优先保留含转折词（但/然而/不过/却/反之/相反）的句子。已补：`R1-7 / R1-11b`（3 条：不含双方 agent/model 与「更好/更差/同意/反对/应该」词表、含「甲方」「乙方」标签；超长截断带省略号且转折句优先；产物 `visibility==='盲'`）、`R1-19b`（产物过 `serializeContext` 只含根材料与中性描述，见第 19 条）。〔R2 第 9 条降级：词表断言只对短 fixture 成立——中性描述里嵌的是双方回复原文，真机长正文若恰好含「更好/同意」等词会命中词表；模板层（标签与句式）保证不写立场词，内容层给不了这个保证。〕
8. **前提结论回写按 id —— 已修**：台账追加 `last_graph_id`（`src/orchestration.rs:258`、`premise.ts:19`），回写按 id 读图，不再按标题 `find`；`SettingsModal.tsx:197-217` 的匹配逻辑抽成纯函数 `matchPremiseResults`（`src-ui/src/graph/premise.ts:68`，`SettingsModal.tsx:184-208` 调用）；`last_checked_at` 用回写时刻（`new Date()`），不用 `graph.updated_at`。已补：`R1-8 / R1-15`（3 条：两次抽查同名图按 id 各归各、结论为空不覆盖、结论未变不重写台账）。
9. **备选留痕与文案 —— 已修**：备选那条留痕的 `vendor` 用 `infer_vendor` 按备选模型 ID 推断（推断不出置空）、`billing_source` 取备选 provider（空则置空），不再抄主目标（`src/dispatch.rs:1246-1256`；`src/resources.rs:1367` `infer_vendor` 改 `pub(crate)`，唯一改动）；节点卡改为直接渲染 `fallbackNoticeOf` 返回的整行「已切备选：主 x → 备 y」（`GraphTab.tsx:601`）；报告把「同 prompt」改口为「换通道不续旧会话，发完整 prompt」（本报告「主备/熔断」段）；「已切备选」标记行进下游上下文与图落盘的代价已在「不确定处」首条点明。已补：`fallback_record_uses_fallback_billing_and_vendor`（`src/dispatch.rs:2889`，主 Anthropic/官方订阅 vs 备 DeepSeek/deepseek，两条留痕各归各）；备选记账既有用例扩展为整行 `toBe('（已切备选：主 … → 备 …）')` 断言。
10. **角色清单 —— 已修**：`ROLES` 加「仲裁」（`NodePanel.tsx:31`）；`roleDefaults` 补「顾问→盲/只读」「仲裁→盲/只读」（`src-ui/src/graph/types.ts:112`；DS 建议仲裁用「指定上游」不采纳——第 7 条已把仲裁定为盲语义）；`parseAdvise` 改 `/^\/advise(?:\s|$)/`（`dispatch.ts:475`，`/advisefoo 你好` 不再当命令）；前提回写的 `.catch(() => {})` 改走 `setPremiseNote` 错误提示，不吞（`SettingsModal.tsx:199-201`）。已补：`R1-10`（2 条：parseAdvise 三边界——`/advisefoo` 不识别、裸 `/advise`、正常命令；roleDefaults 顾问/仲裁映射）。
11. **测试补强 —— 已修**：「第三家厂商放行」用例把 `k3` 放进 `models` 并直接断言 `arbitrationVendorConflict === false`，`findModel` 不再落空（`presets-05b.test.ts:167-172`）；中性描述用例改断言词表与标签（并入 `R1-7`，见第 7 条）；G1 把 `fallback` 下发到请求——测试只有纯函数级（`fallbackSpecOf`，并入 `R1-6`）；`buildRequest` 里的接线点（`DispatchOverlay.tsx:135-136` 一行调用）没有组件级断言，接线正确性靠代码核对。〔R2 第 9 条降级表述，原写法让人以为有组件级覆盖。〕`appendArbitration` 无共同材料祖先分支（`R1-11d 无共同材料祖先拒建`）；`live_arena_and_arbitration` 改成真实仲裁形态（见第 16 条）。已补：即上述各测试。
12. **弱键与文案 —— 已修**：`PRESET_NODE_TEXTS` 取值要求 `preset.builtin` 为真（`src-ui/src/graph/stamp.ts:59`，用户预设 id 撞名不套用内置视角文本）；仲裁编排区标题改「第三方仲裁 · 第 N 阶段」并加说明行「系统不判定分歧，双方由用户选」（`DispatchOverlay.tsx:568-569`）；报告计数改 11 条 vitest（本报告「测试」段）、「增删路数走图界面」改口（本报告「没做什么」段）。已补：`R1-12a`（`builtin:false` 且 `id='builtin-jury'` 的用户预设盖章不写视角文本）。
13. **`appendArbitration` 多材料祖先的 kind —— 已修**：`kind` 按共同材料祖先数定（≥2 → `'accept'`，否则 `'turn'`；`src-ui/src/graph/dispatch.ts:433-472` 内），不再靠 `normalizeNodeKinds` 静默改。已补：`R1-13 共同材料祖先 ≥2 时 kind=accept`。
14. **「选它」按钮可见性 —— 已修**：抽 `sameParentsResultCount`（`src-ui/src/graph/dispatch.ts:385`），按钮条件由「同阶段结果 ≥2」改为「同父结果 ≥2」（`DispatchOverlay.tsx:527`）。已补：`R1-14`（同阶段不同父的两路不出「选它」，同父 ≥2 才出）。
15. **前提回读比较与建图日期 —— 已修**：与第 8 条同做——`matchPremiseResults` 按 `outcome.slice(0,500)` 比较，结论超 500 字不再每次打开设置页都判「有变化」重写台账（`premise.ts:68-86`）；`runPremiseCheck` 建图时只记 `last_graph_id`、不写 `last_checked_at`，回写结论时才写（`SettingsModal.tsx:296`）。已补：并入 `R1-8 / R1-15` 三条。
16. **live 测试按真实仲裁形态 —— 已修**：`live_arena_and_arbitration` 重写（`src/dispatch.rs:3451-3532`）：`enforce_live_success` 检查移到测试最开头（受限环境在擂台开跑前即返回，不存在「跑到仲裁段被跳过」的路径）；dispute 文本按 `node_id` 映射取两边正文，不再假设 `results[0]` 是哪家；标签「甲方/乙方」并断言不含 `pi/`、`codex/` 与两个模型名；仲裁节点按 `appendArbitration` 修正后形态构造（`visibility="盲"`、`role="仲裁"`、`write="只读"`、材料只带共同材料祖先），并断言第三方厂商（Moonshot AI）与两边（zai-coding-cn / OpenAI）不同。保持 `#[ignore]`，本轮未真跑。
17. **备选 agent 参与串行记账 —— 已修**：新增 `node_agents`（`src/dispatch.rs:350`，主 agent + 有效备选 agent 的归一化集合），调度循环的挑选（`src/dispatch.rs:503-505`）、登记（`:524-526`）、释放（`:556-560`）都按整集合：节点主/备任一 agent 已活跃即不并发。已补：`fallback_agent_blocks_same_agent_node_from_running_concurrently`（`src/dispatch.rs:2936`，A 的备选 agent = B 的主 agent → B 等 A 结束；已验证判别力：临时屏蔽备选登记后测试如期失败，随后还原）。worktree 降级显式标记：**不采纳**——第 1 条起可写节点（含 worktree）不再触发备选，「降级成直接写项目」的路径不会走到，无需标记；报告「不确定处」已同步改口。
18. **Rust 测试计数口径 —— 已修（口径）**：复核确认 05B 初版 Rust 新测试为 7 条非 ignored（orchestration 4 + dispatch 3）+ 1 条 ignored 真机，GLM 的实数正确；原报告正文未写「8 条」，无需改字，在此钉死口径。本轮后按实数：05B 相关 Rust 测试 14 条非 ignored + 1 条 ignored；全套 `cargo test` 189 passed / 10 ignored。
19. **GLM 5.3 三条测试 —— 已补**：a) 擂台盖章后（含 roles 配置下 `applyRoleDefaults`/`fill_role_defaults`）各节点目标为空——`R1-3` + `arena_role_defaults_stay_empty`（见第 3 条）；b) `R1-19b`：`appendArbitration` 产物过 `serializeContext`，断言 prompt 只含根材料与中性描述、不含双方 reply 正文；c) `R1-19c`：带 `fallbackFrom` 的 `validation_failed` 结果过 `reducePipelineStep`，修正轮照常生长。

### R1 验证命令与输出原文

```
$ cargo check 2>&1 | grep -cE "^(warning|error)"
0

$ cargo check 2>&1 | tail -3
   Compiling aicross v0.1.0 (D:\work\aicross-p5b)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.98s

$ cargo test 2>&1 | grep "test result"
test result: ok. 189 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 4.29s

$ cd src-ui && npx vitest run 2>&1 | grep -E "Test Files|Tests "
 Test Files  13 passed (13)
      Tests  160 passed (160)

$ cd src-ui && npm run build 2>&1 | grep -E "built in|error|Error"
✓ built in 2.17s
（chunk > 500 kB 警告仍在，是既有项，归 C 节）
```

新增 Rust 测试逐条跑过（`cargo test <测试名>` 各 1 passed）：`fallback_skipped_for_writable_node_returns_primary_failure`、`fallback_reruns_for_blind_node_and_marks_no_skip`、`fallback_record_uses_fallback_billing_and_vendor`、`fallback_agent_blocks_same_agent_node_from_running_concurrently`、`premises_attachments_and_last_graph_id_roundtrip`、`premises_legacy_format_without_new_fields_parses_empty`、`arena_role_defaults_stay_empty`。TS 新增 `presets-05b-r1.test.ts` 25 条全绿，`presets-05b.test.ts` 11 条全绿。

未起窗口、未发真机请求、未 commit、未 push。`src/channels/` 未动。

## 修正轮 R2

依据 `tasks/fix-05b-r2.md`（DeepSeek `tasks/review-05b-r1-ds.md` 与 GLM `tasks/review-05b-r1-glm.md` 对 R1 的复审尾巴，12 条）。逐条如下；测试文件：Rust 在 `src/dispatch.rs` 的 `#[cfg(test)]`，TS 新增 `src-ui/src/graph/__tests__/presets-05b-r2.test.tsx`（12 条）。未动 `src/channels/`，未跑 `cargo fmt`，文件原有换行保持不变。

1. **备选重跑后的 session 归属 —— 已修**：`run_fallback` 在备选作答后把结果的 `session_id` 置空（`src/dispatch.rs:1302`；该 session 属于备选通道，节点 target 仍是主目标，写回会让下游同目标「共享」子节点把它续到主通道；留痕里保留真实 session 仅供查账）。「已切备选」标记行（`fallback_from`/`fallback_to` → `applyDispatchResults` 首行标记）照旧进结果与 Reply，等价于任务书说的在结果里记「已切备选，不续聊」。TS 侧双保险：`continuationSession` 对首行带「已切备选」标记的父回复直接返回空（`src-ui/src/graph/dispatch.ts:129`），兼容本修正前已落盘、Reply 里留着备选 session 的旧图。已补：`R2-1`（presets-05b-r2.test.tsx，2 条：带标记父回复返回空、普通父回复照常续 session）；Rust 断言加在 `fallback_reruns_once_and_records_both_attempts`（`src/dispatch.rs:2867`，备选作答结果不带 session_id）。〔离线钉的是「备选结果 session 置空」这条契约：离线造不出带 session 的成功应答，真机形态留 `live_*` 系列。〕
2. **前提附件改文件选择器 —— 已修**：设置页每条前提加「选择文件」按钮，走 `@tauri-apps/plugin-dialog` 的 `open({multiple:true})`（`src-ui/src/components/common/SettingsModal.tsx:279` `pickPremiseAttachments`，与 `CenterPanel.tsx:1143` 既有用法同款；`dialog:default` 权限已在 `capabilities/default.json:30`），选出的绝对路径与已有附件合并去重。手输入口保留，保存前校验抽成纯函数 `validatePremiseAttachments`（`src-ui/src/graph/premise.ts:60`）：相对路径拒绝并提示「附件需用绝对路径：…」、同名 basename 提示「附件文件名重复：…」（与引擎盲分支的拒绝文案一致），校验不过不落盘、走 `setPremiseNote` 提示（`SettingsModal.tsx:266-272` 手输、`:290-294` 选择器两路同规则）。已补：`R2-2`（3 条：相对路径拒绝、同名 basename 提示、三类绝对路径通过）。
3. **主/备原始输出转存互相覆盖 —— 已修**：`record_stdout` 加 `attempt` 参数，备选尝试写 `<node_id>.fallback.raw.txt`，主尝试仍写 `<node_id>.raw.txt`（`src/dispatch.rs:1164`，调用点 `:1575`）。已补：`record_stdout_keeps_primary_and_fallback_dumps_separate`（`src/dispatch.rs:3160`，两条转存路径不同、内容各自完整）；既有 `r3_7` 用例同步改用新签名并覆盖转存失败分支（`:2133-2151`）。
4. **备选留痕的 vendor 不猜 —— 已修**：`FallbackSpec` 追加 `vendor`（`src/dispatch.rs:113-115`，`#[serde(default)]`；TS 镜像 `src-ui/src/tauri.ts:437`）；前端 `fallbackSpecOf` 加 `models` 参数，按 provider 精确匹配 → 退 agent+id 取真实 vendor 下发（`src-ui/src/graph/dispatch.ts:197`，调用点 `DispatchOverlay.tsx:140` 传 models）；Rust 优先用下发值，空才 `infer_vendor`（`src/dispatch.rs:1280-1289`）。已补：`fallback_record_prefers_dispatched_vendor_over_inference`（`src/dispatch.rs:3117`，`k3-256k` 推断不出厂商，留痕用下发的 Moonshot AI；主留痕不受影响）、`R2-4`（2 条 TS：vendor 精确匹配、查不到置空走推断兜底）。
5. **组件级断言补三处 —— 已修**：`react-dom/server` 在依赖里，直接用 `renderToStaticMarkup`。三处抽成组件 `src-ui/src/components/center/graph/DispatchBits.tsx`：`WritableFallbackHint`（G1「可写节点不自动切备选」提示，`DispatchOverlay.tsx:387`）、`FallbackSkippedLine`（「主失败，未切备选（可写节点）」结果行，`:510`）、`PickOneButton`（「选它」按钮可见性，`:522`），DispatchOverlay 原 JSX 条件改由这三个组件渲染。已补：`R2-5`（3 条：提示渲染/只读不渲染、结果行渲染/缺省不渲染、按钮可见性与「已选中」禁用态）。
6. **live 测试的同义反复断言 —— 已修**：`live_arena_and_arbitration` 不再断言测试 helper 写死的值——两边厂商从擂台结果的留痕 frontmatter 读回（`record_field`，`src/dispatch.rs:3721`），仲裁留痕读回 `role: 仲裁` / `write: 只读` / vendor 与两边互异再断言（`:3790-3794`）；`write` 是 R2 给 `DispatchRecord` 追加的字段（`src/dispatch.rs:279`，盲节点强制只读后的生效值），非 helper 输入。中性描述改用与产品 `disputeExcerpt` 同口径的 `dispute_excerpt`（折叠空白、200 字截断加省略号；`src/dispatch.rs:3221`），替换原 `take(400)`。保持 `#[ignore]`，本轮未真跑。
7. **仲裁只读进 Rust 强制 —— 已修**：只读门禁的角色集合加「仲裁」（`src/dispatch.rs:432-439`，与前端 `validate.ts:26` 同口径），错误文案按角色输出（「节点 x 的『仲裁』角色写权限必须为只读」；验收/验证的旧文案无测试引用，一并换成同句式）。已补：`arbitration_role_is_forced_readonly_by_engine`（`src/dispatch.rs:3177`，仲裁+单写者被门禁拒；仲裁+只读过门禁、死在后面的 missing agent 上）。
8. **仲裁标题文案 —— 已修**：编排区标题改「第三方仲裁 · 第 N 阶段」（N = `group.stage` 阶段序号；`DispatchOverlay.tsx:565`），说明行「系统不判定分歧，双方由用户选」不变。原 `group.title` 在角色命名阶段会显示角色名（如「第三方仲裁 · 执行」），与报告口径字面不符。
9. **报告降级表述 —— 已修（口径）**：R1 节三处按实际断言改写，均以〔R2 第 9 条降级〕注记落在原条目内——第 1 条「备选重跑成功」→ 离线只能断言「重跑发生」（双留痕 + 路由字段）；第 7 条「不含立场词」→ 词表断言只对短 fixture 成立，模板层保证不写立场词、内容层无此保证；第 11 条「G1 把 fallback 下发到请求」→ 只有 `fallbackSpecOf` 纯函数级，`buildRequest` 接线点无组件级断言。
10. **「选它」可见性按 kind/role 收窄 —— 已修**：`sameParentsResultCount` 在同父之外追加 `kind`、`target.role` 一致（`src-ui/src/graph/dispatch.ts:398-411`），与 `pickOne` 的候选口径完全对齐。已补：`R2-10`（同父 1 擂台 + 1 顾问，两侧 count 都是 1 → 不出「选它」）；既有 `R1-14` 用例（同父同角色）不受影响。
11. **`fallbackSkipped` 进留痕 frontmatter —— 已修**：`DispatchRecord` 追加 `fallback_skipped`（`src/dispatch.rs:282`，`#[serde(default)]`，旧记录解析不受影响）；`run_one` 把「可写节点」预先交给主尝试（`skip_reason`，`src/dispatch.rs:1224-1228`），主失败且状态属备选触发集合（error/timeout/identity_mismatch；validation_failed 不触发备选不标）时由 `run_one_attempt` 写进结果与该条留痕 frontmatter（`src/dispatch.rs:1533-1539`），不再由 `run_one` 事后补结果字段。Reply 不加字段。已补：`legacy_frontmatter_without_r2_fields_parses_with_empty_defaults`（`src/dispatch.rs:2355`，去掉 attempt/write/fallback_skipped 三行的旧格式 frontmatter 照常往返、新字段取空）；既有 `fallback_skipped_for_writable_node_returns_primary_failure` 扩展断言落盘留痕带 `fallback_skipped: 可写节点`（`:2955-2963`），`fallback_reruns_for_blind_node_and_marks_no_skip` 反断言切了备选不标（`:3000`）。
12. **G1 备选 onChange 只写 fallback —— 已修**：新纯函数 `updateNodeFallback`（`src-ui/src/graph/mutate.ts:69`，只 patch 图上节点的 `target.fallback`）；G1 备选下拉的即时写回改用它（`DispatchOverlay.tsx:431`），本地改过但未确认的主目标不再随备选落盘（本地编辑仍只在 `targets` state 里，确认派发时才生效）。已补：`R2-12`（写备选后图上主目标保持 codex/gpt 不变、清空备选也不动主目标）。

### R2 验证命令与输出原文

```
$ cargo check 2>&1 | grep -cE "^(warning|error)"
0

$ cargo check 2>&1 | tail -2
   Compiling aicross v0.1.0 (D:\work\aicross-p5b)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.97s

$ touch src/dispatch.rs && cargo test --no-run 2>&1 | grep -cE "^warning|^error"
0   （测试目标同样零 warning）

$ cargo test 2>&1 | grep "test result"
test result: ok. 193 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 4.30s

$ cd src-ui && npx vitest run --configLoader native --pool threads 2>&1 | grep -E "Test Files|Tests "
 Test Files  14 passed (14)
      Tests  172 passed (172)

$ cd src-ui && npm run build 2>&1 | grep -E "built in|error|Error"
✓ built in 2.28s
（chunk > 500 kB 警告仍在，是既有项，归 C 节）
```

新增 Rust 测试逐条跑过（`cargo test <测试名>` 各 1 passed）：`fallback_record_prefers_dispatched_vendor_over_inference`、`record_stdout_keeps_primary_and_fallback_dumps_separate`、`arbitration_role_is_forced_readonly_by_engine`、`legacy_frontmatter_without_r2_fields_parses_with_empty_defaults`。TS 新增 `presets-05b-r2.test.tsx` 12 条全绿；R1 的 `R1-6` 期望同步补了 `vendor` 键（`fallbackSpecOf` 返回 shape 变化），其余既有用例未动。

未起窗口、未发真机请求、未 commit、未 push。`src/channels/` 未动。`live_arena_and_arbitration` 保持 `#[ignore]` 本轮未真跑（编排者已在此前真跑过 R1 版；R2 改了断言口径，真机复跑与否由编排者定）。
