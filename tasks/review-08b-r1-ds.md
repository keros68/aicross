复审完毕。本会话无 shell，结论来自逐行读码 + 与两个评审文档/报告的对照（编排者已复跑 vitest 332 / build 全绿，我按报告原文采信）。

## 逐条核对（10 条）

**1. G1 闸门窗口期阻断 — 已核实修好（两处边角未闭合）**
`swarm.ts:234-276` 三态 `idle|pending|ok|error`，pending 一律 `blocked:true` 且出「正在核对文件范围…」；error 默认阻断、带错误原文、`canProceed:true`，勾「仍要派发」才放行（`DispatchOverlay.tsx:230-231,493,632`，`DispatchBits.tsx:86-110`）。核对在途那段窗口确实被堵上了。边角：① `DispatchOverlay.tsx:218-220` 在 `projectDir === ''` 时把状态置回 `idle`＝不拦也不提示；② 进入确认屏的首帧 effect 尚未跑，`swarmCheck` 仍是 `idle`，存在一帧可点窗口（pending 只在 effect 里置位）。字面要求是「核对未返回时阻断」，实现按「已发起且未返回」解释成立。

**2. 生成幂等 + 上限 — 已核实修好**
`swarm.ts:143-152` 收集同拆分下已有件号；`swarm.ts:154-175` 同号跳过（记 `skipped`）、`existing.size + 新建数 >= limit` 截断（记 `capped`）、一件未建时返回原图引用；`swarmGenerateNotice`（`swarm.ts:215-222`）出「已生成 N 个 / 第 N 件已生成，未重复建 / 件数上限 N 件」；`DispatchOverlay.tsx:408-419` 把 `settings.swarmLimit` 同时喂给解析与生成，没新建就不 commit。测试 R1-2 覆盖三态。

**3. 合并幂等、逐件落图、失败分级 — 已核实修好**
幂等：`swarmPendingMergeIds`（`swarm.ts:319-324`）剔掉 adopted，`DispatchOverlay.tsx:428` 合并中/无待并件直接返回，`:729` 按钮 disabled 含 `merging` 与 `length === 0`。落图：`swarmReplyNote`+`applySwarmMerge`（`swarm.ts:419-444`）把 `[蜂群合并] 第 N 件…` 追加进对应节点 Reply（同行不重复追加），merged 才 `setReplyAdopted`，引擎未接通那批一个字不写。分级：`swarmMergeErrorResults`（`swarm.ts:355-362`）只在 `Tauri IPC not available`/空错误时说未接通，其余写「合并未完成：原文」；`swarmMergeSummary`（`swarm.ts:399-417`）只在「全部 skipped 且 note ＝引擎未接通」时说未接通，空数组说「结果未知」，部分并入按件报数。

**4. 蜂群卡片动作约束 — 已核实修好（多关一个按钮，已注明）**
`swarmCardActions`（`swarm.ts:463-475`）：swarm 件 `showPickOne:false`、`showAdopt` 只在本次 merged 或节点已 adopted、已并入 `showRetry:false` 并给「已并入」；接线在 `DispatchOverlay.tsx:676,698-709`。报告已写明「升级」与「重派」同路径一并关掉，比字面多关一个，合理。

**5. 重叠件就地改范围并自动重核对 — 已核实修好**
重叠路由卡内联 `SwarmEditor`（`DispatchOverlay.tsx:529,541-546`，复用 `NodePanel.tsx:275-315`），写回 `updateNodeSwarm`（`mutate.ts:77-88`）→ `graph` 变 → `DispatchOverlay.tsx:216-229` 的 effect 重发核对、proceed 复位；提示语已含「改到不重叠或合并成一件」（`swarm.ts:271`）。编辑框只在 overlap 已知（`ok` 态）时出现，符合条目字面；R1-5 测试是纯函数级（改后核对入参），与条目要求一致。

**6. 盖预设菜单包含蜂群 — 已核实修好**
`GraphTab.tsx:109`（预设变更 reload）与 `:160`（首次加载）都套 `withSwarmPreset`；全仓 `listPresets()` 只有 GraphTab 两处 + `OrchestrationPanel.tsx:53,74`（也套了），无漏网入口。测试 R1-6 读源码断言两处，弱但可接受。

**7. 拆分提示词与上限、roleDefaults 显式分支 — 已核实修好**
`stamp.ts:9-20` `swarmSplitText(limit)`；`stamp.ts:42-46` `stamp(..., swarmLimit)` 默认 4、`:87` 只对 `builtin-swarm` 走动态文本；两个盖章入口都传 `loadDispatchSettings().swarmLimit`（`GraphTab.tsx:370`、`OrchestrationPanel.tsx:140`）；`swarmPlanNotice(plan, limit)` 直接收上限（`swarm.ts:104-107`）；`types.ts:135` 显式 `拆分→共享/只读`。报告写明 A 侧 split.txt 落地后要删的位置，符合条目要求。

**8. 每件只带自己那条清单 — 已核实修好（一条口径备注）**
生成节点 `user.text` 只有本件（`swarm.ts:119-123`），可见性改 `指定上游` + `upstreams=[graph.root]`（`swarm.ts:180-181`），`context.ts:125` 只保留 upstreams 里的 reply → 拆分回复（整份清单）与别件都进不来，`upstreamArtifactsOf` 也不跨件。备注：`context.ts:101` 对所有祖先的 user 段无条件保留，所以拆分节点的用户消息（含「只读材料，不要改任何文件」）仍在每件 prompt 里；「只带根材料」不算严格成立（R1 之前就如此，非本轮引入）。

**9. `Node.swarm` 持久化说明 — 已核实修好（位置与条目要求不同名）**
`REPORT-08B.md:412-418` 写清 A 侧 `GraphNode.swarm` 镜像并入前 `save_graph` 会静默丢弃、后果（闸门不触发/合并区不出现/编辑框消失且不报错）与「合并对账时与 A-1 一并核对」。报告里没有名为「合并对账」的小节，内容落在 R1 §9，实质满足。

**10. GLM 3 合并入口从图状态派生 + camelCase 键名 — 未修（半条已按允许项处理）**
请求键名那半：保留 `nodeId`+`node_id` 双写并加注释（`tauri.ts:559-565`、`swarm.ts:330-337`），符合条目「或保留但注明」；注释里也说了「多出来的键被 serde 忽略」——若 A 侧用了 `deny_unknown_fields` 双写反而会整批反序列化失败，合并时值得对一眼。派生那半：`swarmMergeEntry(graph, stage)` 不存在，合并区与逐件结果仍绑当次会话（`DispatchOverlay.tsx:422-425、720-729`），报告声明「不采纳（本轮）」。不采纳理由（按批还是按拆分未定、图里没有可用的 runId 供留痕）部分成立，但条目 10 是本轮清单明列项，且 DS 2.4/GLM 2-3 同题——**属明确未闭合**。

## 不采纳项的理由是否成立

- GLM 2-3（= 条目 10）不采纳：**理由不足**——本轮任务书明列此条，且「runId 取值」并非无解（reply/留痕里已有可用的 run 关联线索或可按拆分节点记一次 run），把它推给下一轮等于把 10 条做成 9 条。
- GLM 4-3（核对防抖）不采纳：**成立**，先把在途做成阻断再谈省事，且每次键入只发只读命令。
- GLM 1-1（去掉 `parseSwarmPlan` 默认参数）不采纳：**成立**，唯一调用点已显式传参，属预防性收口。

## 修正是否引入新缺陷 / 既有路径回归

擂台、仲裁、备选、对账、回滚、心跳均未发现回归：`arbitrationCandidates`（`swarm.ts:446-451`）只在有 swarm 件时收窄候选（`DispatchOverlay.tsx:735`），非蜂群批次行为不变；`pickOne`/`sameParentsResultCount` 逻辑未动，蜂群卡不再出「选它」；`retry`→`rollbackBeforeOf`（worktree 不带 rollback）未动；进度 store 未动；`stamp`/`roleDefaults`/`swarmGate` 的签名变化都有默认值或唯一调用点同步；`stamp.ts → swarm.ts → dispatch.ts` 无环（`mutate/validate/layout` 都不反向 import）。

新发现的小口子（都是残留级，不阻断主链路）：
1. `SWARM_CHECK_FALLBACK`（`swarm.ts:230`）在应用代码里已无调用点，只剩测试引用；`tauri.ts:361` 注释仍写「调用方回退到 SWARM_CHECK_FALLBACK」——R1-1 改了行为没同步注释与那条旧断言。
2. 已并入件在结果卡会出「取消采纳」（`DispatchOverlay.tsx:706`）→ adopted 清掉后该件重回待合并清单，可对同一 worktree 再合一次（引擎多半按「无改动→skipped」兜住，但语义上破了「已并入不再并」）。
3. 非同批蜂群兄弟卡（同拆分下手动加的执行节点）出「选它」时，`pickOne` 的 rivals 只看 kind/role/parents（`dispatch.ts:410-428`），能清掉已并入蜂群件的 adopted——R1-4 只做了卡片级而非图级约束。
4. 跨拆分节点件号都从 1 起（`swarm.ts:154-175` 用 index+1），两批同派时 G1 会出「第 1 件与第 1 件都会改…」，合并按件号排序两批交错；DS 2.5 提的「按图上最大值续号」未做。
5. 报告行号有 1 行左右漂移（如 `swarm.ts:416`→实际 419、`DispatchOverlay.tsx:728`→729），不影响结论。

## R1 之后 TASK-08B 是否可交付？

**可交付（B 侧），但必须把「合并入口按图派生」作为已知缺口带进下一轮/A 侧口径确认后立即补——它是本轮 10 条里唯一没做的一条，只在关屏重开、单件重派、分批生成这些次路径上减弱，不阻断「生成→派发→合并」主链路。**

仍未闭合项（按严重度）：
1. 【高】条目 10 / GLM 2-3 / DS 2.4：合并入口与逐件结果绑当次 overlay 会话，无 `swarmMergeEntry(graph, stage)`；关屏重开合并区消失、单件重派后清单缩水、同图另有同拆分的 draft 件时仍按「本批」合并（`DispatchOverlay.tsx:422-425,720-729`）。
2. 【中】跨拆分节点件号重号：G1 文案失效（「第 1 件与第 1 件」）、合并排序交错（`swarm.ts:154-175`，DS 2.5 续号方案未做）。
3. 【中低】R1-1 边角：`projectDir === ''` 时核对不发→idle 放行；确认屏首帧（effect 前）仍是 idle，存在一帧可点窗口（`DispatchOverlay.tsx:218-220`）。
4. 【中低】每件 prompt 仍带拆分节点的用户消息（「不要改任何文件」会进执行者上下文），「上下文只带根材料」不严格（`context.ts:101`，非本轮引入）。
5. 【低】已并入件可「取消采纳」重回合并清单（`DispatchOverlay.tsx:706`）；兄弟卡的 `pickOne` 能清掉已并入件的 adopted（`dispatch.ts:410-428`）。
6. 【低】`SWARM_CHECK_FALLBACK` 成死常量 + `tauri.ts:361` 注释过期；合并请求双写在 A 侧 `deny_unknown_fields` 场景下的风险未验证（`tauri.ts:559-565`）。