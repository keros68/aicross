复核完成。以下为最终审查结论。

**验证限制**：全程只读，未运行构建或测试；REPORT-36.md 里的 cargo/vitest 通过数（531 / 729）与「回改变红」记录按声明采信、未经我复跑，其余结论均以当前代码与夹具原文为证。

---

## 1. R1-1 至 R1-30 逐条结论

| 条目 | 结论 | 证据 |
|---|---|---|
| R1-1 拆纯函数 | 已修 | `plan_dispatch`（src/engine/mod.rs:1992，PlanInput 注入 models/presets/roles/active_runs/existing/git_dirty_count）；`run_pipeline`（mod.rs:2249，PipelineSink 注入落图/摘要/派发/在飞通知，id/时钟/取消信号入参）。生产装配：start_dispatch mod.rs:2581-2597 调 plan_dispatch，run_background→drive_pipeline→run_pipeline+FileSink（mod.rs:2733-2754）。惰性更新按 TS 复刻为队列+排空（preview 前、dispatch 返回后、收尾 save 后）。golden.rs:400-459 用同一对函数重放 |
| R1-2 fixRound 口径 | 已修 | is_fix_round_execution（mod.rs:1092）：执行角色且正文以 `[修正 N/M]`（手写解析，语义与 TS 正则等价）或「根据上一轮验收意见修正实现。」开头；build_request 只对该口径置真。夹具 `fixRound: true` 恰好只出现在 5 个修正轮场景（needs-fix-pass.json:586、reset-draft.json:586/673 等 6 处），通过型夹具无此字段 |
| R1-3 上下文序列化 | 已修 | serialize_context（mod.rs:854-955）：用户消息只受「指定上游」过滤；盲节点带全部祖先 `[用户]` 段与附件；仅非直接父的模型回复受 defaultSelected 约束。economy-pass 第三阶段验收 prompt 含 `[用户]\n实现任务`（夹具钉住） |
| R1-4 派发抛错收尾 | 已修 | run_pipeline dispatch Err 分支：全部在飞节点补失败桩（error / runner unknown / isolation 部分 / stage 取各节点 graph_stage）、reason `Error: {msg}`、run 终态 failed；stage-throw.json 逐字段钉住 |
| R1-5 句柄不继承 | 已修 | spawn_detached Windows 分支（mod.rs:2653-2690）直接 CreateProcessW，bInheritHandles=false，CREATE_NO_WINDOW\|DETACHED_PROCESS\|CREATE_NEW_PROCESS_GROUP；非 Windows（:2693）std 三路全 null。测试 detached_grandchild_does_not_hold_the_pipe + spawn_detached_missing_exe_is_error（helper 标 `#[ignore]`，属 12 个 ignored 之列，我未复跑） |
| R1-6 run 文件时序 | 已修 | start_dispatch spawn 前写一次 run（running、pid:0，mod.rs:2608-2621），spawn 失败才经 fail_run_before_spawn（:2637）改写；run_is_active（:273）pid==0 走 30 秒启动窗口（within_startup_window :267）；cancel（:2877）只写标记、非 running 静默成功；adopt（:2851）无 reply 不改图。4 个对应测试齐 |
| R1-7 pid 复用核对 | 已修 | EngineRun.pid_started_at（mod.rs:183）；own_process_started_at（:359，GetCurrentProcess 创建 FILETIME 十进制串）；process_alive（:335）：OpenProcess + GetProcessTimes，创建时间一致才算活，记录缺失/读不到退化为存在性；非 Windows（:310）只存在性。测试 pid_reuse_guard_checks_creation_time |
| R1-8 推进失败收尾 | 已修 | run_background（mod.rs:2769-2795）：Err 时图上仍 pending 的节点补 failed（error_kind error）、run 落 failed 与原始文案后原样返回。无直接单测（报告已如实声明；逻辑与 stage-throw 图内收尾同形） |
| R1-9 latest_leaf | 已修 | mod.rs:492：跳过遗留节点、深度最大、同深 created_at 严格较新、插入序迭代，与 layout.ts 一致；有单测 |
| R1-10 materials 首现序 | 已修 | serialize_context 内按首现去重、不排序；multi-attachment 夹具钉住 |
| R1-11 验证兄弟跳过 | 已修 | close_waiting_acceptance（mod.rs:1729）把同批 draft 的验证兄弟一并写 skipped_reason；单测 + validation-retry-limit 夹具 |
| R1-12 pending/draft | 已修 | mark_pending（:1230）保留原 reply 全字段只改 status；reset_nodes_to_draft（:1243）只处理 pending；duplicate-results / reset-draft 夹具钉住 |
| R1-13 逐节点 stage | 已修 | build_request：节点 stage 各取 graph_stage、请求级取最小（mod.rs:1151 附近）；needs-fix-pass 钉住修正轮节点 stage |
| R1-14 模型查找口径 | 已修 | build_request 单段 provider 精确查找；额度检查与备选 vendor 两段式（先 provider 精确再 agent+id，trim 判空）；precheck-08 / precheck-order-quota-limit / fallback-continuation 夹具钉住 |
| R1-15 遗留条目解析 | 已修 | leftover_items（mod.rs:1506）：`[-*•]`+空白、数字+`./)、`、`（N）`、`(N)`，无条目行时整段按行兜底前 20 行，与 TS 正则逐形核对一致；单测 + 两个夹具 |
| R1-16 source 保留 | 已修 | 只有新建图写 `console`；continue-existing-graph.json 钉住续写图级 source 保持为空（finalGraph 无 source 键）、新节点带 `source: "console"` |
| R1-17 拆分上限与预设文本 | 部分 | preset_node_text（mod.rs:571）「本次上限 N 件。」max(1)；preset_node_texts（:581）jury/economy/blind 越界不覆盖，实现与 stamp.ts 一致，economy/blind 的覆盖文本有夹具钉住。但实现正确不等于钉住：31 个夹具的 input presets 只含 shared/economy/blind，**拆分上限行与 builtin-jury 覆盖无任何夹具或单测覆盖**，报告「各内置预设场景」的覆盖声称不成立 |
| R1-18 备选续跑 | 已修 | fallback_notice（:1038）认「（已切备选：」；continuation（:1053）provider 缺省与空串视为相等；单测 + fallback-continuation 夹具 |
| R1-19 遗留复用/修正轮 | 已修 | append_leftover（:1563）复用同 source_node_id（IndexMap insert 保位，与 TS 展开覆写等价）；append_correction（:1386）取第一个执行父、无产物不生成修正轮；单测 + fix-limit-leftover / needs-fix-pass 夹具 |
| R1-20 产物兜底 | 已修 | upstream_artifacts（:970）DFS 序收集 + 允许集未走到节点兜底，按节点 id 判重（等价于 TS 的对象引用判重）；单测 |
| R1-21 验收判定窗口 | 已修 | parse_acceptance（:1357）先按 UTF-16 码元取尾 1200 再大写、按词界取最后 PASS/NEEDS_FIX；单测（700 个 emoji 把 PASS 顶出窗口） |
| R1-22 结果字段 | 已修 | apply_results（:1259）：fallback_from 非空总写 fallback_to（可为空串）、quota_resets_at 滤空串；fallback-continuation / quota-exhausted 夹具钉住 |
| R1-23 插入序 | 已修 | Graph.nodes 改 IndexMap（orchestration.rs:37）；node_summary（:392）、waiting_acceptance（:1605）、遗留复用（:1563）等全部按插入序，与 TS Object.values/Object.keys 语义对齐；全部夹具的 nodes 顺序逐字段比对 |
| R1-24 run 目录卫生 | 已修 | active_runs 只数 `.json` 且文件名不以 `.` 开头（有单测）；valid_id（mod.rs:200）+ `__run` 入口校验（console_cli.rs:38-43，缺失或非法退出 2）。基线只查缺失不查格式，此项为净新增 |
| R1-25 graph_stage 记忆化 | 已修 | mod.rs:742 单调用内 memo + 共享 seen 回溯（与 TS 按路径 seen 等价） |
| R1-26 路径归一 | 已修 | normalized：大小写不敏感剥 `\\?\UNC\`→`//`、`\\?\`，反斜杠转正、去尾斜杠、小写；blind_attachment_name 取最后一段、空则回原路径；单测 normalized_strips_extended_prefixes |
| R1-27 不改写路径 | 已修 | start_dispatch（mod.rs:2560-2562）：不做 canonicalize，`is_dir` 检查最前；文档 docs/CONSOLE-CHANNEL.md:175 已写「按原文使用」 |
| R1-28 原始需求入 run | 已修 | EngineRun.original_request（:186）；start_dispatch 写入（:2615）；drive_pipeline 直接读 run（:2809 附近），不从图上反推 |
| R1-29 取消 watcher | 已修 | watch_cancel（mod.rs:2706-2725）：500ms 轮询标记，命中置位并对在飞节点调 cancel_dispatch_node；收尾清标记。三个取消场景单测 + 退出码 3 三来源单测 |
| R1-30 文档 | 已修 | docs/CONSOLE-CHANNEL.md:171（original_request/pid_started_at）、:173（30 秒窗口、存活判定、pending 对账）、:175（路径原文）、:177（设置只认 JSON 数字）、:188（退出码 3 含 propose）逐条核对在文 |

**总评**：29 条已修，R1-17 部分修（实现在、证据声称过头）。逐条与代码核对未发现「报告称已修而代码没有」的情况。

## 2. 金标准是否真钉住行为

**是，机制成立，仅覆盖面有两处声明的偏差。**

- 前端侧（src-ui/src/__tests__/engine-golden.test.ts）真驱动 `runConsoleDispatch`：fake io/context、冻结 Date（2026-09-30）、计数器 UUID，31 个夹具 = 21 场景 + 10 前置拒绝，AICROSS_WRITE_GOLDEN=1 时写盘。
- Rust 侧（src/engine/golden.rs:472）用同一批夹具驱动**生产函数** `plan_dispatch` + `run_pipeline`：RecordingSink 对每次派发比对节点数与 nodeId token 一致性（golden.rs:282-301），记录请求整形后与夹具逐字段比对；runUpdates 逐条、finalGraph 逐字段；另断言注入结果用完、收尾落图即返回图、终态 reason 等于最后一条摘要的 reason（golden.rs:440-453）。唯一 注入 的缝是 dispatch 本身（镜像 TS 的 deps.dispatch 替身），即金标准钉的是「引擎发什么、拿到结果后做什么」，dispatch 内部另有 dispatch.rs 自身测试——这是设计上的合理切缝，不削弱「Rust 侧语义=TS 参照」的主张。
- 归一化不过松：UUID/时间戳按首现序换 token，token 相对顺序被钉住（阶段间 +1ms 的 created_at 偏移错序会变红）；数组顺序全钉（请求 nodes、runUpdates、leftover、最终图节点次序）；`strip_nulls`/`strip_false_flags` 是对齐 TS undefined 与条件 spread 的 IPC 边界差异而非放宽，多余的键会被 first_diff 抓出；swarmLimit 从请求 settings 剥掉是因为 TS 请求本就不带，其效果经「本次上限 N 件。」文本另钉。fixRound 口径差异（TS 窗口路径缺陷）按修复后口径录制且报告已声明，夹具可捕捉回改。
- 检查顺序场景由真链产出：precheck-order-inflight-preset.json 以「在飞已满 3 + preset missing」驱动 plan_dispatch，钉住先报「同时在飞 run 已达上限 3」，plan_dispatch 正是生产 start_dispatch 调用的同一函数（mod.rs:2597）。
- 覆盖缺口：夹具不含 builtin-arena/jury/swarm 与拆分角色（grep 全部 31 个夹具证实），对应 R1-17 的覆盖缺口；「未移植窗口侧 5 秒守卫」报告已声明且理由成立。

## 3. 新问题（按严重程度）

**高**：未发现问题。

**中**：未发现问题。

**低**：

1. **pid==0 且 updated_at 在未来时，启动窗口永不超时**。证据：mod.rs:267-269，`(Utc::now() - t) <= 30s` 对未来时间戳恒真。后果：系统时钟回拨后，一个后台进程没报上来的死 run 会一直保持 running 并计入在飞上限，阻塞新派发，直到时钟追上该时间戳。建议：`t` 显著大于 now（如超过 60 秒）也按过期处理。
2. **后台 30 秒未报到会把活 run 误标 failed，随后被后台自行写回**。证据：mod.rs:291-301（read_run 改判 failed 并落盘）与 FileSink.update（:2737-2745，后台下次更新即覆盖回 running）。触发条件是后台进程启动耗时超过 30 秒（杀软扫描等极端情况；正常为毫秒级）。后果：中间态错报「引擎进程未能启动」，skill 若据此重派会重复执行；随后自愈。这是任务书规定窗口口径的固有取舍，建议在 CONSOLE-CHANNEL.md 补一句「误判后后台报到会自行恢复 running」。
3. **报告两处改动描述与基线对不上**。证据：REPORT-36.md:80 称 orchestration.rs「删 stamp」，基线里无 `fn stamp`（grep 证实）；称 dispatch.rs「dispatch_stage_core 成功后清理取消注册表」，而 `clear_cancelled` 在基线已在同一位置（before dispatch.rs:3527 → after:3528，同在 run_one_with_activity 末尾）。相对所给基线，dispatch.rs 可观察的改动只有 runner_note 加 `#[serde(default)]`（dispatch.rs:235-236）。不影响代码正确性，属报告失真（可能描述的是基线已含的阶段一改动）。
4. **CreateProcessW 命令行拼装无转义（提示，非缺陷）**。证据：mod.rs:2662-2665，朴素 `"{}"` 包裹。当前实参只有 `__run` 与刚生成的 UUID、exe 为自身路径，不含引号空格问题，风险为零；若日后把用户可控文本放进参数会踩坑。建议加一行注释写明这个前提。

针对任务书点名追问的 pid/窗口判定，结论：除上述低 1、低 2 外，其余路径不会误判——死 run（pid 非 0）在 Windows 有创建时间核对不会因 pid 复用被误判活；updated_at 解析不出按过期；非 Windows 退化为存在性检查且文档已注明。

## 4. 清单外改动的必要性与影响

- **Cargo.toml indexmap + serde_json preserve_order**（Cargo.toml:47-52）：金标准重放必需（JSON 键序需与 JS 插入序对齐）。窗口路径兼容：Graph 经 Tauri `save_graph`/`load_graph`（orchestration.rs:872/:877）整体 serde 往返，前端本就按 JS 对象插入序读写；全量 grep 核对后 engine 之外没有按迭代顺序消费 `Graph.nodes` 的生产代码；preserve_order 此前已由 schemars 传递开启（Cargo.toml 注释自述），显式钉住不改变现有二进制行为。结论：必要且安全。
- **orchestration.rs**：新增 `raw_presets_from`（:758）供 start_dispatch（mod.rs:2565）使用是必要的——`presets_from` 会先套 fill_role_defaults，与引擎自己的 apply_role_defaults（:660，对擂台/盲审/评审整份跳过、命中角色五字段无条件覆写）叠加会双重填默认，偏离 TS 口径。Graph.source 的 skip_serializing_if 与 R1-16 对应。图 JSON 往返测试在（orchestration.rs:1303、:1477）。无破坏其他调用方。
- **dispatch.rs**：相对基线仅 runner_note `serde(default)`，只影响反序列化（夹具注入结果缺 runnerNote 键时兜底），不改变序列化输出。其余（dispatch_stage_core、取消注册表、结构体 serde 属性）与基线逐点核对一致。
- **console_cli.rs**：`__run` 增加 valid_id 校验与「run_id 无效」文案（:38-43），退出码映射与基线一致（Unavailable→3，:69）。

## 5. 其他错误结果 / 数据丢失 / 静默失败

**未发现问题。** 重点核对过：read_run 改判失败走原子写（write_json_atomic，mod.rs:207，tmp+替换）；取消标记在收尾删除；spawn 失败与 exe 定位失败都会把 run 落 failed（不留假 running）；adopt/cancel 对无效 run_id 先过 valid_id；golden 比较为逐字段 + 多余键检测，不会静默放过新增字段。