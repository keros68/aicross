## 总判

**可交付但有未闭合项**：合并源、基线、apply/恢复、验证回退这条主线按契约实现了且真实 git 测试能跑通；但有一处会让 08D 想修的「垃圾进主工作树」原样复发（未跟踪新目录绕过排除清单 + glob 过滤），另有若干静默漏并、上报与幂等口径问题。

---

## 按严重度

**1. [高] 未跟踪新目录被 `git status` 折叠成单条 `dir/`，排除清单与 glob 范围过滤同时失效**
`src/dispatch.rs:823`（`status --porcelain -z` 未带 `-uall`）→ `:1016-1025`（选择/范围外）→ `:1031-1041`（`git add -A -- <selected>`）。
- 默认 `status.showUntrackedFiles=normal` 下，完全未跟踪的目录只输出一条 `?? src/foo/`（本仓 `src/git.rs:326` 正是为此用 `--untracked-files=all`，注释写「every untracked file listed」）。于是：
  - (a) **垃圾重新进项目**：该条目未被排除清单命中，若被声明的 glob 命中（`**`、`src/**`、`src/foo` 归一化后加 `/**`），`git add -A -- src/foo/` 会**递归**暂存该目录下所有文件，包括 `src/foo/__pycache__/*.pyc`、`node_modules`、`build` 等排除目录里的内容（脚本侧只过滤了目录条目本身，没过滤递归结果），补丁与合并提交把它带进主工作树，验证 pytest 再把它弄脏——与 08D 的动因同型。demo 项目没有 `.gitignore`，pyc 不被忽略，正好命中。
  - (b) **静默漏并**：声明是文件级（`src/foo/new.rs`，`:837` 的 `swarm_path_matches` 用该模式去匹配 `src/foo/`，还带尾斜杠）→ 不匹配 → `selected` 为空 → `:1027-1029` 写 `skipped：范围内没有改动`，声明范围内的新文件永不并入，note 还暗示模型没干活。`check_swarm_plan` 走的是文件系统（`existing_project_files`），会显示已匹配，两侧口径不一致。
- 复现（30 秒）：`mkdir -p /tmp/x && cd /tmp/x && git init -q && mkdir -p a/b && touch a/b/f && git status --porcelain` → 输出 `?? a/`；再让某件不提交地新建 `src/new/x.rs`（文件级声明）与 `src/new/__pycache__/y.pyc`（目录级声明）各跑一次。
- 建议：取 worktree/项目状态的三处 `status --porcelain -z` 全部加 `--untracked-files=all`；对以 `/` 结尾的条目做显式展开/目录前缀处理；`add`/`diff` 前对 `selected` 再逐条做排除目录校验（不能只校验被 `-A` 递归覆盖的目录条目本身）。

**2. [中] `git apply` 任何非零都判 `conflict`，丢掉了 R2「只有含冲突标志才是 conflict」的分级**
`src/dispatch.rs:1063-1081`、`:1084-1091`。超时/启动失败已正确降为 `skipped`，但其余非零（`error: … does not match index`、二进制补丁 3-way 不支持、`patch does not apply`）一律 `conflict`；此时 `git diff --diff-filter=U` 为空 → `files = selected.join(", ")`，note 把整件声明范围列成「冲突文件」。
- 复现：声明范围内含二进制文件（legit 图片/已跟踪 pyc），项目侧该文件已被前一件改过 → `apply --3way` 对二进制不给 3-way 归并 → 非零且无 unmerged 条目 → 结果 `conflict`，用户被引向并不存在的冲突裁决。
- 建议：`conflicts` 为空时按 R2 降为 `skipped` + 原文 git 报错；只有 `conflicts` 非空（或输出含 CONFLICT/冲突标志）才 `conflict`。

**3. [中] 大规模 worktree 下 note 无上限、`outside` 取差集是 O(n·m)、且忽略 `truncated`**
`src/dispatch.rs:1020-1025`（`selected.contains` + 全量 `join(", ")`）、`:822-835`（只判 `process_succeeded`，不看 `truncated`，`src/process.rs:10` 上限 4 MiB）。
- 触发场景正是 08D 针对的「项目误跟踪了构建产物/node_modules」的 demo：worktree 里几万条路径同时进 `outside` → 2.5e9 次字符串比较 + 一条几百 KB 的 note 写进留痕并过 IPC；若 status/diff 输出被 4 MiB 截断，声明范围内的路径会**静默丢失**，结果报 `merged`/`范围内没有改动`。
- 建议：`truncated` → 该件 `skipped` + note；`outside` 用 `HashSet` 判差集并截断展示（前 N 条 + 「等 M 项」）。

**4. [中] 幂等缺口：已并入件再次进入请求时落点都不是「已并入」语义**
`src/dispatch.rs:1060-1119`（apply + commit 分支）。范围 diff 会被重新生成（`base` 仍是 `merge-base(批初 project_head, worktree HEAD)`），重送时：要么 apply 成功但 index 与 HEAD 无差异 → `git commit` 报 `nothing to commit` → 写成 `skipped：提交声明范围补丁失败…`；要么 3-way 归并出非零 → 写成 `conflict`。两者都不该出现（前端 `swarmPendingMergeIds` 只挡 adopted 件，单件重派/换批重送仍可能带上）。
- 复现：同一 item 连跑两次 `merge_swarm_items_core`（第二次不修任何东西）；同批内重复 item 号也是同一表现。
- 建议：apply 后判 `git diff --cached --quiet`，无差异直接 `skipped：范围内改动已在项目中`（幂等），不要落到 commit 失败分支。

**5. [中] 失效路径只回滚 tracked，残留未跟踪文件不被清理且恢复后不复核状态**
`src/dispatch.rs:504-528`（`checkout -f -- .` / `reset -q` / `checkout -- .` 不删未跟踪文件）、`:482-500`（`reset --hard baseline` 同样不动未跟踪）、`:1133-1149`。
- 三种残留：apply 失败时补丁新增的未跟踪文件（带冲突标记）、验证命令在排除清单**外**新建的文件（该件 test_failed 后仍留在项目里）、排除清单**内**未被跟踪的验证产物。前两类会让**下一批**在 `:922` 的入口整批 `skipped：项目目录有未提交改动…`，与「对项目卫生问题免疫」的目标相反。
- 同时：恢复用的是整棵工作树的 `checkout -f -- .`，比必要的范围大（批次内已被清理干净时无损失，但用户在合并期间动了项目文件会被静默丢弃）。
- 建议：恢复/回退后跑一次 `git status --porcelain` 自检，不干净就写人工指引；恢复路径按补丁涉及的 `selected` 定向 `checkout`/`restore`，别用 `.`。

**6. [低] `parse_git_status_paths` 的重命名处理与仓库既有约定不一致**
`src/dispatch.rs:792-812` 对照 `src/git.rs:324-341`。
- 只判第一列 `R`/`C`（git.rs 判 `x` 或 `y`），工作区侧重命名 ` R` 不消费紧随其后的原路径字段，该字段被当成新记录解析出 `path[3..]` 的垃圾路径（`src/a.rs` → `/a.rs`），污染「范围外改动未并入」note；若垃圾路径被声明的 glob 命中（如 `**`、`**/*.rs`），`:1031` 的 `git add -A -- /a.rs` 会因 pathspec 不匹配而把该件判成 skipped；`restore_validation_exclusions` 也可能据此给出错误文件名的 `test_failed`。对第一列重命名取的又是原路径而非当前路径，与 git.rs 的取法相反。
- 建议：与 git.rs 同口径（两列都判、取当前路径），补一条 rename 用例。

**7. [低] 声明 glob 的三个边界**
`src/dispatch.rs:569-580`（`normalized_swarm_glob`）、`:837-848`。
- `case_sensitive: false`：Linux 上声明 `src/Foo.rs` 会并入 `src/foo.rs`（与 `check_swarm_plan` 同口径，但是合并侧真动文件）。
- 目录判定用 **worktree** 的 `is_dir()`：agent 把声明的目录删掉后（该目录在 worktree 已不存在），模式退化成文件模式，整目录删除不会被并入，只报「范围外改动未并入」；`check_swarm_plan` 用的是项目树，两侧可能给出相反结论。
- 非法 glob（`[broken`）与「glob 匹配不到任何改动」都被压成 `范围内没有改动`（`:1029`），而 plan 侧会给出 `unmatched` + 错误文案；`..`/绝对路径/`C:\…` 这类声明不构成穿越（只触发一次 `is_dir()` 存在性探测，pattern 只对 repo 内相对路径匹配），但同样静默无匹配。
- 建议：note 区分「glob 无效 / 未匹配到改动」与「范围内没有改动」；目录判定以合并基线树或项目树为准。

**8. [低] 验证命令超时/启动失败仍归 `test_failed`**
`src/dispatch.rs:1120-1140` + `run_validation` `:2367-2394`。note 写「验证命令未通过」，超时原文只在 `test_output` 里，与 TASK-08D 第 4 条「超时/启动失败 → skipped」的字面口径不一致（08A R2 的该条只针对 cherry-pick，所以这是既有语义，08D 未动）。要么按任务书收口，要么在契约里写明「验证阶段超时归 test_failed」。

**测试侧**
- `src/dispatch.rs:4036 task08d_merge_wire_defaults_missing_declared_files_to_safe_skip` 是同义反复：只断言 `serde` 反序列化后 `files` 为空，没有断言合并真返回 `skipped：未声明文件范围`（`:995-996`）；报告却把它当作该行为的证据。
- 未覆盖：空 `files` 的 skip 分支、文件级声明 + 新建未跟踪目录（finding 1 的两条症状）、新目录内嵌 `__pycache__` 是否进补丁、已并入件重送（finding 4）、rename（finding 6）、冲突 note 是否真含冲突文件与 worktree 路径（`:3674` 只断言 result 序列，没断言 note）、未跟踪的排除目录产物是否原样保留。
- `:3809 task08d_out_of_range_and_excluded_files_are_not_applied` 用 `src/**` 覆盖新目录场景，在「折叠」与「不折叠」两种实现下都会通过，因此没有钉住 finding 1 的行为。
- `:3686`、`:3845`、`:3865` 的状态断言都带 `--untracked-files=no`，会把「恢复后残留未跟踪文件」这一类回归静默放过。

---

## 核过没问题的点

- **基线取法**：`:931` 批初 `project_head` 固定，`:998` 每件 `merge-base <project_head> HEAD` 在 worktree 内取，与本批前几件是否已合并无关；冲突/回退后重试同一件仍复现同一基线（`:3869` 真跑过）。
- **补丁来源与范围**：`:1031-1047` `add -A -- <selected>` → `diff --binary --full-index --cached <base> -- <selected>`，包含已提交与未提交改动；空补丁 → `skipped：范围内没有改动`（`:1056-1058`）；跟踪的同一 pyc 在两件各改一次 → 两件都 `merged` 且项目 pyc 保持基线内容（`:3787` 真跑两个 worktree）。
- **无命令注入**：git 一律 `run_process_registered` 直跑（`src/process.rs:269-287` 只 `Command::new(program).args(...)`，非脚本分支无 shell），补丁经 `git_result_with_input` 走 stdin；动态实参只有十六进制 sha 与路径，声明里的 `..`/绝对路径/`.git` 不会进入 argv，也没有对 worktree 之外的文件写操作（worktree 侧仅一次 `git add` 暂存，不动磁盘内容）。
- **冲突后状态**：`checkout -f -- .` → `reset -q` → `checkout -- .` 对 tracked 文件确实回到并入前（`:3674` 真冲突：第一件内容保留、`rev-list` 不变、`ls-files -u` 为空），`git apply` 不留下 rebase/cherry-pick 之类的状态文件。
- **验证红回退**：baseline 在 apply 前取（`:1061`），失败只 `reset --hard` 本件，前件保留、后件继续（`task08a_failed_validation_…`、`task08a_r1_middle_validation_failure_…`）；回退失败/清理失败 → halt + 后续件 `skipped` + 人工指引，不吞错（`:3756`）。
- **数据安全底线**：入口 `git status --porcelain` 非空即整批 `skipped`（`:922-946`，note 带 porcelain 摘要），所以正常流程下 `checkout -f -` / `reset --hard` 不会碰到用户未提交的改动；`.dispatch` 留痕自带 `*` 的 `.gitignore`（`:3327-3335`），不会把下一批卡成「项目脏」。
- **排除清单**：复用快照清单（是任务书列举项的超集），`excluded_swarm_path` 逐路径分量判定，`.git` 内部与 `.dispatch` 不会被并入，也不会被当作验证脏改动。
- **测试基础设施**：新增 8 条都用 `swarm_repository` 真建临时 repo + 两个 worktree 真跑 git，缺 git 时 `require_git` 跳过；超时/恢复失败的注入用 `thread_local`，不污染并行用例。
- **报告与代码不一致（非代码缺陷）**：REPORT-08D 说「前端 `swarmMergeItems` 尚未把节点的 `swarm.files` 放入合并请求」，但 `src-ui/src/graph/swarm.ts:399` 已在发 `files`，`src-ui/src/graph/__tests__/swarm-08c.test.tsx:215` 还断言 wire 键含 `files`；前端到后端这条线是通的，报告的边界节写错了（「未改 src-ui」本身属实）。