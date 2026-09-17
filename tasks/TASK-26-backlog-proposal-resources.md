# TASK-26 待办册清理 · 建议条与资源页批（09-17）

清 `tasks/fix-04-r5-backlog.md` 里六条。**只做下面六条**；每条改完在 backlog 里用 `~~…~~` 划掉并注明「已修（09-17，TASK-26）」。

**另有一期在飞（TASK-25，控制台消息流），这些文件一律不要碰**：`src-ui/src/console-chat.ts`、`src-ui/src/console-session.ts`、`src-ui/src/components/center/ConsoleChat.tsx`、`src-ui/src/components/center/ConsoleChat.css`、`src-ui/src/store/app-state.tsx`、`src-ui/src/components/center/CenterPanel.tsx`、`docs/MANUAL.md`。

## 建议条三条（前端）

### 66. 右栏预设下拉的 Esc 被建议条短路（低）
`PresetGroups.tsx` 的 `PresetSelect` 下拉关闭挂在冒泡阶段，建议条（`ProposalBar.tsx`）在捕获阶段 `stopPropagation`，于是有建议条时打开下拉按 Esc：下拉不收、建议反被忽略。修法：下拉打开时让建议条放行——可以在下拉打开时给它的容器标一个属性（例如 `data-popover-open`），建议条的放行判定里一并认它；**复用 `src-ui/src/lib/focus-enforcer.ts` 里已有的判定入口**（`hasOpenModal` 那一处），不要另写一套选择器。

### 67. `contenteditable="false"` 被当成输入控件（极低）
`proposalTargetFlags` 用 `closest('input,textarea,[contenteditable]')`，属性选择器会把显式不可编辑的 `contenteditable="false"` 也算进去，多排除一次按键。改成 `[contenteditable="true"],[contenteditable=""]`（HTML 里空值等同 true），同步改 `isContentEditable` 那一支的判定口径。

### 68. 归类调用并发时日志互写（极低）
归类调用固定了 `runId/nodeId = 'proposal-classify'`，两条建议并发走归类时 `runs/proposal-classify/dispatch/` 下同名互写。判定只取回复文本、结果不受影响，但日志失真。修法：runId 带上建议 id（仍是可预期的固定前缀，便于清理），或者并发时串行化。**报告里说明选了哪种、为什么。**

## 资源页与引擎三条（Rust 为主）

### 56. `detect_into` 的 pi 分支已成生产死代码（低）
该分支保留了「不刷登录」的旧语义，日后若恢复调用会无声回归。**先确认它确实没有生产调用方**（grep 证据写进报告），确认后删除；若发现仍有调用方，则改为注明理由而不删。

### 57. 跳过未登录备选的说明不进主尝试留痕（低）
引擎跳过未登录备选时，说明只加在返回结果上，主尝试的派发留痕里没有，事后查账看不出为什么没切备选。补进主尝试的留痕 `runner_note`。另：引擎每个候选各读一次资源库取 login，改成一次派发读一次、候选间复用。

### 58. `status:"ready"` 但退出码非 0 的口径（仅记录）
现在判成未登录而不是未检查，这是有意取舍。**不改行为**，只在代码注释与 `docs/CONSOLE-CHANNEL.md` 以外的合适位置（资源模块的判定函数处）写清这个口径与理由，并补一条测试把现行为钉住，免得以后有人「顺手修正」。

## 不做

- 不做任何重构、不清理无关死代码、不动留痕 schema 字段名。

## 验证

`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。

每条都要有能红的测试（58 是钉住现行为的测试）。每条先确认去掉实现会红，并在报告里写清怎么红的。**本仓连吃了三次「只测纯函数漏接线」**：66 与 67 至少各有一格走 `proposalShortcutFromTarget` 这类真实接线函数，不要只测底层纯函数。

保持文件原有换行（`src/server.rs`、`src/main.rs` CRLF，其它 Rust LF，`src-ui/src/tauri.ts` 混合只做字节级最小改动）；不 `cargo fmt`；不起窗口、不发真机请求、不调真实模型；不 commit、不 push。报告写 `tasks/REPORT-26.md`，逐条对应 56/57/58/66/67/68。
