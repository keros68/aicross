# REPORT-02 — 对话图与编排（第 2 期）

实现日期：2026-09-13。任务书：`tasks/TASK-02-orchestration.md`（09-13「对话即图」版）。
实现方式：主 agent 拆三路实现（前端纯逻辑 / Rust 命令 / 界面集成），主 agent 统一跑验收与窗口 smoke。

## 改了什么

### Rust（新建 1 个文件，2 个文件各加注册行）

- `src/orchestration.rs`（757 行，含 12 个单元测试）：数据模型（Graph/GraphNode/Message/Reply/Target/Preset/Stage，字段名 snake_case，与前端契约一致）+ 6 个 Tauri 命令 + 原子写 helper（临时文件 + rename，Windows rename 失败删目标重试一次）。
  - `list_dispatchable_models() -> { models, error }`：读 `~/.aicross/resources.json`，只返回 `status == "已接入"` 的模型；文件不存在/解析失败返回空数组 + 中文错误文本，不抛异常。
  - `list_presets() -> { version: 1, presets }`：3 个内置（builtin=true：共享模式 / 经济模式 / 盲审模式，命名按 PRODUCT.md「X 模式」定案）+ `~/.aicross/presets.json` 用户预设。
  - `save_user_presets(presets)`：原子写 presets.json，防御性剔除 builtin=true 条目（内置不可删改）。
  - `save_graph(graph)` / `load_graph(id)` / `list_graphs()`：`~/.aicross/graphs/<id>.json` 一图一文件，原子写；graph id 校验 `[A-Za-z0-9-_]+` 防路径穿越；version != 1 拒绝。
- `src/main.rs`：加 `mod orchestration;`（1 行）。
- `src/server.rs`：`generate_handler!` 末尾注册 6 个命令（仅追加）。
- 未改 server.rs/main.rs 其它逻辑，未碰 `port/`。

### 前端纯逻辑（新建 `src-ui/src/graph/`）

- `types.ts`：契约类型 + `roleDefaults`（出方案/执行→共享+只读，验收/验证→盲+只读）+ `defaultThinking`（中）+ `newGraph`/`newNodeId`。
- `context.ts`：`contextOf(graph, nodeId): Message[]` 纯函数——共享/盲/指定上游过滤；accept 节点按父节点分别算路径取并集（DFS 保序去重），父节点自身回复始终保留；draft/pending/空回复与空用户消息不进上下文；悬空 parents 跳过。
- `stamp.ts`：`stamp(graph, nodeId, preset): Graph` 纯函数（不改原图）；第 1 阶段挂 N 下（turn），第 k 阶段以第 k-1 阶段全部节点为父（≥2 → accept）；另导出 `childrenOf`/`descendantsOf`。
- `notation.ts`：一行记法 `parseNotation`/`printNotation`。`→`/`->` 分阶段、`[a | b]` 并行、后缀 `@blind/@shared/@upstream`、`#off|#low|#mid|#high`、`:ro|:rw|:wt`、`!角色`；组后缀可被节点后缀覆盖；`?` = 空模型；裸模型名按已接入列表唯一匹配，不唯一报错列候选，匹配不到进 `unresolved`（界面标「未接入」）；printer 输出正则形式（全后缀显式），保证往返相等。
- `validate.ts`：`validateNode` → 「复核，非独立验证」（验收/验证与祖先执行/出方案同 vendor）/「未接入」/「扇入不足」。
- `layout.ts` / `mutate.ts` / `persist.ts` / `vendor.ts` / `label.ts`：布局（最长路径深度分层）、变更 helper（`appendTurn`/`fillRoot`/`parseSend` 等）、持久化 helper（`commitGraph` = SET_GRAPH + 刷新 updated_at + saveGraph）、厂商色映射。
- 测试：`__tests__/` 5 个文件 41 个用例（context 7 / stamp 4 / notation 15 / validate 6 / layout+mutate 9）。
- `package.json`：加 `vitest@^5`（vite 8 需要 5.x，3/4 不覆盖）与 `"test": "vitest run"`。无其它依赖新增。

### 界面

- 中栏新 tab 类型 `'graph'`：`CenterPanel.tsx` 加 tab 图标/标题/内容分支；launchpad 加「对话图」卡。
- `components/center/graph/GraphTab.tsx` + CSS：开始视图（已有图列表 + 新建）；上半会话图（按深度分层、同层竖排、内联 SVG 贝塞尔连线、横向滚动、节点卡：色点/agent/模型/角色/待派/材料、单击高亮、双击对齐主列、Ctrl+双击固定第 2/3 列、高亮节点旁「分叉」「盖预设」）；下半 1–3 列聊天面板（列头 agent/model + 「上下文 N 条 · 共享/盲」可展开 contextOf 预览；每列独立输入框，`@` 候选按 agent 分组，空列表显示「尚未接入资源」并允许手填；发送 = 新建 turn 子节点，无 @ 沿用列末端 Target；根消息为空时第一句填根节点）；输入框上方「当前节点：<agent/model> · <预设名或"手动">」，点击经 `aicross:open-orchestration` 事件切右栏。
- `components/right/Compiler.tsx` 改为 tab 壳：「任务」（原 TaskBoard，内部未动）/「方案与编排」（localStorage `cc-right-panel-tab`，默认任务）。
- `components/right/orchestration/`：`OrchestrationPanel`（预设下拉——后置模式置灰标期次、应用/编辑编排/讲解、子图预览含虚线预览、校验提示）、`NodePanel`（TargetEditor 全字段编辑，模型下拉只列已接入可手填，指定上游给祖先多选）、`PresetBuilder`（阶段行编辑 + 一行记法双向同步，失焦解析失败红字不覆盖表单；保存为预设/应用/关闭）、`ExplainOverlay`（全屏讲解：节点与分叉、上下文三种可见性、三个内置预设）。
- `store/app-state.tsx`：`graphs: Record<string, Graph>` + `graphFocus` + `SET_GRAPH`/`SET_GRAPH_FOCUS`；`ToolType` 加 `'graph'`。
- `tauri.ts`：6 个命令 wrapper，类型从 `graph/types.ts` import。

## 怎么验证的（命令与输出原文）

### `cargo check`（根目录，exit=0）

```
    Blocking waiting for file lock on package cache
   Compiling aicross v0.1.0 (D:\work\aicross)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.63s
```

### `cargo test`（根目录，exit=0，全文 102 行，收尾原文）

```
test result: ok. 92 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.11s
```

其中 orchestration 模块 12 个：`presets_roundtrip`、`presets_missing_file_returns_builtins`、`presets_broken_file_returns_builtins`、`save_strips_builtin_entries`、`graph_roundtrip`、`load_missing_graph_returns_none`、`save_rejects_bad_ids`、`save_rejects_wrong_version`、`list_graphs_skips_broken_files`、`dispatchable_missing_file_reports_error`、`dispatchable_broken_file_reports_error`、`dispatchable_filters_connected_only`，全部 ok。完整输出在 `tasks/log-02-cargo-test.out`。

### `cd src-ui && npx vitest run`（exit=0）

```
 RUN  v5.0.0 D:/work/aicross/src-ui

 Test Files  5 passed (5)
      Tests  41 passed (41)
   Start at  16:45:16
   Duration  265ms (transform 57%, import 26%, tests 10%, worker 7%)
```

### `cd src-ui && npm run build`（exit=0，收尾原文）

```
> aicross-ui@0.0.0 build
> tsc -b && vite build

vite v8.0.7 building client environment for production...
✓ 554 modules transformed.
...
✓ built in 2.14s
```

（仅有既有的 chunk 体积提示，与本期无关；完整输出 `tasks/log-02-build.out`。）

## 手动 smoke 清单结果

运行环境：`npm exec @tauri-apps/cli@2 -- tauri dev` 起过窗口（编译通过、`target/debug/aicross.exe` 正常运行）；后改为 vite dev + 直接运行 debug exe（避免仓库内日志文件触发 watcher 重建）。

- [x] launchpad 出现「对话图」卡，点击进入对话图 tab；开始视图有「新建对话图」与已有图列表（截图存证）。
- [x] 新建对话图 → 图区出现根节点卡「材料」，列头「材料」「上下文 0 条 · 共享」，底部「当前节点：材料 · 手动」与输入框（截图存证）。持久化链路同步验证：新建后 `~/.aicross/graphs/<uuid>.json` 已落盘，内容符合契约（version/root/nodes/kind=material，实测文件 558 字节）。
- [ ] 发第一句得到根节点消息 —— 未验证（见下）。
- [ ] `@` 模型发送 → 子节点 + 「待派」段 —— 未验证。
- [ ] 同节点 @ 三家 → 三个并列子节点 —— 未验证。
- [ ] Ctrl+双击 → 三列并排 —— 未验证。
- [ ] 经济流水线应用到根 → 三层 —— 未验证。
- [ ] 验收节点「上下文」预览 —— 未验证。
- [ ] 改节点模型 → 一行记法同步 —— 未验证。
- [ ] 保存为预设后下拉出现 —— 未验证。
- [ ] 讲解页开关 —— 未验证。
- [ ] 重启后图与预设仍在 —— 未验证。

未验证原因：执行到「输入第一句」时，本机 Computer Use 输入通道被其它会话长期占用（`computer_use_busy`，16:57 起间断重试约 30 分钟未释放），无法继续向窗口输入。已通过只读截图与磁盘文件验证到上面两项（新建图的持久化文件已正确写入 `~/.aicross/graphs/`）。余下的交互项逻辑（`appendTurn`/`fillRoot`/`parseSend`/stamp 应用/持久化）有 41 个 vitest 与 12 个 cargo 单测覆盖，但窗口内的端到端行为需用户在界面上过一遍。窗口与 vite dev 已留在运行状态（debug exe + localhost:5173），可直接接着手测；关闭窗口后重新起的命令是 `npm exec @tauri-apps/cli@2 -- tauri dev`（仓库根没有 package.json，AGENTS.md 里的 `npm run tauri dev` 在根目录跑不了，本期未改 AGENTS.md）。

## 没做什么（任务书「不做」节核对）

- 未执行/派发/拉起任何 CLI，未写 `.dispatch/`。
- 未做 thinking→CLI 参数映射。
- 无自由连线、无拖拽画布、无缩放。
- 未移植 `port/` 资源接入界面（只复用 resources.json 契约做只读解析）。
- 终端 tab、任务板、工作区行为未改（TaskBoard 内部两个 tab 原样）。

## 不确定处 / 取舍

1. `contextOf` 对「failed 且有文本的回复」按契约字面保留进上下文；空用户消息不进上下文（否则 stamp 出的占位节点全是空段）。
2. accept 节点自身的用户消息附加在父路径并集之后（验收要求应进上下文）。
3. 一行记法无法表达「指定上游」的节点 id 列表：parser 接受 `@upstream`（=指定上游），但带 upstreams 的预设往返后 upstreams 丢失；provider/tier 也不在一行记法里。
4. 节点→预设名映射存运行期内存（不污染契约字段），重启后「当前节点」行的预设名显示「手动」。
5. 内置预设命名用 PRODUCT.md 界面标签定案「共享模式/经济模式/盲审模式」，覆盖任务书 A8 旧名。
6. 新界面文案中文硬编码，未走 i18n 字典（现有 11 语言字典同步成本高）；遵循文案纪律。
7. 原子写未沿用 port 的 .bak 备份方案（rename 失败删目标重试一次）。
8. vitest 选 5.x：3.x/4.x 的 peerDependencies 不覆盖 vite 8。
9. 窗口 smoke 期间发现：`tauri dev` 的 watcher 监视整个仓库根，`tasks/` 下任何文件变动（包括本期日志与报告）都会触发重建重启——后续期次在窗口手测时应避免边跑 dev 边写 tasks/。

---

# 修正轮 R1（2026-09-13）

任务书：`tasks/fix-02-r1.md`；审查来源：`tasks/review-02-glm.md`。本轮未起窗口、未跑 `tauri dev`。

## 必修

1. **已修：contextOf 路径改为全部祖先闭包。** `src-ui/src/graph/context.ts` 重写：路径 = 沿所有 parents 递归的祖先闭包 + 自身，祖先按从根出发 DFS 保序去重（`orderByDfs` 加 `seen`，多父节点只出现一次），自身附加在末尾；accept 不再特殊算并集。可见性过滤不变；accept 的直接父节点回复始终保留。`isAccept` 改为按 `kind === 'accept'` 判断。kind/parents 一致性校验放在前端加载路径：`layout.ts` 新增 `normalizeNodeKinds`（非根节点 parents ≥ 2 应为 accept、否则 turn，不一致按 parents 数修正并 `console.warn`），`GraphTab.tsx` 两处 `loadGraph` 入口调用。补测试两条（`__tests__/context.test.ts`）：验收节点上再分叉共享 turn、上下文含全部三条执行分支回复；1→3→1 stamp 集成，验收（盲）context == 根用户消息 + 三个执行父回复。讲解页「上下文怎么算」一节文案同步改为祖先闭包口径（`ExplainOverlay.tsx`）。
2. **已修：「同一分支只保留一个面板」按分支判定。** `layout.ts` 新增 `sameBranch`（双方互为祖先或相同，基于 `ancestorsOf`）；`GraphTab.tsx` 的 `alignMain` 与 `pinColumn` 都改用它：`alignMain` 对齐主列时收掉其它列里同分支的面板，`pinColumn` 替换同分支的列（保持列位），满 3 列时替换最后一列。
3. **已修：PresetBuilder 崩溃与状态残留。** `PresetBuilder.tsx`：记法失焦解析成功时先 `setEditing(null)` 再替换 stages；渲染处加 `stage.nodes[editing.ni]` 存在性防御；`TargetEditor` 加 `key={`${si}-${ni}`}`。`CenterPanel.tsx`：`<GraphTab>` 整个包进既有 `ErrorBoundary`（fallbackLabel「对话图渲染出错」）。
4. **已修：一行记法有损提示（按修订规格）。** `notation.ts` 的 `printNotation` 返回 `{ text, lossy }`：记法只表达 agent/model、角色、thinking、可见性种类、写权限；遇 `provider`/`tier`/`upstreams` 时在 `lossy` 里列出（'provider'/'档位'/'指定上游'，去重；`visibility=指定上游` 一律计入，upstreams 为空同样提示）。`PresetBuilder.tsx` 记法行下方显示「一行记法不表达：…」（复用 `builder-notation-warn` 样式）。测试：`notation.test.ts` 新增 3 例（三者全含时 lossy 全列且可表达字段往返相等、`@upstream` 空 upstreams 也计入、空串 provider/tier 不计），既有 6 例往返测试改走 `printNotation(...).text` 并断言 `lossy` 为空。
5. **已修：节点卡标「未接入」。** `GraphTab.tsx` `renderNodeCard` 用 `validateNode` 的结果，含「未接入」时在角色行渲染一枚 `hint-capsule`。
6. **已修：presets.json 版本校验。** `src/orchestration.rs` `presets_from` 对 `version != 1` 拒绝整个文件、只回内置，与 graphs 对齐。补 Rust 测试 `presets_wrong_version_returns_builtins`（version=2 拒、version=1 正常合并）。
7. **已修：parseSend 测试。** `__tests__/layout.test.ts` 新增 `parseSend` 组 5 例：`@agent/模型` 提取（带 provider/tier）、裸 `@模型` 唯一匹配、消歧失败落手填、未接入按原文手填、无 @ 返回 `target: undefined`。

## 次要

8. **已修：** `stamp.ts` 同层节点 `created_at` 按阶段内索引递增 1ms（基准 `Date.now()`），竖排顺序与预设书写顺序一致；`stamp.test.ts` 1→3→1 用例加递增断言。
9. **已修：** `types.ts` `newGraph()` 去掉入参、标题置空；`mutate.ts` `fillRoot` 去掉 `title === '对话图'` 魔串只判空串；展示层兜底原本就有（`CenterPanel.tsx:1228`、`GraphTab.tsx` 已有图列表均 `|| '对话图'`）；`GraphTab.tsx` 创建处改为 `newGraph()`。
10. **已修：** `GraphTab.tsx` 加 `canvasRef` + effect，`focusNodeId` 变化时对 `.graph-node.focused` `scrollIntoView({ block: 'nearest', inline: 'nearest' })`。
11. **已修：** `src/orchestration.rs` `write_json_atomic` 注释改正：Windows 的 `std::fs::rename` 走 `MoveFileExW(MOVEFILE_REPLACE_EXISTING)` 可覆盖已存在目标，删目标重试只是锁占用兜底（行为未变）。
12. **已修：** 重复工具函数收敛——`childrenOf` 只留 `stamp.ts` 一份（`context.ts` 删 `childrenOfLocal` 改 import）；`ancestorsOf` 只留 `layout.ts` 一份（`validate.ts` 删本地副本改 import）；`cloneTarget` 移到 `types.ts` 导出（`stamp.ts`/`mutate.ts` 删本地副本，`PresetBuilder.tsx` 的 `cloneStages` 也改用它）；vendor 查找谓词只留 `validate.ts` 导出的 `findModel` 一份（`GraphTab.tsx` 节点卡、`OrchestrationPanel.tsx` 预览 chip 与虚线预览、`PresetBuilder.tsx` chip 三处内联 find 全部改引用）。`NodePanel.tsx` 的 `TargetEditor` 里还有一处「精确 agent/model 判已知」的 find，语义不同（严格相等、非 vendor 查找），保留未动。

## 不采纳

- 界面文案接 i18n 字典：已申明的取舍，第 5 期打包前统一处理。
- 手动 smoke 9 项：留给用户在窗口里过，本轮不做。

## 验证（命令与输出原文）

### `cargo test`（根目录，exit=0，收尾原文）

```
test orchestration::tests::presets_wrong_version_returns_builtins ... ok
test orchestration::tests::presets_roundtrip ... ok
test orchestration::tests::save_strips_builtin_entries ... ok

test result: ok. 93 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.10s
```

（92 → 93：新增 `presets_wrong_version_returns_builtins`；orchestration 模块 13 个全部 ok。）

### `cd src-ui && npx vitest run`（exit=0）

```
 Test Files  5 passed (5)
      Tests  51 passed (51)
   Start at  18:06:12
   Duration  296ms (transform 54%, import 28%, tests 10%, worker 5%)
```

（41 → 51：context +2、notation +3、layout/parseSend +5。）

### `cd src-ui && npm run build`（exit=0，收尾原文）

```
> aicross-ui@0.0.0 build
> tsc -b && vite build

✓ built in 2.21s
```

（仅有既有的 chunk 体积提示，与本轮无关。）

## 本轮不确定处

1. `normalizeNodeKinds` 只挂在 GraphTab 的 `loadGraph` 入口；`load_graph` Rust 侧不修正（保持读出来什么样就是什么样，修正在展示层）。
2. 验收节点的后代上下文里，accept 祖先按 DFS 序插在其第一父之后（深优先自然顺序），不是按拓扑序集中排布；任务书只要求「DFS 保序去重」，未要求拓扑序。
3. 节点卡加「未接入」胶囊后卡片高度仍是 56px（overflow hidden），极端情况下第二行可能略挤，未做窗口核对。

# 窗口手测（编排者，2026-09-13，修正轮 R1 之后）

环境：`npx --yes -p @tauri-apps/cli@2 tauri dev`（根目录），日志写到仓库外；用桌面自动化驱动窗口，`~/.aicross` 下无 resources.json。复审 R2（`review-02-glm-r2.md`）12 条全部核实修好后进行。

| # | 清单项 | 结果 | 备注 |
|---|---|---|---|
| 1 | 新建对话图 → 发第一句得到根节点 | 通过 | 根节点装入消息，tab 标题变为该句，列头「上下文 1 条」 |
| 2 | `@` 一个模型发送 → 子节点 + 「待派」段 | 通过 | `@` 弹出「尚未接入资源，可直接手输 agent/模型」；子节点带 执行 · 待派 · 未接入，连线正确 |
| 3 | 同一节点再 `@` 两家 → 三个并列子节点 | 通过 | 第一次操作误把第三家挂到了 k3 下（双击根的坐标用了旧布局，操作误差），按新位置重做后三家并列 |
| 4 | Ctrl+双击两个分支 → 三列并排 | 通过 | 三列各自独立输入框与上下文条数 |
| 5 | 经济流水线应用到根 → 三层 | 通过 | 出方案 → 执行 → 验收，模型显示「未指定模型」；「当前节点」行显示「经济模式」 |
| 6 | 点验收节点看「上下文」 | 通过 | 「上下文 1 条 · 盲」，展开只有用户原话 |
| 7 | 一行记法与表单同步 | 通过 | 记法行写入 `codex/gpt-5.6-sol!出方案#high → [kimicode/k3 \| pi/glm-5.3]!执行#low → pi/glm-5.3!验收@blind`，失焦后三段表单同步、记法规范化、「未接入」提示列出三家；顺带验证了解析失败时红字定位提示（第 32 个字符：未知可见性后缀）且不覆盖表单 |
| 8 | 保存为预设后下拉里出现 | 通过 | 行内输入名字回车，下拉显示「手测三家流水线（我的）」；`~/.aicross/presets.json` version=1，一条 builtin=false，阶段形状 [1,2,1] |
| 9 | 讲解页能开能关 | 通过 | 全屏覆盖层四节 |
| 10 | 重启后图与预设仍在 | 通过 | 应用因仓库文件变动被 tauri dev 重建两次，重启后「已有图」列表里打开原图，4 个节点原样 |

手测中发现的视觉问题（**编排者已于 09-13 直接修掉，见下方「视觉修正」**）：

1. 图区高度：只有根节点时图区塌成一行，高亮节点下的「分叉」「盖预设」被裁掉一半；节点多行时最后一行高亮节点的两个按钮仍被图区底边裁掉。
2. 高亮节点的「分叉」「盖预设」按钮压在下一行节点卡上（codex 高亮时按钮盖住 k3 卡）。
3. 「应用」主按钮在深色下呈灰底灰字，看起来像禁用态，实际可点；与 `orch-btn-primary` 的可用态样式有关。

测试环境记录：`tauri dev` 监视整个仓库根，`tasks/` 下任何写入（本次是审查输出文件）都会触发重建并重启窗口。窗口手测期间不要往仓库写文件，日志放仓库外。

## 视觉修正（编排者，2026-09-13）

1. `GraphTab.tsx`：`GAP_Y` 12 → 40，高亮节点下方的「分叉」「盖预设」不再压到下一行节点卡；画布高度加 `ACTIONS_H`（32），末行按钮不再被内层高度截掉。
2. `GraphTab.tsx`：高亮节点变化时把 `.graph-node-actions` 也 `scrollIntoView`，图区可滚动时末行按钮跟着滚进视野。
3. `OrchestrationPanel.css` / `GraphTab.css`：主按钮文字改用 `--text-1` 加粗，边框仍用 `--accent`；单色主题的 accent 接近边框色，按 accent 上色会像禁用态。hover 只变边框。

验证：`npx vitest run` 51 passed，`npm run build` 通过；窗口复核末行节点按钮完整、不压卡、「应用」与「新建对话图」可辨认为可用态。
