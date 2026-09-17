# REPORT-33 控制台遗留小问题收口

## 结果

TASK-33 七条均已完成。改动限于 `src-ui/**`、既有前端测试与本报告；未修改根目录 `src/**`，未启动窗口、未发真机请求、未调用真实模型，未执行 `cargo fmt`，未 commit、未 push。

## 七条对应

1. **运行详情表格宽度**
   - 十列宽度统一为 `68 / 64 / 130 / 122 / 108 / 60 / 86 / 86 / 88 / 88px`，合计 `900px`；表格最小宽度由 `1120px` 降为 `900px`，单元格横向 padding 由 `12px` 降为 `6px`。
   - 请求模型、实际模型继续允许断行；默认约 `940px` 的中间栏可完整显示十列，低于 `900px` 时才出现表内横向滚动。
   - token 从 `10000` 起使用紧凑显示，例如 `49.4k`、`1.2m`；行、总计和阶段小计的悬停标题保留完整数值。
   - 红测：实现前 `RUN_TABLE_COLUMN_WIDTHS` 不可读取，CSS 仍含 `min-width: 1120px`，TASK-33 表格测试失败。

2. **结论的验收链与修正轮数**
   - 结论改为沿图祖先关系找出每条验收链的末次验收；任一末次 `NEEDS_FIX` 为未通过，否则任一非 `PASS` 为无法识别，全部 `PASS` 才通过。
   - `startPipelineHeadless` 返回本次 `fixRoundNodeIds`；结论只统计该集合内、直接接在验收节点后的修正执行节点。预设自带多个验收阶段不再被写成修正轮。
   - 新增不等深并行分支、预设自带两次验收、一轮修正后通过、到上限未通过四类测试；另有一格通过 `createConsoleApplyHandler` 完整驱动一轮修正后通过。
   - 红测：实现前四类纯测试分别得到“通过（第 2 轮验收通过）”或错误轮数；真实接线格将 `pipeline.fixRoundNodeIds` 临时替换为空集合后，收到“通过（验收一次通过）”而非“通过（修正 1 轮后通过）”，测试按预期失败，随后已恢复实现。

3. **详情页焦点**
   - 打开后焦点落到“返回对话”；Tab 与 Shift+Tab 根据页内可聚焦元素循环。
   - 打开前保存焦点；关闭后原元素仍在文档中则恢复原元素，否则回到消息输入框。
   - Tab 目标与关闭后焦点目标均抽为纯函数测试，没有引入 jsdom。
   - 红测：实现前两个纯函数不存在，焦点测试以 `is not a function` 失败；组件也没有打开元素记录、循环或关闭恢复接线。

4. **首条心跳前计时**
   - 阶段开始只创建 `pending` 行，显示“排队中”和耗时“—”；首条心跳到达后才改为 `running`，并以引擎 `elapsedMs` 为起点继续走表。
   - 红测：实现前阶段开始立即得到 `status: running`，测试期望 `pending` 时失败。

5. **外部通道按本次运行计修正轮**
   - `runConsoleDispatch` 调用 `startPipelineHeadless` 时传入 `new Set(created)`，排除旧图中的历史验收节点。
   - 真实接线测试在同一张图连续发送 `channel-1`、`channel-2` 两次事件；第二次首次验收返回 `NEEDS_FIX` 后实际请求顺序为“执行、验收、执行、验收”。
   - 红测：实现前第二次只产生“执行、验收”两次请求，期望四次时失败。

6. **关闭详情页时保留滚动**
   - layout effect 恢复消息区滚动位置时设置一次性标记；自动滚底 effect 在同一帧读取后清除标记并跳过。
   - 跳过判定抽为纯函数测试。
   - 红测：实现前纯函数不存在，且组件源码没有 `skipAutoScrollRef.current = true`，测试失败。

7. **收尾播报阶段号**
   - `announceResults` 先生成换算后的 `displayed` 结果；运行记录与逐路收尾播报均从该集合读取，和阶段系统提示、详情页一致。
   - 红测：实现前收尾仍对原始 `collected` 去重，源码接线断言失败。

## 红测记录

首次只加入 TASK-33 测试、尚未实现时运行：

```text
npx vitest run src/__tests__/console-polish-33.test.ts --configLoader native --pool threads

Test Files  1 failed (1)
Tests  9 failed (9)
```

随后增加的 `createConsoleApplyHandler` 真实接线格按上述第 2 条做定向变异：

```text
npx vitest run src/__tests__/console-polish-33.test.ts -t "createConsoleApplyHandler 真实接线" --configLoader native --pool threads

Expected: "通过（修正 1 轮后通过）"
Received: "通过（验收一次通过）"
Test Files  1 failed (1)
Tests  1 failed | 9 skipped (10)
```

变异后已立即恢复 `pipeline.fixRoundNodeIds`，完整测试复跑通过。

## 验证结果

```text
cargo check --all-targets

Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 03s
```

零 warning。

```text
cargo test

test result: ok. 484 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 29.72s
```

忽略项均为仓库既有真机或真实模型测试，本任务按要求未运行。

```text
cd src-ui
npx vitest run --configLoader native --pool threads

Test Files  46 passed (46)
Tests  666 passed (666)
```

```text
cd src-ui
npm run build

✓ 350 modules transformed.
✓ built in 588ms
```

构建成功；Vite 仍报告既有的单个 chunk 超过 500 kB 提示，不影响退出码。

```text
python scripts/copy-audit.py

共 7 处命中，6 个文件（含误报，需人工复核）
```

退出码 0。命中均为既有项：本次涉及的 `ConsoleRunDetail.tsx` 在 HEAD 中已因占位符“—”命中同一规则，本次没有新增命中文件或规则；其余命中位于未修改字符串。

```text
git diff --check

（无输出）
```

`git ls-files --eol` 显示所有本次修改的已跟踪前端文件均保持 `i/lf w/lf`；新增测试与报告使用 LF。

## 未做

- 未修改根目录 `src/**`。
- 未引入 jsdom 或新依赖。
- 未启动 Tauri 窗口，未发真机请求，未调用真实模型。
- 未执行 `cargo fmt`。
- 未 commit、未 push。
