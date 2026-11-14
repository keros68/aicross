# TASK-73 不留会话的派发另记用量

## 问题

TASK-70 之后，`dispatch.py` 派发 claude、codex、pi 时不保存会话。这些调用因此不在各家 CLI 自己的日志里，按本地日志统计用量的工具（如 Metrik）读不到，派发消耗的 token 在统计里缺失。

## 范围

- skill `references/dispatch.py`：claude、codex、pi、cc-switch 四条路径各自给出互不重叠的四个分量（未缓存输入、缓存读、缓存写、输出）；`call()` 跑完一路后把用量追加到 `$AICROSS_HOME/usage/<年-月>.jsonl`，一个模型一行，schema `aicross-usage/1`。写不下时不影响派发。
- claude 优先取 `modelUsage`（按实际应答的模型分列），没有时退回 `usage` 汇总。
- `references/channels.md`：说明这份记录的位置、字段和不记的情形。
- `skill-bundle/` 重新生成。

## 不做

- kimi 与 agy：会话仍由各自的 CLI 保存，不写这份记录，避免重复统计。
- 引擎 `src/channels/mod.rs` 的派发路径不在本期。
- 不做额度或金额核算，只记 token 分量与归属；不记任务和回答。
- 既有的派发记录（`.aicross/runs/`）格式不变。

## 验证

`cd skills/ai-cross/tests && python -m unittest -q`；真机各跑一次 claude 与 codex，确认四个分量之和等于 CLI 报告的输入总量加输出。
