# TASK-20 R1 修正轮复审

**总判：可交付。** 修正清单 12 条全部闭合，两条阻断项（PTY 快捷键、单行输入回车误采纳）已按裁决修到位且测试能红；未发现任何破坏「永不自动执行」的新路径。以下是静态核查发现的低severity残留项与新引入的小问题。

说明：本会话没有可用 shell，未重跑测试；报告中的验证数字（cargo 446 通过 / vitest 558 通过 / build 通过）未复核，但下文各条「能否红」均为按代码逐格推演的结论。

## 问题清单（按严重度）

1. **【低】「在飞已满，稍后再采纳」与行为矛盾：建议已被移除，不存在「稍后再采纳」** — `src-ui/src/console-channel.ts:322-324`
   `acceptConsoleProposal` 的 catch 先 `publishProposals(filter)` 把该建议从列表删掉，再发通知。清单第 5 条要求的是「在条上显示原因」，实现改成了撤条+通知；而 Rust 侧行仍是 `proposed`（无 text 无法重建，`dismissStaleConsoleProposals` 只在下一次 webview 启动时把它翻成 dismissed），界面上从此没有任何入口。
   复现：在飞 3 条 → 采纳第 4 条 → 条消失，出现「在飞已满，稍后再采纳」，但永远无法再采纳。
   建议：文案改「在飞已满，建议已撤回」，或保留建议条只把原因挂上去。

2. **【低】右栏预设下拉的 Esc 关闭被建议条短路（第 3 条的同类残留）** — `src-ui/src/components/right/orchestration/PresetGroups.tsx:98-108` vs `src-ui/src/components/center/ProposalBar.tsx:163-165`
   `PresetSelect` 的下拉关闭挂在 document 冒泡阶段（`keydown` Escape），建议条在捕获阶段 `preventDefault + stopPropagation`。清单第 3 条点名了 modal（已修，SettingsModal.tsx:397-398 与 ResourcesOverlay.tsx:870-871 都有 `aria-modal="true"`）和派发页（核过：DispatchOverlay 无任何 Esc 处理），但右栏这个非 modal 下拉是第三处 document 级 Esc。
   复现：有建议条 → 打开右栏预设下拉 → 按 Esc → 下拉不收，建议反而被忽略。
   建议：可接受（再按一次 Esc 的损失小）；若要收口，把 `.preset-select[open]` 之类并入放行判定，或在下拉打开时给根挂 `aria-modal`。

3. **【低】`aicross cancel` 对建议不幂等，第二次 cancel 返回 409** — `src/console_api.rs:163-167`、`src/console_api.rs:410`
   第一次 cancel `proposed` → `dismissed`（200，正确）；第二次再 cancel 同一条：`dismiss_proposed_run` → `apply_run_update` 命中 `dismissed 终态` 分支 → `Err("已忽略的建议不能再更改")` → 409，CLI 退出码 1。文档（CONSOLE-CHANNEL.md:94）只写「对 proposed 执行后转为 dismissed」，未承诺幂等，但脚本侧重试/并发 cancel 会拿到失败。
   建议：`run.status == "dismissed"` 时直接返回当前快照（幂等 200）。

4. **【低】外项目建议的兜底展示不显示目标项目，存在误采纳空间** — `src-ui/src/console-channel.ts:53-64`、`src-ui/src/components/center/ProposalBar.tsx:198-208`
   清单 7(b) 的兜底已实现（无匹配 tab → 挂到当前活动图；总指挥条兜底显示全部，`console-channel.ts:188`），但建议条上只有「建议派发 · 模式 · N 路 · 键位 · reason」，没有 project/reason 之外的目录信息。项目 X 的建议显示在项目 Y 的活动图 tab 上时，用户无从分辨，采纳后会给 X 新开图跑流水线。
   建议：兜底展示（graph_id 与 tab 项目不一致或 commander 兜底）时在条上追加目标目录尾巴。

5. **【极低】`[contenteditable]` 选择器把 `contenteditable="false"` 也算输入控件** — `src-ui/src/components/center/ProposalBar.tsx:109`
   属性选择器匹配 `contenteditable="false"`（不可编辑）。仅当焦点落在显式只读的 contenteditable 元素上时多排除一次按键，方向安全（多放行、少抢键），实际元素罕见。
   建议：改用 `closest('[contenteditable="true"],input,textarea')` 或不修。

6. **【极低】固定 runId 并发分类共用同一 run 目录，日志文件互写** — `src-ui/src/console-channel.ts:247` + `src/dispatch.rs:1808-1822`
   固定 `runId/nodeId = 'proposal-classify'` 后，两条建议并发走归类时 `runs/proposal-classify/dispatch/STATE.md` 与 `<node_id>.raw.txt` 同名互写（最后写者胜）。判定只取回复文本、不读 STATE，结果不受影响；清单本身允许「固定 runId 复用」，顺序场景安全。仅并发时日志失真。

## 核过没问题的要点

- **第 1 条（PTY，阻断项）已闭合**：`proposalShortcutAction` 的 `commander && ptyFocused` 分支在 `ProposalBar.tsx:58`，先于输入控件早退（59 行）；PTY 焦点只认 Ctrl+Enter，Esc/Tab/纯回车返回 null 且**不** preventDefault（`ProposalBar.tsx:163` 提前 return），原样进 PTY。组件实际走 `proposalShortcutFromTarget`（159 行）——与测试同一纯函数，接线不再脱节；四格假 target 断言齐（`proposal-20.test.ts:34-50`），报告所列先红路径与代码退化方向吻合。文案「Ctrl+Enter 采纳 · 或点右侧按钮」（67 行）与生效键一致，无 Esc/Tab 字样。
- **第 2 条（单行 input，阻断项）已闭合**：`proposalTargetFlags` 把 INPUT/TEXTAREA/contenteditable/closest 全算输入（106-109 行）；焦点在 `graph-input` 时 handler 对 null 提前返回，`GraphTab.tsx:804-806` 的 `sendInColumn` 能收到回车；测试 :46 可红（去掉 input 判定即变 accept）。
- **「永不自动执行」无破坏路径**：`runConsoleDispatch` 全仓仅三个入口——`acceptConsoleProposal`（键盘/按钮，且先过前端上限 ：309-313 与 Rust `INFLIGHT_LIMIT` :411-413）、`onConsoleDispatch`（`/v1/dispatch` 本就是显式派发）、DispatchOverlay。超时→503、错误→rejected/skipped、重启→dismissed，无定时器、无重试、无自动采纳；`dismissStaleConsoleProposals` 只 dismiss 不派发。回执与 25s 窗口的极限竞态（24.99s 注册成功但 CLI 已超时）下 run 行已在表里，采纳仍可用，不留幽灵。
- **第 4 条已闭合**：先 `await consoleRunUpdate('proposed')`/`consoleRegisterProposal` 再 `mergeProposal`（`console-channel.ts:287-292`）；归类超时 `Math.min(timeoutSecs, 20) < 25`（:250）；采纳/忽略 catch 后本地移除+可见通知（:321-325、:333-336）；`void operation.then(ok, fail)` 双分支清 busy（`ProposalBar.tsx:173`），unhandled rejection 消除。测试 :103-113（登记失败不显示幽灵）可红。
- **第 6 条已闭合**：`console_register_proposal` 命令已注册（`server.rs:3990`），`localIo` 打桩全仓无残留；本地与通道共用 `acceptConsoleProposal` 同一上限检查；本地建议进 run 表，`aicross status/cancel` 可及。测试 :115-129 可红（跳过登记则 `registered` 为空）。
- **第 7 条已闭合**：HTTP cancel 对 proposed → dismissed（`console_api.rs:163-167` + 测试 :785-792 可红）；启动时遗留 proposed 翻 dismissed（`console-channel.ts:491-506` + 测试，且仅 `proposalItems` 为空时执行，不会误伤本会话建议）；`mergeProposal` 按 id 合并非整表替换（:139-143）；「还有 N 条」+ 处理后露出下一条（`ProposalBar.tsx:197`）；无 tab 兜底位置已给（见问题 4 的代价说明）。
- **第 8 条已闭合**：Rust 上限测试改为「塞满 `INFLIGHT_LIMIT` 条 running + proposed→running 期望 Err 且状态保持 proposed」（`console_api.rs:776-781`），删除 ：411-413 检查即在 `unwrap_err` 处红，非同义反复；前端未引入 jsdom，标志计算抽成 `proposalTargetFlags`/`proposalShortcutFromTarget` 且被组件真实调用；modal、单行、登记顺序、按 id 合并、固定目录各有独立断言；`tmp-orch-check.test.ts` 已不在树上。
- **第 9 条已闭合**：`proposalClassificationVerdict` 只取第一个 `DISPATCH|SINGLE` 判词（`console-channel.ts:237-239`，首词 SINGLE 后文含 DISPATCH 也不派）；「归类模型判定不必派发」与「分类失败，已按不派处理：…」分成两句（:218-222）；`classificationTarget` 指定模型缺失直接抛「所选归类模型当前不可用」，无静默回退（:225-235）。
- **第 10 条已闭合**：输入框旁按钮为「建议派发」（`ProposalBar.tsx:91`、`GraphTab.tsx:809-813`、`Gambit.tsx:918`），工具条「派发」未动（`GraphTab.tsx:826`）；targets 预设 `name: ''`（`console-channel.ts:78,273`），`presetDisplayName` 空名不拼分隔（`label.ts:28-31`），测试 :140-154 断言不再出现「自定义模式 · 」。
- **第 11、12 条已闭合**：固定 `runId/nodeId = 'proposal-classify'` + `timeoutSecs: 20` 有断言（测试 :197-209）；`docs/CONSOLE-CHANNEL.md:29`（`/v1/dispatch` 忽略 reason）、`:94`（cancel proposed→dismissed）；`docs/PRODUCT.md:286-289`（入口 `aicross propose`、「建议派发」按钮、三种焦点状态键位文案与实现逐字一致）。
- **三种焦点状态文案与生效键一致**：PTY（`Ctrl+Enter 采纳 · 或点右侧按钮`，:58 生效）/ 输入控件（`点右侧按钮`，:59 全排除）/ 其它（`回车采纳 · Esc 忽略 · Tab 换模式`，:60-63 逐一对应）；文案取 `document.activeElement`、动作取 `event.target`，焦点事件同步先于按键，二者不会撕开；graph tab 内终端聚焦时 commander 为 false，正确落入「点右侧按钮」而非 Ctrl+Enter 档。
- **保持项抽查未动**：`apply_run_update` 状态机方向（`console_api.rs:407-417`）、proposed 不计在飞（:405 计数只数 running）、`console_register_proposal` 发 `proposal` 类事件不触发开图（:450 + `CenterPanel.tsx:516`）、隐藏 tab `offsetParent === null` 守卫仍在（`ProposalBar.tsx:158`）、`tauri.ts:364` 命令表仅追加一行。