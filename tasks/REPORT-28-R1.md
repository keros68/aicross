# TASK-28 修正轮报告

## 完成内容

### 1. 保留带 worktree 警告回复的截断事实

- `answer_summary` 在剥离 `worktree 不可用，已退回单写者：` 警告后，同时返回该摘要是否已触及既有 2000 字截断上限。
- 带警告时依据原始 `stdout_summary` 的长度保留截断事实；不带警告时仍依据答案正文长度判断。
- 没有新增序列化字段。

红测方式：先只加入 `task28_r1_warning_prefixed_answer_keeps_truncation_fact`，运行：

```text
cargo test task28_r1_ -- --nocapture
```

修正前结果：失败。带 worktree 警告、原始摘要已达到 2000 字但剥离警告后约 1970 字的答案没有截断提示；同一测试也覆盖“警告 + 正文恰好 2000 字”应有提示，以及“警告 + 正文 1500 字”不应有提示。

### 2. 全部空答案时不生成 ANSWER.md

- 所有上游产物都没有 diff 和 changed files 时，先剥离 worktree 警告，再检查答案正文。
- 若全部正文经 `trim()` 后为空，则不写 `ANSWER.md`，不加入材料名，也不向 prompt 加说明或内联全文。

红测方式：先只加入 `task28_r1_all_empty_answers_do_not_create_answer_material`，运行同一条定向命令。

修正前结果：失败。空白答案与只有 worktree 警告的答案仍生成了 `ANSWER.md`。

### 3. 拒绝附件与盲审材料重名

- 在复制附件前保留已生成的 `REQUEST.md`、`diff.patch`、`changed`、`ANSWER.md`；启用验证材料时同时保留 `VALIDATION.txt`。
- 附件命中保留名时返回：`附件文件名与盲审材料重名：…`。
- 既有“附件之间重名”的检查与错误文案保持不变。

红测方式：先只加入 `task28_r1_answer_attachment_name_conflict_is_rejected`，运行同一条定向命令。

修正前结果：失败。名为 `ANSWER.md` 的附件被接受，并覆盖了生成的盲审答案材料。

## 红转绿结果

三条测试加入后、实现修改前：

```text
cargo test task28_r1_ -- --nocapture
test result: FAILED. 0 passed; 3 failed
```

实现修改后：

```text
cargo test task28_r1_ -- --nocapture
test result: ok. 3 passed; 0 failed

cargo test task28_ -- --nocapture
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 475 filtered out
```

首版已有一格继续通过 `dispatch_stage_core` 真实入口，修正轮没有改成仅测辅助函数。

## 四件套

1. `cargo check --all-targets`
   - 通过：`Finished dev profile [unoptimized + debuginfo] target(s) in 5.64s`
   - Cargo/Rust 编译零 warning。
2. `cargo test`
   - 通过：`473 passed; 0 failed; 11 ignored`，耗时 25.00s。
3. `cd src-ui && npx vitest run --configLoader native --pool threads`
   - 通过：`Test Files 42 passed (42)`；`Tests 635 passed (635)`；耗时 1.21s。
   - 输出仍有环境中既有的 `NO_COLOR` / `FORCE_COLOR` 提示。
4. `cd src-ui && npm run build`
   - 通过：转换 348 个模块，`built in 558ms`。
   - 输出仍有环境中既有的颜色变量提示和大于 500 kB 的 chunk 提示。

## 保持不变与范围

- 保留 `strip_prefix('\r')` 兼容处理。
- 不改变 Codex/Kimi 多段可见文本的既有聚合行为。
- 不改变 `src-ui/src/graph/types.ts` 中首版新增字段的可选性。
- 修正轮代码只改 `src/dispatch.rs`，另新增本报告；没有修改 TASK-27 禁区文件，也没有改 `tasks/fix-28-r1.md`。
- 没有运行 `cargo fmt`，没有启动窗口，没有发真实模型请求，没有 commit 或 push。

## 编排者核对后的更正（09-17）

修正清单第 1 条是**错的**，责任在编排者：`run_one_attempt` 先把 `channel_output.text` 截到 2000 字、**再**在前面拼 worktree 警告，所以剥掉警告后的正文就是截断后的回复，首版「正文满 2000 字才算截断」本来就对。两家审查与编排者自己都误读成「警告 + 正文合计 2000 字」。

R1 按清单改成「带警告时按整段长度判」，反而引入误标：警告 + 1970 字的完整回复会被标成截断。编排者已改回按正文长度判，并把测试的第一格改为「警告 + 1970 字完整回复 → 不标截断」（测试更名为 `task28_r1_warning_prefix_does_not_change_truncation_fact`）；该格在 R1 写法下为红。其余两条（空答案不出文件、附件与材料重名报错）保留。
