# REPORT-08B — 蜂群 · 界面（TS）

对应 tasks/TASK-08-swarm.md 的「契约」与「B. 界面（TS）」两节，1→6 逐条做。
工作树 D:\work\aicross-p5b，分支 phase/8b。只改了 src-ui/、docs/MANUAL.md、
tasks/；src/（Rust）一行没动。未 commit、未 push、未起窗口、未发真机请求。

## 逐条

### 1. types.ts 的 Node.swarm? + tauri.ts 两条命令

**已做**

- src-ui/src/graph/types.ts:73-77 新增 SwarmItem { item, files, accept }，
  GraphNode 加可选 swarm?: SwarmItem（types.ts:86）。字段名与契约一致；
  Rust GraphNode 是 snake_case，但这三个字段无大小写歧义。
- src-ui/src/graph/types.ts:129-130 在 roleDefaults 上补注释说明「拆分」走
  兜底分支的「共享 / 只读」（原有兜底已经正好是这个值，没加冗余分支）。
- src-ui/src/tauri.ts:359-374：checkSwarmPlan(projectDir, items) 与
  mergeSwarmItems(projectDir, runId, items, validateCommand)。
  src-ui/src/tauri.ts:535-577：SwarmCheckItem / SwarmOverlap / SwarmUnmatched /
  SwarmPlanCheck / SwarmMergeRequestItem / SwarmMergeOutcome / SwarmMergeResult。
- 回退分支在 src-ui/src/graph/swarm.ts：SWARM_CHECK_FALLBACK
  （{ dirty:false, overlaps:[], unmatched:[] }，swarm.ts:176）与
  swarmMergeFallback(items)（全 skipped + 一条 note，swarm.ts:244）。
  后端未接通时 invoke 抛 "Tauri IPC not available"，两个调用点 .catch 到回退，
  界面照常编译、照常渲染、不阻断。

**已补测试**（src-ui/src/graph/__tests__/swarm-08b.test.tsx）

- B1 swarm 字段跟着节点走过一次盖章与一次写回
- B1 没有 swarm 字段的节点不被凭空补上
- B1 后端未接通时两条命令 reject，回退值就是契约里那两个形状

**一处按低风险假设定的取舍（请 A 侧对一眼）**：任务书契约把合并结果写成
{ node_id, …, test_output }（snake_case），而 tauri.ts 里其它派发类结果
（DispatchNodeResult 等）都是 camelCase、Rust 侧对应 serde rename_all camelCase。
两边并行做，无法当场确认引擎选哪种。处理：tauri.ts 的 SwarmMergeResult 声明
camelCase，mergeSwarmItems 的返回类型标 unknown，进界面前统一过
normalizeSwarmMergeResults（swarm.ts:261）——node_id/nodeId、
test_output/testOutput 两种写法都收，未知 result 值按 skipped 处理。
A 侧无论选哪种命名，前端都不用改。

### 2. 内置预设 builtin-swarm（界面名「蜂群模式」）

**已做**

- src-ui/src/graph/swarm.ts:33-57：BUILTIN_SWARM_PRESET（一个阶段一个节点，
  role=拆分、visibility=共享、write=只读、目标留空）与 withSwarmPreset(presets)
  （引擎返回的清单里没有同 id 时前端补一份，有则让位）。
- src-ui/src/components/right/orchestration/OrchestrationPanel.tsx:20 引入，
  两个 listPresets() 调用点（:52、:74）都套上；:27 把「蜂群」从 FUTURE_PRESETS
  （置灰的后续期次）里挪走。
- src-ui/src/graph/dispatch.ts:65-66：applyRoleDefaults 把「拆分」映到
  roles.planning（「出方案」的默认目标），presetWithRoleDefaults 随之生效。
- src-ui/src/components/right/orchestration/NodePanel.tsx:32：ROLES 加「拆分」。
- src-ui/src/graph/stamp.ts:8-27：PRESET_NODE_TEXTS['builtin-swarm']，拆分节点
  的节点消息（只读不改文件 / 每件给标题+glob+验收命令 / 范围不重叠 / 有依赖的
  合成一件 / 不超过 4 件 / 正文附 json 围栏与字段示例）。
- 件数上限：src-ui/src/graph/dispatch-settings.ts:6,14,40-42 的
  UiDispatchSettings.swarmLimit（默认 4，读盘 clamp 到 1–4）；设置页在
  「并发上限」旁边加「蜂群件数上限」下拉
  （src-ui/src/components/common/SettingsModal.tsx:626-633），与契约的
  「与并发上限同一处」一致。

**已补测试**

- B2 盖在根材料上长出一个只读、共享的拆分节点
- B2 拆分角色的默认是共享 / 只读，目标按「出方案」填
- B2 引擎没有同 id 预设时前端兜一份，有则让位
- B2 件数上限与并发上限同一处设置，默认 4

**两处说明**

- 内置预设的权威清单在引擎侧 src/orchestration.rs 的 builtin_presets()，本工作树
  不许动 Rust。所以前端用 withSwarmPreset 兜底：A 侧把 builtin-swarm 加进
  builtin_presets() 之后兜底自动失效，不会出现两条同名预设。
- 拆分角色的提示词模板 fixtures/prompts/split.txt 是 A-4 的活，且 fixtures/ 在本次
  允许改动的范围外（硬性约束只给了 src-ui/、docs/MANUAL.md、tasks/）。TS 的
  ROLE_PROMPT_TEMPLATES 用 ?raw 静态 import，引一个还不存在的文件会直接编译不过。
  所以拆分要求先走 PRESET_NODE_TEXTS（与评审团同一条路子，写进节点 user.text），
  不碰 promptForRole。**待 A 侧 split.txt 落地后，两处文本要合并成一处**，
  否则会重复提示一遍。

### 3. 清单解析 + 按清单生成执行节点

**已做**

- src-ui/src/graph/swarm.ts:78-101 parseSwarmPlan(text, limit = 4)：取正文里
  **最后一个** json 围栏；JSON.parse 失败 → null；items 非数组 / 为空 / 任一件
  title 空 / files 不是非空字符串数组 / accept 非字符串 → null；超 limit 截断。
  返回 { items, dropped }（契约写的是 items | null，这里多带一个 dropped 才能给出
  「截断并提示」那句话，null 语义不变）。swarmPlanNotice(plan)（swarm.ts:103）
  出提示文案。
- 解析失败口径：SWARM_PARSE_FAILED_TEXT = 未解析到清单（swarm.ts:61）；拆分节点
  的回复照常按普通回复展示，只在按钮下面加这一行。
- src-ui/src/graph/swarm.ts:133-170 appendSwarmNodes(graph, splitNodeId, items, execution?)：
  每件一个 draft 节点，parents=[拆分节点]、kind=turn、role=执行、write=worktree、
  visibility 取执行角色默认（共享）、目标按传入的执行角色默认填（没传就留空，
  G1 再选）、user.text = 标题 + 文件范围 + 验收命令（swarmNodeText，swarm.ts:117）、
  swarm = { item: 序号从 1 起, files, accept }。
- 界面入口：src-ui/src/components/center/graph/DispatchOverlay.tsx:396-409 的
  generateSwarmNodes；按钮挂在 role=拆分 且本次结果 ok 的结果卡上
  （DispatchOverlay.tsx:684-686），反馈行在 :688。执行角色默认由本屏
  commands.loadRoles() 读（DispatchOverlay.tsx:128-134，同 OrchestrationPanel 的做法）。

**已补测试**

- B3 取最后一个 json 围栏
- B3 缺围栏 / JSON 坏 / 字段不合格都返回 null
- B3 accept 可缺省，超上限截断并给一句提示
- B3 每件建一个 draft 执行节点：worktree、件号、清单文本

### 4. G1：重叠标红 + 阻断 + NodePanel 编辑框

**已做**

- src-ui/src/graph/swarm.ts:178-215：swarmNodeIdsOf / swarmCheckItems（按件号排序，
  只取带 swarm 的节点）、swarmGate(check)、swarmOverlapItems(check)。
  blocked = dirty || overlaps.length > 0；reasons 是阻断原因（脏树用
  SWARM_DIRTY_TEXT「项目目录有未提交改动，蜂群合并要求干净」，重叠逐对点名两件与
  撞上的路径）；unmatched 只进 hints，不阻断。契约写的返回形状是
  { blocked, reasons }——这里是它的超集，多一个 hints 才能把「只提示」的那类和
  阻断项分开渲染（常规提示不用红色）。
- src-ui/src/components/center/graph/DispatchOverlay.tsx:205-223：本批含 swarm 节点
  且有项目目录时才调 checkSwarmPlan，只读；.catch 到 SWARM_CHECK_FALLBACK。
  effect 依赖里有 graph，用户在节点面板改完 files 会自动重核对，重叠消失即解禁。
- 标红：DispatchOverlay.tsx:514 给涉及重叠的路由卡加 swarm-overlap，样式在
  src-ui/src/components/center/graph/GraphTab.css:34（描红边，沿用 --red）。
- 阻断：DispatchOverlay.tsx:609 的「确认派发」加上 swarmGateState.blocked。
- 提示区：SwarmGateNotice（src-ui/src/components/center/graph/DispatchBits.tsx:85-97），
  阻断项走 dispatch-warning，unmatched 走常规灰字 dispatch-reason；挂载点
  DispatchOverlay.tsx:478。
- 编辑框：SwarmEditor（src-ui/src/components/right/orchestration/NodePanel.tsx:275-317），
  文件范围一行一条 glob（原文留在组件局部状态里，写回图时才拆行去空，否则受控
  textarea 打不出换行），验收命令一个输入框，下面一行标第几件。draft 才可编辑，
  非 draft 只读。写回走 updateNodeSwarm（src-ui/src/graph/mutate.ts:77-88，节点上
  没有 swarm 时不凭空补）。挂载点 NodePanel.tsx:381-388。

**已补测试**

- B4 swarmGate：干净且不重叠放行
- B4 swarmGate：重叠阻断并点名两件与撞上的路径
- B4 swarmGate：脏树阻断，glob 没匹配只提示
- B4 送给引擎的核对项按件号排序、只取带 swarm 的节点
- B4 SwarmGateNotice：阻断走警告样式，提示走常规灰字
- B4 SwarmEditor：draft 给编辑框，非 draft 只读

### 5. G3：合并按钮 + 逐件结果行 + merged 自动 adopted

**已做**

- src-ui/src/graph/swarm.ts:219-298：swarmMergeReady(graph, nodeIds)（全部 swarm
  节点 done/failed 才为真）、swarmMergeItems（按件号排序，worktree 取
  reply.artifacts.worktree，accept 取节点的清单条目）、swarmMergeFallback、
  normalizeSwarmMergeResults、swarmResultLine（已并入 + commit / 冲突待裁决 + note
  （引擎在 note 里放 worktree 路径）/ 测试红 + note / 已跳过 + note）、
  applySwarmMerge（只把 merged 的节点写 adopted=true，冲突与测试红一律不动，
  也不自动重派）。
- 界面：DispatchOverlay.tsx:411-427 的 runSwarmMerge（调 mergeSwarmItems，
  validateCommand 取设置里的验证命令；catch 到全 skipped 回退）；结果屏的
  「蜂群合并」区在 DispatchOverlay.tsx:694-704（本批有 swarm 节点才出，没跑完时
  按钮禁用并说明原因）；逐件行组件 SwarmMergeRow（DispatchBits.tsx:102-113，
  merged 走灰字、其它走警告色，验收输出折在行下）。

**已补测试**

- B5 全部 swarm 节点终态才出合并按钮
- B5 送给引擎的件按件号排序，带 worktree 与本件验收命令
- B5 结果行：已并入 / 冲突待裁决 / 测试红 / 已跳过
- B5 只有 merged 的节点自动 adopted，冲突与测试红不动
- B5 引擎返回 node_id / test_output 或 camelCase 都收得住
- B5 SwarmMergeRow：并入走灰字，其它走警告，验收输出折起来

### 6. docs/MANUAL.md「蜂群模式」一节

**已做**：docs/MANUAL.md 在「.dispatch/ 是什么」之前插入「## 蜂群模式」，写了
怎么盖（右栏选预设盖在根材料上，拆分是只读、用「出方案」默认模型）、清单长什么样
（json 围栏的字段示例、glob、accept 可空、上限 4 与设置项位置、解析不出时的表现）、
派发前的核对（重叠描红阻断、脏树阻断、glob 没匹配只提示）、合并规则（按件号
commit → cherry-pick → 跑验收 → 一件一个 commit → 自动采纳）、冲突怎么办
（abort 留 worktree、验证红 reset 回上一件、单件失败不牵连、不自动重派）。

**已补测试**：B6 docs/MANUAL.md 有「蜂群模式」一节，写清怎么盖、清单、合并、冲突

## 没做的

- fixtures/prompts/split.txt：A-4 的活，且 fixtures/ 在本次允许改动范围外。临时用
  PRESET_NODE_TEXTS['builtin-swarm'] 顶上（见第 2 条的说明）。
- src/（Rust）一行没动：Node.swarm 镜像、两条命令的实现、fill_role_defaults 的
  「拆分」分支、server.rs 注册都在 A 侧。A 侧接通之前，前端两条命令会 reject 并走
  回退分支——G1 不阻断；G3 的合并结果全部显示「已跳过 · 引擎未接通蜂群合并，
  本次未改动项目目录」。
- 没起窗口、没发真机请求、没 commit、没 push；临时文件只写过 %TEMP%。

## 验证

两条命令都实际跑过，输出原文：

    $ cd src-ui && npx vitest run --configLoader native --pool threads

     RUN  v5.0.0 D:/work/aicross-p5b/src-ui


     Test Files  24 passed (24)
          Tests  318 passed (318)
       Start at  08:51:40
       Duration  596ms (transform 67%, import 16%, tests 14%, worker 2%)

      Transform  transforming modules took 3.28s · 67% of tracked time, re-done on every run
                 persist transforms across runs with fsModuleCache: true
                 learn more: https://vitest.dev/guide/improving-performance#caching-between-reruns

本期新增的 24 条用例（同一命令加 --reporter=verbose，全绿）：

     ✓ B1 契约：Node.swarm 与两条命令 > B1 swarm 字段跟着节点走过一次盖章与一次写回 2ms
     ✓ B1 契约：Node.swarm 与两条命令 > B1 没有 swarm 字段的节点不被凭空补上 0ms
     ✓ B1 契约：Node.swarm 与两条命令 > B1 后端未接通时两条命令 reject，回退值就是契约里那两个形状 1ms
     ✓ B2 内置预设「蜂群模式」 > B2 盖在根材料上长出一个只读、共享的拆分节点 1ms
     ✓ B2 内置预设「蜂群模式」 > B2 拆分角色的默认是共享 / 只读，目标按「出方案」填 0ms
     ✓ B2 内置预设「蜂群模式」 > B2 引擎没有同 id 预设时前端兜一份，有则让位 0ms
     ✓ B2 内置预设「蜂群模式」 > B2 件数上限与并发上限同一处设置，默认 4 0ms
     ✓ B3 清单解析与按清单建节点 > B3 取最后一个 json 围栏 0ms
     ✓ B3 清单解析与按清单建节点 > B3 缺围栏 / JSON 坏 / 字段不合格都返回 null 0ms
     ✓ B3 清单解析与按清单建节点 > B3 accept 可缺省，超上限截断并给一句提示 0ms
     ✓ B3 清单解析与按清单建节点 > B3 每件建一个 draft 执行节点：worktree、件号、清单文本 1ms
     ✓ B4 G1 重叠闸门 > B4 swarmGate：干净且不重叠放行 0ms
     ✓ B4 G1 重叠闸门 > B4 swarmGate：重叠阻断并点名两件与撞上的路径 0ms
     ✓ B4 G1 重叠闸门 > B4 swarmGate：脏树阻断，glob 没匹配只提示 0ms
     ✓ B4 G1 重叠闸门 > B4 送给引擎的核对项按件号排序、只取带 swarm 的节点 0ms
     ✓ B4 G1 重叠闸门 > B4 SwarmGateNotice：阻断走警告样式，提示走常规灰字 4ms
     ✓ B4 G1 重叠闸门 > B4 SwarmEditor：draft 给编辑框，非 draft 只读 2ms
     ✓ B5 G3 逐件合并 > B5 全部 swarm 节点终态才出合并按钮 1ms
     ✓ B5 G3 逐件合并 > B5 送给引擎的件按件号排序，带 worktree 与本件验收命令 0ms
     ✓ B5 G3 逐件合并 > B5 结果行：已并入 / 冲突待裁决 / 测试红 / 已跳过 0ms
     ✓ B5 G3 逐件合并 > B5 只有 merged 的节点自动 adopted，冲突与测试红不动 0ms
     ✓ B5 G3 逐件合并 > B5 引擎返回 node_id / test_output 或 camelCase 都收得住 0ms
     ✓ B5 G3 逐件合并 > B5 SwarmMergeRow：并入走灰字，其它走警告，验收输出折起来 1ms
     ✓ B6 手册 > B6 docs/MANUAL.md 有「蜂群模式」一节，写清怎么盖、清单、合并、冲突 0ms

     Test Files  1 passed (1)
          Tests  24 passed (24)

    $ cd src-ui && npm run build

    > aicross-ui@0.0.0 build
    > tsc -b && vite build

    dist/assets/lang-cpp-macro-CgwvTOCX.js                         224.24 kB │ gzip:  18.05 kB
    dist/assets/lang-cpp-BAHFRj0j.js                               402.02 kB │ gzip:  31.11 kB
    dist/assets/main-DAxsJi8t.js                                   435.97 kB │ gzip: 214.22 kB
    dist/assets/xterm-_r9IGi5_.js                                  465.79 kB │ gzip: 123.12 kB

    ✓ built in 608ms

删掉一处未被使用的导出（withSwarmItem）之后这两条命令又各跑了一遍，同样全绿
（Test Files 24 passed / Tests 318 passed；built in 1.44s）。

另外对本期触及的文件逐个跑了 npx eslint，无输出＝零问题。仓库整体 npm run lint
本来就红（217 errors，全在本期未触及的文件里），不是本期引入的，也不在任务书的
验证命令里。

## 换行与改动范围核对

- src-ui/src/tauri.ts 是 CRLF/LF 混排：两处插入都用字节级脚本按 CRLF 写入，
  git diff 的新增行全部以 CRLF 结尾（脚本逐行核对过，无裸 LF 新增行）。
- 其余被改文件原本全是 LF，改后仍是 LF（docs/MANUAL.md crlf 0 / lf 111）。
- git diff 的删除行共 13 条，逐条核对都是本次有意替换的目标行：4 条 import、
  ROLES、FUTURE_PRESETS、listPresets 的 4 行、applyRoleDefaults 的 profile 行、
  路由卡 className、确认派发按钮的 disabled。
- git status 里的 tasks/review-08b-prompt.txt 不是本次产出的文件，未动它。

## 留给 A 侧 / 下一轮的两件事

1. 合并结果的字段命名：A 侧定下 snake 还是 camel 之后，可以把
   normalizeSwarmMergeResults 的兼容分支收掉（现在两种都收，不改也能跑）。
2. 拆分提示词：fixtures/prompts/split.txt 落地后，把
   PRESET_NODE_TEXTS['builtin-swarm'] 与它合并成一处，避免重复提示。

## 修正轮 R1

依据 `tasks/fix-08b-r1.md` 的 9 条（来自 `tasks/review-08b-ds.md` 的盲审）。
`tasks/review-08b-glm.md` 本工作树里不存在，无 GLM 补充项可处理。
仍然只改 src-ui/、docs/MANUAL.md、tasks/；src/ 一行没动；未 commit、未 push、
未起窗口、未发真机请求。

### 1. G1 闸门窗口期阻断（DS 2）

**已修**：核对从「一个可空对象」改成三态。

- src-ui/src/graph/swarm.ts:234 `SwarmCheckState = idle | pending | ok | error`；
  swarm.ts:226-228 三句文案（`正在核对文件范围…` / `无法核对文件范围` /
  `无法核对文件范围，已选择继续派发`）；swarm.ts:254 `swarmGate(state, proceedAnyway)`
  改签名：pending 一律 `blocked=true` 并出灰字「正在核对文件范围…」；error 默认
  `blocked=true`、把命令的错误原文接在「无法核对文件范围：」后面，并置
  `canProceed=true`；用户勾选后 `blocked=false`，只留一行说明。
  idle（本批没有蜂群节点 / 没有项目目录）照旧不拦。swarm.ts:248 `swarmCheckOf`
  给标红那一路取 check。
- src-ui/src/components/center/graph/DispatchOverlay.tsx:129-130 state 换成
  `SwarmCheckState` + `swarmProceed`；:222 发命令前先置 pending 并清掉上一次的
  「仍要派发」；:225-226 成功进 ok、失败进 error（带错误原文）；:229 闸门吃这两个值。
- src-ui/src/components/center/graph/DispatchBits.tsx:86-110 `SwarmGateNotice` 收
  `proceed`/`onProceed`，只在 `canProceed` 时出「仍要派发」勾选框。

**已补测试**：`R1-1 核对没回来时闸门拦住确认派发，核对不了时可以选择继续`、
`R1-1 SwarmGateNotice：核对中出灰字，核对不了才出「仍要派发」`。
原来把窗口期钉成「放行」的那条断言（`swarmGate(null)` → blocked:false）已改成
idle 态，不再为这个洞背书。

### 2. 生成执行节点幂等 + 上限（DS 3）

**已修**：

- src-ui/src/graph/swarm.ts:143 `swarmChildItems(graph, splitNodeId)` 数这个拆分
  节点底下已经有哪些件号；swarm.ts:154 `appendSwarmNodes(..., limit)` 跳过已存在的
  件号（记进 `skipped`），并按「已有件数 + 本次新建」不超过 limit 截断（记进
  `capped`），一个都没建时原样返回原图（引用不变）。
- swarm.ts:215 `swarmGenerateNotice(outcome, limit)` 出反馈行：
  `已生成 N 个执行节点` / `第 1、2 件已生成，未重复建` / `件数上限 2 件，1 件未生成`。
- DispatchOverlay.tsx:407-419 生成时把 `settings.swarmLimit` 一路传进解析和生成，
  没新建节点就不 commit，提示合并成一行。

**已补测试**：`R1-2 同一拆分节点再点一次不重复建件，件数上限在生成时也生效`。

### 3. 合并动作幂等、逐件结果落图、失败分级（DS 4）

**已修**：

- 幂等：src-ui/src/graph/swarm.ts:319 `swarmPendingMergeIds` 把已 adopted（＝已并入）
  的件剔掉，swarm.ts:328 `swarmMergeItems` 走它；DispatchOverlay.tsx:424 算出本次
  待并的件，:427 合并中或没有待并的件直接返回，:728 按钮 disabled 加上
  `swarmPendingItems.length === 0`，:724 出一行「本批的件都已并入，没有要合并的了」。
- 逐件落图：swarm.ts:416 `swarmReplyNote` 拼 `[蜂群合并] 第 N 件：…`；swarm.ts:422
  `applySwarmMerge` 把这行追加进对应节点的 Reply 正文（已经有同一行就不重复追加），
  `merged` 再标 adopted，`conflict`/`test_failed`/`skipped` 只记不采纳；引擎未接通
  那一批（note 是 `SWARM_BACKEND_MISSING_NOTE`）一个字都不写进图。
- 失败分级：swarm.ts:352 `swarmMergeErrorResults` 只在错误是
  `Tauri IPC not available`（或没有错误文本）时说「引擎未接通」，其它错误按
  `合并未完成：<原文>`；swarm.ts:396 `swarmMergeSummary` 只在「全部 skipped 且 note
  是引擎未接通」时说未接通，空数组说「合并结果为空，项目目录状态未知，请自行核对」，
  其余按件报数（`已并入 1 件，冲突 1 件，测试红 1 件`）。
  DispatchOverlay.tsx:434-437 catch 走 `swarmMergeErrorResults`，:726 结果行下面出这一句。

**已补测试**：`R1-3 逐件结果写回节点正文：并入标采纳，冲突与测试红只记 note`、
`R1-3 已并入的件不进下一次合并请求`、`R1-3 失败分级：只有引擎未接通才说未接通，部分并入如实报数`。

### 4. 蜂群卡片的动作约束（DS 5）

**已修**：src-ui/src/graph/swarm.ts:451 `swarmCardActions(node, result)`——非蜂群节点
三个动作全给（行为不变）；蜂群件一律不出「选它」；`showAdopt` 只在该件本次
`merged` 或节点已 adopted 时为真；已并入的件 `showRetry=false` 并给一句「已并入」。
DispatchOverlay.tsx:675 算一次，:698/:705/:707/:708 分别接到「选它」「采纳」「重派」
「升级」和那句提示上。「升级」与「重派」同一条 `retry()` 路径（都重开 worktree），
一起按已并入关掉，报告在此写明这一处比条目字面多关了一个按钮。

**已补测试**：`R1-4 蜂群卡片不出「选它」，未并入不给「采纳」，已并入不给「重派」`。

### 5. 重叠时可就地改文件范围（DS 6）

**已修**：DispatchOverlay.tsx:539-546 在判为重叠的那几张路由卡里内联
`SwarmEditor`（复用节点面板那个组件，一行一条 glob，draft 才可编辑），写回走
`updateNodeSwarm`；写回后 graph 变化触发 :215 那个 effect 自动重核对，重叠消失即
解禁。提示文案改成「改到不重叠或合并成一件」（swarm.ts:271）。

**已补测试**：`R1-5 重叠件改完文件范围，重核对的入参跟着变`（纯函数：
`updateNodeSwarm` → `swarmCheckItems` 的入参跟着变；并钉住提示里的那句出路）。

### 6. 盖预设菜单包含蜂群（DS 7）

**已修**：src-ui/src/components/center/graph/GraphTab.tsx:14 引入
`withSwarmPreset`，:109（预设变更后的 reload）与 :160（首次加载）两处 `listPresets()`
都套上，与右栏 OrchestrationPanel 同一个出口。

**已补测试**：`R1-6 图上「盖预设」菜单也套 withSwarmPreset`（读 GraphTab.tsx 源码断言
没有裸的 `setPresets(f.presets)`、`withSwarmPreset(f.presets)` 正好两处；组件层要起
窗口才测得到，这里沿用 B6 读文件断言的做法）。

### 7. 拆分提示词与上限（DS 8）

**已修**：

- src-ui/src/graph/stamp.ts:9 `swarmSplitText(limit)` 把「最多 N 件」按传入上限拼；
  stamp.ts:34 `PRESET_NODE_TEXTS['builtin-swarm']` 用它的默认值；stamp.ts:42 `stamp()`
  多一个可选 `swarmLimit` 参数，:87 只有 `builtin-swarm` 走动态文本，其它内置预设照旧。
  两个盖章入口把设置传进去：GraphTab.tsx:370、OrchestrationPanel.tsx:140
  （`loadDispatchSettings().swarmLimit`）。
- swarm.ts:104 `swarmPlanNotice(plan, limit)` 直接收上限，不再拿截断后的件数反推
  （DS 4.5 同一处）。
- src-ui/src/graph/types.ts:135 `roleDefaults` 补「拆分」的显式分支（值仍是共享/只读）。
- **A 侧 split.txt 落地后要删的位置**：`src-ui/src/graph/stamp.ts:9-20` 的
  `swarmSplitText` 与 stamp.ts:34 的 `'builtin-swarm'` 条目、stamp.ts:87 那个分支，
  连同 stamp() 的 `swarmLimit` 参数与两个调用点的传参；届时改由 `promptForRole`
  读 `fixtures/prompts/split.txt`，否则会重复提示一遍。

**已补测试**：`R1-7 拆分提示词按设置的件数上限拼「最多 N 件」`（含
`stamp(..., 2)` 盖出来的节点文本、解析截断线与提示里的数三处同一个上限）；
`roleDefaults('拆分')` 的断言在既有 B2 用例里。

### 8. 每件只带自己的清单条目（DS 9）

**已修**：生成的执行节点 `user.text` 本来就只有本件（swarm.ts:113 `swarmNodeText`），
真正的口子在可见性——「共享」会把拆分回复的整份清单带进每件的上下文。
swarm.ts:180-181 改成 `visibility='指定上游'`、`upstreams=[graph.root]`：上下文只剩
根材料与本件自己那条，拆分回复（整份清单）与别的件都进不来，`upstreamArtifactsOf`
也不会跨件带产物。拆分节点的用户消息（拆分要求本身）仍在上下文里，那是模板不是清单。

**已补测试**：`R1-8 每件只带自己的清单条目，上下文不带整份清单`
（`serializeContext` 的 prompt 里有根材料、没有第二件的标题与文件范围，
`contextPartsOf` 里没有任何 reply 段）。既有 B3 用例的可见性断言同步改成「指定上游」。

### 9. `Node.swarm` 持久化说明（DS 1）

**不改代码，按条目要求写进合并对账**：

- 本期前端不丢字段（types.ts:86 追加、图落盘不做白名单重建），但 Rust 侧
  `GraphNode` 目前没有 `swarm`，`save_graph` 是「反序列化 → 重新序列化」，未知字段
  会被静默丢弃。**A 侧 `GraphNode.swarm` 镜像并入之前**，每次 `commitGraph` 写盘都会
  把 `swarm` 抹掉：重启后蜂群节点回来时没有 `swarm`，G1 闸门不触发、合并区不出现、
  文件范围编辑框消失，且不报错。合并对账时请与 A-1（镜像 + 往返测试）一并核对。
- 前端加载旧图缺字段时按无 swarm 处理，已是既有行为，本轮补了一条测试钉住：
  `R1-9 旧图节点没有 swarm 字段时按无蜂群处理`。

### 手册

docs/MANUAL.md「蜂群模式」一节同步了本轮的行为：每件只看根材料与自己那条、
重复生成不重复建、核对窗口期与「仍要派发」、重叠卡片就地改范围、已并入的件不再
参与合并、结果卡的动作限制、整批结论按件报数。

### GLM 补充（`tasks/review-08b-glm.md`，本轮开工后才出现，已一并处理）

与 DS 重复的五条（重复生成、合并入口、擂台「选它」、核对在途不阻断、每件带整份
清单）由上面 1–8 条覆盖。不重复的处理如下：

- **已修 · GLM 2-1（它列的最高风险）合并请求字段命名单押 camelCase**：
  src-ui/src/tauri.ts:561-562 给 `SwarmMergeRequestItem` 加可选 `node_id`（按字节脚本
  局部改，该段原本就是 CRLF，改后混排统计 565/660 → 567/662，无裸 LF 新增行）；
  src-ui/src/graph/swarm.ts:331-337 每件同时发 `nodeId` 与 `node_id`。A 侧无论按契约
  文本写 snake_case 还是随其它派发结构写 camelCase 都能反序列化，多出来的键被 serde
  忽略；以前一旦命名不合，合并会静默全跳过并谎称「引擎未接通」。
  **已补测试**：`R1-GLM1 合并请求同时带 nodeId 与 node_id，A 侧选哪种命名都收得住`。
- **已修 · GLM 2-5 仲裁区在蜂群批次上照常出现**：swarm.ts:459 `arbitrationCandidates`
  把带 `swarm` 的结果排除在仲裁候选之外，DispatchOverlay.tsx:743 用它取代原来的
  `status === 'ok'` 过滤。件与件是分工不是分歧。
  **已补测试**：`R1-GLM4 蜂群件不进第三方仲裁的候选`。
- **已消解 · GLM 2-6 会话续聊让各件续同一个拆分会话**：`continuationSession`
  （dispatch.ts:133-135）只对 `visibility === '共享'` 的节点续聊；第 8 条把蜂群件改成
  「指定上游」之后，各件必然独立起会话，无需另加分支。
- **不采纳（本轮）· GLM 2-3 合并入口绑定单次派发会话**：关掉结果屏重开、或单件重派
  之后合并清单会缩水。确实是洞，但要把合并入口从「本批 results」改成「图上该拆分
  底下的全部件」，牵动 G3 的出现条件与 runId 取值（引擎按 runId 写留痕），
  超出本轮 9 条的范围，且与 DS 2.4 是同一处口径问题——建议下一轮与 A 侧一起定
  「按批还是按拆分」再改。
- **不采纳 · GLM 4-3 给核对加防抖**：`SwarmEditor` 每次键入都会重核对一次
  （只读命令，但会遍历项目文件）。本轮把编辑框放进了确认屏，这一点更明显了；
  但防抖会引入新的时序（正在核对与已核对的交界），本轮已经把「核对在途」
  做成阻断态，先保证正确再谈省事，留到下一轮。
- **不采纳 · GLM 1-1 `parseSwarmPlan` 去掉默认参数**：唯一调用点已显式传
  `settings.swarmLimit`，去掉默认值会让纯函数测试与将来的调用方更啰嗦，收益小。

## R1 验证

两条命令都实际跑过，输出原文：

    $ cd src-ui && npx vitest run --configLoader native --pool threads

     RUN  v5.0.0 D:/work/aicross-p5b/src-ui


     Test Files  24 passed (24)
          Tests  332 passed (332)
       Start at  09:28:58
       Duration  567ms (transform 65%, import 17%, tests 16%, worker 3%)

      Transform  transforming modules took 2.71s · 65% of tracked time, re-done on every run
                 persist transforms across runs with fsModuleCache: true
                 learn more: https://vitest.dev/guide/improving-performance#caching-between-reruns

本轮新增 14 条用例（318 → 332；同一命令加 `--reporter=verbose` 只跑本文件，全绿）：

     ✓ R1 修正轮 > R1-1 核对没回来时闸门拦住确认派发，核对不了时可以选择继续 0ms
     ✓ R1 修正轮 > R1-1 SwarmGateNotice：核对中出灰字，核对不了才出「仍要派发」 1ms
     ✓ R1 修正轮 > R1-2 同一拆分节点再点一次不重复建件，件数上限在生成时也生效 1ms
     ✓ R1 修正轮 > R1-3 逐件结果写回节点正文：并入标采纳，冲突与测试红只记 note 0ms
     ✓ R1 修正轮 > R1-3 已并入的件不进下一次合并请求 0ms
     ✓ R1 修正轮 > R1-3 失败分级：只有引擎未接通才说未接通，部分并入如实报数 0ms
     ✓ R1 修正轮 > R1-4 蜂群卡片不出「选它」，未并入不给「采纳」，已并入不给「重派」 0ms
     ✓ R1 修正轮 > R1-5 重叠件改完文件范围，重核对的入参跟着变 0ms
     ✓ R1 修正轮 > R1-6 图上「盖预设」菜单也套 withSwarmPreset 2ms
     ✓ R1 修正轮 > R1-7 拆分提示词按设置的件数上限拼「最多 N 件」 0ms
     ✓ R1 修正轮 > R1-8 每件只带自己的清单条目，上下文不带整份清单 1ms
     ✓ R1 修正轮 > R1-GLM1 合并请求同时带 nodeId 与 node_id，A 侧选哪种命名都收得住 0ms
     ✓ R1 修正轮 > R1-GLM4 蜂群件不进第三方仲裁的候选 0ms
     ✓ R1 修正轮 > R1-9 旧图节点没有 swarm 字段时按无蜂群处理 0ms

     Test Files  1 passed (1)
          Tests  38 passed (38)

    $ cd src-ui && npm run build

    > aicross-ui@0.0.0 build
    > tsc -b && vite build

    vite v8.0.7 building client environment for production...
    transforming...✓ 324 modules transformed.
    rendering chunks...
    computing gzip size...
    dist/index.html                                                  3.46 kB │ gzip:   1.32 kB
    ...
    dist/assets/main-BzbTGPsY.js                                   439.65 kB │ gzip: 215.16 kB
    dist/assets/xterm-_r9IGi5_.js                                  465.79 kB │ gzip: 123.12 kB

    ✓ built in 595ms

本轮触及的文件逐个跑了 `npx eslint`：只剩 GraphTab.tsx 的两条既有问题
（87:38 `useState<any>`、175:6 的 effect 依赖告警），都在本轮 diff 之外的行上。

## R1 换行与改动范围核对

- 上一轮报告把这批文件记成「LF」是对的，本轮一度按 CRLF 误改过一遍，已全部改回：
  本轮触及的 8 个源文件与 docs/MANUAL.md 现在都是纯 LF（crlf=0），
  它们的 `git diff` 里没有任何 CR 字节。tauri.ts 是 CRLF/LF 混排，只按字节脚本插了
  两行（都以 CRLF 结尾，统计 565/660 → 567/662）。
- 改动范围：src-ui/src/graph/{swarm,stamp,types}.ts、
  src-ui/src/components/center/graph/{DispatchOverlay,DispatchBits,GraphTab}.tsx、
  src-ui/src/components/right/orchestration/OrchestrationPanel.tsx、
  src-ui/src/graph/__tests__/swarm-08b.test.tsx、src-ui/src/tauri.ts（两行）、
  docs/MANUAL.md、本报告。
  src/（Rust）、fixtures/、src/channels/ 一行没动。
- 既有用例改了三处，都是为了不给旧行为背书：B3 的可见性断言（共享 → 指定上游、
  upstreams=[root]）、B4 的 `swarmGate` 入参（check → 三态）与重叠提示文案、
  `SwarmGateNotice` 的 gate 形状多了 `canProceed`。
