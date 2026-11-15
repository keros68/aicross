# REPORT-79：并发派发不冲突，说明与实际能力一致

按 `tasks/TASK-79-concurrent-runs-and-honest-limits.md` 实施。只改了 skill；skill 版本 1.30.1 → 1.30.2。

## 改了什么

- `references/dispatch.py`
  - `claim_run`：占用派发目录并返回目录名，已存在就依次试 `-2`、`-3`。`review`、`run` 用它取代按秒生成的名字；三处临时目录名加进程号。
  - `write_trace`：文件名接受 `trace_tag`；`cmd_review` 在两路程序名与模型名相同时填入条目名。
  - `cmd_run`：方案里的说明与收尾一句按通道区分 codex、kimi 与其余通道。
  - `cmd_smoke`：没有条目被实际测到时提示手动冒烟并以 1 退出。
- `references/inventory.py`
  - `write_manifest`：临时文件名加进程号。
  - `probe_claude` 标 `redirected`；`build_entry` 遇到它时要求写明 `vendor`。
  - `build_entry`：条目名规范化后为空时报错。
- `SKILL.md`、`references/channels.md`：真身核对与执行者能力的说明。
- 测试新增 3 条：同一秒两个目录、空条目名、被重定向的 claude。
- `skill-bundle/ai-cross/` 重新生成。

## 验证

```
$ cd skills/ai-cross && python3 -m unittest discover -s tests
Ran 104 tests in 1.685s

OK (skipped=1)
```

真机（macOS，临时目录）：

1. 让三条通道各读一个目录外的文件（内容是一串标记），不带 `--read-dir`：codex（gpt-6-luna）与 kimi（kimi-for-coding-highspeed）原样回了标记，pi（grok-4.6）回「读不到」。这是改说明文字的依据，在改动之前做的。
2. 同时发两条 `run`（pi，grok-4.6）：两条都退出 0，记录分别在 `20261115-185855/` 与 `20261115-185855-2/`。

## 没做什么

- 任务书「不做」里的各项。
- 没有测 kimi 是否真能改目录外的文件；「改得动」依据的是它没有只读档。
- 同模型经两个 provider 的文件名、`smoke` 全部跳过的退出码、被重定向的 claude：没有真机复现，前两项也没有单独的测试。
- `cargo check`、`cargo test` 没跑：本机没有 cargo，且没有改 Rust 代码。

## 不确定处

- 引擎读 `.aicross/runs/` 时是否接受 `<时间>-2` 这样的目录名没有核对；旧记录迁移产生的目录本来就带后缀。
- 盲审里 codex 与 kimi 同样读得到本机文件，这两条通道已标「隔离不完整」，标注没有变。
