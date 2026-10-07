# TASK-52 清理未发现的模型、引擎文案（10-06）

**只改 `src/**`、`docs/CONSOLE-CHANNEL.md`，以及新建 `tasks/REPORT-52.md`。不改前端。不 commit，不 push。** 在 TASK-50 未提交改动之上继续。

## 1. 清理未发现的模型

- 命令行 `aicross resources prune [--agent <a>]`：从资源库删除最近一次探测未发现（`meta` 含 `MISSING_MARKER`）的模型；用户申报的模型不删。给了 `--agent` 只清这一家。输出清理后的资源（同 `aicross resources`）与 `removed`（被删的 `agent/provider/model` 列表）；stderr 写「已清理 N 个未发现的模型」。在 `store_lock` 内读改写。
- 被 `modes.json` 人选或 `roles.json` 角色引用的模型照样删除（之后这些人选的 `effective.state` 会是「不在资源库」，界面会标失效），在输出里另列 `referenced`（被引用的那些），stderr 一并说明。
- 窗口命令 `resources_prune(agent?)`（async + spawn_blocking），返回同结构。
- 测试：只删带标记的；申报的保留；`--agent`；被引用的列在 `referenced`；删后再探测发现会重新加入。

## 2. 引擎文案（显示到界面上的）

按「中性、简短的说明体；事实 + 后果，可加出路；不用口语、不用劝导」改写：

| 位置 | 现在 | 改为 |
|---|---|---|
| `hosts.rs` claude 官方 | 未设 ANTHROPIC_BASE_URL，按官方端点推定 | 官方端点 |
| `hosts.rs` codex 官方 | 未设第三方 model_provider，按官方端点推定 | 官方端点 |
| `hosts.rs` 第三方端点 | （URL / provider 为 …） | 端点 <主机名或 provider> |
| `hosts.rs` | provider 为 {provider} | provider {provider} |
| `hosts.rs` | {…}，认不出厂商 | {…} · 厂商未识别 |
| `hosts.rs` | kimi provider 未设置 | 未设 provider |
| `hosts.rs` / `recommend.rs` | {message}，为避免覆盖已停止；请先修复或删除该文件 | {message}；已停止写入，修复或删除该文件后重试 |
| `recommend.rs` | 主 agent 厂商未知，不能保证跨厂商 | 主 agent 厂商未知，无法保证跨厂商 |
| `recommend.rs` | 候选池里没有与主 agent 厂商（X）不同且同阶段不重复的模型 | 无可用人选：需不同于主 agent 厂商 X，且同阶段不重复 |
| `recommend.rs` | 该位置将由角色默认 X 执行 | 使用角色默认 X |
| `recommend.rs` | 第 N 阶段有 2 个审查位置同为厂商 Y | 第 N 阶段 2 个审查位置同为 Y |
| `resources.rs` `NOT_DETECTED_NOTE` | 最近一次探测没有发现这个模型；在对应 agent 里确认登录后重新探测 | 最近一次探测未发现 |
| `resources.rs` 冒烟超时 note | 可重试 aicross resources smoke …；不影响派发 | 连接测试超时，不影响派发 |
| `resources.rs` pi 未登录 note | {login}；登录后运行 aicross resources detect 刷新 | 未登录；运行 {login} 后重新探测 |

其余引擎里会到达界面的字符串（错误信息、notes、basis、note）按同一规则自查一遍；命令行专用的参数错误提示可保留命令写法。相关测试断言同步更新。`docs/CONSOLE-CHANNEL.md` 同步。

## 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # 零 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
```

换行规则同前。写 `tasks/REPORT-52.md`。测试一律用临时目录。
