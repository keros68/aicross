# TASK-17 R1 复审报告

**总判：可交付**。9 条修正全部落实，实现与测试互相钉得住，解析器（status 优先、ready 兼容）未被改回。限制：本会话无 shell 工具，四条验证命令无法由我复跑，「测试通过」的结论基于逐行读实现与测试 + 两份报告一致的复跑数字（cargo 414 passed / 11 ignored；vitest 509 passed；build 0 退出）。

## 逐条核对

1. **已修** — 注入点 `check_pi_provider_login_at_with_binary`（src/resources.rs:2325）；真端到端测试 `task17_pi_login_check_uses_safe_args_once_and_ignores_credentials`：假 pi stdout 带 `credentials`/`reason`（resources.rs:2961、2972），走真写库后读回落盘原文断言不含 `do-not-read`、不含 `credentials`（resources.rs:3033-3035），再以整结构体相等断言除 `login`/`login_checked_at` 外全未变（resources.rs:3040-3043）；`errors` 为空有断言（resources.rs:3032）。探测→合并→落盘整链有 `task17_pi_detect_list_auth_merge_persists_without_auth_payload`（resources.rs:3064-3103）。两份 fixture 已用进 `task17_pi_status_fixtures_match_live_output_shape`（resources.rs:3054、3058）。
2. **已修** — 前端：`blockedNodePartitions` 未登录优先、额度区排除交集（DispatchOverlay.tsx:139-148），未登录行标「未登录，额度同时受限」（DispatchOverlay.tsx:642），测试钉住去重与接线（resources-login-17.test.tsx:169-183）。引擎：候选带 login（dispatch.rs:3278、3287），`LoggedOut` 候选跳过并写 note（dispatch.rs:3331-3334、3372-3374），无一执行时 `fallback_skipped="未登录"`（dispatch.rs:3374-3376）；真实 stub 测试断言只调了 p0、`fallback_skipped` 与 note（dispatch.rs:7464-7497）。备选标记：编辑器（ResourcesOverlay.tsx:392、400）与 G1 下拉（DispatchOverlay.tsx:716、750），测试 resources-login-17.test.tsx:185-192。
3. **已修** — `splitPiProviderGroups`：有 `logged_in` 即进正常列，全 `unknown` 才进未检查桶（types.ts:283-288）；正常列内非 logged_in 的行各自标状态（ResourcesTab.tsx:443-445、ResourcesOverlay.tsx:516-517）；测试 resources-login-17.test.tsx:53-71（含渲染断言「未检查」）。
4. **已修** — 单 agent 与全部探测都先 `prepare_pi_detection`（含 `--list-models` + 各 provider 登录检查）再取锁（resources.rs:2263-2264、2289-2290）；`task17_pi_login_probe_runs_before_store_lock` 在假 pi auth 阶段 `try_lock` 成功断言锁空闲（resources.rs:3179-3194）。「重新检查」路径本就先查后锁（resources.rs:2334-2338）。
5. **已修** — 缺 pi 时 `errors` 带「未找到 pi，登录状态未检查」且仍落 unknown + 时间（resources.rs:2334-2352；测试 resources.rs:3106-3122）；界面 `loginCheckHint`（types.ts:299-303）在 unknown 但有检查时间时显示，接入页（ResourcesOverlay.tsx:652）与左栏（ResourcesTab.tsx:318）都有；渲染测试 resources-login-17.test.tsx:126-138。
6. **已修** — pi 二进制缺席时 `apply_prepared_pi_detection` 把该 agent 全部模型刷成 unknown 并写检查时间（resources.rs:868-877）；测试 `task17_missing_pi_detection_clears_stale_login`（resources.rs:3125-3140）。
7. **已修** — `.resource-logged-out-toggle`/`.agent-card-logged-out-toggle` 与两处 `.row-action` 补 2px `--accent` 焦点环（resources.css:419-423、450-454），照同文件既有口径；CSS 断言 resources-login-17.test.tsx:210-214。
8. **已修** — 假 pi argv 日志改脚本同目录固定文件（resources.rs:2953-2959），全文件 grep 无 `set_var`/`AICROSS_PI_ARGS_LOG` 残留，前端有源码卫生断言（resources-login-17.test.tsx:218-221）。G1 接线改走 `initialResourceBlockedSelection`（DispatchOverlay.tsx:150-158，组件 ：165 实际使用），行为级测试 ：169-183——去掉 loggedOut 接线该测试会红（返回 `['both']` ≠ `[]`）。
9. **已修** — REPORT-17.md 补真机 status 形状（:13）、既有测试改动及原因（:34）、测试清单（:17-25）、能红记录（:46-54）、复跑数字（:56-95）；报告中测试名与实际函数逐一核对一致。

## 新发现的问题（按严重度）

- **（低）`detect_into` 的 pi 分支成为生产死代码且保留旧语义** — src/resources.rs:818-830（`let Some(binary) = resolved else { return; }` 不刷登录）。两处生产入口都已改走 prepared 路径，该分支仅测试可达（resources.rs:4268）。若日后有人恢复从 `detect_into` 走 pi，第 6 条会无声回归。建议后续清理或注明，不阻塞本期。
- **（低）引擎跳过测试依赖 PATH 有 python** — dispatch.rs:7465 缺 python 时静默 return，该环境引擎跳过逻辑不被验证。与仓内既有 stub 测试惯例一致（dispatch.rs:6035 等），非新风格。
- **（观察）跳过说明不进留痕文件** — 「跳过 x（未登录）」与 `fallback_skipped` 是在 `run_one_with_activity` 层加到返回结果上的（dispatch.rs:3372-3377），而主尝试留痕写于 `run_one_attempt` 内（dispatch.rs:4134），故留痕 JSON 不含该说明；与既有 quota note 聚合口径相同，结果界面可见。
- **（观察）`status:"ready"` 但退出码非 0 判 LoggedOut 而非 Unknown**（resources.rs:1163-1166，`bad-exit` 用例钉住）。真机 ready 恒配退出码 0，矛盾输出按未登录处理是有意选择，仅提示口径。
- **（观察）引擎每候选各做一次全量读库取 login**（dispatch.rs:3278、3287 → `resource_model_login` 每候选 load 一次），与既有 `resource_quota_until` 同口径，候选多时有多次读盘。
- **（事实披露，非代码问题）** 注错期间的 6 个临时目录留在 Windows Temp（REPORT-17-R1.md:96 已声明，仓外、无害）。

## 核过没问题的点

- **解析器未被改回**：`pi_login_from_check`/`pi_auth_ready` 仍是 `status` 优先、`ready` 布尔兼容、退出码参与就绪判定（resources.rs:1158-1193），fixture 测试钉住真机形状与退出码（resources.rs:3048-3061）。
- **凭据卫生**：argv 逐字 `auth check --provider <p> --json --no-refresh`、去重后恰一次、无 `--credentials`（resources.rs:3014-3023）；探测合并链同样断言落盘无 `do-not-read`/`credentials`（resources.rs:3099-3101）。
- **锁与并发**：并发上限 4 的真实线程峰值测试仍在（resources.rs:3200-3220）；缺命令/超时 → unknown 仍在（resources.rs:3224-3243）。
- **G1 收敛**：去重不改变并集口径，`convergeQuotaSelection` 仍允许触碰后勾回（DispatchOverlay.tsx:217-219）；models 晚到时收敛 effect 兜住初始选择。
- **宽松反序列化仍在**：脏值 → unknown 不炸整库（resources.rs:59-68 + 测试 ：3246-3252）。
- **报告一致性**：REPORT-17.md 与 REPORT-17-R1.md 的测试名、能红记录、复跑数字互相一致；R1 报告对 `fallback_skipped` 只在「无后续备选执行」时写入的取舍有明确说明（REPORT-17-R1.md:97）。