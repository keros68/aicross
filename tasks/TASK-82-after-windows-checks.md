# TASK-82 Windows 核对后留给 skill 的三处

## 问题

`tasks/REPORT-81.md` 留了三项要定的事，只涉及 skill：

| 位置 | 实际情况 |
|---|---|
| `review --text-file` | 在 `review` 里是被审的原始材料，在 `run` 与 `aicross dispatch` 里是任务文本，同名不同义 |
| agy 文件模式 | 材料超过 200 KB 时写进 `REQUEST.md` 让对方读，文件里的模板仍有「不要使用任何工具」 |
| `ENGINE_HOSTS` | 手抄了四个宿主名，引擎认的其他宿主到不了引擎 |

## 范围

只改 `skills/ai-cross/`，改完运行 `python scripts/sync-skill.py`。

1. `review` 的原始材料文本用 `--material-file`，`--text-file` 照认。
2. 写进 `REQUEST.md` 的模板把那一句换成「除读取本文件外不要使用其他工具」；内联时不变。
3. `ENGINE_HOSTS` 只留 `claude-code`、`kimi-code` 两个别名，其余宿主名原样交给 `aicross config show --host`。

## 不做

- 不统一引擎与 skill 的选人参数（`--preset` / `--seat` 对 `--entry` / `--tier`）：两边的派发方式不同。
- 引擎一侧的问题见 `tasks/TASK-83-engine-followups.md`。

## 验证

```bash
cd skills/ai-cross && python -m unittest discover -s tests
```

## 交付

`tasks/REPORT-82.md`；skill 版本升到 1.30.5。
