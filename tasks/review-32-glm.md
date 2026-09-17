# TASK-32 首版审查 · GLM-5.3-Flash（cc-switch，只读）

# TASK-32 首版审查（只读，基于当前树逐行核对）

**总判：可交付但有未闭合项** —— 任务书各条均真实落地，主链路测试走 `dispatch_stage_core` 真实接线；遗留 1 个大仓库截断悬崖（ls-files 4MB 上限导致整节点硬失败）和几个低severity 边角需要裁决，无阻塞性缺陷。

限制：本会话无 shell，无法跑 `git diff` / `cargo test` 复核报告中的验证数字与 EOL 声明，以下基于源码现状阅读；报告中「479×3 通过、红测 7 failed」等结论按报告采信、标注未复核。

---

## 问题清单（按严重度）

**1.【中低】大仓库：`git ls-files` 输出超 4MB 截断 → 整个盲审节点硬失败**
- 位置：`src/dispatch.rs:4643-4647`（`listed.truncated` → `Err`）＋ `src/process.rs:10`（`MAX_PROCESS_OUTPUT = 4 MB`）。
- 问题：`git ls-files -z` 走 `run_process_registered`，stdout 上限 4MB。被跟踪文件路径文本总量超 4MB（约 5–8 万个文件，按平均路径 50–70 字节估）时 `truncated=true`，`readonly_project_paths` 返回 Err，`prepare_outbound` 整体失败——只读任务的盲审节点从「缺副本」变成「跑不了」。任务书点名了大项目性能，实现把超时处理对了（节点 timeout＋取消键＋`timed_out` 显式报错，`dispatch.rs:4620-4631`），但截断被当成致命错误而非降级。
- 复现：临时 git 仓生成 >4MB 的 NUL 拼接路径清单（约 6 万个深路径文件），跑只读盲验收 → 节点 error「读取版本库文件清单失败：git ls-files 输出被截断」。
- 改法（任选其一，需裁决）：截断时降级为 `existing_project_files`（带快照排除）＋「副本不完整」标注；或对该次调用放宽输出上限。不改也说得通（fail-loudly 优于静默缺料），但应写进报告/文档作为已知悬崖。

**2.【低】git 路径的固定排除清单与非 git 路径不对称：缺 `.aicross-runtime`**
- 位置：`src/dispatch.rs:4553-4557`（`excluded_project_copy_path` 只匹配 `.dispatch`/`.git`）；对照 `src/dispatch.rs:430-447`（`excluded_snapshot_directory` 含 `.aicross-runtime`）。
- 问题：非 git 副本沿用快照排除（含 `.aicross-runtime`）；git 副本靠「仅跟踪文件」过滤，若仓库里有被提交的 `.aicross-runtime` 文件（历史误提交），会被带进盲目录。运行时写进项目的常规留痕（`.dispatch/run-*/node.before` 快照，`dispatch.rs:4104`、测试 7572 佐证）已被组件名排除覆盖，即使被跟踪也进不来；`.aicross-runtime` 目前全仓只有排除表和测试引用，生产不写入——所以这是理论边角＋一致性问题。
- 改法：`matches!(part, ".dispatch" | ".git" | ".aicross-runtime")`，一行。

**3.【低】git 模式下单文件读/写失败是硬错误，会杀掉整个只读盲审节点**
- 位置：`src/dispatch.rs:4715-4716`（`fs::read` 失败 → Err）、`4728-4741`（目录创建/写入失败 → Err）。
- 问题：跨平台仓库里被跟踪的 Windows 保留名文件（`aux.py`、`nul.txt`、`con.c` 等）或权限异常文件，读取/创建失败时直接 Err，节点失败；而同类「跳不过去」的文件（超限、链接、非普通文件）都是跳过＋不完整标注。两种失败待遇不一致。触发前提是仓库里真有此类名字——低概率。
- 不确定点：Win11 默认是否仍拒绝创建保留名设备文件（历史上默认拒绝，个别版本可配置放行）；无法在本会话实测。
- 改法：单文件级 IO 失败改为 `unsupported += 1`＋不完整标注；仅副本根目录创建失败保持致命。

**4.【低・不确定可利用性】git 模式中间路径组件若为 junction 可绕过路径排除（理论）**
- 位置：`src/dispatch.rs:4695-4706`——`fs::symlink_metadata(source.join(relative))` 只检查末组件；若磁盘上 `relative` 的某级父目录是 junction（如 `docs` → `project/.dispatch`），读取会穿透，且路径字符串里不含 `.dispatch` 组件，`excluded_project_copy_path` 拦不住。
- 前提链条不确定：需要执行者建 junction 且 `git add` 把 junction 下的文件提交进索引（Git for Windows 对 junction 的遍历/提交行为我没查证）；git 索引正常情况下不会含符号链接下的路径。非 git 路径无此问题（walk 逐级 `symlink_metadata` 跳链接，`dispatch.rs:667-674`）。
- 改法（廉价加固，一并消掉此类）：复制前对 `source_file` 做 `fs::canonicalize` 并校验结果以 canonicalize 后的 source 为前缀，否则按 unsupported 跳过。

**5.【备注】非 git 大目录：walk 无取消点、无上限，先全量收集再复制**
- 位置：`src/dispatch.rs:4679`（`readonly_project_paths(...)?` 先物化整个 Vec）→ `existing_project_files`（`dispatch.rs:660-692`）一次性递归全树，循环内不查 `is_cancelled`。5000/50MB 截停只约束复制阶段；非 git 十万级文件树会有秒级遍历时延，且取消要等 walk 结束才生效。复用既有函数是任务书要求（「不要另写一套」），属可接受代价，记录事实。

**6.【备注】无 git PATH 的机器：只读盲审从「仅 ANSWER.md 可用」变为硬失败**
- 位置：`src/dispatch.rs:4620-4626`——`git rev-parse` spawn 失败被 `map_err` 成 Err。改动前该场景不调 git。产品本身全链路依赖 git（测试也有 `require_git` 门），实际影响趋近于零；按事实记录。

**7.【备注】上限提示文案里「1 MB」为硬编码**
- 位置：`src/dispatch.rs:4747`（`format!("已跳过 {oversized} 个超过 1 MB 的文件")`）。生产只传默认 1MB（`4470` 常量），测试传自定义 `file_limit=16` 时因文件只有 2 字节不会触达该分支——当前无错报路径，仅在未来复用参数化入口时会失真。

---

## 核过没问题的要点

1. **触发条件与 ANSWER.md 严格同源**：副本只在 `readonly_answer_material` 返回 Some 的分支里（`dispatch.rs:4845-4872`）；该函数对任一产物 diff 非空或 changed_files 非空即返回 None（`4503-4510`），上游为空时整个盲材料块不进。反例测试 `task28_nonempty_diff_does_not_write_answer` 已扩展断言：无 `project/` 目录、prompt 无指路句（`8283-8285`）。
2. **上限顺序正确（先判后读＋读后复核）**：`symlink_metadata.len()` 先判跳过（`4707`）→ `fs::read` → 读后字节数复核文件限与总量限（`4717, 4721`），超限在**写入前** break，落盘总量不可能超 50MB；文件数限在复制前判（`4691`），最多 5000 个。「已跳过 / 已停止复制」措辞与实际行为相符；不完整提示紧跟指路句、格式「副本不完整：…」（`4864-4867`）。测试 `8325-8390` 用缩小阈值钉住三种截停。
3. **密钥排除完整且覆盖被跟踪密钥**：`secret_project_file` 在循环层生效（`4688`），git / 非 git 都过；主接线测试把 `.env` **git add 进提交**（`8146-8151`），断言副本无 `.env`（`8227`）——「跟踪的密钥也排除」被真实钉住。清单含 `.env`/`.env.*`/`*.pem/.key/.p12/.pfx/.jks`/`id_rsa*`/`id_dsa*`/`id_ecdsa*`/`id_ed25519*`，小写化匹配。仓内确无既有密钥清单可复用（全 src 搜 `.pem|id_rsa|.env` 仅此一处＋测试），报告说法属实。`.dispatch` 被**跟踪**的文件同样被组件排除且测试钉住（`8143-8151, 8228`）。
4. **worktree 来源不可被执行者伪造**：`artifact.worktree` 由编排者在 `run_one_attempt` 自录（`dispatch.rs:3799` 建、`4118` 写入），不来自执行者输出；`readonly_project_source`（`4577-4613`）要求全部产物一致带 worktree、canonicalize 后去重到 1 个、再对项目跑 `git worktree list --porcelain` 核实登记（`903-918`），混合来源/多 worktree/未登记一律拒绝而非猜测。测试 `8452-8489` 用真 `git worktree add`＋真登记核对，断言副本取 worktree 内容而非主目录。
5. **链接/路径安全**：`safe_relative_path` 拒 Prefix（盘符/UNC）/RootDir/ParentDir/CurDir（`2380-2385`）；末组件 junction（`0x400`）与 symlink 不跟随（`4696-4706`、`2240-2249`）。ls-files 用 `-z`（无 quotepath 转义问题）。中间组件链接的残余面见问题 4。
6. **脱敏与二进制口径同盲审附件**：UTF-8 → `redact_text`（开关同 `settings.redact`，命中并入派发计数），解码失败 → 原字节；测试 `8393-8420` 验证密钥样式被替换且 `[0xff,0x00,0x81]` 原样。与 `REQUEST.md` 口径一致，报告描述与代码相符。
7. **重名保护**：`project` 经 names → reserved（trim 尾斜杠，`4859, 4874-4877`），附件名 `project` 报错文案精确（测试 `8422-8449`）；与 TASK-28 的 `ANSWER.md`/`changed` 同机制。
8. **清理**：副本只写盲目录内 `project/`，不回写项目；备选重跑前 `run_fallback_attempt` 对整个盲目录 `remove_dir_all`（`dispatch.rs:3601-3608`），`fallback_blind_attempt_gets_fresh_material_dir` 已扩展为真接线验证（`run_one`，上游只读产物，断言第二轮 `project/m.txt` 重新物化，`9076-9117`）。
9. **测试真实性与可红性**：主测试经 `dispatch_stage_core` 跑「只读执行（python stub）→ 盲验收」完整两阶段（`8132-8238`），断言 `project/src/beta.py` 内容一致、未跟踪/.env/.dispatch 不在、prompt 含指路句与 ANSWER 全文；其余 6 条走真函数（`prepare_outbound` / `copy_readonly_project_with_limits`）。逐条看，断言全部依赖实现产出的文件/报错/文案存在，去掉实现必然红——红测结论方向上成立；报告所述「临时注入 `AICROSS_TEST_DISABLE_PROJECT_COPY`」的执行过程本身无法从当前树核实（该环境变量现已不在代码中，与「临时改后恢复」的说法一致，标不确定）。
10. **「不做」清单遵守**：git status 仅 `src/dispatch.rs`＋`tasks/REPORT-32.md`，未动 src-ui、协议字段、修正轮、验收解析、ANSWER.md 既有内容；`readonly_answer_material` 的 TASK-28 逻辑（分段、截断标注、警告剥离）未被改动。报告中的 cargo/vitest/build 数字与 LF 声明未复核（无 shell），按报告采信。
