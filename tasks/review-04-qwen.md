I'll start by reading the task requirements, conventions, and the implementer's report.



# 代码审查 · 第 4 期「经济流水线」

---

## 1. 需求完整性

**1.1 盲验收节点收不到任何"材料在工作目录"的提示；pi 盲审还带 `--no-tools`，物理上读不到产物**
- `src/dispatch.rs:976-1018`（写 REQUEST.md/diff.patch/changed/ 但不往 prompt 追加一个字）、`src/channels/mod.rs:241`（pi 盲 → `--no-tools`）、`src-ui/src/graph/context.ts:111`（盲验收把父节点的 reply+diff 从 prompt 里剔掉）
- 为什么：0.2 节要求产物走盲目录而不是 prompt 正文，两边都照做了，结果是验收节点的 prompt 只剩 `BLIND_PROMPT_PREFIX` + 用户原话 + 验收维度，没有任何一句说明工作目录里有 `diff.patch`/`changed/`。pi 盲审无工具根本读不到文件；kimi/codex 有工具但没人叫它读。验收会凭空判定，D 节的 PASS/NEEDS_FIX 与 G4 闭环失去依据——这是本期最核心的功能缺口。
- 建议：`prepare_outbound` 在盲目录交付后追加一行「待验材料已在工作目录：REQUEST.md（用户原话）、diff.patch（统一 diff）、changed/（改后文件全文），请先读取再判定」；对带 `upstream_artifacts` 的 `--no-tools` 通道（pi 盲）改为放开只读工具（`--tools read,grep,find,ls`）或按 30KB 内联规则把 diff 直接内联，并在 REPORT 记明取舍。

**1.2 三类 prompt 模板"可在设置里看"没做**
- `src-ui/src/components/common/SettingsModal.tsx:400-436`（角色页只有模型/档位/thinking）、模板实体在 `src-ui/src/graph/dispatch.ts:190-201`
- 为什么：C 节原文是"规则以 prompt 模板落地（**可在设置里看**、不可关）"。不可关做到了，可看没做，用户无从知道验收维度清单是什么。
- 建议：角色页底部加一段只读展示，三段模板原文 + 一句"固定加入，不可关闭"。

**1.3 E 节"用量表按阶段合计"没做**
- `src-ui/src/components/center/graph/DispatchOverlay.tsx:413-414`
- 为什么：现在是逐节点一行 + 一个 stage 列，没有任何合计。E 节明确要求按阶段合计。
- 建议：表尾按 `result.stage` 分组追加 in/out/cache_read/耗时 合计行。

**1.4 结果屏按角色分栏，不按阶段**
- `DispatchOverlay.tsx:381`
- 为什么：E 节说"按阶段分栏"。经济预设下角色与阶段一一对应，但 `NodePanel.tsx:22` 的 ROLES 还有"验证"、还能填自定义角色文本，这些会掉进 `role` 兜底分支，栏标题直接显示用户随手写的角色字符串，且 `.dispatch-result-grid` 是固定 3 列（`GraphTab.css:44`），第 4 栏会换行错位。
- 建议：按 `result.stage` 分组，栏标题取该阶段首个节点的 role（无名时退化为"第 N 阶段"）。

**1.5 链式 thinking 启发式在自家经济预设上是死代码**
- `src/orchestration.rs:243-249`（经济预设执行节点 `thinking: "低"`）、`DispatchOverlay.tsx:50` 与 `dispatch.ts:185`（触发条件是 `thinking === '关'`）
- 为什么：C 节的规则前提是"执行节点 thinking=关"，而预设默认就是"低"，所以主流程永远不会触发；`dispatch.test.ts:124-129` 只测了纯函数，掩盖了这一点。
- 建议：预设执行节点 thinking 改「关」，让启发式抬到「低」并显示理由；或把条件放宽到 `关|低`。

**1.6 B 节的"执行节点"实际只有 codex 一条路：claude/pi 拿不到写权限**
- `src/channels/mod.rs:141-152`（claude 非盲固定 `--tools Read,Grep,Glob`，不看 `writable`）、`:249`（pi 非盲固定 `--tools read,grep,find,ls`）；`writable` 只影响 codex（`:177-181`）
- 为什么：把 claude 或 pi 配成 `write=单写者`，它会跑完、返回正文，但一个文件都改不了 → `artifacts.diff` 为空 → 盲目录里是一份空 `diff.patch` → 验收必然 NEEDS_FIX，G4 三轮全废。角色配置页允许选任意 agent 当执行者，用户踩不到提示。
- 建议：非盲且 `writable` 时给 claude 加 `Write,Edit`、给 pi 加写工具；或在前端把非 codex 的 `单写者/worktree` 标为不可选并给提示（`validate.ts` 加一条）。

**1.7 第 3 期遗留"live 测试的 prompt 改走生产 `serializeContext` / `BLIND_PROMPT_PREFIX`"未落**
- `src/dispatch.rs:1906-2016`
- 为什么：harness 里三段 prompt 全是手写字符串，验收节点既没有 `BLIND_PROMPT_PREFIX` 也没有 `promptForRole('验收')` 的固定维度模板。真跑验的仍不是生产 prompt。REPORT 把它记成"由前端测试覆盖"，但遗留条目要求的是 live 路。
- 建议：把三段模板抽成 Rust/TS 共用的常量文本（例如放 `fixtures/prompts/*.txt`，两侧各自读），harness 用它拼；否则在 REPORT 明确把这条记为未完成。

**1.8 遗留"identity_mismatch 单独显示"只做了浮层**
- 做了：`DispatchOverlay.tsx:37`（"真身不符"）
- 没做：`src/dispatch.rs:435-439`（STATE.md 与进度事件把它折叠成"失败"）、`dispatch.ts:100`（`Reply.status` 落成 `'failed'`）、`GraphTab.tsx`（节点卡只有 待派/运行中/完成/失败）
- 为什么：重开项目后区分彻底丢失，图上看不出这一路是真身不符还是普通失败；这正是遗留条目要治的问题，只治了半个界面。
- 建议：`Reply` 追加可选 `error_kind`（属契约字段追加，需两期同步并写进 REPORT），或 `write_state` 增加 `identity_mismatch`/`validation_failed` 两个分支 + 节点卡状态词表扩充。

**1.9 遗留"cc-switch usage 正则与真实输出核对"未闭环**
- `src/channels/mod.rs:731-762`（改成顺序无关，正确）、测试样本 `:1096` 用的是 `duration_ms=100`
- 为什么：真实脚本输出是 `[usage] provider=… model=… input=… fresh=… cache_create=… cache_read=… output=… turns=… ms=…`（`cc_switch.py:181-184`），没有 `duration_ms`。测试样本是自己编的，键序无关性验证过了，"与真实输出核对"这件事没做；`live_cc_switch_route` 也不断言 usage。
- 建议：测试样本换成真实那一行；live 测试加 `assert!(result.results[0].tokens_in > 0)`。

**1.10 验收 3 的盲目录断言与 A 节冲突**
- `src/dispatch.rs:2001`、`:1607`
- 为什么：A 节规定盲节点输入 = 用户原话 + **附件** + 上游产物，`prepare_outbound:1020-1050` 也确实把附件拷进同一目录。根节点一旦带附件，"恰好三项"的断言就假失败。
- 建议：改成"三项都在，且不存在任何来自项目目录的其他文件"。

**1.11 G4 修正轮的"上一轮产物"只靠共享可见性兜住，指定上游会断**
- `dispatch.ts:218-252`（新执行节点 `parents: [acceptId]`、`target` 直接复制上一轮执行者）
- 为什么：新执行节点 visibility=共享 时靠祖先闭包能拿到上一轮 diff（`context.ts:64-71`），符合 0.3；但若上一轮执行者是"指定上游"，复制过来的 `upstreams` 仍指向旧节点，新节点自己的产物不在其中，链一断第二轮就只剩验收意见没有代码。
- 建议：`appendCorrectionRound` 里把 `upstreams` 重写为 `[acceptId, previous.id]`，或强制新执行节点 visibility=共享。

---

## 2. 逻辑正确性

**2.1 `artifact_diff` 生成的不是可用的 unified diff**
- `src/dispatch.rs:596-620`
- 为什么：无上下文行，旧文件全行 `-`、新文件全行 `+`；`str::lines()` 会吃掉 CRLF 的 `\r`，也不输出 `\ No newline at end of file`。后果三条：(a) Windows（本项目主平台）上 CRLF 文件的 patch 用 `git apply` 会把整个文件转成 LF；(b) 改 1 行也输出整份文件两遍，>200KB 的文件虽不进 `changed_files`（`:617`），全文仍进 diff，盲验收和共享上下文被撑爆；(c) 下游是 kimi 时直接撞 30000 字节硬限（`channels/mod.rs:205`）报"请改用支持 stdin 的通道"。
- 建议：项目是 git 仓库时用 `git diff --no-index <before> <after>` 或 worktree 内 `git diff` 生成真 diff；否则接一个真正的 LCS/Myers diff 并带 3 行上下文；对单文件 diff 设上限，超限只列文件名 + "全文见 changed/"。

**2.2 快照会跟着符号链接/junction 无限递归，且把整个项目读进内存两遍，失败被吞**
- `src/dispatch.rs:520-560`（`path.is_dir()` 跟随链接、无深度/环检测、排除名单只有 7 个名字）、`:549`（`fs::read` 无大小上限）、`:742`/`:862`（`.ok()`）
- 为什么：`.venv`、`build`、`coverage`、`.next`、`vendor`、Windows 的 `Application Data` junction 全在名单外；写节点每次跑两遍全量读，大项目内存/耗时爆炸。快照失败被 `.ok()` 变成 `None`，表现为"产物为空"而不是错误，用户和验收都看不出区别。
- 建议：`symlink_metadata` 跳过链接；加单文件与总字节上限（超限往 `stdout_summary` 记一条"跳过 N 个大文件"）；快照失败要变成 `status="error"`；优先按 `.gitignore`/`git ls-files` 决定遍历范围。

**2.3 worktree 回退后调度器的写者记账失效**
- `src/dispatch.rs:725-731`（回退时 `uses_single_writer = true` 并去抢全局锁）vs `:392-395`（`active_single_writer` 只看 `node.write == "单写者"`）
- 为什么：非 git 项目里两个 `write=worktree` 节点会同时被放出，两个线程在 `acquire_project_write` 里 25ms 空转互斥；UI 显示"运行中"而不是"等待写锁"，并发额度被空转线程吃掉。互斥没破，可观测性和调度都错了。
- 建议：调度前对 `write=worktree` 的节点先探测一次 `git rev-parse --is-inside-work-tree`，探测失败就在调度层按单写者排队；或让 `run_one` 通过 channel 回报"已退回单写者"。

**2.4 拿不到写锁时静默无锁运行**
- `src/dispatch.rs:736-738`：`acquire_project_write(project).ok()`
- 为什么：互斥量中毒（任何持锁线程 panic）就返回 `None`，节点在**没有写锁**的情况下改项目目录，与 B 节"引擎层加锁"的硬要求相反，且无任何提示。
- 建议：`match` 出来，失败直接 `status="error"`、`error="写锁不可用"`。

**2.5 `acquire_project_write` 是无超时、无取消、非公平的忙等**
- `src/dispatch.rs:504-518`
- 为什么：`cancel_dispatch_node` 只杀注册过的子进程（`process.rs:155-165`），拿锁阶段点"取消"没反应；`HashSet::insert` 抢占没有 FIFO，饿死无上限；挂住期间一直占一个并发额度。
- 建议：换成每项目一个 `Mutex`/信号量做阻塞获取（天然排队公平），并加超时（如 `timeout_secs`）后报错。

**2.6 `stage` 是整批取最小深度，不是逐节点**
- `DispatchOverlay.tsx:85`：`Math.min(...ids.map(graphStage))`
- 为什么：同一批里混着深度 1 和深度 2 的可派节点（一条分支短一条长就会发生）时，两者都记成同一个 stage，STATE.md（`dispatch.rs:1201`）与用量表的阶段归属都错，"阶段=图层"这条口径落空。
- 建议：stage 下移到节点级（`DispatchNodeRequest.stage`），`DispatchRecord`/`DispatchNodeResult` 用节点自己的深度。

**2.7 确认/预览清单与实际派发清单不是同一个集合；自动推进无请求上限**
- `DispatchOverlay.tsx:41-45`（`initialIds` = 所有 draft+target）vs `:174`（实际派发用 `dispatchableNodeIds(working)`）vs `:124-136`（preview 算的是前者）
- 为什么：用户在 G1 看到的脱敏数/隔离级别可能不对应真正外发的节点；`start()` 的 `while(true)` 会把图上**所有**能跑的 draft 一路派完（包括从未确认过的分支），每层一次真实请求，没有总数上限也没有二次确认。
- 建议：第一轮就用 `dispatchableNodeIds(graphWithTargets)` 作为 `nodeIds` 与 preview 的输入；每层派发前跑一次 `preview_dispatch` 把脱敏/隔离汇总刷进浮层；加一个总请求上限（如 8），超出停下来问。

**2.8 `addedCorrection` 只保留最后一个验收结果的值**
- `DispatchOverlay.tsx:198-206`
- 为什么：同批多个验收节点时，若最后一个未能追加（上一轮执行节点没产物），前面已追加成功的轮次会因 `if (!addedCorrection) break` 被丢在图上当永久 draft。
- 建议：`addedCorrection = addedCorrection || !!appended.executionId`。

**2.9 只有第一个验证失败节点会被安排重试，第二个的验收节点会被放行**
- `DispatchOverlay.tsx:186-191`（`find` 而非 `filter`）
- 为什么：同批两个执行节点都 `validation_failed` 时，只给第一个追加修正轮；第二个对应的验收节点仍是 draft，下一轮 `dispatchableNodeIds` 会把它派出去审一份没通过验证的产物——直接违背 D 节"失败不送审"。
- 建议：遍历所有 `validation_failed` 逐个 `appendValidationRetry`；重试计数按节点而不是全局一个 `validationRetries`。

**2.10 `validation_failed` 时模型正文被丢弃**
- `dispatch.ts:98-101`：`text: ok ? result.text : result.error || result.text`
- 为什么：执行节点这一轮真正改了什么、说明了什么，在图上看不到了（只留在 `.dispatch` 记录里）；下一轮修正节点的共享上下文也就拿不到上一轮说明，只剩验证报错。
- 建议：保留 `text`，验证输出单独拼在末尾并标注，或 `Reply` 追加 `validation_output`。

**2.11 验证命令在 worktree 里几乎必失败**
- `src/dispatch.rs:848-860`（cwd = workdir）、`:641-655`（`git worktree add --detach <path> HEAD`）
- 为什么：worktree 是 HEAD 的干净检出，没有 `node_modules`/`.venv`/未提交文件/未跟踪配置，`npm test`、`python -m pytest` 大概率非零 → 一路 `validation_failed` 三次后整个流水线中止。
- 建议：worktree 节点跳过验证命令，并在结果卡写明"worktree 未跑验证命令"；或创建后把项目里被 ignore 的依赖目录复制/junction 过去。

**2.12 恢复未完成 run 会丢掉原 run**
- `GraphTab.tsx:135-146` + `DispatchOverlay.tsx:45`（`useState(() => crypto.randomUUID())`）
- 为什么：`continueUnfinished` 只把 pending 改回 draft，浮层用全新 run id，STATE.md 被覆盖，`.dispatch/` 里出现两个 run id 且旧的没有终态。D 节要的是"继续"这次 run。
- 建议：`inspect_dispatch_state` 返回 STATE.md 里的 run id（`dispatch.rs:1200` 已经写了），`continueUnfinished` 把它传进浮层复用。

**2.13 "放弃"直接删掉 STATE.md**
- `src/dispatch.rs:277-285`
- 为什么：放弃等于销毁这次 run 的状态留痕，与"每轮更新 STATE.md"的记账意图冲突，事后无法追溯这次 run 停在哪。
- 建议：改成重命名为 `STATE-abandoned-<ts>.md`，或在文件里追加一行"用户于 … 放弃"后重写。

**2.14 `stdout_summary` 装的是模型正文，导致共享上下文重复**
- `src/dispatch.rs:865-876`、`context.ts:64-71`
- 为什么：字段名叫 stdout 摘要，内容是 `channel_output.text` 前 2000 字；`artifactSummary` 又把它和 `reply.text` 一起拼进下游 prompt，同一段话出现两次，白烧 token。worktree 回退提示也寄生在这个字段（`:869-875`），节点没有产物时提示彻底丢失。
- 建议：`stdout_summary` 只放子进程 stdout 摘要；worktree 回退提示单独作为 result 字段返回并在结果卡显示。

**2.15 `inspect_dispatch_state` 靠中文状态词判未完成**
- `src/dispatch.rs:266-269` 匹配 `": 运行中"` / `": 排队"` / `": 等待写锁"`，与 `write_state:1229-1234` 的字面量没有共享常量
- 为什么：改一处文案就静默失效，重启后不再提示未完成 run。这和第 3 期遗留里"靠中文前缀判 identity_mismatch"是同一类问题，本期刚被要求改掉。
- 建议：STATE.md 加机器可读 frontmatter（`unfinished: true`），或抽 `const STATUS_RUNNING/QUEUED/WAITING_LOCK`。

**写锁互斥与排队 / G4 终止条件 / 并发上限：未发现其他问题。** `dispatch_stage_core:353-357` 的 `clamp(1,3).min(len)` 在串行推进的每一层都生效；`acceptanceRound <= 3`（`DispatchOverlay.tsx:201`）配合 `appendCorrectionRound` 恰好是最多 6 个新节点，符合 0.3；调度器不会出现"全部被阻塞且 active==0"的死锁（阻塞条件都蕴含 active>0）。

---

## 3. 安全

**3.1 产物内容不过脱敏就返回前端、随图落盘、并外发给下游厂商**
- `src/dispatch.rs:861-876`（artifacts 由快照直接生成，只有 `stdout_summary` 过了 `safe_trace`）、`:953`（原样进 `DispatchNodeResult`）→ `dispatch.ts:106` → `~/.aicross/graphs/*.json`；展示在 `DispatchOverlay.tsx:404` 的 `<pre>`；再经 `context.ts:64-71` 拼进下游共享节点的 prompt
- 为什么：同一份 diff/文件全文写进盲目录时是过了 `redact_text` 的（`dispatch.rs:987`、`:1012`），返回给前端这条路径却完全没过。执行者一旦写出含 token 的 `.env`/配置/连接串，明文永久留在图文件里、显示在结果卡上，并被 artifactSummary 发给下一路的厂商。与 `:879-883` 的注释立场（"无论外发脱敏开关如何，留痕与返回给前端的文本都先做安全过滤"）自相矛盾。
- 建议：`artifact_diff` 之后统一对 `diff` 与每个 `changed_files[].content` 过 `safe_trace`，命中数累加进 `result.redacted`；写盲目录时就不必再脱敏第二次。

**3.2 验收节点可以被配成能写项目目录**
- `NodePanel.tsx:204-212`（写权限对任何角色都可选）、`validate.ts:19-45`（无相关校验）、`dispatch.rs:736`/`:741` 与 `channels/mod.rs:177-181`（只看 `write != "只读"`）
- 为什么：把验收（或验证）节点设成 `单写者`，它就会以 codex `-s workspace-write` 在项目目录里跑，还会执行用户配的验证命令、抢项目写锁、拍出产物。审查要求"验收节点不能写"，目前无任何一层拦。
- 建议：后端在 `dispatch_stage_core` 入口对 `role ∈ {验收, 验证}` 或 `visibility == 盲` 强制 `write = 只读` 并在结果里记一条；前端 `validateNode` 加提示、NodePanel 直接禁用这两个选项。

**3.3 kimi 盲审没有任何隔离开关，而它是 0.4 节定的默认验收通道**
- `src/channels/mod.rs:201-228`（kimi 分支无 blind 处理）、`:77-83`（`isolation_for` 只对 claude/pi 返回"完整"）
- 为什么：0.4 节把验收 fixture 定为 `kimi/kimi-code/k3-256k`，也就是默认验收路只有"部分隔离"，带着工具跑在盲目录里（`parse_kimi` 的注释本身就记录了 kimi 会发 `Read` 工具轮）。它可以用绝对路径读到项目目录、`~/.kimi-code/config.toml` 等，"盲"没有技术保证。G1 的路由行会显示"隔离=部分"，但没有任何警示文案。
- 建议：查 kimi CLI 的等价开关（禁工具/不读上下文文件）并接上；查不到就在 G1 对"盲 + 隔离=部分"的节点加一条明确提示，并把经济预设的默认验收通道换成 pi（隔离完整）。

**3.4 `changed/` 下的文件名未做安全化处理**
- `src/dispatch.rs:999-1016`：路径穿越已挡住（拒绝绝对路径与 `..`），但每个 component 没过 `safe_name`
- 为什么：Windows 保留名（`CON`、`NUL`）或含 `:`/尾随空格的路径会让 `fs::write` 失败，整个节点被判成 preparation error（`:963`），一次执行白跑。
- 建议：写入前对每个 component 走 `safe_name`。

**不读凭据文件：未发现违规。** `dispatch.rs` / `channels/mod.rs` 没有 `~/.pi/auth.json`、`~/.cc-switch/*.db` 读取；cc_switch 脚本路径只来自用户设置且 `preview_core:288-299` 校验存在性；子进程只继承环境（`process.rs:67-69`），未回显未落盘。

---

## 4. 代码质量

**4.1 同一个"复核，非独立验证"规则两套实现，判定范围还不同**
- `DispatchOverlay.tsx:294-301`（内联 `models.find(agent+provider+id)`，只扫 `initialIds` 里 role=执行 的节点）vs `validate.ts:26-37`（`findModel` 只比 id 与 agent、不比 provider，扫全部祖先里 role∈{执行,出方案}）
- 为什么：重开浮层只派验收节点时（执行节点已 done，不在 `initialIds`），浮层不提示、节点卡提示，两个界面互相矛盾；provider 不参与匹配还会在"同 model id 不同 provider"时认错厂商。
- 建议：抽 `resolveModel(models, target)` 与 `sameVendorAsUpstream(graph, nodeId, models)` 放进 `graph/vendor.ts`，浮层直接调 `validateNode` 拿提示。

**4.2 「理由：角色默认 / 用户指定」失真**
- `DispatchOverlay.tsx:73`、`:303`
- 为什么：`userEdited` 是本次浮层的临时 state。节点目标若是在 NodePanel 里由用户选的、或从 `roles.json` 填的，浮层一律显示"角色默认"，遗留条目要的信息给错了。
- 建议：`Target` 追加 `source: 'role' | 'user'`（契约字段追加，需两期同步并写进 REPORT），或按"target 是否等于 `roles.json` 里对应角色配置"推断。

**4.3 关闭脱敏后仍提示"可能连带替换正常内容"**
- `DispatchOverlay.tsx:352` + `dispatch.rs:311-315`（`broad_redaction_count` 无条件统计）
- 建议：`if request.settings.redact` 才统计 `broad_redacted`。

**4.4 dispatch.rs 里留着只被自己测试用到的死代码**
- `src/dispatch.rs:1319-1355`（`Classification` / `parse_classification`）、`:1526-1539`（对应测试）
- 为什么：生产路径没有调用方，"归类模型"设置项已置灰（`SettingsModal.tsx:463-471`）。测试通过会让人以为归类功能在跑。
- 建议：接线或删除；若保留，在注释里写明"未接线，见 TASK-04 遗留"。

**4.5 与第 2/3 期的重复**
- `dispatch.rs:1161-1191`（`write_record`）与 `:1193-1251`（`write_state`）各自拼 Markdown、各自走 `write_atomic`，与 `orchestration.rs:373-410` 的 `write_json_atomic` 是第三份"临时文件 + rename + 失败删目标重试"实现。三份逻辑一致但细节不同（`dispatch.rs:1253-1283` 用 `File::create`，`orchestration.rs` 用 `create_new`）。
- 建议：抽到 `paths.rs` 或新建 `atomic.rs`，一处实现两处调用。

**4.6 契约字段追加合规：未发现问题。** `orchestration.rs:91-93` 与 `types.ts:22` 只加了 `Reply.artifacts`，既有字段无改名无删除；`DispatchNodeResult` 的 `stage`/`validationOutput`/`artifacts` 属派发 IPC，不是图持久化契约。但 `DispatchArtifacts`（camelCase，`dispatch.rs:101-111`）与 `ReplyArtifacts`（snake_case，`types.ts:26-33`）两份定义靠 serde rename 对齐，没有测试锁住键名映射——建议加一条 Rust 测试断言序列化后的键名。

**4.7 第 2/3 期图与派发行为：未发现破坏。** `contextPartsOf` 的盲/共享/指定上游规则、`dispatchableNodeIds`、`continuationSession`、`stamp`、`clearTargetSession` 语义均未变，`context.test.ts` 既有用例全在。唯一行为变化是 `replyMessage`（`context.ts:60-66`）把 artifacts 摘要拼进任何 done 回复，会让第 2/3 期的共享上下文变长并可能撞 kimi 30000 上限（见 2.1）。

**4.8 界面文案：未发现问题。** 按钮均为 2–6 字动词（收起/展开派发/继续/放弃/采纳/取消采纳/重派/升级/确认派发/交人裁决），状态词成对（已采纳 ↔ 取消采纳），无 emoji、无拟人化、无"门禁/落库/作用域"。仅 `DispatchOverlay.tsx:381` 把自定义角色原文当栏标题（见 1.4）。

---

## 5. 测试

已存在且真断言的：三种 visibility 组装（`context.test.ts:60-121`、`:196-231`，含"盲验收 prompt 不含 PLAN_TEXT/EXEC_TEXT/DIFF_EXEC"）、写锁互斥（`dispatch.rs:1570-1586`）、结论解析三态（`dispatch.test.ts:118-122`）、小任务与链式启发式（`dispatch.test.ts:124-129`）、盲目录三项（`dispatch.rs:1588-1615`）。验收 2 的四组齐了。

**最该补的 3 个：**

1. **修正轮上限。** `dispatch.test.ts:131-144` 只测了单轮追加与 `acceptanceRound == 2`；"最多 3 轮 = 最多 6 个节点、第 4 次 NEEDS_FIX 不再追加"这条唯一的判定活在 `DispatchOverlay.tsx:201` 的 `<= 3` 里，零覆盖（组件没有任何测试）。建议把上限判定抽成 `canAppendCorrectionRound(graph, nodeId)` 放进 `dispatch.ts`，测 1/2/3/4 轮四个点 + 节点总数 6。
2. **`artifact_diff` 的边界。** `dispatch.rs:1541-1568` 只测了"改 + 增"，没测删除、二进制、>200KB 截断、CRLF、末行无换行——正好是 2.1 的全部问题面。建议补 5 个断言用例（含 `assert!(diff.contains("Binary files"))`、`assert!(changed_files.is_empty() && diff.contains(...))` 的 200KB 分支）。
3. **调度层的写锁排队与并发上限。** 现在只测了 `acquire_project_write` 原语，没测 `dispatch_stage_core` 里"两个单写者同阶段 → 第二个先进 等待写锁 再运行、`limit` 仍生效"。建议用 `AICROSS_TEST_RUN_ROOT` + 一个 agent 名不存在的 `write=单写者` 节点（`build_invocation` 立即失败，但状态机照走），断言 `emit_progress` 的状态序列与 STATE.md 里出现过"等待写锁"。

其他值得补：`appendValidationRetry` 的重试上限（3 次后停止）；`upstreamArtifactsOf` 在"指定上游"下依赖 `context.ts:151-155` 的 fallback 循环这条路径无专门用例；`inspect_dispatch_state` 的 unfinished 判定无测试；worktree 创建/非 git 回退零集成测试（REPORT 已承认）。

---

## 6. 真跑

**6.1 `live_economy_pipeline`（`dispatch.rs:1906-2016`）——构造基本对，但有 5 处会导致编排者误判**

对的部分：模型符合 0.4 节（`pi/zai-coding-cn/glm-5.3-flash`、`codex/gpt-5.6-luna`、`kimi/kimi-code/k3-256k` 完整别名）；3 次请求；断言 diff 非空（`:1975`）、验证输出非空（`:1966-1969`）、盲目录三项（`:2001`）、`.dispatch` 三份 + STATE.md（`:2002-2011`）。

会导致误判的：

- **`python -m pytest` 在这份 fixture 上大概率收不到测试。** `fixtures/pipeline/roman.py` 只有 docstring 和注释，文件名不匹配 `test_*.py`/`*_test.py`，任务文本也没说测试写哪。执行者最自然的做法是把 3 个测试写进 `roman.py`，pytest 退出码 5（no tests collected）→ `run_validation` 判失败（`dispatch.rs:679-681`）→ `status = validation_failed` → `:1965` 的 `assert_eq!(status,"ok")` 直接挂，看起来像产品 bug。Windows 上只有 `py`/`python3` 没有 `python` 时是同一现象。**改法**：fixture 里放一个 `tests/test_roman.py` 骨架（或 `pytest.ini` 指定 `python_files`），任务文本明确"测试写进 tests/test_roman.py"，验证命令改 `python -m pytest tests`，harness 里先探测解释器、探测不到就 `println!` 跳过而不是失败。
- **没有 ≤8 次请求的断言**，只有 `println!("PIPELINE_RUN={} REQUESTS=3 …")`（`:2012-2015`）把常量打成日志。REPORT 写"请求总数为 3，小于 8 次上限"是把常量说成了断言。**改法**：累加三次 `results.len()` 后 `assert!(total <= 8)`。
- **真身/降级零容错。** 0.4 节自己就写明 zai 会静默升级 glm 系列；`:1943` 对出方案节点硬断言 `status == "ok"`，一旦真身不符（`channels/mod.rs:585-590`）测试在第一步就停，后面产物/盲目录/STATE 的断言全不执行，编排者只看到一句"真身不符"，本期要验的东西一个都没验到。第 3 期的 `live_blind_review` 有 `enforce_live_success` + 替补分支，这条没有。**改法**：沿用同一套结构（不可写/真身不符时打印 SKIP 原因并走 deepseek 替补），或把三段断言各自包在 `if status == "ok"` 里并打印诊断。
- **盲目录断言过严**（见 1.10）：根节点带附件就假失败。
- **G4 闭环在真跑里完全没覆盖**：测试不解析验收结论、不触发修正轮，只验到"验收节点能返回正文"。**改法**：至少把验收正文打印出来供人工核对 PASS/NEEDS_FIX，并在 REPORT 写明是否触发过修正轮；有余力就加第 4 次请求跑一轮修正（仍在 8 次内）。
- 附带：`:1915` 的 `std::env::set_var("AICROSS_TEST_RUN_ROOT", …)` 是进程级全局，与 `live_blind_review`/`live_shared_continuation` 里的同名 set_var 冲突；`cargo test -- --ignored` 一次跑全部时三个测试并行会互相覆盖运行目录。REPORT 的运行命令应加 `--test-threads=1`。

**6.2 `live_cc_switch_route`（`dispatch.rs:2021-2075`）——会把"未验证"报成"通过"**
- `:2022-2030`：环境变量缺失时 `println!` + `return`，测试报 ok。编排者在沙箱外跑 `cargo test -- --ignored` 会看到全绿，很容易误判成"cc-switch 路已验证"，而 REPORT 写的是未验证。
- 另外它不断言 usage（`tokens_in`/`cache_read`/`output` 全不检查），所以遗留条目"usage 正则键序与真实 `cc_switch.py --usage` 输出核对"仍然没闭环。
- **改法**：缺环境变量时 `panic!`（测试是被 `--ignored` 显式点名的，静默通过没有意义）；补 `assert!(result.results[0].tokens_in > 0)`，并把真实 `[usage]` 行原文贴进 REPORT。

**6.3 worktree 路零真跑、零集成测试。** `dispatch.rs:627-660` 与 `:718-733` 的回退分支只经过编译（REPORT 已承认）。建议补一个不需要模型的集成测试：临时 `git init` + 一次 commit，跑一个 `write=worktree` 且 agent 名不存在的节点，断言 `wt` 目录被创建、`artifacts.worktree` 有值；再在非 git 目录跑同一节点，断言 `stdout_summary` 含"已退回单写者"。

---

## 结论

**对照 TASK-04 原始需求：未完整实现。** 骨架（图层推进、`Reply.artifacts` 只追加、盲目录三件套、G4 长在图上且 ≤3 轮 6 节点、`roles.json`、三类模板、遗留 9 条中的 6 条）都在且能编译能测，但"盲验收拿不到产物指引 + 只有 codex 真能写 + diff 不是可用 diff + 产物不过脱敏"这四条叠在一起，使经济流水线在真实项目上产不出可信的验收结论。

缺项列表：

1. 盲验收 prompt 未告知工作目录里的 REQUEST.md/diff.patch/changed/，pi 盲审 `--no-tools` 物理上读不到（1.1）
2. 三类 prompt 模板在设置里不可见（1.2）
3. 用量表未按阶段合计、结果屏按角色而非阶段分栏（1.3、1.4）
4. 链式 thinking 启发式在经济预设上是死代码（1.5）
5. claude/pi 的 单写者/worktree 拿不到写权限，产物必为空（1.6）
6. 遗留"live prompt 走生产 `serializeContext`/`BLIND_PROMPT_PREFIX`"未落（1.7）
7. `identity_mismatch` 只在浮层单独显示，STATE.md 与图节点仍折叠成"失败"（1.8）
8. cc-switch usage 与真实输出核对未闭环（1.9）
9. G4 修正轮在"指定上游"执行者下拿不到上一轮产物（1.11）
10. `artifact_diff` 不是可应用的 unified diff，CRLF/末行换行/体积都有问题（2.1）
11. 快照无符号链接与体积保护，失败被 `.ok()` 吞（2.2）
12. worktree 回退破坏调度器写者记账；写锁获取失败静默无锁运行；忙等无超时无取消（2.3–2.5）
13. `stage` 取批次最小深度而非逐节点（2.6）
14. 确认/预览清单与实际派发清单不一致，自动推进无请求上限（2.7）
15. 多验收/多验证失败时的分支漏项（2.8、2.9）
16. `validation_failed` 丢失模型正文；worktree 里跑验证命令几乎必失败（2.10、2.11）
17. 恢复未完成 run 换新 run id，"放弃"即删 STATE.md（2.12、2.13）
18. `Reply.artifacts` 未脱敏即落盘、上屏并外发下游（3.1）
19. 验收节点可被配成 单写者 并拿到 `workspace-write`（3.2）
20. kimi 盲审无隔离开关却是默认验收通道（3.3）
21. 测试缺修正轮上限、`artifact_diff` 边界、调度层写锁排队（5）
22. `live_economy_pipeline` 的 pytest/解释器/真身容错会造成误判，≤8 次请求未断言，G4 未覆盖；`live_cc_switch_route` 缺环境变量时静默通过（6.1、6.2）
23. worktree 路零集成测试（6.3）