# 窗口复测修正（2026-10-07，集成树，空数据目录）

复测范围：TASK-57/58/59/60 与下拉定位、未验证过滤。由主线程直接修改。

## 发现与改法

1. **codex 隐藏模型进了推荐**（引擎，resources.rs）：`~/.codex/models_cache.json` 里 `visibility: "hide"` 的 `codex-auto-review`（自动审批审查用）、`gpt-5.5`、`gpt-reserve` 被收进资源库；资源库按 id 字母序存，推荐同分取靠前者，于是三个验证位都选了 `codex-auto-review`。
   - `parse_codex_cache` 跳过 `visibility == "hide"`，按 `priority` 升序。
   - `merge_detected_models` 同一 provider 内按事实源给出的顺序排，事实源里没有的排后、按 id。
   - 已存旧库里的隐藏模型在下次探测后带「未在事实源中」标记，不进候选池。
   - 测试 `codex_cache_skips_hidden_and_keeps_priority_order`。
2. **评审模式介绍不随视角变**（引擎，console_cli.rs）：`config show` 里 builtin-jury 的 `description` 改由各位置生效视角去重拼成「多个模型分别从X、Y、Z角度审查」。`task59_config_set_focus_writes_and_shows` 补断言。
3. **推荐提示过时**（界面，ModesPage.tsx）：点「去测试」时清掉推荐提示条，回来重新推荐时按新状态再出。

## 记下不改

- 未填位置的说明里「另有 N 个未验证模型，测试连接后可参与推荐」与下方「pi 有 N 个模型未验证 · 去测试」重复。命令行侧需要前一句，窗口暂不去重。
- 「登记」已存在但未登记的斜杠命令文件时按当前模板重写（因而带上 `disable-model-invocation`）；未登记的旧文件不计算 stale。

## 验证

- 引擎：cargo check --all-targets 零 warning；cargo test 643 通过。
- 界面：tsc、eslint 无输出；vitest 195 通过。
