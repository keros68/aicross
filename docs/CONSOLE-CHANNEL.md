# 控制台通道

本文供 ai-cross skill 的 `channels.md` 维护者使用。控制台通道把控制台会话中的派发交给已运行的 aicross；控制台不可用时，skill 可继续使用原有 CLI 通道。

## 发现控制台

aicross 发布版启动后写入当前用户目录下的 `~/.aicross/console.json`；开发构建使用 `~/.aicross/console.dev.json`，两者互不覆盖：

```json
{
  "version": 1,
  "endpoint": "http://127.0.0.1:43123",
  "token": "64 位十六进制随机值",
  "pid": 12345,
  "started_at": "2026-09-15T12:00:00Z"
}
```

端点只监听 `127.0.0.1`。清单先在临时文件上收紧为仅当前用户可读，再原子替换；应用退出时删除。客户端会核对 PID，进程不存在时删除过期清单。所有请求都带 `Authorization: Bearer <token>`；不要输出或记录令牌。带 `Origin` 或 `Sec-Fetch-Site: cross-site` 的请求会被拒绝。

## HTTP 协议

请求与响应均为 UTF-8 JSON。

### 派发

`POST /v1/dispatch`

请求体中的 `reason` 仅供 `/v1/propose` 使用；`/v1/dispatch` 收到该字段时忽略。

```json
{
  "project": "D:\\work\\project",
  "text": "核对这次修改。",
  "preset": "builtin-economy",
  "attachments": ["D:\\work\\project\\tasks\\TASK.md"]
}
```

`preset` 缺省为 `builtin-economy`。也可传 `targets` 直接形成一个并行阶段：

```json
{
  "project": "D:\\work\\project",
  "text": "分别审查安全性与兼容性。",
  "targets": [
    { "agent": "pi", "provider": "deepseek", "model": "deepseek-v4", "role": "验证", "visibility": "盲", "write": "只读", "thinking": "中" },
    { "agent": "claude", "model": "sonnet", "role": "验证", "visibility": "盲", "write": "只读", "thinking": "中" }
  ]
}
```

Rust 只校验并中继请求；前端用与确认派发相同的 `stamp()`、G1 判定和流水线执行。成功接收返回：

```json
{ "run_id": "…", "accepted": true, "graph_id": "", "node_ids": [] }
```

为保持既有契约，首个响应仍保留空的 `graph_id` 与 `node_ids`；真实图 id 与节点摘要在前端开始处理后通过状态接口提供。界面未运行或 5 秒内没有回执时返回 503。run 同时在飞默认最多 3 个，可在设置的“通道在飞上限”调整；超过上限返回 409。

### 建议派发

`POST /v1/propose`

请求体与 `/v1/dispatch` 相同，可额外带一句 `reason`：

```json
{
  "project": "D:\\work\\project",
  "text": "分别核对实现与测试。",
  "preset": "builtin-blind",
  "reason": "需要独立核对"
}
```

该接口只登记建议，不执行派发。成功返回：

```json
{ "proposal_id": "…", "accepted": true }
```

建议在界面顶部显示；用户采纳后才走与 `/v1/dispatch` 相同的前端流水线，忽略后不执行。命中单步小任务时不登记，返回：

```json
{ "proposal_id": "", "skipped": "单步小任务" }
```

`proposed` 不计入通道在飞上限。设置中显式开启“用归类模型二次判断”后，生成建议时会额外调用一次所选归类模型；默认关闭。

### 查询、采纳与取消

- `GET /v1/runs/<run_id>` 查询状态。
- `POST /v1/runs/<run_id>/adopt`，正文为 `{ "node_id": "…" }`。
- `POST /v1/runs/<run_id>/cancel`，正文可为 `{}`；对 `proposed` 状态执行后转为 `dismissed`。

查询响应：

```json
{
  "run_id": "…",
  "graph_id": "…",
  "project": "D:\\work\\project",
  "status": "running",
  "nodes": [
    {
      "node_id": "…",
      "role": "验证",
      "agent": "pi",
      "model": "deepseek-v4",
      "status": "pending",
      "text_excerpt": "",
      "artifacts_dir": ""
    }
  ],
  "started_at": "2026-09-15T12:00:00Z"
}
```

run 的 `status` 为 `proposed`、`running`、`done`、`failed`、`rejected` 或 `dismissed`。建议采纳后从 `proposed` 转为 `running`，忽略后转为 `dismissed`。节点状态保留图上的原状态词，包括 `draft`、`pending`、`done`、`failed` 与 `quota_exhausted`；额度用尽不改写为 `failed`，有恢复时间时写在 `quota_resets_at`。`leftover` 从遗留节点汇总，仅在有遗留项时出现；`reason` 说明建议理由、rejected 或失败原因；`in_flight_node_ids` 是当时真正执行中的节点。

### 状态码

| 状态码 | 含义 |
| --- | --- |
| 400 | JSON 无效、正文不完整 |
| 401 | 令牌缺失或错误 |
| 403 | 请求带 Origin 或来自 cross-site |
| 404 | 项目、run、节点或接口不存在 |
| 409 | G1 拒绝、在飞超限或操作冲突 |
| 413 | 正文超过 4 MiB |
| 503 | 控制台界面未就绪或未在 5 秒内回执 |

## 命令行

`dispatch`、`status`、`adopt` 与 `cancel` 直接使用无窗口引擎，不需要启动 aicross 窗口。`propose` 仍通过本机 HTTP 服务把建议交给窗口。

多行派发文本必须写入文件，不放进 argv：

```powershell
aicross dispatch --project D:\work\project --preset economy --text-file D:\tmp\dispatch.txt
aicross propose --project D:\work\project --preset blind --reason "需要独立核对" --text-file D:\tmp\dispatch.txt
aicross status <run_id>
aicross status <run_id> --wait --timeout 600
aicross adopt <run_id> <node_id>
aicross cancel <run_id>
```

`--timeout` 单位为秒，默认 600。`--text-file` 接受 UTF-8，可带 UTF-8 BOM；其它编码返回参数错误。`dispatch` 与 `propose` 均不接受 `--text` 或 `--text=...`；`propose` 的 `--reason` 只能是一句话。

预设别名如下：

| CLI 值 | HTTP 值 |
| --- | --- |
| `shared` | `builtin-shared` |
| `economy` | `builtin-economy` |
| `blind` | `builtin-blind` |

开发态使用相同参数。只有 `propose` 读取 `console.dev.json`：

```powershell
cargo run -- dispatch --project D:\work\project --preset economy --text-file D:\tmp\dispatch.txt
cargo run -- propose --project D:\work\project --preset blind --reason "需要独立核对" --text-file D:\tmp\dispatch.txt
```

ai-cross skill 收到已确认的派发请求后，把当前请求写入 UTF-8 临时文件并调用 `aicross dispatch`；`/派发 共享`、`/派发 经济`、`/派发 盲审` 分别传 `--preset shared|economy|blind`。首个响应带真实的 `graph_id` 与本次新建的 `node_ids`：

```json
{ "run_id": "…", "accepted": true, "graph_id": "…", "node_ids": ["…"] }
```

run 摘要存放在 `~/.aicross/runs/<run_id>.json`，除节点摘要、在飞节点、遗留项和结束状态外，还记录本次派发的原始需求文本（`original_request`）、后台进程 pid 与进程创建时间（`pid_started_at`）。后台引擎逐阶段更新；取消标记存放在同目录的 `<run_id>.cancel`，处理完成后删除。`cancel` 只写取消标记，不改写 run 文件；run 已不在 `running` 时取消静默成功（退出 0，打印当前 run）。

存活判定：run 为 `running` 且 pid 非 0 时，Windows 核对进程存在且创建时间与 `pid_started_at` 一致（防止 pid 复用误判）；macOS 与 Linux 只核对进程存在，不做创建时间比对。派发刚接收、后台进程尚未报到时 pid 为 0：`updated_at` 在 30 秒内视为启动中，不判死、计入在飞；超过 30 秒仍为 0 则判 `failed`，原因是「引擎进程未能启动」。进程中途被杀死时 run 判 `failed`、原因是「引擎进程已退出」；此时图上可能留下 `pending` 状态的节点，打开窗口后可核对处理。

项目路径按命令行给的原文使用（不做规范化改写），`graph.project`、run 文件与派发请求里的路径一致；项目目录不存在时派发前拒绝。

命令行派发设置存放在 `~/.aicross/dispatch-settings.json`，字段与窗口设置相同。只认 JSON 数字：写成字符串的数值不生效，按默认值处理；超范围的数字夹取到允许区间。文件不存在或内容无效时使用默认值。窗口设置页仍使用浏览器本地设置，两处暂不同步。

窗口派发与命令行派发共用 `~/.aicross/graphs/`。同一项目同时从窗口和命令行写图时没有并发保护。

命令把 JSON 写到 stdout，把一行说明写到 stderr。每轮状态查询都会重读 run 文件；发现 `running` 对应的后台进程已退出时，将 run 改为 `failed` 并返回 `引擎进程已退出`。`status --wait` 到时以退出码 1 结束。`propose` 的 HTTP 客户端单次请求总超时为 30 秒；HTTP 200 但正文不是 JSON 时，以退出码 1 结束，并输出原文首行。

| 退出码 | 含义 |
| --- | --- |
| 0 | 请求成功 |
| 1 | 业务失败、等待超时或 `propose` 响应不是 JSON |
| 2 | 命令参数、文本文件或编码错误 |
| 3 | 无法确定引擎目录、run 目录不可写、后台进程无法启动，或 `propose` 找不到运行中的窗口（清单缺失或已过期） |

skill 仅在退出码 3 时回退到原有通道。Windows 的 NSIS/MSI 安装包会把安装目录作为独立项加入当前用户 PATH，并在卸载时只删除该项；macOS 与 Linux 安装包不修改 PATH。

如需直接调 HTTP，可从清单读取端点和令牌：

```powershell
$console = Get-Content -Raw "$HOME\.aicross\console.json" | ConvertFrom-Json
$headers = @{ Authorization = "Bearer $($console.token)" }
Invoke-RestMethod -Method Get -Uri "$($console.endpoint)/v1/runs/<run_id>" -Headers $headers
```
