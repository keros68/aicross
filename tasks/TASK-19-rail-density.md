# TASK-19 左栏资源行统一与界面字号（09-15 用户反馈）

用户看左栏截图：Claude 的模型行右侧有浅色等宽的真实 ID，Codex 的行没有，两组长得不一样；整体字太小，27 寸屏都看着小，小屏更不行。

## 已定

1. **模型行一律同一种形态**：一行一个模型名（14px），行高 30px，左对齐，右侧不再放真实 ID / 版本这类第二列；每行只保留一个状态点（原生无点）。Claude 别名对应的真实 ID、pi 的 provider、最近验证时间、来源路径统一进悬停提示与接入页详情——所有 agent 同一规则，不再按 agent 区别对待。agent 头行：图标 + 显示名（14px 加粗）+ 版本（12px 等宽，靠右），四家同款。pi 的 provider 组头维持「名字 + 条数」，未验证折叠行保持。
2. **字号整体上调一级**：左栏与接入页的 11 → 12、12 → 13、13 → 14（`docs/PRODUCT.md`「视觉纪律」的六级阶梯改为 12 / 13 / 14 / 15 / 16 / 18，并同步那一行说明）；行内小标签 `2px 8px` 不变。中栏对话图与右栏同批检查，凡是 11px 的辅助文字升到 12px。
3. **界面缩放设置**：设置页「外观」加「界面缩放」：90% / 100% / 110% / 125% / 150%，默认 100%；用 Tauri 2 webview 的 `set_zoom` 实现（拿不到就退回根元素 CSS `zoom`），存在设置里，启动时应用。快捷键 Ctrl + = / Ctrl + - / Ctrl + 0 走同一份设置。

## 实现

- `ResourcesTab.tsx` / `resources.css`：删掉 `resource-model-resolved` 这类第二列的渲染，把内容并进 `title` 悬停；行高与字号按上面；`ResourcesOverlay.tsx` 的模型行同样只留名 + 状态点，详情行里显示真实 ID / provider。
- `SettingsModal.tsx`「外观」加缩放；`src-ui/src/settings/ui-scale.ts` 纯函数（档位、夹紧、快捷键步进）；Rust 侧若需要命令（`set_ui_zoom`）加在 `src/server.rs`，只做 webview zoom。
- `docs/PRODUCT.md` 视觉纪律那张表改字号阶梯；`docs/MANUAL.md` 加「界面缩放」一句。
- 测试：行渲染统一（四家 agent 的模型行 HTML 结构一致、无第二列元素）；缩放档位纯函数（夹紧、步进、持久化往返）；CSS 断言（模型行 14px / 30px，agent 头 14px）。

验证：`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。保持文件原有换行；不 `cargo fmt`；不起窗口；不 commit、不 push。报告写 `tasks/REPORT-19.md`。等 TASK-17（同样改 ResourcesTab / ResourcesOverlay）并入后再开工。
