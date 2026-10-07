# REPORT-59 — 评审视角可选、短命令只由用户调用、外部安装来源

接 TASK-57，同一工作树。只改 `src/**`、`docs/CONSOLE-CHANNEL.md`，未动
`src-ui/**`，未加依赖，未 commit、未 push。

## 1. 评审视角可选

### 改法

- `src/orchestration.rs`：`HostModes` 增加 `focus: IndexMap<String, Vec<String>>`
  （serde default）。键为模式 id（目前只认 `builtin-jury`），值按扁平位置序
  存视角文字；旧文件没有该字段时按空表处理。
- `src/engine/mod.rs`：
  - 原默认视角表 `JURY_PERSPECTIVES`（安全 / 性能 / 需求覆盖）保留为默认表；
    新增 `JURY_EXTRA_PERSPECTIVES`（规范 / 正确性 / 可维护性，用任务书原文）
    与 `JURY_FOCUS_OPTIONS`（六个内置视角，`config show` 的 focusOptions 用）。
  - 新增 `jury_focus_text(name)`：内置视角查表，其它自定义文字套模板
    「X评审：只从X视角审查这批材料，指出问题与改进建议。」，空串给空。
  - 新增 `jury_focus_effective(section, index)`：host 段设了非空串用设的，
    否则回默认表；新增 `jury_focus_names(section, preset)`：按形状给出全部
    位置的生效视角名单，非评审模式给空名单。`config show` 与派发共用这份
    解析，不抄两份。
  - `preset_node_texts(preset, focus)` 接视角名单；`stamp_with` 增加
    `focus: &[String]` 参数；`plan_dispatch` 在 `shaped_preset`（counts 同一
    条 host 段路径）之后用 `jury_focus_names` 算名单传给盖章。
- `src/console_cli.rs`：
  - `mode_entry` 的 `focus` 改取生效值；评审模式位置另附 `focusOptions`。
  - 新增 `validate_focus_text`：去首尾空白后 1–20 字，空串表示恢复默认；
    换行、制表与其它控制字符（含 ANSI 转义，`char::is_control` 覆盖 ESC 与
    C1）拒绝，退出 2。
  - 新增 `config_set_focus_at(root, host, mode, index, focus)` 与命令行
    `run_config_set_focus`：校验顺序为 视角文字 → 模式不存在 → 只认
    builtin-jury（其它模式退出 2「…的视角不可设置，目前只认评审模式」）→
    序号超出（按该主 agent 调整后的形状算，退出 2「位置序号超出范围：…只认
    1–N」）。读改写在 `modes_write_lock` 内；写入方式是补齐列表到 index 后
    置该位，从不删除已有项，因此数量调小时多出的视角项保留、调大后恢复。
    输出该模式那一项（同 `config show` 的 `modes[i]`）。
- `src/server.rs`（CRLF 保持）：新增窗口命令 `config_set_focus(host, mode,
  index, focus)`，直接调 `config_set_focus_at`，注册进 invoke_handler。

### 输出字段变化

- `config show`（及 `config_snapshot` 的 config 段）：评审模式每个位置的
  `focus` 从固定默认表改为实际生效值（host 段设了非空串用设的，缺省回
  安全 / 性能 / 需求覆盖）；评审模式位置新增 `focusOptions`
  `["安全","性能","需求覆盖","规范","正确性","可维护性"]`。
- `config set-focus`（新增命令）与窗口 `config_set_focus`：输出同
  `config show` 的 `modes[i]`（id、name、description、builtin、default、
  count、stages 含各位置 focus/focusOptions/seat/effective/source）。
- modes.json：`hosts.<host>` 可出现 `"focus": { "builtin-jury": ["…"] }`。

## 2. 短命令只由用户调用

### 改法

- `src/slash.rs` `command_file_text`：claude 与 kimi 的 frontmatter 在
  `description`（claude 另有 `argument-hint`）之后、`aicross-managed` 之前
  增加 `disable-model-invocation: true`；codex 与 pi 不变。
- `list_locked`：每条已登记命令新增 `stale`。仅状态为「已写入」的条目把
  文件内容与 `command_file_text` 现在会生成的内容做整文比对（含预设改名后
  文件里旧名的情况），不一致为 true；被占用、未写入、名称无效一律 false，
  不读磁盘。`slash add` 同名即重写，重写后 stale 回 false。

### 输出字段变化

- `slash list` / `slash add` / `slash add-all` / `slash remove` 与窗口
  `slash_list` 等：`commands[]` 每条新增布尔字段 `stale`（`modes[].command`
  复制同一结构，随带该字段）。
- claude / kimi 的命令文件内容多一行 `disable-model-invocation: true`
  （文件整文比对因此把旧版文件标为 stale）。

## 3. 外部安装的来源

### 改法

- `src/skill_install.rs`：
  - `SkillLocation` 新增 `manager: String`。
  - 新增 `ScanContext { home, project, xdg_state_home }`（家目录与项目目录
    由调用方注入）；`inspect_location_full` 增加 context 参数，
    `inspect_location` 便捷版（install / uninstall 内部与部分旧测试用）传
    空上下文，manager 留空。
  - 新增 `external_manager`：位置是链接且目标（相对目标按链接所在目录解析，
    Windows 比较统一小写）在 `~/.skills-manager/` 下 → `"skills-manager"`；
    否则全局位置在 skills.sh 全局锁、项目位置在项目根 `skills-lock.json`
    里登记了 `ai-cross`（读 `skills` 对象的键）→ `"skills.sh"`；其余空串。
    链接指向优先于锁登记（锁只记技能名，判不了具体路径）。锁文件读不了或
    格式不对按未登记，不报错。
  - 新增 `skills_sh_global_lock_path`：设了非空 `XDG_STATE_HOME` 用
    `$XDG_STATE_HOME/skills/.skill-lock.json`，否则 `~/.agents/.skill-lock.json`；
    `host_status` 拆为读进程环境的外壳与注入版 `host_status_at`，测试不碰
    真实环境变量。

### 输出字段变化

- `skill status`（及 `skill install` / `skill uninstall` 输出的 status、
  窗口 `skill_status`）：`locations[]` 每条新增字符串字段 `manager`；
  只有 `kind` 为 `external` 的位置可能为 `"skills.sh"` / `"skills-manager"`，
  `ours`、`none` 与识别不了的外部位置为空串。

## 测试（全部用临时目录，未写真实家目录与真实数据目录）

新增 8 条：

- `engine::tests::task59_jury_focus_text_lookup_and_template`（内置查表、
  自定义模板、空串给空）
- `engine::tests::task59_jury_focus_names_follow_config_and_shape`（生效
  名单、空串回默认、数量 3↔2 多余项不进名单且调回后恢复、非评审模式空名单）
- `engine::tests::task59_stamp_texts_follow_focus_overrides`（盖章文本用
  规范文本、自定义套模板、空串回默认）
- `engine::tests::task59_dispatch_stamps_configured_focus`
  （start_dispatch_with → plan_dispatch 全路径：规范 / 自定义 / 恢复默认 /
  counts=2 时多余 focus 不影响）
- `console_cli::tests::task59_config_set_focus_writes_and_shows`（写入生效、
  focusOptions、`--focus ""` 恢复默认、show 读回同一份、数量 2→3 后第三位
  视角恢复、序号超范围退出 2）
- `console_cli::tests::task59_config_set_focus_rejects_bad_input`（缺选项、
  其它模式、模式不存在、序号 0 / 超出 / 非数字、超长、换行、制表、ANSI
  转义，均退出 2；失败不写盘）
- `slash::tests::task59_disable_model_invocation_and_stale_flag`（claude /
  kimi 带该行、codex / pi 不带；旧版缺行与手改正文标 stale，重写回 false，
  被占用不标）
- `skill_install::tests::task59_external_manager_lock_files`（XDG 优先、
  家目录回退、项目锁、格式不对 / 缺文件 / 登记别的技能当未登记、ours 与
  none 为空串）
- `skill_install::tests::task59_external_manager_skills_manager_link`（链接
  目标在 `~/.skills-manager` 下标 skills-manager，他处链接不标；建不了链接
  的环境跳过并注明）

改动 3 条既有测试：`engine::tests::task56_stamp_texts` 帮助函数加 focus
参数（旧用例传 `&[]`）；`slash::tests::task53_write_paths_and_contents_per_host`
的整文期望加 claude / kimi 的新行。既有测试全部未改语义、原样通过
（含 task57 的 focus 展示用例，默认行为不变）。

## 验证（工作树根目录，`CARGO_TARGET_DIR=D:/work/aicross/target`）

```
cargo check --all-targets   →  Finished `dev` profile，零 warning
cargo check --release       →  Finished `release` profile，零 warning
cargo test                  →  633 passed; 0 failed; 10 ignored
```

## 没做什么、不确定处

- 界面侧（TASK-60）未动：`focusOptions`、`stale`、`manager` 只在引擎输出里
  给出，前端如何呈现由 TASK-60 决定。
- `manager` 的两种来源同时命中时按链接指向算 `skills-manager`：任务书未定
  优先级，取「位置级证据优先于按技能名登记」，已写进 docs 与代码注释。
- 视角项为空串即「恢复默认」，因此 `focus` 列表里可能留有空串占位（补齐
  序号时产生），modes.json 里可见；语义与「缺失」相同。
- `focus` 写入从不删多余项（含数量调小时），与任务书一致；如需清理只能
  整体删 host 段的键（现无命令，界面也不提供）。

## 修正轮 r1（fix-59-r1）

按 `tasks/fix-59-r1.md` 的「要改」四条修改；「不改」一条（`--focus -性能`
的提示语）未动。

1. **skills-manager 链接判定**（`skill_install.rs` `external_manager`）：
   位置是链接时先对位置与 `~/.skills-manager` 各做 `std::fs::canonicalize`，
   都成功就按真实路径比对（覆盖带 `..` 的相对目标与链式链接）；任一失败退回
   原逻辑（按链接文本，相对目标按链接所在目录拼）。
2. **视角文字校验**：规则收成 `engine::normalize_focus_text`，
   `console_cli::validate_focus_text` 只做错误码包装。先在原串上拒绝
   `\n`、`\r`、`\t`（首尾也拒绝，不再被 trim 吃掉），再 trim；空串仍表示
   恢复默认；1–20 字；其它控制字符（含 ANSI 转义）拒绝；另拒绝 Unicode Cf
   类格式字符（按 Unicode 15 的 Cf 区段列表：U+00AD、U+200B–U+200F、
   U+202A–U+202E、U+2060–U+2064、U+2066–U+206F、U+FEFF 等），报错里以
   `\u{XXXX}` 显示。std 无通用类别表，列表上限已注在代码旁。
3. **读出口校验**（`engine::jury_focus_effective`）：读到的 focus 过
   `normalize_focus_text`，不合格或规整后为空回默认表，不报错。
   `config show`（`console_cli` 扁平 focus）与派发盖章（`jury_focus_names`
   → `preset_node_texts`）都经这一处，一并生效。
4. **stale 比对**（`slash.rs`）：新增 `comparable_text`（去开头 BOM、CRLF
   统一成 LF），文件内容与生成文本两侧都规整后再比。

`docs/CONSOLE-CHANNEL.md` 同步三处：视角校验规则与读出口回落、
skills-manager 按真实路径比对、stale 比对前的规整。

新增测试 4 条，扩充 1 条：

- `engine::tests::task59_normalize_focus_text_rules`（首尾 / 中间换行、
  回车、制表，ANSI，零宽、双向覆盖与隔离、BOM、软连字符，20 / 21 字边界，
  空白串规整为空）
- `engine::tests::task59_focus_from_handwritten_modes_falls_back`（临时
  目录里手写 modes.json：带换行、超长、零宽、双向覆盖、制表的视角回默认表，
  带首尾空格的合格视角去空白后生效，盖章文本同步）
- `skill_install::tests::task59_external_manager_canonical_chain_and_relative`
  （链式链接：位置 → 中转 junction/symlink → `.skills-manager`，判为
  skills-manager；相对目标 `../../.skills-manager/...` 的符号链接同样判定。
  本机 Windows 未开开发者模式，相对目标段建不了 symlink，已跳过并打印说明；
  链式段用 junction 实测通过）
- `slash::tests::task59_stale_ignores_crlf_and_bom`（四个主 agent，CRLF、
  BOM、BOM+CRLF 三种变体都不算 stale，内容真变仍算）
- 扩充 `console_cli::tests::task59_config_set_focus_rejects_bad_input`：
  加首尾换行 / 制表与零宽、双向覆盖、BOM 五个用例，均退出 2。

验证（工作树根目录，`CARGO_TARGET_DIR=D:/work/aicross/target`）：

```
cargo check --all-targets   →  Finished `dev` profile，零 warning
cargo check --release       →  Finished `release` profile，零 warning
cargo test                  →  642 passed; 0 failed; 10 ignored
```

未做项：无。相对目标 symlink 用例在本机跳过，需在能建 symlink 的环境
（Unix 或开了开发者模式的 Windows）上才实际跑到。
