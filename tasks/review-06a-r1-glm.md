# TASK-06A 修正轮 R1 复审

已通读 fix-06a-r1.md（16 条）、TASK-06-consistency.md 契约节（含 R1 追加）、REPORT-06A.md 修正轮 R1 节，以及 src/dispatch.rs（全文）、src/orchestration.rs、src/process.rs、src/server.rs（注册处）、src/channels/mod.rs（工具白名单处）。逐条结论：

## 16 条逐条核对

| # | 结论 | 依据（文件：行） |
|---|------|------|
| 1 | **已核实修好** | 生效模式贯通：`uses_single_writer` 初始化于 `src/dispatch.rs:1680`，worktree 失败置真 `:1690-1693`，写锁 `:1699`、回滚门 `:1707`、快照落盘门 `:1932` 均用它。E2E `task06_rollback_before_runs_inside_run_one_before_redispatch`（`:3308`）真在非 git 目录用 `write: worktree` 触发降级，两次派发用 run-a/run-b 两个 run_id，第二次 stub 里 `assert tracked.txt=='before'`（`:3335`）证明回滚发生在派发前，stub 失败即 status≠ok，断言是真的 |
| 2 | **已核实修好** | `latest_before_dir`（`:1103-1131`）扫项目 `.dispatch` 与运行目录留痕，按 node_id + 非空 before_dir + workdir canonical 匹配，取 `record_order_key` 最大；退路 `:1134`，无快照 note「没有找到派发前快照（首次派发或快照已清理）…」`:1137`。契约已补（`tasks/TASK-06-consistency.md:8`）。E2E 第二次用 run-b 却回滚的是 run-a 的快照，真实验证了不依赖当前 run_id |
| 3 | **已核实修好** | `safe_relative_path` 拒 RootDir/CurDir（`:1047-1052`，测试 `:3376`）；恢复前 `symlink_metadata` 识别链接先删再写（`:1167-1172`）；`create_dir_all` 后 canonicalize 校验父目录在项目内（`:1173-1178`，连带挡住「项目内符号链接目录」的传递穿透）；测试真造链接：Windows `symlink_file`，创建失败 eprintln 跳过（`:3517`）；`.created` 写 `/x` 的越界测试 `:3382` |
| 4 | **已核实修好** | 快照落 `log_dir`（write_project_dispatch=false 时即运行目录，`:1062-1067`），project 外记绝对路径（`:1096-1100`，测试 `task06_snapshot_outside_project_is_absolute_and_colocated_with_records :3394`）；persist 失败降为 runner_note 警告不翻 status（`:1932-1936`，测试 `:3439`）；`.gitignore` 靠 `ensure_log_dir` 在 `.dispatch` 层写 `*`（`:2297-2305`），按 git 语义覆盖 `<run>/` 子目录——机制成立但无专项断言（见末尾余项） |
| 5 | **已核实修好** | ACP 各分支全走 `append_runner_note`（`:1780/:1809/:1819/:1824/:1840`），output.note 追加式 `:1810-1813`，全程无直接赋值（`annotate_kimi_metadata` 的重赋值也保留原文 `:2238`）。测试 `:3424` 只覆盖 append 函数本身，无「回滚+ACP note 同存」端到端断言（余项） |
| 6 | **已核实修好** | `section()` 按行首标题取**最后一个**段、止于下一标题（`:400-411`）；测试含正文段内嵌「## 产物 JSON」字样 + 旧格式无产物段（`:3410-3421`） |
| 7 | **已核实修好** | `interactive_stdout_truncation_marker_is_delivered_at_full_queue_boundary`（`src/process.rs:513`）真压边界：队列到 MAX-16 后标记入队（`recv_line` 收到）、超限普通行 `try_recv_line` 返回 None；active run 测试用 Drop 守卫（`src/dispatch.rs:3361-3364`）且断言释放后可再进（`:3370`）；python 缺失 eprintln（`:3309/:3440/:3462`、`process.rs:501/:514`） |
| 8 | **已核实修好** | `ReconcileNode`/`ReconcileUsage` 带 `sessionId/runner/usage{input,output,cacheRead}`，serde camelCase（`src/dispatch.rs:248-268`），从 frontmatter 回填（`:470-472`）；测试断言 `("s","oneshot",(1,1,0))`（`:3277-3279`）及 acp 记录（`:3419`）；契约同步（TASK-06:8） |
| 9 | **已核实修好** | persist 门加 `entered_channel`（`:1932`），锁/回滚/快照/准备/取消/构建早退都不进通道、不触快照；`task06_early_failure_does_not_replace_previous_before_snapshot`（`:3461`）：首跑 ok 落快照，二跑 agent 不存在构建即败，回滚仍还原出 "old" |
| 10 | **已核实修好** | 回滚直接 `read_dir` 遍历 `.before`（`:1146-1161`），收集→排序→逐条执行→永远返回 summary，失败进 skipped；`.created` 删除在恢复之后（`:1181-1188`）；`ROLLBACK_TEST_TOTAL_LIMIT`（cfg(test) 512，`:787`）注入：513 字节文件完整还原（`:3482-3491`） |
| 11 | **已核实修好** | 命令先 `acquire_project_write` 30s（`:481-485`）；测试持锁时未生效、放锁后还原（`:3494-3507`） |
| 12 | **已核实修好** | `record_order_key` RFC3339→毫秒、同刻 ok 优先、再按路径（`:419-424`）；测试含跨时区同刻比较（`:3431-3436`）；「取最后一个标题段」与第 6 条共用实现 |
| 13 | **已核实修好** | 契约写明「进程内、run 级互斥…RAII 释放，进程重启后不保留」（TASK-06:13）；实现 `:614-620` + `ActiveRunGuard` Drop（`:826-834`）；测试验证释放后放行。唯一小缺口：`dispatch_stage_core` 处未加代码注释（契约已载明，纯装饰性） |
| 14 | **已核实修好** | 无快照 note（`:1137`）与成功 note（`:1193`）均含「排除目录（.git/node_modules/target/dist…）与链接内的改动不在快照内」；测试断言 `note.contains("排除目录")`（`:3303`） |
| 15 | **已核实修好** | 同第 7 条：`recv_line` 行为断言（`process.rs:521-523`）、Drop 守卫（`dispatch.rs:3361`） |
| 16 | **已核实修好** | `tasks/REPORT-06A.md:18` 已改为「Rust 侧只提供并保存字段；标记行由前端在 B.3 停写」，禁外部动作按可写挂载转 06B |

## 无新缺陷（抽查结论）

- **server.rs 注册**在位：`src/server.rs:3952-3953`。
- **留痕契约只追加**：`DispatchRecord` 仅增 `before_dir` 且 `#[serde(default)]`（`dispatch.rs:339-340`），两个 legacy 解析测试（`:2843/:3057`）通过。
- **第 2/3/4/5 期路径无回归**：`run_one` 主/备流（`:1589-1599`）、`run_fallback` 只换目标 + session 置空 + 字段不带标记行（`:1614-1655`，测试 `:3810` 断言 `!text.starts_with("（已切备选：")`）、ACP 分支 `resolve_acp_attempt` 语义未动、验收只读门禁测试仍在（`:4152`）。`channels/mod.rs` 只有 06A 首轮的白名单（`:970` 测试在），未动 acp.rs。
- 测试 `:3328-3329` 有一处死赋值（`write` 先设「单写者」随即被 `:3330` 覆盖为 "worktree"），仅外观问题。

## 最终结论

**R1 之后 TASK-06A 达到可交付**——16 条全部闭环且关键项（生效模式、run_id 无关的快照定位、链接不穿透、早退不覆盖、写锁互斥）都有真实行为断言背书，验证面（cargo 261 项、vitest 221、build 零 warning、沙箱外 live ACP 续接）覆盖主路径；剩余全是低危边角，不阻塞合并：

1. （低-中）`persist_before_snapshot` 先 `remove_dir_all` 旧目录再写（`dispatch.rs:1068-1070`）：真正进过通道后、持久化中途 IO 失败的窄窗口里旧快照仍会丢——DS 2.1 的 `.before.prev` 改名备选未做；可留给下一轮。
2. （低）`latest_before_dir` 对 workdir 无法 canonicalize 的留痕按匹配处理（fail-open，`dispatch.rs:1121`），理论上有跨项目误选可能，靠 `before_dir.is_dir()` 兜底。
3. （低）第 4 条 `.gitignore` 覆盖子目录、第 5 条「回滚+ACP note 同存」均无专项测试，靠机制/单测间接保证。
4. （装饰）第 13 条契约已写明但 `dispatch_stage_core:614` 处无代码注释；测试 `:3328` 死赋值。