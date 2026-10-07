# TASK-54 每个模式的斜杠命令（引擎侧，10-07）

**只改 `src/**`、`docs/CONSOLE-CHANNEL.md`，新建 `tasks/REPORT-54.md`。不改前端。不 commit，不 push。** 在 TASK-53 未提交改动之上继续。

用户 10-07 定：每个模式都要有斜杠命令，默认前缀 `ax-`。

## 1. 模式对应的短名与关键词

内置预设固定映射（写成一张表，放在 `slash.rs`）：

| 预设 id | 短名 slug | 带参数写法里的关键词 |
|---|---|---|
| builtin-shared | shared | 共享 |
| builtin-economy | economy | 经济 |
| builtin-blind | blind | 盲审 |
| builtin-arena | arena | 擂台 |
| builtin-jury | jury | 评审 |
| builtin-swarm | swarm | 蜂群 |

用户自定义预设不参与一键生成（没有稳定的短名），仍可手动添加。

## 2. `slash list` 输出追加 `modes`

`modes: [{ mode, modeName, argForm, suggestedName, command }]`，按上表顺序列出内置模式：

- `argForm`：默认调用写法 + 空格 + 关键词，如 claude `/ai-cross 盲审`、codex `$ai-cross 盲审`、pi / kimi `/skill:ai-cross 盲审`。
- `suggestedName`：`ax-<slug>`。
- `command`：`commands` 里 `mode` 等于该模式的第一条（同 `commands[]` 结构），没有为 `null`。

## 3. 一键生成

- 命令行 `aicross slash add-all --host <h> [--prefix <p>]`，`--prefix` 缺省 `ax-`，须满足名称规则（小写字母、数字、`-`），且拼出的每个名字都要过 `validate_name`。
- 对上表每个模式：该模式已有命令（任何名字）→ 跳过；否则用 `<prefix><slug>` 调现有 `slash_add` 逻辑写入。单个失败（如被占用）不中断，记入 `skipped: [{ name, reason }]`。
- 输出同 `slash list` 加 `added: [name]`、`skipped`。窗口命令 `slash_add_all(host, prefix?)`（async + spawn_blocking）。
- 写入与 modes.json 更新沿用 TASK-53 的锁、写入顺序与回滚。

## 4. 测试

四家 `argForm` 写法；`suggestedName`；`command` 匹配；add-all 全新生成 6 条；已有某模式命令时跳过该模式；被占用时记入 skipped 且其余照常；非法前缀退出 2。家目录一律临时目录。

## 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # 零 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
```

换行规则同前；文档同步。写 `tasks/REPORT-54.md`。
