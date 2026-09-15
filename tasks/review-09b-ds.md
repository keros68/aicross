## 1. 需求完整性（契约与实现 1–6 逐条）

**契约**：`ResourceModel` 追加 `resolved_id: String` + `#[serde(default)]` — 在（src/resources.rs:82-125）；别名表四条照任务书 — 在（src/resources.rs:296-302）；旧库补齐只填空 — 在（src/resources.rs:331-343，`if model.resolved_id.trim().is_empty()`）；冒烟/首派真身覆盖 — 在（src/resources.rs:1513-1516）；失败不抹 — 在（Err 分支不碰 `resolved_id`，Rust 测试 src/resources.rs:2822-2828 覆盖）；`smoke_model` 存完整真身、与 `resolved_id` 不互替 — 在（src/resources.rs:1517、2854-2855）。

**1 官方图标**：基本在，四处偏差。
- `resources.css:46-58` 的浅色压暗规则被 `AgentBadge.tsx:20` 的内联 `--agent-tone` 覆盖 → 深浅两套色实际只有一套。见 4-A1（本轮最严重的一条）。
- `brand-marks.ts:19-20` 注释写「标志名，进 aria-label / title」，但没有任何渲染路径消费 `title`（AgentBadge 只画 `path`）。建议删字段或改注释，否则下轮会照注释去找 aria-label。
- 仓库已自带 `src-ui/src/icons-inline/pi.svg`（`CenterPanel.tsx:132`、`HistoryBoard.tsx:30` 用 `?raw` 内联，同一个 agent 的启动台图标）而本轮 pi 用 π 字形；kimi 走 simple-icons（`brand-marks.ts:39-43`）而启动台用 `icons-inline/kimicode.png`（`CenterPanel.tsx:137`）。同一个 agent 在同一窗口里两套标志，视觉上是最容易被用户抓住的不一致。任务书确实写 π，但既然本仓库已有该 agent 的现成标志（复用它不引依赖），至少要记一句不一致，或改为复用。
- 尺寸：`resources.css:38` `.agent-badge-md` 是 20px，任务书只给 18px；报告第 17 行写「sm / md 同尺寸」与代码不符。

**2 Claude 真实 ID**：在。左栏 `ResourcesTab.tsx:269-271`、接入页 `ResourcesOverlay.tsx:357-361`；相同不重复显示在 `types.ts:170-173`；悬停给来源 + 真身在 `types.ts:258-268`。

**3 长列表整理**：在（分组 `types.ts:205-217`、折叠 `types.ts:223-234`、点 `VerificationMark.tsx:20-31`），两处不完全。
- `types.ts:211` 单 provider 时把 `provider` 抹成 `''`，于是**单 provider 的 pi 在左栏和接入页都看不到 provider 名**（行内后缀已删，接入页详情块里也没有 provider 字段）。见 2-3。
- `PRODUCT.md:235` 的「状态只留一个点」没写范围，接入页仍带文字（报告已声明是刻意的）。建议把「左栏」写进 PRODUCT，否则下轮审查还会判成缺项。

**4 字体与密度**：模型名改 Sans 在（`resources.css:296-305`、`:543`）；4 倍数基本达标。遗留：`resources.css:381` `.resources-page h1 { font-size: 20px }` 不在六级阶梯（11/12/13/14/16/18）上，`resources.css:735` `.res-dialog { gap: 10px }` 不是 4 的倍数——两处都在本轮「字体与密度」视野内但没清（20px 是 TASK-07 遗留）。当前测试只断言「没有 9px」（`resources-tab-09.test.tsx:337-346`），抓不到。

**5 接入页同步**：在（同一 `AgentBadge`、同一 `providerGroups`、别名+真身）。遗留同 3（单 provider 丢 provider 名）。

**6 测试**：在（Rust 4 条 src/resources.rs:2747-2876；TS 分组/折叠/点/真身在 `resources.test.ts:129-192`、`resources-tab-09.test.tsx:158-230`、`resources-overlay-09b.test.tsx:53-95`），但有一条假绿（4-A1）+ 三处缺口（见第 5 节末）。

## 2. 逻辑正确性

**分组键与排序**：在。`modelsByAgent`（`types.ts:130-145`）verification→provider→id，`providerGroups` 组间 `localeCompare`、组内保序。一处不统一：`localeCompare` 受 locale 影响，而后端 `detected()` 用字节序 `a.provider.cmp(&b.provider)`（src/resources.rs:465-470）→ 含非 ASCII provider 名时左栏与下拉/后端顺序可能不一致。建议固定 `localeCompare(b, 'en')` 或统一成字节序。

**折叠状态不串**：基本在（键带 agent，`types.ts:241-243`；state 在 `ResourcesTab.tsx:42` 只增删键）。两处不对称：
- `expanded` 只在首次 load 时初始化（`ResourcesTab.tsx:44,57-59`），之后 `RESOURCES_CHANGED_EVENT` 带来新 agent（如刚探测出 codex）不会展开 → 新 agent 默认折叠、老 agent 默认展开。
- 组件卸载再挂回（切走左栏 tab）会重置 `initialized`，把用户手动折叠的 agent 全展开。
建议改成「按 id 集合增量补默认值」，或明确「默认折叠」。

**单 provider 不出组头**：在（`ResourcesTab.tsx:198-203` + `types.ts:211`），代价：`providerGroups([{provider:'zai'},…])[0].provider === ''` 被测试钉死（`resources.test.ts:149`），于是 provider 名在左栏和接入页都消失（左栏悬停里还有，接入页没有）。修法：group 保留 provider，只把「是否渲染组头」的判据从 `provider !== ''` 换成 `groups.length > 1`，单组时把 provider 放进接入页详情行。

**点击行插入的 @ 引用**：在（`ResourcesTab.tsx:267` → `mention.ts:14-17` → `parseSend` 有测试 `layout.test.ts:213-227`）。但这条链路本身**没有测试**（静态渲染里 `onPick` 是 no-op，`resources-tab-09.test.tsx:60-95`），provider 掉了不会红。

**resolved_id 与 id 相同不重复显示**：在（`types.ts:170-173`，两个组件都判空串）。

**pi 真身取模型半段**（src/resources.rs:318-327）：只剥「与请求 provider 完全同形（**大小写敏感**）」的前缀。
- provider 写法不同（`Zai` vs `zai`；或端点回报的 provider 与 models-store 里的名字不同）→ `resolved_id` 带前缀整串进界面，次文本里再出现一个 provider 名。
- provider 为空但 identity 带 `zai/x`（旧库/手写条目）→ 不剥，同上。
- identity 为空由调用点挡住（src/resources.rs:1514），函数本身返回空也安全。
建议剥前缀时大小写不敏感，并至少补一条测试把当前行为钉住（边界测试见第 5 节）。

**静态值与实测值不可区分**（src/resources.rs:1220-1221、331-343）：一旦写库，`resolved_id` 永久压过静态表；合并逻辑把「静态表算出来的旧值」也当「实测真身」保护起来 → 下一版修正别名表后旧库不会更新。建议加只追加字段 `resolved_source: static|measured`，或合并时判 `existing == resolved_id_for(agent,id)`（还是静态值）则允许被新表覆盖。

**hintId 撞 id**（`ResourcesTab.tsx:257-261`）：非字母数字全替换成 `-`，`glm-4.7` 与 `glm-4-7` 撞成同一个 DOM id，`aria-describedby` 指到第一条。建议用序号或 `encodeURIComponent`。

## 3. 安全与许可

**SVG 来源与许可注明**：在（`brand-marks.ts:1-17`：来源、tag、读取日期、CC0-1.0、商标归属、逃生门）。但不符合仓库既有做法：`NOTICE:70-88` 已有「THIRD-PARTY ASSETS (ICONS/ARTWORK)」段，`src-ui/public/icons/themes/LICENSES.md` 是先例，本轮只写在源文件头。注意 `AGENTS.md:19` 要求 NOTICE 里上游那段逐字保留 → 新条目要加在 NOTICE 顶部 aicross 自己的段落，别动上游段。

**内联 SVG 无脚本**：在。只渲染 `<svg><path d=…>`（`AgentBadge.tsx:24-30`），无 `dangerouslySetInnerHTML`、无 `onload`/`foreignObject`/`use`，路径来自常量不是用户输入。

**不引运行时依赖**：在。`src-ui/package.json` 未动，simple-icons 未进 dependencies（已核）。

**OpenAI 标志风险**：风险是商标，不是版权。simple-icons 的图标数据是 CC0-1.0 且 CC0 不可撤回，用 13.21.0 快照不构成版权问题；撤下是上游自设的「收标志须有品牌方许可」政策（#13944），不是收到法律通知。私用桌面工具 + 18px 指称性标识 + 旁边就有 agent 名 + 不暗示背书，`NOTICE:92-110` 已有 nominative fair use 段 → 风险低。三件事该做：① 写进 NOTICE 的 aicross 段（含「取自 13.21.0、OpenAI 已从上游 master 移除」）；② 把逃生门**测试化**（现在只是注释里一句口头承诺，断言删掉 `BRAND_MARKS.codex` 后走字母兜底）；③ 更保守的做法是把徽标表做成可关闭开关，而不是要求改代码。

**报告数据错误**（要你拍板的那张表）：浅色一列对得上（claude 5.73，我按 WCAG 算 5.72）。「深色」一列（2.99 / 3.06 / 5.16 / 3.16）不是对 `--bg-panel #202020` 的对比度，而是这些颜色对**白底**的对比度——对 #202020 的实际值是 5.22 / 5.09 / 3.02 / 4.93。kimi 那条最能说明问题：`#6E56CF` 对任何深底都不可能到 5.16（对纯黑上限 3.90）。结论不变（浅色压暗是必要的，原色对白底只有 3.0–3.2:1），但报告里「不确定处 3」（kimi 深色 3.0:1）才是真的，表格那栏是错的，且与它自相矛盾。下轮若按表格做决策会走反。

## 4. 视觉纪律与可访问性

**A1（最严重）浅色主题压暗色整条失效。** `AgentBadge.tsx:20` 把 `--agent-tone` 作为**内联**自定义属性写在同一个 `<span>` 上，而 `resources.css:46-58` 的 `[data-theme="light"] .agent-badge-claude { --agent-tone: … }` 命中同一个元素——内联声明在层叠中优先于样式表规则（自定义属性同样适用）。浅色主题下徽标仍是 `#D97757`（对白底 2.99:1），正是这条规则要修的问题。他们的测试 `resources-tab-09.test.tsx:351-358` 只断言「CSS 文本里有这条规则」，而 `:201-202` 断言「markup 里含 vendorColor 值」恰好证明内联值存在 → 两个断言同时通过、功能不生效，典型假绿。修法：已知四家的 tone 全放 CSS（`.agent-badge-claude { --agent-tone: #D97757 }` + 浅色覆盖），内联只留给未收录 agent；或用 JS 按主题取色；`!important` 不推荐。补一条能抓住它的测试：断言已知 slug 的徽标**不带**内联 `--agent-tone`。

**A2 状态点对比度**：`.`verify-dot`（`resources.css:81-91`）未验证点用 `--text-3`。本轮把状态文字删掉后，这个点成了唯一可见标识。cappuccino（`global.css:358/371`：底 #252525、点 #555555）≈2.2:1，低于非文字 3:1；dark 3.02:1 也贴线。verified（`--verify-green` 浅 #1f7a3d ≈6.0:1 / 深 --green ≈9:1）与 failed（深 5.2 / 浅 3.4）都够——只有灰点不达标。建议给点一个独立 token（`--verify-gray`）或 1px 内描边，最省事是 cappuccino/obsidian 这类深底主题里改用 `--text-2`。报告「不确定处 3」讨论了徽标 3.0:1，漏了点。

**A3 徽标色在部分深色主题也不达 3:1**：kimi `#6E56CF` 对 cappuccino `#252525` ≈2.75:1、对 mint `#142623` ≈2.94:1。浅色那套修好之后，深色这套也得按主题定，而不是按一个固定 #202020。

**A4 焦点环**：`.resource-unverified-toggle:focus-visible`（`resources.css:341-344`）沿用「2px 不透明 --accent」，方向对（避开了半透明的 `--border-focus`），但 cappuccino 的 `--accent #4a4a4a` 对 `#252525` 只有 ≈1.7:1 → 该主题下焦点环几乎不可见（obsidian 同款）。既有约定问题，不算本轮引入；建议引入 `--focus-ring` token 或环外加 1px 反向描边。

**A5 键盘可达性有缺口**：模型行 `disabled={!graphOpen}`（`ResourcesTab.tsx:264`）——disabled 按钮不可聚焦，报告说的「aria-describedby 让键盘用户读得到」在「没打开对话图」这条路径上不成立（整行 Tab 不到，读屏也跳过；部分浏览器 disabled 元素不弹 `title`）。建议 `aria-disabled="true"` + 保持 focusable，点击时 no-op 并给提示。

**其余**：`aria-label`/`role="img"`/`title` 三件齐（`VerificationMark.tsx:24-31`）；徽标 `aria-hidden`（`AgentBadge.tsx:25`）旁边就是 agent 名，不重复播报 ✓；字号 ≥11px ✓（只有 20px 越阶）；胶囊只给异常 ✓（左栏失败只给红点，接入页只 failed 用 `.verify-pill`）；`.detection`/`.resources-count` 是文字不是胶囊 ✓；「基线对齐」实际是 flex 视觉居中（`resources.css:10-18`）——观感更好，但任务书/报告要说清，免得下轮当没做。间距例外：`resources.css:619 margin-top:2px`、`:662 min-height:14px`、`:735 gap:10px`。

## 5. 代码质量与契约

**只追加**：Rust 是纯追加（字段 + `#[serde(default)]` + 常量表 + 三个函数），旧 `status`/`smoke_model`/`meta` 全留 ✓；TS interface 只加字段 ✓。报告说「纯追加 209/0」指的是 git 统计，但本轮确实删了 `ambiguousModelIds`/`modelRowLabel`（grep 零残留，是 phase 9 R1 自造、无其他引用）→ 可接受，报告措辞应写成「契约只追加，内部辅助函数有删减」。

**第 2–8 期路径无回归**：`detected()`/orchestration 的派发清单/`model-picker.ts` 未动，`resolved_id` 只是 struct 多一个键，前端忽略 ✓；`@` 文本仍带 `#provider` ✓。一处口径分叉值得写进注释：下拉仍用 `model-picker.ts` 的 `agent · provider` 标签，左栏改成了组头，两套标签体系并存是有意的，不写清下轮会有人「统一」掉。

**我无法独立复核的部分**：本环境只有只读工具（read/grep/find/ls），没有 shell，所以「cargo check 零 warning、369 测试全绿、209 insertions / 0 deletions、换行未翻转、没动 channels/dispatch.rs」只能按报告采信，没有验证。报告里已有两处与代码对不上（`sm/md 同尺寸`；深色对比度那一栏），这两处直接影响拍板，建议先改报告。

**最该补的 3 个测试**：
1. **浅色主题 tone 真的生效**（会立刻抓到 A1）：断言已知 slug 的徽标不带内联 `--agent-tone`，或 `[data-theme="light"]` 下取到压暗值。现有那条只测 CSS 文本，是「测 CSS 而不是测行为」。
2. **左栏行点击把 provider 带进 mention**：断言 `onPick` 收到 `('pi','glm-4.7','bigmodel')`，或端到端断言 GraphTab 收到 `@pi/glm-4.7#bigmodel`。现在 `onPick: () => {}`，provider 丢了也不会红——而这正是任务书点名要保住的「旧行为」。
3. **Rust `resolved_id_from_identity` 边界**：identity 无前缀 / 前缀与 provider 大小写不同 / 前缀不符 / provider 为空但 identity 带前缀。现在只测了「前缀完全同形」一条。

（可选第 4：折叠状态跨渲染不串——两个 agent 挂同名 provider，只展开 A 组 B 组仍折；store 热更新后按键不丢。）

## 是否可交付

**一句话**：可交付（有条件）——契约、别名真身、覆盖/不抹、分组折叠与 provider 引用都对，测试大体真实，但浅色主题压暗色整条失效（A1）+ 报告对比度数据错误必须先修。

缺项（按严重度）：
1. **A1** 浅色主题徽标压暗色被内联 `--agent-tone` 覆盖而完全失效，且对应测试是假绿（`resources.css:46-58` / `AgentBadge.tsx:20` / `resources-tab-09.test.tsx:351-358`）——改代码 + 改测试。
2. **报告对比度表「深色」一列数据错误**（把对白底的对比度当成对 #202020 的，与「不确定处 3」自相矛盾），会误导下一轮拍板；Codex 用 OpenAI 标志的放行也该在这份修正后的数据上做。
3. **单 provider 的 pi 在左栏与接入页都不显示 provider 名**（`types.ts:211` 抹成 `''`，接入页详情里也没有该字段）。
4. **A5** `disabled` 让模型行在未开对话图时键盘完全不可达（与「aria-describedby 让键盘用户读得到」的目标冲突）。
5. **A2** 未验证灰点在 cappuccino 只有 ~2.2:1，而本轮刚删掉状态文字、点成了唯一标识。
6. **A3** kimi 徽标在 cappuccino（2.75:1）/ mint（2.94:1）低于 3:1，dark 3.02:1 贴线。
7. **静态别名值与实测真身在库里不可区分** → 别名表修正无法传播到旧库；建议加 `resolved_source`。
8. **NOTICE 未收录 simple-icons 归属**（加在 aicross 段，不动上游段）；OpenAI 标志的逃生门只是注释里的口头承诺，没有测试。
9. 20px 越阶字号与 `.res-dialog gap:10px` 未清；pi/kimi 图标与仓库既有 `icons-inline` 资源两套并存；`BrandMark.title` 是死字段（注释与实现不符）。
10. 低：真身前缀剥离的大小写/不符边界无测试；`hintId` 会撞 id；下拉与左栏两套标签体系未记档；报告「sm/md 同尺寸」与代码不符。