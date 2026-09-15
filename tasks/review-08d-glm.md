**总判：可交付但有未闭合项** —— 契约主线（范围过滤、基线、--3way、验证清理、分级语义）实现正确且有真实 git 测试支撑；但补丁传输管道存在一处可致静默部分并入的缺陷，冲突恢复对“新建文件”场景不闭合，另有若干小项。

## 按严重度排序

1. **补丁超过 4 MiB 被静默截断，可能标 `merged` 丢掉部分声明文件的改动** — `src/dispatch.rs:1046`（生成补丁走 `git_result`）+ `src/process.rs:10`（`MAX_PROCESS_OUTPUT = 4 MiB`）、`src/process.rs:476`（`bounded_reader` 用 `.take()` 截断并置 `truncated` 标志）。问题：`ProcessResult.truncated` 在合并路径从未检查（dispatch.rs 里该标志只有节点执行路径 2902 行消费）；补丁 stdout 截断后两种结局：(a) 截在文件段中间 → `git apply` 报 corrupt patch → 被误标为 `conflict`（安全但标注错误）；(b) 截在文件段边界 → 恰好是合法的更短补丁 → 干净应用并 commit，排在补丁后面的声明文件改动静默丢失，结果 `merged`。复现：声明范围含一个较大的非排除二进制文件（数据/fixture/图片，`--binary` 后超 4 MiB）或超大文本改动。建议：在 `dispatch.rs:1052` 的 `Ok(patch)` 分支增加 `patch.truncated` 检查 → `skipped` + note「补丁超过输出上限」；或补丁改走临时文件传输。

2. **冲突补丁里“新建文件”在恢复后残留为 untracked 垃圾，项目未回到并入前状态** — `src/dispatch.rs:504-529`（`restore_after_apply_conflict`）、`src/dispatch.rs:1083-1096`。问题：`git apply --3way --index` 部分成功时，补丁中能干净应用的文件（含新建文件）会留在工作区和索引；恢复序列 `checkout -f -- .` → `reset -q` → `checkout -- .` 只能把索引回退到 HEAD，被 apply 新建过的文件在 `reset -q` 后变成 untracked，第三步 checkout 不碰 untracked —— 残留。既有冲突测试（3674）只覆盖“两边改同一个已跟踪文件”，`--untracked-files=no` 断言恰好测不到它。复现：某件范围含一个新建文件 + 一个与项目 HEAD 冲突的已跟踪文件，两文件同补丁，冲突路径。后果不是丢数据而是批次结束后项目残留无主 untracked 文件，违反任务书“恢复到并入前”。timeout 路径（1075-1081）同理。建议：apply 前从补丁文本解析出 `new file` 路径列表，恢复成功后对其中仍是 untracked 的路径做定点删除；或恢复后比对 `git status --porcelain`（含 untracked）非空则进 note/停批。

3. **非 UTF-8 文本文件经 stdout 管道被 lossy 解码损坏** — `src/dispatch.rs:1063-1069`（补丁以 `&str` 经 stdin 回传）+ `src/process.rs:415`（`from_utf8_lossy`）。问题：GBK 等非 UTF-8 编码的文本文件（无 NUL，git 按文本处理）其 diff 原始字节经 lossy 解码变成 U+FFFD 再回写 stdin。新建文件路径：apply 直接用补丁文本构造内容 → **乱码内容被静默并入并 commit**；已跟踪文件路径：上下文行损坏 → 假冲突。复现：worktree 里新增一个 GBK 编码的源文件，在声明范围内。建议：补丁改为字节级传输（`git diff` 输出写临时文件、`git apply <file>`），这是根修；短期至少在 note 提示非 UTF-8 风险。本项目场景（中文 Windows、Python 项目）现实概率不低。

4. **已并入件重复合并会被报成失败** — `src/dispatch.rs:1097-1117`。同一件再次进入请求时：diff 重新生成、`apply --3way` 内容无变化成功、`git commit` 因 nothing to commit 非零 → `skipped` + note「提交声明范围补丁失败：nothing to commit」+ 一次多余但无害的 `reset --hard HEAD`。安全，但幂等重入在 UI 上呈现为错误，用户无从区分真失败。复现：合并成功后同一批再次点合并。建议：commit 失败时识别 stderr 含 "nothing to commit" → `skipped`「范围内改动已并入」且不 reset。当前无测试覆盖此场景。

5. **批中无逐件脏检查，回退会连带清掉批开始后进入项目目录的改动** — `src/dispatch.rs:922-944`（脏检查仅在批开始一次）、`src/dispatch.rs:1141`（回退用 `git reset --hard <baseline>`）。每件验证可跑数分钟，期间用户若在项目工作树改了任何已跟踪文件：无回退时该改动留在工作树（commit 只取索引，不受污染）；一旦走到回退路径（验证红、验证改脏、commit 失败），`reset --hard` 把它一并清掉且无提示。写锁只约束 aicross 自身进程。建议：每件 apply 前重跑一次 `git status --porcelain` 检查（此时排除目录垃圾已被上件清理，检查应为空），非空则余下件全部 `skipped`，避免带脏回退。

6. **裸目录名声明无法覆盖整目录删除** — `src/dispatch.rs:569-581`、`src/dispatch.rs:837-846`。`normalized_swarm_glob` 仅当目录在 worktree 中存在时追加 `/**`；件把某目录整个删掉时该目录在 worktree 已不存在，pattern 保持裸名，`require_literal_separator: true` 下 `olddir` 只匹配字面路径 `olddir`，而 git 报告的删除路径是 `olddir/a.rs` → 不命中 → `skipped`「范围内没有改动」，删除静默不并入。复现：`files: ["olddir"]`，worktree 中删除 `olddir/*`。建议：对不存在的裸目录路径改加 `/**` 或同时匹配 `dir` 与 `dir/**`（plan 阶段会报 unmatched 提示，但合并阶段是静默的）。

7. **合并阶段非法 glob 被静默忽略** — `src/dispatch.rs:838-845`。`compile_glob` 失败时 `is_ok_and` 使该 pattern 直接不参与匹配；plan 阶段有 unmatched/note 报告，合并阶段用户只看到「范围内没有改动」或部分并入，不知道有 pattern 没生效。建议：compile 失败时向 note 追加「glob 无效」。

8. **测试覆盖缺口**（其余测试质量高，见下）：(a) `task08d_merge_wire_defaults_missing_declared_files_to_safe_skip`（dispatch.rs:4036）名实不符——只断言 serde `files` 默认为空，未走到合并层的 `skipped`「未声明文件范围」分支（dispatch.rs:994-996 无任何合并级测试）；(b) 发现 2 的新建文件冲突残留、发现 1 的截断、发现 4 的重复合并均无测试；(c) 冲突后继续并第三件（item1 merged / item2 conflict / item3 merged 组合）未直接覆盖，仅有 timeout 变体（3739）近似。

## 核过没问题的要点

- **范围过滤**：改动清单 = `git diff --name-only -z <base> --`（含已提交）∪ `status --porcelain -z`（含未提交/未跟踪/删除，R/C 两字段解析正确，dispatch.rs:792-835）；按路径任一分量匹配排除清单（`__pycache__`/`.pytest_cache`/`node_modules`/`target`/`dist`/`build`/`.dispatch`/`.git` 等，dispatch.rs:786-790），Windows `\` 统一转 `/`；范围外（含 pyc）只进 note。真实 git 双 worktree 共享被跟踪 pyc 的测试（3787）断言两件 `merged` 且 pyc 保持基线内容，与 08D 立项场景一致。
- **基线**：`git merge-base(本批初始项目 HEAD, worktree HEAD)`（dispatch.rs:998），批内稳定不受前件 commit 影响；`--full-index` 的 blob id 借 linked worktree 共享对象库在项目侧可解，`--3way` 前提成立；逐件回退基线在 apply 前现取（1061-1062），取不到则拒绝回退并停批。
- **冲突恢复（已跟踪文件）**：`checkout -f -- .` / `reset -q` / `checkout -- .` 序列经真实冲突测试验证（3674：内容还原、HEAD 数正确、`ls-files -u` 空）；恢复失败降级为停批 + 人工指引（3756）；apply 超时同样恢复且后件继续（3739）。
- **验证清理与回退**：验证只弄脏排除目录内被跟踪文件 → 逐路径 checkout，未跟踪不动（3830）；排除目录外脏改动 → `test_failed` + 回退本件（3850）；只回退红的那件、后件照常（3692、3967）；回退失败停批不吞（3950）。
- **数据安全**：批前项目不干净整批拒（3709）；worktree 必须 canonicalize 后命中 `git worktree list`（946、991，外来仓/已删目录被 skip，3904、3924）；清理 checkout 只作用于 status 派生且命中排除清单的路径，碰不到范围内文件；未跟踪文件全程不动、reset --hard 不删 untracked——无删除用户数据的路径。
- **注入与穿越**：所有 git 调用走参数向量无 shell；补丁经 stdin；`add`/`diff` 带 `--` 分隔；声明文件含 `..`/绝对路径时只会匹配不到（git 报告的路径恒为仓库相对路径），`project.join` 对绝对 pattern 仅做 is_dir 探测无副作用；`.git` 内部不可能出现在 diff/status 输出且被分量过滤双保险；`item.accept`/验证命令本身按设计进 shell，与 R2 信任级别一致。
- **分级与留痕**：上限 split 在排序后、溢出件 `skipped` 且写留痕（3988）；每件结果（含整批 skip）均写 `aicross-dispatch/1` 留痕，对账忽略合并留痕（4002）；`halt_after_item` 余件全部 skip 并留指引。
- **wire 已接通**：前端 `swarmMergeItems` 携带 `files`（src-ui/src/graph/swarm.ts:399），REPORT-08D 中“前端未接通”的未闭合项已由 01f3d0a 关闭；契约节 TASK-08-swarm.md:10 已更新为按声明范围取 diff 的口径。