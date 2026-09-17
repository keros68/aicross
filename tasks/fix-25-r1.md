# fix-25-r1 控制台进度回流与持久化 · 复审修正轮

依据 GLM-5.3-Flash 与 Antigravity/Gemini 3.8 Flash 的盲审（`tasks/review-25-glm.md`、`tasks/review-25-gemini.md`）。Gemini 判**不可交付**，GLM 判可交付但有未闭合项。契约只追加。

## 必修

1. **续聊回退把一切失败都当成「会话失效」（两家共同；编排者已读代码核实）**
   `dispatchConsoleTurnWithFallback`（`console-chat.ts:189-206`）的回退条件是「抛异常 **或** `result.status === 'error'`」。而后端把网络断、供应商 5xx、CLI 崩溃统一记为 `status: "error"`，发派前的读资源失败、模型不在清单也会抛异常落进 `catch`。于是一次普通失败会：清掉**有效**的续聊 id、插一条与事实不符的「上次的会话已失效」、再用空会话把同一条 prompt **重派一次**——用户丢上下文、多花一次额度，真是网络问题时还会连着看到「已失效」和「失败」两条矛盾提示。
   修法：
   - **只对能确认是会话失效的信号回退**。在 `result.error`（以及抛出异常的消息）上匹配：同时出现「会话类词」（session / thread / conversation / resume）与「失效类词」（not found / no such / expired / invalid / unknown / does not exist / cannot resume / could not resume），大小写不敏感。把这组匹配做成一个带注释的纯函数，写明依据与局限。
   - **发派之前就抛出的异常**（读资源失败、模型不在清单等）一律原样抛出，不进回退。
   - 超时、额度用尽、真身不符、网络与供应商错误：原样返回给上层按普通失败显示，**不清 resumeId、不插失效提示、不重派**。
   - 测试必须补**负向**用例：普通 `status:'error'`（如「HTTP 502」）→ 只发一次请求、`expired` 未被调用、resumeId 保留；发派前抛出的异常 → 不回退；正向用例保留（`session not found` → 回退且只回退一次）。

2. **流水线中途停止的原因与 `onError` 产出的失败节点不回流（两家共同）**
   `startPipelineHeadless` 的 `stopped` 分支（修正轮撞总请求上限、重复批次、缺目标）不产生结果条目也不走 `onError`，错误串只放在返回值里；异常退出时由 `onError` 合成、放进 `pipeline.results` 的失败节点也可能没经过 `onResults`。而控制台侧 `onOutcome` 是空函数，收尾消息只报「成功 X 路，失败 Y 路」。用户看到「失败 1 路」却找不到原因——正是本期要消除的「界面无解释」的缩小版复发。
   修法：收尾前遍历 `pipeline.results`，对未播报过的节点补发结果消息；`pipeline.error` 非空时在收尾消息里写明停止原因。补测试：设小的总请求上限触发 `stopped`，断言消息流里有原因。

3. **未保护的 `await io.saveGraph(graph)` 会吞掉收尾消息（GLM 3）**
   `console-chat.ts:355` 先 `await saveGraph` 再发收尾；落盘失败时整个函数抛出，收尾不发。同函数 `commit()` 里对同一命令是 fire-and-forget 加 catch，口径不一致。改成与 `commit()` 一致，或 try/catch 后仍发收尾。

4. **修正轮重跑后，消息流显示首次状态、收尾按末次计数（GLM 4）**
   同一节点在修正轮里第二次出现时，播报只认首次（如「验证命令未通过」），收尾统计按末次（修好后成功），两者对不上。修法：节点末次状态与已播报的首次不同时，补一条更正消息（形如「验收 · 修正后通过」），收尾统计按末次。补测试。

## 保持（两家都核过没问题）

累积结果按节点去重、失败节点有状态说明、备选作答显示实际作答方、收尾在 `setBusy(false)` 之前、未改引擎、持久化沿用 `getInitialState`/`reducer` 链路、存储三处 try/catch、200 条上限丢最旧、「处理中」不落盘所以重启不会卡住——不要动。

## 验证

`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`；跑 `scripts/copy-audit.py`。第 1、2、4 条每条都要有能红的测试（第 1 条的负向用例是重点），走真实接线。每条先确认去掉实现会红，在报告里写清怎么红的。

保持文件原有换行；不 `cargo fmt`；不起窗口、不发真机请求、不调真实模型；不 commit、不 push。报告写 `tasks/REPORT-25-R1.md`，逐条对应。
