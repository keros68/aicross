# TASK-25 R1 复审 · Antigravity / gemini-3.8-flash-high（盲审，材料内联）

### 总判
**可交付但有未闭合项**（四项核心必修的功能骨架与保护机制均已闭合且测试真实可红，但编排者修改的 `isExpiredResumeError` 匹配器对清单规定的词库存在明显漏判与边缘误判）。

---

### 问题清单

#### 1. 中：`isExpiredResumeError` 漏判失效词在后的句式及 `resume id` 句式
- **文件与位置**：`src-ui/src/console-chat.ts:214-222`
- **问题**：
  1. Pattern 1（`[会话词] ... [失效词]`）的失效词集合仅包含 `not found|does not exist|no longer exists|has expired|expired`，**遗漏了清单要求的 `invalid` 与 `unknown`**。而 Pattern 2 仅支持失效词在前的严格相邻匹配（`invalid\s+session`）。导致自然英文中高频出现的 `"Session 'xxx' is invalid"`、`"Session id is unknown"`、`"Conversation is invalid"` 无法匹配。
  2. Pattern 1 与 Pattern 2 的会话词集合仅为 `(?:session|thread|conversation)`，**遗漏了清单要求的 `resume`**（唯一的 `resume` 仅出现在 Pattern 3 动词短语）。当 CLI 或服务以名词短语报错时（如 `"resume id not found"`、`"invalid resume id"`、`"unknown resume id"`），Pattern 1/2/3 均无法匹配。
  3. Pattern 2 使用严格 `\s+`，中间夹带定语（如 `"no active session"`、`"invalid target conversation"`）也会漏判。
- **复现**：
  在单元测试中调用 `isExpiredResumeError("Error: Session abc is invalid")` 或 `isExpiredResumeError("resume id not found")`，实际均返回 `false`（预期为 `true`）。
- **改法**：
  在 Pattern 1 中补充 `invalid|unknown`；在 Pattern 1 与 Pattern 2 的主词中补充 `resume(?:\s+id)?`；Pattern 2 放宽连接词允许 1-2 个修饰词：
  ```ts
  const EXPIRED_RESUME_PATTERNS: RegExp[] = [
    /\b(?:session|thread|conversation|resume(?:\s+id)?)\b[^.\n]{0,40}?\b(?:not found|does not exist|no longer exists|has expired|expired|invalid|unknown)\b/i,
    /\b(?:no|unknown|invalid|expired|no such)(?:\s+\w+){0,2}?\s+(?:session|thread|conversation|resume(?:\s+id)?)\b/i,
    /\b(?:cannot|can't|could not|couldn't|failed to|unable to)\s+resume\b/i,
  ];
  ```

#### 2. 低：`isExpiredResumeError` 对单行非会话线程异常存在边缘误判
- **文件与位置**：`src-ui/src/console-chat.ts:215`
- **问题**：
  Pattern 1 采用 `\bthread\b[^.\n]{0,40}?\bnot found\b`。虽然排除了跨行和点号，但单行中若出现系统/线程池报错且包含资源未找到（如单行无点号日志 `"Worker thread error: task not found"` 或 `"thread pool: connection not found"`），仍会跨越 40 字符内的非句号文本命中该规则，误判为会话失效。
- **复现**：
  在单元测试中调用 `isExpiredResumeError("Worker thread error: task not found")`，实际返回 `true`（非会话失效被判为失效）。
- **改法**：
  收紧 `thread` 的跨度（如缩短至 20 字符内），或排除常见系统线程词缀（如 `pool`、`worker`、`main`），避免泛化匹配底层线程错误。

---

### 核过没问题的要点

1. **必修 1（续聊回退条件收敛）闭合**：
   - 资源读取、目标校验等前置逻辑已独立在 `try` 块外，发派前抛错原样向上抛出，不再误进 `catch`。
   - 普通 `status: 'error'`（如 HTTP 502）原样返回、不清除有效 `resumeId`、不向用户提示「上次的会话已失效」、不重复扣费重派。
   - 包含正向回退一次与负向（发派前异常、HTTP 502）真实接线测试，能够有效防御旧逻辑。
2. **必修 2（流水线中途停止与错误节点回流）闭合**：
   - 收尾前显式对 `pipeline.results` 的节点末次结果调用 `announceResults`，补齐了未播报的失败节点（含 `onError` 生成的节点）。
   - `pipeline.error` 非空时，收尾消息正确追加 `停止原因：${pipeline.error}`。
   - 对应测试使用真实 `dispatchConsolePreset` 模拟达到总请求上限停止，断言收尾文案包含停止原因。
3. **必修 3（落盘失败保护）闭合**：
   - 收尾前的 `saveGraph` 已改为异步执行并增加 `.catch(...)` 容错，落盘失败仅记录警告，不阻断结果返回与收尾消息。
   - 对应测试注入 `throw new Error('disk full')`，断言仍然能够成功返回结果并产出收尾统计。
4. **必修 4（修正轮状态更新与末次统计一致）闭合**：
   - `announcedResults` 记录每个节点的最近播报状态；节点状态变化时（如从失败修正为通过，或从成功修正为超时），准确补发一条带 `角色 · 修正后通过/超时/失败` 的更正消息。
   - 收尾统计按每个节点的末次结果去重计数，消息流与收尾计数完全一致。
5. **保持项完好**：
   - `storageOrDefault`、`persistConsoleSessions` 与 `loadConsoleSessions` 均有 try/catch 兜底，存储异常不阻断内存运行。
   - Reducer 与加载链路均强制截断最近 200 条消息；`busy` 状态未持久化，重启不会卡死。
   - 换行风格保持原状，无全局格式化污染，无真机调用。
