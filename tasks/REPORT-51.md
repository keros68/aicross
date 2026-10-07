# REPORT-51 主 agent 选择、skill 页、一键推荐（窗口侧）

实现者：Claude 子代理（general-purpose）。报告由编排者按其回复整理落盘。在 TASK-49 未提交改动之上继续，只改 `src-ui/**`；Rust 侧（TASK-50）并行实现，未联调。

## 改动

- `tauri.ts`：新增类型 `HostEntry`、`SkillLocation`、`SkillHostStatus`、`SkillStatus`、`SkillScope`、`RecommendResult`，`Seat.origin?`；新增命令 `hostsList`、`hostSetVendor`、`configRecommend`、`skillStatus`、`skillInstall`、`skillUninstall`、`pickFolder`；`configSnapshot`、`configSetDefaultMode`、`configSetSeat`、`configClearSeat` 加 `host`。
- 左栏（`App.tsx`、新增 `lib/host-select.ts`）：「主 agent」一组（图标 + 名称 + 厂商，未知标红，选中用生效样式，localStorage 记忆，失效回第一个）；打开窗口、探测完成、改厂商后重读列表；导航 资源 → skill → 模式 → 派发记录；换主 agent 时 skill 与模式页整页重建。
- 新增 `components/common/HostBar.tsx`：主 agent 信息条与改厂商下拉（「自动推定」传 `auto`）。
- 新增 `config/SkillPage.tsx`、`config/skill-view.ts`：概览卡、位置行（范围、路径、状态、「指向 <目标>」）、「安装到全局 / 已安装 / 更新」、「安装到项目」（`pick_folder`）、卸载（应用内确认）、重复安装提示、执行中状态与失败说明。
- `config/ModesPage.tsx`：按主 agent 取数与写入；「一键推荐」与「更多 → 全部按推荐重填」（确认框）；`notes` 提示条；「推荐」小标记；顶部提示条按 未探测 → 未装 skill → 未配置人选 顺序只出一条。
- `config/ResourcesPage.tsx`：`config_snapshot` 带当前主 agent（无则 `null`）；探测完成通知外壳。
- 新增 `components/common/ConfirmDialog.tsx`；`Icon.tsx` 加 7 个图标；新增变量 `--shadow`、`--backdrop`（仅浮层）；按钮键盘焦点描边。

## 测试

新增 22 项：`lib/__tests__/host-select.test.ts`（选中记忆、厂商显示与下拉）、`config/__tests__/skill-view.test.ts`（状态文字、写入位置、按钮四态、重复安装、概览、位置过滤、首次使用提示、推荐说明）、`config-view.test.ts`（推荐标记）、`page-tabs.test.ts`（导航顺序与常驻）。

## 验证

vitest 12 个文件 113 项通过；build 通过；eslint 无输出（编排者复跑一致）。写死颜色只剩变量定义与厂商色；改动与新文件 LF。无头浏览器假数据浅色 / 深色截图自查了左栏、skill 页、模式页与确认框。

## 未做与理由

- 未真机联调（TASK-50 并行）。
- 「安装到项目」选过的目录不跨窗口保存。
- 「更多」菜单不支持点外部与 Esc 关闭。

## 待与 TASK-50 对齐的契约

1. `config_snapshot` 在还没有主 agent 时以 `host: null` 调用，Rust 侧需接受空值（返回资源与空配置）。
2. 厂商取值用资源库 `vendor` 的英文写法（Anthropic / OpenAI / Google / Moonshot / Zhipu / DeepSeek / Alibaba / MiniMax / StepFun），「自动推定」为 `auto`。
3. `skill_status` 的位置项最好直接标出「我们安装时写入的位置」，前端目前按路径判断。
4. `notes` 按 `string[]` 处理。
