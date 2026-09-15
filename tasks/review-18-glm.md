# TASK-18 审查报告

**总判：可交付但有未闭合项** —— 任务书四条已定项全部落实且无功能回归，测试与构建报告为绿；遗留项集中在右栏下拉的关闭交互、空态渲染与测试同义反复，均不阻断本任务验收。

（说明：本次为纯静态审查，未重跑 vitest / build，测试绿与构建绿引自 REPORT-18.md 的记录原文；未逐字节核对行尾。）

## 问题清单（按严重度排序）

1. **右栏预设下拉没有「点外部 / Esc」关闭** — `src-ui/src/components/right/orchestration/PresetGroups.tsx:86-102`
   - 问题：`PresetSelect` 的 `<details>` 菜单只在「再点一次 summary」或「选中一项」时关闭；没有 document 级 pointerdown 监听，也没有 Escape 处理。图上「盖预设」菜单两种都有（`GraphTab.tsx:307-317` 配 `src-ui/src/graph/stamp-menu.ts:1-5`），两处菜单行为不一致。
   - 复现：右栏点开预设下拉 → 点「应用」「讲解」或面板任意处 → 菜单保持展开，遮住下方内容。
   - 建议：照 `stamp-menu.ts` 的现成模式补 pointerdown + Escape 关闭（注意排除 summary 自身，避免与原生 toggle 叠加）。

2. **预设为空时「盖预设」菜单渲染空组头** — `src-ui/src/components/center/graph/GraphTab.tsx:633-635`
   - 问题：`presets.length === 0` 时仍渲染 `<PresetGroups>`，出现「基础模式」标签（下面零项）和「更多模式」可折叠空组，然后才是「无可用预设」。
   - 复现：非 Tauri 环境（`listPresets` 失败/为空）在图上点「盖预设」。
   - 建议：`presets.length > 0` 时才渲染 `PresetGroups`，空态只留 `graph-stamp-empty`。

3. **listbox 角色用法不规范（可访问性）** — `src-ui/src/components/right/orchestration/PresetGroups.tsx:52`、`38-49`、`61-69`
   - 问题：容器 `role="listbox"`，但子元素含分组标签 div、嵌套 `<details>/<summary>` 和 disabled 普通按钮，不符合 listbox 的子元素约束；option 项也没有角色所暗示的方向键导航（现靠 Tab/Enter，可用但不匹配角色）。屏幕阅读器可能把分组头、折叠头念成 listbox 的杂项。
   - 复现：任意读屏/ARIA 校验器检查右栏下拉或盖预设菜单。
   - 建议：去掉 `role="listbox"`/`role="option"`，保留原生 button + details 语义即可；或按 combobox 模式补全。

4. **内层「更多模式」展开态不随菜单关闭而复位** — `src-ui/src/components/right/orchestration/PresetGroups.tsx:61-69`、`95-98`
   - 问题：选中「更多模式」里的项后外层菜单关闭，但内层 `<details>` 仍是 open；下次打开下拉直接是展开态。「默认折叠」只在首次挂载成立；测试也只钉了首次渲染（`preset-simplify-18.test.tsx:43-44`）。
   - 复现：右栏下拉 → 展开更多模式 → 选「蜂群模式」→ 重新打开下拉。
   - 建议：外层关闭时同时收起内层 details（非必须，属体验取舍）。

5. **第 4 条测试是同义反复，基本不能红** — `src-ui/src/graph/__tests__/preset-simplify-18.test.tsx:61-64`
   - 问题：断言 `GraphTab.tsx` 源码含 `<PresetGroups` 与 `className="graph-stamp-menu"` 字符串——只复述实现，分组如果坏了它照样绿。同文件 45-46 行对 `OrchestrationPanel.tsx` 的两处源码字符串断言同属此类（但那两行钉的是「默认选经济」这个验收点，且本仓已有对 Rust 源码做同样断言的先例，如 `review-defaults-12.test.ts:294-304`）。
   - 建议：把「复用同一组件」改成行为断言（渲染 GraphTab 菜单或抽出的渲染函数，检查分组结构），或保留字符串断言但明确它是防回退的钉子。

6. **过期注释** — `src-ui/src/components/right/orchestration/OrchestrationPanel.css:27-29`
   - 问题：`.orch-preset-select` 的 32px 高度注释以「原生 select 在 WebView2 里按 padding 算出的高度」为依据，该元素现在是被 `PresetGroups.tsx:87` 复用的 `<summary>`，不再是 select；按 `AGENTS.md`「清理本次修改造成的无用代码」的精神应同步改注释。
   - 建议：把理由改写为当前实现（summary + flex）。

7. **测试对「自定义模式」项位置的覆盖有漏洞（低）** — `src-ui/src/graph/__tests__/preset-simplify-18.test.tsx:41-42`、`53-56`
   - 问题：测试数据里只有一个自定义预设且 `userPresetCount === 1`，其显示名与组标签同为「自定义模式」，`indexOf` 命中的是先出现的组标签——自定义预设条目本身的落位实际没被断言。
   - 建议：测试里放第二个自定义预设（名字如「团队流水线」），让条目名与组标签可区分。

## 核过没问题的要点

- **分组正确**：`label.ts:21-26` `presetGroupOf` 按 `builtin:false → 自定义`、三个基础 id → 基础、其余内置 → 更多；`builtin` 优先于 id，同名自定义预设归自定义（测试 33 行钉住）。`PresetGroups.tsx:6-7` 基础顺序共享/经济/盲审，更多顺序擂台/评审/蜂群，与任务书一致；规划中两项 `disabled` 置灰置末（`PresetGroups.tsx:9-12`、`64-68`，样式 `PresetGroups.css:73-76`）。
- **默认选中经济不变**：`OrchestrationPanel.tsx:74-79` 原逻辑未动（优先 `builtin-economy`，无则第一个）。
- **内置预设未改**：`src/orchestration.rs:361-479` 六个内置 id/name 与蜂群兜底 `swarm.ts:64-83` 均未改；git status 无 Rust 文件，REPORT-18 声明 Rust 未动，一致。
- **三处一致**：右栏下拉（`OrchestrationPanel.tsx:210-217` → `PresetSelect`）、图上盖预设（`GraphTab.tsx:633-635` → 同一 `PresetGroups`，分组/排序/显示名同源）、讲解页（`ExplainOverlay.tsx:33-46` 基础三段 + 更多一段）；手册 `docs/MANUAL.md:59-92` 三层结构与术语表 189-191 行同步。规划中两项只出现在右栏下拉，与任务书第 7 行的字面范围和术语表「右栏预设下拉（置灰）」一致，不算偏差。
- **回归面**：`presetDisplayName`（`label.ts:28-31`）逻辑未动，TASK-11 的 `glossary-11.test.ts` 仍在套件内；盖预设 stamp 路径未动（`GraphTab.tsx:368-381`、`OrchestrationPanel.tsx:131-146`，均照旧走 `stamp()` + `swarmLimit` + `recordStamp`）；蜂群在「更多模式」内可选可盖（`MORE_ORDER` 含 `builtin-swarm`，选中即 `applyStamp`）；`DispatchOverlay.tsx:491-492` 的 `presetNameOf === '经济模式'` 匹配不受影响——内置经济显示名仍为「经济模式」，自定义预设本就不匹配该串，且其后有角色组合兜底分支。
- **键盘可达**：折叠组用原生 `<details>/<summary>`，summary 可聚焦、Enter/Space 展开；项是真 `<button>`，Tab 可达、Enter 触发；图上菜单 Escape/点外关闭仍在（`stamp-menu.ts`）。缺口只有问题 1 所述右栏菜单的关闭方式。
- **文案合规**：新字符串「基础模式 / 自定义模式 / 更多模式」「X模式（规划中）」（全角括号）符合 `AGENTS.md:41-52`；讲解页与手册新增句均为事实+后果的单句说明，无 emoji、无内部词（门禁/留痕/判定等未出现）。
- **测试能否红**：第 1、2、3、5、6 条是对真实渲染物/纯函数/文档结构的断言，对应回归会变红；第 5 条讲解页断言在标题缺失时 `-1 < -1` 为假、能红。弱点集中在问题 5、7 所述两处。