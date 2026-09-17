# REPORT-34 worktree 警告多行规整

## 结果

已完整实现 TASK-34。worktree 创建失败回退到单写者时，错误中的换行、回车、Tab 与连续空白会统一折叠为单个空格；警告仍保留全部非空白文字，但永远只占 `stdout_summary` 第一行。下游盲审的 `ANSWER.md` 只包含执行者回答，不再混入多行 git 错误的后续行。

只修改了 `src/dispatch.rs`，并新增本报告；未修改 `src-ui/**`。

## 实现

`run_one_attempt` 在 `create_worktree` 返回错误后、生成 `worktree_warning` 之前，对错误执行 `split_whitespace().join(" ")`。这个位置是警告进入产物前的根入口，因此没有扩大 `answer_summary` 的猜测规则，也没有改变既有警告前缀、回答截断规则或留痕字段。

`worktree_warning` 的使用与传播位置核对如下：

1. `src/dispatch.rs` 的 `run_one_attempt`：worktree 创建失败时生成警告，并回退到项目目录的单写者执行。
2. 同函数构造 `DispatchArtifacts.stdout_summary` 时：警告位于第一行，执行者回答从第二行开始；错误为空回答时只保留警告。
3. `write_record`：`DispatchArtifacts` 序列化到派发记录的“产物 JSON”，所以留痕继续保留完整错误文字，只把原错误中的空白分隔规整为单个空格。
4. `DispatchNodeResult.artifacts`：同一产物返回给界面；`src-ui/src/graph/dispatch.ts` 将其写进节点 Reply，`src-ui/src/graph/context.ts` 的 `artifactSummary` 仍通过 `stdout_summary` 展示/组装该警告。单行化不删除任何非空白错误文字。
5. 下游盲审：`answer_summary` 按既有 `WORKTREE_WARNING_PREFIX` 剥掉第一行，再由 `readonly_answer_material` 写 `ANSWER.md`。本次没有修改这段消费逻辑。

## 测试与红测

新增 `task34_multiline_worktree_error_stays_out_of_answer_material`：通过仅测试可用的错误注入让 `create_worktree` 返回包含 CRLF、连续空格和 Tab 的多行 git 风格错误，再走真实 `run_one` 入口完成 worktree 回退和本地 stub 执行。测试断言：

- `stdout_summary` 精确为一行完整警告加一行“最终答案”，总计两行；
- 警告保留 `fatal: 第一行`、`hint: 第二行` 与 `末尾`；
- 下游盲审目录的 `ANSWER.md` 精确等于“最终答案”，不含 `worktree`、`fatal:` 或 `hint:`。

先只加入测试和错误注入、暂不加入空白规整，运行：

```text
cargo test task34_multiline_worktree_error_stays_out_of_answer_material -- --nocapture

running 1 test
thread 'dispatch::tests::task34_multiline_worktree_error_stays_out_of_answer_material' panicked at src\dispatch.rs:8866:9:
assertion `left == right` failed
  left: "worktree 不可用，已退回单写者：fatal: 第一行\r\n  hint: 第二行\t末尾\n最终答案"
 right: "worktree 不可用，已退回单写者：fatal: 第一行 hint: 第二行 末尾\n最终答案"
test result: FAILED. 0 passed; 1 failed
```

加入规整实现后，同一命令结果：

```text
running 1 test
test dispatch::tests::task34_multiline_worktree_error_stays_out_of_answer_material ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 495 filtered out
```

因此该测试在去掉本次实现时会红，且失败点就是多行错误未经规整。

## 最终验证

```text
cargo check --all-targets
Finished `dev` profile [unoptimized + debuginfo] target(s) in 14.82s
0 warning

cargo test  （连续第 1 次）
test result: ok. 485 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 32.55s

cargo test  （连续第 2 次）
test result: ok. 485 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 28.27s

cargo test  （连续第 3 次）
test result: ok. 485 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 30.65s

cd src-ui && npx vitest run --configLoader native --pool threads
Test Files  45 passed (45)
Tests  656 passed (656)

cd src-ui && npm run build
tsc -b && vite build
350 modules transformed.
✓ built in 2.23s
```

Vitest 与构建仍输出既有的 `NO_COLOR` 被 `FORCE_COLOR` 忽略提示；构建仍有单个 chunk 超过 500 kB 的既有提示，命令均以 0 退出。`git diff --check` 通过。`src/dispatch.rs` 修改前后均为纯 LF，未改变原有换行格式。未运行 `cargo fmt`。

## 未做

- 未启动窗口。
- 未发真机请求，未调用真实模型。
- 未修改 `src-ui/**`、backlog 或任务书。
- 未 commit，未 push。
