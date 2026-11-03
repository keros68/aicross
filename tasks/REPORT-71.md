# REPORT-71 半开放的个人协作

任务书：`tasks/TASK-71-semi-open-personal.md`。产品依据：`docs/PRODUCT.md`「半开放的个人协作（11-02 用户定）」。本期由主线程直接实现（用户要求一次做完），未按 Runbook 外派实现者。

## 改了什么

- **两类审查结论**：`fixtures/prompts/review-classes.txt` 是「会出错（必须写出出错场景）/ 建议」两节的说明，引擎附给验收与验证位置（`role_prompt`）；`acceptance.txt`、`independent-review.txt` 的结论行改为「会出错有条目时写 NEEDS_FIX」。`engine::parse_review` 按两节拆，条目不含「出错场景」降为建议；`parse_acceptance` 有「会出错」一节时按条目定结论，没有时按旧规则取结论词，修正轮、`track` 与查看页共用。修正轮发回执行方的文本只含「会出错」条目（`correction_text`）。`fixRoundLimit` 上限由 9 改为 3。
- **独立性标注**：`DispatchNodeResult.vendor`、`Reply.vendor`、`Reply.independence`、`RunNode.independence`。`independence_label`：审查类节点（验收 / 验证 / 仲裁或可见性为盲）三条都满足才「独立」，否则「非独立：看得到执行过程、隔离不完整、同一家厂商 / 审查方厂商未知 / 被审方厂商未知」。被审方取上游节点的 `Reply.vendor`，没有上游时取主 agent 厂商（`EngineSettings.host_vendor`，派发开始时按 `hosts::list_hosts_at` 推定，不写盘）。
- **已定事项**：`src/project_dir.rs` 的 `add_decided`；命令 `aicross decided add|show`。`engine::decided_section` 读 `.aicross/decided.md` 附在每个模型节点任务文本末尾，超过 8KB 截断。
- **项目目录**：`src/project_dir.rs`。记录写到 `<项目>/.aicross/runs/<开始时间>-<run_id 前 8 位>/`，`STATE.md` 同目录，快照 `.before` / `.inflight` / `.redgreen` 在其 `.snapshots/` 下；`.aicross/.gitignore` 为 `runs/`；记录 schema `aicross-dispatch/2`，读 `/1` 照常；快照与项目副本排除 `.aicross`。`migrate_legacy` 把旧 `.dispatch/` 按 `run_id` 归组迁入、改写 `before_dir`、快照并入 `.snapshots/`、其余进 `runs/旧记录/`、删 `.dispatch/`。`latest_before_dir`、回滚、`recover` 的快照查找改到新位置。
- **裁决**：`aicross adopt <run_id> --verdict-file <表>` → `engine::adopt_verdict` 写 `裁决.md`，`track` 追加 `type: verdict` 行（`stats` 不读）。
- **查看页**：卡片加「独立性」；审查回复分了两类时列出「会出错」，建议与回复原文折叠；结论与引擎同一规则。
- **skill**（1.26.0）：`dispatch.py` 审查模板带两类说明，附 `decided.md`，记录写 `.aicross/runs/<开始时间>/`，迁移旧 `.dispatch/`，每路标独立性，小结骨架改为「要你拍板（至多 5 件）/ 建议」；`SKILL.md` 收尾改票拟式并写明裁决与已定事项的写法；`channels.md` 补第 6 条与记录契约 /2；其余 references 与 README 改路径。`python scripts/sync-skill.py` 已同步。
- 文档：`docs/PRODUCT.md` 新节并改旧路径，10-01「记住被核实为错的审查意见」一条改写；`docs/CONSOLE-CHANNEL.md`、`docs/MANUAL.md`（命令表、`.aicross/` 一节、审查结论与独立性一节、术语表四行）。
- 验收维度去掉「代码质量」：它会引出风格意见，与「不报风格」相冲。
- 金标准夹具 8 份：只多了审查说明文本与 `independence` 字段，用脚本核对过除此之外与原夹具逐字段一致。

## 怎么验证的

- `cargo check --all-targets`：无 warning。
- `cargo test`：`737 passed; 0 failed; 10 ignored`。
- `cd src-ui && npx vitest run --configLoader native --pool threads`：`256 passed`；`npm run build` 通过；`npx eslint src` 无输出。
- `cd skills/ai-cross && python -m pytest -q tests`：`73 passed`。
- 真机（release 版，`D:\aicross-data\scratch\t71-e2e`，跑完删除）：盲审模式三路（codex gpt-6-luna、pi zai-coding-cn glm-5.3-flash、kimi k3），材料是两个带缺陷的小函数，项目里预置了 `decided.md` 和旧 `.dispatch/`。
  - 第一次：记录落在 `.aicross/runs/20261102-165552-317a86da/`，旧 `.dispatch/` 迁到 `runs/20260913-191317-oldrun/`，`before_dir` 已改写；独立性为 codex、kimi「非独立：隔离不完整」，pi「独立」；三路都引用了已定事项。发现两个验证位置没按两类作答（验证角色没有模板，两类说明没送到），改为 `review-classes.txt` 同时附给验收与验证。
  - 第二次：三路都按两节作答；用 `parse_review` 解析三份真实回复，分别得到会出错 2 / 3 / 3 条、建议 1 / 4 / 3 条，结论都是 NEEDS_FIX。`adopt --verdict-file` 写出 `裁决.md`，`outcomes.jsonl` 多一行 verdict（测试行随后删除）。

## 盲审

用 aicross 自己的盲审模式审本期改动（材料：任务书 + 实现部分的 diff + `project_dir.rs`，108KB；文档与测试没放进材料）。codex gpt-6-luna 完成；kimi k3 完成，但引擎把它的材料截到 30KB，只看到前半；pi glm-5.3-flash 超过 600 秒超时。两家都是「非独立：隔离不完整」。

采纳并已修：
- 迁移重试会错配快照（kimi）：上次记录已迁走、快照没迁完时，重试把快照放进「旧记录」，改写过的 `before_dir` 悬空。改为先挪快照再挪记录，并按后缀找回已迁的派发目录；新增 `migrate_retry_puts_orphan_snapshot_back_with_its_run`，去掉修复时该测试失败、恢复后通过。
- 迁移可能覆盖同名记录、最后递归删除可能删掉没枚举到的条目（codex）：改为从不覆盖（加 `-2` 后缀），最后只删空目录。Python 版同改。
- 以「结论」开头的句子会截断「会出错」小节（codex）：只有含 PASS / NEEDS_FIX 的「结论」行才算结论行；补了测试。
- `adopt --verdict-file` 收下一张认不出任何选择的表（codex）：现在拒绝并说明原因；补了测试。
- skill 与引擎的「会出错」定义措辞不一致（kimi）：skill 改为同一句。

未采纳：
- 「文档、skill、测试没有改」（两家）：这些改动在材料之外，本报告上文有命令结果。
- 「已定事项不是 prompt 的最后一节」（codex）：其后只有固定的如实汇报与禁止外部动作两句，保持现状。
- 「失败、跳过的审查节点没有独立性标注」（codex）：没有结论就没有可标的对象，按设计只标完成的节点。
- `clean_line` 把换行合并成空格而不是拒绝（codex）：任务书要求的就是单行、去控制字符。
- 短 id 碰撞（codex）：同一项目里两个 run_id 前 8 位相同的概率可忽略，不处理。

## 没做

- 独立性在 codex 与 kimi 上总是「隔离不完整」：沿用现有 `channels::isolation_for`（只有 claude、pi 的盲位置算完整），本期没改隔离本身。盲审模式默认人选里 codex、kimi 会因此一直显示「非独立」。
- `stats` 与一键推荐不读裁决行。
- `decided.md` 的过期清理、查看页里编辑已定事项：按产品决定不做。
- 迁移出错时只是保持原样、下次再迁，没有把原因显示到对话里（引擎派发结果没有说明字段可放）。
- 盲审里 kimi 的材料被截到 30KB、GLM 超时：大材料的盲审本身还有这两个限制，本期没处理。
- 未在窗口里实际打开查看页看新卡片（只跑了构建与单测）。
- macOS / Linux 未验证。

## 不确定处

- 「会出错」的判定靠条目里出现「出错场景」四个字，模型改写法（如「触发条件」）会被降为建议。真机三家都照写了。
- 主 agent 厂商在派发开始时现推定；用户事后改了 `config set-host-vendor`，旧 run 的标注不会更新。
