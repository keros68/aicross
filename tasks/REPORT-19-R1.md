# REPORT-19-R1 左栏行统一与界面缩放复审修正

对应 `tasks/fix-19-r1.md`。

## 逐条修正

1. **接入页额度与状态详情**：`AgentCardModelMeta` 增加「状态：已验证 / 未验证 / 验证失败 / 原生」和未来额度的「额度用尽至 HH:MM」；模型主行仍只有名称与状态点；删除无渲染点的 `.agent-card-model-quota`。测试：`接入页详情显示验证状态与未来额度恢复时间`。
2. **未验证折叠按钮样式**：`.agent-card-unverified-toggle` 与左栏 `.resource-unverified-toggle` 共用普通、悬停和 `:focus-visible` 规则。测试：`左栏与接入页未验证折叠按钮共用样式和键盘焦点样式`。
3. **缩放回退只保留一处倍率**：`applyUiScale` 按根元素记住当前模式；从 WebView 模式进入 CSS 回退前先把 WebView 倍率归 1，再设置根元素倍率；重新成功时清除 CSS `zoom`。若归 1 本身失败，则不叠加 CSS 倍率，保留原 WebView 倍率。测试：`成功、回退、再成功时始终只有一处倍率生效`。
4. **缩放与页面布局真机核对**：回退路径已有成功→失败→成功的行为测试；按任务约束未启动窗口。接入页、左栏和终端在 90% / 100% / 150% 下的裁切与布局仍为**待窗口核对**。
5. **关闭对话图时保留模型提示**：禁用原因与 `modelHint` 用换行拼接，真实 ID、provider、验证时间等不再被「先打开对话图」替换。测试：`关闭对话图时悬停提示仍保留模型详情`。
6. **11px 残留**：设置页六处 11px 升为 12px；同时清理 `CenterPanel.css`、`ContributionHeatmap.css`、`Gambit.css` 与 `global.css` 的非终端 `font-size: 11px`。终端菜单和 `pre` 依任务要求未动。测试：`非终端与非代码块界面不再保留 11px 辅助文字`。
7. **快捷键与术语**：设置页和手册统一为 `Ctrl + = / - / 0`；手册术语表新增「界面缩放」。测试：`快捷键文案、术语表、定案编号与 Explorer 字号遗留记录一致`。
8. **缩放行为测试与快捷键基准**：`updateUiScale` / `initializeUiScale` 支持注入 storage、apply 与事件目标，测试持久化键、事件 detail 和 apply 调用；快捷键控制器以内存档位连续步进并订阅同窗口缩放事件，不再每次从存储取基准；更新 TASK-09b 头注释并移除 TASK-19 测试对旧第二列类名的绑定。测试：`updateUiScale 持久化、应用并发送带 detail 的事件`、`initializeUiScale 从注入存储读取并调用应用函数`、`快捷键用内存当前档递进，存储不可用不影响连续步进`。
9. **原生缩放热键**：`tauri.conf.json` 的主 WebView 显式设置 `zoomHotkeysEnabled: false`，由当前 Tauri 配置路径关闭 WebView2 原生缩放热键，避免和应用五档缩放双轨触发；未修改 Rust。测试：`首帧前初始化缩放，WebView 原生缩放热键保持关闭`。快捷键真机行为仍随第 4 条待窗口核对。
10. **冷启动首帧**：初始化从普通 effect 改到 `useLayoutEffect`，在浏览器绘制前读取并应用存档倍率。测试：`首帧前初始化缩放，WebView 原生缩放热键保持关闭`。
11. **文档与已知限制**：`docs/PRODUCT.md` 的定案恢复 1–5 递增；`Explorer.css` 的既有 `12.5px` 已记入 `tasks/fix-04-r5-backlog.md` 第 60 条。**已知限制：多窗口缩放不联动**，已打开的 detached 窗口不会收到主窗口事件，需要重启该窗口后读取新档位。测试：`快捷键文案、术语表、定案编号与 Explorer 字号遗留记录一致`、`修正报告保留窗口核对项与多窗口缩放限制`。

## 能红记录

实现前专项运行结果：

```text
Test Files  2 failed (2)
Tests  10 failed | 7 passed (17)
```

失败项覆盖额度/状态详情、折叠按钮样式、回退叠加、关图提示、注入行为、内存步进、首帧初始化、原生热键、11px 残留及文档/遗留记录。报告文件写入前单跑报告断言：

```text
Test Files  1 failed (1)
Tests  1 failed | 11 skipped (12)
Error: ENOENT: no such file or directory, open 'D:\work\aicross-ui\tasks\REPORT-19-R1.md'
```

## 验证结果

### `cargo check --all-targets`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-ui)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.04s
```

退出码 0，零 warning。

### `cargo test`

```text
running 450 tests
test result: ok. 439 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 26.69s
```

退出码 0；忽略项均为仓库已有的真机或显式本地事实源测试。

### `cd src-ui && npx vitest run --configLoader native --pool threads`

```text
Test Files  36 passed (36)
Tests  535 passed (535)
Duration  1.09s
```

退出码 0。首次完整运行发现既有 4px 视觉纪律测试只识别单选择器规则；合并折叠按钮规则后将该测试改为读取选择器所在的完整规则块，复跑全绿。`NO_COLOR` / `FORCE_COLOR` 提示为既有 Node 环境提示。

### `cd src-ui && npm run build`

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build

vite v8.0.7 building client environment for production...
✓ 339 modules transformed.
✓ built in 615ms
```

退出码 0。

### `python scripts/copy-audit.py`

```text
=== src\components\center\graph\DispatchOverlay.tsx ===
  [英文式破折号] «—»
=== src\components\common\SettingsModal.tsx ===
  [英文式破折号] «—»
=== src\graph\stamp.ts ===
  [预设命名-缺模式] «盲审»
=== src\resources\types.ts ===
  [动作词-冒烟] «冒烟»

共 4 处命中，4 个文件（含误报，需人工复核）
```

退出码 0。四处均与 TASK-19 首版报告相同；本轮只改了 `SettingsModal.tsx` 的缩放快捷键文案，命中的前提日期空值占位未动，其余三个命中文件未改。

## 换行与范围

- 修改前 `src/server.rs` 为 CRLF 4671 / 裸 LF 0，`src-ui/src/tauri.ts` 为 CRLF 602 / 裸 LF 164；本轮未修改两者。
- `docs/PRODUCT.md` 修改前后均为 CRLF 285 / 裸 LF 0；其余改动文件保持纯 LF；新增报告使用 LF。
- 未运行 `cargo fmt`，未启动窗口，未发真机请求，未读取凭据文件。
- 未 commit、未 push，未修改 `D:\work\aicross` 主工作树。

## 没做与限制

- 第 4 条的 90% / 100% / 150% 接入页、左栏与终端布局，以及 Ctrl + = / - / 0 的真机行为，按任务分工留给编排者窗口核对。
- 多窗口缩放不联动；detached 窗口需要重启后读取新档位。
- `Explorer.css` 的既有 `12.5px` 未在本轮扩改，已进入 backlog 60。
