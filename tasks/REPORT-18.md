# REPORT-18 模式做减法

对应 `tasks/TASK-18-preset-simplify.md`。

## 改法与测试

### 1. 三类预设分组

- `src-ui/src/graph/label.ts` 新增纯函数 `presetGroupOf`：
  - `builtin-shared`、`builtin-economy`、`builtin-blind` → 基础模式；
  - `builtin: false` → 自定义模式；
  - 其余内置预设 → 更多模式。
- 分类仅读取既有 `id` 与 `builtin`；未改内置预设的 id、name、阶段、Target 或其它机器语义。
- 测试：`presetGroupOf 按内置 id 与 builtin 分成基础、自定义、更多三类`。

### 2. 右栏预设下拉

- 新增共用组件 `PresetGroups.tsx` 与样式 `PresetGroups.css`。
- 右栏下拉按基础模式、自定义模式、更多模式排列；基础模式固定为共享、经济、盲审，自定义模式位于其后。
- 更多模式用原生 `<details>` 默认折叠；展开后可选择擂台、评审、蜂群，末尾显示置灰的红蓝模式（规划中）与群聊模式（规划中）。
- 保留 `OrchestrationPanel.tsx` 中优先选择 `builtin-economy` 的原逻辑。
- 测试：
  - `右栏下拉按基础、自定义、更多排序，默认收起更多模式且经济模式仍为当前项`；
  - `分组菜单固定基础模式顺序，自定义居中，更多模式末尾含置灰规划项`。

### 3. 图上「盖预设」菜单

- `GraphTab.tsx` 改为复用 `PresetGroups`，因此与右栏使用同一分类、显示名和排序；更多模式默认折叠，展开后仍调用原有 `applyStamp`。
- 测试：`图上盖预设菜单复用同一分组组件`。

### 4. 讲解页

- `ExplainOverlay.tsx` 抽出可直接测试的 `ExplainContent`。
- 讲解页先用一句话分别说明共享、经济、盲审三种基础模式，再单列更多模式说明擂台、评审、蜂群。
- 保留 TASK-12 已要求的验收判定、派发前验证输出和角色默认说明。
- 测试：`讲解页先逐项说明三种基础模式，再单列更多模式`；全量测试中的 TASK-12 文案测试继续通过。

### 5. 手册

- `docs/MANUAL.md` 的「内置预设」改为「基础模式 / 自定义模式 / 更多模式」三层，说明更多模式默认折叠。
- 测试：`手册的内置预设按基础、自定义、更多三层说明`。

## 红测记录

实现前运行：

```text
npx vitest run src/graph/__tests__/preset-simplify-18.test.tsx --configLoader native --pool threads

FAIL  src/graph/__tests__/preset-simplify-18.test.tsx
Error: Cannot find module '../../components/right/orchestration/PresetGroups.tsx'
Test Files  1 failed (1)
Tests  no tests
```

实现后 TASK-18 测试 6 条通过。首次全量回归还发现讲解页误删 TASK-12 既有说明，表现为：

```text
FAIL  src/graph/__tests__/review-defaults-12.test.ts
AssertionError: expected ... to contain '看产物给 PASS / NEEDS_FIX'
Test Files  1 failed | 30 passed (31)
Tests  1 failed | 473 passed (474)
```

补回既有说明后全量测试通过。

## 验证结果

### 前端测试

命令：

```text
cd src-ui && npx vitest run --configLoader native --pool threads
```

结果原文：

```text
Test Files  31 passed (31)
Tests  474 passed (474)
Duration  958ms
```

### 前端构建

命令：

```text
cd src-ui && npm run build
```

结果原文：

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build

vite v8.0.7 building client environment for production...
✓ 336 modules transformed.
✓ built in 619ms
```

### 换行与范围

- 修改前涉及的文件均为 LF；修改后仍为 LF，新文件也为 LF。
- 未修改 `src-ui/src/tauri.ts`，未修改任何 Rust 文件。
- 未运行 `cargo check --all-targets` 或 `cargo test`，因为 Rust 未改。

## 没做与限制

- 未启动窗口；按任务书只做组件渲染测试、全量 Vitest 与生产构建，未做窗口视觉手测。
- 未发真机请求。
- 未运行 `cargo fmt`。
- 未 commit，未 push。
