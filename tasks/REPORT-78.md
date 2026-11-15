# REPORT-78：盲审 skill 后的五处修正

按 `tasks/TASK-78-blind-review-fixes.md` 实施。只改了 skill；skill 版本 1.30.0 → 1.30.1。

## 改了什么

- `references/dispatch.py`
  - `build_prompt`：材料与已定事项拼好后一起脱敏。
  - `print_review`：带 `--full` 时整篇打印，不截断。
  - `cmd_review`：状态为 `skipped` 的一路也调用 `inventory.mark_failure`。
  - `--n` 帮助文字。
- `references/inventory.py`
  - `choose_reviewers`：`n` 小于 1 时报错退出。
  - `print_diff`：有通道读不到清单时写「读到的通道里没有新模型」。
- 测试新增 2 条（已定事项脱敏、`--n` 为负被拒），1 条加断言（`--full` 超过上限仍整篇）。
- `skill-bundle/ai-cross/` 重新生成。

## 验证

```
$ cd skills/ai-cross && python3 -m unittest discover -s tests
Ran 101 tests in 1.705s

OK (skipped=1)
```

盲审记录在本机 `.aicross/runs/20261115-181959/` 与 `.aicross/runs/20261115-182808/`（不进仓库）。

## 没做什么

- 任务书「不做」里的各项。
- `skipped` 记失败、`print_diff` 的措辞没有单独的测试。
- 没有真机复现五处问题，依据是读代码。

## 不确定处

- 两路审查者都标为「非独立：隔离不完整」，xAI 一路 480 秒超时，这次只有两家的静态审查意见。
- 「不做」里的并发与真身核对问题是否值得另起一期，由用户定。
