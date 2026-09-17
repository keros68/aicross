# TASK-32 首版审查 · Antigravity / gemini-3.8-flash-high（盲审，材料内联）

**总判：不可交付。**（任务书第 4 条附件重名保护在生产代码中完全漏写，导致对应测试必然红测失败；且存在单个文件读取失败致整个盲审硬崩溃的未闭合问题）

---

### 按严重度编号的问题清单

#### 1. [严重] 生产代码完全漏写 `project` 附件保留名校验，测试必然红测崩溃
- **文件与大致位置**：`src/dispatch.rs:4475`、`src/dispatch.rs:4800-4850`（`prepare_outbound` 附件检查逻辑处）
- **问题**：任务书第 4 条明确要求「与 TASK-28 R1 的附件重名保护一致：`project` 也并入保留名，附件同名时报错」，报告中也宣称「`project` 与 `ANSWER.md` 一样加入本轮材料保留名」。但通读 diff，除定义了常量 `REVIEW_PROJECT_DIR` 外，**生产代码没有任何一处将 `project` 加入附件重名校验列表**。
- **复现**：运行新增测试 `task32_project_attachment_name_conflict_is_rejected`。由于 `prepare_outbound` 没有对 `project` 做重名拦截，附件会先被写入 `workdir/project` 文件中；随后流程走到 `copy_readonly_project`，调用 `fs::create_dir_all(workdir.join("project"))`，因同名文件已存在而抛出底层错误 `创建项目只读副本失败：...`，导致断言 `assert_eq!(error, "附件文件名与盲审材料重名：project")` **必然失败 panic**。报告中声称“7 条测试全部 passed”与当前 diff 事实不符。
- **改法**：在 `prepare_outbound` 原有校验 `ANSWER.md`、`REQUEST.md` 等保留名的位置，将 `REVIEW_PROJECT_DIR`（`"project"`）并入附件同名检查，遇同名直接返回 `Err("附件文件名与盲审材料重名：project".into())`。

#### 2. [严重] 单文件读取异常导致整个盲审流程硬崩溃，未实现“跳过不可读文件”的契约
- **文件与大致位置**：`src/dispatch.rs:4716`（`copy_readonly_project_with_limits`）
- **问题**：代码中读取文件内容为：
  ```rust
  let bytes = fs::read(&source_file)
      .map_err(|error| format!("读取项目副本文件失败 {}：{error}", source_file.display()))?;
  ```
  使用了 `?` 向上抛出错误。若项目中存在无读权限文件，或在 Windows 环境下文件被杀毒软件/IDE/外部进程排他锁定（Sharing Violation），或在元数据检查与读取之间文件被并发删除（TOCTOU），`prepare_outbound` 会直接返回硬 `Err`，使整个节点的派发阶段彻底失败。这与代码末尾提示语「已跳过 X 个不可读取或非普通文件」的设计意图直接冲突。
- **复现**：在测试项目中放入一个无读取权限的 tracked 文件或在 Windows 下以独占句柄锁定的文件，触发只读任务盲审，盲审节点直接报错中断。
- **改法**：将 `fs::read(&source_file)` 改为容错的 `match` 结构；读取失败时，累计 `unsupported += 1` 并 `continue`，将信息汇总到 `summary.incomplete`，保障主流程继续执行。

#### 3. [主要] `git ls-files` 超出输出缓冲区直接硬报错，大项目无法获得盲审副本
- **文件与大致位置**：`src/dispatch.rs:4638-4641`（`readonly_project_paths`）
- **问题**：在大中型 git 仓库（如数万文件）中，`git ls-files -z` 输出会超出进程缓冲上限触发 `listed.truncated == true`。当前实现直接返回 `Err("读取版本库文件清单失败：git ls-files 输出被截断".into())`，导致整个盲审阶段直接失败。这使得任务书设立的「5000 个文件上限、不完整时在 prompt 提示」的软截停机制在大仓库下完全失效。
- **复现**：在包含数万个 tracked 文件的仓库中触发只读问答盲审，`readonly_project_paths` 因缓冲区截断直接返回 Err。
- **改法**：若检测到 `listed.truncated`，不应直接作为硬错误打断流程，可截取最后一个 `\0` 之前的有效路径，并将“文件清单被截断”记录到 `summary.incomplete` 中降级处理。

#### 4. [次要] 原有 TASK-28 回归测试被就地篡改覆盖
- **文件与大致位置**：`src/dispatch.rs:8130`
- **问题**：任务书要求「必须走真实接线：**参照** `task28_readonly_answer_reaches_blind_review_through_dispatch_stage_core`」，但 diff 直接将原本用于保护 TASK-28 `ANSWER.md` 的端到端回归测试就地重命名为 `task32_readonly_project_copy_reaches_blind_review_through_dispatch_stage_core`，抹掉了原有的独立回归用例。
- **复现**：检查 `src/dispatch.rs` 的 test 模块，原 `task28_readonly_answer_reaches_blind_review_through_dispatch_stage_core` 标识已丢失。
- **改法**：保留原 TASK-28 测试函数不变，单独新立 `task32_readonly_project_copy_reaches_blind_review_through_dispatch_stage_core`。

---

### 核过没问题的要点

1. **盲审边界与敏感信息排除**：
   - git 项目使用 `git ls-files -z` 天然隔离了未跟踪的临时文件、构建产物与运行时留痕；
   - `excluded_project_copy_path` 显式排除了 `.dispatch` 与 `.git`，即便是版本库跟踪的 `.dispatch` 文件也不会进入副本；
   - `secret_project_file` 覆盖了 `.env`、`.env.*`、`*.pem`、`*.key`、`id_rsa*` 等密钥文件名，且大小写不敏感；即便被 git 跟踪，也会在写入前被正确过滤（测试已覆盖跟踪状态下的 `.env`）；
   - 非 git 项目复用了既有的快照目录排除规则（`existing_project_files`）。
2. **路径安全与防伪造**：
   - 路径逐一走 `safe_relative_path`，严格拦截 `..`、根路径及 Windows 盘符；
   - 使用 `fs::symlink_metadata` 配合 `is_link_or_junction`，符号链接与 Windows junction 均不跟随；
   - 对来自上游的 `worktree` 路径先通过 `fs::canonicalize` 规范化，再严格与 `registered_worktrees`（`git worktree list --porcelain`）比对，杜绝伪造外部任意目录读取。
3. **文本脱敏与二进制保真**：
   - 文本内容接入 `redact_text`，脱敏开关与全局口径一致，脱敏计数计入派发统计；
   - 非 UTF-8 二进制文件未被强制按文本解析，保留原始字节原样复制。
4. **diff 非空时行为不变**：
   - 逻辑严格收敛在 `readonly_answer_material`（diff 与 changed_files 均为空）路径下；diff 非空时绝不创建 `project/`，prompt 也不加指路语，反例测试验证充分。
5. **先判后读与上限控制机制**：
   - 单文件 1 MB 与总量 50 MB 均在 `fs::read` 之前先基于元数据（`metadata.len()`）预判，杜绝内存瞬时爆满；
   - 数量超 5000 或总量超 50 MB 停止复制，跳过大文件与截停均会在 prompt 明示「副本不完整：…」，未发生静默吞并。
6. **副本清理与生命周期**：
   - 副本仅写入盲审工作区 `workdir/project/`，不向项目根目录反向写回；备选重跑与结束清理盲审工作区时随目录整体删除。
