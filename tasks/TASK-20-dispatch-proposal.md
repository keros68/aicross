# TASK-20 派发建议条：建议 + 一键裁决（09-16 定案）

依据 `docs/PRODUCT.md`「派发的触发方式（09-16 定案，用户提出）」。目标：日常在总指挥里正常聊天；该派发时由总指挥或用户触发一条**建议**，回车才真发。不自动执行，不要求先选模式。预设下拉的组头已在 `ui/phase20` 去掉，本期不重复。

## 契约（只追加）

- 控制台通道新增 `POST /v1/propose`：请求体与 `/v1/dispatch` 相同，额外可带 `reason`（总指挥为什么建议派，一句话）。语义：只登记一条建议，**不派发**；返回 `{ proposal_id, accepted: true }`。建议同样进 run 表但 `status: "proposed"`，不计入在飞上限。
- `GET /v1/runs/<id>` 的 `status` 增 `proposed`；用户采纳后转 `running`，忽略后转 `dismissed`。
- CLI：`aicross propose --project <dir> [--preset <名>] [--reason <一句话>] --text-file <path>`；退出码与 `dispatch` 同口径。`aicross dispatch` 行为不变（直接派，给脚本用）。
- 前端 `ConsoleProposal`：`{ proposal_id, project, graph_id, preset_id, node_count, reason, created_at }`。

## 实现

1. **建议条**（`src-ui/src/components/center/ProposalBar.tsx`）：出现在当前 tab 顶部（总指挥 tab 与对话图 tab 都出），一行：`建议派发 · <模式名> · N 路 · 回车采纳 · Esc 忽略 · Tab 换模式`，带 `reason` 时在后面接一句浅色说明。回车 → 走 TASK-16 的 `startPipelineHeadless` 真派；Esc → `dismissed`；Tab → 在 基础三模式 + 自定义 + 更多 里循环（复用 `presetGroupOf` 的顺序），选中即改本次建议的预设。建议条不抢输入焦点：快捷键只在没有输入法组字、且焦点不在多行输入里时生效；总指挥 tab 里 PTY 有焦点时改用 `Ctrl+Enter` 采纳（终端要吃回车），文案随之显示。
2. **用户主动触发**：控制台输入框旁加「派发」按钮（2–6 字动词短语），点了就生成一条本地建议（不经 HTTP，直接走同一条建议条），预设取默认经济模式。
3. **总指挥触发**：`aicross propose`（第 1 条契约）。`docs/CONSOLE-CHANNEL.md` 补这条接口与 `/派发` 的用法说明，供 skill 侧 `channels.md` 引用。
4. **要不要建议**：`isSmallEconomyTask(text, attachments)` 命中 → 不出建议条（CLI 侧返回 `{ proposal_id: "", skipped: "单步小任务" }`，不登记）。设置页「派发」栏加一个开关「用归类模型二次判断」，默认关；开时用 `classificationModel` 跑一次分类，两者都认为该派才出建议条。
5. **测试**：`/v1/propose` 登记后不派发（假前端回执 + 断言 `dispatch_graph_stage` 未被调用）；`proposed → running / dismissed` 状态机；小任务不登记；建议条纯函数（模式循环顺序、快捷键在 PTY 焦点下的变体、reason 渲染）；「派发」按钮生成本地建议；CLI `propose` 的退出码与参数校验（多行文本仍只走 `--text-file`）。

## 不做

- 不做「模型自动执行不问」。不做后台静默派发。不在聊天每条消息上跑分类模型（默认关，用户显式打开才跑）。

验证：`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。保持文件原有换行（`src/server.rs`、`src/main.rs` CRLF；`src-ui/src/tauri.ts` 混合只做字节级最小改动）；不 `cargo fmt`；不起窗口、不发真机请求；不 commit、不 push。报告写 `tasks/REPORT-20.md`。
