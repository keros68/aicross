# REPORT-86：空着的位置不再让整次派发被拒绝

按 `tasks/TASK-86-skip-empty-seats.md` 实施。本机 macOS（Apple Silicon）。

## 没验证的

- Windows 上没有运行。
- 评审模式没有真机派发；视角名单随位置去掉这一步只靠读代码，没有单独的测试。
- 窗口的查看页对少一路的派发怎么显示，没有看。
- 擂台模式三个执行位置空一个的情况走同一段代码，没有单独试。

## 改了什么

- `src/engine/mod.rs`：新增 `skip_empty_seats`，`plan_dispatch` 在补完人选之后、建图之前调用。同一阶段里有人选的位置不少于两个且有空位时，去掉空位，各留一句「第 N 阶段第 M 个位置（角色）没有人选，这次没有派」，并入原有的 `notes`。不足两个时不动，仍由后面的检查拒绝。
- `src/engine/tests.rs`：新增 `blind_dispatch_skips_an_empty_seat_when_two_are_filled`，覆盖两种情况：三个位置空一个时派出两路并带说明；空两个时拒绝。
- `skills/ai-cross/references/channels.md` 第 3 条、`docs/CONSOLE-CHANNEL.md`：写明新行为。skill 版本 1.30.6 → 1.30.7，`skill-bundle/` 重新生成。

## 验证

```
$ cargo test
test result: ok. 813 passed; 0 failed; 9 ignored; 0 measured; 0 filtered out; finished in 18.44s
$ cd skills/ai-cross && python3 -m unittest discover -s tests
Ran 110 tests in 1.959s
OK (skipped=1)
```

真机（安装版，本机真实配置，临时项目目录，10 行示例代码）：

- 改动前：`aicross dispatch --host claude --preset blind …` 被拒绝，`reason` 为「角色「验证」没有可派发目标」。
- 改动后同一条命令：`accepted` 为 true，两个节点，`notes` 为「第 1 阶段第 3 个位置（验证）没有人选，这次没有派」；`status --wait` 返回 done，Codex gpt-6.1-sol 与 Kimi k3 两路都完成，结束后没有残留进程。
- 改动前另用 `--seat 1.3=pi:xai:grok-4.6` 临时补上第三个位置做过一次三家盲审，三路都完成。

## 没做什么

- 任务书「不做」里的各项。

## 不确定处

- 「至少两个」对所有模式一律适用。经济模式第三阶段（验收、验证各一个）空一个时只剩一个，仍会被拒绝。
- 用户是否留意到少派了一家，取决于宿主有没有把 `notes` 那句话写进开场；skill 的规则已要求这样做，宿主是否稳定照做没有测。
