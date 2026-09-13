# TASK-04 经济流水线：阶段传递、执行产物交付验收、闭环轮次、G4

先读 `AGENTS.md`、`docs/PRODUCT.md`「经济模式」「自定义工作流·扇入节点收什么」、`tasks/TASK-02`、`TASK-03` 与对应 REPORT。本期把流水线引擎做通：出方案 → 执行 → 验收。

## 0. 与对话图和第 3 期的衔接（2026-09-13，优先于下文旧说法）

第 2 期把中栏做成了 DAG（`src-ui/src/graph/`），第 3 期把派发做成「跑当前图里可执行的 draft 节点」（`src/dispatch.rs`、`src-ui/src/graph/dispatch.ts`，留痕契约 `aicross-dispatch/1`）。本期在此之上做的是**逐阶段自动推进**与**产物交付**，不是另起一套流水线引擎：

1. **阶段 = 图的层**。派发一次只跑「父节点全部 done 的 draft 节点」；本期加一个推进循环：一层跑完自动派下一层，直到没有可跑的 draft 或遇到需要用户裁决的节点（G4）。经济流水线盖到根上就是三层：出方案 → 执行 → 验收（accept）。
2. **输入仍由 `contextOf` 决定**，A 节三种可见性照它算。差别在「产物」：写权限为 `单写者`/`worktree` 的执行节点，`Reply` 新增可选字段 `artifacts { diff, changed_files: [{path, content}], worktree?, stdout_summary }`（这是本期对第 2 期契约唯一的字段追加）；盲的验收节点在 B 节的盲目录里收到 `REQUEST.md` + `diff.patch` + `changed/`，而不是把 diff 塞进 prompt 正文；共享节点则把 `artifacts` 摘要拼进序列化上下文。
3. **G4 的修正轮长在图上**：NEEDS_FIX → 在验收节点下新建执行者的 turn 节点（prompt = 上一轮产物 + 验收意见），再在其下新建验收 accept 节点；最多 3 轮就是最多再长 6 个节点。用户在图上能看见每一轮，不做隐藏状态机。
4. **通道口径按 09-13 定案**（`docs/PRODUCT.md`「通道架构定案」）：pi 是非原生订阅模型的默认通道；验收/盲审默认 `pi --no-tools --no-context-files`（isolation 完整）；真身看 `responseModel`。验收 fixture 的模型改为：出方案 `pi/zai-coding-cn/glm-5.3-flash`（zai 会把 glm-5.2/4.7 静默升级，按真身规则会判不符）、执行 `codex/gpt-5.6-luna`、验收 `kimi/kimi-code/k3-256k`（kimi 模型一律用 config.toml 完整别名）；DeepSeek 若用，走官方 provider `deepseek`，模型 `deepseek-flash`（pi 清单没有就用 `deepseek-v4-flash`）。
5. **小体量纯文本材料内联进 prompt**：第 3 期真跑里 codex 在空目录带工具审 300 字方案花了 389k 输入 token。规则：附件总量 < 30KB 且全为文本时，盲节点把材料内联进 prompt（仍拷进盲目录），并对 codex 这类工具型通道加 `-s read-only` 之外的提示「材料已在正文，勿再用工具读取」；超过阈值才只给目录。C 节「小任务不外派提示」与此配合。
6. **pi 节点的续接走会话文件**（可选，做不完就记后续）：pi 的会话是 JSONL，`--session-id` 能跨进程续。同一目标、共享可见性的 pi 节点链，优先用 session 续接只发本条；只有跨 CLI 才走有损序列化。第 3 期已对所有通道做了「同目标 + 共享 + 父节点有 session_id 则续接」，本期只需确认 pi 路径上 thinking 不丢，并把结论写进 REPORT。
7. **验收 3 的请求上限 ≤ 8 次不变**；沙箱里发不出请求就把 ignored 测试留给编排者在沙箱外跑，REPORT 如实写「未验证」。

## A. 阶段传递
- 阶段串行；上一阶段全部节点完成后，下一阶段开始。
- 每个节点收到的输入由 `visibility` 决定：
  - **共享**：用户原话 + 对话上下文 + 上游全部产物。
  - **指定上游**：用户原话 + 指定节点的产物。
  - **盲**：用户原话 + 附件 + **上游产物**（产物本身是待验对象，必须给），**不给**上游的推理/方案文本以外的东西——具体：验收节点收「用户原话（逐字）+ 执行阶段的产物（diff + 改动文件全文）」，不收出方案节点的方案文本；用户可在 G1 勾选把方案也发。
- "产物"定义见 B。

## B. 执行产物怎么交（本期要定的设计问题）
- 执行节点 `write=单写者` 时在**项目目录**运行，运行前后各拍一次 TASK-00 的目录快照；产物 = `{ diff（统一 diff 文本）, changed_files（相对路径 + 改后全文，单文件 >200KB 只给 diff）, stdout 摘要 }`。
- 执行节点 `write=worktree` 时：`git worktree add ~/.aicross/runs/<run>/<node>/wt`（项目非 git 仓库则退回单写者并提示）；产物同上，另附 worktree 路径；「采纳」= 提示用户手动合并（本期不自动 merge）。
- 同一项目目录同时只允许一个 `单写者` 节点运行（引擎层加锁，冲突时排队并在 G2 显示"等待写锁"）。
- 验收节点在盲目录里收到产物：`diff.patch` + `changed/` 目录（改后文件）+ `REQUEST.md`（用户原话）。

## C. 角色配置与经济规则
- 设置里「角色」页：出方案 / 执行 / 验收 各选默认 `agent/model + 档位 + thinking`，保存到 `~/.aicross/roles.json`；经济流水线预设的空节点从这里填。
- 规则以 **prompt 模板** 落地（可在设置里看、不可关）：
  - 执行节点 prompt 头部固定加入：「需要精确数值的部分必须写代码并执行，给出运行结果；不得心算。」（skill 铁律 1）
  - 验收节点 prompt = 冻结模板 + 固定维度清单「需求完整性、逻辑正确性、边界情况、代码质量、测试覆盖、实际运行结果」+ 末尾必答「对照原始需求，是否完整实现？」。
  - 出方案节点 prompt 末尾要求输出「任务契约」：目标、范围、不做、验收标准，各一句。
- **小任务不外派提示**：任务文本 < 200 字且无附件、且预设是经济流水线 → G1 顶部提示「单步小任务，外派固定足迹不划算，建议直接 @ 一个执行者」，不阻止。
- **链式任务开思考**：执行节点 thinking=关 且任务含数字/表格/多步关键词（简单启发式：连续数字 ≥3 处或含"计算/统计/推导/递推"）→ G1 把该节点 thinking 改为「低」并标注原因；用户可改回。
- **验收换厂商**：验收节点 vendor 与执行节点相同 → G1 标"复核，非独立验证"；相同且用户未改 → 仍执行。

## D. 闭环与 G4
- 验收结论解析：末尾「是否完整实现」判定 PASS / NEEDS_FIX（找不到判定词 → 交人）。
- NEEDS_FIX → 自动回到执行节点，prompt = 上一轮产物 + 验收意见全文，再执行 → 再验收；**最多 3 轮**，每轮修正后先跑用户在设置里配置的验证命令（如 `npm test`，可空），失败不送审直接进下一轮修正。
- 3 轮仍 NEEDS_FIX → 结果屏并列各轮意见交用户。
- `STATE.md` 每轮更新；应用重启后打开项目，若 `STATE.md` 有未完成 run → 中栏提示「上次派发未完成」+「继续」「放弃」。

## E. 结果屏补充
- 流水线的结果屏按阶段分栏：方案 → 产物（diff 可展开）→ 验收意见；用量表按阶段合计。
- 「采纳」对执行产物：单写者 = 已在项目目录，标记为已采纳；worktree = 显示合并指引。

## 不做
- 自动 merge、自动 commit。
- 蜂群 / 多写者。

## 验收
1. 四项自动验证进 `tasks/REPORT-04.md`。
2. 测试：阶段输入组装（三种 visibility 各一例，断言盲节点收不到方案文本）；写锁互斥；验收结论解析 PASS/NEEDS_FIX/未知；小任务与链式启发式。
3. 本机真跑一次经济流水线（≤ 8 次请求）：fixture 任务「给 `fixtures/pipeline/roman.py` 补一个把整数转罗马数字的函数并写 3 个测试」；出方案 `pi/zai-coding-cn/glm-5.2`（thinking 高）→ 执行 `codex/gpt-5.6-luna`（单写者，在临时拷贝的项目目录里）→ 验收 `kimi/k3`。断言：产物 diff 非空、验收节点盲目录里只有 `REQUEST.md`/`diff.patch`/`changed/`、`.dispatch/` 三份 + `STATE.md`、验证命令（`python -m pytest`）通过后才送审。

## 第 3 期遗留（两家 R2 复审列出、本期一并处理）

- `dispatch.rs` 靠中文文案前缀判 `identity_mismatch`，改为类型化错误 kind。
- cc-switch 路零真跑：usage 正则键序要与真实 `cc_switch.py --usage` 输出核对，preview 校验 `ccSwitchPath`；本期真跑一次 cc-switch 路（≤1 次请求）。
- running 期派发浮层锁死中栏：给「收起」入口，后台继续跑，中栏图上看进度。
- `DispatchOverlay` 里 `stage` 硬编码 1；`identity_mismatch` 在界面折叠成「失败」，要单独显示。
- 盲节点输入 token 超阈值（如 50k）在结果卡告警；与第 0 节第 5 条内联规则配合。
- G1 决策行补「理由：角色默认 / 用户指定」段（TASK-03 D 节原要求）。
- 无引号脱敏吞整行的过度替换，G1 提示「可能连带替换正常内容，N 处」。
- live 测试的 prompt 改走生产 `serializeContext` / `BLIND_PROMPT_PREFIX`。
- 「归类模型」设置项本期接线或置灰。

## 参考实现：AutoResearch（备忘，不进本期验收）

github.com/EvoMap/AutoResearch（事实与出处已记入 `D:\work\agentteam\COMPETITIVE-LANDSCAPE.md` 2026-09-03 节）。它是产物交付与验收证据链的公开参考实现：

- **产物清单 + 哈希记账**：全部产物带 SHA256 记账进引擎，`verify-close` 只接受引擎记录能支持的终态；执行者角色报「完成」不生效。B 节的产物交付若后续加哈希核对，参考此条。
- **成功标准派发前冻结**：成功标准写进计划再派发，闸门核对证据不核对执行者的结论声明。C 节的固定验收维度清单是最小形态；后续若把出方案节点的「任务契约」升级为派发前登记的验收标准，参考此条。
- **停止是合法终态**：预实验判负、指标平台期都允许停止，留下证据和后续选项。本任务「3 轮仍 NEEDS_FIX → 各轮意见并列交用户」属此类；后续流水线终态可显式加「停止（附证据）」。
- **每轮只推进一个工作单元，状态可恢复**：`workflow_queue.json` + `state.md` + append-only `decisions.log`，中断后同目录继续。对应本任务 D 节 `STATE.md` 与重启续跑。
