# TASK-02 代码审查

## 1. 需求完整性

- `src-ui/src/graph/notation.ts:265-285` — **往返不等式**：`printNode` 只打 role/thinking/visibility/write 四个后缀，`provider`、`tier`、`upstreams`（指定上游的节点 id 列表）无法进入一行记法。任务书 B 要求「任意合法 Preset → 一行 → 解析 → 与原 Preset 相等」，含这三者的预设往返后必然不相等（REPORT-02 第 3 条已自认）。至少 `@upstream` 打出来是静默有损——解析回 `指定上游` 但列表为空，用户在构建器里看不出丢了什么。建议：解析结果里带 `lossy: true` 之类的标记并在记法行下提示，或扩记法（如 `@up=A,B`）。
- `src-ui/src/components/center/graph/GraphTab.tsx:298-336` — **节点卡没标「未接入」**：A6 要求「节点上标『未接入』」，现在 `renderNodeCard` 只有色点/模型/角色/待派；`validateNode` 的提示只出现在列头（`GraphTab.tsx:453`）、预览 chip 和节点面板。图上非末端节点在界面任何地方都看不到未接入标注。建议节点卡加一枚小提示。
- `src/orchestration.rs:388-400` — **presets.json 不校验 version 值**：`PresetFile.version: u32` 无取值检查，`{"version":2,...}` 会照常合并用户预设，下次 `save_user_presets` 又静默写回 `version:1`。graphs 侧 `save_rejects_wrong_version` 做了拒绝，presets 侧标准不一致。建议 `presets_from` 对 `version != 1` 拒绝该文件（只回内置），与 graphs 对齐。
- 其余核对项与 A 节一致：`contextOf` 四种可见性（含 accept 按父分别算、并集去重、父回复保留，`context.ts:56-100`）✓；stamp 第 k 阶段以 k-1 全部节点为父、≥2 即 accept（`stamp.ts:36-38`）✓；模型只读 `resources.json`、`status=="已接入"` 过滤、失败返回空数组+错误文本（`orchestration.rs:353-385`），代码无手抄模型 ID ✓；原子写临时文件+rename（`orchestration.rs:300-338`）✓；内置预设 Rust 侧剥离（`orchestration.rs:402-411`）+ 前端不发 builtin ✓。
- smoke 清单 9 项未验证但按验收规则如实标注，不算造假，列入最终缺项。

## 2. 逻辑正确性

- `src-ui/src/components/center/graph/GraphTab.tsx:147-155` — **「同一分支只保留一个面板」没实现**：`pinColumn` 只查 `cols.includes(nodeId)`（同一节点），`alignMain` 完全不查。先固定节点 B，再双击（或 Ctrl+双击）其祖先 A 或子孙 D——同一 rootPath 上出现两列。判定应基于分支而非节点 id：新节点与某列末端同在一条 `pathTo` 链上时替换那一列。建议用 `pathTo(graph, nodeId)` 做前缀/后缀比较。
- `src-ui/src/graph/context.ts:14` — **accept 节点的后代只沿 `parents[0]` 回溯**：在验收节点上再分叉或再盖预设（经济流水线跑完后继续干活是常态路径），该后代 `contextOf` 只吃到验收第一父分支的内容，其余父分支的产物静默丢失。A3 没定义这种情形，但当前行为是一个未声明的、有信息损失的取舍（REPORT 未提）。建议：要么路径遇到 accept 时取全部祖先并集，要么在讲解页/文档里写明「验收后分支只继承第一父」。
- `src-ui/src/graph/context.ts:60` — `isAccept` 用 `parents.length >= 2` 而非 `kind === 'accept'`：kind 与父数不一致的脏数据（手工编辑过的 json）下，单父 accept 不走并集、双父 turn 走了并集，与卡片显示的「汇合」标记（按 kind）互相矛盾。本期构造路径不会触发，建议统一按 kind 判断或加载时做一致性校验。
- `src-ui/src/components/right/orchestration/PresetBuilder.tsx:59-68, 198-211` — **记法解析后 `editing` 索引不重置**：节点编辑器开着时失焦记法输入框且解析成功 → `setStages` 替换数组 → `stage.nodes[editing.ni]` 可能越界为 `undefined` → `TargetEditor` 第 41 行 `target.agent` 解引用崩溃。GraphTab 整个不在 ErrorBoundary 里（CenterPanel.tsx:1476-1478 只给 TierTerminal 包了），一崩崩整个窗口。建议 `onNotationBlur` 成功分支加 `setEditing(null)`，渲染处加 `!t && return null` 防御。
- `src-ui/src/components/right/orchestration/PresetBuilder.tsx:206` — `TargetEditor` 没加 `key`：NodePanel.tsx:3 自己的注释写明「换节点必须 key 重置」，构建器里 `editing` 切换节点时复用实例，`manual`/`customRole` 是 mount 时的初值，从自定义角色节点切到标准节点会残留错误 UI。建议 `key={`${si}-${ni}`}`。
- `src-ui/src/graph/stamp.ts:29,44` + `layout.ts:38-44` — stamp 出的同层节点 `created_at` 全相同，`layersOf` 落到 uuid 字典序决胜：盲审三个节点的竖排顺序与预设书写顺序无关且每次盖都随机。功能无损但与「并行组里谁是谁」的直觉相反。建议 stamp 按阶段内索引给递增的时间戳偏移。
- reducer（`app-state.tsx:600-603`）、stamp/context 纯函数的不可变更新、环/悬空防御（`context.ts:8`、`layout.ts:14`）：未发现问题。图不变量（根唯一、无环、无孤儿）由构造路径保证——本期没有任何删除/重挂父操作，成立。

## 3. 边界

- `src/orchestration.rs:300-333` — **原子写注释与 std 事实相反**：「Windows rename 不能覆盖已存在的目标」不对——`std::fs::rename` 在 Windows 走 `MoveFileExW(MOVEFILE_REPLACE_EXISTING)`，可以覆盖。删目标重试的分支实际近乎死代码（只在目标被占用时兜底）。行为无害，但注释会误导后续维护者。建议改正注释，重试留作锁占用兜底。
- `src-ui/src/graph/mutate.ts:48` — `fillRoot` 用魔串 `graph.title === '对话图'` 判默认标题，与 `GraphTab.tsx:139` 的 `newGraph('对话图')` 隐式耦合，任何一处改名即失效。建议 `newGraph` 把 title 置空，标题展示层再兜底显示「对话图」。
- 节点很多时：画布 `max-height:44% + overflow:auto`（GraphTab.css:102-108）横向滚动满足；但**高亮/新建节点不会滚入视野**——没有 scrollIntoView，几十个节点后分叉、双击对齐后图区仍停在原处，用户要自己找新节点。建议 focusNodeId 变化时对节点卡 scrollIntoView。
- resources.json 缺失/坏：有错误文本且有测试 ✓；`agents` 为空数组 → models 空、error 为 null，界面「尚未接入资源」仍显示（`GraphTab.tsx:489-493`）✓；graphs 目录不存在：list 返回空、save `create_dir_all` ✓；Windows 路径：图 id 白名单防穿越（含 `a\\b`、`a.b` 用例）✓。

## 4. 安全

未发现。不读 `auth.json`/`cc-switch.db`；无 shell 拼接、无动态执行；落盘内容只有图文本、附件路径（契约明示 attachments 是文件路径）与预设字段；前端无 `dangerouslySetInnerHTML`。`save_user_presets` 的 builtin 剥离同时防住了伪造内置条目回写。

## 5. 代码质量

- **四组重复工具函数**：`childrenOf`（`stamp.ts:52` 与 `context.ts:20`）、`ancestorsOf`（`layout.ts:84` 与 `validate.ts:12`，逐字相同）、`cloneTarget`（`stamp.ts:4` 与 `mutate.ts:17`）、vendor 查找谓词（`GraphTab.tsx:303-309`、`OrchestrationPanel.tsx:130-136`、`PresetBuilder.tsx:173` 三份）。建议收敛进 graph/ 下单一模块，第 3 期外发前尤其值得——`contextOf` 是要外发的核心。
- 终端 tab / 任务板：`Compiler.tsx` 纯包壳、默认 tab「任务」、TaskBoard 内部未动；CenterPanel 对 `'graph'` 走非 PTY 分支，tab 标题只加一个 case。未发现既有行为破坏。
- 文案：按钮均为 2-6 字动词短语（新建对话图/分叉/盖预设/上移/下移），无 emoji、无内部词，状态词中性（待派/未接入/扇入不足/复核，非独立验证）。中文硬编码偏离仓库既有 11 语言字典模式，REPORT 已作取舍申明，可接受但属于偏差。

## 6. 测试

验收第 2 条各组都在且断言真实：contextOf 7 例（共享/盲/指定上游/accept 并集+去重序）、stamp 4 例（1→3→1、1→1→1、非根、深拷贝）、记法 15 例（含消歧失败、非法后缀、6 例往返）、Rust 12 例（presets 读写与内置剥离、graph 往返、坏 id、坏 version）、同厂商复核 ✓。

最该补的 3 个：
1. **`parseSend` 零测试**（`mutate.ts:74-112`）——`@` 提取、消歧失败落手填、无 @ 沿用末端 Target，是发送主路径，目前完全裸奔。
2. **accept 节点作为中间节点的 `contextOf`**——把上面「parents[0] 丢失」的行为用测试钉住（无论最后定哪种规则）。
3. **stamp + contextOf 集成**：经济流水线盖在根上、给各节点填 reply 后，验收节点 context == 用户消息 + 三个执行父的回复（验收清单 smoke 第 7 条的纯函数版，现在是两端各自测、中间没接）。

---

## 对照 TASK-02 是否完整实现

**核心已实现：数据模型 8 条规则、stamp/contextOf 纯函数、三列面板、右栏编排、构建器与讲解页全部落地，cargo/vitest/build 三关自测通过；但一行记法的「任意 Preset 往返相等」未达标，且窗口端到端行为大部分未验证。**

缺项列表：
1. 一行记法对含 `upstreams`/`provider`/`tier` 的预设往返有损（B 节验收条款未满足，且无有损提示）；
2. 手动 smoke 9 项未验证（发送、三列、经济流水线应用、上下文预览、重启持久化等），按规则如实标注但仍是缺口；
3. 「同一分支只保留一个面板」按节点 id 而非分支判定（C1 条款字面未满足）；
4. 图上节点卡未标「未接入」（A6）；
5. presets.json 版本号不校验（与 graphs 不一致）；
6. 界面文案未接 i18n 字典（已申明的取舍）。

建议第 3 期动工前先补 1、3、4 与上面第 6 节的 3 个测试，`contextOf` 一旦外发，行为就得冻结了。