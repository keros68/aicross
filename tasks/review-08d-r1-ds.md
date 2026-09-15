## 总判

**可交付但有未闭合项** —— fix-08d-r1.md 的 11 条全部在 `src/dispatch.rs` 落地且有对应真实 git 测试（我逐条读了实现与测试）；但第 4 条引入一处新的行为回归：恢复后的“全量复核”把**排除目录内的残留**（验证命令生成的 `__pycache__` 等）也算作“恢复失败”，会把 `test_failed` 降级成 `skipped` 并整批 halt。另有若干低危残留。说明：本环境没有 shell 工具，我无法跑 `cargo check/test`，编译与测试结果只能引用 REPORT-08D-R1，测试“能否红”是按上一轮盲审描述的旧实现静态推演。

## 11 条逐条

| # | 结论 | 依据（文件:行） |
|---|---|---|
| 1 | 已修 | 5 处 status 全带 `--untracked-files=all`：`dispatch.rs:843`（worktree）、`:968`（`project_dirty_paths`，入口 `:1083` 与逐件 `:1244` 共用）、`:935`（验证后）、`:521`/`:536`（恢复）。二次核验 `audit_staged_swarm_paths :978-1010`：`git diff --cached --name-only --no-renames -z <base>` 逐条判 `excluded_swarm_path`+`swarm_path_matches`，越界用 `git reset -q <base> -- <rejected>`（`:1002-1008`）移出 index——只动 index，不删 worktree 文件；rejected/allowed 由 `:992-998` 的 if/else 保证互斥，selected 内文件不受影响。测试：`:4056`（文件级声明+未跟踪新目录）、`:4070`（目录级声明+内嵌 `__pycache__`，断言 pyc 不进项目）、既有 `:4035`。两测试在旧实现下都是红的（旧代码 status 折叠成 `?? src/feature/`）。 |
| 2 | 已修 | `git apply` 非零 → `unmerged_swarm_paths`（`:1040-1051`，`ls-files -u -z`）判据，空则 `skipped` + git 原文，非空才 `conflict` 且只列真冲突文件（`:1285-1293`）。测试 `:4087`：真 repo 里把已跟踪二进制换成同名目录，断言 `skipped`＋note 不含“文件冲突”＋`ls-files -u` 空；旧代码必红。 |
| 3 | 已修 | apply 成功后 `git diff --cached --quiet`（`:1301-1306`），无差异 → `skipped：范围内改动已在项目中` 并 `break`，不进 commit 分支。测试 `:4108`：同一件连并两次，第二次 skipped＋note 命中＋`rev-list --count` 仍为 2。 |
| 4 | 已修（但引出新回归，见新发现 1） | 四条路径全覆盖：timeout（`:1277-1284`）、apply 非零（`:1285-1300`）、commit 失败（`:1313-1332`）、验证红（`:1338-1362`）。恢复全部定向：`reset --soft <baseline>`（`:454-471`）+ `reset -q HEAD -- <selected>`（`:498-503`）+ 逐条 `ls-files --error-unmatch` → `checkout -- <tracked>`（`:505-518`），无 `.`/`--hard`/`-f`（全文件 grep 无 `reset", "--hard`、无 `checkout", "-f`、无 `untracked-files=no`）。残留清理 `:521-535`：只删 untracked 且路径**精确等于** selected 的文件，`symlink_metadata`＋`is_file||is_symlink` 防目录/链接误删。安全依据链：入口全量 status 必须为空（`:1083-1092`）→ 项目本无任何 untracked；逐件 apply 前再查一次（排除目录外必须为空，`:1244-1262`）→ 清理前不存在用户未跟踪文件；`selected` 恒不含排除目录分量（`:1176-1178` + `excluded_swarm_path :803-807`）→ 排除目录里的用户产物不可能被删。因此“只删本件补丁新增”的依据是“干净前置条件＋路径范围”，不是解析补丁的 `new file` 段。测试：`:4123`（新建文件+冲突同补丁→`new.txt` 被清、porcelain 全量空、item1 merged/item2 conflict/item3 merged、下一批 item4 仍 merged）。 |
| 5 | 已修（覆盖有缺口） | `parse_git_status_paths :809-829` 两列都判 R/C、消费 `-z` 后随 origin 字段、取当前路径，与 `src/git.rs:324-341` 同口径。测试 `:4153` 真跑 `git mv` 断言取到 `new.txt` 且无 `/a.rs` 类垃圾；旧实现（取原路径）会红。缺口：只覆盖 index 列 `R `，未覆盖 DS 指出的工作区列 ` R`（代码已处理，测试未钉）。 |
| 6 | 已修（一处同类遗漏） | `truncated` 在 `:840`、`:855`、`:940`、`:972`、`:988`、`:1032`、`:1042`、`:523`、`:538` 全部消费，截断即该件 skipped / 停批；`outside` 用 `HashSet` 差集（`:1175-1183`）；`summarize_paths :921-927` 前 20 + “等 N 项”。测试 `:4166`（注入 truncated→skipped）、`:4179`（25 条 outside→“等 25 项”且不列第 25 条）。遗漏见新发现 4（`:1354` 验证脏路径未截断）。 |
| 7 | 已修 | 补丁改文件通道：`write_swarm_patch :1012-1038` 用 `git diff --binary --full-index --cached <base> --output=<tmp> -- <staged>`，项目侧 `git apply --3way --index <file>`（`:1268`），补丁不经 stdout（dispatch.rs 已无 stdin/`with_input` 调用）。清理覆盖全部早退路径：`:1028`/`:1031`/`:1034`（生成失败/截断）、`:1231`（空补丁）、`:1246`（状态不可确认）、`:1256`（批中脏）、`:1269`（apply 调用后立即删）。临时文件在 `std::env::temp_dir()`＋UUID（`:1018`），经 argv 直传、无 shell，Windows 含空格路径也安全。测试 `:4196`：真造 5 MiB 不可压缩二进制并逐字节核对——旧 stdout 路径下 4 MiB 截断必红。 |
| 8 | 已修 | 同第 7 条文件通道，补丁字节级落盘/读盘，不再经 `from_utf8_lossy`。测试 `:4214`：GBK 字节（`d6 d0 ce c4 0a`）并入前后逐字节相等；旧实现乱码必红。 |
| 9 | 已修 | 每件 apply 前重跑 `project_dirty_paths`（`:1244`）并按 `!excluded_swarm_path` 过滤（`:1254`），非空 → 本件 skipped + 余件全部 skipped + `halt_note`（`:1255-1261`、`:1394-1412`）。测试 `:4228`：注入用户在 item2 前改 `shared.txt` → `["merged","skipped"]`、用户内容原样保留、item2 未并；旧实现（无逐件检查、回退 reset --hard）会红/吞改动。 |
| 10 | 已修 | 三种 note 分开：`glob 无效：<pattern>`（`:894-909` + `:1172-1174`）、`声明范围未匹配到改动`（`:1191`）、`范围内没有改动`（`:1189`/`:1219`/`:1233`）。合并侧不再用 worktree `is_dir()`：`swarm_path_matches :868-893` 裸名分支同时匹配自身与 `path/**` 后代（`:880-883`）；`is_dir()` 仅剩 plan 侧 `:594` 与 worktree 存在性 `:1143`。测试 `:4246`（三种 note 互斥）。 |
| 11 | 已修 | 裸目录名覆盖整目录删除（同上 `:880-883`），非法 glob 单独记 note（`:894-909`）。测试 `:4262`：声明 `["olddir"]` + worktree 删光目录 → merged 且项目 `olddir` 消失；旧实现 skipped 必红。 |

测试补课 4 项均已做：`:4483` 改为真 repo 合并并断言 `skipped：未声明文件范围`＋文件未并入；`:3910` 断言冲突 note 含文件名与 worktree 路径；`:4123` 覆盖“新建+冲突同补丁/三件组合/下一批继续”；`:3912`/`:4142`/`:4292`/`:4312` 已全部换成 `--untracked-files=all` 全量断言（全仓已无 `--untracked-files=no`）。

## 新发现（按严重度）

**1. [中] 恢复复核把“排除目录内的残留”误判为恢复失败 → 结果降级 + 整批 halt（新回归）**
`src/dispatch.rs:536-545`：复核用全量 porcelain，取**所有**剩余路径判空，未按 `excluded_swarm_path` 过滤（与逐件检查 `:1254` 口径不一致）。
复现：项目无 `.gitignore`（08D 的目标场景），项目里 `m.py`；worktree 改 `m.py`，`item.accept = python -c "import m" && exit 9` → 验证产生 `__pycache__/m.cpython-*.pyc`（排除目录、untracked）→ 验证红走回退 `:1356` → `restore_swarm_paths` 复核见 pyc → `Err("恢复后项目仍有改动：__pycache__/m.cpython-312.pyc")` → `:1357-1360` 把 `test_failed` 改成 `skipped`、写“项目状态不确定，请手动恢复工作区与索引”、`halt_after_item` 让余件全 skip。等价的更宽触发：itemN 验证通过但留下 pyc（被容忍），itemN+1 冲突时同样误报。影响：08A 既有语义（验证红只回退本件、后件继续）在实际 pytest 场景下失效，note 指向并不存在的“项目状态不确定”。无任何测试覆盖（现有验证用例只用 `exit 9`/`printf`，不产生缓存文件）。
建议：`:541-545` 先过滤 `!excluded_swarm_path(path)` 再判空（保留“排除目录外的残留才算失败”），或在复核里只关心 `staged ∪ rollback_paths` 相关路径。

**2. [低] 回退删除集合比“补丁新增”更宽，且 note 未说明已删除**
`:1347-1353` 把验证产生的脏路径并入 `rollback_paths`，`:526-535` 对其中 untracked 项无条件删除。项目在件开始时是干净的，所以不会删到用户既有文件（数据安全），但“验证命令在排除目录外新建的产物”会被静默删掉，note 只写“验证命令改动了项目文件：…”。建议 note 补一句“已删除验证产生的未跟踪文件”。

**3. [低] 删除补丁新增文件后不清理空目录**
`:526-535` 只 `remove_file`，`src/new/a.rs` 回退后留空目录 `src/new/`；git 不显示空目录，故测试与入口检查都审不出来。建议顺手向上删空父目录（与 `git apply` 删文件时 `try_remove_empty_parents` 行为对齐）。

**4. [低] 验证脏路径 note 未限长**
`:1354` 用 `paths.join(", ")`，未走 `:921` 的 20 条截断；验证命令一次弄脏几千文件时 note/留痕会膨胀（与第 6 条同类，但漏了这处）。

**5. [低] 临时补丁文件位置与授权**
`:1018` 放在系统 temp：进程被强杀/断电时残留（内含项目源码 diff，权限随 umask，多用户机上可读），且无启动时清理。建议落 `.dispatch/`（已自带 `*` 的 `.gitignore`，见 `:3552-3560`）或加 0600 + 启动清理 `aicross-swarm-*.patch`。

**6. [低] 入口脏检查与逐件脏检查口径不一致**
入口 `:1083-1092` 不过滤排除目录（保持原语义），逐件 `:1254-1255` 过滤。后果：批中验证在 `__pycache__` 等排除目录里留下的垃圾，不挡同批后续件，但会让**下一批**在入口整批 `skipped：项目目录有未提交改动`——fix 第 4 条“下一批入口不再被它卡成项目脏”只闭合了“补丁新增文件”这一类。

**7. [低] baseline 取不到时不做任何清理**
`:1263-1265` 用 `.ok()` 兜底，取不到则 `baseline` 为空；此后任何回退都直接 `Err("无法确定回退基线")`（`:455-459`），已暂存/半应用的补丁留在 index/worktree，只 halt＋人工指引（不静默，但不清残留）。概率极低（批初已成功 `rev-parse HEAD`）。

**8. [低] 测试覆盖/可红性的小缺口**
- `:4166`（截断）与 `:4228`（批中脏）用 thread_local 注入标志/文件，不是真造 >4 MiB status 或真竞态；分支语义覆盖到位，但不是端到端证据。
- `:4153` 只覆盖 `R `（index 列）rename，未覆盖工作区列 ` R`。
- `:4087-4105` 未断言 note 中**不含**“恢复失败/项目状态不确定”，而该用例的恢复路径正是“路径不在 HEAD/index”（`image.bin` 已被换成目录），若 `git reset -q HEAD -- <不存在路径>` 在未来 git 版本改为非零，测试仍会通过。
- 未覆盖新发现 1。

**9. [极低] plan 侧 status 口径未同步**
`:601` 仍是裸 `git status --porcelain`（无 `-z`/`-uall`）。只影响 `SwarmPlanCheck.dirty` 布尔与 plan note（未跟踪目录照样出 `?? dir/`），与合并侧 `-uall` 不一致但不误判；fix 清单未要求，仅记录。

## 核过没问题的点

- **定向恢复的四个调用点全部不碰范围外路径**：`reset -q HEAD -- <selected>` 与 `checkout -- <tracked>` 都带 `-- <path>`；`reset --soft <baseline>` 只动 HEAD（index/worktree 不动），配合定向 reset 后用户/验证在其它路径上的改动不会被吞（`:454-518`，测试 `:4228` 断言用户内容原样）。
- **暂存后二次核验不误删范围内文件**：`rejected`/`allowed` 互斥（`:992-998`），reset 只列 rejected，且只改 index 不改 worktree（`:1002-1008`）。
- **补丁文件清理完备**：写出后 4 个后续分支（空补丁/状态不可确认/批中脏/已调用 apply）与生成失败 3 个分支全 `remove_file`（`:1028-1035`、`:1231`、`:1246`、`:1256`、`:1269`）。
- **大补丁与非 UTF-8 不再受 4 MiB/`from_utf8_lossy` 影响**：`src/process.rs:10` 的上限只剩 stdout 路径，dispatch.rs 无 stdin 传补丁的调用（全 src grep 确认）。
- **note 分级与目录口径**：合并侧已无任何 worktree `is_dir()` 依赖（`:594` 属 plan 侧、`:1143` 是 worktree 目录存在性）；裸声明匹配自身与后代，删除/新增/改名都覆盖。
- **契约未变**：`SwarmMergeItem`/`SwarmMergeResult` 字段、camelCase 序列化与既有契约测试 `task08a_tauri_swarm_contract_serializes_camel_case_and_merge_results` 保持不变；`merge_swarm_items` 命令签名 `:1419-1431` 未变；留痕字段（schema/status/swarm_item/merge_result）`append_swarm_record :742-771` 未变；前端 wire（`src-ui/src/graph/swarm.ts:393-403` 发 `files`）与之对齐。
- **既有 08A/08D 测试只加强未削弱**：三处状态断言由 `--untracked-files=no` 升级为全量 porcelain 为空（`:3912`/`:4142`/`:4292`/`:4312`），冲突用例新增 note 断言（`:3910`），超时→`test_failed`、只回退红件、上限、脏树整批 skipped、reset/恢复失败 halt＋人工指引等既有断言均保留（`:3927`、`:4428`、`:4448`、`:3920`、`:3965`、`:3982`）。
- **无命令注入面扩大**：patch 路径经 argv 直传、无 shell；`--output=`/`apply <file>` 参数均为受控字符串。