# 第 2 期修正轮 R1

先读 `AGENTS.md`、`tasks/TASK-02-orchestration.md`、`tasks/REPORT-02.md`，再读 `tasks/review-02-glm.md`（另一厂商盲审）。逐条处理，每条在 `tasks/REPORT-02.md` 末尾新增「修正轮 R1」一节里写「已修：怎么改的」或「不采纳：原因」。本轮不要起窗口、不要跑 `tauri dev`；验证只跑 `cargo test`、`cd src-ui && npx vitest run`、`cd src-ui && npm run build`，命令与输出原文贴进报告。不要 commit，不要 push。

## 必修

1. **contextOf 的路径改为全部祖先闭包**（审查 2.2，编排者核对确认）。规则定为：任一节点的路径 = 它的全部祖先（沿所有 parents 递归）+ 自身，按从根出发的深度优先顺序排列、去重；accept 节点不再特殊算并集，因为闭包天然覆盖。可见性过滤规则不变（共享 / 盲 / 指定上游；accept 节点的直接父节点回复始终保留）。同时把 `isAccept` 改为按 `kind === 'accept'` 判断（审查 2.3），`load_graph` 或前端加载时校验 kind 与 parents 数一致，不一致按 parents 数修正 kind 并记一条 console.warn。补测试：验收节点上再分叉一个共享 turn，其上下文含全部三个执行分支的回复；经济流水线盖在根上、填好各节点 reply 后，验收节点 context == 用户消息 + 三个执行父的回复（审查 6.2、6.3）。
2. **「同一分支只保留一个面板」按分支判定**（审查 2.1）：新节点与某列末端节点在同一条祖先链上（一方是另一方的祖先或相同）时替换那一列，`alignMain` 与 `pinColumn` 都要查。
3. **PresetBuilder 崩溃**（审查 2.4）：记法解析成功后 `setEditing(null)`；渲染处对 `stage.nodes[editing.ni]` 为 undefined 做防御；`TargetEditor` 加 `key`（审查 2.5）；`GraphTab` 整个包进 `ErrorBoundary`。
4. **一行记法有损提示**（审查 1.1）。规格修订：一行记法只表达 agent/model、角色、thinking、可见性种类、写权限；`provider`、`tier`、指定上游的节点列表不进记法。要求：`parseNotation`/`printNotation` 对可表达字段往返相等；`printNotation` 遇到含 `provider`/`tier`/`upstreams` 的预设返回 `{ text, lossy: string[] }`（或等价形式），构建器在记法行下方显示「一行记法不表达：provider、档位、指定上游」；`@upstream` 解析回「指定上游」且 upstreams 为空时同样提示。补往返测试覆盖 lossy 标记。
5. **图上节点卡标「未接入」**（审查 1.2）：`renderNodeCard` 用 `validateNode` 的结果显示一枚小标签。
6. **presets.json 版本校验**（审查 1.3）：`version != 1` 拒绝该文件、只回内置，与 graphs 对齐，补 Rust 测试。
7. **parseSend 测试**（审查 6.1）：`@` 提取、消歧失败落手填、无 @ 沿用列末端 Target，至少 3 例。

## 次要（本轮一并做）

8. stamp 出的同层节点 `created_at` 按阶段内索引递增毫秒，保证竖排顺序与预设书写顺序一致（审查 2.6）。
9. `fillRoot` 去掉 `title === '对话图'` 魔串：`newGraph` 标题置空，展示层兜底（审查 3.2）。
10. 高亮节点变化时节点卡 `scrollIntoView({ block: 'nearest', inline: 'nearest' })`（审查 3.3）。
11. 原子写注释改正：Windows 的 `std::fs::rename` 可覆盖已存在目标，删目标重试只是锁占用兜底（审查 3.1）。
12. 收敛重复工具函数：`childrenOf`、`ancestorsOf`、`cloneTarget`、vendor 查找谓词各保留一份，放 `src-ui/src/graph/` 下（审查 5.1）。

## 不采纳（报告里照抄原因即可）

- 界面文案接 i18n 字典：已申明的取舍，第 5 期打包前统一处理。
- 手动 smoke 9 项：留给用户在窗口里过，本轮不做。
