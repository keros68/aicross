### 1. 任务书第 3 节「必须保持」逐条核对表

| 核对项 | 必须保持的要求 | 删除后现状与行为一致性 | 证据（文件:行号） | 结论 |
|---|---|---|---|---|
| `aicross dispatch` | 参数解析（`--project`、`--preset`、`--text-file`，拒绝 `--text`）、执行流水线与退出码（0 成功，1 业务失败，2 参数错，3 引擎不可用） | 保持一致。参数在 `build_request` 中校验，别名映射（`shared/economy/blind`）与 `--text` 拦截逻辑未变，直接交由无窗口引擎执行。 | [`src/console_cli.rs:775-794`](file:///src/console_cli.rs#L775-L794) | 一致 |
| `aicross status` | 参数解析（`run_id`、`--wait`、`--timeout` 校验为正整数）、直读引擎 run 状态、退出码（0 成功/完成，1 run 不存在或业务失败，2 参数错，3 引擎不可用） | 保持一致。`run_id` 与正整数超时检查保留，直读引擎 run；测试用例确认 run 缺失返回退出码 1，引擎不可用返回退出码 3。 | [`src/console_cli.rs:796-801`](file:///src/console_cli.rs#L796-L801), [`src/console_cli.rs:836-847`](file:///src/console_cli.rs#L836-L847), [`src/console_cli.rs:948-958`](file:///src/console_cli.rs#L948-L958) | 一致 |
| `aicross adopt` | 参数解析（`run_id`、`node_id`）、调用引擎采纳逻辑、退出码 | 保持一致。位置参数检查保留，构造节点采纳请求直接交由引擎执行。 | [`src/console_cli.rs:802-806`](file:///src/console_cli.rs#L802-L806) | 一致 |
| `aicross cancel` | 参数解析（`run_id`）、调用引擎叫停逻辑、退出码 | 保持一致。`run_id` 位置参数检查保留，取消请求直接交由引擎执行。 | [`src/console_cli.rs:807-810`](file:///src/console_cli.rs#L807-L810) | 一致 |
| `aicross open` | 唤起窗口查看器，支持携带 `run_id` 冷热启动 | 保持一致。`console_cli.rs` 的 `run_if_subcommand` 对 `open` 返回 `None`，直通 Tauri 应用生命周期；单实例插件与启动参数转发处理。源码全文在材料中截断，具体实现行号见报告。 | [`src/console_cli.rs:18-24`](file:///src/console_cli.rs#L18-L24), [`tasks/REPORT-40.md:41-43`](file:///tasks/REPORT-40.md#L41-L43)（具体代码行材料不足以判断） | 一致（行为符合） |
| `aicross resources [detect\|smoke]` | 探测与冒烟命令行解析与执行，直接访问资源库而不依赖窗口 | 保持一致。TASK-39 的直读与冒烟逻辑完整保留，未被修改；相关底层测试保留并通过。 | [`src/console_cli.rs:2-4`](file:///src/console_cli.rs#L2-L4), [`src/console_cli.rs:965-967`](file:///src/console_cli.rs#L965-L967) | 一致 |
| `aicross config show\|set-role\|set` | 角色与派发设置命令行解析与读写，不依赖窗口 | 保持一致。TASK-39 配置展示与修改逻辑完整保留，直接操作配置文件。 | [`src/console_cli.rs:2-4`](file:///src/console_cli.rs#L2-L4), [`src/console_cli.rs:965-967`](file:///src/console_cli.rs#L965-L967) | 一致 |
| 查看器冷热启动与单实例转发 | 窗口冷启动通过 `take_pending_open` 导航至对应 run；热启动由单实例插件转发 `open` 命令行唤醒窗口 | 保持一致。`server.rs` 保留 `take_pending_open`，单实例转发仅认 `open` 命令；源码全文在材料中截断，具体代码行见报告。 | [`tasks/REPORT-40.md:41-43`](file:///tasks/REPORT-40.md#L41-L43)（具体代码行材料不足以判断） | 一致（行为符合） |
| 各数据文件格式 | 资源库、`roles.json`、`dispatch-settings.json`、`graphs/`、`runs/` 格式及派发记录 schema（`aicross-dispatch/1`）保持不变 | 保持一致。派发记录解析仍然强制核对 frontmatter 的 `schema == "aicross-dispatch/1"`，核心序列化结构体未受影响。 | [`src/dispatch.rs:487`](file:///src/dispatch.rs#L487) | 一致 |
| AGPL 与上游义务 | `LICENSE`、`NOTICE`（上游文本逐字保留）、`TRADEMARKS.md` 不动；`README.md` 声明 fork 自 Coffee CLI | 保持一致。`git diff --numstat` 中无上述法律文件变更；`README.md` 与 `AGENTS.md` 保持注明上游源流。 | `git diff --numstat`（未包含上述文件）, [`tasks/REPORT-40.md:79-80`](file:///tasks/REPORT-40.md#L79-L80) | 一致 |
| Windows 安装包加 PATH 逻辑 | 安装程序将可执行目录加入系统 PATH 的逻辑保留 | 保持一致。`windows_path.rs`、`installer.nsi`、`windows/fragments/path.wxs` 均未被删除或修改。 | `git diff --numstat`（未删除上述文件）, [`tasks/REPORT-40.md:79-80`](file:///tasks/REPORT-40.md#L79-L80) | 一致 |

---

### 2. 引擎与派发链路删除判断依据

| 模块 / 模块大块 | 删除内容 | 判断依据与影响分析 |
|---|---|---|
| `src/channels/acp.rs` | 删除了 6 条 Tauri 命令（权限卡片应答/关闭、`/` 斜杠补全、池释放、适配器探测）、界面事件广播（`emit`、`APP` 全局句柄）及「关 tab 即拒」逻辑。 | **确实只服务旧界面**。被删代码全部服务于原有的交互式控制台前端卡片、命令提示与 Tab 管理。底层 `AcpClient` 交互进程生命周期管理、JSON-RPC 解析等核心逻辑完整保留。未删除引擎运行必经分支。 |
| `src/dispatch.rs`（蜂群计划与合并） | 删除了 `check_swarm_plan*`、`merge_swarm_items*` 及其关联的补丁生成、3-way 应用、工作树冲突回滚与对应测试（约 2000 行）。 | **确实只服务旧界面**。依据 [`docs/PRODUCT.md`](file:///docs/PRODUCT.md)「蜂群：分工合并，任务明确的并行开发，窗口版已有，无窗口版另起一期。本期只删不建」。该能力原先仅由前端图形化界面（`src-ui/src/graph/swarm.ts`）调用，无窗口引擎尚未建立命令行入口，删除符合产品定案。 |
| `src/dispatch.rs`（派发预览与脱敏统计） | 删除了 `preview_dispatch`、`preview_core` 以及 `redaction_count` / `broad_redaction_count`。 | **确实只服务旧界面**。该部分仅供旧界面「派发前预览」弹窗展示脱敏计数胶囊。引擎实际派发时的脱敏过滤逻辑在 `redact_text` 中执行并完整保留。 |
| `src/dispatch.rs`（旧派发状态与崩溃对账命令） | 删除了 `inspect_dispatch_state`、`reconcile_dispatch_state`、`abandon_dispatch_state` 及 `rollback_node_changes` 的 Tauri 命令包装。 | **确实只服务旧界面**。被删内容为原前端窗口启动时检测未完成派发的弹窗接口。无窗口引擎对账体系位于 `src/engine/`，工作区快照定向回滚的核心函数 [`rollback_node_changes_core`](file:///src/dispatch.rs) 完整保留。 |
| `src/dispatch.rs`（Tauri 进度事件） | 删除了 `emit_progress`、`emit_running_progress`、`heartbeat_loop` 等。 | **确实只服务旧界面**。无窗口引擎无前台窗口事件接收者，查看器（TASK-37）采用轮询磁盘 `runs/*.json` 机制，不消费 Tauri 事件通道。 |
| `src/orchestration.rs` | 删除了 12 条 Tauri 命令包装、`commander.json` / `premises.json` 读写、预设构建及图文件管理。 | **确实只服务旧界面**。服务于原前端右栏编排板和预设选择器。引擎派发预设与角色默认配置在 `src/engine/` 中直接处理，不经过这些包装。 |
| `src/resources.rs` | 删除了 11 条 Tauri 命令包装（`get_resources`、`detect_*`、`check_pi_provider_login*`、模型增删改等）。 | **确实只服务旧界面**。服务于原前端资源配置页；命令行 `aicross resources` 依赖的 `*_at` 系列底层函数（资源探测、真身核对冒烟等）全部保留在原位。 |
| 引擎核心链路（ACP通道、额度切备选、红绿核对、盲审材料、产物快照、崩溃对账、真身核对） | 未删除。 | **未发现误删**。[`src/dispatch.rs`](file:///src/dispatch.rs) 的 `dispatch_stage_core`、只读项目副本目录隔离、盲审产物快照差分、`redgreen` 测试钉住核验、额度用尽识别与备选模型轮替逻辑保持完好，31 个金标准夹具全部通过。 |

---

### 3. 首页「最近的派发」审查

- **`list_runs` 规范与过滤**：按约定仅收录扩展名为 `.json` 且文件名不以 `.` 开头的文件，按 `updated_at` 倒序截取前 50 条；模式名与任务名复用查看页的 `preset_display_name` 与 `task_title` 纯函数。Rust 侧配套 4 条单元测试覆盖排序、过滤与兜底。
- **页面切换与空状态**：点选 run 进入查看页，查看页「关闭」回到首页；无记录时展示规范文案「还没有派发记录；在宿主里经 ai-cross skill 派发，或运行 aicross dispatch」。
- **轮询读盘行为**：存在无条件固定 5 秒轮询读盘行为，见问题列表（低严重度）。
- **材料证据**：[`tasks/REPORT-40.md:74-76`](file:///tasks/REPORT-40.md#L74-L76), [`tasks/REPORT-40.md:129-130`](file:///tasks/REPORT-40.md#L129-L130)（由于 `run_view.rs` 与 `Home.tsx` 全文在材料中截断，具体实现行号材料不足以判断）。

---

### 4. 窗口外壳审查

- **外壳与事件**：标题栏自绘拖动、最小化、最大化、关闭及缩放边条保留；`capabilities/default.json` 权限收缩后保留了窗口操控和单实例事件所需的最小权限集合。
- **前后端命令对应**：前端旧组件涉及的废弃 Tauri 命令调用（PTY、hook、marketplace、skills 等）已随组件完全移除；后端 `server.rs` 仅留窗口基础控制、`take_pending_open` 与 `list_runs`。
- **材料证据**：[`tasks/REPORT-40.md:41-43`](file:///tasks/REPORT-40.md#L41-L43), [`tasks/REPORT-40.md:65-68`](file:///tasks/REPORT-40.md#L65-L68)（关于查看页单次 run 加载命令的细节见问题列表核查项）。

---

### 5. 文档审查

- **MANUAL / CONSOLE-CHANNEL / README / AGENTS.md**：已根据现行命令行引擎与查看器形态改写，清除了旧界面的说明。源码全文在材料中截断，具体文本行号材料不足以判断。
- **PRODUCT.md 残留不一致**：存在一处旧功能描述未清理，见问题列表（低严重度）。

---

### 6. 测试审查

- **测试清理**：前端删除 14 个测试文件（包含旧组件、终端、旧编排、蜂群前端测试与前端金标准生成器）；Rust 侧删除了随模块下线的测试（PTY、hook、fonts、shell_probe、console_api 以及 `dispatch.rs` 中被删蜂群和 propose 的对应测试）。被删测试均确实属于被删代码。
- **保留测试与金标准重放**：保留的模块测试全部通过（Rust 406 passed，前端 22 passed）。前端金标准生成器虽随前端参考实现删除，但 `fixtures/engine-golden/` 下全部 31 个夹具文件完整保留，新增说明 `fixtures/engine-golden/README.md`，Rust 侧重放测试 `engine::golden::engine_golden_fixtures_replay` 照常执行且通过。

---

### 7. 发现的问题清单

#### 【高严重度】
未发现问题。

---

#### 【中严重度】

##### 问题 1：无窗口形态下 ACP 权限请求会无谓阻塞等待至 deadline 超时
- **问题**：在删除了 `answer_acp_permission` Tauri 命令与前端权限卡片界面后，[`AcpClient::ask_user_permission`](file:///src/channels/acp.rs#L549-L605) 依然将权限请求登记入 `pending_permissions` 并通过 `receiver.recv_timeout` 进入循环等待。在生产无窗口运行时，由于没有任何界面或外部机制能向该通道发送授权应答，代码将一直阻塞空转，直至整个交互轮次的 `deadline` 超时才退出并默认执行拒绝逻辑。
- **证据**：[`src/channels/acp.rs:561-595`](file:///src/channels/acp.rs#L561-L595)（对比已删除的应答命令 [`src/channels/acp.rs:163-172`](file:///src/channels/acp.rs#L163-L172)）。
- **后果**：若在交互式会话（如单线 Kimi）中触发了需要用户权限确认的工具调用，引擎进程将直接卡死空等数分钟（直至轮次截止时间），不仅严重拖慢派发流程，还浪费系统资源。
- **建议改法**：在无窗口形态下，若非测试环境（即生产调用链），且没有外部应答者介入，`ask_user_permission` 登记失败或无需挂起时应直接匹配 `reject_once` 并发送 RPC 拒绝回复，不再进入跨秒循环阻塞等待。

---

#### 【低严重度】

##### 问题 2：首页采用固定 5 秒无条件轮询，后台或最小化闲置时持续读盘
- **问题**：首页最近派发列表采用了固定 5 秒间隔读取 `~/.aicross/runs/` 目录的实现，未接入 `document.visibilityState` 或窗口失焦判定。
- **证据**：[`tasks/REPORT-40.md:129-130`](file:///tasks/REPORT-40.md#L129-L130)（报告注明：「首页轮询... 我按窗口开着时每 5 秒读一次 run 目录实现」）。由于 `viewer/Home.tsx` 全文在材料中截断，具体代码行材料不足以判断。
- **后果**：当应用窗口被最小化或放置在后台长时间闲置时，仍会持续每 5 秒进行一次磁盘目录扫描与反序列化解析，产生无谓的后台 I/O 唤醒与电量开销。
- **建议改法**：在首页轮询的 `useEffect` 中增加 `document.visibilityState === 'visible'` 判断，当窗口不可见时暂停定时器，恢复可见时立即触发一次静默刷新。

##### 问题 3：`docs/PRODUCT.md` 中仍残留查看器展示资源的陈旧表述
- **问题**：`docs/PRODUCT.md` 在本次任务中未作修改，其「机制」第 6 节仍保留「探测、冒烟、就绪状态由 aicross resources 输出，查看器里只读展示」的表述，但本次 TASK-40 已彻底删除前端资源页（`ResourcesTab.tsx` / `ResourcesOverlay.tsx`），查看器最终只剩首页与查看页。
- **证据**：[`docs/PRODUCT.md:37`](file:///docs/PRODUCT.md#L37), [`tasks/REPORT-40.md:124-126`](file:///tasks/REPORT-40.md#L124-L126)。
- **后果**：核心产品设计文档与实际代码形态出现分歧，容易误导后续维护者。
- **建议改法**：更新 [`docs/PRODUCT.md`](file:///docs/PRODUCT.md)，将该句调整为「探测、冒烟、就绪状态由 aicross resources 输出，查看器不再承接资源展示」。

##### 问题 4：查看页单次 run 详情数据加载命令的注册核对提示
- **问题**：[`tasks/REPORT-40.md:41-43`](file:///tasks/REPORT-40.md#L41-L43) 记录的保留命令列表中，`server.rs` 仅明确记载保留了 `window_minimize / window_maximize / window_close / show_main_window / take_pending_open` 与新增的 `list_runs` 6 条命令，未在说明中提及单次派发详情的获取命令（如 `get_run_view`）。由于 `src/server.rs` 源码全文在材料中截断，材料不足以直接查证代码行。
- **证据**：[`tasks/REPORT-40.md:41-43`](file:///tasks/REPORT-40.md#L41-L43)（代码行因截断属材料不足以判断）。
- **后果**：若实现者在收敛 `server.rs` 时误将查看页获取单个 run 数据的命令漏掉，前端从首页点入查看页时将无法加载派发详情数据。
- **建议改法**：编排者在执行发布构建核验时，需重点确认 `src/server.rs` 的 `invoke_handler!` 中是否已正确注册查看页读取单个派发详情的命令。
