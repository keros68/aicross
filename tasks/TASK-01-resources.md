# TASK-01 资源：agent → 模型两级、自动探测、手动输入、冒烟、resources.json

先读 `AGENTS.md`、`tasks/TASK-00-skeleton.md`、`tasks/REPORT-00.md`（含三轮修正）。本期把左栏「资源」tab 从占位做成真的。产品说明见 `D:\work\agentteam\PRODUCT-console.md`「形态」节的资源区与探测事实源表。

## 概念

- **agent**（一级）：壳。来自 TASK-00 的工具注册表（claude / codex / kimi / pi）加 `cc-switch`（provider 桥）和用户手动添加的项。**agent 只是通道，派发目标永远是模型。**
- **模型资源**（二级）：某个 agent 下可用的一个模型。字段：
  - `id`（模型 ID，原样）、`agent`、`provider`（该 agent 内的 provider 名，无则空）
  - `vendor`（**权重厂商**：Anthropic / OpenAI / Moonshot / Zhipu / DeepSeek / MiniMax / Alibaba / StepFun / 其他，按模型 ID 前缀推断，可改）
  - `billing_source`（计费源：官方订阅 / coding plan 名 / provider 名——pi 下一个 provider 可挂多家权重，权重厂商各算独立性，计费源只算一个）
  - `tier`（低 / 中 / 高，用户定，默认空）、`thinking`（支持 / 不支持 / 未知）
  - `status`（未接入 / 待冒烟 / 已接入 / 冒烟失败）
  - `source`（来源三选一：`读:<路径或命令>` / `文档(日期)` / `申报(日期)`）
  - `smoked_at`（最近冒烟时间）、`smoke_model`（冒烟响应体里报的模型名，能拿到就记）、`note`
- 持久化：`~/.aicross/resources.json`，结构 `{ "version": 1, "agents": [ { "id", "kind", "detected", "version", "models": [ …模型资源… ] } ] }`。这是后续所有期的契约，字段名定了就别改。

## 交付项

### A. 探测（每个 agent 一个「探测」按钮 + 接入面板里的「全部探测」）
读各 agent 的**本地事实源**，运行时读，不手抄进代码：

| agent | 事实源 | 说明 |
|---|---|---|
| codex | `~/.codex/models_cache.json` | 先拿一份样本看结构（slug / 描述 / 可用 effort / priority / fetched_at），解析成模型清单；vendor=OpenAI |
| kimi | `~/.kimi-code/config.toml` 的 `[models."…"]` 段 | 别名、真实模型 ID、上下文长度、effort 支持；vendor=Moonshot |
| pi | `~/.pi/agent/models-store.json`（provider → 模型数组，含 baseUrl / contextWindow 等）+ `pi --list-models`（只报已鉴权可用的）+ `pi auth check --provider <名> --json`（就绪状态） | 以 `--list-models` 的结果为"可用"，models-store 补元数据；provider 名进 `provider` 与 `billing_source`；vendor 按模型 ID 推断（glm→Zhipu、deepseek→DeepSeek、kimi→Moonshot、MiniMax→MiniMax、qwen→Alibaba）。**不得读 `~/.pi/auth.json`**；models-store 里若有 key 类字段，只读需要的字段 |
| claude | 无本地清单 | 内置一张小预设表（模型 ID + vendor=Anthropic），`source` 标 `文档(2026-08-30)`，用户可改可加 |
| cc-switch | `python <ai-cross 目录>/references/cc_switch.py list` | ai-cross 目录在设置里配，默认按顺序探测 `~/.claude/skills/ai-cross`、`~/.codex/skills/ai-cross`、`D:\work\agentteam\dist\ai-cross`；解析它的输出（provider 名 + 档位→模型映射，它不输出 token）。找不到脚本就标"未配置 ai-cross 目录" |

探测结果与已有 resources.json **合并**：同 `(agent, provider, id)` 更新元数据、保留用户填的 tier / note / status；新条目状态"待冒烟"；事实源里消失的条目标"未在事实源中"但不删。每次探测记录 `source` 与时间。

### B. 手动输入
每个 agent 下「手动输入」：表单 = 模型 ID、provider（可空）、vendor、tier、note；`source` 自动填 `申报(日期)`，状态"待冒烟"。「手动添加」一个新 agent：名字 + 二进制名 + 启动参数（走 TASK-00 的 `~/.aicross/tools.json` 覆写）。

### C. 冒烟（单条「冒烟」按钮 + 接入面板「冒烟所选」）
判据是**回答的是不是它**，不只是有没有回答。每个通道用最便宜的方式发"只回复 OK"，60 秒超时，全部走 stdin 或参数、不走 shell 拼接：

| agent | 命令 | 成功判据 | 真身 |
|---|---|---|---|
| claude | `claude -p --model <id> --tools "" --output-format json`，prompt 走 stdin | JSON `is_error` 为 false 且 `result` 非空 | 官方端点不核对 |
| codex | `codex exec -m <id> -s read-only --skip-git-repo-check --json -`，prompt 走 stdin | JSONL 出现 `turn.completed` | 官方端点不核对 |
| kimi | `kimi -p "只回复 OK" -m <alias> --output-format text` | 有非空输出且不含 error | 无 |
| pi | `pi --provider <p> --model <id> -p --no-tools --no-extensions --no-context-files --mode json`，prompt 走 stdin | JSON 流出现 `agent_settled` 且末条 assistant 消息含 `text` 块（**不得**用 `thinking` 块当答案） | 比对事件里的 `provider`/`model` 字段，不一致标"冒烟失败：响应模型不符" |
| cc-switch | `python cc_switch.py exec --provider <p> --tier <tier> --task "只回复 OK"`，再跑 `verify_model.py --provider <p>` | 前者 exit 0 且输出无 is_error；后者通过 | verify_model 的比对结果 |

成功 → `status=已接入`、`smoked_at=now`、记 `smoke_model`；失败 → `status=冒烟失败`、`note` 存错误摘要（前 200 字，不含任何环境变量值）。所有子进程附 `AI_CROSS_PEER=1`。退出码不作唯一判据。

### D. 界面
- 左栏「资源」tab：agent 列表（名字、版本、已检测/未检测、「探测」「手动输入」），展开显示模型条目（id、provider、vendor、tier 下拉、状态标签、来源、冒烟日期、「冒烟」「移除」）。状态标签与动作成对：待冒烟 ↔ 冒烟、已接入 ↔ 移除。
- 顶栏「接入资源」面板：上半"检测到的 agent"（勾选）、下半"cc-switch 里的 provider"（勾选，key 只显示"已配置"），按钮「全部探测」「冒烟所选」「关闭」。
- 探测/冒烟过程用文字状态（"探测中…"/"冒烟中…"），不做动画。错误显示中文前缀 + 原文。

### E. 遗留
- `write_terminal` 改 async，`pty.write` 不在全局锁内做 write_all+flush（R3 备注）。
- `cancelled` 集合加过期清理（R3 备注）。

## 验收
1. `cargo check`、`cargo test`、`cargo fmt -- --check`、`cd src-ui && npm run build` 输出原文进 `tasks/REPORT-01.md`。
2. 测试：四个事实源解析各一个 fixture（从本机样本**脱敏**后放 `src-tauri/fixtures/`，不得含任何 key/token/baseUrl 里的凭据）；resources.json 合并规则（保留用户字段、新条目待冒烟、消失条目不删）；冒烟结果解析（pi 的 provider/model 不符判失败；thinking 块不算答案）。
3. 本机实测（可在无窗口环境下用测试或命令行验证，写清怎么验的）：探测 pi 能列出 `--list-models` 的 provider 与模型；对 `zai-coding-cn/glm-4.7` 冒烟成功并记下 `smoke_model`；codex/kimi 探测能读出清单。发模型请求总数 ≤ 5 次。
4. 没做到的写"未做/未验证"。

## 参考
- `D:\work\agentteam\dist\ai-cross\references\channels.md`（各通道命令模板与坑：codex 选项顺序、kimi 无 stdin、pi exit 1 假阴性与 thinking 块）、`setup.md`（盘点流程、聚合型订阅一条）、`usage_probe.py`（各 CLI 本地目录）。
- `D:\work\agentteam\console-proto/resources-research.md`（pi / coffeecli / 竞品的资源结构）。
