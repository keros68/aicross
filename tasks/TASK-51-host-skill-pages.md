# TASK-51 主 agent 选择、skill 页、一键推荐（窗口侧，10-06）

**只改 `src-ui/**`。不改 Rust。不 commit，不 push，不 npm install。** 在 TASK-49 的未提交改动之上继续（同一工作树）。

设计依据：`docs/PRODUCT.md`「主 agent」「skill 安装」「一键推荐」三节。窗口命令契约以 `tasks/TASK-50-host-skill-recommend.md` 第 5 节为准（Rust 侧并行实现）。外观延续 TASK-49 已被用户认可的样式；界面只放名称、数据与动作，不写说明句。

## 1. 左栏

- 应用名下方新增「主 agent」一组：列出 `hosts_list()` 返回的 agent，每项「agent 图标 + 名称」，第二行灰色小字是实际厂商（`未知` 用异常色）。选中项用当前生效样式。选中记在 localStorage；首次没有记录时选第一个。
- 其下是导航：**资源 → skill → 模式 → 派发记录**。「skill」「模式」两页按当前主 agent 显示；「资源」「派发记录」不分主 agent。
- 没有检测到任何可作主 agent 的 agent 时，主 agent 组显示「尚未探测资源」。

## 2. 主 agent 信息条

「skill」「模式」两页页头下方第一行：当前主 agent 图标 + 名称 + 「实际厂商：<厂商>」+ 依据小字（`vendorBasis`）+ 一个下拉可改厂商（选项：自动推定 / 各已知厂商），改动调 `host_set_vendor`。

## 3. skill 页

- 数字卡：内置版本、已安装位置数、需处理（重复安装 + 可更新）。
- 位置列表（来自 `skill_status`，只列当前主 agent 会读取的位置）：每行 范围（全局 / 项目）+ 路径（等宽）+ 状态胶囊（未安装 / 已安装 · 版本 / 由其他软件管理 · 版本 / 可更新）；`external` 有链接目标时在路径下小字显示「指向 <目标>」。
- 页头主按钮「安装到全局」（已是我们安装且最新时变为灰色「已安装」；可更新时变为「更新」）；次按钮「安装到项目」→ 调 `pick_folder()` 选目录后 `skill_install(host, 'project', path)`。
- 我们安装的位置行右侧图标按钮「卸载」（需确认）；`external` 行没有操作按钮。
- `duplicate` 为 true 时页顶提示条「检测到重复安装」，列出两处路径。
- 安装、更新、卸载执行中按钮禁用并显示进行中；失败把返回的说明显示在页顶。

## 4. 模式页

- 改为按当前主 agent 取数（`config_snapshot(host)`），写入命令都带 `host`。
- 页头主按钮「一键推荐」→ `config_recommend(host, null, false)`；旁边「更多」菜单里「全部按推荐重填」，点了先弹确认（应用内确认框，不用浏览器 `confirm`），确认后 `config_recommend(host, null, true)`。
- 推荐返回的 `notes` 显示在页顶提示条，可关闭。
- 流程条胶囊：`origin` 为 `recommended` 的在模型名后加一个小标记「推荐」；手选的不加。
- 当前主 agent 一个人选都没有时，页顶提示条「未配置人选」+ 按钮「一键推荐」。
- 编辑框保存的人选为手选（由 Rust 侧写 `origin: manual`）。

## 5. 首次使用顺序

按 资源 → 选主 agent → skill → 模式：资源未探测时 skill、模式页顶部提示「尚未探测资源」+「去探测」；当前主 agent 未安装 skill 时模式页顶部提示「<主 agent> 未安装 ai-cross skill」+「去安装」（跳 skill 页）。

## 6. 测试（vitest，纯函数）

主 agent 列表与选中记忆；厂商显示（未知着色）；skill 位置的状态文字与按钮状态（none / ours 最新 / ours 可更新 / external）；重复安装提示；推荐标记；首次使用提示的判定。

## 验证

```
cd src-ui && npx vitest run --configLoader native --pool threads
cd src-ui && npm run build
cd src-ui && npx eslint src
```

用无头浏览器 + 假数据浅色 / 深色各看一遍新增的左栏与 skill 页、模式页。写 `tasks/REPORT-51.md`（不能写文件就把全文放在回复里）。
