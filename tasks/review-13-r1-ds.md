以下为只读复核（无 shell，未能运行 `cargo check/test`、`vitest`、`tsc -b`，报告里的通过数字与 `git diff` 边界未复核；所有结论来自源码与测试文件阅读）。

## 总判

**可交付但有未闭合项**：15 条必修全部落到实现与测试上，且关键负例（1/2/5/6/8/9 与前端文案）在旧实现下确实会红；但仍有未闭合项——叫停撞上「派发后验证门」会被报成验证未通过并可能多派一轮修正、被链接替换的既有文件会在恢复阶段被删、以及几处记账失败被当成节点失败。

## 逐条核对

**1. 已修。** `restore_snapshot_paths` 改为逐路径收集失败继续（`src/dispatch.rs:2889-2961`，写入失败入清单 `2939-2942`）；`restoration_error` 产出「红绿核对恢复本次改动失败：<路径清单>，请手动核对工作区」（`2862-2873`），`apply_red_green_status` 把 `workspace_error` 映射成 `status="error"`+`error=note`（`2875-2882`，调用点 `4033-4035`）；结果卡红字靠 `PinnedNote` 在 `status==='error'` 时给 `dispatch-warning`（`DispatchBits.tsx:29-32`，断言在 `review-defaults-12.test.ts:493-494`）。测试 `task13_restore_failure_is_best_effort_and_maps_to_error`（`9163-9190`）旧实现下会红：旧代码首错即返，而被旧回滚删掉的 `z-restored.txt` 只能靠这次尽力恢复写回；状态也只会是 `skipped`/`validation_failed`。残留（见 C）：`3070-3078`（green 阶段写状态文件失败）没有补一次写回 after。

**2. 已修（附一条可见性未闭合）。** 核对前写 `.dispatch/<run>/<node>.redgreen`（结构 `2830-2835`，写入 `3014-3021`，推进到 green `3070-3071`），正常结束各分支删除（`3033`、`3043`、`3052`、`3061`、`3089`）；reconcile 先查该标记 → `unknown` + 「上次红绿核对被打断，工作区可能停在回退状态，快照在 <before_dir>」（`1596-1611`）。测试 `task13_interrupted_marker_makes_reconcile_request_review`（`9285-9298`）旧实现下 `error` 为空 → 红。状态文件写失败：初始写失败（`3015-3021`）返回 skipped+「…，未做红绿核对」，此时尚未回退、工作区未被改动（可接受）；green 阶段写失败（`3070-3078`）置 error，但工作区停在回退态且没有补写回（见 C）。两处写失败均无测试。未闭合：该说明只到 `ReconcileNode.error`，UI 无渲染路径（`src-ui/src/graph/dispatch.ts:999` 对 unknown 固定写 `RECONCILE_UNKNOWN_TEXT`；`DispatchOverlay.tsx:547-556` 只渲染路由行），用户看不到「快照在 …」。

**3. 已修。** 等待验收 `测试未钉住，未送审`（`src-ui/src/graph/dispatch.ts:685`）、兄弟/到顶 `上游测试未钉住`（`672`、`852`）且 `closeWaitingAcceptance` 加 reason 入参（`716`，唯一调用点 `852` 已传）；术语表 `docs/MANUAL.md:193-194`、`204`；设置页 `SettingsModal.tsx:654`。测试 `review-defaults-12.test.ts:452-476`、`490-498`，旧实现下 `469/470/474/475` 会红；非 unpinned 分支仍被 `195-231` 钉住。

**4. 已修。** 恢复失败单独成句（`2862-2873`，note 不含「恢复后验证」字样）；只有恢复成功且绿跑两次都失败才 `validation_failed`（`3127-3134`，note「红绿核对恢复后验证连续 N 次未通过」）。

**5. 已修。** 红跑超时/启动失败在跑绿之前返回（`3051-3067`），绿跑超时/启动失败各自成句（`3111-3125`）先于 Failed 判定（`3127`），红超时不再遮蔽绿失败（红超时已不跑绿）。测试 `9085-9099`、`9210-9220`。口径差异：绿跑启动失败归入 skipped（`3120-3125`），比清单字面 `green.is_err() && !green_timed_out → validation_failed` 更宽，但与第 13 条同口径，可接受。

**6. 已修。** 仅当第一次绿跑 `Failed` 且未取消才重跑（`3079-3084`），两次都失败才 `validation_failed`；note 写次数（`3130`、`3139`、`3146`），输出分「恢复修复后，第 1/2 次」（`3105-3110`）。测试 `9191-9207`（旧实现单次失败即 `validation_failed`，`pinned==Pinned` 断言会红）、`9102-9114`。

**7. 已修。** 独立超时 `min(节点超时,300s)`（`2885-2888`，使用点 `3993`）；进核对前 `is_cancelled`（`3999-4004`）、红跑后（`3042-3050`）、第二次绿跑前（`3080`）；红/绿验证子进程带节点取消键（`3037`、`3079`、`3081`）。测试 `9239-9249`、`9252-9282`。副作用见 A（同一取消键也加到了派发后验证门，且门之后无 `is_cancelled` 兜底）。

**8. 已修。** 目录进快照并在 `.created` 记 `dir:`（`created_snapshot_directories` `2251-2264`、写入 `2345-2348`），回退只删清单里的目录（`2483-2493`），旧的「清空空父目录」逻辑已不存在。测试 `9146-9159` 能红：旧实现回退会删掉既存空目录 `keep-empty` → 红跑 `test -d keep-empty` 失败 → 结论会是 `pinned`，断言 `Unpinned` 不成立。

**9. 已修。** 零改动判断排在 `before_dir` 判空之前（`2976-2981`），note「本轮没有文件改动，未做核对」+ `pinned=skipped`；测试 `9137-9143`（旧实现报「缺少派发前快照」，会红）。测试缺 `require_git` 保护（见 G）。

**10. 已修（覆盖有限）。** 恢复路径先删链接/联接点再按快照重建（`2905-2912`），无快照条目时也先判链接（`2951-2958`），失败进清单；回退路径同策略（`2459-2462`）。测试 `9408-9422` 仅 `#[cfg(unix)]` 且直接调 `restore_snapshot_paths`，未走 `red_green_check`。另见 B。

**11. 已修。** `DispatchOverlay.tsx:216` 的接线断言（测试 `441`）；三态/说明抽到 `DispatchBits.tsx:26-32`，用 `renderToStaticMarkup` 断言三态、类名、警告色（`490-496`）；引擎负例 `task13_trigger_conditions_have_engine_level_negative_cases`（`9364-9381`）覆盖 `fix_round=false`／空验证命令／非执行角色 → `pinned=None` 且 frontmatter 无 `pinned:`。

**12. 已修。** `DispatchRecord.pinned` 为 `Option` + `skip_serializing_if`（`420-421`），旧记录 `pinned: ''` 读作 `None`、非法值反序列化报错（测试 `9127-9133`）；`[红绿核对]` 登记术语表（`MANUAL.md:193`）；测试文件头补 TASK-13（`review-defaults-12.test.ts:2`）。

**13. 已修。** `ValidationVerdict::LaunchFailed`（`2748-2753`、`2780`、`2792-2799`），红阶段启动失败 → skipped+原因（`3060-3067`），测试 `9222-9233`。Windows 判定依赖输出里的英文标识 `2792-2794`（见 I）。

**14. 已修。** `run_validation` 返回 `ValidationRun{verdict,output}`（`2755-2802`），红绿判定只比枚举（`3051`、`3060`、`3080`、`3111`、`3119`、`3127`、`3136`）；「验证命令超时」文本只剩展示（`2789`）与测试断言（`9247`）。

**15. 已修。** 快照记 `mode`（`1994-1995`、`2210-2216`），恢复/回退重建后 `set_permissions`（`2944-2948`、`2471-2478`）；`pinned` 三值枚举（`259-265`）+ 拒绝未知值（`271-277`）。对应测试 `#[cfg(unix)]`（`9390-9405`），本环境未执行。

## 新发现的问题

**A（中低）｜叫停撞上「派发后验证门」被报成验证未通过，可能多派一轮。** `src/dispatch.rs:3931` 现在把节点取消键传给验证门；进程被杀 → 门失败即 `status="validation_failed"`（`3932-3937`），之后没有任何 `is_cancelled` 兜底（对比 `3762` 只在进通道前兜底），本轮取消标记又在 `3370` 被清掉。复现：执行角色+验证命令较长的节点，在门运行期间点「叫停」→ 结果卡「验证命令未通过」；前端把 `validation_failed` 计入 `fixRoundLimit` 并可能生成修正节点（`src-ui/src/graph/dispatch.ts:841-853`），「停」被翻成「再来一轮」。建议：门之后加 `is_cancelled → error("派发已取消")`，或门恢复不传取消键。

**B（中低）｜既有文件被替换成链接后，恢复阶段会删掉它且不报失败。** 快照遍历跳过链接（`2159-2161`），该路径在 after 无条目但在 changed 里（before 有/after 无 → `2237-2249`）；恢复走 `None` 分支删除（`2951-2958`）。复现：修正轮把受管文件 `f.txt` 换成指向 `g.txt` 的符号链接 → 红阶段回退把 `f.txt` 按 before 恢复（`2459-2462` 先删链接）→ 绿阶段恢复按 after 找不到条目 → 删除 `f.txt`；失败清单为空、结论照常计算，工作区少了 `f.txt`。建议：把「before 有、after 无、但路径存在」判为恢复失败并入 `failures`，或让快照记录链接条目。

**C（低）｜标记文件自身失败路径的两处不一致。** ①`3070-3078`：green 阶段写标记失败即以 error 返回，工作区停在回退态却不再尽力写回 after，note 也没说改动已被回退。②`3089-3096`：清标记失败也置 error，而此刻工作区已恢复完毕——把记账失败报成节点失败。③`3039-3041`、`3086-3088`（restoration_error）与上述两处都留下标记，之后对账会把一次「已结束但失败」的运行说成「上次红绿核对被打断」（`1596-1611`，标记无自动清理）。

**D（低）｜新 error 状态带来错误归因行。** error 命中 `4050-4056` 的匹配 → `fallback_skipped="可写节点"` → 结果卡显示「主失败，未切备选（可写节点）」（`src-ui/src/graph/dispatch.ts:258-263`、`DispatchOverlay.tsx:767`）。「主」并未失败，失败的是红绿恢复。

**E（低）｜对账说明进不了界面、`recordPath` 语义错位。** unknown 在 `dispatch.ts:999` 固定写 `RECONCILE_UNKNOWN_TEXT`，节点卡/工具条只显示「待核对」与计数（`dispatch.ts:1021-1023`、`1078-1083`），`ReconcileNode.error` 无渲染点；且该分支把 `record_path` 指向 `.redgreen` JSON（`dispatch.rs:1607`），与「派发记录路径」字段语义不符。

**F（低）｜`.created` 的 `dir:` 项计入 `RollbackSummary.removed`**（`2483-2493`、note `2520`「已回滚上次改动 N 文件」），TASK-06/TASK-14 共用该函数，目录被当文件计数（纯文案）。

**G（低）｜测试卫生。** `task13_zero_change_reports_its_real_reason`（`9137-9143`）经 `task13_repo` 调 `test_git`（`4787-4791` 断言 git 成功）却没有 `require_git` 保护，无 git 环境会失败而非跳过（同文件其它 task13 用例都有保护）。

**H（info）｜** `.redgreen` 固定写 `project/.dispatch/`（`4014-4021`），即使 `write_project_dispatch=false`（此时 `log_dir` 在运行目录，`1802-1806`）也会在项目里建 `.dispatch/<run>/`；清单字面指定该路径，若「不写项目目录」是硬约束需另定路径（reconcile 也按此路径查）。

**I（info）｜** 第 13 条的 Windows「启动失败」判定靠英文标识（`2792-2794`）；PowerShell 对「命令不存在」与「脚本失败」退出码都是 1，换 shell 或文本形态变化会退回 `Failed` → 假 pinned 风险回归；GLM 原始复现的「spawn 失败」变体无测试。

**J（info）｜** worktree 节点的对账文案说「工作区可能停在回退状态」但只给 `before_dir`，未指出受影响的是 worktree（`1596-1611`）。

## 核过、没问题的点

- 触发条件不变且正确（`3987-3991`），`pinned` 只在核对分支被赋值（`4029-4035`），普通节点仍缺省。
- 只动本次 diff：`changed` 用本次内存快照差集（`2237-2249`、`2975`），路径安全+父目录 canonical 前缀校验（`2900-2936`），worktree 内在 workdir 上核对（`4012-4015`）。
- 时序：回退→红跑→恢复→绿跑→再恢复（`3023→3037→3038→3079→3085`），红/绿各自跑完立即恢复，超时/启动失败/取消分支都在恢复之后。
- 锁：单写者沿用整轮项目写锁（`3667-3674`，作用域覆盖核对）；worktree 在 workdir 取锁、取不到则 skipped 不改状态（`3994-4010`）；取消中的取锁立即失败（`2096-2100`），因取消分支在前（`3999`）报「已取消」。
- 记录契约：`pinned` 三值枚举+小写 serde+旧值兼容（`259-277`、`420-421`），`DispatchNodeResult` 同（`239-242`），前端镜像一致（`src-ui/src/tauri.ts:496/541-542`、`graph/dispatch.ts:32-33`）。
- 前端：三态标签与警告色（`pinnedLabelOf` `265-270`、`GraphTab.css:66-68`）、unpinned 计入上限并生成修正轮（`841-861`）、结论随 Reply 下发（`281-291`，测试 `478-489`）。
- `.dispatch` 被快照排除（`428-433`），标记文件不会进 diff/回退集合；reconcile 对标记 JSON 解析失败有兜底（`1600-1603`）。
- 测试可红性：1/2/5/6/8/9 与前端文案的负例在旧实现下会红；删掉 `DispatchOverlay.tsx:216` 会让第 11 条断言红。
- 第 15 条的 mode 与第 10 条的链接测试为 `#[cfg(unix)]`，本机（Windows）未执行，报告已声明。