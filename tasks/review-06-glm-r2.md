复核完毕。以下为复审结论。

# TASK-06 修正轮 R1 复审

## 逐条核对（11 条）

| # | 修正项 | 结论 | 依据 |
|---|---|---|---|
| 1 | 背景图旧值 | **已核实修好** | `app-state.tsx:673-676` 启动无条件 `removeItem('cc-bg-path'/'cc-bg-type')`，`bgPath` 固定空（`:659`）；`CenterPanel.tsx:1263-1273` 不再读 `state.bgPath/bgType`（`bgType` 为字面量 `'none'`，`bgUrl=''` 仅作 TierTerminal 稳定 props），`launchpad-bg` 渲染层 JSX 已删；全仓 grep `bgPath` 仅剩 store 定义与测试，无渲染链残留。 |
| 2 | 多语言死键 | **已核实修好** | diff 确认 11 个语言文件（de/en/es/fr/ja/ko/pt/ru/vi/zh-CN/zh-TW）三组键全删；grep `task.greeting|task.welcome_note|task.show_guide|makeWelcomeNote|onShowGuide` 及 `Feeling ambitious|Großes|масштабное|usage guide|大事|攻略` 在 src-ui 全部零匹配；`task-types.ts` 的 `makeWelcomeNote` 已删，无任何调用点，不存在运行期取键路径。 |
| 3 | `tool: null` 瞬态 | **已核实修好** | `CenterPanel.tsx:925` 兜底分支直接 `ADD_TERMINAL { id, tool, folderPath: cwd ?? null }`；`app-state.tsx:400-405` ADD_TERMINAL 将 active 指向新 session，随后的 `SET_TERMINAL_TOOL`（`CenterPanel.tsx:937`）幂等；全仓已无 `tool: null` 创建点（`:781` 的 `isLaunchpadMode` 仅读 legacy 会话）。 |
| 4 | 重复表达式收敛 | **已核实修好** | `isEmptyGraphSession` 覆盖全部判定点：`CenterPanel.tsx:782`（探测）、`916/919`（启动复用）、`1490`（渲染门）；`createDefaultGraphSession` 覆盖三处字面量：`app-state.tsx:413`（CLOSE_TERMINAL 兜底）、`:747`（初始状态）、`CenterPanel.tsx:899`（新建 tab）。grep `tool: 'graph'` 仅剩工厂自身（`app-state.tsx:161`）与受保护目录 GraphTab 的 SET_TERMINAL_TOOL 派发（非 session 字面量），无漏网。 |
| 5 | 终端下拉受控开合 | **已核实修好** | `CenterPanel.tsx:784-804`：仅打开时挂 document `mousedown`/`keydown`，Escape 与外点关闭，cleanup 双双摘除（卸载安全）；`:786-789` settingsOpen 或离开空 graph tab 即关闭；选中工具/文件夹的 onClick 先关再动作。与 `RecentFolderMenu`（`:36-49`）同模式；少了它的 `setTimeout(0)` 防抖，但本菜单由 summary click 打开、mousedown 先于 click，监听器挂上时开启点击已结束，无自关闭问题。 |
| 6 | 胶囊审计 | **已核实（结论成立）** | 报告写了查过及结论；抽查一致：TaskBoard 状态用圆点（`TaskBoard.tsx:658` task-section-dot、`:293` 注释红绿灯），空态按钮为实心非胶囊；Explorer badge 是大小/diff 计数；remote-history-pills 是可操作连接项；`hint-capsule` 仅在受保护 graph/orchestration 显示「未接入」等异常态。 |
| 7 | tab 圆角 8px | **已核实修好** | `global.css` `:root` 与 `[data-shape="soft"]` 均改 `8px 8px 0 0`（diff 两处），`CenterPanel.css:39` 示例注释同步。 |
| 8 | 注释失真 | **已核实修好** | `CenterPanel.tsx:602`（usage help）、`:1639-1643`（project usage page，链接改 aicross）、`app-state.tsx:216`（listener 位置改 App），三处均与现状一致。 |
| 9 | 条件写存储 | **已核实修好** | `index.html:28-31` 仅在值不同时写 `cc-theme`/`cc-shape`（非法主题迁移只写一次）；`app-state.tsx` init 对 `cc-shape`/`cc-icon-theme` 均为 `!==` 才写。 |
| 10 | 两个测试 | **已核实修好** | `app-state.test.ts:163-191` 断言了规则本身：sakura→dark 且写回、bg 旧键删除、light 保留且匹配值不重写、缺省 dark/soft/outline——非仅渲染。`TaskEmptyState.test.tsx:104-119` 断言文案、点击回调次数、list/note→`handleAdd`、prompt→`handleAddPrompt`。`vite.config.ts:6` 的 `import.meta.url` 推导出与原 `__dirname` 相同的 src-ui 目录，且只用于 `build.rollupOptions.input`——`tauri dev` 走 `devUrl:5173` 不经此路径，`tauri build` 的 `beforeBuildCommand` 为空、产物取 `frontendDist: ./src-ui/dist`；编排者实测的 `npx vitest run`（默认 config 打包路径）与 `npm run build` 恰好覆盖了两种配置加载方式，路径解析无回归。 |
| 11 | 不采纳（TitleBar 中文 aria-label） | **理由成立** | 上一轮审查自己已判定「符合『新增文案直接写中文』成文约定，记录即可，不强制返工」；且 `title` 中保留了快捷键提示（`hk.left`/`hk.right`），信息未丢失。 |

## 回归检查（修正是否引入新缺陷）

- **graph-shell 包裹**只作用于 `t.tool === 'graph'` 分支，TierTerminal/diff tab 分支原样；窗口复核覆盖默认页（即 graph tab）。终端 tab 无回归。
- **Alt+W**：`App.tsx` 中 `matchHotkeyScheme` 返回非 left/right 直接 return，不 preventDefault、不 dispatch，ActiveGambit 已卸载，无副作用。
- **任务板**：FAB 与三视图空态统一走 `getTaskNewHandler`，prompt 侧 `onNew` 已接（`TaskBoard.tsx` diff），接线与测试断言一致；欢迎笔记种子移除后 `cc-tasks-seeded` 无残留引用。
- **编译验证**：`tsc -b` + `vite build` 通过（首次构建抓出的 `setBgUrl` 残留引用已删干净，当前源码 grep 无 `setBgUrl`）。
- 无害残留（非缺陷，属「功能代码不删」范围）：reducer 的 `SET_BG`/`CLEAR_BG`（`app-state.tsx:470-473`，已无派发入口）、`App.tsx:154-156` 的 wallpaperOpacity CSS 变量同步、未挂载的 `Gambit.tsx` 技能 pill。

## 结论

**可以并回 main** —— 11 条修正全部核实到位，56 测试与构建通过，无新缺陷，未闭合项均为记录在案的非阻塞观察项：

- TitleBar aria-label 硬编码中文，对 en 用户是可感知回退（约定允许，已记录）；
- 背景设置的死代码链路（SET_BG/CLEAR_BG/wallpaperOpacity）留存待第 3 期顺手清理；
- Explorer 更新检查无超时（上一轮已记小项）；
- 主 bundle >500kB 的既有构建警告。