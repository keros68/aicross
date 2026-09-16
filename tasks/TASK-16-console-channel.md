# TASK-16 控制台通道：控制台会话里直接派发（09-15 用户提出）

依据 `docs/PRODUCT.md`「用户 09-15 晚四点反馈的定案」第 1 条。目标：用户在控制台（原生 CLI 会话）里干活，遇到要多家核对的地方，ai-cross skill 把派发交给正在运行的控制台，节点长在当前工作区的对话图上，结果屏、留痕、合并全部复用。控制台不在时 skill 行为不变。

## 契约（只追加）

- `~/.aicross/console.json`：`{ version: 1, endpoint: "http://127.0.0.1:<port>", token: "<随机 32 字节 hex>", pid, started_at }`。控制台启动写入（覆盖），退出删除；文件权限只当前用户可读。
- HTTP（沿用现有 hook-server，`Authorization: Bearer <token>`，只监听 127.0.0.1）：
  - `POST /v1/dispatch`：`{ project: string, text: string, preset?: "builtin-shared"|"builtin-economy"|"builtin-blind"|..., targets?: [{agent, provider, model, role, visibility, write}], attachments?: string[] }` → `{ run_id, graph_id, node_ids }`。语义：在该项目的当前对话图（没有就新建）上追加一个根消息节点，盖指定预设（默认经济模式），按 G1 规则自动派发（额度用尽、未验证等仍按现有规则处理，不弹确认屏；不可派时返回 409 与原因）。
  - `GET /v1/runs/<run_id>`：`{ status: "running"|"done"|"failed", nodes: [{node_id, role, agent, model, status, text_excerpt, artifacts_dir}], leftover?: [...] }`。
  - `POST /v1/runs/<run_id>/adopt`：`{ node_id }` → 采纳该节点（与结果卡「采纳」同一函数）。
  - `POST /v1/runs/<run_id>/cancel`。
- 命令行入口 `aicross`（Rust 二进制加子命令，安装包把它加进 PATH；开发态 `cargo run -- dispatch …`）：`aicross dispatch --project <dir> --preset economy --text-file <path>`（多行走文件，不走 argv）、`aicross status <run_id> [--wait]`、`aicross adopt <run_id> <node_id>`、`aicross cancel <run_id>`。找不到 `console.json` 或端点不通 → exit 3 与一句「控制台未运行」。
- 界面：对话图 tab 上由通道创建的图/节点标「来自控制台」；右栏「任务」板列出在飞 run（复用心跳）。

## 实现

1. Rust：`src/console_api.rs` 挂到现有 hook-server；令牌校验；所有写图操作走现有 `save_graph` / 派发入口（`dispatch_stage`），不另造路径。
2. `aicross` 子命令：`src/bin/aicross.rs` 或主二进制 `--` 子命令；输出 JSON 到 stdout，人读的一行到 stderr。
3. 前端：图上「来自控制台」标记；任务板在飞 run 行（读同一 progress store）；通道创建的图自动打开/切换到该 tab（用户在控制台 tab 时只在标题栏计数提示，不抢焦点）。
4. skill 侧（本仓不改 skill；写清接口给 skill 维护者）：`docs/CONSOLE-CHANNEL.md` 写通道协议与示例命令，供 `channels.md` 加「控制台通道」。
5. 测试：令牌错误 401；项目不存在 404；不可派 409 带原因；dispatch → status → adopt 走通（假通道）；`console.json` 写/删与权限；`aicross` 子命令对无控制台的退出码。

验证：`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。保持文件原有换行（`src/server.rs` CRLF）；不 `cargo fmt`；不起窗口、不发真机请求；不 commit、不 push。报告写 `tasks/REPORT-16.md`。
