# TASK-56 内置模式的位置数量可调、介绍改写（引擎侧，10-07）

**只改 `src/**`、`docs/CONSOLE-CHANNEL.md`，新建 `tasks/REPORT-56.md`。不改前端。不 commit，不 push。** 在 TASK-55 未提交改动之上继续。

用户 10-07：模式介绍不要口语（不写「一家 / 三家」，写厂商 / 模型）；内置模式写死 2、3 个位置不合适，要能自定义数量。

## 1. 介绍改写

`builtin_presets()` 的 `description` 改为：

| id | description |
|---|---|
| builtin-shared | 单个模型执行，可读取完整对话上下文；只读 |
| builtin-economy | 高档模型出方案，低档模型执行，其他厂商的模型验收 |
| builtin-blind | 同一份材料由多个厂商的模型独立审查，结果互不可见 |
| builtin-arena | 多个模型分别完成同一任务，从结果中选定一份采纳 |
| builtin-jury | 多个模型分别从安全、性能、需求覆盖角度审查 |
| builtin-swarm | 任务拆分为互不重叠的子任务，由多个模型并行完成后合并 |

## 2. 可调数量

- `modes.json` 每个主 agent 段追加 `counts: { "builtin-blind": 2, … }`（缺省为空，空则用默认）。
- 可调表（写成一张表，放在 `orchestration.rs`）：

  | 预设 | 可调的位置 | 范围 | 默认 |
  |---|---|---|---|
  | builtin-economy | 第 3 阶段的「验证」 | 0–3 | 1 |
  | builtin-blind | 「验证」（另有 1 个「验收」固定） | 1–4 | 2 |
  | builtin-arena | 「执行」 | 2–4 | 3 |
  | builtin-jury | 「验证」 | 1–3 | 3 |

  共享、蜂群不可调。用户自定义预设不走这里（它们的结构由编辑器决定）。
- 生效方式：派发与 `config show` 取预设时，按该主 agent 的 `counts` 把可调角色的位置数改成设定值（增加的位置复制该角色的模板节点；减少的从末尾去掉）。必须改在一个共用函数里（如 `shape_builtin_preset(preset, count)`），派发、`config show`、`fill_seats`、推荐、斜杠命令各处都经它取预设形状，不各写一份。
- 位置文本：盲审的验证位置都用现有的验证路由文本；评审的视角文本按「安全 → 性能 → 需求覆盖」依次取（`preset_node_texts` 改为按位置数生成，不再按固定下标）；经济模式第 3 阶段「验收」之后的验证位置都用验证路由文本，数量为 0 时只剩「验收」。
- seats：数量减少时，`modes.json` 该模式超出的人选在写入 counts 时一并去掉（复用 `shape_seats_to_preset` 的整理）；数量增加时新位置为空，由用户选或一键推荐填。
- 命令：`aicross config set-count --host <h> <模式> <数量>`（别名同 `--preset`；不可调的模式、越界退出 2，信息给出范围）。输出该模式条目（同 `config show` 的 `modes[i]`）。`config show` 的 `modes[]` 追加 `count: { role, value, min, max, default }`（不可调的模式为 null）。窗口命令 `config_set_count(host, mode, count)`。
- `aicross config recommend` 按调整后的形状推荐。

## 3. 测试

介绍文本；每个可调模式的上下限与默认；增减后 `config show` 的阶段形状、位置文本（评审视角按序、盲审与经济的验证文本）；减少时 seats 被裁剪；派发按调整后的形状建节点；不可调模式与越界退出 2；推荐按新形状填。全部用临时目录。

## 4. 文档

`docs/CONSOLE-CHANNEL.md` 补 `config set-count`、`counts` 字段、可调表。

## 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # 零 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
```

换行规则同前。写 `tasks/REPORT-56.md`。
