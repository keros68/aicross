# TASK-55 模式介绍与自定义模式（引擎侧，10-07）

**只改 `src/**`、`docs/CONSOLE-CHANNEL.md`，新建 `tasks/REPORT-55.md`。不改前端。不 commit，不 push。** 在 TASK-54 未提交改动之上继续。

用户 10-07：每个模式要有介绍；要有用户自定义模式的入口。

## 1. 介绍字段

- `Preset` 追加 `description: String`（serde default，空串不写出）。内置预设的介绍写死在 `builtin_presets()`：

  | id | description |
  |---|---|
  | builtin-shared | 一家执行，能看到完整对话上下文；只读 |
  | builtin-economy | 高档模型出方案，低档模型执行，另一家验收 |
  | builtin-blind | 同一份材料交给三家独立审查，彼此不可见 |
  | builtin-arena | 三家各做一份，从中选一份采纳 |
  | builtin-jury | 三家分别从安全、性能、需求覆盖三个角度审查 |
  | builtin-swarm | 拆成互不重叠的小任务，多家并行完成后合并 |

- `config show` 的 `modes[]` 追加 `description`；`slash list` 的 `modes[]` 追加 `description`。

## 2. 自定义模式的增删改

用户预设存在现有的 `presets.json`（`raw_presets_from` 读取，内置预设不落盘）。新增：

- 命令行：
  - `aicross preset save --file <json>`：json 为一个预设对象 `{ id?, name, description?, stages: [{ nodes: [{ role, visibility, write, thinking? }] }] }`。没有 `id` 时新建（生成 uuid），有 `id` 时更新同 id 的用户预设。输出保存后的预设与完整 `modes`（同 `config show` 的 `modes`，需 `--host` 时按 TASK-50 规则）。
  - `aicross preset remove <id>`：删除用户预设；同时删除 `modes.json` 各主 agent 下该预设的 `seats`，默认模式指向它的清空，`slashCommands` 里 `mode` 为它的定义删除且删除对应的带标记文件（复用 slash remove 逻辑）。输出删除结果与受影响项列表。
- 窗口命令：`preset_save(preset)`、`preset_remove(id)`（async + spawn_blocking），返回同结构。
- 校验（不通过退出 2，信息写明哪一条）：
  - `name` 非空，≤40 字，不与其它预设（含内置）重名；不能修改或删除 `builtin: true` 的预设。
  - 至少一个阶段，每阶段至少一个位置，全模式最多 12 个位置。
  - `role` 只认 出方案 / 执行 / 验收 / 验证 / 拆分；`visibility` 只认 共享 / 盲；`write` 只认 只读 / 单写者 / worktree；`thinking` 只认 关 / 低 / 中 / 高 / 空串。
  - 同一阶段最多一个「单写者」位置（同一目录同时只有一个写者）。
  - 位置的 agent / model 不在这里设（人选走 `set-seat`）；传入的 agent / model 忽略。
- 写盘用现有原子写；读改写加锁（与 modes 写锁同一把或独立一把，避免与 seats 清理交错时丢更新——preset remove 需要同时改 presets.json 与 modes.json，按「先改 modes.json 与 slash 文件，再删预设」的顺序，任一步失败停止并返回已完成的部分）。
- 已有的派发对用户预设的支持不变（`--preset <id>`）。

## 3. 测试

介绍字段输出；新建 / 更新 / 删除；删内置被拒；重名被拒；各字段非法值；同阶段两个单写者被拒；位置数上限；删除时 seats、默认模式与 slash 定义的级联清理（slash 文件用临时家目录）；派发能用新建的自定义预设（补人照常）。全部用临时目录。

## 4. 文档

`docs/CONSOLE-CHANNEL.md` 补 `aicross preset save / remove`、预设 json 格式、校验规则、级联清理。

## 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # 零 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
```

换行规则同前。写 `tasks/REPORT-55.md`。
