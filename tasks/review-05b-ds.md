# TASK-05B 盲审（DeepSeek deepseek-v4-flash via pi，只读，2026-09-14）

# 代码审查：phase/5b（TASK-05 B 节）

## 1. 需求完整性

**1.1 擂台「盖章时选 2–4 家」不可达，且 3 路默认是同一家**
`src/orchestration.rs:352-372`（3 个空目标节点）、`GraphTab.tsx:326` 与 `OrchestrationPanel.tsx:126`（盖章时套 `applyRoleDefaults`）、`DispatchOverlay.tsx:89-92`（`hasMissingTargets` 是整批阻断）+ `:684`（确认按钮禁用）。
问题：路径数固定 3；全仓没有删除节点的功能（只有 `PresetBuilder` 的 `removeNode`，图界面没有）；三路又都被「执行」角色默认填上同一个模型。所以既跑不了 2 家，也会在用户不改动的情况下把同一家跑三遍。
建议：盖章 builtin-arena 时跳过 `applyRoleDefaults`（留空，G1 本来就强制逐路选），并补一个节点删除按钮；做不到就把口径改成「固定 3 家」写进报告。

**1.2 评审团「自定义」视角不存在，任何节点的 `user.text` 都改不了**
`stamp.ts:7-13`（只有安全/性能/需求覆盖三条固定文本）、`NodePanel.tsx:31`+`TargetEditor`（只编辑 Target，不编辑 `user`）。
问题：需求列了四个视角含「自定义」；0.1 明确把视角写进 `user.text`，这个设计隐含可编辑，但界面没有入口。仲裁的「中性描述可改」也只在建节点前的一次 textarea（`DispatchOverlay.tsx:578-584`），建完就不能再改。
建议：节点面板加 `user.text` 编辑框，或盖章时弹小窗逐节点填视角。

**1.3 仲裁节点拿不到「盲」的引擎语义**
`dispatch.ts:388-393`（`visibility: '指定上游'`）、对照 `dispatch.rs:1244-1249`（`blind` 只认 `visibility=="盲"`）。
问题：B 节写「盲发给第三方」，0.1 写 `visibility=指定上游`，两处打架，实现照 0.1 字面做。后果：无隔离目录、无强制只读、无盲审前缀，`workdir` 就是项目目录，留痕 `isolation=全`；G1 也因 `blind=false`（`DispatchOverlay.tsx:362,463`）不显示「本次外发」清单，用户看不到第三方会收到什么。内容上确实是盲的（`context.ts:113-125` 指定上游只保留 `upstreams` 里的回复，且根材料的 `user` 文本总会带上）。
建议：改成 `visibility=盲`（隔离 + 只拷共同材料），或把「仲裁不做隔离」写进任务书/报告，别让审计看留痕时误判。

**1.4 前提抽查缺「用户勾选的原始附件」**
`premise.ts:27-45`（根材料只放前提原文）、`orchestration.rs:244-256`（`Premise` 无附件字段）。
问题：0.1 与 B 节都写了「根材料 = 前提原文 + 用户勾选的原始附件」，报告「没做什么」也承认，但这是需求明项，不是可选。
建议：台账加 `attachments: string[]`，`buildPremiseCheckGraph` 塞进 `root.user.attachments`——盲节点引擎本来就会把勾选材料拷进隔离目录，语义正好对。

**1.5 备选不能在节点上设置/持久化**
`DispatchOverlay.tsx:420-434`（只在本轮 G1 临时选）、`NodePanel.tsx` 的 `TargetEditor` 无 fallback 行；且 G1 的 `targets` 只在点「确认派发」时随 `markPending` 写回图（`DispatchOverlay.tsx:213`），关掉弹窗即丢。
问题：B 节写「节点可设备选 agent/model」，现在图里根本存不下备选（除了通过 G1 那一瞬）。
建议：`TargetEditor` 加一行备选下拉（`cloneTarget` 已深拷贝 `fallback`，无需改契约）。

**1.6 两处口径调整的评估**
- 不做分歧自动检测：**合理**。归类模型在设置页明确 disabled（`SettingsModal.tsx:625-632`，「尚未接入」），没有分类器就没有「分歧条目」可挂按钮，退到「每个 ≥2 路成功的阶段都给一组」是唯一可行替代。但文案要跟着改：现在标题写「分歧仲裁 · 第 1 阶段」（`DispatchOverlay.tsx:568`），两边结论一致时也在喊「分歧」，建议改「第三方仲裁 · 第 N 阶段」并写一句「系统不判定分歧，双方由用户选」。
- 擂台不在盖章时选家：**方向可接受**（G1 本就有逐路目标下拉，少一层弹窗），但落成了 §1.1 的三个缺口，等于这项能力没交付。

**1.7 节点卡只显示「已切备选」四个字**
`GraphTab.tsx:595` + `dispatch.ts:167-170`。需求要「已切备选：主 x → 备 y」；路由只存在于回复正文首行。建议：`fallbackNoticeOf` 返回整行内容直接渲染（它本来就取的就是那一行）。

## 2. 逻辑正确性

**2.1 `pickOne` 按「父集合相同」判候选，会误伤同父的不同角色节点**
`dispatch.ts:331-338`。
问题：判据是 `parents` 集合相等且 `adopted===true`。根下同时存在擂台候选 A（执行）与 `/advise` 顾问节点 B（顾问，也有「采纳」按钮 `DispatchOverlay.tsx:530`）时，两者 parents 都是 `[root]`，`pickOne(A)` 会把 B 的 `adopted` 清掉。`stamp` 第二批同父候选也会互相清（这个方向倒是合理的）。
建议：加上 `other.target?.role === node.target?.role && other.kind === node.kind`（或直接用 `recordStamp` 的批次号）。现有测试（`presets-05b.test.ts:113`）没覆盖这个边界。

**2.2 备选重跑对可写节点会重复写项目，主尝试的改动无处交代**
`dispatch.rs:1169-1221`（`run_one` 丢掉主尝试结果）、`:1453` 前后（`DispatchRecord` 里没有 diff 字段）。
问题：主尝试若 `write=单写者`，已经直接改了项目；备选再跑一次是叠加在上面。返回给界面的只有备选的 `artifacts`（且 `before_snapshot` 是主尝试之后的快照，diff 只含备选改动），用户看到「已切备选 + 这份 diff」，会以为采纳范围就这些。
建议：主尝试已产出 artifacts 时一并返回并在结果屏标一行「主尝试已改动项目，未回滚」；更稳的做法是 `write != 只读` 的节点默认不触发备选。

**2.3 worktree 节点的备选静默降级成「直接写项目」**
`dispatch.rs:1246-1258`（worktree 建立失败 → `uses_single_writer = true`）+ `:1054-1077`（`create_worktree` 对已存在路径必然失败）。
问题：主尝试已经 `git worktree add` 出 `run_root/<node>/wt`，备选再 add 同路径必失败 → 退回把产物直接写进项目目录，第一次的 worktree 目录还残留。降级只体现在 `artifacts.stdout_summary`（结果卡渲染的是 `artifacts.diff`，看不到这句）。
建议：备选前复用已有 worktree（或先 `git worktree remove` 再建），或禁止 worktree 节点走备选。

**2.4 备选留痕的 `billing_source` 是主目标的**
`dispatch.rs:1213`（清了 vendor）vs `:1453`（billing_source 直接 clone 主的）。
问题：留痕是 TASK-04 用量/计费对账依据，备选那条记录会写主目标的计费口径；vendor 又空着，前后不一致。
建议：一并清空，或让请求里带上备选的 vendor/billing_source。

**2.5 备选并非「同 prompt」**
`dispatch.rs:1287-1299` + `:1215`（清 session_id）。
问题：主分支若是续聊（共享 + session_id），outbound 只有 `current_message`；备选清空 session 后 `is_continuing=false` → 发完整 prompt。方向正确（换通道不能续旧会话），但报告写的「同 prompt」不准确，费用与暴露面要按全量算。
建议：报告改口径，一行说明即可。

**2.6 前提结论按标题回写，「第一条命中」+ `updated_at` 排序**
`SettingsModal.tsx:201`（`metas.find(m => m.title === premiseGraphTitle(premise))`）、`:207-212`（`last_checked_at` 取 `graph.updated_at`）、`orchestration.rs:651-653`（按 `updated_at` 倒序）。
问题：同一前提抽查两次就产生两张同名图，回写结果取决于排序；理论上存在读到旧图并把较新的 `last_result` 覆盖回去的路径（标题含 id 前 8 位，跨前提串号概率可忽略）。另外 `runPremiseCheck` 也会重名。
建议：台账存 `last_graph_id`，按 id 回读；`last_checked_at` 用回写时刻而不是 `graph.updated_at`。

**2.7 顾问不会被当成执行节点 —— 未发现**
`reducePipelineStep` 只认 `role==='验收'`（`dispatch.ts:551-556`）；后端验证只在 `role=='执行' && write!='只读'`（`dispatch.rs:1386-1391`）；后端拒绝清单只含 验收/验证（`dispatch.rs:400-403`），而顾问被 `parseAdvise` 强制只读。stamp 出的 parents/visibility/write 也正确（`stamp.ts:41-53`）。

**2.8 `PRESET_NODE_TEXTS` 按 `preset.id` 索引是弱键**
`stamp.ts:7,58-64`；`save_user_presets_in` 只过滤 `builtin`（`orchestration.rs:589-598`）。
问题：用户预设可以带 `id: "builtin-jury"` 且 `builtin:false` 落盘，盖章时它会按位把自己节点标成安全/性能/需求覆盖。
建议：取值时要求 `preset.builtin` 为真。

## 3. 安全

**3.1 前提抽查外发范围 —— 干净**
新图、根=前提原文、节点盲 + 隔离目录、`upstream_artifacts` 为空（`premise.ts:27-45`、`dispatch.rs:1244`、`prepare_outbound` 盲分支）。不带项目其它上下文。唯一缺口是附件未做（§1.4），也就是「用户想发什么」目前没有控制手段，只有前提原文这一条固定通道。

**3.2 仲裁外发的中性描述**
`dispatch.ts:345-355`。
问题：(a) 标签用 `【agent/model】` 而不是需求写的 A/B，等于把双方身份告诉第三方，引入厂商锚定；(b) `slice(0,200)` 无省略号、按字符硬切，可能正好切掉两边真正分歧的那句，让第三家对着半句站队。密钥面：正文出引擎已过 `safe_trace`，外发前还走 `redact` 与 `prepare_outbound`，**未发现新泄漏**。
建议：标签改「甲方/乙方」（路由只留在界面），截断加省略号并优先取含转折词的句子。

**3.3 备选重跑是否重复写项目文件 —— 是（见 2.2/2.3）**
写锁本身没问题：主尝试的 `ProjectWriteGuard` 在 `run_one_attempt` 返回时 Drop 释放（`dispatch.rs:646-657`），备选按同一 `cancel_key` 重新取锁，取消也能同时杀掉两次尝试。问题是单写者会重复写、worktree 会静默变成直接写项目。

**3.4 仲裁节点的 `workdir` 是项目目录**
`dispatch.rs:1244`（非盲 → `project.to_path_buf()`）。第三家 CLII 的 cwd 就是用户项目，能读共同材料之外的一切。按 1.3 的任一方案处理。

## 4. 代码质量与契约

**4.1 `Target` 只追加 `fallback`、双侧 `#[serde(default)]`、`cloneTarget` 深拷贝 —— 未发现**
`types.ts:44-52,110-116`；`orchestration.rs:116-160`。旧图缺字段照常解析（`orchestration.rs:904-908` 有断言）。

**4.2 `aicross-dispatch/1` 只追加 `attempt`，请求/结果镜像一致 —— 未发现**
`dispatch.rs:270-271,1474`；`tauri.ts:435-436,478-480`（camelCase 由 `#[serde(rename_all)]` 保证）。留痕文件名含 agent/model 且有去重后缀（`dispatch.rs:1758-1771`），主备两条不会互相覆盖。

**4.3 第 2/3/4 期路径 —— 未发现破坏，但有一处行为变更要记录**
`dispatchableNodeIds`/`nextStageStep`/`reducePipelineStep` 未动，验收 PASS/NEEDS_FIX 仍只认 `role==='验收'`。`applyDispatchResults` 现在保留旧 `adopted`（`dispatch.ts:190`）：等于把 `adopted` 从「本轮结果的属性」变成「节点的属性」，重派失败节点会带着采纳标记，通常无害但应写进报告。

**4.4 「已切备选」走回复正文标记行**
`dispatch.ts:182-184,167-170`。标记行会进下游模型上下文并随图落盘（`GraphTab.tsx:595` 靠它渲染）。本期契约只允许追加 `Target.fallback`，做法可理解，但 UI 元数据混进模型正文不是好味道；留痕/结果字段本来就有纯数据，只是没进图。
建议：至少在报告里点明代价（报告「不确定处」已提一半，没说它进下游上下文这句影响面）。

**4.5 新增角色没进两处清单**
`NodePanel.tsx:31` 的 `ROLES` 缺「仲裁」；`types.ts:119-124` 的 `roleDefaults` 没有 顾问/仲裁，因此 `NodePanel.tsx:148` 把角色改成「顾问」会顺手把 `visibility` 变回「共享」，与「顾问=单轮、盲、只读」相反。
建议：`ROLES` 加 仲裁；`roleDefaults` 补 `顾问→盲/只读`、`仲裁→指定上游/只读`。

**4.6 `parseAdvise` 前缀匹配过松**
`dispatch.ts:406` 用 `startsWith('/advise')`，`/advisefoo 你好` 会被当命令（question=`foo 你好`）。建议 `/^\/advise(?:\s|$)/`。

**4.7 设置页写回失败被吞**
`SettingsModal.tsx:216` `.catch(() => {})`。台账损坏时（`save_premises` 拒写，`orchestration.rs:419-431`）页面照样显示新结论，用户以为已保存。建议复用 `savePremiseFile` 的错误提示路径。

**4.8 TS 无 lint 阻塞**
`presets-05b.test.ts:5-7` 从同一模块 import 两次（`stamp.ts`）。纯风格，建议合并。

## 5. 测试

**5.1 存在性与断言目标**
报告写「10 条 vitest」，实际是 **11 条**（`presets-05b.test.ts:60/76/113/127/154/171/191/207/219/236/253`）；Rust 8 条对得上（`orchestration.rs:859,883,912,936` + `dispatch.rs:2660,2681,2724,3223`，最后一条 ignored）。但有 3 条断言打不到目标行为：
- `:166-168`「第三家厂商放行」用的 `models` 里没有 `k3`，`findModel` 返回 undefined → `arbitrationVendorConflict` 直接放行（`validate.ts:57-59`），断言空过。应把 `k3` 放进 `models`。
- `:139` `expect(text).not.toContain('更好')` 对任何文本都成立，中性描述实际没被测。应断言不含「更好/更差/同意/反对/应该」词表，并按需求口径断言标签是 A/B（现在是 `agent/model`）。
- `:171` 只测 `applyDispatchResults` 的字符串拼装，G1 把 fallback 下发到请求（`DispatchOverlay.tsx:135-141`）没有测。
未覆盖且值得测的纯函数分支：`appendArbitration` 的「没有共同材料祖先」（`dispatch.ts:373-375`）、`pickOne` 的同父异角色边界、前提结论回写的匹配逻辑（现在嵌在 `SettingsModal.tsx:197-217` 里，不可测）。

**5.2 `live_arena_and_arbitration`（`dispatch.rs:3222-3276`）构造偏差**
- 不是真实仲裁形态：手搓 `visibility="盲"`（真实现是 `指定上游`），没有 parents/共同材料，也没有断言第三方厂商与两边不同 → 跑过 ≠ 仲裁预设可用。
- `assert!(!dispatch_file.is_empty())` 是弱断言（失败路径也写留痕）；有效的只有 `status=="ok"`。
- `:3255-3258` 的 `if !enforce_live_success { return; }` 在受限环境下会跳过整段仲裁，这条测试实际只跑擂台。
- 不易误判的点：擂台两家共用同一 prompt/`current_message` 且 session 为空，走全量 prompt，与 G1 真实路径一致。

**5.3 最该补的 3 个**
1. `run_one` **主失败/备成功**的真跑测试（现在两条 Rust 测试都是「主备都构建失败」），并分别覆盖 `单写者`（主已改动项目）与 `盲`（隔离目录）两种模式的副作用。
2. 把 `SettingsModal.tsx:197-217` 抽成纯函数 `matchPremiseResults(premises, metas, loadGraph)` 并测：两次抽查同名图、`last_result` 未变、结论为空。
3. `pickOne` 边界测试（同父异角色、两批盖章、无 reply），因为它对应 §2.1 这个确定的功能性问题——先写失败用例再修。

## 对照 B 节是否完整实现

**一句话**：六个预设都落了地、图契约只追加了 `Target.fallback` 且 Rust/TS 镜像一致、第 2/3/4 期派发路径没被破坏，但「2–4 家」「视角自定义」「抽查附件」「节点上设备选」四处明确口径没实现，另有两处真问题（备选重复写项目、`pickOne` 误清 adopted）。

**缺项（按严重度）**
1. 备选重跑对可写节点重复写项目，主尝试改动无交代（`dispatch.rs:1169-1221,1246-1258`）。
2. `pickOne` 误清同父不同角色节点的 `adopted`（`dispatch.ts:331-338`，且无测试）。
3. 擂台「2–4 家」不可达（无节点删除 + G1 整批阻断 + 三路默认同模型）（`GraphTab.tsx:326`、`DispatchOverlay.tsx:89`）。
4. 评审团「自定义」视角与任何 `user.text` 编辑入口都不存在（`stamp.ts:7-13`、`NodePanel.tsx`）。
5. 前提抽查缺「用户勾选的原始附件」（`premise.ts:27-45`、`orchestration.rs:244-256`）。
6. 备选只能在本轮 G1 临时选，节点上存不下（`DispatchOverlay.tsx:420-434`、`NodePanel.tsx`）。
7. 前提结论按图标题回写、无 `last_graph_id`（`SettingsModal.tsx:201`）。
8. 仲裁身份标签用 `agent/model` 而非 A/B；节点卡只显示「已切备选」四字（`dispatch.ts:354`、`GraphTab.tsx:595`）。
9. 顾问/仲裁 未进 `roleDefaults`，`ROLES` 缺 仲裁（`types.ts:119-124`、`NodePanel.tsx:31`）。
10. 报告计数与措辞偏差：vitest 11 非 10；「增删路数走图界面」不成立（图界面删不了节点）。

---
usage: {"input": 147, "output": 4955, "cacheRead": 151168, "cacheWrite": 0, "reasoning": 46, "totalTokens": 156270, "cost": {"input": 2.0580000000000003e-05, "output": 0.0013874, "cacheRead": 0.0004232704, "cacheWrite": 0, "total": 0.0018312504000000001}}
