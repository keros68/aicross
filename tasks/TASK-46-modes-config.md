# TASK-46 模式人选与默认模式（10-06）

**只改 `src/**`、`docs/CONSOLE-CHANNEL.md`。不改前端（`src-ui/**`）。不 commit，不 push。**

## 背景

`docs/PRODUCT.md`「编排配置器」：用户在窗口里为每个模式的每个位置选人、选一张默认模式，存成配置；ai-cross skill 读这份配置照办，执行交给 `aicross dispatch`。本期做引擎侧：配置文件、派发时按它补人、命令行与窗口命令。窗口页面是 TASK-47。

现状：经济、共享、蜂群模式的空位置按 `roles.json` 的三个角色补（`src/engine/mod.rs` 的 `apply_role_defaults`）；盲审、擂台、评审模式整份跳过，所以 `aicross dispatch --preset blind` 会以「下一阶段有节点未指定模型，请先选择目标。」被拒。本期之后，这几种模式也能按配置派发。

## 1. 配置文件 `modes.json`

位置 `<aicross_root>/modes.json`（与 `roles.json` 同目录，`AICROSS_HOME` 生效）。格式（字段名 camelCase）：

```json
{
  "version": 1,
  "defaultMode": "builtin-blind",
  "seats": {
    "builtin-blind": [
      [
        { "agent": "pi", "provider": "zai-coding-cn", "model": "glm-5.3-flash", "tier": "", "thinking": "中",
          "fallback": { "agent": "claude", "provider": null, "model": "sonnet" } },
        null,
        { "agent": "codex", "provider": null, "model": "gpt-5.6-luna", "tier": "", "thinking": "", "fallback": null }
      ]
    ]
  }
}
```

- `seats[<预设 id>]` 是「阶段数组 → 位置数组」，下标与该预设 `stages[i].nodes[j]` 一一对应；`null` 表示这个位置没设人选。内置预设与 `presets.json` 里的用户预设都可以有。
- 人选字段：`agent`、`model` 必填；`provider` 可为 `null`；`tier`、`thinking` 空串表示沿用预设的默认值；`thinking` 只认 `关` / `低` / `中` / `高` / 空串；`fallback` 可为 `null`，有值时 `agent`、`model` 必填。
- `defaultMode` 可缺省或为空串。
- 读取分两种（照 `roles_from` / `roles_from_strict` 的分法）：派发与展示用宽松读取，文件缺失、损坏或 `version` 不认都按空配置处理；命令行写入用严格读取，文件损坏或版本不认时退出 1、不覆盖。写盘用现有的原子写（`write_json_atomic`）。

## 2. 派发时补人

规则做成一个纯函数，派发与展示（第 3 节 `config show`）共用同一个，不各写一份：

1. 预设节点的 `model` 非空（用户预设里写死的人选）：不动。
2. 否则 `modes.json` 里这个位置有人选：写入 `agent`、`provider`、`model`；`tier`、`thinking` 非空时覆盖预设值；`fallback` 有值时写入 `Target.fallback`。
3. 否则沿用现有的 `apply_role_defaults` 规则（含它对擂台 / 盲审 / 评审的整份跳过）。
4. 以上都没有：保持空，由现有检查以「下一阶段有节点未指定模型，请先选择目标。」拒绝。

`seats` 里的阶段数或位置数与预设对不上时（预设被改过），多出的忽略、缺的按 `null`。

**默认模式**：`aicross dispatch` 没给 `--preset`、也没有直给 targets 时，用 `defaultMode`；它为空或指向不存在的预设时用 `builtin-economy`（现有行为）。run 文件的 `preset_id` 记实际用的那个。找出所有写死 `builtin-economy` 作缺省的地方一起改（至少 `start_dispatch_with` 与 `plan_dispatch` 两处），不要只改一处。

## 3. 命令行

照现有 `run_config_set_role` 的写法加进 `src/console_cli.rs`。输出约定沿用：JSON 到 stdout，一行说明到 stderr；退出码 0 / 1 / 2 / 3 同 `docs/CONSOLE-CHANNEL.md`。

- `aicross config show`：在现有 `{ roles, dispatch, acp }` 上追加 `defaultMode`（实际生效的预设 id，规则同第 2 节）与 `modes`。`modes` 按 `raw_presets_from` 的顺序列出全部预设，每项：

  ```json
  { "id": "builtin-blind", "name": "盲审模式", "builtin": true, "default": true,
    "stages": [[
      { "role": "验收", "visibility": "盲", "write": "只读",
        "seat": { …modes.json 里的原样… } ,
        "effective": { "agent": "pi", "provider": "zai-coding-cn", "model": "glm-5.3-flash",
                       "tier": "", "thinking": "中", "fallback": { … } },
        "source": "位置" }
    ]] }
  ```

  `source` 取 `预设`（第 2 节规则 1）/ `位置`（规则 2）/ `角色`（规则 3）/ `未指定`（规则 4）；`未指定` 时 `effective` 为 `null`，`seat` 没设时为 `null`。
- `aicross config set-default-mode <模式>`：`<模式>` 认 `shared` / `economy` / `blind` 别名与预设 id（与 `--preset` 同一个解析函数）。不存在退出 2，stderr 列出可选的 id 与名称。写入后输出 `{ defaultMode, modes }`（同 `config show` 里这两项）。
- `aicross config set-seat <模式> <阶段> <位置> --agent <a> --model <m> [--provider <p>] [--tier <t>] [--thinking <关|低|中|高>] [--fallback-agent <a> --fallback-model <m> [--fallback-provider <p>]]`：
  - `<阶段>`、`<位置>` 从 1 开始数（命令行给人看），存盘按 0 起的下标。
  - 模式不存在、阶段或位置超出该预设范围、`--thinking` 非法值、只给了 `--fallback-agent` 或 `--fallback-model` 之一、`--fallback-provider` 没有配套的另两项：退出 2。
  - 人选与备选都要在资源库里存在，校验复用 `set-role` 的同一函数（agent + model，给了 provider 时一并匹配）；不存在退出 2 并列出该 agent 下可选的模型。
  - 写入后输出该模式那一项（同 `config show` 的 `modes[i]`）。
- `aicross config clear-seat <模式> [<阶段> <位置>]`：给了位置只清这一个（置 `null`），没给清掉该模式全部人选（删掉 `seats` 里这个键）。输出同 `set-seat`。

## 4. 窗口命令

在 `src/server.rs` 加 Tauri 命令并在注册处登记，返回值与对应命令行的 stdout JSON 同一结构，错误时返回 stderr 那行说明（`Result<serde_json::Value, String>`）。直接调用第 3 节背后的函数，不拉起子进程、不重复写逻辑：

| 命令 | 对应 |
|---|---|
| `config_snapshot()` | `{ "resources": <aicross resources 的输出>, "config": <aicross config show 的输出> }` |
| `resources_detect()` | `aicross resources detect` |
| `resources_smoke(agent, model?, provider?)` | `aicross resources smoke` |
| `config_set_default_mode(mode)` | `aicross config set-default-mode` |
| `config_set_seat(mode, stage, index, seat)` | `aicross config set-seat`；`stage`、`index` 从 1 开始；`seat` 是第 1 节的人选对象 |
| `config_clear_seat(mode, stage?, index?)` | `aicross config clear-seat` |

探测与冒烟会跑很久（单次冒烟最长 180 秒），这两个命令用 `async` + `tauri::async_runtime::spawn_blocking`，不能卡住窗口主线程。

## 5. 约束

- 不改 `roles.json`、`presets.json`、`resources.json`、`dispatch-settings.json` 的格式与现有行为；`apply_role_defaults` 的规则不改。
- 不读凭据、不输出任何 token（密钥六铁律）。
- 不写宿主的任何配置文件。

## 6. 测试

- 补人纯函数：四种 `source` 各一条；`tier` / `thinking` 空串不覆盖、非空覆盖；`fallback` 写入；`seats` 与预设长度对不上（多、少）；用户预设写死的人选不被覆盖；盲审模式有人选时能派出（不再被「未指定模型」拒绝）、没有人选时仍被拒。
- 默认模式：未设 → 经济；设了盲审 → 盲审，run 文件 `preset_id` 为 `builtin-blind`；指向不存在的预设 → 经济；显式 `--preset` 优先于默认模式。
- 读取：宽松读对缺失 / 损坏 / 错版本回空配置；严格读对损坏 / 错版本报错，命令行退出 1 且文件内容不变。
- 命令行：`config show` 含 `defaultMode` 与 `modes`；`set-default-mode` 别名与 id 都认、未知退出 2；`set-seat` 每个退出 2 的来源各一条；`set-seat` 写入后 `config show` 读回一致；`clear-seat` 两种用法。全部用临时目录作 `aicross_root`，不真探测、不真冒烟。
- 窗口命令：至少 `config_snapshot` 与 `config_set_seat` 各一条，验证返回结构与命令行一致（可直接测命令背后的函数）。

## 7. 文档

`docs/CONSOLE-CHANNEL.md`「资源与配置」一节补：`modes.json` 的位置与格式、补人顺序、默认模式、四条新命令（含输出字段与退出码）。开头第 5–7 行的窗口说明改为「窗口承载资源、模式与最近的派发三页」。

## 验证

```
cargo check --all-targets        # 零 warning
cargo check --release
cargo test
```

每个改动文件保持原有换行（`src/server.rs`、`src/main.rs` 是 CRLF，其余 Rust 是 LF），新文件 LF。写 `tasks/REPORT-46.md`：改动文件、复用了哪些现成函数、测试名、`config show` / `set-seat` / `set-default-mode` 在临时目录里的实际输出、未做与理由。

## 编排者真机核（实现者不做）

本机 `aicross config set-seat blind 1 1 …` 设三个位置，`config set-default-mode blind`，对一个小项目不带 `--preset` 派发一次，确认 run 的 `preset_id` 为 `builtin-blind`、三路按人选派出；最后把默认模式与人选恢复原样。
