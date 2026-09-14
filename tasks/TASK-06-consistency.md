# TASK-06 重派与崩溃恢复的一致性 + 收尾

依据 `docs/PRODUCT.md`「未解的设计问题 → 重派与崩溃恢复的一致性」的已定口径与 `tasks/fix-04-r5-backlog.md` 第 10、12–14、16–20 条。两块并行、各自工作树；契约先定在本文件，两侧按契约各做各的，合并时对账。

## 契约（两侧共同遵守，只追加）

- Tauri 命令 `reconcile_dispatch_state(project_dir: String) -> ReconcileSummary`：
  `{ exists: bool, run_id: string, nodes: [{ node_id, verdict: "completed" | "incomplete" | "unknown", record_path: string, reply_text: string, artifacts: ReplyArtifacts | null, error: string }] }`（camelCase 镜像：`runId`、`nodeId`、`recordPath`、`replyText`）。只读，不改任何文件。
- Tauri 命令 `rollback_node_changes(project_dir: String, run_id: String, node_id: String) -> RollbackSummary`：`{ restored: string[], removed: string[], skipped: string[], note: string }`。按 `.dispatch/<run_id>/<node_id>.before/` 恢复；没有快照目录时返回空并在 `note` 说明，不报错。
- 留痕 `aicross-dispatch/1` frontmatter 追加 `before_dir: string`（相对项目目录，空串 = 无）。
- `Reply` 追加可选 `fallback_from?: string`、`fallback_to?: string`（TS）/ `#[serde(default)] Option<String>`（Rust 镜像）；正文首行「（已切备选：…）」标记不再写入，读旧图时仍识别。
- `DispatchStageRequest.nodes[]` 追加可选 `rollback_before: bool`（camelCase `rollbackBefore`），为真时引擎在派发该节点前先回滚上次改动。

## A. 引擎（Rust）— codex

1. **before 快照落盘 + 回滚**（backlog 12）：`run_one_attempt` 已有 before/after 快照；把 after 与 before 相比被改动/删除的文件，从 before 快照另存到 `.dispatch/<run_id>/<node_id>.before/<相对路径>`（新建文件记进 `<node_id>.before/.created` 列表），留痕 frontmatter 记 `before_dir`。实现 `rollback_node_changes`：恢复 before 文件、删除 `.created` 里的文件、超限（>8MB 未读内容）的文件进 `skipped`。`DispatchNodeRequest.rollback_before` 为真时在写锁内先回滚再派发，结果 `runner_note` 记「已回滚上次改动 N 文件」。worktree 节点跳过（本来每次从 HEAD 建）。补测试：单写者节点改文件 → 回滚 → 文件内容与新建文件都复原；`rollback_before` 走 `run_one` 端到端。
2. **对账**（backlog 13）：实现 `reconcile_dispatch_state`：读 `STATE.md` 头的 `run_id` 与 `active_node_ids`，扫 `.dispatch/*.md` 找 `run_id`/`node_id` 匹配的留痕（frontmatter 已有这两个字段）：`status: ok` → completed（`reply_text` 取留痕「## 原始输出」之前的回复正文段——按现有留痕结构取，产物按留痕已记的 diff/changed 组装成 `ReplyArtifacts`，脱敏已在写留痕时做过）；其它 status → incomplete；无留痕 → unknown。补测试：三种 verdict 各一条 + 留痕在但 run_id 不同不算。
3. **禁外部动作**（backlog 14）：`fixtures/prompts/execution.txt` 加一行「不得 git push、发布、安装依赖或调用付费接口；这些留给用户在验收之后做。」；claude 通道的写节点 `--tools` 白名单排除 `WebFetch`/`WebSearch`（只读节点已排除），补测试断言 argv。
4. **推进幂等测试**（backlog 10）：Rust 侧 `dispatch_stage_core` 同一 `run_id` + 同一节点集合重复调用时第二次拒绝（返回「该 run 已在进行」）并补测试；前端幂等由 B 侧做。
5. **8MB 标记边界**（backlog 20）：读线程预留失败时把标记行本身的字节算进上限（预留 `MAX + marker.len()`）或在消费端补发一次标记；补测试。
6. **Reply 镜像**：`orchestration.rs` `Reply` 追加 `fallback_from`/`fallback_to`；`run_fallback` 的结果不再在正文前面塞标记行，改填这两个字段（`DispatchNodeResult.fallbackFrom/To` 已有）。既有测试同步。

验证：`cargo check` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。不跑 `cargo fmt`，保持文件原有换行（`server.rs` CRLF、其余 Rust LF）。不动 `src/channels/acp.rs`。不起窗口，不发真机请求，不 commit，不 push。报告写 `tasks/REPORT-06A.md`。

## B. 界面（TS）— kimi

1. **崩溃后「继续」走对账**（backlog 13）：`GraphTab.tsx` 的「继续」先调 `reconcileDispatchState`（`tauri.ts` 按契约加命令与类型，未接通时按 `exists:false` 回退到现在的行为）：completed → 把 `reply_text`/`artifacts` 写回节点 Reply（status done）；incomplete → draft；unknown → Reply 加 `status: 'draft'` 且 `text: '上次派发结果未知，请检查项目目录后重派或放弃'`，并在 G1 确认屏对该节点显示「待核对」标签、默认不勾选派发。纯函数 `applyReconcile(graph, summary)` + 测试三种 verdict。
2. **单写者重派回滚**（backlog 12）：结果屏与节点卡的「重派」「升级」对 `write != 只读` 且非 worktree 的节点，在 G1 显示「将先回滚上次改动」并在请求里带 `rollbackBefore: true`（`tauri.ts` 追加字段）；G1 加一行项目目录未提交改动提示（调用现有 git 状态命令若有，没有就按 B 侧新增只读命令 `git_status_dirty(project_dir) -> bool` 的契约留接口，未接通时不显示）。测试：`buildRequest` 对单写者重派带 `rollbackBefore`，对只读/worktree 不带。
3. **Reply.fallback 字段**（backlog 17）：`types.ts` `Reply` 追加 `fallback_from?`/`fallback_to?`；`applyDispatchResults` 从结果 `fallbackFrom/To` 写入字段、不再前置标记行；`fallbackNoticeOf` 优先读字段、退而识别旧标记（兼容旧图）；节点卡与结果屏照旧显示整行。测试更新。
4. **G1 备选下拉溢出**（backlog 16）：确认屏目标行改成可折行（`flex-wrap` 或两行），窗口 1568 宽下四个控件不溢出卡片；快照/字符串断言即可。
5. **前提附件输入**（backlog 19）：手输改成每行一条（textarea 按行拆）或逐条添加，不再按逗号拆；「抽查」前再跑一次 `validatePremiseAttachments`，不通过就提示不建图。测试两条。
6. **盲父节点不传 session/runner**（backlog 18）：`continuationSession`/`continuationRunner` 在父节点 `visibility === '盲'` 时也返回空；测试一条。
7. **推进幂等**（backlog 10，前端半边）：`start()` 对同一 `run_id` 的重复回调只推进一次（记已处理的结果批次 id）；纯函数测试。

验证同 A（vitest 用 `--configLoader native --pool threads`）。不动 `src/`（Rust 由 A 做；`tauri.ts` 的命令签名按契约先写，后端未接通时前端要能编译并按回退分支工作）。不起窗口，不 commit，不 push。报告写 `tasks/REPORT-06B.md`。
