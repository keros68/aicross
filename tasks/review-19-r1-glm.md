核完了。以下为复审结论（本环境无 shell，未跑 `git diff` 与测试；判断基于当前工作区文件与 cargo registry 内实际依赖 crate 源码）。

## 总判

**可交付** —— 11 条中 10 条实现与配套测试均已闭合且负例在旧实现下会红；唯一留白是第 4 条真机窗口核对，属任务书明确分派给编排者的部分，报告已如实标注。

## 逐条

**1. 已修。** `AgentCardModelMeta` 现渲染 `状态：…`（ResourcesOverlay.tsx:349，四态词来自 types.ts:145-150）与未来额度的 `额度用尽至 HH:MM`（ResourcesOverlay.tsx:350-352，types.ts:106-114 只对未过期时刻返回文案）；行内仍只有名字 + 一个点（ResourcesOverlay.tsx:517-525）。死规则 `.agent-card-model-quota` 全仓已无任何出现。测试 `接入页详情显示验证状态与未来额度恢复时间`（rail-density-19.test.tsx:85-97）渲染带 2099 额度的 meta，旧实现详情区没有这两行，会红；期望时间用同一 formatter 现算，避开时区假绿。

**2. 已修。** `.resource-unverified-toggle, .agent-card-unverified-toggle` 合并基础规则（resources.css:335-351）、:hover（:353-357）、`:focus-visible` 2px outline（:359-363）；接入页按钮在 ResourcesOverlay.tsx:607 使用。CSS 断言在 rail-density-19.test.tsx:128-131，旧 CSS（仅左栏单选择器）下两个正则都匹配不上，会红。连带：4px 纪律测试改为取选择器所在完整规则块（resources-tab-09.test.tsx:738-743），与 REPORT 自述一致。

**3. 已修。** `applyUiScale` 用模块级 `appliedZoomModes`（WeakMap，按 root 记 webview/css 模式，ui-scale.ts:28、80）；进回退分支时若之前是 webview 模式，先把 webview 倍率归 1 再设 CSS zoom（ui-scale.ts:87-95）；归 1 本身失败则不叠 CSS、保留原倍率并维持 webview 模式。测试 `成功、回退、再成功时始终只有一处倍率生效`（ui-scale-19.test.ts:67-84）：setZoom 只对 1.25 抛错，断言回退后 `webviewFactor===1` 且 `zoom==='1.25'`、再成功后 `zoom===''`，并用调用序列 `[1.5, 1.25, 1, 1.1]` 钉住归 1 调用——旧实现（失败分支只设 root zoom、不动 webview）下 `webviewFactor` 会停在 1.5，该测试红。

**4. 按分工留白，报告如实标注。** REPORT-19-R1.md:10 与 ：105 写明 90/100/150 三档布局与 Ctrl 按键行为**待窗口核对**；实现者侧的回退路径行为测试已在上条覆盖。

**5. 已修。** ResourcesTab.tsx:420 改为 `` graphOpen ? modelHint(model) : `先打开对话图\n${modelHint(model)}` ``；同一文本仍挂 `title` 与 `aria-describedby` 指向的 sr-only span（ResourcesTab.tsx:430-445），键盘/读屏同样读得到。测试 rail-density-19.test.tsx:99-114 断言 `先打开对话图\nsonnet` 及模型 ID/状态行——旧三元（整段替换）下不含换行内容，会红。

**6. 已修（选了「一起升」分支）。** 全 `src-ui/src` 现仅存两处 11px：TierTerminal.css:339（终端菜单 kbd）与 SettingsModal.css:355（`.settings-role-template pre` 的 `font: 11px/1.55`）——均为任务书要求不动的终端/代码块；SettingsModal.css、CenterPanel.css、ContributionHeatmap.css、Gambit.css、global.css 及内联样式均无 11px。断言测试 ui-scale-19.test.ts:138-150 逐文件匹配 `font-size:\s*11px`。PRODUCT.md:88 阶梯表述未改（见下「新发现」第 3 条）。

**7. 已修。** SettingsModal.tsx:460 与 MANUAL.md:29 统一为 `Ctrl + = / - / 0`；术语表 MANUAL.md:227 补「| 界面整体显示比例 | 界面缩放 | 设置页「外观」、本手册 |」。断言 ui-scale-19.test.ts:152-159，旧文案 `Ctrl + / - / 0` 下 `toContain('Ctrl + = / - / 0')` 红。

**8. 已修。** `updateUiScale`/`initializeUiScale` 支持注入 storage/apply/eventTarget，行为测试断言持久化键 `'aicross-ui-scale'`、事件 type+detail、apply 调用（ui-scale-19.test.ts:86-108）——旧签名无注入点，spy 不会被调，红。快捷键基准改内存档：`createUiScaleShortcutController(initialScale)` + `sync`（ui-scale.ts:119-141），App 用 `initializeUiScale()` 返回值作初值、经同一事件回同步（App.tsx:111-124），测试 ：110-125 覆盖连续步进与 sync。resources-overlay-09b.test.tsx:1-2 头注释已改为「主行显示别名，真实 ID 进入悬停与详情」；rail-density 结构断言现以「结构正则 + 行内不得出现登录词」（:46-49）为主，旧类名仅作回归负例保留（:70）。

**9. 已修（键真实、路径完整），但有一点必须说清：该键默认值本来就是 false。** 链条逐环核实：tauri.conf.json:27 → tauri-utils **2.8.3**（本仓锁定版本）`WindowConfig.zoom_hotkeys_enabled`（config.rs:1867-1868，带 kebab alias，**Default 即 false**：config.rs:2033）→ tauri-runtime-wry **2.10.1** `.with_hotkeys_zoom(webview_attributes.zoom_hotkeys_enabled)`（lib.rs:4631）→ wry **0.54.4** Windows 下 `settings.SetIsZoomControlEnabled(...)` 且 `SetIsPinchZoomEnabled(...)` 同值（webview2/mod.rs:572、593）。所以：① 键不是编造的，配置路径真实生效；② 连带影响只有「同时关掉捏合缩放」，但默认态即如此，**净行为变化为零**，程序化 `SetZoomFactor`（webview2/mod.rs:1387-1388）不受影响（所有 Tauri Windows 应用默认即运行在 false 且 setZoom 正常）；③ 反推可知 GLM 担心的「原生缩放双轨触发」在这套栈上改动前就不成立（tauri 仅在非 Windows 注入 zoom 热键 polyfill，tauri-2.10.3/src/manager/webview.rs:525 的 `#[cfg(all(desktop, not(target_os = "windows")))])`）。测试 ui-scale-19.test.ts:127-135 只钉 JSON 值；真机按键行为随第 4 条待核。

**10. 已修，无 SSR/测试告警风险。** 初始化移到 `useLayoutEffect`（App.tsx:111-124），带完整 cleanup；本项目是 Vite SPA（main.tsx `createRoot`），无 SSR；全部测试只对叶子组件用 `renderToStaticMarkup`，没有任何测试渲染 `<App/>`，也没有 react-test-renderer，故不会触发 useLayoutEffect 服务端警告。dev StrictMode 双调用幂等（监听有清理、setZoom 同值重设）。残余：apply 走异步 IPC，zoom 落地理论上可晚于首次绘制，但窗口 `visible: false` 且双 rAF 后才显示（main.tsx:54-58），实际可见首帧更晚——与任务书认可的修法一致。

**11. 已修。** PRODUCT.md:272-276 定案恢复 1–5 递增（测试断言编号序列 `[1,2,3,4,5]`，ui-scale-19.test.ts:160-161）；多窗口不联动写入 REPORT-19-R1.md:17 与 ：105-106；Explorer.css:364 的 `12.5px` 仍在源码，已记 backlog 第 60 条（fix-04-r5-backlog.md:102-104），断言在 ui-scale-19.test.ts:162-163。报告存在性断言（:166-172）在报告写出前单跑会红（REPORT 自录了 ENOENT）。

## 新发现的问题（按严重度）

1. 【低】**第 9 条报告表述偏强**：REPORT-19-R1.md:15 称显式 `zoomHotkeysEnabled: false` 「关闭 WebView2 原生缩放热键，避免双轨触发」——实测 tauri-utils 与 wry 的该开关默认值均已是 false（tauri-utils config.rs:2033、wry lib.rs:845），此行是把默认态写显式，净行为不变；「避免双轨」的保障在改动前就存在。无害，但报告未说明这一点，结论「快捷键真机行为待窗口核对」仍然有效。
2. 【低】**第 3 条的未覆盖边界**：若页面重载后 WebView2 残留上次 zoom 因子（可跨导航保持）且重启后首次 `applyUiScale` 即失败，`appliedZoomModes` 为空 → 不走归 1 分支 → 直接叠 CSS 倍率，出现一次性双倍率。现有实现与测试都不覆盖此序列。实际风险低：setZoom 权限是静态配置，上一会话成功则本会话不会突变为拒绝。
3. 【低】**rail-density-19.test.tsx:46 结构正则的「能红」范围比盲审描述窄**：`[\s\S]*<\/span><\/button>$` 为贪婪匹配，状态槽**之后**再插入一个 span 仍会通过；只有插在 name 与 status 之间才红。非本轮修正项（该条只要求更新旧类名绑定，已做），仅修正「换名加列也会红」的说法。
4. 【信息】PRODUCT.md:88 字号阶梯未记「终端与 pre 例外」；本轮清理后非终端界面已无 11px，仅剩 TierTerminal.css:339 与 `.settings-role-template pre`，字面上与阶梯条目的绝对表述不一致，但属任务书明确保留范围。

## 核过没问题的点

- 第 6 条回归面：TASK-17 provider 分组两处同源（ResourcesTab.tsx:259-261、ResourcesOverlay.tsx:472），未登录/未知 provider 折叠 + 「重新检查」两处都在（ResourcesTab.tsx:278-328、ResourcesOverlay.tsx:621-662）；resources-login-17.test.tsx 不在本轮改动清单。
- TASK-14 回归面：DispatchOverlay.tsx 本轮未改；`QuotaResetLine`/`resultStatus`/`quotaClearVisible` 断言未动（resources-overlay-09b.test.tsx:175-197），MANUAL「恢复时间」术语断言在（:199-202）；接入页详情的「清除」按钮旁现在有额度文字说明缘由（ResourcesOverlay.tsx:350-352、569-578）。
- TASK-15 回归面：左栏 agent 摘要仍是 `AgentBadge` + `agentDisplayName` + 弱化版本（ResourcesTab.tsx:201-203），接入页同款徽标断言在（resources-overlay-09b.test.tsx:63-67、resources-tab-09.test.tsx:161-170、385-437）。
- 缩放接线完整：设置页档位按钮与 App 快捷键都走 `updateUiScale`（SettingsModal.tsx:454、App.tsx:113），设置页监听 `UI_SCALE_CHANGED_EVENT` 同步高亮（SettingsModal.tsx:184-185），控制器经同一事件 sync 内存档（App.tsx:115-119）。
- 权限最小未破：capabilities/default.json:22 仍是唯一缩放权限 `core:webview:allow-set-webview-zoom`，测试钉住（ui-scale-19.test.ts:134）。
- 11 条各自的测试负例我逐条推演过在旧实现下均会红（上文明注）；我未实际运行 vitest/cargo，REPORT-19-R1.md:36-77 的 cargo check 零 warning、cargo test 439 通过、vitest 535 通过、build 成功均为报告自述，未经本审复现。