# fix-49-r4 代码审查修正

来源：GLM-5.3-Flash 与 Antigravity（Gemini 3.8 Flash）对 TASK-47 / 49 前端代码的盲审（外观已由用户确认，本轮只改代码问题）。

## 要改

1. **残留的说明句与劝导词（两家共同，中）**：
   - `ModesPage.tsx` 预设写定位置下的「该位置由预设写定，保存的人选不生效」：删掉这句；预设写定的位置（`source` 为「预设」）把「保存」禁用，来源标签已说明情况。
   - 下拉缺省项「请选择」改为「未选择」。
   - `RunViewer.tsx` 里仍有的「找不到这次派发」改为「未找到该派发」。再全局搜一遍「请」「找不到」「即可」「建议」。
2. **保存 / 清除在途时被卸载（两家共同，低）**：`SeatEditor` 加卸载标志（`useRef` + effect cleanup），`await` 返回后先判断再 `setSaving` / `setError`。保存在途时把位置卡点击与卡片收起禁用，避免失败信息随组件卸载丢失。
3. **加载态（两家，低）**：资源页在第一次快照返回前显示「读取中…」；派发记录在 `list_runs` 首次返回前不显示「暂无派发记录」，改显示「读取中…」，只有确实为空时才显示空态。
4. **快照新旧覆盖（GLM 独有，低）**：模式页的 `configSnapshot` 刷新、`setDefault`、`onSaved` 共用 `setSnapshot`；给快照请求加递增序号，过期的结果丢弃。
5. **测试缺口（两家，低）**：
   - `followSystemTheme`：mock `window.matchMedia`，断言初始赋值、change 事件切换、返回的注销函数移除监听。
   - 上次探测时间的测试不要跨 UTC 与本地时区硬编码时分，预期值用本地时间计算。
   - `RunViewer.tsx` 的 `stageGridColumns` 抽到 `viewer/run-list.ts` 并补断言。
   - `lib/window-drag.ts` 的可点元素选择器抽成常量或纯函数，补「按钮 / 下拉 / 链接不拖动、标题文字可拖动」的断言。

## 不改

- 两家都列为「高」的 `config_set_default_mode` 参数名：Rust 端 `src/server.rs` 的签名是 `config_set_default_mode(mode: String)`，与前端 `{ mode }` 一致；TASK-47 任务书第 3 节写成 `(id)` 是任务书措辞不一致。
- inline SVG 重复 id（GLM）：`pi.svg`、`grok.svg` 内没有 `id` 属性，不会串扰。
- 「仅在 aicross 窗口中可用」（GLM 指与 TASK-47 原文不同）：r3 按文案规则有意替换，保留。

## 验证

```
cd src-ui && npx vitest run --configLoader native --pool threads
cd src-ui && npm run build
cd src-ui && npx eslint src
```

不 commit，不 push，不 npm install。回复里给逐条修改说明与结果。
