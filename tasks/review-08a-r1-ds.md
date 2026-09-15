## 逐条核对（13 条）

| # | 判定 | 依据与理由 |
|---|---|---|
| 1 | 已核实修好 | `orchestration.rs:411-425` 新增 `builtin-swarm`（单节点 拆分/只读/共享，`preset_text: include_str!("../fixtures/prompts/split.txt")`）；`fill_role_defaults` 在 `:508-520` 先强制只读/共享、再按 planning 填目标（`:516` `"出方案" \| "拆分"`）；计数断言同步 `:890`(8)、`:901`(6)、`:908`；测试 `:939-959` 断言名称/节点数/字段/目标/preset_text。 |
| 2 | 已核实修好 | 六个结构体全部 `#[serde(rename_all="camelCase")]`：`dispatch.rs:518/526/534/541/549/561`（含 `SwarmMergeItem.title`、`SwarmMergeResult.test_output`）；契约 `tasks/TASK-08-swarm.md:9-10` 已写 camelCase/nodeId/title?；测试 `dispatch.rs:3448-3461` 正反断言 `nodeId`/`testOutput` 且无 snake_case。 |
| 3 | 已核实修好 | `split.txt` 为唯一来源（`orchestration.rs:420` 编译期读入，随预设 wire 返回），前端 `stamp.ts:48` 只消费 `target.preset_text`，`PRESET_NODE_TEXTS`（`stamp.ts:8-14`）已无 builtin-swarm，报告写明 B 侧不得再加。残余风险：若 B 分支的该条目活到合并，`stamp.ts:60-65` 的覆盖逻辑会静默顶掉冻结文本——只有报告文字约束，无测试护栏。 |
| 4 | 已核实修好 | `dispatch.rs:605` status 只写 `ok/error`，`merge_result` 单列；对账 `:1047` 加 `record.merge_result.is_empty()` 过滤；测试 `:3430-3446` 断言 `("ok","merged")` 且同 run/node 对账判 `unknown`（不被误判为 incomplete）。 |
| 5 | 已核实修好 | `:639-648` 读 `git worktree list --porcelain` 并 canonicalize 成集合；`:727-733` 输入路径 canonicalize 后不命中 → `skipped`+note；测试 `:3375-3393` 用另一个真 `git init` 目录断言「未登记」。次要缺陷见下（陈旧登记项会整批 Err）。 |
| 6 | 已核实修好 | `:1179` `matches!(node.role.as_str(), "验收"\|"验证"\|"仲裁"\|"拆分")`，与前端同口径；测试 `:3272-3282`（拆分+worktree → error）。 |
| 7 | 已核实修好 | `:746` `rev-list --reverse <project_head>..HEAD` 取领先提交、`:754-757` 逐个 cherry-pick；`:771-786` 验证过 → `merged`；`:836` 验证红 → `reset --hard <baseline>`（回退本件全部提交，baseline 在 pick 前取于 `:749`）。测试 `:3362-3373` 覆盖「agent 已提交件并入」。缺口：多提交逐序 pick、ahead 路径验证红回退均无测试。 |
| 8 | 已核实修好 | 六项逐一对上：`:670` `settings.timeout_secs`→验证(`:771/:823` 传 `run_validation`，`:2093`)与写锁(`:671`)；`:664-668` 留痕遵守 `write_project_dispatch`；`:780/:836` `test_failed` 带 note；`:796-797` 提交信息用 `title`（空回退 node_id）；`:453-466` glob 归一化（`./`、尾 `/`、纯目录→`/**`）+ `:484` `case_sensitive:false`；`:508-512` `check_swarm_plan` 改 `async + spawn_blocking`。小瑕：合并侧 timeout 未像派发侧那样 `clamp(1,3600)`；`write_project_dispatch=false` 时用 `aicross_root()`（`:668`）而派发用 `dispatch_data_root()`。 |
| 9 | 已核实修好 | `:658-662` merge 超限件切出、`skipped`+note「超过蜂群件数上限」（测试 `:3416-3427`，断言第二件未落盘）；`:511` check 只处理前 N。唯一缺口：`SwarmPlanCheck` 无 note 字段（契约也未定义），check 侧只能静默截断，与「超过上限返回 note」的字面要求差一半。 |
| 10 | 已核实修好 | `:3205-3212` `git_available`/`require_git`（不满足 eprintln 跳过），10 条 git 测试全部带护栏；`:3219-3236` 真 `git init`+`worktree add`；`:3375-3393` 无改动/不存在/未登记；`:3395-3414` 3 件中间件验证红、后件仍 merged 且前件仍在；`:3430-3446` 留痕不影响对账。 |
| 11 | 已核实修好（测试欠一条） | `:553-554` `#[serde(default)] title`、`:796-797` 入提交信息、契约 `:10` 同步；但测试只断言了 serde 形状（`:3449`），`merge_item` 永远写空 title（`:3238-3246`），「提交信息含 title」无任何断言。 |
| 12 | 已核实修好（测试未补） | 闭环成立：失败件 worktree 保留其提交 → 下次 `:746` 判领先 → `:754` cherry-pick → `:771` 验证过 → `merged`。但全仓只有单轮调用（`:3290-3434`），没有「第一次 test_failed、修好验证后第二次 merged」的测试。 |
| 13 | **改法有问题** | 启动失败 Err→skipped 做对了（`:757`、fresh 路径 `:863`）；但三处不对：①`:761-767` 的 abort 判据方向反了——Err/非冲突型 Ok 失败时根本没有 cherry-pick 在进行，`git cherry-pick --abort` 必然非零，于是追加「中止失败」并置 `halt_after_item`，后续件被 `:904` 全标 skipped，恰好把「启动失败只标 skipped」变成「误停整批」；②超时（`process_succeeded` 假 + `timed_out` 真）在 `:756`/`:851` 仍判 `conflict`，未按第 13 条降为 skipped；③halt/后续件文案没有第 13 条要求的「项目可能处于冲突态，需人工 `git cherry-pick --abort`」。测试：无。 |

## 专项核对结论

- **builtin-swarm & preset_text**：闭合。`presets_from:620-635` 先 `fill_role_defaults` 再序列化，`preset_text` 非空即上 wire（`skip_serializing_if` 只在空时省略），前端盖章只消费该字段 ✔
- **合并留痕**：`status` 只有 ok/error（`:605`），对账显式忽略合并记录（`:1047`），回滚路径靠 `before_dir` 为空天然跳过（`:1769`）✔
- **worktree 登记校验**：canonicalize 后比对 `worktree list`，未登记即 skipped ✔（陈旧/不可 canonicalize 的登记项会让 `:639-648` 返回 Err → 整批 skipped，而非该件 skipped）
- **领先提交按序 pick / 验证红回退本件全部 / 重试闭环**：逻辑齐备（`:746-790`、`:836`、`:904`）✔ 测试只覆盖单提交与单轮
- **cherry-pick 分级**：部分（见第 13 条）
- **swarm_limit**：两条命令都生效 ✔（check 侧无 note）
- **测试真实性**：临时 git 仓库真跑 ✔，中间件失败后续继续 ✔，git 缺失跳过 ✔
- **新缺陷/回归**：未发现 2–7 期路径回归——`process_note` 只被 swarm 路径与测试使用（`:404/470/642/740…`），无外溢；`run_one` 只读护栏未动（盲节点强制只读 `:2338-2340`），拆分只加在派发门禁；对账唯一改动是 `merge_result` 过滤。**新引入**的问题都在 swarm 自身：①abort 误判停批（上述）；②`:713-714` `rev-parse HEAD` 失败时 `project_head` 为空 → 抢范围退化成 `..HEAD`，该件会被误报「没有改动」；③冲突路径 `reset` 失败被 `let _` 吞掉（`:762`），fresh 路径 reset 失败会把 note 覆盖成 git 错误、丢掉「验证命令未通过」（`:842`）；④`write_project_dispatch=false` 用 `aicross_root()`，与派发路径的 `dispatch_data_root()` 在测试构建下不一致；⑤R1 顺手改了既有 ACP 用例夹具 `dispatch.rs:4115` 超时 1s→10s，超出本轮范围；⑥报告自相矛盾（三处「修正轮 R1」、「没做 A6」与 R1 已做并存、289/296 混排），且报告内没有任何一次 `npm run build` 成功日志（只有 spawn EPERM），「build 零 warning」在报告文本里无证据。

## R1 之后 TASK-08A 是否可交付？

**可交付（有条件）：A 节 1–6 与 13 条中 11 条已闭合、主路径有真 git 回归护栏，但第 13 条的 abort 误判与第 11/12 条的测试缺口应在 B 节联调合并前收尾。**

未闭合项（按严重度）：

1. **[中] 第 13 条未收口**：非冲突失败（含启动失败、超时）会经 `:761-767` 误判「abort 失败」并停整批（`:904`），超时仍标 `conflict`，缺人工 `git cherry-pick --abort` 指引，且无测试。
2. **[中] 第 12 条测试缺失**：无「test_failed → 修好 → 第二次 merged」端到端；第 7 条的多提交逐序 pick、ahead 路径验证红全回退同样无测试。
3. **[中] 第 11 条测试缺失**：提交信息 `<title>` 无断言（只有 serde 形状断言）。
4. **[低] worktree 登记项 canonicalize 失败 → 整批 skipped**（`:639-648`），陈旧登记会挡住其它件。
5. **[低] `project_head` 取空时领先判定退化**，该件被误报「worktree 没有改动」（`:713-714`+`:746-789`）。
6. **[低] 回退失败被吞/被覆盖**：冲突路径 `:762`、fresh 路径 `:842`。
7. **[低] check 侧超限无 note、非法 glob 仍整条命令 Err**（`:447-448`，DS F4.4 未闭）。
8. **[低] 写盘开关路径不一致**：`:668` 用 `aicross_root()`，测试构建下与 `dispatch_data_root()` 不同（可能写用户家目录）。
9. **[低] 范围外改动与证据缺口**：`dispatch.rs:4115` 既有夹具超时 1s→10s；报告自相矛盾且无 build 成功日志。**（说明：我未复跑 cargo/vitest，以上为静态核对，测试通过数取编排者复盘。）**