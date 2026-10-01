# REPORT-37 查看器第一版

对应 `tasks/TASK-37-viewer.md`。分支 `viewer/task37`，基于 TASK-36 合并后的 main（0f93482）。未 commit。

## 做了什么

- 新增命令 `aicross open <run_id>`：照 `launch` 的冷热两条路——冷启动经
  `AppState::pending_open` 与 `take_pending_open`，热启动经单实例回调重解析 argv
  后发 `open-run-request` 事件。打开动作只读，不派发、不改文件。退出码按契约：
  0 已打开（热启动转发后第二个进程由插件退出 0）/ 1 找不到 run / 2 参数错 /
  3 窗口起不来（引擎根不可用同按 3）。
- 新增 Tauri 命令 `load_run_view(run_id)`：经 TASK-36 的 `read_run` 读 run 文件
  （顺带完成死进程判定），再读该 run 的图文件，返回 run 摘要 + 节点（按 run
  摘要里的节点顺序，阶段号取图深度）。不读 `.dispatch/` 原文。
- 新增 Tauri 命令 `adopt_run_node(run_id, node_id)`：包 TASK-36 的 `adopt`
  （经其合并写，不整写 run 文件），随后按同一口径重读整页返回。
- 前端新增查看页 `RunViewer`：页头（模式名、run 状态、开始时间、总耗时、失败
  或拒绝原因、遗留条目），按阶段分组、组内各路并排（最多 3 列，超出换行，容器
  过窄时横向滚动），每路一张卡（标题「角色 · 任务」、次标题 agent/model、厂商
  色点、请求与实际模型、状态与恢复时间、token 与耗时、备选切换、跳过原因、
  验收结论、正文 Markdown、产物 diff 折叠、worktree 提示），有结果的卡上只有
  「采纳 / 取消采纳」一个按钮。run 运行中每 2 秒重取一次，终态停止；run 文件
  找不到时显示「找不到这次派发」。
- `docs/CONSOLE-CHANNEL.md` 补 `open` 一条；`docs/MANUAL.md` 加「查看器」一节，
  术语表补「查看器」一行。

## 改动文件

| 文件 | 改动 |
|---|---|
| `src/run_view.rs` | 新增：`parse_open_args`、`build_run_view`、模式名推断、`load_run_view` / `adopt_run_node` 命令与测试 |
| `src/main.rs` | `mod run_view`；`open` 的参数与存在性校验（退出码 1/2/3）；`start_ui` 多带一个 `pending_open`；起窗失败时 `open` 退出 3 |
| `src/server.rs` | `AppState::pending_open`；`take_pending_open` 命令；单实例回调转发 `open-run-request`；注册两条查看器命令 |
| `src/console_cli.rs` | `attach_parent_console` 改 `pub(crate)`（`open` 的错误信息要挂回宿主终端，逻辑未动） |
| `src/engine/mod.rs` | `depth_map` 改 `pub(crate)`（查看器按它分阶段，逻辑未动） |
| `src-ui/src/viewer/run-view.ts` | 新增：页面数据组装纯函数（分组、标题、真身、quota、验收、轮询间隔、耗时与数字格式化） |
| `src-ui/src/viewer/RunViewer.tsx` | 新增：查看页组件 |
| `src-ui/src/viewer/RunViewer.css` | 新增：按视觉纪律的样式 |
| `src-ui/src/tauri.ts` | `RunView` / `RunViewNode` 契约类型（camelCase 镜像 Rust）；`takePendingOpen` / `loadRunView` / `adoptRunNode` |
| `src-ui/src/App.tsx` | 入口接线：冷启动取 `take_pending_open`，热启动听 `open-run-request`，挂载查看页 |
| `src-ui/src/__tests__/run-viewer-37.test.ts` | 新增：前端纯函数测试 |
| `docs/CONSOLE-CHANNEL.md` | 命令行一节补 `open`（含退出码语义） |
| `docs/MANUAL.md` | 「查看器」一节 + 术语表两处 |

未删任何现有窗口界面；`npm install` 曾把 `src-ui/package-lock.json` 换行重写，
已还原（依赖无变化）。

## 复用的现成部件

- **launch 的冷热启动机制**：`AppState::pending_launch` / `take_pending_launch` /
  单实例回调发 `launch-request`（`src/launch.rs`、`src/server.rs`、CenterPanel），
  `open` 逐条对应成 `pending_open` / `take_pending_open` / `open-run-request`。
- **TASK-36 引擎**：`read_run`（读 run + 死进程判定）、`adopt`（合并写、不动
  run 文件）、`valid_id`、`depth_map`（分阶段）。引擎行为零改动，只有两处
  可见性放宽。
- **`src-ui/src/components/center/ConsoleRunDetail.tsx`**：`compactRunNumber`
  （token 缩写）直接引用；`reportedModelLabel`（未报告 / kimi 不报真身 /
  运行中「—」，不回填请求值）按 viewer 的数据形状重写，口径与
  `console-run-detail.ts` 一致。
- **`src-ui/src/graph/dispatch.ts`**：`parseAcceptance`（验收结论判定）直接引用。
- **`src-ui/src/resources/agent-display.ts`**：`agentModelDisplay`（卡次标题）。
- **`src-ui/src/graph/vendor.ts`**：`vendorColor`（厂商色点，只做小面积标识）。
- **`src-ui/src/components/center/ConsoleMarkdown.tsx`**：正文 Markdown 渲染。
- **结果屏的展示语言**（`DispatchOverlay.tsx` + `GraphTab.css`）：产物
  `<details>` 折叠、等宽 diff、worktree 提示「采纳前请手动合并：」、采纳卡
  描边用 `--accent`，样式按同一套视觉纪律重写在 `RunViewer.css`。

## 模式名从哪来（任务书页头要求，run 文件里没有）

TASK-36 的 run 文件不记预设。查看器按 run 摘要里的角色序列（盖章插入序）对
`raw_presets_from` 的清单做前缀匹配：修正轮补的节点排在初始清单之后，不破坏
前缀；取最长匹配，同长内置优先；推不出来显示「自定义模式」（用户自建预设命中
时「自定义模式 · 名称」）。run 被拒前没建节点（G1 拒绝）时省略模式名。根本
解法是派发时把 preset id 写进 run 文件，属 TASK-36 契约改动，本期未动，见
「未做」。

## 测试名

Rust（`cargo test run_view`，8 条）：

- `open_parse_covers_argv0_flags_and_invalid_ids`（含 `--flag=value` 形式与非法
  run id、argv[0] 有无、缺 id、多位置参数）
- `load_run_view_missing_run_is_error`
- `load_run_view_running_keeps_status_and_infer_mode`（running 保持 + 模式推断 +
  四节点分阶段 1/2/3/3）
- `load_run_view_done_maps_reply_fields`（token / 真身 / 蜂群标题 / diff /
  worktree / 额度恢复时间 / 备选 / 遗留）
- `load_run_view_failed_carries_reason_and_degrades_without_graph`（失败原因；
  图缺失时 run 摘要降级可用）
- `adopt_run_node_merges_graph_and_leaves_run_file_untouched`（采纳经合并写、
  同图别的 run 节点不丢、run 文件字节不变）
- `infer_preset_name_matches_builtin_shapes_and_falls_back`（六个内置形状 +
  修正轮前缀 + targets 直给退自定义）
- `infer_preset_name_prefers_longest_match_then_builtin`（用户预设更长时胜出）

前端（`run-viewer-37.test.ts`，13 条，`run viewer data assembly (TASK-37)`）：
分组与列顺序（`groups nodes by stage in summary order with role titles`、
`caps stage columns at 3`）、标题取名顺序（`titles take swarm item title, then
graph title, then agent/model`）、真身显示规则（`reports identity without
backfilling the requested model`、`flags identity mismatch against the effective
requested model`）、quota 显示（`shows quota reset time only for quota states`）、
验收结论（`derives acceptance verdict for review roles only`）、状态与胶囊
（`labels node statuses including skipped and error kinds`）、备选说明
（`formats fallback switch text`）、采纳可用性（`offers adopt only on nodes
with settled results`）、轮询终态停止（`stops polling once the run reaches a
terminal state`）、总耗时（`computes elapsed time from started_at to updatedAt
for terminal runs`）、数字格式（`formats durations and token numbers`）。

另做了两条命令行实测（发布语义的参数分支，不经窗口）：

```
> aicross open            → 缺少 run_id（exit 2）
> aicross open bad/id     → run_id 无效（exit 2）
> aicross open --run=abc  → open 不接受选项参数（exit 2）
> aicross open no-such-run-123 → 读取 run 失败：系统找不到指定的文件。(exit 1)
```

## 四条验证命令的实际输出

```
> cargo check --all-targets
   Compiling aicross v0.1.0 (D:\work\aicross-p37)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.51s
（零 warning）

> cargo test
test result: ok. 558 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 60.44s

> cd src-ui && npx vitest run --configLoader native --pool threads
 Test Files  51 passed (51)
      Tests  742 passed (742)

> cd src-ui && npm run build
✓ built in 831ms
（vite 对 main chunk > 500 kB 的提示为既有告警，改动前 518.74 kB、改动后 527.56 kB，与本次无关）
```

## 未做与理由

1. **真机核**（任务书「编排者真机核」一节）：真派一次经济模式、冷启动打开、
   运行中逐阶段刷新、采纳后重开、热切换另一个 run。实现者不做，留给编排者。
2. **run 文件记 preset id**：页头的模式名靠角色序列推断，与内置预设形状相同的
   用户预设会被标成内置名（如用户自建恰好也是「执行×3」会显示擂台模式）；
   G1 前被拒的 run 无节点，页头省略模式名。根治要在 `start_dispatch` 写 run 时
   多记一个字段，属 TASK-36 契约改动，任务书禁止借道改引擎，未做。
3. **成功路径的验证输出**：引擎 `apply_results` 只在 `validation_failed` 时把
   `[验证输出]` 并进节点正文；验证通过时的输出只存在于当次的
   `DispatchNodeResult`，不落图。查看页按图展示，成功路看不到验证输出。这是
   TASK-36 的既有行为（发现记此处，未改引擎）。红绿核对的 `[红绿核对]` 段在
   正文里，正常可见。
4. **开发构建的热启动转发**：单实例插件按上游既有 `#[cfg(not(debug_assertions))]`
   只在发布包注册；dev 下第二次 `aicross open` 会再开一个窗口（与 `launch`
   行为一致）。真机核请用发布包或 `cargo build --release`。
5. **「最多 3 列，超出横向滚动」的实现**：列数上限 3，组内超出换行成多行；
   容器窄于 3×280px 时横向滚动。若想要「超出的列横向排开」另一种读法，改
   `stageColumnCount` 与对应 grid 一处即可。
6. **采纳与运行中 run**：运行中的 run 也允许对已有结果的卡采纳（引擎
   `adopt` 本就不限状态，且不整写 run 文件）；蜂群件「已并入不给取消采纳」的
   特例在 DispatchOverlay 有、查看器第一版未复刻（蜂群无窗口派发本就另起一期）。

## R1（fix-37-r1 修正轮，10-01）

依据 `tasks/fix-37-r1.md`，必修 R1-1…R1-6 与应修 R1-7…R1-12 全部完成。未
commit。engine/mod.rs 只按本轮授权追加了 `preset_id` 字段与 `start_dispatch`
的填入，`spawn_detached` 未外放（可见性不在授权内），冷启动拉起在
`run_view.rs` 内按同一做法复制实现。

### 逐条改动

- **R1-1 发布构建编译失败**：`src/server.rs` 的 `OpenRunRequest` 补
  `#[derive(Clone)]`（`emit` 要求载荷 Clone；这段在
  `#[cfg(not(debug_assertions))]` 下，开发构建不编译它）。`cargo check
  --release` 已进验证命令。
- **R1-2 取消采纳无效**：第一版不做取消。已采纳的卡只显示「已采纳」状态词，
  不可点；未采纳且结果落定的卡显示「采纳」。规则收进纯函数 `adoptAction`。
  测试：`adopt action has no cancel: adopted cards show a settled label and
  are not clickable`。
- **R1-3 真身误报**：`identityMismatch` 只认引擎判定
  （`errorKind === 'identity_mismatch'`），前端不再做字符串比较（claude 请求
  别名、pi 真身带 provider 前缀，比较必然误报）；实际模型原样显示，标记文案
  改「真身不符」；备选路由不再用 `split('/')[1]` 拆模型名。测试
  `flags identity mismatch only when the engine judged it` 钉住新规则（原
  「请求 sonnet、真身 claude-sonnet-4-6 → 不符」的期望已删）。
- **R1-4 模式名不再靠推断**：`EngineRun` 追加 `preset_id: String`
  （`serde(default)`，空串不写出）；`start_dispatch` 写 run 时填入本次预设
  id，`targets` 直给时填空串，预设缺省记 `builtin-economy`，拒绝路径同样
  填。查看器按 `preset_id` 从预设清单取显示名（内置用自己的名字，用户自建
  「自定义模式 · 名称」），为空或找不到显示「自定义模式」；`infer_preset_name`
  与两条推断测试删除，旧 run 文件（无该字段）按空处理。
  `docs/CONSOLE-CHANNEL.md` 的 run 字段说明补 `preset_id`。测试：
  Rust `start_dispatch_records_preset_id_in_run_file`（engine/tests.rs）、
  `preset_name_comes_from_preset_id`、
  `preset_name_labels_user_presets_as_custom`、
  `load_run_view_running_keeps_status_and_maps_preset_id`、
  `load_run_view_failed_carries_reason_and_degrades_without_graph`（旧 run
  文件形状）。
- **R1-5 冷启动 open 阻塞**：`main.rs` 不再在命令行进程里起窗口；以分离方式
  另起窗口进程（argv 仍 `open <run_id>`，Windows 上 CreateProcessW 且
  bInheritHandles=FALSE，不继承句柄、不挂父控制台，与引擎 `spawn_detached`
  同做法），随即退出 0。拉起方与被拉起方靠环境变量 `AICROSS_OPEN_WINDOW`
  区分，避免无限接力；被拉起的进程清掉标记再起窗。测试：
  `open_window_args_carry_open_and_run_id`、
  `spawn_open_window_with_spawns_and_marks_env_without_blocking`（注入
  spawn，验证参数与不阻塞）。
- **R1-6 层级与焦点**：查看页改为 `top: 40px`（让出自绘标题栏），
  z-index 12500（在 `.dispatch-overlay` 12000 之上）；打开期间
  `html.run-viewer-open` 把 `.resize-edges` 抬到 13000，缩放边条仍可拖。
  挂载后焦点落「关闭」，Tab 用焦点陷阱折返不出查看页；Esc 先用
  `elementFromPoint` 看视口中心是否命中查看页，上面还有层时不抢，只关最
  上层。
- **R1-7 轮询出错停摆**：已拿到过数据时出错保留旧数据、运行中的 run 按间隔
  重试（终态不另起循环）；只有首次读取失败才显示「找不到这次派发」。命令行
  `open` 改用 `open_run_exists`：只有 run 文件不存在按 1，其它读取错误按 3。
  测试：`open_run_exists_distinguishes_missing_from_other_errors`（Windows
  上「runs 被同名文件占住」也报 NotFound，归入找不到，测试注释已记）。
- **R1-8 竞态**：挂载点 `<RunViewer key={runId}>`；组件内请求序号
  （`requestSeq`）对轮询与采纳统一递增，过期响应不落地；采纳占线时序号作废
  的响应同样丢弃。轮询链在响应过期时仍续排下一次重取（采纳打断后不会断流）。
- **R1-9 同 run 重开不刷新**：热启动 `open-run-request` 事件带递增 `seq`
  （server.rs 的 `AtomicU32`），作为 `refreshSeq` 依赖触发重取；冷启动
  `take_pending_open` 固定 seq 0。
- **R1-10 监听注销竞态**：App.tsx 注册处加 `disposed` 标志，异步注册完成时
  若已卸载立即注销。
- **R1-11 视觉纪律**：去掉 `var(--blue)`；run 状态按 `runStatusTone` 给胶囊
  （运行中中性胶囊，失败与拒绝警示胶囊，已完成纯文字）；token 行中文标签
  用 Sans、数字用 Mono。测试：`tones run statuses: pills for running and
  failures, plain text for done`。
- **R1-12 恒真断言**：测试里 `expect(x === y).toBe(true)` 一条删除，换成
  「图标题与 agent/model 都没有 → `执行 · 未指定模型`」有区分力的断言。

### 变红验证（R1-3、R1-4）

- R1-3：临时把 `identityMismatch` 换回字符串比较，`flags identity mismatch
  only when the engine judged it` 变红（旧实现对
  `errorKind: 'identity_mismatch'` 返回 false）；恢复后全绿。
- R1-4：临时 `preset_id.clear()`，`start_dispatch_records_preset_id_in_run_file`
  变红（left "" / right "u1"）；临时让 `preset_display_name` 无视 id 恒返
  「自定义模式」，`preset_name_comes_from_preset_id`、
  `preset_name_labels_user_presets_as_custom`、
  `load_run_view_running_keeps_status_and_maps_preset_id` 三条变红；恢复后
  全绿。临时改动已全部还原（全仓无残留标记）。

### 命令行实测（本机，target/debug）

```
> aicross open                 → 缺少 run_id（exit 2）
> aicross open bad/id          → run_id 无效（exit 2）
> aicross open --run=abc       → open 不接受选项参数（exit 2）
> aicross open no-such-run-123 → 找不到 run：no-such-run-123（exit 1）
> aicross open smoke-run-r15   → exit 0，耗时约 0.1 秒，不等窗口
   （临时 run 文件放 ~/.aicross/runs 下实测冷启动：命令立即返回，恰有一个
   分离的 aicross.exe 窗口进程被拉起、没有接力链；窗口随后 taskkill，临时
   文件已删）
```

### 验证命令输出（修正轮后重跑）

```
> cargo check --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.93s
（零 warning）

> cargo check --release
    Finished `release` profile [optimized] target(s) in 9.17s

> cargo test
test result: ok. 562 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 63.26s

> cd src-ui && npx vitest run --configLoader native --pool threads
 Test Files  51 passed (51)
      Tests  744 passed (744)

> cd src-ui && npm run build
✓ built in 698ms
（main chunk > 500 kB 的提示仍为既有告警）
```

### 改动文件（修正轮）

| 文件 | 改动 |
|---|---|
| `src/engine/mod.rs` | R1-4 授权范围：`EngineRun.preset_id` 字段 + `start_dispatch` 填入 |
| `src/engine/tests.rs` | R1-4 测试一条 |
| `src/server.rs` | R1-1 derive Clone；R1-9 事件带递增 seq |
| `src/run_view.rs` | R1-4 `preset_display_name`（删推断）；R1-5 冷启动分离拉起；R1-7 `open_run_exists`；测试重写 |
| `src/main.rs` | R1-5 冷启动拉起后立即退出 0；R1-7 退出码 1/3 区分 |
| `src-ui/src/viewer/run-view.ts` | R1-2 `adoptAction`；R1-3 `identityMismatch`；R1-11 `runStatusTone` |
| `src-ui/src/viewer/RunViewer.tsx` | R1-6 焦点陷阱与层级；R1-7 出错重试；R1-8 序号丢弃过期响应；R1-2/11 展示 |
| `src-ui/src/viewer/RunViewer.css` | R1-6 top/z-index/边条；R1-11 胶囊与 token 行；R1-2 已采纳样式 |
| `src-ui/src/App.tsx` | R1-8 key；R1-9 seq；R1-10 disposed |
| `src-ui/src/__tests__/run-viewer-37.test.ts` | R1-2/3/11 测试，R1-12 换断言 |
| `docs/CONSOLE-CHANNEL.md` | `open` 行为与退出码、run 字段 `preset_id` |
| `docs/MANUAL.md` | 查看器一节：冷启动即返、真身不符、无取消采纳、出错重试 |

### 修正轮未做与不确定处

1. **R1-6 的真机核**：焦点陷阱、标题栏让位、边条抬升与 Esc 分层只做了代码
   实现与本地构建验证，未逐项手动操作窗口（同「编排者真机核」，留给编排者）。
2. **Esc 最上层的判法**：用 `elementFromPoint(视口中心)` 判查看页是否被更
   高层盖住。中心点恰好落进上层透明区（pointer-events: none）时仍视为查看页
   在最上；现有各浮层（任务便签 99999、设置 2000、结果屏 12000）中心都有
   实体，未发现误判场景。
3. **环境变量标记的传递**：`AICROSS_OPEN_WINDOW` 由被拉起的窗口进程在
   main 里清除，不会传给应用内终端；若用户手工把它设进系统环境，`open`
   会退化为在当前进程起窗（阻塞至窗口关闭），属可接受的降级。
4. **热启动转发仍只在发布包生效**（单实例插件的既有 `#[cfg]`，未动）。

## R2 编排者追加（10-01，真机核后）

发布构建（`tauri build --no-bundle`，内嵌前端）窗口真机核：冷启动 `open` 0 秒返回并直接进查看页；窗口已开时 `open` 另一个 run 切页；运行中自动刷新到终态；采纳后卡片显示「已采纳」、图落盘、run 文件字节不变；关窗重开采纳仍在；真身不误报（Codex 显示「未报告」）；标题栏未被遮挡、焦点落「关闭」。

真机核发现并修复两处（同项目的派发共用一张图导致）：

- **卡片标题写成该项目第一次派发的任务**：`RunView` 追加 `task_title`（run 文件 `original_request` 的第一行非空文本，最多 40 字），`cardTitle` 的任务名顺序改为蜂群条目标题 → 本次原始需求 → 图标题 → agent/model。测试：`run_view::tests::task_title_takes_first_nonempty_line_of_this_runs_request`、前端 `cardTitle` 新增断言。
- **遗留带上别的 run 的**：`RunView.leftover` 改为只取来源节点属于本 run 的遗留节点（读不到图时退回 run 摘要）。测试：`run_view::tests::leftover_is_scoped_to_this_runs_nodes`，`load_run_view_done_maps_reply_fields` 改为图上同时放本 run 与别的 run 的遗留；去掉过滤时两条测试变红。引擎侧 run 摘要的遗留汇总记入积压 103，待「每次派发是否新建图」裁决。

验证：`cargo check --all-targets` 零 warning；`cargo check --release` 零 warning；`cargo test` 564 passed；vitest 744 passed；`npm run build` 通过。
