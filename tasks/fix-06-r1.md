# TASK-06 修正轮 R1

先读 `AGENTS.md`、`tasks/TASK-06-identity.md`、`tasks/REPORT-06.md`，再读 `tasks/review-06-glm.md`（另一厂商盲审）。逐条处理，每条在 `tasks/REPORT-06.md` 末尾新增「修正轮 R1」一节里写「已修：怎么改的」或「不采纳：原因」。验证只跑 `cd src-ui && npx vitest run` 与 `cd src-ui && npm run build`，命令与输出原文贴进报告；沙箱里 vitest 起不来就用 `npx vitest run --configLoader native --pool threads` 并注明。不起窗口。不要 commit，不要 push。范围约束不变：不动 `src-ui/src/graph/`、`components/center/graph/`、`components/right/orchestration/` 的逻辑，不改 Rust、不加依赖。

## 必修

1. **背景图旧值仍渲染**（审查 1-D）：`getInitialState` 忽略并 `removeItem('cc-bg-path')`、`'cc-bg-type'`，`bgPath` 置空；`CenterPanel` 的背景层不再读它。REPORT 里「背景图不再露出」改成与实际一致。
2. **多语言死键**（审查 1-B/E）：删除 `task.greeting.*`、`task.show_guide`、`task.welcome_note` 在全部语言文件里的键与 `task-types.ts` 的 `makeWelcomeNote`；删废弃键不属于「不改键名」的约束。全仓再搜一遍 `Feeling ambitious`、`Großes`、`масштабное`、`usage guide`、「大事」「攻略」，一个不留。
3. **`tool: null` 瞬态**（审查 1-A，CenterPanel.tsx:898 附近）：`applyLaunchRequest` 兜底分支直接建 `{ tool, folderPath }` 的 session，不再先建 `tool: null` 再改。
4. **收敛重复表达式**（审查 5，为合并第 3 期做准备）：「空 graph 起始视图」的判定收成一个 helper（三处同义表达式改调它）；默认 session 字面量 `{ tool: 'graph', toolData: '' }` 收成一个工厂函数（app-state 与 CenterPanel 共用）。
5. **终端下拉不收起**（编排者窗口实测）：起始视图「打开终端」下拉按 Escape 或点击外部不关闭，设置弹窗打开时它仍在底下开着。改成受控开合：Escape、点击外部、选中一项后都关闭；沿用仓库里 `RecentFolderMenu` 的 outside-click 写法即可。
6. **胶囊审计**（审查 1-D4）：把本期可见面里的状态胶囊查一遍，常态与完成态改色点或纯文字，只留异常状态用胶囊；查过没问题的也在报告里写结论，不能空着。

## 次要（一并做）

7. `--shape-radius-tab` 10px → 8px（审查 4）。
8. 注释失真：CenterPanel.tsx 597「Coffee 101」、1635「coffeecli.com」、app-state.tsx 205「in ActiveGambit」（审查 1-E / 1-C）。
9. index.html 与 init 每次启动无条件写 `cc-shape` / `cc-icon-theme`：只在值不同时写。
10. 补两个测试（审查 6）：`getInitialState` 的主题迁移（`sakura` → `dark` 且写回；`light` 保留；缺省 → dark/soft/outline）；`TaskEmptyState` 渲染「还没有任务」「新建」并把三处 onNew 接对。测试文件放 `src-ui/src/__tests__/` 或组件旁的 `__tests__/`，用 vitest 现有配置。

## 不采纳

- TitleBar aria-label 硬编码中文：与「新增文案直接写中文」的约定一致，记录即可。

## 编排者已补的验收项

窗口截图项已由编排者在窗口里核过（默认页、终端下拉、右栏默认、设置页只剩深浅、明亮主题），报告里「未验证」改为「编排者窗口复核通过（09-13）」。
