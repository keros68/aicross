总判：**可交付但有未闭合项**——TASK-19 的两条主线（行形态统一 / 字号上调 / 五档缩放与权限）都已落地且有能红的测试，但接入页丢了 TASK-14 的可见额度文案、一个新增折叠按钮没有样式、缩放回退路径不幂等且布局未验证。

说明：本审查只读工作区文件，环境无 shell，无法跑 `git diff`/测试/窗口真机；涉及「哪一行是本轮新增」的判断以报告自述为准，其余均为当前树的事实。

## 未闭合项（按严重度）

1. 【中】接入页不再有任何可见的「额度用尽至 HH:MM」，与 TASK-14 固定点相抵。
   - 文件:行：`src-ui/src/components/resources/ResourcesOverlay.tsx:334`（`AgentCardModelMeta` 只有模型 ID / provider / 登录 / 来源 / 最近验证 / meta / 失败 / 备注，无额度、无状态词）；`:512`（额度只剩 `title={modelHint(model)}` 悬停）；`:563`（额度受限时只出现一个不带时间的「清除」按钮）；`src-ui/src/components/resources/resources.css:647`（`.agent-card-model-quota` 已无渲染点，死规则）；`docs/MANUAL.md:229` 仍写「资源页状态短语为「额度用尽至 HH:MM」」；`tasks/REPORT-14.md:21` 也把该短语记作接入页行内状态。
   - 复现：给任一模型写未来的 `quota_exhausted_until`，打开接入页——行内与展开的「详情」都看不到时间，只能悬停主行看 title。
   - 建议：在 `AgentCardModelMeta` 里补一行 `额度用尽至 HH:MM`（顺带可补 `状态：已验证/未验证/…`，验证状态现在也只有点 + aria-label），行内仍然只留一个点；删掉死 CSS。

2. 【中】接入页「未验证 N 个 · 展开」按钮没有任何样式规则。
   - 文件:行：`src-ui/src/components/resources/ResourcesOverlay.tsx:601` 用 `className="agent-card-unverified-toggle"`，全仓 CSS 无此选择器（左栏对应物在 `resources.css:335 .resource-unverified-toggle`，接入页同族的 `.agent-card-detail-toggle` 是 TASK-17-R1 刚补的样式，这个漏了）。
   - 复现：接入页任一 provider 组内有未验证模型 → 折叠行按浏览器默认按钮渲染（灰底、系统字体、凹陷边框）。
   - 建议：与 `.resource-unverified-toggle` 合并成一条规则（或复制），并补一条同类 CSS 断言。无法用 git 核实是否为 TASK-19 引入，但它在 TASK-19 改动的同一区块且现在确实没样式。

3. 【中】CSS `zoom` 回退不幂等：webview 缩放成功过一次后再走回退，会在残留的 webview zoom 之上再叠根元素 zoom。
   - 文件:行：`src-ui/src/settings/ui-scale.ts:58-72`（成功分支只清 `root.style.zoom`，失败分支只设 `root.style.zoom`，都不动 webview 的已设倍率）。
   - 复现：注入「第一次 resolve、第二次 throw」的 `setZoom` 替身，`applyUiScale(150)` → `'webview'`；再 `applyUiScale(125)` → `root.style.zoom === '1.25'`，而 webview 仍是 1.5，实际约 1.875 倍。现有测试 `ui-scale-19.test.ts:55-59` 各用一个全新 root/替身，抓不到。
   - 建议：进入回退分支前把 webview zoom 归 1（或记住当前生效模式再切换），并补「成功→失败→成功」序列的单元测试。另：`.resources-page` 用 `height/max-height: 100vh` + `position: fixed` 背景（`resources.css:447-453`），根元素 CSS zoom 是否把接入页底部裁掉没有窗口验证，测试只断言了样式属性被写入；若要保回退可用，建议真机过一遍 90/150，或把回退改成 zoom `#root` 并按倍率补偿尺寸。

4. 【低】关掉对话图时，模型行的悬停/读屏文本被整段替换，TASK-19 承诺进悬停的信息暂时读不到。
   - 文件:行：`src-ui/src/components/resources/ResourcesTab.tsx:420` `const hint = graphOpen ? modelHint(model) : '先打开对话图';`，同一文本还挂到 `aria-describedby`（`:444-450` 附近）。
   - 复现：关掉对话图 tab，悬停 Claude 别名行——只显示「先打开对话图」，真实 ID / provider / 验证时间都不出现（此前第二列还看得见真身）。
   - 建议：`graphOpen === false` 时用 `先打开对话图` + 换行 + `modelHint(model)`。

5. 【低】11px 残留与 `docs/PRODUCT.md:88` 新下限 12 不一致，报告只声明「未改终端字号和非本期中心页面」。
   - 文件:行：`SettingsModal.css:84`（rail 版本）、`:148`（section 标签）、`:199`（主题名）、`:374`（key 副标题）、`:556`、`:560`（前提台账辅助行）——这是本期新增控件所在的同一页；`CenterPanel.css:345/400/408/664/687/706/729/869/983/1475`（总指挥/技能等非对话图页）；`Gambit.css:316/511`、`ContributionHeatmap.css:49/128`；`global.css:1221`（`.section-title`，全仓无引用）；右栏 `ChangesBoard.css:223` 计数胶囊 10px。
   - 复现：全仓 `font-size: 11px` 仍有 20+ 处（上面列的是可见/半可见的）。
   - 建议：把设置页与保留页面的 11px 一并升 12，或在报告「没做与限制」里列明残留并同步 PRODUCT 的适用范围；不要动终端与 `pre`（见下）。

6. 【低】缩放快捷键提示与手册两种写法，术语表没登记新概念。
   - 文件:行：`SettingsModal.tsx:460` 写 `Ctrl + / - / 0`；`docs/MANUAL.md:28-29` 写 `Ctrl + = 和 Ctrl + - … Ctrl + 0`；`docs/MANUAL.md:196` 起的术语表没有「界面缩放」行（AGENTS.md 要求新增用户可见概念进表）。
   - 复现：设置页「外观」与手册对照即可见；「Ctrl + /」易被读成 Ctrl+斜杠。
   - 建议：统一为 `Ctrl + = / - / 0`（或三个 kbd 片段），术语表补一行「界面缩放 → 设置页外观 / 手册」。

7. 【低】「共用一条路径 / 启动时应用」的接线测试是字符串包含，`updateUiScale` 本身无行为测试；快捷键基准值每次从存储读。
   - 文件:行：`ui-scale-19.test.ts:61-72` 只 `toContain('UI_SCALE_LEVELS.map')` 等文本，把调用挪进注释也能绿；`ui-scale.ts:74-83` 的 `updateUiScale`/`initializeUiScale` 没有持久化键、事件 detail、apply 调用的测试；`App.tsx:109` 用 `readUiScale()` 取当前值（`ui-scale.ts:42-48` 在存储不可用时恒回 100，步进会从 100 起算）。
   - 复现：把 `onClick={() => updateUiScale(level)}` 改成注释掉再调用别的空函数、字符串留在文件里，测试仍绿；把 localStorage 置为抛错再连按 Ctrl+=，倍率不从当前档递进。
   - 建议：给 `updateUiScale` 加注入 storage + spy apply 的测试，App 侧维护内存当前值或在 handler 里缓存。

8. 【低】清洁度：死 CSS 与过期注释。`resources.css:647 .agent-card-model-quota` 无渲染点；`resources-overlay-09b.test.tsx:1-4` 头注释仍写「Claude 卡片同时显示别名和真实 ID」（正文已改成悬停/详情）；`rail-density-19.test.tsx:47` 的「无第二列」断言绑定旧类名 `resource-model-resolved`（好在同条的结构正则仍能兜住换名重加）。

## 核过没问题的要点

- 行形态统一：左栏四家共用 `ResourceModelRow`（`ResourcesTab.tsx:438-440`），接入页四家共用同一 `agent-card-model-main`（`ResourcesOverlay.tsx:512-521`），都只有「名字槽 + 状态槽」两个固定槽，无第二列；`rail-density-19.test.tsx:43-49` 的结构正则要求 `name</span><span status>…</button>$`，换名加列也会红。
- 原生无点：`VerificationMark` 对 `native` 直接返回 null（`components/resources/VerificationMark.tsx:13`），`resources-tab-09.test.tsx:151` 的 `not.toContain('verify-native')` 在删掉该分支时会红（类名由 `verify-${verification}` 生成）。
- 信息迁移：真实 ID、provider、验证状态、额度、登录状态/最近登录检查、来源、最近验证（带 smoke_model）、失败原因都进了 `modelHint`（`resources/types.ts:365-390`）；接入页详情有模型 ID、provider、登录/最近检查、来源、最近验证、meta、失败、备注（`ResourcesOverlay.tsx:334-357`）；`resources-overlay-09b.test.tsx:70-79`、`resources-tab-09.test.tsx:170-181/498-514` 覆盖。
- TASK-17 语义保留：`splitPiProviderGroups` 同时被左栏与接入页调用（`ResourcesTab.tsx:262-265`、`ResourcesOverlay.tsx:453-456`）；未登录/未检查折叠 + 「重新检查」两处都在（`ResourcesTab.tsx:276-330`、`ResourcesOverlay.tsx:610-660`）；G1 默认不勾 `logged_out` 仍在（`graph/dispatch.ts:1053-1066`、`DispatchOverlay.tsx:137-145/618-628`）；`resources-login-17.test.tsx` 全部断言未删。
- TASK-14 的派发侧额度显示未动：G1「额度用尽」分区与默认不勾（`DispatchOverlay.tsx:597-612`、`quotaBlockedNodeIds`）、结果卡「额度用尽」+ `QuotaResetLine`（`DispatchOverlay.tsx:105/820`、`DispatchBits.tsx:47`）都在。
- 字号：`.resource-model-row` 30px、`.resource-model-name` 14px、`.resource-agent-name` 14px、`.resource-agent-version` 12px（`resources.css:285/317/223/231`）；`resources.css`、`components/center/graph/*.css`、`components/right/*.css` 已无 11px；接入页与左栏的组头/折叠行都是 12px；`2px 8px` 行内小标签保持（`resources.css:729`）。
- 未动不该动的：xterm 字号仍 `fontSize: 14`（`TierTerminal.tsx:534`），角色模板代码块仍 `font: 11px/1.55`（`SettingsModal.css:355`），与任务书「终端或代码块字号不该改」一致。
- 缩放功能面：五档 `[90,100,110,125,150]`、最近档夹紧、边界停住的步进、序列化往返、启动 `initializeUiScale()`、设置页与 Ctrl+=/-/0 都走 `updateUiScale`（`ui-scale.ts:3/14-39/74-83`、`SettingsModal.tsx:446-461`、`App.tsx:107-115`；按键在 document 捕获阶段 `preventDefault`+`stopPropagation` 且 `event.repeat` 不步进）；档位/步进/回退的行为测试是注入替身的真测试，不是同义反复。
- 权限最小：`capabilities/default.json:22` 只有 `core:webview:allow-set-webview-zoom` 一条与缩放有关；Tauri 2.10.3 的 `core:webview:default` 只含 get-all-webviews / position / size / internal-toggle-devtools（crate 内 `permissions/webview/autogenerated/reference.md`），不含 set zoom，所以这条是必需且最小；`core:default` 之外没有再加别的 webview 权限。
- 文案：本轮新增可见字符串「界面缩放」「Ctrl + / - / 0」「90%…150%」无 emoji、无英文破折号、无内部词；`modelHint`/详情行全角冒号；状态词与术语表一致（已登录/未登录/未检查、未验证/已验证/验证失败/原生）。
- 缩放对左栏与终端的作用面：两者都在 webview 文档内，webview zoom 会整体放大；终端自带 `ResizeObserver → fit()`（`TierTerminal.tsx:1383-1397`），缩放导致的容器 CSS 尺寸变化会触发 refit——结构上不会错位，但报告已承认无真机窗口验证，风险点见第 3 条。