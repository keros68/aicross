# TASK-08A 修正轮 R1 复审

已通读 `tasks/fix-08a-r1.md`、`tasks/TASK-08-swarm.md`、`tasks/REPORT-08A.md`「修正轮 R1」三处、`src/dispatch.rs`（swarm 实现与测试）、`src/orchestration.rs`（内置预设/角色默认/测试）、`src/server.rs`（注册）、`fixtures/prompts/split.txt`，并旁证 `src-ui/src/graph/stamp.ts`、`validate.ts`。

## 13 条逐条核对

| # | 结论 | 依据 |
|---|------|------|
| 1 builtin-swarm 预设 | **已核实修好** | `src/orchestration.rs:411-425` 单节点 拆分/只读/共享、目标留空；`orchestration.rs:901-908` 总数 5→6 与既有断言同步；盖章测试 `orchestration.rs:939-959` 验证字段与 planning 目标填充 |
| 2 camelCase | **已核实修好** | `src/dispatch.rs:517-569` 五个 swarm 结构全 `rename_all="camelCase"`；往返测试 `dispatch.rs:3448-3460` 断言 `nodeId`/`testOutput` 且 snake_case 键不存在；契约 `tasks/TASK-08-swarm.md:9-10` 已同步 |
| 3 split.txt 单一来源 | **已核实修好** | `src/orchestration.rs:420` `include_str!("../fixtures/prompts/split.txt")` 填入 `preset_text`；`src-ui/src/graph/stamp.ts:48` 盖章只消费 `target.preset_text ?? ''`，`PRESET_NODE_TEXTS`（stamp.ts:8）不含 builtin-swarm，无重复文本可删；测试 `orchestration.rs:958` |
| 4 合并留痕 status | **已核实修好** | `src/dispatch.rs:605` 只写 ok/error，`merge_result` 单列（`:610`）；对账 `dispatch.rs:1047` 以 `merge_result.is_empty()` 过滤合并记录；真实 git 测试 `dispatch.rs:3430-3445` 验证 verdict 为 unknown |
| 5 worktree 归属 canonicalize | **已核实修好** | `src/dispatch.rs:639-648` 登记清单逐条 canonicalize，`:728` 入参 canonicalize 后比对，未命中 skipped；测试 `dispatch.rs:3375-3392` 覆盖不存在/未登记/无改动三种 |
| 6 拆分进只读护栏 | **已核实修好** | `src/dispatch.rs:1179` 引擎角色集合含「拆分」，与 R2-7 仲裁同一入口（dispatch_stage_core，run_one 之前）；测试 `dispatch.rs:3271-3282`。注：前端 `validate.ts:26` 尚无拆分，属 TASK-08B 未落地，非本轮缺口 |
| 7 已领先提交按序并入 | **已核实修好** | `src/dispatch.rs:746-759` `rev-list --reverse {基线}..HEAD` 逐个 pick，基线在 pick 前取（`:749`）；验证失败 `:782` reset 回基线回退本件全部提交；测试 `dispatch.rs:3361-3372` |
| 8 超时/写盘/note/title/glob/spawn_blocking | **已核实修好** | 超时 `dispatch.rs:670`（`:771`/`:823` 生效）、写盘开关 `:665-669`、test_failed note `:780`/`:836`、title 提交信息 `:796-797`、glob 归一化+大小写 `:453-465`/`:482-486`、`check_swarm_plan` 进 spawn_blocking `:509`；脏树 note 带 porcelain 摘要 `:697` |
| 9 swarm_limit | **已核实修好** | 检查 `dispatch.rs:475` `.take(limit)`；合并 `:658-663` 超限 split_off 且记 skipped+留痕（`:910-915`）；默认 4（`:68`，测试 `:3458-3459`）；测试 `:3416-3427` |
| 10 测试补强 | **已核实修好** | 全部用 `swarm_repository`（`dispatch.rs:3219`）真实临时 git 仓库+worktree；中间失败继续 `:3395-3413`（merged/test_failed/merged 且文件状态逐一断言）；git 缺失 `require_git` eprintln 跳过 `:3209-3211` |
| 11 入参带 title | **已核实修好** | `src/dispatch.rs:554-555` `#[serde(default)] title`；提交信息用它（`:796-797`，空则回退 node_id）；契约 `TASK-08-swarm.md:10` 含 `title?` |
| 12 失败件重试闭环 | **改法有问题（仅测试缺口）** | 机制已由第 7 条路径覆盖：test_failed+reset 后 worktree 仍领先，再次合并走 rev-list 重新 pick（`dispatch.rs:746`）；但 fix 明确要求的「第一次 test_failed → 修好后第二次 merged」闭环测试不存在（9 处 `merge_swarm_items_core` 测试调用均单次合并） |
| 13 cherry-pick 失败分级 | **改法有问题（半边）** | 启动失败→skipped 已分级（`dispatch.rs:757`、`:865-868`），abort 失败停批+后续 skipped 已做（`:765-767`、`:903-907`）；但 **cherry-pick 超时仍归 conflict**：`:756`/`:850` 的 `Ok(pick)` 分支不区分 `timed_out`，与 GLM 5「超时不是 conflict」不符；指定的分级测试也未补 |

无新缺陷判定：对账仅加一个过滤条件（`dispatch.rs:1047`），普通留痕 `merge_result` 为空不受影响；run_one 只读强制路径未动，拆分门禁在 dispatch_stage_core 入口与第 2–7 期仲裁同型；stamp.ts 改为消费 `preset_text` 后 builtin-jury 视角文本覆盖逻辑保留（`stamp.ts:60-66`），294 条 vitest 通过佐证无回归。

## 结论：R1 之后 TASK-08A 是否可交付？

**可交付**——13 条中 11 条已核实修好、引擎行为无阻塞缺陷，剩余为测试与分级措辞级残留，不必单开一轮，可随 TASK-08B 联调或 R2 收口。

仍未闭合项（按严重度）：
1. **（中）第 13 条**：cherry-pick **超时**仍标 `conflict` 而非 `skipped`+note（`src/dispatch.rs:756`、`:850`），与 fix 原文不符；前端会把它当「冲突待裁决」展示。
2. **（低）第 13 条**：启动失败分级与 abort 失败停批均无直接测试（难以注入属实话，但 fix 要求了「补测试」，报告未按「不采纳」明示）。
3. **（低）第 12 条**：缺「第一次 test_failed → 修好后第二次 merged」的闭环测试（机制已由第 7 条覆盖）。
4. **（低，措辞）** abort 失败 note 为「中止 cherry-pick 失败，已停止后续合并」（`dispatch.rs:766`、`:857`），未写明「需人工 `git cherry-pick --abort`」。
5. **（交接提示，非 08A 缺口）** 前端 `validate.ts:26` 只读集合与 `ROLES` 尚无「拆分」，`PRESET_NODE_TEXTS` 不得在 B 侧补 builtin-swarm 文本——都在 TASK-08B 范围内落地时对齐。