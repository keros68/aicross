# REPORT-81：Windows 实机核对 skill 1.30.x 改动

按 `tasks/TASK-81-windows-checks.md` 在 Windows 11 开发机逐项核对 `skills/ai-cross`（1.30.3）与 `aicross` 引擎。只改动 `skills/ai-cross/` 并重新同步 `skill-bundle/ai-cross/`，skill 版本升至 `1.30.4`；引擎 `src/` 与 `Cargo.toml` 未改动，`~/.claude/settings.json` 未改动，所有 `~/.cc-switch/*.db` 读取仅限第 3 项且未回显或写入任何密钥。

## 没做完与不符合的项

### 一、不符合并已在 skill 侧修复的项

1. **第 2 项（入口命令走引擎的那条路）**：
   - 宿主已在 `resources.json` 探测到但尚未在 `modes.json`/`roles.json` 配人选（或 `modes.json` 损坏）时，`aicross config show --host <宿主>` 仍退出码 `0` 且所有位置为 `effective: null`。修复前 `dispatch.py start` 误判为「有配置」并列出 6 个全空模式，而不退回 `manifest.json`。
   - 宿主已配人选时（如本机 `codex`、`kimi`），未配置任何有效人选或空位原因的非默认模式（如盲审模式、擂台模式、评审模式）仍被打印为「没有人选（未设）」的不可用清单。
   - **已修**：在 `skills/ai-cross/references/dispatch.py` 新增 `_mode_has_seat(mode)`，`engine_config` 在全部模式都没有任何 `effective` 或 `unfilledReason` 时返回 `(None, f"宿主 {name} 还没有配人选")`，`cmd_start` 跳过没有任何已设人选或空位原因的非默认模式；同步更新 `skills/ai-cross/references/channels.md` 的 `start` 判断顺序。
2. **第 3 项前半（`cc-switch` 冒烟衔接 `verify_model.py`）**：
   - 修复前 `cmd_smoke` 仅提示 `verify_model.py --provider "Zhipu GLM"`，既没有脚本目录路径，也没有带 `--models`。直接照跑时 `verify_model.py` 按 `~/.cc-switch/cc-switch.db` 的默认映射检查三个槽位，其中 `sonnet` 与 `opus` 映射为 `GLM-5.3-Flash[1M]`，端点返回 HTTP 400（模型不存在），退出码 `1`，与 manifest 仅登记低档 `GLM-5.3-Flash` 不一致。
   - **已修**：`cmd_smoke` 对 `cc-switch` 条目改为输出带脚本路径与条目已登记模型清单的命令：`python <references 目录>/verify_model.py --provider "<provider>" --models "<models>"`。真机重跑后两步顺畅衔接并通过。
3. **第 4 项（`agy` 大材料 > 200 KB 首次试跑被旧版本号误导）**：
   - 本机实装 `agy` 为 `1.5.9`，但 `D:\aicross-data\skill\manifest.json` 里 `agy.google` 仍记录旧版本 `1.3.1`。修复前 `dispatch.py` 只看 manifest 里的 `version`，误走 `< 1.5` 的 stdin 内联路径，`agy` 在 209 KB 文件末尾 `# Section 1004` 处截断，漏掉文件末尾缺陷。
   - **已修**：在 `skills/ai-cross/references/dispatch.py` 新增 `agy_supports_1_5(pick)`，登记版本低于 `1.5` 且本地可执行文件存在时按路径补查一次 `inventory.cli_version(path)`。修复后真实派发改走 `--settings` 与 `REQUEST.md` 分支，`agy` 两次调用 `read_file` 读完首尾并检出两处缺陷。
   - **保留观察**：走 `REQUEST.md` 时提示词内仍含「材料已全部附在下面，不要使用任何工具」一句；本次实测模型（`gemini-3.8-flash-low`）没有拒读工具，未触发任务书要求改模板的条件，故未动该句，是否移除以消除语义矛盾交由用户决定。
4. **第 7 项前半（`dispatch.py verdict` 自动选最新派发目录）**：
   - 对 `<时间>` 与 `<时间>-2` 并存时 `verdict` 能命中 `<时间>-2`，但原代码按字典序排序目录名，当同秒存在 `<时间>-10` 时会排在 `<时间>-2` 之前而误选中 `-2`。
   - **已修**：在 `dispatch.py` 的 `cmd_verdict` 新增 `_run_dir_sort_key`，按时间戳与数字后缀排序，经单测与临时 `<时间>-10` 目录实测确认选中 `-10`。

### 二、不符合未修及引擎侧问题记录（本期按要求不改 `src/`）

1. **第 7 项后半（引擎查看器与 `aicross` 命令读 `<时间>-2` 目录）**：
   - `dispatch.py` 仅把记录写进 `<项目>/.aicross/runs/<时间>[-N]/`，不写 `$AICROSS_HOME/runs/<run_id>.json`。引擎查看器（`src/run_view.rs` 的 `list_runs_at`）与 `aicross status / open / recover / adopt` 只认 `$AICROSS_HOME/runs/*.json`，因此引擎查看器与命令都不会列出、打开 skill 自行产生的派发目录（与目录名是否带 `-2` 无关，仅在 skill 侧改名无法解决）。
   - 把 `<时间>-2` 放在 `<项目>/.aicross/runs/` 不会破坏引擎自身对项目快照目录的扫描（`src/project_dir.rs` 的 `all_run_dirs` 扫描全部子目录并按记录头匹配，`find_run_dir` 只按 8 位 `short_id` 后缀匹配）。
   - 用人工构造的 `$AICROSS_HOME/runs/20261115-201451-2.json` 指向上述项目实测：`aicross status` 与 `aicross recover` 返回退出码 `0`；`aicross adopt 20261115-201451-2 --verdict-file ...` 因 `short_id("20261115-201451-2")` 只取前 8 位字母数字 `"20261115"`，无法匹配 `-2` 结尾的项目目录，裁决被写进了 `$AICROSS_HOME/runs/20261115-201451-2/裁决.md` 而非 `<项目>/.aicross/runs/20261115-201451-2/裁决.md`。
2. **第 5 项后半（引擎路径下 `kimi` 作执行者）**：
   - 本机 `D:\aicross-data` 当前设置 `"acpEnabled": false`。核对 `src/dispatch.rs`（第 3139 行附近）、`src/channels/mod.rs`（第 550–572 行）与 `src/channels/acp.rs`：当 `acpEnabled` 为假或 ACP 启动失败回退时，引擎走 `kimi -p <prompt> -m <model> --output-format stream-json`（`-p` 隐含 `--yolo`），同样能读目录外文件与改写项目文件，且只读席位在引擎侧没有派发前后 `git status` 变更提示；即使开启 ACP 拦截 `Write`/`Bash`，Kimi 默认文件读取也无目录沙箱。
3. **第 2 项附带观察（引擎与宿主映射差异）**：
   - 本机 PATH 中安装的 `aicross.exe` 为 `0.3.7`，而工作区 `Cargo.toml` 源码版本为 `0.3.10`（本次 CLI 实测均基于 PATH 中的 `0.3.7` 可执行文件）。
   - `modes.json` 损坏时 `aicross config show --host <宿主>` 静默回退为空模式且退出码仍为 `0`。
   - `src/hosts.rs` 的扩展宿主（`agy`、`opencode`、`gemini` 等）未列入 `dispatch.py` 的 `ENGINE_HOSTS`（仅 `claude`、`codex`、`kimi`、`pi`）；现有单测 `test_engine_config_is_none_without_aicross_or_for_unknown_hosts` 明确要求 `qoder` 返回 `(None, None)`，本期未改，交由用户确认范围。

### 三、本机环境未设条件、按任务书改用单元测试的项

- **第 3 项后半（重定向的 `claude` 条目校验）**：经只读布尔检查，本机 `C:\Users\keros68\.claude\settings.json` 存在且 `env` 中未设置 `ANTHROPIC_BASE_URL`。按任务书要求未修改该配置文件，改以对应的 3 条单元测试代替验证。

## 逐项核对记录

### 1. 基础（单测与 `cargo check && cargo test`）

- 变更前基线（commit `c401be9`，skill `1.30.3`）：

```powershell
cd skills/ai-cross && python -m unittest discover -s tests
```

关键输出：`Ran 106 tests in 10.108s`、`OK`（Windows 下 0 条跳过）。

```powershell
cargo check && cargo test
```

关键输出：`src/lib.rs` 为 `test result: ok. 0 passed; 0 failed`；`src/main.rs` 为 `test result: ok. 811 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 70.16s`。引擎 `src/` 本期未作任何修改。

- 最终修改同步后（skill `1.30.4`）：

```powershell
python scripts/sync-skill.py
cd skills/ai-cross && python -m unittest discover -s tests
```

关键输出：

```text
已同步 21 个文件：D:\work\aicross\skills\ai-cross -> D:\work\aicross\skill-bundle\ai-cross
...
Ran 109 tests in 8.708s

OK
```

- 核对第 7 项同时补跑引擎查看器单测：`cargo test run_view::tests::` 输出 `test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 802 filtered out; finished in 0.07s`。
- **结论：符合**。

### 2. 入口命令走引擎的那条路

- 修前复现现象：
  - 对本机 `codex`、`kimi` 宿主，`aicross config show --host codex` 返回 `rc=0` 且盲审/擂台/评审模式内所有槽位为 `effective: null, unfilledReason: null`，原 `dispatch.py start` 仍印出多行「没有人选（未设）」。
  - 在临时 `AICROSS_HOME` 中放入已探测 `codex` 的 `resources.json`（无 `modes.json` 或 `roles.json`），`aicross config show --host codex` 退出码 `0` 且 6 个模式全为空位，原 `dispatch.py start --host codex` 误报「宿主 codex 有配置」且列出 6 个空模式而不回退 manifest。
- 修复后实机验证命令与关键输出：

```powershell
python skills/ai-cross/references/dispatch.py start --host claude
python skills/ai-cross/references/dispatch.py start --host codex
python skills/ai-cross/references/dispatch.py start --host kimi
```

关键输出原文：

```text
走 aicross 引擎：已安装，宿主 claude 有配置。不用盘点，也不看 manifest。
- 共享模式：执行 claude/opus，可用
- 经济模式（默认）：出方案 claude/fable，可用；执行 claude/sonnet，可用；验收 pi/zai-coding-cn/glm-5.3-flash，已验证；验证 pi/opencode-go/deepseek-v4.1-flash，已验证
- 盲审模式：验收 codex/gpt-6.1-sol，可用；验证 pi/opencode-go/deepseek-v4.1-flash，已验证；验证 kimi/kimi-code/k3-256k，可用
- 擂台模式：执行 claude/sonnet，可用；执行 claude/sonnet，可用；执行 claude/sonnet，可用
- 评审模式：验证 pi/zai-coding-cn/glm-5.3-flash，已验证；验证 pi/opencode-go/deepseek-v4.1-flash，已验证；验证 pi/opencode-go/grok-4.7，已验证
- 蜂群模式：拆分 claude/fable，可用
---CODEX---
走 aicross 引擎：已安装，宿主 codex 有配置。不用盘点，也不看 manifest。
- 共享模式：执行 codex/gpt-5.6-luna，可用
- 经济模式（默认）：出方案 pi/zai-coding-cn/glm-5.3-flash，已验证；执行 codex/gpt-5.6-luna，可用；验收 kimi/kimi-code/k3-256k，可用；验证 kimi/kimi-code/k3-256k，可用
- 蜂群模式：拆分 pi/zai-coding-cn/glm-5.3-flash，已验证
```

（`--host kimi` 与 `--host codex` 相同，仅列出共享模式、经济模式（默认）、蜂群模式，全空非默认模式已过滤；字段 `defaultMode`、`modes[].name`、`stages[][].effective.{agent,provider,model,state}`、`unfilledReason` 均经 `aicross config show` 真机输出核对存在。）

- 未配置宿主与读不了配置两种错误路径（临时 `AICROSS_HOME`）：
  1. 已探测但未配置人选：`aicross config show --host codex` 退出码 `0` 全空，修复后 `dispatch.py start --host codex` 输出 `装了 aicross 但这次用不了：宿主 codex 还没有配人选。把这句告诉用户，改按 manifest 派发。` 并回退清单（退出码 `0`）。
  2. 损坏的 `resources.json`：`aicross config show --host claude` 退出码 `3`，stderr 为 `resources.json 解析失败：key must be a string at line 1 column 2`；`dispatch.py start --host claude` 打印该错误并回退 manifest。
  3. 未知宿主：`aicross config show --host <未知宿主>` 退出码 `2`，stderr 为 `未知 host：...`；`dispatch.py start` 打印原因并回退 manifest。
- **结论：不符合并已修（引擎 `modes.json` 损坏返回 `0` 与扩展宿主问题记入上方引擎侧记录）**。

### 3. `cc-switch` 与重定向的 `claude`

#### 3a. `smoke` 衔接 `verify_model.py`

- 修前实机命令：

```powershell
python skills/ai-cross/references/dispatch.py smoke --entry cc-switch-zhipu-glm.zhipu
python skills/ai-cross/references/verify_model.py --provider "Zhipu GLM"
```

关键输出：

```text
通过  cc-switch-zhipu-glm.zhipu 低档（GLM-5.3-Flash） 13.1s
提示  cc-switch-zhipu-glm.zhipu 是第三方端点，另跑一次真身核对：verify_model.py --provider "Zhipu GLM"
```

不带 `--models` 跑 `verify_model.py` 时检查了数据库中的三个槽位，其中 `sonnet` 与 `opus` 为 `GLM-5.3-Flash[1M]`，端点返回 HTTP 400（`[1211][模型不存在]`）导致失败退出，无法直接衔接。

- 修复后实机重跑命令与关键输出：

```powershell
python skills/ai-cross/references/dispatch.py smoke --entry cc-switch-zhipu-glm.zhipu
python D:/work/aicross/skills/ai-cross/references/verify_model.py --provider "Zhipu GLM" --models "GLM-5.3-Flash"
```

关键输出原文：

```text
通过  cc-switch-zhipu-glm.zhipu 低档（GLM-5.3-Flash） 7.2s
提示  cc-switch-zhipu-glm.zhipu 是第三方端点，另跑一次真身核对：python D:/work/aicross/skills/ai-cross/references/verify_model.py --provider "Zhipu GLM" --models "GLM-5.3-Flash"
...
GLM-5.3-Flash GLM-5.3-Flash ✅ 一致
```

两步命令均退出码 `0`（注：本次真机冒烟同时将本机 `D:\aicross-data\skill\manifest.json` 中该条目的 `smoke` 合法更新为 `ok`，日期 `2026-11-15`；未回显或写入任何密钥）。

- **结论：不符合并已修**。

#### 3b. 重定向的 `claude` 校验

- 本机 `C:\Users\keros68\.claude\settings.json` 未设置 `env.ANTHROPIC_BASE_URL`，未修改用户配置，改为执行对应单元测试：

```powershell
cd skills/ai-cross
python -m unittest tests.test_inventory.TestProbes.test_claude_flags_global_override_without_value tests.test_inventory.TestPick.test_overridden_claude_host_is_not_anthropic tests.test_dispatch.TestProjectFolder.test_redirected_claude_needs_an_explicit_vendor -v
```

关键输出：`Ran 3 tests in 0.022s`、`OK`。

- **结论：本机环境未设该配置，按任务书以单元测试覆盖通过**。

### 4. `agy` 大材料（超过 200 KB）

- 准备一份无敏感内容的 209,294 字节（约 204 KB）Python 示例文件，首部含 `head_avg` 除零缺陷，末尾（`# TAIL_MARKER_T81_215KB`）含 `tail_compute_ratio`（`denominator = 0`）除零缺陷。
- 第一次真实盲审（修前，记录 `20261115-200759`）：
  - 本机 `manifest.json` 对 `agy.google` 记着旧版 `1.3.1`，实际磁盘 `C:\Users\keros68\AppData\Local\agy\bin\agy.EXE` 为 `1.5.9`。脚本误走 `< 1.5` 内联 stdin 分支，结果原文关键行：`代码文件末尾存在截断（在 # Section 1004 处截断 18937 字节），截断后的内容无法核验`，漏掉末尾缺陷。
- 修复 `agy_supports_1_5` 补查实装版本后，第二次真实盲审（`review --entry agy.google --tier low --file <209KB 文件> --go`，临时 `REQUEST.md` 为 7,719 行、210,517 字节）：
  - 命令退出码 `0`，耗时 `23s`，命令行使用 `--settings` 与 `D:/aicross-data/blind/agy-1/REQUEST.md`。
  - `agy` 实际调用了两次 `read_file`（先读头部，再读第 `7600..7719` 行末尾），回答同时准确报出了首部 `head_avg` 与末尾 `tail_compute_ratio` 两处缺陷，没有因为提示词里的「不要使用任何工具」而拒读。临时目录 `%TEMP%\aicross-t81-check4` 已清理。
- **结论：不符合并已修（旧 manifest 版本兜底已修；模板内「不要使用任何工具」本次未触发拒读，保留给用户裁定）**。

### 5. `kimi`

#### 5a. 不带 `--read-dir` 的越界读写与 git 改动提示

- 在临时目录放置目录外文件 `outside.txt`（内容含 `OUTSIDE_MARKER_81=KIMI_CAN_READ_OUTSIDE`），并在临时 git 项目内运行：

```powershell
python skills/ai-cross/references/dispatch.py run --host claude --entry kimi.moonshot --tier low --text-file <task.txt> --project <temp_git_repo> --go
```

- 关键输出与核对结果：耗时 `12s`，退出码 `0`；`kimi` 读出 `OUTSIDE_MARKER_81=KIMI_CAN_READ_OUTSIDE`，在 `<temp_git_repo>` 内新建 `made-by-kimi.txt`（内容同该标记），且命令输出原文包含：

```text
注意：派发期间项目里这些文件有变化：made-by-kimi.txt。kimi 没有只读档，核对是不是它改的。
```

结果与 `REPORT-79`、`REPORT-80` 一致。临时目录已清理。

- **结论：符合**。

#### 5b. 引擎路径下 `kimi` 作执行者（只读代码核对）

- 核对本机 `D:\aicross-data` 配置与 `src/dispatch.rs`、`src/channels/mod.rs`、`src/channels/acp.rs`：
  - 本机当前 `"acpEnabled": false`。引擎按 `uses_acp = (settings.acp_enabled || node.interactive) && acp::supports(agent, node.interactive)` 决定是否走 ACP；关闭 ACP 或启动失败回退时，引擎执行单次 CLI `kimi -p <prompt> -m <model> --output-format stream-json`（`-p` 隐含 `--yolo`），同样可读目录外文件、可改写项目文件，且只读席位未做派发前后 `git status` 变化提醒。开启 ACP 后会通过 `is_read_request` 拦截 `Write`/`Bash`，但 Kimi 默认文件读取仍无目录沙箱。
- **结论：存在同类风险，已记入引擎侧问题（按任务书要求不在本期改 `src/`）**。

### 6. 并发

#### 6a. 并发两条 `dispatch.py run`

- 同时启动两个进程向同一临时 git 项目执行 `dispatch.py run --host claude --entry pi-zai-coding-cn.zhipu --tier low --thinking low --text-file <task.txt> --project %TEMP%\aicross-t81-check6a --go`：
  - 两条命令均退出码 `0`（均输出 `OK`，耗时 `2s`），分别创建了 `.aicross/runs/20261115-201451/` 与 `.aicross/runs/20261115-201451-2/`。
- **结论：符合**。

#### 6b. 12 进程并发调用 `inventory.update_manifest`（Windows `msvcrt.locking`）

- 在临时 `AICROSS_HOME` 下以统一栅栏文件启动 12 个并发 Python 进程，各自在 `inventory.update_manifest` 回调内停留 `0.03s` 并追加一条唯一备注。
- 关键输出：`non_zero: []`、`notes count: 12 unique: 12`。Windows `msvcrt` 文件锁路径工作正常，临时目录已自动清理。
- **结论：符合**。

### 7. 带序号的派发目录（`<时间>-2`）

#### 7a. `dispatch.py verdict`

- 在第 6a 项目录 `%TEMP%\aicross-t81-check6a` 运行：

```powershell
python skills/ai-cross/references/dispatch.py verdict --verdict-file <verdict.md> --project <aicross-t81-check6a>
```

关键输出：`已记录裁决：改 1 件，不改 0 件，以后不再报 0 件 → C:\Users\keros68\AppData\Local\Temp\aicross-t81-check6a\.aicross\runs\20261115-201451-2\裁决.md`（正确命中 `-2`）。

- 进一步核对当 `.aicross/runs/` 同时存在 `20261115-201451-10` 时，原字典序会把 `-10` 排在 `-2` 前而误写进 `-2`。修复 `_run_dir_sort_key` 后重测：
  - `STDOUT: 已记录裁决：改 1 件，不改 0 件，以后不再报 0 件 → C:\Users\keros68\AppData\Local\Temp\aicross-t81-check6a\.aicross\runs\20261115-201451-10\裁决.md`
  - 单测 `TestStartAndVerdict` 8 条全部通过（`Ran 8 tests in 0.278s`、`OK`）。
- **结论：不符合并已修**。

#### 7b. 引擎查看器与 `aicross` 相关命令

- 直接以空的临时 `AICROSS_HOME` 对仅存在于 `<项目>/.aicross/runs/20261115-201451-2/` 的目录调用引擎命令：

```powershell
aicross status 20261115-201451-2
```

关键输出（退出码 `1`）：`读取 run 失败：系统找不到指定的路径。 (os error 3)`。因为引擎查看器（`src/run_view.rs` 的 `list_runs_at`）与 CLI 只读 `$AICROSS_HOME/runs/*.json`，不扫描项目内的 `.aicross/runs/`。

- 在临时 `AICROSS_HOME` 构造最小 `EngineRun`（`run_id` 为 `"20261115-201451-2"`，`pid_started_at` 为 `""`，`project` 指向上述项目目录）及配套 `Graph` 后继续实测：

```powershell
aicross status 20261115-201451-2
aicross recover 20261115-201451-2
aicross adopt 20261115-201451-2 --verdict-file <verdict.md>
```

关键输出：
  - `status`：`rc=0`，输出运行 JSON，stderr `引擎请求完成`。
  - `recover`：`rc=0`，输出 `{"rollback":false,"run_id":"20261115-201451-2","status":"done"}`，stderr `无需核对`。
  - `adopt --verdict-file`：`rc=0`，stdout `{"fix":1,"keep":0,"never":0,"path":"C:\\Users\\keros68\\AppData\\Local\\Temp\\aicross-t81-chk7b-...\\runs\\20261115-201451-2\\裁决.md"}`，写到 `$AICROSS_HOME/runs/...` 而未落到 `<项目>/.aicross/runs/20261115-201451-2/裁决.md`。原因是 `src/project_dir.rs` 的 `find_run_dir` 按 `short_id(run_id)`（取前 8 位字母数字 `"20261115"`）匹配 `-20261115` 后缀，找不到 `-2` 目录。
  - 所有检查完成后，`%TEMP%\aicross-t81-check6a` 与临时 `AICROSS_HOME` 均已删除。
- **结论：不符合未修（引擎与 skill 两条留痕体系分仓存储且 `short_id` 后缀规则不同，只在 skill 侧改目录名无效，按本期不改 `src/` 要求记入报告）**。

### 8. 参数名对照（只列表与给建议，未改代码）

实际捕获命令输出摘要：

- `aicross --version`（退出码 `0`）：`aicross 0.3.7`（源码 `Cargo.toml` 为 `0.3.10`）。
- `aicross dispatch --help`（退出码 `2`）：`STDERR: 缺少 --project`（引擎子命令未接 `--help`，仅顶层 `aicross --help` 输出总览；实际参数按 `src/console_cli.rs` 的 `build_request` 与 `dispatch` 选项解析核对）。
- `dispatch.py review --help` / `dispatch.py run --help`（退出码 `0`）：完整参数见下表。

| 语义 | `aicross dispatch` | `dispatch.py review` | `dispatch.py run` | 现状与建议 |
|---|---|---|---|---|
| 项目目录 | `--project <DIR>`（必填） | `--project <PROJECT>`（选填，默认当前目录） | `--project <PROJECT>`（选填，默认当前目录） | 同名同义；保留差异（skill 可脱离项目独立调用，默认 `cwd` 合理）。 |
| 主任务文本 / 原始材料文本 | `--text-file <FILE>`（必填，禁 `--text`） | `--text-file <TEXT_FILE>`（选填，不成文件的待审原始材料） | `--text-file` / `--task-file <TASK_FILE>`（必填，任务文本） | **语义分叉**：`aicross dispatch` 与 `dispatch.py run` 里 `--text-file` 指「任务文本」，而 `dispatch.py review` 里 `--text-file` 指「不成文件的待审原文」。建议保留 `--text-file` 兼容，在 `review` 增加 `--material-text-file` 或至少维持文档与 help 说明醒目区分。 |
| 附带材料文件 | 无（只有 `--accept-file` 验收条件） | `--file <FILE>`（可重复，待审材料） | `--file <FILE>`（可重复，随任务附参考材料） | 两边职责不同，不强制统一；保留现有命名。 |
| 主 agent（宿主） | `--host <HOST>`（可选，单配置时可推定） | `--host <HOST>`（必填） | `--host <HOST>`（必填） | 同名同义，均认 `claude` 与 `claude-code`。 |
| 任务类型 | `--kind <KIND>`（6 类中英文） | `--kind`（`code` / `general` + 6 类引擎类型） | 无 | `review` 已兼容 6 类；`run` 暂不按类型统计与路由，建议维持现状，若以后接 `stats` 再给 `run` 补 `--kind`。 |
| 只读控制 | `--read-only-task`（标记此次派发为只读任务） | 固定只读盲审 | `--read-dir <READ_DIR>`（给执行者开放指定目录只读工具；不带时除 `codex`/`kimi` 外无工具） | **易混淆**：`--read-only-task` 表示「任务本身不改代码，零改动不算失败」；`--read-dir` 表示「给原本没工具的通道开只读读目录权限」。含义不同，**不要**合并成同名参数；在文档对照写清即可。 |
| 选人 / 档位 / 模式 | `--preset`、`--seat` | `--entry`、`--tier`、`--thinking`、`--n`、`--author` | `--entry`、`--tier`、`--thinking` | 引擎按「模式与位置」派发，skill 按「单路条目与档位」派发，机制不同，**不建议**强行改名对齐；`review` 与 `run` 内部同名已统一。 |
| 子命令帮助 | `--help` 报 `缺少 --project`（`rc=2`） | `--help` 打印用法（`rc=0`） | `--help` 打印用法（`rc=0`） | 建议后续在引擎 `src/console_cli.rs` 为 `dispatch` 等子命令补 `--help` / `-h` 处理，使 `--help` 退出码 `0` 并输出说明。 |

## 改了什么

- `skills/ai-cross/references/dispatch.py`：
  - 第 2 项：新增 `_mode_has_seat(mode)`，`engine_config` 在全空配置时退回 `manifest.json`，`cmd_start` 过滤全空非默认模式。
  - 第 3a 项：`cmd_smoke` 对 `cc-switch` 条目输出带脚本完整路径与 `--models "<条目已登记模型>"` 的真身核对命令。
  - 第 4 项：新增 `agy_supports_1_5(pick)` 与 `_AGY_LIVE_VERSIONS` 缓存，当 manifest 版本低于 `1.5` 且本机可执行文件存在时补查一次 `inventory.cli_version(path)`。
  - 第 7a 项：新增 `_run_dir_sort_key(path)`，使 `cmd_verdict` 对 `<时间>-10` 与 `<时间>-2` 按数字后缀排序。
- `skills/ai-cross/references/channels.md`：补全 `start` 走引擎的条件说明（需至少有一个模式已配人选）。
- `skills/ai-cross/SKILL.md`：版本号 `1.30.3` → `1.30.4`。
- `skills/ai-cross/tests/test_dispatch.py`：新增与第 2、3a、4、7a 项对应的 4 条测试（共 109 条全部通过）。
- `skill-bundle/ai-cross/`：通过 `python scripts/sync-skill.py` 同步 21 个文件。

## 待裁定项

1. **第 8 项参数对照**：是否接受上表建议（保持模式/位置 vs 条目/档位两套参数各自独立；为 `review` 增加文本材料别名并后续在引擎补齐 `aicross dispatch --help`）？
2. **第 4 项 `REQUEST.md` 内模板句**：走文件模式（`agy 1.5+`、`REQUEST.md`）时，文本里是否去掉「材料已全部附在下面，不要使用任何工具」以消除语义冲突（尽管 `gemini-3.8-flash-low` 本次没有拒读，但更严格遵循指令的模型未来可能被误导）？
3. **第 2 项扩展宿主范围**：`dispatch.py` 的 `ENGINE_HOSTS` 目前只有 `claude`、`codex`、`kimi`、`pi`，是否要同步纳入 `src/hosts.rs` 的扩展宿主（需同时调整对应单元测试）？
4. **第 5b 项与第 7b 项引擎侧问题**：是否单开引擎任务处理 `kimi -p` 只读席位未检查 git 改动、`modes.json` 损坏退出码仍为 `0`、`find_run_dir` 与查看器识别 skill 派发记录等问题？
