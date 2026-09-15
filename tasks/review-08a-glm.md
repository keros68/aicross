# TASK-08 A 节评审（phase/8a）

已核对 `docs/PRODUCT.md` 第 3 期草案、TASK-08 契约、REPORT-08A 与全部改动文件。报告声称的 8 条 task08a 测试全部真实存在且在跑；`server.rs` 全文 4620 行含新增行均为 CRLF。未复跑 cargo/vitest（本会话无 shell）。

## 1. 需求完整性

基本逐条落地：`Node.swarm` 镜像（`src/orchestration.rs:58`、`orchestration.rs:64`）、留痕两追加字段带 `serde(default)`（`src/dispatch.rs:361`、`dispatch.rs:363`）、`check_swarm_plan` 签名与返回形状、`merge_swarm_items` 签名与 camelCase 结果、`split.txt` 四要素、拆分角色默认、server 注册，均有对应实现与测试。发现 3 条：

- `src/dispatch.rs:675` — 提交信息是 `aicross swarm item N: <node_id>`，契约写 `<title>`。为什么：契约自相矛盾（冻结入参 `SwarmMergeItem` 没有 title 字段），报告已如实记录偏差，但这是用户会在 `git log` 里长期看到的文案。建议：给 `SwarmMergeItem` 加 `#[serde(default)] title: String`，空则回退 node_id；或把契约该行改成 node_id 口径，别让两份文本各说各话。
- `src/dispatch.rs:37` — `swarm_limit` 只有定义和默认值，A 侧无任何消费点，合并命令也不校验件数上限。为什么：契约把截断放在 B 侧解析，属可接受分工，但引擎侧对任意长度 `items` 照单全收。建议：`merge_swarm_items_core` 开头对 `items.len() > swarm_limit` 给整批 skipped + note，一处防御性校验即可。
- `src/dispatch.rs:361` — `swarm_item` 是 `Option<u32>`，TASK-08 契约写 number，但 `docs/PRODUCT.md:219` 草案写 `swarm_item: string`。实现按任务书没错，建议顺手把 PRODUCT.md 那行同步成 number，避免下期照旧文写 B 侧类型。

## 2. 逻辑正确性

正确的部分先钉住：重叠判定取两条 glob 匹配集交集的最小路径（`dispatch.rs:481`，同一现存文件、确定性输出）；排除目录复用 `excluded_snapshot_directory`（`dispatch.rs:425`、`dispatch.rs:366`）与快照一致；`sort_by_key` 稳定保序；脏树/status 失败整批 skipped 且每件写留痕；后续件不因前件失败停；冲突 note 含 worktree 路径；`test_output` 经 `safe_trace` 后按 UTF-8 边界截 8KB；超时与非零都有可读 note（`process_note` + `safe_trace`）。发现 5 条：

- `src/dispatch.rs:737` — cherry-pick 的 `Err`（进程启动失败，如 git 不存在）被标成 `"conflict"`。为什么：conflict 的语义是“cherry-pick 已运行且遇冲突”，启动失败归 conflict 会误导 G3 裁决方向（用户会去查文件冲突而不是环境）。建议：`Err` → `skipped` + note，只有 `Ok` 且非零/超时才判 conflict。同型问题：`run_validation` 的 Err 把 spawn 失败也归 `test_failed` 并 reset 掉已并入的 commit（`dispatch.rs:710`），环境问题被当内容问题回退——至少在 note 里区分。
- `src/dispatch.rs:729` — `cherry-pick --abort` 自身失败时只往 note 追加一句，然后继续下一件。为什么：此时项目停在未完成 cherry-pick 状态，“abort 后回到并入前状态”已破坏，后续每件 cherry-pick 都会失败且全被误标 conflict，一件错误污染整批。建议：abort 失败时立即终止循环，返回已有结果并在末件 note 标明“项目处于 cherry-pick 中，需手动处理”。
- `src/dispatch.rs:668` — “worktree 无改动”用 `diff --cached --quiet` 判定，但 `test_failed`/`conflict` 之后 worktree 分支仍持有那条 commit，用户裁决后重按“合并”会走到这里被 skip 为“worktree 没有改动”——失败件永远无法靠重试并入。为什么：B 节 G3 的设计就是“冲突与测试红交用户裁决”，重试是第一入口。建议：无暂存改动时再查 `git rev-list --count <merge-base>..HEAD`，>0 则直接 cherry-pick 现有 HEAD；至少把 note 改成明确指引（“分支已有提交未并入，可手动 cherry-pick <sha>”）。
- `src/dispatch.rs:640` — 脏树 note 是硬编码“项目目录有未提交改动”，不带 porcelain 内容。为什么：验证命令自己产生的未跟踪缓存/构建产物 reset --hard 不清（只动 tracked），一批跑完项目常会“脏”，下一次整批被 skipped 时用户看不到脏在哪。建议：note 附 `status.stdout` 前几行。
- `src/dispatch.rs:465` — glob 匹配 `case_sensitive: true`。为什么：Windows 路径大小写不敏感，拆分模型给的 glob 大小写漂移（`Readme.md` vs `README.md`）会造成假 unmatched、漏报 overlap——而 overlap 是 G1 红线的输入。建议：Windows 上用 `case_insensitive: true`（`cfg!(windows)` 选择）。

## 3. 安全

- `src/dispatch.rs:647` — `item.worktree` 直接 `PathBuf::from` 后就在其中执行 `git add -A && git commit`，没有校验它是本项目登记的 worktree。为什么：传错或传恶意路径会在任意目录（乃至另一个仓库）做 git 写操作；本仓 UI 虽可信，但这条命令是契约暴露面。建议：合并前 `git worktree list --porcelain` 校验 canonicalize 后的路径在列表内且隶属本项目，否则 skipped。
- 未发现其余问题：目录遍历用 `symlink_metadata` 并跳过 symlink/junction（`dispatch.rs:419`、`dispatch.rs:1390`），不越界；git 全部经 `run_process_registered` 直跑、无 shell 拼接，动态参数只有 rev-parse 的十六进制 sha 和作为单参的提交信息；验证复用 `run_validation`（120s 超时、ANSI 清洗、错误含退出码）；无 push、无远端操作。小点：`dispatch.rs:699` 验证进程 `registry_key=None`，不可经运行登记表取消，与 git 步骤不对称。

## 4. 代码质量与契约

- `src/dispatch.rs:490` — `check_swarm_plan` 是同步 `fn` 命令。为什么：Tauri 同步命令跑在主线程，而它要 `git status` + 遍历整棵项目树，大项目会卡 UI；同批新增的 `merge_swarm_items` 用了 `spawn_blocking`（`dispatch.rs:786`），同批不一致。建议：改成同样的 `async + spawn_blocking`。
- `src/dispatch.rs:646` — 合并循环 7 层嵌套 match 约 120 行，事务分支难读也难测单分支。建议：抽 `merge_one(...) -> SwarmMergeResult`，主循环只剩锁、脏检查、排序、留痕。低优先。
- 未发现其余问题：追加式契约成立（旧图 `swarm`→None、旧留痕缺字段取默认，均有测试）；`rename_all = "camelCase"` 覆盖全部新结构体并有序列化断言；改动仅限 dispatch/orchestration/server/Cargo 四处，2–7 期路径（run_one、worktree 创建、对账、回滚、心跳）零触碰；`server.rs:3940-3941` 注册到位且保持 CRLF。

## 5. 测试

报告所述测试全部真实：`swarm_repository`（`dispatch.rs:3070`）在 temp_dir 里 `git init` + 真实 `worktree add` 两个 worktree，四条合并/检查测试真跑 git；断言命中目标行为——乱序入参按 item 序合并、`rev-list --count` 3、最新提交含 "item 2"（`dispatch.rs:3129-3133`）；冲突件断言 `shared.txt` 仍是第一件内容且 `CHERRY_PICK_HEAD` 不存在（`dispatch.rs:3155-3157`）；验证失败断言 `two.txt` 被清、count 2（`dispatch.rs:3171-3173`）；脏树全 skipped 且仍写两条留痕（`dispatch.rs:3185-3189`）。验证测试顺带钉住了 accept 优先、空 accept 回退 `validate_command`。最该补 3 个：

1. **前件失败、后件继续并入**——现有测试失败件都在最后，“单件失败不停整批”（PRODUCT.md 已定③）从未被执行路径覆盖；
2. **worktree 无改动 → skipped**——契约明文路径，同时钉住 finding 2-c 的重试语义（当前实现下 test_failed 重试会被 skip，这个行为需要一个被测试固定的决策）；
3. **worktree 目录不存在 → skipped + note**——与安全项 3-a 配套的防御分支，现在完全无覆盖。

## 结论：对照 A 节原始需求是否完整实现？

**1→5 条完整实现且验证真实，A6（builtin-swarm）明示未做；另有 2 处契约偏差与上述逻辑缺陷需在 B 节或补做轮收敛。**

缺项（按严重度）：
1. A6 `builtin-swarm` 预设未做——任务书外部追加后本次明示限定 1→5，内置清单断言（5→6）也未同步；
2. 提交信息 title 缺失，用 node_id 顶替（契约/入参自相矛盾，需定口径）；
3. worktree 未校验登记归属即执行 git 写操作（安全防御缺失）；
4. test_failed/conflict 后重试必被“没有改动”skip，失败件无重试闭环；
5. cherry-pick 启动失败误标 conflict、abort 失败不终止整批；
6. `swarm_limit` 无消费点、`check_swarm_plan` 阻塞主线程、glob 大小写在 Windows 偏严、验证命令脏残留无提示（均为低危改进项）。