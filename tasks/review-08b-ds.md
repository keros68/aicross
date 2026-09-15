读完了：`docs/PRODUCT.md` 第 3 期草案、`tasks/TASK-08-swarm.md`、`AGENTS.md`、`tasks/REPORT-08B.md`，以及本期待审的 `src-ui/src/graph/{types,swarm,dispatch,stamp,mutate,dispatch-settings,context}.ts`、`components/center/graph/{DispatchOverlay,DispatchBits,GraphTab}.tsx`、`components/right/orchestration/{NodePanel,OrchestrationPanel}.tsx`、`tauri.ts`、`docs/MANUAL.md`，并只读对照了 `src/orchestration.rs` 的 `builtin_presets` / `GraphNode` / `save_graph`。

说明：本环境没有 shell，没跑 `vitest` / `npm run build` / `cargo`；编译与测试结论来自读码 + 报告里的原文输出。

---

## 1. 需求完整性

- **B-2 半落地：「蜂群模式」进不了图上「盖预设」菜单。** `GraphTab.tsx:105`、`GraphTab.tsx:154` 两处 `listPresets()` 都没有套 `withSwarmPreset`（只有 `OrchestrationPanel.tsx:52,74` 套了）。A 侧 `src/orchestration.rs:292-...` 的 `builtin_presets()` 目前是 5 条、无 `builtin-swarm`，所以在前端兜底失效的那个入口上，用户看不到蜂群模式——图上的「盖预设」是盖预设的主入口，右栏是另一条。**改法**：两处也套 `withSwarmPreset`，或把兜底下沉到唯一的 preset 加载出口。
- **`swarm_limit` 只在解析时生效，不是全局上限。** `swarm.ts:99-101` 截断 ✓、`dispatch-settings.ts:40-42` clamp 1–4 ✓、`SettingsModal.tsx:626-633` 与并发上限同处 ✓；但 `stamp.ts:17-27` 的拆分提示词把「最多 4 件」写死（用户设 2 时模型仍出 4 件，多出的两件被静默丢掉，只有一行提示），而且生成按钮可以点多次、总件数不受限制（见 2.5）。**改法**：提示词从设置插值；生成前按「同 split 下已有 swarm 节点数」设限。
- **后端未接通回退：存在且形状对。** `swarm.ts:176` 的 `SWARM_CHECK_FALLBACK`、`swarm.ts:244-256` 的 `swarmMergeFallback`，调用点 `DispatchOverlay.tsx:217` 与 `:422-424` 都 `.catch` 到；`invoke` 在非 Tauri 下 reject（`tauri.ts:81-83`），B1 用例真跑过。符合 B-1 的「低风险回退」要求。
- **A 侧依赖已在报告写明**（`tasks/REPORT-08B.md`「没做的」「两处说明」）：`builtin-swarm`、`Node.swarm` 镜像、两条命令、`fill_role_defaults` 的「拆分」、`fixtures/prompts/split.txt`。**但漏了一条致命的**：见 4.1。
- 其余 4 条（解析+建件、G1、G3、手册）逐条对得上，见下。

## 2. 逻辑正确性

- **2.1 G1 在核对返回前不阻断。** `DispatchOverlay.tsx:123` 初值 `null` → `swarm.ts:192-196` `swarmGate(null)` 返回 `blocked:false` → `DispatchOverlay.tsx:609` 按钮可点。每次进确认屏（含关屏改完再进）到 `check_swarm_plan` 返回之间，重叠/脏树闸门是不存在的；这条命令要遍历项目文件，大仓库上这个窗口不是毫秒级。**改法**：核对状态做三态（pending/ok/error），pending 时 `blocked:true` 并显示「核对中」，别把「还没查」和「查不了」混成同一个 `null`。
- **2.2 判出重叠后现场改不了。** 闸门说「改到不重叠再派」（`swarm.ts:200`），但确认屏是 `position:fixed; inset:0; z-index:12000` 的全屏遮罩（`GraphTab.css:24`），NodePanel 的 `np-backdrop` 是 `z-index:100`（`OrchestrationPanel.css:174-181`），改 `files` 只能先关掉确认屏、回节点面板改、再重开。提示语也没给出路。B-4 的字面（NodePanel 有编辑框）满足，实际动线是断的。**改法**：路由卡内嵌 `SwarmEditor`（draft 时），或至少在原因行写「关闭本屏 → 节点面板改文件范围」。
- **2.3 G1 路由卡看不到件号与清单内容。** `DispatchOverlay.tsx:514-525` 的路由卡走 `routeLine`（agent/model/档位/thinking/可见性/写权限），没有 `第 N 件`、`files`、`accept`。原因行说「第 1 件与第 2 件都会改 src/a/x.ts」时，用户只能靠红框猜哪张是哪件；「清单确认」这个闸门实际上没法在屏上确认清单。**改法**：swarm 卡片加一行 `第 N 件 · files.join('、') · accept`。
- **2.4 终态判据只看本批 results。** `DispatchOverlay.tsx:411-412` 用 `swarmNodeIdsOf(graph, results.map(...))`，`swarm.ts:222-229` 只对这批 id 求终态；图上同属一个拆分、还停在 `draft` 的件既不在 items 里也不阻止合并（重复生成、分批生成后都会出现）。B-5 写的是「本阶段全部 swarm 节点终态」，**改法**：按拆分节点的 swarm 子节点全集算，或在契约里明确「按本批」，两种都行但不能靠实现悄悄选一种。
- **2.5 「按清单生成执行节点」不幂等。** `DispatchOverlay.tsx:685 → 397-409` 每次都新建节点，件号都从 1 起。连点两次（或对拆分节点先「重派」再生成）会得到两套同件号节点：G1 会报「第 1 件与第 1 件都会改 src/a/**」（`swarmGate` 对 a==b 的文案毫无意义），用户按提示改成不同 glob 就等于把两套都放行——同一改动被 cherry-pick 两次。**改法**：同 split 已有 swarm 子节点时按钮变「已生成 N 件」或二次确认「替换」；件号改成按图上的最大值续号。
- **2.6 合并动作没有一次性守卫，逐件结果不落图。** `DispatchOverlay.tsx:124` 的 `mergeResults` 是组件 state，`:701` 的按钮在合并完成后依旧 `disabled={!swarmReady || merging}` 为 false（节点仍是 done），可再点一次把同一批 worktree 再 cherry-pick；关掉结果屏再打开，逐件结果行全没了，只剩节点的 adopted 标记。**改法**：合并成功后落一个「本次 run 已合并」标记（或看节点 adopted 全 true 即禁用）；把 `commit`/`result` 记进图节点（契约需追加字段，归 A）或至少 persist 到本地。
- **2.7 蜂群卡片与擂台/采纳/重派三处打架。**
  - 「选它」：swarm 件彼此同 `kind`、同 `role`、同 `parents`，`sameParentsResultCount`（`dispatch.ts:430-444`）≥2 → 每张卡都出「选它」（`DispatchOverlay.tsx:674`）；`pickOne`（`dispatch.ts:410-428`）会把同组里 `adopted===true` 的兄弟全部清掉——合并后点一下，其它已 merged 的件 adopted 就被抹掉，和「merged 自动 adopted」直接冲突。
  - 「采纳」：未合并的件也能被标成已采纳（`DispatchOverlay.tsx:680-681`），与手册「并进去的件=已采纳」语义冲突。
  - 「重派」：对已 merged 的件可用，worktree 每次从 HEAD 新建而 HEAD 已含该件的 merge，重派再合并会重复应用/冲突。
  - 附带：「第三方仲裁」区对同阶段 ≥2 条 ok 结果无条件出现（`DispatchOverlay.tsx:707-761`），蜂群件之间也被当成「分歧」。
  **改法**：三处按 `node.swarm` 设限（至少 swarm 兄弟件不出「选它」、未合并不出「采纳」、已 adopted 不出「重派」）。
- **2.8 任何合并失败都被说成「引擎未接通」。** `DispatchOverlay.tsx:422-424` 的 `catch` 丢弃错误、只给 `SWARM_BACKEND_MISSING_NOTE`（`swarm.ts:242`）＝「本次未改动项目目录」；引擎在并到第 2 件时挂掉、或字段名不匹配（见 4.2）时这句话是**错的**。`normalizeSwarmMergeResults` 收到非数组返回 `[]`（`swarm.ts:262`），界面连一行都不出。**改法**：catch 把 error 文本带出来；非数组按「结果未知，请检查项目目录」处理，不要按「已跳过」。
- **2.9 解析细节。** `swarm.ts:63` 的 `/```json[^\n]*\n/` 会把 ` ```json5 ` / ` ```jsonc ` 也认成清单；`cleanStrings`（`:65-76`）不去重、不校验 glob 形态。都是小口子，按契约字面不算错，收一下围栏更稳。
- **2.10 与既有路径**：对账/回滚不冲突（worktree 节点 `rollbackBeforeOf` 返回 undefined，`dispatch.ts` 尾部），脏树提示与脏树阻断并存（重复但无害）。除 2.7 外未发现干扰。

## 3. 安全

- **前端不拼 git：未发现。** 蜂群只有 `tauri.ts:362-372` 两条契约命令；全 `src-ui` 无 `cherry-pick` / `git add` / `git merge` 调用，合并参数只有 worktree 路径与验收命令字符串，走 IPC。
- **清单文本进 prompt 是干净的**（`swarm.ts:117-121`：标题+范围+验收命令），执行节点也是「共享」而非「盲」，没有把项目其它材料塞进 user.text。
- **但每件都会收到别的件的清单。** 执行节点 `visibility='共享'`，其上下文包含拆分节点回复全文（`context.ts:120-133`），也就是整份 `items`（含其它件的 files/accept）。PRODUCT 的「不做」里明写「跨件共享上下文（各件只看根材料 + 自己那一件的清单条目）」，现在等于允许模型看到并顺手改别人的范围，而重叠闸门只按**声明**范围判。B 节字面没要求，但这是已定口径，属于实现与设计不一致。**改法**：执行节点用「指定上游 = 仅拆分节点的用户消息/清单条目」，或在 prompt 里只裁剪出本件那一条。

## 4. 代码质量与契约

- **4.1 类型只追加 ✓，但 `swarm` 目前存不下来（报告漏写）。** `types.ts:73-77,86`、`dispatch-settings.ts:6` 都是追加，图落盘路径没有白名单式重建，前端侧不丢字段。可 `src/orchestration.rs:43-59` 的 `GraphNode` 没有该字段，而 `save_graph` 是「`graph: Graph` 反序列化 → `write_json_atomic` 重新序列化」（`orchestration.rs:615-621, 509-516`）——**未知字段被静默丢弃**。A 侧落地前，每次 `commitGraph` 写盘都把 `swarm` 抹掉，重启后 `load_graph` 回来的节点没有 `swarm`：G1 闸门不触发、合并区不出现、NodePanel 的范围编辑消失，且没有任何报错。这是本期最该在报告里补的一条（A 侧加字段 + 往返测试）。
- **4.2 合并结果的命名两种都收，请求方向没有兼容层。** 契约 `merge_swarm_items` 的请求/响应字段写的是 `node_id` / `test_output`，`tauri.ts:558-577` 声明 camelCase、`invoke<unknown>` 再 normalize（`swarm.ts:261-280`）。响应方向两种都收 ✓；**请求方向**送的是 `nodeId`（`swarm.ts:236`），若 A 侧严格照契约写 snake_case，命令会直接 deserialize 失败，而界面只会显示「引擎未接通…未改动目录」（见 2.8）。**改法**：A 侧统一 camelCase（报告已提，最省事），否则请求项也做一次 key 兼容。报告里把这处标成「低风险假设」，实际风险在错误提示的误导性上，比假设本身更值得处理。
- **4.3 `roleDefaults` 没有显式的「拆分」分支**（`types.ts:129-132`），靠兜底返回「共享/只读」；B-3 写的是「补 拆分→共享/只读」。当前值正确、也有测试钉住，但以后兜底一动就静默改行为。**改法**：照字面加一行分支。
- **4.4 `SwarmPlan` 多返回 `dropped`**（`swarm.ts:24-27`）：契约是 `items | null`，属可接受超集，报告已说明。
- **4.5 `swarmPlanNotice` 拿 `plan.items.length` 当上限**（`swarm.ts:103-107`）：只在 `limit === items.length` 时成立，直接把 `limit` 传进来更直白。
- **4.6 预设计数**：前端兜底 + 后端清单以 id 让位（`swarm.ts:53-57`），逻辑对；但见 1.1 的入口不一致。
- **4.7 文案**：`未解析到清单`／`已跳过`／`冲突待裁决`／`蜂群件数上限` 都合规，无内部词、无 emoji。

## 5. 测试

- **报告里的 24 条真实存在**（`src-ui/src/graph/__tests__/swarm-08b.test.tsx`：B1 3 / B2 4 / B3 4 / B4 6 / B5 6 / B6 1），断言目标与描述一致，B1 的 reject+回退形状是真跑（`tauri.ts:81-83` 非 Tauri 必然 reject）。
- **弱项**：(a) B4 的 `swarmGate(null)` 断言「放行」正好把 2.1 的窗口期钉成预期行为；(b) 一条 DispatchOverlay 的接线都没有——「本批不含 swarm 就不调命令」「确认为 disabled」这类只有组件层测得到，本仓库已有 `renderToStaticMarkup` 的先例；(c) B6 只是 grep 手册里的词。
- **最该补 3 个**：
  1. 合并闸门按「该拆分下的全部 swarm 子节点」算：留一件 `draft` → `swarmMergeReady` 必须为 false（现在为 true，暴露 2.4）。
  2. 蜂群 × 擂台/采纳互斥：swarm 兄弟件不出「选它」，`pickOne` 不清掉其它已 merged 件的 adopted，已 adopted 的件不出「重派」（暴露 2.7）。
  3. 「按清单生成」幂等 + 上限随设置：同 split 再点不产生第二套件号；`swarm_limit=2` 时提示词与生成件数都为 2（暴露 1.2 / 2.5）。
  （次优：核对未返回前「确认派发」必须 disabled，暴露 2.1。）

---

## 对照 B 节原始需求是否完整实现？

**没有完整实现：主链路代码是通的，但有若干会在真实使用中立刻显形的洞，其中「swarm 字段在 A 落地前会被 save_graph 丢弃」和「G1 在核对返回前不拦」是最要命的两条。**

缺项（按严重度）：

1. `Node.swarm` 未持久化——A 侧加镜像字段前，`save_graph` 会静默丢掉该字段，重启后蜂群状态全丢；报告未记这一条。
2. G1 闸门窗口期不阻断：`swarmCheck` 为 `null` 即放行，`check_swarm_plan` 返回前可点「确认派发」。
3. 「按清单生成执行节点」不幂等：可生成同件号的两套节点，`swarm_limit` 只在解析时生效。
4. 合并可重复点、逐件结果不落图；且任何合并失败都被统一显示成「引擎未接通、未改动项目目录」（含引擎已并一半的情况）。
5. swarm 卡片的「选它／采纳／重派」与蜂群语义冲突：可清掉其它已 merged 件的 adopted、未合并即采纳、合并后再派再并。
6. 判出重叠后在确认屏里无法就地改 `files`（NodePanel 被遮罩挡住），提示也没给出路。
7. 「蜂群模式」在图上「盖预设」菜单里没有（`GraphTab.tsx:105,154` 未套 `withSwarmPreset`）。
8. 拆分提示词写死「最多 4 件」不随 `swarm_limit` 变；`roleDefaults('拆分')` 靠兜底而非显式分支。
9. 每件都收到整份清单（跨件共享上下文），与 PRODUCT「各件只看根材料 + 自己那一件的清单条目」相违。

（不属 B 节缺项、但 A 侧必须做：`builtin-swarm` 入 `builtin_presets()`、两条命令、`fill_role_defaults` 的「拆分」、`fixtures/prompts/split.txt`——报告已列。）