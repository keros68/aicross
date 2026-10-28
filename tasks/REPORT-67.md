# REPORT-67 按名字找模型、单次换人

## 改了什么

- `src/model_find.rs`：按口头名字找模型。名字拆成字母段与数字段，与模型编号、`resolved_id`、提供方、厂商、agent 比对。规则如下：
  - 厂商别名是一张手写小表，口语虚词（code、cli、agent…）忽略。
  - 版本号从头对上才算。
  - 能派发（可用 / 已验证 / 未验证 / 冒烟超时）的在前，与说法多出的段少的在前，最多 20 个。
  - 没对上时 `related` 列出不看版本号对上的模型。
- `aicross resources find <名字>`：退出码 0 有对上，1 没有，2 名字不可比对，3 资源库读不了。
- `aicross dispatch --seat <阶段>.<位置>=<agent>:<provider>:<model>`，可重复。
  - 命令行先在内存里的 modes 上放人选，用的是从 `config_set_seat_at` 抽出的 `place_seat`，与 set-seat 同一套校验。
  - 结果通过 `DispatchInput.seats` 交给引擎，引擎只在本次替换该模式的人选，modes.json 不写。
- `place_seat` 新增一条拒绝：模式里写定了模型的位置不能换人。这类位置引擎本来就不读人选，写了也不生效。set-seat 同样受此约束。
- skill 1.24.0：
  - SKILL.md 加一条：用户点名模型时先找，只查同名程序就说没有不算数。
  - channels.md「按窗口配置派发」第 4 条改为 find → 一个用 / 多个让用户挑 / 没有才说没有，单次换人用 `--seat`，开场写明对应关系。
  - `skill-bundle` 已同步。
- 文档：`docs/CONSOLE-CHANNEL.md`（find 与 --seat 的契约）、`docs/MANUAL.md`（命令表与说明），`--help` 两行。

## 验证

```
cargo test                                → 709 passed, 10 ignored
skills/ai-cross: python -m pytest -q tests → 70 passed
```

真机（本机资源库，只读）：
- `resources find claude 5.5` 对上 7 个，都可派发：pi 的 claude-haiku-5-5，agy 的 claude-opus-5-5 与 claude-sonnet-5-5 各三档。Claude Code 的 opus 实际型号是 claude-opus-5，没对上，符合规则。
- `gpt 6 sol` → gpt-6-sol、gpt-6.1-sol，不含 gpt-5.6-sol。
- `glm 5.2` → 只对上 pi 的 glm-5.2。
- `kimi k3` → pi 的 kimi-k3 与 kimi 下 4 个。

隔离的 `AICROSS_HOME`，项目目录不存在，所以不会真派：
- `dispatch --preset blind --seat 1.1=pi:opencode-go:glm-5.2`：stderr 出现「这次临时换人（不改配置）：盲审模式 第 1 阶段第 1 个位置 → pi:opencode-go:glm-5.2」，随后按「项目目录不存在」拒绝。没有生成 modes.json。
- `--seat 1.1=pi:opencode-go:glm-9`：退出 2，列出 pi 下可选模型。

## 没做 / 不确定

- 没有真的派一次 `--seat` 到模型并拿回结果，只跑到派发前检查。引擎按传入人选建节点有单元测试。
- 「claude 5.5」这类说法常对上多个（opus / sonnet / haiku、不同档位），按规则由用户挑。主 agent 会不会照做取决于 skill 的遵守情况，没有在各家宿主里实测。
- 厂商别名表是手写的，新厂商要补。
- 没装 aicross 的用户走 `inventory.py`，本期只在 skill 里加了「看全部条目再说没有」，脚本本身没加按名字找。
