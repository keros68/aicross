# REPORT-04 — 经济流水线

日期：2026-09-13

## 结果

第 4 期已在第 2 期对话图和第 3 期派发引擎上实现，没有另建流水线引擎。经济模式现在按图层自动推进“出方案 → 执行 → 验收”，执行节点可交付结构化产物，盲验收通过独立目录接收待验文件，G4 会把修正轮作为新节点继续长在图上。

四项自动验证的最终结果：

- `cargo check`：通过。
- `cargo test`：通过，133 passed、6 ignored。
- `npx vitest run`：沙箱内因 `spawn EPERM` 未能加载配置；按修正单允许的 fallback 复跑，通过，8 files、78 tests。
- `npm run build`：通过。

真机经济流水线和 cc-switch 路均已由编排者在沙箱外真跑通过；本修正轮没有重复发送真机请求。

## 改了什么

### 阶段推进与图上闭环

- 派发浮层沿用 `dispatchableNodeIds`，每次只派父节点全部完成的 draft 层；一层完成后自动计算并派下一层。
- 请求中的 `stage` 改为按图深度计算，不再固定为 1；结果和用量表使用后端返回的实际阶段。
- 验收结论解析支持 `PASS`、`NEEDS_FIX` 和未知。未知或 PASS 交回用户；NEEDS_FIX 会在验收节点下追加一对可见的执行、验收节点，最多追加 3 轮、6 个节点。
- 验证命令失败时标记 `validation_failed`，不派当前验收节点，图上追加下一次修正与验收节点；最多重试 3 次。
- `STATE.md` 可识别未完成 run。打开项目后中栏显示“上次派发未完成”，提供“继续”和“放弃”。

### 执行产物与写入隔离

- 单写者执行前后拍项目目录快照，产出统一 diff、相对路径与改后全文、stdout 摘要；单文件大于 200 KB 时不附全文。
- worktree 节点用 Git worktree 在运行目录创建独立工作树；非 Git 项目或创建失败时退回单写者并返回提示。本期只显示手动合并指引。
- 同一项目的单写者由引擎级全局锁互斥；同阶段排队时状态写为“等待写锁”。
- Codex 写节点使用 `workspace-write`，只读节点继续使用 `read-only`。
- 盲验收目录只写 `REQUEST.md`、`diff.patch` 和 `changed/`。原始请求逐字交付，执行产物不塞入 prompt 正文，方案文本默认不交给盲验收。
- 普通盲节点的附件若全是文本且总量小于 30 KB，会继续复制到盲目录并同时内联进 prompt；Codex 额外收到“材料已在正文，勿再用工具读取”。
- 结果屏按方案、产物、验收意见分栏；diff 可展开，显示验证输出、worktree 手动合并提示和输入超过 50k tokens 的告警。

### 角色配置与经济规则

- 设置新增“角色”页，配置出方案、执行、验收各自的 agent/provider/model、档位和 thinking，保存到 `~/.aicross/roles.json`。
- 经济模式的空目标从角色配置填充，执行节点默认写权限改为“单写者”。
- 三类固定 prompt 模板已接入：方案节点输出任务契约；执行节点要求精确数值必须写代码并执行；验收节点使用固定六维清单并在末尾回答是否完整实现。
- G1 增加小任务提示、链式任务 thinking 自动调低、同厂商复核提示、角色默认/用户指定理由和宽泛脱敏可能连带替换的提示。
- “归类模型”设置已置灰并标明尚未接入。

### 第 3 期遗留

- 通道解析错误增加类型化 `ChannelErrorKind`，派发层不再靠中文前缀判断 `identity_mismatch`；结果屏单独显示“真身不符”。
- cc-switch preview 会检查配置路径；usage 解析改为从 usage 行逐项取 `input/cache_read/output`，不依赖字段顺序。
- running 浮层可“收起”，后台继续执行，中栏恢复可操作；收起后可“展开派发”。
- 生产派发入口统一由 `buildRequest` 调用 `serializeContext`，盲节点使用 `BLIND_PROMPT_PREFIX`；新增前端测试直接覆盖这条生产序列化路径。
- pi 续接的静态 argv 测试确认 `--session-id` 与 `--thinking high` 同时保留；本期没有再发 pi 真机请求，因此只确认构造路径，不把 thinking 的服务端实际效果写成真机已验证。

### 安全边界

- 没有读取 `~/.pi/auth.json`、`~/.cc-switch` 或其他凭据文件。
- 实现没有回显、记录或写入环境变量值；密钥仍只由子进程环境继承。
- 没有启动窗口或运行 `tauri dev`，没有 commit 或 push。

## 对第 2/3 期契约的字段追加

本期追加了两个可选字段：`Reply.artifacts` 保存结构化执行产物；修正轮 R2 追加 `Reply.error_kind` 保存失败分类。结构为：

```text
artifacts {
  diff,
  changed_files: [{ path, content }],
  worktree?,
  stdout_summary
}
error_kind?: "identity_mismatch" | "validation_failed" | "error" | "timeout" | 其他后端失败状态
```

既有 `Reply` 字段没有改名或删除；`error_kind` 缺省时仍按普通“失败”显示，兼容旧图。阶段、验证输出、原始请求和上游产物等字段属于本期派发 IPC，不是第 2/3 期图持久化契约的额外 `Reply` 字段。

## 验收 1：四项自动验证

#### `cargo check`（仓库根目录，exit=0）

输出原文：

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.93s
```

#### `cargo test`（仓库根目录，exit=0）

输出原文：

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 6.87s
     Running unittests src\main.rs (target\debug\deps\aicross-a390c24a3e96ccb6.exe)

running 134 tests
test channels::tests::cc_switch_requires_configured_script_path ... ok
test channels::tests::blind_claude_contains_restricted ... ok
test channels::tests::pi_continuation_preserves_thinking_setting ... ok
test channels::tests::blind_pi_with_no_context_files_is_fully_isolated ... ok
test channels::tests::kimi_requires_configured_alias ... ok
test channels::tests::claude_null_api_error_status_is_not_an_error ... ok
test channels::tests::codex_high_maps_to_high_and_never_to_xhigh ... ok
test channels::tests::codex_resume_follows_parent_options ... ok
test channels::tests::pi_error_event_surfaces_error_message ... ok
test channels::tests::kimi_reads_plain_string_assistant_content_and_resume_hint ... ok
test channels::tests::pi_response_model_mismatch_fails_and_reports_true_identity ... ok
test channels::tests::pi_reads_camel_case_cache_usage ... ok
test channels::tests::pi_uses_explicit_session_and_app_owned_session_directory ... ok
test channels::tests::kimi_rejects_more_than_30000_utf8_bytes ... ok
test dispatch::tests::live_blind_review ... ignored, real model requests; run once for TASK-03 acceptance
test dispatch::tests::live_cc_switch_route ... ignored, one real cc-switch request; requires AICROSS_CC_SWITCH_PATH and provider
test dispatch::tests::live_economy_pipeline ... ignored, three real model requests; run once outside the sandbox for TASK-04 acceptance
test dispatch::tests::live_shared_continuation ... ignored, two real model requests; run once for TASK-03 acceptance
test channels::tests::scheduling_aliases_normalize_to_one_agent_key ... ok
test channels::tests::writable_codex_uses_workspace_write_sandbox ... ok
test dispatch::tests::classification_parser_tolerates_fences_and_missing_arrays ... ok
test dispatch::tests::frontmatter_roundtrips ... ok
test dispatch::tests::snapshot_builds_diff_and_changed_file_content ... ok
test channels::tests::kimi_session_parser_ignores_resume_text_in_answer ... ok
test channels::tests::cc_switch_reads_usage_from_stderr ... ok
test channels::tests::kimi_collects_stream_usage_when_present ... ok
test channels::tests::kimi_resume_uses_current_cli_session_flag ... ok
test dispatch::tests::broad_redaction_warning_only_counts_unquoted_whole_lines ... ok
test dispatch::tests::trace_fields_are_always_sanitized ... ok
test hook_installer::tests::codex_features_enables_hooks_and_strips_legacy ... ok
test hook_installer::tests::codex_features_creates_section_if_missing ... ok
test launch::tests::ignores_non_launch_argv ... ok
test launch::tests::parses_equals_form_and_validates_cwd ... ok
test launch::tests::parses_tool_with_and_without_argv0 ... ok
test launch::tests::rejects_unknown_tool ... ok
test dispatch::tests::redacts_all_six_secret_shapes_and_stays_clean ... ok
test hook_installer::tests::codex_features_preserves_user_hooks_false_overwrites ... ok
test hook_installer::tests::codex_hooks_json_preserves_user_hooks ... ok
test hook_installer::tests::kimi_config_creates_with_9_events ... ok
test hook_installer::tests::kimi_config_blocks_have_only_allowed_fields ... ok
test hook_installer::tests::codex_hooks_json_creates_with_4_events ... ok
test orchestration::tests::dispatchable_broken_file_reports_error ... ok
test hook_installer::tests::codex_hooks_json_unparseable_left_untouched ... ok
test orchestration::tests::dispatchable_filters_connected_only ... ok
test dispatch::tests::blind_directory_contains_only_selected_materials ... ok
test orchestration::tests::dispatchable_missing_file_reports_error ... ok
test dispatch::tests::state_uses_actual_limit_and_separates_failures ... ok
test orchestration::tests::load_missing_graph_returns_none ... ok
test orchestration::tests::presets_missing_file_returns_builtins ... ok
test hook_installer::tests::codex_features_idempotent ... ok
test orchestration::tests::presets_broken_file_returns_builtins ... ok
test orchestration::tests::save_rejects_bad_ids ... ok
test server::resume_cwd_tests::does_not_split_on_a_dot_inside_a_component ... ok
test server::resume_cwd_tests::does_not_touch_an_ordinary_hidden_dir_without_worktrees ... ok
test dispatch::tests::blind_acceptance_directory_contains_only_artifact_delivery ... ok
test server::resume_cwd_tests::handles_mixed_separators_and_non_ascii ... ok
test server::resume_cwd_tests::leaves_a_plain_project_dir_unchanged ... ok
test orchestration::tests::save_rejects_wrong_version ... ok
test server::resume_cwd_tests::leaves_a_real_subdir_unchanged ... ok
test hook_installer::tests::kimi_config_reinstall_strips_stale_keeps_user_hooks ... ok
test orchestration::tests::graph_roundtrip ... ok
test hook_installer::tests::kimi_config_preserves_existing_content ... ok
test server::resume_cwd_tests::requires_worktrees_to_be_a_whole_next_component ... ok
test server::resume_cwd_tests::strips_only_dot_claude_worktrees_not_other_dot_dir_worktrees ... ok
test orchestration::tests::presets_wrong_version_returns_builtins ... ok
test server::resume_cwd_tests::strips_unix_worktree_cwd_to_project_root ... ok
test server::resume_cwd_tests::strips_windows_worktree_cwd_to_project_root ... ok
test server::tests::codex_desktop_files_only_block_returns_empty ... ok
test server::tests::codex_desktop_plain_block_passes_through_unchanged ... ok
test orchestration::tests::list_graphs_skips_broken_files ... ok
test hook_installer::tests::codex_hooks_json_idempotent_no_accumulation ... ok
test dispatch::tests::preparation_failure_records_only_unsent_placeholder ... ok
test hook_installer::tests::kimi_config_idempotent_no_duplicates ... ok
test resources::tests::connected_filters_status ... ok
test orchestration::tests::presets_roundtrip ... ok
test server::resume_cwd_tests::map_is_empty_when_claude_json_is_missing ... ok
test server::tests::codex_parser_drops_subagent_source_unit_variant ... ok
test server::resume_cwd_tests::map_is_empty_and_does_not_panic_on_malformed_json ... ok
test server::tests::codex_parser_drops_subagent_source_object ... ok
test server::resume_cwd_tests::map_has_no_entry_for_an_unregistered_folder ... ok
test server::tests::strips_codex_desktop_file_preamble_to_real_request ... ok
test shell_probe::tests::end_to_end_detects_installed_pwsh_with_version ... ignored
test shell_probe::tests::finds_msix_pwsh_alias ... ignored
test shell_probe::tests::auto_is_default_for_unknown_string ... ok
test server::tests::codex_parser_drops_subagent_thread_source ... ok
test shell_probe::tests::json_mirror_hides_paths ... ok
test shell_probe::tests::parses_windows_ids ... ok
test shell_probe::tests::resolve_gitbash_falls_back_when_absent ... ok
test shell_probe::tests::resolve_cmd_uses_system_path ... ok
test shell_probe::tests::resolve_powershell_is_absolute ... ok
test skills::tests::rel_path_validation ... ok
test skills::tests::skill_name_validation ... ok
test terminal::tests::find_preset_known_tools ... ok
test server::resume_cwd_tests::map_resolves_real_world_hyphenated_project_path ... ok
test terminal::tests::find_preset_resume_program_matches_tool ... ok
test server::resume_cwd_tests::map_resolves_second_real_world_project_path ... ok
test terminal::tests::claude_token_valid_uuid ... ok
test terminal::tests::find_preset_unknown_returns_none ... ok
test orchestration::tests::save_strips_builtin_entries ... ok
test terminal::tests::claude_token_rejects_injection ... ok
test terminal::tests::osc7_absent_returns_none ... ok
test terminal::tests::osc7_basic_with_hostname ... ok
test terminal::tests::osc7_embedded_in_larger_buffer ... ok
test terminal::tests::osc7_no_hostname ... ok
test terminal::tests::osc7_percent_encoded_space ... ok
test terminal::tests::osc7_percent_encoded_chinese ... ok
test terminal::tests::osc7_st_terminator ... ok
test terminal::tests::opencode_token_valid_format ... ok
test terminal::tests::osc7_windows_style_path ... ok
test tool_config::tests::empty_entry_check ... ok
test server::tests::codex_parser_keeps_legacy_session_without_thread_source ... ok
test tool_config::tests::expand_path_handles_tilde_and_passthrough ... ok
test tool_config::tests::parse_simple ... ok
test server::tests::codex_parser_keeps_user_fork_session ... ok
test windows_path::tests::already_present_skipped_case_insensitive_and_trailing_slash ... ok
test windows_path::tests::blank_entries_in_current_dont_shadow_candidates ... ok
test windows_path::tests::duplicate_candidates_deduped ... ok
test windows_path::tests::empty_candidates_returns_current_unchanged ... ok
test windows_path::tests::mixed_present_and_new ... ok
test windows_path::tests::empty_current_joins_all_candidates ... ok
test terminal::tests::opencode_token_rejects_invalid ... ok
test orchestration::tests::roles_roundtrip_and_fill_economy_nodes ... ok
test server::tests::codex_parser_keeps_user_main_session ... ok
test server::tests::counts_array_content_user_message_with_mixed_blocks ... ok
test server::tests::drops_pure_compaction_subtask_session ... ok
test windows_path::tests::real_path_dirs_reads_registry ... ok
test server::tests::keeps_real_session_with_real_user_message ... ok
test server::tests::keeps_session_that_continued_after_compaction ... ok
test terminal::tests::hermes_token_valid_format ... ok
test terminal::tests::hermes_token_rejects_invalid ... ok
test server::tests::drizzle_scanner_excludes_subagent_and_archived_sessions ... ok
test dispatch::tests::project_write_lock_is_mutually_exclusive ... ok
test process::tests::run_process_truncates_and_marks_output ... ok
test process::tests::run_process_times_out_and_terminates_the_child_tree ... ok

test result: ok. 128 passed; 0 failed; 6 ignored; 0 measured; 0 filtered out; finished in 4.42s
```

 #### `cd src-ui && npx vitest run`（src-ui，exit=1）
#### `cd src-ui && npx vitest run`（src-ui，exit=1）

输出原文：

```text
failed to load config from D:\work\aicross-p4\src-ui\vite.config.ts

⎯⎯⎯⎯⎯⎯⎯ Startup Error ⎯⎯⎯⎯⎯⎯⎯⎯
Error: Build failed with 1 error:

[plugin externalize-deps]
Error: spawn EPERM
    at ChildProcess.spawn (node:internal/child_process:458:11)
    at spawn (node:child_process:813:9)
    at Object.execFile (node:child_process:349:17)
    at exec (node:child_process:236:25)
    at optimizeSafeRealPathSync (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:1954:2)
    at windowsSafeRealPathSync (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:1940:3)
    at getRealPath (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:32413:36)
    at tryResolveRealFileOrType (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:32407:9)
    at tryCleanFsResolve (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:32151:21)
    at tryFsResolve (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:32144:14)
    at aggregateBindingErrorsIntoJsError (file:///D:/work/aicross-p4/src-ui/node_modules/rolldown/dist/shared/error-DBGOT6sf.mjs:48:18)
    at unwrapBindingResult (file:///D:/work/aicross-p4/src-ui/node_modules/rolldown/dist/shared/error-DBGOT6sf.mjs:18:128)
    at #build (file:///D:/work/aicross-p4/src-ui/node_modules/rolldown/dist/shared/rolldown-build-hRnqgxyz.mjs:3313:34)
    at async bundleConfigFile (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:34633:17)
    at async bundleAndLoadConfigFile (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:34541:18)
    at async loadConfigFromFile (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:34510:42)
    at async resolveConfig (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:34134:22)
    at async resolveConfig$1 (file:///D:/work/aicross-p4/src-ui/node_modules/vitest/dist/chunks/index.B89dZ0-N.js:14773:25)
    at async createVitest (file:///D:/work/aicross-p4/src-ui/node_modules/vitest/dist/chunks/cli-api.LUtK11-x.js:26:17)
    at async prepareVitest (file:///D:/work/aicross-p4/src-ui/node_modules/vitest/dist/chunks/cli-api.LUtK11-x.js:421:14) {
  errors: [Getter/Setter]
}
```

标准命令在测试文件加载前因沙箱 `spawn EPERM` 失败，随后使用任务书允许的 fallback。

#### `cd src-ui && npx vitest run --configLoader native --pool threads`（src-ui，exit=0）

输出原文：

```text
(node:6732) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:30536) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)

 RUN  v5.0.0 D:/work/aicross-p4/src-ui

(node:30536) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:30536) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:30536) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:30536) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:30536) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:30536) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:30536) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:30536) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)

 Test Files  8 passed (8)
      Tests  73 passed (73)
   Start at  21:40:17
   Duration  267ms (transform 59%, import 22%, tests 11%, worker 8%)
```
#### `cd src-ui && npm run build`（src-ui，首次采集 exit=1）

输出原文：

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build

failed to load config from D:\work\aicross-p4\src-ui\vite.config.ts
error during build:
Error: Build failed with 1 error:

[plugin externalize-deps]
Error: spawn EPERM
```

首次报告采集与标准 Vitest 一样在 Vite 配置加载前遇到沙箱 `spawn EPERM`；未改代码，立即按同一命令重跑。

#### `cd src-ui && npm run build`（src-ui，重跑 exit=0）

输出原文：

```text

> aicross-ui@0.0.0 build
> tsc -b && vite build

(node:31280) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
vite v8.0.7 building client environment for production...
transforming...✓ 571 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                                         2.65 kB │ gzip:   1.16 kB
dist/assets/main-CPSnVwiP.css                         144.77 kB │ gzip:  25.30 kB
dist/assets/dist-js-5OZv1H5C.js                         0.16 kB │ gzip:   0.15 kB
dist/assets/app-XgSAopP-.js                             0.25 kB │ gzip:   0.21 kB
dist/assets/codeowners-YLWRiiGA.js                      0.54 kB │ gzip:   0.31 kB
dist/assets/image-B_hDXdWL.js                           0.61 kB │ gzip:   0.32 kB
dist/assets/shellsession-uwY3V5Kj.js                    0.71 kB │ gzip:   0.42 kB
dist/assets/tsv-FmpeqnFg.js                             0.73 kB │ gzip:   0.34 kB
dist/assets/html-derivative-y8cTjkde.js                 0.83 kB │ gzip:   0.45 kB
dist/assets/git-rebase-BkCIAZbs.js                      0.98 kB │ gzip:   0.44 kB
dist/assets/qmldir-DtRCXXGi.js                          1.00 kB │ gzip:   0.45 kB
dist/assets/csv-BCAXOHVN.js                             1.14 kB │ gzip:   0.37 kB
dist/assets/git-commit-D9uUyrpH.js                      1.22 kB │ gzip:   0.53 kB
dist/assets/xsl-Bo_Acr9l.js                             1.36 kB │ gzip:   0.50 kB
dist/assets/event-CCOELuOC.js                           1.40 kB │ gzip:   0.69 kB
dist/assets/dotenv-BG-R-dnJ.js                          1.42 kB │ gzip:   0.53 kB
dist/assets/sparql-I6g4RV9o.js                          1.47 kB │ gzip:   0.81 kB
dist/assets/ini-D1hC5V4K.js                             1.52 kB │ gzip:   0.50 kB
dist/assets/fortran-fixed-form-Dh8pw333.js              1.66 kB │ gzip:   0.69 kB
dist/assets/docker-BgEUgtmq.js                          1.73 kB │ gzip:   0.60 kB
dist/assets/hxml-_7MT1TaI.js                            1.74 kB │ gzip:   0.88 kB
dist/assets/desktop-DeW1bojR.js                         1.83 kB │ gzip:   0.76 kB
dist/assets/core-CAYfUUdT.js                            1.98 kB │ gzip:   0.82 kB
dist/assets/erb-Bx3F2RzE.js                             2.13 kB │ gzip:   0.62 kB
dist/assets/wenyan-B7WaDmpb.js                          2.15 kB │ gzip:   1.09 kB
dist/assets/jssm-nqlDcCik.js                            2.23 kB │ gzip:   0.61 kB
dist/assets/edge-BMCHHRKD.js                            2.30 kB │ gzip:   0.67 kB
dist/assets/reg-C9YB0Q6j.js                             2.34 kB │ gzip:   0.69 kB
dist/assets/diff-IPIU9Fzm.js                            2.56 kB │ gzip:   0.69 kB
dist/assets/gleam-Bqam2yKl.js                           2.57 kB │ gzip:   0.82 kB
dist/assets/defineProperty-Cc0ioC06.js                  2.59 kB │ gzip:   1.30 kB
dist/assets/hy-B7p7U1BT.js                              2.64 kB │ gzip:   1.18 kB
dist/assets/openscad-C-9uKvUO.js                        2.82 kB │ gzip:   1.01 kB
dist/assets/log-vQtdJHWP.js                             2.85 kB │ gzip:   0.90 kB
dist/assets/json-DG5xeaaD.js                            2.88 kB │ gzip:   0.81 kB
dist/assets/cairo-ClvR2owv.js                           2.93 kB │ gzip:   0.80 kB
dist/assets/berry-BUpy9_cc.js                           3.00 kB │ gzip:   0.81 kB
dist/assets/jsonl-DhVam5Gu.js                           3.00 kB │ gzip:   0.78 kB
dist/assets/jsonc-BK4mmDTw.js                           3.10 kB │ gzip:   0.79 kB
dist/assets/logo-CCxtNjv2.js                            3.12 kB │ gzip:   1.47 kB
dist/assets/po-CJUqynl0.js                              3.23 kB │ gzip:   0.91 kB
dist/assets/json5-B0H0ftRm.js                           3.25 kB │ gzip:   0.83 kB
dist/assets/mipsasm-BLGWrDdP.js                         3.25 kB │ gzip:   1.17 kB
dist/assets/tasl-BGeyykQ_.js                            3.28 kB │ gzip:   0.84 kB
dist/assets/webview-Dcvwb5z8.js                         3.31 kB │ gzip:   1.12 kB
dist/assets/genie-CN4rq7A0.js                           3.35 kB │ gzip:   1.21 kB
dist/assets/rel-DifwsMId.js                             3.36 kB │ gzip:   1.11 kB
dist/assets/vala-C6OpFbMA.js                            3.36 kB │ gzip:   1.19 kB
dist/assets/splunk-Cf3CW7wI.js                          3.43 kB │ gzip:   1.52 kB
dist/assets/hurl-2ACva3j7.js                            3.51 kB │ gzip:   1.10 kB
dist/assets/fluent-DGOyQ8Ws.js                          3.60 kB │ gzip:   0.90 kB
dist/assets/ssh-config-CKUNcAMa.js                      3.61 kB │ gzip:   1.60 kB
dist/assets/jsonnet-BOcQ7pWS.js                         3.61 kB │ gzip:   1.04 kB
dist/assets/kdl-DBigZy8w.js                             3.62 kB │ gzip:   1.04 kB
dist/assets/narrat-Cq3CR6rl.js                          3.67 kB │ gzip:   1.10 kB
dist/assets/turtle-D_eMCpxI.js                          3.69 kB │ gzip:   0.97 kB
dist/assets/glsl-DtQQAfSz.js                            3.70 kB │ gzip:   1.46 kB
dist/assets/zenscript-wcLUgsD2.js                       3.91 kB │ gzip:   1.28 kB
dist/assets/ron-ChSsJUxP.js                             3.91 kB │ gzip:   0.98 kB
dist/assets/gn-DJDjd0xf.js                              4.00 kB │ gzip:   1.48 kB
dist/assets/pascal-BHiDpJdH.js                          4.14 kB │ gzip:   1.67 kB
dist/assets/http-duTJ0maY.js                            4.41 kB │ gzip:   1.06 kB
dist/assets/tcl-CGMQ2Gre.js                             4.42 kB │ gzip:   1.50 kB
dist/assets/nextflow-DYYpq5fR.js                        4.50 kB │ gzip:   1.16 kB
dist/assets/rosmsg-C8yR4p9t.js                          4.52 kB │ gzip:   1.06 kB
dist/assets/polar-BiOMIDty.js                           4.66 kB │ gzip:   1.12 kB
dist/assets/sdbl-DbzyTi_y.js                            4.70 kB │ gzip:   2.01 kB
dist/assets/fennel-Cp7mIZmg.js                          4.76 kB │ gzip:   1.54 kB
dist/assets/bibtex-CMf_khHV.js                          4.79 kB │ gzip:   0.83 kB
dist/assets/llvm-BUQffIyu.js                            5.04 kB │ gzip:   2.02 kB
dist/assets/wgsl-8R5OMxfU.js                            5.14 kB │ gzip:   1.39 kB
dist/assets/gdresource-VMh-5Lix.js                      5.28 kB │ gzip:   1.34 kB
dist/assets/zig-7Lbxfjhx.js                             5.33 kB │ gzip:   1.55 kB
dist/assets/qml-DV-9MRuy.js                             5.33 kB │ gzip:   1.38 kB
dist/assets/dax-DlTOXg5S.js                             5.36 kB │ gzip:   2.22 kB
dist/assets/bicep-mN0ruteN.js                           5.38 kB │ gzip:   1.15 kB
dist/assets/xml-CyIjMK9K.js                             5.45 kB │ gzip:   1.25 kB
dist/assets/awk-C2cQpOjH.js                             5.45 kB │ gzip:   1.38 kB
dist/assets/coq-M9LrrHou.js                             5.52 kB │ gzip:   1.91 kB
dist/assets/jinja-CaX9vMDt.js                           5.62 kB │ gzip:   1.36 kB
dist/assets/lean-Dtsklvza.js                            5.78 kB │ gzip:   1.92 kB
dist/assets/moonbit-DelbyvlD.js                         5.89 kB │ gzip:   1.68 kB
dist/assets/powerquery-DDoDbTf0.js                      5.90 kB │ gzip:   1.52 kB
dist/assets/shaderlab-C-QDXGQP.js                       5.92 kB │ gzip:   2.08 kB
dist/assets/verilog-BNuILg-8.js                         5.92 kB │ gzip:   1.89 kB
dist/assets/cypher-Bf0d0j36.js                          5.95 kB │ gzip:   1.72 kB
dist/assets/vb-vQ7FBk14.js                              6.09 kB │ gzip:   2.34 kB
dist/assets/red-CgmMwQ5s.js                             6.26 kB │ gzip:   1.60 kB
dist/assets/min-dark-CBFjRQ4v.js                        6.28 kB │ gzip:   1.70 kB
dist/assets/gdshader-DLlTfkMm.js                        6.32 kB │ gzip:   1.72 kB
dist/assets/prisma-CPh4c3GQ.js                          6.32 kB │ gzip:   1.39 kB
dist/assets/ara-CpyWD-c6.js                             6.36 kB │ gzip:   1.81 kB
dist/assets/clojure-C3jjSFuY.js                         6.41 kB │ gzip:   1.42 kB
dist/assets/postcss-DAA_T9V0.js                         6.41 kB │ gzip:   1.91 kB
dist/assets/toml-BlynkCpr.js                            6.42 kB │ gzip:   1.28 kB
dist/assets/solarized-light-DLLndLS4.js                 6.47 kB │ gzip:   1.71 kB
dist/assets/proto-C0LdvFNL.js                           6.54 kB │ gzip:   1.42 kB
dist/assets/smalltalk-BIVJlCyX.js                       6.58 kB │ gzip:   1.62 kB
dist/assets/r-B4zVi_fs.js                               6.61 kB │ gzip:   1.83 kB
dist/assets/talonscript-Dr-lPTln.js                     6.75 kB │ gzip:   1.49 kB
dist/assets/solarized-dark-DgqPf06l.js                  6.84 kB │ gzip:   1.78 kB
dist/assets/riscv-BM628xIq.js                           6.91 kB │ gzip:   1.98 kB
dist/assets/soy-jLaB-7Vb.js                             6.92 kB │ gzip:   1.62 kB
dist/assets/min-light-OvUhfe4y.js                       6.96 kB │ gzip:   1.88 kB
dist/assets/scheme-BIfFz6Pe.js                          7.16 kB │ gzip:   2.05 kB
dist/assets/hlsl-I4TUSC5a.js                            7.26 kB │ gzip:   2.19 kB
dist/assets/qss-BgVkvApy.js                             7.47 kB │ gzip:   2.57 kB
dist/assets/systemd-D4187P0y.js                         7.86 kB │ gzip:   2.54 kB
dist/assets/monokai-DHFwemSO.js                         7.88 kB │ gzip:   1.90 kB
dist/assets/regexp-BnAW9hSB.js                          8.05 kB │ gzip:   1.47 kB
dist/assets/haml-DtItqECI.js                            8.34 kB │ gzip:   1.86 kB
dist/assets/typst-BP5N7E5y.js                           8.38 kB │ gzip:   1.67 kB
dist/assets/vue-html-C5v0WTbT.js                        8.47 kB │ gzip:   1.67 kB
dist/assets/plsql-DEXRAcVm.js                           8.51 kB │ gzip:   2.99 kB
dist/assets/dart-3JwWll0a.js                            8.62 kB │ gzip:   2.00 kB
dist/assets/horizon-C9nqL9R6.js                         8.77 kB │ gzip:   1.95 kB
dist/assets/kotlin-BYL6QRVS.js                          8.78 kB │ gzip:   2.13 kB
dist/assets/horizon-bright-Css32xyz.js                  8.78 kB │ gzip:   1.96 kB
dist/assets/make-Dw3yrZ0Q.js                            8.95 kB │ gzip:   1.77 kB
dist/assets/andromeeda-DGoCYgJo.js                      9.01 kB │ gzip:   2.36 kB
dist/assets/sas-CbDNbSM0.js                             9.06 kB │ gzip:   3.82 kB
dist/assets/dark-plus-DY33niKa.js                       9.09 kB │ gzip:   2.10 kB
dist/assets/slack-dark-BAvxGu1N.js                      9.11 kB │ gzip:   1.96 kB
dist/assets/sass-DocrZT3r.js                            9.28 kB │ gzip:   2.49 kB
dist/assets/plastic-iniznJdO.js                         9.29 kB │ gzip:   1.97 kB
dist/assets/slack-ochin-DWqn_Lk-.js                     9.42 kB │ gzip:   2.09 kB
dist/assets/tex-6AKuZd1g.js                             9.67 kB │ gzip:   3.06 kB
dist/assets/jison--pZ8m-HK.js                           9.69 kB │ gzip:   1.87 kB
dist/assets/cmake-DrqqGEK_.js                           9.85 kB │ gzip:   3.38 kB
dist/assets/light-plus-DfRHv4Xo.js                      9.93 kB │ gzip:   2.27 kB
dist/assets/hcl-WYMZvHje.js                            10.05 kB │ gzip:   1.93 kB
dist/assets/rst-CK2J_xNG.js                            10.30 kB │ gzip:   2.25 kB
dist/assets/pkl-DawJyhw3.js                            10.37 kB │ gzip:   1.38 kB
dist/assets/beancount-QTI4W_q2.js                      10.37 kB │ gzip:   1.43 kB
dist/assets/nextflow-groovy-A4by5qzv.js                10.43 kB │ gzip:   2.14 kB
dist/assets/dream-maker-C3tt4vD1.js                    10.46 kB │ gzip:   2.26 kB
dist/assets/raku-Ba22PpAY.js                           10.47 kB │ gzip:   2.94 kB
dist/assets/yaml-CzLfUaQ1.js                           10.57 kB │ gzip:   2.32 kB
dist/assets/just-RTSWN6SD.js                           10.77 kB │ gzip:   2.60 kB
dist/assets/elm-D_V6tPeg.js                            10.94 kB │ gzip:   2.10 kB
dist/assets/github-light-CIJNwzyc.js                   11.18 kB │ gzip:   2.49 kB
dist/assets/prolog-BCLGsSxa.js                         11.35 kB │ gzip:   3.85 kB
dist/assets/terraform-DfAmkD3Z.js                      11.38 kB │ gzip:   2.51 kB
dist/assets/github-dark-OFaljh7J.js                    11.40 kB │ gzip:   2.53 kB
dist/assets/puppet-BI7VePLm.js                         11.43 kB │ gzip:   2.11 kB
dist/assets/laserwave-CQtCIGFc.js                      11.49 kB │ gzip:   2.57 kB
dist/assets/gherkin-Cr0wk206.js                        11.94 kB │ gzip:   5.05 kB
dist/assets/wasm-Beqnk49Y.js                           12.00 kB │ gzip:   2.18 kB
dist/assets/hjson-CbFL5VWg.js                          12.05 kB │ gzip:   1.64 kB
dist/assets/handlebars-Dit8JC0x.js                     12.16 kB │ gzip:   2.38 kB
dist/assets/window-Cf5upF_J.js                         12.41 kB │ gzip:   3.01 kB
dist/assets/apache-mOm_gFPl.js                         12.45 kB │ gzip:   3.71 kB
dist/assets/vesper-B0XbIQa2.js                         12.69 kB │ gzip:   1.98 kB
dist/assets/bat-CD8xesoU.js                            12.88 kB │ gzip:   3.23 kB
dist/assets/fish-Bw4coD8r.js                           13.03 kB │ gzip:   1.73 kB
dist/assets/v-DG0D3hZq.js                              13.21 kB │ gzip:   2.73 kB
dist/assets/vitesse-light-piMchJtN.js                  13.61 kB │ gzip:   3.03 kB
dist/assets/aurora-x-WbeSdnAE.js                       13.65 kB │ gzip:   2.28 kB
dist/assets/vitesse-black-DuzBzXNC.js                  13.67 kB │ gzip:   3.06 kB
dist/assets/vitesse-dark-ChPfzWLu.js                   13.75 kB │ gzip:   3.06 kB
dist/assets/pug-CM6j4wdg.js                            13.85 kB │ gzip:   2.58 kB
dist/assets/luau-CsHfLQlf.js                           13.96 kB │ gzip:   3.17 kB
dist/assets/synthwave-84-B1wrF6Pb.js                   14.03 kB │ gzip:   2.85 kB
dist/assets/github-light-default-Cskbnfwe.js           14.15 kB │ gzip:   3.02 kB
dist/assets/clarity-zNHM1tMD.js                        14.27 kB │ gzip:   2.47 kB
dist/assets/github-light-high-contrast-BUZL_cRA.js     14.27 kB │ gzip:   3.00 kB
dist/assets/github-dark-dimmed-DLGfz6FO.js             14.43 kB │ gzip:   3.11 kB
dist/assets/github-dark-default-BqbMvdcT.js            14.43 kB │ gzip:   3.12 kB
dist/assets/github-dark-high-contrast-wBnEARx0.js      14.59 kB │ gzip:   3.08 kB
dist/assets/gnuplot-Bd1MpVfM.js                        14.78 kB │ gzip:   3.28 kB
dist/assets/rust-jpCPbsEK.js                           15.06 kB │ gzip:   2.72 kB
dist/assets/kusto-COSTdHqw.js                          15.17 kB │ gzip:   3.93 kB
dist/assets/actionscript-3-LMiqWuSE.js                 15.20 kB │ gzip:   2.68 kB
dist/assets/nix-BTV8Kl4V.js                            15.50 kB │ gzip:   2.48 kB
dist/assets/lua-BB1sjGF9.js                            15.60 kB │ gzip:   3.19 kB
dist/assets/abap-s17gYGWC.js                           15.85 kB │ gzip:   5.93 kB
dist/assets/solidity-D-b1Sb1s.js                       16.07 kB │ gzip:   3.10 kB
dist/assets/matlab-D7Ymh7TH.js                         16.08 kB │ gzip:   3.07 kB
dist/assets/cue-CHmhgwIJ.js                            16.20 kB │ gzip:   2.05 kB
dist/assets/elixir-1Wj0-XTx.js                         16.26 kB │ gzip:   2.76 kB
dist/assets/odin-CzGDjIwK.js                           16.51 kB │ gzip:   2.93 kB
dist/assets/ts-tags-NZ2iHfv6.js                        16.54 kB │ gzip:   2.05 kB
dist/assets/kanagawa-wave-D_Hm3vX6.js                  17.12 kB │ gzip:   2.90 kB
dist/assets/kanagawa-lotus-CqgQE5Hl.js                 17.12 kB │ gzip:   2.91 kB
dist/assets/kanagawa-dragon-CByus8H8.js                17.12 kB │ gzip:   2.92 kB
dist/assets/move-BG_bNIu4.js                           17.50 kB │ gzip:   3.07 kB
dist/assets/graphql-DZnKSmmH.js                        18.08 kB │ gzip:   2.54 kB
dist/assets/liquid-DAb-uuE2.js                         18.10 kB │ gzip:   3.15 kB
dist/assets/svelte-CPH69QWw.js                         18.25 kB │ gzip:   3.14 kB
dist/assets/material-theme-CY5ZxTT5.js                 18.61 kB │ gzip:   3.10 kB
dist/assets/material-theme-darker-BnDPOEon.js          18.62 kB │ gzip:   3.10 kB
dist/assets/material-theme-ocean-CAu3HLd7.js           18.62 kB │ gzip:   3.12 kB
dist/assets/material-theme-lighter-CXYE62LH.js         18.63 kB │ gzip:   3.11 kB
dist/assets/material-theme-palenight-BLPSe7SZ.js       18.63 kB │ gzip:   3.12 kB
dist/assets/gdscript-C2D5WM82.js                       18.99 kB │ gzip:   3.75 kB
dist/assets/groovy-BeOakm-O.js                         19.17 kB │ gzip:   3.60 kB
dist/assets/mdc-C5YQj5X-.js                            19.54 kB │ gzip:   6.67 kB
dist/assets/ayu-dark-C8AZSEqd.js                       20.07 kB │ gzip:   3.93 kB
dist/assets/ayu-mirage-DJDt9_Ik.js                     20.08 kB │ gzip:   3.91 kB
dist/assets/glimmer-js-Z7X_LugF.js                     20.08 kB │ gzip:   2.94 kB
dist/assets/glimmer-ts-CmBNQRbz.js                     20.08 kB │ gzip:   2.94 kB
dist/assets/powershell-DThFFtVM.js                     20.14 kB │ gzip:   4.06 kB
dist/assets/ayu-light-DGy4vufb.js                      20.15 kB │ gzip:   3.90 kB
dist/assets/viml-CP_eTIVX.js                           20.36 kB │ gzip:   6.74 kB
dist/assets/nushell-_kwMquEq.js                        20.41 kB │ gzip:   5.22 kB
dist/assets/snazzy-light-DEGtvnBx.js                   20.77 kB │ gzip:   3.81 kB
dist/assets/twig-EY5SnVCc.js                           20.90 kB │ gzip:   3.66 kB
dist/assets/dracula-C8tduV4h.js                        21.06 kB │ gzip:   4.00 kB
dist/assets/dracula-soft-i03TrcGR.js                   21.07 kB │ gzip:   4.02 kB
dist/assets/wit-Q3uzgIzy.js                            21.46 kB │ gzip:   2.88 kB
dist/assets/rose-pine-IZyio8vo.js                      21.74 kB │ gzip:   3.86 kB
dist/assets/rose-pine-moon-BfRCxBHe.js                 21.75 kB │ gzip:   3.88 kB
dist/assets/rose-pine-dawn-D1oZXPoh.js                 21.75 kB │ gzip:   3.88 kB
dist/assets/nim-mCDe1XWm.js                            22.45 kB │ gzip:   3.16 kB
dist/assets/common-lisp-D0DsnuEJ.js                    22.57 kB │ gzip:   6.07 kB
dist/assets/surrealql-DhK7dDdV.js                      22.58 kB │ gzip:   4.31 kB
dist/assets/gruvbox-dark-hard-BqtMDTUh.js              22.63 kB │ gzip:   4.16 kB
dist/assets/gruvbox-dark-soft-CV57n6pA.js              22.63 kB │ gzip:   4.15 kB
dist/assets/gruvbox-light-hard-b4k0L1XS.js             22.63 kB │ gzip:   4.16 kB
dist/assets/gruvbox-light-soft-DP66dnY9.js             22.63 kB │ gzip:   4.16 kB
dist/assets/gruvbox-dark-medium-Dgfy8-dz.js            22.63 kB │ gzip:   4.16 kB
dist/assets/gruvbox-light-medium-DSewW6QF.js           22.63 kB │ gzip:   4.16 kB
dist/assets/bird2-Bh111mYM.js                          22.94 kB │ gzip:   4.66 kB
dist/assets/sql-DcHRMm-Q.js                            23.47 kB │ gzip:   7.49 kB
dist/assets/cadence-DXH0Dgwr.js                        23.66 kB │ gzip:   3.66 kB
dist/assets/astro-XmnDH_Dd.js                          24.02 kB │ gzip:   7.60 kB
dist/assets/apl-DjqofZJD.js                            24.03 kB │ gzip:   4.17 kB
dist/assets/templ-BkokcB7D.js                          24.06 kB │ gzip:   5.41 kB
dist/assets/angular-html-BlerCZUb.js                   24.25 kB │ gzip:   3.98 kB
dist/assets/vhdl-3hDyUkZR.js                           24.26 kB │ gzip:   3.86 kB
dist/assets/vue-CqFBltHt.js                            24.50 kB │ gzip:   2.97 kB
dist/assets/purescript-DKMYP_A1.js                     24.68 kB │ gzip:   3.24 kB
dist/assets/typespec-BQy10TzM.js                       25.04 kB │ gzip:   2.62 kB
dist/assets/one-light-27e-9kYd.js                      25.29 kB │ gzip:   3.63 kB
dist/assets/fsharp-BQAqzjqk.js                         25.30 kB │ gzip:   4.13 kB
dist/assets/marko-Cj1_9Cb9.js                          25.48 kB │ gzip:   3.59 kB
dist/assets/c3-qRBCI8Uv.js                             25.62 kB │ gzip:   3.86 kB
dist/assets/night-owl-light-DtpYOdyl.js                25.90 kB │ gzip:   4.24 kB
dist/assets/system-verilog-jOLk1CBm.js                 26.20 kB │ gzip:   4.83 kB
dist/assets/nord-BQkWtC2v.js                           26.72 kB │ gzip:   4.36 kB
dist/assets/codeql-BoicTpVa.js                         26.88 kB │ gzip:   3.80 kB
dist/assets/scss-Dw42T1BI.js                           27.27 kB │ gzip:   4.23 kB
dist/assets/java-CoApPKT4.js                           27.28 kB │ gzip:   4.31 kB
dist/assets/coffee-fAb6LsdV.js                         27.42 kB │ gzip:   6.35 kB
dist/assets/razor-ccbgdA8j.js                          27.45 kB │ gzip:   3.54 kB
dist/assets/scala-D0LjI2Qi.js                          28.88 kB │ gzip:   3.93 kB
dist/assets/night-owl-DDmCLEp1.js                      28.91 kB │ gzip:   5.15 kB
dist/assets/crystal-HHV-Zo9X.js                        29.41 kB │ gzip:   4.44 kB
dist/assets/mermaid-CAtOlIbf.js                        29.50 kB │ gzip:   3.65 kB
dist/assets/applescript-C5yVau2U.js                    29.56 kB │ gzip:   5.95 kB
dist/assets/julia-jUIBt023.js                          30.99 kB │ gzip:   4.30 kB
dist/assets/stylus-CNnrGufP.js                         31.07 kB │ gzip:   8.02 kB
dist/assets/poimandres-H4_Eyox2.js                     33.49 kB │ gzip:   5.49 kB
dist/assets/one-dark-pro-C4hn0Zoq.js                   33.78 kB │ gzip:   5.50 kB
dist/assets/bsl-ny3e_7dl.js                            33.86 kB │ gzip:   8.35 kB
dist/assets/haxe-C2AyQsKn.js                           35.16 kB │ gzip:   5.91 kB
dist/assets/nginx-B8zy1DQG.js                          35.35 kB │ gzip:   4.46 kB
dist/assets/houston-ByNQ9eAt.js                        35.42 kB │ gzip:   5.76 kB
dist/assets/tokyo-night-DQIh82d3.js                    35.66 kB │ gzip:   6.23 kB
dist/assets/erlang-Q-lfVOgV.js                         37.47 kB │ gzip:   4.36 kB
dist/assets/cobol-DQafa4Ab.js                          39.07 kB │ gzip:  10.89 kB
dist/assets/asm-Bbv1919r.js                            40.71 kB │ gzip:   8.17 kB
dist/assets/haskell-DTAtmdln.js                        41.48 kB │ gzip:   6.41 kB
dist/assets/shellscript-3Eyrpdfm.js                    41.54 kB │ gzip:   6.13 kB
dist/assets/perl-CEA6MP1k.js                           43.14 kB │ gzip:   4.65 kB
dist/assets/d-Dt1IpaRS.js                              43.79 kB │ gzip:   8.43 kB
dist/assets/ruby-B-fKc9x2.js                           46.60 kB │ gzip:   5.71 kB
dist/assets/go-D3d4JNoO.js                             46.81 kB │ gzip:   5.15 kB
dist/assets/apex-Bkns-Kf9.js                           46.98 kB │ gzip:   6.73 kB
dist/assets/catppuccin-mocha-D4_Pt6ay.js               47.25 kB │ gzip:   7.98 kB
dist/assets/catppuccin-latte-Coqss7en.js               47.25 kB │ gzip:   7.99 kB
dist/assets/catppuccin-frappe-C5l_8kFT.js              47.25 kB │ gzip:   7.99 kB
dist/assets/catppuccin-macchiato-DEc0mBz1.js           47.26 kB │ gzip:   7.99 kB
dist/assets/ada-CgdG45k-.js                            48.08 kB │ gzip:   5.96 kB
dist/assets/css-BaIEaAP4.js                            49.11 kB │ gzip:  11.96 kB
dist/assets/imba-D2BupS_o.js                           49.92 kB │ gzip:   9.47 kB
dist/assets/everforest-dark-DnGtViwz.js                53.74 kB │ gzip:   8.37 kB
dist/assets/everforest-light-CrRagu-i.js               53.74 kB │ gzip:   8.37 kB
dist/assets/wikitext-CokaWg4C.js                       55.88 kB │ gzip:   4.82 kB
dist/assets/stata-LzqjS0NL.js                          56.98 kB │ gzip:  12.39 kB
dist/assets/html-B1M_0gd8.js                           57.32 kB │ gzip:  11.79 kB
dist/assets/ballerina-C5NCmhdh.js                      58.69 kB │ gzip:   8.11 kB
dist/assets/markdown-DrnMgzIX.js                       59.33 kB │ gzip:   5.67 kB
dist/assets/ocaml-X3W8brJ7.js                          62.44 kB │ gzip:   5.05 kB
dist/assets/mojo-BEOkTXe0.js                           69.79 kB │ gzip:   9.22 kB
dist/assets/python-BtjLSkiI.js                         69.95 kB │ gzip:   9.10 kB
dist/assets/c-yOg8G6Qk.js                              72.17 kB │ gzip:  10.55 kB
dist/assets/latex-DFkYhUza.js                          72.61 kB │ gzip:   6.69 kB
dist/assets/vyper-BGjJ65g4.js                          74.64 kB │ gzip:  10.70 kB
dist/assets/hack-CB6YBmfz.js                           80.18 kB │ gzip:  26.27 kB
dist/assets/swift-ByDBHGEa.js                          86.68 kB │ gzip:  14.69 kB
dist/assets/fortran-free-form-BXrNE5X_.js              88.96 kB │ gzip:  11.19 kB
dist/assets/csharp-DK3gKUDm.js                         89.68 kB │ gzip:  10.64 kB
dist/assets/racket-DChGz2wN.js                         92.38 kB │ gzip:  15.08 kB
dist/assets/less-CCCdi_4d.js                           97.63 kB │ gzip:  14.85 kB
dist/assets/blade-DhyUajd-.js                         104.97 kB │ gzip:  28.30 kB
dist/assets/objective-c-Do3Dh539.js                   105.41 kB │ gzip:  23.64 kB
dist/assets/php-D1n20mva.js                           111.31 kB │ gzip:  28.66 kB
dist/assets/asciidoc-Mnh7mpqW.js                      131.52 kB │ gzip:   9.32 kB
dist/assets/mdx-BhxtouIL.js                           136.11 kB │ gzip:  23.54 kB
dist/assets/objective-cpp-DzvX7O7S.js                 171.97 kB │ gzip:  30.98 kB
dist/assets/javascript-DMDj563O.js                    174.89 kB │ gzip:  16.68 kB
dist/assets/tsx-Du47Vi3R.js                           175.60 kB │ gzip:  16.67 kB
dist/assets/jsx-BgYHCIzu.js                           177.86 kB │ gzip:  16.77 kB
dist/assets/typescript-Biw9qNLM.js                    181.14 kB │ gzip:  16.29 kB
dist/assets/angular-ts-BErpjn_O.js                    183.73 kB │ gzip:  16.77 kB
dist/assets/vue-vine-DaNosUn7.js                      190.06 kB │ gzip:  18.07 kB
dist/assets/dist-CbAFeoTq.js                          212.97 kB │ gzip:  67.93 kB
dist/assets/wolfram-BPifCLvx.js                       262.38 kB │ gzip:  77.64 kB
dist/assets/wasm-DA6mYL5T.js                          622.32 kB │ gzip: 232.09 kB
dist/assets/cpp-Cyj1bR0k.js                           626.13 kB │ gzip:  48.10 kB
dist/assets/emacs-lisp-DzXDqNne.js                    779.87 kB │ gzip: 197.56 kB
dist/assets/main-B9PhhwAs.js                        1,225.38 kB │ gzip: 439.06 kB

✓ built in 2.06s
[plugin builtin:vite-reporter] 
(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rolldownOptions.output.codeSplitting to improve chunking: https://rolldown.rs/reference/OutputOptions.codeSplitting
- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
```

重跑构建退出码为 0。Vite 的 chunk 大于 500 kB 提示为既有构建警告。

 ## 验收 2：指定行为测试
## 验收 2：指定行为测试

Rust 测试覆盖：

- `snapshot_builds_diff_and_changed_file_content`：快照、diff 和改后全文。
- `project_write_lock_is_mutually_exclusive`：同项目写锁互斥。
- `blind_acceptance_directory_contains_only_artifact_delivery`：盲验收目录只有三类交付物。
- `roles_roundtrip_and_fill_economy_nodes`：角色配置往返和经济预设填充。
- `pi_continuation_preserves_thinking_setting`：pi 续接不丢 thinking。
- `writable_codex_uses_workspace_write_sandbox`：Codex 写节点使用 workspace-write。
- `pi_response_model_mismatch_fails_and_reports_true_identity`：类型化真身不符。
- `cc_switch_reads_usage_from_stderr`：cc-switch usage 字段顺序无关。
- `broad_redaction_warning_only_counts_unquoted_whole_lines`：宽泛脱敏提示。

前端测试覆盖：

- 共享、指定上游、盲三种输入可见性；盲验收收到用户原话与执行 artifacts，但收不到方案文本。
- PASS、NEEDS_FIX、未知三种验收结论。
- 小任务与链式任务启发式。
- 角色固定 prompt。
- 图层阶段计算、修正轮节点追加、验证失败重试节点追加和最大轮次。

## 验收 3：真机经济流水线

状态：**已验证**。编排者于 2026-09-13 21:47–21:52 在沙箱外执行真机测试。

ignored 测试：`dispatch::tests::live_economy_pipeline`。它固定为 3 次请求，没有替补分支：

1. 出方案：`pi/zai-coding-cn/glm-5.3-flash`，thinking 高。
2. 执行：`codex/gpt-5.6-luna`，单写者，在临时项目副本运行。
3. 验收：`kimi/kimi-code/k3-256k`，盲目录。

测试断言：

- `fixtures/pipeline/roman.py` 的执行产物 diff 非空。
- `python -m pytest` 通过并保存验证输出后才进入验收。
- 验收工作目录恰好只有 `REQUEST.md`、`diff.patch`、`changed/`。
- 项目 `.dispatch/` 有三份派发记录和 `STATE.md`。
- 请求总数为 3，小于 8 次上限。

沙箱外运行命令：

```powershell
cargo test dispatch::tests::live_economy_pipeline -- --ignored --nocapture
```

结果：**1 passed**，`REQUESTS=3`；实际顺序为出方案 `pi/glm-5.3-flash` → 执行 `codex/gpt-5.6-luna` → 验收 `kimi/k3-256k`，run 为 `task04-pipeline-20260913-214756`，`STATE.md` 位于临时项目目录。该结论来自编排者给出的沙箱外真跑记录；本修正轮没有重复发请求。

## 第 3 期遗留 2：cc-switch 真跑

状态：**已验证**。编排者于 2026-09-13 21:47–21:52 在沙箱外执行真机测试。

ignored 测试：`dispatch::tests::live_cc_switch_route`，最多 1 次请求。它只从显式环境变量接收脚本路径和 provider，不读取 `~/.cc-switch`：

```powershell
$env:AICROSS_CC_SWITCH_PATH = 'D:\path\to\cc_switch.py'
$env:AICROSS_CC_SWITCH_PROVIDER = 'provider-name'
cargo test dispatch::tests::live_cc_switch_route -- --ignored --nocapture
```

编排者使用 `AICROSS_CC_SWITCH_PATH=… AICROSS_CC_SWITCH_PROVIDER="Zhipu GLM" cargo test dispatch::tests::live_cc_switch_route -- --ignored --nocapture` 真跑：**1 passed**，`CC_SWITCH_REQUESTS=1`；留痕为 `.dispatch/20260913-215132-cc-switch-cc-switch-验证.md`，用量为 in 7531 / out 63，usage 正则已成功解析真实 `--usage` 输出。

## 没做什么

- 没有自动 merge、自动 commit。
- 没有蜂群、多写者或并行修改同一项目。
- 没有重做第 2 期图、第 3 期面板或派发引擎。
- 没有运行 `tauri dev` 或打开桌面窗口。
- 修正实现者没有在沙箱内重复执行两个 ignored 真机测试；采用编排者给出的沙箱外真跑结果。

## 不确定处与交接

- 编排者已在沙箱外完成真机经济流水线与 cc-switch 一次请求；本修正轮未重复核验窗口是否出现黑窗。
- Rust ignored 真机 harness 直接构造后端 `DispatchStageRequest`，未走前端生产 `serializeContext`；这是当前过渡实现。`serializeContext` / `BLIND_PROMPT_PREFIX` 的生产路径由前端测试覆盖，真机 harness 验证其后的派发、产物和验收交付链，但它本身不覆盖前端推进循环。
- worktree 的创建、非 Git 回退和手动合并提示已实现并通过编译，但本轮没有做真实 Git 仓库集成跑。
- 重启后“继续/放弃”和 running 浮层收起的界面路径已通过 TypeScript 构建，按用户要求没有起窗口做视觉交互检查。
- `tasks/impl-04-prompt.txt` 是原有未跟踪文件，本期未修改。

## 修正轮 R1

### 逐条处理

1. 已修：盲验收的 `contextPartsOf` 现在额外给出默认不选的“被排除的上游回复”候选；G1 可勾选，选中后写入 `selection.messageKeys`，`serializeContext` 会保留该材料。已补默认排除和主动选入测试。
2. 已修：`parseAcceptance` 只识别带单词边界的 `PASS` / `NEEDS_FIX`，在末尾 1200 字符窗口内按最后出现位置判定。已覆盖 `all tests passed`、双词同现和截断线外三种情况。
3. 已修：设置页角色区域底部只读展示方案、执行、验收三类固定 prompt 模板。
4. 已修：用量表按 stage 分组，并在各阶段记录后插入小计行。
5. 已修：单写者且目标不是 codex 时，G1 显示“该通道无写工具，产物为空”；执行角色默认值优先选择已接入的 codex 可写通道，没有可用 codex 时不伪填目标。
6. 已修：项目写锁获取失败不再用 `.ok()` 丢弃错误；节点直接记为 `status=error`，不会无锁继续执行。
7. 已修：盲验收父节点是否走目录交付只看 artifacts，不再看 `kind`；有 artifacts 时父回复不进 prompt，无 artifacts 时回退进入 prompt。已补 turn 型单父验收测试。
8. 已修：快照对单文件设置 8 MB 上限，超限只保存大小和修改时间标记；发生变化时按 Binary 产物处理，不把大文件读入内存。
9. 已修：“继续”会先从 `STATE.md` 读取 `run_id` 和活跃节点 ID，逐个调用 `cancel_dispatch_node` 清理孤儿进程，再重新派发。
10. 已修：G1 浮层与卡片改用不透明面板底色；材料摘要按清理后的 vendor 去重连接，不再留下末尾顿号；资源列表为空时目标可手填 agent/model；未指定模型显示“未指定模型”并阻止预览、确认和派发。
11. 已修：`changed/` 路径校验同时拒绝 Windows 盘符前缀、绝对路径和父目录分量，并补盘符相对路径测试。
12. 已修：`STATE.md` 首行写入机器可读 `aicross-state` JSON，`inspect_dispatch_state` 只按该头部恢复 run 与活跃节点；`validation_failed` 在状态表中单列。
13. 已修：`applyDispatchResults` 重建 reply 时保留已有 `adopted` 值，并补测试。
14. 已修：确认列表保留深层草稿节点；父节点尚未完成时明确标注“待上游完成”，与本轮实际派发集合区分。
15. 已修：抽出 `is_continuing(node)` 供 preview/run 共用；移除归类模型 input 的无效 `onChange`；两套原子写实现的注释已互相指向，合并仍留到第 5 期。
16. 已修：阶段推进抽为纯函数 `reducePipelineStep`，覆盖非成功停止、验证失败重试上限、验收修正和封顶；`artifact_diff` 新增删除、二进制、超过 200 KB 三类测试，并覆盖 8 MB 快照上限。
17. 已修：报告将 fallback 表述改为“按修正单允许的 fallback”，并明确真机 harness 未经过前端生产 `serializeContext`，属于当前过渡实现。

视觉修正按要求没有启动桌面窗口；完成了源码检查、前端测试和生产构建。编排者真跑事实已同步到“验收 3”和“第 3 期遗留 2”。

### 验证

以下为本轮实际运行命令及输出文本；终端 ANSI 色码已去除，内容未改写。

#### `cargo check`（仓库根目录，exit=0）

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.85s
```

#### `cargo test`（仓库根目录，exit=0）

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 6.72s
     Running unittests src\main.rs (target\debug\deps\aicross-a390c24a3e96ccb6.exe)

running 139 tests
test channels::tests::codex_high_maps_to_high_and_never_to_xhigh ... ok
test channels::tests::blind_claude_contains_restricted ... ok
test channels::tests::blind_pi_with_no_context_files_is_fully_isolated ... ok
test channels::tests::codex_resume_follows_parent_options ... ok
test channels::tests::cc_switch_requires_configured_script_path ... ok
test channels::tests::pi_uses_explicit_session_and_app_owned_session_directory ... ok
test channels::tests::claude_null_api_error_status_is_not_an_error ... ok
test channels::tests::pi_continuation_preserves_thinking_setting ... ok
test channels::tests::pi_response_model_mismatch_fails_and_reports_true_identity ... ok
test channels::tests::pi_reads_camel_case_cache_usage ... ok
test channels::tests::kimi_reads_plain_string_assistant_content_and_resume_hint ... ok
test channels::tests::kimi_requires_configured_alias ... ok
test channels::tests::kimi_rejects_more_than_30000_utf8_bytes ... ok
test channels::tests::pi_error_event_surfaces_error_message ... ok
test channels::tests::scheduling_aliases_normalize_to_one_agent_key ... ok
test channels::tests::writable_codex_uses_workspace_write_sandbox ... ok
test dispatch::tests::live_blind_review ... ignored, real model requests; run once for TASK-03 acceptance
test dispatch::tests::live_cc_switch_route ... ignored, one real cc-switch request; requires AICROSS_CC_SWITCH_PATH and provider
test dispatch::tests::live_economy_pipeline ... ignored, three real model requests; run once outside the sandbox for TASK-04 acceptance
test dispatch::tests::live_shared_continuation ... ignored, two real model requests; run once for TASK-03 acceptance
test dispatch::tests::classification_parser_tolerates_fences_and_missing_arrays ... ok
test dispatch::tests::artifact_diff_covers_deleted_binary_large_and_oversized_markers ... ok
test channels::tests::cc_switch_reads_usage_from_stderr ... ok
test channels::tests::kimi_collects_stream_usage_when_present ... ok
test channels::tests::kimi_session_parser_ignores_resume_text_in_answer ... ok
test dispatch::tests::snapshot_builds_diff_and_changed_file_content ... ok
test dispatch::tests::frontmatter_roundtrips ... ok
test channels::tests::kimi_resume_uses_current_cli_session_flag ... ok
test dispatch::tests::broad_redaction_warning_only_counts_unquoted_whole_lines ... ok
test dispatch::tests::snapshot_marks_files_over_eight_megabytes_without_reading_content ... ok
test dispatch::tests::inspect_state_uses_machine_header_instead_of_status_prose ... ok
test dispatch::tests::trace_fields_are_always_sanitized ... ok
test dispatch::tests::redacts_all_six_secret_shapes_and_stays_clean ... ok
test dispatch::tests::blind_directory_contains_only_selected_materials ... ok
test hook_installer::tests::codex_features_preserves_user_hooks_false_overwrites ... ok
test hook_installer::tests::codex_hooks_json_creates_with_4_events ... ok
test launch::tests::ignores_non_launch_argv ... ok
test launch::tests::parses_equals_form_and_validates_cwd ... ok
test launch::tests::parses_tool_with_and_without_argv0 ... ok
test hook_installer::tests::codex_hooks_json_preserves_user_hooks ... ok
test hook_installer::tests::codex_features_creates_section_if_missing ... ok
test hook_installer::tests::codex_features_enables_hooks_and_strips_legacy ... ok
test launch::tests::rejects_unknown_tool ... ok
test hook_installer::tests::codex_hooks_json_unparseable_left_untouched ... ok
test dispatch::tests::blind_acceptance_rejects_drive_relative_changed_path ... ok
test orchestration::tests::dispatchable_missing_file_reports_error ... ok
test orchestration::tests::dispatchable_broken_file_reports_error ... ok
test hook_installer::tests::kimi_config_blocks_have_only_allowed_fields ... ok
test orchestration::tests::dispatchable_filters_connected_only ... ok
test hook_installer::tests::kimi_config_creates_with_9_events ... ok
test dispatch::tests::blind_acceptance_directory_contains_only_artifact_delivery ... ok
test orchestration::tests::load_missing_graph_returns_none ... ok
test hook_installer::tests::codex_features_idempotent ... ok
test dispatch::tests::state_uses_actual_limit_and_separates_failures ... ok
test orchestration::tests::single_writer_role_defaults_to_connected_codex ... ok
test hook_installer::tests::kimi_config_preserves_existing_content ... ok
test orchestration::tests::presets_broken_file_returns_builtins ... ok
test orchestration::tests::presets_missing_file_returns_builtins ... ok
test server::resume_cwd_tests::does_not_split_on_a_dot_inside_a_component ... ok
test server::resume_cwd_tests::does_not_touch_an_ordinary_hidden_dir_without_worktrees ... ok
test orchestration::tests::graph_roundtrip ... ok
test server::resume_cwd_tests::handles_mixed_separators_and_non_ascii ... ok
test server::resume_cwd_tests::leaves_a_plain_project_dir_unchanged ... ok
test server::resume_cwd_tests::leaves_a_real_subdir_unchanged ... ok
test orchestration::tests::save_rejects_wrong_version ... ok
test orchestration::tests::save_rejects_bad_ids ... ok
test orchestration::tests::list_graphs_skips_broken_files ... ok
test server::resume_cwd_tests::requires_worktrees_to_be_a_whole_next_component ... ok
test server::resume_cwd_tests::strips_only_dot_claude_worktrees_not_other_dot_dir_worktrees ... ok
test server::resume_cwd_tests::strips_unix_worktree_cwd_to_project_root ... ok
test dispatch::tests::preparation_failure_records_only_unsent_placeholder ... ok
test server::resume_cwd_tests::strips_windows_worktree_cwd_to_project_root ... ok
test server::tests::codex_desktop_files_only_block_returns_empty ... ok
test server::tests::codex_desktop_plain_block_passes_through_unchanged ... ok
test hook_installer::tests::codex_hooks_json_idempotent_no_accumulation ... ok
test orchestration::tests::presets_wrong_version_returns_builtins ... ok
test hook_installer::tests::kimi_config_reinstall_strips_stale_keeps_user_hooks ... ok
test resources::tests::connected_filters_status ... ok
test orchestration::tests::presets_roundtrip ... ok
test hook_installer::tests::kimi_config_idempotent_no_duplicates ... ok
test server::resume_cwd_tests::map_is_empty_when_claude_json_is_missing ... ok
test server::tests::codex_parser_drops_subagent_source_unit_variant ... ok
test server::resume_cwd_tests::map_is_empty_and_does_not_panic_on_malformed_json ... ok
test server::resume_cwd_tests::map_has_no_entry_for_an_unregistered_folder ... ok
test server::tests::strips_codex_desktop_file_preamble_to_real_request ... ok
test shell_probe::tests::auto_is_default_for_unknown_string ... ok
test server::tests::codex_parser_drops_subagent_source_object ... ok
test shell_probe::tests::end_to_end_detects_installed_pwsh_with_version ... ignored
test shell_probe::tests::finds_msix_pwsh_alias ... ignored
test server::tests::codex_parser_drops_subagent_thread_source ... ok
test shell_probe::tests::json_mirror_hides_paths ... ok
test shell_probe::tests::parses_windows_ids ... ok
test shell_probe::tests::resolve_gitbash_falls_back_when_absent ... ok
test skills::tests::rel_path_validation ... ok
test server::resume_cwd_tests::map_resolves_second_real_world_project_path ... ok
test shell_probe::tests::resolve_cmd_uses_system_path ... ok
test server::resume_cwd_tests::map_resolves_real_world_hyphenated_project_path ... ok
test shell_probe::tests::resolve_powershell_is_absolute ... ok
test skills::tests::skill_name_validation ... ok
test terminal::tests::find_preset_known_tools ... ok
test terminal::tests::find_preset_resume_program_matches_tool ... ok
test terminal::tests::find_preset_unknown_returns_none ... ok
test terminal::tests::claude_token_valid_uuid ... ok
test orchestration::tests::save_strips_builtin_entries ... ok
test terminal::tests::claude_token_rejects_injection ... ok
test server::tests::codex_parser_keeps_legacy_session_without_thread_source ... ok
test terminal::tests::osc7_absent_returns_none ... ok
test terminal::tests::osc7_basic_with_hostname ... ok
test terminal::tests::osc7_embedded_in_larger_buffer ... ok
test terminal::tests::osc7_no_hostname ... ok
test terminal::tests::osc7_percent_encoded_chinese ... ok
test terminal::tests::osc7_percent_encoded_space ... ok
test terminal::tests::osc7_st_terminator ... ok
test terminal::tests::osc7_windows_style_path ... ok
test server::tests::codex_parser_keeps_user_fork_session ... ok
test tool_config::tests::empty_entry_check ... ok
test tool_config::tests::expand_path_handles_tilde_and_passthrough ... ok
test tool_config::tests::parse_simple ... ok
test terminal::tests::opencode_token_valid_format ... ok
test windows_path::tests::already_present_skipped_case_insensitive_and_trailing_slash ... ok
test windows_path::tests::blank_entries_in_current_dont_shadow_candidates ... ok
test windows_path::tests::duplicate_candidates_deduped ... ok
test windows_path::tests::empty_candidates_returns_current_unchanged ... ok
test windows_path::tests::empty_current_joins_all_candidates ... ok
test windows_path::tests::mixed_present_and_new ... ok
test orchestration::tests::roles_roundtrip_and_fill_economy_nodes ... ok
test terminal::tests::opencode_token_rejects_invalid ... ok
test server::tests::codex_parser_keeps_user_main_session ... ok
test server::tests::counts_array_content_user_message_with_mixed_blocks ... ok
test windows_path::tests::real_path_dirs_reads_registry ... ok
test server::tests::drops_pure_compaction_subtask_session ... ok
test server::tests::keeps_real_session_with_real_user_message ... ok
test server::tests::keeps_session_that_continued_after_compaction ... ok
test terminal::tests::hermes_token_valid_format ... ok
test terminal::tests::hermes_token_rejects_invalid ... ok
test server::tests::drizzle_scanner_excludes_subagent_and_archived_sessions ... ok
test dispatch::tests::project_write_lock_is_mutually_exclusive ... ok
test process::tests::run_process_truncates_and_marks_output ... ok
test process::tests::run_process_times_out_and_terminates_the_child_tree ... ok

test result: ok. 133 passed; 0 failed; 6 ignored; 0 measured; 0 filtered out; finished in 4.27s
```

#### `npx vitest run`（`src-ui`，exit=1）

标准命令因沙箱内 `spawn EPERM` 未能加载 Vite 配置，输出原文：

```text
(node:32976) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:35256) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
failed to load config from D:\work\aicross-p4\src-ui\vite.config.ts

⎯⎯⎯⎯⎯⎯⎯ Startup Error ⎯⎯⎯⎯⎯⎯⎯⎯
Error: Build failed with 1 error:

[plugin externalize-deps]
Error: spawn EPERM
    at ChildProcess.spawn (node:internal/child_process:458:11)
    at spawn (node:child_process:813:9)
    at Object.execFile (node:child_process:349:17)
    at exec (node:child_process:236:25)
    at optimizeSafeRealPathSync (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:1954:2)
    at windowsSafeRealPathSync (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:1940:3)
    at getRealPath (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:32413:36)
    at tryResolveRealFileOrType (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:32407:9)
    at tryCleanFsResolve (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:32151:21)
    at tryFsResolve (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:32144:14)
    at aggregateBindingErrorsIntoJsError (file:///D:/work/aicross-p4/src-ui/node_modules/rolldown/dist/shared/error-DBGOT6sf.mjs:48:18)
    at unwrapBindingResult (file:///D:/work/aicross-p4/src-ui/node_modules/rolldown/dist/shared/error-DBGOT6sf.mjs:18:128)
    at #build (file:///D:/work/aicross-p4/src-ui/node_modules/rolldown/dist/shared/rolldown-build-hRnqgxyz.mjs:3313:34)
    at async bundleConfigFile (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:34633:17)
    at async bundleAndLoadConfigFile (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:34541:18)
    at async loadConfigFromFile (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:34510:42)
    at async resolveConfig (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:34134:22)
    at async resolveConfig$1 (file:///D:/work/aicross-p4/src-ui/node_modules/vitest/dist/chunks/index.B89dZ0-N.js:14773:25)
    at async createVitest (file:///D:/work/aicross-p4/src-ui/node_modules/vitest/dist/chunks/cli-api.LUtK11-x.js:26:17)
    at async prepareVitest (file:///D:/work/aicross-p4/src-ui/node_modules/vitest/dist/chunks/cli-api.LUtK11-x.js:421:14) {
  errors: [Getter/Setter]
}
```

#### `npx vitest run --configLoader native --pool threads`（`src-ui`，exit=0）

按修正单允许的 fallback 复跑，输出原文：

```text
(node:38216) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:44412) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)

 RUN  v5.0.0 D:/work/aicross-p4/src-ui

(node:44412) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:44412) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:44412) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:44412) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:44412) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:44412) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:44412) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:44412) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)

 Test Files  8 passed (8)
      Tests  78 passed (78)
   Start at  22:25:17
   Duration  252ms (transform 60%, import 24%, tests 10%, worker 7%)
```

#### `npm run build`（`src-ui`，exit=0）

```text

> aicross-ui@0.0.0 build
> tsc -b && vite build

(node:42276) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
vite v8.0.7 building client environment for production...
transforming...✓ 571 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                                         2.65 kB │ gzip:   1.16 kB
dist/assets/main-eV4MaA_R.css                         145.31 kB │ gzip:  25.39 kB
dist/assets/dist-js-5OZv1H5C.js                         0.16 kB │ gzip:   0.15 kB
dist/assets/app-XgSAopP-.js                             0.25 kB │ gzip:   0.21 kB
dist/assets/codeowners-YLWRiiGA.js                      0.54 kB │ gzip:   0.31 kB
dist/assets/image-B_hDXdWL.js                           0.61 kB │ gzip:   0.32 kB
dist/assets/shellsession-uwY3V5Kj.js                    0.71 kB │ gzip:   0.42 kB
dist/assets/tsv-FmpeqnFg.js                             0.73 kB │ gzip:   0.34 kB
dist/assets/html-derivative-y8cTjkde.js                 0.83 kB │ gzip:   0.45 kB
dist/assets/git-rebase-BkCIAZbs.js                      0.98 kB │ gzip:   0.44 kB
dist/assets/qmldir-DtRCXXGi.js                          1.00 kB │ gzip:   0.45 kB
dist/assets/csv-BCAXOHVN.js                             1.14 kB │ gzip:   0.37 kB
dist/assets/git-commit-D9uUyrpH.js                      1.22 kB │ gzip:   0.53 kB
dist/assets/xsl-Bo_Acr9l.js                             1.36 kB │ gzip:   0.50 kB
dist/assets/event-CCOELuOC.js                           1.40 kB │ gzip:   0.69 kB
dist/assets/dotenv-BG-R-dnJ.js                          1.42 kB │ gzip:   0.53 kB
dist/assets/sparql-I6g4RV9o.js                          1.47 kB │ gzip:   0.81 kB
dist/assets/ini-D1hC5V4K.js                             1.52 kB │ gzip:   0.50 kB
dist/assets/fortran-fixed-form-Dh8pw333.js              1.66 kB │ gzip:   0.69 kB
dist/assets/docker-BgEUgtmq.js                          1.73 kB │ gzip:   0.60 kB
dist/assets/hxml-_7MT1TaI.js                            1.74 kB │ gzip:   0.88 kB
dist/assets/desktop-DeW1bojR.js                         1.83 kB │ gzip:   0.76 kB
dist/assets/core-CAYfUUdT.js                            1.98 kB │ gzip:   0.82 kB
dist/assets/erb-Bx3F2RzE.js                             2.13 kB │ gzip:   0.62 kB
dist/assets/wenyan-B7WaDmpb.js                          2.15 kB │ gzip:   1.09 kB
dist/assets/jssm-nqlDcCik.js                            2.23 kB │ gzip:   0.61 kB
dist/assets/edge-BMCHHRKD.js                            2.30 kB │ gzip:   0.67 kB
dist/assets/reg-C9YB0Q6j.js                             2.34 kB │ gzip:   0.69 kB
dist/assets/diff-IPIU9Fzm.js                            2.56 kB │ gzip:   0.69 kB
dist/assets/gleam-Bqam2yKl.js                           2.57 kB │ gzip:   0.82 kB
dist/assets/defineProperty-Cc0ioC06.js                  2.59 kB │ gzip:   1.30 kB
dist/assets/hy-B7p7U1BT.js                              2.64 kB │ gzip:   1.18 kB
dist/assets/openscad-C-9uKvUO.js                        2.82 kB │ gzip:   1.01 kB
dist/assets/log-vQtdJHWP.js                             2.85 kB │ gzip:   0.90 kB
dist/assets/json-DG5xeaaD.js                            2.88 kB │ gzip:   0.81 kB
dist/assets/cairo-ClvR2owv.js                           2.93 kB │ gzip:   0.80 kB
dist/assets/berry-BUpy9_cc.js                           3.00 kB │ gzip:   0.81 kB
dist/assets/jsonl-DhVam5Gu.js                           3.00 kB │ gzip:   0.78 kB
dist/assets/jsonc-BK4mmDTw.js                           3.10 kB │ gzip:   0.79 kB
dist/assets/logo-CCxtNjv2.js                            3.12 kB │ gzip:   1.47 kB
dist/assets/po-CJUqynl0.js                              3.23 kB │ gzip:   0.91 kB
dist/assets/json5-B0H0ftRm.js                           3.25 kB │ gzip:   0.83 kB
dist/assets/mipsasm-BLGWrDdP.js                         3.25 kB │ gzip:   1.17 kB
dist/assets/tasl-BGeyykQ_.js                            3.28 kB │ gzip:   0.84 kB
dist/assets/webview-Dcvwb5z8.js                         3.31 kB │ gzip:   1.12 kB
dist/assets/genie-CN4rq7A0.js                           3.35 kB │ gzip:   1.21 kB
dist/assets/rel-DifwsMId.js                             3.36 kB │ gzip:   1.11 kB
dist/assets/vala-C6OpFbMA.js                            3.36 kB │ gzip:   1.19 kB
dist/assets/splunk-Cf3CW7wI.js                          3.43 kB │ gzip:   1.52 kB
dist/assets/hurl-2ACva3j7.js                            3.51 kB │ gzip:   1.10 kB
dist/assets/fluent-DGOyQ8Ws.js                          3.60 kB │ gzip:   0.90 kB
dist/assets/ssh-config-CKUNcAMa.js                      3.61 kB │ gzip:   1.60 kB
dist/assets/jsonnet-BOcQ7pWS.js                         3.61 kB │ gzip:   1.04 kB
dist/assets/kdl-DBigZy8w.js                             3.62 kB │ gzip:   1.04 kB
dist/assets/narrat-Cq3CR6rl.js                          3.67 kB │ gzip:   1.10 kB
dist/assets/turtle-D_eMCpxI.js                          3.69 kB │ gzip:   0.97 kB
dist/assets/glsl-DtQQAfSz.js                            3.70 kB │ gzip:   1.46 kB
dist/assets/zenscript-wcLUgsD2.js                       3.91 kB │ gzip:   1.28 kB
dist/assets/ron-ChSsJUxP.js                             3.91 kB │ gzip:   0.98 kB
dist/assets/gn-DJDjd0xf.js                              4.00 kB │ gzip:   1.48 kB
dist/assets/pascal-BHiDpJdH.js                          4.14 kB │ gzip:   1.67 kB
dist/assets/http-duTJ0maY.js                            4.41 kB │ gzip:   1.06 kB
dist/assets/tcl-CGMQ2Gre.js                             4.42 kB │ gzip:   1.50 kB
dist/assets/nextflow-DYYpq5fR.js                        4.50 kB │ gzip:   1.16 kB
dist/assets/rosmsg-C8yR4p9t.js                          4.52 kB │ gzip:   1.06 kB
dist/assets/polar-BiOMIDty.js                           4.66 kB │ gzip:   1.12 kB
dist/assets/sdbl-DbzyTi_y.js                            4.70 kB │ gzip:   2.01 kB
dist/assets/fennel-Cp7mIZmg.js                          4.76 kB │ gzip:   1.54 kB
dist/assets/bibtex-CMf_khHV.js                          4.79 kB │ gzip:   0.83 kB
dist/assets/llvm-BUQffIyu.js                            5.04 kB │ gzip:   2.02 kB
dist/assets/wgsl-8R5OMxfU.js                            5.14 kB │ gzip:   1.39 kB
dist/assets/gdresource-VMh-5Lix.js                      5.28 kB │ gzip:   1.34 kB
dist/assets/zig-7Lbxfjhx.js                             5.33 kB │ gzip:   1.55 kB
dist/assets/qml-DV-9MRuy.js                             5.33 kB │ gzip:   1.38 kB
dist/assets/dax-DlTOXg5S.js                             5.36 kB │ gzip:   2.22 kB
dist/assets/bicep-mN0ruteN.js                           5.38 kB │ gzip:   1.15 kB
dist/assets/xml-CyIjMK9K.js                             5.45 kB │ gzip:   1.25 kB
dist/assets/awk-C2cQpOjH.js                             5.45 kB │ gzip:   1.38 kB
dist/assets/coq-M9LrrHou.js                             5.52 kB │ gzip:   1.91 kB
dist/assets/jinja-CaX9vMDt.js                           5.62 kB │ gzip:   1.36 kB
dist/assets/lean-Dtsklvza.js                            5.78 kB │ gzip:   1.92 kB
dist/assets/moonbit-DelbyvlD.js                         5.89 kB │ gzip:   1.68 kB
dist/assets/powerquery-DDoDbTf0.js                      5.90 kB │ gzip:   1.52 kB
dist/assets/shaderlab-C-QDXGQP.js                       5.92 kB │ gzip:   2.08 kB
dist/assets/verilog-BNuILg-8.js                         5.92 kB │ gzip:   1.89 kB
dist/assets/cypher-Bf0d0j36.js                          5.95 kB │ gzip:   1.72 kB
dist/assets/vb-vQ7FBk14.js                              6.09 kB │ gzip:   2.34 kB
dist/assets/red-CgmMwQ5s.js                             6.26 kB │ gzip:   1.60 kB
dist/assets/min-dark-CBFjRQ4v.js                        6.28 kB │ gzip:   1.70 kB
dist/assets/gdshader-DLlTfkMm.js                        6.32 kB │ gzip:   1.72 kB
dist/assets/prisma-CPh4c3GQ.js                          6.32 kB │ gzip:   1.39 kB
dist/assets/ara-CpyWD-c6.js                             6.36 kB │ gzip:   1.81 kB
dist/assets/clojure-C3jjSFuY.js                         6.41 kB │ gzip:   1.42 kB
dist/assets/postcss-DAA_T9V0.js                         6.41 kB │ gzip:   1.91 kB
dist/assets/toml-BlynkCpr.js                            6.42 kB │ gzip:   1.28 kB
dist/assets/solarized-light-DLLndLS4.js                 6.47 kB │ gzip:   1.71 kB
dist/assets/proto-C0LdvFNL.js                           6.54 kB │ gzip:   1.42 kB
dist/assets/smalltalk-BIVJlCyX.js                       6.58 kB │ gzip:   1.62 kB
dist/assets/r-B4zVi_fs.js                               6.61 kB │ gzip:   1.83 kB
dist/assets/talonscript-Dr-lPTln.js                     6.75 kB │ gzip:   1.49 kB
dist/assets/solarized-dark-DgqPf06l.js                  6.84 kB │ gzip:   1.78 kB
dist/assets/riscv-BM628xIq.js                           6.91 kB │ gzip:   1.98 kB
dist/assets/soy-jLaB-7Vb.js                             6.92 kB │ gzip:   1.62 kB
dist/assets/min-light-OvUhfe4y.js                       6.96 kB │ gzip:   1.88 kB
dist/assets/scheme-BIfFz6Pe.js                          7.16 kB │ gzip:   2.05 kB
dist/assets/hlsl-I4TUSC5a.js                            7.26 kB │ gzip:   2.19 kB
dist/assets/qss-BgVkvApy.js                             7.47 kB │ gzip:   2.57 kB
dist/assets/systemd-D4187P0y.js                         7.86 kB │ gzip:   2.54 kB
dist/assets/monokai-DHFwemSO.js                         7.88 kB │ gzip:   1.90 kB
dist/assets/regexp-BnAW9hSB.js                          8.05 kB │ gzip:   1.47 kB
dist/assets/haml-DtItqECI.js                            8.34 kB │ gzip:   1.86 kB
dist/assets/typst-BP5N7E5y.js                           8.38 kB │ gzip:   1.67 kB
dist/assets/vue-html-C5v0WTbT.js                        8.47 kB │ gzip:   1.67 kB
dist/assets/plsql-DEXRAcVm.js                           8.51 kB │ gzip:   2.99 kB
dist/assets/dart-3JwWll0a.js                            8.62 kB │ gzip:   2.00 kB
dist/assets/horizon-C9nqL9R6.js                         8.77 kB │ gzip:   1.95 kB
dist/assets/kotlin-BYL6QRVS.js                          8.78 kB │ gzip:   2.13 kB
dist/assets/horizon-bright-Css32xyz.js                  8.78 kB │ gzip:   1.96 kB
dist/assets/make-Dw3yrZ0Q.js                            8.95 kB │ gzip:   1.77 kB
dist/assets/andromeeda-DGoCYgJo.js                      9.01 kB │ gzip:   2.36 kB
dist/assets/sas-CbDNbSM0.js                             9.06 kB │ gzip:   3.82 kB
dist/assets/dark-plus-DY33niKa.js                       9.09 kB │ gzip:   2.10 kB
dist/assets/slack-dark-BAvxGu1N.js                      9.11 kB │ gzip:   1.96 kB
dist/assets/sass-DocrZT3r.js                            9.28 kB │ gzip:   2.49 kB
dist/assets/plastic-iniznJdO.js                         9.29 kB │ gzip:   1.97 kB
dist/assets/slack-ochin-DWqn_Lk-.js                     9.42 kB │ gzip:   2.09 kB
dist/assets/tex-6AKuZd1g.js                             9.67 kB │ gzip:   3.06 kB
dist/assets/jison--pZ8m-HK.js                           9.69 kB │ gzip:   1.87 kB
dist/assets/cmake-DrqqGEK_.js                           9.85 kB │ gzip:   3.38 kB
dist/assets/light-plus-DfRHv4Xo.js                      9.93 kB │ gzip:   2.27 kB
dist/assets/hcl-WYMZvHje.js                            10.05 kB │ gzip:   1.93 kB
dist/assets/rst-CK2J_xNG.js                            10.30 kB │ gzip:   2.25 kB
dist/assets/pkl-DawJyhw3.js                            10.37 kB │ gzip:   1.38 kB
dist/assets/beancount-QTI4W_q2.js                      10.37 kB │ gzip:   1.43 kB
dist/assets/nextflow-groovy-A4by5qzv.js                10.43 kB │ gzip:   2.14 kB
dist/assets/dream-maker-C3tt4vD1.js                    10.46 kB │ gzip:   2.26 kB
dist/assets/raku-Ba22PpAY.js                           10.47 kB │ gzip:   2.94 kB
dist/assets/yaml-CzLfUaQ1.js                           10.57 kB │ gzip:   2.32 kB
dist/assets/just-RTSWN6SD.js                           10.77 kB │ gzip:   2.60 kB
dist/assets/elm-D_V6tPeg.js                            10.94 kB │ gzip:   2.10 kB
dist/assets/github-light-CIJNwzyc.js                   11.18 kB │ gzip:   2.49 kB
dist/assets/prolog-BCLGsSxa.js                         11.35 kB │ gzip:   3.85 kB
dist/assets/terraform-DfAmkD3Z.js                      11.38 kB │ gzip:   2.51 kB
dist/assets/github-dark-OFaljh7J.js                    11.40 kB │ gzip:   2.53 kB
dist/assets/puppet-BI7VePLm.js                         11.43 kB │ gzip:   2.11 kB
dist/assets/laserwave-CQtCIGFc.js                      11.49 kB │ gzip:   2.57 kB
dist/assets/gherkin-Cr0wk206.js                        11.94 kB │ gzip:   5.05 kB
dist/assets/wasm-Beqnk49Y.js                           12.00 kB │ gzip:   2.18 kB
dist/assets/hjson-CbFL5VWg.js                          12.05 kB │ gzip:   1.64 kB
dist/assets/handlebars-Dit8JC0x.js                     12.16 kB │ gzip:   2.38 kB
dist/assets/window-Cf5upF_J.js                         12.41 kB │ gzip:   3.01 kB
dist/assets/apache-mOm_gFPl.js                         12.45 kB │ gzip:   3.71 kB
dist/assets/vesper-B0XbIQa2.js                         12.69 kB │ gzip:   1.98 kB
dist/assets/bat-CD8xesoU.js                            12.88 kB │ gzip:   3.23 kB
dist/assets/fish-Bw4coD8r.js                           13.03 kB │ gzip:   1.73 kB
dist/assets/v-DG0D3hZq.js                              13.21 kB │ gzip:   2.73 kB
dist/assets/vitesse-light-piMchJtN.js                  13.61 kB │ gzip:   3.03 kB
dist/assets/aurora-x-WbeSdnAE.js                       13.65 kB │ gzip:   2.28 kB
dist/assets/vitesse-black-DuzBzXNC.js                  13.67 kB │ gzip:   3.06 kB
dist/assets/vitesse-dark-ChPfzWLu.js                   13.75 kB │ gzip:   3.06 kB
dist/assets/pug-CM6j4wdg.js                            13.85 kB │ gzip:   2.58 kB
dist/assets/luau-CsHfLQlf.js                           13.96 kB │ gzip:   3.17 kB
dist/assets/synthwave-84-B1wrF6Pb.js                   14.03 kB │ gzip:   2.85 kB
dist/assets/github-light-default-Cskbnfwe.js           14.15 kB │ gzip:   3.02 kB
dist/assets/clarity-zNHM1tMD.js                        14.27 kB │ gzip:   2.47 kB
dist/assets/github-light-high-contrast-BUZL_cRA.js     14.27 kB │ gzip:   3.00 kB
dist/assets/github-dark-dimmed-DLGfz6FO.js             14.43 kB │ gzip:   3.11 kB
dist/assets/github-dark-default-BqbMvdcT.js            14.43 kB │ gzip:   3.12 kB
dist/assets/github-dark-high-contrast-wBnEARx0.js      14.59 kB │ gzip:   3.08 kB
dist/assets/gnuplot-Bd1MpVfM.js                        14.78 kB │ gzip:   3.28 kB
dist/assets/rust-jpCPbsEK.js                           15.06 kB │ gzip:   2.72 kB
dist/assets/kusto-COSTdHqw.js                          15.17 kB │ gzip:   3.93 kB
dist/assets/actionscript-3-LMiqWuSE.js                 15.20 kB │ gzip:   2.68 kB
dist/assets/nix-BTV8Kl4V.js                            15.50 kB │ gzip:   2.48 kB
dist/assets/lua-BB1sjGF9.js                            15.60 kB │ gzip:   3.19 kB
dist/assets/abap-s17gYGWC.js                           15.85 kB │ gzip:   5.93 kB
dist/assets/solidity-D-b1Sb1s.js                       16.07 kB │ gzip:   3.10 kB
dist/assets/matlab-D7Ymh7TH.js                         16.08 kB │ gzip:   3.07 kB
dist/assets/cue-CHmhgwIJ.js                            16.20 kB │ gzip:   2.05 kB
dist/assets/elixir-1Wj0-XTx.js                         16.26 kB │ gzip:   2.76 kB
dist/assets/odin-CzGDjIwK.js                           16.51 kB │ gzip:   2.93 kB
dist/assets/ts-tags-NZ2iHfv6.js                        16.54 kB │ gzip:   2.05 kB
dist/assets/kanagawa-wave-D_Hm3vX6.js                  17.12 kB │ gzip:   2.90 kB
dist/assets/kanagawa-lotus-CqgQE5Hl.js                 17.12 kB │ gzip:   2.91 kB
dist/assets/kanagawa-dragon-CByus8H8.js                17.12 kB │ gzip:   2.92 kB
dist/assets/move-BG_bNIu4.js                           17.50 kB │ gzip:   3.07 kB
dist/assets/graphql-DZnKSmmH.js                        18.08 kB │ gzip:   2.54 kB
dist/assets/liquid-DAb-uuE2.js                         18.10 kB │ gzip:   3.15 kB
dist/assets/svelte-CPH69QWw.js                         18.25 kB │ gzip:   3.14 kB
dist/assets/material-theme-CY5ZxTT5.js                 18.61 kB │ gzip:   3.10 kB
dist/assets/material-theme-darker-BnDPOEon.js          18.62 kB │ gzip:   3.10 kB
dist/assets/material-theme-ocean-CAu3HLd7.js           18.62 kB │ gzip:   3.12 kB
dist/assets/material-theme-lighter-CXYE62LH.js         18.63 kB │ gzip:   3.11 kB
dist/assets/material-theme-palenight-BLPSe7SZ.js       18.63 kB │ gzip:   3.12 kB
dist/assets/gdscript-C2D5WM82.js                       18.99 kB │ gzip:   3.75 kB
dist/assets/groovy-BeOakm-O.js                         19.17 kB │ gzip:   3.60 kB
dist/assets/mdc-C5YQj5X-.js                            19.54 kB │ gzip:   6.67 kB
dist/assets/ayu-dark-C8AZSEqd.js                       20.07 kB │ gzip:   3.93 kB
dist/assets/ayu-mirage-DJDt9_Ik.js                     20.08 kB │ gzip:   3.91 kB
dist/assets/glimmer-js-Z7X_LugF.js                     20.08 kB │ gzip:   2.94 kB
dist/assets/glimmer-ts-CmBNQRbz.js                     20.08 kB │ gzip:   2.94 kB
dist/assets/powershell-DThFFtVM.js                     20.14 kB │ gzip:   4.06 kB
dist/assets/ayu-light-DGy4vufb.js                      20.15 kB │ gzip:   3.90 kB
dist/assets/viml-CP_eTIVX.js                           20.36 kB │ gzip:   6.74 kB
dist/assets/nushell-_kwMquEq.js                        20.41 kB │ gzip:   5.22 kB
dist/assets/snazzy-light-DEGtvnBx.js                   20.77 kB │ gzip:   3.81 kB
dist/assets/twig-EY5SnVCc.js                           20.90 kB │ gzip:   3.66 kB
dist/assets/dracula-C8tduV4h.js                        21.06 kB │ gzip:   4.00 kB
dist/assets/dracula-soft-i03TrcGR.js                   21.07 kB │ gzip:   4.02 kB
dist/assets/wit-Q3uzgIzy.js                            21.46 kB │ gzip:   2.88 kB
dist/assets/rose-pine-IZyio8vo.js                      21.74 kB │ gzip:   3.86 kB
dist/assets/rose-pine-moon-BfRCxBHe.js                 21.75 kB │ gzip:   3.88 kB
dist/assets/rose-pine-dawn-D1oZXPoh.js                 21.75 kB │ gzip:   3.88 kB
dist/assets/nim-mCDe1XWm.js                            22.45 kB │ gzip:   3.16 kB
dist/assets/common-lisp-D0DsnuEJ.js                    22.57 kB │ gzip:   6.07 kB
dist/assets/surrealql-DhK7dDdV.js                      22.58 kB │ gzip:   4.31 kB
dist/assets/gruvbox-dark-hard-BqtMDTUh.js              22.63 kB │ gzip:   4.16 kB
dist/assets/gruvbox-dark-soft-CV57n6pA.js              22.63 kB │ gzip:   4.15 kB
dist/assets/gruvbox-light-hard-b4k0L1XS.js             22.63 kB │ gzip:   4.16 kB
dist/assets/gruvbox-light-soft-DP66dnY9.js             22.63 kB │ gzip:   4.16 kB
dist/assets/gruvbox-dark-medium-Dgfy8-dz.js            22.63 kB │ gzip:   4.16 kB
dist/assets/gruvbox-light-medium-DSewW6QF.js           22.63 kB │ gzip:   4.16 kB
dist/assets/bird2-Bh111mYM.js                          22.94 kB │ gzip:   4.66 kB
dist/assets/sql-DcHRMm-Q.js                            23.47 kB │ gzip:   7.49 kB
dist/assets/cadence-DXH0Dgwr.js                        23.66 kB │ gzip:   3.66 kB
dist/assets/astro-XmnDH_Dd.js                          24.02 kB │ gzip:   7.60 kB
dist/assets/apl-DjqofZJD.js                            24.03 kB │ gzip:   4.17 kB
dist/assets/templ-BkokcB7D.js                          24.06 kB │ gzip:   5.41 kB
dist/assets/angular-html-BlerCZUb.js                   24.25 kB │ gzip:   3.98 kB
dist/assets/vhdl-3hDyUkZR.js                           24.26 kB │ gzip:   3.86 kB
dist/assets/vue-CqFBltHt.js                            24.50 kB │ gzip:   2.97 kB
dist/assets/purescript-DKMYP_A1.js                     24.68 kB │ gzip:   3.24 kB
dist/assets/typespec-BQy10TzM.js                       25.04 kB │ gzip:   2.62 kB
dist/assets/one-light-27e-9kYd.js                      25.29 kB │ gzip:   3.63 kB
dist/assets/fsharp-BQAqzjqk.js                         25.30 kB │ gzip:   4.13 kB
dist/assets/marko-Cj1_9Cb9.js                          25.48 kB │ gzip:   3.59 kB
dist/assets/c3-qRBCI8Uv.js                             25.62 kB │ gzip:   3.86 kB
dist/assets/night-owl-light-DtpYOdyl.js                25.90 kB │ gzip:   4.24 kB
dist/assets/system-verilog-jOLk1CBm.js                 26.20 kB │ gzip:   4.83 kB
dist/assets/nord-BQkWtC2v.js                           26.72 kB │ gzip:   4.36 kB
dist/assets/codeql-BoicTpVa.js                         26.88 kB │ gzip:   3.80 kB
dist/assets/scss-Dw42T1BI.js                           27.27 kB │ gzip:   4.23 kB
dist/assets/java-CoApPKT4.js                           27.28 kB │ gzip:   4.31 kB
dist/assets/coffee-fAb6LsdV.js                         27.42 kB │ gzip:   6.35 kB
dist/assets/razor-ccbgdA8j.js                          27.45 kB │ gzip:   3.54 kB
dist/assets/scala-D0LjI2Qi.js                          28.88 kB │ gzip:   3.93 kB
dist/assets/night-owl-DDmCLEp1.js                      28.91 kB │ gzip:   5.15 kB
dist/assets/crystal-HHV-Zo9X.js                        29.41 kB │ gzip:   4.44 kB
dist/assets/mermaid-CAtOlIbf.js                        29.50 kB │ gzip:   3.65 kB
dist/assets/applescript-C5yVau2U.js                    29.56 kB │ gzip:   5.95 kB
dist/assets/julia-jUIBt023.js                          30.99 kB │ gzip:   4.30 kB
dist/assets/stylus-CNnrGufP.js                         31.07 kB │ gzip:   8.02 kB
dist/assets/poimandres-H4_Eyox2.js                     33.49 kB │ gzip:   5.49 kB
dist/assets/one-dark-pro-C4hn0Zoq.js                   33.78 kB │ gzip:   5.50 kB
dist/assets/bsl-ny3e_7dl.js                            33.86 kB │ gzip:   8.35 kB
dist/assets/haxe-C2AyQsKn.js                           35.16 kB │ gzip:   5.91 kB
dist/assets/nginx-B8zy1DQG.js                          35.35 kB │ gzip:   4.46 kB
dist/assets/houston-ByNQ9eAt.js                        35.42 kB │ gzip:   5.76 kB
dist/assets/tokyo-night-DQIh82d3.js                    35.66 kB │ gzip:   6.23 kB
dist/assets/erlang-Q-lfVOgV.js                         37.47 kB │ gzip:   4.36 kB
dist/assets/cobol-DQafa4Ab.js                          39.07 kB │ gzip:  10.89 kB
dist/assets/asm-Bbv1919r.js                            40.71 kB │ gzip:   8.17 kB
dist/assets/haskell-DTAtmdln.js                        41.48 kB │ gzip:   6.41 kB
dist/assets/shellscript-3Eyrpdfm.js                    41.54 kB │ gzip:   6.13 kB
dist/assets/perl-CEA6MP1k.js                           43.14 kB │ gzip:   4.65 kB
dist/assets/d-Dt1IpaRS.js                              43.79 kB │ gzip:   8.43 kB
dist/assets/ruby-B-fKc9x2.js                           46.60 kB │ gzip:   5.71 kB
dist/assets/go-D3d4JNoO.js                             46.81 kB │ gzip:   5.15 kB
dist/assets/apex-Bkns-Kf9.js                           46.98 kB │ gzip:   6.73 kB
dist/assets/catppuccin-mocha-D4_Pt6ay.js               47.25 kB │ gzip:   7.98 kB
dist/assets/catppuccin-latte-Coqss7en.js               47.25 kB │ gzip:   7.99 kB
dist/assets/catppuccin-frappe-C5l_8kFT.js              47.25 kB │ gzip:   7.99 kB
dist/assets/catppuccin-macchiato-DEc0mBz1.js           47.26 kB │ gzip:   7.99 kB
dist/assets/ada-CgdG45k-.js                            48.08 kB │ gzip:   5.96 kB
dist/assets/css-BaIEaAP4.js                            49.11 kB │ gzip:  11.96 kB
dist/assets/imba-D2BupS_o.js                           49.92 kB │ gzip:   9.47 kB
dist/assets/everforest-dark-DnGtViwz.js                53.74 kB │ gzip:   8.37 kB
dist/assets/everforest-light-CrRagu-i.js               53.74 kB │ gzip:   8.37 kB
dist/assets/wikitext-CokaWg4C.js                       55.88 kB │ gzip:   4.82 kB
dist/assets/stata-LzqjS0NL.js                          56.98 kB │ gzip:  12.39 kB
dist/assets/html-B1M_0gd8.js                           57.32 kB │ gzip:  11.79 kB
dist/assets/ballerina-C5NCmhdh.js                      58.69 kB │ gzip:   8.11 kB
dist/assets/markdown-DrnMgzIX.js                       59.33 kB │ gzip:   5.67 kB
dist/assets/ocaml-X3W8brJ7.js                          62.44 kB │ gzip:   5.05 kB
dist/assets/mojo-BEOkTXe0.js                           69.79 kB │ gzip:   9.22 kB
dist/assets/python-BtjLSkiI.js                         69.95 kB │ gzip:   9.10 kB
dist/assets/c-yOg8G6Qk.js                              72.17 kB │ gzip:  10.55 kB
dist/assets/latex-DFkYhUza.js                          72.61 kB │ gzip:   6.69 kB
dist/assets/vyper-BGjJ65g4.js                          74.64 kB │ gzip:  10.70 kB
dist/assets/hack-CB6YBmfz.js                           80.18 kB │ gzip:  26.27 kB
dist/assets/swift-ByDBHGEa.js                          86.68 kB │ gzip:  14.69 kB
dist/assets/fortran-free-form-BXrNE5X_.js              88.96 kB │ gzip:  11.19 kB
dist/assets/csharp-DK3gKUDm.js                         89.68 kB │ gzip:  10.64 kB
dist/assets/racket-DChGz2wN.js                         92.38 kB │ gzip:  15.08 kB
dist/assets/less-CCCdi_4d.js                           97.63 kB │ gzip:  14.85 kB
dist/assets/blade-DhyUajd-.js                         104.97 kB │ gzip:  28.30 kB
dist/assets/objective-c-Do3Dh539.js                   105.41 kB │ gzip:  23.64 kB
dist/assets/php-D1n20mva.js                           111.31 kB │ gzip:  28.66 kB
dist/assets/asciidoc-Mnh7mpqW.js                      131.52 kB │ gzip:   9.32 kB
dist/assets/mdx-BhxtouIL.js                           136.11 kB │ gzip:  23.54 kB
dist/assets/objective-cpp-DzvX7O7S.js                 171.97 kB │ gzip:  30.98 kB
dist/assets/javascript-DMDj563O.js                    174.89 kB │ gzip:  16.68 kB
dist/assets/tsx-Du47Vi3R.js                           175.60 kB │ gzip:  16.67 kB
dist/assets/jsx-BgYHCIzu.js                           177.86 kB │ gzip:  16.77 kB
dist/assets/typescript-Biw9qNLM.js                    181.14 kB │ gzip:  16.29 kB
dist/assets/angular-ts-BErpjn_O.js                    183.73 kB │ gzip:  16.77 kB
dist/assets/vue-vine-DaNosUn7.js                      190.06 kB │ gzip:  18.07 kB
dist/assets/dist-CbAFeoTq.js                          212.97 kB │ gzip:  67.93 kB
dist/assets/wolfram-BPifCLvx.js                       262.38 kB │ gzip:  77.64 kB
dist/assets/wasm-DA6mYL5T.js                          622.32 kB │ gzip: 232.09 kB
dist/assets/cpp-Cyj1bR0k.js                           626.13 kB │ gzip:  48.10 kB
dist/assets/emacs-lisp-DzXDqNne.js                    779.87 kB │ gzip: 197.56 kB
dist/assets/main-DNwN4V9z.js                        1,228.36 kB │ gzip: 440.07 kB

✓ built in 2.17s
[plugin builtin:vite-reporter] 
(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rolldownOptions.output.codeSplitting to improve chunking: https://rolldown.rs/reference/OutputOptions.codeSplitting
- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
```

## 修正轮 R2

### 逐条处理

1. 已修：盲验收目录交付后，prompt 追加任务书指定的待验材料提示；上游 diff 总量小于 30 KB 时同时内联全文。pi 盲节点在收到结构化产物时由 `--no-tools` 改为只读 `read,grep,find,ls`，无产物的 pi 盲节点仍保持无工具。补了提示行、内联 diff、目录三项和 pi argv 回归测试。
2. 已修：非盲可写 Claude 启用 `Read,Grep,Glob,Write,Edit,Bash`，pi 启用 `read,write,edit,bash,grep,find,ls`；设置页不再把执行角色候选限制为 Codex，经济模式角色填充也不再把已配置的 Claude/pi 单写者强制改成 Codex。补了三通道 argv 与角色填充测试。
3. 已修：执行节点的链式启发式覆盖 `thinking=关|低`，命中后抬到“中”，确认页显示“任务含数字或链式关键词，thinking 已调为中；可修改”；内置预设默认值未改。
4. 已修：`appendCorrectionRound` 新执行节点的指定上游固定重写为 `[acceptId, previous.id]`，使修正指令和上一轮执行产物同时进入上下文；前端测试锁定顺序与内容。
5. 已修：Git 项目把前后快照按原字节落到运行目录内临时区，调用 `git diff --no-index --binary --unified=3` 生成可用 unified diff，保留 CRLF 字节；非 Git 回退实现增加 200 KB 单文件 diff 上限，超限只写文件名和“全文见 changed/”。Kimi 在 30000 字节硬限前按 UTF-8 边界保留首尾并标注裁剪。
6. 已修：快照改用 `symlink_metadata`，跳过符号链接与 Windows reparse point/junction；排除目录补 `.venv`、`build`、`coverage`、`.next`、`vendor`；设置 8 MB 单文件读取上限和 128 MB 总读取上限。前后快照或 diff 构建失败均写成 `status=error`，不再伪装为空产物。
7. 已修：阶段调度前执行 `git rev-parse --is-inside-work-tree`；非 Git 项目的 worktree 节点预先按单写者记账和排队，避免回退后与另一写者并发。
8. 已修：写锁换成按规范化项目路径共享的 `Mutex + Condvar + FIFO VecDeque`；等待使用 `timeout_secs` 截止时间，取消会写入取消标记并唤醒所有锁等待者。补了互斥与等待取消测试。
9. 已修：`DispatchNodeRequest.stage` 成为逐节点字段；运行结果、派发记录和用量分组均使用节点自己的 stage，批次 stage 仅保留兼容与 STATE 摘要用途。补了不同节点 stage 的结果/记录测试。
10. 已修：首轮确认只取 `dispatchableNodeIds`；自动推进每层派发前重新构造请求并刷新 preview。设置新增总请求上限，默认 8、范围 1–100；累计请求即将越界时停止并提示需要重新确认，后端也拒绝单批超限。补了分组与上限纯函数测试。
11. 已修：G4 的 `addedCorrection` 使用或累计；验证失败按 nodeId 逐个追加重试，重试计数跟随各自新执行节点，任一分支达到 3 次即停止；模型正文保留，验证输出以 `[验证输出]` 单独追加。补了多节点失败、逐节点计数和正文保留测试。
12. 已修：Rust/TypeScript `Reply` 追加可选 `error_kind`；失败写回时保存后端状态。`STATE.md`、结果浮层和图节点卡分别显示“真身不符”“验证失败”，普通失败仍显示“失败”。上方“对第 2/3 期契约的字段追加”已同步兼容规则，并补了前端真身不符写回测试。
13. 已修：结果按 `result.stage` 排序分组；栏标题取该阶段首个节点的 role，空角色退化为“第 N 阶段”；CSS 网格列数由阶段数动态设置。
14. 已修：cc-switch usage 单测样本换成真实字段形态 `[usage] provider=p model=m input=20 fresh=4 cache_create=6 cache_read=10 output=7 turns=1 ms=100`；`live_cc_switch_route` 增加 `tokens_in > 0` 断言。
15. 已修：`live_economy_pipeline` 对验收工作目录先排序再精确断言根目录只有 `REQUEST.md`、`changed`、`diff.patch`，并另行断言项目文件 `roman.py` 不存在。
16. 已修：方案、执行、验收模板抽到 `fixtures/prompts/planning.txt`、`execution.txt`、`acceptance.txt`；盲前缀也抽到 `blind-prefix.txt`。TypeScript 通过 raw import、Rust 通过 `include_str!` 读取同一组文件，live harness 使用这些模板拼装 prompt。

### qwen 审查第 3–6 节中已由 R1 覆盖的条目

- R1 已修：4.4 已明确把归类解析标成未接线过渡代码；4.5 已给两套原子写实现加交叉说明，合并留到第 5 期。
- R1 已修：第 5 节的验收修正上限、验证重试上限，以及删除/二进制/>200 KB diff 边界测试；R2 在此基础上补真实 Git unified diff、CRLF/上下文、锁取消、阶段、批量失败与总请求上限。
- R1 已修：第 6 节报告已明确 live harness 尚未经过前端 `serializeContext` 的边界；R2 进一步共用 prompt 文件，并加入盲目录精确集合、请求数和 cc-switch tokens 输入断言。

本轮没有启动窗口，没有发真机模型请求，没有 commit 或 push。

### 验证

以下为本轮重新采集的实际命令及输出；终端 ANSI 色码已去除，文本未改写。Vite 在当前 Windows 沙箱中存在间歇性子进程 `spawn EPERM`：任务书原始 Vitest 命令的失败原样保留；同套测试用原生配置加载器、单 worker 复跑通过。生产构建最终通过。

#### `cargo check`（仓库根目录，exit=0）

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.92s
```

#### `cargo test`（仓库根目录，exit=0）

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p4)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 6.87s
     Running unittests src\main.rs (target\debug\deps\aicross-a390c24a3e96ccb6.exe)

running 147 tests
test channels::tests::codex_high_maps_to_high_and_never_to_xhigh ... ok
test channels::tests::blind_pi_with_no_context_files_is_fully_isolated ... ok
test channels::tests::cc_switch_requires_configured_script_path ... ok
test channels::tests::blind_pi_with_artifacts_keeps_read_only_tools ... ok
test channels::tests::blind_claude_contains_restricted ... ok
test channels::tests::codex_resume_follows_parent_options ... ok
test channels::tests::kimi_requires_configured_alias ... ok
test channels::tests::pi_continuation_preserves_thinking_setting ... ok
test channels::tests::claude_null_api_error_status_is_not_an_error ... ok
test channels::tests::pi_error_event_surfaces_error_message ... ok
test channels::tests::pi_reads_camel_case_cache_usage ... ok
test channels::tests::kimi_reads_plain_string_assistant_content_and_resume_hint ... ok
test channels::tests::pi_response_model_mismatch_fails_and_reports_true_identity ... ok
test channels::tests::pi_uses_explicit_session_and_app_owned_session_directory ... ok
test channels::tests::scheduling_aliases_normalize_to_one_agent_key ... ok
test channels::tests::writable_claude_enables_write_tools ... ok
test channels::tests::writable_codex_uses_workspace_write_sandbox ... ok
test dispatch::tests::classification_parser_tolerates_fences_and_missing_arrays ... ok
test dispatch::tests::live_blind_review ... ignored, real model requests; run once for TASK-03 acceptance
test channels::tests::writable_pi_enables_write_tools ... ok
test dispatch::tests::live_cc_switch_route ... ignored, one real cc-switch request; requires AICROSS_CC_SWITCH_PATH and provider
test dispatch::tests::live_economy_pipeline ... ignored, three real model requests; run once outside the sandbox for TASK-04 acceptance
test dispatch::tests::frontmatter_roundtrips ... ok
test dispatch::tests::artifact_diff_covers_deleted_binary_large_and_oversized_markers ... ok
test dispatch::tests::live_shared_continuation ... ignored, two real model requests; run once for TASK-03 acceptance
test channels::tests::kimi_session_parser_ignores_resume_text_in_answer ... ok
test channels::tests::kimi_collects_stream_usage_when_present ... ok
test channels::tests::cc_switch_reads_usage_from_stderr ... ok
test dispatch::tests::non_git_worktree_is_accounted_as_single_writer ... ok
test channels::tests::kimi_resume_uses_current_cli_session_flag ... ok
test dispatch::tests::snapshot_builds_diff_and_changed_file_content ... ok
test channels::tests::kimi_crops_more_than_30000_utf8_bytes ... ok
test dispatch::tests::broad_redaction_warning_only_counts_unquoted_whole_lines ... ok
test dispatch::tests::trace_fields_are_always_sanitized ... ok
test dispatch::tests::redacts_all_six_secret_shapes_and_stays_clean ... ok
test dispatch::tests::snapshot_marks_files_over_eight_megabytes_without_reading_content ... ok
test dispatch::tests::inspect_state_uses_machine_header_instead_of_status_prose ... ok
test hook_installer::tests::codex_features_enables_hooks_and_strips_legacy ... ok
test dispatch::tests::blind_directory_contains_only_selected_materials ... ok
test hook_installer::tests::codex_features_creates_section_if_missing ... ok
test dispatch::tests::blind_acceptance_rejects_drive_relative_changed_path ... ok
test hook_installer::tests::codex_features_preserves_user_hooks_false_overwrites ... ok
test dispatch::tests::node_stage_is_used_for_result_and_record ... ok
test launch::tests::ignores_non_launch_argv ... ok
test launch::tests::parses_equals_form_and_validates_cwd ... ok
test launch::tests::parses_tool_with_and_without_argv0 ... ok
test launch::tests::rejects_unknown_tool ... ok
test dispatch::tests::snapshot_skips_excluded_directories_and_enforces_total_bytes ... ok
test hook_installer::tests::codex_hooks_json_creates_with_4_events ... ok
test hook_installer::tests::codex_hooks_json_preserves_user_hooks ... ok
test dispatch::tests::blind_acceptance_directory_contains_only_artifact_delivery ... ok
test dispatch::tests::state_uses_actual_limit_and_separates_failures ... ok
test hook_installer::tests::codex_features_idempotent ... ok
test orchestration::tests::dispatchable_filters_connected_only ... ok
test dispatch::tests::preparation_failure_records_only_unsent_placeholder ... ok
test orchestration::tests::dispatchable_missing_file_reports_error ... ok
test orchestration::tests::load_missing_graph_returns_none ... ok
test hook_installer::tests::codex_hooks_json_unparseable_left_untouched ... ok
test orchestration::tests::presets_broken_file_returns_builtins ... ok
test hook_installer::tests::kimi_config_creates_with_9_events ... ok
test orchestration::tests::dispatchable_broken_file_reports_error ... ok
test hook_installer::tests::kimi_config_blocks_have_only_allowed_fields ... ok
test orchestration::tests::single_writer_role_uses_configured_writable_agent ... ok
test orchestration::tests::presets_missing_file_returns_builtins ... ok
test orchestration::tests::graph_roundtrip ... ok
test server::resume_cwd_tests::does_not_split_on_a_dot_inside_a_component ... ok
test server::resume_cwd_tests::does_not_touch_an_ordinary_hidden_dir_without_worktrees ... ok
test server::resume_cwd_tests::handles_mixed_separators_and_non_ascii ... ok
test server::resume_cwd_tests::leaves_a_plain_project_dir_unchanged ... ok
test server::resume_cwd_tests::leaves_a_real_subdir_unchanged ... ok
test hook_installer::tests::codex_hooks_json_idempotent_no_accumulation ... ok
test orchestration::tests::save_rejects_bad_ids ... ok
test orchestration::tests::save_rejects_wrong_version ... ok
test orchestration::tests::list_graphs_skips_broken_files ... ok
test orchestration::tests::presets_wrong_version_returns_builtins ... ok
test dispatch::tests::project_write_lock_wait_can_be_cancelled ... ok
test server::resume_cwd_tests::requires_worktrees_to_be_a_whole_next_component ... ok
test server::resume_cwd_tests::strips_only_dot_claude_worktrees_not_other_dot_dir_worktrees ... ok
test server::resume_cwd_tests::strips_unix_worktree_cwd_to_project_root ... ok
test server::resume_cwd_tests::strips_windows_worktree_cwd_to_project_root ... ok
test server::tests::codex_desktop_files_only_block_returns_empty ... ok
test server::tests::codex_desktop_plain_block_passes_through_unchanged ... ok
test hook_installer::tests::kimi_config_preserves_existing_content ... ok
test hook_installer::tests::kimi_config_reinstall_strips_stale_keeps_user_hooks ... ok
test resources::tests::connected_filters_status ... ok
test orchestration::tests::presets_roundtrip ... ok
test server::resume_cwd_tests::map_is_empty_when_claude_json_is_missing ... ok
test server::tests::codex_parser_drops_subagent_source_unit_variant ... ok
test server::resume_cwd_tests::map_has_no_entry_for_an_unregistered_folder ... ok
test server::tests::codex_parser_drops_subagent_source_object ... ok
test server::resume_cwd_tests::map_is_empty_and_does_not_panic_on_malformed_json ... ok
test server::tests::codex_parser_drops_subagent_thread_source ... ok
test hook_installer::tests::kimi_config_idempotent_no_duplicates ... ok
test server::tests::strips_codex_desktop_file_preamble_to_real_request ... ok
test shell_probe::tests::end_to_end_detects_installed_pwsh_with_version ... ignored
test shell_probe::tests::finds_msix_pwsh_alias ... ignored
test shell_probe::tests::auto_is_default_for_unknown_string ... ok
test server::resume_cwd_tests::map_resolves_real_world_hyphenated_project_path ... ok
test shell_probe::tests::json_mirror_hides_paths ... ok
test shell_probe::tests::parses_windows_ids ... ok
test shell_probe::tests::resolve_gitbash_falls_back_when_absent ... ok
test shell_probe::tests::resolve_cmd_uses_system_path ... ok
test shell_probe::tests::resolve_powershell_is_absolute ... ok
test skills::tests::rel_path_validation ... ok
test skills::tests::skill_name_validation ... ok
test terminal::tests::find_preset_known_tools ... ok
test server::resume_cwd_tests::map_resolves_second_real_world_project_path ... ok
test terminal::tests::find_preset_resume_program_matches_tool ... ok
test terminal::tests::find_preset_unknown_returns_none ... ok
test terminal::tests::claude_token_valid_uuid ... ok
test terminal::tests::claude_token_rejects_injection ... ok
test server::tests::codex_parser_keeps_legacy_session_without_thread_source ... ok
test orchestration::tests::save_strips_builtin_entries ... ok
test terminal::tests::osc7_absent_returns_none ... ok
test terminal::tests::osc7_basic_with_hostname ... ok
test terminal::tests::osc7_embedded_in_larger_buffer ... ok
test terminal::tests::osc7_no_hostname ... ok
test terminal::tests::osc7_percent_encoded_chinese ... ok
test terminal::tests::osc7_percent_encoded_space ... ok
test terminal::tests::osc7_st_terminator ... ok
test terminal::tests::osc7_windows_style_path ... ok
test tool_config::tests::empty_entry_check ... ok
test tool_config::tests::expand_path_handles_tilde_and_passthrough ... ok
test tool_config::tests::parse_simple ... ok
test terminal::tests::opencode_token_valid_format ... ok
test windows_path::tests::already_present_skipped_case_insensitive_and_trailing_slash ... ok
test windows_path::tests::blank_entries_in_current_dont_shadow_candidates ... ok
test windows_path::tests::duplicate_candidates_deduped ... ok
test server::tests::codex_parser_keeps_user_fork_session ... ok
test windows_path::tests::empty_current_joins_all_candidates ... ok
test windows_path::tests::empty_candidates_returns_current_unchanged ... ok
test windows_path::tests::mixed_present_and_new ... ok
test terminal::tests::opencode_token_rejects_invalid ... ok
test orchestration::tests::roles_roundtrip_and_fill_economy_nodes ... ok
test server::tests::codex_parser_keeps_user_main_session ... ok
test windows_path::tests::real_path_dirs_reads_registry ... ok
test server::tests::keeps_real_session_with_real_user_message ... ok
test server::tests::counts_array_content_user_message_with_mixed_blocks ... ok
test server::tests::keeps_session_that_continued_after_compaction ... ok
test terminal::tests::hermes_token_valid_format ... ok
test server::tests::drops_pure_compaction_subtask_session ... ok
test terminal::tests::hermes_token_rejects_invalid ... ok
test dispatch::tests::project_write_lock_is_mutually_exclusive ... ok
test server::tests::drizzle_scanner_excludes_subagent_and_archived_sessions ... ok
test dispatch::tests::git_repository_uses_three_line_unified_diff ... ok
test process::tests::run_process_truncates_and_marks_output ... ok
test process::tests::run_process_times_out_and_terminates_the_child_tree ... ok

test result: ok. 141 passed; 0 failed; 6 ignored; 0 measured; 0 filtered out; finished in 4.54s

```

#### `npx vitest run`（`src-ui`，exit=1）

```text
(node:40276) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:13740) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
failed to load config from D:\work\aicross-p4\src-ui\vite.config.ts

⎯⎯⎯⎯⎯⎯⎯ Startup Error ⎯⎯⎯⎯⎯⎯⎯⎯
Error: Build failed with 1 error:

[plugin externalize-deps]
Error: spawn EPERM
    at ChildProcess.spawn (node:internal/child_process:458:11)
    at spawn (node:child_process:813:9)
    at Object.execFile (node:child_process:349:17)
    at exec (node:child_process:236:25)
    at optimizeSafeRealPathSync (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:1954:2)
    at windowsSafeRealPathSync (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:1940:3)
    at getRealPath (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:32413:36)
    at tryResolveRealFileOrType (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:32407:9)
    at tryCleanFsResolve (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:32151:21)
    at tryFsResolve (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:32144:14)
    at aggregateBindingErrorsIntoJsError (file:///D:/work/aicross-p4/src-ui/node_modules/rolldown/dist/shared/error-DBGOT6sf.mjs:48:18)
    at unwrapBindingResult (file:///D:/work/aicross-p4/src-ui/node_modules/rolldown/dist/shared/error-DBGOT6sf.mjs:18:128)
    at #build (file:///D:/work/aicross-p4/src-ui/node_modules/rolldown/dist/shared/rolldown-build-hRnqgxyz.mjs:3313:34)
    at async bundleConfigFile (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:34633:17)
    at async bundleAndLoadConfigFile (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:34541:18)
    at async loadConfigFromFile (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:34510:42)
    at async resolveConfig (file:///D:/work/aicross-p4/src-ui/node_modules/vite/dist/node/chunks/node.js:34134:22)
    at async resolveConfig$1 (file:///D:/work/aicross-p4/src-ui/node_modules/vitest/dist/chunks/index.B89dZ0-N.js:14773:25)
    at async createVitest (file:///D:/work/aicross-p4/src-ui/node_modules/vitest/dist/chunks/cli-api.LUtK11-x.js:26:17)
    at async prepareVitest (file:///D:/work/aicross-p4/src-ui/node_modules/vitest/dist/chunks/cli-api.LUtK11-x.js:421:14) {
  errors: [Getter/Setter]
}



```

#### `npx vitest run --configLoader native --pool threads --maxWorkers 1`（`src-ui`，exit=0）

```text
(node:42188) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:20432) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)

 RUN  v5.0.0 D:/work/aicross-p4/src-ui

(node:20432) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:20432) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:20432) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:20432) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:20432) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:20432) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:20432) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
(node:20432) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)

 Test Files  8 passed (8)
      Tests  82 passed (82)
   Start at  23:06:32
   Duration  1.66s (transform 39%, import 33%, tests 18%, worker 10%)

    Isolate  8 workers spawned · ~135ms startup each (spawn + environment, per file)
             at least ~948ms faster with isolate: false — reuses workers across files instead of one per file

```

#### `npm run build`（`src-ui`，exit=0）

```text

> aicross-ui@0.0.0 build
> tsc -b && vite build

(node:39176) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)
vite v8.0.7 building client environment for production...
transforming...✓ 575 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                                         2.65 kB │ gzip:   1.16 kB
dist/assets/main-BO_rORq6.css                         145.26 kB │ gzip:  25.40 kB
dist/assets/dist-js-5OZv1H5C.js                         0.16 kB │ gzip:   0.15 kB
dist/assets/app-XgSAopP-.js                             0.25 kB │ gzip:   0.21 kB
dist/assets/codeowners-YLWRiiGA.js                      0.54 kB │ gzip:   0.31 kB
dist/assets/image-B_hDXdWL.js                           0.61 kB │ gzip:   0.32 kB
dist/assets/shellsession-uwY3V5Kj.js                    0.71 kB │ gzip:   0.42 kB
dist/assets/tsv-FmpeqnFg.js                             0.73 kB │ gzip:   0.34 kB
dist/assets/html-derivative-y8cTjkde.js                 0.83 kB │ gzip:   0.45 kB
dist/assets/git-rebase-BkCIAZbs.js                      0.98 kB │ gzip:   0.44 kB
dist/assets/qmldir-DtRCXXGi.js                          1.00 kB │ gzip:   0.45 kB
dist/assets/csv-BCAXOHVN.js                             1.14 kB │ gzip:   0.37 kB
dist/assets/git-commit-D9uUyrpH.js                      1.22 kB │ gzip:   0.53 kB
dist/assets/xsl-Bo_Acr9l.js                             1.36 kB │ gzip:   0.50 kB
dist/assets/event-CCOELuOC.js                           1.40 kB │ gzip:   0.69 kB
dist/assets/dotenv-BG-R-dnJ.js                          1.42 kB │ gzip:   0.53 kB
dist/assets/sparql-I6g4RV9o.js                          1.47 kB │ gzip:   0.81 kB
dist/assets/ini-D1hC5V4K.js                             1.52 kB │ gzip:   0.50 kB
dist/assets/fortran-fixed-form-Dh8pw333.js              1.66 kB │ gzip:   0.69 kB
dist/assets/docker-BgEUgtmq.js                          1.73 kB │ gzip:   0.60 kB
dist/assets/hxml-_7MT1TaI.js                            1.74 kB │ gzip:   0.88 kB
dist/assets/desktop-DeW1bojR.js                         1.83 kB │ gzip:   0.76 kB
dist/assets/core-CAYfUUdT.js                            1.98 kB │ gzip:   0.82 kB
dist/assets/erb-Bx3F2RzE.js                             2.13 kB │ gzip:   0.62 kB
dist/assets/wenyan-B7WaDmpb.js                          2.15 kB │ gzip:   1.09 kB
dist/assets/jssm-nqlDcCik.js                            2.23 kB │ gzip:   0.61 kB
dist/assets/edge-BMCHHRKD.js                            2.30 kB │ gzip:   0.67 kB
dist/assets/reg-C9YB0Q6j.js                             2.34 kB │ gzip:   0.69 kB
dist/assets/diff-IPIU9Fzm.js                            2.56 kB │ gzip:   0.69 kB
dist/assets/gleam-Bqam2yKl.js                           2.57 kB │ gzip:   0.82 kB
dist/assets/defineProperty-Cc0ioC06.js                  2.59 kB │ gzip:   1.30 kB
dist/assets/hy-B7p7U1BT.js                              2.64 kB │ gzip:   1.18 kB
dist/assets/openscad-C-9uKvUO.js                        2.82 kB │ gzip:   1.01 kB
dist/assets/log-vQtdJHWP.js                             2.85 kB │ gzip:   0.90 kB
dist/assets/json-DG5xeaaD.js                            2.88 kB │ gzip:   0.81 kB
dist/assets/cairo-ClvR2owv.js                           2.93 kB │ gzip:   0.80 kB
dist/assets/berry-BUpy9_cc.js                           3.00 kB │ gzip:   0.81 kB
dist/assets/jsonl-DhVam5Gu.js                           3.00 kB │ gzip:   0.78 kB
dist/assets/jsonc-BK4mmDTw.js                           3.10 kB │ gzip:   0.79 kB
dist/assets/logo-CCxtNjv2.js                            3.12 kB │ gzip:   1.47 kB
dist/assets/po-CJUqynl0.js                              3.23 kB │ gzip:   0.91 kB
dist/assets/json5-B0H0ftRm.js                           3.25 kB │ gzip:   0.83 kB
dist/assets/mipsasm-BLGWrDdP.js                         3.25 kB │ gzip:   1.17 kB
dist/assets/tasl-BGeyykQ_.js                            3.28 kB │ gzip:   0.84 kB
dist/assets/webview-Dcvwb5z8.js                         3.31 kB │ gzip:   1.12 kB
dist/assets/genie-CN4rq7A0.js                           3.35 kB │ gzip:   1.21 kB
dist/assets/rel-DifwsMId.js                             3.36 kB │ gzip:   1.11 kB
dist/assets/vala-C6OpFbMA.js                            3.36 kB │ gzip:   1.19 kB
dist/assets/splunk-Cf3CW7wI.js                          3.43 kB │ gzip:   1.52 kB
dist/assets/hurl-2ACva3j7.js                            3.51 kB │ gzip:   1.10 kB
dist/assets/fluent-DGOyQ8Ws.js                          3.60 kB │ gzip:   0.90 kB
dist/assets/ssh-config-CKUNcAMa.js                      3.61 kB │ gzip:   1.60 kB
dist/assets/jsonnet-BOcQ7pWS.js                         3.61 kB │ gzip:   1.04 kB
dist/assets/kdl-DBigZy8w.js                             3.62 kB │ gzip:   1.04 kB
dist/assets/narrat-Cq3CR6rl.js                          3.67 kB │ gzip:   1.10 kB
dist/assets/turtle-D_eMCpxI.js                          3.69 kB │ gzip:   0.97 kB
dist/assets/glsl-DtQQAfSz.js                            3.70 kB │ gzip:   1.46 kB
dist/assets/zenscript-wcLUgsD2.js                       3.91 kB │ gzip:   1.28 kB
dist/assets/ron-ChSsJUxP.js                             3.91 kB │ gzip:   0.98 kB
dist/assets/gn-DJDjd0xf.js                              4.00 kB │ gzip:   1.48 kB
dist/assets/pascal-BHiDpJdH.js                          4.14 kB │ gzip:   1.67 kB
dist/assets/http-duTJ0maY.js                            4.41 kB │ gzip:   1.06 kB
dist/assets/tcl-CGMQ2Gre.js                             4.42 kB │ gzip:   1.50 kB
dist/assets/nextflow-DYYpq5fR.js                        4.50 kB │ gzip:   1.16 kB
dist/assets/rosmsg-C8yR4p9t.js                          4.52 kB │ gzip:   1.06 kB
dist/assets/polar-BiOMIDty.js                           4.66 kB │ gzip:   1.12 kB
dist/assets/sdbl-DbzyTi_y.js                            4.70 kB │ gzip:   2.01 kB
dist/assets/fennel-Cp7mIZmg.js                          4.76 kB │ gzip:   1.54 kB
dist/assets/bibtex-CMf_khHV.js                          4.79 kB │ gzip:   0.83 kB
dist/assets/llvm-BUQffIyu.js                            5.04 kB │ gzip:   2.02 kB
dist/assets/wgsl-8R5OMxfU.js                            5.14 kB │ gzip:   1.39 kB
dist/assets/gdresource-VMh-5Lix.js                      5.28 kB │ gzip:   1.34 kB
dist/assets/zig-7Lbxfjhx.js                             5.33 kB │ gzip:   1.55 kB
dist/assets/qml-DV-9MRuy.js                             5.33 kB │ gzip:   1.38 kB
dist/assets/dax-DlTOXg5S.js                             5.36 kB │ gzip:   2.22 kB
dist/assets/bicep-mN0ruteN.js                           5.38 kB │ gzip:   1.15 kB
dist/assets/xml-CyIjMK9K.js                             5.45 kB │ gzip:   1.25 kB
dist/assets/awk-C2cQpOjH.js                             5.45 kB │ gzip:   1.38 kB
dist/assets/coq-M9LrrHou.js                             5.52 kB │ gzip:   1.91 kB
dist/assets/jinja-CaX9vMDt.js                           5.62 kB │ gzip:   1.36 kB
dist/assets/lean-Dtsklvza.js                            5.78 kB │ gzip:   1.92 kB
dist/assets/moonbit-DelbyvlD.js                         5.89 kB │ gzip:   1.68 kB
dist/assets/powerquery-DDoDbTf0.js                      5.90 kB │ gzip:   1.52 kB
dist/assets/shaderlab-C-QDXGQP.js                       5.92 kB │ gzip:   2.08 kB
dist/assets/verilog-BNuILg-8.js                         5.92 kB │ gzip:   1.89 kB
dist/assets/cypher-Bf0d0j36.js                          5.95 kB │ gzip:   1.72 kB
dist/assets/vb-vQ7FBk14.js                              6.09 kB │ gzip:   2.34 kB
dist/assets/red-CgmMwQ5s.js                             6.26 kB │ gzip:   1.60 kB
dist/assets/min-dark-CBFjRQ4v.js                        6.28 kB │ gzip:   1.70 kB
dist/assets/gdshader-DLlTfkMm.js                        6.32 kB │ gzip:   1.72 kB
dist/assets/prisma-CPh4c3GQ.js                          6.32 kB │ gzip:   1.39 kB
dist/assets/ara-CpyWD-c6.js                             6.36 kB │ gzip:   1.81 kB
dist/assets/clojure-C3jjSFuY.js                         6.41 kB │ gzip:   1.42 kB
dist/assets/postcss-DAA_T9V0.js                         6.41 kB │ gzip:   1.91 kB
dist/assets/toml-BlynkCpr.js                            6.42 kB │ gzip:   1.28 kB
dist/assets/solarized-light-DLLndLS4.js                 6.47 kB │ gzip:   1.71 kB
dist/assets/proto-C0LdvFNL.js                           6.54 kB │ gzip:   1.42 kB
dist/assets/smalltalk-BIVJlCyX.js                       6.58 kB │ gzip:   1.62 kB
dist/assets/r-B4zVi_fs.js                               6.61 kB │ gzip:   1.83 kB
dist/assets/talonscript-Dr-lPTln.js                     6.75 kB │ gzip:   1.49 kB
dist/assets/solarized-dark-DgqPf06l.js                  6.84 kB │ gzip:   1.78 kB
dist/assets/riscv-BM628xIq.js                           6.91 kB │ gzip:   1.98 kB
dist/assets/soy-jLaB-7Vb.js                             6.92 kB │ gzip:   1.62 kB
dist/assets/min-light-OvUhfe4y.js                       6.96 kB │ gzip:   1.88 kB
dist/assets/scheme-BIfFz6Pe.js                          7.16 kB │ gzip:   2.05 kB
dist/assets/hlsl-I4TUSC5a.js                            7.26 kB │ gzip:   2.19 kB
dist/assets/qss-BgVkvApy.js                             7.47 kB │ gzip:   2.57 kB
dist/assets/systemd-D4187P0y.js                         7.86 kB │ gzip:   2.54 kB
dist/assets/monokai-DHFwemSO.js                         7.88 kB │ gzip:   1.90 kB
dist/assets/regexp-BnAW9hSB.js                          8.05 kB │ gzip:   1.47 kB
dist/assets/haml-DtItqECI.js                            8.34 kB │ gzip:   1.86 kB
dist/assets/typst-BP5N7E5y.js                           8.38 kB │ gzip:   1.67 kB
dist/assets/vue-html-C5v0WTbT.js                        8.47 kB │ gzip:   1.67 kB
dist/assets/plsql-DEXRAcVm.js                           8.51 kB │ gzip:   2.99 kB
dist/assets/dart-3JwWll0a.js                            8.62 kB │ gzip:   2.00 kB
dist/assets/horizon-C9nqL9R6.js                         8.77 kB │ gzip:   1.95 kB
dist/assets/kotlin-BYL6QRVS.js                          8.78 kB │ gzip:   2.13 kB
dist/assets/horizon-bright-Css32xyz.js                  8.78 kB │ gzip:   1.96 kB
dist/assets/make-Dw3yrZ0Q.js                            8.95 kB │ gzip:   1.77 kB
dist/assets/andromeeda-DGoCYgJo.js                      9.01 kB │ gzip:   2.36 kB
dist/assets/sas-CbDNbSM0.js                             9.06 kB │ gzip:   3.82 kB
dist/assets/dark-plus-DY33niKa.js                       9.09 kB │ gzip:   2.10 kB
dist/assets/slack-dark-BAvxGu1N.js                      9.11 kB │ gzip:   1.96 kB
dist/assets/sass-DocrZT3r.js                            9.28 kB │ gzip:   2.49 kB
dist/assets/plastic-iniznJdO.js                         9.29 kB │ gzip:   1.97 kB
dist/assets/slack-ochin-DWqn_Lk-.js                     9.42 kB │ gzip:   2.09 kB
dist/assets/tex-6AKuZd1g.js                             9.67 kB │ gzip:   3.06 kB
dist/assets/jison--pZ8m-HK.js                           9.69 kB │ gzip:   1.87 kB
dist/assets/cmake-DrqqGEK_.js                           9.85 kB │ gzip:   3.38 kB
dist/assets/light-plus-DfRHv4Xo.js                      9.93 kB │ gzip:   2.27 kB
dist/assets/hcl-WYMZvHje.js                            10.05 kB │ gzip:   1.93 kB
dist/assets/rst-CK2J_xNG.js                            10.30 kB │ gzip:   2.25 kB
dist/assets/pkl-DawJyhw3.js                            10.37 kB │ gzip:   1.38 kB
dist/assets/beancount-QTI4W_q2.js                      10.37 kB │ gzip:   1.43 kB
dist/assets/nextflow-groovy-A4by5qzv.js                10.43 kB │ gzip:   2.14 kB
dist/assets/dream-maker-C3tt4vD1.js                    10.46 kB │ gzip:   2.26 kB
dist/assets/raku-Ba22PpAY.js                           10.47 kB │ gzip:   2.94 kB
dist/assets/yaml-CzLfUaQ1.js                           10.57 kB │ gzip:   2.32 kB
dist/assets/just-RTSWN6SD.js                           10.77 kB │ gzip:   2.60 kB
dist/assets/elm-D_V6tPeg.js                            10.94 kB │ gzip:   2.10 kB
dist/assets/github-light-CIJNwzyc.js                   11.18 kB │ gzip:   2.49 kB
dist/assets/prolog-BCLGsSxa.js                         11.35 kB │ gzip:   3.85 kB
dist/assets/terraform-DfAmkD3Z.js                      11.38 kB │ gzip:   2.51 kB
dist/assets/github-dark-OFaljh7J.js                    11.40 kB │ gzip:   2.53 kB
dist/assets/puppet-BI7VePLm.js                         11.43 kB │ gzip:   2.11 kB
dist/assets/laserwave-CQtCIGFc.js                      11.49 kB │ gzip:   2.57 kB
dist/assets/gherkin-Cr0wk206.js                        11.94 kB │ gzip:   5.05 kB
dist/assets/wasm-Beqnk49Y.js                           12.00 kB │ gzip:   2.18 kB
dist/assets/hjson-CbFL5VWg.js                          12.05 kB │ gzip:   1.64 kB
dist/assets/handlebars-Dit8JC0x.js                     12.16 kB │ gzip:   2.38 kB
dist/assets/window-Cf5upF_J.js                         12.41 kB │ gzip:   3.01 kB
dist/assets/apache-mOm_gFPl.js                         12.45 kB │ gzip:   3.71 kB
dist/assets/vesper-B0XbIQa2.js                         12.69 kB │ gzip:   1.98 kB
dist/assets/bat-CD8xesoU.js                            12.88 kB │ gzip:   3.23 kB
dist/assets/fish-Bw4coD8r.js                           13.03 kB │ gzip:   1.73 kB
dist/assets/v-DG0D3hZq.js                              13.21 kB │ gzip:   2.73 kB
dist/assets/vitesse-light-piMchJtN.js                  13.61 kB │ gzip:   3.03 kB
dist/assets/aurora-x-WbeSdnAE.js                       13.65 kB │ gzip:   2.28 kB
dist/assets/vitesse-black-DuzBzXNC.js                  13.67 kB │ gzip:   3.06 kB
dist/assets/vitesse-dark-ChPfzWLu.js                   13.75 kB │ gzip:   3.06 kB
dist/assets/pug-CM6j4wdg.js                            13.85 kB │ gzip:   2.58 kB
dist/assets/luau-CsHfLQlf.js                           13.96 kB │ gzip:   3.17 kB
dist/assets/synthwave-84-B1wrF6Pb.js                   14.03 kB │ gzip:   2.85 kB
dist/assets/github-light-default-Cskbnfwe.js           14.15 kB │ gzip:   3.02 kB
dist/assets/clarity-zNHM1tMD.js                        14.27 kB │ gzip:   2.47 kB
dist/assets/github-light-high-contrast-BUZL_cRA.js     14.27 kB │ gzip:   3.00 kB
dist/assets/github-dark-dimmed-DLGfz6FO.js             14.43 kB │ gzip:   3.11 kB
dist/assets/github-dark-default-BqbMvdcT.js            14.43 kB │ gzip:   3.12 kB
dist/assets/github-dark-high-contrast-wBnEARx0.js      14.59 kB │ gzip:   3.08 kB
dist/assets/gnuplot-Bd1MpVfM.js                        14.78 kB │ gzip:   3.28 kB
dist/assets/rust-jpCPbsEK.js                           15.06 kB │ gzip:   2.72 kB
dist/assets/kusto-COSTdHqw.js                          15.17 kB │ gzip:   3.93 kB
dist/assets/actionscript-3-LMiqWuSE.js                 15.20 kB │ gzip:   2.68 kB
dist/assets/nix-BTV8Kl4V.js                            15.50 kB │ gzip:   2.48 kB
dist/assets/lua-BB1sjGF9.js                            15.60 kB │ gzip:   3.19 kB
dist/assets/abap-s17gYGWC.js                           15.85 kB │ gzip:   5.93 kB
dist/assets/solidity-D-b1Sb1s.js                       16.07 kB │ gzip:   3.10 kB
dist/assets/matlab-D7Ymh7TH.js                         16.08 kB │ gzip:   3.07 kB
dist/assets/cue-CHmhgwIJ.js                            16.20 kB │ gzip:   2.05 kB
dist/assets/elixir-1Wj0-XTx.js                         16.26 kB │ gzip:   2.76 kB
dist/assets/odin-CzGDjIwK.js                           16.51 kB │ gzip:   2.93 kB
dist/assets/ts-tags-NZ2iHfv6.js                        16.54 kB │ gzip:   2.05 kB
dist/assets/kanagawa-wave-D_Hm3vX6.js                  17.12 kB │ gzip:   2.90 kB
dist/assets/kanagawa-lotus-CqgQE5Hl.js                 17.12 kB │ gzip:   2.91 kB
dist/assets/kanagawa-dragon-CByus8H8.js                17.12 kB │ gzip:   2.92 kB
dist/assets/move-BG_bNIu4.js                           17.50 kB │ gzip:   3.07 kB
dist/assets/graphql-DZnKSmmH.js                        18.08 kB │ gzip:   2.54 kB
dist/assets/liquid-DAb-uuE2.js                         18.10 kB │ gzip:   3.15 kB
dist/assets/svelte-CPH69QWw.js                         18.25 kB │ gzip:   3.14 kB
dist/assets/material-theme-CY5ZxTT5.js                 18.61 kB │ gzip:   3.10 kB
dist/assets/material-theme-darker-BnDPOEon.js          18.62 kB │ gzip:   3.10 kB
dist/assets/material-theme-ocean-CAu3HLd7.js           18.62 kB │ gzip:   3.12 kB
dist/assets/material-theme-lighter-CXYE62LH.js         18.63 kB │ gzip:   3.11 kB
dist/assets/material-theme-palenight-BLPSe7SZ.js       18.63 kB │ gzip:   3.12 kB
dist/assets/gdscript-C2D5WM82.js                       18.99 kB │ gzip:   3.75 kB
dist/assets/groovy-BeOakm-O.js                         19.17 kB │ gzip:   3.60 kB
dist/assets/mdc-C5YQj5X-.js                            19.54 kB │ gzip:   6.67 kB
dist/assets/ayu-dark-C8AZSEqd.js                       20.07 kB │ gzip:   3.93 kB
dist/assets/ayu-mirage-DJDt9_Ik.js                     20.08 kB │ gzip:   3.91 kB
dist/assets/glimmer-js-Z7X_LugF.js                     20.08 kB │ gzip:   2.94 kB
dist/assets/glimmer-ts-CmBNQRbz.js                     20.08 kB │ gzip:   2.94 kB
dist/assets/powershell-DThFFtVM.js                     20.14 kB │ gzip:   4.06 kB
dist/assets/ayu-light-DGy4vufb.js                      20.15 kB │ gzip:   3.90 kB
dist/assets/viml-CP_eTIVX.js                           20.36 kB │ gzip:   6.74 kB
dist/assets/nushell-_kwMquEq.js                        20.41 kB │ gzip:   5.22 kB
dist/assets/snazzy-light-DEGtvnBx.js                   20.77 kB │ gzip:   3.81 kB
dist/assets/twig-EY5SnVCc.js                           20.90 kB │ gzip:   3.66 kB
dist/assets/dracula-C8tduV4h.js                        21.06 kB │ gzip:   4.00 kB
dist/assets/dracula-soft-i03TrcGR.js                   21.07 kB │ gzip:   4.02 kB
dist/assets/wit-Q3uzgIzy.js                            21.46 kB │ gzip:   2.88 kB
dist/assets/rose-pine-IZyio8vo.js                      21.74 kB │ gzip:   3.86 kB
dist/assets/rose-pine-moon-BfRCxBHe.js                 21.75 kB │ gzip:   3.88 kB
dist/assets/rose-pine-dawn-D1oZXPoh.js                 21.75 kB │ gzip:   3.88 kB
dist/assets/nim-mCDe1XWm.js                            22.45 kB │ gzip:   3.16 kB
dist/assets/common-lisp-D0DsnuEJ.js                    22.57 kB │ gzip:   6.07 kB
dist/assets/surrealql-DhK7dDdV.js                      22.58 kB │ gzip:   4.31 kB
dist/assets/gruvbox-dark-hard-BqtMDTUh.js              22.63 kB │ gzip:   4.16 kB
dist/assets/gruvbox-dark-soft-CV57n6pA.js              22.63 kB │ gzip:   4.15 kB
dist/assets/gruvbox-light-hard-b4k0L1XS.js             22.63 kB │ gzip:   4.16 kB
dist/assets/gruvbox-light-soft-DP66dnY9.js             22.63 kB │ gzip:   4.16 kB
dist/assets/gruvbox-dark-medium-Dgfy8-dz.js            22.63 kB │ gzip:   4.16 kB
dist/assets/gruvbox-light-medium-DSewW6QF.js           22.63 kB │ gzip:   4.16 kB
dist/assets/bird2-Bh111mYM.js                          22.94 kB │ gzip:   4.66 kB
dist/assets/sql-DcHRMm-Q.js                            23.47 kB │ gzip:   7.49 kB
dist/assets/cadence-DXH0Dgwr.js                        23.66 kB │ gzip:   3.66 kB
dist/assets/astro-XmnDH_Dd.js                          24.02 kB │ gzip:   7.60 kB
dist/assets/apl-DjqofZJD.js                            24.03 kB │ gzip:   4.17 kB
dist/assets/templ-BkokcB7D.js                          24.06 kB │ gzip:   5.41 kB
dist/assets/angular-html-BlerCZUb.js                   24.25 kB │ gzip:   3.98 kB
dist/assets/vhdl-3hDyUkZR.js                           24.26 kB │ gzip:   3.86 kB
dist/assets/vue-CqFBltHt.js                            24.50 kB │ gzip:   2.97 kB
dist/assets/purescript-DKMYP_A1.js                     24.68 kB │ gzip:   3.24 kB
dist/assets/typespec-BQy10TzM.js                       25.04 kB │ gzip:   2.62 kB
dist/assets/one-light-27e-9kYd.js                      25.29 kB │ gzip:   3.63 kB
dist/assets/fsharp-BQAqzjqk.js                         25.30 kB │ gzip:   4.13 kB
dist/assets/marko-Cj1_9Cb9.js                          25.48 kB │ gzip:   3.59 kB
dist/assets/c3-qRBCI8Uv.js                             25.62 kB │ gzip:   3.86 kB
dist/assets/night-owl-light-DtpYOdyl.js                25.90 kB │ gzip:   4.24 kB
dist/assets/system-verilog-jOLk1CBm.js                 26.20 kB │ gzip:   4.83 kB
dist/assets/nord-BQkWtC2v.js                           26.72 kB │ gzip:   4.36 kB
dist/assets/codeql-BoicTpVa.js                         26.88 kB │ gzip:   3.80 kB
dist/assets/scss-Dw42T1BI.js                           27.27 kB │ gzip:   4.23 kB
dist/assets/java-CoApPKT4.js                           27.28 kB │ gzip:   4.31 kB
dist/assets/coffee-fAb6LsdV.js                         27.42 kB │ gzip:   6.35 kB
dist/assets/razor-ccbgdA8j.js                          27.45 kB │ gzip:   3.54 kB
dist/assets/scala-D0LjI2Qi.js                          28.88 kB │ gzip:   3.93 kB
dist/assets/night-owl-DDmCLEp1.js                      28.91 kB │ gzip:   5.15 kB
dist/assets/crystal-HHV-Zo9X.js                        29.41 kB │ gzip:   4.44 kB
dist/assets/mermaid-CAtOlIbf.js                        29.50 kB │ gzip:   3.65 kB
dist/assets/applescript-C5yVau2U.js                    29.56 kB │ gzip:   5.95 kB
dist/assets/julia-jUIBt023.js                          30.99 kB │ gzip:   4.30 kB
dist/assets/stylus-CNnrGufP.js                         31.07 kB │ gzip:   8.02 kB
dist/assets/poimandres-H4_Eyox2.js                     33.49 kB │ gzip:   5.49 kB
dist/assets/one-dark-pro-C4hn0Zoq.js                   33.78 kB │ gzip:   5.50 kB
dist/assets/bsl-ny3e_7dl.js                            33.86 kB │ gzip:   8.35 kB
dist/assets/haxe-C2AyQsKn.js                           35.16 kB │ gzip:   5.91 kB
dist/assets/nginx-B8zy1DQG.js                          35.35 kB │ gzip:   4.46 kB
dist/assets/houston-ByNQ9eAt.js                        35.42 kB │ gzip:   5.76 kB
dist/assets/tokyo-night-DQIh82d3.js                    35.66 kB │ gzip:   6.23 kB
dist/assets/erlang-Q-lfVOgV.js                         37.47 kB │ gzip:   4.36 kB
dist/assets/cobol-DQafa4Ab.js                          39.07 kB │ gzip:  10.89 kB
dist/assets/asm-Bbv1919r.js                            40.71 kB │ gzip:   8.17 kB
dist/assets/haskell-DTAtmdln.js                        41.48 kB │ gzip:   6.41 kB
dist/assets/shellscript-3Eyrpdfm.js                    41.54 kB │ gzip:   6.13 kB
dist/assets/perl-CEA6MP1k.js                           43.14 kB │ gzip:   4.65 kB
dist/assets/d-Dt1IpaRS.js                              43.79 kB │ gzip:   8.43 kB
dist/assets/ruby-B-fKc9x2.js                           46.60 kB │ gzip:   5.71 kB
dist/assets/go-D3d4JNoO.js                             46.81 kB │ gzip:   5.15 kB
dist/assets/apex-Bkns-Kf9.js                           46.98 kB │ gzip:   6.73 kB
dist/assets/catppuccin-mocha-D4_Pt6ay.js               47.25 kB │ gzip:   7.98 kB
dist/assets/catppuccin-latte-Coqss7en.js               47.25 kB │ gzip:   7.99 kB
dist/assets/catppuccin-frappe-C5l_8kFT.js              47.25 kB │ gzip:   7.99 kB
dist/assets/catppuccin-macchiato-DEc0mBz1.js           47.26 kB │ gzip:   7.99 kB
dist/assets/ada-CgdG45k-.js                            48.08 kB │ gzip:   5.96 kB
dist/assets/css-BaIEaAP4.js                            49.11 kB │ gzip:  11.96 kB
dist/assets/imba-D2BupS_o.js                           49.92 kB │ gzip:   9.47 kB
dist/assets/everforest-dark-DnGtViwz.js                53.74 kB │ gzip:   8.37 kB
dist/assets/everforest-light-CrRagu-i.js               53.74 kB │ gzip:   8.37 kB
dist/assets/wikitext-CokaWg4C.js                       55.88 kB │ gzip:   4.82 kB
dist/assets/stata-LzqjS0NL.js                          56.98 kB │ gzip:  12.39 kB
dist/assets/html-B1M_0gd8.js                           57.32 kB │ gzip:  11.79 kB
dist/assets/ballerina-C5NCmhdh.js                      58.69 kB │ gzip:   8.11 kB
dist/assets/markdown-DrnMgzIX.js                       59.33 kB │ gzip:   5.67 kB
dist/assets/ocaml-X3W8brJ7.js                          62.44 kB │ gzip:   5.05 kB
dist/assets/mojo-BEOkTXe0.js                           69.79 kB │ gzip:   9.22 kB
dist/assets/python-BtjLSkiI.js                         69.95 kB │ gzip:   9.10 kB
dist/assets/c-yOg8G6Qk.js                              72.17 kB │ gzip:  10.55 kB
dist/assets/latex-DFkYhUza.js                          72.61 kB │ gzip:   6.69 kB
dist/assets/vyper-BGjJ65g4.js                          74.64 kB │ gzip:  10.70 kB
dist/assets/hack-CB6YBmfz.js                           80.18 kB │ gzip:  26.27 kB
dist/assets/swift-ByDBHGEa.js                          86.68 kB │ gzip:  14.69 kB
dist/assets/fortran-free-form-BXrNE5X_.js              88.96 kB │ gzip:  11.19 kB
dist/assets/csharp-DK3gKUDm.js                         89.68 kB │ gzip:  10.64 kB
dist/assets/racket-DChGz2wN.js                         92.38 kB │ gzip:  15.08 kB
dist/assets/less-CCCdi_4d.js                           97.63 kB │ gzip:  14.85 kB
dist/assets/blade-DhyUajd-.js                         104.97 kB │ gzip:  28.30 kB
dist/assets/objective-c-Do3Dh539.js                   105.41 kB │ gzip:  23.64 kB
dist/assets/php-D1n20mva.js                           111.31 kB │ gzip:  28.66 kB
dist/assets/asciidoc-Mnh7mpqW.js                      131.52 kB │ gzip:   9.32 kB
dist/assets/mdx-BhxtouIL.js                           136.11 kB │ gzip:  23.54 kB
dist/assets/objective-cpp-DzvX7O7S.js                 171.97 kB │ gzip:  30.98 kB
dist/assets/javascript-DMDj563O.js                    174.89 kB │ gzip:  16.68 kB
dist/assets/tsx-Du47Vi3R.js                           175.60 kB │ gzip:  16.67 kB
dist/assets/jsx-BgYHCIzu.js                           177.86 kB │ gzip:  16.77 kB
dist/assets/typescript-Biw9qNLM.js                    181.14 kB │ gzip:  16.29 kB
dist/assets/angular-ts-BErpjn_O.js                    183.73 kB │ gzip:  16.77 kB
dist/assets/vue-vine-DaNosUn7.js                      190.06 kB │ gzip:  18.07 kB
dist/assets/dist-CbAFeoTq.js                          212.97 kB │ gzip:  67.93 kB
dist/assets/wolfram-BPifCLvx.js                       262.38 kB │ gzip:  77.64 kB
dist/assets/wasm-DA6mYL5T.js                          622.32 kB │ gzip: 232.09 kB
dist/assets/cpp-Cyj1bR0k.js                           626.13 kB │ gzip:  48.10 kB
dist/assets/emacs-lisp-DzXDqNne.js                    779.87 kB │ gzip: 197.56 kB
dist/assets/main-Cb9MTTPq.js                        1,229.32 kB │ gzip: 440.28 kB

✓ built in 2.32s
[plugin builtin:vite-reporter] 
(!) Some chunks are larger than 500 kB after minification. Consider:
- Using dynamic import() to code-split the application
- Use build.rolldownOptions.output.codeSplitting to improve chunking: https://rolldown.rs/reference/OutputOptions.codeSplitting
- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
```
