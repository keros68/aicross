总判：**可交付但有未闭合项** —— 11 条里实现侧 10 条已落地、能红的负例基本成立；第 4 条的真机核对按任务分工留给编排者，仍未做；另有几处低危测试/边界/记录问题（非本轮引入者已注明）。

说明：本审查只读，无 shell，未复跑 `git diff`/测试/窗口；以下依据当前工作树 + `tasks/REPORT-19-R1.md`，构建/测试数字引自报告。

## 逐条核对

1. **接入页额度与状态详情 —— 已修**
   `AgentCardModelMeta` 现已渲染 `状态：{verificationLabel(...)}`（`src-ui/src/components/resources/ResourcesOverlay.tsx:349`）与未来额度行（`:336`、`:350-352`，走 `quotaExhaustedLabel`）；主行仍只有名称槽 + 状态槽（`:520-523`）；`.agent-card-model-quota` 全仓已无任何引用（grep 无命中）。测试 `接入页详情显示验证状态与未来额度恢复时间`（`src-ui/src/resources/__tests__/rail-density-19.test.tsx:85-94`）在旧组件（无状态/额度行）下必红。

2. **未验证折叠按钮样式 —— 已修**
   `.agent-card-unverified-toggle` 与 `.resource-unverified-toggle` 合并为同一条普通/悬停/`:focus-visible` 规则（`src-ui/src/components/resources/resources.css:335-363`），含 `background: none; border: none` 与 `outline: 2px solid var(--accent)`。断言在 `rail-density-19.test.tsx:125-129`；旧 CSS 无该选择器，必红。

3. **缩放回退幂等 —— 已修（同会话内）**
   `applyUiScale` 用模块级 `WeakMap` 记模式（`src-ui/src/settings/ui-scale.ts:28`）；进 CSS 回退前若上一次生效模式是 webview，先 `await setWebviewZoom(1)`，失败则不再叠 CSS、保留原 webview 倍率（`:80-97`）；webview 成功时清 `root.style.zoom`（`:83`）。序列测试 `成功、回退、再成功时始终只有一处倍率生效`（`src-ui/src/settings/__tests__/ui-scale-19.test.ts:78-99`）断言 `webviewFactor===1`、CSS 只 1.25、调用序列 `[1.5,1.25,1,1.1]`——旧实现（残留 1.5 再叠 CSS）下前两条都会红，测试有效。
   边界：幂等只覆盖同一 JS 会话；`WeakMap` 不跨页面重载（见新发现 2）。

4. **缩放/布局真机核对 —— 部分（按分工）**
   实现侧按任务书只要求保证回退可用 + 报告标注；序列测试已有，报告两处标注「待窗口核对」「90% / 100% / 150%」（`tasks/REPORT-19-R1.md` 第 4 条、「没做与限制」）。90/100/150 三档与接入页/左栏/终端的窗口核对本身未做，留给编排者。

5. **关图时悬停信息 —— 已修**
   `const hint = graphOpen ? modelHint(model) : `先打开对话图\n${modelHint(model)}``（`src-ui/src/components/resources/ResourcesTab.tsx:420`），`title` 与 `aria-describedby` 共用该值（`:430-431`、`:443-445`）。测试 `关闭对话图时悬停提示仍保留模型详情`（`rail-density-19.test.tsx:96-111`）断言同时含「先打开对话图\nsonnet」「模型 ID：claude-sonnet-5」「状态：原生」；旧实现只替换成「先打开对话图」，必红。

6. **11px 清理 —— 已修**
   `SettingsModal.css` 六处 `font-size: 11px` 已升 12（grep 该文件无 11px 字号）；`CenterPanel.css`、`ContributionHeatmap.css`、`Gambit.css`、`global.css` 也已无 `font-size: 11px`。测试 `非终端与非代码块界面不再保留 11px 辅助文字`（`ui-scale-19.test.ts:138-150`）覆盖这 5 个文件，旧文件必红。
   误伤检查：全仓仅剩 `TierTerminal.css:339`（终端菜单 `.term-ctx-item kbd`）与 `SettingsModal.css:355`（角色模板 `pre` 的 `font: 11px/1.55`），正是任务书豁免的两类；xterm `fontSize: 14`（`TierTerminal.tsx:534`）未动。

7. **文案与术语 —— 已修**
   设置页 `Ctrl + = / - / 0`（`SettingsModal.tsx:460`）、手册同写法（`docs/MANUAL.md:29`）、术语表新增 `| 界面整体显示比例 | 界面缩放 | 设置页「外观」、本手册 |`（`MANUAL.md:227`）；旧字符串/缺行会红（`ui-scale-19.test.ts:152-166`）。

8. **测试补课与快捷键基准 —— 已修（接线断言未全替掉）**
   `updateUiScale`/`initializeUiScale` 有注入 storage/apply/eventTarget 的行为测试，断言持久化键、事件 detail、apply 调用（`ui-scale-19.test.ts:78-108`）；快捷键控制器持内存 `currentScale` + `sync`（`ui-scale.ts:119-141`），App 建控制器并订阅事件（`App.tsx:111-121`），测试 `快捷键用内存当前档递进`（`ui-scale-19.test.ts:111-124`）旧实现下会红；TASK-09b 头注释已更新（`resources-overlay-09b.test.tsx:1-2`），旧类名断言改为负向（`rail-density-19.test.tsx:70`）。遗留：设置页芯片与 App 首帧仍是源码字符串/正则断言（`ui-scale-19.test.ts:128-136`），未按「替掉」完全替换（见新发现 3）。

9. **`zoomHotkeysEnabled: false` —— 已修（键真实、生效；真机核对随第 4 条）**
   键真实：`tauri-utils 2.8.3 WindowConfig`（camelCase + deny_unknown_fields，`config.rs:1641-1868`）→ `WebviewAttributes::from(&WindowConfig)`（`tauri-runtime 2.10.1 src/webview.rs:421-426`）→ wry `with_hotkeys_zoom`（`tauri-runtime-wry 2.10.1 src/lib.rs:4631`）；Windows 落到 `SetIsZoomControlEnabled(false)` 且连带 `SetIsPinchZoomEnabled(false)`（`wry-0.54.4 src/webview2/mod.rs:572,593`），macOS/Linux 阻止注入 `zoom-hotkey.js`（`tauri-2.10.3 src/manager/webview.rs:526`）。程序化 `setZoom` 走 `SetZoomFactor`，不受影响（`wry src/webview2/mod.rs:1387-1389`），只关缩放控制/捏合，不涉及其他加速键。
   注意：Tauri 该字段默认值本来就是 false（`tauri-utils config.rs:2033`），首版默认配置下原生缩放热键已是关闭，本轮显式写死 + 测试钉住（`ui-scale-19.test.ts:135`）是加固，不是行为变化；`tauri.macos.conf.json` 未镜像该键但也回落到同一默认值。

10. **冷启动首帧 —— 已修（无 SSR/测试告警）**
    `initializeUiScale()` 已移到 `useLayoutEffect`（`App.tsx:111-112`），在绘制前执行；入口是纯客户端 `createRoot`（`main.tsx`），无 SSR，且没有任何测试渲染 `App`，故不会触发「useLayoutEffect does nothing on the server」；jsdom/客户端渲染不产生该告警。实际首帧保证只对 CSS 回退路径成立（webview `setZoom` 本身异步），但窗口在挂载后才 `show_main_window`，首个可见帧已被覆盖。

11. **文档与遗留 —— 已修**
    `docs/PRODUCT.md:270-276` 定案恢复 1–5 递增（测试 `ui-scale-19.test.ts:158-164`）；`Explorer.css` 12.5px 记入 `tasks/fix-04-r5-backlog.md:102-104` 第 60 条；报告写明多窗口不联动与 detached 需重启（测试 `:167-173`）。

## 新发现的问题

1. 【低】额度时间断言依赖本机时区（本轮新增测试）：`rail-density-19.test.tsx:89-93` 写死 `额度用尽至 15:45`，而 `quotaExhaustedLabel` 用 `Intl.DateTimeFormat` 按系统时区格式化（`src-ui/src/resources/types.ts:112-113`）。在非 UTC+8 的机器/CI 上会得到其他时间而变红；建议按同一 formatter 计算期望值或固定 TZ。
2. 【低】第 3 条的幂等只在当前 JS 会话内：`appliedZoomModes` 是内存 `WeakMap`（`ui-scale.ts:28`），页面重载/新上下文后清空；若控制器仍保留非 1 倍率且首次 `setZoom` 失败，会跳过「归 1」直接设 CSS 倍率（`ui-scale.ts:80-97`），仍有叠乘可能。需真机确认 WebView2 重载后倍率是否保留；新测试覆盖不到该场景。
3. 【低，测试强度】第 8/10 条的 App/设置页接线仍是字符串/正则断言（`ui-scale-19.test.ts:128-136`），证明不了 `useLayoutEffect` 时机与芯片 `onClick` 真接线；缩放模块本身是行为测试，此项属残留而非缺陷。
4. 【低，既有记录缺口】PRODUCT「不许 0.5px 步进」与 `SettingsModal.css:502/513`、`CenterPanel.css:858/1631/2058`、`HistoryBoard.css:52/215`、`ChangesBoard.css:223`、`global.css:1383/1389` 的 12.5/10.5/10px 不一致（均非 11px、非本轮引入）；本轮只把 `Explorer.css:364` 记入 backlog。
5. 【信息】`zoomHotkeysEnabled:false` 与 Tauri 默认等价（见第 9 条依据），报告的「关闭 WebView2 原生缩放热键」相对 HEAD 并非行为修复；无副作用问题，仅需知晓。

## 核过没问题的点

- 死 CSS：全仓无 `agent-card-model-quota` 引用，`resources.css` 无残留规则/悬挂注释。
- 行统一：左栏与接入页都是「名称槽 + 状态槽」，原生无点；结构正则可挡住换名加列（`rail-density-19.test.tsx:43-50`），旧第二列断言只保留负向检查。
- TASK-17：`splitPiProviderGroups` 两处同用、未登录/未检查折叠与「重新检查」、G1 默认不勾 `logged_out` 都在；`resources-login-17.test.tsx` 完整未削弱。
- TASK-14：G1 额度分区/`resultStatus`/`QuotaResetLine`/「清除」都在，`modelHint` 仍含额度行（`types.ts:387-389`）。
- TASK-15：`agentDisplayName` + `AgentBadge` 四个渲染面未动，`agent-display-r1.test.ts` 断言未删。
- 11px 未误伤终端/`pre`（见第 6 条）；权限只多 `core:webview:allow-set-webview-zoom` 一条（`capabilities/default.json:22`），未新增 Rust 命令。
- `useLayoutEffect` 无 SSR/测试告警（纯客户端入口，无测试渲染 App）。
- 文件:行依据均由当前树核对；`cargo check` 零 warning、450 Rust 测试、535 vitest、`npm run build` 通过均引自 `tasks/REPORT-19-R1.md`（本审查无 shell，未复跑）。