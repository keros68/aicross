# TASK-56（窗口侧）模式位置数量调节

**只改 `src-ui/**`。不 commit，不 push，不 npm install。** 契约见同目录 `TASK-56-mode-counts.md` 第 2 节（引擎并行实现：`config show` 的 `modes[].count`、窗口命令 `config_set_count(host, mode, count)`）。

## 要做

- 模式卡片头，名称与介绍右侧（「设为默认」左边）：`count` 不为 null 时显示数量调节「<角色> × <数值>」+ 「−」「+」两个小图标按钮；到达 `min` / `max` 时对应按钮禁用。点击调 `config_set_count`，用返回的模式条目刷新本卡片；进行中两个按钮禁用。
- 减少数量会去掉末尾位置的人选：若被去掉的位置有本模式人选（`seat` 不为 null），先弹应用内确认框「减少到 N 个<角色>，末尾 M 个人选将被清除」，确认后再调。
- 新增的位置人选为空（显示「未选择」），可直接在下拉里选，或点「一键推荐」。
- 介绍文字由引擎提供（已改为不含数量的正式说法），前端不写死。
- `tauri.ts` 补 `count` 类型与 `configSetCount`。纯函数测试：按钮禁用判定、减少时需确认的判定与确认文字。

## 验证

```
cd src-ui && npx vitest run --configLoader native --pool threads
cd src-ui && npm run build
cd src-ui && npx eslint src
```

回复里给修改说明与结果摘要。
