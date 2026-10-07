# TASK-54（窗口侧）斜杠命令卡片按模式列出

**只改 `src-ui/**`。不 commit，不 push，不 npm install。** 契约见同目录 `TASK-54-slash-per-mode.md` 第 2、3 节（引擎并行实现）。界面只放名称、数据与动作。

## 要做

skill 页「斜杠命令」卡片改为：

1. 第一行「默认」+ `invoke` + 复制（不变）。
2. 下面按 `modes` 每个内置模式一行：模式图标与模式名｜带参数写法 `argForm`（等宽，复制按钮）｜短命令：有 `command` 时显示其 `trigger`（等宽，复制）与状态胶囊和编辑 / 删除按钮；没有时显示灰色 `suggestedName` 的触发写法预览与小按钮「生成」（调 `slash_add`，名称为 `suggestedName`、模式为该行模式）。
3. 卡片头右侧：「全部生成」（调 `slash_add_all(host, 'ax-')`；所有模式都已有命令时禁用），以及原有「添加」（用于自定义模式或自定义名称）。`skipped` 非空时卡片顶部提示条列出「<名称>：<原因>」。
4. `commands` 里不属于内置模式的命令（自定义模式或用户手动添加的额外名称）列在模式行下方「其它」小节，保持原有行为。
5. 未安装 skill 的提示保留。

`tauri.ts` 补类型与 `slashAddAll`。纯函数测试：模式行数据（有命令 / 无命令）、全部生成按钮的禁用判定、「其它」的划分。

## 验证

```
cd src-ui && npx vitest run --configLoader native --pool threads
cd src-ui && npm run build
cd src-ui && npx eslint src
```

回复里给修改说明与结果摘要。
