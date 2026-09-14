# TASK-05B 盲审（GLM-5.3-Flash via cc-switch，只读，2026-09-14）

# TASK-05B 代码审查

## 1. 需求完整性

**1.1 擂台：角色默认填充击穿了「G1 逐家选」的口径调整**
- `src/orchestration.rs:582`（`fill_role_defaults` 进 `presets_from`）+ `src-ui/src/components/center/graph/GraphTab.tsx:326`（`applyRoleDefaults`）+ `src-ui/src/graph/dispatch.ts:52-60`
- 问题：`builtin-arena` 的 3 个节点 role=执行、model 空。只要 roles.json 配了执行角色，Rust 侧 `fill_role_defaults` 和前端 `applyRoleDefaults` 会把 **同一个** 执行默认模型填进全部 3 个节点。REPORT 在「不确定处」承认了这点，但这不是可以放过的边角：盖章后 G1 里是三行一模一样的目标，用户很容易直接确认——派出去的「擂台」是同一家打三份，预设的核心用途（多家比拼）在默认路径上不成立。
- 建议：`fill_role_defaults` / `applyRoleDefaults` 跳过 `preset.id === 'builtin-arena'`（或跳过多路同角色执行层），让目标真的留空；再补一条测试钉住。

**1.2 评审团「自定义」视角没有入口**
- `src-ui/src/graph/stamp.ts:7-13`
- B 节写「安全 / 性能 / 需求覆盖 / 自定义」。三个内置视角进了 `user.text`，但盖章后没有任何 UI 能编辑已盖节点的 `user.text`（NodePanel 只编辑 Target，聊天列只会新挂节点）。「自定义视角」实际不可达。
- 建议：NodePanel 加一个只读/可编辑的「节点消息」区，或承认只做了三个固定视角并在任务书偏差里写明。

**1.3 前提抽查「勾选的原始附件」未做**
- `src-ui/src/graph/premise.ts:27-45`、`SettingsModal.tsx:278-294`
- 0.1 节明确「根材料 = 前提原文 + 用户勾选的原始附件」。实现只放原文。REPORT 以「schema 无附件字段」为由未做——但 0.1 的抽查定义和 schema 同段，应以功能定义为准，schema 少一个字段是可实现冲突而不是不做的理由。缺项，非口径调整。

**1.4 节点面板不能设备选**
- `src-ui/src/components/right/orchestration/NodePanel.tsx:70-242`
- B 节原文「主备/熔断：**节点可设备选** agent/model」。现在 fallback 只在 G1 确认屏可设（`DispatchOverlay.tsx:420-442`），TargetEditor 没有该字段。G1 设置会在 `markPending` 时写回图，功能等价可达，但「节点级配置」的字面要求只满足了一半，且不派发就关掉 overlay 时 G1 里的修改全部丢失（含 fallback）。
- 建议：TargetEditor 追加一行备选下拉（与 G1 同一套 options）。

**1.5 两处口径调整的评价**
- 不做分歧自动检测（REPORT「没做什么」第 2 条）：合理。归类模型本来就没接，给每个 ≥2 路成功的阶段一组仲裁编排区、由用户选双方，能力保留、误判为零，是正确的降级。
- 擂台不在盖章时选家：方向合理（G1 本就有每路目标下拉），但被 1.1 的角色默认填充抵消了大半——盖出来不是「空目标待选」而是「三份相同目标待改」。调整要成立，前提是先修 1.1。

## 2. 逻辑正确性

**2.1 仲裁中性描述带厂商名，破坏「盲」的意图**
- `src-ui/src/graph/dispatch.ts:354`（`【${routeOf(aId)}】认为：…`）、`src/dispatch.rs:3260`（live 测试同样写死厂商名）
- 0.1 节给的形状是「A 认为…；B 认为…」。实现把 `agent/model` 写进标签，仲裁者在裁决前就知道每个立场出自哪家厂商——对厂商的好恶本身就是立场，这正是仲裁要走盲节点 + 排除双方厂商的原因。文本没有褒贬词（「不带立场词」字面满足），但身份泄露让排厂商的意义打折。
- 建议：标签改「甲方/乙方」或 A/B；厂商路由留在结果屏给用户看即可。live 测试同步。

**2.2 `appendArbitration` 多材料祖先时 kind 与契约不一致**
- `src-ui/src/graph/dispatch.ts:384-385`
- `kind: 'turn'` 写死，但 `parents: [...materials]`——一旦共同材料祖先 ≥2（图里出现多个 material 节点时可能），就造出 `parents≥2 && kind='turn'` 的节点，下次加载被 `normalizeNodeKinds`（`layout.ts:111-131`）静默改成 `accept` 并 console.warn。同文件的 `appendDecision`（`dispatch.ts:238`）就按 parents 数定了 kind。
- 建议：`kind: materials.length >= 2 ? 'accept' : 'turn'`，或只取根材料一个。

**2.3 「选它」清同父候选：纯函数无误伤，按钮可见性比语义宽**
- `src-ui/src/graph/dispatch.ts:328-341`、`DispatchOverlay.tsx:518`
- `pickOne` 只清「parents 集合相同且 `adopted===true`」的节点：不同分支的已采纳节点不受影响，两个擂台盖在同一父上时互为候选也正确。无误伤。
- 但按钮条件是 `group.rows.length >= 2`（同阶段结果数），不是同父。一次运行把不同分支的待派节点收进同一 stage 时（`dispatchableNodeIds` 是全图收集），非竞争关系的卡片之间也会出现「选它」，点了会把另一分支的采纳清掉。建议条件加「与该节点同父的结果 ≥2」。

**2.4 备选重跑：隔离目录、锁、留痕基本正确，两处账记不上**
- `src/dispatch.rs:1192-1219`：盲目录清掉重铺 ✓；`retry.fallback=None` 防套娃 ✓；`session_id` 清空 ✓；主备各一条留痕（`attempt: 主/备`）✓。
- 问题 a（`dispatch.rs:1244-1258` + REPORT 已认）：worktree 主尝试失败后备选降级单写者，**直接写项目目录**——用户为本意是 worktree 隔离的节点拿到了项目内写入，且降级只体现在 `stdout_summary` 尾巴里。至少应在 `fallbackFrom/fallbackTo` 之外给结果一个显式降级标记供界面展示。
- 问题 b（`dispatch.rs:1262-1280` + `run_one:1178-1185`）：单写者/共享节点的主尝试可能已经写了部分文件再 error/timeout；备选尝试的 `before_snapshot` 把这些文件当基线，其 diff 永远不含它们，主尝试的结果对象（含 artifacts）被 `run_one` 整个丢弃。失败主尝试的项目内残留写入在两条留痕和结果屏里都无账可查。低频但属于「重复写项目文件」审查点的实质缺口。
- 问题 c（小）：`dispatch_stage_core` 的 `active_agents` 调度（`dispatch.rs:484-488`）只按主 agent 串行；备选 agent 若恰有另一排队节点，会与本次备选尝试并发，同 agent 串行约束对备选路径失效。

**2.5 「已切备选」标记行进下游上下文**
- `src-ui/src/graph/dispatch.ts:182-184, 165-170`
- `（已切备选：主 x → 备 y）` 拼进 `reply.text` 首行，随 `serializeContext` 进入所有子节点的 prompt。REPORT 已列为不确定项。这是拿正文当信道，属于已知最简但最脏的做法；一旦某模型回正文恰好以「（已切备选：」开头，`fallbackNoticeOf` 还会误报。建议下期给 Reply 加字段（`aicross` 图契约允许追加），本期至少在 `fallbackNoticeOf` 上没有误报风险的场合接受现状。

**2.6 前提抽查按图标题匹配写回：不会串到别的 premise，但有一处自比较 bug**
- `SettingsModal.tsx:199-215`
- 标题含 `#<uuid前8>`，uuid 碰撞可忽略；文本不可编辑，标题稳定；`list_graphs` 按 `updated_at` 降序（`orchestration.rs:653`），同 premise 重复抽查时 `metas.find` 取最新一张。跨 premise 串写不成立。
- bug：`outcome !== premise.last_result` 用**全文**比较，存储的却是 `outcome.slice(0,500)`（208-212 行）。结论超过 500 字时每次打开设置页都判定「有变化」，重写一次 `premises.json` 并把 `last_checked_at` 刷成图更新时间。建议比较 `outcome.slice(0,500)`。
- 另：`runPremiseCheck`（286-291 行）在**建图时**就写 `last_checked_at`，此后若从未派发，「最近抽查」列显示的是一个没发生过的抽查日期。建议建图时不写，或改列名语义。

**2.7 顾问节点在推进循环里：不会被当成执行节点**
- `reducePipelineStep`（`dispatch.ts:557-607`）只对 `validation_failed`（需 role=执行 且 write≠只读）和 role=验收 的结果分叉；顾问/仲裁节点 只读、role 不匹配，走「无验收结果 → continueRunning=true」的正常通道，只派发不触发修正轮。`promptForRole('顾问')` 把模板拼在上下文之后（`dispatch.ts:317-322`），与「只要方案不要执行」的语序一致。无问题。

**2.8 stamp 的 parents/visibility/write：正确**
- 擂台/评审团都从同一 `prevLevel=[nodeId]` 挂 parents，visibility/write 来自预设 Target 原样落盘；`PRESET_NODE_TEXTS` 按 `created` 顺序（单阶段内即书写顺序）写 `user.text`，与 `serializeContext` 的 `[用户]` 块拼接（`context.ts:205, 213-215`：盲前缀在前、视角文本在后）吻合。仲裁节点上下文经 `contextPartsOf` 验证：parents=[材料祖先] ⇒ 闭包不含分歧双方节点，外发 = 根材料 + 自身 `user.text`（中性描述），无多余内容。

## 3. 安全

**3.1 前提抽查外发范围：符合**
- 新图只有根材料（前提原文）+ 盲节点冻结模板；外发经既有 redact 管道。「勾选附件」未实现反而是当前无超范围外发的原因（见 1.3，功能补上时要守住这个边界）。

**3.2 仲裁外发：不泄密钥，但泄露双方身份（见 2.1）**
- 外发内容 = 根材料 + 双方回复各 200 字摘录 + 裁决指令。双方回复在留痕侧已过 `safe_trace`，外发侧走 redact，无密钥通道。厂商名入文是唯一越界项。

**3.3 备选重跑重复写项目文件：见 2.4b**——失败主尝试的项目内残留写入无账可查；worktree 降级单写者把预期隔离的写入落进项目目录。除此之外备选与主尝试同 prompt 同可见性，无额外外发。

## 4. 代码质量与契约

- `Target.fallback` 镜像：`types.ts:42-46,59` ↔ `orchestration.rs:121-131,158-160`，双侧 `#[serde(default)]`/可省，`cloneTarget` 深拷贝 ✓，往返测试双侧都有 ✓。`DispatchNodeRequest.fallback`（`FallbackSpec.provider` 用 `String` 而非 `Option`，与 TS 发送的 `provider ?? ''` 一致）✓。
- `aicross-dispatch/1`：仅追加 `attempt`（`dispatch.rs:268-270`，`#[serde(default)]`）与 `fallbackFrom/fallbackTo`（196-199），不改名不删 ✓。
- 既有路径回归：`dispatchableNodeIds`/`nextStageStep`/`reducePipelineStep` 未动；`applyDispatchResults` 仅前置 notice，验收判定 `parseAcceptance` 吃的是后端 `result.text`（未加 notice），PASS/NEEDS_FIX 路径不受影响 ✓。`validateNode` 新增仲裁只读提示未改既有文案 ✓。
- 注册：`server.rs:3856-3857` 挂了 `load_premises/save_premises`，`tauri.ts:343-346` 类型镜像 ✓。
- 小项：`dispatch.ts:260` 用 `Target & { validation_command?: string }` 断言取一个 TS 类型里不存在的字段——TS 侧 `Target` 本就没有 `validation_command`，这是从 Rust 侧漏过来的幻影字段，属于旧账不是本期引入，顺带记录。
- 未发现其它契约问题。

## 5. 测试

**5.1 存在性与断言质量**
- TS 10 条：逐一核对 `presets-05b.test.ts`，与 REPORT 清单一致，断言打在目标行为上（擂台节点数/可见性/写/parents、评审团视角文本按序、pickOne 记账与原图不变、仲裁字段与拦厂商、fallback 标记行、JSON 往返+深拷贝、routeLine、parseAdvise、validateNode、前提图构建与回读）。`expect(text).not.toContain('更好')` 偏弱但可接受。
- Rust：REPORT 列的是 **7** 条新测试（orchestration 4 + dispatch 3），任务问的「8 条」对不上 REPORT 也对不上代码——`builtin_arena_and_jury_shapes`、`target_fallback_json_roundtrip`、`premises_roundtrip`、`premises_broken_…`、`fallback_predicate_…`、`fallback_reruns_once_…`、`fallback_blind_attempt_…`，共 7。`fallback_reruns_once` 断言了主备各一条留痕 + 路由 + frontmatter，扎实。

**5.2 `live_arena_and_arbitration` 构造有一处会误判**
- `src/dispatch.rs:3258-3263`
- `dispatch_stage_core` 的 results 按完成顺序 push（`dispatch.rs:550`），两家谁先完成不确定；dispute 文本却把 `results[0]` 硬标成 `【pi/glm-5.3-flash】`。codex 先完成时标签张冠李戴，仲裁正文与真实立场错配——真机验收会「成功」但验的不是设计的对照组。建议按 `node_id` 映射取正文再拼。其余构造（共享两家、盲仲裁、`enforce_live_success` 门）与既有 live 测试同型，不会误判。

**5.3 最该补的 3 个**
1. **擂台角色填充**：roles 配置下 `applyRoleDefaults(builtin-arena 各节点)` 断言目标为空——现在写出来直接红，正好钉住 1.1。
2. **仲裁节点上下文隔离**：`appendArbitration` 产物过 `serializeContext`，断言 prompt 只含根材料与中性描述、不含双方 reply——钉住 parents/upstreams 语义，防止 `contextPartsOf` 将来变动把分歧双方带进外发。
3. **fallback 与验收/修正轮交互**：`fallbackFrom` 存在的 `validation_failed`/验收 NEEDS_FIX 结果过 `reducePipelineStep`，断言修正轮照常生长——备选重跑与 3/4 期流水线的交界目前零覆盖。

---

## 对照 B 节原始需求是否完整实现？

**一句话：六个预设全部有可用落地且契约克制（图契约仅追加 `Target.fallback`），但擂台的默认路径被角色填充破坏、仲裁描述泄露双方厂商身份、前提抽查少了附件项——主干完整，三处需要返工，其余为已披露的可接受偏差。**

缺项列表（按严重度）：
1. 擂台 3 节点被角色默认填成同一模型，「逐家选 2–4 家」名存实亡（1.1）。
2. 仲裁中性描述用厂商名而非 A/B，违背盲仲裁意图（2.1；live 测试同病，5.2）。
3. 前提抽查「勾选的原始附件」未做（1.3）。
4. 备选重跑：worktree 降级单写者直写项目、失败主尝试的残留写入无账（2.4a/b）。
5. 评审团「自定义」视角无编辑入口（1.2）。
6. 节点面板不能设备选，fallback 只在 G1 可设（1.4）。
7. 前提回读全文 vs 截断比较导致重复写台账；建图即写「最近抽查」日期（2.6）。
8. `appendArbitration` 多材料祖先时 kind/parents 不一致，靠 normalize 掩盖（2.2）。
9. 「选它」按钮可见性宽于同父语义（2.3）；节点卡「已切备选」缺主备路由（相对 0.1 的字面要求）。
---
usage: [usage] provider=Zhipu GLM model=GLM-5.3-Flash input=1967910 fresh=116710 cache_create=0 cache_read=1851200 output=21280 turns=40 ms=829894
