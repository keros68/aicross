# REPORT-26-R1 TASK-26 复审修正轮

## 完成内容

### 1. 归类运行目录精确清理

- 保留 `proposal-classify-${proposal_id}` 的并发隔离，在后端真实派发入口为这一类 run 增加 RAII 收尾。
- 只对当次的完整 `run_root` 执行 `remove_dir_all`，不扫描也不按前缀批量删除；run id 含路径分隔符或冒号时不启用清理。
- 收尾在正常返回和建目录后的提前错误路径都会执行；目录不存在视为已清理，其他删除错误只写 `stderr` 日志，不改变派发结果。
- `task26_r1_classification_run_directory_is_removed_after_success_and_error` 通过真实 `dispatch_stage_core` 和本地 Python stub 验证成功节点，再制造 `dispatch` 为普通文件的建目录错误，两条路径结束后当次目录均不存在。
- `task26_r1_concurrent_classification_runs_remove_only_their_own_directories` 并发跑两个真实派发入口，断言两个目录各自删除，同前缀的无关哨兵目录及其文件保留。

### 2. 恢复原生 `isContentEditable` 优先判定

- `proposalTargetFlags` 在 `isContentEditable` 为布尔值时直接采信浏览器计算结果，因此外层 `true` 内的 `false` 遮蔽区不再被误当为输入控件，`plaintext-only` 也能正确识别。
- 只在该原生属性不可用的测试替身或非 HTMLElement 目标上回退选择器：`[contenteditable="true"],[contenteditable=""],[contenteditable="plaintext-only"]`。
- 回归用例均通过 `proposalShortcutFromTarget` 真实快捷键接线，覆盖嵌套 `false`、原生 `plaintext-only`、回退路径的 `false` / `true` / 空值 / `plaintext-only`。

### 3. 备选真跑后回写主尝试记录

- 新增 `task26_r1_logged_out_after_failed_fallback_updates_main_record`：资源库 `fallback_order` 含两条，备 1 经本地 stub 真实运行后因额度错误失败，备 2 因未登录跳过。
- 用例确认实际调用顺序只有 `p0` 和 `p1`，并从真实留痕目录找到 `attempt: 主` 的文件，断言其 `runner_note` 含「跳过 cc-switch/logged-out（未登录）」。该场景使得最终结果文件与主尝试文件不同，确实进入了本轮要求的主记录回写分支。

## 保持，不改

- 保留「一次派发只读一次资源库」的冻结语义；在快照读取处补了注释：主尝试开始后的登录与额度变化在下一次派发生效。未改动读取时机。
- 保留 66 的连带效果：预设下拉打开时终端焦点守卫也不抢焦点。本轮未扩展实现；既有 `proposalShortcutFromTarget` 用例继续覆盖下拉打开时的 Esc 放行。

## 红测确认

先增测试、保留修正前实现时执行：

```text
npx vitest run src/__tests__/proposal-20.test.ts --configLoader native --pool threads
Test Files  1 failed (1)
Tests  2 failed | 20 passed (22)
```

- 嵌套 `contenteditable="false"` 用例期望 `"accept"`，修正前因 `closest` 命中外层 `true` 得到 `null`。
- 回退路径的 `plaintext-only` 用例期望 `null`，修正前选择器漏掉它而得到 `"accept"`。

```text
cargo test task26_r1_ -- --nocapture
running 3 tests
task26_r1_classification_run_directory_is_removed_after_success_and_error ... FAILED
task26_r1_concurrent_classification_runs_remove_only_their_own_directories ... FAILED
task26_r1_logged_out_after_failed_fallback_updates_main_record ... ok
test result: FAILED. 1 passed; 2 failed
```

- 两个清理用例都在 `assert!(!runtime.join("runs").join(...).exists())` 处失败，证明修正前当次与并发归类目录会残留。

57 的分支是上一轮已有实现，因此新用例加入后先通过；随后临时屏蔽 `result.dispatch_file != main_record.0` 的回写分支做变异验证：

```text
cargo test task26_r1_logged_out_after_failed_fallback_updates_main_record -- --nocapture
running 1 test
task26_r1_logged_out_after_failed_fallback_updates_main_record ... FAILED
test result: FAILED. 0 passed; 1 failed
```

失败输出显示主尝试记录仍为 `runner_note: ''`，且不含「跳过 cc-switch/logged-out（未登录）」。变异后已恢复正式实现。

恢复正式实现后的 TASK-26 定向结果：

```text
cargo test task26_ -- --nocapture
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 469 filtered out

npx vitest run src/__tests__/proposal-20.test.ts src/lib/__tests__/focus-enforcer.test.ts --configLoader native --pool threads
Test Files  2 passed (2)
Tests  28 passed (28)
```

## 完整验证

```text
cargo check --all-targets
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.46s
```

零 warning。

```text
cargo test
test result: ok. 464 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 23.63s
```

```text
cd src-ui && npx vitest run --configLoader native --pool threads
Test Files  40 passed (40)
Tests  609 passed (609)
```

```text
cd src-ui && npm run build
✓ 347 modules transformed.
✓ built in 556ms
```

Vite 仍输出既有的 500 kB chunk size 提示；构建成功。

## 范围与限制

- 本轮只改了 `src/dispatch.rs`、`src-ui/src/components/center/ProposalBar.tsx`、`src-ui/src/__tests__/proposal-20.test.ts` 和本报告；其余已有未提交文件保持上一轮状态。
- TASK-26 禁区文件均未修改；`src/server.rs`、`src/main.rs`、`src-ui/src/tauri.ts` 也未修改。
- `src/server.rs`、`src/main.rs`、`CenterPanel.tsx` 保持 CRLF，其他本轮涉及的 Rust/TSX 文件保持 LF，`src-ui/src/tauri.ts` 的混合换行未触碰；`git diff --check` 无输出。
- 未运行 `cargo fmt`；未启动窗口，未发真机请求，未调用真实模型。测试中的成功/失败调用仅使用本地 Python stub。
- 未 commit，未 push。
