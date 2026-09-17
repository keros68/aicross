# REPORT-28 只读问题的盲审答案文件

## 完成内容

### 1. 空改动产物补充 `ANSWER.md`

- 盲目录收到上游产物后，只有全部 `diff` 都为空且全部 `changed_files` 都为空时，才从各产物的 `stdout_summary` 生成 `ANSWER.md`。
- `ANSWER.md` 加入材料名单；小于 30KB 时以 `[ANSWER.md 全文]` 内联进 prompt，并追加「执行者未改文件，其最终回复见 ANSWER.md」。
- 单个上游直接写回答，不加标题；多个上游按 `## 上游节点 <source_node_id>` 分节。旧产物没有来源字段时回退为顺序号。
- 为保留真实上游节点标识，`DispatchArtifacts` 只追加可选的 `source_node_id`，在执行产物形成时由 Rust 写入；前端只在获准的 `src-ui/src/graph/types.ts` 增加同名可选字段，没有修改 TASK-27 的控制台、组件、store 或手册文件。
- 先剥离 `worktree 不可用，已退回单写者：…` 警告首行，再按与 `REQUEST.md` 相同的外发开关调用 `redact_text`。已有 2000 字摘要在文件末尾追加「（回复较长，已截断至前 2000 字）」。
- 核对了各通道的 `ChannelOutput.text` 来源：Claude 取 `result`，Codex 取 `agent_message`，Kimi 取 assistant 正文并排除 tool，pi 取最终 assistant text，ACP 取最终回复；原始输出另存。`cc-switch` 的用量行在 stderr 的 `[usage]` 中，测试 stub 也按该契约构造。因此未发现需要改用其他最终回复字段的 runner，没有把原始输出或工具轨迹写进 `ANSWER.md`。

### 2. 非空 diff 保持原行为

- 任一上游的 `diff` 非空或 `changed_files` 非空时，不生成 `ANSWER.md`，不在 prompt 提及它。
- 盲目录仍精确为 `REQUEST.md`、`changed`、`diff.patch`，返回的材料名单仍为 `REQUEST.md`、`diff.patch`、`changed/`。
- 未修改修正轮、验收结论解析、`stdout_summary` 的 2000 字长度及原有用途，也未改任何既有留痕字段名。

### 3. 产品口径

- `docs/PRODUCT.md` 在盲审材料说明处补充：只读任务没有 diff 和改后文件时，最终回复作为 `ANSWER.md` 提供；答案是交付物，不是推理过程。

## 测试与红测

新增 6 个 TASK-28 回归测试，其中 `task28_readonly_answer_reaches_blind_review_through_dispatch_stage_core` 使用本地 Python stub 连续跑执行、盲验收两个阶段，确实经过 `dispatch_stage_core`。它断言执行节点没有文件改动，下游盲目录出现回答原文，任务 prompt 同时含指路句与内联全文。

先加入测试、保留旧实现时执行：

```text
cargo test task28_ -- --nocapture
running 6 tests
test result: FAILED. 1 passed; 5 failed; 0 ignored; 0 measured; 475 filtered out
```

- 真实入口、多上游、警告剥离、脱敏、截断 5 格均因 `ANSWER.md` 不存在而失败。
- 非空 diff 反例在旧实现下本来就通过；实现后临时移除「diff / changed_files 必须全空」保护再跑，失败为目录实际多出 `ANSWER.md`：

```text
cargo test task28_nonempty_diff_does_not_write_answer -- --nocapture
left: ["ANSWER.md", "REQUEST.md", "changed", "diff.patch"]
right: ["REQUEST.md", "changed", "diff.patch"]
test result: FAILED. 0 passed; 1 failed
```

- 多上游测试追加真实来源节点 ID 后，临时改回顺序号标题做变异验证，失败为实际 `上游节点 1/2`、期望 `上游节点 execute-a/execute-b`。随后恢复正式实现。

恢复实现后的定向结果：

```text
cargo test task28_ -- --nocapture
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 475 filtered out
```

## 四件套

```text
cargo check --all-targets
Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.34s
```

零 warning。

```text
cargo test
test result: ok. 470 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 25.67s
```

```text
cd src-ui && npx vitest run --configLoader native --pool threads
Test Files  42 passed (42)
Tests  635 passed (635)
Duration  1.81s
```

Vitest 输出既有的 `NO_COLOR` 被 `FORCE_COLOR` 忽略提示；测试通过。

```text
cd src-ui && npm run build
✓ 348 modules transformed.
✓ built in 662ms
```

Vite 仍输出既有的 500 kB chunk size 提示；构建成功。

## 范围与限制

- 只修改 `src/dispatch.rs`、获准的前端契约文件 `src-ui/src/graph/types.ts`、`docs/PRODUCT.md` 与本报告。
- `src/dispatch.rs` 与 `src-ui/src/graph/types.ts` 保持 LF；`docs/PRODUCT.md` 保持原有混合字节计数（CRLF 392、LF 5），与修改前一致。`git -c core.whitespace=cr-at-eol diff --check` 无输出。
- 未运行 `cargo fmt`；未启动窗口，未发真机请求，未调用真实模型。测试只调用本地 stub。
- 未 commit、未 push；未修改 backlog。
