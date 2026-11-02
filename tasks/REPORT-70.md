# REPORT-70 一次性派发不留会话

## 改了什么

- `src/channels/mod.rs`：claude 在 blind=true（盲审、冒烟）时加 `--no-session-persistence`；新增测试 `claude_executor_keeps_session_for_revision_rounds`，确认执行位置不加；`blind_claude_contains_restricted` 补断言。
- `skills/ai-cross/references/dispatch.py`：claude `--no-session-persistence`、codex `--ephemeral`、pi `--no-session`。
- `skills/ai-cross/references/cc_switch.py`：exec 加 `--no-session-persistence`。
- `skills/ai-cross/references/channels.md`：命令表同步，新增「一次性派发不留会话」一段。
- `skill-bundle/` 由 `python scripts/sync-skill.py` 重新生成（21 个文件）。
- `docs/PRODUCT.md` 定位一节补「给谁、解决什么」（用户 11-02 定），与本期无代码关系，同批提交。

## 怎么验证的

- 参数存在（本机 `--help`）：pi `--no-session  Don't save session (ephemeral)`；claude `--no-session-persistence  Disable session persistence`；codex exec `--ephemeral  Run without persisting session files to disk`；kimi 无对应参数。
- `cargo test --quiet channels::`：`91 passed; 0 failed; 1 ignored`。
- `cd skills/ai-cross && python -m pytest -q tests`：`70 passed`。
- 真机（`D:\aicross-data\scratch\20261102-nosession-check`，跑完删除）：`claude -p --model haiku … --no-session-persistence` 返回 JSON 正常；`pi --provider zai-coding-cn --model glm-4.7 … --no-session` 返回 OK 文本。之后 `~/.claude/projects` 与 `~/.pi/agent/sessions` 中匹配该目录名的条目数均为 0。
- codex 的 `--ephemeral` 引擎盲审早已在用，本期没有单独真跑 `dispatch.py` 的 codex 路径。

## 没做

- 已留下的会话目录未清理。
- kimi 与 `cli_agents` 里其他通道（agy、copilot 等）的会话保存未核查。
- 未跑 `cargo check --all-targets` 全量、前端构建与盲审（改动只涉及参数拼接）。
