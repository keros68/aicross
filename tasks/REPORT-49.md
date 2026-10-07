# REPORT-49 窗口重做：小窗、左侧导航、跟随系统配色

实现者：Claude 子代理（general-purpose）。报告由编排者按其回复落盘。在 TASK-47 未提交的改动之上继续；只改 `src-ui/**` 与 `tauri.conf.json` 主窗口四个尺寸字段。

## 改动

- 窗口：`tauri.conf.json` 主窗口 960×640，最小 800×560。
- 外壳：`App.tsx` 左侧导航（aicross + 模式 / 资源 / 派发记录）+ 右侧标题栏与内容区；查看页在右侧内容区内，导航保留并高亮「派发记录」；点导航关掉查看页并切页；常驻页、localStorage、默认进「模式」与冷 / 热启动打开查看页的逻辑不变。`TitleBar.tsx` 只占右侧顶部；新增 `lib/window-drag.ts`（标题栏与导航顶部共用拖动、双击最大化）。
- 配色：新增 `lib/theme.ts`（`themeFromMedia`、`followSystemTheme`），`main.tsx` 启动时调用；`index.html` 首帧前按 `prefers-color-scheme` 设 `data-theme`。`global.css` 两套变量补齐 `--bg-nav`、`--bg-selected`、`--on-accent`、`--close-bg`、`--close-fg` 与 `color-scheme`；浅色卡片底纯白，`--text-3`、`--red` 加深。
- 模式页：每个模式一张卡片（模式名、默认标记、设为默认、编辑 / 收起）；收起时每个位置一行（角色 + agent/model + 来源），多阶段标阶段号并以细线分隔；未指定与人选失效标异常色并写「人选失效：<状态>」；编辑展开阶段列与人选 / 思考 / 备选编辑框，同一时间只展开一张。
- 资源页：页头「上次探测：<时间>」/「尚未探测」+「重新探测」，超过 7 天提示；每个 agent 一张卡片；`未发现` 默认收起，点开灰色列出，不给冒烟。
- 派发记录：`Home` 改名与卡片样式，数据与轮询不变。查看页铺满右侧内容区，去掉全窗覆盖；同阶段卡片放不下时折行。
- TASK-48 接入：`ResourcesOutput.detected_at?`、`EffectiveSeat.state?`；`未发现` 列为异常状态并在人选下拉禁用；新增纯函数 `seatInvalidState`、`seatStatus`、`modeRows`、`splitMissingModels`、`missingModelsText`、`detectStatus`。
- 任务外：`ResizeEdges.tsx` 两处空 `catch {}` 补注释（原有代码，否则 `npx eslint src` 报 `no-empty`）。

## 测试

新增 13 项：`lib/__tests__/theme.test.ts`（配色跟随系统 2 项）；`config-view.test.ts` 的「模式卡片的行」「资源卡片」「人选下拉禁用未发现」。`page-tabs.test.ts` 两项按新命名改断言。原有 49 项保留。

## 验证

- vitest：7 个文件、62 项通过；`npm run build` 通过；`npx eslint src` 无输出。（编排者复跑一致。）
- 写死颜色只剩 `global.css` 变量定义与 `viewer/vendor.ts` 厂商色（小色点，视觉纪律允许）。
- 改动与新文件全部 LF，`tauri.conf.json`、`index.html` 换行不变。

## 未做与理由

- 真窗口里两种配色、最小尺寸与各页交互未看：留给编排者真机核。
- macOS 下导航顶部给红绿灯让位只按配置推算，未在 macOS 上看过。
- `未发现` 的模型展开后不逐条重复说明文字。

## 外观返工 r1（`tasks/fix-49-r1.md`，用户反馈「好丑，没有按钮，顺序不对」）

- 照 CC Switch 截图改：导航带线框图标、顺序资源 → 模式 → 派发记录、首次打开进资源；页头一行（页面图标 + 标题 + 说明 + 橙色主按钮 + 窗口按钮，整行可拖动）；列表行左侧标识块、右侧线框图标按钮；默认模式用蓝色生效样式；派发记录改表格；查看页页头改「返回」。
- 新增 `components/common/Icon.tsx`（12 个内联线框图标）、`PageHeader.tsx`；`--accent` 改橙色，新增 `--active-*` 蓝色生效态变量。
- 编排者复跑：vitest 66 项通过，build 通过，eslint 无输出。真窗口看了模式页，观感与参照同类；用户随后反馈「单调，模块之间得处理下」，进入 r2。

## r2 / r3（外观层次与文案）

- r2（用户「单调，模块之间得处理下」，并要求 agent 与模式都配图标）：页顶数字卡、模式分组与模式图标 / 颜色、流程条胶囊带 agent 图标、资源页厂商色与小统计、按 provider 分组、派发记录筛选、间距分级；恢复 TASK-40 前的 agent 图标资产（`src-ui/src/icons-inline/`）。
- r3（用户「太多这种口语化表述了」）：删掉全部说明句，页头只留图标、标题与动作；页头一行 56px 垂直居中；同阶段位置卡横排；图片图标加底板；派发记录模式名不截断。

## 代码盲审（GLM + Gemini，均「修改后可以合并」）与 r4

- 两家都列为高的 `config_set_default_mode` 参数名不成立（Rust 签名为 `mode`，前端一致；TASK-47 任务书措辞写成 `id`）。inline SVG 无 `id` 属性，不会串扰。
- r4 修正：残留说明句与劝导词；保存在途时卸载与外部操作；资源页与派发记录加载态；模式页快照请求序号；补 `followSystemTheme`、时区无关的探测时间、`stageGridColumns`、窗口拖动判定的测试。
- 编排者复跑：vitest 91 项通过，build 通过，eslint 无输出。

