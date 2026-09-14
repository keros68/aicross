# TASK-04 修正轮 R3b（R3 的补课：测试与被推掉的必修项）

先读 `tasks/fix-04-r3.md` 与 `tasks/REPORT-04.md` 的「修正轮 R3」一节。编排者核对 R3 的 diff：22 条里 Rust 测试新增 **0** 条、vitest 用例新增 **1** 条，而修正单在第 1、2、3、4、5、6、7、8、12、13、14 条都明文要求补测试；第 10、15、18 三条被标「不采纳」但它们是必修；第 6 条资源冒烟的 meta 与第 9 条验证命令显示没做。本轮把这些补齐，在 `tasks/REPORT-04.md` 末尾新增「修正轮 R3b」一节逐条记录。验证跑 `cargo check`、`cargo test`、`cd src-ui && npx vitest run`（你的沙箱起不来 vitest 就用 `--configLoader native --pool threads` 再试一次，仍不行就如实写，编排者会沙箱外复跑）、`cd src-ui && npm run build`，命令与输出原文贴进报告。不起窗口，不发真机请求，不 commit，不 push。

## 必补测试（每条对应 R3 的编号）

- R3-1：Rust `serde_json::to_value(DispatchArtifacts{…})` 的键恰为 `diff / changed_files / worktree / stdout_summary`，`ChangedFile` 的键为 `path / content`；vitest：`artifactSummary` 与结果屏辅助函数对 `{}`、`{ changed_files: undefined }` 不抛。
- R3-2：vitest：`failedNodesWithoutResults` 在「两节点派出、一节点已 ok」时只返回另一节点。
- R3-3：vitest：推进循环抽出的纯函数遇到某层含空 agent/model 时返回「停下、该层节点 id」；Rust：`dispatch_graph_stage` 收到空 agent 节点时不创建 run 目录、不拍快照、返回 status=error 且 error 含「未指定」。
- R3-4：vitest：`applyRoleDefaults(preset, roles)` 只填空目标、不覆盖已填目标；提供者/档位/thinking 按角色带过去。
- R3-5：vitest：failed 节点 `redraftNode` 后出现在 `dispatchableNodeIds`；有 artifacts 的父节点不受影响。
- R3-6：Rust `parse_pi`：请求 `deepseek/deepseek-v4-flash` 响应 `deepseek-flash` 通过且 identity 为 `deepseek/deepseek-flash`；请求 `zai-coding-cn/glm-4.7` 响应 `glm-5.3-flash` 仍 identity_mismatch。资源冒烟：命中别名时 `smoke_model` 记实际 id，meta 追加「路由别名：deepseek-v4-flash→deepseek-flash」（`src/resources.rs` 冒烟成功分支，`identity.contains(model.id)` 这条对别名不成立，要用同一张 `KNOWN_MODEL_ROUTES` 判断），补测试。
- R3-7：Rust：原始输出 40KB 时留痕正文 ≤ 32KB + 指针行，raw 文件存在且内容完整；20KB 时原样内联。
- R3-8：Rust：`project_dir` 为空时 `preview_dispatch` 与 `dispatch_graph_stage` 都返回含「工作目录」的 Err，且 `current_dir` 下不出现任何新文件。
- R3-12：Rust：diff 与 changed_files 内容含 `sk-abcdef0123456789abcdef` 时产物里被打码且 `redacted ≥ 1`；盲目录 `diff.patch`/`changed/` 同样打码。
- R3-13：Rust：`project_is_git_work_tree` 注入不存在的 git 路径时返回 false 且 `artifact_diff_for_project` 走回退产出非空 diff；`materialize_snapshot` 失败后 `diff-*` 临时目录不存在。
- R3-14：vitest：`validate` 对 `role=验收, write=单写者` 与 `visibility=盲, write=worktree` 报错；Rust：盲节点请求带 `write=单写者` 时不拿写锁（用现有写锁测试的套路断言第二个盲节点不排队）。
- R3-16、R3-17：Rust：worktree 节点 `validation_output` 含「未跑验证命令」；`abandon_dispatch_state` 后存在 `STATE.abandoned-*.md` 且 `STATE.md` 不在；`continueUnfinished` 用的 run_id 与 STATE 头一致（vitest 或 Rust 任一侧）。

## 被推掉的必修项，本轮做

- R3-10：关闭图 tab 时清 `graphFocus`。关 tab 的动作在 `src-ui/src/components/center/CenterPanel.tsx` 的 tab 关闭按钮 → reducer `CLOSE_TERMINAL`（`src-ui/src/store/app-state.tsx`）。在 reducer 里：被关的 tab `tool === 'graph'` 且 `graphFocus.graphId === toolData` 时置 `graphFocus: null`。补 reducer 测试。
- R3-15：G1 在 `write ∈ {单写者, worktree}` 且 `agent = cc-switch` 时提示「cc-switch 单轮无工具，产物将为空」；`visibility=盲` 且 `agent = kimi` 时提示「kimi 无只读档，隔离=部分」。删掉 `fill_role_defaults` 的 `_models` 死参（`src/orchestration.rs`）。补 vitest（提示生成抽纯函数）。
- R3-18：`live_cc_switch_route` 缺 `AICROSS_CC_SWITCH_PATH` 时 `panic!("需要 AICROSS_CC_SWITCH_PATH 与 AICROSS_CC_SWITCH_PROVIDER")`；`live_economy_pipeline` 同样对缺失前置条件 panic 而不是静默通过。
- R3-9 补完：G1 决策行在设置了验证命令时显示「验证=<命令>」（已有写权限段）。

## 不采纳

- 无。
