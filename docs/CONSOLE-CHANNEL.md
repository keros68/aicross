# 控制台通道

本文供 ai-cross skill 的 `channels.md` 维护者使用。控制台通道把总指挥会话中的派发交给已运行的 aicross；控制台不可用时，skill 可继续使用原有 CLI 通道。

## 发现控制台

aicross 启动后写入当前用户目录下的 `~/.aicross/console.json`：

```json
{
  "version": 1,
  "endpoint": "http://127.0.0.1:43123",
  "token": "64 位十六进制随机值",
  "pid": 12345,
  "started_at": "2026-09-15T12:00:00Z"
}
```

端点只监听 `127.0.0.1`。清单仅允许当前用户读取，应用退出时删除。客户端必须拒绝清单中的非 `http://127.0.0.1:<port>` 地址，所有请求都带 `Authorization: Bearer <token>`；不要输出或记录令牌。

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

成功返回：

```json
{ "run_id": "…", "graph_id": "…", "node_ids": ["…"] }
```

项目目录不存在返回 404。预设不存在、角色没有可派发目标或现有派发规则拒绝请求时返回 409，并在 `error` 字段说明原因。令牌缺失或错误返回 401。

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
      "status": "running",
      "text_excerpt": "",
      "artifacts_dir": ""
    }
  ],
  "started_at": "2026-09-15T12:00:00Z"
}
```

`status` 为 `running`、`done` 或 `failed`。`leftover` 仅在有遗留项时出现。

## 命令行

多行派发文本必须写入文件，不放进 argv：

```powershell
aicross dispatch --project D:\work\project --preset economy --text-file D:\tmp\dispatch.txt
aicross status <run_id>
aicross status <run_id> --wait
aicross adopt <run_id> <node_id>
aicross cancel <run_id>
```

开发态使用相同参数：

```powershell
cargo run -- dispatch --project D:\work\project --preset economy --text-file D:\tmp\dispatch.txt
```

命令把 JSON 写到 stdout，把一行人读说明写到 stderr。找不到 `console.json`、清单无效或端点不通时输出 `控制台未运行` 并以退出码 3 结束。skill 遇到退出码 3 后可回退到原有通道；401、404、409 等业务响应不得按“控制台未运行”处理。

如需直接调 HTTP，可从清单读取端点和令牌：

```powershell
$console = Get-Content -Raw "$HOME\.aicross\console.json" | ConvertFrom-Json
$headers = @{ Authorization = "Bearer $($console.token)" }
Invoke-RestMethod -Method Get -Uri "$($console.endpoint)/v1/runs/<run_id>" -Headers $headers
```
