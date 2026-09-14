# TASK-04 复审 R2（DeepSeek deepseek-v4-flash via pi，只读盲审，2026-09-14 08:36–08:40）

# 复审报告 · 第 4 期（R1 17 条 + R2 16 条）

核对基准：当前 `D:/work/aicross` 工作树（并回资源接入后的 main）。文件行号以当前源码为准。

---

## 一、R1 逐条核对（17 条）

| # | 判定 | 依据 |
|---|---|---|
| 1 | **已核实修好** | `context.ts:112-124,138` 盲节点把非直接父回复产出为 `defaultSelected=false` 候选；`DispatchOverlay.tsx:353` 渲染「可选上游 ·」复选框，勾选进 `selection.messageKeys`，`serializeContext:192` 保留；测试 `context.test.ts`「盲验收可在 G1 额外勾选方案回复」双向断言 |
| 2 | **已核实修好** | `dispatch.ts:185-188` 改为 `matchAll(/\b(?:PASS\|NEEDS_FIX)\b/g)` 取最后位置；测试覆盖 `all tests passed`→UNKNOWN、双词同现取最后、判定词在 1200 字符窗口外→UNKNOWN（`dispatch.test.ts:112-117`） |
| 3 | **已核实修好** | `SettingsModal.tsx:442-450` 角色页底部只读 `<pre>` 展示 `ROLE_PROMPT_TEMPLATES` 三类模板，文件由 `dispatch.ts:3-5` 的 `?raw` 提供 |
| 4 | **已核实修好** | `DispatchOverlay.tsx:258-263` 按 `stage` 归组求和，`:430` 插 `dispatch-usage-subtotal` 小计行（in/out/cache/耗时/路数） |
| 5 | **未修** | 全仓无「无写工具」「产物为空」文案（`grep` 仅命中修正单本身）；`DispatchOverlay.tsx:308-313` 的 warning 只有未指定模型/链式/同厂商三类。第二半（角色填充选可写通道）被 R2 第 2 条显式撤销：`orchestration.rs:243` 的 `fill_role_defaults(..., _models)` 已不参与判定 |
| 6 | **已核实修好** | `dispatch.rs:1122-1128` 写锁错误进 `error`，`error.is_empty()` 门控使节点保持 `status="error"`，不再无锁运行；测试 `state_uses_actual_limit_and_separates_failures` 覆盖 |
| 7 | **已核实修好** | `context.ts:112-119` 只按 `n.reply?.artifacts` 判目录交付，不看 `kind`；无 artifacts 的直接父回复回退进 prompt；测试「单父 turn 盲验收在父回复无 artifacts 时回退进 prompt」 |
| 8 | **已核实修好** | `dispatch.rs:530` `SNAPSHOT_FILE_LIMIT=8MB`，超限 `bytes=None` 只记 size/mtime（`:715-721`），diff 走 `Binary files … differ`（`:820-825`）；测试 `snapshot_marks_files_over_eight_megabytes_without_reading_content` |
| 9 | **改法有问题** | `GraphTab.tsx:174-179` 确实按 STATE 的 `runId/activeNodeIds` 逐节点 `cancelDispatchNode`，但 `process.rs:18-21` 的进程注册表是本进程内存 `HashMap`，应用重启后 `cancel_registered` 必 false——正是审查描述的「崩溃后孤儿进程」场景下无效，且未按审查给的备选加风险文案 |
| 10 | **已核实修好** | `GraphTab.css:21` 弹窗底 `var(--bg-app)`（各主题均为不透明色）；`:253-255` 按 vendor 去重后 join、不再有多余顿号；`models.length===0` 走 `:337-340` 手填 agent/model；`:78/:309/:370` 未指定模型显示「未指定模型」并禁用预览+确认。视觉仅源码核对（无窗口，无法端到端确认） |
| 11 | **已核实修好** | `dispatch.rs:1355-1361` 拒绝 `Prefix`/绝对路径/`ParentDir`；`#[cfg(windows)]` 测试 `blind_acceptance_rejects_drive_relative_changed_path` 覆盖 `C:escape.txt` |
| 12 | **已核实修好** | `dispatch.rs:1565-1578` STATE 首行写 `aicross-state: {json}`（含 `unfinished`/`active_node_ids`），`:286-302` 只按该头部恢复，不再匹配中文子串；`## 验证失败` 单列（`:1610-1616`）；测试 `inspect_state_uses_machine_header_instead_of_status_prose` |
| 13 | **已核实修好** | `dispatch.ts:116` 重建 Reply 时透传 `adopted`；测试「结果写回 Reply…保留采纳状态」 |
| 14 | **未修** | 无「待上游完成」标注（`grep` 零命中）。深层草稿节点被 `DispatchOverlay.tsx:40` 的 `dispatchableNodeIds` 直接过滤掉——原「两集合不一致」问题被 R2 第 10 条以另一种方式消掉，但报告写的「保留深层节点并标注」与实现不符 |
| 15 | **已核实修好** | `dispatch.rs:314` 抽 `is_continuing`，preview（`:338`）与 run_one（`:1102`）共用；`SettingsModal.tsx:488-493` 归类模型 input 已 `disabled` 且无 `onChange`；`orchestration.rs` 与 `dispatch.rs` 两份原子写注释互指（`dispatch.rs:1636`、`orchestration.rs:377`） |
| 16 | **已核实修好** | `dispatch.ts:355` `reducePipelineStep` 纯函数 + 4 个测试（非 ok 停止、重试上限、NEEDS_FIX 生长、超 3 轮不生长）；`artifact_diff_covers_deleted_binary_large_and_oversized_markers` 覆盖删除/二进制/>200KB |
| 17 | **已核实修好** | REPORT:13/972/739 已改为「按修正单允许的 fallback」并如实标注 harness 未过 `serializeContext`。残留：REPORT:272 仍写「任务书允许的 fallback」（措辞未同步） |

---

## 二、R2 逐条核对（16 条）

| # | 判定 | 依据 |
|---|---|---|
| 1 | **已核实修好** | `channels/mod.rs:6` 冻结提示语；`dispatch.rs:1375` 追上提示、`:1376-1379` `<30KB` 内联 `[diff.patch 全文]`；`mod.rs:241-245` 带产物 pi 盲审改 `--tools read,grep,find,ls`、无产物仍 `--no-tools`；测试 `blind_acceptance_directory_contains_only_artifact_delivery`（`dispatch.rs:2176-2178`）与 `blind_pi_with_artifacts_keeps_read_only_tools`（`mod.rs:844-852`）。注：argv 靠 prompt 含该句来判定，属隐式耦合（可读，但脆） |
| 2 | **已核实修好** | `mod.rs:146-147` claude 非盲可写 `Read,Grep,Glob,Write,Edit,Bash`；`:253-258` pi 可写 `read,write,edit,bash,grep,find,ls`；`validate.ts` 无非 codex 拦截，`SettingsModal.tsx:409-440` 执行角色候选不再限 codex；测试 `writable_claude_enables_write_tools`/`writable_pi_enables_write_tools`/`single_writer_role_uses_configured_writable_agent`。注：修正单本身把 `Bash` 写给 claude，已照做，但这两条通道没有 codex 的 `workspace-write` 沙箱，写节点等于无 OS 级约束（见新风险 5） |
| 3 | **已核实修好** | `DispatchOverlay.tsx:46` 条件放宽带 `关\|低`→「中」，`:310-311` 显示「任务含数字或链式关键词…」；内置预设未改（`orchestration.rs:222-233` 执行仍 `低`） |
| 4 | **已核实修好** | `dispatch.ts:251` `upstreams: [acceptId, previous.id]`；测试断言 `['a','e']` |
| 5 | **改法有问题（部分）** | `dispatch.rs:902-961` 用临时 `before/after` 目录 + `git diff --no-index --binary --unified=3`（`a/before/`→`a/` 重写）、非 git 回退 `fallback_diff_for_paths` + 200KB 单文件上限、kimi 侧 `crop_utf8_bytes` 裁剪（`mod.rs:311-329`）都到位，测试 `git_repository_uses_three_line_unified_diff`。但 `:884-894` 的探测在 **git 不可执行时返回 Err**，经 `:1208` 的 `?` 直接把节点判成 `error`——模型已经跑完（钱已花），本该按「否则退回现有实现」用内置 diff。另路径重写 `a/before/` 无断言 |
| 6 | **已核实修好** | `:651-658` `symlink_metadata` + Windows reparse point(0x400) 跳过；`:685-712` 排除名单补齐 `.venv build coverage .next vendor`；`:530-531` 8MB/128MB 上限；前后快照或 diff 失败 → `status="error"`（`:1074-1079`、`:1226-1229`，顺带把 qwen 2.4 的「空验收」路径也关掉，因为父节点变 failed 后不再可派）。注：总字节上限是硬失败而非降级 |
| 7 | **已核实修好** | `:398-399` 调度前 `git rev-parse --is-inside-work-tree` 探测，非 git 时 `:318-319` 把 worktree 节点按单写者记账，`:427-443` 参与排队；`:1066` 拿到锁后才拍 before 快照 |
| 8 | **已核实修好** | `:601-648` 规范化项目路径 → `Mutex<ProjectLockState>{held, queue:VecDeque}` + `Condvar`，FIFO 只认队首、`timeout_secs` 截止、`is_cancelled` 命中即出队并唤醒全体；`mark_cancelled:565-574` 通知所有锁；测试 `project_write_lock_is_mutually_exclusive`、`project_write_lock_wait_can_be_cancelled` |
| 9 | **已核实修好** | `DispatchNodeRequest.stage` 逐节点（`tauri.ts:411-413`→`dispatch.rs:67-68`），run_one `:1038`、结果 `:1285`、记录 `:1244`、用量/结果屏归组（`DispatchOverlay.tsx:252-263`）都用节点自身 stage；批次 stage 仅留 STATE 摘要（`dispatch.rs:401-406`）；测试 `node_stage_is_used_for_result_and_record` |
| 10 | **已核实修好** | `DispatchOverlay.tsx:40` 首轮就是 `dispatchableNodeIds`；`:174-184` 每层重算请求并 `previewDispatch` 刷新；`:178` `withinRequestLimit` 越界即停并提示；后端二次拦截（`dispatch.rs:366-371`）；`dispatch-settings.ts:15,38` 默认 8、范围 1–100，`SettingsModal.tsx:477-486` 可改；测试 `withinRequestLimit` 边界 |
| 11 | **已核实修好** | `dispatch.ts:395` `addedCorrection \|=`；`:357-380` 逐节点 `appendValidationRetry`、重试计数按新执行节点、`count>=3` 关掉等待中的验收；`applyDispatchResults:104` 保留正文并把验证输出以 `[验证输出]` 追加；测试 3 条（多节点、逐节点计数、正文保留）。注：下一轮修正节点仍拿不到失败节点的正文（它不是新节点的祖先，且 `context.ts:56-58` 只转发 `done`），不过文件状态留在项目目录，影响有限 |
| 12 | **已核实修好** | `orchestration.rs:88-90` 与 `types.ts:25` 只追加可选 `error_kind`；`dispatch.ts:109` 写回；STATE 词表（`dispatch.rs:486`、`write_state` 把「真身不符」单列）与节点卡（`GraphTab.tsx:400-405`）分别显示；测试 `真身不符作为独立失败类型写回 Reply`。既有字段无改名/删除，兼容旧图 |
| 13 | **已核实修好** | `dispatch.ts:412` `groupResultsByStage`，栏标题取该阶段首节点 role、无名退化「第 N 阶段」（测试）；`DispatchOverlay.tsx:392` 列数按阶段数动态设置 |
| 14 | **已核实修好** | `mod.rs:1124` 样本换成真实 `[usage] provider=… ms=…` 形态；`dispatch.rs:2659` 加 `tokens_in > 0` 断言。残留：env 缺失仍 `println`+`return`，`--ignored` 下报 ok（qwen 6.2 的假通过没治，未列入本轮修正单） |
| 15 | **已核实修好** | `dispatch.rs:2588-2590` 排序后精确断言根目录只有 `REQUEST.md/changed/diff.patch`，并另断言项目 `roman.py` 不在盲目录 |
| 16 | **已核实修好** | `fixtures/prompts/{planning,execution,acceptance,blind-prefix}.txt`；TS 用 `?raw`（`dispatch.ts:3-5`、`context.ts:3`），Rust 用 `include_str!`（`dispatch.rs:2494-2497`）读同一组文件拼 harness。注：见新风险 2 |

**两项修正单的总判定**：R1 14/17 已核实修好（未修：5、14；改法有问题：9）；R2 15/16 已核实修好（改法有问题：5）。两条修正单都注明「不采纳：无」，但 R1-5、R1-14 实际没有落地，报告措辞与代码不符。

---

## 三、是否引入新缺陷

**并发 / 锁**：未发现新破口。写锁 `ProjectWriteGuard` 生命周期正确（元组绑定不提前 drop）；快照在持锁后拍摄；worktree 单点失败（`create_worktree` Err）仍会走 `uses_single_writer` 抢锁，互斥不破（只是状态可能不显示「等待写锁」）。小瑕疵：`reducePipelineStep` 在「同批既有 `validation_failed` 又有 `identity_mismatch`」时走重试分支并 `continueRunning=true`，另一路失败不阻止继续（`dispatch.ts:357-380`）。

**留痕契约**：`aicross-dispatch/1` 未变（`dispatch.rs:1242`），`DispatchRecord` 字段无增删；`Reply` 只追加 `artifacts`/`error_kind` 且 Rust/TS 两侧同步（`orchestration.rs:83-92` ↔ `types.ts:23-26`）；`DispatchNodeResult.stage` 属派发 IPC 非图契约。合规。

**第 2/3 期图与派发行为**：`stamp/mutate/layout/notation/persist/validate` 未动；`dispatchableNodeIds`、`continuationSession`、`clearTargetSession`、`adopt` 语义未变。行为变化仅两处且都是本期要求：盲节点父回复规则（`context.ts:112-124`）与盲节点负默认勾选。`replyMessage` 继续把 artifacts 摘要拼进 done 回复，会让共享上下文变长（既有行为 + kimi 30k 裁剪兜住）。

**server.rs / tauri.ts 注册**（并回资源接入后）：`preview_dispatch / dispatch_graph_stage / cancel_dispatch_node / inspect_dispatch_state / abandon_dispatch_state / load_roles / save_roles` 全部在 `server.rs:3842-3857` 注册，与 `tauri.ts:331-374` 一一对齐；`build_invocation` 的 `writable` 参数只有两处调用（`dispatch.rs:1130`、`resources.rs:1108`，冒烟传 false），无遗漏。未见破坏。

**新引入的问题（按严重度）**：
1. **（中）git 不可执行时写节点必 error**：`dispatch.rs:884-901` 探测失败经 `?` 冒泡 → `:1208` 失败 → `:1226-1229` `status="error"`。模型请求已发生却拿不到产物，且不回退内置 diff。修法：`project_is_git_work_tree(...).unwrap_or(false)` 或 match 后走回退。
2. **（中）`fixtures/prompts/*.txt` 在 Vite root 之外**：`dispatch.ts:3-5`/`context.ts:3` 是本仓首例越出 `src-ui/` 的 `?raw` 导入，`vite.config.ts` 无 `server.fs.allow`。`npm run build` 走 Rollup 无碍（已通过），但 `tauri dev` 下 `/@fs/...` 可能被 allow 列表拒绝 → 白屏。需在窗口/dev 里确认；否则加 `server.fs.allow` 或把模板移到 `src-ui` 内。
3. **（低）`materialize_snapshot` 失败不清理临时目录**：`:906-916` 的 `?` 早于 `:938` 的 `remove_dir_all`，会在 `~/.aicross/runs/<run>/diff-*/{before,after}` 留下**未经脱敏**的前后文件副本。
4. **（低）`cancelled_dispatches` 只增不减**：`continueUnfinished` 取消的旧 run key 永不清除（`:560-586`），长期运行会缓慢累积。

---

## 四、两条修正单之外、仍未闭合的审查发现（两轮未收）

- **（高）产物不过脱敏**（qwen 3.1）：`dispatch.rs:1205-1230` 只对 `stdout_summary` 过 `safe_trace`，`diff`/`changed_files[].content` 原样进 `DispatchNodeResult` → 前端结果卡 → `~/.aicross/graphs/*.json` → 经 `context.ts:artifactSummary` 外发下游厂商。与同文件「返回前端的文本都先做安全过滤」的注释立场矛盾。
- **（中）验收/验证节点可配成单写者**（qwen 3.2）：`NodePanel.tsx:204-212` 写权限对任意 role 可选，`validate.ts` 无校验，后端只看 `write != "只读"`（`dispatch.rs:1130-1145`、`mod.rs:183`）→ 验收节点会拿项目写锁、以 `workspace-write` 跑在项目目录。
- **（中）kimi 盲审无隔离开关**（qwen 3.3），且 `isolation_for` 对 kimi 恒「部分」，G1 无警示文案。
- **（中）worktree 节点仍跑验证命令**（qwen 2.11）：`dispatch.rs:1193` 在干净检出里跑 `validation_command`，几乎必 `validation_failed`。
- **（中）恢复 run 换新 run id / 放弃即删 STATE**（qwen 2.12、2.13）：`DispatchOverlay.tsx:44` 每次 `crypto.randomUUID()`，`abandon_dispatch_state:304-311` `remove_file`。
- **（中）live harness 的三处误判面**（qwen 6.1/6.2）：fixture 无测试文件名约定 + `python -m pytest`（退出码 5 风险）、出方案/执行节点硬断言 `ok`（zai 静默升级即整条红）、G4 闭环未在真跑覆盖、R2 之后未再真机跑（编排者 R1 前的 3 次请求通过，但用不上 R2 的 git diff / 盲提示 / 内联改动）。
- **（低）**：preview 的 `broad_redacted` 未按 `settings.redact` 门控（`dispatch.rs:350`，GLM 4.3）；理由行仍恒「角色默认」（`DispatchOverlay.tsx:308`，GLM 4.2）；同厂商复核在浮层与 `validate.ts` 两套实现且不比对 provider（qwen 4.1）；`stdout_summary` 仍承载模型正文并寄生 worktree 提示（qwen 2.14）。

---

## 五、结论

**一句话：第 4 期达到可交付（主干与两条修正单的实质条目已落地、四项自动验证全绿、留痕/第 2·3 期契约与注册无破坏），但建议在交付前补一个 3 条的窄修正轮：R1-5 的 G1 写通道提示、产物脱敏、git 不可用时的 diff 回退。**

**仍未闭合项（按严重度）**：
1. artifacts（diff/changed_files）未过 `redact_text/safe_trace` 即返回前端、随图落盘并外发下游（qwen 3.1）。
2. R1-5 未修：G1 缺「单写者 && agent≠codex/cc-switch 无写工具，产物为空」提示；kimi 作为执行者的写能力既无通道保证也无提示（`fill_role_defaults` 的 models 参数已成死参）。
3. R2-5 新缺陷：git 不可执行时写节点在模型跑完后判 error，不回退内置 diff。
4. R2-16 风险：`fixtures/prompts` 在 Vite root 之外，`tauri dev` 可能被 `server.fs.allow` 拦（build 已通过）。
5. 验收/验证节点仍可配成单写者并拿 `workspace-write` + 项目写锁（qwen 3.2）；kimi 盲审无隔离且无警示（qwen 3.3）。
6. R1-9 跨进程孤儿清理无效（进程内注册表），未加风险文案；恢复 run 换新 run id、放弃删 STATE.md（qwen 2.12/2.13）。
7. worktree 节点仍跑验证命令（qwen 2.11）；R1-14「待上游完成」标注未实现（改由过滤取代，报告措辞不符）。
8. R2 之后未复跑真机；live harness 的 pytest/真身容错/≤8 断言/G4 覆盖与 `live_cc_switch_route` 缺变量即报 ok（qwen 6.1/6.2）待窗口确认。
9. 低优先：`materialize` 失败残留未脱敏临时副本、`cancelled_dispatches` 只增不减、broad_redacted 未按开关门控、理由行失真、同厂商复核两套实现、同批 `identity_mismatch`+`validation_failed` 仍继续派发。

---
usage: {"input": 538, "output": 7049, "cacheRead": 197248, "cacheWrite": 0, "reasoning": 1482, "totalTokens": 204835, "cost": {"input": 7.532e-05, "output": 0.00197372, "cacheRead": 0.0005522944, "cacheWrite": 0, "total": 0.0026013343999999996}}
