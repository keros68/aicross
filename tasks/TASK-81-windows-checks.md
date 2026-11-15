# TASK-81 在 Windows 上核对 skill 1.30.x 的改动

## 背景

TASK-76 到 TASK-80 都在一台 macOS 上做，那台机器没有 agy、cc-switch、aicross 引擎和 cargo。下面各项在那边只有单元测试或读代码的依据。本期在 Windows 开发机上逐项真机核对，发现问题就地修。

先 `git pull`，读 `tasks/REPORT-76.md` 到 `REPORT-80.md` 的「没做什么」与「不确定处」。

授权：本期允许经 `references/cc_switch.py` 与 `references/verify_model.py` 读取 `~/.cc-switch/*.db`，只用于第 3 项；key 不得回显、不得写进任何文件或记录。

## 核对项

每项写明：跑了什么命令、输出原文的关键几行、结论（符合 / 不符合并已修 / 不符合未修及原因）。

1. **基础**：`cd skills/ai-cross && python -m unittest discover -s tests` 全部通过；`cargo check && cargo test` 通过。
2. **入口命令走引擎的那条路**：`python skills/ai-cross/references/dispatch.py start --host claude`（再换 `codex`、`kimi`）。核对它列出的模式与位置同 `aicross config show --host <宿主>` 的实际输出一致；`dispatch.py` 的 `engine_config`、`seat_text` 读的字段名都真实存在。引擎未配置该宿主、引擎读不了配置两种情形各看一次输出。
3. **cc-switch**：
   - 对一个 cc-switch 条目跑 `dispatch.py smoke --entry <条目>`，再按提示跑 `verify_model.py`，确认两步接得上。
   - `~/.claude/settings.json` 的 `env` 里有 `ANTHROPIC_BASE_URL` 时，`inventory.py save` 一个不写 `vendor` 的 `claude` 条目应被拒绝，写了 `vendor` 应通过。本机当前没有这个设置就不要为测试去改它，改用单元测试的结论并注明。
4. **agy 大材料**：`dispatch.py` 在盲审材料超过 `AGY_INLINE_LIMIT`（200 KB）且 agy 不低于 1.5 时，把任务写进 `REQUEST.md` 让对方读，而模板里固定有一句「材料已全部附在下面，不要使用任何工具」。用一份超过 200 KB 的材料点名 agy 条目真实盲审一次：对方若拒读或没看材料就作答，改成材料走文件时不带那一句、换成只读取 `REQUEST.md` 的说明，再跑一次确认。
5. **kimi**：
   - 不带 `--read-dir`，让 kimi 读一个目录外的文件、再在一个 git 项目里新建一个文件，核对结果同 `REPORT-79`、`REPORT-80`（读得到、写得成、输出里有「派发期间项目里这些文件有变化」）。
   - 引擎路径下 kimi 当执行者时是否有同样的情况，有就记下，不在本期改引擎。
6. **并发**：
   - 同时发两条 `dispatch.py run … --go`（同一条目、最便宜的档），核对两条都成功、记录分别在 `<时间>` 与 `<时间>-2` 两个目录。
   - 临时 `AICROSS_HOME` 下 12 个进程同时调用 `inventory.update_manifest` 各追加一条备注，核对 12 条都在（Windows 走 `msvcrt` 锁，这条路径没有运行过）。
7. **带序号的派发目录**：让引擎的查看器和 `aicross` 的相关命令读一个名为 `<时间>-2` 的派发目录，核对能正常列出和打开。不能的话，在 skill 一侧换一种引擎认的命名，或在引擎一侧放宽，取改动小的。
8. **参数名对齐**：对照 `aicross dispatch --help` 与 `dispatch.py review|run --help`，列出同一含义却不同名的参数。skill 一侧已认 `--text-file`、`--task-file` 与引擎的六个任务类型；其余差异列成表，给出建议，改不改由用户定，本期不改引擎参数。

## 不做

- 不改应用版本号，不发 Release。
- 不重写 `cc_switch.py`、`verify_model.py`、`usage_probe.py`。
- 不给 codex、kimi 加系统级沙箱，不给 pi 以外的通道加真身核对。

## 交付

`tasks/REPORT-81.md`，没做完、不符合的项写在最前面。有代码改动时 skill 版本升到 1.30.4，运行 `python scripts/sync-skill.py`，Conventional Commits 英文提交，不 push。
