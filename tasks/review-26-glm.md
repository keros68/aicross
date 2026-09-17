审查完毕。以下为结论。

## 总判

六条全部闭合、实现与测试走真实接线、禁区未碰；但 **68 的选型重新引入了 fix-20-r1 第 11 条当年判定要修的「runs/ 目录无清理增殖」问题，且报告未记录这一冲突**，另有 57 的一个新分支无测试覆盖等几处低危事项。

## 问题清单（按严重度）

1. **（中低）68 改回按建议 id 建 run 目录，撤销了 fix-20-r1 #11 的修复，且无清理补偿** — `src-ui/src/console-channel.ts:254`
   - 问题：`classifyId = proposal-classify-${proposal_id}`（UUID）后，每条建议归类各建一个 `runs/proposal-classify-<uuid>/dispatch/`（STATE.md + raw）。这正是 `tasks/fix-20-r1.md:32`（DS 10）当年记为缺陷、由 TASK-20 用固定 runId 修掉的问题；该修复又产生了本条 backlog 68 的并发互写，本期选了「带 id」方案等于回到原点。
   - 证据：全仓无任何 `runs/` 清理逻辑；唯一读方 `latest_before_dir`（`src/dispatch.rs:2478`）只做全量扫描。classify 在 `classifyProposals` 开启时每次 propose 都跑。
   - 复现：开二次判断，连续提建议 → 数据目录 `runs/` 下每条建议新增一个目录，永不清理。
   - 改法：归类调用结束后（成功/失败）删除 `runs/<classifyId>`，或在 REPORT-26 补记与 fix-20-r1 #11 的取舍及清理计划。任务书确实给了这个选项，但「便于清理」的前提（前缀可 glob）目前没有任何清理动作兑现，报告只字未提冲突。

2. **（低）57 新增的「备选尝试已发生后回写主尝试记录」分支无测试覆盖** — `src/dispatch.rs:3475-3481`
   - 问题：`task26_logged_out_fallback_uses_one_resource_snapshot_and_updates_main_record`（dispatch.rs:7750）里未登录候选是唯一候选、从未真跑备选，`result.dispatch_file == main_record.0`，走不进 3475-3481 这个新分支；报告所述红测（退回旧写法后主记录 `runner_note` 为空失败）在该场景命中的是 `append_result_runner_note` 的同步（result 即主记录），不是这条分支。
   - 复现：构造 `fallback_order` 两条（备 1 真跑失败、备 2 未登录）即可使 `result.dispatch_file != main_record.0`。
   - 改法：补一例覆盖该分支，断言主记录文件含「跳过 …（未登录）」。
   - 附 nit：`dispatch_file` 可为空串（`write_record(...).unwrap_or_default()`，dispatch.rs:4227-4237），极端组合下（主留痕写失败 + 备选真跑 + 遇未登录）会对空路径调 `sync_record_runner_note`，只多一句「更新主尝试派发记录失败」note，无损坏。

3. **（低/记录）67 丢了对 `contenteditable="plaintext-only"` 的排除** — `src-ui/src/components/center/ProposalBar.tsx:108`
   - 旧选择器 `[contenteditable]` 会把 `plaintext-only`（Chrome 136+ 支持）也当输入控件排除按键；新选择器不命中，焦点在其中时 Enter/Esc/Tab 会被建议条抢走。本应用未见该取值的元素，且任务书指定了 precisely 这个选择器（与 `global.css:91-92` 同口径），仅记录。

4. **（记录）66 的连带效果：下拉打开期间终端焦点守卫也不抢** — `src-ui/src/lib/focus-enforcer.ts:23`
   - `.preset-select[open]` 并入 `hasOpenModal` 后，`shouldEnforceTerminalFocus`（`CenterPanel.tsx:823` 在用）同样放行。这是合理效果（抢焦点会 blur 收起下拉），但 `focus-enforcer.test.ts` 无对应用例（同一实现路径经 `proposalShortcutFromTarget` 用例覆盖，见下）。

5. **（记录）57 一次派发只读一次库的过期窗口** — `src/dispatch.rs:3351`
   - 快照取于主尝试之前；主尝试耗时期间库内 login/额度变化（含并发另一派发经 `record_quota_hit` 写入的标记）对本派发不可见：可能多试一个刚耗尽的候选（自然失败并留痕），或晚一轮才识别恢复的登录。后果有界、下次派发自愈，且是任务书明确要求的形态——属接受取舍，非缺陷。

## 核过没问题的要点

- **56 死代码删除无误伤**：`detect_into` 的 pi 分支已删，`detect_into_supports_agent`（resources.rs:825-827）显式拒绝并报错，不再无声回旧语义。全部 4 个调用点均传不进 pi：2335 处 pi 被 `prepared_pi` Some 分支截走（2327）、2367 是 `agent_id=="pi"` 的显式 else、2599 的 id 是刚生成的 `manual-<uuid>`（2576）、4475 是 `#[ignore]` 真机测试且只循环 claude/codex/kimi（pi 走 4477 prepared 路径）。`detect_pi` 仍被 `prepare_pi_detection`（893）使用。报告含删除前 grep。
- **66 判定入口复用与恢复**：放行收敛在 `hasOpenModal` 单点，ProposalBar 经 `proposalShortcutFromTarget` 复用（ProposalBar.tsx:18,119），未另写选择器，PresetGroups.tsx 未改。放行是每次 keydown 现查 `document` 的动态判定，下拉收起后立即恢复吃键，不会永久放行；捕获阶段返回 null 不再 `stopPropagation`，PresetSelect 冒泡监听（PresetGroups.tsx:98-108）正常收下拉，再按一次 Esc 才忽略建议——与 backlog 66 预期一致。
- **67 正常可编辑目标不受影响**：`contenteditable="true"` 与空值仍排除（测试 77/78 两格），`"false"` 放行；`isContentEditable` 分支是删除而非改口径（属性反射使两者等价），偏差无害。66、67 均走 `proposalShortcutFromTarget` 真实接线（proposal-20.test.ts:58-79），满足任务书要求。
- **68 查找/清理兼容**：src 内无任何按 `proposal-classify` 字面量的查找或清理逻辑；`latest_before_dir` 全量扫 `runs/*` 不依赖 runId；classify 是只读节点、无 before 快照，不影响回退查找。proposal_id 两处来源均为 UUID（console_api.rs:338 / `crypto.randomUUID()`），做目录名安全。并发测试走真 `classifyProposal` + `Promise.all`。
- **58 口径钉住**：注释在判定函数处（resources.rs:1196-1200），行为未改（1206-1210）；`task26_ready_status_with_nonzero_exit_is_logged_out_by_policy`（3136）钉住 ready+退出码 7 → LoggedOut，既有 task17 用例钉住 ready+0 → LoggedIn。
- **红测方向推演成立**：66（旧 `hasOpenModal` 返 false → `'dismiss'` ≠ `null`）、67（旧选择器含 `[contenteditable]` 子串 → 假 target 命中 → `null` ≠ `'accept'`）、68（旧常量 runId 不匹配）、57（读库 3→1、记录文件断言）按新旧实现逐条推演均可红；58 报告自述为变异检验（临时改分支变红），方式合理。
- **禁区与收尾**：git status 改动文件与任务书禁区（console-chat.ts、console-session.ts、ConsoleChat.*、app-state.tsx、CenterPanel.tsx、MANUAL.md，及 server.rs/main.rs/tauri.ts）无交集；backlog 六条全部划线并注明「已修（09-17，TASK-26）」；四个改动代码文件均无 CRLF（grep `\r$` 零命中）。
- **限制**：本会话无 shell 工具，`cargo check/test`、vitest、build 无法复核，报告中的验证结果只能采信其记录；红测结论是我按实现推演的，非实际执行。