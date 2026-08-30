# REPORT-01：资源接入

## 完成内容

- 实现 `~/.aicross/resources.json` v1 契约，按 `agent → 模型` 两级保存资源；配置使用临时文件替换，并测试了重复写入。
- 资源 agent 来自工具注册表中的 claude / codex / kimi / pi、cc-switch 桥和用户手动添加项；PowerShell 不进入模型资源清单。
- 实现 Codex `models_cache.json`、Kimi `config.toml`、pi `models-store.json + --list-models + auth check`、cc-switch `cc_switch.py list` 四类事实源解析。Claude 使用 `opus / sonnet / haiku` CLI 别名预设，来源标为 `文档(2026-08-30)`。
- 探测结果按 `(agent, provider, id)` 合并：保留用户修改的 vendor / tier / note / status 和最近冒烟记录；新条目为“待冒烟”；事实源消失的自动条目保留并在备注中标“未在事实源中”；申报条目不被误标。
- Codex 记录描述、priority 和 effort；Kimi 记录别名、真实模型 ID、上下文和 effort；pi 只反序列化模型 ID、provider、reasoning 和 contextWindow，不读取 `~/.pi/auth.json`。
- 实现模型手动输入和手动 agent。手动 agent 写入 `~/.aicross/tools.json` 覆写文件，模型来源自动标为 `申报(日期)`。
- 实现 Claude、Codex、Kimi、pi、cc-switch 冒烟命令。所有资源探测/冒烟子进程使用参数数组，附 `AI_CROSS_PEER=1`；支持 60 秒超时、受限输出和环境变量值脱敏。
- 冒烟判据按通道解析响应体。pi 要求 `agent_settled`、末条 assistant 消息存在非空 `text` 块，并把同一响应中的 provider/model 与请求值比对；thinking 块不算回答。cc-switch 同时检查桥执行结果和 `verify_model.py` 的“全部一致”结论。
- 左栏资源 tab 已替换占位页：agent 展开、探测、手动输入、模型厂商/档位、状态、来源、冒烟日期、冒烟和移除均已接入。
- 顶栏新增“接入资源”面板，显示 agent 与 cc-switch provider；provider 只显示“已配置/未配置”，不显示 key。冒烟默认空选，避免一次误触请求全部模型。
- 设置新增 ai-cross 目录；留空时按 `~/.claude/skills/ai-cross`、`~/.codex/skills/ai-cross`、`D:\work\agentteam\dist\ai-cross` 查找。
- `write_terminal` 已改为 async；PTY writer 使用独立锁，`write_all + flush` 不再占用全局 PTY 状态锁。`cancelled` 改为带时间的集合，并按 5 分钟 TTL 懒清理。
- 添加 8 个脱敏 fixture，未包含 key、token 或带凭据的 baseUrl；新增事实源、合并、冒烟解析、配置替换和取消项过期测试。

## 自动验证

### Rust 检查

命令：

```text
cargo check --manifest-path src-tauri/Cargo.toml
```

结果原文：

```text
    Checking aicross v0.0.0 (D:\work\aicross\src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.13s
```

### Rust 测试

命令：

```text
cargo test --manifest-path src-tauri/Cargo.toml -- --nocapture
```

结果原文：

```text
warning: linker stdout: 正在创建库 D:\work\aicross\src-tauri\target\debug\deps\aicross_lib.dll.lib 和对象 D:\work\aicross\src-tauri\target\debug\deps\aicross_lib.dll.exp
  |
  = note: `#[warn(linker_messages)]` on by default

warning: `aicross` (lib) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 9.99s
     Running unittests src\lib.rs (src-tauri\target\debug\deps\aicross_lib-e6e129a316898a1b.exe)

running 19 tests
test resources::tests::live_codex_and_kimi_fact_sources_parse ... ignored, explicit local fact-source check
test pty::tests::cancelled_terminal_ids_expire ... ok
test resources::tests::other_smoke_parsers_apply_channel_specific_success_rules ... ok
test resources::tests::merge_preserves_user_fields_adds_pending_and_marks_missing ... ok
test resources::tests::parses_four_local_fact_sources ... ok
test tests::executable_extensions_require_confirmation_case_insensitively ... ok
test workspace::tests::compare_and_diff_cover_added_deleted_mtime_binary_and_hunk_counts ... ok
test resources::tests::pi_smoke_rejects_identity_mismatch_and_thinking_only_answer ... ok
test config::tests::damaged_settings_do_not_prevent_loading ... ok
test sessions::tests::sessions_sort_by_instant_deduplicate_and_skip_garbage ... ok
test sessions::tests::pi_parser_keeps_only_requested_metadata ... ok
test workspace::tests::snapshot_reports_modified_text_and_diff ... ok
test config::tests::json_config_can_be_replaced_repeatedly ... ok
test config::tests::tools_overrides_merge_existing_and_new_entries_and_report_invalid_json ... ok
test workspace::tests::safe_path_rejects_traversal_absolute_paths_and_external_symlinks ... ok
test pty::tests::portable_pty_runs_a_child_and_reads_output ... ok
test workspace::tests::large_utf8_text_is_not_reported_as_binary ... ok
test pty::tests::powershell_launch_uses_registered_default_args ... ok
claude: detected=true, version=2.1.251 (Claude Code)
codex: detected=true, version=codex-cli 0.149.1
kimi: detected=true, version=0.39.1
pi: detected=true, version=0.84.2
powershell: detected=true, version=PowerShell 7.6.5
test tools::tests::embedded_registry_detects_all_five_tools ... ok

test result: ok. 18 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 2.90s

     Running unittests src\main.rs (src-tauri\target\debug\deps\aicross-a58949d6294ae227.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests aicross_lib

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

默认忽略的测试只读取本机 Codex/Kimi 事实源，已在“本机实测”中单独执行。

### Rust 格式检查

命令：

```text
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```

结果：退出码 0，无输出。

### 前端构建

命令：

```text
cd src-ui
npm run build
```

结果原文：

```text
> aicross-ui@0.0.0 build
> tsc && vite build

(node:36132) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
vite v7.3.6 building client environment for production...
transforming...
✓ 47 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                   0.45 kB │ gzip:   0.29 kB
dist/assets/index-B93QKmsr.css   17.50 kB │ gzip:   4.21 kB
dist/assets/index-Hz4mD7dg.js   514.87 kB │ gzip: 143.09 kB

(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rollupOptions.output.manualChunks to improve chunking: https://rollupjs.org/configuration-options/#output-manualchunks
- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
✓ built in 1.43s
```

构建成功；保留上一期已有的单 chunk 体积警告。

### 额外 Rust lint

命令：

```text
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

结果原文：

```text
    Checking aicross v0.0.0 (D:\work\aicross\src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.57s
```

## 本机实测

### Codex / Kimi 事实源

命令：

```text
cargo test --manifest-path src-tauri/Cargo.toml resources::tests::live_codex_and_kimi_fact_sources_parse -- --ignored --nocapture
```

关键结果原文：

```text
running 1 test
live codex models=9
live kimi models=4
test resources::tests::live_codex_and_kimi_fact_sources_parse ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.01s
```

Codex/Kimi 当前本机事实源均能由本期解析器读出清单。

### cc-switch 清单

命令运行 `python D:\work\agentteam\dist\ai-cross\references\cc_switch.py list`，只统计非敏感字段。结果：

```text
providers     : 10
configured    : 3
mapped_models : 4
```

未输出或保存 token；资源解析结构不包含 endpoint 字段。

### pi 清单

命令：

```text
pi --list-models
```

结果未通过。当前受控环境允许读取事实源文件，但 pi 自身会在读取设置/鉴权时创建锁目录；该写入被沙箱拒绝：

```text
Warning: (startup session lookup, global settings) EPERM: operation not permitted, mkdir 'C:\Users\keros68\.pi\agent\settings.json.lock'
Warning: errors loading models.json:
Availability refresh: EPERM: operation not permitted, mkdir 'C:\Users\keros68\.pi\agent\auth.json.lock'
EPERM: operation not permitted, mkdir 'C:\Users\keros68\.pi\agent\auth.json.lock'
```

因此本机 `--list-models` 的 provider/模型输出未验证。fixture 已覆盖“只保留 `--list-models` 可用项、models-store 补元数据”的合并规则，但不能替代本机命令成功。

### pi `zai-coding-cn/glm-4.7` 冒烟

按任务书命令形态实际尝试 1 次：

```text
$env:AI_CROSS_PEER='1'
'只回复 OK' | pi --provider zai-coding-cn --model glm-4.7 -p --no-tools --no-extensions --no-context-files --no-session --mode json
```

结果原文：

```text
Warning: (startup session lookup, global settings) EPERM: operation not permitted, mkdir 'C:\Users\keros68\.pi\agent\settings.json.lock'
Warning: (runtime creation, global settings) EPERM: operation not permitted, mkdir 'C:\Users\keros68\.pi\agent\settings.json.lock'
{"type":"session","version":3,"id":"01a05294-76c7-7b62-b599-ed98bbc1d6eb","timestamp":"2026-08-30T12:10:56.583Z","cwd":"D:\\work\\aicross"}
Warning: (runtime creation, global settings) EPERM: operation not permitted, mkdir 'C:\Users\keros68\.pi\agent\settings.json.lock'
Credential store read failed for zai-coding-cn: EPERM: operation not permitted, mkdir 'C:\Users\keros68\.pi\agent\auth.json.lock'
```

失败发生在读取 credential store 的锁文件阶段，没有出现 `agent_settled` 或 assistant 响应，未到模型请求成功阶段；未记录 `smoke_model`。本期实际发起 1 次 CLI 冒烟尝试，成功模型请求 0 次，总数不超过 5 次。

## 独立审查

只读审查发现并已修正：接入面板默认全选会导致批量真实请求、cc-switch 只看退出码、pi 身份未绑定 assistant 响应、输出线程可能破坏超时、auth 空输出误判就绪、工具检测缺少 `AI_CROSS_PEER`。修正后由主实现者重新运行 Rust 测试、前端构建与 clippy。

## 未做 / 未验证

- 未通过本机 pi `--list-models` 验收；原因是当前沙箱拒绝 pi 在用户配置目录创建锁文件。
- 未完成 `zai-coding-cn/glm-4.7` 的真实成功冒烟，也未写入实际 `smoke_model`。没有读取、复制或改写 `~/.pi/auth.json` 来绕过限制。
- 未在普通桌面窗口中人工点击资源 tab、接入面板、手动输入和移除；前后端接口已编译，界面未做人工窗口 smoke。
- 未通过 Tauri 窗口实际写入用户目录下的 `~/.aicross/resources.json` / `tools.json`；重复持久化、合并和字段契约由临时目录测试覆盖。
- 未运行 Claude、Codex、Kimi 或 cc-switch 的真实模型冒烟，避免为本期验收增加不必要请求。
- 未修改第 0 期 3 个既有未跟踪审查文件。
- 未 commit，未 push。

## 不确定处

- pi 与 cc-switch 的 CLI/输出格式会漂移；当前解析器对已知 JSON/JSONL 结构做严格成功判定，未知 auth readiness 结构会报未就绪，不会按 exit 0 猜成功。
- 资源页新增后，前端单 chunk 为 514.87 kB；构建成功，但后续功能继续增长时需要再评估拆包。

## 修正轮 R1

### 逐条处理结果

#### 需求与数据正确性

1. **1.1 已修**：`ResourceModel` 增加独立 `meta` 字段。探测说明、事实源缺失标记、鉴权状态和冒烟失败摘要写入 `meta`；`note` 只保存用户备注。二次探测刷新 `meta`，保留 `note`、tier、状态和冒烟记录。旧 `resources.json` 没有 `meta` 时按空字符串读取。
2. **1.2 已修**：pi auth check 改为逐 provider 判断。未就绪 provider 的模型保留，在对应模型 `meta` 标记“鉴权未就绪”，并在操作错误中记录该 provider；不再使整个 pi 探测失败。
3. **1.3 已修**：移除 `pi auth check` 的 `--no-refresh`。未运行 help，因为已直接删除该非任务书参数。
4. **1.4 已修**：待冒烟和冒烟失败只显示“冒烟”；已接入显示“移除”和“重新冒烟”。
5. **1.5 已修**：Claude 预设的系统元数据标明“CLI 别名”。
6. **1.6 已修**：读取 Codex 缓存顶层 `fetched_at`，写入各模型 `meta`。
7. **1.7 未补做**：没有重新运行真实 pi 模型冒烟；本轮明确要求不发送任何模型请求。原报告中的本机 pi 验收限制继续有效。
8. **1.8 已修**：接入面板“检测到的 agent”只列 `detected=true` 的 agent，cc-switch 单独放在 provider 区。
9. **2.1 已修**：所有资源探测、手动输入、更新、移除、单条冒烟、批量冒烟和手动 agent 添加共用一把资源操作锁，读、处理、保存和内存回写整体串行。冒烟期间到达的用户操作排队后从最新清单执行，不会被旧快照覆盖。
10. **2.2 已修**：只勾 cc-switch provider 时会自动纳入 `cc-switch` agent；若选择项没有可冒烟模型，返回明确错误。新增 provider-only 选择测试。
11. **2.3 已修**：pi `--list-models` 按空白 token 的精确 `provider/id` 或相邻 provider、model 列匹配，不再使用子串。新增 `glm-4` / `glm-4.7` 前缀回归测试。
12. **2.4 已修**：stdin broken pipe 视为子进程提前退出，继续收集 stdout/stderr；新增测试确认真实 stderr 保留。
13. **2.5 已修**：`ProcessResult` 保存实际 timeout，20 秒和 60 秒进程分别生成真实超时文案。
14. **2.6 已修**：统一读取线程在子进程退出或杀树后完整接收并 join，不再以 2 秒超时丢弃 JoinHandle 和输出。
15. **2.7 已修**：统一子进程入口设置 `PYTHONUTF8=1`；Python 子进程的中文管道输出按 UTF-8 产生。
16. **2.8 已修**：Kimi 配置改用 `toml` crate 解析，支持行内注释和多行数组；新增带行内注释 fixture。
17. **2.9 已修**：Codex 空 `models` 清单返回错误，不调用缺失标记合并；新增空清单断言。
18. **2.10 已修**：Claude CLI 未检测到时直接结束该 agent 探测，不合并预设。
19. **2.11 已修**：手动 agent 在同一资源操作锁内准备 tools/resources 两份新状态；先原子替换 `resources.json`，再原子替换 `tools.json`，第二步失败时恢复旧 `resources.json`，两份内存状态只在文件写入全部成功后一起更新。

#### 安全、边界与质量

1. **4.3 已修**：`safe_summary` 增加正则脱敏，覆盖 `sk-[A-Za-z0-9_-]{8,}`、URL `user:pass@`、`key`/`token` 查询参数和 `Bearer` token；新增回归测试。
2. **4.4 已修**：pi auth 就绪只认每个 JSON 对象的顶层 `ready: true`；`ready: false`、嵌套 true、`status: ready` 和嵌套 `ok: true` 均不算就绪。仓库没有 auth-check fixture，因此采用审查建议的安全侧顶层字段。
3. **已修**：新增 `process.rs`，统一资源子进程、版本探测和 Windows 进程树终止；`resources.rs`、`tools.rs`、`pty.rs` 共用该模块。
4. **已修**：批量冒烟改为在一份 store 上逐条原位回写，不再每个模型克隆整库。
5. **已修**：冒烟后 cc-switch provider 刷新失败返回 `null`，前端保留已有 provider，不再用空数组覆盖。
6. **已修**：Rust 和 TypeScript 都只保留一个 `ResourceModelRequest` 类型，添加与更新共用。
7. **已修**：手动 agent 启动参数支持单引号和双引号，含空格的 Windows 路径可作为一个参数；未闭合引号返回明确错误。
8. **已修**：pi 手动模型 provider 为空时拒绝添加；旧数据若 provider 为空，冒烟前也返回“pi 模型缺少 provider，无法冒烟”。
9. **已修**：`provider（可空）` 改为 `provider（可留空）`。

#### 新增测试

1. `process::tests::*`：覆盖超时杀树且不留下延迟写文件的子进程、broken pipe 后保留 stderr、每个输出流 1 MiB 截断。
2. `pi_source_matching_uses_exact_tokens_and_auth_requires_top_level_ready`：覆盖模型 ID 前缀不误报、auth 顶层 `ready` 判定。
3. `detection_refreshes_meta_preserves_note_and_parses_commented_kimi_toml`：覆盖二次探测刷新元数据、保留用户备注、旧 JSON 兼容和 Kimi 行内注释 TOML。
4. `provider_only_selection_includes_cc_switch_models`：覆盖只选 provider 时纳入 cc-switch 模型，以及 pi 空 provider 拒绝。
5. `safe_summary_redacts_common_cli_secret_shapes`：覆盖四类新增脱敏形态和真实 timeout 文案。

本轮没有运行任何 Claude、Codex、Kimi、pi 或 cc-switch 模型请求。

### 最终自动验证

#### Rust 检查

命令：

```text
cargo check --manifest-path src-tauri/Cargo.toml
```

结果原文：

```text
    Checking aicross v0.0.0 (D:\work\aicross\src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.89s
```

#### Rust 测试

命令：

```text
cargo test --manifest-path src-tauri/Cargo.toml -- --nocapture
```

结果原文：

```text
   Compiling aicross v0.0.0 (D:\work\aicross\src-tauri)
warning: linker stdout: 正在创建库 D:\work\aicross\src-tauri\target\debug\deps\aicross_lib.dll.lib 和对象 D:\work\aicross\src-tauri\target\debug\deps\aicross_lib.dll.exp
  |
  = note: `#[warn(linker_messages)]` on by default

warning: `aicross` (lib) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 7.98s
     Running unittests src\lib.rs (src-tauri\target\debug\deps\aicross_lib-303a603e9ddec3ac.exe)

running 26 tests
test resources::tests::live_codex_and_kimi_fact_sources_parse ... ignored, explicit local fact-source check
test resources::tests::other_smoke_parsers_apply_channel_specific_success_rules ... ok
test pty::tests::cancelled_terminal_ids_expire ... ok
test resources::tests::provider_only_selection_includes_cc_switch_models ... ok
test resources::tests::pi_smoke_rejects_identity_mismatch_and_thinking_only_answer ... ok
test resources::tests::pi_source_matching_uses_exact_tokens_and_auth_requires_top_level_ready ... ok
test resources::tests::merge_preserves_user_fields_adds_pending_and_marks_missing ... ok
test resources::tests::detection_refreshes_meta_preserves_note_and_parses_commented_kimi_toml ... ok
test resources::tests::parses_four_local_fact_sources ... ok
test config::tests::damaged_settings_do_not_prevent_loading ... ok
test sessions::tests::pi_parser_keeps_only_requested_metadata ... ok
test tests::executable_extensions_require_confirmation_case_insensitively ... ok
test config::tests::tools_overrides_merge_existing_and_new_entries_and_report_invalid_json ... ok
test workspace::tests::compare_and_diff_cover_added_deleted_mtime_binary_and_hunk_counts ... ok
test sessions::tests::sessions_sort_by_instant_deduplicate_and_skip_garbage ... ok
test resources::tests::safe_summary_redacts_common_cli_secret_shapes ... ok
test config::tests::json_config_can_be_replaced_repeatedly ... ok
test workspace::tests::snapshot_reports_modified_text_and_diff ... ok
test pty::tests::portable_pty_runs_a_child_and_reads_output ... ok
test workspace::tests::safe_path_rejects_traversal_absolute_paths_and_external_symlinks ... ok
test workspace::tests::large_utf8_text_is_not_reported_as_binary ... ok
test pty::tests::powershell_launch_uses_registered_default_args ... ok
test process::tests::run_process_keeps_stderr_after_broken_pipe ... ok
test process::tests::run_process_truncates_each_output_stream ... ok
test process::tests::run_process_times_out_and_terminates_the_child_tree ... ok
claude: detected=true, version=2.1.251 (Claude Code)
codex: detected=true, version=codex-cli 0.149.1
kimi: detected=true, version=0.39.1
pi: detected=true, version=0.84.2
powershell: detected=true, version=PowerShell 7.6.5
test tools::tests::embedded_registry_detects_all_five_tools ... ok

test result: ok. 25 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 4.05s

     Running unittests src\main.rs (src-tauri\target\debug\deps\aicross-7760f71de8d14d2b.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests aicross_lib

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

默认忽略的测试只读取本机 Codex/Kimi 事实源；本轮未执行该显式本机测试。

#### Rust 格式检查

命令：

```text
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```

结果原文：退出码 0，无输出。

#### Rust lint

命令：

```text
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

结果原文：

```text
    Checking aicross v0.0.0 (D:\work\aicross\src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.49s
```

#### 前端构建

命令：

```text
cd src-ui
npm run build
```

结果原文：

```text
> aicross-ui@0.0.0 build
> tsc && vite build

(node:39036) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
vite v7.3.6 building client environment for production...
transforming...
✓ 47 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                   0.45 kB │ gzip:   0.28 kB
dist/assets/index-BUZGShvl.css   17.58 kB │ gzip:   4.22 kB
dist/assets/index-enTvYRRB.js   515.61 kB │ gzip: 143.39 kB

(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rollupOptions.output.manualChunks to improve chunking: https://rollupjs.org/configuration-options/#output-manualchunks
- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
✓ built in 1.29s
```

构建成功；单 chunk 体积警告仍在。本轮未 commit、未 push。
