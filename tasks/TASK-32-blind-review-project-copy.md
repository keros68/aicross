# TASK-32 只读问题的盲审附项目只读副本（09-17）

对应 backlog 85，**用户 09-17 裁决选 (a)**。口径已写进 `docs/PRODUCT.md`（搜「09-17 用户裁决（选项 a）」）。**只做这一件事。** 不碰 `src-ui/**`，除非契约非改不可（写理由）。

## 问题（编排者 09-17 窗口实测三次）

控制台里用经济模式问「用一句话说明 beta.py / alpha.py / tests/test_beta.py 是干什么的，不要改文件」：执行者答对，`ANSWER.md`（TASK-28）也送到了盲审，但盲目录里没有项目文件，验收与验证都判「材料中无法核实、疑似臆造」→ NEEDS_FIX → 白跑修正轮 → 收尾「未通过」。只读问题永远过不了验收。

## 口径

「盲」= 看不到执行者的过程、推理、工具轨迹和其他审查者的结论；**不是**看不到项目。审查者要能自己打开文件核对答案。

## 要做

1. **触发条件与 `ANSWER.md` 相同**：上游产物 diff 全空且 changed_files 全空时（`readonly_answer_material` 返回 Some 的那条路径），在盲目录里另建 `project/`，放项目的只读副本；材料名单加上 `project/`，prompt 补一句指路（中性简短，例如「项目只读副本见 project/，用于核对回答，不要修改」）。diff 非空时行为完全不变。
2. **副本内容**：
   - 项目是 git 仓库时只取**版本库跟踪的文件**（`git ls-files`，沿用本文件里现有的 git 调用封装与取消键约定），天然排除 `.dispatch/`、`node_modules/`、`target/` 等未跟踪内容。
   - 不是 git 仓库时：先查本仓已有的派发前快照（`before` 快照 / `artifact_diff_for_project` 一带）用的排除规则，复用它，不要另写一套。
   - 无论哪种，**一律排除**：`.dispatch/`、`.git/`、密钥类文件（`.env`、`.env.*`、`*.pem`、`*.key`、`id_rsa*` 等——先看本仓是否已有这类清单，有就复用）。
   - 设上限：单文件超过 1 MB 跳过、总量超过 50 MB 或 5000 个文件时停止复制；被跳过 / 截停时在 prompt 指路句后写明「副本不完整：…」，不要静默。
   - 文本文件走现有脱敏（`redact_text`，外发开关口径与 `REQUEST.md` 一致）；二进制文件按原样复制还是跳过，看本仓现有判定，报告里写清。
   - 路径安全：沿用 `changed/` 写入时的安全相对路径检查。
3. **副本来源时点**：只读任务 diff 为空，项目当前状态即执行者看到的状态；直接取派发时的项目目录即可。若执行节点走过 worktree，取执行者实际所在目录（`effective` 工作目录），报告里写清怎么确定的。
4. 与 TASK-28 R1 的附件重名保护一致：`project` 也并入保留名，附件同名时报错。
5. 盲目录清理沿用现有逻辑，确认副本随盲目录一起清掉、不在项目里留东西。

## 不做

- 不给改代码类任务加副本（它们已有 `diff.patch` + `changed/`；要不要加以后再定）。
- 不改修正轮、验收解析、`ANSWER.md` 既有内容。

## 验证

`cargo check --all-targets` 零 warning、`cargo test`（注意 TASK-31 刚修过并发偶发失败，全量至少连跑 3 次）、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`。

- **必须走真实接线**：参照 `task28_readonly_answer_reaches_blind_review_through_dispatch_stage_core`，在一个临时 git 项目里（含一个被跟踪的 `src/beta.py`、一个未跟踪文件、一个 `.env`、一个 `.dispatch/` 下的文件）经 `dispatch_stage_core` 跑「只读执行 → 盲验收」两阶段，断言盲目录 `project/src/beta.py` 存在且内容一致、未跟踪文件 / `.env` / `.dispatch` 都不在、prompt 含指路句。
- 反例：diff 非空时没有 `project/`；非 git 项目走复用的排除规则；超限时 prompt 写明不完整；附件名 `project` 报错；副本里的密钥样式文本被脱敏。
- 每条先确认去掉实现会红，在报告里写清怎么红的。

保持文件原有换行（`src/server.rs`、`src/main.rs` CRLF，其它 Rust LF，`docs/PRODUCT.md` CRLF）；不 `cargo fmt`；不起窗口、不发真机请求、不调真实模型；不 commit、不 push。报告写 `tasks/REPORT-32.md`。backlog 不用改。
