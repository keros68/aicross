# REPORT-17-R1 接入页登录状态复审修正

## 逐条对应

1. 「重新检查」端到端与凭据不落盘
   - 改法：`check_pi_provider_login_at_with_binary` 允许测试注入假 pi 路径。假 pi stdout 同时带 `credentials` 和 `reason`，读回 `resources.json` 后逐字段核对只有 `login` / `login_checked_at` 改变，`errors` 为空，敏感串和字段名均未落盘。另用同一注入点跑通假 pi `--list-models` + `auth check` 探测、合并、保存整链。两份 `fixtures/resources/pi_auth_check_*.json` 已直接用于真机输出形状测试。
   - 测试：`task17_pi_login_check_uses_safe_args_once_and_ignores_credentials`、`task17_pi_detect_list_auth_merge_persists_without_auth_payload`、`task17_pi_status_fixtures_match_live_output_shape`。

2. 额度用尽与未登录去重、备选跳过
   - 改法：G1 以未登录优先，额度分区排除交集 id，未登录行标「额度同时受限」。引擎执行备选前跳过 `logged_out`，`runner_note` 记跳过路由；若没有任何后续备选被执行，`fallback_skipped` 记「未登录」。若后续备选成功，仅保留 note，避免结果界面误称「未切备选」。G1 备选下拉和资源页备选顺序编辑器均给未登录模型加后缀。
   - 测试：「同时未登录与额度用尽时只进未登录分区，且 G1 接线会默认取消勾选」、「备选顺序编辑器给未登录模型标记」、`task17_r1_logged_out_fallback_is_skipped_and_recorded`。

3. 混合登录状态不整组降级
   - 改法：provider 组内有任一 `logged_in` 就保留在正常列；全部 `unknown` 才收入未检查桶。正常列中的 `unknown` / `logged_out` 模型行各自显示状态。
   - 测试：「混合 provider 保留已登录列，只给未检查的模型标记」。

4. 探测路径的登录检查移到锁外
   - 改法：单 agent 与全部探测都先解析 pi、执行 `--list-models` 和 provider 登录检查，然后获取资源库写锁做合并与原子保存。
   - 测试：`task17_pi_login_probe_runs_before_store_lock`；假 pi 在 auth 阶段暂停时直接检查 store mutex 仍可获取。

5. 命令缺失时给说明
   - 改法：重新检查找不到 pi 时，`errors` 返回「未找到 pi，登录状态未检查」，并仍更新状态与检查时间。左栏和接入页在 `login_checked_at` 非空且状态为 `unknown` 时显示「登录状态未检查」。
   - 测试：`task17_recheck_missing_pi_reports_and_persists_unknown`、「已执行检查但仍未知时显示登录状态未检查」。

6. pi 未检测到时刷新旧登录状态
   - 改法：探测时 pi 二进制缺席，库中该 agent 的全部模型改为 `unknown`，并写入本次检查时间，不保留旧「已登录」。
   - 测试：`task17_missing_pi_detection_clears_stale_login`。

7. `focus-visible`
   - 改法：两种登录折叠行与两处 provider 「重新检查」按钮补 `outline: 2px solid var(--accent)`。
   - 测试：「登录折叠行与重新检查按钮有键盘焦点环」。

8. 测试卫生与 G1 行为接线
   - 改法：假 pi 用脚本所在目录的固定 `argv.log`，不再 `set_var/remove_var`。G1 新增 `initialResourceBlockedSelection`，组件初始选择直接走该函数，测试从 graph + models 验证未登录目标默认不勾；去掉 logged-out 接线会立即变红。
   - 测试：「登录检查测试不用进程级环境变量传参」、「同时未登录与额度用尽时只进未登录分区，且 G1 接线会默认取消勾选」。

9. 报告同步
   - 改法：`REPORT-17.md` 已更正为真机 `status` 形状，记录两份既有前端测试为何补 `logged_in` 夹具，补齐当前测试清单、能红记录和复跑数字。
   - 测试：「REPORT-17 记录真机 status 形状、既有测试调整与当前清单」。

## 能红记录

- 前端新测试先行运行得到 `7 failed | 6 passed`，逐项命中第 2、3、5、7、8、9 条的旧行为。
- 后端临时反向注错：重新检查恒返回 unknown、登录检查移回锁内、pi 缺席保留 logged_in、不跳过未登录备选。`cargo test task17 -- --nocapture` 得到 `4 failed | 7 passed`，分别命中第 1、4、6、2 条；恢复后 `11 passed`。
- 所有临时注错均已恢复，最终工作树不包含反向代码。

## 验证结果

### `cargo check --all-targets`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-ui)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.28s
```

退出码 0，无 warning。

### `cargo test`

```text
running 425 tests
test result: ok. 414 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 18.39s
```

退出码 0。11 项 ignored 为仓库既有真机或显式本地事实源测试。

### `cd src-ui && npx vitest run --configLoader native --pool threads`

```text
Test Files  33 passed (33)
Tests  509 passed (509)
Duration  1.04s
```

退出码 0。输出含既有 `NO_COLOR` / `FORCE_COLOR` 提示，不影响测试。

### `cd src-ui && npm run build`

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build
vite v8.0.7 building client environment for production...
✓ 337 modules transformed.
✓ built in 933ms
```

退出码 0。

## 换行与边界

- 改前：`src/server.rs` CRLF 4667 / 裸 LF 0；`src/resources.rs` CRLF 0 / 裸 LF 4073；`src-ui/src/tauri.ts` CRLF 576 / 裸 LF 114。
- 改后：`src/server.rs` CRLF 4667 / 裸 LF 0；`src/resources.rs` CRLF 0 / 裸 LF 4321；`src/dispatch.rs` CRLF 0 / 裸 LF 9494；`src-ui/src/tauri.ts` CRLF 576 / 裸 LF 114。其他改动文件也保持纯 LF。
- 未运行真实 pi，未发真机请求；所有新增登录链路测试都使用假 pi。真机输出形状来自编排者已有实测记录与仓内 fixture，本轮未重跑真机。
- 未读取任何凭据文件，未使用 `--credentials`，未记录 `credentials` 或 `reason`。
- 未运行 `cargo fmt`，未启动窗口，未 commit，未 push，未修改 `D:\work\aicross` 主工作树。
- 反向注错期间的 panic 在 Windows Temp 留下 6 个专用测试目录（`aicross-pi-*` / `aicross-login-fallback-*`）；已核对目标，但当前沙箱策略拒绝递归删除，未绕过策略处理。它们不在仓库或主工作树内，只含假 CLI 脚本、标记文件与临时 `resources.json`。
- 无没做或部分完成项；第 2 条的 `fallback_skipped` 仅在「所有可用备选都因未登录而没有执行」时写入，这是为了不在后续备选成功时显示错误的「未切备选」说明；每个跳过事件都会写入 note。
