# TASK-06 A 节代码审查（phase/6a）

说明：本会话无 shell 工具，`cargo test`/`vitest` 无法复跑，以下验证均为静态读码；报告所列 12 个测试全部在源码中找到（行号见维度 5）。

## 1. 需求完整性

- **契约形状** — 未发现。`ReconcileSummary`/`ReconcileNode`/`RollbackSummary`（`src/dispatch.rs:239-265`）字段与 camelCase 与契约一致；`rollback_before`（`src/dispatch.rs:113`）、`before_dir`（`src/dispatch.rs:329`）、`Reply.fallback_from/to`（`src/orchestration.rs:101-104`）均带 `#[serde(default)]`，只追加；旧留痕/旧图可解析。
- **A6 标记行** — 未发现。`run_fallback` 只填 `fallback_from/to`（`src/dispatch.rs:1558-1559`），不再改 `text`；测试断言 `!result.text.starts_with("（已切备选：")`（`src/dispatch.rs:3569`）。
- **A1 快照目录位置与 `write_project_dispatch=false` 冲突** — `src/dispatch.rs:1030`：`persist_before_snapshot` 无条件写进 `project/.dispatch/`，即使 `settings.write_project_dispatch == false`（该模式下留痕已改写到运行目录 `src/dispatch.rs:595-599`）。为什么：关掉项目内写留痕的用户（可能项目目录只读、或不想要任何额外文件）会被悄悄写入 `.dispatch/<run>/<node>.before/`，且 `ensure_log_dir` 的 `.gitignore` 兜底只写在 `log_dir`（`src/dispatch.rs:2186-2193`），这些快照文件不进 ignore，可能被用户误提交；同时 `persist` 失败会把一次已成功的派发改判为 error（`src/dispatch.rs:1854-1857`），是新增的失败源。建议：`write_project_dispatch=false` 时把 `.before` 落到运行目录并在 `before_dir` 记绝对路径，或至少同步写 `.gitignore`、persist 失败降级为 `runner_note` 警告而不是翻转为 error。

## 2. 逻辑正确性

- **快照/回滚主路径** — 未发现。只存 changed/deleted（`changed_snapshot_paths` + before bytes，`src/dispatch.rs:1037-1051`）、新建记 `.created`（1054）、超限进 `.skipped`（1042-1044、1057）；回滚在写锁内（rollback 在 `acquire_project_write` 之后，`src/dispatch.rs:1604-1619`）、恢复内容并删新建文件、`.created`/`.skipped` 复原逻辑正确；E2E 测试里 stub 进程亲自断言了「回滚已发生才继续跑」，这是真断言。
- **worktree 退回单写者时不落快照、不回滚（最重）** — `src/dispatch.rs:1585-1599` vs `1612`、`1831`：`create_worktree` 失败时 `uses_single_writer = true`、`workdir` 切到项目目录，写锁也按单写者拿（1604）；但 rollback 条件是 `node.write == "单写者"`、persist 条件也是 `node.write == "单写者"`，都是原始字段而非生效模式。为什么：非 git 项目里 `worktree_falls_back`（`src/dispatch.rs:614`）使所有 worktree 节点必然走此路径——而恰恰在无 git 的项目里，回滚是唯一的恢复手段，A 节要解决的「重派叠加」在这些节点上原样存在。建议：1612 与 1831 的条件改用 `uses_single_writer`（该变量在两处之前已定值）。
- **rollback_before 依赖「重派复用原 run_id」这一未写进契约的前提** — `src/dispatch.rs:1613`：引擎用当前请求的 `run_id` 拼 `.dispatch/<run_id>/<node_id>.before/`。为什么：快照是上次派发按当时的 run_id 落的；前端重派若生成新 runId，回滚静默 no-op，且 `runner_note` 记的是「没有派发前快照，无需回滚」——把「找不到」说成「无需」，G1 的「将先回滚上次改动」提示就变成谎报。E2E 测试（`src/dispatch.rs:3215,3221`）两次都用 `"run"`，钉不住这个前提。建议：在 TASK-06 契约里明文「rollback_before 必须复用原 run_id」并同步 B 侧；或引擎在 `rollback_before && !快照目录存在` 时在 `runner_note` 里区分「首次派发」与「run_id 不匹配，找不到快照」。
- **对账** — 未发现。按 `run_id + node_id` 匹配（`src/dispatch.rs:427`），同节点多条留痕按 `(ended_at, 是否备, 路径)` 确定取最新（428-432），三种 verdict 判定正确（440、446）；`reply_text`/`artifacts` 的切分与 `write_record` 的落盘格式逐字对上（389-397 vs 2227）；对账不再做任何脱敏/过滤，不会二次损坏。两个边界，低severity：正文里若嵌有「## 产物 JSON」等标题（上游回复经 prompt 回灌时可能），`split_once` 取首个会错位（389-395）；无「## 产物 JSON」段的更老留痕会把整段回复判空（391 的 `unwrap_or_default`）。建议：按行扫描取「## 回复正文」到下一个 `## ` 标题为止，天然兼容旧格式。
- **同 run 重复派发被拒** — 未发现。`src/dispatch.rs:585-591`，RAII 覆盖全部后续 `?` 退出路径；`run_id` 为空时先发 uuid，不会自拒。
- **8MB 标记必达** — 实现正确：`src/process.rs:13-15` 预留 `MAX + marker_bytes`，普通行仍受 `MAX` 约束（143），标记只在队列 ≥ MAX 时才需要超额额度，构造上必达。但见维度 5：测试没压到边界。

## 3. 安全

- **`safe_relative_path` 放行 rooted 路径** — `src/dispatch.rs:1016-1021`：只拒绝 `is_absolute()`、`Prefix`、`ParentDir`，Windows 上 `Path::new("/x").is_absolute()` 为 false，`RootDir` 分量不在拒绝列表；`project.join("/x")` 会整体替换为 `/x`（当前盘根）。为什么可达：`.dispatch` 不在快照白名单里，但单写者节点对项目目录有全权写——模型可以在派发期间改写 `.created`/`.skipped`（回滚在 `src/dispatch.rs:1074-1075` 原样读入），或把 `.before` 里的普通文件换成链接。建议：把 `Component::RootDir`（顺带 `CurDir`）加进拒绝列表，一行改动。
- **回滚恢复会穿透派发后新出现的符号链接/junction** — `src/dispatch.rs:1092`：`fs::write(&target, bytes)` 跟随符号链接。场景：before 快照里是真实文件 `a.txt`，模型派发期间删掉并换成指向项目外的 junction/symlink，after 快照里该路径消失（快照 walk 跳链接，926-927）被判为「已删除」，回滚时 `fs::write` 穿透链接写到项目外。persist 侧（1051）源是已vetted 的快照键，风险低；restore 侧直接面向磁盘现状。建议：写前 `fs::symlink_metadata(&target)`，是链接就删除链接（或进 `skipped`）；`create_dir_all(parent)`（1089-1091）后可用 `canonicalize(parent)` 前缀校验仍落在 canonicalize(project) 之内。
- **对账只读** — 未发现。`reconcile_dispatch_state` 全程只读（401-450），`inspect_dispatch_state` 同。
- **禁外部动作的实际效力** — 提示词已生效：`fixtures/prompts/execution.txt:2`，前端按 `?raw` 注入执行角色（`src-ui/src/graph/dispatch.ts:7,360`）；claude `--tools` 白名单 `Read,Grep,Glob,Write,Edit,Bash`（`src/channels/mod.rs:176`）确实排除 WebFetch/WebSearch。但要如实说：Bash 在白名单里，`git push`/`npm install`/`curl` 对 claude 写节点仍然技术可行，禁外部动作对 claude/kimi 只有提示词一层，codex 才有 sandbox 禁网——这与 PRODUCT 09-14 已定口径一致，属设计内残留，建议在界面隔离标签上体现「部分」。

## 4. 代码质量与契约

- **runner_note 回滚说明会被覆盖** — `src/dispatch.rs:1669` 初始化为回滚 note，但 1684、1707-1711、1726（ACP 成功/重置路径）整体覆盖 `runner_note`。为什么：`acp_enabled` + 可写节点是合法组合，此时「已回滚上次改动 N 文件」从结果里消失，前端 G1 的回滚提示与实际不符。建议：覆盖处改为追加（现有 `output.runner_note` 已用 `push_str("；")` 的模式，1712-1715）。
- **留痕只追加** — 未发现；`before_dir` 参与了 `frontmatter_roundtrips`（`src/dispatch.rs:2684-2724`）。
- **server.rs 注册** — 未发现（`src/server.rs:3952-3953`）。
- **旧路径回归** — 未发现。调度/写锁/取消、`run_validation` 与 worktree 免验证（1813-1825）、ACP 分支（1681-1745）、cc-switch、preview 均未被本期改动触及语义；`DispatchArtifacts` 维持 snake_case 与前端 `ReplyArtifacts`（`src-ui/src/graph/types.ts:38`、测试 `r3_1_artifacts_serialize_with_snake_case_keys`）一致，`ReconcileSummary` 外层 camelCase 与内层 artifacts snake_case 的组合与既有结果路径同构。

## 5. 测试

报告 12 个测试全部实存且多数断言到位：`task06_reconcile_...`（dispatch.rs:3145，含 run_id 过滤）、`task06_single_writer_rollback_...`（3172）、`task06_rollback_before_runs_inside_run_one_...`（3193，stub 内嵌前置条件断言，最硬的一条）、`task06_rollback_reports_over_eight_megabyte_...`（3230）、`task06_duplicate_active_run_...`（3242）、`task06_execution_prompt_...`（2487）、`task06_dispatch_request_reads_rollback_before_camel_case`（2479）、`frontmatter_roundtrips`（2684）、`writable_claude_excludes_web_tools_from_allowlist`（channels/mod.rs:970）、`task06_reply_fallback_fields_...`（orchestration.rs:1158）、`task06_fallback_reruns_once_...`（3540）、`interactive_stdout_truncation_marker_...`（process.rs:513）。弱点：process.rs:513 是同义反复（断言 helper 等于它自己的定义）；3193 依赖 PATH 有 python，无 python 时静默通过（report 的 239 passed 里可能含此类空转）；3242 直接改全局注册表，没覆盖 guard 的释放半边。

最该补的 3 个：

1. **worktree 建失败退回单写者**的 `run_one` E2E（对应维度 2 第一条）——目前没有任何测试走过 `uses_single_writer=true` 且 `node.write=="worktree"` 的组合。
2. **rollback_before 换 run_id 派发**——钉住「必须复用原 run_id」契约，或钉住区分性 note；否则 B 侧接上后这类 bug 只会在真机上现形。
3. **回滚对抗**：`.created` 写入 `/x`（RootDir）与恢复目标为符号链接两个 case，断言不写出项目目录、进 `skipped`。（备选：把 process.rs 的预留逻辑抽成纯函数后做队列压力模拟，替换同义反复测试。）

---

## 对照 A 节是否完整实现

**一句话：6 条全部落地、契约逐项对上且只追加，但第 1 条（回滚）在两个真实场景下会失效，不补则本期目标「重派不叠加」并未完全达成。**

缺项（按严重度）：

1. worktree 建失败退回单写者的节点：不落 before 快照、不执行 rollback_before（`src/dispatch.rs:1612,1831` 用 `node.write` 而非生效模式 `uses_single_writer`）——非 git 项目里所有 worktree 节点都走此路径。
2. rollback_before 用当前请求 run_id 定位快照，重派换 run_id 即静默 no-op 且 note 谎称「无需回滚」；契约未写明须复用原 run_id。
3. 回滚路径两个越界口子：`safe_relative_path` 放行 rooted 路径（`/x`）、`fs::write` 穿透派发后新出现的符号链接/junction。
4. `write_project_dispatch=false` 时 before 快照仍写进项目 `.dispatch` 且无 `.gitignore` 兜底；persist 失败会把已成功的派发翻转为 error。
5. 次要：回滚 runner_note 在 ACP 路径被覆盖；对账对含内嵌标题/无产物段的旧留痕解析会错位或判空；8MB 边界测试为同义反复。