# REPORT-07 — 资源接入并回 fork

实现日期：2026-09-13。任务书：`tasks/TASK-07-resources-return.md`。本轮未启动 Tauri 窗口（任务书要求），未 commit、未 push。当前分支 `ui/resources`。

## 改了什么

### Rust（`src/resources.rs`，整体重写；102 行契约读取 → 完整资源接入）

- 数据契约沿用第 1 期（`port/src-tauri-old/src/resources.rs`），字段名不改：`ResourceStore` / `ResourceAgent` / `ResourceModel`（13 个字段全 serde default）。补齐第 1 期契约里 `ResourceAgent` 的 `kind` / `detected` / `version`（serde default，兼容旧文件），`ResourceStore` / `ResourceAgent` 补 `Serialize`（要回传前端）。
- `load()` 保持原语义（文件缺失/损坏都报错，`orchestration.rs` 依赖它区分「尚未接入」）；命令层新增 `load_or_default()`（缺失视为空仓库，损坏报错不覆盖）。写盘用原子写（同目录临时文件 + rename，照抄 `orchestration.rs:254` 的 `write_json_atomic` 做法）。
- 探测：claude `--version` + opus/sonnet/haiku 预设（source `文档(2026-08-30)`）；codex 读 `~/.codex/models_cache.json`；kimi 读 `~/.kimi-code/config.toml` 的 `[models]` 段，**id 保留完整别名（含 `kimi-code/` 前缀）**——channels 要求 kimi 模型必须含 `/`；pi 读 `~/.pi/agent/models-store.json` ∩ `pi --list-models`（逐 token 精确匹配），逐 provider 跑 `pi auth check --provider <p> --json` 标鉴权未就绪。合并逻辑保留用户字段（tier/status/冒烟结果/vendor/note），新条目进「待冒烟」，消失的标「未在事实源中」（申报条目不标）。
- **cc-switch 降级为发现提示**：`ensure_agents` 不再建 cc-switch agent，并迁移删除历史 store 里的 cc-switch agent；`connected()` 过滤 `agent != "cc-switch"`（不再是可派发通道）；探测只在 `ccSwitchPath`（派发设置项，前端从 `loadDispatchSettings()` 读出作为命令参数传入，空则跳过）非空时跑 `python cc_switch.py list`，产生 `CcSwitchHint{name, has_token, models}` 供前端显示「cc-switch 里有 X，建议加进 pi」。不产生可派发条目。
- 冒烟：复用第 3 期 `channels::build_invocation`（blind=true、prompt 固定「只回复 OK」、session_dir 指向 `~/.aicross/sessions/smoke`）+ `process::run_process`（60s）+ `channels::parse_output`（真身核对已在 channels 里，pi 的 403 `AccessDenied.Unpurchased` 经 `stopReason=="error"` 进错误）。claude 第三方端点（进程环境 `ANTHROPIC_BASE_URL` 非空）比对响应体 `model`：identity 非空且不包含请求 id 判「响应模型不符」（别名兼容）。成功 → `已接入` + `smoked_at` + `smoke_model`；失败 → `冒烟失败` + `smoke_failure = safe_summary(...)`（sk-/URL 凭据/query secret/Bearer/环境变量值脱敏，截 200 字）。手动 agent 冒烟被 channels 拒（「不支持的派发 agent」）。
- 二进制解析：channels 的 `resolve_binary` 是私有的且 channels 不许改，在 resources.rs 写了本地 `resolve_cli`（Windows `where.exe name.exe/name.cmd/name.bat` 取首行，Unix `which`；kimi 额外 fallback `~/.kimi-code/bin/kimi.exe`）。
- 手动 agent：二进制与启动参数存 `~/.aicross/tools.json`（key = `manual-<uuid>`，复用 `tool_config.rs`），名字存 resources.json；登记后立即探测一次。
- Tauri 命令（9 个，函数在 resources.rs，注册只追加进 `src/server.rs` 的 `generate_handler!` 末尾）：`get_resources`、`detect_resource_agent(agent_id, cc_switch_path)`、`detect_all_resources(cc_switch_path)`、`add_resource_model`、`update_resource_model`、`remove_resource_model`、`add_manual_agent`、`smoke_resource_model`、`smoke_agent_models(agent_id)`。探测/冒烟/手动添加走 async + `spawn_blocking`；所有写操作 load → 改 → 原子写 → 返回新 store。
- 新增测试：pi 三 provider（deepseek / qwen-token-plan-cn / zai-coding-cn）`--list-models` 解析、kimi 别名保留完整前缀、cc-switch 只产生发现提示且 `connected()` 排除 cc-switch、`ensure_agents` 迁移。第 1 期的合并/解析测试并回（fixture 路径改为 `fixtures/resources/`，`ProcessResult` 构造器补 `truncated` 字段）；第 1 期的 smoke 解析测试未并回——其覆盖的行为（真身不符、403 error event）由第 3 期 channels 测试用自己的 fixture 等价覆盖，对应三个 `pi_smoke_*.jsonl` fixture 在修正轮 R1 删除。

### 前端

- `src-ui/src/resources/types.ts`：契约类型（snake_case，与 Rust 一致）+ 纯函数（`connectedByAgent`、`resourceSummary(Text)`、`groupModelsByProvider`、`agentMark`、`parseCommandArgs`、`errorText`）。
- `src-ui/src/resources/mention.ts`：`insertMention(agent, model)` 派发 `aicross:insert-mention`；`openResourcesPage()` 派发 `aicross:open-resources`；`notifyResourcesChanged()` 派发 `aicross:resources-changed`；`appendMention`（草稿拼接，与 GraphTab 共用）。
- `src-ui/src/components/resources/ResourcesTab.tsx`：左栏「资源」tab——只列 `已接入` 模型，按 agent 折叠分组（组头 agent 名+版本，模型行显示 id 与来源），点模型把 `@agent/模型` 插进当前对话图主面板输入框，栏底「N 个待冒烟 · M 个未接入 · 管理 →」。
- `src-ui/src/components/resources/ResourcesOverlay.tsx` + `resources.css`：接入页全屏覆盖层（与「讲解」ExplainOverlay 同形态：portal 到 body、右上角「关闭」）。agent 卡片网格：标记（id 前两位大写）、名称、版本、检测状态、全部模型与状态，卡底「探测 / 冒烟 / 手动输入」；模型详情里可改厂商/档位、单条冒烟/移除；cc-switch 提示条；底部「手动添加 agent」。样式用 fork 主题变量（--bg-panel/--text-1..3/--accent/--green/--red）。
- `src-ui/src/tauri.ts`：commands 末尾追加 9 个 wrapper（import 资源类型）。
- `src-ui/src/components/left/Explorer.tsx`：tab union 加 `'resources'`，无存值默认 `'resources'`；tab 栏加「资源」按钮（排第一）；内容区三分支渲染。
- `src-ui/src/components/common/TitleBar.tsx`：齿轮左侧加「接入」图标按钮（内联 SVG 插头，strokeWidth 1.8），本地 open state + 监听 `aicross:open-resources`，挂载 ResourcesOverlay。
- `src-ui/src/components/center/graph/GraphTab.tsx`（**禁区文件，只加不改**）：追加一个 useEffect 监听 `aicross:insert-mention`，把文本经 `appendMention` 追加到 `drafts[0]`（主面板列）。既有 drafts/发送/@弹层逻辑未动。
- `src-ui/src/i18n/zh-CN.ts` / `en.ts`：各加 `explorer.tab.resources`、`resources.open` 两个 key（其余 9 种语言经 useT 回退 en）。
- `src-ui/src/resources/__tests__/resources.test.ts`：9 个用例（18 个断言）覆盖汇总计数、分组、标记、参数解析、mention 拼接。

### fixtures 与依赖

- `port/src-tauri-old/fixtures/*`（11 个）全部搬到 `fixtures/resources/`；新增 `pi_models_store_three.json` + `pi_list_models_three.txt`（三 provider）。
- `Cargo.toml` 加 `toml = "0.9"`（kimi 事实源是 TOML；fork 原来没有这个 crate）。

## 自动验证（命令与输出原文）

### `cargo check`（根目录，exit=0）

```text
   Compiling aicross v0.1.0 (D:\work\aicross-ui)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.75s
```

### `cargo test`（根目录，exit=0，138 项；resources 用例与收尾原文）

```text
running 138 tests
test resources::tests::add_model_requires_provider_for_pi ... ok
test resources::tests::ensure_agents_drops_cc_switch_and_seeds_builtins ... ok
test resources::tests::detection_refreshes_meta_preserves_note_and_parses_commented_kimi_toml ... ok
test resources::tests::kimi_alias_keeps_full_prefix ... ok
test resources::tests::live_codex_and_kimi_fact_sources_parse ... ignored, explicit local fact-source check
test resources::tests::live_detect_four_agents ... ignored, 真机验收：探测四个内置 agent（内存 store，不写 resources.json）
test resources::tests::live_smoke_pi_models ... ignored, 真机验收：冒烟两条 pi 模型（2 次模型请求；内存 store，不写盘）
test resources::tests::merge_preserves_user_fields_adds_pending_and_marks_missing ... ok
test resources::tests::pi_source_matching_uses_exact_tokens_and_auth_uses_top_level_status ... ok
test resources::tests::parses_four_local_fact_sources ... ok
test resources::tests::pi_three_providers_parse_from_list_models ... ok
test resources::tests::safe_summary_redacts_common_cli_secret_shapes ... ok
test resources::tests::connected_filters_status ... ok
test resources::tests::cc_switch_produces_hints_only_no_dispatchable_models ... ok

test result: ok. 131 passed; 0 failed; 7 ignored; 0 measured; 0 filtered out; finished in 4.10s
```

### `cd src-ui && npx vitest run`（exit=0）

REPORT-03 记录过本机 `npx vitest run` 的 `spawn EPERM`；本次未复现，一次通过，未用 fallback。

```text
 Test Files  9 passed (9)
      Tests  75 passed (75)
   Start at  21:36:10
   Duration  357ms (transform 48%, import 31%, tests 13%, worker 8%)
```

### `cd src-ui && npm run build`（exit=0；chunk 体积警告为既有现象）

```text
✓ built in 3.74s
```

## 真机实测

以 `cargo test -- --ignored --nocapture --test-threads 1` 跑三个 `#[ignore]` 验收测试。探测与冒烟都用内存 store，**没有写 `~/.aicross/resources.json`**（不碰用户真实数据）；冒烟的 session 落在 `~/.aicross/sessions/smoke`（冒烟自身需要）。密钥未读任何凭据文件，全部子进程（pi）自己处理。

输出原文：

```text
test resources::tests::live_codex_and_kimi_fact_sources_parse ... live codex models=8
live kimi models=4
ok
test resources::tests::live_detect_four_agents ... agent=claude detected=true version=2.1.270 (Claude Code) models=3
agent=codex detected=true version=codex-cli 0.153.4 models=8
agent=kimi detected=true version=0.42.0 models=4
agent=pi detected=true version=0.85.1 models=5
ok
test resources::tests::live_smoke_pi_models ... smoke pi/zai-coding-cn/glm-5.3-flash: status=已接入 smoke_model=zai-coding-cn/glm-5.3-flash failure=
smoke pi/qwen-token-plan-cn/deepseek-v4-flash: status=冒烟失败 smoke_model= failure=403: {"message":"Access to model denied. Please make sure you are eligible for using the model.","id":"4042e106-98a2-49f0-b386-93323f741e6f","type":"AccessDenied.Unpurchased","code":"AccessDenied.Unpu
ok
```

- 探测四 agent：全部 detected=true，版本与模型数如上（claude 3 条预设；codex 8；kimi 4；pi 5）。
- 冒烟 `pi/zai-coding-cn/glm-5.3-flash`：**通过**（已接入，真身 `zai-coding-cn/glm-5.3-flash`）。
- 冒烟 `pi/qwen-token-plan-cn/deepseek-v4-flash`：**如期失败**，`smoke_failure` 显示 403 `AccessDenied.Unpurchased`（尾部截断是 `safe_summary` 的 200 字上限，符合设计）。

两点如实说明：

1. 命令用了 `--ignored` 全量过滤，连带跑了第 3 期的 `dispatch::tests::live_blind_review` / `live_shared_continuation`，多产生 5 次模型请求（3+2，均成功）。本期自己的验收是 2 次模型请求，在 ≤4 预算内；连带部分属操作失误，不是功能行为。
2. 同一命令下既有的 `shell_probe::tests::finds_msix_pwsh_alias`（#[ignore]）失败：本机 pwsh 是真实安装（`C:\Program Files\PowerShell\7\pwsh.exe`）而非 MSIX 别名，与本期改动无关，默认 `cargo test` 不含它。

## 没做什么

- 未动 `src-ui/src/graph/`、`components/center/graph/` 逻辑、`components/right/orchestration/`、`src/dispatch.rs`、`src/channels/`（GraphTab.tsx 见下，只加了一个事件监听）。
- 未起窗口（未跑 `tauri dev`），界面未做真窗截图验收。
- 未做 cc-switch 可派发通道（任务书定为降级）；`SettingsModal.tsx` 未改（ccSwitchPath 设置项第 3 期已有）。
- 新界面文案硬编码中文，未进 11 语言字典（与第 2/6 期口径一致，第 5 期统一处理）。
- 「全部探测」一键按钮、左栏无对话图时的点模型反馈，初版未做，修正轮 R1 已补（见末节）。

## 不确定处

- `detect_resource_agent` / `detect_all_resources` 每次都会顺带刷新 cc-switch 提示（一次 `python cc_switch.py list`，20s 超时兜底）；ccSwitchPath 为空时完全跳过。
- 冒烟 thinking 固定传「低」（pi → `--thinking low`，claude → `--effort low`）；若某模型只支持特定 effort，冒烟会失败并显示原因，用户可在接入页看到。
- 手动 agent 只允许录入模型、不能冒烟（channels 拒「不支持的派发 agent」），与第 1 期行为一致。

## 与 phase/4 分支可能冲突的文件

当前 `git diff --stat main phase/4` 为空（两分支无差异），以下按「phase/4 将来会动」的预期列出：

- `src/server.rs`：只在 `generate_handler!` 末尾（约 3845–3853 行）追加 9 行命令注册。
- `src-ui/src/tauri.ts`：追加一处 import（约 93–100 行）与 commands 对象末尾 9 个 wrapper（约 346–369 行）。
- `src-ui/src/components/left/Explorer.tsx`：4 处——import（17 行）、activeTab union 与默认值（约 633–641 行）、tab 按钮（约 743–748 行）、内容区分支（约 781–783 行）。
- `src-ui/src/components/common/TitleBar.tsx`：import + open state（约 10–30 行）、「接入」按钮（约 128–145 行）、末尾挂载 overlay（约 205 行）。
- `src-ui/src/components/center/graph/GraphTab.tsx`：**禁区文件，纯追加**——一行 import（22 行）+ 一个 useEffect 监听（约 50–62 行），不改任何既有逻辑。
- `src-ui/src/i18n/zh-CN.ts` / `en.ts`：各加 2 行 key。
- `Cargo.toml` / `Cargo.lock`：加 `toml = "0.9"` 一行。
- 纯新增（无冲突面）：`src-ui/src/resources/`、`src-ui/src/components/resources/`、`fixtures/resources/`。

## 修正轮 R1（2026-09-13）

修正单：`tasks/fix-07-r1.md`（前两条来自编排者窗口实测，其余来自 `tasks/review-07-glm.md`）。范围约束遵守：未动 `src-ui/src/graph/`、`components/right/orchestration/` 的既有逻辑、`src/dispatch.rs`、`src/channels/`；`GraphTab.tsx` 与 `OrchestrationPanel.tsx` 各只追加一个事件监听。本轮未起窗口，未 commit、未 push。

### 逐条处理

1. **pi 探测只读到 5 个模型 —— 已修**。`parse_pi_sources` 改为以 `pi --list-models` 为主：逐行解析清单（表格式 `provider model [context]` 跳表头，或单 token `provider/model`），清单里的模型全部收入；`models-store.json` 只用来补 `contextWindow`/`reasoning` 元数据，读不到或损坏按空处理、不再拦截探测（`src/resources.rs` 的 `detect_pi`/`parse_pi_sources`/`parse_pi_list_models`）。补测试 `pi_list_models_is_primary_store_only_supplies_metadata`：list-models 有、store 没有的模型必须出现在结果里，store 有的补元数据，坏 store 不拦截。真机复验见末节：`agent=pi detected=true models=31`（修正前 5）。
2. **探测写盘后右栏不刷新 —— 已修**。`GraphTab.tsx` 与 `OrchestrationPanel.tsx` 各追加一个 `aicross:resources-changed` 监听，触发重新调 `listDispatchableModels`；@ 弹层、「尚未接入资源」提示、节点面板模型下拉都吃这两处 models state。均为纯追加（新 import + 新 useEffect），不改既有逻辑。
3. **并发写 resources.json 无锁 —— 已修**。新增进程级 `store_lock()`（`static OnceLock<Mutex<()>>`，中毒时取内层值），所有写命令按「持锁 → load → 改 → 原子写」执行；命令体拆成 `*_at(path, …)` 自由函数便于测试。`smoke_agent_models` 改为每条冒烟都重新 load → 改 → 写盘（锁覆盖整条序列），不再整批长持旧 store。补测试 `smoke_agent_models_partial_failure_writes_each_result`：两条模型一成一败 → store 一已接入一冒烟失败，errors 恰含一条，且失败条已落盘。
4. **cc-switch 提示条被空 hints 覆盖 —— 已修**（前端方案）：`finishOperation` 在 `cc_switch_hints` 为空数组时保留旧值，不再清空提示条。
5. **claude 第三方端点判定 —— 已修**。`claude_third_party_endpoint()` 除进程环境 `ANTHROPIC_BASE_URL` 外，还读 `~/.claude/settings.json` 的 `env.ANTHROPIC_BASE_URL`；两处都没有（官方端点）时，冒烟成功会在模型 meta 追加「未核对真身（官方端点）」，不再静默跳过。补测试 `claude_base_url_read_from_settings_env`。
6. **死接口与死文件 —— 已修**。接入页顶部（导语行右侧）加「全部探测」按钮，接 `detectAllResources`，busy 态全按钮禁用复用现有机制。三个 `pi_smoke_*.jsonl` fixture 已删除；本报告 Rust 一节改口为「第 1 期 smoke 解析测试未并回，其覆盖行为由第 3 期 channels 测试等价覆盖」。
7. **补测试 —— 已修**，三个全补：`declared_models_are_not_marked_missing`（申报 source 的消失条目不标「未在事实源中」，对照组 `读:old` 条目仍标）；`corrupted_store_blocks_writes_and_keeps_bytes`（损坏的 resources.json 上 `load_or_default` 报错不覆盖，add/update 报错且文件字节不变）；以及第 3 条的批量冒烟部分失败测试。
8. **报告口径 —— 已修**。前端「18 个用例」改为「9 个用例（18 个断言）」；「没做什么」补「新界面文案硬编码中文，未进 11 语言字典（与第 2/6 期口径一致，第 5 期统一处理）」。
9. **add_manual_agent 吞探测错误 —— 已修**。命令返回类型改为 `ResourceOperation`，探测 errors 随结果返回；前端 `ManualAgentDialog.onSaved` 改收 `ResourceOperation`，错误显示在接入页错误条。
10. **resolve_cli 漂移 —— 已修**。注释记下已知漂移（kimi `.cmd` 探测能过、冒烟会被 channels 拒，channels 解禁后应提 `resolve_binary` 为 `pub(crate)` 收敛）；kimi 解析顺序改为先 `~/.kimi-code/bin/kimi.exe` 再 PATH，与 channels 的 fallback 路径一致。
11. **claude 未检测时不合并 —— 已修**。未检测到 claude 时也走一次空合并（`merge_detected_models(store, "claude", vec![])`），只给既有条目刷「未在事实源中」标记、不加新预设条目，与 codex/kimi 走同一条合并路径。
12. **`as any` 与硬编码底色 —— 已修**。`Explorer.tsx` / `TitleBar.tsx` 里本期引入的 `t('explorer.tab.resources' as any)` / `t('resources.open' as any)` 去掉 `as any`（key 已在 en/zh-CN，是合法 `I18nKey`；全仓其余 `as any` 是第 2 期前遗产，不在本期范围）。`.resource-status.connected/.failed` 底色改 `color-mix(in srgb, var(--green/--red) 12%, transparent)`，跟随主题 token。
13. **write_json_atomic 重复 —— 已修（注释互指）**。`src/resources.rs` 与 `src/orchestration.rs` 两份的 doc 注释互相指向对方并注明「收敛留第 5 期」；orchestration.rs 只加注释，逻辑未动。
14. **无对话图时点模型无反馈 —— 已修**。`ResourcesTab` 读 `state.terminals` 判断是否有 `tool === 'graph'` 的 tab；没有时模型行 `disabled` 置灰，title 提示「先打开对话图」，新增 `:disabled` 样式。

### 不采纳（维持修正单结论）

- 移除手动 agent 的途径：本期需求外，记 Later。
- 三处中文状态字符串并存（Rust / TS / i18n）：第 1 期遗产，契约不改。

### 自动验证（命令与输出原文）

#### `cargo check`（根目录，exit=0）

```text
   Compiling aicross v0.1.0 (D:\work\aicross-ui)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.20s
exit=0
```

#### `cargo test`（根目录，exit=0，143 项；resources 用例与收尾原文）

```text
test resources::tests::add_model_requires_provider_for_pi ... ok
test resources::tests::claude_base_url_read_from_settings_env ... ok
test resources::tests::ensure_agents_drops_cc_switch_and_seeds_builtins ... ok
test resources::tests::declared_models_are_not_marked_missing ... ok
test resources::tests::merge_preserves_user_fields_adds_pending_and_marks_missing ... ok
test resources::tests::pi_list_models_is_primary_store_only_supplies_metadata ... ok
test resources::tests::pi_source_matching_uses_exact_tokens_and_auth_uses_top_level_status ... ok
test resources::tests::kimi_alias_keeps_full_prefix ... ok
test resources::tests::pi_three_providers_parse_from_list_models ... ok
test resources::tests::parses_four_local_fact_sources ... ok
test resources::tests::detection_refreshes_meta_preserves_note_and_parses_commented_kimi_toml ... ok
test resources::tests::connected_filters_status ... ok
test resources::tests::cc_switch_produces_hints_only_no_dispatchable_models ... ok
test resources::tests::safe_summary_redacts_common_cli_secret_shapes ... ok
test resources::tests::corrupted_store_blocks_writes_and_keeps_bytes ... ok
test resources::tests::smoke_agent_models_partial_failure_writes_each_result ... ok

test result: ok. 136 passed; 0 failed; 7 ignored; 0 measured; 0 filtered out; finished in 4.21s
```

（136 passed 含 resources 模块 16 个非 ignore 用例；7 个 ignored 为既有 live 验收测试。）

#### `cd src-ui && npx vitest run`（exit=0）

```text
 Test Files  9 passed (9)
      Tests  75 passed (75)
   Start at  22:14:50
   Duration  393ms (transform 48%, import 31%, tests 12%, worker 9%)
```

#### `cd src-ui && npm run build`（exit=0；chunk 体积警告为既有现象）

```text
✓ built in 2.25s
```

### 真机复验（修正轮 R1）

`cargo test live_detect_four_agents -- --ignored --nocapture`（探测不产生模型请求；内存 store，不写 resources.json）：

```text
agent=claude detected=true version=2.1.270 (Claude Code) models=3
agent=codex detected=true version=codex-cli 0.153.4 models=8
agent=kimi detected=true version=0.42.0 models=4
agent=pi detected=true version=0.85.1 models=31
test resources::tests::live_detect_four_agents ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 142 filtered out; finished in 5.65s
```

pi 从修正前的 5 个变为 31 个（`--list-models` 为主后，deepseek / qwen-token-plan-cn / zai-coding-cn 三个 provider 全量收入，与修正单描述的本机清单一致）。探测写盘后右栏刷新、提示条保留、置灰提示三项是界面行为，本轮未起窗口，留给编排者窗口复验。
