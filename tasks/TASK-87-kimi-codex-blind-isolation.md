# TASK-87 盲审位置上 kimi 与 codex 的完整隔离

## 问题

引擎与 skill 把 kimi、codex 的盲审一律记为「隔离不完整」，这两家的结论因此只能标「非独立」。规则是按早期版本写的：kimi 当时没有关工具的办法，codex 在空目录里仍加载 `~/.codex/AGENTS.md`。2026-11-16 在 macOS 上实测，两家的现行版本都能做到：

| | 做法 | 实测 |
|---|---|---|
| kimi 2.5.2 | `--agent-file` 指向工具列表为空、正文不引用默认提示词的 agent 文件 | 模型列不出任何技能名与工具名；试图调用 Bash 只输出文字，没有执行 |
| codex 0.169.0 | `CODEX_HOME` 指到只含 `auth.json` 符号链接的目录 | 模型不再能复述用户级指令里的专有名词；`--ignore-user-config` 与 `project_doc_max_bytes=0` 挡不住 |

codex 的公开源码（`codex-rs/login/src/auth/storage_file.rs`）写明刷新登录时顺着符号链接改写原文件，硬链接被拒绝；本机安装的 0.169.0 里有对应的报错字符串。

## 授权（2026-11-16 用户定）

用户要求「把 codex 也做好」。本期允许引擎与 `inventory.py` 在 `<AICROSS_HOME>/codex-blind-home/` 下建立一个指向 `<CODEX_HOME 或 ~/.codex>/auth.json` 的符号链接。不读取、不复制、不回显该文件的内容，不写日志。

## 范围

1. 引擎（`src/channels/mod.rs`、`src/dispatch.rs`、`src/process.rs`、`src/agent_catalog.rs`）
   - kimi 版本不低于 2.5：盲位置带 `--agent-file`，文件写在 `<run>/kimi-agents/`。材料没有全部写进任务正文，或任务超长要它读 `KIMI_TASK.md` 时，只留 `Read`、`Grep`、`Glob`。
   - codex 版本不低于 0.169：盲位置设 `CODEX_HOME`，并 `--disable` `apps`、`plugins`、`browser_use`、`computer_use`、`in_app_browser`；不需要读文件时再关 `shell_tool`、`view_image`。
   - 用上了才把隔离记为完整。版本不够、建不了符号链接、这次走了 ACP 时照旧记部分。
2. skill（`references/dispatch.py`、`references/inventory.py`）：同样的做法。能否做到在盘点时判定并记进 manifest；`inventory.py diff` 顺带刷新已有 manifest 的这一项。盘点记为完整而派发时目录建不起来的，这一路不派。
3. 文档：`references/channels.md`、`references/setup.md`、`docs/CONSOLE-CHANNEL.md`。skill 版本升到 1.30.8，运行 `python scripts/sync-skill.py`。

## 不做

- 不改 claude、pi、agy 的隔离做法。
- 不改非盲位置的调用。
- 不改应用版本号，不发 Release。

## 验证

`cargo check && cargo test`、skill 的单元测试；真机经引擎与经 `dispatch.py` 各做一次盲审，材料里带探测题（上下文里有没有用户级指令中的专有名词、可用工具有哪些）。

## 交付

`tasks/REPORT-87.md`。
