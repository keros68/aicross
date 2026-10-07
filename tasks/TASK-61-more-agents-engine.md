# TASK-61 更多 agent：支持清单 + Antigravity（引擎）

**只改 `src/**`、`fixtures/**`、`docs/CONSOLE-CHANNEL.md`，新建 `tasks/REPORT-61.md`。不改 `src-ui/**`。不 commit，不 push，不加依赖。** 设计依据：`docs/PRODUCT.md`「更多 agent（10-07 用户定）」。Antigravity 的实测事实以 `D:\work\skill\xiaoyu-skill\skills\ai-cross\references\channels.md` 的「Antigravity CLI」一节与 `references/dispatch.py` 的 `call_agy` 为准（只读参考，不改那边）。

## 1. 支持清单

把资源库里固定的四家 agent 改成一张表驱动的支持清单（一个 `const` 表或等价结构，放在 `resources.rs` 或新文件）。每项至少有：

- `id`、显示名、命令名（及 Windows 上的常见绝对路径兜底，如 agy 的 `%LOCALAPPDATA%\agy\bin\agy.exe`）、版本参数；
- 模型清单从哪读；
- `homepage`（官网，资源页「未安装」里用）；
- `untested`：命令模板只照官方文档写、没在真机跑通的为 true。

首批：现有 claude / codex / kimi / pi（行为不变），新增 `agy`（Antigravity，`untested: false`），以及 `cursor`（`cursor-agent`）、`codebuddy`（显示名「WorkBuddy / CodeBuddy」）、`copilot`（GitHub Copilot CLI）、`qoder`（`qodercli`）四家（`untested: true`）。

`ResourceAgent` 加两个字段：`untested: bool`、`homepage: String`（都 `#[serde(default)]`），探测时按表填。没装的 agent 也在资源库里出现（`detected: false`、无模型），界面据此折叠。

窗口与命令行的资源输出（`console_cli.rs` 的 `resources_value` / `resources_value_filtered`，界面类型 `ResourceAgentEntry`）每个 agent 同样加 `untested`（布尔）与 `homepage`（字符串），其余字段不变。界面另有 TASK-62 按这两个字段改，契约以此为准。

## 2. 厂商按模型算

新 agent 的模型厂商按模型名推（复用 `resources::infer_vendor`，缺的前缀补上：`gemini-` → Google，`claude-` → Anthropic，`gpt-`/`o` 系 → OpenAI，`gpt-oss` 也记 OpenAI）。认不出的（如 cursor 的 `auto`、qoder 的五个档位名）记「未知」。

推荐（`recommend.rs`）：厂商「未知」的模型不进验收 / 验证位，只可当执行位；`untested` agent 的模型一律不进推荐（手动选仍可，状态里能看出未实测）。补测试。

## 3. Antigravity 全链路（真机可测）

- **探测**：`agy --version` 取版本；模型清单跑 `agy models`，解析「id<TAB>显示名」行（跳过 `Fetching available models...` 这类非表格行），超时 30 秒。原生模型（订阅），`verification` 记 native；`billing_source` 记「官方订阅」。
- **派发**（一次性 CLI 通道，与 `dispatch.rs` 里现有通道并列）：
  - 只接**盲（只读、看不到项目）**的位置。共享 / 单写者位置选了 agy 时，派发前直接失败并说明「Antigravity 无头模式下工具全被拒，只能做纯文本盲审」。配置写入时（`config set` 一类）也拦住这种组合，或在 `config show` 里标出，二选一，报告里写明。
  - 命令：`agy --model <M> --input-format stream-json --output-format stream-json --print-timeout <超时>`，材料作为 stdin 的一行 NDJSON：`{"event":"user","message":{"role":"user","content":<prompt>}}`。在派发用的空目录里跑。**绝不加 `--dangerously-skip-permissions`。**
  - 解析最后一行 `{"event":"result"}`：`result.status` 不是 SUCCESS 或 `result.response` 为空 → 失败（错误写明「agy 没给出回答」及 status / error）；`usage` 记 token。
  - prompt 末尾追加一句「不要使用任何工具，直接用文字回答。」（与现有如实汇报后缀同一处拼接，别重复）。
  - 思考档：`--effort`（低 / 中 / 高 → low / medium / high）。
- **冒烟 / 验证**：沿用原生模型规则（读清单即可用）；资源页「测试」走一次最小派发，回答非空即通过。

## 4. 未实测的四家

- **探测**：命令存在则 `detected: true` 并取版本；模型清单：cursor 跑 `cursor-agent models`（解析失败就留空并写错误）；copilot、codebuddy 没有可靠的列模型命令，先给一张随版本发布的小表（写在数据文件里，注明来源与日期，如 copilot 文档示例里的模型名）；qoder 固定五档 `lite / efficient / auto / performance / ultimate`。
- **派发**：按官方文档写一次性命令模板（prompt 走 stdin 或临时文件，绝不走 argv 多行），只接盲位置。官方文档链接写在代码注释里：
  - cursor：`cursor-agent -p --output-format json --model <M>`；
  - codebuddy：`codebuddy -p --output-format json --model <M> --permission-mode plan`；
  - copilot：`copilot -p … --model <M>`，禁用写与命令类工具；
  - qoder：`qodercli -p --model <档位>`。
  拿不准 stdin 用法的，用临时文件 + 文档里给的读文件方式；仍拿不准的就让派发直接失败并提示「未实测通道」，不要猜。
- 这四家在本机都没装，**不做真机验证**；单元测试覆盖解析与命令拼装（用假输出）。

## 5. 不变

- 主 agent（`hosts.rs` 的 HOST_IDS）仍是四家，本期不扩。
- 现有四家的探测结果、排序、推荐不变（golden 与现有测试不改断言，除非是新增字段的序列化）。

## 验证

```
CARGO_TARGET_DIR=D:/work/aicross/target cargo check --all-targets   # 零 warning
CARGO_TARGET_DIR=D:/work/aicross/target cargo test
```

另做一次真机：在临时 `AICROSS_HOME`（`D:\aicross-data\scratch\t61-<随意>`，用完删）下 `aicross resources detect`，确认 agy 出现、模型非空、厂商按模型分开；再用 `gemini-3.8-flash-low` 走一次最小盲派发（问一道算术题），贴回答与 token。只用 low 档，只跑一两次。

换行：`src/server.rs`、`src/main.rs` 是 CRLF，其它 Rust 文件 LF，用编辑工具局部改。测试一律用临时目录。`REPORT-61.md` 写：改了什么、新增测试名、验证结果、真机结果、没做的与理由。
