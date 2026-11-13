# TASK-74 完成报告

## 阶段 1：pi 输出、通用截断、超时

### 改动

- 子进程输出支持按行过滤；过滤行仍计作输出活动。输出超过 4 MB 时保留开头 1 MB 与结尾 3 MB，并按行边界处理；单行超过 3 MB 的结尾行只保留尾部。增加连续无输出超时，与总时长超时使用不同错误文字。
- pi 通道过滤 `message_update`，处理未完成响应与输出截断；pi 思考档位按模型支持值调整，实际档位记入记录。stdout 截断后解析失败显示「输出超过 4 MB，中间被截断」；部分通道解析成功但输出截断时添加节点备注「输出超过 4 MB，中间被丢弃，回答可能不完整」。
- 派发设置新增 `execTimeoutSecs`（默认 1800）与 `idleTimeoutSecs`（默认 300）；连续无输出上限目前只用于 pi 的只读与盲位置。git 项目快照按跟踪文件及未被忽略的未跟踪文件取范围，超限错误列出最大的三个顶层目录；执行中快照清单增加可选字段 `ignored`、`used_git`。
- 覆盖文件遇 Windows 拒绝访问或共享冲突时增加有限重试；写 run 文件失败时保留已完成节点结果。
- 清理仅供测试调用的快照包装函数与 `run_process_registered_with_activity`。

### 新增测试

- `task74_codex_stdout_truncation_adds_incomplete_answer_note`
- `task74_pi_idle_timeout_marks_node_timeout_with_idle_message`
- `task74_pi_writable_seat_ignores_idle_limit_until_total_timeout`
- `task74_pi_total_timeout_uses_exec_timeout_only_for_writable_seat`
- `git_snapshot_excludes_ignored_files_and_includes_untracked_files`
- `snapshot_limit_error_lists_three_largest_directories`
- `git_snapshot_listing_failure_falls_back_to_directory_walk`
- `after_snapshot_keeps_previously_ignored_paths_out_after_ignore_rule_is_removed`
- `after_snapshot_keeps_previously_included_file_when_new_ignore_rule_covers_it`
- `after_snapshot_excludes_new_files_under_previously_ignored_directory`
- `after_non_git_snapshot_keeps_directory_walk_scope_after_git_init`
- `empty_git_listing_for_ignored_project_falls_back_to_filesystem_walk`
- `task74_timeout_settings_default_and_clamp`
- `line_filter_drops_matching_lines_and_keeps_others`
- `filtered_lines_do_not_count_toward_output_limit`
- `overlong_filtered_line_is_discarded_without_buffering`
- `oversized_output_keeps_head_and_tail_and_final_line`
- `oversized_single_final_line_keeps_its_tail_when_it_ends_with_newline`
- `oversized_multiline_output_has_no_blank_gap_between_head_and_tail`
- `idle_timeout_terminates_silent_process_and_marks_idle`
- `continuous_output_does_not_trigger_idle_timeout`
- `pi_message_update_filter_matches_expected_prefix`

### 实现与审查过程

协调者把阶段 1 拆成六包，由 Codex gpt-6-luna 逐包实现；每包由协调者审查 diff 并在沙箱外运行测试。中途进行一次三家盲审（Antigravity gemini-4-carbon-medium、Claude sonnet、Antigravity gemini-4-argon-medium），核实成立的 8 条问题已修。

### 验证

- 执行者在沙箱内运行：`cargo check --offline`（仅编译核对，不作为最终全量依据；执行者无法在沙箱内运行需要创建临时目录的测试，执行者自己的测试结果不作依据）。
- 协调者在文档对齐并同步 `skill-bundle/` 后，于 Windows 11 沙箱外逐项运行：
  - `python scripts/sync-skill.py`：`已同步 21 个文件：D:\work\aicross-task74\skills\ai-cross -> D:\work\aicross-task74\skill-bundle\ai-cross`
  - `cargo check`：

    ```text
       Compiling aicross v0.3.8 (D:\work\aicross-task74)
        Finished `dev` profile [unoptimized + debuginfo] target(s) in 37.86s
    ```

    零警告（无 `snapshot_directory`、`snapshot_directory_with_limits`、`run_process_registered_with_activity` 的 `never used` 警告）。
  - `cargo test`：

    ```text
    test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
    test result: ok. 776 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 45.99s
    ```

    改动前基线约 750 项通过；新增测试见上表。
  - 空闲超时与只读/可写分流测试：
    - `cargo test idle_timeout`：`test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 783 filtered out; finished in 5.50s`（覆盖 `process::tests::idle_timeout_terminates_silent_process_and_marks_idle`、`process::tests::continuous_output_does_not_trigger_idle_timeout`、`dispatch::tests::task74_pi_idle_timeout_marks_node_timeout_with_idle_message`）。
    - `cargo test task74_pi_writable_seat_ignores_idle_limit_until_total_timeout`：`test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 785 filtered out; finished in 3.88s`。
  - `cd skills/ai-cross && python -m pytest -q tests`：`82 passed in 5.99s`。
- 既有测试改动：`fixtures/engine-golden/*.json` 共 31 个文件，`settings` 各增加 `execTimeoutSecs` 与 `idleTimeoutSecs` 两个键，只增不删；`cli_agents` 中 `effective_thinking` 的既有断言因新增 provider 参数机械补参，含义不变。其余既有断言未改。
- 真机验证（`cargo build` 产物 `D:\work\aicross-task74\target\debug\aicross.exe`，材料 224,011 字节 / 约 219 KB，由 `git diff -- src` 重复拼接）：
  - 命令：

    ```powershell
    D:\work\aicross-task74\target\debug\aicross.exe dispatch --host claude --project D:\work\aicross-task74 --preset blind --kind other --seat 1.1=pi:opencode-go:deepseek-v4.1-flash --seat 1.2=pi:zai-coding-cn:glm-5.3-flash --seat 1.3=agy::gemini-4-carbon-medium --text-file <224011 字节文本>
    ```

  - 派发响应：`{"accepted":true,"graph_id":"ee8f58b8-a559-4e5d-b0da-04522c941099","node_ids":["040c6a20-d4e2-412a-bbe3-d54ce269597e","2645afd4-3339-4802-bb2d-836edf8f3806","cdbf89f9-c53f-4425-a197-fee24124dc1a"],"notes":[],"run_id":"0ea1efbe-31b9-49f9-b442-b4775f8bdb1b"}`
  - 位置 1.1（`pi:opencode-go:deepseek-v4.1-flash`，记录 `D:\work\aicross-task74\.aicross\runs\20261113-153332-0ea1efbe\20261113-153720-pi-deepseek-v4-1-flash-验收.md`）：

    ```yaml
    agent: pi
    provider: opencode-go
    model: deepseek-v4.1-flash
    thinking: 已调整为：low
    truncated: false
    status: ok
    duration_ms: 228163
    tokens_in: 70653
    tokens_out: 21433
    identity: opencode-go/deepseek-v4.1-flash
    exit_code: 0
    error: ''
    ```

    回复正文共 7,996 字符，含「会出错」评审条目。
  - 位置 1.2（`pi:zai-coding-cn:glm-5.3-flash`，记录 `D:\work\aicross-task74\.aicross\runs\20261113-153332-0ea1efbe\20261113-154533-pi-glm-5-3-flash-验证.md`）：

    ```yaml
    agent: pi
    provider: zai-coding-cn
    model: glm-5.3-flash
    thinking: 已调整为：low
    truncated: false
    status: ok
    duration_ms: 493237
    tokens_in: 57563
    tokens_out: 15394
    identity: zai-coding-cn/glm-5.3-flash
    exit_code: 0
    error: ''
    ```

    回复正文共 7,379 字符，含「会出错」评审条目。
  - 位置 1.3（`agy::gemini-4-carbon-medium`，记录 `D:\work\aicross-task74\.aicross\runs\20261113-153332-0ea1efbe\20261113-153332-agy-gemini-4-carbon-medium-验证.md`）：处于阶段 1 未改 agy（阶段 2 范围）的预期行为，因材料合计 220 KB 超过 `TEXT_ONLY_INLINE_LIMIT`（200 KB）未发出：`status: error`、`error: agy 只能内联文字材料：提示词与材料合计 220 KB，超过 200 KB 上限，不发出`，整次 run 终态随之记为 `failed`。

### 未做与不确定处

- 新增测试中的 macOS / Linux 脚本未运行。codex、kimi、agy 的连续无输出上限未启用，输出节奏尚未实测；pi 在可写位置不启用连续无输出上限。
- 输出超限时保留开头 1 MB 与结尾 3 MB；单行超过 3 MB 的结尾行只能保留其尾部。读取 `~/.pi/agent/models-store.json` 失败时，思考档位保持现有映射。
- git 项目快照不包含被忽略的文件；执行者写入被忽略路径的内容不进入 diff，也不会被回滚。
- 阶段 2（Antigravity）、阶段 3（Kimi）、阶段 4（流水线提示词与结论判定，含任务书第 9–12 条）本期未做。
- 执行者不能确认跨平台脚本行为；macOS / Linux 行为仅编译覆盖，未实测。

## 阶段 2–4

未做。
