# TASK-07 收尾与 G2 空转提示

来源：`tasks/fix-04-r5-backlog.md` 第 11 条（G2 疑似空转提示，依据 2606.01365）+ TASK-06 两轮复审剩余的低危项（`tasks/review-06a-r1-{glm,ds}.md`、`tasks/review-06b-r1-{glm,ds}.md`）。两块并行、各自工作树；契约先定在本文件。

## 契约（只追加）

- Tauri 事件 `dispatch-progress` 的载荷 `DispatchProgress` 追加可选字段（camelCase）：`elapsedMs: number`、`idleMs: number`（距上一条输出/通知的毫秒数）、`toolCalls: number`（本次尝试的工具调用次数，ACP 才有，一次性为 0）、`repeatedToolCalls: number`（本次尝试内连续同名工具调用次数）、`lastTool: string`。`status` 新增值 `"running"`（周期心跳，每 15s 一次，节点结束时最后一条仍是原有终态）；终态事件不带心跳字段，前端保留上一条 `running` 的 `elapsedMs`/`lastTool`。旧前端忽略未知字段与未知 status。
- `Reply` 追加可选 `tokens_cache_read?: number`（TS）/ Rust 镜像 `#[serde(default)]`，对账与派发结果都写它。

## A. 引擎（Rust）— codex

1. **进度心跳**：`run_one` 期间每 15s `emit_progress(status="running")` 带 `elapsedMs`/`idleMs`；ACP 节点从 `session/update` 的 `tool_call` 通知累计 `toolCalls`，按 `title` 连续相同计 `repeatedToolCalls`，`lastTool` 记标题；一次性节点 `idleMs` 按子进程最后一次 stdout/stderr 输出计（`process.rs` 的读线程记时间戳）。心跳线程在节点结束/取消后停止。补测试：假 ACP server 连发 3 次同名 tool_call → 事件里 `repeatedToolCalls == 3`；一次性 stub 静默 2s → `idleMs ≥ 2000`。不动 `src/channels/acp.rs` 的协议部分之外的行为（只加计数）。
2. **before 快照写前保留旧目录**（06A-R1 复审 GLM 1）：`persist_before_snapshot` 先把旧 `.before` 改名 `.before.prev`（覆盖上一个 `.prev`），写成功后再删 `.prev`；写失败时把 `.prev` 改回。补测试。
3. **进通道但零改动的重派不覆盖非空快照**（DS 06A-R1 3）：after == before 时不写空快照、不动旧目录。补测试。
4. **`latest_before_dir` 的 workdir 过滤 fail-closed**（DS 06A-R1 2）：canonicalize 失败即不匹配。补测试。
5. **留痕旧字段补 `serde(default)`**（DS 06A-R1 5）：`DispatchRecord` 全部字段可缺省，跨版本旧留痕不退化为 unknown；`before_dir` 文档注明绝对路径例外；`dispatch_stage_core` 幂等处加代码注释；回滚 note 报 `skipped` 条数。
6. **`usage.cacheRead` 回填**：对账与派发结果都填 `Reply.tokens_cache_read`（Rust 镜像 + `DispatchNodeResult` 已有 `cacheRead`）。补测试。
7. **测试补强**（DS 06A-R1 4）：8MB 标记测试加同步点（等生产者处理完再 recv）；`task06_rollback_reads_before_directory_beyond_snapshot_total_limit` 真用一个小上限常量注入证明不读上限；幂等释放半边走 `dispatch_stage_core`。

验证：`cargo check` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。不跑 `cargo fmt`、保持文件原有换行（`server.rs` CRLF、其余 Rust LF）。不起窗口、不发真机请求、不 commit、不 push。报告写 `tasks/REPORT-07A.md`。

## B. 界面（TS）— Claude 子代理

1. **G2「疑似空转」提示**：监听 `dispatch-progress`（`tauri.ts` 类型按契约追加，缺字段按 0 处理），结果屏/节点卡的运行中状态显示 `elapsed`、`lastTool`；纯函数 `idleHint(progress, now)`：`repeatedToolCalls ≥ 3` 或 `idleMs ≥ 180000` → 返回「疑似空转：同一工具连续 N 次」/「3 分钟无输出」，UI 在该节点旁显示胶囊提示 + 现有「取消」按钮（文案「叫停」）。不用红色。测试：三种输入（正常/重复工具/长时间无输出）。
2. **对账说明随派发收敛**（DS 06B-R1 3）：派发成功后从 `needsReview` 剔除已不在 `dispatchableNodeIds` 的 id，`reconcileNote` 一并清；测试。
3. **`Reply.tokens_cache_read`**：类型追加，`applyDispatchResults` 与 `applyReconcile` 写入，结果屏 cache_read 列优先读它；测试。
4. **`start()` 幂等停止分支整链测试**（GLM 06B-R1 3 / DS 5）：把 `start()` 的循环体抽成可注入 `dispatch` 函数的纯异步函数并测「重复批次 → 本层复位 draft + 提示」。
5. **小质量**（DS 06B-R1 7）：`global.d.ts` 的 `node:fs` 手写声明改成与 `shiki.test.ts` 同一套写法；`.dispatch-warning` 只留给异常，其余常规提示（「单步小任务」「复核，非独立验证」「已切备选」）改 `dispatch-reason`；`applyReconcile` 注释补「无 reply 节点按非 draft 跳过」；报告 R0 行号漂移不用改。

验证同 A（vitest 用 `--configLoader native --pool threads`）。不动 `src/`；`tauri.ts` 新字段按契约先写。不起窗口、不 commit、不 push。报告写 `tasks/REPORT-07B.md`。
