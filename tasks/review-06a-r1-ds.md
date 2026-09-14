## 复核方法与范围

本会话没有 shell（工具只有 read/grep/find/ls），所以下面是**静态读码复核**，`cargo 250 / vitest 221 / build` 与 live 跑批按编排者结论采信、不重复验证。已读：`tasks/fix-06a-r1.md`、`tasks/TASK-06-consistency.md`（契约节）、`tasks/REPORT-06A.md`「修正轮 R1」、`tasks/review-06a-glm.md`、`tasks/review-06a-ds.md`、`src/dispatch.rs`（全文）、`src/process.rs`、`src/server.rs`（注册处）、`src/channels/mod.rs`（白名单/测试）、`fixtures/prompts/execution.txt`、`src/orchestration.rs`、`src-ui/src/graph/{dispatch,mutate,types}.ts`（旁证）。未读 `target`/`node_modules`/`dist`/`port`/`src/channels/acp.rs`（按约定）。

---

## 16 条逐条核对

| # | 结论 | 证据与理由 |
|---|---|---|
| 1 | **已核实修好** | `uses_single_writer` 定义/赋值 `dispatch.rs:1680,1691`；回滚条件 `:1707`、快照落盘条件 `:1932` 都已改用生效模式；E2E `task06_rollback_before_runs_inside_run_one_before_redispatch`（`dispatch.rs:3308`）真用了**两个不同 run_id**（`run-a` 3332 / `run-b` 3336），子进程 stub 里断言「回滚已发生才继续」（3337），并断言 `tracked.txt=="before"`、`created.txt` 消失（3340-3342）。 |
| 2 | **已核实修好**（有一处残留，见下） | `latest_before_dir` 扫项目 `.dispatch` + 运行目录（`:1103-1131`），取「最新非空 `before_dir`」，再退 `.dispatch/<当前 run>`（`:1134-1135`）；无快照 note 已改成「没有找到派发前快照（首次派发或快照已清理）」（`:1137`）；契约第 8 行已追加「回滚按 `node_id` 找最新快照，不要求复用 run_id」。残留：`:1123` 的 workdir 过滤是「canonicalize 成功且不等才跳过」——canonicalize 失败时**放行**，理论上会取到别的项目/别的用例的留痕（node_id 是 `crypto.randomUUID()`，实际要靠复制/删除项目才触发，低）。 |
| 3 | **已核实修好**（删除路径漏了父链校验；测试可能整条跳过） | `safe_relative_path` 已拒绝 `RootDir/CurDir`（`:1047-1052`）；恢复前 `symlink_metadata` 命中链接/junction 先删链接（`:1152-1160`）；`create_dir_all(parent)` 后 canonicalize 校验在项目内（`:1162-1166`）；`.created` 里 `/x` 进 `skipped`（`:1172-1174`）。测试齐全：`3376`（RootDir/CurDir）、`3383`（`/x` 不越界）、`3511`（Windows `symlink_file`，失败时 `eprintln`+return 跳过——**在无 Developer Mode 的机器上这条等于从未真断言过**）。 |
| 4 | **已核实修好** | 开关关掉时 `log_dir = run_root.join("dispatch")`（`:624-628`），快照随之落运行目录并返回绝对路径（`:1056-1061`、`:1094-1098`）；`.gitignore` 由 `ensure_log_dir` 写 `*`（`:2297-2305`，无斜杠的 `*` 按 git 规则匹配任意层级 → `.dispatch/<run>/<node>.before/` 全覆盖）；persist 失败只 `append_runner_note`（`:1932-1935`），`status` 不被翻转。测试 `3395`、`3439`。残留：契约「相对项目目录」未补绝对路径例外；`ensure_log_dir` 仅在 `.gitignore` 不存在时写；无「覆盖子目录」的断言（推理而非测试）。 |
| 5 | **已核实修好**（测试空转） | 四条 ACP 分支 + `previous_runner=="acp"` 分支现在全走 `append_runner_note`（`:1780,1809-1812,1819,1824,1840`），无残留整体赋值（`2238` 的 kimi 分支是自带前值的整体赋值，不丢回滚 note）。但 `task06_acp_runner_note_append_preserves_rollback_note`（`:3424`）只断言 helper 自身语义，**不钉调用点**——将来改回覆盖式赋值这条测试仍绿。 |
| 6 | **已核实修好**（测试未覆盖题面点名的那一点） | `section()` 按行扫、取**最后**一个标题、截到下一个 `## `（`:393-410`），旧格式无产物段能取回正文。测试 `3410` 的「旧格式」半边有判别力（旧实现的 `split_once` 会判空），但「正文内嵌 `## 产物 JSON` 字样」实际写在**「任务全文」段**里（不是回复正文），新旧实现都不会错位 → 题面要求的那个 case 没真正被覆盖；另外正文里若有行首 `## xxx` 仍会截断（题面规则本身如此）。 |
| 7 | **已核实修好**（两处测试仍弱） | process 测试改成真压边界（`process.rs:513-527`：8MB-17 首行 + 32 字节次行，断言标记入队、`try_recv==None`）；active-run 用 `Drop` 守卫（`:3359-3361`）避免中毒 ✓；python 缺失三处都 `eprintln`（`:3310,3440,3462`）。弱点：① process 测试**无同步点**（下有详述，非确定性）；② 「释放半边」（`:3371-3373`）只验测试自己的 `Reset`，没有第二次走 `dispatch_stage_core`。 |
| 8 | **已核实修好** | `ReconcileNode` 加 `session_id/runner/usage`、`ReconcileUsage.cache_read`，`rename_all="camelCase"`（`:248-271`）；回填 `:467-471`（tokens_in/out/cache_read → input/output/cacheRead）；测试 `3256` 断言 `sessionId/runner/usage`；契约第 8 行已追加。注：`DispatchRecord` 的多数旧字段无 `#[serde(default)]`，跨版本旧留痕（缺 `vendor/billing_source`）会整条解析失败→`unknown`，而非契约说的「缺失为空/0」（本版自洽，低）。 |
| 9 | **已核实修好**（题面后半句只做了一半） | 落盘只在 `uses_single_writer && entered_channel`（`:1932`），准备失败/取消/构建失败/回滚失败都到不了 → 旧快照保留；测试 `3461`（第二次用未知 agent 早退，断言旧快照仍能复原 `a.txt`）有判别力。残留：题面「至少不用空快照覆盖非空快照」只覆盖「没进通道」——**进了通道但零改动**（ACP 立即失败、模型没改文件）仍会 `remove_dir_all` 后写空快照（`:1032-1035`）。 |
| 10 | **已核实修好**（测试对上限不敏感） | 回滚直接 `read_dir` 遍历 `.before`（`:1149-1161`），顺序＝收集 → 逐条恢复（`:1163-1170`）→ 删 `.created`（`:1171-1179`），无中途 `Err`（`:1136` 的「无快照」也是 `Ok`）。但 `task06_rollback_reads_before_directory_beyond_snapshot_total_limit`（`:3482`）只是拿 `ROLLBACK_TEST_TOTAL_LIMIT+1=513` 字节的文件跑一遍——**回滚路径压根不读任何总量上限**，旧实现（复用带 128MB 上限的快照器）这条测试也会通过 → 不具判别力。 |
| 11 | **已核实修好** | 命令内 `acquire_project_write(..., 30s, ...)`（`:481-486`）；契约第 9 行已追加「命令取项目写锁，与派发互斥」；测试 `3494` 真持锁跨线程、断言持锁期间文件未变、释放后才复原 ✓。 |
| 12 | **已核实修好** | `record_order_key` 解析 RFC3339→毫秒、同刻 ok 优先、再按文件名（`:419-425`），用于对账（`:455`）与快照定位（`:1128`）；测试 `3431` 两条断言都有判别力（字符串序会失败、ok 优先级会失败）✓。 |
| 13 | **已核实修好**（口径在契约、未加代码注释） | guard + RAII 在 `:611-620`、`Drop` 在 `:826-833`，覆盖其后所有 `?` 退出路径；契约第 13 行「进程内、run 级互斥…RAII 释放…重启不保留」与代码一致。报告写「已修（src/dispatch.rs:610）」其实指向的是未改动的 guard 代码，本轮**没有新增**「进程内/run 级」的代码注释，也没有「第二次真派发能进」的测试。 |
| 14 | **已核实修好** | 两条 note 都带「排除目录（.git/node_modules/target/dist…）与链接内的改动不在快照内」（`:1137`、`:1193`），经 `:1707 → :1765 → :2001/:2056` 进结果与留痕。残留：note 只报「已回滚 N 文件」，不报 `skipped` 条数（跳过大文件时读起来像全成功），`skipped` 只在数组里。 |
| 15 | **已核实修好**（同第 7 条） | 同 7：`process.rs:513` 换成行为断言、`:3359` 用 Drop 守卫；但两处强度问题相同。 |
| 16 | **已核实修好** | REPORT 第 18 行已改成「Rust 侧只提供并保存 `fallback_from/fallback_to`；标记行由前端在 B.3 停写」；旁证：前端仍在 `src-ui/src/graph/dispatch.ts:239-241` 写标记、`fallbackNoticeOf`（`:197-200`）读标记，本轮确实未动前端；禁外部动作按「可写」挂载归 06B R1（现 `execution.txt:2` 仍只随「执行」角色注入）——报告口径与代码一致。 |

---

## 特别核对项的答复

- **第 1、2 条**：按 `uses_single_writer` 生效模式已落实（`:1680/1691/1707/1932`）；`rollback_before` 已不依赖当前 run_id（`:1103-1135`）；E2E 是**真**两个 run_id 且断言文件复原（`3332/3336/3340-3342`）。唯一残留是 `:1123` 的宽容过滤（低）。
- **第 3 条**：RootDir/CurDir 拒绝 ✓；恢复目标为链接时不穿透 ✓；`canonicalize` 校验父目录 ✓；测试真造了 `symlink_file`（`3511`），创建失败时 `eprintln`+跳过 ✓——但因此这条断言在本机很可能从未真正执行（题面允许跳过，属于接受的残留）。
- **第 9、10、11 条**：早退不覆盖旧快照 ✓（仅半满足「空不覆盖非空」）；回滚 `read_dir`、收集→执行→summary 永远返回、`.created` 删除不早于恢复 ✓；命令取项目写锁 ✓。
- **第 4 条**：运行目录 + 绝对路径 + `.gitignore` 兜底（推理成立）+ persist 降级为 warning ✓（契约例外未补）。
- **第 6、12 条**：取最后一个标题段 ✓、RFC3339 排序同刻 ok 优先 ✓（测试有判别力）。
- **第 8 条**：从 frontmatter 回填、camelCase ✓。
- **新缺陷/回归**：未见对第 2/3/4/5 期路径的语义回归（`run_one`/`run_fallback`/ACP 分支逐段读过：写锁、验证、worktree 免验、备选路由、`fallback_skipped` 都未动；`run_fallback` 对可写节点本就不跑，因此不会出现二次快照）；留痕契约只追加（`before_dir` 带 `#[serde(default)]`，`DispatchArtifacts` 仍 snake_case，`r3_1` 测试在）；`server.rs:3951-3954` 注册在位，R1 未新增命令（故 CRLF 不受影响）。

---

## R1 之后是否可交付？

**达到可交付（无阻塞缺陷，16 条全部落地且第 1/2/3/4/5 条的原始缺口已被真测试钉住），下列 5 项仍未闭合，按严重度排序：**

1. **（中低·安全）** `.created` 删除路径只查目标本身、不查父目录链（`dispatch.rs:1171-1179`）——在「after 快照之后、回滚之前」的窗口里父目录被换成 junction/symlink 时，`remove_file/remove_dir` 会删到项目外；同一处 `create_dir_all` 先于 canonicalize（`:1162-1165`）也允许在项目外建空目录（不写内容）。恢复路径已防、删除路径漏，属不对称加固。
2. **（低）** `latest_before_dir` 的 workdir 过滤在 canonicalize 失败时放行（`:1123`）→ 可能取到本项目之外的留痕快照（node_id 为 UUID，实际接近理论性；改成「必须等于 canonicalize(project)」是一行修）。
3. **（低）** 第 9 条的「不用空快照覆盖非空快照」只覆盖「未进通道」；进了通道但零改动的重派仍会覆盖（`:1932` vs `:1032-1035`）。
4. **（低·测试强度）** `process.rs:513` 的 8MB 标记测试**无同步点**：`recv_line` 在消费端做 `fetch_sub`（`process.rs:44`），子进程的第 2 行与首行换行在同一 flush 到达（Python 块缓冲），生产者的第二次预留与消费者的扣减赛跑——实践上生产者多数先跑完（所以套件绿），但是真实的 flake 隐患（确定性写法：spawn 后先等生产者把两行都处理完再 `recv`）；另有 `task06_acp_runner_note_...`（`:3424`）为 helper 自证、`task06_rollback_reads_..._total_limit`（`:3482`）不读任何上限（旧实现也会过）、幂等释放半边（`:3371`）没走 `dispatch_stage_core`、`3511` 符号链接用例在无权限时整条跳过。
5. **（低·口径/文档）** 契约 A.1 仍写「worktree 节点跳过」（降级路径行为已变，`TASK-06-consistency.md:17` 未补）；`before_dir` 的「相对项目目录」未写绝对路径例外；第 13 条未加代码注释而报告引用 `src/dispatch.rs:610`（实为 guard 代码）；回滚 note 不报 `skipped` 条数（`:1193`）；`write_project_dispatch=false` 下对账仍完全不可用（STATE.md/留痕都不在项目里）而回滚已支持该模式，口径不一致；`DispatchRecord` 旧字段无 `serde(default)` → 跨版本旧留痕退化为 `unknown`。