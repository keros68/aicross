# REPORT-48 — 资源库里「未发现」的模型

按 `tasks/TASK-48-missing-models.md` 实现。工作树 `D:\work\aicross-wt\t46`，在 TASK-46 的未提交改动之上继续，未 commit、未 push。

## 改了什么

### src/resources.rs（状态、标记读取、探测时间、批量冒烟跳过）

- 新增 `RESOURCE_STATE_NOT_DETECTED`（「未发现」）与统一说明 `NOT_DETECTED_NOTE`（「最近一次探测没有发现这个模型；在对应 agent 里确认登录后重新探测」）。
- 新增 `is_missing_from_detection(model)`：只读 `meta` 里的 `MISSING_MARKER`（按「；」分段 trim 比对，与 `append_note_marker` 同口径），不改 `merge_detected_models` 的合并规则。
- `model_state` 优先级插入「未发现」：在「未安装」之后、额度用尽 / 原生 / 未登录 / 验证结果之前；原生模型同样适用。用户申报的模型本来就不打标记，不受影响。
- `ResourceStore` 追加 `detected_at`（落盘键名 `detectedAt`，`skip_serializing_if` 空时不写出；旧文件没有该字段时读出 `None`）。`detect_all_resources_at` 与测试用的单 agent 探测入口在成功写回前盖 RFC3339 时间戳。
- `ResourceOperation` 追加 `skipped: Vec<String>`（serde default，不改变既有字段）。
- `smoke_agent_models_filtered_at` 加 `skip_reason` 参数：被排除条目里 `skip_reason` 给得出说明的汇总进 `skipped`，不算失败、不进 `errors`、不影响退出码；`smoke_agent_third_party_at` 的过滤改为「非原生且未带标记」，未发现的第三方模型不发请求，逐个给出「跳过 <agent>/<model>：<note>」。

### src/console_cli.rs（冒烟、选人选、展示、detected_at、stderr 说明）

- `resources_smoke_at` 单模型路径：解析出目标后先查状态，「未发现」时不调 `smoke_one`，退出 1，stderr 写 `NOT_DETECTED_NOTE`；判在「原生无需冒烟」之前，原生模型带标记同样退出 1。命令行与窗口 `resources_smoke` 共用这一份。
- 按 agent 冒烟路径：`operation.skipped` 逐行进 stderr，排在失败说明之前；退出码仍只看失败数。
- `resolve_resource_model`：人选是「未发现」时返回退出码 2，原因「<agent>/<model>：<note>；该 agent 下可选：…」，候选列表用新增的 `model_candidates_filtered` 去掉未发现的模型。`set-role` 与 `set-seat` 的主目标和备选都走这里，一处拦截两处生效。
- `modes_section` / `mode_entry` / `seat_entry_after_write`：加载资源库（读不了按 `None` 处理，不让 show 失败），`effective` 追加 `state` 字段——新增 `resource_state_of` 按 provider 精确 → agent+id 两级查找（与引擎 `find_model_two_level` 同口径），找到就给 `model_state` 的状态，找不到给「不在资源库」。
- `resources_value_filtered` 顶层输出追加 `detected_at`（`None` 序列化为 null），窗口 `config_snapshot` 的资源段同构受益。
- `aicross dispatch` 的 Ok 分支：响应里的 `notes` 逐行写 stderr（引擎已接收派发那行之后）。

### src/engine/mod.rs（派发前拦截与备选丢弃）

- `plan_dispatch` 在「目标验证失败」检查之前加「未发现」拦截（同一处 `strict` 查找，不另写一套）：目标模型带标记时拒绝，原因「<agent>/<model> 在最近一次探测中未发现，请重新探测或换人选」。状态优先级里未发现压过验证结果，所以判在前面。
- 备选处理：主目标通过后，逐节点检查 `Target.fallback`，备选模型带标记（provider 精确 → agent+id 两级查找，与 `build_request` 的备选 vendor 同口径）时把该备选置 `None`（视为未设备选），说明「备选 <agent>/<model> 在最近一次探测中未发现，本次不作为备选」收进 `notes`。备选在资源库里压根没有时维持原行为。
- `Plan::Ready` 加 `notes` 字段；`EngineRun` 加 `notes: Vec<String>`（serde default，空时不写出）；接受的响应 JSON 加 `notes` 数组。即 run 文件与 stderr 都留说明。

### src/engine/golden.rs、src/engine/tests.rs、src/run_view.rs

- `Plan::Ready` 的两处解构补 `notes`（金标准忽略该字段）；`run_view` 测试的 `EngineRun` 字面量补 `notes: Vec::new()`。

### docs/CONSOLE-CHANNEL.md

- 「资源与配置」：`detected_at` 的含义与更新时机；「未发现」状态的含义（meta 带「未在事实源中」标记、原生同样适用）与冒烟对它的处理（单模型退出 1、按 agent 跳过并逐行说明、不算失败）；`set-role` / `set-seat` 对「未发现」退出 2 并在候选列表里去掉它们；`config show` 的 `effective.state`（含「不在资源库」）。
- 「派发」：首个响应示例加 `notes`；派发前拦截补「未发现」拒绝原因与备选丢弃的说明去向；「状态、采纳与取消」的 run 摘要字段清单补 `notes`。

## 复用的现成函数

- `resources::merge_detected_models` 打的 `MISSING_MARKER`（只读，合并规则零改动）、`model_state`、`is_native_model`、`get_resources_at`、`save_store`。
- 引擎里验证失败拦截用的同一个 `strict` 查找闭包；`build_request` 的备选两级查找口径。
- console_cli 的 `ResourcesOps` 注入边界（`smoke_one` / `smoke_agent_third_party` / `read_resources`）、`resources_smoke_at`、`resolve_resource_model`、`model_candidates`、`modes_section` / `mode_entry` / `config_show_value`、`resources_value_filtered`。
- `fill_seats`（TASK-46 的补人纯函数，`effective` 直接用它的结果，不在展示层另写一份）。

## 新增测试

| 文件 | 测试名 | 覆盖 |
|---|---|---|
| src/resources.rs | `task48_model_state_reports_not_detected` | 带标记的第三方 / 原生（原为 verified）/ 原为已验证的模型都「未发现」；agent 不可用仍「未安装」；去掉标记恢复原状态 |
| src/resources.rs | `task48_merge_marks_missing_then_recovers` | 探测 A、B → 只剩 A 时 B「未发现」→ 再探测到 B 恢复且标记消失 |
| src/resources.rs | `task48_smoke_agent_skips_missing_models` | 批量冒烟只发未带标记的模型（冒烟闭包注入），`skipped` 有逐条说明、`errors` 为空 |
| src/resources.rs | `task48_detect_updates_detected_at_and_old_files_read_none` | 探测写回 `detectedAt`（RFC3339，落盘键名核对）；旧文件无该字段读出 `None` |
| src/console_cli.rs | `task48_resources_lists_missing_state_and_detected_at` | `aicross resources` 输出「未发现」状态与 note；`detected_at` 有值输出、无值为 null |
| src/console_cli.rs | `task48_smoke_single_missing_exits_one_without_request` | 单模型「未发现」退出 1、不发请求（冒烟闭包注入 panic）、stderr 写 note；原生带标记同样退出 1 |
| src/console_cli.rs | `task48_smoke_agent_reports_skipped_missing` | 按 agent 冒烟跳过说明逐行进 stderr，不算失败、退出 0 |
| src/console_cli.rs | `task48_set_role_rejects_missing_model_and_lists_candidates` | set-role 指向「未发现」退出 2，候选列表去掉它，不写 roles.json |
| src/console_cli.rs | `task48_set_seat_rejects_missing_target_and_fallback` | set-seat 主目标或备选「未发现」都退出 2，不写 modes.json |
| src/console_cli.rs | `task48_config_show_effective_state` | `effective.state`：未发现 / 不在资源库 / 已验证 |
| src/engine/tests.rs | `task48_dispatch_rejects_missing_target_with_reason` | 目标「未发现」被拒，原因逐字核对 |
| src/engine/tests.rs | `task48_dispatch_drops_missing_fallback_but_keeps_normal_one` | 备选「未发现」丢掉并留 notes，主目标照常；正常备选保留、notes 为空 |
| src/engine/tests.rs | `task48_start_dispatch_writes_fallback_note_to_run_file` | 端到端：run 文件与响应都带 notes，落图的备选已丢掉 |

## 验证结果

在本工作树根目录执行（`CARGO_TARGET_DIR=D:/work/aicross/target`）：

- `cargo check --all-targets` — 通过，零 warning。
- `cargo check --release` — 通过，零 warning。
- `cargo test` — `496 passed; 0 failed; 10 ignored`（TASK-46 基线 483 + 新增 13）。

换行保持：本任务改动的文件全部为 LF，未动 `src/server.rs`、`src/main.rs`（CRLF）。

## 未做与理由

- 未改前端（`src-ui/**`）：任务书明确不改。窗口展示「未发现」状态、`detected_at`、失效人选提示所需的输出字段（`state` / `note` / `detected_at` / `effective.state`）已随共用实现体就绪。
- 未改 `merge_detected_models` 的合并规则：任务书约束只读它打的标记；重新探测发现后整条替换、标记随 meta 自然消失，由 `task48_merge_marks_missing_then_recovers` 固定。
- 未给「资源库读不了」时的 `config show` 单独测试：`resource_store_for_state` 读不了按 `None` →「不在资源库」，与「模型不在库里」同一条展示路径，后者已测；宽松读取的分法沿用 `modes_from` 的既有模式。
- 未给「按 agent 冒烟只含未发现模型」单独断言退出码文案：跳过不算失败时最终行仍是「冒烟完成」（`errors` 为空的既有分支），已在 `task48_smoke_agent_reports_skipped_missing` 里覆盖「无『条失败』」。
- 未动 `detect_resource_agent_at_with_pi_binary` 的生产行为定位：它只服务测试（TASK-17 恢复用），让探测时间戳盖在两个探测入口是保持「探测成功写回即更新」同一口径，不引入新函数。

## 修正轮 r1

按 `tasks/fix-48-r1.md` 逐条修改，工作树不变，未 commit、未 push。只改了 `src/resources.rs`、`src/engine/mod.rs`、`src/engine/tests.rs`、`src/console_cli.rs`、`docs/CONSOLE-CHANNEL.md`。

### 逐条改法

1. **主目标查找不看 provider（R1）**：`resources.rs` 新增共用查找 `find_target_model(models, agent, provider, id)`——provider 非空时只按 agent + provider + id 精确匹配，provider 为空时按 agent + id 取第一条。「未发现」的主目标检查（原用 `strict`）、备选检查（原手写两级查找）、`resource_state_of`（原手写两级查找）都改用它，三处手写查找删除；`resource_state_of` 在给了 provider 却没精确匹配时落到「不在资源库」，不再退回 agent+id。`strict` 只服务不在本期范围的「验证失败」「额度用尽」检查，`find_model_two_level` 只服务额度用尽的恢复时间查找，原样不动。
2. **只检查第一阶段（R2）**：「未发现」的主目标拒绝与备选丢弃从遍历 `first` 改为遍历本次 `created` 里所有带 target 的节点；角色无目标、未接入、验证失败、额度用尽等原有检查仍只看第一阶段，未动。
3. **全部跳过时冒烟输出空资源库（R3）**：`smoke_agent_models_filtered_at` 的返回值 `latest` 改用第一次加锁读到的 store 初始化；全部条目被跳过（可冒烟清单为空）时循环体不执行，返回的仍是读到的那个库，不再落回 `ResourceStore::default()`。
4. **生产入口写 `detectedAt` 没有测试（R4）**：盖时间戳抽成 `resources::stamp_detected_at`，生产 `detect_all_resources_at` 与测试用的单 agent 探测入口都调用它；新增 console_cli 测试用注入的 `detect_all` 走同一份盖章加落盘，断言 `resources detect` 输出的 `detected_at` 非 null 且写回文件里有 `detectedAt`。
5. **按 agent 冒烟的过滤与跳过说明没测到生产代码（R5）**：过滤与跳过说明提成具名函数 `smoke_agent_third_party_filter` / `smoke_agent_third_party_skip_reason`，生产入口 `smoke_agent_third_party_at` 与新增的测试注入入口 `smoke_agent_third_party_with`（`#[cfg(test)]`）共用；原抄闭包的测试改走注入入口，另补一条全跳过的测试。
6. **丢掉备选写回了图（R6）**：说明文字改为「备选 <agent>/<model> 在最近一次探测中未发现，已从该节点移除」，如实说明备选已写进本次 run 的图；文档同步（派发段与 run 摘要字段说明）。行为本身不变。
7. **判法不一致（R7）**：命令行的单模型冒烟与选人选（`resolve_resource_model`）从查状态（`model_state` == 未发现，agent 未安装时会先返回「未安装」）改为直接读标记 `is_missing_from_detection`，与引擎一致；文档写明「未发现」按最近一次探测的标记判断，agent 未安装、验证失败都不改变判定。
8. **文档 `notes` 描述不一致（R8）**：按实现改：响应总有 `notes` 字段（空时 `[]`），run 文件的 `notes` 为空时不写出，两处分开写。

### 新增测试

| 文件 | 测试名 | 覆盖 |
|---|---|---|
| src/engine/tests.rs | `task48_dispatch_target_lookup_respects_provider` | 同一 id 挂两个 provider、只有 deepseek 那条带标记：目标选正常 provider 能派出、选带标记的被拒；备选同口径（带标记丢弃并留说明、正常保留） |
| src/engine/tests.rs | `task48_dispatch_rejects_missing_target_in_later_stage` | 第二阶段人选未发现，派发前即被拒，原因逐字核对 |
| src/engine/tests.rs | `task48_dispatch_drops_missing_fallback_in_later_stage` | 第二阶段备选未发现，从该节点移除并留 notes，主目标照常 |
| src/resources.rs | `task48_smoke_agent_all_missing_keeps_store_entries` | 某 agent 下第三方模型全部未发现：不发请求，返回值仍含该 agent 的完整条目与逐条跳过说明（R3 + R5 的具名过滤函数） |
| src/console_cli.rs | `task48_detect_output_reports_detected_at_after_real_write_back` | 注入的 detect_all 走真实盖章加落盘，输出 `detected_at` 非 null、文件里有 `detectedAt` |

改写的既有测试：`task48_smoke_agent_skips_missing_models`（改走 `smoke_agent_third_party_with`，不再抄闭包）、`task48_config_show_effective_state`（补同名模型双 provider 各认各的、provider 没精确匹配为「不在资源库」）、`task48_dispatch_drops_missing_fallback_but_keeps_normal_one` 与 `task48_start_dispatch_writes_fallback_note_to_run_file`（核对 R6 的新说明文字）。夹具 `task48_plan_with` 拆出 `task48_plan_with_preset` 供多阶段用例共用。

### 验证结果

在本工作树根目录执行（`CARGO_TARGET_DIR=D:/work/aicross/target`）：

- `cargo check --all-targets` — 通过，零 warning。
- `cargo check --release` — 通过，零 warning。
- `cargo test` — `501 passed; 0 failed; 10 ignored`（修正前 496 + 新增 5）。

换行保持：本轮改动的文件全部为 LF；未动 `src/server.rs`、`src/main.rs`（CRLF）。未改 `src-ui/**`，未装依赖，未联网。

## 复审（Claude 子代理，结论「可以合并」）与编排者追加

- 修正清单 8 条中 7 条判定改到位；R4 只做到盖时间戳共用 `stamp_detected_at`，`detect_all_resources_at` 里调用它的那一行仍无测试直接覆盖（Later：抽成「盖章 + 写回」一个函数再测）。
- 编排者直接修复审的另一条低危：`find_target_model` 在没给 provider 时优先取没带「未发现」标记的同名条目，全部带标记才返回带标记的（本机 `pi/deepseek-v4-flash` 同时挂在已撤掉的 `deepseek` 与正常的 `opencode-go` 下）。新增测试 `task48_find_target_model_prefers_detected_entry_without_provider`。
- 编排者验证：`cargo check --all-targets` 零 warning，`cargo test` 502 通过、0 失败、10 忽略。

