# 控制台通道

本文供 ai-cross skill 的 `channels.md` 维护者使用。控制台通道把总指挥会话中的派发交给已运行的 aicross；控制台不可用时，skill 可继续使用原有 CLI 通道。

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

### 查询、采纳与取消

- `GET /v1/runs/<run_id>` 查询状态。
- `POST /v1/runs/<run_id>/adopt`，正文为 `{ "node_id": "…" }`。
- `POST /v1/runs/<run_id>/cancel`，正文可为 `{}`。

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

run 的 `status` 为 `running`、`done`、`failed` 或 `rejected`。节点状态保留图上的原状态词，包括 `draft`、`pending`、`done`、`failed` 与 `quota_exhausted`；额度用尽不改写为 `failed`，有恢复时间时写在 `quota_resets_at`。`leftover` 从遗留节点汇总，仅在有遗留项时出现；`reason` 说明 rejected 或失败原因；`in_flight_node_ids` 是当时真正执行中的节点。

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

多行派发文本必须写入文件，不放进 argv：

```powershell
aicross dispatch --project D:\work\project --preset economy --text-file D:\tmp\dispatch.txt
aicross status <run_id>
aicross status <run_id> --wait --timeout 600
aicross adopt <run_id> <node_id>
aicross cancel <run_id>
```

`--timeout` 单位为秒，默认 600。`--text-file` 接受 UTF-8，可带 UTF-8 BOM；其它编码返回参数错误。`--text` 与 `--text=...` 均不接受。

预设别名如下：

| CLI 值 | HTTP 值 |
| --- | --- |
| `shared` | `builtin-shared` |
| `economy` | `builtin-economy` |
| `blind` | `builtin-blind` |

开发态使用相同参数，但读取 `console.dev.json`：

```powershell
cargo run -- dispatch --project D:\work\project --preset economy --text-file D:\tmp\dispatch.txt
```

命令把 JSON 写到 stdout，把一行说明写到 stderr。每轮状态查询都会重读清单。HTTP 客户端单次请求总超时为 30 秒；`status --wait` 到时以退出码 1 结束。HTTP 200 但正文不是 JSON 时，以退出码 1 结束，并输出原文首行。

| 退出码 | 含义 |
| --- | --- |
| 0 | 请求成功 |
| 1 | HTTP 业务失败、等待超时或响应不是 JSON |
| 2 | 命令参数、文本文件或编码错误 |
| 3 | 控制台未运行、清单无效、PID 已结束或端点不可达 |

skill 仅在退出码 3 时回退到原有通道。Windows 的 NSIS/MSI 安装包会把安装目录作为独立项加入当前用户 PATH，并在卸载时只删除该项；macOS 与 Linux 安装包不修改 PATH。

如需直接调 HTTP，可从清单读取端点和令牌：

```powershell
$console = Get-Content -Raw "$HOME\.aicross\console.json" | ConvertFrom-Json
$headers = @{ Authorization = "Bearer $($console.token)" }
Invoke-RestMethod -Method Get -Uri "$($console.endpoint)/v1/runs/<run_id>" -Headers $headers
```
