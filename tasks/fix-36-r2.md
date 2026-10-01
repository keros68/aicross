# TASK-36 修正轮 R2（10-01）

依据：`tasks/review-36-r1-claude.md`（Claude 子代理复审，下称 C）、`tasks/review-36-r1-glm.md`（GLM 复审，下称 G）。下列条目编排者都已对照代码核实。范围、禁区、换行规则仍按 `tasks/TASK-36-headless-engine.md`；本轮允许改 `src/**`、`Cargo.toml`、`Cargo.lock`、`docs/CONSOLE-CHANNEL.md`、`tasks/REPORT-36.md`。**不改** `fixtures/engine-golden/` 与 `src-ui/`。不 commit，不 push。完成后在 `tasks/REPORT-36.md` 末尾追加「R2」一节：逐条写改了什么、对应测试名；同时把 R1 节里与代码不符的三处说法（「dispatch_stage_core 成功后清理取消注册表」「orchestration.rs 删 stamp」「preserve_order 之前已由 schemars 传递开启」）改成事实。

## 必修

**R2-1 含中文的项目路径导致崩溃**（C 高-1）
`src/engine/mod.rs` 里 `normalized` 的 `strip_prefix_ci` 用 `value[..prefix.len()]` 按字节切片，切在多字节字符中间会 panic；`pick_project_graph` 对图目录里每张图都调用它。改为按字符边界安全的写法（如 `value.get(..prefix.len())`，取不到即不匹配）。测试：`D:\项目`、`D:\项`、`\\?\D:\论文\数据` 等含中文路径各一例，断言不崩且结果与 TS `normalizedPath` 口径一致；再加一条「图目录里有一张中文路径的图时，对另一个项目的 `plan_dispatch` 照常工作」。

**R2-2 落图前先合并，防止并发 run 互相覆盖**（C 高-2、C 中-1）
照 TS `mergeConsoleCommit`（`src-ui/src/console-channel.ts` 约 351-355、408-410 行）移植：每次保存图之前从磁盘读回该图的最新版本，只用「本 run 上次提交以来改动过或新增的节点」覆盖它，其余节点以磁盘为准；`root` 取本 run 的值。生产路径（文件落盘的 sink）用合并写；重放测试的 sink 行为不变。测试：两个 run 交替提交同一张图，双方的节点与结果都保留；`adopt` 在 run 进行中改了某节点的 `adopted`，后台随后提交别的节点时不把它抹掉。

**R2-3 `adopt` 不写 run 文件**（C 中-1）
`adopt` 只改图（经 R2-2 的合并写），不再整写 run 文件；返回值用磁盘上的 run 与刚改好的图现算摘要。测试：`adopt` 前后 run 文件内容不变。

**R2-4 后台进程启动时先确认 run 仍在运行**（C 中-2、G 低-2）
`run_background` 读到 run 后，若 `status` 不是 `running`（例如已被判「引擎进程未能启动」），直接退出、不写 pid、不派发。测试：run 文件为 `failed` 时调用 `run_background`，不发起任何派发、run 文件不变。

**R2-5 后台子命令不挂回父进程控制台**（C 中-3）
`src/console_cli.rs` 的 `run_if_subcommand` 对 `__run` 跳过 `attach_parent_console()`。测试：用可注入的方式断言 `__run` 分支不调用挂接（或把挂接判断抽成纯函数测）。

**R2-6 推进中途失败的收尾只动本 run 的节点**（C 对 R1-8 的「部分」）
R1-8 的收尾把整张图里所有 pending 节点都标失败，会误伤同图其它 run。改为只处理本 run 的节点（run 文件里的节点 id 与本 run 新建的节点），收尾时用 `node_summary` 重算 run 摘要的 `nodes`。补一条测试（注入一个在提交时返回错误的 sink）。

**R2-7 `preserve_order` 只在测试构建里打开**（C 低-4，`Cargo.lock` 已证实是本轮首次开启）
生产依赖恢复为 `serde_json = "1"`；在 `[dev-dependencies]` 里给 serde_json 打开 `preserve_order`，金标准测试照常通过。`Graph.nodes` 的 `IndexMap` 保留。改正 `Cargo.toml` 里不实的注释。验证：`cargo tree -e features -i serde_json`（非 dev 构建）里不再出现 `preserve_order`。

## 应修（小）

- **R2-8** 取消：登记在飞节点之后再检查一次取消标志；判定取消时，本阶段已经返回的结果先写回图，再把仍未返回的节点标「派发已取消」。（C 低-1）
- **R2-9** `upstream_artifacts` 第一轮只在允许集内做 DFS，照 TS `orderByDfs(graph, allowed)`。（C 低-2）
- **R2-10** 启动窗口：`pid == 0` 且 `updated_at` 比当前时间晚 60 秒以上，按过期处理。（G 低-1）
- **R2-11** `CreateProcessW` 拼命令行处加注释：参数只含 `__run` 与已校验的 run id，不做转义；以后传用户文本必须先补转义。（C、G）
- **R2-12** 补测试：退出码 3 的另两种来源——run 目录不可写、后台进程起不来（可注入失败的 spawn）。（C 对 R1-29 的「部分」）

## 不在本轮（进积压）

- 金标准输入时间按原始先后重建、检查顺序再多钉几对（C「金标准不足」）：要动前端测试与夹具，另起一期。
- 「用管道捕获 `dispatch` 输出时提前返回」的端到端测试：由编排者真机核覆盖。

## 验证

任务书第 7 节四条命令全绿。R2-1、R2-2 的新测试须能在修复临时改回时变红（编排者会抽查）。
