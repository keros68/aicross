# TASK-13 红绿核对（自动、不阻断）（09-15 定案）

依据 `docs/PRODUCT.md`「第 4 期草案」第 3 条与用户裁决：自动做，不阻断，结果标注；修正轮里「未钉住」按未修好计入上限。只动 `src/dispatch.rs`（含 tests）与结果卡显示；契约只追加。

## 契约（只追加）

- `BackendNodeResult` 追加 `pinned?: 'pinned' | 'unpinned' | 'skipped'` 与 `pinned_note?: string`（Rust 同名，serde default）。
- 触发条件：节点是执行角色、写权限为单写者或 worktree、验证命令非空、本次派发是修正轮（前端在 `DispatchNodeRequest` 追加 `fix_round: true`，由 `reducePipelineStep` 生成修正节点时置位；普通执行节点不做）。
- 派发记录 frontmatter 追加 `pinned`。

## 实现

1. 修正轮节点跑完且验证命令通过后，引擎做红绿核对：用派发前快照（`before_dir`，TASK-06 已有）把本次改动**临时回退**（只回退本次 diff 涉及的文件，worktree 节点在 worktree 内做），跑验证命令，期望失败；再恢复本次改动，跑验证命令，期望通过。两步都对 → `pinned: 'pinned'`；回退后仍通过 → `pinned: 'unpinned'`，note「回退修复后测试仍通过：新增测试没有钉住这个修复」；恢复后失败 → 按 `validation_failed` 处理并 note「红绿核对恢复后验证未通过」。任何一步命令超时 / 快照缺失 → `pinned: 'skipped'` + 原因，不影响 status。
2. 回退与恢复都不得碰本次 diff 以外的文件；核对期间持该节点的写锁；核对结束后项目目录字节级等于核对前。
3. 前端：结果卡在状态旁标「测试已钉住 / 测试未钉住 / 未核对」，`unpinned` 用警告色并显示 note；`reducePipelineStep` 把 `unpinned` 视同 `validation_failed` 计入修正轮上限（TASK-12 的 `fixRoundLimit`），修正节点提示词末尾追加「上一轮修复回退后测试仍通过，请补能失败的测试」。
4. 测试：真建临时 repo，用一个「改函数 + 加测试」的修复和一个「只改函数不加测试」的修复各跑一次，断言 pinned / unpinned；快照缺失 → skipped；核对后目录与核对前逐字节相等；超时注入 → skipped。

验证同 TASK-12。不 `cargo fmt`；`src/dispatch.rs` 保持 LF；不 commit、不 push。报告写 `tasks/REPORT-13.md`。
