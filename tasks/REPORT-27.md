# REPORT-27 控制台任务详情页 + 收尾给最终结论

## 完成内容

### 一、任务详情页

- 控制台模式派发现在生成独立运行记录，并通过现有 `dispatchConsolePreset` 接线逐阶段写入：角色、agent、请求模型、实际模型、状态、验收结论、输入/输出 token、缓存命中、耗时与主备切换。
- 派发中在「处理中…」旁显示「查看进度」；阶段消息与收尾消息也关联同一运行记录。详情页占满中间区域，有「返回对话」，Esc 可返回，并用 `role="dialog"`、`aria-modal="true"` 配合现有焦点守卫。
- 详情页按阶段分组，每阶段有小计；全局同时列墙钟耗时和各路耗时合计，并说明两者口径。已完成行输入、输出 token 同为 0 时显示「未报告」，不计入 token 合计，合计注明未报告路数。
- `dispatch-progress` 心跳已接入运行记录。进行中的行在两次心跳之间由前端时钟继续计时；工具调用存在时显示最近工具和次数。
- 运行记录跟随控制台会话写入原有 `aicross:console-sessions:v1`，每个会话保留最近 20 份。旧数据没有该字段时加载为空数组；重启加载时，未结束记录和未结束行冻结为「已中断」。

### 二、收尾给最终结论

- 收尾消息改为两句：第一句给验收结论，第二句给总路数、运行失败路数和墙钟耗时。
- 结论取最后一轮的验收/验证节点：任一 `NEEDS_FIX` 为未通过；无 `NEEDS_FIX` 但含 `UNKNOWN` 为「验收结论无法识别」；全部 `PASS` 为通过；无验收节点为「已完成（本模式不含验收）」；流水线停止时优先写停止原因。
- 「成功/失败」统计改为「运行失败 N 路」，避免与验收结论混淆。

## 红测

先加入最小占位接口，在未实现行为时逐条运行：

1. `cd src-ui && npx vitest run src/__tests__/console-run-detail-27.test.ts --configLoader native --pool threads`
   - 结果：7 项测试全部失败。
   - 失败分别钉住：真实 apply handler 接线仍输出旧收尾且没有运行记录；未报告 token/两种耗时口径未实现；运行中记录重启未转已中断；旧数据未补空记录数组；20 份上限未实现；实际模型缺失文案未实现；持久化记录解析未实现。
2. `cd src-ui && npx vitest run src/__tests__/console-run-detail-27.test.ts -t "心跳落到|最终结论区分" --configLoader native --pool threads`
   - 结果：2 项测试全部失败。
   - 心跳测试仍保留 `pending` 且计时不走；结论测试不能区分未知、中途停止与无验收模式。
3. `cd src-ui && npx vitest run src/__tests__/console-run-detail-27.test.ts -t "详情页是" --configLoader native --pool threads`
   - 结果：1 项失败，详情页文件不存在，无法满足满版、Esc、返回入口和焦点守卫要求。

实现后定向复跑：`10 passed`。真实接线格从 apply handler 驱动两阶段假派发，包含一路 token 为 0、一路切备选、一路 `NEEDS_FIX`；断言运行记录、收尾结论，并经 reducer 写入后由初始化链路模拟重启恢复。

## 最终验证

- `cargo check --all-targets`
  - 结果：退出码 0；`Finished dev profile`；零 warning。
- `cargo test`
  - 结果：退出码 0；`464 passed; 0 failed; 11 ignored`。忽略项均为仓库既有真机/真实模型测试，本次未运行。
- `cd src-ui && npx vitest run --configLoader native --pool threads`
  - 结果：退出码 0；`43 passed`，`645 passed`。
- `cd src-ui && npm run build`
  - 结果：退出码 0；TypeScript 构建和 Vite 构建成功，`350 modules transformed`。Vite 保留既有的单个大 chunk 提示，不影响构建结果。
- `python scripts/copy-audit.py`
  - 结果：退出码 0；共 6 处存量命中，均位于本期未改的 5 个文件；本期改动文件无命中，新增 0 处。

## 范围与换行

- 未修改任务书禁区：`src/**`、`src-ui/src/tauri.ts`、`src-ui/src/graph/context.ts`、`src-ui/src/graph/types.ts`、`src-ui/src/graph/dispatch.ts`。
- 未修改旧画布 `DispatchOverlay` 行为。
- 既有 LF 文件继续为 LF；`src-ui/src/store/app-state.tsx` 保留原有混合换行计数（HEAD：CRLF 754 / LF 211；当前：CRLF 754 / LF 231），仅新增行使用该文件现有的 LF 区段风格。未运行 `cargo fmt`。
- 未启动窗口，未发真机请求，未调用真实模型；未 commit、未 push。

## 未做

- 未改引擎、派发流程或修正轮逻辑。
- 未做成本折算、额度百分比或图表。
- 按任务书未改 backlog。
