# REPORT-08C 蜂群联调收口

任务书 `tasks/TASK-08C-swarm-integration.md` 的 8 条。分支 `phase/8c`（基于已含 8a/8b/09 的 main）。
前端为主；`src/` 一行没改（第 1 条选了「前端在 preset_text 后追加一行」，见下）。
未起窗口、未发真机请求、未 commit、未 push，没有写仓库以外的位置。
`src-ui/src/tauri.ts` 是 CRLF/LF 混排，按字节脚本局部改，改后 crlf 567→575、bare_lf 95 不变；
其余文件本来就是纯 LF，改后仍是纯 LF。

## 逐条

1. **已做：拆分提示词单一来源。** 删掉 `stamp.ts` 的 `swarmSplitText(limit)`、`PRESET_NODE_TEXTS['builtin-swarm']`
   与 `stamp()` 里那个 `preset.id === 'builtin-swarm'` 分支；盖章统一走
   `presetNodeText(target, swarmLimit)`（`src-ui/src/graph/stamp.ts:8`、`:65`、`:77`），文本只来自
   引擎 `Target.preset_text`。「最多 N 件」选**前端在 `preset_text` 后追加一行**
   `本次上限 N 件。`（`swarmLimitLine`，`src-ui/src/graph/swarm.ts:27`；只对 `role === '拆分'`
   且 preset_text 非空的节点追加）——split.txt 模板不动占位，因此 `src/orchestration.rs` 无需改动。
   兜底预设 `BUILTIN_SWARM_PRESET` 的 `preset_text` 直接读同一份冻结文件
   （`import splitPrompt from '../../../fixtures/prompts/split.txt?raw'`，`swarm.ts:24`、`:77`），
   TS 里不再维护第二份文本；`withSwarmPreset` 仍只在引擎清单没有 `builtin-swarm` 时兜底（未改）。
   ／已补：`08C-1 拆分提示词只有一份来源：盖章消费引擎的 preset_text，前端不另写文本`；
   既有 `R1-7` 改写为 `R1-7 拆分提示词按设置的件数上限补「本次上限 N 件」`，
   `B2 盖在根材料上长出一个只读、共享的拆分节点` 的断言同步改成「split.txt + 上限行」。

2. **已做：合并入口按图派生。** 新增 `swarmMergeEntry(graph, stage)`（`swarm.ts:495`）与
   `swarmStageOf(graph, nodeIds)`（`swarm.ts:528`）：件的状态从节点正文里最后一条
   `[蜂群合并]` 留痕读（`swarmRecordedResult`，`swarm.ts:459`），返回
   `nodeIds / ready / pendingIds / mergedIds / disputedIds / rows`。结果屏改成按它渲染
   （`DispatchOverlay.tsx:429-433`、`:735-758`），不再绑当次 `results`：单件重派、关屏重开之后
   清单仍是该阶段图上的全部件，冲突件不会没地方再并。本次合并过的件仍用
   `SwarmMergeRow`（带验收输出折叠），其余按图上的留痕出一行。
   ／已补：`08C-2 合并入口按图派生：可并件 / 已并件 / 待裁决件都从图上算`。

3. **已做：跨拆分节点件号续号。** `swarmItemBase(graph, splitNodeId)`（`swarm.ts:184`）：
   这个拆分节点已经生成过件时锚在自己那批的起点（`own[0] - 1`，保证重复点「生成」仍幂等），
   否则取图上别处已用掉的最大件号；`appendSwarmNodes` 的件号改成 `base + index + 1`
   （`swarm.ts:211`、`:222`）。G1 文案与合并排序都按件号，两批不再撞号、不再交错。
   ／已补：`08C-3 跨拆分节点件号续号：第二个拆分从最大件号 +1 起编，重复生成仍幂等`。

4. **已做：G1 首帧与空目录。** 核对状态加 `noProject` 态与文案
   `未选工作目录，无法核对文件范围`（`swarm.ts:276`、`:318`）；闸门读的状态由纯函数
   `swarmCheckInput(swarmCount, projectDir, state)` 推出来（`swarm.ts:291`，界面 `DispatchOverlay.tsx:234`）：
   没有蜂群件不拦，有件但没选工作目录时命令不发、直接阻断并说原因（不给「仍要派发」）。
   首帧：`swarmCheck` 初值从 `idle` 改成 `pending`（`DispatchOverlay.tsx:135`），effect 还没跑的那一帧
   按 pending 阻断，不留可点窗口；effect 里也不再把状态打回 `idle`（`:223`）。
   ／已补：`08C-4 G1 首帧与空目录：没选工作目录时阻断并说原因，首帧按 pending 处理`。

5. **已做：只发 camelCase，并补上漏发的 `settings`。** 删掉 `SwarmMergeRequestItem.node_id` 双写
   （`tauri.ts:566-573`、`swarm.ts:392`）、删掉死常量 `SWARM_CHECK_FALLBACK` 与
   `tauri.ts` 里「调用方回退到 SWARM_CHECK_FALLBACK」「两种写法都可能出现」两处过期注释。
   联调时发现两处 wire 对不上，一并修（属本条「合并请求」范围，报告在此写明）：
   - A 侧 `check_swarm_plan` / `merge_swarm_items` 都要 `settings: DispatchSettings`（`src/dispatch.rs:598`、`:1059`），
     前端两条命令都没发，实际调用会直接 reject（闸门永远显示「无法核对文件范围」、合并永远「未接通」）。
     现在补 `SwarmSettings`（`DispatchSettings` + `swarmLimit`，`tauri.ts:562`）与
     `swarmWireSettings(settings)`（`swarm.ts:34`，界面设置里的 `classificationModel` 不发），
     两条命令都带上（`tauri.ts:362`、`:371`，`DispatchOverlay.tsx:227`、`:440`）。
   - `SwarmMergeItem.title` 前端从不填，合并提交标题一直是空的（`aicross swarm item N:` 后面什么都没有）；
     现在取本件消息的第一行（`swarm.ts:392`）。`SwarmPlanCheck.note`（超上限、非法 glob 的说明）
     前端类型里没有、拿到也不显示，现在加可选字段并进闸门 hints（`tauri.ts:558`、`swarm.ts:344`）。
   ／已补：`08C-5 合并与核对只发 camelCase，且都带 settings`；既有
   `R1-GLM1 合并请求同时带 nodeId 与 node_id` 已随双写删除，`B1`/`B5` 的断言同步。

6. **已做：已并入件的保护。** `swarmCardActions` 认图上的合并留痕（不只认当次结果），
   已 adopted 的已并入件不再给「采纳 / 取消采纳」按钮，仍标「已并入」、仍不给「重派 / 升级」
   （`swarm.ts:612-621`）；`pickOne` 的 rivals 排除所有带 `swarm` 的节点
   （`src-ui/src/graph/dispatch.ts:417`），同一拆分底下手动加的普通兄弟按「选它」不会再清掉
   已并入件的 adopted、把它送回待合并清单。
   ／已补：`08C-6 已并入的件不给「取消采纳」，兄弟卡的「选它」也清不掉它的采纳`；
   既有 `R1-4` 的「已并入仍给采纳」断言同步改掉。

7. **已做（改了 context.ts）：每件上下文只带根材料。** 原来「指定上游」只过滤模型回复、
   用户消息无条件全留，所以拆分节点的用户消息（拆分要求，含「只读根材料，不修改文件」）
   会进每件的 prompt。现在用户消息与模型回复同口径：只留 upstreams 点名的节点加本节点自己
   （`src-ui/src/graph/context.ts:104-105`）。蜂群件 `upstreams=[root]`，因此只剩根材料与本件那条。
   连带改了两处口径说明：`ExplainOverlay.tsx:33` 的一句，与 `context.test.ts` 里
   「指定上游：用户消息全留」那条用例（改名为「用户消息与模型回复都只留 upstreams 列表里的节点，加自己」）。
   影响面核对过：`指定上游` 在产品里只有蜂群件在用（仲裁节点是「盲」，见 `dispatch.ts` 的
   `appendArbitration`），其余是用户在节点面板手选。
   ／已补：`08C-7 每件上下文只带根材料，不带拆分节点的用户消息`。

8. **已做：前端只读校验集合加「拆分」。** `validate.ts` 抽出 `READONLY_ROLES = ['验收','验证','仲裁','拆分']`
   （`src-ui/src/graph/validate.ts:6`、`:31`），提示语按角色出（拆分 → 「拆分节点只能只读」）。
   与 `NodePanel.tsx:32` 的 `ROLES`、引擎 `src/dispatch.rs:1318` 的 `readonly_role` 同一份名单。
   ／已补：`08C-8 前端只读角色集合含「拆分」，与 NodePanel 和引擎护栏同口径`（同时读
   NodePanel 与 `src/dispatch.rs` 源码对齐名单）。

### 顺带改的文档

`docs/MANUAL.md`「蜂群模式」一节按本轮行为改了四处：件号续号、未选工作目录阻断、
合并清单按图算（单件重派/关屏重开后冲突件仍在）、已并入的件不给「取消采纳」。

### 没做

- `src/orchestration.rs` 未改：第 1 条选了「前端在 `preset_text` 后追加一行」，
  split.txt 的 `N` 占位不动，引擎侧不需要知道本机上限。
- 核对防抖（08B-R1 未采纳项）本轮未做，不在这 8 条里。
- `tasks/review-08b-r1-ds.md` 的「报告行号漂移」一项不涉及代码，未处理。

## 验证命令与输出原文

`cargo check`（零 warning）

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p5b)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.40s
```

`cargo test`

```text
running 329 tests
test result: ok. 318 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 8.93s
```

`cd src-ui && npx vitest run --configLoader native --pool threads`

```text
 Test Files  26 passed (26)
      Tests  360 passed (360)
```

同一命令加 `--reporter verbose`，本轮新增/改名的用例（ANSI 色码已去除）：

```text
 ✓ src/graph/__tests__/swarm-08c.test.tsx > TASK-08C 蜂群联调收口 > 08C-1 拆分提示词只有一份来源：盖章消费引擎的 preset_text，前端不另写文本 5ms
 ✓ src/graph/__tests__/swarm-08c.test.tsx > TASK-08C 蜂群联调收口 > 08C-2 合并入口按图派生：可并件 / 已并件 / 待裁决件都从图上算 10ms
 ✓ src/graph/__tests__/swarm-08c.test.tsx > TASK-08C 蜂群联调收口 > 08C-3 跨拆分节点件号续号：第二个拆分从最大件号 +1 起编，重复生成仍幂等 1ms
 ✓ src/graph/__tests__/swarm-08c.test.tsx > TASK-08C 蜂群联调收口 > 08C-4 G1 首帧与空目录：没选工作目录时阻断并说原因，首帧按 pending 处理 8ms
 ✓ src/graph/__tests__/swarm-08c.test.tsx > TASK-08C 蜂群联调收口 > 08C-5 合并与核对只发 camelCase，且都带 settings 2ms
 ✓ src/graph/__tests__/swarm-08c.test.tsx > TASK-08C 蜂群联调收口 > 08C-6 已并入的件不给「取消采纳」，兄弟卡的「选它」也清不掉它的采纳 3ms
 ✓ src/graph/__tests__/swarm-08c.test.tsx > TASK-08C 蜂群联调收口 > 08C-7 每件上下文只带根材料，不带拆分节点的用户消息 1ms
 ✓ src/graph/__tests__/swarm-08c.test.tsx > TASK-08C 蜂群联调收口 > 08C-8 前端只读角色集合含「拆分」，与 NodePanel 和引擎护栏同口径 3ms
 ✓ src/graph/__tests__/swarm-08b.test.tsx > B1 契约：Node.swarm 与两条命令 > B1 后端未接通时两条命令 reject，回退值就是契约里那个形状 2ms
 ✓ src/graph/__tests__/swarm-08b.test.tsx > B2 内置预设「蜂群模式」 > B2 盖在根材料上长出一个只读、共享的拆分节点 2ms
 ✓ src/graph/__tests__/swarm-08b.test.tsx > R1 修正轮 > R1-7 拆分提示词按设置的件数上限补「本次上限 N 件」 0ms
 Test Files  26 passed (26)
      Tests  360 passed (360)
```

`cd src-ui && npm run build`

```text
dist/assets/main-Bdfp6vop.js                                441.99 kB │ gzip: 215.62 kB
dist/assets/xterm-C7jn2Fqg.js                               465.79 kB │ gzip: 123.12 kB

✓ built in 2.16s
```

## 改动文件

```text
 src-ui/src/components/center/graph/DispatchOverlay.tsx       |  53 +++---
 src-ui/src/components/right/orchestration/ExplainOverlay.tsx |   2 +-
 src-ui/src/graph/__tests__/context.test.ts                   |   6 +-
 src-ui/src/graph/__tests__/swarm-08b.test.tsx                |  51 ++---
 src-ui/src/graph/context.ts                                  |   6 +-
 src-ui/src/graph/dispatch.ts                                 |   2 +
 src-ui/src/graph/stamp.ts                                    |  37 +--
 src-ui/src/graph/swarm.ts                                    | 173 ++++++++++++--
 src-ui/src/graph/validate.ts                                 |  13 +-
 src-ui/src/tauri.ts                                          |  26 ++-
 docs/MANUAL.md                                               |   9 +-
 src-ui/src/graph/__tests__/swarm-08c.test.tsx                | 新增
```

## 不确定处

- 第 7 条改的是「指定上游」的通用口径，不只是蜂群。核对过现有用法（蜂群件、用户手选），
  没有别的内置路径依赖「非上游节点的用户消息也带上」，但这是一条对外可见的语义变更，
  `ExplainOverlay` 与手册说明已同步。
- 第 2 条的 `swarmStageOf` 取「本次 results 与本次可派节点里第一个蜂群件所在阶段」。
  一张图里两个拆分节点恰好落在同一阶段时，合并清单会把两批件并成一张表（按件号排序，
  第 3 条已保证不撞号）。若后续要按拆分节点分表，改 `swarmMergeEntry` 的入参即可。
- 第 5 条修的两处 wire 不一致（缺 `settings`、缺 `title`）是读 A 侧签名比对出来的，
  没有起窗口做真机联调；引擎侧行为以 `cargo test` 里 A 侧的既有用例为准。
