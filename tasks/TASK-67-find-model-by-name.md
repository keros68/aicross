# TASK-67 按名字找模型、单次换人

用户 10-28 反馈：pi 里装了 Claude 的模型，对主 agent 说「claude 5.5」，主 agent 去找 Claude Code，没找到就回答没有。另有用户点名模型后派错、要再强调一遍的情况。

原因：
- 没有程序把用户口头说的名字对应到资源库里的模型，主 agent 按字面猜 agent。
- 写人选时模型编号要一字不差。
- `aicross dispatch` 不能单次换人；skill 让主 agent 改走 `dispatch.py --entry`，依赖另一份盘点清单，很多用户没有，于是退回窗口里的默认人选。

## 要做

1. `aicross resources find <名字>`
   - 在所有 agent 下的模型里找，比对模型编号、实际型号、提供方、厂商、agent。
   - 忽略大小写、空格、横线、点号；厂商与系列互认（claude ↔ anthropic / opus / sonnet / haiku 等）；版本号从头对上才算。
   - 输出每个对上的模型经哪个 agent、状态、能否派发，以及可直接用于派发的目标；能派发的在前。没对上时列出只按名字对上的模型。
2. `aicross dispatch --seat <阶段>.<位置>=<agent>:<provider>:<model>`
   - 只对这一次派发换人，不写 modes.json；校验与 `config set-seat` 相同。
   - 模式里写定了模型的位置不读人选，写进去不生效，set-seat 与 --seat 都当面拒绝。
3. skill
   - 用户点名模型时先跑 `resources find`；一个能用就用，多个让用户挑，一个都没有才说没有。
   - 单次换人用 `--seat`；开场一行写明「用户说的名字 → agent · 模型」。
   - 只查名字相同的程序就说没有，明确不允许。
4. 文档：`docs/CONSOLE-CHANNEL.md`、`docs/MANUAL.md` 与 `--help`。

## 完成条件

- 单元测试：拆词、跨 agent 找到、别名、排序、版本号从头比、没对上时的 related；`--seat` 的解析、校验、不写配置、写定位置拒绝；引擎按传入的人选建节点。
- 真机：用本机资源库跑 `resources find`；`dispatch --seat` 在隔离目录下跑到派发前检查。
