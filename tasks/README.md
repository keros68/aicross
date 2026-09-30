# 任务书与交付流程

## 状态

TASK-00 至 TASK-35 均已合并 main（最后一期 TASK-35，09-21 `dcaf39c`）；每期的实现、审查与修正记录在本目录的 `REPORT-NN*.md`、`review-NN-*.md`、`fix-NN-*.md`，进度流水在 `D:\work\agentteam\archive\PROGRESS-0707-0930.md`。

当前：TASK-36（引擎无窗口可跑，`TASK-36-headless-engine.md`）。低危积压统一记在 `fix-04-r5-backlog.md`，随 09-30 定案作废的条目已划掉并注明。

## Runbook（编排者操作手册，2026-09-15 定稿）

主线程只做四件事：写任务书、独立复跑、裁决与合并、窗口真跑。实现与审查全部外派；主线程不长时间自己写代码，这样用户随时能和主线程交流而不打断在跑的任务（用户 09-15 指出的好处）。

**一期的步骤**

1. **任务书** `tasks/TASK-NN-*.md`：契约（字段、命令签名、wire 大小写）只追加；范围、禁区、验证命令、换行规则、「不 commit 不 push」写死；需要窗口核的项列成清单。写文件用 Write/Edit 工具，不用 shell heredoc（heredoc 会把 `\\` 折成 `\`，09-15 两次把 Windows 路径写坏）。任务书先提交到 main，再从 main 开分支给实现者。
2. **派实现**（各自的 git worktree，一个树一个写者）。**实现者顺序：kimi → GLM → Claude 子代理 → codex**（用户 09-30：codex 的 sol 模型近期质量差，只作兜底）：
   - kimi：5 小时额度窗口；无 stdin，prompt 走 argv 一句话（指向任务书路径）；`-p` 下工具默认自动执行，不能与 `-y`/`--auto` 同用。
   - GLM：经 pi 的 `zai-coding-cn` provider（`glm-5.3`），在工作树目录跑，prompt 走 stdin：`pi --provider zai-coding-cn --model glm-5.3 -p --mode json --tools read,write,edit,bash,grep,find,ls --no-extensions --no-session < prompt.txt > out.jsonl`。09-30 冒烟通过（小仓里修 bug + 建测试 + 跑通，19 秒）。必须带 `--no-extensions`：不带时用户级扩展 `advanced-footer.ts` 在收尾崩溃，活干完了退出码却是 1。这条通道的事件里没有 `responseModel`，只有请求回显的 `model`，真身核对不了，报告里标「真身未核对」。
   - Claude 子代理（Agent 工具，general-purpose）：适合前端 + 设计 skill；会撞会话额度，撞了按「限额即换家」交给下一家，让它先 `git status / git diff` 通读再续。
   - codex（兜底）：`codex exec -m gpt-5.6-sol -s workspace-write --json - < prompt.txt`，在工作树目录跑；完成判据是 jsonl 里出现行首 `{"type":"turn.completed"`（松匹配 `"type":"error"` 出过假阳性）。
   - 审查者必须与实现者不同厂商：GLM 实现时，第 4 步的 GLM 盲审换成 kimi 或 Claude。
   - 排期先看文件重叠：两期会改同一批文件的串行，不并行。
3. **独立复跑**（不信报告）：`cargo check --all-targets` 零 warning、`cargo test`、`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`；再用 python 数每个改动文件的 CRLF/LF 与 HEAD 比对（`server.rs` CRLF，其它 Rust LF，`src-ui/src/tauri.ts` 混合）。复跑与盲审同时起，不串行。
4. **盲审**（两家、只读、不带主线程判断；提示词写到 scratchpad 的 task 文件，只描述契约与关注点）：
   - GLM-5.3-Flash：`python D:\work\skill\xiaoyu-skill\skills\ai-cross\references\cc_switch.py exec --provider "Zhipu GLM" --tier haiku --task-file <file> --timeout 1500`（默认工具 Read,Grep,Glob；15–25 分钟）。
   - 第二家默认是 **Antigravity 的 Gemini**（订阅额度，本机原本没有的厂商轴）：材料**全部内联**进 prompt，一行 NDJSON 走 stdin——
     `cat in.ndjson | "C:/Users/keros68/AppData/Local/agy/bin/agy.exe" --model gemini-3.8-flash-high --print-timeout 10m --input-format stream-json --output-format stream-json`，
     **用 3.8 Flash 档，不要用 3.1 Pro**（用户 09-16 指定）。`in.ndjson` 是一行 `{"event":"user","message":{"role":"user","content":"…"}}`，结果取 `{"event":"result"}` 行的 `result.response`。
     **它在无头模式下读不了文件**（权限被自动拒），所以 `git diff`、任务书、相关源码都要内联进去；prompt 里明写「不要使用任何工具」。别加 `--dangerously-skip-permissions`。
   - DeepSeek（`pi --provider deepseek --model deepseek-v4-flash -p --mode json --tools read,grep,find,ls --no-extensions --no-context-files --no-session`，用 `ds_extract.py` 取文本）**按次真扣钱，排最后**：
     只在 GLM 与 Antigravity 都不可用、或确实需要它的视角时才派，并在汇报里写明为什么非它不可（用户 09-16 明确要求少用）。
   - 审查员没有 shell，「测试能否红」是静态推演，主线程要自己验（见 6）。
5. **修正轮**：两份审查合成 `tasks/fix-NN-r1.md`（标出两家共同 / 各自独有；编排者已核实的条目注明；「不改」项写原因），按第 2 步的实现者顺序派（一般交回原实现者，它熟悉这批改动）。**只做一轮复审。** 复审新出的中等项若改动小，主线程直接修并附回归测试；低危进 `tasks/fix-04-r5-backlog.md` 编号续排，不派第三轮。
6. **自己验能不能红**：修正轮新增的关键测试，把修复暂时回退再跑一次，红了才算数（08D-R1 这样验过一次）。
7. **合并**：实现者不 commit，由主线程在其分支提交（提交信息带 Co-Authored-By 与 Claude-Session 两行）→ `git merge --no-edit <branch>` 到 main → 合并后再跑一遍第 3 步 → `git push origin main`（用户已授权直接推）。合并命令和后续的 `git add -A && git commit` 不要写在同一条 `&&` 链里：合并冲突时 add -A 会把带冲突标记的文件直接提交（09-15 在 MANUAL.md 上发生过一次）；先看 `git status` 有没有 UU，再解决、再提交。审查文件归档到 `tasks/review-NN-*.md`，报告补记主线程的追加改动。
8. **窗口真跑**（只有主线程能做，抓的是测试与审查都不覆盖的那类问题）：dev 窗口 `npx --yes -p @tauri-apps/cli@2 tauri dev`（根目录，日志落 scratchpad）；写仓库或跑 build/vitest 会触发重建，重建期间不操作窗口；kimi-cu 坐标空间 image 1568×882 / ax 2018×1135；输入用 `set_value` + 点击 + Enter。按任务书的清单逐项核，发现的缺口直接修在 main 或开小任务。
9. **记录**：每次状态变化写 `D:\work\agentteam\PROGRESS.md`（Current / Completed / Next / Blocker）；产品决定写 `docs/PRODUCT.md`；新规则写 `AGENTS.md`。上下文被压缩后靠这三处 + `tasks/` 接手，不靠记忆。

**不再做的事**：`cargo fmt`（上游文件 263 处会翻面）；三轮复审；等某家额度恢复；主线程自己实现整期。

**真机验收项**（发模型请求、pi 锁目录、ACP 续聊）沙箱跑不了，由主线程在沙箱外补跑并记进 REPORT 或 PROGRESS。
