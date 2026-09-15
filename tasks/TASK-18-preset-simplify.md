# TASK-18 模式做减法（09-15 用户提出）

依据 `docs/PRODUCT.md`「用户 09-15 晚四点反馈的定案」第 4 条：模式太多让人蒙。基础三种 + 用户自定义；其余收进折叠组。

## 已定

- 右栏「方案与编排」预设下拉分三组：**基础模式**（共享模式 / 经济模式 / 盲审模式）、**自定义模式**（用户保存的，多个时「自定义模式 · 名称」）、**更多模式**（擂台模式 / 评审模式 / 蜂群模式，默认折叠，展开后与基础模式同样可选）；「红蓝模式（规划中）」「群聊模式（规划中）」放在更多模式末尾置灰。默认选中经济模式不变。
- 图上「盖预设」菜单同样分组（基础 / 自定义 / 更多）。
- 讲解页按同样分层：先讲三种基础模式一句话各是什么，再「更多模式」一段。
- 内置预设的 id、name、机器语义都不改；只是分组与折叠。

## 实现

- `OrchestrationPanel.tsx` 下拉用 `<optgroup>` 或分组列表 + 折叠项；`GraphTab.tsx` 盖预设菜单分组；`ExplainOverlay.tsx` 分层；`label.ts` 加 `presetGroupOf(preset)` 纯函数（builtin-shared/economy/blind → 基础；其余内置 → 更多；非内置 → 自定义）。
- `docs/MANUAL.md`「内置预设」一节改成同样的三层结构。
- 测试：`presetGroupOf` 三类；下拉渲染组顺序与折叠默认态；盖预设菜单分组；讲解页分层文案。

验证：`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`；Rust 不动（如需改注释跑 `cargo check`）。保持文件原有换行；不起窗口；不 commit、不 push。报告写 `tasks/REPORT-18.md`。
