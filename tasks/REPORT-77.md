# REPORT-77：一条入口命令、对齐参数、没装引擎也能记裁决

按 `tasks/TASK-77-one-entry-and-verdict.md` 实施。只改了 skill，引擎 `src/` 未动；skill 版本 1.29.0 → 1.30.0。

## 改了什么

- `references/dispatch.py`
  - `start`（`cmd_start`、`engine_config`、`seat_text`）：四种情形各给下一步。命令里的 skill 路径用脚本所在目录的实际路径。
  - `verdict`（`cmd_verdict`、`count_verdict`）：认表规则与 `src/track.rs` 的 `count_verdict` 相同，写入内容与 `src/engine/mod.rs` 的 `adopt_verdict` 相同（原文加换行）。
  - `run` 的任务文本参数认 `--text-file` 与 `--task-file`；`review --kind` 认引擎的六个任务类型（`ENGINE_KINDS`）。
  - 各路都没有结果时的提示补一句「原命令再跑一次会改选排在前面的其他条目」。
- `references/inventory.py`：`mark_failure` 记 `quota`；`choose_reviewers` 把额度用尽条目所在订阅的其他条目一起排后；`print_manifest` 多一行近一天排后的条目。
- `SKILL.md`：「从这里开始」改为先跑 `start`，一张表对应四种输出；审与干活的要点保留；裁决的记法两条路都写明。文件 20,295 → 18,186 字节。
- `references/channels.md`：脚本一节补 `start`、`verdict`、参数对齐与连带排后的说明。
- `skill-bundle/ai-cross/`：重新同步。

## 验证

```
$ cd skills/ai-cross && python3 -m unittest discover -s tests
Ran 99 tests in 1.672s

OK (skipped=1)
```

新增 9 条：`TestStartAndVerdict` 下 7 条（有清单、没清单退出 3、被派出的子任务、引擎配置优先与引擎报错回退、没装引擎或宿主不在引擎名单、裁决写入最近一次派发、认不出选择与找不到派发目录），另有 `test_quota_failure_moves_the_whole_subscription_back`、`test_review_accepts_engine_task_kinds`。现有测试没有改断言，只删掉了文件中间重复的 `unittest.main()` 入口。

真机（本机 macOS，没装 aicross 引擎）：

- `start --host claude-code`：打印清单与三条命令，退出码 0。
- `AI_CROSS_PEER=1 … start`：只打印「只完成交给你的任务并直接作答」。
- `AICROSS_HOME=<空目录> … start --host codex`：打印「这台机器还没盘点」，退出码 3。
- `verdict --verdict-file … --project <临时项目>`：输出「已记录裁决：改 1 件，不改 1 件，以后不再报 0 件」，`裁决.md` 落在最近一次派发目录 `20261115-170141/`，内容与表一致。
- `run --host claude --text-file …`、`review --kind backend …`（都不带 `--go`）：正常给出路由决策行。

## 没做什么

- 「走 aicross 引擎」这一情形没有真机验证：本机没装引擎，只用伪造的 `config show` 输出做了单元测试。输出的 JSON 字段按 `channels.md`「按窗口配置派发」的描述取（`defaultMode`、`modes[].stages[][].effective`、`unfilledReason`），与引擎实际输出是否逐字段一致未核对。
- `cargo check`、`cargo test` 没跑：没有改 Rust 代码，本机也没有 cargo。
- 宿主模型是否稳定地先跑 `start` 再行动，没有做行为测试。

## 不确定处

- `SKILL.md` 只减了约一成。入口的分支判断移进了脚本，但「派发前的判断」「拿到结果后的判断」是每次都要用的规则，留在正文里。
- 连带排后只对新记下的失败生效。本机 TASK-76 真机派发时记下的两条失败没有 `quota` 标记，所以千问条目今天仍排在前面；一天后这两条记录自然失效。
- 额度用尽靠错误文字里的关键词识别（429、usage limit、limit reached、quota、rate limit、额度、限额），别的写法认不出，只会排后那一个条目。
