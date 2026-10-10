# REPORT fix-fullcheck-engine（10-08 全面检查，引擎）

12 条全部核实成立并改了；复审（见文末「第二轮」）指出第一轮的跨进程锁协议有竞态、安装残留只凭名称前缀删除、收尾句去重会改动材料，第二轮已修正。第一轮验证结果：cargo check --all-targets / --release 零 warning；cargo test 692 passed，0 failed，10 ignored。未提交，未加依赖，未跑真实 agent CLI。

## 没做完、没验证或与要求不符（先看这里）

- 测试临时目录曾写到 `D:\aicross-data\tests\<pid>\`：本机 shell 设了 `AICROSS_HOME`，`paths::test_temp_dir()` 按设计把测试临时目录放在其下（10-03 起的约定）。本次新测试留下的子目录已删；其余 `<pid>` 目录属既有测试，下一次运行按「超过一小时」自动清。最终验证在 `AICROSS_HOME` 未设下跑。
- A6 跨卷没有端到端复现（测试机只有一个卷）：测试钉住「暂存目录按解析后的真实路径建」「经 junction 新装与更新都成功、两处无残留」；真正跨卷时旧实现换名失败只有代码证据（旧 `write_bundle` 用字面 `skills_dir.parent()`）。
- A2 文件 symlink 用例在本机跳过（无开发者模式、无权限建文件 symlink）；目录 junction 与悬空 junction 两条真跑。
- 先红后绿只对部分条目实测：A2 两条 junction 用例改前失败已观察；B6 改前 `parse_codex_cache` 返回 Err 已观察；其余条目「改前会红」的依据是代码路径，没有回退代码逐条实跑。
- A6 用例前期曾假绿（路径正斜杠让 `mklink` 失败而走了跳过分支），已改为 `join(".agents").join("skills")`，现在真跑通过。
- A4 并发测试依赖 300 ms 等待；机器极慢时可能没测到竞态而误绿，不会误红。
- B1 没有真开第二个进程：用「冒烟回调里直接写库」与「测试线程直接持有锁文件」模拟。
- （第二轮更正）`src/main.rs` 在 HEAD 与工作树都是 CRLF（280 / 280 个换行为 CRLF，`git ls-files --eol` 为 `i/crlf w/crlf`）；第一轮写成「实际为 LF」是错的——当时用 Git Bash 的 `grep $'\r'` 检查，该工具读文本时吃掉了 CR。第一轮对它的改动（加一行 `mod file_lock;`）保持了 CRLF。`src/server.rs` 未改。
- （第二轮已替换）第一轮的跨进程锁是自写锁文件协议，过期接管与释放有竞态；第二轮改为系统文件锁，见「第二轮」第 1 条。
- `skill status` 不再清理残留：早期版本留在 skills 目录里的临时目录要等下一次安装或卸载才清。

## 逐条

### A1（P1）查询状态会删安装备份 —— 成立
- 证据：`host_status_at` 对每个读取位置调 `sweep_temp_dirs` 按前缀删除；`status_at` 与窗口 `skill_status` 都走它；只有进程内 Mutex。
- 改法（`src/skill_install.rs`，新增 `src/file_lock.rs`）：status 只读、不拿锁、不清理；新增跨进程锁 `file_lock::FileLock`（`create_new` 锁文件、令牌比对释放、过期接管、等待超时，不加依赖）；安装 / 更新 / 卸载另拿暂存目录里的 `install.lock`（过期 30 秒、等锁 40 秒）；拿锁后 `recover_leftovers`：安装 / 更新时目标缺失而有带标记的备份就先恢复最新一份（失败则报错、备份保留），其余临时 / 备份目录删除（新旧两种位置都认），卸载不恢复直接清；换名失败且备份改不回时保留备份并在错误里写明下次安装自动恢复。
- 测试：`a1_status_is_read_only_and_install_cleans_leftovers`（替换断言了缺陷行为的旧测试 `status_sweeps_leftover_temp_and_backup_dirs`）、`a1_interrupted_update_backup_is_restored_before_install`、`a1_install_waits_for_cross_process_lock`、`file_lock_excludes_second_holder_until_released`、`file_lock_takes_over_stale_lock_and_drop_keeps_foreign_lock`。

### A2（P1）斜杠命令沿链接写删外部文件 —— 成立
- 证据：`file_state` 用跟随链接的 `metadata`；kimi 的 `~/.kimi-code/skills/<名>/` 是指向外部带标记 `SKILL.md` 的 junction 时判为「已写入」。改前两条新测试实测失败。
- 改法（`src/slash.rs`）：`file_state` 作为写删共用的归属检查——命令目录或文件本身是 symlink / junction（含悬空）、元数据读不到（NotFound 除外）、不是普通文件，一律「被占用」；`write_command_file` 写前再过一次；kimi 的 `~/.agents/skills/<名>` 改用 `symlink_metadata` 判存在。list、add、add-all、remove 与 preset remove 级联删除都经它。更上层祖先目录不查。
- 测试：`a2_kimi_linked_command_dir_is_external`、`a2_dangling_command_dir_link_is_occupied`、`a2_symlinked_command_file_is_external`（本机跳过）。

### A4（P2）预设在锁外读取 —— 成立
- 证据：`config_set_seat_at`、`config_clear_seat_at`、`config_set_count_at`、`config_set_focus_at`、`config_set_default_mode_at`、`recommend_at_home` 都在拿 `modes_write_lock` 前读 presets.json，而预设保存 / 删除在同一把锁里写。
- 改法：新增 `console_cli::find_preset`，六处都在锁内重读预设并据此算形状、校验、写入（set-seat、set-count 锁外先查一次只为保持报错顺序）；clear-seat 整段在锁内。
- 测试：`a4_set_seat_rechecks_preset_inside_lock`。

### A5（P2）推荐不采用的非法角色默认仍会生效 —— 成立
- 证据：推荐写 `None` 并注明「未采用」，但 `engine::fill_seats` 把 `None` 解释为用角色默认，验收位最终仍是 agy。
- 改法：新增 `agent_catalog::position_violation`（`seat_violation` + 只看得到文字的 agent 不填验收位），推荐与补人共用；`fill_seats` 用角色默认补位前按它校验，不合规不补、来源「未指定」；新增 `fill_seats_explained` 给出原因：派发 `plan_dispatch` 经 `fill_seats`，`config show` 节点新增 `unfilledReason`，推荐把原因写进 notes。推荐自身的规则（厂商未知、未实测）不是派发硬规则，派发时角色默认照补，notes 改为「使用角色默认 X（不在推荐范围：原因）」。
- 测试：`a5_illegal_role_default_not_used_after_recommend`（推荐 → 保存 → config show 未指定并给原因 → plan_dispatch 验收节点没有 agy）；按新行为更新 `task61_r1_set_role_rejects_blind_only_agent_and_show_marks_invalid`。

### A6（P2）跨卷暂存 —— 成立（只有代码证据）
- 改法：`staging_dir` 先用 `resolve_existing` 把 skills 目录解析成真实路径；暂存目录建在真实 skills 目录的上一级 `.aicross-install-<skills 目录名>/`（真实目录已是卷根时退到其内）；临时与备份放在暂存目录下一层 `ai-cross/`，宿主扫描不到；换名一律用真实路径。
- 测试：`a6_staging_follows_redirected_skills_dir`（本机真跑）。

### B1（P1）资源库跨进程写入丢更新 —— 成立
- 证据：`store_lock` 只是进程内 Mutex；冒烟在锁内跑最长 180 秒后整库写回。
- 改法（`src/resources.rs`）：`store_lock(path)` 先拿 `resources.json.lock`（FileLock，过期 30 秒、等锁 40 秒）再拿进程内互斥，全部调用点传路径；单模型冒烟拆出 `smoke_resource_model_with`：锁内取目标即放锁，冒烟在锁外，写回时重新加锁重读、只合并目标一条；按 agent 冒烟同理；探测合并前的 cc-switch 提示（拉 Python、最长 20 秒）移到锁外；首次验证的两次只读读取拿不到锁也照读（原子写保证整份）；删除只被忽略的真机测试使用的 `smoke_one_in_place`（在该测试里就地展开）。
- 测试：`fullcheck_b1_smoke_write_back_keeps_interleaved_writes`、`fullcheck_b1_store_writes_wait_for_cross_process_lock`。

### B2（P1）验证失败的模型仍能经后续阶段或备选派出 —— 成立
- 证据：`plan_dispatch` 只对第一阶段查 `failed`；`verify_on_first_use_at` 对 `failed` 返回 `Skipped` 后照常进通道；备选循环只查登录与额度。
- 改法：新增 `resources::dispatch_block_reason`（未发现或验证失败返回原因，timeout 放行）；派发共同入口 `run_one_with_activity` 对主目标按 provider 精确查找，被拦不进通道（`run_one_attempt` 新增 `blocked` 参数），显式备选与资源库有序备选被拦则跳过并写进 runner 说明；`verify_on_first_use_at` 新增 `FirstUseVerification::Rejected`；`plan_dispatch` 的验证失败检查扩到本次新建的全部节点。
- 测试：`fullcheck_first_use_rejects_failed_and_missing_by_exact_provider`、`fullcheck_b2_later_stage_failed_target_is_rejected`、`fullcheck_b2_failed_models_never_enter_channel`；按新行为更新 `task09_first_use_failure_persists_and_does_not_retry`（第二次期望 `Rejected`）。

### B3（P1）资源库 `fallback_order` 备选没查「未发现」 —— 成立
- 改法：与 B2 共用候选拦截 `resource_block`，未发现的有序备选跳过并记原因。
- 测试：`fullcheck_b3_missing_resource_fallback_is_skipped`。

### B4（P2）验证失败检查不匹配 provider —— 成立
- 改法：改用 `resources::find_target_model`（provider 精确匹配；provider 为空时优先非未发现、非验证失败的那条）；首次验证同名多 provider 用同一优先序；额度恢复说明改用 `find_model_two_level`。
- 测试：`fullcheck_b4_failed_check_matches_provider_in_both_orders`（另有 resources 侧用例覆盖两种顺序）。

### B5（P2）如实汇报后缀不在最终外发文本里 —— 成立
- 证据：续接且 `current_message` 非空时外发当前消息不含后缀；盲审材料说明、diff、agy 内联材料拼在后缀之后。
- 改法：新增 `dispatch::with_closing`，在定下最终外发文本后去掉文中已有的如实汇报句（agy 另含「不要使用任何工具」句）再在末尾追加一次；用于完整 prompt（一次性通道、ACP、ACP 回退的完整上下文共用）与续接的 `current_message`。引擎在 node.prompt 末尾的追加保留，金标准请求结构不变；`HONEST_REPORTING` 改为 `pub(crate)`。
- 测试：`fullcheck_b5_final_outbound_ends_with_honest_reporting_once`（断言假通道实际读到的 task-file）、`fullcheck_b5_agy_closing_follows_inlined_materials`；按新行为更新 3 条 ACP 测试的期望外发文本。

### B6（P2）Codex 模型全部隐藏时旧条目仍可选 —— 成立（改前实测失败）
- 改法：`parse_codex_cache` 区分解析失败 / 无任何带 slug 的模型（仍报错、旧条目不动）与全部隐藏（返回空清单，合并时旧条目标未发现）。
- 测试：`fullcheck_b6_codex_cache_all_hidden_marks_old_models_missing`。

### 12 盲审 / 评审单独派发用错模板 —— 成立
- 证据：金标准 `blind-three-way` 第一阶段三个审查位的 prompt 都带两行固定关注点；验收位用「对照原始需求，是否完整实现」模板而父节点只有材料。
- 改法（`src/engine/mod.rs`）：新增 `has_upstream_product`（直接父节点里有模型节点才算）；没有上游产物时盲前缀去掉固定关注点，验收位改用新模板 `fixtures/prompts/independent-review.txt`（仍以 PASS / NEEDS_FIX 收尾，解析不变）；评审视角句照常；有上游产物时文本逐字不变。
- 测试：`walkthrough12_standalone_reviewers_get_independent_template`；金标准 `fixtures/engine-golden/blind-three-way.json` 只改第一阶段 3 个节点的 `prompt`，其余 30 个夹具不变。

## 不改
- Kimi 一次性通道 prompt 走 argv：未动。
- skill 读 aicross 配置（A3）：未动内置包。

## 文档
`docs/CONSOLE-CHANNEL.md`：如实汇报的追加位置与审查模板、派发层拦截、资源库跨进程锁与 codex 全隐藏、角色默认不合规不补与 `unfilledReason`、写命令锁内重读预设、skill 安装暂存位置 / 跨进程锁 / 中断恢复 / status 只读、斜杠命令链接判定。

## 验证（`AICROSS_HOME` 未设）
```
cargo check --all-targets   # 0 warning
cargo check --release       # 0 warning
cargo test                  # 692 passed，0 failed，10 ignored
```
派发相关测试用 python 假 cc-switch 脚本与仓库自带的 ACP 假服务。

## 改动文件
修改：`src/skill_install.rs`、`src/slash.rs`、`src/console_cli.rs`、`src/recommend.rs`、`src/agent_catalog.rs`、`src/engine/mod.rs`、`src/engine/tests.rs`、`src/resources.rs`、`src/dispatch.rs`、`src/main.rs`、`docs/CONSOLE-CHANNEL.md`、`fixtures/engine-golden/blind-three-way.json`；新增：`src/file_lock.rs`、`fixtures/prompts/independent-review.txt`。

## 第二轮

来源：`tasks/fix-fullcheck-engine-r2.md`（Codex 复审「修后合并」）。

### 没做完、没改或与要求不符（先看这里）

- **第 3 条（Kimi 截断掉收尾句）没复现，没改裁剪逻辑**。`crop_utf8_bytes` 保留开头 60% 与结尾 40%（约 1.2 万字节），而收尾句由 `with_closing` 放在最终文本最末尾，裁剪后仍在。新测试 `channels::tests::kimi_crop_keeps_honest_closing_at_end` 先在未改裁剪代码时跑：中文 2 万字与 ASCII 6 万字两种正文，实际 invocation 文本都以收尾句结尾、不超过 30000 字节、收尾句只出现一次，直接通过。按「核实不成立不改」只保留这条测试作回归保护。若复审指的是别的路径（例如收尾句不在末尾的文本），需要给出具体输入再看。
- 第 2 条的链接用例依赖能建目录 junction；本机能建，真跑通过。建不了的平台只跳过链接部分，其余断言照跑。
- 第 2、4 条的新测试与修复同轮写成，没有回退代码实跑旧版本。「改前会红」的依据：第一轮按前缀删除、对链接备份照样 `rename`；`with_closing` 对全文 `replace`，会删掉材料里引用的收尾句。
- B1 / A1 的跨进程测试仍是同进程模拟：测试线程用另一个文件句柄持锁，系统文件锁对不同句柄互斥，没有拉起第二个进程。

### 1（P1）自写锁文件不能保证互斥 —— 成立，已替换

改法：
- `src/file_lock.rs` 改用 `std::fs::File::try_lock()`（Windows 上是 `LockFileEx`，Unix 上是 `flock`）轮询加超时。锁随句柄释放，进程崩溃由系统释放。
- 锁文件常驻、不删除。
- 删掉过期接管、令牌比对与 `remove_stale`，以及常量 `STORE_LOCK_STALE`、`INSTALL_LOCK_STALE`。
- `FileLock::acquire(path, timeout)` 只剩两个参数，资源库锁（`resources.json.lock`）与 skill 安装锁（暂存目录里的 `install.lock`）都用它。
- 安装后不再删除「空」暂存目录，因为锁文件常驻：每个 skills 目录的上一级常驻一个 `.aicross-install-<名>/install.lock`。
- 复审第 3 条（移不走的过期锁导致无限循环）随过期逻辑一并消失。

测试：
- 删除：`file_lock_excludes_second_holder_until_released`、`file_lock_takes_over_stale_lock_and_drop_keeps_foreign_lock`。
- 新增：`file_lock::tests::file_lock_two_holders_are_exclusive_and_wait_times_out`（两个持有者互斥，超时返回错误）、`file_lock::tests::file_lock_released_when_holder_handle_closes`（持有者句柄关闭后，等待方立即拿到；锁文件常驻）。
- 更新：`a1_install_waits_for_cross_process_lock`、`fullcheck_b1_store_writes_wait_for_cross_process_lock` 改用新签名；`install_creates_updates_and_refuses_external`、`a1_*`、`a6_*` 的残留断言改为「暂存目录里只剩锁文件」。

### 2（P1）安装残留仅凭名称前缀删除、恢复未排除链接 —— 成立

改法（`src/skill_install.rs`）：
- `write_bundle` 在临时 / 备份根目录里先写操作记录 `.aicross-op.json`（目标真实路径与操作令牌），再放内容。
- 新增 `leftover_owner_check`，只有满足下列条件的残留才清理或恢复：
  - 残留根目录或内容目录是 symlink / junction 的，一律保留；
  - 新格式要有操作记录，且记录的目标与本次真实目标一致；
  - 旧格式（skills 目录与其上一级）要带我们的安装标记。
- 不归属的保留，原因收进 `recover_leftovers` 的返回值；`install_at` / `uninstall_at` 的输出带 `notes`，没有时不写出该字段。
- `recover_leftovers` 改为同时接收真实目标与字面目标。

测试：
- 新增 `skill_install::tests::r2_unowned_and_linked_leftovers_are_kept`：无标记的旧位置同名前缀目录、没有操作记录的、记录目标不符的、内容目录是 junction 指向外部带标记目录的，四种都保留不删、不恢复；目标缺失时装上的是内置版本；输出带 `notes`。
- 更新 `a1_status_is_read_only_and_install_cleans_leftovers`、`a1_interrupted_update_backup_is_restored_before_install`：给作为「本次残留」的目录补上操作记录或安装标记。

### 3（P2）Kimi 长提示词截断掉如实汇报后缀 —— 未复现，未改（证据见上）

新增测试：`channels::tests::kimi_crop_keeps_honest_closing_at_end`。

### 4（P2）后缀去重篡改原始材料 —— 成立

改法（`src/dispatch.rs`）：
- `with_closing` 不再对全文 `replace`。文本已以收尾句结尾就原样返回，否则在末尾追加一次。
- 新增 `strip_engine_closing`：在 `run_one_attempt` 铺材料前，只摘掉引擎拼在 `node.prompt` 末尾的那一段（`"\n\n" + 收尾句`，agy 是两句）。
- 用户原文、diff、内联源码里引用的同样文字一个字不动。

测试：
- 新增 `dispatch::tests::r2_closing_does_not_rewrite_quoted_material`。
- 更新 `fullcheck_b5_agy_closing_follows_inlined_materials`：先摘再追加。
- `fullcheck_b5_final_outbound_ends_with_honest_reporting_once` 不改，照常通过。

### 5（P2）报告事实错误 —— 成立，已更正

开头摘要与「先看这里」第 `main.rs` 一条已就地更正：`main.rs` 为 CRLF；第一轮的锁协议与残留清理有缺口，在本节修正。复核：`src/main.rs` 280/280、`src/server.rs` 680/680 个换行为 CRLF；按字节计数，不用 Git Bash 的 grep。

### 文档

`docs/CONSOLE-CHANNEL.md` 更新了三处：
- 收尾句的处理方式与 Kimi 裁剪；
- 资源库锁改为系统文件锁；
- skill 安装的系统文件锁、操作记录、残留归属判定与 `notes`。

### 验证（worktree 根目录，`AICROSS_HOME` 未设）

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # Finished，0 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release       # Finished，0 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo test                  # 695 passed，0 failed，10 ignored
```

未提交，未加依赖，未运行真实 agent CLI。

第二轮改动文件：`src/file_lock.rs`、`src/resources.rs`、`src/skill_install.rs`、`src/dispatch.rs`、`src/channels/mod.rs`（只加测试）、`docs/CONSOLE-CHANNEL.md`、`tasks/REPORT-fullcheck-engine.md`。

## 第三轮

来源：第二轮复审（Codex）无 P0/P1，一条 P2。

- **问题（成立）**：跨 agent 切备选时，`run_fallback_attempt` 先换了 `agent` 再交给 `run_one_attempt`，后者按备选的 agent 摘收尾，摘不掉主目标的那一段。agy 主目标的收尾是两句，备选会多出一句如实汇报，并残留「不要使用任何工具」；反方向同理。
- **改法**（`src/dispatch.rs`）：`run_fallback_attempt` 在换 agent 之前，按主目标的 agent 调 `strip_engine_closing` 摘掉收尾；最终外发文本仍由 `with_closing` 按备选的 agent 追加一次。
- **新测试**：`dispatch::tests::r3_cross_agent_fallback_swaps_closing`。
  - agy 主目标在资源库里标为验证失败、不进通道，切到假 cc-switch 备选，断言实际外发的 task-file 等于「原材料 + 如实汇报句」，且只有一次。
  - 材料里引用的两句原样保留。
  - 非 agy → agy 方向不能真跑 agy，在函数层按同一步骤断言。
  - 临时去掉修复行后这条测试失败，恢复后通过。
- **验证**（`AICROSS_HOME` 未设）：`cargo check --all-targets` 0 warning；`cargo test` 696 passed，0 failed，10 ignored。
