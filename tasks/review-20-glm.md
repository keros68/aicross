# TASK-20 审查报告

**总判：可交付但有未闭合项。**「只登记、不派发」这一核心契约在 Rust 与前端两侧都成立；但键盘三条键里只闭合了回车（Esc/Tab 在 PTY 聚焦时仍被建议条抢走），另有单行输入框回车误采纳、超时后幽灵建议条等问题。

## 问题清单（按严重度）

1. **【高】PTY 聚焦时 Esc / Tab 仍被建议条抢走，三键只有回车做了变体** — `src-ui/src/components/center/ProposalBar.tsx:50-56`
   `proposalShortcutAction` 里 `Escape → dismiss`、`Tab → cycle` 的判定在 `commander && ptyFocused` 分支（54 行，只管 Enter）**之前**，且 handler 挂在 document 捕获阶段（137 行）并 `preventDefault + stopPropagation`。复现：总指挥 tab 有待裁决建议 → 点进 codex 终端 → 按 Esc：建议被（ unknowingly）忽略，codex 收不到「编辑上一条」；按 Tab：模式循环，codex 收不到补全。建议条可见期间终端完全失去这两个键，且此时文案仍显示「Esc 忽略 · Tab 换模式」，引导用户在终端场景去按。改法：`commander && ptyFocused` 时对 Escape/Tab 一并返回 `null`（还给终端），并把这两种状态下文案里的 Esc/Tab 提示去掉或改为带修饰键的变体；测试 `src-ui/src/__tests__/proposal-20.test.ts:19-27` 只断言了 Enter 的 PTY 变体，Escape/Tab 的 PTY 用例缺失（23-24 行传的是 `ptyFocused: false`）。

2. **【高】单行输入框里回车会误采纳建议 → 真实派发** — `src-ui/src/components/center/ProposalBar.tsx:87-90` 与 `src-ui/src/components/center/graph/GraphTab.tsx:799-807`
   `isMultilineTarget` 只认 `TEXTAREA` 和 `contentEditable`；对话图每列的输入框是单行 `<input class="graph-input">` 且回车发送。复现：对话图 tab 点「派发」出建议条 → 焦点进列输入框继续打字 → 按回车想发消息 → `proposalShortcutAction` 返回 `accept`，`stopPropagation` 使 React 的 `onKeyDown`（`GraphTab.tsx:804-806`）根本收不到事件，用户消息没发出去，反而按建议文本真起了流水线（花额度、可能写文件）。改法：把单行文本 `input` 也纳入排除（或 Enter 采纳仅在焦点不在任何文本输入时生效）。

3. **【中】建议条捕获 Esc，弹窗的 Esc 关闭被吞** — `src-ui/src/components/center/ProposalBar.tsx:124-125` vs `src-ui/src/components/common/SettingsModal.tsx:175-177`
   SettingsModal 的 Esc 监听挂在 document 冒泡阶段；建议条在 document 捕获阶段 `stopPropagation`，直接短路它。复现：建议条可见 → 打开设置 → 按 Esc：设置不关，建议被顺手忽略（b20edcc 修的「Esc 能关设置」被本功能回归）。确认派发页（DispatchOverlay）等其它 document 级 Esc 处理同理。改法：bar 的 handler 在存在打开的弹层（dialog/overlay）时直接返回。

4. **【中】采纳在飞上限拒绝时静默失败，且该新分支无测试** — `src/console_api.rs:394-396`、`src-ui/src/console-channel.ts:236`、`src-ui/src/components/center/ProposalBar.tsx:134`
   采纳时前端先 `consoleRunUpdate({status:'running'})`；已有 3 个通道 run 在飞时 Rust 返回「同时在飞 run 已达上限 3」，`await` 抛出 → 建议条原地不动、无任何用户可见提示；`void operation.finally(...)` 使 rejection 无人接。另外 `console_api.rs:394` 这条 proposed→running 的上限校验没有测试覆盖：`proposed_run_transitions_only_to_running_or_dismissed`（`console_api.rs:740-743`）第三段只是重插 3 个 running 再数一遍，恒真，没对 proposed run 调 `apply_run_update`。改法：catch 后在条上短暂显示原因；补「3 个 running + proposed 采纳 → Err」的测试。

5. **【中】前端处理超过 25 秒时产生不可关闭的幽灵建议条** — `src-ui/src/console-channel.ts:222-223`、`src/console_api.rs:22,316-321`
   `runConsoleProposal` 先 `publishProposals` 再 `await consoleRunUpdate('proposed')`。开「二次判断」且归类模型慢于 `PROPOSAL_TIMEOUT`（25s；分类阶段的 timeout 用的是 600s 档）时：Rust 已 503（CLI 退出码 1）、pending 已删、run 未登记；前端随后仍把条画出来，`consoleRunUpdate` 报「找不到 run」→ 采纳（236 行）与忽略（255 行）的 await 都抛、`publishProposals(filter)` 永远不执行——条无法采纳也无法忽略，Esc 每次重复抛错。改法：先 `await` 回执成功再 `publishProposals`；accept/dismiss 里对更新失败做 catch 并本地移除条。

6. **【中低】webview 重载后 proposed 行成永久孤儿；HTTP 无法忽略建议** — `src/console_api.rs:169,298-322`
   run 表在进程内存，进程重启会清空（无跨重启残留）；但 webview 重载（同进程）后 `proposalItems` 丢失且无任何从 `list_console_runs` 重建建议条的路径：行永远停在 `proposed`，`GET /v1/runs/<id>` 永远报 proposed，采纳不可能（`ConsoleRun` 不存 text），`POST .../cancel` 对 proposed 走 `should_relay_cancel == false` 返回 200 但什么都不做。同项目多条建议时 `proposalForTab`（`console-channel.ts:143-147`）只 `find` 最新一条，旧条同样变孤儿。改法：启动或 `list_console_runs` 时把遗留 `proposed` 翻成 `dismissed`（或剔除）；让 cancel 对 proposed 执行 dismissed。

7. **【低】本地（按钮）派发绕开通道在飞上限、不进 run 表** — `src-ui/src/console-channel.ts:244`
   `localIo` 把 `listConsoleRuns` stub 成 `[]`、`consoleRunUpdate` 成 no-op：按钮派发既不受前端 `consoleRunLimit` 约束（272 行恒过），Rust 表也看不到（`aicross status/cancel` 不可及）。单次 `maxRequests` 上限（307 行）仍生效，故仅为并发 run 数的旁路。若是有意为之（按钮=普通 UI 派发）建议在报告写明；否则本地也计数。

8. **【低】并发建议发布互相覆盖** — `src-ui/src/console-channel.ts:222`
   两个建议的处理同时 in-flight（窗口含 `listPresets/loadRoles` 乃至归类模型调用）时，后发布的整表替换会把先发布的从条上挤掉，其 run 行留在 Rust 表里变孤儿（同问题 6 的归宿）。改法：`publishProposals` 按 id 合并而非整表替换。

9. **【低】文档/契约小出入**
   - `src/console_api.rs:40`：`reason` 在共享的 `ConsoleDispatchRequest` 上，`/v1/dispatch` 也照收（前端忽略），而 `docs/CONSOLE-CHANNEL.md` 只在 propose 下写 reason。
   - `docs/PRODUCT.md:287` 写入口 1 是 `aicross dispatch --propose`，实现按任务书为 `aicross propose`（`tasks/TASK-20-dispatch-proposal.md:9`）；PRODUCT.md 未同步。
   - 「采纳」现在同时指节点结果采纳（MANUAL 术语表既有行）与建议采纳（新增行），轻度重载；PRODUCT.md 本身这样用，算按规范，仅提示。

## 核过没问题的要点

- **不自动执行**：`relay_proposal`（`console_api.rs:298-322`）不写 runs 表，仅在前端回执 `proposed` 后经 `insert_run` 登记（无在飞计数）；emit 失败/超时只 503、不登记；迟到回执落「找不到 run」；进程重启内存表清空。全仓 `startPipelineHeadless` 仅 `runConsoleDispatch`（采纳/通道 dispatch）与 DispatchOverlay 调用，建议链路不经过；前端测试用「会抛『不应派发』的 dispatchGraphStage」钉住（`proposal-20.test.ts:56-71`），非同义反复。
- **proposed 不计在飞**：Rust `insert_inflight_run` 仅用于 dispatch（349 行只数 `running`）；前端 `consoleInflightCount` 只数 `running`。
- **状态机**：pending 阶段 `skipped`（不登记，返回 `{proposal_id:"",skipped}`）/`rejected`/`proposed`；注册后仅 `proposed → running|dismissed`，`dismissed` 终态（390-393 行，测试曾红灯验证）；旧状态词 `running/done/failed/rejected` 含义未动，`docs/CONSOLE-CHANNEL.md:117` 已同步口径。
- **薄中继**：Rust 侧无提示词拼装、无图写入、无模型调用；归类调用在前端复用既有 `dispatchGraphStage`（`writeProjectDispatch:false`）；`proposal` 类事件不触发开图/抢 tab（`CenterPanel.tsx:516`）。
- **判据同源**：`isSmallEconomyTask` 唯一实现在前端（`dispatch.ts:412-414`，计附件、<120 字、排除词），界面按钮（含图片附件 `Gambit.tsx:918`）与 CLI 建议同走 `proposalDecision`，不存在「界面判派、CLI 判小」分叉；「用归类模型二次判断」默认关（`dispatch-settings.ts:17,54`），归类模型挂掉 → 建议 rejected（409），不会自动派。
- **CLI**：`propose`/`dispatch` 共用 `build_request`/`run_at`，退出码 0/1/2/3 同口径；`--text`/`--text=` 两者均拒、多行仅 `--text-file`、BOM/UTF-8 校验、reason 单行 CLI+Rust 双侧拒；`dispatch` 无 reason、路径与正文不变、既有 dispatch 测试全部保留。
- **键盘其余项**：监听在捕获阶段；`isComposing` 与多行 textarea（妙手输入区是真实 textarea）排除有效；PTY 聚焦时纯回车还给终端、`Ctrl+Enter` 采纳且文案同步；隐藏标签页经 `offsetParent===null` 守卫不响应（与报告一致）。
- **文案**：内置预设名即「共享模式/经济模式/盲审模式」（`orchestration.rs:371,385,434`），按钮「派发」两字动词，「采纳/忽略/换模式」动词短语，MANUAL 术语表新增两行，无生造词。