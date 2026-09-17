# REPORT-27-R1 控制台任务详情页复审修正

## 修正内容

### 一、异常收尾与订阅释放

- `dispatchConsolePreset` 从建立心跳订阅起进入 `try/catch/finally`：无论 `commitGraph`、反馈写入或后续流程在哪一步抛错，`finally` 都会释放订阅。
- 异常分支用当前运行记录、当前图、空结果与 `String(error)` 调用 `finishConsoleRun`，同步最后一份非 `running` 记录后重新抛出，由既有 apply handler 展示错误。
- `startPipelineHeadless` 会把阶段回调异常折入 `pipeline.error`；因此额外保留反馈写入异常，并在管线返回后重新抛出，确保首阶段消息写入失败也走统一异常收尾。

### 二、心跳不再触发全量持久化

- 心跳先计算下一份记录；外部 `runId` 或其他无效心跳返回同一对象时立即退出，不再 dispatch。
- 新增 `UPDATE_CONSOLE_RUN_PROGRESS`，只更新内存状态，不进入 `CONSOLE_PERSIST_ACTIONS`。阶段开始、结果回流与最终收尾继续使用 `UPSERT_CONSOLE_RUN` 写盘。
- `ConsoleChat` 分开接线持久更新与心跳更新；测试通过真实 reducer 和 `localStorage` 计数确认心跳期间没有写盘，阶段、结果和收尾仍写盘。

### 三、Esc 行为测试

- 抽出纯函数 `runDetailKeyAction`：仅非输入法组合状态下的 Escape 返回 `close`，其余返回 `null`。
- 详情组件通过该函数决定是否返回；测试覆盖 Escape、输入法组合和其他按键，并保留组件源码接线断言。未引入 jsdom。

### 四、死参数

- 删除 `createConsoleRunRecord` 未使用的 `_graph`、`_nodeIds` 参数，并同步唯一调用处。

## 红测

实现前逐项运行 `src/__tests__/console-run-detail-27.test.ts` 的新增测试，确认旧实现会红：

1. `-t "阶段消息异常时终止运行记录并释放心跳订阅"`
   - 旧实现把首阶段 `append` 异常折成管线结果并正常 resolve，测试期望异常统一收尾后 rethrow，报错为 `promise resolved ... instead of rejecting`。
   - 新实现下订阅 `dispose` 恰好调用一次，最后一次 `onRunUpdate` 状态不是 `running`，异常继续抛给外层。
2. `-t "只更新本次运行的心跳且不写盘"`
   - 旧实现的写盘计数在外部心跳和本次心跳后从 `[2, 2, 2]` 实际变为 `[2, 3, 4]`。
   - 新实现下外部心跳不 dispatch，本次心跳只走非持久化 action；阶段、结果、收尾的持久更新均有记录。
3. `-t "Esc 按键判定排除输入法组合与其他按键"`
   - 旧实现报 `TypeError: runDetailKeyAction is not a function`。
   - `-t "详情页是可由 Esc 退出"` 同时确认旧组件不含 `runDetailKeyAction(event)` 接线。
4. `-t "运行记录构造器不保留未使用"`
   - 旧实现仍含 `_graph: Graph` 与 `_nodeIds: string[]`，源码断言失败。

修正后定向复跑：`1 file passed`，`14 passed`。其中心跳格通过实际 `dispatchConsolePreset`、订阅回调、app reducer 与内存 `localStorage` 串起真实前端接线；派发 IO 全部为假实现，未请求真实模型。

## 最终验证

- `cargo check --all-targets`
  - 退出码 0；`Finished dev profile`；零 warning。
- `cargo test`
  - 退出码 0；`464 passed; 0 failed; 11 ignored`。忽略项为仓库既有真机或真实模型测试，本轮未运行。
- `cd src-ui && npx vitest run --configLoader native --pool threads`
  - 退出码 0；`43 passed`，`649 passed`。
- `cd src-ui && npm run build`
  - 退出码 0；TypeScript 与 Vite 构建成功，`350 modules transformed`。保留既有单个大 chunk 提示，不影响构建结果。
- `python scripts/copy-audit.py`
  - 退出码 0；仍为 5 个未改文件中的 6 处存量命中，与首版报告一致；本轮新增 0 处。

## 保持不改

- 不等深并行验收分支的结论口径，以及预设自带多个验收阶段时「修正 N 轮后」的措辞：现有经济模式等预设是一条线性验收链，其他口径需产品先定，本轮按修正单留在 backlog。
- Tab 焦点循环与关闭后焦点归还：详情页是聊天区整体替换，不是浮层；现有 `aria-modal` 豁免保持不变，本轮不加焦点陷阱。
- 阶段开始至首条心跳之间包含排队时间：首条心跳会按引擎值纠正，本轮保持现状。

## 范围与限制

- 未修改禁区：`src/**`、`src-ui/src/tauri.ts`、`src-ui/src/graph/context.ts`、`src-ui/src/graph/types.ts`、`src-ui/src/graph/dispatch.ts`。
- 未改上述三项「保持，不改」行为；未引入 jsdom；未运行 `cargo fmt`。
- 保持既有换行：LF 文件仍为 LF；`app-state.tsx` 从修正前 CRLF 754 / LF 231 变为 CRLF 754 / LF 233，仅两条新增 action/case 行沿用其当前 LF 区段。
- 未启动窗口、未发真机请求、未调用真实模型；未 commit、未 push。

## 编排者核对 Gemini 复审（09-17）

- **P0「分支回滚了 TASK-28」不成立**：复审材料用的是 `git diff main`，而 main 在本分支分出后已合入 TASK-28，于是 TASK-28 的代码显示成被删。本分支工作区对 `src/**`、`docs/**` 无任何改动（`git status -- src docs` 为空），三方合并不会回滚。是编排者打包材料的口径错，不是实现问题。
- **P2 收尾连写两次持久化**：属实但无害——`replaceRow` 保留记录状态，第二次写只是冗余；调换顺序仍是两次写，不值得为此改结构，不改。
