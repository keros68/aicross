复审完成。先给逐条核对，再答 smoke 问题。

说明：本会话没有 shell 工具，两套测试我无法亲自重跑；测试存在的核验是静态的（逐一读了测试源码并清点用例数），REPORT 里的运行输出只做了数目交叉核对（详见文末）。

## 修正单 12 条逐条核对

| # | 结论 | 依据 |
|---|------|------|
| 1 contextOf 祖先闭包 | **已核实修好** | `context.ts:5-17` 沿所有 parents 递归闭包（悬空跳过、环防御）；`context.ts:21-32,56-57` 从根 DFS、`seen` 去重、自身附加末尾；`context.ts:59` `isAccept` 按 `kind === 'accept'`；`context.ts:74-75` accept 直接父回复无条件保留；`layout.ts:111-131` `normalizeNodeKinds` 挂在两处 loadGraph 入口（`GraphTab.tsx:79,141`）并 `console.warn`。两个新测试都在且断言了三条分支回复：`context.test.ts:167-196`（再分叉共享 turn 含 答E1/答E2/答E3，我手工推演了 DFS 序与期望数组一致）和 `context.test.ts:198-227`（1→3→1 stamp 集成，验收(盲) context 恰为根消息+三个执行父回复，盲过滤正确剔除了祖父阶段回复与自身回复） |
| 2 同分支单面板 | **已核实修好** | `layout.ts:103-106` `sameBranch` 基于祖先链互查；`GraphTab.tsx:159-160` `alignMain` 收掉 2/3 列同分支面板、`GraphTab.tsx:163-169` `pinColumn` 原位替换同分支列、满 3 列替换末列。语义符合「按分支而非节点 id」 |
| 3 PresetBuilder 崩溃 | **已核实修好** | `PresetBuilder.tsx:72` 解析成功先 `setEditing(null)`；`PresetBuilder.tsx:205` `stage.nodes[editing.ni]` 存在性守卫；`PresetBuilder.tsx:214` `key={si-ni}`；`CenterPanel.tsx:1479-1481` GraphTab 整体包 ErrorBoundary，PTY 分支（:1483）不受影响。且其余会移位 stages 的路径（删节点/删阶段/移阶段 :88,99,104）也都先 `setEditing(null)`，无漏网 |
| 4 记法有损提示 | **已核实修好** | `notation.ts:299-301` 覆盖 provider / tier / upstreams 三种（`visibility==='指定上游'` 一律计入，空列表也提示；空串 provider/tier 不计）；`PresetBuilder.tsx:242-244` 记法行下显示「一行记法不表达：…」；`notation.test.ts:201-250` 3 例 lossy 测试 + `:51` 往返 helper 断言 `lossy` 为空，覆盖 lossy |
| 5 节点卡标未接入 | **已核实修好** | `GraphTab.tsx:321,344` 用 `validateNode` 结果渲染 `hint-capsule`，样式在 `GraphTab.css:308` |
| 6 presets 版本校验 | **已核实修好** | `orchestration.rs:390-404` `version != 1` 拒绝整文件只回内置；测试 `orchestration.rs:653-680` 覆盖 version=2 拒 / version=1 合并 |
| 7 parseSend 测试 | **已核实修好** | `layout.test.ts:171-208` 5 例：@agent/模型带 provider/tier、裸 @ 唯一匹配、消歧落手填、未接入原样、无 @ 返回 undefined（「沿用列末端」在 `GraphTab.tsx:208` 的 `target ?? endNode.target ?? defaultTurnTarget()`，纯函数侧断言 undefined 即等价覆盖） |
| 8 created_at 递增 | **已核实修好** | `stamp.ts:37` `nowMs + index`；`stamp.test.ts:51-54` 递增断言 |
| 9 魔串去除 | **已核实修好** | `types.ts:105-124` `newGraph()` 标题置空；`mutate.ts:44-46` 只判空串；展示兜底在 `CenterPanel.tsx:1228` 与 `GraphTab.tsx:255`；全仓 `newGraph(` 调用均无参 |
| 10 scrollIntoView | **已核实修好** | `GraphTab.tsx:127-133` canvasRef + effect，`focusNodeId` 变化时对 `.graph-node.focused` `scrollIntoView({block:'nearest',inline:'nearest'})` |
| 11 原子写注释 | **已核实修好** | `orchestration.rs:297-300,327-331` 注释改为 MOVEFILEEXW 覆盖语义、删目标重试定位为锁占用兜底，行为未动 |
| 12 工具函数收敛 | **已核实修好** | grep 全仓：`childrenOf` 仅 `stamp.ts:47`、`ancestorsOf` 仅 `layout.ts:85`（validate.ts:2 改 import）、`cloneTarget` 仅 `types.ts:99`（stamp/mutate/PresetBuilder 均改 import）、`findModel` 仅 `validate.ts:6`（GraphTab:317、OrchestrationPanel:130,217、PresetBuilder:180 均引用）。`NodePanel.tsx:42` 保留的精确相等 find 语义确实不同（严格 agent+model 判 known），REPORT 申明成立 |

## 回归检查

- **终端 tab / 任务板**：CenterPanel 本轮唯一改动是 graph 分支加 ErrorBoundary，`'graph'` 走非 PTY 分支（:1476），TierTerminal 分支与其 ErrorBoundary（:1483）未动；Compiler.tsx / TaskBoard 本轮未触碰。未破坏。
- **新重复**：无。无循环依赖（context→stamp→types、validate→layout 均单向）。
- **两处非阻塞观察**（不要求本轮处理）：
  1. `contextOf` 经 `orderByDfs`→`childrenOf`（`stamp.ts:47` 每次全图扫描）变成 O(N²)，且祖先必须从 `graph.root` 可达——手改 JSON 造成断连子图时那些祖先会静默丢（构造路径不可达此态，旧实现反而能收到）。当前规模无感，外发前值得留意。
  2. 节点卡第二行极端时 4 枚胶囊（角色+汇合+待派+未接入）在 56px 定高下可能裁切——REPORT「本轮不确定处 3」已自报，留 smoke 看。

## 两条「不采纳」

均成立。i18n 是第 1 轮已申明的取舍、修正单明示第 5 期统一处理；smoke 9 项是修正单自己规定「本轮不要起窗口」的直接结果，留给人工窗口正是本轮设计。

## 测试运行的交叉核对（未能重跑的说明）

`tasks/log-02-*.out` 是第 1 轮的陈旧产物（vitest 41、cargo 92，时间 16:45，无 `presets_wrong_version_returns_builtins`），REPORT R1 的数字（51/93）只贴在报告内。静态清点与 51 吻合：notation 18（15+3）+ context 9（7+2）+ layout 14（含 parseSend 5）+ stamp 4 + validate 6 = 51；orchestration 13 个 `#[test]`（12+1）与「92→93」吻合。数字自洽但我未独立复跑，smoke 前建议顺手跑一次 `npx vitest run` + `cargo test` 确认。

---

## 结论：修正轮 R1 可以进入人工窗口 smoke。

12 条全部「已核实修好」，无未修、无改法有问题，未发现阻断性回归。仍未闭合的项（均非本轮缺陷）：

1. 手动 smoke 9 项——本轮按规不做，正是窗口环节要过的；
2. 界面文案 i18n——已申明推迟到第 5 期；
3. 上述两处非阻塞观察（contextOf O(N²)/断连子图丢祖先、节点卡胶囊裁切）；
4. 测试结果为静态核对 + REPORT 数字自洽，未独立复跑（会话无 shell），窗口前建议复跑一次两套测试。