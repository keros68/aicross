# REPORT-73 不留会话的派发另记用量

## 改了什么

- `skills/ai-cross/references/dispatch.py`：新增 `usage_part`、`record_usage`；`call_claude`、`call_cc_switch`、`call_codex`、`call_pi` 填 `out["usage"]`；`call()` 增加可选的 `project`、`run_id`，跑完后追加用量记录；`cmd_review`、`cmd_run` 传入项目与派发编号。
- `skills/ai-cross/tests/test_dispatch.py`：新增两条测试，覆盖四条路径的分量、按实际模型记录、无用量的失败调用与 agy 不写记录。
- `skills/ai-cross/references/channels.md`：新增「不留会话的派发另记用量」一段。
- `skill-bundle/` 由 `python scripts/sync-skill.py` 重新生成。

## 怎么验证的

- `cd skills/ai-cross/tests && python3 -m unittest -q`：`Ran 77 tests … OK`（改动前 75）。本机没有装 pytest，用 unittest 跑的同一批测试。
- 真机（macOS，`AICROSS_HOME` 指向临时目录）经 `dispatch.call` 各派一次：
  - claude（haiku）：记录 `input_uncached 2 / cache_read 649 / cache_write 2363 / output 4`，合计 3018，等于 `tokens_in 3014 + tokens_out 4`。
  - codex（gpt-5.6-luna）：记录 `input_uncached 10122 / cache_read 11008 / cache_write 0 / output 5`，合计 21135，等于 `tokens_in 21130 + tokens_out 5`。

## 没做

- pi 与 cc-switch 路径没有真机派发，只有伪造输出的测试；pi 的分量定义沿用 pi 会话日志（`input` 不含缓存读写）。
- 引擎 `src/channels/mod.rs` 的派发不写这份记录。
- `cargo test`、前端构建没跑（没有改 Rust 与前端）；Windows 上没验证。
- 没有提交，也没有 push。

## 不确定处

- smoke 路径（`cmd_smoke`）也经过 `call()`，会写记录但没有项目与派发编号。
- 用量文件按月追加、不清理；长期体积未评估。
