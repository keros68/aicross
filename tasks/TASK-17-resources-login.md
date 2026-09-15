# TASK-17 接入页：pi 只显示已登录的 provider，页面不再拉长（09-15 用户提出）

依据 `docs/PRODUCT.md`「用户 09-15 晚四点反馈的定案」第 3 条。用户看到 pi 下 qwen-token-plan-cn 的 18 个模型全部「未验证」把页面拉得很长，而本机 qwen 早已退出登录。

## 契约（只追加）

- `ResourceModel` 追加 `login: "unknown"|"logged_in"|"logged_out"`（serde default unknown）与 `login_checked_at`。
- 探测时对 pi 的每个 provider 跑一次 `pi auth check --provider <p> --json --no-refresh`（绝不带 `--credentials`；只读 exit code 与 JSON 里的就绪布尔，不读也不记任何凭据字段）：就绪 → 该 provider 下模型 `logged_in`；不就绪 → `logged_out`；命令不存在/超时 → `unknown`。探测结果随 `resources.json` 落盘（只落状态与时间）。
- 其它 agent：claude / codex / kimi 原生走各自 CLI 的登录判断已有的探测口径，不新增。

## 实现

1. `src/resources.rs`：探测流程加 provider 登录检查（每个 provider 一次，并发上限 4，单次超时 10s）；`list_dispatchable_models` 不过滤（派发仍可手动选），但把 `login` 带给前端。
2. 接入页与左栏：pi 卡先按 provider 分组，已登录的 provider 正常列（未验证仍折叠成一行）；未登录的 provider 合成一行「未登录 N 个 provider · 展开」，展开后每个 provider 一行名字 + 「重新检查」。整页改为四列等高、卡片内滚动（`max-height` 按视口，内部 `overflow:auto`），页面本身不再超过一屏。
3. G1 确认屏：目标是 `logged_out` 的模型给一句提示并默认不勾（与额度用尽同一分区样式）。
4. 「全部探测」按钮同时刷新登录状态；单个 provider 行有「重新检查」。
5. 文案遵守 `AGENTS.md`「界面文案与命名规范」：未登录 / 已登录 / 未检查；术语表加行。
6. 测试：`pi auth check` 输出解析（就绪 / 未就绪 / 命令缺失 / 超时）不读凭据字段；旧 `resources.json` 缺字段默认 unknown；分组折叠纯函数；G1 默认不勾 logged_out。

验证：`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。保持文件原有换行；不 `cargo fmt`；不起窗口、不发真机请求（探测在测试里用假 `pi` 脚本）；不 commit、不 push。报告写 `tasks/REPORT-17.md`。
