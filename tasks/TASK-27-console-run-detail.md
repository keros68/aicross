# TASK-27 控制台任务详情页 + 收尾给最终结论（09-17）

用户 09-17 原话：「我们需要一个满版展示任务进度或者说任务详情，就是用了啥模型、消耗了多少 token、每个 agent 干了多长时间」。
对应 backlog 78、80。**只做下面两件事。**

**另有一期在飞（TASK-28，引擎给盲审补答案文件），这些文件一律不要碰**：`src/**`（全部 Rust）、`src-ui/src/tauri.ts`、`src-ui/src/graph/context.ts`、`src-ui/src/graph/types.ts`。`src-ui/src/graph/dispatch.ts` 只读不改；要新的纯函数就写在控制台自己的模块里。

## 现状（编排者核过）

- 每一路结果 `DispatchNodeResult`（`src-ui/src/tauri.ts:519`）已带 `modelReported`、`tokensIn`、`tokensOut`、`cacheRead`、`durationMs`、`status`、`stage`、`fallbackFrom/To`。
- 在跑的节点有 `dispatch-progress` 心跳（`tauri.ts:623`，每 15s）：`elapsedMs`、`idleMs`、`toolCalls`、`lastTool`。
- 旧画布结果屏 `DispatchOverlay.tsx:568` 起有按阶段的用量表与小计，控制台没接。
- TASK-25 已让控制台逐阶段、逐路插消息，收尾一条「模式派发结束：成功 N 路，失败 M 路，总耗时 X 秒」，消息落 `console-persistence.ts`。
- 验收结论解析已有 `parseAcceptance`（`graph/dispatch.ts:404`，PASS / NEEDS_FIX / UNKNOWN）。

## 一、任务详情页（backlog 80）

1. **入口**：控制台里一次模式派发对应一份「运行记录」。派发进行中，「处理中…」旁给「查看进度」；收尾消息上给「查看详情」。点开后**占满中间区域**（盖住消息流，不是小弹窗），有明确的「返回对话」，Esc 也能返回（注意 `lib/focus-enforcer.ts` 的焦点守卫，参照设置弹窗的做法）。
2. **内容**，按阶段分组，每一路一行：
   - 角色（出方案 / 执行 / 验收 / 验证 …）、agent、**请求的模型**、**实际回答的模型**（`modelReported`；为空或 CLI 不报真身时如实写，比如「kimi 不报真身」，沿用现有文案口径）、状态（成功 / 失败 / 超时 / 进行中）、验收类节点的结论（通过 / 需修正 / 未知）、输入 token、输出 token、缓存命中、耗时。
   - 切过备选的行标出「主 → 备」。
   - **token 未报告要显式写「未报告」**：输入与输出都为 0 的已完成行视为未报告（真实调用不可能输入为 0），不要显示成 0，也不要计入合计；合计行注明「N 路未报告，未计入」。
   - 进行中的行：实时计时（按心跳 `elapsedMs`，两次心跳之间前端自己走表）、最近一次工具调用名与调用次数（有才显示）。
   - 每阶段小计、全局合计（路数、token、总耗时=墙钟时间，不是各路耗时相加；两者都列，写清口径）。
3. **持久化**：运行记录跟随控制台会话落盘（沿用 `console-persistence.ts`，同一 localStorage 键下加字段并兼容旧数据），每个会话保留最近 20 份，重启后「查看详情」仍可打开；进行中的记录重启后标为「已中断」，不要显示成还在跑。
4. 文案遵守 `docs/PRODUCT.md` 与现有界面口径：中性、简短；不出现美元、额度百分比（产品边界：只展示用了哪些模型、多少 token、多长时间）。

## 二、收尾给最终结论（backlog 78）

实测：两轮验收都判 NEEDS_FIX，收尾仍写「成功 7 路，失败 0 路」，读起来像通过了。
改为收尾第一句是**结论**，第二句才是路数统计，例如：
- 「结论：通过（第 2 轮验收通过）。共 7 路，失败 0 路，用时 357.7 秒。」
- 「结论：未通过（修正 1 轮后验收仍需修正，已到上限）。共 7 路 …」
- 「结论：中途停止（原因：…）。…」
- 没有验收节点的模式：「结论：已完成（本模式不含验收）。…」
结论取**最后一轮**验收类节点的 `parseAcceptance`；多个验收类节点任一 NEEDS_FIX 即未通过；UNKNOWN 写「验收结论无法识别」而不是通过。「成功 / 失败」的措辞改成「运行失败 N 路」，不要让人误读成验收结果。

## 不做

- 不改引擎、不改派发流程与修正轮逻辑、不动旧画布 `DispatchOverlay` 的行为（可以抽共享的纯展示函数，但旧屏输出不变，有测试钉住）。
- 不做成本折算、不做图表。

## 验证

`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`、`python scripts/copy-audit.py`（不得新增命中）。

- **本仓连吃四次「只测纯函数漏接线」**：详情页至少一格测试走真实接线——从 `dispatchConsolePreset` / apply handler 驱动一次假的两阶段派发（含一路 token 为 0、一路切备选、一路 NEEDS_FIX），断言运行记录内容、收尾结论文案、以及持久化后再加载仍能取到记录。
- 未报告 token 不计入合计、墙钟 vs 各路耗时和、重启后进行中标「已中断」、旧数据无运行记录字段时正常加载，各一格。
- 每条先确认去掉实现会红，在报告里写清怎么红的。

保持文件原有换行（`CenterPanel.tsx`、`docs/PRODUCT.md` CRLF）；不 `cargo fmt`；不起窗口、不发真机请求、不调真实模型；不 commit、不 push。报告写 `tasks/REPORT-27.md`。backlog 不用改（与 TASK-28 同文件相邻行，编排者合并时统一划）。
