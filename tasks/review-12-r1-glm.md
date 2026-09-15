**总判：可交付但有未闭合项** —— 12 条必修全部落实且各有能红的测试；未闭合的只剩文档/显示级残留（MANUAL 重复标题、结果屏「已跳过」跨 run 残留等），无阻断级行为问题。说明：本会话没有 shell 工具，未实际重跑 cargo/vitest/build，REPORT-12-R1 的运行结果按报告采信，以下为对工作区现状的静态审查。

## 12 条逐条

1. **已修**。`reducePipelineStep` 验收 NEEDS_FIX 分支前置，失败停止移到其后：`src-ui/src/graph/dispatch.ts:781-803`（验收循环 + `addedCorrection` 早退）、`dispatch.ts:830`（`some(!== 'ok')` 停止）。混合批次测试 `review-defaults-12.test.ts:164-170` 断言 `continueRunning=true` 且新增 执行/验收/验证 三节点——旧实现在 830 行先停，该测试红。「验证路失败可重派」：error/timeout 经 `applyDispatchResults` 写成 `failed`+`error_kind`（`dispatch.ts:262-269`），节点卡对 `failed` 显示「重派」（`GraphTab.tsx:584-588`）。
2. **已修**。`OutboundPreparation::Materialized / Unsent`（`src/dispatch.rs:3362-3366`）；`run_one_attempt` 只在 `verified` 时物化（2987-2992）；attach 对 Unsent 直接返回 `(Ok(Unsent), false)`，不跑命令、不写文件（3489-3496）；三态解包 3003-3007。测试 `task12_unsent_does_not_run_validation_or_write_blind_material`（7652-7677）用 marker 文件断言命令未执行——旧哨兵 `Ok("(未发送)")` 行为下命令会执行，红。
3. **已修**。`latestLeaf` 跳过遗留（`layout.ts:68-83`）；`continuationNodeId` 回退来源验收（`layout.ts:86-89`），列维护与发送都走它（`GraphTab.tsx:228-229、386`）；「分叉 / 盖预设」对遗留隐藏（`GraphTab.tsx:594-617`，`!node.leftover`）；「删除」对无子遗留开放（`GraphTab.tsx:589-593`；`mutate.ts:101-108` 守卫有子不可删）；NodePanel 遗留走只读清单（`NodePanel.tsx:379-380`）。测试 `review-defaults-12.test.ts:172-183`——旧 `latestLeaf` 返回遗留 id、旧 `removeDraftNode` 拒收遗留，两处断言均红。
4. **已修**。同一份（safe_trace 脱敏 + 8KB 字符边界截断）正文既写 `VALIDATION.txt` 又内联进盲 prompt：`dispatch.rs:3497-3508`。测试断言 prompt 含「验证命令输出：\n」与输出片段（7590-7591）——旧实现只追加指路句，红。
5. **已修**。`src/orchestration.rs:396-397、418-419` 注释、`ExplainOverlay.tsx:43、46`、`docs/MANUAL.md:66-73、81` 统一为「验收盲、只读，看产物给 PASS / NEEDS_FIX；派盲审前由执行节点跑一次验证命令」。测试 `review-defaults-12.test.ts:364-371`——旧文案没有这三句关键表述，红。全库已无「验收跑验证命令」残留（仅审查文件引用）。
6. **已修**。缓存以 `run_id + canonicalize(project)` 为键（`dispatch.rs:3375-3383`），`Arc<OnceLock>` + `get_or_init` 保证并发只执行一次（3452-3475），阶段收尾清缓存（1880），超时取节点超时一半（3470）。测试 `task12_same_run_and_project_executes_review_validation_once`（7679-7709）以 marker 行数断言只跑一次——旧实现逐节点执行得 2 行，红。
7. **已修**。`task12_run_one_materializes_validation_and_uses_half_node_timeout`（`dispatch.rs:7711-7752`）：走完整 `run_one`，断言 `runs/node-1/VALIDATION.txt` 存在、frontmatter `validation_attached: true`、注入 hook 记录 `(项目目录, 5s)` 即 10s 的一半。删掉 2994-2995 的调用点即红。
8. **已修**。跳过名单两侧一致（`dispatch.ts:76` / `orchestration.rs:610`）；「验证」沿用验收默认（`dispatch.ts:67` / `orchestration.rs:629`）；手册写明盲审模式三路须在确认派发页选目标、验证空目标沿用验收默认（`MANUAL.md:72、81`；`ExplainOverlay.tsx:46`）。测试 349-371。
9. **已修**。`SettingsModal.tsx:654` 补「验证命令未通过的重派与修正轮共用此上限；验证未通过到顶时该验收标『已跳过』」。测试 373-376。
10. **已修**。`appendValidationRetry`（`dispatch.ts:640-651`）与 `closeWaitingAcceptance`（683-694）都把同父仍 draft 的「验证」兄弟标 `skipped_reason: 上游验证未通过`；`nodeStatusLabel` 显示「已跳过」（361）；结果屏列出（`DispatchOverlay.tsx:469、696`）。测试 `review-defaults-12.test.ts:208-221` 覆盖重派与到顶两条路径——旧实现无此标记，红。
11. **已修**。`appendLeftover` 按 `source_node_id` 去重更新（`dispatch.ts:751-761`）；`leftoverIds` 随步状态传出（787-794、802/828/831/834/836 各早退路径都带）；`runStageLoop` 累积去重（1388-1390）；结果屏只画本轮（`DispatchOverlay.tsx:122、284、319、330、471-472、750`）。测试 223-231——旧实现会挂第二个遗留节点，红。
12. **已修**。`fixRoundLimitFromInput`（`dispatch-settings.ts:26-32`：空/非数字回 1，显式 0 保持 0，夹 0–9），接线 `SettingsModal.tsx:652`。测试 `dispatch-settings.test.ts:41-45`。

## 新发现的问题（按严重度）

- **低（文档）`docs/MANUAL.md:59-60`**：连续两行标题「## 三个内置预设」「## 内置预设」，前者为残留（内置预设已有六个）。建议删「## 三个内置预设」一行。
- **低（显示）`DispatchOverlay.tsx:469、696`**：结果屏「已跳过」行取全图所有带 `skipped_reason` 的节点，不按本轮过滤。复现：第一次 run 验证到顶标跳过 → 关闭后再对其它节点派发 → 新 run 结果屏再次列出上次的已跳过行。与第 11 条修掉的遗留卡跨 run 问题同型，仅显示层。建议同 `leftoverIds` 一样随步状态收集本轮 id。
- **低（边界）`dispatch.ts:801-803` vs `804-829`**：同批既含验收 NEEDS_FIX（生成修正轮）又含执行 `validation_failed`（跨分支）时，`addedCorrection` 早退跳过验证失败处理——该执行节点的等待验收保持 draft「待派」且父 failed 永不可派，重试计数也不记；只能手动「重派」该执行节点恢复（节点卡有出口，非死路）。内置单链预设到不了这个形状，多分支图可到。
- **观察（行为不对称）**：`appendCorrectionRound` 为修正轮重生验证兄弟（`dispatch.ts:595-605`），`appendValidationRetry` 的重派轮只长执行+验收、不重生验证兄弟（646-675）→ 重派轮的产物没有盲验证、没有 VALIDATION.txt。首版既有行为，两轮审查与报告均未记录此取舍；若属有意建议在手册或报告补一句。
- **观察（缓存键边界）`dispatch.rs:3375-3377、3410-3417、3471`**：缓存键只有 run_id+项目目录，命令实际跑在首个存在的上游 worktree；同 run 同项目但两个验证节点上游 worktree 不同时，后到者复用先到者在其目录跑的输出。内置预设同父同 worktree 不受影响；与「按 run_id+project 键」的裁决一致，仅记录。
- **观察（窄窗口竞态）`dispatch.rs:3396-3407、3044-3046`**：verify 通过后、验证命令执行期间收到叫停，命令不挂 cancel key 仍跑完，盲目录与 VALIDATION.txt 已写、`validation_attached: true`，随后节点标「派发已取消」。上一轮第 2 条只覆盖 verify 前取消，此残余窗口未变。
- **观察（资源）`run_one` 路径不清验证缓存**：仅 `dispatch_stage` 收尾清（1880），`run_one_with_activity`（2736-2784）无清理；条目按 run_id+项目留在进程全局表缓慢累积，行为正确性不受影响。
- **观察（措辞）`dispatch.rs:3502-3507`**：prompt 指路句「验证命令输出见 VALIDATION.txt」后紧跟内联全文，两句并存略冗余；模型可见文本非界面文案，无害。
- **低（术语表）**：「已跳过 / 上游验证未通过」随第 9、10 条进入设置页与结果屏，但 MANUAL 术语表（186-202）无对应行；AGENTS.md:52 要求改文案按表复核。

## 核过没问题的点

- 12 条新测试的「能否红」逐条核对成立（各条依据里已标回退点）；无同义反复。
- `latestLeaf` 空图/仅根回退 root；分叉、盖预设、列输入、双击 pin/align 后发送四条造子入口全部经 `continuationNodeId` 或被 `!node.leftover` 封住（`GraphTab.tsx:342-352、386、594-617`）。
- `dispatchableNodeIds` 排除遗留、强塞 draft+target 仍排除（`dispatch.ts:96`，测试 151-162）；`nodeStatusLabel` 对遗留返回 null。
- Unsent 路径零副作用（不碰命令与文件系统，`dispatch.rs:3489-3496`）；`Err(备料)` 同样不跑命令。
- 内联与文件同源：脱敏/截断一份正文两处用（3440-3450、3497-3508）。
- 缓存并发语义正确：`Arc<OnceLock>` + `get_or_init`，无重入；canonicalize 防 Windows 路径形变。
- `run_one` 集成测试同时钉住 cwd 与 timeout 两个注入参数（7750）；python 缺失跳过与既有夹具口径一致。
- 材料目录优先上游 worktree 有独立测试（7773-7787）。
- 角色默认前后端映射与跳过名单一致（`dispatch.ts:60-76` / `orchestration.rs:604-644`）。
- `StageLoopState.leftoverIds` 可选字段向后兼容；`start()` 与 catch 两路都取 `loop.leftoverIds` 且 start 时清空（`DispatchOverlay.tsx:284、319、330`）。
- 新增界面文案符合 AGENTS.md 规范：中性说明体、事实+后果、全角标点、无内部词（`SettingsModal.tsx:654`、`MANUAL.md:66-81`、`ExplainOverlay.tsx:43-46`、`NodePanel.tsx:63`）。
- 「保持」项落实：预设形状测试改为按 id 提取 Preset 块、不依赖声明顺序（`review-defaults-12.test.ts:291-301`）。
- 旧测试期望与新语义一致（R5-1/R5-2 保持、旧口径测试显式传 3）。