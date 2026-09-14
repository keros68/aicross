## 逐条核对

### R1-1 终态事件保留上一条心跳读数 ——**已核实修好**
- `src-ui/src/graph/dispatch.ts:1065-1071`：`progressSnapshot(payload, receivedAt, previous?)` 在 `hasHeartbeat` 为假且 `previous.runId === payload.runId` 时返回 `{ ...previous, status: payload.status }`，`elapsedMs/idleMs/toolCalls/repeatedToolCalls/lastTool/hasHeartbeat/receivedAt` 全沿用；`:1097` 把 `current[payload.nodeId]` 喂进去。
- 冻结不是靠这个分支，而是 `runningDetailOf`（`:1154-1160`）的 `progressRunning` 判据——终态时 `wall = 0`，所以读数停在最后一条心跳。两点合起来才成立，R0 的报告已就地更正。
- 测试 `idle-and-tail-07c.test.tsx:267-311`：只喂 `{runId,nodeId,status:'完成'}`、过 5 分钟仍 42s/read_file、不挂胶囊、`progressRunningAny` 假，另加跨 runId 不借用、无上一条心跳不编数两条负向。跨轮守卫实际是兜底（`nextProgress` 已按 runId 挡写入），不编数的行为正确。
- 小疵（非缺陷）：老用例 `:177-190` 仍用 `heartbeat({status:'完成'})`（带字段）验「冻结」，标题与引擎形状不符，覆盖已由新用例补上。

### R1-2 listen 取消守卫 ——**已核实修好**
- `src-ui/src/graph/dispatch-progress.ts:73-97`：`disposed` + `:86-87` 的 `if (disposed) stop(); else unlisten = stop;`，`:91-95` 幂等 dispose；`:77` 空 runId 不订阅；`:89` 失败改 `console.warn`。hook 侧 `:124-131` 只剩建订阅 + `dispose`，依赖 `[run.runId]`。
- 测试 `:313-366`：空 runId 不 listen、解析后 dispose 各一次、重复 dispose 不重复解除、**迟到 resolve 的监听就地被 stop**、失败留痕。换图那条路径也自洽：`[graphId]` 复位把 runId 变空，旧订阅在下一渲染被 dispose，窗口里的事件被 `nextProgress`（runId 不匹配）返回同引用挡掉。

### R1-3 异常收尾后时钟停 ——**已核实修好**（留一个低危小洞，见下）
- `dispatch-progress.ts:56-69` `endProgressRun`：只把仍是 running 的快照标成 `'已结束'`，读数保留；全终态返回同引用。`:144` `endRun`，死接口 `resetRun` 全仓已无（grep 0 处）。
- `DispatchOverlay.tsx:260-265` `start()` 的 `finally` 调 `endRun()`，覆盖异常、`missing-target` 早退、正常跑完；门控 `progressRunningAny`（`dispatch-progress.ts:138`）翻假后 `:136-141` 的 effect 清掉 interval，`idleHint` 又被 `progressRunning` 前置挡住 → 不挂胶囊。`retry()` 先 `beginRun`（`:279-283`）本就清空，不需要再收尾。
- 测试 `:368-426`：fake timers 下 running 期间 now 每 5s 前进 4 次，`endProgressRun` 后再推 10 分钟不动、读数冻结、无胶囊、重复调用同引用；接线断言 `finally { … endRun() }` 且无 `resetRun`。
- 新洞（低危、非回归）：`stopped` 路径下引擎若在 `finally` 之后仍发一条 running 心跳，`applyProgressEvent` 会把状态翻回 running、时钟重启；只有在该节点终态事件永不到达时才会在 3 分钟后误挂胶囊。R0 是「永不停止」，此处只赢不输，自愈，不阻塞。

### R1-4 胶囊不裁切 ——**已核实修好**（样式层，仍无目视）
- `GraphTab.css:255-278`：第三行 `flex-wrap: wrap` + `row-gap: 2px` + 显式 `line-height: 1.4`（`:223` 的 `.graph-node-row` 也补了）；读数 `flex: 1 1 100%` 独占一行 + ellipsis；胶囊 `max-width: 100%` + `white-space: normal` + `overflow-wrap: anywhere`，`flex-shrink: 0` 已删。全局 `box-sizing: border-box`（`styles/global.css:83`）让 `max-width: 100%` 含 padding/border，142px 内容宽内换两行 36.8px 成立，不再溢出被 `.graph-node{overflow:hidden}`（`:201`）切。
- 高度核算对得上：内容 16.8+4+15.4+4+54.2=94.4 + padding 16 + 边框 2 = 112.4 ≤ `NODE_H = 116`（`GraphTab.tsx:40`，留 ~3.6px 余量，`justify-content: center`）。
- 测试 `:428-466`：样式断言（wrap/行高/胶囊不再 shrink/读数占行/`NODE_H >= 112`）+ 字符串断言（最长文案原样在 `<span class="hint-capsule">` 内、markup 无省略号）。报告已如实写「未起窗口目视」。

## 特别核对项
- **终态读数冻结**：成立（见 R1-1），同 run 内不再被补 0 冲掉。
- **dispose 守卫**：成立，换 runId/卸载各一次、迟到 listen 立即解除、幂等。
- **`start()` finally 的 endRun**：成立，时钟停（门控 + 清 interval）、不挂胶囊。
- **节点卡第三行与 `NODE_H`/连线一致**：成立。`NODE_H` 仍是唯一常量——分层坐标与画布高 `GraphTab.tsx:415/418/421`、连线端点 `:527/529`（`NODE_H/2`）、卡片 inline 高 `:553`、按钮行 `:576`（`NODE_H+4`）、canvas-inner 高 `:816`；grep 无第二处硬编码旧值。
- **新缺陷/第 2–7 期回归**：未见。改动面只在新模块、`progressSnapshot` 可选参数（07b 两处 2 参调用语义不变）、overlay 消费 store、`NODE_H` 布局增量；`runStageLoop`/对账收敛/rollback/擂台/仲裁未动，`beginRun` 仍是 start+retry 各一次，`endRun` 对正常完成是同引用空操作。唯一语义外溢：同 run 内「只有状态词」的非终态事件（如「等待写锁」）现在会继承上一条心跳读数——显示上非 running → 冻结读数、不点时钟、不挂胶囊，且契约写明引擎只在终态发无字段事件，风险可忽略。

## R1 之后 TASK-07C 是否可交付
**可交付**——4 条必修全部核实修好、报告 F1/F4 两处失实陈述已就地更正，余项均为已记录的低危/观察项，无阻塞。

仍未闭合项（按严重度）：
1. **切图再切回**：在飞 run 的读数与胶囊永久消失（`dispatch-progress.ts:119-121` + `:125-126`），已按 GLM 缺项 2 记为已知代价——契约如此，但用户可见。
2. **`endRun` 可被迟到的 running 心跳翻回**（stopped 路径，`dispatch-progress.ts:61-69` + `DispatchOverlay.tsx:260-265`），时钟可能重启、极端下误挂胶囊；比 R0 只赢不输。
3. **无 effect 级测试**：R1-3 的时钟用例在测试里复制了门控逻辑（`idle-and-tail-07c.test.tsx:381-394`），hook effect 仍靠源码正则（仓库无 jsdom 的既有约束）。
4. **卡片 116px 未目视核对**（报告已声明）；字体度量差异只影响留白，不会再退回裁切。
5. 老用例 `07C-2「终态冻结」`（`:177-190`）仍喂带字段的终态事件，标题易误读，覆盖已由 R1-1 用例补上。
6. 清理项：store 的 `runId`/`progress` 无消费者（报告记录理由）；非 Tauri 环境订阅失败走 `console.warn`，属噪声。