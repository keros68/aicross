# TASK-36 修正轮 R1（09-30）

依据：`tasks/review-36-glm.md`（GLM-5.3-Flash，下称 G）、`tasks/review-36-kimi.md`（kimi，下称 K），以及编排者复核（下称 O）。标「已核」的条目编排者已对照两边代码确认成立。范围、禁区、验证命令、换行规则仍按 `tasks/TASK-36-headless-engine.md`；本轮另允许改 `src-ui/src/__tests__/engine-golden.test.ts` 与重新生成 `fixtures/engine-golden/`。不 commit，不 push。完成后在 `tasks/REPORT-36.md` 末尾追加「R1」一节：逐条写改了什么、对应测试名，有意偏离单列。

## 必修（高）

**R1-1 金标准要真正钉住行为**（G-H2、K 高-5、K 高-6、O，已核）

- **TS 侧**：`runConsoleDispatch(event, context, io)` 本身可注入 `io` 与 `context`。金标准生成改为**直接驱动 `runConsoleDispatch`**：用假 `io`（`listDispatchableModels` / `listPresets` / `loadRoles` / `listConsoleRuns` / `gitChanges` 返回场景数据，`dispatchGraphStage` 按阶段返回注入结果，`previewDispatch` 返回空，`saveGraph` 与 `consoleRunUpdate` 记录每次调用）和假 `context`。夹具记录：输入（事件、资源、预设、角色、设置、已有图、git 状态）、**每一阶段实际发出的请求**、每阶段注入的结果、每次 `consoleRunUpdate` 的摘要、最终图。拒绝类场景由真实检查链产生（不再硬编码文案），检查顺序由「同时满足两条拒绝条件时报哪条」的场景钉住。`crypto.randomUUID` 与时间用确定值替身。
- **唯一有意偏离**：见 R1-2。生成夹具时，请求里的 `fixRound` 按 `isFixRoundExecution(graph, nodeId)` 计算写入（窗口控制台路径从不带该字段，这是 TS 侧的缺陷，不改 TS），在测试与报告里注明。
- **Rust 侧**：把 `start_dispatch` 的「建图 + 盖章 + 检查」拆成纯函数，把 `run_background` 的循环体拆成注入式推进函数（阶段派发、id、时钟、取消信号、落盘回调都由调用方给）。生产路径与测试走同一个函数。测试**逐夹具**：同样输入 → 拒绝结果或逐阶段请求逐字段相等 → 注入同样结果 → 每阶段摘要与最终图逐字段相等。
- **场景补齐**：盲审模式三路；执行（带 artifacts）→ 验收 NEEDS_FIX → 修正轮 → 通过（`needs-fix-pass` 现在的输入图没有带产物的执行父，修正轮根本不会生成）；修正到上限 → 遗留；验证命令失败 → 重试到上限（含验证兄弟节点被标跳过）；多附件（顺序非字母序）；续写有分叉且含遗留节点的已有图；单阶段派发抛错；备选作答后的续会话；遗留条目识别的边界文本（`3 处需要修改`、`（1）…`、`(2)…`）。保留现有全部场景。

**R1-2 `fix_round` 只给修正轮执行节点**（G-H1、K 高-2，已核）

`src/engine/mod.rs` 现在把 `all_ids` 全部放进 `fix_ids`，首轮执行节点也带 `fix_round: true`，配了验证命令时会多跑红绿核对并压掉 worktree 注记。改为只对 `isFixRoundExecution` 口径的节点置真（正文以 `[修正 N/M]` 开头或以「根据上一轮验收意见修正实现。」开头）。

**R1-3 盲节点的上下文保留全部祖先的用户消息与附件**（K 高-3，已核）

TS `graph/context.ts` 的用户消息只受「指定上游」过滤，盲节点照样带上所有祖先的 `[用户]` 段与附件；只有模型回复受 `defaultSelected` 约束。Rust `serialize_context` 把用户消息也关进了 `default_selected`，改为与 TS 一致。

**R1-4 单阶段派发抛错要停止并收尾**（O、K 中-4、G-L4，已核）

TS `startPipelineHeadless` 在派发抛错时：给没有结果的在飞节点补失败桩（`status: error`、`runner: unknown`、`stage` 取各节点自身的 `graphStage`）、写回图、**停止推进**、run 以 `failed` 收尾且 `reason` 为错误文案。Rust 现在把错误当普通结果继续交给 `reduce_step`，改为与 TS 一致。

**R1-5 后台进程不继承标准输入输出**（K 高-4，已核）

`spawn_background` 未设 stdio，调用方用管道读 `aicross dispatch` 输出时会一直等到整条流水线结束。改为 `stdin` / `stdout` / `stderr` 全部 `Stdio::null()`；加一条测试：用管道捕获 `dispatch` 输出时，命令在后台 run 结束前返回。

**R1-6 run 文件的写竞态**（G-M2、K 中-2，已核）

- 父进程在 spawn 之后不得再整写 run 文件。改为：spawn 前写一次（`status: running`，`pid` 为 0），后台进程启动后自己写入 pid。`pid` 为 0 且 `updated_at` 在 30 秒内视为「启动中」，不判死、计入在飞；超过 30 秒仍为 0 判 `failed`、`reason: 引擎进程未能启动`。
- `cancel` 只写标记文件，不写 run 文件。run 已不在 `running` 时按 TS 口径静默成功（退出 0，打印当前 run）。
- `adopt` 对没有结果的节点按 TS `setReplyAdopted` 口径不改图、正常返回。

**R1-7 Windows 下 pid 复用**（G-M3、K 中-3，已核）

run 文件追加 `pid_started_at`（后台进程自己的创建时间）。存活判定 = 进程存在且创建时间一致。Windows 用 `GetProcessTimes`；其它平台保持现状并在文档注明。

**R1-8 推进中途读写失败也要收尾**（K 中-10）

`run_background` 里任何 `?` 提前返回都要先把在飞节点补失败、run 写 `failed` 与原始错误文案，再返回。

## 应修（中低，全部照 TS 对齐）

| 编号 | 问题 | 来源 |
|---|---|---|
| R1-9 | `latest_leaf` 改为 TS `layout.ts` 口径：从根深度最大、同深取较新，跳过遗留节点 | G-M4、K 中-1 |
| R1-10 | `materials` 保首次出现顺序去重，不排序 | G-M5、K 低-9 |
| R1-11 | 验证重试到上限时，同批 draft 的「验证」兄弟节点一并写跳过说明（同 TS `closeWaitingAcceptance`） | G-M6、K 中-5 |
| R1-12 | `mark_pending` 保留原 reply 全部字段；`reset_draft` 只处理 pending 节点并保留其余字段 | K 中-6 |
| R1-13 | 请求里每个节点的 `stage` 取各自 `graph_stage`，请求级才取最小值 | G-L6′、K 中-7 |
| R1-14 | 模型查找口径：`build_request` 只按 provider 精确匹配；额度检查与备选 vendor 按 TS 两段式（先 provider 精确，再 agent+id）；备选模型判空用 `trim` | G-M1、G-L9、K 中-8、K 低-13 |
| R1-15 | `leftover_items` 照 TS 正则：`[-*•]\s+`、`\d+[.)、]\s*`、`（\d+）`、`\(\d+\)` | K 中-9，已核 |
| R1-16 | 续写已有图时 `source` 原样保留（含空），只有新建图写 `console` | G-L7、K 中-11 |
| R1-17 | 拆分上限行文案改为「本次上限 N 件。」并同样夹到至少 1；内置预设节点文本的覆盖规则照 `stamp.ts` | G-L3、K 中-12、K 低-16 |
| R1-18 | `continuation`：认旧图正文首行「（已切备选：」标记；provider 比较把缺省与空串视为相等 | G-L1、K 低-7 |
| R1-19 | `append_leftover` 复用同源遗留节点；`append_correction` 取第一个「执行」父，无产物则不生成修正轮 | G-L2、K 低-5、K 低-17 |
| R1-20 | `upstream_artifacts` 补 TS 的不可达祖先兜底 | G-L12、K 低-6 |
| R1-21 | `parse_acceptance` 尾窗与标题截断按 UTF-16 码元（复用 `utf16_excerpt` 的口径） | G-L8、K 低-11、K 低-12 |
| R1-22 | `apply_results`：有 `fallback_from` 时总写 `fallback_to`（可为空串） | G-L5 |
| R1-23 | run 摘要节点顺序按插入序（不要用 HashSet 迭代序） | K 低-3 |
| R1-24 | `active_runs` 只数 `.json` 且不以 `.` 开头的文件；`__run` 入口校验 `run_id` | K 低-1、K 低-2 |
| R1-25 | `graph_stage` 加记忆化 | G-L10 |
| R1-26 | 路径归一照 TS `normalizedPath`（含 `\\?\UNC\` → `//`）；盲附件显示名照 TS 取最后一段 | K 低-14、K 低-15 |
| R1-27 | 不再用 `canonicalize` 改写项目路径：`graph.project`、run 文件、`project_dir` 一律用用户给的路径；「项目目录不存在」检查保留，写进文档 | G-L6、K 低-4 |
| R1-28 | 删掉 `start_dispatch` 里未使用的 `original`；本次派发的原始需求文本在建图时写进 run 文件，后台直接读，不再从图上反推 | G-L16 |
| R1-29 | 测试补：退出码 3 的三种来源各一条；取消 watcher 真实启动并读到标记后对在飞节点调取消、run 收尾为 `failed / 派发已取消` | G-H2、K 中-13 |
| R1-30 | `docs/CONSOLE-CHANNEL.md`：`propose` 仍可能因窗口未运行退出 3；进程中断后图上的 pending 节点要开窗口对账；设置文件只认 JSON 数字；R1-6 的启动中口径与 R1-7 的存活判定 | G-6、K 中-14、G-L13 |

## 不改（附理由）

- **K 高-1「越界改动：删 docs/history、回退 PRODUCT.md、改三个测试」**：误报。审查快照的 `before/` 取自之后又前进过的 main（编排者的文档清理 `3e8d3ea`），实现者的工作树开自更早的 `f669aa8`，实现者没有动这些文件。分支已合入最新 main。
- **G-L15 运行中摘要保留实际遗留**：Rust 的数据更准，保留；在 R1 报告里注明为有意偏离。
- **K 低-19 选图不看窗口 tab、Unix 未 `setsid`、Job Object**：无窗口引擎拿不到 tab 状态，属设计；开发机与目标平台是 Windows，Unix 行为写进文档即可。
- **G-L13 设置文件对字符串数字的容忍度**：只在文档说明（R1-30）。
- **`src/dispatch.rs` 的测试稳定性调整**：两家都判为合理，保留。
- **`EngineRun` 与 `ConsoleRun` 两份同构类型**：维护性问题，进积压，不在本轮。

## 进度与第二阶段说明

- **已完成（`1e49125`）**：R1-1 的前端部分。`src-ui/src/__tests__/engine-golden.test.ts` 用假 io/context 驱动 `runConsoleDispatch`，`fixtures/engine-golden/` 现有 31 个夹具。**夹具与该测试文件就是契约**：字段结构以它们为准，不要改它们来迁就 Rust；确需改夹具（例如发现 TS 侧生成逻辑有误），先在报告里说明理由。夹具里请求的 `fixRound` 是按 `isFixRoundExecution` 补写的有意偏离（R1-2）。
- **第二阶段（本次）**：Rust 侧全部内容——R1-1 的 Rust 部分（建图与检查拆成纯函数、推进循环改注入式、逐夹具重放）、R1-2 至 R1-29、R1-30 文档。现有 Rust 测试 `typescript_golden_fixture_drives_rust_pure_functions` 按旧夹具写的，当前为红，用逐夹具重放的新测试替换它。

## 验证

同任务书第 7 节四条命令全绿；另外逐夹具的 Rust 测试须能在「把 R1-2 或 R1-3 的修复临时改回去」时变红（编排者会抽查）。
