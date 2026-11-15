# REPORT-83：引擎 0.3.10 遗留项修复与 skill Windows 补测

按 `tasks/TASK-83-engine-followups.md` 与本期追加核对要求，在本机（Windows 11）基于源码 `0.3.10`（未改动 `Cargo.toml` 版本号）完成全部四项引擎问题复现、修复与单测补充，并完成 `skills/ai-cross` 的两项 Windows 补测。涉及配置与派发测试均使用临时 `AICROSS_HOME` 与临时项目目录，未改动本机 `modes.json` 与 `~/.claude/settings.json`。

## 未完成与不符合项

### 一、未完成项

- 无。`tasks/TASK-83-engine-followups.md` 四项与 skill Windows 两项补测均已实跑并验证通过。

### 二、核对中发现的不符合并已在 skill 侧修复的项

- **第 4b 项后半（`dispatch.py` 的 `_mode_has_seat` 对 `unfilledReason` 的判断与引擎含义不一致）**：
  - 修复前 `skills/ai-cross/references/dispatch.py` 中 `_mode_has_seat` 写成 `any(seat.get("effective") or seat.get("unfilledReason") ...)`，把「没有 `effective`、只有 `unfilledReason`」的空位也算作已配置人选。
  - 核对引擎实际语义（`src/engine/mod.rs`、`src/console_cli.rs`）：位置未设置时，只有当全局 `roles.json` 默认人选违反硬规则才会生成 `unfilledReason`，此时 `effective` 仍为 `null`（并没有实际可用人选）。如果宿主所有位置都处于这种状态，`_mode_has_seat` 返回 `True` 会导致 `engine_config` 不回退到 `manifest.json`，而印出满屏「没有人选（角色默认……不合规）」；非默认模式全是这类空位时也会被顺手列出来。
  - **已修**：将 `_mode_has_seat` 改为仅检查 `bool(seat.get("effective"))`，保留 `seat_text` 原样以便默认模式或已有部分有效人选的模式仍显示具体空位原因；补单测 `test_mode_has_seat_ignores_unfilled_reason_without_effective`，`skills/ai-cross/SKILL.md` 版本从 `1.30.5` 升到 `1.30.6`，并通过 `python scripts/sync-skill.py` 同步 `skill-bundle/ai-cross/`（21 个文件）。

### 三、需由用户决定的事项

- **新 `0.3.10` 二进制尚未发布安装**：任务书要求不改应用版本号、不发 Release，因此本机 PATH 里原已安装的仍是 `aicross 0.3.7`（`C:\Users\keros68\AppData\Local\aicross\aicross.exe`）；本次构建与验证的新功能在 `D:\work\aicross\target\debug\aicross.exe`（`0.3.10`）。是否重新打包并替换本机安装版本、以及何时推送仓库与发布，由用户决定。

## 引擎四项复现、修改与验证记录（源码 `0.3.10`）

动手前先用当前源码构建 `target\debug\aicross.exe`（`aicross --version` 输出 `aicross 0.3.10`），确认四项在 `0.3.10` 中均仍存在（无跳过项）。

### 1. kimi 只读位置改了项目文件且没有提示

- **修前复现（`0.3.10`）**：
  - 在临时目录 `%TEMP%\aicross-t83-1-home` 设置 `acpEnabled: false` 并保存 1 节点共享只读自定义预设（`id=f75ed8b5-3d84-47c2-92f3-7099c3146e07`），临时 git 仓库 `%TEMP%\aicross-t83-1-proj` 里任务要求新建 `kimi-readonly-probe.txt`：
  - 命令：`.\target\debug\aicross.exe dispatch --host claude --preset f75ed8b5-3d84-47c2-92f3-7099c3146e07 --seat "1.1=kimi::kimi-code/k3-256k" --project $p1 --text-file "$h1\task.txt" --timeout 180` 后接 `.\target\debug\aicross.exe status ede9ef4c-666c-42fd-90a1-ac72a46ea63f --wait --timeout 180`
  - 关键输出：`PROBE_EXISTS=True`、`GIT_STATUS=?? .aicross/ ?? kimi-readonly-probe.txt`，`status` JSON 为 `"status":"done"` 且不含 `notes` 字段，结果记录 `.aicross\runs\20261115-231822-ede9ef4c\20261115-231837-kimi-kimi-code-k3-256k-验证.md` 的 `runner_note` 仅有 `kimi 不报用量`，未提示项目文件被改动。
- **修改内容**：
  - `src/dispatch.rs`：新增 `KIMI_CHANGED_FILES_PREFIX`、`format_kimi_changed_files_note`、`is_aicross_relative_path`、`git_project_state`、`git_changed_files` 与 `DispatchNodeResult.summary_notes`；当 `kimi` 在单次命令行通道（`!writable && needs_oneshot`）执行只读位置时，派发前后各取一次项目目录的 git 状态（排除 `.aicross/`），非 git 仓库静默跳过，有变化时把「`派发期间项目里这些文件有变化：…。kimi 没有只读档，核对是不是它改的。`」写入 `runner_note` 与 `summary_notes`（并在备选重派间保留）。
  - `src/engine/mod.rs`：`RunUpdate` 新增 `notes`，`make_update`、`run_pipeline` 与 `FileSink::update` 将节点 `summary_notes`（及 `runner_note` 中的变化提示）汇总进本次 `run` 摘要 `notes`。
  - 单元测试：`dispatch::tests::task83_kimi_readonly_warns_on_project_file_changes_and_skips_non_git`、`engine::tests::task83_kimi_changed_files_note_propagates_to_run_summary_and_filesink`。
- **修后复现验证（`0.3.10`）**：
  - 在全新临时 git 仓库 `%TEMP%\aicross-t83-1-proj-post` 重跑同一命令：

```powershell
$env:AICROSS_HOME = "$env:TEMP\aicross-t83-1-home"
.\target\debug\aicross.exe dispatch --host claude --preset f75ed8b5-3d84-47c2-92f3-7099c3146e07 --seat "1.1=kimi::kimi-code/k3-256k" --project "$env:TEMP\aicross-t83-1-proj-post" --text-file "$env:TEMP\aicross-t83-1-home\task.txt" --timeout 180
.\target\debug\aicross.exe status 1e1fb27b-16c4-475e-a84c-0bf27670e716 --wait --timeout 180
```

  - 关键输出原文：
    - `PROBE_EXISTS=True`、`GIT_STATUS=?? .aicross/; ?? kimi-readonly-probe.txt`
    - `STATUS_STDOUT` 含 `"notes":["派发期间项目里这些文件有变化：kimi-readonly-probe.txt。kimi 没有只读档，核对是不是它改的。"]`
    - 记录文件 `.aicross\runs\20261115-233701-1e1fb27b\20261115-233724-kimi-kimi-code-k3-256k-验证.md` 的 `runner_note` 为：
      `派发期间项目里这些文件有变化：kimi-readonly-probe.txt。kimi 没有只读档，核对是不是它改的。` 与 `kimi 不报用量`。
- **结论：已修复并实测通过**。

### 2. `adopt --verdict-file` 在带序号的派发目录上落错位置

- **修前复现（`0.3.10`）**：
  - 构造临时 `AICROSS_HOME` `%TEMP%\aicross-t83-2-home` 与临时项目 `%TEMP%\aicross-t83-2-proj\.aicross\runs\20261115-201451-2\`，执行：
    `.\target\debug\aicross.exe adopt 20261115-201451-2 --verdict-file "$h2\verdict.md"`
  - 关键输出原文：`EXIT=0`、`STDOUT={"fix":1,"keep":0,"never":0,"path":"C:\\Users\\keros68\\AppData\\Local\\Temp\\aicross-t83-2-home\\runs\\20261115-201451-2\\裁决.md"}`、`EXISTS_HOME=True`、`EXISTS_PROJ=False`。
- **修改内容**：
  - `src/project_dir.rs`：修改 `find_run_dir`，当 `crate::engine::valid_id(run_id)` 为真时先按目录名完全相等检查 `runs_dir(project).join(run_id)`，命中即返回；未命中再按原有 `-short_id` 后缀规则查找。
  - 单元测试：`project_dir::tests::task83_find_run_dir_prefers_exact_match_before_short_id_suffix`、`engine::tests::task83_adopt_verdict_writes_to_project_exact_suffix_dir`。
- **修后复现验证（`0.3.10`）**：
  - 删除修前产生的 `$h2\runs\20261115-201451-2\裁决.md` 后重跑同一命令：

```text
CMD=aicross adopt 20261115-201451-2 --verdict-file "C:\Users\keros68\AppData\Local\Temp\aicross-t83-2-home\verdict.md"
EXIT=0
STDOUT={"fix":1,"keep":0,"never":0,"path":"C:\\Users\\keros68\\AppData\\Local\\Temp\\aicross-t83-2-proj\\.aicross\\runs\\20261115-201451-2\\裁决.md"}
STDERR=已记录裁决：改 1 件，不改 0 件，以后不再报 0 件
EXISTS_HOME=False
EXISTS_PROJ=True
```

- **结论：已修复并实测通过**。

### 3. `modes.json` 损坏时退出码仍为 0

- **修前复现（`0.3.10`）**：
  - 在临时目录 `%TEMP%\aicross-t83-3-home` 写入损坏内容 `{ broken` 到 `modes.json`：
  - `CMD=aicross config show --host claude EXIT=0`、`STDOUT_PREFIX={"acp":[{"agent":"claude","state":"未安装"},...`、`STDERR=已读取配置`
  - `CMD=aicross config show EXIT=0`、`STDOUT_PREFIX={"acp":[{"agent":"claude","state":"未安装"},...`、`STDERR=已读取配置`
- **修改内容**：
  - `src/orchestration.rs`：更新 `modes_from_strict`，在读文件失败、旧格式 v1、version 不支持与 JSON 解析错误消息中均带上 `modes_path(root).display()` 与具体原因。
  - `src/console_cli.rs`、`src/engine/mod.rs`、`src/hosts.rs`、`src/slash.rs`：在配置读取与解析入口（`resolve_host_or_err`、`referenced_model_targets`、`run_hosts_at`、`start_dispatch_with`、`list_hosts_at`、`list_locked`）统一使用 `modes_from_strict`，使 `modes.json` 损坏时返回退出码 `1` 并把路径与原因输出到 stderr（文件不存在时仍合法按空配置退出 `0`）。
  - 单元测试：`console_cli::tests::task83_corrupt_modes_json_reports_path_and_reason_nonzero`，并同步更新 `task46_config_seat_writes_reject_broken_modes_file`、`task50_modes_v1_lenient_read_is_empty_and_strict_read_rejects`、`task46_modes_strict_read_rejects_broken_and_wrong_version`、`task46_r1_strict_read_rejects_non_utf8_files`。
- **修后复现验证（`0.3.10`）**：

```text
CMD=aicross config show --host claude EXIT=1
STDOUT=
STDERR=modes.json 无法解析（C:\Users\keros68\AppData\Local\Temp\aicross-t83-3-home\modes.json）：key must be a string at line 1 column 3
CMD=aicross config show EXIT=1
STDOUT=
STDERR=modes.json 无法解析（C:\Users\keros68\AppData\Local\Temp\aicross-t83-3-home\modes.json）：key must be a string at line 1 column 3
```

- **结论：已修复并实测通过**。

### 4. `aicross dispatch --help` 等子命令认 `--help` / `-h`

- **修前复现（`0.3.10`）**：
  - `CMD=aicross dispatch --help EXIT=2`、`STDOUT=`、`STDERR=缺少 --project`
  - `CMD=aicross status --help EXIT=2`、`STDOUT=`、`STDERR=缺少 run_id`
  - `CMD=aicross open --help EXIT=2`、`STDOUT=`、`STDERR=open 不接受选项参数`
- **修改内容**：
  - `src/console_cli.rs`：新增 `USER_SUBCOMMANDS`（15 个用户子命令）、`VALUE_OPTIONS`、`subcommand_help_requested`、`subcommand_usage` 与 `write_subcommand_help`，并在 `run_if_subcommand`、`dispatch_root`、`run_at_with` 接入；跳过带值选项后的参数（避免 `--text -h` 被误判），遇到 `--help` / `-h` 时打印对应子命令用法并返回退出码 `0`（`open --help` 也不启动图形窗口）。
  - 单元测试：`console_cli::tests::task83_all_subcommands_accept_help_and_short_h`（覆盖全部 15 个子命令的 `--help` / `-h`、多级子命令及 `--text -h` 边界）。
- **修后复现验证（`0.3.10`）**：

```text
CMD=aicross dispatch --help EXIT=0
STDOUT:
用法：
  aicross dispatch --project <目录> --text-file <文件> [选项]

常用选项：
  --preset <模式>        指定预设 id 或别名（shared / economy / blind）
  --host <主 agent>      指定宿主配置（多于一份配置时必填）
  --seat <规格>          仅本次覆盖某位置，写法 <阶段>.<位置>=<agent>:<provider>:<model>
  --accept-file <文件>   附带验收条件全文
  --kind <类型>          记录任务类型
  --read-only-task       标明本次为只读任务
  --help, -h             显示此用法
STDERR=
CMD=aicross status --help EXIT=0 ... STDERR=
CMD=aicross open --help EXIT=0 ... STDERR=
CMD=aicross dispatch -h EXIT=0 ... STDERR=
```

- **结论：已修复并实测通过**。

## skill Windows 两项补测记录

### 4a. 超过 200 KB 材料点名 `agy` 盲审（`REQUEST.md` 提示与读取作答）

- 运行前确认本机 `agy.EXE --version` 为 `1.5.9`。生成无敏感占位内容 + 末尾含 `compute_average([])` 空列表除零缺陷的测试文件 `material.txt`（`269,186` 字节），并在旁路线程轮询 `D:\aicross-data\blind\agy-*/REQUEST.md` 捕获槽位清理前的全文头尾。
- 执行命令：

```text
python D:\work\aicross\skills\ai-cross\references\dispatch.py review --host claude --entry agy.google --tier low --kind general --material-file C:\Users\keros68\AppData\Local\Temp\aicross-t83-4a\material.txt --project C:\Users\keros68\AppData\Local\Temp\aicross-t83-4a --full --go
```

- 关键输出原文：
  - `EXIT=0`
  - `CAPTURED_PATH=D:\aicross-data\blind\agy-1\REQUEST.md`、`CAPTURED_BYTES=270405`
  - `HAS_NO_TOOLS_FILE=True`、`HAS_OLD_NO_TOOLS=False`（首段结尾为：`材料已全部附在下面，除读取本文件外不要使用其他工具。`）
  - 回答正文：`Google gemini-3.8-flash-low（经 agy） ｜ ok ｜ 非独立：隔离不完整`，完整指出末尾 `compute_average(items)` 在 `items = []` 时抛出 `ZeroDivisionError` 违反需求返回值 `0`。
- **结论：符合，`skills/ai-cross/` 无需为此项修改**。

### 4b. `dispatch.py start --host qoder` 与 `_mode_has_seat` 核对

- 将新构建目录 `$PWD\target\debug`（`aicross 0.3.10`）置入 `PATH` 最前并执行：

```powershell
python skills/ai-cross/references/dispatch.py start --host qoder
```

- 关键输出原文：

```text
装了 aicross 但这次用不了：未知 host：qoder；检测到：claude、codex、kimi、pi、opencode、agy、hermes、zcode、codebuddy、workbuddy、qoder-cn。把这句告诉用户，改按 manifest 派发。


能力清单（更新于 2026-11-15）
...
```

- 说明：`dispatch.py` 入口按要求输出一行原因后改看 `manifest.json`。对于 `_mode_has_seat` 将只有 `unfilledReason` 而没有 `effective` 的空位误判为已配置的差异，已在本期修复并同步至 `1.30.6`（见文首「不符合并已在 skill 侧修复的项」）。

## 全量测试与验证

所有代码改动、单测补充与 `python scripts/sync-skill.py` 完成后重跑全套验证，临时测试目录 `%TEMP%\aicross-t83*` 已清理完毕，`Cargo.toml` 版本保持 `0.3.10`。

1. skill 单元测试（在 `D:\work\aicross\skills\ai-cross`）：

```powershell
python -m unittest discover -s tests
```

关键输出：`Ran 110 tests in 8.026s`、`OK`（0 失败，0 跳过）。

2. 引擎构建与单元测试（在 `D:\work\aicross`）：

```powershell
cargo check && cargo test
```

关键输出：
- `cargo check`：`Finished dev profile [unoptimized + debuginfo] target(s) in 3.60s`
- `src\lib.rs`：`test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`
- `src\main.rs`：`test result: ok. 817 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 73.26s`
- `Doc-tests aicross`：`test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`
