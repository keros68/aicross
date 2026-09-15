# 任务书与交付流程

| 期 | 任务书 | 内容 | 状态 |
|---|---|---|---|
| 0 | `TASK-00-skeleton.md` | 三栏 tab、终端 tab、文件树、修改记录、会话记录 | 代码验收完成（三轮），窗口手测待用户 |
| 1 | `TASK-01-resources.md` | agent→模型两级、四事实源探测、手动输入、冒烟、`resources.json` v1 | 代码验收完成（三轮），窗口手测待用户 |
| 1B | `TASK-1B-visual.md` | 界面对齐到 08-31 原型：视觉纪律、外壳与标签栏、左栏只列可派发、接入卡片页 | 已提交（a73c3e8），未盲审，窗口手测待用户 |
| F1 | `TASK-F1-rebrand.md` | fork Coffee CLI 后改名换标，履行 AGPL 与商标义务 | 已提交（a73c3e8），未盲审 |
| F2 | `TASK-F2-decouple.md` | 切断与上游的运行时冲突：不自动写外部 CLI 配置，状态迁到 `~/.aicross` | 已提交（a73c3e8），未盲审 |
| 2 | `TASK-02-orchestration.md` | 对话图（节点 / 分叉 / 扇入 / 三列面板）、上下文计算、预设图章、一行记法、构建器、讲解页。09-13 按「对话即图」重写 | 代码验收完成（kimi 实现 → GLM-5.3-Flash 盲审 → kimi 修 R1 → GLM 复审 12 条全核实），编排者复跑 cargo 93 / vitest 51 / build 全绿，窗口手测 10 项全过（REPORT-02 末节）。未提交 |
| 3 | `TASK-03-dispatch.md` | 通道执行器、盲审真跑、`.dispatch/` 契约、G1/G2/G3、共享对话续聊。09-13 加 0 节对接对话图 | 代码验收完成（codex 实现 → GLM + qwen 双盲审 → codex 修 R1 23 条 → 编排者补修 kimi 解析/adopted/正文脱敏 → GLM + qwen 复审放行）。真机：三路盲审 ok、续聊 ok、kimi `-S` 续聊 ok。提交 09-13 |
| 4 | `TASK-04-pipeline.md` | 阶段传递、执行产物交付验收、经济规则、闭环 G4。09-13 加 0 节衔接与第 3 期遗留 | 代码验收完成（codex 实现 → GLM + qwen 双盲审 → codex 修 R1 17 条 + R2 16 条）。真机：经济流水线 3 次请求 ok、cc-switch 路 ok。**R2 之后未再派复审、未做窗口手测**，09-14 首件事 |
| 5 | `TASK-05-acp-and-polish.md` | ACP 长驻、第 2 期预设、打包与手册 | 后置 |

## Runbook（编排者操作手册，2026-09-15 定稿）

主线程只做四件事：写任务书、独立复跑、裁决与合并、窗口真跑。实现与审查全部外派；主线程不长时间自己写代码，这样用户随时能和主线程交流而不打断在跑的任务（用户 09-15 指出的好处）。

**一期的步骤**

1. **任务书** `tasks/TASK-NN-*.md`：契约（字段、命令签名、wire 大小写）只追加；范围、禁区、验证命令、换行规则、「不 commit 不 push」写死；需要窗口核的项列成清单。写文件用 Write/Edit 工具，不用 shell heredoc（heredoc 会把 `\\` 折成 `\`，09-15 两次把 Windows 路径写坏）。任务书先提交到 main，再从 main 开分支给实现者。
2. **派实现**（各自的 git worktree，一个树一个写者）：
   - codex：`codex exec -m gpt-5.6-sol -s workspace-write --json - < prompt.txt`，在工作树目录跑；完成判据是 jsonl 里出现行首 `{"type":"turn.completed"`（松匹配 `"type":"error"` 出过假阳性）。修正轮只用 sol（luna 在修正轮敷衍过：零测试、推掉必修）。
   - Claude 子代理（Agent 工具，general-purpose）：适合前端 + 设计 skill；会撞会话额度，撞了按「限额即换家」交给 codex 接手，让它先 `git status / git diff` 通读再续。
   - kimi：5 小时额度窗口；无 stdin，prompt 走 argv 一句话。
   - 排期先看文件重叠：两期会改同一批文件的串行，不并行。
3. **独立复跑**（不信报告）：`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`；再用 python 数每个改动文件的 CRLF/LF 与 HEAD 比对（`server.rs` CRLF，其它 Rust LF，`src-ui/src/tauri.ts` 混合）。复跑与盲审同时起，不串行。
4. **盲审**（两家、只读、不带主线程判断；提示词写到 scratchpad 的 task 文件，只描述契约与关注点）：
   - GLM-5.3-Flash：`python D:\work\agentteam\dist\ai-cross\references\cc_switch.py exec --provider "Zhipu GLM" --tier haiku --task-file <file> --timeout 1500`（默认工具 Read,Grep,Glob；15–25 分钟）。
   - DeepSeek：`pi --provider deepseek --model deepseek-v4-flash -p --mode json --tools read,grep,find,ls --no-extensions --no-context-files --no-session "<prompt>"` 输出 jsonl，用 `ds_extract.py` 取最后一条 assistant 文本。官方账户 402 时先告诉用户。
   - 审查员没有 shell，「测试能否红」是静态推演，主线程要自己验（见 6）。
5. **修正轮**：两份审查合成 `tasks/fix-NN-r1.md`（标出两家共同 / 各自独有；编排者已核实的条目注明；「不改」项写原因），派 codex sol。**只做一轮复审。** 复审新出的中等项若改动小，主线程直接修并附回归测试；低危进 `tasks/fix-04-r5-backlog.md` 编号续排，不派第三轮。
6. **自己验能不能红**：修正轮新增的关键测试，把修复暂时回退再跑一次，红了才算数（08D-R1 这样验过一次）。
7. **合并**：实现者不 commit，由主线程在其分支提交（提交信息带 Co-Authored-By 与 Claude-Session 两行）→ `git merge --no-edit <branch>` 到 main → 合并后再跑一遍第 3 步 → `git push origin main`（用户已授权直接推）。合并命令和后续的 `git add -A && git commit` 不要写在同一条 `&&` 链里：合并冲突时 add -A 会把带冲突标记的文件直接提交（09-15 在 MANUAL.md 上发生过一次）；先看 `git status` 有没有 UU，再解决、再提交。审查文件归档到 `tasks/review-NN-*.md`，报告补记主线程的追加改动。
8. **窗口真跑**（只有主线程能做，抓的是测试与审查都不覆盖的那类问题）：dev 窗口 `npx --yes -p @tauri-apps/cli@2 tauri dev`（根目录，日志落 scratchpad）；写仓库或跑 build/vitest 会触发重建，重建期间不操作窗口；kimi-cu 坐标空间 image 1568×882 / ax 2018×1135；输入用 `set_value` + 点击 + Enter。按任务书的清单逐项核，发现的缺口直接修在 main 或开小任务。
9. **记录**：每次状态变化写 `D:\work\agentteam\PROGRESS.md`（Current / Completed / Next / Blocker）；产品决定写 `docs/PRODUCT.md`；新规则写 `AGENTS.md`。上下文被压缩后靠这三处 + `tasks/` 接手，不靠记忆。

**不再做的事**：`cargo fmt`（上游文件 263 处会翻面）；三轮复审；等某家额度恢复；主线程自己实现整期。

**真机验收项**（发模型请求、pi 锁目录、ACP 续聊）沙箱跑不了，由主线程在沙箱外补跑并记进 REPORT 或 PROGRESS。

## 门槛

- 第 2 期开工前：用户先在窗口里过一遍第 0、1 期的手动 smoke 清单。
- 第 3 期开工前：`.dispatch/` 契约（TASK-03 C 节）同步进 skill 的 `channels.md`（skill 侧一个版本）。
- 第 4 期开工前：第 3 期的盲审真跑在编排者机器上通过。
