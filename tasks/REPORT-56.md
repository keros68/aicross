# REPORT-56 — 内置模式的位置数量可调、介绍改写（引擎侧）

## 改了什么

### 介绍改写

- `src/orchestration.rs`：`builtin_presets()` 六个内置模式的 `description` 按任务书
  全部替换（不再写「一家 / 三家」，改写厂商 / 模型）。
- `src/preset.rs`：`task55_builtin_presets_carry_descriptions` 与
  `task55_description_shows_in_config_and_slash_list` 的期望文本同步更新。

### 位置数量可调

- `src/orchestration.rs`：
  - `HostModes` 追加 `counts: IndexMap<String, u32>`（serde 缺省为空；序列化键
    `counts`）。
  - 新增可调表 `COUNT_SPECS`（`CountSpec`：preset_id / role / stage / min / max /
    default）与 `count_spec()`：经济（第 3 阶段「验证」0–3 默认 1）、盲审
    （「验证」1–4 默认 2，另有 1 个「验收」固定）、擂台（「执行」2–4 默认 3）、
    评审（「验证」1–3 默认 3）；共享、蜂群不可调。
  - 新增 `shape_builtin_preset(preset, count)`：增加复制该角色的模板节点（附在
    固定节点之后），减少从末尾去掉，越界夹取；不可调预设原样返回。
  - 新增 `shaped_preset(preset, section)`：查该主 agent 的 `counts`（缺省或越界用
    默认值）后调 `shape_builtin_preset`。派发、`config show`、补人、推荐、
    set-seat / clear-seat 的范围判断都经它取形状，只此一份。
- `src/engine/mod.rs`：
  - `plan_dispatch` 的预设查找改为经 `shaped_preset`（含 `--preset` 与默认模式两条路）。
  - `preset_node_texts` 从「按预设 id 查固定下标表」改为按传入预设的位置生成：
    评审视角文本按「安全 → 性能 → 需求覆盖」依次取（新增需求覆盖一段文案）；
    盲审与经济的「验证」位置都用现有验证路由文本，其余位置不覆盖；`stamp_with`
    相应改传整个预设。
- `src/console_cli.rs`：
  - `mode_entry`（`config show` 的 `modes[i]`）按调整后的形状展示，并追加
    `count` 字段：可调模式为 `{ role, value, min, max, default }`（value 为当前
    生效值），不可调为 null；`presetThinking` 改从调整后的形状取。
  - 新增 `config_set_count_at` + `run_config_set_count`，挂在
    `aicross config set-count <模式> <数量> [--host <h>]`：别名解析同 `--preset`；
    模式不存在、不可调、越界都退出 2（越界信息给范围，如「只认 1–4」）；写入
    counts 后对已有人选复用 `shape_seats_to_preset` 按新形状整理（减少裁掉、
    增加补 null）；输出该模式那一项（同 `config show` 的 `modes[i]`）。
  - `config_set_seat_at` / `config_clear_seat_at` 的阶段、位置范围改按调整后的
    形状算（counts 加位后新位置可设人选，减位后超出范围拒绝）。
- `src/recommend.rs`：`recommend_into` 对每个预设先经 `shaped_preset` 再补人；
  写盘回填 `recommend_at_home` 同样按调整后的形状 `shape_seats_to_preset`，
  减掉的位置不因回填复原。
- `src/server.rs`：新增窗口命令 `config_set_count(host, mode, count)`（同实现体，
  阻塞线程池），并注册进 invoke_handler。
- `docs/CONSOLE-CHANNEL.md`：modes.json 样例与字段说明补 `counts`；新增可调表；
  `config show` 的 `modes[]` 说明补 `count` 字段；新增 `config set-count` 条目；
  set-seat 说明改为「范围按调整后的形状算」；示例块补一条 set-count。

## 复用的现成函数

- `shape_seats_to_preset`（set-count 裁剪/补位、推荐回填、set-seat 落盘前整理）。
- `fill_seats`（补人与来源标注，形状由调用方先经 `shaped_preset` 整好）。
- `resolve_preset_name` / `unknown_mode_message` / `resolve_host_for_write` /
  `modes_write_lock` / `strict_modes_for_write` / `seat_entry_after_write` /
  `mode_entry`（set-count 与既有 config 写命令同源）。
- `stamp_with` / `preset_node_text`（位置文本仍走盖章路径，只换文本生成）。

## 新增测试

`src/orchestration.rs`：
- `task56_count_specs_list_adjustable_modes`（可调表的角色、阶段、范围、默认；
  默认值与内置预设现形状一致；共享、蜂群不可调）
- `task56_shape_builtin_preset_grows_and_shrinks`（四个模式增减后的阶段形状、
  模板复制、其余阶段不动、不可调原样、越界夹取）
- `task56_shaped_preset_reads_counts_and_falls_back_to_default`（counts 生效、
  越界与缺省回默认、其它模式的 counts 不串、不可调模式原样）

`src/engine/tests.rs`：
- `task56_stamp_texts_follow_adjusted_shape`（评审视角按序取、盲审与经济的验证
  文本、经济减到 0 只剩验收）
- `task56_dispatch_builds_nodes_per_counts`（盲审 counts=1 派发只建验收 + 1 路验证，
  验证节点正文为验证路由文本）

`src/console_cli.rs`：
- `task56_config_show_reports_count_and_shaped_stages`（count 字段四态、擂台减位
  后 stages 形状、超出的旧人选不显示）
- `task56_config_set_count_trims_and_pads_seats`（写入 counts、减少裁掉人选、
  增加补空位、输出同 `modes[i]`）
- `task56_config_set_count_rejects_non_adjustable_and_out_of_range`（缺少参数、
  不可调、越界、非数字、未知模式退出 2；失败不写盘）
- `task56_seat_range_follows_adjusted_shape`（set-seat 范围随 counts 变化）

`src/recommend.rs`：
- `task56_recommend_fills_adjusted_shape`（擂台减到 2 只填两处并裁掉第三路手选、
  changed 置位；盲审加到 4 后按 5 个位置填，同阶段厂商互不相同，凑不出留空并
  写 notes）

更新：`src/preset.rs` 的 `task55_builtin_presets_carry_descriptions`、
`task55_description_shows_in_config_and_slash_list`（新介绍文本）。

## 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets
  → Finished `dev` profile … in 3.45s（无 warning、无 error）
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
  → Finished `release` profile … in 7.72s（无 warning、无 error）
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
  → test result: ok. 604 passed; 0 failed; 10 ignored; 0 measured
```

测试全部用临时目录（`paths::test_temp_dir()` 与各模块的 temp 夹具），不碰真实
家目录与真实 aicross 数据目录。

## 换行

保持原样：`src/server.rs` 全文 CRLF（新增命令行同为 CRLF，已用脚本核对全文
651 行均为 CRLF）；其余改动文件（orchestration.rs、engine/mod.rs、
engine/tests.rs、console_cli.rs、recommend.rs、preset.rs、
docs/CONSOLE-CHANNEL.md）均为 LF。

## 未做与理由

- 前端（src-ui/**）不改：任务限定引擎侧；窗口命令 `config_set_count` 已在 Rust
  侧注册，前端接线留给界面期。
- `slash.rs` 未改：短命令只取模式名与介绍，不消费预设形状；介绍随
  `builtin_presets` 自动更新（preset.rs 的 slash 断言已覆盖）。
- 推荐对「执行」位置的同阶段厂商与模型去重规则未动（现状只对审查位去重），
  擂台加位后推荐可能在两路选出同一模型；改推荐规则超出本期范围，如需要另开一期。
- `config show` 的 `count.value` 展示时对越界值按默认形状回退（与
  `shaped_preset` 同一判断），未在展示层报错：手改 modes.json 的容错沿用
  宽松读取的既有约定。

## 不确定处

- 需求覆盖视角的正文为新写文案（「需求覆盖评审：只从需求覆盖视角审查这批材料，
  指出遗漏、偏差与补充建议。」），与既有两段保持同构；如用户另有措辞偏好再改。
- `config set-count` 把数量设回默认值时 `counts` 里仍保留该条目（值等于默认），
  不主动删除；行为与 set-default-mode 保留显式值一致。

## 修正轮 r1（fix-56-r1，TASK-54 到 56 审查意见）

按 `tasks/fix-56-r1.md`「要改」逐条处理；「不改」条目（数量调到 0 后再调大
模板丢失）未动，实际加固由第 9 条覆盖。只改了 `src/**`、
`docs/CONSOLE-CHANNEL.md` 与本报告，未动 `src-ui/**`，未 commit。

### 逐条改法

1. **`preset remove` 级联顺序**（`src/preset.rs` `preset_remove_at`）：改为先在
   锁内逐个删带标记的短命令文件，全部成功后一次写入 modes.json（人选、默认
   模式、slash 定义），最后删预设。任一步失败停止；删文件失败时错误说明列
   出已删与未删的条目（host / 命令名，形如 `claude/order-a`），并写明
   modes.json 与 presets.json 未改动。`src/slash.rs` 的测试注入
   `FAIL_REMOVE_FILE` 从一次性布尔改为计数器（非 0 时第 n 次删文件失败，
   用后即清，原 `set(true)` 用法语义不变），供中途失败用例注入。
2. **级联前校验**（`src/preset.rs` + `src/slash.rs`）：`validate_host` 与
   `validate_name` 改为 `pub(crate)`；`preset remove` 在删文件前对每条要级
   联的短命令定义重新过这两道校验，host 或名称不合法的定义只从 modes.json
   里移除、不拼路径不碰磁盘，在输出 `affected.invalidSlash`
   （`[{ host, name }]`）里标出，不再触发 `command_path` 里的 `unreachable!`。
3. **一键生成撞名**（`src/slash.rs` `slash_add_all_at`）：生成前查
   `<前缀><短名>` 是否已被别的模式的定义使用；是则不改写，记入 `skipped`，
   原因「名称已被 <模式名> 使用」（预设缺失时回落成 id）。
4. **更新自定义模式后人选错位**（`src/preset.rs` `preset_save_at`）：更新已
   有预设时，阶段数、每阶段位置数或任一位置的角色变了，就删各主 agent 下该
   模式的 seats（slashCommands 与默认模式保留），受影响的主 agent 列在输出
   新增的 `seatsCleared` 字段；只改介绍或思考强度不清。modes.json 先写、
   presets.json 后写，后者失败时错误说明写明人选已清、预设未更新。
5. **更新时 id 不存在**：显式给了 `id` 但 presets.json 里没有该 id 时退出 2
   「模式不存在：<id>」，不静默新建（`task55_preset_id_validation` 的期望同步
   更新：合法但不存在 → 退出 2；先无 id 新建、再按 id 更新照常）。
6. **id 与别名冲突**：`preset save` 显式 id 经 `resolve_preset_name` 解析后
   不是原样时退出 2「预设 id 不能用内置模式的别名：<id>」，覆盖 shared /
   economy / blind 及后续可能新增的别名。
7. **介绍校验**（`src/preset.rs` `validate_input`）：`description` 最长 200
   字；`name` 与 `description` 不收换行、制表以外的控制字符与 ANSI 转义
   （ESC 本身是控制字符，同一判断覆盖），退出 2；换行与制表可以原样保存。
8. **set-seat / clear-seat 形状在锁外取**（`src/console_cli.rs`）：形状改在
   `modes_write_lock` 内按严格读到的 modes.json 重新取 `shaped_preset`，并
   在锁内重新校验阶段与位置范围；counts 与人选同一次读改写，不被并发的
   set-count 改动隔开。
9. **可调位置的模板与顺序**（`src/orchestration.rs` `shape_builtin_preset`）：
   底版与模板固定从 `builtin_presets()` 的原始预设按 id 取，不再拿传入预设
   （可能是已整过形的副本）当模板；在原始形状里把可调角色的整段节点原位换
   成 count 个模板副本——内置模式的可调节点都靠在阶段末尾，等价于「多则从
   末尾去掉、少则在该角色最后一个节点之后追加」，阶段内顺序不打乱。
10. **测试**见下节；裁剪用例各位置填不同模型，盲审 4 → 1 时验收人选保留
    （从末尾裁，第 2 位人选留在剩下的验证位上），经济 1 → 0 → 2 位置恢复、
    恢复的位置人选为空。

### 新增测试

- `src/preset.rs`：`task55_alias_ids_are_rejected`（第 6 条）、
  `task55_description_and_name_character_rules`（第 7 条）、
  `task55_save_update_clears_seats_when_shape_changes`（第 4 条）、
  `task55_remove_file_failure_stops_before_modes_write`（第 1 条，注入第 2 次
  删文件失败）、`task55_remove_drops_invalid_slash_definitions_without_disk`
  （第 2 条，坏 host 与坏名称不碰磁盘）；`task55_preset_id_validation` 更新
  （第 5 条）。
- `src/slash.rs`：`task54_add_all_skips_names_taken_by_other_mode`（第 3 条，
  同一用例覆盖「已有命令跳过」与「撞名跳过」两种原因）。
- `src/console_cli.rs`：`task56_set_count_trim_keeps_fixed_seats_and_restores_shape`
  （第 10 条裁剪语义）、`task56_set_seat_reads_modes_strictly_under_lock`
  （第 8 条，损坏的 modes.json 退出 1 不写盘）。
- `src/engine/tests.rs`：`task56_custom_preset_dispatches_with_own_shape`
  （第 10 条，自定义预设走一次 `start_dispatch_with`，节点 target 的角色与
  人选逐个核对）。
- `src/orchestration.rs`：`task56_shape_builtin_preset_is_idempotent_from_any_input`
  （第 9 条，对已整过形的副本再整形与从原始预设直接整形一致）。

### 文档

`docs/CONSOLE-CHANNEL.md`：`preset save` 补显式 id 必须存在、别名不可作 id、
name 与 description 的字符规则、更新改结构时清人选与 `seatsCleared`；
`preset remove` 的顺序改为先删文件、再写 modes.json、最后删预设，补
`invalidSlash` 说明；`slash add-all` 补撞名不改写的原因；`set-seat` 补「形状
在写入锁内按严格读到的 modes.json 取」。

### 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets
  → Finished `dev` profile …，0 warning 0 error
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
  → Finished `release` profile …，0 warning 0 error
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
  → test result: ok. 614 passed; 0 failed; 10 ignored（原 604 passed，新增 10）
```

### 换行与其它

改动文件（orchestration.rs、preset.rs、slash.rs、console_cli.rs、
engine/tests.rs、docs/CONSOLE-CHANNEL.md）均为 LF；server.rs 与 main.rs 本轮
未改，保持 CRLF。测试全部用临时目录（`paths::test_temp_dir()` 与各模块
temp 夹具），不碰真实家目录与真实数据目录。未加依赖，未联网。
