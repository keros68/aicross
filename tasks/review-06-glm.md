# TASK-06 审查

## 1. 需求完整性

**A 中栏**：新 tab 与兜底 session 均为 graph（CenterPanel.tsx:869、app-state.tsx:402、752）✓；起始视图「打开终端」下拉过滤正确——`tool.type === 'ai-cli'` 恰好排除 four/three/two-split 与 installer（CenterPanel.tsx:601-625 中 terminal 也标 'ai-cli'，被 `key === 'terminal'` 显式收进），四分屏等直接不露出，符合任务允许项 ✓；A3 文案在 GraphTab.tsx:243 原样沿用 ✓。

- **CenterPanel.tsx:898** — `applyLaunchRequest` 兜底分支仍 `ADD_TERMINAL { tool: null }`。问题：这是全仓最后一个 tool:null 会话创建点，靠同一次事件循环里 910 行的 `SET_TERMINAL_TOOL` + React 批处理才不闪出 launchpad；第 3 期任何人改动这段或批处理语义，launchpad（工具卡网格、热力图、「识别不到？切换模型」）就会复活。建议：改成 `session: { id, tool, folderPath: cwd ?? null }`，910 行对该分支幂等可留。
- LaunchpadGrid 代码整体保留但已不可达（仅 898 一处瞬态），符合「功能代码不删」。

**B 右栏**：默认「方案与编排」+ 新存储键 ✓；空态「还没有任务」+「新建」且三视图（list/note/prompt）都接了 onNew ✓；修改记录等 tab 未动 ✓。

- **src-ui/src/i18n/en.ts:62-64（及 fr/de/es/ja/ko/ru 同键）** — `task.greeting.*` 死键仍存问候语与「大事」等价文案（"Evening. Feeling ambitious?" / "Etwas Großes geplant?" / "Что-нибудь масштабное?"）；en.ts:152 `task.show_guide` 仍是 "View the usage guide"。问题：TASK-E 要求全仓搜改；TaskEmptyState 已不引用，属死键，但 zh 侧把同批键值改成了「还没有任务」「新建」（zh-CN.ts:60-62、149），键名与值语义错位，比纯死键更误导。建议：删除 `task.greeting.*`、`task.show_guide`、`task.welcome_note` 全语言死键与 task-types.ts:23 `makeWelcomeNote`——删废弃键不属于「不改键名」的约束对象。

**C 标题栏**：合并输入按钮、多 agent 布局、常驻快捷键提示均移除；Alt+W 不再响应（App.tsx:104-106 只处理 left/right）✓；左/右/设置/更新/窗口键保留 ✓。
- **app-state.tsx:205** — 注释仍说监听器 "in ActiveGambit"，已失真（监听器移到 App.tsx）。建议顺手改，防第 3 期按注释找错位置。

**D 主题**：收成深浅、默认深、旧值迁移 ✓；形状固定 soft、图标固定 outline ✓；字体栈与 tabular-nums ✓。

- **app-state.tsx:667-682 + CenterPanel.tsx:1238-1269** — **背景图只删了入口，没断渲染链**：getInitialState 仍读 `cc-bg-path`，CenterPanel 仍把它转 URL 渲染背景层。设置里清除按钮已删，存过背景图的老用户会继续看到它且界面无处可关。REPORT-06「背景图……均不再露出」与实际不符。建议：迁移同主题处理——init 时忽略并 `removeItem('cc-bg-path'/'cc-bg-type')`，bgPath 置空。
- **D4 胶囊审计未执行**：「检查现有界面里滥用的胶囊」无任何对应改动，报告也无结论。可见面里未发现明显滥用（hint-capsule 在保护目录内动不了），但应把「查过、结论无」写进报告，否则视为缺项。

**E 文案**：用户可见面（标题栏、空态、下拉、设置）已中性 ✓。残留见上两条 i18n 项，另：
- **CenterPanel.tsx:597** — 注释仍写 "Coffee 101"，就在本期改掉的 602 行标签正上方；**CenterPanel.tsx:1635** — 注释仍说 "Claude Code course on coffeecli.com"，而紧邻代码已改指 aicross 仓库。两处是本次改动直接造成的注释失真，应改。
- TitleBar aria-label 原为英文模板，现硬编码中文（TitleBar.tsx「切换左栏」等）——符合「新增文案直接写中文」的成文约定，但对 en 用户是可感知回退，记录在案即可，不强制返工。

## 2. 逻辑正确性

- **主题迁移一致性**：未发现问题。index.html 先行把非 'light' 旧值写回 'dark'，getInitialState 再读必合法；'light' 两处都保留；SET_THEME 仅 UI 两值入口，无写回覆盖用户选择的路径。首屏脚本先于 React，无闪烁。小项：index.html 无条件写 cc-shape、init 无条件写 cc-shape/cc-icon-theme，每次启动写存储，无害冗余。
- **graph 兜底后的终端路径**：PTY 仅在非 graph 分支创建；diff tab 在 TaskBoard 与 tool 无关；`isToolPickerMode` 扩到空 graph 是有意行为（供下拉的 CLI 检测），激活空 graph 时一次 300ms debounce 扫描可接受。真正的问题就是上面 898 行的 tool:null 瞬态。
- **applyLaunchRequest 复用空 graph tab**（889-896）：行为正确，外部 `launch --tool` 顶掉未动过的起始 tab、对话中的 tab 不会被抢占。
- **右栏存储键**：新键 `aicross-right-panel-tab` 正确；旧键被忽略属本期意图（默认回到 orchestration），旧键残留 localStorage 无害。
- **快捷键监听**：App.tsx 捕获段监听 + cleanup 完整；ActiveGambit 卸载后全仓无第二处活跃监听，无双触发；`event.repeat` 先吞后判，按住不反复切换，与注释一致。
- **更新检查**（Explorer.tsx:604-624）：tag_name 去前缀、段比较、403/离线/非 JSON 均静默跳过，失败处理成立。小项：匿名 GitHub API 限额 60 次/h/IP，每次启动一次请求，单机够用；无超时，网络挂起时 hasUpdate 只是不亮，无阻塞。

## 3. 合规

未发现违规。LICENSE/NOTICE/TRADEMARKS.md 未动；README.md:5 fork 声明在。app-state.tsx:1 与 global.css:1 删掉的是产品名标题注释，非版权声明，属改名换标义务范围，与「不删版权头」不冲突。coffeecli.com 用户可达链接已全部替换（设置反馈、使用说明、自更新），残留 5 处全为注释（CenterPanel.tsx:264、542、575、1635；tauri.ts:297）——tauri.ts:297 描述的 Rust 下载命令仍存在且注释如实，可留；CenterPanel.tsx:1635 需改（见维度 1）。`[COFFEE-DONE:pane]` PTY 标记与图标枚举值 'coffee' 是与 Rust/存储耦合的内部协议值，**不应改**，保留正确；'gambit.title' 只改了 en，其余语言同键旧值在未挂载组件里，不可见。

## 4. 视觉纪律

token 主体达标：单一低饱和强调色、发丝边框、radius md/lg 归一 8px、Sans/Mono 分工、body tabular-nums、阴影仅用于终端下拉浮层。

- **global.css [data-shape="soft"] 块 `--shape-radius-tab: 10px 10px 0 0`**（diff 行 1538 附近）——tab 是容器，纪律规定 8px，10px 是旧值的漏网。建议改 8px。
- 旧 16 套主题 CSS、`--green`/`--red` 状态色保留：前者任务明示可留，后者属状态语义非强调色，未发现大面积填充或阴影分块。

## 5. 范围

未碰 `src-ui/src/graph/`、`components/center/graph/`、`components/right/orchestration/`（gitStatus 快照与 patch 内容一致，18 个 src-ui 文件 + tasks/）；未改 Rust、依赖、port ✓。

与第 3 期（graph 接派发引擎）的冲突：
- **app-state.tsx** 改动点：402（CLOSE_TERMINAL 兜底）、608-635（VALID_THEMES/init 迁移）、752（初始 session）。
- **CenterPanel.tsx** 改动点：602、774-782、790、843、866-872、889-898、1476-1528、1639。
- **高风险**：1476-1528 的 graph-shell 包裹改变了 GraphTab 分支的缩进与结构，第 3 期对同一渲染分支的 hunk 几乎必然文本冲突；session 字面量 `{ tool: 'graph', toolData: '' }` 在 402/752/869 三处重复，第 3 期要给 toolData 换成图数据模型时三处语义冲突。
- **建议**：第 3 期动手前把「空 graph 起始视图」判定收敛为单一 helper（CenterPanel.tsx:890/892/1476 三处同义表达式），并把默认 session 字面量收敛到一处工厂函数。风险中等，但报告已按要求列明改动行，可控。

## 6. 测试与验证

沙箱外复跑 51 passed、build 通过——现有改动可交付。本期主体是呈现层，大面积补测试不值；最该补 2 个：

1. **主题/形状/图标迁移**（app-state getInitialState）：`cc-theme='sakura'` → currentTheme 'dark' 且写回；'light' 保留；缺省 → dark/soft/outline。纯 localStorage 逻辑，jsdom 可测，钉死「旧值迁移到深色」这一决策，防第 3 期改 init 时回归。
2. **TaskEmptyState 及三处接线**：渲染「还没有任务」+「新建」，点击回调；断言 list/note 传 `handleAdd`、prompt 视图传 `handleAddPrompt`（TaskBoard.tsx:645、654、830 附近）——prompt 侧 onNew 是本期新接的，最易在后续重构中接错。

## 结论

**TASK-06 主体实现完整、可交付，但有 2 个实质缺项和 3 个收尾项。**

缺项清单：
1. **背景图旧值仍渲染且无法清除**（app-state.tsx:667-682、CenterPanel.tsx:1238-1269）——REPORT 中「不再露出」的表述不成立，需迁移清除；
2. **D4 胶囊审计未做也未记录结论**；
3. 多语言死键里残留问候语/攻略/「大事」文案（en.ts:62-64、152 及 6 个语言文件），zh 侧键值语义错位；
4. 收尾：CenterPanel.tsx:898 tool:null 瞬态、1635/597 注释失真、`--shape-radius-tab` 10px；
5. 验收项 2（窗口截图）未做，报告已如实标注「未验证」——合并前建议补默认页、右栏、设置页、深浅四张。