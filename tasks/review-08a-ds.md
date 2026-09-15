## 1. 需求完整性

**逐条对照 A 节（A1–A6）**

| 条 | 状态 | 依据 |
|---|---|---|
| A1 Node.swarm 镜像 + 留痕两字段 + 往返测试 | 完成 | `src/orchestration.rs:57-58,64-70`；`src/dispatch.rs:360-363`；测试 `dispatch.rs:3508`、`orchestration.rs:1183`。另加了 `swarm_limit`（`dispatch.rs:37-38,68`） |
| A2 check_swarm_plan | 完成 | `dispatch.rs:450-493`；测试 `dispatch.rs:3099` |
| A3 merge_swarm_items | 完成 | `dispatch.rs:616-792`；四条测试 `dispatch.rs:3121/3146/3162/3178` |
| A4 split.txt + 拆分角色默认 | 完成 | `fixtures/prompts/split.txt:1`；`orchestration.rs:491-493,499`；测试 `orchestration.rs:1196` |
| A5 server.rs 注册 | 完成 | `server.rs:3940-3941`（CRLF 保留，已验证行尾 `\r`） |
| **A6 内置预设 builtin-swarm** | **未做** | `orchestration.rs:304` 仍只有 5 条内置，`:884` 断言仍是 5，`:886-890` id 列表无 swarm；全仓 `builtin-swarm` 零命中 |

**F1.1（高）A6 整条缺失** — `src/orchestration.rs:304,884`
问题：任务书 A 节现在有 6 条，第 6 条（内置「蜂群模式」）没有实现，内置清单与断言都还是 5。
为什么：B 侧第 2 条要求界面出现「蜂群模式」预设并盖出一个 `拆分` 节点；预设清单由 Rust `list_presets` 提供，Rust 不产这条，B 侧无论怎么写都盖不出预设，蜂群模式的入口整条断掉。报告把它归为「用户限定 1→5」，但任务书文本已含 A6，交付口径必须回用户确认，不能由实现者默认放弃。
改法：补 `Preset { id:"builtin-swarm", name:"蜂群模式", builtin:true, stages:[Stage{ nodes:[Target{ role:"拆分", write:"只读", visibility:"共享", model:"" }] }] }`（目标留空由 `fill_role_defaults` 按 planning 填），同步 `:884` 5→6、`:886-890` 加 id 断言，并加测试断言「该预设只有一个节点、role=拆分、write/visibility 被 fill_role_defaults 强制」。

**F1.2（中）wire 形状在任务书里没写死，实现按 camelCase，B 侧踩空就整条命令失败** — `src/dispatch.rs:497-556,780-792`
问题：`SwarmMergeItem`/`SwarmPlanItem`/`SwarmMergeResult` 都带 `#[serde(rename_all = "camelCase")]`，wire 上是 `nodeId/worktree/testOutput`；任务书契约文字写的是 `node_id, item, worktree, accept`，且 `node_id`、`item` 没有 `#[serde(default)]`。
为什么：B 侧按任务书字面写 `node_id`，Tauri 反序列化直接 `missing field nodeId`，G1/G3 全哑，且报错落在 IPC 层不易定位；两个工作树各写各的，这种错只有联调才暴露。
改法：在 TASK-08 契约里补一句「wire 为 camelCase（与仓库既有命令一致）」，或给 `node_id`/`item` 加 `#[serde(default)]` 并把空 node_id 降级为可读错误。

**F1.3（中）swarm_limit 只有字段没有使用点** — `src/dispatch.rs:37-38,68`
问题：Rust 侧没有任何地方读 `swarm_limit`，合并也不校验件数上限。
为什么：契约「上限 4 件」目前完全靠前端截断；引擎侧没有第二道闸，超限清单照样按序全并（每件一次 cherry-pick + 一次验证），与「上限与并发上限同一处设置」的意图不符。
改法：`merge_swarm_items_core` 开头按上限截断并把超出件标 `skipped: 超过上限`；或明确在契约里写「上限只在前端」，别留模糊。

**F1.4（低）split.txt 里的「N」是字面量，没有渲染点** — `fixtures/prompts/split.txt:1`
为什么：模板现在只被 `orchestration.rs:1222` 的测试 include，真正注入在 B 侧（对照 `src-ui/src/graph/dispatch.ts:9-12` 的角色模板表）。A 节没要求接线，不算缺项，但契约应写明「N 由 B 侧按 swarm_limit 渲染」，否则 B 侧可能原样把 N 发给模型。
建议：契约补一句，或在 A 节注明模板由前端渲染。

**F1.5 其余契约项核对**：留痕字段只追加 ✓（`dispatch.rs:360-363`，append 在 `before_dir` 之后，`skip_serializing_if` 保证旧记录字节不变，测试 `dispatch.rs:3985-4007`）；拆分角色默认 ✓（`orchestration.rs:491-500`）；命令签名/返回形状与契约语义一致 ✓。

## 2. 逻辑正确性

**逐项核验**：glob 重叠（同一现存文件被两条 glob 匹配）✓（`dispatch.rs:459-483`，按 item 取并集后两两求交，取交集最小路径）；排除目录与快照同一函数 ✓（`excluded_snapshot_directory`，`dispatch.rs:366-389` 被 `existing_project_files:426-433` 复用）；项目干净前置 ✓；每件 commit sha ✓（`dispatch.rs:701-706`，取项目 HEAD）；冲突 abort 回到并入前 ✓（`dispatch.rs:726`，测试 `:3156` 断言 rev-list=2 且第一件内容仍在）；验证红 reset 只退本件 ✓（`dispatch.rs:712`，测试 `:3173`）；写锁 ✓（`dispatch.rs:624`）；后续件不停 ✓（循环无 break）。问题如下：

**F2.1（中高）留痕 `status` 被填成 merge_result 值，污染 TASK-06 对账** — `src/dispatch.rs:582`
问题：`append_swarm_record` 写 `status: result.result`（`merged|conflict|test_failed|skipped`），而 `reconcile_dispatch_state` 按 `record.run_id == state.run_id && record.node == node_id` 匹配（`dispatch.rs:906`）、按 `ended_at` 取最新（`dispatch.rs:873-878,908`）、`verdict = if record.status == "ok" { completed } else { incomplete }`（`dispatch.rs:918`）。
为什么：merge 留痕与执行节点同 run 同 node，且写入时间更晚 → 一旦该 node 之后又处于 STATE.md 的 active 集合（崩溃在重派途中、或合并后重派该件时进程崩了），对账会把 merge 留痕当成该节点最新留痕：`incomplete` + 空 `reply_text` + 空产物，前端 `applyReconcile` 直接把它打回 draft 重跑（`src-ui/src/graph/dispatch.ts:845-847`），绕过了「未知 → 待核对，由用户裁决」这条既定路径。即使不触发对账，`status` 被塞进 ok/failed 之外的枚举值，对任何按 status 判定的既有/后续逻辑都是隐性污染。
改法：`status` 写 `ok`（成功）或 `failed`（非 merged），`merged|conflict|test_failed|skipped` 只放 `merge_result`；对账侧额外过滤 `merge_result` 非空的记录（只追加、不破坏旧留痕）；补一条「merge 留痕不改变对账结论」的测试。

**F2.2（中）worktree 内已被 agent 提交的改动会被判「没有改动」→ 静默 skipped** — `src/dispatch.rs:668-672`
问题：只用 `git diff --cached --quiet` 判断有无改动；worktree 里若有 agent 自己产生的 commit，索引与 HEAD 一致 → `skipped: worktree 没有改动`。
为什么：执行提示词只禁 push/发布/装包/付费接口（`fixtures/prompts/execution.txt:2`），**没有禁 git commit**；claude/codex 在 worktree 里顺手 commit 是常见行为。这一件的工作永远不会并入，结果行还写着「没有改动」，用户不会察觉（`artifacts.diff` 里也看不到，因为 diff 是按前后快照算的）。
改法：判据改为「索引有改动 **或** worktree HEAD 不是项目 HEAD 的祖先」，后者直接 `cherry-pick <worktree HEAD>`；至少也要把 note 写成「worktree 无未提交改动，但 HEAD 已有 N 个提交，可能已被 agent 提交，请人工处理」。

**F2.3（中）验证超时硬编码 120s，忽略 `settings.timeout_secs`** — `src/dispatch.rs:699`
为什么：`run_one` 里验证走 `settings.timeout_secs`（默认 600、界面可调，`dispatch.rs:2202,2447`），蜂群合并却固定 120s；`cargo test` / `npm test` 超 120s 的件会被判「测试红」并 `reset --hard`，用户没有任何旋钮可用，且现象（测试红）指向错误的方向。
改法：timeout 通过入参/设置传入（契约签名要改就向用户报备），或至少对齐 `default_timeout()`。

**F2.4（中）worktree 路径来自前端且不做「是否本项目登记的 worktree」校验** — `src/dispatch.rs:647-656`
问题：仅 `PathBuf::from(&item.worktree)` + `is_dir()`，随后在它里面执行 `git add -A`、`git commit`。
为什么：契约明确「只在项目目录与登记的 worktree 内执行 git」；传入任意带 `.git` 的目录（甚至相对路径，按进程 CWD 解析）就会在别处建提交，改的是用户的另一个仓库。`is_dir()` 还跟随符号链接/junction。
改法：用 `git worktree list --porcelain` 校验路径在册，或至少要求它位于 run_root 之下并与项目 `.git/worktrees/*` 对应；用 `symlink_metadata` + `is_link_or_junction` 拒绝链接。

**F2.5（中）超时在 git 步骤丢失，note 不可读** — `src/dispatch.rs:405-412` 被 `dispatch.rs:451-...` 各分支调用
问题：`process_note` 只在 stderr/stdout 都为空时才用 fallback 文案，`result.timed_out` 没有分支处理，超时的 git 步骤会得到「暂存 worktree 改动失败」这类不含超时信息的 note（同文件已有 `nonempty_process_error` 会格式化超时）。
改法：`process_note` 先判 `result.timed_out`，或在 merge 各分支改用 `nonempty_process_error`。

**F2.6（中）拆分角色没进引擎的只读护栏** — `src/dispatch.rs:1038`
问题：`readonly_role = matches!(node.role.as_str(), "验收" | "验证" | "仲裁")`，没有「拆分」。
为什么：产品口径是拆分节点只读；现在只读只靠 `fill_role_defaults`（预设层）和 B 侧 `validateNode`。手改节点、旧图、或用户预设里 write=worktree 的拆分节点会真的建 worktree / 直接改项目——正好破坏「合并前项目必须干净」这条前提。
改法：把 `"拆分"` 加进该列表（与 R2-7 对仲裁的处理同口径），并在 B 侧 `validateNode` 同步。

**F2.7（低）非冲突型 cherry-pick 失败一律记 `conflict`** — `src/dispatch.rs:724-740`
为什么：坏 sha、索引被占、sequencer 残留都会落到 `conflict`，note 里虽有 git 原文，但结果枚举对 G3 的分区（冲突待裁决 vs 其它）是误导。
改法：按 stderr 关键字区分，或至少把 note 前缀写成「cherry-pick 未成功」并保留原文。

**F2.8（低）`test_failed` 没有 note** — `src/dispatch.rs:709-720`
问题：失败信息只在 `test_output` 里，`note` 为空；契约/任务书要求「每步失败都返回可读 note」，冲突与跳过都有 note。
另外 `merged` / `test_failed` 都不回传 worktree 路径（只有 conflict 的 note 里有），而两种情形 worktree 都留着。
改法：`test_failed` 的 note 写「验证命令非零退出（worktree：<path>）」，merged 也带上 worktree。

**F2.9（低）锁超时不是「整批 skipped」而是命令级 Err** — `src/dispatch.rs:624`
为什么：契约只定义了「项目脏 → 整批 skipped + note」，锁等不到（120s 内被别的派发占着）直接 `Err(String)`，B 侧要么当命令失败展示、要么没处理。既有 `acquire_project_write` 的取消/超时都是 Err 语义，这里按整批 skipped 返回更贴契约。
改法：把锁超时降级成 `skipped_swarm_results(items, "等待项目写锁超时…")` 并写留痕。

**F2.10（低）不校验重复 item / 件数与上限** — `src/dispatch.rs:622-646`
同号两件会各写一条同号留痕、各做一次 merge；上限见 F1.3。

**F2.11（低）合并后不回收 worktree，PRODUCT 里「回收已有」不成立** — 全仓无 `worktree remove/prune`
为什么：`create_worktree`（`dispatch.rs:1925`）有，回收路径一处都没有，合并成功/测试红/冲突后 worktree 都留在 run_root 下（每个都是完整工作副本）。PRODUCT「能借现成的就借：worktree 创建/回收」是错的，任务书据此判断工作量会偏。
改法：至少在报告/任务书里更正前提；合并成功的件建议 `git worktree remove --force`，冲突件保留。

## 3. 安全

正向确认：所有 git 与验证都过 `run_process_registered`（`dispatch.rs:411-420` `git_result`、`run_validation:1952-1975`）；无 push/fetch/远端写入；项目遍历用 `symlink_metadata` + `is_link_or_junction` 跳过符号链接/junction 与排除目录（`dispatch.rs:426-433`）；验证输出经 `safe_trace` 脱敏 + UTF-8 边界截 8KB（`dispatch.rs:541-549,704,713`）。

- **F3.1（中）= F2.4**：worktree 越界执行 git，是本期唯一实打实的安全缺口。补充一点：`error`/note 里的 git stderr 已过 `safe_trace`，但 `format!("worktree 不存在：{}", worktree.display())`（`dispatch.rs:655`）未过滤，只是路径，可接受。
- **F3.2（低）**：`check_swarm_plan` 未校验项目是不是 git 仓库/裸库，非 git 目录返回 Err（可读），不阻断界面，可接受。
- 其余未发现。

## 4. 代码质量与契约

- 只追加 ✓（新字段全在结构体末尾，`#[serde(default)]` + 序列化跳过，旧留痕字节不变）；旧图/旧留痕可读 ✓（`orchestration.rs:1191`、`dispatch.rs:3985-4007`）；`server.rs` 两处注册 ✓ 且 CRLF 保持 ✓；第 2–7 期路径未见编译层回归（`cargo check` 零 warning、289 测试通过，与报告一致）。
- **F4.1（中）`check_swarm_plan` 是同步命令，在主线程跑整棵项目树 + git 子进程** — `dispatch.rs:489-493`
  为什么：同文件重的命令都走了 `spawn_blocking`（`preview_dispatch:795`、`merge_swarm_items:786`）；这条是 G1 每次都要调的，大仓库遍历 + `git status`（最长 120s）会把 webview 事件循环卡住。
  改法：`pub async fn` + `tauri::async_runtime::spawn_blocking`。
- **F4.2（中）留痕固定写项目 `.dispatch`，忽略 `write_project_dispatch`** — `dispatch.rs:571-576`
  为什么：该设置（`dispatch.rs:49`）存在的意义就是「别往项目里写」；派发路径会按它选 `run_root/dispatch`（`dispatch.rs:1079-1083`），合并路径绕过。契约签名没带 settings，所以至少要在契约里写明「合并留痕始终写项目 .dispatch」，别让行为成为暗坑。
- **F4.3（中）glob 语义与「目录白名单是 glob 子集」不符，重叠检测会漏** — `dispatch.rs:459-463`
  问题：`require_literal_separator: true` 下 `src/*` 不匹配嵌套；`src`（纯目录）、`src/`、`./src/a.rs` 一律匹配不到任何**文件** → 进 `unmatched` 提示，重叠（这个检查的全部意义）不报。
  改法：匹配前归一化 pattern（去 `./`、尾 `/`、纯目录名展开成 `dir/**`），或在 `unmatched` 提示里明确写「目录请写 `dir/**`」。`case_sensitive: true` 在 Windows（git 默认 `core.ignorecase=true`）也会漏，建议 `case_sensitive: false`。
- **F4.4（低）非法 glob 直接 Err 掉整条命令** — `dispatch.rs:447-448`
  一个手输错的 pattern 会让 G1 整块检查失败，而不是给一条提示。建议并入 `unmatched`（或加 hint 字段）。
- **F4.5（低）提交信息用 node_id 顶替契约里的 `<title>`** — `dispatch.rs:675`（报告已说明：冻结入参没有 title）。建议给 `SwarmMergeItem` 追加可选 `title`（只追加），`aicross swarm item N: <title|node_id>`。
- **F4.6（低）`GraphNode.swarm` 未加 `skip_serializing_if`** — `orchestration.rs:57-58`
  保存旧图会多写 `"swarm": null`；与本文件其它 `Option` 字段（`Reply` 各字段）一致，不算回归，但 B 侧用 `swarm !== undefined` 判空会在 `null` 上判错，B 侧要用真值判断。

## 5. 测试

**真实性与覆盖**：4 条合并测试 + 1 条检查测试都用临时仓库真跑 git——`swarm_repository`（`dispatch.rs:3070-3087`）真 `git init`、配 user、造 base commit、两个真 `git worktree add`；断言的是目标行为而非实现细节：
- 顺序合并各一 commit ✓（入参乱序 → 结果 `[1,2]`，`rev-list --count=3`，`log -2` 首行含 `item 2`，`dispatch.rs:3131-3133`）
- 第二件冲突、第一件仍在 ✓（内容仍是 `first`、`rev-list=2`、`CHERRY_PICK_HEAD` 已清、`:3154-3157`）
- 验证失败只退本件 ✓（`one.txt` 在、`two.txt` 不在、`rev-list=2`，并顺带覆盖了「accept 优先、空 accept 回退 validate_command」，`:3171-3173`）
- 脏树全 skipped ✓（含 note 文案、rev-list=1、每件一条留痕，`:3185-3189`）
另有 `Node.swarm` 往返、留痕字段往返、camelCase 形状、server 注册的断言（`orchestration.rs:1183/1196`、`dispatch.rs:3508/3194/3205`）。报告中的 289 passed / 11 ignored 与测试清单自洽（300 = 289+11）。

**缺口（按优先级，最该补的 3 个）**
1. **「worktree 无改动 → skipped」与「worktree 路径不存在 → skipped」零覆盖**（契约明列的第四种结果）。这一条路径上正挂着 F2.2 的真实 bug（agent 自己 commit → 误判 skipped）；再加一条「worktree 未列入本项目 worktree list → 拒绝」顺带把 F2.4 钉住。
2. **失败件在中间时后续件继续**：现有四条里失败/冲突件都是最后一件，「单件失败不停整批」（产品已定 ③）没有任何断言。补 3 件场景，第 2 件验证红，断言第 3 件仍 `merged`、commit 数与文件内容对，且第 3 件的 reset 不会误退第 1 件。
3. **merge 留痕与 TASK-06 对账不打架**：写一条 `merged` 留痕后调 `reconcile_dispatch_state`，断言同 run/node 仍判 `completed`（或断言对账忽略 `merge_result` 非空的记录）——现在这条会直接红，正是 F2.1。
   次要：全文件对「缺外部二进制」的惯例是 `resolve_binary("python")` 后 eprintln 跳过（`dispatch.rs:3681/4352`），这 5 条测试没有 git 存在性保护，无 git 环境下会 panic 而非跳过；`test_failed` 的 `test_output`/`note` 内容未断言；`accept` 与 `validate_command` 都为空时「不跑验证」这条未单独断言（行为正确但无测试）。

---

## 对照 A 节是否完整实现

**没有完整实现：A1–A5 与契约主体达标，A6（内置 `builtin-swarm` 预设）整条缺失，另有若干契约/护栏遗漏。**

缺项（按严重度）：
1. A6 内置预设 `builtin-swarm`（含 5→6 断言同步）未做 —— 蜂群模式在界面上没有可选预设。
2. 合并留痕 `status` 写成 `merge_result` 值，与 TASK-06 对账的 `status == "ok" → completed` 判定冲突（`dispatch.rs:582` vs `dispatch.rs:918`）。
3. `merge_swarm_items` 不校验 worktree 是否属于本项目登记的 worktree（可对任意外部仓库建提交）。
4. 拆分角色未进引擎只读护栏（`dispatch.rs:1038` 只列验收/验证/仲裁）。
5. worktree 内已被 agent 提交的件被判「没有改动」→ 静默 skipped（`dispatch.rs:668-672`）。
6. 验证超时硬编码 120s，忽略 `settings.timeout_secs`（`dispatch.rs:699`）。
7. `swarm_limit` 在 Rust 侧无使用点（上限只靠前端）。
8. 契约未写明 wire 是 camelCase，`node_id`/`item` 无 default，B 侧按字面写会整条失败。
9. 留痕落盘忽略 `write_project_dispatch`；`test_failed` 无 note；提交信息无 `<title>`；glob 目录条目/`./` 前缀不归一化；`check_swarm_plan` 同步跑在主线程。
10. 测试缺「worktree 无改动/不存在」「失败件在中间仍继续」「merge 留痕不影响对账」三类，且无 git 存在性跳过保护。