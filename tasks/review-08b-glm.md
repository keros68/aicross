# TASK-08 B 节代码审查（phase/8b）

审查方式说明：本会话无 shell 工具，测试结论基于逐条通读 `swarm-08b.test.tsx` 全部 24 条用例的断言体（与 REPORT-08B.md 的 verbose 清单逐名核对一致）+ 报告原文输出，未亲自复跑。

## 1. 需求完整性

B1–B6 逐条对照均落地，回退分支存在且被测试钉住（`swarm.ts:176` `SWARM_CHECK_FALLBACK`、`swarm.ts:244` `swarmMergeFallback`；测试 B1-3 断言 reject 后回退形状）。`swarm_limit` 生效链完整：`dispatch-settings.ts:40-42` clamp 1–4 → `SettingsModal.tsx:626-633` 下拉 → `DispatchOverlay.tsx:398` `parseSwarmPlan(text, settings.swarmLimit)` 截断。未发现阻断性缺项。

- **swarm.ts:78** — `parseSwarmPlan` 默认参数 `limit = SWARM_LIMIT_DEFAULT` 硬编码 4，而设置项可改成 1–4。为什么：若未来有调用方忘了传第二参，就绕过用户设置。建议：`parseSwarmPlan` 不带默认值，强制调用方显式传（现在唯一调用点已传，属预防性收口，低优先）。
- **types.ts:130-135** — 任务书 B-2 字面要求「roleDefaults 补 拆分→共享/只读」，实现走兜底分支 + 注释。行为等价且报告已声明，不算缺项，仅记录口径差异。

## 2. 逻辑正确性

parseSwarmPlan（最后一个围栏 `swarm.ts:83`、字段校验 `:90-97`、截断 `:99-100`）、生成节点字段（`swarm.ts:156-164`：parents=[拆分]、role=执行、write=worktree、swarm 件号从 1）、G1 触发条件（`DispatchOverlay.tsx:208-219` 只在本批含 swarm 节点且 phase=confirm 时调）、标红（`:514`）、禁用确认（`:609`）、unmatched 只提示（`swarm.ts:203-204`）、G3 终态判定（`swarm.ts:222-229`，failed 也算终态）、merged 自动 adopted（`swarm.ts:292-300`）、conflict/test_failed 不自动重派（`runSwarmMerge` 无 retry 调用）——均正确。

按严重度排序的发现：

- **tauri.ts:559 + swarm.ts:236 — 合并请求的字段命名与契约文本不一致，且请求侧没有兼容层。** 契约写 `items: [{ node_id, ... }]`，TS 声明 `SwarmMergeRequestItem.nodeId` 直接随 invoke 下发。为什么：响应侧做了 `normalizeSwarmMergeResults` 双写兼容，请求侧却单押 camelCase——若 A 侧按任务书字面用 snake_case serde 字段，`merge_swarm_items` 反序列化失败 → 前端 catch → 全部「已跳过 · 引擎未接通」，蜂群合并静默失效且难以察觉。建议：请求体同样双写（每件同时带 `nodeId` 与 `node_id`），或与 A 侧确认后统一一种并在任务书勾销歧义。
- **DispatchOverlay.tsx:684-686 + swarm.ts:133-170 — 「按清单生成执行节点」无防重复点击守卫。** 按钮在生成后仍可点，再点一次会生成第二套同件号节点：G1 核对会出现 `a===b` 的重叠对（提示文案变成「第 1 件与第 1 件都会改…」），合并请求件号重复。同理，对两个不同拆分节点各生成一批时件号跨批冲突，合并按件号排序会交错两批。建议：生成前查 `childrenOf(splitNodeId)` 是否已有带 `swarm` 的子节点（`stamp.ts:85` 现成），有则置灰按钮或直接 return。
- **DispatchOverlay.tsx:411 + 694-704 — G3 合并区绑定本次 overlay 会话的 `results`。** 关闭弹层重开后 `results=[]`，合并区消失，即使全部件已终态也无法合并；对单件「重派」（`:316` retry 会 `setResults([])` 并换 runId）后，合并清单缩水成只剩被重派那一件。为什么：合并是图状态驱动的动作，不该依附单次派发会话。建议：`swarmResultIds` 改从图上算（全部带 `swarm` 且终态的节点，或至少「全部带 swarm 的节点」），runId 用这些节点 reply 里记录的 run——需要图上有 run 记录的话与 A 侧对齐。
- **DispatchOverlay.tsx:673-680 — 擂台「选它」出现在蜂群批次上。** 同父同角色的 N 件执行结果使 `sameParentsResultCount >= 2` 成立，每张卡都出「选它」；合并后点它会清掉其它件的 adopted（`pickOne` 的 rivals 逻辑），与「件件互补、逐件合并」语义相扰。建议：`node.swarm` 存在时不渲染 PickOneButton。
- **DispatchOverlay.tsx:705-708 — 仲裁区同理** 在 ≥2 件 ok 的蜂群阶段照常出现。两件蜂群活不是分歧，用户误入会建出无意义仲裁节点。建议同上，swarm 批次内不出仲裁行。
- **dispatch.ts:133-149 — 会话续聊对蜂群件生效。** 执行件是共享、单父；若执行默认与拆分（出方案）默认同 agent/model，N 件会并发续同一个拆分会话，互相踩写且让每件看到完整拆分过程。为什么：PRODUCT.md 明确「各件只看根材料 + 自己那一件」是边界。建议：`continuationSession` 对 `node.swarm` 存在的节点返回 undefined（各件独立起会话）。
- **DispatchOverlay.tsx:208-221 — G1 核对在途时门禁是开的。** `swarmCheck` 初始为 null，`swarmGate(null)` 返回 blocked:false，异步核对返回前确认按钮可点，存在绕过重叠/脏树阻断的小窗口；`projectDir === ''` 时则完全不核对。建议：swarmIds 非空且 check 未返回时按 pending 处理（禁用确认或至少提示「核对中」）。
- **DispatchOverlay.tsx:694-701 — 「合并」按钮是常显禁用而非终态后才出现。** 任务书 B-5 字面是「全部终态后出按钮」，实现为「提前出现 + 禁用 + 原因行」。体验上更好，记录为口径偏差，可不改。

## 3. 安全

- 合并动作只调 `commands.mergeSwarmItems`（DispatchOverlay.tsx:417），前端无任何 git 拼接。通过。
- 拆分提示词是静态文本（`stamp.ts:17-27`），不带项目上下文。通过。
- **上下文边界偏差（低）**：每件执行节点的 prompt 默认带拆分节点的整份回复——含**其它件**的 title/files（`context.ts:119-120` 共享节点保留全部祖先回复；确认页上下文勾选只对盲节点开放，DispatchOverlay.tsx:587，用户无法自行剔掉）。与 PRODUCT.md「各件只看根材料 + 自己那一件的清单条目」不符，也是上面会话续聊问题的上下文面。B 节任务书未要求做这层过滤，属留给下期的缺口而非本期 bug；建议下期对 swarm 节点默认只勾「根材料 + 自身 user.text」，或拆分回复入上下文时只留本件条目。

## 4. 代码质量与契约

- 类型只追加：`types.ts` 仅增 `SwarmItem`、`swarm?` 与注释，无删改；`tauri.ts` 蜂群段为纯新增。通过。
- 命令命名 camelCase 一致（`checkSwarmPlan`/`mergeSwarmItems` ↔ `check_swarm_plan`/`merge_swarm_items`），返回类型显式 `unknown` + 归一化，处理克制。请求侧命名问题见维度 2 第一条。
- 内置预设：Rust `builtin_presets()`（orchestration.rs:292-397）现有 5 个（shared/economy/blind/arena/jury），**无 builtin-swarm**；前端 `withSwarmPreset`（swarm.ts:53-57）兜底并在 A 侧补上后自动让位，方案成立。**由 A 侧做**：① `builtin_presets()` 加 `builtin-swarm`（role 拆分/共享/只读），并同步改 orchestration.rs:858-867 的顺序断言测试；② `fixtures/prompts/split.txt` 落地后与 `PRESET_NODE_TEXTS['builtin-swarm']` 合并（报告已自查出这两条，属实）。
- 其余小项：`swarm.ts` 纯函数分层干净、与 `dispatch.ts` 的既有模式一致；`SwarmEditor` 每次键入都触发 `updateNodeSwarm` → G1 effect 重跑 `checkSwarmPlan`（NodePanel.tsx:281 + DispatchOverlay.tsx:209），只读命令但会全项目 glob 遍历，建议对核对加 300ms 级防抖或仅按 files 字段变化触发。

## 5. 测试

报告所列 24 条用例与 `swarm-08b.test.tsx` 逐一对应，断言体确实钉住目标行为（最后围栏、截断+提示、gate 阻断/提示分色、件号排序、双命名归一、adopted 只写 merged、MANUAL 关键词）。「318 passed」的总数声明与 24 条新增一致，本审查未复跑。

最该补的 3 个（对应上面的发现）：

1. **合并请求体形状**：断言 `mergeSwarmItems` 发出的 items 字段含 `node_id`（或与 A 侧定稿的命名）、按件号排序、worktree/accept 取值正确——直接钉住命名歧义这个最高风险。
2. **防重复生成**：拆分节点已有带 `swarm` 的子节点时，`generateSwarmNodes`/按钮不再产生第二套件号（修复后写守卫的纯函数测试）。
3. **G1 门禁时序**：swarm 批次 + check 返回 `dirty:true`/overlaps → 确认禁用；check 在途与 catch 回退两种状态下确认按钮的行为有明确定义并测住（现在是未定义区）。

## 对照 B 节原始需求是否完整实现？

**是——B1–B6 六条全部有实现、有测试、后端未接通的回退可编译且行为符合契约；但有一处契约歧义和两处交互级缺口，建议在 A 侧接通前修掉第一条。** 缺项（按严重度）：

1. 合并请求 `nodeId` vs 契约文本 `node_id`，请求侧无双写兼容——A 侧按字面实现则合并静默全跳过（tauri.ts:559）。
2. 「按清单生成执行节点」可重复点击/跨拆分节点生成，件号重复破坏核对与合并顺序（DispatchOverlay.tsx:684）。
3. G3 合并入口绑定单次 overlay 会话，重开丢失、单件重派后清单缩水（DispatchOverlay.tsx:411）。
4. 擂台「选它」与仲裁区在蜂群批次上照常出现，可互相清 adopted（DispatchOverlay.tsx:673、705）。
5. G1 异步核对在途时确认不阻断，可绕过重叠/脏树门禁（DispatchOverlay.tsx:220）。
6. 每件 prompt 默认带整份清单（含他件范围），偏离 PRODUCT.md 的上下文边界（context.ts:119，下期处理可接受）。