# fix-47-r1 修正清单

来源：GLM-5.3-Flash 与 Antigravity（Gemini 3.8 Flash）两份盲审。编排者已逐条核对代码。

## 要改

1. **切换页签丢失进行中的探测 / 冒烟（两家共同，中）**：`App.tsx` 用条件渲染切页签，资源页卸载后 `busy` 丢失，切回来按钮又能点，违背「同一时间只跑一个」；卸载后还会 setState。改法：资源页与模式页挂载后不卸载，切页签只用 CSS 隐藏（如 `hidden` 属性或 `display: none`）；「最近的派发」可保持现状。改后补一个纯函数或说明，保证切回时 `busy` 仍在。
2. **保存后旧编辑框 setState（Gemini 独有，低）**：`SeatEditor` 的 key 含 `JSON.stringify(selectedNode.seat)`，保存成功后旧实例被卸载，随后的 `setSaving(false)` 落在已卸载组件上。改法：用挂载标志在卸载后跳过 setState，或把 `setSaving(false)` 放在 `onSaved` 之前；保留「保存后编辑框按新值重置」的行为。
3. **重新探测后残留旧的行级错误（GLM 独有，低）**：`detect` 成功后 `setRowErrors({})`。
4. **次按钮 hover 用了强调色（GLM 独有，低）**：`.cfg-btn:hover:not(:disabled)` 改为只用背景差，强调色只留给主按钮与选中态。
5. **下拉整体用等宽字体（GLM 独有，低）**：`.cfg-field select` 改用 Sans；中文选项与状态词按视觉纪律用 Sans。
6. **死类名 `cfg-mode-name`（GLM 独有，低）**：删掉或补样式。

## 不改

- Gemini 第 1 条「原生模型在状态异常时也会出现冒烟按钮」：引擎 `src/resources.rs` 的 `model_state` 对原生 agent 的模型一律返回 `可用`（未安装、额度用尽除外），不会出现 `未验证` / `验证失败` / `冒烟超时`，按钮条件不会对原生模型成立。
- GLM 第 2 条「缺 REPORT-47 与验证输出」：报告已在 `tasks/REPORT-47.md`，只是没放进审查材料；编排者已独立复跑 vitest（48 项通过）与 build（通过）。

## 验证

```
cd src-ui && npx vitest run --configLoader native --pool threads
cd src-ui && npm run build
```

改完在回复里给出修改说明，编排者追加到 `tasks/REPORT-47.md`。不 commit，不 push。
