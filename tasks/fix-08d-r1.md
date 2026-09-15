# fix-08d-r1 蜂群合并（08D）复审修正轮

依据 GLM-5.3-Flash + DeepSeek 对 main（含 037706a 08D、7ef8340 08E）的盲审。只动 `src/dispatch.rs`（及其 tests），不动前端、不动契约。编排者已在代码里核实第 1 条。

## 必修（两家共同：3/4/10/11；DeepSeek 独有：1/2/5/6；GLM 独有：7/8/9）

1. **未跟踪新目录被折叠**：`swarm_changed_paths`（:823）、项目侧（:863）、入口脏树检查（:922）的 `status --porcelain` 都没带 `--untracked-files=all`，未跟踪目录只出一条 `?? dir/`。后果 (a) 目录级声明命中 `dir/` 后 `git add -A -- dir/` 递归把 `dir/__pycache__/*.pyc` 等排除目录里的东西并进主工作树（08D 想修的那类垃圾原样复发）；(b) 文件级声明（`src/foo/new.rs`）匹配不上 `src/foo/`，selected 为空 → 「范围内没有改动」，新文件永不并入。修法：三处都加 `--untracked-files=all`（本仓 `src/git.rs:326` 已是这个口径）；`add`/`diff` 之后再对暂存清单逐条核一次排除目录与 glob（`git diff --cached --name-only`），有越界的从 index 里 `reset` 掉并记入「范围外改动未并入」。测试：文件级声明 + 新建未跟踪目录；目录级声明 + 目录内嵌 `__pycache__`，断言 pyc 不进补丁。
2. **`git apply` 非零一律判 conflict**（:1063-1091）：无 unmerged 条目（`diff --diff-filter=U` 为空）时按 08A-R2 口径降为 `skipped` + git 原文；只有 `ls-files -u`/unmerged 非空才 `conflict`，冲突文件列表只列真冲突的。测试：二进制文件 3-way 失败 → skipped 而非 conflict。
3. **幂等**：apply 后 `git diff --cached --quiet` 无差异 → `skipped：范围内改动已在项目中`，不落到 `nothing to commit` 的提交失败分支。测试：同一件连并两次。
4. **失效路径残留未跟踪文件**（:504-528、:482-500）：apply 失败/验证红回退后跑一次 `status --porcelain --untracked-files=all`，把补丁新增的（在 selected 内的）未跟踪文件删掉；仍不干净则 note 写人工指引，不整批静默。恢复路径按 `selected` 定向 `checkout`/`restore`，不用 `.`（用户在合并期间动的文件不被吞）。测试：apply 失败留下的新文件被清；下一批入口不再被它卡成「项目脏」。
5. **`parse_git_status_paths` 重命名口径**（:792-812）与 `src/git.rs:324-341` 对齐：两列都判 R/C，消费 `-z` 下紧随的原路径字段，取当前路径。补 rename 用例。
6. **输出截断与规模**（:822-835、:1020-1025）：status/diff 结果 `truncated` → 该件 `skipped` + note；`outside` 用 HashSet 求差集，note 只列前 20 条 + 「等 N 项」。
7. **补丁超 4 MiB 被静默截断**（:1046 走 `git_result`，`src/process.rs:10` 上限 4 MiB，`truncated` 标志在合并路径从未检查）：截在文件段边界时是合法的更短补丁，干净应用并 commit，后面的声明文件改动静默丢失、结果仍 `merged`。修法：补丁不走 stdout/&str——`git diff … --output=<临时文件>` 落盘，`git apply --3way --index <file>` 读文件，同时解决第 8 条；若保留 stdout 路径则 `patch.truncated` → `skipped` + note「补丁超过输出上限」。测试：注入小上限或造 >4 MiB 二进制改动，断言 skipped 或完整并入。
8. **非 UTF-8 文本经 stdout 管道 lossy 解码**（:1063-1069 补丁作 `&str` 经 stdin 回传，`src/process.rs:415` `from_utf8_lossy`）：GBK 源文件新建路径会把 U+FFFD 乱码并进项目并 commit，已跟踪路径变假冲突。修法同第 7 条：补丁字节级落盘再 apply。测试：worktree 新增一个 GBK 编码文件在范围内，断言并入后字节与 worktree 一致。
9. **批中无逐件脏检查**（:922-944 只在批开始查一次；:1141 回退 `reset --hard`）：每件验证可跑数分钟，用户此间改了项目已跟踪文件，一旦本件走回退会被 `reset --hard` 一并清掉。修法：每件 apply 前重跑 `status --porcelain --untracked-files=all`（排除目录外）非空 → 余下件全部 `skipped` + note，不带脏回退。
10. **note 分级**（:1029）：glob 无效 / 未匹配到改动 / 范围内没有改动 三种分开说；目录判定以合并基线树（`git cat-file -e <base>:<dir>` 或 `ls-tree`）为准，不用 worktree 的 `is_dir()`，agent 删了整目录也能并入。

11. **裸目录名声明覆盖不到整目录删除 / 非法 glob 静默**（:569-581、:837-846）：目录在 worktree 已不存在时 pattern 保持裸名，删除路径 `olddir/a.rs` 不命中；`compile_glob` 失败的 pattern 直接不参与匹配。修法：裸名同时匹配 `dir` 与 `dir/**`（不依赖 `is_dir()`）；compile 失败追加 note「glob 无效：<pattern>」。测试：声明 `["olddir"]` + worktree 删光 `olddir/*` → 删除并入。

## 测试补课

- `task08d_merge_wire_defaults_missing_declared_files_to_safe_skip` 只断言反序列化，补断言合并真返回 `skipped：未声明文件范围`。
- 冲突用例（:3674 附近）断言 note 含冲突文件与 worktree 路径；补「新建文件 + 冲突文件同补丁」用例，断言恢复后 `status --porcelain --untracked-files=all` 为空（配合第 4 条）；补 item1 merged / item2 conflict / item3 merged 组合。
- :3686/:3845/:3865 三处状态断言去掉 `--untracked-files=no`，改成全量 porcelain 为空（配合第 4 条）。

## 保持

- 基线取法（批初 project_head + 每件 merge-base）、`add -A -- selected` → `diff --binary --full-index --cached` 的补丁来源、验证红只回退本件、脏树整批 skipped、排除清单复用快照清单——都核过没问题，不动。
- 验证命令超时仍归 `test_failed`（既有语义，本轮不改，任务书第 4 条口径以代码为准）。

验证：`cargo check` 零 warning、`cargo test`（新增用例真建 repo + worktree 跑 git）。不 `cargo fmt`；`src/dispatch.rs` 保持原有换行。不 commit、不 push。报告写 `tasks/REPORT-08D-R1.md`。
