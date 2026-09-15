## 总判

**可交付但有未闭合项**：契约（追加字段、serde default、旧记录可读）、触发条件、回退/恢复的主路径（只用本次快照、只碰本次 diff、字节级还原、worktree 内做）、判定语义与流水线接线基本按任务书落地，测试是真跑临时 git 仓库、能红；但回退失败的**半回退状态**在代码里被 `let _ =` 静默吞掉且节点仍报 ok（唯一实质正确性缺口），另有 unpinned 复用「验证命令未通过」文案、恢复失败与恢复后验证失败共用一句话、超时分支遮蔽恢复后失败等语义/文案未闭合项。

> 审查方式：仅静态阅读（本环境无 shell），未运行 `cargo check/test`、`vitest`、`git status/diff`；报告中的测试数字与「未 commit」等流程项未复核。工作区 diff 边界由任务书推断。

---

## 问题清单（按严重度）

### 1. 中｜回退失败后重放本次改动的结果被丢弃，可能留下半回退状态且节点仍是 `ok`
- 位置：`src/dispatch.rs:2792-2795`（`if let Err(note) = rollback { let _ = restore_snapshot_paths(workdir, after, &changed); ... }`），配合 `src/dispatch.rs:2705-2740`（`restore_snapshot_paths` 内任何一处失败即 `?` 立即返回，**后续路径一律不再处理**）。
- 问题：`restore_before_dir` 是逐文件执行、运行时失败会进 `summary.skipped`（写失败、快照目录内文件读失败、路径不安全、链接、父目录 canonical 失效），此时它已经把**排在前面的**文件回退掉了；随后这次「重放」的结果被 `let _ =` 吞掉。若重放又在某个路径上失败（只读文件被 `fs::write` 拒绝、链接、父目录校验失败），排在它后面的文件就永久停在回退后的内容上，而 `status` 仍是 `ok`、`pinned` 是 `skipped`、note 只说「…未做红绿核对」。
- 复现条件：修正轮让某文件变为只读（模型自己 `attrib +R` / chmod 0444）或让某 changed 路径在核对期间变成链接/联接点，再让另一个 changed 路径排在它之后（`changed` 已排序）→ 回退部分生效、重放中断、无任何提示。
  （说明：*静态*快照不完整——如 >8MB 的 changed 文件进 `.skipped`——已在 `2774-2778` 提前拦掉，不走这条路；本问题需要运行时失败。）
- 建议改法：`restore_snapshot_paths` 的返回值必须判定：失败时把原因写进 `note`/`error` 并（至少）把节点标成非 ok（或在前端表现为「需人工核对」）；`restore_snapshot_paths` 内部改为**收集**失败路径继续处理其余路径（best-effort 全量重放），再把失败清单返回，避免一个坏路径截断整轮恢复。

### 2. 中｜核对窗口内进程被杀 → 项目目录停在回退状态且不留任何记录
- 位置：`src/dispatch.rs:2782-2816`（回退→恢复之间），留痕写在 `src/dispatch.rs:3750` 之后（`write_record` 在 `3820` 附近）。
- 问题：核对把工作树临时回退（单写者是**项目目录本身**），在此之前没有任何留痕标记。核对两次跑命令，各用一次派发超时（默认 600s，见 `2797/2807` 传入的 `timeout`），即窗口最长 ~20 分钟。此间杀进程 / 断电 → 目录停在回退内容，派发记录没写、`STATE.md` 仍是「运行中」，TASK-06 对账只能给 unknown/待核对，没人知道目录被动过，也没有指向 `.dispatch/<run>/<node>.before` 的记录。
- 复现条件：单写者修正轮，验证命令跑得久（或注入 `sleep`），在红/绿之间 `kill` aicross。
- 建议改法：核对前先落一个「核对中」标记（记录或 `.dispatch/<run>/<node>.redgreen` 状态文件，含 `before_dir` 与 changed 清单），崩溃对账时提示「上次核对被打断，目录可能停在回退状态」并给出恢复入口；或把核对放到拷贝目录（引擎已有盲目录拷贝机制）而不就地回退。

### 3. 中｜unpinned 复用「验证命令未通过」文案，结果卡/节点卡把「测试未钉住」说成验证失败
- 位置：`src-ui/src/graph/dispatch.ts:684`（等待中的验收被写 `'验证命令未通过，未送审'`）、`dispatch.ts:676` 与 `:728`（验证兄弟/到顶时 `skipped_reason: '上游验证未通过'`）、分流处 `dispatch.ts:842`（`status === 'validation_failed' || pinned === 'unpinned'`）。
- 问题：unpinned 时验证命令**本身是通过的**（回退后仍通过才是 unpinned），但等待验收的卡写成「验证命令未通过，未送审」、`nodeStatusLabel` 落成「失败」，同批验证兄弟与到顶时的验收写「上游验证未通过」。`docs/MANUAL.md:202` 把「上游验证未通过」定义为验证未通过这一情形（术语表行也在 `docs/MANUAL.md:193/202/203`），这里被用在验证通过的情形上，属 AGENTS.md「说明句 = 事实 + 后果」的偏差。
- 复现条件：修正轮 unpinned（本轮改动不含回归测试）→ 结果屏出现「验证命令未通过，未送审」；到上限时验收标「已跳过：上游验证未通过」。
- 建议改法：`appendValidationRetry` 已收到 `unpinned` 标志，把两句改为条件文案（`测试未钉住，未送审` / `上游测试未钉住`），`closeWaitingAcceptance` 增加原因入参；同步改 `src-ui/src/graph/__tests__/review-defaults-12.test.ts:467` 的断言（该测试目前把错误文案钉住了）。

### 4. 中低｜恢复失败与「恢复后验证失败」共用一句 note，且恢复失败被当成验证失败
- 位置：`src/dispatch.rs:2799-2806`（重放 `after` 失败，**绿跑从未执行**）与 `src/dispatch.rs:2808-2816`、`2833-2839`，三处 note 都是「红绿核对恢复后验证未通过」，真正原因只落在 `validation_output`（IO 错误文本）。
- 问题：`validation_failed: true` 会让 `status` 变 `validation_failed`（`3729-3732`），前端显示「验证命令未通过」并触发一轮修正；事实是写文件失败、目录可能已不完整（与第 1 条同源）。用户看到的原因与处置方向都是错的。
- 复现条件：changed 中任一文件在恢复时不可写（被占用/只读）→ 结果卡「验证命令未通过 未核对」，note 说验证没过，而验证压根没跑。
- 建议改法：恢复失败单独一句（如「核对恢复本次改动失败：<原因>」），并把 IO 原因并入 `note` 或 `runner_note`；是否需要 `validation_failed` 与是否需要重派应分开判断。

### 5. 中低｜超时分支排在「恢复后失败」之前，红跑超时 + 绿跑失败时失败结论被丢掉
- 位置：`src/dispatch.rs:2818-2828`（`if red_timed_out || green_timed_out { skipped, validation_failed: false }`）先于 `2833`（`green.is_err()`）。
- 问题：绿跑**断言失败**（非超时）而红跑超时时，返回 `skipped`、`status` 保持 `ok`，管线继续送验收——而此刻工作树上的状态并不通过验证命令。
- 复现条件：验证命令在被回退的树上挂住（超时），同一命令在恢复后的树上失败（如测试非幂等/写入仓库导致第二次跑失败）→ 结论是「未核对」而非「恢复后验证未通过」。
- 建议改法：先判 `green.is_err() && !green_timed_out` → `validation_failed`；绿跑超时单独成句（「恢复后验证命令超时，未核对」）。

### 6. 中低｜flaky/非幂等验证命令：核对把同一命令从 1 次变 3 次，且绿跑单次失败即判 `validation_failed`（连带消耗修正轮额度）
- 位置：`src/dispatch.rs:3638-3656`（验证门第一次跑）、`2797`（红跑）、`2807`（绿跑），判定 `2818-2851` 无任何重试。
- 问题：任务书确实规定「恢复后失败 → validation_failed」，但绿跑是一次采样；测试套件不幂等（快照测试、会写 fixture/lockfile）或本身 flaky 时，一次偶发失败就把通过验证门的节点打成 `validation_failed`，进而在 `dispatch.ts:842` 计入 `fixRoundLimit` 并被重派；反之红跑偶发失败会被判成 `pinned`（假「已钉住」）。界面与 note 都不提「跑了一次」。
- 复现条件：验证命令为 `cargo test` 之类、其中有依赖执行顺序或写盘的用例；连续两轮修正轮即可观察到随机翻转。
- 建议改法：绿跑失败时按同一命令重跑一次再定论（或在 `pinned_note` 里注明「恢复后验证跑了一次」）；至少给个显式的复核入口，避免一次采样直接改 status。

### 7. 低｜核对额外占用最多 2×超时，期间持项目写锁且叫停无效
- 位置：`src/dispatch.rs:3697-3734`（核对在 `_write_guard` 生命周期内，`3372-3388` 取得），`2797/2807` 使用同一个 `settings.timeout_secs`，`run_validation`（`2667-2690`）最后一个参数 `registry_key = None`。
- 问题：单写者修正轮在模型结束后还要持项目写锁跑两次验证（默认每次最长 600s）；同项目其它单写者节点一直排队。用户点叫停不会中止已开始的核对（`is_cancelled` 只在 `3337` 与派发前检查过）。
- 复现条件：验证命令每次跑 ~9 分钟 + 默认超时 → 节点额外占锁 ~18 分钟，期间叫停无反应。
- 建议改法：核对用独立的、更短的超时（如 `min(timeout, 5min)`，可配），并在进入核对前检查 `is_cancelled(&cancel_key)`，已取消则直接 `skipped`。

### 8. 低｜回退/恢复会删掉「核对前就存在的空目录」
- 位置：`src/dispatch.rs:2784-2788`（回退后对新文件路径调 `remove_empty_snapshot_parents`）、`2691-2697`（实现）、`2730-2737`（恢复时 `None` 分支同样调用）。
- 问题：快照只记文件，无法区分「原本空目录」与「本次新建出的目录」。本次新建 `a/b/new.txt` 且 `a/b/` 原本就是空目录时，回退会把 `a/b` 删掉，恢复时靠 `fs::write` 重建路径——文件字节相等仍成立（测试按文件字节比，测不出），但目录结构不再等于核对前。
- 复现条件：仓库里存在空的 `build-keep/`（未被 `excluded_snapshot_directory` 排除）之类目录，修正轮在里面新建文件 → 核对后该空目录消失。
- 建议改法：把快照扩展出目录集合（或 `remove_empty_snapshot_parents` 只删本次确实新建出来的目录，例如在 `.created` 里连带记录新建目录）。

### 9. 低｜「本次改动为 0 个文件」被报成「缺少派发前快照」
- 位置：`src/dispatch.rs:2751-2756`，成因在 `src/dispatch.rs:2227-2231`（`changed` 为空时 `persist_before_snapshot_with_note` 返回空路径）。
- 问题：修正轮没改任何文件（回退后当然仍通过，语义上接近 `unpinned`）时，note 说「缺少派发前快照，未做红绿核对」，与事实不符，用户/下游也无法区分这两种情况。
- 建议改法：`before_dir` 为空且 `changed` 为空时单独一句（如「本轮没有文件改动，未做核对」）。

### 10. 低｜恢复路径与回退路径对链接的处理不一致
- 位置：`src/dispatch.rs:2713-2717`（changed 路径是符号链接/联接点时整轮 `restore_snapshot_paths` 直接返回 Err）对比 `2344-2382`（`restore_before_dir` 对同类路径是「删掉再写」）。
- 问题：同一障碍在回退时被容忍、在恢复时被当成致命错误，于是恢复更容易整体失败（放大第 1、4 条）。
- 建议改法：两条路径统一策略（删除链接后重建 + 记账，或都判为 skipped 并保持目录不变）。

### 11. 低｜测试覆盖缺口：DispatchOverlay 的接线没有断言；结果卡是源码字符串包含
- 位置：`src-ui/src/components/center/graph/DispatchOverlay.tsx:217`（`fixRound: isFixRoundExecution(sourceGraph, id)`）；测试见 `src-ui/src/graph/__tests__/review-defaults-12.test.ts:421-446`（只测 `buildNodeRequest` 与 `isFixRoundExecution` 两个纯函数）与 `:482-484`（`toContain('pinnedLabelOf(result.pinned)')` 等三条源码字符串断言）。
- 问题：删掉 `DispatchOverlay.tsx:217` 那一行，全部测试仍然全绿，但引擎永远收不到 `fixRound`，整个 TASK-13 静默失效（典型「测试不红」）。结果卡三条断言是「源码里有这几个字」型同义反复：把标签渲染到错误的状态上、或把 note 行删掉换成别处渲染，断言依旧过。
- 建议改法：按本仓库既有做法补 `expect(dispatchOverlaySource).toContain('fixRound: isFixRoundExecution(sourceGraph, id)')`；若要真断言渲染，照 TASK-05B R2-5 的先例把结果卡头部抽到 `DispatchBits.tsx` 用 `renderToStaticMarkup` 断言三态标签/警告类名/note。

### 12. 低｜触发条件的负例无测试
- 位置：`src/dispatch.rs:3692-3696`（触发条件），正例只有 `8813-8836` 两条（单写者 unpinned、worktree pinned）；未覆盖：`fix_round=false` 的执行节点 `pinned` 应为 `None`、验证命令为空不触发、角色非执行不触发、worktree 建失败退回单写者、写锁未取得（`3697-3706`）。
- 建议改法：至少补两条：普通执行节点（复用 `task13_engine_run` 把 `fix_round` 置 false）断言 `result.pinned.is_none()`；`validation_command` 留空 + `fix_round=true` 断言不触发且 `frontmatter` 无 `pinned:` 值。

### 13. 低｜文案/术语表未随新增用户可见词更新
- 位置：`docs/MANUAL.md:188-206`（术语表）没有「测试已钉住 / 测试未钉住 / 未核对」「红绿核对」行；AGENTS.md:39/52 要求新增用户可见字符串前先读规范、术语表按「概念 → 唯一用词 → 出现位置」维护。
- `src-ui/src/components/common/SettingsModal.tsx:654` 仍写「验证命令未通过的重派与修正轮共用此上限；验证未通过到顶时该验收标「已跳过」」，未提「测试未钉住」也计入同一上限并同样标已跳过（AGENTS.md:48 说明句=事实+后果）；对应断言在 `review-defaults-12.test.ts:380-383`。
- `src-ui/src/graph/dispatch.ts:290` 往 Reply 正文追加 `[红绿核对]` 标题——这句会出现在节点卡、并随上下文进下游验收材料（任务书只要求三态标签与结论）。若保留，应登记进术语表；否则直接只写三态。
- `src/.../dispatch.rs:3794` 的 `pinned` 没加 `skip_serializing_if = "String::is_empty"`（相邻的 `merge_result` 有），于是每条派发记录都多出一行 `pinned: ''`；不影响兼容，但记录噪声增加。
- `src-ui/src/graph/__tests__/review-defaults-12.test.ts:1-3` 的文件头注释仍只写 TASK-12 的契约清单，TASK-13 的四条用例挂在其后（是否新建文件看仓库惯例，头注释应补）。

---

## 核过、没问题的要点

1. **契约只追加**：`DispatchNodeRequest.fix_round`（`src/dispatch.rs:135-139`，结构体 `rename_all = "camelCase"` → `fixRound`，`#[serde(default)]`）；`DispatchNodeResult.pinned/pinned_note`（`240-249`，`Option` + `default` + `skip_serializing_if` → `pinned`/`pinnedNote`）；`DispatchRecord.pinned`（`397-400`）+ 结构级 `#[serde(default)]`（`343-345`）→ 旧记录缺字段按空串读，且有既有 legacy frontmatter 解析测试兜底（`r1_record_unknown_key_is_ignored_and_missing_keys_use_defaults` 5596-5605、`task06_reconcile_parses_embedded_heading_and_legacy_record` 6674-6685）；`src-ui/src/tauri.ts:496/541-542` 镜像一致；`test:8767-8779` 断言了 `fixRound` 默认 false、`pinnedNote` 大小写与 frontmatter 行。
2. **触发条件齐全且集中在 `3692-3696`**：`status == "ok"`（等价于「首次验证已通过」——首次验证失败会在 `3643-3656` 把 status 置 `validation_failed`）、`node.fix_round`、`role == "执行"`、`write ∈ {单写者, worktree}`（`WriteMode` 只有这三个值，`src-ui/src/graph/types.ts:7`）、`validation_command` 非空白。盲节点在 `3345-3347` 已被强制 `write = 只读`，不会误触发。
3. **只用本次尝试的快照**：`before_dir` 来自本尝试的 `persist_before_snapshot_with_note`（`3650-3663`），不是 `latest_before_dir`（`2307-2331`，那是重派/额度回滚用的）；`red_green_check` 收到的 `before`/`after` 也是本尝试的同一对内存快照。
4. **只碰本次 diff**：`changed` 由本尝试 before/after 差集算出（`2750`）；回退用快照（`restore_before_dir`，含按 `.created` 删除本次新建文件），恢复用 after 字节（`restore_snapshot_paths`）；恢复路径有 `safe_relative_path`、父目录 canonical 前缀校验、链接拒绝（`2711-2725`）。新建文件处理对称：**回退时删除、恢复时按 after 字节重建**（含必要目录 `create_dir_all`），不是「靠重跑修复重建」。
5. **worktree 在 worktree 内做**：`workdir = run_root/<node>/wt`（`3354-3371`），核对传的就是 `workdir`，快照与 `.before` 落在项目 `.dispatch`（`log_dir` 恒为 `project/.dispatch` 或运行目录，`1764-1768`）；worktree 建失败退回单写者时 `worktree_path` 为 `None`、走项目目录并使用已持有的项目锁（`3703-3709` 的分支判断正确）。
6. **核对期间的写锁**：单写者沿用整次派发已持有的项目写锁（`3372-3388` 的 `_write_guard` 活到函数结束，核对在 `3697-3734` 之内）；worktree 在自身目录取节点写锁，取不到则 `skipped` 且不改 status（`3698-3708`）。
7. **判定语义**：红失败+绿通过 → `pinned`（`2849-2851`）；红通过 → `unpinned` + 任务书原文 note（`2841-2847`，`8702` 断言全文）；绿失败 → `validation_failed` + 「红绿核对恢复后验证未通过」（`2833-2839`、`3729-3732`）；快照缺失/不存在/不完整/含未保存内容/未取锁/超时 → `skipped` 且 `validation_failed = false`（不改 status）。核对出的 `validation_failed` 与验证门的 `validation_failed` 走同一条 TASK-12 修正重派路径。
8. **时序正确**：回退 → 红跑 → 恢复 → 绿跑 → 再恢复（`2782→2797→2798→2807→2808`），最终回到 after 字节，符合「核对后目录字节级等于核对前」；`validation_output` 记两份 transcript（`2819-2824`），且只在非空时覆盖首次验证输出（`3724-3726`）。
9. **前端**：三态标签 `pinnedLabelOf`（`src-ui/src/graph/dispatch.ts:265-270`）在结果卡状态旁渲染（`DispatchOverlay.tsx:758-764`），`unpinned` 加 `.unpinned` 警告色（`GraphTab.css:67`，`--warning` 未定义故用 `#c18401` 兜底，同 `resources.css:603` 先例），note 单独一行（`DispatchOverlay.tsx:773`）；`unpinned` 计入 `fixRoundLimit`（`dispatch.ts:842`），未到顶生成修正节点、提示词以「上一轮修复回退后测试仍通过，请补能失败的测试」收尾（`dispatch.ts:691`，`test:463` 断言 `endsWith`），到顶走 `closeWaitingAcceptance` 把等待验收与同父验证兄弟收尾（`853-856`、`720-733`）。
10. **结论进下游材料**：`applyDispatchResults` 把三态写进执行节点 Reply（`dispatch.ts:278-291`），节点卡与下游验收上下文都能读到（`test:470-477` 真调用了该函数，不是纯字符串断言）。
11. **结论与 TASK-12 遗留节点/验证兄弟的关系**：unpinned 走的是 TASK-12 既有的「验证未通过」分支（重派、到顶标已跳过、不生成遗留节点），与任务书「按未修好计入上限」一致；遗留节点仍只在验收 NEEDS_FIX 到顶时生成（`dispatch.ts:826-831`）。
12. **测试能红**：`8680-8764` 六条单测真跑临时 git 仓库（`test_git init/commit`，`8619-8633`），pinned/unpinned/skipped/超时/字节相等（`task13_file_bytes` 整表比对，同时验证无关文件 `unrelated.bin` 未被动）/恢复后失败改 status 各自断言具体值；`8813-8836` 两条引擎级用例走完整 `dispatch_node`（真 stub 通道 + 真 worktree 创建/清理），断言 `(status, pinned)`、项目目录未被 worktree 污染、以及 frontmatter 里的 `pinned: pinned/unpinned`——删掉实现即红。`.skipped` / 未保存内容 / 未取锁三条 `skipped` 分支没有专门用例（见第 12 条，属覆盖不足而非同义反复）。
13. **与 Runbook 一致**：`tasks/README.md:33` 第 6 条「把修复暂时回退再跑一次，红了才算数」与实现的两次运行、两种失败模式一致；`docs/PRODUCT.md:255/265`（第 4 期第 3 条、已定 ②）要求的「自动、不阻断、未钉住计入上限、验收材料带结论」都在实现里能找到对应落点。