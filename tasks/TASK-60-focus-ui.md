# TASK-60 评审视角选择、短命令更新、外部来源（界面）

引擎侧为 TASK-59（t46 并行实现），先按下面契约写，集成树联调。

## 引擎契约（TASK-59）

- `config show` / `config_snapshot`：评审模式节点有 `focusOptions: string[]`；位置的 `focus` 是生效值。
- 窗口命令 `config_set_focus(host, mode, index, focus)`（index 从 1 起；`focus` 为空串恢复默认），返回该模式节点（同 `config show` 结构）；校验失败返回错误文字。
- `slash list` 已登记命令增加 `stale: boolean`。
- `skill status` 外部位置增加 `manager: "skills.sh" | "skills-manager" | ""`。

## 要做

1. **评审视角**：模式页评审模式每个位置的视角（TASK-58 已显示在角色旁）改为可点的下拉：列出 `focusOptions`，末项「自定义…」进入行内输入框（回车保存、Esc 取消，1–20 字）；另有「默认」恢复。保存调用 `config_set_focus`，失败在该行显示引擎返回的错误。按主 agent 分开，切换主 agent 后重读。
2. **短命令更新**：`stale` 为 true 的命令显示标签「待更新」与按钮「更新」（调用 `slash_add`，名称、模式、说明沿用原值）；有任一条 stale 时「全部生成」旁加「全部更新」。
3. **外部来源**：skill 页外部位置的状态写「由 skills.sh 管理」「由 skills-manager 管理」，`manager` 为空时仍是「由其他软件管理」。页头状态标签同理。

## 规则

文案只放名称、数据、动作；只改 `src-ui/**` 与 `tasks/REPORT-60.md`；不 commit、不 push、不加依赖；纯逻辑放 `*.ts` 并补 vitest。

```
cd src-ui && npx vitest run && npm run build && npx eslint src
```
