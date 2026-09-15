# REPORT-16 控制台通道

## 结果

已完成 `TASK-16-console-channel.md`。总指挥会话可通过本机 `aicross` 子命令，把请求交给正在运行的 aicross；请求进入当前项目的对话图，调用既有派发引擎，状态、结果、采纳和取消通过同一个本机通道返回。控制台不在时，命令以退出码 3 和单行 `控制台未运行` 结束。

## 逐条实现

### 1. 追加契约与本机 HTTP 通道

- 新增 `src/console_api.rs`，实现 `POST /v1/dispatch`、`GET /v1/runs/<run_id>`、`POST /v1/runs/<run_id>/adopt`、`POST /v1/runs/<run_id>/cancel`。
- 复用 `src/hook_server.rs` 的监听器；监听地址仍固定为 `127.0.0.1:0`，旧 hook JSON 行协议保持不变，HTTP 依首行分流。
- 所有路由先校验 `Authorization: Bearer <token>`；请求正文上限为 4 MiB。
- `Graph` 和 `GraphNode` 只追加可选的 `project`、`source` 字段；旧图缺字段时按空值读取。
- 当前项目已有带 `project` 的图时继续该图，否则新建；图与通道节点标记 `source=console`。
- 预设缺失、目标缺失、资源不可读、未接入、验证失败、有效期内额度用尽、只读规则不符或超过请求上限时，在派发前返回 409 和原因；未验证目标仍交给既有首次派发验证逻辑。
- 图写入继续调用既有 `save_graph` 持久化实现；派发只抽出并调用既有 `dispatch_stage`；`src/dispatch.rs` 仅增加这一窄入口和结果构造所需的 crate 内字段可见性，没有改红绿核对逻辑。
- 新增共享 `adopt_graph_node` 写入口，结果卡和 HTTP adopt 都调用它，最终仍由既有图持久化实现保存。

### 2. `console.json` 生命周期与权限

- 控制台启动时在 `~/.aicross/console.json` 写入版本、仅 IPv4 loopback 的端点、64 位 hex 令牌、PID 与启动时间；临时文件写完并同步后再发布。
- Unix 文件权限设为 `0600`。Windows 以当前用户 SID 执行 `icacls /grant:r *SID:F /inheritance:r`，权限收紧失败时控制台启动失败，不带着宽权限清单继续运行。
- Tauri 收到 `ExitRequested` 时删除清单；不存在时按成功处理。

### 3. `aicross` 子命令与安装 PATH

- 新增 `src/console_cli.rs`；主二进制在启动 GUI 前识别 `dispatch`、`status`、`adopt`、`cancel`。
- `dispatch` 只接受 `--text-file`；显式传 `--text` 会作为参数错误拒绝，多行正文只从文件读取并进入 JSON 请求体。
- 清单缺失、清单无效或端点不通时：stdout 为空，stderr 仅一行 `控制台未运行`，退出码为 3；HTTP 业务错误不混同为退出码 3。
- stdout 输出 JSON，成功时 stderr 输出一行说明；`status --wait` 轮询至非 running 状态。
- NSIS 安装模板幂等地把安装目录追加到当前用户 PATH，卸载按完整 PATH 项删除并广播环境变更；WiX 通过可卸载的用户级 `Environment` 组件维护同一 PATH 项。

### 4. 前端

- 通道创建的图和节点显示“来自总指挥”。
- 收到通道事件后，普通 tab 前台会打开或切换到对应图；总指挥 tab 前台时只在后台开图，不抢焦点。
- 总指挥标题显示 `N 路在飞`；右栏任务板列出通道 run，并复用既有 `dispatch-progress` 心跳与状态解释函数。
- 普通结果卡的“采纳 / 取消采纳”改为调用共享 Rust 采纳入口；擂台“选定”的既有多候选语义未改变。

### 5. 维护者文档

- 新增 `docs/CONSOLE-CHANNEL.md`，说明清单发现、安全边界、四个 HTTP 接口、状态码、CLI 用法、PowerShell 示例及退出码 3 的回退约定。
- 未修改本仓外的 skill；文档供 skill 维护者后续接入。

## 行为测试

Rust：

- `console_api::tests::wrong_token_is_401_before_routing`
- `console_api::tests::missing_project_is_404`
- `console_api::tests::unavailable_route_is_409_with_reason`
- `console_api::tests::prepare_dispatch_marks_new_graph_and_nodes_as_console_source`
- `console_api::tests::fake_channel_dispatch_status_adopt_round_trip`
- `console_api::tests::manifest_write_delete_and_private_permissions`
- `console_cli::tests::no_console_manifest_exits_three_with_one_line`
- `console_cli::tests::dispatch_reads_multiline_text_from_file_only`
- `console_cli::tests::dispatch_rejects_text_on_argv`
- `console_cli::tests::manifest_endpoint_must_be_ipv4_loopback`
- `console_cli::tests::status_wait_adopt_and_cancel_build_protocol_paths`
- `orchestration::tests::task16_adopt_graph_node_uses_shared_persistence_entry`

前端：`src-ui/src/__tests__/console-channel.test.ts` 覆盖总指挥不抢焦点、普通 tab 自动打开或切换、在飞计数、图/任务板接线，以及 NSIS/WiX PATH 安装与卸载配置。

## 换行与范围核对

- 改动前先按字节统计 CRLF/LF；`src/server.rs` 改前为纯 CRLF，改后仍为 `CRLF=4669, LF=0`。
- `src/orchestration.rs`、`src/dispatch.rs`、`src/hook_server.rs` 和新增 Rust 文件保持 LF。
- `src/main.rs` 改前实际为纯 CRLF，按实际格式保留；`src-ui/src/tauri.ts`、`CenterPanel.tsx`、`app-state.tsx` 原本为混合换行，只对变更块做字节级增改。
- `git -c core.whitespace=cr-at-eol diff --check`：无输出。
- `src/terminal.rs`：无改动。
- 未运行 `cargo fmt`；未 commit，未 push。

## 没做或部分验证

- 按任务限制，没有启动窗口，也没有向真实模型或真机通道发请求；HTTP 端到端由假通道覆盖。
- Windows 测试环境不允许修改临时文件 DACL，因此测试实际覆盖清单写入/删除和精确 ACL 参数；Unix 下测试实际断言 `0600`。生产 Windows 路径会真实执行 ACL 收紧，并在失败时拒绝启动通道。
- 未生成 MSI/NSIS 安装包；PATH 配置由前端源文件回归测试和 Tauri 配置编译检查覆盖。
- 未修改仓外 skill，符合任务书要求。

## 四项验证

### `cargo check --all-targets`

```text
   Compiling aicross v0.1.0 (D:\work\aicross-p6)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.66s
```

退出码 0，无 warning。

### `cargo test`

```text
running 405 tests
test result: ok. 394 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 16.52s
```

退出码 0。11 项均为仓库既有、明确标记的真机或真实模型测试。

### `cd src-ui && npx vitest run --configLoader native --pool threads`

```text
 Test Files  31 passed (31)
      Tests  473 passed (473)
   Duration  897ms (transform 72%, import 14%, tests 12%, worker 2%)
```

退出码 0。命令同时输出既有环境变量提示：`The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.`

### `cd src-ui && npm run build`

```text
> aicross-ui@0.0.0 build
> tsc -b && vite build

vite v8.0.7 building client environment for production...
✓ 335 modules transformed.
✓ built in 703ms
```

退出码 0。命令同时输出与 Vitest 相同的 `NO_COLOR` / `FORCE_COLOR` 提示。
