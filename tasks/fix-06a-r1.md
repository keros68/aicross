# TASK-06A 修正轮 R1（引擎：回滚/对账收口）

先读 `tasks/TASK-06-consistency.md`（契约 + A 节）、`tasks/REPORT-06A.md`，再读 `tasks/review-06a-glm.md` 与 `tasks/review-06a-ds.md`（若存在）。逐条处理，每条在 `tasks/REPORT-06A.md` 末尾「修正轮 R1」一节写「已修：怎么改的（文件:行）／已补：测试名」或「不采纳：原因」。验证：`cargo check` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`，命令与输出原文贴进报告。不跑 `cargo fmt`、不整文件重排、保持文件原有换行（`server.rs` CRLF、其余 Rust LF）；不动 `src/channels/acp.rs`；不起窗口、不发真机请求、不 commit、不 push。

## 必修（按严重度）

1. **生效模式而非声明字段**（GLM 2-①）：worktree 建失败退回单写者时（`uses_single_writer == true`），before 快照落盘与 `rollback_before` 都要按生效模式执行；`dispatch.rs` 里两处 `node.write == "单写者"` 的条件改用 `uses_single_writer`。补 `run_one` 端到端：非 git 目录里 `write: worktree` 的节点 → 退回单写者 → 落快照 → 第二次带 `rollback_before` 派发前文件被复原。
2. **快照定位不依赖当前 run_id**（GLM 2-②）：`rollback_before` 时按 `node_id` 找该节点最新一条留痕的 `before_dir`（扫 `.dispatch/*.md` frontmatter，取 `ended_at` 最新且 `before_dir` 非空的），找不到再退到 `.dispatch/<当前 run_id>/<node_id>.before/`；都没有时 `runner_note` 写「没有找到派发前快照（首次派发或快照已清理）」，不要写「无需回滚」。契约文件 `tasks/TASK-06-consistency.md` 「契约」节补一句「回滚按 node_id 找最新快照，不要求复用 run_id」。补测试：换 run_id 重派仍回滚。
3. **回滚越界两处**（GLM 3）：`safe_relative_path` 拒绝 `Component::RootDir` 与 `CurDir`；恢复文件前 `symlink_metadata(target)`，目标是符号链接/junction 就先删链接再写（或进 `skipped` 并记 note），`create_dir_all(parent)` 后用 `canonicalize` 校验父目录仍在项目内。补测试：`.created` 里写 `/x` 不写出项目目录；恢复目标为链接时不穿透（Windows 用 junction 或 `symlink_file`，创建失败就跳过并 eprintln）。
4. **`write_project_dispatch=false` 时的快照位置与失败降级**（GLM 1）：该开关关掉时 before 快照落到运行目录（与留痕同处），`before_dir` 记绝对路径；开着时 `.dispatch/<run>/` 目录也要被 `.gitignore` 兜底（`ensure_log_dir` 已写 `.gitignore` 的话确认覆盖到子目录）。`persist_before_snapshot` 失败不得把成功派发翻成 error：降级为 `runner_note` 警告。补测试两条。
5. **回滚 note 在 ACP 路径被覆盖**（GLM 4）：ACP 成功/重置分支对 `runner_note` 改为追加（现有 `push_str("；")` 模式），补断言。
6. **对账解析按行扫描**（GLM 2-④）：`reply_text` 取「## 回复正文」到下一个 `## ` 标题之间的段，不用 `split_once`；无产物段的旧留痕也能取回正文。补测试：正文内嵌「## 产物 JSON」字样、旧格式无产物段。
7. **测试补强**（GLM 5）：`interactive_stdout_truncation_marker_*` 改成真压边界（队列填到 MAX 后标记仍入队、普通行不入队）；`task06_duplicate_active_run_*` 补 guard 释放半边（第一次结束后第二次能进）；python 缺失时的 E2E 跳过要 eprintln。

8. **对账结果带 session/用量/runner**（GLM-06B 复审 2，契约追加）：`ReconcileNode` 追加 `sessionId: string`、`runner: string`、`usage: { input, output, cacheRead }`（camelCase），从留痕 frontmatter 取；`tasks/TASK-06-consistency.md` 契约节同步。补测试。

## DeepSeek 补充

DeepSeek（`tasks/review-06a-ds.md`）与 GLM 在第 1、2、3、5、6 条上一致（换 run_id 不回滚、worktree 退回单写者、符号链接越界、runner_note 覆盖、对账解析）。DeepSeek 额外的：

9. **早退的重派不得清空上一次的 before 快照**（DS 2.1）：同一节点第二次派发在真正发出请求前就失败（回滚失败/取消/构建报错）时不覆盖旧快照；只在尝试真正进过通道后才写；或覆盖前把旧目录改名 `.before.prev`。至少不用空快照覆盖非空快照。补测试。
10. **回滚不复用带上限的快照器**（DS 2.4、2.9）：回滚时直接 `read_dir` 遍历 `.before` 目录；顺序改成「收集计划 → 逐条执行 → 无论成败都返回 `RollbackSummary`（失败项进 `skipped` + `note`）」，不再中途 Err；`.created` 删除不早于恢复。补测试：`.before` 总量超 128MB 仍完整回滚（用小上限常量注入或 cfg(test) 阈值）。
11. **`rollback_node_changes` 命令取项目写锁**（DS 3.4）：命令内部走 `acquire_project_write`，与派发互斥；补测试。
12. **对账排序与解析**（DS 2.5、2.6）：`ended_at` 解析成时间戳比较，同刻按 ok 优先再文件名；正文/产物定位从末尾（`rfind`）或按最后一个「## 产物 JSON」/「## 原始输出」切（与第 6 条一起做）。
13. **幂等口径写清**（DS 2.7）：要么按契约把节点集合纳入 key，要么把「run 级互斥、进程内有效」写进契约与代码注释；补 RAII 释放后第二次放行的测试。
14. **回滚 note 点明未纳入快照的排除项**（DS 2.3）：`RollbackSummary.note` 与 `runner_note` 加一句「排除目录（.git/node_modules/target/dist…）与链接内的改动不在快照内」。
15. **测试卫生**（DS 4.4、4.5）：`process.rs` 的标记测试改成行为断言（队列填到 `MAX-16` 后再来一行超限内容，`recv_line` 收到标记）；`task06_duplicate_active_run_*` 用 Drop 守卫清全局表，避免 Mutex 中毒。
16. **报告口径**（DS 4.3）：A.6 改成「Rust 侧只提供字段；标记行由前端在 B.3 停写」。禁外部动作按「可写」而非角色挂载属 B 侧（已转给 06B R1）。
