# TASK-77 一条入口命令、对齐参数、没装引擎也能记裁决

## 问题

1. **入口分散**：skill 触发后，模型要自己查 `AI_CROSS_PEER`、跑 `aicross config show`、再跑 `inventory.py show`，按三种结果读 `SKILL.md` 里不同的分支。2026-11-13 的宿主会话里，没装引擎的机器每次都白跑一次 `aicross config show`。
2. **两条路的参数不一致**：任务文本在 `dispatch.py run` 里是 `--task-file`，在 `aicross dispatch` 里是 `--text-file`；`--kind` 在 `dispatch.py review` 里是材料类型（code / general），在引擎里是任务类型（frontend / backend 等）。
3. **没装引擎时裁决无处记**：用户逐件选的「改 / 不改 / 以后不再报」只能经 `aicross adopt --verdict-file` 写入。
4. **同一份订阅的条目不连带排后**（TASK-76 遗留）：2026-11-15 OpenCode Go 月额度用尽，其下 DeepSeek、智谱失败后被排后，同一订阅的千问仍排在前面。

## 范围

只改 `skills/ai-cross/`，改完运行 `python scripts/sync-skill.py`。引擎 `src/` 不动（本机没有 cargo，无法验证）。

1. `dispatch.py start --host <宿主名>`：依次判断并打印下一步。
   - 带 `AI_CROSS_PEER=1`：只作答，不派发。
   - `aicross config show --host <主 agent>` 退出码 0：走引擎，列出默认模式和有位置的模式的人选，指向 `channels.md`「按窗口配置派发」。
   - 命令不存在：看 manifest。其他退出码：打印原因（stderr 第一行）后看 manifest。
   - 没有 manifest：退出码 3，指向 `setup.md`。有 manifest：打印清单与「审」「干活」「记裁决」三条命令。
2. 参数：`run` 认 `--text-file`（`--task-file` 照认）；`review --kind` 也认引擎的六个任务类型，`frontend`、`backend` 用代码审查模板，其余用通用模板。
3. `dispatch.py verdict --verdict-file <表> [--project] [--run]`：按引擎同一规则认表，存成该次派发目录的 `裁决.md`；不指定 `--run` 取最近一次派发。
4. 失败原因里有额度用尽字样时记 `quota: true`；选审查者时 `billing_source` 相同的条目一起排后。清单显示里加一行近一天排后的条目。
5. `SKILL.md`「从这里开始」改为先跑 `start`、照输出走；`channels.md` 补命令说明。skill 版本 1.30.0。

## 不做

- 不改引擎的参数名与输出。
- `verdict` 不写 `track/outcomes.jsonl`（那份记录由引擎维护）。
- 不加 `decided` 子命令，没装引擎时已定事项仍直接编辑 `decided.md`。
- `run` 不加 `--kind`：脚本没有用它的地方。

## 验证

`cd skills/ai-cross && python -m unittest discover -s tests`；真机跑 `start` 的各情形、`verdict`、两个参数别名。
