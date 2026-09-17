# fix-33-r1 控制台遗留小问题收口 · 复审修正轮

依据 GLM-5.3-Flash（可交付）与 Antigravity/Gemini 3.8 Flash（判不可交付）的审查，编排者逐条对照代码核过。禁区与 TASK-33 任务书相同。

## 编排者核过「不成立」的，不要改

- **Gemini P2「修正执行节点的父为『验证』时漏计」不成立**：`graph/dispatch.ts` 的 `appendCorrectionRound` 开头要求 `accept.target.role === '验收'`，修正执行节点的 `parents` 恒为该验收节点，不会挂在「验证」下。

## 必修

1. **预设自带的后续执行节点被当成修正轮（Gemini P1，已核实）**
   `fixRoundNodeIds` 初值是本次 stamp 新建的**全部**节点（`console-chat.ts` / `console-channel.ts` 传 `new Set(created)`），`correctionRoundCount` 只看「role=执行、在集合内、父为验收」，于是「执行 → 验收 → 执行 → 验收」这类预设没修正也会写成「修正 1 轮后通过」。
   修法：轮数只数**流水线推进中新生长的**修正执行节点——`startPipelineHeadless` 另外返回（或由调用方计算）「集合里去掉初始 created 的部分」，`finishConsoleRun` 用它计轮。修正轮上限的计数范围（`fixRoundNodeIds` 本身）不变。补测试：预设「执行 → 验收 → 执行 → 验收」全 PASS → 「验收一次通过」。

2. **并行分支同一轮修正被数成多轮（Gemini P3，已核实）**
   现在 `filter(...).length` 数节点数，两条并行分支各修一次会写「修正 2 轮后」。
   修法：按**每条验收链**上的修正深度计（末次验收节点往上数经过了几个修正执行节点），取各链最大值。补测试：两条并行分支各修正一次后都通过 → 「修正 1 轮后通过」。

3. **两格静态源码断言换成行为断言（两家共同）**
   - 收尾播报用换算后阶段号（第 7 条）：在已有的 `createConsoleApplyHandler` 真实接线测试里，对「同一张图第二次应用」断言收尾播报 / `onRunUpdate` 记录里的 stage 是换算后的（从 1 起）。删除 `readFileSync` 匹配源码的那一格。
   - 自动滚底跳过标记（第 6 条）：纯函数行为测试保留；组件接线的源码断言可以保留，但要补一格纯函数层面的「置标记 → 下一次读到后清除 → 再下一次不跳过」序列测试，证明标记不会卡住。

4. **表格宽度余量（GLM 1）**
   中间栏 940px 减去详情页左右内边距与阶段框边框后可用约 898px，小于表格 `min-width: 900px`，会出现约 2px 的表内横向滚动。把列宽合计与 `min-width` 压到 ≤ 880px（或同时收窄 `.console-run-body` 左右内边距），留出余量。报告写清最终数字。

5. **详情页焦点兜底（Gemini P5）**
   根容器加 `tabIndex={-1}`；`runDetailTabTarget` 返回 null 且事件发生在详情页打开期间时，同样 `preventDefault` 并把焦点放回根容器，不让焦点溢出到左右栏。纯函数测试补一格「页内无可聚焦元素」。

6. **正在查看的记录被 20 份上限挤掉时详情页静默退回（GLM 2）**
   `UPSERT_CONSOLE_RUN` 裁剪时若挤掉的是当前 `openRunId` 的记录，界面直接退回聊天、不恢复焦点与滚动。修法二选一，报告写清：(a) 详情页打开期间持有一份该记录的快照，被裁剪后仍显示快照并标「这份记录已被较新的运行替换，关闭后不再保留」；(b) 检测到记录消失时主动走一次关闭恢复流程（焦点、滚动都恢复）。倾向 (b)，更简单。补纯函数测试。

## 验证

`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`、`python scripts/copy-audit.py`（占位符「—」属已知误报，不得新增其它命中）。每条先确认去掉实现会红，在报告里写清怎么红的。

保持文件原有换行；不 `cargo fmt`；不引入 jsdom；不起窗口、不发真机请求、不调真实模型；不 commit、不 push。报告写 `tasks/REPORT-33-R1.md`，逐条对应。
