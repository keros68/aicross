# fix-27-r1 控制台任务详情页 · 复审修正轮

依据 GLM-5.3-Flash 与 Antigravity/Gemini 3.8 Flash 的审查（两家都判「可交付但有未闭合项」）。禁区与 TASK-27 任务书相同。

## 必修

1. **心跳订阅与运行记录收尾没有兜底（两家共同第 1 条）**
   `console-chat.ts` 建立订阅到 `dispose()` / `finishConsoleRun` 之间约百行没有 try/finally。编排者核过：`startPipelineHeadless` 自己会把派发异常折成 `pipeline.error`，所以现在难触发；但 `commitGraph` / `append` 抛错、或以后改动一脚踩空，就会订阅泄漏到重启、记录一直「进行中」、按钮一直「查看进度」。
   修法：从建立订阅起包 try/catch/finally——finally 里 `dispose()`；catch 里 `finishConsoleRun(runRecord, graph, [], String(error), now())` 并 `onRunUpdate`，然后 rethrow（外层 `createConsoleApplyHandler` 的 `.catch` 负责展示错误）。补测试：让 `feedback.append` 在第 1 阶段消息时抛错，断言订阅被 dispose、最后一次 `onRunUpdate` 的记录状态不是 running。

2. **每条心跳都全量同步写盘（两家共同）**
   进度回调无条件 dispatch `UPSERT_CONSOLE_RUN`，该 action 在持久化集合里，每次对全部控制台会话 `JSON.stringify` + `localStorage.setItem`（消息含完整模型输出，体积可观）；runId 不匹配时 `withConsoleRunProgress` 原样返回也照样写。旧画布同时派发时它的心跳也会被接住白写。
   修法：(a) `next === runRecord` 时不 dispatch；(b) 心跳引起的更新走一个**不在持久化集合里**的 action（例如 `UPDATE_CONSOLE_RUN_PROGRESS`），阶段开始、结果回流、收尾仍走会落盘的 `UPSERT_CONSOLE_RUN`。重启时进行中记录本来就标「已中断」，丢失最后几次心跳的计时细节可以接受。补测试：外部 runId 的心跳不触发 dispatch；本 run 的心跳不触发持久化，阶段/结果/收尾触发。

3. **Esc 返回只有源码字符串断言（两家共同）**
   现在的测试是 `readFileSync` + `toContain('role="dialog"')`，没有证明 Esc 真能返回。vitest 跑 node 环境没有 jsdom，不要引入 jsdom。
   修法：把按键判定抽成纯函数（例如 `runDetailKeyAction(event) => 'close' | null`，含 `isComposing` 与非 Escape 的排除），组件里用它；测试走这个函数，并保留一格断言组件确实用它（可以沿用源码断言，但判定本身要有行为测试）。

## 顺手（很小，一起做）

4. `createConsoleRunRecord` 的 `_graph`、`_nodeIds` 占位死参删掉（GLM 5）。

## 保持，不改（记录理由，写进报告即可）

- **不等深并行验收分支的结论口径**（GLM 3）与**预设自带多个验收阶段时「修正 N 轮后」措辞**（Gemini 4）：经济模式等现有预设都是线性的一条验收链，触发前提苛刻；「轮」的口径需要产品先定。编排者记进 backlog，本轮不改。
- **Tab 焦点循环与关闭后焦点归还**（Gemini 2 的延伸）：详情页整体替换聊天区而不是浮层弹窗，焦点守卫已通过 `aria-modal` 豁免；不做焦点陷阱。
- 阶段开始到首条心跳之间计时含排队时间（GLM 6）：首条心跳后被引擎值纠正，可接受。

## 验证

`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`、`python scripts/copy-audit.py` 不新增命中。每条先确认去掉实现会红，在报告里写清怎么红的。

保持文件原有换行（`app-state.tsx` 是混合换行，只做最小改动）；不 `cargo fmt`；不起窗口、不发真机请求、不调真实模型；不 commit、不 push。报告写 `tasks/REPORT-27-R1.md`，逐条对应。
