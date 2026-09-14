# TASK-04 修正轮 R3c（只补测试）

R3b 的报告把 `tasks/fix-04-r3b.md` 里「必补测试」的 R3-1～R3-8、R3-12～R3-14、R3-16～R3-17 写成「沿用 R3 已实现的真实行为测试」——这些测试并不存在（编排者核对 diff：R3 新增 Rust 测试 0 条，R3b 新增 1 条）。本轮只做一件事：把 `tasks/fix-04-r3b.md`「必补测试」一节列出的每一条都写成**具名测试**，Rust 放进对应模块的 `#[cfg(test)]`，前端放进 `src-ui/src/graph/__tests__/` 或 `src-ui/src/__tests__/`。要求：

- 每条至少一个测试，测试名要能对上编号（Rust 例：`r3_1_artifacts_serialize_with_snake_case_keys`；vitest 例：`R3-2 异常只标记未收到结果的节点`）。
- 断言必须真的检查目标行为（键名、返回值、文件存在与否、错误文本）；不要 `assert!(true)`、不要只测编译。
- 只改测试文件；若写测试时发现实现有 bug，做最小修复并在报告里单列「测试逼出来的修复」。
- 需要注入假 git 路径、临时目录、假 `--help` 文本等的，用现有测试里已有的套路（`temp_root`、`AICROSS_TEST_RUN_ROOT`、`fixtures/`）。
- 在 `tasks/REPORT-04.md` 末尾新增「修正轮 R3c」一节，按编号列出测试名与文件:行。
- 验证：`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`（沙箱里原命令 EPERM 就用这个），命令与输出原文贴进报告。不起窗口，不发真机请求，不 commit，不 push。

对照表（编号 → 要断言什么）见 `tasks/fix-04-r3b.md`「必补测试」一节，逐条照做，不要合并、不要跳过。
