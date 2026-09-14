# TASK-05B 修正轮 R1 复审（19 条逐条核对）

## 逐条结论

| # | 结论 | 证据（文件:行）与理由 |
|---|---|---|
| 1 | **已核实修好** | `src/dispatch.rs:1202-1212`：主失败且配了备选时，`node.visibility != "盲" && node.write != "只读"` 直接返回主结果并填 `fallback_skipped = "可写节点"`（`:1209`），盲/只读才走 `run_fallback`；`run_fallback` 对盲节点先 `remove_dir_all(run_root/node_id)`（`:1223-1235`），重跑在隔离目录、引擎对盲强制 `write=只读`、不 snapshot → 不重复写项目。UI：`dispatch.ts:200-207` + `DispatchOverlay.tsx:388,510`。测试：`dispatch.rs:2807`（可写→1 条留痕、标跳过）、`:2848`（盲→双留痕、无跳过）。**保留**：`fallback_reruns_for_blind_node_and_marks_no_skip` 只用「不存在的 agent」，断言的是「重跑发生」而非单子要求的「备选重跑成功」（离线不可能造 ok，见 `:3140` 全是 ignored 用例）。 |
| 2 | **已核实修好** | `dispatch.ts:366-381`：rivals 同时要求 `other.kind === node.kind`、`other.target?.role === node.target?.role`、parents 集合相等且 `adopted===true`。测试 `presets-05b-r1.test.ts:97`（擂台候选 + 同父 `/advise` 顾问，不动顾问 adopted）。 |
| 3 | **已核实修好** | 盖章留空：TS `dispatch.ts:67-81`，两个调用点 `GraphTab.tsx:327`、`OrchestrationPanel.tsx:133`（无遗漏，`grep stamp(` 已确认仅这两处生产调用）；Rust `orchestration.rs:458-465` 同步跳过。删除：`mutate.ts:79-88`（存在 + draft + 无任何节点以它为父），按钮 `GraphTab.tsx:511` 条件与其一致。测试 `presets-05b-r1.test.ts:119-179`、`orchestration.rs:1098`。2–4 家可达：删 3→2、根上「分叉」加同层第 4 路。 |
| 4 | **已核实修好** | `NodePanel.tsx:318-334`（draft 显示 textarea，非 draft 只读展示，placeholder「自定义视角」；`mutate.ts:68-75` 写回）；`stamp.ts:11` 第三条视角文本为 `''`；仲裁节点 draft + 有 `user.text` → 面板可改。测试 `presets-05b-r1.test.ts:184`、`presets-05b.test.ts:75`（断言 `texts[2] === ''`）。 |
| 5 | **改法有问题（口径偏差）** | 数据链路通了：`orchestration.rs:257-259`（`#[serde(default)]`）、`premise.ts:18-19,41`、`SettingsModal.tsx:263-271,671-678`；测试 `orchestration.rs:945,968`、`presets-05b-r1.test.ts:200`。但设置页是**逗号分隔的路径文本框**，不是单子写的「勾选文件」（`@tauri-apps/plugin-dialog` 的 `open()` 在本仓库已有用法，见 `CenterPanel.tsx:1145`）。真机风险：相对路径 / 同名 basename 会在派发时让整个抽查节点失败——`dispatch.rs:1637-1650`「附件不存在：docs/x.md」「附件文件名重复」。 |
| 6 | **已核实修好** | `NodePanel.tsx:233-255`（TargetEditor 备选下拉，`patch`→`updateNodeTarget`，可清空）；`DispatchOverlay.tsx:425-441` G1 备选下拉 onChange 即时 `onCommit(updateNodeTarget(...))` 写回。测试 `presets-05b-r1.test.ts:217`。 |
| 7 | **已核实修好（一件水分）** | `dispatch.ts:433-472`：`visibility:'盲'`、`upstreams: undefined`、`write:'只读'`、`role:'仲裁'`、params=共同材料祖先；盲分支只拷 parents 的 user/附件（`context.ts:120-160`、`dispatch.rs:1570-1660`）；G1 「本次外发」走既有 `blind` 条件（`DispatchOverlay.tsx:450-456`）。中性描述 `dispatch.ts:424-431` 只有【甲方】【乙方】，不含 agent/model/立场词。**保留**：`presets-05b-r1.test.ts:254` 的「不含更好/更差/同意/反对/应该」只在短 fixture 正文下成立——描述里嵌的是双方回复原文，真机上大概率会命中词表；该断言给的是假保证。 |
| 8 | **已核实修好** | `premise.ts:68-86`：按 `last_graph_id` 读图、`last_checked_at = new Date()`（回写时刻）、不按标题；`SettingsModal.tsx:196-202` 调用并**不吞**写盘错误（`setPremiseNote`）。测试 `presets-05b-r1.test.ts:436-473`。 |
| 9 | **已核实修好** | 留痕 `dispatch.rs:1246-1256`（`infer_vendor` 按备选模型、`billing_source` 取备选 provider）；测试 `dispatch.rs:2889`；节点卡整行 `GraphTab.tsx:601` + `presets-05b.test.ts:187`；报告已改口「换通道不续旧会话，发完整 prompt」，「已切备选」进下游上下文/落盘的代价写在「不确定处」首条。 |
| 10 | **已核实修好** | `NodePanel.tsx:31`（ROLES 含仲裁）、`types.ts:111-116`（顾问/仲裁→盲/只读）、`dispatch.ts:475`（`/^\/advise(?:\s|$)/`）、台账写回失败提示 `SettingsModal.tsx:200`。测试 `presets-05b-r1.test.ts:476-492`。 |
| 11 | **已核实修好（弱化）** | `presets-05b.test.ts:167-172`（k3 进 models，真走 conflict）、中性描述改词表/标签、`R1-11d` 无共同祖先、live 重写。**保留**：单子要的「G1 把 fallback 下发到请求」只有纯函数测试 `presets-05b-r1.test.ts:217`，`buildRequest` 的调用点（`DispatchOverlay.tsx:133-135`）没有组件级断言——接线是读了代码才确认的。 |
| 12 | **已核实修好** | `stamp.ts:60`（`preset.builtin` 才取文本）+ 测试 `presets-05b-r1.test.ts:496`；仲裁区标题/说明 `DispatchOverlay.tsx:568-569`；报告计数 11 条与「增删路数」改口已落（`REPORT-05B.md` 测试段/没做什么段）。小偏差：标题用 `group.title`（角色名，如「第三方仲裁 · 执行」）而非字面「第 N 阶段」。 |
| 13 | **已核实修好** | `dispatch.ts:467`：`kind: materials.length >= 2 ? 'accept' : 'turn'`，不依赖 `normalizeNodeKinds`；测试 `presets-05b-r1.test.ts:329`（两材料祖先→accept，单祖先→turn）。 |
| 14 | **已核实修好** | `dispatch.ts:385-397`（parents 集合相同计数）+ `DispatchOverlay.tsx:522`（≥2 才出「选它」）；测试 `presets-05b-r1.test.ts:512`。同样只有 helper 级断言，JSX 条件未被组件测试覆盖。 |
| 15 | **已核实修好** | `premise.ts:81`（`outcome.slice(0,500)` 比较）、`:70-80` 未变则原对象返回；`SettingsModal.tsx:282-287` 建图只写 `last_graph_id`；测试 `presets-05b-r1.test.ts:461`（含 `toBe(premise)` 断言未重写）。 |
| 16 | **已核实修好（两条同义反复）** | `dispatch.rs:3451-3540`：`enforce_live_success` 提前到最前（`:3457-3461`），`body_of(node_id)` 按 id 取正文，标签甲方/乙方并断言不含 `pi/`、`codex/`、两模型名，第三方厂商与两边不同。**保留**：`assert_ne!(arb.vendor, first_vendor)` / `assert_eq!(arb.write, "只读")` 由测试 helper 写死（`dispatch.rs:3008-3014` 把 vendor 按 agent 映射、write 恒为只读），不可能失败；live 的中性描述用 `take(400)`，与产品 `disputeExcerpt`（200+省略号+转折句优先）不同形。 |
| 17 | **已核实修好** | `node_agents`（`dispatch.rs:350-358`）在挑选（`:503-505`）、登记（`:524-526`）、释放（`:556-560`）三处整集合记账；测试 `:2936`（A 的备选 agent = B 的主 agent，B 等 A 结束，用秒级 started/ended 比较，非 flaky）。worktree 显式降级标记「不采纳」有正当理由（第 1 条已把可写节点排除），单子允许。 |
| 18 | **已核实修好（口径）** | 实数核对无误：R1 新增 7 条（dispatch 4 + orchestration 3），182+7=189 passed / 10 ignored；05B 原 7 条 = orchestration 4（`:866,890,919,982`）+ dispatch 3（`:2704,2725,2768`）；报告正文确实没有写过「8 条」。 |
| 19 | **已核实修好** | a) `presets-05b-r1.test.ts:119-144` + `orchestration.rs:1098`；b) `presets-05b-r1.test.ts:386`（过 `serializeContext` 只含根材料+中性描述，断言不含双方 reply 正文，我按 `context.ts` 的祖先闭包推演确实成立）；c) `presets-05b-r1.test.ts:535`（`fallbackFrom` 的 validation_failed 过 `reducePipelineStep` 长修正轮，节点数 5、retries=[1]）。 |

## 特别核对项

- **第 1 条（可写不触发 / 盲只读走隔离）**：成立。可写（含 worktree）在 `run_one` 就被拦成终态；盲节点重跑前清隔离目录、引擎强制只读、无 snapshot/artifacts；只读共享节点重跑与主尝试同路径（本就不落产物），不会「重复写项目」。
- **第 2、14 条**：`pickOne` 清候选=同父集合相等 ∧ 同 kind ∧ 同 role ∧ 已被 adopted；按钮=同父结果数（含自身）≥2。两条都对。
- **第 3 条**：盖章三路确实留空（TS 与 Rust 双侧，两处调用点全覆盖）；删除只允许「存在 + draft + 无子节点」，UI 与纯函数守卫一致。
- **第 7、13 条**：`visibility='盲'`、`parents=共同材料祖先`、`kind` 按祖先数（≥2→accept）全部落实；中性描述模板层只有甲/乙方标签、无 agent/model/立场词（但词表断言只对短回复有效，见上）。
- **第 8、15 条**：按 `last_graph_id` 读图、`last_checked_at` 只在回写时写、比较 `slice(0,500)`——三点都在 `premise.ts:68-86` + `SettingsModal.tsx:196-202/282-287`。
- **第 11、16、19 条**：测试都真实存在（我逐条读过并核对了断言与实现是否一致）；`live_arena_and_arbitration` 用 `node_id` 取正文、不再假设 `results[0]`，受限环境在最前面就返回、不会「跑到仲裁段被跳过」。**但它不是「走 appendArbitration 建节点」**——Rust 侧只能手工复刻形态，报告里也是这么写的（合理，只是别当成跨语言一致性验证）。
- **回归扫描**：`dispatchableNodeIds`、`reducePipelineStep`、`applyDispatchResults`、`parseAcceptance` 的 PASS/NEEDS_FIX 路径本轮**未被改动**；`pickOne` 收窄只影响跨角色兄弟（`grep` 确认无其它调用点）；`presetWithRoleDefaults` 只对 builtin-arena 生效；`Target` 只追加 `fallback`（两侧 `#[serde(default)]`）；`DispatchRecord.attempt` 追加在结构体末位、旧记录解析不受影响（`:2291` 往返测试）。**未发现新引入的路径回归。**

## 未闭合 / 会误判的点（按严重度）

1. **（中）** 前提附件口径与真机路径：路径文本框而非文件选择器；相对路径、同名 basename 会让抽查节点整体失败（`SettingsModal.tsx:671`、`dispatch.rs:1637-1650`）。
2. **（中）** 备选重跑后的 session 归属未收口：`run_fallback` 清掉入站 session 并用备选的新 `session_id` 写回 Reply（`dispatch.rs:1260-1261`），而节点 target 仍是主目标；下游同目标「共享」子节点经 `continuationSession`（`dispatch.ts:127-137`）会把备选 session 交给主通道。05B 起存在，R1 未动、无测试。
3. **（中低）** 新 JSX 条件无组件级测试：`DispatchOverlay.tsx:388/510/522` 三条只在纯函数层断言，「G1 提示/结果行/选它按钮」是否真渲染没有被验证（第 1、11、14 条的「补测试」有一半落在纸面上）。
4. **（低）** `live_arena_and_arbitration` 里「第三方厂商不同」「仲裁只读」是同义反复断言，给不了信心（`dispatch.rs:3008-3014`）；且 live 的摘录口径与产品 `disputeExcerpt` 不一致。
5. **（低）** 主/备两次尝试的 `record_stdout` 写同一路径 `run_root/<node_id>.raw.txt`，>32KB 时主尝试的原始输出转存被备选覆盖（`dispatch.rs:1151-1155`）；UI 也只展示最后一个 `dispatch_file`。
6. **（低）** 备选留痕的 `vendor` 靠 `infer_vendor(model id)` 猜（`:1246-1256`），前端本来知道真实 vendor 却在下发时不传 → 如 `k3-256k` 之类备选留痕 vendor 为空。
7. **（低）** 仲裁标题用角色名而非「第 N 阶段」（`DispatchOverlay.tsx:568`），与报告口径字面不符。
8. **（低）** Rust 只读强制只覆盖 验收/验证（`dispatch.rs:428`），未含 仲裁；因盲节点被 `run_one_attempt` 强制 `write=只读` 而不构成真实缺口，但契约上「仲裁只读」仅靠前端 `validateNode` 提示。

## R1 之后 TASK-05B 是否可交付

**基本可交付**：19 条中 18 条已按单子落实（第 5 条为口径偏差、第 11/16 条为「测试没验到它声称的东西」），未发现会破坏第 2/3/4 期图与派发路径的回归；但建议至少把「前提附件改文件选择器（或强制绝对路径）」与「备选 session 归属」记入报告的不确定处/后续期次，并把第 3、4 条那几处同义反复或纸面断言在报告里降级表述，再交编排者复跑验证。