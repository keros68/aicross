# TASK-72 kimi 一次性派发固定目录

## 问题

TASK-70 给 claude、codex、pi 加了关会话参数，kimi 没有对应参数。kimi 按工作目录登记工作区，skill 的 `dispatch.py` 每次盲审、试跑、冒烟都给 kimi 新建一个空目录，`~/.kimi-code/workspaces.json` 里每跑一次多一个 `kimi.moonshot` 工作区。2026-11-07 本机统计：15 个指向 `.aicross/scratch/*/kimi.moonshot`。

## 范围

- `skills/ai-cross/references/dispatch.py`：kimi 一律在 `<AICROSS_HOME>/kimi-oneshot/` 里跑。锁文件 `<AICROSS_HOME>/kimi-oneshot.lock` 放在目录外，同一时间只跑一路；每路运行前后清空目录，盲审隔离不变。等锁超过本次超时就不派，返回错误说明。
- `references/channels.md`：命令表与「一次性派发不留会话」一段同步。
- `skill-bundle/` 由 `scripts/sync-skill.py` 重新生成。

## 不做

- 引擎（`src/`）的 kimi 盲审：盲审目录里放着给模型读的材料，改成固定目录要另行设计；本期不动。
- 不读写 kimi 自己的 `workspaces.json` / `session_index.jsonl`：格式随 kimi 版本可能变，也可能与运行中的 kimi 争写。

## 验证

`cd skills/ai-cross && python -m pytest -q tests`；真机连跑两次 `dispatch.py smoke --entry kimi.moonshot`，`workspaces.json` 只多 `kimi-oneshot` 一个工作区，目录跑完为空。
