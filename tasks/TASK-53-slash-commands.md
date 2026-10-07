# TASK-53 斜杠命令：默认调用方式与自定义短命令（引擎侧，10-06）

**只改 `src/**`、`docs/CONSOLE-CHANNEL.md`，以及新建 `tasks/REPORT-53.md`。不改前端。不 commit，不 push。** 在 TASK-52 未提交改动之上继续。设计见 `docs/PRODUCT.md`「在主 agent 里怎么用」（先读）。窗口侧是 TASK-53 的前端部分，按第 3 节契约并行开发。

## 1. 数据

`modes.json` 每个主 agent 段追加 `slashCommands: [{ "name": "cross-review", "mode": "builtin-blind", "note": "" }]`（缺省为空数组）。`name` 只允许小写字母、数字、`-`，1–32 位；`mode` 必须是存在的预设 id（别名解析同 `--preset`）；`note` 为可选的固定附加说明（单行，≤200 字）。

## 2. 写入的文件

| host | 路径（家目录可注入） | 文件内容 |
|---|---|---|
| claude | `~/.claude/commands/<name>.md` | frontmatter：`description`、`argument-hint: <任务>`、`aicross-managed: true`；正文见下 |
| codex | `~/.codex/prompts/<name>.md` | frontmatter：`description`、`argument-hint`、`aicross-managed: true`；正文见下 |
| pi | `~/.pi/agent/prompts/<name>.md` | frontmatter：`description`、`aicross-managed: true`；正文见下 |
| kimi | `~/.kimi-code/skills/<name>/SKILL.md` | frontmatter：`name`、`description`、`aicross-managed: true`；正文见下 |

正文（参数占位：claude / codex 用 `$ARGUMENTS`，pi 与 kimi 的参数会追加在指令后，不写占位）：

```
使用 ai-cross skill，按「<模式名>」处理下面的请求；派发时带 --host <host>。<note>
$ARGUMENTS
```

`description` 为「ai-cross · <模式名>」。

- 文件存在且 frontmatter 里没有 `aicross-managed: true` → 视为别人的，不覆盖，返回退出 1「已存在同名命令（非 aicross 管理）：<路径>」。
- 写入用临时文件 + rename；删除只删带标记的。

## 3. 命令与窗口命令

- `aicross slash list --host <h>`：`{ host, invoke: "<默认调用写法>", commands: [{ name, mode, modeName, note, path, state: "已写入" | "未写入" | "被占用", trigger: "<该 host 下的输入写法>" }] }`。默认调用写法：claude `/ai-cross`，codex `$ai-cross`，pi `/skill:ai-cross`，kimi `/skill:ai-cross`。`trigger`：claude `/<name>`，codex `/prompts:<name>`，pi `/<name>`，kimi `/skill:<name>`。`state` 按磁盘实际判断（文件存在且带标记 = 已写入；存在无标记 = 被占用；不存在 = 未写入）。
- `aicross slash add --host <h> --name <n> --mode <m> [--note <text>]`：校验后写入定义与文件；同名定义已存在则更新（覆盖我们自己的文件）。输出同 list。
- `aicross slash remove --host <h> --name <n>`：删除定义与带标记的文件。输出同 list。
- 窗口命令：`slash_list(host)`、`slash_add(host, name, mode, note?)`、`slash_remove(host, name)`（async + spawn_blocking），返回同结构。写 `modes.json` 与文件放在同一把进程内锁里。
- `--host` 规则同 TASK-50。

## 4. 测试

四家各自的路径与文件内容（含参数占位差异、kimi 为目录 + SKILL.md）；名称校验；模式不存在退出 2；被占用（无标记）不覆盖退出 1；更新覆盖自己的文件；remove 只删带标记的；list 的 state 三种；trigger 与 invoke 写法。家目录一律用临时目录，绝不写真实家目录。

## 5. 文档

`docs/CONSOLE-CHANNEL.md` 补 `aicross slash` 三个子命令、各家路径与写法、`modes.json` 的 `slashCommands`。

## 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # 零 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
```

换行规则同前。写 `tasks/REPORT-53.md`。
