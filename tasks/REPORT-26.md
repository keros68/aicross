# REPORT-26 待办册清理 · 建议条与资源页批

## 完成内容

### 56. 删除 `detect_into` 的 pi 生产死分支

- 删除 `detect_into` 内直接执行 pi 探测的旧分支；pi 仍只走 `prepare_pi_detection` → `apply_prepared_pi_detection`，探测阶段不持有资源库锁。
- `detect_into` 对 pi 增加明确拒绝，避免以后误接回旧路径时静默恢复旧语义。
- 同步调整忽略的本地事实源测试，使其也走正式的 pi 预处理路径。

删除前执行：

```text
rg -n "detect_into\(" src/resources.rs
842:fn detect_into(
2356:        detect_into(&mut store, agent_id, &mut errors);
2388:            detect_into(&mut store, &agent_id, &mut errors);
2620:        detect_into(&mut store, &id, &mut errors);
4475:            detect_into(&mut store, id, &mut errors);
```

生产调用核对：2356 所在分支对 pi 已由 `prepared_pi` 截走；2388 位于 `agent_id == "pi"` 的 `else`；2620 的 id 是刚创建的 `manual-*`；4475 是 `#[ignore]` 真机测试。因此没有生产调用把 pi 传进 `detect_into`。

### 57. 一次派发复用资源快照并补主尝试记录

- 派发开始时只读取一次资源库，显式备选和资源库备选都从同一份快照取 login 与额度时间。
- 跳过未登录备选时，返回结果与主尝试派发记录的 `runner_note` 都写入说明；若已经发生过备选尝试，也会单独更新最初的主尝试记录。

### 58. 固定 pi 登录判定口径

- 在 `pi_login_from_check` 旁写明：可解析到 `status: "ready"` 但退出码非 0 时判为未登录；未检查只用于命令不可用、超时或没有可解析结论。
- 新增聚焦测试固定该行为，不改生产判定。

### 66. 预设下拉打开时建议条放行 Esc

- `hasOpenModal` 复用既有判定入口，把原生 `<details class="preset-select" open>` 纳入放行条件。
- 建议条捕获阶段不再吞掉 Esc，后续冒泡监听可关闭预设下拉。

### 67. 精确识别可编辑目标

- 输入目标选择器改为 `input,textarea,[contenteditable="true"],[contenteditable=""]`。
- 移除宽泛的 `isContentEditable` 分支；显式 `contenteditable="false"` 不再被当作输入控件，true 与空值仍排除快捷键。

### 68. 并发归类留痕隔离

- 采用 runId 带建议 id 的方案：`proposal-classify-${proposal_id}`，nodeId 同步使用该值。
- 选择该方案是因为它保留可预期的 `proposal-classify-` 前缀，同时直接隔离并发留痕，不引入串行等待。

## 红测确认

先加回归断言、保留旧实现时执行定向测试：

```text
npx vitest run src/__tests__/proposal-20.test.ts --configLoader native --pool threads
Test Files  1 failed (1)
Tests  3 failed | 18 passed (21)
```

- 66：旧实现返回 `"dismiss"`，新断言要求 `null`，证明建议条仍会吞 Esc。
- 67：旧实现对 `contenteditable="false"` 返回 `null`，新断言要求 `"accept"`。
- 68：旧请求仍为 `runId/nodeId: "proposal-classify"`，新断言要求分别为 `proposal-classify-p1` 与 `proposal-classify-p2`。

Rust 定向红测：

```text
cargo test task26_ -- --nocapture
running 3 tests
resources::tests::task26_detect_into_excludes_pi_prepared_outside_the_store_lock ... FAILED
resources::tests::task26_ready_status_with_nonzero_exit_is_logged_out_by_policy ... ok
dispatch::tests::task26_logged_out_fallback_uses_one_resource_snapshot_and_updates_main_record ... FAILED
test result: FAILED. 1 passed; 2 failed
```

- 56：旧边界返回允许 pi，失败于 `assertion failed: !detect_into_supports_agent("pi")`。
- 57：旧实现一次派发读资源状态 3 次，失败于 `left: 3 / right: 1`。实现后再临时退回只更新返回结果的旧写法，测试在主记录仍为 `runner_note: ''` 处失败。
- 58：现有行为本来就符合任务要求；为确认测试不是自证，临时把非零退出码分支改为 `Unknown`，测试失败为 `left: Unknown / right: LoggedOut`，随后恢复。

恢复正式实现后的定向结果：

```text
cargo test task26_ -- --nocapture
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 469 filtered out

npx vitest run src/__tests__/proposal-20.test.ts src/lib/__tests__/focus-enforcer.test.ts --configLoader native --pool threads
Test Files  2 passed (2)
Tests  27 passed (27)
```

## 验证

```text
cargo check --all-targets
Finished `dev` profile [unoptimized + debuginfo] target(s) in 15.90s
```

零 warning。

```text
cargo test
test result: ok. 461 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 23.50s
```

```text
cd src-ui && npx vitest run --configLoader native --pool threads
Test Files  40 passed (40)
Tests  608 passed (608)
```

```text
cd src-ui && npm run build
✓ 347 modules transformed.
✓ built in 1.46s
```

Vite 仍输出既有的 500 kB chunk size 提示；构建成功。

## 范围与限制

- 只修改 56/57/58/66/67/68 及其测试、backlog 状态和本报告。
- 未修改 TASK-25 禁区文件；未修改 `src/server.rs`、`src/main.rs`、`src-ui/src/tauri.ts`。
- 修改的 Rust 与前端文件均保持 LF；未运行 `cargo fmt`。
- 未启动窗口、未发真机请求、未调用真实模型。
- 未 commit、未 push。
