# TASK-08D 蜂群合并按声明范围取 diff（09-15 窗口真跑发现）

真跑现象：两件文件范围互不相交，但各自 worktree 的 `git add -A` 把 `__pycache__/*.pyc`（demo 项目误跟踪）一起提交，第二件 cherry-pick 在 `src/__pycache__/__init__.cpython-312.pyc` 上二进制冲突；合并后验证命令跑 pytest 又把项目里被跟踪的 pyc 改脏。引擎要对这类项目卫生问题免疫。在 main 之上做，只动 `src/dispatch.rs`（合并循环）与其测试、`tasks/TASK-08-swarm.md` 契约节。

1. **合并源改为「声明范围内的 diff」**：不再 cherry-pick worktree 提交；对每件：`git -C <worktree> add -A -- <files...>`（只加声明范围）后 `git diff <base>..HEAD -- <files...>`（base = worktree 创建时的项目 HEAD，即 `git merge-base`）连同未提交改动一起取（先在 worktree 里对范围内文件做一次临时提交或直接用 `git diff <base> -- <files>` 含工作区），在项目里 `git apply --3way --index` 该补丁 → 成功后 `git commit -m "aicross swarm item N: <title>"`；补丁为空 → `skipped`「范围内没有改动」；`apply` 冲突 → `git checkout -- . && git reset -q`（把项目恢复到并入前）→ `conflict`，note 列出冲突文件（`git apply` 输出里的路径）与 worktree 路径。
2. **范围外改动与垃圾目录**：worktree 里范围外的改动只进 note「范围外改动未并入：a, b…」，不并入；`__pycache__/`、`.pytest_cache/`、`node_modules/`、`target/`、`dist/`、`build/`、`.dispatch/`、`.git/` 无论是否在范围内都排除（沿用快照排除清单）。补测试：范围外文件与 pyc 不进补丁。
3. **验证后清理**：每件验证命令跑完，`git status --porcelain` 里若只有排除清单内的路径被改脏 → `git checkout -- <这些路径>`（未跟踪的不动）；有排除清单之外的脏改动 → 该件标 `test_failed`（note 写「验证命令改动了项目文件：…」）并回退本件。补测试。
4. **保留 R2 的分级语义**：超时/启动失败 → `skipped`；`reset`/`checkout` 失败 → 停批 + 指引；`swarm_limit`、写锁、登记校验不变；对账仍忽略合并留痕。既有测试改口径，新增：两件范围不相交但都改了同一个被跟踪 pyc → 两件都 `merged`；范围内文件冲突 → `conflict` 且项目干净。
5. 契约节把「cherry-pick」改成「按声明范围取 diff 应用」并说明原因。

验证：`cargo check` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。不跑 `cargo fmt`、保持换行；不动 `src/channels/`；不起窗口、不发真机请求、不 commit、不 push。报告写 `tasks/REPORT-08D.md`。
