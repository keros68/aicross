# fix-17-r1 接入页登录状态（TASK-17）复审修正轮

依据 GLM-5.3-Flash 与 DeepSeek 对 ui/phase17 的盲审（`tasks/review-17-glm.md`、`tasks/review-17-ds.md`）。DeepSeek 的阻塞项（解析只认 `ready` 布尔，真机输出是 `{"status":"ready"|"not_ready"}`）与宽松读盘已由编排者修好（`b389f9e`，真机核过三个 provider）。以下为剩余项。契约只追加。

## 必修

1. **「重新检查」端到端 + 凭据不落盘的真断言（DeepSeek 2）**：给 `check_pi_provider_login_at` 加注入点（假 pi 路径/runner），跑一条走假 pi（stdout 带 `credentials` 与 `reason`）→ 读回落盘 JSON，断言只多了 `login` / `login_checked_at`、不含 `do-not-read`、`errors` 为空；再加一条假 pi `--list-models` + auth 的探测合并落盘测试。把 `fixtures/resources/pi_auth_check_*.json` 用进测试或删掉。
2. **额度用尽与未登录双分区去重（DeepSeek 4 / GLM 3）**：G1 里同一节点只出现一行——未登录优先，额度用尽区排除已在未登录区的 id；未登录区那一行注明「额度同时受限」；引擎切备选前跳过 `login == logged_out` 的候选并写 `fallback_skipped`/note；备选顺序编辑器对未登录模型标「未登录」。
3. **混合登录状态的 provider 不整组降级（DeepSeek 5）**：组内有 logged_in 就按已登录列，只有未检查的那几行单独标「未检查」；全部 unknown 才进「未检查」桶。
4. **探测路径的登录检查挪到锁外（DeepSeek 6 / GLM 2）**：与 `check_pi_provider_login_at` 同口径——先查后锁；或分批查完即落盘。
5. **命令缺失时给说明（DeepSeek 7）**：`errors` 带「未找到 pi，登录状态未检查」；界面在 `login_checked_at` 更新但状态为未检查时显示一行说明。
6. **pi 未检测到时刷新登录（GLM 4）**：探测路径 pi 缺席时把该 agent 全部模型的 login 刷成 unknown，不留旧「已登录」。
7. **focus-visible（DeepSeek 8）**：折叠行与「重新检查」补 2px `--accent` 焦点环，照同文件既有规则。
8. **测试卫生（GLM 5、6）**：登录检查测试不用进程级环境变量传参（改固定临时文件或 stdin）；G1 接线补一条行为级断言（去掉 loggedOutIds 会红）。
9. **报告同步（DeepSeek 9 / GLM 1）**：REPORT-17 补真机输出形态来源、改过的既有测试及原因、当前测试清单与复跑数字。

验证：`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。保持文件原有换行（`src/server.rs` CRLF；`src-ui/src/tauri.ts` 混合只做字节级最小改动）；不 `cargo fmt`；不起窗口、不发真机请求；不 commit、不 push。报告写 `tasks/REPORT-17-R1.md`（逐条对应本清单）。
