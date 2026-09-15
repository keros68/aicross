# REPORT-17 接入页：pi provider 登录状态与等高布局

## 逐条实现

1. 资源契约与兼容
   - `ResourceModel` 只追加 `login` 与 `login_checked_at`。Rust 使用 `ResourceLogin`，序列化值为 `unknown` / `logged_in` / `logged_out`；缺字段时 serde 默认 `unknown`。
   - 前端资源模型与派发模型同步追加三态联合类型，旧数据和异常值按「未检查」展示。
   - 测试：`resources::tests::task17_old_resource_json_defaults_login_to_unknown`、`resources::tests::task17_dirty_login_value_reads_as_unknown_instead_of_failing`。

2. pi provider 登录检查
   - 探测到 pi 模型后，按 provider 去重，每个 provider 只运行一次 `pi auth check --provider <p> --json --no-refresh`。
   - 检查按每批最多 4 个并发执行，单次超时 10 秒；命令缺失或超时记为 `unknown`。
   - 本机 pi 0.85 于 2026-09-15 的实测输出形状为 `{"status":"ready"|"not_ready","provider":…,"authType":…}`；解析器只声明顶层 `status` 与兼容用 `ready`，并结合退出码判断。两份真机形状 fixture 已直接进入测试。
   - 实现未传 `--credentials`，不声明、不读取、不保存 `credentials` 与 `reason`；单 provider 重新检查和探测合并均有假 pi 端到端落盘断言。
   - pi 模型清单与 provider 登录检查在资源库写锁外完成，仅合并与保存持锁；`detect_all_resources` 与单个 pi 探测都刷新登录状态。pi 未检测到时，旧状态刷为 `unknown`。
   - 新增 `check_pi_provider_login(provider)` 命令，供单个 provider 的「重新检查」使用。
   - 测试：
     - `resources::tests::task17_pi_login_check_uses_safe_args_once_and_ignores_credentials`
     - `resources::tests::task17_pi_login_checks_cap_concurrency_at_four`
      - `resources::tests::task17_pi_login_missing_command_and_timeout_are_unknown`
      - `resources::tests::task17_pi_status_fixtures_match_live_output_shape`
      - `resources::tests::task17_pi_detect_list_auth_merge_persists_without_auth_payload`
      - `resources::tests::task17_recheck_missing_pi_reports_and_persists_unknown`
      - `resources::tests::task17_missing_pi_detection_clears_stale_login`
      - `resources::tests::task17_pi_login_probe_runs_before_store_lock`

3. 接入页与左栏
   - pi 只有已登录 provider 正常显示模型；组内混合状态时保留已登录列，只给未检查的行标「未检查」。
   - 未登录与未检查 provider 分别合成「未登录 N 个 provider · 展开」和「未检查 N 个 provider · 展开」。展开后只显示 provider 名与「重新检查」，不铺开模型。
   - 接入页固定四列等高；页面限制在视口内，模型区在卡片内滚动。
   - 「全部探测」沿用全量探测命令，因此同时刷新登录状态。
   - 命令缺失的重新检查返回「未找到 pi，登录状态未检查」；已有检查时间但状态未知的 provider 行显示「登录状态未检查」。
   - 登录折叠行与「重新检查」补了 2px `--accent` 键盘焦点环。
   - 为适配 pi 只展示已登录 provider 的新前提，`resources-tab-09.test.tsx` 和 `resources-overlay-09b.test.tsx` 的既有展示夹具显式补了 `login: 'logged_in'`；没有削弱原断言。
   - 测试：`src-ui/src/resources/__tests__/resources-login-17.test.tsx` 中的 pi provider 分组、折叠渲染与 CSS 断言。

4. 确认派发
   - `logged_out` 目标进入提示分区并默认不勾。同时额度用尽时只在未登录区显示一行，并标「额度同时受限」。
   - 引擎在切换前跳过未登录备选；全部跳过时 `fallback_skipped` 记「未登录」，`runner_note` 记具体路由。确认屏与接入页的备选编辑器都标「未登录」。
   - 测试：`TASK-17 G1 登录提示的默认选择`、「同时未登录与额度用尽时只进未登录分区，且 G1 接线会默认取消勾选」、`dispatch::tests::task17_r1_logged_out_fallback_is_skipped_and_recorded`。

5. 文案与术语
   - 用户可见状态统一为「已登录 / 未登录 / 未检查」，按钮使用「重新检查」。
   - `docs/MANUAL.md` 术语表已追加 pi provider 登录状态及出现位置。

## 能红检查

在最终复跑前临时注入回归，确认新增测试会失败，随后恢复实现：

- R1 前端先行新增测试时，专项结果为 `7 failed | 6 passed`；失败点逐项对应混合 provider、未知说明、G1 去重与行为接线、备选标记、焦点环、环境变量卫生与报告同步。实现后仅报告同步测试保持红态，报告更新后通过。
- R1 Rust 专项测试覆盖真假 pi 形状、两条落盘链、缺命令、锁窗口、旧状态刷新和备选跳过；其断言分别钉住实际字段、文件内容、锁可用性、调用路由与 `fallback_skipped`/note，不是同义反复。
- 临时反向注错为「重新检查恒返回 unknown、登录检查移回锁内、pi 缺席保留 logged_in、不跳过未登录备选」，`cargo test task17 -- --nocapture` 得到 `4 failed | 7 passed`；恢复后专项 `11 passed`。

临时回归均已恢复，未留在工作树。

## 验证结果

### `cargo check --all-targets`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-ui)
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.28s
```

退出码 0，无 warning。

### `cargo test`

```text
test result: ok. 414 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 18.39s
```

退出码 0。忽略项均为仓库原有的真机或显式本地事实源测试。

### `cd src-ui && npx vitest run --configLoader native --pool threads`

```text
Test Files  33 passed (33)
Tests  509 passed (509)
Duration  1.04s
```

退出码 0。命令输出包含既有的 `NO_COLOR` / `FORCE_COLOR` 提示，不影响测试结果。

### `cd src-ui && npm run build`

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build
vite v8.0.7 building client environment for production...
337 modules transformed.
built in 933ms
```

退出码 0。

## 换行核对

- `src/server.rs`：CRLF 4667 / 裸 LF 0，保持纯 CRLF，本轮未改动。
- 改动的 Rust 文件保持纯 LF：`src/resources.rs` 裸 LF 4321，`src/dispatch.rs` 裸 LF 9494。
- `src-ui/src/tauri.ts`：CRLF 576 / 裸 LF 114，本轮未改动，字节统计与改前相同。
- 其他改动前端、文档与测试文件：保持纯 LF。

## 未做与边界

- 未运行真实 `pi`，登录检查测试全部使用假 `pi` 脚本或进程替身。
- 未启动窗口；按任务书仅完成代码、静态布局断言与构建验证。
- 未读取任何本机凭据文件，未使用 `--credentials`。
- 未运行 `cargo fmt`，未 commit，未 push，未修改 `D:\work\aicross` 主工作树。
