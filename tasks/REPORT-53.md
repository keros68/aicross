# REPORT-53 — 斜杠命令：默认调用方式与自定义短命令（引擎侧）

## 改动文件

- `src/slash.rs`（新建）：短命令模块。`slash_list_at` / `slash_add_at` /
  `slash_remove_at`（root + 注入的家目录 + host），三种文件状态
  `已写入 / 未写入 / 被占用` 按磁盘实际判断（frontmatter 带
  `aicross-managed: true` 为已写入）；各家路径（claude
  `~/.claude/commands/<名>.md`、codex `~/.codex/prompts/<名>.md`、pi
  `~/.pi/agent/prompts/<名>.md`、kimi `~/.kimi-code/skills/<名>/SKILL.md`）、
  invoke 与 trigger 写法、文件内容生成（claude / codex 带 `argument-hint:
  <任务>` 与 `$ARGUMENTS` 占位，pi 无占位、kimi 另有 `name`；`description`
  为「ai-cross · <模式名>」）；写入用临时文件 + rename，失败清临时文件；
  remove 只删带标记的文件，kimi 连空目录一起移除。定义更新与文件写删都在
  `modes_write_lock()` 一把进程内锁里。
- `src/orchestration.rs`：`SlashCommand`（name / mode / note，camelCase），
  `HostModes` 加 `slash_commands` 字段（`#[serde(default)]`，旧文件按空数
  组处理）。
- `src/console_cli.rs`：`aicross slash list|add|remove`（`run_slash` +
  `run_slash_at`，家目录注入与 `run_skill` 同一写法）；`slash` 进
  `run_if_subcommand` 的子命令清单与 `run_at_with` 分派；参数校验用现成的
  `unknown_option_error` / `option_value`；`strict_modes_for_write` 改为
  `pub(crate)` 供 slash 模块复用（modes.json 损坏 / 旧格式退出 1 的说明同
  一份）。stderr：`已列出短命令` / `已添加短命令` / `已移除短命令`。
- `src/main.rs`：`mod slash;`。
- `src/server.rs`：窗口命令 `slash_list(host)`、
  `slash_add(host, name, mode, note?)`、`slash_remove(host, name)`，全部
  async + `spawn_blocking`，注册进 `invoke_handler`。
- `docs/CONSOLE-CHANNEL.md`：命令清单加 `aicross slash`；新增「自定义短命
  令」一节（三个子命令、各家路径与输入写法表、文件内容与标记、被占用退出
  1、list 输出与三种 state、modes.json 的 `slashCommands`）；modes.json 示
  例补 `"slashCommands": []`。

## 复用的现成函数

- `console_cli::resolve_preset_name`（别名解析同 `--preset`）、
  `console_cli::unknown_mode_message`（模式不存在退出 2 的说明）、
  `console_cli::modes_write_lock`（modes.json 写锁，定义与文件同锁）、
  `console_cli::strict_modes_for_write`（损坏 / 旧格式拒写）、
  `console_cli::resolve_host_for_write`（`--host` 规则同 TASK-50）。
- `orchestration::modes_from`（list 宽松读取，同 `config show`）、
  `modes_from_strict`（经 `strict_modes_for_write`）、`raw_presets_from`、
  `host_section` / `host_section_mut`、`save_modes_in`（内含临时文件 +
  rename 的原子写）。
- `paths::test_temp_dir`（测试目录）；CLI 家目录注入模式照 `run_skill`；
  窗口命令照 `resolve_window_host` + `spawn_blocking` 模式。

## 设计决定（任务书未明说处）

- 三个子命令都经 `resolve_host_for_write` 解析 `--host`：短命令文件按 host
  分路径写家目录，没有具体 host 无法定位；给了 `--host` 但资源库读不了退
  出 3、不是检测到的 agent 退出 2，与 TASK-50 写命令一致。
- `remove` 未找到同名定义退出 2「未找到短命令：<名>」；定义在而文件被人换
  成无标记内容时照删定义、不动文件。
- `add` 的校验顺序：名称 → 说明 → 模式（不存在退出 2）→ 持锁后先查被占用
  （退出 1 不覆盖），再严格读 modes.json（损坏退出 1，不写文件也不改定义），
  然后写文件、存定义。
- 正文 `note` 按任务书模板直接接在指令句号后（非空时），占位行 `$ARGUMENTS`
  另起一行，文件末尾一个换行。
- list 对 modes.json 损坏按宽松读取显示空清单（与 `config show` 同一处理），
  不让读命令因配置文件损坏而失败。

## 新增测试

src/slash.rs：

- `task53_write_paths_and_contents_per_host`（四家路径、frontmatter 差异、
  `$ARGUMENTS` 只有 claude / codex、kimi 目录 + SKILL.md、modes.json 定义、
  list 的 trigger / invoke / modeName / path / state）
- `task53_name_validation`（大写、空格、下划线、斜杠、空名、33 位退出 2；
  1 / 32 位合法）
- `task53_unknown_mode_exits_2_and_alias_resolves`（`nope` 退出 2 并列可选；
  `blind` 解析成 `builtin-blind` 落盘）
- `task53_note_validation`（多行、201 字退出 2；200 字合法且进正文）
- `task53_occupied_file_not_overwritten`（无标记文件退出 1、内容原样、定义
  不写入）
- `task53_add_updates_existing_definition_and_overwrites_own_file`（同名定
  义更新、自己的文件被覆盖、清单只一条）
- `task53_remove_only_deletes_marked_files`（别人的文件留下、定义照删；自
  己的文件连 kimi 空目录一起删）
- `task53_list_reports_all_three_states`（未写入 / 已写入 / 被占用）
- `task53_corrupt_modes_json_blocks_writes`（损坏与旧格式退出 1，文件与定
  义都不动）

src/console_cli.rs：

- `task53_slash_commands_use_injected_home_and_validate_args`（list 空清单
  与 `/ai-cross`、缺子命令 / 缺 `--mode` / 坏名称 / 未知模式退出 2、未检测
  的 host 退出 2、add 写进注入家目录且 stdout 与实现体一致、remove 后文件
  随定义删除、未找到退出 2；全程临时目录）

## 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets
  → Finished `dev` profile；warning 计数 0
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
  → Finished `release` profile；warning 计数 0
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
  → test result: ok. 562 passed; 0 failed; 10 ignored
CARGO_TARGET_DIR=D:/work/aicross/target cargo test task53
  → 10 passed（上面列的 10 个新测试）
```

换行核对（python 字节统计）：`src/server.rs`（567 行）与 `src/main.rs`
（277 行）纯 CRLF；`src/slash.rs`、`src/console_cli.rs`、
`src/orchestration.rs`、`docs/CONSOLE-CHANNEL.md` 纯 LF；未用 shell
heredoc 写源码。测试全部走 `paths::test_temp_dir` 与注入的家目录，未写真
实家目录与真实 aicross 数据目录。

## 未做与理由

- 前端（slash 三个窗口命令的界面）不在本期：任务书指明窗口侧按第 3 节契
  约并行开发，本期只交付命令与窗口命令（`src-ui/**` 未动）。
- 被占用文件里的 frontmatter 用逐行 `key: value` 识别，只认
  `aicross-managed: true` 的精确写法：标记是我们自己的契约，别人按别的格
  式写同名文件时按「被占用」处理是安全侧。
- `modes.json` 的跨进程锁没加：与 set-seat 等写命令同一边界（同进程互不踩，
  跨进程后写的为准），不在这期单独立规矩。
- 真机验证各宿主对斜杠命令的识别（claude / codex / pi / kimi 是否照
  frontmatter 与占位生效）未做：无宿主环境，路径与格式按 PRODUCT.md 10-06
  经官方文档核实的表实现，留用户实测。

## 修正轮 r1（fix-53-r1）

逐条改法（对应 fix-53-r1「要改」）：

1. **配置与文件一致**：`slash_remove_at` 改为先删带标记的文件（kimi 只删
   我们写的 SKILL.md，目录空了才连目录一起删）再存定义，删文件失败直接报
   错、配置不动；定义不在但文件带标记时也清掉文件，两者都不在才退出 2。
   `slash_add_at` 先写文件再存定义，写前记住旧文件内容，存定义失败时回滚
   文件（新建的删掉，更新的恢复原内容），真实失败与测试注入同一路径。
2. **保留名**：`validate_name` 拒绝 `ai-cross` 与 Windows 设备名（con、nul、
   aux、prn、com1–com9、lpt1–lpt9），退出 2；kimi 写入前另查
   `~/.agents/skills/<名>`（我们从不往那里写短命令，存在即退出 1）与
   `~/.kimi-code/skills/<名>`（目录已在而我们没有带标记的 SKILL.md 时
   同样退出 1）。
3. **frontmatter 转义**：`command_file_text` 的 `description`（与 kimi 的
   `name`）改用 serde_json 序列化成带引号字符串（JSON 风格转义）；模式名
   与说明在正文里的换行、控制字符替换为空格（`single_line_text`）。
4. **标记判定**：`has_managed_marker` 重写——先去掉开头的 BOM；只认首行
   `---` 起且找到闭合 `---` 的 frontmatter（未闭合不认）；标记必须顶格
   （`strip_prefix` 吃不到缩进行），值允许行尾 `# 注释`。
5. **名称与 host 校验**：三个入口（`slash_list_at` / `slash_add_at` /
   `slash_remove_at`）开头都跑 `validate_host`（只认 claude / codex / pi /
   kimi，退出 2）；`slash_remove_at` 对名称跑 `validate_name`；list 对名称
   不合法的条目标 `state: "名称无效"`、不读磁盘。`invoke_hint` /
   `trigger_hint` / `command_path` 改成四家显式分支，不再用 `_` 通配落到
   kimi（未预期的 host 值走 `unreachable!`）。
6. **`--note` 以 - 开头**：`run_slash_at` 的 add 分支先摘出 `--note=<说明>`
   参数再对其余参数跑 `unknown_option_error`，说明以 `-` 开头时可用这种
   写法；文档已写明。
7. **测试**：见下。失败注入用 `#[cfg(test)]` 的 thread_local 标记
   （`FAIL_REMOVE_FILE` / `FAIL_SAVE_MODES`，用后即清，每个测试线程各自
   一份），只在测试构建里存在。

新增测试（src/slash.rs，除前两个外均为本轮新增；
`task53_write_paths_and_contents_per_host` 重写为完整文件内容精确比对）：

- `task53_write_paths_and_contents_per_host`（改为四家完整文件内容逐一比
  对，含 kimi 的 `name:`、只有 claude / codex 有 `argument-hint` 与
  `$ARGUMENTS`、frontmatter 值带引号）
- `task53_frontmatter_escaped_and_body_single_line`（description 转义引号
  与换行；正文控制字符替换为空格；kimi 无 argument-hint / 占位）
- `task53_has_managed_marker_rules`（正常、CRLF、行尾 # 注释、BOM、缩进、
  frontmatter 外、未闭合、无 frontmatter、值非 true、同前缀不同键）
- `task53_reserved_names_rejected`
- `task53_unknown_host_rejected_at_entry_points`（`_` 与其它非法 host 在三
  个入口都退出 2，不落到 kimi 路径；remove 对非法名称退出 2）
- `task53_remove_file_failure_keeps_config`（删文件失败注入：定义留在配
  置、文件留在磁盘；注入清除后能正常删）
- `task53_add_save_failure_rolls_back_file`（存配置失败注入：新建的删掉、
  更新的恢复原内容，定义不变）
- `task53_kimi_agents_dir_blocks_add`（`.agents/skills` 已存在、以及
  `.kimi-code/skills` 目录已在而没有我们带标记的 SKILL.md，两种占用）
- `task53_kimi_remove_keeps_extra_files`
- `task53_remove_cleans_orphan_file_without_definition`
- `task53_list_marks_invalid_names`

src/console_cli.rs：`task53_slash_commands_use_injected_home_and_validate_args`
追加 `--note=-开头说明` 写法断言（说明以 - 开头时写进正文）。

文档（docs/CONSOLE-CHANNEL.md）：保留名与 `--note=<说明>` 写法；
description / name 带引号转义与正文控制字符处理；标记判定细则；kimi 的
`~/.agents/skills` 占用；add / remove 的先后顺序与回滚；state 增加名称无
效；正文措辞同步为「按「<模式名>」处理」（去掉旧句多出的「模式」二字）。

验证（均在 D:\work\aicross-wt\t46）：

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets
  → Finished `dev`；warning 0
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
  → Finished `release`；warning 0
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
  → test result: ok. 572 passed; 0 failed; 10 ignored
```

换行核对（python 字节统计）：`src/slash.rs`、`src/console_cli.rs`、
`docs/CONSOLE-CHANNEL.md` 纯 LF（CRLF 计 0）；`src/server.rs`、
`src/main.rs` 纯 CRLF。测试全程用 `paths::test_temp_dir` 与注入的家目录，
未写真实家目录。未 commit、未 push。
