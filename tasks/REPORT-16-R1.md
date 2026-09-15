# REPORT-16-R1：控制台通道复审修正

## 结果

TASK-16 的控制台通道已改为薄中继。Rust 只负责本机 HTTP 校验、事件转发、前端回执等待和容量受限的运行摘要；图选择、预设盖章、G1 判定、请求构造、阶段执行、结果落图、采纳与取消均回到前端既有流水线。首版新增的 Rust 图写入、prompt 拼装、默认派发设置及共享采纳入口已删除。

契约采用追加方式：派发响应新增 `accepted`，原有 `graph_id`、`node_ids` 字段保留为空值以兼容旧调用方；运行摘要新增 `rejected`、`reason`、`in_flight_node_ids` 和节点 `quota_resets_at`。

## 必修项对应

### 1. Rust 改为薄中继

- `src/console_api.rs` 不再调用 `run_prepared`、`build_node_request`、`dispatch_stage`、`save_graph` 或 `DispatchSettings::default()`。
- `POST /v1/dispatch` 建立运行摘要，发送 `aicross:console-dispatch`，等待最多 5 秒的 `console_run_update` 回执；超时返回 503。
- `adopt`、`cancel` 发送 `aicross:console-action`，由 `console_action_ack` 完成往返。
- 摘要最多保留 200 条，按创建顺序淘汰最旧记录；Rust 不保存整图。

测试：`event_relay_round_trip_uses_frontend_update`、`frontend_ack_timeout_is_503`、`run_summary_evicts_oldest_over_200`、`missing_project_is_404_before_event_emit`。

### 2. 前端复用既有派发流水线

- `src-ui/src/console-channel.ts` 接收派发与动作事件；优先按 `Graph.project` 选图，旧图按 tab 的 `folderPath` 回退匹配，没有图时创建控制台来源图。
- `newGraph(project)` 现在把 `project` 写在 Graph 本身。
- 控制台节点使用 `stamp()`；默认经济模式，显式 targets 生成自定义预设；请求继续由 `buildNodeRequest` 构造。
- `src-ui/src/graph/dispatch.ts` 抽出 `startPipelineHeadless`，统一持有 `runStageLoop`、结果应用、异常收尾与 finally；`DispatchOverlay` 和控制台通道都调用该函数。
- 每阶段、每个节点落图及最终状态均通过 `console_run_update` 回报；`leftover` 和真实在飞节点 id 来自流水线状态。
- 结果卡采纳恢复为前端 `setReplyAdopted`；控制台动作也调用该函数。取消调用既有 `cancelDispatchNode`。总指挥 tab 在前台时不抢焦点。

测试：`new graphs own project and legacy graphs fall back to tab directory`、`reports only actual in-flight nodes`、`foreground commander tab is not focused again`、`background commander tab is focused`、`startPipelineHeadless runs the shared loop and always finishes`、`DispatchOverlay and console channel share startPipelineHeadless`，以及 `idle-and-tail-07c.test.tsx` 中的 finally/接线断言。

### 3. HTTP 与清单安全

- bearer 使用常量时间比较；鉴权发生在读取 body 之前。
- 带 `Origin` 或 `Sec-Fetch-Site: cross-site` 的请求返回 403。
- `HEAD`、`OPTIONS` 进入 HTTP 路由，不落入 JSON 行 hook 分支。
- 清单失败只报告“控制台通道不可用”，hook server 继续启动。
- Windows 通过 `%SystemRoot%\\System32` 定位 `whoami.exe`、`icacls.exe`；先收紧临时文件 ACL，再用 `MoveFileExW` 原子替换正式清单。

测试：`constant_time_comparison_accepts_only_exact_bearer`、`wrong_token_is_401_before_routing`、`origin_and_cross_site_are_403`、`unauthorized_is_decided_before_body_read`、`head_and_options_are_http_methods`、`manifest_write_delete_and_private_permissions`。

### 4. CLI 修正

- `status --wait` 支持 `--timeout <secs>`，默认 600 秒；超时退出 1。
- 等待循环每次重读清单；清单 PID 不存活时删除清单并报告“控制台未运行（清单已过期）”，退出 3。
- debug 使用 `console.dev.json`，release 使用 `console.json`。
- `--text-file` 剥离 UTF-8 BOM，非 UTF-8 给出明确错误；`--text` 和 `--text=` 均按用法错误退出 2。
- HTTP 请求设置 30 秒总超时；HTTP 200 非 JSON 按业务失败退出 1，并输出原文首行。
- 参数与文本文件先解析，再检查清单，因此本地输入错误不会被“控制台未运行”遮盖。

测试：`status_wait_timeout_defaults_to_600_and_accepts_override`、`stale_pid_removes_manifest`、`dispatch_strips_utf8_bom_and_rejects_non_utf8`、`dispatch_rejects_text_on_argv`、`http_200_non_json_is_business_error_with_original_text`、`http_client_total_timeout_is_30_seconds`、`no_console_manifest_exits_three_with_one_line`。

### 5. 安装包 PATH

- NSIS 改为逐项解析用户 Path，只添加或删除安装目录这一项；比较忽略大小写和尾反斜杠。
- 读取注册表时保留未展开的 `%VAR%`，写回时保留原 `REG_SZ` / `REG_EXPAND_SZ` 类型。
- 卸载时 Path 不存在或没有本项，不写注册表也不广播变更；安装时 Path 不存在则只写非空的本项。
- WiX 原有 item-level `Environment` 声明已符合相同语义，未作无效改写。
- 删除同义源码字符串断言，改为 PATH 纯函数行为测试。

测试：`adds one Windows PATH item without rewriting existing entries`、`removes only matching Windows PATH items and preserves missing values`。

### 6. 契约与文档

- 运行状态增加 `rejected`；节点状态不压平，额度用尽状态及 `quota_resets_at` 原样保留。
- `leftover` 由遗留节点摘要真实填充。
- `docs/CONSOLE-CHANNEL.md` 已同步薄中继机制、状态码、退出码、预设别名、平台 PATH、清单与安全行为。
- 新设置的用户可见术语同步到 `docs/MANUAL.md`。

测试：`rejected_reason_returns_409_and_remains_in_summary`、`quota_status_and_reset_time_are_preserved`，以及前端真实 leftover/在飞状态的流水线测试。

### 7. 在飞上限与 G1

- Rust 保留硬上限 3，第四条在飞 run 返回 409；前端新增 1–3 的“通道在飞上限”设置，默认 3，可进一步收紧。
- 控制台派发复用当前界面设置执行资源连接、只读、盲审写入、首次验证、额度、请求数和脏 worktree 检查；拒绝原因写入 run 摘要。
- 脏 worktree 使用既有 `dirtyWorktreeHintOf` 提示，不另造文案。

测试：`fourth_inflight_run_is_rejected`、`rejected_reason_returns_409_and_remains_in_summary`，以及前端设置归一化和 G1 既有测试集。

### 8. 测试整理

- 新增假前端回执的事件往返、rejected 原因、200 非 JSON、常量时间比较、Origin 403、body 前 401、PID 判活、PATH 纯函数、共享启动函数行为与接线测试。
- 删除首版 Rust 采纳入口测试及与新契约冲突的宽泛源码字符串断言；没有保留同义反复测试。

### 9. GLM 补充项

- 已结束 run 的 cancel 返回当前状态，不转发且不改状态。
- 200 非 JSON、HTTP 总超时、Windows 清单 ACL 后原子替换、额度状态与恢复时间、NSIS 卸载条件写回均按要求实现。

测试：`completed_cancel_does_not_relay_or_change_status`、`http_200_non_json_is_business_error_with_original_text`、`manifest_write_delete_and_private_permissions`、`quota_status_and_reset_time_are_preserved`、PATH 纯函数测试。

## 验证

- `cargo check --all-targets`：通过，零 warning。
- `cargo test`：通过；406 passed，0 failed，11 ignored。忽略项均为仓库既有真机、真实模型或外部环境测试。
- `cd src-ui && npx vitest run --configLoader native --pool threads`：31 个测试文件通过；474 passed，0 failed。
- `cd src-ui && npm run build`：`tsc -b` 与 Vite production build 通过。
- `git -c core.whitespace=cr-at-eol diff --check`：通过，无输出。
- 两段 NSIS `-EncodedCommand` 解码后经 PowerShell AST parser 检查：零语法错误；未执行注册表写入。
- 换行复核：`src/server.rs` 与 `src/main.rs` 仍为纯 CRLF；其他改动 Rust 文件为 LF；`src-ui/src/tauri.ts` 保持混合换行，改动限定在命令与契约追加区域。
- `src/terminal.rs` 无差异；未运行 `cargo fmt`。

## 未做与边界

- 按任务限制未启动桌面窗口、未发送真实控制台或模型请求。
- 未生成 NSIS / WiX 安装包，因此安装器行为由逐项脚本实现和 PATH 纯函数测试覆盖，未做安装包真机验收。
- Windows 实际 DACL、PID 与原子替换未对真实用户目录执行；测试使用临时目录和可注入判活/命令边界验证顺序与结果。
- 未修改 `src/terminal.rs`，未 commit，未 push。

## 复审后补记（编排者，a66c5ff）

两家复审（`tasks/review-16-r1-*.md`）都判可交付但有未闭合项，编排者直接修：DeepSeek 1——后台 run 整图回写会盖掉用户并发编辑，改成每次落图先合到 store 最新图、只带本次改动节点（`mergeConsoleCommit` + 测试）；GLM N1——每次落图回报把在飞清单清空，执行期 `aicross cancel` 与「N 路在飞」不工作，改成在飞清单随阶段记录并在每次回报里带上，取消目标 = 摘要在飞 ∪ 图上 pending（`consoleCancelTargets` + 测试）；GLM N2——终态 run 不接受迟到的 running 回报（Rust 侧守卫 + 测试），前端起流水线前确认 run 仍在 running；GLM N3——续写用户自己的图保留原来源；N7 文档节点状态示例改 pending。改后 cargo 407 / vitest 476 / build 通过。剩余低危记入 `tasks/fix-04-r5-backlog.md` 第 53–55 条。
