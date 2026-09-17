# TASK-28 首版审查 · GLM-5.3-Flash（cc-switch，只读）

## 总判

**可交付但有未闭合项** —— 任务书三条主旨全部落实且接线真实，盲审边界（推理/轨迹/引擎提示/密钥）核过没破口；有一处截断标注在 worktree 警告组合下会漏，属低危窄口径缺口。

说明：本会话无 shell 工具，`cargo`/`vitest`/`git diff` 均无法复跑，行尾字节计数与红跑记录只能采信报告；以下结论均来自直接读码。

## 问题清单（按严重度）

1. **低｜截断标注在「worktree 警告 + 长回复」组合下漏标**
   - 位置：`src/dispatch.rs:4113-4123`（2000 字上限先于警告拼接生效）与 `src/dispatch.rs:4496-4497`（`truncated` 按剥警告后的答案字符数判断）。
   - 问题：`stdout_summary` 是「警告行 + 正文前 2000 字」。若执行节点 worktree 回退单写者（警告约 20-30 字）且回复超长，剥掉警告后答案 < 2000 字，`（回复较长，已截断至前 2000 字）`不出现，但答案实际在 1970 字附近被切断。
   - 复现：上游 `write="worktree"` 创建失败回退，`channel_output.text` 长于约 2000 字，diff 为空 → ANSWER.md 末尾无截断说明。
   - 改法：在产物构建处记录「摘要已被封顶」的事实（或剥警告时一并传递原始长度），`readonly_answer_material` 据此补标注；或把警告剥离挪到 `stdout_summary` 组装之前并独立封顶。

2. **低｜空 `stdout_summary` 仍会写出空 ANSWER.md 并附指路句**
   - 位置：`src/dispatch.rs:4484-4517`。
   - 问题：全部上游 `stdout_summary` 为空时，仍写空 ANSWER.md，prompt 出现「执行者未改文件，其最终回复见 ANSWER.md」，对盲审者是误导性指路。
   - 复现：现役五类通道解析器在 status=ok 时均拒绝空正文（claude 取 `result`、codex 取 `agent_message`、kimi 取 assistant 正文、pi 取 assistant text、cc-switch 取 stdout，均已核实空则报错），所以正常链路难触达；但旧图/恢复图中带空 `stdout_summary` 的产物（serde default 补空）会触发。
   - 改法：拼接后的答案 trim 为空时返回 `None`（连同指路句与内联都不出）。

3. **信息｜materials 附件与生成文件同名会覆盖（既有缺陷类，ANSWER.md 新加入）**
   - 位置：`src/dispatch.rs:4586-4597`（写 ANSWER.md）与 `4612-4630`（复制附件，重名检查只在附件之间）。
   - 问题：盲节点附件里若带名为 `ANSWER.md` 的文件，会覆盖刚生成的答案文件且 `names` 出现两次。`REQUEST.md`/`diff.patch` 在 TASK-28 之前就有同类问题，本次未扩大边界，仅多一个成员。
   - 改法（可选）：把生成文件名并入 `used` 冲突集合。

4. **信息｜codex/kimi 的 `text` 是全部 agent 消息拼接，不是仅最后一条**（`src/channels/mod.rs:677-686`、`812-840`）。仍是执行者自己的可见回复，不含推理块与 tool 内容，任务书要求的边界（不送推理/轨迹）不破；报告的表述「Codex 取 agent_message」「Kimi 取 assistant 正文并排除 tool」与代码一致，无需改。

5. **信息｜`types.ts` 改动非编译必需**：前端对 artifacts 全程原样透传（见下），不加这个字段声明也能编译运行，报告「非改不可」的论证偏薄。但改动仅为可选字段声明，使 TS 契约与 Rust 结构体同步，零行为影响，`tauri.ts` 确实未动，可接受。

## 核过没问题的要点

- **任务书第 1 条**：门槛为「至少一个上游、全部 diff 为空且全部 changed_files 为空」（`readonly_answer_material`，dispatch.rs:4484-4491）；单上游不加标题、多上游按 `## 上游节点 <id>` 分节、旧产物无来源字段回退顺序号（4492-4508）；脱敏走与 REQUEST.md 同一外发开关（4517、4539），且产物侧 `safe_trace` 已先行脱敏，只紧不松；ANSWER.md 进材料名单（4590）；prompt 指路句中性（4470、4591-4592）；小于 30KB 内联与 diff.patch 同口径、同字节判断（4593）。仅盲节点、且派发前验证通过才物化（4528、3846）。
- **任务书第 2 条**：任一上游 diff/changed_files 非空即整体不出（4486-4489）；反例测试 `task28_nonempty_diff_does_not_write_answer`（7931-7950）钉住目录精确等于 `["REQUEST.md","changed","diff.patch"]`、names 精确三元、prompt 不含 ANSWER.md。既有 `blind_acceptance_directory_contains_only_artifact_delivery`（7779）断言未动，仅为新字段补了结构体字面量。
- **任务书第 3 条**：`docs/PRODUCT.md:203` 已补「只读任务…作为 ANSWER.md 随盲审材料提供；答案属于交付物，不包含推理过程」，插在原句行内、未新增行，行尾结构大概率未变（字节计数未能独立核验）。
- **盲审边界**：ANSWER.md 唯一来源是 `channel_output.text`（4113 → 4170 → 4496），逐 runner 核实只取最终回复字段，工具轨迹/原始输出另存（`record_stdout`）；引擎提示只有 worktree 警告一行，且 `answer_summary` 精确匹配前缀剥离并处理 `\r`（4474-4482）；ANSI 经 `safe_trace` 剥除。未发现推理过程、工具输出或密钥绕过脱敏进盲目录的路径。
- **`source_node_id` 副作用**：serde `default` + 空值跳过序列化（161），`r3_1` 测试（6000-6023）钉住空值不出现在序列化键里 → 旧留痕/旧图字节不变、缺字段反序列化为空；生产链路端到端核实：dispatch.rs:4112 写入 → 结果 JSON → `dispatch.ts:308` 原样入图 → `context.ts:160-180` 原样收集 → `dispatch.ts:1196` 原样进请求 → Rust serde default 兜底，多上游标题在生产环境拿得到真实 id；无任何逻辑按值比较 `DispatchArtifacts`，展示路径（`artifactSummary` 等）不读该字段。
- **测试接线与可红性**：`task28_readonly_answer_reaches_blind_review_through_dispatch_stage_core`（7818-7903）连续两阶段走 `dispatch_stage_core` 真实入口，stub 按 cc-switch 契约（正文 stdout、`[usage]` stderr）构造，断言产物 `source_node_id`、空 diff、盲目录 ANSWER.md 原文、任务文件含指路句与内联全文；python 缺失静默跳过与既有测试惯例一致（6419 等）。其余五格断言的都是旧实现不会产出的文件/内容，结构性必红；非空 diff 反例旧实现本绿这一点报告如实说明并用临时摘保护变异证红，诚实。
- **范围**：git status 仅 `src/dispatch.rs`、`src-ui/src/graph/types.ts`、`docs/PRODUCT.md`、新增 `tasks/REPORT-28.md`，未触碰 TASK-27 禁改清单，未动 backlog（任务书豁免）。
