# REPORT-24 待办册清理 · 引擎批二

## 结果

完成 TASK-24 指定的 21、22、25、26、33、37 六条。条目 37 经用户授权，只额外修改 `src-ui/src/components/center/graph/DispatchOverlay.tsx` 与一个对应测试文件；其余 `src-ui/**` 和任务书列出的在飞文件未动。

没有 commit、push、`cargo fmt`、启动窗口、发送真机请求或调用真实模型。四件套均已实际执行并通过。

## 逐条改动

### 21. 回退删除路径进入 note 与留痕

- 不改变既有回退语义；`restore_swarm_paths` 只额外返回实际删除的未跟踪路径。
- 验证失败回退后将清单写入结果 note，随后由既有合并记录写入留痕。
- 清单复用 `summarize_paths`：最多展开 20 条，超出后写 `等 N 项`。

### 22. 冲突判定改为三态

- `unmerged_swarm_paths` 改为 `Result<Vec<String>, String>`：非空为有冲突，空清单为无冲突，命令失败或输出截断为无法判定。
- 无法判定时结果按 `conflict` 处理，不再降级成 `skipped`；note 明写「冲突状态无法判定」。
- 测试覆盖有冲突、无冲突、查询失败和输出截断，并覆盖无法判定时的端到端结果。

### 25. 临时补丁移入派发目录

- 选择将临时补丁写入该次派发记录目录下的 `patches/`，即项目内记录开启时为 `.dispatch/patches/`，外置记录时跟随对应的运行记录目录。
- 选择该方案是因为补丁与派发留痕共用位置和排除规则，不依赖下次启动清理，也不会把项目 diff 留在系统 Temp。
- 补丁仍只作为 `git apply` 的输入，内容不写入 prompt、回复或其它模型上下文；应用后按原流程删除。

### 26. 入口脏检查过滤排除目录

- `check_swarm_plan` 的 `git status` 改为 `--porcelain=v1 -z --untracked-files=all`，并拒绝截断输出。
- 计划检查与执行入口都复用 `excluded_swarm_path`，`__pycache__` 等排除目录不再把整批判为脏。
- 测试同时确认计划检查为 clean，且同一缓存残留不会阻止后续合并。

### 33. 额度回滚失败进入派发记录

- 回滚失败时统一追加结果 `runner_note`，并同步改写该次尝试已生成的派发记录 frontmatter。
- 返回前端的 `frontmatter` 与磁盘记录同步，记录正文中的任务、回复、产物和原始输出保持不变。

### 37. 结果屏只显示本轮跳过

- 推进循环每次提交图状态时，收集本轮新写入 `skipped_reason` 的节点 id。
- 结果屏按这组 id 过滤，不再扫描全图；上一轮已跳过节点不会混入本轮。
- 遗留节点仍沿用独立的本轮 `leftoverIds`，行为未改。

## 红测证据

六条均先在旧实现上运行新增断言并确认失败，随后再实现修复。

| 条目 | 测试 | 旧实现的实际失败 |
|---|---|---|
| 21 | `task24_validation_dirty_path_note_uses_twenty_path_summary` | note 只有「验证命令改动了项目文件」，缺少「回退删除了未跟踪路径」；exit 101。 |
| 22 | `task24_unmerged_query_failure_or_truncation_is_not_downgraded_to_skip` | 查询失败注入后实际结果为 `skipped`，预期 `conflict`；exit 101。三态另由 `task24_unmerged_query_distinguishes_conflict_none_and_unknown` 固定。 |
| 25 | `task24_swarm_patch_is_created_under_this_dispatch_directory` | 实际路径为系统 Temp 下的 `aicross-swarm-*.patch`，不在项目 `.dispatch/`；exit 101。 |
| 26 | `task24_excluded_cache_does_not_dirty_plan_or_skip_merge_entry` | 只有 `src/__pycache__/generated.pyc` 时，计划检查仍返回 `dirty=true`；exit 101。 |
| 33 | `task14_r1_writable_quota_stops_when_rollback_fails` | 返回结果含「回滚本次改动失败」，磁盘派发记录的 `runner_note` 仍为空；exit 101。 |
| 37 | `TASK-24 结果屏只收集并显示本轮新增的已跳过节点` | 旧实现没有本轮 id 收集函数，定向 Vitest 报 `collectRunSkippedIds is not a function`；exit 1。 |

最终上述定向测试全部通过，全量 Rust 与前端测试也覆盖并通过这些用例。

## 验证

### `cargo check --all-targets`

```text
Compiling aicross v0.1.0 (D:\work\aicross-p4)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.57s
```

退出码 0，零 warning。

### `cargo test`

```text
running 470 tests
test result: ok. 459 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 29.45s
```

统一终端的 PTY 在后段测试结束时未返回退出码，因此同一条 `cargo test` 另将输出写到系统临时日志；测试程序给出上述最终 `test result: ok`。未增加参数、未运行 ignored 的真实模型测试。

### `cd src-ui && npx vitest run --configLoader native --pool threads`

```text
Test Files  39 passed (39)
Tests  593 passed (593)
Duration  1.89s
```

退出码 0。控制台有既有的 `NO_COLOR` / `FORCE_COLOR` 提示，不是测试 warning 或失败。

### `cd src-ui && npm run build`

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build
✓ 343 modules transformed.
✓ built in 2.17s
```

退出码 0。

## 文件与边界

- 修改：`src/dispatch.rs`、`src-ui/src/components/center/graph/DispatchOverlay.tsx`、`src-ui/src/graph/__tests__/review-defaults-12.test.ts`、`tasks/fix-04-r5-backlog.md`。
- 新增：`tasks/REPORT-24.md`。
- `src/dispatch.rs` 保持 LF；未触碰 CRLF 的 `src/server.rs`、`src/main.rs`。
- 除用户为条目 37 授权的两个前端文件外，没有修改其它 `src-ui` 文件。
