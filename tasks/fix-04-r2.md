# TASK-04 修正轮 R2（qwen3.8-max 盲审的独有发现）

先读 `AGENTS.md`、`tasks/TASK-04-pipeline.md`、`tasks/REPORT-04.md`（含修正轮 R1）、`tasks/review-04-qwen.md`（完整审查，含 GLM 未提的条目）。逐条处理，每条在 `tasks/REPORT-04.md` 末尾新增「修正轮 R2」一节里写「已修：怎么改的」或「不采纳：原因」。验证跑 `cargo check`、`cargo test`、`cd src-ui && npx vitest run`、`cd src-ui && npm run build`，命令与输出原文贴进报告。不起窗口。不要 commit，不要 push。真机请求本轮不发。

## 必修

1. **盲验收读不到产物**（qwen 1.1，本期最核心的缺口）：`prepare_outbound` 在盲目录交付后往 prompt 追加一行「待验材料已在工作目录：REQUEST.md（用户原话）、diff.patch（统一 diff）、changed/（改后文件全文），请先读取再判定」；带 `upstream_artifacts` 的盲节点若通道是 `--no-tools`（pi 盲），改为放开只读工具 `--tools read,grep,find,ls`；diff 总量 < 30KB 时同时内联进 prompt。补测试：盲验收 prompt 含该提示行；pi 盲验收 argv 含只读工具。
2. **写权限只对 codex 有效**（qwen 1.6，与 GLM 2.2 同源但要求不同）：非盲且 `writable` 时 claude 加 `--tools Read,Grep,Glob,Write,Edit,Bash`，pi 加写工具集；`validate.ts` 对非 codex 的 `单写者/worktree` 不再阻止但 G1 提示已在 R1 做。补 argv 测试。
3. **链式启发式对经济预设是死代码**（qwen 1.5）：条件放宽到 `关|低`，抬到「中」并显示理由；预设默认值不改。
4. **G4 指定上游断链**（qwen 1.11）：`appendCorrectionRound` 把新执行节点的 `upstreams` 重写为 `[acceptId, previous.id]`。
5. **`artifact_diff` 不是可用 diff**（qwen 2.1）：项目是 git 仓库时用 `git diff --no-index` 生成真 unified diff（带 3 行上下文、保留 CRLF），否则退回现有实现但对单文件 diff 设上限（超限只列文件名 + 「全文见 changed/」）；kimi 通道 30000 字节硬限前先裁。
6. **快照跟随符号链接、无上限、失败被吞**（qwen 2.2）：`symlink_metadata` 跳过链接与 junction；排除名单加 `.venv build coverage .next vendor`；单文件与总字节上限；快照失败 `status=error` 而不是产物为空。
7. **worktree 回退后调度记账**（qwen 2.3）：调度前探测 `git rev-parse --is-inside-work-tree`，非 git 项目的 worktree 节点按单写者排队。
8. **写锁忙等**（qwen 2.5）：换每项目 `Mutex` 阻塞获取（公平排队），加 `timeout_secs` 超时报错；取消能中断等锁。
9. **stage 逐节点**（qwen 2.6）：`DispatchNodeRequest.stage` 按节点深度，记录与用量表用节点自己的 stage。
10. **确认清单与实际派发集合一致 + 总请求上限**（qwen 2.7）：第一轮就用 `dispatchableNodeIds` 作 preview 输入；每层派发前刷新 preview；加总请求上限（默认 8，设置可改），超出停下来问。
11. **G4 批量处理缺陷**（qwen 2.8、2.9、2.10）：`addedCorrection` 用或运算累计；`validation_failed` 逐节点 `appendValidationRetry`，重试计数按节点；验证失败时保留模型正文，验证输出单独拼在末尾并标注。
12. **identity_mismatch 全链路单独显示**（qwen 1.8）：`Reply` 追加可选 `error_kind`（写进 REPORT 的契约追加节），STATE.md 与节点卡状态词扩充。
13. **结果屏按阶段分栏**（qwen 1.4）：按 `result.stage` 分组，栏标题取该阶段首个节点的 role，无名退化为「第 N 阶段」；网格列数按阶段数。
14. **cc-switch usage 测试样本换成真实输出行**（qwen 1.9）：`[usage] provider=… model=… input=… fresh=… cache_create=… cache_read=… output=… turns=… ms=…`；`live_cc_switch_route` 加 `tokens_in > 0` 断言（编排者已真跑：in 7531 / out 63）。
15. **验收 3 盲目录断言**（qwen 1.10）：改为「三项都在，且不存在来自项目目录的其他文件」。
16. **live harness prompt 共用模板**（qwen 1.7）：三段模板抽成 `fixtures/prompts/*.txt`，TS 与 Rust 两侧各自读；harness 用它拼。

## 不采纳

- 无。qwen 审查其余章节（3–6）若有 R1 已覆盖的条目，在报告里注明「R1 已修」即可。
