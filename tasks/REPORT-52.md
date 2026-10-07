# REPORT-52 — 清理未发现的模型、引擎文案

## 改动文件

- `src/resources.rs`：新增 `PruneOutcome`、`ModelReference` 与
  `prune_missing_models_at(path, agent_filter, referenced)`（`store_lock` 内读改写，
  删 `meta` 带 `MISSING_MARKER` 的模型，被引用的照样删并单列；没有要删的不写盘）；
  文案三处：`NOT_DETECTED_NOTE` 改「最近一次探测未发现」，冒烟超时 note 改
  「连接测试超时，不影响派发」，pi 未登录 note 改「未登录；在 pi 里执行 /login … 后重新探测」。
- `src/console_cli.rs`：`resources prune [--agent <a>]` 命令体（`resources_prune_at` +
  `run_resources_prune`），stdout 为 `aicross resources` 形状外加 `removed` /
  `referenced`，stderr 写「已清理 N 个未发现的模型」与被引用说明；窗口共用实现体
  `resources_prune_command_at`；`ResourcesOps` 加 `prune_missing` 注入点；引用收集
  `referenced_model_targets`（modes.json 人选含备选 + roles.json 三角色，provider 空
  表示不限 provider）；`value_with_messages` 从 `outcome_value_with_messages` 提出共用；
  文案两处（roles.json / modes.json 严格读取拒写说明改「…；已停止写入，修复或删除该
  文件后重试」）。
- `src/server.rs`：窗口命令 `resources_prune(agent?)`（async + spawn_blocking），
  注册进 `invoke_handler`。
- `src/hosts.rs`：厂商依据（vendorBasis）文案：官方端点两条改「官方端点」；第三方
  端点与 provider 匹配的依据统一为「端点 <主机名或 provider>」（`finish_endpoint_match`
  去掉 basis_prefix 参数，直接给「端点 <主机名或原文>」；`finish_provider_match` 去掉
  field_label 参数）；「认不出厂商」改「· 厂商未识别」；kimi 未设 provider 改
  「未设 provider」；managed provider 依据改「provider <名>」；modes.json 拒写说明同上。
- `src/recommend.rs`：notes 文案：「不能保证跨厂商」改「无法保证跨厂商」；「候选池里
  没有与主 agent 厂商（X）不同且同阶段不重复的模型」改「无可用人选：需不同于主 agent
  厂商 X，且同阶段不重复」；「该位置将由角色默认 X 执行」改「使用角色默认 X」；
  「第 N 阶段有 2 个审查位置同为厂商 Y，要跨厂商时需手动调整」改「第 N 阶段 2 个审查
  位置同为 Y」（同阶段的「与主 agent 同为厂商 Y」一条同规则改）；modes.json 拒写说明同上。
- `docs/CONSOLE-CHANNEL.md`：资源命令清单加 `resources prune [--agent <a>]`；资源一节
  补 prune 说明（删除范围、输出结构、被引用的照样删、`effective.state` 变「不在资源
  库」、再探测会重新加入）；推荐一节引用的 notes 句子同步。

## 复用的现成函数

- `resources::is_missing_from_detection`（判定，不复制标记逻辑）、`resource_model_key`、
  `store_lock` / `load_or_default` / `save_store` / `ensure_agents`、
  `resources_value`（prune 的资源输出同 `aicross resources`）。
- `orchestration::modes_from` / `roles_from`（宽松读取，收集被引用的模型指向）。
- `engine` 派发前拦截与 `config show` 的 `effective.state` 不改：prune 删掉人选指向的
  模型后，它们按现有逻辑自然落到「不在资源库」，无需新代码。
- CLI 参数校验复用 `unknown_option_error` / `option_value` / `join_or_none`；
  窗口命令沿用 spawn_blocking 模式。

## 新增测试

resources.rs：

- `task52_prune_removes_only_missing_models`（只删带标记的；探测到与申报的保留；removed 键与落盘）
- `task52_prune_with_agent_filter_only_cleans_that_agent`（`--agent` 大小写不敏感只清一家）
- `task52_prune_lists_referenced_but_still_removes`（被引用的照样删并列 referenced；provider 空不限 provider）
- `task52_prune_then_detect_readds_rediscovered_model`（删后再探测经 merge 重新加入）

console_cli.rs：

- `task52_resources_prune_removes_missing_and_lists_referenced`（stdout 的 removed /
  referenced、stderr 两行说明、modes.json 人选引用、申报保留）
- `task52_resources_prune_agent_filter_and_argument_errors`（`--agent` 过滤、未知 agent
  退出 2、不认识的选项与缺值退出 2）

既有断言同步：console_cli（pi 未登录 note、冒烟超时 note、roles.json 拒写说明）、
resources（同三处）、hosts（官方端点依据、端点主机名依据）、recommend（三处 notes）。

## 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets
  → Finished `dev` profile；warning 计数 0
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --release
  → Finished `release` profile；warning 计数 0
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
  → test result: ok. 552 passed; 0 failed; 10 ignored
CARGO_TARGET_DIR=D:/work/aicross/target cargo test task52
  → 6 passed（上面列的 6 个新测试）
```

换行核对（python 字节统计）：`src/server.rs` 纯 CRLF（518 行），其余改动文件纯 LF；
未用 shell heredoc 写源码。

## 未做与理由

- 不改 `src-ui/**`、不 commit / push、不加依赖、不联网（任务边界）。
- 不改 `docs/MANUAL.md` 术语表：任务书只允许改 `src/**`、`docs/CONSOLE-CHANNEL.md`
  与本报告。
- 引擎派发前拦截、`config show` 状态、冒烟过滤逻辑不动：prune 删除后由现有
  `is_missing_from_detection` / 查找逻辑自然生效，删掉的模型再探测会重新加入
  （`merge_detected_models` 原行为，测试覆盖）。
- `resources prune` 不处理「额度用尽」「验证失败」等其它状态的模型：任务书范围只有
  「最近一次探测未发现」。
- 被引用模型删除后不自动清空 modes.json / roles.json 里的人选：任务书明确「照样删除，
  界面会标失效」，由 `effective.state`＝「不在资源库」呈现。

## 不确定处

- pi 未登录 note 的写法取「未登录；在 pi 里执行 /login <provider> 后重新探测」：任务书
  给的目标句是「未登录；运行 {login} 后重新探测」，{login} 原文是一句动宾短语，直接嵌入
  「运行 …」不通，改为把动作原样接在「未登录；」后、以「后重新探测」收尾。
- 「第 N 阶段有审查位置与主 agent 同为厂商 Y，要跨厂商时需手动调整」一句任务书未单列，
  按「审查位置同为厂商 Y」同一条的规则一并改为「第 N 阶段审查位置与主 agent 同为 Y」。
- 被引用的判断粒度：modes.json 人选或角色里 provider 未填（null / 空串）时，视为对
  agent + model 不限 provider 的引用（与派发补人的查找口径一致）；引用了未带标记模型
  的条目不影响输出。
