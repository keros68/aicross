# TASK-53（窗口侧）skill 页的斜杠命令区

**只改 `src-ui/**`。不 commit，不 push，不 npm install。** 契约见同目录 `TASK-53-slash-commands.md` 第 3 节（引擎并行实现）。界面只放名称、数据与动作，不写说明句。

## 要做

在 skill 页（按当前主 agent）位置列表下方加一张「斜杠命令」卡片：

- 第一行：「默认」+ 等宽显示 `invoke`（如 `/ai-cross`）+ 小按钮「复制」。
- 下面每个自定义命令一行：等宽 `trigger`（如 `/cross-review`）+ 模式图标与模式名 + 附加说明（有则灰色一行，省略）+ 状态胶囊（已写入 / 未写入 / 被占用，后两者异常色）+ 图标按钮「复制」「编辑」「删除」（删除先确认）。
- 卡片头右侧按钮「添加」：展开一行表单——名称输入框（前缀显示该 host 的触发写法预览，如 `/prompts:` + 名称）、模式下拉（带模式图标）、附加说明输入框（可空）、「保存」「取消」。名称不合规时输入框下显示「只能用小写字母、数字和 -」。保存调 `slash_add`，失败把返回说明显示在表单下。
- 编辑即同名再 `slash_add`；删除调 `slash_remove`。
- 当前主 agent 未安装 ai-cross skill 时，卡片顶部提示条「<主 agent> 未安装 ai-cross skill」（命令需要 skill 才能工作）。

`tauri.ts` 加类型与 `slashList`、`slashAdd`、`slashRemove`。纯函数测试：名称校验、触发写法预览、状态到文字与色调的映射。

## 验证

```
cd src-ui && npx vitest run --configLoader native --pool threads
cd src-ui && npm run build
cd src-ui && npx eslint src
```

回复里给修改说明与结果摘要。
