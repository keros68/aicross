• 四个子代理在后台核对，我同时自己读参考实现的核心文件。

• 我继续读参考实现的其余小文件来自行核对关键点。

• 我自己已核对完核心参考实现的关键函数，现在等子代理的核对结果。

• 推进循环核对完成，继续等其余三个。

• 四个子代理都完成了。我对几个关键高危发现自行验证后再写结论。

• 所有核对完成，关键发现均已由我本人对照两侧代码逐一验证。以下是完整审查结果。

  ---

  # TASK-36 只读代码审查结果

  审查范围：`after/` 全部改动，对照参考实现 `src-ui/src/console-channel.ts` 的 `runConsoleDispatch` 及 `src-ui/src/graph/` 各函数。`before/` 与 `after/` 大量文件的差异经核实仅为换行符差异（如 `fonts.rs`、`git.rs`），真实改动集中在任务书列出的文件，外加一组**未申报的越界改动**（见高-1）。

  ## 高严重度

  **高-1 越界改动且报告未披露：删除 `docs/history/`、回退 `docs/PRODUCT.md`、改动 3 个既有前端测试**
  - 证据：`after/docs/PRODUCT.md` 内容是「产品说明第一稿」（08-30 旧版），`before/docs/PRODUCT.md` 是「只写现行设计」的 09-30 定案版；`before/docs/history/PRODUCT-0830-0930.md` 在 `after/` 中整个被删。三个既有测试被改指向旧文档：`src-ui/src/__tests__/console-chat-22.test.ts:231`、`src-ui/src/resources/__tests__/agent-display-r1.test.ts:15`、`src-ui/src/settings/__tests__/ui-scale-19.test.ts:155`（均从 `docs/history/PRODUCT-0830-0930.md` 改指 `docs/PRODUCT.md`）。
  - 后果：任务书只允许「改 `src/**`、`docs/CONSOLE-CHANNEL.md`，外加一个前端测试文件与 fixtures」。这些改动疑似把别期的文档整理成果整体回退，且 REPORT-36 改动文件表只字未提，交付物不可信。
  - 建议：恢复 `docs/history/`、`docs/PRODUCT.md` 与三个测试文件；若确属别期成果被误回退，单独处理并在报告披露。

  **高-2 `fix_round` 对全部节点置 true，首轮执行节点也会触发红绿核对**
  - 证据：Rust `after/src/engine/mod.rs:1864`（`scope = all_ids.clone()`，含全部新建节点）+ `:922`（`fix_round: fix_ids.contains(id)`）；TS 参考 `console-channel.ts:436` 调 `buildNodeRequest` 时不传 `fixRound`，`graph/dispatch.ts:1209` 的 `...(parts.fixRound ? {fixRound:true} : {})` 在控制台路径永不生效。`fix_round=true` 是 `after/src/dispatch.rs:4159-4163` 红绿核对的触发条件（另抑制 `:2932` 的「worktree 节点未跑验证命令」提示）。
  - 后果：设置了验证命令时，无窗口引擎会让**首轮**执行节点多跑一次验证命令并追加 `[红绿核对]` 结论——参考实现在控制台路径从不对首轮做这件事。派发行为实质性不同，还白耗一次命令执行。
  - 建议：按 TS 口径，只对修正轮执行节点（`isFixRoundExecution` 同口径：`[修正 N/M]` 前缀或「根据上一轮验收意见修正实现。」开头）置 `fix_round`。

  **高-3 盲节点的上下文丢失非直接父祖先的用户消息与附件**
  - 证据：Rust `engine/mod.rs:660-662` 把用户消息/附件的收集关进 `default_selected` 闸门（盲节点只保留自身与直接父）；TS `graph/context.ts:104-113` 的用户消息只按「指定上游」过滤，盲节点**所有祖先**的用户消息都进 prompt（`:202` `part.defaultSelected !== false` 对用户 part 恒真）。
  - 后果：经济/盲审预设里盲的「验证」节点，TS 的 prompt 含根材料的 `[用户]` 段（原始需求）与其附件，Rust 没有；`materials` 数组同样缺这些附件。盲审输入不同，结论可能不同——这是每次经济模式派发都会踩到的差异。
  - 建议：用户消息/附件的收集只用 `keep_user` 闸门，`default_selected` 只用于回复块（与 `context.ts` 对齐），并用盲审三路夹具钉住。

  **高-4 后台进程继承父进程 stdio，管道捕获场景下「同步完成」失效**
  - 证据：`engine/mod.rs:1797-1810` `spawn_background` 只设了 `creation_flags`，未配置 stdin/stdout/stderr，Rust `Command` 默认三路继承。
  - 后果：宿主/skill 以管道捕获 `aicross dispatch` 输出时（Python `capture_output`、Node `execFile` 等），管道写端被 `__run` 子进程继承，调用方 read-to-EOF 会一直阻塞到整条流水线跑完——dispatch 进程虽退出 0，调用方表现等同同步等待全部派发，还会耗尽其超时预算。任务书「起后台进程 → 打印响应并退出 0」的设计被架空。
  - 建议：spawn 前 `.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())`。

  **高-5 Rust 金标准测试未满足任务书 §5.2：不驱动推进循环，23/24 夹具只查 JSON 形状**
  - 证据：`engine/mod.rs:2187-2283` 唯一夹具测试：对 24 个夹具只断言 `inputGraph/requests/finalGraph/runSummary` 四个 key 的形状（`:2228-2234`），**仅 economy-pass** 做请求/最终图/run 摘要逐字段相等（`:2254-2282`）。`run_background`/`reduce_step` 从未被夹具驱动。
  - 后果：任务书 §5.2 要求「读同一批夹具，驱动纯函数**与推进循环**（阶段结果由夹具注入），断言请求与最终图逐字段相等」——未做到。修正轮、验证重试、遗留、重复批次、quota 等核心链路在 Rust 侧零一致性保障。REPORT-36 第 9 行虽承认只核对一次通过场景，但未声明这与任务书要求的差距。
  - 建议：Rust 测试逐夹具驱动 `build_request`/`apply_results`/`reduce_step`（id 与时钟注入确定值），对每阶段请求与最终图逐字段断言。

  **高-6 金标准夹具本身钉不住要求的行为（TS 侧）**
  - needs-fix-pass 空转：`fixtures/engine-golden/needs-fix-pass.json` 的输入图只有根材料 + 一个验收节点（无带 artifacts 的执行父），`reducePipelineStep` 无法生成修正轮——`finalGraph` 无任何修正节点，注入的第二批 `[ok]` 全程未被消费，「NEEDS_FIX → 修正轮 → 通过」实际只钉了「NEEDS_FIX 后无动作」。对应 `__tests__/engine-golden.test.ts:82-85, 108`。
  - precheck 十条是硬编码字符串：`engine-golden.test.ts:122-128` 把十条拒绝文案直接写死在测试里，`validateNode`、quota 检查、`dirtyWorktreeHintOf` 与 `runConsoleDispatch` 的检查顺序（`console-channel.ts:361-400`）**从未被执行**；「检查顺序钉住」钉的是作者声明的顺序，TS 侧改文案或换顺序测试照样绿。
  - 金标准用简化替身：请求用测试内自写 `requestOf`（`engine-golden.test.ts:36-42`，`sessionId/previousRunner/currentMessage/upstreamArtifacts` 全空、`stage` 写死 1），不是 `runConsoleDispatch` 的 `buildRequest` 闭包（`console-channel.ts:428-438`）；run 摘要用自写 `summaryOf`（`:43-49`），缺 `quota_resets_at` 与 `skipped_reason` 回退（真实 `nodeSummary` 在 `console-channel.ts:82-93`）。多阶段场景后续阶段发出的请求也不入夹具（`:53` 只从输入图生成）。
  - 后果：任务书 §5.1「用现有 TypeScript 函数跑场景」部分落空；金标准形状与线上真实请求/摘要不一致，Rust 比对的对象本身就是替身。
  - 建议：needs-fix-pass 改为「执行（带 artifacts）→ 验收 NEEDS_FIX」的输入图；precheck 逐条构造触发输入并真实调用 `validateNode`/`quotaBlockedNodeIds`/`dirtyWorktreeHintOf` 及顺序断言；请求与摘要改用与 `runConsoleDispatch` 同口径的装配。

  ## 中严重度

  **中-1 `latest_leaf` 与 TS `latestLeaf` 算法不同，且不跳过遗留节点**
  - 证据：Rust `engine/mod.rs:346-359` 取「不是任何人父节点」的叶子中 `(created_at, id)` 最大者；TS `graph/layout.ts:68-83` 取**深度最大**节点（同深度取 created_at 最新）且 `:73` 明确跳过 leftover 节点。
  - 后果：有分叉的图上续写（continue-existing-graph 场景），新材料挂的父节点两边不同；修正到上限产生遗留节点后再派发，Rust 会把新材料挂到遗留节点下，TS 不会。图形状与后续全部上下文分叉。
  - 建议：按深度（最长路径）实现并跳过 leftover。

  **中-2 cancel 与后台收尾的写竞态：done 可被覆盖回 running**
  - 证据：`engine/mod.rs:2099-2108` `cancel` 读 run（:2100）后无条件把 `status:"running"` 整条写回（:2106-2107）；后台在 `:2055-2056` 写完 done 并删除标记。cancel 读与写之间后台恰好收尾时，done 被覆盖回 running 且残留 `.cancel` 标记。
  - 后果：已完成的 run 变回 running，下次 status 读到死 pid 改写成 `failed/引擎进程已退出`（理由误导，编排者真机核第 3 条会被污染），标记文件永久残留。
  - 建议：cancel 只写标记不写 run 文件；或写回前重读并仅在仍为 running 时落盘。

  **中-3 Windows pid 存活判断存在 pid 复用误判**
  - 证据：`engine/mod.rs:261-274` `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` 成功即判存活。
  - 后果：引擎进程死后 pid 被别的进程复用 → run 永远停在 running、在飞上限槽位泄漏、cancel 标记写给死 run。
  - 建议：OpenProcess 后用 `QueryFullProcessImageNameW` 比对可执行文件名，或记录并比对进程启动时间。

  **中-4 dispatch 抛错时 run.reason 丢失**
  - 证据：Rust `engine/mod.rs:1954-1976` 把 `dispatch_stage_core` 的 Err 内联合成 error 结果后照常走循环，`error` 变量保持空，最终 `run.reason = ""`（:2047）；TS `graph/dispatch.ts:1539-1547` catch 把错误文案存入 `pipeline.error`，`console-channel.ts:465` 作为最终 reason。
  - 后果：节点状态与图一致，但 run 摘要 reason 为空，CLI/界面看不到失败原因。
  - 建议：合成 error 结果的同时把错误文案记入 run.reason。

  **中-5 验证重试到上限时不跳过同父的 draft「验证」兄弟节点**
  - 证据：Rust 到上限分支只改等待中的验收节点（`engine/mod.rs:1422-1448`）；TS `closeWaitingAcceptance` 同时给所有 draft 验证兄弟写 `skipped_reason`（`graph/dispatch.ts:716-733`，reduce 调用点 :857）。
  - 后果：最终图上验证兄弟缺「上游验证未通过/上游测试未钉住」的跳过说明，节点卡「已跳过」标签缺失，与 TS 图状态不一致。
  - 建议：到上限分支复用 `append_validation_retry` 里 :1310-1320 的兄弟跳过逻辑。

  **中-6 `mark_pending`/`reset_draft` 的字段保留语义与 TS 不同**
  - 证据：TS `markPending` 保留原 reply 全部字段（`graph/dispatch.ts:205-213`），`resetNodesToDraft` 只动 `status==='pending'` 的节点且保留其余字段（`:1013-1023`）；Rust `mark_pending` 只保留 `adopted`（`engine/mod.rs:936-949`），`reset_draft` 无条件重置为全新 Reply、连 adopted 也丢（:950-961）。
  - 后果：重复批次停止时若节点曾有 adopted/skipped_reason 等，TS 保留、Rust 抹掉。
  - 建议：`reset_draft` 只处理 pending 节点并保留原字段；`mark_pending` 保留原 reply。

  **中-7 请求内节点 `stage` 用批次最小值而非逐节点深度**
  - 证据：Rust `engine/mod.rs:852-856, 897`；TS `console-channel.ts:436` 每节点 `stage: graphStage(sourceGraph, id)`（请求级才是 min，:430）。
  - 后果：dispatchable 节点跨深度时（多父汇合图），引擎按节点 stage 记结果/留痕的值不同。
  - 建议：节点级 `stage` 改为逐节点 `graph_stage`。

  **中-8 模型匹配口径两处与 TS 不同**
  - 证据：①请求装配的 model 查找，Rust 多了 agent+id 回退（`engine/mod.rs:862-873`），TS 仅 provider 精确（`graph/dispatch.ts:1179-1181`）——provider 不匹配时 TS 的 vendor/billingSource 为 ''，Rust 会填别 provider 同 id 模型的值；②额度检查 TS 先 provider 精确再退 agent+id（`graph/dispatch.ts:1046-1047`），Rust 的 `find_model`（`engine/mod.rs:1522-1526`）只有 agent+id——同 agent+id 多 provider 且只有一家额度用尽时可能误拒/漏拒。
  - 建议：两处分别对齐 TS 的查找口径。

  **中-9 `leftover_items` 的 bullet 口径放宽**
  - 证据：TS `graph/dispatch.ts:774` 要求 `-/•/* + 空白` 或 `数字+[.)、]` 或 `（n）`/`(n)`；Rust `engine/mod.rs:1212-1222` 把**任何 ASCII 数字开头**的行当 bullet，且不识别 `（n）`/`(n)`。
  - 后果：验收正文里「3 处需要修改」「2025 计划」这类行 Rust 会收进遗留、TS 落入「整段按行取」分支，遗留节点内容分叉。
  - 建议：按 TS 正则逐条对齐并加夹具。

  **中-10 循环中途 IO 失败无异常收尾**
  - 证据：`engine/mod.rs:1952, 2020` 的 `?` 直接从 `run_background` 返回 Err；TS 循环内任何 throw 都进 catch → onError 补失败节点 → run 置 failed 带 reason（`graph/dispatch.ts:1539-1547`）。
  - 后果：磁盘错误等场景下节点悬在 pending、run 停在 running，只能靠下次 status 的死 pid 清扫补「引擎进程已退出」，理由误导且无失败节点补写。
  - 建议：`run_background` 包一层收尾：任何 Err 先把在飞节点补 failed、run 写 failed+原始错误文案再返回。

  **中-11 续写既有图时 `source` 为空会被改写成 console**
  - 证据：Rust `engine/mod.rs:1660-1668`（`graph_source.is_empty()` → "console"）；TS `console-channel.ts:371-373` 注释明确「续写用户自己的图保留原来源」，原样保留（含空）。
  - 后果：旧图（无 source 字段）经一次命令行派发后来源被改成 console。
  - 建议：仅新建图（无 existing）时写 console。

  **中-12 拆分盖章的限额行文案不同**
  - 证据：Rust `engine/mod.rs:418-419`「本次最多拆分 {n} 件。」；TS `graph/swarm.ts:27-29` + `graph/stamp.ts:8-12` 拼出「本次上限 {n} 件。」（且带 `Math.max(1, limit)` 夹取）。
  - 后果：`--preset builtin-swarm` 直给时拆分节点冻结文本与窗口路径逐字不同（蜂群编排虽不在本期范围，但 stamp 的这段被移植了）。
  - 建议：文案对齐为「本次上限 N 件。」并加同样的下限夹取。

  **中-13 退出码 3 与取消链路的测试缺口**
  - 证据：任务书要求的「退出码 3 的三种来源」在 `engine/mod.rs` 与 `console_cli.rs` 测试中均无断言（三条路径：`console_cli.rs:19-22`、`engine/mod.rs:1591-1592, 1780/1792, 1781-1788`）；「cancel 标记被后台线程读到」的测试 `engine/mod.rs:2173-2179` 只是写文件再断言文件存在，后台 watcher 线程（:1890-1902）从未在测试中启动。
  - 建议：退出码 3 三条路径各加一条 `run_at` 级测试；取消测试应驱动 watcher 逻辑（注入标记文件，断言在飞节点走了 `cancel_dispatch_node` 且 run 收尾为 failed/派发已取消）。

  **中-14 文档退出码表与代码实际行为不符**
  - 证据：`docs/CONSOLE-CHANNEL.md:184` 称 3 只有三种来源；但 `propose` 仍走 HTTP 循环，可以「控制台未运行/清单过期」退出 3（`console_cli.rs:75-89`，propose 会落到 :70 之后的循环）；另外图保存失败也映射为 3（`engine/mod.rs:1769`），超出列举来源。
  - 建议：文档补一句 propose 的既有 3 语义；明确图目录不可写是否算第 3 类。

  ## 低严重度

  - **低-1 `active_runs` 把原子写临时文件多计一次**：tmp 文件与目标同目录且 `active_runs` 不过滤文件名（`engine/mod.rs:184-188, 1571-1581`），并发 dispatch 在 rename 前窗口内会把同一 run 数两次 → 在飞上限偶发误拒。建议只统计 `.json` 且非 `.` 开头的条目。
  - **低-2 `__run` 的 run_id 未过 `valid_id`**：`run_background` 直接拼 `run_path`（`engine/mod.rs:1837-1841`），`aicross __run ../x` 可越出 runs/ 目录。建议入口加 `valid_id` 校验。
  - **低-3 run 摘要 nodes 顺序不稳定**：`all_ids` 是 HashSet（`engine/mod.rs:1857, 2044`），TS 的 `allRunIds` 是插入序 Set（`console-channel.ts:379`）。run.nodes 数组顺序非确定。建议改用 Vec+HashSet 保序去重。
  - **低-4 新增「项目目录不存在」检查排在在飞上限之前**（`engine/mod.rs:1612-1614`），TS 检查链无此项且顺序无基准；另 `fs::canonicalize` 在 Windows 产出 `\\?\` 前缀路径，会写进 `graph.project` 并作为 `project_dir` 下发（:1585-1590, 1940），需真机复核各工具对该前缀的容忍度。
  - **低-5 `append_correction` 选「上一轮执行」的口径**：TS 取第一个 role=执行 的父、无 artifacts 就整体不生成修正轮（`graph/dispatch.ts:599-600`）；Rust 跳过无 artifacts 的执行父选下一个（`engine/mod.rs:1100-1113`）。仅多执行父图有分歧。
  - **低-6 `upstream_artifacts` 缺 TS 的兜底第二遍**（`graph/context.ts:180-185` 把不可达 allowed 节点的 artifacts 补上；`engine/mod.rs:768-787` 只走 DFS）。损坏/恢复图上直接父产物会丢。
  - **低-7 continuation 两处**：TS 还认旧图正文首行 `（已切备选：` 标记（`graph/dispatch.ts:222-228`），Rust 只看 `fallback_from` 字段（`engine/mod.rs:806-808`）——旧图会误续会话；provider 比较 TS 把 `None` 与 `''` 视为相等（`dispatch.ts:157`），Rust `pt.provider != target.provider` 不等（`engine/mod.rs:809`）。
  - **低-8 adopt 对无 reply 节点报错退出 1**（`engine/mod.rs:2081-2084`），TS `setReplyAdopted` 静默 no-op（`graph/dispatch.ts:196-198`）。
  - **低-9 materials 顺序**：TS 保序去重（`graph/context.ts:226-227`），Rust `sort()+dedup()`（`engine/mod.rs:720-721`）。
  - **低-10 上下文 DFS 子节点排序**：TS 按 `Object.keys` 插入序（`graph/stamp.ts:96-102`），Rust 按 created_at+id（`engine/mod.rs:588-593`）；从磁盘读回的图两者可能不同（注意 Rust 写图用 HashMap，文件键序本身就不稳定，TS 读回也受牵连——需复核是否要对齐）。
  - **低-11 `fillRoot` 标题截断**：TS 按 UTF-16 码元 `slice(0,20)`（`graph/mutate.ts:44-46`），Rust 按 Unicode 字符 `chars().take(20)`（`engine/mod.rs:376-378`）——含 emoji 时标题不同。
  - **低-12 `parse_acceptance` 尾部窗口**：TS `slice(-1200)` 按 UTF-16 码元（`graph/dispatch.ts:407`），Rust `chars().rev().take(1200)` 按标量（`engine/mod.rs:1057-1065`）。
  - **低-13 备选 vendor 查找**：TS 先 provider 精确再 agent+id（`graph/dispatch.ts:239-243`），Rust 只 agent+id（`engine/mod.rs:888-892`）；TS 判 `fallback.model.trim()` 非空，Rust 判 `!is_empty()`。
  - **低-14 `normalizedPath`**：TS 额外把 `\\?\UNC\` 归一为 `//`（`console-channel.ts:33-38`），Rust 只剥 `\\?\`（`engine/mod.rs:1515-1521`），UNC 路径匹配不上既有图。
  - **低-15 盲附件 basename**：TS `split(/[\\/]/).pop() || path`（`graph/context.ts:218`），Rust `Path::file_name`（`engine/mod.rs:668-679`）——尾部带分隔符的附件路径显示不同。
  - **低-16 内置预设文本优先级边界**：TS 对 builtin-economy 前 3 个、builtin-blind 第 1 个节点用 `''` 覆盖 `presetNodeText`（`graph/stamp.ts:85-91`），Rust 这些位置走拆分/preset_text 分支（`engine/mod.rs:400-422`）。仅当内置预设对应节点带 拆分 role+preset_text 才有差，实际预设不会命中。
  - **低-17 `leftoverLine` 空 role 回退**（`graph/dispatch.ts:783` vs `engine/mod.rs:1241-1248`）与 `appendLeftover` 不复用同源遗留节点（`graph/dispatch.ts:793-794` vs `engine/mod.rs:1249-1266`）——理论差异。
  - **低-18 设置小数截断**：TS 保留小数（如 700.5），Rust `as u64` 截断（`engine/mod.rs:109`）。
  - **低-19 平台性差异（需复核确认可接受）**：TS 选图优先已打开的图 tab（`console-channel.ts:40-51`），Rust 只按 updated_at 扫 graphs 目录（`engine/mod.rs:1503-1514`）——无窗口拿不到 tab 态；Unix 下后台进程未 setsid，终端关闭 SIGHUP 可能杀引擎；Windows 宿主若以 Job Object 管理进程，DETACHED_PROCESS 逃不出 job。

  ## 按审查项逐条结论

  1. **行为一致性**：前置检查的顺序与九条拒绝文案逐字一致（已逐字核对 `engine/mod.rs:1615-1767` vs `console-channel.ts:361-400`）；盖章、角色默认值、写护栏、提示词模板 `include_str!`、UTF-16 截断的 run 摘要均一致。不一致处：高-2、高-3，中-1、中-4~中-9、中-11、中-12，低-5~低-17。
  2. **测试要求**：未真正满足——见高-5、高-6、中-13。已满足的：设置缺失/越界/解析失败、原子写、pid 已死判 failed、adopt 无窗口、类型往返、中文/表情截断（`engine/mod.rs:2120-2283`、`orchestration.rs:1292-1312`）。
  3. **进程与持久化**：spawn 标志、原子写、死 pid 改写、在飞计数、退出码主流程符合契约；问题见高-4（stdio 继承）、中-2（cancel 竞态）、中-3（pid 复用）、低-1/低-2。「run 永远停在 running」除中-2/中-3 外，后台起后即崩且无人调 status 的路径属契约内设计（不重跑），无异议。
  4. **契约兼容**：命令行参数一字未改（`build_request` diff 逐字核对）；`ConsoleRun` 追加 `pid`/`updated_at` 带 `serde(default)`（`console_api.rs:81-82`）；`Target.validation_command`、`ReplyArtifacts.source_node_id` 只追加且有往返测试（`orchestration.rs:147-148, 204-205, 1292-1312`）；首个响应带真实 `graph_id`/`node_ids`（`engine/mod.rs:1794`）。**除中-14 的文档小出入外未发现问题**。
  5. **`src/dispatch.rs` 测试改动**：仅为 `pub(crate)` 可见性 + TASK-13 测试串行化（静态互斥锁）+ 测试脚本 sleep 5→8 秒、两个超时预算 2s→4s 与 5s→15s（`dispatch.rs:1837, 10715-11210`）。生产逻辑零改动，sleep 与预算同比例放宽（仍保持 timeout < sleep 的判定关系），是合理的 Windows 并行稳定性调整，**未发现掩盖真实问题**；唯一残余风险是更宽的窗口测不出「启动变慢」类回归，可接受。
  6. **其它**：REPORT-36「照原样移植的 TypeScript 缺陷：未发现」一节与事实相符（上述均为移植偏差而非 TS 缺陷），但报告未披露任何行为差异与高-1 的越界改动；报告 §验证 的全绿结论本次按只读要求未复跑，无法背书。

  **总体结论**：主体移植（检查链、推进循环骨架、结果写回、摘要、持久化）方向正确且相当部分逐字对齐，但存在 6 项高严重度问题——其中高-2（fix_round）、高-3（盲上下文）会让真实派发的行为与窗口路径可观察地不同，高-4（stdio 继承）会让 skill 集成的核心场景失效，高-5/高-6 使「金标准钉住行为一致性」这一本期验收手段大体落空，高-1 涉及未申报的越界改动。建议修复后再交付。

