# TASK-04 修正轮 R4（R3 系列复审的收尾）

先读 `tasks/review-04-ds-r3.md`（DeepSeek 复审）与 `tasks/review-04-glm-r3.md`（GLM 复审），再读 `tasks/REPORT-04.md` 的 R3/R3b/R3c 三节。两家都判「可交付」，本轮只收下面这些尾巴，在 `tasks/REPORT-04.md` 末尾新增「修正轮 R4」一节逐条记录。验证跑 `cargo check`（**不得有 warning**）、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`，命令与输出原文贴进报告。不起窗口，不发真机请求，不 commit，不 push。

## 必修

1. **B16 worktree 回退后仍跳过验证**（DS 1）：`successful_validation_output` 按请求里的 `node.write=="worktree"` 判断，但 worktree 创建失败/非 git/git 不可用时会退回单写者直接改项目（`dispatch.rs` 回退处 `node.write` 不变）。改为按**实际生效**的写模式判断：回退后照常跑验证命令；只有真的在 worktree 里跑完才写「worktree 节点未跑验证命令，采纳合并后再验」。补测试：回退路径下 `validation_output` 不含「未跑验证」且验证命令被执行（用 `AICROSS_TEST_RUN_ROOT` 套路 + 一条必失败/必成功的验证命令）。
2. **B14 前端盲节点只读校验缺口 + 假绿测试**（DS 2）：`validate.ts` 加 `visibility=盲 && write!=只读` 分支；`validate.test.ts` 那条盲用例改成 `role=执行, visibility=盲, write=worktree` 必须报错（现在返回空数组）。后端对「验收/验证非只读」不要整层 Err：改为该节点级 `status=error`（与 R3-3 的空 agent 同一取向），其余节点照常派。补测试。
3. **B13 git 不可执行**（DS 3）：测试要按修正单注入假 git 路径（`project_is_git_work_tree` 用不存在的可执行文件），断言返回 false 且走回退 diff；另外探测通过后 `git diff --no-index` 本体失败（非 0/1 退出码或进程错误）也要回退内置 diff 而不是把跑完的节点判 error。补测试。
4. **R3-19 报告两段与代码不符**（DS 4）：`REPORT-04.md` 修正轮 R1 第 5 条、第 14 条按现状重写（第 5 条：G1 提示已改为 cc-switch/kimi 两条通道提示，角色填充不再限 codex；第 14 条：确认清单改为只列 `dispatchableNodeIds`，不再有「待上游完成」标注）。
5. **测试欠账**（DS 5）：R3-20 小任务启发式补三类用例（194 字长句、含多步词、含换行都不判小任务）；R3-21 补「内容行含 `a/before/` 不被改写」断言；推进循环抽纯函数 `nextStageStep(graph, collected, settings)` 返回「派这层 / 停在未指定 / 达上限 / 结束」四态并测试，`DispatchOverlay.tsx` 的 `start()` 改为调用它。
6. **编译 warning**（DS 6）：`orchestration.rs` 两处未使用的 `models` 清掉，`presets_from` 不再白读 resources.json。`cargo check` 输出必须零 warning（本仓库自己文件范围内）。
7. **「继续」提示**（DS 7）：`GraphTab.tsx` 未完成 run 的提示区加一句「若上次是崩溃退出，请先手动检查残留进程」。
8. `DispatchArtifacts.diff` 加 `#[serde(default)]`，与 `orchestration.rs` 对称（DS 8）。

## 待 GLM 复审补充

GLM 与 DeepSeek 的结论一致（可交付；validate.ts 盲分支、unused `models`、小任务测试、REPORT 措辞四条两家都点了）。GLM 额外的：

9. `REPORT-04.md` 现第 315 行附近仍写「任务书允许的 fallback」，改为「修正单允许的」（与第 4 条一起做）。
10. `r3_13`（materialize 清理）与 `r3_14`（盲节点不拿写锁）两条断言比补课单弱：清理断言要走 `artifact_diff_for_project` 这条真实路径（让 materialize 失败后断言 `diff-*` 不存在）；盲节点断言要用写锁测试套路证明第二个盲节点不排队。
11. G1 路由提示测试补 kimi 盲的「隔离=部分」断言。
12. `record_stdout` 写 raw 文件失败时不要 `let _ =` 吞掉：留痕里写「原始输出转存失败：<错误>」并保留末尾 4KB。
