# TASK-1B — 界面对齐到原型（插在第 2 期之前）

## 背景

第 0、1 期实现时，`docs/proto/index.html` 是旧版。2026-08-31 原型做了一轮结构与视觉重做，`docs/PRODUCT.md` 新增了「视觉纪律」一节并修订了「形态」。本期只做一件事：**把已实现的界面对齐到当前原型**。

**基准文件**：`docs/proto/index.html`（其 `<style>` 块与 `renderResourceTree` / `renderAgentCards` 两个函数）、`docs/PRODUCT.md` 的「视觉纪律」与「形态」两节。有冲突时以 `PRODUCT.md` 为准，原型是它的实现示例。

## 边界

- **只改前端**：`src-ui/src/styles.css`、`src-ui/src/App.tsx`、`src-ui/src/components/*.tsx`。
- **不改 Rust**，不改 `src-tauri/` 下任何文件，不改 `api.ts` 的调用签名。
- **不加功能、不删功能**。现有的工作区 / 资源 / 记录 / 终端 / 修改记录 / 任务列表 tab 全部保留，开关行为不变。
- **不引 CDN、不引 CSS 框架、不加 npm 依赖**。桌面应用离线可用，Google Fonts 一律不用。
- 不 commit，不 push。

## A. 字体

原型用了 IBM Plex Sans / IBM Plex Mono，走的是 Google Fonts。桌面端不能这么干。本期用系统栈，两条：

```css
--font-sans: "Segoe UI", "PingFang SC", "Microsoft YaHei", system-ui, sans-serif;
--font-mono: ui-monospace, "Cascadia Mono", Consolas, "SF Mono", monospace;
```

打包 IBM Plex 字体文件（OFL-1.1，可打包）留作后续，本期不做。

## B. 视觉纪律（照抄 `docs/PRODUCT.md` 的「视觉纪律」表）

逐条落到 `styles.css`：

1. **字体分工**：正文与标题用 `--font-sans`。结构性文字用 `--font-mono`：模型 ID、provider 与 agent 名、路径、命令、任务 ID、版本号、token 数与耗时；数字加 `font-variant-numeric: tabular-nums`。**中文状态词仍用 sans**——等宽对中文字形无效，套上去只会走 CJK 回退、字形发脏。现有代码里凡是给整个中文区域上等宽的，一律改掉。
2. **分块**：1px 边框 + 背景差，不用阴影。阴影只留给浮层（下拉、弹窗、抽屉）。**边框只给真容器**（栏、消息气泡、弹窗、卡片区块）；列表行不做容器，只用 hover 背景差分。
3. **圆角**：容器一律 8px，列表行 4px，胶囊标签 999px。色点等图形不受此限。
4. **强调色**：全局只有一个 `--accent`，用于主按钮和选中态。
5. **状态胶囊**：胶囊只给需要快速扫描的异常状态（待冒烟、冒烟失败、运行中）。常态与完成态用色点或纯文字。
6. **间距**：内边距与间隙取 4 的倍数。例外只有行内小标签，统一 `2px 8px`。
7. **字号阶梯六级**：11 辅助 / 12 次要 / 13 正文 / 14 基准与栏目标题 / 16 弹窗标题 / 18 页标题。**不许出现 0.5px 步进**，改完自查一遍 `styles.css` 里所有 `font-size`，只能是这六个值。
8. **图标**：统一线框，颜色随文字色。不用实心图标、不用 emoji。

深浅色两套 token 都要覆盖，写法照原型：浅色定义在裸 `:root`，`@media (prefers-color-scheme: dark)` 里用 `:root:not([data-theme="light"])` 包住，再用 `:root[data-theme="dark"]` 覆盖一遍，让手动切换在两个方向都赢。

## C. 外壳

对照原型的 `.topbar` / `.tabbar` / `.pane` / `#workbench`：

- **chrome 走灰底（`--bg`），中间内容面走白（`--surface`）**，边界靠 1px 线。两侧栏（资源、编排等）属于 chrome，背景用 `--bg`；中栏内容面用 `--surface`。
- **顶栏**：方形品牌标记（22px、4px 圆角、`--accent` 底、等宽两字母）+ 产品名 + 1px 竖分隔线 + 等宽项目名 + 弹性空隙 + 右侧图标按钮。图标按钮 28px、8px 圆角、透明底，hover 才出边框和底色。
- **顶栏右侧加一个深浅色切换按钮**：切换 `document.documentElement` 的 `data-theme` 在 `"light"` / `"dark"` 之间；当前值存进已有的 settings（`api.saveSettings`），启动时读回来。settings 里没有对应字段时不要新增 Rust 结构，用前端 `localStorage` 兜底，并在 REPORT 里说明选了哪条。
- **中栏标签栏**：标签是等宽 12px，`8px 8px 0 0` 圆角，`margin-bottom:-1px` 压在分隔线上；选中态白底 + 边框、下边框与内容面同色，看起来是连成一片的。未选中透明底、`--text-3` 文字。禁用态 `opacity:.45`。

## D. 左栏资源（`components/Resources.tsx`）

当前左栏同时承担两件频率差一个量级的事——装机期的接入管理，和每次派发的挑模型。拆开：

- **左栏只列 `已接入` 的模型**，按 agent 分组。agent 组头 = 折叠箭头 + agent 名（等宽）+ 版本号（等宽、`--text-3`）。**组头不放状态胶囊，不放「探测 / 手动输入 / 冒烟 / 移除」按钮。**
- 模型行 = 模型 ID（等宽 13px）+ 下面一行来源（11px、`--text-3`）。**没有状态胶囊**——列出来的都是能派发的，状态是冗余信息。行本身无边框、4px 圆角、hover 才出背景。
- 某个 agent 没有任何已接入模型时，该 agent 整块不显示在左栏。
- **栏底一行汇总链接**：`N 个待冒烟 · M 个未接入 · 管理 →`（数量为 0 的那半句省略）。点击打开接入页。

## E. 接入页（`components/Resources.tsx` 的接入对话框）

从窄弹窗改成 **agent 卡片网格**：

- 对话框加宽到 `max-width: 980px`；网格 `repeat(auto-fit, minmax(280px, 1fr))`，间距 12px。
- 一张卡 = 一个 agent，8px 圆角、1px 边框、`--surface` 底、12px 内边距：
  - **卡头**：24px 方形标记（4px 圆角、`--surface-alt` 底、1px 边框、等宽两字母，取 agent id 前两位大写——`Claude Code` 与 `Codex CLI` 都取名称首字母会撞成 `CC`，所以按 id 取）+ agent 名（等宽 13px 粗）+ 检测状态胶囊。
  - 版本号一行（等宽 11px、`--text-3`）。
  - **模型清单**：上边框分隔，每行 = 模型 ID（等宽，占满剩余宽）+ 状态胶囊（已接入 / 待冒烟 / 冒烟失败 / 未接入）。有 group 的按 group 分小节，小节标题 11px 大写。
  - **一行操作提示**（探测中… / 冒烟中… / 结果），16px 最小高度，避免布局跳动。
  - **卡底操作区**：上边框分隔，放现有的探测 / 冒烟 / 手动输入 / 移除按钮。按钮沿用现有的 `btn btn-sm`。
- 对话框底部操作：`＋ 手动添加 agent` 与 `关闭`。
- 入口两个：顶栏齿轮图标，和左栏底部那行汇总链接。

**已有的后端调用与状态流转全部不变**，只换渲染形态。冒烟成功后模型转 `已接入` 并出现在左栏，这条行为要保住。

## F. 预设命名（只在已存在的地方改）

界面上的预设一律叫「X 模式」：共享模式 / 经济模式 / 盲审模式 / 擂台模式 / 评审团模式。
**注意区分**：`盲审` 同时还是流水线里的阶段名，阶段名与结果页的「盲审结果」不加「模式」后缀。
当前代码里如果还没有预设下拉（属于第 2 期），本节跳过，在 REPORT 里注明跳过。

## 验收

必须实际跑过并把命令与输出原文贴进 REPORT：

1. `cd src-ui && npm run build` 通过。
2. `npx tsc --noEmit -p tsconfig.app.json` 无错误。
3. `cargo check --manifest-path src-tauri/Cargo.toml` 仍通过（本期不该动 Rust，跑一遍确认没误伤）。
4. 自查：`grep -o 'font-size:[^;]*' src-ui/src/styles.css | sort -u`，输出只能是 11/12/13/14/16/18px。把命令输出贴进 REPORT。
5. 自查：`grep -c 'box-shadow' src-ui/src/styles.css`，逐处说明是不是浮层。

**跑不了的不要写「已验证」。** 窗口手测由编排者在沙箱外做，不要声称做过。

## 交付

写 `tasks/REPORT-1B.md`：改了什么、怎么验证的（命令与结果原文）、没做什么、不确定处。不 commit。
