# 任务书与交付流程

| 期 | 任务书 | 内容 | 状态 |
|---|---|---|---|
| 0 | `TASK-00-skeleton.md` | 三栏 tab、终端 tab、文件树、修改记录、会话记录 | 代码验收完成（三轮），窗口手测待用户 |
| 1 | `TASK-01-resources.md` | agent→模型两级、四事实源探测、手动输入、冒烟、`resources.json` v1 | 代码验收完成（三轮），窗口手测待用户 |
| 2 | `TASK-02-orchestration.md` | 工作流数据模型、一行记法、阶段条、构建器、预设、讲解页 | 待派 |
| 3 | `TASK-03-dispatch.md` | 通道执行器、盲审真跑、`.dispatch/` 契约、G1/G2/G3、共享对话续聊 | 待派 |
| 4 | `TASK-04-pipeline.md` | 阶段传递、执行产物交付验收、经济规则、闭环 G4 | 待派 |
| 5 | `TASK-05-acp-and-polish.md` | ACP 长驻、第 2 期预设、打包与手册 | 后置 |

## 每期流程

1. 实现：`codex exec -m gpt-5.6-sol -c model_reasoning_effort="high" -s workspace-write -c sandbox_workspace_write.network_access=true --json -`，prompt 从 stdin：「按 AGENTS.md 与 tasks/TASK-NN-*.md 实现第 N 期，完成后写 tasks/REPORT-NN.md，不要 commit」。
2. 编排者独立复跑 `cargo test` / `cargo fmt -- --check` / `npm run build`，全绿才 commit。
3. 盲审：`pi --provider zai-coding-cn --model glm-5.2 -p --tools read,grep,find,ls --no-extensions --no-context-files --thinking medium --mode json < tasks/review-NN-prompt.txt`，六个维度 + 「是否完整实现」。
4. 修正轮：把审查文件交回 Codex，逐条「已修 / 不采纳+原因」，重跑验证，追加到 REPORT。
5. 复审 → 修 → 定向复核，最多三轮；仍分歧交用户。
6. 需要真机的验收项（发模型请求、pi 锁目录）Codex 沙箱跑不了，由编排者在沙箱外补跑并记进 REPORT 或 PROGRESS。

## 门槛

- 第 2 期开工前：用户先在窗口里过一遍第 0、1 期的手动 smoke 清单。
- 第 3 期开工前：`.dispatch/` 契约（TASK-03 C 节）同步进 skill 的 `channels.md`（skill 侧一个版本）。
- 第 4 期开工前：第 3 期的盲审真跑在编排者机器上通过。
