# TASK-07B 修正轮 R1（界面：空转判定/轮次复位）

先读 `tasks/TASK-07-idle-and-tail.md`（契约 + B 节）、`tasks/REPORT-07B.md`，再读 `tasks/review-07b-ds.md`（与 `tasks/review-07b-glm.md`，若存在）。逐条处理，每条在 `tasks/REPORT-07B.md` 末尾「修正轮 R1」一节写「已修：怎么改的（文件:行）／已补：测试名」或「不采纳：原因」。验证：`cd src-ui && npx vitest run --configLoader native --pool threads`、`cd src-ui && npm run build`，命令与输出原文贴进报告。不动 `src/`；保持文件原有换行（`tauri.ts` 混排，用字节级改法）；不起窗口、不发真机请求、不 commit、不 push。

## 必修（按严重度）

1. **无心跳元数据的快照不累计墙上时间**（DS 2.1，高）：`ProgressSnapshot` 加 `hasHeartbeat`（`elapsedMs`/`idleMs` 任一存在），`idleHint` 对无元数据快照只显示状态、不判空转；补一条「缺字段快照 + 3 分钟后仍为 null」的测试（现状会红）。
2. **新一轮复位进度状态**（DS 2.2，中）：`start()` 与 `retry()` 清 `progress`/`statuses`/`currentNodes`（或快照带 runId、渲染时过滤）；把 progress 更新抽成纯函数 `nextProgress(current, payload, now, runId)` 并测「重派后不显示上一轮读数、不立刻出胶囊」。
3. **重复工具计数加空闲门槛**（DS 2.4）：`repeatedToolCalls ≥ 3` 需同时 `idleMs ≥ 30000` 才提示（契约文件同步改一句：「同名工具连续 ≥3 且 30 秒无输出」）；引擎侧按尝试归零已在 07A R1 做。补测试。
4. **终态快照冻结耗时**（DS 2.3）：`runningDetailOf` 在非 running 时返回空串或冻结 `receivedAt`；补测试。
5. **`reconcileUiConverged` 按终态过滤**（DS 2.5）：只剔除已 done/failed 或 pending 的 id，保留仍为 `RECONCILE_UNKNOWN_TEXT` 的 draft；注释与代码一致；补测试。
6. **节点卡口径写回任务书**（DS 1.1）：本期 B-1 只覆盖 G2 运行行，在 `tasks/TASK-07-idle-and-tail.md` B-1 加括注「节点卡的 elapsed/lastTool 下期做（需把 dispatch-progress 订阅提到 GraphTab）」；不在本轮大改。
7. **测试补强**（DS 5.3–5.5）：运行行渲染断言「叫停」文案与 `cancelDispatchNode` 接线；`runStageLoop` 的 `limit` 分支与 `dispatch` 抛错路径（state 形状）；`DUPLICATE_BATCH_TEXT` 用字面量断言；GraphTab 收敛接线用读源文件断言；源码扫描断言改宽松正则；`hint-capsule` 注释改「提示胶囊」。

## GLM 补充

（编排者在 GLM 盲审到达后追加；若为空即以上为全部。）
