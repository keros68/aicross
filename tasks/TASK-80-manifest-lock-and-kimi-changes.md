# TASK-80 manifest 加锁，kimi 动过项目文件时报出来

## 问题

| 位置 | 实际情况 |
|---|---|
| manifest | 两条派发同时收尾时各自读、改、写，后写的盖掉先写的那次冒烟日期或失败标记 |
| kimi 执行者 | 没有只读档。2026-11-15 真机：让它在自己目录之外新建一个文件，建成了；脚本的输出里看不出来 |

## 范围

只改 `skills/ai-cross/`，改完运行 `python scripts/sync-skill.py`。

1. `inventory.manifest_lock`、`inventory.update_manifest`：读改写在 `manifest.lock` 的独占锁内完成。`dispatch.py` 收尾时的三处更新与 `inventory.py` 的 `save`、`set`、`diff` 都走它。
2. 选中 kimi 时，派发前后各取一次项目目录的 git 状态，有变化就在「注意」里列出文件。

## 不做

- 等 30 秒拿不到锁时不报错，照常写。
- 不检查项目目录之外、被 git 忽略、或项目不在 git 里时的改动。
- 不拦 kimi 写文件。

## 验证

```bash
cd skills/ai-cross && python -m unittest discover -s tests
```

真机：12 个进程同时更新同一份 manifest；让 kimi 在一个 git 项目里新建文件。

## 交付

`tasks/REPORT-80.md`；skill 版本升到 1.30.3。
