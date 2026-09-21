结论：`PASS`

本次改动完整实现了任务书 TASK-35 的各项契约，纯函数边界清晰，既有导出签名及持久化白名单保持向后兼容，未触碰模式派发与盲节点隔离逻辑，新增测试覆盖完整且具备有效证伪能力。

---

### 问题列表

#### 高严重度
无。

#### 中严重度
无。

#### 低严重度

1. [src-ui/src/console-chat.ts#L287](file:///src-ui/src/console-chat.ts#L287)
   - **问题**：在 [`dispatchConsoleTurnWithFallback`](file:///src-ui/src/console-chat.ts#L262-L288) 失效重试分支中，调用 [`dispatchConsoleTurn`](file:///src-ui/src/console-chat.ts#L176-L186) 时参数第 7 位（`freshText`）传入了 `undefined`：
     ```ts
     return dispatchConsoleTurn(project, freshText ?? text, target, '', io, `${runId}-fresh`, undefined, originalRequest);
     ```
   - **会导致什么**：虽然 [`prepareConsoleTurn`](file:///src-ui/src/console-chat.ts#L192-L225) 定义了默认参数 `freshText = text`，在收到 `undefined` 时会自动回退为入参 `text`（即 `freshText ?? text`），因此实际运行时 `prompt` 与 `currentMessage` 均正确收到了全量投影，逻辑未出错；但依靠下层函数的形参默认值级联回退较为隐晦，存在维护上的认知负担。
   - **建议改法**：显式透传 `freshText ?? text`：
     ```ts
     return dispatchConsoleTurn(project, freshText ?? text, target, '', io, `${runId}-fresh`, freshText ?? text, originalRequest);
     ```

---

### 重点核查结论

1. **正确性**
   - **空列表与无可补内容**：[`projectConsoleCatchup`](file:///src-ui/src/console-projection.ts#L83-L121) 在 `messages` 为空、无可补段落或 `outbound` 以 `/` 开头时，直接返回 `{ text: outbound, included: 0, omitted: 0 }`，[`consoleCatchupNotice`](file:///src-ui/src/console-projection.ts#L123-L127) 输出空串，不污染界面。
   - **截断与代理对切点**：[`clipBody`](file:///src-ui/src/console-projection.ts#L35-L43) 在头切点检测高代理项（`headEnd - 1`）并前移、在尾切点检测低代理项（`tailStart`）并后移，规避了拆散 UTF-16 代理对（如 Emoji）导致的序列化乱码；`tailStart - headEnd` 精确等于实际省略字数；刚好 4000 字符不触发截断。
   - **总长丢弃与一致性**：段落总长度超过 24000 时由最早段落开始移除，首行追加 `[更早的 ${omitted} 条对话已省略]`，`omitted` 与提示文案及 [`ConsoleCatchup.omitted`](file:///src-ui/src/console-projection.ts#L14) 完全一致。

2. **契约一致性**
   - [`dispatchConsoleTurn`](file:///src-ui/src/console-chat.ts#L176-L186) 与 [`dispatchConsoleTurnWithFallback`](file:///src-ui/src/console-chat.ts#L262-L288) 的新参数 `freshText` 与 `originalRequest` 均放在参数列表末尾并声明为可选，既有调用方在不传新参数时 `prompt`、`currentMessage`、`originalRequest` 均回退为 `text`，默认行为不变。
   - [`ConsoleMessage`](file:///src-ui/src/console-session.ts#L15-L33) 增加可选字段 `resumeKey?: string`，[`src-ui/src/console-persistence.ts#L56`](file:///src-ui/src/console-persistence.ts#L56) 白名单中已追加反序列化校验，空字符串不落盘。

3. **接线逻辑**
   - **三字段在三种路径下的传值**：
     - **首次派发**（`resumeId === ''`）：`currentMessage` 与 `prompt` 均为 `freshCatchup.text`，`originalRequest` 为用户原话 `outbound`。引擎执行 `is_continuing` 为 false 分支，取 `prompt` 发送。
     - **续聊派发**（`resumeId !== ''`）：`currentMessage` 为 `catchup.text`（仅增量），`prompt` 为 `freshCatchup.text`（全量备用），`originalRequest` 为 `outbound`。引擎执行 `is_continuing` 为 true 分支，取 `currentMessage` 发送；若 ACP 失败降级一次性 CLI，引擎取 `prompt`。
     - **失效重开**（`onExpired` 后 `${runId}-fresh`）：`currentMessage` 与 `prompt` 均为 `freshCatchup.text`，`originalRequest` 仍为 `outbound`。
   - **闭包与追加时序**：在 [`ConsoleChat.tsx`](file:///src-ui/src/components/center/ConsoleChat.tsx#L275-L285) 的 `send` 中，`projectConsoleCatchup(messages, ...)` 执行在 `append(consoleMessage('user', raw))` 之前，读取的 `messages` 绝对不含当前正在发送的这条消息；系统提示消息在用户消息之后、网络请求之前追加，时序正确。

4. **隔离性**
   - [`dispatchConsolePreset`](file:///src-ui/src/console-chat.ts) 未做任何修改；[`createConsoleApplyHandler`](file:///src-ui/src/console-chat.ts) 未引入 catchup 投影。
   - 盲节点的 `visibility == "盲"` 强约束未变，Rust 侧 `is_continuing` 明确排除盲节点，补上的增量上下文不会流入模式派发或 `.dispatch/` 盲审材料。

5. **测试有效性**
   - [`src-ui/src/__tests__/console-acp.test.ts`](file:///src-ui/src/__tests__/console-acp.test.ts#L120-L182) 与 [`src-ui/src/__tests__/console-projection.test.ts`](file:///src-ui/src/__tests__/console-projection.test.ts) 中的测试用例无自反/同义反复断言。
   - 若将 `freshText` 或 `originalRequest` 接线还原，或者移除 `resumeKey`、代理对避让、总长丢弃判断，对应测试格均会直接失败变红。

6. **外发内容防泄露**
   - [`segmentOf`](file:///src-ui/src/console-projection.ts#L68-L79) 仅接收 `kind === 'user'` 与满足 `isOkReply`（`status === undefined || status === 'ok'` 且正文非空）的 assistant 消息。
   - `kind === 'system'` 消息、`status` 为 `error`/`timeout`/`quota_exhausted` 等失败状态文本、以及存放在单独 state 里的权限请求卡片（`AcpPermissionRequest`），均无法通过过滤网进入外发文本。

---

### 存疑与顺手核对说明

1. **存疑：会话失效重开时的提示重复场景**
   - **位置**：[`src-ui/src/components/center/ConsoleChat.tsx#L312`](file:///src-ui/src/components/center/ConsoleChat.tsx#L312)
   - **现象**：若当前会话存在有效的 `resumeId`，但在当前会话的 `messages` 列表里目标从未发过言（例如较早的历史消息已被 `CONSOLE_MESSAGE_LIMIT` 截断清除），此时 `catchup.included` 本身就等于 `freshCatchup.included`。在首次发出前已提示过「已附带此前 N 条对话给 X」；会话失效走 `onExpired` 时，会再次输出一条完全相同的「已附带此前 N 条对话给 X」。
   - **评估**：该行为严格遵循了任务书「会话失效重开走了 freshText 时，按 freshText 的条数再报一次」的规定，非实现错误；若后续需要提升用户体验，可在 `freshCatchup.included === catchup.included` 且 `freshCatchup.omitted === catchup.omitted` 时省略二次提示。

2. **顺手核一件事：Kimi ACP 回退到一次性 CLI 在 Windows 下的截断风险**
   - **核查结论**：在 Windows 环境下，若进程调用外部一次性 CLI 时通过命令行参数（`argv`）传递 prompt（如 `kimi.exe -p "<prompt>"`），底层 `CreateProcess` 命令行解析机制会在首个换行符（`\r` 或 `\n`）处截断参数。本改动生成的 catchup 文本在首行即带有换行（前缀之后为 `\n\n`），一旦走该回退路径，模型将仅收到首行前缀提示而丢失后续所有正文与当前消息。确认该问题根因在引擎调度外部 CLI 的传参方式（应改走 stdin 或临时 task 文件），当前前端改动无需变更，结论按任务书要求记入报告即可。
