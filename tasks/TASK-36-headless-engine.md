# TASK-36 引擎无窗口可跑：派发编排挪进 Rust（09-30）

**只改 Rust（`src/**`）、`docs/CONSOLE-CHANNEL.md`，外加第 5 节规定的一个前端测试文件与 `fixtures/engine-golden/`。不改其它前端代码，不删窗口代码。不 commit，不 push。**

## 背景

`docs/PRODUCT.md`「为什么是这个形态」「机制」两节：用户在 Claude Code、Codex 等宿主里说话，ai-cross skill 调 `aicross` 命令行派发，结果并排交给查看器裁决。本期是「顺序」第 1 步：`aicross dispatch / status / adopt / cancel` 在**窗口没开时**也能跑完整条流水线。

现状（查清的边界原文在 `docs/history/PRODUCT-0830-0930.md` 末节「待验证 1」）：

- 单阶段执行 `dispatch_stage_core(request, app: Option<AppHandle>)` 传 `None` 即可无窗口跑（`src/dispatch.rs` 的测试就是这样调的）。图、预设、角色的读写在 `src/orchestration.rs`。
- 「一次派发怎么走完」只在前端：`src-ui/src/console-channel.ts` 的 `runConsoleDispatch`，经 `src-ui/src/graph/` 下的 `stamp.ts`、`context.ts`、`validate.ts`、`dispatch.ts`（`buildNodeRequest`、`runStageLoop`、`nextStageStep`、`reducePipelineStep`、`applyDispatchResults` 等）。
- 两处状态绑在窗口上：派发设置在浏览器 `localStorage`（`graph/dispatch-settings.ts`）；在飞 run 只在窗口进程内存（`src/console_api.rs` 的 `RUNS`）。命令行因此在窗口没开时报「控制台未运行」退出 3。

## 1. 移植范围

**以 `runConsoleDispatch` 实际调到的函数为准，调用图自己追到底**，逐个移植成 Rust，行为与 TypeScript 一致。放在新模块 `src/engine/`（`mod.rs` + 按需分文件），纯逻辑与读写分开：纯函数只接收数据、返回数据，不碰文件和进程。

必须覆盖（不限于）：

- 素材与预设：`appendConsoleMaterial`、`fillRoot`、`latestLeaf`、`newGraph`、`consolePreset`（含 `targets` 直给的分支）、`presetWithRoleDefaults` / `applyRoleDefaults`、`stamp`。
- 派发前检查（顺序与拒绝文案与 `runConsoleDispatch` 逐字一致）：在飞上限、读取资源失败、找不到预设、角色没有可派发目标、`validateNode` 的阻断提示（`未接入`、含`只能只读`）、目标验证失败、额度用尽（含恢复时间）、超过总请求上限、`dirtyWorktreeHintOf`。
- 请求构造：`graphStage`、`serializeContext`、`serializeCurrentMessage`、`runRequestOf`、`upstreamArtifactsOf`、`continuationSession`、`continuationRunner`、`buildNodeRequest` 及其内部用到的提示词拼装（`promptForRole`、`promptWithWriteGuard`、盲前缀等）。提示词模板用 `include_str!` 读 `fixtures/prompts/*.txt`，不复制文本。
- 推进循环：`runStageLoop` 与 `startPipelineHeadless` 的语义（含 `onError` 分支、重复批次停止、缺目标停止、请求上限停止）、`nextStageStep`、`markPending`、`applyDispatchResults`、`reducePipelineStep`（修正轮、验证重试、遗留节点）、`dispatchBatchKey` / `markBatchProcessed`、`resetNodesToDraft`、`failedNodesWithoutResults`、`leftoverNodes`。
- run 摘要：`nodeSummary`（`text_excerpt` 取前 160 个**字符**，与 JS `slice` 同口径按 UTF-16 码元截断；实现时注明取舍并在测试里钉住中文与表情符号各一例）。

**不在范围**：蜂群（`swarm.ts`，需要在对话里确认清单，另起一期）、仲裁与 `pickOne`、`propose` / 建议条、界面用的对账与进度展示函数、`preview_dispatch`（无窗口路径不调预览）。

**类型对齐（只追加）**：`src/orchestration.rs` 缺 TypeScript 已有的两个字段，补上并保持 `serde(default)` 与省略空值：`Target.validation_command: Option<String>`、`ReplyArtifacts.source_node_id: Option<String>`。补完后，前端写出的图文件经 Rust 读入再写回不得丢字段——加一条往返测试。

**节点 id 与时间**：新节点 id 用 UUID v4 字符串（与 `crypto.randomUUID()` 同格式），时间用 RFC 3339 UTC（与 `toISOString()` 同格式，毫秒三位、`Z` 结尾）。纯函数通过参数注入 id 生成器与时钟，测试里用确定值。

## 2. 设置落盘

新文件 `~/.aicross/dispatch-settings.json`（开发构建与发布版用同一套 `aicross_root()` 口径），字段与 `UiDispatchSettings` 同名同大小写（camelCase），缺失或越界时的默认值与夹取规则逐条照 `loadDispatchSettings`。文件不存在或解析失败 → 全部默认值，不报错、不写回。本期不改窗口的设置页，窗口仍读 `localStorage`；两边暂不同步，写进第 6 节文档。

## 3. run 状态落盘与后台进程

- run 摘要存 `~/.aicross/runs/<run_id>.json`，形状就是现有 `ConsoleRun`（`src/console_api.rs`），另追加两个字段：`pid: u32`（后台进程）、`updated_at: String`。写入一律先写同目录临时文件再改名。
- `aicross dispatch` 同步完成：读设置、资源、预设、角色 → 建图并盖预设 → 第 1 节的全部派发前检查 → 保存图（`save_graph`）→ 写 run 文件（`status: running`）→ 起后台进程 → 打印响应并退出 0。**首个响应填真实值**：`{ "run_id", "accepted": true, "graph_id": <真实图 id>, "node_ids": [<本次新建的节点 id>] }`（原先为空，是追加信息，不改字段）。检查不通过：写 run 文件 `status: rejected`、`reason` 为拒绝文案，打印同形状响应加 `status` 与 `reason`，退出 1。
- 后台进程：`aicross` 自己，隐藏子命令 `__run <run_id>`（不写进帮助），Windows 下 `CREATE_NO_WINDOW | DETACHED_PROCESS`，与父进程脱离，父进程退出不影响它。它读 run 文件与图，跑推进循环，每次落图后更新 run 文件（节点摘要、`in_flight_node_ids`、`leftover`），结束写 `done` / `failed` 与 `reason`（规则同 `runConsoleDispatch` 末尾）。`dispatch_stage_core` 传 `app: None`。
- 在飞上限：数 `runs/` 下 `status: running` 且 `pid` 仍存活的 run。
- 进程死了：`status` 读到 `running` 但 `pid` 已不存在时，把该 run 改写为 `failed`、`reason: 引擎进程已退出`，再返回。不自动重跑。
- `aicross status <run_id> [--wait] [--timeout N]`：读 run 文件，输出与现在同形状；`--wait` 轮询到非 `running` 或超时，超时退出 1（现有语义）。
- `aicross adopt <run_id> <node_id>`：直接改图文件（`setReplyAdopted` 语义），不要求窗口开着、不要求图在内存里。
- `aicross cancel <run_id>`：写 `runs/<run_id>.cancel` 标记；后台进程起一条线程每 500ms 查它，发现后对在飞节点调用现有的取消登记（`mark_cancelled` 那一套），run 写成 `failed`、`reason: 派发已取消`，在飞节点摘要改为 `failed`、`text_excerpt: 派发已取消`（同 `runConsoleAction`）。
- `propose` 不在范围：保持现在走窗口 HTTP 的行为不动。

**退出码语义保持**：0 成功 / 1 业务失败或等待超时 / 2 参数与编码错 / 3 引擎不可用。3 现在只在 `aicross_root()` 不可用、run 目录不可写、后台进程起不来时出现——skill 读到 3 才回退到直接调 CLI。命令行参数（`--project`、`--preset`、`--text-file`、别名映射、拒收 `--text`）一字不改。

## 4. 与窗口的关系（过渡期）

- `dispatch / status / adopt / cancel` **不再经窗口的 HTTP 接口**，窗口开着也走无窗口引擎。`src/console_api.rs` 的 HTTP 服务保持原样不删（`propose` 还在用），只是命令行不再调它。
- 窗口里的前端派发路径（对话图、控制台单线）本期不改，冻结，不加新行为；命令行建的图存进同一个图目录，窗口重开后能看到。
- 同一项目目录同时有窗口派发与命令行派发时的写冲突，本期不处理，写进文档。

## 5. 测试

**行为一致性用参考实现出的金标准钉住**（GT 由参考实现算出）：

1. 新前端测试文件 `src-ui/src/__tests__/engine-golden.test.ts`：用现有 TypeScript 函数跑下列场景，把「输入图 + 设置 + 每阶段注入的结果 → 每阶段发出的请求 + 最终图 + run 摘要」写成 `fixtures/engine-golden/<场景>.json`。环境变量 `AICROSS_WRITE_GOLDEN=1` 时写文件，否则读文件逐项断言（保证夹具与当前 TypeScript 行为一致）。节点 id 与时间戳按出现顺序归一化后再比。
2. Rust 测试读同一批夹具，驱动 `src/engine/` 的纯函数与推进循环（阶段结果由夹具注入，不起真进程），断言请求与最终图逐字段相等。

场景至少：经济模式一次通过；验收 NEEDS_FIX → 修正轮 → 通过；修正轮到上限 → 遗留节点；验证命令失败 → 重试到顶；盲审模式三路；共享模式；某路 `quota_exhausted`；同一批结果重复送达；下一阶段缺目标；超过总请求上限；每一条派发前拒绝各一例（检查顺序也要钉住）；`targets` 直给；续写已有图（图上已有别的材料节点）。

另需 Rust 单测：设置文件缺失 / 越界 / 解析失败；run 文件原子写；`pid` 已死判 `failed`；cancel 标记被后台线程读到；`adopt` 在无窗口时改图文件；退出码 3 的三种来源；类型往返不丢字段。

**不要**为了让测试通过改 TypeScript 的行为；发现 TypeScript 本身的缺陷，照原样移植并在报告里单列，由编排者裁决。

## 6. 文档

`docs/CONSOLE-CHANNEL.md` 的命令行一节改为最终状态：不需要窗口；run 文件位置；首个响应带真实 `graph_id` / `node_ids`；退出码 3 的新含义；设置文件位置与「窗口设置页暂不同步」；窗口与命令行同目录并发写不受保护。只写最终状态，不写「原来如何」。

## 7. 验证命令（全绿才交）

```
cargo check --all-targets        # 零 warning
cargo test
cd src-ui && npx vitest run --configLoader native --pool threads
cd src-ui && npm run build
```

换行：每个改动文件保持原有换行（`server.rs` CRLF，其它 Rust LF，`src-ui/src/tauri.ts` 混合）；新文件 LF。写含反斜杠的内容用编辑工具，不用 shell heredoc。

## 8. 交付

- 在工作树里改完即停，不 commit、不 push。
- 写 `tasks/REPORT-36.md`：改了哪些文件；移植清单（TypeScript 函数 → Rust 函数）；照原样移植的 TypeScript 缺陷；第 7 节命令的实际输出摘要；未做与理由。

## 编排者真机核（实现者不做）

窗口关闭状态下，在一个干净的小项目里：`aicross dispatch --preset economy` 真派一次到 `done`；`status --wait` 期间 `cancel` 一次；杀掉后台进程后 `status` 报 `引擎进程已退出`；`adopt` 后重开窗口能看到该图与采纳标记。
