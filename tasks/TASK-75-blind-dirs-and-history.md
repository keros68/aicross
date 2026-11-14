# TASK-75 派发不在各家 CLI 的历史里留散乱条目

## 问题

各家 CLI 按工作目录记历史。引擎给每个盲位置新建一个目录 `<AICROSS_HOME>/runs/<派发>/<节点>`，CLI 没有「不存会话」开关时，每跑一次历史里就多一个工作区或项目。TASK-70 给 claude、codex、pi 加了开关，TASK-72 让 skill 的 `dispatch.py` 在固定目录里跑 kimi；引擎里的 kimi 与 agy 没处理。

2026-11-14 本机统计：

| CLI | 盲位置现状 | 历史里的条目 |
|---|---|---|
| claude | `--no-session-persistence` | `~/.claude/projects` 37 个目录，其中 2 个指向 aicross 数据目录，8 个指向 `…\Temp\sunwu-draft-…\peer-inventory\scratch\<时间>-run\claude.anthropic`（11-12 产生，来源未查明） |
| codex | `--ephemeral` | 无 |
| pi | 会话存到 `<AICROSS_HOME>/sessions/pi` | 无 |
| kimi | 没有开关，引擎每个盲位置一个新目录 | `~/.kimi-code/workspaces.json` 192 个路径，46 个是 aicross 的盲审目录 |
| agy | 没有开关 | `~/.gemini/antigravity-cli/conversations` 552 条，近两天 424 条；按什么归组未核实 |

另一类条目来自非盲位置（出方案、执行）：它们在用户的项目目录里跑，会话要留给修正轮续接，所以混在项目自己的历史列表里，标题与用户自己的对话分不出来。

`kimi session` 子命令只有 `list`，不能删除会话。`agy --help` 有 `--project <项目 id 或名字>` 与 `--new-project`，无头模式下是否可用未核实。

## 范围

分三个阶段，按顺序做。每个阶段单独通过验证后再做下一个；同一时间只有一个执行者改代码。

### 阶段 1：kimi 盲位置用固定槽位目录

1. 槽位目录是 `<AICROSS_HOME>/blind/kimi-1` 到 `kimi-8`，按需创建。每个槽位一个锁文件 `<AICROSS_HOME>/blind/kimi-<n>.lock`，放在槽位目录外，用 `src/file_lock.rs` 的 `FileLock`（跨进程）。
2. kimi 落在盲位置时（一次性通道与 ACP 通道都算；备选改派到 kimi 的那次尝试也算）：
   - 从 1 号起找第一个能立即锁上的槽位。全部占用时等待，等待受本节点超时与叫停约束；超时后节点失败，错误写「盲审目录都在使用中」。
   - 锁上后先清空槽位，再把材料铺进槽位（现在铺进 `runs/<派发>/<节点>` 的全部内容：`REQUEST.md`、`diff.patch`、`changed/`、`project/`、`ANSWER.md`、`ACCEPT.md`、`VALIDATION.txt`、附件、`KIMI_TASK.md`），kimi 的工作目录是槽位。
   - 进程树结束后（成功、失败、超时、叫停都一样），把槽位里的全部内容移到 `runs/<派发>/<节点>`，清空槽位，再放锁。移动遇到 Windows 拒绝访问或共享冲突时按 `paths::replace_file_with_retry` 的办法退避重试，仍失败则改为复制后删除。归档失败不改变节点结果，原因写进节点备注。
   - 上一次崩溃留在槽位里的内容，下一次使用前清空。
3. 派发结束后读 `runs/<派发>/<节点>` 的地方行为不变：`recover`、查看页、记录里的 `materials`、重试前清目录（`run_with_fallback` 开头）。动手前列出所有读写盲目录的代码位置，写进 REPORT；有在运行期间读盲目录的，逐个说明怎么处理。
4. 记录与 `status` 里出现的盲目录路径写归档位置，不写槽位路径。发给 kimi 的文本里若带盲目录的绝对路径，发出时用槽位路径。
5. 盲位置的 kimi 不传 `--continue`、`--session`。
6. 其他 agent 的盲目录不变。

### 阶段 2：Antigravity

先在真机上核实，再按结果改。核实用 `gemini-4-carbon-low`，每次一句话的提示词，总调用不超过 8 次。只读查看 `~/.gemini/antigravity-cli/` 下的对话数据，不改。

要核实并写进 REPORT 的事实：

- agy 的对话列表按什么归组（工作目录、项目，或不归组）；无头调用是否每次新增一个项目或工作区条目。
- `--project <名字>` 在无头模式下能否使用；项目不存在时的行为；配合 `--new-project` 的行为。
- 带 `--project` 后，对话是否归到该项目下。

按结果改：

- 按工作目录归组：agy 的盲位置同样走槽位（`blind/agy-<n>`），与阶段 1 共用一套实现。`--settings` 的禁止规则照常传。
- `--project` 可用：盲位置、首次验证与 `aicross resources smoke` 的 agy 调用带 `--project aicross`。非盲位置不带。
- 两条都成立就都做；都不成立就不改代码，REPORT 写明。
- `skills/ai-cross/references/dispatch.py` 的 `call_agy` 与 `references/channels.md` 同步。

### 阶段 3：非盲位置的会话标记与遗留清单

1. claude、codex、kimi、agy 落在非盲位置时，发给 CLI 的文本最前面加一行 `[aicross 派发]`。
   - 只加在 `src/dispatch.rs` 定最终外发文本的地方，引擎层拼的提示词与黄金夹具不变。
   - 续接会话的后续轮不加。
   - 记录里的「任务全文」与实际外发一致。
2. 新命令 `aicross leftovers`：列出各家 CLI 历史里指向 aicross 数据目录的条目，只列不删，输出 JSON（每家：条数、路径列表、读不到时的原因）。
   - 数据目录取当前 `AICROSS_HOME` 与默认的 `~/.aicross` 两处。固定槽位与 `kimi-oneshot` 不算遗留。
   - claude：`~/.claude/projects` 下目录名对应这两处之下路径的目录（目录名是路径把非字母数字字符换成 `-`）。
   - kimi：只读 `~/.kimi-code/workspaces.json`，取其中的路径。格式认不出时该家返回「读不到」，不报错退出。
   - agy：阶段 2 核实出可读的归组信息就列，否则不列。
   - `aicross --help` 与 `docs/CONSOLE-CHANNEL.md` 补这条命令；`docs/MANUAL.md` 写一段手动清理的办法（claude 的目录可直接删；kimi 的条目要在 kimi 关闭时改它自己的文件）。
3. 查清上表 8 条 claude 记录的来源：哪个程序、哪条调用路径留下的。是本仓库代码就修；是别的项目或过期的已安装 skill 副本就只在 REPORT 里写明，不动。

## 不做

- 不删除任何一家 CLI 的历史记录，`leftovers` 没有删除选项。
- 不写 kimi 的 `workspaces.json`、`session_index.jsonl`。TASK-72 定过不读写这两个文件（格式随版本可能变，也可能与运行中的 kimi 争写）；本期只在 `leftovers` 里只读 `workspaces.json`。
- 不改 skill `dispatch.py` 的 `kimi-oneshot` 单目录。
- 非盲位置不改成不存会话；修正轮续接依赖会话，是否加这个设置另行决定。
- worktree 位置的目录不改。
- 不改盲审隔离、真身核对、只读护栏的规则。同一时间一个槽位只给一个节点用，节点之间看不到对方的材料。
- 不改用户各家 CLI 的全局配置。阶段 2 核实可行时，agy 里新建一个名为 `aicross` 的项目是允许的，别的不建。
- 不读 `~/.pi/auth.json`、`~/.cc-switch/*.db` 等凭据文件。
- 不 push，不发布，不改版本号。

## 纪律

- 先读 `AGENTS.md`。prompt 不走多行 argv；密钥只走子进程环境变量。
- 开工前 `git status` 要干净；不干净先问用户，不要自行还原或提交别人的改动。
- 每个阶段改了 `skills/ai-cross/` 后运行 `python scripts/sync-skill.py`。
- 新行为都要有测试：槽位的占用、等待、清空、归档用桩程序测（参照 `src/dispatch.rs` 里 `task74_kimi_long_prompt_uses_task_file_and_marks_unreported_usage` 的写法）。
- 不为通过测试改断言含义；现有测试因行为变更需要改时，在 REPORT 里逐条说明。
- 新增的用户可见文字按 `AGENTS.md`「界面文案与命名规范」。

## 验证

每个阶段结束时：

```bash
cargo check && cargo test
cd skills/ai-cross && python -m pytest -q tests
cd src-ui && npm run build        # 只在改了界面时
```

单元测试至少覆盖：

- 两个 kimi 盲节点同时跑，拿到不同槽位；跑完槽位为空，材料在 `runs/<派发>/<节点>`。
- 槽位全部占用时等待，超时后错误文字正确；叫停时立即结束等待。
- 槽位里有上次留下的文件时，使用前被清空。
- kimi 进程失败、超时时同样归档并放锁。
- 非 kimi 的盲节点目录不变。
- 非盲位置外发文本首行是 `[aicross 派发]`，盲位置没有，续接轮没有。
- `leftovers` 在 `workspaces.json` 缺失、格式不认识时不报错。

真机验证：

- 阶段 1：盲审模式连派两次，每次含一个 kimi 位置。派发前后各记一次 `~/.kimi-code/workspaces.json` 里的路径数，新增的只有槽位目录；两次派发的归档目录里材料齐全；kimi 的回答正常。
- 阶段 2：按核实结果，派一次含 agy 盲位置的派发，对话落在预期的项目或槽位下。
- 阶段 3：经济模式派一次只改一个文件的任务，执行位置用 claude 或 codex；在该 CLI 的会话文件里确认首条用户消息以 `[aicross 派发]` 开头。运行 `aicross leftovers`，输出与手工统计一致。

## 交付

`tasks/REPORT-75.md` 按阶段写：改了什么、验证命令与结果原文、真机核实得到的事实、没做什么、不确定处。没跑过的不写「已验证」。执行者不提交，由协调者验收后提交。
