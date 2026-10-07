# REPORT-62 更多 agent：资源页按支持清单显示（界面）

## 未完成与未验证

- **官网链接不能点开**：仓库前端与 Rust 侧都没有打开外链的方式（无 opener / shell 插件，无对应命令）。按任务约定，未安装行显示网址文字（可选中）加复制按钮，不新增依赖或 Rust 命令。
- **没在窗口里实测**：TASK-61 的引擎未合入，界面只用单元测试里的假数据验证了纯函数；资源页折叠行、「未实测」标签、下拉分组标记的实际外观没有在 Tauri 窗口里看过。
- cursor、codebuddy、copilot、qoder 无图标资产，走现有的厂商色首字母退路（`AgentIcon` 已有），没有另做字母块。

## 改动（只在 src-ui/）

- `tauri.ts`：`ResourceAgentEntry` 加可选 `untested?: boolean`、`homepage?: string`。
- `config/config-view.ts`：新增 `isInstalled`、`splitInstalled`、`notInstalledText`、`isUntested`、`agentHomepage`；`resourcesOverview` 的 agent 数与模型统计只算已安装的；`SeatOption` 加可选 `untested`，`seatOptions` 按 agent 带上。
- `config/picker.ts`：`PickerGroup` 加 `untested`，分组时取自选项。
- `config/ModelPicker.tsx`：未实测 agent 的分组名后显示「未实测」小标签（沿用 `pill muted`）。下拉过滤规则未改。
- `config/ResourcesPage.tsx`：主列表只列已安装 agent（引擎顺序）；未安装的折叠成末尾一张卡「未安装 N 个」，展开后每行图标、名称、官网地址与复制按钮；未实测 agent 卡片标题旁「未实测」标签（含「不可用」筛选视图）；「暂无资源记录」改按全部 agent 判断；「未探测」空态逻辑未动；筛选「不可用」时不显示未安装卡。
- `config/SlashCard.tsx`：导出现有 `CopyButton` 供资源页复用。
- `config/ConfigPages.css`：下拉分组标题改 flex 以容纳标签；官网地址单行省略、可选中。
- `lib/agent-icon-key.ts`：`agy` 对应 antigravity 图标。
- 主 agent 下拉未改。

## 测试

在 `src-ui/`：

- `npx tsc --noEmit -p .`：通过，无输出。
- `npx eslint src`：通过，无输出。
- `npx vitest run`：19 个文件、201 个测试全过。

新增 / 调整的测试：

- `config/__tests__/config-view.test.ts`：已装 / 未装分组与顺序、折叠行文字、缺字段兼容、统计卡只算已安装、下拉选项带未实测标记；原概览用例的 agent 数由 3 改为 2（夹具里 kimi 为「未安装」）。
- `config/__tests__/picker.test.ts`：分组带未实测标记。
- `lib/__tests__/agent-icon-key.test.ts`：`agy` 取 antigravity 图标，四家新 agent 无图标。

## 修正轮 r1（Codex 只读审查，结论「修后合并」）

1. 统计卡 agent 数把「未探测」算成已安装：新增 `isConfirmedInstalled`（排除未安装与未探测），统计只计它；未探测的 agent 仍留在主列表。测试「从未探测的 agent 留在主列表，但不计入 agent 数」。
2. 「去测试」的目标已变为未安装时无法定位：未安装行加 `agent-card-<id>`；目标不在主列表时每次跳转展开一次折叠行再滚到该行（按跳转序号只处理一次，用户可再收起）。

验证：tsc、eslint 无输出；vitest 202 通过。
