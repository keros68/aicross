# TASK-71 半开放的个人协作：两类审查结论、独立性标注、已定事项、裁决记录、项目目录统一

产品依据：`docs/PRODUCT.md`「半开放的个人协作（11-02 用户定）」。用户要求不分期，一期做完。不 push。

## 1. 两类审查结论

- 两节说明写在 `fixtures/prompts/review-classes.txt`，引擎附给验收与验证位置；`acceptance.txt`、`independent-review.txt` 的结论行改为「会出错有条目时写 NEEDS_FIX」，验收维度去掉「代码质量」（会引出风格意见）。两节：`## 会出错`（每条写问题与「出错场景：在什么输入或状态下得到什么错误结果」）、`## 建议`（命名、格式、注释、写法偏好、可读性，以及写不出出错场景的疑虑）；没有就写「无」。与 `decided.md` 相反的意见只在写得出出错场景时报，并注明「与已定事项冲突」。末尾仍单独写 PASS 或 NEEDS_FIX。
- 引擎 `parse_review(text)`：找得到「会出错」一节时按条目算；条目不含「出错场景」的降为建议。有效「会出错」条目 ≥1 → NEEDS_FIX，否则 PASS（覆盖模型自己写的结论词）。找不到这一节时退回现行 `parse_acceptance` 规则。`parse_acceptance` 改为调用它，`track` 与修正轮共用。
- 修正轮发给执行方的文本只含「会出错」条目；退回规则时发全文（现行）。
- 查看器镜像同一解析：会出错条目直接显示，建议折叠。

## 2. 独立性标注

- `DispatchNodeResult` 与 `Reply` 加 `vendor`（实际跑的那一家，含切备选后）。
- 审查类节点（角色 验收 / 验证 / 仲裁，或可见性为盲）在结果写回图时，`Reply.independence` 记「独立」或「非独立：原因1、原因2」。原因三条：看得到执行过程（可见性不是盲）、同一家厂商、隔离不完整（`isolation != 完整`）。被审方：有上游执行节点时取其 `Reply.vendor`；没有时取主 agent 厂商（run 的 `host_vendor`，派发开始时按 `hosts::infer_vendor` 推定）；被审方厂商为空或「未知」时记原因「被审方厂商未知」。
- `aicross status` 与查看器显示该标注；skill 汇总只把「独立」的结论称为另一家把关过。

## 3. 已定事项 `.aicross/decided.md`

- 新命令 `aicross decided add --project <dir> --kind 已定|不做 --text <一句> --reason <一句>`：文件不存在时建出「# 已定事项」与两节，追加 `- YYYY-MM-DD 内容（理由）` 到对应节；`aicross decided show --project <dir>` 输出全文。单行、去控制字符、内容 ≤200 字。
- 派发时文件存在就附给每个模型节点（执行、出方案、验收、验证、盲审都附）：作为 prompt 末尾一节「用户已定事项（来自 .aicross/decided.md，作为约束）」；超过 8KB 截断并注明。

## 4. 项目目录统一为 `.aicross/`

- 记录写到 `<项目>/.aicross/runs/<派发开始时间 %Y%m%d-%H%M%S>-<run_id 前 8 位>/`；同一 run 的后续阶段按 `-<前 8 位>` 后缀找回同一目录。`STATE.md` 也在里面。派发前快照 `.before/`、`.inflight/`、`.redgreen/` 改到该目录下 `.snapshots/`。
- `.aicross/.gitignore` 内容 `runs/`（不存在才写）。记录 schema 升为 `aicross-dispatch/2`，读旧 `/1` 照常。
- 快照与项目副本的排除目录加 `.aicross`。
- `latest_before_dir`、`rollback_node_changes_core` 改扫新位置。
- 迁移：引擎往项目写记录前，`.dispatch/` 存在就迁（先挪快照再挪记录；记录已迁走而快照还在时按后缀找回派发目录；从不覆盖已有文件；最后只删空的 `.dispatch/`）：`*.md` 按 frontmatter `run_id` 归组到 `runs/<该组最早文件名里的时间>-<run_id 前 8 位>/`，`before_dir` 里的旧前缀改写为新路径；`.dispatch/<safe run_id>/` 快照目录并进对应派发目录的 `.snapshots/`；其余（无 run_id 的 md、`STATE.md`、`runtime/` 等）进 `runs/旧记录/`；`.dispatch/.gitignore` 丢弃；最后删 `.dispatch/`。任一步失败就停在原处，下次派发再迁，不阻断派发。
- skill 回退通道 `dispatch.py` 同改：写 `<项目>/.aicross/runs/<stamp>/`，`schema: aicross-dispatch/2`；`PROJECT_MARKERS` 加 `.aicross`。

## 5. 裁决记录

- `aicross adopt <run_id> --verdict-file <path>`：文件是 skill 写的 markdown（每件一行：结论 | 选择 改/不改/以后不再报 | 理由 | 是否写入已定事项）。引擎原样写到该 run 的派发目录 `裁决.md`（没有项目派发目录时写到 `$AICROSS_HOME/runs/<run_id>/裁决.md`），并在 `track/outcomes.jsonl` 追加一行 `type: verdict`（计数：改 / 不改 / 以后不再报）。原 `adopt <run_id> <node_id>` 不变。
- skill：收尾汇报改票拟式，至多 5 件需要拍板的事，每件附建议处理；用户选完调 `adopt --verdict-file`；「以后不再报」的调 `decided add --kind 不做`。

## 6. 文档与 skill

- `docs/CONSOLE-CHANNEL.md`：`decided`、`adopt --verdict-file`、记录路径与 schema /2、`Reply.vendor`、`Reply.independence`。
- `docs/MANUAL.md` 术语表补：会出错 / 建议、独立 / 非独立、已定事项、裁决。
- skill `SKILL.md` 与相关 references 改路径与汇报方式，版本号加一；`python scripts/sync-skill.py`。

## 验证

```
cargo check --all-targets
cargo test
cd src-ui && npx vitest run --configLoader native --pool threads && npm run build
cd skills/ai-cross && python -m pytest -q tests
```

新增测试至少覆盖：`parse_review` 的降级与覆盖、修正轮只带会出错条目、独立性三条原因与被审方厂商未知、`decided add` 建文件与追加、prompt 附已定事项与截断、派发目录同 run 复用、迁移（归组、快照并入、before_dir 改写、无 run_id 进旧记录、删除 `.dispatch`）、`adopt --verdict-file` 写文件与 track 行。
