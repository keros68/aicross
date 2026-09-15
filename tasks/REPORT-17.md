# REPORT-17 接入页：pi provider 登录状态与等高布局

## 逐条实现

1. 资源契约与兼容
   - `ResourceModel` 只追加 `login` 与 `login_checked_at`。Rust 使用 `ResourceLogin`，序列化值为 `unknown` / `logged_in` / `logged_out`；缺字段时 serde 默认 `unknown`。
   - 前端资源模型与派发模型同步追加三态联合类型，旧数据和异常值按「未检查」展示。
   - 测试：`resources::tests::task17_old_resource_json_defaults_login_to_unknown`。

2. pi provider 登录检查
   - 探测到 pi 模型后，按 provider 去重，每个 provider 只运行一次 `pi auth check --provider <p> --json --no-refresh`。
   - 检查按每批最多 4 个并发执行，单次超时 10 秒；命令缺失或超时记为 `unknown`。
   - 只反序列化顶层 `ready: bool` 并结合退出码判断。实现未传 `--credentials`，不读取、不保存、不返回其他字段或检查输出。
   - 结果与检查时间写入 `resources.json`；`detect_all_resources` 与单个 pi 探测都会刷新登录状态。
   - 新增 `check_pi_provider_login(provider)` 命令，供单个 provider 的「重新检查」使用。
   - 测试：
     - `resources::tests::task17_pi_login_check_uses_safe_args_once_and_ignores_credentials`
     - `resources::tests::task17_pi_login_checks_cap_concurrency_at_four`
     - `resources::tests::task17_pi_login_missing_command_and_timeout_are_unknown`

3. 接入页与左栏
   - pi 只有已登录 provider 正常显示模型；组内未验证模型继续折叠。
   - 未登录与未检查 provider 分别合成「未登录 N 个 provider · 展开」和「未检查 N 个 provider · 展开」。展开后只显示 provider 名与「重新检查」，不铺开模型。
   - 接入页固定四列等高；页面限制在视口内，模型区在卡片内滚动。
   - 「全部探测」沿用全量探测命令，因此同时刷新登录状态。
   - 测试：`src-ui/src/resources/__tests__/resources-login-17.test.tsx` 中的 pi provider 分组、折叠渲染与 CSS 断言。

4. 确认派发
   - `logged_out` 目标进入与额度用尽相同样式的提示分区，初始默认不勾；用户仍可显式勾回。
   - 测试：`TASK-17 G1 登录提示的默认选择`。

5. 文案与术语
   - 用户可见状态统一为「已登录 / 未登录 / 未检查」，按钮使用「重新检查」。
   - `docs/MANUAL.md` 术语表已追加 pi provider 登录状态及出现位置。

## 能红检查

在最终复跑前临时注入回归，确认新增测试会失败，随后恢复实现：

- Rust：把并发上限改为 5、去掉 `--no-refresh`、把旧 JSON 默认改为已登录、把超时改为未登录。4 个 TASK-17 测试中对应 4 项均能失败；恢复后 `4 passed`。
- 前端：让未登录/未检查 provider 正常铺开、取消 G1 默认排除、把四列改为三列。专项文件得到 `5 failed | 1 passed`；另将登录目标识别临时置空后，G1 目标识别测试单独失败。恢复后 `6 passed`。

临时回归均已恢复，未留在工作树。

## 验证结果

### `cargo check --all-targets`

```text
Compiling aicross v0.1.0 (D:\work\aicross-ui)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.75s
```

退出码 0，无 warning。

### `cargo test`

```text
test result: ok. 407 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 21.58s
```

退出码 0。忽略项均为仓库原有的真机或显式本地事实源测试。

### `cd src-ui && npx vitest run --configLoader native --pool threads`

```text
Test Files  33 passed (33)
Tests  502 passed (502)
Duration  1.10s
```

退出码 0。命令输出包含既有的 `NO_COLOR` / `FORCE_COLOR` 提示，不影响测试结果。

### `cd src-ui && npm run build`

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build
vite v8.0.7 building client environment for production...
337 modules transformed.
built in 793ms
```

退出码 0。

## 换行核对

- `src/server.rs`：CRLF 4667 / LF 4667，保持纯 CRLF。
- 其他改动 Rust 文件：保持纯 LF。
- `src-ui/src/tauri.ts`：HEAD 为 CRLF 576 / LF 688，当前为 CRLF 576 / LF 690；只新增两行 LF，未改变既有行终止符。
- 其他改动前端、文档与测试文件：保持纯 LF。

## 未做与边界

- 未运行真实 `pi`，登录检查测试全部使用假 `pi` 脚本或进程替身。
- 未启动窗口；按任务书仅完成代码、静态布局断言与构建验证。
- 未读取任何本机凭据文件，未使用 `--credentials`。
- 未运行 `cargo fmt`，未 commit，未 push，未修改 `D:\work\aicross` 主工作树。
