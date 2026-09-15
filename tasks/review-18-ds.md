**总判：可交付但有未闭合项** —— 分组、默认经济模式、内置预设 id/name、盖章路径、蜂群可盖、DispatchOverlay 按名匹配、手册分层都对；但右栏下拉从原生 `<select>` 换成 `<details>` 后丢掉了「点外/Esc 关闭」，且新测试对「默认选中经济模式」和「盖预设菜单分组」的覆盖偏弱。

> 说明：只读审查。除当前工作区文件外，用主检出 `D:/work/aicross`（无 PresetGroups.tsx、右栏仍是 `<select>`）还原改动前状态做对照；未能执行 vitest/build，报告里的命令输出无法在本环境复核。

### 按严重度

1. **中 — 右栏预设下拉没有关闭途径，且展开时盖住工具条按钮**
   `src-ui/src/components/right/orchestration/PresetGroups.tsx:86-101`（外层 `<details>` 只在选中时 `removeAttribute('open')`，:97），配套 `PresetGroups.css:23-27`（`absolute; z-index:20; top:36px`）。
   问题：改造前的原生 `<select>`（主检出 `OrchestrationPanel.tsx:218-237`）点面板别处或按 Esc 会自动收起；现在没有任何 mousedown/keydown 监听，菜单会一直停着，并且从 y=36px 起覆盖 `.orch-toolbar-btns`（该行从 y=40px 开始），挡在「应用 / 编辑编排 / 讲解」上面。
   复现：右栏点开预设下拉 → 按 Esc 或点右栏空白 → 菜单不关；此时点「应用」需先点 summary 收起菜单。
   建议：仿 `graph/stamp-menu.ts` 的 `stampMenuShouldClose`，在 `PresetSelect` 挂 document 级 mousedown（target 不在 `.preset-select` 内则关）+ Escape 关闭；或把菜单 portal 出去，避免遮按钮。

2. **低 — 「更多模式」的展开态在右栏不会复位，第二次打开仍是展开的**
   `PresetGroups.tsx:61`（`<details className="preset-more">` 不受控；外层关闭时 DOM 不卸载，React 不碰 `open` 属性）。
   问题：任务写的是「默认折叠」，实现只保证首帧；用户展开过一次后，此后每次打开下拉「更多模式」都是展开的（图上盖预设菜单因为 portal 卸载，反而会复位——两处行为不一致）。
   复现：展开「更多模式」→ 选一项 → 再点开下拉 → 仍展开。
   建议：给 `preset-more` 加受控 `open` state，在外层 details 关闭或选中条目后置回 false；如果不复位是有意的，请在 REPORT/MANUAL 里写明。

3. **低 — 新测试有三处「测不出要测的东西」**
   `src-ui/src/graph/__tests__/preset-simplify-18.test.tsx`
   - `:40` `expect(html).toContain('经济模式')` 对 `PRESETS` 恒真——用例名里的「经济模式仍为当前项」没有行为断言（summary 文本 / `aria-selected` 都没查），只有 :45-46 的源码 grep。把 `selectedId` 传成 `builtin-shared` 该用例照样过。建议断言 summary 文本或经济模式条目 `aria-selected="true"`。
   - `:61-64`「图上盖预设菜单复用同一分组组件」只 grep 源码 `<PresetGroups` / `graph-stamp-menu`；GraphTab 若给 stamp 菜单传 `includeFuture`、或分组没渲染出来，测试仍红不了。建议渲染菜单内容或用 `?raw` 断言 `<PresetGroups ... />` 不带 `includeFuture`。
   - `:84-90` 手册用例只查 `### 基础/自定义/更多` 三个标题的相对位置，不查层里有没有内容；另外 `end = indexOf('## 蜂群模式')` 为 -1 时 `slice(start,-1)` 仍可能过。建议断言每层的关键词（如「默认折叠」「自定义模式 · 名称」）。
   - 报告里的「红测」失败是 `Cannot find module PresetGroups.tsx`，属于「组件不存在」级别的红，不能单独证明旧界面会违反这些断言（实际会，但没被记录）。

4. **低 — 讲解页丢了「修正轮上限 / 遗留」这句**
   `src-ui/src/components/right/orchestration/ExplainOverlay.tsx:36`（经济模式只剩「高档位出方案，低档位执行，验收…与验证并行；派盲审前…」）；主检出旧版讲解页有「修正轮只看验收那一路，到「修正轮上限」后未闭合的条目生成遗留节点」，现在只在 `docs/MANUAL.md:69-71` 保留。
   复现：讲解页 → 基础模式 → 经济模式，看不到修正轮/遗留说明。
   TASK-12 的测试（`review-defaults-12.test.ts:369-371`）只要求三条文案，仍全过；TASK-18 说「一句话各是什么」也支持缩写，所以不算硬伤——但这是讲解页信息量缩减，建议确认产品上接受，或补一句。

5. **低 — 键盘/ARIA 相对原生 select 退步**
   `PresetGroups.tsx:52-68`（`role="listbox"` 容器里混 `<div>` 组标签、`<details>/<summary>`；规划项按钮没有 `role="option"`/`aria-selected`）、`:87-89`（summary 无方向键处理）。
   事实：折叠组**能用键盘展开**（`<summary>` 原生 Enter/Space），展开后条目是 `<button>`，可 Tab 聚焦、Enter 选择，选择后菜单关闭——这一点没问题；但失去了原生 select 的方向键切换、字母跳转与 Esc 关闭，且焦点要 Tab 数次。建议补 Escape（同问题 1），必要时给 listbox 加 `aria-activedescendant` 或改成纯按钮列表语义。

6. **提示 — 没有用户预设时不出现「自定义模式」这一层**
   `PresetGroups.tsx:55`（`custom.length > 0` 才渲染组标签）。全新安装的下拉只有「基础模式 / 更多模式」两层；与任务书「分三组」字面不同，但与 `MANUAL.md:77-80`「用户保存的预设显示在基础模式之后」自洽。另外只有一个用户预设时会出现组标签「自定义模式」+ 条目「自定义模式」两行同名。建议确认是否要保留空组占位。

7. **提示 — 三处并非同形**
   `GraphTab.tsx:634` 的盖预设菜单没传 `includeFuture`，所以没有「红蓝模式（规划中）/群聊模式（规划中）」；改造前 stamp 菜单本来也不含规划项，讲解页同样不提，所以不违反任务书（置灰要求只写在右栏那条）。若按「三处一致」从严理解，需要补规划项或在对齐说明里写明取舍。

8. **提示 — 小的一致性/裁切风险**
   `PresetGroups.tsx:2-3` 用 `.ts` 后缀导入，仓库其余源码无后缀（`allowImportingTsExtensions` 开着，能编译）；`OrchestrationPanel.css:11` 的 `.orch-panel { overflow: hidden }` 会把非 portal 的 `absolute` 菜单裁在面板内——窗口极矮时下拉会被切，原生 select 弹层不受这个限制。

### 我核过没问题的要点

- **分组函数**：`presetGroupOf`（`label.ts:21-26`）基础三项 → 基础，其余内置 → 更多，非内置 → 自定义；`builtin` 优先，用户预设撞内置 id 也归自定义（测试 :33 覆盖）。
- **内置预设机器语义未动**：`src/orchestration.rs:359-485` 六个 id/name（builtin-shared/economy/blind/arena/jury/swarm）与主检出逐字一致，未改 Rust；默认选中经济模式的逻辑原样保留（`OrchestrationPanel.tsx:74-79`：先 `builtin-economy`，否则第一个）。
- **TASK-11 命名未回退**：`presetDisplayName`（`label.ts:28-31`）未改；右栏、盖章记录（`GraphTab.tsx:377`、`OrchestrationPanel.tsx:142`）用同一个函数和同一个 `!builtin` 计数。
- **盖章路径**：`applyStamp`（`GraphTab.tsx:368-381`）仍是 `stamp(graph, nodeId, effective, loadDispatchSettings().swarmLimit)` + `recordStamp` + `closeStamp`，菜单项改由 `PresetGroups` 触发但调的是同一个 `applyStamp`；`stampMenuShouldClose` 的 `.graph-stamp-menu` closest 判断对新增的分组结构仍成立，Esc/外点关闭没退化。
- **蜂群仍可选可盖**：`BUILTIN_SWARM_PRESET` 是 `builtin: true / builtin-swarm`（`swarm.ts:64-67`）→ 落「更多模式」；GraphTab 仍两处 `withSwarmPreset(f.presets)`（`GraphTab.tsx:112,163`），swarm-08b R1-6 的结构断言前提未被破坏。
- **DispatchOverlay 名匹配**：`DispatchOverlay.tsx:491-492` 仍按 `presetNameOf(...) === '经济模式'` 判经济模式；内置名未改、`recordStamp` 写入名未改，逻辑照旧生效。
- **排序与置灰**：`BASIC_ORDER`/`MORE_ORDER` 固定基础与更多两组顺序，自定义居中，规划两项 `disabled` 且在更多末尾（`PresetGroups.tsx:6-7,14-23,52-68`；置灰样式 `PresetGroups.css` 的 `.preset-group-item:disabled`）。
- **讲解页分层**：`ExplainOverlay.tsx:33-46` 先基础三模式一句一段，再单列更多三模式；TASK-12 要求的三条文案（`看产物给 PASS / NEEDS_FIX`、`派盲审前引擎先跑一次验证命令`、`沿用验收角色的默认目标`）仍在，`review-defaults-12.test.ts:367-374` 能过。
- **手册**：`docs/MANUAL.md:59-93` 三层结构、更多默认折叠说明、自定义命名说明与界面一致；术语表「内置预设」「规划中的预设」行未动。
- **文案规范**：组标签是名词短语，沿用「X模式（规划中）」写法，无内部词/emoji/英文破折号，符合 `AGENTS.md`「界面文案与命名规范」。