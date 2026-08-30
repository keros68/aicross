# TASK-03 派发第一刀：扇出比对引擎、盲审真跑、G1 / G3、`.dispatch/` 契约

先读 `AGENTS.md`、`docs/PRODUCT.md`（「形态」「闸门」「与 ai-cross skill 的关系」）、`tasks/TASK-02-orchestration.md` 的数据模型、`tasks/REPORT-01.md`（冒烟命令与 `process.rs`）。还要读 skill 的 `D:\work\agentteam\dist\ai-cross\references\channels.md`（各通道命令模板与坑）和 `SKILL.md`「验证的框架隔离」「执行闭环规则」两节——本期把它们变成代码。

## 目标
中栏输入一段任务 + 附件，选「盲审」预设，@ 三家 → G1 确认 → 三路在各自空目录里真跑 → 结果屏并排 → 每路留痕到 `.dispatch/`。「共享对话」预设则单节点带上下文执行并延续会话。

## A. 通道执行器（Rust，`src-tauri/src/channels/`）
每个 agent 一个执行器，输入 `{ node, prompt, materials_dir, workdir, thinking, session? }`，输出 `{ text, raw_stdout, raw_stderr, usage?, identity?, session_id?, exit_code, timed_out }`。全部走 `process.rs`，argv 数组，附 `AI_CROSS_PEER=1`，默认超时 10 分钟可配。

| agent | 命令形态 | thinking 映射 | 续聊 | 用量 | 真身 |
|---|---|---|---|---|---|
| claude | `claude -p --model <id> --tools <白名单> --output-format json`，prompt 走 stdin | `--effort low/medium/high`（`关` 也用 low，官方 CLI 无关闭档） | 首轮取 `session_id`，续轮 `--resume <id>` | JSON `usage`（区分 `cache_read`） | 不核对（官方） |
| codex | `codex exec -m <id> -c model_reasoning_effort=<x> -s read-only --skip-git-repo-check --json -`，prompt 走 stdin；**父级选项必须在 `resume` 之前** | low/medium/high/xhigh | `thread.started.thread_id`，续轮 `codex exec <父级选项> resume <id> -` | `turn.completed.usage` | 不核对 |
| kimi | `kimi -p "<prompt>" -m <alias> --output-format stream-json`；**无 stdin**，prompt 按 UTF-8 字节 >30000 直接拒绝并提示换通道 | 不可关（always_thinking），只记录 | `-r <session_id>`（从输出末尾 `To resume this session` 行取） | 流里若有 usage 就记，否则 "—" | 无 |
| pi | `pi --provider <p> --model <id> -p --no-tools 或 --tools read,grep,find,ls --no-extensions --no-context-files --mode json --thinking <off/low/medium/high>`，prompt 走 stdin | 直接映射 | `--session-id <固定 uuid>` | JSON 事件里的 usage 字段（有则记） | 事件 `provider`/`model` 与请求比对，不符即失败 |
| cc-switch | `python cc_switch.py exec --provider <p> --tier <tier> --task-file <f> --usage` | 由 cc_switch 处理 | 不支持，标"单轮" | 它打印的 usage | `verify_model.py` 在冒烟时已做，派发时不重复 |

判定成功看输出体，不看退出码；pi 只认 `text` 块。

## B. 材料与隔离
- **材料** = 用户勾选的附件 + 勾选的对话消息（TASK-02 之后中栏已有对话流）。共享预设：全部对话作为上下文（拼成带角色标记的文本）+ 附件路径列表。盲预设：默认只勾附件与用户消息，模型回复默认不勾（用户可改）。
- **盲节点运行目录**：`~/.aicross/runs/<run_id>/<node_id>/`，空目录，拷入勾选的附件（保留文件名），prompt 用 skill 的冻结模板：「以下是一批原始材料。请独立分析并给出你的结论、依据与不确定处。不要猜测提供者想听什么。」+ 用户问题 + 材料清单。不拷项目里的 `CLAUDE.md`/`AGENTS.md`/`.dispatch/`。
- **共享节点运行目录**：项目目录（只读工具白名单仍生效）。
- 并发：同一阶段的节点并行，上限 = min(节点数, 3)；同一 agent 同时最多 1 个（避免单源限流）。

## C. `.dispatch/` 契约（与 skill 共用，字段定了不改）
每路一份 `<project>/.dispatch/<YYYYMMDD-HHMMSS>-<agent>-<model>-<role>.md`，首次创建目录时写入 `.gitignore`（内容 `*`）。frontmatter：

```yaml
---
schema: aicross-dispatch/1
run_id: <uuid>
stage: 1
node: <node_id>
agent: codex
provider: ""          # pi/cc-switch 时填
model: gpt-5.6-terra
vendor: OpenAI
billing_source: 官方订阅
role: 验证
tier: 中
thinking: 中
visibility: 盲
workdir: ~/.aicross/runs/<run_id>/<node_id>
materials: [spec.md, data.csv]
status: ok | error | timeout | identity_mismatch
started_at: 2026-08-31T09:00:00+08:00
ended_at: 2026-08-31T09:01:12+08:00
duration_ms: 72000
tokens_in: 14238
tokens_out: 812
cache_read: 12100
identity: "zai-coding-cn/glm-5.2"   # 响应体报的真身，拿不到写 ""
session_id: ""
exit_code: 0
error: ""
---
```
正文两节：`## 任务全文`（发出去的 prompt 原文）、`## 原始输出`（stdout 全文；stderr 另起 `## stderr`）。**绝不写入任何环境变量值。** 同时写 `<project>/.dispatch/STATE.md`（当前 run、各节点状态、下一步），格式见 skill `SKILL.md` 闭环规则第 5 条。

## D. 闸门
- **G1（发送前）**：弹卡。内容：每节点一行路由决策行 `[aicross] <任务摘要> → <agent/model> ｜ <档位> · thinking=<x> ｜ 理由：<角色默认/用户指定>`（档位、thinking 可改）；盲预设显示材料勾选；外发授权一行「材料将发送至 <厂商列表>」；「确认派发」「取消」。共享预设不显示材料勾选。
- **G2（执行中）**：中栏系统行，每节点一行文字状态（排队 / 运行中 / 完成 / 失败 / 超时）+ 「取消」按钮（杀进程树）。不做动画。
- **G3（结果屏）**：顶部回显决策行；每路一张结论卡（`text`），卡上「采纳」「重派」「升级」（升级 = 同节点换更高档或换厂商，回到 G1）；底部「交人裁决」（把并排结果写进对话流作为一条系统消息，等用户）。**本次用量**表：`阶段 | agent/模型 | in / out tokens | cache_read | 耗时 | 结果`。「留痕」折叠区列出本次 `.dispatch/` 文件，点开显示 frontmatter + 输出前 2000 字。
- **共识 / 分歧 / 独有发现**（可选，按钮「归类」）：用设置里指定的便宜模型（默认取 resources 里 tier=低 的第一个已接入模型）跑固定 prompt：输入 N 份结论原文，输出 JSON `{consensus:[], disagreements:[{point, positions:{route:stance}}], unique:[{route, finding}]}`；界面标"机器归类，可改"，每条可删可编辑；**独有发现**栏每条带「核验」（暂只复制到剪贴板）「转发」（选另一个已接入模型，把该条 + 原始材料发过去，走同一引擎）。归类失败不影响结果屏。

## E. 共享对话执行
- 中栏 @ 一个模型发送 → G1（简版）→ 单节点执行，prompt = 对话上下文 + 本条 → 回复进对话流，带 `agent/model` 标签。
- 每个 `agent/model` 目标保存 `session_id`，下次 @ 同目标走续聊；用户可「新开会话」。
- 用量记进对话流那条消息的角标（in/out）。

## F. 设置
- 默认超时、并发上限、归类模型、`.dispatch/` 是否写入项目目录（默认写；关掉则只写 `~/.aicross/runs/`）。

## 不做
- 流水线阶段间传递（第 4 期）。
- 长驻协议 / ACP（第 5 期）。
- 结果屏的自动"采纳"写盘（采纳只是标记）。

## 验收
1. 四项自动验证输出进 `tasks/REPORT-03.md`。
2. 测试：frontmatter 序列化/反序列化往返；盲节点运行目录只含勾选附件（用临时项目目录含 `CLAUDE.md`、`.dispatch/` 断言未拷入）；kimi >30000 字节拒绝；codex 续聊命令选项顺序（构造 argv 断言 `resume` 在父级选项之后）；pi 真身不符判失败；归类 JSON 解析容错。
3. **本机真跑一次盲审**（发模型请求 ≤ 4 次）：fixture 任务（`src-tauri/fixtures/blind-task/` 放一份 300 字的方案 + 一个 20 行 CSV），三路 `pi/zai-coding-cn/glm-4.7`、`codex/gpt-5.6-luna`、`kimi/kimi-for-coding-highspeed`；断言三份 `.dispatch/*.md` 生成且 frontmatter 完整、`STATE.md` 存在、用量表有数。沙箱开不了就在 REPORT 写明，由编排者在沙箱外补跑。
4. 共享对话续聊真跑一次（≤ 2 次请求）：@ 同一目标两轮，第二轮能引用第一轮内容。
