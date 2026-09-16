# REPORT-19 左栏资源行统一、字号上调与界面缩放

对应 `tasks/TASK-19-rail-density.md`。

## 逐条实现

### 1. 左栏资源行统一

- 四家 agent 的模型行统一为两个固定槽位：模型名与状态；原生模型保留空状态槽，不显示状态点，其余模型只显示一个验证状态点。
- 删除 Claude 真实 ID 第二列及 pi 模型行内登录文字。真实 ID、provider、验证状态、额度状态、登录状态、最近登录检查、来源与验证时间统一进入悬停提示。
- agent 头维持图标、显示名和靠右版本的统一结构；名称为 14px 加粗，版本为 12px 等宽。
- TASK-17 的 pi provider 分组、已登录 / 未登录 / 未检查、未验证折叠与重新检查行为均保留。
- 测试：`四家模型行只有名称槽与状态槽，HTML 结构一致且没有第二列`、`claude 别名行只显示别名，真实 ID 进入悬停提示`。

### 2. 接入页模型行与详情

- 接入页模型主行同样只显示模型名与一个状态点，不再显示真实 ID 第二列、验证状态文字、登录文字或额度文字。
- 主行悬停沿用完整模型提示；展开「详情」后显示模型 ID、provider、登录状态、最近登录检查、来源、最近验证、失败原因与备注。
- 未登录 / 未检查 provider 折叠、provider 组头和卡片内滚动保持 TASK-17 行为。
- 测试：`接入页模型主行只显示名称和状态点，真实 ID 与登录检查进入详情`、`Claude 卡片主行只显示别名，真实 ID 进入悬停与详情`。

### 3. 字号与密度

- 左栏模型行改为 14px、30px；agent 名改为 14px，版本为 12px。
- 左栏与接入页的辅助 / 次要 / 正文字号按任务书上调；行内小标签的 `2px 8px` 未改。
- 中栏对话图及右栏仍为 11px 的辅助文字统一升为 12px；未改终端字号和非本期中心页面。
- `docs/PRODUCT.md` 的字号阶梯同步为 12 / 13 / 14 / 15 / 16 / 18。
- 测试：`模型行是 14px / 30px，agent 名是 14px，版本是 12px`，原有资源页 CSS 视觉纪律测试同步复核。

### 4. 界面缩放

- 新增 `src-ui/src/settings/ui-scale.ts`，集中定义 90% / 100% / 110% / 125% / 150% 五档、最近档夹紧、步进、序列化 / 解析、持久化和应用逻辑；缺少或损坏的设置回到 100%。
- 设置页「外观」新增「界面缩放」档位按钮；值存在 `localStorage` 的 `aicross-ui-scale`，启动时读取并应用。
- Ctrl + =、Ctrl + -、Ctrl + 0 与设置页共用 `updateUiScale`；按住快捷键不重复步进。
- 优先调用 Tauri 2 `getCurrentWebview().setZoom`，并只追加 `core:webview:allow-set-webview-zoom` 权限；调用失败时退回根元素 CSS `zoom`。未新增 Rust 命令，未修改契约字段。
- 测试：`只使用五个档位并把越界或非档位值夹到最近档`、`步进在边界停住，序列化与解析可以往返`、`Ctrl 加减与归零走同一档位函数`、`优先调用 WebView setZoom，失败时回退根元素 CSS zoom`、`设置页、启动应用与 Tauri 权限接到同一缩放模块`。

### 5. 文档

- `docs/MANUAL.md` 改为真实模型 ID 在悬停与接入页详情显示，并补充五档缩放与三个快捷键。
- `docs/PRODUCT.md` 只改定案要求的字号阶梯，保持 CRLF。

## 能红记录

实现前运行 TASK-19 两个新增测试文件，结果原文：

```text
FAIL  src/settings/__tests__/ui-scale-19.test.ts
Error: Cannot find module '../ui-scale'
FAIL  src/resources/__tests__/rail-density-19.test.tsx (3 tests | 3 failed)
Test Files  2 failed (2)
Tests  3 failed (3)
```

缩放实现后先补权限接线测试，确认漏加 Tauri 权限时单独变红：

```text
FAIL  src/settings/__tests__/ui-scale-19.test.ts (5 tests | 1 failed)
AssertionError: expected capability to contain 'core:webview:allow-set-webview-zoom'
Test Files  1 failed (1)
Tests  1 failed | 4 passed (5)
```

加入权限后专项与既有 TASK-09 / TASK-17 资源测试共 70 条通过；旧的第二列断言改为检查悬停与详情，登录分组及折叠断言未删除。

## 验证结果

### `cargo check --all-targets`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-ui)
    Checking windows v0.58.0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 38.39s
```

退出码 0，无 warning。

### `cargo test`

```text
running 450 tests
test result: ok. 439 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 21.80s
```

退出码 0；忽略项均为仓库已有的真机或显式本地事实源测试。

### `cd src-ui && npx vitest run --configLoader native --pool threads`

```text
Test Files  36 passed (36)
Tests  525 passed (525)
Duration  1.14s
```

退出码 0。输出中的 `NO_COLOR` / `FORCE_COLOR` 提示为既有 Node 环境提示，不影响测试结果。

### `cd src-ui && npm run build`

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build

vite v8.0.7 building client environment for production...
✓ 339 modules transformed.
✓ built in 2.06s
```

退出码 0。

### `python scripts/copy-audit.py`

```text
共 4 处命中，4 个文件（含误报，需人工复核）
```

退出码 0。四处均可在 HEAD 中复现，不是本期新增：两处英文式破折号命中代码 / 空值占位，`盲审：` 命中既有说明句，`冒烟失败` 是旧资源状态兼容常量；本期未改旧契约值。

## 换行与范围

- 修改前 `src/server.rs` 为 CRLF 4671 / 裸 LF 0，`src-ui/src/tauri.ts` 为 CRLF 602 / 裸 LF 164；本期未修改两者。
- `docs/PRODUCT.md` 修改前后均为 CRLF 285 / 裸 LF 0。
- 其余改动文件保持纯 LF；新增文件使用 LF。
- 未修改任何 Rust 源文件，未运行 `cargo fmt`。

## 没做与限制

- 未启动窗口，因此未做真实 WebView 缩放和 27 寸 / 小屏视觉手测；`setZoom` 成功路径与 CSS 回退路径使用可注入替身测试，生产构建已验证 API 类型与权限名。
- 未发真机模型请求，未读取凭据文件。
- 未新增 `set_ui_zoom` Rust 命令，因为当前 Tauri 2 JS API 已直接提供 `setZoom`。
- 未 commit、未 push，未修改 `D:\work\aicross` 主工作树。
