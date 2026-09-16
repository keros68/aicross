# TASK-19 审查结论

**总判：可交付但有未闭合项** —— 行统一、TASK-17 语义、字号、缩放主体全部成立且测试能红；接入页额度标注弱化、死 CSS、文档编号与文案小疵未闭合。

## 问题清单（按严重度）

1. **接入页「额度用尽至 HH:MM」不再可见，详情里只剩一个没有缘由的「清除」按钮；`.agent-card-model-quota` 成死 CSS**（中）
   - 文件：`src-ui/src/components/resources/ResourcesOverlay.tsx:334-363`（`AgentCardModelMeta` 无额度行）、`ResourcesOverlay.tsx:563-572`（清除按钮）、`src-ui/src/components/resources/resources.css:647-651`（`agent-card-model-quota` 全仓无引用）。
   - 问题：PRODUCT.md:257（TASK-14 定案）要求「资源页给该模型标『额度用尽至 HH:MM』」；现在接入页唯一可见痕迹是详情操作区出现「清除」，额度文字只剩主行 `title` 悬停（`ResourcesOverlay.tsx:512`）与 G1 的 `QuotaHint`。REPORT-19 §1 也只写「额度状态……进入悬停提示」。
   - 复现：模型带未来时刻的 `quota_exhausted_until` → 打开接入页展开详情：有「清除」按钮，无任何额度说明；悬停主行才见「额度用尽至 …」。
   - 建议：`AgentCardModelMeta` 加一行 `quotaExhaustedLabel(model)`（该函数已导入，现仅用于 `quotaClearVisible`）；随后删除或复用 `.agent-card-model-quota` 死类。

2. **缩放快捷键与 WebView2 原生浏览器加速键的交互未验证，缩放状态可能漂移**（中，未闭合项）
   - 文件：`src-ui/src/App.tsx:106-117`。
   - 问题：`keydown` 上的 `preventDefault()` 拦不住 WebView2 在页面事件之前处理的浏览器加速键（PreTranslateAccelerior）；按 Ctrl + = 可能原生缩放与 `setZoom` 双轨触发。碰巧 Chromium 步进 90/100/110/125/150 与五档重合、且 `setZoom` 是同一 ZoomFactor 的后写覆盖，实际大概率一致，但「存储档位 vs 实际 zoom」无对账，也全靠推断。
   - 复现：需起真窗口按 Ctrl + = / - / 0 观察一轮。任务书写明「不起窗口」，REPORT-19 也如实声明未做，属按规留白，但合入前值得补一次真机核对（或在 Rust 侧关掉 browser accelerator keys）。

3. **多窗口缩放不联动**（低）
   - 文件：`src-ui/src/settings/ui-scale.ts:74-78`（`updateUiScale` 只 dispatch 到本窗口 `window`）。
   - 问题：capabilities 覆盖 `main` / `island` / `detached-*`（`capabilities/default.json:5`）。每个窗口启动时各自 `initializeUiScale()` 读同一存储值，但运行中改档的事件不跨窗口，已开的 detached 窗口保持旧缩放直到重启。首版可接受，记为已知限制即可。

4. **`docs/PRODUCT.md`「用户 09-15 晚四点反馈的定案」编号乱序**（低，文档）
   - 文件：`docs/PRODUCT.md:272-276`：顺序为 1、2、3、**5**、**4**，第 5 条（TASK-19）插在第 4 条（模式做减法）之前。
   - 建议：交换两行位置，编号恢复 1-4 递增。

5. **设置页快捷键提示文案歧义**（低，文案）
   - 文件：`src-ui/src/components/common/SettingsModal.tsx:460`：`Ctrl + / - / 0` 会读成「Ctrl + 斜杠」；`docs/MANUAL.md:29` 的写法（Ctrl + = 和 Ctrl + -）是清楚的。建议提示改为「Ctrl + = / - / 0」。

6. **缩放应用挂在 `useEffect` 后，非 100% 档冷启动有一帧 100% 闪烁**（低）
   - 文件：`src-ui/src/App.tsx:106-107`。`initializeUiScale()` 在首帧后才调 `setZoom`。可在模块加载或首个布局前应用；不阻塞交付。

7. **范围外残留 11px 与 0.5px 步进，与新字号阶梯矛盾**（低，记录性质）
   - 本期范围（左栏/接入页 `resources.css`、对话图 `GraphTab.css`、右栏各 css、`Explorer.css`）内 `font-size: 11px` 已清零；但范围外仍有：`SettingsModal.css:84,148,199,374,556,560`、`CenterPanel.css` 10 处、`global.css:1221`、`ContributionHeatmap.css:49,128`、`Gambit.css:316,511`、`TierTerminal.css:339`（终端菜单 kbd）。`SettingsModal.css:355` 角色模板代码块 `font: 11px/1.55` 属代码块，可豁免。另 `Explorer.css:364` 有 `font-size: 12.5px`，与「不许 0.5px 步进」冲突；审查环境无法取 git diff，不能断定是否本期引入，若非本期则留后续清理。

## 核过没问题的要点

- **行形态统一**：左栏 `ResourcesTab.tsx:426-447` 与接入页 `ResourcesOverlay.tsx:511-519` 同为「名称槽 + 状态槽」两槽结构；原生模型状态槽为空（`VerificationMark.tsx:12` 返回 null，`resources-tab-09.test.tsx:157-158` 钉住「原生条目连点都没有」）。`resource-model-resolved` 在源码与 CSS 中均已无残留；`modelSecondaryId` 只活在 `modelHint` 与详情 meta。
- **信息没丢（除问题 1 的额度外）**：`types.ts:374-392` `modelHint` 收齐真身/状态/provider/登录/最近登录检查/来源/最近验证/失败原因；`AgentCardModelMeta`（`ResourcesOverlay.tsx:334-363`）收齐模型 ID/provider/登录状态/最近登录检查/来源/最近验证/meta/失败原因/备注；claude 别名真身进悬停有专门断言（`resources-tab-09.test.tsx:171-176`）。
- **TASK-17 语义完整**：`splitPiProviderGroups` 在左栏（`ResourcesTab.tsx:259`）与接入页（`ResourcesOverlay.tsx:465`）同用；未登录/未检查折叠 + 「重新检查」两处都在；G1 默认不勾 logged_out 与额度受限（`DispatchOverlay.tsx:127-163`，`resources-login-17.test.tsx:141-183` 未删未改）；整卡重验只挂第三方卡片（`ResourcesOverlay.tsx:477,673-682`）。
- **字号与密度**：模型行 14px / 30px、agent 名 14px 加粗、版本 12px 等宽靠右（`resources.css:285-298,317-325,223-241`）；PRODUCT.md:88 阶梯已改 12/13/14/15/16/18；行内小标签 `2px 8px` 未动；终端字号未动（`TierTerminal.tsx:534` 的 xterm `fontSize: 14` 不在改动文件里），对话图代码块（`GraphTab.css:75,81`）无独立字号、未被波及。
- **缩放模块**：五档、最近档夹紧（平局取低档，确定性）、步进边界停住、序列化往返、持久化 try/catch 兜底（`ui-scale.ts`）；设置页档位按钮与 Ctrl 快捷键走同一 `updateUiScale`，设置页监听 `UI_SCALE_CHANGED_EVENT` 同步高亮（`SettingsModal.tsx:180-186,448-458`）；`applyUiScale` 成功清 CSS zoom、失败回退根元素 zoom，两条路径均有行为级测试；快捷键有 `event.repeat` 防连发、capture 阶段挂载。权限只新增 `core:webview:allow-set-webview-zoom` 一行，未加 Rust 命令、未改契约；`@tauri-apps/api ^2.10.1` 支持 `setZoom`。
- **测试能红、无同义反复**：REPORT-19 记录了两个新文件实现前全红、权限缺失单测独红的原文；`rail-density-19.test.tsx:46` 的结构正则（状态槽必须是最后一个子元素）对任何插入的第二列都会红，CSS 断言取的是无嵌套花括号的完整规则块，改字号即红；ui-scale 测的是真函数行为。文件内容 grep 式「接线」测试（`ui-scale-19.test.ts:64-73`）偏弱但是仓库既有风格，且确实抓过漏加权限。
- **文案**：新增界面词「界面缩放」「90%…150%」「检查中…」「重新检查」均为中性说明体，按钮动词短语，无口语腔与造词；MANUAL.md 新增段落（缩放、悬停、别名）简短准确。