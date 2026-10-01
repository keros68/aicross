# REPORT-39 资源与配置的命令行

对应 `tasks/TASK-39-resources-config-cli.md`。分支 `cli/resources39`。未 commit。

## 做了什么

- `aicross resources`：读资源库（经 `get_resources_at`，含 `ensure_agents`、
  `ensure_verification`、`rejudge_stored_smoke_failures`、`ensure_resolved_id`、
  `clear_expired_quota` 的旧数据迁移与重判），不探测、不发模型请求。输出全部
  agent：`id`、`name`、`version`、`state`、`note`、`models`；每个模型：
  `id`、`agent`、`provider`、`resolved_id`、`state`、`note`。
- `aicross resources detect`：复用接入页「探测」的同一函数（`detect_all_resources_at`，
  含 pi provider 登录检查，不读凭据），写回资源库后输出同上；cc-switch 发现提示
  的路径取派发设置里的 `ccSwitchPath`。
- `aicross resources smoke <agent> [--model <id>] [--provider <p>]`：对该 agent 的
  第三方模型冒烟；给了 `--model` 只冒烟这一个（复用接入页单条「验证」的
  `smoke_resource_model_at`），没给则逐条批量（`smoke_agent_models_at` 的循环体
  抽出 `filter` 参数，新增 `smoke_agent_third_party_at` 只挑第三方）。原生模型
  不冒烟，stderr 报「原生模型无需冒烟」并退出 0。输出冒烟后的该 agent 条目。
- `aicross config show`：输出 `{ roles, dispatch }`——`roles` 来自
  `orchestration::roles_from`（`~/.aicross/roles.json`），`dispatch` 来自
  `engine::load_settings`（`~/.aicross/dispatch-settings.json`，含默认值与夹取）。
- `aicross config set-role <planning|execution|acceptance> --agent <a> --model <m>
  [--provider <p>] [--tier <t>] [--thinking <关|低|中|高>]`：校验目标在资源库里
  存在（agent 大小写不敏感、匹配后回写规范 id；给了 provider 一并匹配；同名模型
  挂多个 provider 且未指定时报错），不存在退出 2 并在 stderr 列出该 agent 下可选
  的模型；省略 `--tier` / `--thinking` 保持原值；经 `save_roles_in` 写
  `roles.json`，输出新的 `roles`。
- `aicross config set <key> <value>`：只认派发设置的 13 个键；数值与布尔按 JSON
  写入，夹取复用 `settings_from_value`（单键 map 取对应字段，区间与取整和
  `load_settings` 同一套），越界时 stderr 说明；未知键退出 2；保留文件里其余键
  不动；写入后输出新的 `dispatch`。
- 状态映射做成纯函数（`agent_state` / `agent_state_note` / `model_state`）：
  agent 可用 / 未安装；模型 原生可用 / 未验证 / 已验证 / 验证失败 / 未登录 /
  额度用尽。优先级：原生 → 未过期的额度用尽 → pi 未登录 → 验证结果（任务书未
  规定冲突取法，测试钉住）。
- 退出码：0 成功 / 1 业务失败（冒烟失败、roles 或设置写盘失败、设置文件损坏）/
  2 参数错 / 3 引擎根不可用或资源库不可读写。
- `docs/CONSOLE-CHANNEL.md` 命令行一节补「资源与配置」小节（命令、输出字段、
  状态取值、登录提示、退出码），退出码总表的 3 补「资源库不可读写」。

## 改动文件

| 文件 | 改动 |
|---|---|
| `src/console_cli.rs` | 子命令分发加 `resources` / `config`；`run_at` 拆 `run_at_with`（注入 `ResourcesOps`）；命令实现、输出组装、参数工具与测试 |
| `src/resources.rs` | `is_native_model` / `get_resources_at` / `detect_all_resources_at` / `smoke_resource_model_at` / `save_store` 改 `pub(crate)`；`smoke_agent_models_at` 抽 `filter` 参数并新增 `smoke_agent_third_party_at`；新增状态映射纯函数与测试 |
| `src/orchestration.rs` | `save_roles_in` 改 `pub(crate)`（命令行与窗口设置页共用），逻辑未动 |
| `docs/CONSOLE-CHANNEL.md` | 「资源与配置」小节与退出码表 |
| `tasks/REPORT-39.md` | 本文件 |

换行：`src/console_cli.rs`、`src/resources.rs`、`src/orchestration.rs`、
`docs/CONSOLE-CHANNEL.md` 保持 LF；`src/main.rs`、`src/server.rs`（CRLF）未改。

## 复用的现成函数

- `resources::get_resources_at`：接入页 `get_resources` 的同步部分（读取含迁移与重判）。
- `resources::detect_all_resources_at`：接入页「探测」按钮的同步部分（含
  `check_pi_provider_logins`，只读 pi 的退出码与顶层 status，不读凭据）。
- `resources::smoke_resource_model_at`：接入页单条「验证」的同步部分。
- `resources::smoke_agent_models_at` → `smoke_agent_models_filtered_at`：接入页
  批量「验证」的循环体，只加了 `filter` 参数；Tauri 命令与既有测试不受影响。
- `resources::is_native_model` / `VERIFICATION_*` / `ResourceLogin`：原生与登录判定。
- `engine::load_settings` / `settings_from_value` / `write_json_atomic`：设置的读、
  夹取与原子写。
- `orchestration::roles_from` / `save_roles_in`：roles.json 读写。
- `console_cli` 既有 `option_value` / `positional` / `dispatch_root`。

## 测试

`src/resources.rs`：

- `task39_agent_state_maps_detected_flag`：agent 可用 / 未安装与下一步说明。
- `task39_model_state_covers_native_verification_login_and_quota`：原生可用、
  未验证 / 已验证 / 验证失败、认不得的 verification 按未验证、未登录（含
  provider 为空的 note）、额度用尽未过期与已过期、额度优先于未登录。
- `task39_smoke_agent_third_party_respects_endpoint_kind`：claude 官方端点是原生
  不冒烟；挂第三方端点（`set_claude_third_party`）后要冒烟。

`src/console_cli.rs`（全部用临时目录作 root，探测与冒烟经 `ResourcesOps` 注入，
不真探测、不真冒烟）：

- `task39_resources_lists_states_without_probing`
- `task39_resources_detect_uses_injected_detector_and_passes_cc_switch`
- `task39_smoke_single_native_model_skips_request`
- `task39_smoke_single_third_party_success_and_failure`
- `task39_smoke_agent_batch_reports_failures`
- `task39_smoke_agent_without_third_party_exits_zero`
- `task39_smoke_rejects_bad_arguments`（缺参数、未知子命令、未知 agent / 模型、
  多 provider、未知选项、`--provider` 无 `--model`、候选清单内容）
- `task39_config_show_outputs_roles_and_dispatch`（缺省默认值、文件值、夹取口径）
- `task39_config_set_role_writes_canonical_target`（规范 id / provider、tier 与
  thinking 保持原值、写入后再读一致）
- `task39_config_set_role_rejects_bad_arguments`（缺参数、非法角色、非法
  `--thinking`、目标不存在列候选、多 provider、失败不写盘）
- `task39_config_set_clamps_writes_and_roundtrips`（越界夹取、按 JSON 类型写入、
  保留未知键、写入后再读一致）
- `task39_config_set_rejects_bad_input`（未知键、非数字、非布尔、多余参数、
  损坏文件不覆盖）
- `task39_resources_and_config_exit_3_sources`（引擎根不可用各一条、资源库损坏
  不覆盖）
- 既有 `run_subcommand_skips_parent_console_attach` 补 `resources` / `config`。

## 验证（命令与结果原文）

```
$ cargo check --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.97s
（无 warning）

$ cargo check --release
    Finished `release` profile [optimized] target(s) in 1m 11s
（无 warning）

$ cargo test
test result: ok. 580 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 61.50s
```

另跑过只读真机抽查（不属任务书验证项，未改任何文件）：

```
$ cargo run --quiet -- resources
{"agents":[{"id":"claude","models":[{"agent":"claude","id":"fable",...,"state":"可用"},...
stderr: 已读取资源库

$ cargo run --quiet -- config show
{"dispatch":{"acpEnabled":false,...,"timeoutSecs":600,...},"roles":{...}}
stderr: 已读取配置

$ cargo run --quiet -- resources smoke
stderr: 缺少 agent（退出 2）
```

## 未做与理由

- 真机核（`resources detect`、对 pi 第三方模型 `smoke`、`set-role` 与 `set` 后
  改回）按任务书由编排者做，实现者不做。
- 未登录的 note：任务书示例是 `pi auth login --provider <p>`，本机 pi 0.86.1 的
  `pi --help` 与 `pi auth --help` 没有 login 子命令（只有 check / print-api-key /
  print-bearer-token），按「按 `pi --help` 实际为准」改为「在 pi 里执行
  /login <provider>」（pi 文档口径：`/login` 是会话内命令，API key 走环境变量）。
- `resources detect` 的单 agent 探测失败（未检测到、事实源读取失败）退出码为 0，
  错误逐行写 stderr：「未安装」是有效探测结果，命令本身已把库写回；任务书未
  规定，此处取宽松口径并写进文档。
- stderr 的「一行说明」：`detect` / `smoke` 有多条错误时先逐行给错误、再给一行
  总结，不把多条错误挤进一行。
- `config set` 只写被设置的键并保留文件里其余键（含未知键），不把整个 dispatch
  全量写回：避免把默认值固化进文件，之后改默认值传播不到。
- `config set` 的负数值（如 `-5`）会被共用的 `positional` 助手当成选项前缀而报
  「缺少值」：沿用既有 CLI 的取值规则，未为此新增解析。
- 窗口设置页（浏览器本地存储）与 `dispatch-settings.json` 仍不同步：维持文档
  既有说明，本期不动窗口侧。
