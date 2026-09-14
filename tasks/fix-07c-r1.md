# TASK-07C 修正轮 R1

先读 `tasks/TASK-07-idle-and-tail.md`（契约：终态事件不带心跳字段，前端保留上一条 running 的 elapsedMs/lastTool）、`tasks/REPORT-07C.md`，再读 `tasks/review-07c-ds.md`（与 `tasks/review-07c-glm.md`，若存在）。逐条处理，每条在 `tasks/REPORT-07C.md` 末尾「修正轮 R1」一节写「已修：怎么改的（文件:行）／已补：测试名」或「不采纳：原因」，并更正报告里 F1「冻结」、F4「胶囊完整显示」两处与事实不符的陈述。验证：`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`，命令与输出原文贴进报告。不动 `src/`；保持文件原有换行；不起窗口、不 commit、不 push。

## 必修（按严重度）

1. **终态事件保留上一条心跳读数**（DS F1）：`applyProgressEvent`/`nextProgress` 收到无心跳字段的终态事件时，只更新 status，保留上一条 running 的 `elapsedMs`/`lastTool`/`toolCalls`（`hasHeartbeat` 沿用）；终态耗时按 R1 语义冻结。补测试「无字段终态事件后耗时与工具仍显示且不再增长」。
2. **listen 取消守卫**（DS F2）：hook 的异步 `listen` 用 `cancelled` 标志 + 卸载/换 runId 时对已解析的 unlisten 调用；把 effect 体抽成可测的 `subscribeProgress(runId, onEvent) -> { dispose }`，在 node 环境用 mock 订阅断言：runId 变更与卸载各 unlisten 一次、迟到 resolve 的 listen 立即被解除。
3. **异常收尾后时钟停**（DS F3）：`start()`/`retry()` 的 catch/finally 把本轮未收到终态的节点快照标成终态（或清空本轮），5s 时钟随 `progressRunningAny` 停；删掉死接口 `resetRun` 或接上用。补测试（fake timers）：全终态/异常收尾后 `now` 不再前进、不挂胶囊。
4. **胶囊不裁切**（DS F4）：节点卡第三行改成可换行或省略中间、`line-height` 显式；最长文案「疑似空转：同一工具连续 N 次」在卡宽内完整显示（字符串/样式断言 + 报告如实写「未起窗口目视」）。
