# TASK-78 盲审 skill 后的五处修正

## 问题

2026-11-15 对 `SKILL.md`、`references/dispatch.py`、`references/inventory.py` 做了一次盲审（OpenAI gpt-6.1-sol 经 codex、Moonshot k3.1-pro 经 kimi；xAI grok-4.7 一路超时）。对照代码核实后，下面五处成立且改动小：

| 位置 | 实际情况 |
|---|---|
| `build_prompt` | 审查时附上的「用户已定事项」没有脱敏，`run` 一侧是脱敏的 |
| `print_review` | `--full` 仍在 12,000 字处截断，与帮助文字不符 |
| `choose_reviewers` | `--n` 为负数时会选中全部厂商 |
| `cmd_review` | 手工申报的条目被选中后只会跳过，不记失败，下次仍先选它 |
| `print_diff` | 有通道读不到清单时仍说「档位里的模型都还在」 |

## 范围

只改 `skills/ai-cross/`，改完运行 `python scripts/sync-skill.py`。

1. 已定事项与材料一并脱敏。
2. `--full` 不截断。
3. `--n` 小于 1 时拒绝；帮助文字写明没有要回避的厂商时默认两家。
4. 审查里被跳过的一路同样记 `last_failure`。
5. 有通道读不到清单时，结论限定为「读到的通道里」。

## 不做

- 同一秒启动的两次派发共用目录、同模型不同 provider 的记录重名、manifest 并发写入：都要两条命令同时跑才出现，改动涉及记录命名。
- 非零退出码不作为失败依据：`AGENTS.md` 写明各家退出码不可信。
- 除 pi 外各通道的真身核对、claude 指向第三方端点时的厂商认定、agy 大材料与「不要用工具」的冲突、条目 ID 规范化为空：本机无法实测或需要另行设计。
- codex 执行者在只读沙箱里仍能读本机文件，与「看不到项目文件」的说明不符：未实测。

## 验证

```bash
cd skills/ai-cross && python -m unittest discover -s tests
```

## 交付

`tasks/REPORT-78.md`；skill 版本升到 1.30.1。
