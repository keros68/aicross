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
