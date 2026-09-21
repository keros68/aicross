# 审查结论：PASS

无高、中级问题。锚点、过滤、截断、总长丢弃、拼装逐项核对无边界错误；三条派发路径接线正确；模式派发与盲节点无泄漏路径；新增测试逐格核对均钉住了对应实现。以下为低级问题与存疑项。

## 低

1. **`ConsoleChat.tsx:286-291` — 派发失败时留下不成立的通知。** 「已附带此前 N 条对话给 X」在 `dispatchConsoleTurnWithFallback` 之前追加；若 `prepareConsoleTurn` 抛错（如目标已不在可派发清单，`console-chat.ts:205`），消息流里保留用户消息、这句通知和错误提示，但什么都没发出去。会导出致：转写记录与事实不符。建议改法：通知挪到首次派发成功后再补，或失败路径上不追责（最低限度在 REPORT 里记为已知）。现有模式里用户消息同样滞留，故定低。

2. **`ConsoleChat.tsx:327-329` + `console-projection.ts:63` — 引擎内部 ACP 重置那轮的 stamp 与滞留 resumeId 组合出一个受限的欠包含窗口。** 引擎侧 ACP 失败回退 oneshot 的结果（`dispatch.rs:3985-3986`）是 status ok、`fallbackTo` 空、`sessionId` 空：`consoleReplyResumeKey` 会打上 resumeKey（锚点成立），而 `turn.result.sessionId` 为空导致旧 resumeId 原样保留。若下一轮 `isExpiredResumeError` 漏判（`console-chat.ts:242-247` 注释自认可能漏判），目标只收到锚点之后的内容。后果受限：引擎对死会话会走 `reset_acp_fallback(&prompt)` 用完整 prompt 重发（`dispatch.rs:3933`），且漏判时该轮本就报错可见。建议改法：`result.sessionId` 为空时不打 stamp（一行守卫），让锚点与实际会话严格对应。

3. **`console-projection.ts:41` — 「省略 N 字」按 UTF-16 码元计数。** emoji 一对计 2 字（测试把 1004 钉为预期，属有意取舍）。含大量增补平面字符的正文，提示的省略字数会大于读者直觉的字数。可接受；若要改，按码点数计数并同步改测试。

4. **`console-projection.ts:111` — 不可达的防御分支语义不自洽。** `kept.length === 0` 时返回 `{ text: outbound, included: 0, omitted: n }`：现有常量下单段最长约 3.8k < 24000，只要有段就至少保留一条，此分支只在 `segments` 为空（omitted = 0）时到达。但若将来常量调整使其可达，会出现 `omitted > 0` 而 `text` 原样返回、界面无任何「已省略」提示的组合。建议加注释说明不可达依据，或改为断言。

5. **`console-chat.ts:159` + `console-projection.ts:76` — 合成文案会被当真实回复外发。** status ok 但 `result.text` 为空的成功回复在消息流里是「已完成。」，后续补对话会把它作为 `[模型回复 X]\n已完成。` 发给别家。发生率低（ACP/oneshot 的 ok 结果都有正文），但属「不该带出去的合成内容」一类。建议：空正文成功回复不打 resumeKey 或不收段。

6. **`console-persistence.ts:71`（`CONSOLE_MESSAGE_LIMIT = 200` 截断）** — 锚点消息滑出 200 条窗口后 resumeKey 丢失，该目标下一轮整段重发（内容重复，不少发）。行为内含于既有截断机制，量级小，记录即可。

## 核对通过项（要点）

- **边界**：空列表、锚点后无新对话、恰好 4000 不截、总长恰好不超、切点避开代理对（头尾两处 + 计数 `tailStart - headEnd` 与测试 1004 一致）、`omitted` 与首行提示同源变量不会不一致、`/` 开头在一切之前短路返回——均正确。
- **契约**：两个导出函数新参数均在末尾且可选，默认值（`freshText = text`、`originalRequest = text`）完整复现旧行为；既有调用方（`console-chat-22`、`console-feedback-persist-25`、旧 ACP 测试，均 ≤ 7 参）不受影响；`messageFrom` 白名单新增 resumeKey 且空串不落盘，写侧只有真值才展开该字段。
- **接线**：首次（resumeId 空）prompt = currentMessage = fresh 投影；续聊 prompt = fresh、currentMessage = catchup、originalRequest = 原话——与引擎 `is_continuing`（`dispatch.rs:1776-1778`，console 单线恒为「共享」）取 `current_message`、否则取 `prompt` 的分支吻合；前端检测的失效重开三项都换用 fresh。`send` 里投影在 `append(user)` 之前计算，闭包里的 `messages` 不含刚输入的这条；`busyRef` 同步置位挡住重入。`onExpired` 的二次提示按 freshText 条数、与首次条数不同源，无重复计数路径。
- **隔离**：投影只在 `ConsoleChat.send → dispatchConsoleTurnWithFallback` 一条路上；`dispatchConsolePreset` 用原文，未触碰。单线路 visibility 恒「共享」、materials/upstreamArtifacts 恒空，盲审目录（REQUEST.md 等）对这轮不写入，`.dispatch/` 留痕内容即实际外发全文。
- **外发内容**：收的是剥 @ 后的用户消息、ok/旧格式回复正文、前缀模板、当前消息；system 消息、失败/超时/额度/真身不符/验证失败正文（`status !== 'ok'` 一律排除，status 是宽 string，新状态默认被排除，方向安全）、权限卡片（非消息）、运行记录都不进。
- **测试会红**：逐格静态推演——锚点扫描删掉则「只取之后/没说过取全部」红；`isOkReply` 放宽则 system/失败格红；`clipBody` 朴素切片则 emoji 格红（孤立代理对正则 + 1004 计数双钉）；总长循环 off-by-one 则 6/3 格红；白名单删行则持久化格红；`prepareConsoleTurn` 把 prompt 退回 text 则 acp 新格一 `sent[0].prompt` 红；fallback 退回则 freshText 被忽略、同一断言红。无同义反复；`console-projection.test.ts:45` 的 `startsWith` 在全文相等断言后属冗余但无害。

## 存疑

1. **工作区与补丁快照有漂移**：`console-chat.ts:287-288` 现为 `const fresh = freshText ?? text; … fresh`，快照里是内联 `freshText ?? text, …, undefined`。功能等价，本审查以当前工作区为准；快照之后工作区仍在变动，建议冻结后再交付。
2. **任务书与实现锚点口径不一致**：任务书规则 1 写三项全等，实现只认 resumeKey（本审查契约同实现）。实现更稳——引擎报回的真实 model（`deepseek/deepseek-flash` 形状）会破坏三项全等，导致锚点永远找不到、每轮整段重发；但这条口径差异应写进 REPORT，避免后人按任务书「修回去」。
3. **未获运行证据**：本会话只有只读工具，vitest / tsc / build 未实际执行；「会红」的结论全部来自静态推演。任务书验证三条命令需主线程补跑后再写 REPORT。
4. **顺手核（argv 问题）**：ACP 回退一次性 CLI 走 `reset_acp_fallback(&prompt)`，从 `dispatch.rs:7272-7273` 的断言看 oneshot 用 `--task-file` 传全文，不经 argv，多行不会被换行截断。这是对测试替身调用形状的断言，真实 CLI 命令行组装未逐条核对。
5. **历史 @ 剥除依赖当前可用模型**：`segmentOf` 用当下 `mentions` 剥 @，已被移除的 agent 的历史消息不剥、原样外发（仅多几个字，无害）；若希望剥除口径只看历史，需要另存快照，现阶段不值得。