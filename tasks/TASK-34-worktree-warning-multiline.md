# TASK-34 worktree 警告多行时漏进 ANSWER.md（09-17）

对应 backlog 84。**只做这一件事。** 不碰 `src-ui/**`（另有 TASK-33 在改前端）。

## 问题

`run_one_attempt` 里 `worktree_warning = format!("worktree 不可用，已退回单写者：{error}")`（`src/dispatch.rs` 约 3804 行），再拼到 `stdout_summary` 前面、以换行分隔。`answer_summary` 只剥第一行。若 `{error}`（git 报错）含换行，第二行起的警告文字会被当成执行者回答写进盲审的 `ANSWER.md`——引擎提示进了盲审材料。

## 要做

1. 在生成 `worktree_warning` 时把 `{error}` 规整成单行（换行、回车替换为空格或「；」，连续空白合并），保证警告永远只占一行；这是根上的修法，不要只在 `answer_summary` 里猜多行边界。
2. 留痕 / 界面里原本显示这条警告的地方，确认单行化后信息不丢（报告里列出用到 `worktree_warning` 的位置）。
3. 补测试：构造多行 git 报错走到 worktree 回退，断言 `stdout_summary` 里警告只占第一行、`ANSWER.md` 不含任何警告文字；至少一格走 `dispatch_stage_core` 或 `run_one` 真实入口（参照 `task28_answer_strips_worktree_warning_prefix` 与 TASK-32 的测试写法）。先确认去掉实现会红。

## 验证

`cargo check --all-targets` 零 warning、`cargo test` 全量连跑 3 次、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。

保持文件原有换行；不 `cargo fmt`；不起窗口、不发真机请求、不调真实模型；不 commit、不 push。报告写 `tasks/REPORT-34.md`。backlog 不用改。
