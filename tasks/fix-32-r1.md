# fix-32-r1 只读问题盲审附项目副本 · 复审修正轮

依据 GLM-5.3-Flash（可交付但有未闭合项）与 Antigravity/Gemini 3.8 Flash（判不可交付）的审查，编排者逐条对照代码核过。禁区与 TASK-32 任务书相同。

## 编排者核过「不成立」的，不要改

- **Gemini 1「`project` 未并入附件保留名」不成立**：保留名由 `names` 去尾斜杠生成（`prepare_outbound` 约 4874 行），`project/` 在 `names` 里，`task32_project_attachment_name_conflict_is_rejected` 实际通过（编排者全量连跑 3 次 479 全过）。

## 必修

1. **`git ls-files` 输出被截断时整个盲审节点硬失败（两家共同）**
   进程输出上限（`src/process.rs` 的输出上限常量）在大仓库会被 `ls-files -z` 撑满，现在直接 `Err`，只读任务的盲审从「缺副本」变成「跑不了」。本来就有 5000 个文件上限。
   修法：截断时只取最后一个 `\0` 之前的完整路径继续复制，并在「副本不完整：…」里写明「版本库文件清单过长，只取了前 N 个」。补测试（可用 `#[cfg(test)]` 的截断注入，参照本文件已有的 `SWARM_TEST_TRUNCATE_NEXT_CHANGED_STATUS` 做法）。

2. **单个文件读 / 写失败会杀掉整个盲审节点（两家共同）**
   `fs::read` 与副本内建目录 / 写文件失败都 `?` 上抛；被占用、无权限、Windows 保留名（`aux.py`、`nul.txt` 等）都会让节点失败，而超限、链接、非普通文件是跳过 + 标注，待遇不一致。
   修法：单文件级 IO 失败计入跳过（与「不可读取或非普通文件」同一计数）并进不完整标注；**只有副本根目录 `project/` 创建失败保持致命**。补测试：构造一个读失败的条目（例如目录冒充文件、或测试注入），断言节点不失败、提示里有跳过说明、其它文件照常复制。

3. **路径中间组件是链接 / junction 时可绕过排除（GLM 4）**
   现在只对末组件做 `symlink_metadata`；若某级父目录是 junction，读取会穿透到项目外或 `.dispatch` 里，路径字符串里又不含 `.dispatch`。
   修法：复制前对源文件 `fs::canonicalize`，校验结果以 canonicalize 后的副本来源根为前缀，否则按跳过处理。补测试（Windows 上用 junction 或 symlink，建不出来时跳过该测试并打印原因，参照本仓已有的测试跳过写法）。

4. **git 路径的固定排除缺 `.aicross-runtime`（GLM 2）**
   非 git 路径复用的快照排除里有它，git 路径的 `excluded_project_copy_path` 只认 `.dispatch` / `.git`。补上并在主测试里加一个被跟踪的 `.aicross-runtime/x` 断言不进副本。

5. **TASK-28 的端到端回归测试被就地改名（Gemini 4，已核实）**
   `task28_readonly_answer_reaches_blind_review_through_dispatch_stage_core` 在本分支被改名成 `task32_...`，原名不在了。恢复原 TASK-28 测试（内容与 main 上一致），TASK-32 的端到端测试另立。

6. **提示文案里「1 MB」写死（GLM 7）**：改为按 `file_limit` 生成。

## 保持，不改（写进报告即可）

- 非 git 大目录遍历无取消点、先全量收集（GLM 5）：任务书要求复用既有 `existing_project_files`，代价可接受。
- 机器上没有 git 时只读盲审失败（GLM 6）：产品全链路依赖 git，不处理。

## 验证

`cargo check --all-targets` 零 warning、`cargo test` 全量连跑 3 次、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。每条先确认去掉实现会红，在报告里写清怎么红的。

保持文件原有换行；不 `cargo fmt`；不起窗口、不发真机请求、不调真实模型；不 commit、不 push。报告写 `tasks/REPORT-32-R1.md`，逐条对应。
