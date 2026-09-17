# REPORT-33-R1 控制台遗留小问题收口·复审修正轮

## 结果

`fix-33-r1` 六条必修均已完成。改动限于 `src-ui/**`、前端测试和本报告；没有修改根目录 `src/**`。任务书标为“不成立，不要改”的“验证角色父节点漏计”未改，`appendCorrectionRound` 仍只从“验收”节点生成修正轮。

## 六条对应

1. **预设节点不再计入修正轮**
   - `startPipelineHeadless` 保留完整 `fixRoundNodeIds` 供修正轮上限判断，同时另行返回扣除初始节点后的 `generatedFixRoundNodeIds`。
   - 控制台收尾只把 `generatedFixRoundNodeIds` 交给 `finishConsoleRun`。预设自带的“执行 → 验收 → 执行 → 验收”全 PASS 现在写为“通过（验收一次通过）”；真实接线的一轮修正仍写为“通过（修正 1 轮后通过）”。
   - 红测：实现前，预设四节点测试收到“通过（修正 1 轮后通过）”而非“通过（验收一次通过）”。

2. **并行分支按修正深度计轮**
   - 从每条末次验收节点沿父链计算生成的修正执行节点深度，再取各链最大值；不再把并行分支的修正节点数相加。
   - 两条并行链各修正一次后都通过，结论为“通过（修正 1 轮后通过）”。
   - 红测：实现前同一用例收到“通过（修正 2 轮后通过）”。

3. **两处测试改为行为验证**
   - 阶段号：`announceResults` 先使用换算后的结果集合，再更新运行记录和生成逐路收尾消息；消息携带换算后的 `stage`，持久化解析也保留该字段。既有 `createConsoleApplyHandler` 真实接线测试对同一张图第二次应用，断言逐路消息与 `onRunUpdate` 记录均为阶段 `1 / 2 / 3 / 4`；已删除该项的静态源码断言。
   - 自动滚底：`consoleAutoScrollStep` 的序列测试覆盖“标记为真 → 下一次读到后跳过并清零 → 再下一次恢复滚底”。
   - 红测：实现前真实接线消息的阶段字段为四个 `undefined`；滚动纯函数不存在，序列测试报 `is not a function`。

4. **表格宽度留出余量**
   - 十列宽度调整为 `66 / 62 / 126 / 118 / 106 / 58 / 84 / 84 / 86 / 90px`，合计 `880px`；CSS 的 `width` 下限和 `min-width` 同步为 `880px`。
   - 相对任务书给出的约 `898px` 可用宽度，保留约 `18px` 余量。
   - 红测：实现前列宽合计为 `900px`，`toBeLessThanOrEqual(880)` 收到 `900`。

5. **详情页无可聚焦元素时回到根容器**
   - 详情根容器保留 `tabIndex={-1}`；Tab 处理始终阻止默认行为。页内没有可聚焦元素时，`runDetailTabPlan` 返回根容器方案并调用根容器 `focus()`，不会溢出到左右栏。
   - 红测：实现前无焦点项只能得到 `null`，新的根容器计划函数不存在，测试报 `is not a function`。

6. **被 20 份上限挤掉的详情主动关闭**
   - 采用任务书方案 **(b)**。纯函数检测 `openRunId` 对应记录已不存在后，组件主动调用与“返回对话”相同的 `closeRunDetail`，随后由既有关闭恢复流程还原消息区滚动与焦点。
   - 红测：实现前记录消失没有 `close` 动作，新的可用性判定函数不存在，测试报 `is not a function`。

## 红测记录

先加入六条必修对应的测试、尚未写实现时运行：

```text
cd src-ui
npx vitest run src/__tests__/console-polish-33.test.ts src/__tests__/console-per-apply-29.test.ts --configLoader native --pool threads

Test Files  2 failed (2)
Tests  7 failed | 8 passed (15)
```

七个失败点分别是：表格宽度收到 `900`；预设节点误计为修正 1 轮；两条并行链误计为修正 2 轮；`runDetailTabPlan` 不存在；`consoleAutoScrollStep` 不存在；`runDetailAvailabilityAction` 不存在；真实接线的四条收尾消息阶段均为 `undefined`。实现后用同一命令复跑：

```text
Test Files  2 passed (2)
Tests  15 passed (15)
```

## 验证结果

```text
cargo check --all-targets

Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.81s
```

零 warning。

```text
cargo test

test result: ok. 484 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 25.72s
```

忽略项均为仓库既有真机或真实模型测试，本轮按要求未运行。

```text
cd src-ui
npx vitest run --configLoader native --pool threads

Test Files  46 passed (46)
Tests  667 passed (667)
```

```text
cd src-ui
npm run build

✓ 350 modules transformed.
✓ built in 587ms
```

构建成功；Vite 仍有既有的单个 chunk 超过 500 kB 提示。

```text
python scripts/copy-audit.py

共 7 处命中，6 个文件（含误报，需人工复核）
```

退出码 0。命中数量与首版报告一致；本轮没有新增文案审计命中。`ConsoleRunDetail.tsx` 的命中仍是占位符“—”误报。

```text
git diff --check

（无输出）
```

`git ls-files --eol` 显示本轮涉及的已跟踪前端文件均保持 `i/lf w/lf`；新增报告使用 LF。

## 未做

- 未修改根目录 `src/**`。
- 未引入 jsdom 或新依赖。
- 未启动窗口，未发真机请求，未调用真实模型。
- 未执行 `cargo fmt`。
- 未 commit、未 push。
