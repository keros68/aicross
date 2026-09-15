# REPORT-12-R1 TASK-12 复审修正轮

分支 `phase/12`，未 commit、未 push；未启动窗口，未发送真机请求。12 条必修均已完成，没有未做或部分完成项。

## 逐条修正

1. **混合批次保留验收修正轮**：`reducePipelineStep` 先处理状态为 `ok` 且正文为 `NEEDS_FIX` 的验收结果；同批验证路的 `error` / `timeout` 仍留在该路，但不再吞掉已生成的修正轮。测试：`验收 NEEDS_FIX 与验证 error 同批返回时仍生成修正轮`。
2. **未发送不附验证材料**：新增 `OutboundPreparation::Materialized / Unsent`；首次验证失败或取消产生 `Unsent`，直接跳过验证命令和 `VALIDATION.txt`，`validation_attached=false`。测试：`task12_unsent_does_not_run_validation_or_write_blind_material`，用 marker 文件断言命令没有执行。
3. **遗留节点不再成为死路**：`latestLeaf` 跳过遗留节点；列末端与发送入口通过 `continuationNodeId` 回退到来源验收；遗留卡隐藏「分叉 / 盖预设」，提供「删除」，`removeDraftNode` 接受无子节点的遗留节点。测试：`遗留焦点回退到来源验收，新节点不会挂在遗留下；遗留可删除`。
4. **验证输出直接进入盲 prompt**：保留 `VALIDATION.txt`，同时把脱敏、8 KB 截断后的完整正文内联到 prompt，不依赖 claude / pi 的文件读取工具。测试：`task12_validation_txt_materialized_for_exit_zero_command` 同时断言 prompt 含输出片段。
5. **验收职责文案与引擎一致**：`src/orchestration.rs` 注释、讲解页和手册统一为「验收盲、只读，看产物给 PASS / NEEDS_FIX」；验证命令由执行节点在派盲审前运行。测试：`手册与讲解页说明验收只读判定、验证输出来源及角色默认`。
6. **同批验证命令只跑一次**：新增独立缓存函数，以 `run_id + canonical project path` 为键，用 `OnceLock` 保证并发节点只执行一次并复用正文；阶段收尾清缓存。测试：`task12_same_run_and_project_executes_review_validation_once`，两路盲验证通过 marker 行数断言只执行一次。
7. **run_one 集成测试**：复用 `cc-switch` 盲节点夹具，走完整 `run_one`，核对 `runs/node-1/VALIDATION.txt`、frontmatter 的 `validation_attached: true`，并用测试注入记录验证命令超时为节点 10 秒的一半（5 秒）。测试：`task12_run_one_materializes_validation_and_uses_half_node_timeout`。
8. **角色默认和跳过名单说明**：手册写明盲审模式三路均须在确认派发页选目标；手册与讲解页写明验证角色空目标除盲审 / 评审模式外沿用验收默认。测试：`角色默认：验证沿用验收的默认目标；盲审 / 评审预设整体不填`、`手册与讲解页说明验收只读判定、验证输出来源及角色默认`。
9. **设置页补齐上限后果**：「修正轮上限」说明明确验证命令未通过的重派与修正轮共用上限，验证未通过到顶时验收标「已跳过」。测试：`设置页完整说明共用上限与到顶已跳过`。
10. **关闭验证兄弟**：`appendValidationRetry` 与 `closeWaitingAcceptance` 都把同父、仍为 draft 的「验证」兄弟标为 `skipped_reason: 上游验证未通过`；状态为「已跳过」，不在可派集合。测试：`验证命令未通过时同父验证兄弟一并已跳过，不再可派`，覆盖重派与到顶两条路径。
11. **遗留去重且结果页只看本轮**：`appendLeftover` 按来源验收更新既有节点；`PipelineStep / StageLoopState` 传出本轮触及的遗留 ID，结果屏只按这组 ID 渲染。测试：`同一验收重复到顶只更新一张遗留卡，并随步状态传出同一个 id`。
12. **清空恢复默认 1**：新增 `fixRoundLimitFromInput`；空字符串或空白回默认 1，显式 `0` 保持 0。测试：`修正轮上限输入清空时回到默认 1，显式 0 保持 0`。

另按「保持」项把预设形状测试改为先解析 `Preset` 块、再按 `id` 查找，不依赖声明顺序。

## 验证

- `cargo check --all-targets`：`Finished 'dev' profile [unoptimized + debuginfo] target(s) in 18.14s`，零 warning。
- `cargo test`：`test result: ok. 364 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out`；忽略项均为仓库既有真机 / 真模型测试。
- `cd src-ui && npx vitest run --configLoader native --pool threads`：`Test Files 30 passed (30)`；`Tests 458 passed (458)`。
- `cd src-ui && npm run build`：`✓ 334 modules transformed.`；`✓ built in 650ms`。
- `git diff --check`：通过，无空白错误。

## 边界与文件约束

- 未运行 `cargo fmt`，未改 `src/terminal.rs`、`src/server.rs` 或 `src-ui/src/tauri.ts`。
- `src/dispatch.rs` 的产品代码改动收敛在阶段结束清缓存、`run_one_attempt` 的准备状态接线，以及新建的验证缓存 / 附加函数；未改 `run_one_with_activity` 的备选循环与 artifacts 逻辑。
- 未新增契约字段；TASK-12 既有契约保持只追加。
