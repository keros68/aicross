# REPORT-12 修正轮上限、验收与盲审并行、审查材料附带验证输出

分支 `phase/12`（基于 main dcaf24b），未 commit、未 push。改动 24 个文件（含 1 个新测试文件）；`src-ui/package-lock.json` 的改动在开工前就已存在（前端依赖安装所致），本期未动。

## 契约（只追加）

| 契约 | 改法 | 测试 |
|---|---|---|
| `DispatchSettings.fix_round_limit: usize`（serde default 1） | `src/dispatch.rs` 顶部结构体 + `Default` + `default_fix_round_limit()`；`src-ui/src/tauri.ts` `DispatchSettings.fixRoundLimit: number`（字节级追加两行，CRLF/LF 计数 576/105，HEAD 576/101）；`dispatch-settings.ts` 默认 1、读取夹到 0–9；`DispatchOverlay.tsx` 与 `swarm.ts` 的 wire settings 一并下发 | Rust `task12_settings_fix_round_limit_defaults_to_one_and_reads_camel_case`；TS `dispatch-settings.test.ts`「修正轮上限默认 1，可保存并恢复，越界夹紧」；`swarm-08c` 08C-5 期望值加了 `fixRoundLimit: 1` |
| `Node.leftover?: { source_node_id, items }` | `src-ui/src/graph/types.ts` `Leftover` + `GraphNode.leftover?`；`src/orchestration.rs` `Leftover` 结构体 + `GraphNode.leftover: Option<Leftover>`（serde default），3 处测试字面量补 `leftover: None` | Rust `task12_graph_node_leftover_roundtrip_and_default_none`；TS `review-defaults-12.test.ts`「遗留节点不在可派发集合里」 |
| `DispatchNodeRequest.review_material_validation: bool`（default false） | `src/dispatch.rs` 结构体字段；`tauri.ts` `reviewMaterialValidation?: boolean`；`buildNodeRequest` 对「验证 + 盲」的目标带 `reviewMaterialValidation: true`（`reviewMaterialValidationOf`），验收 / 共享验证 / 顾问不带 | Rust `task12_request_reads_review_material_validation_camel_case_default_false`；TS「盲的验证节点请求带 reviewMaterialValidation: true，验收与共享验证不带」 |
| 派发记录 frontmatter `validation_attached: true\|false` | `DispatchRecord.validation_attached`（serde default），`run_one_attempt` 记录字面量填一行 | Rust `task12_record_frontmatter_carries_validation_attached` |
| `fixtures/prompts/acceptance.txt` / `blind-prefix.txt` 末尾两条固定关注点 | 各追加两行「固定关注点：契约（字段、命令签名、序列化大小写）有没有变。」「固定关注点：新增的测试能不能红：把修复回退后该测试是否会失败。」；前端仍由 `?raw` 读文件，Rust 测试 `include_str!` 读文件，代码里没有复制文本 | Rust `task12_review_prompt_files_end_with_fixed_focus_points`；TS「验收提示词与盲审前缀末尾各带两条固定关注点」 |

## 实现

### 1. 修正轮上限 + 遗留节点（`src-ui/src/graph/dispatch.ts`）

- `reducePipelineStep(graph, results, validationRetries, fixRoundLimit = DEFAULT_FIX_ROUND_LIMIT)`：原来写死的 `count >= 3`（验证命令未通过的重派）与 `acceptanceRound > 3`（验收 NEEDS_FIX 的修正轮）都改读 `fixRoundLimit`。到顶后：验收 NEEDS_FIX → `appendLeftover` 在该验收节点下挂遗留节点，不再派修正；验证未通过 → 沿用既有行为（等待中的验收标「已跳过」）。`runStageLoop` 从 settings 取 `fixRoundLimit` 传入。
- `appendValidationRetry` 加 `limit` 参数，修正节点文案 `[修正 N/L]`；`routeLine` / `routeReason` 的正则从 `/3` 改成 `/(\d+)`。
- `leftoverItems(text)`：按行取「- 」「* 」「1.」「2)」「3、」「（4）」开头的行；一条都没有就整段按行截前 20 行（`LEFTOVER_LINE_LIMIT`）。
- `appendLeftover`：`kind: 'turn'`、无 target 无 reply、`user.text = 遗留：N 条未闭合，来源 验收（agent/model）`（`leftoverLine`）。`dispatchableNodeIds` 一开头就排除 `node.leftover`；`nodeStatusLabel` 对遗留节点返回 null；`nodeLabel` 返回「遗留」。
- 界面：`GraphTab.tsx` 节点卡顶行显示「遗留」、第二行胶囊「N 条未闭合」，聊天列里不显示回复槽、把清单按行列出（`.graph-leftover`）；`NodePanel.tsx` 新增 `LeftoverList`（只读清单 + 「复制清单」按钮 + 「已复制」），不显示 TargetEditor；`DispatchOverlay.tsx` 结果屏在「流水线结果」里对本轮验收生成的遗留节点各出一张「遗留」卡（`leftoverNodes` 过滤 `source_node_id` 在本轮结果里的）。
- 设置页 `SettingsModal.tsx`「派发」栏加数字框「修正轮上限」（0–9），说明句「到上限后未闭合的条目生成遗留节点，不再派修正」。

测试（`src-ui/src/graph/__tests__/review-defaults-12.test.ts`）：「上限 1：第一轮 NEEDS_FIX 派修正，第二轮 NEEDS_FIX 只生成遗留节点」「上限 3：第二轮还派修正，第四轮才生成遗留节点」「上限 0」「PASS 不生成遗留节点」「遗留节点不在可派发集合里，节点卡没有派发状态」「验证命令未通过的重派同样读上限」「runStageLoop 把设置里的上限传给推进函数」「leftoverItems」「appendLeftover 空正文不生成」。`dispatch.test.ts` 里三条旧测试改为显式传上限 3（R5-2、验证重试上限、验收修正上限；后者现在断言只多一个遗留节点）。

### 2. 验收与盲审同阶段并行

- `src/orchestration.rs` `builtin_presets()`：经济模式第三段改为两路 `[验收, 验证]`（都盲、只读）；盲审模式改为一段三路 `[验收, 验证, 验证]`（总请求数不变，对应 Runbook「复跑 ∥ 两家盲审」；若要保留三路验证改成四路，只需改这一处的数组、`PRESET_NODE_TEXTS['builtin-blind']` 与两处说明文字）。
- 角色默认：`fill_role_defaults`（Rust）与 `applyRoleDefaults`（TS）把「验证」也按 `roles.acceptance` 填，否则经济模式第三段会因验证那一路没目标而在 `missing-target` 处停下；跳过名单从 `builtin-arena` 扩到 `builtin-arena | builtin-blind | builtin-jury`（多路盲验证必须由用户逐家选不同厂商），两侧一致（`ROLE_DEFAULT_SKIPPED_PRESETS`）。
- `stamp.ts` `PRESET_NODE_TEXTS` 加 `builtin-economy: ['', '', '', VERIFICATION_ROUTE_TEXT]`、`builtin-blind: ['', 文本, 文本]`；文本「盲审：独立审查同一份产物，只看材料与验证命令输出，不看验收结论。」
- `appendCorrectionRound`：修正轮里与验收同父的「验证」兄弟节点一起长出来（挂在新执行节点下，沿用其 user.text），修正轮的产物同样被盲审。
- 验证门只看「验收」：`reducePipelineStep` 的 NEEDS_FIX 分支本来就只过滤 `role === '验收'`，验证那一路的正文不触发修正轮（测试「验证门只看验收」）。
- 讲解页 `ExplainOverlay.tsx` 经济模式 / 盲审模式各补一句；`docs/MANUAL.md` 预设说明与术语表（遗留 / 修正轮上限 / VALIDATION.txt）。

测试：Rust `task12_economy_acceptance_stage_has_acceptance_and_verification_routes`、`task12_blind_preset_has_one_acceptance_and_two_verification_routes`、`task12_fill_role_defaults_fills_economy_verification_from_acceptance_but_not_blind_preset`；TS「引擎侧内置预设：经济模式第三段是「验收 + 验证」…」（读 `orchestration.rs?raw` 核对形状）、「经济模式盖章：第三段两路同父…」、「盲审模式盖章…」、「修正轮：验收那一路与同父的验证那一路一起长出来」、「角色默认：验证沿用验收的默认目标；盲审 / 评审预设整体不填」。

### 3. 审查材料附带验证输出（`src/dispatch.rs`）

新函数三个 + `run_one_attempt` 内两处最小改动（一行调用 + 记录字面量一行），不动备选循环：

- `review_material_dir(project, node)`：上游产物带 worktree 且目录还在就用它，否则项目目录。
- `review_validation_text(command, outcome)`：`验证命令：<cmd>` + 「验证命令通过（exit 0）\n<输出>」或 `run_validation` 的错误文本（含 `验证命令失败（exit=9）`）；命令为空只写一行「未配置验证命令」。先 `safe_trace`（strip ANSI + 脱敏，不受外发脱敏开关影响）再按 8 KB 截断（落在字符边界，末尾注明「…（输出超过 8 KB 已截断）」）。
- `attach_review_validation(project, settings, node, workdir, blind, timeout, prepared)`：只在 `blind && review_material_validation` 且 `prepare_outbound` 成功后生效；超时取节点超时的一半；写 `VALIDATION.txt` 到盲目录，材料名列表加 `VALIDATION.txt`，prompt 末尾追加「验证命令输出见 VALIDATION.txt」；返回 `(prepared, validation_attached)`。备选重跑会清盲目录再走同一条路，文件随之重建。
- 调用点：`run_one_attempt` 里 `prepare_outbound` 之后一行；`DispatchRecord.validation_attached` 一行。

测试（真跑命令，Windows 走 `powershell.exe -NoProfile -Command`）：`task12_validation_txt_materialized_for_exit_zero_command`（`echo review-ok`）、`task12_validation_txt_materialized_for_exit_nine_command_and_still_attached`（`echo boom; exit 9`）、`task12_validation_txt_says_unset_when_command_empty`、`task12_validation_txt_not_written_when_flag_off_or_not_blind`、`task12_validation_text_truncates_at_8kb_on_char_boundary`、`task12_validation_text_is_redacted`、`task12_review_material_dir_prefers_existing_upstream_worktree`。

## 「能不能红」自验

把修复暂时回退再跑一次（脚本自动回退并恢复，恢复后文件字节一致）：

- TS 回退四处（验收上限改回 `> 3`、去掉 `dispatchableNodeIds` 的 leftover 排除、去掉 `reviewMaterialValidation` 装配、验证重派改回 `>= 3`）→ `review-defaults-12.test.ts` 6 条红：上限 1、上限 0、遗留节点不在可派发集合里、验证命令未通过的重派同样读上限、runStageLoop 传上限、reviewMaterialValidation。
- Rust 回退（`attach_review_validation` 直接返回、去掉脱敏与截断）+ 删掉两份提示词末尾的关注点 → `cargo test task12` 6 条红：exit 0、exit 9、未配置验证命令、截断、脱敏、提示词关注点。
- 只删提示词关注点 → TS「验收提示词与盲审前缀末尾各带两条固定关注点」红。

## 验证结果原文

```
$ cargo check --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.24s
warnings: 0

$ cargo test
test result: ok. 361 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 15.89s

$ cd src-ui && npx vitest run --configLoader native --pool threads
 Test Files  30 passed (30)
      Tests  451 passed (451)

$ cd src-ui && npm run build
✓ 334 modules transformed.
✓ built in 674ms
```

换行核对（python 数 CRLF/LF 与 HEAD 比对）：`src/server.rs` 未动；`src/dispatch.rs`、`src/orchestration.rs` 与全部 TS/TSX/CSS/MD 仍为纯 LF；`src-ui/src/tauri.ts` CRLF 576 / LF 105（HEAD 576 / 101，只多了追加的 4 行 LF）。未 `cargo fmt`，未起窗口，未发真机请求。

## 没做 / 判断

- 盲审模式改成「验收 + 两路验证」而不是「验收 + 三路验证」：总请求数与原来一致（3），对应 Runbook 的「复跑 ∥ 两家盲审」；编排者若要保留三路验证，改 `builtin_presets()` 一个数组、`PRESET_NODE_TEXTS['builtin-blind']` 与讲解页 / 手册各一句即可。
- 验证命令未通过到顶时仍沿用旧行为（验收标「已跳过」），没有生成遗留节点：任务书的遗留清单来源是验收结果，验证未通过没有可拆的条目。
- `reviewMaterialValidation` 由前端按「验证 + 盲」角色决定，不新增 Target 字段（契约只追加、任务书没列）；评审模式的三个盲验证节点因此也会附 `VALIDATION.txt`（命令为空时只有一行「未配置验证命令」）。
- 遗留节点的清单展示只做了纯函数测试；节点面板与结果屏的 DOM 没有测试（面板走 `createPortal`，现有测试框架用 `renderToStaticMarkup` 渲染不了）。窗口核对项：设置页数字框、经济模式盖章后第三段两路并排、修正轮到顶后图上出现「遗留」卡与面板「复制清单」、结果屏「遗留」卡、`.dispatch/` 记录里 `validation_attached: true` 与盲目录里的 `VALIDATION.txt`。
- 真机（发模型请求的经济流水线）未跑。
