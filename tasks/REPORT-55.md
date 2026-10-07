# REPORT-55 — 模式介绍与自定义模式（引擎侧）

任务书：`tasks/TASK-55-custom-modes.md`。在 TASK-54 未提交改动之上继续，只改 `src/**` 与 `docs/CONSOLE-CHANNEL.md`，未动前端。

## 改动文件

- `src/orchestration.rs`
  - `Preset` 追加 `description: String`（serde default，空串不写出）。
  - 六个内置预设的介绍写死在 `builtin_presets()`（任务书表格逐条照抄）。
  - 新增 `presets_from_strict`（严格读 presets.json：缺失回空清单，损坏、读不了或 version != 1 报错拒写，避免覆盖丢预设）与 `save_presets_in`（原子写）。
- `src/preset.rs`（新文件）
  - `PresetInput`：`preset save` 的输入（复用 orchestration 的 `Stage`/`Target` 反序列化）。
  - `validate_preset_id`（字母、数字、`-`、`_`，1–64 位）、`validate_input`（名字非空 ≤40 字且不重名、阶段与位置数、role/visibility/write/thinking 取值、同阶段最多一个单写者，错误写明第几个阶段第几个位置）、`normalized_preset`（只保留 role/visibility/write/thinking，agent/model 等人选字段清空，builtin 恒为 false）。
  - `preset_save_at`：无 id 新建（生成 uuid），有 id 更新同 id 用户预设；内置（含手写进 presets.json 的 builtin 条目）不可修改。输出 `{ "preset", "defaultMode", "modes" }`，modes 同 `config show`。
  - `preset_remove_at`：内置不可删除、未找到退出 2；级联清理按「先改 modes.json（各主 agent 下该模式的 seats、指向它的 defaultMode 清空、mode 为它的 slashCommands 定义），再删带标记短命令文件，最后删预设」的顺序，任一步失败停止，已完成的部分写进错误说明。输出 `{ "removed": {id,name}, "affected": { "seats", "defaultMode", "slash" } }`。
  - 读改写与短命令、人选共用 `console_cli::modes_write_lock`。
- `src/slash.rs`
  - 删带标记文件的逻辑抽成 `remove_marked_command_file`（只删我们写的文件、kimi 空目录连删、测试注入钩子 `FAIL_REMOVE_FILE` 随之内移），`slash_remove_at` 与 preset remove 级联清理共用。
  - `builtin_mode_items`（`slash list` 的 `modes[]`）追加 `description`（预设已删时留空）。
- `src/console_cli.rs`
  - `mode_entry`（`config show` 的 `modes[]`）追加 `description`。
  - 新增 `preset` 子命令（`run_preset` / `run_preset_save` / `run_preset_remove`）：`aicross preset save --file <json> [--host <h>]`（文件 UTF-8 可带 BOM，读不了、编码错、json 坏退出 2；`--host` 同 TASK-50 规则）与 `aicross preset remove <id>`（家目录解析失败退出 3）；stderr 提示「已保存模式」「已移除模式」。
- `src/server.rs`：新增窗口命令 `preset_save(host, preset)`、`preset_remove(id)`（async + spawn_blocking，返回同结构），并注册进 `invoke_handler`。
- `src/main.rs`：注册 `mod preset;`。
- `src/engine/mod.rs`、`src/engine/tests.rs`、`src/run_view.rs`：既有测试与 console 直给目标的 `Preset` 字面量补 `description: String::new()`（字段新增的连带改动，不改行为）。
- `docs/CONSOLE-CHANNEL.md`：命令清单加 `aicross preset`；`config show` 的 `modes` 字段说明加 `description`；新增「自定义模式」一节（命令、预设 json 格式、校验规则、输出、级联清理顺序与受影响项列表、窗口命令）；`slash list` 的 `modes` 字段说明加 `description`；「留痕与摘要」补 `presets.json`。

## 复用的现成函数

- `orchestration::raw_presets_from` / `builtin_presets`（合并内置与用户预设、内置清单与保护判断）、`write_json_atomic`（经 `save_presets_in`）、`THINKING_LEVELS`。
- `console_cli::modes_section`（保存后的完整 modes 输出，与 `config show` 同源）、`modes_write_lock`、`strict_modes_for_write`、`resolve_host_or_err`、`option_value` / `unknown_option_error` / `unknown_option_error_allow_positionals` / `positional_values`。
- `slash::command_path` / `file_state` / `STATE_WRITTEN` / `remove_empty_command_dir`（经抽出的 `remove_marked_command_file`）、`slash_add_at`（级联测试里造短命令）。
- `engine::fill_seats`（验证自定义预设派发补人）、`paths::test_temp_dir`（测试临时目录）。

## 新增测试

`src/preset.rs`（全部用临时目录，家目录注入）：

- `task55_builtin_presets_carry_descriptions`：六个内置预设的介绍逐条核对。
- `task55_description_shows_in_config_and_slash_list`：`config show` 的 `modes[]` 与 `slash list` 的 `modes[]` 都带介绍。
- `task55_save_creates_updates_and_removes`：新建生成 uuid、更新同 id、介绍留空不写出、删除后回到内置 6 条。
- `task55_builtin_presets_are_protected`：改内置、删内置（含手写 builtin 条目）被拒，不存在的 id 退出 2。
- `task55_name_rules`：空名、超 40 字、与内置重名、与用户预设重名被拒；更新时与自己同名合法；40 字合法。
- `task55_field_validation_and_ignored_seat_fields`：role/visibility/write/thinking 非法值退出 2 且写明位置与字段；agent/model/provider/tier/fallback 忽略并清空。
- `task55_single_writer_per_stage`：同阶段两个单写者被拒，不同阶段各一个合法。
- `task55_node_limit`：13 个位置被拒，12 个合法；零阶段、空阶段被拒。
- `task55_remove_cascades_seats_default_and_slash`：删预设级联清掉两个主 agent 的人选、指向它的默认模式、两个主 agent 的短命令定义与带标记文件（kimi 连空目录），内置模式的短命令不受影响。
- `task55_saved_preset_dispatches_with_seat_fill`：派发同源查找取到新建预设，`fill_seats` 补人照常（有人选来源「位置」，没有走「未指定」）。
- `task55_corrupt_presets_json_blocks_writes`：presets.json 损坏或版本不认时 save/remove 退出 1、不覆盖。
- `task55_preset_id_validation`：非法 id 退出 2，合法 id 直接采用。

`src/console_cli.rs`：

- `task55_cli_preset_save_reads_file_and_outputs_modes`：CLI 全链路（读文件、输出 preset+modes）与缺 `--file`、文件不存在、json 坏、重名各退出 2，未知与缺少子命令退出 2。
- `task55_cli_preset_remove_arg_checks`：缺 id 与多余位置参数退出 2（发生在读家目录之前，不碰真实家目录）。

## 验证

工作树根目录执行（结果原文摘录）：

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.82s     # 0 warning

CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
    Finished `release` profile [optimized] target(s) in 8.67s                # 0 warning

CARGO_TARGET_DIR=D:/work/aicross/target cargo test
test result: ok. 594 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 30.65s
```

单独跑 `cargo test task55`：14 个新测试全过。换行复查：`src/server.rs`（632/632 行）、`src/main.rs`（278/278 行）仍为 CRLF，其余改动文件 CR 计数为 0（LF）。

## 未做与理由

- 前端不接 `preset_save` / `preset_remove`（任务书明确不改 `src-ui/**`）；窗口命令已注册，后续期次可调。
- `preset remove` 不做别名解析（shared→builtin-shared 之类）：别名只指向内置预设，而内置不可删；用户预设与别名撞名（如 id 恰为 `blind`）时精确 id 匹配不误伤。
- 保存时校验返回第一条失败即停，不一次列全部问题（与现有 CLI 校验风格一致）。
- 未加依赖、未联网、未 commit、未 push。
