# 复审结论

**总判：可交付（工作区提示）**——11 条全部在代码与测试层面落实，其中 1 处真实缺陷（恢复后复核把排除目录残留误判为恢复失败）在本复审进行期间由并行提交 `763b21b`（"post-restore recheck ignores junk-dir residue"）修复并有测试钉住；剩余均为窄窗口边界项，不动摇可交付性。

**复审对象说明（事实）**：审查开始时分支 tip 为 `feb54ce`（工作区 clean）；审查中途 tip 前移到 `763b21b`（reflog：`D:\work\aicross\.git\logs\refs\heads\phase\8d-r1` 第 4 行，提交时间晚 feb54ce 约 7 分钟）。以下引用行号均为当前工作区（= 763b21b 后）状态。本环境无 shell 工具，`cargo test`/`git diff` 未能独立执行，测试通过与否引自 REPORT-08D-R1.md（336 passed / 0 failed）——该报告只写到 feb54ce，未含 `task08d_r1_validation_red_with_cache_residue_stays_test_failed`，已落后 tip 一个提交。

## 11 条逐条核对

| # | 判定 | 依据 |
|---|------|------|
| 1 未跟踪目录折叠 + 暂存后二次核验 | **已修** | 全部状态读取带 `--untracked-files=all`：worktree 改动 `src/dispatch.rs:848`、项目入口/逐件经 `project_dirty_paths:973`、恢复/复核 `:521/:536/:940`。暂存后核验 `audit_staged_swarm_paths:983-1015`：`diff --cached --name-only --no-renames -z <base>`（:989）逐条判排除/范围外（:998-1000），越界者 `git reset -q <base> -- <rejected>`（:1007）移出 index——只 reset rejected，范围内路径不进该分支。测试 `:4061`（文件级声明 + 未跟踪新目录 → merged）、`:4075`（目录级声明 + 内嵌 `__pycache__` → pyc 不进项目、note 提及）、`:4040`（范围外/排除双断言，内容逐字节比对） |
| 2 非零 apply 判据 | **已修** | `:1289-1297`：apply 非零先 `unmerged_swarm_paths`（`ls-files -u -z`，:1045-1056）；空 → `skipped` 带 git 原文与 worktree 路径；非空 → `conflict` 且列表只含真冲突。ls-files 失败/截断按空处理，方向保守（降为 skipped）。测试 `:4092`（二进制文件被同名目录替换 → skipped、note 无「文件冲突」、`ls-files -u` 空） |
| 3 幂等 | **已修** | `:1305-1311`：apply 成功后 `diff --cached --quiet` 为 0 → `skipped：范围内改动已在项目中`，不再落 commit 失败分支。测试 `:4113`（同一件连跑两次，第二次 skipped + 该 note，rev-list 仍为 2） |
| 4 定向恢复 + 残留清理 | **已修**（有窄窗口边界，见新发现 B/C） | 四条路径全覆盖：apply 失败 `:1298`、超时 `:1283`、commit 失败 `:1319/:1328`、验证红 `:1363`（rollback_paths = staged ∪ 验证后脏路径，:1352-1357）。恢复 = `reset --soft <baseline>`（:468）+ `restore_swarm_paths:489-551`（`reset -q HEAD -- <paths>` :498 → 仅 tracked 逐条 `checkout --` :505-519 → 删「untracked 且 ∈ 本件路径集」:526-535 → 全量复核 :536-549）。全文件已无 `reset --hard`/`checkout -f`/整树 `.`（grep 零命中）。误删依据：入口检查 `:1088` + 逐件 apply 前检查 `:1249` 保证 apply 前项目无非排除未跟踪文件，故恢复时 selected 内的 untracked 只能来自本补丁。测试 `:4128`（新建文件+冲突同补丁、`-uall` porcelain 空、item1 merged/item2 conflict/item3 merged、下一批 item4 再 merged）、`:3970`、`:4302`、`:3923`、`:3987` |
| 5 rename 解析 | **已修** | `parse_git_status_paths:814-834`：X/Y 两列都判 R/C（:827），消费紧随字段（:828-830），取当前路径（:820，porcelain v1 -z 为 `XY 新\0旧\0`）。测试 `:4158`（`git mv` 后只出 `new.txt`，无 `/a.rs` 型垃圾） |
| 6 截断/差集/note 上限 | **已修** | truncated 检查：`swarm_changed_paths:845-846/:860-862`、audit `:993-995`、`project_dirty_paths:977-979`、恢复复核 `:523/:538`、验证后 `:945-947`；被截断 → skipped/停批。范围外 HashSet 差集 `:1183-1187`；`summarize_paths:926-933` 前 20 条 + 「等 N 项」。测试 `:4171`（注入 truncated → skipped + 「截断」）、`:4184`（含「等 25 项」、不含 outside-24） |
| 7 补丁超 4 MiB | **已修** | `write_swarm_patch:1017-1043`：`%TEMP%\aicross-swarm-<uuid>.patch`（:1023），`--output=<路径>` 单一 argv（:1024-1026）；apply 以文件参数读取 `:1272-1273`，不经 stdout/stdin（`git_result_with_input` 已零命中）。清理覆盖：生成失败三分支（:1030/:1034/:1038）、空补丁 :1235-1240、逐件脏检查中止 :1251/:1261、apply 后无条件删除 :1274——所有分支收敛。Windows 安全：argv 直传无 shell，空格/反斜杠无碍；UUID 防碰撞。测试 `:4201`（真生成 5 MiB 伪随机二进制，逐字节相等；旧 stdout 路径 4 MiB 截断下必红） |
| 8 非 UTF-8 保真 | **已修** | 与第 7 条共文件通道，无 lossy 解码环节。测试 `:4219`（GBK 字节 d6 d0 ce c4 并入后逐字节相等；旧路径 lossy → U+FFFD 必红） |
| 9 逐件脏检查 | **已修** | `:1249-1269`：补丁生成后、apply 前重跑全量 porcelain；排除目录外非空 → 本件 skipped + 余下全部 skipped（halt `:1264-1266`），尚未 apply 故无回退；状态读取失败/截断同样停批（:1244-1256）。测试 `:4233`（注入 mid-batch 用户编辑：item2 skipped、`shared.txt` 用户内容原样保留、item1 merged；旧实现下 item2 会 merged，可红） |
| 10 note 分级 + 目录口径 | **已修** | 分级：`glob 无效：<pattern>` :1177（`invalid_swarm_globs:899`）、`声明范围未匹配到改动` :1196（有改动但 selected 空）vs `范围内没有改动` :1194/:1224/:1238。目录判定：合并侧 `swarm_path_matches:873-885` 裸名同时匹配自身与后代（:883-885），不调 `is_dir()`；`normalized_swarm_glob`（含 is_dir）只剩 plan 检查在用（:620），合并路径不再引用。测试 `:4251` |
| 11 裸目录删除 + 非法 glob | **已修** | 同上机制：声明 `olddir` 命中删除路径 `olddir/a.rs`（:883-885）；非法 glob 追加独立 note（:1177）。测试 `:4267`（worktree 删光 olddir → merged、项目 olddir 消失）、`:4261` |

**测试补课**：wire 默认值测试已扩为真实合并并断言 `skipped：未声明文件范围` + 文件未并入（:4513-4528）；冲突测试补 note 断言（冲突文件名 + worktree 路径，:3914）；`:3917/:4147/:4297/:4317` 状态断言均已改 `--untracked-files=all` 全量。既有 08A/08D 测试只增强未削弱（timeout :3970、恢复失败 :3987、reset 失败 :4427、连并 :3878、留痕/对账 :4479、wire 序列化 :4497 均保留）。

## 新发现的问题（按严重度，均为当前 tip 状态下的剩余项）

1. **[低中] 验证窗口内用户文件的静默清除** — `src/dispatch.rs:1352-1357` + `restore_validation_exclusions:944-950`。回退路径集把「验证结束后所有非排除脏路径」并入 rollback_paths，无法区分验证产物与用户在验证运行期间（可达 timeout 分钟级）写入项目的文件：用户的未跟踪非排除文件会被 `:526-535` 定点删除（验证绿但 dirty 非空时同样触发，:1339 vs :1345），用户的已跟踪文件若在 rollback 集内会被 `:512-519` checkout 覆盖；note 只写「验证命令改动了项目文件」，不提删除。复现：item 提交后、验证运行中，用户往项目根写 `notes.txt`；验证 exit 9 → `test_failed` + 回退 → `notes.txt` 消失。建议：回退前对 rollback 集里的 untracked 路径先比对 apply 前状态快照，非补丁来源的移入临时隔离目录而非直接删；至少在 note 明列被删路径。
2. **[低] unmerged 查询失败静默降级** — `unmerged_swarm_paths:1045-1049` 对 ls-files 出错/截断一律返回空 → 真冲突可能被降级为 `skipped`（方向安全，但丢失冲突文件清单，与第 2 条「只列真冲突」的语义在异常分支上不完整）。建议区分「无 unmerged」与「无法判定」。
3. **[低] 补丁新建目录留下空壳** — 残留清理只删文件/符号链接（:530-532），补丁新建文件所在目录清空后空目录留存；`git status` 不可见故复核通过。纯卫生问题。建议对清空后的父目录做一次 `remove_dir` 尝试。
4. **[信息] 入口脏检查不过滤排除目录** — `:1088-1102` 对任何 untracked（含未跟踪 `__pycache__` 残留）整批拒绝，与逐件检查（:1259 过滤）口径不一。系 08D 既有「入口脏树整批 skipped」语义（fix 清单「保持」项），非本轮回归，仅提示与 mid-batch 口径的差。
5. **[信息] `write_swarm_patch` 的 truncated 检查（:1037）在 `--output` 下不可达**（stdout 恒空）——死代码级防御，无害。

## 核过没问题的点

- **四条失效路径的恢复调用链**：:1283/:1298/:1319/:1328/:1363 全部走定向 restore；恢复失败一律 `halt_after_item` + 人工指引 + 余件 skipped（:1399-1412），不吞错；`baseline` 取不到时 `reset_swarm_item:461-463` 拒绝回退（fail-closed）。
- **误删防线**：untracked 删除条件「∈ 本件 rollback 集 && untracked && 是文件/符号链接」，目录跳过；`project.join` 的路径全部来自 git 输出的仓库相对路径，无穿越面。
- **补丁文件生命周期**：所有 early-return 分支与 apply 后均删除，无泄漏路径（第 7 条已列）。
- **763b21b 修复正确性**：恢复后复核改为过滤排除目录（:541-546），与逐件脏检查同口径；`:4322` 测试用真实验证命令造 `__pycache__` 残留 + exit 9，断言 `test_failed`/后件 `merged`/无「手动恢复」——旧代码（feb54ce 的无条件复核）下该测试必红。
- **契约未变**：`SwarmMergeItem/SwarmMergeResult`（:705-726，camelCase）与 `merge_swarm_items` 命令签名（:1424-1430）未动；留痕 `aicross-dispatch/1` 的 status ok/error 与 merge_result 映射未动（:757-767）；wire 序列化测试（:4497）与 server 注册测试（:4531）仍在。
- **测试非同义反复**：16 条 R1 新测试全部真建 repo + worktree 跑 git，断言均指向「旧实现必失败」的可区分行为（逐条推演见上表）；注入钩子全部 `#[cfg(test)]` thread_local、单次消费，不污染并行用例。
- **既有语义保持**：验证超时仍归 `test_failed`（`run_validation:2615-2619` + `:1346-1349`）；批初 project_head + 每件 merge-base（:1158）、`add -A -- selected` → `diff --binary --full-index --cached` 补丁来源（:1199/:1227）、排除清单复用快照清单（`excluded_swarm_path:808-812` → `excluded_snapshot_directory:366-388`）均未动，与 fix 清单「保持」节一致。

**留给合并者的两个动作**：(1) `tasks/REPORT-08D-R1.md` 补记 763b21b（filter + cache-residue 测试），否则报告与 tip 不一致；(2) 在有 shell 的环境跑一次 `cargo test` 确认 763b21b 后仍 0 failed（本复审未能独立执行）。