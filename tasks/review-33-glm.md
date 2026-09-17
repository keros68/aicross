# TASK-33 首版审查 · GLM-5.3-Flash（cc-switch，只读）

## 总判

**可交付**——七条任务书逐条落实且实现正确，未发现阻塞性或引入性缺陷；有一条低严重度视觉项需实测确认（第 1 条），另有两条边缘备注。

## 问题清单（按严重度）

1. **低（不确定，需实测）— 940px 中栏下表格余量可能差约 2px，出现微小表内横向滚动**
   - 位置：`src-ui/src/components/center/ConsoleChat.css:232-240`（`.console-run-body` padding 20px）、`:281-288`（stage 边框 1px×2）、`:307-313`（`min-width: 900px`）。
   - 问题：中栏 940px − body 左右 padding 40px − stage 边框 2px ≈ 898px 可用宽 < 表格 min-width 900px。若编排者实测的「约 940px」指中栏总宽，则 `.console-run-table-wrap`（overflow-x: auto）会出一条约 2px 的横向滚动；十列仍全部可见，仅多一个滚动条。940 是任务书的约数，我无法起窗口实测，**不确定是否实际触发**。
   - 改法：列宽合计压到 ≤896，或 `.console-run-body` 的左右 padding 收窄（如 16px），或 min-width 降为 896。

2. **低（边缘，非本次引入）— 正在查看的 run 被 20 份上限裁剪时详情页静默退回**
   - 位置：`src-ui/src/components/center/ConsoleChat.tsx:56`（`openRun = runsById.get(openRunId)`）、`src-ui/src/store/app-state.tsx:591`（`slice(-CONSOLE_RUN_LIMIT)`）。
   - 问题：openRunId 指向的记录被新 run 的 upsert 挤出 20 份窗口时，`openRun` 变 undefined，组件直接退回聊天视图——不走 `closeRunDetail`，焦点不恢复，`scrollSnapshotRef` 滞留（下次打开详情再关闭时才被消费）。触发条件苛刻（会话已有 20 份且正查看最老一份时新 run 开始），且 openRunId 机制是 TASK-27/29 既有，本次未恶化。滚动标记不会因此卡住（skip 未置，自动滚底正常）。
   - 改法：后续 backlog——裁剪时排除 `openRunId` 正在查看的记录，或裁剪命中时主动走一次关闭恢复。

3. **信息 — backlog 91/第 6 条的「一格测试」是静态源码断言，验证最弱**
   - 位置：`src-ui/src/__tests__/console-polish-33.test.ts:296-300`（断言源码含 `const displayed = displayResults(collected)` 等字符串）、`:286-294` 后半（断言源码含 `skipAutoScrollRef.current = true`）。
   - 问题：这类断言对代码重排脆弱、对「换算结果传错变量」类逻辑错误不敏感。任务书对第 7 条只要求「补一格断言」，算达成；且行为面有间接覆盖——`console-per-apply-29.test.ts:152` 断言真实链路产出的 rows stage 为 `[1,2,3,4]`（换算后），displayResults 接线错了它会红。滚动跳过的纯函数与组件接线分开测，组件侧无行为测试（无 jsdom 的既定取舍）。

4. **信息 — 报告中的命令级验证本次未复跑**
   - cargo check/test、vitest 666 过、build、copy-audit、换行检查均出自 `tasks/REPORT-33.md`，我只读环境无 shell，无法独立确认。已静态核实各测试与实现的咬合及红测机理（见下），报告中红测描述与代码机理一致。

## 核过没问题的要点

- **(1) 验收链末次判定**：`console-run-detail.ts:273-320`。`hasAncestor` 防环正确；`last` 取「无更晚验收后代」的行，正确处理：不等深两分支（浅 NEEDS_FIX → 未通过，polish-33 测试 2 走真实 `finishConsoleRun`）、并行分支互不为祖先各自保留、修正轮接续（A1→E1→A2 链上 A1 被排除）、预设自带串联验收只取末次（测试 3，旧逻辑按 stage 会误写「第 2 轮」）。轮数只数 `fixRoundNodeIds` 内 role=执行且父为验收的行：修正轮执行节点父=验收计 1；`appendValidationRetry` 的重试执行节点父=执行不计；失败/超时的修正节点也计；round=0 不写「修正 N 轮」。`console-per-apply-29.test.ts:149` 真实链路断言「修正 1 轮后验收仍要求修正」。
- **(2) 焦点**：打开即 focus「返回对话」（`ConsoleRunDetail.tsx:77-79`）；Tab/Shift+Tab 在页内循环、焦点在页外时拉回页内（`runDetailTabTarget`，document 捕获级监听）；页内无可聚焦元素时返回 null 不抢 Tab（防御性，实际「返回对话」恒在）；关闭后打开按钮的 DOM 已随聊天子树重挂必然 `isConnected=false` → 回输入框（`ConsoleRunFocusTarget` 接线在 `ConsoleChat.tsx:96-102`）。抽纯函数、未引 jsdom。
- **(3) pending→running 与持久化**：阶段开始建 pending 行（`withConsoleRunStage:195`，保守保留已 running 行的 progressAt）；首条心跳才转 running 并以引擎 `elapsedMs` 起表（`withConsoleRunProgress:209-217`；`console-run-detail-27.test.ts:338-351` 验证 3000+1200=4200）；详情页 pending 行显示「排队中」、耗时「—」（`ConsoleRunDetail.tsx:59,66`）。旧记录兼容：`rowFrom` 字段缺失 finite 兜底，加载时 `interruptConsoleRun` 把 running/pending 冻结为 interrupted，旧 running 行 progressAt=0 时退回 durationMs 不崩（27 测试 299-311 覆盖带/不带心跳字段两种）。
- **(4) 滚动标记不卡住**：`skipAutoScrollRef` 仅在关闭恢复的 layout effect 置 true；同一提交内 layout effect 先于 effect 执行，自动滚底 effect 读到 true 即清并跳过一次；`openRunId` 在其依赖里，关闭时 effect 必跑，标记必然被消费。打开详情页期间新消息到达时 effect 提前 return，不动标记。
- **(5) 表格与紧凑数字**：`RUN_TABLE_COLUMN_WIDTHS` 合计恰 900；`compactRunNumber` ≥1 万紧凑、≥100 万 m；行、阶段小计、顶部合计三处 token 均带 `title` 全数；状态列最长文案「验证命令未通过」在 108px 内单行，模型列允许换行。
- **(6) 旧画布无影响**：`startPipelineHeadless` 返回新增 `fixRoundNodeIds` 字段；`DispatchOverlay.tsx:416-427` 只按名读 `outcome/error/leftoverIds/results/graph`，不展开返回对象；且旧画布不传 `fixRoundNodeIds` 时 `reducePipelineStep` scope=undefined，保持原行为。
- **(7) 测试真实性与红测机理**：测试 2/3/4/7 调真实导出函数；测试 5（`createConsoleApplyHandler`）、测试 8（`runConsoleDispatch`）为完整真实接线（含 `startPipelineHeadless`→`reducePipelineStep`→`appendCorrectionRound`→`finishConsoleRun` 全链）。任务书要求的四类场景 + 真实接线格齐。测试 8 的红测机理核实成立：第二次事件的材料挂在第一次的验收节点（latestLeaf）下，去掉 `fixRoundNodeIds: new Set(created)`（`console-channel.ts:444`）则 scope 变 undefined，祖先链数出 2 个验收 > 上限 → 走 leftover 不修正，`['执行','验收','执行','验收']` 断言即红。
- 范围合规：改动仅 `src-ui/**` + 测试 + 报告，未碰根目录 `src/**`；未引 jsdom/新依赖（基于文件内容与 git status，diff 级夹带检查因无 shell 未能逐行比对 HEAD）。
