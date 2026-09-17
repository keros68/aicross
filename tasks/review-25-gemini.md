# TASK-25 盲审 · Antigravity / Gemini 3.8 Flash (High)

2026-09-17，材料全部内联。

---

**总判：不可交付**

---

### 严重度编号清单

#### 1. 【严重】单线续聊将普通业务错误与所有异常无差别判定为会话失效，误清有效会话并自动重派
- **文件:行**：`src-ui/src/console-chat.ts:198-205`
- **问题**：
  ```ts
  try {
    const resumed = await dispatchConsoleTurn(project, text, target, resumeId, io);
    if (resumed.result.status !== 'error') return resumed;
  } catch {
    // 续聊 id 可能由外部 CLI 清理；清除后按同一目标重新发起一次全新会话。
  }
  onExpired();
  return dispatchConsoleTurn(project, text, target, '', io);
  ```
  1. `resumed.result.status === 'error'` 是执行过程中的普通业务错误（如 CLI 进程退出码非 0、模型生成失败、执行报错等），根本不是“会话不存在”。代码将所有 `status === 'error'` 均判定为会话失效。
  2. `catch` 块将瞬时网络抖动、超时等异常也全部当作失效处理。
  3. 误判后不仅通过 `onExpired()` 清空了用户的有效 `resumeId`、插入了具有误导性的「上次的会话已失效，已开新会话」提示，还自动使用空 `sessionId` 重新向模型发起了一次全新的请求，造成用户上下文丢失以及意外的二次 token/额度消耗。
- **复现**：
  在控制台中与正常会话继续交流，让 agent 执行一段抛错的指令，底层返回 `status: 'error'`。控制台立即清空当前 agent 的 `resumeId`，提示“上次的会话已失效，已开新会话”，并在后台自动发起了第二次请求。
- **改法**：
  - 精确识别会话失效错误特征（例如匹配错误文本中是否含有 `session not found`、`session expired`、`invalid session`、`cannot resume` 等特定标识），只有匹配时才触发回退。
  - 对于普通的 `result.status === 'error'` 以及非会话失效异常，直接返回或向外抛出结果，不得调用 `onExpired()`，不得抹去 `resumeId`，不得静默重派。

---

#### 2. 【主要】模式派发收尾前未兜底比对 `pipeline.results`，非 `onResults` 路径产出的失败节点存在漏消息风险
- **文件:行**：`src-ui/src/console-chat.ts:356-364` 与 `src-ui/src/components/center/ConsoleChat.tsx:158`
- **问题**：
  1. `ConsoleChat.tsx` 将原有的 `onOutcome` 清空为 `() => {}`，节点消息完全依赖流水线推进期间的 `onResults` 回调。
  2. 若流水线发生中断或异常，由 `onError` 生成并放入 `pipeline.results` 的失败节点若未经过 `onResults` 抛出，`dispatchConsolePreset` 在收尾时仅统计了成功/失败路数并输出收尾消息，未向消息流补发这些节点的回复。
  3. 用户将看到“模式派发结束：成功 0 路，失败 1 路”，但消息流中完全缺失该失败节点的原因说明。
- **复现**：
  阶段派发发生未捕获异常退出流水线时，`pipeline.results` 包含由 `onError` 填充的失败节点，但由于中途退出未触发该阶段的 `onResults`，消息流中没有该失败节点的状态与原因展示。
- **改法**：
  在生成收尾统计消息前，遍历 `pipeline.results`，对 `!announcedResults.has(result.nodeId)` 的节点统一调用 `feedback.append(consoleResultMessage(...))` 补齐回复后再发出收尾消息。

---

#### 3. 【次要】测试缺少“普通错误不回退会话”的负向用例，接线验证存在盲区
- **文件:行**：`src-ui/src/__tests__/console-feedback-persist-25.test.ts:182-198`
- **问题**：
  测试仅覆盖了 `throw new Error('session not found')` 时触发回退并置空重派的正向分支，完全没有覆盖返回普通错误（如 `result.status === 'error'`）时**不能**触发 `expired` 回调、**不能**置空 `sessionId` 的负向断言，导致问题 1 的错误实现漏网。
- **复现**：
  运行现有测试集全绿，但无法防护普通报错清空有效会话的回归。
- **改法**：
  在 `console-feedback-persist-25.test.ts` 中增加负向测试用例：当 `dispatchConsoleTurn` 返回普通业务失败（`status: 'error'`）时，断言 `expired` 未被调用，请求只发送一次且保留原 `resumeId`。

---

### 核过没问题的要点

1. **引擎未被篡改**：
   未修改 `src-ui/src/graph/dispatch.ts`，`startPipelineHeadless` 及其回调签名保持原样，未破坏引擎隔离边界。
2. **`onResults` 累积去重有效**：
   采用 `announcedStages` (Set<number>) 和 `announcedResults` (Set<string>) 维护通报记录，同一阶段只插一条系统消息，累积结果回调多次触发时同一节点只在首次出现时插入消息。
3. **节点状态呈现完备**：
   `consoleResultMessage` 结合 `resultStatusLabel` 对超时、额度用尽、真身不符、验证失败与常规失败均有状态与原因提取，备选节点（`fallbackTo` / `modelReported`）能正常呈现实际作答方与路由，不静默跳过。
4. **收尾消息与「处理中」解除顺序**：
   收尾消息在 `startPipelineHeadless` 结束后、函数返回前完成 append；控制台通过 promise 链路在后续 `finally` 阶段解除 `busy`，保证先出收尾消息再解除“处理中”。
5. **持久化与 200 条上限**：
   - 沿用了 `app-state` 的 reducer 与 `localStorage` 初始化链路，未引入多余状态容器。
   - 所有存储读写、序列化及反序列化均有 try-catch 兜底，存储异常（隐私模式、配额满、损坏 JSON）时不向外抛错。
   - 在追加、落盘和加载时均使用 `slice(-200)`，超出时截断丢弃最旧消息。
   - `busy` 等瞬时状态仅在组件内部通过 `useState` 管理，未持久化，重启不会出现卡在“处理中”的现象；关闭 tab 时能够同步清理对应的持久化记录。
6. **文案规范性**：
   阶段通报、收尾统计、状态提示及「上次的会话已失效，已开新会话」文案均严格遵循任务书与中文标点规范，通过了 `copy-audit.py` 检查。
