# TASK-34 审查 · Antigravity / gemini-3.8-flash-high

**总判：可交付但有未闭合项**（核心单行化逻辑与真实测试接线均已做到，但测试注入钩子的静态变量未用 `#[cfg(test)]` 隔离，在生产构建中仍被编译存在）。

---

### 问题清单

#### [低] 1. `src/dispatch.rs:505` 测试注入变量在生产构建中未做 `#[cfg(test)]` 隔离
- **文件:大致位置**：`src/dispatch.rs:505`（`thread_local!` 块内）
- **问题**：`create_worktree` 内的读取逻辑加了 `#[cfg(test)]`（约 2841 行），但在 `thread_local!` 声明处的 `static WORKTREE_TEST_ERROR` 缺少 `#[cfg(test)]` 属性。在非测试构建（如 `cargo build` 或发布二进制）中，该 TLS 变量仍会被编译进二进制，虽然生产运行时不会被访问，但未达到“测试注入钩子只在测试构建存在”的完整隔离要求（注：同文件现存的 `SWARM_TEST_LAST_PATCH_PATH` 等前人钩子亦有此现象，但本任务新引入变量应更规范）。
- **复现**：非测试构建（如查看未带 `--test` 的编译单元或二进制符号）下，该静态变量依然被定义并分配 TLS 插槽，且由于使用点被 `#[cfg(test)]` 剔除，该变量在生产编译期完全未被使用。
- **改法**：在 `thread_local!` 块内部，为 `static WORKTREE_TEST_ERROR` 声明加上 `#[cfg(test)]` 属性：
  ```rust
  #[cfg(test)]
  static WORKTREE_TEST_ERROR: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
  ```

---

### 核过没问题的要点

1. **单行化入口唯一且彻底，无其它拼装旁路**
   - 单行化实现在 `src/dispatch.rs:3811`，紧跟 `create_worktree` 返回 `Err(error)` 分支之后、在组装 `worktree_warning` 之前完成。
   - 这是 `worktree_warning` 拼入 `stdout_summary` 并进入产物的唯一源头。
   - 使用 `error.split_whitespace().collect::<Vec<_>>().join(" ")`，将全部 `\r`、`\n`、`\t` 及连续空白折叠为单个空格，保证警告字符串绝对不含换行符，且前后无冗余空白；下游 `answer_summary` 剥离第一行后，绝无可能渗漏第二行及后续错误文字到盲审的 `ANSWER.md`。

2. **留痕与界面显示不丢失关键报错信息**
   - 空白归整只改变格式与排版，`fatal:`、`hint:`、错误码及文字内容等所有语义信息 100% 保留。
   - 报告已完整梳理了 5 个关键消费位置（`run_one_attempt`、`DispatchArtifacts.stdout_summary`、`write_record` 留痕 JSON、前端 `DispatchNodeResult.artifacts` / Reply / `artifactSummary`、下游盲审 `ANSWER.md`），留痕和展示层信息无损。

3. **测试注入钩子的运行时安全**
   - `create_worktree` 中消费钩子的代码块（`WORKTREE_TEST_ERROR.with(...)`）被 `#[cfg(test)]` 明确包裹，生产运行时绝对不会触发注入逻辑。
   - 钩子使用 `.take()` 取出并消费错误，同线程后续调用或失败时均已复位为 `None`，不会污染其他测试用例。

4. **测试走真实接线且能够真实红测**
   - 测试通过 `run_one` 真实入口发起执行（指定 `write = "worktree"`），真实调用 `create_worktree`、命中回退单写者逻辑、真实调度本地 Python stub 进程输出答案，并最终通过 `prepare_outbound_test` 真实生成盲审目录与 `ANSWER.md`。
   - 红测证实：在去除单行化实现时，测试因 `stdout_summary` 含有换行导致两行结构被打破而立即失败（若继续往下走也会因 `ANSWER.md` 包含 `hint:` 等警告行而失败），测试具有有效证伪能力。

5. **任务边界与验证要求**
   - 未改动 `src-ui/**` 目录；
   - 未改动 backlog；
   - `cargo check --all-targets` 0 warning，`cargo test` 全量连跑 3 次通过，前端 Vitest 与 Build 均通过；
   - 文件保持原有 LF 换行，未执行 `cargo fmt`，未提交或推送。

---

编排者核对：第 1 条不成立——该 `thread_local!` 块整体位于 `#[cfg(test)]` 之下（dispatch.rs 约 492 行），生产构建不存在该变量。
