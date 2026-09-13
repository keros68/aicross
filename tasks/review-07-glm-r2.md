复审完毕。已通读上一轮审查、修正单、REPORT 修正轮 R1 一节与全量 diff（约 4800 行），并打开工作树源文件做了抽查。逐条结论如下。

## 修正单 14 条逐项核对

| # | 结论 | 依据（文件：行号 + 理由） |
|---|------|------|
| 1 | **已核实修好** | `src/resources.rs` `detect_pi`：`--list-models` 空/失败即报错（主事实源），models-store 读不到→`unwrap_or_else(\|_\| "{}")`，`parse_pi_sources` 里坏 store `unwrap_or_default()` 按空处理不拦截（补丁 2836–2901）；测试 `pi_list_models_is_primary_store_only_supplies_metadata`（补丁 4145–4173）断言「store 没有的模型收入、store 有的补元数据、`"{ 损坏"` 不拦截」三点。真机 31 个模型与编排者复跑一致。 |
| 2 | **已核实修好** | `GraphTab.tsx:65-85`、`OrchestrationPanel.tsx:83-103`：均空依赖、只响应事件（挂载加载仍是原有 effect，`GraphTab.tsx:88-110`，不重复请求）、cleanup 里 removeEventListener + `dead` 标志防卸载后 setState，无泄漏。`notifyResourcesChanged` 只在命令完成路径派发、`listDispatchableModels` 不回派事件，无循环。`GraphTab.tsx:53-61` 的 insert-mention 监听同样纯追加。 |
| 3 | **已核实修好** | 8 个写命令全部持 `store_lock()`（补丁 3561/3582/3610/3622/3629/3636 及 `smoke_agent_models_at`、`add_manual_agent` 内联）；`smoke_agent_models_at`（补丁 3646–3699）快照阶段独立作用域持锁，逐条循环内持锁→load→冒烟→apply→**save**，无嵌套获取（std Mutex 不可重入，不会自锁）。测试 `smoke_agent_models_partial_failure_writes_each_result`（补丁 4240–4279）断言 errors 恰 1 条、两条状态、**磁盘上**失败条已落盘。观察（非缺陷）：锁在一次冒烟子进程期间（≤60s）被持有，批量冒烟会阻塞其他写命令——保守但正确，逐条重读已消除旧 store 覆盖。 |
| 4 | **已核实修好** | `ResourcesOverlay.tsx` `finishOperation`（补丁 446–455）：`length > 0` 才 setHints，smoke 返回空 hints 不再清条。代价是 hint 只在下次探测时刷新（可能短暂过期），修正单明示接受此取舍。 |
| 5 | **已核实修好** | `claude_third_party_endpoint()`（补丁 3260–3282）：env → settings.json `env.ANTHROPIC_BASE_URL`；文件读不到/坏 JSON 走 `.ok()` 链判为官方端点，成功时 meta 记「未核对真身（官方端点）」，不再静默。测试覆盖解析分支含坏 JSON。边缘情形：settings.json 存在但临时读失败会误判官方端点——只影响一个 meta 标记，方向可接受。 |
| 6 | **已核实修好** | 导语行「全部探测」按钮（补丁 587–594）接 `detectAllResources`、busy 禁用；`fixtures/resources/` 现存 10 个文件、全仓无 `pi_smoke` 代码/fixture 引用（仅 tasks 文档提及）；REPORT 第 17 行已改口。 |
| 7 | **已核实修好** | 三测试均真断言：`declared_models_are_not_marked_missing`（补丁 4176–4213，正反对照）；`corrupted_store_blocks_writes_and_keeps_bytes`（4216–4237，`fs::read` 前后字节相等）；批量冒烟测试见第 3 条。无空断言。 |
| 8 | **已核实修好** | `REPORT-07.md:30`「9 个用例（18 个断言）」；`:119` 硬编码中文已入「没做什么」；`:17` smoke 解析测试口径已改。 |
| 9 | **已核实修好** | `add_manual_agent` 返回 `ResourceOperation`（补丁 3747–3787），errors 随结果回传；前端 `ManualAgentDialog.onSaved` 收 operation 并经 `operationErrors` 显示（补丁 648–658、954–960）。 |
| 10 | **已核实修好** | `resolve_cli`（补丁 2514–2521）注释完整记下 kimi `.cmd` 漂移与收敛计划；解析顺序先 `~/.kimi-code/bin/kimi.exe` 再 PATH，与 channels fallback 同路径。 |
| 11 | **已核实修好** | `detect_into`：claude 未检测 → `merge_detected_models(store, "claude", vec![])` 后 return（补丁 2613–2620），与 codex/kimi 同一合并路径刷 missing，不加预设。 |
| 12 | **已核实修好** | `Explorer.tsx:747`、`TitleBar.tsx:138-139` 无 `as any`（`I18nKey` 源自 en.ts，key 已加，类型合法；剩余 `as any` 均为 menu.*/workspace/history 期前遗产）；`resources.css:1532-1545` 底色改 `color-mix(... var(--green/--red) ...)`。 |
| 13 | **已核实修好** | `orchestration.rs` 仅加两行注释（补丁 2197–2198），resources.rs `write_json_atomic` doc 互指并注明第 5 期收敛（补丁 2415–2418）。 |
| 14 | **已核实修好** | `ResourcesTab.tsx:27` 用 `session.tool === 'graph'` 判定（与 `CenterPanel.tsx:1483` 判别式一致）；模型行 `disabled` + title「先打开对话图」（`ResourcesTab.tsx:160-166`）；`resources.css:117,122` 有 `:disabled` 样式。 |

**14/14 全部「已核实修好」，无「未修」、无「改法有问题」。**

## 不采纳两条

- **移除手动 agent 途径**：理由成立——任务书范围外，Later 记录明确，manual-* 只增不减是已知且已披露的代价。
- **三处中文状态串并存**：理由成立——第 1 期契约，本期改契约的影响面大于收益。

## 回归检查

- 禁区确认：`src-ui/src/graph/`、`src/channels/`、`src/dispatch.rs` 不在 diff 中；`components/right/orchestration/` 仅 OrchestrationPanel 追加一个监听；`orchestration.rs` 仅注释。第 2/3 期图与派发逻辑零触碰（新监听只 set models state）。
- `server.rs` 只在 `generate_handler!` 末尾追加 9 行；`tauri.ts`/`Explorer`/`TitleBar` 均追加型改动，合并冲突面小且 REPORT 已如实列出。
- 一个非缺陷的行为变化：左栏无存值默认 tab 从 workspace 改为 `resources`（`Explorer.tsx:633-641`），属本期产品意图且 REPORT 已披露。

## 最终结论

**可以并回 main**——14 条修正全部核实落地、三个重点复核项（pi 主源/锁覆盖/测试真断言）经代码与测试双重确认、禁区未破、回归面干净，136+75 测试与真机 31 模型由编排者独立复跑吻合。

仍未闭合的项（均不阻塞合并）：
1. 三个界面行为（探测后右栏刷新、hints 保留、无图置灰提示）未起窗口真验——REPORT 自己也留给编排者窗口复验；
2. 新界面硬编码中文未进 11 语言字典（已披露，第 5 期统一）；
3. `write_json_atomic` 双份重复（第 5 期收敛）；
4. 无移除手动 agent 途径；
5. `resolve_cli` 与 channels `resolve_binary` 重复及 kimi `.cmd` 漂移的最终收敛（channels 解禁后）；
6. 小疵两处：`smoke_agent_models_at` keys 为空时返回 `ResourceStore::default()`（当前前端不可达，防御性问题）；批量冒烟期间写锁被单个子进程长持（正确但偏粗）。