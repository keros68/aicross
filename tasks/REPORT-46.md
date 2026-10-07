# REPORT-46 — 模式人选与默认模式（modes.json）

按 `tasks/TASK-46-modes-config.md` 实现。工作树 `D:\work\aicross-wt\t46`，未 commit、未 push。

## 改了什么

### src/orchestration.rs（modes.json 的读写）

- 新增 `SeatChoice`（agent / provider / model / tier / thinking / fallback，camelCase，`fallback` 复用现有 `FallbackTarget`）与 `ModesFile`（version / defaultMode / seats，`seats` 是「预设 id → 阶段数组 → 位置数组」的 `IndexMap`，保持写盘键序）。
- `modes_path` / `modes_from`（宽松读：缺失、损坏、version 不认都按空配置）/ `modes_from_strict`（严格读：缺失回默认，损坏或版本不认报错，调用方拒写）/ `save_modes_in`（version 校验 + `write_json_atomic` 原子写）。分法照 `roles_from` / `roles_from_strict`。

### src/engine/mod.rs（补人纯函数与默认模式）

- 新增 `default_preset_id(modes, presets)`：defaultMode 指向存在的预设时用它，为空或指向不存在的预设时回 `builtin-economy`（原有行为）。
- 新增补人纯函数 `fill_seats(preset, roles, seat_stages) -> (Preset, sources)`，派发与展示共用同一份：
  1. 节点 model 非空（预设写死）→ 不动，来源「预设」；
  2. modes.json 该位置有人选 → 写 agent/provider/model，tier / thinking 非空覆盖，fallback 写进 `Target.fallback`，来源「位置」；
  3. 否则对照 `apply_role_defaults` 的结果套角色默认（整份跳过与角色匹配直接用它的结果，不重写），来源「角色」；
  4. 都没有 → 保持空，来源「未指定」，由派发前检查拒绝。
  seats 与预设长度对不上时多出的忽略、缺的按 null。
- `PlanInput` 加 `modes: &ModesFile` 与 `default_preset: &str` 两个字段（金标准重放与既有测试同改，见下）。
- `plan_dispatch`：`input.preset` 缺省时用 `input.default_preset`；`apply_role_defaults` 调用点换成 `fill_seats`。写死 `builtin-economy` 缺省的两处（wanted 解析与「找不到预设」消息）一起改。
- `start_dispatch_with`：resources / presets / roles 的读取上移，新增 modes 读取；`default_preset_id` 解析实际缺省；run 文件的 `preset_id` 记实际生效的预设 id（未给 `--preset` 时随 defaultMode 走）。第三处写死 `builtin-economy` 的地方（preset_id 缺省）一并改掉。

### src/engine/golden.rs、src/engine/tests.rs

- 两处 `PlanInput` 构造补新字段：金标准按空 modes 配置与经济缺省重放（夹具不变，全部通过）；`task42_plan_with_verification` 同步。

### src/console_cli.rs（命令行）

- 抽出 `resolve_preset_name`：`--preset` 的别名映射（shared / economy / blind）与 `config set-default-mode` / `set-seat` / `clear-seat` 共用同一个。`build_request` 不再在缺省时写死 `economy`，`--preset` 未给时不传，交给引擎按默认模式解析。
- 抽出 `resolve_resource_model`：agent + model（给了 provider 一并匹配）的资源库查找，`set-role` 与 `set-seat` 共用同一份（含三种报错文案与退出码）；`run_config_set_role` 改为调用它，行为不变。
- `config show` 输出追加 `defaultMode`（实际生效的预设 id）与 `modes`（`raw_presets_from` 顺序的全部预设，每项 id / name / builtin / default / stages，每个位置给 seat 原样、effective、source）。实现体抽成 `config_show_value`，窗口命令复用。
- 新增三条命令（`run_config_set_default_mode` / `run_config_set_seat` / `run_config_clear_seat` 是参数解析与输出的薄壳，实现体 `config_set_default_mode_at` / `config_set_seat_at` / `config_clear_seat_at` 返回 `Result<Value, (退出码, 说明)>`，窗口命令直接调）：
  - `set-default-mode <模式>`：别名与 id 都认；不存在退出 2 并列出可选 id（名称）；modes.json 损坏退出 1、不覆盖。
  - `set-seat <模式> <阶段> <位置> …`：阶段位置 1 起数；模式不存在 / 阶段位置超出范围 / `--thinking` 非法 / 备选缺配对 / 人选或备选不在资源库，退出 2 并列出可选；agent 名回写规范 id，provider 取资源库规范值（原生模型空 provider 写 null）；写入后输出该模式那一项。
  - `clear-seat <模式> [<阶段> <位置>]`：给了位置只清这一个（本来就没设时不改盘），没给删掉 seats 里该模式全部键；输出同 `set-seat`。
- `resources smoke` 的实现体重排为 `resources_smoke_at`（返回 stdout JSON + stderr 说明行 + 退出码），CLI 薄壳打印；报错文案与退出码不变，既有测试全部通过。
- `output_resources` 拆出 `resources_value_filtered` / `resources_value`，窗口命令复用同一输出结构。
- 窗口命令实现体：`config_snapshot_at`（`{ resources, config }`，config 段同 `config show`）、`resources_detect_at`、`resources_smoke_command_at`。

### src/server.rs（窗口命令，CRLF 保持）

- 新增 6 个 Tauri 命令并登记：`config_snapshot`、`resources_detect`、`resources_smoke`、`config_set_default_mode`、`config_set_seat`（stage / index 从 1 起，seat 是第 1 节的人选对象）、`config_clear_seat`。返回值与对应命令行 stdout 同构，错误返回 stderr 那行说明（`Result<serde_json::Value, String>`）。全部直接调 console_cli 的实现体，不拉子进程、不重写逻辑。`resources_detect` 与 `resources_smoke` 用 `async` + `tauri::async_runtime::spawn_blocking`，不卡窗口主线程。

### docs/CONSOLE-CHANNEL.md

- 开头的窗口说明改为「窗口承载资源、模式与最近的派发三页」；`--preset` 缺省行为改为按 modes.json 的默认模式；`config show` 输出结构更新；「资源与配置」的命令块补四条新命令；新增「模式人选与默认模式（modes.json）」一节：位置与格式、补人顺序、默认模式、四条命令的输出字段与退出码、宽松与严格读取的分法。

## 复用的现成函数

- `orchestration::write_json_atomic`（modes.json 原子写）、`raw_presets_from` / `builtin_presets`（预设清单与查找）、`roles_from` / `roles_from_strict`（分法镜像）、`FallbackTarget`（seat.fallback 类型）、`PresetFile`。
- `engine::apply_role_defaults`（作为「角色」档的基准结果，规则零改动）、`load_settings`（show 与 detect 的 ccSwitchPath）。
- `resources::get_resources_at` / `detect_all_resources_at` / `smoke_resource_model_at` / `smoke_agent_third_party_at` / `is_native_model` / `VERIFICATION_FAILED`（全部经现有 `ResourcesOps` 进程边界）。
- `channels::acp_setup::adapter_status`（config_snapshot 的 acp 段）；`tauri::async_runtime::spawn_blocking`。

## 新增测试

| 文件 | 测试名 | 覆盖 |
|---|---|---|
| src/orchestration.rs | `task46_modes_roundtrip_keeps_camel_case_and_seat_order` | 写盘读回一致，camelCase 字段名与 fallback 对象 |
| src/orchestration.rs | `task46_modes_lenient_read_defaults_on_missing_broken_or_wrong_version` | 宽松读：缺失 / 损坏 / 错版本回空配置 |
| src/orchestration.rs | `task46_modes_strict_read_rejects_broken_and_wrong_version` | 严格读：缺失回默认，损坏 / 错版本报错；save 校验 version |
| src/engine/tests.rs | `task46_fill_seats_reports_all_four_sources` | 预设 / 位置 / 角色 / 未指定四种来源各一条 |
| src/engine/tests.rs | `task46_fill_seats_overrides_tier_thinking_and_writes_fallback` | tier / thinking 空串不覆盖、非空覆盖；fallback 写入 |
| src/engine/tests.rs | `task46_fill_seats_tolerates_shape_mismatch` | 阶段多出忽略、位置缺的按 null |
| src/engine/tests.rs | `task46_fill_seats_keeps_user_preset_hardwired_target` | 用户预设写死的人选不被覆盖 |
| src/engine/tests.rs | `task46_blind_dispatch_with_seats_is_accepted_and_without_is_rejected` | 盲审有人选能派出（三路按人选、run 记 builtin-blind）；没有人选仍被拒 |
| src/engine/tests.rs | `task46_default_mode_resolved_for_dispatch_without_preset` | 未设→经济；设盲审→盲审；指向不存在→经济；显式 --preset 优先；run 的 preset_id 记实际值 |
| src/engine/tests.rs | `task46_default_preset_id_rules` | 空 / 未知回经济，存在的预设生效，trim 后匹配 |
| src/console_cli.rs | `task46_config_show_includes_default_mode_and_modes` | show 含 defaultMode 与 modes，source / seat / effective 结构 |
| src/console_cli.rs | `task46_config_show_defaults_when_modes_missing` | 未配置时 defaultMode 为经济、全部位置未指定 |
| src/console_cli.rs | `task46_config_set_default_mode_accepts_alias_and_id` | 别名与 id 都认、show 读回一致、未知退出 2 并列出可选 |
| src/console_cli.rs | `task46_config_set_seat_writes_and_show_reads_back` | 写入后 show 读回一致；规范 id / provider、fallback provider 为 null、tier / thinking 覆盖 |
| src/console_cli.rs | `task46_config_set_seat_rejects_bad_arguments` | 每个退出 2 来源各一条（含多 provider、备选缺配对、备选模型不存在），失败不写盘 |
| src/console_cli.rs | `task46_config_seat_writes_reject_broken_modes_file` | 三条写命令遇损坏退出 1 且文件内容不变；show 宽松读取不受影响 |
| src/console_cli.rs | `task46_config_clear_seat_single_and_all` | 清单个置 null、清全部删 seats 键、重复清幂等 |
| src/console_cli.rs | `task46_window_commands_match_cli_shapes` | 窗口命令实现体与命令行输出同构；错误返回 (退出码, 说明) |
| src/console_cli.rs | `task46_dispatch_without_preset_flag_omits_preset` | `--preset` 缺省时不写死经济；显式别名照旧解析 |

## 实际输出（临时目录，真机二进制）

环境：`AICROSS_HOME=C:\Users\keros68\AppData\Local\Temp\tmp.8sAnnKUbaF\aicross-t46`，预先放入只含 pi / claude 两条的最小 `resources.json`，不真探测、不真冒烟、不派发。

`aicross config set-seat blind 1 1 --agent pi --model glm-4.7 --provider zai-coding-cn --thinking 中 --fallback-agent claude --fallback-model sonnet`（stdout，stderr 为「已写入模式人选」，退出 0）：

```json
{"builtin":true,"default":false,"id":"builtin-blind","name":"盲审模式","stages":[[{"effective":{"agent":"pi","fallback":{"agent":"claude","model":"sonnet","provider":null},"model":"glm-4.7","provider":"zai-coding-cn","thinking":"中","tier":""},"role":"验收","seat":{"agent":"pi","fallback":{"agent":"claude","model":"sonnet","provider":null},"model":"glm-4.7","provider":"zai-coding-cn","thinking":"中","tier":""},"source":"位置","visibility":"盲","write":"只读"},{"effective":null,"role":"验证","seat":null,"source":"未指定","visibility":"盲","write":"只读"},{"effective":null,"role":"验证","seat":null,"source":"未指定","visibility":"盲","write":"只读"}]]}
```

`aicross config set-default-mode blind`（stdout 截去其它模式的 stages，stderr 为「已写入默认模式」，退出 0）：

```json
{"defaultMode":"builtin-blind","modes":[{"id":"builtin-shared","name":"共享模式","builtin":true,"default":false,"stages":…},{"id":"builtin-economy","name":"经济模式","builtin":true,"default":false,"stages":…},{"id":"builtin-blind","name":"盲审模式","builtin":true,"default":true,"stages":[[{"role":"验收","visibility":"盲","write":"只读","seat":{"agent":"pi","provider":"zai-coding-cn","model":"glm-4.7","tier":"","thinking":"中","fallback":{"agent":"claude","provider":null,"model":"sonnet"}},"effective":{…同 seat…},"source":"位置"},{"role":"验证","seat":null,"effective":null,"source":"未指定"},{"role":"验证","seat":null,"effective":null,"source":"未指定"}]]},{"id":"builtin-arena","name":"擂台模式","builtin":true,"default":false,"stages":…},{"id":"builtin-jury","name":"评审模式","builtin":true,"default":false,"stages":…},{"id":"builtin-swarm","name":"蜂群模式","builtin":true,"default":false,"stages":…}]}
```

`aicross config show`（写入前）：`defaultMode` 为 `builtin-economy`、六个预设的 `modes` 全量列出，每个位置 `"source":"未指定"`、`"seat":null`、`"effective":null`；`roles` / `dispatch` / `acp` 各段照旧。写入后 `defaultMode` 为 `builtin-blind`，盲审模式 `default:true`、首位置 `source` 为 `位置`。

退出码抽查：`set-seat` 未知 agent → stderr「未知 agent：ghost；资源库中可选：pi、claude、codex、kimi」退出 2；位置超出 → 「位置超出范围：盲审模式 第 1 阶段共 3 个位置」退出 2；`clear-seat blind 1 1` 与 `clear-seat blind` 都退出 0，后者把 `modes.json` 的 seats 清成 `{}`。

## 验证命令（共用主仓编译目录 `D:/work/aicross/target`）

| 命令 | 结果 |
|---|---|
| `CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets` | 通过，0 warning 0 error |
| `CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release` | 通过，0 warning 0 error |
| `CARGO_TARGET_DIR=D:/work/aicross/target cargo test` | `test result: ok. 472 passed; 0 failed; 10 ignored` |

换行核对：`src/server.rs` 全文 CRLF（319/319 行），`src/main.rs` 未改动，其余 Rust 文件保持 LF，新文件 LF。

## 未做的事与理由

- 不做窗口页面（TASK-47）；本期的窗口命令在 `server.rs` 注册后暂无前端调用方，结构与命令行对齐并由测试覆盖。
- 不改 ai-cross skill 读配置的部分（排在 TASK-46 之后）。
- 不做「编排者真机核」：`aicross config set-seat blind 1 1 …` + 不带 `--preset` 的真实派发会拉起各家 CLI 消耗额度，任务书明确这步由编排者做。
- 不给 `resources_smoke` 窗口命令回传逐 agent 的 stderr 说明行（operation.errors）：任务书约定返回值同 stdout 结构、错误只回一行说明，冒烟失败本身已在返回的资源数据里（`verification` / `smoke_failure`），窗口页可读。
- 未收敛 `roles_from_strict` 与 `modes_from_strict`、两份 `write_json_atomic` 的既有重复（注释里已注明收敛留待后续）。

## 不确定处

- `resources_smoke_at` 重排后，agent 整体冒烟路径里「先打印各条 errors 再读库失败（退出 3）」的极端顺序变成「直接退出 3、errors 不再逐行打印」；开始读库失败的主路径行为不变，既有测试全部通过。这条差异只影响冒烟写库成功但回读失败的罕见组合。
- `clear-seat <模式> <阶段> <位置>` 对「本来就没设」的位置不做任何写盘（等价于已清空）；任务书只说「置 null」，两种实现的最终状态一致。
- `config show` 在 modes.json 损坏时不给 stderr 提示（roles.json 损坏有提示）：任务书只要求宽松读取按空配置处理，没要求提示行；如需要可与 roles 同款补一行。

## 修正轮 r1（按 tasks/fix-46-r1.md）

改动文件：`src/server.rs`（CRLF 保持）、`src/console_cli.rs`、`src/orchestration.rs`、`src/engine/mod.rs`、`src/engine/tests.rs`、`docs/CONSOLE-CHANNEL.md`。逐条改法：

1. **窗口命令卡主线程**：六个窗口命令（`config_snapshot`、`resources_detect`、`resources_smoke`、`config_set_default_mode`、`config_set_seat`、`config_clear_seat`）全部 `async` + `tauri::async_runtime::spawn_blocking`。`config_snapshot_at` 改为接收调用方查好的 acp 报告（原来在主线程拉 `where.exe`），生产调用方在阻塞线程里查 `acp_setup::adapter_status()`。
2. **严格读取误把读不了当缺失**：`roles_from_strict` / `modes_from_strict` 只有 `ErrorKind::NotFound` 回默认值，非 UTF-8、无权限等读取错误返回 Err（「roles.json 读不了：…」/「modes.json 读不了：…」）；`strict_modes_for_write` 与 `set-role` 把这条原文接进「为避免覆盖已停止」，`config show` 的提示行同源。非 UTF-8 时写命令退出 1、文件内容不变。
3. **窗口探测另写一份**：抽出共用的 `resources_detect_at(root, ops)` 返回 `{ value, messages }`（messages 含逐条探测说明与「已探测 N 个 agent」），CLI `detect` 分支与窗口 `resources_detect_command_at` 都调它；窗口返回 JSON 追加 `messages` 字段（字符串数组，空时也给 `[]`），不再丢 cc-switch 相关说明。
4. **窗口冒烟的校验与失败**：「--provider 需要 --model」挪进共用的 `resources_smoke_at`（返回参数错误 2），窗口不再绕过；冒烟失败（命令行退出 1）窗口返回 Err，内容为 stderr 那几行说明（映射在 `smoke_command_value`），成功仍回 Ok。
5. **空人选**：`fill_seats` 对 agent 或 model trim 后为空的人选按未设处理，继续走规则 3；`thinking` 不在 关/低/中/高 内时忽略、不覆盖预设值。合法值清单收进 `orchestration::THINKING_LEVELS`，命令行校验与引擎补人共用一份。
6. **进程内写锁**：三条 modes 写命令经 `modes_write_lock()` 覆盖「严格读 → 修改 → 写盘」全程，窗口与命令行同进程并发写不再互相覆盖；跨进程不加锁，文档注明「同时从窗口和命令行改人选，后写的为准」。
7. **seats 按预设形状补齐**：`ensure_seat_slot` 换成 `shape_seats_to_preset`，写盘前把阶段数与每阶段位置数补到与该预设一致（null），前面的阶段不再落盘为空数组。
8. **文档**：`set-seat` 的退出 2 按来源分写（参数与范围错 / 不在资源库时列出可选模型），补资源库读不了退出 3；modes.json 一节补「agent/model 为空按没设处理、非法 thinking 不覆盖」。

新增测试：

| 文件 | 测试名 | 覆盖 |
|---|---|---|
| src/orchestration.rs | `task46_r1_strict_read_rejects_non_utf8_files` | 非 UTF-8 的 roles.json / modes.json 严格读报错、宽松读照旧回默认 |
| src/engine/tests.rs | `task46_fill_seats_ignores_empty_seat_and_invalid_thinking` | 空 agent / 空 model 的人选不算「位置」；非法 thinking 沿用预设 |
| src/engine/tests.rs | `task46_blind_with_roles_but_no_seats_stays_unspecified` | 盲审设了角色、无位置人选时来源仍全部「未指定」 |
| src/engine/tests.rs | `task46_fill_seats_ignores_extra_positions_in_row` | 同一阶段人选位置数多于预设节点数，多出的忽略 |
| src/console_cli.rs | `task46_r1_window_outputs_match_cli_json_exactly` | snapshot 对 show、set-seat、clear-seat、set-default-mode 四对窗口/命令行输出完整 JSON 比对 |
| src/console_cli.rs | `task46_r1_detect_window_value_and_messages_match_cli` | 窗口探测值去掉 messages 后等于命令行 stdout，messages 等于 stderr 逐行；空时 `[]` |
| src/console_cli.rs | `task46_r1_window_smoke_failure_returns_stderr_lines` | 窗口冒烟失败回 Err(stderr 行)、批量部分失败、--provider 无 --model 的参数错 |
| src/console_cli.rs | `task46_r1_non_utf8_config_files_rejected_without_overwrite` | 非 UTF-8 的 modes.json / roles.json 写命令退出 1、内容不变 |
| src/console_cli.rs | `task46_r1_seat_write_pads_seats_to_preset_shape` | 写入后 seats 补齐到预设的阶段数与位置数 |

另在 `task46_config_set_seat_rejects_bad_arguments` 补位置为 0、位置非整数、备选 agent 不在资源库三条退出 2 用例。

验证（工作树根目录，共用 `D:/work/aicross/target`）：

| 命令 | 结果 |
|---|---|
| `CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets` | 通过，0 warning 0 error |
| `CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release` | 通过，0 warning 0 error |
| `CARGO_TARGET_DIR=D:/work/aicross/target cargo test` | `test result: ok. 481 passed; 0 failed; 10 ignored` |

换行核对：`src/server.rs` 全文 CRLF（335/335 行），`src/main.rs` 未改动（CRLF），其余改动文件保持 LF。未 commit、未 push。

## 复审（Claude 子代理，结论「可以合并」）与编排者追加

- 修正清单 9 条复审判定全部改到位。
- 编排者直接修复审的三条低危：`shape_seats_to_preset` 写入时把 seats 截成预设形状（预设改短后的旧人选清掉，文档同步）；`modes_write_lock` 注释改为「严格读 → 修改 → 写盘」；`fill_seats` 对 agent 或 model 为空的备选按未设处理。新增测试 `task46_fill_seats_ignores_empty_fallback`、`task46_seat_write_trims_seats_beyond_preset_shape`。
- 编排者验证：撤掉 `modes_from_strict` 的 NotFound 判断后 `task46_r1_strict_read_rejects_non_utf8_files` 与 `task46_r1_non_utf8_config_files_rejected_without_overwrite` 变红，恢复后通过。最终 `cargo check --all-targets` 零 warning，`cargo test` 483 通过、0 失败、10 忽略。
- Later：cc-switch 发现提示在 `resources detect` 的命令行与窗口输出里都没有（原有行为，本机不用 cc-switch），有需要时在探测结果里加 `ccSwitchHints`。
