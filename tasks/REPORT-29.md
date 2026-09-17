# REPORT-29 控制台每次应用模式按本次计阶段与修正轮 + 详情页显示修整

## 结果

TASK-29 的 86、87、88 三条已完整实现。未修改 `src/**`，未启动窗口，未发真机请求，未调用真实模型，未 commit，未 push。

## 86. 每次应用独立计阶段与修正轮

- 保留 `existingGraph` 续写，未改成空图；真实 apply 接线测试断言两次运行的 graph id 相同。
- 后端派发请求的 `stage` 仍保留全图深度，避免改动引擎排序和留痕语义；仅在控制台系统提示与运行记录层换算为本次从第 1 阶段开始。
- `startPipelineHeadless` 增加可选的 `fixRoundNodeIds`。控制台以本次 stamp 新建的节点初始化该集合，推进过程再把本次生长的修正节点纳入；旧图中的验收与修正节点不再占用本次上限。
- 旧画布 `DispatchOverlay` 不传该参数，仍按全图计数；单元测试同时断言默认全图语义和控制台范围语义。
- 新增真实接线测试：通过 `createConsoleApplyHandler` 实际进入 `dispatchConsolePreset`，在同一 `existingGraph` 上连续两次假派发。两次均跑执行 → 验收 NEEDS_FIX → 修正执行 → 再验收；第二次系统提示和详情行为阶段 1–4，共 4 路，收尾为“修正 1 轮后验收再次要求修正，已到上限”。

## 87. 运行详情页

- 每个阶段表格共用同一组 `colgroup` 列宽，表格启用 `table-layout: fixed`。
- 详情页和阶段容器限定为中间栏宽度，页面本身隐藏横向溢出；只有 `.console-run-table-wrap` 允许横向滚动。
- `pending` / `running` 行的“实际模型”显示“—”；已完成且没有真实模型时才显示“未报告”，kimi 仍显示“kimi 不报真身”。
- 页头不再显示 `console-mode-<uuid>`，改为“模式名 · YYYY-MM-DD HH:mm”，使用本地时间。

## 88. 详情页返回后恢复滚动

- 打开详情前记录消息流的 `scrollTop` / `scrollHeight` / `clientHeight` / 消息数和是否处于底部。
- 返回时在布局 effect 内恢复：原先不在底部则回到原位置；原先在底部且详情页期间有新消息，则滚到最新。
- 滚动判定与目标位置计算保持为纯函数，Vitest 仍为 node 环境，未引入 jsdom。

## 红测

1. 先新增 `src-ui/src/__tests__/console-per-apply-29.test.ts`，在未加实现时运行：

   ```text
   npx vitest run src/__tests__/console-per-apply-29.test.ts --configLoader native --pool threads
   Test Files  1 failed (1)
   Tests  4 failed (4)
   ```

   - 86：第二次系统提示实际是全图阶段 `['4', '5']`，不是本次 `['1', '2', '3', '4']`；且没有派出修正轮。
   - 87 模型：`pending` 实际得到“未报告”，期望“—”。
   - 87 页头：`consoleRunHeaderLabel is not a function`。
   - 88：`consoleScrollSnapshot is not a function`。

2. 实现后又做一次修正轮范围变异：临时让 `reducePipelineStep` 忽略 `fixRoundNodeIds`，再运行：

   ```text
   npx vitest run src/graph/__tests__/dispatch.test.ts src/__tests__/console-per-apply-29.test.ts -t "超过验收修正上限|同一张图连续应用两次" --configLoader native --pool threads
   Test Files  2 failed (2)
   Tests  2 failed | 37 skipped (39)
   ```

   真实 apply 接线测试回退为只有阶段 1、2；旧画布/范围成对测试的控制台分支从 `true` 变为 `false`。恢复实现后同一命令为 `2 passed | 37 skipped`。

## 验证

1. `cargo check --all-targets`

   ```text
   Compiling aicross v0.1.0 (D:\work\aicross-ui)
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.89s
   ```

   零 warning。

2. `cargo test`

   ```text
   test result: ok. 473 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 26.49s
   ```

3. `cd src-ui && npx vitest run --configLoader native --pool threads`

   ```text
   Test Files  44 passed (44)
   Tests  653 passed (653)
   ```

4. `cd src-ui && npm run build`

   ```text
   tsc -b && vite build
   350 modules transformed.
   built in 572ms
   ```

   构建成功；Vite 仍输出 `main` chunk 超过 500 kB 的体积提示，无 TypeScript 或打包失败。

5. `python scripts/copy-audit.py`

   ```text
   共 6 处命中，5 个文件（含误报，需人工复核）
   ```

   命中仍位于 `DispatchOverlay.tsx`、`SettingsModal.tsx`、`console-channel.ts`、`graph/stamp.ts`、`resources/types.ts`；本次修改文件无新命中。

6. `git diff --check` 无输出。所有被修改的既有前端文件在 HEAD 中均为 LF，修改后仍是纯 LF；新增测试也是 LF。未运行 `cargo fmt`。

## 未做与限制

- 按任务书没有窗口手测，因此表格滚动和返回后的视觉效果仅完成了样式/纯函数/构建验证，未声称真机视觉验收。
- 未改 backlog，未改 `docs/PRODUCT.md`，未改 `src/**`。
