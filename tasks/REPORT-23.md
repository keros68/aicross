# REPORT-23 待办册清理 · 引擎批

## 结果

完成 TASK-23 指定的 3、8、9、23、24、34、35 七条。没有改任务书列出的在飞文件，没有 commit、push、`cargo fmt`、启动窗口或调用真实模型。

四件套均已实际执行。Rust 两项全绿；前端测试与构建分别被本期范围外、且涉及在飞任务或禁区文件的既有问题拦住，原始结果见“验证”。

## 逐条改动

### 3. 验证失败文本

- ANSI 清理补齐 CSI、BEL 结尾 OSC 和 ST 结尾 OSC。
- 验证错误继续统一写成 `exit=1`；无退出码写成 `exit=未知`。
- `safe_trace` 与验证失败正文共用同一个清理函数，因此留痕与 Reply 走同一处理。

### 8. 带引号的新增文件补丁头

- 当前基线已经包含带引号 `before/`、`after/` 前缀的改写实现，本期补强回归测试。
- 测试覆盖 `+++ "after/..."`，并确认引号内的 git 八进制转义保持原样。

### 9. kimi 留痕说明

- 当前基线已经写入「kimi 不报真身」「kimi 不报用量」，本期补强回归测试。
- 测试同时确认该补注只作用于 kimi，不改变 codex 等其它 agent。

### 23. 回退后清理补丁新建的空目录

- 应用补丁前记录声明路径中尚不存在的父目录。
- 回退文件后按深度从内向外删除其中仍为空的目录；项目根不进入清单，补丁前已有的空目录也不进入清单。
- 回归测试同时覆盖“新建多层目录被清理”和“既有空目录保留”。

### 24. 验证脏路径清单截断

- 验证脏路径 note 改为复用 `summarize_paths`，只展示前 20 条，并按既有格式写 `等 N 项`。
- 回归测试制造 21 条脏路径，确认第 20 条可见、第 21 条不展开且出现 `等 21 项`。

### 34. 额度恢复时间误识别

- `resets 数字` 不再单独构成额度语义；只接受 `reset ... at/in`、`retry-after` 或已有明确额度文本。
- ISO 恢复时间只从 reset/retry-after 语境中提取，不再把错误正文开头的日志时间当成恢复时间。
- 回归测试覆盖 `server resets 3 connections` 与额度错误前置日志时间两个反例。

### 35. 额度状态码漏判

- 状态码语境补齐 `status 429` 和 `HTTP/1.1 429`。
- 裸 `429` 仍不命中；测试覆盖两个正例与一个裸数字反例。

## 红测证据

每条测试均确认过撤掉对应实现会失败；临时变异随后已恢复。

| 条目 | 测试 | 红测结果 |
|---|---|---|
| 3 | `r5_3_validation_error_strips_ansi_and_formats_exit_code` | 原实现只删 CSI，实际结果仍含两段 OSC；exit 101。 |
| 8 | `r5_8_quoted_new_file_diff_headers_are_normalized` | 临时撤掉带引号 `after/` 改写后，`+++ "b/\346\226\260 file.txt"` 断言失败；exit 101。 |
| 9 | `r5_9_kimi_missing_identity_and_usage_are_explicit_in_trace_fields` | 临时撤掉 kimi 补注后，identity 实际为空而预期为「kimi 不报真身」；exit 101。 |
| 23 | `task23_failed_patch_removes_only_directories_created_by_that_patch` | 原实现删文件后 `new/deep` 仍存在；exit 101。 |
| 24 | `task24_validation_dirty_path_note_uses_twenty_path_summary` | 原实现展开 `dirty-00.txt` 至 `dirty-20.txt`，没有 `等 21 项`；exit 101。 |
| 34 | `task34_quota_reset_does_not_use_unrelated_reset_prose_or_log_timestamp` | 恢复旧逻辑后输出 `reset_prose_detected=true, log_reset=Some("2026-09-16T08:30:00+08:00")`；exit 101。 |
| 35 | `task35_quota_classifier_accepts_status_code_context_without_bare_numbers` | 恢复旧状态码正则后输出 `status_space=false, http_version=false, bare_number=false`；exit 101。 |

最终七条定向测试均通过；全量 Rust 测试也覆盖并通过这些测试。

## 验证

### `cargo check --all-targets`

```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.58s
```

退出码 0，无 warning。

### `cargo test`

```text
running 466 tests
test result: ok. 455 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 23.16s
```

### `cd src-ui && npx vitest run --configLoader native --pool threads`

```text
Test Files  1 failed | 38 passed (39)
Tests  1 failed | 584 passed (585)
FAIL  src/settings/__tests__/ui-scale-19.test.ts
AssertionError: expected [ 1, 2, 3, 4, 5, 1, 2, 3, 4, 5, 6 ] to deeply equal [ 1, 2, 3, 4, 5 ]
```

退出码 1。失败测试从 `docs/PRODUCT.md` 的 TASK-19 决定段开始取到文末，把后续另一组编号也计入。本期未改 `docs/PRODUCT.md` 或该测试；修复它不属于七条，并可能与在飞任务冲突，因此未越界处理。

当前 worktree 原先没有 `node_modules`，首次 `npx` 被 npm 离线缓存拦截。为实际运行上述命令，临时复用了 `D:\work\aicross\src-ui\node_modules`；两边 `package-lock.json` SHA-256 相同。验证后已删除 worktree 内的临时 junction，外部依赖目录未改动。

### `cd src-ui && npm run build`

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build

src/commander/launcher.ts(219,3): error TS6133: 'installed' is declared but its value is never read.
```

退出码 1。`src-ui/src/commander/**` 在 TASK-23 的“不要碰”清单中，本期未修改该文件，也未处理这个在飞改动造成的构建错误。

## 文件与边界

- 修改：`src/dispatch.rs`、`src/channels/mod.rs`、`tasks/fix-04-r5-backlog.md`。
- 新增：`tasks/REPORT-23.md`。
- `src/dispatch.rs`、`src/channels/mod.rs` 保持 LF；未触碰 CRLF 的 `src/server.rs`、`src/main.rs`。
- 没有修改任何任务书禁止触碰的前端目录或文件。
