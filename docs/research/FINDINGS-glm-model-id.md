# GLM Anthropic 兼容端点：模型 ID 静默降级问题

**日期**：2026-07-09
**环境**：Windows 11，claude CLI 2.1.205，端点 `https://open.bigmodel.cn/api/anthropic`
**结论摘要**：该端点对**不认识但格式合法**的模型名不报错，而是静默用 `glm-4.7` 应答。据此发现 cc-switch 中 opus 档 ID 无效、sonnet 档 ID 被降级。

---

## 一、怎么发现的

起因是在给 ai-cross 的派发通道加只读护栏（`claude -p --tools`）时，顺手验证「模型 ID 能不能不写死」。

`claude --help` 说 `--model` 接受**别名**："Provide an alias for the latest model (e.g. 'fable', 'opus', or 'sonnet')"。别名跟随最新模型、天然免维护，看起来是理想方案。于是测试别名在第三方覆写端点（GLM）下是否可用。

测试命令返回了正常的 `OK`。**但这个 OK 是假的信号**——它只证明"有模型回答了"，不证明"回答的是哪个模型"。

## 二、第一层证据：CLI 的 modelUsage（不可靠，但有提示性）

用 `claude -p --output-format json` 看 `modelUsage` 字段：

| 请求 `--model` | `modelUsage` 报告 |
|---|---|
| `haiku`（别名） | `claude-haiku-4-5-20251001` |
| `glm-5-turbo` | `glm-5-turbo` |

说明：**CLI 在本地把别名展开成 Anthropic 官方模型 ID**，然后原样发给 GLM 端点。

⚠️ 但 `modelUsage` 记的是**请求值**，不是服务端返回值。**它不能用来判断服务端真正用了哪个模型。**（这一点我一开始判断错了，差点据此下结论。）

## 三、第二层证据：直接读 API 响应体的 `model` 字段（ground truth）

绕开 CLI，直接 POST `{base}/v1/messages`，读响应体里服务端自报的 `model`：

| 请求 model | **服务端响应 model** | 行为 |
|---|---|---|
| `glm-5-turbo` | `glm-5-turbo` | ✅ 正常 |
| `glm-5.2` | `glm-5.2` | ✅ 正常 |
| `GLM-4.7` | `glm-4.7` | ✅ 正常（该 ID 仍在服务） |
| `glm-5.1` | **`glm-5.2`** | ⚠️ 静默降级/重定向 |
| `glm-5.2[1M]` | — | ❌ HTTP 400 `[1211] 模型不存在` |
| `claude-haiku-4-5-20251001` | **`glm-4.7`** | ⚠️ 静默 fallback |
| `haiku` | **`glm-4.7`** | ⚠️ 静默 fallback |
| `totally-fake-model-xyz` | — | ❌ HTTP 400 `[1211] 模型不存在` |

**核心行为**：
1. 请求一个**格式像模型名、但该端点不提供**的 ID（例如 Anthropic 官方 ID），端点**不报错**，用 `glm-4.7` 回答。
2. 只有**完全无法解析**的名字才吃 400。
3. 因此「所有请求都真实打到了智谱，从未打到 Anthropic」——**不存在 claude 模型冒充 GLM 的情况**，反过来是 GLM 用 `glm-4.7` 冒充了一切它不认识的名字。

## 四、由此暴露的两个现存 bug

### bug 1：opus 档 ID 无效，且被误诊为「端点过载」

cc-switch 中 Zhipu GLM 的档位映射为：

```
haiku  = glm-5-turbo     ✅ 可用
sonnet = glm-5.1         ⚠️ 实际响应 glm-5.2
opus   = glm-5.2[1M]     ❌ HTTP 400 模型不存在
```

经 `cc_switch.py exec --tier opus` 派发时，claude CLI 收到 400 后**反复重试直到 180 秒超时**，脚本随后报：

```
[超时] provider=Zhipu GLM model=glm-5.2[1M] 超过 180s 未返回。
提示: 该端点可能过载(如 529)。按稳健性规则，连续失败 2 次应熔断切同档备选通道。
```

**诊断完全错误**：实际原因是模型名不存在（400），不是过载（529）。这会误导熔断策略。

### bug 2：`cc_switch.py` 的 `is_error` 检查只覆盖了一半

`references/cc_switch.py` 中，对「API 报错但 exit 0、错误文本塞进 result 字段」的检查**只在 `--usage` 分支内**（需要 `--output-format json` 才能解析）。普通派发（不带 `--usage`）走的是 `sys.stdout.write(res.stdout)`，会把 `API Error: 400 [1211] 模型不存在...` 当成模型的正常答案输出给上游。

`references/channels.md` 里已记录过这个坑（"API 错误可能伪装成正常回答"），但代码只堵了一半。

## 五、可复现脚本

token 只在进程内读取并注入请求头，不打印、不落盘。

```python
"""直接打 GLM 的 Anthropic 兼容端点，读响应体里服务端返回的 model 字段。"""
import sqlite3, os, json, sys, urllib.request

DB = os.path.expanduser("~/.cc-switch/cc-switch.db")
con = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
con.row_factory = sqlite3.Row
row = next(con.execute("SELECT settings_config FROM providers WHERE name=?", ("Zhipu GLM",)))
con.close()
env_cfg = json.loads(row["settings_config"]).get("env", {})
base = env_cfg["ANTHROPIC_BASE_URL"].rstrip("/")
token = env_cfg.get("ANTHROPIC_AUTH_TOKEN") or env_cfg.get("ANTHROPIC_API_KEY")

for req_model in sys.argv[1:]:
    body = json.dumps({
        "model": req_model, "max_tokens": 16,
        "messages": [{"role": "user", "content": "Reply with just: OK"}],
    }).encode()
    req = urllib.request.Request(f"{base}/v1/messages", data=body, method="POST", headers={
        "content-type": "application/json", "anthropic-version": "2023-06-01",
        "x-api-key": token, "authorization": f"Bearer {token}",
    })
    try:
        with urllib.request.urlopen(req, timeout=90) as r:
            d = json.loads(r.read())
        text = "".join(b.get("text", "") for b in d.get("content", []))
        print(f"请求 model={req_model!r:34} → 响应 model={d.get('model')!r:28} text={text.strip()[:20]!r}")
    except urllib.error.HTTPError as e:
        print(f"请求 model={req_model!r:34} → HTTP {e.code}: {e.read()[:120].decode('utf-8','replace')}")
```

用法：`python probe_raw.py "glm-5.2[1M]" "glm-5.2" "glm-5.1"`

## 六、需要那边 agent 查证的问题

1. **智谱当前（2026-07）对外提供的 coding plan 模型 ID 完整清单是什么？** 以官方文档为准，注明出处与日期。特别是：
   - `glm-5-turbo` / `glm-5.1` / `glm-5.2` / `GLM-4.7` 各自是否仍在服务、是否有下线计划。
   - `glm-5.1` 请求为何返回 `glm-5.2`？是别名、是合并，还是降级？

2. **1M 长上下文版本的正确模型 ID 是什么？** cc-switch 里配的 `glm-5.2[1M]` 在 Anthropic 兼容端点上返回 400。需要确认：
   - 长上下文是靠独立模型 ID，还是靠请求参数？
   - `[1M]` 这种方括号后缀是某个客户端（如 cc-switch / zcode）的私有约定，还是官方 ID 的一部分？
   - 如果是私有约定，Anthropic 兼容端点上应该传什么？

3. **该端点的模型名解析规则**：官方是否文档化了「未知模型名 fallback 到 glm-4.7」这个行为？还是未定义行为？（影响我们是否能依赖 400 来做接入校验。）

4. **Anthropic 兼容端点是否支持 `GET /v1/models` 之类的枚举接口？** 若支持，我们可以运行时发现模型 ID，彻底不写死。

5. 同样的问题对 **Kimi coding plan**（`https://api.kimi.com/coding/`，当前文档写死 `kimi-for-coding`）和 **StepFun** 也需要确认一遍：是否存在同样的静默 fallback？

## 七、结论与后续动作（本地侧）

**「模型 ID 不能写死」的真正代价不是"过期要维护"，而是"过期了你不知道"。** 静默降级不会报错，只会让你以为在用 opus 档，实际拿到的是最便宜的模型的回答。

因此修复方向不只是把 ID 挪个地方，而是要有来源 + 有校验：

- **来源**：真相在 provider 自己的配置里。已实测可行的机制——把 cc-switch 的 `ANTHROPIC_DEFAULT_{HAIKU,SONNET,OPUS}_MODEL` 一并注入子进程，命令只传别名 `--model haiku|sonnet|opus`，CLI 会按注入值展开（实测请求 `haiku` 正确落到 `glm-5-turbo`）。这样命令模板里不出现任何具体 ID。
- **校验**：CLI 不暴露服务端返回的 model，校验只能在**接入时**做一次——直接打端点，比对响应体 `model` 与请求是否一致，不一致即为静默降级。结果记入 `manifest.md`。
  - 这是对项目既有原则「未冒烟就别假设可用」的升级：冒烟判据要从「**有没有回答**」改成「**回答的是不是它**」。
- **待修**：`cc_switch.py` 非 `--usage` 模式补 `is_error` 检查；opus/sonnet 档位 ID 待第 6 节查证后更正；`channels.md` 中硬编码的 `GLM-4.7` / `glm-5.2[1m]` / `kimi-for-coding` 降级为「示例，以自身配置为准」。
