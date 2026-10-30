# REPORT-69 模型的使用记录与用途标签

## 改了什么

- `src/track.rs`（新）：任务类型表与解析；按最终的图把执行链判成一次通过 / 修正后通过 / 未通过 / 失败 / 无验收 / 结论不明（叫停、未跑、切了备选的不记）；`track/outcomes.jsonl` 追加与读取（同一类型、run、节点只取最后一行）；`model-notes.json` 读写（加文件锁）；`stats` 汇总（`judged` ≥ 5 才给通过率，`ranked` 与 `tagged`）。
- 引擎：`EngineRun.task_kind`、`DispatchInput.task_kind`；后台跑完且引擎没有中途出错时写结果行；`adopt` 写采纳行。`parse_acceptance`、`USABLE_STATES` 改为 crate 内可见。
- 命令行：`dispatch --kind`；`stats [--kind]`；`resources note`；`--help` 一览。
- 窗口：`model_track`、`model_note_set` 两个命令；资源页模型行显示「擅长：」与「记录：」，「标注用途」就地改标签与备注。
- skill 1.25.0（`channels.md`「按窗口配置派发」第 1、5 条与命令示例）：派发带 `--kind`；派发前读 `stats` 按规则提示换人；挑方案类任务提示擂台模式；用户在对话里认可时调 `adopt`。已跑 `scripts/sync-skill.py`。
- 文档：`docs/MANUAL.md`（命令表、「使用记录与用途标注」一节、术语表），`docs/CONSOLE-CHANNEL.md`（`--kind`、`task_kind`、记录格式与 `stats` / `resources note` 契约），`docs/PRODUCT.md`（设计一节；「已采纳、未排期」改为一键推荐读记录）。

## 怎么验证的

- `cargo test`：`test result: ok. 723 passed; 0 failed; 10 ignored`。新增 14 个测试：`track::tests` 10 个（类型解析、模型写法、六种结果、修正轮、验证重试含早期无前缀文本、叫停 / 备选 / 未跑不记、验收失败为结论不明、标注读写清空、汇总门槛与排序和采纳去重），`console_cli::tests::task69_*` 2 个，`engine::tests::task69_*` 2 个（采纳写行且重复只算一次；后台 `run_background_with` 跑完一次经济形状的派发写出 `first_pass` 与 `task_kind`），另在引擎写盘失败的既有测试里断言不写记录；`accept_criteria` 的 run 文件测试加了 `task_kind`。
- 用本机 30 张真实派发图试跑判定（临时测试，跑完已删）：发现早期验证重试文本没有「[修正 N/M]」前缀，被拆成多条链，已改为也认「请修正后重新运行」并补测试。
- `cd src-ui && npm run build`：通过；`npx vitest run`：`23 passed (23)` 文件，`256 passed` 测试；`npx tsc --noEmit -p .` 无输出。
- `python -m pytest -q skills/ai-cross/tests`：`70 passed`。
- 命令行实跑（临时 `AICROSS_HOME`）：`resources note codex::gpt-5.6-luna --tags 后端,前端 --note 稳` 输出标签按表序 `["frontend","backend"]`；只读、写法错误（退出 2）、`stats --kind 前端`（`tagged` 含该模型、`usable: true`）、`stats --kind 视频`（退出 2）均符合预期。
- 窗口（`tauri dev`，临时 `AICROSS_HOME`，写入样例记录）：资源页显示「擅长：前端、后端。稳」「记录：前端 7 次，一次通过 5，通过率 71%，采纳 1；文案 1 次」；点「标注用途」出现类型勾选与备注；取消「后端」后保存，行内即时变为「擅长：前端。稳」。

## 没做什么

- 没有真的派发到外部模型做端到端实跑；后台写记录的路径由上面的引擎测试覆盖。
- 不补算旧派发；一键推荐不读记录；出方案、验收、验证位置不记。

## 不确定处

- 蜂群里同一人选的多条执行同父时，验证重试按建立先后分给各链，可能分错（代码旁已注明）。
- 「验收通过」是模型的结论，通过率只能作参考。任务类型由 skill 判断，可能分错。
- skill 的提示规则（通过率差 20 个百分点、样本 5 次）是初定值，需要数据攒起来后再看。
