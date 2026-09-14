# TASK-07C 代码审查（phase/7c）

先给结论，再按维度逐条。

---

## 1. 需求完整性

**F1（必修，违反契约）`src-ui/src/graph/dispatch.ts:1061-1072`（`progressSnapshot`）+ `:1078-1086`（`nextProgress`）— 终态事件把上一条心跳读数整体冲掉，"终态冻结"在真实引擎下不成立。**

- 问题：`nextProgress` 是 `{ ...current, [nodeId]: progressSnapshot(payload, now) }`，`progressSnapshot` 把缺失字段补成 `elapsedMs: 0 / lastTool: ''`。契约 `tasks/TASK-07-idle-and-tail.md:7` 写明「终态事件不带心跳字段，前端保留上一条 `running` 的 `elapsedMs`/`lastTool`」，引擎侧也按此发（`tasks/REPORT-07A.md:93,107` 的 `r1_terminal_progress_after_heartbeat_stop_has_no_running_fields_or_late_emit`）。
- 为什么严重：节点完成的那一瞬间 `elapsedMs` 变 0，`runningDetailOf`（`dispatch.ts:1143`）返回空串、`idleHint` 又是 null → `NodeProgressRow` 整行消失（`DispatchBits.tsx:62`），运行屏那一行的「已运行 … · 工具 …」也一起消失。也就是说 **"冻结"实际是"清空"**，而且节点卡的读数是本期的新用户可见面。`REPORT-07C:108` 的「不清快照，`runningDetailOf` 自己冻结」只在终态事件恰好带字段的假前提上成立。
- 建议：`progressSnapshot` 增加一个可选的上一份快照参数（或在 `nextProgress` 里判断），当 payload 无心跳元数据（`elapsedMs === undefined && idleMs === undefined`）且同 `runId`/`nodeId` 有旧快照时，回填 `elapsedMs/idleMs/toolCalls/repeatedToolCalls/lastTool`（`receivedAt` 保留旧值即可，终态本来就不加墙上时间）；补一条只喂 `{ runId, nodeId, status: '完成' }` 的用例（现状会红）。

其余子项已核实：
- 节点卡与运行屏同源 ✓：唯一入口是 `dispatch-progress.ts:49-51` 的 `progressRunOf` / store 的 `snapshotOf`，两个消费者分别见 `GraphTab.tsx:570`、`DispatchOverlay.tsx:532`。
- 订阅只有一处 ✓：`'dispatch-progress'` 字面量只在 `dispatch-progress.ts:82`；overlay 与 GraphTab 都已无事件名、无 `@tauri-apps/api/event` import。
- 按 runId 取快照 ✓（`dispatch.ts:1090-1096`）；重派复位 ✓（`DispatchOverlay.tsx:218,278`）；换图复位 ✓（`dispatch-progress.ts:73-75`）；无心跳元数据不判空转 ✓（`dispatch.ts:1124` 的 `hasHeartbeat` 门槛）。

---

## 2. 逻辑正确性

**F2（应修）`src-ui/src/graph/dispatch-progress.ts:78-86` — 异步 `listen` 缺取消守卫，卸载/换 runId 可能泄漏监听。**

- 问题：`import(...).then(listen).then(stop => { unlisten = stop; })`；若 effect 在 promise resolve 之前被清理（组件卸载、runId 变更），cleanup 里 `unlisten` 还是 `undefined`，listener 照样装上且**永远不会被 unlisten**。`App.tsx:6 → lib/agent-status-bus.ts:23` 静态 import 了同一模块，所以实际窗口多为一个微任务，但模式本身不健全，且会随每次重派累计；仓库既有同型代码都带守卫（`components/left/Explorer.tsx:680-708` 的 `cancelled` / `if (cancelled) { handle(); return; }`，`lib/git-status.tsx:176-186` 的 `if (cancelled) fn(); else …`）。
- 建议：`let disposed = false; … .then(stop => { if (disposed) stop(); else unlisten = stop; })`，cleanup 里 `disposed = true; unlisten?.()`；`.catch(() => {})` 顺手改成 `console.warn`，否则订阅失败时界面完全没有读数也没有任何痕迹。

**F3（低-中）`dispatch-progress.ts:89-96` + `dispatch.ts:1105-1112` — 异常收尾后 stale `running` 让 5s 时钟永不停止。**

- 问题：`running` 只由最后一条快照推断。派发异常/命令 reject 的收尾路径（`DispatchOverlay.tsx:250-259` catch → `setPhase('results')`）不会给没有终态事件的节点清快照，于是 status 永远停在 `running`：时钟每 5s 重渲染整张图永不停、3 分钟后节点卡挂出「疑似空转」，而这一层已经判失败了。改动前 overlay 的时钟挂在 `phase === 'running'` 上，异常一收尾就停。
- 建议：给共享层一个明确的「本轮结束」信号（异常/收尾时 `resetRun`，或把时钟门控在显式 `runActive` 上）。顺带：`resetRun`（`:65,99,103`）导出了但全仓无调用点——是死接口，看起来正是为这个场景留的，要么用起来要么删掉。

已核实无问题：
- 迟到事件按 runId 丢弃 ✓：`applyProgressEvent` 原引用返回（`dispatch-progress.ts:39-47`），且监听里用函数式 `setRun(current => …)`，不存在闭包串轮次/串图。
- 5s 时钟只在有运行中节点时走 ✓（正常完成路径；`:90-95` 的 effect 依赖布尔 `running`，翻转即重建）。
- `NODE_H` 布局/坐标同步 ✓：`GraphTab.tsx:37` 一处 76，被分层坐标（`:412-418`）、画布高（`:813`）、连线端点（`:524-526`）、按钮行（`:573`）、卡片高（`:550`）共用，无第二处硬编码旧值。
- **F4（低，排版）`GraphTab.css:250-266` + `DispatchBits.tsx:60-69`**：最长提示「疑似空转：同一工具连续 3 次」约 160px（12 个 11px 全角字 + 数字 + 16px padding + 2px 边框），而卡片内容宽只有 168−24−2 = 142px；胶囊 `flex-shrink: 0`、不换行，尾部被 `.graph-node { overflow: hidden }`（`:201`）裁掉。`REPORT-07C:120-121` 说「胶囊完整显示，读数被省略号截断」不成立（此时读数被压到 0 宽，胶囊自己溢出）。建议缩短文案（如「空转 · 同一工具×N」）、允许 wrap 或把全文放 `title`。顺带 `NODE_H = 76` 纵向只剩约 1px 余量（全仓未显式设 line-height，body 也没设——`styles/global.css:861-867`），报告已承认没目视核对，建议给行/胶囊定显式 line-height。

---

## 3. 安全

**未发现。** `lastTool` 只经 JSX 文本与 `title` 属性（`DispatchBits.tsx:64`、`dispatch.ts:1146`），React 转义，无 `dangerouslySetInnerHTML`；引擎侧已脱敏 + 截 120 字符（`REPORT-07A:95`）。本期无新增命令/网络外发，`cancelDispatchNode`（`DispatchOverlay.tsx:543`）是既有调用。

---

## 4. 代码质量与契约

- 类型只追加 ✓（`tauri.ts:530-544` 新字段全可选；`ProgressRun`/`DispatchProgressStore` 是新类型，无既有类型被改）。
- overlay 本地 state 无遗留引用 ✓：全文件已无 `setProgress(` / `setNow(` / `statuses`，`progressOf`/`nextProgress` 也没被误留 import；`runningDetailOf`/`progressStatusLabel`/`IdleHint` 均在用。
- 2–7 期路径无回归 ✓：`start()`/`retry()` 只是各多一次 `beginRun`，`retry` 的 `nextRunId` 重排等价（`setRunId` 语义不变）；`runStageLoop`、对账收敛、`rollbackNodes`、擂台/仲裁代码未动。`onStage` 从「写 statuses + currentNodes」变「只写 currentNodes」后，"排队"改由快照缺失推断；同一 run 内不会重派同一 nodeId（`runStageLoop` 每个节点只派一次），等价。
- **F5（轻微）** `dispatch-progress.ts:65,99,103` `resetRun` 死接口；store 暴露的 `runId`/`progress` 没有消费者（正门是 `snapshotOf`）。列为清理项，不阻塞。

---

## 5. 测试

- 报告里的 **10 条真实存在**，名称与 `REPORT-07C:155-166` 一致（3+3+3+1）。其中 7 条是行为断言（07C-1-1/3、07C-2 三条、07C-3-1/2），3 条是源码正则（07C-1-2、07C-3-3、07C-4）。后者钉接线可以，但像 `expect(shared).toMatch(/setRun\(EMPTY_PROGRESS_RUN\);\s*\},\s*\[graphId\]\)/)` 这类对格式敏感，价值有限。
- **关键缺口：07C-2 的「终态冻结」用例（`idle-and-tail-07c.test.tsx:144-154`）喂的是带 `elapsedMs/lastTool` 的终态事件**（`heartbeat({ status: '完成' })` 仍带默认字段），正是引擎不会发的形状——它验证了假前提，也是 F1 漏网的原因。
- **最该补的 3 条**：
  1. 终态事件只带 `{ runId, nodeId, status: '完成' }` → 节点卡/运行行仍显示上一条心跳的「已运行 42 秒 · 工具 read_file」且不出胶囊（现状红，直接暴露 F1）。
  2. 订阅生命周期：`vi.mock('@tauri-apps/api/event')`，并把 effect 体抽成可测的 `subscribeProgress(runId, onEvent)`（仓库无 jsdom/testing-library，纯 hook 渲染测不了；抽出后即可在 node 环境断言 runId 变更与卸载各 unlisten 一次、迟到的 listen 不残留）→ 覆盖 F2。
  3. 时钟门控：fake timers 断言 `now` 只在有 running 快照时前进、全终态后停，异常收尾不再 tick → 覆盖 F3 的 effect 那半（现在只测了纯函数 `progressRunningAny`，effect 靠读源码）。
- 报告与事实的两处偏差（F1 的"冻结"、F4 的"胶囊完整显示"）建议一并更正，避免假结论留在档案里。

---

## 是否可交付

**不能按现在这样交付：差一处契约实现（F1，终态把上一条心跳读数清空，用户可见）和一处监听泄漏竞态（F2）；这两条修掉后可交付，F3/F4/F5 可作收尾。**

缺项列表（按严重度）：
1. **F1** 终态事件覆盖上一条心跳，违反 TASK-07 契约「前端保留 `elapsedMs`/`lastTool`」——`dispatch.ts:1061-1072, 1078-1086`，补"无字段终态事件"测试。
2. **F2** 异步 `listen` 无取消守卫，卸载/换 runId 可能泄漏监听——`dispatch-progress.ts:78-86`，补 mock 订阅测试。
3. **F3** 异常收尾后 stale `running` 让 5s 时钟永不停并误挂空转胶囊；`resetRun` 是死接口——`dispatch-progress.ts:89-96, 65/99`。
4. **F4** 节点卡最长空转胶囊被裁切（报告「胶囊完整显示」不实）——`GraphTab.css:250-266`；顺带显式 line-height。
5. 报告更正 F1/F4 两处陈述 + 清理 `resetRun` 死代码。