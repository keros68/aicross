# TASK-08A 修正轮 R1

先读 `tasks/TASK-08-swarm.md`（契约 + A 节，含第 6 条）、`tasks/REPORT-08A.md`，再读 `tasks/review-08a-glm.md` 与 `tasks/review-08a-ds.md`（若存在）。逐条处理，每条在 `tasks/REPORT-08A.md` 末尾「修正轮 R1」一节写「已修：怎么改的（文件:行）／已补：测试名」或「不采纳：原因」。验证：`cargo check` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`，命令与输出原文贴进报告。不跑 `cargo fmt`、保持文件原有换行（`server.rs` CRLF、其余 Rust LF）；不动 `src/channels/`；不起窗口、不发真机请求、不 commit、不 push。

## 必修

1. **A6 未做：内置预设 `builtin-swarm`（Rust 侧）**：`orchestration.rs` 内置清单加「蜂群模式」——一个节点：role `拆分`、`write=只读`、`visibility=共享`、目标留空由 `fill_role_defaults` 按「出方案」填；内置总数 5→6，既有断言同步；补测试（盖章后节点字段与目标）。

2. **合并结果字段用 camelCase**（B 侧对账）：`merge_swarm_items` 返回项与 `check_swarm_plan` 返回项都 `#[serde(rename_all = "camelCase")]`（`nodeId`、`testOutput`），与前端 `tauri.ts` 其它派发结果一致；契约文件同步。补往返测试断言键名。
3. **拆分提示词单一来源**（B 侧对账）：前端暂用 `PRESET_NODE_TEXTS['builtin-swarm']` 顶替，A 侧 `split.txt` 落地后内置预设 `builtin-swarm` 的节点文本由引擎读 `fixtures/prompts/split.txt` 填入（与 advisor/premise-check 同一机制），前端不再重复注入；在报告里写明合并时 B 侧要删的那处文本。

## 复审补充

DeepSeek（`tasks/review-08a-ds.md`）判「没有完整实现」，缺项按严重度：

4. **合并留痕 `status` 不得写 `merge_result` 值**（DS 2）：留痕 `status` 只用既有取值（ok/error…），`merge_result` 单独放追加字段；否则 TASK-06 对账的 `status == ok → completed` 判定被撞坏。补测试：合并留痕过 `reconcile_dispatch_state` 不被误判。
5. **worktree 归属校验**（DS 3）：`merge_swarm_items` 只接受 `git worktree list` 里登记在本项目下的路径（canonicalize 后比对），否则该件 `skipped` + note。补测试。
6. **拆分角色进只读护栏**（DS 4）：引擎强制只读的角色集合加「拆分」，与前端 `validateNode` 同口径。补测试。
7. **worktree 内已被 agent 提交的件不算「无改动」**（DS 5）：以「worktree HEAD 是否领先于项目 HEAD（或有未提交改动）」判定；已领先的直接用其 HEAD 作为 cherry-pick 源（多个提交时按顺序逐个 pick 或 `--no-commit` 合成一个）。补测试。
8. **验证超时与写盘开关**（DS 6、9）：验证命令用 `settings.timeout_secs`；留痕落盘遵守 `write_project_dispatch`；`test_failed` 带 note；提交信息带 `<title>`；glob 目录条目与 `./` 前缀归一化；`check_swarm_plan` 改 `spawn_blocking`。
9. **`swarm_limit` 引擎侧生效**（DS 7）：`merge_swarm_items`/`check_swarm_plan` 超过上限返回 note 并只处理前 N 件（N 从设置读，默认 4）。补测试。
10. **测试补强**（DS 10）：worktree 无改动/不存在 → skipped；失败件在中间时后续件仍继续；合并留痕不影响对账；git 不存在时跳过并 eprintln。

## GLM 补充

GLM（`tasks/review-08a-glm.md`）与 DeepSeek 在 A6、worktree 归属、swarm_limit、主线程阻塞上一致。GLM 额外的：

11. **合并入参带 `title`**（GLM 2）：`merge_swarm_items` 的 items 追加 `title: string`（`#[serde(default)]`），提交信息用它；契约文件同步；补测试。
12. **失败件的重试闭环**（GLM 4）：`conflict`/`test_failed` 后再次合并同一件，worktree 已有该提交时不得判「没有改动」——以 worktree HEAD 领先于项目 HEAD 的提交为源（与第 7 条一起做）；补测试：第一次 test_failed、修好验证后第二次 merged。
13. **cherry-pick 失败分级**（GLM 5）：git 启动失败/超时不是 `conflict`，标 `skipped` + note；`cherry-pick --abort` 失败时整批停止并把后续件标 `skipped`，note 写明项目可能处于冲突态需人工 `git cherry-pick --abort`。补测试。
