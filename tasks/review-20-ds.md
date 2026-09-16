I'll start by examining the repository state and the task documents.

I'll start by examining the repository state and the task documents.

## 总判

**不可交付**：硬约束「不自动执行」在结构上守住了（`/v1/propose` 只登记、Rust 只中继），但建议条在**总指挥 tab 无法裁决**（三个键全被 `targetMultiline` 早退吃掉，文案却写 Ctrl+Enter），且**对话图单行输入框的回车会把「发消息」静默升级成一次真派发**。这两条必须在验收前闭合。

---

## 按严重度排序

### 1. 总指挥 tab 里建议条完全无法裁决；文案写的 Ctrl+Enter 不生效
- 位置：`src-ui/src/components/center/ProposalBar.tsx:50`（早退）、`:87-90`（`isMultilineTarget` 判 `tagName === 'TEXTAREA'`）、`:121-122`（`targetMultiline` 与 `ptyFocused` 取自同一个 `event.target`）、`:142/144`（文案）、`:144-147`（整条 `div` 无任何点击处理）
- 问题：xterm 的 helper 就是 `<textarea class="xterm-helper-textarea">`（`TierTerminal.tsx:600/625/1845` 同款写法可证）。PTY 有焦点时 `isPtyTarget=true` **同时** `isMultilineTarget=true`，`proposalShortcutAction` 第一行就 `return null`，所以 Enter / Esc / Tab 三个键一个都不归建议条，全部原样进 codex。`ptyFocused` 为真时文案显示「Ctrl+Enter 采纳」，而这个组合恰恰是永远不生效的那一格。建议条又没有可点区域（无 `onClick`、无按钮），于是总指挥 tab 里既不能采纳也不能忽略——`aicross propose` 这个主入口的产物在主入口所在 tab 里是死的。
- 复现：总指挥 tab（codex 会话，焦点在终端）→ 让总指挥执行 `aicross propose --project <cwd> --reason "…" --text-file …`（或直接 `POST /v1/propose`）→ 顶部出现「建议派发 · 经济模式 · N 路 · **Ctrl+Enter 采纳** · Esc 忽略 · Tab 换模式」→ 按 Ctrl+Enter：建议条无变化，按键进了终端；Esc / Tab 同样进终端（codex 的「编辑上一条」/ 补全）。此时无任何鼠标路径可用。
- 建议改法：`targetMultiline: isMultilineTarget(event.target) && !isPtyTarget(event.target)`，让 PTY 走 `state.commander && state.ptyFocused` 分支；但立刻会付代价——在活的 codex 会话里吞掉 Esc（打断生成）和 Tab（补全）是真实损失，需要产品裁决：推荐「PTY 焦点下只认 Ctrl+Enter，Esc/Tab 放行」，同时给建议条加可点的「采纳 / 忽略」两个按钮（现在一条鼠标出路都没有），文案按实际生效键改写。

### 2. 对话图输入框按回车 = 静默起一次真派发（消息没发出去）
- 位置：`src-ui/src/components/center/ProposalBar.tsx:136-137`（`document.addEventListener('keydown', …, true)` + `:130-131` 的 `preventDefault/stopPropagation`）、`src-ui/src/graph/GraphTab.tsx:799-806`（单行 `<input class="graph-input">`，`onKeyDown` 里 `Enter` 即 `sendInColumn`，placeholder 明写「回车发送」）
- 问题：捕获阶段挂在 `document` 上的监听先于 React 根容器触发，`stopPropagation()` 把事件掐在到达 `graph-input` 之前，`sendInColumn` 根本不会被调用。`isMultilineTarget` 只排除 `TEXTAREA` / contenteditable，单行 `INPUT` 不排除，于是任务书「焦点不在多行输入里」这条规则在对话图主输入框上恰好失效：用户以为在发消息，实际执行了 `acceptConsoleProposal → consoleRunUpdate(running) → runConsoleDispatch`，真的开跑并改图/花额度，草稿还留在框里（看上去像没发出去）。
- 复现：对话图 tab 有建议条 → 在底部输入框打字 → 回车。期望：消息进图；实际：输入框无反应，建议条消失，图上升出新节点、模型开跑。
- 建议改法：把「输入类元素」整体排除（`INPUT` / `TEXTAREA` / contenteditable / `closest('input,textarea,[contenteditable]')` 一律不抢键），只在焦点不在任何输入里（或建议条自身聚焦）时认 Enter；否则至少要求修饰键。

### 3. propose 超时/登记失败后，界面留着一条永远无法裁决的建议
- 位置：`src/console_api.rs:22`（`PROPOSAL_TIMEOUT = 25s`）、`:298-320`（超时只丢 pending，**不建 run**）、`src-ui/src/console-channel.ts:222-223`（先 `publishProposals` 再 `consoleRunUpdate('proposed')`）、`:231-236`（`await commands.consoleRunUpdate(…)` 抛错就中断，后面的 `publishProposals` 与 `runConsoleDispatch` 都不执行）
- 问题：开启了「用归类模型二次判断」时，分类是一次真模型调用（`console-channel.ts:178-196` 走 `dispatchGraphStage`，超时按 `settings.timeoutSecs`，默认 600s），经常超过 25s。此时 CLI 已拿到 503，Rust 里没有这条 run，但前端仍把建议条画出来：采纳时 `consoleRunUpdate` 抛「找不到 run」，`await` 中断 → 不派发、也不报错、建议条留着；`void operation.finally(...)` 还留下 unhandled rejection（`ProposalBar.tsx:135`）。忽略同理（`:255` 在前、`:257` 在后），连 Esc 都清不掉。
- 复现：设置页开启二次判断、归类模型选一个慢的 → `aicross propose …`（CLI 约 25s 后 503）→ 建议条出现 → 按 Enter：无任何反应、无提示。
- 建议改法：超时后前端不要保留（`consoleRunUpdate('proposed')` 失败就把该建议从列表里撤掉并提示「控制台未登记，请重试」）；或让 Rust 的登记幂等（收到 `proposed` 但无 pending 时按新 run 插入）；采纳/忽略路径加 try/catch 与错误提示。

### 4. 「派发」按钮的本地建议绕开通道在飞上限，且不进 run 表
- 位置：`src-ui/src/console-channel.ts:243-247`（`const localIo = { ...commands, listConsoleRuns: async () => [], consoleRunUpdate: async () => {} }`）、`:226-228`（`requestLocalProposal`）
- 问题：本地建议采纳后 `runConsoleDispatch` 拿到空的 `activeRuns`，`settings.consoleRunLimit` 检查恒不触发（`console-channel.ts:274-277`），Rust 的 `INFLIGHT_LIMIT`（`console_api.rs:20/350`）也够不着——这条路径既没有上限，也不在 run 表里：`aicross status/cancel` 看不到、停不了，CenterPanel 的「N 路在飞」（`CenterPanel.tsx:535`）也不计。用户可以连点连发任意条真派发。
- 复现：对话图输入框点「派发」→ 回车采纳 → 再点一次「派发」→ 再回车……三条 pipeline 同时在飞（`通道在飞上限` 设为 1 也拦不住）。
- 建议改法：本地建议也走一次 Rust 登记（加一个 UI 侧的登记命令），或采纳前用真实 `listConsoleRuns` 做上限与状态检查，不要用打桩 io 绕开。

### 5. 未裁决建议会变成 UI 看不见、Rust 里清不掉的幽灵
- 位置：`src-ui/src/console-channel.ts:143-147`（`proposalForTab` 只返回**第一条**匹配项）与 `:222`（新建议插到最前）；`src/console_api.rs:393` + `:169`（`should_relay_cancel` 对 `proposed` 返回 false）
- 问题：(a) 同一项目同时存在两条建议时，界面上只有新的那条；旧的那条既没显示也没入口（把新的处理掉才会重新露出，但 tab 一关就彻底找不到）。(b) 项目目录当前没有打开的 tab（CLI 给的是别的目录）时，建议只在 Rust 表里躺着，界面根本不出现（TASK-16 的派发会开图，propose 故意不开图，但没有通知或回退位置）。(c) webview 重载（开发热更、异常恢复）后 `proposalItems` 丢失而 Rust 里那条 run 仍是 `proposed`，CLI 的 `aicross cancel` 对它返回 0 却不改状态（`console_api.rs:169/160-166` 直接 `Ok(run)`），只能等 200 条上限把它挤掉。
- 复现：对同一项目连发两条 `aicross propose`（不同文本）→ 只看到第二条；重启 webview → 两条都不再显示，`aicross status <id>` 仍是 `proposed`。
- 建议改法：`cancel` 支持 `proposed → dismissed`；对没有对应 tab 的建议给一个兜底位置或提示；`proposalForTab` 返回全部匹配项并在条上标「还有 N 条」，或登记时限制同项目只留一条。

### 6. 归类模型判据过宽，且「模型判不必派」与「模型挂了」共用一句话
- 位置：`src-ui/src/console-channel.ts:196`（`/\bDISPATCH\b/i.test(answer.text)`）、`:182`（提示词里就写着「只输出 DISPATCH 或 SINGLE」）、`:167-177`（`classificationTarget` 找不到用户选的模型时回落到 `models[0]`）、`:176`（skipped 文案）
- 问题：判定用全文包含而非首个词——模型复述规则（「按规则应输出 DISPATCH 或 SINGLE，本次为 SINGLE」）就会被判成该派，方向上是多出一条建议（不是自动派发，危害有限，但开关的目的正是少误触发）。失败降级方向是安全的（不派发），但失败与非 DISPATCH 都写成「归类模型不建议派发」，用户无法区分是模型否决还是调用挂了；用户点名的分类模型不在资源列表时还会静默换成别的模型。
- 复现：开启开关 → 给一个会让模型复述规则的长文本 → 出现建议条（应为不建议）。
- 建议改法：只取回复里第一个 `DISPATCH|SINGLE` 词判定；失败与非建议分两句文案（如「分类失败，已按不派处理」）；选定模型不可用时提示而不是回落。

### 7. 测试：一处同义反复，一处测的是现实里不可能出现的组合
- 位置：`src/console_api.rs:741-743`；`src-ui/src/__tests__/proposal-20.test.ts:20-22`
- 问题：(a) `proposed_run_transitions_only_to_running_or_dismissed` 末段插 10 条 proposed + 3 条 running 后断言 `running 计数 == 3`——无论 proposed 是否占在飞上限都会通过，钉不住「建议不计入在飞上限」。能红的写法：先塞满 `INFLIGHT_LIMIT` 条 running，再把一条 proposed 改成 running 期望 `Err(409)`（`console_api.rs:394-395`）。(b) `proposalShortcutAction({key:'Enter',ctrlKey:true},{commander:true,ptyFocused:true})` 这一格传的 `targetMultiline` 是 undefined，而组件里两个值都从 `event.target` 算，`ptyFocused=true` 时 `targetMultiline` 必然为 true——这正是第 1 条漏网的原因。整套前端用例没有一个渲染 ProposalBar、派发真实 keydown 的集成断言，捕获阶段/offscreen/单行输入这些接线全无覆盖。
- 建议改法：补「3 条 running + 1 条 proposed 采纳 → 409」；补 `ProposalBar` 的 keydown 集成用例（target 为 xterm textarea、单行 input、隐藏 tab 各一格）。

### 8. 工作区里留着一个与实现相反的临时用例
- 位置：`src-ui/src/__tests__/tmp-orch-check.test.ts:10-21`
- 问题：两条断言按当前 `proposalShortcutAction` 的早退分支必然失败（内容看像是编排者的核对探针）。报告里的「38 文件 / 549 条通过」不包含它。若确为探针，交付前删掉或转成正式用例；若是实现者留的，则报告与树不一致。
- 建议改法：删除该文件，或把它按上一条并进 `proposal-20.test.ts` 并让实现通过。

### 9. 文案小项：同屏两个「派发」按钮 + 可能的「自定义模式 · 自定义模式」
- 位置：`src-ui/src/components/center/graph/GraphTab.tsx:826`（工具条「派发」→ 开确认屏）与 `:809-813`（输入框旁「派发」→ 只生成建议）；`src-ui/src/console-channel.ts:207-208`（`name: '自定义模式'`）配 `src-ui/src/graph/label.ts:36-39`
- 问题：同屏两个同名按钮行为不同（一个进确认屏真派，一个只出建议），AGENTS.md「一个概念一个词」下建议给建议入口换动词（如「建议派发」）。另：走 `targets` 的建议在用户已有自定义模式时，`presetDisplayName` 会拼成「自定义模式 · 自定义模式」（TASK-16 的 `consolePreset` 就是这样命名的，本期把这块名字搬到了建议条上）。
- 建议改法：按钮改「建议派发」；`targets` 生成的预设 `name` 传空串或节点来源名。

### 10. 二次判断每跑一次都在 dispatch 数据根下留一个 run 目录
- 位置：`src/dispatch.rs:1808-1826`（`run_root = dispatch_data_root()/runs/<run_id>`、`write_project_dispatch=false` 时 `log_dir = run_root/dispatch`）、`src-ui/src/console-channel.ts:184`（`runId: proposal-classify-<id>`）
- 问题：分类走 `dispatchGraphStage`，于是每次生成建议都会新建 `runs/proposal-classify-<uuid>/dispatch/STATE.md` 且永不清理，同时向全局发 `dispatch-progress`（各订阅方按 runId 过滤，界面无副作用，已核）。项目目录没被污染（`writeProjectDispatch:false` 生效，已核）。
- 建议改法：分类用固定 runId 复用同一目录，或结束后清理。

---

## 核过没问题的要点

- **`/v1/propose` 只登记不派发**：Rust 走 `relay_proposal`（`console_api.rs:298-320`），发的是 `aicross:console-propose`（与 `aicross:console-dispatch` 不同名）；前端 `runConsoleProposal` 只 `publishProposals` + `consoleRunUpdate('proposed')`（`console-channel.ts:199-224`），全程无 `runConsoleDispatch` / `dispatchGraphStage`（前端用例 `proposal-20.test.ts:53-70` 能让这条红）。无重试逻辑；超时、错误分支都只落 `skipped`/`rejected`/503，不会起派发；`onConsolePropose` 的 catch 也只回 `rejected`。
- **proposed 不计入在飞上限**：`insert_run`（用于 proposed）不做上限判断，`insert_inflight_run` 与 `apply_run_update` 只数 `status=="running"`；`proposed→running` 才判上限且 `running_count` 不含自身（`console_api.rs:390-395`）。前端 `runConsoleDispatch` 也只数 `running`。
- **状态机方向闭合**：`proposed` 只接受 `running`/`dismissed`（`:390-391`），`dismissed` 之后一律 `Err`（`:393`），终态拒绝迟到的 `running`（`:398-401`，`terminal_run_ignores_late_running_update` 钉住）；`skipped` 只在有 pending 建议时被吃掉且不落 run 表（`:366-372`）。新状态是追加，未改 `running/done/failed/rejected` 的既有含义；`console_run_update` 只把 `proposed/dismissed` 归到 `proposal` 事件类（`:422`），CenterPanel 靠它避免为建议切/开 tab（`CenterPanel.tsx:516`）。
- **薄中继没被破坏**：Rust 侧仍是校验（text 非空、目录 canonicalize、`reason` 单行 → 400）+ 发事件 + run 摘要（200 条上限、按 started_at 淘汰），没有拼提示词、没有写图、没碰 `.dispatch`；派发仍由前端 `startPipelineHeadless` 完成（`console-channel.ts:264` 起）。
- **CLI 同口径**：`propose` 与 `dispatch` 共用同一构造分支（`console_cli.rs:99-127`），退出码同路径（2 参数/编码、3 未运行、1 HTTP 业务失败/等待超时、0 成功）；`--text`/`--text=` 一律拒绝（`:100-102`），多行只走 `--text-file`（含 BOM 剥离、非 UTF-8 报错），`--reason` 多行在 CLI 与 Rust 两侧都拒；`aicross dispatch` 行为未改。
- **小任务判据同源**：只有一份 `isSmallEconomyTask`（`graph/dispatch.ts:412`），建议路径（`proposalDecision`）与既有派发路径共用；附件按 `event.attachments` 传入，CLI 侧本就没有附件参数，所以不会出现两套口径。二次判断默认关（`dispatch-settings.ts:17/54`），开启后失败的结果是「不出建议」而非派发，Rust 没有新增模型调用路径。
- **文案与规范**：建议条一行与定案文本一致（`建议派发 · <模式名> · N 路 · …`，`reason` 作浅色尾巴），模式名走 `presetDisplayName`（一律「X模式」），「派发」是 2 字动词短语，术语表已补 `docs/MANUAL.md:207-208`，未造新词（「忽略」出自定案）。`docs/CONSOLE-CHANNEL.md` 的接口/状态/CLI/`/派发` 段与实现一致（`proposed|running|done|failed|rejected|dismissed`、`skipped` 响应、`proposed` 不计上限、二次判断默认关）。
- **快捷键里能用对的部分**：捕获阶段挂 `document`（`ProposalBar.tsx:137`）确实先于 xterm 的 textarea 监听；`isComposing` 组字不触发；`TEXTAREA`/contenteditable 不触发；隐藏 tab 用 `offsetParent === null` 排除，一次按键不会触发两条建议的采纳（`CenterPanel.tsx:1711` 只有活动 tab 是 `display:flex`）；建议条不主动取焦点；`updateProposalPreset` 在自定义 ↔ 内置之间切换时 `targets` 的置空/还原正确。

## 总判

**不可交付**：硬约束「不自动执行」在结构上守住了（`/v1/propose` 只登记、Rust 只中继），但建议条在**总指挥 tab 无法裁决**（三个键全被 `targetMultiline` 早退吃掉，文案却写 Ctrl+Enter），且**对话图单行输入框的回车会把「发消息」静默升级成一次真派发**。这两条必须在验收前闭合。

---

## 按严重度排序

### 1. 总指挥 tab 里建议条完全无法裁决；文案写的 Ctrl+Enter 不生效
- 位置：`src-ui/src/components/center/ProposalBar.tsx:50`（早退）、`:87-90`（`isMultilineTarget` 判 `tagName === 'TEXTAREA'`）、`:121-122`（`targetMultiline` 与 `ptyFocused` 取自同一个 `event.target`）、`:142/144`（文案）、`:144-147`（整条 `div` 无任何点击处理）
- 问题：xterm 的 helper 就是 `<textarea class="xterm-helper-textarea">`（`TierTerminal.tsx:600/625/1845` 同款写法可证）。PTY 有焦点时 `isPtyTarget=true` **同时** `isMultilineTarget=true`，`proposalShortcutAction` 第一行就 `return null`，所以 Enter / Esc / Tab 三个键一个都不归建议条，全部原样进 codex。`ptyFocused` 为真时文案显示「Ctrl+Enter 采纳」，而这个组合恰恰是永远不生效的那一格。建议条又没有可点区域（无 `onClick`、无按钮），于是总指挥 tab 里既不能采纳也不能忽略——`aicross propose` 这个主入口的产物在主入口所在 tab 里是死的。
- 复现：总指挥 tab（codex 会话，焦点在终端）→ 让总指挥执行 `aicross propose --project <cwd> --reason "…" --text-file …`（或直接 `POST /v1/propose`）→ 顶部出现「建议派发 · 经济模式 · N 路 · **Ctrl+Enter 采纳** · Esc 忽略 · Tab 换模式」→ 按 Ctrl+Enter：建议条无变化，按键进了终端；Esc / Tab 同样进终端（codex 的「编辑上一条」/ 补全）。此时无任何鼠标路径可用。
- 建议改法：`targetMultiline: isMultilineTarget(event.target) && !isPtyTarget(event.target)`，让 PTY 走 `state.commander && state.ptyFocused` 分支；但立刻会付代价——在活的 codex 会话里吞掉 Esc（打断生成）和 Tab（补全）是真实损失，需要产品裁决：推荐「PTY 焦点下只认 Ctrl+Enter，Esc/Tab 放行」，同时给建议条加可点的「采纳 / 忽略」两个按钮（现在一条鼠标出路都没有），文案按实际生效键改写。

### 2. 对话图输入框按回车 = 静默起一次真派发（消息没发出去）
- 位置：`src-ui/src/components/center/ProposalBar.tsx:136-137`（`document.addEventListener('keydown', …, true)` + `:130-131` 的 `preventDefault/stopPropagation`）、`src-ui/src/graph/GraphTab.tsx:799-806`（单行 `<input class="graph-input">`，`onKeyDown` 里 `Enter` 即 `sendInColumn`，placeholder 明写「回车发送」）
- 问题：捕获阶段挂在 `document` 上的监听先于 React 根容器触发，`stopPropagation()` 把事件掐在到达 `graph-input` 之前，`sendInColumn` 根本不会被调用。`isMultilineTarget` 只排除 `TEXTAREA` / contenteditable，单行 `INPUT` 不排除，于是任务书「焦点不在多行输入里」这条规则在对话图主输入框上恰好失效：用户以为在发消息，实际执行了 `acceptConsoleProposal → consoleRunUpdate(running) → runConsoleDispatch`，真的开跑并改图/花额度，草稿还留在框里（看上去像没发出去）。
- 复现：对话图 tab 有建议条 → 在底部输入框打字 → 回车。期望：消息进图；实际：输入框无反应，建议条消失，图上升出新节点、模型开跑。
- 建议改法：把「输入类元素」整体排除（`INPUT` / `TEXTAREA` / contenteditable / `closest('input,textarea,[contenteditable]')` 一律不抢键），只在焦点不在任何输入里（或建议条自身聚焦）时认 Enter；否则至少要求修饰键。

### 3. propose 超时/登记失败后，界面留着一条永远无法裁决的建议
- 位置：`src/console_api.rs:22`（`PROPOSAL_TIMEOUT = 25s`）、`:298-320`（超时只丢 pending，**不建 run**）、`src-ui/src/console-channel.ts:222-223`（先 `publishProposals` 再 `consoleRunUpdate('proposed')`）、`:231-236`（`await commands.consoleRunUpdate(…)` 抛错就中断，后面的 `publishProposals` 与 `runConsoleDispatch` 都不执行）
- 问题：开启了「用归类模型二次判断」时，分类是一次真模型调用（`console-channel.ts:178-196` 走 `dispatchGraphStage`，超时按 `settings.timeoutSecs`，默认 600s），经常超过 25s。此时 CLI 已拿到 503，Rust 里没有这条 run，但前端仍把建议条画出来：采纳时 `consoleRunUpdate` 抛「找不到 run」，`await` 中断 → 不派发、也不报错、建议条留着；`void operation.finally(...)` 还留下 unhandled rejection（`ProposalBar.tsx:135`）。忽略同理（`:255` 在前、`:257` 在后），连 Esc 都清不掉。
- 复现：设置页开启二次判断、归类模型选一个慢的 → `aicross propose …`（CLI 约 25s 后 503）→ 建议条出现 → 按 Enter：无任何反应、无提示。
- 建议改法：超时后前端不要保留（`consoleRunUpdate('proposed')` 失败就把该建议从列表里撤掉并提示「控制台未登记，请重试」）；或让 Rust 的登记幂等（收到 `proposed` 但无 pending 时按新 run 插入）；采纳/忽略路径加 try/catch 与错误提示。

### 4. 「派发」按钮的本地建议绕开通道在飞上限，且不进 run 表
- 位置：`src-ui/src/console-channel.ts:243-247`（`const localIo = { ...commands, listConsoleRuns: async () => [], consoleRunUpdate: async () => {} }`）、`:226-228`（`requestLocalProposal`）
- 问题：本地建议采纳后 `runConsoleDispatch` 拿到空的 `activeRuns`，`settings.consoleRunLimit` 检查恒不触发（`console-channel.ts:274-277`），Rust 的 `INFLIGHT_LIMIT`（`console_api.rs:20/350`）也够不着——这条路径既没有上限，也不在 run 表里：`aicross status/cancel` 看不到、停不了，CenterPanel 的「N 路在飞」（`CenterPanel.tsx:535`）也不计。用户可以连点连发任意条真派发。
- 复现：对话图输入框点「派发」→ 回车采纳 → 再点一次「派发」→ 再回车……三条 pipeline 同时在飞（`通道在飞上限` 设为 1 也拦不住）。
- 建议改法：本地建议也走一次 Rust 登记（加一个 UI 侧的登记命令），或采纳前用真实 `listConsoleRuns` 做上限与状态检查，不要用打桩 io 绕开。

### 5. 未裁决建议会变成 UI 看不见、Rust 里清不掉的幽灵
- 位置：`src-ui/src/console-channel.ts:143-147`（`proposalForTab` 只返回**第一条**匹配项）与 `:222`（新建议插到最前）；`src/console_api.rs:393` + `:169`（`should_relay_cancel` 对 `proposed` 返回 false）
- 问题：(a) 同一项目同时存在两条建议时，界面上只有新的那条；旧的那条既没显示也没入口（把新的处理掉才会重新露出，但 tab 一关就彻底找不到）。(b) 项目目录当前没有打开的 tab（CLI 给的是别的目录）时，建议只在 Rust 表里躺着，界面根本不出现（TASK-16 的派发会开图，propose 故意不开图，但没有通知或回退位置）。(c) webview 重载（开发热更、异常恢复）后 `proposalItems` 丢失而 Rust 里那条 run 仍是 `proposed`，CLI 的 `aicross cancel` 对它返回 0 却不改状态（`console_api.rs:169/160-166` 直接 `Ok(run)`），只能等 200 条上限把它挤掉。
- 复现：对同一项目连发两条 `aicross propose`（不同文本）→ 只看到第二条；重启 webview → 两条都不再显示，`aicross status <id>` 仍是 `proposed`。
- 建议改法：`cancel` 支持 `proposed → dismissed`；对没有对应 tab 的建议给一个兜底位置或提示；`proposalForTab` 返回全部匹配项并在条上标「还有 N 条」，或登记时限制同项目只留一条。

### 6. 归类模型判据过宽，且「模型判不必派」与「模型挂了」共用一句话
- 位置：`src-ui/src/console-channel.ts:196`（`/\bDISPATCH\b/i.test(answer.text)`）、`:182`（提示词里就写着「只输出 DISPATCH 或 SINGLE」）、`:167-177`（`classificationTarget` 找不到用户选的模型时回落到 `models[0]`）、`:176`（skipped 文案）
- 问题：判定用全文包含而非首个词——模型复述规则（「按规则应输出 DISPATCH 或 SINGLE，本次为 SINGLE」）就会被判成该派，方向上是多出一条建议（不是自动派发，危害有限，但开关的目的正是少误触发）。失败降级方向是安全的（不派发），但失败与非 DISPATCH 都写成「归类模型不建议派发」，用户无法区分是模型否决还是调用挂了；用户点名的分类模型不在资源列表时还会静默换成别的模型。
- 复现：开启开关 → 给一个会让模型复述规则的长文本 → 出现建议条（应为不建议）。
- 建议改法：只取回复里第一个 `DISPATCH|SINGLE` 词判定；失败与非建议分两句文案（如「分类失败，已按不派处理」）；选定模型不可用时提示而不是回落。

### 7. 测试：一处同义反复，一处测的是现实里不可能出现的组合
- 位置：`src/console_api.rs:741-743`；`src-ui/src/__tests__/proposal-20.test.ts:20-22`
- 问题：(a) `proposed_run_transitions_only_to_running_or_dismissed` 末段插 10 条 proposed + 3 条 running 后断言 `running 计数 == 3`——无论 proposed 是否占在飞上限都会通过，钉不住「建议不计入在飞上限」。能红的写法：先塞满 `INFLIGHT_LIMIT` 条 running，再把一条 proposed 改成 running 期望 `Err(409)`（`console_api.rs:394-395`）。(b) `proposalShortcutAction({key:'Enter',ctrlKey:true},{commander:true,ptyFocused:true})` 这一格传的 `targetMultiline` 是 undefined，而组件里两个值都从 `event.target` 算，`ptyFocused=true` 时 `targetMultiline` 必然为 true——这正是第 1 条漏网的原因。整套前端用例没有一个渲染 ProposalBar、派发真实 keydown 的集成断言，捕获阶段/offscreen/单行输入这些接线全无覆盖。
- 建议改法：补「3 条 running + 1 条 proposed 采纳 → 409」；补 `ProposalBar` 的 keydown 集成用例（target 为 xterm textarea、单行 input、隐藏 tab 各一格）。

### 8. 工作区里留着一个与实现相反的临时用例
- 位置：`src-ui/src/__tests__/tmp-orch-check.test.ts:10-21`
- 问题：两条断言按当前 `proposalShortcutAction` 的早退分支必然失败（内容看像是编排者的核对探针）。报告里的「38 文件 / 549 条通过」不包含它。若确为探针，交付前删掉或转成正式用例；若是实现者留的，则报告与树不一致。
- 建议改法：删除该文件，或把它按上一条并进 `proposal-20.test.ts` 并让实现通过。

### 9. 文案小项：同屏两个「派发」按钮 + 可能的「自定义模式 · 自定义模式」
- 位置：`src-ui/src/components/center/graph/GraphTab.tsx:826`（工具条「派发」→ 开确认屏）与 `:809-813`（输入框旁「派发」→ 只生成建议）；`src-ui/src/console-channel.ts:207-208`（`name: '自定义模式'`）配 `src-ui/src/graph/label.ts:36-39`
- 问题：同屏两个同名按钮行为不同（一个进确认屏真派，一个只出建议），AGENTS.md「一个概念一个词」下建议给建议入口换动词（如「建议派发」）。另：走 `targets` 的建议在用户已有自定义模式时，`presetDisplayName` 会拼成「自定义模式 · 自定义模式」（TASK-16 的 `consolePreset` 就是这样命名的，本期把这块名字搬到了建议条上）。
- 建议改法：按钮改「建议派发」；`targets` 生成的预设 `name` 传空串或节点来源名。

### 10. 二次判断每跑一次都在 dispatch 数据根下留一个 run 目录
- 位置：`src/dispatch.rs:1808-1826`（`run_root = dispatch_data_root()/runs/<run_id>`、`write_project_dispatch=false` 时 `log_dir = run_root/dispatch`）、`src-ui/src/console-channel.ts:184`（`runId: proposal-classify-<id>`）
- 问题：分类走 `dispatchGraphStage`，于是每次生成建议都会新建 `runs/proposal-classify-<uuid>/dispatch/STATE.md` 且永不清理，同时向全局发 `dispatch-progress`（各订阅方按 runId 过滤，界面无副作用，已核）。项目目录没被污染（`writeProjectDispatch:false` 生效，已核）。
- 建议改法：分类用固定 runId 复用同一目录，或结束后清理。

---

## 核过没问题的要点

- **`/v1/propose` 只登记不派发**：Rust 走 `relay_proposal`（`console_api.rs:298-320`），发的是 `aicross:console-propose`（与 `aicross:console-dispatch` 不同名）；前端 `runConsoleProposal` 只 `publishProposals` + `consoleRunUpdate('proposed')`（`console-channel.ts:199-224`），全程无 `runConsoleDispatch` / `dispatchGraphStage`（前端用例 `proposal-20.test.ts:53-70` 能让这条红）。无重试逻辑；超时、错误分支都只落 `skipped`/`rejected`/503，不会起派发；`onConsolePropose` 的 catch 也只回 `rejected`。
- **proposed 不计入在飞上限**：`insert_run`（用于 proposed）不做上限判断，`insert_inflight_run` 与 `apply_run_update` 只数 `status=="running"`；`proposed→running` 才判上限且 `running_count` 不含自身（`console_api.rs:390-395`）。前端 `runConsoleDispatch` 也只数 `running`。
- **状态机方向闭合**：`proposed` 只接受 `running`/`dismissed`（`:390-391`），`dismissed` 之后一律 `Err`（`:393`），终态拒绝迟到的 `running`（`:398-401`，`terminal_run_ignores_late_running_update` 钉住）；`skipped` 只在有 pending 建议时被吃掉且不落 run 表（`:366-372`）。新状态是追加，未改 `running/done/failed/rejected` 的既有含义；`console_run_update` 只把 `proposed/dismissed` 归到 `proposal` 事件类（`:422`），CenterPanel 靠它避免为建议切/开 tab（`CenterPanel.tsx:516`）。
- **薄中继没被破坏**：Rust 侧仍是校验（text 非空、目录 canonicalize、`reason` 单行 → 400）+ 发事件 + run 摘要（200 条上限、按 started_at 淘汰），没有拼提示词、没有写图、没碰 `.dispatch`；派发仍由前端 `startPipelineHeadless` 完成（`console-channel.ts:264` 起）。
- **CLI 同口径**：`propose` 与 `dispatch` 共用同一构造分支（`console_cli.rs:99-127`），退出码同路径（2 参数/编码、3 未运行、1 HTTP 业务失败/等待超时、0 成功）；`--text`/`--text=` 一律拒绝（`:100-102`），多行只走 `--text-file`（含 BOM 剥离、非 UTF-8 报错），`--reason` 多行在 CLI 与 Rust 两侧都拒；`aicross dispatch` 行为未改。
- **小任务判据同源**：只有一份 `isSmallEconomyTask`（`graph/dispatch.ts:412`），建议路径（`proposalDecision`）与既有派发路径共用；附件按 `event.attachments` 传入，CLI 侧本就没有附件参数，所以不会出现两套口径。二次判断默认关（`dispatch-settings.ts:17/54`），开启后失败的结果是「不出建议」而非派发，Rust 没有新增模型调用路径。
- **文案与规范**：建议条一行与定案文本一致（`建议派发 · <模式名> · N 路 · …`，`reason` 作浅色尾巴），模式名走 `presetDisplayName`（一律「X模式」），「派发」是 2 字动词短语，术语表已补 `docs/MANUAL.md:207-208`，未造新词（「忽略」出自定案）。`docs/CONSOLE-CHANNEL.md` 的接口/状态/CLI/`/派发` 段与实现一致（`proposed|running|done|failed|rejected|dismissed`、`skipped` 响应、`proposed` 不计上限、二次判断默认关）。
- **快捷键里能用对的部分**：捕获阶段挂 `document`（`ProposalBar.tsx:137`）确实先于 xterm 的 textarea 监听；`isComposing` 组字不触发；`TEXTAREA`/contenteditable 不触发；隐藏 tab 用 `offsetParent === null` 排除，一次按键不会触发两条建议的采纳（`CenterPanel.tsx:1711` 只有活动 tab 是 `display:flex`）；建议条不主动取焦点；`updateProposalPreset` 在自定义 ↔ 内置之间切换时 `targets` 的置空/还原正确。