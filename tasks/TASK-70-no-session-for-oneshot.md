# TASK-70 一次性派发不留会话

## 问题

各家 CLI 按工作目录保存会话。盲审、冒烟和 skill 的 `dispatch.py` 每次都在新的空目录里跑，于是每跑一次，Claude、pi 等 CLI 的历史里就多一个项目。会话记录里出现大量 `coderB-SDM-…`、`…-blind-…` 目录。2026-11-02 本机统计：`~/.pi/agent/sessions` 下有 315 个目录指向 `scratch`，`~/.claude/projects` 下有 19 个。

## 范围

- 引擎 `src/channels/mod.rs`：claude 盲审位置（冒烟同样走 blind=true）加 `--no-session-persistence`；codex 盲审已有 `--ephemeral`；pi 已用 `--session-dir` 存到 `$AICROSS_HOME/sessions/`，不改。
- skill `references/dispatch.py`：每次都是一问一答，claude 加 `--no-session-persistence`，codex 加 `--ephemeral`，pi 加 `--no-session`。
- skill `references/cc_switch.py exec`：加 `--no-session-persistence`。
- `references/channels.md`：命令表与说明同步。
- 要续接会话的执行位置不加。

## 不做

- kimi 没有关闭会话保存的参数，不处理。
- 已经留下的会话目录不删除，清理由用户决定。
- `D:\work\agentteam` 基准测试脚本（`coding/coderB-…` 那 221 个）不在本仓库。

## 验证

`cargo test channels::`；`cd skills/ai-cross && python -m pytest -q tests`；真机各跑一次 claude 与 pi，确认有回答且历史里没有新目录。
