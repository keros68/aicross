# REPORT-38 每次派发新建一张图

实现：Claude 子代理（10-01）。子代理环境不允许它创建报告文件，本文由编排者按其交回的全文写入。

## 改了什么（行号指改后的文件）

- `src/engine/mod.rs`
  - 2634–2636：`start_dispatch_with` 给 `plan_dispatch` 的 `existing` 固定为 `None`，附注释说明派发之间不共享上下文；删掉原 2630 行 `let existing = latest_project_graph(root, &input.project);`。
  - 1888–1891：`normalized` 的文档注释去掉对 `pick_project_graph` 的引用，加 `#[cfg(test)]`。编译器确认它在生产路径已无调用方（只剩金标准重放与它自己的单元测试），不加该属性 `cargo check --release` 报 dead_code。
  - 删除原 1911–1933：`pick_project_graph`、`latest_project_graph`。
- `src/engine/golden.rs` 327–339：`pick_project_graph` 移到这里，金标准重放仍用它从夹具 `existingGraphs` 挑同项目最新图喂给 `plan_dispatch`。`plan_dispatch`、夹具、`src-ui/` 未改。
- `src/engine/tests.rs`
  - 删除 `plan_dispatch_works_with_chinese_project_graph_in_dir`（原 737–796）：它测 `latest_project_graph` 扫图目录遇中文路径不 panic，该路径已不存在。路径归一的多字节用例由 `normalized_handles_multibyte_paths` 覆盖，保留。
  - 新增 `start_dispatch_creates_new_graph_each_time`（1332–1408）：同一项目连续两次 `start_dispatch_with`（注入 spawn，不起进程）；第一次后在第一张图上补一个遗留节点并落盘；断言第二次 `graph_id` 不同、第二张图不含第一张图任何节点 id 与「第一次任务」正文、第二张图的遗留为空、第二次 run 文件 `graph_id` 指向第二张图且 `leftover` 为空、第一张图原样保留。
- `docs/CONSOLE-CHANNEL.md` 182–184：写明每次 `aicross dispatch` 新建一张图、派发之间不共享上下文，追问同一件事由调用方把前情写进任务文本；删掉「多个命令行后台 run 交替提交同一张图」，保留后台提交按节点合并、`adopt` 采纳标记不被抹掉、窗口与命令行同写无并发保护。
- `tasks/fix-04-r5-backlog.md` 第 103 条：划掉，注明「已修（TASK-38，每次派发新建图）」。

Rust 文件保持 LF，未跑 cargo fmt，未 commit。

## 回退验证

临时把 `existing: None` 换回旧的「读 graphs/、按 normalized 比项目、取 updated_at 最新」逻辑，`cargo test start_dispatch_creates_new_graph_each_time` 失败于 `src\engine\tests.rs:1392`：`assertion left != right failed: 第二次派发应新建一张图`。改回后通过。

## 验证

- `cargo check --all-targets`：`Finished dev profile`，warning / error 行数 0。
- `cargo check --release`：`Finished release profile`，warning / error 行数 0。
- `cargo test`：564 passed，0 failed，12 ignored；`engine_golden_fixtures_replay` 覆盖全部 31 个夹具，全绿。

## 没做的

- 窗口侧 `src-ui/` 的选图逻辑 `selectConsoleGraph` 未改：按任务书只改命令行派发，窗口界面将在 TASK-40 下线。
