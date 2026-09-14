# TASK-07C 代码审查

已读：TASK-07 契约与 B-1、REPORT-07C、dispatch-progress.ts、dispatch.ts 相关组（1055–1259）、07C 测试全文、07B 被改用例（204–216）、GraphTab.tsx / DispatchOverlay.tsx / DispatchBits.tsx / GraphTab.css 相关段，并交叉 grep 了全 src-ui 的引用面。编排者已复跑 vitest 288 / build 绿，以下为静态审查结论。

## 1. 需求完整性

五项语义逐条核对，与 R1 一致：

- **同一份快照**：GraphTab 只建一个 store（GraphTab.tsx:81），运行屏走 `progressStore.snapshotOf(node.nodeId)`（DispatchOverlay.tsx:532），节点卡走 `progressStore.snapshotOf(id)`（GraphTab.tsx:570），两者都收敛到 `progressRunOf` → `progressOf`（dispatch-progress.ts:49-51 → dispatch.ts:1090-1097）。grep 确认无第二个消费入口。
- **订阅只有一处**：`'dispatch-progress'` 字面量与 5s 时钟只在 dispatch-progress.ts:82/94；DispatchOverlay 已无 `@tauri-apps/api/event` import 与 `setProgress`/`setNow`/`statuses` 残留（grep 全仓干净）。
- **按 runId 取快照**：`nextProgress` 按 `payload.runId !== runId` 丢（dispatch.ts:1084），`progressOf` 再按 `snapshot.runId === runId` 挡渲染（dispatch.ts:1096），双层都在。
- **重派/换图复位**：`beginRun` 在 start（DispatchOverlay.tsx:218）与 retry（:274-278，先生成 `nextRunId` 再一起写）各一次；换图按 `[graphId]` 复位 `EMPTY_PROGRESS_RUN`（dispatch-progress.ts:73-75）。与 R1-2 口径一致。
- **终态冻结 / 无心跳元数据不判空转**：`runningDetailOf` 非 running 时 wall=0（dispatch.ts:1144）；`idleHint` 先查 `progressRunning` + `hasHeartbeat`（dispatch.ts:1124），排队/旧引擎事件（elapsedMs、idleMs 都缺 → hasHeartbeat=false）一律不判。与 R1-1/R1-4 一致。

**发现 1 条（低，语义副作用）**
- `dispatch-progress.ts:73-75` — **问题**：换图复位对「切走再切回」也生效，复位后 `run.runId === ''`，监听 effect（:79）不再挂，本轮后续心跳永远进不来。派发进行中在同一会话里切到图 B 再切回图 A，节点卡与运行屏的读数/胶囊会永久消失（直到重派），不是「晚 15s 恢复」。 — **为什么**：复位目的是防跨图污染，但按 graphId 无条件复位把「离开在飞的那张图」也当成了丢弃；旧实现监听在 overlay 里（依赖 `[phase, runId]`），切图不丢。另注：切到别的终端 session 导致 GraphTab 整体卸载、丢派发 UI，是既有行为，非本期引入。 — **建议**：不必本期改（契约明写「换图复位」，测试也钉了）；若要修，把 run 按 graphId 存 map（`Record<graphId, ProgressRun>`），复位只清「进入的那张图」的键，或复位时保留仍 live 的 runId 并重挂监听。至少在任务书或 REPORT 里记一句这个代价。

## 2. 逻辑正确性

**发现 2 条（均低危，其一为继承）**

- `dispatch-progress.ts:78-86` — **问题**：listen effect 的经典竞态——动态 `import().then(listen)` 尚未 resolve 时组件卸载/换 runId，cleanup 先执行（此时 `unlisten` 还是 undefined），promise 随后 resolve 把 `stop` 赋给已无人消费的变量，监听器泄漏。 — **为什么**：泄漏的监听器仍会触发 `setRun(applyProgressEvent(...))`，但 runId 不匹配被 ：1084 挡下并返回同引用，不重渲染——后果限于每次重派累积一个空转的 listener 与一次多余的 state updater 调用，无串图、无错读数。该模式系从旧 overlay effect 原样搬来（REPORT「不确定处」也认了重挂窗口），非本期新引入。 — **建议**：加 cancelled 标志：`let cancelled = false; ... .then((stop) => { if (cancelled) stop(); else unlisten = stop; })`，cleanup 里 `cancelled = true; unlisten?.();`。一行级修复，顺手可做。
- `dispatch-progress.ts:86`（重挂窗口）— **问题**：runId 变化时先 cleanup 旧监听、再异步挂新监听，新 run 的第一条心跳（若 15s 内就到，引擎首跳前还有排队期，实际难撞上）或旧 run 的终态事件落在窗口里会被丢。 — **为什么**：旧 effect 依赖 `[phase, runId]` 同样有此窗口，无新增风险；丢一条心跳只是读数晚一跳。REPORT 已如实记录。 — **建议**：不处理；若日后出现「重派后首跳丢失」的实测报告，再考虑先挂新监听后拆旧的。

**核对无误的项**：
- 5s 时钟只在 `progressRunningAny` 为真时走（:90-96），全终态即停；running 为布尔，心跳更新不重触发 effect，间隔不重置，正确。beginRun 后 `now` 陈旧导致首帧 wall 被 `Math.max(0,…)` 钳为 0，下一帧即被 effect 的 `setNow` 纠正，无负数、无持续错误。
- 迟到事件/别轮次事件：`nextProgress`（runId 挡写入）+ `progressOf`（runId 挡渲染）+ `applyProgressEvent` 同引用返回不触发重渲染，三层都验证到位（07C-3 两条用例钉住）。
- NODE_H 56→76：唯一常量（GraphTab.tsx:37），画布高（:412）、层内偏移（:415/418）、连线端点 `p.y + NODE_H/2`（:524/526）、卡片 inline height（:550）、按钮行 `p.y + NODE_H + 4`（:573）全部派生自它；grep 无遗留的硬编码 56。`.graph-node` 确为 `overflow: hidden`（GraphTab.css:201），加高必要。三行内容（约 16+4+16+4+14+16px）在 76px 定高内放得下，`justify-content: center` 居中。
- 串图：runId 为空不挂监听，别图事件天然被 runId 挡；换图先复位再重挂，顺序正确。

## 3. 安全

未发现。`lastTool` 只经 `runningDetailOf` 拼成字符串进 JSX 文本与 `title` 属性（DispatchBits.tsx:64-65），React 对两者都转义，无 `dangerouslySetInnerHTML`。共享层只 `listen` 不 invoke，无新增外发；「叫停」仍走既有 `cancelDispatchNode(runId, nodeId)`（DispatchOverlay.tsx:543），runId 与 `beginRun` 传入的同一份，口径一致。

## 4. 代码质量与契约

- **类型只追加**：`DispatchProgress` 五个字段全部可选（tauri.ts:530-544），`Reply.tokens_cache_read?` 在 types.ts:22，均纯追加，符合契约「旧前端忽略未知字段」。
- **无遗留引用**：`setProgress(`/`setNow(`/`statuses` 全仓 grep 只剩共享层自身的 `setNow` 与测试的反向断言；`onStage` 已简化为只写 `currentNodes`（DispatchOverlay.tsx:238），「排队」改为「无快照」默认值的口径合一，报告的等价性论证成立（onStage 与首跳之间确实在排队）。
- **第 2–7 期路径无回归**：`runStageLoop`（dispatch.ts:1216-1259，重复批次放回 draft、missing-target、limit 分支原样）、`reconcileUiConverged`（:1158）、`rollbackBeforeOf`/`toggleRollbackOnReview`（:912/:924，overlay :99-101/:196-197/:281/:430 照旧）、擂台/仲裁（`sameParentsResultCount`/`appendArbitration`/`pickOne`，overlay :586-658）通读均为 07B 既有形状，07C 只加了 `progressRunningAny`（:1109-1113）这一个导出。叠加 tsc+vitest 全绿，未见回归面。
- 小疵 2 条（不必改）：DispatchBits.tsx:64 — 只挂胶囊没有读数时 `title=""`，空 title 属性无害但可 `title={detail || undefined}`；dispatch-progress.ts:54-67 — store 接口里的 `runId`/`progress` 两个消费方都没用（overlay 用自己的本地 runId），属多余 API 面，留作后续即可。

## 5. 测试

报告所列 10 条**全部真实存在**（07C-1×3、07C-2×3、07C-3×3、07C-4×1），且断言的是行为而非存在性：同对象恒等 + 同一句文案（:48-62）、单订阅的源码反断言（:64-83）、时钟门控（:85-95）、NodeProgressRow 三态含终态冻结与「不占行」（:99-155）、重派/换图复位含迟到事件同引用（:159-196）、接线断言（:198-210）、判定逻辑不复制（:214-229）。07B 的接线用例同步改到 `snapshotOf`/`beginRun` 且「叫停」断言保留（idle-and-tail-07b.test.tsx:204-216）。源码正则式断言脆，但与仓库「无 jsdom、读源文件断接线」的既有做法一致，不算缺陷。

最该补的 3 个：

1. **hook 生命周期（vi.mock 事件桥）**：mock `@tauri-apps/api/event`，断 runId 为空不挂 listen、每个 runId 恰好一次、换图/卸载时 unlisten 被调，特别是「listen 在 unmount 之后才 resolve 时仍要 stop()」——把上面那条竞态钉死，替掉目前对 ：78-86 的纯正则断言。
2. **时钟行为（fake timers）**：running 期间 `now` 每 5s 前进、全终态后 interval 停——目前 `setInterval` 的启停只有源码断言，`progressRunningAny` 的纯函数测试盖不住 effect 那半边。
3. **运行屏「排队→运行中」经共享 store 的组件级用例**：无快照显示「排队」、首条心跳后变「运行中」+ 读数——把报告「不确定处」里 `statuses` 删除后的语义变化在组件层钉住，防止将来有人往 `onStage` 加状态词时两边漂移。

---

## 是否可交付？

**可交付**——需求语义与 R1 逐条对得上、订阅确实只剩一处、288 测试与 build 均绿，所列缺项无一阻塞合并。

缺项（按严重度排序）：
1. listen/unlisten 竞态泄漏（dispatch-progress.ts:78-86，继承自旧实现，一行级 cancelled 标志可修）；
2. 换图「切走再切回」后在飞 run 的进度显示永久丢失（dispatch-progress.ts:73-75 + :79 的组合后果，与 R1 契约一致但契约本身有此代价，建议记录或按 graphId 存 run）；
3. runId 重挂窗口可能丢首跳/终态事件（报告已声明，无新增风险，观察即可）；
4. 测试缺口：hook 生命周期、时钟启停、运行屏排队→运行中三例（见第 5 节）；
5. 小疵：空 `title=""`、store 的 `runId`/`progress` 两个未消费字段。