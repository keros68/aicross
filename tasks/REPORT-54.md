# REPORT-54 — 每个模式的斜杠命令（引擎侧）

任务书：`tasks/TASK-54-slash-per-mode.md`。在 TASK-53 未提交改动之上继续，只改 `src/**` 与 `docs/CONSOLE-CHANNEL.md`，未动前端。

## 改动文件

- `src/slash.rs`
  - 新增短名表 `MODE_SHORTNAMES`（预设 id、短名、关键词，六个内置模式按定案顺序）与默认前缀 `DEFAULT_PREFIX = "ax-"`。
  - `list_locked` 输出追加 `modes`：每条含 `mode`、`modeName`、`argForm`、`suggestedName`、`command`（helpers：`builtin_mode_items`）。`argForm` = 该 host 默认调用写法 + 关键词；`suggestedName` = `ax-<slug>`；`command` 取 `commands` 里 mode 等于该模式的第一条，没有为 `null`。
  - `slash_add_at` 拆成「校验 + 取锁」的入口与实现体 `slash_add_locked`（调用方持锁），写入逻辑一行未改。
  - 新增 `slash_add_all_at`：前缀校验 `validate_prefix`（小写字母、数字、`-`，可空）→ 拼出的六个名字逐个过 `validate_name` → 取锁 → 严格读 modes.json → 逐模式「已有该模式命令（任何名字）则跳过，否则按 `<前缀><短名>` 走 `slash_add_locked`」→ 单个失败记入 `skipped` 不中断 → 输出同 list 另加 `added`、`skipped`。
- `src/console_cli.rs`：`run_slash_at` 新增 `add-all` 子命令（`--host` 必给、`--prefix` 缺省 `ax-`，未知选项退出 2，stdout 同实现体输出，stderr「已生成短命令」，有跳过时加一行「跳过 N 条，原因见输出」）；缺子命令的提示改为 `list | add | add-all | remove`。
- `src/server.rs`：新增窗口命令 `slash_add_all(host, prefix?)`（async + spawn_blocking，host 解析同其余三个短命令），并注册进 `invoke_handler`。
- `docs/CONSOLE-CHANNEL.md`：命令块加 `add-all`；`slash list` 输出补 `modes` 与固定短名/关键词清单；新增 add-all 行为段（跳过规则、前缀规则、退出码、`added`/`skipped`）；损坏拒写一句补上 add-all；窗口命令清单加 `slash_add_all`；「三个子命令」改为「各子命令」。

## 复用的现成函数

- `slash_add_at` 的写入逻辑（拆出的 `slash_add_locked`，add-all 逐模式调用，写入顺序与文件回滚与单条 add 完全一致）。
- `validate_host` / `validate_name`（拼出的名字照过名称校验）、`invoke_hint`（argForm）、`file_state` 与占用判定（经 `slash_add_locked`）。
- `console_cli::modes_write_lock`（整批一把锁，与单条 add、其它写 modes.json 的命令互斥）、`console_cli::strict_modes_for_write`（损坏/旧格式拒写）、`console_cli::resolve_host_for_write`、`option_value` / `unknown_option_error`。
- `orchestration::modes_from` / `raw_presets_from` / `save_modes_in` / `host_section`，`paths::test_temp_dir`（测试临时目录）。

## 新增测试

`src/slash.rs`（全部用临时目录）：

- `task54_list_modes_arg_forms_and_suggested_names`：四家 argForm 写法、suggestedName、模式名与顺序、无命令时 command 为 null。
- `task54_list_modes_command_matches_first_of_mode`：command 取同模式第一条，其余模式为 null。
- `task54_add_all_generates_every_builtin_mode`：全新生成 6 条、文件内容与手写 add 一致、换前缀与另一 host 照常、空前缀合法。
- `task54_add_all_skips_modes_that_already_have_commands`：已有某模式命令（任何名字）时跳过并给原因，已有命令不被改动。
- `task54_add_all_records_occupied_names_as_skipped`：目标名字被别人占用时记入 skipped 且其余照常，别人的文件原样。
- `task54_add_all_invalid_prefix_exits_2`：前缀字符不认与拼出的名字超长都退出 2，且发生在任何写入之前。
- `task54_add_all_corrupt_modes_blocks_writes`：modes.json 损坏时退出 1、不写任何文件。

`src/console_cli.rs`：

- `task54_slash_add_all_command_line_uses_default_prefix`：默认前缀 `ax-`、未知选项退出 2、`--prefix` 覆盖、全部跳过时 stderr 提示与 skipped 原因。

## 验证

工作树根目录执行（结果原文摘录）：

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.97s     # 零 warning

CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
    Finished `release` [optimized] target(s) in 8.09s                       # 零 warning

CARGO_TARGET_DIR=D:/work/aicross/target cargo test
test result: ok. 580 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out
```

单独跑 `cargo test task54`：8 个新测试全过。换行复查：`src/server.rs` 仍为 CRLF，`src/slash.rs`、`src/console_cli.rs`、`docs/CONSOLE-CHANNEL.md` 仍为 LF。

## 未做与理由

- 前端不接 `slash_add_all`（任务书明确不改 `src-ui/**`）；窗口命令已注册，后续期次可调。
- 不做删除批量生成的命令：卸载已有单条 `slash remove`，任务书未要求批量版。
- 未加依赖、未联网、未 commit。

## 不确定处

- `--prefix` 允许为空（名字即短名 shared / blind 等）：任务书只给「小写字母、数字、`-`」的字符规则，按字符规则实现并加了测试；若要求非空，改 `validate_prefix` 一处即可。
- 目标名字已是我们自己的命令但指向别的模式时，add-all 会按 add 的既有「同名定义则更新」语义把该命令改成对应模式（说明保留）：任务书要求「调现有 slash_add 逻辑写入」，故未额外拦截；测试未覆盖此分支。
- 批次内有跳过时整体仍退出 0（跳过不是失败，原因在输出的 `skipped` 里）；若要求「全部跳过也算失败」需另定。
