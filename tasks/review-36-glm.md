核对完毕。我读了两边仓库的对应文件（任务书、报告、`src/engine/mod.rs`、五处改动、TS 参考实现 `console-channel.ts` / `graph/{dispatch,context,stamp,validate,layout,types,swarm}.ts` / `dispatch-settings.ts`、金标准测试与夹具、新旧 `console_cli.rs` / `console_api.rs`）。我没有运行任何命令（本轮只有读文件工具），验证结论均来自静态对照；REPORT-36 里声称的命令结果未复跑。

先给结论概览：契约兼容（问题 4）与 `dispatch.rs` 测试改动（问题 5）未发现问题；主循环、文案、盖章、上下文拼装大体忠实，但存在 1 个会改变派发行为的高危偏差（`fix_round`）、任务书第 5 节的测试要求大面积未真正满足，以及若干中低危不一致与两个会导致 run 卡在 running 的竞态。

---

## 高

**H1. 无窗口引擎给所有节点发了 `fix_round: true`，参考实现的控制台通道从不发这个字段**

- 证据：`src/engine/mod.rs:1864`（`let mut scope = all_ids.clone();`，all_ids 即本次全部新建节点）与 `src/engine/mod.rs:922`（`fix_round: fix_ids.contains(id)`）——首个批次每个节点都带 `fix_round: true`。参考实现 `src-ui/src/console-channel.ts:428-438` 的 `buildRequest` 调 `buildNodeRequest` 时根本不传 `fixRound`（`dispatch.ts:1207-1209` 只在 `parts.fixRound` 为真才带），即窗口控制台通道的请求**永远没有** `fixRound`。后端该字段有实际行为：`src/dispatch.rs:4159-4163`（`fix_round && role==执行 && write∈{单写者,worktree} && validation_command 非空` 时跑红绿核对，含回滚恢复），`src/dispatch.rs:2931-2934`（`!fix_round` 才给 worktree 执行节点写「worktree 节点未跑验证命令，采纳合并后再验」注记）。
- 后果：只要用户配了验证命令，无窗口派发会对首轮执行节点做红绿核对（含恢复回滚）、抑制 worktree 注记——窗口同一预设不会；结果里多出 `unpinned` / `validation_failed`，再喂给 `reduce_step` 触发窗口不会有的验证重试/遗留，整条流水线走向分叉。且金标准测试把这个偏差**掩盖**了：`engine/mod.rs:2252` 用空 `&HashSet::new()` 调 `build_request`，`engine/mod.rs:2204-2219` 的 `request_defaults` 又把 `fixRound == false` 删掉再比——夹具（`economy-pass.json` 的 requests 里无 `fixRound`）与真实 `run_background` 产出的请求（全为 true）互相矛盾，测试却通过。
- 建议：`run_background` 里 `fix_round` 的判定改为「仅 `reduce_step` 新生成的修正轮执行节点」（即 TS `isFixRoundExecution` 的口径：user.text 以 `[修正 n/m]` 或「根据上一轮验收意见修正实现。」开头），不要把初始 `all_ids` 混进 fix_ids；同时让金标准测试真实驱动一次 `run_background` 的请求构造路径，使该字段进比对。

**H2. 任务书第 5 节的测试要求大部分没有做到**

- 证据：
  - 任务书要求「Rust 测试读同一批夹具，驱动纯函数**与推进循环**……断言请求与最终图逐字段相等」。实际 `src/engine/mod.rs:2228-2234` 对 24 个夹具只断言四个键存在（冒烟），深比对只有 `economy-pass.json` 一个场景（`engine/mod.rs:2239-2283`），且只调了 `build_request` / `apply_results` / `node_summary` 三个纯函数，`run_background`（推进循环、批次键、重复批次停止、缺目标停止、请求上限、取消、异常收尾）零覆盖。夹具里 `needs-fix-pass`、`validation-retry-limit`、`duplicate-results` 等场景的 Rust 侧变换从未执行。
  - 任务书要求的 Rust 单测里两条缺失：「cancel 标记被后台线程读到」——`engine/mod.rs:2173-2179` 只验证了标记文件写进去能 `exists()`，watcher 线程没测；「退出码 3 的三种来源」——全仓没有任何对应测试（run 目录不可写、后台起不来这两条没有用例）。
  - TS 侧金标准也没有按任务书执行：「每一条派发前拒绝各一例（检查顺序也要钉住）」实际是把 10 条文案**硬编码**进数组写成夹具（`engine-golden.test.ts:122-128`），没有跑 `runConsoleDispatch` 的检查链，顺序是数据不是行为——Rust 检查顺序就算错了也拦不住；「每阶段发出的请求」只对输入图节点各构造了一次请求（`engine-golden.test.ts:53`），修正轮/验证重试等后续阶段的请求没有进夹具；`runSummary` 用的是测试里本地另写的 `summaryOf`（`engine-golden.test.ts:43-49`），不是真正的 `nodeSummary`——丢了 `quota_resets_at` 和 `skipped_reason` 兜底（`console-channel.ts:82-93`），所以 `quota-exhausted` 夹具的 runSummary 与真实摘要形状不一致，这套「契约」本身就是失真的。
- 后果：本期唯一的行为一致性保障（金标准）对 Rust 引擎只有单点、单层的约束；H1 这类请求构造偏差正是从覆盖缺口里漏掉的。
- 建议：Rust 侧把 `run_background` 的循环体提为可注入（阶段结果、id、时钟、取消标记由测试给），对 `needs-fix-pass` / `validation-retry-limit` / `duplicate-results` / 缺目标 / 请求上限逐夹具跑全程并逐字段比；TS 侧 precheck 场景改为真实调用前置检查链（至少把检查顺序用执行结果钉住），`runSummary` 换用 `console-channel.ts` 的 `nodeSummary` 重新生成夹具；补 watcher 与退出码 3 两条单测。

## 中

**M1. `build_request` 查模型时加了 provider 回退，参考实现只按 provider 精确匹配**

- 证据：`src/engine/mod.rs:862-873` 先按 agent+model+provider 找、找不到退回 agent+model；参考 `dispatch.ts:1179-1182` 的 `buildNodeRequest` 只有精确匹配一次（provider 不匹配就拿不到 model，`vendor`/`billingSource`/`tier` 落空）。
- 后果：目标 provider 与资源库不一致时（典型：角色默认没填 provider、模型条目 provider 非空），Rust 会发出真实的 `vendor`/`billingSource`/`tier`，窗口发出空串，两边请求不同，计费与留痕口径跟着分叉。
- 建议：删掉 `or_else` 回退，照 TS 只做一次精确匹配（`fallbackSpecOf` 那种两段式才允许回退）。

**M2. run 文件两处「读-判-写」竞态，能把 `done` 覆写回 `running`**

- 证据：① `src/engine/mod.rs:1781-1792`：父进程 spawn 后无条件再写一次 run（填 pid）；若后台进程在两次写之间已把 run 写成终态（图可派节点为空时几乎立刻 `done`），父进程的写入把终态盖回 `running`。② `src/engine/mod.rs:2099-2109`：`cancel` 先 `read_run` 判 `running` 再写标记和 run，与 `run_background` 的收尾写（`engine/mod.rs:2044-2055`）并发时，cancel 的写会把刚落盘的 `done` 盖回 `running`，且 2056 行的标记删除可能发生在 cancel 写标记之前，留下孤儿 `<run_id>.cancel`。
- 后果：被盖写的 run 依赖 pid 死亡自愈（`read_run`，`engine/mod.rs:238-243`），但自愈给的 reason 是「引擎进程已退出」而非真实结论，用户看到错误结论；孤儿标记永久留在 runs 目录。
- 建议：父进程不再二次整写 run，spawn 改为把 pid 作为参数传给 `__run` 由后台自己落；`cancel` 写完标记后改为轮询 run 终态而不是直接改写 run 文件（或写前重读一次并比较 `updated_at`）。

**M3. Windows 下 pid 复用会让死掉的引擎被判定存活，run 永远停在 running**

- 证据：`src/engine/mod.rs:260-274` 存活判定只有 `OpenProcess` 成功与否；`engine/mod.rs:1571-1581` 在飞上限按「running 且 pid 存活」计数；自愈也只走这条路（`engine/mod.rs:238`）。Windows pid 回收很快，后台引擎被杀后 pid 被别的进程占用时，`read_run` 永不自愈、`status --wait` 只能超时，且该 run 持续占用在飞名额（上限默认 3，占满后所有新派发被拒）。
- 后果：正是任务书第 3 节点名要查的「run 永远停在 running」路径；无需用户误操作，杀进程即可触发。
- 建议：run 文件里同时存进程启动时间（`QueryFullProcessInformation`/`system_process_info` 或创建文件时记录），存活判定加「启动时间一致」；或后台周期心跳 `updated_at`，超过若干倍超时即判死。

**M4. `latest_leaf` 口径与参考实现不同，续写图时材料可能挂错节点**

- 证据：TS `layout.ts:68-83` 取「根出发深度最大的节点（同深取较新）」；Rust `src/engine/mod.rs:346-359` 取「无子节点里 created_at 最新」。多分支图上两者不同：浅分支的叶子可以比深分支叶子更新。
- 后果：窗口续写和命令行续写会把新材料挂到不同父节点，图结构与后续上下文分叉。
- 建议：照 TS 实现按 `depthMap` 口径移植（或至少按「最大深度、再 created_at」排序选取）。

**M5. `materials` 顺序：Rust 排序去重，TS 保插入序**

- 证据：`src/engine/mod.rs:720-721` `materials.sort(); materials.dedup();`；TS `context.ts:226-227` `[...new Set(materials)]` 保首次出现顺序。
- 后果：附件多于一个且顺序非字母序时，请求的 `materials` 数组两边不同；金标准夹具没有多附件场景，测不出。
- 建议：改为按出现序去重（`HashSet` 记已见 + 保留原序）。

**M6. 验证重试到顶时，Rust 没有把「验证」兄弟节点标成已跳过**

- 证据：`src/engine/mod.rs:1422-1449`（`count >= limit` 分支）只把等待中的验收节点写 `skipped_reason`；TS `closeWaitingAcceptance`（`dispatch.ts:716-733`）对验收节点**和**同批 draft 的验证兄弟一起标。注意 `append_validation_retry` 里成功重试的那条路径 Rust 是对齐的（`engine/mod.rs:1310-1320`），漏的只有到顶关闭这条。
- 后果：「验证命令失败 → 重试到顶」场景的最终图与 TS 不一致：验证兄弟节点缺「上游验证未通过」标记，结果屏与下次打开的节点卡显示不同。
- 建议：把该分支改成复用与 TS `closeWaitingAcceptance` 同构的函数（含兄弟节点）。

## 低

**L1. `continuation` 漏了旧图首行备选标记的判断**：TS `dispatch.ts:152-159` 经 `fallbackNoticeOf`（`dispatch.ts:222-228`）把正文首行「（已切备选：…）」也视为备选作答、不续会话；Rust `engine/mod.rs:806-812` 只查 `fallback_from` 字段。旧格式图的备选回复会续上备选通道的 session，泄漏到共享子节点。另外 `engine/mod.rs:810` 直接比较 `Option<String>`，`None` 与 `Some("")` 不等，TS 用 `?? ''` 视为相等——provider 一边缺省一边空串时不续会话。建议补首行判断并把 provider 归一为空串比较。

**L2. 遗留节点不复用同源的已有节点**：TS `dispatch.ts:793-794` 先找 `leftover.source_node_id` 相同的节点复用 id；Rust `append_leftover`（`engine/mod.rs:1230-1267`）总是新建。同一验收节点二次触发到顶遗留时会出两个遗留节点。建议加复用查找。

**L3. 内置预设节点文本覆盖规则不一致**：TS `stamp.ts:85-91` 对 `PRESET_NODE_TEXTS` 命中的索引**无条件覆盖**（含写成空串），索引超出不覆盖；Rust `preset_text`（`engine/mod.rs:399-423`）对 economy/blind 未命中索引回退到 `target.preset_text`，对 jury 超出索引写成空串。当前内置预设目标没有 `preset_text`，暂无可见差异，但口径不同。另外拆分上限行文案不同：Rust「本次最多拆分 N 件。」（`engine/mod.rs:419`）vs TS `swarmLimitLine`「本次上限 N 件。」（`swarm.ts:27-29`）。建议逐条照 TS 对齐并复用同一文案常量。

**L4. 派发整体失败时的错误结果桩与 TS 不同**：TS `console-channel.ts:459` 给每个失败桩补 `runner: 'unknown'`、stage 取各节点自身 `graphStage`；Rust（`engine/mod.rs:1956-1975`）runner 为空（图中不写 `runner` 字段）、stage 用批次最小值。引擎级失败后图的 reply 形状与窗口不同。建议补 `runner: "unknown"`；stage 字段影响面小，可保持但记录取舍。

**L5. `fallback_to` 空串处理**：TS `dispatch.ts:295-297` 在 `fallbackFrom` 存在时恒写 `fallback_to`（可为 `''`）；Rust `engine/mod.rs:1047-1048` 空串时省略字段。图 JSON 形状不同（下游只读 `fallback_from`，无功能影响）。建议对齐为「有 from 即写 to」。

**L6. 新增了任务书之外的拒绝项「项目目录不存在」，且排在在飞上限之前**：`engine/mod.rs:1612-1614`；TS 无此检查。另外 `fs::canonicalize`（`engine/mod.rs:1585-1590`）把 `\\?\` 前缀路径写进 `graph.project` / run 文件 / `project_dir`。功能上能跑（窗口侧 `normalizedPath` 会剥前缀），但这是未在任务书出现的契约新增，拒绝顺序也与参考不同。建议在文档写明这条拒绝，或仅在目录不存在时保底报错而不改写项目路径。

**L7. 既有图 `source` 为空时被改写成 `console`**：`engine/mod.rs:1660-1668`；TS `console-channel.ts:372-373` 保留原值（含 undefined）。用户自建的图经命令行续写一次后会被打上 console 来源。建议空值保持为空。

**L8. 截断口径：字符数 vs UTF-16 码元**：`parse_acceptance` 尾窗 1200 取的是 char（`engine/mod.rs:1056-1065`），TS `slice(-1200)` 是码元（`dispatch.ts:406-410`）；`fillRoot` 标题取 20 char（`engine/mod.rs:377`）vs TS 20 码元（`mutate.ts:44-46`）。仅当边界落在增补平面字符（emoji 等）时结论/标题不同。`text_excerpt` 已正确按码元（`utf16_excerpt`）处理，这两处建议统一用同一套码元截断。

**L9. 额度检查查模型不带 provider 口径**：`engine/mod.rs:1729-1746` 用 `find_model`（agent+id，`engine/mod.rs:1522-1526`）；TS `quotaBlockedNodeIds`（`dispatch.ts:1037-1050`）先精确 provider 再回退。同一 agent+model 挂在多个 provider 下时可能判到另一条目的额度。建议照 TS 两段式。

**L6′. `build_request` 每节点 stage 用批次最小值**：TS 每节点 `stage: graphStage(sourceGraph, id)`（`console-channel.ts:436`），请求级 stage 才是 min（`console-channel.ts:430`）；Rust 全部写 min（`engine/mod.rs:852-856, 897`）。混批（不同深度分支同时可派）时节点 stage 失真；后端只把它落进结果与 state 文件，影响是记录性的。

**L10. `graph_stage` 无记忆化**：`engine/mod.rs:551-570` 每个父分支克隆 seen 重算，TS `dispatch.ts:581-594` 有 memo。菱形图深时指数膨胀；修正轮叠加 accept 节点会加深菱形。建议加 memo。

**L11. 上下文 DFS 子节点排序与 TS 不同**：Rust 按 created_at,id 排（`engine/mod.rs:588-594`）；TS `childrenOf` 按对象键序（`stamp.ts:96-102`）。Rust 存图用 `HashMap`（`orchestration.rs:35`），键序随机——窗口重开同一张图后 TS 的遍历序与引擎不一致，菱形汇合处的上下文块顺序两边可能不同。建议图序列化按 created_at 排序键（顺带解决窗口侧漂移）。

**L12. 不可达祖先的产物兜底缺失**：TS `upstreamArtifactsOf` 末尾把「允许集内但从根不可达」的祖先产物补进结果（`context.ts:180-184`）；Rust `upstream_artifacts`（`engine/mod.rs:748-788`）只走 `context_order`（从根可达），损坏/恢复图会少带产物。建议补同款兜底循环。

**L13. 设置读取对「数值字符串/布尔/null」的容忍度不同**：TS `Number(parsed.x)` 会把 `"12"`、`true`、`null` 转成数（`dispatch-settings.ts:44-69`）；Rust `as_f64` 直接取默认（`engine/mod.rs:85-90`）。仅影响手写怪异设置文件，默认值方向一致。可不动，建议在文档说明只认 JSON 数字。

**L14. `cancel` / `adopt` 对非法状态比 TS 严格**：TS 对非 running 的 cancel 静默成功（`console-channel.ts:491-496` 无 else 报错）、对无 reply 节点的 adopt 原样返回（`dispatch.ts:196-206`）；Rust 分别报「run 当前不在运行」「节点没有结果」退 1（`engine/mod.rs:2101-2103, 2077-2084`）。skill 侧可能把本可静默的情况当失败。建议至少在文档写明。

**L15. 运行中 run 更新的 `leftover` 字段与 TS 不同**：TS `onStage` 更新时恒写 `leftover: []`（`console-channel.ts:455`）；Rust `update_running` 保留图上实有遗留（`engine/mod.rs:1823-1835`）。Rust 的数据更准，属「有意偏离」，建议在报告里注明即可。

**L16. 死代码**：`start_dispatch` 里算出的 `original` 未使用（`engine/mod.rs:1676-1680` 计算行 + `1793` `let _ = original;`），run 请求实际在 `run_background` 里按「created 节点的 material 父」重推（`engine/mod.rs:1870-1883`）。正常流等价，但绕了一圈；建议直接删掉 `start_dispatch` 里的计算，或把 material 文本随 run 落盘以免重推逻辑与 `runRequestOf` 将来漂移。

---

## 按题逐项结论

1. **行为一致性**：派发前检查的顺序（除 L6 新增的项目目录检查在最前）与全部文案逐字一致；盖章、上下文拼装、角色模板、写护栏、盲前缀、结果写回、修正轮、验证重试、遗留、run 摘要与终态规则在主路径上一致。不一致处即上列 H1、M1、M4、M5、M6、L1–L5、L6′、L8–L12、L15，其中 H1 会实际改变派发行为。
2. **测试要求**：夹具文件 24 个、场景名齐全（含 10 条拒绝与 13 个行为场景），但如 H2 所述：Rust 只深比对 1 个场景且未驱动推进循环；cancel watcher 与退出码 3 两条单测缺失；TS 侧 precheck 是硬编码文案、后续阶段请求未进夹具、runSummary 是简化副本。任务书第 5 节「真正满足」的部分只有：设置三种异常、原子写、死 pid、adopt、类型往返（`orchestration.rs:1293-1313`，注意它验证的是 Rust 写读往返，未字面使用「前端写出的文件」，因图 JSON 本就是 snake_case 镜像，可接受）。
3. **进程与持久化**：后台启动/脱离（`CREATE_NO_WINDOW|DETACHED_PROCESS`）、隐藏子命令、run 原子写（同目录临时文件 + `MoveFileExW(REPLACE_EXISTING|WRITE_THROUGH)`、失败清 tmp）、取消标记与 watcher（500ms 轮询、对在飞节点走 `cancel_dispatch_node`、pending 改 failed/「派发已取消」）、死亡自愈（running+pid 死 → failed/「引擎进程已退出」，不重跑）、在飞计数（running 且 pid 活）、退出码 0/1/2/3 语义，均与任务书一致且 CLI 实现正确（`console_cli.rs:27-131`）。例外即 M2 的两处写竞态与 M3 的 pid 复用；另有后台中途 `?` 退出时 run 短暂停留 running（pid 死后可自愈，可接受）。
4. **契约兼容**：未发现问题。CLI 参数逐字未动（新旧 `build_request` 一致，别名映射、BOM、拒 `--text`、`--timeout` 相同）；`propose` 仍走窗口 HTTP；`ConsoleRun` 只追加 `pid`/`updated_at`（带 `serde(default)`，`console_api.rs:81-82`）；`Target.validation_command`、`ReplyArtifacts.source_node_id` 均为 `Option + serde(default) + skip`，有往返测试；`DispatchNodeRequest` 新字段全带 `serde(default)`，只追加。一个维护性提示：`engine::EngineRun/RunNode` 与 `console_api::ConsoleRun/ConsoleRunNode` 是字段相同的手抄副本，今后改一处易漏另一处，建议合并为一个类型。
5. **dispatch.rs 测试改动**：未发现问题。`task13_repo` 加了进程级互斥锁（`dispatch.rs:10715-10730`，含 poisoned 恢复），两个超时窗口 2s→4s（`dispatch.rs:11014`）、5s→15s（`dispatch.rs:11027`）只放宽等待上限，断言的 verdict/note 原样保留，不掩盖产品缺陷；生产代码侧对 `dispatch_stage_core` 仅 `fn`→`pub(crate)`。
6. **其它**：除 M2/M3 的卡死与结论失真、H1 的行为分叉外，未见会丢数据的路径（图与 run 均整文件原子替换，窗口与命令行同图并发写已按任务书记为不受保护并写入文档）。窗口重开后图上遗留的 `pending` 节点只能靠窗口的对账/重置流程恢复，命令行无对应入口——与「不自动重跑」的定案一致，但建议在 `docs/CONSOLE-CHANNEL.md` 加一句「进程中断后 pending 节点需开窗口处理」，否则 skill 用户没有出路。