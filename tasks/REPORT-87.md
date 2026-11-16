# REPORT-87：盲审位置上 kimi 与 codex 的完整隔离

按 `tasks/TASK-87-kimi-codex-blind-isolation.md` 实施。本机 macOS（Apple Silicon），kimi 2.5.2，codex 0.169.0。

## 没验证的

- Windows 上没有运行。Windows 建符号链接需要开发者模式或管理员权限，建不了时 codex 仍记部分；这条分支没有真机试。
- codex 刷新登录时顺着链接改写原文件：依据是公开源码与安装版里的报错字符串，没有等到一次真实的刷新。
- codex 关掉命令工具后，剩下的脚本工具能否读本机文件没有证实：让模型在脚本里试其他读法，它拒绝尝试。网页搜索与派子任务两类工具关不掉。
- kimi 的最低版本定为 2.5，只在 2.5.2 上试过；更早的版本是否认 `--agent-file` 与空工具列表没有查实。引擎按版本号判断，skill 按 `--help` 里有没有这个参数判断。
- codex 被 `--disable` 的功能名在以后的版本里若被删除，会不会报错没有试。
- 引擎走 ACP 的盲位置没有真机试；代码上这种情况不加固、记部分。

## 改了什么

- **引擎**
  - `channels::blind_hardening` 按探测到的版本决定用哪种做法；`codex_blind_home_at` 建目录与符号链接，原位置已有普通文件时不动它；`kimi_blind_agent_file` 生成 agent 文件；`apply_blind_hardening` 把参数与环境变量加到调用上。
  - `isolation_for` 多一个参数：kimi、codex 只有这次调用确实加固了才是完整。
  - `Invocation` 与 `ProcessOptions` 各多一个 `env`，只给这个子进程加环境变量。
  - `agent_catalog::version_at_least`：从 `agy_at_least_1_5` 提出来，并认得「codex-cli 0.169.0」这种带名字的版本。
  - `blind_slot` 一条偶发失败的测试改为等锁最多 2 秒。失败原因是别的测试恰在锁文件打开期间拉起子进程，子进程在 exec 前短暂带着锁的副本。
- **skill**
  - `inventory.blind_isolation`、`inventory.codex_blind_home`；`cmd_diff` 刷新 manifest 里各条目的隔离，报告里多一个 `isolation` 列表。
  - `dispatch.call_kimi`、`call_codex` 在盲审且条目记为完整时加固；`run_process` 多一个 `env_extra`。
  - 版本 1.30.7 → 1.30.8，`skill-bundle/` 重新生成。
- **文档**：`channels.md` kimi 一节与「手写命令时的隔离写法」、`setup.md` 审查顺序一句、`docs/CONSOLE-CHANNEL.md` 独立性一节。

## 验证

```
$ cargo test
test result: ok. 818 passed; 0 failed; 9 ignored; 0 measured; 0 filtered out; finished in 18.79s
$ cd skills/ai-cross && python3 -m unittest discover -s tests
Ran 113 tests in 1.992s
OK (skipped=1)
```

改动前的探测（空目录，低档模型）：

- codex 现行写法：回答 `yourmem`（用户级 `AGENTS.md` 里的专有名词）。加 `--ignore-user-config -c project_doc_max_bytes=0`：仍回答 `yourmem`。换 `CODEX_HOME`：回答 `NONE`。
- codex 只读沙箱：用命令读出了 `~/.codex/AGENTS.md` 的开头。加 `--disable shell_tool`：`TypeError: tools.exec_command is not a function`。
- kimi 默认调用：列出 25 个内置工具与 30 个 MCP 工具。带无工具的 agent 文件：`NONE`、`NO-TOOLS`。

改动后真机：

- 经引擎（安装版，本机真实配置的盲审模式，10 行示例代码）：codex gpt-6.1-sol 与 kimi k3 两路都完成，`independence` 都是「独立」，记录里 `isolation: 完整`。codex 输入 15,878 词元（改动前同类调用约 23,000）。带探测题的那次，kimi 回答 `NONE`，工具只有 `Glob, Grep, Read`（盲目录里有 `VALIDATION.txt`，属于要读文件的情况）。
- 经 `dispatch.py`（点名 `kimi.moonshot` 与 `codex.openai`，低档）：两路都是「独立」。kimi 回答 `NONE`、`NO-TOOLS`；codex 的回答里没有出现用户级指令中的专有名词，输入 16,442 词元。
- `inventory.py diff --human` 把本机 manifest 里这两个条目从部分改为完整。
- 各次派发后 `~/.codex/auth.json` 的修改时间与链接数没有变化；`~/.aicross/codex-blind-home/auth.json` 仍是符号链接；没有残留进程。

## 没做什么

- 任务书「不做」里的各项。
- 没有清理 `codex-blind-home` 里 codex 自己生成的数据库与缓存文件。

## 不确定处

- 引擎对「完整」的现行标准是读不到用户级指令并且工具受限，pi 在要读文件时同样留着读文件的工具。kimi、codex 在要读文件时留下的工具能读工作目录以外的文件，与 pi 一致。
- codex 的隔离目录是各次盲审共用的，与默认配置目录下多个 codex 同时运行的情形相同，没有做并发压测。
