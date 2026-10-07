# TASK-62 更多 agent：资源页按支持清单显示（界面）

**只改 `src-ui/**`，新建 `tasks/REPORT-62.md`。不改 Rust。不 commit，不 push，不加依赖。** 设计依据：`docs/PRODUCT.md`「更多 agent（10-07 用户定）」。引擎侧同时在做 TASK-61，契约如下（以此为准，引擎没合进来前用假数据测）：

- 资源输出（`ResourcesOutput.agents[]`，类型 `ResourceAgentEntry`）每项新增 `untested: boolean`、`homepage: string`。
- agent 不再固定四家：新增 `agy`（Antigravity）、`cursor`（Cursor）、`codebuddy`（WorkBuddy / CodeBuddy）、`copilot`（GitHub Copilot）、`qoder`（Qoder）。没装的也在列表里，`state` 为「未安装」、无模型。

## 要做

1. **类型**：`tauri.ts` 的 `ResourceAgentEntry` 加两个可选字段（缺省按 false / 空串处理，兼容旧引擎）。
2. **资源页布局**：
   - 装了的 agent 照常列卡片（顺序按引擎给的）。
   - 没装的折叠成列表末尾一行「未安装 N 个」，点开每行只有图标、名称与「官网」链接（`homepage`，用系统浏览器打开；仓库里若已有打开外链的方式就复用，没有就只显示网址文字可复制，不新增依赖或 Rust 命令）。
   - 统计卡里的「agent」数改为已安装的数量。
   - 原有「未探测」空态逻辑不变。
3. **未实测标记**：`untested` 为 true 的 agent，卡片标题旁一个小标签「未实测」（沿用现有标签样式，不写说明句）。
4. **图标**：`lib/agent-icon-key.ts` 与 `AgentIcon` 补 `agy`（已有 `icons-inline/antigravity.svg`）。cursor、codebuddy、copilot、qoder 没有现成图标的，用字母块占位（与现有无图标时的退路一致；没有退路就做一个：圆角方块 + 首字母），不从网上下载商标图。
5. **模式页下拉**：沿用现有过滤（只列可用、已验证的）；`untested` agent 的模型照列（引擎会给出状态），在下拉里该 agent 分组名后加「未实测」。
6. **主 agent 下拉**：不变（只有引擎 `hosts_list` 给的那几家）。

## 测试

- 纯函数放 `config/*.ts`（如分出已装 / 未装、统计数），配 vitest。
- `npx tsc --noEmit -p .`、`npx eslint src`、`npx vitest run` 全过。

## 规矩

- 文案遵守 `C:\Users\keros68\.codex\guidance\copywriting.md`：只放名称、数据、动作，不写说明句。
- 换行保持 LF。`REPORT-62.md` 写改了什么、测试结果、没做的与理由。
