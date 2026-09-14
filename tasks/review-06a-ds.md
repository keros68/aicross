## 1. 需求完整性

**1.1 A.1 的核心目的没打通：`rollback_before` 按「当前 run_id」找快照，而重派用的是新 run_id**
- 文件:行号：`src/dispatch.rs:1612`（`rollback_node_changes_core(project, run_id, …)`）、`src/dispatch.rs:1026`（快照写在 `.dispatch/<run_id>/<node>.before`）、`src-ui/.../DispatchOverlay.tsx:100-101`（请求 runId 取 state）、`:241-252`（`retry()` 里 `redraftNode` 后 `setRunId(crypto.randomUUID())`）。
- 问题：前端「重派/升级」保持 node_id 不变但换 run_id；引擎只查 `.dispatch/<新 run>/<node>.before`，那里从来不存在 → 返回「没有派发前快照，无需回滚」，半截改动照样叠在新一轮上。契约给 `rollback_before: bool` 这一形状本身不足以表达「上次是哪个 run」。
- 为什么：TASK-06 要修的就是「失败后重派叠在半截改动上」；现在的接线只能在同一 (run,node) 被派两次时生效，而前端从不这么干。测试 `dispatch.rs:3193` 两次 `run_one` 都用 run_id `"run"`，把这条掩盖了。
- 建议：三选一——① 引擎在 `.dispatch/*/<node>.before` 里取该节点最新快照（不依赖 run_id）；② before 快照改存与 run 无关的路径（`.dispatch/before/<node>/`）；③ 请求加 `rollback_run_id`，或 B 侧在 `retry()` 里用**旧** runId 直接调 `rollback_node_changes`（命令已存在）而不是只置 flag。同时把测试改成「两次派发用不同 run_id」。

**1.2 A.1 的 `runner_note` 结果字段在 ACP 路径被覆盖**
- 文件:行号：`src/dispatch.rs:1669`（`runner_note = rollback_note`）被 `:1681`、`:1701`、`:1710`、`:1716`、`:1726`、`:1730` 直接赋值覆盖。
- 问题：`节点.write=单写者 + acp_enabled + 支持 ACP` 时，「已回滚上次改动 N 文件」丢失；`previous_runner=="acp"` 的一次性回退路径同样丢。
- 为什么：任务书明写「结果 `runner_note` 记「已回滚上次改动 N 文件」」，留痕里查不到回滚发生过。
- 建议：这几处改 append（与 `:1718-1721` 追加 `output.runner_note` 同法），或把回滚信息单独成字段。

**1.3 A.3 禁外部动作挂错维度**
- 文件:行号：`fixtures/prompts/execution.txt:2` + `src-ui/src/graph/dispatch.ts:359-368`（模板只挂在 `role==='执行'`）。
- 问题：可写节点若不是「执行」角色（自定义文本 + 单写者/worktree），拿不到这句禁 external actions 的话；`claude` 白名单 `src/channels/mod.rs:176-181` 仍含 `Bash`，push/装包在 claude 上只有提示词拦。
- 为什么：A.3 的意图是「不可回滚的外部动作禁掉」，判据应是「可写」而不是「角色名」。
- 建议：在 `buildRequest` 里按 `target.write !== '只读'` 追加这句（与模板解耦），或在契约里明确「只覆盖执行角色 + claude 白名单只排除 web 工具」。

**1.4 契约「正文首行标记不再写入」只落了 Rust 半边**
- 文件:行号：`src/dispatch.rs:1556-1560`（只填字段，不写标记）；写入方在 `src-ui/src/graph/dispatch.ts:239-240`。
- 问题：标记一直是前端写的，本分支前端仍会写；合并 B 前，字段与标记会同时存在（`fallbackNoticeOf` 读标记 → 双份语义）。
- 为什么：契约是两侧共同遵守的，A 单独合并后行为与契约文字不符。
- 建议：合并时把 B.3 一起带上；A 的交付说明里把「Rust 未写过标记」写清楚（见 4.3）。

其余 A 节条目按契约落地：`reconcile_dispatch_state`/`ReconcileSummary`（`dispatch.rs:402-449`，`exists/runId/nodeId/recordPath/replyText` 形状对）、`rollback_node_changes`/`RollbackSummary`（`:453-458`、`:1062-1100`，无快照时 note 且不报错）、`before_dir` 追加（`:329`，`#[serde(default)]`）、`Reply.fallback_from/to`（`orchestration.rs:99-104`）、8MB 标记预留（`process.rs:13`、`:157`）、幂等拒绝（`dispatch.rs:585-591`）。

## 2. 逻辑正确性

**2.1 早早退场的重派会清空上一次的 before 快照**
- 文件:行号：`src/dispatch.rs:1827-1831`（无条件持久化）+ `:1031-1033`（写前 `remove_dir_all`）。
- 问题：同一 (run,node) 第二次派发若在真正发起前就失败（回滚失败、取消、prepare/build 报错），before/after 两次快照内容相同 → changed 集为空 → 旧快照被替换成只有空 `.created`/`.skipped` 的目录，之后 `rollback_node_changes` 只会说「已回滚 0 文件」。
- 为什么：恰好是「上次改动没跑起来、用户最需要回滚」的场合，恢复能力被静默抹掉。
- 建议：只在尝试真正发出请求后覆盖（例如 `status != "error" || 已进过通道`），或覆盖前把旧目录改名 `<node>.before.prev`；至少不要用空快照覆盖非空快照。

**2.2 回滚不防符号链接/连接点，可写到项目外**
- 文件:行号：`src/dispatch.rs:1074-1095`（`safe_relative_path` 只做词法检查：绝对路径/`..`/前缀），`fs::write`/`create_dir_all` 分别跟随最终路径与父目录链。
- 问题：agent 把项目内某个目录换成 junction/symlink（或把待恢复路径变成 symlink）后，回滚把内容写到项目外（`dispatch.rs:926` 只在快照时跳过链接，不管回滚目标）。
- 为什么：任务 3 明确要求「回滚不得越出项目目录」；这是唯一一条真正的逃逸面。
- 建议：恢复前对 `project` 起逐段 `symlink_metadata` 校验（父目录链 + 目标本身），命中 `is_link_or_junction` 就进 `skipped` 并写进 note；`.created` 的删除同理（删链接本身而不是跟随）。

**2.3 回滚「完整性」被高估：排除目录与链接里的改动既不记也不恢复**
- 文件:行号：`src/dispatch.rs:926-951`（快照跳过 `.git/.dispatch/node_modules/target/dist/build/vendor/__pycache__…` 与链接）、`:1050-1057`（只记 `.created`/`.skipped`）、`:1097`（note「已回滚上次改动 N 文件」）。
- 问题：agent 在 `dist/`、`vendor/` 或链接目录下新建/改动的文件不进 `.created`，回滚后残留，用户看到的却是「已回滚 N 文件」。
- 为什么：单写者回滚的语义被当作「恢复原状」使用（重派前先回滚），残留会继续污染下一轮。
- 建议：把「本次未纳入快照的排除项」在结果/note 里点明（或干脆在 `RollbackSummary.note` 里写「已回滚 N 文件；排除目录改动不在快照内」）。

**2.4 before 目录再快照复用带总量上限的快照器，超限即半途失败**
- 文件:行号：`src/dispatch.rs:1083` → `:905-989`（`SNAPSHOT_TOTAL_LIMIT = 128MB`，`dispatch.rs:756`）。
- 问题：`.before` 目录总字节 >128MB（单文件都 <8MB 时完全可能）时 `snapshot_directory(&root)` 直接 Err；此时 `.created` 里的文件**已经删掉**、before 文件一个都没恢复，命令返回 Err（`RollbackSummary` 形状丢失）。
- 为什么：契约要求「恢复 + 删除 + skipped」，不能出现部分删除后报错。
- 建议：回滚时直接 `read_dir` 遍历 before 目录（它本来就是受控内容），不要复用带总量/单文件上限的快照器；并把已完成的动作放进错误信息。

**2.5 对账的正文/产物解析基于「首次出现」的固定标记串**
- 文件:行号：`src/dispatch.rs:386-397`（`split_once("## 回复正文\n\n")` / `"## 产物 JSON\n\n"`），而 `write_record`（`:2227`）把 `## 任务全文`（含 prompt 与材料）放在这两个段之前。
- 问题：prompt/材料里出现 `## 回复正文` 或 `## 产物 JSON`（用户贴历史留痕、上游产物被内联、模型复述）时，`reply_text` 会从材料里截、`artifacts` 会解析失败静默为 `null`。
- 为什么：崩溃恢复把这份文本直接写回节点 Reply，错一截用户看不出来。
- 建议：改成从末尾定位（`rfind`）或按「最后一个 `## 产物 JSON` / `## 原始输出`」切；更稳的是把回复正文单独落到 `.reply.txt`，留痕只存指针。

**2.6 对账「取最新留痕」的判序不稳**
- 文件:行号：`src/dispatch.rs:427-437`（key = `(ended_at, attempt=="备", path)`）。
- 问题：`ended_at` 只到秒且用字符串比较，跨时区/偏移不同（夏令时、机器时区改动）时顺序会错；同一秒多条的兜底靠「备 > 主」和文件名。
- 为什么：verdict 是给用户「已完成/待核对」的判定，选错记录会让恢复拿到旧结论。
- 建议：解析成时间戳比较；同 key 时按「ok 优先」再按文件名，并在 note/日志里说明选了哪条。

**2.7 幂等拒绝的口径比契约宽**
- 文件:行号：`src/dispatch.rs:585-591`（只按 `run_id` 判定），契约写的是「同一 run_id + 同一节点集合」。
- 问题：同一 run 里想派另一批节点（不同 stage/节点集）也会被拒；本分支前端是串行 stage 所以没触发，但差异是实打实的。另外该登记只在进程内有效，重启即失效（崩溃恢复流程不受保护），也没有「第一次正常返回后第二次必须放行」的测试。
- 建议：按契约把节点集合纳入 key（或把口径回写进契约/注释说明「run 级互斥」），补一条 RAII 释放回归测试。

**2.8 worktree 退回单写者时既无快照也不回滚**
- 文件:行号：`src/dispatch.rs:1585`、`:1597`（`uses_single_writer=true`，`node.write` 仍是 `"worktree"`）、`:1612` 与 `:1829` 都用 `node.write == "单写者"` 判定。
- 问题：这条路径直接改项目目录，却既不落 before 快照、也不响应 `rollback_before`（`runner_note` 只有 worktree 警告）。
- 为什么：与「worktree 节点不需要回滚」的假设相反，它已经退化成单写者了。
- 建议：判定改用 `uses_single_writer`。

**2.9 `.created` 删除与文件复原的顺序**
- 文件:行号：`src/dispatch.rs:1074-1096`。
- 问题：先删新建文件、再恢复旧文件；若某条恢复失败（现在是返回 Err），`.created` 的删除无法撤回，且没有 `RollbackSummary` 返回。另外不回滚时留下的空目录也不清理。
- 建议：与 2.4 一起改成「收集计划 → 逐条执行 → 无论成败都返回 summary + note/error 列表」。

## 3. 安全

**3.1 符号链接逃逸（同 2.2）** — 回滚的写路径缺少链接校验，是本期唯一能越出项目目录的路径。建议按 2.2 处理。

**3.2 对账只读** — 未发现。`reconcile_dispatch_state`（`dispatch.rs:402-449`）只做 `read_dir`/`read_to_string`/`project_path`，无写、无删除、无 `STATE.md` 改动。

**3.3 禁外部动作的真实效力** — 见 1.3。补充事实：`codex` 可写仍是 `-s workspace-write`（默认禁网，`channels/mod.rs:216-224`）、只读 claude/pi 已排除 web 工具，唯一实质缺口是 claude 可写仍带 `Bash`、kimi 全靠提示词（文档已记，但报告里「claude 走 --tools 白名单」容易被读成 push/装包被拦）。

**3.4 `rollback_node_changes` 命令本身不取项目写锁**
- 文件:行号：`src/dispatch.rs:453-458`。
- 问题：契约允许前端直接调，但这条路径不加锁；若此时仍有单写者节点在跑，回滚会与派发互相踩（写一半被恢复）。
- 建议：命令内部用 `acquire_project_write`（或明确写进契约：只允许紧接在用 `rollback_before` 的派发请求里）。

## 4. 代码质量与契约

**4.1 只追加与向后兼容** — 未发现。`before_dir`（`dispatch.rs:328-329`）、`Reply.fallback_from/to`（`orchestration.rs:99-104`）都带 `#[serde(default)]`；`DispatchArtifacts` 仍是 snake_case 键（`dispatch.rs:131-140`），与 `src-ui/src/graph/types.ts:34-39` 的 `ReplyArtifacts` 一致，所以对账取回的产物能直接塞回 `Reply.artifacts`，不会二次脱敏（留痕正文与 stdout 在写入时已 `safe_trace`，回读不再处理，模式对 `[REDACTED]` 幂等）。

**4.2 命令注册** — 未发现。`reconcile_dispatch_state`、`rollback_node_changes` 都在 `src/server.rs:3952-3953` 的 `invoke_handler` 里。

**4.3 报告口径与代码不符（A.6）**
- 文件:行号：`tasks/REPORT-06A.md:17` 与 `src/dispatch.rs:1556-1560`。
- 问题：「备选结果不再在正文前面塞标记行」写成引擎改动；实际标记从来由前端写（`src-ui/src/graph/dispatch.ts:239-240`），Rust 侧只是新增两个字段。
- 为什么：读报告的人会以为线上回复正文里不再有标记，从而漏掉 B.3 的合并依赖。
- 建议：改成「Rust 侧只提供字段；标记行由前端在 B.3 停写」。

**4.4 报告里的「标记必达」测试是同义反复**
- 文件:行号：`src/process.rs:513-517`。
- 问题：断言的是 `interactive_queue_limit(marker)` 等于它自己的定义，以及 `X ≤ X`；没有验证「队列接近满时标记仍能入队」这个被修的行为（旧实现在队列 > MAX-marker 时丢标记）。
- 建议：见测试一节 T2。

**4.5 测试直接改全局静态，失败会毒化后续用例**
- 文件:行号：`src/dispatch.rs:3244-3250`（`active_runs().lock().unwrap().insert(...)`，清理在断言之后）。
- 问题：断言 panic 时 `Mutex` 中毒，之后所有派发用例都返回「派发运行状态不可用」，故障信息误导。
- 建议：用 `struct Reset(String); impl Drop` 或 `let _ = ...` + `catch_unwind` 清理。

**4.6 `dispatch_data_root` 的 `#[cfg(test)]` 早退**（`src/dispatch.rs:744-751`）：生产分支未变（仍 `aicross_root()`），逻辑正确；但两段 `#[cfg(test)]` 顺序敏感，可加一行注释说明「非 test 构建第一段被剔掉」。低。

**4.7 第 2/3/4/5 期路径** — 未发现实质回归：`run_one`/`run_fallback` 对只读与盲节点不触发回滚（`write` 强制只读），`live_economy_pipeline` 的 stage 2/3 复用同一 run_id 仍然合法（guard 在函数返回时释放，`:585`），验收 PASS/NEEDS_FIX 与修正轮走前端未改，ACP 分支只受 1.2 影响。

## 5. 测试

报告点名的测试都真实存在：`task06_reconcile_classifies_completed_incomplete_unknown_and_ignores_other_run`（`dispatch.rs:3145`，三态 + run_id 不同不算，断言了 verdict/reply_text/artifacts）、`task06_single_writer_rollback_restores_changed_deleted_and_created_files`（`:3172`，改/删/新建三态与 `restored`/`removed` 形状）、`task06_rollback_before_runs_inside_run_one_before_redispatch`（`:3193`，端到端但同一 run_id，见 1.1）、`task06_rollback_reports_over_eight_megabyte_snapshot_entries_as_skipped`（`:3230`，注入了伪造快照，只验 `.skipped` 回读）、`task06_duplicate_active_run_is_rejected_before_dispatch`（`:3242`）、`task06_dispatch_request_reads_rollback_before_camel_case`（`:2479`）、`task06_execution_prompt_forbids_external_side_effects`（`:2487`）、`writable_claude_excludes_web_tools_from_allowlist`（`channels/mod.rs:970`）、`task06_reply_fallback_fields_roundtrip_and_default_for_old_graphs`（`orchestration.rs:1158`）、`task06_fallback_reruns_once_uses_fields_without_reply_marker`（`dispatch.rs:3540`）、`interactive_stdout_truncation_marker_has_reserved_capacity_beyond_eight_megabytes`（`process.rs:513`，同义反复）。

最该补的 3 个：
1. **跨 run_id 的 `rollback_before` 端到端**（`run_one` 两次、run_id 不同），断言文件确实回到派发前内容——照现在的实现这条会失败，正好锁住 1.1。
2. **8MB 标记必达的行为断言**（`process.rs`）：先打一行 `MAX_INTERACTIVE_OUTPUT - 16` 字节并留在队列里，再打一行超限内容，断言第二行 `recv_line` 收到 `"[ACP stdout 已截断]"`（旧实现在这里丢标记）。
3. **回滚的安全/保全两条**：① 把待恢复路径或其父目录换成 symlink/junction，断言进 `skipped` 且项目外文件未被改写；② 同一 (run,node) 第二次派发提前失败（取消/构建失败）后，旧 `.before` 快照仍可用（覆盖 2.1）。

## 结论

**没有完整实现**：6 条里 5 条按契约落地，「单写者重派前先回滚上次改动」这条在真实前端流程里打不通（换 run_id 就查不到快照），另有几处安全与保全缺项。

缺项（按严重度）：
1. `rollback_before` 按当前 run_id 找快照 → 前端换 run_id 的重派路径**永远不会回滚**（`dispatch.rs:1612`、`DispatchOverlay.tsx:252`）。
2. 回滚写路径不防符号链接/连接点（可越出项目目录）；`.created` 删除先于恢复，超限/失败时报错返回、且旧快照会被早退尝试清空（`dispatch.rs:1074-1095`、`:1827-1831`）。
3. worktree 退回单写者时既不落 before 快照也不响应 `rollback_before`（`dispatch.rs:1585-1619`）。
4. ACP / 旧 ACP 回退路径覆盖 `runner_note`，丢掉「已回滚上次改动 N 文件」（`dispatch.rs:1681`、`:1701-1730`）。
5. 禁外部动作只挂在「执行」角色模板上，可写的自定义角色无此约束；claude 可写仍留 `Bash`（`execution.txt:2`、`dispatch.ts:359-368`）。
6. （低）对账解析用首次出现的固定段标记、时效排序用秒级字符串；`rollback_node_changes` 命令不取写锁。