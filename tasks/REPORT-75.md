# REPORT-75：派发不在各家 CLI 历史里留散乱条目

按 `tasks/TASK-75-blind-dirs-and-history.md` 分三个阶段实施与验证；按任务书要求未执行 `git commit` / `git push`，未改动版本号，未修改 `src-ui/`。

---

## 一、阶段 1：kimi 盲位置改用固定槽位目录

### 1. 改动内容

- **跨进程锁扩展**（`src/file_lock.rs`）：为 `FileLock` 增加非阻塞尝试加锁方法 `try_acquire`。
- **盲审固定槽位模块**（新建 `src/blind_slot.rs`，并在 `src/main.rs` 挂载）：
  - 定义 `SLOT_COUNT = 8`，`blind_slot_prefix`（阶段 1 为 `"kimi" => Some("kimi")`，阶段 2 扩展 `"agy" => Some("agy")`），工作目录位于 `<AICROSS_HOME>/blind/<prefix>-1..8`，锁文件 `<AICROSS_HOME>/blind/<prefix>-<n>.lock` 放在槽位目录外。
  - 提供 `BlindSlotGuard`、`acquire`、`acquire_with_cancel`、`clear_dir_contents`、`archive_and_release`：
    - 从 1 号起依次尝试 `try_acquire`；全被占用时按节点超时与叫停信号等待，超时失败并报「盲审目录都在使用中」。
    - 取到锁后先清空槽位残留（包含上次崩溃遗留），再放入本次节点材料；路径重定向写入槽位目录的 `KIMI_TASK.md`。
    - 进程树结束后（正常结束、失败、超时、叫停一致处理）先关闭占用该槽位的 ACP 客户端（`src/channels/acp.rs` 为 `ClientSlot` 记录 `cwd` 并提供 `close_by_cwd`），再将槽位全部内容归档迁移到 `runs/<派发>/<节点>`（遇 Windows 拒绝访问或共享冲突按 `paths::replace_file_with_retry` 办法退避重试，仍失败则改为复制后删除），清空槽位后再释放锁；归档失败只记入节点备注，不改节点状态。
- **通道与派发接线**（`src/dispatch.rs`、`src/channels/mod.rs`）：
  - `src/dispatch.rs` 对落入盲槽位的 `kimi` 在节点记录与 `status` 中写归档目录 `runs/<派发>/<节点>`，实际向 CLI 传递与路径改写用槽位路径。
  - `src/channels/mod.rs` 在盲位置不再对 `kimi` 传 `-S` / 续接会话参数。

### 2. 盲目录读写代码位置审计

| 位置 | 读写时机与行为 | 运行期间是否读正在跑的盲槽位 |
|---|---|---|
| `src/blind_slot.rs` | 获取锁后清空槽位、结束归档迁移并清空、放锁 | 否（锁文件保护独占使用） |
| `src/dispatch.rs` | `BlindSlotGuard::acquire` 取槽，`prepare_outbound` / `rewrite_blind_slot_paths` / 写 `KIMI_TASK.md`，结束后 `guard.archive_and_release` 迁至 `runs/<派发>/<节点>`，并记归档目录为 `record_workdir` | 否（先持锁写入，结束后先停 ACP 再归档） |
| `src/channels/acp.rs`（`close_by_cwd`） | 归档前关停仍占用该槽位 `cwd` 的 pooled ACP 子进程，避免 Windows 目录占用 | 否 |
| `src/resources.rs`（`SmokeScratch::Slot`） | `agy` 首次验证与 `smoke` 临时借用并离开即清 | 否 |
| `src/engine/mod.rs`（`recover`） | 若 run 状态为 `running` 或 `done` 直接早退；仅读取单写者 `.inflight` / `.before` 快照与 worktree 路径 | 否（不读取运行中的盲槽位目录） |
| `src/run_view.rs` | 读取已落盘 run/graph/记录文件（盲节点 `artifacts_dir` 为空） | 否（不扫描运行中的盲槽位目录） |

### 3. 阶段 1 真机核实

- 在 `AICROSS_HOME=D:\aicross-data` 下连续两次以盲审模式跑 `kimi::kimi-code/k3`：
  - `~/.kimi-code/workspaces.json` 路径数前后保持 `241`（未新增条目）。
  - `~/.kimi-code/projects.json` 首次新增 1 条 `wd_kimi-1_d19fc509d49d`（对应 `blind\kimi-1`），第二次复用 `blind\kimi-1` 新增 `0` 条。
  - 两次派发的 `runs/<派发>/<节点>` 归档材料完整，`blind\kimi-1` 跑完为空。
  - **额外发现**：本机 `kimi 2.6.0` 同时维护 `~/.kimi-code/workspaces.json` 与 `~/.kimi-code/projects.json`（格式含 `schema_version: 2`、`projects`、`deleted_project_ids`），阶段 3 的 `leftovers` 已按两文件并集只读统计。

---

## 二、阶段 2：Antigravity（agy）

### 1. 真机核实事实（`agy 1.5.8`，`gemini-4-carbon-low`，8 次限额已用完）

- **P1–P5（按工作目录 / 项目归组）**：`agy` 无头模式同时按工作目录和项目归组，每换一个 `cwd` 会在 `~/.gemini/antigravity-cli/cache/last_conversations.json` 与 SQLite `conversation_summaries` 的 `workspace_uris` 里新增工作区记录，默认 `project_id` 为 `default-cli-project`。
- **P6–P7（`--new-project` 与 `--project aicross`）**：在目录名 `aicross` 的临时目录下执行一次 `--new-project`，创建了 `project_id = 3d89123b-c15c-4f18-be00-1f6d94e151c7`、名为 `"aicross"` 的项目（`default_project_id.txt` 仍为 `default-cli-project`）；后续在其他目录带 `--project aicross` 会自动解析到该项目。
- **P8（最终真机盲审验证，`run_id = 0ffd9a68-41df-4fdf-8304-f7970703a7ff`，第 8/8 次调用）**：
  - 以 `--preset blind --seat 1.3=agy::gemini-4-carbon-low` 运行：节点 `0d6334fe-ae98-4fc1-860b-3e9da92a614e` 状态 `"done"`。
  - 跑完后 `D:\aicross-data\blind\agy-1` 目录为空；`D:\aicross-data\runs\0ffd9a68-41df-4fdf-8304-f7970703a7ff` 中已归档 `0d6334fe-ae98-4fc1-860b-3e9da92a614e\ACCEPT.md`、`VALIDATION.txt`、`.raw.txt`、`agy-settings\0d6334fe-ae98-4fc1-860b-3e9da92a614e-主.json`、`tasks\`。
  - `~/.gemini/antigravity-cli/cache/last_conversations.json` 仅新增 `'D:\\aicross-data\\blind\\agy-1': 'f2fd3380-9fa6-4353-bd76-e82c7c9f7544'`（`128 -> 129`）；`conversation_summaries` 写入 `project_id = '3d89123b-c15c-4f18-be00-1f6d94e151c7'`、`workspace_uris = ["file:///.../scratch/aicross","file:///D:/aicross-data/blind/agy-1"]`。

### 2. 代码与同步改动

- **槽位与 `--project aicross` 接线**：
  - `src/blind_slot.rs`：`blind_slot_prefix("agy") => Some("agy")`，与阶段 1 共用 `blind/agy-1..8` 槽位与归档逻辑。
  - `src/channels/cli_agents.rs`：`agy` 命令行拼接在 `blind == true` 时附带 `--project aicross`，非盲不传；只读 `--settings` 禁止规则不变。
  - `src/resources.rs`：`SmokeScratch::create_in` 对 `agy` 改用 `blind/agy-1..8` 固定槽位并在离开作用域后清空（不再新建一次性目录）。
- **skill 同步与文档**：
  - `skills/ai-cross/references/dispatch.py`：增加 `agy_blind_slot`，更新 `call_agy`（`review` / `smoke` 在不进 `--read-dir` 时复用 `blind/agy-1..8` 并带 `--project aicross`，`run` 保持默认项目行为不传 `--project`）。
  - `skills/ai-cross/references/channels.md` 与 `docs/CONSOLE-CHANNEL.md`：补记 `agy` 固定槽位与 `--project aicross` 规则；执行 `python scripts/sync-skill.py` 同步到 `skill-bundle/ai-cross/`。

---

## 三、阶段 3：非盲标记、`leftovers` 与 Claude 遗留排查

### 1. 非盲位置 `[aicross 派发]` 首行标记

- **`src/dispatch.rs`**：新增 `NON_BLIND_SESSION_TAG = "[aicross 派发]"` 与 `should_prepend_session_tag(agent, blind, continuing)`。当 `agent` 为 `claude`、`codex`、`kimi`、`agy` 且 `!blind && !continuing` 时，在最终外发文本首行前置 `[aicross 派发]\n`（`pi`、盲位置、续接轮不加），记录文件里的「任务全文」与外发内容一致；黄金夹具与 `src/engine/` 拼提示词层未改。

### 2. 新命令 `aicross leftovers`

- **`src/leftovers.rs`**（在 `src/main.rs` 注册）与 **`src/console_cli.rs`**：
  - 注册 `aicross leftovers`（只列不删，无删除参数），更新 `aicross --help`、`docs/CONSOLE-CHANNEL.md` 与 `docs/MANUAL.md`。
  - 通过 `collect` / `collect_with_home` 同时扫描当前 `AICROSS_HOME` 与默认 `~/.aicross`（归一化去重），自动排除 `<root>/kimi-oneshot` 与 `<root>/blind/{kimi,agy}-1..8`。
  - 只读读取三家本机数据：
    - `claude`：扫描 `~/.claude/projects/` 子目录；目录内存在 `sessions-index.json`（`originalPath` / `entries[].projectPath`）或 `*.jsonl`（`cwd`）时直接用真实路径匹配；若为空 `memory/` 目录无索引文件，则限定匹配 `<encoded_root>` 或 `<encoded_root>-{runs,scratch,blind,sessions,worktrees,smoke,tests}` 边界，避免把兄弟目录（例如 `aicross-data-other`）误报为遗留。
    - `kimi`：合并只读 `~/.kimi-code/workspaces.json`（扣除 `deleted_workspace_ids`）与 `~/.kimi-code/projects.json`（扣除 `deleted_project_ids`）的 `root` 路径，按归一化路径去重；缺失文件返回空列表与 `error: null`，格式不可识别或无法读取返回 `error: "读不到 …"` 而不使命令失败。
    - `agy`：只读 `~/.gemini/antigravity-cli/cache/last_conversations.json` 的工作区路径键。
  - stdout 输出 JSON `{ roots, claude, kimi, agy }`，stderr 输出统计摘要并退出 `0`；传多余参数退出 `2`，引擎根目录不可用退出 `3`。

### 3. 8 条 Claude `sunwu-draft` 目录排查与修复

- **真机排查结论**：
  - 本机 `~/.claude/projects` 中 8 个 `sunwu-draft-...-run-claude-anthropic` 目录（`2026-11-12` 产生，位于系统 Temp 下的外部已安装/运行过的 skill 副本路径）与 2 个 `D--aicross-data-scratch-...` 目录内部均只有空的 `memory/` 子目录、0 个 `.jsonl` 会话文件。
  - 根因定位：对 `claude.exe` 仅传 `--no-session-persistence` 而不带 `--restricted` 时，Claude Code 仍按默认的 `autoMemoryEnabled` 在 `~/.claude/projects/<编码目录>/memory/` 建空目录；仓库内 `skills/ai-cross/references/dispatch.py`（`call_claude` 非盲 `run` 且无 `--restricted` 分支）与 `skills/ai-cross/references/cc_switch.py`（`cmd_exec`）均存在该缺口。
  - **已修复本仓库代码**：在 `skills/ai-cross/references/dispatch.py` 与 `skills/ai-cross/references/cc_switch.py` 的 `--no-session-persistence` 路径同时加上 `"--settings", '{"autoMemoryEnabled":false}'`，更新 `skills/ai-cross/references/channels.md` 与对应单测，并执行 `python scripts/sync-skill.py`（同步 21 个文件到 `skill-bundle/ai-cross/`）。
  - 本机已安装到宿主 `~/.codex/skills/ai-cross/` 的外部副本按任务书要求未改动。

### 4. 文档更新

- `docs/CONSOLE-CHANNEL.md`：更新开头命令概览，在 `## 派发` 补记 `[aicross 派发]` 规则，新增 `## 历史遗留条目（leftovers）`。
- `docs/MANUAL.md`：命令表新增 `aicross leftovers`，新增 `## 清理 CLI 历史遗留条目` 说明如何识别并在各自 CLI 关闭后手动清理。

---

## 四、现有测试调整说明

1. `src/dispatch.rs::task74_kimi_long_prompt_uses_task_file_and_marks_unreported_usage`：原断言期望盲位置工作目录在 `run_root/kimi-blind`；阶段 1 后盲 `kimi` 在 `blind/kimi-1` 槽位执行、跑完清空槽位并把 `KIMI_TASK.md` 与记录 `workdir` 归档到 `run_root/kimi-blind`，对应更新断言。
2. `src/channels/cli_agents.rs::task61_agy_invocation_is_stdin_ndjson_with_effort_and_no_skip_permissions`：阶段 2 后盲 `agy` 命令参数增加 `"--project", "aicross"`，补充参数断言。
3. `src/resources.rs::task61_r1_agy_smoke_scratch_is_private_and_removed`：`SmokeScratch::create_in(..., "agy")` 由每路一次性目录改为分配 `blind/agy-1`、`blind/agy-2` 并在离开作用域时清空槽位，更新目录名与释放后可复用的断言。
4. `src/dispatch.rs::task74_agy_1_5_6_blind_large_project_copy_single_writer_and_worktree_gate`：阶段 2 后盲位置 `agy` 使用 `blind/agy-1`、结束后清空归档且带 `--project aicross`，非盲单写者不带 `--project aicross`，更新对应断言。
5. `skills/ai-cross/tests/test_dispatch.py`（`test_claude_blind_flags_and_error_masquerade`、`test_named_executor_and_claude_not_restricted` 等）与 `skills/ai-cross/tests/test_cc_switch.py`（`test_env_injection_and_cmd_shape` 等）：阶段 2 为 `call_agy` 补充 `test_agy_uses_blind_slots_and_project_flag`；阶段 3 为 `--no-session-persistence` 路径补充 `--settings '{"autoMemoryEnabled":false}'` 断言。

---

## 五、全量验证命令与结果原文

### 1. skill 同步与 Python 测试（最后一次编辑后重跑）

```text
> python scripts/sync-skill.py
已同步 21 个文件：D:\work\aicross\skills\ai-cross -> D:\work\aicross\skill-bundle\ai-cross

> Set-Location D:\work\aicross\skills\ai-cross; python -m pytest -q tests
........................................................................ [ 85%]
............                                                             [100%]
84 passed in 5.97s
```

### 2. Rust 编译与单元测试（最后一次编辑后重跑）

```text
> cargo check
    Checking aicross v0.3.8 (D:\work\aicross)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.87s

> cargo test
     Running unittests src\lib.rs (target\debug\deps\aicross-3677c3fdaac21494.exe)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\main.rs (target\debug\deps\aicross-d4c806706fa7f0da.exe)
test result: ok. 810 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 62.07s

   Doc-tests aicross
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

> cargo build
   Compiling aicross v0.3.8 (D:\work\aicross)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 13.45s
```

说明：10 个 `ignored` 为既有 `#[ignore]` 真机/辅助进程用例（`live_kimi_acp_two_turn_continuation`、`live_arena_and_arbitration`、`live_blind_review`、`live_cc_switch_route`、`live_economy_pipeline`、`live_shared_continuation`、`pipe_eof_helper`、`live_codex_and_kimi_fact_sources_parse`、`live_detect_four_agents`、`live_smoke_pi_models`）。

### 3. 阶段 3 真机验证

#### （1）`aicross leftovers` 与手工解析对照

```text
> AICROSS_HOME=D:\aicross-data D:\work\aicross\target\debug\aicross.exe leftovers
leftovers rc: 0
leftovers stderr: 已列出历史遗留条目：claude 2 条，kimi 47 条，agy 45 条
Expected counts: claude=2, kimi=47, agy=45
Got counts:      claude=2, kimi=47, agy=45
PARITY CHECK PASSED!
```

确认输出与独立解析 `~/.claude/projects`、`~/.kimi-code/workspaces.json` + `~/.kimi-code/projects.json`、`~/.gemini/antigravity-cli/cache/last_conversations.json` 的集合与条数完全一致，且 `D:\aicross-data\blind\kimi-1`、`D:\aicross-data\blind\agy-1` 与 `kimi-oneshot` 均未计入遗留。

#### （2）经济模式非盲 `[aicross 派发]` 真机验证（`AICROSS_HOME=D:\aicross-data`，未用 `agy`）

- **第 1 次（`--seat 1.1=claude::haiku --seat 2.1=claude::haiku`，`run_id = 493a7d00-0e68-4f06-a14f-91aebb075eb2`）**：
  - 第 1 阶段（出方案）记录 `.aicross/runs/20261114-123347-493a7d00/20261114-123418-claude-haiku-出方案.md` 的 `## 任务全文` 与 Claude 会话文件 `C:\Users\keros68\.claude\projects\C--Users-keros68--gemini-antigravity-cli-brain-2ea0618c-98a1-4317-b0e6-8f4b1e2a38df-scratch-phase3-econ-proj\dee33669-bef4-47c5-b53c-d397a0daca56.jsonl` 首条用户消息均以 `[aicross 派发]\n角色：出方案...` 开头。
  - 第 2 阶段（执行）在同会话续接，记录 `20261114-123454-claude-haiku-执行.md` 与该 `.jsonl` 第二条用户消息均为 `角色：执行...`（未重复追加 `[aicross 派发]`）。因该临时 git 目录位于 `~/.gemini/` 下触发 Claude Code 对该目录的 `Edit` 权限保护（`"you haven't granted it yet"`），`hello.txt` 未改（`status: no_changes`，整轮 `failed`）。
- **第 2 次（`--seat 1.1=codex::gpt-5.6-luna --seat 2.1=codex::gpt-5.6-luna`，`run_id = ea3a8b90-dc58-4170-a35d-dfa5cfd83dfd`）**：
  - `hello.txt` 已从 `'v1\n'` 改写为 `'v2\n'`；第 1 阶段（出方案 `codex::gpt-5.6-luna`）、第 2 阶段（执行 `codex::gpt-5.6-luna`）、第 3 阶段（盲审验收 `pi::glm-5.3-flash`、盲审验证 `pi::deepseek-v4.1-flash`）均完成（`"status": "done"`）。
  - 第 1 阶段记录 `20261114-123654-codex-gpt-5-6-luna-出方案.md` 与 `~/.codex/sessions/2026/11/14/rollout-2026-11-14T12-35-56-01a1d857-2070-73c3-b834-1b13454bf851.jsonl` 首条用户消息以 `[aicross 派发]` 开头。
  - 第 2 阶段续接记录 `20261114-123829-codex-gpt-5-6-luna-执行.md` 与同会话第二条用户消息以 `角色：执行...` 开头（未重复加 `[aicross 派发]`）。
  - 第 3 阶段盲审节点 `20261114-123841-pi-glm-5-3-flash-验收.md`、`20261114-123940-pi-deepseek-v4-1-flash-验证.md` 首行均为 `以下是一批原始材料。...`（不含 `[aicross 派发]`）。
  - 备注：`status --wait --timeout 300` 在第 3 阶段触发修正节点（`54ad2a67-...`，其新会话 `rollout-2026-11-14T12-39-41-01a1d85a-a479-7431-8217-7671dcf757f4.jsonl` 首行亦带 `[aicross 派发]`）后到达 300 秒超时返回退出码 1（`等待状态超时`），驱动脚本退出后 Windows 作业对象回收了在途引擎进程（最终 `status` 显示 `reason: "引擎进程已退出"`）。

---

## 六、没做什么与不确定处

- **未执行项**：
  - 未提交 git commit、未 push、未改版本号。
  - 未删除任何 CLI 的现有历史文件或目录，未修改用户全局 CLI 配置（除阶段 2 按任务书建了名为 `aicross` 的 agy 项目），未读 `~/.pi/auth.json`、`~/.cc-switch/*.db`，未改动本机 `~/.codex/skills/ai-cross/` 外部安装副本。
  - `src-ui/` 未改动，未运行 `cd src-ui && npm run build`。
  - 未在 macOS/Linux 上验证。
- **不确定处**：
  - 对 `~/.claude/projects` 里没有 `sessions-index.json` 或 `*.jsonl` 的空 `memory/` 目录，因 Claude 的目录名编码把分隔符与 `-` 统一映射为 `-`，只能依赖 `<encoded_root>` 与已知数据子目录名（`runs`、`scratch`、`blind`、`sessions`、`worktrees`、`smoke`、`tests`）识别；若更早版本曾在 `<AICROSS_HOME>` 下直接以其它未列出的目录名起过 `claude` 且未留 `.jsonl`，将不会被空目录回退规则匹配到。
  - 本机已通过修改 `skills/ai-cross/` 修复后续 `--no-session-persistence` 产生空 `memory/` 目录的逻辑，但此前落在本机 `~/.codex/skills/ai-cross/`（若有旧版副本）需在用户下次安装/更新 skill 时覆盖。

---

## 七、验收复核（2026-11-14）

### 1. 复核内容与结果

- `cargo check` 无警告；`cargo test`：`811 passed; 0 failed; 10 ignored`；`python -m pytest -q tests`：`84 passed`；`python scripts/sync-skill.py` 后内置包与源码一致。
- `aicross leftovers`（`AICROSS_HOME=D:\aicross-data`）输出 claude 2、kimi 47、agy 45，与另写脚本对三家历史文件的统计一致；`aicross leftovers --delete` 退出 2。
- 本机历史文件：`~/.kimi-code/projects.json` 里固定目录只有 `kimi-oneshot` 与 `blind\kimi-1`；`last_conversations.json` 里固定目录只有 `blind\agy-1`；`~/.gemini/antigravity-cli/settings.json` 修改时间仍是 2026-11-13。
- Codex 会话 `rollout-2026-11-14T12-35-56-…jsonl` 首条用户消息以 `[aicross 派发]` 开头。

### 2. 探针核实的两条事实（共 3 次调用）

| 调用 | 结果 |
|---|---|
| `claude -p --model haiku --tools "" --output-format json --no-session-persistence`，空目录 | `~/.claude/projects/<目录>/memory/` 被建出，内容为空 |
| 同上，加 `--settings '{"autoMemoryEnabled":false}'` | `~/.claude/projects` 下没有新目录 |
| `agy --model gemini-4-carbon-low … --project aicross-missing-probe75`（项目不存在），只读 `--settings` | 退出 0，`status: SUCCESS`，stderr 为空 |

探针建出的空目录已删除。

### 3. 复核时的改动

- `src/blind_slot.rs`：拿到锁但清空失败的槽位放锁并换下一个；没有槽位被占用却全部清空失败时立即返回清空失败的原因，不等到超时。新增用例 `slot_that_cannot_be_cleared_is_skipped_and_all_unclearable_fails_at_once`。
- `skills/ai-cross/references/dispatch.py` 的 `agy_blind_slot`：清空后目录仍有内容的槽位放锁并换下一个；`test_agy_uses_blind_slots_and_project_flag` 补对应断言。
- `docs/MANUAL.md`「清理 CLI 历史遗留条目」一段改为只写现状。

### 4. 限制

- 产品里没有代码创建 Antigravity 的 `aicross` 项目；本机这个项目由执行者手动建出。没有该项目的机器上 `--project aicross` 不起作用，对话落在默认项目，工作区条目仍限制在 8 个槽位内。
- 引擎（Rust `File::try_lock`）与 skill 脚本（Python `msvcrt.locking`）共用 `blind/agy-<n>.lock`，两边同时运行时的互斥没有实测。
- 经济模式真机验证没有一次完整跑到结束（见第五节 3（2）），只核实了首行标记。
- `leftovers` 除任务书写明的 `workspaces.json` 外还只读 `~/.kimi-code/projects.json`：Kimi 2.6.0 把工作目录记在后者。
