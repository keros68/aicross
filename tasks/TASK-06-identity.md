# TASK-06 界面身份第一刀：去掉 Coffee CLI 的痕迹，把壳子对齐到 aicross 的主线

先读 `AGENTS.md`、`docs/PRODUCT.md`（「形态」「视觉纪律」「不做」三节）、`tasks/REPORT-1B.md`（上一轮视觉对齐做过什么）、`tasks/REPORT-02.md`（第 2 期把中栏改成了对话图）。原型 `docs/proto/index.html` 只看视觉语汇。

本期只改前端，**不改 Rust、不加依赖、不引 CDN**。本仓库是 Coffee CLI 的 fork（`AGENTS.md`「与 coffeecli 的关系」），改名换标已在 F1 期做完；本期要做的是**形态**层面的去 coffee 味：coffee 的主线是「挑一个 CLI 开终端」，aicross 的主线是「接入资源 → 在对话图上派发 → 收结果裁决」，壳子要按后者组织。

## 背景：现在哪些地方一眼是 coffee

- 中栏默认页是 coffee 的 launchpad：工具卡网格（Claude Code / OpenCode / Codex CLI / Antigravity / 终端 / 独立四屏）+ 贡献热力图「618 次会话、232,180 条消息」+「识别不到？切换模型」。对话图只是网格里的一张卡。
- 右栏默认是任务板，空态文案「晚上好，想干点什么大事？」「查看使用攻略」。
- 标题栏有「Merged input (Alt + W)」（coffee 的 gambit 合并输入）与多 agent 布局切换。
- 设置里 16 套主题色、6 种形状、图标主题、背景图。
- 快捷键提示 `Alt + Q / W / E` 常驻标题栏。

## 要做的

### A. 中栏默认页

1. 新 tab 的默认内容 = **对话图起始视图**（`components/center/graph/GraphTab.tsx` 里已有：新建对话图 + 已有图列表），不再是 launchpad。tab 类型默认 `'graph'`。
2. launchpad 的工具卡网格、贡献热力图、「识别不到？切换模型」从默认页移除。开终端的能力保留，但降级为次级入口：起始视图底部一行「打开终端」，点开一个下拉列出本机检测到的 agent CLI 与「系统终端」，沿用现有 `selectTool` 与文件夹选择逻辑。四分屏 / 远程等 coffee 特有入口进同一个下拉的「更多」或直接不露出（功能代码不删）。
3. 起始视图文案改说明体，一句话说清楚这是什么：「会话是一张从左向右生长的图。发第一句得到根节点，@ 模型分叉，盖预设编排。」已有，可沿用。

### B. 右栏

1. 默认 tab = **方案与编排**，任务板是第二个 tab。
2. 任务板空态：去掉问候语与「查看使用攻略」，换成一行「还没有任务」+「新建」。
3. 修改记录、会话记录等 coffee 的 tab 保留在原位置，不改行为。

### C. 标题栏

1. 去掉「Merged input」按钮与多 agent 布局切换（gambit 是 coffee 的功能，本产品不做；`docs/PRODUCT.md`「不做」）。功能代码可留，界面不露。
2. 快捷键提示文字改成 hover 时的 title 提示，不常驻显示。
3. 保留：左右栏开关、设置、更新检查、窗口三键。

### D. 主题与视觉纪律

按 `docs/PRODUCT.md`「视觉纪律」表逐条对齐，重点：

1. **主题收成两套**：深、浅。默认深。其余 14 套 coffee 主题从设置里移除（CSS 可留，选项不露，`VALID_THEMES` 相应收窄并把旧值迁移到深/浅）。两套的 token 按纪律定：中性灰底、单一强调色（深色用暖灰橙 `#c4956a` 这类低饱和色即可，不要高饱和）、发丝边框、8px 容器圆角、4px 列表行圆角。
2. **形状 / 图标主题 / 背景图** 三个个性化项从设置里移除（代码可留）。
3. 字体分工：正文 Sans、结构性文字（模型 ID、路径、数字）Mono，数字开 `tabular-nums`。字体栈 `"IBM Plex Sans", system-ui, …` / `"IBM Plex Mono", ui-monospace, …`，本机没装就落到系统字体，不要引网络字体。
4. 状态胶囊只给异常状态（待冒烟、运行中、失败），常态用色点或纯文字。检查现有界面里滥用的胶囊。
5. 厂商色只做色点与 3px 边线，不做大面积填充。

### E. 文案复查

全仓 `src-ui/src` 搜一遍：问候语、感叹号、emoji、「大事」「攻略」「Hyper」「Gambit」「Coffee」「哨兵」等，一律改成中性说明体或删除。按钮 2–6 字动词短语；状态词与动作词成对。

## 不做

- 不动 `src-ui/src/graph/`、`components/center/graph/`、`components/right/orchestration/` 里的**逻辑**（第 3 期正在另一分支上给这些文件接派发引擎；只允许改它们的 CSS 文件里的颜色与字体 token，不改结构）。
- 不动 Rust、不动 `port/`。
- 不删 coffee 的终端、工作区、会话记录、修改记录功能，只调整入口与默认值。
- 不改 i18n 字典的键名；新增文案直接写中文（与第 2 期一致）。

## 验收

1. `cd src-ui && npx vitest run` 与 `cd src-ui && npm run build` 输出原文进 `tasks/REPORT-06.md`。
2. 启动窗口截图（能起就起：根目录 `npx --yes -p @tauri-apps/cli@2 tauri dev`，日志写到仓库外，否则 tauri 的监视器会因 tasks/ 写入反复重建）：默认页、右栏默认 tab、设置页主题选项、深浅两套各一张。起不了就如实写「未验证」。
3. 报告里列出改动文件清单，并单独一节「与第 3 期可能冲突的文件」（凡是改了 `app-state.tsx`、`CenterPanel.tsx`、`tauri.ts` 的，写明改了哪几行、为什么）。
