# TASK-13 红绿核对 审查报告

**总判：可交付但有未闭合项** —— 契约、触发条件、回退/恢复主路径、判定语义、流水线与界面均符合任务书与设计裁决，测试真实可红；但恢复失败/进程被杀两条失败路径存在半回退遗留与文案错位，未闭合。

（说明：本环境无 shell 工具，无法跑 `git diff`，以下基于工作区当前文件内容与 TASK-13 标注逐段核对。）

## 问题清单（按严重度）

1. **src/dispatch.rs:2798-2816 —— 恢复操作失败被标成「验证未通过」，且可能留下半回退目录。**
   问题：red 跑完后 `restore_snapshot_paths` 失败（2798-2806）或最终恢复失败（2808-2816）时，返回 `pinned: "skipped"` + note「红绿核对恢复后验证未通过」+ `validation_failed: true`，调用方据此把 status 改为 `validation_failed`（3729-3732）。此时验证命令根本没跑（第一处）或已跑完（第二处），树处于「部分文件=修复后、部分文件=回退后」的混合态；下一轮修正节点会在混合树上继续写。note 与事实不符（`validation_output` 里是恢复报错，不是测试输出）。
   复现：回退/恢复期间某个 diff 文件被占用（Windows 文件锁、杀软扫描）或变成链接（2716 显式报错）；`restore_snapshot_paths` 按序写、首错即返，其后路径不恢复。
   建议：恢复失败单列 note（如「红绿核对恢复改动失败」），status 用 `error` 而非 `validation_failed`（树已不可信，不该进修正轮计数）；对 `changed` 全集再补一轮完整恢复；2793 的 `let _ = restore_snapshot_paths(...)` 丢弃了二次恢复错误，至少并入 note。

2. **src/dispatch.rs:2782-2808 —— 进程在核对中途被杀会静默留下回退态，无落盘标记、无自动恢复。**
   问题：回退完成到最终恢复之间引擎被杀/退出，workdir（单写者=项目目录）停在修复前状态；持久化的 before-dir 只存 before 字节与 `.created` 清单（2248-2274），不存 after 内容，修复内容只能靠留痕里的 diff/changed_files 手工找回。后续任一节点把该状态当作自己的 before 快照，误差继续传播。
   复现：核对期间杀进程（恰好落在 red 验证运行窗口）。
   建议：回退前在 before_dir 写进行中标记、最终恢复后清除；同节点再次派发时检测到标记即从留痕 changed_files 修复；至少在文档/报告里写明手工恢复路径。

3. **src/dispatch.rs:2833-2840 —— 恢复后验证失败把已通过的修复轮改判 `validation_failed`，flaky 命令会制造虚假修正轮。**
   问题：green 失败（含 flaky、环境性失败）→ status 改 `validation_failed`、计入 `fixRoundLimit`，尽管首次验证已通过且改动已完整恢复在树上。同样，red 因环境原因偶现失败会给出假 `pinned`。命令不确定时没有任何重跑阻尼。字面上符合任务书「恢复后失败 → 按 validation_failed 处理」，但 flaky 表现应在报告/文档里如实记。
   复现：验证命令偶发失败（端口占用、时序依赖测试）。
   建议：green 失败先原样重跑一次再定论；或 note 里注明「可能与命令不稳定有关」。

4. **src/dispatch.rs:2797 + 2841-2854 —— red 阶段验证命令「启动失败」会被当成「测试通过」判出假 pinned。**
   问题：`run_validation` 的 Err 除超时/非零退出外还有进程启动失败（2662 `?` 透传）；red=Err(启动失败)、green=Ok 时，green 非错、red 非 Ok，落到 2849 返回 `pinned`——启动失败不是测试红的证据。
   复现：red 运行时 powershell.exe/sh 暂时无法 spawn（资源耗尽），green 恢复正常。
   建议：非超时的进程级 Err 一律按 `skipped` 处理，只有非零退出码算红。

5. **src/dispatch.rs:2797-2807 —— red 超时后仍会跑 green，白等一个完整超时。**
   问题：判 skipped 所需的信息在 red 超时即已确定，但 green 照跑（再花最多 timeout 秒）并多做一轮恢复。
   建议：red 超时则恢复后直接返回 skipped。

6. **src/dispatch.rs:2228-2229 + 2751-2756 —— 修正轮零改动时 note 误导。**
   问题：本次无改动 → 持久化返回空路径 → `before_dir` 为空 → note「缺少派发前快照，未做红绿核对」，结果卡显示「未核对：缺少派发前快照」，与真实原因（无改动）不符。
   建议：区分「本次无改动，未核对」。

7. **src/dispatch.rs:2818-2819 —— 超时判定靠错误字符串前缀「验证命令超时」。**
   问题：与 `run_validation` 的报错文案（2670）字符串耦合，改文案会静默把超时滑进 green 失败/pinned 分支。
   建议：返回结构化标记（如 `RedGreenOutcome` 前先返回 `(verdict, output)`）而非前缀匹配。

8. **src-ui/src/graph/dispatch.ts:684, 676 —— unpinned 触发的修正轮，验收/验证兄弟文案说「验证命令未通过」，与事实相反。**
   问题：unpinned 时验证命令是通过的，但等待中的验收节点被标 `验证命令未通过，未送审`、验证兄弟标 `上游验证未通过`，与结果卡的「测试未钉住」矛盾。
   复现：修正轮节点返回 `pinned: 'unpinned'` 且未到上限，看图上验收节点文案。
   建议：按 `unpinned` 参数分叉文案（如「测试未钉住，未送审」「上游测试未钉住」）。

9. **src/dispatch.rs:2662 —— 核对期间的验证命令不接受「叫停」。**
   问题：`run_validation` 传 cancel_key 为 None，红/绿两跑无法被节点的叫停打断（只能等超时）；且 3745-3746 的 `duration_ms` 把核对耗时计入节点时长，观察口径未区分。
   建议：低优先；至少在报告里注明。

10. **src/dispatch.rs:2728 + 2152 —— 恢复重建的新文件不保留权限位（非 Windows）。**
    问题：快照结构只有 bytes/size/modified_ns；新建文件经回退→恢复重建后丢失可执行位，可能导致 green 假失败。当前主平台 Windows 不受影响。
    建议：记录为已知限制或快照补 mode 字段。

11. **测试缺口（info）。** 回退失败→skipped（`.skipped` 非空路径 2789）、恢复失败→validation_failed 两分支无测试；引擎级无「验证命令为空 / 角色≠执行 / write 不在集合内→不核对」的反例。前端 review-defaults-12.test.ts:482-484 用 `?raw` 源码字符串断言界面接线，属弱断言（沿用了本文件 TASK-12 的既有模式，非新增坏味）；`pinnedLabelOf` 三态断言（478-481）是同义反复式的映射核对，但起到了钉住文案的作用。

12. **info —— `pinned` 在 Rust 侧是自由 `String`**（240），不是三值枚举；拼错值会原样过网。前端是字面量联合类型，风险有限。

## 核过没问题的要点

- **契约只追加、旧数据可读**：`DispatchNodeResult.pinned/pinned_note` 为 `Option` + `serde(default)` + `skip_serializing_if`（src/dispatch.rs:238-242）；`DispatchNodeRequest.fix_round` `serde(default)`（135-137），旧引擎不认识该字段也不会拒收（无 deny_unknown_fields）；`DispatchRecord` 容器级 `#[serde(default)]` + `pinned` 字段 default（343-399），旧 frontmatter 缺 `pinned` 读为空；`serialize_frontmatter` 全量 serde_yaml 输出（4246-4249）。旧引擎→新前端 `pinned` 缺省 → `pinnedLabelOf` 返回 null，不出标签。
- **触发条件与任务书逐条一致**（3692-3696）：status==ok（首次验证通过，3640 验证失败会先翻状态）、`fix_round`、角色「执行」、write∈{单写者，worktree}、验证命令非空；worktree 修正轮节点首次验证在 worktree 内跑（2678-2681 排除 fix_round，3633-3644），普通 worktree 节点保留「合并后再验」。前端只在 `isFixRoundExecution` 命中的两种修正节点（`[修正 n/m]`、`根据上一轮验收意见修正实现。`）上置 `fixRound: true`（dispatch.ts:1145-1150、1187；DispatchOverlay.tsx:217），swarm/普通执行不带。
- **回退用本次尝试的快照**：before/after 内存对按本次运行捕获（3648-3660），`before_dir` 逐 run 持久化后显式传入 `red_green_check`（3710-3718）；历史快照查找 `latest_before_dir` 只用于重派回滚，不进红绿核对。
- **只碰 diff 文件**：持久化 before-dir 只含 changed 路径的 before 字节 + `.created`/`.skipped` 清单（2219-2289）；回退只写这些文件并按 `.created` 删新建文件（2344-2402），另对不在 before 的新文件补删并清空父目录（2784-2786）；恢复把 changed 路径写回 after 字节、对 after 中不存在的新建文件删除（2705-2739），有链接/越界防护（2714-2716、2724-2726）。快照缺失/不存在/带 `.skipped` 标记/改动含未保存大文件 → 一律 skipped 且不改 status（2751-2780）。
- **写锁**：单写者沿用在函数顶部取得、作用域覆盖整个节点（含核对）的项目写锁（3372-3379，锁失败→节点 error→不核对）；worktree 在核对前对 worktree 目录取节点锁（3698-3708），取不到→skipped；worktree 不可用退回单写者时自然落到项目锁（3353、3364-3366）。
- **worktree 内做**：回退/恢复目标均为 workdir（worktree 路径），引擎级测试断言项目目录 `function.txt` 保持 `1\n`、worktree 内含修复与测试（8823-8841）。
- **判定语义**：red 失败+green 通过→pinned；red 通过→unpinned（note 与任务书原文一致）；green 失败→`validation_failed`+「红绿核对恢复后验证未通过」；超时→skipped 且树已恢复（测试断言逐字节）；恢复后验证输出以「[回退修复后]/[恢复修复后]」合并进 `validation_output`，结果卡「验证结果」可查（783）。
- **流水线**：`unpinned` 与 `validation_failed` 同列计入上限（dispatch.ts:841-843），重试计数经 `validationRetries` 正确穿线（1419、1451-1452）；到上限 `closeWaitingAcceptance` 关验收并跳过验证兄弟（715-732）；验收 NEEDS_FIX 的遗留节点路径不受影响；修正提示词以「上一轮修复回退后测试仍通过，请补能失败的测试」收尾（691）；红绿结论以 `[红绿核对]` 进 Reply（dispatch.ts:281-290），节点卡渲染该文本（GraphTab.tsx:688），下游验收上下文经 context.ts:64-66 收到。
- **界面**：三态标签「测试已钉住/测试未钉住/未核对」在状态旁（DispatchOverlay.tsx:758-765），`unpinned` 用 `var(--warning, #c18401)`（GraphTab.css:66-68，与仓内既有 fallback 写法一致），仅 unpinned 显示 note（773）；文案为中性状态词，与设计裁决原文一致，无新造词。
- **测试可红**：Rust 9 个 task13 测试全部落在真实临时 git 仓库与真实命令上（pinned/unpinned/缺快照/逐字节恢复/超时/green 失败/契约缺省/引擎单写者/引擎 worktree），断言绑定具体结论值与 note 原文，逻辑破坏时会红；前端 4 个测试走真实函数（buildNodeRequest 序列化、reducePipelineStep 两种修正节点、上限计数与提示词收尾、Reply 注入），非同义反复。报告所列验证命令与产物口径齐备（LF 复核、未 commit/push）。