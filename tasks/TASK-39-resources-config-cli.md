# TASK-39 资源与配置的命令行（10-01）

**只改 `src/**`、`docs/CONSOLE-CHANNEL.md`。不改前端，不删窗口界面。不 commit，不 push。**

## 背景

`docs/PRODUCT.md`「机制」第 6 条：资源接入是命令。现在探测、冒烟只能在窗口接入页里点，角色与派发设置也只能在窗口里改；「顺序」第 4 步要下线窗口界面，之前必须先有命令行替代。本期只加命令，窗口界面与它们的 Tauri 命令保持可用。

## 1. 命令

所有命令把 JSON 写到 stdout、一行说明写到 stderr，退出码沿用：0 成功 / 1 业务失败 / 2 参数错 / 3 引擎不可用（`aicross_root()` 不可用、资源库不可读写）。

- `aicross resources`：读资源库（`~/.aicross/resources.json`，经 `src/resources.rs` 现有读取函数，含其旧数据迁移与重判），不探测、不发模型请求。输出每个 agent：`id`、`name`、`version`、`state`；每个模型：`id`、`agent`、`provider`、`resolved_id`、`state`、`note`。
  - agent `state`：`可用`（已探测到）/ `未安装`（未探测到）。`note` 写下一步，例如未安装时「安装后运行 aicross resources detect」。
  - 模型 `state`：原生模型 `可用`；第三方模型按现有 `verification` 映射为 `未验证` / `已验证` / `验证失败`；pi provider 未登录时为 `未登录`，`note` 给登录命令（`pi auth login --provider <p>`，按 `pi --help` 实际为准）；`quota_exhausted_until` 未过期时为 `额度用尽`，`note` 写恢复时间。
- `aicross resources detect`：重新探测全部 agent（复用接入页「探测」用的同一函数，含 pi provider 登录检查，不读凭据），写回资源库，输出同上。
- `aicross resources smoke <agent> [--model <id>] [--provider <p>]`：对该 agent 的第三方模型做冒烟（复用接入页「冒烟」的同一函数，真发一次短请求、比对真身）；给了 `--model` 只冒烟这一个。原生模型不冒烟，报「原生模型无需冒烟」并退出 0。输出冒烟后的该 agent 条目。
- `aicross config show`：输出 `{ roles, dispatch }`——`roles` 来自 `~/.aicross/roles.json`（`orchestration` 现有读写），`dispatch` 来自 `~/.aicross/dispatch-settings.json`（TASK-36 的 `load_settings` 口径，含默认值与夹取）。
- `aicross config set-role <planning|execution|acceptance> --agent <a> --model <m> [--provider <p>] [--tier <t>] [--thinking <关|低|中|高>]`：校验目标在资源库里存在（agent + model，给了 provider 时一并匹配），不存在退出 2 并在 stderr 列出该 agent 下可选的模型；通过后写 `roles.json`，输出新的 `roles`。
- `aicross config set <key> <value>`：只认派发设置的键（与 `dispatch-settings.json` 字段同名：`timeoutSecs`、`concurrency`、`maxRequests`、`fixRoundLimit`、`swarmLimit`、`consoleRunLimit`、`validationCommand`、`redact`、`writeProjectDispatch`、`acpEnabled`、`ccSwitchPath`、`classificationModel`、`classifyProposals`）。数值与布尔按 JSON 写入，越界按 `load_settings` 的夹取规则夹到允许区间并在 stderr 说明；未知键退出 2。写入后输出新的 `dispatch`。

子命令加进 `src/console_cli.rs` 的分发（照现有 `dispatch/status/...` 的写法），不走窗口 HTTP。

## 2. 约束

- 不改资源库、`roles.json`、`dispatch-settings.json` 的格式（只读写既有字段）；窗口接入页与设置页照常可用。
- 探测与冒烟复用现有函数，不复制逻辑；需要时把 Tauri 命令里的同步部分抽成普通函数，Tauri 命令改为调用它。
- 不读凭据、不输出任何 token（密钥六铁律）。

## 3. 测试

- 状态映射做成纯函数测：agent 可用 / 未安装；模型 原生可用 / 未验证 / 已验证 / 验证失败 / 未登录 / 额度用尽（未过期与已过期）。
- 命令层：参数解析（缺参数、未知子命令、未知键、`--thinking` 非法值）、`set-role` 目标不存在退出 2 且列出候选、`set` 越界夹取、写入后再读一致；全部用临时目录作 `aicross_root`，不真探测、不真冒烟（探测与冒烟函数可注入）。
- 退出码 3 的来源各一条。

## 4. 文档

`docs/CONSOLE-CHANNEL.md` 命令行一节补这些命令、输出字段、状态取值与退出码。

## 验证

```
cargo check --all-targets        # 零 warning
cargo check --release
cargo test
```

每个改动文件保持原有换行（`src/server.rs`、`src/main.rs` 是 CRLF），新文件 LF。写 `tasks/REPORT-39.md`：改动文件、复用了哪些现成函数、测试名、三条命令实际输出、未做与理由。

## 编排者真机核（实现者不做）

本机运行 `aicross resources`、`aicross resources detect`、对一个 pi 第三方模型 `smoke`、`config show`、`set-role` 一次（再改回原值）、`config set timeoutSecs` 一次（再改回）。
