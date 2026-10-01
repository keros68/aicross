# TASK-36 R1 复审（Claude 子代理，10-01）

原文较长，以下是编排者核实后的要点；逐条核对表的结论：30 条中 23 条已修、7 条部分修（R1-6、R1-8、R1-20、R1-26、R1-29 等），无未修。

## 新问题（编排者已对照代码核实）

- **高-1** `normalized` 的 `strip_prefix_ci` 按字节切片 `value[..prefix.len()]`（`src/engine/mod.rs` 约 1863 行），路径前 8 字节含中文时 panic；`pick_project_graph` 对图目录里每张图都调用，任一图的项目路径含中文即所有 dispatch 崩溃，退出码 101。R1 之前的实现不会崩。
- **高-2** 同一项目上的并发 run 整图互相覆盖：`start_dispatch` 续写同项目最新的图，落图写本进程内存里的整张图（约 2301-2306 行），没有移植 TS `mergeConsoleCommit`（`console-channel.ts:351-355`、`:408-410`）。后果：后写盘的 run 抹掉别的 run 的节点与结果；R1-8 的收尾把别的 run 的 pending 节点误标失败。
- **中-1** `adopt` 改图后会被后台下次整图写回覆盖；`adopt` 还整写 run 文件，可能把已落盘的 `done` 写回 `running`。
- **中-2** 后台进程启动时不检查 run 是否已被判 `failed`（约 2760-2768 行），启动超过 30 秒时会「复活」并真的派发，造成重复执行。
- **中-3** `run_if_subcommand` 对 `__run` 也调用 `attach_parent_console()`（`src/console_cli.rs:16`），后台进程会挂回父进程控制台，宿主终端关闭时可能被一起结束。
- **低** 取消标记出现在「循环开头检查」与「登记在飞」之间时本阶段跑完才停；判定取消后丢弃了本阶段已返回的结果；R1-20 第一轮 DFS 在全部祖先上做，顺序与 TS 只在允许集内 DFS 不同；R1-8 收尾未刷新 run 摘要的 `nodes`；`preserve_order` 是本轮首次全局开启（`Cargo.lock` 中 serde_json 新增 indexmap 依赖），影响整个程序 `serde_json::Map` 的键序。
- **金标准不足** 输入时间重建按记号编号而非原始先后分配，依赖 `created_at` 先后的逻辑钉不住；检查顺序只钉了两对。
- **报告失真** REPORT-36 R1 节称「dispatch_stage_core 成功后清理取消注册表」「orchestration.rs 删 stamp」，与基线对不上；Cargo.toml 注释称 preserve_order「之前已由 schemars 传递开启」不实。
